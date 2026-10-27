//! Dış etki, geri alınabilirlik ve iş durumu ön koşulları (işlem başına).
//!
//! Kaynak dosyalar dilden bağımsız satır desenleriyle taranır (ayrıştırıcı gerekmez; dosya başına bir geçiş):
//! - **Giden çağrılar:** HTTP istemcileri (fetch, axios, got, node:http, NestJS `HttpService`, requests, httpx,
//!   aiohttp, urllib, HttpClient, RestTemplate / WebClient / RestClient / Feign, java.net.http, Ktor HttpClient, Guzzle / Laravel `Http`,
//!   curl, net/http, resty, Net::HTTP / Faraday / HTTParty / RestClient), kuyruk ve olay yayını (Kafka, RabbitMQ /
//!   amqplib / pika, SQS / SNS / EventBridge, Pub/Sub, Azure Service Bus, MassTransit, olay veri yolu, NestJS
//!   mikroservis istemcisi), e-posta (nodemailer, SendGrid, smtplib, `emails`, Django / Flask / FastAPI posta,
//!   Laravel `Mail`, ActionMailer, JavaMailSender, SmtpClient, net/smtp, SES), ödeme ve bulut SDK'ları (stripe,
//!   aws-sdk / boto3, @google-cloud) ve bildirim (Twilio, Laravel bildirimleri).
//! - **Hedef ve güveni:** sabit adres (ana makine) `certain`; ortam değişkeni ya da yapılandırma anahtarı `inferred`
//!   (değeri çalışma anında belli olur); çözülemeyen ifade `unknown` ("dinamik"). SDK çağrısının hedefi SDK'nın adıdır
//!   (paket dosyada içe aktarılmışsa `certain`, değilse `inferred`). Göreli adres (`/api/...`) uygulamanın kendisine
//!   gider: dış çağrı sayılmaz.
//! - **İşleme bağlama:** işleyici fonksiyondan başlayarak çağrı adları ad eşlemesiyle (alıcının adı hedef dosyanın
//!   adına uyuyor, aynı dosyada ya da projede tek) en çok 5 adım izlenir; işlemin yaydığı olay ve işlerin
//!   dinleyicileri ertelenmiş adım olarak eklenir. Bu yerel ve sınırlı bir izlemedir: zincirden ulaşılan etkinin
//!   güveni en fazla `inferred`; işleyicinin kendi gövdesindeki etki hedefin güvenini taşır.
//! - **Geri alınabilirlik:** yumuşak silme (alan yazması ya da model düzeyinde SoftDeletes / paranoid /
//!   acts_as_paranoid / DeleteDateColumn / gorm.DeletedAt / SafeDeleteModel), geri yükleme ucu, kalıcı silme çağrısı,
//!   kaskad, işlem (transaction) sınırı ve geri alınamaz dış etki → `evet | kismen | hayir | bilinmiyor`, gerekçe ve
//!   damga (en fazla `inferred`: geri alınabilirlik bir çıkarımdır).
//! - **Ön koşullar:** zincirdeki koşullu fırlatma ve iptaller (`if (!x) throw new NotFoundException()`,
//!   `abort_unless`, `raise HTTPException`, `ResponseStatusException`, `return NotFound()`, `head :forbidden`) red
//!   koduyla davranış özetinin hata senaryolarına eklenir; önizleme bunları "kontrol edilemedi" sayar.
//! - **Onay:** dış sisteme yazan ya da geri alınamaz (veya silen ama geri alınabilirliği kanıtlanmamış) yazma
//!   işlemi onaysız gidemez: gerekçe haritadaki kapıya `etki_onayi` olarak yazılır, canlı modun tek onay kuralı okur.
//!
//! Bulunmayan etki "yok" demek değildir: kapsam her zaman `acik`.
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// İşleyiciden en çok kaç adım izlenir
const EN_FAZLA_DERINLIK: usize = 5;
/// Bir işlem için ziyaret edilecek en fazla fonksiyon
const EN_FAZLA_FONKSIYON: usize = 120;
/// Bir işlemde tutulacak en fazla dış çağrı
const EN_FAZLA_DIS: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dil {
    Js,
    Py,
    Jvm,
    Cs,
    Go,
    Php,
    Rb,
}

fn dil_sec(uzanti: &str) -> Option<Dil> {
    Some(match uzanti {
        "ts" | "js" | "mjs" | "cjs" | "mts" | "cts" => Dil::Js,
        "py" => Dil::Py,
        "java" | "kt" => Dil::Jvm,
        "cs" => Dil::Cs,
        "go" => Dil::Go,
        "php" => Dil::Php,
        "rb" => Dil::Rb,
        _ => return None,
    })
}

/// Bir giden çağrı (dış etki)
#[derive(Clone, Debug)]
struct Dis {
    /// http | kuyruk | eposta | odeme | bulut | bildirim
    tur: &'static str,
    istemci: String,
    yontem: Option<String>,
    /// Ana makine, ortam değişkeni / yapılandırma anahtarı, SDK adı ya da "dinamik"
    hedef: String,
    /// url | ortam | sdk | dinamik
    hedef_turu: &'static str,
    /// Adresin yol kısmı (`/v1/oauth2/token`), biliniyorsa
    yol: Option<String>,
    /// Kuyruk / konu / posta sınıfı adı
    kanal: Option<String>,
    yazma: bool,
    ertelenmis: bool,
    guven: &'static str,
    satir: usize,
    /// İstemci örneğinin adı (`catalogWebClient`): taban adresi DI kaydından bulmak için
    alici: Option<String>,
}

impl Dis {
    /// Grafikteki dış sistem düğümü: `external:<ana makine | ortam.AD | sdk>`
    fn dugum(&self) -> String {
        match self.hedef_turu {
            "url" | "sdk" => format!("external:{}", self.hedef),
            "ortam" => format!("external:ortam.{}", self.hedef),
            _ => "external:dinamik".into(),
        }
    }
}

/// Koşullu red: ön koşul
#[derive(Clone, Debug)]
struct Ret {
    durum: Option<u16>,
    istisna: String,
    kosul: Option<String>,
    satir: usize,
}

/// Silme çağrısı: alıcı (değişken / depo adı), yöntem, tablo (biliniyorsa), satır
#[derive(Clone, Debug)]
struct Silme {
    alici: String,
    yontem: String,
    tablo: Option<String>,
    satir: usize,
}

/// Fonksiyon sınırı (satırlar 1'den, kapalı aralık)
#[derive(Clone, Debug)]
struct Blok {
    dosya: usize,
    ad: String,
    bas: usize,
    son: usize,
    /// İçinde bulunduğu sınıf (Go'da yöntem alıcısının tipi)
    sinif: Option<String>,
}

/// Bir dosyanın taranmış hâli
#[derive(Default)]
struct DosyaSonuc {
    bloklar: Vec<(String, usize, usize)>,
    /// Blokların sınıfları (`bloklar` ile aynı sırada)
    siniflar: Vec<Option<String>>,
    /// Alan / parametre / değişken adı -> tipi (`private service: SearchService`, `IBasketService _basketService`)
    alan_tipleri: HashMap<String, String>,
    /// (satır, alıcı, ad)
    cagrilar: Vec<(usize, Option<String>, String)>,
    disler: Vec<Dis>,
    retler: Vec<Ret>,
    /// (satır, ifade)
    islem_sinirlari: Vec<(usize, String)>,
    silmeler: Vec<Silme>,
    /// Yumuşak silme çağrısı ya da geri yükleme çağrısı (satır, yöntem)
    yumusak_cagrilar: Vec<(usize, String)>,
    /// Model düzeyinde yumuşak silme: (model, satır, işaret)
    yumusak_modeller: Vec<(String, usize, String)>,
    /// Kuyruk dinleyicileri: (tür, kanal, satır)
    dinleyiciler: Vec<(String, String, usize)>,
    /// Kalıtım ve gömme: (alt sınıf / yapı, üst tip, satır); yumuşak silme taban sınıftan gelebilir
    ustler: Vec<(String, String, usize)>,
    /// DI ile kaydedilen istemcinin taban adresi: (tip / bean adı ya da "*", çözülmüş hedef, satır)
    tabanlar: Vec<(String, Cozum, usize)>,
    /// Önceki değeri saklayan sürüm / geçmiş işaretleri: model düzeyi (model, satır, işaret)
    gecmis_modeller: Vec<(String, usize, String)>,
    /// Gövdede sürüm / geçmiş kaydı yazımı: (satır, ifade)
    gecmis_yazmalari: Vec<(usize, String)>,
    /// İstek yöntemine göre ayrılan dallar (`if request.method == "POST":`)
    yontem_dallari: Vec<crate::cagri::YontemDali>,
}

struct Dosya {
    yol: String,
    mutlak: PathBuf,
    ozet: String,
    sonuc: DosyaSonuc,
}

/// Projenin taranmış hâli
pub struct Tarama {
    dosyalar: Vec<Dosya>,
    bloklar: Vec<Blok>,
    ad_dizini: HashMap<String, Vec<usize>>,
    yol_dizini: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------------------------------------------
// Dosya toplama

/// Test, derleme ve bağımlılık klasörleri taranmaz
fn atla(goreli: &str) -> bool {
    let alt = goreli.to_ascii_lowercase();
    let parcalar: Vec<&str> = alt.split('/').collect();
    let klasor = parcalar[..parcalar.len().saturating_sub(1)].iter().any(|c| {
        matches!(
            *c,
            "node_modules"
                | "dist"
                | "build"
                | "vendor"
                | ".venv"
                | "venv"
                | "site-packages"
                | "target"
                | "__pycache__"
                | "bin"
                | "obj"
                | "test"
                | "tests"
                | "__tests__"
                | "spec"
                | "specs"
                | "e2e"
                | "testdata"
                | "fixtures"
                | ".sezgi"
                | "coverage"
                | "storage"
                | "public"
                | "static"
                | "wwwroot"
                | "www"
                | "frontend"
        )
    });
    // Ön yüz kaynakları (Laravel `resources/js`): tarayıcıdaki çağrılar uygulamanın dış etkisi değildir
    let on_yuz = alt.contains("resources/assets/") || alt.contains("resources/js/") || alt.contains("resources/ts/");
    let ad = parcalar.last().copied().unwrap_or("");
    klasor
        || on_yuz
        || ad.ends_with(".d.ts")
        || ad.contains(".spec.")
        || ad.contains(".test.")
        || ad.starts_with("test_")
        || ad.ends_with("_test.py")
        || ad.ends_with("_test.go")
        || ad.ends_with("_spec.rb")
        || ad.ends_with("_test.rb")
        || ad.ends_with("test.java")
        || ad.ends_with("tests.java")
        || ad.ends_with("tests.cs")
        || ad.ends_with("test.cs")
        || ad.ends_with("test.kt")
        || ad.ends_with(".min.js")
}

/// Kökteki kaynak dosyaları tarar. `haric`: ön yüz klasörü (ön yüzün çağrıları istemci bağlarıdır, dış etki değil)
pub fn tara(kok: &Path, haric: Option<&Path>) -> Tarama {
    let mut yollar: Vec<(PathBuf, String, Dil)> = vec![];
    for g in ignore::WalkBuilder::new(kok).hidden(true).git_ignore(true).build().flatten() {
        let p = g.path();
        if g.path_is_symlink() || !p.is_file() || haric.is_some_and(|h| p.starts_with(h)) {
            continue;
        }
        let Some(d) = p.extension().and_then(|e| e.to_str()).and_then(dil_sec) else { continue };
        let goreli = crate::guvenli::goreli(kok, p);
        if atla(&goreli) {
            continue;
        }
        yollar.push((p.to_path_buf(), goreli, d));
    }
    yollar.sort_by(|a, b| a.1.cmp(&b.1));
    let sonuclar: Vec<Option<Dosya>> = std::thread::scope(|s| {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
        let parca = yollar.len().div_ceil(n).max(1);
        let isler: Vec<_> = yollar
            .chunks(parca)
            .map(|c| {
                s.spawn(move || {
                    c.iter()
                        .map(|(p, g, d)| {
                            let m = crate::guvenli::oku(p)?;
                            let ozet = crate::kesif::icerik_ozeti(m.as_bytes());
                            Some(Dosya { yol: g.clone(), mutlak: p.clone(), ozet, sonuc: dosya_tara(*d, &m) })
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        isler.into_iter().flat_map(|i| i.join().unwrap_or_default()).collect()
    });
    let mut dosyalar: Vec<Dosya> = sonuclar.into_iter().flatten().collect();
    tabanlari_uygula(&mut dosyalar);
    let mut bloklar = vec![];
    let mut ad_dizini: HashMap<String, Vec<usize>> = HashMap::new();
    let mut yol_dizini = HashMap::new();
    for (i, d) in dosyalar.iter().enumerate() {
        yol_dizini.insert(d.yol.clone(), i);
        for (j, (ad, bas, son)) in d.sonuc.bloklar.iter().enumerate() {
            if !ad.is_empty() {
                ad_dizini.entry(ad.clone()).or_default().push(bloklar.len());
            }
            bloklar.push(Blok { dosya: i, ad: ad.clone(), bas: *bas, son: *son, sinif: d.sonuc.siniflar.get(j).cloned().flatten() });
        }
    }
    Tarama { dosyalar, bloklar, ad_dizini, yol_dizini }
}

// ---------------------------------------------------------------------------------------------------------------
// Fonksiyon sınırları

/// Satır başlarının bayt konumları
fn satir_baslari(m: &str) -> Vec<usize> {
    let mut v = vec![0];
    v.extend(m.match_indices('\n').map(|(i, _)| i + 1));
    v
}

/// Bayt konumunun satırı (1'den)
fn satir_no(baslar: &[usize], konum: usize) -> usize {
    match baslar.binary_search(&konum) {
        Ok(i) => i + 1,
        Err(i) => i,
    }
}

/// Kıvrık parantez eşleri (açılışın bayt konumu -> kapanışınki); dizeler ve yorumlar atlanır. JS şablon dizelerindeki
/// `${...}` ifadeleri kod sayılır
fn parantez_eslesmeleri(m: &str, dil: Dil) -> HashMap<usize, usize> {
    let b = m.as_bytes();
    let mut es = HashMap::new();
    // (açılış, şablon ifadesi mi)
    let mut yigin: Vec<(usize, bool)> = vec![];
    // Şablon dizesini `i`den tarar: dönen (sonraki konum, ifade açılışı)
    let sablon = |mut i: usize| -> (usize, Option<usize>) {
        while i < b.len() {
            match b[i] {
                b'\\' => i += 2,
                b'`' => return (i + 1, None),
                b'$' if b.get(i + 1) == Some(&b'{') => return (i + 2, Some(i + 1)),
                _ => i += 1,
            }
        }
        (i, None)
    };
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'#' if dil == Dil::Php && b.get(i + 1) != Some(&b'[') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            b'"' | b'\'' => {
                // Go ve JVM'de tek tırnak karakter sabitidir; aynı kural işler
                i += 1;
                while i < b.len() && b[i] != c && b[i] != b'\n' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'`' if dil == Dil::Js => {
                let (j, ifade) = sablon(i + 1);
                if let Some(a) = ifade {
                    yigin.push((a, true));
                }
                i = j;
                continue;
            }
            b'`' if dil == Dil::Go => {
                i += 1;
                while i < b.len() && b[i] != b'`' {
                    i += 1;
                }
            }
            b'{' => yigin.push((i, false)),
            b'}' => {
                if let Some((a, sab)) = yigin.pop() {
                    es.insert(a, i);
                    if sab {
                        let (j, ifade) = sablon(i + 1);
                        if let Some(a) = ifade {
                            yigin.push((a, true));
                        }
                        i = j;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    es
}

/// `(` konumundan eşleşen `)` (dizeleri kabaca atlayarak); bulunamazsa None
fn kapanan_parantez(b: &[u8], ac: usize) -> Option<usize> {
    let mut d = 0i32;
    let mut i = ac;
    while i < b.len() && i < ac + 4000 {
        match b[i] {
            b'(' => d += 1,
            b')' => {
                d -= 1;
                if d == 0 {
                    return Some(i);
                }
            }
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < b.len() && b[i] != q && b[i] != b'\n' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Parametre listesinden sonra gövdenin `{` konumu: dönüş tipi, `throws`, `where`, `use (...)`, `: base(...)` atlanır;
/// `;` ya da (izin verilmeyen) `=>` önce gelirse gövde yoktur. `ok`: JS ok fonksiyonu (`=>` beklenir)
fn govde_acilisi(b: &[u8], kapanis: usize, ok: bool) -> Option<usize> {
    let mut i = kapanis + 1;
    let mut ok_goruldu = false;
    while i < b.len() && i < kapanis + 400 {
        match b[i] {
            b'{' => return (!ok || ok_goruldu).then_some(i),
            b';' => return None,
            b'=' if b.get(i + 1) == Some(&b'>') => {
                if !ok {
                    return None;
                }
                ok_goruldu = true;
                i += 1;
            }
            b'=' => return None,
            b'(' => i = kapanan_parantez(b, i)?,
            _ => {}
        }
        i += 1;
    }
    None
}

const ANAHTAR_SOZCUKLER: [&str; 30] = [
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "return",
    "function",
    "await",
    "new",
    "typeof",
    "super",
    "else",
    "do",
    "try",
    "foreach",
    "elseif",
    "using",
    "lock",
    "fixed",
    "synchronized",
    "when",
    "match",
    "throw",
    "sizeof",
    "nameof",
    "default",
    "case",
    "yield",
    "delete",
    "void",
];

fn desenler_bloklar(dil: Dil) -> &'static [Regex] {
    static D: OnceLock<HashMap<u8, Vec<Regex>>> = OnceLock::new();
    let h = D.get_or_init(|| {
        let r = |s: &str| Regex::new(s).unwrap();
        let mut h = HashMap::new();
        h.insert(
            0u8,
            vec![
                r(r"^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s*\*?\s*([A-Za-z_$][\w$]*)\s*(?:<[^>]*>)?\s*\("),
                r(r"^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=]*)?=\s*(?:async\s+)?(?:function\b[^(]*|\(|[A-Za-z_$][\w$]*\s*=>)"),
                r(r"^\s*(?:(?:public|private|protected|static|async|readonly|override|abstract|get|set)\s+)*\*?\s*([A-Za-z_$][\w$]*)\s*(?:<[^>]*>)?\s*\("),
                // Sınıf alanına atanmış ok fonksiyonu: `handle = async (req, res) => {`
                r(r"^\s*(?:(?:public|private|protected|static|readonly)\s+)*([A-Za-z_$][\w$]*)\s*(?::[^=]*)?=\s*(?:async\s+)?\("),
            ],
        );
        h.insert(
            2u8,
            vec![
                r(r"^\s*(?:@\w+(?:\([^)]*\))?\s+)*(?:(?:public|private|protected|internal|static|final|async|override|virtual|suspend|open|synchronized|abstract|sealed|partial|new|unsafe|extern|inline|operator|default)\s+)*(?:fun\s+(?:<[^>]*>\s*)?(?:[\w.]+\.)?|[\w<>\[\],.?]+(?:<[^()]*>)?\s+)([A-Za-z_]\w*)\s*(?:<[^()]*>)?\s*\("),
            ],
        );
        h.insert(4u8, vec![r(r"^\s*func\s+(?:\([^)]*\)\s*)?([A-Za-z_]\w*)\s*(?:\[[^\]]*\])?\s*\(")]);
        h.insert(5u8, vec![r(r"^\s*(?:(?:public|private|protected|static|final|abstract)\s+)*function\s+&?\s*([A-Za-z_]\w*)\s*\(")]);
        h
    });
    let anahtar = match dil {
        Dil::Js => 0,
        Dil::Jvm | Dil::Cs => 2,
        Dil::Go => 4,
        Dil::Php => 5,
        Dil::Py | Dil::Rb => return &[],
    };
    h.get(&anahtar).map(Vec::as_slice).unwrap_or(&[])
}

/// Dosyanın fonksiyon blokları: (ad, ilk satır, son satır); adsız bloklar (ok fonksiyonu, kapanış, Go fonksiyon
/// değişmezi) boş adla
fn bloklari_bul(dil: Dil, m: &str, baslar: &[usize]) -> Vec<(String, usize, usize)> {
    let satirlar: Vec<&str> = m.lines().collect();
    let mut v = vec![];
    match dil {
        Dil::Py => {
            static D: OnceLock<Regex> = OnceLock::new();
            let r = D.get_or_init(|| Regex::new(r"^(\s*)(?:async\s+)?def\s+(\w+)\s*\(").unwrap());
            for (i, s) in satirlar.iter().enumerate() {
                let Some(c) = r.captures(s) else { continue };
                let girinti = c[1].len();
                let mut son = i;
                for (j, t) in satirlar.iter().enumerate().skip(i + 1) {
                    let tt = t.trim();
                    if tt.is_empty() || tt.starts_with('#') {
                        continue;
                    }
                    let g = t.len() - t.trim_start().len();
                    // Parametre listesi birden fazla satıra yayılabilir: `)` ya da `):` ile başlayan satır gövdedir
                    if g <= girinti && !tt.starts_with(')') && j > i {
                        break;
                    }
                    son = j;
                }
                v.push((c[2].to_string(), i + 1, son + 1));
            }
        }
        Dil::Rb => {
            static D: OnceLock<Regex> = OnceLock::new();
            let r = D.get_or_init(|| Regex::new(r"^(\s*)def\s+(?:self\.)?([\w?!=]+)").unwrap());
            for (i, s) in satirlar.iter().enumerate() {
                let Some(c) = r.captures(s) else { continue };
                let girinti = &c[1];
                let mut son = i;
                // Tek satırlık tanım (`def x = ...` ya da `def x; ...; end`)
                if !s.contains(" end") && !s.trim_end().ends_with("end") {
                    for (j, t) in satirlar.iter().enumerate().skip(i + 1) {
                        son = j;
                        if t.starts_with(girinti) && t[girinti.len()..].starts_with("end") && t.len() - t.trim_start().len() == girinti.len() {
                            break;
                        }
                    }
                }
                v.push((c[2].to_string(), i + 1, son + 1));
            }
        }
        _ => {
            let es = parantez_eslesmeleri(m, dil);
            let b = m.as_bytes();
            let mut adli: BTreeSet<usize> = BTreeSet::new();
            for (i, s) in satirlar.iter().enumerate() {
                let t = s.trim_start();
                if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") || t.starts_with('#') {
                    continue;
                }
                for (ri, r) in desenler_bloklar(dil).iter().enumerate() {
                    let Some(c) = r.captures(s) else { continue };
                    let ad = c[1].to_string();
                    if ANAHTAR_SOZCUKLER.contains(&ad.as_str()) {
                        continue;
                    }
                    let tam = c.get(0).unwrap();
                    let ok = dil == Dil::Js && (ri == 1 || ri == 3);
                    // Eşleşmenin son `(`'ı parametre listesinin başı; JS ok fonksiyonunda `=` sonrası ilk `(`
                    let bas = baslar[i];
                    let ac = match s[..tam.end()].rfind('(') {
                        Some(p) => bas + p,
                        None => {
                            // `const f = x => {` ya da `function` anahtar sözcüğü: gövde `{`'ı doğrudan aranır
                            let Some(g) = m[bas + tam.end()..].find('{').map(|x| bas + tam.end() + x) else { break };
                            if let Some(k) = es.get(&g) {
                                v.push((ad, i + 1, satir_no(baslar, *k)));
                                adli.insert(g);
                            }
                            break;
                        }
                    };
                    let Some(kap) = kapanan_parantez(b, ac) else { break };
                    // Çağrı (`foo(() => {`) yöntem başı sanılmasın: parametre listesinde `=>` ya da `;` olmamalı
                    if !ok && ri != 0 && m[ac..kap].contains("=>") {
                        break;
                    }
                    match govde_acilisi(b, kap, ok) {
                        Some(g) => {
                            if let Some(k) = es.get(&g) {
                                v.push((ad, i + 1, satir_no(baslar, *k)));
                                adli.insert(g);
                            }
                        }
                        None if ok => {
                            // Gövdesiz ok fonksiyonu: tek ifade, satırın sonuna kadar
                            v.push((ad, i + 1, i + 1));
                        }
                        None => {}
                    }
                    break;
                }
            }
            // Adsız bloklar: `=> {`, `function (`, Go `func(`, PHP `function (` / `fn (`. Satır bir atamaysa
            // (`const updateOrder = asyncHandler(async (req, res) => {`, `exports.x = function (`, `x: async () => {`)
            // blok o adı alır: sarmalanmış işleyiciler adıyla bulunur
            static D: OnceLock<[Regex; 2]> = OnceLock::new();
            let [r, atama] = D.get_or_init(|| {
                [
                    Regex::new(r"=>\s*\{|\bfunction\s*\*?\s*\(|\bfunc\s*\(|->\s*\{").unwrap(),
                    Regex::new(r"^\s*(?:export\s+)?(?:(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=]*)?=|(?:module\s*\.\s*)?exports\s*\.\s*([A-Za-z_$][\w$]*)\s*=|([A-Za-z_$][\w$]*)\s*:\s*(?:async\s+)?(?:function\b|\())").unwrap(),
                ]
            });
            let mut adlanan: BTreeSet<usize> = BTreeSet::new();
            for c in r.find_iter(m) {
                let g = if c.as_str().ends_with('{') {
                    c.end() - 1
                } else {
                    let Some(ac) = m[c.start()..].find('(').map(|x| c.start() + x) else { continue };
                    let Some(kap) = kapanan_parantez(b, ac) else { continue };
                    let Some(g) = govde_acilisi(b, kap, false) else { continue };
                    g
                };
                if adli.contains(&g) {
                    continue;
                }
                if let Some(k) = es.get(&g) {
                    let bas = satir_no(baslar, c.start());
                    // Satırın ilk adsız bloğu atamanın adını alır
                    let ad = if dil == Dil::Js && adlanan.insert(bas) {
                        atama.captures(satirlar.get(bas - 1).unwrap_or(&"")).and_then(|x| x.iter().skip(1).flatten().next().map(|a| a.as_str().to_string())).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    v.push((ad, bas, satir_no(baslar, *k)));
                }
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------------------------
// Satır desenleri

const YONTEMLER: &str = "get|post|put|patch|delete|head|options";

fn yontem_buyuk(s: &str) -> Option<String> {
    let s = s.to_ascii_uppercase();
    let s = s.trim_end_matches("ASYNC").trim_end_matches("ASJSON").trim_end_matches("FROMJSON");
    let s = match s {
        "GETFOROBJECT" | "GETFORENTITY" | "GETSTRING" | "GETBYTEARRAY" | "GETSTREAM" | "GETRESPONSE" | "GET_RESPONSE" | "URLOPEN" => "GET",
        "POSTFOROBJECT" | "POSTFORENTITY" | "POSTFORLOCATION" | "POSTFORM" | "POST_FORM" => "POST",
        "PATCHFOROBJECT" => "PATCH",
        x => x,
    };
    matches!(s, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS").then(|| s.to_string())
}

/// `method: 'POST'` / `method="POST"` / `HttpMethod.Post` / `CURLOPT_POST`
fn yontem_secenegi(s: &str) -> Option<String> {
    static D: OnceLock<[Regex; 2]> = OnceLock::new();
    let [a, b] = D.get_or_init(|| {
        [
            Regex::new(r#"(?i)\bmethod\s*[:=]>?\s*['"`](GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS)['"`]"#).unwrap(),
            Regex::new(r"\bHttpMethod\s*[.:]+\s*(Get|Post|Put|Patch|Delete|Head|GET|POST|PUT|PATCH|DELETE|HEAD)\b").unwrap(),
        ]
    });
    if let Some(c) = a.captures(s).or_else(|| b.captures(s)) {
        return Some(c[1].to_uppercase());
    }
    if s.contains("CURLOPT_POST") {
        return Some("POST".into());
    }
    None
}

/// Çağrının ilk argümanının metni: `(` sonrasından üst düzey virgüle ya da kapanışa kadar
fn ilk_arguman(s: &str) -> Option<String> {
    argumanlar(s).into_iter().next()
}

/// Çağrının argümanları (`s` açılış parantezinden hemen sonrası): üst düzey virgüllerle ayrılır
fn argumanlar(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let mut v = vec![];
    let mut d = 0i32;
    let mut bas = 0;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' | b'[' | b'{' => d += 1,
            b')' | b']' | b'}' => {
                if d == 0 {
                    let a = s[bas..i].trim();
                    if !a.is_empty() {
                        v.push(a.to_string());
                    }
                    return v;
                }
                d -= 1;
            }
            b',' if d == 0 => {
                v.push(s[bas..i].trim().to_string());
                bas = i + 1;
            }
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let a = s[bas..].trim();
    if !a.is_empty() {
        v.push(a.to_string());
    }
    v
}

/// Ham bulgu: hedef ifadesi henüz çözülmemiş
struct Ham {
    tur: &'static str,
    istemci: String,
    yontem: Option<String>,
    /// Hedef ifadesi (çözülecek)
    ifade: Option<String>,
    /// SDK hedefi (ifade yerine)
    sdk: Option<String>,
    kanal: Option<String>,
    /// Okuma mı yazma mı; None ise yöntemden
    yazma: Option<bool>,
    ertelenmis: bool,
    alici: Option<String>,
}

impl Ham {
    fn http(istemci: &str, yontem: Option<String>, ifade: Option<String>) -> Ham {
        Ham { tur: "http", istemci: istemci.into(), yontem, ifade, sdk: None, kanal: None, yazma: None, ertelenmis: false, alici: None }
    }
    fn sdk(tur: &'static str, istemci: &str, sdk: &str, kanal: Option<String>, yazma: bool) -> Ham {
        Ham { tur, istemci: istemci.into(), yontem: None, ifade: None, sdk: Some(sdk.into()), kanal, yazma: Some(yazma), ertelenmis: false, alici: None }
    }
}

/// Dosya düzeyindeki bağlam: içe aktarılan paketler ve istemci örneklerinin adları
#[derive(Default)]
struct Baglam {
    metin_kucuk: String,
    /// İstemci örneği adı -> (istemci türü, taban adres ifadesi)
    ornekler: HashMap<String, (String, Option<String>)>,
}

impl Baglam {
    fn var(&self, s: &str) -> bool {
        self.metin_kucuk.contains(s)
    }
}

fn baglam_kur(dil: Dil, m: &str) -> Baglam {
    let mut b = Baglam { metin_kucuk: m.to_ascii_lowercase(), ..Default::default() };
    static D: OnceLock<Vec<(u8, Regex, &'static str)>> = OnceLock::new();
    let desenler = D.get_or_init(|| {
        let r = |s: &str| Regex::new(s).unwrap();
        vec![
            (0, r(r"\b([A-Za-z_$][\w$]*)\s*=\s*axios\s*\.\s*create\s*\(\s*(?:\{[^}]*?baseURL\s*:\s*([^,}\n]+))?"), "axios"),
            (0, r(r"\b([A-Za-z_$][\w$]*)\s*[:!?]*\s*:\s*HttpService\b"), "HttpService"),
            (0, r(r"\b([A-Za-z_$][\w$]*)\s*[:!?]*\s*:\s*(?:ClientProxy|ClientKafka|ClientRMQ|ClientNats|ClientRedis|ClientGrpc)\b"), "mikroservis"),
            (0, r(r"\b([A-Za-z_$][\w$]*)\s*=\s*(?:new\s+Stripe|require\(\s*['\x22]stripe['\x22]\s*\))\s*\("), "stripe"),
            (0, r(r"\b([A-Za-z_$][\w$]*)\s*=\s*\w+\s*\.\s*producer\s*\("), "kafka"),
            (1, r(r"\b(\w+)\s*=\s*requests\s*\.\s*Session\s*\("), "requests"),
            (1, r(r"requests\s*\.\s*Session\s*\([^)]*\)\s*as\s+(\w+)"), "requests"),
            (1, r(r"\b(\w+)\s*=\s*httpx\s*\.\s*(?:Async)?Client\s*\(\s*(?:[^)]*?base_url\s*=\s*([^,)\n]+))?"), "httpx"),
            (1, r(r"httpx\s*\.\s*(?:Async)?Client\s*\(\s*(?:[^)]*?base_url\s*=\s*([^,)\n]+))?[^)]*\)\s*as\s+(\w+)"), "httpx_as"),
            (1, r(r"aiohttp\s*\.\s*ClientSession\s*\([^)]*\)\s*as\s+(\w+)"), "aiohttp"),
            (1, r(r"\b(\w+)\s*=\s*aiohttp\s*\.\s*ClientSession\s*\("), "aiohttp"),
            (1, r(r#"\b(\w+)\s*=\s*boto3\s*\.\s*(?:client|resource)\s*\(\s*['"](\w+)['"]"#), "boto3"),
            (6, r(r"\b(\w+)\s*=\s*Faraday\s*\.\s*new\s*\(\s*(?:url:\s*([^,)\n]+))?"), "faraday"),
            (5, r(r"\$(\w+)\s*=\s*new\s+(?:\\?GuzzleHttp\\)?Client\s*\(\s*(?:\[[^\]]*?'base_uri'\s*=>\s*([^,\]\n]+))?"), "guzzle"),
        ]
    });
    let kod = match dil {
        Dil::Js => 0,
        Dil::Py => 1,
        Dil::Php => 5,
        Dil::Rb => 6,
        _ => 9,
    };
    for (d, r, tur) in desenler.iter().filter(|(d, _, _)| *d == kod) {
        let _ = d;
        for c in r.captures_iter(m) {
            match *tur {
                "httpx_as" => {
                    if let Some(ad) = c.get(2) {
                        b.ornekler.insert(ad.as_str().to_string(), ("httpx".into(), c.get(1).map(|x| x.as_str().trim().to_string())));
                    }
                }
                "boto3" => {
                    b.ornekler.insert(c[1].to_string(), (format!("boto3:{}", &c[2]), None));
                }
                _ => {
                    b.ornekler.insert(c[1].to_string(), (tur.to_string(), c.get(2).map(|x| x.as_str().trim().to_string())));
                }
            }
        }
    }
    b
}

/// AWS hizmet adından tür
fn aws_turu(hizmet: &str) -> &'static str {
    match hizmet.to_ascii_lowercase().as_str() {
        "sqs" | "sns" | "eventbridge" | "events" | "kinesis" | "firehose" => "kuyruk",
        "ses" | "sesv2" => "eposta",
        _ => "bulut",
    }
}

/// SDK yöntem adından okuma mı (get / list / describe / retrieve / head / download / search / query / scan)
fn okuma_adi(ad: &str) -> bool {
    let a = ad.to_ascii_lowercase();
    ["get", "list", "describe", "retrieve", "head", "download", "search", "query", "scan", "read", "fetch", "find", "all"].iter().any(|p| a.starts_with(p))
}

/// AWS SDK v3 komut adından (`PutObjectCommand` -> s3 benzeri) hizmet: komut adının kendisi yeterli değil; bilinen
/// komutlar için hizmet, ötekiler "aws"
fn aws_komutu(komut: &str) -> (&'static str, &'static str) {
    match komut {
        "SendMessage" | "SendMessageBatch" => ("kuyruk", "aws.sqs"),
        "Publish" | "PublishBatch" => ("kuyruk", "aws.sns"),
        "PutEvents" => ("kuyruk", "aws.eventbridge"),
        "PutRecord" | "PutRecords" => ("kuyruk", "aws.kinesis"),
        "SendEmail" | "SendRawEmail" | "SendTemplatedEmail" | "SendBulkTemplatedEmail" | "SendBulkEmail" => ("eposta", "aws.ses"),
        "PutObject" | "GetObject" | "DeleteObject" | "DeleteObjects" | "CopyObject" | "HeadObject" | "ListObjectsV2" | "CreateMultipartUpload" => ("bulut", "aws.s3"),
        "PutItem" | "UpdateItem" | "DeleteItem" | "GetItem" | "Query" | "Scan" | "BatchWriteItem" => ("bulut", "aws.dynamodb"),
        "Invoke" => ("bulut", "aws.lambda"),
        _ => ("bulut", "aws"),
    }
}

/// Satırdaki dış çağrılar. `devam`: satır ve ardından gelen birkaç satır (çok satırlı argümanlar için)
fn satir_disleri(dil: Dil, satir: &str, devam: &str, b: &Baglam) -> Vec<Ham> {
    static D: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    let r = D.get_or_init(|| {
        let mut h = HashMap::new();
        let mut e = |ad: &'static str, s: &str| {
            h.insert(ad, Regex::new(s).unwrap());
        };
        // JS / TS
        e("fetch", r"(?:^|[^\w.$])(?:globalThis\.|window\.)?fetch\s*\(");
        e("axios_yontem", &format!(r"\baxios\s*\.\s*({YONTEMLER}|request)\s*(?:<[^>]*>)?\s*\("));
        e("axios", r"\baxios\s*\(");
        e("ornek_yontem", &format!(r"\b(?:this\s*\.\s*)?([A-Za-z_$][\w$]*)\s*\.\s*(?:axiosRef\s*\.\s*)?({YONTEMLER}|request)\s*(?:<[^>]*>)?\s*\("));
        e("got", &format!(r"\bgot\s*(?:\.\s*({YONTEMLER}))?\s*\("));
        e("node_http", r"\bhttps?\s*\.\s*(request|get)\s*\(");
        e("sendmail", r"\.\s*sendMail\s*\(");
        e("sendgrid", r"\b(?:sgMail|sendgrid|sg)\s*\.\s*(send|sendMultiple)\s*\(");
        e("stripe", r"\b([A-Za-z_$][\w$]*)\s*\.\s*([a-zA-Z]+)(?:\s*\.\s*[a-zA-Z]+)?\s*\.\s*(create|update|del|cancel|capture|confirm|refund|retrieve|list|search|attach|detach|pay|finalizeInvoice|voidInvoice|send|modify|delete)\s*\(");
        e("aws_v3", r"\.\s*send\s*\(\s*new\s+(\w+?)Command\s*\(");
        e("aws_v2", r"\.\s*(putObject|upload|deleteObject|deleteObjects|copyObject|getObject|headObject|sendMessage|publish|sendEmail|putItem|updateItem|deleteItem|invoke)\s*\(");
        e("gcp_pubsub", r"\.\s*(publish|publishMessage)\s*\(");
        e("gcp_storage", r"\.\s*(save|upload|delete|createWriteStream)\s*\(");
        e("kafka_js", r"\b(\w*[pP]roducer)\s*\.\s*send(?:Batch)?\s*\(");
        e("amqp_js", r"\.\s*(sendToQueue|publish)\s*\(");
        e("mikroservis", r"\b(?:this\s*\.\s*)?([A-Za-z_$][\w$]*)\s*\.\s*(emit|send)\s*\(");
        e("twilio", r"\.\s*messages\s*\.\s*create\s*\(");
        // Python
        e("requests", &format!(r"\brequests\s*\.\s*({YONTEMLER}|request)\s*\("));
        e("httpx", &format!(r"\bhttpx\s*\.\s*({YONTEMLER}|request|stream)\s*\("));
        e("py_ornek", &format!(r"\b(?:self\s*\.\s*)?(\w+)\s*\.\s*({YONTEMLER}|request|stream)\s*\("));
        e("urlopen", r"\b(?:urllib\s*\.\s*request\s*\.\s*)?urlopen\s*\(");
        e("smtplib", r"\bsmtplib\s*\.\s*SMTP(?:_SSL)?\s*\(");
        e("emails_send", r"\.\s*send\s*\(\s*(?:to\s*=|mail_to\s*=)");
        e("django_mail", r"\b(send_mail|send_mass_mail|mail_admins|mail_managers|EmailMessage|EmailMultiAlternatives)\s*\(");
        e("flask_mail", r"\bmail\s*\.\s*send(?:_message)?\s*\(");
        e("fastapi_mail", r"\.\s*send_message\s*\(");
        e("boto3_cagri", r"\b(\w+)\s*\.\s*([a-z_]+)\s*\(");
        e("stripe_py", r"\bstripe\s*\.\s*(\w+)\s*\.\s*(create|modify|delete|cancel|capture|confirm|retrieve|list|search|refund)\s*\(");
        e("kafka_py", r"\b(\w*producer)\s*\.\s*(send|produce)\s*\(");
        e("pika", r"\.\s*basic_publish\s*\(");
        // Java / Kotlin
        e("rest_template", r"\b(\w*[rR]estTemplate|\w*[rR]estOperations)\s*\.\s*(getForObject|getForEntity|postForObject|postForEntity|postForLocation|put|delete|exchange|patchForObject|execute)\s*\(");
        e("web_client", r"\b(\w*(?:[wW]ebClient|[rR]estClient)\w*)\s*\.\s*(get|post|put|patch|delete|head|method)\s*\(");
        e("http_request", r"\bHttpRequest\s*\.\s*newBuilder\s*\(");
        // Ktor istemcisi: `client.post("https://...") { }`, `client.request(url) { method = HttpMethod.Put }`
        e("ktor_client", r"\b(\w*[cC]lient)\s*\.\s*(get|post|put|patch|delete|head|options|request|submitForm|submitFormWithBinaryData)\s*(?:<[^<>()]*>)?\s*\(");
        e("kafka_template", r"\b(\w*[kK]afkaTemplate|\w*[kK]afkaProducer)\s*\.\s*send(?:Default)?\s*\(");
        e("rabbit_template", r"\b(\w*(?:[rR]abbit|[aA]mqp|[jJ]ms)Template)\s*\.\s*(convertAndSend|send)\s*\(");
        e("mail_sender", r"\b(\w*[mM]ail[sS]ender\w*)\s*\.\s*send\s*\(");
        e("aws_java", r"\b(\w*(?:[sS]qs|[sS]ns|[sS]3|[sS]es)\w*)\s*\.\s*(sendMessage|publish|putObject|deleteObject|sendEmail|getObject)\s*\(");
        // C#
        e("http_client", r"\b(\w+)\s*\.\s*(GetAsync|PostAsync|PutAsync|PatchAsync|DeleteAsync|SendAsync|GetStringAsync|GetByteArrayAsync|GetStreamAsync|GetFromJsonAsync|PostAsJsonAsync|PutAsJsonAsync|PatchAsJsonAsync|DeleteFromJsonAsync)\s*(?:<[^>]*>)?\s*\(");
        e("olay_yolu", r"\b(\w*(?:[pP]ublishEndpoint|[bB]us|[eE]vent[bB]us|[sS]endEndpoint|[pP]ublisher))\s*\.\s*(Publish|PublishAsync|Send|SendAsync)\s*(?:<[^>]*>)?\s*\(");
        e("smtp_cs", r"\b(\w*(?:[sS]mtp|[mM]ail)\w*)\s*\.\s*(Send|SendAsync|SendMailAsync)\s*\(");
        e("service_bus", r"\.\s*(SendMessageAsync|SendMessagesAsync|ScheduleMessageAsync)\s*\(");
        // Go
        e("go_http", r"\bhttp\s*\.\s*(Get|Post|PostForm|Head)\s*\(");
        e("go_istek", r"\bhttp\s*\.\s*NewRequest(?:WithContext)?\s*\(");
        e("resty", r"\.\s*(Get|Post|Put|Patch|Delete|Head)\s*\(");
        e("go_smtp", r"\bsmtp\s*\.\s*SendMail\s*\(");
        e("go_kafka", r"\.\s*(WriteMessages|SendMessage|SendMessages|Produce)\s*\(");
        e("go_amqp", r"\.\s*(Publish|PublishWithContext)\s*\(");
        e("go_aws", r"\.\s*(PutObject|SendMessage|Publish|SendEmail|DeleteObject)\s*\(");
        // PHP
        e("laravel_http", r"\bHttp::");
        e("laravel_fiil", &format!(r"(?:->|::)\s*({YONTEMLER}|send)\s*\("));
        e("guzzle", r"\$(\w+)(?:\s*->\s*(\w+))?\s*->\s*(get|post|put|patch|delete|head|request|getAsync|postAsync|requestAsync|send|sendAsync)\s*\(");
        e("laravel_mail", r"\bMail::(?:to|cc|bcc|send|raw|queue|later|mailer|html)\b");
        e("bildirim", r"(?:->\s*notify(?:Now)?\s*\(\s*new\s+\\?([\w\\]+)|\bNotification::send(?:Now)?\s*\()");
        e("curl", r"\bcurl_init\s*\(");
        e("stripe_php", r"\\?Stripe\\(\w+)::(create|update|retrieve|all|cancel|capture)\s*\(|\$\w+\s*->\s*(\w+)\s*->\s*(create|update|retrieve|all|cancel|capture)\s*\(");
        // Ruby
        e("net_http", r"\bNet::HTTP\s*\.\s*(get|get_response|post|post_form|start|new)\b");
        e("net_http_istek", r"\bNet::HTTP::(Get|Post|Put|Patch|Delete)\s*\.\s*new\s*\(");
        e("rb_istemci", &format!(r"\b(Faraday|HTTParty|RestClient|HTTP)\s*\.\s*({YONTEMLER})\b"));
        e("rb_ornek", &format!(r"\b(\w+)\s*\.\s*({YONTEMLER})\s*\("));
        e("mailer", r"\b(\w+Mailer)\s*\.\s*(?:with\([^)]*\)\s*\.\s*)?(\w+)(?:\([^)]*\))?\s*\.\s*deliver(_now|_later)?!?");
        e("stripe_rb", r"\bStripe::(\w+)\s*\.\s*(create|update|retrieve|list|delete|cancel|capture)\b");
        e("kafka_rb", r"\.\s*(deliver_message|produce_async|produce_sync|produce)\s*\(");
        h
    });
    let g = |ad: &str| &r[ad];
    let arg_sonra = |m: regex::Match| -> Option<String> { ilk_arguman(&devam[m.end()..]) };
    let mut v: Vec<Ham> = vec![];
    match dil {
        Dil::Js => {
            if let Some(m) = g("fetch").find(satir) {
                v.push(Ham::http("fetch", yontem_secenegi(&devam[m.end()..]).or(Some("GET".into())), arg_sonra(m)));
            }
            if let Some(c) = g("axios_yontem").captures(satir) {
                let m = c.get(0).unwrap();
                let y = if &c[1] == "request" { yontem_secenegi(&devam[m.end()..]) } else { yontem_buyuk(&c[1]) };
                let ifade = if &c[1] == "request" { url_secenegi(&devam[m.end()..]) } else { arg_sonra(m) };
                v.push(Ham::http("axios", y, ifade));
            } else if let Some(m) = g("axios").find(satir) {
                let a = &devam[m.end()..];
                let ifade = if a.trim_start().starts_with('{') { url_secenegi(a) } else { ilk_arguman(a) };
                v.push(Ham::http("axios", yontem_secenegi(a).or_else(|| (!a.trim_start().starts_with('{')).then(|| "GET".into())), ifade));
            }
            if v.is_empty() {
                for c in g("ornek_yontem").captures_iter(satir) {
                    let ad = &c[1];
                    let Some((tur, taban)) = b.ornekler.get(ad) else { continue };
                    if tur != "axios" && tur != "HttpService" {
                        continue;
                    }
                    let m = c.get(0).unwrap();
                    let y = if &c[2] == "request" { yontem_secenegi(&devam[m.end()..]) } else { yontem_buyuk(&c[2]) };
                    let ifade = arg_sonra(m).map(|a| tabanla(taban.as_deref(), &a));
                    v.push(Ham::http(if tur == "HttpService" { "HttpService" } else { "axios" }, y, ifade));
                }
            }
            if b.var("from 'got'") || b.var("from \"got\"") || b.var("require('got')") {
                if let Some(c) = g("got").captures(satir) {
                    let m = c.get(0).unwrap();
                    v.push(Ham::http("got", c.get(1).and_then(|x| yontem_buyuk(x.as_str())).or_else(|| yontem_secenegi(&devam[m.end()..])).or(Some("GET".into())), arg_sonra(m)));
                }
            }
            if ["'http'", "\"http\"", "'https'", "\"https\"", "'node:http'", "'node:https'", "\"node:http\"", "\"node:https\""].iter().any(|p| b.var(p)) {
                if let Some(c) = g("node_http").captures(satir) {
                    let m = c.get(0).unwrap();
                    let y = if &c[1] == "get" { Some("GET".into()) } else { yontem_secenegi(&devam[m.end()..]) };
                    v.push(Ham::http("node:http", y, arg_sonra(m).filter(|a| !a.starts_with('{')).or_else(|| url_secenegi(&devam[m.end()..]))));
                }
            }
            if g("sendmail").is_match(satir) {
                v.push(Ham::sdk("eposta", "nodemailer", "smtp", None, true));
            }
            if b.var("@sendgrid/mail") && g("sendgrid").is_match(satir) {
                v.push(Ham::sdk("eposta", "sendgrid", "sendgrid", None, true));
            }
            if b.var("stripe") {
                if let Some(c) = g("stripe").captures(satir) {
                    let ad = &c[1];
                    if ad.eq_ignore_ascii_case("stripe") || b.ornekler.get(ad).is_some_and(|o| o.0 == "stripe") {
                        v.push(Ham::sdk("odeme", "stripe", "stripe", Some(c[2].to_string()), !okuma_adi(&c[3])));
                    }
                }
            }
            if b.var("@aws-sdk/") {
                if let Some(c) = g("aws_v3").captures(satir) {
                    let (tur, sdk) = aws_komutu(&c[1]);
                    v.push(Ham::sdk(tur, "aws-sdk", sdk, Some(format!("{}Command", &c[1])), !okuma_adi(&c[1])));
                }
            } else if b.var("aws-sdk") {
                if let Some(c) = g("aws_v2").captures(satir) {
                    let a = &c[1];
                    let (tur, sdk) = match a {
                        "sendMessage" => ("kuyruk", "aws.sqs"),
                        "publish" => ("kuyruk", "aws.sns"),
                        "sendEmail" => ("eposta", "aws.ses"),
                        "putItem" | "updateItem" | "deleteItem" => ("bulut", "aws.dynamodb"),
                        "invoke" => ("bulut", "aws.lambda"),
                        _ => ("bulut", "aws.s3"),
                    };
                    v.push(Ham::sdk(tur, "aws-sdk", sdk, None, !okuma_adi(a)));
                }
            }
            if b.var("@google-cloud/pubsub") {
                if let Some(c) = g("gcp_pubsub").captures(satir) {
                    v.push(Ham::sdk("kuyruk", "@google-cloud/pubsub", "gcp.pubsub", None, !okuma_adi(&c[1])));
                }
            }
            if b.var("@google-cloud/storage") {
                if let Some(c) = g("gcp_storage").captures(satir) {
                    v.push(Ham::sdk("bulut", "@google-cloud/storage", "gcp.storage", None, !okuma_adi(&c[1])));
                }
            }
            if b.var("kafkajs") && g("kafka_js").is_match(satir) {
                v.push(Ham::sdk("kuyruk", "kafkajs", "kafka", konu(devam), true));
            }
            if b.var("amqplib") {
                if let Some(c) = g("amqp_js").captures(satir) {
                    let m = c.get(0).unwrap();
                    let a = argumanlar(&devam[m.end()..]);
                    let kanal = a.first().and_then(|x| sabit_metin(x)).filter(|x| !x.is_empty()).or_else(|| a.get(1).and_then(|x| sabit_metin(x)));
                    v.push(Ham::sdk("kuyruk", "amqplib", "rabbitmq", kanal, true));
                }
            }
            for c in g("mikroservis").captures_iter(satir) {
                if b.ornekler.get(&c[1]).is_some_and(|o| o.0 == "mikroservis") {
                    let m = c.get(0).unwrap();
                    v.push(Ham::sdk("kuyruk", "@nestjs/microservices", "mikroservis", arg_sonra(m).and_then(|a| sabit_metin(&a)), true));
                }
            }
            if b.var("twilio") && g("twilio").is_match(satir) {
                v.push(Ham::sdk("bildirim", "twilio", "twilio", None, true));
            }
        }
        Dil::Py => {
            if let Some(c) = g("requests").captures(satir) {
                let m = c.get(0).unwrap();
                let a = argumanlar(&devam[m.end()..]);
                let (y, ifade) = if &c[1] == "request" {
                    (a.first().and_then(|x| sabit_metin(x)).and_then(|x| yontem_buyuk(&x)), a.get(1).cloned())
                } else {
                    (yontem_buyuk(&c[1]), a.first().cloned())
                };
                v.push(Ham::http("requests", y, ifade.map(|i| anahtar_atla(&i))));
            } else if let Some(c) = g("httpx").captures(satir) {
                let m = c.get(0).unwrap();
                v.push(Ham::http("httpx", yontem_buyuk(&c[1]), arg_sonra(m).map(|i| anahtar_atla(&i))));
            } else {
                for c in g("py_ornek").captures_iter(satir) {
                    let Some((tur, taban)) = b.ornekler.get(&c[1]) else { continue };
                    if !matches!(tur.as_str(), "requests" | "httpx" | "aiohttp") {
                        continue;
                    }
                    let m = c.get(0).unwrap();
                    let ifade = arg_sonra(m).map(|a| tabanla(taban.as_deref(), &anahtar_atla(&a)));
                    v.push(Ham::http(tur, yontem_buyuk(&c[2]), ifade));
                }
            }
            if b.var("urlopen") {
                if let Some(m) = g("urlopen").find(satir) {
                    v.push(Ham::http("urllib", yontem_secenegi(devam).or(Some("GET".into())), arg_sonra(m)));
                }
            }
            if let Some(m) = g("smtplib").find(satir) {
                let h = arg_sonra(m);
                let mut x = Ham::sdk("eposta", "smtplib", "smtp", None, true);
                x.ifade = h;
                v.push(x);
            }
            if b.var("import emails") && g("emails_send").is_match(satir) {
                v.push(Ham::sdk("eposta", "emails", "smtp", None, true));
            }
            if b.var("django.core.mail") {
                if let Some(c) = g("django_mail").captures(satir) {
                    v.push(Ham::sdk("eposta", &format!("django.core.mail.{}", &c[1]), "smtp", None, true));
                }
            }
            if b.var("flask_mail") && g("flask_mail").is_match(satir) {
                v.push(Ham::sdk("eposta", "flask_mail", "smtp", None, true));
            }
            if b.var("fastapi_mail") && g("fastapi_mail").is_match(satir) {
                v.push(Ham::sdk("eposta", "fastapi_mail", "smtp", None, true));
            }
            if b.var("boto3") {
                for c in g("boto3_cagri").captures_iter(satir) {
                    let Some((tur, _)) = b.ornekler.get(&c[1]) else { continue };
                    let Some(hizmet) = tur.strip_prefix("boto3:") else { continue };
                    let yontem = &c[2];
                    if matches!(yontem, "close" | "meta") {
                        continue;
                    }
                    let t = aws_turu(hizmet);
                    let t = if hizmet == "ses" || hizmet == "sesv2" { "eposta" } else { t };
                    v.push(Ham::sdk(t, "boto3", &format!("aws.{hizmet}"), Some(yontem.to_string()), !okuma_adi(yontem)));
                }
            }
            if let Some(c) = g("stripe_py").captures(satir) {
                v.push(Ham::sdk("odeme", "stripe", "stripe", Some(c[1].to_string()), !okuma_adi(&c[2])));
            }
            if (b.var("kafka") || b.var("confluent_kafka")) && g("kafka_py").is_match(satir) {
                let m = g("kafka_py").find(satir).unwrap();
                v.push(Ham::sdk("kuyruk", "kafka", "kafka", arg_sonra(m).and_then(|a| sabit_metin(&a)), true));
            }
            if b.var("pika") && g("pika").is_match(satir) {
                v.push(Ham::sdk("kuyruk", "pika", "rabbitmq", py_anahtar(devam, "routing_key"), true));
            }
        }
        Dil::Jvm => {
            if let Some(c) = g("rest_template").captures(satir) {
                let m = c.get(0).unwrap();
                let y = if &c[2] == "exchange" || &c[2] == "execute" { yontem_secenegi(&devam[m.end()..]) } else { yontem_buyuk(&c[2]) };
                v.push(Ham::http("RestTemplate", y, arg_sonra(m)));
            }
            if let Some(c) = g("web_client").captures(satir) {
                let istemci = if c[1].to_lowercase().contains("webclient") { "WebClient" } else { "RestClient" };
                let ifade = uri_secenegi(devam);
                let y = if &c[2] == "method" { yontem_secenegi(devam) } else { yontem_buyuk(&c[2]) };
                let mut x = Ham::http(istemci, y, ifade);
                x.alici = Some(c[1].to_string());
                v.push(x);
            }
            if b.var("io.ktor.client") {
                let istemci = g("ktor_client").captures(satir).filter(|c| !c[1].to_lowercase().contains("webclient") && !c[1].to_lowercase().contains("restclient"));
                if let Some(c) = istemci {
                    let m = c.get(0).unwrap();
                    let y = match &c[2] {
                        "request" => yontem_secenegi(devam).or(Some("GET".into())),
                        "submitForm" | "submitFormWithBinaryData" => Some("POST".into()),
                        x => yontem_buyuk(x),
                    };
                    // Göreli adres: istemcinin `defaultRequest { url(...) }` tabanı (`tabanlari_uygula`)
                    let mut x = Ham::http("Ktor HttpClient", y, arg_sonra(m));
                    x.alici = Some(c[1].to_string());
                    v.push(x);
                }
            }
            if g("http_request").is_match(satir) {
                let y = Regex::new(r"\.\s*(GET|POST|PUT|DELETE)\s*\(").ok().and_then(|r| r.captures(devam).map(|c| c[1].to_string())).or_else(|| yontem_secenegi(devam));
                v.push(Ham::http("java.net.http", y, uri_secenegi(devam)));
            }
            if let Some(m) = g("kafka_template").find(satir) {
                v.push(Ham::sdk("kuyruk", "KafkaTemplate", "kafka", arg_sonra(m).and_then(|a| sabit_metin(&a)), true));
            }
            if let Some(c) = g("rabbit_template").captures(satir) {
                let m = c.get(0).unwrap();
                let sdk = if c[1].to_lowercase().contains("jms") { "jms" } else { "rabbitmq" };
                let a = argumanlar(&devam[m.end()..]);
                v.push(Ham::sdk("kuyruk", &c[1], sdk, a.first().and_then(|x| sabit_metin(x)), true));
            }
            if g("mail_sender").is_match(satir) {
                v.push(Ham::sdk("eposta", "JavaMailSender", "smtp", None, true));
            }
            if b.var("amazon") {
                if let Some(c) = g("aws_java").captures(satir) {
                    let a = &c[2];
                    let (tur, sdk) = match a {
                        "sendMessage" => ("kuyruk", "aws.sqs"),
                        "publish" => ("kuyruk", "aws.sns"),
                        "sendEmail" => ("eposta", "aws.ses"),
                        _ => ("bulut", "aws.s3"),
                    };
                    v.push(Ham::sdk(tur, "aws-sdk", sdk, None, !okuma_adi(a)));
                }
            }
        }
        Dil::Cs => {
            if b.var("httpclient") {
                if let Some(c) = g("http_client").captures(satir) {
                    let alici = c[1].to_ascii_lowercase();
                    if alici.contains("client") || alici.contains("http") {
                        let m = c.get(0).unwrap();
                        let a = &c[2];
                        let (y, ifade) = if a == "SendAsync" { (yontem_secenegi(devam), istek_mesaji(devam)) } else { (yontem_buyuk(a), arg_sonra(m)) };
                        v.push(Ham::http("HttpClient", y, ifade));
                    }
                }
            }
            if let Some(c) = g("olay_yolu").captures(satir) {
                let sdk = if b.var("masstransit") { "masstransit" } else { "olay-yolu" };
                v.push(Ham::sdk("kuyruk", &c[1], sdk, None, true));
            }
            if (b.var("smtpclient") || b.var("mailkit")) && g("smtp_cs").is_match(satir) {
                v.push(Ham::sdk("eposta", if b.var("mailkit") { "MailKit" } else { "SmtpClient" }, "smtp", None, true));
            }
            if b.var("azure.messaging.servicebus") && g("service_bus").is_match(satir) {
                v.push(Ham::sdk("kuyruk", "ServiceBusSender", "azure.servicebus", None, true));
            }
        }
        Dil::Go => {
            if let Some(c) = g("go_http").captures(satir) {
                let m = c.get(0).unwrap();
                v.push(Ham::http("net/http", yontem_buyuk(&c[1]), arg_sonra(m)));
            } else if let Some(m) = g("go_istek").find(satir) {
                let a = argumanlar(&devam[m.end()..]);
                // NewRequestWithContext(ctx, yöntem, adres, gövde) / NewRequest(yöntem, adres, gövde)
                let i = if m.as_str().contains("WithContext") { 1 } else { 0 };
                let y = a.get(i).and_then(|x| sabit_metin(x).or_else(|| x.rsplit('.').next().map(|s| s.trim_start_matches("Method").to_string()))).and_then(|x| yontem_buyuk(&x));
                v.push(Ham::http("net/http", y, a.get(i + 1).cloned()));
            } else if b.var("go-resty/resty") && (satir.contains(".R()") || satir.contains("SetBody") || satir.contains("SetResult")) {
                if let Some(c) = g("resty").captures(satir) {
                    let m = c.get(0).unwrap();
                    v.push(Ham::http("resty", yontem_buyuk(&c[1]), arg_sonra(m)));
                }
            }
            if let Some(m) = g("go_smtp").find(satir) {
                let mut x = Ham::sdk("eposta", "net/smtp", "smtp", None, true);
                x.ifade = arg_sonra(m);
                v.push(x);
            }
            if (b.var("segmentio/kafka-go") || b.var("sarama") || b.var("confluent-kafka-go")) && g("go_kafka").is_match(satir) {
                v.push(Ham::sdk("kuyruk", "kafka", "kafka", None, true));
            }
            if b.var("amqp") && g("go_amqp").is_match(satir) {
                let m = g("go_amqp").find(satir).unwrap();
                let a = argumanlar(&devam[m.end()..]);
                let i = if m.as_str().contains("WithContext") { 1 } else { 0 };
                let kanal = a.get(i).and_then(|x| sabit_metin(x)).filter(|x| !x.is_empty()).or_else(|| a.get(i + 1).and_then(|x| sabit_metin(x)));
                v.push(Ham::sdk("kuyruk", "amqp", "rabbitmq", kanal, true));
            }
            if b.var("aws-sdk-go") {
                if let Some(c) = g("go_aws").captures(satir) {
                    let a = &c[1];
                    let (tur, sdk) = aws_komutu(a);
                    v.push(Ham::sdk(tur, "aws-sdk-go", sdk, None, !okuma_adi(a)));
                }
            }
        }
        Dil::Php => {
            if let Some(m) = g("laravel_http").find(satir) {
                let a = &devam[m.start()..];
                // Zincirdeki ilk fiil: Http::withToken($t)->post($adres, [...])
                if let Some(c) = g("laravel_fiil").captures(a) {
                    let f = c.get(0).unwrap();
                    let args = argumanlar(&a[f.end()..]);
                    let (y, ifade) = if &c[1] == "send" {
                        (args.first().and_then(|x| sabit_metin(x)).and_then(|x| yontem_buyuk(&x)), args.get(1).cloned())
                    } else {
                        (yontem_buyuk(&c[1]), args.first().cloned())
                    };
                    let taban = Regex::new(r"baseUrl\s*\(([^)]*)\)").ok().and_then(|r| r.captures(a).map(|c| c[1].trim().to_string()));
                    v.push(Ham::http("Http", y, ifade.map(|i| tabanla(taban.as_deref(), &i))));
                }
            }
            if b.var("guzzlehttp") {
                if let Some(c) = g("guzzle").captures(satir) {
                    let ad = c.get(2).map(|x| x.as_str()).unwrap_or(&c[1]);
                    let alt = ad.to_ascii_lowercase();
                    if alt.contains("client") || alt.contains("http") || alt.contains("guzzle") {
                        let m = c.get(0).unwrap();
                        let args = argumanlar(&devam[m.end()..]);
                        let f = c[3].trim_end_matches("Async");
                        let (y, ifade) = if f == "request" || f == "send" {
                            (args.first().and_then(|x| sabit_metin(x)).and_then(|x| yontem_buyuk(&x)), args.get(1).cloned())
                        } else {
                            (yontem_buyuk(f), args.first().cloned())
                        };
                        let taban = b.ornekler.get(&c[1]).and_then(|o| o.1.clone());
                        v.push(Ham::http("Guzzle", y, ifade.map(|i| tabanla(taban.as_deref(), &i))));
                    }
                }
            }
            if g("laravel_mail").is_match(satir) {
                let mut x = Ham::sdk("eposta", "Mail", "smtp", None, true);
                x.ertelenmis = satir.contains("Mail::queue") || satir.contains("Mail::later") || devam.lines().take(3).any(|l| l.contains("->queue(") || l.contains("->later("));
                v.push(x);
            }
            if let Some(c) = g("bildirim").captures(satir) {
                v.push(Ham::sdk("bildirim", "Notification", "bildirim", c.get(1).map(|x| x.as_str().rsplit('\\').next().unwrap_or("").to_string()), true));
            }
            if let Some(m) = g("curl").find(satir) {
                v.push(Ham::http("curl", None, arg_sonra(m)));
            }
            if b.var("stripe") {
                if let Some(c) = g("stripe_php").captures(satir) {
                    let (nesne, f) = (c.get(1).or(c.get(3)).map(|x| x.as_str()).unwrap_or(""), c.get(2).or(c.get(4)).map(|x| x.as_str()).unwrap_or(""));
                    v.push(Ham::sdk("odeme", "stripe", "stripe", Some(nesne.to_string()), !okuma_adi(f)));
                }
            }
        }
        Dil::Rb => {
            if let Some(c) = g("net_http_istek").captures(satir) {
                let m = c.get(0).unwrap();
                v.push(Ham::http("Net::HTTP", yontem_buyuk(&c[1]), arg_sonra(m)));
            } else if let Some(c) = g("net_http").captures(satir) {
                let a = &devam[c.get(0).unwrap().end()..];
                let ifade = ilk_arguman(a.trim_start().trim_start_matches('(')).or_else(|| a.split_whitespace().next().map(String::from));
                v.push(Ham::http("Net::HTTP", yontem_buyuk(&c[1]), ifade));
            }
            if let Some(c) = g("rb_istemci").captures(satir) {
                let a = &devam[c.get(0).unwrap().end()..];
                let ifade = ilk_arguman(a.trim_start().trim_start_matches('(')).or_else(|| a.split_whitespace().next().map(|s| s.trim_end_matches(',').to_string()));
                v.push(Ham::http(&c[1], yontem_buyuk(&c[2]), ifade));
            } else {
                for c in g("rb_ornek").captures_iter(satir) {
                    let Some((tur, taban)) = b.ornekler.get(&c[1]) else { continue };
                    let m = c.get(0).unwrap();
                    v.push(Ham::http(tur, yontem_buyuk(&c[2]), arg_sonra(m).map(|a| tabanla(taban.as_deref(), &a))));
                }
            }
            if let Some(c) = g("mailer").captures(satir) {
                let mut x = Ham::sdk("eposta", "ActionMailer", "smtp", Some(format!("{}#{}", &c[1], &c[2])), true);
                x.ertelenmis = c.get(3).is_some_and(|d| d.as_str() == "_later");
                v.push(x);
            }
            if let Some(c) = g("stripe_rb").captures(satir) {
                v.push(Ham::sdk("odeme", "stripe", "stripe", Some(c[1].to_string()), !okuma_adi(&c[2])));
            }
            if (b.var("kafka") || b.var("karafka") || b.var("waterdrop")) && g("kafka_rb").is_match(satir) {
                v.push(Ham::sdk("kuyruk", "kafka", "kafka", konu(devam), true));
            }
        }
    }
    v
}

/// `{ url: x }` / `url=x` seçeneği
fn url_secenegi(s: &str) -> Option<String> {
    static D: OnceLock<Regex> = OnceLock::new();
    let r = D.get_or_init(|| Regex::new(r#"\b(?:url|uri|href|baseURL)\s*[:=]\s*([^,}\n]+)"#).unwrap());
    r.captures(s).map(|c| c[1].trim().to_string())
}

/// `.uri("...")` / `URI.create("...")` / `URI("...")`
fn uri_secenegi(s: &str) -> Option<String> {
    static D: OnceLock<Regex> = OnceLock::new();
    let r = D.get_or_init(|| Regex::new(r"(?:\.\s*uri|URI\s*\.\s*create|\bURI|newBuilder)\s*\(").unwrap());
    let m = r.find(s)?;
    ilk_arguman(&s[m.end()..])
}

/// `new HttpRequestMessage(HttpMethod.Post, adres)` içindeki adres
fn istek_mesaji(s: &str) -> Option<String> {
    let i = s.find("HttpRequestMessage(")?;
    argumanlar(&s[i + "HttpRequestMessage(".len()..]).get(1).cloned()
}

/// Kafka konusu: `topic: 'x'` / `topic='x'`
fn konu(s: &str) -> Option<String> {
    static D: OnceLock<Regex> = OnceLock::new();
    let r = D.get_or_init(|| Regex::new(r#"\btopic\s*[:=]\s*['"]([^'"]+)['"]"#).unwrap());
    r.captures(s).map(|c| c[1].to_string())
}

/// Python anahtar argüman metni (`routing_key="x"`)
fn py_anahtar(s: &str, ad: &str) -> Option<String> {
    let i = s.find(&format!("{ad}="))?;
    sabit_metin(argumanlar(&s[i + ad.len() + 1..]).first()?)
}

/// `url=adres` biçimindeki anahtar argümanın değeri
fn anahtar_atla(a: &str) -> String {
    match a.split_once('=') {
        Some((k, d)) if k.trim().chars().all(|c| c.is_alphanumeric() || c == '_') && !d.starts_with('=') => d.trim().to_string(),
        _ => a.to_string(),
    }
}

/// Taban adresli istemcide göreli adres: taban + yol
fn tabanla(taban: Option<&str>, a: &str) -> String {
    match taban {
        Some(t) if sabit_metin(a).is_some_and(|x| x.starts_with('/') || !x.contains("://")) => format!("{t} + {a}"),
        _ => a.to_string(),
    }
}

/// Tırnaklı sabit metin (`'x'`, `"x"`, değişkensiz şablon) ise içeriği
fn sabit_metin(a: &str) -> Option<String> {
    let a = a.trim();
    let a = a.strip_prefix(['f', 'r', 'b', '@', '$']).filter(|x| x.starts_with(['"', '\''])).unwrap_or(a);
    let ilk = a.chars().next()?;
    if !matches!(ilk, '"' | '\'' | '`') || a.len() < 2 || !a.ends_with(ilk) {
        return None;
    }
    Some(a[1..a.len() - 1].to_string())
}

// ---------------------------------------------------------------------------------------------------------------
// Hedef çözümleme

/// Ortam değişkeni / yapılandırma okuması: değişkenin adı
fn ortam_adi(ifade: &str) -> Option<String> {
    static D: OnceLock<Vec<Regex>> = OnceLock::new();
    let d = D.get_or_init(|| {
        [
            r"process\s*\.\s*env\s*(?:\.\s*([A-Za-z_]\w*)|\[\s*['\x22]([^'\x22]+)['\x22]\s*\])",
            r#"os\s*\.\s*(?:environ\s*(?:\.\s*get\s*\(|\[)|getenv\s*\()\s*['"]([^'"]+)['"]"#,
            r#"\b(?:env|getenv)\s*\(\s*['"]([^'"]+)['"]"#,
            r#"\bconfig\s*\(\s*['"]([^'"]+)['"]"#,
            r#"\bENV\s*(?:\[|\.fetch\s*\()\s*['"]([^'"]+)['"]"#,
            r#"System\s*\.\s*getenv\s*\(\s*"([^"]+)""#,
            r#"Environment\s*\.\s*GetEnvironmentVariable\s*\(\s*"([^"]+)""#,
            r#"os\s*\.\s*Getenv\s*\(\s*"([^"]+)""#,
            r#"(?i)\b\w*configuration\w*\s*(?:\[\s*"([^"]+)"\s*\]|\.\s*GetValue\s*<[^>]*>\s*\(\s*"([^"]+)"|\.\s*GetConnectionString\s*\(\s*"([^"]+)")"#,
            r#"\$\{([A-Za-z_][\w.\-]*)(?::[^}]*)?\}"#,
            r"\b(?:settings|config|conf|cfg|app\.config|current_app\.config|self\.settings|self\.config|this\.config|Rails\.configuration)\s*(?:\.\s*([A-Za-z_]\w*)|\[\s*['\x22]([^'\x22]+)['\x22]\s*\])",
            r#"configService\s*\.\s*get(?:OrThrow)?\s*(?:<[^>]*>)?\s*\(\s*['"]([^'"]+)['"]"#,
        ]
        .iter()
        .map(|s| Regex::new(s).unwrap())
        .collect()
    });
    for r in d {
        if let Some(c) = r.captures(ifade) {
            if let Some(a) = c.iter().skip(1).flatten().next() {
                return Some(a.as_str().to_string());
            }
        }
    }
    None
}

/// Adresten ana makine ve yol: `https://api.paypal.com/v1/x?y` -> ("api.paypal.com", "/v1/x")
fn adres_parcala(u: &str) -> Option<(String, Option<String>)> {
    let i = u.find("://")?;
    let kalan = &u[i + 3..];
    let son = kalan.find(['/', '?', '#']).unwrap_or(kalan.len());
    let ana = kalan[..son].rsplit('@').next().unwrap_or("").to_ascii_lowercase();
    if ana.is_empty() || ana.contains(['{', '$', '}', '<', ' ', '%']) {
        return None;
    }
    let yol = kalan[son..].split(['?', '#']).next().filter(|y| y.len() > 1).map(String::from);
    Some((ana, yol))
}

/// Çözülmüş hedef: (hedef, tür, yol, güven)
type Cozum = (String, &'static str, Option<String>, &'static str);

/// Hedef ifadesini çözer: dize sabiti, şablon / birleştirme, ortam değişkeni, aynı fonksiyonda ya da dosyada atanmış
/// değişken (en çok 4 adım). Göreli yol `Some(("", "goreli", ...))` döner: uygulamanın kendisine gider
fn hedef_coz(ifade: &str, fonk: &str, dosya: &str, derinlik: usize) -> Cozum {
    let dinamik = || ("dinamik".to_string(), "dinamik", None, "unknown");
    let e = ifade.trim().trim_end_matches(';').trim();
    if e.is_empty() || derinlik > 4 {
        return dinamik();
    }
    // Doğrudan ortam / yapılandırma okuması (dize ve şablonlar aşağıda çözülür)
    if !e.starts_with(['"', '\'', '`']) && !e.starts_with("f\"") && !e.starts_with("f'") {
        if let Some(a) = ortam_adi(e) {
            return (a, "ortam", yol_kuyrugu(e), "inferred");
        }
    }
    // `new URL('yol', taban)`: taban çözülür
    if let Some(i) = e.find("new URL(") {
        let a = argumanlar(&e[i + 8..]);
        if let Some(t) = a.get(1) {
            let (h, tur, _, g) = hedef_coz(t, fonk, dosya, derinlik + 1);
            return (h, tur, a.first().and_then(|x| sabit_metin(x)).map(|y| format!("/{}", y.trim_start_matches('/'))), g);
        }
        if let Some(t) = a.first() {
            return hedef_coz(t, fonk, dosya, derinlik + 1);
        }
    }
    // Dize sabiti (değişkensiz)
    if let Some(s) = sabit_metin(e).filter(|s| !s.contains("${") && !(e.starts_with(['f', '$']) && s.contains('{')) && !s.contains("#{")) {
        return match adres_parcala(&s) {
            Some((ana, yol)) => (ana, "url", yol, "certain"),
            // Göreli adres: `/api/x` ya da typed istemcinin taban adresine eklenen `api/catalog/`
            None if s.starts_with('/') || (!s.is_empty() && !s.contains(char::is_whitespace) && s.contains('/') && !s.starts_with('.')) => {
                (String::new(), "goreli", Some(format!("/{}", s.trim_start_matches('/'))), "certain")
            }
            None => dinamik(),
        };
    }
    // Şablon / f-dizesi / Ruby ara değer: baştaki değişken taban adrestir
    static SAB: OnceLock<Regex> = OnceLock::new();
    let sab = SAB.get_or_init(|| Regex::new(r#"^(?:[fF]|\$)?[`"']\s*(?:\$\{([^}]+)\}|\{\$?([^}]+)\}|#\{([^}]+)\}|\$([A-Za-z_]\w*))([^`"']*)"#).unwrap());
    if let Some(c) = sab.captures(e) {
        let ic = c.get(1).or(c.get(2)).or(c.get(3)).or(c.get(4)).map(|x| x.as_str()).unwrap_or("");
        let kuyruk = c.get(5).map(|x| x.as_str().to_string()).filter(|y| y.starts_with('/'));
        let (h, tur, _, g) = hedef_coz(ic, fonk, dosya, derinlik + 1);
        return (h, tur, kuyruk.map(|y| temiz_yol(&y)), g);
    }
    // Şablonun içinde sabit adres: `https://api.x.com/${id}`
    if e.starts_with(['`', 'f', '"', '\'']) {
        if let Some(i) = e.find("://") {
            let ham = &e[e[..i].rfind(['`', '"', '\'']).map(|j| j + 1).unwrap_or(0)..];
            if let Some((ana, yol)) = adres_parcala(ham.trim_end_matches(['`', '"', '\''])) {
                return (ana, "url", yol.map(|y| temiz_yol(&y)), "certain");
            }
        }
        return dinamik();
    }
    // Birleştirme: `TABAN + '/yol'`, `$taban . '/yol'`
    for ayrac in [" + ", " . ", "+", "."] {
        if ayrac == "." && !e.starts_with('$') {
            continue;
        }
        if let Some((sol, sag)) = e.split_once(ayrac) {
            if sol.trim().is_empty() || sol.contains('(') && !sol.contains(')') {
                continue;
            }
            let (h, tur, _, g) = hedef_coz(sol, fonk, dosya, derinlik + 1);
            if tur != "dinamik" {
                let y = sabit_metin(sag.trim()).filter(|y| y.starts_with('/'));
                return (h, tur, y, g);
            }
            break;
        }
    }
    // Değişken: aynı fonksiyonda, sonra dosyada atanmış değer
    let ad = e.trim_start_matches("this.").trim_start_matches("self.").trim_start_matches("$this->").trim_start_matches('$').trim_start_matches('@');
    if !ad.is_empty() && ad.chars().all(|c| c.is_alphanumeric() || c == '_') {
        if let Some(d) = atanan(fonk, ad).or_else(|| atanan(dosya, ad)) {
            return hedef_coz(&d, fonk, dosya, derinlik + 1);
        }
        // `const { PAYPAL_API_URL } = process.env`
        let yapi = Regex::new(&format!(r"\{{[^}}]*\b{}\b[^}}]*\}}\s*=\s*process\s*\.\s*env", regex::escape(ad))).ok();
        if yapi.is_some_and(|r| r.is_match(dosya)) {
            return (ad.to_string(), "ortam", None, "inferred");
        }
        // Spring `@Value("${x.y}") private String ad;`
        let deger = Regex::new(&format!(r#"@Value\s*\(\s*"\$\{{([^}}:]+)[^"]*"\s*\)\s*(?:private|protected|public)?\s*(?:final\s+)?\w+\s+{}\b"#, regex::escape(ad))).ok();
        if let Some(c) = deger.and_then(|r| r.captures(dosya)) {
            return (c[1].to_string(), "ortam", None, "inferred");
        }
    }
    dinamik()
}

/// Yolun değişkenli kısımlarını `{}` ile gösterir: `/v2/checkout/orders/${id}` -> `/v2/checkout/orders/{}`
fn temiz_yol(y: &str) -> String {
    static D: OnceLock<Regex> = OnceLock::new();
    let r = D.get_or_init(|| Regex::new(r"\$\{[^}]*\}|\{[^}]*\}|#\{[^}]*\}").unwrap());
    r.replace_all(y, "{}").split(['?', '#']).next().unwrap_or("").to_string()
}

/// İfadenin sonundaki sabit yol: `TABAN + '/x'`
fn yol_kuyrugu(e: &str) -> Option<String> {
    let (_, sag) = e.rsplit_once('+')?;
    sabit_metin(sag.trim()).filter(|y| y.starts_with('/'))
}

/// `ad = değer` ataması (const / let / var / val / tipli bildirim / Python / PHP / Ruby): ilk eşleşmenin değeri
fn atanan(metin: &str, ad: &str) -> Option<String> {
    let r = Regex::new(&format!(r"(?m)(?:^|[\s;(,.{{])\$?{}\s*(?::\s*[\w<>\[\]|. ?]+)?\s*(?::=|=)\s*([^=;\n][^;\n]*)", regex::escape(ad))).ok()?;
    let c = r.captures(metin)?;
    let d = c[1].trim().trim_end_matches(',').to_string();
    (!d.is_empty() && !d.starts_with('>')).then_some(d)
}

// ---------------------------------------------------------------------------------------------------------------
// Ön koşullar, işlem sınırları, silme ve yumuşak silme

/// Ad ve sınıftan HTTP kodu (ör. `ModelNotFoundException` 404, `AccessDeniedException` 403)
fn ad_kodu(ad: &str) -> Option<u16> {
    if let Some(k) = crate::dna::kod_ifadesi(ad) {
        return Some(k);
    }
    let a = ad.rsplit(['.', '\\', ':']).next().unwrap_or(ad).to_ascii_lowercase();
    Some(if a.contains("notfound") || a == "http404" || a.contains("doesnotexist") || a.contains("recordnotfound") {
        404
    } else if a.contains("forbidden") || a.contains("accessdenied") || a.contains("notauthorized") || a.contains("permissiondenied") || a.contains("authorization") {
        403
    } else if a.contains("unauthorized") || a.contains("unauthenticated") || a.contains("authentication") {
        401
    } else if a.contains("conflict") || a.contains("alreadyexists") || a.contains("duplicate") {
        409
    } else if a.contains("unprocessable") || a.contains("recordinvalid") || a.contains("validationexception") {
        422
    } else if a.contains("badrequest") || a.contains("validation") || a.contains("invalid") || a.contains("illegalargument") {
        400
    } else if a.contains("toomany") || a.contains("throttle") {
        429
    } else if a.contains("gone") {
        410
    } else {
        return None;
    })
}

/// Rails / Rack durum simgesi -> kod
fn simge_kodu(s: &str) -> Option<u16> {
    Some(match s {
        "bad_request" => 400,
        "unauthorized" => 401,
        "forbidden" => 403,
        "not_found" => 404,
        "conflict" => 409,
        "gone" => 410,
        "unprocessable_entity" | "unprocessable_content" => 422,
        "too_many_requests" => 429,
        _ => return s.parse().ok().filter(|k| (400..600).contains(k)),
    })
}

/// Satırın ya da üstündeki birkaç satırın koşulu
fn kosul(dil: Dil, satirlar: &[&str], i: usize) -> Option<String> {
    static D: OnceLock<[Regex; 5]> = OnceLock::new();
    let [ayni, ust, py_ust, rb_son, rb_ust] = D.get_or_init(|| {
        [
            Regex::new(r"^\s*(?:\}\s*)?(?:else\s+)?if\s*\((.+)\)\s*\{?\s*(?:throw|return|abort|raise)").unwrap(),
            Regex::new(r"^\s*(?:\}\s*)?(?:else\s*)?(?:if|elseif|unless)\s*\(?(.+?)\)?\s*(?:\{|:|then)?\s*$").unwrap(),
            Regex::new(r"^(\s*)(?:el)?if\s+(.+?):\s*(?:raise\b.*)?$").unwrap(),
            Regex::new(r"\b(if|unless)\s+(.+?)\s*$").unwrap(),
            Regex::new(r"^\s*(if|unless|elsif)\s+(.+)$").unwrap(),
        ]
    });
    let s = satirlar[i];
    if let Some(c) = ayni.captures(s) {
        return Some(crate::dna::kisa(&c[1]));
    }
    match dil {
        Dil::Py => {
            if let Some(c) = py_ust.captures(s) {
                return Some(crate::dna::kisa(&c[2]));
            }
            let g = s.len() - s.trim_start().len();
            for j in (i.saturating_sub(4)..i).rev() {
                let t = satirlar[j];
                if t.trim().is_empty() {
                    continue;
                }
                if let Some(c) = py_ust.captures(t).filter(|c| c[1].len() < g) {
                    return Some(crate::dna::kisa(&c[2]));
                }
                if t.len() - t.trim_start().len() < g {
                    return None;
                }
            }
            None
        }
        Dil::Rb => {
            if let Some(c) = rb_son.captures(s) {
                let k = crate::dna::kisa(&c[2]);
                return Some(if &c[1] == "unless" { format!("!({k})") } else { k });
            }
            for j in (i.saturating_sub(3)..i).rev() {
                if let Some(c) = rb_ust.captures(satirlar[j]).map(|c| (c[1].to_string(), c[2].to_string())) {
                    let k = crate::dna::kisa(&c.1);
                    return Some(if c.0 == "unless" { format!("!({k})") } else { k });
                }
            }
            None
        }
        _ => {
            for j in (i.saturating_sub(3)..i).rev() {
                let t = satirlar[j].trim();
                if t.is_empty() {
                    continue;
                }
                if let Some(c) = ust.captures(satirlar[j]) {
                    // Parantezli koşul dengeli parantezle alınır (`if (uniqueKeys.has(key)) {`)
                    let satir = satirlar[j];
                    let ic = satir.find("if").and_then(|p| satir[p..].find('(').map(|q| p + q)).and_then(|ac| kapanan_parantez(satir.as_bytes(), ac).map(|k| &satir[ac + 1..k]));
                    let k = crate::dna::kisa(ic.unwrap_or_else(|| c[1].trim_end_matches(')')));
                    return Some(if satirlar[j].contains("unless") { format!("!({k})") } else { k });
                }
                if t.ends_with(';') || t.ends_with('}') {
                    return None;
                }
            }
            None
        }
    }
}

/// Satırdaki red (ön koşul) deseni: (durum, istisna, koşul satırda verildiyse koşul)
fn satir_reti(dil: Dil, satir: &str) -> Option<(Option<u16>, String, Option<String>)> {
    static D: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    let r = D.get_or_init(|| {
        let mut h = HashMap::new();
        let mut e = |a: &'static str, s: &str| {
            h.insert(a, Regex::new(s).unwrap());
        };
        e("throw", r"\bthrow\s+new\s+\\?([\w.\\]+)\s*\(");
        e("raise", r"\braise\s+([\w.]+)\s*(\()?");
        e("abort", r"\babort(_if|_unless)?\s*\(");
        e("response_status", r"ResponseStatusException\s*\(\s*HttpStatus(?:Code)?\s*\.\s*(\w+)");
        e(
            "cs_sonuc",
            r"\breturn\s+(?:Results\s*\.\s*|TypedResults\s*\.\s*)?(NotFound|BadRequest|Conflict|Forbid|Unauthorized|UnprocessableEntity|ValidationProblem|StatusCode)\s*\(",
        );
        e("rb_raise", r"\braise\s+([\w:]+)");
        // Django: hata yanıtı döndürme (`return HttpResponseForbidden()`, `return JsonResponse(..., status=400)`)
        e("py_yanit", r"\breturn\s+(HttpResponse(?:Forbidden|BadRequest|NotFound|Gone|NotAllowed)?|JsonResponse)\s*\(");
        e("rb_head", r"\b(?:head|render\b[^\n]*?status:)\s*:?\s*:(\w+)|\bstatus:\s*(\d{3})");
        h
    });
    let t = satir.trim_start();
    if t.starts_with("//") || t.starts_with('#') && dil != Dil::Php || t.starts_with('*') {
        return None;
    }
    match dil {
        Dil::Js | Dil::Jvm | Dil::Cs | Dil::Php => {
            if let Some(c) = r["response_status"].captures(satir) {
                return Some((crate::dna::kod_ifadesi(&format!("HttpStatus.{}", &c[1])).or_else(|| ad_kodu(&c[1])), "ResponseStatusException".into(), None));
            }
            if dil == Dil::Cs {
                if let Some(c) = r["cs_sonuc"].captures(satir) {
                    let kod = match &c[1] {
                        "NotFound" => Some(404),
                        "BadRequest" | "ValidationProblem" => Some(400),
                        "Conflict" => Some(409),
                        "Forbid" => Some(403),
                        "Unauthorized" => Some(401),
                        "UnprocessableEntity" => Some(422),
                        _ => ilk_arguman(&satir[c.get(0).unwrap().end()..]).and_then(|a| a.parse().ok()),
                    };
                    return Some((kod, c[1].to_string(), None));
                }
            }
            if dil == Dil::Php {
                if let Some(c) = r["abort"].captures(satir) {
                    let a = argumanlar(&satir[c.get(0).unwrap().end()..]);
                    let (kosul, kod) = match c.get(1).map(|x| x.as_str()) {
                        Some("_if") => (a.first().map(|x| crate::dna::kisa(x)), a.get(1)),
                        Some(_) => (a.first().map(|x| format!("!({})", crate::dna::kisa(x))), a.get(1)),
                        None => (None, a.first()),
                    };
                    let kod = kod.and_then(|k| k.trim().parse::<u16>().ok().or_else(|| crate::dna::kod_ifadesi(k)));
                    return Some((kod, format!("abort{}", c.get(1).map(|x| x.as_str()).unwrap_or("")), kosul));
                }
            }
            let c = r["throw"].captures(satir)?;
            let ad = c[1].to_string();
            let args = argumanlar(&satir[c.get(0).unwrap().end()..]);
            let kod = ad_kodu(&ad).or_else(|| args.iter().take(2).find_map(|a| crate::dna::kod_ifadesi(a).filter(|k| *k >= 400)));
            Some((kod, ad, None))
        }
        Dil::Py => {
            if let Some(c) = r["py_yanit"].captures(satir) {
                let ad = c[1].to_string();
                let kod = match ad.as_str() {
                    "HttpResponseForbidden" => Some(403),
                    "HttpResponseBadRequest" => Some(400),
                    "HttpResponseNotFound" => Some(404),
                    "HttpResponseGone" => Some(410),
                    "HttpResponseNotAllowed" => Some(405),
                    _ => argumanlar(&satir[c.get(0).unwrap().end()..])
                        .iter()
                        .find_map(|a| a.strip_prefix("status").map(|x| x.trim_start().trim_start_matches('=').trim().to_string()))
                        .and_then(|d| crate::dna::kod_ifadesi(&d)),
                };
                // Başarılı yanıt ret değildir
                return kod.filter(|k| *k >= 400).map(|k| (Some(k), ad, None));
            }
            let c = r["raise"].captures(satir)?;
            let ad = c[1].to_string();
            let mut kod = ad_kodu(&ad);
            if c.get(2).is_some() {
                let args = argumanlar(&satir[c.get(0).unwrap().end()..]);
                for a in &args {
                    if let Some(d) = a.strip_prefix("status_code").map(|x| x.trim_start().trim_start_matches('=')) {
                        kod = crate::dna::kod_ifadesi(d.trim()).or(kod);
                    }
                }
                if kod.is_none() || ad.ends_with("HTTPException") {
                    kod = args.first().and_then(|a| crate::dna::kod_ifadesi(a)).or(kod);
                }
            }
            Some((kod, ad, None))
        }
        Dil::Rb => {
            if let Some(c) = r["rb_head"].captures(satir) {
                let s = c.get(1).or(c.get(2)).map(|x| x.as_str()).unwrap_or("");
                let kod = simge_kodu(s)?;
                return (kod >= 400).then(|| (Some(kod), format!("status :{s}"), None));
            }
            let c = r["rb_raise"].captures(satir)?;
            Some((ad_kodu(&c[1]), c[1].to_string(), None))
        }
        Dil::Go => None,
    }
}

/// İşlem (transaction) sınırı ifadesi
fn islem_siniri(satir: &str) -> Option<&'static str> {
    static D: OnceLock<Regex> = OnceLock::new();
    let r = D.get_or_init(|| {
        Regex::new(r"(\$transaction\s*\(|\.transaction\s*\(|@Transactional\b|transaction\.atomic|DB::transaction\s*\(|DB::beginTransaction\s*\(|BeginTransaction(?:Async)?\s*\(|new\s+TransactionScope|\.begin_nested\s*\(|\.Transaction\s*\(\s*func|\.Begin(?:Tx)?\s*\(|\.transaction\s+do\b|\.transaction\s*\{|startTransaction\s*\(|withTransaction\s*\(|session\.begin\s*\(|\.atomic\s*\(|transaction\(\s*async)").unwrap()
    });
    r.find(satir).map(|m| match m.as_str() {
        s if s.contains("@Transactional") => "@Transactional",
        s if s.contains("$transaction") => "$transaction",
        s if s.contains("atomic") => "transaction.atomic",
        s if s.contains("DB::") => "DB::transaction",
        s if s.contains("TransactionScope") => "TransactionScope",
        s if s.contains("BeginTransaction") => "BeginTransaction",
        s if s.contains("Begin") => "Begin",
        _ => "transaction",
    })
}

/// Satırdaki silme ve yumuşak silme / geri yükleme çağrıları
fn satir_silmeleri(dil: Dil, satir: &str, no: usize, s: &mut DosyaSonuc) {
    static D: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    let r = D.get_or_init(|| {
        let mut h = HashMap::new();
        let mut e = |a: &'static str, x: &str| {
            h.insert(a, Regex::new(x).unwrap());
        };
        e("js", r"\b([A-Za-z_$][\w$]*)\s*\.\s*(delete|deleteMany|deleteOne|destroy|remove|deleteFrom|findByIdAndDelete|findOneAndDelete|findByIdAndRemove|softDelete|softRemove|restore|recover)\s*\(");
        e("py", r"\b(\w+)\s*\.\s*(delete|soft_delete|restore|undelete)\s*\(\s*(\w*)");
        e("php", r"\$?(\w+)\s*(?:->|::)\s*(delete|forceDelete|destroy|restore|detach)\s*\(");
        e("rb", r"\b@?(\w+)\s*\.\s*(destroy!?|delete|destroy_all|delete_all|really_destroy!|discard|undiscard|restore)\b");
        e("jvm", r"\b(\w*(?:[rR]epository|[rR]epo|[dD]ao|entityManager|em|[sS]ervice))\s*\.\s*(delete|deleteById|deleteAll|deleteAllById|deleteInBatch|deleteAllInBatch|remove)\s*\(");
        e("cs", r"\b([\w.]*\w)\s*\.\s*(Remove|RemoveRange|ExecuteDelete|ExecuteDeleteAsync)\s*\(");
        e("go", r"\b(\w+)\s*(?:\.\s*Unscoped\s*\(\s*\))?\s*\.\s*Delete\s*\(");
        e("sql", r#"(?i)["'`]\s*DELETE\s+FROM\s+["`\[]?(\w+)"#);
        e("tablo", r#"(?:deleteFrom|delete_from)\s*\(\s*['"](\w+)['"]"#);
        e("prisma", r"\bprisma\s*\.\s*(\w+)\s*\.\s*(delete|deleteMany)\s*\(");
        h
    });
    let haric = [
        "map",
        "set",
        "cache",
        "headers",
        "params",
        "searchparams",
        "formdata",
        "session",
        "localstorage",
        "sessionstorage",
        "storage",
        "url",
        "cookies",
        "res",
        "req",
        "router",
        "app",
        "fs",
        "fspromises",
        "object",
        "reflect",
        "array",
        "list",
        "keys",
        "weakmap",
        "client",
        "redis",
        "file",
        "files",
        "dir",
        "path",
        "os",
        "shutil",
        "self",
        "this",
        "super",
        "request",
        "response",
        "http",
        "axios",
        "api",
        "requests",
        "httpx",
        "route",
        "routes",
        "r",
        "e",
        "mux",
        "group",
        "g",
        "v1",
        "api_router",
    ];
    let mut ekle = |alici: &str, yontem: &str, tablo: Option<String>| {
        let alt = alici.to_ascii_lowercase();
        if haric.contains(&alt.as_str()) && tablo.is_none() {
            return;
        }
        // Servis / denetleyici katmanına devreden çağrı (`this.service.delete(id)`) silmenin kendisi değildir: zincir
        // veri katmanına kadar izlenir
        if tablo.is_none() && ["service", "services", "controller", "api", "client", "handler", "usecase", "facade"].iter().any(|e| alt.ends_with(e)) {
            return;
        }
        let y = yontem.trim_end_matches('!');
        if matches!(y, "softDelete" | "softRemove" | "soft_delete" | "discard" | "restore" | "recover" | "undelete" | "undiscard") {
            s.yumusak_cagrilar.push((no, y.to_string()));
            return;
        }
        s.silmeler.push(Silme { alici: alici.to_string(), yontem: yontem.to_string(), tablo, satir: no });
    };
    if let Some(c) = r["sql"].captures(satir) {
        ekle("sql", "DELETE FROM", Some(c[1].to_string()));
        return;
    }
    if let Some(c) = r["tablo"].captures(satir) {
        ekle("db", "deleteFrom", Some(c[1].to_string()));
        return;
    }
    if let Some(c) = r["prisma"].captures(satir) {
        ekle("prisma", &c[2], Some(c[1].to_string()));
        return;
    }
    if let Some((alici, yontem, tablo)) = toplu_silme(dil, satir) {
        ekle(&alici, &yontem, tablo);
        return;
    }
    let anahtar = match dil {
        Dil::Js => "js",
        Dil::Py => "py",
        Dil::Php => "php",
        Dil::Rb => "rb",
        Dil::Jvm => "jvm",
        Dil::Cs => "cs",
        Dil::Go => "go",
    };
    if let Some(c) = r[anahtar].captures(satir) {
        let yontem = if dil == Dil::Go {
            if satir.contains("Unscoped") {
                "Unscoped.Delete".to_string()
            } else {
                "Delete".to_string()
            }
        } else {
            c[2].to_string()
        };
        // Python: `session.delete(item)` -> argüman modeldir; Django `obj.delete()`
        let alici = if dil == Dil::Py && c.get(3).is_some_and(|x| !x.as_str().is_empty()) { c[3].to_string() } else { c[1].rsplit('.').next().unwrap_or(&c[1]).to_string() };
        if dil == Dil::Go && !matches!(c[1].to_ascii_lowercase().as_str(), "db" | "tx" | "gorm" | "conn" | "orm") && !c[1].ends_with("DB") && !c[1].ends_with("db") {
            return;
        }
        // Genel adlı silme (`delete` / `remove` / `Remove`) koleksiyonlarda da var (Map, Set, Hash, List): yalnızca veri
        // katmanına benzeyen alıcıda (depo, veritabanı, ORM, model sınıfı) silme sayılır
        let veri_katmani = |a: &str| {
            let k = a.to_ascii_lowercase();
            a.starts_with(|c: char| c.is_ascii_uppercase())
                || ["repository", "repo", "db", "prisma", "knex", "trx", "tx", "model", "dao", "collection", "table", "manager", "orm", "sequelize", "context", "entities"]
                    .iter()
                    .any(|e| k.contains(e))
        };
        let genel = matches!(yontem.as_str(), "delete" | "remove" | "Remove" | "RemoveRange");
        if genel && matches!(dil, Dil::Js | Dil::Cs) && !veri_katmani(&c[1]) {
            return;
        }
        if dil == Dil::Rb && yontem == "delete" && !c[1].starts_with(|c: char| c.is_ascii_uppercase()) {
            return;
        }
        ekle(&alici, &yontem, None);
    }
}

/// Sorgu zincirinin sonundaki toplu silme: (alıcı ya da model, yöntem, tablo). Eloquent `Model::where(...)->delete()`,
/// `DB::table('t')->...->delete()`; Django `Model.objects.filter(...).delete()`; SQLAlchemy `query(Model)...delete()`,
/// `delete(Model).where(...)`; TypeORM `createQueryBuilder()...delete()`; ActiveRecord `Model.where(...).delete_all`;
/// EF Core `.Where(...).ExecuteDelete()`. Alıcı model adıdır: modeli yumuşak silen projede (SoftDeletes, paranoid,
/// deletedAt) normal `delete()` kalıcı sayılmaz, `forceDelete` sayılır
fn toplu_silme(dil: Dil, satir: &str) -> Option<(String, String, Option<String>)> {
    static D: OnceLock<[Regex; 8]> = OnceLock::new();
    let [php_tablo, php, py_dj, py_sa, py_sa_cekirdek, js_qb, rb, cs] = D.get_or_init(|| {
        [
            Regex::new(r#"\bDB::table\(\s*['"](\w+)['"]\s*\).*->\s*(delete|truncate)\s*\("#).unwrap(),
            Regex::new(r"\b([A-Z]\w*)::(?:where\w*|query|whereIn|whereNotNull|onlyTrashed|withTrashed|whereKey)\b.*->\s*(delete|forceDelete|truncate)\s*\(").unwrap(),
            Regex::new(r"\b([A-Z]\w*)\.objects\b.*\.\s*(delete)\s*\(\s*\)").unwrap(),
            Regex::new(r"\.query\(\s*([A-Z]\w*)\s*\).*\.\s*(delete)\s*\(").unwrap(),
            Regex::new(r"(?:^|[^.\w])(delete)\(\s*([A-Z]\w*)\s*\)").unwrap(),
            Regex::new(r"(?:\b([A-Za-z_$][\w$]*)\s*\.\s*)?createQueryBuilder\(.*\.\s*(delete)\s*\(\s*\)(?:.*\.\s*from\(\s*([A-Za-z_$][\w$]*))?").unwrap(),
            Regex::new(r"\b([A-Z]\w*)\.(?:where|all|joins|includes|rewhere|unscoped)\b.*\.(delete_all|destroy_all)\b").unwrap(),
            Regex::new(r"\b([\w.]*\w)\s*\.\s*Where\(.*\)\s*\.\s*(ExecuteDelete(?:Async)?)\s*\(").unwrap(),
        ]
    });
    let ikili = |c: regex::Captures| Some((c[1].to_string(), c[2].to_string(), None));
    match dil {
        Dil::Php => {
            if let Some(c) = php_tablo.captures(satir) {
                return Some(("DB".into(), c[2].to_string(), Some(c[1].to_string())));
            }
            php.captures(satir).and_then(ikili)
        }
        Dil::Py => match py_dj.captures(satir).or_else(|| py_sa.captures(satir)) {
            Some(c) => ikili(c),
            None => py_sa_cekirdek.captures(satir).map(|c| (c[2].to_string(), c[1].to_string(), None)),
        },
        // TypeORM: model `.from(Model)`'den, yoksa sorgu kurucusunun alıcısı
        Dil::Js => js_qb.captures(satir).map(|c| {
            let alici = c.get(3).or(c.get(1)).map(|m| m.as_str().to_string()).unwrap_or_else(|| "queryBuilder".into());
            (alici, c[2].to_string(), None)
        }),
        Dil::Rb => rb.captures(satir).and_then(ikili),
        Dil::Cs => cs.captures(satir).map(|c| (c[1].rsplit('.').next().unwrap_or(&c[1]).to_string(), c[2].to_string(), None)),
        _ => None,
    }
}

/// GORM silmesinin model tipi: argüman bileşik değişmezse tipi, değişkense son bildirimi
fn go_silme_tipi(satir: &str, onceki: &[&str]) -> Option<String> {
    static D: OnceLock<Regex> = OnceLock::new();
    let r = D.get_or_init(|| Regex::new(r"\.\s*Delete\s*\(\s*&?(?:\[\])?(?:\w+\.)?(\w+)(\s*\{)?").unwrap());
    let c = r.captures(satir)?;
    if c.get(2).is_some() {
        return Some(c[1].to_string());
    }
    let ad = regex::escape(&c[1]);
    let bildirim = Regex::new(&format!(r"(?:\bvar\s+{ad}\s+\*?(?:\[\])?(?:\w+\.)?(\w+)|\b{ad}\s*:?=\s*&?(?:\[\])?(?:\w+\.)?(\w+)\s*\{{)")).ok()?;
    onceki.iter().rev().take(60).find_map(|l| bildirim.captures(l).and_then(|x| x.get(1).or(x.get(2)).map(|m| m.as_str().to_string())))
}

/// Model düzeyinde yumuşak silme işaretleri
fn yumusak_modeller(dil: Dil, m: &str) -> Vec<(String, usize, String)> {
    static D: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    let r = D.get_or_init(|| {
        let mut h = HashMap::new();
        let mut e = |a: &'static str, x: &str| {
            h.insert(a, Regex::new(x).unwrap());
        };
        e("sinif", r"(?m)^\s*(?:export\s+)?(?:abstract\s+)?(?:public\s+|final\s+)*class\s+(\w+)");
        e("go_yapi", r"(?m)^\s*type\s+(\w+)\s+struct\b");
        e("sequelize", r#"\.define\s*\(\s*['"](\w+)['"]"#);
        e("ef", r"Entity\s*<\s*(\w+)\s*>\s*\(\s*\)\s*\.\s*HasQueryFilter\s*\([^)]*(?:IsDeleted|DeletedAt|Deleted)");
        h
    });
    let mut v = vec![];
    let isaretler: &[&str] = match dil {
        Dil::Php => &["use SoftDeletes", "\\SoftDeletes;", " SoftDeletes,", ", SoftDeletes"],
        Dil::Rb => &["acts_as_paranoid", "include Discard::Model", "acts_as_archival"],
        Dil::Js => &["@DeleteDateColumn", "paranoid: true", "paranoid:true", "mongoose-delete"],
        Dil::Go => &["gorm.DeletedAt", "gorm.Model", "soft_delete.DeletedAt"],
        Dil::Py => &["SafeDeleteModel", "SoftDeleteModel", "SoftDeleteMixin", "SoftDeletableModel"],
        Dil::Cs => &["ISoftDelete"],
        Dil::Jvm => &["@SQLDelete", "@SoftDelete"],
    };
    let baslar = satir_baslari(m);
    for isaret in isaretler {
        for (i, _) in m.match_indices(isaret) {
            let satir = satir_no(&baslar, i);
            // Modelin adı: işaretten önceki son sınıf / yapı tanımı (PHP trait, TypeORM sütunu sınıfın içindedir);
            // Sequelize `define('ad', ..., { paranoid: true })`; Java `@SQLDelete` sınıfın üstündedir (sonraki sınıf)
            let ad = if dil == Dil::Jvm {
                r["sinif"].captures(&m[i..]).map(|c| c[1].to_string())
            } else if dil == Dil::Go {
                r["go_yapi"].captures_iter(&m[..i]).last().map(|c| c[1].to_string())
            } else if isaret.starts_with("paranoid") {
                r["sequelize"].captures_iter(&m[..i]).last().map(|c| c[1].to_string()).or_else(|| r["sinif"].captures_iter(&m[..i]).last().map(|c| c[1].to_string()))
            } else {
                r["sinif"].captures_iter(&m[..i]).last().map(|c| c[1].to_string()).or_else(|| r["sinif"].captures(&m[i..]).map(|c| c[1].to_string()))
            };
            if let Some(ad) = ad {
                if !v.iter().any(|(a, _, _): &(String, usize, String)| *a == ad) {
                    v.push((ad, satir, isaret.trim_matches([' ', ',', ';', '\\']).to_string()));
                }
            }
        }
    }
    for c in r["ef"].captures_iter(m) {
        v.push((c[1].to_string(), satir_no(&baslar, c.get(0).unwrap().start()), "HasQueryFilter".into()));
    }
    v
}

/// Kuyruk dinleyicileri (içeri gelen mesaj): `@KafkaListener(topics = "x")`, `@RabbitListener(queues = "x")`,
/// `@EventPattern('x')`, `consumer.subscribe({ topic: 'x' })`, `channel.consume('x', ...)`
fn dinleyiciler(m: &str) -> Vec<(String, String, usize)> {
    static D: OnceLock<Regex> = OnceLock::new();
    let r = D.get_or_init(|| {
        Regex::new(r#"@(KafkaListener|RabbitListener|JmsListener|SqsListener)\s*\(\s*(?:\w+\s*=\s*)?\{?\s*"([^"]+)"|\b(subscribe)\s*\(\s*\{\s*topics?\s*:\s*\[?\s*['"]([^'"]+)['"]|\.(consume)\s*\(\s*['"]([^'"]+)['"]"#).unwrap()
    });
    let baslar = satir_baslari(m);
    r.captures_iter(m)
        .filter_map(|c| {
            let (tur, kanal) = match (c.get(1), c.get(3), c.get(5)) {
                (Some(t), _, _) => (t.as_str().replace("Listener", "").to_lowercase(), c[2].to_string()),
                (_, Some(_), _) => ("kafka".to_string(), c[4].to_string()),
                (_, _, Some(_)) => ("rabbit".to_string(), c[6].to_string()),
                _ => return None,
            };
            Some((tur, kanal, satir_no(&baslar, c.get(0)?.start())))
        })
        .collect()
}

/// Çağrı adları: (alıcı, ad). Ruby'de parantezsiz üye çağrısı da sayılır
fn satir_cagrilari(dil: Dil, satir: &str) -> Vec<(Option<String>, String)> {
    static D: OnceLock<[Regex; 2]> = OnceLock::new();
    let [genel, rb] = D.get_or_init(|| {
        [
            Regex::new(r"(?:([A-Za-z_$][\w$]*)\s*(?:\?\.|\.|->|::)\s*)?([A-Za-z_$][\w$]*)\s*(?:<[\w\s,<>\[\]?.]*>)?\s*\(").unwrap(),
            Regex::new(r"\b@?([A-Za-z_]\w*)\s*\.\s*([a-z_]\w*[?!]?)").unwrap(),
        ]
    });
    // Alıcısı ifade olan üye çağrısı (`foo().replaceAll(`, `x[0].save(`): alıcı bilinmez, "?" ile işaretlenir (izlenmez)
    let mut v: Vec<(Option<String>, String)> = genel
        .captures_iter(satir)
        .map(|c| {
            let alici = c.get(1).map(|x| x.as_str().to_string()).or_else(|| {
                let once = satir[..c.get(0).map_or(0, |m| m.start())].trim_end();
                (once.ends_with('.') || once.ends_with("->") || once.ends_with("::")).then(|| "?".to_string())
            });
            (alici, c[2].to_string())
        })
        .collect();
    if dil == Dil::Rb {
        v.extend(rb.captures_iter(satir).map(|c| (Some(c[1].to_string()), c[2].to_string())));
    }
    v.retain(|(_, a)| !ANAHTAR_SOZCUKLER.contains(&a.as_str()) && a.len() > 1);
    v
}

/// `i`. satırdan başlayan ifade, devam satırlarıyla tek satırda: sonraki satır `->`, `?->` ya da `.` ile başlıyorsa ya da
/// açılan parantez / köşeli parantez kapanmadıysa birleştirilir (en çok 20 satır). Gövde açan satırda (`{`, `=>`,
/// sonda `:`) durulur: gövdenin içi ayrı ifadelerdir. Dönen: (ifade, son satırın sırası)
fn ifade_birlestir(satirlar: &[&str], i: usize) -> (String, usize) {
    let derinlik = |t: &str| -> i32 {
        let mut tirnak: Option<char> = None;
        let mut d = 0;
        for c in t.chars() {
            match (tirnak, c) {
                (Some(q), x) if x == q => tirnak = None,
                (Some(_), _) => {}
                (None, '\'' | '"' | '`') => tirnak = Some(c),
                (None, '(' | '[') => d += 1,
                (None, ')' | ']') => d -= 1,
                _ => {}
            }
        }
        d
    };
    let govde = |t: &str| {
        let t = t.trim_end();
        t.ends_with('{') || t.contains("=>") || t.ends_with(':') || t.contains("function")
    };
    let mut ifade = satirlar[i].to_string();
    let mut d = derinlik(satirlar[i]);
    let mut son = i;
    while son + 1 < satirlar.len() && son + 1 - i < 20 && !govde(satirlar[son]) {
        let sonraki = satirlar[son + 1].trim_start();
        let zincir = sonraki.starts_with("->") || sonraki.starts_with("?->") || (sonraki.starts_with('.') && !sonraki.starts_with("..."));
        if !(zincir || d > 0) || sonraki.is_empty() {
            break;
        }
        ifade.push(' ');
        ifade.push_str(sonraki);
        d += derinlik(sonraki);
        son += 1;
    }
    (ifade, son)
}

/// Tek dosyayı tarar
fn dosya_tara(dil: Dil, m: &str) -> DosyaSonuc {
    let baslar = satir_baslari(m);
    let satirlar: Vec<&str> = m.lines().collect();
    let mut s = DosyaSonuc { bloklar: bloklari_bul(dil, m, &baslar), ..Default::default() };
    s.yontem_dallari = crate::cagri::yontem_dallari(if dil == Dil::Py { crate::cagri::Dil::Py } else { crate::cagri::Dil::Ts }, m);
    let b = baglam_kur(dil, m);
    // Feign: arayüz yöntemleri dış çağrıdır (gövdeleri yoktur): her eşleme ek bir tek satırlık blok olur
    let feign = (dil == Dil::Jvm && m.contains("@FeignClient")).then(|| {
        let url = Regex::new(r#"@FeignClient\s*\([^)]*?url\s*=\s*"([^"]+)""#).ok().and_then(|r| r.captures(m).map(|c| c[1].to_string()));
        let ad = Regex::new(r#"@FeignClient\s*\(\s*(?:(?:name|value)\s*=\s*)?"([^"]+)""#).ok().and_then(|r| r.captures(m).map(|c| c[1].to_string()));
        (url, ad)
    });
    static FEIGN: OnceLock<[Regex; 2]> = OnceLock::new();
    let [esleme, yontem_adi] = FEIGN.get_or_init(|| {
        [Regex::new(r#"@(Get|Post|Put|Delete|Patch|Request)Mapping\s*\(\s*(?:(?:value|path)\s*=\s*)?"?([^",)]*)"?"#).unwrap(), Regex::new(r"\b(\w+)\s*\([^)]*\)\s*;").unwrap()]
    });
    let yorum = |t: &str| t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') || (t.starts_with('#') && dil != Dil::Php) || t.starts_with("--");
    // Silme taramasında birleştirilen son ifadenin bittiği satırın bir sonrası
    let mut devam_son = 0usize;
    for (i, satir) in satirlar.iter().enumerate() {
        let t = satir.trim_start();
        if t.is_empty() || yorum(t) {
            continue;
        }
        let no = i + 1;
        let devam = &m[baslar[i]..baslar.get(i + 8).copied().unwrap_or(m.len())];
        for (alici, ad) in satir_cagrilari(dil, satir) {
            s.cagrilar.push((no, alici, ad));
        }
        // Dış çağrı: hedef aynı fonksiyonda ve dosyada çözülür
        let hamlar = satir_disleri(dil, satir, devam, &b);
        if !hamlar.is_empty() {
            let fonk = s
                .bloklar
                .iter()
                .filter(|(_, bas, son)| *bas <= no && no <= *son)
                .min_by_key(|(_, bas, son)| son - bas)
                .map(|(_, bas, son)| satirlar[bas - 1..(*son).min(satirlar.len())].join("\n"))
                .unwrap_or_default();
            for h in hamlar {
                if let Some(d) = cozumle(h, &fonk, m, &b, no) {
                    if !s.disler.iter().any(|x| x.satir == no && x.istemci == d.istemci) {
                        s.disler.push(d);
                    }
                }
            }
        }
        if let Some(fe) = &feign {
            if let Some(c) = esleme.captures(satir) {
                // Sonraki yöntem bildirimi
                if let Some((j, ad)) = satirlar.iter().enumerate().skip(i).take(6).find_map(|(j, l)| yontem_adi.captures(l).map(|c| (j, c[1].to_string()))) {
                    let (hedef, tur, guven) = match (&fe.0, &fe.1) {
                        (Some(u), _) => match adres_parcala(u) {
                            Some((a, _)) => (a, "url", "certain"),
                            None => ortam_adi(u).map(|o| (o, "ortam", "inferred")).unwrap_or(("dinamik".into(), "dinamik", "unknown")),
                        },
                        (None, Some(a)) => (a.clone(), "sdk", "inferred"),
                        _ => ("dinamik".into(), "dinamik", "unknown"),
                    };
                    let y = if &c[1] == "Request" {
                        yontem_secenegi(satir).or_else(|| satir.split("RequestMethod.").nth(1).map(|x| x.chars().take_while(|c| c.is_alphanumeric()).collect()))
                    } else {
                        Some(c[1].to_uppercase())
                    };
                    let yazma = !matches!(y.as_deref(), Some("GET" | "HEAD" | "OPTIONS"));
                    s.disler.push(Dis {
                        tur: "http",
                        istemci: "Feign".into(),
                        yontem: y,
                        hedef,
                        hedef_turu: tur,
                        yol: Some(c[2].to_string()).filter(|y| y.starts_with('/')),
                        kanal: None,
                        yazma,
                        ertelenmis: false,
                        guven,
                        satir: j + 1,
                        alici: None,
                    });
                    s.bloklar.push((ad, j + 1, j + 1));
                }
            }
        }
        if let Some((durum, istisna, ic_kosul)) = satir_reti(dil, satir) {
            let k = ic_kosul.or_else(|| kosul(dil, &satirlar, i));
            if durum.is_some_and(|d| d >= 400) || (k.is_some() && (istisna.ends_with("Exception") || istisna.ends_with("Error"))) {
                s.retler.push(Ret { durum: durum.filter(|d| *d >= 400), istisna, kosul: k, satir: no });
            }
        }
        if let Some(x) = islem_siniri(satir) {
            s.islem_sinirlari.push((no, x.to_string()));
        }
        if let Some(g) = gecmis_yazmasi(satir) {
            s.gecmis_yazmalari.push((no, g));
        }
        let once = s.silmeler.len();
        // Çok satırlı sorgu zinciri (`Model::where(...)` alt satırda `->delete()`, parantez içinde bölünmüş çağrı) tek
        // ifade olarak taranır; satır numarası ifadenin ilk satırı. Zincirin devam satırları ayrıca taranmaz
        if i < devam_son {
            continue;
        }
        let (ifade, son) = ifade_birlestir(&satirlar, i);
        devam_son = son + 1;
        satir_silmeleri(dil, &ifade, no, &mut s);
        // GORM: silinen modelin tipi argümandan (`Delete(&entity)` -> `var entity system.SysApi`, `Delete(&model.X{})`);
        // yumuşak silme modelden gelir
        if dil == Dil::Go && s.silmeler.len() > once {
            if let Some(tip) = go_silme_tipi(satir, &satirlar[..i]) {
                s.silmeler.last_mut().unwrap().tablo = Some(tip);
            }
        }
    }
    s.yumusak_modeller = yumusak_modeller(dil, m);
    s.dinleyiciler = dinleyiciler(m);
    s.siniflar = blok_siniflari(dil, m, &baslar, &satirlar, &s.bloklar);
    s.ustler = ustler(dil, m, &baslar);
    s.tabanlar = taban_kayitlari(dil, m, &baslar);
    s.gecmis_modeller = gecmis_modelleri(dil, m, &baslar);
    s.alan_tipleri = alan_tipleri(dil, m);
    s
}

/// Blokların sınıfları: içinde bulundukları sınıf / arayüz / yapı tanımının adı; Go'da yöntem alıcısının tipi
fn blok_siniflari(dil: Dil, m: &str, baslar: &[usize], satirlar: &[&str], bloklar: &[(String, usize, usize)]) -> Vec<Option<String>> {
    static D: OnceLock<[Regex; 3]> = OnceLock::new();
    let [sinif, girintili, go] = D.get_or_init(|| {
        [
            Regex::new(r"\b(?:class|interface|struct|object|trait|record|enum)\s+([A-Za-z_]\w*)").unwrap(),
            Regex::new(r"^(\s*)class\s+([A-Za-z_]\w*)").unwrap(),
            Regex::new(r"^\s*func\s+\(\s*\w*\s+\*?([A-Za-z_]\w*)").unwrap(),
        ]
    });
    // Sınıf aralıkları: (ad, ilk satır, son satır)
    let mut araliklar: Vec<(String, usize, usize)> = vec![];
    match dil {
        Dil::Go => {
            return bloklar.iter().map(|(_, bas, _)| satirlar.get(bas - 1).and_then(|l| go.captures(l)).map(|c| c[1].to_string())).collect();
        }
        Dil::Py | Dil::Rb => {
            for (i, l) in satirlar.iter().enumerate() {
                let Some(c) = girintili.captures(l) else { continue };
                let g = c[1].len();
                let mut son = i;
                for (j, t) in satirlar.iter().enumerate().skip(i + 1) {
                    let tt = t.trim();
                    if tt.is_empty() || tt.starts_with('#') {
                        continue;
                    }
                    if t.len() - t.trim_start().len() <= g {
                        if dil == Dil::Rb && tt.starts_with("end") {
                            son = j;
                        }
                        break;
                    }
                    son = j;
                }
                araliklar.push((c[2].to_string(), i + 1, son + 1));
            }
        }
        _ => {
            let es = parantez_eslesmeleri(m, dil);
            for c in sinif.captures_iter(m) {
                let son = c.get(0).unwrap().end();
                let Some(ac) = m[son..].find(['{', ';']).map(|x| son + x).filter(|&x| m.as_bytes()[x] == b'{') else { continue };
                if let Some(k) = es.get(&ac) {
                    araliklar.push((c[1].to_string(), satir_no(baslar, c.get(0).unwrap().start()), satir_no(baslar, *k)));
                }
            }
        }
    }
    bloklar.iter().map(|(_, bas, son)| araliklar.iter().filter(|(_, a, b)| a <= bas && son <= b).min_by_key(|(_, a, b)| b - a).map(|x| x.0.clone())).collect()
}

/// DI kayıtlarındaki taban adresler: ASP.NET `AddHttpClient<T>(o => o.BaseAddress = new(...))` (tip T ve varsa
/// uygulaması), NestJS `HttpModule.register({ baseURL })` (projede bütün HttpService'ler: "*"), Spring
/// `WebClient.builder()...baseUrl(...)` / `RestClient.builder()` bean yöntemi (bean adı). Hedef kaydın dosyasında çözülür
fn taban_kayitlari(dil: Dil, m: &str, baslar: &[usize]) -> Vec<(String, Cozum, usize)> {
    static D: OnceLock<[Regex; 5]> = OnceLock::new();
    let [cs, nest, jvm_taban, bean, ktor_taban] = D.get_or_init(|| {
        [
            Regex::new(r"AddHttpClient\s*<\s*(\w+)\s*(?:,\s*(\w+)\s*)?>\s*\(").unwrap(),
            Regex::new(r"HttpModule\s*\.\s*register(?:Async)?\s*\(\s*\{[^}]*?baseURL\s*:\s*([^,}\n]+)").unwrap(),
            Regex::new(r"(?:WebClient|RestClient)\s*\.\s*builder\s*\(\s*\)").unwrap(),
            Regex::new(r"\b(?:WebClient|RestClient)\s+(\w+)\s*\([^)]*\)\s*\{").unwrap(),
            Regex::new(r"\b(?:val|var)\s+(\w+)\s*(?::\s*HttpClient\s*)?=\s*HttpClient\s*(?:\([^)]*\))?\s*\{").unwrap(),
        ]
    });
    let mut v = vec![];
    let coz = |e: &str| hedef_coz(e, "", m, 0);
    match dil {
        Dil::Cs => {
            for c in cs.captures_iter(m) {
                let son = c.get(0).map_or(0, |x| x.end());
                let pencere = &m[son..m.len().min(son + 400)];
                let Some(i) = pencere.find("BaseAddress") else { continue };
                let kalan = &pencere[i..];
                let Some(j) = kalan.find('(') else { continue };
                let Some(e) = ilk_arguman(&kalan[j + 1..]) else { continue };
                let h = coz(&e);
                let satir = satir_no(baslar, son);
                for t in [c.get(1), c.get(2)].into_iter().flatten() {
                    v.push((t.as_str().to_string(), h.clone(), satir));
                }
            }
        }
        Dil::Js => {
            for c in nest.captures_iter(m) {
                v.push(("*".to_string(), coz(c[1].trim()), satir_no(baslar, c.get(0).map_or(0, |x| x.start()))));
            }
        }
        Dil::Jvm => {
            for c in jvm_taban.captures_iter(m) {
                let bas = c.get(0).map_or(0, |x| x.start());
                let pencere = &m[bas..m.len().min(bas + 400)];
                let Some(i) = pencere.find(".baseUrl(") else { continue };
                let Some(e) = ilk_arguman(&pencere[i + 9..]) else { continue };
                // Bean yöntemi: kurucudan önceki son `WebClient ad(...) {`
                let ad = bean.captures_iter(&m[..bas]).last().map(|b| b[1].to_string()).unwrap_or_else(|| "*".into());
                v.push((ad, coz(&e), satir_no(baslar, bas)));
            }
            // Ktor: `val client = HttpClient(CIO) { defaultRequest { url("https://...") } }`; ad istemci örneğinin adı
            for c in ktor_taban.captures_iter(m) {
                let bas = c.get(0).map_or(0, |x| x.end());
                let pencere = &m[bas..m.len().min(bas + 600)];
                let Some(i) = pencere.find("defaultRequest") else { continue };
                let kalan = &pencere[i..];
                let Some(j) = kalan.find("url(") else { continue };
                let Some(e) = ilk_arguman(&kalan[j + 4..]) else { continue };
                v.push((c[1].to_string(), coz(&e), satir_no(baslar, bas)));
            }
        }
        _ => {}
    }
    v.retain(|(_, h, _)| h.1 != "goreli");
    v
}

/// Göreli adresli istemci çağrılarına DI kaydındaki taban adresi uygular: istemci örneğinin adı (bean), çağrının
/// sınıfı (typed HttpClient) ya da projede genel kayıt ("*"). Aynı ad birden fazla kayıtla (aynı depoda iki uygulama)
/// eşleşirse çağrıya yolca en yakın kayıt seçilir; yakınlık eşitse hedef bilinmez (`unknown`). Kaydı bulunamayan
/// göreli çağrı uygulamanın kendisine gider sayılır ve atılır. Güven tabanın ve yolun en zayıfı
fn tabanlari_uygula(dosyalar: &mut [Dosya]) {
    let mut tablo: HashMap<String, Vec<(Cozum, String)>> = HashMap::new();
    for d in dosyalar.iter() {
        for (t, h, _) in &d.sonuc.tabanlar {
            tablo.entry(t.clone()).or_default().push((h.clone(), d.yol.clone()));
        }
    }
    let ortak = |a: &str, b: &str| a.split('/').zip(b.split('/')).take_while(|(x, y)| x == y).count();
    let sec = |t: &str, yol: &str| -> Option<Cozum> {
        let l = tablo.get(t)?;
        if l.iter().all(|x| x.0 .0 == l[0].0 .0) {
            return Some(l[0].0.clone());
        }
        let en = l.iter().map(|x| ortak(&x.1, yol)).max()?;
        let enler: Vec<&(Cozum, String)> = l.iter().filter(|x| ortak(&x.1, yol) == en).collect();
        Some(if enler.iter().all(|x| x.0 .0 == enler[0].0 .0) { enler[0].0.clone() } else { ("dinamik".into(), "dinamik", None, "unknown") })
    };
    for d in dosyalar.iter_mut() {
        let yol = d.yol.clone();
        let s = &mut d.sonuc;
        let siniflar: Vec<(usize, usize, Option<String>)> = s.bloklar.iter().zip(&s.siniflar).map(|((_, a, b), c)| (*a, *b, c.clone())).collect();
        s.disler.retain_mut(|x| {
            if x.hedef_turu != "goreli" {
                return true;
            }
            let sinif = siniflar.iter().filter(|(a, b, _)| *a <= x.satir && x.satir <= *b).min_by_key(|(a, b, _)| b - a).and_then(|y| y.2.clone());
            let taban = x.alici.as_deref().and_then(|a| sec(a, &yol)).or_else(|| sinif.as_deref().and_then(|c| sec(c, &yol))).or_else(|| sec("*", &yol));
            match taban {
                Some((hedef, tur, _, guven)) => {
                    x.hedef = hedef;
                    x.hedef_turu = tur;
                    x.guven = zayif(guven, x.guven);
                    true
                }
                None => false,
            }
        });
    }
}

/// Önceki değeri saklayan sürüm / geçmiş işaretleri (model düzeyi): PaperTrail `has_paper_trail`, Audited, Envers
/// `@Audited`, django-simple-history `HistoricalRecords()`, django-reversion, Laravel `RevisionableTrait` /
/// `LogsActivity`, EF Core zamansal tablolar (`IsTemporal`), mongoose-history. Dönen: (model, satır, işaret)
fn gecmis_modelleri(dil: Dil, m: &str, baslar: &[usize]) -> Vec<(String, usize, String)> {
    static D: OnceLock<[Regex; 2]> = OnceLock::new();
    let [sinif, ef] = D.get_or_init(|| {
        [
            Regex::new(r"(?m)^\s*(?:export\s+)?(?:abstract\s+)?(?:public\s+|final\s+)*class\s+(\w+)").unwrap(),
            Regex::new(r"Entity\s*<\s*(\w+)\s*>\s*\(\s*\)[^;]*IsTemporal").unwrap(),
        ]
    });
    let isaretler: &[&str] = match dil {
        Dil::Rb => &["has_paper_trail", "audited", "has_associated_audits"],
        Dil::Py => &["HistoricalRecords(", "reversion.register", "VersionAdmin"],
        Dil::Php => &["RevisionableTrait", "Revisionable", "LogsActivity", "Auditable"],
        Dil::Jvm => &["@Audited"],
        Dil::Js => &["mongoose-history", "paper-trail", "sequelize-paper-trail"],
        _ => &[],
    };
    let mut v = vec![];
    for isaret in isaretler {
        for (i, _) in m.match_indices(isaret) {
            // Envers `@Audited` sınıfın üstündedir (sonraki sınıf); ötekiler sınıfın içinde (önceki sınıf)
            let ad = if dil == Dil::Jvm { sinif.captures(&m[i..]).map(|c| c[1].to_string()) } else { sinif.captures_iter(&m[..i]).last().map(|c| c[1].to_string()) };
            if let Some(ad) = ad.filter(|a| !v.iter().any(|x: &(String, usize, String)| x.0 == *a)) {
                v.push((ad, satir_no(baslar, i), isaret.trim_end_matches('(').to_string()));
            }
        }
    }
    if dil == Dil::Cs {
        for c in ef.captures_iter(m) {
            v.push((c[1].to_string(), satir_no(baslar, c.get(0).map_or(0, |x| x.start())), "IsTemporal".into()));
        }
    }
    v
}

/// Satırda bir sürüm / geçmiş kaydı oluşturuluyor mu (`new PageRevision`, `revisionRepo->storeNewForPage`,
/// `history.create(...)`, `AuditLog.create`)
fn gecmis_yazmasi(satir: &str) -> Option<String> {
    static D: OnceLock<Regex> = OnceLock::new();
    let r = D.get_or_init(|| {
        Regex::new(r"(?:\bnew\s+\\?(\w*(?:Revision|History|Version|Snapshot)\w*)\s*\(|\b(\w*(?:[rR]evision|[hH]istory|[sS]napshot)\w*)\s*(?:\.|->|::)\s*(?:create|save|store\w*|insert|add|record|push|snapshot)\w*\s*\()").unwrap()
    });
    let c = r.captures(satir)?;
    // Adres (slug) geçmişi içerik sürümü değildir
    Some(c.get(1).or(c.get(2))?.as_str().to_string()).filter(|n| !n.to_ascii_lowercase().contains("slug"))
}

/// Kalıtım ve gömme ilişkileri: `class Page extends Entity`, `class X(Base)`, `class X < ApplicationRecord`,
/// `class X : Base`, Go yapısında gömülü tip (`global.GVA_MODEL`)
fn ustler(dil: Dil, m: &str, baslar: &[usize]) -> Vec<(String, String, usize)> {
    static D: OnceLock<[Regex; 5]> = OnceLock::new();
    let [extends, py, rb, cs, go] = D.get_or_init(|| {
        [
            Regex::new(r"\bclass\s+(\w+)(?:<[^>]*>)?\s+extends\s+([\w\\.]+)").unwrap(),
            Regex::new(r"(?m)^\s*class\s+(\w+)\s*\(\s*([\w.]+)").unwrap(),
            Regex::new(r"(?m)^\s*class\s+(\w+)\s*<\s*([\w:]+)").unwrap(),
            Regex::new(r"\bclass\s+(\w+)(?:<[^>]*>)?\s*:\s*([\w.]+)").unwrap(),
            Regex::new(r"(?m)^type\s+(\w+)\s+struct\s*\{").unwrap(),
        ]
    });
    let son = |s: &str| s.rsplit(['.', '\\', ':']).next().unwrap_or(s).to_string();
    let mut v = vec![];
    let mut ekle = |r: &Regex| {
        for c in r.captures_iter(m) {
            v.push((c[1].to_string(), son(&c[2]), satir_no(baslar, c.get(0).map_or(0, |x| x.start()))));
        }
    };
    match dil {
        Dil::Js | Dil::Jvm | Dil::Php => ekle(extends),
        Dil::Py => ekle(py),
        Dil::Rb => ekle(rb),
        Dil::Cs => ekle(cs),
        Dil::Go => {
            static G: OnceLock<Regex> = OnceLock::new();
            let gomulu = G.get_or_init(|| Regex::new(r"^\s*\*?(?:\w+\.)?([A-Z]\w*)\s*(?:`[^`]*`)?\s*$").unwrap());
            for c in go.captures_iter(m) {
                let bas = c.get(0).map_or(0, |x| x.end());
                for (i, l) in m[bas..].lines().enumerate() {
                    if l.trim_start().starts_with('}') {
                        break;
                    }
                    if let Some(g) = gomulu.captures(l) {
                        v.push((c[1].to_string(), g[1].to_string(), satir_no(baslar, bas) + i));
                    }
                }
            }
        }
    }
    v
}

/// Alan, yapıcı parametresi ve değişkenlerin tipleri: alıcının hangi sınıfa ait olduğunu söyler
fn alan_tipleri(dil: Dil, m: &str) -> HashMap<String, String> {
    static D: OnceLock<HashMap<u8, Vec<Regex>>> = OnceLock::new();
    let d = D.get_or_init(|| {
        let r = |x: &str| Regex::new(x).unwrap();
        let katman = r"([A-Z]\w*(?:Service|Services|Repository|Repo|Client|Manager|Dao|Store|Provider|Gateway|Handler|UseCase|Usecase|Context|Api|Helper|Mailer|Notifier|Sender))";
        let mut h = HashMap::new();
        // TS alanı / yapıcı parametresi; göreli modül içe aktarması (`const servis = require('./siparis.service')`,
        // `import * as servis from './siparis.service'`): tip, modül dosyasının adıdır
        h.insert(
            0u8,
            vec![
                r(r"\b(?:private|public|protected|readonly)\s+(?:readonly\s+)?([A-Za-z_$][\w$]*)\s*[?!]?\s*:\s*([A-Za-z_$][\w$]*)"),
                r(r#"\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*require\(\s*['"]\.{1,2}/(?:[^'"]*/)?([^'"/]+?)(?:\.[cm]?[jt]s)?['"]"#),
                r(r#"\bimport\s+(?:\*\s+as\s+)?([A-Za-z_$][\w$]*)\s+from\s+['"]\.{1,2}/(?:[^'"]*/)?([^'"/]+?)(?:\.[cm]?[jt]s)?['"]"#),
            ],
        );
        h.insert(1u8, vec![r(r"self\.(\w+)\s*=\s*(\w+)\("), r(r"\b(\w+)\s*:\s*(?:Annotated\[\s*)?([A-Z]\w+)")]);
        h.insert(2u8, vec![r(&format!(r"{katman}(?:<[^>]*>)?\s+(_?[a-z]\w*)\b")), r(r"\b(?:val|var)\s+(\w+)\s*:\s*([A-Z]\w*)")]);
        h.insert(5u8, vec![r(r"\b(?:private|protected|public)\s+(?:readonly\s+)?\??\\?(?:[A-Za-z_]\w*\\)*([A-Z]\w*)\s+\$(\w+)")]);
        h.insert(4u8, vec![r(r"(?m)^\s*(?:var\s+)?(\w+)\s*(?::=|=)\s*&?(?:\w+\.)*([A-Z]\w*)\s*(?:\{|$)"), r(r"(?m)^\s+(\w+)\s+\*?(?:\w+\.)?([A-Z]\w*)\s*$")]);
        h
    });
    let anahtar = match dil {
        Dil::Js => 0,
        Dil::Py => 1,
        Dil::Jvm | Dil::Cs => 2,
        Dil::Go => 4,
        Dil::Php => 5,
        Dil::Rb => return HashMap::new(),
    };
    let mut v = HashMap::new();
    for (i, r) in d.get(&anahtar).into_iter().flatten().enumerate() {
        for c in r.captures_iter(m) {
            // JVM / C# katman deseninde sıra (tip, ad); ötekilerde (ad, tip)
            let (ad, tip) = if anahtar == 2 && i == 0 { (c[2].to_string(), c[1].to_string()) } else { (c[1].to_string(), c[2].to_string()) };
            v.entry(ad).or_insert(tip);
        }
    }
    v
}

/// Ham bulguyu hedefiyle birlikte dış çağrıya çevirir. Göreli adres (uygulamanın kendisi) dış çağrı değildir
fn cozumle(h: Ham, fonk: &str, dosya: &str, b: &Baglam, satir: usize) -> Option<Dis> {
    let (mut hedef, mut hedef_turu, mut yol, mut guven): Cozum = match (&h.sdk, &h.ifade) {
        (Some(s), None) => {
            // SDK paketi dosyada içe aktarıldıysa kesin
            let ice = b.var(&s.to_ascii_lowercase()) || b.var(&h.istemci.to_ascii_lowercase());
            (s.clone(), "sdk", None, if ice { "certain" } else { "inferred" })
        }
        (_, Some(e)) => hedef_coz(e, fonk, dosya, 0),
        (None, None) => ("dinamik".into(), "dinamik", None, "unknown"),
    };
    // Göreli adres: taban adresi DI kaydından gelebilecek istemcilerde (typed HttpClient, NestJS HttpService, Spring
    // WebClient / RestClient / RestTemplate, Ktor `defaultRequest`) taramanın sonunda çözülür (`tabanlari_uygula`); ötekiler uygulamanın kendisidir
    if hedef_turu == "goreli" && !matches!(h.istemci.as_str(), "HttpClient" | "HttpService" | "WebClient" | "RestClient" | "RestTemplate" | "Ktor HttpClient") {
        return None;
    }
    if let (Some(s), true) = (&h.sdk, hedef_turu == "dinamik") {
        // smtplib.SMTP(host): host çözülemezse SDK'nın kendisi
        (hedef, hedef_turu, yol, guven) = (s.clone(), "sdk", None, "inferred");
    }
    let mut tur = h.tur;
    if tur == "http" {
        let a = format!("{hedef} {}", yol.as_deref().unwrap_or("")).to_ascii_lowercase();
        if ["paypal", "stripe", "iyzico", "adyen", "braintree", "checkout.com", "squareup", "klarna", "razorpay", "mollie", "payu", "paddle"].iter().any(|p| a.contains(p)) {
            tur = "odeme";
        } else if ["mailgun", "sendgrid", "postmark", "mandrill", "mailchimp", "sparkpost", "mailjet", "smtp"].iter().any(|p| a.contains(p)) {
            tur = "eposta";
        }
    }
    let yazma = h.yazma.unwrap_or(!matches!(h.yontem.as_deref(), Some("GET" | "HEAD" | "OPTIONS")));
    Some(Dis { tur, istemci: h.istemci, yontem: h.yontem, hedef, hedef_turu, yol, kanal: h.kanal, yazma, ertelenmis: h.ertelenmis, guven, satir, alici: h.alici })
}

// ---------------------------------------------------------------------------------------------------------------
// İşleme bağlama

/// Adı küçük harfe ve harf-rakama indirger
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Katman son eklerini atar: `assetRepository` -> "asset", `user.service` -> "user"
fn kok_ad(s: &str) -> String {
    let mut n = norm(s);
    loop {
        let once = n.len();
        for e in [
            "repository",
            "repositories",
            "repo",
            "services",
            "service",
            "svc",
            "client",
            "manager",
            "dao",
            "store",
            "provider",
            "helpers",
            "helper",
            "handler",
            "usecase",
            "interactor",
            "gateway",
            "adapter",
            "controller",
            "utils",
            "util",
            "impl",
            "model",
            "models",
        ] {
            if n.len() > e.len() + 2 {
                if let Some(x) = n.strip_suffix(e) {
                    n = x.to_string();
                }
            }
        }
        if n.len() == once {
            return n;
        }
    }
}

const DURAK: [&str; 64] = [
    "log",
    "info",
    "warn",
    "error",
    "debug",
    "push",
    "map",
    "filter",
    "reduce",
    "forEach",
    "then",
    "catch",
    "finally",
    "json",
    "status",
    "parseInt",
    "String",
    "Number",
    "Boolean",
    "Object",
    "Array",
    "Promise",
    "resolve",
    "reject",
    "setTimeout",
    "print",
    "len",
    "str",
    "int",
    "isinstance",
    "range",
    "list",
    "dict",
    "set",
    "join",
    "split",
    "replace",
    "trim",
    "toString",
    "includes",
    "indexOf",
    "slice",
    "keys",
    "values",
    "entries",
    "assign",
    "stringify",
    "parse",
    "require",
    "append",
    "format",
    "sprintf",
    "printf",
    "Println",
    "Printf",
    "Errorf",
    "Sprintf",
    "make",
    "len",
    "isEmpty",
    "equals",
    "hashCode",
    "ToString",
    "Add",
];

impl Tarama {
    fn blok_metni_satirlari(&self, i: usize) -> (usize, usize, usize) {
        let b = &self.bloklar[i];
        (b.dosya, b.bas, b.son)
    }

    /// Kapının işleyici bloğu: aynı dosyada fonksiyon adıyla (kapının satırına en yakın), yoksa satırdan başlayan /
    /// satırı içeren blok; dosyada yoksa projede tek adlı fonksiyon
    fn isleyici(&self, dosya: &str, satir: usize, fonksiyon: &str, kontrolcu: &str) -> Option<usize> {
        let ad = fonksiyon.rsplit(['.', '#', ':', '@', '\\']).next().unwrap_or(fonksiyon).trim();
        let di = self.yol_dizini.get(dosya).copied();
        if let Some(di) = di {
            let adaylar: Vec<usize> = self.ad_dizini.get(ad).into_iter().flatten().copied().filter(|&b| self.bloklar[b].dosya == di).collect();
            if let Some(b) = adaylar.iter().copied().min_by_key(|&b| {
                let x = &self.bloklar[b];
                // Kapının satırı genelde dekoratör ya da tanım satırıdır: tanım ondan sonra gelir
                if x.bas + 2 >= satir {
                    x.bas.abs_diff(satir)
                } else {
                    1000 + satir - x.bas
                }
            }) {
                return Some(b);
            }
            let ayni: Vec<usize> = (0..self.bloklar.len()).filter(|&b| self.bloklar[b].dosya == di).collect();
            // Satırda ya da hemen sonrasında başlayan (adsız işleyici: `router.post('/', async (req, res) => {`)
            if let Some(b) = ayni
                .iter()
                .copied()
                .filter(|&b| self.bloklar[b].bas >= satir && self.bloklar[b].bas <= satir + 8)
                .min_by_key(|&b| (self.bloklar[b].bas, usize::MAX - self.bloklar[b].son))
            {
                return Some(b);
            }
            // İşleyici başka dosyada adıyla (rota dosyası denetleyiciye işaret ediyor): kaydı içeren blok değil
            if let Some(b) = self.ada_gore(ad, kontrolcu) {
                return Some(b);
            }
            return ayni.iter().copied().filter(|&b| self.bloklar[b].bas <= satir && satir <= self.bloklar[b].son).min_by_key(|&b| self.bloklar[b].son - self.bloklar[b].bas);
        }
        self.ada_gore(ad, kontrolcu)
    }

    /// Projede adıyla: tek aday, ya da denetleyicinin sınıfına / dosya adına uyan, işleyici katmanındaki aday
    fn ada_gore(&self, ad: &str, kontrolcu: &str) -> Option<usize> {
        let adaylar = self.ad_dizini.get(ad)?;
        if let [tek] = adaylar.as_slice() {
            return Some(*tek);
        }
        let uyan: Vec<usize> = adaylar.iter().copied().filter(|&b| !kontrolcu.is_empty() && self.uyar(b, kontrolcu)).collect();
        let katman = |b: usize| {
            let y = self.dosyalar[self.bloklar[b].dosya].yol.to_ascii_lowercase();
            !["api", "apis", "controller", "controllers", "handler", "handlers", "routes", "router", "views", "endpoints", "rest", "web"]
                .iter()
                .any(|k| y.split('/').any(|p| p == *k))
        };
        uyan.iter().copied().min_by_key(|&b| (katman(b), self.bloklar[b].bas))
    }

    /// Dosyada ilk satırı (±1) tutan blok; yoksa aralığı kapsayan en dar blok
    fn blok_bul(&self, dosya: &str, bas: usize, son: usize) -> Option<usize> {
        let di = *self.yol_dizini.get(dosya)?;
        let ayni = (0..self.bloklar.len()).filter(|&b| self.bloklar[b].dosya == di);
        ayni.clone()
            .find(|&b| self.bloklar[b].bas.abs_diff(bas) <= 1)
            .or_else(|| ayni.filter(|&b| self.bloklar[b].bas <= bas && son <= self.bloklar[b].son).min_by_key(|&b| self.bloklar[b].son - self.bloklar[b].bas))
    }

    /// Blok, adı verilen sınıfa / tipe ait mi: bloğun sınıfı ya da dosyasının adı (katman son ekleri atılarak) uyuyor
    fn uyar(&self, b: usize, tip: &str) -> bool {
        // C# arayüz adı: IBasketService -> BasketService
        let tip = match tip.strip_prefix('I') {
            Some(r) if r.starts_with(|c: char| c.is_ascii_uppercase()) => r,
            _ => tip,
        };
        let (tn, tk) = (norm(tip), kok_ad(tip));
        if tn.len() < 2 {
            return false;
        }
        let x = &self.bloklar[b];
        let yol = &self.dosyalar[x.dosya].yol;
        let govde = yol.rsplit('/').next().unwrap_or(yol);
        let govde = govde.split('.').filter(|p| !matches!(*p, "ts" | "js" | "mjs" | "cjs" | "py" | "java" | "kt" | "cs" | "go" | "php" | "rb")).collect::<Vec<_>>().join(".");
        let esit = |a: &str| norm(a) == tn || (tk.len() >= 3 && kok_ad(a) == tk);
        esit(&govde) || x.sinif.as_deref().is_some_and(esit)
    }

    /// Çağrının olası hedefleri: alıcının adı dosya adına uyan (ör. `this.assetService.x` -> asset.service.ts), alıcı
    /// `this` / `self` ise aynı dosyadaki, alıcısız ise aynı dosyadaki ya da projede tek
    fn hedefler(&self, kaynak_dosya: usize, alici: Option<&str>, ad: &str) -> Vec<usize> {
        if DURAK.contains(&ad) {
            return vec![];
        }
        let Some(adaylar) = self.ad_dizini.get(ad) else { return vec![] };
        if alici == Some("?") {
            return vec![];
        }
        let kendi = |a: &str| matches!(a, "this" | "self" | "$this" | "cls" | "static" | "super" | "base");
        let ayni: Vec<usize> = adaylar.iter().copied().filter(|&b| self.bloklar[b].dosya == kaynak_dosya).collect();
        match alici {
            Some(a) if !kendi(a) => {
                // Alıcının bildirilmiş tipi (`private service: SearchService`) varsa o; yoksa alıcının adı
                let a = a.trim_start_matches('$');
                let tipli = self.dosyalar[kaynak_dosya].sonuc.alan_tipleri.get(a).map(|t| adaylar.iter().copied().filter(|&b| self.uyar(b, t)).collect::<Vec<_>>());
                let v = match tipli {
                    Some(v) if !v.is_empty() => v,
                    _ => adaylar.iter().copied().filter(|&b| self.uyar(b, a)).collect(),
                };
                if v.len() <= 3 {
                    v
                } else {
                    vec![]
                }
            }
            // `self.save()` / `this.x()`: yöntem nesnenin sınıfında (aynı dosyada) ya da çerçevenin taban sınıfındadır;
            // projede aynı adı taşıyan başka bir dosyadaki fonksiyon hedef değildir
            Some(_) => ayni,
            None => {
                // Alıcısız çağrı PHP, Python, JavaScript / TypeScript ve Go'da bir sınıfın yöntemine gidemez: Laravel'in
                // küresel `redirect()` yardımcısı aynı adlı `SocialiteController::redirect` değildir
                let uzanti = self.dosyalar[kaynak_dosya].yol.rsplit('.').next().unwrap_or("");
                let (ayni, adaylar) = if matches!(uzanti, "php" | "py" | "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "go") {
                    let yalin = |b: &usize| self.bloklar[*b].sinif.is_none();
                    (ayni.into_iter().filter(yalin).collect::<Vec<_>>(), adaylar.iter().copied().filter(yalin).collect::<Vec<_>>())
                } else {
                    (ayni, adaylar.clone())
                };
                if !ayni.is_empty() {
                    ayni
                } else if adaylar.len() == 1 {
                    adaylar.clone()
                } else {
                    vec![]
                }
            }
        }
    }

    /// Bloğun içindeki çağrılar (iç içe adsız bloklar dahil); `haric` satır aralıklarındakiler (başka yöntemin dalı) hariç
    fn blok_cagrilari(&self, b: usize, haric: &[(usize, usize)]) -> Vec<(Option<String>, String)> {
        let (d, bas, son) = self.blok_metni_satirlari(b);
        let kendi = &self.bloklar[b].ad;
        self.dosyalar[d]
            .sonuc
            .cagrilar
            .iter()
            .filter(|(s, _, a)| *s >= bas && *s <= son && !(*s == bas && a == kendi) && !haric.iter().any(|(x, y)| s >= x && s <= y))
            .map(|(_, r, a)| (r.clone(), a.clone()))
            .collect()
    }
}

/// Çağrı grafiğinin işlem başına ulaştığı fonksiyonlar: "işlem-kimliği dosya:satır" -> (dosya, ilk satır, son satır,
/// derinlik, ad). Yerel ad eşlemesinin bulamadığı halkaları tamamlar
pub type CagriErisimi = HashMap<String, Vec<(String, usize, usize, usize, String)>>;

/// İşlemin zincirinde bulunan bir öğe: blok, derinlik, ertelenmiş mi, yol (fonksiyon adları)
struct Halka {
    blok: usize,
    derinlik: usize,
    ertelenmis: bool,
    yol: Vec<String>,
}

/// İşleyiciden başlayan zincir (ad eşlemesiyle; en çok `EN_FAZLA_DERINLIK` adım). `dinleyiciler`: işlemin
/// tetiklediği olay / işlerin dinleyici blokları (ertelenmiş, 1. adım). `haric`: işleyicide işlemin yönteminde çalışmayan
/// satırlar
fn zincir(t: &Tarama, bas: usize, dinleyiciler: &[usize], haric: &[(usize, usize)]) -> Vec<Halka> {
    let ad = |b: usize| -> String {
        let x = &t.bloklar[b];
        if x.ad.is_empty() {
            format!("{}:{}", t.dosyalar[x.dosya].yol, x.bas)
        } else {
            x.ad.clone()
        }
    };
    let mut v = vec![Halka { blok: bas, derinlik: 0, ertelenmis: false, yol: vec![ad(bas)] }];
    let mut gorulen: BTreeSet<usize> = BTreeSet::from([bas]);
    for &d in dinleyiciler {
        if gorulen.insert(d) {
            v.push(Halka { blok: d, derinlik: 1, ertelenmis: true, yol: vec![ad(bas), ad(d)] });
        }
    }
    let mut i = 0;
    while i < v.len() && gorulen.len() < EN_FAZLA_FONKSIYON {
        let (b, derinlik, ertelenmis, yol) = (v[i].blok, v[i].derinlik, v[i].ertelenmis, v[i].yol.clone());
        i += 1;
        if derinlik >= EN_FAZLA_DERINLIK {
            continue;
        }
        let kaynak = t.bloklar[b].dosya;
        for (alici, cagri) in t.blok_cagrilari(b, if b == bas { haric } else { &[] }) {
            for h in t.hedefler(kaynak, alici.as_deref(), &cagri) {
                if gorulen.len() >= EN_FAZLA_FONKSIYON || !gorulen.insert(h) {
                    continue;
                }
                let mut y = yol.clone();
                y.push(ad(h));
                v.push(Halka { blok: h, derinlik: derinlik + 1, ertelenmis, yol: y });
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------------------------
// Harita ile birleştirme

/// En zayıf güven
fn zayif(a: &'static str, b: &'static str) -> &'static str {
    let s = |x: &str| match x {
        "certain" => 3,
        "observed" => 2,
        "inferred" => 1,
        _ => 0,
    };
    if s(a) <= s(b) {
        a
    } else {
        b
    }
}

fn dis_json(d: &Dis, dosya: &str, fonksiyon: &str, yol: &[String], derinlik: usize, ertelenmis: bool) -> Value {
    let erisim = if derinlik == 0 { "certain" } else { "inferred" };
    let mut v = json!({
        "tur": d.tur,
        "istemci": d.istemci,
        "yontem": d.yontem,
        "hedef": d.hedef,
        "hedef_turu": d.hedef_turu,
        "dugum": d.dugum(),
        "yazma": d.yazma,
        "zaman": if ertelenmis || d.ertelenmis { "ertelenmis" } else { "senkron" },
        "kanit": format!("{dosya}:{}", d.satir),
        "fonksiyon": fonksiyon,
        "derinlik": derinlik,
        "guven": zayif(d.guven, erisim),
    });
    if let Some(y) = &d.yol {
        v["yol"] = json!(y);
    }
    if let Some(k) = &d.kanal {
        v["kanal"] = json!(k);
    }
    if yol.len() > 1 {
        v["zincir"] = json!(yol);
    }
    v
}

/// Bir fonksiyon aralığında satır desenleriyle bulunan dış çağrılar: çağrı grafiğinin polimorfik izlemesi (dis_zincir.rs)
/// ulaştığı fonksiyonlara da bakar. Zincirden ulaşıldığı için güven en fazla inferred
pub fn araliktaki_disler(t: &Tarama, dosya: &str, bas: usize, son: usize, fonksiyon: &str, yol: &[String], derinlik: usize) -> Vec<Value> {
    let Some(&di) = t.yol_dizini.get(dosya) else { return vec![] };
    t.dosyalar[di].sonuc.disler.iter().filter(|d| d.satir >= bas && d.satir <= son).map(|d| dis_json(d, dosya, fonksiyon, yol, derinlik.max(1), false)).collect()
}

/// Dış etkinin kısa adı (onay gerekçesi ve özetler için)
pub fn etki_adi(d: &Value) -> String {
    let tur = match d["tur"].as_str().unwrap_or("") {
        "http" => "HTTP",
        "kuyruk" => t!("kuyruk", "queue"),
        "eposta" => t!("e-posta", "e-mail"),
        "odeme" => t!("ödeme", "payment"),
        "bulut" => t!("bulut", "cloud"),
        "bildirim" => t!("bildirim", "notification"),
        x => x,
    };
    let hedef = match d["hedef_turu"].as_str() {
        Some("ortam") => format!("${}", d["hedef"].as_str().unwrap_or("")),
        _ => d["hedef"].as_str().unwrap_or("").to_string(),
    };
    let y = d["yontem"].as_str().map(|y| format!("{y} ")).unwrap_or_default();
    format!("{tur} {y}{hedef}{}", d["yol"].as_str().unwrap_or(""))
}

/// İşlemin olay / iş dinleyicilerinin blokları (`ozet.olaylar`'daki dinleyici kanıtından)
fn dinleyici_bloklari(t: &Tarama, v: &Value, olaylar: &Value) -> Vec<usize> {
    let mut sonuc = vec![];
    for tt in v["tetikler"].as_array().into_iter().flatten() {
        let id = format!("{}:{}", tt["tur"].as_str().unwrap_or(""), tt["ad"].as_str().unwrap_or(""));
        for d in olaylar[id.as_str()]["dinleyiciler"].as_array().into_iter().flatten() {
            let Some((dosya, satir)) = d["kanit"].as_str().and_then(|k| k.rsplit_once(':')).and_then(|(a, b)| Some((a.to_string(), b.parse::<usize>().ok()?))) else { continue };
            if let Some(b) = t.isleyici(&dosya, satir, d["fonksiyon"].as_str().unwrap_or(""), "") {
                sonuc.push(b);
            }
        }
    }
    sonuc
}

/// Model / tablo adlarını karşılaştırmak için: küçük harf, alt çizgisiz, sondaki çoğul eki atılmış
fn model_anahtari(s: &str) -> String {
    let n = norm(s);
    n.strip_suffix("ies").map(|x| format!("{x}y")).or_else(|| n.strip_suffix('s').map(String::from)).unwrap_or(n)
}

/// İşlemin geri alınabilirlik kararı ve gerekçesi
struct GeriAlma {
    karar: &'static str,
    gerekce: Vec<String>,
    konum: Option<String>,
}

/// POST'un oluşturduğu kaydı silen uç: yol + tek parametre (tek kaydı silen uç); aynı yoldaki DELETE yalnızca yol tekil
/// bir kaynak ya da bir kaydın alt kaynağıysa (koleksiyonun toplu silmesi tek kaydı oluşturan işlemin tersi değildir)
fn silme_ucu<'k>(k: &crate::adaptor::Kapi, kapilar: &'k [crate::adaptor::Kapi]) -> Option<&'k crate::adaptor::Kapi> {
    let p = |y: &str| y.trim_end_matches('/').to_string();
    let yol = p(&k.yol);
    let silme = kapilar.iter().filter(|d| d.yontem == "DELETE" && d.ana);
    silme
        .clone()
        .find(|d| p(&d.yol).strip_prefix(&yol).is_some_and(|r| r.starts_with('/') && r[1..].split('/').count() == 1 && parametre_mi(&r[1..])))
        .or_else(|| silme.clone().find(|d| p(&d.yol) == yol && !koleksiyon_yolu(&yol)))
}

/// Yol parçası bir parametre mi (`:id`, `{id}`, `<int:id>`)
fn parametre_mi(p: &str) -> bool {
    p.starts_with(':') || p.starts_with('{') || p.starts_with('<')
}

/// Yol bir koleksiyonun kendisi mi: son parçası çoğul bir ad ve önünde kayıt kimliği yok (`/api/people`,
/// `/api/assets`, `/etiketler`). `/albums/:id/assets` bir kaydın alt koleksiyonudur; `/auth/pin-code` tekil kaynaktır
fn koleksiyon_yolu(yol: &str) -> bool {
    let parcalar: Vec<&str> = yol.trim_end_matches('/').split('/').filter(|p| !p.is_empty()).collect();
    let Some((son, onceki)) = parcalar.split_last() else { return false };
    if parametre_mi(son) || onceki.iter().any(|p| parametre_mi(p)) {
        return false;
    }
    let n = norm(son);
    matches!(n.as_str(), "people" | "children" | "media" | "data")
        || (n.ends_with('s') && !n.ends_with("ss") && !n.ends_with("us") && n.len() > 3)
        || ((n.ends_with("ler") || n.ends_with("lar")) && n.len() > 4)
}

/// Yazmanın türü (geri alma eşlemesi için): ekleme (kayıt oluşturma, alt koleksiyona ekleme, işaret koyma) ya da
/// güncelleme (var olan değerin üzerine yazma; tersi silme değildir)
#[derive(PartialEq, Clone, Copy)]
enum YazmaTuru {
    Ekleme,
    Guncelleme,
}

/// İşlemin yazma türü: fonksiyon adının ilk sözcüğü (İngilizce fiil) ya da son sözcüğü (Türkçe fiil); ad bir şey
/// söylemiyorsa yöntem: PATCH / PUT güncelleme, POST ekleme (yol bir kaydın kimliğiyle bitiyorsa güncelleme)
fn yazma_turu(k: &crate::adaptor::Kapi) -> YazmaTuru {
    const EKLEME: [&str; 31] = [
        "create",
        "add",
        "insert",
        "new",
        "setup",
        "register",
        "link",
        "attach",
        "assign",
        "tag",
        "follow",
        "favorite",
        "favourite",
        "like",
        "star",
        "flag",
        "mark",
        "lock",
        "join",
        "subscribe",
        "upload",
        "vote",
        "upvote",
        "bookmark",
        "ekle",
        "olustur",
        "kur",
        "isaretle",
        "bagla",
        "takip",
        "yukle",
    ];
    const GUNCELLEME: [&str; 18] = [
        "update",
        "edit",
        "change",
        "set",
        "upsert",
        "modify",
        "patch",
        "replace",
        "save",
        "rename",
        "move",
        "reorder",
        "sort",
        "overwrite",
        "guncelle",
        "degistir",
        "duzenle",
        "yenile",
    ];
    let ad = k.fonksiyon.rsplit(['.', '#', ':', '@', '\\']).next().unwrap_or(&k.fonksiyon);
    let mut sozcukler: Vec<String> = vec![];
    for c in ad.chars() {
        if c == '_' || c == '-' {
            sozcukler.push(String::new());
        } else if c.is_uppercase() || sozcukler.is_empty() {
            sozcukler.push(c.to_lowercase().collect());
        } else if let Some(s) = sozcukler.last_mut() {
            s.push(c);
        }
    }
    sozcukler.retain(|s| !s.is_empty());
    let tur = |s: &str| {
        if EKLEME.contains(&s) {
            Some(YazmaTuru::Ekleme)
        } else if GUNCELLEME.contains(&s) {
            Some(YazmaTuru::Guncelleme)
        } else {
            None
        }
    };
    if let Some(t) = sozcukler.first().and_then(|s| tur(s)).or_else(|| sozcukler.last().and_then(|s| tur(s))) {
        return t;
    }
    match k.yontem.as_str() {
        "POST" if !k.yol.trim_end_matches('/').rsplit('/').next().is_some_and(parametre_mi) => YazmaTuru::Ekleme,
        _ => YazmaTuru::Guncelleme,
    }
}

/// Genel geri yükleme ucu (çöp kutusu): yolunda restore / undelete / untrash / recover / recycle geçen yazma
fn genel_geri_yukleme(kapilar: &[crate::adaptor::Kapi]) -> Option<&crate::adaptor::Kapi> {
    kapilar.iter().find(|t| t.yontem != "GET" && t.yol.split('/').any(|p| matches!(p.to_lowercase().as_str(), "restore" | "undelete" | "untrash" | "recover")))
}

/// Davranış özetine dış etkileri, ek ön koşulları, işlem sınırını ve geri alınabilirliği ekler; uygulama düzeyindeki
/// dış çağrı envanterini döndürür (`ozet.dis`). Okunan dosyalar (dayanak) `okunan`a eklenir. Onay gerekçesi olan
/// kapılar: "işlem-kimliği dosya:satır" -> gerekçe
pub fn isle(
    t: &Tarama,
    kapilar: &[crate::adaptor::Kapi],
    dna: &mut Value,
    olaylar: &Value,
    cagri: &CagriErisimi,
    okunan: &mut Vec<(PathBuf, String)>,
) -> (Value, BTreeMap<String, String>) {
    if !dna.is_object() {
        *dna = json!({});
    }
    let mut yumusak: Vec<(String, String, String)> =
        t.dosyalar.iter().flat_map(|d| d.sonuc.yumusak_modeller.iter().map(move |(m, s, i)| (model_anahtari(m), format!("{}:{s}", d.yol), format!("{m} ({i})")))).collect();
    // Taban sınıftan / gömülü yapıdan gelen yumuşak silme (`class Page extends Entity`, `global.GVA_MODEL`)
    let ustler: Vec<(String, String, String)> =
        t.dosyalar.iter().flat_map(|d| d.sonuc.ustler.iter().map(move |(a, u, s)| (a.clone(), u.clone(), format!("{}:{s}", d.yol)))).collect();
    for _ in 0..6 {
        let mut yeni = vec![];
        for (alt, ust, kn) in &ustler {
            let (ak, uk) = (model_anahtari(alt), model_anahtari(ust));
            if !yumusak.iter().any(|x| x.0 == ak) && !yeni.iter().any(|x: &(String, String, String)| x.0 == ak) {
                if let Some(u) = yumusak.iter().find(|x| x.0 == uk) {
                    yeni.push((ak, kn.clone(), format!("{alt} ← {}", u.2)));
                }
            }
        }
        if yeni.is_empty() {
            break;
        }
        yumusak.extend(yeni);
    }
    let genel_geri = genel_geri_yukleme(kapilar);
    // Sürüm / geçmiş tutan modeller: (model anahtarı, kanıt, açıklama)
    let gecmis_modelleri_tum: Vec<(String, String, String)> =
        t.dosyalar.iter().flat_map(|d| d.sonuc.gecmis_modeller.iter().map(move |(m, s, i)| (model_anahtari(m), format!("{}:{s}", d.yol), format!("{m} ({i})")))).collect();
    let mut kullanilan: BTreeSet<usize> = BTreeSet::new();
    // (dosya, satır) -> işlem kimlikleri
    let mut erisen: BTreeMap<(usize, usize), BTreeSet<String>> = BTreeMap::new();
    let mut onaylar = BTreeMap::new();
    let mut eklenen_ret = 0usize;
    // Aynı satırda birden fazla işlem (`router.route('/:id').get(...).put(...).delete(...)`) aynı davranış özetini
    // paylaşır: her işlem adaptörün özgün özetinden başlar, sonuçlar en temkinli olan korunarak birleştirilir
    let asil = dna.clone();
    let mut islenen: BTreeSet<String> = BTreeSet::new();
    for k in kapilar {
        let yer = format!("{}:{}", k.dosya, k.satir);
        let okuma = matches!(k.yontem.as_str(), "GET" | "HEAD" | "OPTIONS");
        let mut v = asil.get(&yer).cloned().unwrap_or_else(|| json!({ "hatalar": [], "sahiplik_denetimi": [], "dallar": [], "yumusak_silme": null, "kapsam": "acik" }));
        // Önceki adımın yer tutucusu ("bilinmiyor"): karar burada verilir
        if let Some(o) = v.as_object_mut() {
            o.remove("geri_alinabilir");
        }
        // Çağrı grafiğinin ulaştığı fonksiyonlar (varsa): bu taramanın bloklarına çevrilir
        let grafikten: Vec<(usize, usize, String)> = cagri
            .get(&format!("{} {yer}", k.id))
            .into_iter()
            .flatten()
            .filter_map(|(d, bas, son, derinlik, ad)| t.blok_bul(d, *bas, *son).map(|b| (b, *derinlik, ad.clone())))
            .collect();
        let Some(bas) = t.isleyici(&k.dosya, k.satir, &k.fonksiyon, &k.kontrolcu).or_else(|| grafikten.iter().find(|x| x.1 == 0).map(|x| x.0)) else {
            if !okuma {
                let g = geri_alma_karari(k, kapilar, &v, &[], &[], &[], &yumusak, genel_geri, None, &Gecmis { yazma: None, modeller: &gecmis_modelleri_tum });
                geri_yaz(&mut v, &g);
                if let Some(o) = onay_gerekcesi(k, &v, &g) {
                    onaylar.insert(format!("{} {yer}", k.id), o.clone());
                    v["onay_gerekcesi"] = json!(o);
                }
                birlestir(dna, &yer, v, &mut islenen);
            }
            continue;
        };
        // Birden çok yöntemi karşılayan işleyicide başka yöntemin dalı (`if request.method == "POST":`) bu işlemin değildir
        let haric = {
            let xb = &t.bloklar[bas];
            crate::cagri::yontem_disi(&t.dosyalar[xb.dosya].sonuc.yontem_dallari, xb.bas, xb.son, &k.yontem)
        };
        let mut halkalar = zincir(t, bas, &dinleyici_bloklari(t, &v, olaylar), &haric);
        let isleyici_adi = halkalar.first().and_then(|h| h.yol.first().cloned()).unwrap_or_default();
        for (b, derinlik, ad) in grafikten {
            if !halkalar.iter().any(|h| h.blok == b) {
                halkalar.push(Halka { blok: b, derinlik, ertelenmis: false, yol: vec![isleyici_adi.clone(), format!("… {ad}")] });
            }
        }
        let mut disler: Vec<Value> = vec![];
        let mut silmeler: Vec<(String, Silme)> = vec![];
        let mut yumusak_cagri: Vec<String> = vec![];
        let mut sinir: Option<(String, String)> = None;
        let mut gecmis: Option<(String, String)> = None;
        let mut hatalar = v["hatalar"].as_array().cloned().unwrap_or_default();
        let mevcut: BTreeSet<String> = hatalar.iter().filter_map(|h| h["kanit"].as_str().map(String::from)).collect();
        for h in &halkalar {
            let x = &t.bloklar[h.blok];
            let d = &t.dosyalar[x.dosya];
            let icinde = |s: usize| s >= x.bas && s <= x.son && !(h.blok == bas && haric.iter().any(|(a, b)| s >= *a && s <= *b));
            let fonksiyon = h.yol.last().cloned().unwrap_or_default();
            for dd in d.sonuc.disler.iter().filter(|dd| icinde(dd.satir)) {
                kullanilan.insert(x.dosya);
                erisen.entry((x.dosya, dd.satir)).or_default().insert(k.id.clone());
                let kanit = format!("{}:{}", d.yol, dd.satir);
                if disler.len() < EN_FAZLA_DIS && !disler.iter().any(|z| z["kanit"] == kanit.as_str()) {
                    disler.push(dis_json(dd, &d.yol, &fonksiyon, &h.yol, h.derinlik, h.ertelenmis));
                }
            }
            for r in d.sonuc.retler.iter().filter(|r| icinde(r.satir)) {
                let kanit = format!("{}:{}", d.yol, r.satir);
                if mevcut.contains(&kanit) || hatalar.iter().any(|z| z["kanit"] == kanit.as_str()) || hatalar.len() >= 48 || h.ertelenmis {
                    continue;
                }
                kullanilan.insert(x.dosya);
                let sahiplik = r.kosul.as_deref().is_some_and(|c| crate::dna::sahiplik_mi(c, r.durum));
                hatalar.push(json!({ "durum": r.durum, "istisna": r.istisna, "kosul": r.kosul, "kaynak": if h.derinlik == 0 { "isleyici" } else { "servis" }, "sahiplik": sahiplik, "kanit": kanit, "guven": "inferred", "ek": "dis" }));
                eklenen_ret += 1;
            }
            if sinir.is_none() {
                // İşlem sınırı: gövdede ya da (`@Transactional`) tanımın hemen üstünde
                if let Some((s, ifade)) = d.sonuc.islem_sinirlari.iter().find(|(s, _)| icinde(*s) || (*s < x.bas && *s + 4 >= x.bas)) {
                    kullanilan.insert(x.dosya);
                    sinir = Some((format!("{}:{s}", d.yol), ifade.clone()));
                }
            }
            // Sürüm / geçmiş kaydı yazımı (güncellemede önceki değer saklanıyor)
            if gecmis.is_none() && h.derinlik <= 2 && !h.ertelenmis {
                if let Some((s, g)) = d.sonuc.gecmis_yazmalari.iter().find(|(s, _)| icinde(*s)) {
                    kullanilan.insert(x.dosya);
                    gecmis = Some((format!("{}:{s}", d.yol), g.clone()));
                }
            }
            // Silme veri katmanında aranır; silmeyen işlemde yalnızca yakın halkalarda (derinlik ≤ 2): uzak bir yardımcıdaki
            // koşullu silme (ör. eski kapak resmini kaldırma) işlemin kendisini kalıcı silme yapmaz
            if !h.ertelenmis && (k.yontem == "DELETE" || h.derinlik <= 2) {
                for sm in d.sonuc.silmeler.iter().filter(|s| icinde(s.satir)) {
                    kullanilan.insert(x.dosya);
                    silmeler.push((format!("{}:{}", d.yol, sm.satir), sm.clone()));
                }
                for (s, y) in d.sonuc.yumusak_cagrilar.iter().filter(|(s, _)| icinde(*s)) {
                    if !matches!(y.as_str(), "restore" | "recover" | "undelete" | "undiscard") {
                        kullanilan.insert(x.dosya);
                        yumusak_cagri.push(format!("{y} ({}:{s})", d.yol));
                    }
                }
            }
        }
        if !disler.is_empty() {
            v["dis"] = json!(disler);
        }
        if hatalar.len() != v["hatalar"].as_array().map(Vec::len).unwrap_or(0) {
            v["hatalar"] = json!(hatalar);
        }
        if let Some((konum, ifade)) = &sinir {
            v["islem_siniri"] = json!({ "ifade": ifade, "kanit": konum, "guven": "inferred" });
        }
        if !okuma {
            let g = geri_alma_karari(
                k,
                kapilar,
                &v,
                &disler,
                &silmeler,
                &yumusak_cagri,
                &yumusak,
                genel_geri,
                sinir.as_ref(),
                &Gecmis { yazma: gecmis.as_ref(), modeller: &gecmis_modelleri_tum },
            );
            geri_yaz(&mut v, &g);
            if let Some(o) = onay_gerekcesi(k, &v, &g) {
                onaylar.insert(format!("{} {yer}", k.id), o.clone());
                v["onay_gerekcesi"] = json!(o);
            }
        }
        let bos = v["dis"].is_null() && v["geri_alinabilir"].is_null() && crate::dna::bos(&v) && v["islem_siniri"].is_null();
        if !bos {
            birlestir(dna, &yer, v, &mut islenen);
        }
    }
    // Uygulama düzeyi envanter: bütün dış çağrılar (işleme bağlı olsun olmasın), sistemler ve dinleyiciler
    let mut cagrilar = vec![];
    let mut sistemler: BTreeMap<String, Value> = BTreeMap::new();
    for (di, d) in t.dosyalar.iter().enumerate() {
        for dd in &d.sonuc.disler {
            let islemler: Vec<String> = erisen.get(&(di, dd.satir)).map(|s| s.iter().cloned().collect()).unwrap_or_default();
            let fonksiyon = t
                .bloklar
                .iter()
                .filter(|b| b.dosya == di && b.bas <= dd.satir && dd.satir <= b.son && !b.ad.is_empty())
                .min_by_key(|b| b.son - b.bas)
                .map(|b| b.ad.clone())
                .unwrap_or_default();
            let mut c = dis_json(dd, &d.yol, &fonksiyon, &[], 0, false);
            c["guven"] = json!(dd.guven);
            c["islemler"] = json!(islemler);
            let s = sistemler
                .entry(dd.dugum())
                .or_insert_with(|| json!({ "dugum": dd.dugum(), "tur": dd.tur, "hedef": dd.hedef, "hedef_turu": dd.hedef_turu, "guven": dd.guven, "cagri": 0, "islemler": [] }));
            s["cagri"] = json!(s["cagri"].as_u64().unwrap_or(0) + 1);
            let mut l: BTreeSet<String> = s["islemler"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
            l.extend(islemler.iter().cloned());
            s["islemler"] = json!(l);
            if !islemler.is_empty() {
                kullanilan.insert(di);
            }
            cagrilar.push(c);
        }
    }
    let dinleyici: Vec<Value> = t
        .dosyalar
        .iter()
        .flat_map(|d| d.sonuc.dinleyiciler.iter().map(move |(tur, kanal, s)| json!({ "tur": tur, "kanal": kanal, "kanit": format!("{}:{s}", d.yol), "guven": "certain" })))
        .collect();
    for (di, d) in t.dosyalar.iter().enumerate() {
        if !d.sonuc.dinleyiciler.is_empty() || !d.sonuc.disler.is_empty() || !d.sonuc.yumusak_modeller.is_empty() {
            kullanilan.insert(di);
        }
    }
    for di in kullanilan {
        let d = &t.dosyalar[di];
        if !okunan.iter().any(|(p, _)| *p == d.mutlak) {
            okunan.push((d.mutlak.clone(), d.ozet.clone()));
        }
    }
    let bagli = cagrilar.iter().filter(|c| c["islemler"].as_array().is_some_and(|a| !a.is_empty())).count();
    let ozet = json!({
        "cagrilar": cagrilar,
        "sistemler": sistemler.into_values().collect::<Vec<_>>(),
        "dinleyiciler": dinleyici,
        "yumusak_modeller": yumusak.iter().map(|(_, k, a)| json!({ "model": a, "kanit": k })).collect::<Vec<_>>(),
        "istatistik": { "dosya": t.dosyalar.len(), "fonksiyon": t.bloklar.len(), "cagri": cagrilar.len(), "isleme_bagli": bagli, "eklenen_on_kosul": eklenen_ret },
        "kapsam": "acik",
    });
    (ozet, onaylar)
}

/// Geri alma kararının ağırlığı (aynı satırı paylaşan işlemlerde en temkinlisi korunur)
fn agirlik(k: &Value) -> u8 {
    match k.as_str() {
        Some("hayir") => 3,
        Some("kismen") => 2,
        Some("bilinmiyor") => 1,
        Some("evet") => 0,
        _ => 0,
    }
}

/// İşlemin özetini haritaya yazar; aynı "dosya:satır" bu döngüde yazıldıysa birleştirir: dış etkiler ve hata
/// senaryoları birleşir, geri almada en temkinli karar ve ilk onay gerekçesi korunur
fn birlestir(dna: &mut Value, yer: &str, v: Value, islenen: &mut BTreeSet<String>) {
    if islenen.insert(yer.to_string()) {
        dna[yer] = v;
        return;
    }
    let e = &mut dna[yer];
    for alan in ["dis", "hatalar"] {
        let mut l = e[alan].as_array().cloned().unwrap_or_default();
        for x in v[alan].as_array().into_iter().flatten() {
            if !l.iter().any(|y| y["kanit"] == x["kanit"]) {
                l.push(x.clone());
            }
        }
        if !l.is_empty() {
            e[alan] = json!(l);
        }
    }
    if e["islem_siniri"].is_null() && !v["islem_siniri"].is_null() {
        e["islem_siniri"] = v["islem_siniri"].clone();
    }
    if !v["geri_alinabilir"].is_null() && (e["geri_alinabilir"].is_null() || agirlik(&v["geri_alinabilir"]) > agirlik(&e["geri_alinabilir"])) {
        for a in ["geri_alinabilir", "geri_alma_gerekce", "geri_alma_guveni", "geri_alma_konumu"] {
            e[a] = v[a].clone();
        }
    }
    if e["onay_gerekcesi"].is_null() && !v["onay_gerekcesi"].is_null() {
        e["onay_gerekcesi"] = v["onay_gerekcesi"].clone();
    }
}

fn geri_yaz(v: &mut Value, g: &GeriAlma) {
    v["geri_alinabilir"] = json!(g.karar);
    v["geri_alma_gerekce"] = json!(g.gerekce.join("; "));
    v["geri_alma_guveni"] = json!(if g.karar == "bilinmiyor" { "unknown" } else { "inferred" });
    v["geri_alma_konumu"] = json!(g.konum);
}

/// Güncellemede önceki değeri saklayan kayıt: zincirdeki sürüm / geçmiş yazımı ("dosya:satır", ifade) ve sürüm
/// tutan modeller (model anahtarı, kanıt, açıklama)
struct Gecmis<'a> {
    yazma: Option<&'a (String, String)>,
    modeller: &'a [(String, String, String)],
}

/// Ters yazma işlemi: aynı ön ekte `un`-li karşılık (`flag` ↔ `unflag`, `upvote` ↔ `unvote`, `follow` ↔ `unfollow`),
/// aynı yolda DELETE (`POST /x/:id/flag` ↔ `DELETE /x/:id/flag`) ya da yolun altında geri alma eylemi (`/undo`,
/// `/cancel`, `/revert`, `/geri-al`, `/iptal`). Aynı yoldaki DELETE yalnızca eklemenin / işaretlemenin tersidir ve
/// koleksiyonun kendisinde sayılmaz: güncellemenin (`PUT /people`, `PATCH /users/me`) tersi silme değildir
fn ters_yazma<'k>(k: &crate::adaptor::Kapi, kapilar: &'k [crate::adaptor::Kapi]) -> Option<&'k crate::adaptor::Kapi> {
    let yol = k.yol.trim_end_matches('/');
    let (on, son) = yol.rsplit_once('/')?;
    let son = son.to_ascii_lowercase();
    if son.is_empty() || parametre_mi(&son) {
        return None;
    }
    let ayni_yolda_silme = yazma_turu(k) == YazmaTuru::Ekleme && !koleksiyon_yolu(yol);
    let kok = son.trim_start_matches("up").trim_start_matches("down");
    let mut karsilik: Vec<String> = vec![format!("un{son}"), format!("un-{son}"), format!("un{kok}"), format!("cancel-{son}"), format!("remove-{son}")];
    // Oy ve işaretler tek bir geri alma eylemiyle kaldırılır (`unvote`)
    if matches!(kok, "vote" | "flag" | "like" | "star") {
        karsilik.push("unvote".into());
    }
    kapilar.iter().find(|t| {
        if t.id == k.id || t.yontem == "GET" || !t.ana {
            return false;
        }
        let ty = t.yol.trim_end_matches('/');
        if t.yontem == "DELETE" && ty == yol && k.yontem != "DELETE" && ayni_yolda_silme {
            return true;
        }
        if let Some((ton, tson)) = ty.rsplit_once('/') {
            let tson = tson.to_ascii_lowercase();
            if ton == on && karsilik.contains(&tson) {
                return true;
            }
            if ton == yol && matches!(tson.as_str(), "undo" | "cancel" | "revert" | "geri-al" | "iptal" | "rollback") {
                return true;
            }
        }
        false
    })
}

/// İşlemin kaynağı: yoldaki son anlamlı parça, tekil (`/api/pages/:id` -> "page")
fn kaynak_adi(yol: &str) -> Option<String> {
    yol.split('/').rfind(|p| p.len() > 2 && !p.starts_with(':') && !p.starts_with('{') && !matches!(*p, "api" | "v1" | "v2" | "v3")).map(model_anahtari)
}

/// Aynı kaynağın sürümüne geri döndürme ucu: yolunda sürüm / geçmiş sözcüğü (revisions, versions, history, revizyon,
/// surum) ile geri döndürme sözcüğü (restore, revert, rollback, geri-al) geçen ve sürüm sözcüğünden önceki kaynağı
/// işlemin kaynağı olan yazma (`/books/:b/page/:p/revisions/:r/restore` sayfanın sürümüdür, kitabın değil)
fn surum_geri_ucu<'k>(k: &crate::adaptor::Kapi, kapilar: &'k [crate::adaptor::Kapi]) -> Option<&'k crate::adaptor::Kapi> {
    let benim = kaynak_adi(&k.yol)?;
    kapilar.iter().find(|t| {
        let y = t.yol.to_ascii_lowercase();
        if t.yontem == "GET" || !["restore", "revert", "rollback", "geri-al", "geri-yukle"].iter().any(|s| y.contains(s)) {
            return false;
        }
        let Some(i) = ["/revisions", "/versions", "/history", "/revizyon", "/surum"].iter().find_map(|s| y.find(s)) else { return false };
        kaynak_adi(&y[..i]).is_some_and(|a| a == benim)
    })
}

/// Geri alınabilirlik kararı (bkz. modül açıklaması). Bütün kararlar çıkarımdır (en fazla `inferred`)
#[allow(clippy::too_many_arguments)]
fn geri_alma_karari(
    k: &crate::adaptor::Kapi,
    kapilar: &[crate::adaptor::Kapi],
    v: &Value,
    disler: &[Value],
    silmeler: &[(String, Silme)],
    yumusak_cagri: &[String],
    yumusak_modeller: &[(String, String, String)],
    genel_geri: Option<&crate::adaptor::Kapi>,
    sinir: Option<&(String, String)>,
    gecmis: &Gecmis,
) -> GeriAlma {
    let mut gerekce: Vec<String> = vec![];
    let mut konum: Option<String> = None;
    // Silinen / yazılan tablolar: zincirde ORM ile silinenler, silme işleminde veri bağının yazdıkları
    let mut tablolar: Vec<String> = v["silinen"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    if k.yontem == "DELETE" {
        tablolar.extend(k.veri.as_ref().map(|x| x.yazar.clone()).unwrap_or_default());
    }
    tablolar.extend(silmeler.iter().filter_map(|(_, s)| s.tablo.clone()));
    tablolar.sort();
    tablolar.dedup();
    let model_yumusak = |ad: &str| yumusak_modeller.iter().find(|(m, _, _)| *m == model_anahtari(ad));
    // Yumuşak silme işareti: alan yazması (davranış özeti), yumuşak silme çağrısı ya da silinen modelin model düzeyinde
    // yumuşak silmesi (silme çağrısının alıcısı da model adı olabilir: `$book->delete()`)
    let alan = v["yumusak_silme"].as_str().map(|y| (format!("yumuşak silme alanı yazılıyor ({y})"), y.to_string()));
    let cagri = yumusak_cagri.first().map(|c| (format!("yumuşak silme çağrısı: {c}"), c.rsplit_once('(').map(|x| x.1.trim_end_matches(')').to_string()).unwrap_or_default()));
    let zorla = silmeler.iter().find(|(_, s)| matches!(s.yontem.as_str(), "forceDelete" | "really_destroy!" | "Unscoped.Delete"));
    let model = tablolar
        .iter()
        .find_map(|t| model_yumusak(t))
        .or_else(|| silmeler.iter().find_map(|(_, s)| model_yumusak(&s.alici).filter(|_| s.yontem != "detach")))
        .filter(|_| zorla.is_none())
        .map(|(_, kn, ad)| (format!("model düzeyinde yumuşak silme: {ad}"), kn.clone()));
    let yumusak = alan.or(cagri).or(model);
    let kalici_tablolar: Vec<&String> = tablolar.iter().filter(|t| model_yumusak(t).is_none()).collect();
    // GORM `Delete` modeli yumuşak silen projede, modeli çözülemeyen silme kalıcı sayılmaz (bilinmiyor kalır)
    let kalici_cagri = zorla.or_else(|| {
        silmeler.iter().find(|(_, s)| {
            s.yontem != "detach"
                && model_yumusak(&s.alici).is_none()
                && s.tablo.as_deref().is_none_or(|t| model_yumusak(t).is_none())
                && !(s.yontem == "Delete" && s.tablo.is_none() && !yumusak_modeller.is_empty())
        })
    });
    let ters = crate::dna::geri_yukleme_ucu(k, kapilar);
    let dolayli: Vec<String> = v["dolayli_yazar"].as_array().into_iter().flatten().filter_map(|x| x["tablo"].as_str().map(String::from)).collect();
    let mut karar = if let Some((neden, kn)) = &yumusak {
        gerekce.push(neden.clone());
        konum = Some(kn.clone());
        match ters.or(genel_geri) {
            Some(t) => {
                gerekce.push(format!("geri yükleme ucu: {} {}", t.yontem, t.yol));
                "evet"
            }
            None => {
                gerekce.push("kayıt silinmez, işaretlenir; geri yükleme ucu bulunamadı (geri alma veride elle yapılır)".into());
                "kismen"
            }
        }
    } else if k.yontem == "DELETE" && (!kalici_tablolar.is_empty() || kalici_cagri.is_some()) {
        let ne = if let Some((kn, s)) = kalici_cagri {
            konum = Some(kn.clone());
            format!("kalıcı silme çağrısı: {}.{} ({kn})", s.alici, s.yontem)
        } else {
            format!("kalıcı silme: {}", kalici_tablolar.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))
        };
        gerekce.push(ne);
        if !dolayli.is_empty() {
            gerekce.push(format!("kaskadla dolaylı silinen: {}", dolayli.join(", ")));
        }
        match ters {
            Some(t) => {
                gerekce.push(format!("ama geri yükleme ucu var: {} {}", t.yontem, t.yol));
                "kismen"
            }
            None => {
                gerekce.push("yumuşak silme işareti ve geri yükleme ucu bulunamadı".into());
                "hayir"
            }
        }
    } else if k.yontem == "DELETE" {
        match ters {
            Some(t) => {
                gerekce.push(format!("geri yükleme ucu: {} {}", t.yontem, t.yol));
                konum = Some(format!("{}:{}", t.dosya, t.satir));
                "evet"
            }
            None => {
                gerekce.push("silmenin kalıcı mı işaretleme mi olduğu koddan çıkarılamadı".into());
                "bilinmiyor"
            }
        }
    } else {
        // Yazma: ters işlem (`flag` ↔ `unflag`, aynı yolda DELETE, `/cancel`, `/geri-al`), oluşturulan kaydı silen uç,
        // güncellemede önceki sürümü saklayan kayıt (revisions, PaperTrail, Envers, simple-history ...)
        let tablolar_yazilan: Vec<String> = k.veri.as_ref().map(|x| x.yazar.clone()).unwrap_or_default();
        let gecmis_model = tablolar_yazilan.iter().find_map(|t| gecmis.modeller.iter().find(|(m, _, _)| *m == model_anahtari(t)));
        let surum = gecmis
            .yazma
            .map(|(kn, ifade)| (format!("önceki sürüm kaydı yazılıyor: {ifade} ({kn})"), kn.clone()))
            .or_else(|| gecmis_model.filter(|_| matches!(k.yontem.as_str(), "PUT" | "PATCH")).map(|(_, kn, ad)| (format!("model önceki sürümleri saklıyor: {ad}"), kn.clone())));
        if let Some(t) = ters_yazma(k, kapilar) {
            gerekce.push(format!("ters işlem: {} {}", t.yontem, t.yol));
            konum = Some(format!("{}:{}", t.dosya, t.satir));
            "evet"
        } else if let (Some((neden, kn)), true) = (&surum, matches!(k.yontem.as_str(), "PUT" | "PATCH")) {
            gerekce.push(neden.clone());
            konum = Some(kn.clone());
            match surum_geri_ucu(k, kapilar) {
                Some(t) => {
                    gerekce.push(format!("sürüme geri döndürme ucu: {} {}", t.yontem, t.yol));
                    "evet"
                }
                None => {
                    gerekce.push("önceki değer saklanıyor ama geri döndürme ucu bulunamadı (geri alma veride elle yapılır)".into());
                    "kismen"
                }
            }
        } else if let Some(d) = (k.yontem == "POST" && yazma_turu(k) == YazmaTuru::Ekleme).then(|| silme_ucu(k, kapilar)).flatten() {
            gerekce.push(format!("oluşturulan kayıt şu işlemle silinebilir: {} {}", d.yontem, d.yol));
            konum = Some(format!("{}:{}", d.dosya, d.satir));
            "evet"
        } else if k.yontem == "POST" {
            gerekce.push("bu işlemin etkisini tersine çeviren bir işlem bulunamadı".into());
            "bilinmiyor"
        } else {
            gerekce.push("önceki değerlerin saklandığı bir sürüm / geçmiş kaydı ya da ters işlem bulunamadı".into());
            "bilinmiyor"
        }
    };
    // DELETE dışı yöntemle kalıcı silme (`POST /trash/clear`, `POST /purge`): zorla silme (`forceDelete`,
    // `really_destroy!`, `Unscoped().Delete`) ya da adı silme söyleyen işlemin zincirindeki kalıcı silme işlemin asıl
    // etkisidir; yöntemi ne olursa olsun geri alınamaz. Öteki işlemlerde (yakın halkada) kalıcı silme bir yan etkidir:
    // o kısım geri alınamaz
    if k.yontem != "DELETE" && yumusak.is_none() {
        if let Some((kn, s)) = kalici_cagri {
            if zorla.is_some() || silme_amacli(k) {
                gerekce.clear();
                gerekce.push(format!("kalıcı silme çağrısı: {}.{} ({kn})", s.alici, s.yontem));
                if !dolayli.is_empty() {
                    gerekce.push(format!("kaskadla dolaylı silinen: {}", dolayli.join(", ")));
                }
                gerekce.push("işlem yöntemi ne olursa olsun kayıtları kalıcı siliyor".into());
                konum = Some(kn.clone());
                karar = "hayir";
            } else {
                gerekce.push(format!("yan etki olarak kalıcı silme yapabilir: {}.{} ({kn})", s.alici, s.yontem));
                if !dolayli.is_empty() {
                    gerekce.push(format!("kaskadla dolaylı silinen: {}", dolayli.join(", ")));
                }
                if konum.is_none() {
                    konum = Some(kn.clone());
                }
                karar = "kismen";
            }
        }
    }
    // İki yollu silme: `force ? Deleted : Trashed`
    if karar == "evet" {
        if let Some(d) = v["dallar"].as_array().into_iter().flatten().find(|d| {
            let a = format!("{} {} {}", d["kosul"].as_str().unwrap_or(""), d["evet"].as_str().unwrap_or(""), d["hayir"].as_str().unwrap_or("")).to_lowercase();
            ["force", "permanent", "hard", "deleted", "kalici"].iter().any(|p| a.contains(p))
        }) {
            gerekce.push(format!("bir koşulda kalıcı olabilir: `{}` ({})", d["kosul"].as_str().unwrap_or(""), d["kanit"].as_str().unwrap_or("")));
            karar = "kismen";
        }
    }
    let dis_yazma: Vec<&Value> = disler.iter().filter(|d| d["yazma"] == true).collect();
    if !dis_yazma.is_empty() {
        let mut adlar: Vec<String> = dis_yazma.iter().map(|d| etki_adi(d)).collect();
        adlar.dedup();
        let fazla = adlar.len().saturating_sub(3);
        gerekce.push(format!(
            "dış etki geri alınamaz: {}{}",
            adlar.into_iter().take(3).collect::<Vec<_>>().join(", "),
            if fazla > 0 { format!(" +{fazla}") } else { String::new() }
        ));
        if konum.is_none() {
            konum = dis_yazma[0]["kanit"].as_str().map(String::from);
        }
        karar = match karar {
            "evet" | "kismen" => "kismen",
            _ => "hayir",
        };
    }
    if let Some((kn, ifade)) = sinir {
        gerekce.push(format!("işlem sınırı var ({ifade}, {kn}): hata olursa veritabanı yazmaları birlikte geri sarılır; tamamlanan işlemi geri almaz"));
    }
    // Kanıt satırı bulunamadıysa işlemin kendisi (karar onun gövdesinden çıkarıldı)
    let konum = konum.or_else(|| (karar != "bilinmiyor").then(|| format!("{}:{}", k.dosya, k.satir)));
    GeriAlma { karar, gerekce, konum }
}

/// İşlemin adı (yolun son anlamlı parçası ya da fonksiyonu) silme / boşaltma söylüyor mu: `clear`, `purge`, `empty`,
/// `truncate`, `wipe`, `destroy`, `delete`, `remove`, `forceDelete`, `hard-delete`, `temizle`, `sil`, `bosalt`
fn silme_amacli(k: &crate::adaptor::Kapi) -> bool {
    let ad = |x: &str| -> String { x.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect() };
    let parca = k.yol.rsplit('/').find(|p| !p.is_empty() && !p.starts_with(':') && !p.starts_with('{')).map(ad).unwrap_or_default();
    let fonk = ad(&k.fonksiyon);
    const SILME: [&str; 13] = ["clear", "purge", "empty", "truncate", "wipe", "destroy", "delete", "remove", "forcedelete", "harddelete", "temizle", "bosalt", "sil"];
    SILME.iter().any(|s| parca == *s || fonk.starts_with(s))
}

/// Onaysız gidemeyecek yazmanın gerekçesi: dış sisteme yazan, geri alınamaz ya da silen / yumuşak silen / kimlik bilgisi
/// değiştiren ama geri alınabilirliği kanıtlanmamış işlem. Okuma ve DELETE (zaten her seferinde onay ister) için None
fn onay_gerekcesi(k: &crate::adaptor::Kapi, v: &Value, g: &GeriAlma) -> Option<String> {
    if matches!(k.yontem.as_str(), "GET" | "HEAD" | "OPTIONS") {
        return None;
    }
    let dis: Vec<String> = v["dis"].as_array().into_iter().flatten().filter(|d| d["yazma"] == true).map(etki_adi).collect();
    if !dis.is_empty() {
        let mut d = dis.clone();
        d.dedup();
        return Some(format!("işlem uygulamanın dışına etki ediyor ({}): dış sistemdeki etki geri alınamaz", d.into_iter().take(3).collect::<Vec<_>>().join(", ")));
    }
    if g.karar == "hayir" {
        return Some(format!("işlem geri alınamaz ({})", g.gerekce.first().cloned().unwrap_or_default()));
    }
    let silen = !v["yumusak_silme"].is_null() || v["silinen"].as_array().is_some_and(|a| !a.is_empty()) || v["dolayli_yazar"].as_array().is_some_and(|a| !a.is_empty());
    if silen && g.karar != "evet" {
        return Some(format!("işlem kayıt siliyor ve geri alınabilirliği kanıtlanmadı (geri alma: {})", g.karar));
    }
    if kimlik_bilgisi_amacli(k) && g.karar != "evet" {
        return Some("işlem kimlik bilgisi değiştiriyor (parola, token, anahtar ya da iki adımlı doğrulama) ve geri alınabilirliği kanıtlanmadı".into());
    }
    None
}

/// İşlemin adı (yolun sabit parçaları ya da fonksiyonu) kimlik bilgisi söylüyor mu: parola, token, API anahtarı, gizli
/// anahtar, iki adımlı doğrulama. Kısa adlar (`2fa`, `mfa`, `otp`, `totp`) yalnızca tam parça ya da fonksiyon öneki
fn kimlik_bilgisi_amacli(k: &crate::adaptor::Kapi) -> bool {
    let ad = |x: &str| -> String { x.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect() };
    let parcalar: Vec<String> = k.yol.split('/').filter(|p| !p.is_empty() && !p.starts_with(':') && !p.starts_with('{')).map(ad).collect();
    let fonk = ad(&k.fonksiyon);
    const UZUN: [&str; 9] = ["password", "passwd", "passphrase", "parola", "sifre", "credential", "token", "apikey", "secret"];
    const KISA: [&str; 5] = ["2fa", "mfa", "otp", "totp", "twofactor"];
    UZUN.iter().any(|s| fonk.contains(s) || parcalar.iter().any(|p| p.contains(s))) || KISA.iter().any(|s| fonk.starts_with(s) || parcalar.iter().any(|p| p == s))
}

/// Haritanın kapılarına (JSON) onay gerekçesini `etki_onayi` olarak yazar: canlı modun onay kuralı kapıdan okur.
/// Anahtar "işlem-kimliği dosya:satır": aynı satırı paylaşan işlemlerin gerekçesi ayrı kalır
pub fn kapilara_yaz(endpointler: &mut Value, onaylar: &BTreeMap<String, String>) {
    for e in endpointler.as_array_mut().into_iter().flatten() {
        let anahtar = format!("{} {}:{}", e["id"].as_str().unwrap_or(""), e["dosya"].as_str().unwrap_or(""), e["satir"]);
        if let Some(o) = onaylar.get(&anahtar) {
            e["etki_onayi"] = json!(o);
        }
    }
}

/// Geri alınabilirliğin insana gösterilen tek satırı: karar, gerekçe ve damga (gerçek veride onay metni ve `tani`)
pub fn geri_metni(v: &Value, en: bool) -> String {
    let (karar, bilinmiyor) = match (v["geri_alinabilir"].as_str(), en) {
        (Some("evet"), false) => ("evet", false),
        (Some("evet"), true) => ("yes", false),
        (Some("kismen"), false) => ("kısmen", false),
        (Some("kismen"), true) => ("partly", false),
        (Some("hayir"), false) => ("hayır", false),
        (Some("hayir"), true) => ("no", false),
        (_, false) => ("bilinmiyor", true),
        (_, true) => ("unknown", true),
    };
    let gerekce = v["geri_alma_gerekce"].as_str().filter(|g| !g.is_empty()).map(|g| format!(" — {}", gerekce_cevir(g, en))).unwrap_or_default();
    let damga = match (bilinmiyor, en) {
        (true, _) => "",
        (false, false) => " [Yorum]",
        (false, true) => " [Inferred]",
    };
    format!("{karar}{gerekce}{damga}")
}

/// Haritaya Türkçe yazılan geri alma ve onay gerekçesinin (`geri_alma_gerekce`, `etki_onayi`) İngilizcesindeki kalıplar.
/// Uzun kalıp önce: biri ötekini içerebilir. Kalıplar arasındaki parçalar koddan gelen adlardır, çevrilmez
const GEREKCE_KALIPLARI: [(&str, &str); 43] = [
    (
        "işlem kimlik bilgisi değiştiriyor (parola, token, anahtar ya da iki adımlı doğrulama) ve geri alınabilirliği kanıtlanmadı",
        "the operation changes credentials (password, token, key or two-factor authentication) and its reversibility is not proven",
    ),
    // Çağrı grafiğiyle izlenen dış etkinin bilinmezliği (dis_zincir.rs)
    ("): dışarıya gidip gitmediği kanıtlanmadı", "): it is not proven whether it reaches outside the application"),
    ("işlemin dış etkisi bilinmiyor (", "the operation's external effect is unknown ("),
    ("sınıflandırılamayan kütüphane çağrısı: ", "unclassified library call: "),
    ("hedefi koddan çıkmayan dinamik çağrı: ", "dynamic call whose target the code does not show: "),
    ("aday sayısı sınırı aşan yöntem çağrısı: ", "method call with too many candidate targets: "),
    ("çağrı zinciri izleme sınırında kesildi", "the call chain was cut at the tracing limit"),
    (" silinince kaskad", ": cascade on delete"),
    (
        "kayıt silinmez, işaretlenir; geri yükleme ucu bulunamadı (geri alma veride elle yapılır)",
        "the record is flagged, not deleted; no restore endpoint was found (undo is done by hand in the data)",
    ),
    (
        "önceki değer saklanıyor ama geri döndürme ucu bulunamadı (geri alma veride elle yapılır)",
        "the previous value is kept but no revert endpoint was found (undo is done by hand in the data)",
    ),
    (
        "önceki değerlerin saklandığı bir sürüm / geçmiş kaydı ya da ters işlem bulunamadı",
        "no version or history record that keeps the previous values, and no reverse operation, was found",
    ),
    (
        "): hata olursa veritabanı yazmaları birlikte geri sarılır; tamamlanan işlemi geri almaz",
        "): on an error the database writes are rolled back together; it does not undo a completed operation",
    ),
    ("): hata olursa veritabanı yazmaları birlikte geri sarılır", "): on an error the database writes are rolled back together"),
    ("işlem kayıt siliyor ve geri alınabilirliği kanıtlanmadı (geri alma: kismen)", "the operation deletes records and its reversibility is not proven (undo: partly)"),
    ("işlem kayıt siliyor ve geri alınabilirliği kanıtlanmadı (geri alma: bilinmiyor)", "the operation deletes records and its reversibility is not proven (undo: unknown)"),
    ("işlem kayıt siliyor ve geri alınabilirliği kanıtlanmadı (geri alma: ", "the operation deletes records and its reversibility is not proven (undo: "),
    ("silmenin kalıcı mı işaretleme mi olduğu koddan çıkarılamadı", "the code does not show whether the delete is permanent or a flag"),
    ("bu işlemin etkisini tersine çeviren bir işlem bulunamadı", "no operation that reverses this operation's effect was found"),
    ("yumuşak silme işareti ve geri yükleme ucu bulunamadı", "no soft-delete marker and no restore endpoint were found"),
    ("): dış sistemdeki etki geri alınamaz", "): the effect in the external system cannot be undone"),
    ("işlem uygulamanın dışına etki ediyor (", "the operation affects something outside the application ("),
    ("oluşturulan kayıt şu işlemle silinebilir: ", "the created record can be deleted with "),
    ("yan etki olarak kalıcı silme yapabilir: ", "may delete permanently as a side effect: "),
    ("model düzeyinde yumuşak silme: ", "model-level soft delete: "),
    ("yumuşak silme alanı yazılıyor (", "a soft-delete field is written ("),
    ("model önceki sürümleri saklıyor: ", "the model keeps previous versions: "),
    ("önceki sürüm kaydı yazılıyor: ", "a previous-version record is written: "),
    ("sürüme geri döndürme ucu: ", "revert-to-version endpoint: "),
    ("ama geri yükleme ucu var: ", "but there is a restore endpoint: "),
    ("kaskadla dolaylı silinen: ", "deleted indirectly by cascade: "),
    ("bir koşulda kalıcı olabilir: ", "may be permanent under a condition: "),
    ("dış etki geri alınamaz: ", "the external effect cannot be undone: "),
    ("yumuşak silme çağrısı: ", "soft-delete call: "),
    ("işlem yöntemi ne olursa olsun kayıtları kalıcı siliyor", "the operation deletes records permanently, whatever its method"),
    ("kalıcı silme çağrısı: ", "permanent delete call: "),
    ("işlem geri alınamaz (", "the operation cannot be undone ("),
    ("geri yükleme ucu: ", "restore endpoint: "),
    ("işlem sınırı var (", "there is a transaction boundary ("),
    ("kalıcı silme: ", "permanent delete: "),
    ("ters işlem: ", "reverse operation: "),
    ("e-posta", "e-mail"),
    ("ödeme ", "payment "),
    ("kuyruk ", "queue "),
];

/// Haritadaki geri alma ya da onay gerekçesi seçilen dilde (gerekçe haritaya Türkçe yazılır)
pub fn gerekce_metni(g: &str) -> String {
    gerekce_cevir(g, crate::dil::en())
}

pub fn gerekce_cevir(g: &str, en: bool) -> String {
    if !en {
        return g.to_string();
    }
    let s = GEREKCE_KALIPLARI.iter().fold(g.to_string(), |s, (tr, ing)| s.replace(tr, ing));
    // Haritadaki gerekçe uzunluk sınırıyla kesilmiş olabilir: kalıba uymayan (Türkçe kalan) kesik parça atılır
    let turkce = |p: &str| p.chars().any(|c| "çğıöşüÇĞİÖŞÜ".contains(c));
    if !turkce(&s) {
        return s;
    }
    let mut v: Vec<&str> = vec![];
    for p in s.split("; ") {
        if turkce(p) {
            v.push("…");
            break;
        }
        v.push(p);
    }
    v.join("; ").replace("; …", " …")
}

/// Kartta dış etki türünün adı
pub fn tur_en(tur: &str) -> &'static str {
    match tur {
        "http" => "http",
        "kuyruk" => "queue",
        "eposta" => "email",
        "odeme" => "payment",
        "bulut" => "cloud",
        "bildirim" => "notification",
        _ => "other",
    }
}

/// `tani` için kısa özet satırları (dış etki, ön koşul sayımı, geri alma, işlem sınırı)
pub fn tani_satirlari(v: &Value, en: bool) -> Vec<String> {
    let d = |tr: &str, e: &str| if en { e.to_string() } else { tr.to_string() };
    let damga = |g: &str| match (g, en) {
        ("certain", false) => "[Kesin]",
        ("certain", true) => "[Certain]",
        ("inferred", false) => "[Yorum]",
        ("inferred", true) => "[Inferred]",
        (_, false) => "[Bilinmiyor]",
        (_, true) => "[Unknown]",
    };
    let mut s = vec![];
    let disler = v["dis"].as_array().cloned().unwrap_or_default();
    for x in disler.iter().take(6) {
        let zincir = x["zincir"]
            .as_array()
            .map(|z| z.iter().filter_map(|a| a.as_str()).collect::<Vec<_>>().join(" → "))
            .filter(|z| !z.is_empty())
            .map(|z| format!(" ← {z}"))
            .unwrap_or_default();
        let zaman = if x["zaman"] == "ertelenmis" { d(" (ertelenmiş)", " (deferred)") } else { String::new() };
        let kanal = x["kanal"].as_str().map(|k| format!(" [{k}]")).unwrap_or_default();
        s.push(format!(
            "  {}: {}{kanal}{zaman} · {} ({}{zincir}) {}",
            d("dış etki", "external effect"),
            etki_adi(x),
            x["istemci"].as_str().unwrap_or(""),
            x["kanit"].as_str().unwrap_or(""),
            damga(x["guven"].as_str().unwrap_or(""))
        ));
    }
    if disler.len() > 6 {
        s.push(format!("  … +{} {}", disler.len() - 6, d("dış etki daha", "more external effects")));
    }
    // Çağrı grafiğiyle izlenen dış etkinin durumu (bilinmiyorsa nedenleri)
    s.extend(crate::dis_zincir::tani_satirlari(v, en));
    let mut kodlar: BTreeMap<String, usize> = BTreeMap::new();
    for h in v["hatalar"].as_array().into_iter().flatten().filter(|h| h["kosul"].is_string() && h["kosul"] != "catch" && h["kosul"] != "except") {
        *kodlar.entry(h["durum"].as_u64().map(|c| c.to_string()).unwrap_or_else(|| "?".into())).or_default() += 1;
    }
    if !kodlar.is_empty() {
        let n: usize = kodlar.values().sum();
        let liste: Vec<String> = kodlar.iter().map(|(k, n)| format!("{k} ×{n}")).collect();
        s.push(format!(
            "  {}: {n} ({}) · {}",
            d("ön koşul (koşullu red)", "preconditions (conditional rejects)"),
            liste.join(", "),
            d("listelenmeyen 'yok' demek değildir", "unlisted does not mean absent")
        ));
    }
    if let Some(sn) = v["islem_siniri"].as_object() {
        s.push(format!("  {}: {} ({}) {}", d("işlem sınırı", "transaction boundary"), sn["ifade"].as_str().unwrap_or(""), sn["kanit"].as_str().unwrap_or(""), damga("inferred")));
    }
    s
}
