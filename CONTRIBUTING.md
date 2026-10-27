# Contributing

Thank you for helping Sezgi understand more applications.

## The most valuable contribution: framework knowledge

Sezgi's accuracy depends on framework knowledge. NestJS conventions are **data**: [`adapters/nestjs.toml`](adapters/nestjs.toml) tells the engine which decorators mean a route, a body, a role, a public endpoint, a version or a deprecation. Many NestJS improvements are a few lines in that file.

Where the line is in 0.1: the framework is chosen in `src/kesif.rs` (one discovery step for `init`, `doktor` and `paylas`); the TypeScript class and decorator model, zod, the module graph and Prisma tracing are in `src/model.rs` and `src/adaptor.rs`; Express / Fastify / Next.js in `src/express.rs`; FastAPI in `src/python.rs` and `src/fastapi.rs`; front ends (React, Next.js, Preact, Vue, Svelte) and screen changes in `src/web.rs`, user journeys in `src/yolculuk.rs`; the behaviour summary (rejections, ownership checks, two-way conditions, cascades from schemas and migrations, events and jobs) in `src/dna.rs`; the evidence store and graph in `src/kanit.rs` and `src/graf.rs`; run-time observation in `src/gozlem.rs` and `src/senaryo.rs`; entity mappings and change plans in `src/varlik.rs` and `src/plan.rs`; applications without source code, Electron and the REA bridge in `src/kesif/`. A decorator-based TypeScript framework is mostly a new adapter file; a framework without decorators or another language needs Rust code. See the [glossary](docs/GLOSSARY.md) for the Turkish names.

### Workflow

1. Find a pattern Sezgi misses in a real application (an endpoint not found, a field missing, wrong auth).
2. Reproduce it in the smallest possible application (a `package.json` or `pyproject.toml` with the framework dependency and a few source files) and note the expected `kapilar.json` output.
3. Change the adapter file first; change Rust code only if the pattern cannot be expressed as data.
4. Run `sezgi init` on the small application and, if you can, measure a real application before and after (`kanit/karsilastir-openapi.js` against its OpenAPI document).
5. Open a pull request with the small application (as files in the description or a link), the expected and actual output and, if you measured, the numbers before and after.

### Build and checks

The toolchain is pinned in `rust-toolchain.toml` (Rust 1.99.0 with rustfmt and clippy); a C compiler is needed for tree-sitter. CI runs, and a pull request must pass:

```sh
cargo fmt --check
cargo clippy --release --all-targets -- -D warnings
cargo build --release --locked
cargo deny check                 # licences (deny.toml), advisories, sources
cargo audit
node kanit/belge-tutarlilik.js   # every number in the READMEs matches its source in kanit/
```

The integration test suite (fixture applications, the negative corpus of places where Sezgi must not claim certainty, live-mode and security tests) is maintained outside this repository; the repository holds the product only. Maintainers run your change against it and add your small application as a fixture; a fix comes with a test that failed before it.

### Rules

- **No guessing.** If something cannot be resolved statically, it must be reported as unresolved, not filled in.
- **General patterns only.** A rule must describe how a framework or a common library works, not one application's naming.
- **No regressions.** Re-measure the public benchmark (`kanit/acik`) when a change touches the adapters, and report numbers that went down as well as up.
- **No new dependencies** without a reason in the pull request; licences must be on the `deny.toml` list.

## Measuring

`kanit/` contains the benchmark tools: independent referees (`kanit/hakem/`: a NestJS referee, a TypeScript type-checker referee for table links, a FastAPI SDK referee), comparison against official OpenAPI files, and the agent task runner. Report numbers together with what was not measured; a held-out measurement is pre-registered before Sezgi is run.

## Developer Certificate of Origin

Contributions are accepted under the [DCO](https://developercertificate.org/). Sign off each commit (`git commit -s`) to certify you have the right to submit it under the Apache-2.0 license.
