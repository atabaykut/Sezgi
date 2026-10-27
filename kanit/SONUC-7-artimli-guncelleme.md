# Artımlı güncelleme ölçümü: `sezgi calistir` (14.10.2026)

Vizyon §15.3'teki 0.1 hedefi: **tek dosya değişikliği sonrası güncelleme < 500 ms.**

## Yöntem

- Uygulamalar: açık kanıttaki sabit commit'ler (`acik/getir.sh`): Immich `server`, Cal.com `apps/api/v2` (kardeş paketler dahil).
- Sezgi `cargo build --release`, Windows (MINGW64 x86_64), aynı makine.
- `sezgi calistir <uygulama> --cikti <geçici klasör> --aralik 100 --en-fazla 1` başlatıldı. İlk kurulumdan sonra bir controller dosyasının sonuna bir yorum satırı eklendi. Sezgi'nin yazdığı güncelleme satırındaki süre okundu. Ardından dosya `git checkout` ile geri alındı. Bu işlem her uygulama için 3 kez yapıldı.
- Ölçülen süre, değişiklik fark edildikten sonraki bölümdür: keşif ve parmak izi karşılaştırması, değişen dosyanın ayrıştırılması, adaptörün bütün model üzerinde yeniden çalışması, `kapilar.json` ve `benlik.json` dosyalarının atomik yazımı. Değişikliğin fark edilmesi buna ek olarak en fazla `--aralik` kadar sürer (varsayılan 500 ms; sessiz dönemde 4 katına kadar seyrekleşir).

## Sonuç

| Uygulama | Dosya | Yeniden ayrıştırılan | Güncelleme süresi (3 koşu) |
|---|---|---|---|
| Immich | 457 | 1 | 78 · 66 · 83 ms |
| Cal.com | 741 | 1 | 319 · 314 · 315 ms |

Bütün koşular hedefin (500 ms) altında kaldı. Cal.com'daki sürenin çoğu keşiften geliyor: uygulama ve kardeş paketlerin klasör ağacı her güncellemede yeniden gezilir (ilk kurulumda keşif ~110 ms).

## Sınırlar

- Tek makine, tek koşu dizisi; depolar işletim sisteminin önbelleğindeydi.
- Adaptör her güncellemede bütün model üzerinde çalışır. Yalnızca ayrıştırma artımlıdır, çünkü yetki, modül grafiği ve tablo bağları dosyalar arasıdır.
- Değişiklik git'ten değil, dosya parmak izinden (değişim zamanı ve boyut) öğrenilir.

## Tekrar üretme

```sh
cargo build --release
sezgi calistir kanit/acik/depolar/immich/server --cikti /tmp/harita --aralik 100 --en-fazla 1 &
sleep 4; echo "// deneme" >> kanit/acik/depolar/immich/server/src/controllers/activity.controller.ts; wait
git -C kanit/acik/depolar/immich checkout -- .
```
