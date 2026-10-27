# Hesap ipuçları: ön kayıt (16.10.2026, ölçümden önce)

`tani` aracının "Hesap ipuçları" bölümü (vizyon 10, GPU ve hesap israfı) adlardan çıkarılır ve bugüne kadar GPU kullanan gerçek bir uygulamada ölçülmedi. Bu belge ölçümü **ölçümden önce** kayda geçirir. Ölçüm bir kez yapılır; sonuç ne çıkarsa yazılır; ölçüm sırasında Sezgi değiştirilmez.

## Ölçülen kod

Sezgi `c2dc794`, `cargo build --release`.

## Uygulama

mrhan1993/Fooocus-API `79718cd9932e8566465ea4c8cd98cde0f1bc3a15` (2026-08-04). Stable Diffusion XL ile görüntü üreten FastAPI uygulaması; her istek GPU'da çalışır. Ölçümden önce yalnızca dosya listesi ve `docs/api_doc_en.md` okundu; uygulama kodu okunmadı.

## Hakem (belgeden, ölçümden önce sabitlendi)

Hakem uygulamanın kendi parametre belgesidir (`docs/api_doc_en.md`). Belgeye göre GPU süresini ya da kalitesini belirleyen girdiler:

| Alan | Belgedeki açıklama |
|---|---|
| `performance_selection` | `Speed`, `Quality`, `Extreme Speed` |
| `aspect_ratios_selection` | çözünürlük |
| `image_number` | üretilecek görüntü sayısı (en fazla 32) |
| `base_model_name` | temel model |
| `refiner_model_name` | iyileştirici model |
| `uov_method` | `Vary (Subtle)`, `Vary (Strong)`, `Upscale (1.5x)`, `Upscale (2x)`, `Upscale (Fast 2x)`, `Upscale (Custom)` |
| `upscale_value` | büyütme oranı |

Belgedeki hafif seçenekler: `performance_selection` içinde `Speed` ve `Extreme Speed`; `uov_method` içinde `Upscale (Fast 2x)`.

Hesabı belirlemeyen girdiler (bunlara verilen ipucu yanlıştır): istem metinleri, tohum (`image_seed`), kaydetme ve biçim seçenekleri (`require_base64`, `save_*`, `meta_scheme`), `async_process`, `webhook_url`, girdi görüntüleri ve maskeler.

## Ölçütler

Sezgi haritası `sezgi init` ile kurulur; her uç için MCP `tani` çağrılır, "Hesap ipuçları" satırları toplanır (`kanit/hesap-ipuclari-olc.js`).
1. **Bulma:** hakem tablosundaki 7 alandan kaçı en az bir uçta ipucu aldı.
2. **Hafif seçenek:** belgedeki 3 hafif seçenekten kaçı "hafif seçenek" olarak gösterildi.
3. **Yanlış ipucu:** hesabı belirlemeyen girdilere verilen ipucu sayısı.
4. Ek bilgi: ipucu verilen uç sayısı, ipucu satırlarının toplamı.

## Bilinen sınırlar (ölçümden önce)

- Hakemi Sezgi'nin geliştiricisi belgeden çıkardı; bağımsız bir kişinin hakemlemesi değildir. Tablo ölçümden önce sabitlendiği için sonradan uydurulamaz.
- Ölçüm ipuçlarının doğru alanları gösterip göstermediğini ölçer; ipuçlarının gerçekten GPU tasarrufu sağladığını ölçmez (uygulama çalıştırılmaz).
