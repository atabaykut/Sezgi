# Sezgi'nin hedefi

[English](VISION.md)

**Hedef:** Yapay zekânın her yazılımı bir kez okuyup gerçekten tanıdığı ve içinde güvenle iş yaptığı evrensel bir yazılım zekâsı.

Bugünkü Sezgi bu hedefin ilk adımıdır: uygulamanın kodunu okur, haritasını çıkarır ve asistana kaynaklı, damgalı cevaplar verir. Bu belge nereye gittiğimizi ve bugün nerede olduğumuzu dürüstçe yan yana koyar.

## Üç ilke

1. **Uydurmaz.** Her bilgi kaynağıyla (dosya:satır) ve güven damgasıyla gelir: kesin > gözlem > yorum > bilinmiyor. Sezgi bilmediğini "bilinmiyor" diye söyler.
2. **Güvenli olmayan hiçbir şey onaysız gitmez.** Emin olunmayan yerde karar onaydır (fail-closed). Gerçek veride her yazma insanın parolasını ister.
3. **Asistan kodu taramaz, sorar.** Cevap kısa olmalı: aynı soruyu kodu tarayarak cevaplamaktan daha az token harcamalı.

## 17 temel soru

Bağlanan bir asistan, hiç görmediği bir uygulama için ilk dakikada şu soruları doğru, kaynaklı ve dürüst cevaplayabilmeli. Durum sütunu, bu sürümde hiç görülmemiş üç açık kaynak uygulamada (NestJS, Django, Next.js) yapılan son ölçümdür (2026-11-03).

| # | Soru | Bugün |
|---|---|---|
| 1 | Uygulama nedir? | Kısmen: README ve alan varlıklarından; alan sınıflandırması bazen yanılıyor |
| 2 | Neden var? | Kısmen: README'den alıntıyla |
| 3 | Hangi iş problemini çözer? | Kısmen: bazen kurulum cümlesi seçiliyor |
| 4 | Kullanıcıları kim? | Kısmen: rol adlarından |
| 5 | Kullanıcı ne yapabilir? | Çalışıyor: işlemlerin tamamına yakını bulunuyor; okunamayan rota açıkça söyleniyor |
| 6 | Hangi özellik gerçekten çalışıyor? | Kısmen: yalnızca tarayıcı kaydı ya da canlı mod gözlemiyle |
| 7 | Hangi işlem hangi ekrandan başlar? | Zayıf: yalnızca bilinen istek kalıpları |
| 8 | Hangi fonksiyonlar birbirini çağırır? | Kısmen: sekiz dilde çağrı grafiği |
| 9 | Hangi tablolar etkilenir? | Kısmen: ORM'ler ve ham SQL; kaskadlar dahil |
| 10 | Ön koşullar neler? | Kısmen: doğrulama kuralları ve servis içindeki redler |
| 11 | Kim hangi işleme yetkili? | Kısmen: giriş kararı çoğu çatıda güvenilir; roller ve özel kimlik dekoratörleri eksik kalabiliyor |
| 12 | Başka uygulamaları etkiler mi? | Kısmen: HTTP, kuyruk, e-posta, ödeme, bulut; zincirde bilinmeyen varsa "bilinmiyor" |
| 13 | Geri alınabilir mi? | Kısmen: yumuşak silme, ters işlem, sürüm kaydı, kalıcı silme |
| 14 | İnsan onayı gerekir mi? | Kısmen: karar fail-closed; gerekçe her zaman doğru değil |
| 15 | Bu bilgi kesin mi, tahmin mi? | Kısmen: her iddia damgalı; "kesin" damgalı yanlışlar hâlâ çıkabiliyor |
| 16 | Kod değişince ne geçersiz olur? | Çalışıyor: içerik özetine bağlı bayatlık |
| 17 | Bir görev için hangi işlemler gerekir? | Kısmen: Türkçe ve İngilizce görev planı; tanınmayan adım boş bırakılır |

Aynı ölçümde elle doğrulanan iddiaların yaklaşık yarısı doğruydu. Bu sürümdeki düzeltmeler (izin dekoratörleri, yumuşak silme, dış etki, geri alma) bu oranı yükseltmek içindir. Hedef, hiç görülmemiş uygulamalarda **%90'ın üstünde doğruluk ve sıfır "kesin" damgalı yanlış**.

## Haritadan anlama

Bugün haritanın merkezi işlemdir (endpoint). Hedef, uygulamanın tamamını tek bir anlam grafında tutmaktır:

- **Düğümler:** ekran, bileşen, işlem, fonksiyon, tablo, iş kuralı, kullanıcı tipi, kullanıcı yolculuğu, dış sistem.
- **Kenarlar:** çağırır, okur, yazar, siler, tetikler, yetkilendirir, geri alır.
- **Davranış özeti (DNA):** her işlemin kimin yapabileceği, ön koşulları, hata hâlleri, veri etkisi, dış etkisi ve geri alınabilirliği; hepsi kanıtlı.

Graf ve kanıt deposu bugün var (`.sezgi/graf.json`, `.sezgi/kanit.json`). Eksik olan, ekran ve iş kuralı düğümlerinin derinliği ve her kenarın doğruluğu.

## Kodun ötesi

- **Kaynak kodu olmayan sistemler:** .exe, .dll, APK, SQLite ve masaüstü uygulamaları; REA ile native ikili, firmware ve süreç gözlemi. Bugün kimlik ve şema düzeyinde.
- **Çalışma zamanı gözlemi:** tarayıcı kaydı ve senaryolar "kodda var" ile "gerçekten çalışıyor"u ayırır.
- **Birden çok uygulama:** santral birden fazla uygulamayı tek bağlantıda toplar; uygulamalar arası varlık eşlemesi ("bu müşteri, şu sistemdeki şu kayıt") ve onaylı değişiklik planları.

## Benlik Kartı

Her uygulamanın kendini imzalı, standart bir kartla tanıtması: ne yaptığı, kimin ne yapabileceği, hangi işlemin onay istediği. Belirtim açıktır ([benlik/SPEC.md](https://github.com/atabaykut/Sezgi/blob/main/benlik/SPEC.md)). 1.0 hedefi: iki bağımsız kart üreticisi ve kartı okuyan bir ajan.

## Sırada ne var

1. **Doğruluk:** hiç görülmemiş uygulamalarda ölçüm her sürümde tekrarlanır ve yayımlanır; "kesin" damgalı yanlış sıfıra indirilir.
2. **Kapsam:** Supabase (edge function ve SQL fonksiyonları), Spring servlet filtreleri, Next.js `basePath`, sunucu şablonları (Blade, ERB, Jinja).
3. **Asistan deneyimi:** süzgeçli listeleme ("giriş gerektirmeyen yazmalar"), kısa adla çağıran/çağrılan sorgusu, "bu dosyayı değiştirirsem ne bozulur".
4. **1.0:** bağımsız güvenlik denetimi, iki kart üreticisi, kartı okuyan ajan.
