# Veri bağları testi: hangi işlem hangi tabloyu okuyor ve yazıyor (14.10.2026)

**Ne ölçüldü:** Sezgi her endpoint için Prisma çağrılarını controller'dan servis metotlarına kadar izleyip okunan ve yazılan tabloları çıkarıyor. Bunun doğruluğu bağımsız bir hakemle karşılaştırıldı.

**Bağımsız hakem:** `hakem/veri-hakem.js`. Uygulamanın kendi TypeScript derleyicisinin **tip denetleyicisiyle** her çağrıyı gerçek hedefine çözer (düz fonksiyonlar, arayüzler, takma adlar dahil) ve 12 adıma kadar iner. Sezgi ise alan tipi ve ad eşleştirmesi kullanır. İki yöntem birbirinden bağımsızdır.

**Hakemin gerçekten izlediğinin kontrolü:** Özel uygulamada derinlik 0'da 0, derinlik 1'de 188, tam derinlikte 217 bağlı metot buldu.

## Sonuç

| Uygulama | Şema | Karşılaştırılan metot | Birebir aynı | Doğru bağ | Fazla | Eksik |
|---|---|---|---|---|---|---|
| Özel NestJS uygulaması | 43 model | 264 | 264 (%100) | 603 | 0 | 0 |
| Cal.com API v2 (`54343aa`) | 100 model, kardeş paketten (`package.json` → `prisma.schema`) | 175 | 175 (%100) | 341 | 0 | 0 |
| Immich | Prisma kullanmıyor (Kysely) | — | — | — | — | — |

Süre: Bağlar mevcut harita süresine eklendi; özel uygulamada toplam ~55 ms, Cal.com'da ~150–240 ms (kardeş paketler dahil).

## Sezgi'nin izlediği zincirler (testlerle sabitlendi)

- `this.servis.metot()`: constructor parametresi ya da alan olarak enjekte edilen servis
- `this.yardimci()`: aynı sınıftaki özel metot
- Üst sınıftan miras alınan metot (`extends TemelServis`)
- `$transaction(async (tx) => tx.model.create(...))`
- `this.kitap.count()` gibi **servis** çağrıları, adları bir modele benzese bile tablo sayılmaz

## Dürüstlük notları

- Bağlar **Sezgi'nin izleyebildiği kod için** dokunulabilecek tabloları listeler (koşullu yollar dahil); her çağrının her tabloya dokunduğu anlamına gelmez. Tam liste değildir: iç içe yazmalar, cascade ve kütüphane içi çağrılar izlenmez (Benlik SPEC 6.1). Kartta damgası `inferred`, ham SQL ya da derinlik sınırında `partial: true`.
- Derlenmiş kütüphanelere (`node_modules`) ve başka servislere giden çağrılar izlenmez. Ne Sezgi ne hakem görür; Cal.com'da `@calcom/platform-libraries` içinden yapılan veritabanı işlemleri bu yüzden ikisinde de yok. Kapsam raporu bağı olmayan kapıları ayrıca listeler.
- Hakem de Sezgi de model adını şema dosyasından aynı kuralla okur (`model X {` → `x` erişimcisi). Bu kısım ortak; bağımsız olan, çağrı zincirinin çözümüdür.
- Özel uygulamada bağ bulunamayan 47 kapının listesi incelendi: Adları dış servis kapılarına işaret ediyor (dış arama servisleri, yapay zekâ servisi, dosya yükleme imzası). Bunlardan örneklenen birkaçının kodu açılıp doğrulandı; 47 kapının hepsi tek tek açılmadı.
