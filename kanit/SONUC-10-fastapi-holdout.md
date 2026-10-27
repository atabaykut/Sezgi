# FastAPI holdout: Prefect (14.10.2026)

Ön kayıt: `HOLDOUT-ON-KAYIT-2-fastapi.md`. Ön kayıt ölçümden önce iki commit'le işlendi (`da27357`, `1390d29`). Ölçüm bir kez yapıldı. Ölçülen kod Sezgi `d7dd24f`'tir; ölçümden önce kaynak farkının sıfır olduğu denetlendi. Ölçümden önce Prefect'in kodu okunmadı, yalnızca hakem belgesi okundu (işlem sayısı ve `servers` / `security` alanları için).

- **Uygulama:** PrefectHQ/prefect `302beb37eb79cd6ba37fcaa9fa002c4429b45702`. Seyrek klon: `src/prefect/`, `pyproject.toml`. 852 Python dosyası, 205.226 satır (sunucu, istemci ve CLI birlikte).
- **Hakem:** `docs/v3/api-ref/rest-api/server/schema.json`. Sunucu kodundan üretilmiş OpenAPI 3.1, 179 işlem. Karşılaştırıcı: `karsilastir-openapi.js`. Önek verilmedi (belgede `servers` yok).

## Sonuç (ön kayıttaki ölçütler)

| Ölçüt | Sonuç |
|---|---|
| Endpoint bulma | **177/179 (%98,9)** |
| Belgede olmayan fazla işlem | 10 |
| Giriş durumu | %100 (belgede güvenlik yok; Sezgi'nin 187 kararının hepsi çıkarım damgalı, kesin damgalı yanlış 0) |
| Gövde alanı bulma | **124/470 (%26,4)** |
| Zorunluluk (bulunan alanlarda) | %94,4 |
| Süre | 146 ms (852 dosya) |

## Farkların sınıflandırılması (ölçümden sonra)

| Fark | Sayı | Sınıf | Neden |
|---|---|---|---|
| Eksik `GET /health`, `GET /version` | 2 | Sezgi sınırı | Bu uçlar `create_api_app()` fonksiyonunun içinde, çalışma anında kaydediliyor. Sezgi yalnızca modül düzeyindeki tanımları okur. |
| Fazla `POST /failure`, `POST /success` | 2 | **Sezgi hatası** | Bu uçlar CLI'ın yerel giriş geri çağırma sunucusuna ait (`cli/_cloud_utils.py`). Asıl API uygulaması bir fonksiyon içinde kurulduğu için Sezgi modül düzeyindeki tek FastAPI örneğini (CLI'ınkini) "ana uygulama" seçti. |
| Fazla `/ui/...` | 8 | Belge farkı | Uçlar API'ye dahil (`server.py`'deki router listesi), ama yayımlanan şemada yoklar. |
| Gövde alanları eksik | 346 | **Sezgi hatası** | (1) Parametre tipi modül yoluyla yazılınca (`flow: schemas.actions.FlowCreate`) Sezgi yalnızca son adı (`FlowCreate`) rota dosyasında aradı ve bulamadı. Bu yüzden gövde hiç tanınmadı. (2) `import prefect.server.schemas as schemas` sonrasında `schemas.actions` gibi bir alt modüle, paket onu kendisi içe aktarmıyorsa ulaşılamadı. |
| "Ana uygulama" | 185 uç ikincil | **Sezgi hatası** | Ana uygulama yanlış seçildiği için API uçlarının hepsi "ikincil" işaretlendi. Benlik Kartı yalnızca ana uygulamanın uçlarını içerdiğinden kart neredeyse boş kaldı (2 işlem). |

## Değerlendirme

- Uç bulma genelleşti (%98,9). Router önekleri, `PrefectRouter` gibi `APIRouter` alt sınıfları ve 852 dosyalık bir depo sorunsuz okundu.
- Gövde alanları genelleşmedi (%26,4). Neden iki çözümleme hatasıdır ve ikisi de genel FastAPI kalıplarıdır: modül yoluyla yazılmış tip ve alt modül erişimi.
- Uygulamanın bir fabrika fonksiyonu içinde kurulması (`create_app()` kalıbı) desteklenmiyor. Bu kalıp yanlış "ana uygulama" seçimine yol açtı.
- Kesinlik ilkesi korundu: güvenlik şeması bulunamadığı için hiçbir giriş kararı kesin damgalanmadı. Prefect'in isteğe bağlı kimlik doğrulaması bir ara katman yazılımıyla kuruluyor; "açık" kararı bu yüzden çıkarım olarak kaldı.
- **Dil kalite seviyesi:** FastAPI ön kayıtlı bir holdout'ta ölçüldüğü için seviye 2'ye geçer. Seviye, sonucun iyi olduğunu değil nasıl ölçüldüğünü söyler; gövde alanı sonucu README'de seviyenin yanında yazılır.

Düzeltmeler bu ölçümden sonra ayrı commit'lerde yapılır. Prefect bundan sonra holdout sayılmaz; düzeltme sonrası ölçüm "gerileme ve ilerleme ölçümü" olarak bu belgeye eklenir.

## Düzeltmelerden sonra (holdout değil)

Holdout'ta bulunan genel FastAPI kalıpları, her biri önce testle olmak üzere düzeltildi (`tests/ornekler/fastapi-fabrika`, `tests/fastapi.rs`):
- modül yoluyla yazılmış tip;
- alt modül erişimi;
- fabrika fonksiyonu içinde kurulan uygulama ve döngüyle dahil edilen router'lar (doğru ana uygulama seçimi);
- birden fazla gövde parametresinin gömülmesi;
- `Field(default=...)` zorunluluğu;
- işaret döndüren yardımcı fonksiyon;
- bağımlılık fonksiyonlarının kendi parametreleri.

Aynı yöntemle yeniden ölçüldü. Bu bir gerileme ve ilerleme ölçümüdür: değişiklikler Prefect'ten öğrenildi.

| Ölçüt | Holdout (`d7dd24f`) | Düzeltmelerden sonra |
|---|---|---|
| Endpoint bulma | 177/179 | 177/179 |
| Ana uygulamaya ait uç | 2 (yanlış ana uygulama) | 185 (`create_api_app().api_app`) |
| Gövde alanı bulma | 124/470 (%26,4) | **437/470 (%93,0)** |
| Zorunluluk | %94,4 | %99,5 |
| Giriş | %100, hepsi çıkarım | %100, hepsi çıkarım |
| Geliştirme uygulaması (gerileme denetimi) | 23/23 · 38/38 · %100 | 23/23 · 38/38 · %100 |

Kalan 33 eksik alanın 31'i (`limit`, `offset`) aynı kalıptan geliyor: iç içe tanımlanmış bir fonksiyon döndüren bağımlılık fabrikası (`def LimitBody(): def get_limit(limit: int = Body(None)): ...; return Depends(get_limit)`). İç içe fonksiyonlar izlenmiyor; bu bilinen bir sınırdır. Fabrika içinde kaydedilen `/health` ve `/version` hâlâ eksik. CLI'ın `/failure` ve `/success` uçları artık doğru olarak ikincil, ama karşılaştırıcı onları belgede olmayan uç olarak saymaya devam ediyor.

## İkinci düzeltme turu (2026-10-16, holdout değil)

Kalan iki kalıp önce testle (`tests/ornekler/fastapi-fabrika`, `tests/ornekler/fastapi-temel`), sonra genel olarak düzeltildi:
- parametre varsayılanı olarak kullanılan ve `Depends(iç fonksiyon)` döndüren fabrika (`limit: int = LimitBody()`); aynı turda `Depends(rol_iste("admin"))` gibi iç fonksiyon döndüren fabrikalar ve `__call__`'lu sınıf örnekleri;
- yolu fabrikanın parametre varsayılanından gelen rota (`def create_api_app(health_check_path: str = "/health")`); iç içe ve koşul altında tanımlanan rotalar.

Aynı commit'te (`302beb37`), aynı karşılaştırıcıyla yeniden ölçüldü:

| Ölçüt | Holdout (`d7dd24f`) | Birinci tur | **İkinci tur** |
|---|---|---|---|
| Endpoint bulma | 177/179 | 177/179 | **179/179** |
| Gövde alanı bulma | 124/470 (%26,4) | 437/470 (%93,0) | **470/470 (%100,0)** |
| Zorunluluk | %94,4 | %99,5 | **468/470 (%99,6)** |
| Giriş | %100, hepsi çıkarım | %100, hepsi çıkarım | %100, hepsi çıkarım |
| Geliştirme uygulaması (gerileme denetimi) | 23/23 · 38/38 · %100 | 23/23 · 38/38 · %100 | 23/23 · 38/38 · %100 |

Kalan iki zorunluluk farkı `POST /block_types` içindeki `logo_url` ve `documentation_url`: Sezgi zorunlu diyor, belge değil diyor. Belgede olmayan 11 uç: CLI'ın ikincil uygulamasındaki `/failure`, `/success` ve `/status`, ayrıca belgeye girmeyen `/ui/...` uçları. Bu sayılar Prefect'ten öğrenildikten sonra ölçüldü; FastAPI için yeni bir holdout bağımsız kanıt olur.

## Tekrar üretme

```sh
git clone --filter=blob:none --no-checkout https://github.com/PrefectHQ/prefect.git prefect
printf '/docs/v3/api-ref/rest-api/server/schema.json\n/pyproject.toml\n/src/prefect/\n' | git -C prefect sparse-checkout set --no-cone --stdin
git -C prefect checkout 302beb37eb79cd6ba37fcaa9fa002c4429b45702
git -C <sezgi> checkout d7dd24f && cargo build --release
sezgi init prefect --cikti sonuc
node karsilastir-openapi.js sonuc/kapilar.json prefect/docs/v3/api-ref/rest-api/server/schema.json
```
