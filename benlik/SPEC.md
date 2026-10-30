# Benlik Card — Specification

**Status:** Draft 0.1.1, one reference generator so far (not yet a standard; see §11) · **Editor:** Benlik Card contributors · **License:** text CC BY 4.0, schema and examples Apache-2.0

> *Benlik* is Turkish for "self". A Benlik Card is an application's signed self-description for AI agents.

## 1. Purpose

An AI agent arriving at an application today either reads its source code, guesses from incomplete API documentation, or waits for someone to write an integration. All three are slow, expensive and error-prone.

A **Benlik Card** is a single machine-readable file, generated from the application's own code, that tells an agent:

- what the application is and which operations exist,
- what each operation needs, which rules can reject it, which roles it requires and what side effects it has,
- under whose identity writes happen,
- what data is stored and for how long,
- what changed since the previous version,
- and — unlike existing API descriptions — **how each fact is known, and what is not known.**

| | Tells an agent |
|---|---|
| `robots.txt` | where not to go |
| OpenAPI | which endpoints exist and their shapes |
| **Benlik Card** | what it can do, under which rules, on whose behalf, and how certain that knowledge is |

A Benlik Card complements OpenAPI and MCP; it does not replace them. A generator SHOULD also emit OpenAPI and MCP tool descriptions from the same knowledge.

The key words MUST, MUST NOT, SHOULD and MAY are to be interpreted as described in RFC 2119.

## 2. Location and discovery

An agent looks for a card in this order:

1. `https://<application>/.well-known/benlik.json`
2. `benlik.json` at the repository root — or `.sezgi/benlik.json` — in the source repository, when the owner commits it. (Sezgi writes its output to `.sezgi/` and marks that folder git-ignored, so a card reaches the repository only by a deliberate commit.)
3. The description of an MCP server that declares it serves the application

Publishing a card at `/.well-known/` is the application owner's decision. A generator MUST NOT publish it without explicit consent, and the owner MAY hide operations from a published card.

## 3. Stamps

Every fact in a card carries a **stamp** describing how it is known:

| Stamp | Meaning | May enable a write |
|---|---|---|
| `certain` | Extracted from code, tests, schemas or UI text | Yes |
| `approved` | Proposed by an agent or tool and approved by a human | **No** (since 0.1.1) |
| `data` | Read from a configuration table the owner selected | **No** (since 0.1.1) |
| `inferred` | Proposed, not yet approved | **No** |
| `unknown` | Could not be determined | **No** |

Rules:

- Order from strongest to weakest: `certain` > `approved` > `data` > `inferred` > `unknown`.
- An operation's overall `stamp` MUST be the weakest stamp among the facts a write would rely on (inputs, rules, login requirement).
- An agent MUST NOT perform a `write` or `destructive` operation whose overall stamp is not `certain`. (Draft 0.1 also allowed `approved` and `data`; since 0.1.1 they count as `observed` on the trust scale of §3.3, and an observation never enables a write on its own. Approval of a fact is recorded separately from how the fact is known.)
- Facts SHOULD carry `sources` (file, lines, kind) so a human can verify them.
- Unknown facts MUST be shown as gaps, never filled with guesses.

### 3.1 Effect

`effect` (`read`, `write`, `destructive`) tells an agent how careful to be. When a generator derives it only from the HTTP method, `effect_stamp` MUST be `inferred`: a `POST /search` may only read, a `GET` may change state. `requires_approval` states whether a write would wait for a human before it is sent, as far as the generator can tell from the code: it MUST be `true` for destructive operations, for writes whose overall stamp is not `certain`, and for writes that require roles (whether the human holds a role is known only at run time, §4). A `certain`, role-free write MAY carry `false`; the decision is completed at run time, where a write still waits for a human when the preview could not check every known rule or a required role is not verified for the human. An agent MUST NOT treat an operation with `effect: read` and `effect_stamp` `inferred` or `unknown` as harmless: it either treats it as possibly state-changing or sends it only through the application's own entry point with the human's own session, so the application's authorization decides (the reference implementation does the latter for `GET` and `HEAD`).

### 3.2 Extensions

Members whose names start with `x-` MAY be added to the card, `app`, operations and fields. Consumers MUST ignore extension members they do not understand. Everything else is closed (`additionalProperties: false`), so typos are caught. Extension members are covered by the signature like every other member.

Draft 0.1.1 defines these extension members (the schema describes them; a 0.1 validator accepts a 0.1.1 card because `benlik` matches `0.1.x` and the members start with `x-`):

| Member | Where | Section |
|---|---|---|
| `x-issued_at`, `x-expires_at` | card | §8 |
| `x-basis` | card | §8 |
| `x-revision` | card | §7 |
| `x-evidence`, `x-evidence_root`, `x-evidence_bundle` | card | §8.1 |
| `x-dna` | operation | §3.4 |
| `x-response` | operation | §3.6 |
| `x-journeys` | card | §3.5 |
| `x-nodes`, `x-relationships` | card | §3.7 |
| `x-sourceless`, `x-app_node`, `x-preview`, `x-capabilities`, `x-screens`, `x-webmcp` | card | §3.8 |
| `x-meaning` | `app` | §3.9 |
| `x-unread_route_sources` | card | §6 |

### 3.3 Trust scale of extension members

Extension members use one scale: `certain` > `observed` > `inferred` > `unknown`. `observed` means seen at run time (a browser, a network capture, a test run) or read from shipped files (§3.8), but not derived from the source; it never enables a write on its own. Members of the 0.1 view (`stamp` everywhere outside `x-` members) never carry `observed`: a generator writes it as `inferred` there. `approved` and `data` exist only in the 0.1 view.

### 3.4 Behaviour (`operation.x-dna`)

`x-dna` summarizes how an operation behaves: `auth` (`required`: `yes`, `no`, `optional` or `unknown`, with a stamp), `preconditions` and `failure_modes` (`condition`, `result`, `stamp`; a failure mode found in the operation's own handler MAY be `certain`, one reached through a service or dependency the operation calls is `inferred`), `data_effect` (`reads`, `writes`, `stamp`, and `indirect`: tables changed indirectly such as rows a database cascade deletes, each with `table`, `reason`, `stamp` and `sources`; an indirect effect is derived from the schema or from migration files, never from a running database, and its stamp is never `certain`), `branches` (conditions that split the behaviour into two paths: `condition`, `when_true`, `when_false`, `stamp`, `sources`; the condition is code text and carries no request values), `reversibility` (`reversible`, `compensable`, `irreversible` or `unknown`), `risk` (`class` and its `basis`), `requires_approval`, `coverage` and `evidence_refs`.

- `coverage` marks every section the generator could not close as `open`. An open section is not evidence of absence: an empty `failure_modes` with `coverage.failure_modes: "open"` means "not known", not "cannot fail".
- `evidence_refs` name entries of `x-evidence` (§8.1); a ref that is not there makes the card invalid.
- `auth.object_level` lists object-level authorization found along the call chain: `kind` (`access_check` on specific records, `ownership` comparing the record's owner field with the requester, `ownership_filter` in a query, `authorization` without a record), `expression` (code text), optional `permission`, `stamp`, `sources`. A uniqueness check or a comparison with the caller's own id is not ownership. Its outcome depends on the record and the caller, so a preview never passes an operation that has one. `call_chain` summarizes the static call graph from the handler (`functions` on the path to the data layer, `reached`, `unresolved_calls`, `stamp` of the weakest call on the path).

### 3.5 User journeys (`x-journeys`)

`x-journeys` lists user journeys that end in an operation: `id`, `name`, `operation` (an `operations[].id`), `stamp` and ordered `steps` (`order`, `node`, `kind` such as `screen` or `ui`, `stamp`). Steps derived from the source are at most `inferred`. A journey whose request sequence was observed at run time MAY be `observed`; its UI steps stay `inferred` unless the UI steps themselves were observed (a network capture alone does not observe them; a browser run that links each UI step to the request it started does, and the reference generator writes such a step `observed` only when the run was declared isolated in a container or virtual machine). Observations carry no values (§9). A journey never enables a write.

### 3.6 Response schema (`operation.x-response`)

`x-response` describes the successful response read from code: `type` (schema name), `array`, `status` (when known), `fields` (`type`, `required`), `stamp`, `basis` (where it was read: e.g. `response_model`, a return type, a handler guess) and `sources`. It is `certain` only for an explicit declaration or a type the framework enforces; a return type or a guess from the handler is `inferred`. The schema file is part of `x-basis`. An observed response that disagrees with it is a contradiction (§3.3).

### 3.7 Events and jobs (`x-nodes`, `x-relationships`)

Internal events and queued jobs are not operations: an agent cannot call them, but an operation can set them off. `x-nodes` lists them: `type` (`event` or `job`), `id` (`event:<name>` or `job:<name>`), `name`, `handlers` (the listening functions), `stamp` and `sources`. `x-relationships` lists edges: `from` (an `operations[].id` or a node id), `to` (a node id or a table as `db:<table>`), `kind` (`triggers`: emits the event or queues the job; `writes`: a listener of the event or job deletes rows of the table; `may_affect`: a table whose rows a cascade from that delete removes), `stamp` and, where known, `sources`. Both are derived statically and stamped at most `inferred`; a missing edge is not evidence that no event or job is set off.

### 3.8 Cards without source code

A card generated from shipped files only (executables, packages, database files) carries `x-sourceless: true` and no `operations`: no entry point with known inputs exists for an agent to call. `x-app_node` names the application node in the generator's evidence graph. `x-preview` states that a request preview can never pass for such a system (`passes: false`, `coverage: "open"`, `reason`). `x-capabilities` (`id`, `implemented_by`, `kind`), `x-screens` and `x-webmcp` (per origin: `origin`, `status`, `tools`, `untrusted: true`; tool names and schema skeletons a page declared, never their descriptions) list what was found; every item carries `stamp` (at most `observed`) and `requires_approval: true`, and capabilities carry `coverage: "open"`. A WebMCP tool lists `id` (`cap:webmcp/<origin>#<name>`), `name`, `kind`, `inputs` (the declared input schema's skeleton: `ad` field name, `tur` type, `zorunlu` required), `input_schema_sha256`, `read_only` and `consequential` as the page declared them. Nothing in such a card enables a write without a human.

### 3.9 What the application is for (`app.x-meaning`)

`x-meaning` describes the application in business terms: `domain` (`id`, `label`, `stamp`, `evidence`: the files, lines and terms it was counted from; or only `stamp: "unknown"` and a `reason`), `kind` (for example a project template or an example app), `purpose` and `problem` (a `quote` of the application's own text with its `source`, or a `text` derived from the domain, with `basis`), `actors` (user types with their operation count), `auth_flows` (sign-up, login, password reset, external identity, verification, multi-factor) and `capabilities` (`id` `cap:<resource>.<action>`, `name`, `resource`, `action`, `endpoints`). Every stamp is at most `inferred`; `stamp_ceiling` says so. A quote is what the application says about itself and is not evidence of behaviour. When the generator cannot tell the domain apart from another candidate it MUST write `unknown` rather than a guess. `app.description` MAY carry the quoted sentence as a `stampedText` with stamp `inferred`. None of these members enables a write.

## 4. Identity: narrowing, never granting

A card's `identity` member declares how writes are attributed. The governing rule:

> **Effective permission = the user's permission in the application ∩ the token's scope.**

- `write_requires_human` MUST be `true`: no write is performed without a real user identity.
- `service_account_writes`, if present, MUST be `false`.
- Writes MUST go through the application's own public entry points, so the application's own validation and authorization run. The final authorization decision always belongs to the application.
- A role an operation requires (`roles`) counts as held only when it is verified for the human: a claim in a signed identity token whose signature was checked, an explicit declaration by the human, or the application's own "current user" endpoint. An agent's own statement about roles is never verification.
- `model` is one of `oidc-token-exchange` (short-lived user tokens via OAuth 2.0 Token Exchange, RFC 8693), `user-session`, or `read-only`.

## 5. Data ledger

The `data_ledger` lists everything the generator or its runtime stores, in three classes:

| Class | Typical contents | Expectation |
|---|---|---|
| `code_derived` | map, cards, stamps | no personal data; retention while the code exists (`null`) |
| `operational_log` | human, agent, operation, time, result code | short retention; erasable |
| `customer_data` | parameter values, response bodies, cached results | SHOULD NOT be stored by default |

An organization can therefore read what a tool will keep **before** installing it.

## 6. Coverage

The `coverage` member reports how much is known. A clean coverage report means "no recorded gap", **not** proof of completeness. Agents SHOULD surface coverage to humans when asked about reliability.

`x-unread_route_sources` (card) lists places where the code registers routes the generator could not read (`count`, `sources`: `file:line reason`, `coverage: "partial"`). When it is present, `operations` is incomplete: an operation that is not listed may still exist, and agents SHOULD say so.

### 6.1 Data touched

An operation MAY carry a `data` member listing the tables or collections it may read (`reads`) and may write (`writes`), **as far as the generator could follow the code**, including conditional paths.

- It is neither a complete list nor a promise that every call touches every table. Storage reached through code the generator cannot see (compiled libraries, other services, nested writes inside a single ORM call, database triggers and cascades) is not listed.
- A generator that sees storage access it cannot resolve (for example raw SQL) MUST set `partial: true`.
- The stamp SHOULD be `inferred` unless every call target was resolved by a type checker.
- Agents MUST NOT treat an empty list as proof that an operation touches no data.

## 7. Changes

The `changes` member lists operations added, removed and **breaking** changes since `previous_source_ref` (for example, a newly required input). Generators SHOULD produce it automatically so integrations can be checked before a release.

- Changes are computed by comparing the operations of the previous card with the new ones, not by comparing commits: a generator produces them for uncommitted changes and for projects without version control too (`previous_source_ref` is then `unknown`). When the operations did not change, the previous card's `changes` are kept.
- Operations are matched by their entry (method and path). An operation that keeps its `id` but moves to another path or method is **not** an addition plus a removal: it is a breaking change (`path changed from /a to /b`), because callers that know the old address stop working.
- `x-revision` is `sha256:` followed by the SHA-256 of the RFC 8785 form of `operations`. It identifies the set of propositions the card makes, independent of commits.

## 8. Signature and freshness

- A published card MUST be signed. The signature covers the canonical JSON (RFC 8785: keys sorted by UTF-16 code units, ECMAScript number formatting) of the card without the `signature` member.
- `alg` is `ML-DSA-65` (FIPS 204, quantum-resistant, RECOMMENDED) or `Ed25519`. A development card that is not signed uses `alg: "none"`; agents MUST NOT perform writes based on it.
- For ML-DSA the FIPS 204 context string is the ASCII string `benlik-card` (domain separation: a signature made with the same key for any other purpose is not a valid card signature). `value` is the base64 encoding of the FIPS 204 signature. Public keys are published as `{"keys": [{"key_id", "alg", "public_key"}]}` with `public_key` in base64; a verifier MUST check that the key's `alg` matches the signature's.
- Interoperability: cards signed by the reference generator (Rust, RustCrypto `ml-dsa`) verify with an independent implementation (JavaScript: `canonicalize` for RFC 8785 and `@noble/post-quantum` for ML-DSA).
- The public key is published at `public_key_url`, on the application's own origin.
- An agent MUST ignore a card whose signature does not verify.
- A key that arrives with the card proves integrity, not identity: anyone who can replace the card can replace the key document too. An agent MUST NOT write unless the signing key is one it trusts for the application (pinned by the human, or published on the application's own origin and checked there); a signature that verifies with an untrusted key is reported as such and enables no write.
- An agent MUST treat a card as **stale** when `app.source_ref` does not match the running build; it MUST NOT perform writes based on a stale card.
- `x-issued_at` and `x-expires_at` (RFC 3339) bound the card's validity; both are signed. After `x-expires_at` an agent MUST NOT perform writes based on the card; it is regenerated. The reference generator uses 30 days by default (configurable from 1 to 365 days), always writes `x-expires_at`, and its runtime does not write based on a card that has no `x-expires_at`, has one it cannot parse, or cannot be read, so removing the member does not extend a card's life.
- Staleness of an operation is **computed**, not declared: `x-basis` maps every source file the operations rely on (the `sources` of the operation, its name, its inputs and its rules; paths relative to the application root) to its SHA-256. An operation is stale when one of its files is missing or has another digest. The 0.1 member `stale` is kept for compatibility: a generator SHOULD NOT write it, and a consumer MUST treat `stale: true` as stale.

### 8.1 Evidence

Facts in a card can be backed by evidence records kept **outside** the card, in an evidence bundle (`x-evidence_bundle`, e.g. `benlik-evidence.json` next to the card):

- The bundle is `{"benlik_evidence": "0.1.1", "app_id", "card_revision", "records": [...]}`. A record carries at least `id`, `kind` (e.g. `SOURCE_CODE`, `RUNTIME_TRACE`, `USER_CONFIRMED`), `claim_kind`, `confidence` (§3.3), `authority` and `shareable`; it MAY carry a subject, a locator, the provider and contradicting records. `card_revision` equals the card's `x-revision`.
- The card carries `x-evidence`: one summary per record with `id`, `kind`, `claim_kind`, `confidence`, `authority`, `shareable` and `sha256`, the SHA-256 of the RFC 8785 form of the full record. The summary is a value-free privacy projection: paths, lines and contents stay in the bundle.
- `x-evidence_root` is the RFC 6962 Merkle tree hash over the `x-evidence` digests in order (leaf `SHA-256(0x00 ‖ digest)`, node `SHA-256(0x01 ‖ left ‖ right)`, split at the largest power of two below n; an empty list hashes to `SHA-256("")`). Because the card is signed, the signature binds every record.
- The bundle MAY be signed with the same key and algorithm as the card, over the RFC 8785 form of the bundle without `signature`, with the FIPS 204 context string `benlik-evidence` (a bundle signature is never a card signature and vice versa).
- A bundle MAY be presented partially. A record whose digest does not match, a record that is not in `x-evidence`, a record that appears twice, a root that does not match, or a bundle whose `card_revision` is not the card's `x-revision` (a bundle of another card version) makes the card invalid together with that bundle; a record that is not presented is reported as not presented, not as an error. A bundle signature that does not verify in the `benlik-evidence` context makes the card invalid together with that bundle.

## 9. What a card never contains

Tokens, credentials, personal data, customer data, raw configuration tables or configuration secrets, and source code.

A rule MAY state a limit that the generator read from a setting the owner selected for publication (for example "A member may hold at most 5 books."). Such a rule is stamped `data` and its `sources` MUST name the setting with kind `config_table`. The card carries the rule text, never the table itself or any value the owner did not select.

## 10. Schema

The normative schema is [`schema/benlik-card.schema.json`](schema/benlik-card.schema.json) (JSON Schema 2020-12). An example is in [`examples/benlik.json`](examples/benlik.json).

## 11. Governance

- The specification lives in its own repository with its own semantic version, independent of any generator.
- Changes follow an RFC process: proposal → two weeks of public comment → editor decision, recorded publicly.
- A new **required** member enters the specification only after two independent implementations support it.
- Version 1.0 requires at least two independent generators and at least one agent that consumes cards.
- Transfer to a neutral foundation will be considered after 1.0.

## 12. Implementations

- **Sezgi**: reference generator (0.1: NestJS, Express, Fastify, Next.js, FastAPI back ends; React, Next.js, Preact, Vue and Svelte front ends for journeys; applications without source code; writes Draft 0.1.1 cards with an evidence bundle).
- Validators: `benlik` on crates.io, npm and PyPI (0.2.0) give the same decision on the shared [test vectors](test-vectors/), including signed cards produced by the reference generator. Given the publisher's key document they verify the card and evidence bundle signatures (ML-DSA-65; the PyPI package with its optional `imza` extra) and report `unsigned`, `not_verified`, `invalid`, `untrusted_key` or `verified`.
- Hand-written cards are valid; see the example. A hand-written card is not an independent generator in the sense of §11.
- Conformance checklist: [CONFORMANCE.md](CONFORMANCE.md).

## Appendix: Turkish field names

The international format uses English member names. Turkish documentation refers to them as follows:

| Member | Türkçe |
|---|---|
| `app` | uygulama |
| `generator` | üretici |
| `operations` | işlemler |
| `rules` | kurallar |
| `data` (`reads`, `writes`) | dokunduğu veri (okur, yazar) |
| `stamp` | damga (`certain` Kesin, `data` Veri, `approved` Onaylı, `inferred` Yorum, `unknown` Bilinmiyor; x- üyelerinde `observed` Gözlem) |
| `x-evidence` | kanıt özetleri |
| `x-basis` | dayanak özetleri |
| `x-dna` | davranış özeti |
| `x-response` | yanıt şeması |
| `x-journeys` | kullanıcı yolculukları |
| `x-nodes`, `x-relationships` | olay ve iş düğümleri, ilişkiler |
| `x-sourceless` | kaynak kodu olmadan üretilmiş kart |
| `coverage` | kapsam |
| `identity` | kimlik |
| `data_ledger` | veri defteri |
| `changes` | değişiklik |
| `signature` | imza |
