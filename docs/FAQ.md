# Frequently asked questions

Answers to questions a skeptical engineer or AppSec reviewer is likely to ask. Sezgi is 0.1 and experimental; where the answer is "not yet", it says so. Measurements are in [`kanit/`](../kanit/README.md) (detailed files in Turkish).

### 1. Where are the tests, and how do I run them?

In the `testler/` folder. Run them with `bash testler/calistir.sh`; CI runs them on every push and pull request, next to `cargo fmt`, `cargo clippy -D warnings`, `cargo build --locked`, `cargo deny` and `cargo audit`. The suite includes fixture apps and a negative corpus: code patterns where Sezgi must not claim certainty. See [CONTRIBUTING.md](../CONTRIBUTING.md) for the other checks.

### 2. Will it work on my app the first time?

Maybe not. Sezgi was measured on apps whose code had not been looked at, each with a plan registered in advance. Two of five first runs failed badly: Express RealWorld found **0/19** endpoints ([SONUC-12](../kanit/SONUC-12-express-holdout.md)) and Airflow **0/134** ([SONUC-15](../kanit/SONUC-15-airflow-holdout.md)), which also produced 89 wrong auth decisions stamped `certain` in a diagnostic run. Both were fixed afterwards (19/19 and 134/134), but those later numbers are not held-out. Other first runs found endpoints but missed most body fields (Prefect 124/470, Directus 0/174). Run `sezgi rapor` after `sezgi init`: it lists unresolved schemas, partly read files, unchecked rules and endpoints without data links. `sezgi olc --openapi <file>` compares the map with your own OpenAPI document if you have one.

### 3. Aren't these your own benchmarks?

Yes, and that is a real limitation. To reduce bias, held-out runs are pre-registered (`HOLDOUT-ON-KAYIT*.md` committed before the run), ground truth is the app's own published OpenAPI or an independent referee, failures are published, and the public benchmark in [`kanit/acik`](../kanit/acik/README.md) reproduces with one command at pinned commits. Sample sizes are small: one model and one run for the agent comparison ([SONUC-3](../kanit/SONUC-3-immich-ajan.md)), one app per held-out run. No third party has repeated the measurements yet.

### 4. How much should I trust an `inferred` fact?

Treat it as unknown, not as likely. On Ghostfolio (held out), inferred auth decisions were right in only 18 of 46 cases ([SONUC-5](../kanit/SONUC-5-holdout-ghostfolio.md)). `certain` means read directly from code, but name-based guard recognition can still produce a wrong `certain` decision; [REFERENCE.md](REFERENCE.md#known-limitations) marks those cases "(not stamped)". Live mode never writes without approval on facts that are not `certain`.

### 5. How is it different from existing tools?

Each part of Sezgi exists elsewhere; what it adds is the combination and a confidence stamp on every fact. This comparison is qualitative. The only measured comparison is against a generic OpenAPI tool server written for the benchmark (`kanit/b-mcp.js`), not a third-party product.

| Tool category | Where it is stronger | What Sezgi adds |
|---|---|---|
| OWASP Noir (static endpoint discovery) | Built for security testing, community backed; aims at broad language and framework coverage | Table reads and writes per endpoint, external effects, reversibility, front-end links, a stamp and source line per fact. Sezgi has not been measured against Noir |
| Framework OpenAPI generators (NestJS Swagger, FastAPI, springdoc, Scramble) | First party in their framework; usually more accurate there; some see run-time information | Works without annotations or a running app; facts OpenAPI cannot express (tables, side effects, undo). Where a generator exists, Sezgi's `openapi.json` adds little |
| OpenAPI to MCP (Stainless, FastMCP, fastapi-mcp) | Mature, maintained; with a good OpenAPI document they produce tools quickly | Does not need a document. On Immich, whose document is rich, a generic OpenAPI server came close: 15/16 against 16/16, at about twice the tokens ([SONUC-3](../kanit/SONUC-3-immich-ajan.md)) |
| Serena, LSP-based code tools | General purpose, any code, any question, mature | A ready operation model (inputs, auth, tables) instead of symbols. Measured on one app with one model only |

If you have a good OpenAPI document and only need tool calling, an OpenAPI-to-MCP tool is the simpler choice.

### 6. Is it safe to let an agent write?

Writing is opt-in and off by default. Live mode starts on a declared test copy, with a permission token that opens only chosen operations. Deletes, external effects, irreversible or unknown-effect writes and credential changes wait for human approval. Writing to real data is experimental and needs a system anchor plus the human password for every write. Sezgi takes no backup and cannot undo a write. There has been no independent security audit. Details: [REFERENCE.md](REFERENCE.md#live-mode), [SECURITY.md](../SECURITY.md).

### 7. Can the approval decision be tampered with?

Approval decisions (destructive, external effect, reversibility, stamps, rules) are read from `kapilar.json`. The card carries `x-map_digest`, the canonical SHA-256 of that map. Live mode refuses writes when the digest does not match, when the card lacks it, or when an app that had a signed card now has an unsigned one. With an unsigned card this binding is only a consistency check: a process that can edit both files can recompute it. Sign the card (`sezgi init --sign`) and pin its key (`sezgi key pin`); real-data writes require a signed card with a pinned key. A process running as you can still edit your config folder; run the agent as a separate OS user for stronger isolation.

### 8. Does any data leave my machine?

Building the map uses no network and no language model; there is no telemetry. `sezgi mcp` without a token, `sezgi ayna` and `sezgi sun` make no outgoing requests, and Ayna and `sezgi sun` listen only on 127.0.0.1. Requests leave only in live mode (to your target), for OIDC sign-in, for a "current user" role endpoint you chose, and for `sezgi rea kur`. The text the MCP server returns goes to your AI tool, which may send it to its model provider. Full list: [SECURITY.md](../SECURITY.md#network).

### 9. What about prompt injection?

Code-derived text (summaries, field names, rule texts) reaches the agent. Sezgi replaces invisible and direction-changing characters, limits length and tells the agent the text is data. Visible instruction text such as "ignore previous instructions" is **not** removed. Responses from the target app in live mode (which can carry text its users wrote) are cleaned of the same invisible characters, cut at 8,000 characters and labelled as data, but their visible text is passed through as is. Treat any repository you map as untrusted input to your agent.

### 10. Does Sezgi back up or undo writes?

No. It reports each write's reversibility (yes, partly, no, unknown) with its reason, and an irreversible write always waits for approval. Back up the application yourself before such a change.

### 11. It has a single maintainer. What if it is abandoned?

That risk is real. Mitigations: Apache-2.0 license; outputs are plain JSON and OpenAPI 3.1 that other tools can read; `sezgi kaldir` undoes every change `sezgi bagla` made to your AI tools; removal is documented in [UNINSTALL.md](UNINSTALL.md). Nothing in your application's code is modified.

### 12. How is the supply chain handled?

Builds use the committed `Cargo.lock` with `--locked` and a pinned Rust toolchain. CI runs `cargo deny` (licences, advisories, sources) and `cargo audit`. GitHub Actions are pinned by commit SHA, and release builds get build-provenance attestations. The ML-DSA implementation (`ml-dsa` 0.1.x) has not had an independent audit. The npm launcher runs any binary named in `SEZGI_IKILI`; do not set it to something you do not control.

### 13. Why are the identifiers in Turkish?

Sezgi was written in Turkish. Commands, main flags and MCP tools have English aliases, and `--lang en` switches all output to English. Source identifiers and the internal `kapilar.json` format stay Turkish; [GLOSSARY.md](GLOSSARY.md) and [ARCHITECTURE.md](ARCHITECTURE.md) are the English entry points.

### 14. Who else reads the Benlik Card?

Nobody yet. Sezgi is currently the only generator and the only consumer. Two independent generators and a consuming agent are 1.0 goals ([REFERENCE.md](REFERENCE.md#roadmap)). Until then, treat the card as Sezgi's own export format.

### 15. Which platforms are supported?

Prebuilt through npm (Node.js 20+): Windows x64, macOS 11+ on Apple Silicon and Intel, Linux x64 and arm64 with glibc 2.34+. Alpine / musl, Windows on ARM and other platforms need a source build (`cargo build --release`, Rust 1.99+, a C compiler). macOS binaries are signed ad hoc, not notarized.
