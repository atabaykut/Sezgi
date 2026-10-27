//! Ktor adaptörü (Kotlin): yönlendirme DSL'inden NestJS, FastAPI ve Spring adaptörleriyle aynı kapı yapısını (`Kapi`)
//! üretir; kart, MCP, önizleme, OpenAPI, canlı mod ve santral dilden bağımsız çalışır. Kaynaklar Kotlin okuyucusuyla
//! (`kotlin.rs`) okunur; DTO şemaları, servis zinciri, veri etkisi ve davranış özeti Spring adaptörünün dil bağımsız
//! çözümleyicisiyle (`spring::Baglam`) çıkar.
//! - **Rotalar (kesin):** `routing { }` (ve `install(Routing) { }`) kökünden `route("/x") { }`, HTTP fiilleri
//!   (`get("/{id}") { }`, `post { }`...), `route(yol, HttpMethod.X) { handle { } }` ve `method(HttpMethod.X) { }`
//!   iç içe birleşir. Rota fonksiyonları (`fun Route.gorevRotalari(...)`) çağrıldıkları yerin önekiyle açılır; yol
//!   sabitleri ve dizgi şablonları izlenir. Tip güvenli kaynaklar (`get<Makaleler> { }`, `@Resource("/x")`) desteklenir.
//!   `ktor.deployment.rootPath` (application.conf / .yaml) bütün yolların önündedir. `if` / `when` / döngü içindeki
//!   kayıt koşulludur; çözülemeyen yol kapı üretmez.
//! - **Girdiler:** `call.receive<Dto>()` (ya da tipi yazılan yerel) gövdedir; zorunluluk serileştiriciden:
//!   kotlinx.serialization'da varsayılansız alan zorunlu (`@SerialName` adı), Jackson'da boş olamayan ve varsayılansız
//!   alan. Yol parametresinin tipi dönüşümden (`toLong()`, `getOrFail<Long>`), `queryParameters["x"]` sorgudur.
//!   `receiveParameters()` form, `receiveMultipart()` dosya yüklemesidir (önizlemede denetlenemez). RequestValidation
//!   doğrulayıcısı uygulamanın kodudur: kapının notuna yazılır.
//! - **Giriş ve rol:** `authenticate(...) { }` içi giriş ister (kesin); `optional = true` herkese açıktır. Uygulamanın
//!   kendi sarmalayıcısı (`rolIle("ADMIN") { }`) gövdesiyle açılır (`build()` çağrısı içeriği yerleştirir); tanınmayan
//!   rota eklentisi (`install(X)`) ve tanınmayan sarmalayıcı "tanınmayan guard"dır, argümanlarındaki adlar rol tahminidir.
//!   Rotaya bağlı eklenti ya da sarmalayıcı yoksa kayıt tanımı kesindir: Ktor'da varsayılan kimlik denetimi yoktur.
//! - **Yanıt:** `call.respond(...)` yükünün tipi (kurucu çağrısı, tipi bilinen yerel, servis yönteminin dönüş tipi);
//!   `HttpStatusCode.Created` başarı kodu. `call.respond(HttpStatusCode.X)` ve `throw NotFoundException()` hata
//!   yanıtıdır; StatusPages `exception<X> { call.respond(HttpStatusCode.Y) }` eşlemesi uygulanır.
//! - **Veri:** Exposed tabloları ve DAO varlıkları (insert / update / delete / select), JPA / Spring Data depoları.
//! - **Kapsam dışı:** çalışma anında kurulan rotalar (veriden ya da yapılandırmadan döngüyle), uygulama düzeyi
//!   eklentilerin (`createApplicationPlugin`) iç mantığı.
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::adaptor::{Alan, Girdi, Kapi, Sema, Yanit};
use crate::dna::GovdeDna;
use crate::guvenli;
use crate::kotlin::{DslCagri, DslFonk, KtDosya, FIILLER};
use crate::spring::{self, Baglam, Cagri, Dosya, SRef, Serilestirici, YRef, Yontem};

pub const SURUM: &str = "0.1.0";

/// Ktor'un yerleşik özel durumları -> HTTP kodu (Ktor'un varsayılan hata yanıtı)
const YERLESIK: [(&str, u16); 7] = [
    ("NotFoundException", 404),
    ("BadRequestException", 400),
    ("MissingRequestParameterException", 400),
    ("ParameterConversionException", 400),
    ("CannotTransformContentToTypeException", 415),
    ("UnsupportedMediaTypeException", 415),
    ("PayloadTooLargeException", 413),
];
/// Rotada kurulduğunda kimlik ya da yetki denetimi yapmayan eklentiler
const SIRADAN_EKLENTILER: [&str; 16] = [
    "ContentNegotiation",
    "CORS",
    "CallLogging",
    "Compression",
    "DefaultHeaders",
    "StatusPages",
    "CachingHeaders",
    "ConditionalHeaders",
    "AutoHeadResponse",
    "PartialContent",
    "CallId",
    "RateLimit",
    "DoubleReceive",
    "RequestValidation",
    "Resources",
    "Routing",
];
/// Gövdesi rota ağacının parçası olan ve kimlik denetlemeyen DSL çağrıları
const SAYDAM: [&str; 13] = ["with", "apply", "run", "also", "let", "param", "optionalParam", "header", "accept", "contentType", "host", "port", "rateLimit"];
/// API olmayan kayıtlar (durağan dosyalar, WebSocket, SSE)
const API_DISI: [&str; 9] = ["static", "staticResources", "staticFiles", "staticRootFolder", "singlePageApplication", "webSocket", "webSocketRaw", "sse", "files"];

/// Ktor uygulaması mı: yapı dosyası (ya da Gradle sürüm kataloğu) Ktor sunucusunu kullanıyor
pub fn ktor_mi(kok: &Path) -> bool {
    ["build.gradle.kts", "build.gradle", "pom.xml", "gradle/libs.versions.toml"]
        .iter()
        .filter_map(|d| guvenli::oku(&kok.join(d)))
        .any(|m| m.contains("io.ktor") && (m.contains("ktor-server") || m.contains("io.ktor.plugin")))
}

/// Uygulamanın adı ve sürümü: Gradle (`rootProject.name`, `version = "..."`) ya da pom.xml
pub fn paket(kok: &Path) -> Value {
    spring::paket(kok)
}

/// `ktor.deployment.rootPath` (application.conf / application.yaml); ortam değişkeni yer tutucusu bilinmez
fn kok_yolu(ayarlar: &[String]) -> String {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r#"(?m)^\s*rootPath\s*[=:]\s*"?([^"\s#]+)"?"#).unwrap());
    ayarlar.iter().find_map(|m| re.captures(m).map(|c| c[1].to_string())).filter(|y| y.starts_with('/')).unwrap_or_default()
}

/// Ktor yolu Sezgi biçimine: `{id}`, `{id?}`, `{yol...}` -> `:id`
fn sezgi_yolu(y: &str) -> String {
    let y = y.replace("?}", "}").replace("...}", "}");
    spring::sezgi_yolu(&spring::duzgun(&y))
}

/// DSL'de bir yerdeki bağlam: önek, yöntem, giriş, tanınmayan guard'lar, rol tahmini, koşul, rota fonksiyonu
#[derive(Clone, Default)]
struct Ortam<'a> {
    onek: String,
    yontem: Option<String>,
    /// (giriş zorunlu mu, guard metni)
    giris: Option<(bool, String)>,
    guardlar: Vec<String>,
    bilinmeyen: bool,
    /// Sarmalayıcı argümanlarındaki adlar: tanınmayan guard varsa rol tahmini
    aday_roller: Vec<String>,
    kosullu: bool,
    /// En içteki rota fonksiyonunun düğümü
    router: String,
    /// Sarmalayıcının lambda parametresi -> (çağıranın dosyası, sınıfı, lambdasındaki DSL çağrıları)
    baglar: Vec<(&'a str, usize, usize, &'a [DslCagri])>,
}

/// Kaydedilen bir uç: yöntem, uygulama içi yol, işleyici, bağlam
struct Kayit<'a> {
    yontem: String,
    yol: String,
    isleyici: YRef,
    satir: usize,
    ortam: Ortam<'a>,
    /// Tip güvenli kaynak (`get<Makaleler.Id>`)
    kaynak: Option<SRef>,
}

struct Gezgin<'a> {
    b: &'a Baglam<'a>,
    dsl: &'a [Vec<DslFonk>],
    /// Ad -> (dosya, fonksiyon) listesi
    fonklar: HashMap<&'a str, Vec<(usize, usize)>>,
    kayitlar: Vec<Kayit<'a>>,
    routerlar: BTreeMap<String, Value>,
    /// Çözülemeyen yol, yöntem ya da derinlik sınırı: "dosya:satır metin"
    cozulemeyen: Vec<String>,
    yigin: Vec<(usize, usize)>,
}

impl<'a> Gezgin<'a> {
    fn router_id(&self, di: usize, fi: usize) -> String {
        format!("{}:{}", self.b.goreli(di), self.dsl[di][fi].ad)
    }

    /// Yol argümanı: ilk konumsal metin argümanı (sabitler ve şablonlar izlenir); argüman yoksa boş yol
    fn yol_argumani(&self, di: usize, si: usize, c: &DslCagri) -> Option<String> {
        match c.argumanlar.iter().find(|(a, _)| a.as_deref().is_none_or(|a| a == "path")) {
            None => Some(String::new()),
            Some((_, v)) if v.trim_start().starts_with("HttpMethod.") => Some(String::new()),
            Some((_, v)) => self.b.metin_degeri(di, Some((di, si)), v, 0),
        }
    }

    /// Tip güvenli kaynağın yolu: `@Resource("{id}")` ve kurucudaki üst kaynak (`val parent: Makaleler`)
    fn kaynak_yolu(&self, r: SRef, derinlik: usize) -> Option<String> {
        if derinlik > 6 {
            return None;
        }
        let s = self.b.sinif(r);
        let a = s.anotasyonlar.iter().find(|a| a.ad == "Resource")?;
        let kendi = a.deger(&["path", "value"]).and_then(|v| self.b.metin_degeri(r.0, Some(r), v, 0))?;
        let ust =
            s.alanlar.iter().filter(|f| f.kurucu).find_map(|f| self.b.bul(&f.tip, r.0).filter(|u| *u != r && self.b.sinif(*u).anotasyonlar.iter().any(|x| x.ad == "Resource")));
        Some(match ust.and_then(|u| self.kaynak_yolu(u, derinlik + 1)) {
            Some(u) => spring::yol_birlestir(&u, &kendi),
            None => kendi,
        })
    }

    /// Projedeki rota fonksiyonu: aynı ad, alıcısı Route / Routing (ya da alıcısız ama rota kuran), argüman sayısı uyar
    fn rota_fonksiyonu(&self, c: &DslCagri) -> Option<(usize, usize)> {
        if !(c.nesne.is_empty() || c.nesne == "this") {
            return None;
        }
        let verilen = c.argumanlar.len() + usize::from(c.lambda);
        self.fonklar.get(c.ad.as_str())?.iter().copied().find(|(di, fi)| {
            let f = &self.dsl[*di][*fi];
            let alici = matches!(f.alici.as_str(), "Route" | "Routing") || (f.alici.is_empty() && rota_kurar(&f.cagrilar));
            let zorunlu = self
                .b
                .sinif((*di, f.sinif))
                .yontemler
                .iter()
                .find(|y| y.ad == f.ad && y.satir == f.satir)
                .map(|y| y.parametreler.iter().filter(|p| p.deger.is_none()).count())
                .unwrap_or(0);
            alici && verilen >= zorunlu && verilen <= f.parametre_sayisi
        })
    }

    /// DSL çağrılarını bağlamla dolaşır. `di`, `si`: çağrıların bulunduğu dosya ve sınıf (yol sabitleri ve işleyiciler)
    fn gez(&mut self, di: usize, si: usize, cagrilar: &'a [DslCagri], ortam: &Ortam<'a>) {
        if self.yigin.len() > 12 {
            self.cozulemeyen.push(format!("{}: rota fonksiyonu zinciri çok derin", self.b.goreli(di)));
            return;
        }
        // Rotaya kurulan eklenti bütün kardeş kayıtları kapsar (sıradan bağımsız)
        let mut ortam = ortam.clone();
        for c in cagrilar.iter().filter(|c| c.ad == "install" && c.nesne.is_empty()) {
            let ad = c.argumanlar.first().map(|(_, v)| v.trim().to_string()).unwrap_or_default();
            if !ad.is_empty() && !SIRADAN_EKLENTILER.contains(&spring::temel_ad(&ad).as_str()) && ad != "Authentication" {
                ortam.guardlar.push(format!("install({ad})"));
                ortam.bilinmeyen = true;
            }
        }
        for c in cagrilar {
            let mut o = ortam.clone();
            o.kosullu |= c.kosullu;
            let fiil = FIILLER.iter().find(|(f, _)| *f == c.ad).map(|(_, y)| y.to_string());
            if let (Some(yi), true) = (c.isleyici, fiil.is_some() || c.ad == "handle") {
                let yontem = match fiil.or_else(|| o.yontem.clone()) {
                    Some(y) => y,
                    None => {
                        self.cozulemeyen.push(format!("{}:{} handle {{ }}: yöntem belirtilmemiş", self.b.goreli(di), c.satir));
                        continue;
                    }
                };
                // Tip güvenli kaynak: get<Makaleler> { }
                let kaynak = c.tip_argumanlari.first().and_then(|t| self.b.bul(t, di)).filter(|r| self.b.sinif(*r).anotasyonlar.iter().any(|a| a.ad == "Resource"));
                let yol = match kaynak {
                    Some(r) => self.kaynak_yolu(r, 0),
                    None => self.yol_argumani(di, si, c),
                };
                let Some(yol) = yol else {
                    self.cozulemeyen.push(format!("{}:{} {}: yol çözülemedi", self.b.goreli(di), c.satir, c.ad));
                    continue;
                };
                let yol = spring::yol_birlestir(&o.onek, &yol);
                self.kayitlar.push(Kayit { yontem, yol, isleyici: (di, si, yi), satir: c.satir, ortam: o, kaynak });
                continue;
            }
            match c.ad.as_str() {
                "route" => {
                    let Some(y) = self.yol_argumani(di, si, c) else {
                        self.cozulemeyen.push(format!("{}:{} route: yol çözülemedi", self.b.goreli(di), c.satir));
                        continue;
                    };
                    o.onek = spring::yol_birlestir(&o.onek, &y);
                    if let Some(m) = c.argumanlar.iter().find_map(|(_, v)| v.trim().strip_prefix("HttpMethod.")) {
                        o.yontem = Some(m.to_uppercase());
                    }
                    self.gez(di, si, &c.cocuklar, &o);
                }
                "method" => {
                    o.yontem = c.argumanlar.iter().find_map(|(_, v)| v.trim().strip_prefix("HttpMethod.")).map(|m| m.to_uppercase());
                    self.gez(di, si, &c.cocuklar, &o);
                }
                "authenticate" => {
                    let istege = c.argumanlar.iter().any(|(a, v)| (a.as_deref() == Some("optional") && v.trim() == "true") || v.contains("AuthenticationStrategy.Optional"));
                    let adlar: Vec<String> = c.argumanlar.iter().filter(|(a, _)| a.is_none()).filter_map(|(_, v)| spring::java_metni(v)).collect();
                    let mut metin = format!("authenticate({})", adlar.iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(", "));
                    if istege {
                        metin = metin.replacen(')', if adlar.is_empty() { "optional = true)" } else { ", optional = true)" }, 1);
                    }
                    // İç içe authenticate: en içteki geçerli
                    o.giris = Some((!istege, metin));
                    self.gez(di, si, &c.cocuklar, &o);
                }
                "routing" => self.gez(di, si, &c.cocuklar, &o),
                "install" if c.argumanlar.first().is_some_and(|(_, v)| v.trim() == "Routing") => self.gez(di, si, &c.cocuklar, &o),
                x if SAYDAM.contains(&x) => self.gez(di, si, &c.cocuklar, &o),
                x if API_DISI.contains(&x) || x == "install" => {}
                _ => {
                    // Sarmalayıcının lambda parametresi: çağıranın lambdasındaki kayıtlar buraya yerleşir
                    if c.nesne.is_empty() || c.nesne == "this" {
                        if let Some((_, bdi, bsi, cocuklar)) = o.baglar.iter().rev().find(|(ad, ..)| *ad == c.ad).copied() {
                            self.gez(bdi, bsi, cocuklar, &o);
                            continue;
                        }
                    }
                    if let Some((fdi, ffi)) = self.rota_fonksiyonu(c).filter(|r| !self.yigin.contains(r)) {
                        let dsl: &'a [Vec<DslFonk>] = self.dsl;
                        let f: &'a DslFonk = &dsl[fdi][ffi];
                        let id = self.router_id(fdi, ffi);
                        let icerme = format!("{}:{}", self.b.goreli(di), c.satir);
                        let ust = json!({ "id": o.router, "icerme": icerme, "kosul": o.kosullu.then_some("koşullu kayıt"), "kosullu": o.kosullu });
                        let dugum = self.routerlar.entry(id.clone()).or_insert_with(|| json!({ "id": id, "dosya": self.b.goreli(fdi), "satir": f.satir, "ustler": [] }));
                        if let Some(l) = dugum["ustler"].as_array_mut() {
                            if !l.contains(&ust) {
                                l.push(ust);
                            }
                        }
                        let mut o2 = o.clone();
                        o2.router = id;
                        if c.lambda {
                            // Sarmalayıcı: guard metni ve rol tahmini; lambda parametresine çağıranın içeriği bağlanır
                            o2.guardlar.push(cagri_metni(c));
                            o2.aday_roller.extend(c.argumanlar.iter().filter_map(|(_, v)| spring::java_metni(v)));
                            if let Some(p) = f.lambda_parametreleri.last() {
                                o2.baglar.push((p.as_str(), di, si, &c.cocuklar));
                            }
                        }
                        self.yigin.push((fdi, ffi));
                        self.gez(fdi, f.sinif, &f.cagrilar, &o2);
                        self.yigin.pop();
                        continue;
                    }
                    if c.lambda && !c.cocuklar.is_empty() && rota_kurar(&c.cocuklar) {
                        // Tanınmayan sarmalayıcı (kitaplık ya da çözülemeyen): içi rota, kendisi tanınmayan guard
                        o.guardlar.push(cagri_metni(c));
                        o.bilinmeyen = true;
                        o.aday_roller.extend(c.argumanlar.iter().filter_map(|(_, v)| spring::java_metni(v)));
                        self.gez(di, si, &c.cocuklar, &o);
                    }
                }
            }
        }
    }
}

/// Çağrının kısa metni: `rolIle("ADMIN")`
fn cagri_metni(c: &DslCagri) -> String {
    let args: Vec<String> = c.argumanlar.iter().map(|(a, v)| a.as_ref().map(|a| format!("{a} = {v}")).unwrap_or_else(|| v.clone())).collect();
    crate::dna::kisa(&format!("{}({})", c.ad, args.join(", ")))
}

/// Çağrı ağacında rota kaydı (HTTP fiili, route, authenticate) var mı
fn rota_kurar(cagrilar: &[DslCagri]) -> bool {
    cagrilar.iter().any(|c| c.isleyici.is_some() || matches!(c.ad.as_str(), "route" | "authenticate" | "method") || rota_kurar(&c.cocuklar))
}

/// Ağaçta adı verilen çağrılar (derinlemesine)
fn bul<'a>(cagrilar: &'a [DslCagri], ad: &str, out: &mut Vec<&'a DslCagri>) {
    for c in cagrilar {
        if c.ad == ad {
            out.push(c);
        }
        bul(&c.cocuklar, ad, out);
    }
}

/// Alıcı isteğin kendisi mi (`call`, `this.call`, `context`; denetleyicide `ApplicationCall` parametresi)
fn cagri_adi_mi(nesne: &str, adlar: &[String]) -> bool {
    let n = nesne.strip_prefix("this.").unwrap_or(nesne);
    adlar.iter().any(|a| a == n)
}

/// İşleyici ve isteği (`call`) argüman olarak alan proje yöntemleri, en çok iki kat: `kullanici.giris(call)` ->
/// `suspend fun giris(ctx: ApplicationCall)`. Her yöntemde isteğin adları
fn cagri_zinciri(b: &Baglam, r: YRef) -> Vec<(YRef, Vec<String>)> {
    let mut v: Vec<(YRef, Vec<String>, usize)> = vec![(r, vec!["call".into(), "context".into()], 0)];
    let mut i = 0;
    while i < v.len() && v.len() < 16 {
        let (x, adlar, derinlik) = v[i].clone();
        i += 1;
        if derinlik >= 2 {
            continue;
        }
        let y = b.yontem(x);
        for c in &y.cagrilar {
            let konumlar: Vec<usize> = c.argumanlar.iter().enumerate().filter(|(_, a)| cagri_adi_mi(a.trim(), &adlar)).map(|(j, _)| j).collect();
            if konumlar.is_empty() {
                continue;
            }
            for h in b.hedefler((x.0, x.1), y, c) {
                if v.iter().any(|(z, ..)| *z == h) {
                    continue;
                }
                let m = b.yontem(h);
                let yeni: Vec<String> = konumlar.iter().filter_map(|j| m.parametreler.get(*j).map(|p| p.ad.clone())).collect();
                if !yeni.is_empty() {
                    v.push((h, yeni, derinlik + 1));
                }
            }
        }
    }
    v.into_iter().map(|(x, a, _)| (x, a)).collect()
}

/// Dönüşüm fonksiyonundan tip: `toLong()` -> Long
fn donusum_tipi(f: &str) -> Option<&'static str> {
    Some(match f.trim_end_matches("OrNull") {
        "toLong" => "Long",
        "toInt" => "Integer",
        "toShort" => "Short",
        "toDouble" => "Double",
        "toFloat" => "Float",
        "toBoolean" | "toBooleanStrict" => "Boolean",
        "toUUID" | "toUuid" => "UUID",
        "toBigDecimal" => "BigDecimal",
        _ => return None,
    })
}

/// İşleyicideki parametre okumaları: ad -> (tip, zorunlu mu, sorgu mu). `call.parameters` yol ve sorgu
/// parametrelerini birlikte taşır; `queryParameters` yalnızca sorgu. `getOrFail` eksikse 400 verir
fn parametre_okumalari(y: &Yontem) -> BTreeMap<String, (Option<String>, bool, bool)> {
    let mut m: BTreeMap<String, (Option<String>, bool, bool)> = BTreeMap::new();
    for c in &y.cagrilar {
        let (ad, tip, zorunlu) = match c.ad.as_str() {
            "[]" if c.nesne.ends_with("parameters") || c.nesne.ends_with("Parameters") => {
                (c.argumanlar.first().and_then(|a| spring::java_metni(a)), c.argumanlar.get(1).and_then(|f| donusum_tipi(f)).map(String::from), false)
            }
            "getOrFail" if c.nesne.ends_with("parameters") || c.nesne.ends_with("Parameters") => {
                (c.argumanlar.first().and_then(|a| spring::java_metni(a)), c.tip_argumanlari.first().cloned(), true)
            }
            _ => continue,
        };
        let Some(ad) = ad else { continue };
        let sorgu = c.nesne.ends_with("queryParameters");
        let e = m.entry(ad).or_insert((None, false, sorgu));
        if e.0.is_none() {
            e.0 = tip;
        }
        e.1 |= zorunlu;
    }
    m
}

impl<'a> Gezgin<'a> {
    /// Yanıt yükünün tipi: (tip, kesin mi). Kurucu çağrısı ve tipi bilinen yerel kesin; servis yönteminin dönüş tipi
    /// zincirden çıkarımdır
    fn yuk_tipi(&self, r: YRef, ifade: &str) -> Option<(String, bool)> {
        static D: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
        let [kurucu, liste, cagri] = D.get_or_init(|| {
            [
                regex::Regex::new(r"^([A-Z]\w*)\s*\(").unwrap(),
                regex::Regex::new(r"^(?:listOf|setOf|mutableListOf|arrayListOf)\s*\(\s*([A-Z]\w*)\s*\(").unwrap(),
                regex::Regex::new(r"^([a-z_]\w*(?:\.\w+)*)\.(\w+)\s*\(").unwrap(),
            ]
        });
        let y = self.b.yontem(r);
        let e = ifade.trim();
        if let Some(c) = kurucu.captures(e) {
            return Some((c[1].to_string(), true));
        }
        if let Some(c) = liste.captures(e) {
            return Some((format!("List<{}>", &c[1]), true));
        }
        if e.chars().all(|c| c.is_alphanumeric() || c == '_') {
            if let Some(t) = y.yereller.get(e) {
                return Some((t.clone(), true));
            }
            let (nesne, ad) = y.kaynak_cagrilari.get(e)?;
            return self.donus_tipi(r, nesne, ad).map(|t| (t, false));
        }
        let c = cagri.captures(e)?;
        self.donus_tipi(r, &c[1], &c[2]).map(|t| (t, false))
    }

    /// İşleyicideki çağrının hedef yönteminin dönüş tipi
    fn donus_tipi(&self, r: YRef, nesne: &str, ad: &str) -> Option<String> {
        let y = self.b.yontem(r);
        let c = y.cagrilar.iter().find(|c| c.nesne == nesne && c.ad == ad).cloned().unwrap_or_else(|| Cagri { nesne: nesne.into(), ad: ad.into(), ..Default::default() });
        self.b.hedefler((r.0, r.1), y, &c).into_iter().map(|h| self.b.yontem(h).donus.clone()).find(|t| !t.is_empty() && t != "void")
    }
}

pub fn kapilar(kok: &Path, dosyalar: &[Dosya], dsl: &[Vec<DslFonk>], ayarlar: &[String], yapi_metinleri: &[String]) -> (Vec<Kapi>, Value) {
    let mut b = Baglam::yeni(kok, dosyalar, false, false);
    let metin = |x: &str| yapi_metinleri.iter().any(|m| m.contains(x));
    let acik_bos = !dosyalar.iter().flat_map(|d| &d.siniflar).flat_map(|s| &s.yontemler).any(|y| y.govde_metni.replace(' ', "").contains("explicitNulls=false"));
    b.serilestirici = if metin("serialization-kotlinx") || metin("kotlinx-serialization") {
        Serilestirici::Kotlinx { acik_bos }
    } else if metin("serialization-jackson") {
        Serilestirici::Jackson { kotlin_modulu: true }
    } else {
        Serilestirici::Bilinmeyen
    };
    // Özel durum eşlemesi: Ktor'un yerleşikleri, sonra StatusPages `exception<X> { call.respond(HttpStatusCode.Y) }`
    for (ad, kod) in YERLESIK {
        b.istisnalar.entry(ad.into()).or_insert(kod);
    }
    for f in dsl.iter().flatten() {
        let mut v = vec![];
        bul(&f.cagrilar, "exception", &mut v);
        for c in v {
            let mut yanitlar = vec![];
            bul(&c.cocuklar, "respond", &mut yanitlar);
            bul(&c.cocuklar, "respondText", &mut yanitlar);
            let kod = yanitlar.iter().flat_map(|r| r.argumanlar.iter()).find_map(|(_, v)| crate::kotlin::kod(v));
            if let (Some(t), Some(kod)) = (c.tip_argumanlari.first(), kod) {
                b.istisnalar.insert(spring::temel_ad(t), kod);
            }
        }
    }
    let b = b;
    // RequestValidation: `validate<Dto> { }` doğrulayıcıları
    let mut dogrulananlar: HashSet<String> = HashSet::new();
    for f in dsl.iter().flatten() {
        let mut v = vec![];
        bul(&f.cagrilar, "validate", &mut v);
        dogrulananlar.extend(v.iter().filter_map(|c| c.tip_argumanlari.first().map(|t| spring::temel_ad(t))));
    }
    let genel_onek = {
        let o = spring::duzgun(&kok_yolu(ayarlar));
        if o == "/" {
            String::new()
        } else {
            o
        }
    };
    let mut fonklar: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for (di, l) in dsl.iter().enumerate() {
        for (fi, f) in l.iter().enumerate() {
            fonklar.entry(f.ad.as_str()).or_default().push((di, fi));
        }
    }
    let mut g = Gezgin { b: &b, dsl, fonklar, kayitlar: vec![], routerlar: BTreeMap::new(), cozulemeyen: vec![], yigin: vec![] };
    // Kökler: `routing { }` ve `install(Routing) { }`
    let mut genel_guardlar = vec![];
    for (di, l) in dsl.iter().enumerate() {
        for (fi, f) in l.iter().enumerate() {
            let mut kokler = vec![];
            bul(&f.cagrilar, "routing", &mut kokler);
            let mut kurulumlar = vec![];
            bul(&f.cagrilar, "install", &mut kurulumlar);
            kokler.extend(kurulumlar.iter().filter(|c| c.argumanlar.first().is_some_and(|(_, v)| v.trim() == "Routing")));
            // Kimlik sağlayıcıları: install(Authentication) { jwt("ad") { } }
            for k in kurulumlar.iter().filter(|c| c.argumanlar.first().is_some_and(|(_, v)| v.trim() == "Authentication")) {
                for s in &k.cocuklar {
                    let ad = s.argumanlar.first().and_then(|(_, v)| spring::java_metni(v)).unwrap_or_else(|| "auth".into());
                    genel_guardlar.push(format!("Authentication {}({ad})", s.ad));
                }
            }
            if kokler.is_empty() {
                continue;
            }
            let id = g.router_id(di, fi);
            g.routerlar.entry(id.clone()).or_insert_with(|| json!({ "id": id, "dosya": b.goreli(di), "satir": f.satir, "ustler": [] }));
            let o = Ortam { router: id, ..Default::default() };
            g.yigin.push((di, fi));
            for k in kokler {
                g.gez(di, f.sinif, &k.cocuklar, &o);
            }
            g.yigin.pop();
        }
    }
    // Modeller: JPA varlıkları, Exposed tabloları ve depoların yönettiği tipler
    let modeller: HashSet<String> = dosyalar.iter().flat_map(|d| &d.siniflar).filter(|s| spring::varlik_mi(s)).map(|s| s.ad.clone()).collect();
    let mut zengin: HashMap<YRef, GovdeDna> = HashMap::new();
    for (di, d) in dosyalar.iter().enumerate() {
        for (si, s) in d.siniflar.iter().enumerate() {
            for (yi, y) in s.yontemler.iter().enumerate() {
                if y.govdeli {
                    zengin.insert((di, si, yi), b.zengin_dna((di, si, yi)));
                }
            }
        }
    }
    let mut kapilar: Vec<Kapi> = vec![];
    let mut gorulen: HashSet<String> = HashSet::new();
    let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
    let mut kosullu_kapilar = vec![];
    let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
    for k in &g.kayitlar {
        let tam = if genel_onek.is_empty() { spring::duzgun(&k.yol) } else { spring::duzgun(&spring::yol_birlestir(&genel_onek, &k.yol)) };
        let yol = sezgi_yolu(&tam);
        let id = format!("{} {yol}", k.yontem);
        if !gorulen.insert(id.clone()) {
            continue;
        }
        let (di, _, _) = k.isleyici;
        let y = b.yontem(k.isleyici);
        let konum = format!("{}:{}", b.goreli(di), k.satir);
        let mut notlar: Vec<String> = vec![];
        if k.yol.contains("?}") {
            notlar.push("yolda isteğe bağlı parametre var ({ad?}): parametresiz yol da bu işleyiciye gelir".into());
        }
        // İşleyici ve `call`u alan proje yöntemleri (denetleyici deseni): girdiler ve yanıt bunlardan okunur
        let zincir = cagri_zinciri(&b, k.isleyici);
        // Parametreler: yoldaki adlar; tip dönüşümden ya da tip güvenli kaynağın alanından
        let mut okumalar = BTreeMap::new();
        for (x, _) in zincir.iter().rev() {
            okumalar.extend(parametre_okumalari(b.yontem(*x)));
        }
        let yol_adlari: Vec<String> = yol.split('/').filter_map(|p| p.strip_prefix(':').map(String::from)).collect();
        let kaynak_alani = |ad: &str| k.kaynak.and_then(|r| b.sinif(r).alanlar.iter().find(|f| f.ad == ad).map(|f| f.tip.clone()));
        let parametreler: Vec<Girdi> = yol_adlari
            .iter()
            .map(|a| Girdi { ad: Some(a.clone()), tip: Some(kaynak_alani(a).or_else(|| okumalar.get(a).and_then(|o| o.0.clone())).unwrap_or_else(|| "String".into())), sema: None })
            .collect();
        let mut sorgu_alanlari: BTreeMap<String, Alan> = BTreeMap::new();
        for (ad, (tip, zorunlu, _)) in okumalar.iter().filter(|(a, _)| !yol_adlari.contains(a)) {
            let mut alan = b.alan(di, tip.as_deref().unwrap_or("String"), &[], y.satir, false, 0);
            alan.zorunlu = *zorunlu;
            sorgu_alanlari.insert(ad.clone(), alan);
        }
        // Tip güvenli kaynağın yolda olmayan alanları sorgudur
        if let Some(r) = k.kaynak {
            for f in b
                .sinif(r)
                .alanlar
                .iter()
                .filter(|f| f.kurucu && !yol_adlari.contains(&f.ad) && !b.bul(&f.tip, r.0).is_some_and(|u| b.sinif(u).anotasyonlar.iter().any(|a| a.ad == "Resource")))
            {
                let mut alan = b.alan(r.0, &f.tip, &[], f.satir, false, 0);
                alan.zorunlu = f.deger.is_none() && !f.bos_olabilir;
                sorgu_alanlari.insert(f.ad.clone(), alan);
            }
        }
        // Gövde
        let mut govde: Vec<Girdi> = vec![];
        for (x, adlar) in &zincir {
            let yx = b.yontem(*x);
            for c in yx.cagrilar.iter().filter(|c| cagri_adi_mi(&c.nesne, adlar)) {
                match c.ad.as_str() {
                    "receive" | "receiveNullable" | "receiveOrNull" => {
                        let Some(t) = c.tip_argumanlari.first().cloned().or_else(|| c.argumanlar.first().and_then(|a| a.strip_suffix("::class").map(String::from))) else {
                            notlar.push("gövde tipi okunamadı: call.receive() tipi koddan çıkarılamıyor".into());
                            continue;
                        };
                        let (ic, dizi) = spring::sarmalayicisiz(&t);
                        match b.bul(&ic, di).filter(|r| matches!(b.sinif(*r).tur, "class" | "record")) {
                            Some(r) => {
                                govde.push(Girdi { ad: None, tip: Some(b.sinif(r).ad.clone()), sema: Some(b.sema(r, false, dizi, true)) });
                                if dogrulananlar.contains(&b.sinif(r).ad) {
                                    notlar.push(format!("RequestValidation: {} için validate {{ }} doğrulayıcısı uygulamanın kodu; önizlemede sınanamaz", b.sinif(r).ad));
                                }
                                if b.serilestirici == Serilestirici::Bilinmeyen {
                                    notlar.push("JSON serileştiricisi tanınmadı (kotlinx.serialization ya da Jackson değil): alanların zorunluluğu okunmadı".into());
                                }
                            }
                            None => govde.push(Girdi { ad: None, tip: Some(t), sema: None }),
                        }
                    }
                    "receiveParameters" => {
                        let yerel = yx.kaynak_cagrilari.iter().find(|(_, (n, a))| cagri_adi_mi(n, adlar) && a == "receiveParameters").map(|(ad, _)| ad.clone());
                        let mut alanlar = BTreeMap::new();
                        for x in yx.cagrilar.iter().filter(|x| x.ad == "[]" && yerel.as_deref() == Some(x.nesne.as_str())) {
                            if let Some(ad) = x.argumanlar.first().and_then(|a| spring::java_metni(a)) {
                                alanlar.insert(ad, b.alan(di, x.argumanlar.get(1).and_then(|f| donusum_tipi(f)).unwrap_or("String"), &[], x.satir, false, 0));
                            }
                        }
                        govde.push(Girdi {
                            ad: None,
                            tip: Some("form".into()),
                            sema: Some(Sema {
                                adaylar: vec![],
                                dizi: false,
                                dto: "form".into(),
                                dosya: Some(b.goreli(di)),
                                bulunamadi: alanlar.is_empty(),
                                kalitim: None,
                                alanlar,
                            }),
                        });
                    }
                    "receiveMultipart" => {
                        notlar.push("dosya doğrulaması kontrol edilemedi: receiveMultipart(); dosyanın varlığı, türü ve içeriği önizlemede sınanmaz".into());
                        govde.push(Girdi { ad: None, tip: Some("multipart".into()), sema: None });
                    }
                    "receiveText" | "receiveChannel" | "receiveStream" => govde.push(Girdi { ad: None, tip: Some("text".into()), sema: None }),
                    _ => {}
                }
            }
        }
        let sorgu = if sorgu_alanlari.is_empty() {
            vec![]
        } else {
            vec![Girdi {
                ad: None,
                tip: None,
                sema: Some(Sema { adaylar: vec![], dizi: false, dto: "query".into(), dosya: Some(b.goreli(di)), bulunamadi: false, kalitim: None, alanlar: sorgu_alanlari }),
            }]
        };
        // Yanıt: başarılı respond çağrısının yükü
        let mut yanit = None;
        let yanitlar = zincir.iter().flat_map(|(x, adlar)| b.yontem(*x).cagrilar.iter().filter(move |c| cagri_adi_mi(&c.nesne, adlar)).map(move |c| (*x, c)));
        for (x, c) in yanitlar.filter(|(_, c)| matches!(c.ad.as_str(), "respond" | "respondNullable")) {
            let durum = c.argumanlar.first().and_then(|a| crate::kotlin::kod(a));
            if durum.is_some_and(|d| d >= 300) || (durum.is_some() && c.argumanlar.len() < 2) {
                continue;
            }
            let Some((t, kesin)) = c.argumanlar.last().and_then(|a| g.yuk_tipi(x, a)) else { continue };
            let (ic, dizi) = spring::sarmalayicisiz(&t);
            if let Some(r) = b.bul(&ic, di).filter(|r| matches!(b.sinif(*r).tur, "class" | "record")) {
                yanit = Some(Yanit { kaynak: "call.respond".into(), kesin, durum, sema: b.sema(r, false, dizi, false) });
                break;
            }
        }
        let veri = b.veri_bagi(k.isleyici, &modeller);
        if let std::collections::btree_map::Entry::Vacant(e) = dna_haritasi.entry(konum.clone()) {
            let v = b.dna_zinciri(k.isleyici, &zengin);
            if !crate::dna::bos(&v) {
                e.insert(v);
            }
        }
        // Giriş kararı
        let o = &k.ortam;
        let (mut giris, mut kanit) = match &o.giris {
            Some((true, _)) => ("gerekli", "kesin"),
            _ => ("herkese_acik", "kesin"),
        };
        let mut guardlar: Vec<String> = o.giris.iter().map(|(_, m)| m.clone()).collect();
        guardlar.extend(o.guardlar.iter().cloned());
        let (mut roller, mut roller_kaniti) = (vec![], None);
        if o.bilinmeyen {
            if giris == "herkese_acik" {
                kanit = "varsayim_bilinmeyen_guard";
            }
            if !o.aday_roller.is_empty() {
                roller = o.aday_roller.clone();
                roller_kaniti = Some("varsayim".to_string());
                giris = "gerekli";
            }
        }
        if o.kosullu {
            kosullu_kapilar.push(id.clone());
        }
        if !notlar.is_empty() {
            denetlenemeyen.entry(konum.clone()).or_default().extend(notlar);
        }
        kapi_routerlari.insert(konum.clone(), o.router.clone());
        let ilk =
            k.yol.split('/').find(|p| !p.is_empty() && !p.starts_with('{') && *p != "api" && !(p.starts_with('v') && p[1..].chars().all(|c| c.is_ascii_digit()) && p.len() > 1));
        kapilar.push(Kapi {
            id,
            yontem: k.yontem.clone(),
            yol,
            kontrolcu: ilk.map(|p| p.chars().filter(|c| c.is_alphanumeric()).collect()).unwrap_or_else(|| "api".into()),
            fonksiyon: k.yontem.to_lowercase(),
            dosya: b.goreli(di),
            satir: k.satir,
            giris: giris.into(),
            guardlar,
            roller,
            roller_kaniti,
            ozet: None,
            govde,
            sorgu,
            parametreler,
            kullanimdan_kalkmis: false,
            yerine: None,
            surum: None,
            uygulama: None,
            ana: true,
            giris_kaniti: kanit.into(),
            veri,
            yanit,
        });
    }
    kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
    let ana = dosyalar.iter().enumerate().find_map(|(di, d)| d.siniflar.first().filter(|s| s.yontemler.iter().any(|y| y.ad == "main")).map(|s| (di, s)));
    (
        kapilar,
        json!({
            "genel_onek": genel_onek, "genel_guardlar": genel_guardlar,
            "ana_uygulama": ana.map(|(di, s)| format!("{}.{}", dosyalar[di].paket, s.ad).trim_start_matches('.').to_string()),
            "kosullu_kapilar": kosullu_kapilar, "denetlenemeyen": denetlenemeyen, "dna": dna_haritasi,
            "kaskadlar": spring::kaskadlar(&b), "olaylar": spring::olaylar(&b, &zengin),
            "routerlar": g.routerlar.into_values().collect::<Vec<_>>(), "kapi_routerlari": kapi_routerlari,
            "veri_modelleri": spring::veri_modelleri(&b), "islem_kimlikleri": {},
            "dinamik_kayitlar": g.cozulemeyen,
        }),
    )
}

/// Keşif, paralel ayrıştırma ve Ktor adaptörü. Test kaynakları ve derleme çıktıları okunmaz
pub(crate) fn topla(kok: &Path, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<KtDosya>>, t0: Instant) -> crate::Toplam {
    let mut yollar = vec![];
    let (mut atlanan, mut buyuk) = (0usize, vec![]);
    let (mut ayar_dosyalari, mut yapi_dosyalari): (Vec<PathBuf>, Vec<PathBuf>) = (vec![], vec![]);
    for g in ignore::WalkBuilder::new(kok).hidden(true).git_ignore(true).build().flatten() {
        let p = g.path();
        if g.path_is_symlink() || !p.is_file() {
            if g.path_is_symlink() {
                atlanan += 1;
            }
            continue;
        }
        let goreli = p.strip_prefix(kok).unwrap_or(p);
        if goreli.components().any(|c| {
            ["target", "build", "out", ".gradle", "node_modules", "test", "tests", "testFixtures", "integrationTest", "androidTest"].contains(&c.as_os_str().to_str().unwrap_or(""))
        }) {
            atlanan += 1;
            continue;
        }
        let ad = p.file_name().and_then(|a| a.to_str()).unwrap_or("");
        if YAPILANDIRMA_DOSYALARI.contains(&ad) {
            if ad.starts_with("application.") {
                ayar_dosyalari.push(p.to_path_buf());
            } else {
                yapi_dosyalari.push(p.to_path_buf());
            }
            continue;
        }
        if p.extension().is_some_and(|e| e == "kt") {
            if g.metadata().is_ok_and(|m| m.len() > crate::EN_BUYUK_DOSYA) {
                buyuk.push(guvenli::goreli(kok, p));
                continue;
            }
            yollar.push(p.to_path_buf());
        } else {
            atlanan += 1;
        }
    }
    let surum_katalogu = kok.join("gradle").join("libs.versions.toml");
    if surum_katalogu.is_file() {
        yapi_dosyalari.push(surum_katalogu);
    }
    let t_kesif = t0.elapsed();
    let (sonuclar, mut okunan, ayristirilan) = crate::kesif::ayristir_hepsi(&yollar, onbellek, |p, b| crate::kotlin::ayristir_metin(p, std::str::from_utf8(&b).ok()?, true));
    let (dosyalar, dsl): (Vec<Dosya>, Vec<Vec<DslFonk>>) = sonuclar.into_iter().map(|(_, d)| (d.model, d.dsl)).unzip();
    let t_ayristirma = t0.elapsed();
    ayar_dosyalari.sort_by_key(|p| (p.components().count(), p.clone()));
    let mut oku = |p: &PathBuf| -> Option<String> {
        let m = guvenli::oku(p)?;
        okunan.push((p.clone(), crate::kesif::icerik_ozeti(m.as_bytes())));
        Some(m)
    };
    let ayarlar: Vec<String> = ayar_dosyalari.iter().filter_map(&mut oku).collect();
    let yapi_metinleri: Vec<String> = yapi_dosyalari.iter().filter_map(&mut oku).collect();
    let (kapilar, bilgi) = kapilar(kok, &dosyalar, &dsl, &ayarlar, &yapi_metinleri);
    let t_toplam = t0.elapsed();
    crate::Toplam {
        kapilar,
        bilgi,
        adaptor: format!("ktor@{SURUM}"),
        dosya_sayisi: dosyalar.len(),
        satir: dosyalar.iter().map(|d| d.satir_sayisi).sum(),
        kismi: dosyalar.iter().filter(|d| !d.hatali_satirlar.is_empty()).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        atlanan,
        buyuk,
        ek_paketler: vec![],
        veri_dosyalari: dosyalar.iter().filter(|d| d.siniflar.iter().any(spring::varlik_mi)).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}

/// Haritayı etkileyen Ktor yapılandırma dosyaları (izleme ve bayatlık); Kotlin kaynakları `spring::JAVA_UZANTILARI`'nda
pub(crate) const YAPILANDIRMA_DOSYALARI: &[&str] =
    &["build.gradle.kts", "build.gradle", "settings.gradle.kts", "settings.gradle", "pom.xml", "application.conf", "application.yaml", "application.yml"];
