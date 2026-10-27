# Genelleme testi: İkinci NestJS uygulaması (Immich), 14.10.2026

**Amaç:** NestJS adaptörünün yalnızca ilk deneme uygulamasına (özel bir uygulama) uyum sağlayıp sağlamadığını görmek.
**Uygulama:** [Immich](https://github.com/immich-app/immich) sunucusu, commit `aec0dd1`. Başkası tarafından yazılmış, açık kaynak, 457 dosya, 61.567 satır, 47 controller.
**Bağımsız doğru cevap:** Immich'in çalışan sunucudan ürettiği ve depoda yayınladığı resmî OpenAPI dosyası (`open-api/immich-openapi-specs.json`, 279 işlem). Bu dosyayı biz yazmadık.
**Karşılaştırma aracı:** `kanit/karsilastir-openapi.js`

## Sonuç

| Ölçüm | Adaptör ilk hâliyle | Genel yetenekler eklendikten sonra |
|---|---|---|
| Endpoint bulma | 237/279 (%84,9) | **279/279 (%100)** |
| Giriş durumu | %92,8 | **%100** |
| Girdi alanı bulma | 0/419 (%0) | **436/485 (%89,9)** |
| Zorunluluk | — | **%100** |
| Süre (61.567 satır) | 1.236 ms | **75 ms** |

Özel uygulamadaki sonuçlar her adımda yeniden ölçüldü ve %100'de kaldı (gerileme yok).

## Immich'in ortaya çıkardığı ve eklenen genel yetenekler

Hiçbiri Immich'e özel değildir; NestJS dünyasında yaygın kalıplardır:

| Eksik | Örnek | Eklenen yetenek |
|---|---|---|
| Sabitlerle yazılmış adresler | `@Controller(RouteKey.Asset)` | `enum` ve sabit değerlerinin çözülmesi |
| Önek başka dosyada | `setGlobalPrefix` `app.common.ts` içinde | Önek bulunamazsa bütün kaynaklarda arama |
| Zod ile yazılmış girdiler | `class X extends createZodDto(XSchema)` | Zod şemalarını okuma: `z.object`, `.extend`, `.merge`, `.pick`, `.omit`, `.partial`, `.optional/.nullish/.default`, `...Schema.shape` yayma |
| Sarmalayıcı fonksiyonlar | `withShapeExclusivity(Schema)`, `nonEmptyPartial({...})` | Sarmalayıcının içindeki şemayı okuma; adında "partial" geçen sarmalayıcı alanları isteğe bağlı yapar |
| Şemadan gelen isteğe bağlılık | `albums: AlbumsUpdateSchema` (şema `.optional()` ile bitiyor) | Atıf yapılan şemanın isteğe bağlılığı alana geçer |
| İşaretle açılan yetki | `@Authenticated({ permission: Permission.X, admin: true })`, `@Authenticated({ public: true })` | Kimlik dekoratörü, izin ve yönetici alanları, kendi içinde herkese açıklık |
| Özel özet işareti | `@Endpoint({ summary })` | Özet dekoratörleri liste olarak tanımlanabilir |
| Takma adla dışa aktarma | `export { AdminConfigDto as SystemConfigDto }` | Takma adların çözülmesi |
| Aynı depoda iki uygulama | API (`ApiModule`) ve bakım sunucusu (`MaintenanceModule`) aynı adresleri kullanıyor | Modül ağacı: `NestFactory.create(Kök)` → `@Module({ imports, controllers })`, `...dizi` açılımıyla. En çok controller'ı olan kök "ana" uygulamadır; diğerleri "ikincil" işaretlenir ve Benlik Kartı'na girmez |

## Kalan farklar ve nedenleri

1. **Sistem ayarları şeması (44 alan):** `applyVisibility(Admin)! as z.ZodType<…>` şema çalışma anında özyinelemeli bir fonksiyonla türetiliyor. Kodu çalıştırmadan kesin bilinemez. Sezgi bunu tahmin etmedi; kapsam raporunda **"şema çözülemedi"** olarak bildirdi. Bu, "bilmediğini bil" ilkesinin doğru işlediğini gösterir.
2. **Dosya yükleme alanları (5 alan):** `file`, `assetData` gibi çok parçalı (multipart) yükleme alanları girdi tanımında değil, ayrı yükleme işaretleriyle geliyor. Sonraki adaptör sürümünde eklenebilir.
3. **Belgede olmayan 16 kapı:** Çoğu kodda gerçekten var olan ama resmî dokümandan gizlenmiş eski `PATCH` takma adları ve önekten muaf tutulan iki adres (`/.well-known/immich`, `/custom.css`). Hata değil.

## Dürüstlük notları

- Bu, tek bir ikinci uygulamadır. Adaptörün olgunluğu için daha fazla NestJS uygulaması (farklı ekipler, farklı sürümler) gerekir.
- Yetenekler Immich'te görülen eksiklere bakılarak eklendi. Hepsi genel kalıplar olsa da, bir sonraki üçüncü uygulama testi adaptöre **dokunmadan** yapılmalı; gerçek genelleme oranı o zaman görülür.
- Ajan düzeyinde (A/B/C/D) görev testi Immich'te yapılmadı; bu test yalnızca haritanın doğruluğunu ölçer.
