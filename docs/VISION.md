# Where Sezgi is going

[Türkçe](VIZYON.md)

> **Sezgi is not an application, it is a layer.** One can of paint covers a whole building, and a building holds thousands of cans.

**Goal:** give software a sense of self. Sezgi wraps the application it is installed on, whatever language it is written in and whatever it does, end to end, and turns it into a single API. It knows what the application is, what it can do and which rules it follows without having to think. It gives its owner a token whose permissions the owner chooses. Any AI, MCP tool or program holding that token can read and write the application like a database, but only as far as allowed and always through the application's own rules.

This page puts where we are going and where we are today side by side, honestly.

## The problem

To connect an application to AI today, three jobs are done: an API is written, it is documented, and an MCP server is built. All three are repeated for every application. In most companies they were never done.

Picture an application of a million lines spread over a thousand folders, with no API documentation:

- **A developer** who wants the documentation walks every folder, finds the routes, reads what each one takes and returns, and writes it down one by one. It takes weeks, and goes stale as the code changes.
- **An AI** is a stranger to the application. It opens files one at a time, reads, guesses, and starts over for every question. Hours and millions of LLM tokens go by, and the answers and actions are often incomplete or wrong.

People do not work like this. Asked their name, they say it; they do not wonder who they are. A finger finds a key on its own. They learned it once and it became part of them. **Sezgi gives software that sense of self.**

## Who it is for

| Who | Their problem today | With Sezgi |
|---|---|---|
| **Developers** | Spend weeks walking undocumented code route by route; everything goes stale when the code changes. | The map, the OpenAPI document and the Benlik Card come out of the code by themselves; when code changes, only the difference is re-read. |
| **AI assistants and agents** | Re-scan the code every session, guess, and make wrong calls through poorly described tools. | They ask Sezgi instead of scanning, and get short answers with sources. A wrong write is stopped at the door, in the preview. |
| **Enterprises** (banking, insurance, healthcare, travel, manufacturing) | Undocumented legacy systems; opening them to AI is a months-long project; the auditor asks "what did the agent touch?" | The map is there on day one. Writes go with the real user's identity, through the application's own checks. Every action is logged as "which human, which agent, what". |
| **SaaS companies** | Customers ask "can I connect my AI to your product?"; every release breaks customer integrations. | An up-to-date OpenAPI, a ready MCP server and a Benlik Card appear by themselves. `karsilastir` catches changes that would break customers before release. No separate integration team. |
| **AI and agent companies** | Their agents re-read customer code every time; they pay for the wrong actions. | The agent reads the card, not the code. Accuracy goes up; LLM tokens and GPU waste go down. |
| **Agencies and integrators** | Getting to know each new client's system takes weeks. | The map is there on day one. The coverage report goes into the proposal; the gaps found go to the client. |
| **Application owners** | Cannot see what they are opening to AI. | The Ayna screen shows what is known and what is not. They choose the permissions; the last word is theirs. |
| **Security and audit teams** | Undocumented doors and endpoints without access checks go unnoticed. | In ten minutes, without granting anyone write access, they see writes that need no login, endpoints with no access check found, and links this release will break. |

## The parts of the self

This table is the project's contract; every design decision serves it. "Today" is the state in this release.

| # | Part | Meaning | Today |
|---|---|---|---|
| 1 | **Knows itself** | Knows what the application is, what it does and what it can be asked to do. | Partly: the `kimsin` introduction and domain classification; sometimes wrong |
| 2 | **Reaches directly** | Nothing is scanned to reach an operation; answer time does not depend on code size. | Yes: the assistant queries the map instead of scanning code |
| 3 | **Covers everything** | Even five million lines are one brain: no door, screen or setting is left out. | Partly: operations in thirteen frameworks; unreadable routes are reported |
| 4 | **Learns repetition once** | Something repeated in 50 places is one card. | Partly: composed decorators and shared middleware are resolved from one definition |
| 5 | **Does not think, knows** | No language model inside. It does not make things up; it knows or says "unknown". | Yes: every claim has a source and a trust stamp |
| 6 | **Knows the rules** | Knows each door's rules and applies them at the door. | Partly: validation rules are applied in the preview; some rules inside services |
| 7 | **Is one API** | The whole application through one cable. | Yes: MCP, HTTP and OpenAPI |
| 8 | **Use the app like a database** | Reading and writing as simple as a query. | Partly: live mode on a test copy; experimental on real data, with a human password |
| 9 | **Masters the interface** | Knows screens, forms and buttons; can do anything a person can do on screen. | Weak: which requests the front end sends, and browser captures; no acting on screens |
| 10 | **Issues tokens** | The owner creates access tokens with permissions of their choosing. | Yes |
| 11 | **Single counterpart, a bridge** | The AI talks only to Sezgi, not to the application. | Yes: the switchboard joins several apps in one connection |
| 12 | **Does not discriminate** | A game, a hospital system, factory software or an AI model are wrapped the same way. | Yes: no domain knowledge in the core |
| 13 | **Works in every language** | A language-independent core with pluggable readers. | Partly: TypeScript/JavaScript, Python, Java, Kotlin, Go, PHP, Ruby, C# |
| 14 | **Almost no memory** | Keeps the map, not the data: the library catalogue, not the books. | Partly: customer data is not kept by default; the map is not yet in a compressed binary format |
| 15 | **Tries to be unbreakable** | Standard post-quantum cryptography and defence in depth. | Partly: ML-DSA signatures, an encrypted chained log; no independent audit yet |
| 16 | **Removes waste** | GPU seconds, not hours: repeated work done once, wrong work stopped before it starts. | Partly: fewer tokens for the same question (measurements in `kanit/`) |
| 17 | **Is a layer** | Does not replace the application; leaves no trace when removed. | Yes: never writes into the application's code; `sezgi kaldir` |
| 18 | **Goes beyond the app, if asked** | Command-line tools, programs without source, the operating system. | Started: source-less discovery at identity and schema level |
| 19 | **Does no harm** | If Sezgi crashes or is removed, the application does not notice; permissions open step by step. | Yes: observation by default; when unsure, the answer is approval |
| 20 | **Publishes a Benlik Card** | The application introduces itself with a signed, open card. | Yes: signed card and open specification |

Also: it learns once and then only learns the difference; it needs no internet and runs entirely locally; it is open source.

## What Sezgi is not

- **Not an application.** It has no business logic and no data of its own; it is applied on top of the application.
- **Not a language model.** It does not think or generate text. The AI connected to it does the thinking.
- **Not a code search tool.** It also answers "where is this code", but its real job is to present the application as one API.
- **It does not change the application's logic or skip its rules.** Writes go through the application's own code, never straight into the database, because that path skips calculations, checks and notifications.
- **It does not spread.** It only knows the applications explicitly configured.

## Principles that do not change

1. **Do no harm.** If something goes wrong, Sezgi stops, rolls back and says what happened.
2. **A layer.** Leaves no trace when removed.
3. **The application's logic is not touched.** By default no code file changes.
4. **The application's rules are not bypassed.**
5. **Only authorised scope.** Only explicitly configured applications are read.
6. **Sezgi does not think.** It says "unknown" when it does not know.
7. **No discrimination.** The core carries no domain knowledge.
8. **Local and offline.** Code and data are never sent to an outside service; no telemetry.
9. **Closed by default.** Nothing is reachable until the owner opens it.
10. **No home-made cryptography.** Only audited, post-quantum standards.
11. **Open formats.** The Benlik Card, memory and permission formats are open standards.
12. **Sezgi grants nothing, it only narrows.** Effective permission = the user's permission in the application ∩ the token's permission. No human, no write.
13. **Customer data is not written by default.** Everything kept is declared in the data ledger.
14. **Evidence before claims.** Every accuracy and speed claim is backed by a measurement.

## The 17 founding questions

A connected assistant should answer these about an application it has never seen, within its first minute, correctly, with sources and honestly. The status column is the latest measurement on three held-out open-source apps (NestJS, Django, Next.js), 2026-11-03.

| # | Question | Today |
|---|---|---|
| 1 | What is the application? | Partly: from the README and domain entities; domain classification is sometimes wrong |
| 2 | Why does it exist? | Partly: quoted from the README |
| 3 | What business problem does it solve? | Partly: sometimes picks a setup sentence |
| 4 | Who are its users? | Partly: from role names |
| 5 | What can a user do? | Works: almost all operations are found; unreadable routes are reported |
| 6 | Which features actually work? | Partly: only with a network capture or live-mode observation |
| 7 | Which screen starts which operation? | Weak: known request patterns only |
| 8 | Which functions call each other? | Partly: call graph in eight languages |
| 9 | Which tables are affected? | Partly: ORMs and raw SQL, cascades included |
| 10 | What are the preconditions? | Partly: validation rules and rejections inside services |
| 11 | Who may do what? | Partly: the login decision is reliable in most frameworks; roles and custom auth decorators can be missed |
| 12 | Does it affect other systems? | Partly: HTTP, queues, e-mail, payment, cloud; "unknown" when the chain has an unclassified call |
| 13 | Can it be undone? | Partly: soft delete, inverse operations, revision history, permanent delete |
| 14 | Does it need human approval? | Partly: the decision is fail-closed; the stated reason is not always right |
| 15 | Is this certain, inferred or unknown? | Partly: every claim is stamped; wrong claims stamped "certain" can still occur |
| 16 | What becomes stale when code changes? | Works: staleness tied to content digests |
| 17 | Which operations does a task need? | Partly: Turkish and English task plans; unrecognised steps are left empty |

In the same measurement about half of the hand-checked claims were correct. The target is **above 90% accuracy on never-seen apps and zero wrong claims stamped "certain"**.

## Three doors into an application

1. **Existing APIs:** routes, inputs, validation, access, data effects. This is where Sezgi is strong today.
2. **Internal code:** calling functions that have no API, isolated and with least privilege. Not yet.
3. **The interface:** screens, forms, buttons and the request behind each button. Today only the front end's request patterns and browser captures.

## From a map to understanding

The goal is one meaning graph of the whole application:

- **Nodes:** screen, component, operation, function, table, business rule, user type, user journey, external system.
- **Edges:** calls, reads, writes, deletes, triggers, authorizes, undoes.
- **Behaviour summary:** for every operation who may do it, preconditions, failure modes, data effect, external effect and reversibility, all with evidence.

The graph and the evidence store exist today (`.sezgi/graf.json`, `.sezgi/kanit.json`). What is missing is depth in screen and business-rule nodes and the accuracy of every edge.

## Beyond the application

Sezgi's method (find, map, connect through one cable, enforce the rules) applies to more than applications:

- **Command-line tools** (git, docker, ffmpeg): every command, parameter and rule.
- **Programs without source** (.exe, .dll, APK, SQLite, desktop apps): through the operating system's accessibility interfaces and the program's open doors, anything a person can do with it.
- **The operating system:** files, processes, settings, services, within the user's own permission.

This capability is a separate package, off by default; every target is added by the user, one at a time, with the reason shown. Destructive actions always need approval. It does not open before core 1.0 and an independent security audit. Today, source-less discovery works at identity and schema level.

**Many applications:** the switchboard joins several apps in one connection. The goal is entity mapping across apps ("this customer is that record over there") and change plans approved step by step. The switchboard and approved plans exist today; automatic entity mapping is partial.

## Benlik Card

Every application introduces itself with a signed, standard card: what it can do, which rules it follows, on whose behalf writes happen, what Sezgi keeps, and **what is not certain**. It is the first API description that can say "I am not sure". OpenAPI describes an application's menu; the Benlik Card describes its capabilities, rules and limits.

The specification is open ([benlik/SPEC.md](https://github.com/atabaykut/Sezgi/blob/main/benlik/SPEC.md)). An open standard is also the best defence against large players adding the same thing to their products: whoever copies the standard makes it bigger. 1.0 goal: two independent card generators and an agent that consumes the card.

## How it differs from what exists

| Exists | What it does | What is missing |
|---|---|---|
| Framework OpenAPI generators | API docs for one framework | Need setup and annotations; carry no rules, side effects or approval |
| OpenAPI → MCP converters | Turn existing docs into MCP tools | Do nothing without docs |
| MCP gateways | Put many MCP servers behind one door | Do not know the application; only carry traffic |
| Browser automation agents | Click by looking at the screen | Slow and brittle; do not know the request behind the button |
| Code indexes and code maps | Answer "where is this code" | Do not perform actions; no token, permission or approval |
| Hand-written assistant notes (CLAUDE.md, AGENTS.md) | Give the assistant context | Written by hand, go stale; the assistant still walks the folder |

**What makes Sezgi different:** writing an API, documenting it and connecting it to MCP are three separate jobs done for every application today. Sezgi does all three at once, without writing code and by itself; it knows and enforces the rules; it updates itself when code changes; and it opens the application to any AI through a token whose permissions the owner chooses. All of it locally, offline and without a language model.

## Roadmap

| Stage | Content | Status |
|---|---|---|
| 0.1 | Code reading, map, Benlik Card, MCP, reads and preview, Ayna | Released |
| 0.2 | Writes on a test copy, identity delegation, data ledger, approval queue | Mostly there (live mode) |
| Languages and frameworks | A published quality level per language and framework | Thirteen frameworks |
| Interface (door 3) | Reading screens, button → request mapping | Started |
| Internal code (door 2) | Calling functions without an API, isolated | Not yet |
| Scale | Many applications, a five-million-line benchmark | Switchboard exists; no large benchmark yet |
| 1.0 | Independent security audit, writes on real data, two card generators | In progress |
| Beyond the application | Command line, operating system, programs without source (separate package) | After 1.0 |

## Next

1. **Accuracy:** the held-out measurement is repeated and published with every release; wrong "certain" claims go to zero.
2. **Coverage:** Supabase (edge functions and SQL functions), Spring servlet filters, Next.js `basePath`, server templates (Blade, ERB, Jinja).
3. **Assistant experience:** filtered listing ("writes that need no login"), callers/callees by short name, "what breaks if I change this file".
4. **Interface:** screens, and which operation each button starts.
5. **1.0:** independent security audit, two card generators, a consuming agent.
