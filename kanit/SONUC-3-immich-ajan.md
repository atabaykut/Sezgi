# Ajan düzeyinde görev testi: Immich (14.10.2026)

**Uygulama:** Immich sunucusu (commit `aec0dd1`), başkası tarafından yazılmış açık kaynak uygulama.
**Görevler:** 16 çağrı görevi (`immich/gorevler.json`). Cevap anahtarı Immich'in resmî OpenAPI dosyasından otomatik türetildi: adres, giriş, izin (`x-immich-permission`), yönetici zorunluluğu (`x-immich-admin-only`) ve zorunlu alanlar.
**Model:** claude-sonnet-5-5, iki düzende aynı model ve aynı istem.

## Sonuç

| Düzen | Yapay zekânın elindeki | Tam doğru | Ortalama puan | Token / görev | Süre | Adım | Maliyet |
|---|---|---|---|---|---|---|---|
| A | Immich kod klasörü (Read, Grep, Glob) | 14/16 | %96,3 | 39.521 | 15,8 sn | 6,8 | 0,76 $ |
| B | Immich'in resmî OpenAPI dokümanı, Sezgi'yle aynı araç arayüzüyle | 15/16 | %93,8 | 43.561 | 19,0 sn | 7,6 | 0,56 $ |
| **C** | **Yalnızca Sezgi** | **16/16** | **%100** | **20.467** | **8,5 sn** | **4,2** | **0,37 $** |

C, A'ya göre: Token **%48**, süre **%46**, maliyet **%51** daha az; doğruluk 14/16'dan 16/16'ya çıktı.

A'nın hataları:
- **I04 (paylaşım linki):** `assetIds` alanını zorunlu, `asset.share` iznini gerekli sandı.
- **I15 (kişi adı değiştirme):** `name` alanını zorunlu sandı (resmî dokümana göre bütün alanlar isteğe bağlı).

## Ölçüm sırasında yakalanan üç hata ve düzeltmeleri

1. **Ezber etkisi.** Immich çok bilinen bir proje; model ilk denemede bazı görevleri hiçbir araç kullanmadan, ezberden ve doğru cevapladı. Bu durumda test Sezgi'yi değil modelin hafızasını ölçer. Çözüm olarak iki düzene de aynı talimat eklendi: "Ezberden cevap verme, her parçayı araçlarla bu sürüm üzerinde doğrula." **Sonuç:** Modelin ezberleyemeyeceği özel bir uygulamadaki test (toplu sonuçları README'de; ayrıntıları yayınlanmıyor) bu etkiden arınmış, ama başkaları tarafından tekrar üretilemez. Ezberden arınmış ve tekrar üretilebilir ölçüm için holdout uygulamasına bakın (`SONUC-5`).
2. **Göreli yol hatası.** İlk C denemesinde haritanın yolu göreli verildiği için Sezgi açılamadı ve model yine ezberden cevapladı (ortalama 1 adım). Ölçücü düzeltildi (yollar mutlak), C yeniden çalıştırıldı.
3. **Cevap anahtarı hatası.** I05 için görev hazırlanırken kullanımdan kaldırılmış `POST /people/{id}/merge` seçilmişti. Hem A hem C, onun yerine gelen güncel `POST /people/merge` kapısını buldu. Cevap anahtarı düzeltildi, iki düzen yeniden puanlandı.

## Sezgi için çıkan ders

Kodda bir işlem için hem eski hem yeni kapı olabilir. Sezgi kartlarında bir kapının **kullanımdan kaldırıldığı** ve **yerine hangisinin geldiği** gösterilmeli. Immich bunu `@Endpoint({ history: ...deprecated('v3.2.1', { replacementId }) })` ile yazıyor.

## Dürüstlük notları

- 16 görev ve tek model. Genelleme için daha fazlası gerekir.
- Görevleri ben seçtim; cevap anahtarı bağımsız kaynaktan (resmî OpenAPI) otomatik türetildi.
- **B düzeni:** Immich'in resmî dokümanı zengin olmasına rağmen B, C'den iki kat fazla token ve süre harcadı; tek hatası adresin başındaki `/api` önekini unutmaktı (Immich dokümanı öneki `servers` alanında tutuyor). Adil olmak için not: B sunucusu (`b-mcp.js`) genel bir araçtır; dokümanın `servers` önekini ve Immich'e özel `x-immich-permission` alanını göstermez. Bunlar gösterilseydi B'nin doğruluğu artabilirdi; token farkının ne kadarının kalacağı ayrıca ölçülmeli.
- Sezgi'nin en büyük farkı dokümanı olmayan ya da zayıf uygulamalarda ortaya çıkıyor (özel uygulamada B: 13/28, C: 28/28).
