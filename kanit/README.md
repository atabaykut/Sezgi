# Evidence

Every measurement behind Sezgi's documented numbers: methods, pre-registrations, results (failures included) and tools. Detailed files are in Turkish; this is the English summary. Numbers below are copied from the files; when a file and this page disagree, the file wins.

## How to read the results

- **Held-out:** measured once on an application whose code had not been looked at, with the plan, ground truth and pass threshold committed beforehand in a pre-registration file. The result is published whatever it is.
- **Development:** the adapter was written or fixed while looking at that application. These numbers show the adapter works there; they say nothing about generalization.
- **After fixes:** when a held-out run exposes bugs, they are fixed and the same app is measured again. That second number is no longer held-out, and the app no longer counts as an independent test.
- Ground truth is the application's own published OpenAPI document where one exists, otherwise an independent referee (`hakem/`, built on the TypeScript type checker) or a hand count.

## First held-out runs

Five held-out measurements of endpoint mapping have been made. Two failed completely on the first run, and two others missed most body fields:

| App (framework) | First run, pre-registered | After fixes (not held-out) | File |
|---|---|---|---|
| Ghostfolio (NestJS) | endpoints 120/120; auth 88/120 (73.3%); 4 wrong auth decisions stamped `certain` | auth 120/120; `certain` decisions 98/98 right | [SONUC-5](SONUC-5-holdout-ghostfolio.md), [SONUC-6](SONUC-6-ghostfolio-gerileme.md) |
| Prefect (FastAPI) | endpoints 177/179; body fields 124/470 (26.4%) | endpoints 179/179; body fields 470/470 | [SONUC-10](SONUC-10-fastapi-holdout.md) |
| RealWorld (Express) | **endpoints 0/19** | endpoints 19/19; auth 100%; body fields 6/6 | [SONUC-12](SONUC-12-express-holdout.md) |
| Airflow (FastAPI) | **endpoints 0/134**; in a diagnostic run, 89 auth decisions stamped `certain` were all wrong | endpoints 134/134; auth 99.3%; 0 wrong `certain`; body fields 275/275 | [SONUC-15](SONUC-15-airflow-holdout.md) |
| Directus (Express) | endpoints 128/136 (94.1%); auth 98.4%; **body fields 0/174** | endpoints 129/136; body fields 49/174 (28.2%) | [SONUC-16](SONUC-16-directus-holdout.md) |

The causes were general bugs (for example dotted file names in imports, chained routers, routers attached inside helper functions, `auto_error=False` read as "login optional" with a `certain` stamp). Each was fixed with a test first. A new held-out app is the only way to know whether the fixes generalize.

## All results

| # | What was measured | App | Kind | Headline result |
|---|---|---|---|---|
| [1](SONUC-1-immich.md) | Endpoint mapping vs. official OpenAPI | Immich server | Development | First: 237/279 endpoints, 0/419 body fields. After general fixes: 279/279, 436/485, auth 100% |
| [2](SONUC-2-calcom.md) | Same, adapter untouched first | Cal.com API v2 | Untouched, then development | Untouched: 117/121, auth 95.7%, fields 124/238 (52.1%). After fixes: 121/121, 99.2%, 250/250 |
| [3](SONUC-3-immich-ajan.md) | Agent tasks (16), one model, one run | Immich | Development app | Code only 14/16 at 39,521 tokens per task; official OpenAPI 15/16 at 43,561; Sezgi 16/16 at 20,467 |
| [4](SONUC-4-veri-baglari.md) | Table links vs. type-checker referee | Private app, Cal.com | Development | Private app 603 links, Cal.com 341 links; 0 extra, 0 missing |
| [5](SONUC-5-holdout-ghostfolio.md) | Endpoints, auth, table links | Ghostfolio | **Held-out** | 120/120; auth 88/120; table links precision 100%, recall 80.3% |
| [6](SONUC-6-ghostfolio-gerileme.md) | Regression after fixes | Ghostfolio | Not held-out | Auth 92/120, then 120/120; an intermediate build dropped to 38/120 and was fixed |
| [7](SONUC-7-artimli-guncelleme.md) | Update time after one file change | Immich, Cal.com | Timing | 66–83 ms and 314–319 ms (target < 500 ms) |
| [8](SONUC-8-01-hedefleri.md) | The eight 0.1 targets | Immich, Cal.com, Ghostfolio | Mixed | All met on one machine; startup about 10 ms; lookup p95 0.92 / 0.68 ms; rules 265/265 `certain` on Cal.com |
| [9](SONUC-9-fastapi-gelistirme.md) | FastAPI adapter | full-stack-fastapi-template | Development | 23/23 endpoints; auth 100%; fields 38/38 |
| [10](SONUC-10-fastapi-holdout.md) | FastAPI adapter | Prefect | **Held-out** | 177/179; body fields 124/470 (26.4%); after two fix rounds 179/179, 470/470 |
| [11](SONUC-11-islem-araclari.md) | Real writes on a test copy: per-operation tools vs. search tools | Immich test copy | Pre-registered | 12/12 at 3.83 turns and $0.201 per task vs. 11/12 at 6.33 turns and $0.233; one model, 6 tasks × 2 runs |
| [12](SONUC-12-express-holdout.md) | Express adapter | RealWorld Express | **Held-out** | **0/19**; after fixes 19/19 |
| [13](SONUC-13-olcek.md) | Scale (time and memory only) | Twenty server | Timing | 981,837 lines in 3,647 ms, peak 273 MB; accuracy not measured |
| [14](SONUC-14-hesap-ipuclari.md) | Compute-cost hints | Fooocus-API | Pre-registered | 0/7 cost fields, 0/3 light options; after fixes (not held-out) 7/7 and 3/3; wrong hints on apps without a GPU |
| [15](SONUC-15-airflow-holdout.md) | FastAPI adapter, second app | Airflow | **Held-out** | **0/134**; after fixes 134/134, auth 99.3% |
| [16](SONUC-16-directus-holdout.md) | Express adapter, second app | Directus | **Held-out** | 128/136, auth 98.4%, body fields **0/174**; after fixes 49/174 |
| [17](SONUC-17-django-flask.md) | Django and Flask adapters | RealWorld apps, DRF tutorial, microblog, flasky | Development | Django RealWorld 18/19, auth 18/18, fields 16/18; Flask RealWorld 19/19, auth 19/19, fields 18/18, required 9/18; hand counts all matched |

The private application mentioned in some files is not published; only its aggregate numbers are, and they cannot be reproduced by others.

## Pre-registration

Each held-out or pre-registered run has a file committed before the run that fixes the app and commit, the Sezgi commit or binary, the ground truth, the metrics and the expectation:

- [HOLDOUT-ON-KAYIT.md](HOLDOUT-ON-KAYIT.md): Ghostfolio
- [HOLDOUT-ON-KAYIT-2-fastapi.md](HOLDOUT-ON-KAYIT-2-fastapi.md): Prefect
- [HOLDOUT-ON-KAYIT-4-express.md](HOLDOUT-ON-KAYIT-4-express.md): RealWorld Express
- [HOLDOUT-ON-KAYIT-6-airflow.md](HOLDOUT-ON-KAYIT-6-airflow.md): Airflow
- [HOLDOUT-ON-KAYIT-7-directus.md](HOLDOUT-ON-KAYIT-7-directus.md): Directus
- [ON-KAYIT-3-islem-araclari.md](ON-KAYIT-3-islem-araclari.md): operation tools (SONUC-11)
- [ON-KAYIT-5-hesap-ipuclari.md](ON-KAYIT-5-hesap-ipuclari.md): compute hints (SONUC-14)

Classifying differences (Sezgi bug, documentation gap, not knowable from code) happened after each run, as the files state.

## Correction note

[DUZELTME-NOTU-1.md](DUZELTME-NOTU-1.md) records inconsistencies found after the Ghostfolio run. The most important: the pre-registration's shortened adapter digest was wrong (a mix of two digests), and the adapter engine `adaptor.rs` changed between the first freeze and the pre-registration. That earlier version was not kept, so the pre-registration's claim that the inference logic did not change cannot be verified. The rule file `adapters/nestjs.toml` did not change, and the pre-registration was still written before Sezgi ran on Ghostfolio. The note also lists timing numbers removed from the READMEs because they had no source.

## Reproducing the public benchmark

[`acik/`](acik/README.md) reproduces the Immich and Cal.com numbers at pinned commits. It needs the pinned Rust toolchain, Node.js 22+, git and bash (Git Bash on Windows). From `kanit/acik`:

```sh
./getir.sh                    # sparse clone of Immich and Cal.com at pinned commits (read only)
npm ci --ignore-scripts       # referee tools, installed only in this folder
./olc.sh                      # build Sezgi, map both apps, compare; summary in sonuc/OZET.md
```

The card schema check also needs the Benlik Card schema (see [`acik/README.md`](acik/README.md)). The measured repositories are only read, never built or run. Expected results: [`acik/sonuc/OZET.md`](acik/sonuc/OZET.md); the pre-correction summary is [`OZET-duzeltme-oncesi.md`](acik/sonuc/OZET-duzeltme-oncesi.md). Other result files end with their reproduction commands.

## Tools in this folder

- `karsilastir-openapi.js`: compares a map with an official OpenAPI document.
- `hakem/`: independent referees (NestJS, type-checker table links, FastAPI SDK).
- `olc.js`, `b-mcp.js`: the agent benchmark and the generic OpenAPI tool server used as setup B.
- `islem-araclari-olc.js`, `hesap-ipuclari-olc.js`, `olc-hedefler.js`: harnesses for SONUC-11, SONUC-14 and SONUC-8.
