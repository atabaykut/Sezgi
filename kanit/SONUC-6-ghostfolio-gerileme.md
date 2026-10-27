# Ghostfolio gerileme ölçümü (düzeltme turundan sonra, 14.10.2026)

**Bu bir holdout değildir.** Ghostfolio, 0.1.0 adayının holdout'u olarak bir kez ölçüldü (`SONUC-5-holdout-ghostfolio.md`). Düzeltme turu çıkarım kodunu değiştirdiği ve bazı değişiklikler holdout'tan öğrenilen derslere dayandığı için (OAuth strateji guard'ları), Ghostfolio artık bağımsız bir ölçüt değildir. Burada yalnızca **gerileme testi** olarak, aynı yöntemle yeniden ölçüldü: yeni kod eskisinden kötü mü?

## Yöntem

- Aynı uygulama ve commit: Ghostfolio `c169311fa307051ad35dd929f5e20f44d9a1b672` (`apps/api`, `libs`, `prisma`; seyrek klon, yalnızca okundu).
- Aynı araçlar: bağımsız TypeScript hakemi (`hakem/nestjs-hakem.js`), endpoint karşılaştırıcı (`karsilastir.js`) ve holdout'ta kullanılan elle hakemleme kuralı (`holdout-hakemle.js`, kural holdout ölçümünden sonra yazılmıştı ve değiştirilmedi).
- İki ikili: düzeltme öncesi temel durum (`06af7d7`) ve düzeltme turundan sonraki kod.
- Veri bağları yeniden ölçülmedi: tablo bağı izleme kodu bu turda değişmedi.

## Sonuç

| Ölçüm | Düzeltme öncesi | Düzeltme sonrası |
|---|---|---|
| Endpoint (hakemle) | 120/120, fazla 0 | 120/120, fazla 0 |
| Giriş, elle hakemleme kuralıyla | 88/120 (%73,3) | **92/120 (%76,7)** |
| "Kesin" damgalı giriş kararları | 74 kararın 70'i doğru, **4 yanlış** | 70 kararın 70'i doğru, **0 yanlış** |
| "Çıkarım" damgalı giriş kararları | 46 kararın 18'i doğru | 50 kararın 22'si doğru |
| Kart işlem damgaları | certain 61 · inferred 44 · unknown 15 | certain 57 · inferred 48 · unknown 15 |

- Önceki 4 kesin yanlışın hepsi OAuth giriş uçlarıydı (`AuthGuard('google')`, `AuthGuard('oidc')`, `OAuthCallbackGuard('google' | 'oidc')`). Bu uçlar artık "herkese açık" ve çıkarım damgalı.
- Kalan 28 yanlış, bileşik dekoratörle verilen guard'lardır (`applyDecorators(UseGuards(...))`). Bilinen sınır; hepsi çıkarım damgalı, yazma kilitli ve önizleme "GEÇER" demiyor.

## Bu ölçümün bulduğu gerileme

Düzeltme turunun ara bir hâli Ghostfolio'da giriş doğruluğunu **38/120'ye düşürdü.** Sebebi şuydu: Herkese açıklık işaretini tanımından tanıyan yeni kural, `SetMetadata(ANAHTAR, true)` yazan her dekoratörü aday sayıyordu. Ghostfolio'nun `@AllowDuringImpersonation()` işareti bu yüzden kütüphaneden gelen `AuthGuard('jwt')` ile birlikte kapıları "herkese açık" yaptı.

Düzeltildi: Bir işaret ancak adı bir herkese açıklık işaretiyse ya da anahtarı bir giriş guard'ı tarafından okunuyorsa karara katılır. Sonuç yukarıdaki son sütundur. Bu kalıp artık negatif korpusta (`tests/kesinlik/y17-ilgisiz-meta-isaret`).

## Bileşik dekoratörlerden sonra (0.2 madde 4, 14.10.2026)

Bileşik dekoratör desteğinden sonra (`applyDecorators(UseGuards(...))` tanımdan izlenir; korpus vakası `tests/kesinlik/b1-bilesik-dekorator`) aynı yöntemle yeniden ölçüldü. Bu da holdout değil, gerileme ve ilerleme ölçümüdür: değişiklik Ghostfolio'nun `@RequiresScope` kalıbından öğrenildi.

| Ölçüm | Düzeltme turundan sonra | Bileşik dekoratörlerden sonra |
|---|---|---|
| Endpoint (hakemle) | 120/120, fazla 0 | 120/120, fazla 0 |
| Giriş, elle hakemleme kuralıyla | 92/120 (%76,7) | **120/120 (%100)** |
| "Kesin" damgalı giriş kararları | 70 kararın 70'i doğru | 98 kararın 98'i doğru |
| "Çıkarım" damgalı giriş kararları | 50 kararın 22'si doğru | 22 kararın 22'si doğru |
| Kart işlem damgaları | certain 57 · inferred 48 · unknown 15 | certain 85 · inferred 20 · unknown 15 |

- Kalan 22 çıkarım damgalı karar, tanınmayan guard'ı olan uçlardır (ör. `@RequiresScopeOfAccess`: `AccessGuard`, `ScopeGuard`). Karar doğru ama kesin denemez.
- Immich ve Cal.com aynı kodla yeniden ölçüldü (`acik/olc.sh`): bütün kalite sayıları aynı (279/279, %100; 121/121, %99,2; veri bağları 341/341). `acik/sonuc/OZET.md` bu ölçümle değiştirilmedi, yalnızca süreler farklıydı.

## Tekrar üretme

`SONUC-5-holdout-ghostfolio.md` "Tekrar üretme" bölümündeki komutlar. Düzeltme öncesi ikili için depo `06af7d7` commit'inde derlenir.
