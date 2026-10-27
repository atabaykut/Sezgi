# Genelleme testi: Üçüncü NestJS uygulaması (Cal.com API v2), adaptöre dokunmadan

**Tarih:** 14.10.2026
**Uygulama:** [Cal.com](https://github.com/calcom/cal.com) `apps/api/v2`, commit `54343aa`. Başka bir ekip tarafından yazılmış, 569 dosya, 64.663 satır.
**Kural:** Sezgi'nin adaptörü ve kodu bu testten önce **hiç değiştirilmedi**. Ölçülen, gerçek genelleme oranıdır.
**Bağımsız doğru cevap:** Cal.com'un depoda yayınladığı resmî OpenAPI dosyası (`docs/api-reference/v2/openapi.json`, 121 herkese açık işlem).

## Sonuç (dokunulmadan)

| Ölçüm | Sonuç |
|---|---|
| Endpoint bulma | **117/121 (%96,7)** |
| Giriş durumu | **%95,7** |
| Bulunan alanlarda zorunluluk | **%100** |
| Girdi alanı bulma | 124/238 (%52,1) |
| Süre (64.663 satır) | 1.435 ms |

## Farkların nedenleri

1. **Girdi alanları (%52):** Cal.com bir monorepo. Girdi tanımlarının çoğu uygulama klasöründe değil, kardeş paketlerde (`@calcom/platform-types` 154, `@calcom/platform-libraries` 163 içe aktarma). Sezgi'ye yalnızca uygulama klasörü verildi; kardeş paketleri görmedi ve 34 şemayı **"çözülemedi"** diye dürüstçe raporladı. Bu adaptör değil, kapsam eksiğidir.
2. **4 eksik endpoint:** `calendars/{}/events/{}` ve `calendars/{}/event/{}` (ayrıntısı incelenecek).
3. **43 fazla endpoint:** `atoms/*` ve `health` gibi resmî dokümanda yayınlanmayan iç işlemler. Kodda var oldukları için hata değil.
4. **Giriş (5 fark):** İsteğe bağlı kimlik doğrulama başlığı olan birkaç işlem.

## Ölçüm notu

Cal.com kimlik doğrulamasını OpenAPI'nin `security` alanıyla değil, `Authorization` başlık parametresiyle anlatıyor. Karşılaştırıcı (`karsilastir-openapi.js`) bu durumu tanıyacak şekilde düzeltildi: Doküman `security` kullanmıyorsa giriş, kimlik başlığının varlığına ve zorunluluğuna göre ölçülür. Düzeltmeden sonra Immich sonuçları değişmedi.

## Üç uygulamanın karşılaştırması

| Uygulama | Adaptörün durumu | Endpoint | Giriş | Girdi alanı |
|---|---|---|---|---|
| Özel uygulama (ilk deneme) | Bu uygulamayla geliştirildi | %100 | %100 | %100 |
| Immich | İlk hâl → genel yetenekler eklendi | %84,9 → %100 | %92,8 → %100 | %0 → %89,9 |
| **Cal.com** | **Dokunulmadı** | **%96,7** | **%95,7** | **%52,1** |

## Genel yetenekler eklendikten sonra

Dokunulmadan ölçülen sonuç yukarıda kayıtlıdır. Ardından Cal.com'un ortaya çıkardığı eksikler için genel yetenekler eklendi:

| Eksik | Eklenen yetenek |
|---|---|
| Girdi tanımları kardeş paketlerde (monorepo) | **Çalışma alanı desteği:** Üst klasörlerde `workspaces` (package.json) ya da `pnpm-workspace.yaml` aranır; uygulamanın bağımlı olduğu kardeş paketler bulunur ve girdi tanımları, şemalar, sabitler onlardan da okunur. Kapılar yalnızca uygulamanın kendi klasöründen alınır. |
| Bir metot birden fazla adrese bağlı | `@Get(["/a", "/b"])` gibi dizi adreslerden her adres için ayrı kapı |
| Aynı adrese sürüme göre farklı controller | **API sürümü:** `@Controller({ version })` ve `@Version(...)` okunur, kapıya `surum` eklenir; yapay zekâ hangi sürümün hangi girdiyi istediğini görür |
| Sabitten sabite atıf | `VERSION_X_VALUE = VERSION_X as unknown as T` → `VERSION_X = "2024-06-14"` zincirinin sonuna kadar izlenmesi; `as`, `satisfies`, `!` ifadelerinin içine bakılması |

| Ölçüm | Dokunulmadan | Genel yetenekler sonrası |
|---|---|---|
| Endpoint bulma | %96,7 | **121/121 (%100)** |
| Giriş durumu | %95,7 | **%99,2** |
| Girdi alanı bulma | %52,1 | **250/250 (%100)** |
| Zorunluluk | %100 | **%100** |
| Süre | 1.435 ms | **160 ms** |

Özel uygulama (%100) ve Immich (%100 endpoint / %100 giriş / %89,9 alan) her adımda yeniden ölçüldü; gerileme yok.

**Kalan tek fark:** `GET /v2/auth/oauth2/clients/{}` için Sezgi "giriş gerekli" diyor, resmî doküman kimlik başlığı göstermiyor. **İncelendi:** Kod bu kapıyı `@UseGuards(ApiAuthGuard)` ile koruyor (`oauth2.controller.ts:47`); resmî doküman bu işlem için kimlik başlığını yazmamış. Sezgi kodu izliyor; fark dokümandaki bir eksiklik, ölçüm olduğu gibi bırakıldı.

**Karşılaştırıcı notu:** Aynı adrese birden fazla sürüm cevap verdiğinde karşılaştırıcı, resmî dokümandaki `cal-api-version` başlığının gösterdiği sürümü seçer.

## Genelleme hakkında dürüst özet

- **Adaptöre dokunulmadan gerçek oran (Cal.com):** Endpoint %96,7, giriş %95,7, alan %52,1. Alan oranındaki düşüklük adaptörden değil, monorepo kapsamından kaynaklandı.
- Her yeni uygulama Sezgi'ye yeni genel yetenekler kazandırdı (Immich 9, Cal.com 4). Yeteneklerin sayısı azalıyor; bu, adaptörün olgunlaştığına işaret ediyor. Ama sonraki uygulama yine **dokunmadan** ölçülmeli.
