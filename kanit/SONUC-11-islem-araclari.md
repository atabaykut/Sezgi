# Sonuç 11: işlem tool'ları ile arama düzeni (Immich test kopyası, gerçek yazma)

Ön kayıt: [`ON-KAYIT-3-islem-araclari.md`](ON-KAYIT-3-islem-araclari.md), ölçümden önce `d04e5df` ile kaydedildi. Ölçüm düzeneği: [`islem-araclari-olc.js`](islem-araclari-olc.js). Ham sonuçlar: [`islem-araclari/sonuclar.jsonl`](islem-araclari/sonuclar.jsonl). Yapay zekânın son mesajlarında geçen test hesabının e-posta adresi `<e-posta>`, paylaşım bağlantısı anahtarları `<anahtar>` ile değiştirildi.

- **Model:** `claude-opus-5-5`, Claude Code 2.1.296.
- **Uygulama:** Immich v3.3.0-rc.0, Docker test kopyası.
- **Ölçüm:** 6 görev × 2 düzen × 2 tekrar = 24 çalıştırma. Toplam maliyet 5,20 $.

## Sonuç

| | A: işlem tool'ları | B: ara → tani → cagir |
|---|---|---|
| Başarı (Immich'ten doğrulanan son durum) | **12/12** | 11/12 |
| Ortalama tur | **3,83** | 6,33 |
| Ortalama maliyet | **0,201 $** | 0,233 $ |
| Ortalama giriş token'ı | **179 bin** | 265 bin |
| Ortalama Sezgi çağrısı | **1,83** | 4,42 |

| Görev | A | B |
|---|---|---|
| G1 albüm oluştur | ✓✓ · 3 tur | ✓✓ · 6 tur |
| G2 albümün adını değiştir | ✓✓ · 4 tur | ✓✓ · 8 tur |
| G3 etiket oluştur | ✓✓ · 4 tur | ✓✓ · 6 tur |
| G4 albüme indirilemez paylaşım bağlantısı | ✓✓ · 4 tur | ✓✓ · 8 tur |
| G5 tek izinli API anahtarı | ✓✓ · 4 tur | ✓✓ · 6 tur |
| G6 Anılar özelliğini kapat (iç içe tercih) | ✓✓ · 4 tur | ✓✗ · 7 / 1 tur |

## Başarısız çalıştırma

**G6, B, 2. tekrar.** Yapay zekâ hiçbir tool çağırmadı. "Hesabımda Anılar özelliğini kapat" istemini Claude'un kendi hafıza ayarı sandı ve claude.ai'de nasıl kapatılacağını anlattı. B düzeninde yalnızca genel tool'lar (`ara`, `tani`, `cagir` …) görünür; istemin Immich'le ilgili olduğu çıkarılamadı. A düzeninde `user_update_my_preferences` tool'unun adı ve açıklaması bu bağlamı veriyor. Tek bir örnek; eğilim olarak not edilir.

## Gözlemler

- **A'da yapay zekâ doğrudan doğru tool'u çağırdı.** Gerekirse önce listeleyip kimlik buldu (G2, G4) ya da tercihleri okudu (G6).
- **B'de her görev en az `ara` → `tani` → (`onizle`) → `cagir` zincirinden geçti.** Bu 2–4 ek tur demek.
- **G4 ve G6'da iç içe şemalar işe yaradı.** G4'te paylaşım bağlantısının tür alanı izinli değer listesinden doğru seçildi. G6'da tercihlerin iç içe nesnesi şemadan okundu; diğer tercih grupları değişmedi (denetlendi).

## Düzenek düzeltmesi (sonuç sayılmayan çalıştırma)

İlk denemede Windows'ta istem kabuk argümanı olarak bozuk iletildi: tırnaklar düştü, yapay zekâ görevi almadan 1 turda bitti. Ayrıca yönetici API'sine yapılan bir çağrıda bağlantı koptu ve düzenek durdu. Bu tek çalıştırma ölçüm değil, düzenek hatasıdır ve silindi. Düzeneğe iki düzeltme yapıldı: istem standart girdiden veriliyor, Immich API çağrıları yeniden deneniyor. Ölçüm baştan, 24 çalıştırmanın tamamıyla yeniden yapıldı. Görevler, ölçütler ve yöntem ön kayıtla aynı kaldı.

## Sınırlar

- **Örneklem küçük:** tek uygulama, tek model, 6 görev, 2 tekrar. Sonuç bir eğilimdir.
- **Token tek bir görev için dar değil:** 12 işlem açıktı. Çok daha fazla işlem açılınca (eşik 40) A düzeni B'ye döner.
- **Yıkıcı işlem yok:** Görevlerde silme yoktu. Onaylı silme ayrıca uçtan uca denendi: insan onayıyla HTTP 204, kayıt zinciri sağlam.
