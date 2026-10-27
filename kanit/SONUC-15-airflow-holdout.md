# Sonuç 15: FastAPI holdout 2 (Airflow)

Ön kayıt: `HOLDOUT-ON-KAYIT-6-airflow.md` (commit `aa94f7a`, ölçümden önce). İkili `sezgi-release.exe` (SHA-256 `343093181cdef9b8…`); uygulamayı okuyan kod bugünkü `HEAD` ile aynı.

- **Uygulama:** apache/airflow `48a0f8e22be2deb6ee2ee41a4e31763d8d0cca6b`, `airflow-core/` (620 Python dosyası okundu, 142.515 satır).
- **Hakem:** `core_api/openapi/v2-rest-api-generated.yaml`, 134 işlem.
- **Süre:** 1.934 ms.

## Ön kayıtlı sonuç

| Ölçüt | Sonuç |
|---|---|
| Endpoint bulma | **0/134** |
| Belgede olmayan | 170 |
| Giriş, gövde alanı, zorunluluk | ölçülemedi (eşleşen uç yok) |

Ön kayıtlı kuralla adaptör bu uygulamada başarısız oldu.

## Farkların sınıflandırması (ölçümden sonra yapıldı)

Ölçümden sonra uygulamanın kodu okundu. İki Sezgi hatası:

1. **Uygulamaya yardımcı fonksiyonla bağlanan router'lar.** `create_app()` uygulamayı `init_views(app)` gibi bir fonksiyona verir; o fonksiyon router'ı **kendi içinde içe aktarır** (`from ...public import public_router`) ve `app.include_router(public_router)` yapar. Sezgi ne parametre olarak gelen uygulamayı ne de fonksiyon içi içe aktarmayı izliyor. Bu yüzden `public_router = AirflowRouter(prefix="/api/v2")` uygulamaya bağlanamadı ve `/api/v2` öneki kayboldu. (Express'teki `registerX(app)` desteğinin FastAPI karşılığı yok.)
2. **`auto_error=False` yanlış yorumlanıyor ve "kesin" damgalanıyor.** Airflow'un güvenlik şemaları `OAuth2PasswordBearer(..., auto_error=False)` ve `HTTPBearer(auto_error=False)`; belirteç yoksa 401'i şemayı kullanan `get_user` fonksiyonu kendisi fırlatıyor. Sezgi'nin kuralı "zincir `auto_error=False` olan bir şemaya ulaşırsa giriş isteğe bağlıdır" diyor ve bunu **kesin** damgalıyordu. Kural genel olarak yanlış: `auto_error=False` yalnızca şemanın kendisinin hata vermediğini söyler; girişin isteğe bağlı olduğunu söylemez. Bu, kesinlik ilkesinin ihlalidir.

## Tanı ölçümü (ön kayıt dışı)

Önek elle eklenerek (`core_api/routes/public` altındaki 118 uca `/api/v2`) aynı karşılaştırıcıyla:

| Ölçüt | Sonuç |
|---|---|
| Endpoint bulma | 118/134 (%88,1) |
| Gövde alanı bulma | **213/213 (%100)** |
| Zorunluluk | %100 |
| Giriş durumu | **%2,5** |
| "Kesin" damgalı giriş kararı | 89, **hepsi yanlış** (isteğe bağlı dendi, belge giriş gerekli diyor) |

Eksik 16 uç `task_instances` altında; nedeni ölçülmedi.

## Okuma

- Prefect'ten öğrenilen gövde alanı düzeltmeleri görülmemiş bir uygulamada genelleşti (tanı ölçümünde %100; ön kayıttaki %70 eşiğinin üstünde, ama önek hatası yüzünden ön kayıtlı ölçümde değil tanı ölçümünde).
- Router bağlama (önek) iki görülmemiş uygulamada da (Express, Airflow) ilk denemede kırıldı.
- En ağır bulgu: **kesin damgalı yanlış giriş kararı.** Daha önceki ölçümlerde (Immich, Cal.com, Prefect, şablon) kesin damgalı yanlış yoktu. Bu düzeltilene kadar FastAPI'de `auto_error=False` görülen uçların giriş kararına güvenilmemelidir.

## Düzeltmeden sonra (holdout değil)

Üç hata önce testle (`tests/ornekler/fastapi-yardimci`, kesinlik korpusu `f3-auto-error-false-401`), sonra genel olarak düzeltildi. Rust araç zinciri bu makinede olmadığı için derleme ve testler `masa/docker` imajında (Rust 1.99.0, Linux) yapıldı.
1. Uygulamayı ya da router'ı argüman alan yardımcı fonksiyonun içindeki `include_router` / `mount` çağrıları argümana bağlanır; fonksiyon içi içe aktarmalar modülün adlarına eklenir.
2. `auto_error=False` bir şemaya ulaşan zincirde karar artık **hiçbir zaman kesin değildir**: şemayı kullanan fonksiyon ya da çağırdığı bir proje fonksiyonu (iki kat) 401 / 403 fırlatıyorsa "giriş gerekli (tahmin)", fırlatmıyorsa "isteğe bağlı (tahmin)".
3. Yol metin toplamayla kurulabiliyor (`task_instances_prefix + "/{task_id}"`): eksik 16 ucun nedeni buydu.

| Ölçüt | Ön kayıt | Tanı (önek elle) | **Düzeltmeden sonra** |
|---|---|---|---|
| Endpoint bulma | 0/134 | 118/134 | **134/134** |
| Giriş durumu | ölçülemedi | %2,5 | **%99,3** |
| Kesin damgalı yanlış giriş kararı | ölçülemedi | 89 | **0** |
| Gövde alanı bulma | ölçülemedi | 213/213 | **275/275** |
| Zorunluluk | ölçülemedi | %100 | %100 |

Kalan tek giriş farkı `GET /api/v2/auth/logout` (Sezgi isteğe bağlı diyor, belge gerekli).

Gerileme denetimi (aynı ikiliyle): Prefect 179/179, giriş %100, alan 470/470, zorunluluk %99,6; full-stack-fastapi-template 23/23, giriş %100, alan 38/38 (değişmedi). NestJS ve Express kodu değişmedi.
