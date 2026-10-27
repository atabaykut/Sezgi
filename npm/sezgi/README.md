# Sezgi

*Sezgi is Turkish for intuition: knowing without thinking.*

**Let your AI assistant understand your application, and work in it safely.**

When an assistant such as Claude, Cursor or Copilot needs to do something in your application, it scans the code from scratch every time and guesses. Sezgi reads the code **once** and builds a map: every operation with its inputs and validation rules, who can call it, which database tables it touches, its outside effects (e-mail, payment, queues, cloud) and whether it can be undone. The assistant asks Sezgi over MCP instead of scanning code, and gets small answers with their source. Every fact is stamped *certain*, *observed*, *inferred* or *unknown*; Sezgi reports gaps instead of filling them.

Supported: TypeScript + NestJS, JavaScript / TypeScript + Express, Fastify, Next.js, Python + FastAPI, Django and Flask, Java / Kotlin + Spring Boot, Kotlin + Ktor, Ruby on Rails, C# + ASP.NET Core, Go (net/http, Gin, Echo, Chi, Fiber, gorilla/mux) and PHP + Laravel, plus React, Vue, Angular, Svelte, Remix and Solid front ends. Output is in English or Turkish.

## Get started

```sh
npm install -g sezgi      # or without installing: npx sezgi <command>
cd path/to/your/app
sezgi init                # reads the code, writes the map to .sezgi/ (a few seconds)
sezgi connect             # adds Sezgi to your AI tool; asks tool by tool, `sezgi remove` undoes it
```

Then restart your assistant and ask, for example: "If I delete a user, which tables are affected, and can it be undone?"

Optional: `sezgi mirror` shows in the browser what Sezgi knows (this computer only), `sezgi watch` keeps the map up to date, `sezgi doctor` explains what is missing, `sezgi help` lists every command.

## Safety

Sezgi only reads by default and never writes to your code. An opt-in live mode lets the assistant call a **test copy** of your application with your session and only the operations you allow; deletes, outside effects, irreversible writes, credential changes and anything uncertain wait for your approval, and everything goes to an encrypted, hash-chained log. Writing to real data is experimental: it needs a key set up once by an administrator and the human's password for every write, so an agent with shell access under the same account cannot open real data or approve anything without the password. It does not stop a program that learns the password or runs as administrator ([SECURITY.md](https://github.com/atabaykut/Sezgi/blob/main/SECURITY.md)). Sezgi takes no backup and cannot undo a write.

## Platforms

This package is a small launcher and needs Node.js 20 or later. npm installs only the prebuilt program for your computer (`@sezgi/cli-<os>-<cpu>`): Windows x64, macOS arm64 / x64 (macOS 11+), Linux x64 / arm64 (glibc 2.34+: Ubuntu 22.04+, Debian 12+, RHEL 9+). Alpine / musl is not supported; on other platforms build from source (`cargo build --release`) and point `SEZGI_IKILI` at the binary.

If you installed with `--omit=optional` (or from a lock file made on another platform), the program is missing and Sezgi says so; reinstall with `npm install sezgi` (`npm install -g sezgi` for a global install).

Documentation: [github.com/atabaykut/Sezgi](https://github.com/atabaykut/Sezgi).

License: Apache-2.0. © 2026 Sezgi contributors.
