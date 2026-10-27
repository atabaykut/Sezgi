//! Go HTTP adaptörü: net/http (`ServeMux`, Go 1.22 yöntem + desen rotaları), Gin, Echo, Chi, Fiber ve gorilla/mux.
//! Go modelinden (`golang.rs`) NestJS ve FastAPI adaptörleriyle aynı kapı yapısını (`Kapi`) üretir; kart, MCP, önizleme,
//! OpenAPI, kanıt grafı ve canlı mod dilden bağımsız çalışır.
//!
//! Neyin nereden okunduğu:
//! - **Rotalar (kesin):** rota kuran kod sırasıyla yorumlanır: `main` ve `init` fonksiyonlarından başlayıp çağrılan
//!   proje fonksiyonlarına (router'ı argüman alan `routes.Kaydet(api)`, router döndüren `NewRouter()`, alanında router
//!   tutan yapının metotları `s.routes()`) inilir. Gruplar ve alt yönlendiriciler (`Group`, `Route`, `With`, `Mount`,
//!   `PathPrefix(...).Subrouter()`, `http.StripPrefix`) tam yolu kurar. Yolu çözülemeyen kayıt (çalışma anında
//!   kurulan metin) uydurulmaz: dinamik kayıt olarak sayılır. Hiçbir uygulamaya bağlanmamış router'ın (yalnızca router
//!   parametresi alan fonksiyon) rotaları ikincildir, önekleri bilinmez.
//! - **Giriş ve rol:** zincirdeki ara katmanlar (uygulama, grup ve rota düzeyi; çatının sıra kuralıyla: Gin ve Echo
//!   grupları kayıt anındaki ara katmanları, Fiber kayıt sırasını, Chi / gorilla / net/http sarmalaması hepsini alır).
//!   Tanınan kitaplık doğrulayıcıları (echo-jwt, Echo `middleware.JWT` / `KeyAuth` / `BasicAuth`, Gin `BasicAuth`,
//!   gin-jwt, go-chi/jwtauth `Authenticator`, Fiber jwtware / basicauth / keyauth) kesin karardır. Projedeki ara katman
//!   tanımından okunur: 401 / 403 ile reddediyorsa giriş gerekli (çıkarım), belirteci okuyup bağlama yazıyor ama
//!   reddetmiyorsa isteğe bağlı. Tanımı izlenemeyen ara katman "tanınmayan guard" olarak bildirilir. Rol, 403 döndüren
//!   ara katmanın metin argümanlarından (`RequireRole("admin")`) ve tanımındaki rol karşılaştırmalarından (Yorum).
//! - **Girdiler:** gövde bağlama (`ShouldBindJSON`, `Bind`, `BodyParser`, `json.NewDecoder(r.Body).Decode`) hedefinin
//!   struct tipi; alan adları `json` / `form` / `query` / `uri` / `param` etiketlerinden, kurallar `binding` (Gin) ya da
//!   `validate` (go-playground/validator, doğrulama çağrısı görülürse) etiketlerinden. Doğrulama yoksa zorunluluk koddan
//!   kesin çıkarılamaz (`belirsiz`). Sorgu ve yol parametreleri kullanımdan (`c.Query("q")`, `chi.URLParam(r, "id")`,
//!   `r.PathValue("id")`), tipleri `strconv` dönüşümlerinden.
//! - **Yanıt:** başarılı `c.JSON(200, x)` / `json.NewEncoder(w).Encode(x)` değerinin statik tipi (kesin); `gin.H{...}`
//!   gibi harita değişmezlerinin anahtarları (Yorum).
//! - **Veri bağı (Yorum):** işleyiciden 3 kat çağrı zinciri boyunca GORM çağrıları (`db.Create(&u)`,
//!   `db.Model(&User{}).Updates(...)`) ve SQL metinleri (database/sql, sqlx, sqlc sabitleri). GORM modelleri, tablo
//!   adları (`TableName()` ya da GORM adlandırması) ve `constraint:OnDelete:CASCADE` kaskadları tanımdan okunur.
use crate::adaptor::{Alan, Girdi, Kapi, Sema, VeriBag, Yanit};
use crate::golang::{Dosya, Modul};
use crate::guvenli;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

pub const SURUM: &str = "0.1.0";
/// Yorumlamada ziyaret edilecek en fazla düğüm (dev depolarda süre sınırı)
const ADIM_SINIRI: usize = 400_000;
/// İç içe çağrı derinliği sınırı
const DERINLIK: usize = 12;
/// Bağlanmamış router'ın uygulama alanı (Express adaptörüyle aynı metin)
const BAGLANMAMIS: &str = "bağlanmamış router (önek bilinmiyor)";

/// Adaptörün sonucu
pub(crate) struct Sonuc {
    pub kapilar: Vec<Kapi>,
    pub bilgi: Value,
    pub veri_dosyalari: Vec<String>,
}

/// Bir tanım: (dosya, düğüm) — fonksiyon, metot ya da fonksiyon değişmezi
type Tanim<'a> = (usize, Node<'a>);
/// Paket düzeyi sabit ya da değişken: (dosya, ilk değer, bildirilen tip)
type PaketDegeri<'a> = (usize, Option<Node<'a>>, Option<Node<'a>>);
/// Gövdedeki veri kullanımı: (okunan, yazılan, eksik olabilir, silinen modeller: (model, satır, yumuşak))
type VeriGovdesi = (Vec<String>, Vec<String>, bool, Vec<(String, usize, bool)>);
/// Yerel değişken tipleri: ad -> (tipin yazıldığı dosya, tip metni, satır içi struct düğümü)
type Tipler<'a> = HashMap<String, (usize, String, Option<Node<'a>>)>;

fn metin<'k>(n: Node, k: &'k [u8]) -> &'k str {
    n.utf8_text(k).unwrap_or("")
}

fn cocuklar(n: Node) -> Vec<Node> {
    let mut w = n.walk();
    n.named_children(&mut w).filter(|c| c.kind() != "comment").collect()
}

fn adli_cocuklar<'t>(n: Node<'t>, alan: &str) -> Vec<Node<'t>> {
    let mut w = n.walk();
    n.children_by_field_name(alan, &mut w).collect()
}

fn satir(n: Node) -> usize {
    n.start_position().row + 1
}

fn sikistir(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Go metin değişmezi: "a\"b" (kaçışlar çözülür) ya da `ham`
fn metin_degismezi(n: Node, k: &[u8]) -> Option<String> {
    let t = metin(n, k);
    match n.kind() {
        "raw_string_literal" => Some(t.trim_matches('`').to_string()),
        "interpreted_string_literal" => {
            let ic = t.strip_prefix('"')?.strip_suffix('"')?;
            let (mut s, mut kacis) = (String::new(), false);
            for c in ic.chars() {
                if kacis {
                    s.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        x => x,
                    });
                    kacis = false;
                } else if c == '\\' {
                    kacis = true;
                } else {
                    s.push(c);
                }
            }
            Some(s)
        }
        _ => None,
    }
}

/// Sürüm eki olmadan içe aktarma yolu: github.com/labstack/echo/v4 -> github.com/labstack/echo
fn surumsuz(yol: &str) -> &str {
    match yol.rsplit_once('/') {
        Some((on, son)) if son.len() > 1 && son.starts_with('v') && son[1..].chars().all(|c| c.is_ascii_digit()) => on,
        _ => yol,
    }
}

/// İçe aktarma yolunun varsayılan paket adı: bilinen istisnalar tabloda, kalanlarda son parça (`.vN` ve `go-` önekleri
/// atılır: gopkg.in/yaml.v3 -> yaml, github.com/redis/go-redis/v9 -> redis)
fn varsayilan_ad(yol: &str) -> String {
    match surumsuz(yol) {
        "github.com/appleboy/gin-jwt" | "github.com/golang-jwt/jwt" | "github.com/dgrijalva/jwt-go" | "github.com/form3tech-oss/jwt-go" => return "jwt".into(),
        "github.com/gofiber/contrib/jwt" | "github.com/gofiber/jwt" => return "jwtware".into(),
        "github.com/labstack/echo-jwt" => return "echojwt".into(),
        "github.com/swaggo/gin-swagger" => return "ginSwagger".into(),
        "github.com/swaggo/echo-swagger" => return "echoSwagger".into(),
        "github.com/swaggo/http-swagger" => return "httpSwagger".into(),
        _ => {}
    }
    let son = surumsuz(yol).rsplit('/').next().unwrap_or(yol);
    let son = son.split('.').next().unwrap_or(son);
    son.rsplit('-').next().unwrap_or(son).to_string()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cati {
    Gin,
    Echo,
    Chi,
    Fiber,
    Mux,
    Http,
}

fn cati_yolu(yol: &str) -> Option<Cati> {
    Some(match surumsuz(yol) {
        "github.com/gin-gonic/gin" => Cati::Gin,
        "github.com/labstack/echo" => Cati::Echo,
        "github.com/go-chi/chi" => Cati::Chi,
        "github.com/gofiber/fiber" => Cati::Fiber,
        "github.com/gorilla/mux" => Cati::Mux,
        "net/http" => Cati::Http,
        _ => return None,
    })
}

/// Çatının router tipleri (parametre ve alan bildirimlerinde)
fn router_tipi_mi(c: Cati, ad: &str) -> bool {
    match c {
        Cati::Gin => matches!(ad, "Engine" | "RouterGroup" | "IRouter" | "IRoutes"),
        Cati::Echo => matches!(ad, "Echo" | "Group"),
        Cati::Chi => matches!(ad, "Router" | "Mux"),
        Cati::Fiber => matches!(ad, "App" | "Router" | "Group"),
        Cati::Mux => ad == "Router",
        Cati::Http => ad == "ServeMux",
    }
}

/// İşleyici tipi mi (sarmalayıcı ara katmanın parametresi, fabrikanın dönüşü)
fn isleyici_tipi_mi(t: &str) -> bool {
    let t = t.trim_start_matches('*');
    t.ends_with("Handler") || t.ends_with("HandlerFunc") || (t.starts_with("func(") && (t.contains("ResponseWriter") || t.contains("Context") || t.contains("Ctx")))
}

/// GORM adlandırması: UserProfile -> user_profiles
fn gorm_tablo_adi(ad: &str) -> String {
    let mut s = String::new();
    let karakterler: Vec<char> = ad.chars().collect();
    for (i, c) in karakterler.iter().enumerate() {
        if c.is_uppercase() {
            let onceki_kucuk = i > 0 && karakterler[i - 1].is_lowercase();
            let sonraki_kucuk = karakterler.get(i + 1).is_some_and(|x| x.is_lowercase());
            if i > 0 && (onceki_kucuk || (sonraki_kucuk && karakterler[i - 1].is_uppercase())) {
                s.push('_');
            }
            s.extend(c.to_lowercase());
        } else {
            s.push(*c);
        }
    }
    if s.ends_with('y') && !s.ends_with("ay") && !s.ends_with("ey") && !s.ends_with("oy") {
        s.pop();
        s.push_str("ies");
    } else if s.ends_with('s') || s.ends_with('x') || s.ends_with("ch") || s.ends_with("sh") {
        s.push_str("es");
    } else {
        s.push('s');
    }
    s
}

/// snake_case -> PascalCase (dosya adından kontrolcü adı)
fn pascal(s: &str) -> String {
    s.split(['_', '-', '.'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut k = p.chars();
            k.next().map(|f| f.to_uppercase().collect::<String>() + k.as_str()).unwrap_or_default()
        })
        .collect()
}

/// Satır içi işleyicinin adı: GET /users/:id -> getUsersId
fn fonksiyon_adi(yontem: &str, yol: &str) -> String {
    let mut s = yontem.to_lowercase();
    for p in yol.split('/').filter(|p| !p.is_empty()) {
        let mut ilk = true;
        for c in p.trim_start_matches(':').chars().filter(|c| c.is_alphanumeric()) {
            if ilk {
                s.extend(c.to_uppercase());
                ilk = false;
            } else {
                s.push(c);
            }
        }
    }
    s
}

/// Çatının yol şablonu -> Sezgi şablonu (`:ad`): {id} / {id:[0-9]+} / {yol...} / *dosya / * / :ad? ; {$} atılır
fn yol_normal(yol: &str) -> String {
    let mut out: Vec<String> = vec![];
    for p in yol.split('/') {
        let p = if let Some(ic) = p.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            if ic == "$" {
                // Tam eşleşme işareti: yol eğik çizgiyle biter
                out.push(String::new());
                continue;
            }
            format!(":{}", ic.split(':').next().unwrap_or(ic).trim_end_matches("...").trim())
        } else if let Some(ad) = p.strip_prefix('*') {
            format!(":{}", if ad.is_empty() { "wildcard" } else { ad })
        } else if p == "+" {
            ":wildcard".to_string()
        } else if let Some(ad) = p.strip_prefix(':') {
            // Fiber kısıtı (`:id<int>`) ve isteğe bağlılık (`:ad?`) şablonun parçası değildir
            format!(":{}", ad.split('<').next().unwrap_or(ad).trim_end_matches(['?', '+', '*']))
        } else {
            p.to_string()
        };
        out.push(p);
    }
    let mut s = out.join("/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    s
}

fn yol_birlestir(a: &str, b: &str) -> String {
    if b.is_empty() {
        return a.to_string();
    }
    if a.is_empty() {
        return b.to_string();
    }
    format!("{}/{}", a.trim_end_matches('/'), b.trim_start_matches('/'))
}

/// Yol şablonundaki tamsayı kısıtlı parametreler: {id:[0-9]+} / {id:\d+} / {id:int} (chi) / :id<int> (fiber)
fn yol_tamsayilari(yol: &str) -> Vec<String> {
    let mut v = vec![];
    for p in yol.split('/') {
        if let Some(ic) = p.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            if let Some((ad, desen)) = ic.split_once(':') {
                if matches!(desen, "[0-9]+" | "\\d+" | "[0-9]*" | "int" | "[1-9][0-9]*") {
                    v.push(ad.to_string());
                }
            }
        } else if let Some(x) = p.strip_prefix(':') {
            if let Some((ad, k)) = x.split_once('<') {
                if k.starts_with("int") {
                    v.push(ad.to_string());
                }
            }
        }
    }
    v
}

// ---- Proje dizini ----

/// GORM modeli
struct Model<'a> {
    di: usize,
    ad: String,
    tablo: String,
    tablo_kaynagi: &'static str,
    /// `gorm.Model` ya da `gorm.DeletedAt` alanı: silme yumuşaktır (`Unscoped()` yoksa)
    yumusak: bool,
    dugum: Node<'a>,
    satir: usize,
}

struct Proje<'a> {
    kok: &'a Path,
    dosyalar: &'a [Dosya],
    /// Dosyanın içe aktarma yolu
    paket: Vec<String>,
    /// Dosyanın `package` adı
    paket_adi: Vec<String>,
    /// Dosya başına içe aktarma: takma ad -> yol
    iceri: Vec<HashMap<String, String>>,
    yerel_paketler: HashSet<String>,
    fonksiyonlar: HashMap<(String, String), Tanim<'a>>,
    /// (paket, alıcı tipi, ad) -> metot
    metotlar: HashMap<(String, String, String), Tanim<'a>>,
    /// metot adı -> (dosya, düğüm, alıcı tipi)
    metot_adlari: HashMap<String, Vec<(usize, Node<'a>, String)>>,
    /// arayüz metot bildirimleri: ad -> (dosya, method_elem)
    arayuz_metotlari: HashMap<String, Vec<(usize, Node<'a>)>>,
    /// (paket, tip) -> type_spec
    tipler: HashMap<(String, String), (usize, Node<'a>)>,
    /// paket düzeyi sabit ve değişkenler: (paket, ad) -> (dosya, değer, tip)
    degerler: HashMap<(String, String), PaketDegeri<'a>>,
    /// `main` (package main) ve `init` fonksiyonları
    girisler: Vec<Tanim<'a>>,
    modeller: BTreeMap<(String, String), Model<'a>>,
    gorm: bool,
    sql: bool,
}

impl<'a> Proje<'a> {
    fn yeni(kok: &'a Path, dosyalar: &'a [Dosya], moduller: &[Modul]) -> Proje<'a> {
        let mut p = Proje {
            kok,
            dosyalar,
            paket: vec![],
            paket_adi: vec![],
            iceri: vec![],
            yerel_paketler: HashSet::new(),
            fonksiyonlar: HashMap::new(),
            metotlar: HashMap::new(),
            metot_adlari: HashMap::new(),
            arayuz_metotlari: HashMap::new(),
            tipler: HashMap::new(),
            degerler: HashMap::new(),
            girisler: vec![],
            modeller: BTreeMap::new(),
            gorm: false,
            sql: false,
        };
        for d in dosyalar {
            let k = d.kaynak.as_bytes();
            p.paket.push(crate::golang::ice_aktarma_yolu(kok, moduller, &d.yol));
            let ad = cocuklar(d.agac.root_node()).into_iter().find(|c| c.kind() == "package_clause").and_then(|c| c.named_child(0)).map(|c| metin(c, k).to_string());
            p.paket_adi.push(ad.unwrap_or_default());
        }
        // Yerel paketin adı (package clause): takma adsız içe aktarmada kullanılır
        let mut yerel_ad: HashMap<String, String> = HashMap::new();
        for (di, y) in p.paket.iter().enumerate() {
            if !p.paket_adi[di].ends_with("_test") {
                yerel_ad.entry(y.clone()).or_insert_with(|| p.paket_adi[di].clone());
            }
        }
        p.yerel_paketler = yerel_ad.keys().cloned().collect();
        for (di, d) in dosyalar.iter().enumerate() {
            let k = d.kaynak.as_bytes();
            let pkg = p.paket[di].clone();
            let mut iceri = HashMap::new();
            for n in cocuklar(d.agac.root_node()) {
                match n.kind() {
                    "import_declaration" => {
                        let specler: Vec<Node> = cocuklar(n).into_iter().flat_map(|c| if c.kind() == "import_spec_list" { cocuklar(c) } else { vec![c] }).collect();
                        for s in specler.into_iter().filter(|s| s.kind() == "import_spec") {
                            let Some(yol) = s.child_by_field_name("path").and_then(|x| metin_degismezi(x, k)) else { continue };
                            p.gorm |= yol == "gorm.io/gorm" || yol == "github.com/jinzhu/gorm";
                            p.sql |= yol == "database/sql" || yol.starts_with("github.com/jmoiron/sqlx") || yol.starts_with("github.com/jackc/pgx");
                            let ad = match s.child_by_field_name("name").map(|x| metin(x, k).to_string()) {
                                Some(a) if a == "_" || a == "." => continue,
                                Some(a) => a,
                                None => yerel_ad.get(&yol).cloned().unwrap_or_else(|| varsayilan_ad(&yol)),
                            };
                            iceri.insert(ad, yol);
                        }
                    }
                    "function_declaration" => {
                        let ad = n.child_by_field_name("name").map(|x| metin(x, k).to_string()).unwrap_or_default();
                        if (ad == "main" && p.paket_adi[di] == "main") || ad == "init" {
                            p.girisler.push((di, n));
                        } else {
                            p.fonksiyonlar.insert((pkg.clone(), ad), (di, n));
                        }
                    }
                    "method_declaration" => {
                        let ad = n.child_by_field_name("name").map(|x| metin(x, k).to_string()).unwrap_or_default();
                        let tip = alici_tipi_metni(n, k);
                        p.metotlar.insert((pkg.clone(), tip.clone(), ad.clone()), (di, n));
                        p.metot_adlari.entry(ad).or_default().push((di, n, tip));
                    }
                    "type_declaration" => {
                        for s in cocuklar(n).into_iter().filter(|s| matches!(s.kind(), "type_spec" | "type_alias")) {
                            let Some(ad) = s.child_by_field_name("name").map(|x| metin(x, k).to_string()) else { continue };
                            if let Some(t) = s.child_by_field_name("type").filter(|t| t.kind() == "interface_type") {
                                let mut yigin = cocuklar(t);
                                while let Some(m) = yigin.pop() {
                                    if m.kind() == "method_elem" {
                                        if let Some(a) = m.child_by_field_name("name") {
                                            p.arayuz_metotlari.entry(metin(a, k).to_string()).or_default().push((di, m));
                                        }
                                    } else {
                                        yigin.extend(cocuklar(m));
                                    }
                                }
                            }
                            p.tipler.insert((pkg.clone(), ad), (di, s));
                        }
                    }
                    "var_declaration" | "const_declaration" => {
                        for s in specler(n) {
                            let degerler: Vec<Node> = s.child_by_field_name("value").map(cocuklar).unwrap_or_default();
                            let tip = s.child_by_field_name("type");
                            for (i, a) in adli_cocuklar(s, "name").into_iter().enumerate() {
                                p.degerler.insert((pkg.clone(), metin(a, k).to_string()), (di, degerler.get(i).copied(), tip));
                            }
                        }
                    }
                    _ => {}
                }
            }
            p.iceri.push(iceri);
        }
        p.girisler.sort_by_key(|(di, n)| (p.dosyalar[*di].yol.clone(), n.start_byte()));
        if p.gorm {
            p.modelleri_bul();
        }
        p
    }

    fn k(&self, di: usize) -> &'a [u8] {
        self.dosyalar[di].kaynak.as_bytes()
    }

    fn goreli(&self, di: usize) -> String {
        guvenli::goreli(self.kok, &self.dosyalar[di].yol)
    }

    /// Tanımın adı (fonksiyon / metot); fonksiyon değişmezinde None
    fn ad(&self, t: Tanim<'a>) -> Option<String> {
        t.1.child_by_field_name("name").filter(|_| t.1.kind() != "func_literal").map(|n| metin(n, self.k(t.0)).to_string())
    }

    fn alici(&self, t: Tanim<'a>) -> Option<String> {
        (t.1.kind() == "method_declaration").then(|| alici_tipi_metni(t.1, self.k(t.0)))
    }

    /// Parametreler: (ad, tip metni, değişken sayılı mı)
    fn parametreler(&self, t: Tanim<'a>) -> Vec<(Option<String>, String, bool)> {
        let k = self.k(t.0);
        let mut v = vec![];
        for p in t.1.child_by_field_name("parameters").map(cocuklar).unwrap_or_default() {
            let tip = p.child_by_field_name("type").map(|x| sikistir(metin(x, k))).unwrap_or_default();
            let degisken = p.kind() == "variadic_parameter_declaration";
            let adlar = adli_cocuklar(p, "name");
            if adlar.is_empty() {
                v.push((None, tip.clone(), degisken));
            }
            for a in adlar {
                v.push((Some(metin(a, k).to_string()), tip.clone(), degisken));
            }
        }
        v
    }

    fn param_tipi(&self, t: Tanim<'a>, i: usize) -> Option<String> {
        let ps = self.parametreler(t);
        ps.get(i).or_else(|| ps.last().filter(|p| p.2)).map(|p| p.1.clone())
    }

    /// Sonuç tipleri (metin, tanımın dosyasında)
    fn sonuc_tipleri(&self, di: usize, n: Node<'a>) -> Vec<String> {
        let k = self.k(di);
        let Some(r) = n.child_by_field_name("result") else { return vec![] };
        if r.kind() == "parameter_list" {
            cocuklar(r)
                .into_iter()
                .flat_map(|p| {
                    let tip = p.child_by_field_name("type").map(|x| sikistir(metin(x, k))).unwrap_or_default();
                    let n = adli_cocuklar(p, "name").len().max(1);
                    std::iter::repeat_n(tip, n)
                })
                .collect()
        } else {
            vec![sikistir(metin(r, k))]
        }
    }

    /// Fonksiyonun döndürdüğü işleyici kapanışı: `return func(w, r) {...}` / `return http.HandlerFunc(func(...) {...})`
    fn donen_kapanis(&self, t: Tanim<'a>) -> Option<Tanim<'a>> {
        let govde = t.1.child_by_field_name("body")?;
        let mut yigin = vec![govde];
        while let Some(n) = yigin.pop() {
            match n.kind() {
                "func_literal" => continue,
                "return_statement" => {
                    let e = cocuklar(n).into_iter().flat_map(|e| if e.kind() == "expression_list" { cocuklar(e) } else { vec![e] }).next()?;
                    let mut e = e;
                    for _ in 0..3 {
                        match e.kind() {
                            "func_literal" => return Some((t.0, e)),
                            "call_expression" => e = e.child_by_field_name("arguments").and_then(|a| cocuklar(a).into_iter().last())?,
                            "parenthesized_expression" => e = e.named_child(0)?,
                            _ => break,
                        }
                    }
                }
                _ => yigin.extend(cocuklar(n)),
            }
        }
        None
    }

    fn isleyici_donduren(&self, t: Tanim<'a>) -> bool {
        self.sonuc_tipleri(t.0, t.1).first().is_some_and(|r| self.isleyici_tipi(t.0, r))
    }

    /// İşleyici tipi mi; projedeki adlı fonksiyon tipi (`type IsleyiciFonk func(db *gorm.DB, w http.ResponseWriter, r *http.Request)`)
    /// tanımından
    fn isleyici_tipi(&self, di: usize, tip: &str) -> bool {
        if isleyici_tipi_mi(tip) {
            return true;
        }
        let t = tip.trim().trim_start_matches('*');
        let (pkg, ad) = match t.split_once('.') {
            Some((on, ad)) => match self.iceri[di].get(on) {
                Some(y) => (y.clone(), ad.to_string()),
                None => return false,
            },
            None => (self.paket[di].clone(), t.to_string()),
        };
        self.tipler
            .get(&(pkg, ad))
            .and_then(|(sdi, spec)| spec.child_by_field_name("type").filter(|x| x.kind() == "function_type").map(|x| metin(x, self.k(*sdi)).to_string()))
            .is_some_and(|m| isleyici_tipi_mi(&m))
    }

    /// Tip metninin çatısı, metin bir router tipiyse: `*gin.RouterGroup`, `chi.Router`, `*http.ServeMux`
    fn router_catisi(&self, di: usize, tip: &str) -> Option<Cati> {
        let t = tip.trim().trim_start_matches('*');
        let (on, ad) = t.split_once('.')?;
        let c = cati_yolu(self.iceri[di].get(on)?)?;
        router_tipi_mi(c, ad).then_some(c)
    }

    /// Tip metninden projedeki tip: (dosya, type_spec'in tip düğümü, paket, ad, dizi mi). Takma ad tipleri
    /// (`type Liste []Kullanici`, `type A = B`) 4 adıma kadar izlenir
    fn yapi_bul(&self, di: usize, tip: &str) -> Option<(usize, Node<'a>, String, String, bool)> {
        self.yapi_bul_derin(di, tip, 0)
    }

    fn yapi_bul_derin(&self, di: usize, tip: &str, derinlik: usize) -> Option<(usize, Node<'a>, String, String, bool)> {
        if derinlik > 4 {
            return None;
        }
        let mut t = tip.trim();
        let mut dizi = false;
        loop {
            if let Some(x) = t.strip_prefix('*').or_else(|| t.strip_prefix('&')) {
                t = x.trim();
            } else if let Some(x) = t.strip_prefix("[]") {
                dizi = true;
                t = x.trim();
            } else if t.starts_with('[') && t.contains(']') {
                dizi = true;
                t = t[t.find(']')? + 1..].trim();
            } else {
                break;
            }
        }
        let t = t.split('[').next().unwrap_or(t).trim_end_matches(['{', '}', '(', ')']);
        let (pkg, ad) = match t.split_once('.') {
            Some((on, ad)) => (self.iceri[di].get(on)?.clone(), ad.to_string()),
            None => (self.paket[di].clone(), t.to_string()),
        };
        let &(sdi, spec) = self.tipler.get(&(pkg.clone(), ad.clone()))?;
        let tn = spec.child_by_field_name("type")?;
        match tn.kind() {
            "type_identifier" | "qualified_type" | "slice_type" | "pointer_type" | "generic_type" | "array_type" => {
                // Adlı tip başka bir tipin adıysa izlenir; temel tipse (`type Durum string`) alttaki tip düğümü döner
                let ic = metin(tn, self.k(sdi)).to_string();
                match self.yapi_bul_derin(sdi, &ic, derinlik + 1).filter(|_| ic != ad) {
                    Some(mut r) => {
                        r.4 |= dizi;
                        Some(r)
                    }
                    None => Some((sdi, tn, pkg, ad, dizi)),
                }
            }
            _ => Some((sdi, tn, pkg, ad, dizi)),
        }
    }

    /// Struct alanının tipi (gömülü yapılardan yükselenler dahil): (tipin dosyası, metni)
    fn alan_tipi(&self, pkg: &str, tip: &str, alan: &str, derinlik: usize) -> Option<(usize, String)> {
        let &(sdi, spec) = self.tipler.get(&(pkg.to_string(), tip.to_string()))?;
        let st = spec.child_by_field_name("type").filter(|t| t.kind() == "struct_type")?;
        let k = self.k(sdi);
        let mut gomulu = vec![];
        for fd in struct_alanlari(st) {
            let t = fd.child_by_field_name("type").map(|x| sikistir(metin(x, k))).unwrap_or_default();
            let adlar = adli_cocuklar(fd, "name");
            if adlar.is_empty() {
                gomulu.push(t);
            } else if adlar.iter().any(|a| metin(*a, k) == alan) {
                return Some((sdi, t));
            }
        }
        if derinlik < 4 {
            for g in gomulu {
                if let Some((_, _, p2, t2, _)) = self.yapi_bul(sdi, &g) {
                    if let Some(r) = self.alan_tipi(&p2, &t2, alan, derinlik + 1) {
                        return Some(r);
                    }
                }
            }
        }
        None
    }

    /// Metot: tipin kendisinde ya da gömülü yapılarında
    fn metot_bul(&self, pkg: &str, tip: &str, ad: &str, derinlik: usize) -> Option<Tanim<'a>> {
        if let Some(t) = self.metotlar.get(&(pkg.to_string(), tip.to_string(), ad.to_string())) {
            return Some(*t);
        }
        if derinlik >= 4 {
            return None;
        }
        let &(sdi, spec) = self.tipler.get(&(pkg.to_string(), tip.to_string()))?;
        let st = spec.child_by_field_name("type").filter(|t| t.kind() == "struct_type")?;
        let k = self.k(sdi);
        for fd in struct_alanlari(st).into_iter().filter(|fd| adli_cocuklar(*fd, "name").is_empty()) {
            let t = fd.child_by_field_name("type").map(|x| metin(x, k).to_string()).unwrap_or_default();
            if let Some((_, _, p2, t2, _)) = self.yapi_bul(sdi, &t) {
                if let Some(m) = self.metot_bul(&p2, &t2, ad, derinlik + 1) {
                    return Some(m);
                }
            }
        }
        None
    }

    /// Projede tek tanımlı metot (alıcısının tipi bilinmeyen çağrı ya da arayüz üzerinden çağrı); test taklitleri
    /// (`mock`) sayılmaz
    fn tekil_metot(&self, ad: &str) -> Option<Tanim<'a>> {
        let adaylar: Vec<&(usize, Node<'a>, String)> =
            self.metot_adlari.get(ad)?.iter().filter(|(di, _, t)| !self.goreli(*di).to_lowercase().contains("mock") && !t.to_lowercase().contains("mock")).collect();
        match adaylar.as_slice() {
            [(di, n, _)] => Some((*di, *n)),
            _ => None,
        }
    }

    /// Bildirimin hemen üstündeki yorum satırları (`//` atılmış)
    fn belge(&self, t: Tanim<'a>) -> Vec<String> {
        let k = self.k(t.0);
        let mut satirlar = vec![];
        let mut n = t.1;
        while let Some(o) = n.prev_sibling().filter(|o| o.kind() == "comment" && o.end_position().row + 1 >= n.start_position().row) {
            satirlar.push(metin(o, k).trim_start_matches("//").trim_start_matches("/*").trim_end_matches("*/").trim().to_string());
            n = o;
        }
        satirlar.reverse();
        satirlar
    }

    /// GORM modelleri: `gorm.Model` gömen, `gorm` etiketli alanı ya da `TableName()` metodu olan, `AutoMigrate`'e
    /// verilen yapılar
    fn modelleri_bul(&mut self) {
        let mut adaylar: HashSet<(String, String)> = HashSet::new();
        let mut tablo_adlari: HashMap<(String, String), String> = HashMap::new();
        for ((pkg, tip, ad), (di, n)) in &self.metotlar {
            if ad == "TableName" {
                adaylar.insert((pkg.clone(), tip.clone()));
                let k = self.k(*di);
                let mut yigin = vec![*n];
                while let Some(x) = yigin.pop() {
                    if x.kind() == "return_statement" {
                        if let Some(m) = cocuklar(x).into_iter().flat_map(|e| if e.kind() == "expression_list" { cocuklar(e) } else { vec![e] }).find_map(|e| metin_degismezi(e, k))
                        {
                            tablo_adlari.insert((pkg.clone(), tip.clone()), m);
                        }
                    }
                    yigin.extend(cocuklar(x));
                }
            }
        }
        for ((pkg, ad), (di, spec)) in &self.tipler {
            let Some(st) = spec.child_by_field_name("type").filter(|t| t.kind() == "struct_type") else { continue };
            let k = self.k(*di);
            let govde = metin(st, k);
            if govde.contains("gorm.Model") || govde.contains("gorm:\"") || govde.contains("gorm.DeletedAt") {
                adaylar.insert((pkg.clone(), ad.clone()));
            }
        }
        // AutoMigrate(&User{}, &Post{})
        for (di, d) in self.dosyalar.iter().enumerate() {
            let k = d.kaynak.as_bytes();
            if !d.kaynak.contains("AutoMigrate") {
                continue;
            }
            let mut yigin = vec![d.agac.root_node()];
            while let Some(n) = yigin.pop() {
                if n.kind() == "call_expression" && n.child_by_field_name("function").is_some_and(|f| metin(f, k).ends_with("AutoMigrate")) {
                    for a in n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default() {
                        let t = metin(a, k).trim_start_matches('&').split('{').next().unwrap_or("").to_string();
                        if let Some((_, _, p2, t2, _)) = self.yapi_bul(di, &t) {
                            adaylar.insert((p2, t2));
                        }
                    }
                }
                yigin.extend(cocuklar(n));
            }
        }
        for (pkg, ad) in adaylar {
            let Some(&(di, spec)) = self.tipler.get(&(pkg.clone(), ad.clone())) else { continue };
            let Some(st) = spec.child_by_field_name("type").filter(|t| t.kind() == "struct_type") else { continue };
            let govde = metin(st, self.k(di));
            let (tablo, kaynak) = match tablo_adlari.get(&(pkg.clone(), ad.clone())) {
                Some(t) => (t.clone(), "TableName()"),
                None => (gorm_tablo_adi(&ad), "varsayılan (GORM adlandırması)"),
            };
            let yumusak = govde.contains("gorm.Model") || govde.contains("gorm.DeletedAt") || govde.contains("soft_delete.DeletedAt");
            self.modeller.insert((pkg, ad.clone()), Model { di, ad, tablo, tablo_kaynagi: kaynak, yumusak, dugum: st, satir: satir(spec) });
        }
    }

    /// Tip metninin GORM modeli
    fn model(&self, di: usize, tip: &str) -> Option<&Model<'a>> {
        let (_, _, pkg, ad, _) = self.yapi_bul(di, tip)?;
        self.modeller.get(&(pkg, ad))
    }

    /// Tablo adından model adı (yoksa tablo adının kendisi)
    fn tablo_modeli(&self, tablo: &str) -> String {
        self.modeller.values().find(|m| m.tablo.eq_ignore_ascii_case(tablo)).map(|m| m.ad.clone()).unwrap_or_else(|| tablo.to_string())
    }
}

/// Alıcı tipinin adı: `func (s *Sunucu) x()` -> "Sunucu"
fn alici_tipi_metni(n: Node, k: &[u8]) -> String {
    n.child_by_field_name("receiver")
        .and_then(|r| cocuklar(r).into_iter().next())
        .and_then(|p| p.child_by_field_name("type"))
        .map(|t| metin(t, k).trim_start_matches('*').split('[').next().unwrap_or("").trim().to_string())
        .unwrap_or_default()
}

fn specler(n: Node) -> Vec<Node> {
    cocuklar(n).into_iter().flat_map(|c| if c.kind().ends_with("_spec_list") { cocuklar(c) } else { vec![c] }).filter(|c| c.kind().ends_with("_spec")).collect()
}

fn struct_alanlari(st: Node) -> Vec<Node> {
    cocuklar(st).into_iter().filter(|c| c.kind() == "field_declaration_list").flat_map(cocuklar).filter(|c| c.kind() == "field_declaration").collect()
}

/// Struct etiketi: `json:"ad,omitempty" binding:"required"` -> {json: "ad,omitempty", binding: "required"}
fn etiketler(t: &str) -> HashMap<String, String> {
    static D: OnceLock<Regex> = OnceLock::new();
    let re = D.get_or_init(|| Regex::new(r#"(\w+):"((?:[^"\\]|\\.)*)""#).unwrap());
    re.captures_iter(t).map(|c| (c[1].to_string(), c[2].to_string())).collect()
}

// ---- Yorumlayıcı ----

/// Ara katman ifadesi ve çözümü
#[derive(Clone, Debug)]
struct AraD<'a> {
    metin: String,
    /// Projedeki tanımı (fonksiyon, metot ya da değişmez)
    fonk: Option<Tanim<'a>>,
    /// Dış paketin içe aktarma yolu
    dis: Option<String>,
    /// Son ad: JWT, RequireRole, MiddlewareFunc
    ad: String,
    /// Metin argümanları (rol adları): sabitler çözülmüş
    argumanlar: Vec<String>,
    /// Argümanların kod metni (`AuthMiddleware(false)` gibi bayraklar için)
    ham: Vec<String>,
    /// Fiber `Use("/onek", mw)`: yalnızca bu önekle başlayan yollara
    yol: Option<String>,
}

#[derive(Clone)]
enum Deger<'a> {
    Router(usize),
    /// gorilla/mux rota taslağı (`r.Methods("GET").Path("/x")`)
    Taslak(usize),
    /// Kaydedilen rotalar (gorilla `.Methods(...)` zinciri)
    Rotalar(Vec<usize>),
    Fonk(Tanim<'a>),
    /// Projedeki bir struct tipinin değeri: (paket, tip)
    Yapi(String, String),
    /// Struct değişmezi (tablo tabanlı rota listeleri): (dosya, literal_value, tip)
    Lit(usize, Node<'a>, Option<(String, String)>),
    Liste(Vec<Deger<'a>>),
    Metin(String),
    /// `http.StripPrefix("/api", alt)`
    Soyulmus(String, Box<Deger<'a>>),
    /// Değerlendirilmemiş ifade (işleyici ya da ara katman olarak sonra çözülür)
    Ifade(usize, Node<'a>),
    /// Dış paketin bir öğesi: (yol, ad)
    Dis(String, String),
    Bilinmeyen,
}

struct Bag<'a> {
    ust: usize,
    onek: String,
    /// Önek çalışma anında kuruluyor (`r.Group(ayar.Onek)`): ifadenin metni
    dinamik: Option<String>,
    ara: Vec<AraD<'a>>,
    sira: u64,
    di: usize,
    satir: usize,
    kosul: Option<String>,
}

struct RouterD<'a> {
    cati: Cati,
    di: usize,
    satir: usize,
    ad: String,
    kok_mu: bool,
    /// Parametre ya da alan için kurulan, hiçbir uygulamaya bağlanmamış router
    bagimsiz: bool,
    /// `main` / `init` yorumlanırken kuruldu
    ana_adayi: bool,
    baglar: Vec<Bag<'a>>,
    ara: Vec<(AraD<'a>, u64)>,
}

#[derive(Clone, Default)]
struct Isleyici<'a> {
    tanim: Option<Tanim<'a>>,
    /// Adlı tanım (fonksiyon, metot ya da kapanışı döndüren fabrika): kapının fonksiyonu ve konumu
    adli: Option<Tanim<'a>>,
    ad: Option<String>,
    alici: Option<String>,
    ara: Vec<AraD<'a>>,
}

struct RotaD<'a> {
    router: usize,
    yontemler: Vec<String>,
    /// Yöntem kayıtta açıkça verildi (Go 1.22 deseni, `GET`, `.Methods(...)`); değilse işleyici gövdesinden aranır
    yontem_acik: bool,
    yol: String,
    isl: Isleyici<'a>,
    ara: Vec<AraD<'a>>,
    di: usize,
    satir: usize,
    sira: u64,
    kosul: Option<String>,
}

struct Taslak {
    router: usize,
    yol: Option<String>,
    onek: Option<String>,
    yontemler: Vec<String>,
}

struct Cerceve<'a> {
    di: usize,
    ortam: HashMap<String, Deger<'a>>,
    kosullar: Vec<String>,
    donus: Option<Deger<'a>>,
    derinlik: usize,
}

impl<'a> Cerceve<'a> {
    fn yeni(di: usize, derinlik: usize) -> Self {
        Cerceve { di, ortam: HashMap::new(), kosullar: vec![], donus: None, derinlik }
    }
}

struct Yorum<'a, 'p> {
    p: &'p Proje<'a>,
    routerlar: Vec<RouterD<'a>>,
    taslaklar: Vec<Taslak>,
    rotalar: Vec<RotaD<'a>>,
    sira: u64,
    adim: usize,
    yigin: Vec<usize>,
    yorumlanan: HashSet<usize>,
    alanlar: HashMap<(String, String), Deger<'a>>,
    paket_degerleri: HashMap<(String, String), Deger<'a>>,
    ana_mod: bool,
    varsayilan_mux: Option<usize>,
    /// Yolu çözülemeyen kayıtlar: "dosya:satır ifade"
    dinamik: Vec<String>,
}

impl<'a, 'p> Yorum<'a, 'p> {
    fn sonraki(&mut self) -> u64 {
        self.sira += 1;
        self.sira
    }

    fn yeni_router(&mut self, cati: Cati, di: usize, satir: usize, bagimsiz: bool, ad: &str) -> usize {
        self.routerlar.push(RouterD { cati, di, satir, ad: ad.to_string(), kok_mu: true, bagimsiz, ana_adayi: self.ana_mod && !bagimsiz, baglar: vec![], ara: vec![] });
        self.routerlar.len() - 1
    }

    fn varsayilan(&mut self, f: &Cerceve<'a>) -> usize {
        if let Some(r) = self.varsayilan_mux {
            return r;
        }
        let r = self.yeni_router(Cati::Http, f.di, 0, false, "http.DefaultServeMux");
        self.varsayilan_mux = Some(r);
        r
    }

    fn kosul(f: &Cerceve<'a>) -> Option<String> {
        (!f.kosullar.is_empty()).then(|| f.kosullar.join(" && "))
    }

    /// Alt router: grup, `Route`, `With`, `Subrouter`
    fn alt_router(&mut self, ust: usize, onek: String, ara: Vec<AraD<'a>>, n: Node<'a>, f: &Cerceve<'a>) -> usize {
        let sira = self.sonraki();
        let c = self.routerlar[ust].cati;
        self.routerlar.push(RouterD {
            cati: c,
            di: f.di,
            satir: satir(n),
            ad: String::new(),
            kok_mu: false,
            bagimsiz: false,
            ana_adayi: false,
            baglar: vec![Bag { ust, onek, dinamik: None, ara, sira, di: f.di, satir: satir(n), kosul: Self::kosul(f) }],
            ara: vec![],
        });
        self.routerlar.len() - 1
    }

    /// Öneki çözülemeyen grup: önek uydurulmaz; rotaları ikincil sayılır (yolu eksik olabilir)
    fn dinamik_alt(&mut self, ust: usize, ifade: Option<Node<'a>>, ara: Vec<AraD<'a>>, n: Node<'a>, f: &Cerceve<'a>) -> usize {
        let m = ifade.map(|x| sikistir(metin(x, self.p.k(f.di)))).unwrap_or_default();
        self.dinamik.push(format!("{}:{} önek {m}", self.p.goreli(f.di), satir(n)));
        let alt = self.alt_router(ust, String::new(), ara, n, f);
        if let Some(b) = self.routerlar[alt].baglar.last_mut() {
            b.dinamik = Some(m);
        }
        alt
    }

    fn bagla_alt(&mut self, ust: usize, alt: usize, onek: String, n: Node<'a>, f: &Cerceve<'a>) {
        if ust == alt || self.routerlar[alt].baglar.iter().any(|b| b.ust == ust && b.onek == onek) {
            return;
        }
        let sira = self.sonraki();
        self.routerlar[alt].baglar.push(Bag { ust, onek, dinamik: None, ara: vec![], sira, di: f.di, satir: satir(n), kosul: Self::kosul(f) });
    }

    // ---- Deyimler ----

    fn calistir(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) {
        self.adim += 1;
        if self.adim > ADIM_SINIRI {
            return;
        }
        let k = self.p.k(f.di);
        match n.kind() {
            "block" | "statement_list" | "labeled_statement" => {
                for c in cocuklar(n) {
                    self.calistir(c, f);
                }
            }
            "expression_statement" | "go_statement" | "defer_statement" => {
                for c in cocuklar(n) {
                    self.ifade(c, f);
                }
            }
            "short_var_declaration" | "assignment_statement" => self.atama(n, f),
            "var_declaration" | "const_declaration" => {
                for s in specler(n) {
                    let degerler: Vec<Node<'a>> = s.child_by_field_name("value").map(cocuklar).unwrap_or_default();
                    let tip = s.child_by_field_name("type").map(|t| metin(t, k).to_string());
                    for (i, a) in adli_cocuklar(s, "name").into_iter().enumerate() {
                        let v = match degerler.get(i) {
                            Some(d) => self.ifade(*d, f),
                            None => tip.as_deref().and_then(|t| self.p.yapi_bul(f.di, t)).filter(|x| !x.4).map(|x| Deger::Yapi(x.2, x.3)).unwrap_or(Deger::Bilinmeyen),
                        };
                        self.bagla(a, v, degerler.get(i).copied(), true, f);
                    }
                }
            }
            "if_statement" => {
                if let Some(i) = n.child_by_field_name("initializer") {
                    self.calistir(i, f);
                }
                let kosul = n.child_by_field_name("condition").map(|c| sikistir(metin(c, k))).unwrap_or_default();
                if let Some(c) = n.child_by_field_name("consequence") {
                    f.kosullar.push(kosul.clone());
                    self.calistir(c, f);
                    f.kosullar.pop();
                }
                if let Some(a) = n.child_by_field_name("alternative") {
                    f.kosullar.push(format!("!({kosul})"));
                    self.calistir(a, f);
                    f.kosullar.pop();
                }
            }
            "for_statement" => self.dongu(n, f),
            "expression_switch_statement" | "type_switch_statement" | "select_statement" => {
                if let Some(i) = n.child_by_field_name("initializer") {
                    self.calistir(i, f);
                }
                let deger = n.child_by_field_name("value").map(|v| sikistir(metin(v, k))).unwrap_or_default();
                for c in cocuklar(n).into_iter().filter(|c| c.kind().ends_with("_case")) {
                    let etiket = match c.child_by_field_name("value") {
                        Some(v) => format!("{deger} == {}", sikistir(metin(v, k))),
                        None => format!("{deger}: default"),
                    };
                    f.kosullar.push(etiket);
                    for s in cocuklar(c).into_iter().filter(|s| s.kind() == "statement_list") {
                        self.calistir(s, f);
                    }
                    f.kosullar.pop();
                }
            }
            "return_statement" => {
                let ifadeler: Vec<Node<'a>> = cocuklar(n).into_iter().flat_map(|e| if e.kind() == "expression_list" { cocuklar(e) } else { vec![e] }).collect();
                if let Some(e) = ifadeler.first() {
                    let v = match self.ifade(*e, f) {
                        Deger::Bilinmeyen => Deger::Ifade(f.di, *e),
                        v => v,
                    };
                    if f.donus.as_ref().is_none_or(|d| matches!(d, Deger::Bilinmeyen | Deger::Ifade(..))) {
                        f.donus = Some(v);
                    }
                }
            }
            _ => {}
        }
    }

    /// Döngü: tablo tabanlı rota listesinde (`for _, r := range rotalar { mux.HandleFunc(r.Yol, r.Isleyici) }`) her öğe
    /// için gövde bir kez; değilse gövde bir kez
    fn dongu(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) {
        let Some(govde) = n.child_by_field_name("body") else { return };
        let aralik = cocuklar(n).into_iter().find(|c| c.kind() == "range_clause");
        if let Some(r) = aralik {
            let sol: Vec<Node<'a>> = r.child_by_field_name("left").map(cocuklar).unwrap_or_default();
            let ogeler = match r.child_by_field_name("right").map(|s| self.ifade(s, f)) {
                Some(Deger::Liste(l)) => l,
                _ => vec![],
            };
            if let (Some(deg), false) = (sol.last(), ogeler.is_empty()) {
                let ad = metin(*deg, self.p.k(f.di)).to_string();
                for o in ogeler.into_iter().take(512) {
                    f.ortam.insert(ad.clone(), o);
                    self.calistir(govde, f);
                }
                return;
            }
        }
        self.calistir(govde, f);
    }

    fn atama(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) {
        let sol: Vec<Node<'a>> = n.child_by_field_name("left").map(cocuklar).unwrap_or_default();
        let sag: Vec<Node<'a>> = n.child_by_field_name("right").map(cocuklar).unwrap_or_default();
        let tanim = n.kind() == "short_var_declaration";
        let degerler: Vec<Deger<'a>> = sag.iter().map(|s| self.ifade(*s, f)).collect();
        for (i, l) in sol.iter().enumerate() {
            let (v, kaynak) = if sag.len() == sol.len() {
                (degerler[i].clone(), Some(sag[i]))
            } else if i == 0 {
                (degerler.first().cloned().unwrap_or(Deger::Bilinmeyen), sag.first().copied())
            } else {
                (Deger::Bilinmeyen, None)
            };
            self.bagla(*l, v, kaynak, tanim, f);
        }
    }

    fn bagla(&mut self, l: Node<'a>, v: Deger<'a>, kaynak: Option<Node<'a>>, tanim: bool, f: &mut Cerceve<'a>) {
        let k = self.p.k(f.di);
        let v = match (v, kaynak) {
            (Deger::Bilinmeyen, Some(s)) => match self.metin_degeri(s, f, 0) {
                Some(m) => Deger::Metin(m),
                None => Deger::Ifade(f.di, s),
            },
            (v, _) => v,
        };
        if let Deger::Router(r) = &v {
            if self.routerlar[*r].ad.is_empty() {
                self.routerlar[*r].ad = sikistir(metin(l, k));
            }
        }
        match l.kind() {
            "identifier" => {
                let ad = metin(l, k);
                if ad == "_" {
                    return;
                }
                if !tanim && !f.ortam.contains_key(ad) {
                    let anahtar = (self.p.paket[f.di].clone(), ad.to_string());
                    if self.p.degerler.contains_key(&anahtar) {
                        self.paket_degerleri.insert(anahtar, v);
                        return;
                    }
                }
                f.ortam.insert(ad.to_string(), v);
            }
            "selector_expression" => {
                if let (Some(o), Some(alan)) = (l.child_by_field_name("operand"), l.child_by_field_name("field")) {
                    if let Deger::Yapi(_, t) = self.ifade(o, f) {
                        self.alanlar.insert((t, metin(alan, k).to_string()), v.clone());
                    }
                }
                f.ortam.insert(sikistir(metin(l, k)), v);
            }
            _ => {}
        }
    }

    // ---- İfadeler ----

    fn ifade(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) -> Deger<'a> {
        self.adim += 1;
        if self.adim > ADIM_SINIRI {
            return Deger::Bilinmeyen;
        }
        let k = self.p.k(f.di);
        match n.kind() {
            "identifier" => self.ad_degeri(metin(n, k), f),
            "selector_expression" => self.secici(n, f),
            "call_expression" => self.cagri(n, f),
            "unary_expression" | "type_assertion_expression" => n.child_by_field_name("operand").map(|o| self.ifade(o, f)).unwrap_or(Deger::Bilinmeyen),
            "parenthesized_expression" | "variadic_argument" => n.named_child(0).map(|o| self.ifade(o, f)).unwrap_or(Deger::Bilinmeyen),
            "composite_literal" => self.bilesik(n, f),
            "func_literal" => Deger::Fonk((f.di, n)),
            "interpreted_string_literal" | "raw_string_literal" | "binary_expression" => self.metin_degeri(n, f, 0).map(Deger::Metin).unwrap_or(Deger::Bilinmeyen),
            _ => Deger::Bilinmeyen,
        }
    }

    fn paket_degeri(&mut self, anahtar: (String, String), derinlik: usize) -> Deger<'a> {
        if let Some(v) = self.paket_degerleri.get(&anahtar) {
            return v.clone();
        }
        if let Some(&(vdi, deger, tip)) = self.p.degerler.get(&anahtar) {
            // Paket düzeyi değişkenin ilk değeri bir kez yorumlanır (döngüye karşı önce yer tutucu)
            self.paket_degerleri.insert(anahtar.clone(), Deger::Bilinmeyen);
            let mut c = Cerceve::yeni(vdi, derinlik + 1);
            let v = match deger {
                Some(d) => match self.ifade(d, &mut c) {
                    Deger::Bilinmeyen => Deger::Ifade(vdi, d),
                    v => v,
                },
                None => tip.and_then(|t| self.p.yapi_bul(vdi, metin(t, self.p.k(vdi)))).filter(|x| !x.4).map(|x| Deger::Yapi(x.2, x.3)).unwrap_or(Deger::Bilinmeyen),
            };
            self.paket_degerleri.insert(anahtar, v.clone());
            return v;
        }
        if let Some(&t) = self.p.fonksiyonlar.get(&anahtar) {
            return Deger::Fonk(t);
        }
        Deger::Bilinmeyen
    }

    fn ad_degeri(&mut self, ad: &str, f: &mut Cerceve<'a>) -> Deger<'a> {
        if let Some(v) = f.ortam.get(ad) {
            return v.clone();
        }
        if matches!(ad, "nil" | "true" | "false") {
            return Deger::Bilinmeyen;
        }
        self.paket_degeri((self.p.paket[f.di].clone(), ad.to_string()), f.derinlik)
    }

    fn secici(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) -> Deger<'a> {
        let k = self.p.k(f.di);
        let (Some(o), Some(alan)) = (n.child_by_field_name("operand"), n.child_by_field_name("field")) else { return Deger::Bilinmeyen };
        let alan = metin(alan, k).to_string();
        let tam = sikistir(metin(n, k));
        if let Some(v) = f.ortam.get(&tam) {
            return v.clone();
        }
        if o.kind() == "identifier" && !f.ortam.contains_key(metin(o, k)) {
            if let Some(yol) = self.p.iceri[f.di].get(metin(o, k)).cloned() {
                if yol == "net/http" && alan == "DefaultServeMux" {
                    return Deger::Router(self.varsayilan(f));
                }
                if self.p.yerel_paketler.contains(&yol) {
                    return self.paket_degeri((yol, alan), f.derinlik);
                }
                return Deger::Dis(yol, alan);
            }
        }
        match self.ifade(o, f) {
            Deger::Yapi(pkg, t) => {
                if let Some(x) = self.alanlar.get(&(t.clone(), alan.clone())) {
                    return x.clone();
                }
                if let Some(m) = self.p.metot_bul(&pkg, &t, &alan, 0) {
                    return Deger::Fonk(m);
                }
                if let Some((tdi, tip)) = self.p.alan_tipi(&pkg, &t, &alan, 0) {
                    if let Some(c) = self.p.router_catisi(tdi, &tip) {
                        // Alanın değeri izlenemedi: router bağlanmamış sayılır
                        let r = self.yeni_router(c, tdi, satir(n), true, &format!("{t}.{alan}"));
                        self.alanlar.insert((t, alan), Deger::Router(r));
                        return Deger::Router(r);
                    }
                    if let Some(x) = self.p.yapi_bul(tdi, &tip).filter(|x| !x.4) {
                        return Deger::Yapi(x.2, x.3);
                    }
                }
                Deger::Bilinmeyen
            }
            Deger::Lit(ldi, lit, tip) => self.lit_alani(ldi, lit, tip, &alan, f),
            Deger::Dis(y, a) => Deger::Dis(y, format!("{a}.{alan}")),
            _ => Deger::Bilinmeyen,
        }
    }

    /// Struct değişmezinin alanı: anahtarlı öğe ya da (tip biliniyorsa) sıradaki öğe
    fn lit_alani(&mut self, ldi: usize, lit: Node<'a>, tip: Option<(String, String)>, alan: &str, f: &mut Cerceve<'a>) -> Deger<'a> {
        let k = self.p.k(ldi);
        let ogeler = cocuklar(lit);
        let mut hedef = None;
        for (i, o) in ogeler.iter().enumerate() {
            if o.kind() == "keyed_element" {
                let c = cocuklar(*o);
                if c.len() == 2 && metin(c[0], k).trim() == alan {
                    hedef = c[1].named_child(0).or(Some(c[1]));
                }
            } else if let Some((pkg, t)) = &tip {
                let sira = self.p.tipler.get(&(pkg.clone(), t.clone())).and_then(|&(sdi, spec)| {
                    let st = spec.child_by_field_name("type")?;
                    let adlar: Vec<String> = struct_alanlari(st).into_iter().flat_map(|fd| adli_cocuklar(fd, "name")).map(|a| metin(a, self.p.k(sdi)).to_string()).collect();
                    adlar.iter().position(|a| a == alan)
                });
                if sira == Some(i) {
                    hedef = o.named_child(0).or(Some(*o));
                }
            }
        }
        let Some(h) = hedef else { return Deger::Bilinmeyen };
        let v = if ldi == f.di {
            self.ifade(h, f)
        } else {
            let mut c = Cerceve::yeni(ldi, f.derinlik);
            self.ifade(h, &mut c)
        };
        match v {
            Deger::Bilinmeyen => Deger::Ifade(ldi, h),
            v => v,
        }
    }

    fn bilesik(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) -> Deger<'a> {
        let k = self.p.k(f.di);
        let tip = n.child_by_field_name("type");
        let Some(govde) = n.child_by_field_name("body") else { return Deger::Bilinmeyen };
        let tip_metni = tip.map(|t| sikistir(metin(t, k))).unwrap_or_default();
        // http.Server{Handler: r}
        if let Some((on, ad)) = tip_metni.split_once('.') {
            if self.p.iceri[f.di].get(on).map(String::as_str) == Some("net/http") && ad == "Server" {
                for o in cocuklar(govde).into_iter().filter(|o| o.kind() == "keyed_element") {
                    let c = cocuklar(o);
                    if c.len() == 2 && metin(c[0], k).trim() == "Handler" {
                        if let Some(e) = c[1].named_child(0) {
                            self.ifade(e, f);
                        }
                    }
                }
                return Deger::Bilinmeyen;
            }
        }
        // Dilim değişmezi: öğeler (tablo tabanlı rotalar)
        if tip.is_some_and(|t| matches!(t.kind(), "slice_type" | "array_type" | "implicit_length_array_type")) {
            let oge_tipi = tip.and_then(|t| t.child_by_field_name("element")).and_then(|e| self.p.yapi_bul(f.di, metin(e, k))).filter(|x| !x.4).map(|x| (x.2, x.3));
            let mut v = vec![];
            for o in cocuklar(govde).into_iter().take(512) {
                let ic = if o.kind() == "literal_element" { o.named_child(0).unwrap_or(o) } else { o };
                v.push(match ic.kind() {
                    "literal_value" => Deger::Lit(f.di, ic, oge_tipi.clone()),
                    _ => match self.ifade(ic, f) {
                        Deger::Bilinmeyen => Deger::Ifade(f.di, ic),
                        x => x,
                    },
                });
            }
            return Deger::Liste(v);
        }
        let yapi = self.p.yapi_bul(f.di, &tip_metni).filter(|x| !x.4).map(|x| (x.2, x.3));
        for o in cocuklar(govde).into_iter().filter(|o| o.kind() == "keyed_element") {
            let c = cocuklar(o);
            if c.len() != 2 {
                continue;
            }
            let Some(e) = c[1].named_child(0) else { continue };
            let v = self.ifade(e, f);
            if let (Some((_, t)), Deger::Router(_) | Deger::Yapi(..) | Deger::Fonk(_)) = (&yapi, &v) {
                self.alanlar.insert((t.clone(), metin(c[0], k).trim().to_string()), v);
            }
        }
        match yapi {
            Some((pkg, t)) => Deger::Yapi(pkg, t),
            None => Deger::Lit(f.di, govde, None),
        }
    }

    /// Metin değeri: değişmez, `+` birleştirme, `fmt.Sprintf`, yerel / paket sabiti, struct değişmezi alanı
    fn metin_degeri(&mut self, n: Node<'a>, f: &mut Cerceve<'a>, derinlik: usize) -> Option<String> {
        if derinlik > 8 {
            return None;
        }
        let k = self.p.k(f.di);
        match n.kind() {
            "interpreted_string_literal" | "raw_string_literal" => metin_degismezi(n, k),
            "binary_expression" => {
                let op = n.child_by_field_name("operator").map(|o| metin(o, k)).unwrap_or("");
                if op != "+" {
                    return None;
                }
                let a = self.metin_degeri(n.child_by_field_name("left")?, f, derinlik + 1)?;
                let b = self.metin_degeri(n.child_by_field_name("right")?, f, derinlik + 1)?;
                Some(a + &b)
            }
            "parenthesized_expression" => self.metin_degeri(n.named_child(0)?, f, derinlik + 1),
            "call_expression" => {
                let g = sikistir(metin(n.child_by_field_name("function")?, k));
                let args = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
                match g.as_str() {
                    "fmt.Sprintf" => {
                        let bicim = self.metin_degeri(*args.first()?, f, derinlik + 1)?;
                        let mut kalan = args[1..].iter();
                        let mut out = String::new();
                        let mut ch = bicim.chars().peekable();
                        while let Some(c) = ch.next() {
                            if c == '%' {
                                match ch.next()? {
                                    '%' => out.push('%'),
                                    's' | 'v' | 'd' => out.push_str(&self.metin_degeri(*kalan.next()?, f, derinlik + 1)?),
                                    _ => return None,
                                }
                            } else {
                                out.push(c);
                            }
                        }
                        Some(out)
                    }
                    "path.Join" => {
                        let parcalar: Option<Vec<String>> = args.iter().map(|a| self.metin_degeri(*a, f, derinlik + 1)).collect();
                        let p = parcalar?.iter().map(|x| x.trim_matches('/').to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>().join("/");
                        Some(format!("/{p}"))
                    }
                    _ => None,
                }
            }
            "identifier" | "selector_expression" => match self.ifade(n, f) {
                Deger::Metin(s) => Some(s),
                Deger::Ifade(di, e) if e.id() != n.id() => {
                    if di == f.di {
                        self.metin_degeri(e, f, derinlik + 1)
                    } else {
                        let mut c = Cerceve::yeni(di, f.derinlik);
                        self.metin_degeri(e, &mut c, derinlik + 1)
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Metin listesi: `[]string{"a", "b"}` ya da tek metin
    fn metin_listesi(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) -> Vec<String> {
        if n.kind() == "composite_literal" {
            if let Some(g) = n.child_by_field_name("body") {
                return cocuklar(g).into_iter().filter_map(|o| o.named_child(0)).filter_map(|e| self.metin_degeri(e, f, 0)).collect();
            }
        }
        if n.kind() == "selector_expression" {
            let t = metin(n, self.p.k(f.di));
            if let Some(m) = t.strip_prefix("http.Method") {
                return vec![m.to_uppercase()];
            }
        }
        self.metin_degeri(n, f, 0).into_iter().collect()
    }

    // ---- Çağrılar ----

    fn cagri(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) -> Deger<'a> {
        let k = self.p.k(f.di);
        let Some(g) = n.child_by_field_name("function") else { return Deger::Bilinmeyen };
        let args: Vec<Node<'a>> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
        match g.kind() {
            "selector_expression" => {
                let (Some(o), Some(a)) = (g.child_by_field_name("operand"), g.child_by_field_name("field")) else { return Deger::Bilinmeyen };
                let ad = metin(a, k).to_string();
                if o.kind() == "identifier" && !f.ortam.contains_key(metin(o, k)) {
                    if let Some(yol) = self.p.iceri[f.di].get(metin(o, k)).cloned() {
                        return self.paket_cagrisi(&yol, &ad, n, &args, f);
                    }
                }
                let v = self.ifade(o, f);
                match v {
                    Deger::Router(r) => return self.router_metodu(r, &ad, n, &args, f),
                    Deger::Taslak(t) => return self.taslak_metodu(t, &ad, n, &args, f),
                    Deger::Rotalar(rs) => return self.rota_metodu(rs, &ad, &args, f),
                    Deger::Yapi(pkg, t) => {
                        if let Some(m) = self.p.metot_bul(&pkg, &t, &ad, 0) {
                            let degerler = self.argumanlar(&args, f);
                            return self.fonksiyon_cagir(m, Some(Deger::Yapi(pkg, t)), degerler, n, f);
                        }
                    }
                    _ => {}
                }
                // Alıcısı bilinmeyen metot: router argümanı alıyorsa (`h.RotalariKaydet(api)`) projede tek tanımlı metot
                let degerler = self.argumanlar(&args, f);
                if degerler.iter().any(|d| matches!(d, Deger::Router(_))) {
                    if let Some(m) = self.p.tekil_metot(&ad) {
                        let alici = self.p.alici(m).and_then(|t| self.p.yapi_bul(m.0, &t)).map(|x| Deger::Yapi(x.2, x.3));
                        return self.fonksiyon_cagir(m, alici, degerler, n, f);
                    }
                }
                self.sarmalayici(degerler, n, f)
            }
            "identifier" => {
                let ad = metin(g, k);
                match self.ad_degeri(ad, f) {
                    Deger::Fonk(t) => {
                        let degerler = self.argumanlar(&args, f);
                        self.fonksiyon_cagir(t, None, degerler, n, f)
                    }
                    _ if ad == "new" => match args.first().and_then(|a| self.p.yapi_bul(f.di, metin(*a, k))) {
                        Some(x) if !x.4 => Deger::Yapi(x.2, x.3),
                        _ => Deger::Bilinmeyen,
                    },
                    _ => {
                        let degerler = self.argumanlar(&args, f);
                        if matches!(ad, "append") {
                            let mut v = vec![];
                            for d in degerler {
                                match d {
                                    Deger::Liste(l) => v.extend(l),
                                    x => v.push(x),
                                }
                            }
                            return Deger::Liste(v);
                        }
                        self.sarmalayici(degerler, n, f)
                    }
                }
            }
            "func_literal" => {
                let degerler = self.argumanlar(&args, f);
                self.fonksiyon_cagir((f.di, g), None, degerler, n, f)
            }
            _ => {
                // Zincirli çağrı (`handlers.CORS(...)(r)`, `cors.New(...).Handler(r)`)
                let _ = self.ifade(g, f);
                let degerler = self.argumanlar(&args, f);
                self.sarmalayici(degerler, n, f)
            }
        }
    }

    fn argumanlar(&mut self, args: &[Node<'a>], f: &mut Cerceve<'a>) -> Vec<Deger<'a>> {
        args.iter()
            .map(|a| match self.ifade(*a, f) {
                Deger::Bilinmeyen => match self.metin_degeri(*a, f, 0) {
                    Some(m) => Deger::Metin(m),
                    None => Deger::Ifade(f.di, *a),
                },
                v => v,
            })
            .collect()
    }

    /// Çözülemeyen çağrı router'ı argüman alıyorsa onu saran bir ara katmandır (`handlers.LoggingHandler(os.Stdout, r)`,
    /// `cors.Default().Handler(mux)`): router'ın bütün rotalarına uygulanır
    fn sarmalayici(&mut self, degerler: Vec<Deger<'a>>, n: Node<'a>, f: &mut Cerceve<'a>) -> Deger<'a> {
        let routerlar: Vec<usize> = degerler.iter().filter_map(|d| if let Deger::Router(r) = d { Some(*r) } else { None }).collect();
        let deyim = n.parent().is_some_and(|u| matches!(u.kind(), "expression_statement" | "go_statement" | "defer_statement"));
        if let ([r], false) = (routerlar.as_slice(), deyim) {
            let ara = self.ara_coz(n.child_by_field_name("function").unwrap_or(n), f);
            if !ara.metin.ends_with("Fatal") && !ara.metin.contains("log.") && !ara.metin.starts_with("fmt.") {
                self.routerlar[*r].ara.push((ara, 0));
            }
            return Deger::Router(*r);
        }
        Deger::Bilinmeyen
    }

    fn paket_cagrisi(&mut self, yol: &str, ad: &str, n: Node<'a>, args: &[Node<'a>], f: &mut Cerceve<'a>) -> Deger<'a> {
        let cati = cati_yolu(yol);
        let (di, sat) = (f.di, satir(n));
        let yeni = |s: &mut Self, c: Cati| Deger::Router(s.yeni_router(c, di, sat, false, ""));
        match (cati, ad) {
            (Some(Cati::Gin), "Default" | "New") => return yeni(self, Cati::Gin),
            (Some(Cati::Echo), "New") => return yeni(self, Cati::Echo),
            (Some(Cati::Chi), "NewRouter" | "NewMux") => return yeni(self, Cati::Chi),
            (Some(Cati::Fiber), "New") => return yeni(self, Cati::Fiber),
            (Some(Cati::Mux), "NewRouter") => return yeni(self, Cati::Mux),
            (Some(Cati::Http), "NewServeMux") => return yeni(self, Cati::Http),
            (Some(Cati::Http), "HandleFunc" | "Handle") => {
                let r = self.varsayilan(f);
                return self.router_metodu(r, ad, n, args, f);
            }
            (Some(Cati::Http), "ListenAndServe" | "ListenAndServeTLS" | "Serve" | "ServeTLS") => {
                let i = if ad == "ListenAndServeTLS" { 3 } else { 1 };
                match args.get(i).map(|a| (metin(*a, self.p.k(f.di)), *a)) {
                    Some(("nil", _)) | None => {
                        self.varsayilan(f);
                    }
                    Some((_, a)) => {
                        self.ifade(a, f);
                    }
                }
                return Deger::Bilinmeyen;
            }
            (Some(Cati::Http), "HandlerFunc" | "TimeoutHandler" | "MaxBytesHandler" | "AllowQuerySemicolons")
            | (Some(Cati::Gin), "WrapF" | "WrapH")
            | (Some(Cati::Echo), "WrapHandler") => {
                return args.first().map(|a| self.ifade(*a, f)).unwrap_or(Deger::Bilinmeyen);
            }
            (Some(Cati::Http), "StripPrefix") => {
                let onek = args.first().and_then(|a| self.metin_degeri(*a, f, 0)).unwrap_or_default();
                let ic = args.get(1).map(|a| self.ifade(*a, f)).unwrap_or(Deger::Bilinmeyen);
                return Deger::Soyulmus(onek, Box::new(ic));
            }
            _ => {}
        }
        if self.p.yerel_paketler.contains(yol) {
            if let Some(&t) = self.p.fonksiyonlar.get(&(yol.to_string(), ad.to_string())) {
                let degerler = self.argumanlar(args, f);
                return self.fonksiyon_cagir(t, None, degerler, n, f);
            }
            if self.p.tipler.contains_key(&(yol.to_string(), ad.to_string())) {
                // Tip dönüşümü: paket.Tip(x)
                return Deger::Yapi(yol.to_string(), ad.to_string());
            }
        }
        let degerler = self.argumanlar(args, f);
        match self.sarmalayici(degerler, n, f) {
            Deger::Bilinmeyen => Deger::Dis(yol.to_string(), ad.to_string()),
            v => v,
        }
    }

    /// Proje fonksiyonunu argüman değerleriyle yorumlar; dönüş değerini verir
    fn fonksiyon_cagir(&mut self, t: Tanim<'a>, alici: Option<Deger<'a>>, degerler: Vec<Deger<'a>>, n: Node<'a>, f: &mut Cerceve<'a>) -> Deger<'a> {
        let (tdi, dugum) = t;
        if self.yigin.contains(&dugum.id()) || f.derinlik >= DERINLIK || self.adim > ADIM_SINIRI {
            return Deger::Bilinmeyen;
        }
        // Router'ı işleyici parametresine alıp işleyici döndüren proje fonksiyonu bir sarmalayıcıdır
        if let Some(i) = degerler.iter().position(|d| matches!(d, Deger::Router(_))) {
            if self.p.isleyici_donduren(t) && self.p.param_tipi(t, i).is_some_and(|x| self.p.isleyici_tipi(t.0, &x)) {
                if let Deger::Router(r) = degerler[i] {
                    let mut ara = self.ara_coz(n.child_by_field_name("function").unwrap_or(n), f);
                    ara.fonk = Some(t);
                    self.routerlar[r].ara.push((ara, 0));
                    return Deger::Router(r);
                }
            }
        }
        let mut c = Cerceve::yeni(tdi, f.derinlik + 1);
        let params = self.p.parametreler(t);
        for (i, (ad, tip, degisken)) in params.iter().enumerate() {
            let mut v = if *degisken { Deger::Liste(degerler.get(i..).map(|x| x.to_vec()).unwrap_or_default()) } else { degerler.get(i).cloned().unwrap_or(Deger::Bilinmeyen) };
            // Değeri izlenemeyen router parametresi: bağlanmamış router (kayıtlar kaybolmasın)
            if matches!(v, Deger::Bilinmeyen | Deger::Ifade(..)) {
                if let Some(cati) = self.p.router_catisi(tdi, tip) {
                    v = Deger::Router(self.yeni_router(cati, tdi, satir(dugum), true, ad.as_deref().unwrap_or("router")));
                } else if let Some(x) = self.p.yapi_bul(tdi, tip).filter(|x| !x.4 && x.1.kind() == "struct_type") {
                    v = Deger::Yapi(x.2, x.3);
                }
            }
            if let Some(ad) = ad {
                c.ortam.insert(ad.clone(), v);
            }
        }
        if dugum.kind() == "method_declaration" {
            let k = self.p.k(tdi);
            let ad =
                dugum.child_by_field_name("receiver").and_then(|r| cocuklar(r).into_iter().next()).and_then(|p| p.child_by_field_name("name")).map(|x| metin(x, k).to_string());
            let alici = alici.or_else(|| self.p.alici(t).and_then(|tip| self.p.yapi_bul(tdi, &tip)).map(|x| Deger::Yapi(x.2, x.3)));
            if let (Some(ad), Some(v)) = (ad, alici) {
                c.ortam.insert(ad, v);
            }
        }
        c.kosullar = f.kosullar.clone();
        self.yigin.push(dugum.id());
        self.yorumlanan.insert(dugum.id());
        if let Some(govde) = dugum.child_by_field_name("body") {
            self.calistir(govde, &mut c);
        }
        self.yigin.pop();
        match c.donus {
            Some(Deger::Fonk(x)) if x.1.kind() == "func_literal" && self.p.isleyici_donduren(t) => Deger::Ifade(f.di, n),
            Some(v) => v,
            None => Deger::Bilinmeyen,
        }
    }

    /// Fonksiyon değeriyle çağrı (chi `Route("/x", func(r chi.Router) {...})`)
    fn deger_cagir(&mut self, fonk: Node<'a>, degerler: Vec<Deger<'a>>, n: Node<'a>, f: &mut Cerceve<'a>) {
        let t = match fonk.kind() {
            "func_literal" => Some((f.di, fonk)),
            _ => match self.ifade(fonk, f) {
                Deger::Fonk(t) => Some(t),
                _ => None,
            },
        };
        if let Some(t) = t {
            self.fonksiyon_cagir(t, None, degerler, n, f);
        }
    }

    // ---- Router metotları ----

    #[allow(clippy::too_many_arguments)]
    fn kaydet(
        &mut self,
        r: usize,
        yontemler: Vec<String>,
        yontem_acik: bool,
        yol: Option<Node<'a>>,
        isleyici: Option<Node<'a>>,
        ara: &[Node<'a>],
        n: Node<'a>,
        f: &mut Cerceve<'a>,
    ) -> Deger<'a> {
        let yol_metni = match yol {
            Some(y) => self.metin_degeri(y, f, 0),
            None => Some(String::new()),
        };
        let Some(yol_metni) = yol_metni else {
            let ifade = yol.map(|y| sikistir(metin(y, self.p.k(f.di)))).unwrap_or_default();
            self.dinamik.push(format!("{}:{} {ifade}", self.p.goreli(f.di), satir(n)));
            return Deger::Bilinmeyen;
        };
        self.kaydet_yolla(r, yontemler, yontem_acik, yol_metni, isleyici, ara, n, f)
    }

    #[allow(clippy::too_many_arguments)]
    fn kaydet_yolla(
        &mut self,
        r: usize,
        yontemler: Vec<String>,
        yontem_acik: bool,
        yol: String,
        isleyici: Option<Node<'a>>,
        ara: &[Node<'a>],
        n: Node<'a>,
        f: &mut Cerceve<'a>,
    ) -> Deger<'a> {
        let isl = isleyici.map(|i| self.isleyici_coz(i, f, 0)).unwrap_or_default();
        let mut aralar = vec![];
        for a in ara {
            aralar.extend(self.ara_listesi(*a, f));
        }
        let sira = self.sonraki();
        self.rotalar.push(RotaD { router: r, yontemler, yontem_acik, yol, isl, ara: aralar, di: f.di, satir: satir(n), sira, kosul: Self::kosul(f) });
        Deger::Rotalar(vec![self.rotalar.len() - 1])
    }

    /// Ara katman argümanı: değişken sayılı liste (`mws...`) açılır
    fn ara_listesi(&mut self, a: Node<'a>, f: &mut Cerceve<'a>) -> Vec<AraD<'a>> {
        if a.kind() == "variadic_argument" {
            if let Some(i) = a.named_child(0) {
                if let Deger::Liste(l) = self.ifade(i, f) {
                    return l.into_iter().filter_map(|d| self.ara_degerden(d, f)).collect();
                }
            }
        }
        vec![self.ara_coz(a, f)]
    }

    fn ara_degerden(&mut self, d: Deger<'a>, f: &mut Cerceve<'a>) -> Option<AraD<'a>> {
        match d {
            Deger::Ifade(di, e) => {
                if di == f.di {
                    Some(self.ara_coz(e, f))
                } else {
                    let mut c = Cerceve::yeni(di, f.derinlik);
                    Some(self.ara_coz(e, &mut c))
                }
            }
            Deger::Fonk(t) => Some(AraD {
                metin: self.p.ad(t).unwrap_or_else(|| "func".into()),
                fonk: Some(t),
                dis: None,
                ad: self.p.ad(t).unwrap_or_default(),
                argumanlar: vec![],
                ham: vec![],
                yol: None,
            }),
            Deger::Dis(y, a) => {
                Some(AraD { metin: a.clone(), fonk: None, dis: Some(y), ad: a.rsplit('.').next().unwrap_or(&a).to_string(), argumanlar: vec![], ham: vec![], yol: None })
            }
            _ => None,
        }
    }

    fn ara_ekle(&mut self, r: usize, args: &[Node<'a>], f: &mut Cerceve<'a>) {
        let sira = self.sonraki();
        for a in args {
            for x in self.ara_listesi(*a, f) {
                self.routerlar[r].ara.push((x, sira));
            }
        }
    }

    fn router_metodu(&mut self, r: usize, ad: &str, n: Node<'a>, args: &[Node<'a>], f: &mut Cerceve<'a>) -> Deger<'a> {
        let cati = self.routerlar[r].cati;
        let a = |i: usize| args.get(i).copied();
        let son_haric = |v: &[Node<'a>]| -> Vec<Node<'a>> { v.split_last().map(|(_, x)| x.to_vec()).unwrap_or_default() };
        match (cati, ad) {
            (Cati::Gin, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS" | "HEAD") => {
                let h = args.get(1..).map(son_haric).unwrap_or_default();
                self.kaydet(r, vec![ad.into()], true, a(0), args.last().copied().filter(|_| args.len() > 1), &h, n, f)
            }
            (Cati::Gin, "Any") => {
                let h = args.get(1..).map(son_haric).unwrap_or_default();
                self.kaydet(r, vec!["ALL".into()], true, a(0), args.last().copied().filter(|_| args.len() > 1), &h, n, f)
            }
            (Cati::Gin, "Handle") => {
                let y = a(0).map(|x| self.metin_listesi(x, f)).unwrap_or_default();
                let h = args.get(2..).map(son_haric).unwrap_or_default();
                self.kaydet(r, y.iter().map(|m| m.to_uppercase()).collect(), true, a(1), args.last().copied().filter(|_| args.len() > 2), &h, n, f)
            }
            (Cati::Gin, "Match") => {
                let y = a(0).map(|x| self.metin_listesi(x, f)).unwrap_or_default();
                let h = args.get(2..).map(son_haric).unwrap_or_default();
                self.kaydet(r, y.iter().map(|m| m.to_uppercase()).collect(), true, a(1), args.last().copied().filter(|_| args.len() > 2), &h, n, f)
            }
            (Cati::Echo, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS" | "HEAD" | "CONNECT" | "TRACE") => {
                let h = args.get(2..).unwrap_or(&[]).to_vec();
                self.kaydet(r, vec![ad.into()], true, a(0), a(1), &h, n, f)
            }
            (Cati::Echo, "Any") => {
                let h = args.get(2..).unwrap_or(&[]).to_vec();
                self.kaydet(r, vec!["ALL".into()], true, a(0), a(1), &h, n, f)
            }
            (Cati::Echo, "Add" | "Match") => {
                let y = a(0).map(|x| self.metin_listesi(x, f)).unwrap_or_default();
                let h = args.get(3..).unwrap_or(&[]).to_vec();
                self.kaydet(r, y.iter().map(|m| m.to_uppercase()).collect(), true, a(1), a(2), &h, n, f)
            }
            (Cati::Gin | Cati::Echo | Cati::Fiber, "Group") => {
                let onek = a(0).and_then(|x| self.metin_degeri(x, f, 0));
                let mut ara = vec![];
                for x in args.iter().skip(1) {
                    ara.extend(self.ara_listesi(*x, f));
                }
                match onek {
                    Some(onek) => Deger::Router(self.alt_router(r, onek, ara, n, f)),
                    None => Deger::Router(self.dinamik_alt(r, a(0), ara, n, f)),
                }
            }
            (Cati::Gin | Cati::Echo | Cati::Chi | Cati::Mux, "Use") | (Cati::Echo, "Pre") => {
                self.ara_ekle(r, args, f);
                Deger::Router(r)
            }
            (Cati::Gin, "Run" | "RunTLS" | "RunUnix" | "RunListener" | "Handler")
            | (Cati::Echo, "Start" | "StartTLS" | "StartAutoTLS" | "StartServer" | "StartH2CServer")
            | (Cati::Fiber, "Listen" | "ListenTLS" | "Listener" | "ListenMutualTLS") => Deger::Router(r),
            (Cati::Chi, "Get" | "Post" | "Put" | "Patch" | "Delete" | "Head" | "Options" | "Connect" | "Trace") => {
                self.kaydet(r, vec![ad.to_uppercase()], true, a(0), a(1), &[], n, f)
            }
            (Cati::Chi, "Method" | "MethodFunc") => {
                let y = a(0).map(|x| self.metin_listesi(x, f)).unwrap_or_default();
                self.kaydet(r, y.iter().map(|m| m.to_uppercase()).collect(), true, a(1), a(2), &[], n, f)
            }
            (Cati::Chi, "Handle" | "HandleFunc") => match a(1).map(|x| self.ifade(x, f)) {
                Some(Deger::Router(s)) => {
                    let onek = a(0).and_then(|x| self.metin_degeri(x, f, 0)).unwrap_or_default();
                    self.bagla_alt(r, s, onek.trim_end_matches("/*").trim_end_matches('*').to_string(), n, f);
                    Deger::Bilinmeyen
                }
                _ => self.kaydet(r, vec!["ALL".into()], false, a(0), a(1), &[], n, f),
            },
            (Cati::Chi, "Route") => {
                let alt = match a(0).and_then(|x| self.metin_degeri(x, f, 0)) {
                    Some(onek) => self.alt_router(r, onek, vec![], n, f),
                    None => self.dinamik_alt(r, a(0), vec![], n, f),
                };
                if let Some(fn_) = a(1) {
                    self.deger_cagir(fn_, vec![Deger::Router(alt)], n, f);
                }
                Deger::Router(alt)
            }
            (Cati::Chi, "Group") => {
                let alt = self.alt_router(r, String::new(), vec![], n, f);
                if let Some(fn_) = a(0).filter(|x| metin(*x, self.p.k(f.di)) != "nil") {
                    self.deger_cagir(fn_, vec![Deger::Router(alt)], n, f);
                }
                Deger::Router(alt)
            }
            (Cati::Chi, "With") => {
                let mut ara = vec![];
                for x in args {
                    ara.extend(self.ara_listesi(*x, f));
                }
                Deger::Router(self.alt_router(r, String::new(), ara, n, f))
            }
            (Cati::Chi, "Mount") | (Cati::Fiber, "Mount") => {
                let onek = a(0).and_then(|x| self.metin_degeri(x, f, 0)).unwrap_or_default();
                if let Some(Deger::Router(s)) = a(1).map(|x| self.ifade(x, f)) {
                    self.bagla_alt(r, s, onek, n, f);
                }
                Deger::Bilinmeyen
            }
            (Cati::Fiber, "Get" | "Post" | "Put" | "Patch" | "Delete" | "Head" | "Options" | "Connect" | "Trace" | "All") => {
                let y = if ad == "All" { "ALL".to_string() } else { ad.to_uppercase() };
                let h = args.get(1..).map(son_haric).unwrap_or_default();
                self.kaydet(r, vec![y], true, a(0), args.last().copied().filter(|_| args.len() > 1), &h, n, f)
            }
            (Cati::Fiber, "Add") => {
                let y = a(0).map(|x| self.metin_listesi(x, f)).unwrap_or_default();
                let h = args.get(2..).map(son_haric).unwrap_or_default();
                self.kaydet(r, y.iter().map(|m| m.to_uppercase()).collect(), true, a(1), args.last().copied().filter(|_| args.len() > 2), &h, n, f)
            }
            (Cati::Fiber, "Route") => {
                let Some(onek) = a(0).and_then(|x| self.metin_degeri(x, f, 0)) else { return Deger::Bilinmeyen };
                let alt = self.alt_router(r, onek, vec![], n, f);
                if let Some(fn_) = a(1) {
                    self.deger_cagir(fn_, vec![Deger::Router(alt)], n, f);
                }
                Deger::Router(alt)
            }
            (Cati::Fiber, "Use") => {
                let onek = a(0).and_then(|x| (matches!(x.kind(), "interpreted_string_literal" | "raw_string_literal")).then(|| self.metin_degeri(x, f, 0)).flatten());
                let bas = usize::from(onek.is_some());
                let sira = self.sonraki();
                for x in args.iter().skip(bas) {
                    match self.ifade(*x, f) {
                        Deger::Router(s) => self.bagla_alt(r, s, onek.clone().unwrap_or_default(), n, f),
                        _ => {
                            for mut ara in self.ara_listesi(*x, f) {
                                ara.yol = onek.clone().filter(|o| o != "/" && !o.is_empty());
                                self.routerlar[r].ara.push((ara, sira));
                            }
                        }
                    }
                }
                Deger::Router(r)
            }
            (Cati::Mux, "HandleFunc" | "Handle") => match a(1).map(|x| self.ifade(x, f)) {
                Some(Deger::Router(s)) => {
                    self.bagla_alt(r, s, String::new(), n, f);
                    Deger::Bilinmeyen
                }
                Some(Deger::Soyulmus(onek, ic)) if matches!(*ic, Deger::Router(_)) => {
                    if let Deger::Router(s) = *ic {
                        self.bagla_alt(r, s, onek, n, f);
                    }
                    Deger::Bilinmeyen
                }
                _ => self.kaydet(r, vec!["ALL".into()], false, a(0), a(1), &[], n, f),
            },
            (Cati::Mux, "Methods" | "Path" | "PathPrefix" | "Host" | "Schemes" | "Headers" | "Queries" | "MatcherFunc" | "HeadersRegexp" | "Name" | "NewRoute") => {
                self.taslaklar.push(Taslak { router: r, yol: None, onek: None, yontemler: vec![] });
                let t = self.taslaklar.len() - 1;
                self.taslak_metodu(t, ad, n, args, f)
            }
            (Cati::Mux, "StrictSlash" | "SkipClean" | "UseEncodedPath") => Deger::Router(r),
            (Cati::Http, "HandleFunc" | "Handle") => {
                let Some(desen) = a(0).and_then(|x| self.metin_degeri(x, f, 0)) else {
                    let ifade = a(0).map(|y| sikistir(metin(y, self.p.k(f.di)))).unwrap_or_default();
                    self.dinamik.push(format!("{}:{} {ifade}", self.p.goreli(f.di), satir(n)));
                    return Deger::Bilinmeyen;
                };
                // Go 1.22 deseni: "[YÖNTEM ][host]/yol"
                let (yontem, kalan) = match desen.split_once(' ') {
                    Some((m, y)) if m.chars().all(|c| c.is_ascii_uppercase()) => (Some(m.to_string()), y.trim().to_string()),
                    _ => (None, desen.clone()),
                };
                let yol = match kalan.find('/') {
                    Some(i) => kalan[i..].to_string(),
                    None => kalan,
                };
                if ad == "Handle" {
                    match a(1).map(|x| self.ifade(x, f)) {
                        Some(Deger::Router(s)) => {
                            self.bagla_alt(r, s, String::new(), n, f);
                            return Deger::Bilinmeyen;
                        }
                        Some(Deger::Soyulmus(onek, ic)) => {
                            if let Deger::Router(s) = *ic {
                                self.bagla_alt(r, s, onek, n, f);
                                return Deger::Bilinmeyen;
                            }
                        }
                        _ => {}
                    }
                }
                let acik = yontem.is_some();
                self.kaydet_yolla(r, yontem.into_iter().collect(), acik, yol, a(1), &[], n, f)
            }
            _ => {
                for x in args {
                    self.ifade(*x, f);
                }
                Deger::Bilinmeyen
            }
        }
    }

    /// gorilla/mux rota taslağı
    fn taslak_metodu(&mut self, t: usize, ad: &str, n: Node<'a>, args: &[Node<'a>], f: &mut Cerceve<'a>) -> Deger<'a> {
        match ad {
            "Methods" => {
                let mut y = vec![];
                for a in args {
                    y.extend(self.metin_listesi(*a, f).into_iter().map(|m| m.to_uppercase()));
                }
                self.taslaklar[t].yontemler = y;
            }
            "Path" => self.taslaklar[t].yol = args.first().and_then(|a| self.metin_degeri(*a, f, 0)),
            "PathPrefix" => self.taslaklar[t].onek = args.first().and_then(|a| self.metin_degeri(*a, f, 0)),
            "HandlerFunc" | "Handler" => {
                let r = self.taslaklar[t].router;
                let yol = self.taslaklar[t].yol.clone().or_else(|| self.taslaklar[t].onek.clone()).unwrap_or_default();
                if ad == "Handler" {
                    match args.first().map(|x| self.ifade(*x, f)) {
                        Some(Deger::Router(s)) => {
                            self.bagla_alt(r, s, String::new(), n, f);
                            return Deger::Bilinmeyen;
                        }
                        Some(Deger::Soyulmus(onek, ic)) if matches!(*ic, Deger::Router(_)) => {
                            if let Deger::Router(s) = *ic {
                                self.bagla_alt(r, s, onek, n, f);
                            }
                            return Deger::Bilinmeyen;
                        }
                        _ => {}
                    }
                }
                let y = self.taslaklar[t].yontemler.clone();
                let acik = !y.is_empty();
                return self.kaydet_yolla(r, y, acik, yol, args.first().copied(), &[], n, f);
            }
            "Subrouter" => {
                let r = self.taslaklar[t].router;
                let onek = self.taslaklar[t].onek.clone().or_else(|| self.taslaklar[t].yol.clone()).unwrap_or_default();
                return Deger::Router(self.alt_router(r, onek, vec![], n, f));
            }
            _ => {}
        }
        Deger::Taslak(t)
    }

    /// gorilla/mux kaydedilen rota: `.Methods("GET")` yöntemi belirler
    fn rota_metodu(&mut self, rs: Vec<usize>, ad: &str, args: &[Node<'a>], f: &mut Cerceve<'a>) -> Deger<'a> {
        if ad == "Methods" {
            let mut y = vec![];
            for a in args {
                y.extend(self.metin_listesi(*a, f).into_iter().map(|m| m.to_uppercase()));
            }
            for &i in &rs {
                if !y.is_empty() {
                    self.rotalar[i].yontemler = y.clone();
                    self.rotalar[i].yontem_acik = true;
                }
            }
        }
        Deger::Rotalar(rs)
    }

    // ---- İşleyici ve ara katman çözümü ----

    /// Çağrılan ifadenin projedeki tanımı (yorumlamadan)
    fn cagrilan_tanim(&mut self, g: Node<'a>, f: &mut Cerceve<'a>) -> Option<Tanim<'a>> {
        let k = self.p.k(f.di);
        match g.kind() {
            "identifier" => match self.ad_degeri(metin(g, k), f) {
                Deger::Fonk(t) => Some(t),
                _ => None,
            },
            "selector_expression" => {
                let o = g.child_by_field_name("operand")?;
                let alan = metin(g.child_by_field_name("field")?, k).to_string();
                if o.kind() == "identifier" && !f.ortam.contains_key(metin(o, k)) {
                    if let Some(yol) = self.p.iceri[f.di].get(metin(o, k)) {
                        return self.p.fonksiyonlar.get(&(yol.clone(), alan)).copied();
                    }
                }
                match self.ifade(o, f) {
                    Deger::Yapi(pkg, t) => self.p.metot_bul(&pkg, &t, &alan, 0),
                    Deger::Dis(..) => None,
                    _ => self.p.tekil_metot(&alan),
                }
            }
            "func_literal" => Some((f.di, g)),
            "parenthesized_expression" => self.cagrilan_tanim(g.named_child(0)?, f),
            _ => None,
        }
    }

    fn isleyici_coz(&mut self, n: Node<'a>, f: &mut Cerceve<'a>, derinlik: usize) -> Isleyici<'a> {
        if derinlik > 6 {
            return Isleyici::default();
        }
        match n.kind() {
            "func_literal" => Isleyici { tanim: Some((f.di, n)), ..Default::default() },
            "parenthesized_expression" | "unary_expression" => n.named_child(0).map(|i| self.isleyici_coz(i, f, derinlik + 1)).unwrap_or_default(),
            "call_expression" => self.isleyici_cagrisi(n, f, derinlik),
            _ => {
                let v = self.ifade(n, f);
                let m = sikistir(metin(n, self.p.k(f.di)));
                self.isleyici_degerden(v, &m, f, derinlik)
            }
        }
    }

    fn adli_isleyici(&self, t: Tanim<'a>) -> Isleyici<'a> {
        Isleyici { tanim: Some(t), adli: Some(t), ad: self.p.ad(t), alici: self.p.alici(t), ara: vec![] }
    }

    fn isleyici_degerden(&mut self, v: Deger<'a>, m: &str, f: &mut Cerceve<'a>, derinlik: usize) -> Isleyici<'a> {
        match v {
            Deger::Fonk(t) if t.1.kind() == "func_literal" => Isleyici { tanim: Some(t), ..Default::default() },
            Deger::Fonk(t) => self.adli_isleyici(t),
            Deger::Ifade(di, e) => {
                if di == f.di {
                    self.isleyici_coz(e, f, derinlik + 1)
                } else {
                    let mut c = Cerceve::yeni(di, f.derinlik);
                    self.isleyici_coz(e, &mut c, derinlik + 1)
                }
            }
            _ => {
                // Alıcısının tipi bilinmeyen metot değeri: projede tek tanımlı metot
                if let Some((_, son)) = m.rsplit_once('.') {
                    if let Some(t) = self.p.tekil_metot(son) {
                        return self.adli_isleyici(t);
                    }
                }
                Isleyici { ad: m.rsplit('.').next().map(String::from).filter(|s| !s.is_empty()), ..Default::default() }
            }
        }
    }

    fn isleyici_cagrisi(&mut self, n: Node<'a>, f: &mut Cerceve<'a>, derinlik: usize) -> Isleyici<'a> {
        let k = self.p.k(f.di);
        let Some(g) = n.child_by_field_name("function") else { return Isleyici::default() };
        let args: Vec<Node<'a>> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
        let gm = sikistir(metin(g, k));
        let dis = g.child_by_field_name("operand").filter(|o| o.kind() == "identifier").and_then(|o| self.p.iceri[f.di].get(metin(o, k)).cloned());
        let son = gm.rsplit('.').next().unwrap_or("");
        let donusum = match dis.as_deref().map(cati_yolu) {
            Some(Some(Cati::Http)) => son == "HandlerFunc",
            Some(Some(Cati::Gin)) => matches!(son, "WrapF" | "WrapH"),
            Some(Some(Cati::Echo)) => son == "WrapHandler",
            _ => dis.as_deref().is_some_and(|d| d.contains("gofiber") && d.ends_with("adaptor")),
        };
        if donusum {
            if let Some(a) = args.first() {
                return self.isleyici_coz(*a, f, derinlik + 1);
            }
        }
        let cagrilan = self.cagrilan_tanim(g, f);
        // Sarmalayıcı: işleyici tipli parametreye işleyici veren argüman (`auth(http.HandlerFunc(h))`)
        for (i, a) in args.iter().enumerate() {
            let isleyici_param = match cagrilan {
                Some(t) => self.p.param_tipi(t, i).is_some_and(|x| self.p.isleyici_tipi(t.0, &x)),
                None => matches!(a.kind(), "identifier" | "selector_expression" | "call_expression" | "func_literal"),
            };
            if !isleyici_param {
                continue;
            }
            let mut ic = self.isleyici_coz(*a, f, derinlik + 1);
            if ic.tanim.is_some() {
                let mut ara = self.ara_coz(n, f);
                ara.metin = gm.clone();
                if let Some(t) = cagrilan {
                    ara.fonk = Some(t);
                }
                ic.ara.insert(0, ara);
                return ic;
            }
        }
        if let Some(t) = cagrilan {
            // Fabrika: döndürdüğü kapanış işleyicidir (`s.handleIndex()`)
            let mut isl = self.adli_isleyici(t);
            if let Some(kapanis) = self.p.donen_kapanis(t) {
                isl.tanim = Some(kapanis);
            }
            return isl;
        }
        Isleyici { ad: Some(son.to_string()).filter(|s| !s.is_empty()), ..Default::default() }
    }

    /// Ara katman ifadesinin çözümü
    fn ara_coz(&mut self, n: Node<'a>, f: &mut Cerceve<'a>) -> AraD<'a> {
        let k = self.p.k(f.di);
        let m = sikistir(metin(n, k));
        let mut a = AraD { metin: m.clone(), fonk: None, dis: None, ad: String::new(), argumanlar: vec![], ham: vec![], yol: None };
        match n.kind() {
            "call_expression" => {
                let g = n.child_by_field_name("function");
                let args: Vec<Node<'a>> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
                for x in args {
                    a.ham.push(sikistir(metin(x, k)));
                    let liste = self.metin_listesi(x, f);
                    a.argumanlar.extend(liste);
                }
                if let Some(g) = g {
                    a.ad = sikistir(metin(g, k)).rsplit('.').next().unwrap_or("").to_string();
                    a.fonk = self.cagrilan_tanim(g, f);
                    if a.fonk.is_none() {
                        a.dis = self.dis_kok(g, f, 0);
                    }
                }
            }
            "identifier" | "selector_expression" => {
                a.ad = m.rsplit('.').next().unwrap_or("").to_string();
                match self.ifade(n, f) {
                    Deger::Fonk(t) => a.fonk = Some(t),
                    Deger::Dis(y, _) => a.dis = Some(y),
                    Deger::Ifade(di, e) if e.id() != n.id() => {
                        let mut ic = if di == f.di {
                            self.ara_coz(e, f)
                        } else {
                            let mut c = Cerceve::yeni(di, f.derinlik);
                            self.ara_coz(e, &mut c)
                        };
                        ic.metin = m;
                        return ic;
                    }
                    _ => {}
                }
            }
            "func_literal" => a.fonk = Some((f.di, n)),
            _ => {}
        }
        a
    }

    /// Çağrı zincirinin başındaki dış paket (`authMiddleware.MiddlewareFunc()` -> gin-jwt yolu)
    fn dis_kok(&mut self, g: Node<'a>, f: &mut Cerceve<'a>, derinlik: usize) -> Option<String> {
        if derinlik > 6 {
            return None;
        }
        let k = self.p.k(f.di);
        match g.kind() {
            "identifier" => {
                if let Some(v) = f.ortam.get(metin(g, k)).cloned() {
                    return match v {
                        Deger::Dis(y, _) => Some(y),
                        Deger::Ifade(di, e) if di == f.di && e.id() != g.id() => self.dis_kok(e, f, derinlik + 1),
                        _ => None,
                    };
                }
                if let Some(y) = self.p.iceri[f.di].get(metin(g, k)) {
                    return Some(y.clone());
                }
                match self.ad_degeri(metin(g, k), f) {
                    Deger::Dis(y, _) => Some(y),
                    _ => None,
                }
            }
            "selector_expression" => self.dis_kok(g.child_by_field_name("operand")?, f, derinlik + 1),
            "call_expression" => self.dis_kok(g.child_by_field_name("function")?, f, derinlik + 1),
            "unary_expression" | "parenthesized_expression" => self.dis_kok(g.named_child(0)?, f, derinlik + 1),
            _ => None,
        }
    }

    // ---- Sürücü ----

    fn yorumla(&mut self) {
        self.ana_mod = true;
        for t in self.p.girisler.clone() {
            let mut c = Cerceve::yeni(t.0, 0);
            self.fonksiyon_cagir(t, None, vec![], t.1, &mut c);
        }
        self.ana_mod = false;
        // Hiçbir girişten ulaşılmayan rota fonksiyonları: router parametresi alan, router kuran ya da alanında router
        // tutan yapının metotları. Router'ları bağlanmamış sayılır
        let mut adaylar: Vec<Tanim<'a>> = self.p.fonksiyonlar.values().chain(self.p.metotlar.values()).copied().collect();
        adaylar.sort_by_key(|(di, n)| (self.p.dosyalar[*di].yol.clone(), n.start_byte()));
        for t in adaylar {
            if self.yorumlanan.contains(&t.1.id()) || !self.rota_fonksiyonu_mu(t) {
                continue;
            }
            let mut c = Cerceve::yeni(t.0, 0);
            self.fonksiyon_cagir(t, None, vec![], t.1, &mut c);
        }
    }

    fn rota_fonksiyonu_mu(&self, t: Tanim<'a>) -> bool {
        let k = self.p.k(t.0);
        let govde = t.1.child_by_field_name("body").map(|b| metin(b, k)).unwrap_or("");
        if self.p.parametreler(t).iter().any(|(_, tip, _)| self.p.router_catisi(t.0, tip).is_some()) {
            return true;
        }
        if [".NewRouter(", ".NewServeMux(", "gin.Default(", "gin.New(", "echo.New(", "fiber.New(", "http.HandleFunc(", "http.Handle(", "chi.NewMux("]
            .iter()
            .any(|x| govde.contains(x))
        {
            return true;
        }
        // Alanında router tutan yapının metodu
        if let Some(tip) = self.p.alici(t) {
            let pkg = &self.p.paket[t.0];
            if let Some(&(sdi, spec)) = self.p.tipler.get(&(pkg.clone(), tip)) {
                if let Some(st) = spec.child_by_field_name("type").filter(|x| x.kind() == "struct_type") {
                    let sk = self.p.k(sdi);
                    return struct_alanlari(st).iter().any(|fd| fd.child_by_field_name("type").is_some_and(|ty| self.p.router_catisi(sdi, metin(ty, sk)).is_some()));
                }
            }
        }
        false
    }
}

// ---- İşleyici analizi ----

/// İşleyicinin gövdesinden okunanlar
#[derive(Clone, Default)]
struct Analiz {
    govde: Vec<Girdi>,
    sorgu: BTreeMap<String, Alan>,
    form: BTreeMap<String, Alan>,
    /// `r.FormValue("x")`: GET'te sorgu, diğerlerinde form gövdesi
    form_ya_da_sorgu: BTreeMap<String, Alan>,
    /// Echo `c.Bind(&x)` gövde alanları: yalnızca gövdeli yöntemlerde
    bind_govde: Vec<Girdi>,
    yol_tipleri: HashMap<String, String>,
    yanit: Option<Yanit>,
    notlar: Vec<String>,
    yontemler: Vec<String>,
}

/// Çatıların istek bağlamı üzerindeki bağlama metotları
const CATI_BAGLAMA: [&str; 25] = [
    "ShouldBindJSON",
    "BindJSON",
    "ShouldBind",
    "Bind",
    "ShouldBindBodyWithJSON",
    "ShouldBindYAML",
    "ShouldBindXML",
    "ShouldBindTOML",
    "MustBindWith",
    "ShouldBindWith",
    "BindWith",
    "ShouldBindBodyWith",
    "ShouldBindQuery",
    "BindQuery",
    "ShouldBindUri",
    "BindUri",
    "BindBody",
    "BindQueryParams",
    "BindPathParams",
    "BodyParser",
    "QueryParser",
    "ParamsParser",
    "Body",
    "JSON",
    "Query",
];

/// Çatının bağlama çağrısı: (hedef argümanın sırası, bölüm, ad etiketi, kural etiketi, çatı doğrular mı).
/// `arg(i)`: i. argümanın metni
fn baglama_turu(cati: Cati, son: &str, gm: &str, n: usize, arg: impl Fn(usize) -> Option<String>) -> Option<(usize, &'static str, &'static str, Option<&'static str>, bool)> {
    Some(match (cati, son) {
        (Cati::Gin, "ShouldBindJSON" | "BindJSON" | "ShouldBind" | "Bind" | "ShouldBindBodyWithJSON" | "ShouldBindYAML" | "ShouldBindXML" | "ShouldBindTOML" | "MustBindWith") => {
            (0, "govde", "json", Some("binding"), true)
        }
        (Cati::Gin, "ShouldBindWith" | "BindWith" | "ShouldBindBodyWith") => {
            let tur = arg(1).unwrap_or_default();
            if tur.contains("Query") {
                (0, "sorgu", "form", Some("binding"), true)
            } else if tur.contains("Uri") {
                (0, "yol", "uri", Some("binding"), true)
            } else if tur.contains("Form") {
                (0, "form", "form", Some("binding"), true)
            } else {
                (0, "govde", "json", Some("binding"), true)
            }
        }
        (Cati::Gin, "ShouldBindQuery" | "BindQuery") => (0, "sorgu", "form", Some("binding"), true),
        (Cati::Gin, "ShouldBindUri" | "BindUri") => (0, "yol", "uri", Some("binding"), true),
        (Cati::Echo, "Bind") if n == 1 => (0, "hepsi", "json", Some("validate"), false),
        (Cati::Echo, "BindBody") => (n.checked_sub(1)?, "govde", "json", Some("validate"), false),
        (Cati::Echo, "BindQueryParams") => (n.checked_sub(1)?, "sorgu", "query", Some("validate"), false),
        (Cati::Echo, "BindPathParams") => (n.checked_sub(1)?, "yol", "param", Some("validate"), false),
        (Cati::Fiber, "BodyParser") => (0, "govde", "json", Some("validate"), false),
        (Cati::Fiber, "QueryParser") => (0, "sorgu", "query", Some("validate"), false),
        (Cati::Fiber, "ParamsParser") => (0, "yol", "params", Some("validate"), false),
        (Cati::Fiber, "Body" | "JSON") if gm.ends_with("Bind().Body") || gm.ends_with("Bind().JSON") => (0, "govde", "json", Some("validate"), false),
        (Cati::Fiber, "Query") if gm.ends_with("Bind().Query") => (0, "sorgu", "query", Some("validate"), false),
        (_, "Decode") if gm.contains("NewDecoder(") => (0, "govde", "json", Some("validate"), false),
        (_, "Unmarshal") if gm == "json.Unmarshal" => (1, "govde", "json", Some("validate"), false),
        (_, "Bind" | "DecodeJSON") if gm.starts_with("render.") => (1, "govde", "json", Some("validate"), false),
        _ => return None,
    })
}

/// Bağlama hedefi: (değişken, bölüm, ad etiketi, kural etiketi, çatı doğrular mı)
type Baglama = (String, &'static str, &'static str, Option<&'static str>, bool);

struct Cozumleyici<'a, 'p> {
    p: &'p Proje<'a>,
    analizler: HashMap<usize, Analiz>,
}

/// Sorgu / form alanı (kullanımdan)
fn kullanim_alani(tip: &str, zorunlu: bool, belirsiz: bool, satir: usize) -> Alan {
    let (t, k) = match tip {
        "int" => ("int", "IsInt()"),
        "number" => ("number", "IsNumber()"),
        "boolean" => ("boolean", "IsBoolean()"),
        _ => ("string", "IsString()"),
    };
    Alan { tip: Some(t.into()), zorunlu, belirsiz, kurallar: vec![k.into()], satir, kalitimdan: None, degerler: vec![], ic: None }
}

/// go-playground/validator etiketi -> kurallar (class-validator adlarıyla), zorunlu, koşullu zorunluluk, izinli değerler
fn dogrulama_kurallari(etiket: &str, tip: &str) -> (Vec<String>, bool, bool, Vec<String>) {
    let (mut v, mut zorunlu, mut kosullu, mut degerler) = (vec![], false, false, vec![]);
    for parca in etiket.split(',') {
        let (ad, deger) = parca.split_once('=').unwrap_or((parca, ""));
        let sayi = deger.parse::<f64>().ok();
        match (ad.trim(), sayi) {
            ("required", _) => zorunlu = true,
            ("required_if" | "required_unless" | "required_with" | "required_without" | "required_with_all" | "required_without_all", _) => kosullu = true,
            ("omitempty" | "" | "-", _) => {}
            ("dive", _) => break,
            ("min" | "gte", Some(n)) => v.push(match tip {
                "string" => format!("MinLength({n})"),
                "array" => format!("ArrayMinSize({n})"),
                _ => format!("Min({n})"),
            }),
            ("max" | "lte", Some(n)) => v.push(match tip {
                "string" => format!("MaxLength({n})"),
                "array" => format!("ArrayMaxSize({n})"),
                _ => format!("Max({n})"),
            }),
            ("len", Some(n)) => match tip {
                "string" => v.extend([format!("MinLength({n})"), format!("MaxLength({n})")]),
                "array" => v.extend([format!("ArrayMinSize({n})"), format!("ArrayMaxSize({n})")]),
                _ => v.push(parca.to_string()),
            },
            ("gt", Some(n)) => v.push(match tip {
                "string" => format!("MinLength({})", n + 1.0),
                "array" => format!("ArrayMinSize({})", n + 1.0),
                "int" => format!("Min({})", n + 1.0),
                _ => format!("gt({n})"),
            }),
            ("lt", Some(n)) => v.push(match tip {
                "string" => format!("MaxLength({})", n - 1.0),
                "array" => format!("ArrayMaxSize({})", n - 1.0),
                "int" => format!("Max({})", n - 1.0),
                _ => format!("lt({n})"),
            }),
            ("email", _) => v.push("IsEmail()".into()),
            ("url" | "uri" | "http_url", _) => v.push("IsUrl()".into()),
            ("uuid" | "uuid4" | "uuid_rfc4122" | "uuid4_rfc4122", _) => v.push("IsUUID()".into()),
            ("oneof", _) => {
                degerler = deger.split_whitespace().map(|x| x.trim_matches('\'').to_string()).collect();
                v.push(format!("IsIn([{}])", degerler.iter().map(|d| format!("'{d}'")).collect::<Vec<_>>().join(", ")));
            }
            ("alpha", _) => v.push("Matches(/^[a-zA-Z]+$/)".into()),
            ("alphanum", _) => v.push("Matches(/^[a-zA-Z0-9]+$/)".into()),
            ("numeric", _) => v.push("Matches(/^[-+]?[0-9]+(?:\\.[0-9]+)?$/)".into()),
            ("number", _) => v.push("Matches(/^[0-9]+$/)".into()),
            _ => v.push(parca.trim().to_string()),
        }
    }
    (v, zorunlu, kosullu, degerler)
}

impl<'a, 'p> Cozumleyici<'a, 'p> {
    /// Yerel değişkenlerin tipleri: parametreler, `var x T`, değişmez atamaları, `new(T)`, çağrı sonuçları, range
    fn yerel_tipler(&self, t: Tanim<'a>) -> Tipler<'a> {
        let (di, fnode) = t;
        let k = self.p.k(di);
        let mut tipler: Tipler<'a> = HashMap::new();
        let mut yigin = vec![fnode];
        let mut sirali = vec![];
        while let Some(n) = yigin.pop() {
            sirali.push(n);
            let mut c = cocuklar(n);
            c.reverse();
            yigin.extend(c.into_iter().filter(|c| (c.kind() != "function_declaration" && c.kind() != "method_declaration") || c.id() == fnode.id()));
        }
        for n in sirali {
            match n.kind() {
                "parameter_declaration" | "variadic_parameter_declaration" => {
                    if let Some(ty) = n.child_by_field_name("type") {
                        for a in adli_cocuklar(n, "name") {
                            tipler.insert(metin(a, k).to_string(), (di, sikistir(metin(ty, k)), (ty.kind() == "struct_type").then_some(ty)));
                        }
                    }
                }
                "var_spec" => {
                    let degerler: Vec<Node> = n.child_by_field_name("value").map(cocuklar).unwrap_or_default();
                    for (i, a) in adli_cocuklar(n, "name").into_iter().enumerate() {
                        let tip = match n.child_by_field_name("type") {
                            Some(ty) => Some((di, sikistir(metin(ty, k)), (ty.kind() == "struct_type").then_some(ty))),
                            None => degerler.get(i).and_then(|d| self.ifade_tipi(di, *d, &tipler, 0)),
                        };
                        if let Some(t) = tip {
                            tipler.insert(metin(a, k).to_string(), t);
                        }
                    }
                }
                "short_var_declaration" | "assignment_statement" => {
                    let sol: Vec<Node> = n.child_by_field_name("left").map(cocuklar).unwrap_or_default();
                    let sag: Vec<Node> = n.child_by_field_name("right").map(cocuklar).unwrap_or_default();
                    if sol.len() == sag.len() {
                        for (l, r) in sol.iter().zip(&sag) {
                            if l.kind() == "identifier" {
                                if let Some(t) = self.ifade_tipi(di, *r, &tipler, 0) {
                                    tipler.insert(metin(*l, k).to_string(), t);
                                }
                            }
                        }
                    } else if let [r] = sag.as_slice() {
                        let sonuclar = self.cagri_sonuclari(di, *r, &tipler);
                        for (l, t) in sol.iter().zip(sonuclar) {
                            if l.kind() == "identifier" {
                                tipler.insert(metin(*l, k).to_string(), (t.0, t.1, None));
                            }
                        }
                    }
                }
                "range_clause" => {
                    let sol: Vec<Node> = n.child_by_field_name("left").map(cocuklar).unwrap_or_default();
                    if let (Some(v), Some(r)) = (sol.get(1), n.child_by_field_name("right")) {
                        if let Some((tdi, t, _)) = self.ifade_tipi(di, r, &tipler, 0) {
                            let oge = t.strip_prefix("[]").map(String::from).or_else(|| t.starts_with("map[").then(|| t[t.find(']').unwrap_or(0) + 1..].to_string()));
                            if let Some(o) = oge {
                                tipler.insert(metin(*v, k).to_string(), (tdi, o, None));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        tipler
    }

    /// Çağrının sonuç tipleri: (tipin dosyası, metni)
    fn cagri_sonuclari(&self, di: usize, n: Node<'a>, tipler: &Tipler<'a>) -> Vec<(usize, String)> {
        let n = if n.kind() == "unary_expression" { n.named_child(0).unwrap_or(n) } else { n };
        if n.kind() != "call_expression" {
            return vec![];
        }
        match self.cagri_tanimi(di, n, tipler, true) {
            Some(t) => self.p.sonuc_tipleri(t.0, t.1).into_iter().map(|s| (t.0, s)).collect(),
            None => {
                // Arayüz metodu: bildirimdeki sonuçlar
                let k = self.p.k(di);
                let ad = n.child_by_field_name("function").filter(|g| g.kind() == "selector_expression").and_then(|g| g.child_by_field_name("field")).map(|a| metin(a, k));
                match ad.and_then(|a| self.p.arayuz_metotlari.get(a)).map(Vec::as_slice) {
                    Some([(mdi, m)]) => self.p.sonuc_tipleri(*mdi, *m).into_iter().map(|s| (*mdi, s)).collect(),
                    _ => vec![],
                }
            }
        }
    }

    /// İfadenin statik tipi (biliniyorsa): (tip metninin dosyası, metin, satır içi struct)
    fn ifade_tipi(&self, di: usize, n: Node<'a>, tipler: &Tipler<'a>, derinlik: usize) -> Option<(usize, String, Option<Node<'a>>)> {
        if derinlik > 6 {
            return None;
        }
        let k = self.p.k(di);
        match n.kind() {
            "composite_literal" => {
                let t = n.child_by_field_name("type")?;
                Some((di, sikistir(metin(t, k)), (t.kind() == "struct_type").then_some(t)))
            }
            "unary_expression" | "parenthesized_expression" => self.ifade_tipi(di, n.named_child(0)?, tipler, derinlik + 1),
            "identifier" => tipler.get(metin(n, k)).cloned().or_else(|| {
                // Paket düzeyi değişken: bildirilen tipi ya da ilk değerinin tipi
                let &(vdi, deger, tip) = self.p.degerler.get(&(self.p.paket[di].clone(), metin(n, k).to_string()))?;
                match tip {
                    Some(t) => Some((vdi, sikistir(metin(t, self.p.k(vdi))), None)),
                    None => self.ifade_tipi(vdi, deger?, &HashMap::new(), derinlik + 1),
                }
            }),
            "type_assertion_expression" => n.child_by_field_name("type").map(|t| (di, sikistir(metin(t, k)), None)),
            "index_expression" => {
                let (tdi, t, _) = self.ifade_tipi(di, n.child_by_field_name("operand")?, tipler, derinlik + 1)?;
                t.strip_prefix("[]").map(|o| (tdi, o.to_string(), None))
            }
            "selector_expression" => {
                let (tdi, t, _) = self.ifade_tipi(di, n.child_by_field_name("operand")?, tipler, derinlik + 1)?;
                let (_, _, pkg, ad, _) = self.p.yapi_bul(tdi, &t)?;
                let alan = metin(n.child_by_field_name("field")?, k);
                self.p.alan_tipi(&pkg, &ad, alan, 0).map(|(a, b)| (a, b, None))
            }
            "call_expression" => {
                let g = n.child_by_field_name("function")?;
                let gm = metin(g, k);
                let args = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
                if gm == "new" || gm == "make" {
                    return args.first().map(|a| (di, sikistir(metin(*a, k)), None));
                }
                self.cagri_sonuclari(di, n, tipler).into_iter().next().map(|(a, b)| (a, b, None))
            }
            _ => None,
        }
    }

    /// Çağrının projedeki tanımı: paket fonksiyonu, tipi bilinen alıcının metodu, tek tanımlı metot
    fn cagri_tanimi(&self, di: usize, n: Node<'a>, tipler: &Tipler<'a>, tekil: bool) -> Option<Tanim<'a>> {
        let k = self.p.k(di);
        let g = n.child_by_field_name("function")?;
        match g.kind() {
            "identifier" => self.p.fonksiyonlar.get(&(self.p.paket[di].clone(), metin(g, k).to_string())).copied(),
            "selector_expression" => {
                let o = g.child_by_field_name("operand")?;
                let alan = metin(g.child_by_field_name("field")?, k);
                if o.kind() == "identifier" && !tipler.contains_key(metin(o, k)) {
                    if let Some(yol) = self.p.iceri[di].get(metin(o, k)) {
                        return self.p.fonksiyonlar.get(&(yol.clone(), alan.to_string())).copied();
                    }
                }
                if let Some((tdi, t, _)) = self.ifade_tipi(di, o, tipler, 0) {
                    if let Some((_, tn, pkg, ad, false)) = self.p.yapi_bul(tdi, &t) {
                        if let Some(m) = self.p.metot_bul(&pkg, &ad, alan, 0) {
                            return Some(m);
                        }
                        // Arayüz tipli alan: tek uygulayan metot
                        if tn.kind() == "interface_type" {
                            return self.p.tekil_metot(alan);
                        }
                        return None;
                    }
                    if self.p.yapi_bul(tdi, &t).is_none() && t.contains('.') {
                        // Dış paketin tipi (gorm.DB, sql.DB): projede değildir
                        let on = t.trim_start_matches('*').split('.').next().unwrap_or("");
                        if self.p.iceri[tdi].get(on).is_some_and(|y| !self.p.yerel_paketler.contains(y)) {
                            return None;
                        }
                    }
                }
                if tekil {
                    self.p.tekil_metot(alan)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Bağlama hedefi değişkeni: `&req` / `req`
    fn hedef(n: Option<Node<'a>>, k: &[u8]) -> Option<String> {
        let n = n?;
        let n = if n.kind() == "unary_expression" { n.child_by_field_name("operand")? } else { n };
        (n.kind() == "identifier").then(|| metin(n, k).to_string())
    }

    /// Struct alanları -> şema alanları. `ad_etiketi`: adın okunduğu etiket (json, form, query, uri, param);
    /// `kural_etiketi`: doğrulama kuralları (binding, validate); `dogrulanir`: çatı ya da işleyici doğruluyor mu.
    /// `suz`: yalnızca bu etiketi taşıyan alanlar (Echo Bind'da yol / sorgu / gövde ayrımı); `haric`: bu etiketlerden
    /// birini taşıyan alanlar atlanır
    #[allow(clippy::too_many_arguments)]
    fn sema_alanlari(
        &self,
        di: usize,
        st: Node<'a>,
        ad_etiketi: &str,
        kural_etiketi: Option<&str>,
        dogrulanir: bool,
        suz: bool,
        haric: &[&str],
        derinlik: usize,
        ziyaret: &mut Vec<usize>,
    ) -> BTreeMap<String, Alan> {
        let k = self.p.k(di);
        let mut out = BTreeMap::new();
        if derinlik > 4 || ziyaret.contains(&st.id()) {
            return out;
        }
        ziyaret.push(st.id());
        for fd in struct_alanlari(st) {
            let Some(tip) = fd.child_by_field_name("type") else { continue };
            let et = fd.child_by_field_name("tag").and_then(|t| metin_degismezi(t, k)).map(|t| etiketler(&t)).unwrap_or_default();
            if haric.iter().any(|h| et.contains_key(*h)) {
                continue;
            }
            if suz && !et.contains_key(ad_etiketi) {
                // Gömülü yapının alanları yine aranır
                if adli_cocuklar(fd, "name").is_empty() {
                    if let Some((sdi, sn, _, _, false)) = self.p.yapi_bul(di, metin(tip, k)) {
                        if sn.kind() == "struct_type" {
                            out.extend(self.sema_alanlari(sdi, sn, ad_etiketi, kural_etiketi, dogrulanir, suz, haric, derinlik + 1, ziyaret));
                        }
                    }
                }
                continue;
            }
            let ad_et = et.get(ad_etiketi).map(|e| e.split(',').next().unwrap_or("").to_string()).unwrap_or_default();
            if ad_et == "-" || et.get("binding").is_some_and(|b| b == "-") {
                continue;
            }
            let adlar = adli_cocuklar(fd, "name");
            if adlar.is_empty() {
                if ad_et.is_empty() {
                    if let Some((sdi, sn, _, _, false)) = self.p.yapi_bul(di, metin(tip, k)) {
                        if sn.kind() == "struct_type" {
                            out.extend(self.sema_alanlari(sdi, sn, ad_etiketi, kural_etiketi, dogrulanir, suz, haric, derinlik + 1, ziyaret));
                        }
                    }
                    continue;
                }
                let alan = self.alan_kur(di, tip, kural_etiketi, kural_etiketi.and_then(|e| et.get(e)).map(String::as_str), dogrulanir, satir(fd), derinlik, ziyaret, ad_etiketi);
                out.insert(ad_et, alan);
                continue;
            }
            for a in adlar {
                let ad = metin(a, k);
                if !ad.chars().next().is_some_and(char::is_uppercase) {
                    continue;
                }
                let anahtar = if ad_et.is_empty() { ad.to_string() } else { ad_et.clone() };
                let kural = kural_etiketi.and_then(|e| et.get(e)).map(String::as_str);
                let alan = self.alan_kur(di, tip, kural_etiketi, kural, dogrulanir, satir(fd), derinlik, ziyaret, ad_etiketi);
                out.insert(anahtar, alan);
            }
        }
        ziyaret.pop();
        out
    }

    /// Go tipinden alan: tip, kurallar, zorunluluk, iç içe yapı
    #[allow(clippy::too_many_arguments)]
    fn alan_kur(
        &self,
        di: usize,
        tip: Node<'a>,
        kural_etiketi: Option<&str>,
        kural: Option<&str>,
        dogrulanir: bool,
        satir: usize,
        derinlik: usize,
        ziyaret: &mut Vec<usize>,
        ad_etiketi: &str,
    ) -> Alan {
        let k = self.p.k(di);
        let mut t = tip;
        let mut nullable = false;
        while t.kind() == "pointer_type" || t.kind() == "parenthesized_type" {
            nullable |= t.kind() == "pointer_type";
            let Some(i) = t.named_child(0) else { break };
            t = i;
        }
        let mut kurallar: Vec<String> = vec![];
        let mut ic = None;
        let tip_ad: Option<&str> = match t.kind() {
            "slice_type" | "array_type" => {
                let oge = t.child_by_field_name("element");
                if oge.is_some_and(|o| metin(o, k) == "byte") {
                    kurallar.push("IsString()".into());
                    Some("string")
                } else {
                    kurallar.push("IsArray()".into());
                    if let Some((sdi, sn, _, _, _)) = oge.and_then(|o| self.p.yapi_bul(di, metin(o, k))).filter(|x| x.1.kind() == "struct_type") {
                        let a = self.sema_alanlari(sdi, sn, ad_etiketi, kural_etiketi, dogrulanir, false, &[], derinlik + 1, ziyaret);
                        if !a.is_empty() {
                            ic = Some(a);
                        }
                    }
                    Some("array")
                }
            }
            "map_type" => {
                kurallar.push("IsObject()".into());
                Some("object")
            }
            "struct_type" => {
                kurallar.push("ValidateNested()".into());
                let a = self.sema_alanlari(di, t, ad_etiketi, kural_etiketi, dogrulanir, false, &[], derinlik + 1, ziyaret);
                if !a.is_empty() {
                    ic = Some(a);
                }
                Some("object")
            }
            "type_identifier" | "qualified_type" | "generic_type" => {
                let ham = metin(t, k).trim().to_string();
                match ham.as_str() {
                    "string" => {
                        kurallar.push("IsString()".into());
                        Some("string")
                    }
                    "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "rune" => {
                        kurallar.push("IsInt()".into());
                        if ham.starts_with('u') {
                            kurallar.push("Min(0)".into());
                        }
                        Some("int")
                    }
                    "float32" | "float64" | "decimal.Decimal" | "big.Float" => {
                        kurallar.push("IsNumber()".into());
                        Some("number")
                    }
                    "bool" => {
                        kurallar.push("IsBoolean()".into());
                        Some("boolean")
                    }
                    "time.Time" | "civil.Date" | "null.Time" | "sql.NullTime" => {
                        kurallar.push("IsDateString()".into());
                        nullable |= ham.contains("Null") || ham.starts_with("null.");
                        Some("date")
                    }
                    "uuid.UUID" | "uuid.NullUUID" => {
                        kurallar.push("IsUUID()".into());
                        Some("string")
                    }
                    "sql.NullString" | "null.String" => {
                        nullable = true;
                        kurallar.push("IsString()".into());
                        Some("string")
                    }
                    "sql.NullInt64" | "sql.NullInt32" | "null.Int" => {
                        nullable = true;
                        kurallar.push("IsInt()".into());
                        Some("int")
                    }
                    "sql.NullBool" | "null.Bool" => {
                        nullable = true;
                        kurallar.push("IsBoolean()".into());
                        Some("boolean")
                    }
                    "sql.NullFloat64" | "null.Float" => {
                        nullable = true;
                        kurallar.push("IsNumber()".into());
                        Some("number")
                    }
                    _ => match self.p.yapi_bul(di, &ham) {
                        Some((sdi, sn, _, _, dizi)) if sn.kind() == "struct_type" => {
                            let a = self.sema_alanlari(sdi, sn, ad_etiketi, kural_etiketi, dogrulanir, false, &[], derinlik + 1, ziyaret);
                            if !a.is_empty() {
                                ic = Some(a);
                            }
                            if dizi {
                                kurallar.push("IsArray()".into());
                                Some("array")
                            } else {
                                kurallar.push("ValidateNested()".into());
                                Some("object")
                            }
                        }
                        // Adlı temel tip (`type Durum string`): alttaki tip
                        Some((sdi, sn, _, _, _)) if derinlik < 4 => {
                            let mut alt = self.alan_kur(sdi, sn, kural_etiketi, None, dogrulanir, satir, derinlik + 1, ziyaret, ad_etiketi);
                            kurallar.append(&mut alt.kurallar);
                            alt.tip.as_deref().map(|x| match x {
                                "string" => "string",
                                "int" => "int",
                                "number" => "number",
                                "boolean" => "boolean",
                                "array" => "array",
                                "object" => "object",
                                "date" => "date",
                                _ => "unknown",
                            })
                        }
                        _ => None,
                    },
                }
            }
            _ => None,
        };
        let (mut zorunlu, mut belirsiz, mut degerler) = (false, !dogrulanir, vec![]);
        if dogrulanir {
            if let Some(kural) = kural {
                let (mut v, z, kos, d) = dogrulama_kurallari(kural, tip_ad.unwrap_or(""));
                kurallar.append(&mut v);
                zorunlu = z && !kos;
                belirsiz = kos;
                degerler = d;
            }
        }
        if nullable {
            kurallar.push("nullable()".into());
        }
        Alan { tip: tip_ad.filter(|t| *t != "unknown").map(String::from), zorunlu, belirsiz, kurallar, satir, kalitimdan: None, degerler, ic }
    }

    /// Yanıt şeması alanları: `json` adları; `omitempty` yoksa alan her yanıtta vardır (zorunlu)
    fn yanit_alanlari(&self, di: usize, st: Node<'a>, derinlik: usize, ziyaret: &mut Vec<usize>) -> BTreeMap<String, Alan> {
        let mut a = self.sema_alanlari(di, st, "json", None, true, false, &[], derinlik, ziyaret);
        let k = self.p.k(di);
        // omitempty olmayanlar zorunludur
        let mut bos_olabilir = HashSet::new();
        for fd in struct_alanlari(st) {
            let et = fd.child_by_field_name("tag").and_then(|t| metin_degismezi(t, k)).map(|t| etiketler(&t)).unwrap_or_default();
            if let Some(j) = et.get("json") {
                if j.contains("omitempty") {
                    let ad = j.split(',').next().unwrap_or("").to_string();
                    let ad = if ad.is_empty() { adli_cocuklar(fd, "name").first().map(|x| metin(*x, k).to_string()).unwrap_or_default() } else { ad };
                    bos_olabilir.insert(ad);
                }
            }
        }
        for (ad, x) in a.iter_mut() {
            x.zorunlu = !bos_olabilir.contains(ad);
            x.belirsiz = false;
        }
        a
    }

    /// Yardımcı yanıt fonksiyonu: (durum kodu parametresi, değer parametresi), gövdesi parametreyi JSON yazıyorsa
    /// (`func respondJSON(w http.ResponseWriter, code int, v any)`)
    fn yanit_yardimcisi(&self, t: Tanim<'a>) -> Option<(Option<usize>, usize)> {
        let k = self.p.k(t.0);
        let govde = metin(t.1.child_by_field_name("body")?, k);
        let params: Vec<String> = self.p.parametreler(t).into_iter().map(|p| p.0.unwrap_or_default()).collect();
        if !(govde.contains("Encode(") || govde.contains(".JSON(") || govde.contains("json.Marshal(")) {
            return None;
        }
        let deger = params
            .iter()
            .position(|p| !p.is_empty() && (govde.contains(&format!("Encode({p})")) || govde.contains(&format!(", {p})")) || govde.contains(&format!("Marshal({p})"))))?;
        let kod = params
            .iter()
            .position(|p| !p.is_empty() && (govde.contains(&format!("WriteHeader({p})")) || govde.contains(&format!("JSON({p},")) || govde.contains(&format!("Status({p})"))));
        Some((kod, deger))
    }

    fn analiz(&mut self, t: Tanim<'a>, cati: Cati) -> Analiz {
        if let Some(a) = self.analizler.get(&t.1.id()) {
            return a.clone();
        }
        let a = self.analiz_yap(t, cati);
        self.analizler.insert(t.1.id(), a.clone());
        a
    }

    fn analiz_yap(&self, t: Tanim<'a>, cati: Cati) -> Analiz {
        let (di, fnode) = t;
        let k = self.p.k(di);
        let tipler = self.yerel_tipler(t);
        // İstek bağlamı değişkenleri (`c *gin.Context`, `c echo.Context`, `c *fiber.Ctx`, `r *http.Request`): sorgu ve
        // parametre okumaları yalnızca bunların üzerinde sayılır (`db.Query("SELECT ...")` sorgu parametresi değildir)
        let baglam: HashSet<String> =
            tipler.iter().filter(|(_, (_, t, _))| t.ends_with("Context") || t.ends_with("Ctx") || t.ends_with("http.Request")).map(|(a, _)| a.clone()).collect();
        let mut an = Analiz::default();
        let mut bagli: Vec<Baglama> = vec![];
        let mut dogrulanan: HashSet<String> = HashSet::new();
        let mut ozel: HashSet<String> = HashSet::new();
        // Yanıt adayları: (durum, değer düğümü)
        let mut yanitlar: Vec<(Option<u16>, Node<'a>, &'static str)> = vec![];
        let mut sorgu_degiskenleri: HashSet<String> = HashSet::new();
        let mut yol_degiskenleri: HashSet<String> = HashSet::new();
        // `dec := json.NewDecoder(r.Body)`: sonra `dec.Decode(&x)`
        let mut cozuculer: HashSet<String> = HashSet::new();
        let mut yigin = vec![fnode];
        let mut son_basari: Option<u16> = None;
        while let Some(n) = yigin.pop() {
            let mut c = cocuklar(n);
            c.reverse();
            yigin.extend(c);
            // `q := r.URL.Query()` / `vars := mux.Vars(r)`
            if n.kind() == "short_var_declaration" {
                let sol: Vec<Node> = n.child_by_field_name("left").map(cocuklar).unwrap_or_default();
                let sag: Vec<Node> = n.child_by_field_name("right").map(cocuklar).unwrap_or_default();
                if let (Some(l), Some(r)) = (sol.first(), sag.first()) {
                    let rm = metin(*r, k);
                    if rm.ends_with("URL.Query()") || rm.ends_with(".QueryParams()") {
                        sorgu_degiskenleri.insert(metin(*l, k).to_string());
                    }
                    if rm.starts_with("mux.Vars(") {
                        yol_degiskenleri.insert(metin(*l, k).to_string());
                    }
                    if rm.contains("NewDecoder(") {
                        cozuculer.insert(metin(*l, k).to_string());
                    }
                }
                continue;
            }
            // mux.Vars(r)["id"] / vars["id"]
            if n.kind() == "index_expression" {
                if let (Some(o), Some(i)) = (n.child_by_field_name("operand"), n.child_by_field_name("index")) {
                    let om = metin(o, k);
                    if om.starts_with("mux.Vars(") || yol_degiskenleri.contains(om) {
                        if let Some(ad) = metin_degismezi(i, k) {
                            let tip = Self::donusum_tipi(n, k);
                            an.yol_tipleri.entry(ad).or_insert(tip);
                        }
                    }
                }
                continue;
            }
            // net/http yöntem denetimi: r.Method == http.MethodPost / switch r.Method { case "GET": }
            if n.kind() == "binary_expression" {
                let (l, r) = (n.child_by_field_name("left").map(|x| metin(x, k)).unwrap_or(""), n.child_by_field_name("right").map(|x| metin(x, k)).unwrap_or(""));
                let op = n.child_by_field_name("operator").map(|o| metin(o, k)).unwrap_or("");
                if (op == "==" || op == "!=") && (l.ends_with(".Method") || r.ends_with(".Method")) {
                    let diger = if l.ends_with(".Method") { r } else { l };
                    if let Some(m) = yontem_sabiti(diger) {
                        if !an.yontemler.contains(&m) {
                            an.yontemler.push(m);
                        }
                    }
                }
                continue;
            }
            if n.kind() == "expression_switch_statement" && n.child_by_field_name("value").is_some_and(|v| metin(v, k).ends_with(".Method")) {
                for c in cocuklar(n).into_iter().filter(|c| c.kind() == "expression_case") {
                    for v in c.child_by_field_name("value").map(cocuklar).unwrap_or_default() {
                        if let Some(m) = yontem_sabiti(metin(v, k)) {
                            if !an.yontemler.contains(&m) {
                                an.yontemler.push(m);
                            }
                        }
                    }
                }
                continue;
            }
            if n.kind() != "call_expression" {
                continue;
            }
            let Some(g) = n.child_by_field_name("function") else { continue };
            let gm: String = metin(g, k).chars().filter(|c| !c.is_whitespace()).collect();
            let son = gm.rsplit('.').next().unwrap_or("").to_string();
            let gm = match gm.strip_suffix(".Decode") {
                Some(on) if cozuculer.contains(on) => "json.NewDecoder().Decode".to_string(),
                _ => gm,
            };
            let args: Vec<Node<'a>> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
            let arg = |i: usize| args.get(i).copied();
            let ilk_metin = arg(0).and_then(|a| metin_degismezi(a, k));
            let tip = Self::donusum_tipi(n, k);
            let istekte = baglam.contains(&sol_ad(g, k));
            // Gövde bağlama: çatı metotları yalnızca istek bağlamı üzerinde
            let son_e = if !istekte && !gm.starts_with("echo.") && CATI_BAGLAMA.contains(&son.as_str()) { "" } else { son.as_str() };
            match (cati, son_e) {
                _ if baglama_turu(cati, son_e, &gm, args.len(), |i| args.get(i).map(|a| metin(*a, k).to_string())).is_some() => {
                    if let Some((i, bolum, ad_et, kural_et, dogrular)) = baglama_turu(cati, son_e, &gm, args.len(), |i| args.get(i).map(|a| metin(*a, k).to_string())) {
                        if let Some(h) = Self::hedef(arg(i), k) {
                            bagli.push((h, bolum, ad_et, kural_et, dogrular));
                        }
                    }
                }
                (Cati::Echo, "Validate") if !args.is_empty() => {
                    if let Some(h) = Self::hedef(arg(0), k) {
                        dogrulanan.insert(h);
                    }
                }
                (_, "Struct" | "StructCtx") => {
                    if let Some(h) = Self::hedef(args.last().copied(), k) {
                        dogrulanan.insert(h);
                    }
                }
                (_, "Validate") if args.is_empty() => {
                    if let Some(o) = g.child_by_field_name("operand").filter(|o| o.kind() == "identifier") {
                        ozel.insert(metin(o, k).to_string());
                    }
                }
                _ => {
                    // Yardımcı: `app.readJSON(w, r, &input)`, `validate(input)`
                    if let Some(yt) = self.cagri_tanimi(di, n, &tipler, true) {
                        let ym = yt.1.child_by_field_name("body").map(|b| metin(b, self.p.k(yt.0))).unwrap_or("");
                        // Bağlayan yardımcı: argümanı ya da alıcısı yardımcının içinde bağlanır (`common.Bind(c, s)`,
                        // `v.Bind(c)` metodu alıcısını bağlar)
                        if let Some((i, bolum, ad_et, kural_et, dogrular)) = self.baglayici(yt, cati, 0) {
                            let hedef_dugum = match i {
                                Some(i) => arg(i),
                                None => g.child_by_field_name("operand"),
                            };
                            if let Some(h) = Self::hedef(hedef_dugum, k).filter(|h| tipler.contains_key(h)) {
                                bagli.push((h, bolum, ad_et, kural_et, dogrular));
                            }
                        }
                        for a in &args {
                            if let Some(h) = Self::hedef(Some(*a), k).filter(|h| tipler.contains_key(h)) {
                                if ym.contains(".Struct(") || ym.contains(".Validate(") {
                                    dogrulanan.insert(h);
                                }
                            }
                        }
                        // Yanıt yardımcısı
                        if let Some((kod_i, deger_i)) = self.yanit_yardimcisi(yt) {
                            let durum = kod_i.and_then(arg).and_then(|a| crate::dna::go_kod_ifadesi(metin(a, k)));
                            if let Some(v) = arg(deger_i) {
                                yanitlar.push((durum, v, "json"));
                            }
                        }
                    }
                }
            }
            // Sorgu, form ve yol parametreleri (kullanımdan)
            let yerel = g.child_by_field_name("operand").map(|o| metin(o, k).to_string()).unwrap_or_default();
            match son.as_str() {
                _ if !istekte && !yerel.ends_with("Query()") && !sorgu_degiskenleri.contains(&yerel) && !gm.starts_with("chi.") => {}
                "Query" | "GetQuery" | "QueryArray" | "GetQueryArray" | "QueryParam" | "QueryInt" | "QueryBool" | "QueryFloat"
                    if ilk_metin.is_some() && !gm.ends_with("Bind().Query") =>
                {
                    let ad = ilk_metin.clone().unwrap_or_default();
                    let varsayilanli = (cati == Cati::Fiber && args.len() > 1) || son == "QueryInt" && args.len() > 1;
                    let tip = match son.as_str() {
                        "QueryInt" => "int".to_string(),
                        "QueryBool" => "boolean".to_string(),
                        "QueryFloat" => "number".to_string(),
                        _ => tip.clone(),
                    };
                    an.sorgu.entry(ad).or_insert_with(|| kullanim_alani(&tip, false, !varsayilanli, satir(n)));
                }
                "DefaultQuery" => {
                    if let Some(ad) = ilk_metin.clone() {
                        an.sorgu.entry(ad).or_insert_with(|| kullanim_alani(&tip, false, false, satir(n)));
                    }
                }
                "Get" if (yerel.ends_with("URL.Query()") || sorgu_degiskenleri.contains(&yerel) || yerel.ends_with("QueryParams()")) && ilk_metin.is_some() => {
                    an.sorgu.entry(ilk_metin.clone().unwrap_or_default()).or_insert_with(|| kullanim_alani(&tip, false, true, satir(n)));
                }
                "Has" if (yerel.ends_with("URL.Query()") || sorgu_degiskenleri.contains(&yerel)) && ilk_metin.is_some() => {
                    an.sorgu.entry(ilk_metin.clone().unwrap_or_default()).or_insert_with(|| kullanim_alani("string", false, false, satir(n)));
                }
                "PostForm" | "DefaultPostForm" | "GetPostForm" | "PostFormArray" | "PostFormValue" if ilk_metin.is_some() => {
                    let varsayilanli = son == "DefaultPostForm";
                    an.form.entry(ilk_metin.clone().unwrap_or_default()).or_insert_with(|| kullanim_alani(&tip, false, !varsayilanli, satir(n)));
                }
                "FormValue" if ilk_metin.is_some() => {
                    let hedef = if cati == Cati::Http { &mut an.form_ya_da_sorgu } else { &mut an.form };
                    hedef.entry(ilk_metin.clone().unwrap_or_default()).or_insert_with(|| kullanim_alani(&tip, false, true, satir(n)));
                }
                "FormFile" | "MultipartForm" | "ParseMultipartForm" | "SaveUploadedFile" => {
                    let not = format!(
                        "dosya doğrulaması kontrol edilemedi: {} ({}); dosyanın varlığı, türü ve içeriği önizlemede sınanmaz",
                        ilk_metin.clone().map(|a| format!("'{a}' alanı")).unwrap_or_else(|| "çok parçalı form".into()),
                        son
                    );
                    if !an.notlar.contains(&not) {
                        an.notlar.push(not);
                    }
                }
                "Param" | "Params" | "ParamsInt" | "PathValue" if ilk_metin.is_some() => {
                    let tip = if son == "ParamsInt" { "int".to_string() } else { tip.clone() };
                    an.yol_tipleri.entry(ilk_metin.clone().unwrap_or_default()).or_insert(tip);
                }
                "URLParam" if gm.starts_with("chi.") => {
                    if let Some(ad) = arg(1).and_then(|a| metin_degismezi(a, k)) {
                        an.yol_tipleri.entry(ad).or_insert(tip.clone());
                    }
                }
                _ => {}
            }
            // Yanıtlar
            match son.as_str() {
                "JSON" | "Render" if gm.starts_with("render.") => {
                    if let Some(a) = arg(2) {
                        yanitlar.push((None, a, "render.JSON"));
                    }
                }
                "JSON" | "IndentedJSON" | "SecureJSON" | "PureJSON" | "AsciiJSON" | "JSONPretty" if args.len() >= 2 && !gm.contains("Bind()") => {
                    let durum = arg(0).and_then(|a| crate::dna::go_kod_ifadesi(metin(a, k)));
                    yanitlar.push((durum, args[1], "c.JSON"));
                }
                "JSON" if args.len() == 1 && cati == Cati::Fiber && !gm.contains("Bind()") => {
                    // c.Status(201).JSON(x)
                    let durum = g
                        .child_by_field_name("operand")
                        .filter(|o| o.kind() == "call_expression")
                        .and_then(|o| o.child_by_field_name("arguments"))
                        .and_then(|a| cocuklar(a).into_iter().next())
                        .and_then(|a| crate::dna::go_kod_ifadesi(metin(a, k)));
                    yanitlar.push((durum, args[0], "c.JSON"));
                }
                "Encode" if gm.contains("NewEncoder(") => {
                    if let Some(a) = arg(0) {
                        yanitlar.push((son_basari, a, "json.Encode"));
                    }
                }
                "WriteHeader" => {
                    son_basari = arg(0).and_then(|a| crate::dna::go_kod_ifadesi(metin(a, k)));
                }
                _ => {}
            }
        }
        // Bağlanan yapılar
        for (h, bolum, ad_et, kural_et, cati_dogrular) in bagli {
            let dogrulanir = cati_dogrular || dogrulanan.contains(&h);
            let kural_et = if dogrulanan.contains(&h) && !cati_dogrular { Some("validate") } else { kural_et };
            let Some((tdi, tm, satir_ici)) = tipler.get(&h).cloned() else { continue };
            let yapi = match satir_ici {
                Some(st) => Some((tdi, st, String::new(), "body".to_string(), false)),
                None => self.p.yapi_bul(tdi, &tm).filter(|x| x.1.kind() == "struct_type"),
            };
            let dosya = self.p.goreli(yapi.as_ref().map(|x| x.0).unwrap_or(di));
            let dto = yapi.as_ref().map(|x| x.3.clone()).unwrap_or_else(|| tm.trim_start_matches('*').to_string());
            let sema = |alanlar: BTreeMap<String, Alan>, bulunamadi: bool, dizi: bool| Sema {
                dto: dto.clone(),
                dosya: Some(dosya.clone()),
                bulunamadi,
                kalitim: None,
                alanlar,
                adaylar: vec![],
                dizi,
            };
            if ozel.contains(&h) {
                an.notlar.push(format!("nesne düzeyi doğrulama kontrol edilemedi: {dto}.Validate() metodu önizlemede sınanmaz"));
            }
            let Some((sdi, st, _, _, dizi)) = yapi else {
                if bolum == "govde" || bolum == "hepsi" {
                    an.govde.push(Girdi { ad: Some(h), tip: Some(dto.clone()), sema: Some(sema(BTreeMap::new(), true, false)) });
                }
                continue;
            };
            let mut z = vec![];
            match bolum {
                "govde" | "form" => {
                    let ad_et = if bolum == "form" { "form" } else { ad_et };
                    let alanlar = self.sema_alanlari(sdi, st, ad_et, kural_et, dogrulanir, false, &[], 0, &mut z);
                    an.govde.push(Girdi { ad: Some(h), tip: Some(dto.clone()), sema: Some(sema(alanlar, false, dizi)) });
                }
                "sorgu" => {
                    for (a, x) in self.sema_alanlari(sdi, st, ad_et, kural_et, dogrulanir, false, &[], 0, &mut z) {
                        an.sorgu.insert(a, x);
                    }
                }
                "yol" => {
                    for (a, x) in self.sema_alanlari(sdi, st, ad_et, kural_et, dogrulanir, true, &[], 0, &mut z) {
                        an.yol_tipleri.insert(a, x.tip.unwrap_or_else(|| "string".into()));
                    }
                }
                _ => {
                    // Echo Bind: `param` etiketliler yol, `query` etiketliler sorgu, kalanlar gövde
                    for (a, x) in self.sema_alanlari(sdi, st, "param", kural_et, dogrulanir, true, &[], 0, &mut z) {
                        an.yol_tipleri.insert(a, x.tip.unwrap_or_else(|| "string".into()));
                    }
                    for (a, x) in self.sema_alanlari(sdi, st, "query", kural_et, dogrulanir, true, &[], 0, &mut z) {
                        an.sorgu.insert(a, x);
                    }
                    let alanlar = self.sema_alanlari(sdi, st, "json", kural_et, dogrulanir, false, &["param", "query"], 0, &mut z);
                    if !alanlar.is_empty() {
                        an.bind_govde.push(Girdi { ad: Some(h), tip: Some(dto.clone()), sema: Some(sema(alanlar, false, dizi)) });
                    }
                }
            }
        }
        an.yanit = self.yanit_kur(di, &tipler, &yanitlar);
        an
    }

    /// Bağlayan yardımcı fonksiyon: parametresini (Some(sıra)) ya da alıcısını (None) bir bağlama çağrısına veren
    /// fonksiyon, 2 kat (`func Bind(c *gin.Context, obj any) error { return c.ShouldBindWith(obj, b) }`,
    /// `func (s *Dogrulayici) Bind(c *gin.Context) error { return common.Bind(c, s) }`)
    #[allow(clippy::type_complexity)]
    fn baglayici(&self, t: Tanim<'a>, cati: Cati, derinlik: usize) -> Option<(Option<usize>, &'static str, &'static str, Option<&'static str>, bool)> {
        if derinlik > 2 {
            return None;
        }
        let k = self.p.k(t.0);
        let params: Vec<String> = self.p.parametreler(t).into_iter().map(|p| p.0.unwrap_or_default()).collect();
        let alici = (t.1.kind() == "method_declaration")
            .then(|| t.1.child_by_field_name("receiver").and_then(|r| cocuklar(r).into_iter().next()).and_then(|p| p.child_by_field_name("name")).map(|x| metin(x, k).to_string()))
            .flatten();
        let konum = |ad: &str| -> Option<Option<usize>> {
            if alici.as_deref() == Some(ad) {
                return Some(None);
            }
            params.iter().position(|p| p == ad).map(Some)
        };
        let tipler = self.yerel_tipler(t);
        let mut yigin = vec![t.1.child_by_field_name("body")?];
        while let Some(n) = yigin.pop() {
            yigin.extend(cocuklar(n));
            if n.kind() != "call_expression" {
                continue;
            }
            let Some(g) = n.child_by_field_name("function") else { continue };
            let gm: String = metin(g, k).chars().filter(|c| !c.is_whitespace()).collect();
            let son = gm.rsplit('.').next().unwrap_or("").to_string();
            let args: Vec<Node<'a>> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
            // Çatı metodu yalnızca istek bağlamı üzerinde (`common.Bind(c, s)` Gin'in `c.Bind`'i değildir)
            let istekte = tipler.get(&sol_ad(g, k)).is_some_and(|(_, t, _)| t.ends_with("Context") || t.ends_with("Ctx") || t.ends_with("http.Request"));
            let son = if !istekte && !gm.starts_with("echo.") && CATI_BAGLAMA.contains(&son.as_str()) { String::new() } else { son };
            if let Some((i, b, a, kr, d)) = baglama_turu(cati, &son, &gm, args.len(), |i| args.get(i).map(|a| metin(*a, k).to_string())) {
                if let Some(yer) = Self::hedef(args.get(i).copied(), k).and_then(|h| konum(&h)) {
                    return Some((yer, b, a, kr, d));
                }
            }
            if let Some(alt) = self.cagri_tanimi(t.0, n, &tipler, false) {
                if let Some((yer, b, a, kr, d)) = self.baglayici(alt, cati, derinlik + 1) {
                    let hedef = match yer {
                        Some(i) => args.get(i).copied(),
                        None => g.child_by_field_name("operand"),
                    };
                    if let Some(yer2) = Self::hedef(hedef, k).and_then(|h| konum(&h)) {
                        return Some((yer2, b, a, kr, d));
                    }
                }
            }
        }
        None
    }

    /// Değerin dönüştürüldüğü tip: `strconv.Atoi(c.Param("id"))` -> int
    fn donusum_tipi(n: Node, k: &[u8]) -> String {
        let ust = n
            .parent()
            .filter(|p| p.kind() == "argument_list")
            .and_then(|p| p.parent())
            .and_then(|c| c.child_by_field_name("function"))
            .map(|g| metin(g, k).to_string())
            .unwrap_or_default();
        match ust.as_str() {
            "strconv.Atoi" | "strconv.ParseInt" | "strconv.ParseUint" => "int".into(),
            "strconv.ParseFloat" => "number".into(),
            "strconv.ParseBool" => "boolean".into(),
            "uuid.Parse" | "uuid.MustParse" | "uuid.FromString" => "uuid".into(),
            _ => "string".into(),
        }
    }

    /// Başarılı yanıtların şeması: tek anlamlı statik tip (kesin) ya da tek harita değişmezi (Yorum)
    fn yanit_kur(&self, di: usize, tipler: &Tipler<'a>, yanitlar: &[(Option<u16>, Node<'a>, &'static str)]) -> Option<Yanit> {
        let k = self.p.k(di);
        let mut adaylar: Vec<Yanit> = vec![];
        for (durum, v, kaynak) in yanitlar {
            if durum.is_some_and(|d| d >= 300) {
                continue;
            }
            let y = if let Some((tdi, tm, satir_ici)) = self.ifade_tipi(di, *v, tipler, 0) {
                let yapi = match satir_ici {
                    Some(st) => Some((tdi, st, String::new(), "response".to_string(), false)),
                    None => self.p.yapi_bul(tdi, &tm).filter(|x| x.1.kind() == "struct_type"),
                };
                match yapi {
                    Some((sdi, st, _, ad, dizi)) => {
                        let mut z = vec![];
                        let alanlar = self.yanit_alanlari(sdi, st, 0, &mut z);
                        Some(Yanit {
                            kaynak: kaynak.to_string(),
                            kesin: true,
                            durum: *durum,
                            sema: Sema { dto: ad, dosya: Some(self.p.goreli(sdi)), bulunamadi: false, kalitim: None, alanlar, adaylar: vec![], dizi: dizi || tm.starts_with("[]") },
                        })
                    }
                    None if tm.starts_with("map[") || tm.ends_with(".H") || tm.ends_with(".Map") => self.harita_yaniti(di, *v, *durum, kaynak),
                    None => None,
                }
            } else {
                self.harita_yaniti(di, *v, *durum, kaynak)
            };
            if let Some(y) = y {
                if !adaylar.iter().any(|a| a.sema.dto == y.sema.dto && a.sema.alanlar.keys().eq(y.sema.alanlar.keys())) {
                    adaylar.push(y);
                }
            } else if metin(*v, k) != "nil" {
                // Tipi çözülemeyen başarılı yanıt: şema tek anlamlı değildir
                return None;
            }
        }
        if adaylar.len() != 1 {
            return None;
        }
        adaylar.pop()
    }

    /// `gin.H{"a": x}` / `map[string]any{...}` / `fiber.Map{}`: anahtarlar (değer tipleri bilinmez), Yorum
    fn harita_yaniti(&self, di: usize, v: Node<'a>, durum: Option<u16>, kaynak: &str) -> Option<Yanit> {
        let k = self.p.k(di);
        let v = if v.kind() == "unary_expression" { v.named_child(0)? } else { v };
        if v.kind() != "composite_literal" {
            return None;
        }
        let mut alanlar = BTreeMap::new();
        for o in cocuklar(v.child_by_field_name("body")?).into_iter().filter(|o| o.kind() == "keyed_element") {
            let c = cocuklar(o);
            let anahtar = c.first().and_then(|a| a.named_child(0)).and_then(|a| metin_degismezi(a, k))?;
            alanlar.insert(anahtar, Alan { tip: None, zorunlu: true, belirsiz: true, kurallar: vec![], satir: satir(o), kalitimdan: None, degerler: vec![], ic: None });
        }
        if alanlar.is_empty() {
            return None;
        }
        let dto = metin(v.child_by_field_name("type")?, k).to_string();
        Some(Yanit {
            kaynak: kaynak.to_string(),
            kesin: false,
            durum,
            sema: Sema { dto, dosya: Some(self.p.goreli(di)), bulunamadi: false, kalitim: None, alanlar, adaylar: vec![], dizi: false },
        })
    }

    // ---- Veri bağı ----

    /// Fonksiyon gövdesindeki GORM ve SQL kullanımı: (okunan, yazılan, eksik olabilir, silinen modeller ve yumuşak silme)
    fn veri_govdesi(&self, t: Tanim<'a>, tipler: &Tipler<'a>) -> VeriGovdesi {
        static D: OnceLock<[Regex; 4]> = OnceLock::new();
        let [okuma, ekleme, guncelleme, silme] = D.get_or_init(|| {
            let ad = r#"[`"\[]?(?:\w+[`"\]]?\.[`"\[]?)?(\w+)[`"\]]?"#;
            [
                Regex::new(&format!(r"(?i)\b(?:FROM|JOIN)\s+{ad}")).unwrap(),
                Regex::new(&format!(r"(?i)\bINSERT\s+(?:OR\s+\w+\s+)?INTO\s+{ad}")).unwrap(),
                Regex::new(&format!(r"(?i)\bUPDATE\s+{ad}\s+SET\b")).unwrap(),
                Regex::new(&format!(r"(?i)\bDELETE\s+FROM\s+{ad}")).unwrap(),
            ]
        });
        const GORM_OKUMA: [&str; 10] = ["Find", "First", "Last", "Take", "Scan", "Count", "Pluck", "FindInBatches", "Row", "Rows"];
        const GORM_YAZMA: [&str; 9] = ["Create", "CreateInBatches", "Save", "Update", "Updates", "UpdateColumn", "UpdateColumns", "Delete", "FirstOrCreate"];
        const SQL: [&str; 17] = [
            "Query",
            "QueryRow",
            "QueryContext",
            "QueryRowContext",
            "Exec",
            "ExecContext",
            "Get",
            "Select",
            "GetContext",
            "SelectContext",
            "NamedExec",
            "NamedQuery",
            "Queryx",
            "QueryRowx",
            "MustExec",
            "Raw",
            "Prepare",
        ];
        let (di, fnode) = t;
        let k = self.p.k(di);
        let (mut okur, mut yazar, mut eksik, mut silinen) = (vec![], vec![], false, vec![]);
        let baglam: HashSet<&String> = tipler.iter().filter(|(_, (_, t, _))| t.ends_with("Context") || t.ends_with("Ctx") || t.ends_with("http.Request")).map(|(a, _)| a).collect();
        let mut yigin = vec![fnode];
        while let Some(n) = yigin.pop() {
            yigin.extend(cocuklar(n));
            if n.kind() != "call_expression" {
                continue;
            }
            let Some(g) = n.child_by_field_name("function").filter(|g| g.kind() == "selector_expression") else { continue };
            let son = g.child_by_field_name("field").map(|f| metin(f, k)).unwrap_or("");
            let args: Vec<Node<'a>> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
            let gorm = GORM_OKUMA.contains(&son) || GORM_YAZMA.contains(&son);
            let sql = SQL.contains(&son);
            if (!gorm && !sql) || baglam.contains(&sol_ad(g, k)) || metin(g, k).contains("URL.Query") {
                continue;
            }
            // Proje metodu ise ORM değildir (zincir ayrıca izlenir)
            if self.cagri_tanimi(di, n, tipler, false).is_some() {
                continue;
            }
            // SQL metni: değişmez ya da sabit (sqlc)
            let sql_metni = args.iter().find_map(|a| self.sql_metni(di, *a, 0));
            if let Some(s) = sql_metni.as_deref() {
                let silinenler: Vec<String> = silme.captures_iter(s).map(|c| c[1].to_string()).collect();
                for c in okuma.captures_iter(s) {
                    let once = &s[..c.get(0).map_or(0, |m| m.start())];
                    if once.trim_end().to_uppercase().ends_with("DELETE") {
                        continue;
                    }
                    okur.push(self.p.tablo_modeli(&c[1]));
                }
                for re in [ekleme, guncelleme] {
                    yazar.extend(re.captures_iter(s).map(|c| self.p.tablo_modeli(&c[1])));
                }
                for t in silinenler {
                    let m = self.p.tablo_modeli(&t);
                    yazar.push(m.clone());
                    silinen.push((m, satir(n), false));
                }
                continue;
            }
            if sql && !gorm && !matches!(son, "Get" | "Select") {
                // Metni koddan çıkarılamayan sorgu
                eksik = true;
                continue;
            }
            if !gorm || !self.p.gorm {
                continue;
            }
            // GORM: model argümandan ya da zincirdeki Model(...) / Table("...") çağrısından
            let mut model: Option<String> = None;
            let mut yumusak = false;
            for a in &args {
                if let Some((tdi, tm, _)) = self.ifade_tipi(di, *a, tipler, 0) {
                    if let Some(m) = self.p.model(tdi, &tm) {
                        model = Some(m.ad.clone());
                        yumusak = m.yumusak;
                        break;
                    }
                }
            }
            let mut zincir = g.child_by_field_name("operand");
            let mut kapsamsiz = false;
            while let Some(z) = zincir.filter(|z| z.kind() == "call_expression") {
                let zg = z.child_by_field_name("function").filter(|x| x.kind() == "selector_expression");
                let zs = zg.and_then(|x| x.child_by_field_name("field")).map(|f| metin(f, k)).unwrap_or("");
                let zargs: Vec<Node<'a>> = z.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
                match zs {
                    "Unscoped" => kapsamsiz = true,
                    "Model" if model.is_none() => {
                        if let Some((tdi, tm, _)) = zargs.first().and_then(|a| self.ifade_tipi(di, *a, tipler, 0)) {
                            if let Some(m) = self.p.model(tdi, &tm) {
                                model = Some(m.ad.clone());
                                yumusak = m.yumusak;
                            }
                        }
                    }
                    "Table" if model.is_none() => {
                        if let Some(t) = zargs.first().and_then(|a| metin_degismezi(*a, k)) {
                            model = Some(self.p.tablo_modeli(t.split_whitespace().next().unwrap_or(&t)));
                        }
                    }
                    "Raw" | "Exec" => {
                        if let Some(s) = zargs.iter().find_map(|a| self.sql_metni(di, *a, 0)) {
                            okur.extend(okuma.captures_iter(&s).map(|c| self.p.tablo_modeli(&c[1])));
                        } else {
                            eksik = true;
                        }
                    }
                    _ => {}
                }
                zincir = zg.and_then(|x| x.child_by_field_name("operand"));
            }
            let Some(m) = model else { continue };
            if GORM_OKUMA.contains(&son) || son == "FirstOrCreate" {
                okur.push(m.clone());
            }
            if GORM_YAZMA.contains(&son) {
                yazar.push(m.clone());
            }
            if son == "Delete" {
                silinen.push((m, satir(n), yumusak && !kapsamsiz));
            }
        }
        (okur, yazar, eksik, silinen)
    }

    /// SQL metni: değişmez, aynı paketteki ya da içe aktarılan paketteki sabit (sqlc `const createUser = ...`)
    fn sql_metni(&self, di: usize, n: Node<'a>, derinlik: usize) -> Option<String> {
        if derinlik > 4 {
            return None;
        }
        let k = self.p.k(di);
        let s = match n.kind() {
            "interpreted_string_literal" | "raw_string_literal" => metin_degismezi(n, k)?,
            "identifier" => {
                let &(vdi, deger, _) = self.p.degerler.get(&(self.p.paket[di].clone(), metin(n, k).to_string()))?;
                return self.sql_metni(vdi, deger?, derinlik + 1);
            }
            "selector_expression" => {
                let o = n.child_by_field_name("operand")?;
                let yol = self.p.iceri[di].get(metin(o, k))?;
                let &(vdi, deger, _) = self.p.degerler.get(&(yol.clone(), metin(n.child_by_field_name("field")?, k).to_string()))?;
                return self.sql_metni(vdi, deger?, derinlik + 1);
            }
            "binary_expression" => {
                let a = self.sql_metni(di, n.child_by_field_name("left")?, derinlik + 1).unwrap_or_default();
                let b = self.sql_metni(di, n.child_by_field_name("right")?, derinlik + 1).unwrap_or_default();
                a + " " + &b
            }
            _ => return None,
        };
        let b = s.to_uppercase();
        (b.contains("SELECT") || b.contains("INSERT") || b.contains("UPDATE") || b.contains("DELETE")).then_some(s)
    }

    /// İşleyiciden başlayıp proje içi çağrılara (3 kat) inerek veri bağı
    fn veri_bagi(&self, t: Tanim<'a>) -> Option<VeriBag> {
        if !self.p.gorm && !self.p.sql {
            return None;
        }
        let mut bag = VeriBag::default();
        let mut gorulen = HashSet::new();
        let mut yigin = vec![(t, 0usize)];
        while let Some((t, derinlik)) = yigin.pop() {
            if !gorulen.insert(t.1.id()) || gorulen.len() > 200 {
                continue;
            }
            let tipler = self.yerel_tipler(t);
            let (o, y, e, _) = self.veri_govdesi(t, &tipler);
            bag.okur.extend(o);
            bag.yazar.extend(y);
            bag.eksik_olabilir |= e;
            if derinlik < 3 {
                for c in self.cagrilar(t, &tipler) {
                    yigin.push((c, derinlik + 1));
                }
            }
        }
        bag.okur.sort();
        bag.okur.dedup();
        bag.yazar.sort();
        bag.yazar.dedup();
        Some(bag)
    }

    /// Gövdede çözülen proje çağrıları (kaynak sırasıyla, tekrarsız)
    fn cagrilar(&self, t: Tanim<'a>, tipler: &Tipler<'a>) -> Vec<Tanim<'a>> {
        let mut v: Vec<Tanim<'a>> = vec![];
        let mut yigin = vec![t.1];
        while let Some(n) = yigin.pop() {
            let mut c = cocuklar(n);
            c.reverse();
            yigin.extend(c);
            if n.kind() == "call_expression" {
                if let Some(x) = self.cagri_tanimi(t.0, n, tipler, true) {
                    if x.1.id() != t.1.id() && !v.iter().any(|y| y.1.id() == x.1.id()) && v.len() < 32 {
                        v.push(x);
                    }
                }
            }
        }
        v
    }

    /// Davranış zinciri: işleyici, projedeki ara katmanlar (bağımlılık) ve gövdede çağrılan proje fonksiyonları (2 kat)
    fn dna_zinciri(&self, t: Tanim<'a>, aralar: &[Tanim<'a>]) -> Value {
        let mut kayitlar: Vec<(String, crate::dna::GovdeDna, usize, &'static str)> = vec![];
        let mut gorulen = HashSet::new();
        let mut yigin: Vec<(Tanim<'a>, usize, &'static str)> = aralar.iter().rev().map(|a| (*a, 1, "bagimlilik")).collect();
        yigin.push((t, 0, "isleyici"));
        while let Some((t, derinlik, tur)) = yigin.pop() {
            if !gorulen.insert(t.1.id()) || kayitlar.len() >= 32 {
                continue;
            }
            let k = self.p.k(t.0);
            let govde = t.1.child_by_field_name("body").unwrap_or(t.1);
            let mut dna = crate::dna::go_govde(govde, k);
            let tipler = self.yerel_tipler(t);
            let (_, _, _, silinen) = self.veri_govdesi(t, &tipler);
            for (m, satir, yumusak) in silinen {
                if !dna.silinen.contains(&m) {
                    dna.silinen.push(m);
                }
                if yumusak && dna.yumusak.is_none() {
                    dna.yumusak = Some(satir);
                }
            }
            kayitlar.push((self.p.goreli(t.0), dna, derinlik, tur));
            if derinlik < 2 {
                let sonraki: Vec<(Tanim<'a>, usize, &'static str)> =
                    self.cagrilar(t, &tipler).into_iter().map(|c| (c, derinlik + 1, if tur == "bagimlilik" { "bagimlilik" } else { "servis" })).collect();
                yigin.extend(sonraki.into_iter().rev());
            }
        }
        let halkalar: Vec<crate::dna::Halka> = kayitlar.iter().map(|(d, dna, derinlik, tur)| crate::dna::Halka { dosya: d.clone(), dna, derinlik: *derinlik, tur }).collect();
        crate::dna::ozet(&halkalar)
    }
}

/// Seçici / çağrı zincirinin başındaki ad: `c.Request().Header.Get` -> "c"
fn sol_ad(mut n: Node, k: &[u8]) -> String {
    for _ in 0..16 {
        match n.kind() {
            "selector_expression" => n = n.child_by_field_name("operand").unwrap_or(n),
            "call_expression" => n = n.child_by_field_name("function").unwrap_or(n),
            "parenthesized_expression" | "unary_expression" => n = n.named_child(0).unwrap_or(n),
            _ => break,
        }
    }
    metin(n, k).to_string()
}

/// `http.MethodPost` / "POST" -> "POST"
fn yontem_sabiti(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(m) = s.strip_prefix("http.Method").or_else(|| s.strip_prefix("fiber.Method")) {
        return Some(m.to_uppercase());
    }
    let m = s.trim_matches('"');
    (s.starts_with('"') && matches!(m, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS")).then(|| m.to_string())
}

// ---- Giriş ve rol ----

/// Ara katmanın yetki etkisi
#[derive(Clone, Copy, PartialEq, Debug)]
enum Etki {
    /// Kimliksiz isteği reddeder
    Gerekli,
    /// Kimliği okur ama reddetmez
    Istege,
    /// Yetkiyle ilgisi yok
    Yok,
    /// Tanımı izlenemedi
    Bilinmeyen,
}

struct Karar {
    etki: Etki,
    kesin: bool,
    roller: Vec<String>,
}

/// Tanınan kitaplık doğrulayıcıları: (yol öneki, adlar)
const KITAPLIK_GIRIS: [(&str, &[&str]); 10] = [
    ("github.com/labstack/echo-jwt", &["JWT", "WithConfig"]),
    ("github.com/labstack/echo/middleware", &["JWT", "JWTWithConfig", "KeyAuth", "KeyAuthWithConfig", "BasicAuth", "BasicAuthWithConfig"]),
    ("github.com/labstack/echo/v4/middleware", &["JWT", "JWTWithConfig", "KeyAuth", "KeyAuthWithConfig", "BasicAuth", "BasicAuthWithConfig"]),
    ("github.com/gin-gonic/gin", &["BasicAuth", "BasicAuthForRealm"]),
    ("github.com/appleboy/gin-jwt", &["MiddlewareFunc"]),
    ("github.com/go-chi/jwtauth", &["Authenticator"]),
    ("github.com/go-chi/chi/v5/middleware", &["BasicAuth"]),
    ("github.com/gofiber/contrib/jwt", &["New"]),
    ("github.com/gofiber/jwt", &["New"]),
    ("github.com/gofiber/fiber/v2/middleware/basicauth", &["New"]),
];

/// Yetkiyle ilgisi olmayan bilinen kitaplıklar (günlük, CORS, sıkıştırma, kurtarma, ölçüm)
const KITAPLIK_ILGISIZ: [&str; 14] = [
    "github.com/gin-gonic/gin",
    "github.com/labstack/echo",
    "github.com/go-chi/chi",
    "github.com/go-chi/cors",
    "github.com/go-chi/httprate",
    "github.com/go-chi/render",
    "github.com/rs/cors",
    "github.com/gin-contrib",
    "github.com/gofiber/fiber",
    "github.com/gorilla/handlers",
    "github.com/prometheus",
    "go.opentelemetry.io",
    "github.com/unrolled",
    "net/http",
];

fn yetki_adi_mi(ad: &str) -> bool {
    static D: OnceLock<Regex> = OnceLock::new();
    D.get_or_init(|| Regex::new(r"(?i)(auth|jwt|token|login|session|protect|require.?user|verify|bearer|api.?key|guard|permission|role|acl|casbin|oauth|oidc|admin)").unwrap())
        .is_match(ad)
}

fn istege_adi_mi(ad: &str) -> bool {
    let a = ad.to_lowercase();
    a.contains("optional") || a.contains("maybe") || a.contains("soft") || a.contains("istege")
}

impl Cozumleyici<'_, '_> {
    fn karar(&self, a: &AraD) -> Karar {
        let yok = || Karar { etki: Etki::Yok, kesin: true, roller: vec![] };
        if let Some(dis) = &a.dis {
            let kesin = !(a.metin.contains("Skipper") || a.metin.contains("Filter") || a.metin.contains("ContinueOnIgnoredError") || a.metin.contains("Next:"));
            if KITAPLIK_GIRIS.iter().any(|(y, adlar)| dis.starts_with(y) && adlar.contains(&a.ad.as_str())) {
                return Karar { etki: Etki::Gerekli, kesin, roller: vec![] };
            }
            if dis.starts_with("github.com/go-chi/jwtauth") && matches!(a.ad.as_str(), "Verifier" | "Verify") {
                return Karar { etki: Etki::Istege, kesin: true, roller: vec![] };
            }
            if dis.starts_with("github.com/gofiber/fiber") && dis.ends_with("keyauth") && a.ad == "New" {
                return Karar { etki: Etki::Gerekli, kesin, roller: vec![] };
            }
            if KITAPLIK_ILGISIZ.iter().any(|y| dis.starts_with(y)) && !yetki_adi_mi(&a.ad) {
                return yok();
            }
            if yetki_adi_mi(&a.metin) {
                return Karar { etki: if istege_adi_mi(&a.metin) { Etki::Istege } else { Etki::Gerekli }, kesin: false, roller: vec![] };
            }
            return if KITAPLIK_ILGISIZ.iter().any(|y| dis.starts_with(y)) { yok() } else { Karar { etki: Etki::Bilinmeyen, kesin: false, roller: vec![] } };
        }
        let Some(t) = a.fonk else {
            if yetki_adi_mi(&a.metin) {
                return Karar { etki: if istege_adi_mi(&a.metin) { Etki::Istege } else { Etki::Gerekli }, kesin: false, roller: vec![] };
            }
            return Karar { etki: Etki::Bilinmeyen, kesin: false, roller: vec![] };
        };
        self.tanim_karari(t, &a.argumanlar, &a.ham, 0)
    }

    /// Projedeki ara katmanın tanımından karar: 401 / 403 ile ret, belirteç okuma, bağlama yazma, kitaplık
    /// doğrulayıcısına devir ve rol karşılaştırmaları
    fn tanim_karari(&self, t: Tanim<'_>, argumanlar: &[String], ham: &[String], derinlik: usize) -> Karar {
        let k = self.p.k(t.0);
        let govde = metin(t.1, k);
        let mut red401 = ["StatusUnauthorized", "401", "ErrUnauthorized"].iter().any(|x| govde.contains(x));
        let mut red403 = ["StatusForbidden", "403", "ErrForbidden"].iter().any(|x| govde.contains(x));
        // `false` verilen bool parametre bütün retleri koşullandırıyorsa (`AuthMiddleware(auto401 bool)` ve
        // `if auto401 { c.AbortWithStatus(401) }`) bu çağrıda ret yoktur
        let kapali: Vec<String> =
            self.p.parametreler(t).iter().enumerate().filter(|(i, p)| p.1 == "bool" && ham.get(*i).is_some_and(|h| h == "false")).filter_map(|(_, p)| p.0.clone()).collect();
        if !kapali.is_empty() && (red401 || red403) {
            let dna = crate::dna::go_govde(t.1.child_by_field_name("body").unwrap_or(t.1), k);
            let retler: Vec<&crate::dna::Firlatma> = dna.firlatmalar.iter().filter(|f| matches!(f.durum, Some(401 | 403))).collect();
            let kelime = |m: &str, a: &str| m.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|x| x == a);
            if !retler.is_empty() && retler.iter().all(|f| f.kosul.as_deref().is_some_and(|c| kapali.iter().any(|a| kelime(c, a) && !c.contains(&format!("!{a}"))))) {
                red401 = false;
                red403 = false;
            }
        }
        // Belirteç okuma ve bağlama yazma, çağrılan proje yardımcıları dahil (1 kat)
        let mut metinler = vec![govde.to_string()];
        if derinlik == 0 {
            let tipler = self.yerel_tipler((t.0, t.1));
            for c in self.cagrilar((t.0, t.1), &tipler) {
                metinler.push(metin(c.1, self.p.k(c.0)).to_string());
            }
        }
        let tum = metinler.join(" ");
        let yardimci_red = metinler.iter().skip(1).any(|m| ["StatusUnauthorized", "ErrUnauthorized"].iter().any(|x| m.contains(x)));
        let okur = ["Authorization", "Bearer", "Cookie(", "token", "Token", "jwt", "JWT", "session", "Session", "APIKey", "X-API-Key", "api_key"].iter().any(|x| tum.contains(x));
        let yazar = [".Set(", "context.WithValue", ".Locals(", "WithContext(", ".SetUser("].iter().any(|x| tum.contains(x));
        let mut roller: Vec<String> = vec![];
        if red403 || (red401 && argumanlar.iter().any(|a| !a.is_empty())) {
            roller.extend(argumanlar.iter().filter(|a| !a.is_empty() && a.len() <= 40 && !a.contains('/') && !a.contains(' ')).cloned());
            static R: OnceLock<[Regex; 3]> = OnceLock::new();
            let [kars, ters, bayrak] = R.get_or_init(|| {
                [
                    Regex::new(r#"(?i)\brol\w*\s*(?:!=|==)\s*"([^"]+)""#).unwrap(),
                    Regex::new(r#"(?i)"([^"]+)"\s*(?:!=|==)\s*[\w.]*rol\w*\b"#).unwrap(),
                    Regex::new(r"!\s*[\w.]+\.Is(Admin|Superuser|SuperAdmin|Staff)\b").unwrap(),
                ]
            });
            for re in [kars, ters] {
                roller.extend(re.captures_iter(govde).map(|c| c[1].to_string()));
            }
            roller.extend(bayrak.captures_iter(govde).map(|c| c[1].to_lowercase()));
        }
        roller.sort();
        roller.dedup();
        // Kitaplık doğrulayıcısını koşulsuz döndüren proje fonksiyonu (`func JWT() echo.MiddlewareFunc { return echojwt.WithConfig(...) }`)
        let iceri = &self.p.iceri[t.0];
        let kitaplik =
            KITAPLIK_GIRIS.iter().any(|(y, adlar)| iceri.iter().any(|(takma, yol)| yol.starts_with(y) && adlar.iter().any(|a| govde.contains(&format!("{takma}.{a}(")))));
        if kitaplik {
            return Karar { etki: Etki::Gerekli, kesin: true, roller };
        }
        if red401 || (red403 && !roller.is_empty()) || (red403 && okur) || (yardimci_red && okur && kapali.is_empty()) {
            return Karar { etki: Etki::Gerekli, kesin: false, roller };
        }
        if okur && yazar {
            return Karar { etki: Etki::Istege, kesin: false, roller };
        }
        // Ret, ara katmanın çağırdığı proje fonksiyonunda olabilir (1 kat)
        if derinlik == 0 {
            let tipler = self.yerel_tipler(t);
            for c in self.cagrilar(t, &tipler) {
                let alt = self.tanim_karari(c, &[], &[], 1);
                if alt.etki == Etki::Gerekli && okur {
                    return Karar { etki: Etki::Gerekli, kesin: false, roller };
                }
            }
        }
        Karar { etki: Etki::Yok, kesin: true, roller }
    }
}

// ---- Sonuç ----

/// Router'dan köke bir yol: (önek, ara katmanlar, koşullar, kök)
struct Ulasim<'a> {
    onek: String,
    /// Zincirde öneki çözülemeyen bağ: ifadesi
    dinamik: Option<String>,
    ara: Vec<AraD<'a>>,
    kosullar: Vec<String>,
    kok: usize,
}

impl<'a> Yorum<'a, '_> {
    /// Ara katmanın bu rotaya uygulanıp uygulanmadığı (çatının sıra kuralı)
    fn uygulanir(&self, r: usize, ara_sira: u64, sinir: Option<u64>, rota_sira: u64) -> bool {
        let rd = &self.routerlar[r];
        match rd.cati {
            Cati::Gin => ara_sira < sinir.unwrap_or(rota_sira),
            Cati::Echo if rd.kok_mu => true,
            Cati::Echo => ara_sira < sinir.unwrap_or(rota_sira),
            Cati::Fiber => ara_sira < rota_sira,
            Cati::Chi | Cati::Mux | Cati::Http => true,
        }
    }

    fn ulasimlar(&self, r: usize, sinir: Option<u64>, rota_sira: u64, yol: &str, derinlik: usize) -> Vec<Ulasim<'a>> {
        let rd = &self.routerlar[r];
        let kendi: Vec<AraD<'a>> =
            rd.ara.iter().filter(|(a, s)| self.uygulanir(r, *s, sinir, rota_sira) && a.yol.as_deref().is_none_or(|o| yol.starts_with(o))).map(|(a, _)| a.clone()).collect();
        if rd.baglar.is_empty() || derinlik > 16 {
            return vec![Ulasim { onek: String::new(), dinamik: None, ara: kendi, kosullar: vec![], kok: r }];
        }
        let mut v = vec![];
        for b in &rd.baglar {
            let alt_yol = yol_birlestir(&b.onek, yol);
            for u in self.ulasimlar(b.ust, Some(b.sira), rota_sira, &alt_yol, derinlik + 1) {
                if v.len() >= 16 {
                    break;
                }
                let mut ara = u.ara;
                ara.extend(b.ara.iter().cloned());
                ara.extend(kendi.iter().cloned());
                let mut kosullar = u.kosullar;
                kosullar.extend(b.kosul.iter().cloned());
                v.push(Ulasim { onek: yol_birlestir(&u.onek, &b.onek), dinamik: u.dinamik.or_else(|| b.dinamik.clone()), ara, kosullar, kok: u.kok });
            }
        }
        v
    }

    fn router_kimligi(&self, r: usize) -> String {
        let rd = &self.routerlar[r];
        let ad = if rd.ad.is_empty() { format!("router@{}", rd.satir) } else { rd.ad.clone() };
        let ad = ad.replace(' ', "");
        let ayni = self.routerlar.iter().filter(|x| x.di == rd.di && x.ad == rd.ad).count() > 1;
        if ayni {
            format!("{}:{ad}@{}", self.p.goreli(rd.di), rd.satir)
        } else {
            format!("{}:{ad}", self.p.goreli(rd.di))
        }
    }

    fn sonuc(self) -> Sonuc {
        let p = self.p;
        let mut c = Cozumleyici { p, analizler: HashMap::new() };
        // Rota başına ulaşımlar
        let mut kayitlar: Vec<(usize, Ulasim<'a>, String)> = vec![];
        for (i, r) in self.rotalar.iter().enumerate() {
            for u in self.ulasimlar(r.router, None, r.sira, &r.yol, 0) {
                let tam = yol_normal(&yol_birlestir(&u.onek, &r.yol));
                kayitlar.push((i, u, tam));
            }
        }
        // Ana uygulama: girişten kurulan, en çok rotaya ulaşan kök
        let mut sayilar: BTreeMap<usize, usize> = BTreeMap::new();
        for (_, u, _) in &kayitlar {
            *sayilar.entry(u.kok).or_default() += 1;
        }
        let aday = |sadece_giris: bool| {
            sayilar
                .iter()
                .filter(|(r, _)| !self.routerlar[**r].bagimsiz && (!sadece_giris || self.routerlar[**r].ana_adayi))
                .max_by_key(|(r, n)| (**n, std::cmp::Reverse(**r)))
                .map(|(r, _)| *r)
        };
        let ana_kok = aday(true).or_else(|| aday(false));
        let mut kapilar: Vec<Kapi> = vec![];
        let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
        let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut kosullu_kapilar: Vec<String> = vec![];
        let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
        let mut islem_kimlikleri: BTreeMap<String, Option<String>> = BTreeMap::new();
        let mut veri_onbellegi: HashMap<usize, Option<VeriBag>> = HashMap::new();
        for (i, u, tam) in kayitlar {
            let r = &self.rotalar[i];
            let cati = self.routerlar[r.router].cati;
            let analiz = r.isl.tanim.map(|t| c.analiz(t, cati)).unwrap_or_default();
            let yontemler: Vec<String> = if r.yontem_acik || r.isl.tanim.is_none() {
                if r.yontemler.is_empty() {
                    vec!["ALL".into()]
                } else {
                    r.yontemler.clone()
                }
            } else if !analiz.yontemler.is_empty() {
                analiz.yontemler.clone()
            } else {
                vec!["ALL".into()]
            };
            // Ara katman zinciri: köke kadar devralınanlar + rota düzeyi + işleyiciyi saranlar
            let mut aralar = u.ara.clone();
            aralar.extend(r.ara.iter().cloned());
            aralar.extend(r.isl.ara.iter().cloned());
            let (mut gerekli, mut gerekli_kesin, mut istege, mut bilinmeyen) = (false, false, false, false);
            let mut roller: Vec<String> = vec![];
            let mut guardlar: Vec<String> = vec![];
            let mut ara_tanimlari: Vec<Tanim<'a>> = vec![];
            for a in &aralar {
                let kr = c.karar(a);
                match kr.etki {
                    Etki::Gerekli => {
                        gerekli = true;
                        gerekli_kesin |= kr.kesin;
                        bilinmeyen |= !kr.kesin && a.fonk.is_none();
                    }
                    Etki::Istege => istege = true,
                    Etki::Bilinmeyen => bilinmeyen = true,
                    Etki::Yok => {}
                }
                roller.extend(kr.roller);
                if kr.etki != Etki::Yok {
                    guardlar.push(guvenli::temiz(&kisa_ad(&a.metin)));
                }
                if let Some(t) = a.fonk {
                    if !ara_tanimlari.iter().any(|x| x.1.id() == t.1.id()) {
                        ara_tanimlari.push(t);
                    }
                }
            }
            roller.sort();
            roller.dedup();
            guardlar.dedup();
            let (giris, kanit) = if gerekli {
                (
                    "gerekli",
                    if gerekli_kesin {
                        "kesin"
                    } else if bilinmeyen {
                        "varsayim_bilinmeyen_guard"
                    } else {
                        "varsayim"
                    },
                )
            } else if istege {
                ("istege_bagli", if bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" })
            } else if bilinmeyen {
                ("herkese_acik", "varsayim_bilinmeyen_guard")
            } else {
                ("herkese_acik", "varsayim")
            };
            // Kapının konumu: adlı işleyicinin tanımı; satır içi işleyicide kayıt satırı
            let (kdi, ksatir) = match (r.isl.adli, r.isl.tanim) {
                (Some(t), _) => (t.0, satir(t.1)),
                (None, Some(t)) if t.0 == r.di => (r.di, r.satir),
                (None, Some(t)) => (t.0, satir(t.1)),
                _ => (r.di, r.satir),
            };
            let dosya = p.goreli(kdi);
            let konum = format!("{dosya}:{ksatir}");
            let belge = r.isl.adli.map(|t| p.belge(t)).unwrap_or_default();
            let ozet = belge
                .iter()
                .find_map(|l| l.strip_prefix("@Summary").map(|s| s.trim().to_string()))
                .or_else(|| belge.iter().find(|l| !l.starts_with('@') && !l.is_empty() && !l.ends_with("godoc")).cloned());
            let kalkmis = belge.iter().any(|l| l.starts_with("@Deprecated") || l.starts_with("Deprecated:"));
            if let Some(id) = belge.iter().find_map(|l| l.strip_prefix("@ID").map(|s| s.trim().to_string())).filter(|s| !s.is_empty()) {
                match islem_kimlikleri.get(&konum) {
                    Some(Some(e)) if *e != id => {
                        islem_kimlikleri.insert(konum.clone(), None);
                    }
                    Some(_) => {}
                    None => {
                        islem_kimlikleri.insert(konum.clone(), Some(id));
                    }
                }
            }
            let kontrolcu = r
                .isl
                .alici
                .clone()
                .filter(|a| !a.is_empty())
                .unwrap_or_else(|| pascal(&p.dosyalar[kdi].yol.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()));
            let parametre_adlari: Vec<String> = tam.split('/').filter_map(|x| x.strip_prefix(':')).map(String::from).collect();
            let tamsayilar = yol_tamsayilari(&yol_birlestir(&u.onek, &r.yol));
            let parametreler: Vec<Girdi> = parametre_adlari
                .iter()
                .map(|a| {
                    let tip = analiz
                        .yol_tipleri
                        .get(a)
                        .filter(|t| *t != "string")
                        .cloned()
                        .or_else(|| tamsayilar.contains(a).then(|| "int".to_string()))
                        .unwrap_or_else(|| "string".into());
                    Girdi { ad: Some(a.clone()), tip: Some(tip), sema: None }
                })
                .collect();
            let veri = r.isl.tanim.and_then(|t| veri_onbellegi.entry(t.1.id()).or_insert_with(|| c.veri_bagi(t)).clone());
            let yer = konum.clone();
            if let Some(t) = r.isl.tanim {
                if let std::collections::btree_map::Entry::Vacant(e) = dna_haritasi.entry(yer.clone()) {
                    let v = c.dna_zinciri(t, &ara_tanimlari);
                    if !crate::dna::bos(&v) {
                        e.insert(v);
                    }
                }
            }
            if !analiz.notlar.is_empty() {
                let l = denetlenemeyen.entry(yer.clone()).or_default();
                for n in &analiz.notlar {
                    if !l.contains(n) {
                        l.push(n.clone());
                    }
                }
            }
            kapi_routerlari.insert(yer.clone(), self.router_kimligi(r.router));
            let kosullu = r.kosul.is_some() || !u.kosullar.is_empty();
            let fonksiyon = r.isl.ad.clone().filter(|a| !a.is_empty()).unwrap_or_else(|| fonksiyon_adi(yontemler.first().map(String::as_str).unwrap_or("all"), &tam));
            let (ana, uygulama) = if self.routerlar[u.kok].bagimsiz {
                (false, Some(BAGLANMAMIS.to_string()))
            } else if let Some(d) = &u.dinamik {
                (false, Some(format!("dinamik önek: {}", d.chars().take(80).collect::<String>())))
            } else if Some(u.kok) == ana_kok {
                (true, None)
            } else {
                (false, Some(self.router_kimligi(u.kok)))
            };
            for yontem in &yontemler {
                let govdeli = !matches!(yontem.as_str(), "GET" | "HEAD" | "DELETE" | "OPTIONS");
                let mut govde = analiz.govde.clone();
                if govdeli || yontem == "ALL" {
                    govde.extend(analiz.bind_govde.iter().cloned());
                }
                let mut form = analiz.form.clone();
                let mut sorgu_alanlari = analiz.sorgu.clone();
                for (a, x) in &analiz.form_ya_da_sorgu {
                    if govdeli {
                        form.insert(a.clone(), x.clone());
                    } else {
                        sorgu_alanlari.insert(a.clone(), x.clone());
                    }
                }
                if !form.is_empty() {
                    govde.push(Girdi {
                        ad: None,
                        tip: Some("Form".into()),
                        sema: Some(Sema { dto: "form".into(), dosya: Some(dosya.clone()), bulunamadi: false, kalitim: None, alanlar: form, adaylar: vec![], dizi: false }),
                    });
                }
                let sorgu = if sorgu_alanlari.is_empty() {
                    vec![]
                } else {
                    vec![Girdi {
                        ad: None,
                        tip: None,
                        sema: Some(Sema {
                            dto: "query".into(),
                            dosya: Some(dosya.clone()),
                            bulunamadi: false,
                            kalitim: None,
                            alanlar: sorgu_alanlari,
                            adaylar: vec![],
                            dizi: false,
                        }),
                    }]
                };
                let id = format!("{yontem} {tam}");
                if kosullu {
                    kosullu_kapilar.push(id.clone());
                }
                kapilar.push(Kapi {
                    id,
                    yontem: yontem.clone(),
                    yol: tam.clone(),
                    kontrolcu: kontrolcu.clone(),
                    fonksiyon: fonksiyon.clone(),
                    dosya: dosya.clone(),
                    satir: ksatir,
                    giris: giris.into(),
                    guardlar: guardlar.clone(),
                    roller: roller.clone(),
                    roller_kaniti: (!roller.is_empty()).then(|| "varsayim".into()),
                    ozet: ozet.clone(),
                    govde,
                    sorgu,
                    parametreler: parametreler.clone(),
                    kullanimdan_kalkmis: kalkmis,
                    yerine: None,
                    surum: None,
                    uygulama: uygulama.clone(),
                    ana,
                    giris_kaniti: kanit.into(),
                    veri: veri.clone(),
                    yanit: analiz.yanit.clone(),
                });
            }
        }
        kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)).then(a.ana.cmp(&b.ana).reverse()));
        kapilar.dedup_by(|a, b| a.id == b.id && a.fonksiyon == b.fonksiyon && a.dosya == b.dosya && a.uygulama == b.uygulama);
        kosullu_kapilar.sort();
        kosullu_kapilar.dedup();
        // Router grafı: her router, onu içeren üstler ve içerme satırı
        let mut routerlar_v = vec![];
        for (i, rd) in self.routerlar.iter().enumerate() {
            let ustler: Vec<Value> = rd
                .baglar
                .iter()
                .map(|b| json!({ "id": self.router_kimligi(b.ust), "icerme": format!("{}:{}", p.goreli(b.di), b.satir), "kosul": b.kosul, "kosullu": b.kosul.is_some() }))
                .collect();
            routerlar_v.push(json!({ "id": self.router_kimligi(i), "dosya": p.goreli(rd.di), "satir": rd.satir, "ustler": ustler }));
        }
        // GORM modelleri ve kaskadlar
        let mut veri_modelleri = vec![];
        let mut kaskadlar = vec![];
        let mut veri_dosyalari: BTreeSet<String> = BTreeSet::new();
        for m in p.modeller.values() {
            let k = p.k(m.di);
            let mut alanlar = vec![];
            if metin(m.dugum, k).contains("gorm.Model") {
                for (a, t) in [("ID", "uint"), ("CreatedAt", "time.Time"), ("UpdatedAt", "time.Time"), ("DeletedAt", "gorm.DeletedAt")] {
                    alanlar.push(json!({ "ad": a, "tip": t, "satir": m.satir, "sutun": gorm_sutun(a) }));
                }
            }
            for fd in struct_alanlari(m.dugum) {
                let tip = fd.child_by_field_name("type").map(|t| sikistir(metin(t, k))).unwrap_or_default();
                let et = fd.child_by_field_name("tag").and_then(|t| metin_degismezi(t, k)).map(|t| etiketler(&t)).unwrap_or_default();
                let gorm_et = et.get("gorm").cloned().unwrap_or_default();
                let adlar = adli_cocuklar(fd, "name");
                // İlişki alanı (başka model ya da model dilimi) sütun değildir; kaskad olabilir
                let iliski = p.model(m.di, &tip).map(|x| x.ad.clone());
                if let Some(hedef) = &iliski {
                    let gl = gorm_et.to_lowercase().replace(' ', "");
                    if gl.contains("ondelete:cascade") {
                        let alan_adi = adlar.first().map(|a| metin(*a, k).to_string()).unwrap_or_default();
                        let ait = struct_alanlari(m.dugum).iter().flat_map(|f| adli_cocuklar(*f, "name")).any(|a| metin(a, k) == format!("{alan_adi}ID"));
                        let (e, cocuk) = if ait { (hedef.clone(), m.ad.clone()) } else { (m.ad.clone(), hedef.clone()) };
                        if e != cocuk {
                            kaskadlar.push(json!([e, cocuk, format!("{}:{}", p.goreli(m.di), satir(fd))]));
                        }
                    }
                    continue;
                }
                if gorm_et.split(';').any(|x| x.trim() == "-") || adlar.is_empty() {
                    continue;
                }
                for a in adlar {
                    let ad = metin(a, k);
                    if !ad.chars().next().is_some_and(char::is_uppercase) {
                        continue;
                    }
                    let sutun = gorm_et.split(';').find_map(|x| x.trim().strip_prefix("column:").map(String::from)).unwrap_or_else(|| gorm_sutun(ad));
                    alanlar.push(json!({ "ad": ad, "tip": tip, "satir": satir(fd), "sutun": sutun }));
                }
            }
            veri_dosyalari.insert(p.goreli(m.di));
            veri_modelleri.push(json!({ "ad": m.ad, "tablo": m.tablo, "tablo_kaynagi": m.tablo_kaynagi, "dosya": p.goreli(m.di), "satir": m.satir, "alanlar": alanlar }));
        }
        let ana = ana_kok.map(|r| self.router_kimligi(r));
        // Uygulama geneli giriş ara katmanları: ana uygulamanın kendi ara katmanlarından yetkiyle ilgili olanlar
        let genel: Vec<String> = ana_kok
            .map(|r| {
                self.routerlar[r].ara.iter().filter(|(a, _)| matches!(c.karar(a).etki, Etki::Gerekli | Etki::Istege)).map(|(a, _)| guvenli::temiz(&kisa_ad(&a.metin))).collect()
            })
            .unwrap_or_default();
        let mut dinamik = self.dinamik.clone();
        dinamik.sort();
        dinamik.dedup();
        Sonuc {
            kapilar,
            bilgi: json!({
                "genel_onek": "", "genel_guardlar": genel, "ana_uygulama": ana, "surum_secimi": null,
                "kosullu_kapilar": kosullu_kapilar, "denetlenemeyen": denetlenemeyen, "dna": dna_haritasi, "kaskadlar": kaskadlar,
                "routerlar": routerlar_v, "kapi_routerlari": kapi_routerlari, "veri_modelleri": veri_modelleri,
                "islem_kimlikleri": islem_kimlikleri.into_iter().filter_map(|(a, k)| k.map(|k| (a, k))).collect::<BTreeMap<_, _>>(),
                "dinamik_kayitlar": dinamik,
            }),
            veri_dosyalari: veri_dosyalari.into_iter().collect(),
        }
    }
}

/// Ara katmanın görünen adı: uzun argümanlı çağrıda yalnızca çağrılan (`jwtware.New(…)`)
fn kisa_ad(m: &str) -> String {
    if m.chars().count() <= 60 {
        return m.to_string();
    }
    match m.find('(') {
        Some(i) => format!("{}(…)", &m[..i]),
        None => m.chars().take(60).collect(),
    }
}

/// GORM sütun adı: CreatedAt -> created_at, UserID -> user_id
fn gorm_sutun(ad: &str) -> String {
    let t = gorm_tablo_adi(ad);
    // Tablo adlandırmasının çoğul eki geri alınır
    if let Some(x) = t.strip_suffix("ies") {
        format!("{x}y")
    } else if let Some(x) = t.strip_suffix("es").filter(|x| x.ends_with('s') || x.ends_with('x') || x.ends_with("ch") || x.ends_with("sh")) {
        x.to_string()
    } else {
        t.strip_suffix('s').unwrap_or(&t).to_string()
    }
}

/// Go HTTP adaptörü: kapılar ve yan bilgi
pub(crate) fn kapilar(kok: &Path, dosyalar: &[Dosya], moduller: &[Modul]) -> Sonuc {
    let p = Proje::yeni(kok, dosyalar, moduller);
    let mut y = Yorum {
        p: &p,
        routerlar: vec![],
        taslaklar: vec![],
        rotalar: vec![],
        sira: 0,
        adim: 0,
        yigin: vec![],
        yorumlanan: HashSet::new(),
        alanlar: HashMap::new(),
        paket_degerleri: HashMap::new(),
        ana_mod: false,
        varsayilan_mux: None,
        dinamik: vec![],
    };
    y.yorumla();
    y.sonuc()
}
