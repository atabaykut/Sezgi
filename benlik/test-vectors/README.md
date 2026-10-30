# Test vectors

Shared vectors for Benlik Card validators. Every implementation (Rust `benlik` crate, npm `benlik`, PyPI `benlik`) must give the same result for each file:

- `beklenen.valid`: the card conforms to the JSON Schema and to the rules the schema cannot express (SPEC §9, §3.4, §8.1), together with the evidence bundle when one is given. It says nothing about the card's signature: a card can be `valid` with `imza: invalid` (vector 25); such a card has no writable operation and an agent MUST ignore it (SPEC §8);
- `beklenen.writable`: ids of operations an agent may perform as writes, in card order;
- `beklenen.bayat` (optional): ids of stale operations;
- `beklenen.sunulmayan` (optional): evidence ids the presented bundle does not contain;
- `beklenen.imza`, `beklenen.paket_imzasi` (optional): status of the card and evidence bundle signatures, one of `unsigned`, `not_verified`, `invalid`, `untrusted_key` or `verified`.

Optional inputs in `girdi`: `simdi` (the current time, RFC 3339), `guncel_ozetler` (current SHA-256 of the project's files, by relative path), `kanit_paketi` (the evidence bundle), `anahtarlar` (the publisher's key document, `benlik-keys.json`) and `guvenilen_anahtarlar` (the keys the validator trusts, same shape).

Vectors 24-27 are a real card and evidence bundle signed by the reference generator (`sezgi init --imzala`, ML-DSA-65) with a key made for these vectors; only its public key is here. 25 is the same card changed after signing, 26 the bundle signed with the card's context string instead of `benlik-evidence`, 27 the card checked against a different trusted key.

Vector 28 carries `x-dna.data_effect.indirect` and `x-dna.branches` and stays valid; 29 is the same card with an indirect effect stamped `certain`, which the schema rejects.

Vector 30 adds events and jobs (`x-nodes`, `x-relationships`, SPEC §3.7) to card 02 and stays valid with the same writable operation; 31 stamps one of those edges `certain`, which the schema rejects. Vector 32 is a card generated without source code (SPEC §3.8; from the reference generator's output for an Android package, evidence members removed, a WebMCP origin added): valid, nothing writable. 33 is the same card with a capability stamped `certain` and another not requiring approval, which the schema rejects.

Write rules (SPEC §3, §8): an operation is writable only if its effect is not `read`, its stamp is `certain`, it is not stale (`stale: true`, or a file in `x-basis` changed), and the card is signed (`signature.alg` is not `none`) and not expired (`x-expires_at`). A non-writable operation does not make the card invalid. When `girdi.anahtarlar` is given, validators verify the signatures (contexts `benlik-card` and `benlik-evidence`) and an operation is writable only if the card signature is `verified`, that is, it verifies with a key among `guvenilen_anahtarlar`; a bundle signature that does not verify makes the card invalid. Without keys, validators report `not_verified` and `writable` assumes the caller verified the signature. A validator that cannot verify (the PyPI package without its `imza` extra) says so and makes nothing writable when keys are given.

Comparisons are exact: `"NONE"`, `"none "`, `"READ"` and `"true"` (a string for a boolean) are not accepted, and JSON Schema patterns are ECMA-262 patterns (`$` does not match before a trailing newline).
