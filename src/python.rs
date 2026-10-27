//! Python kaynak kodundan çatıdan bağımsız model (tree-sitter-python): içe aktarmalar, üst düzey atamalar, dekoratörlü
//! fonksiyonlar ve parametreleri, sınıflar ve tipli alanları, modül düzeyindeki çağrılar. Çatı anlamı (FastAPI rotaları,
//! bağımlılıklar, Pydantic modelleri) bu modelin üstünde `fastapi.rs` ile verilir.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tree_sitter::{Node, Parser};

use crate::guvenli;

#[derive(Debug, Clone, Default)]
pub struct Cagri {
    /// Çağrılan ad: "APIRouter", "router.get", "api_router.include_router", "Depends"
    pub ad: String,
    pub argumanlar: Vec<String>,
    pub anahtarli: Vec<(String, String)>,
    /// Çağrı her çalışmada gerçekleşmeyebilir (if, try, while)
    pub kosullu: bool,
    /// Çağrı bir `for` döngüsünün içinde: (döngü değişkeni, gezilen ifade), ör. ("router", "API_ROUTERS")
    pub dongu: Option<(String, String)>,
    /// Çağrının satırı
    pub satir: usize,
    /// Çağrı bir `if` altındaysa koşulun kod metni (`settings.ENVIRONMENT == "local"`; `else` kolu için `not (...)`)
    pub kosul: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Parametre {
    pub ad: String,
    pub tip: Option<String>,
    pub varsayilan: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Fonksiyon {
    pub ad: String,
    pub dekoratorler: Vec<Cagri>,
    pub parametreler: Vec<Parametre>,
    pub belge: Option<String>,
    pub satir: usize,
    /// Gövdedeki çağrılı atamalar (fabrika fonksiyonu: `api_app = FastAPI()`)
    pub ic_atama_cagrilari: HashMap<String, Cagri>,
    /// Gövdedeki ifade çağrıları (fabrika fonksiyonu: `api_app.include_router(r)`), döngü ve koşul bilgisiyle
    pub ic_cagrilar: Vec<Cagri>,
    /// İlk koşulsuz `return` ifadesinin metni (parametre yardımcıları: `return Body(10, ge=1)`)
    pub donus: Option<String>,
    /// Gövdenin metni (en fazla 64 KiB): veri bağları (ORM çağrıları) buradan okunur
    pub govde_metni: String,
    /// Dönüş tipi bildirimi (`-> User | None`)
    pub donus_tipi: Option<String>,
    /// İçinde tanımlanan fonksiyonlar (fabrika: `def kur(): router = APIRouter(); @router.get(...) def x(): ...`)
    pub ic_fonksiyonlar: Vec<Fonksiyon>,
    /// İç fonksiyonsa tanımlandığı fonksiyonun adı
    pub kapsam: Option<String>,
    /// Koşul, döngü ya da try içinde tanımlanmış iç fonksiyon (ör. `if ayar.X:` altında kaydedilen rota)
    pub kosullu_tanim: bool,
    /// Fonksiyon gövdesindeki içe aktarmalar (`def kur(app): from x import r`): ad -> (modül, öğe)
    pub ic_iceaktarmalar: Vec<(String, (String, Option<String>))>,
    /// Davranış özeti öğeleri: `raise` ve koşulları, iki yollu ifadeler, yumuşak silme işareti
    pub dna: crate::dna::GovdeDna,
}

#[derive(Debug, Clone)]
pub struct Alan {
    pub ad: String,
    pub tip: Option<String>,
    pub varsayilan: Option<String>,
    pub satir: usize,
}

#[derive(Debug, Clone)]
pub struct Sinif {
    pub ad: String,
    pub tabanlar: Vec<String>,
    pub anahtarli: Vec<(String, String)>,
    pub alanlar: Vec<Alan>,
    /// `__call__` yöntemi: örneği bağımlılık olarak verilen sınıf (`Depends(RolDenetimi(["admin"]))`)
    pub cagri_yontemi: Option<Box<Fonksiyon>>,
    /// Sınıf gövdesindeki yöntemler, dekoratörleriyle (Django / DRF görünümleri, Flask MethodView, izin sınıfları)
    pub metotlar: Vec<Fonksiyon>,
    /// İç sınıflar (`class Meta:`)
    pub ic_siniflar: Vec<Sinif>,
    /// Sınıf dekoratörleri (`@method_decorator(login_required, name="dispatch")`, `@blp.route("/x")`)
    pub dekoratorler: Vec<Cagri>,
    pub satir: usize,
}

/// Artırımlı atama (`urlpatterns += [...]`): koşul altındakiler dahil
#[derive(Debug, Clone)]
pub struct Artirim {
    pub ad: String,
    pub ifade: String,
    pub satir: usize,
    pub kosullu: bool,
    pub kosul: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Dosya {
    pub yol: PathBuf,
    /// Noktalı modül adı (kökten): app/api/routes/items.py -> app.api.routes.items
    pub modul: String,
    /// İçe aktarılan ad -> (modül, öğe): `from app.api import deps` -> deps -> ("app.api", Some("deps"))
    pub iceaktarmalar: HashMap<String, (String, Option<String>)>,
    /// Üst düzey atama: ad -> ifade metni
    pub atamalar: HashMap<String, String>,
    /// Üst düzey atamanın çağrı biçimi (sağ taraf çağrıysa)
    pub atama_cagrilari: HashMap<String, Cagri>,
    pub fonksiyonlar: Vec<Fonksiyon>,
    pub siniflar: Vec<Sinif>,
    /// Modül düzeyindeki ifade çağrıları (ör. include_router), koşullu olanlar işaretli
    pub cagrilar: Vec<Cagri>,
    pub satir_sayisi: usize,
    pub hatali_satirlar: Vec<usize>,
    /// Üst düzey atamaların satırı
    pub atama_satirlari: HashMap<String, usize>,
    /// Modül düzeyindeki artırımlı atamalar (`urlpatterns += router.urls`)
    pub artirimlar: Vec<Artirim>,
    /// Yıldızlı içe aktarmaların modülleri (`from .base import *`)
    pub yildizlar: Vec<String>,
}

fn metin(n: Node, k: &[u8]) -> String {
    n.utf8_text(k).unwrap_or("").to_string()
}

fn cagri(n: Node, k: &[u8], kosullu: bool) -> Option<Cagri> {
    if n.kind() != "call" {
        return None;
    }
    let ad = metin(n.child_by_field_name("function")?, k);
    let mut c = Cagri { ad, kosullu, satir: n.start_position().row + 1, ..Default::default() };
    if let Some(a) = n.child_by_field_name("arguments") {
        let mut w = a.walk();
        for x in a.named_children(&mut w) {
            match x.kind() {
                "keyword_argument" => {
                    let ad = x.child_by_field_name("name").map(|v| metin(v, k)).unwrap_or_default();
                    let deger = x.child_by_field_name("value").map(|v| metin(v, k)).unwrap_or_default();
                    c.anahtarli.push((ad, deger));
                }
                "comment" => {}
                _ => c.argumanlar.push(metin(x, k)),
            }
        }
    }
    Some(c)
}

fn belge(govde: Node, k: &[u8]) -> Option<String> {
    let mut w = govde.walk();
    let ilk = govde.named_children(&mut w).next()?;
    let s = (ilk.kind() == "expression_statement").then(|| ilk.named_child(0)).flatten().filter(|s| s.kind() == "string")?;
    let ham = metin(s, k);
    let ic = ham.trim_start_matches(['r', 'u', 'R', 'U']).trim_matches('"').trim_matches('\'').trim();
    ic.lines().map(str::trim).find(|l| !l.is_empty()).map(guvenli::temiz)
}

fn parametreler(p: Node, k: &[u8]) -> Vec<Parametre> {
    let mut v = vec![];
    let mut w = p.walk();
    for x in p.named_children(&mut w) {
        let (ad, tip, varsayilan) = match x.kind() {
            "identifier" => (metin(x, k), None, None),
            "typed_parameter" => {
                let ad = x.named_child(0).filter(|c| c.kind() == "identifier").map(|c| metin(c, k));
                let Some(ad) = ad else { continue };
                (ad, x.child_by_field_name("type").map(|t| metin(t, k)), None)
            }
            "default_parameter" => (x.child_by_field_name("name").map(|c| metin(c, k)).unwrap_or_default(), None, x.child_by_field_name("value").map(|c| metin(c, k))),
            "typed_default_parameter" => (
                x.child_by_field_name("name").map(|c| metin(c, k)).unwrap_or_default(),
                x.child_by_field_name("type").map(|t| metin(t, k)),
                x.child_by_field_name("value").map(|c| metin(c, k)),
            ),
            _ => continue,
        };
        if !ad.is_empty() {
            v.push(Parametre { ad, tip, varsayilan });
        }
    }
    v
}

fn fonksiyon(n: Node, k: &[u8], dekoratorler: Vec<Cagri>) -> Option<Fonksiyon> {
    let mut f = Fonksiyon {
        ad: metin(n.child_by_field_name("name")?, k),
        dekoratorler,
        parametreler: n.child_by_field_name("parameters").map(|p| parametreler(p, k)).unwrap_or_default(),
        belge: n.child_by_field_name("body").and_then(|b| belge(b, k)),
        satir: n.start_position().row + 1,
        ic_atama_cagrilari: HashMap::new(),
        ic_cagrilar: vec![],
        donus: None,
        donus_tipi: n.child_by_field_name("return_type").map(|t| metin(t, k)),
        ic_fonksiyonlar: vec![],
        kapsam: None,
        kosullu_tanim: false,
        ic_iceaktarmalar: vec![],
        govde_metni: n.child_by_field_name("body").map(|b| metin(b, k).chars().take(64 * 1024).collect()).unwrap_or_default(),
        dna: n.child_by_field_name("body").map(|b| crate::dna::py_govde(b, k)).unwrap_or_default(),
    };
    if let Some(g) = n.child_by_field_name("body") {
        govde_gez(g, k, &mut f, false, None);
    }
    Some(f)
}

/// Fonksiyon gövdesi: çağrılı atamalar ve ifade çağrıları (iç fonksiyonlara girilmez)
fn govde_gez(g: Node, k: &[u8], f: &mut Fonksiyon, kosullu: bool, dongu: Option<(String, String)>) {
    let mut w = g.walk();
    for x in g.named_children(&mut w) {
        match x.kind() {
            "expression_statement" => {
                let Some(ic) = x.named_child(0) else { continue };
                match ic.kind() {
                    "assignment" => {
                        let (Some(sol), Some(sag)) = (ic.child_by_field_name("left"), ic.child_by_field_name("right")) else { continue };
                        if let (true, Some(mut c)) = (sol.kind() == "identifier", cagri(sag, k, kosullu)) {
                            c.dongu = dongu.clone();
                            f.ic_atama_cagrilari.insert(metin(sol, k), c);
                        }
                    }
                    "call" => {
                        if let Some(mut c) = cagri(ic, k, kosullu) {
                            c.dongu = dongu.clone();
                            f.ic_cagrilar.push(c);
                        }
                    }
                    _ => {}
                }
            }
            "import_from_statement" | "import_statement" => f.ic_iceaktarmalar.extend(iceaktarma_oku(x, k)),
            "return_statement" if !kosullu && dongu.is_none() && f.donus.is_none() => {
                f.donus = x.named_child(0).map(|r| metin(r, k));
            }
            "for_statement" => {
                let degisken = x.child_by_field_name("left").filter(|l| l.kind() == "identifier").map(|l| metin(l, k));
                let ifade = x.child_by_field_name("right").map(|r| metin(r, k));
                let d = degisken.zip(ifade).or(dongu.clone());
                if let Some(b) = x.child_by_field_name("body") {
                    govde_gez(b, k, f, kosullu, d);
                }
            }
            "if_statement" | "try_statement" | "while_statement" | "with_statement" => {
                let kos = kosullu || x.kind() != "with_statement";
                let mut w2 = x.walk();
                for c in x.named_children(&mut w2) {
                    let once = f.ic_cagrilar.len();
                    if c.kind() == "block" {
                        govde_gez(c, k, f, kos, dongu.clone());
                    } else {
                        let mut w3 = c.walk();
                        for b in c.named_children(&mut w3).filter(|b| b.kind() == "block") {
                            govde_gez(b, k, f, kos, dongu.clone());
                        }
                    }
                    if let Some(kosul) = kol_kosulu(x, c, k) {
                        for cg in &mut f.ic_cagrilar[once..] {
                            cg.kosul.get_or_insert_with(|| kosul.clone());
                        }
                    }
                }
            }
            // İç fonksiyon: dekoratörleriyle birlikte, koşul altındaysa işaretli; düzleştirme ayrıştırmanın sonunda
            "decorated_definition" => {
                let mut w2 = x.walk();
                let dekoratorler: Vec<Cagri> = x
                    .named_children(&mut w2)
                    .filter(|c| c.kind() == "decorator")
                    .filter_map(|c| c.named_child(0))
                    .filter_map(|c| if c.kind() == "call" { cagri(c, k, false) } else { Some(Cagri { ad: metin(c, k), ..Default::default() }) })
                    .collect();
                if let Some(t) = x.child_by_field_name("definition").filter(|t| t.kind() == "function_definition") {
                    f.ic_fonksiyonlar.extend(fonksiyon(t, k, dekoratorler).map(|mut i| {
                        i.kosullu_tanim = kosullu || dongu.is_some();
                        i
                    }));
                }
            }
            "function_definition" => f.ic_fonksiyonlar.extend(fonksiyon(x, k, vec![]).map(|mut i| {
                i.kosullu_tanim = kosullu || dongu.is_some();
                i
            })),
            _ => {}
        }
    }
}

/// `if` deyiminin bir kolunun koşulu: ana blok için koşulun metni, `elif` için kendi koşulu, `else` için `not (...)`;
/// `if` dışı deyimlerde (try, while, with) None
fn kol_kosulu(deyim: Node, kol: Node, k: &[u8]) -> Option<String> {
    if deyim.kind() != "if_statement" {
        return None;
    }
    let kosul = || deyim.child_by_field_name("condition").map(|c| metin(c, k).split_whitespace().collect::<Vec<_>>().join(" "));
    match kol.kind() {
        "block" => kosul(),
        "elif_clause" => kol.child_by_field_name("condition").map(|c| metin(c, k).split_whitespace().collect::<Vec<_>>().join(" ")),
        "else_clause" => kosul().map(|c| format!("not ({c})")),
        _ => None,
    }
}

/// `decorated_definition` düğümünün dekoratörleri (çağrı ya da düz ad)
fn dekoratorler(x: Node, k: &[u8]) -> Vec<Cagri> {
    let mut w = x.walk();
    x.named_children(&mut w)
        .filter(|c| c.kind() == "decorator")
        .filter_map(|c| c.named_child(0))
        .map(|c| cagri(c, k, false).unwrap_or(Cagri { ad: metin(c, k), satir: c.start_position().row + 1, ..Default::default() }))
        .collect()
}

fn sinif(n: Node, k: &[u8]) -> Option<Sinif> {
    let mut s = Sinif {
        ad: metin(n.child_by_field_name("name")?, k),
        tabanlar: vec![],
        anahtarli: vec![],
        alanlar: vec![],
        cagri_yontemi: None,
        metotlar: vec![],
        ic_siniflar: vec![],
        dekoratorler: vec![],
        satir: n.start_position().row + 1,
    };
    if let Some(a) = n.child_by_field_name("superclasses") {
        let mut w = a.walk();
        for x in a.named_children(&mut w) {
            match x.kind() {
                "keyword_argument" => s
                    .anahtarli
                    .push((x.child_by_field_name("name").map(|v| metin(v, k)).unwrap_or_default(), x.child_by_field_name("value").map(|v| metin(v, k)).unwrap_or_default())),
                "comment" => {}
                _ => s.tabanlar.push(metin(x, k)),
            }
        }
    }
    if let Some(g) = n.child_by_field_name("body") {
        let mut w = g.walk();
        for x in g.named_children(&mut w) {
            let tanim = match x.kind() {
                "function_definition" | "class_definition" => Some(x),
                "decorated_definition" => x.child_by_field_name("definition").filter(|t| matches!(t.kind(), "function_definition" | "class_definition")),
                _ => None,
            };
            if let Some(t) = tanim {
                let dek = if x.kind() == "decorated_definition" { dekoratorler(x, k) } else { vec![] };
                if t.kind() == "class_definition" {
                    if let Some(mut ic) = sinif(t, k) {
                        ic.dekoratorler = dek;
                        s.ic_siniflar.push(ic);
                    }
                    continue;
                }
                if t.child_by_field_name("name").is_some_and(|a| metin(a, k) == "__call__") {
                    s.cagri_yontemi = fonksiyon(t, k, vec![]).map(Box::new);
                }
                s.metotlar.extend(fonksiyon(t, k, dek));
                continue;
            }
            let Some(a) = (x.kind() == "expression_statement").then(|| x.named_child(0)).flatten().filter(|a| a.kind() == "assignment") else { continue };
            let Some(sol) = a.child_by_field_name("left") else { continue };
            if sol.kind() != "identifier" {
                continue;
            }
            // Tipsiz atamalar da tutulur (Enum üyeleri: ROMAN = "roman"); model alanı yalnızca tipli olandır
            s.alanlar.push(Alan {
                ad: metin(sol, k),
                tip: a.child_by_field_name("type").map(|t| metin(t, k)),
                varsayilan: a.child_by_field_name("right").map(|r| metin(r, k)),
                satir: x.start_position().row + 1,
            });
        }
    }
    Some(s)
}

/// Modül gövdesini gezer: üst düzey tanımlar ve (koşullu bloklar dahil) modül düzeyi çağrılar
/// `from m import a as b` / `import m.n as o`: (yerel ad, (modül, öğe))
fn iceaktarma_oku(x: Node, k: &[u8]) -> Vec<(String, (String, Option<String>))> {
    let mut v = vec![];
    let mut w2 = x.walk();
    match x.kind() {
        "import_from_statement" => {
            let modul = x.child_by_field_name("module_name").map(|m| metin(m, k)).unwrap_or_default();
            for ad in x.children_by_field_name("name", &mut w2) {
                let (asil, takma) = if ad.kind() == "aliased_import" {
                    (ad.child_by_field_name("name").map(|v| metin(v, k)).unwrap_or_default(), ad.child_by_field_name("alias").map(|v| metin(v, k)).unwrap_or_default())
                } else {
                    (metin(ad, k), metin(ad, k))
                };
                v.push((takma, (modul.clone(), Some(asil))));
            }
        }
        "import_statement" => {
            for ad in x.children_by_field_name("name", &mut w2) {
                let (asil, takma) = if ad.kind() == "aliased_import" {
                    let a = ad.child_by_field_name("name").map(|v| metin(v, k)).unwrap_or_default();
                    (a.clone(), ad.child_by_field_name("alias").map(|v| metin(v, k)).unwrap_or(a))
                } else {
                    let a = metin(ad, k);
                    (a.clone(), a.split('.').next().unwrap_or("").to_string())
                };
                v.push((takma, (asil, None)));
            }
        }
        _ => {}
    }
    v
}

/// `if` kolu (blok ya da elif/else): bloğu koşullu gezer
fn gez_kollu(kol: Node, k: &[u8], d: &mut Dosya) {
    if kol.kind() == "block" {
        gez(kol, k, d, true);
        return;
    }
    let mut w = kol.walk();
    for b in kol.named_children(&mut w).filter(|b| b.kind() == "block") {
        gez(b, k, d, true);
    }
}

fn gez(govde: Node, k: &[u8], d: &mut Dosya, kosullu: bool) {
    let mut w = govde.walk();
    for x in govde.named_children(&mut w) {
        match x.kind() {
            "import_from_statement" | "import_statement" => {
                let mut w2 = x.walk();
                if x.kind() == "import_from_statement" && x.named_children(&mut w2).any(|c| c.kind() == "wildcard_import") {
                    d.yildizlar.extend(x.child_by_field_name("module_name").map(|m| metin(m, k)));
                }
                d.iceaktarmalar.extend(iceaktarma_oku(x, k))
            }
            "expression_statement" => {
                let Some(ic) = x.named_child(0) else { continue };
                match ic.kind() {
                    "assignment" if !kosullu => {
                        let (Some(sol), Some(sag)) = (ic.child_by_field_name("left"), ic.child_by_field_name("right")) else { continue };
                        if sol.kind() == "identifier" {
                            let ad = metin(sol, k);
                            if let Some(c) = cagri(sag, k, false) {
                                d.atama_cagrilari.insert(ad.clone(), c);
                            }
                            d.atama_satirlari.insert(ad.clone(), x.start_position().row + 1);
                            d.atamalar.insert(ad, metin(sag, k));
                        }
                    }
                    "augmented_assignment" => {
                        let (Some(sol), Some(sag)) = (ic.child_by_field_name("left"), ic.child_by_field_name("right")) else { continue };
                        let artis = ic.child_by_field_name("operator").is_some_and(|o| metin(o, k) == "+=");
                        if sol.kind() == "identifier" && artis {
                            d.artirimlar.push(Artirim { ad: metin(sol, k), ifade: metin(sag, k), satir: x.start_position().row + 1, kosullu, kosul: None });
                        }
                    }
                    "call" => {
                        if let Some(c) = cagri(ic, k, kosullu) {
                            d.cagrilar.push(c);
                        }
                    }
                    _ => {}
                }
            }
            "decorated_definition" => {
                let dekoratorler = dekoratorler(x, k);
                match x.child_by_field_name("definition") {
                    Some(f) if f.kind() == "function_definition" && !kosullu => d.fonksiyonlar.extend(fonksiyon(f, k, dekoratorler)),
                    Some(c) if c.kind() == "class_definition" && !kosullu => d.siniflar.extend(sinif(c, k).map(|mut s| {
                        s.dekoratorler = dekoratorler;
                        s
                    })),
                    _ => {}
                }
            }
            "function_definition" if !kosullu => d.fonksiyonlar.extend(fonksiyon(x, k, vec![])),
            "class_definition" if !kosullu => d.siniflar.extend(sinif(x, k)),
            // Modül düzeyi döngü: `for r in ROUTERS: app.include_router(r)` her çalışmada gerçekleşir
            "for_statement" => {
                let degisken = x.child_by_field_name("left").filter(|l| l.kind() == "identifier").map(|l| metin(l, k));
                let ifade = x.child_by_field_name("right").map(|r| metin(r, k));
                if let (Some(b), Some(dongu)) = (x.child_by_field_name("body"), degisken.zip(ifade)) {
                    let once = d.cagrilar.len();
                    gez(b, k, d, kosullu);
                    for c in &mut d.cagrilar[once..] {
                        c.dongu.get_or_insert_with(|| dongu.clone());
                    }
                }
            }
            // Koşullu bloklar: içindeki çağrılar koşullu (rota kaydı her ortamda olmayabilir)
            "if_statement" | "try_statement" | "while_statement" | "with_statement" => {
                let mut w2 = x.walk();
                for c in x.named_children(&mut w2) {
                    let once = d.cagrilar.len();
                    let once_artirim = d.artirimlar.len();
                    if let Some(kosul) = kol_kosulu(x, c, k) {
                        gez_kollu(c, k, d);
                        for cg in &mut d.cagrilar[once..] {
                            cg.kosul.get_or_insert_with(|| kosul.clone());
                        }
                        for a in &mut d.artirimlar[once_artirim..] {
                            a.kosul.get_or_insert_with(|| kosul.clone());
                        }
                        continue;
                    }
                    if c.kind() == "block" {
                        gez(c, k, d, true);
                    } else {
                        let mut w3 = c.walk();
                        for b in c.named_children(&mut w3).filter(|b| b.kind() == "block") {
                            gez(b, k, d, true);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn hatalar(n: Node, out: &mut Vec<usize>) {
    if n.is_error() || n.is_missing() {
        out.push(n.start_position().row + 1);
        return;
    }
    if !n.has_error() {
        return;
    }
    let mut w = n.walk();
    for c in n.children(&mut w) {
        hatalar(c, out);
    }
}

pub fn ayristir_metin(yol: &Path, modul: &str, kaynak: &str) -> Option<Dosya> {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_python::LANGUAGE.into()).ok()?;
    let agac = p.parse(kaynak, None)?;
    let k = kaynak.as_bytes();
    let mut d = Dosya {
        yol: yol.to_path_buf(),
        modul: modul.to_string(),
        iceaktarmalar: HashMap::new(),
        atamalar: HashMap::new(),
        atama_cagrilari: HashMap::new(),
        fonksiyonlar: vec![],
        siniflar: vec![],
        cagrilar: vec![],
        satir_sayisi: kaynak.lines().count(),
        hatali_satirlar: vec![],
        atama_satirlari: HashMap::new(),
        artirimlar: vec![],
        yildizlar: vec![],
    };
    gez(agac.root_node(), k, &mut d, false);
    // İç fonksiyonlar (her derinlikte) dosyanın fonksiyonlarına: kapsam tanımlandıkları fonksiyonun adı; koşul altındaki
    // bir fonksiyonun içindekiler de koşullu
    let mut sira = 0;
    while sira < d.fonksiyonlar.len() && d.fonksiyonlar.len() < 10_000 {
        let ust = d.fonksiyonlar[sira].ad.clone();
        let ust_kosullu = d.fonksiyonlar[sira].kosullu_tanim;
        let icler = std::mem::take(&mut d.fonksiyonlar[sira].ic_fonksiyonlar);
        for mut i in icler {
            i.kapsam = Some(ust.clone());
            i.kosullu_tanim |= ust_kosullu;
            d.fonksiyonlar.push(i);
        }
        sira += 1;
    }
    // Fonksiyon içi içe aktarmalar modülün adlarına eklenir; aynı ad modül düzeyinde varsa o kalır
    let icler: Vec<(String, (String, Option<String>))> = d.fonksiyonlar.iter().flat_map(|f| f.ic_iceaktarmalar.iter().cloned()).collect();
    for (a, v) in icler {
        d.iceaktarmalar.entry(a).or_insert(v);
    }
    hatalar(agac.root_node(), &mut d.hatali_satirlar);
    Some(d)
}

/// Python bağımlılık dosyaları (çatı seçimi bunlara bakar); `requirements/` klasöründeki `*.txt` dosyaları da okunur
pub(crate) const BAGIMLILIK_DOSYALARI: [&str; 6] = ["pyproject.toml", "requirements.txt", "requirements.in", "setup.py", "setup.cfg", "Pipfile"];

/// Bağımlılık dosyalarında (yorum satırları hariç) paket adı geçiyor mu
fn bagimlilik_var(kok: &Path, paket: &str) -> bool {
    let mut metinler: Vec<String> = BAGIMLILIK_DOSYALARI.iter().filter_map(|d| guvenli::oku(&kok.join(d))).collect();
    if let Ok(oge) = std::fs::read_dir(kok.join("requirements")) {
        let mut yollar: Vec<PathBuf> = oge.flatten().map(|g| g.path()).filter(|p| p.extension().is_some_and(|e| e == "txt")).collect();
        yollar.sort();
        metinler.extend(yollar.iter().take(16).filter_map(|p| guvenli::oku(p)));
    }
    metinler.iter().any(|m| m.lines().any(|l| !l.trim_start().starts_with('#') && l.to_lowercase().contains(paket)))
}

/// Python + FastAPI uygulaması mı: bağımlılık dosyalarında fastapi geçiyor mu
pub fn fastapi_mi(kok: &Path) -> bool {
    bagimlilik_var(kok, "fastapi")
}

/// Python + Django uygulaması mı: bağımlılık dosyalarında django geçiyor ya da kökte `DJANGO_SETTINGS_MODULE` kuran
/// bir manage.py var
pub fn django_mi(kok: &Path) -> bool {
    bagimlilik_var(kok, "django") || guvenli::oku(&kok.join("manage.py")).is_some_and(|m| m.contains("DJANGO_SETTINGS_MODULE"))
}

/// Python + Flask uygulaması mı: bağımlılık dosyalarında flask geçiyor mu
pub fn flask_mi(kok: &Path) -> bool {
    bagimlilik_var(kok, "flask")
}

/// Python çatısı (adaptör seçimi)
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum PyCati {
    FastApi,
    Django,
    Flask,
}

/// Django'nun modül kökü: en sığdaki manage.py'nin klasörü (proje bir alt klasördeyse modül adları ona göre)
fn manage_klasoru(kok: &Path) -> Option<PathBuf> {
    let mut kuyruk = vec![(kok.to_path_buf(), 0)];
    while let Some((k, derinlik)) = kuyruk.first().cloned() {
        kuyruk.remove(0);
        if k.join("manage.py").is_file() {
            return Some(k);
        }
        if derinlik >= 2 {
            continue;
        }
        let Ok(oge) = std::fs::read_dir(&k) else { continue };
        let mut alt: Vec<PathBuf> = oge
            .flatten()
            .map(|g| g.path())
            .filter(|p| p.is_dir() && !p.file_name().and_then(|a| a.to_str()).is_some_and(|a| a.starts_with('.') || matches!(a, "node_modules" | "venv" | "site-packages")))
            .collect();
        alt.sort();
        kuyruk.extend(alt.into_iter().map(|a| (a, derinlik + 1)));
    }
    None
}

/// Uygulamanın adı ve sürümü: pyproject.toml [project]
pub fn paket(kok: &Path) -> Value {
    let t: toml::Table = guvenli::oku(&kok.join("pyproject.toml")).and_then(|m| m.parse().ok()).unwrap_or_default();
    let p = t.get("project").or_else(|| t.get("tool").and_then(|x| x.get("poetry")));
    json!({ "name": p.and_then(|x| x.get("name")).and_then(|x| x.as_str()), "version": p.and_then(|x| x.get("version")).and_then(|x| x.as_str()) })
}

/// Python dosyasının modül adı: kökten göreli yol, `__init__` paketin kendisi
fn modul_adi(kok: &Path, yol: &Path) -> String {
    let g = yol.strip_prefix(kok).unwrap_or(yol).with_extension("");
    let parcalar: Vec<String> = g.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    let parcalar: Vec<&str> = parcalar.iter().map(String::as_str).filter(|p| *p != "__init__").collect();
    parcalar.join(".")
}

/// Keşif, paralel ayrıştırma ve çatının adaptörü (FastAPI, Django, Flask)
pub(crate) fn topla(kok: &Path, cati: PyCati, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<Dosya>>, t0: Instant) -> crate::Toplam {
    let mut yollar = vec![];
    let (mut atlanan, mut buyuk) = (0usize, vec![]);
    for g in ignore::WalkBuilder::new(kok).hidden(true).git_ignore(true).build().flatten() {
        let p = g.path();
        if g.path_is_symlink() || !p.is_file() {
            if g.path_is_symlink() {
                atlanan += 1;
            }
            continue;
        }
        // Sanal ortamlar, bağımlılıklar, göç betikleri ve testler uygulamanın API'si değildir. Yol uygulama köküne göre
        // denetlenir: uygulamanın kendisi bir `tests/` klasörünün altında olabilir
        let goreli = p.strip_prefix(kok).unwrap_or(p);
        if goreli
            .components()
            .any(|c| ["site-packages", "node_modules", ".venv", "venv", "__pycache__", "alembic", "migrations", "tests", "test"].contains(&c.as_os_str().to_str().unwrap_or("")))
        {
            atlanan += 1;
            continue;
        }
        if p.extension().and_then(|e| e.to_str()).is_some_and(|e| crate::izle::PY_UZANTILARI.contains(&e)) {
            if g.metadata().is_ok_and(|m| m.len() > crate::EN_BUYUK_DOSYA) {
                buyuk.push(guvenli::goreli(kok, p));
                continue;
            }
            yollar.push(p.to_path_buf());
        } else {
            atlanan += 1;
        }
    }
    let t_kesif = t0.elapsed();
    // Modül adı Python'un arama yolu gibi: dosya `src/` düzenindeyse src'ye, değilse uygulama köküne göre
    // Değişmeyen dosya önbellekten (sezgi calistir): yalnızca değişen dosya yeniden ayrıştırılır
    // Django: modül adları manage.py'nin klasörüne göre (ROOT_URLCONF ve INSTALLED_APPS o kökten yazılır)
    let src = kok.join("src");
    let django_kok = if cati == PyCati::Django { manage_klasoru(kok) } else { None };
    let (sonuclar, okunan, ayristirilan) = crate::kesif::ayristir_hepsi(&yollar, onbellek, |p, b| {
        let m = String::from_utf8(b).ok()?;
        let modul = match &django_kok {
            Some(dk) if p.starts_with(dk) => modul_adi(dk, p),
            _ if p.starts_with(&src) => modul_adi(&src, p),
            _ => modul_adi(kok, p),
        };
        ayristir_metin(p, &modul, &m)
    });
    let dosyalar: Vec<Dosya> = sonuclar.into_iter().map(|(_, d)| d).collect();
    let t_ayristirma = t0.elapsed();
    let (kapilar, bilgi, adaptor) = match cati {
        PyCati::FastApi => {
            let (k, b) = crate::fastapi::kapilar(kok, &dosyalar);
            (k, b, format!("fastapi@{}", crate::fastapi::SURUM))
        }
        PyCati::Django => {
            let (k, b) = crate::django::kapilar(kok, &dosyalar);
            (k, b, format!("django@{}", crate::django::SURUM))
        }
        PyCati::Flask => {
            let (k, b) = crate::flask::kapilar(kok, &dosyalar);
            (k, b, format!("flask@{}", crate::flask::SURUM))
        }
    };
    let t_toplam = t0.elapsed();
    crate::Toplam {
        kapilar,
        bilgi,
        adaptor,
        dosya_sayisi: dosyalar.len(),
        satir: dosyalar.iter().map(|d| d.satir_sayisi).sum(),
        kismi: dosyalar.iter().filter(|d| !d.hatali_satirlar.is_empty()).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        atlanan,
        buyuk,
        ek_paketler: vec![],
        // Tablo tanımlayan modellerin dosyaları (SQLModel / SQLAlchemy, Django modelleri): rapor tablo bağlarını bunlara göre sayar
        veri_dosyalari: match cati {
            PyCati::FastApi => dosyalar.iter().filter(|d| d.siniflar.iter().any(crate::fastapi::tablo_modeli_mi)).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
            PyCati::Django => crate::django::model_dosyalari(&dosyalar).into_iter().map(|d| guvenli::goreli(kok, &d.yol)).collect(),
            PyCati::Flask => crate::flask::model_dosyalari(&dosyalar).into_iter().map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        },
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}
