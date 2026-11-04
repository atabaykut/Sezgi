# Sezgi

*Sezgi: düşünmeden bilmek.*

**Yazılımlara bir benlik.** Sezgi uygulamanızın kodunu bir kez okur, onu tek bir API'ye çevirir ve yapay zekâya kanıtla anlatır: asistanınız uygulamanızı tanısın ve onda güvenle iş yapsın.

**Kimin için:** yazılımcılar · yapay zekâ asistanları ve ajanlar · kurumsal şirketler · SaaS şirketleri · ajanslar ve entegratörler · güvenlik ve denetim ekipleri. Tanıtım sayfası: **[atabaykut.github.io/Sezgi](https://atabaykut.github.io/Sezgi/tr.html)**

Claude, Cursor ya da Copilot gibi bir asistan uygulamanızda bir iş yapmak istediğinde kodu her seferinde baştan tarar ve tahmin eder: "Kullanıcı ekleme adresi hangisiydi, hangi alanlar zorunlu, bunu kim yapabilir?" Sezgi kodu **bir kez** okur ve uygulamanın haritasını çıkarır. Asistan artık kodu taramaz, Sezgi'ye sorar; cevap kısa ve kaynağıyla birlikte gelir.

> **Durum: 0.1, deneysel.** Sezgi varsayılan olarak yalnızca okur; uygulamanızın koduna hiçbir zaman yazmaz. Asistanın uygulamayı gerçekten kullanması (canlı mod) isteğe bağlıdır ve önce bir test kopyasında çalışır. Gerçek veride yazma deneyseldir: yöneticinin bir kez kurduğu bir anahtar ve yalnızca insanın bildiği bir parola ister; aynı hesapta kabuk erişimi olan bir ajan parolayı bilmeden gerçek veriyi açamaz, isteği onaylayamaz. Parolayı öğrenen ya da yönetici yetkisiyle çalışan bir programı durdurmaz ([SECURITY.md](SECURITY.md)).

## Ne işe yarar

| | Sezgi olmadan | Sezgi ile |
|---|---|---|
| **İşlemi bulmak** | Asistan her soruda rotaları, controller'ları, servisleri tek tek açar; bir sonraki oturumda baştan başlar | Sezgi'ye bir kez sorar; işlemi, adresini ve kaynak dosyasını kısa bir cevapta alır |
| **Girdiler ve kurallar** | "`quantity` zorunlu mu, sınırı ne?" sorusunu tahminle cevaplar, istek 400 ile düşer | Kural koddan okunur (örneğin bir zod şeması: `quantity` tam sayı, 1 ile 10 arası), dosya ve satırıyla gösterilir |
| **Kim yapabilir** | Bir ucun giriş, ötekinin `admin` istediğini, üçüncüsünün yalnızca kendi kaydına izin verdiğini görmez | Giriş, roller (`requireRole('admin')`) ve sahiplik kontrolleri (`order.user_id !== req.user.id`) haritadadır |
| **Neye dokunur** | "Kullanıcı sil" tek satır gibi görünür; kullanıcının siparişlerinin de gittiğini bilmez | Her işlemin okuduğu ve yazdığı tablolar, şemadaki zincirleme silmeler dahil (`users` → `orders`, `ON DELETE CASCADE`) |
| **Dış etkiler** | "İade"yi çağırır; gerçek bir ödeme sağlayıcısına gittiğini ya da e-posta attığını bilmez | Dışarı giden HTTP çağrıları, e-posta, ödeme, kuyruk ve bulut servisleri işlem işlem listelenir |
| **Geri alınır mı?** | Kimse söylemez | Evet, kısmen, hayır ya da bilinmiyor, gerekçesiyle; geri alınamayan işlem her zaman insan onayı bekler |
| **Bilmediğinde** | Boşluğu doldurur, emin gibi konuşur | Her bilgi **kesin**, **gözlem**, **yorum** ya da **bilinmiyor** damgası taşır; boşluk raporlanır, asla doldurulmaz |
| **Uygulamada iş yapmak** | Ya hiç yapamaz ya da her şeyi açan bir anahtar alır | İsteğe bağlı canlı mod: önce test kopyası, yalnızca izin verdiğiniz işlemleri açan bir jeton, riskli adımda onay, şifreli kayıt |
| **Kod değişince** | Elle yazılmış doküman ve entegrasyon eskir; kaldırılan bir alan müşteriyi canlıda bozar | `sezgi izle` yalnızca değişeni yeniden okur; `sezgi karsilastir` ve [GitHub eylemi](https://github.com/marketplace/actions/sezgi-ai-contract-check) bir işlemi kaldıran, zorunlu girdi ya da yeni giriş kuralı ekleyen PR'ı düşürür |
| **Birden çok uygulama** | Her biri için ayrı API, doküman ve MCP sunucusu | `sezgi santral` hepsini tek bağlantıdan sunar |
| **Sahibin gördüğü** | Yapay zekânın neye erişebildiğini bilmez | `sezgi ayna` tarayıcıda neyin bilindiğini, neyin bilinmediğini ve neyin açık olduğunu gösterir |
| **Maliyet** | Her görevde kod okumaya on binlerce token | Immich'te aynı 16 görev: kod okuyan ajan 14/16, görev başına 39,5 bin token; **Sezgi 16/16, 20,5 bin** ([ölçüm](#ölçüm)) |

### Bir örnek

Bu depodaki küçük [örnek mağazayı](ornek/siparis/README.md) alın ve sorun: *"Bir kullanıcıyı silersem ne olur, geri alınabilir mi?"*

**Sezgi olmadan** asistan `server.js`'i, rota dosyalarını, `auth.js`'i, `db.js`'i ve `schema.sql`'i okur; yine de zincirleme silmeyi ya da admin kontrolünü kaçırabilir.

**Sezgi ile** bir kez sorar ve şuna benzer bir cevap alır (kısaltılmış):

> `DELETE /api/users/:id` · rol `admin` (`requireRole('admin')`) · `users`'tan siler; kullanıcının `orders` kayıtları da silinir (`schema.sql`'de `ON DELETE CASCADE`) · kalıcı silme, geri alınamaz · canlı modda her zaman insan onayı bekler.

Her parça okunduğu dosyayı gösterir. Sezgi aynı uygulamada, yalnızca koddan, iadenin `api.payments.example.com`'a gittiğini ve geri alınamadığını, makbuzun nodemailer ile e-postayla gittiğini de bulur.

### Neler sorabilirsiniz

Bağladıktan sonra asistanınıza şunları sorabilirsiniz:

- "Bu uygulamada iade nasıl başlatılır, hangi alanları ister, kim yapabilir?"
- "Kullanıcıyı silersem hangi tablolar etkilenir, geri alınabilir mi?"
- "Fatura modülünde giriş gerektirmeyen işlem var mı?"
- "Hangi işlemler uygulamanın dışına çıkıyor: ödeme, e-posta, başka servisler?"
- "Hangi işlemler insan onayı ister, neden?"
- "Hangi uçlar bir kullanıcının başkasının kaydını değiştirmesine izin veriyor?"
- "Bu dal birleşirse API'yi kullananlar için ne bozulur?"
- "Bu işlemi hangi ekran, hangi düğme başlatıyor?"
- "Sezgi bu uygulama hakkında henüz neyi bilmiyor?"

## Neden var, kimin için

Bugün bir uygulamayı yapay zekâya bağlamak için API yazılır, dokümanlanır ve MCP sunucusu kurulur; her uygulama için ayrı ayrı. Çoğu şirkette bu hiç yapılmamıştır. Sezgi bunu kod yazmadan yapan **bir katmandır**: uygulamanın üstüne sürülür, onu tek bir API'ye çevirir, kaldırılınca iz bırakmaz. Hedefi yazılımlara bir benlik vermek: yapay zekâ her yazılımı bir kez okuyup gerçekten tanısın ve içinde güvenle iş yapsın.

- **Yazılımcılar:** dokümansız kodu haftalarca gezmek yerine harita, OpenAPI ve Benlik Kartı koddan kendiliğinden çıkar.
- **Yapay zekâ asistanları ve ajanlar:** kodu her seferinde taramaz, sorar; yanlış yazma kapıda kesilir, token israfı düşer.
- **Kurumsal şirketler:** dokümansız eski sistemler aylarca entegrasyon projesi olmadan yapay zekâya açılır; yazma gerçek kullanıcının kimliğiyle, kayıtlı olarak yapılır.
- **SaaS şirketleri:** müşteriye hazır MCP ve OpenAPI; müşteriyi bozacak değişiklik yayından önce yakalanır.
- **Ajanslar ve entegratörler:** yeni müşterinin sistemi ilk gün haritalanır.
- **Güvenlik ve denetim:** girişsiz yazmalar ve yetki kontrolü bulunamayan uçlar, kimseye yetki vermeden görünür.

Bu hedef **17 temel soruyla** ölçülür (uygulama nedir, neden var, kim neyi yapabilir, bir işlem hangi tablolara ve sistemlere dokunur, geri alınabilir mi, hangi ekrandan başlar…). Bugünkü Sezgi, hiç görmediği uygulamalarda bunların 2'sini güvenilir, 14'ünü kısmen cevaplıyor. Benliğin 20 parçası, değişmez ilkeler, uygulamanın ötesi, yol haritası ve her birinin dürüst durumu: [docs/VIZYON.md](docs/VIZYON.md).

## 3 adımda başlayın

Node.js 20 ya da sonrası gerekir.

```sh
npm install -g sezgi      # ya da kurmadan: npx sezgi <komut>
cd uygulamanizin/klasoru
sezgi init                # kodu okur, haritayı .sezgi/ klasörüne yazar (büyük uygulamada birkaç saniye)
sezgi bagla               # yapay zekâ aracınıza bağlar; her aracı tek tek sorar, `sezgi kaldir` geri alır
```

Hepsi bu. Asistanınızı yeniden başlatın ve uygulamanız hakkında soru sorun.

İsteğe bağlı:

```sh
sezgi ayna                # tarayıcıda: Sezgi bu uygulama hakkında ne biliyor, ne bilmiyor
sezgi izle                # kod değiştikçe haritayı güncel tutar
sezgi doktor              # bir sorun varsa neyin eksik olduğunu ve çözüm komutunu söyler
```

npm, bilgisayarınıza uyan hazır programı kendisi seçer: Windows x64, macOS (Apple Silicon ve Intel, macOS 11+), Linux x64 ve arm64 (glibc 2.34+: Ubuntu 22.04+, Debian 12+, RHEL 9+). Alpine / musl ve diğer platformlarda kaynaktan derleyin (`cargo build --release`, Rust 1.99+).

## Haritada ne var

Sezgi her işlem (endpoint) için şunları çıkarır:

- **Ne istediği:** girdiler, tipleri, zorunlu alanlar ve doğrulama kuralları.
- **Kimin yapabileceği:** giriş gerekip gerekmediği, roller, kaydın sahibine özel denetimler.
- **Neye dokunduğu:** okuduğu ve yazdığı veritabanı tabloları, zincirleme silmeler, tetiklediği olay ve işler.
- **Dışarıya etkisi:** e-posta, ödeme, kuyruk, bulut ya da başka bir sisteme giden istek.
- **Geri alınabilirliği:** evet, kısmen, hayır ya da bilinmiyor; gerekçesiyle.
- **Ön yüzdeki karşılığı:** hangi ekranın hangi düğmesi bu işlemi çağırıyor.
- **Uygulamanın amacı:** iş alanı, kullanıcı tipleri ve iş yetenekleri; uygulamanın kendi README'sinden alıntıyla.

Her bilginin bir güven damgası vardır: **kesin** (koddan doğrudan okundu), **gözlem** (çalışırken görüldü), **yorum** (çıkarım) ya da **bilinmiyor**. Sezgi boşlukları doldurmaz, bildirir. Harita sizin bilgisayarınızda, uygulamanın `.sezgi/` klasöründe durur; haritayı çıkarırken Sezgi ağa çıkmaz ve dil modeli kullanmaz.

Sezgi ayrıca uygulamanın **[Benlik Kartı](https://github.com/atabaykut/Sezgi/blob/main/benlik/README.tr.md)**'nı yazar: başka araçların da okuyabileceği, imzalanabilen açık bir tanım dosyası (`sezgi init --imzala`).

## Desteklenen diller

| Dil ve çatı | Kalite seviyesi |
|---|---|
| TypeScript + NestJS | 2 |
| JavaScript / TypeScript + Express, Fastify, Next.js | 2 |
| Python + FastAPI | 2 |
| Python + Django (Django REST Framework dahil), Flask | 1 |
| Java / Kotlin + Spring Boot, Kotlin + Ktor | 1 |
| Ruby on Rails | 1 |
| C# + ASP.NET Core | 1 |
| Go + net/http, Gin, Echo, Chi, Fiber, gorilla/mux | 1 |
| PHP + Laravel | 1 |

**Seviye 1:** geliştirme sırasında bağımsız bir hakeme ya da elle sayıma karşı ölçüldü. **Seviye 2:** ayrıca koduna önceden bakılmamış bir uygulamada, ölçüm planı önceden kayda alınarak bir kez ölçüldü. React, Vue, Angular, Svelte, Remix ve Solid ön yüzleri de okunur. Kaynak kodu olmayan uygulamalar (`.exe`, `.dll`, APK, SQLite, Electron) için `sezgi kesfet` vardır.

Sezgi 0.1 REST/HTTP işlemlerini haritalar; GraphQL, gRPC, tRPC ve WebSocket henüz haritalanmaz. Bunlar uygulamanızda varsa, ya da Sezgi koddaki rota kayıtlarına göre çok az işlem bulduysa (kendi yönlendiricisi, yansımayla kurulan rotalar), `sezgi init`, `sezgi rapor` ve yapay zekânın gördüğü özet bunu açıkça söyler: eksik bir harita tam gibi sunulmaz.

## Ölçüm

Herkese açık, tek komutla tekrar üretilebilir ölçüm ([`kanit/acik`](https://github.com/atabaykut/Sezgi/tree/main/kanit/acik)). Doğru cevap, her uygulamanın kendi geliştiricilerinin yayınladığı resmî API belgesidir.

| Uygulama (commit) | Bulunan endpoint | Belgede olmayan | Giriş kararı | Gövde alanı |
|---|---|---|---|---|
| Immich sunucusu (`aec0dd1`, 61.567 satır) | 279/279 | 16 | %100 | 437/485 |
| Cal.com API v2 (`54343aa`, monorepo) | 121/121 | 41 | %99,2 | 250/250 |

Immich'te aynı model ve aynı 16 görevle (tek koşu):
- Kodu okuyan ajan: 14/16, görev başına 39,5 bin token.
- Resmî OpenAPI'yi okuyan ajan: 15/16, 43,6 bin token.
- **Sezgi'ye soran ajan: 16/16, 20,5 bin token.**

Adaptörler bu iki uygulamada geliştirildi. Hiç görülmemiş uygulamalardaki sonuçlar, hatalar ve uyarılar dahil, [ayrıntılı başvuruda](docs/BASVURU.md#neden) ve [`kanit/`](https://github.com/atabaykut/Sezgi/tree/main/kanit) klasöründe.

## Asistana iş yaptırmak (isteğe bağlı)

Varsayılan olarak Sezgi yalnızca haritayı sunar; asistan uygulamanızı çağıramaz. Canlı modu açarsanız asistan uygulamayı **sizin oturumunuzla** ve **sizin verdiğiniz izinlerle** kullanabilir:

```sh
sezgi canli kur           # hedef adres, test kopyası onayı, mod, oturum ve ilk izin anahtarı (token); adım adım sorar
```

Bu modda:

- **Önce test kopyası:** yazmalar yalnızca test kopyası olarak bildirdiğiniz adrese gider.
- **İzin anahtarı (token):** asistan yalnızca sizin açtığınız işlemleri görür ve çağırabilir. Varsayılan kapalıdır, süresi dolar, iptal edilebilir.
- **Riskli işler size sorulur:** silme, dış sisteme etkisi olan işlemler (e-posta, ödeme, kuyruk, bulut), geri alınamaz işlemler, geri alınabilirliği gösterilemeyen kimlik bilgisi değişiklikleri (parola, token, anahtar, iki adımlı doğrulama) ve Sezgi'nin emin olmadığı her şey `sezgi onay` ile sizin onayınızı bekler.
- **İstek önceden sınanır:** bilinen kurallara uymayan istek uygulamaya hiç gitmez.
- **Her şey kayda geçer:** şifreli, zincirlenmiş bir işlem kaydı (`sezgi kayit`).
- **Gerçek veri (deneysel):** ayrıca yöneticinin kurduğu bir anahtar (`sezgi insan kur --sistem`) ister; gerçek veride her yazma terminalde sizin parolanızla onaylanır.

Sezgi yedek almaz ve bir yazmayı geri alamaz: geri alınamayacak bir işlemden önce uygulamanızın yedeğini alın. Bütün kapılar ve koruma sınırları: [ayrıntılı başvuru, Canlı mod](docs/BASVURU.md#canlı-mod) ve [SECURITY.md](SECURITY.md).

> **Güvenlik notu.** Sezgi'nin koddan çıkardığı metinler (özetler, alan adları, kural metinleri) asistana veri olarak gider. Kötü niyetli bir depo bu metinlere talimat yerleştirebilir. Sezgi kontrol karakterlerini temizler, uzunluğu sınırlar ve asistana bu metinlere uymamasını söyler; yine de koddan gelen her metni güvenilmez kabul edin.

## Komutlar

Günlük kullanım için `init`, `bagla`, `ayna` ve `izle` yeterli. Komutların İngilizce adları da çalışır; `sezgi yardim <komut>` her komutun ayrıntısını verir.

| Grup | Komut | Ne yapar |
|---|---|---|
| Başla | `init` | Kodu okur, haritayı ve Benlik Kartı'nı `.sezgi/` klasörüne yazar |
| | `kesfet` (`discover`) | Kaynak kodu olmayan uygulamayı inceler |
| | `izle` (`watch`) | Kod değiştikçe haritayı güncel tutar |
| | `doktor` (`doctor`) | Sorun varsa neyin eksik olduğunu ve çözüm komutunu söyler |
| Yapay zekâya bağla | `bagla` (`connect`) | Yapay zekâ aracınıza bağlar |
| | `kaldir` (`remove`) | Sezgi'nin eklediği her şeyi geri alır |
| | `mcp` | MCP sunucusu (yapay zekâ aracı bunu kendisi başlatır) |
| | `sun` (`serve`) | MCP konuşmayan araçlar için yerel HTTP + OpenAPI |
| | `santral` (`switchboard`) | Birden fazla uygulamayı tek bağlantıda toplar |
| Bak | `ayna` (`mirror`) | Tarayıcıda gösterir: neler biliniyor, işlemler, izinler |
| | `rapor` (`report`) | Neyin bilindiğini ve neyin bilinmediğini listeler |
| | `karsilastir` (`diff`) | İki sürüm arasındaki bozucu değişiklikleri bulur |
| | `olc` (`measure`) | Kendi uygulamanızda hız ve doğruluk ölçer |
| | `gozlem` (`observe`) | Tarayıcı ağ kaydını kanıt olarak alır (yalnızca biçim, değer değil) |
| Canlı mod | `canli` (`live`) | Durumu gösterir; `canli kur` adım adım kurar |
| | `mod` (`mode`) | Gözlem (varsayılan), okuma ya da yazma |
| | `hedef` (`target`) | İsteklerin gideceği adres |
| | `oturum` (`session`) | Uygulamadaki kendi oturumunuz ve rolleriniz |
| | `token` | Asistanın hangi işlemleri yapabileceği |
| | `onay` (`approve`) | Onay bekleyen istekler |
| | `plan` | Birden fazla uygulamaya yayılan değişiklik planı (adım adım onaylanır) |
| | `kayit` (`log`) | Şifreli, zincirli işlem kaydı |
| Güvenlik ve veri | `anahtar` (`key`) | Benlik Kartı imza anahtarı |
| | `insan` (`human`) | Gerçek veri için yöneticinin kurduğu anahtar ve insan parolası |
| | `dogrula` (`verify`) | Kartın imzasını ve geçerliliğini doğrular |
| | `defter` (`ledger`) | Sezgi'nin ne tuttuğu ve ne kadar sakladığı |
| | `unut` (`forget`) | Bir kişinin ya da bütün müşteri verisinin silinmesi (KVKK / GDPR) |
| Gelişmiş | `hafiza` (`memory`) | Aynı okumanın tekrar uygulamaya gitmemesi (varsayılan kapalı) |
| | `paylas` (`share`) | Ekibin depoya ekleyeceği imzalı harita |
| | `rea` | Kaynaksız analiz için isteğe bağlı REA aracı |
| | `teshis` (`diagnostics`) | Yalnızca yerel sayaç tutan tanılama |

## Daha fazlası

- [Örnek uygulama](ornek/siparis/README.md): 5 dakikada deneyebileceğiniz küçük bir sipariş uygulaması ve Sezgi'nin onda bulması gerekenler (İngilizce).
- [Sık sorulan sorular](docs/FAQ.md), [mimari](docs/ARCHITECTURE.md) ve [Sezgi'yi tamamen kaldırma](docs/UNINSTALL.md) (İngilizce).
- [Ayrıntılı başvuru](docs/BASVURU.md): her dilin nasıl okunduğu, Benlik Kartı, canlı modun bütün kapıları, çalışma zamanı gözlemi, santral, bilinen sınırlar ve yol haritası.
- [SECURITY.md](SECURITY.md): tehdit modeli ve güvenlik açığı bildirimi.
- [`kanit/`](https://github.com/atabaykut/Sezgi/tree/main/kanit): bütün ölçümler, yöntemleri ve ham sonuçları.
- [Sözlük](docs/GLOSSARY.md) ve [değişiklik günlüğü](CHANGELOG.md).
- [English README](README.md).

## Lisans

Apache-2.0. Telif © 2026 Sezgi geliştiricileri.
