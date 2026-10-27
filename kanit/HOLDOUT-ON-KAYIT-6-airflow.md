# FastAPI holdout 2 (Airflow): ön kayıt (19.10.2026, ölçümden önce)

FastAPI adaptörünün ilk holdout'u Prefect'ti (`SONUC-10`); ondan öğrenilen iki düzeltme turundan sonra Prefect artık bağımsız değil. Bu belge, düzeltmelerin koduna bakılmamış ikinci bir uygulamada tutup tutmadığını **ölçümden önce** kayda geçirir. Ölçüm bir kez yapılır; sonuç ne çıkarsa yazılır; ölçüm sırasında Sezgi değiştirilmez.

## Ölçülen kod

- İkili: `sezgi/sezgi-release.exe`, SHA-256 `343093181cdef9b8…`, 16.10.2026 09:29'da `cargo build --release` ile derlendi (çalışma kopyası `4d2a7bd` ile `5c894ae` arasında).
- Uygulamayı okuyan kod (`adaptor.rs`, `fastapi.rs`, `python.rs`, `model.rs`, `express.rs`) `4d2a7bd`'den bugünkü `HEAD`'e kadar değişmedi (`git diff --stat 4d2a7bd HEAD -- sezgi/src` yalnızca `canli.rs`, `mcp.rs`, `oidc.rs`, `main.rs`). Bu yüzden ölçüm bugünkü kodun haritalamasını ölçer.
- Bu makinede şu an Rust araç zinciri yok; yeni derleme yapılmaz.

## Uygulama ve hakem

- **Uygulama:** apache/airflow `48a0f8e22be2deb6ee2ee41a4e31763d8d0cca6b` (2026-10-19), `airflow-core/` (seyrek klon). Airflow 3'ün REST API'si FastAPI ile yazılmıştır. Ölçümden önce yalnızca dosya adları ve hakem belgesi okundu; uygulama kodu okunmadı. Sezgi geliştirilirken Airflow'a hiç bakılmadı.
- **Hakem:** `airflow-core/src/airflow/api_fastapi/core_api/openapi/v2-rest-api-generated.yaml` (çalışan uygulamadan üretilmiş, depoya işlenmiş OpenAPI; 134 işlem, 131'inde `security`, 40'ında gövde). `yaml.safe_load` ile değiştirilmeden JSON'a çevrilir.
- **Önek:** yollar `/api/v2` önekini zaten taşıyor; `--onek` verilmez.
- `sezgi init airflow/airflow-core` ile haritalanır.

## Ölçütler

`kanit/karsilastir-openapi.js` (bugünkü hâliyle, `SONUC-12`'de genişletildiği gibi): endpoint bulma, belgede olmayan, giriş durumu, gövde alanı bulma, zorunluluk. Ek: kesin damgalı giriş kararlarında yanlış sayısı, süre.

## Beklenti (ölçümden önce yazıldı, sonucu bağlamaz)

Prefect'teki ilk holdout'ta endpoint %98,9, alan %26,4 idi. İkinci turdan sonra alan bulmanın görülmemiş bir uygulamada %70'in üstünde çıkması "düzeltmeler genelleşti" diye okunacaktır; altında çıkarsa "Prefect'e özgü kaldı" diye.

## Sonradan

Sonuç `kanit/SONUC-15-airflow-holdout.md`. Farklar ölçümden sonra sınıflandırılır (Sezgi hatası / belge farkı) ve öyle belirtilir.
