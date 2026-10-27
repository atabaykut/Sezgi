# FastAPI holdout: ön kayıt (14.10.2026, ölçümden önce)

Bu belge, Python + FastAPI adaptörünün koduna bakılmamış bir uygulamada nasıl ölçüleceğini **ölçümden önce** kayda geçirir. Ölçüm bir kez yapılır. Sonuç ne çıkarsa yazılır. Ölçümden önce ya da ölçüm sırasında adaptör değiştirilmez.

## Ölçülen kod

Sezgi deposu commit `d7dd24f` ("Madde 13 (1/2): Python + FastAPI, dil kalite seviyeleri"). Ölçüm bu commit'ten `cargo build --release` ile derlenen ikiliyle yapılır. Sonradan yapılan değişiklikler bu ölçüme dahil değildir.

## Uygulama seçimi (koda bakmadan)

Seçim kuralı:
- açık kaynak;
- arka ucu FastAPI;
- depoda uygulamanın kendi OpenAPI belgesi (ya da ondan otomatik üretilmiş bir istemci) işlenmiş;
- en az 50 işlem;
- Sezgi geliştirilirken koduna bakılmamış.

Adaylar bu sırayla denenir; kuralı sağlayan ilk aday ölçülür:
1. PrefectHQ/prefect (sunucu API'si)
2. polarsource/polar
3. Netflix/dispatch

Kuralın denetlenmesi için yalnızca dosya adları listelenir (OpenAPI belgesi var mı). Uygulama kodu ölçümden önce okunmaz. Seçilen aday, ölçüm commit'i ve OpenAPI belgesinin yolu, ölçümden önce bu belgenin sonuna eklenir.

## Ölçütler ve hakem

- **Hakem:** uygulamanın depodaki OpenAPI belgesi. Karşılaştırıcı `kanit/karsilastir-openapi.js`'tir, Immich ve Cal.com'daki ile aynı kurallarla:
  - endpoint bulma (belgedeki işlemlerden kaçı bulundu);
  - belgede olmayan fazla işlem;
  - giriş durumu (belgenin `security` alanına göre);
  - gövde alanı bulma;
  - zorunluluk.
- **Önek:** OpenAPI'nin `servers` alanında bir yol öneki varsa `--onek` ile verilir. Bu kural ölçümden önce burada kayıtlıdır.
- **Ek bilgi:** kesin ve çıkarım damgalı giriş kararlarının sayısı, kesin damgalı kararlarda yanlış sayısı. Sezgi'nin süresi.
- **Raporlama:** her ölçüt, olumsuz olanlar dahil yazılır. Bulunan farklar sınıflandırılır (Sezgi hatası, hakem/belge farkı). Sınıflandırma ölçümden sonra yapıldığı için öyle belirtilir.

## Sonradan

Sonuç `kanit/SONUC-10-fastapi-holdout.md` dosyasına yazılır. Ölçümden sonra yapılacak adaptör düzeltmeleri ayrı commit'lerde kalır. Bu uygulama düzeltmeden sonra yeniden holdout sayılmaz.

## Seçilen aday (ölçümden önce eklenir)

- **Aday:** 1. sıradaki PrefectHQ/prefect kuralı sağlıyor.
- **Commit:** `302beb37eb79cd6ba37fcaa9fa002c4429b45702`. Seyrek klon: `src/prefect/`, `pyproject.toml`, `docs/v3/api-ref/rest-api/server/schema.json`.
- **Hakem belgesi:** `docs/v3/api-ref/rest-api/server/schema.json`. Sunucu kodundan üretilmiştir (`scripts/generate_oss_openapi_schema.py`, yalnızca adı okundu). OpenAPI 3.1.0, 179 işlem.
- **Önek ve güvenlik:** belgede `servers` yok, bu yüzden önek verilmez. Belgede `security` alanı yok: karşılaştırıcı girişi kimlik başlığı parametrelerine göre değerlendirir (Immich / Cal.com kuralıyla aynı).
- **Sezgi'nin çalıştırılacağı kök:** depo kökü (`pyproject.toml` FastAPI'yi buradan bildirir).
