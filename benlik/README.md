# Benlik Card

**An application's signed self-description for AI agents.**

A Benlik Card is a single JSON file, generated from an application's own code and published at `/.well-known/benlik.json`. It tells an AI agent which operations exist, what they need, which rules reject them, under whose identity writes happen, what data is kept — and how certain each of those facts is.

- Specification: [SPEC.md](SPEC.md) (Draft 0.1.1, one reference generator so far)
- Conformance checklist: [CONFORMANCE.md](CONFORMANCE.md)
- Schema: [schema/benlik-card.schema.json](schema/benlik-card.schema.json)
- Example: [examples/benlik.json](examples/benlik.json)
- Test vectors for validators: [test-vectors/](test-vectors/)
- Türkçe özet: [README.tr.md](README.tr.md)

## Licenses

Summary: [LICENSE](LICENSE) (CC BY 4.0 for the text, Apache-2.0 for schema, examples, test vectors and code).

- Specification text: [CC BY 4.0](LICENSE-CC-BY-4.0)
- Schema, examples and code: [Apache-2.0](LICENSE-APACHE-2.0)

Copyright © 2026 Benlik Card contributors.
