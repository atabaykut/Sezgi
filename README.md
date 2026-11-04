# Sezgi

*Sezgi is Turkish for intuition: knowing without thinking.*

**A sense of self for software.** Sezgi reads your application's code once, turns it into one API and explains it to AI with evidence, so your assistant understands your application and works in it safely.

**Who it is for:** developers · AI assistants and agents · enterprises · SaaS companies · agencies and integrators · security and audit teams. Overview page: **[atabaykut.github.io/Sezgi](https://atabaykut.github.io/Sezgi/)**

When an assistant such as Claude, Cursor or Copilot needs to do something in your application, it scans the code from scratch every time and guesses: "Which endpoint creates a user, which fields are required, who is allowed to do it?" Sezgi reads the code **once** and builds a map of the application. The assistant stops scanning code and asks Sezgi instead; the answer is short and comes with its source.

> **Status: 0.1, experimental.** Sezgi only reads by default and never writes to your application's code. Letting the assistant actually use the application (live mode) is opt-in and starts on a test copy. Writing to real data is experimental: it needs a key that an administrator sets up once and a password only the human knows, so an agent with shell access under the same account cannot open real data or approve a request without the password. It does not stop a program that learns the password or runs as administrator ([SECURITY.md](SECURITY.md)).

## What it does

| | Without Sezgi | With Sezgi |
|---|---|---|
| **Finding an operation** | The assistant opens routes, controllers and services one by one for every question, and starts over in the next session | It asks Sezgi once and gets the operation, its address and its source file in a short answer |
| **Inputs and rules** | "Is `quantity` required? What is the limit?" is answered by guessing, and the call fails with a 400 | The rule is read from the code (for example a zod schema: `quantity` integer, 1 to 10) and shown with file and line |
| **Who may do it** | It does not see that one endpoint needs login, another needs `admin`, a third lets users touch only their own records | Login, roles (`requireRole('admin')`) and ownership checks (`order.user_id !== req.user.id`) are on the map |
| **What it touches** | "Delete user" looks like one row; it does not know the user's orders go with it | The tables each operation reads and writes, including cascades found in the schema (`users` → `orders`, `ON DELETE CASCADE`) |
| **Outside effects** | It calls "refund" without knowing it charges a real payment provider or sends an e-mail | Outgoing HTTP calls, e-mail, payment, queues and cloud services are listed per operation |
| **Can it be undone?** | Nobody says | Yes, partly, no or unknown, with the reason; irreversible operations always wait for a human |
| **When it does not know** | It fills the gap and sounds sure | Every fact is stamped **certain**, **observed**, **inferred** or **unknown**; gaps are reported, never filled |
| **Acting in the app** | Either it cannot, or it gets a key that opens everything | Opt-in live mode: a test copy first, a token with only the operations you allow, approval for risky steps, an encrypted log |
| **When the code changes** | Hand-written docs and integrations go stale; a removed field breaks clients in production | `sezgi watch` re-reads only what changed; `sezgi diff` and the [GitHub Action](https://github.com/marketplace/actions/sezgi-ai-contract-check) fail a pull request that removes an operation, adds a required input or a new login rule |
| **Many applications** | A separate API, docs and MCP server for each one | `sezgi switchboard` serves them all over one connection |
| **What the owner sees** | No idea what the AI can reach | `sezgi mirror` shows in the browser what is known, what is not, and what is open |
| **Cost** | Tens of thousands of tokens per task spent reading code | On Immich, the same 16 tasks: code reading 14/16 at 39.5k tokens per task, **Sezgi 16/16 at 20.5k** ([benchmark](#benchmark)) |

### An example

Take the small [example shop](ornek/siparis/README.md) in this repository and ask: *"If I delete a user, what happens, and can it be undone?"*

**Without Sezgi**, the assistant reads `server.js`, the route files, `auth.js`, `db.js` and `schema.sql`, and may still miss the cascade or the admin check.

**With Sezgi**, it asks once and gets an answer like this (shortened):

> `DELETE /api/users/:id` · role `admin` (`requireRole('admin')`) · deletes from `users`; the user's `orders` are deleted too (`ON DELETE CASCADE` in `schema.sql`) · hard delete, cannot be undone · in live mode it always waits for a human's approval.

Each part points to the file it was read from. In the same app Sezgi also finds, from code alone, that the refund calls `api.payments.example.com` and cannot be undone, and that the receipt goes out by e-mail through nodemailer.

### What you can ask

Once connected, you can ask your assistant things like:

- "How do I start a refund in this app, what does it need, and who can do it?"
- "If I delete a user, which tables are affected, and can it be undone?"
- "Is there any operation in the billing module that does not require login?"
- "Which operations reach outside this app: payment, e-mail, other services?"
- "Which operations need a human's approval, and why?"
- "Which endpoints let a user change someone else's record?"
- "What would break for API clients if this branch is merged?"
- "Which screen and button starts this operation?"
- "What does Sezgi not know about this app yet?"

## Why it exists and who it is for

To connect an application to AI today, an API is written, documented and wrapped in an MCP server, separately for every application. Most companies never did it. Sezgi is **a layer** that does this without writing code: it is applied on top of the application, turns it into one API, and leaves no trace when removed. The goal is to give software a sense of self: an AI that reads any software once, truly understands it, and works in it safely.

- **Developers:** instead of walking undocumented code for weeks, the map, OpenAPI and Benlik Card come out of the code by themselves.
- **AI assistants and agents:** ask instead of re-scanning the code; a wrong write is stopped at the door, and token waste goes down.
- **Enterprises:** undocumented legacy systems open to AI without a months-long integration project; writes go with the real user's identity, and are logged.
- **SaaS companies:** a ready MCP server and OpenAPI for customers; changes that would break them are caught before release.
- **Agencies and integrators:** a new client's system is mapped on day one.
- **Security and audit:** writes that need no login and endpoints with no access check found become visible without granting anyone access.

This goal is measured against **17 founding questions** (what the app is, why it exists, who may do what, which tables and other systems an operation touches, whether it can be undone, which screen starts it…). Today's Sezgi answers 2 reliably and 14 partly on never-seen apps. The 20 parts of the self, the principles, going beyond the application, the roadmap and an honest status for each: [docs/VISION.md](docs/VISION.md).

## Get started in 3 steps

Requires Node.js 20 or later.

```sh
npm install -g sezgi      # or without installing: npx sezgi <command>
cd path/to/your/app
sezgi init                # reads the code, writes the map to .sezgi/ (a few seconds on a large app)
sezgi connect             # adds Sezgi to your AI tool; asks tool by tool, `sezgi remove` undoes it
```

That's it. Restart your assistant and ask about your application.

Optional:

```sh
sezgi mirror              # in the browser: what Sezgi knows about this app, and what it does not
sezgi watch               # keeps the map up to date as the code changes
sezgi doctor              # if something is wrong, says what is missing and the command that fixes it
```

Every command also has a Turkish name, for example `sezgi bagla` (`connect`) and `sezgi izle` (`watch`).

npm picks the prebuilt program for your computer: Windows x64, macOS (Apple Silicon and Intel, macOS 11+), Linux x64 and arm64 (glibc 2.34+: Ubuntu 22.04+, Debian 12+, RHEL 9+). On Alpine / musl and other platforms, build from source (`cargo build --release`, Rust 1.99+).

## What is in the map

For every operation (endpoint), Sezgi extracts:

- **What it needs:** inputs, their types, required fields and validation rules.
- **Who can do it:** whether login is required, roles, and checks that only the record's owner passes.
- **What it touches:** the database tables it reads and writes, cascading deletes, and the events and jobs it triggers.
- **Its outside effects:** e-mail, payment, queues, cloud services or requests to another system.
- **Whether it can be undone:** yes, partly, no or unknown, with the reason.
- **Where it lives in the UI:** which button on which screen calls it.
- **What the application is for:** business domain, user types and business capabilities, quoted from the application's own README.

Every fact carries a confidence stamp: **certain** (read directly from the code), **observed** (seen at run time), **inferred** or **unknown**. Sezgi does not fill gaps; it reports them. The map stays on your computer, in the application's `.sezgi/` folder; building it uses no network and no language model.

Sezgi also writes the application's **[Benlik Card](https://github.com/atabaykut/Sezgi/blob/main/benlik/SPEC.md)**: an open, signable description file that other tools can read too (`sezgi init --sign`).

## Supported languages

| Language and framework | Quality level |
|---|---|
| TypeScript + NestJS | 2 |
| JavaScript / TypeScript + Express, Fastify, Next.js | 2 |
| Python + FastAPI | 2 |
| Python + Django (including Django REST Framework), Flask | 1 |
| Java / Kotlin + Spring Boot, Kotlin + Ktor | 1 |
| Ruby on Rails | 1 |
| C# + ASP.NET Core | 1 |
| Go + net/http, Gin, Echo, Chi, Fiber, gorilla/mux | 1 |
| PHP + Laravel | 1 |

**Level 1:** measured during development against an independent referee or a hand count. **Level 2:** also measured once on an application whose code had not been looked at, with the measurement plan registered in advance. React, Vue, Angular, Svelte, Remix and Solid front ends are read too. For applications without source code (`.exe`, `.dll`, APK, SQLite, Electron) there is `sezgi discover`.

Sezgi 0.1 maps REST/HTTP operations; GraphQL, gRPC, tRPC and WebSocket are not mapped yet. If your application uses them, or Sezgi finds far fewer operations than the route registrations in the code suggest (its own router, routes registered by reflection), `sezgi init`, `sezgi report` and the overview the AI sees say so plainly: an incomplete map is never presented as complete.

## Benchmark

A public benchmark you can reproduce with one command ([`kanit/acik`](https://github.com/atabaykut/Sezgi/tree/main/kanit/acik)). Ground truth is each application's own official API document, published by its developers.

| Application (commit) | Endpoints found | Not in the doc | Auth decision | Body fields |
|---|---|---|---|---|
| Immich server (`aec0dd1`, 61,567 lines) | 279/279 | 16 | 100% | 437/485 |
| Cal.com API v2 (`54343aa`, monorepo) | 121/121 | 41 | 99.2% | 250/250 |

On Immich, with the same model and the same 16 tasks (one run): an agent reading code 14/16 at 39.5k tokens per task, the official OpenAPI 15/16 at 43.6k, **Sezgi 16/16 at 20.5k**.

The adapters were developed on these two applications. Results on applications never seen before, including the mistakes and caveats, are in the [detailed reference](docs/REFERENCE.md#why) and the [`kanit/`](https://github.com/atabaykut/Sezgi/tree/main/kanit) folder.

## Letting the assistant act (optional)

By default Sezgi only serves the map; the assistant cannot call your application. If you turn on live mode, the assistant can use the application **with your session** and **only with the permissions you grant**:

```sh
sezgi live setup          # target address, test-copy confirmation, mode, session and first permission token; asks step by step
```

In this mode:

- **Test copy first:** writes go only to the address you declared as a test copy.
- **Permission token:** the assistant sees and calls only the operations you opened. Off by default, it expires and can be revoked.
- **Risky actions are asked:** deletes, operations with outside effects (e-mail, payment, queue, cloud), irreversible operations, credential changes whose reversibility is not proven (password, token, key, two-factor authentication) and anything Sezgi is not sure about wait for your approval with `sezgi approve`.
- **Requests are checked first:** a request that breaks a known rule never reaches the application.
- **Everything is logged:** an encrypted, hash-chained log (`sezgi log`).
- **Real data (experimental):** additionally needs a key set up by an administrator (`sezgi human setup --system`); on real data every write is approved in a terminal with your password.

Sezgi takes no backup and cannot undo a write: back up your application before an operation that cannot be undone. All gates and protection limits: [detailed reference, Live mode](docs/REFERENCE.md#live-mode) and [SECURITY.md](SECURITY.md).

> **Security note.** Text that Sezgi extracts from code (summaries, field names, rule texts) is passed to the assistant as data. A malicious repository can put instructions there. Sezgi strips control characters, limits length and tells the assistant not to follow such text; still, treat any code-derived text as untrusted.

## Commands

For everyday use, `init`, `connect`, `mirror` and `watch` are enough. Turkish names work too; `sezgi help <command>` gives the details of each command.

| Group | Command | What it does |
|---|---|---|
| Start | `init` | Reads the code, writes the map and the Benlik Card to `.sezgi/` |
| | `discover` (`kesfet`) | Inspects an application without source code |
| | `watch` (`izle`) | Keeps the map up to date as the code changes |
| | `doctor` (`doktor`) | If something is wrong, says what is missing and how to fix it |
| Connect to AI | `connect` (`bagla`) | Adds Sezgi to your AI tool |
| | `remove` (`kaldir`) | Undoes everything Sezgi added |
| | `mcp` | The MCP server (your AI tool starts it) |
| | `serve` (`sun`) | Local HTTP + OpenAPI for tools that do not speak MCP |
| | `switchboard` (`santral`) | Serves several applications over one connection |
| Look | `mirror` (`ayna`) | Shows in the browser what is known, the operations and permissions |
| | `report` (`rapor`) | Lists what is known and what is not |
| | `diff` (`karsilastir`) | Finds breaking changes between two versions |
| | `measure` (`olc`) | Measures speed and accuracy on your own application |
| | `observe` (`gozlem`) | Takes a browser network capture as evidence (shapes only, never values) |
| Live mode | `live` (`canli`) | Shows the status; `live setup` sets it up step by step |
| | `mode` (`mod`) | Observe (default), read or write |
| | `target` (`hedef`) | Where requests go |
| | `session` (`oturum`) | Your own session in the application, and your roles |
| | `token` | Which operations the assistant may perform |
| | `approve` (`onay`) | Requests waiting for approval |
| | `plan` | A change plan across several applications (approved step by step) |
| | `log` (`kayit`) | The encrypted, hash-chained log |
| Security and data | `key` (`anahtar`) | Benlik Card signing keys |
| | `human` (`insan`) | The administrator-installed key and the human password for real data |
| | `verify` (`dogrula`) | Verifies a card's signature and validity |
| | `ledger` (`defter`) | What Sezgi keeps and for how long |
| | `forget` (`unut`) | Erases a person's data or all customer data (GDPR) |
| Advanced | `memory` (`hafiza`) | Avoids sending the same read to the application twice (off by default) |
| | `share` (`paylas`) | A signed map your team can commit |
| | `rea` | The optional REA tool for analysis without source code |
| | `diagnostics` (`teshis`) | Local-only diagnostics that keep counters only |

## More

- [Example application](ornek/siparis/README.md): a small order app you can try in 5 minutes, with what Sezgi should find in it.
- [FAQ](docs/FAQ.md): the hard questions, answered with links to the evidence; [architecture](docs/ARCHITECTURE.md); [removing Sezgi completely](docs/UNINSTALL.md).
- [Detailed reference](docs/REFERENCE.md): how each language is read, the Benlik Card, every live-mode gate, run-time observation, the switchboard, known limitations and the roadmap.
- [SECURITY.md](SECURITY.md): threat model and how to report a vulnerability.
- [`kanit/`](https://github.com/atabaykut/Sezgi/tree/main/kanit): every measurement, its method and raw results (English summary in its README).
- [Glossary](docs/GLOSSARY.md) and [changelog](CHANGELOG.md).
- [Türkçe README](README.tr.md).

## License

Apache-2.0. Copyright © 2026 Sezgi contributors.
