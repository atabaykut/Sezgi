# Holdout ön kaydı: Ghostfolio (adaptöre dokunulmadan ölçüm)

Bu belge **Sezgi Ghostfolio üzerinde hiç çalıştırılmadan önce** yazıldı. Sonuç ne çıkarsa çıksın, aşağıdaki kurallarla raporlanacak. Ölçümden sonra adaptör ya da karşılaştırıcı değiştirilirse, düzeltme ayrı yazılacak ve iki sonuç birlikte verilecek.

## Neden Ghostfolio

Tartışma 5'te oylandı: Codex ve Grok Ghostfolio, Claude Novu önerdi.

Ghostfolio'yu seçme gerekçeleri:
- Tek bir NestJS API'si var (`apps/api`).
- Dekoratör ve class-validator kullanıyor.
- Prisma kullanıyor.
- Sezgi'nin geliştirildiği hiçbir uygulamaya benzemiyor ve Sezgi'nin geliştiricisi bu kodu daha önce görmedi.

## Sabitlenenler

| Ne | Değer |
|---|---|
| Uygulama | https://github.com/ghostfolio/ghostfolio, commit `c169311fa307051ad35dd929f5e20f44d9a1b672` |
| Okunan klasörler | `apps/api`, `libs`, `prisma` ve kök `package.json` (sparse checkout; hiçbir şey kurulmadı ya da çalıştırılmadı) |
| Adaptör | `adapters/nestjs.toml` sha256 `10e77e7d…a114d2`. Tartışma 5 Tur 2'den önce dondurulduğu andakiyle aynı (`kanit/acik/DONDURMA.txt`). |
| Çekirdek | Sezgi 0.1.0. Ölçüm anındaki dosya özetleri `kanit/acik/DONDURMA-ONKAYIT.txt` içinde. Dondurmadan sonra yalnızca kart üretimi, MCP metinleri ve yorumlar değişti; çıkarım mantığı değişmedi. |
| Komut | `sezgi init <ghostfolio kökü> --cikti …`. Kök verilir, çünkü `@nestjs/core` bağımlılığı kökteki `package.json` dosyasında. |

## Doğru cevap ve sınırları

Depoda resmî bir OpenAPI dosyası **yok**. Uygulamayı çalıştırıp Swagger üretmek kapsam dışı: üçüncü taraf kod çalıştırılmaz.

Doğru cevap, bağımsız TypeScript hakemi `kanit/hakem/nestjs-hakem.js` (AST, `apps/api` üzerinde). Hakemin bilinen kısıtları, sonuçtan önce şöyle kayda geçiyor:
- **Önek:** Hakem öneki Sezgi'ye benzer bir düzenli ifadeyle `src/main.ts` içinde arıyor. Bu kısım ortak.
- **Giriş:** Hakem yalnızca `JwtAuthGuard` ve `@Public` adlarını tanıyor (ilk deneme uygulamasına göre yazılmıştı). Ghostfolio başka kalıplar kullanıyorsa hakemin giriş kararı yanlış olur. Bu yüzden **giriş karşılaştırmasındaki her uyuşmazlık kod okunarak elle hakemlenecek** ve liste olarak verilecek.
- **Roller:** Hakem yalnızca adı `Roles` ile biten dekoratörleri tanıyor. Ghostfolio'nun izin dekoratörü farklıysa iki taraf da boş döner. Bu durum "ölçülemedi" diye yazılacak, başarı sayılmayacak.
- **Girdi alanları:** Hakem yalnızca göreli içe aktarmaları çözüyor. `@ghostfolio/...` takma adıyla gelen DTO'ları çözemez. Alan karşılaştırması yalnızca hakemin çözdüğü DTO'larda yapılacak; kapsam oranı ayrıca verilecek.

## Raporlanacak sayılar

1. **Endpoint (yöntem + yol), birincil ölçü:**
   - hakeme göre bulma (recall),
   - Sezgi'nin fazla bulduğu kapılar (kesinlik için) ve her birinin elle sınıflandırılması.
2. **Giriş durumu:** uyuşma oranı ve elle hakemlenmiş uyuşmazlık listesi.
3. **Girdi alanı ve zorunluluk:** hakemin çözdüğü DTO'larda.
4. **Sezgi'nin kendi kapsam raporu:** çözülemeyen şemalar, kısmen okunan dosyalar.
5. **Veri bağları:** `kanit/hakem/veri-hakem.js` (tip denetleyicisi) çalışabiliyorsa onunla karşılaştırma; çalışamıyorsa nedeni.
6. **Süre.**

README'ye yazılacak cümle şablonu: "Held-out NestJS app (Ghostfolio, measured once with the frozen adapter, n=1): endpoints X/Y, auth Z% (after manual adjudication of N disagreements), body fields A/B where the referee resolved the DTO."
