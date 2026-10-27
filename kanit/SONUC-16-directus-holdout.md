# Sonuç 16: Express holdout 2 (Directus)

Ön kayıt: `HOLDOUT-ON-KAYIT-7-directus.md` (commit `d777e4b`, ölçümden önce). Sezgi `sezgi/src` = `270bb9a`, Linux release (Docker).

- **Uygulama:** directus/directus `8e140f94250834cea0a0fb6eb822cac39ed9faec`, `api/` (848 dosya, 85.521 satır okundu).
- **Hakem:** `packages/specs`, `kanit/hakem/openapi-birlestir.py` ile birleştirilmiş: 136 işlem.
- **Süre:** 5.314 ms (Sezgi'nin kendi ölçümü).

## Ön kayıtlı sonuç

| Ölçüt | Sonuç |
|---|---|
| Endpoint bulma | **128/136 (%94,1)** |
| Belgede olmayan | 128 |
| Giriş durumu | **%98,4** |
| Gövde alanı bulma | **0/174 (%0,0)** |
| Kesin damgalı giriş kararı | 0 (Express kararları hep çıkarım) |

Endpoint bulma ve giriş, ön kayıttaki eşiğin (%70) üstünde: bu iki ölçütte Express düzeltmeleri genelleşti. Gövde alanında adaptör başarısız oldu.

## Farkların sınıflandırması (ölçümden sonra yapıldı)

Ölçümden sonra uygulamanın kodu okundu.

1. **Sezgi hatası, genel:** Directus rotaları cevabı yazan ara katmanı işleyiciden sonra verir: `router.post('/x', asyncHandler(fn), respond)`. Sezgi son argümanı (`respond`) işleyici sandı; gerçek işleyicinin okuduğu alanlar hiç görülmedi. Gövde alanının 0 çıkmasının ana nedeni bu.
2. **Sezgi eksiği, genel:** Joi şemaları (`Joi.object({...})` + `sema.validate(req.body)`) okunmuyordu; zod gibi tanımdır.
3. **Sezgi eksiği, genel:** İşleyicinin isteği verdiği yardımcı fonksiyonun okuduğu alanlar (`getCurrentMode(req)` içinde `req.body.mode`) görülmüyordu.
4. **Sezgi eksiği:** İsteğe bağlı yol parametresi (`/:pk/:filename?`) yalnızca uzun biçimle eşleşiyordu (`GET /assets/{id}` eksikti).
5. **Koddan bilinemez:** Gövdelerin çoğu (`presets`, `roles`, `users`...) olduğu gibi genel bir servise verilir (`service.createOne(req.body)`); alanlar Directus'un veritabanındaki sistem tablolarında tanımlıdır, kodda değil. Sezgi bunları kullanımdan çıkaramaz ve uydurmaz.
6. **Dinamik rota:** `POST /auth/login` kimlik sağlayıcıları üzerinden döngüyle (`/login/${provider.name}`) kaydedilir; yol çalışma zamanında belli olur.
7. **Belge ile kod farkı:** `PATCH/DELETE /relations/{id}` belgede tek, kodda iki parametrelidir (`/:collection/:field`).

## Düzeltmeden sonra (holdout değil)

1–4 önce testle (`tests/ornekler/express-temel/routes/paylasim.js`), sonra genel olarak düzeltildi. Ayrıca kullanımdan çıkan alan, yalnızca denetleniyorsa (`if (req.body.x)`, `typeof`) ya da yedeği varsa (`||`, `??`, `?.`) isteğe bağlı sayılır (yine Yorum).

| Ölçüt | Ön kayıt | **Düzeltmeden sonra** |
|---|---|---|
| Endpoint bulma | 128/136 | **129/136** |
| Giriş durumu | %98,4 | %98,4 |
| Gövde alanı bulma | 0/174 | **49/174 (%28,2)** |
| Zorunluluk | ölçülemedi | %53,1 |

Kalan alanların çoğu 5. maddedeki veritabanı tanımlı gövdelerdir.

Gerileme denetimi: RealWorld 19/19, giriş %100, alan 6/6, zorunluluk %100 (değişmedi). dört özel Express / Fastify / Next.js uygulamasında uç ve giriş sayıları değişmedi (ayrıntıları yayınlanmaz); iki uygulama bu makinede artık bulunmadığı için ölçülemedi.
