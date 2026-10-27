# Python + FastAPI: geliştirme ölçümü (14.10.2026)

**Bu bir holdout değildir.** FastAPI adaptörü (`src/python.rs`, `src/fastapi.rs`) bu uygulamanın koduna bakılarak yazıldı. Sonuç yalnızca adaptörün bu uygulamada çalıştığını gösterir; genelleme iddiası taşımaz. Bağımsız ölçüm için ayrı bir holdout gerekir (dil kalite seviyesi 1 → 2).

## Uygulama

- [fastapi/full-stack-fastapi-template](https://github.com/fastapi/full-stack-fastapi-template) `7257e606cb94eada1dca40173e2b7f99dd549bc6`, `backend/` (FastAPI'nin yazarının resmî şablonu; SQLModel, OAuth2 ile giriş, router önekleri, `Annotated` bağımlılık takma adları, ortama bağlı router).
- `backend/app` 1.513 satır Python. Kod yalnızca okundu, çalıştırılmadı.

## Hakem

FastAPI, OpenAPI belgesini kodundan kendisi üretir. Şablonun ön yüzündeki istemci bu belgeden otomatik üretilmiş ve depoya işlenmiştir: `frontend/src/client/sdk.gen.ts` her işlemin yöntemini, adresini ve güvenliğini, `types.gen.ts` gövde ve sorgu alanlarını ve zorunluluklarını taşır. `kanit/hakem/fastapi-sdk-hakem.js` bu iki dosyayı TypeScript derleyicisiyle okur. Hakem uygulamanın kendi beyanıdır ve Sezgi'nin kodundan bağımsızdır. Doğrulama kuralları türlerde bulunmadığı için karşılaştırılamadı.

## Sonuç

| Ölçüm | Sonuç |
|---|---|
| Endpoint bulma | 23/23 (%100), fazla 0 |
| Giriş durumu | %100 (17 kesin, 6 çıkarım: gerçekten açık uçlar) |
| Gövde ve sorgu alanı bulma | 38/38 (%100) |
| Zorunluluk | %100 |

Geliştirme sırasında bulunup düzeltilen hata: `Annotated` takma adının içindeki bağımlılık, takma adın tanımlandığı modül yerine kullanıldığı modülde aranıyordu. Bu yüzden zincir kopuyor ve giriş isteyen 11 uç "herkese açık" çıkıyordu. Bu hata artık negatif korpusta: `tests/kesinlik/f3-takma-ad-baska-modulde`.

## Kesinlik

Anlam adlardan değil tanımlardan çıkarılır; ilke NestJS adaptörüyle aynıdır. Bağımlılık zinciri (`CurrentUser` → `get_current_user` → `Depends(reusable_oauth2)` → `OAuth2PasswordBearer`) izlenip bir güvenlik şemasına ulaşırsa giriş kararı kesindir. Hiç güvenlik şeması bulunamazsa uç açık sayılır ama çıkarım damgası alır, çünkü bir ara katman yazılımı kimlik isteyebilir. İzlenemeyen bir bağımlılık varsa uç "tanınmayan guard" olarak bildirilir.

## Tekrar üretme

```sh
git clone https://github.com/fastapi/full-stack-fastapi-template && git -C full-stack-fastapi-template checkout 7257e606cb94eada1dca40173e2b7f99dd549bc6
sezgi init full-stack-fastapi-template/backend --cikti sonuc
TS_YOLU=<kanit/acik/node_modules/typescript> node hakem/fastapi-sdk-hakem.js full-stack-fastapi-template/frontend/src/client hakem.json
node karsilastir.js sonuc/kapilar.json hakem.json
```
