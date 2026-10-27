# Sürüm 0.1 ölçüm hedefleri: hepsi bir arada (14.10.2026)

Vizyon §15.3, 0.1 için sekiz ölçüm hedefi koyar. Bu belge her hedef için ölçülen değeri, kaynağını ve sınırını tek tabloda toplar. Uygulamalar açık kanıttaki sabit commit'lerdir (Immich, Cal.com; `acik/getir.sh`). Gerileme ölçümü olarak Ghostfolio da kullanılır; Ghostfolio artık bağımsız ölçüt değildir (`SONUC-6`). Bütün ölçümler tek makinede yapıldı: Windows (MINGW64 x86_64), `cargo build --release`.

| Hedef (0.1) | Ölçülen | Kaynak | Sonuç |
|---|---|---|---|
| İlk öğrenme, ~100 bin satırlık NestJS uygulaması < 15 sn | Cal.com 78.273 satır (kardeş paketler dahil): soğuk 1.899 ms, sıcak 208 ms · Immich 61.567 satır: soğuk 1.215 ms, sıcak 91 ms | `acik/sonuc/OZET-duzeltme-oncesi.md` (soğuk), `acik/sonuc/OZET.md` (sıcak) | Geçti (ölçülen en büyük uygulama 78 bin satır) |
| Sezgi'nin açılışı, harita hazırken < 100 ms | Immich ortanca 10,3 ms (en kötü 13,4) · Cal.com ortanca 9,8 ms (en kötü 13,4) | `olc-hedefler.js` (aşağıda) | Geçti |
| Kart getirme ve yönlendirme p95 < 5 ms | Immich p95 0,92 ms · Cal.com p95 0,68 ms (1.000 çağrı; kart getirme p95 0,19 / 0,17 ms) | `olc-hedefler.js` | Geçti |
| Endpoint bulma oranı (dokümansız uygulamada) ≥ %90 | Ghostfolio 120/120 (hakemle, fazla 0) · Immich 279/279 ve Cal.com 121/121 (resmî OpenAPI ile) | `SONUC-6`, `acik/sonuc/OZET.md` | Geçti |
| Endpoint başına girdi şeması doğruluğu ≥ %85 | Immich gövde alanı 436/485 (%89,9), zorunluluk %100 · Cal.com 250/250, zorunluluk %100 | `acik/sonuc/OZET.md` | Geçti |
| Doğrulama kurallarının Kesin damgayla çıkarılması ≥ %60 | Cal.com 265/265 · Ghostfolio 219/219 (bağımsız TypeScript hakemine göre; Sezgi'nin şemadan okuduğu her kural `certain` damgalıdır) | Aşağıdaki komutlar, `SONUC-6` | Geçti (Immich ölçülemedi, aşağıda) |
| Yanlış bağ < %2 | Veri bağları: 341 bağ, fazla 0 (%0) · Kapılar: Cal.com'da hakemin "fazla" saydığı 4 kapı, hakemin açamadığı dizi rotalarıdır (`@Get(['/a', '/b'])`), yani gerçek yanlış 0 · Ghostfolio fazla 0 | `acik/sonuc/OZET.md`, aşağıda | Geçti |
| Tek dosya değişikliği sonrası güncelleme < 500 ms | Immich 66–83 ms · Cal.com 314–319 ms | `SONUC-7-artimli-guncelleme.md` | Geçti |

## Sınırlar

- **Tek makine.** Süreler başka donanımda farklı olur. Açılış ve gecikme ölçümlerinde depolar ve ikili işletim sisteminin önbelleğindeydi.
- **Gecikme, boru gecikmesi dahil üst sınırdır.** Ölçüm, Node istemcisinden stdio üzerinden gidiş-dönüştür. Sezgi'nin kendi işlem süresi bundan kısadır.
- **"~100 bin satır" burada doğrudan ölçülmedi** (en büyük uygulama 78 bin satırdı). Sonradan ölçüldü: Twenty sunucusu, 981.837 satır, 3.647 ms (`SONUC-13-olcek.md`).
- **Immich'te kural oranı ölçülemedi.** Bağımsız TypeScript hakemi Immich'in girdi şemalarını çözemiyor (0 alan) ve genel `/api` önekini görmüyor. Resmî OpenAPI karşılaştırması alanları ve zorunluluğu ölçer, kuralları ölçmez.
- **Ghostfolio bağımsız değildir.** Bileşik dekoratör desteği Ghostfolio'dan öğrenildi (`SONUC-6`). Yeni bir holdout 0.2 ölçümüne bırakıldı.
- **Hakem karşılaştırmasındaki giriş ve rol sayıları burada kullanılmadı.** TypeScript hakemi genel guard'ı ve bileşik dekoratörü izlemez. Giriş doğruluğu resmî OpenAPI ile (Immich %100, Cal.com %99,2) ve Ghostfolio'da elle hakemleme kuralıyla ölçülür.

## Tekrar üretme

```sh
cd kanit/acik && ./getir.sh && npm ci --ignore-scripts && ./olc.sh      # haritalar: sonuc/immich, sonuc/calcom
node ../olc-hedefler.js ../../sezgi/target/release/sezgi sonuc/immich 1000
node ../olc-hedefler.js ../../sezgi/target/release/sezgi sonuc/calcom 1000
# Doğrulama kuralları ve kapılar (bağımsız TypeScript hakemi):
TS_YOLU=$PWD/node_modules/typescript node ../hakem/nestjs-hakem.js depolar/cal.com/apps/api/v2 hakem-calcom.json
node ../karsilastir.js sonuc/calcom/kapilar.json hakem-calcom.json
```

Bu koşudaki çıktı:

```
immich  Açılış (15 kez): ortanca 10.27 ms · en kötü 13.37 ms
        Kart getirme (tani, 500): p50 0.11 ms · p95 0.19 ms
        Yönlendirme (ara, 500): p50 0.79 ms · p95 0.96 ms
        Kartı dolu gelmeyen tani cevabı: 0
        Hepsi (1000): p95 0.92 ms · p99 1.02 ms
calcom  Açılış (15 kez): ortanca 9.77 ms · en kötü 13.37 ms
        Kart getirme (tani, 500): p50 0.09 ms · p95 0.17 ms
        Yönlendirme (ara, 500): p50 0.60 ms · p95 0.71 ms
        Kartı dolu gelmeyen tani cevabı: 0
        Hepsi (1000): p95 0.68 ms · p99 0.75 ms
calcom  Endpoint bulma 158/160 · Yanlış/fazla kapı 4 (dizi rotaları, yukarıda) · Girdi alanı bulma 146/146 · Zorunluluk %100 · Doğrulama kuralları 265/265
```
