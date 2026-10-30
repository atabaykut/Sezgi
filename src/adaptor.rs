//! Çatı adaptörü motoru. Kural dosyasını (ör. adapters/nestjs.toml) okur ve genel TypeScript
//! modelinden kapıları (endpoint), girdi şemalarını, doğrulama kurallarını ve yetkiyi çıkarır.
//! NestJS kalıpları (dekoratör adları, guard desenleri, önek) kural dosyasında veridir; TypeScript sınıf modeli,
//! zod, modül grafiği ve Prisma izleme burada, Rust'ta durur.

use crate::model::{Dekorator, Dosya, Sinif};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct Kurallar {
    pub adaptor: AdaptorBilgi,
    uygulama: UygulamaKural,
    #[serde(default)]
    modul: Option<ModulKural>,
    kontrolcu: KontrolcuKural,
    rota: BTreeMap<String, String>,
    yetki: YetkiKural,
    parametre: ParametreKural,
    dto: DtoKural,
    ozet: OzetKural,
    #[serde(default)]
    kullanimdan_kalkma: Option<KalkmaKural>,
    #[serde(default)]
    pub veri: Option<VeriKural>,
}
#[derive(Deserialize)]
pub struct VeriKural {
    pub sema_uzantisi: String,
    #[serde(default)]
    pub paket_ayari: Vec<String>,
    model_desen: String,
    okuma: Vec<String>,
    yazma: Vec<String>,
    derinlik: usize,
}
#[derive(Deserialize)]
struct KalkmaKural {
    dekorator: Vec<String>,
    desen: String,
    yerine_desen: String,
}
#[derive(Deserialize)]
pub struct AdaptorBilgi {
    pub ad: String,
    pub surum: String,
    pub isaret_bagimlilik: String,
}
#[derive(Deserialize)]
struct UygulamaKural {
    genel_onek_dosya: String,
    genel_onek_desen: String,
}
#[derive(Deserialize)]
struct ModulKural {
    dekorator: String,
    kok_desen: String,
    /// Genel guard'ın sağlayıcı belirteci: providers: [{ provide: APP_GUARD, useClass: X }]
    genel_guard_saglayici: String,
}
#[derive(Deserialize)]
struct KontrolcuKural {
    dekorator: String,
    dosya_eki: String,
}
#[derive(Deserialize)]
struct YetkiKural {
    herkese_acik_dekorator: Vec<String>,
    guard_dekorator: String,
    rol_dekorator_desen: String,
    kimlik_guard_desen: String,
    istege_bagli_guard_desen: String,
    #[serde(default)]
    kimlik_disi_guard_desen: Option<String>,
    /// AuthGuard('strateji') biçimi ve oturum doğrulayan stratejiler (giriş başlatanlar giriş guard'ı değildir)
    #[serde(default)]
    strateji_guard_desen: Option<String>,
    #[serde(default)]
    oturum_strateji_desen: Option<String>,
    #[serde(default)]
    genel_guard_kurulum_desen: Option<String>,
    rol_oneki_sil: String,
    #[serde(default)]
    kimlik_dekorator: Vec<String>,
    #[serde(default)]
    izin_alani_desen: Option<String>,
    #[serde(default)]
    yonetici_alani_desen: Option<String>,
    #[serde(default)]
    herkese_acik_alani_desen: Option<String>,
}
#[derive(Deserialize)]
struct ParametreKural {
    govde: String,
    sorgu: String,
    yol: String,
    /// Dosya parametresi dekoratörleri (@UploadedFile): dosya gövdenin şemasında değildir, önizlemede sınanmaz
    #[serde(default)]
    dosya: Vec<String>,
    /// Metot ya da parametre dekoratörlerinde dosya yüklemesini gösteren desen (ParseFilePipe, FileInterceptor, multipart)
    #[serde(default)]
    dosya_desen: Option<String>,
}
#[derive(Deserialize)]
struct DtoKural {
    istege_bagli_dekorator: Vec<String>,
    /// Doğrulayıcı dekoratörlerin modülleri (ör. class-validator): buradan gelen dekoratör alanı doğrular
    #[serde(default)]
    dogrulayici_modul: Vec<String>,
    kural_olmayan_dekorator: Vec<String>,
    tumu_istege_bagli: Vec<String>,
    secici: Vec<String>,
    haric: Vec<String>,
    birlesim: Vec<String>,
    #[serde(default)]
    zod_dto_desen: Option<String>,
    /// Değeri bir enum'un üyeleriyle sınırlayan dekoratörler: @IsEnum(Durum)
    #[serde(default)]
    enum_dekorator: Vec<String>,
    /// Değeri sabit bir listeyle sınırlayan dekoratörler: @IsIn(['a', 'b'])
    #[serde(default)]
    liste_dekorator: Vec<String>,
}
#[derive(Deserialize)]
struct OzetKural {
    dekorator: Vec<String>,
    alan_desen: String,
}

/// Haritadan (kapilar.json) geri okunabilir: MCP işlem tool'larının girdi şeması buradan kurulur
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Alan {
    pub tip: Option<String>,
    #[serde(default)]
    pub zorunlu: bool,
    /// Zorunluluk koddan kesin çıkarılamadı (doğrulayıcı yok, tanımı izlenemeyen ya da koşullu dekoratör);
    /// `zorunlu` yalnızca tahmindir
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub belirsiz: bool,
    #[serde(default)]
    pub kurallar: Vec<String>,
    #[serde(default)]
    pub satir: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kalitimdan: Option<String>,
    /// İzin verilen değerler (IsEnum / IsIn): yalnızca tamamı çözülebildiyse
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub degerler: Vec<String>,
    /// İç içe yapı, tanımdan: alan bir nesne şemasıysa onun alanları, nesne dizisiyse öğesinin alanları.
    /// Yalnızca iç şema eksiksiz çözüldüyse dolar (kısmi alan listesi kesin sayılmaz).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ic: Option<BTreeMap<String, Alan>>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Sema {
    pub dto: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dosya: Option<String>,
    pub bulunamadi: bool,
    pub kalitim: Option<String>,
    pub alanlar: BTreeMap<String, Alan>,
    /// Gövde birkaç biçimden biri olabiliyorsa (`type Girdi = A | B`) her biçimin şeması. Hangisinin kabul edileceğine
    /// uygulamanın kendi kodu (ör. bir pipe) karar verir: şema bu yüzden `bulunamadi` kalır, adaylar yorumdur.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub adaylar: Vec<Sema>,
    /// Girdinin kendisi bu şemanın dizisi (ör. `z.array(z.object({...}))` gövde): alanlar öğenin alanlarıdır
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub dizi: bool,
}

/// Bir şemanın iç içe alanlarına ayrılan bütçe (üst düzey alanlar her zaman kalır). İç içe serializer ve DTO'lar katman
/// katman çoğalabilir: büyük bir uygulamada (ör. 900 işlemli bir Django REST uygulaması) her işlemin şeması yüzlerce KB
/// tutuyor, harita yüz MB'ı aşıp okunamıyordu.
pub const IC_ALAN_BUTCESI: usize = 120;
/// Bütçe yüzünden iç ayrıntısı kesilen alanın kuralı: önizleme bunu tanımaz, "kontrol edilemedi" sayar; böylece
/// kesilen kısım hiçbir zaman denetlenmiş sayılmaz (yazma onaysız gitmez)
pub const KESILDI_KURALI: &str = "IcAyrintiKesildi()";

/// Alanları katman katman (genişlik öncelikli) bütçeye göre kırpar; bütçe biten katmanda alanın iç ayrıntısı atılır ve
/// kesildi kuralı eklenir. Kesme olduysa true.
fn alanlari_kirp(alanlar: &mut BTreeMap<String, Alan>, butce: &mut usize) -> bool {
    let mut kesildi = false;
    // Önce bu katmandaki iç alanlar sayılır; sonra her biri (bütçe kaldıkça) kendi içine iner
    for alan in alanlar.values_mut() {
        if let Some(ic) = &alan.ic {
            if *butce >= ic.len() {
                *butce -= ic.len();
            } else {
                alan.ic = None;
                alan.kurallar.push(KESILDI_KURALI.into());
                kesildi = true;
            }
        }
    }
    for alan in alanlar.values_mut() {
        if let Some(ic) = &mut alan.ic {
            kesildi |= alanlari_kirp(ic, butce);
        }
    }
    kesildi
}

fn sema_kirp(s: &mut Sema) -> bool {
    let mut kesildi = alanlari_kirp(&mut s.alanlar, &mut IC_ALAN_BUTCESI.clone());
    for a in &mut s.adaylar {
        kesildi |= sema_kirp(a);
    }
    kesildi
}

/// Bütün işlemlerin girdi ve yanıt şemalarına iç alan bütçesini uygular; iç ayrıntısı kesilen işlem sayısını döner
pub fn semalari_kirp(kapilar: &mut [Kapi]) -> usize {
    let mut n = 0;
    for k in kapilar {
        let mut kesildi = false;
        for g in k.govde.iter_mut().chain(k.sorgu.iter_mut()).chain(k.parametreler.iter_mut()) {
            if let Some(s) = &mut g.sema {
                kesildi |= sema_kirp(s);
            }
        }
        if let Some(y) = &mut k.yanit {
            kesildi |= sema_kirp(&mut y.sema);
        }
        n += usize::from(kesildi);
    }
    n
}

#[derive(Serialize, Clone, Debug)]
pub struct Girdi {
    pub ad: Option<String>,
    pub tip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sema: Option<Sema>,
}

/// İşlemin başarılı yanıtının şeması (girdi şemasıyla aynı yapı). Kaynağı: FastAPI `response_model=` ya da dönüş tipi,
/// NestJS `@ApiResponse({ type })` ya da metodun dönüş tipi, Express'te `res.json({...})` nesnesi (yalnızca yorum)
#[derive(Serialize, Clone, Debug)]
pub struct Yanit {
    /// "response_model", "donus_tipi", "ApiResponse", "res.json"
    pub kaynak: String,
    /// Açık beyan ya da tek anlamlı tip (kesin); çıkarım (yorum)
    pub kesin: bool,
    /// Başarı durum kodu (biliniyorsa)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durum: Option<u16>,
    pub sema: Sema,
}

/// Uygulama geneli OpenAPI işlem kimliği üreticisi: `operationIdFactory: (controllerKey, methodKey) => ifade`.
/// İfade yalnızca bu iki adın ve metin parçalarının birleşimiyse (`methodKey`, `` `${controllerKey}_${methodKey}` ``,
/// `controllerKey + '_' + methodKey`) çözülür; başka bir şey (fonksiyon çağrısı) tahmin edilmez
pub struct KimlikFabrikasi {
    /// (metin parçası, 0 = metin, 1 = controller adı, 2 = metot adı)
    parcalar: Vec<(String, u8)>,
}

impl KimlikFabrikasi {
    pub fn oku(metin: &str) -> Option<KimlikFabrikasi> {
        static D: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let re = D.get_or_init(|| {
            Regex::new(r"operationIdFactory\s*:\s*\(\s*([A-Za-z_$][\w$]*)\s*(?::\s*\w+)?\s*,\s*([A-Za-z_$][\w$]*)\s*(?::\s*\w+)?\s*\)\s*(?::\s*\w+\s*)?=>\s*([^,\n}]+)").unwrap()
        });
        let c = re.captures(metin)?;
        let (ctrl, met, ifade) = (&c[1], &c[2], c[3].trim().trim_end_matches(';').trim());
        let ad = |x: &str| -> Option<u8> {
            match x.trim() {
                a if a == ctrl => Some(1),
                a if a == met => Some(2),
                _ => None,
            }
        };
        let mut parcalar = vec![];
        if let Some(sablon) = ifade.strip_prefix('`').and_then(|s| s.strip_suffix('`')) {
            let mut kalan = sablon;
            while let Some(i) = kalan.find("${") {
                parcalar.push((kalan[..i].to_string(), 0));
                let j = kalan[i..].find('}')? + i;
                parcalar.push((String::new(), ad(&kalan[i + 2..j])?));
                kalan = &kalan[j + 1..];
            }
            parcalar.push((kalan.to_string(), 0));
        } else {
            for p in ifade.split('+') {
                let p = p.trim();
                match ad(p) {
                    Some(t) => parcalar.push((String::new(), t)),
                    None if p.starts_with(['\'', '"']) => parcalar.push((tirnaksiz(p), 0)),
                    None => return None,
                }
            }
        }
        Some(KimlikFabrikasi { parcalar })
    }

    pub fn uret(&self, kontrolcu: &str, metot: &str) -> String {
        self.parcalar
            .iter()
            .map(|(m, t)| match t {
                1 => kontrolcu,
                2 => metot,
                _ => m.as_str(),
            })
            .collect()
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Kapi {
    pub id: String,
    pub yontem: String,
    pub yol: String,
    pub kontrolcu: String,
    pub fonksiyon: String,
    pub dosya: String,
    pub satir: usize,
    pub giris: String,
    pub guardlar: Vec<String>,
    pub roller: Vec<String>,
    /// Rol kararı kesin değilse "varsayim": çözülemeyen rol değeri ya da sınıf ve metot düzeyinde birlikte rol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roller_kaniti: Option<String>,
    pub ozet: Option<String>,
    pub govde: Vec<Girdi>,
    pub sorgu: Vec<Girdi>,
    pub parametreler: Vec<Girdi>,
    /// Kullanımdan kaldırılmış kapı; yapay zekâ yerine gelen işlemi kullanmalı
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub kullanimdan_kalkmis: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yerine: Option<String>,
    /// API sürümü (ör. "2024-06-14"); aynı adrese birden fazla sürüm cevap verebilir
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surum: Option<String>,
    /// Kapının ait olduğu uygulama (kök modül). Modül bilgisi yoksa boş.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uygulama: Option<String>,
    /// Ana uygulamaya mı ait (aynı depodaki ikincil uygulamalar için false)
    pub ana: bool,
    /// Giriş kararının dayanağı: "kesin" (tanımı izlenen işaret ya da modül grafiğindeki tanınan guard), "varsayim" (yokluktan),
    /// "varsayim_bilinmeyen_guard" (tanınmayan guard var), "varsayim_isaret" (herkese açıklık işareti doğrulanamadı)
    pub giris_kaniti: String,
    /// Veritabanı bağları: işlemin okuduğu ve yazdığı tablolar (ORM şeması varsa)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub veri: Option<VeriBag>,
    /// Başarılı yanıtın şeması (bulunduysa)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yanit: Option<Yanit>,
}

#[derive(Serialize, Clone, Default, Debug)]
pub struct VeriBag {
    pub okur: Vec<String>,
    pub yazar: Vec<String>,
    /// Ham SQL gibi tabloyu koddan çıkaramadığımız bir çağrı görüldü: liste eksik olabilir
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub eksik_olabilir: bool,
}

pub struct Motor<'a> {
    k: &'a Kurallar,
    kok: PathBuf,
    dosyalar: &'a [Dosya],
    /// sınıf adı -> (dosya indeksi, sınıf indeksi) listesi
    sinif_dizini: HashMap<&'a str, Vec<(usize, usize)>>,
    rol_desen: Regex,
    kimlik_desen: Regex,
    istege_bagli_desen: Regex,
    ozet_desen: Regex,
    tip_desen: Regex,
    yol_desen: Regex,
    surum_desen: Regex,
    /// enum ve sabit değerleri: "RouteKey.Asset" -> [(dosya indeksi, "assets")]. Aynı ad birden fazla dosyada olabilir:
    /// hangisinin kastedildiği kullanan dosyaya göre seçilir (`dosya_sec`)
    sabitler: HashMap<&'a str, Vec<(usize, &'a str)>>,
    /// sabitten sabite atıflar: "VERSION_X_VALUE" -> [(dosya indeksi, "VERSION_X")]
    atiflar: HashMap<&'a str, Vec<(usize, &'a str)>>,
    /// zod şema adı -> (dosya indeksi, şema indeksi) adayları
    zod_dizini: HashMap<&'a str, Vec<(usize, usize)>>,
    zod_dto_desen: Option<Regex>,
    izin_desen: Option<Regex>,
    yonetici_desen: Option<Regex>,
    acik_alan_desen: Option<Regex>,
    /// (kullanımdan kalkma deseni, yerine gelen işlem deseni) — bir kez derlenir
    kalkma_desenleri: Option<(Regex, Regex)>,
    /// ORM model erişimcisi -> model adı: "user" -> "User"
    modeller: HashMap<String, String>,
    /// enum adı -> (dosya indeksi, metin değerleri) adayları
    enumlar: HashMap<&'a str, Vec<(usize, &'a Vec<String>)>>,
    /// ORM şemasındaki enum'lar (Prisma `enum Durum { ACIK KAPALI }`, `@prisma/client`'tan içe aktarılır);
    /// aynı adlı TypeScript enum'u varsa o önceliklidir
    orm_enumlar: HashMap<String, Vec<String>>,
    /// fonksiyon / sabit tanımı adı -> tanımın bulunduğu dosyalar
    tanim_dizini: HashMap<&'a str, Vec<usize>>,
    /// ad dizileri (`const LISTE = ['a', 'b']`, modül ve controller listeleri) -> (dosya indeksi, öğeler) adayları
    dizi_dizini: HashMap<&'a str, Vec<(usize, &'a Vec<String>)>>,
    /// Girdi şemaları çözülürken görülen, önizlemenin çalıştıramadığı nesne düzeyi kurallar (ör. zod `superRefine`).
    /// Her işlemin girdileri çözülmeden önce boşaltılır, sonra işlemin "denetlenemeyen" notlarına aktarılır
    nesne_notlari: std::cell::RefCell<Vec<String>>,
    /// Yanıt şemaları (tip adı, isteyen dosya) -> şema: aynı yanıt DTO'su birçok işlemde döner
    yanit_semalari: std::cell::RefCell<HashMap<(String, PathBuf), Sema>>,
}

/// Bir alan dekoratörünün zorunluluğa etkisi, dekoratörün tanımından çıkarılır
#[derive(Clone, Copy, PartialEq, Debug)]
enum DekAnlam {
    /// Alanı koşulsuz isteğe bağlı yapar (IsOptional, ValidateIf ya da bunlara çözülen özel dekoratör)
    Istege,
    /// İsteğe bağlılık yalnızca bir koşulda eklenir: zorunluluk koddan kesin çıkarılamaz
    KosulluIstege,
    /// Alanı doğrular (class-validator): alan yoksa istek reddedilir
    Dogrulayici,
    /// Doğrulamaya etkisi yok (ApiProperty, Type...)
    Etkisiz,
    /// Tanımı izlenemedi
    Bilinmeyen,
}

use crate::guvenli::{goreli, temiz};

fn tirnaksiz(s: &str) -> String {
    s.trim().trim_matches(|c| c == '\'' || c == '"' || c == '`').to_string()
}

/// Metni üst düzeyde `ayrac` karakterlerinden böler: parantez, köşeli ve süslü parantez ile metin ('…', "…", `…`)
/// içindeki ayraçlar sayılmaz. İlk eşleşmede durmak için `ilk` (anahtar: değer ayırmak için).
fn ust_duzey_bol(s: &str, ayrac: char, ilk: bool) -> Vec<String> {
    let (mut derinlik, mut parca, mut parcalar) = (0i32, String::new(), Vec::new());
    let mut tirnak: Option<char> = None;
    let mut kacis = false;
    for ch in s.chars() {
        if let Some(t) = tirnak {
            parca.push(ch);
            if kacis {
                kacis = false;
            } else if ch == '\\' {
                kacis = true;
            } else if ch == t {
                tirnak = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => {
                tirnak = Some(ch);
                parca.push(ch);
            }
            '[' | '(' | '{' => {
                derinlik += 1;
                parca.push(ch);
            }
            ']' | ')' | '}' => {
                derinlik -= 1;
                parca.push(ch);
            }
            x if x == ayrac && derinlik == 0 && !(ilk && !parcalar.is_empty()) => parcalar.push(std::mem::take(&mut parca)),
            _ => parca.push(ch),
        }
    }
    parcalar.push(parca);
    parcalar.into_iter().map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect()
}

/// `[a, b, ...c]` dizisinin öğeleri; dizi değilse None
fn dizi_ogeleri(t: &str) -> Option<Vec<String>> {
    let ic = t.trim().strip_prefix('[')?.strip_suffix(']')?;
    Some(ust_duzey_bol(ic, ',', false))
}

/// `{ a: ..., b: [...] }` nesne metninde üst düzey `anahtar`ın değeri. Kısayol üye (`{ controllers }`) adın kendisidir.
/// İç içe nesnelerdeki aynı adlı anahtarlar (ör. `imports: [X.forRoot({ providers: [...] })]`) sayılmaz.
fn nesne_degeri(nesne: &str, anahtar: &str) -> Option<String> {
    let ic = nesne.trim().strip_prefix('{')?.strip_suffix('}')?;
    for uye in ust_duzey_bol(ic, ',', false) {
        let kv = ust_duzey_bol(&uye, ':', true);
        match kv.as_slice() {
            [k, v] if k.trim_matches(|c| c == '\'' || c == '"') == anahtar => return Some(v.clone()),
            [k] if k == anahtar => return Some(k.clone()),
            _ => {}
        }
    }
    None
}

/// Nesnedeki `anahtar: [ ... ]` dizisinin öğeleri. Değer bir adsa (`controllers: LISTE`) `...LISTE` döner, çağıran
/// sabit dizilerden açar; değer başka bir ifadeyse (fonksiyon çağrısı) boş döner.
fn dizi_cek(nesne: &str, anahtar: &str) -> Vec<String> {
    let Some(v) = nesne_degeri(nesne, anahtar) else { return vec![] };
    if let Some(o) = dizi_ogeleri(&v) {
        return o;
    }
    if !v.is_empty() && v.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.') {
        return vec![format!("...{v}")];
    }
    vec![]
}

/// Test dosyası mı (uçtan uca testler, test modülleri): kök modül ve genel guard aranırken sayılmaz.
/// Yol uygulama köküne göreli verilir (uygulamanın kendisi bir "tests" klasöründe olabilir)
fn test_dosyasi(yol: &Path) -> bool {
    let s = format!("/{}", yol.to_string_lossy().replace('\\', "/").to_lowercase());
    s.contains("/test/") || s.contains("/tests/") || s.contains("/__tests__/") || s.contains(".spec.") || s.contains(".test.") || s.contains("-spec.") || s.contains(".e2e.")
}

/// Modül grafiği: controller'ların ait olduğu kök modüller ve her kökten ulaşılan modüllerdeki genel guard'lar
struct UygulamaHaritasi {
    /// controller -> kök modüller
    ait: HashMap<String, Vec<String>>,
    ana: Option<String>,
    /// kök modül -> o kökten ulaşılan modüllerde `APP_GUARD` olarak verilen guard'lar
    kok_guardlari: HashMap<String, Vec<String>>,
    /// Kök modül bulunamadığında: grafikten bağımsız bulunan APP_GUARD'lar (kesin değil)
    yedek_guardlar: Vec<String>,
}

/// Takma ad ve atıf zincirlerinde izlenecek en fazla adım (döngü ve kötü niyetli derinlik sınırı)
const EN_UZUN_ZINCIR: usize = 32;

/// `extends` ifadesi: `Taban`, `Taban<T>`, `PartialType(OmitType(X, ['id'] as const))` gibi
#[derive(Debug, PartialEq)]
enum Turetme {
    Sinif(String),
    Cagri(String, Vec<Turetme>),
    /// Metin anahtar listesi; öğelerden biri metin değilse (değişken, yayma) None
    Anahtarlar(Option<Vec<String>>),
    /// Seçenek nesnesi, metin ya da tanınmayan argüman
    Diger,
}

/// `extends` metnini türetme ağacına çevirir; beklenmeyen sözdiziminde None (çağıran belirsiz sayar)
fn turetme_ayristir(metin: &str) -> Option<Turetme> {
    struct A {
        c: Vec<char>,
        i: usize,
    }
    impl A {
        fn bak(&self) -> Option<char> {
            self.c.get(self.i).copied()
        }
        fn bosluk(&mut self) {
            while self.bak().is_some_and(char::is_whitespace) {
                self.i += 1;
            }
        }
        /// Dengeli parantez bloğunu ve içindeki metinleri atlar: { ... }, < ... >
        fn atla(&mut self, ac: char, kapa: char) -> Option<()> {
            let mut d = 0usize;
            while let Some(ch) = self.bak() {
                self.i += 1;
                match ch {
                    '\'' | '"' | '`' => {
                        self.metin(ch)?;
                    }
                    x if x == ac => d += 1,
                    x if x == kapa => {
                        d = d.checked_sub(1)?;
                        if d == 0 {
                            return Some(());
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        /// Açılış tırnağından sonra: kapanışa kadar okur
        fn metin(&mut self, tirnak: char) -> Option<String> {
            let mut s = String::new();
            while let Some(ch) = self.bak() {
                self.i += 1;
                match ch {
                    '\\' => {
                        s.push(self.bak()?);
                        self.i += 1;
                    }
                    x if x == tirnak => return Some(s),
                    x => s.push(x),
                }
            }
            None
        }
        fn ad(&mut self) -> String {
            let bas = self.i;
            while self.bak().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.') {
                self.i += 1;
            }
            self.c[bas..self.i].iter().collect()
        }
        /// `as const` / `as X` eki
        fn as_eki(&mut self) {
            self.bosluk();
            let geri = self.i;
            if self.ad() == "as" {
                self.bosluk();
                self.ad();
            } else {
                self.i = geri;
            }
        }
        fn dizi(&mut self) -> Option<Turetme> {
            self.i += 1; // [
            let mut ogeler = Some(Vec::new());
            loop {
                self.bosluk();
                match self.bak()? {
                    ']' => {
                        self.i += 1;
                        return Some(Turetme::Anahtarlar(ogeler));
                    }
                    ',' => self.i += 1,
                    t @ ('\'' | '"' | '`') => {
                        self.i += 1;
                        let m = self.metin(t)?;
                        if let Some(o) = ogeler.as_mut() {
                            o.push(m);
                        }
                    }
                    _ => {
                        // Değişken, yayma ya da ifade: liste kesin bilinemez
                        ogeler = None;
                        while !matches!(self.bak()?, ',' | ']') {
                            match self.bak()? {
                                '(' => self.atla('(', ')')?,
                                '[' => self.atla('[', ']')?,
                                _ => self.i += 1,
                            }
                        }
                    }
                }
            }
        }
        fn ifade(&mut self, derinlik: usize) -> Option<Turetme> {
            if derinlik > 16 {
                return None;
            }
            self.bosluk();
            let t = match self.bak()? {
                '[' => self.dizi()?,
                '{' => {
                    self.atla('{', '}')?;
                    Turetme::Diger
                }
                t @ ('\'' | '"' | '`') => {
                    self.i += 1;
                    self.metin(t)?;
                    Turetme::Diger
                }
                c if c.is_alphabetic() || c == '_' || c == '$' => {
                    let ad = self.ad();
                    self.bosluk();
                    if self.bak() == Some('<') {
                        self.atla('<', '>')?;
                        self.bosluk();
                    }
                    if self.bak() == Some('(') {
                        self.i += 1;
                        let mut args = Vec::new();
                        loop {
                            self.bosluk();
                            if self.bak()? == ')' {
                                self.i += 1;
                                break;
                            }
                            args.push(self.ifade(derinlik + 1)?);
                            self.as_eki();
                            self.bosluk();
                            match self.bak()? {
                                ',' => self.i += 1,
                                ')' => {
                                    self.i += 1;
                                    break;
                                }
                                _ => return None,
                            }
                        }
                        Turetme::Cagri(ad, args)
                    } else {
                        Turetme::Sinif(ad)
                    }
                }
                _ => return None,
            };
            Some(t)
        }
    }
    let mut a = A { c: metin.chars().collect(), i: 0 };
    a.ifade(0)
}

fn yol_birlestir(parcalar: &[&str]) -> String {
    let p: Vec<&str> = parcalar.iter().map(|x| x.trim_matches('/')).filter(|x| !x.is_empty()).collect();
    format!("/{}", p.join("/"))
}

impl<'a> Motor<'a> {
    pub fn yeni(k: &'a Kurallar, kok: &Path, dosyalar: &'a [Dosya]) -> Self {
        let mut sinif_dizini: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
        let mut sabitler: HashMap<&str, Vec<(usize, &str)>> = HashMap::new();
        let mut atiflar: HashMap<&str, Vec<(usize, &str)>> = HashMap::new();
        let mut zod_dizini: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
        let mut enumlar: HashMap<&str, Vec<(usize, &Vec<String>)>> = HashMap::new();
        let mut tanim_dizini: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut dizi_dizini: HashMap<&str, Vec<(usize, &Vec<String>)>> = HashMap::new();
        for (di, d) in dosyalar.iter().enumerate() {
            for (a, v) in &d.diziler {
                dizi_dizini.entry(a.as_str()).or_default().push((di, v));
            }
            for (a, v) in &d.enumlar {
                enumlar.entry(a.as_str()).or_default().push((di, v));
            }
            for a in d.tanimlar.keys() {
                tanim_dizini.entry(a.as_str()).or_default().push(di);
            }
            for (si, s) in d.siniflar.iter().enumerate() {
                sinif_dizini.entry(s.ad.as_str()).or_default().push((di, si));
            }
            for (a, v) in &d.sabitler {
                sabitler.entry(a.as_str()).or_default().push((di, v.as_str()));
            }
            for (a, v) in &d.sabit_atiflari {
                atiflar.entry(a.as_str()).or_default().push((di, v.as_str()));
            }
            for (zi, z) in d.zod_semalar.iter().enumerate() {
                zod_dizini.entry(z.ad.as_str()).or_default().push((di, zi));
            }
        }
        let desen = |s: &Option<String>| s.as_deref().map(crate::kural::derle);
        Motor {
            k,
            kok: kok.to_path_buf(),
            dosyalar,
            sinif_dizini,
            rol_desen: crate::kural::derle(&k.yetki.rol_dekorator_desen),
            kimlik_desen: crate::kural::derle(&k.yetki.kimlik_guard_desen),
            istege_bagli_desen: crate::kural::derle(&k.yetki.istege_bagli_guard_desen),
            ozet_desen: crate::kural::derle(&k.ozet.alan_desen),
            tip_desen: crate::kural::derle(r"^[A-Z]\w*$"),
            // { path: 'x' } ya da { path: Sabit.Uye } ya da { path: ['a', 'b'] }
            yol_desen: crate::kural::derle(r"path:\s*(\[[^\]]*\]|'[^']*'|\x22[^\x22]*\x22|`[^`]*`|[\w.]+)"),
            surum_desen: crate::kural::derle(r"version:\s*(\[[^\]]*\]|[^,}\n]+)"),
            sabitler,
            atiflar,
            zod_dizini,
            zod_dto_desen: desen(&k.dto.zod_dto_desen),
            izin_desen: desen(&k.yetki.izin_alani_desen),
            yonetici_desen: desen(&k.yetki.yonetici_alani_desen),
            acik_alan_desen: desen(&k.yetki.herkese_acik_alani_desen),
            kalkma_desenleri: k.kullanimdan_kalkma.as_ref().map(|kk| (crate::kural::derle(&kk.desen), crate::kural::derle(&kk.yerine_desen))),
            modeller: HashMap::new(),
            orm_enumlar: HashMap::new(),
            enumlar,
            tanim_dizini,
            dizi_dizini,
            nesne_notlari: Default::default(),
            yanit_semalari: Default::default(),
        }
    }

    /// Bir fonksiyon / sabit tanımını bul: önce içe aktarma yolu, sonra aynı dosya, sonra adı tek olan tanım.
    /// Belirsizse (aynı adlı birden fazla tanım) bulunamadı sayılır.
    fn tanim_bul(&self, ad: &str, kimden: &Dosya) -> Option<(&'a Dosya, &'a Vec<crate::model::Cagri>)> {
        let adaylar = self.tanim_dizini.get(ad)?;
        let sec = |di: usize| self.dosyalar[di].tanimlar.get(ad).map(|t| (&self.dosyalar[di], t));
        if let Some(modul) = kimden.iceaktarmalar.get(ad).filter(|m| m.starts_with('.')) {
            let taban = kimden.yol.parent()?.join(modul);
            for &di in adaylar {
                let d = &self.dosyalar[di];
                if d.yol.with_extension("") == taban || (d.yol.parent() == Some(taban.as_path()) && d.yol.file_stem().is_some_and(|s| s == "index")) {
                    return sec(di);
                }
            }
        }
        if let Some(&di) = adaylar.iter().find(|&&di| self.dosyalar[di].yol == kimden.yol) {
            return sec(di);
        }
        match adaylar.as_slice() {
            [di] => sec(*di),
            _ => None,
        }
    }

    /// Herkese açıklık işaretleri: her biri işaretin yazdığı üstveri anahtarı. `SetMetadata(ANAHTAR, true)` doğrudan ya da
    /// projedeki bir dekoratörün tanımında (koşulsuz) bulunursa anahtar bilinir; yalnızca adı tanınan (ör. kütüphaneden
    /// gelen `@Public()`) işaretin anahtarı None'dır.
    /// (anahtar, adı herkese açıklık işareti mi). `SetMetadata(K, true)` yazan her işaret herkese açıklık değildir
    /// (ör. @AllowDuringImpersonation): çağıran, adı listede olmayan işareti ancak anahtarı bir giriş guard'ınca
    /// okunuyorsa hesaba katar.
    fn acik_isaretleri(&self, ds: &[Dekorator], dosya: &'a Dosya) -> Vec<(Option<String>, bool)> {
        let mut out = Vec::new();
        for x in ds {
            let ad = dosya.iceaktarma_asil_adlari.get(&x.ad).cloned().unwrap_or_else(|| x.ad.clone());
            let adli = self.k.yetki.herkese_acik_dekorator.contains(&ad);
            match self.meta_yazar(&ad, &x.argumanlar, dosya, 0).or_else(|| (ad != x.ad).then(|| self.meta_yazar(&x.ad, &x.argumanlar, dosya, 0)).flatten()) {
                Some(a) => out.push((Some(a), adli)),
                None if adli => out.push((None, true)),
                None => {}
            }
        }
        out
    }

    /// Dekoratör `SetMetadata(ANAHTAR, true)` yazıyorsa ANAHTAR (sabit çözülerek); tanım 3 adıma kadar izlenir
    fn meta_yazar(&self, ad: &str, argumanlar: &[String], dosya: &'a Dosya, derinlik: usize) -> Option<String> {
        if ad == "SetMetadata" {
            return match argumanlar {
                [k, v] if v.trim() == "true" => Some(self.deger(k, Some(dosya))),
                _ => None,
            };
        }
        if derinlik >= 3 || ad.contains('.') {
            return None;
        }
        let (d2, cagrilar) = self.tanim_bul(ad, dosya)?;
        cagrilar.iter().filter(|c| !c.kosullu && c.ad != ad).find_map(|c| self.meta_yazar(&c.ad, &c.argumanlar, d2, derinlik + 1))
    }

    /// Bileşik dekoratörler açılmış dekoratör listesi: projedeki bir dekoratörün tanımı koşulsuz olarak guard, rol ya da
    /// üstveri dekoratörü uyguluyorsa (`applyDecorators(SetMetadata(..), UseGuards(..))` ya da doğrudan `UseGuards(..)`,
    /// iç içe 3 adıma kadar) onlar da listeye eklenir. NestJS applyDecorators'a verilen her dekoratörü uygular; koşullu
    /// çağrılar (if, üçlü ifade, iç fonksiyon) eklenmez. Asıl dekoratör de listede kalır.
    fn acilmis(&self, ds: &[Dekorator], dosya: &'a Dosya) -> Vec<Dekorator> {
        let mut out = ds.to_vec();
        for x in ds {
            self.dek_ac(&x.ad, dosya, 0, &mut out);
        }
        out
    }

    fn dek_ac(&self, ad: &str, dosya: &'a Dosya, derinlik: usize, out: &mut Vec<Dekorator>) {
        let asil = dosya.iceaktarma_asil_adlari.get(ad).map(String::as_str).unwrap_or(ad);
        if derinlik >= 3 || asil.contains('.') || asil == self.k.yetki.guard_dekorator || asil == "SetMetadata" {
            return;
        }
        let Some((d2, cagrilar)) = self.tanim_bul(asil, dosya) else { return };
        for c in cagrilar.iter().filter(|c| !c.kosullu && c.ad != asil) {
            let c_asil = d2.iceaktarma_asil_adlari.get(&c.ad).cloned().unwrap_or_else(|| c.ad.clone());
            if c_asil == self.k.yetki.guard_dekorator || c_asil == "SetMetadata" || self.rol_desen.is_match(&c_asil) || self.k.yetki.herkese_acik_dekorator.contains(&c_asil) {
                out.push(Dekorator { ad: c_asil, argumanlar: c.argumanlar.clone() });
            } else if c_asil != "applyDecorators" {
                self.dek_ac(&c.ad, d2, derinlik + 1, out);
            }
        }
    }

    /// Guard sınıfının (ve üst sınıflarının) Reflector ile okuduğu üstveri anahtarları; sınıf bu depoda yoksa
    /// (kütüphane, `AuthGuard('jwt')` gibi bir çağrı) None
    fn guard_anahtarlari(&self, guard: &str, kimden: &'a Dosya) -> Option<Vec<String>> {
        let ad = guard.trim().trim_start_matches("new ").trim_end_matches("()");
        if !ad.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
            return None;
        }
        let (mut d, mut s) = self.sinif_bul(ad, kimden)?;
        let mut anahtarlar = Vec::new();
        for _ in 0..5 {
            for m in &s.metotlar {
                for (yol, arg) in &m.meta_okumalari {
                    let p: Vec<&str> = yol.split('.').collect();
                    if p.len() == 3 && p[0] == "this" && self.alan_tipi(s, p[1]).as_deref() == Some("Reflector") {
                        anahtarlar.push(self.deger(arg, Some(d)));
                    }
                }
            }
            let Some(ust) = s.kalitim.as_deref().map(|k| k.split(['(', '<', ' ']).next().unwrap_or(k).to_string()) else { break };
            match self.sinif_bul(&ust, d) {
                Some((d2, s2)) => (d, s) = (d2, s2),
                None => break,
            }
        }
        Some(anahtarlar)
    }

    /// Dekoratörün zorunluluğa etkisi: ad listeleri (IsOptional), modülü (class-validator) ya da projedeki tanımı.
    /// Tanım içindeki çağrılar 4 adıma kadar izlenir.
    fn dek_anlami(&self, ad: &str, dosya: &'a Dosya, derinlik: usize) -> DekAnlam {
        let dto = &self.k.dto;
        // İçe aktarma takma adı asıl adına çevrilir (import { IsOptional as Secimli }); modül takma adla kayıtlı kalır
        let modul = dosya.iceaktarmalar.get(ad);
        let ad = dosya.iceaktarma_asil_adlari.get(ad).map(String::as_str).unwrap_or(ad);
        if dto.istege_bagli_dekorator.iter().any(|x| x == ad) {
            return DekAnlam::Istege;
        }
        if dto.kural_olmayan_dekorator.iter().any(|x| x == ad) {
            return DekAnlam::Etkisiz;
        }
        if let Some(m) = modul {
            if dto.dogrulayici_modul.iter().any(|x| m == x || m.starts_with(&format!("{x}/"))) {
                return DekAnlam::Dogrulayici;
            }
        }
        if derinlik >= 4 {
            return DekAnlam::Bilinmeyen;
        }
        let Some((d2, cagrilar)) = self.tanim_bul(ad, dosya) else { return DekAnlam::Bilinmeyen };
        let (mut istege, mut kosullu, mut dogrulayici) = (false, false, false);
        for c in cagrilar {
            // Dizi ve nesne işlemleri (d.push, Reflect.x) ve kendine atıf anlam taşımaz; içlerindeki çağrılar ayrıca kayıtlı
            if c.ad.contains('.') || c.ad == ad {
                continue;
            }
            match self.dek_anlami(&c.ad, d2, derinlik + 1) {
                DekAnlam::Istege if c.kosullu => kosullu = true,
                DekAnlam::Istege => istege = true,
                DekAnlam::KosulluIstege => kosullu = true,
                DekAnlam::Dogrulayici => dogrulayici = true,
                _ => {}
            }
        }
        if istege {
            DekAnlam::Istege
        } else if kosullu {
            DekAnlam::KosulluIstege
        } else if dogrulayici {
            DekAnlam::Dogrulayici
        } else {
            DekAnlam::Bilinmeyen
        }
    }

    /// ORM şema dosyalarının metinlerinden modelleri öğrenir (ör. schema.prisma içindeki `model User {`)
    pub fn veri_semalari(mut self, metinler: &[String]) -> Self {
        if let Some(v) = &self.k.veri {
            let re = crate::kural::derle(&v.model_desen);
            for m in metinler {
                for c in re.captures_iter(m) {
                    let ad = c[1].to_string();
                    let mut erisimci: String = ad.chars().take(1).flat_map(|c| c.to_lowercase()).collect();
                    erisimci.push_str(&ad[ad.chars().next().map(|c| c.len_utf8()).unwrap_or(0)..]);
                    self.modeller.insert(erisimci, ad);
                }
                self.orm_enumlarini_oku(m);
            }
        }
        self
    }

    /// Prisma enum blokları: üyeler tanımlayıcıdır, istemcideki değer de odur (`@map` yalnızca veritabanı adını değiştirir).
    /// Blok tam okunamazsa (beklenmeyen satır) enum eklenmez: eksik değer listesi kesin sayılmaz.
    fn orm_enumlarini_oku(&mut self, metin: &str) {
        static DESEN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let re = DESEN.get_or_init(|| Regex::new(r"(?m)^\s*enum\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{([^}]*)\}").unwrap());
        for c in re.captures_iter(metin) {
            let mut degerler = vec![];
            let mut tam = true;
            for satir in c[2].lines().map(|l| l.split("//").next().unwrap_or("").trim()).filter(|l| !l.is_empty() && !l.starts_with("@@")) {
                let ad = satir.split_whitespace().next().unwrap_or("");
                if ad.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') && !ad.is_empty() {
                    degerler.push(ad.to_string());
                } else {
                    tam = false;
                }
            }
            if tam && !degerler.is_empty() {
                self.orm_enumlar.insert(c[1].to_string(), degerler);
            }
        }
    }

    /// Enum adının değerleri: önce TypeScript enum'u (aynı adlı birden fazla enum varsa kullanan dosyanın gördüğü), sonra
    /// ORM şemasındaki enum. Hangi enum'un kastedildiği belirsizse ve değerleri farklıysa None: değer listesi uydurulmaz.
    fn enum_degerleri(&self, ad: &str, kimden: Option<&'a Dosya>) -> Option<Vec<String>> {
        match self.enumlar.get(ad) {
            Some(adaylar) => self.aday_sec(ad, adaylar, kimden, |a, b| a == b).cloned(),
            None => self.orm_enumlar.get(ad).cloned(),
        }
    }

    /// Bir metottan başlayıp servis metotlarına inerek ORM çağrılarını toplar.
    /// Yalnızca kesin izlenebilen zincirler: this.metot(), this.alan.metot() (alanın tipi bilinen bir sınıfsa)
    fn veri_bagi(&self, d: &'a Dosya, s: &'a Sinif, m: &'a crate::model::Metot) -> Option<VeriBag> {
        let v = self.k.veri.as_ref()?;
        if self.modeller.is_empty() {
            return None;
        }
        let mut bag = VeriBag::default();
        let mut gorulen = std::collections::HashSet::new();
        let mut yigin: Vec<(&Dosya, &Sinif, &crate::model::Metot, usize)> = vec![(d, s, m, 0)];
        while let Some((d, s, m, derinlik)) = yigin.pop() {
            if !gorulen.insert((s.ad.as_str(), m.ad.as_str(), d.yol.as_path())) {
                continue;
            }
            for c in &m.cagrilar {
                let p: Vec<&str> = c.split('.').collect();
                let n = p.len();
                // $queryRaw / $executeRaw: hangi tabloya dokunduğu SQL metnindedir, izlenmez
                if p[n - 1].starts_with("$queryRaw") || p[n - 1].starts_with("$executeRaw") || p[n - 1] == "$runCommandRaw" {
                    bag.eksik_olabilir = true;
                    continue;
                }
                // <alıcı>.<model>.<işlem>: alıcı "this" ise bu bir sınıf alanı çağrısıdır, ORM değil
                if n >= 3 && p[n - 3] != "this" {
                    if let Some(model) = self.modeller.get(p[n - 2]) {
                        if v.okuma.iter().any(|x| x == p[n - 1]) {
                            bag.okur.push(model.clone());
                            continue;
                        }
                        if v.yazma.iter().any(|x| x == p[n - 1]) {
                            bag.yazar.push(model.clone());
                            continue;
                        }
                    }
                }
                if p[0] == "this" && derinlik >= v.derinlik && n <= 3 {
                    // Zincir derinlik sınırına takıldı: bağ listesi eksik olabilir
                    bag.eksik_olabilir = true;
                }
                if derinlik >= v.derinlik || p[0] != "this" {
                    continue;
                }
                let hedef = match n {
                    2 => self.metot_bul(d, s, p[1]),
                    3 => self.alan_tipi(s, p[1]).and_then(|t| self.sinif_bul(&t, d)).and_then(|(d2, s2)| self.metot_bul(d2, s2, p[2])),
                    _ => None,
                };
                if let Some((d2, s2, m2)) = hedef {
                    yigin.push((d2, s2, m2, derinlik + 1));
                }
            }
        }
        for l in [&mut bag.okur, &mut bag.yazar] {
            l.sort();
            l.dedup();
        }
        Some(bag)
    }

    /// İşlemin davranış zinciri: handler metodundan servis metotlarına (`this.m()`, `this.alan.m()`, üst sınıf dahil) ve
    /// adıyla çağrılan üst düzey yardımcı fonksiyonlara (`requireAccess(...)`) en fazla 4 kat iner. Dönen: davranış özeti
    fn dna_zinciri(&self, d: &'a Dosya, s: &'a Sinif, m: &'a crate::model::Metot) -> Value {
        const DERINLIK: usize = 4;
        let mut halkalar = vec![];
        let mut gorulen = std::collections::HashSet::new();
        // (dosya, sınıf: metotsa, ad, davranış öğeleri, üye çağrıları, uzaklık)
        type Dugum<'b> = (&'b Dosya, Option<&'b Sinif>, &'b str, &'b crate::dna::GovdeDna, &'b [String], usize);
        let mut yigin: Vec<Dugum<'a>> = vec![(d, Some(s), m.ad.as_str(), &m.dna, m.cagrilar.as_slice(), 0)];
        while let Some((d, s, ad, dna, uyeler, derinlik)) = yigin.pop() {
            if !gorulen.insert((d.yol.as_path(), s.map(|x| x.ad.as_str()), ad)) || halkalar.len() >= 32 {
                continue;
            }
            halkalar.push(crate::dna::Halka { dosya: goreli(&self.kok, &d.yol), dna, derinlik, tur: if derinlik == 0 { "isleyici" } else { "servis" } });
            if derinlik >= DERINLIK {
                continue;
            }
            let mut sonraki: Vec<Dugum<'a>> = vec![];
            if let Some(s) = s {
                for c in uyeler {
                    let p: Vec<&str> = c.split('.').collect();
                    let hedef = match (p.first(), p.len()) {
                        (Some(&"this"), 2) => self.metot_bul(d, s, p[1]),
                        (Some(&"this"), 3) => self.kalitimli_alan_tipi(d, s, p[1]).and_then(|(t, dt)| self.sinif_bul(&t, dt)).and_then(|(d2, s2)| self.metot_bul(d2, s2, p[2])),
                        _ => None,
                    };
                    if let Some((d2, s2, m2)) = hedef {
                        sonraki.push((d2, Some(s2), m2.ad.as_str(), &m2.dna, m2.cagrilar.as_slice(), derinlik + 1));
                    }
                }
            }
            for ad in dna.cagrilar.iter().filter(|a| !a.contains('.')) {
                if let Some((d2, _)) = self.tanim_bul(ad, d) {
                    if let Some((a2, t)) = d2.tanim_dna.get_key_value(ad.as_str()) {
                        sonraki.push((d2, None, a2.as_str(), t, &[], derinlik + 1));
                    }
                }
            }
            yigin.extend(sonraki.into_iter().rev());
        }
        crate::dna::ozet(&halkalar)
    }

    /// TypeORM varlıklarındaki kaskad silmeler: [ebeveyn, çocuk, "dosya:satır"]
    /// Dahili olay ve iş düğümleri: dinleyici dekoratörlü metotlar (`@OnEvent`, `@OnJob`, `@Process`, `@EventPattern`) ve
    /// her birinin zincirinin yaydığı olaylar, kuyruğa koyduğu işler ve sildiği tablolar. "event:X" / "job:X" -> düğüm
    fn olaylar(&self) -> BTreeMap<String, Value> {
        let mut v: BTreeMap<String, Value> = BTreeMap::new();
        for d in self.dosyalar.iter().filter(|d| !d.ek_paket) {
            for s in &d.siniflar {
                for m in &s.metotlar {
                    for x in &m.dekoratorler {
                        let asil = d.iceaktarma_asil_adlari.get(&x.ad).cloned().unwrap_or_else(|| x.ad.clone());
                        let Some((tur, ad)) = crate::dna::dinleyici(&asil, &x.argumanlar) else { continue };
                        let ozet = self.dna_zinciri(d, s, m);
                        let dugum = v.entry(format!("{tur}:{ad}")).or_insert_with(|| json!({ "tur": tur, "ad": ad, "dinleyiciler": [], "tetikler": [], "silinen": [] }));
                        if let Some(l) = dugum["dinleyiciler"].as_array_mut() {
                            l.push(json!({ "fonksiyon": format!("{}.{}", s.ad, m.ad), "kanit": format!("{}:{}", goreli(&self.kok, &d.yol), m.satir) }));
                        }
                        for (alan, anahtar) in [("tetikler", "tetikler"), ("silinen", "silinen")] {
                            let ekler = ozet[anahtar].as_array().cloned().unwrap_or_default();
                            if let Some(l) = dugum[alan].as_array_mut() {
                                for e in ekler {
                                    if !l.contains(&e) {
                                        l.push(e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        v
    }

    /// Varlık sınıflarındaki kaskadlar (TypeORM `@ManyToOne(..., { onDelete: 'CASCADE' })`, `@ForeignKeyColumn(() => X,
    /// { onDelete: 'CASCADE' })`). Sınıfın tablo adı `@Entity('ad')` / `@Table('ad')` / `@Table({ name: 'ad' })`'dan;
    /// yoksa sınıf adı. Sorgu kodu (Kysely `deleteFrom('asset')`) tablo adıyla sildiği için kaskad tablo adlarıyla yazılır
    fn typeorm_kaskadlari(&self) -> Vec<Value> {
        let tablo_adi = |s: &Sinif| -> String {
            s.dekoratorler
                .iter()
                .filter(|x| matches!(x.ad.as_str(), "Entity" | "Table"))
                .find_map(|x| {
                    let a = x.argumanlar.first()?.trim();
                    if a.starts_with(['\'', '"', '`']) {
                        return Some(tirnaksiz(a));
                    }
                    let i = a.find("name")?;
                    let kalan = a[i + 4..].trim_start().strip_prefix(':')?.trim_start();
                    let son = kalan.get(1..)?.find(['\'', '"', '`'])? + 1;
                    Some(tirnaksiz(&kalan[..=son]))
                })
                .filter(|a| !a.is_empty())
                .unwrap_or_else(|| s.ad.clone())
        };
        let siniflar: HashMap<&str, String> = self.dosyalar.iter().flat_map(|d| d.siniflar.iter()).map(|s| (s.ad.as_str(), tablo_adi(s))).collect();
        let mut v = vec![];
        for d in self.dosyalar.iter().filter(|d| !d.ek_paket) {
            for s in &d.siniflar {
                for o in &s.ozellikler {
                    for x in &o.dekoratorler {
                        if let Some(e) = crate::dna::typeorm_kaskadi(&s.ad, &x.ad, &x.argumanlar) {
                            let eb = siniflar.get(e.as_str()).cloned().unwrap_or(e);
                            let cocuk = siniflar.get(s.ad.as_str()).cloned().unwrap_or_else(|| s.ad.clone());
                            v.push(json!([eb, cocuk, format!("{}:{}", goreli(&self.kok, &d.yol), o.satir)]));
                        }
                    }
                }
            }
        }
        v
    }

    /// Alanın tipi, sınıfta yoksa üst sınıflarında (`class AssetService extends BaseService`, alanlar üst sınıfın
    /// constructor'ında): (tip adı, alanı tanımlayan sınıfın dosyası: tip adı o dosyanın içe aktarmalarıyla çözülür)
    fn kalitimli_alan_tipi(&self, d: &'a Dosya, s: &'a Sinif, alan: &str) -> Option<(String, &'a Dosya)> {
        let (mut d, mut s) = (d, s);
        for _ in 0..5 {
            if let Some(t) = self.alan_tipi(s, alan) {
                return Some((t, d));
            }
            let ust: String = s.kalitim.as_deref()?.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let (d2, s2) = self.sinif_bul(&ust, d)?;
            d = d2;
            s = s2;
        }
        None
    }

    /// Sınıf alanının tipi: constructor(private readonly x: Tip) ya da x: Tip
    fn alan_tipi(&self, s: &Sinif, alan: &str) -> Option<String> {
        let tip = s
            .metotlar
            .iter()
            .filter(|m| m.ad == "constructor")
            .flat_map(|m| m.parametreler.iter())
            .find(|p| p.ad == alan)
            .and_then(|p| p.tip.clone())
            .or_else(|| s.ozellikler.iter().find(|o| o.ad == alan).and_then(|o| o.tip.clone()))?;
        let ad: String = tip.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        (!ad.is_empty()).then_some(ad)
    }

    /// Metodu sınıfta, yoksa üst sınıflarında arar
    fn metot_bul(&self, d: &'a Dosya, s: &'a Sinif, ad: &str) -> Option<(&'a Dosya, &'a Sinif, &'a crate::model::Metot)> {
        let (mut d, mut s) = (d, s);
        for _ in 0..5 {
            if let Some(m) = s.metotlar.iter().find(|m| m.ad == ad) {
                return Some((d, s, m));
            }
            let ust: String = s.kalitim.as_deref()?.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let (d2, s2) = self.sinif_bul(&ust, d)?;
            d = d2;
            s = s2;
        }
        None
    }

    /// Aynı adlı tanımların bulunduğu dosyalar arasından `kimden` dosyasının gördüğünü seçer: göreli içe aktarma yolu,
    /// aynı dosya, tek aday, takma adlı içe aktarma yolunun sonu, uygulamanın kendi dosyası. `ad` içe aktarmada aranan
    /// addır (`Rol.Admin` için `Rol`). Dönen: `dosyalar` içindeki konum. Belirsizse None: tahmin edilmez.
    fn dosya_sec(&self, ad: &str, dosyalar: &[usize], kimden: &Dosya) -> Option<usize> {
        if let Some(modul) = kimden.iceaktarmalar.get(ad).filter(|m| m.starts_with('.')) {
            let taban = kimden.yol.parent()?.join(modul);
            if let Some(i) = dosyalar.iter().position(|&di| {
                let d = &self.dosyalar[di];
                d.yol.with_extension("") == taban || d.yol.parent() == Some(taban.as_path()) && d.yol.file_stem().is_some_and(|s| s == "index")
            }) {
                return Some(i);
            }
        }
        if let Some(i) = dosyalar.iter().position(|&di| self.dosyalar[di].yol == kimden.yol) {
            return Some(i);
        }
        if dosyalar.len() == 1 {
            return Some(0);
        }
        // Takma adlı içe aktarma (ör. "@/modules/x/y.input"): yolun sonu aday dosyanın yoluna uyuyorsa o
        if let Some(modul) = kimden.iceaktarmalar.get(ad) {
            let son: String = modul.trim_start_matches(['@', '~', '#']).trim_start_matches('/').to_string();
            let uyan: Vec<usize> = (0..dosyalar.len())
                .filter(|&i| {
                    let y = self.dosyalar[dosyalar[i]].yol.with_extension("").to_string_lossy().replace('\\', "/");
                    !son.is_empty() && (y.ends_with(&son) || y.ends_with(&format!("{son}/index")))
                })
                .collect();
            if let [i] = uyan.as_slice() {
                return Some(*i);
            }
        }
        // Uygulamanın kendi dosyası kardeş paketteki aynı adlı tanımdan önce gelir
        let kendi: Vec<usize> = (0..dosyalar.len()).filter(|&i| !self.dosyalar[dosyalar[i]].ek_paket).collect();
        match kendi.as_slice() {
            [i] => Some(*i),
            _ => None,
        }
    }

    /// Ad dizini adaylarından kullanan dosyanın gördüğünü seçer (`dosya_sec`). Dosya bilinmiyor ya da seçim belirsizse
    /// ancak bütün adaylar aynı şeyi söylüyorsa (`ayni`) o kullanılır; aksi hâlde None.
    fn aday_sec<T: Copy>(&self, ad: &str, adaylar: &[(usize, T)], kimden: Option<&Dosya>, ayni: impl Fn(&T, &T) -> bool) -> Option<T> {
        let ice = ad.split('.').next().unwrap_or(ad);
        let dosyalar: Vec<usize> = adaylar.iter().map(|(di, _)| *di).collect();
        if let Some(i) = kimden.and_then(|k| self.dosya_sec(ice, &dosyalar, k)) {
            return Some(adaylar[i].1);
        }
        let (_, ilk) = adaylar.first()?;
        adaylar.iter().all(|(_, x)| ayni(ilk, x)).then_some(*ilk)
    }

    /// Öğe listesindeki `...LISTE` açılımları (sabit diziler) `kimden` dosyasının gördüğü diziden açılır; her iç açılım
    /// dizinin tanımlandığı dosyadan çözülür. Hangi dizinin kastedildiği belirsizse açılım atlanır (tahmin edilmez).
    fn dizi_ac(&self, ogeler: &[String], kimden: &Dosya, out: &mut Vec<String>, derinlik: usize) {
        for o in ogeler {
            let Some(ad) = o.strip_prefix("...").map(str::trim) else {
                out.push(o.clone());
                continue;
            };
            let Some(adaylar) = self.dizi_dizini.get(ad).filter(|_| derinlik < 8) else { continue };
            let dosyalar: Vec<usize> = adaylar.iter().map(|(di, _)| *di).collect();
            let secilen = match self.dosya_sec(ad, &dosyalar, kimden) {
                Some(i) => Some(adaylar[i]),
                None if adaylar.iter().all(|(_, v)| *v == adaylar[0].1) => Some(adaylar[0]),
                None => None,
            };
            if let Some((di, v)) = secilen {
                self.dizi_ac(v, &self.dosyalar[di], out, derinlik + 1);
            }
        }
    }

    /// Dekoratör argümanının koddan çözülen değeri: 'metin' -> metin, RouteKey.Asset -> assets. Sabit zinciri izlenir;
    /// her adım, adın tanımlandığı dosyanın bakış açısından çözülür. Çözülemezse ya da hangi tanımın kastedildiği
    /// belirsizse None.
    fn sabit_coz(&self, s: &str, kimden: Option<&'a Dosya>) -> Option<String> {
        let t = s.trim();
        if t.starts_with(['\'', '"', '`']) {
            return Some(tirnaksiz(t));
        }
        // Sabit zincirini izle: X -> Y -> "değer"
        let (mut ad, mut yer) = (t, kimden);
        for _ in 0..8 {
            if let Some(v) = self.sabitler.get(ad).and_then(|a| self.aday_sec(ad, a, yer, |x, y| x == y)) {
                return Some(v.to_string());
            }
            let adaylar = self.atiflar.get(ad)?;
            let dosyalar: Vec<usize> = adaylar.iter().map(|(di, _)| *di).collect();
            let i = match yer.and_then(|k| self.dosya_sec(ad.split('.').next().unwrap_or(ad), &dosyalar, k)) {
                Some(i) => i,
                None if adaylar.iter().all(|(_, x)| *x == adaylar[0].1) => 0,
                None => return None,
            };
            ad = adaylar[i].1;
            yer = Some(&self.dosyalar[adaylar[i].0]);
        }
        None
    }

    /// Dekoratör argümanını değere çevirir; çözülemeyen değer olduğu gibi kalır
    fn deger(&self, s: &str, kimden: Option<&'a Dosya>) -> String {
        self.sabit_coz(s, kimden).unwrap_or_else(|| s.trim().to_string())
    }

    /// Değer koddan çözülebiliyor mu: metin ya da bulunan sabit / enum üyesi (sayı değerli enum, değişken çözülmez)
    fn deger_cozulur(&self, s: &str, kimden: Option<&'a Dosya>) -> bool {
        self.sabit_coz(s, kimden).is_some()
    }

    /// Dizi ya da tek değer: ["/a", "/b"] -> ["/a", "/b"], "/a" -> ["/a"]
    fn degerler(&self, s: &str, kimden: Option<&'a Dosya>) -> Vec<String> {
        match dizi_ogeleri(s) {
            Some(o) => o.iter().map(|o| self.deger(o, kimden)).collect(),
            None => vec![self.deger(s.trim(), kimden)],
        }
    }

    /// Uygulama sürümlü işlemleri nasıl seçiyor (`app.enableVersioning({ type: VersioningType.X, header: '...' })`).
    /// `header` ve `media_type` (anahtarıyla) kesindir; `uri` ve `custom` (özel fonksiyon) Sezgi'nin göndereceği bir
    /// başlıkla seçilemez. Çağrı yoksa ya da birden fazla farklı çağrı varsa null: sürüm seçimi bilinmiyor.
    fn surum_secimi(&self) -> Value {
        static DESEN: std::sync::OnceLock<(Regex, Regex, Regex, Regex)> = std::sync::OnceLock::new();
        let (cagri, tur, baslik, anahtar) = DESEN.get_or_init(|| {
            (
                Regex::new(r"enableVersioning\(\s*\{").unwrap(),
                Regex::new(r"type:\s*VersioningType\.(HEADER|MEDIA_TYPE|URI|CUSTOM)").unwrap(),
                Regex::new(r#"header:\s*['"]([^'"]+)['"]"#).unwrap(),
                Regex::new(r#"key:\s*['"]([^'"]+)['"]"#).unwrap(),
            )
        });
        let mut bulunan: Vec<Value> = vec![];
        for d in self.dosyalar.iter().filter(|d| !d.ek_paket && !test_dosyasi(d.yol.strip_prefix(&self.kok).unwrap_or(&d.yol))) {
            for m in cagri.find_iter(&d.metin) {
                // Seçenek nesnesi: ilk kapanan süslü paranteze kadar (iç içe nesne varsa tür yine ilk satırlardadır)
                let govde: String = d.metin[m.end()..].chars().take(600).collect();
                let govde = govde.split("})").next().unwrap_or("");
                let v = match tur.captures(govde).map(|c| c[1].to_string()).as_deref() {
                    Some("HEADER") => match baslik.captures(govde) {
                        Some(b) => json!({ "tur": "header", "baslik": b[1].to_string() }),
                        None => json!({ "tur": "custom" }),
                    },
                    Some("MEDIA_TYPE") => match anahtar.captures(govde) {
                        Some(a) => json!({ "tur": "media_type", "anahtar": a[1].to_string() }),
                        None => json!({ "tur": "custom" }),
                    },
                    Some("URI") => json!({ "tur": "uri" }),
                    _ => json!({ "tur": "custom" }),
                };
                if !bulunan.contains(&v) {
                    bulunan.push(v);
                }
            }
        }
        match bulunan.as_slice() {
            [tek] => tek.clone(),
            _ => Value::Null,
        }
    }

    fn genel_onek(&self) -> String {
        let re = crate::kural::derle(&self.k.uygulama.genel_onek_desen);
        // Ayrıştırılmış (yorumları silinmiş) metin kullanılır: yorumdaki setGlobalPrefix önek değildir
        let ilk = self.kok.join(&self.k.uygulama.genel_onek_dosya);
        if let Some(c) = self.dosyalar.iter().find(|d| d.yol == ilk).and_then(|d| re.captures(&d.metin)) {
            return c[1].to_string();
        }
        // Önek başka bir dosyada tanımlanmış olabilir (ör. app.common.ts); test dosyaları ve kardeş paketler sayılmaz
        for d in self.dosyalar.iter().filter(|d| !d.ek_paket && !test_dosyasi(d.yol.strip_prefix(&self.kok).unwrap_or(&d.yol))) {
            if let Some(c) = re.captures(&d.metin) {
                return c[1].to_string();
            }
        }
        String::new()
    }

    /// Hangi controller hangi uygulamaya (kök modül) ait, her kökte hangi genel guard'lar (APP_GUARD) kurulu?
    /// Genel guard yalnızca kökten içe aktarmalarla ulaşılan modüllerin `providers` dizisinden okunur (AST; yorum,
    /// test modülü ve uygulamanın içe aktarmadığı kardeş paket sayılmaz).
    /// Modül grafiği (bağımlılık grafiği için yan bilgi): modül -> tanım yeri, içe aktardığı modüller, kontrolcüleri
    fn modul_grafi(&self) -> Value {
        let Some(mk) = &self.k.modul else { return json!({}) };
        let bas_ad = crate::kural::derle(r"^[\w$]+");
        let ac = |ogeler: &[String], kimden: &Dosya| -> Vec<String> {
            let mut tam = Vec::new();
            self.dizi_ac(ogeler, kimden, &mut tam, 0);
            tam.into_iter()
                .filter_map(|o| {
                    let o = match o.strip_prefix("forwardRef") {
                        Some(r) => r.split_once("=>").map(|(_, x)| x.trim().trim_end_matches(')').trim().to_string()).unwrap_or_default(),
                        None => o,
                    };
                    bas_ad.find(&o).map(|m| m.as_str().to_string())
                })
                .collect()
        };
        let mut v = serde_json::Map::new();
        for d in self.dosyalar.iter().filter(|d| !d.ek_paket) {
            for s in &d.siniflar {
                let Some(m) = s.dekoratorler.iter().find(|x| x.ad == mk.dekorator) else { continue };
                let arg = m.argumanlar.first().cloned().unwrap_or_default();
                v.insert(
                    s.ad.clone(),
                    json!({ "dosya": goreli(&self.kok, &d.yol), "satir": s.satir, "imports": ac(&dizi_cek(&arg, "imports"), d), "kontrolculer": ac(&dizi_cek(&arg, "controllers"), d) }),
                );
            }
        }
        Value::Object(v)
    }

    fn uygulama_haritasi(&self) -> UygulamaHaritasi {
        let mut sonuc: HashMap<String, Vec<String>> = HashMap::new();
        let mut kok_guardlari: HashMap<String, Vec<String>> = HashMap::new();
        let Some(mk) = &self.k.modul else { return UygulamaHaritasi { ait: sonuc, ana: None, kok_guardlari, yedek_guardlar: vec![] } };
        let bas_ad = crate::kural::derle(r"^[\w$]+");
        let mut moduller: HashMap<&str, (&Dekorator, &Dosya)> = HashMap::new();
        for d in self.dosyalar {
            for s in &d.siniflar {
                if let Some(m) = s.dekoratorler.iter().find(|x| x.ad == mk.dekorator) {
                    moduller.insert(s.ad.as_str(), (m, d));
                }
            }
        }
        // Öğenin adı: X, X.forRoot(...) -> X, forwardRef(() => X) -> X. `...LISTE` açılımı `kimden` dosyasının gördüğü diziden
        let ac = |ogeler: &[String], kimden: &Dosya, out: &mut Vec<String>| {
            let mut tam = Vec::new();
            self.dizi_ac(ogeler, kimden, &mut tam, 0);
            for o in tam {
                let o = match o.strip_prefix("forwardRef") {
                    Some(r) => r.split_once("=>").map(|(_, x)| x.trim().trim_end_matches(')').trim().to_string()).unwrap_or_default(),
                    None => o,
                };
                if let Some(m) = bas_ad.find(&o) {
                    out.push(m.as_str().to_string());
                }
            }
        };
        // providers: [{ provide: APP_GUARD, useClass: X }] (özellik sırası önemsiz); useFactory ile kurulan guard'ın adı okunamaz
        let saglayici = mk.genel_guard_saglayici.as_str();
        let genel_guardlar = |arg: &str, kimden: &Dosya| -> Vec<String> {
            let mut tam = Vec::new();
            self.dizi_ac(&dizi_cek(arg, "providers"), kimden, &mut tam, 0);
            tam.iter()
                .filter(|o| nesne_degeri(o, "provide").is_some_and(|p| p.rsplit('.').next() == Some(saglayici)))
                .map(|o| {
                    nesne_degeri(o, "useClass")
                        .or_else(|| nesne_degeri(o, "useExisting"))
                        .and_then(|s| bas_ad.find(&s).map(|m| m.as_str().to_string()))
                        .unwrap_or_else(|| "(useFactory)".into())
                })
                .collect()
        };
        let kok_re = crate::kural::derle(&mk.kok_desen);
        let mut kokler: Vec<String> = Vec::new();
        // Kök modül uygulamanın kendi, test olmayan dosyalarında aranır (e2e testi kendi modülünü başlatabilir)
        for d in self.dosyalar.iter().filter(|d| !d.ek_paket && !test_dosyasi(d.yol.strip_prefix(&self.kok).unwrap_or(&d.yol))) {
            let metin = &d.metin;
            if !metin.contains("NestFactory") {
                continue;
            }
            for c in kok_re.captures_iter(metin) {
                if moduller.contains_key(&c[1]) && !kokler.contains(&c[1].to_string()) {
                    kokler.push(c[1].to_string());
                }
            }
        }
        let mut sayilar: Vec<(String, usize)> = Vec::new();
        for kok in &kokler {
            let (mut yigin, mut gorulen, mut adet) = (vec![kok.clone()], std::collections::HashSet::new(), 0usize);
            while let Some(m) = yigin.pop() {
                if !gorulen.insert(m.clone()) {
                    continue;
                }
                let Some((dek, md)) = moduller.get(m.as_str()) else { continue };
                let arg = dek.argumanlar.first().cloned().unwrap_or_default();
                let mut ctrl = Vec::new();
                ac(&dizi_cek(&arg, "controllers"), md, &mut ctrl);
                for c in ctrl {
                    adet += 1;
                    sonuc.entry(c).or_default().push(kok.clone());
                }
                kok_guardlari.entry(kok.clone()).or_default().extend(genel_guardlar(&arg, md));
                let mut imp = Vec::new();
                ac(&dizi_cek(&arg, "imports"), md, &mut imp);
                yigin.extend(imp.into_iter().filter(|i| moduller.contains_key(i.as_str())));
            }
            sayilar.push((kok.clone(), adet));
        }
        let ana = sayilar.into_iter().max_by_key(|(_, n)| *n).filter(|(_, n)| *n > 0).map(|(k, _)| k);
        // Kök bulunamadı: modül grafiği çözülemedi. Uygulamanın test olmayan modüllerindeki APP_GUARD'lar yine de
        // bildirilir ama bunlara dayanan giriş kararı kesin sayılmaz.
        let yedek_guardlar = if kokler.is_empty() {
            moduller
                .values()
                .filter(|(_, d)| !d.ek_paket && !test_dosyasi(d.yol.strip_prefix(&self.kok).unwrap_or(&d.yol)))
                .flat_map(|(dek, md)| genel_guardlar(dek.argumanlar.first().map(String::as_str).unwrap_or(""), md))
                .collect()
        } else {
            vec![]
        };
        for g in kok_guardlari.values_mut() {
            g.sort();
            g.dedup();
        }
        UygulamaHaritasi { ait: sonuc, ana, kok_guardlari, yedek_guardlar }
    }

    /// Adı verilen zod şeması, `kimden` dosyasının gördüğü tanım (içe aktarma → aynı dosya → tek aday). Aynı adlı
    /// birden fazla şema varken hangisinin kastedildiği belirsizse None: yanlış şemanın alanları kesin diye yazılmaz.
    fn zod_bul(&self, ad: &str, kimden: &'a Dosya) -> Option<(&'a Dosya, &'a crate::model::ZodSema)> {
        let adaylar = self.zod_dizini.get(ad)?;
        let dosyalar: Vec<usize> = adaylar.iter().map(|(di, _)| *di).collect();
        let (di, zi) = adaylar[self.dosya_sec(ad, &dosyalar, kimden)?];
        Some((&self.dosyalar[di], &self.dosyalar[di].zod_semalar[zi]))
    }

    /// Zod şemasının alanlarını türetmeleriyle birlikte çözer; `kimden` şemanın tanımlandığı dosyadır (içindeki adlar
    /// oradan çözülür). Bir parça çözülemezse (bilinmeyen ifade, bulunamayan ya da belirsiz atıf, döngü ya da derinlik
    /// sınırı) `eksik` işaretlenir: alan listesi kesin sayılmaz.
    fn zod_alanlari(&self, ifade: &crate::model::ZodIfade, kimden: &'a Dosya, derinlik: usize, eksik: &mut bool) -> BTreeMap<String, Alan> {
        use crate::model::ZodIfade as Z;
        let alan = |a: &crate::model::ZodAlan| Alan {
            tip: Some(a.tip.clone()),
            zorunlu: !a.istege_bagli,
            belirsiz: false,
            kurallar: a.kurallar.iter().map(|r| temiz(r)).collect(),
            satir: a.satir,
            kalitimdan: None,
            degerler: vec![],
            ic: None,
        };
        if derinlik > 8 {
            *eksik = true;
            return BTreeMap::new();
        }
        // Alan başka bir şemaya atıf yapıyor ve o şema `.optional()` ile bitiyorsa alan isteğe bağlıdır
        let nesne = |v: &Vec<(String, crate::model::ZodAlan)>, eksik: &mut bool| -> BTreeMap<String, Alan> {
            let mut m = BTreeMap::new();
            for (k, a) in v {
                if let Some(kaynak) = k.strip_prefix("...") {
                    m.extend(self.zod_ada_gore(kaynak, kimden, derinlik + 1, eksik));
                    continue;
                }
                let mut al = alan(a);
                if self.zod_bul(&a.tip, kimden).is_some_and(|(_, z)| z.istege_bagli) {
                    al.zorunlu = false;
                }
                self.zod_tip_coz(&mut al, a, kimden, derinlik);
                m.insert(k.clone(), al);
            }
            m
        };
        match ifade {
            Z::Nesne(v) => nesne(v, eksik),
            Z::Atif(ad) => self.zod_ada_gore(ad, kimden, derinlik + 1, eksik),
            Z::Genislet(b, v) => {
                let mut m = self.zod_alanlari(b, kimden, derinlik + 1, eksik);
                m.extend(nesne(v, eksik));
                m
            }
            Z::Birlestir(b, diger) => {
                let mut m = self.zod_alanlari(b, kimden, derinlik + 1, eksik);
                m.extend(self.zod_ada_gore(diger, kimden, derinlik + 1, eksik));
                m
            }
            Z::Kismi(b) => self
                .zod_alanlari(b, kimden, derinlik + 1, eksik)
                .into_iter()
                .map(|(k, mut a)| {
                    a.zorunlu = false;
                    (k, a)
                })
                .collect(),
            Z::Sec(b, anahtarlar) => self.zod_alanlari(b, kimden, derinlik + 1, eksik).into_iter().filter(|(k, _)| anahtarlar.contains(k)).collect(),
            Z::Haric(b, anahtarlar) => self.zod_alanlari(b, kimden, derinlik + 1, eksik).into_iter().filter(|(k, _)| !anahtarlar.contains(k)).collect(),
            Z::Dizi(oge) => self.zod_alanlari(oge, kimden, derinlik + 1, eksik),
            Z::Bilinmiyor => {
                *eksik = true;
                BTreeMap::new()
            }
        }
    }

    /// Alanın tipi bir zod sabitinin adıysa (`tarih: isoDatetimeToDate`) tipi ve kuralları sabitin tanımından okur;
    /// sabit nesne şemasıysa tip `object` olur. Enum değerleri liste ya da TypeScript enum'u tam çözülebiliyorsa yazılır.
    /// Tanım bulunamazsa tip olduğu gibi kalır (bilinmiyor): addan tip çıkarılmaz.
    fn zod_tip_coz(&self, al: &mut Alan, a: &crate::model::ZodAlan, kimden: &'a Dosya, derinlik: usize) {
        let mut ham = a.degerler_ham.clone();
        let mut tip = a.tip.clone();
        // Adların çözüldüğü dosya: sabit zinciri izlendikçe tanımın bulunduğu dosyaya geçer
        let mut yer = kimden;
        // İç içe nesne: şema adının alanları eksiksiz çözülürse (en fazla 3 kat)
        let ic_alanlar = |ad: &str, yer: &'a Dosya| -> Option<BTreeMap<String, Alan>> {
            if derinlik >= 3 {
                return None;
            }
            let mut eksik = false;
            let m = self.zod_ada_gore(ad, yer, derinlik + 1, &mut eksik);
            (!eksik && !m.is_empty()).then_some(m)
        };
        for _ in 0..8 {
            let Some((zd, z)) = self.zod_bul(&tip, yer) else { break };
            match &z.skaler {
                Some(s) if s.tip != tip => {
                    // Sabitin kuralları önce, alanın kendi eklediği kurallar sonra
                    let mut kurallar: Vec<String> = s.kurallar.iter().map(|r| temiz(r)).collect();
                    kurallar.append(&mut al.kurallar);
                    al.kurallar = kurallar;
                    if ham.is_none() {
                        ham = s.degerler_ham.clone();
                    }
                    tip = s.tip.clone();
                    yer = zd;
                }
                Some(_) => break,
                None => {
                    al.ic = ic_alanlar(&tip, yer);
                    tip = "object".into();
                    break;
                }
            }
        }
        // z.array(SemaAdi): öğe bir nesne şemasıysa öğenin alanları
        if tip == "array" {
            let oge = al.kurallar.iter().find_map(|r| r.strip_prefix("of(").and_then(|r| r.strip_suffix(')'))).map(|x| x.trim().to_string());
            if let Some(o) = oge.filter(|o| o.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')) {
                if self.zod_bul(&o, yer).is_some_and(|(_, z)| z.skaler.is_none()) {
                    al.ic = ic_alanlar(&o, yer);
                }
            }
        }
        al.tip = Some(tip);
        if let Some(h) = ham {
            let h = h.trim();
            let degerler = if h.starts_with('[') {
                let v = self.degerler(h, Some(yer));
                let tam = dizi_ogeleri(h).unwrap_or_default().iter().zip(&v).all(|(ham, d)| ham.trim().starts_with(['\'', '"', '`']) || ham.trim() != d);
                if tam {
                    v
                } else {
                    vec![]
                }
            } else {
                self.enum_degerleri(h.trim_start_matches("Object.values(").trim_end_matches(')'), Some(yer)).unwrap_or_default()
            };
            if !degerler.is_empty() {
                al.degerler = degerler;
            }
        }
    }

    /// Adı verilen zod şeması (atamalar izlenerek) bir dizi mi: `z.array(z.object({...}))`
    fn zod_dizi_mi(&self, ad: &str, kimden: &'a Dosya) -> bool {
        let (mut ad, mut yer) = (ad.to_string(), kimden);
        for _ in 0..8 {
            let Some((zd, z)) = self.zod_bul(&ad, yer) else { return false };
            match &z.ifade {
                crate::model::ZodIfade::Dizi(_) => return true,
                crate::model::ZodIfade::Atif(n) => (ad, yer) = (n.clone(), zd),
                _ => return false,
            }
        }
        false
    }

    /// Adı verilen zod şemasının alanları, `kimden` dosyasının gördüğü tanımdan; bulunamaz ya da belirsizse `eksik`
    fn zod_ada_gore(&self, ad: &str, kimden: &'a Dosya, derinlik: usize, eksik: &mut bool) -> BTreeMap<String, Alan> {
        match self.zod_bul(ad, kimden) {
            Some((zd, z)) => {
                if !z.nesne_kurallari.is_empty() {
                    let not = format!(
                        "nesne düzeyi kural: {} şemasında {} (alanlar arası koşul ya da dönüşüm) önizlemede çalıştırılmaz",
                        temiz(&z.ad),
                        z.nesne_kurallari.iter().map(|k| format!(".{k}()")).collect::<Vec<_>>().join(", ")
                    );
                    let mut notlar = self.nesne_notlari.borrow_mut();
                    if !notlar.contains(&not) {
                        notlar.push(not);
                    }
                }
                self.zod_alanlari(&z.ifade, zd, derinlik, eksik)
            }
            None => {
                *eksik = true;
                BTreeMap::new()
            }
        }
    }

    /// Sınıfı bul: önce içe aktarma yolu, sonra aynı dosya, sonra adı tek olan sınıf.
    /// Yanıt şeması: `@ApiResponse` / `@ApiOkResponse` / `@ApiCreatedResponse({ type: X })` açık beyanı (uygulamanın
    /// kendi OpenAPI'sinin kaynağı; kesin), yoksa metodun dönüş tipi (`Promise<X>`, `X[]`; TypeScript tipi çalışma anında
    /// silinir, gönderilen nesne farklı olabilir: yorum). Tip bir sınıfa çözülmezse (arayüz, any, void) şema yok
    fn yanit(&self, m: &crate::model::Metot, d: &'a Dosya, yontem: &str) -> Option<crate::adaptor::Yanit> {
        static TIP: std::sync::OnceLock<(Regex, Regex)> = std::sync::OnceLock::new();
        let (tip_re, durum_re) =
            TIP.get_or_init(|| (Regex::new(r"type:\s*(\[)?\s*([A-Za-z_$][\w$.]*)").expect("desen"), Regex::new(r"status:\s*(?:(\d{3})|HttpStatus\.([A-Z_]+))").expect("desen")));
        let http = |a: &str| match a {
            "OK" => Some(200),
            "CREATED" => Some(201),
            "ACCEPTED" => Some(202),
            "NO_CONTENT" => Some(204),
            _ => None,
        };
        let varsayilan = m
            .dekoratorler
            .iter()
            .find(|x| x.ad == "HttpCode")
            .and_then(|x| x.argumanlar.first())
            .and_then(|a| a.trim().parse::<u16>().ok().or_else(|| a.trim().strip_prefix("HttpStatus.").and_then(http)))
            .or(Some(if yontem == "POST" { 201 } else { 200 }));
        for x in m.dekoratorler.iter().filter(|x| matches!(x.ad.as_str(), "ApiResponse" | "ApiOkResponse" | "ApiCreatedResponse" | "ApiAcceptedResponse")) {
            let arg = x.argumanlar.join(",");
            let durum = match durum_re.captures(&arg) {
                Some(c) => c.get(1).and_then(|s| s.as_str().parse::<u16>().ok()).or_else(|| c.get(2).and_then(|s| http(s.as_str()))),
                None if x.ad == "ApiOkResponse" => Some(200),
                None if x.ad == "ApiCreatedResponse" => Some(201),
                None if x.ad == "ApiAcceptedResponse" => Some(202),
                None => varsayilan,
            };
            if !durum.is_some_and(|s| (200..300).contains(&s)) {
                continue;
            }
            let Some(c) = tip_re.captures(&arg) else { continue };
            let ad = c[2].rsplit('.').next().unwrap_or(&c[2]).to_string();
            let mut sema = self.yanit_semasi(&ad, d);
            if sema.bulunamadi {
                return None;
            }
            sema.dizi = c.get(1).is_some() || arg.contains("isArray: true");
            return Some(crate::adaptor::Yanit { kaynak: "ApiResponse".into(), kesin: true, durum, sema });
        }
        let mut t = m.donus_tipi.as_deref()?.trim().to_string();
        for sarmal in ["Promise<", "Observable<"] {
            if let Some(ic) = t.strip_prefix(sarmal).and_then(|x| x.strip_suffix('>')) {
                t = ic.trim().to_string();
            }
        }
        let (ad, dizi) = match t.strip_suffix("[]").or_else(|| t.strip_prefix("Array<").and_then(|x| x.strip_suffix('>'))) {
            Some(ic) => (ic.trim().to_string(), true),
            None => (t.clone(), false),
        };
        if ad.is_empty() || !ad.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') || self.sinif_bul(&ad, d).is_none() {
            return None;
        }
        let mut sema = self.yanit_semasi(&ad, d);
        if sema.bulunamadi {
            return None;
        }
        sema.dizi = dizi;
        Some(crate::adaptor::Yanit { kaynak: "donus_tipi".into(), kesin: false, durum: varsayilan, sema })
    }

    /// Yanıt şeması, önbellekli
    fn yanit_semasi(&self, ad: &str, d: &'a Dosya) -> Sema {
        let anahtar = (ad.to_string(), d.yol.clone());
        if let Some(s) = self.yanit_semalari.borrow().get(&anahtar) {
            return s.clone();
        }
        let s = self.sema(ad, d, 0);
        self.yanit_semalari.borrow_mut().insert(anahtar, s.clone());
        s
    }

    fn sinif_bul(&self, ad: &str, kimden: &Dosya) -> Option<(&'a Dosya, &'a Sinif)> {
        // export { Asil as TakmaAd } ile yeniden adlandırılmış sınıf: zincir izlenir. Döngü ya da çok uzun zincir
        // (kötü niyetli depo) çözülmez: sınıf "bulunamadı" kalır, Sezgi çökmez.
        let mut ad = ad;
        let mut gorulen: Vec<&str> = Vec::new();
        let adaylar = loop {
            if let Some(a) = self.sinif_dizini.get(ad) {
                break a;
            }
            if gorulen.contains(&ad) || gorulen.len() >= EN_UZUN_ZINCIR {
                return None;
            }
            gorulen.push(ad);
            ad = self.dosyalar.iter().find_map(|d| d.takma_adlar.get(ad))?.as_str();
        };
        // Aynı adlı sınıflar arasından içe aktarma → aynı dosya → tek aday → takma adlı yol → kendi dosyası; belirsizse
        // tahmin etmek yerine "bulunamadı"
        let dosyalar: Vec<usize> = adaylar.iter().map(|(di, _)| *di).collect();
        let (di, si) = adaylar[self.dosya_sec(ad, &dosyalar, kimden)?];
        Some((&self.dosyalar[di], &self.dosyalar[di].siniflar[si]))
    }

    fn sema(&self, ad: &str, kimden: &Dosya, derinlik: usize) -> Sema {
        let Some((d, s)) = (if derinlik < 5 { self.sinif_bul(ad, kimden) } else { None }) else {
            // `type Girdi = A | B`: her üye bir sınıfsa adaylar verilir; hangisinin geçerli olduğu kesin değildir
            let adaylar: Vec<Sema> = match self.dosyalar.iter().find_map(|d| d.tip_birlesimleri.get(ad)).filter(|_| derinlik < 3) {
                Some(uyeler) => {
                    let s: Vec<Sema> = uyeler.iter().map(|u| self.sema(u, kimden, derinlik + 1)).collect();
                    if s.iter().all(|x| !x.bulunamadi && !x.alanlar.is_empty()) {
                        s
                    } else {
                        vec![]
                    }
                }
                None => vec![],
            };
            return Sema { adaylar, dizi: false, dto: ad.into(), dosya: None, bulunamadi: true, kalitim: None, alanlar: BTreeMap::new() };
        };
        let mut alanlar: BTreeMap<String, Alan> = BTreeMap::new();
        // Zod: class X extends createZodDto(XSchema) {}
        let zod = s.kalitim.as_deref().and_then(|kal| self.zod_dto_desen.as_ref().and_then(|re| re.captures(kal)).map(|c| c[1].to_string()));
        if let Some(sema_adi) = zod {
            let mut eksik = false;
            let alanlar = self.zod_ada_gore(&sema_adi, d, 0, &mut eksik);
            // Şema bulunamadıysa ya da alanları çözülemediyse (ör. çalışma anında türetilen şema) dürüstçe bildir
            let bulunamadi = eksik || alanlar.is_empty();
            let dizi = self.zod_dizi_mi(&sema_adi, d);
            return Sema { adaylar: vec![], dizi, dto: ad.into(), dosya: Some(goreli(&self.kok, &d.yol)), bulunamadi, kalitim: s.kalitim.clone(), alanlar };
        }
        // Üst şemalardan biri çözülemezse (bulunamadı, döngü, derinlik sınırı, desteklenmeyen türetme) bu şema da kesin değildir
        let mut bulunamadi = false;
        if let Some(kal) = &s.kalitim {
            match turetme_ayristir(kal) {
                Some(t) => alanlar.extend(self.turetme_alanlari(&t, d, derinlik, &mut bulunamadi)),
                None => bulunamadi = true,
            }
        }
        for o in &s.ozellikler {
            // Zorunluluk dekoratörlerden çıkar, TypeScript'teki `?` işaretinden değil: class-validator `?` görmez,
            // IsOptional yoksa undefined değeri reddeder. Doğrulayıcısı olmayan alanın varlığı hiç denetlenmez.
            let anlamlar: Vec<DekAnlam> = o.dekoratorler.iter().map(|x| self.dek_anlami(&x.ad, d, 0)).collect();
            let (zorunlu, belirsiz) = if anlamlar.contains(&DekAnlam::Istege) {
                (false, false)
            } else if anlamlar.iter().any(|a| matches!(a, DekAnlam::KosulluIstege | DekAnlam::Bilinmeyen)) || !anlamlar.contains(&DekAnlam::Dogrulayici) {
                // Tahmin: `?` varsa isteğe bağlı; ama kesin değil
                (!o.opsiyonel_isaret, true)
            } else {
                (true, false)
            };
            let kurallar =
                o.dekoratorler.iter().filter(|x| !self.k.dto.kural_olmayan_dekorator.contains(&x.ad)).map(|x| temiz(&format!("{}({})", x.ad, x.argumanlar.join(", ")))).collect();
            alanlar.insert(
                o.ad.clone(),
                Alan {
                    tip: o.tip.clone(),
                    zorunlu,
                    belirsiz,
                    kurallar,
                    satir: o.satir,
                    kalitimdan: None,
                    degerler: self.izinli_degerler(&o.dekoratorler, d),
                    ic: self.ic_sinif(o, d, derinlik),
                },
            );
        }
        Sema { adaylar: vec![], dizi: false, dto: ad.into(), dosya: Some(goreli(&self.kok, &d.yol)), bulunamadi, kalitim: s.kalitim.clone(), alanlar }
    }

    /// @ValidateNested ile doğrulanan sınıf tipli alanın (Kisi ya da Kisi[]) iç alanları; sınıf eksiksiz çözülürse, en fazla 3 kat.
    /// ValidateNested yoksa iç nesne doğrulanmaz: iç yapı kesin değildir, verilmez.
    fn ic_sinif(&self, o: &crate::model::Ozellik, d: &'a Dosya, derinlik: usize) -> Option<BTreeMap<String, Alan>> {
        if derinlik >= 3 || !o.dekoratorler.iter().any(|x| x.ad == "ValidateNested") {
            return None;
        }
        let t = o.tip.as_deref()?.trim();
        let ad = t.strip_suffix("[]").or_else(|| t.strip_prefix("Array<").and_then(|x| x.strip_suffix('>'))).unwrap_or(t).trim();
        if ad.is_empty() || !ad.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        let s = self.sema(ad, d, derinlik + 1);
        (!s.bulunamadi && !s.alanlar.is_empty()).then_some(s.alanlar)
    }

    /// Türetme ifadesinin alanları: sınıf, PartialType / PickType / OmitType / IntersectionType ve iç içe birleşimleri.
    /// Desteklenmeyen fonksiyon, değişkenle verilen anahtar listesi ya da çözülemeyen sınıf `bulunamadi` işaretler.
    fn turetme_alanlari(&self, t: &Turetme, d: &'a Dosya, derinlik: usize, bulunamadi: &mut bool) -> BTreeMap<String, Alan> {
        let dto = &self.k.dto;
        let var = |l: &Vec<String>, f: &str| l.iter().any(|x| x == f);
        match t {
            Turetme::Sinif(ad) => {
                let s = self.sema(ad, d, derinlik + 1);
                *bulunamadi |= s.bulunamadi;
                s.alanlar
                    .into_iter()
                    .map(|(k, mut v)| {
                        v.kalitimdan = Some(ad.clone());
                        (k, v)
                    })
                    .collect()
            }
            Turetme::Cagri(f, args) if var(&dto.tumu_istege_bagli, f) => match args.first() {
                // PartialType: bütün alanlar kesin olarak isteğe bağlı (IsOptional eklenir)
                Some(ilk) => self
                    .turetme_alanlari(ilk, d, derinlik, bulunamadi)
                    .into_iter()
                    .map(|(k, mut v)| {
                        v.zorunlu = false;
                        v.belirsiz = false;
                        (k, v)
                    })
                    .collect(),
                None => {
                    *bulunamadi = true;
                    BTreeMap::new()
                }
            },
            Turetme::Cagri(f, args) if var(&dto.secici, f) || var(&dto.haric, f) => {
                let secici = var(&dto.secici, f);
                let (Some(ilk), Some(Turetme::Anahtarlar(Some(anahtarlar)))) = (args.first(), args.get(1)) else {
                    // Anahtar listesi metin değil (değişken, yayma): hangi alanların kaldığı bilinemez
                    *bulunamadi = true;
                    return BTreeMap::new();
                };
                self.turetme_alanlari(ilk, d, derinlik, bulunamadi).into_iter().filter(|(k, _)| anahtarlar.contains(k) == secici).collect()
            }
            Turetme::Cagri(f, args) if var(&dto.birlesim, f) && args.len() >= 2 => {
                let mut m = BTreeMap::new();
                for a in args {
                    m.extend(self.turetme_alanlari(a, d, derinlik, bulunamadi));
                }
                m
            }
            // Tanınmayan türetme fonksiyonu (mixin, özel yardımcı) ya da beklenmeyen argüman
            _ => {
                *bulunamadi = true;
                BTreeMap::new()
            }
        }
    }

    /// @IsEnum(Durum) -> Durum'un değerleri; @IsIn(['a', Sabit.B]) -> [a, b]. Bir öğe çözülemezse liste boş kalır.
    fn izinli_degerler(&self, ds: &[Dekorator], dosya: &'a Dosya) -> Vec<String> {
        for x in ds {
            let Some(arg) = x.argumanlar.first() else { continue };
            if self.k.dto.enum_dekorator.contains(&x.ad) {
                let ad = arg.trim().trim_start_matches("Object.values(").trim_end_matches(')');
                if let Some(v) = self.enum_degerleri(ad, Some(dosya)) {
                    return v;
                }
            }
            // @IsIn(SABIT) / @IsIn(SABIT as unknown as string[]): sabit dizinin öğeleri; hepsi metin değilse verilmez
            let ad = arg.split(" as ").next().unwrap_or("").trim();
            if self.k.dto.liste_dekorator.contains(&x.ad) && !ad.is_empty() && ad.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
                if let Some(ogeler) = self.dizi_dizini.get(ad).and_then(|a| self.aday_sec(ad, a, Some(dosya), |x, y| x == y)) {
                    if !ogeler.is_empty() && ogeler.iter().all(|o| o.trim().starts_with(['\'', '"']) && o.trim().len() >= 2) {
                        return ogeler.iter().map(|o| tirnaksiz(o)).collect();
                    }
                }
            }
            if self.k.dto.liste_dekorator.contains(&x.ad) && arg.trim_start().starts_with('[') {
                let v = self.degerler(arg, Some(dosya));
                // Çözülemeyen öğe (ör. değişken) varsa liste eksik olabilir: hiç verme
                let tam = dizi_ogeleri(arg).unwrap_or_default().iter().zip(&v).all(|(ham, d)| ham.trim().starts_with(['\'', '"', '`']) || ham.trim() != d);
                if tam && !v.is_empty() {
                    return v;
                }
            }
        }
        vec![]
    }

    fn girdiler(&self, ps: &[crate::model::Parametre], dek_ad: &str, dosya: &Dosya, sema_coz: bool) -> Vec<Girdi> {
        ps.iter()
            .filter_map(|p| {
                let d = p.dekoratorler.iter().find(|d| d.ad == dek_ad)?;
                // Girdinin adı yalnızca metin argümandır: @Body('alan'). @Body(new ValidationPipe()) ya da @Body(Pipe) ad değildir
                let ad = d.argumanlar.first().filter(|a| a.trim_start().starts_with(['\'', '"', '`'])).map(|a| tirnaksiz(a)).filter(|a| !a.is_empty());
                let sema = match (&ad, &p.tip) {
                    (None, Some(t)) if sema_coz && self.tip_desen.is_match(t) => Some(self.sema(t, dosya, 0)),
                    _ => None,
                };
                Some(Girdi { ad, tip: p.tip.clone(), sema })
            })
            .collect()
    }

    pub fn kapilar(&self) -> (Vec<Kapi>, Value) {
        let onek = self.genel_onek();
        // Kimlik guard'ı olduğu anlaşılamayan genel guard (ör. SessionGuard) ya da useGlobalGuards(...) ile kodla kurulan guard:
        // bunlar varken "guard yok, açık" sonucu yalnızca çıkarımdır ve önizleme GEÇER demez
        let kimlik_disi = self.k.yetki.kimlik_disi_guard_desen.as_deref().map(crate::kural::derle);
        // AuthGuard('strateji'): Some(true) oturum doğrulayan strateji, Some(false) giriş başlatan ya da bilinmeyen; None strateji guard'ı değil
        let strateji_re = self.k.yetki.strateji_guard_desen.as_deref().map(crate::kural::derle);
        let oturum_re = self.k.yetki.oturum_strateji_desen.as_deref().map(crate::kural::derle);
        let strateji = |g: &str| -> Option<bool> {
            let c = strateji_re.as_ref()?.captures(g.trim())?;
            Some(c.get(1).is_some_and(|s| oturum_re.as_ref().is_some_and(|r| r.is_match(s.as_str()))))
        };
        let tanidik = |g: &String| strateji(g).unwrap_or(true) && (self.kimlik_desen.is_match(g) || kimlik_disi.as_ref().map(|r| r.is_match(g)).unwrap_or(false));
        let kurulum = self.k.yetki.genel_guard_kurulum_desen.as_deref().map(crate::kural::derle);
        let kurulum_var = kurulum
            .as_ref()
            .is_some_and(|r| self.dosyalar.iter().filter(|d| !d.ek_paket && !test_dosyasi(d.yol.strip_prefix(&self.kok).unwrap_or(&d.yol))).any(|d| r.is_match(&d.metin)));
        let harita = self.uygulama_haritasi();
        let (ait, ana_kok) = (&harita.ait, &harita.ana);
        // Bir controller'a uygulanan genel guard'lar ve bunun kesin olup olmadığı: controller kökten ulaşılan modül
        // grafiğindeyse kesin; grafik çözülemediyse (kök yok) ya da controller grafikte değilse yalnızca tahmin
        let genel_icin = |ctrl: &str| -> (Vec<String>, bool) {
            let birlesim = |kokler: &mut dyn Iterator<Item = &String>| -> Vec<String> {
                let mut g: Vec<String> = kokler.filter_map(|k| harita.kok_guardlari.get(k)).flatten().cloned().collect();
                g.sort();
                g.dedup();
                g
            };
            match ait.get(ctrl) {
                Some(kokler) => (birlesim(&mut kokler.iter()), true),
                None if !harita.kok_guardlari.is_empty() => (birlesim(&mut harita.kok_guardlari.keys()), false),
                None => (harita.yedek_guardlar.clone(), false),
            }
        };
        let genel_ozet: Vec<String> = ana_kok.as_ref().and_then(|a| harita.kok_guardlari.get(a)).cloned().unwrap_or_else(|| harita.yedek_guardlar.clone());
        let mut kapilar = Vec::new();
        // Önizlemenin sınayamadığı kurallar (nesne düzeyi doğrulama, dosya yüklemesi), kapının "dosya:satır" anahtarıyla
        let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
        // Uygulamanın kendi işlem kimliği (OpenAPI operationId), kapının "dosya:satır" anahtarıyla
        let mut islem_kimlikleri: BTreeMap<String, String> = BTreeMap::new();
        let operation_id = crate::kural::derle(r#"operationId:\s*['"`]([^'"`]+)['"`]"#);
        let kimlik_fabrikasi = self.dosyalar.iter().filter(|d| !d.ek_paket).find_map(|d| KimlikFabrikasi::oku(&d.metin));
        let dosya_desen = self.k.parametre.dosya_desen.as_deref().map(crate::kural::derle);
        let ad_var = |ds: &[Dekorator], adlar: &[String]| ds.iter().any(|d| adlar.contains(&d.ad));
        // Controller dosyası: adı eke uyan (`.controller.ts`) ya da `@Controller` dekoratörlü sınıf taşıyan dosya. Ek yalnızca
        // bir alışkanlıktır: eke uymayan dekoratörlü sınıf da uç sunar (test dosyaları sayılmaz)
        let kontrolculer: Vec<&Dosya> = self
            .dosyalar
            .iter()
            .filter(|d| !d.ek_paket)
            .filter(|d| {
                d.yol.to_string_lossy().ends_with(&self.k.kontrolcu.dosya_eki)
                    || (!test_dosyasi(d.yol.strip_prefix(&self.kok).unwrap_or(&d.yol))
                        && d.siniflar.iter().any(|s| s.dekoratorler.iter().any(|x| x.ad == self.k.kontrolcu.dekorator)))
            })
            .collect();
        // İşaretle açılan yetki modeli: uygulama kimlik dekoratörünü (ör. @Authenticated) kullanıyor mu?
        let isaretli_mod = !self.k.yetki.kimlik_dekorator.is_empty()
            && kontrolculer
                .iter()
                .flat_map(|d| d.siniflar.iter())
                .any(|s| ad_var(&s.dekoratorler, &self.k.yetki.kimlik_dekorator) || s.metotlar.iter().any(|m| ad_var(&m.dekoratorler, &self.k.yetki.kimlik_dekorator)));
        for d in kontrolculer {
            for s in &d.siniflar {
                let Some(ctrl) = s.dekoratorler.iter().find(|x| x.ad == self.k.kontrolcu.dekorator) else { continue };
                // Controller adresi tek değer ya da dizi olabilir: @Controller(['a', 'b']), @Controller({ path: ['a', 'b'] })
                let ctrl_yollari: Vec<String> = ctrl
                    .argumanlar
                    .first()
                    .map(|a| {
                        if a.trim_start().starts_with('{') {
                            self.yol_desen.captures(a).map(|c| self.degerler(&c[1], Some(d))).unwrap_or_default()
                        } else {
                            self.degerler(a, Some(d))
                        }
                    })
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| vec![String::new()]);
                let guard_al = |ds: &[Dekorator]| -> Vec<String> { ds.iter().filter(|x| x.ad == self.k.yetki.guard_dekorator).flat_map(|x| x.argumanlar.clone()).collect() };
                // Roller tek değer, birden fazla argüman ya da dizi olabilir: @Roles('a', Rol.B), @Roles([Rol.A, 'b']).
                // (rol, çözüldü mü): sabit / enum değeri bulunamayan rol (ör. sayı değerli enum) kesin değildir
                let rol_al = |ds: &[Dekorator]| -> Vec<(String, bool)> {
                    ds.iter()
                        .filter(|x| self.rol_desen.is_match(&x.ad))
                        .flat_map(|x| x.argumanlar.iter().flat_map(|a| dizi_ogeleri(a).unwrap_or_else(|| vec![a.clone()])))
                        .map(|a| {
                            let cozuldu = self.deger_cozulur(&a, Some(d));
                            (self.deger(&a, Some(d)).trim_start_matches(&self.k.yetki.rol_oneki_sil).to_string(), cozuldu)
                        })
                        .collect()
                };
                // @Authenticated({ permission: Permission.X, admin: true }) -> izin değeri ve "admin"
                let izin_al = |ds: &[Dekorator]| -> Vec<String> {
                    let mut v = Vec::new();
                    for x in ds.iter().filter(|x| self.k.yetki.kimlik_dekorator.contains(&x.ad)) {
                        for a in &x.argumanlar {
                            if let Some(c) = self.izin_desen.as_ref().and_then(|re| re.captures(a)) {
                                v.push(self.deger(&c[1], Some(d)));
                            }
                            if self.yonetici_desen.as_ref().map(|re| re.is_match(a)).unwrap_or(false) {
                                v.push("admin".into());
                            }
                        }
                    }
                    v
                };
                // Bileşik dekoratörler (@RequiresScope() -> applyDecorators(UseGuards(..))) açılır
                let sd = self.acilmis(&s.dekoratorler, d);
                for m in &s.metotlar {
                    let Some((dek, yontem)) = m.dekoratorler.iter().find_map(|x| self.k.rota.get(&x.ad).map(|y| (x, y))) else { continue };
                    let md = self.acilmis(&m.dekoratorler, d);
                    let mut guardlar = guard_al(&sd);
                    guardlar.extend(guard_al(&md));
                    guardlar.dedup();
                    let (sinif_rolleri, metot_rolleri) = (rol_al(&sd), rol_al(&md));
                    // Sınıf ve metot düzeyinde birlikte rol varsa birleşim mi geçersiz kılma mı olduğu uygulamanın
                    // guard'ına bağlıdır (Reflector getAllAndMerge / getAllAndOverride): rol kararı çıkarımdır
                    let roller_kaniti =
                        (sinif_rolleri.iter().chain(&metot_rolleri).any(|(_, c)| !c) || (!sinif_rolleri.is_empty() && !metot_rolleri.is_empty())).then(|| "varsayim".to_string());
                    let mut roller: Vec<String> = sinif_rolleri.into_iter().chain(metot_rolleri).map(|(r, _)| r).collect();
                    roller.extend(izin_al(&sd));
                    roller.extend(izin_al(&md));
                    roller.dedup();
                    let acik_alan = |ds: &[Dekorator]| {
                        ds.iter()
                            .filter(|x| self.k.yetki.kimlik_dekorator.contains(&x.ad))
                            .any(|x| x.argumanlar.iter().any(|a| self.acik_alan_desen.as_ref().map(|re| re.is_match(a)).unwrap_or(false)))
                    };
                    let (genel, genel_kesin) = genel_icin(&s.ad);
                    let genel_bilinmeyen = genel.iter().any(|g| !tanidik(g)) || kurulum_var;
                    let istege_bagli = guardlar.iter().any(|g| self.istege_bagli_desen.is_match(g));
                    let giris_guardi = |g: &&String| strateji(g).unwrap_or(true) && self.kimlik_desen.is_match(g) && !self.istege_bagli_desen.is_match(g);
                    let yerel_kimlik = guardlar.iter().any(|g| giris_guardi(&g));
                    let kimlik = yerel_kimlik || genel.iter().any(|g| giris_guardi(&g));
                    // Herkese açıklık işaretin adından değil tanımından çıkar: işaret SetMetadata(ANAHTAR, true) yazar,
                    // giriş guard'ı aynı ANAHTAR'ı Reflector ile okur. Her giriş guard'ı için: Some(true) işaretle kesin
                    // atlanır, Some(false) kesin atlanmaz (işareti okumuyor), None bilinmiyor (işaretin ya da guard'ın tanımı yok).
                    // Giriş guard'larının Reflector ile okuduğu anahtarlar (guard sınıfı bu depoda değilse None)
                    let okunanlar: Vec<Option<Vec<String>>> = guardlar.iter().chain(genel.iter()).filter(giris_guardi).map(|g| self.guard_anahtarlari(g, d)).collect();
                    // İlgili işaret: adı herkese açıklık işareti olan ya da anahtarı bir giriş guard'ınca okunan.
                    // Kimlikle ilgisiz SetMetadata(K, true) işaretleri (ör. @AllowDuringImpersonation) karara katılmaz.
                    let isaretler: Vec<Option<String>> = self
                        .acik_isaretleri(&sd, d)
                        .into_iter()
                        .chain(self.acik_isaretleri(&md, d))
                        .filter(|(a, adli)| *adli || a.as_ref().is_some_and(|k| okunanlar.iter().flatten().any(|o| o.contains(k))))
                        .map(|(a, _)| a)
                        .collect();
                    let durumlar: Vec<Option<bool>> = okunanlar
                        .iter()
                        .map(|okunan| {
                            if isaretler.is_empty() {
                                return Some(false);
                            }
                            let okunan = okunan.as_ref()?;
                            if isaretler.iter().flatten().any(|a| okunan.contains(a)) {
                                Some(true)
                            } else if isaretler.iter().all(Option::is_some) {
                                Some(false)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let herkese_acik = (!isaretler.is_empty() && !durumlar.contains(&Some(false))) || acik_alan(&m.dekoratorler);
                    // Kesin: en az bir giriş guard'ı var ve hepsi işareti okuyor (yalnızca işaret olup guard yoksa açıklık yokluktan gelir)
                    let herkese_acik_kesin = !isaretler.is_empty() && !durumlar.is_empty() && durumlar.iter().all(|x| *x == Some(true));
                    let isaretli = ad_var(&s.dekoratorler, &self.k.yetki.kimlik_dekorator) || ad_var(&m.dekoratorler, &self.k.yetki.kimlik_dekorator);
                    // NestJS guard sırası: genel, controller, rota; hepsi geçmelidir (docs.nestjs.com/faq/request-lifecycle).
                    // Kesin çalışan (işaretle atlanmayan) bir giriş guard'ı varsa rotadaki isteğe bağlı guard girişi açmaz.
                    // İşaretli modda (@Authenticated) genel guard yalnızca işaretli kapılarda giriş ister: bu kural uygulanmaz.
                    let calisan_giris = !isaretli_mod && durumlar.contains(&Some(false));
                    let giris = if calisan_giris {
                        "gerekli"
                    } else if istege_bagli {
                        "istege_bagli"
                    } else if herkese_acik {
                        "herkese_acik"
                    } else if isaretli_mod {
                        // Giriş, endpoint'teki kimlik dekoratörüyle ya da yerel guard'la açılır
                        if isaretli || yerel_kimlik {
                            "gerekli"
                        } else {
                            "herkese_acik"
                        }
                    } else if kimlik {
                        "gerekli"
                    } else {
                        "herkese_acik"
                    };
                    // Kararın kanıtı: açık işaret ya da tanınan guard "kesin"; hiçbir işaret yoksa "varsayim".
                    // Varsayımla açık sayılan kapıda tanınmayan bir guard varsa (ör. PermissionsGuard) bu ayrıca bildirilir.
                    // "Giriş gerekli" yalnızca genel guard'dan geliyorsa ve modül grafiği o guard'ı kesin göstermiyorsa çıkarımdır
                    let gerekli_kesin = giris == "gerekli" && (yerel_kimlik || isaretli || genel_kesin);
                    let istege_kesin = isaretli_mod || durumlar.iter().all(|x| *x == Some(true));
                    let giris_kaniti = if (giris == "istege_bagli" && istege_kesin) || (giris == "herkese_acik" && herkese_acik_kesin) || gerekli_kesin {
                        "kesin"
                    } else if giris == "herkese_acik" && (!isaretler.is_empty() || acik_alan(&m.dekoratorler)) {
                        // Herkese açıklık işareti var ama tanımı ya da giriş guard'ının onu okuduğu doğrulanamadı
                        "varsayim_isaret"
                    } else if genel_bilinmeyen || guardlar.iter().any(|g| !tanidik(g)) {
                        "varsayim_bilinmeyen_guard"
                    } else {
                        "varsayim"
                    };
                    let ozet = m
                        .dekoratorler
                        .iter()
                        .find(|x| self.k.ozet.dekorator.contains(&x.ad))
                        .and_then(|x| x.argumanlar.first())
                        .and_then(|a| self.ozet_desen.captures(a).map(|c| temiz(&c[1])));
                    // API sürümü: metottaki @Version(...) ya da controller'daki { version: ... }
                    let surum = m
                        .dekoratorler
                        .iter()
                        .find(|x| x.ad == "Version")
                        .and_then(|x| x.argumanlar.first())
                        .map(|a| self.degerler(a, Some(d)))
                        .or_else(|| ctrl.argumanlar.first().and_then(|a| self.surum_desen.captures(a)).map(|c| self.degerler(&c[1], Some(d))))
                        .map(|v| v.join(","));
                    // Kullanımdan kaldırılma: @Deprecated() ya da özet dekoratöründe deprecated(...) / deprecated: true
                    let (kullanimdan_kalkmis, yerine) = match (&self.k.kullanimdan_kalkma, &self.kalkma_desenleri) {
                        (Some(kk), Some((desen, yerine_re))) => {
                            let ozet_args: Vec<&String> = m.dekoratorler.iter().filter(|x| self.k.ozet.dekorator.contains(&x.ad)).flat_map(|x| x.argumanlar.iter()).collect();
                            let kalkmis = ad_var(&m.dekoratorler, &kk.dekorator) || ozet_args.iter().any(|a| desen.is_match(a) || a.contains("deprecated: true"));
                            let yerine = ozet_args.iter().find_map(|a| yerine_re.captures(a).map(|c| c[1].to_string()));
                            (kalkmis, if kalkmis { yerine } else { None })
                        }
                        _ => (false, None),
                    };
                    let veri = self.veri_bagi(d, s, m);
                    let yanit = self.yanit(m, d, yontem);
                    let dna = self.dna_zinciri(d, s, m);
                    if !crate::dna::bos(&dna) {
                        dna_haritasi.insert(format!("{}:{}", goreli(&self.kok, &d.yol), m.satir), dna);
                    }
                    self.nesne_notlari.borrow_mut().clear();
                    let govde = self.girdiler(&m.parametreler, &self.k.parametre.govde, d, true);
                    let sorgu = self.girdiler(&m.parametreler, &self.k.parametre.sorgu, d, true);
                    let mut notlar = std::mem::take(&mut *self.nesne_notlari.borrow_mut());
                    // Dosya yüklemesi: dosya gövde şemasının parçası değildir; varlığı, türü ve içeriği önizlemede sınanmaz
                    let dosya_param = m.parametreler.iter().find_map(|p| {
                        p.dekoratorler
                            .iter()
                            .map(|x| d.iceaktarma_asil_adlari.get(&x.ad).cloned().unwrap_or_else(|| x.ad.clone()))
                            .find(|a| self.k.parametre.dosya.contains(a))
                            .map(|a| format!("'{}' parametresi (@{a})", temiz(&p.ad)))
                    });
                    let dosya_isareti = dosya_desen.as_ref().and_then(|re| {
                        md.iter().chain(m.parametreler.iter().flat_map(|p| p.dekoratorler.iter())).find_map(|x| {
                            let metin = format!("{}({})", x.ad, x.argumanlar.join(", "));
                            re.find(&metin).map(|b| temiz(b.as_str()))
                        })
                    });
                    if let Some(neden) = dosya_param.or(dosya_isareti) {
                        notlar.push(format!("dosya doğrulaması kontrol edilemedi: {neden}; dosyanın varlığı, türü ve içeriği önizlemede sınanmaz"));
                    }
                    if !notlar.is_empty() {
                        denetlenemeyen.insert(format!("{}:{}", goreli(&self.kok, &d.yol), m.satir), notlar);
                    }
                    if let Some(c) =
                        m.dekoratorler.iter().filter(|x| self.k.ozet.dekorator.contains(&x.ad)).flat_map(|x| x.argumanlar.iter()).find_map(|a| operation_id.captures(a))
                    {
                        islem_kimlikleri.insert(format!("{}:{}", goreli(&self.kok, &d.yol), m.satir), temiz(&c[1]));
                    } else if let Some(f) = &kimlik_fabrikasi {
                        // Uygulama geneli `operationIdFactory` (SwaggerModule.createDocument seçenekleri)
                        islem_kimlikleri.insert(format!("{}:{}", goreli(&self.kok, &d.yol), m.satir), temiz(&f.uret(&s.ad, &m.ad)));
                    }
                    let parametreler = self.girdiler(&m.parametreler, &self.k.parametre.yol, d, false);
                    // Bir metot birden fazla adrese bağlanabilir: @Get(["/a", "/b"]); controller adresleriyle her birleşim bir kapıdır
                    let adresler = dek.argumanlar.first().map(|a| self.degerler(a, Some(d))).unwrap_or_else(|| vec![String::new()]);
                    for (ctrl_yol, adres) in ctrl_yollari.iter().flat_map(|c| adresler.iter().map(move |a| (c, a))) {
                        let yol = yol_birlestir(&[&onek, ctrl_yol, adres]);
                        kapilar.push(Kapi {
                            id: format!("{} {}", yontem, yol),
                            yontem: yontem.clone(),
                            yol,
                            kontrolcu: s.ad.clone(),
                            fonksiyon: m.ad.clone(),
                            dosya: goreli(&self.kok, &d.yol),
                            satir: m.satir,
                            giris: giris.into(),
                            giris_kaniti: giris_kaniti.into(),
                            guardlar: guardlar.clone(),
                            roller: roller.clone(),
                            roller_kaniti: roller_kaniti.clone(),
                            ozet: ozet.clone(),
                            govde: govde.clone(),
                            sorgu: sorgu.clone(),
                            parametreler: parametreler.clone(),
                            surum: surum.clone(),
                            kullanimdan_kalkmis,
                            yerine: yerine.clone(),
                            uygulama: ait.get(&s.ad).map(|k| k.join(",")),
                            // Yalnızca ikincil bir uygulamaya ait olduğu kesinse ana değildir; bilinmiyorsa ana sayılır
                            ana: match (&ana_kok, ait.get(&s.ad)) {
                                (Some(a), Some(k)) => k.contains(a),
                                _ => true,
                            },
                            veri: veri.clone(),
                            yanit: yanit.clone(),
                        });
                    }
                }
            }
        }
        kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
        let bilgi = json!({ "genel_onek": onek, "genel_guardlar": genel_ozet, "ana_uygulama": ana_kok, "surum_secimi": self.surum_secimi(), "denetlenemeyen": denetlenemeyen, "islem_kimlikleri": islem_kimlikleri, "dna": dna_haritasi, "kaskadlar": self.typeorm_kaskadlari(), "moduller": self.modul_grafi(), "olaylar": self.olaylar() });
        (kapilar, bilgi)
    }
}

pub fn alan_tipi(tip: &Option<String>, kurallar: &[String]) -> &'static str {
    // { each: true } kuralları dizinin öğelerine uygulanır, alanın kendi tipini belirlemez (@IsInt({ each: true }) ids: number[])
    let k = kurallar.iter().filter(|r| !crate::kural::her_oge(r)).cloned().collect::<Vec<_>>().join(" ");
    if k.is_empty() && kurallar.iter().any(|r| crate::kural::her_oge(r)) {
        return "array";
    }
    if k.contains("IsInt") {
        return "integer";
    }
    if k.contains("IsNumber") || k.contains("IsLatitude") || k.contains("IsLongitude") {
        return "number";
    }
    if k.contains("IsBoolean") {
        return "boolean";
    }
    if k.contains("IsDateString") || k.contains("IsDate") {
        return "datetime";
    }
    if k.contains("IsEnum") || k.contains("IsIn(") {
        return "enum";
    }
    if k.contains("IsArray") {
        return "array";
    }
    if k.contains("IsObject") || k.contains("ValidateNested") {
        return "object";
    }
    // zod: z.coerce.number() gibi zincirlerde asıl tip kuralların içindedir
    let zod_kural = |ad: &str| kurallar.iter().any(|r| r.starts_with(&format!("{ad}(")));
    match tip.as_deref() {
        Some("string") => "string",
        Some("number") => "number",
        Some("int") | Some("bigint") => "integer",
        Some("email") | Some("uuid") | Some("uuidv4") | Some("uuidv7") | Some("guid") | Some("url") | Some("cuid") | Some("cuid2") | Some("ulid") | Some("nanoid") => "string",
        Some("boolean") => "boolean",
        // z.stringbool(): istekte metin gelir ("true" / "false" gibi), uygulama mantıksal değere çevirir
        Some("stringbool") => "string",
        Some("date") => "datetime",
        Some("enum") | Some("nativeEnum") => "enum",
        Some("array") => "array",
        // z.record(anahtar, değer) / z.map: anahtar-değer nesnesi
        Some("record") => "object",
        Some("object") => "object",
        Some("z.coerce") if zod_kural("int") => "integer",
        Some("z.coerce") if zod_kural("number") => "number",
        Some("z.coerce") if zod_kural("boolean") => "boolean",
        Some("z.coerce") if zod_kural("date") => "datetime",
        Some(t) if t.ends_with("[]") => "array",
        // `string | null` gibi: null / undefined atılınca tek tip kalıyorsa o tiptir (boş değere izin ayrıca kural değildir)
        Some(t) if t.contains('|') => {
            let kalan: Vec<&str> = t.split('|').map(str::trim).filter(|p| !matches!(*p, "null" | "undefined")).collect();
            match kalan.as_slice() {
                [tek] if !tek.contains('|') => alan_tipi(&Some((*tek).to_string()), kurallar),
                _ => "unknown",
            }
        }
        Some(_) => "unknown",
        None => {
            if k.contains("IsString") || k.contains("IsEmail") {
                "string"
            } else {
                "unknown"
            }
        }
    }
}

/// Rota kaynağı bulunan ama okunamayan yerler ("dosya:satır neden"), bütün adaptörler için: adaptörün
/// `okunamayan_rotalar`ı (FastAPI: router oluşturulmuş ama rotası okunamadı, tanınmayan rota dekoratörü, çözülemeyen
/// yol), yolu çalışma anında kurulan kayıtlar (`dinamik_kayitlar`: Go, Ktor, Laravel), işleme çevrilemeyen rotalar
/// (`cozulemeyen_rotalar`: Rails) ve çözülemeyen URL kayıtları (`haritalanmayan`: Django; bilerek dışarıda bırakılan
/// yönetim paneli ve proje dışı yapılandırma sayılmaz). Boş değilse işlem listesi eksiktir: kapsam "kısmen"
pub fn okunamayan_rota_kaynaklari(ozet: &Value) -> Vec<String> {
    let mut v: Vec<String> = ["okunamayan_rotalar", "dinamik_kayitlar", "cozulemeyen_rotalar"]
        .iter()
        .flat_map(|a| ozet[*a].as_array().into_iter().flatten())
        .filter_map(|x| x.as_str().map(String::from))
        .collect();
    v.extend(ozet["haritalanmayan"].as_array().into_iter().flatten().filter_map(|h| {
        let (yer, neden) = (h["yer"].as_str()?, h["neden"].as_str()?);
        (neden.contains("çözülemedi") || neden.contains("tanınmayan")).then(|| format!("{yer} {neden}"))
    }));
    v.sort();
    v.dedup();
    v
}

/// Tek satırlık kapsam uyarısı: "okunamayan rota kaynağı: N yer (dosya:satır, …)"; liste boşsa None
pub fn okunamayan_rota_uyarisi(liste: &[String]) -> Option<String> {
    if liste.is_empty() {
        return None;
    }
    let yerler: Vec<&str> = liste.iter().take(3).map(|x| x.split_whitespace().next().unwrap_or(x)).collect();
    let fazla = if liste.len() > 3 { tf!(" ve {} yer daha", " and {} more", liste.len() - 3) } else { String::new() };
    Some(tf!(
        "okunamayan rota kaynağı: {} yer ({}{fazla}); bu yerlerin işlemleri haritada yok",
        "unreadable route source: {} {p} ({}{fazla}); their operations are not in the map",
        liste.len(),
        yerler.join(", ");
        p = crate::dil::cogul(liste.len(), "place", "places")
    ))
}

/// Okunamayan rota kaynağının seçilen dildeki satırı (neden haritaya Türkçe yazılır; dosya:satır ve kod parçası çevrilmez)
pub fn okunamayan_rota_satiri(s: &str) -> String {
    if !crate::dil::en() {
        return s.to_string();
    }
    [
        (": router uygulamaya bağlı ama rotası okunamadı", ": router is mounted in the application but its routes could not be read"),
        (": tanınmayan rota dekoratörü", ": unrecognised route decorator"),
        (": işleyici okunamadı", ": handler could not be read"),
        (": yol çözülemedi", ": path could not be resolved"),
        (" önek ", " prefix "),
    ]
    .iter()
    .fold(s.to_string(), |m, (tr, en)| m.replace(tr, en))
}
