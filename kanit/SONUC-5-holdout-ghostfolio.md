# Holdout ölçümü: Ghostfolio, adaptöre dokunulmadan (14.10.2026)

Kurallar ölçümden önce [`HOLDOUT-ON-KAYIT.md`](HOLDOUT-ON-KAYIT.md) dosyasına yazıldı. Ölçüm bir kez yapıldı; adaptör ve karşılaştırıcılar ölçümden sonra değiştirilmedi.

**Uygulama:** [Ghostfolio](https://github.com/ghostfolio/ghostfolio), commit `c169311`, `apps/api`. NestJS, Prisma, Nx monoreposu. Sezgi'nin geliştiricisi bu kodu ölçümden önce görmedi.
**Doğru cevap:**
- Depoda resmî OpenAPI yok. Bu yüzden doğru cevap bağımsız TypeScript hakemi (`hakem/nestjs-hakem.js`) ile tip denetleyicili veri hakemidir (`hakem/veri-hakem.js`).
- Hakemin giriş kararları güvenilir değildi; ön kayıtta söylendiği gibi kod okunarak hakemlendi (`holdout-hakemle.js`).

## Sonuç

| Ölçü | Sonuç | Not |
|---|---|---|
| Endpoint (yöntem + yol) | **120/120**, fazla 0 | Hakemle birebir |
| Giriş durumu | **88/120 (%73,3)** | Bağımsız hakem: 22/120. Ayrıntı aşağıda |
| Gövde şeması çözme | 26/27 (134 alan) | Hakem yalnızca 3/27 şemayı çözebildi (`@ghostfolio/...` takma adlı içe aktarmalar). Bu 3 şemadaki 7 alan birebir aynı; geri kalanı bağımsız doğrulanmadı |
| Veri bağları (hakem derinlik 12) | Kesinlik **%100** (314 doğru, 0 fazla), duyarlılık **%80,3** (77 eksik) | 17 endpoint'te Sezgi kendisi "eksik olabilir" dedi |
| Veri bağları (hakem derinlik 6, Sezgi ile aynı) | Kesinlik %99,4, duyarlılık %88,9 (39 eksik) | Eksiklerin yaklaşık yarısı Sezgi'nin 6 adım sınırından |
| Süre | İlk (soğuk) çalıştırma 1.393 ms; tekrar 94 ms | Windows 11 |

## Giriş hatalarının dökümü

**28 endpoint'te Sezgi "açık" dedi, gerçekte giriş gerekli.** Ghostfolio yetkiyi bileşik dekoratörle veriyor:

```ts
export function RequiresScope(...scopes) {
  return applyDecorators(SetMetadata(...), UseGuards(AuthGuard('jwt'), HasPermissionGuard, ImpersonationGuard, ScopeGuard));
}
```

Sezgi `applyDecorators` içine bakmıyor; bağımsız hakem de bakmıyor. **Ancak bu 28 kararın hepsi kartta ve MCP'de çıkarım olarak işaretli** (`giris_kaniti: varsayim_bilinmeyen_guard`). Bunun nedeni Ghostfolio'nun tanınmayan genel guard'ı (`ImpersonationWriteGuard`). Sonuç olarak:
- işlem damgası `inferred` olduğu için SPEC §3 gereği yazma kilitli,
- `onizle` bu kapılarda "GEÇER" demiyor,
- `tani` ve `ara` satırında `[Yorum: tanınmayan guard var]` görünüyor.

Yani hata gerçek, ama Sezgi onu "kesin" diye sunmadı. Tartışma 4'te eklenen damga kuralı tam bu durum için yazılmıştı.

**4 endpoint'te Sezgi "giriş gerekli" dedi, gerçekte açık.** `GET /api/auth/google`, `/api/auth/google/callback`, `/api/auth/oidc` ve `/api/auth/oidc/callback` girişi başlatan uçlar. Passport'un `AuthGuard('google')` guard'ı kullanıcıyı sağlayıcıya yönlendiriyor; çağıranın oturumu gerekmiyor. Sezgi adında "auth" geçen her guard'ı kimlik guard'ı saydığı için bu 4 karar **yanlış ve "kesin" damgalı**. Bu, holdout'un bulduğu gerçek bir doğruluk hatası.

Sezgi'nin "kesin" damgalı giriş kararlarında doğruluk: 74 kararın 70'i doğru (%94,6). Çıkarım damgalı kararlarda: 46 kararın 18'i doğru.

## Ne öğrendik (0.2 işleri; holdout sonrası ve ayrı ölçülecek)

1. **Bileşik dekoratörler:** `applyDecorators(UseGuards(...))` ile kurulan dekoratörler açılmalı. NestJS'te yaygın bir kalıp.
2. **Passport stratejisi adı:** `AuthGuard('<strateji>')` için strateji adına bakılmalı. `jwt`, `api-key`, `bearer` kimlik ister; `google`, `oidc`, `github`, `local` girişi başlatır.
3. **İzleme derinliği:** 6 adım sınırı bu uygulamada eksiklerin yaklaşık yarısını üretti. Sınır yükseltilmeli ya da sınıra takılan her zincir şu anki gibi "eksik olabilir" diye işaretlenmeye devam etmeli.
4. **Hakem:** NestJS hakemi tek bir uygulamanın kalıplarıyla yazılmıştı; burada giriş kararlarında Sezgi'den kötü çıktı. Hakem de genel kalıpları öğrenmeli ve tsconfig yol takma adlarını çözmeli. Aksi hâlde alan doğruluğu bağımsız ölçülemez.

## Tekrar üretme

Ghostfolio kodu bu depoya alınmaz (ayrı lisans; holdout'un holdout kalması için). Elle:

```sh
git clone --filter=blob:none --sparse https://github.com/ghostfolio/ghostfolio.git ghostfolio
git -C ghostfolio sparse-checkout set apps/api libs prisma
git -C ghostfolio checkout c169311fa307051ad35dd929f5e20f44d9a1b672
sezgi init ghostfolio --out sonuc
TS_YOLU=<kanit/acik/node_modules/typescript> node hakem/nestjs-hakem.js ghostfolio/apps/api hakem.json
node karsilastir.js sonuc/kapilar.json hakem.json
node holdout-hakemle.js ghostfolio/apps/api hakem.json sonuc/kapilar.json
TS_YOLU=... VERI_SEMA=ghostfolio/prisma/schema.prisma VERI_TSCONFIG=ghostfolio/apps/api/tsconfig.app.json \
  node hakem/veri-hakem.js ghostfolio/apps/api veri.json && node karsilastir-veri.js veri.json sonuc/kapilar.json
```

## Ölçümden sonra değişenler

Ölçümden sonra yalnızca `src/mcp.rs` değişti: tanınmayan guard uyarısının metni (guard uygulama genelindeyse boş parantez yerine açıklama). Adaptör dosyası, `Cargo.lock` ve çıkarım yapan bütün dosyalar (`model.rs`, `adaptor.rs`, `calisma_alani.rs`, `kural.rs`, `guvenli.rs`) ölçülen hâliyle aynı (özetler: `kanit/acik/DONDURMA-ONKAYIT.txt`). Holdout derslerinin düzeltmesi 0.2'ye bırakıldı (Tartışma 6); Ghostfolio düzeltmeden sonra yeniden "holdout" olarak puanlanmayacak.

## Dürüstlük notları

- **Tek holdout (n=1), tek koşu.**
- **Alan doğruluğu bağımsız ölçülemedi.** Sezgi'nin çözdüğü 26 şemanın yalnızca 3'ü hakem tarafından da çözüldü.
- **Giriş hakemlemesi kod okunarak yazılmış bir kurala dayanıyor** (`holdout-hakemle.js`). Kural ölçümden sonra, Ghostfolio'nun kodu okunarak yazıldı. Ön kayıtta bu adım "elle hakemlenecek" diye tanımlanmıştı.
- **Derinlik 6 karşılaştırması ölçümden sonra yapılan açıklayıcı bir analizdir;** birincil sonuç derinlik 12 olanıdır.
