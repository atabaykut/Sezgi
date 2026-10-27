# Sonuç 13: Ölçeklenebilirlik (0.1 hedefi "~100 bin satır < 15 sn")

`SONUC-8`'de bu hedef doğrudan ölçülmemişti (en büyük uygulama 78 bin satırdı). Burada büyük bir açık NestJS uygulamasında ölçüldü.

- **Uygulama:** twentyhq/twenty `ca92ea8bfe907d4919c83d585503d619e459acea` (2026-10-15), `packages/twenty-server` (seyrek klon; `packages/twenty-shared` ve kök yapılandırma dosyaları da alındı).
- **Sezgi:** `cb0bb10`, `cargo build --release`.
- **Makine:** Intel Core i7-13620H (10 çekirdek, 16 iş parçacığı), 32 GB RAM, Windows 11. Ölçüm sırasında arka planda bir fuzz çalışması (tek iş parçacığı, tam yük) sürüyordu: süreler bu yük altındadır.

## Sonuç

| Ölçüt | Değer |
|---|---|
| Okunan satır (test dosyaları hariç) | **981.837** |
| Okunan dosya | 11.555 |
| Bulunan REST ucu | 165 (Twenty'nin API'sinin çoğu GraphQL'dir; GraphQL Sezgi'nin kapsamında değil) |
| `sezgi init` duvar saati, 1. çalıştırma | **3.647 ms** |
| 2. ve 3. çalıştırma | 1.626 ms, 1.962 ms |
| Sezgi'nin kendi ölçtüğü süre (3. çalıştırma) | keşif 602 ms, ayrıştırma 1.123 ms, adaptör 67 ms, toplam 1.793 ms |
| Tepe bellek (çalışma kümesi, 20 ms örnekleme) | **273 MB** |
| Kısmen okunan dosya | 1 |

Hedef 100 bin satır için 15 saniyeydi. Bunun yaklaşık on katı büyüklükte bir uygulama 4 saniyenin altında haritalandı.

## Sınırlar

- Tek makine, tek uygulama; arka plan yükü altında.
- Doğruluk burada ölçülmedi: Twenty'nin depoda REST için resmî bir OpenAPI belgesi yok. Bu yalnızca süre ve bellek ölçümüdür.
- 1. çalıştırmada depo dosyaları işletim sisteminin önbelleğinde olabilir (klon yeni yapılmıştı); tamamen soğuk disk ölçülmedi.

## Tekrar üretme

```sh
git clone --depth 1 --filter=blob:none --no-checkout https://github.com/twentyhq/twenty.git twenty
printf '/packages/twenty-server/\n/packages/twenty-shared/\n/package.json\n/nx.json\n/tsconfig.base.json\n' | git -C twenty sparse-checkout set --no-cone --stdin
git -C twenty checkout ca92ea8bfe907d4919c83d585503d619e459acea
sezgi init twenty/packages/twenty-server --cikti sonuc
```
