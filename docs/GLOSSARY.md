# Glossary (Turkish → English)

Sezgi's commands, MCP tools and the internal map file use Turkish names. English aliases exist for commands, the main flags and MCP tools (`sezgi help` lists both names). `kapilar.json` is an **internal format and not yet stable**; the interoperable, English format is the [Benlik Card](https://github.com/atabaykut/Sezgi/tree/main/benlik).

## Commands

| Turkish | English | Meaning |
|---|---|---|
| `sezgi init` | — | build the map (`.sezgi/`) |
| `sezgi kesfet` | `sezgi discover` | map an application without source code (executables, ELF, APKs, SQLite, Electron; more with REA) |
| `sezgi izle` | `sezgi watch` | keep the map current as the code changes |
| `sezgi doktor` | `sezgi doctor` | health check with the fixing command |
| `sezgi bagla` | `sezgi connect` | connect AI tools (asks tool by tool) |
| `sezgi kaldir` | `sezgi remove` | undo everything `bagla` did |
| `sezgi mcp` | — | MCP server |
| `sezgi sun` | `sezgi serve` | local HTTP + OpenAPI bridge |
| `sezgi santral` | `sezgi switchboard` | several applications behind one MCP server; `ekle` (`add`), `cikar` (`remove`), `baglar` (`links`), `bagla-varlik` (`link-entity`), `varliklar` (`entities`), `varlik-gozle` (`observe-entity`) |
| `sezgi ayna` | `sezgi mirror` | local UI: what the app is, what Sezgi can do and has ready here, and the "Ne yapmak istiyorsun?" search (Sezgi tab); the 17 founding questions with trust stamps (Ne biliyoruz tab); each operation's effects: call chain, tables, authorization, external effects, reversibility, approval (Kapılar tab); evidence, preview, tokens; opens only with its keyed address, also without a map |
| `sezgi rapor` | `sezgi report` | coverage report |
| `sezgi karsilastir` | `sezgi diff` | compare two cards, breaking changes |
| `sezgi olc` | `sezgi measure` | measure on your own app |
| `sezgi gozlem` | `sezgi observe` | run-time observation: `al` (`import`) a HAR file, `senaryo` (`scenario`) a browser scenario; not the same as observe mode (`sezgi mod gozlem`) |
| `sezgi canli` | `sezgi live` | live mode status; `canli kur` (`live setup`) |
| `sezgi mod` | `sezgi mode` | `gozlem` (observe) · `okuma` (read) · `yazma` (write) |
| `sezgi hedef` | `sezgi target` | where requests go; `--test-kopyasi` declares a test copy, `--gercek-veri` opens writes on real data (terminal only; experimental) |
| `sezgi oturum` | `sezgi session` | the human's own session and verified roles |
| `sezgi token` | — | permission tokens for agents |
| `sezgi onay` | `sezgi approve` | requests waiting for human approval |
| `sezgi plan` | — | change plans across apps: `listele` (`list`), `goster` (`show`), `onayla` (`approve`), `reddet` (`reject`) |
| `sezgi kayit` | `sezgi log` | encrypted, hash-chained request log |
| `sezgi anahtar` | `sezgi key` | create a signing key, pin an app's key |
| `sezgi insan` | `sezgi human` | the human key for real data: `durum` (`status`) shows the system anchor, `kur --sistem` (`setup`, as administrator) sets the human password |
| `sezgi dogrula` | `sezgi verify` | verify a card: signature, validity period, evidence bundle, staleness |
| `sezgi defter` | `sezgi ledger` | what Sezgi keeps and for how long |
| `sezgi unut` | `sezgi forget` | deletion request (KVKK / GDPR) |
| `sezgi hafiza` | `sezgi memory` | experience memory (off by default) |
| `sezgi paylas` | `sezgi share` | signed, compressed map for the team |
| `sezgi rea` | — | install (`kur`, `install`), check (`durum`, `status`) or remove (`kaldir`, `uninstall`) the optional, version-pinned REA |
| `sezgi teshis` | `sezgi diagnostics` | local counters only |

## Flags

| Turkish | English | Meaning |
|---|---|---|
| `--cikti` | `--out` | output folder |
| `--imzala [key]` | `--sign [key]` | sign the Benlik Card |
| `--on-yuz <dir>` | `--frontend <dir>` | the front-end folder |
| `--gecerlilik <days>` | — | how long the card is valid (1–365, default 30) |
| `--deneme` | `--dry-run` | write nothing, explain |
| `--harita` | `--map` | map folder |
| `--sert` | `--strict` | exit 1 on a breaking change |
| `--arac` | `--tool` | which AI tools to connect |
| `--yonlendir` | `--steer` | add an "ask Sezgi first" line to the tool's instructions |
| `--elle` | `--manual` | print the connection lines, write nothing |
| `--harita-sil` | `--delete-map` | also delete the map |
| `--evet` | `--yes` | do not ask |
| `--hepsi` | `--all` | everything |
| `--kisi` | `--person` | a person's id |
| `--ad` | `--name` | name |
| `--test-kopyasi` | — | the target is a test copy |
| `--gercek-veri` | — | `hedef`: open writes on real data; `token olustur`: a token that may write on real data (both terminal only, with the human password) |
| `--sistem` | `--system` | `insan kur`: write the anchor to the system location (the only kind of anchor) |
| `--kullanici <uid\|name>` | `--user` | `insan kur`: whose anchor, as a uid or a user name (on Linux and macOS the name is looked up in `/etc/passwd`; default: the user who ran `sudo`, or the current user on Windows) |
| `--saatlik-yazma <n>` | — | `hedef`: real-data writes per hour, 1–200 (default 20; terminal only, with the human password) |
| `--izin <pattern:level>` | — | token permission: `oku` read, `yaz` write, `yikici` destructive |
| `--tum-islemler` | — | confirm a `*` pattern that opens writes on everything |
| `--kesin` | — | `token olustur`: a permission pattern that matches no operation is an error, not a warning |
| `--rol`, `--rol-ucu`, `--rol-claim` | — | the human's verified role: declared in a terminal, from the app's "current user" endpoint, from an OIDC claim |
| `--lang en\|tr` | — | output language of every command (also MCP tool names and texts); otherwise `SEZGI_LANG`, then the system language, then Turkish; Ayna also has an **EN / TR** button |
| `--rea` | — | use REA (`kesfet`; install it with `sezgi rea kur`); with `doktor`, check the REA bridge |
| `--yetenekler` | — | list the REA commands Sezgi uses, their class and availability (`kesfet`) |
| `--onayla` | — | approve an REA command that starts a process, writes files or reaches a browser |
| `--yalitim konteyner\|vm\|yok` | `container`, `vm`, `none` (values) | isolation declaration: container, virtual machine or none |
| `--calistir` | — | allow a command that runs or changes the target |
| `--sahibiyim` | — | declare that you own the target |
| `--webmcp <CDP>` | — | discover the WebMCP tools of a page in a browser on this computer |
| `--kaynak <origin>` | — | the origin of that page |
| `--cdp <address>` | — | the loopback debugging address of the browser a scenario runs in |
| `--isaretler <json>` | — | time marks to link UI steps to HAR requests (inferred only) |
| `--ornek <n>` | — | how many records `varlik-gozle` compares (default 50) |
| `--anahtar <name>` | — | a note naming the key of an entity mapping |
| `--adim <n>`, `--ozet <digest>` | — | the plan step to approve and the first 8 characters of its content digest |

## MCP tools (`--lang en`)

| Turkish | English | Arguments (Turkish → English) |
|---|---|---|
| `kimsin` ("who are you") | `app` | — |
| `ara` ("search") | `search` | `sorgu` → `query`, `adet` → `limit`, `gorev` → `task` |
| `tani` ("identify") | `describe` | `kapi` → `endpoint` |
| `onizle` ("preview") | `preview` | `kapi` → `endpoint`, `govde` → `body`, `sorgu` → `query_params`, `parametreler` → `params`, `oturum` → `logged_in`, `roller` → `roles`, `islem` → `operation` |
| `kanit` ("evidence") | `evidence` | `kapi` → `endpoint`, `islem` → `operation`, `varlik` → `entity` |
| `bilinmeyenler` ("unknowns") | `unknowns` | `kapi` → `endpoint`, `islem` → `operation`, `adet` → `limit` |
| `etki` ("impact") | `impact` | `kapi` → `endpoint`, `islem` → `operation`, `varlik` → `entity`, `derinlik` → `depth` |
| `cagir` ("call") | `call` | as `onizle`, plus `tekrar_anahtari` (idempotency key) |
| `onay_durumu` | `approval_status` | `id` |
| `baglar` ("links", `--santral` only) | — | `kapi` |
| `varliklar` ("entities", `--santral` only) | `entities` | — |
| `plan` (`--santral` only) | — | `baslik` (title), `adimlar` (steps: `uygulama` app, `kapi` endpoint, `islem` operation, `govde` body, `sorgu` query, `parametreler` params) |

With a token, each operation the token allows is also its own tool (e.g. `kitap_olustur`). With `--lang en`, tool names, instructions and every answer (preview details, behaviour summaries, reversibility reasons, evidence notes, task plans, errors) are English; details taken from the code stay as they are. In Ayna the **EN / TR** button switches the language.

## Files in `.sezgi/`

| File | Meaning |
|---|---|
| `kapilar.json` | the map (`kapi` = gate = endpoint) |
| `benlik.json` | Benlik Card |
| `benlik-evidence.json` | the card's evidence bundle (kanıt paketi) |
| `benlik-keys.json` | public key document (when signed) |
| `openapi.json` | the app's OpenAPI 3.1 document, generated from code |
| `kanit.json` | evidence store: evidence, claims (önerme) and their links |
| `graf.json` | the same claims as nodes and edges; function calls are `fn:file#Class.method -[CALLS]-> fn:...` edges |
| `web-baglari.json` | front-end links: screen → UI action → client call → endpoint |
| `paylasim.bin` | signed, compressed map for the team (`sezgi paylas`) |
| `kaynaksiz.json` | inspection without source code (`sezgi kesfet`) |
| `gozlem.json` | run-time observations: request shapes only (`sezgi gozlem`, live mode) |

In the user's configuration directory, the switchboard keeps `santral.json` (registered apps), `santral-baglar.json` (cross-links), `santral-varliklar.json` (entity mappings) and `santral-planlar/` (change plans), all owner-only.

## `kapilar.json`

| Key | Meaning |
|---|---|
| `endpointler` | endpoints |
| `kapi` | gate = endpoint |
| `yontem`, `yol` | HTTP method, path |
| `kontrolcu`, `fonksiyon`, `dosya`, `satir` | controller, method, file, line |
| `giris` | login: `gerekli` required, `herkese_acik` public, `istege_bagli` optional, `bilinmiyor` unknown (decided at run time, e.g. Django REST Framework `get_permissions`) |
| `giris_kaniti` | evidence for `giris`: `kesin` (decorator, recognized guard or recognized middleware), `varsayim` (inferred from absence), `varsayim_bilinmeyen_guard` (inferred, unrecognized guard present), `varsayim_isaret` (a public marker that could not be verified) |
| `guardlar`, `roller`, `roller_kaniti` | guards, roles / permissions, evidence for the role decision |
| `ozet` | summary |
| `govde`, `sorgu`, `parametreler` | body, query, path parameters |
| `sema`, `dto`, `alanlar`, `bulunamadi`, `dizi` | schema, DTO name, fields, not resolved, array body |
| `zorunlu`, `belirsiz`, `tip`, `kurallar`, `degerler` | required, requiredness uncertain, type, rules, allowed values |
| `veri.okur`, `veri.yazar`, `veri.eksik_olabilir` | tables read, tables written, list may be incomplete |
| `ozet.dna[...].cagri` | call chain summary: handler (`isleyici`), functions reached (`erisilen`), depth (`derinlik`), unresolved dynamic calls (`dinamik`), limit reached (`kesildi`), chain (`zincir`) and its weakest link (`zincir_guveni`) |
| `ozet.dna[...].tablo_etkisi`, `erisim_denetimi` | tables reached through the call chain (`tablo`, `islem` okur/yazar, `kaynak` ORM, `zincir`); access checks (`ifade`, `izin` permission, `nesne_duzeyi` object-level) |
| `yanit.kaynak` | where the response schema was read: `response_model`, `donus_tipi` (return type), `ApiResponse`, `res.json`; Rails `jbuilder`, `serializer`, `render_json` |
| `kullanimdan_kalkmis`, `yerine` | deprecated, replaced by |
| `surum` | API version |
| `uygulama`, `ana` | app (root module), belongs to the primary app |
| `ozet` (top level) | summary of the run: files, lines, timings (`sure_ms`), unresolved schemas (`dto_bulunamayan`), skipped large files (`buyuk_diye_atlanan`), partially parsed files (`kismen_okunan_dosya`), files read with their SHA-256 (`okunan_dosyalar`), the front-end folder (`on_yuz`), routes registered only under conditions (`kosullu_kapilar`), route registrations whose path is built at run time and so are not in the map (`dinamik_kayitlar`, Go and Ktor), URL registrations the adapter did not map with the reason, such as the Django admin site or a URL configuration outside the project (`haritalanmayan`) |

## Spring Boot (Java and Kotlin) map

| Key or term | Meaning for a Spring Boot app |
|---|---|
| `spring@0.1.0` | adapter id (`ozet.adaptor`) |
| `genel_onek` | `server.servlet.context-path` (with `spring.mvc.servlet.path`); every path starts with it |
| `kontrolcu`, `fonksiyon` | controller class, handler method (the implementing class when the mapping is declared on an interface) |
| `guardlar` | the `SecurityFilterChain` rule that decided (`bean: requestMatchers(...).permitAll()`) and the method security annotation |
| `kosullu_kapilar` | routes of a controller registered under `@Profile` or `@ConditionalOn…` |
| `denetlenemeyen` | per operation: a body without `@Valid` (its Jakarta constraints do not run), a Kotlin constraint on a constructor property without `@field:` (it does not validate the field), multipart files, a method-less `@RequestMapping` |
| `@X (AOP)` (in `guardlar`) | a project annotation an `@Aspect` cuts: the check is the app's own code, an unrecognized guard |
| `event:X`, `job:C.m` | a Spring event type, Kafka topic or JMS / Rabbit destination; an `@Async` or `@Scheduled` method |

## Ktor (Kotlin) map

| Key or term | Meaning for a Ktor app |
|---|---|
| `ktor@0.1.0` | adapter id (`ozet.adaptor`) |
| `genel_onek` | `ktor.deployment.rootPath` from `application.conf` / `.yaml`; every path starts with it |
| `kontrolcu`, `fonksiyon` | the first fixed path segment (after `api` and a version such as `v1`) and the HTTP method in lower case: the card's operation id (`gorevler.get`) |
| `guardlar` | `authenticate("name")` (with `optional = true` when login is optional), the project's route wrappers (`rolIle("ADMIN")`) and route-scoped plugins (`install(X)`) |
| `genel_guardlar` | authentication providers installed with `install(Authentication)` (`Authentication jwt(auth-jwt)`) |
| `kosullu_kapilar` | routes registered inside `if` / `when` / a loop |
| `dinamik_kayitlar` | registrations whose path or method could not be resolved; not in the map |
| `denetlenemeyen` | per operation: a RequestValidation `validate<Dto> { }` check, an unrecognized JSON serializer, a file upload, an optional path parameter (`{id?}`) |
| `mod:` (graph) | a route function (`dosya:gorevRotalari`); it `DEPENDS_ON` the function that called it |
| `db:` tables | Exposed table objects (`object Gorevler : LongIdTable("gorevler")`; without a name the object name with a trailing `Table` dropped) and JPA entities |

## ASP.NET Core (C#) map

| Key or term | Meaning for an ASP.NET Core app |
|---|---|
| `aspnet@0.1.0` | adapter id (`ozet.adaptor`) |
| `kontrolcu`, `fonksiyon` | controller class and action (`[ActionName]` applied, the `Async` suffix dropped); for a Minimal API the first `WithTags` tag (else the handler's class or the file) and the handler method or `WithName` |
| `guardlar` | the authorization markers on the endpoint: `Authorize(Policy = …)`, `RequireAuthorization(…)`, `AllowAnonymous`, `FallbackPolicy (…)`, `AuthorizeFilter (MVC genel filtresi)` |
| `genel_guardlar` | the global `FallbackPolicy`, the MVC global `AuthorizeFilter` and middleware that answers 401 / 403 |
| `surum` | the API version from `[ApiVersion]` / `HasApiVersion`, when the endpoint has exactly one |
| `kosullu_kapilar` | routes registered under an `if`, for example `app.Environment.IsDevelopment()` |
| `islem_kimlikleri` | operation ids from `[HttpGet(Name = …)]` and `.WithName(…)` |
| `denetlenemeyen` | per operation: uploaded files (`IFormFile`) the preview cannot check |
| `belirsiz` | requiredness the framework does not enforce: a Minimal API without `AddValidation()`, a controller without `[ApiController]`, `[Required]` on a value type |
| `ek_paketler` (top level) | projects referenced with `<ProjectReference>` outside the app folder |
| `mod:` (graph) | a controller (`dosya:Sinif`) or a Minimal API route group (`dosya:/önek`, nested groups `DEPENDS_ON` their parent) |

## Stamps and confidence

| Turkish | Value | Meaning |
|---|---|---|
| Kesin | `certain` | read directly from code (a definition) |
| Gözlem | `observed` | seen in shipped files or at run time, not derived from source; never enables a write on its own |
| Yorum | `inferred` | concluded from absence, naming or heuristics |
| Bilinmiyor | `unknown` | could not be determined |
| Veri, Onaylı | `data`, `approved` | Benlik Card 0.1 values; since Draft 0.1.1 they count as observed and do not enable writes |

## Other terms

| Turkish | English | Meaning |
|---|---|---|
| kanıt | evidence | an observation: source kind, file, line, digest |
| önerme | claim | what the evidence says about an endpoint, table, role or screen |
| kanıt paketi | evidence bundle | `benlik-evidence.json`, bound to the card by digests and a Merkle root |
| dayanak özetleri | basis digests | `x-basis`: SHA-256 of the files the card relies on |
| davranış özeti | behaviour summary | `x-dna` in the card |
| bayat | stale | a source file changed since the map was built |
| ara katman | middleware | a Laravel or Express request filter; a Laravel alias (`auth`, `role`) is resolved to its class before a decision |
| rota dosyası | route file | a Laravel `routes/*.php` file; with what loads it (`bootstrap/app.php`, a service provider, `require`) a `mod:` node |
| gözlem | observation | a request seen at run time, kept as its shape only (method, path template, status, field names) |
| yolculuk | journey | screen and UI-action steps that end in an operation (`x-journeys`) |
| ekran geçişi | screen change | a `NAVIGATES` link from one screen to another |
| ekran koruyucusu | screen guard | an Angular `canActivate` / `canMatch` / `canActivateChild` or Vue `beforeEnter` / `meta.requiresAuth` rule on a screen; in `web-baglari.json` the screen gets `korumalar`, `giris` (`gerekli` required, `misafir` only for signed-out users, `bilinmiyor`) and `giris_guven: inferred`, and a `REQUIRES_PERMISSION` edge to `guard:web/<name>` |
| sunucu fonksiyonu | server function | a `"use server"` function (SolidStart, React); its calls are followed, it is not an endpoint itself (`istatistik.sunucu_fonksiyonu`) |
| şablon | template | the markup of an Angular, Vue or Svelte component; its events become UI actions and its links screen changes |
| rota modülü | route module | a Remix / React Router route file; `<Form>`, `useFetcher` and `useSubmit` reach its `action` or `loader` |
| yanıt şeması | response schema | the successful response read from code (`x-response`) |
| kullanım yeri hedefi | use-site target | Kotlin's `@field:` / `@get:` / `@param:` before an annotation; without it, a Bean Validation constraint on a constructor property goes to the parameter and does not validate the field |
| rota fonksiyonu | route function | a Kotlin function that registers Ktor routes (`fun Route.gorevRotalari(...)`); opened where it is called, with the caller's prefix and login |
| sarmalayıcı | wrapper | a project function that takes the routes as a lambda (`fun Route.rolIle(rol: String, build: Route.() -> Unit)`); its body decides what applies to them |
| olay, iş | event, job | internal events and queued jobs with their listeners (`x-nodes`, `x-relationships`) |
| kaskad, dolaylı yazma | cascade, indirect write | rows another delete removes through the schema (`x-dna.data_effect.indirect`) |
| sahiplik denetimi | ownership check | a comparison of the record's owner field with the requester's identity (a uniqueness check or "not your own account" is not one) |
| çağrı grafiği, çağrı zinciri | call graph, call chain | function-to-function `CALLS` edges; the path from an endpoint's handler down to the data layer |
| nesne düzeyi erişim denetimi (`NESNE_YETKI`) | object-level access check | a call such as `requireAccess(...)` or `authorize('update', $post)` that decides access to a specific record; `YETKI_DENETIMI` when it carries no record |
| dinamik çağrı | dynamic call | a call whose target the code does not tell (index access, reflection, a function passed in); counted, never guessed |
| dış etki, dış sistem | external effect, external system | an outgoing call (HTTP, queue, e-mail, payment, cloud SDK) from an operation's call chain; node `external:<host | ortam.NAME | sdk>`, `x-dna.side_effects`, `x-external` |
| `dis_etki` (var / bilinmiyor / bulunmadi) | external effect status (found / unknown / none found) | from the call graph: `bilinmiyor` when the chain has an unclassified library call, a dynamic call, too many candidates or is cut at the depth limit; a write with `bilinmiyor` needs approval (fail-closed) |
| polimorfik dağıtım, aday | polymorphic dispatch, candidate | a method call that may reach several same-named methods (subclasses, or the project when the receiver's type is unknown); followed for external effects, at most `inferred` |
| geri alınabilir | reversible | `evet` / `kismen` / `hayir` / `bilinmiyor` with a reason (soft delete, restore endpoint, hard delete, cascade, external effect); at most `inferred` |
| etki onayı | effect approval | `etki_onayi` on an endpoint in `kapilar.json`: why a write that reaches outside or cannot be undone always waits for human approval |
| ön koşul | precondition | a conditional rejection in the handler or its services (`if (!x) throw new NotFoundException()`, `abort_unless`), with its status code when known |
| varlık eşlemesi | entity mapping | the same real-world thing in two apps, mapped at schema level |
| iş alanı | domain | what kind of application it is ("photo and video library", "e-commerce"), from a dictionary and counted evidence; `unknown` when the evidence is not enough (`app.x-meaning.domain`) |
| amaç alıntısı | purpose quote | the app's own sentence about itself (README, package description), quoted with file and line; at most `inferred` |
| kullanıcı tipi | user type | anonymous visitor, signed-in user, API key / token client, administrator or a role name (`actor:` nodes) |
| API anahtarı / belirteci ile istemci | API key / token client | user type of operations behind a guard that reads a key or token from the request and answers 401 / 403 (`actor:api_client`, guard marked `[api key/token]`) |
| iş yeteneği | business capability | operations grouped by what they do for a user ("share album"): a `cap:` node its endpoints `IMPLEMENTS` |
| görev planı | task plan | a natural-language task split into ordered steps with endpoint, roles and approval (`ara` with `gorev: true`); it runs nothing |
| fiil-nesne eşlemesi | verb-object matching | how a task step is resolved: the object to a resource, the verb to the operation's action; a read verb never picks a write |
| değişiklik planı | change plan | steps across several apps that a human approves one by one; it runs nothing by itself |
| yalıtım | isolation | the container or VM an approved REA command runs in, as declared by the human |
| otomatik, onaylı, dinamik | automatic, approved, dynamic | REA command classes: records evidence only; starts a process, writes files or reaches a browser; runs or changes the target |
| doğrulanmış rol | verified role | a role the human holds, from their declaration in a terminal, a verified OIDC claim or the app's "current user" endpoint |
| test kopyası | test copy | a target declared as a copy; writes follow the normal approval rules |
| gerçek veri (`GERÇEK VERİ`) | real data | a target where a human opened writes in a terminal with the human password (experimental): no automatic writes, approval only in a terminal with the password, hourly limit |
| koruma sınırı | protection boundary | what a check stops and what it does not: the terminal check stops an agent connected over MCP but not one with shell access under the same user account; the human password with a system anchor stops that one too, but not a program that learns the password or runs as administrator |
| insan anahtarı, insan parolası | human key, human password | a password only the human knows; Sezgi derives a signing key from it (PBKDF2-HMAC-SHA256, Ed25519) and never stores the password or the private key |
| sistem çapası | system anchor | the file with the human key's salt and public key, owned by root or Administrators so the user (and an agent running as the user) cannot change it |
| insan imzası, kapalıya düşme | human signature, fail-closed | records a human step leaves (real-data target, real-data token, roles) are signed with the human key; an edited record fails verification and its permission is ignored |
| sahte terminal | pseudo-terminal | a terminal a program opens for another program (for example `script`); it passes the terminal check |
| dinamik kayıt, dinamik önek | dynamic registration, dynamic prefix | a Go route whose path, or a group whose prefix, is built at run time: the route is not in the map, or is kept as secondary |
