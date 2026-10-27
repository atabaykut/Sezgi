# Sonuç 14: Hesap ipuçları (Fooocus-API)

Ön kayıt: `ON-KAYIT-5-hesap-ipuclari.md` (commit `4d2a7bd`, ölçümden önce). Ölçülen kod Sezgi `c2dc794`, release. Ölçüm aracı `kanit/hesap-ipuclari-olc.js` (her uç için MCP `tani`, "Hesap ipuçları" satırları).

- **Uygulama:** mrhan1993/Fooocus-API `79718cd9932e8566465ea4c8cd98cde0f1bc3a15`, 22 uç (Sezgi'nin bulduğu).
- **Hakem:** uygulamanın parametre belgesi (`docs/api_doc_en.md`); hesap alanları ve hafif seçenekler ön kayıtta sabitlendi.

## Ön kayıtlı sonuç

| Ölçüt | Sonuç |
|---|---|
| Hesabı belirleyen alanlardan ipucu alan | **0/7** |
| Hafif seçenek | **0/3** |
| Yanlış ipucu | 0 |
| İpucu verilen uç | 0/22 |

Ön kayıtlı kuralla ipuçları bu uygulamada hiç çalışmadı. Sezgi alanları ve değerlerini doğru çıkarmıştı (ör. `performance_selection`: Speed, Quality, Extreme Speed, Lightning, Hyper-SD); ipucu desenleri ise yalnızca adın tamamıyla eşleşiyordu (`mode`, `steps`, `fast`...). `performance_selection`, `image_number` ya da `Extreme Speed` bu desenlere uymadı.

## Düzeltmeden sonra (holdout değil)

Desenler adın ve değerin parçalarına uygulanır (`performance_selection` → performance, selection; `Upscale (Fast 2x)` → upscale, fast, 2x). Sözlük genişletildi (speed, lightning, performance, aspect, upscale, refiner); sayı alanı yalnızca üretilen şeyin sayısıysa hesap sayılır (`image_number`, `num_images`, `batch_size`, `n`). Sayfalama, dosya ve kota boyutu, lens / kamera adları hesap sayılmaz.

| Ölçüt | Ön kayıt | Düzeltmeden sonra |
|---|---|---|
| Hesap alanı | 0/7 | **7/7** |
| Hafif seçenek | 0/3 | **3/3** |
| Ön kayıttaki "hesabı belirlemeyen" girdilere ipucu | 0 | 0 |
| Hakem tablosunda olmayan alanlara ipucu | 0 | 4 alan: `refiner_switch`, `mask_model`, `sam_model`, `enhance_uov_method` |

Hakem tablosunda olmayan dört alanın üçü (`mask_model`, `sam_model`, `enhance_uov_method`) belgeye göre de model ya da büyütme seçimidir; tablo onları içermediği için doğru ya da yanlış sayılmadı. `refiner_switch` iyileştiriciye geçiş noktasıdır; hesabı belirleyip belirlemediği belgede açık değil.

## GPU kullanmayan uygulamalarda gürültü (geliştirme verisi)

Aynı ipuçları GPU'suz uygulamalarda da çıkar, çünkü adlardan okunur:

| Uygulama | İpucu satırı (eski desen) | İpucu satırı (yeni desen) | Not |
|---|---|---|---|
| Immich | 18 | 18 | `model` (kamera modeli süzgeci) ve `size` (sayfa boyutu) eski desende de vardı; yanlış. `size: preview` (küçük resim) makul. |
| Cal.com | 0 | 0 | |
| Prefect | 45 | 45 | 41'i aynı modüldeki gerçek `resume` ucu, tekrar eden satır; `mode` 2, `pull_steps` 2. |

İpuçları "Yorum: addan çıkarım, kesin değil" damgasıyla verilir. GPU'suz uygulamada yanlış ipucu verir; kaliteye kullanıcı karar verir.

## Sınırlar

- Hakemi Sezgi'nin geliştiricisi belgeden çıkardı (bağımsız değil); tablo ölçümden önce sabitlendi.
- Uygulama çalıştırılmadı; ipuçlarının gerçek GPU tasarrufu ölçülmedi.
- Düzeltme bu uygulamadan öğrenildi; yeni bir GPU uygulamasında ön kayıtlı ölçüm yapılmadı.
