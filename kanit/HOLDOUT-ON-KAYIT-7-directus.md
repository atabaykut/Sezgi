# Express holdout 2 (Directus): ön kayıt (19.10.2026, ölçümden önce)

Kullanılabilirlik turundan (`PROMPT-BAGLAM-KULLANILABILIRLIK.md`, K0–K8) sonra Sezgi'nin koduna bakılmamış ikinci bir Express uygulamasında nasıl ölçüleceğini **ölçümden önce** kayda geçirir. Ölçüm bir kez yapılır; sonuç ne çıkarsa yazılır; ölçüm sırasında Sezgi değiştirilmez.

## Ölçülen kod

Sezgi `sezgi/src` içeriği `270bb9a` ile aynı. Rust araç zinciri bu makinede olmadığı için `masa/docker` imajında (Rust 1.99.0, Linux) `cargo build --release` ile derlenir.

## Uygulama ve hakem

- **Uygulama:** directus/directus `8e140f94250834cea0a0fb6eb822cac39ed9faec` (2026-10-09), `api/` (seyrek klon). Directus'un REST API'si Express ile yazılmıştır. Ölçümden önce yalnızca dosya listesi, `api/package.json` adı ve hakem belgeleri görüldü; uygulama kodu okunmadı. Sezgi geliştirilirken Directus'a hiç bakılmadı.
- **Hakem:** aynı commit'teki `packages/specs` (Directus'un yayınladığı `@directus/specs` OpenAPI belgesi). Dosyalara bölünmüştür; projenin kendi derlemesi `swagger-cli bundle` kullanır. Burada aynı işi yapan `kanit/hakem/openapi-birlestir.py` ile tek JSON'a çevrilir: 70 yol, 136 işlem, belge düzeyinde `security` (Auth, KeyAuth), 9 işlem kendi `security`'sini veriyor, 60 işlemde gövde. Belge elle yazılmıştır, koddan üretilmemiştir: farklar Sezgi hatası da olabilir, belgenin koddan sapması da; ölçümden sonra sınıflandırılır.
- **Önek:** belgede `servers` yok; yollar kökten. `--onek` verilmez.

## Karşılaştırıcıda ölçümden önce yapılan düzeltme

`kanit/karsilastir-openapi.js` işlemin kendi `security`'si yoksa belgenin genel `security`'sini kullanmıyordu (OpenAPI 3 kuralı). Directus belgesi genel `security` kullandığı için bu ölçümden önce düzeltildi. Daha önceki hakem belgelerinin (Immich, Cal.com, Prefect, Airflow, RealWorld) hiçbirinde genel `security` yok; sonuçları değişmez (Airflow ve Prefect'te yeniden denetlendi).

## Ölçütler

`karsilastir-openapi.js`: endpoint bulma, belgede olmayan, giriş durumu, gövde alanı bulma, zorunluluk. Ek: kesin damgalı giriş kararı sayısı ve yanlışı, süre.

## Beklenti (ölçümden önce yazıldı, sonucu bağlamaz)

Express adaptörü seviye 1'dir; ilk Express holdout'u 0/19 ile başarısız olmuştu. Endpoint bulmanın %70'in üstünde ve kesin damgalı yanlış giriş kararının 0 çıkması "Express düzeltmeleri genelleşti" diye okunacaktır.

## Sonradan

Sonuç `kanit/SONUC-16-directus-holdout.md`.
