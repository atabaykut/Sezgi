# Düzeltme notu 1: sayılar, dondurma özetleri ve küçük tutarsızlıklar (14.10.2026)

Bu not, kanıt belgelerindeki tutarsızlıkları kayda geçirir. **Ön kayıt (`HOLDOUT-ON-KAYIT.md`), dondurma dosyaları ve `acik/sonuc/OZET.md` değiştirilmedi**; düzeltmeler burada ve README'lerde.

## 1. Harita süreleri (README'deki 111 ms / 275 ms)

README "kaynak: `kanit/acik/sonuc/OZET.md`" diyerek Immich için 111 ms, Cal.com için 275 ms veriyordu. Bu iki sayı projedeki hiçbir kaynak dosyada geçmiyor. Kaynağı olmayan sayı yazılmadığı için README'lerden kaldırıldı; README artık süreleri yalnızca kaynağına gösteriyor.

Projede kaynağı bulunan süreler:

| Sayı | Uygulama | Ne ölçüyor | Kaynak |
|---|---|---|---|
| 1215 ms | Immich | `sezgi init` toplam süresi; temiz yayın ağacında `olc.sh` ile, `cargo build --release` sonrası ilk çalıştırma (soğuk). Windows (MINGW64 x86_64), 2026-10-13T23:38Z | `acik/sonuc/OZET.md` |
| 1899 ms | Cal.com | Aynı koşu; kardeş paketler dahil | `acik/sonuc/OZET.md` |
| 1.236 ms → 75 ms | Immich | Adaptörün ilk hâli → genel yetenekler eklendikten sonra; soğuk/sıcak olduğu yazılmamış (75 ms tekrar çalıştırma süresine benziyor, doğrulanamadı) | `SONUC-1-immich.md` |
| 1.435 ms → 160 ms | Cal.com | Aynı biçimde, dokunulmadan → genel yetenekler sonrası | `SONUC-2-calcom.md` |
| 1.393 ms / 94 ms | Ghostfolio | İlk (soğuk) çalıştırma / tekrar | `SONUC-5-holdout-ghostfolio.md` |
| ~55 ms, ~150–240 ms | özel uygulama, Cal.com | Veri bağları eklendikten sonra toplam süre (yaklaşık) | `SONUC-4-veri-baglari.md` |

Gözlem: aynı makinede soğuk ilk çalıştırma ~1,2–1,9 sn, tekrar çalıştırmalar 75–160 ms aralığında görünüyor. README'de hangi ölçümün gösterileceği bir karar konusudur (açık karar, `DUZELTME_RAPORU.md` §4).

## 2. Dondurma ve holdout özetleri

Tam SHA-256 özetleri (`DONDURMA.txt` 23:10Z, `DONDURMA-ONKAYIT.txt` 23:23Z):

| Dosya | Dondurma (23:10Z) | Ön kayıt (23:23Z) | Değişti mi |
|---|---|---|---|
| `adapters/nestjs.toml` | `10e77e7d…68405d15` | `10e77e7d…68405d15` | Hayır |
| `src/adaptor.rs` | `7e416cb7…a9a114d2` | `784097b4…7468a297e` | **Evet** |
| `src/benlik.rs` | `929f3ea8…b7555d73` | `1b4406d1…19279b2dc` | Evet |
| `src/main.rs` | `7eb9cbee…43aa5b6c` | `4a8f8994…9400a3f0` | Evet |
| `src/mcp.rs` | `771f9434…8d492b2c` | `b5a01d1b…e6932ac4` | Evet |
| diğerleri (`ayna.rs`, `calisma_alani.rs`, `guvenli.rs`, `imza.rs`, `kural.rs`, `model.rs`, `rapor.rs`, `Cargo.lock`) | | | Hayır |

**Ön kayıttaki kısaltılmış özet hatalı.** `HOLDOUT-ON-KAYIT.md` adaptör için `10e77e7d…a114d2` yazıyor. Bu iki ayrı özetin karışımı: baş kısmı `nestjs.toml`'un özeti (`10e77e7d…68405d15`), son kısmı ilk dondurmadaki `adaptor.rs`'nin özeti (`…a9a114d2`). Doğrusu: `nestjs.toml` sha256 `10e77e7d033566c1e26776ca36b8b5e7963379c6c74b275fe4a680bf68405d15`. Bu dosya ölçüm boyunca değişmedi.

**`adaptor.rs` iki dondurma arasında değişti.** Ön kayıt "dondurmadan sonra yalnızca kart üretimi, MCP metinleri ve yorumlar değişti; çıkarım mantığı değişmedi" diyor. Bu iddia **bu depodan doğrulanamıyor**: ilk dondurmadaki `adaptor.rs` (`7e416cb7…`) projede saklanmamış (o sırada sürüm denetimi yoktu), bu yüzden iki sürüm arasındaki diff çıkarılamadı. Kesin olan:
- Ölçümde kullanılan çekirdek, ön kayıt anındaki çekirdektir (`DONDURMA-ONKAYIT.txt`). Ön kayıt Ghostfolio çalıştırılmadan önce yazıldı. Bu yüzden holdout'un "ölçümden önce kayıt" niteliği korunuyor.
- "Adaptörün" anlamı belirsiz kalmış: kural dosyası (`nestjs.toml`) dondurulmuş; adaptör motoru (`adaptor.rs`) ise ilk dondurmadan sonra değişti ve bu değişikliğin çıkarımı etkileyip etkilemediği belgelenemiyor.
- Ön kayıttan ölçüme kadar yalnızca `mcp.rs` değişti (`SONUC-5`'te yazılı); bu, ön kayıtla bugünkü temel durum (`06af7d7`) karşılaştırılarak doğrulandı.

Öneri ve karar: holdout'un bu sınırla birlikte belgelenmesi mi, yeni bir holdout mu (açık karar, `DUZELTME_RAPORU.md` §4).

## 3. Küçük tutarsızlıklar

| Konu | Değerler | Açıklama |
|---|---|---|
| Cal.com satır sayısı | README 75.950 · SONUC-2 64.663 | 64.663, yalnızca `apps/api/v2` klasörü (569 dosya; SONUC-2). 75.950'nin projede kaynağı yok (kardeş paketler dahil sayım olabilir, doğrulanamadı). README'den kaldırıldı. |
| Cal.com fazla endpoint | SONUC-2 43 · OZET 41 | Farklı ölçümler: 43, adaptöre dokunulmadan yapılan ilk ölçüm; 41, genel yetenekler sonrası güncel ölçüm (OZET). İkisi de kendi ölçümü için doğru. README güncel ölçümü (41) kullanıyor. |
| İmzalı kart boyu | CONFORMANCE "260 KB" · OZET 265.761 bayt | 265.761 bayt ≈ 259,5 KiB ≈ 265,8 kB. CONFORMANCE'ta bayt cinsinden kaynaktaki değer yazıldı. |

## 4. Bu düzeltme turundan sonra

Bu turda Sezgi'nin çıkarım kodu değişti (zorunluluk, türetilmiş DTO'lar, genel guard, herkese açıklık, guard sırası; `DUZELTME_RAPORU.md`). `OZET.md` ve SONUC dosyalarındaki sayılar **düzeltme öncesi** koda aittir (temel durum `06af7d7`). Değişikliklerin çoğu yalnızca damgaları (kesin / çıkarım) etkiler; README'deki sayıların yeni kodla yeniden üretilip üretilmeyeceği ayrıca karar konusudur.

## 5. Yeniden ölçüm (14.10.2026, düzeltme turundan sonra)

Proje sahibinin onayıyla `olc.sh` düzeltilmiş kodla yeniden çalıştırıldı. `acik/sonuc/OZET.md` yeni sonuçları içeriyor; önceki özet değiştirilmeden `acik/sonuc/OZET-duzeltme-oncesi.md` olarak saklandı.

- Bütün kalite sayıları aynı: Immich 279/279, giriş %100, gövde alanı 436/485, zorunluluk %100; Cal.com 121/121, %99,2, 250/250, %100; veri bağları 341/341 (fazla 0, eksik 0). Kartlar şemaya uygun; imza bağımsız JS kütüphaneleriyle doğrulandı (267.512 bayt).
- `olc.sh` artık her uygulamayı iki kez haritalar ve iki süreyi ayrı yazar. Bu koşuda depolar işletim sisteminin önbelleğindeydi (aynı gün önceden okunmuştu), bu yüzden "ilk çalıştırma" soğuk değildir: Immich 91 / 91 ms, Cal.com 204 / 212 ms (son koşu, Y17 düzeltmesinden sonra). Depolar yeni indirildikten hemen sonra ölçülen soğuk ilk çalıştırma önceki özette: 1215 ms ve 1899 ms. README ikisini de koşuluyla birlikte yazar.
