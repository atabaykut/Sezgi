# Public benchmark

Everything here can be reproduced by anyone. The measured applications are open source and pinned to exact commits. Ground truth is either **the application's own official OpenAPI file** (written by its maintainers, not by us) or **an independent referee** built on the TypeScript compiler.

The private application used in the main README is **not** included; only its aggregate numbers are published.

## Run

Requirements: Rust 1.99.0 (pinned by `rust-toolchain.toml`; rustup installs it), Node.js 22+ (enforced by `engines` and `.npmrc`; `canonicalize` needs 22, `@noble/post-quantum` 20.19), git, bash (Git Bash on Windows).

The card schema check needs the Benlik Card schema. Clone the specification next to Sezgi, or point `BENLIK_SEMA` at the schema file:

```sh
git clone https://github.com/atabaykut/Sezgi
git clone https://github.com/benlik-spec/benlik-spec        # next to sezgi/: olc.sh finds ../benlik-spec/schema/
cd sezgi/kanit/acik
# or: export BENLIK_SEMA=/path/to/benlik-card.schema.json
```

Without the schema, `olc.sh` still runs and writes "(şema bulunamadı …)" in place of the card check.

```sh
./getir.sh                    # sparse clone of Immich and Cal.com at pinned commits (read-only)
npm ci --ignore-scripts       # referee tools (TypeScript compiler, ajv), installed only in this folder
./olc.sh                      # build Sezgi, map both apps, compare; summary in sonuc/OZET.md
```

The measured repositories are **only read**: no dependency is installed in them, no script of theirs is executed, nothing is built.

## What is measured

| Measure | Ground truth | Tool |
|---|---|---|
| Endpoints found, auth, body fields, required flags | Official OpenAPI file of the app | `../karsilastir-openapi.js` |
| Database table links (reads/writes per endpoint) | Referee that follows calls with the **TypeScript type checker** (Sezgi follows field types and names) | `../hakem/veri-hakem.js`, `../karsilastir-veri.js` |
| Benlik Card validity | `benlik-card.schema.json` (JSON Schema 2020-12) | `../kart-dogrula.js` |

Expected results (Sezgi 0.1.0):

| App (commit) | Endpoints | Auth | Body fields | Required | Data links |
|---|---|---|---|---|---|
| Immich server (`aec0dd1`) | 279/279 (+16 not in the doc) | 100% | 437/485 | 100% | n/a (Kysely, not Prisma) |
| Cal.com API v2 (`54343aa`) | 121/121 (+41 not in the doc) | 99.2% | 250/250 | 100% | 341/341 links, 0 extra |

These are recall numbers against the official document; endpoints the document does not list are counted separately, not scored. The NestJS adapter was improved on these two apps (first, untouched measurements: Immich 84.9% endpoints and 0% fields, Cal.com 96.7% and 52.1%); a held-out app (Ghostfolio) was measured once afterwards with the frozen adapter: see `../SONUC-5-holdout-ghostfolio.md` and the pre-registration `../HOLDOUT-ON-KAYIT.md`.

The run also signs a card with a throwaway key and verifies it with independent JavaScript libraries (RFC 8785 `canonicalize`, `@noble/post-quantum`).

Known gaps, reported rather than hidden:
- Immich: 49 body fields belong to 4 schemas built at runtime (for example `AdminConfigDto`); Sezgi reports them as unresolved instead of guessing.
- Cal.com: one auth mismatch (`GET /v2/auth/oauth2/clients/{}`). Investigated: the code protects it with `@UseGuards(ApiAuthGuard)` (`oauth2.controller.ts:47`) but the official document omits the auth header for this operation. Sezgi follows the code; the 99.2% is therefore a documentation gap, kept as measured.
- Data links: calls into compiled libraries (`node_modules`) and other services are not followed, by Sezgi or by the referee.

## Agent benchmark (optional, costs API credits)

`../olc.js` runs the same tasks through an AI agent in different setups (A: code only, B: official OpenAPI behind the same tool interface, C: Sezgi only) and scores answers against a key derived automatically from the official OpenAPI file:

```sh
node ../olc.js A --uygulama depolar/immich/server --gorevler ../immich/gorevler.json --etiket immich
node ../olc.js C --uygulama depolar/immich/server --gorevler ../immich/gorevler.json --harita sonuc/immich --etiket immich
```

Published result (16 tasks, one model): code reading 14/16 at 39.5k tokens per task, official OpenAPI 15/16 at 43.6k, Sezgi 16/16 at 20.5k. Method and caveats: [`../SONUC-3-immich-ajan.md`](../SONUC-3-immich-ajan.md).

## Third-party applications

The measured applications are **not** included in this repository. They are fetched at pinned commits into git-ignored folders, only read, never built or run. Derived maps and cards (`sonuc/*/`) are not committed; only the summary `sonuc/OZET.md` is.

| Application | Repository | Commit | License (repository root) |
|---|---|---|---|
| Immich server | https://github.com/immich-app/immich | `aec0dd1e6ba4` | AGPL-3.0 |
| Cal.com API v2 | https://github.com/calcom/cal.com | `54343aa685ae` | MIT (with separately licensed `ee` parts) |
| Ghostfolio API (held-out) | https://github.com/ghostfolio/ghostfolio | `c169311fa307` | AGPL-3.0 |

`../immich/gorevler.json` (agent tasks) was generated from Immich's official OpenAPI file and contains endpoint paths and field names from it.
