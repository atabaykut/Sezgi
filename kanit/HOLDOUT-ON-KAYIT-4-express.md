# Express holdout: ön kayıt (16.10.2026, ölçümden önce)

Bu belge, Express / Fastify / Next.js adaptörünün (seviye 1) koduna bakılmamış açık bir uygulamada nasıl ölçüleceğini **ölçümden önce** kayda geçirir. Ölçüm bir kez yapılır. Sonuç ne çıkarsa yazılır. Ölçümden önce ya da ölçüm sırasında adaptör değiştirilmez.

## Ölçülen kod

Sezgi deposu commit `4ea49d2`. Ölçüm bu commit'ten `cargo build --release` ile derlenen ikiliyle yapılır.

## Uygulama ve hakem

- **Uygulama:** gothinkster/node-express-realworld-example-app `30b68e1e881462b2f4164ea09ab4c4f5699c7b0b` (2024-01-04). RealWorld ("Conduit") projesinin resmî Express uygulaması: TypeScript, Express 4, express-jwt, Prisma. Ölçümden önce yalnızca dosya listesi ve `package.json` görüldü; uygulama kodu okunmadı.
- **Hakem:** RealWorld projesinin API belgesi `api/openapi.yml`, gothinkster/realworld `a0767b86710599a793fe185e56d4c5756060573d` (2024-01-04, uygulamayla aynı gün; sonraki 2026 sürümleri yeni yanıt kodları ve kısıtlar ekliyor). 19 işlem. Belge uygulamadan ayrı yazılmıştır; uygulama bu belgeye uymayı amaçlar ve resmî Postman koleksiyonuyla sınanır. Bu yüzden hakem uygulamanın kendi ürettiği bir belge değildir: bir fark Sezgi hatası da olabilir, uygulamanın belgeden sapması da. Farklar ölçümden sonra bu iki sınıfa ayrılır ve sınıflandırmanın ölçümden sonra yapıldığı belirtilir.
- **Önek:** belgedeki `servers` adresi `/api` yolunu taşır: `--onek /api`.
- YAML belge, `yaml.safe_load` ile değiştirilmeden JSON'a çevrilir.

## Ölçütler

`kanit/karsilastir-openapi.js`, Immich, Cal.com ve Prefect'teki kurallarla:
- endpoint bulma ve belgede olmayan fazla işlem;
- giriş durumu (belgenin `security` alanına göre);
- gövde alanı bulma ve zorunluluk.

Ek bilgi: giriş kararlarının damgası (bu adaptörde hepsi çıkarım olmalı), Sezgi'nin süresi.

## Bilinen sınırlar (ölçümden önce)

- 19 işlem küçük bir örnektir; sonuç bu adaptörün kalitesi hakkında kaba bir işaret verir, kesin bir oran değil.
- Adaptör girdileri çoğunlukla kullanımdan çıkarır (Yorum); zorunluluk kullanımdan bilinemez, bu yüzden zorunluluk düşük çıkabilir.

## Sonradan

Sonuç `kanit/SONUC-12-express-holdout.md` dosyasına yazılır. Ölçümden sonraki adaptör düzeltmeleri ayrı commit'lerde kalır; bu uygulama düzeltmeden sonra holdout sayılmaz.
