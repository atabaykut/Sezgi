# Sezgi'nin hedefi

[English](VISION.md)

> **Sezgi bir uygulama değil, bir katmandır.** Bir kap boya bütün binayı boyar; bir binaya da binlerce kap sığar.

**Hedef:** Yazılımlara bir benlik vermek. Sezgi kurulduğu uygulamayı, hangi dilde yazılmış ve ne işe yarıyor olursa olsun, baştan sona sarar ve tek bir API'ye dönüştürür. Uygulamanın ne olduğunu, neyi yapabildiğini ve hangi kurala uyduğunu düşünmeden bilir. Kurana, izinlerini kendisinin belirlediği bir token verir. Bu token'ı alan her yapay zekâ, MCP aracı ya da program uygulamayı bir veritabanı gibi okuyup yazabilir; ama yalnızca izin verilen kadar ve uygulamanın kendi kurallarından geçerek.

Bu belge nereye gittiğimizi ve bugün nerede olduğumuzu dürüstçe yan yana koyar.

## Sorun

Bugün bir uygulamayı yapay zekâya bağlamak için üç iş yapılır: önce bir API yazılır, sonra dokümanı hazırlanır, sonra bir MCP sunucusu kurulur. Bu üç iş her uygulama için ayrı ayrı tekrarlanır. Çoğu şirkette hiç yapılmamıştır.

1 milyon satırlık, 1000 klasöre dağılmış ve dokümanı olmayan bir uygulama düşünün:

- **Bir yazılımcı** dokümanı çıkarmak için bütün klasörleri gezer, rotaları bulur, her birinin ne istediğini ve ne döndürdüğünü okur, tek tek yazar. Haftalar sürer. Kod değiştikçe doküman bayatlar.
- **Bir yapay zekâ** uygulamaya yabancıdır. Dosyaları tek tek açar, okur, tahmin eder ve her soruda bunu baştan yapar. Saatler ve milyonlarca LLM token'ı harcanır; cevaplar ve işlemler çoğu zaman eksik ya da yanlış olur.

İnsan böyle çalışmaz. Adı sorulunca adını söyler, "ben kimim?" diye düşünmez. Klavyede bir harfe basarken parmağı kendiliğinden gider. Bunları bir kez öğrenmiştir ve artık benliğindedir. **Sezgi yazılımlara bu benliği verir.**

## Kimin derdini çözer

| Kim | Bugünkü derdi | Sezgi ile |
|---|---|---|
| **Yazılımcılar** | Dokümansız kodu haftalarca gezip rota rota yazıyor; kod değişince her şey bayatlıyor. | Harita, OpenAPI dokümanı ve Benlik Kartı koddan kendiliğinden çıkar; kod değişince yalnızca fark yeniden okunur. |
| **Yapay zekâ asistanları ve ajanlar** | Her oturumda kodu baştan tarıyor, tahmin ediyor, kötü tanımlı araçlarla yanlış çağrı yapıyor. | Kodu taramaz, Sezgi'ye sorar; kısa ve kaynaklı cevap alır. Yanlış yazma daha kapıda, önizlemede kesilir. |
| **Kurumsal şirketler** (banka, sigorta, hastane, turizm, üretim) | Dokümanı olmayan eski iç sistemler var. Bunları yapay zekâya açmak aylar süren bir proje. Denetçi "ajan neye dokundu?" diye soruyor. | Kurulduğu gün harita çıkar. Yazma, gerçek kullanıcının kimliğiyle ve uygulamanın kendi kontrolünden geçer. Her işlem "hangi insan, hangi ajan, ne yaptı" diye kayıtlıdır. |
| **SaaS şirketleri** | Müşteriler "yapay zekâmı ürününüze bağlayabilir miyim?" diye soruyor. Her sürümde müşteri entegrasyonları kırılıyor. | Güncel OpenAPI, hazır MCP sunucusu ve Benlik Kartı kendiliğinden oluşur. `karsilastir`, müşteriyi bozacak değişikliği yayından önce yakalar. Ayrı bir entegrasyon ekibi gerekmez. |
| **Yapay zekâ ve ajan şirketleri** | Ajanları müşteri kodunu her seferinde baştan okuyor; hatalı işlemin faturası onlara kesiliyor. | Ajan kartı okur, kodu değil. Doğruluk artar; LLM token'ı ve GPU israfı düşer. |
| **Ajanslar ve entegratörler** | Her yeni müşteride sistemi sıfırdan tanımak haftalar sürüyor. | İlk gün harita çıkar. Kapsam raporu teklif aşamasında kullanılır; bulunan boşluklar müşteriye sunulur. |
| **Uygulama sahibi** | Yapay zekâya neyi açtığını göremiyor. | Ayna ekranında neyin bilindiğini ve neyin bilinmediğini görür. İzinleri kendisi seçer; son karar onda kalır. |
| **Güvenlik ve denetim ekipleri** | Dokümansız kapılar, yetki kontrolü olmayan uçlar gözden kaçıyor. | On dakikada, kimseye yazma yetkisi vermeden girişsiz yazmaları, yetkisi bulunamayan uçları ve bu sürümde kırılacak bağları görür. |

## Benliğin parçaları

Bu tablo projenin sözleşmesidir; her tasarım kararı bu maddelere hizmet eder. "Bugün" sütunu bu sürümdeki durumdur.

| # | Parça | Anlamı | Bugün |
|---|---|---|---|
| 1 | **Kendini bilir** | Uygulamanın ne olduğunu, ne yaptığını, neler yaptırılabileceğini hazır bilir. | Kısmen: `kimsin` tanıtımı ve alan sınıflandırması; bazen yanılıyor |
| 2 | **Doğrudan ulaşır** | Bir işleme ulaşmak için hiçbir şey taranmaz; cevap süresi kodun büyüklüğünden bağımsızdır. | Var: asistan haritayı sorgular, kodu taramaz |
| 3 | **Her yeri sarar** | 5 milyon satır da olsa tek beyin: hiçbir kapı, ekran, ayar dışarıda kalmaz. | Kısmen: on üç çatıda işlemler; okunamayan rota açıkça söyleniyor |
| 4 | **Tekrarı bir kez öğrenir** | 50 yerde tekrar eden şey tek bir karttır. | Kısmen: bileşik dekoratörler ve ortak ara katmanlar tek tanımdan çözülür |
| 5 | **Düşünmez, bilir** | İçinde dil modeli yoktur. Uydurmaz; ya bilir ya "bilmiyorum" der. | Var: her iddia kaynaklı ve damgalı |
| 6 | **Kuralları bilir** | Her kapının kurallarını bilir ve kapıda uygular. | Kısmen: doğrulama kuralları önizlemede uygulanır; servis içi kuralların bir kısmı |
| 7 | **Tek API'dir** | Uygulamanın tamamı tek bir kablodan erişilir. | Var: MCP, HTTP ve OpenAPI |
| 8 | **Uygulamayı veritabanı gibi kullandırır** | Okuma ve yazma bir sorgu kadar basittir. | Kısmen: canlı mod test kopyasında; gerçek veride deneysel, insan parolası ister |
| 9 | **Arayüze hâkimdir** | Ekranları, formları ve düğmeleri bilir; ekranda yapılabilen her şeyi yapabilir. | Zayıf: ön yüzün hangi isteği attığı ve tarayıcı kaydı; ekranda iş yapma yok |
| 10 | **Token verir** | Kurucu, izinlerini kendisi belirlediği erişim token'ları üretir. | Var |
| 11 | **Tek muhataptır, köprüdür** | Yapay zekâ uygulamayla değil, yalnızca Sezgi ile konuşur. | Var: santral birden çok uygulamayı tek bağlantıda toplar |
| 12 | **Ayrım yapmaz** | Oyun, hastane sistemi, fabrika yazılımı ya da bir yapay zekâ modeli; hepsini aynı şekilde sarar. | Var: çekirdekte iş alanı bilgisi yok |
| 13 | **Her dilde çalışır** | Dilden bağımsız çekirdek, her dil için takılabilir okuyucular. | Kısmen: TypeScript/JavaScript, Python, Java, Kotlin, Go, PHP, Ruby, C# |
| 14 | **Hafızası yok denecek kadar azdır** | Veriyi değil haritayı tutar; kütüphanenin kitaplarını değil kataloğunu. | Kısmen: müşteri verisi varsayılan olarak tutulmaz; harita henüz sıkıştırılmış ikili biçimde değil |
| 15 | **Kırılmaz olmaya çalışır** | Kuantuma dayanıklı standart şifreleme ve katmanlı savunma. | Kısmen: ML-DSA imza, şifreli ve zincirli kayıt; bağımsız denetim yapılmadı |
| 16 | **İsrafı sıfırlar** | GPU saatler değil saniyeler: tekrarlanan işi bir kez yapar, yanlış işi başlamadan durdurur. | Kısmen: aynı soruda daha az token (ölçümler `kanit/` altında) |
| 17 | **Bir katmandır** | Uygulamanın yerine geçmez, üstüne sürülür; kaldırılınca iz bırakmaz. | Var: uygulamanın koduna yazmaz; `sezgi kaldir` |
| 18 | **İstenirse uygulamanın ötesine geçer** | Komut satırı araçları, kaynak kodu olmayan programlar, işletim sistemi. | Başlangıç: kaynaksız keşif kimlik ve şema düzeyinde |
| 19 | **Zarar vermez** | Sezgi çökse de kaldırılsa da uygulama fark etmez; yetki kademeli açılır. | Var: varsayılan gözlem; emin olunmayan yerde karar onaydır |
| 20 | **Benlik Kartı yayınlar** | Uygulama kendini imzalı, açık bir kartla tanıtır. | Var: imzalı kart ve açık belirtim |

Ayrıca: bir kez öğrenir, değişince yalnızca farkı öğrenir; internet şart değildir, tamamen yerel çalışır; açık kaynaktır.

## Sezgi ne değildir

- **Bir uygulama değildir.** Kendi iş mantığı, kendi verisi yoktur; kurulduğu uygulamanın üstüne sürülür.
- **Bir dil modeli değildir.** Düşünmez, metin üretmez. Düşünen, ona bağlanan yapay zekâdır.
- **Bir kod arama aracı değildir.** "Bu kod nerede" sorusunu da cevaplar; asıl işi uygulamayı tek bir API olarak sunmaktır.
- **Uygulamanın mantığını değiştirmez ve kurallarını atlamaz.** Yazma uygulamanın kendi kodundan geçer; doğrudan veritabanına yazılmaz, çünkü bu yol hesaplamaları, kontrolleri ve bildirimleri atlar.
- **Yayılmaz.** Yalnızca açıkça yapılandırılan uygulamaları tanır.

## Değişmez ilkeler

1. **Zarar vermez.** Bir şey ters giderse Sezgi durur, geri alır ve ne olduğunu söyler.
2. **Bir katmandır.** Kaldırıldığında iz bırakmaz.
3. **Uygulamanın mantığına dokunulmaz.** Varsayılan olarak hiçbir kod dosyası değişmez.
4. **Uygulamanın kuralları atlanmaz.**
5. **Yalnızca yetkili kapsam.** Sadece açıkça yapılandırılan uygulamalar okunur.
6. **Sezgi düşünmez.** Bilmediğini "bilmiyorum" diye söyler.
7. **Ayrım yapılmaz.** Çekirdek hiçbir iş alanı hakkında ön bilgi taşımaz.
8. **Yerel ve internetsiz.** Kod ve veri hiçbir dış servise gönderilmez; kullanım verisi toplanmaz.
9. **Varsayılan kapalı.** Hiçbir işlem, kurucu açmadan erişilebilir olmaz.
10. **Kendine has şifreleme yok.** Yalnızca denetlenmiş, kuantuma dayanıklı standartlar.
11. **Açık biçimler.** Benlik Kartı, hafıza ve izin biçimleri açık standartlardır.
12. **Sezgi yetki vermez, yalnızca daraltır.** Etkin yetki = kullanıcının uygulamadaki yetkisi ∩ token izni. İnsan yoksa yazma da yoktur.
13. **Müşteri verisi varsayılan olarak yazılmaz.** Tutulan her şey veri defterinde ilan edilir.
14. **Önce kanıt, sonra iddia.** Her doğruluk ve hız iddiası ölçümle desteklenir.

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

Aynı ölçümde elle doğrulanan iddiaların yaklaşık yarısı doğruydu. Hedef, hiç görülmemiş uygulamalarda **%90'ın üstünde doğruluk ve sıfır "kesin" damgalı yanlış**.

## Uygulamaya üç kapıdan girilir

1. **Mevcut API'ler:** rotalar, girdiler, doğrulama, yetki, veri etkisi. Bugün Sezgi'nin asıl gücü burada.
2. **İç kod:** API'si olmayan fonksiyonları yalıtılmış ve en az yetkiyle çağırmak. Henüz yok.
3. **Arayüz:** ekranlar, formlar, düğmeler ve her düğmenin arkasındaki istek. Bugün yalnızca ön yüzün istek kalıpları ve tarayıcı kaydı.

## Haritadan anlama

Hedef, uygulamanın tamamını tek bir anlam grafında tutmaktır:

- **Düğümler:** ekran, bileşen, işlem, fonksiyon, tablo, iş kuralı, kullanıcı tipi, kullanıcı yolculuğu, dış sistem.
- **Kenarlar:** çağırır, okur, yazar, siler, tetikler, yetkilendirir, geri alır.
- **Davranış özeti:** her işlemin kimin yapabileceği, ön koşulları, hata hâlleri, veri etkisi, dış etkisi ve geri alınabilirliği; hepsi kanıtlı.

Graf ve kanıt deposu bugün var (`.sezgi/graf.json`, `.sezgi/kanit.json`). Eksik olan, ekran ve iş kuralı düğümlerinin derinliği ve her kenarın doğruluğu.

## Benliğin ötesi

Sezgi'nin yöntemi (bul, haritala, tek kabloya bağla, kuralları uygula) yalnızca uygulamalara değil, bilgisayardaki başka hedeflere de uygulanabilir:

- **Komut satırı araçları** (git, docker, ffmpeg): bütün komutlar, parametreler ve kurallar.
- **Kaynak kodu olmayan programlar** (.exe, .dll, APK, SQLite, masaüstü uygulamaları): işletim sisteminin erişilebilirlik arayüzleri ve programın açık kapıları üzerinden, bir insanın o programla yapabildiği her şey.
- **İşletim sistemi:** dosyalar, işlemler, ayarlar, hizmetler; kullanıcının yetkisi kadar.

Bu yetenek çekirdekten ayrı bir pakettir, varsayılan olarak kapalıdır ve her hedef kullanıcı tarafından tek tek, gerekçesi söylenerek eklenir. Yıkıcı işlemler her zaman onay ister. Çekirdek 1.0 ve bağımsız güvenlik denetiminden önce açılmaz. Bugün kaynaksız keşif kimlik ve şema düzeyindedir.

**Birden çok uygulama:** santral birden fazla uygulamayı tek bağlantıda toplar. Hedef, uygulamalar arası varlık eşlemesi ("bu müşteri, şu sistemdeki şu kayıt") ve adım adım onaylanan değişiklik planlarıdır. Santral ve onaylı planlar bugün var; otomatik varlık eşlemesi kısmen.

## Benlik Kartı

Her uygulama kendini imzalı, standart bir kartla tanıtır: ne yapabildiği, hangi kurala uyduğu, kimin adına yazıldığı, Sezgi'nin ne tuttuğu ve **neyden emin olunmadığı**. "Emin değilim" diyebilen ilk API tarifidir. OpenAPI bir uygulamanın menüsünü anlatır; Benlik Kartı yeteneklerini, kurallarını ve sınırlarını anlatır.

Belirtim açıktır ([benlik/SPEC.md](https://github.com/atabaykut/Sezgi/blob/main/benlik/SPEC.md)). Açık standart, büyük oyuncuların aynı şeyi kendi ürünlerine eklemesine karşı da en iyi savunmadır: standardı kopyalayan onu büyütmüş olur. 1.0 hedefi: iki bağımsız kart üreticisi ve kartı okuyan bir ajan.

## Var olanlardan farkı

| Var olan | Ne yapar | Eksik kalan |
|---|---|---|
| Çatıların OpenAPI üreticileri | Tek bir çatıda API dokümanı üretir | Kurulum ve kod içi işaretleme ister; kural, yan etki ve onay bilgisi taşımaz |
| OpenAPI → MCP çeviricileri | Hazır dokümanı MCP aracına çevirir | Doküman yoksa çalışmaz |
| MCP ağ geçitleri | Birden çok MCP sunucusunu tek kapıda toplar | Uygulamayı tanımaz, yalnızca trafiği taşır |
| Tarayıcı otomasyon ajanları | Ekranı görerek tıklar | Yavaş ve kırılgandır; düğmenin arkasındaki isteği bilmez |
| Kod indeksleri ve kod haritaları | "Bu kod nerede" sorusunu çözer | İşlem yaptırmaz; token, izin ve onay yoktur |
| Elle yazılmış asistan notları (CLAUDE.md, AGENTS.md) | Asistana bağlam verir | Elle yazılır, bayatlar; asistan yine klasörü gezer |

**Sezgi'nin farkı:** API yazmak, dokümanlamak ve MCP'ye bağlamak bugün her uygulama için ayrı ayrı yapılan üç iştir. Sezgi üçünü birden, kod yazmadan ve kendiliğinden yapar; kuralları bilir ve uygular; kod değişince kendini günceller; izinleri kurucunun belirlediği bir token ile her yapay zekâya açar. Bütün bunları yerel, internetsiz ve dil modeli olmadan yapar.

## Yol haritası

| Aşama | İçerik | Durum |
|---|---|---|
| 0.1 | Kodu okuma, harita, Benlik Kartı, MCP, okuma ve önizleme, Ayna | Yayında |
| 0.2 | Test kopyasında yazma, kimlik aktarımı, veri defteri, onay kuyruğu | Büyük ölçüde var (canlı mod) |
| Diller ve çatılar | Her dil ve çatı için yayınlanmış kalite seviyesi | On üç çatı |
| Arayüz (kapı 3) | Ekran okuma, düğme → istek eşleme | Başlangıç |
| İç kod (kapı 2) | API'si olmayan fonksiyonları yalıtılmış çağırma | Yok |
| Ölçek | Birden çok uygulama, 5 milyon satırda benchmark | Santral var; büyük benchmark yok |
| 1.0 | Bağımsız güvenlik denetimi, gerçek veride yazma, iki kart üreticisi | Yolda |
| Benliğin ötesi | Komut satırı, işletim sistemi, kaynaksız programlar (ayrı paket) | 1.0'dan sonra |

## Sırada ne var

1. **Doğruluk:** hiç görülmemiş uygulamalarda ölçüm her sürümde tekrarlanır ve yayımlanır; "kesin" damgalı yanlış sıfıra indirilir.
2. **Kapsam:** Supabase (edge function ve SQL fonksiyonları), Spring servlet filtreleri, Next.js `basePath`, sunucu şablonları (Blade, ERB, Jinja).
3. **Asistan deneyimi:** süzgeçli listeleme ("giriş gerektirmeyen yazmalar"), kısa adla çağıran/çağrılan sorgusu, "bu dosyayı değiştirirsem ne bozulur".
4. **Arayüz:** ekranlar ve düğmelerin hangi işlemi başlattığı.
5. **1.0:** bağımsız güvenlik denetimi, iki kart üreticisi, kartı okuyan ajan.
