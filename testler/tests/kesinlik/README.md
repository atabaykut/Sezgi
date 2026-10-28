# Negative certainty corpus

Each folder is a small NestJS app built around one trap: a pattern where Sezgi must **not** claim a fact as certain. `tests/kesinlik.rs` maps every case and fails if any forbidden claim appears with a certain stamp.

`beklenti.json` lists the checks (`konu`):

| Check | Fails when |
|---|---|
| `giris` | the endpoint's login requirement equals `yasak` and its evidence is `kesin` |
| `alan` | body field `alan` has required = `yasak_zorunlu` as a certain fact (`null`: any certain value), in the map or in the card |
| `sema` | the body schema is resolved as certain but its fields differ from `beklenen_alanlar` (`null`: must be reported uncertain; `cozulmeli: true`: a supported pattern, must be resolved correctly) |
| `onizle` | the preview result starts with `yasak` (for example `GEÇER` with a missing required field) |
| `giris_kesin` | positive check: a traceable pattern must give login requirement `beklenen` as certain |

Every case (and every app in `tests/ornekler`) is also checked against general invariants: a `certain` operation has only `certain` inputs and a certain login decision; a field whose required flag is uncertain in the map is never `certain` in the card.

Adding a case: create `<id>-<short-name>/` with `package.json` (must depend on `@nestjs/core`), `src/…` and `beklenti.json`. Write the case first and see it fail, then fix the code. The test runs in CI with `cargo test`.

| Case | Trap |
|---|---|
| `k1-1-soru-isareti` | `?` without `@IsOptional`, fields without validators, custom and conditional optional decorators |
| `k1-2-ic-ice-turetme` | nested `PartialType` / `PickType` / `OmitType` / `IntersectionType`, `mixin`, keys from a variable |
| `k1-3a/b/c` | `APP_GUARD` in a comment, in a test module, in a sibling package not imported |
| `k1-4a/b/c` | public markers by definition (`SetMetadata`), guard reading another key, marker without definition |
| `o6-…` | global login guard plus route-level optional guard |
| `y1-…` | Passport strategies that start a login (`AuthGuard('google')`, `AuthGuard('local')`) |
