# Sezgi

*Sezgi is Turkish for intuition: knowing without thinking.*

Sezgi reads an application's code once — TypeScript + NestJS, Python + FastAPI, Django (with Django REST Framework) and Flask, JavaScript / TypeScript + Express, Fastify, Next.js, Java / Kotlin + Spring Boot, Kotlin + Ktor, Ruby on Rails, C# + ASP.NET Core, Go + net/http, Gin, Echo, Chi, Fiber, gorilla/mux and PHP + Laravel — extracts its operations, inputs, validation rules, login and role requirements and database links, serves them to AI agents over MCP and writes a signable **Benlik Card**. It also builds a function call graph (table effects and access checks deep in the service layer), finds what each operation does outside the application (outgoing HTTP, queues, e-mail, payment) and whether it can be undone, summarizes what the application is for (business domain, users, business capabilities, all at most *inferred*) and turns a task in plain Turkish or English into the operations it needs. It summarizes how each operation behaves (rejections, cascading deletes, events and jobs), links front-end screens to operations as user journeys, keeps run-time observations as request shapes only (`sezgi observe`, never values) and maps applications shipped without source code (`sezgi discover`). Every fact is stamped *certain* (read from a definition), *observed* (seen at run time or in shipped files) or *inferred* and kept with its evidence. Output is in Turkish or English (`--lang en`, `SEZGI_LANG`, or the system language).

By default Sezgi only reads; an opt-in live mode lets an agent call the app's test copy through permission tokens and the human's own session. A write goes without human approval only when it is not destructive, it has no effect outside the application (outgoing HTTP write, e-mail, payment, queue, cloud) and is not marked as impossible to undo, its evidence is certain and nothing contradicts it, the preview checked every known rule, any required role is verified for the human (never the agent's claim), the token does not ask for approval of every write, the map is fresh and the card has not expired; everything else waits for the human's approval, and everything is written to an encrypted, hash-chained log. Writes on real data are experimental and need a human key: an administrator sets up a system anchor once (`sudo sezgi insan kur --sistem`; on Windows, `sezgi insan kur --sistem` in a terminal run as administrator), and every human step on real data (opening it, a real-data token, each approval, a limit change) then asks for the human's password in a terminal. On real data every write waits for that approval. An agent with shell access under the same user account, even one that opens a pseudo-terminal or edits Sezgi's files, cannot open, approve or re-sign anything without the password; this does not stop a program that learns the password or runs as administrator or root, nor direct calls to the app with the session Sezgi keeps (threat model: [SECURITY.md](https://github.com/atabaykut/Sezgi/blob/main/SECURITY.md)). Sezgi takes no backup and cannot undo a write.

```
npm install -g sezgi        # or run without installing: npx sezgi
cd path/to/app
sezgi init                  # map the application (a few seconds)
sezgi connect               # add Sezgi to your AI tool (asks tool by tool; `sezgi remove` undoes it)
sezgi mirror                # see what Sezgi knows, in the browser (this computer only)
sezgi help                  # all commands
```

This package is a small launcher. It needs Node.js 20 or later. npm installs the prebuilt binary for your platform (`@sezgi/cli-<os>-<cpu>`) as an optional dependency: Windows x64, macOS arm64/x64 (macOS 11 Big Sur or later) and Linux x64/arm64 (glibc 2.34+: Ubuntu 22.04+, Debian 12+, RHEL 9+; Alpine / musl is not supported). On other platforms build from source (`cargo build --release` in the repository) and point `SEZGI_IKILI` at the binary.

If you installed with `--omit=optional` (or from a lock file made on another platform), the binary is missing and Sezgi says so; reinstall with `npm install sezgi` (`npm install -g sezgi` for a global install).

License: Apache-2.0. © 2026 Aykut Atabay.
