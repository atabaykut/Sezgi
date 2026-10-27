# Sonuç 12: Express holdout (RealWorld)

Ön kayıt: `HOLDOUT-ON-KAYIT-4-express.md` (commit `e0242bf`, ölçümden önce). Ölçülen kod Sezgi `4ea49d2`, `cargo build --release`.

- **Uygulama:** gothinkster/node-express-realworld-example-app `30b68e1e881462b2f4164ea09ab4c4f5699c7b0b`. TypeScript, Express 4, express-jwt, Prisma.
- **Hakem:** gothinkster/realworld `a0767b86710599a793fe185e56d4c5756060573d`, `api/openapi.yml` (19 işlem), `yaml.safe_load` ile JSON'a çevrildi. Önek `/api` (belgenin `servers` adresinden).
- **Süre:** 351 ms.

## Ön kayıtlı sonuç

| Ölçüt | Sonuç |
|---|---|
| Endpoint bulma | **0/19** |
| Belgede olmayan | 20 |
| Giriş, gövde alanı, zorunluluk | ölçülemedi (eşleşen uç yok) |
| Kesin damgalı giriş kararı | 0/20 (hepsi çıkarım) |

Ön kayıtlı kuralla adaptör bu uygulamada başarısız oldu.

## Farkların sınıflandırması (ölçümden sonra yapıldı)

Ölçümden sonra uygulamanın kodu okundu. Farkların hepsi Sezgi hatası çıktı:

1. **Noktalı dosya adı:** `import tagsController from './tag/tag.controller'` çözülemedi. Uzantı eklenmesi gerekirken `.controller` kısmı `.ts` ile değiştiriliyordu (`tag.ts`). Bu yüzden dört controller'ın router'ı uygulamaya bağlanamadı ve "bağlanmamış router" sayıldı. Hata geneldi: `x.controller.ts`, `x.routes.js` gibi adların hepsi etkileniyordu.
2. **Zincirle kurulan router:** `const api = Router().use(a).use(b)` ve `export default Router().use('/api', api)`. Zincirdeki bağlamalar izlenmiyordu; `/api` öneki kayboldu.
3. **İsteğe bağlı kimlik:** `auth.optional` (express-jwt, `credentialsRequired: false`) adındaki "auth" yüzünden "giriş gerekli" sayıldı. Kimliksiz istek geçtiği için doğru karar "isteğe bağlı"dır.

Önek verilmeden (ön kayıt dışı, yalnızca tanı için) aynı ikiliyle: endpoint 19/19, giriş %73,7 (5 fark, hepsi 3. maddedeki isteğe bağlı kimlik).

## Düzeltmeden sonra (holdout değil)

Üç hata önce testle (`tests/ornekler/express-temel`), sonra genel olarak düzeltildi. Karşılaştırıcıya iki ekleme yapıldı (aşağıda). Aynı belgeyle yeniden ölçüldü:

| Ölçüt | Holdout (`4ea49d2`) | Düzeltmeden sonra |
|---|---|---|
| Endpoint bulma | 0/19 | **19/19** |
| Giriş | ölçülemedi | **%100** |
| Gövde alanı (üst düzey) | ölçülemedi | **6/6** |
| Zorunluluk | ölçülemedi | %100 |
| Belgede olmayan | 20 | 1 (`GET /`, uygulamanın durum ucu) |

Gövde alanları belgede sarmalayıcıdır (`{"user": {...}}`, `{"article": {...}}`); karşılaştırıcı üst düzey alanları sayar. İç alanlar kullanımdan okunamıyor (`createUser({...req.body.user})`), bu ölçüm onları sınamıyor.

## Karşılaştırıcıdaki değişiklikler (ölçümden sonra, bütün uygulamalara uygulandı)

- **İsteğe bağlı kimlik:** belgede `security` alanını kullanan uygulamalarda Sezgi'nin "isteğe bağlı" kararı "gerekli" sayılıyordu. Artık kimliksiz isteğin geçtiğini söyleyen iki durum ("isteğe bağlı" ve belgede boş ya da `{}` içeren `security`) "herkese açık" sayılır.
- **`components/requestBodies`:** gövde bu yolla bağlıysa ve şema satır içi yazılmışsa okunmuyordu.

Bu değişikliklerden sonra Immich (279/279, giriş %100, alan %89,9, zorunluluk %100), Cal.com (121/121, giriş %99,2, alan 250/250) ve Prefect (179/179, giriş %100, alan 470/470, zorunluluk %99,6) değişmedi.

## Özel uygulamalar (gerileme denetimi)

Aynı düzeltmelerden sonra altı özel Express / Fastify / Next.js uygulamasındaki uç ve giriş sayıları değişmedi. Ayrıntı yalnızca özel arşivde.

## Tekrar üretme

```sh
git clone https://github.com/gothinkster/node-express-realworld-example-app rw && git -C rw checkout 30b68e1e881462b2f4164ea09ab4c4f5699c7b0b
git clone https://github.com/gothinkster/realworld && git -C realworld show a0767b86710599a793fe185e56d4c5756060573d:api/openapi.yml > rw.yml
python -c "import yaml,json; json.dump(yaml.safe_load(open('rw.yml')), open('rw.json','w'))"
sezgi init rw --cikti sonuc
node karsilastir-openapi.js sonuc/kapilar.json rw.json --onek /api
```
