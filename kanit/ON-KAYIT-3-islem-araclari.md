# Ön kayıt 3: işlem tool'ları ile arama düzeni (Immich test kopyası, gerçek yazma)

Bu belge ölçümden **önce** yazıldı. Görevler, başarı ölçütleri ve yöntem ölçümden sonra değiştirilmez. Sonuçlar `SONUC-11-islem-araclari.md` dosyasına yazılır.

## Soru

Token'ın açtığı işlemler yapay zekâya kendi tool'ları olarak verildiğinde (A), aynı işlemlerin genel `ara` → `tani` → `cagir` düzeniyle kullanılmasına (B) göre:
- görev başarısı,
- tur sayısı,
- maliyet

nasıl değişiyor?

## Düzen

- **Uygulama:** Immich v3.3.0-rc.0, Docker test kopyası, `127.0.0.1:2283`. Haritanın üretildiği kaynakla aynı sürüm.
- **Sezgi:** yazma modunda, hedef `--test-kopyasi` olarak bildirilmiş, insan oturumu tek bir yönetici hesabı.
- **Token:** her çalıştırmada yeni bir token üretilir, böylece tekrar koruması çalıştırmalar arasında taşınmaz. İzinler iki düzende de aynı 13 işlemdir:
  - albüm: listele, oluştur, bilgi, güncelle;
  - etiket: listele, oluştur;
  - paylaşım bağlantısı: listele, oluştur;
  - API anahtarı: listele, oluştur;
  - kullanıcı tercihleri: oku, güncelle;
  - tek okuma: `album.get-album-info`.
  Yıkıcı işlem yoktur, bu yüzden insan onayı gerekmez.
- **A:** varsayılan; işlemler kendi tool'larıyla verilir.
- **B:** `SEZGI_ISLEM_ESIGI=0`; yalnızca genel tool'lar verilir.
- **Yapay zekâ:** Claude Code komut satırı, `-p` (tek istem), `--strict-mcp-config`, yalnızca `mcp__sezgi__*` tool'larına izin. Varsayılan model; model adı sonuçla birlikte kaydedilir. Boş bir geçici klasörden çalışır, uygulamanın kodunu görmez.
- **Tekrar:** her görev her düzende 2 kez çalıştırılır.
- **Durum:** her çalıştırmadan önce Immich durumu sıfırlanır (bu ölçümün oluşturduğu albüm, etiket, bağlantı, anahtar silinir; tercihler varsayılana döner), görevin gerektirdiği başlangıç durumu Immich API'siyle doğrudan kurulur.

## Görevler ve başarı ölçütleri

Başarı, Immich API'sinden okunan son durumla otomatik denetlenir; yapay zekânın kendi beyanı sayılmaz.

| # | İstem (Türkçe) | Başlangıç | Başarı |
|---|---|---|---|
| G1 | 'Yaz 2026' adlı, açıklaması 'Deniz tatili' olan bir albüm oluştur. | — | Adı `Yaz 2026`, açıklaması `Deniz tatili` olan tam bir albüm var |
| G2 | 'Yaz 2026' albümünün adını 'Yaz 2026 - Ege' yap. | `Yaz 2026` albümü | Albümün adı `Yaz 2026 - Ege`; başka albüm oluşmamış |
| G3 | 'Seyahat' adlı bir etiket oluştur. | — | `Seyahat` etiketi var |
| G4 | 'Yaz 2026 - Ege' albümü için indirmeye izin vermeyen bir paylaşım bağlantısı oluştur. | `Yaz 2026 - Ege` albümü | Bu albüme bağlı, `allowDownload=false` olan bir paylaşım bağlantısı var |
| G5 | Adı 'yedekleme' olan ve yalnızca albümleri okuma izni (album.read) taşıyan bir API anahtarı oluştur. | — | Adı `yedekleme`, izinleri tam olarak `["album.read"]` olan bir API anahtarı var |
| G6 | Hesabımda Anılar (memories) özelliğini kapat. | Anılar açık | Tercihlerde `memories.enabled=false`; diğer tercih grupları değişmemiş |

## Ölçülenler

Her çalıştırma için:
- başarı (evet/hayır),
- tur sayısı,
- maliyet (USD, Claude Code'un bildirdiği),
- giriş ve çıkış token'ları,
- Sezgi tool çağrısı sayısı,
- süre.

## Raporlama

- Her görev ve düzen için başarı sayısı (x/2), ortalama tur ve ortalama maliyet. Toplamlar.
- Başarısız her çalıştırmanın nedeni (son durum ve yapay zekânın son mesajı) kısaca yazılır.
- 12 görev × düzen hücresi, 2 tekrar küçük bir örnektir. Sonuç bir eğilim olarak raporlanır, genelleme yapılmaz.
