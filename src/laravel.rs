//! Laravel adaptörü (PHP): PHP kaynağından (tree-sitter-php) NestJS ve FastAPI adaptörleriyle aynı kapı yapısını (`Kapi`)
//! üretir; kart, MCP, önizleme, OpenAPI, kanıt ve canlı mod dilden bağımsız çalışır.
//!
//! Kesinlik ilkesi diğer adaptörlerdekiyle aynıdır: anlam adlardan değil tanımlardan çıkar.
//! - **Rotalar:** `routes/*.php` dosyalarındaki `Route::get/post/put/patch/delete/options/any/match/view` kayıtları,
//!   `Route::resource / apiResource / resources / apiResources / singleton / apiSingleton` (üretilen rotalar, `only`,
//!   `except`, `parameters`, `shallow`, iç içe `photos.comments`), `prefix / name / middleware / namespace / controller /
//!   where` zincirleri ve `group` (kapanış, dizi biçimli nitelikler ya da dosya yolu) iç içe izlenir. Eylem
//!   `[Controller::class, 'm']`, `'Controller@m'`, `Controller::class` (invokable) ya da kapanış olabilir. Rota dosyalarını
//!   yükleyenler: `bootstrap/app.php` `withRouting(web:, api:, apiPrefix:, then:, using:, health:)`, RouteServiceProvider
//!   (`$this->routes(...)`, `map*Routes()`, `Route::prefix('api')->group(base_path(...))`), `loadRoutesFrom` ve
//!   `require`. Hiçbir yükleyicide geçmeyen rota dosyası Laravel'in varsayılanıyla okunur (`web.php` öneksiz, `api.php`
//!   `/api`); bunun dışındaki bağlanmamış dosyaların kapıları ikincildir. Koşul altındaki kayıt koşullu kapıdır.
//! - **Giriş ve rol:** rota, grup, controller (`$this->middleware(...)`, `HasMiddleware::middleware()`) ve genel ara
//!   katmanlar (Kernel ya da `withMiddleware`) takma adlarından sınıflarına izlenir. `auth` / `auth:sanctum` / `verified`
//!   (çatının Authenticate sınıfına ya da ondan türeyene çözülen) ve Spatie `role` / `permission` sınıfları girişi kesin
//!   ister; `can:` ve `$this->authorize()` / `Gate` giriş için yalnızca tahmindir. Proje ara katmanının `handle()`
//!   gövdesi okunur; tanımı izlenemeyen ara katman "tanınmayan guard" olarak bildirilir. Rolü Gate / politika gövdesinden
//!   okunamayan modelsiz yetenek bir izin adıdır ve rol / izin sayılır: `can:x` (çatının Authorize sınıfı) ve gövdenin
//!   üst düzeyindeki `$this->authorize('x')` kesin; projenin izin yardımcısı (`$this->checkPermission(Permission::X)`,
//!   enum değeri çözülür), projenin `can` ara katmanı, enum metoduyla hesaplanan ara katman ve sonuç döndüren denetim
//!   (`->can()`, `Gate::allows`) yorumdur.
//! - **Girdiler:** Form Request sınıfının `rules()` dizisi ya da `$request->validate([...])` / `Validator::make` kuralları
//!   gövde şemasıdır (GET'te sorgu). `required` zorunluluğu kesin, `sometimes` / `required_if` belirsizdir; `string`,
//!   `integer`, `max:255`, `in:a,b`, `Rule::in`, `Rule::enum`, `email`, `uuid`, `regex` önizlemenin kural adlarına
//!   çevrilir, çevrilemeyen kural (ör. `exists:users,id`) olduğu gibi kalır ve "kontrol edilemedi" sayılır.
//!   `array_merge([...], $this->kurallar())` ve projenin kural dizisi döndüren metotları açılır; koşula bağlı ya da
//!   çalışma anında kurulan kural (`$x ? [...] : null`, değişken) kural adı sayılmaz: alan belirsiz kalır, not yazılır.
//! - **Yanıt:** API Resource sınıfının `toArray()` dizisi (`data` sarmalıyla), `response()->json([...])` (yorum).
//! - **Veri ve davranış:** Eloquent model çağrıları (okuma / yazma, ilişki yazmaları, `DB::table`), göçlerdeki
//!   `onDelete('cascade')` / `cascadeOnDelete()` kaskadları, `abort` / `abort_if` / `throw` / `findOrFail` hata
//!   senaryoları, `event(...)` / `X::dispatch(...)` olay ve işleri, dinleyiciler ve kuyruk işleri (DNA).
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tree_sitter::{Node, Parser};

use crate::adaptor::{Alan, Girdi, Kapi, Sema, VeriBag, Yanit};
use crate::dna::{Dal, Firlatma, GovdeDna};
use crate::guvenli;

pub const SURUM: &str = "0.1.0";

/// İfadenin ağaçta en fazla derinliği (iç içe ifadeler belleği doldurmasın)
const EN_DERIN: usize = 48;

// ---- Model ----

/// PHP ifadesinin adaptörün okuduğu hâli (yalnızca anlamı olan biçimler ayrıntılıdır)
#[derive(Clone, Debug)]
pub enum Ifade {
    Metin(String),
    Sayi(String),
    /// `X::class`: sınıf adı yazıldığı gibi
    Sinif(String),
    /// Ad ya da sabit: `__DIR__`, `true`, `Uuid::REGEX`
    Ad(String),
    Degisken(String),
    /// `$a->b`
    Ozellik(Box<Ifade>, String),
    /// Dizi: (anahtar, değer) öğeleri ve her öğenin satırı
    Dizi(Vec<(Option<Ifade>, Ifade)>, Vec<usize>),
    Cagri(Box<Cagri>),
    /// `new X(...)`: sınıf adı ve argümanlar
    Yeni(String, Vec<Arg>),
    Kapanis(Box<Kapanis>),
    /// Metin birleştirme: `__DIR__ . '/x.php'`
    Birlesim(Vec<Ifade>),
    /// Koşul ya da döngü altındaki deyimler: koşulun kısa metni, gövde, döngü mü ve (`if` / `elseif`'te) koşul ifadesi.
    /// Koşul ifadesi gezintilere girmez (gövdedeki çağrılar gibi sayılmaz); yalnızca açıkça okuyanlar kullanır
    Kosul(String, Vec<Ifade>, bool, Option<Box<Ifade>>),
    /// `foreach ($kaynak as $anahtar => $deger) { ... }`: kaynak, anahtar, değer değişkeni, gövde, başlığın kısa metni
    Dongu(Box<Ifade>, Option<String>, String, Vec<Ifade>, String),
    Don(Box<Ifade>),
    Ata(Box<Ifade>, Box<Ifade>),
    /// `require __DIR__ . '/auth.php'`
    Dahil(Box<Ifade>, usize),
    /// `(int) $x`
    Donustur(String, Box<Ifade>),
    /// Başka bir ifade: kısa metni ve içindeki ifadeler
    Diger(String, Vec<Ifade>),
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub ad: Option<String>,
    pub deger: Ifade,
}

#[derive(Clone, Debug)]
pub struct Cagri {
    /// Üye çağrısında nesne, statik çağrıda sınıf (`Ifade::Ad("Route")`), fonksiyon çağrısında yok
    pub alici: Option<Ifade>,
    pub statik: bool,
    pub ad: String,
    pub arglar: Vec<Arg>,
    pub satir: usize,
}

#[derive(Clone, Debug)]
pub struct Kapanis {
    pub parametreler: Vec<Parametre>,
    pub govde: Vec<Ifade>,
    pub dna: GovdeDna,
    pub satir: usize,
    /// Gövde oturumu denetleyip reddediyor (`abort_unless(auth()->check(), 403)`)
    pub giris_denetimi: bool,
}

#[derive(Clone, Debug)]
pub struct Parametre {
    pub ad: String,
    pub tip: Option<String>,
    pub varsayilan: Option<Ifade>,
}

#[derive(Clone, Debug)]
pub struct Metot {
    pub ad: String,
    pub satir: usize,
    pub statik: bool,
    pub parametreler: Vec<Parametre>,
    /// PHPDoc özetinin ilk satırı
    pub ozet: Option<String>,
    pub donus_tipi: Option<String>,
    pub govde: Vec<Ifade>,
    pub dna: GovdeDna,
    /// Gövde kimliği soruyor mu (`auth()->check()`, `$request->user()`, `Auth::guest()` ...)
    pub kimlik_izi: bool,
    /// Gövde 401 / 403 ile reddediyor ya da girişe yönlendiriyor mu
    pub red_izi: bool,
    /// Gövde oturumu denetleyip reddediyor (`if (! auth()->check()) abort(401)`)
    pub giris_denetimi: bool,
}

#[derive(Clone, Debug)]
pub struct Ozellik {
    pub ad: String,
    pub tip: Option<String>,
    pub deger: Option<Ifade>,
    pub satir: usize,
}

#[derive(Clone, Debug)]
pub struct Sinif {
    pub ad: String,
    /// "class", "enum", "trait", "interface"
    pub tur: &'static str,
    pub taban: Option<String>,
    pub arayuzler: Vec<String>,
    pub traitler: Vec<String>,
    pub ozellikler: Vec<Ozellik>,
    pub metotlar: Vec<Metot>,
    /// enum durumları: (ad, değer)
    pub durumlar: Vec<(String, Option<String>)>,
    pub satir: usize,
}

/// Göçteki tablo tanımı (`Schema::create` / `Schema::table`)
#[derive(Clone, Debug)]
pub struct Tablo {
    pub tablo: String,
    pub olustur: bool,
    pub satir: usize,
    /// (sütun, tip, satır)
    pub sutunlar: Vec<(String, String, usize)>,
    pub yabancilar: Vec<Yabanci>,
}

/// Yabancı anahtar: ebeveyn tablo (`constrained()` sütun adından türetilir; `foreignIdFor(X::class)` için
/// "sinif:X"), silmede kaskad mı, satır
#[derive(Clone, Debug)]
pub struct Yabanci {
    pub ebeveyn: Option<String>,
    pub kaskad: bool,
    pub satir: usize,
}

#[derive(Clone, Debug)]
pub struct Dosya {
    pub yol: PathBuf,
    pub ad_alani: String,
    /// `use` takma adı -> tam ad
    pub kullanimlar: HashMap<String, String>,
    pub siniflar: Vec<Sinif>,
    /// Dosya düzeyindeki fonksiyon tanımları (`function userCan(...)`: projenin yardımcıları)
    pub fonksiyonlar: Vec<Metot>,
    /// Dosya düzeyindeki deyimler (rota dosyaları, `bootstrap/app.php`)
    pub ifadeler: Vec<Ifade>,
    pub tablolar: Vec<Tablo>,
    pub satir_sayisi: usize,
    pub hatali: bool,
}

// ---- Ayrıştırma ----

fn metin(n: Node, k: &[u8]) -> String {
    n.utf8_text(k).unwrap_or("").to_string()
}

fn satir(n: Node) -> usize {
    n.start_position().row + 1
}

fn cocuklar(n: Node) -> Vec<Node> {
    let mut w = n.walk();
    n.named_children(&mut w).filter(|c| c.kind() != "comment").collect()
}

/// PHP metin sabiti: tek tırnakta yalnızca `\\` ve `\'`; çift tırnakta değişken varsa çözülmez
fn metin_sabiti(n: Node, k: &[u8]) -> Option<String> {
    let t = metin(n, k);
    match n.kind() {
        "string" => {
            let ic = t.strip_prefix('\'').and_then(|x| x.strip_suffix('\''))?;
            Some(ic.replace("\\\\", "\u{0}").replace("\\'", "'").replace('\u{0}', "\\"))
        }
        "encapsed_string" => {
            if cocuklar(n).iter().any(|c| !matches!(c.kind(), "string_content" | "escape_sequence")) {
                return None;
            }
            let ic = t.strip_prefix('"').and_then(|x| x.strip_suffix('"'))?;
            Some(ic.replace("\\\\", "\u{0}").replace("\\\"", "\"").replace("\\$", "$").replace('\u{0}', "\\"))
        }
        _ => None,
    }
}

/// Tip metni: `?int` -> "int"; baştaki `\` (tam ad) korunur
fn tip_metni(n: Node, k: &[u8]) -> String {
    metin(n, k).trim().trim_start_matches('?').to_string()
}

fn parametreler(n: Option<Node>, k: &[u8], d: usize) -> Vec<Parametre> {
    let Some(n) = n else { return vec![] };
    cocuklar(n)
        .into_iter()
        .filter(|p| matches!(p.kind(), "simple_parameter" | "property_promotion_parameter" | "variadic_parameter"))
        .map(|p| Parametre {
            ad: p.child_by_field_name("name").map(|a| metin(a, k).trim_start_matches('$').to_string()).unwrap_or_default(),
            tip: p.child_by_field_name("type").map(|t| tip_metni(t, k)),
            varsayilan: p.child_by_field_name("default_value").map(|v| ir(v, k, d + 1)),
        })
        .collect()
}

fn argumanlar(n: Option<Node>, k: &[u8], d: usize) -> Vec<Arg> {
    let Some(n) = n else { return vec![] };
    cocuklar(n)
        .into_iter()
        .filter(|a| a.kind() == "argument" || a.kind() == "variadic_placeholder")
        .filter_map(|a| {
            let ad = a.child_by_field_name("name").map(|x| metin(x, k));
            let deger = cocuklar(a).into_iter().rfind(|c| Some(c.id()) != a.child_by_field_name("name").map(|x| x.id()))?;
            Some(Arg { ad, deger: ir(deger, k, d + 1) })
        })
        .collect()
}

/// İfade düğümünden `Ifade`
fn ir(n: Node, k: &[u8], d: usize) -> Ifade {
    if d > EN_DERIN {
        return Ifade::Diger(crate::dna::kisa(&metin(n, k)), vec![]);
    }
    match n.kind() {
        "string" | "encapsed_string" => match metin_sabiti(n, k) {
            Some(t) => Ifade::Metin(t),
            // "rest/{$uc}{format?}" / "a/$x": parçaları birleştirme olarak (değişken bağlıysa çözülür)
            None if n.kind() == "encapsed_string" && cocuklar(n).iter().all(|c| matches!(c.kind(), "string_content" | "escape_sequence" | "variable_name")) => Ifade::Birlesim(
                cocuklar(n)
                    .into_iter()
                    .map(|c| match c.kind() {
                        "variable_name" => Ifade::Degisken(metin(c, k).trim_start_matches('$').to_string()),
                        "escape_sequence" => Ifade::Metin(metin(c, k).trim_start_matches('\\').to_string()),
                        _ => Ifade::Metin(metin(c, k)),
                    })
                    .collect(),
            ),
            None => Ifade::Diger(crate::dna::kisa(&metin(n, k)), vec![]),
        },
        "integer" | "float" => Ifade::Sayi(metin(n, k)),
        "boolean" | "null" | "name" | "qualified_name" | "relative_scope" | "scoped_property_access_expression" => Ifade::Ad(metin(n, k)),
        "variable_name" => Ifade::Degisken(metin(n, k).trim_start_matches('$').to_string()),
        "class_constant_access_expression" => {
            let c = cocuklar(n);
            match c.as_slice() {
                [s, a] if metin(*a, k) == "class" => Ifade::Sinif(metin(*s, k)),
                _ => Ifade::Ad(metin(n, k)),
            }
        }
        "member_access_expression" | "nullsafe_member_access_expression" => match (n.child_by_field_name("object"), n.child_by_field_name("name")) {
            (Some(o), Some(a)) => Ifade::Ozellik(Box::new(ir(o, k, d + 1)), metin(a, k)),
            _ => Ifade::Diger(crate::dna::kisa(&metin(n, k)), vec![]),
        },
        "array_creation_expression" => {
            let ogeler: Vec<Node> = cocuklar(n).into_iter().filter(|e| e.kind() == "array_element_initializer").collect();
            Ifade::Dizi(
                ogeler
                    .iter()
                    .map(|e| {
                        let c = cocuklar(*e);
                        match c.as_slice() {
                            [a, v] => (Some(ir(*a, k, d + 1)), ir(*v, k, d + 1)),
                            [v] => (None, ir(*v, k, d + 1)),
                            _ => (None, Ifade::Diger(metin(*e, k), vec![])),
                        }
                    })
                    .collect(),
                ogeler.iter().map(|e| satir(*e)).collect(),
            )
        }
        "function_call_expression" => {
            let f = n.child_by_field_name("function");
            let arglar = argumanlar(n.child_by_field_name("arguments"), k, d);
            match f.filter(|f| matches!(f.kind(), "name" | "qualified_name")) {
                Some(f) => Ifade::Cagri(Box::new(Cagri { alici: None, statik: false, ad: metin(f, k).trim_start_matches('\\').to_string(), arglar, satir: satir(n) })),
                None => Ifade::Diger(crate::dna::kisa(&metin(n, k)), arglar.into_iter().map(|a| a.deger).chain(f.map(|f| ir(f, k, d + 1))).collect()),
            }
        }
        "member_call_expression" | "nullsafe_member_call_expression" | "scoped_call_expression" => {
            let statik = n.kind() == "scoped_call_expression";
            let alici = n.child_by_field_name(if statik { "scope" } else { "object" }).map(|o| ir(o, k, d + 1));
            let ad = n.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
            Ifade::Cagri(Box::new(Cagri { alici, statik, ad, arglar: argumanlar(n.child_by_field_name("arguments"), k, d), satir: satir(n) }))
        }
        "object_creation_expression" => {
            let c = cocuklar(n);
            match c.iter().find(|x| matches!(x.kind(), "name" | "qualified_name")) {
                Some(s) => Ifade::Yeni(metin(*s, k), argumanlar(c.iter().find(|x| x.kind() == "arguments").copied(), k, d)),
                None => Ifade::Diger("new class".into(), vec![]),
            }
        }
        "anonymous_function" | "arrow_function" => {
            let govde_n = n.child_by_field_name("body");
            let govde = match govde_n {
                Some(g) if g.kind() == "compound_statement" => deyimler(g, k, d + 1),
                Some(g) => vec![Ifade::Don(Box::new(ir(g, k, d + 1)))],
                None => vec![],
            };
            Ifade::Kapanis(Box::new(Kapanis {
                parametreler: parametreler(n.child_by_field_name("parameters"), k, d),
                govde,
                dna: govde_n.map(|g| php_dna(g, k)).unwrap_or_default(),
                satir: satir(n),
                giris_denetimi: govde_n.is_some_and(|g| giris_denetimi(&metin(g, k))),
            }))
        }
        "binary_expression" if n.child_by_field_name("operator").is_some_and(|o| metin(o, k) == ".") => {
            let mut v = vec![];
            for a in ["left", "right"] {
                match n.child_by_field_name(a).map(|x| ir(x, k, d + 1)) {
                    Some(Ifade::Birlesim(p)) => v.extend(p),
                    Some(x) => v.push(x),
                    None => {}
                }
            }
            Ifade::Birlesim(v)
        }
        "parenthesized_expression" => cocuklar(n).first().map(|c| ir(*c, k, d + 1)).unwrap_or(Ifade::Diger(String::new(), vec![])),
        "cast_expression" => match (n.child_by_field_name("type"), n.child_by_field_name("value")) {
            (Some(t), Some(v)) => Ifade::Donustur(metin(t, k).to_lowercase(), Box::new(ir(v, k, d + 1))),
            _ => Ifade::Diger(metin(n, k), vec![]),
        },
        "assignment_expression" | "reference_assignment_expression" => match (n.child_by_field_name("left"), n.child_by_field_name("right")) {
            (Some(l), Some(r)) => Ifade::Ata(Box::new(ir(l, k, d + 1)), Box::new(ir(r, k, d + 1))),
            _ => Ifade::Diger(metin(n, k), vec![]),
        },
        "include_expression" | "include_once_expression" | "require_expression" | "require_once_expression" => {
            Ifade::Dahil(Box::new(cocuklar(n).first().map(|c| ir(*c, k, d + 1)).unwrap_or(Ifade::Diger(String::new(), vec![]))), satir(n))
        }
        _ => Ifade::Diger(crate::dna::kisa(&metin(n, k)), cocuklar(n).into_iter().map(|c| ir(c, k, d + 1)).collect()),
    }
}

/// Deyim listesi (fonksiyon gövdesi, dosya, kapanış): koşul ve döngü gövdeleri `Kosul` olarak iç içe
fn deyimler(n: Node, k: &[u8], d: usize) -> Vec<Ifade> {
    let mut v = vec![];
    for c in cocuklar(n) {
        deyim(c, k, d, &mut v);
    }
    v
}

fn deyim(c: Node, k: &[u8], d: usize, v: &mut Vec<Ifade>) {
    if d > EN_DERIN {
        return;
    }
    let kosul_metni = |x: Option<Node>| x.map(|x| crate::dna::kisa(&metin(x, k))).unwrap_or_default();
    match c.kind() {
        "expression_statement" => v.extend(cocuklar(c).into_iter().map(|x| ir(x, k, d + 1))),
        "return_statement" => {
            if let Some(x) = cocuklar(c).first() {
                v.push(Ifade::Don(Box::new(ir(*x, k, d + 1))));
            }
        }
        "compound_statement" | "colon_block" => v.extend(deyimler(c, k, d + 1)),
        "if_statement" => {
            let kosul = kosul_metni(c.child_by_field_name("condition"));
            let kosul_ifadesi = |x: Option<Node>| x.map(|x| Box::new(ir(x, k, d + 1)));
            let mut govde = vec![];
            if let Some(b) = c.child_by_field_name("body") {
                deyim(b, k, d + 1, &mut govde);
            }
            v.push(Ifade::Kosul(kosul.clone(), govde, false, kosul_ifadesi(c.child_by_field_name("condition"))));
            let mut w = c.walk();
            for a in c.children_by_field_name("alternative", &mut w) {
                let (metin_, b, ki) = match a.kind() {
                    "else_if_clause" => (kosul_metni(a.child_by_field_name("condition")), a.child_by_field_name("body"), kosul_ifadesi(a.child_by_field_name("condition"))),
                    _ => (format!("!({kosul})"), a.child_by_field_name("body"), None),
                };
                let mut g = vec![];
                if let Some(b) = b {
                    deyim(b, k, d + 1, &mut g);
                }
                v.push(Ifade::Kosul(metin_, g, false, ki));
            }
        }
        "foreach_statement" => {
            let c2 = cocuklar(c);
            let govde_n = c.child_by_field_name("body");
            let basliklar: Vec<Node> = c2.iter().filter(|x| Some(x.id()) != govde_n.map(|g| g.id())).copied().collect();
            let mut g = vec![];
            if let Some(b) = govde_n {
                deyim(b, k, d + 1, &mut g);
            }
            let ad = |x: Node| metin(x, k).trim_start_matches('&').trim_start_matches('$').to_string();
            let baslik = crate::dna::kisa(metin(c, k).lines().next().unwrap_or(""));
            match basliklar.as_slice() {
                [kaynak, cift] if cift.kind() == "pair" => {
                    let p = cocuklar(*cift);
                    if let [ka, va] = p.as_slice() {
                        v.push(Ifade::Dongu(Box::new(ir(*kaynak, k, d + 1)), Some(ad(*ka)), ad(*va), g, baslik));
                    }
                }
                [kaynak, deger] => v.push(Ifade::Dongu(Box::new(ir(*kaynak, k, d + 1)), None, ad(*deger), g, baslik)),
                _ => v.push(Ifade::Kosul(baslik, g, true, None)),
            }
        }
        "for_statement" | "while_statement" | "do_statement" | "switch_statement" => {
            let mut g = vec![];
            for x in cocuklar(c) {
                if matches!(x.kind(), "compound_statement" | "colon_block" | "switch_block" | "expression_statement" | "return_statement" | "if_statement") {
                    deyim(x, k, d + 1, &mut g);
                } else if x.kind() == "case_statement" || x.kind() == "default_statement" {
                    for y in cocuklar(x) {
                        deyim(y, k, d + 1, &mut g);
                    }
                }
            }
            let dongu = c.kind() != "switch_statement";
            v.push(Ifade::Kosul(crate::dna::kisa(metin(c, k).lines().next().unwrap_or("")), g, dongu, None));
        }
        "switch_block" => {
            for x in cocuklar(c) {
                for y in cocuklar(x) {
                    deyim(y, k, d + 1, v);
                }
            }
        }
        "try_statement" => {
            for x in cocuklar(c) {
                match x.kind() {
                    "compound_statement" => v.extend(deyimler(x, k, d + 1)),
                    "catch_clause" | "finally_clause" => {
                        let mut g = vec![];
                        if let Some(b) = x.child_by_field_name("body") {
                            deyim(b, k, d + 1, &mut g);
                        }
                        v.push(Ifade::Kosul("catch".into(), g, false, None));
                    }
                    _ => {}
                }
            }
        }
        "echo_statement"
        | "unset_statement"
        | "global_declaration"
        | "function_static_declaration"
        | "break_statement"
        | "continue_statement"
        | "empty_statement"
        | "named_label_statement"
        | "goto_statement"
        | "text_interpolation"
        | "php_tag"
        | "namespace_definition"
        | "namespace_use_declaration"
        | "function_definition"
        | "class_declaration"
        | "interface_declaration"
        | "trait_declaration"
        | "enum_declaration"
        | "const_declaration"
        | "declare_statement"
        | "comment" => {}
        _ => v.push(ir(c, k, d + 1)),
    }
}

/// Symfony / Laravel HTTP istisnaları ve `Response::HTTP_*` sabitleri -> HTTP kodu
fn istisna_kodu(ad: &str) -> Option<u16> {
    let son = ad.rsplit('\\').next().unwrap_or(ad);
    Some(match son {
        "BadRequestHttpException" => 400,
        "UnauthorizedHttpException" | "AuthenticationException" => 401,
        "AccessDeniedHttpException" | "AuthorizationException" | "UnauthorizedException" => 403,
        "NotFoundHttpException" | "ModelNotFoundException" => 404,
        "MethodNotAllowedHttpException" => 405,
        "ConflictHttpException" => 409,
        "GoneHttpException" => 410,
        "PreconditionFailedHttpException" => 412,
        "UnprocessableEntityHttpException" | "ValidationException" => 422,
        "TooManyRequestsHttpException" | "ThrottleRequestsException" => 429,
        "ServiceUnavailableHttpException" => 503,
        _ => return None,
    })
}

/// HTTP kodu ifadesi: `403`, `Response::HTTP_FORBIDDEN`, `Response::HTTP_UNPROCESSABLE_ENTITY`
fn php_kod(s: &str) -> Option<u16> {
    let s = s.trim();
    let son = s.rsplit("::").next().unwrap_or(s);
    match son.strip_prefix("HTTP_") {
        Some(r) => crate::dna::kod_ifadesi(r),
        None => crate::dna::kod_ifadesi(son),
    }
}

/// Düğümün içinde bulunduğu koşul: en yakın `if` (else dalındaysa olumsuzu), `catch`
fn php_kosul(mut n: Node, kok: Node, k: &[u8]) -> Option<String> {
    while let Some(ust) = n.parent() {
        if n.id() == kok.id() {
            break;
        }
        match ust.kind() {
            "if_statement" => {
                let kosul = ust.child_by_field_name("condition").map(|c| crate::dna::kisa(&metin(c, k)))?;
                let olumlu = ust.child_by_field_name("body").is_some_and(|c| c.id() == n.id()) || n.kind() == "parenthesized_expression";
                return Some(if olumlu { kosul } else { format!("!({kosul})") });
            }
            "else_if_clause" => return ust.child_by_field_name("condition").map(|c| crate::dna::kisa(&metin(c, k))),
            "catch_clause" => return Some("catch".into()),
            _ => {}
        }
        n = ust;
    }
    None
}

fn basit_mi(n: Node) -> bool {
    matches!(
        n.kind(),
        "string"
            | "encapsed_string"
            | "integer"
            | "float"
            | "boolean"
            | "null"
            | "name"
            | "qualified_name"
            | "variable_name"
            | "class_constant_access_expression"
            | "member_access_expression"
    )
}

/// PHP fonksiyon gövdesinin davranış öğeleri: `abort(403)`, `abort_if(koşul, 404)`, `throw new X`, `$this->authorize()`,
/// `Gate::authorize()`, `findOrFail()` ve iki yollu `koşul ? a : b` ifadeleri. Gövdedeki kapanışlar (`DB::transaction`)
/// gövdenin parçasıdır; iç sınıf tanımlarına girilmez
fn php_dna(govde: Node, k: &[u8]) -> GovdeDna {
    const EN_FAZLA: usize = 32;
    let mut d = GovdeDna::default();
    let mut yigin = vec![govde];
    let ekle = |d: &mut GovdeDna, n: Node, durum: Option<u16>, istisna: String, kosul: Option<String>| {
        if d.firlatmalar.len() < EN_FAZLA {
            let sahiplik = kosul.as_deref().is_some_and(|c| crate::dna::sahiplik_mi(c, durum));
            d.firlatmalar.push(Firlatma { durum, istisna, kosul, satir: satir(n), sahiplik });
        }
    };
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "class_declaration" | "anonymous_class" if n.id() != govde.id() => continue,
            "function_call_expression" => {
                let f = n.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                let args: Vec<Node> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default().into_iter().filter_map(|a| cocuklar(a).last().copied()).collect();
                match f.trim_start_matches('\\') {
                    "abort" => {
                        let durum = args.first().and_then(|a| php_kod(&metin(*a, k)));
                        ekle(&mut d, n, durum, "abort".into(), php_kosul(n, govde, k));
                    }
                    a @ ("abort_if" | "abort_unless" | "throw_if" | "throw_unless") => {
                        let kosul = args.first().map(|c| crate::dna::kisa(&metin(*c, k)));
                        let kosul = if a.ends_with("unless") { kosul.map(|c| format!("!({c})")) } else { kosul };
                        let (durum, istisna) = match args.get(1) {
                            Some(x) if x.kind() == "object_creation_expression" => {
                                let ad = cocuklar(*x).iter().find(|c| matches!(c.kind(), "name" | "qualified_name")).map(|c| metin(*c, k)).unwrap_or_default();
                                (istisna_kodu(&ad), ad)
                            }
                            Some(x) => (php_kod(&metin(*x, k)), a.to_string()),
                            None => (None, a.to_string()),
                        };
                        ekle(&mut d, n, durum, istisna, kosul);
                    }
                    _ => {}
                }
            }
            "throw_expression" => {
                if let Some(x) = cocuklar(n).first() {
                    let (ad, durum) = match x.kind() {
                        "object_creation_expression" => {
                            let c = cocuklar(*x);
                            let ad = c.iter().find(|c| matches!(c.kind(), "name" | "qualified_name")).map(|c| metin(*c, k)).unwrap_or_default();
                            let arg_kod = c
                                .iter()
                                .find(|c| c.kind() == "arguments")
                                .map(|a| cocuklar(*a))
                                .unwrap_or_default()
                                .iter()
                                .take(2)
                                .find_map(|a| cocuklar(*a).last().and_then(|v| php_kod(&metin(*v, k))));
                            let durum = istisna_kodu(&ad).or(arg_kod.filter(|_| ad.ends_with("HttpException")));
                            (ad, durum)
                        }
                        "scoped_call_expression" => {
                            let s = x.child_by_field_name("scope").map(|s| metin(s, k)).unwrap_or_default();
                            let durum = istisna_kodu(&s);
                            (s, durum)
                        }
                        _ => (crate::dna::kisa(&metin(*x, k)), None),
                    };
                    if durum.is_some() || ad.ends_with("Exception") || ad.ends_with("Error") {
                        ekle(&mut d, n, durum, ad, php_kosul(n, govde, k));
                    }
                }
            }
            "member_call_expression" | "scoped_call_expression" | "nullsafe_member_call_expression" => {
                let ad = n.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
                let alici = n.child_by_field_name(if n.kind() == "scoped_call_expression" { "scope" } else { "object" }).map(|a| metin(a, k)).unwrap_or_default();
                let cagri = crate::dna::kisa(&metin(n, k));
                match ad.as_str() {
                    "authorize" | "authorizeForUser" if alici == "$this" || alici.ends_with("Gate") => {
                        ekle(&mut d, n, Some(403), "AuthorizationException".into(), Some(cagri));
                    }
                    "findOrFail" | "firstOrFail" | "sole" => ekle(&mut d, n, Some(404), "ModelNotFoundException".into(), Some(cagri)),
                    _ => {}
                }
            }
            "conditional_expression" if d.dallar.len() < EN_FAZLA => {
                if let (Some(c), Some(e), Some(h)) = (n.child_by_field_name("condition"), n.child_by_field_name("body"), n.child_by_field_name("alternative")) {
                    let kosul = metin(c, k);
                    if basit_mi(e) && basit_mi(h) && kosul.len() <= 60 && !kosul.contains('\n') {
                        let baglam = n
                            .parent()
                            .and_then(|p| match p.kind() {
                                "assignment_expression" => p.child_by_field_name("left").map(|l| metin(l, k)),
                                "array_element_initializer" => cocuklar(p).first().filter(|x| x.id() != n.id()).map(|x| metin(*x, k)),
                                "argument" => p
                                    .parent()
                                    .and_then(|a| a.parent())
                                    .and_then(|c| c.child_by_field_name("name").or_else(|| c.child_by_field_name("function")))
                                    .map(|x| metin(x, k)),
                                _ => None,
                            })
                            .unwrap_or_default();
                        d.dallar.push(Dal {
                            kosul: crate::dna::kisa(&kosul),
                            evet: crate::dna::kisa(&metin(e, k)),
                            hayir: crate::dna::kisa(&metin(h, k)),
                            baglam: baglam.trim_matches(['\'', '"']).to_string(),
                            satir: satir(n),
                        });
                    }
                }
            }
            _ => {}
        }
        let bas = yigin.len();
        let mut w = n.walk();
        yigin.extend(n.named_children(&mut w));
        yigin[bas..].reverse();
    }
    d
}

/// Göçteki `Schema::create('tablo', function (Blueprint $table) { ... })` ve `Schema::table(...)` tanımları
fn semalar(kok: Node, k: &[u8], out: &mut Vec<Tablo>) {
    let mut yigin = vec![kok];
    while let Some(n) = yigin.pop() {
        if n.kind() == "scoped_call_expression"
            && n.child_by_field_name("scope").is_some_and(|s| metin(s, k).trim_start_matches('\\').rsplit('\\').next() == Some("Schema"))
            && n.child_by_field_name("name").is_some_and(|a| matches!(metin(a, k).as_str(), "create" | "table"))
        {
            let olustur = n.child_by_field_name("name").is_some_and(|a| metin(a, k) == "create");
            let args: Vec<Node> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default().into_iter().filter_map(|a| cocuklar(a).last().copied()).collect();
            if let (Some(t), Some(f)) = (args.first().and_then(|a| metin_sabiti(*a, k)), args.get(1).filter(|f| f.kind() == "anonymous_function")) {
                let mut tablo = Tablo { tablo: t, olustur, satir: satir(n), sutunlar: vec![], yabancilar: vec![] };
                let degisken = f
                    .child_by_field_name("parameters")
                    .and_then(|p| cocuklar(p).into_iter().next())
                    .and_then(|p| p.child_by_field_name("name"))
                    .map(|a| metin(a, k))
                    .unwrap_or_else(|| "$table".into());
                if let Some(g) = f.child_by_field_name("body") {
                    for s in cocuklar(g).into_iter().filter(|s| s.kind() == "expression_statement") {
                        if let Some(c) = cocuklar(s).first() {
                            sutun_zinciri(*c, k, &degisken, &mut tablo);
                        }
                    }
                }
                out.push(tablo);
            }
        }
        let mut w = n.walk();
        yigin.extend(n.named_children(&mut w));
    }
}

/// `$table->foreignId('user_id')->constrained()->cascadeOnDelete()` gibi bir sütun zinciri
fn sutun_zinciri(n: Node, k: &[u8], degisken: &str, t: &mut Tablo) {
    // Zincir halkaları kökten sona: (ad, metin argümanları, sınıf argümanı)
    let mut halkalar: Vec<(String, Vec<String>, Option<String>)> = vec![];
    let mut x = n;
    loop {
        if x.kind() != "member_call_expression" {
            if metin(x, k) != degisken {
                return;
            }
            break;
        }
        let ad = x.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
        let args: Vec<Node> = x.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
        let metinler: Vec<String> = args.iter().filter_map(|a| cocuklar(*a).last().and_then(|v| metin_sabiti(*v, k))).collect();
        let sinif = args
            .iter()
            .filter_map(|a| cocuklar(*a).last().copied())
            .find(|v| v.kind() == "class_constant_access_expression")
            .and_then(|v| cocuklar(v).first().map(|s| metin(*s, k)));
        halkalar.push((ad, metinler, sinif));
        match x.child_by_field_name("object") {
            Some(o) => x = o,
            None => return,
        }
    }
    halkalar.reverse();
    let Some((ilk, args, sinif)) = halkalar.first().cloned() else { return };
    let sat = satir(n);
    let ad_ = args.first().cloned();
    match ilk.as_str() {
        "id" => t.sutunlar.push((ad_.unwrap_or_else(|| "id".into()), "id".into(), sat)),
        "timestamps" | "timestampsTz" | "nullableTimestamps" => {
            t.sutunlar.push(("created_at".into(), "timestamp".into(), sat));
            t.sutunlar.push(("updated_at".into(), "timestamp".into(), sat));
        }
        "softDeletes" | "softDeletesTz" => t.sutunlar.push((ad_.unwrap_or_else(|| "deleted_at".into()), "timestamp".into(), sat)),
        "rememberToken" => t.sutunlar.push(("remember_token".into(), "string".into(), sat)),
        "morphs" | "nullableMorphs" | "uuidMorphs" | "ulidMorphs" => {
            if let Some(a) = ad_ {
                t.sutunlar.push((format!("{a}_id"), "id".into(), sat));
                t.sutunlar.push((format!("{a}_type"), "string".into(), sat));
            }
        }
        "foreign" => {
            // $table->foreign('user_id')->references('id')->on('users')->onDelete('cascade')
            let ebeveyn = halkalar.iter().find(|h| h.0 == "on").and_then(|h| h.1.first().cloned());
            if ad_.is_some() {
                t.yabancilar.push(Yabanci { ebeveyn, kaskad: kaskad_mi(&halkalar), satir: sat });
            }
        }
        "dropForeign" | "dropColumn" | "dropColumns" | "index" | "unique" | "primary" | "dropIndex" | "dropUnique" | "renameColumn" | "dropTimestamps" | "dropSoftDeletes" => {}
        tip => {
            let sutun = match (tip, &sinif) {
                ("foreignIdFor", Some(s)) => Some(ad_.clone().unwrap_or_else(|| format!("{}_id", yilan(s.rsplit('\\').next().unwrap_or(s))))),
                _ => ad_.clone(),
            };
            let Some(sutun) = sutun else { return };
            t.sutunlar.push((sutun.clone(), tip.to_string(), sat));
            if let Some(c) = halkalar.iter().find(|h| h.0 == "constrained") {
                let ebeveyn = c.1.first().cloned().or_else(|| match (tip, &sinif) {
                    ("foreignIdFor", Some(s)) => Some(format!("sinif:{}", s.trim_start_matches('\\'))),
                    _ => sutun.strip_suffix("_id").map(cogul),
                });
                t.yabancilar.push(Yabanci { ebeveyn, kaskad: kaskad_mi(&halkalar), satir: sat });
            } else if halkalar.iter().any(|h| h.0 == "references") {
                let ebeveyn = halkalar.iter().find(|h| h.0 == "on").and_then(|h| h.1.first().cloned());
                t.yabancilar.push(Yabanci { ebeveyn, kaskad: kaskad_mi(&halkalar), satir: sat });
            }
        }
    }
}

fn kaskad_mi(h: &[(String, Vec<String>, Option<String>)]) -> bool {
    h.iter().any(|(a, args, _)| a == "cascadeOnDelete" || (a == "onDelete" && args.first().is_some_and(|x| x.eq_ignore_ascii_case("cascade"))))
}

/// PHPDoc özetinin ilk anlamlı satırı (metodun hemen önündeki `/** ... */`)
fn doc_ozeti(n: Node, k: &[u8]) -> Option<String> {
    let mut o = n.prev_sibling();
    while let Some(x) = o.filter(|x| x.kind() == "attribute_list") {
        o = x.prev_sibling();
    }
    let o = o.filter(|x| x.kind() == "comment")?;
    let t = metin(o, k);
    let t = t.strip_prefix("/**")?;
    t.lines()
        .map(|l| l.trim().trim_start_matches("*/").trim_start_matches('*').trim().trim_end_matches("*/").trim())
        .find(|l| !l.is_empty() && !l.starts_with('@'))
        .map(guvenli::temiz)
}

fn sinif_oku(n: Node, k: &[u8]) -> Option<Sinif> {
    let tur = match n.kind() {
        "class_declaration" => "class",
        "enum_declaration" => "enum",
        "trait_declaration" => "trait",
        "interface_declaration" => "interface",
        _ => return None,
    };
    let ad = n.child_by_field_name("name").map(|a| metin(a, k))?;
    let mut s = Sinif { ad, tur, taban: None, arayuzler: vec![], traitler: vec![], ozellikler: vec![], metotlar: vec![], durumlar: vec![], satir: satir(n) };
    for c in cocuklar(n) {
        match c.kind() {
            "base_clause" => s.taban = cocuklar(c).first().map(|x| metin(*x, k)),
            "class_interface_clause" => s.arayuzler = cocuklar(c).iter().map(|x| metin(*x, k)).collect(),
            _ => {}
        }
    }
    let Some(govde) = n.child_by_field_name("body") else { return Some(s) };
    let mut w = govde.walk();
    for u in govde.named_children(&mut w) {
        match u.kind() {
            "use_declaration" => s.traitler.extend(cocuklar(u).iter().filter(|x| matches!(x.kind(), "name" | "qualified_name")).map(|x| metin(*x, k))),
            "property_declaration" => {
                let tip = u.child_by_field_name("type").map(|t| tip_metni(t, k));
                for e in cocuklar(u).into_iter().filter(|e| e.kind() == "property_element") {
                    s.ozellikler.push(Ozellik {
                        ad: e.child_by_field_name("name").map(|a| metin(a, k).trim_start_matches('$').to_string()).unwrap_or_default(),
                        tip: tip.clone(),
                        deger: e.child_by_field_name("default_value").map(|v| ir(v, k, 0)),
                        satir: satir(e),
                    });
                }
            }
            "const_declaration" => {
                for e in cocuklar(u).into_iter().filter(|e| e.kind() == "const_element") {
                    let c = cocuklar(e);
                    if let [a, v] = c.as_slice() {
                        s.ozellikler.push(Ozellik { ad: format!("::{}", metin(*a, k)), tip: None, deger: Some(ir(*v, k, 0)), satir: satir(e) });
                    }
                }
            }
            "enum_case" => s.durumlar.push((
                u.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default(),
                u.child_by_field_name("value").and_then(|v| metin_sabiti(v, k).or_else(|| (v.kind() == "integer").then(|| metin(v, k)))),
            )),
            "method_declaration" => {
                // Yapıcıda terfi eden parametreler sınıfın özellikleridir: `private PostService $service`
                if let Some(p) = u.child_by_field_name("parameters") {
                    for x in cocuklar(p).into_iter().filter(|x| x.kind() == "property_promotion_parameter") {
                        s.ozellikler.push(Ozellik {
                            ad: x.child_by_field_name("name").map(|a| metin(a, k).trim_start_matches('$').to_string()).unwrap_or_default(),
                            tip: x.child_by_field_name("type").map(|t| tip_metni(t, k)),
                            deger: None,
                            satir: satir(x),
                        });
                    }
                }
                s.metotlar.push(metot_oku(u, k));
            }
            _ => {}
        }
    }
    Some(s)
}

/// Metot ya da dosya düzeyindeki fonksiyon tanımı
fn metot_oku(u: Node, k: &[u8]) -> Metot {
    let govde_n = u.child_by_field_name("body");
    let govde_metni = govde_n.map(|g| metin(g, k)).unwrap_or_default();
    Metot {
        ad: u.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default(),
        satir: satir(u),
        statik: cocuklar(u).iter().any(|x| x.kind() == "static_modifier"),
        parametreler: parametreler(u.child_by_field_name("parameters"), k, 0),
        ozet: doc_ozeti(u, k),
        donus_tipi: u.child_by_field_name("return_type").map(|t| tip_metni(t, k)),
        govde: govde_n.map(|g| deyimler(g, k, 0)).unwrap_or_default(),
        dna: govde_n.map(|g| php_dna(g, k)).unwrap_or_default(),
        kimlik_izi: kimlik_izi(&govde_metni),
        red_izi: red_izi(&govde_metni),
        giris_denetimi: giris_denetimi(&govde_metni),
    }
}

fn kimlik_izi(t: &str) -> bool {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    // `user()`: projelerin yaygın "oturumdaki kullanıcı" yardımcısı (metot çağrısı değil)
    D.get_or_init(|| {
        regex::Regex::new(r"auth\(\s*(?:'[^']*'|\x22[^\x22]*\x22)?\s*\)\s*->\s*(?:check|guest|user|id|guard|authenticate|shouldUse)\b|Auth::(?:check|guest|user|id|guard|viaRequest|authenticate|setUser|login|onceUsingId)\b|->authenticate\(|->user\(\s*\)|(?:^|[^>\w$:])user\(\s*\)|->bearerToken\(|->hasValidSignature\(")
            .unwrap()
    })
        .is_match(t)
}

fn red_izi(t: &str) -> bool {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        regex::Regex::new(
            r"\b40[13]\b|Auth\w*Exception|AuthorizationException|AccessDeniedHttpException|Unauthori[sz]ed|Forbidden|HTTP_UNAUTHORIZED|HTTP_FORBIDDEN|route\(\s*'login'\s*\)|redirect\(\s*\)\s*->\s*guest\(",
        )
        .unwrap()
    })
        .is_match(t)
}

/// Oturum denetimi ve red birlikte: `auth()->check()` / `Auth::guest()` ve 401 / 403
fn giris_denetimi(t: &str) -> bool {
    crate::kural::derle(r"auth\(\s*\)\s*->\s*(?:check|guest)\(\)|Auth::(?:check|guest)\(\)").is_match(t) && red_izi(t)
}

fn hatali(n: Node) -> bool {
    n.has_error()
}

/// Bir PHP dosyasını ayrıştırır
pub fn ayristir_metin(yol: &Path, kaynak: &str) -> Option<Dosya> {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_php::LANGUAGE_PHP.into()).ok()?;
    let agac = p.parse(kaynak, None)?;
    let k = kaynak.as_bytes();
    let kok = agac.root_node();
    let mut d = Dosya {
        yol: yol.to_path_buf(),
        ad_alani: String::new(),
        kullanimlar: HashMap::new(),
        siniflar: vec![],
        fonksiyonlar: vec![],
        ifadeler: vec![],
        tablolar: vec![],
        satir_sayisi: kaynak.lines().count(),
        hatali: hatali(kok),
    };
    let mut ust: Vec<Node> = cocuklar(kok);
    let mut i = 0;
    while i < ust.len() && i < 100_000 {
        let c = ust[i];
        i += 1;
        match c.kind() {
            "namespace_definition" => {
                d.ad_alani = c.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
                // `namespace X { ... }` biçimi: gövdedeki tanımlar da dosyanın üst düzeyidir
                if let Some(g) = c.child_by_field_name("body") {
                    ust.extend(cocuklar(g));
                }
            }
            "namespace_use_declaration" => {
                let onek = c.child_by_field_name("body").and_then(|_| cocuklar(c).into_iter().find(|x| x.kind() == "namespace_name")).map(|x| metin(x, k));
                let mut yazdir = |cl: Node, onek: Option<&str>| {
                    let tam = cocuklar(cl).into_iter().find(|x| matches!(x.kind(), "qualified_name" | "name" | "namespace_name")).map(|x| metin(x, k)).unwrap_or_default();
                    let tam = match onek {
                        Some(o) => format!("{o}\\{tam}"),
                        None => tam,
                    };
                    let tam = tam.trim_start_matches('\\').to_string();
                    let takma = cl.child_by_field_name("alias").map(|a| metin(a, k)).unwrap_or_else(|| tam.rsplit('\\').next().unwrap_or(&tam).to_string());
                    d.kullanimlar.insert(takma.to_lowercase(), tam);
                };
                for cl in cocuklar(c) {
                    match cl.kind() {
                        "namespace_use_clause" => yazdir(cl, None),
                        "namespace_use_group" => {
                            for g in cocuklar(cl).into_iter().filter(|g| g.kind() == "namespace_use_clause") {
                                yazdir(g, onek.as_deref());
                            }
                        }
                        _ => {}
                    }
                }
            }
            "class_declaration" | "enum_declaration" | "trait_declaration" | "interface_declaration" => {
                if let Some(s) = sinif_oku(c, k) {
                    d.siniflar.push(s);
                }
            }
            "function_definition" => d.fonksiyonlar.push(metot_oku(c, k)),
            _ => deyim(c, k, 0, &mut d.ifadeler),
        }
    }
    if kaynak.contains("Schema::") {
        semalar(kok, k, &mut d.tablolar);
    }
    Some(d)
}

// ---- Ad yardımcıları ----

/// PascalCase / camelCase -> snake_case: "BlogPost" -> "blog_post"
fn yilan(s: &str) -> String {
    let mut o = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !o.ends_with('_') {
                o.push('_');
            }
            o.extend(c.to_lowercase());
        } else {
            o.push(c);
        }
    }
    o
}

/// İngilizce çoğul (Laravel `Str::plural` sadeleştirilmiş): post -> posts, category -> categories, person -> people
fn cogul(s: &str) -> String {
    let duzensiz = [("person", "people"), ("child", "children"), ("man", "men"), ("woman", "women"), ("mouse", "mice"), ("goose", "geese"), ("tooth", "teeth"), ("foot", "feet")];
    if let Some((t, c)) = duzensiz.iter().find(|(t, _)| s == *t || s.ends_with(&format!("_{t}"))) {
        return format!("{}{c}", &s[..s.len() - t.len()]);
    }
    let degismez = ["data", "media", "equipment", "information", "news", "series", "species", "metadata", "feedback", "sheep", "fish", "deer"];
    if degismez.iter().any(|d| s.ends_with(d)) {
        return s.to_string();
    }
    let sesli = |c: char| "aeiou".contains(c);
    if let Some(k) = s.strip_suffix('y') {
        if !k.ends_with(sesli) {
            return format!("{k}ies");
        }
    }
    if ["s", "x", "z", "ch", "sh"].iter().any(|e| s.ends_with(e)) {
        return format!("{s}es");
    }
    format!("{s}s")
}

/// İngilizce tekil (Laravel `Str::singular` sadeleştirilmiş): photos -> photo, categories -> category, people -> person
fn tekil(s: &str) -> String {
    let duzensiz = [("people", "person"), ("children", "child"), ("men", "man"), ("women", "woman"), ("mice", "mouse"), ("geese", "goose"), ("teeth", "tooth"), ("feet", "foot")];
    if let Some((c, t)) = duzensiz.iter().find(|(c, _)| s == *c || s.ends_with(&format!("_{c}"))) {
        return format!("{}{t}", &s[..s.len() - c.len()]);
    }
    let degismez = [
        "data",
        "media",
        "equipment",
        "information",
        "news",
        "series",
        "species",
        "metadata",
        "feedback",
        "sheep",
        "fish",
        "deer",
        "status",
        "bus",
        "alias",
        "address",
        "analysis",
        "basis",
        "axis",
    ];
    if degismez.iter().any(|d| s.ends_with(d)) {
        return s.to_string();
    }
    if let Some(k) = s.strip_suffix("ies") {
        return format!("{k}y");
    }
    for e in ["sses", "xes", "zes", "ches", "shes", "uses"] {
        if let Some(k) = s.strip_suffix(e) {
            return format!("{k}{}", &e[..e.len() - 2]);
        }
    }
    if s.ends_with("ss") {
        return s.to_string();
    }
    s.strip_suffix('s').map(String::from).unwrap_or_else(|| s.to_string())
}

fn pascal(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut k = p.chars();
            k.next().map(|f| f.to_uppercase().collect::<String>() + k.as_str()).unwrap_or_default()
        })
        .collect()
}

/// Sınıfın kısa adı: "App\Models\Post" -> "Post"
fn kisa_ad(s: &str) -> &str {
    s.trim_start_matches('\\').rsplit('\\').next().unwrap_or(s)
}

// ---- Bağlam: sınıf dizini ve ad çözümü ----

/// (dosya indeksi, sınıf indeksi)
type Yer = (usize, usize);

/// Eloquent modeli: yeri, tablo adı ve kaynağı, yumuşak silme, ilişkiler
#[derive(Clone, Debug)]
struct Model {
    yer: Yer,
    tablo: String,
    tablo_acik: bool,
    yumusak: bool,
    /// ilişki metodu -> (ilişki türü, hedef model kısa adı, pivot tablo)
    iliskiler: HashMap<String, (String, String, Option<String>)>,
}

struct Baglam<'a> {
    kok: &'a Path,
    dosyalar: &'a [Dosya],
    /// küçük harfli tam ad -> yer
    siniflar: HashMap<String, Yer>,
    /// küçük harfli kısa ad -> yerler
    kisa: HashMap<String, Vec<Yer>>,
    /// köke göre yol ("routes/api.php") -> dosya
    yollar: HashMap<String, usize>,
    modeller: BTreeMap<String, Model>,
    /// küçük harfli tablo -> model adı
    tablolar: HashMap<String, String>,
    kullanici_modeli: Option<String>,
    /// `JsonResource::withoutWrapping()` çağrılıyor mu
    sarmasiz: bool,
}

const MODEL_TABANLARI: [&str; 4] =
    ["Illuminate\\Database\\Eloquent\\Model", "Illuminate\\Foundation\\Auth\\User", "Illuminate\\Database\\Eloquent\\Relations\\Pivot", "Jenssegers\\Mongodb\\Eloquent\\Model"];
const ILISKILER: [&str; 11] =
    ["hasMany", "hasOne", "belongsTo", "belongsToMany", "morphMany", "morphOne", "morphTo", "morphToMany", "morphedByMany", "hasManyThrough", "hasOneThrough"];

impl<'a> Baglam<'a> {
    fn yeni(kok: &'a Path, dosyalar: &'a [Dosya]) -> Self {
        let mut b = Baglam {
            kok,
            dosyalar,
            siniflar: HashMap::new(),
            kisa: HashMap::new(),
            yollar: HashMap::new(),
            modeller: BTreeMap::new(),
            tablolar: HashMap::new(),
            kullanici_modeli: None,
            sarmasiz: false,
        };
        for (di, d) in dosyalar.iter().enumerate() {
            b.yollar.insert(guvenli::goreli(kok, &d.yol), di);
            for (ci, s) in d.siniflar.iter().enumerate() {
                let tam = if d.ad_alani.is_empty() { s.ad.clone() } else { format!("{}\\{}", d.ad_alani, s.ad) };
                b.siniflar.entry(tam.to_lowercase()).or_insert((di, ci));
                b.kisa.entry(s.ad.to_lowercase()).or_default().push((di, ci));
                for m in &s.metotlar {
                    if !b.sarmasiz && govde_ara(&m.govde, &mut |c| c.ad == "withoutWrapping") {
                        b.sarmasiz = true;
                    }
                }
            }
        }
        // Modeller ve ilişkileri
        let mut modeller = BTreeMap::new();
        for (di, d) in dosyalar.iter().enumerate() {
            for (ci, s) in d.siniflar.iter().enumerate() {
                if s.tur != "class" || !b.turer((di, ci), &MODEL_TABANLARI) {
                    continue;
                }
                let acik = b.ozellik((di, ci), "table").and_then(|(_, o)| o.deger.as_ref()).and_then(|v| match v {
                    Ifade::Metin(t) => Some(t.clone()),
                    _ => None,
                });
                let mut iliskiler = HashMap::new();
                for m in &s.metotlar {
                    for x in &m.govde {
                        let Ifade::Don(v) = x else { continue };
                        let iliski = |c: &Cagri| c.alici.as_ref().is_some_and(|a| matches!(a, Ifade::Degisken(t) if t == "this")) && ILISKILER.contains(&c.ad.as_str());
                        let Some(c) = zincir_koku(v, &iliski) else { continue };
                        let hedef = match c.arglar.first().map(|a| &a.deger) {
                            Some(Ifade::Sinif(h)) => kisa_ad(h).to_string(),
                            Some(Ifade::Metin(h)) => kisa_ad(h).to_string(),
                            _ if c.ad == "morphTo" => String::new(),
                            _ => continue,
                        };
                        let pivot = (c.ad == "belongsToMany").then(|| match c.arglar.get(1).map(|a| &a.deger) {
                            Some(Ifade::Metin(t)) => t.clone(),
                            _ => {
                                let mut ikili = [yilan(&s.ad), yilan(&hedef)];
                                ikili.sort();
                                ikili.join("_")
                            }
                        });
                        iliskiler.insert(m.ad.clone(), (c.ad.clone(), hedef, pivot));
                    }
                }
                let yumusak = b.trait_var((di, ci), "SoftDeletes");
                let tablo = acik.clone().unwrap_or_else(|| varsayilan_tablo(&s.ad));
                modeller.insert(s.ad.clone(), Model { yer: (di, ci), tablo, tablo_acik: acik.is_some(), yumusak, iliskiler });
            }
        }
        for (ad, m) in &modeller {
            b.tablolar.insert(m.tablo.to_lowercase(), ad.clone());
        }
        // Kimliği doğrulanan kullanıcının modeli: Authenticatable'dan türeyen (birden fazlaysa User)
        let kullanicilar: Vec<&String> =
            modeller.iter().filter(|(_, m)| b.turer(m.yer, &["Illuminate\\Foundation\\Auth\\User"]) || b.uygular(m.yer, "Authenticatable")).map(|(a, _)| a).collect();
        b.kullanici_modeli = match kullanicilar.as_slice() {
            [tek] => Some((*tek).clone()),
            _ => kullanicilar.iter().find(|a| a.as_str() == "User").map(|a| (*a).clone()).or_else(|| modeller.contains_key("User").then(|| "User".to_string())),
        };
        b.modeller = modeller;
        b
    }

    fn sinif_(&self, y: Yer) -> &'a Sinif {
        &self.dosyalar[y.0].siniflar[y.1]
    }

    fn goreli(&self, di: usize) -> String {
        guvenli::goreli(self.kok, &self.dosyalar[di].yol)
    }

    /// Dosyada yazıldığı biçimiyle bir sınıf adının tam adı (`use` takma adları ve ad alanı)
    fn tam_ad(&self, di: usize, ad: &str) -> String {
        let ad = ad.trim();
        if let Some(m) = ad.strip_prefix('\\') {
            return m.to_string();
        }
        let d = &self.dosyalar[di];
        let (ilk, kalan) = ad.split_once('\\').map(|(a, b)| (a, Some(b))).unwrap_or((ad, None));
        if let Some(t) = d.kullanimlar.get(&ilk.to_lowercase()) {
            return match kalan {
                Some(k) => format!("{t}\\{k}"),
                None => t.clone(),
            };
        }
        if d.ad_alani.is_empty() {
            ad.to_string()
        } else {
            format!("{}\\{ad}", d.ad_alani)
        }
    }

    /// Tam adla projedeki sınıf
    fn tam_sinif(&self, tam: &str) -> Option<Yer> {
        self.siniflar.get(&tam.trim_start_matches('\\').to_lowercase()).copied()
    }

    /// Dosyada yazılan sınıf adı projede hangi sınıf: tam ad, bulunamazsa projede tek olan kısa ad
    fn sinif(&self, di: usize, ad: &str) -> Option<Yer> {
        if matches!(ad, "self" | "static" | "parent" | "") {
            return None;
        }
        self.tam_sinif(&self.tam_ad(di, ad)).or_else(|| match self.kisa.get(&kisa_ad(ad).to_lowercase()).map(Vec::as_slice) {
            Some([tek]) => Some(*tek),
            _ => None,
        })
    }

    /// Projedeki sınıfın tam adı
    fn tam(&self, y: Yer) -> String {
        let d = &self.dosyalar[y.0];
        let s = &d.siniflar[y.1];
        if d.ad_alani.is_empty() {
            s.ad.clone()
        } else {
            format!("{}\\{}", d.ad_alani, s.ad)
        }
    }

    /// Sınıfın ataları (tam adlarıyla, en yakından): projede bulunmayan ilk atada durur
    fn atalar(&self, y: Yer) -> Vec<String> {
        let mut v = vec![];
        let mut x = y;
        for _ in 0..12 {
            let Some(t) = self.sinif_(x).taban.as_ref() else { break };
            let tam = self.tam_ad(x.0, t);
            v.push(tam.clone());
            match self.tam_sinif(&tam) {
                Some(u) if u != x => x = u,
                _ => break,
            }
        }
        v
    }

    /// Sınıf verilen tam adlardan birinden türüyor mu. Projede bulunmayan ata kısa adıyla da eşlenir (`use` satırı
    /// olmadan yazılmış taban); `Model` gibi genel kısa adlar yalnızca tam adla
    fn turer(&self, y: Yer, hedefler: &[&str]) -> bool {
        self.atalar(y).iter().any(|a| {
            hedefler.iter().any(|h| a.eq_ignore_ascii_case(h) || (self.tam_sinif(a).is_none() && kisa_ad(a) == kisa_ad(h) && !matches!(kisa_ad(h), "Model" | "User" | "Kernel")))
        })
    }

    /// Sınıf ya da ataları kısa adı verilen arayüzü uyguluyor mu
    fn uygular(&self, y: Yer, arayuz: &str) -> bool {
        let mut x = Some(y);
        for _ in 0..12 {
            let Some(u) = x else { break };
            if self.sinif_(u).arayuzler.iter().any(|a| kisa_ad(a) == arayuz) {
                return true;
            }
            x = self.ust(u);
        }
        false
    }

    /// Projedeki üst sınıf
    fn ust(&self, y: Yer) -> Option<Yer> {
        self.sinif_(y).taban.as_ref().and_then(|t| self.tam_sinif(&self.tam_ad(y.0, t))).filter(|z| *z != y)
    }

    /// Sınıf ya da ataları kısa adı verilen trait'i kullanıyor mu
    fn trait_var(&self, y: Yer, ad: &str) -> bool {
        self.trait_tam(y, ad).is_some()
    }

    /// Kullanılan trait'in tam adı
    fn trait_tam(&self, y: Yer, ad: &str) -> Option<String> {
        let mut x = Some(y);
        for _ in 0..12 {
            let u = x?;
            if let Some(t) = self.sinif_(u).traitler.iter().find(|t| kisa_ad(t) == ad) {
                return Some(self.tam_ad(u.0, t));
            }
            x = self.ust(u);
        }
        None
    }

    /// Metot: sınıfta, trait'lerinde, sonra atalarında (PHP'de metot adı büyük-küçük harf duyarsız)
    fn metot(&self, y: Yer, ad: &str) -> Option<(Yer, &'a Metot)> {
        let mut x = Some(y);
        for _ in 0..12 {
            let u = x?;
            let s = self.sinif_(u);
            if let Some(m) = s.metotlar.iter().find(|m| m.ad.eq_ignore_ascii_case(ad)) {
                return Some((u, m));
            }
            for t in &s.traitler {
                if let Some(ty) = self.sinif(u.0, t).filter(|t| *t != u) {
                    if let Some(m) = self.sinif_(ty).metotlar.iter().find(|m| m.ad.eq_ignore_ascii_case(ad)) {
                        return Some((ty, m));
                    }
                }
            }
            x = self.ust(u);
        }
        None
    }

    /// Özellik (ve terfi eden yapıcı parametresi): sınıfta, sonra atalarında
    fn ozellik(&self, y: Yer, ad: &str) -> Option<(Yer, &'a Ozellik)> {
        let mut x = Some(y);
        for _ in 0..12 {
            let u = x?;
            if let Some(o) = self.sinif_(u).ozellikler.iter().find(|o| o.ad == ad) {
                return Some((u, o));
            }
            x = self.ust(u);
        }
        None
    }

    /// Özelliğin sınıf tipi: bildirim tipi ya da yapıcıda `$this->x = $param` atamasındaki parametrenin tipi
    fn ozellik_tipi(&self, y: Yer, ad: &str) -> Option<Yer> {
        if let Some((u, o)) = self.ozellik(y, ad) {
            if let Some(t) = o.tip.as_deref() {
                return self.sinif(u.0, t);
            }
        }
        let (u, yapici) = self.metot(y, "__construct")?;
        for x in &yapici.govde {
            if let Ifade::Ata(sol, sag) = x {
                if matches!(sol.as_ref(), Ifade::Ozellik(n, a) if a == ad && matches!(n.as_ref(), Ifade::Degisken(t) if t == "this")) {
                    match sag.as_ref() {
                        Ifade::Degisken(p) => return yapici.parametreler.iter().find(|q| &q.ad == p).and_then(|q| q.tip.as_deref()).and_then(|t| self.sinif(u.0, t)),
                        Ifade::Yeni(s, _) => return self.sinif(u.0, s),
                        _ => {}
                    }
                }
            }
        }
        None
    }

    fn model_adi(&self, y: Yer) -> Option<String> {
        let ad = &self.sinif_(y).ad;
        self.modeller.get(ad).filter(|m| m.yer == y).map(|_| ad.clone())
    }
}

/// Laravel'in varsayılan tablo adı: sınıf adının snake_case çoğulu ("BlogPost" -> "blog_posts")
fn varsayilan_tablo(ad: &str) -> String {
    let y = yilan(ad);
    match y.rsplit_once('_') {
        Some((bas, son)) => format!("{bas}_{}", cogul(son)),
        None => cogul(&y),
    }
}

/// Gövdedeki bütün çağrılar için `f`; biri `true` dönerse durur
fn govde_ara(govde: &[Ifade], f: &mut dyn FnMut(&Cagri) -> bool) -> bool {
    govde.iter().any(|x| ifade_ara(x, f, 0))
}

fn ifade_ara(x: &Ifade, f: &mut dyn FnMut(&Cagri) -> bool, d: usize) -> bool {
    if d > EN_DERIN * 2 {
        return false;
    }
    match x {
        Ifade::Cagri(c) => f(c) || c.alici.as_ref().is_some_and(|a| ifade_ara(a, f, d + 1)) || c.arglar.iter().any(|a| ifade_ara(&a.deger, f, d + 1)),
        Ifade::Ozellik(a, _) | Ifade::Don(a) | Ifade::Dahil(a, _) | Ifade::Donustur(_, a) => ifade_ara(a, f, d + 1),
        Ifade::Dizi(v, _) => v.iter().any(|(a, b)| a.as_ref().is_some_and(|a| ifade_ara(a, f, d + 1)) || ifade_ara(b, f, d + 1)),
        Ifade::Yeni(_, a) => a.iter().any(|a| ifade_ara(&a.deger, f, d + 1)),
        Ifade::Kapanis(k) => k.govde.iter().any(|x| ifade_ara(x, f, d + 1)),
        Ifade::Birlesim(v) | Ifade::Kosul(_, v, _, _) | Ifade::Diger(_, v) => v.iter().any(|x| ifade_ara(x, f, d + 1)),
        Ifade::Dongu(k, _, _, v, _) => ifade_ara(k, f, d + 1) || v.iter().any(|x| ifade_ara(x, f, d + 1)),
        Ifade::Ata(a, b) => ifade_ara(a, f, d + 1) || ifade_ara(b, f, d + 1),
        _ => false,
    }
}

/// Bir ifade ağacının bütün düğümleri için `f` (kapanış gövdeleri dahil)
fn ifade_gez<'x>(x: &'x Ifade, f: &mut dyn FnMut(&'x Ifade), d: usize) {
    if d > EN_DERIN * 2 {
        return;
    }
    f(x);
    match x {
        Ifade::Cagri(c) => {
            if let Some(a) = &c.alici {
                ifade_gez(a, f, d + 1);
            }
            for a in &c.arglar {
                ifade_gez(&a.deger, f, d + 1);
            }
        }
        Ifade::Ozellik(a, _) | Ifade::Don(a) | Ifade::Dahil(a, _) | Ifade::Donustur(_, a) => ifade_gez(a, f, d + 1),
        Ifade::Dizi(v, _) => {
            for (a, b) in v {
                if let Some(a) = a {
                    ifade_gez(a, f, d + 1);
                }
                ifade_gez(b, f, d + 1);
            }
        }
        Ifade::Yeni(_, a) => {
            for a in a {
                ifade_gez(&a.deger, f, d + 1);
            }
        }
        Ifade::Kapanis(k) => {
            for x in &k.govde {
                ifade_gez(x, f, d + 1);
            }
        }
        Ifade::Birlesim(v) | Ifade::Kosul(_, v, _, _) | Ifade::Diger(_, v) => {
            for x in v {
                ifade_gez(x, f, d + 1);
            }
        }
        Ifade::Dongu(k, _, _, v, _) => {
            ifade_gez(k, f, d + 1);
            for x in v {
                ifade_gez(x, f, d + 1);
            }
        }
        Ifade::Ata(a, b) => {
            ifade_gez(a, f, d + 1);
            ifade_gez(b, f, d + 1);
        }
        _ => {}
    }
}

/// Zincirde (`a()->b()->c()`, dıştan köke) koşulu sağlayan en içteki (köke en yakın) çağrı
fn zincir_koku<'x>(x: &'x Ifade, kosul: &dyn Fn(&Cagri) -> bool) -> Option<&'x Cagri> {
    let mut y = x;
    let mut bulunan = None;
    for _ in 0..64 {
        let Ifade::Cagri(c) = y else { break };
        if kosul(c) {
            bulunan = Some(c.as_ref());
        }
        match &c.alici {
            Some(a) if !c.statik => y = a,
            _ => break,
        }
    }
    bulunan
}

/// Zincirin halkaları kökten sona ve kök: `Route::prefix('x')->middleware('y')->group(...)` -> (Route, [prefix, middleware,
/// group]). Statik çağrı halkadır, alıcısı köktür
fn halkalar(x: &Ifade) -> (Option<&Ifade>, Vec<&Cagri>) {
    let mut v = vec![];
    let mut y = x;
    let mut kok = None;
    for _ in 0..256 {
        let Ifade::Cagri(c) = y else {
            kok = Some(y);
            break;
        };
        v.push(c.as_ref());
        match &c.alici {
            Some(a) if !c.statik && matches!(a, Ifade::Cagri(_)) => y = a,
            a => {
                kok = a.as_ref();
                break;
            }
        }
    }
    v.reverse();
    (kok, v)
}

fn arg<'x>(c: &'x Cagri, ad: &str, sira: usize) -> Option<&'x Ifade> {
    c.arglar.iter().find(|a| a.ad.as_deref() == Some(ad)).or_else(|| c.arglar.iter().filter(|a| a.ad.is_none()).nth(sira)).map(|a| &a.deger)
}

/// Metin listesi: 'a', ['a', 'b']
fn metinler(x: &Ifade) -> Vec<String> {
    match x {
        Ifade::Metin(t) => vec![t.clone()],
        Ifade::Dizi(v, _) => v.iter().filter_map(|(_, e)| if let Ifade::Metin(t) = e { Some(t.clone()) } else { None }).collect(),
        _ => vec![],
    }
}

/// Çağrının bütün metin argümanları (`except('update', 'destroy')` ya da `except(['update'])`)
fn arg_metinleri(c: &Cagri) -> Vec<String> {
    c.arglar.iter().flat_map(|a| metinler(&a.deger)).collect()
}

// ---- Ara katmanlar ----

/// Ara katman girişi: takma ad (parametreleriyle: "auth:sanctum") ya da sınıfın tam adı
#[derive(Clone, Debug, PartialEq)]
enum Ara {
    Takma(String),
    Sinif(String),
    /// Çalışma anında hesaplanan takma ad: projedeki enum metodunun döndürdüğü metin
    /// (`Permission::X->middleware()` -> "can:x"); anlamı takma adınki, ama kaynakta yazılı değil
    Hesap(String),
}

impl Ara {
    fn metin(&self) -> String {
        match self {
            Ara::Takma(t) | Ara::Hesap(t) => t.clone(),
            Ara::Sinif(s) => kisa_ad(s).to_string(),
        }
    }
    fn ad(&self) -> String {
        match self {
            Ara::Takma(t) | Ara::Hesap(t) => t.split(':').next().unwrap_or(t).to_string(),
            Ara::Sinif(s) => s.clone(),
        }
    }
}

#[derive(Clone, Debug)]
enum AraTur {
    /// Giriş ister (kesin: çatının Authenticate sınıfı ya da ondan türeyen)
    Kimlik(bool),
    /// Rol / izin ister: roller, kesin mi
    Rol(Vec<String>, bool),
    /// `can:yetenek,model`: politika ya da Gate; kesin mi (yetenek kaynakta yazılı ve çatının Authorize sınıfı)
    Yetki(String, bool),
    Misafir,
    Notr,
    Bilinmeyen,
}

/// Takma adlar, gruplar ve genel ara katmanlar: Laravel'in varsayılanları, Kernel ve `bootstrap/app.php` `withMiddleware`
struct AraOrtam {
    takma: HashMap<String, String>,
    gruplar: HashMap<String, Vec<Ara>>,
    genel: Vec<Ara>,
}

const KIMLIK_SINIFLARI: [&str; 10] = [
    "Illuminate\\Auth\\Middleware\\Authenticate",
    "Illuminate\\Auth\\Middleware\\AuthenticateWithBasicAuth",
    "Illuminate\\Auth\\Middleware\\EnsureEmailIsVerified",
    "Laravel\\Sanctum\\Http\\Middleware\\CheckAbilities",
    "Laravel\\Sanctum\\Http\\Middleware\\CheckForAnyAbility",
    "Laravel\\Passport\\Http\\Middleware\\CheckScopes",
    "Laravel\\Passport\\Http\\Middleware\\CheckForAnyScope",
    "Laravel\\Passport\\Http\\Middleware\\CheckToken",
    "Laravel\\Passport\\Http\\Middleware\\CheckTokenForAnyScope",
    "Laravel\\Passport\\Http\\Middleware\\CheckClientCredentials",
];
const ROL_SINIFLARI: [&str; 6] = [
    "Spatie\\Permission\\Middleware\\RoleMiddleware",
    "Spatie\\Permission\\Middleware\\PermissionMiddleware",
    "Spatie\\Permission\\Middleware\\RoleOrPermissionMiddleware",
    "Spatie\\Permission\\Middlewares\\RoleMiddleware",
    "Spatie\\Permission\\Middlewares\\PermissionMiddleware",
    "Spatie\\Permission\\Middlewares\\RoleOrPermissionMiddleware",
];
/// Kimlik istemeyen, tanımı bilinen paketlerin ad alanları
const NOTR_AD_ALANLARI: [&str; 9] = ["Illuminate\\", "Symfony\\", "Laravel\\", "Fruitcake\\", "Inertia\\", "Barryvdh\\", "Livewire\\", "Spatie\\", "Sentry\\"];

impl AraOrtam {
    fn varsayilan() -> Self {
        let takma = [
            ("auth", "Illuminate\\Auth\\Middleware\\Authenticate"),
            ("auth.basic", "Illuminate\\Auth\\Middleware\\AuthenticateWithBasicAuth"),
            ("auth.session", "Illuminate\\Session\\Middleware\\AuthenticateSession"),
            ("cache.headers", "Illuminate\\Http\\Middleware\\SetCacheHeaders"),
            ("can", "Illuminate\\Auth\\Middleware\\Authorize"),
            ("guest", "Illuminate\\Auth\\Middleware\\RedirectIfAuthenticated"),
            ("password.confirm", "Illuminate\\Auth\\Middleware\\RequirePassword"),
            ("precognitive", "Illuminate\\Foundation\\Http\\Middleware\\HandlePrecognitiveRequests"),
            ("signed", "Illuminate\\Routing\\Middleware\\ValidateSignature"),
            ("throttle", "Illuminate\\Routing\\Middleware\\ThrottleRequests"),
            ("verified", "Illuminate\\Auth\\Middleware\\EnsureEmailIsVerified"),
            ("bindings", "Illuminate\\Routing\\Middleware\\SubstituteBindings"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let s = |x: &str| Ara::Sinif(x.to_string());
        let gruplar = HashMap::from([
            (
                "web".to_string(),
                vec![
                    s("Illuminate\\Cookie\\Middleware\\EncryptCookies"),
                    s("Illuminate\\Cookie\\Middleware\\AddQueuedCookiesToResponse"),
                    s("Illuminate\\Session\\Middleware\\StartSession"),
                    s("Illuminate\\View\\Middleware\\ShareErrorsFromSession"),
                    s("Illuminate\\Foundation\\Http\\Middleware\\ValidateCsrfToken"),
                    s("Illuminate\\Routing\\Middleware\\SubstituteBindings"),
                ],
            ),
            ("api".to_string(), vec![s("Illuminate\\Routing\\Middleware\\SubstituteBindings")]),
        ]);
        AraOrtam { takma, gruplar, genel: vec![] }
    }
}

impl Baglam<'_> {
    /// Ara katman ifadesi (metin, `X::class`, dizi) -> girişler
    fn ara_girisleri(&self, di: usize, x: &Ifade) -> Vec<Ara> {
        match x {
            Ifade::Metin(t) if t.contains('\\') => vec![Ara::Sinif(t.trim_start_matches('\\').to_string())],
            Ifade::Metin(t) => vec![Ara::Takma(t.clone())],
            Ifade::Sinif(c) => vec![Ara::Sinif(self.tam_ad(di, c))],
            Ifade::Birlesim(v) => match v.first() {
                Some(Ifade::Metin(t)) => vec![Ara::Takma(format!("{t}…"))],
                Some(Ifade::Sinif(c)) => vec![Ara::Sinif(self.tam_ad(di, c))],
                _ => vec![],
            },
            Ifade::Dizi(v, _) => v.iter().flat_map(|(_, e)| self.ara_girisleri(di, e)).collect(),
            Ifade::Cagri(c) => self.enum_metni(di, c).map(|t| vec![Ara::Hesap(t)]).unwrap_or_default(),
            _ => vec![],
        }
    }

    /// Projedeki enum durumunun metin döndüren metodu: `Permission::ContentExport->middleware()` ve metodun gövdesi
    /// `return 'can:' . $this->value;` -> "can:content-export". Gövde metin ve `$this->value` / `$this->name`
    /// birleşiminden başka bir şeyse None
    fn enum_metni(&self, di: usize, c: &Cagri) -> Option<String> {
        let (Some(Ifade::Ad(durum)), false, true) = (&c.alici, c.statik, c.arglar.is_empty()) else { return None };
        let (sinif, ad) = durum.rsplit_once("::")?;
        let y = self.sinif(di, sinif).filter(|y| self.sinif_(*y).tur == "enum")?;
        let (ad, deger) = self.sinif_(y).durumlar.iter().find(|(a, _)| a == ad)?;
        let (_, m) = self.metot(y, &c.ad)?;
        let [Ifade::Don(x)] = m.govde.as_slice() else { return None };
        let parcalar = match x.as_ref() {
            Ifade::Birlesim(v) => v.as_slice(),
            x => std::slice::from_ref(x),
        };
        let mut t = String::new();
        for p in parcalar {
            match p {
                Ifade::Metin(m) => t.push_str(m),
                Ifade::Ozellik(o, a) if matches!(o.as_ref(), Ifade::Degisken(v) if v == "this") && a == "value" => t.push_str(deger.as_deref().unwrap_or(ad)),
                Ifade::Ozellik(o, a) if matches!(o.as_ref(), Ifade::Degisken(v) if v == "this") && a == "name" => t.push_str(ad),
                _ => return None,
            }
        }
        Some(t)
    }

    /// İzin (yetenek) adı: metin ya da projedeki enum durumu / sınıf sabiti (`Permission::SettingsManage`)
    fn izin_adi(&self, di: usize, x: &Ifade) -> Option<String> {
        match x {
            Ifade::Metin(t) => Some(t.clone()),
            Ifade::Ad(s) => {
                let (sinif, ad) = s.rsplit_once("::")?;
                let y = self.sinif(di, sinif)?;
                if let Some((a, v)) = self.sinif_(y).durumlar.iter().find(|(a, _)| a == ad) {
                    return Some(v.clone().unwrap_or_else(|| a.clone()));
                }
                match self.ozellik(y, &format!("::{ad}")).and_then(|(_, o)| o.deger.as_ref()) {
                    Some(Ifade::Metin(t)) => Some(t.clone()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Kernel ve `bootstrap/app.php` ile kurulan ara katman ortamı
    fn ara_ortami(&self) -> AraOrtam {
        let mut o = AraOrtam::varsayilan();
        // Laravel 10 ve öncesi: App\Http\Kernel
        for (di, d) in self.dosyalar.iter().enumerate() {
            for (ci, s) in d.siniflar.iter().enumerate() {
                if !self.turer((di, ci), &["Illuminate\\Foundation\\Http\\Kernel"]) {
                    continue;
                }
                for oz in &s.ozellikler {
                    let Some(deger) = &oz.deger else { continue };
                    match oz.ad.as_str() {
                        "middleware" => o.genel = self.ara_girisleri(di, deger),
                        "middlewareGroups" => {
                            if let Ifade::Dizi(v, _) = deger {
                                for (a, e) in v {
                                    if let Some(Ifade::Metin(g)) = a {
                                        o.gruplar.insert(g.clone(), self.ara_girisleri(di, e));
                                    }
                                }
                            }
                        }
                        "middlewareAliases" | "routeMiddleware" => {
                            if let Ifade::Dizi(v, _) = deger {
                                for (a, e) in v {
                                    if let (Some(Ifade::Metin(t)), Some(Ara::Sinif(s))) = (a, self.ara_girisleri(di, e).into_iter().next()) {
                                        o.takma.insert(t.clone(), s);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        // Laravel 11+: bootstrap/app.php ->withMiddleware(function (Middleware $middleware) { ... })
        for (di, d) in self.dosyalar.iter().enumerate() {
            for x in &d.ifadeler {
                let x = match x {
                    Ifade::Don(v) => v.as_ref(),
                    v => v,
                };
                let (_, h) = halkalar(x);
                for c in h.iter().filter(|c| c.ad == "withMiddleware") {
                    let Some(Ifade::Kapanis(k)) = arg(c, "callback", 0) else { continue };
                    let Some(p) = k.parametreler.first() else { continue };
                    for s in &k.govde {
                        let (kok, zincir) = halkalar(s);
                        if !matches!(kok, Some(Ifade::Degisken(v)) if *v == p.ad) {
                            continue;
                        }
                        for c in zincir {
                            self.ortam_cagrisi(di, c, &mut o);
                        }
                    }
                }
            }
        }
        o
    }

    /// `$middleware->alias([...])`, `->api(prepend: [...], append: [...])`, `->web(...)`, `->group('ad', [...])`,
    /// `->appendToGroup('ad', ...)`, `->append(...)`, `->prepend(...)`, `->use([...])`, `->statefulApi()`
    fn ortam_cagrisi(&self, di: usize, c: &Cagri, o: &mut AraOrtam) {
        let g = |ad: &str, sira: usize| arg(c, ad, sira).map(|x| self.ara_girisleri(di, x)).unwrap_or_default();
        match c.ad.as_str() {
            "alias" => {
                if let Some(Ifade::Dizi(v, _)) = arg(c, "aliases", 0) {
                    for (a, e) in v {
                        if let (Some(Ifade::Metin(t)), Some(Ara::Sinif(s))) = (a, self.ara_girisleri(di, e).into_iter().next()) {
                            o.takma.insert(t.clone(), s);
                        }
                    }
                }
            }
            grup @ ("web" | "api") => {
                let mut l = o.gruplar.remove(grup).unwrap_or_default();
                if let Some(Ifade::Dizi(v, _)) = arg(c, "replace", 99) {
                    for (a, e) in v {
                        if let (Some(a), Some(yeni)) = (a.as_ref().map(|a| self.ara_girisleri(di, a)), self.ara_girisleri(di, e).into_iter().next()) {
                            for x in l.iter_mut().filter(|x| a.contains(x)) {
                                *x = yeni.clone();
                            }
                        }
                    }
                }
                let cikar = g("remove", 99);
                l.retain(|x| !cikar.contains(x));
                let mut on = g("prepend", 99);
                on.append(&mut l);
                on.extend(g("append", 99));
                o.gruplar.insert(grup.to_string(), on);
            }
            "group" => {
                if let Some(Ifade::Metin(ad)) = arg(c, "group", 0) {
                    o.gruplar.insert(ad.clone(), g("middleware", 1));
                }
            }
            "appendToGroup" | "prependToGroup" => {
                if let Some(Ifade::Metin(ad)) = arg(c, "group", 0) {
                    let yeni = g("middleware", 1);
                    let l = o.gruplar.entry(ad.clone()).or_default();
                    if c.ad == "appendToGroup" {
                        l.extend(yeni);
                    } else {
                        let mut v = yeni;
                        v.append(l);
                        *l = v;
                    }
                }
            }
            "append" => o.genel.extend(g("middleware", 0)),
            "prepend" => {
                let mut v = g("middleware", 0);
                v.append(&mut o.genel);
                o.genel = v;
            }
            "use" => o.genel = g("middleware", 0),
            "remove" => {
                let cikar = g("middleware", 0);
                o.genel.retain(|x| !cikar.contains(x));
            }
            "statefulApi" => o.gruplar.entry("api".into()).or_default().insert(0, Ara::Sinif("Laravel\\Sanctum\\Http\\Middleware\\EnsureFrontendRequestsAreStateful".into())),
            _ => {}
        }
    }

    /// Ara katmanın türü ve (projedeyse) sınıfı
    fn ara_turu(&self, o: &AraOrtam, a: &Ara) -> (AraTur, Option<Yer>) {
        let hesap = matches!(a, Ara::Hesap(_));
        let (sinif, param) = match a {
            Ara::Takma(t) | Ara::Hesap(t) => {
                let (ad, param) = t.split_once(':').map(|(a, p)| (a, Some(p.to_string()))).unwrap_or((t.as_str(), None));
                match o.takma.get(ad) {
                    Some(s) => (s.clone(), param),
                    // Kayıtlı olmayan rol takma adı: Spatie benzeri bir paket varsayılır, kesin değil
                    None if matches!(ad, "role" | "permission" | "role_or_permission") => return (AraTur::Rol(rol_parametresi(param.as_deref()), false), None),
                    None => return (AraTur::Bilinmeyen, None),
                }
            }
            Ara::Sinif(s) => (s.clone(), None),
        };
        let yer = self.tam_sinif(&sinif);
        let mut zincir = vec![sinif.clone()];
        if let Some(y) = yer {
            zincir.extend(self.atalar(y));
        }
        let eslesir = |liste: &[&str]| zincir.iter().any(|z| liste.iter().any(|l| z.eq_ignore_ascii_case(l)));
        let tur = if eslesir(&KIMLIK_SINIFLARI) {
            AraTur::Kimlik(true)
        } else if eslesir(&ROL_SINIFLARI) {
            AraTur::Rol(rol_parametresi(param.as_deref()), !hesap)
        } else if eslesir(&["Illuminate\\Auth\\Middleware\\Authorize"]) {
            AraTur::Yetki(param.unwrap_or_default(), !hesap)
        } else if eslesir(&["Illuminate\\Auth\\Middleware\\RedirectIfAuthenticated"]) {
            AraTur::Misafir
        } else if let Some(y) = yer {
            // Projedeki ara katman: handle() gövdesi ve sınıf içinden çağırdığı metotlar okunur. Takma ad Laravel'in alışılmış
            // adıysa (auth, can, role, permission, guest) anlamı ondan varsayılır
            let takma = match a {
                Ara::Takma(t) | Ara::Hesap(t) => t.split(':').next().unwrap_or(""),
                Ara::Sinif(_) => "",
            };
            match (takma, self.metot(y, "handle").or_else(|| self.metot(y, "__invoke"))) {
                // Projenin `can` ara katmanı: izin adı parametreden, kararı proje kodunda (yorum)
                ("can", _) => AraTur::Yetki(param.unwrap_or_default(), false),
                ("role" | "permission" | "role_or_permission", _) => AraTur::Rol(rol_parametresi(param.as_deref()), false),
                ("guest", _) => AraTur::Misafir,
                (_, Some((u, m))) => {
                    let (kimlik, red) = self.izler(u, m, 0);
                    // Sınıfın adı da kimlik doğruladığını söylüyorsa (AuthenticateAudioRequests, ApiAuth) tek iz yeter
                    let ad_izi = crate::kural::derle(r"(?i)authenticat|auth$|^auth").is_match(&self.sinif_(y).ad);
                    if (kimlik && red) || (ad_izi && (kimlik || red)) || takma == "auth" {
                        AraTur::Kimlik(false)
                    } else {
                        AraTur::Notr
                    }
                }
                ("auth", None) => AraTur::Kimlik(false),
                (_, None) if self.atalar(y).last().is_some_and(|a| NOTR_AD_ALANLARI.iter().any(|n| a.starts_with(n))) => AraTur::Notr,
                (_, None) => AraTur::Bilinmeyen,
            }
        } else if NOTR_AD_ALANLARI.iter().any(|n| sinif.starts_with(n)) {
            AraTur::Notr
        } else {
            AraTur::Bilinmeyen
        };
        (tur, yer)
    }

    /// Metodun ve sınıf içinden çağırdığı metotların (`$this->x()`, 2 kat) kimlik ve red izleri
    fn izler(&self, y: Yer, m: &Metot, d: usize) -> (bool, bool) {
        let (mut kimlik, mut red) = (m.kimlik_izi, m.red_izi);
        if d < 2 && !(kimlik && red) {
            let mut adlar = vec![];
            govde_ara(&m.govde, &mut |c| {
                if matches!(&c.alici, Some(Ifade::Degisken(t)) if t == "this") {
                    adlar.push(c.ad.clone());
                }
                false
            });
            for a in adlar {
                if let Some((u, mm)) = self.metot(y, &a).filter(|(_, mm)| !std::ptr::eq(*mm, m)) {
                    let (k, r) = self.izler(u, mm, d + 1);
                    kimlik |= k;
                    red |= r;
                }
            }
        }
        (kimlik, red)
    }

    /// Grupları açar (iç içe, en çok 4 kat) ve `withoutMiddleware` ile çıkarılanları atar
    fn ara_ac(&self, o: &AraOrtam, liste: &[(Ara, String)], haric: &[Ara]) -> Vec<(Ara, String)> {
        let mut v: Vec<(Ara, String)> = vec![];
        let mut yigin: Vec<(Ara, String, usize)> = liste.iter().rev().map(|(a, k)| (a.clone(), k.clone(), 0)).collect();
        while let Some((a, k, d)) = yigin.pop() {
            if let Ara::Takma(t) = &a {
                if let Some(g) = o.gruplar.get(t).filter(|_| d < 4) {
                    yigin.extend(g.iter().rev().map(|x| (x.clone(), k.clone(), d + 1)));
                    continue;
                }
            }
            let cikti = haric.iter().any(|h| {
                h == &a
                    || h.ad() == a.ad()
                    || match (h, &a) {
                        (Ara::Takma(t), Ara::Sinif(s)) | (Ara::Sinif(s), Ara::Takma(t)) => o.takma.get(t.split(':').next().unwrap_or(t)).is_some_and(|x| x.eq_ignore_ascii_case(s)),
                        _ => false,
                    }
            });
            if !cikti && !v.iter().any(|(x, _)| *x == a) {
                v.push((a, k));
            }
        }
        v
    }
}

/// `role:admin|editor,web` -> ["admin", "editor"] (virgülden sonrası koruma adıdır)
fn rol_parametresi(p: Option<&str>) -> Vec<String> {
    p.map(|p| p.split(',').next().unwrap_or("").split('|').map(|r| r.trim().to_string()).filter(|r| !r.is_empty()).collect()).unwrap_or_default()
}

// ---- Rotalar ----

/// Değerlendirilen metin yollarında uygulama kökünün yeri
const KOK_ISARETI: &str = "\u{1}kok";

/// Rota grubu nitelikleri: önek, ad öneki, ara katmanlar (kanıt konumlarıyla), çıkarılanlar, controller ad alanı,
/// `Route::controller(...)`, parametre desenleri, koşullar
#[derive(Clone, Debug)]
struct Grup {
    onek: Vec<String>,
    ad: String,
    ara: Vec<(Ara, String)>,
    haric: Vec<Ara>,
    ad_alani: Option<String>,
    kontrolcu: Option<(usize, String)>,
    desenler: Vec<(String, String)>,
    kosullar: Vec<String>,
    dongu: bool,
    /// Rota dosyası
    dosya: Option<usize>,
    /// Ana uygulamaya bağlı mı (yükleyicisi bulunamayan rota dosyası ikincildir)
    ana: bool,
}

impl Default for Grup {
    fn default() -> Self {
        Grup {
            onek: vec![],
            ad: String::new(),
            ara: vec![],
            haric: vec![],
            ad_alani: None,
            kontrolcu: None,
            desenler: vec![],
            kosullar: vec![],
            dongu: false,
            dosya: None,
            ana: true,
        }
    }
}

#[derive(Clone, Debug)]
enum Eylem<'a> {
    /// Controller metodu: adın yazıldığı dosya, sınıf adı, adın tam ad olup olmadığı, metot
    Metot {
        di: usize,
        sinif: String,
        tam: bool,
        metot: String,
    },
    Kapanis(usize, &'a Kapanis),
    Gorunum,
    Saglik,
}

#[derive(Clone, Debug)]
struct Rota<'a> {
    yontemler: Vec<String>,
    /// Laravel biçiminde tam yol: "/api/posts/{post}"
    uri: String,
    eylem: Eylem<'a>,
    grup: Grup,
    ad: Option<String>,
    di: usize,
    satir: usize,
}

/// Değerlendirmede bağlı değişkenler: kaynaktaki ifade ya da çözülmüş metin
#[derive(Clone)]
enum Deger<'a> {
    Ifade(&'a Ifade),
    Metin(String),
}

type Degerler<'a> = HashMap<String, Deger<'a>>;

/// Yükleme: (rota dosyası, yükleyen dosya, satır, koşul)
type Yukleme = (usize, usize, usize, Option<String>);

/// Kayıt sürerken toplanan nitelikler (zincirin kayıttan sonraki halkaları)
struct Kayit<'a> {
    tur: KayitTur<'a>,
    grup: Grup,
    ad: Option<String>,
    sadece: Option<Vec<String>>,
    haric_metotlar: Vec<String>,
    parametre_adlari: HashMap<String, String>,
    sig: bool,
    olusturulabilir: bool,
    silinebilir: bool,
    di: usize,
    satir: usize,
}

enum KayitTur<'a> {
    Tekil {
        yontemler: Vec<String>,
        uri: String,
        eylem: Eylem<'a>,
    },
    /// (kaynak adı, controller (dosya, ad, tam ad mı)), api mi, singleton mı
    Kaynak(Vec<(String, (usize, String, bool))>, bool, bool),
}

const KAYIT_FIILLERI: [&str; 15] =
    ["get", "post", "put", "patch", "delete", "options", "any", "match", "view", "resource", "apiResource", "resources", "apiResources", "singleton", "apiSingleton"];
const HER_YONTEM: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];

struct RotaMotoru<'b, 'a> {
    b: &'b Baglam<'a>,
    rotalar: Vec<Rota<'a>>,
    yuklemeler: Vec<Yukleme>,
    /// İçinde bulunulan dosyalar (döngüye karşı)
    yigin: Vec<usize>,
    adim: usize,
    /// `Route::pattern` ile verilen genel desenler
    desenler: Vec<(String, String)>,
    /// Değerlendirilen metotlar (sağlayıcılar, yardımcılar): (yer, metot adı)
    gezilen: HashSet<(Yer, String)>,
    /// Değerlendirilemeyen (dinamik) rota kayıtları: "dosya:satır"
    dinamik: Vec<String>,
    /// API rota dosyasının öneki (genel önek)
    api_oneki: Option<String>,
    /// Yükleyicilerin kanıt yeri: bootstrap/app.php ya da RouteServiceProvider ("dosya")
    yukleyici: Option<usize>,
}

/// "api/v1/" -> ["api", "v1"]
fn parcalar(s: &str) -> Vec<String> {
    s.split('/').map(str::trim).filter(|p| !p.is_empty()).map(String::from).collect()
}

/// Yolu parçalara ayırıp `..` ve `.` çözer
fn yol_duzle(s: &str) -> String {
    let mut v: Vec<&str> = vec![];
    for p in s.split(['/', '\\']) {
        match p {
            "" | "." => {}
            ".." => {
                v.pop();
            }
            p => v.push(p),
        }
    }
    v.join("/")
}

impl<'b, 'a> RotaMotoru<'b, 'a> {
    fn yeni(b: &'b Baglam<'a>) -> Self {
        RotaMotoru { b, rotalar: vec![], yuklemeler: vec![], yigin: vec![], adim: 0, desenler: vec![], gezilen: HashSet::new(), dinamik: vec![], api_oneki: None, yukleyici: None }
    }

    fn konum(&self, di: usize, satir: usize) -> String {
        format!("{}:{satir}", self.b.goreli(di))
    }

    /// Metin değeri: sabit, birleştirme, `__DIR__`, `base_path()`, `app_path()`, `sprintf`, bağlı değişken, `$this->x`
    /// ve sınıf sabiti. Yollar kökten `KOK_ISARETI` ile başlar
    fn deger(&self, di: usize, ctx: Option<Yer>, x: &Ifade, vars: &Degerler<'a>, d: usize) -> Option<String> {
        if d > 12 {
            return None;
        }
        let dizin = |di: usize| {
            let g = self.b.goreli(di);
            let ust = g.rsplit_once('/').map(|x| x.0).unwrap_or("");
            if ust.is_empty() {
                KOK_ISARETI.to_string()
            } else {
                format!("{KOK_ISARETI}/{ust}")
            }
        };
        match x {
            Ifade::Metin(t) | Ifade::Sayi(t) => Some(t.clone()),
            Ifade::Birlesim(v) => v.iter().map(|p| self.deger(di, ctx, p, vars, d + 1)).collect::<Option<Vec<_>>>().map(|v| v.concat()),
            Ifade::Ad(a) if a == "__DIR__" => Some(dizin(di)),
            Ifade::Ad(a) if a.contains("::") => {
                let (s, c) = a.split_once("::")?;
                let y = if matches!(s, "self" | "static") { ctx? } else { self.b.sinif(di, s)? };
                let (u, o) = self.b.ozellik(y, &format!("::{c}"))?;
                self.deger(u.0, Some(u), o.deger.as_ref()?, &Degerler::new(), d + 1)
            }
            Ifade::Degisken(v) => match vars.get(v)? {
                Deger::Ifade(x) => self.deger(di, ctx, x, vars, d + 1),
                Deger::Metin(t) => Some(t.clone()),
            },
            Ifade::Ozellik(n, p) if matches!(n.as_ref(), Ifade::Degisken(t) if t == "this") => {
                let (u, o) = self.b.ozellik(ctx?, p)?;
                self.deger(u.0, Some(u), o.deger.as_ref()?, &Degerler::new(), d + 1)
            }
            Ifade::Donustur(_, x) => self.deger(di, ctx, x, vars, d + 1),
            Ifade::Cagri(c) if c.alici.is_none() => {
                let a0 = || arg(c, "path", 0).and_then(|x| self.deger(di, ctx, x, vars, d + 1)).unwrap_or_default();
                let ekle = |taban: &str, p: String| if p.is_empty() { taban.to_string() } else { format!("{taban}/{}", p.trim_start_matches('/')) };
                match c.ad.as_str() {
                    "base_path" => Some(ekle(KOK_ISARETI, a0())),
                    "app_path" => Some(ekle(&format!("{KOK_ISARETI}/app"), a0())),
                    "database_path" => Some(ekle(&format!("{KOK_ISARETI}/database"), a0())),
                    "resource_path" => Some(ekle(&format!("{KOK_ISARETI}/resources"), a0())),
                    "dirname" => {
                        let v = self.deger(di, ctx, arg(c, "path", 0)?, vars, d + 1)?;
                        let v = yol_duzle(&v);
                        Some(v.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or(v))
                    }
                    "sprintf" => {
                        let mut bicim = self.deger(di, ctx, arg(c, "format", 0)?, vars, d + 1)?;
                        for a in c.arglar.iter().skip(1) {
                            let v = self.deger(di, ctx, &a.deger, vars, d + 1)?;
                            let i = bicim.find("%s").or_else(|| bicim.find("%d"))?;
                            bicim.replace_range(i..i + 2, &v);
                        }
                        Some(bicim)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Değişkeni kaynaktaki ifadesine çözer (`$uclar` -> `[...]`); metne çözülen değişken için yok
    fn coz_ifade(&self, x: &'a Ifade, vars: &Degerler<'a>, d: usize) -> Option<&'a Ifade> {
        match x {
            Ifade::Degisken(v) if d < 8 => match vars.get(v)? {
                Deger::Ifade(y) => self.coz_ifade(y, vars, d + 1),
                Deger::Metin(_) => None,
            },
            x => Some(x),
        }
    }

    /// Ara katman argümanı: değişkense çözülür (`->withoutMiddleware($oturum)`)
    fn ara_argumani(&self, di: usize, x: &'a Ifade, vars: &Degerler<'a>) -> Vec<Ara> {
        match (x, self.coz_ifade(x, vars, 0)) {
            (_, Some(y)) => self.b.ara_girisleri(di, y),
            (Ifade::Degisken(v), None) => match vars.get(v) {
                Some(Deger::Metin(t)) => self.b.ara_girisleri(di, &Ifade::Metin(t.clone())),
                _ => vec![],
            },
            _ => vec![],
        }
    }

    /// Değerlendirilmiş yolun dosyası
    fn dosya_bul(&self, yol: &str) -> Option<usize> {
        let duz = yol_duzle(yol);
        let goreli = duz.strip_prefix(&format!("{}/", yol_duzle(KOK_ISARETI)))?;
        self.b.yollar.get(goreli).copied()
    }

    /// Deyim listesi: rota zincirleri, koşullar, `require`, yükleyiciler
    fn deyimler(&mut self, di: usize, ctx: Option<Yer>, ifadeler: &'a [Ifade], g: &Grup, vars: &Degerler<'a>, d: usize) {
        if d > 24 {
            return;
        }
        // Deyimlerdeki atamalar sonraki deyimlerde kullanılır: $uclar = [...]; $kapanis = fn () => ...;
        let mut yerel = vars.clone();
        for x in ifadeler {
            self.adim += 1;
            if self.adim > 200_000 {
                return;
            }
            match x {
                Ifade::Ata(sol, sag) => {
                    if let Ifade::Degisken(v) = sol.as_ref() {
                        yerel.insert(v.clone(), Deger::Ifade(sag));
                    }
                }
                Ifade::Kosul(k, govde, dongu, _) => {
                    let mut g2 = g.clone();
                    if k != "catch" {
                        g2.kosullar.push(k.clone());
                    }
                    g2.dongu |= dongu;
                    self.deyimler(di, ctx, govde, &g2, &yerel, d + 1);
                }
                // Sabit bir dizi üzerinde döngü öğe öğe açılır (`foreach ($uclar as $ad => $controller) Route::...`);
                // kaynağı çözülemeyen döngüdeki kayıtlar koşullu sayılır
                Ifade::Dongu(k, anahtar, deger, govde, baslik) => match self.coz_ifade(k, &yerel, 0) {
                    Some(Ifade::Dizi(v, _)) if v.len() <= 512 => {
                        for (i, (a, e)) in v.iter().enumerate() {
                            let mut v2 = yerel.clone();
                            if let Some(an) = anahtar {
                                v2.insert(an.clone(), a.as_ref().map(Deger::Ifade).unwrap_or_else(|| Deger::Metin(i.to_string())));
                            }
                            v2.insert(deger.clone(), Deger::Ifade(e));
                            self.deyimler(di, ctx, govde, g, &v2, d + 1);
                        }
                    }
                    _ => {
                        let mut g2 = g.clone();
                        g2.kosullar.push(baslik.clone());
                        g2.dongu = true;
                        self.deyimler(di, ctx, govde, &g2, &yerel, d + 1);
                    }
                },
                Ifade::Dahil(y, satir) => {
                    if let Some(h) = self.deger(di, ctx, y, &yerel, 0).and_then(|y| self.dosya_bul(&y)) {
                        self.dosya_yukle(h, di, *satir, g, d + 1);
                    }
                }
                Ifade::Don(v) => self.zincir(di, ctx, v, g, &yerel, d + 1),
                Ifade::Cagri(_) => self.zincir(di, ctx, x, g, &yerel, d + 1),
                _ => {}
            }
        }
    }

    fn dosya_yukle(&mut self, hedef: usize, yukleyen: usize, satir: usize, g: &Grup, d: usize) {
        if self.yigin.contains(&hedef) || d > 24 {
            return;
        }
        let kosul = (!g.kosullar.is_empty()).then(|| g.kosullar.join(" && "));
        if !self.yuklemeler.iter().any(|y| y.0 == hedef && y.1 == yukleyen && y.2 == satir) {
            self.yuklemeler.push((hedef, yukleyen, satir, kosul));
        }
        if self.b.goreli(hedef).ends_with("api.php") && !g.onek.is_empty() {
            self.api_oneki.get_or_insert(format!("/{}", g.onek.join("/")));
        }
        let mut g2 = g.clone();
        g2.dosya = Some(hedef);
        self.yigin.push(hedef);
        let dosyalar: &'a [Dosya] = self.b.dosyalar;
        let ifadeler = &dosyalar[hedef].ifadeler;
        self.deyimler(hedef, None, ifadeler, &g2, &Degerler::new(), d + 1);
        self.yigin.pop();
    }

    /// Projedeki bir metodun gövdesini argümanları parametrelere bağlayarak değerlendirir (yükleyici yardımcıları:
    /// `RouteServiceProvider::loadVersionAwareRoutes('web')`, `$this->mapApiRoutes()`)
    #[allow(clippy::too_many_arguments)]
    fn metot_degerlendir(&mut self, di: usize, ctx: Option<Yer>, y: Yer, ad: &str, c: Option<&'a Cagri>, g: &Grup, vars: &Degerler<'a>, d: usize) {
        let Some((u, m)) = self.b.metot(y, ad) else { return };
        if d > 16 || !self.gezilen.insert((u, format!("{ad}:{}", c.map(|c| c.satir).unwrap_or(0)))) {
            return;
        }
        let mut bagli = Degerler::new();
        for (i, p) in m.parametreler.iter().enumerate() {
            let a = c.and_then(|c| arg(c, &p.ad, i)).or(p.varsayilan.as_ref());
            if let Some(a) = a {
                let v = self.deger(di, ctx, a, vars, 0).map(Deger::Metin).unwrap_or(Deger::Ifade(a));
                bagli.insert(p.ad.clone(), v);
            }
        }
        self.deyimler(u.0, Some(y), &m.govde, g, &bagli, d + 1);
    }

    /// Bir zincir: rota kaydı, grup, yükleyici (`withRouting`, `$this->routes`, `loadRoutesFrom`) ya da projedeki yardımcı
    fn zincir(&mut self, di: usize, ctx: Option<Yer>, x: &'a Ifade, g: &Grup, vars: &Degerler<'a>, d: usize) {
        let (kok, h) = halkalar(x);
        let Some(ilk) = h.first() else { return };
        let route_mu = match kok {
            Some(Ifade::Ad(a)) if ilk.statik => kisa_ad(a) == "Route" && !self.b.tam_ad(di, a).starts_with("App\\"),
            Some(Ifade::Degisken(v)) => v == "router" || v == "route",
            _ => false,
        };
        if route_mu {
            self.rota_zinciri(di, ctx, &h, g, vars, d);
            return;
        }
        // bootstrap/app.php: Application::configure(...)->withRouting(...)
        if let Some(c) = h.iter().find(|c| c.ad == "withRouting") {
            self.with_routing(di, ctx, c, g, vars, d);
            return;
        }
        match kok {
            Some(Ifade::Degisken(t)) if t == "this" => {
                match ilk.ad.as_str() {
                    // RouteServiceProvider: $this->routes(function () { ... })
                    "routes" => {
                        if let Some(Ifade::Kapanis(k)) = arg(ilk, "routesCallback", 0) {
                            self.yukleyici.get_or_insert(di);
                            self.deyimler(di, ctx, &k.govde, g, vars, d + 1);
                        }
                    }
                    "loadRoutesFrom" => {
                        if let Some(h2) = arg(ilk, "path", 0).and_then(|p| self.deger(di, ctx, p, vars, 0)).and_then(|y| self.dosya_bul(&y)) {
                            self.dosya_yukle(h2, di, ilk.satir, g, d + 1);
                        }
                    }
                    ad => {
                        if let Some(y) = ctx {
                            self.metot_degerlendir(di, ctx, y, ad, Some(ilk), g, vars, d + 1);
                        }
                    }
                }
            }
            Some(Ifade::Ad(s)) if ilk.statik => {
                let y = if matches!(s.as_str(), "self" | "static") { ctx } else { self.b.sinif(di, s) };
                if let Some(y) = y {
                    self.metot_degerlendir(di, ctx, y, &ilk.ad, Some(ilk), g, vars, d + 1);
                }
            }
            _ => {}
        }
    }

    /// `withRouting(web:, api:, apiPrefix:, then:, using:, health:)`
    fn with_routing(&mut self, di: usize, ctx: Option<Yer>, c: &'a Cagri, g: &Grup, vars: &Degerler<'a>, d: usize) {
        self.yukleyici.get_or_insert(di);
        let kanit = self.konum(di, c.satir);
        let yollar = |x: &Ifade| -> Vec<Ifade> {
            match x {
                Ifade::Dizi(v, _) => v.iter().map(|(_, e)| e.clone()).collect(),
                x => vec![x.clone()],
            }
        };
        if let Some(Ifade::Kapanis(k)) = c.arglar.iter().find(|a| a.ad.as_deref() == Some("using")).map(|a| &a.deger) {
            self.deyimler(di, ctx, &k.govde, g, vars, d + 1);
        }
        for (anahtar, grup) in [("web", "web"), ("api", "api")] {
            let Some(x) = c.arglar.iter().find(|a| a.ad.as_deref() == Some(anahtar)).map(|a| &a.deger) else { continue };
            let mut g2 = g.clone();
            g2.ara.push((Ara::Takma(grup.into()), kanit.clone()));
            if anahtar == "api" {
                let onek = c.arglar.iter().find(|a| a.ad.as_deref() == Some("apiPrefix")).and_then(|a| self.deger(di, ctx, &a.deger, vars, 0)).unwrap_or_else(|| "api".into());
                g2.onek.extend(parcalar(&onek));
                self.api_oneki = Some(format!("/{}", parcalar(&onek).join("/")));
            }
            for y in yollar(x) {
                if let Some(h) = self.deger(di, ctx, &y, vars, 0).and_then(|y| self.dosya_bul(&y)) {
                    self.dosya_yukle(h, di, c.satir, &g2, d + 1);
                }
            }
        }
        if let Some(Ifade::Kapanis(k)) = c.arglar.iter().find(|a| a.ad.as_deref() == Some("then")).map(|a| &a.deger) {
            self.deyimler(di, ctx, &k.govde, g, vars, d + 1);
        }
        if let Some(h) = c.arglar.iter().find(|a| a.ad.as_deref() == Some("health")).and_then(|a| self.deger(di, ctx, &a.deger, vars, 0)) {
            self.rotalar.push(Rota {
                yontemler: vec!["GET".into()],
                uri: format!("/{}", parcalar(&h).join("/")),
                eylem: Eylem::Saglik,
                grup: g.clone(),
                ad: None,
                di,
                satir: c.satir,
            });
        }
    }

    /// Grup niteliklerinin dizi biçimi: `Route::group(['prefix' => 'x', 'middleware' => [...], 'as' => 'x.', 'namespace' => 'Api'], ...)`
    fn dizi_nitelikleri(&self, di: usize, ctx: Option<Yer>, v: &'a [(Option<Ifade>, Ifade)], g: &mut Grup, kanit: &str, vars: &Degerler<'a>) {
        for (a, e) in v {
            let Some(Ifade::Metin(a)) = a else { continue };
            match a.as_str() {
                "prefix" => g.onek.extend(self.deger(di, ctx, e, vars, 0).map(|p| parcalar(&p)).unwrap_or_default()),
                "middleware" => g.ara.extend(self.ara_argumani(di, e, vars).into_iter().map(|x| (x, kanit.to_string()))),
                "excluded_middleware" | "withoutMiddleware" => g.haric.extend(self.ara_argumani(di, e, vars)),
                "as" => g.ad.push_str(&self.deger(di, ctx, e, vars, 0).unwrap_or_default()),
                "namespace" => {
                    if let Some(n) = self.deger(di, ctx, e, vars, 0) {
                        g.ad_alani = Some(ad_alani_birlestir(g.ad_alani.as_deref(), &n));
                    }
                }
                "controller" => {
                    if let Ifade::Sinif(s) = e {
                        g.kontrolcu = Some((di, s.clone()));
                    }
                }
                "where" => {
                    if let Ifade::Dizi(w, _) = e {
                        for (p, r) in w {
                            if let (Some(Ifade::Metin(p)), Some(r)) = (p, self.deger(di, ctx, r, vars, 0)) {
                                g.desenler.push((p.clone(), r));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Eylem: `[C::class, 'm']`, `'C@m'`, `'m'` (grup controller'ı), `C::class` (invokable), kapanış ya da `['uses' => ...]`
    fn eylem(&self, di: usize, x: Option<&'a Ifade>, g: &Grup, vars: &Degerler<'a>) -> Option<Eylem<'a>> {
        let x = match x? {
            Ifade::Degisken(v) => match vars.get(v)? {
                Deger::Ifade(y) => return self.eylem(di, Some(y), g, vars),
                Deger::Metin(t) => return self.metin_eylemi(di, t, g),
            },
            x => x,
        };
        match x {
            Ifade::Dizi(v, _) => {
                if let Some((_, u)) = v.iter().find(|(a, _)| matches!(a, Some(Ifade::Metin(t)) if t == "uses")) {
                    return self.eylem(di, Some(u), g, vars);
                }
                let ogeler: Vec<&Ifade> = v.iter().filter(|(a, _)| a.is_none()).map(|(_, e)| e).collect();
                match ogeler.as_slice() {
                    [Ifade::Sinif(c), Ifade::Metin(m)] => Some(Eylem::Metot { di, sinif: c.clone(), tam: false, metot: m.clone() }),
                    [Ifade::Metin(c), Ifade::Metin(m)] => Some(Eylem::Metot { di, sinif: c.clone(), tam: true, metot: m.clone() }),
                    [Ifade::Sinif(c)] => Some(Eylem::Metot { di, sinif: c.clone(), tam: false, metot: "__invoke".into() }),
                    _ => v.iter().find(|(a, e)| a.is_none() && matches!(e, Ifade::Kapanis(_))).and_then(|(_, e)| self.eylem(di, Some(e), g, vars)),
                }
            }
            Ifade::Metin(t) => self.metin_eylemi(di, t, g),
            Ifade::Sinif(c) => Some(Eylem::Metot { di, sinif: c.clone(), tam: false, metot: "__invoke".into() }),
            Ifade::Kapanis(k) => Some(Eylem::Kapanis(di, k.as_ref())),
            _ => None,
        }
    }

    /// Metin eylemi: `'C@m'`, grup controller'ında `'m'`, ad alanıyla invokable `'App\X'`
    fn metin_eylemi(&self, di: usize, t: &str, g: &Grup) -> Option<Eylem<'a>> {
        match t.split_once('@') {
            Some((c, m)) => {
                let sinif = match (&g.ad_alani, c.starts_with('\\')) {
                    (Some(n), false) => format!("{n}\\{c}"),
                    _ => c.trim_start_matches('\\').to_string(),
                };
                Some(Eylem::Metot { di, sinif, tam: true, metot: m.to_string() })
            }
            None => match &g.kontrolcu {
                Some((kdi, c)) => Some(Eylem::Metot { di: *kdi, sinif: c.clone(), tam: false, metot: t.to_string() }),
                // Ad alanıyla yazılmış invokable controller: 'App\Http\Controllers\X'
                None if t.contains('\\') || g.ad_alani.is_some() => {
                    let sinif = match &g.ad_alani {
                        Some(n) if !t.starts_with('\\') && !t.contains('\\') => format!("{n}\\{t}"),
                        _ => t.trim_start_matches('\\').to_string(),
                    };
                    Some(Eylem::Metot { di, sinif, tam: true, metot: "__invoke".into() })
                }
                None => None,
            },
        }
    }

    /// `Route::...` zinciri: kayıttan önceki halkalar grup niteliği, sonrakiler kaydın niteliğidir
    fn rota_zinciri(&mut self, di: usize, ctx: Option<Yer>, h: &[&'a Cagri], g: &Grup, vars: &Degerler<'a>, d: usize) {
        let mut g2 = g.clone();
        let mut kayit: Option<Kayit> = None;
        for c in h {
            let kanit = self.konum(di, c.satir);
            let ad = c.ad.as_str();
            let ilk_metin = || arg(c, "", 0).and_then(|x| self.deger(di, ctx, x, vars, 0));
            if let Some(k) = kayit.as_mut() {
                match ad {
                    "name" | "as" => k.ad = ilk_metin().map(|n| format!("{}{n}", k.grup.ad)),
                    "names" => {}
                    "middleware" => k.grup.ara.extend(c.arglar.iter().flat_map(|a| self.ara_argumani(di, &a.deger, vars)).map(|x| (x, kanit.clone()))),
                    "withoutMiddleware" => k.grup.haric.extend(c.arglar.iter().flat_map(|a| self.ara_argumani(di, &a.deger, vars))),
                    "can" => {
                        let a = arg_metinleri(c);
                        if !a.is_empty() {
                            k.grup.ara.push((Ara::Takma(format!("can:{}", a.join(","))), kanit.clone()));
                        }
                    }
                    "where" => desen_ekle(c, &mut k.grup.desenler, |x| self.deger(di, ctx, x, vars, 0)),
                    w if w.starts_with("where") => {
                        let a = arg_metinleri(c);
                        let desen = match w {
                            "whereNumber" => "[0-9]+",
                            "whereUuid" => "[\\da-fA-F]{8}-[\\da-fA-F]{4}-[\\da-fA-F]{4}-[\\da-fA-F]{4}-[\\da-fA-F]{12}",
                            "whereUlid" => "[0-7][0-9a-hjkmnp-tv-zA-HJKMNP-TV-Z]{25}",
                            "whereAlpha" => "[a-zA-Z]+",
                            "whereAlphaNumeric" => "[a-zA-Z0-9]+",
                            _ => "",
                        };
                        for p in a.iter().take(if w == "whereIn" { 1 } else { usize::MAX }) {
                            k.grup.desenler.push((p.clone(), desen.to_string()));
                        }
                    }
                    "only" => k.sadece = Some(arg_metinleri(c)),
                    "except" => k.haric_metotlar.extend(arg_metinleri(c)),
                    "parameters" => {
                        if let Some(Ifade::Dizi(v, _)) = arg(c, "parameters", 0) {
                            for (a, e) in v {
                                if let (Some(Ifade::Metin(a)), Ifade::Metin(e)) = (a, e) {
                                    k.parametre_adlari.insert(a.clone(), e.clone());
                                }
                            }
                        }
                    }
                    "shallow" => k.sig = true,
                    "creatable" => k.olusturulabilir = true,
                    "destroyable" => k.silinebilir = true,
                    _ => {}
                }
                continue;
            }
            match ad {
                "prefix" => g2.onek.extend(ilk_metin().map(|p| parcalar(&p)).unwrap_or_default()),
                "name" | "as" => g2.ad.push_str(&ilk_metin().unwrap_or_default()),
                "middleware" => g2.ara.extend(c.arglar.iter().flat_map(|a| self.ara_argumani(di, &a.deger, vars)).map(|x| (x, kanit.clone()))),
                "withoutMiddleware" => g2.haric.extend(c.arglar.iter().flat_map(|a| self.ara_argumani(di, &a.deger, vars))),
                "namespace" => {
                    if let Some(n) = ilk_metin() {
                        g2.ad_alani = Some(ad_alani_birlestir(g2.ad_alani.as_deref(), &n));
                    }
                }
                "controller" => {
                    if let Some(Ifade::Sinif(s)) = arg(c, "controller", 0) {
                        g2.kontrolcu = Some((di, s.clone()));
                    }
                }
                "where" => desen_ekle(c, &mut g2.desenler, |x| self.deger(di, ctx, x, vars, 0)),
                "pattern" => {
                    if let (Some(p), Some(r)) = (arg(c, "key", 0).and_then(|x| self.deger(di, ctx, x, vars, 0)), arg(c, "pattern", 1).and_then(|x| self.deger(di, ctx, x, vars, 0)))
                    {
                        self.desenler.push((p, r));
                    }
                }
                "patterns" => {
                    let mut yeni = vec![];
                    desen_ekle(c, &mut yeni, |x| self.deger(di, ctx, x, vars, 0));
                    self.desenler.extend(yeni);
                }
                "group" => {
                    let mut g3 = g2.clone();
                    let mut govde = c.arglar.last().map(|a| &a.deger);
                    if let (Some(Ifade::Dizi(v, _)), true) = (c.arglar.first().map(|a| &a.deger), c.arglar.len() > 1) {
                        self.dizi_nitelikleri(di, ctx, v, &mut g3, &kanit, vars);
                    } else if c.arglar.len() == 1 && matches!(c.arglar[0].deger, Ifade::Dizi(..)) {
                        govde = None;
                    }
                    match govde {
                        Some(Ifade::Kapanis(k)) => self.deyimler(di, ctx, &k.govde, &g3, vars, d + 1),
                        Some(Ifade::Metin(_) | Ifade::Birlesim(_) | Ifade::Cagri(_) | Ifade::Degisken(_)) => {
                            if let Some(h2) = govde.and_then(|x| self.deger(di, ctx, x, vars, 0)).and_then(|y| self.dosya_bul(&y)) {
                                self.yukleyici.get_or_insert(di);
                                self.dosya_yukle(h2, di, c.satir, &g3, d + 1);
                            }
                        }
                        _ => {}
                    }
                    return;
                }
                f if KAYIT_FIILLERI.contains(&f) => {
                    kayit = self.kayit_baslat(di, ctx, c, &g2, vars);
                    if kayit.is_none() {
                        return;
                    }
                }
                _ => {}
            }
        }
        if let Some(k) = kayit {
            self.kayit_bitir(k);
        }
    }

    fn kayit_baslat(&mut self, di: usize, ctx: Option<Yer>, c: &'a Cagri, g: &Grup, vars: &Degerler<'a>) -> Option<Kayit<'a>> {
        let deger = |x: Option<&Ifade>| x.and_then(|x| self.deger(di, ctx, x, vars, 0));
        let yeni = |tur: KayitTur<'a>| Kayit {
            tur,
            grup: g.clone(),
            ad: None,
            sadece: None,
            haric_metotlar: vec![],
            parametre_adlari: HashMap::new(),
            sig: false,
            olusturulabilir: false,
            silinebilir: false,
            di,
            satir: c.satir,
        };
        let tur = match c.ad.as_str() {
            y @ ("get" | "post" | "put" | "patch" | "delete" | "options" | "any") => {
                let Some(uri) = deger(arg(c, "uri", 0)) else {
                    self.dinamik.push(self.konum(di, c.satir));
                    return None;
                };
                let yontemler = if y == "any" { HER_YONTEM.iter().map(|x| x.to_string()).collect() } else { vec![y.to_uppercase()] };
                let x = arg(c, "action", 1);
                let mut k = yeni(KayitTur::Tekil { yontemler, uri, eylem: Eylem::Gorunum });
                // ['as' => 'ad', 'middleware' => ..., 'uses' => ...] biçimi
                if let Some(Ifade::Dizi(v, _)) = x {
                    let kanit = self.konum(di, c.satir);
                    for (a, e) in v {
                        match a {
                            Some(Ifade::Metin(a)) if a == "as" => k.ad = deger(Some(e)).map(|n| format!("{}{n}", g.ad)),
                            Some(Ifade::Metin(a)) if a == "middleware" => k.grup.ara.extend(self.b.ara_girisleri(di, e).into_iter().map(|x| (x, kanit.clone()))),
                            _ => {}
                        }
                    }
                }
                let Some(eylem) = self.eylem(di, x, g, vars) else {
                    self.dinamik.push(self.konum(di, c.satir));
                    return None;
                };
                if let KayitTur::Tekil { eylem: e, .. } = &mut k.tur {
                    *e = eylem;
                }
                return Some(k);
            }
            "match" => {
                let yontemler: Vec<String> = arg(c, "methods", 0)
                    .map(metinler)
                    .unwrap_or_default()
                    .iter()
                    .flat_map(|m| m.split('|').map(|x| x.trim().to_uppercase()).collect::<Vec<_>>())
                    .filter(|m| m != "HEAD")
                    .collect();
                let uri = deger(arg(c, "uri", 1));
                match (uri, self.eylem(di, arg(c, "action", 2), g, vars)) {
                    (Some(uri), Some(eylem)) if !yontemler.is_empty() => KayitTur::Tekil { yontemler, uri, eylem },
                    _ => {
                        self.dinamik.push(self.konum(di, c.satir));
                        return None;
                    }
                }
            }
            "view" => KayitTur::Tekil { yontemler: vec!["GET".into()], uri: deger(arg(c, "uri", 0))?, eylem: Eylem::Gorunum },
            f @ ("resource" | "apiResource" | "singleton" | "apiSingleton") => {
                let (Some(ad), Some(kc)) = (deger(arg(c, "name", 0)), arg(c, "controller", 1).and_then(|x| self.coz_ifade(x, vars, 0)).and_then(|x| kaynak_kontrolcusu(di, x, g)))
                else {
                    self.dinamik.push(self.konum(di, c.satir));
                    return None;
                };
                let mut k = yeni(KayitTur::Kaynak(vec![(ad, kc)], f.starts_with("api"), f.ends_with("ingleton")));
                if let Some(Ifade::Dizi(v, _)) = arg(c, "options", 2) {
                    for (a, e) in v {
                        match a {
                            Some(Ifade::Metin(a)) if a == "only" => k.sadece = Some(metinler(e)),
                            Some(Ifade::Metin(a)) if a == "except" => k.haric_metotlar.extend(metinler(e)),
                            Some(Ifade::Metin(a)) if a == "parameters" => {
                                if let Ifade::Dizi(p, _) = e {
                                    for (x, y) in p {
                                        if let (Some(Ifade::Metin(x)), Ifade::Metin(y)) = (x, y) {
                                            k.parametre_adlari.insert(x.clone(), y.clone());
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                return Some(k);
            }
            f @ ("resources" | "apiResources") => {
                let Some(Ifade::Dizi(v, _)) = arg(c, "resources", 0) else { return None };
                let liste = v.iter().filter_map(|(a, e)| Some((a.as_ref().and_then(|a| deger(Some(a)))?, kaynak_kontrolcusu(di, e, g)?))).collect();
                KayitTur::Kaynak(liste, f == "apiResources", false)
            }
            _ => return None,
        };
        Some(yeni(tur))
    }

    fn kayit_bitir(&mut self, k: Kayit<'a>) {
        let tam_yol = |g: &Grup, uri: &str| -> String {
            let mut p = g.onek.clone();
            p.extend(parcalar(uri));
            format!("/{}", p.join("/"))
        };
        match &k.tur {
            KayitTur::Tekil { yontemler, uri, eylem } => {
                self.rotalar.push(Rota {
                    yontemler: yontemler.clone(),
                    uri: tam_yol(&k.grup, uri),
                    eylem: eylem.clone(),
                    grup: k.grup.clone(),
                    ad: k.ad.clone(),
                    di: k.di,
                    satir: k.satir,
                });
            }
            KayitTur::Kaynak(liste, api, singleton) => {
                for (ad, (cdi, c, tam)) in liste {
                    for (yontemler, uri, metot, rota_adi) in kaynak_rotalari(ad, *api, *singleton, &k) {
                        self.rotalar.push(Rota {
                            yontemler,
                            uri: tam_yol(&k.grup, &uri),
                            eylem: Eylem::Metot { di: *cdi, sinif: c.clone(), tam: *tam, metot },
                            grup: k.grup.clone(),
                            ad: Some(format!("{}{rota_adi}", k.grup.ad)),
                            di: k.di,
                            satir: k.satir,
                        });
                    }
                }
            }
        }
    }
}

/// Kaynak controller'ı: `X::class` ya da grup ad alanıyla metin (`'CourseController'`)
fn kaynak_kontrolcusu(di: usize, x: &Ifade, g: &Grup) -> Option<(usize, String, bool)> {
    match x {
        Ifade::Sinif(s) => Some((di, s.clone(), false)),
        Ifade::Metin(t) => Some((
            di,
            match &g.ad_alani {
                Some(n) if !t.starts_with('\\') && !t.contains('\\') => format!("{n}\\{t}"),
                _ => t.trim_start_matches('\\').to_string(),
            },
            true,
        )),
        _ => None,
    }
}

/// `where('id', '[0-9]+')` ya da `where(['id' => '[0-9]+'])`
fn desen_ekle(c: &Cagri, desenler: &mut Vec<(String, String)>, deger: impl Fn(&Ifade) -> Option<String>) {
    match c.arglar.first().map(|a| &a.deger) {
        Some(Ifade::Dizi(v, _)) => {
            for (p, r) in v {
                if let Some(p) = p.as_ref().and_then(&deger) {
                    desenler.push((p, deger(r).unwrap_or_default()));
                }
            }
        }
        Some(p) => {
            if let Some(p) = deger(p) {
                desenler.push((p, c.arglar.get(1).and_then(|a| deger(&a.deger)).unwrap_or_default()));
            }
        }
        None => {}
    }
}

/// Grup ad alanı: içteki `\` ile başlamıyorsa dıştakine eklenir
fn ad_alani_birlestir(dis: Option<&str>, ic: &str) -> String {
    match dis {
        Some(d) if !ic.starts_with('\\') && !d.is_empty() => format!("{}\\{}", d.trim_end_matches('\\'), ic.trim_matches('\\')),
        _ => ic.trim_matches('\\').to_string(),
    }
}

/// Kaynak rotaları (Laravel ResourceRegistrar): (yöntemler, kaynak yolu, controller metodu, rota adı soneki).
/// `photos.comments` iç içe (`photos/{photo}/comments`), `admin/photos` önekli; parametre adı tekil (`-` -> `_`)
fn kaynak_rotalari(ad: &str, api: bool, singleton: bool, k: &Kayit<'_>) -> Vec<(Vec<String>, String, String, String)> {
    let (onek, ad) = match ad.rsplit_once('/') {
        Some((o, a)) => (format!("{o}/"), a),
        None => (String::new(), ad),
    };
    let segmentler: Vec<&str> = ad.split('.').filter(|s| !s.is_empty()).collect();
    let Some(son) = segmentler.last() else { return vec![] };
    let param = |s: &str| k.parametre_adlari.get(s).cloned().unwrap_or_else(|| tekil(s).replace('-', "_"));
    let mut ebeveyn = String::new();
    for s in &segmentler[..segmentler.len() - 1] {
        ebeveyn.push_str(&format!("{s}/{{{}}}/", param(s)));
    }
    let taban = format!("{onek}{ebeveyn}{son}");
    let sig_taban = format!("{onek}{son}");
    let p = param(son);
    let ad_oneki = segmentler.join(".");
    let y = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut v: Vec<(Vec<String>, String, String)> = vec![];
    if singleton {
        v.push((y(&["GET"]), taban.clone(), "show".into()));
        if !api {
            v.push((y(&["GET"]), format!("{taban}/edit"), "edit".into()));
        }
        v.push((y(&["PUT", "PATCH"]), taban.clone(), "update".into()));
        if k.olusturulabilir {
            if !api {
                v.push((y(&["GET"]), format!("{taban}/create"), "create".into()));
            }
            v.push((y(&["POST"]), taban.clone(), "store".into()));
        }
        if k.olusturulabilir || k.silinebilir {
            v.push((y(&["DELETE"]), taban.clone(), "destroy".into()));
        }
    } else {
        let tekil_yol = if k.sig && !ebeveyn.is_empty() { &sig_taban } else { &taban };
        v.push((y(&["GET"]), taban.clone(), "index".into()));
        if !api {
            v.push((y(&["GET"]), format!("{taban}/create"), "create".into()));
        }
        v.push((y(&["POST"]), taban.clone(), "store".into()));
        v.push((y(&["GET"]), format!("{tekil_yol}/{{{p}}}"), "show".into()));
        if !api {
            v.push((y(&["GET"]), format!("{tekil_yol}/{{{p}}}/edit"), "edit".into()));
        }
        v.push((y(&["PUT", "PATCH"]), format!("{tekil_yol}/{{{p}}}"), "update".into()));
        v.push((y(&["DELETE"]), format!("{tekil_yol}/{{{p}}}"), "destroy".into()));
    }
    v.into_iter()
        .filter(|(_, _, m)| k.sadece.as_ref().is_none_or(|s| s.contains(m)) && !k.haric_metotlar.contains(m))
        .map(|(yo, u, m)| {
            let a = format!("{ad_oneki}.{m}");
            (yo, u, m, a)
        })
        .collect()
}

// ---- Gövdeler: değişken tipleri, veri etkisi, olaylar, çağrı zinciri ----

/// Davranış zincirindeki bir gövde: sınıfın metodu ya da rota kapanışı
#[derive(Clone, Copy)]
enum Govde<'a> {
    Metot(Yer, &'a Metot),
    Kapanis(usize, &'a Kapanis),
}

impl<'a> Govde<'a> {
    fn ifadeler(&self) -> &'a [Ifade] {
        match self {
            Govde::Metot(_, m) => &m.govde,
            Govde::Kapanis(_, k) => &k.govde,
        }
    }
    fn dna(&self) -> &'a GovdeDna {
        match self {
            Govde::Metot(_, m) => &m.dna,
            Govde::Kapanis(_, k) => &k.dna,
        }
    }
    fn di(&self) -> usize {
        match self {
            Govde::Metot(y, _) => y.0,
            Govde::Kapanis(di, _) => *di,
        }
    }
    fn ctx(&self) -> Option<Yer> {
        match self {
            Govde::Metot(y, _) => Some(*y),
            Govde::Kapanis(..) => None,
        }
    }
    fn parametreler(&self) -> &'a [Parametre] {
        match self {
            Govde::Metot(_, m) => &m.parametreler,
            Govde::Kapanis(_, k) => &k.parametreler,
        }
    }
    fn satir(&self) -> usize {
        match self {
            Govde::Metot(_, m) => m.satir,
            Govde::Kapanis(_, k) => k.satir,
        }
    }
    fn kimlik(&self) -> (usize, usize) {
        (self.di(), self.satir())
    }
}

/// Bir gövdenin veri etkisi, tetikledikleri ve çağırdığı proje gövdeleri
#[derive(Default)]
struct Etki<'a> {
    okur: BTreeSet<String>,
    yazar: BTreeSet<String>,
    /// (model, satır)
    silinen: Vec<(String, usize)>,
    /// Yumuşak silinen modelin silme satırı
    yumusak: Option<usize>,
    eksik: bool,
    /// (tür, ad, satır)
    tetikler: Vec<(String, String, usize)>,
    cagrilan: Vec<Govde<'a>>,
}

const YAZMA: [&str; 33] = [
    "create",
    "forceCreate",
    "createMany",
    "createQuietly",
    "insert",
    "insertGetId",
    "insertOrIgnore",
    "upsert",
    "update",
    "updateOrCreate",
    "updateOrInsert",
    "updateQuietly",
    "firstOrCreate",
    "delete",
    "deleteQuietly",
    "destroy",
    "forceDelete",
    "save",
    "saveMany",
    "saveQuietly",
    "push",
    "increment",
    "decrement",
    "incrementEach",
    "restore",
    "truncate",
    "attach",
    "detach",
    "sync",
    "syncWithoutDetaching",
    "toggle",
    "updateExistingPivot",
    "touch",
];
/// Model ya da sorgu kurucusu döndürmeyen çağrılar (sayı, değer listesi, etkilenen satır sayısı)
const SKALER: [&str; 20] = [
    "count",
    "exists",
    "doesntExist",
    "sum",
    "avg",
    "average",
    "max",
    "min",
    "pluck",
    "value",
    "insert",
    "insertGetId",
    "insertOrIgnore",
    "upsert",
    "destroy",
    "truncate",
    "delete",
    "forceDelete",
    "increment",
    "decrement",
];
const SILME: [&str; 5] = ["delete", "deleteQuietly", "destroy", "forceDelete", "truncate"];
const PIVOT: [&str; 6] = ["attach", "detach", "sync", "syncWithoutDetaching", "toggle", "updateExistingPivot"];
const OKUMA: [&str; 30] = [
    "find",
    "findOrFail",
    "findMany",
    "findOrNew",
    "first",
    "firstOrFail",
    "firstOrNew",
    "firstWhere",
    "sole",
    "get",
    "all",
    "paginate",
    "simplePaginate",
    "cursorPaginate",
    "cursor",
    "lazy",
    "chunk",
    "each",
    "count",
    "exists",
    "doesntExist",
    "sum",
    "avg",
    "max",
    "min",
    "pluck",
    "value",
    "load",
    "loadMissing",
    "fresh",
];

impl<'a> Baglam<'a> {
    fn kullanici(&self) -> Option<Yer> {
        self.kullanici_modeli.as_ref().and_then(|m| self.modeller.get(m)).map(|m| m.yer)
    }

    /// Gövdedeki değişkenlerin sınıfları: parametre tipleri ve atamalar (`$post = Post::findOrFail($id)`)
    fn degiskenler(&self, g: Govde<'a>) -> HashMap<String, Yer> {
        let di = g.di();
        let mut v: HashMap<String, Yer> = HashMap::new();
        for p in g.parametreler() {
            if let Some(y) = p.tip.as_deref().and_then(|t| self.sinif(di, t)) {
                v.insert(p.ad.clone(), y);
            }
        }
        let mut atamalar: Vec<(&String, &Ifade)> = vec![];
        for x in g.ifadeler() {
            ifade_gez(
                x,
                &mut |y| {
                    if let Ifade::Ata(sol, sag) = y {
                        if let Ifade::Degisken(a) = sol.as_ref() {
                            atamalar.push((a, sag.as_ref()));
                        }
                    }
                },
                0,
            );
        }
        for _ in 0..2 {
            for (a, sag) in &atamalar {
                if let Some(y) = self.ifade_sinifi(di, g.ctx(), &v, sag, 0) {
                    v.insert((*a).clone(), y);
                }
            }
        }
        v
    }

    /// İfadenin sınıfı (model ya da servis)
    fn ifade_sinifi(&self, di: usize, ctx: Option<Yer>, vars: &HashMap<String, Yer>, x: &Ifade, d: usize) -> Option<Yer> {
        if d > 16 {
            return None;
        }
        match x {
            Ifade::Degisken(t) if t == "this" => ctx,
            Ifade::Degisken(v) => vars.get(v).copied(),
            Ifade::Yeni(s, _) => self.sinif(di, s),
            Ifade::Ozellik(o, p) => {
                if matches!(o.as_ref(), Ifade::Degisken(t) if t == "this") {
                    if let Some(y) = ctx.and_then(|c| self.ozellik_tipi(c, p)) {
                        return Some(y);
                    }
                }
                let y = self.ifade_sinifi(di, ctx, vars, o, d + 1)?;
                let m = self.modeller.get(&self.model_adi(y)?)?;
                m.iliskiler.get(p).and_then(|(_, h, _)| self.modeller.get(h)).map(|m| m.yer)
            }
            Ifade::Cagri(c) => {
                match (&c.alici, c.statik) {
                    (None, _) => match c.ad.as_str() {
                        "app" | "resolve" => match c.arglar.first().map(|a| &a.deger) {
                            Some(Ifade::Sinif(s)) => self.sinif(di, s),
                            _ => None,
                        },
                        _ => None,
                    },
                    (Some(a), true) => {
                        let s = match a {
                            Ifade::Ad(s) => s,
                            _ => return None,
                        };
                        let y = if matches!(s.as_str(), "self" | "static") { ctx? } else { self.sinif(di, s)? };
                        if self.model_adi(y).is_some() {
                            return (!SKALER.contains(&c.ad.as_str())).then_some(y);
                        }
                        // Auth::user()
                        if kisa_ad(s) == "Auth" && c.ad == "user" {
                            return self.kullanici();
                        }
                        self.donus_sinifi(y, &c.ad)
                    }
                    (Some(a), false) => {
                        // auth()->user(), request()->user(), $request->user(), $this->user() (Form Request)
                        if c.ad == "user" {
                            return self.kullanici();
                        }
                        let y = self.ifade_sinifi(di, ctx, vars, a, d + 1)?;
                        if let Some(m) = self.model_adi(y).and_then(|m| self.modeller.get(&m)) {
                            if let Some((_, h, _)) = m.iliskiler.get(&c.ad) {
                                return self.modeller.get(h).map(|m| m.yer);
                            }
                            return (!SKALER.contains(&c.ad.as_str())).then_some(y);
                        }
                        self.donus_sinifi(y, &c.ad)
                    }
                }
            }
            _ => None,
        }
    }

    /// Projedeki metodun dönüş tipindeki sınıf
    fn donus_sinifi(&self, y: Yer, metot: &str) -> Option<Yer> {
        let (u, m) = self.metot(y, metot)?;
        let t = m.donus_tipi.as_deref()?;
        if matches!(t, "self" | "static") {
            return Some(u);
        }
        self.sinif(u.0, t)
    }

    /// Çağrının dokunduğu model ya da tablo (ilişki yazmasında pivot tablo): `Post::where(...)`, `$post->comments()`,
    /// `DB::table('posts')`
    fn zincir_modeli(&self, di: usize, ctx: Option<Yer>, vars: &HashMap<String, Yer>, x: &Ifade, d: usize) -> Option<(String, Option<String>)> {
        if d > 24 {
            return None;
        }
        match x {
            Ifade::Cagri(c) if c.statik => {
                let Some(Ifade::Ad(s)) = &c.alici else { return None };
                if kisa_ad(s) == "DB" && c.ad == "table" {
                    let t = metinler(c.arglar.first().map(|a| &a.deger)?).into_iter().next()?;
                    let t = t.split_whitespace().next().unwrap_or(&t).to_string();
                    return Some((self.tablolar.get(&t.to_lowercase()).cloned().unwrap_or(t), None));
                }
                let y = if matches!(s.as_str(), "self" | "static") { ctx? } else { self.sinif(di, s)? };
                self.model_adi(y).map(|m| (m, None))
            }
            Ifade::Cagri(c) => {
                let a = c.alici.as_ref()?;
                let ic = self.zincir_modeli(di, ctx, vars, a, d + 1).or_else(|| {
                    if let Ifade::Cagri(_) = a {
                        // auth()->user() gibi: ifadenin sınıfı
                        return self.ifade_sinifi(di, ctx, vars, a, d + 1).and_then(|y| self.model_adi(y)).map(|m| (m, None));
                    }
                    self.ifade_sinifi(di, ctx, vars, a, d + 1).and_then(|y| self.model_adi(y)).map(|m| (m, None))
                })?;
                match self.modeller.get(&ic.0).and_then(|m| m.iliskiler.get(&c.ad)) {
                    Some((_, h, p)) => Some((h.clone(), p.clone())),
                    None => Some(ic),
                }
            }
            Ifade::Degisken(_) | Ifade::Ozellik(..) => self.ifade_sinifi(di, ctx, vars, x, d + 1).and_then(|y| self.model_adi(y)).map(|m| (m, None)),
            _ => None,
        }
    }

    /// Olay mı iş mi: `Illuminate\Foundation\Events\Dispatchable` olay, `Illuminate\Foundation\Bus\Dispatchable` /
    /// `ShouldQueue` iş; trait çözülemezse ad alanı (`App\Events`, `App\Jobs`)
    fn olay_turu(&self, y: Yer) -> &'static str {
        if let Some(t) = self.trait_tam(y, "Dispatchable") {
            if t.contains("Events") {
                return "event";
            }
            if t.contains("Bus") {
                return "job";
            }
        }
        let tam = self.tam(y);
        if tam.contains("\\Events\\") {
            "event"
        } else if tam.contains("\\Jobs\\") || self.uygular(y, "ShouldQueue") || self.metot(y, "handle").is_some() {
            "job"
        } else {
            "event"
        }
    }

    /// Bir gövdenin veri etkisi, tetiklediği olay / işler ve çağırdığı proje gövdeleri
    fn etki(&self, g: Govde<'a>) -> Etki<'a> {
        let di = g.di();
        let ctx = g.ctx();
        let vars = self.degiskenler(g);
        let mut e = Etki::default();
        let mut cagrilar: Vec<&Cagri> = vec![];
        let mut ozellikler: Vec<(&Ifade, &String)> = vec![];
        for x in g.ifadeler() {
            ifade_gez(
                x,
                &mut |y| match y {
                    Ifade::Cagri(c) => cagrilar.push(c),
                    Ifade::Ozellik(o, p) => ozellikler.push((o, p)),
                    _ => {}
                },
                0,
            );
        }
        // İlişki özelliği okuması: $post->comments
        for (o, p) in ozellikler {
            if let Some(m) = self.ifade_sinifi(di, ctx, &vars, o, 0).and_then(|y| self.model_adi(y)).and_then(|m| self.modeller.get(&m)) {
                if let Some((_, h, _)) = m.iliskiler.get(p) {
                    e.okur.insert(h.clone());
                }
            }
        }
        let olay_adi = |x: Option<&Ifade>| -> Option<(String, Option<Yer>)> {
            match x? {
                Ifade::Yeni(s, _) => Some((kisa_ad(s).to_string(), self.sinif(di, s))),
                _ => None,
            }
        };
        let tetik = |e: &mut Etki, tur: &str, ad: String, satir: usize| {
            if !e.tetikler.iter().any(|(t, a, _)| t == tur && *a == ad) && e.tetikler.len() < 32 {
                e.tetikler.push((tur.to_string(), ad, satir));
            }
        };
        for c in cagrilar {
            let ad = c.ad.as_str();
            // Olaylar ve işler
            match (&c.alici, c.statik) {
                (None, _) => match ad {
                    "event" | "broadcast" => {
                        if let Some((a, _)) = olay_adi(c.arglar.first().map(|a| &a.deger)) {
                            tetik(&mut e, "event", a, c.satir);
                        }
                    }
                    "dispatch" | "dispatch_sync" | "dispatch_now" => {
                        if let Some((a, y)) = olay_adi(c.arglar.first().map(|a| &a.deger)) {
                            tetik(&mut e, y.map(|y| self.olay_turu(y)).unwrap_or("job"), a, c.satir);
                        }
                    }
                    _ => {}
                },
                (Some(Ifade::Ad(s)), true) => {
                    let k = kisa_ad(s);
                    if matches!(ad, "dispatch" | "dispatchSync" | "dispatchNow" | "dispatchIf" | "dispatchUnless" | "dispatchAfterResponse" | "push" | "broadcast")
                        && matches!(k, "Event" | "Bus" | "Queue")
                    {
                        if let Some((a, y)) = olay_adi(c.arglar.first().map(|a| &a.deger)) {
                            let tur = if k == "Event" { "event" } else { y.map(|y| self.olay_turu(y)).unwrap_or("job") };
                            tetik(&mut e, tur, a, c.satir);
                        }
                    } else if matches!(ad, "dispatch" | "dispatchSync" | "dispatchNow" | "dispatchIf" | "dispatchUnless" | "dispatchAfterResponse" | "broadcast") {
                        if let Some(y) = self.sinif(di, s).filter(|y| self.model_adi(*y).is_none()) {
                            tetik(&mut e, self.olay_turu(y), kisa_ad(s).to_string(), c.satir);
                        }
                    }
                    if k == "DB" && matches!(ad, "select" | "insert" | "update" | "delete" | "statement" | "unprepared" | "affectingStatement") {
                        e.eksik = true;
                    }
                }
                _ => {}
            }
            // Veri: modelin (ya da tablonun) okunması / yazılması
            let model = if c.statik {
                self.zincir_modeli(di, ctx, &vars, &Ifade::Cagri(Box::new(c.clone())), 0)
            } else {
                c.alici.as_ref().and_then(|a| self.zincir_modeli(di, ctx, &vars, a, 0))
            };
            if let Some((m, pivot)) = model {
                let iliski_cagrisi = !c.statik && self.modeller.get(&m).is_some_and(|x| x.iliskiler.contains_key(ad));
                if YAZMA.contains(&ad) {
                    match (&pivot, PIVOT.contains(&ad)) {
                        (Some(p), true) => {
                            e.yazar.insert(p.clone());
                        }
                        _ => {
                            e.yazar.insert(m.clone());
                        }
                    }
                    if matches!(ad, "firstOrCreate" | "updateOrCreate") {
                        e.okur.insert(m.clone());
                    }
                    if SILME.contains(&ad) {
                        e.silinen.push((m.clone(), c.satir));
                        if ad != "forceDelete" && ad != "truncate" && self.modeller.get(&m).is_some_and(|x| x.yumusak) && e.yumusak.is_none() {
                            e.yumusak = Some(c.satir);
                        }
                    }
                } else if !iliski_cagrisi && (c.statik || OKUMA.contains(&ad) || ad.starts_with("where") || ad == "with") {
                    e.okur.insert(m.clone());
                }
                // with('comments') / load('comments'): ilişkili modeller okunur
                if matches!(ad, "with" | "load" | "loadMissing") {
                    if let Some(mm) = self.modeller.get(&m) {
                        for r in c.arglar.iter().flat_map(|a| metinler(&a.deger)) {
                            if let Some((_, h, _)) = mm.iliskiler.get(r.split('.').next().unwrap_or(&r)) {
                                e.okur.insert(h.clone());
                            }
                        }
                    }
                }
            }
            // Çağrılan proje gövdeleri: $this->m(), $this->servis->m(), $servis->m(), Sinif::m(), app(X::class)->m()
            if e.cagrilan.len() < 16 {
                let hedef = match (&c.alici, c.statik) {
                    (Some(Ifade::Ad(s)), true) => match s.as_str() {
                        "self" | "static" => ctx,
                        "parent" => ctx.and_then(|y| self.ust(y)),
                        s => self.sinif(di, s),
                    },
                    (Some(a), false) => self.ifade_sinifi(di, ctx, &vars, a, 0),
                    _ => None,
                };
                if let Some((u, m)) = hedef.and_then(|y| self.metot(y, ad)) {
                    let gm = Govde::Metot(u, m);
                    if !e.cagrilan.iter().any(|x| x.kimlik() == gm.kimlik()) && gm.kimlik() != g.kimlik() {
                        e.cagrilan.push(gm);
                    }
                }
            }
        }
        e
    }

    /// İşlemin davranış zinciri: işleyici (derinlik 0), proje ara katmanları ve Form Request (bağımlılık), işleyicinin
    /// çağırdığı servisler (2 kat). Dönen: (davranış özeti, veri bağı)
    fn zincir(&self, isleyici: Govde<'a>, bagimliliklar: &[Govde<'a>], ek_okuma: &BTreeSet<String>) -> (Value, VeriBag) {
        let mut sira: Vec<(Govde<'a>, usize, &'static str)> = vec![(isleyici, 0, "isleyici")];
        sira.extend(bagimliliklar.iter().map(|g| (*g, 1, "bagimlilik")));
        let mut gorulen: HashSet<(usize, usize)> = HashSet::new();
        let mut dnalar: Vec<(String, GovdeDna, usize, &'static str)> = vec![];
        let mut bag = VeriBag::default();
        let mut i = 0;
        while i < sira.len() && dnalar.len() < 32 {
            let (g, derinlik, tur) = sira[i];
            i += 1;
            if !gorulen.insert(g.kimlik()) {
                continue;
            }
            let e = self.etki(g);
            if derinlik <= 3 {
                bag.okur.extend(e.okur.iter().cloned());
                bag.yazar.extend(e.yazar.iter().cloned());
                bag.eksik_olabilir |= e.eksik;
            }
            let mut dna = g.dna().clone();
            dna.silinen = e.silinen.iter().map(|(m, _)| m.clone()).collect::<BTreeSet<_>>().into_iter().collect();
            dna.yumusak = dna.yumusak.or(e.yumusak);
            dna.tetikler = e.tetikler.clone();
            dnalar.push((self.goreli(g.di()), dna, derinlik, tur));
            if derinlik < 2 {
                let alt = if tur == "bagimlilik" { "bagimlilik" } else { "servis" };
                sira.extend(e.cagrilan.iter().map(|c| (*c, derinlik + 1, alt)));
            }
        }
        bag.okur.extend(ek_okuma.iter().cloned());
        bag.okur.sort();
        bag.okur.dedup();
        bag.yazar.sort();
        bag.yazar.dedup();
        let halkalar: Vec<crate::dna::Halka> = dnalar.iter().map(|(d, dna, derinlik, tur)| crate::dna::Halka { dosya: d.clone(), dna, derinlik: *derinlik, tur }).collect();
        (crate::dna::ozet(&halkalar), bag)
    }
}

// ---- Girdiler: doğrulama kuralları ----

/// Laravel doğrulama kuralının okunan biçimi
#[derive(Clone, Debug)]
enum LKural {
    /// Metin kuralı: ad (küçük harf) ve parametreler (`max:255` -> ("max", "255"))
    Metin(String, String),
    In(Vec<String>),
    Enum(String, Vec<String>),
    /// Özel kural sınıfı ya da kapanış
    Ozel(String),
    /// Kuralı çalışma anında kurulan alan: koşula bağlı (`$x ? [...] : null`), değişken ya da tanımı izlenemeyen
    /// çağrı. Kural adı değildir; alanın zorunluluğu belirsiz kalır ve önizleme notu yazılır
    Dinamik(String),
    /// Koşullu zorunluluk (`Rule::requiredIf(...)`)
    Kosullu,
    Parola(Option<String>),
    Dosya,
}

#[derive(Default, Debug)]
struct Dugum {
    kurallar: Vec<LKural>,
    var: bool,
    /// Kuralın kaynak satırı (alt anahtarlarla tanımlanan düğümde 0: ilk alt anahtarınki kullanılır)
    satir: usize,
    cocuklar: BTreeMap<String, Dugum>,
}

impl Baglam<'_> {
    /// Kural değeri -> kurallar: 'required|string|max:255', ['required', Rule::in([...]), new Enum(X::class)]
    fn kurallar(&self, di: usize, x: &Ifade) -> Vec<LKural> {
        self.kural_ifadesi(di, x, 0)
    }

    /// `kurallar`, iç içe kural kaynakları açılarak: `array_merge([...], $this->kurallar())` ve projenin kural dizisi
    /// döndüren metotları (en çok 4 kat)
    fn kural_ifadesi(&self, di: usize, x: &Ifade, d: usize) -> Vec<LKural> {
        let metin_kurali = |t: &str| -> LKural {
            let (a, p) = t.split_once(':').unwrap_or((t, ""));
            LKural::Metin(a.trim().to_lowercase(), p.to_string())
        };
        match x {
            Ifade::Metin(t) => t.split('|').filter(|p| !p.trim().is_empty()).map(|p| metin_kurali(p.trim())).collect(),
            Ifade::Dizi(v, _) => v
                .iter()
                .flat_map(|(_, e)| match e {
                    Ifade::Metin(t) => vec![metin_kurali(t.trim())],
                    e => self.kural_ifadesi(di, e, d + 1),
                })
                .collect(),
            Ifade::Yeni(s, a) => vec![match kisa_ad(s) {
                "Enum" => match a.first().map(|a| &a.deger) {
                    Some(Ifade::Sinif(e)) => self.enum_kurali(di, e),
                    _ => LKural::Ozel("Enum".into()),
                },
                "In" => LKural::In(a.iter().flat_map(|a| metinler(&a.deger)).collect()),
                "Password" => LKural::Parola(None),
                "File" | "ImageFile" | "Dimensions" => LKural::Dosya,
                "Unique" | "Exists" => LKural::Metin(kisa_ad(s).to_lowercase(), a.first().and_then(|a| metinler(&a.deger).into_iter().next()).unwrap_or_default()),
                k => LKural::Ozel(k.to_string()),
            }],
            Ifade::Cagri(c) if c.alici.is_none() && c.ad.eq_ignore_ascii_case("array_merge") && d < 4 => {
                c.arglar.iter().flat_map(|a| self.kural_ifadesi(di, &a.deger, d + 1)).collect()
            }
            Ifade::Cagri(_) => {
                let (kok, h) = halkalar(x);
                let Some(ilk) = h.first() else { return vec![LKural::Dinamik("ifade".into())] };
                // Projenin kural dizisi döndüren metodu: `$this->getImageValidationRules()` (dosyadaki sınıflarda ve
                // atalarında aranır), gövdesi tek `return [...]`
                if h.len() == 1 && matches!(kok, Some(Ifade::Degisken(t)) if t == "this") && d < 4 {
                    let govde = (0..self.dosyalar[di].siniflar.len()).find_map(|ci| self.metot((di, ci), &ilk.ad)).map(|(u, m)| (u.0, &m.govde));
                    if let Some((u, [Ifade::Don(r)])) = govde.map(|(u, g)| (u, g.as_slice())) {
                        return self.kural_ifadesi(u, r, d + 1);
                    }
                }
                match kok.map(|k| match k {
                    Ifade::Ad(s) => kisa_ad(s).to_string(),
                    _ => String::new(),
                }) {
                    Some(s) if s == "Rule" => vec![match ilk.ad.as_str() {
                        "in" => LKural::In(ilk.arglar.iter().flat_map(|a| metinler(&a.deger)).collect()),
                        "enum" => match ilk.arglar.first().map(|a| &a.deger) {
                            Some(Ifade::Sinif(e)) => self.enum_kurali(di, e),
                            _ => LKural::Ozel("Rule::enum".into()),
                        },
                        "unique" | "exists" => LKural::Metin(ilk.ad.clone(), ilk.arglar.first().and_then(|a| metinler(&a.deger).into_iter().next()).unwrap_or_default()),
                        "requiredIf" | "requiredUnless" | "prohibitedIf" | "prohibitedUnless" | "excludeIf" | "excludeUnless" | "when" | "unless" => LKural::Kosullu,
                        "dimensions" | "file" | "imageFile" => LKural::Dosya,
                        "notIn" => LKural::Metin("not_in".into(), String::new()),
                        a => LKural::Ozel(format!("Rule::{a}")),
                    }],
                    Some(s) if s == "Password" => vec![LKural::Parola(h.iter().find(|c| c.ad == "min").and_then(|c| c.arglar.first()).and_then(|a| match &a.deger {
                        Ifade::Sayi(n) => Some(n.clone()),
                        _ => None,
                    }))],
                    Some(s) if s == "File" => vec![LKural::Dosya],
                    // Projedeki kural sınıfının fabrikası (`Uppercase::make()`): özel kural
                    Some(s) if !s.is_empty() && self.sinif(di, &s).is_some() => vec![LKural::Ozel(s)],
                    _ => vec![LKural::Dinamik(format!("{}()", ilk.ad))],
                }
            }
            Ifade::Kapanis(_) => vec![LKural::Ozel("kapanış".into())],
            // 'max:' . $boy: kuralın adı yazılı, değeri çalışma anında ("max:…"); izinli değer listesi bilinmeden
            // `in` kuralı kurulamaz
            Ifade::Birlesim(v) => match v.first() {
                Some(Ifade::Metin(t)) if v.len() > 1 => {
                    // 'required|max:' . $boy: sondaki kuralın değeri çalışma anında; öncekiler tam
                    let mut p: Vec<&str> = t.split('|').map(str::trim).collect();
                    let son = p.pop().unwrap_or("");
                    let mut l: Vec<LKural> = p.into_iter().filter(|x| !x.is_empty()).map(metin_kurali).collect();
                    if !son.is_empty() {
                        l.push(match metin_kurali(&format!("{son}…")) {
                            LKural::Metin(a, _) if matches!(a.as_str(), "in" | "not_in") => LKural::Dinamik(format!("{a}:…")),
                            k => k,
                        });
                    }
                    l
                }
                Some(Ifade::Metin(t)) => vec![metin_kurali(t.trim_end_matches(':').trim())],
                _ => vec![LKural::Dinamik("birleştirilen metin".into())],
            },
            Ifade::Degisken(v) => vec![LKural::Dinamik(format!("${v}"))],
            Ifade::Diger(t, _) | Ifade::Ad(t) => vec![LKural::Dinamik(t.clone())],
            _ => vec![LKural::Dinamik("ifade".into())],
        }
    }

    /// PHP enum'unun değerleri (backed enum'da değer, değilse durum adı)
    fn enum_kurali(&self, di: usize, e: &str) -> LKural {
        match self.sinif(di, e).map(|y| self.sinif_(y)).filter(|s| s.tur == "enum") {
            Some(s) => LKural::Enum(s.ad.clone(), s.durumlar.iter().map(|(a, v)| v.clone().unwrap_or_else(|| a.clone())).collect()),
            None => LKural::Enum(kisa_ad(e).to_string(), vec![]),
        }
    }

    /// Kural dizisi -> alanlar (`items.*.name` iç içe), önizlemenin denetleyemediği notlar ve kuralların okuduğu tablolar
    fn kural_semasi(&self, di: usize, dizi: &[(Option<Ifade>, Ifade)], satirlar: &[usize], satir: usize) -> (BTreeMap<String, Alan>, Vec<String>, BTreeSet<String>) {
        let mut kok = Dugum::default();
        let mut notlar = vec![];
        let mut tablolar = BTreeSet::new();
        for (i, (a, e)) in dizi.iter().enumerate() {
            let Some(Ifade::Metin(anahtar)) = a else {
                if a.is_some() {
                    notlar.push("alan adı koddan çözülemedi: kural dizisinin bir anahtarı ifade".to_string());
                }
                continue;
            };
            let kurallar = self.kurallar(di, e);
            for k in &kurallar {
                match k {
                    LKural::Metin(ad, p) if ad == "exists" || ad == "unique" => {
                        if let Some(t) = p.split(',').next().map(str::trim).filter(|t| !t.is_empty()) {
                            let t = kisa_ad(t).rsplit('.').next().unwrap_or(t).to_string();
                            tablolar.insert(self.tablolar.get(&t.to_lowercase()).cloned().or_else(|| self.modeller.contains_key(&t).then(|| t.clone())).unwrap_or(t));
                        }
                    }
                    LKural::Ozel(o) => notlar.push(format!("özel doğrulama kuralı kontrol edilemedi: '{anahtar}' alanı ({o}); kuralın kodu önizlemede çalıştırılmaz")),
                    LKural::Dinamik(o) => {
                        notlar.push(format!("doğrulama kuralı çalışma anında kuruluyor: '{anahtar}' alanı ({o}); önizleme bu kuralı sınamaz, zorunluluğu belirsiz"))
                    }
                    LKural::Dosya => notlar.push(format!("dosya doğrulaması kontrol edilemedi: '{anahtar}' alanı; dosyanın varlığı, türü ve içeriği önizlemede sınanmaz")),
                    LKural::Metin(ad, _) if matches!(ad.as_str(), "file" | "image" | "mimes" | "mimetypes" | "dimensions" | "extensions") => {
                        let n = format!("dosya doğrulaması kontrol edilemedi: '{anahtar}' alanı; dosyanın varlığı, türü ve içeriği önizlemede sınanmaz");
                        if !notlar.contains(&n) {
                            notlar.push(n);
                        }
                    }
                    _ => {}
                }
            }
            let mut d = &mut kok;
            for p in anahtar.split('.') {
                d = d.cocuklar.entry(p.to_string()).or_default();
            }
            d.var = true;
            d.satir = satirlar.get(i).copied().unwrap_or(satir);
            d.kurallar.extend(kurallar);
        }
        let mut alanlar = BTreeMap::new();
        for (ad, d) in &kok.cocuklar {
            if ad != "*" {
                alanlar.insert(ad.clone(), alan_kur(d, satir));
            }
        }
        notlar.dedup();
        (alanlar, notlar, tablolar)
    }
}

/// Kural listesine tekrarsız ekler
fn kural_ekle(kurallar: &mut Vec<String>, k: String) {
    if !kurallar.contains(&k) {
        kurallar.push(k);
    }
}

/// `min` / `max` kuralının önizleme adı alanın tipine göre: metin uzunluğu, sayı değeri ya da dizi boyu (Laravel'in
/// `getSize` kuralı). Tip bilinmiyorsa çevrilmez
fn olcu_kurali(tip: Option<&str>, alt: bool, n: &str) -> Option<String> {
    let n = n.trim();
    n.parse::<f64>().ok()?;
    let ad = match (tip?, alt) {
        ("string", true) => "MinLength",
        ("string", false) => "MaxLength",
        ("integer" | "number", true) => "Min",
        ("integer" | "number", false) => "Max",
        ("array", true) => "ArrayMinSize",
        ("array", false) => "ArrayMaxSize",
        _ => return None,
    };
    Some(format!("{ad}({n})"))
}

/// Kurallardan Sezgi alanı: tip, zorunluluk, önizlemenin kural adları (çevrilemeyen kural olduğu gibi), izinli değerler,
/// iç içe alanlar (`adres.sehir` nesne, `ogeler.*.ad` nesne dizisi; `etiketler.*` kuralları her öğeye)
fn alan_kur(d: &Dugum, satir: usize) -> Alan {
    let satir = if d.satir > 0 { d.satir } else { d.cocuklar.values().map(|c| c.satir).find(|s| *s > 0).unwrap_or(satir) };
    let adlar: Vec<&str> = d.kurallar.iter().filter_map(|k| if let LKural::Metin(a, _) = k { Some(a.as_str()) } else { None }).collect();
    let var = |a: &str| adlar.contains(&a);
    let mut ic: BTreeMap<String, Alan> = BTreeMap::new();
    let mut oge: Option<Alan> = None;
    for (ad, c) in &d.cocuklar {
        if ad == "*" {
            oge = Some(alan_kur(c, satir));
        } else {
            ic.insert(ad.clone(), alan_kur(c, satir));
        }
    }
    let nesne = !ic.is_empty();
    let tip: Option<&str> = if var("integer") || var("int") {
        Some("integer")
    } else if var("numeric") || var("decimal") {
        Some("number")
    } else if var("boolean") || var("bool") {
        Some("boolean")
    } else if nesne {
        Some("object")
    } else if var("array") || var("list") || oge.is_some() {
        Some("array")
    } else if d.kurallar.iter().any(|k| matches!(k, LKural::Dosya)) || var("file") || var("image") || var("mimes") || var("mimetypes") {
        Some("file")
    } else if var("date") {
        Some("datetime")
    } else if ["string", "email", "uuid", "url", "ip", "json", "alpha", "alpha_num", "alpha_dash"].iter().any(|a| var(a)) {
        Some("string")
    } else if var("in") || d.kurallar.iter().any(|k| matches!(k, LKural::In(_) | LKural::Enum(..))) {
        Some("enum")
    } else {
        None
    };
    let mut kurallar: Vec<String> = vec![];
    let mut degerler: Vec<String> = vec![];
    let mut zorunlu = false;
    let mut belirsiz = false;
    let liste = |v: &[String]| format!("IsIn([{}])", v.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", "));
    for k in &d.kurallar {
        match k {
            LKural::Metin(a, p) => match a.as_str() {
                "required" | "present" => zorunlu = true,
                "nullable" => kural_ekle(&mut kurallar, "nullable()".into()),
                "sometimes" | "bail" | "exclude" => {}
                a if a.starts_with("required_") || a.starts_with("present_") => belirsiz = true,
                "filled" => kural_ekle(&mut kurallar, "IsNotEmpty()".into()),
                "string" => kural_ekle(&mut kurallar, "IsString()".into()),
                "integer" | "int" => kural_ekle(&mut kurallar, "IsInt()".into()),
                "numeric" | "decimal" => kural_ekle(&mut kurallar, "IsNumber()".into()),
                "boolean" | "bool" => kural_ekle(&mut kurallar, "IsBoolean()".into()),
                // Laravel `array` ilişkisel diziyi (JSON nesnesi) de kabul eder
                "array" | "list" => kural_ekle(&mut kurallar, if nesne { "IsObject()".into() } else { "IsArray()".into() }),
                "email" => kural_ekle(&mut kurallar, "IsEmail()".into()),
                "uuid" => kural_ekle(&mut kurallar, "IsUUID()".into()),
                "url" => kural_ekle(&mut kurallar, "IsUrl()".into()),
                "date" => kural_ekle(&mut kurallar, "IsDate()".into()),
                "in" => {
                    degerler = p.split(',').map(|x| x.trim().trim_matches('"').to_string()).filter(|x| !x.is_empty()).collect();
                    kural_ekle(&mut kurallar, liste(&degerler));
                }
                "min" | "max" => kural_ekle(&mut kurallar, olcu_kurali(tip, a == "min", p).unwrap_or_else(|| format!("{a}:{p}"))),
                "between" | "size" => {
                    let (x, y) = if a == "size" { (p.as_str(), p.as_str()) } else { p.split_once(',').unwrap_or((p, "")) };
                    match (olcu_kurali(tip, true, x), olcu_kurali(tip, false, y)) {
                        (Some(r1), Some(r2)) => {
                            kural_ekle(&mut kurallar, r1);
                            kural_ekle(&mut kurallar, r2);
                        }
                        _ => kural_ekle(&mut kurallar, format!("{a}:{p}")),
                    }
                }
                "regex" if p.starts_with('/') && p.rfind('/').is_some_and(|i| i > 0) => kural_ekle(&mut kurallar, format!("Matches({p})")),
                a => kural_ekle(&mut kurallar, if p.is_empty() { a.to_string() } else { format!("{a}:{p}") }),
            },
            LKural::In(v) => {
                degerler = v.clone();
                kural_ekle(&mut kurallar, liste(v));
            }
            LKural::Enum(ad, v) if v.is_empty() => kural_ekle(&mut kurallar, format!("enum:{ad}")),
            LKural::Enum(_, v) => {
                degerler = v.clone();
                kural_ekle(&mut kurallar, liste(v));
            }
            LKural::Ozel(o) => kural_ekle(&mut kurallar, format!("{}()", o.replace("::", "."))),
            LKural::Kosullu | LKural::Dinamik(_) => belirsiz = true,
            LKural::Parola(min) => {
                kural_ekle(&mut kurallar, "Password()".into());
                if let Some(n) = min {
                    kural_ekle(&mut kurallar, format!("MinLength({n})"));
                }
            }
            LKural::Dosya => {}
        }
    }
    // `etiketler.*` kuralları dizinin her öğesine: `IsString({ each: true })`
    if let Some(o) = &oge {
        for r in o.kurallar.iter().filter(|r| r.ends_with(')') && *r != "nullable()") {
            let (ad, arg) = r.split_once('(').unwrap_or((r, ")"));
            let arg = arg.trim_end_matches(')');
            kural_ekle(&mut kurallar, if arg.is_empty() { format!("{ad}({{ each: true }})") } else { format!("{ad}({arg}, {{ each: true }})") });
        }
    }
    if var("sometimes") || belirsiz {
        zorunlu = false;
    }
    // Yalnızca alt anahtarları kurallı nesne: alt alanlardan biri zorunluysa nesne de gönderilmeli
    if !d.var && nesne {
        zorunlu = ic.values().any(|a| a.zorunlu);
    }
    let ic_sema = if nesne { Some(ic) } else { oge.and_then(|o| o.ic) };
    Alan { tip: tip.map(String::from), zorunlu, belirsiz, kurallar, satir, kalitimdan: None, degerler, ic: ic_sema }
}

// ---- Kapı parçaları: girdiler, yanıt, giriş ----

/// API Resource sınıfları (ResourceCollection da bir JsonResource'tur)
const KAYNAK_SINIFLARI: [&str; 3] =
    ["Illuminate\\Http\\Resources\\Json\\JsonResource", "Illuminate\\Http\\Resources\\Json\\ResourceCollection", "Illuminate\\Http\\Resources\\Json\\AnonymousResourceCollection"];

/// Dizi öğeleri ve satırları
type DiziParcasi<'x> = (&'x [(Option<Ifade>, Ifade)], &'x [usize]);

/// Gövdedeki kural dizisi ifadesi: dizi sabiti ya da gövdede dizi atanmış değişken
fn dizi_coz<'x>(x: &'x Ifade, govde: &'x [Ifade]) -> Option<DiziParcasi<'x>> {
    match x {
        Ifade::Dizi(v, s) => Some((v, s)),
        Ifade::Degisken(a) => govde.iter().find_map(|y| match y {
            Ifade::Ata(sol, sag) if matches!(sol.as_ref(), Ifade::Degisken(b) if b == a) => match sag.as_ref() {
                Ifade::Dizi(v, s) => Some((v.as_slice(), s.as_slice())),
                _ => None,
            },
            _ => None,
        }),
        _ => None,
    }
}

/// Form Request kuralları: öğeler, satırları, sınıfın dosyası ve `rules()` satırı
type FormKurallari = (Vec<(Option<Ifade>, Ifade)>, Vec<usize>, usize, usize);

/// Girdinin kaynağı: Form Request sınıfı, satır içi doğrulama ya da yalnızca okunan istek alanları
struct GirdiSonucu {
    sema: Option<Sema>,
    notlar: Vec<String>,
    tablolar: BTreeSet<String>,
    form_istegi: Option<Yer>,
}

impl<'a> Baglam<'a> {
    /// Form Request'in kuralları: `rules()` dönüşü (dizi, dizi atanmış değişken, `array_merge(parent::rules(), [...])`)
    fn form_kurallari(&self, y: Yer, d: usize) -> Option<FormKurallari> {
        let (u, m) = self.metot(y, "rules")?;
        if d > 6 {
            return None;
        }
        for x in &m.govde {
            let Ifade::Don(v) = x else { continue };
            if let Some((dz, sz)) = dizi_coz(v, &m.govde) {
                return Some((dz.to_vec(), sz.to_vec(), u.0, m.satir));
            }
            if let Ifade::Cagri(c) = v.as_ref() {
                if c.ad == "array_merge" && c.alici.is_none() {
                    let (mut toplam, mut satirlar) = (vec![], vec![]);
                    for a in &c.arglar {
                        match &a.deger {
                            Ifade::Cagri(p) if p.ad == "rules" && matches!(&p.alici, Some(Ifade::Ad(s)) if s == "parent") => {
                                let (v, sz, ..) = self.ust(u).and_then(|z| self.form_kurallari(z, d + 1))?;
                                toplam.extend(v);
                                satirlar.extend(sz);
                            }
                            x => {
                                let (v, sz) = dizi_coz(x, &m.govde)?;
                                toplam.extend(v.iter().cloned());
                                satirlar.extend(sz.iter().copied());
                            }
                        }
                    }
                    return Some((toplam, satirlar, u.0, m.satir));
                }
            }
        }
        None
    }

    /// İşleyicinin girdisi
    fn girdi(&self, isleyici: Option<Govde<'a>>) -> GirdiSonucu {
        let mut s = GirdiSonucu { sema: None, notlar: vec![], tablolar: BTreeSet::new(), form_istegi: None };
        let Some(g) = isleyici else { return s };
        let di = g.di();
        // Satır içi doğrulamanın ve okunan alanların şema adı işleme özgüdür: "AuthController.register", "web:12"
        let sema_adi = match g {
            Govde::Metot(y, m) => format!("{}.{}", self.sinif_(y).ad, m.ad),
            Govde::Kapanis(di, k) => format!("{}:{}", self.goreli(di).rsplit('/').next().unwrap_or("").trim_end_matches(".php"), k.satir),
        };
        // 1) Form Request parametresi
        for p in g.parametreler() {
            let Some(y) = p.tip.as_deref().and_then(|t| self.sinif(di, t)) else { continue };
            if !self.turer(y, &["Illuminate\\Foundation\\Http\\FormRequest"]) {
                continue;
            }
            s.form_istegi = Some(y);
            let sinif = self.sinif_(y);
            match self.form_kurallari(y, 0) {
                Some((dizi, satirlar, kdi, satir)) => {
                    let (alanlar, notlar, tablolar) = self.kural_semasi(kdi, &dizi, &satirlar, satir);
                    s.notlar = notlar;
                    s.tablolar = tablolar;
                    s.sema =
                        Some(Sema { dto: sinif.ad.clone(), dosya: Some(self.goreli(y.0)), bulunamadi: false, kalitim: sinif.taban.clone(), alanlar, adaylar: vec![], dizi: false });
                }
                None => {
                    s.sema = Some(Sema {
                        dto: sinif.ad.clone(),
                        dosya: Some(self.goreli(y.0)),
                        bulunamadi: true,
                        kalitim: sinif.taban.clone(),
                        alanlar: BTreeMap::new(),
                        adaylar: vec![],
                        dizi: false,
                    })
                }
            }
            for (ad, neden) in [("withValidator", "withValidator()"), ("after", "after()"), ("prepareForValidation", "prepareForValidation()")] {
                if self.metot(y, ad).is_some() {
                    s.notlar.push(format!("nesne düzeyi doğrulama kontrol edilemedi: {} sınıfının {neden} kodu önizlemede çalıştırılmaz", sinif.ad));
                }
            }
            return s;
        }
        // 2) Satır içi doğrulama: $request->validate([...]), request()->validate(...), $this->validate($request, [...]),
        //    Validator::make($request->all(), [...]), $request->validateWithBag('bag', [...])
        let mut bulunan: Option<(DiziParcasi, usize)> = None;
        let govde = g.ifadeler();
        for x in govde {
            ifade_gez(
                x,
                &mut |y| {
                    let Ifade::Cagri(c) = y else { return };
                    if bulunan.is_some() {
                        return;
                    }
                    let kurallar = match (c.ad.as_str(), &c.alici, c.statik) {
                        ("validate", Some(Ifade::Degisken(t)), false) if t == "this" => arg(c, "rules", 1),
                        ("validate", Some(_), false) => arg(c, "rules", 0),
                        ("validateWithBag", Some(_), false) => arg(c, "rules", 1),
                        ("make" | "validate", Some(Ifade::Ad(v)), true) if kisa_ad(v) == "Validator" => arg(c, "rules", 1),
                        _ => None,
                    };
                    if let Some(dz) = kurallar.and_then(|k| dizi_coz(k, govde)) {
                        bulunan = Some((dz, c.satir));
                    }
                },
                0,
            );
        }
        if let Some((dz, satir)) = bulunan {
            let (alanlar, notlar, tablolar) = self.kural_semasi(di, dz.0, dz.1, satir);
            s.notlar = notlar;
            s.tablolar = tablolar;
            s.sema = Some(Sema { dto: sema_adi, dosya: Some(self.goreli(di)), bulunamadi: false, kalitim: None, alanlar, adaylar: vec![], dizi: false });
            return s;
        }
        // 3) Doğrulamasız okunan alanlar: $request->input('x'), ->get('x'), ->integer('x'), ->boolean('x'), ->only([...]),
        //    $request->x; ->all() bütün gövdeyi alır (şema bilinmez)
        let istek: Vec<&str> = g
            .parametreler()
            .iter()
            .filter(|p| p.tip.as_deref().is_some_and(|t| kisa_ad(t) == "Request" || self.sinif(di, t).is_some_and(|y| self.turer(y, &["Illuminate\\Http\\Request"]))))
            .map(|p| p.ad.as_str())
            .collect();
        let istek_mi = |a: &Ifade| match a {
            Ifade::Degisken(v) => istek.contains(&v.as_str()),
            Ifade::Cagri(c) => c.alici.is_none() && c.ad == "request" && c.arglar.is_empty(),
            _ => false,
        };
        let mut alanlar: BTreeMap<String, Alan> = BTreeMap::new();
        let mut hepsi = false;
        let alan = |tip: Option<&str>, satir: usize| Alan {
            tip: tip.map(String::from),
            zorunlu: false,
            belirsiz: true,
            kurallar: vec![],
            satir,
            kalitimdan: None,
            degerler: vec![],
            ic: None,
        };
        for x in govde {
            ifade_gez(
                x,
                &mut |y| match y {
                    Ifade::Cagri(c) if c.alici.as_ref().is_some_and(istek_mi) => {
                        let tip = match c.ad.as_str() {
                            "input" | "get" | "post" | "query" | "string" | "str" | "has" | "filled" => Some(None),
                            "integer" => Some(Some("integer")),
                            "float" => Some(Some("number")),
                            "boolean" => Some(Some("boolean")),
                            "date" => Some(Some("datetime")),
                            "file" => Some(Some("file")),
                            "only" | "except" => {
                                for a in c.arglar.iter().flat_map(|a| metinler(&a.deger)) {
                                    alanlar.entry(a).or_insert_with(|| alan(None, c.satir));
                                }
                                None
                            }
                            "all" | "json" if c.arglar.is_empty() => {
                                hepsi = true;
                                None
                            }
                            _ => None,
                        };
                        if let (Some(tip), Some(Ifade::Metin(a))) = (tip, c.arglar.first().map(|a| &a.deger)) {
                            let a = a.split('.').next().unwrap_or(a).to_string();
                            alanlar.entry(a).or_insert_with(|| alan(tip, c.satir));
                        }
                    }
                    Ifade::Ozellik(o, p) if istek_mi(o) => {
                        alanlar.entry(p.clone()).or_insert_with(|| alan(None, g.satir()));
                    }
                    _ => {}
                },
                0,
            );
        }
        if hepsi || !alanlar.is_empty() {
            s.sema = Some(Sema { dto: sema_adi, dosya: Some(self.goreli(di)), bulunamadi: hepsi, kalitim: None, alanlar, adaylar: vec![], dizi: false });
        }
        s
    }

    /// API Resource sınıfının alanları: `toArray()` dizisi. Koşullu alanlar (`when`, `whenLoaded`, `mergeWhen`)
    /// zorunlu değildir; iç kaynaklar (`new UserResource(...)`, `X::collection(...)`) iç içe çözülür. Dönen: (alanlar,
    /// eksiksiz mi)
    fn kaynak_alanlari(&self, y: Yer, d: usize) -> Option<(BTreeMap<String, Alan>, bool)> {
        if d > 3 {
            return None;
        }
        let (u, m) = self.metot(y, "toArray").filter(|(u, _)| self.tam_sinif(&self.tam(*u)).is_some())?;
        let dizi = m.govde.iter().find_map(|x| match x {
            Ifade::Don(v) => dizi_coz(v, &m.govde).map(|x| x.0),
            _ => None,
        })?;
        let mut alanlar = BTreeMap::new();
        let mut eksiksiz = true;
        for (a, e) in dizi {
            let Some(Ifade::Metin(a)) = a else {
                eksiksiz = false;
                continue;
            };
            let mut alan = Alan { tip: None, zorunlu: true, belirsiz: false, kurallar: vec![], satir: m.satir, kalitimdan: None, degerler: vec![], ic: None };
            // Koşullu değer: `$this->when(koşul, değer)`, `whenNotNull(değer)`, `whenLoaded('ilişki', değer?)`; iç kaynağın
            // argümanı koşulluysa (`new UserResource($this->whenLoaded('author'))`) alan da koşulludur
            let kosullu = |c: &Cagri| matches!(&c.alici, Some(Ifade::Degisken(t)) if t == "this") && c.ad.starts_with("when");
            let mut x = e;
            let mut bilinmez = false;
            if let Ifade::Cagri(c) = x {
                if kosullu(c) {
                    alan.zorunlu = false;
                    match c.ad.as_str() {
                        "when" | "whenHas" => x = c.arglar.get(1).map(|a| &a.deger).unwrap_or(x),
                        "whenNotNull" => x = c.arglar.first().map(|a| &a.deger).unwrap_or(x),
                        _ => match c.arglar.get(1) {
                            Some(a) => x = &a.deger,
                            None => bilinmez = true,
                        },
                    }
                }
            }
            if ifade_ara(x, &mut |c| kosullu(c), 0) {
                alan.zorunlu = false;
            }
            if bilinmez {
                alanlar.insert(a.clone(), alan);
                continue;
            }
            match x {
                Ifade::Donustur(t, _) => {
                    alan.tip = Some(
                        match t.as_str() {
                            "int" | "integer" => "integer",
                            "bool" | "boolean" => "boolean",
                            "float" | "double" => "number",
                            "array" => "array",
                            _ => "string",
                        }
                        .into(),
                    );
                }
                Ifade::Metin(_) => alan.tip = Some("string".into()),
                Ifade::Sayi(_) => alan.tip = Some("number".into()),
                Ifade::Ad(b) if b == "true" || b == "false" => alan.tip = Some("boolean".into()),
                Ifade::Yeni(s, _) => {
                    if let Some(z) = self.sinif(u.0, s).filter(|z| self.turer(*z, &KAYNAK_SINIFLARI)) {
                        alan.tip = Some("object".into());
                        alan.ic = self.kaynak_alanlari(z, d + 1).map(|x| x.0);
                    }
                }
                Ifade::Cagri(c) if c.statik => {
                    if let Some(Ifade::Ad(s)) = &c.alici {
                        if let Some(z) = self.sinif(u.0, s).filter(|z| self.turer(*z, &KAYNAK_SINIFLARI)) {
                            alan.tip = Some(if c.ad == "collection" { "array" } else { "object" }.into());
                            alan.ic = self.kaynak_alanlari(z, d + 1).map(|x| x.0);
                        }
                    }
                }
                _ => {}
            }
            alanlar.insert(a.clone(), alan);
        }
        Some((alanlar, eksiksiz))
    }

    /// Yanıt şeması: `return new PostResource(...)`, `PostResource::make(...)`, `PostResource::collection(...)`,
    /// `new PostCollection(...)` (API Resource, `data` sarmalıyla) ya da `response()->json([...], kod)` (yorum)
    fn yanit(&self, g: Govde<'a>) -> Option<Yanit> {
        let di = g.di();
        let kaynak_mi = |y: Yer| self.turer(y, &KAYNAK_SINIFLARI);
        let mut donusler: Vec<&Ifade> = vec![];
        for x in g.ifadeler() {
            ifade_gez(
                x,
                &mut |y| {
                    if let Ifade::Don(v) = y {
                        donusler.push(v);
                    }
                },
                0,
            );
        }
        for v in donusler {
            let (kok, h) = halkalar(v);
            let durum = h.iter().find(|c| c.ad == "setStatusCode").and_then(|c| c.arglar.first()).and_then(|a| php_kod(&ifade_metni(&a.deger)));
            // Kaynak sınıfı ve dizi mi
            let kaynak = match (kok, h.first()) {
                (Some(Ifade::Yeni(s, _)), _) => self.sinif(di, s).map(|y| (y, false)),
                (_, Some(c)) if c.statik => match &c.alici {
                    Some(Ifade::Ad(s)) if matches!(c.ad.as_str(), "make" | "collection") => self.sinif(di, s).map(|y| (y, c.ad == "collection")),
                    _ => None,
                },
                _ => match v {
                    Ifade::Yeni(s, _) => self.sinif(di, s).map(|y| (y, false)),
                    _ => None,
                },
            };
            if let Some((y, mut dizi)) = kaynak.filter(|(y, _)| kaynak_mi(*y)) {
                // ResourceCollection: öğe kaynağı $collects ya da ad kuralı (PostCollection -> PostResource / Post)
                let mut oge = y;
                if self.turer(y, &["Illuminate\\Http\\Resources\\Json\\ResourceCollection"]) {
                    dizi = true;
                    let ad = self.sinif_(y).ad.trim_end_matches("Collection").to_string();
                    oge = self
                        .ozellik(y, "collects")
                        .and_then(|(u, o)| match &o.deger {
                            Some(Ifade::Sinif(s)) => self.sinif(u.0, s),
                            _ => None,
                        })
                        .or_else(|| self.sinif(y.0, &format!("{ad}Resource")))
                        .unwrap_or(y);
                }
                let (alanlar, eksiksiz) = self.kaynak_alanlari(oge, 0).unwrap_or_default();
                let sarma = match self.ozellik(y, "wrap").and_then(|(_, o)| o.deger.as_ref()) {
                    Some(Ifade::Ad(n)) if n.eq_ignore_ascii_case("null") => None,
                    Some(Ifade::Metin(w)) => Some(w.clone()),
                    _ if self.sarmasiz => None,
                    _ => Some("data".to_string()),
                };
                let sinif = self.sinif_(oge);
                let sema_alanlari = match &sarma {
                    Some(w) => BTreeMap::from([(
                        w.clone(),
                        Alan {
                            tip: Some(if dizi { "array" } else { "object" }.into()),
                            zorunlu: true,
                            belirsiz: false,
                            kurallar: vec![],
                            satir: sinif.satir,
                            kalitimdan: None,
                            degerler: vec![],
                            ic: Some(alanlar),
                        },
                    )]),
                    None => alanlar,
                };
                return Some(Yanit {
                    kaynak: "JsonResource".into(),
                    kesin: eksiksiz && !sema_alanlari.is_empty(),
                    durum,
                    sema: Sema {
                        dto: sinif.ad.clone(),
                        dosya: Some(self.goreli(oge.0)),
                        bulunamadi: sema_alanlari.is_empty(),
                        kalitim: sinif.taban.clone(),
                        alanlar: sema_alanlari,
                        adaylar: vec![],
                        dizi: dizi && sarma.is_none(),
                    },
                });
            }
            // response()->json([...], 201)
            if let Some(c) = h.iter().find(|c| c.ad == "json" && matches!(&c.alici, Some(Ifade::Cagri(r)) if r.ad == "response" && r.alici.is_none())) {
                let Some(Ifade::Dizi(v, _)) = c.arglar.first().map(|a| &a.deger) else { continue };
                let alanlar: BTreeMap<String, Alan> = v
                    .iter()
                    .filter_map(|(a, e)| match a {
                        Some(Ifade::Metin(a)) => Some((
                            a.clone(),
                            Alan {
                                tip: match e {
                                    Ifade::Metin(_) => Some("string".into()),
                                    Ifade::Sayi(_) => Some("number".into()),
                                    Ifade::Ad(b) if b == "true" || b == "false" => Some("boolean".into()),
                                    Ifade::Dizi(..) => Some("object".into()),
                                    _ => None,
                                },
                                zorunlu: true,
                                belirsiz: false,
                                kurallar: vec![],
                                satir: c.satir,
                                kalitimdan: None,
                                degerler: vec![],
                                ic: None,
                            },
                        )),
                        _ => None,
                    })
                    .collect();
                if alanlar.is_empty() {
                    continue;
                }
                let durum = c.arglar.get(1).and_then(|a| php_kod(&ifade_metni(&a.deger)));
                return Some(Yanit {
                    kaynak: "response()->json".into(),
                    kesin: false,
                    durum,
                    sema: Sema { dto: "json".into(), dosya: Some(self.goreli(di)), bulunamadi: false, kalitim: None, alanlar, adaylar: vec![], dizi: false },
                });
            }
        }
        None
    }

    /// Controller'ın ara katmanları (metoda göre süzülmüş): yapıcıda `$this->middleware('auth')->only([...])`,
    /// `HasMiddleware::middleware()` dönüşü (`new Middleware('auth', only: [...])`), `authorizeResource(Post::class)`
    fn kontrolcu_arasi(&self, y: Yer, metot: &str) -> Vec<(Ara, String)> {
        let mut v = vec![];
        let uygun = |sadece: &[String], haric: &[String]| {
            (sadece.is_empty() || sadece.iter().any(|s| s.eq_ignore_ascii_case(metot))) && !haric.iter().any(|s| s.eq_ignore_ascii_case(metot))
        };
        if let Some((u, m)) = self.metot(y, "__construct") {
            for x in &m.govde {
                let (kok, h) = halkalar(x);
                if !matches!(kok, Some(Ifade::Degisken(t)) if t == "this") {
                    continue;
                }
                let Some(ilk) = h.first() else { continue };
                let mut sadece: Vec<String> = h.iter().filter(|c| c.ad == "only").flat_map(|c| arg_metinleri(c)).collect();
                let mut haric: Vec<String> = h.iter().filter(|c| c.ad == "except").flat_map(|c| arg_metinleri(c)).collect();
                if let Some(Ifade::Dizi(secenek, _)) = ilk.arglar.get(1).map(|a| &a.deger) {
                    for (a, e) in secenek {
                        match a {
                            Some(Ifade::Metin(a)) if a == "only" => sadece.extend(metinler(e)),
                            Some(Ifade::Metin(a)) if a == "except" => haric.extend(metinler(e)),
                            _ => {}
                        }
                    }
                }
                let kanit = format!("{}:{}", self.goreli(u.0), ilk.satir);
                match ilk.ad.as_str() {
                    "middleware" if uygun(&sadece, &haric) => v.extend(ilk.arglar.iter().take(1).flat_map(|a| self.ara_girisleri(u.0, &a.deger)).map(|a| (a, kanit.clone()))),
                    "authorizeResource" => {
                        let yetenek = match metot {
                            "index" => "viewAny",
                            "show" => "view",
                            "create" | "store" => "create",
                            "edit" | "update" => "update",
                            "destroy" => "delete",
                            _ => continue,
                        };
                        let model = match ilk.arglar.first().map(|a| &a.deger) {
                            Some(Ifade::Sinif(s)) => kisa_ad(s).to_string(),
                            _ => String::new(),
                        };
                        v.push((Ara::Takma(format!("can:{yetenek},{model}")), kanit));
                    }
                    _ => {}
                }
            }
        }
        if let Some((u, m)) = self.metot(y, "middleware").filter(|(_, m)| m.statik) {
            let kanit = format!("{}:{}", self.goreli(u.0), m.satir);
            for x in &m.govde {
                let Ifade::Don(d) = x else { continue };
                let Ifade::Dizi(liste, _) = d.as_ref() else { continue };
                for (_, e) in liste {
                    match e {
                        Ifade::Yeni(s, a) if kisa_ad(s) == "Middleware" => {
                            let sadece = a.iter().find(|x| x.ad.as_deref() == Some("only")).map(|x| metinler(&x.deger)).unwrap_or_default();
                            let haric = a.iter().find(|x| x.ad.as_deref() == Some("except")).map(|x| metinler(&x.deger)).unwrap_or_default();
                            if uygun(&sadece, &haric) {
                                if let Some(ilk) = a.iter().find(|x| x.ad.is_none() || x.ad.as_deref() == Some("middleware")) {
                                    v.extend(self.ara_girisleri(u.0, &ilk.deger).into_iter().map(|a| (a, kanit.clone())));
                                    if matches!(ilk.deger, Ifade::Kapanis(_)) {
                                        v.push((Ara::Takma("kapanış".into()), kanit.clone()));
                                    }
                                }
                            }
                        }
                        Ifade::Kapanis(_) => v.push((Ara::Takma("kapanış".into()), kanit.clone())),
                        e => v.extend(self.ara_girisleri(u.0, e).into_iter().map(|a| (a, kanit.clone()))),
                    }
                }
            }
        }
        v
    }

    /// Gate / politika yeteneğinin gövdesinden okunan roller: `hasRole('admin')`, `hasAnyRole([...])`,
    /// `hasPermissionTo('x')`, `isAdmin()`, `->role === 'admin'`
    fn yetki_rolleri(&self, yetenek: &str, model: Option<&str>) -> Vec<String> {
        let mut govdeler: Vec<&[Ifade]> = vec![];
        let politikalar: Vec<Yer> = model.filter(|m| !m.is_empty()).map(|m| self.politikalar(m)).unwrap_or_default();
        for (di, d) in self.dosyalar.iter().enumerate() {
            for (ci, s) in d.siniflar.iter().enumerate() {
                // Politika: $policies / Gate::policy kaydı ya da ad kuralı ({Model}Policy)
                if model.is_some_and(|m| !m.is_empty()) && politikalar.contains(&(di, ci)) {
                    if let Some((_, mt)) = self.metot((di, ci), yetenek) {
                        govdeler.push(&mt.govde);
                    }
                }
                // Gate::define('yetenek', fn ($user) => ...)
                for mt in &s.metotlar {
                    for x in &mt.govde {
                        let (kok, h) = halkalar(x);
                        let Some(c) = h.first() else { continue };
                        if !(matches!(kok, Some(Ifade::Ad(a)) if kisa_ad(a) == "Gate") && c.ad == "define") {
                            continue;
                        }
                        if !matches!(arg(c, "ability", 0), Some(Ifade::Metin(a)) if a == yetenek) {
                            continue;
                        }
                        match arg(c, "callback", 1) {
                            Some(Ifade::Kapanis(k)) => govdeler.push(&k.govde),
                            Some(Ifade::Dizi(v, _)) => {
                                if let [(None, Ifade::Sinif(s)), (None, Ifade::Metin(m))] = v.as_slice() {
                                    if let Some((_, mt)) = self.sinif(di, s).and_then(|y| self.metot(y, m)) {
                                        govdeler.push(&mt.govde);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        let mut roller = BTreeSet::new();
        let desen = crate::kural::derle(r#"role\s*(?:===?|==)\s*['"]([\w.-]+)['"]"#);
        for g in govdeler {
            for x in g {
                ifade_gez(
                    x,
                    &mut |y| match y {
                        Ifade::Cagri(c) if matches!(c.ad.as_str(), "hasRole" | "hasAnyRole" | "hasAllRoles" | "hasPermissionTo" | "hasAnyPermission" | "hasPermission") => {
                            roller.extend(c.arglar.iter().flat_map(|a| metinler(&a.deger)).flat_map(|r| r.split('|').map(String::from).collect::<Vec<_>>()));
                        }
                        Ifade::Cagri(c) if c.ad == "isAdmin" => {
                            roller.insert("admin".to_string());
                        }
                        Ifade::Ozellik(_, p) if p == "is_admin" => {
                            roller.insert("admin".to_string());
                        }
                        Ifade::Diger(t, _) => {
                            for c in desen.captures_iter(t) {
                                roller.insert(c[1].to_string());
                            }
                        }
                        _ => {}
                    },
                    0,
                );
            }
        }
        roller.into_iter().collect()
    }
}

impl Baglam<'_> {
    /// Modelin politika sınıfları: `$policies` dizisi, `Gate::policy(M::class, P::class)` ve ad kuralı ({Model}Policy)
    fn politikalar(&self, model: &str) -> Vec<Yer> {
        let mut v = vec![];
        for (di, d) in self.dosyalar.iter().enumerate() {
            for (ci, s) in d.siniflar.iter().enumerate() {
                if s.ad.eq_ignore_ascii_case(&format!("{model}Policy")) {
                    v.push((di, ci));
                }
                if let Some(Ifade::Dizi(p, _)) = s.ozellikler.iter().find(|o| o.ad == "policies").and_then(|o| o.deger.as_ref()) {
                    for (a, e) in p {
                        if let (Some(Ifade::Sinif(m)), Ifade::Sinif(pc)) = (a, e) {
                            if kisa_ad(m) == model {
                                v.extend(self.sinif(di, pc));
                            }
                        }
                    }
                }
                for mt in &s.metotlar {
                    for x in &mt.govde {
                        let (kok, h) = halkalar(x);
                        if let (Some(Ifade::Ad(g)), Some(c)) = (kok, h.first()) {
                            if kisa_ad(g) == "Gate" && c.ad == "policy" {
                                if let (Some(Ifade::Sinif(m)), Some(Ifade::Sinif(pc))) = (arg(c, "class", 0), arg(c, "policy", 1)) {
                                    if kisa_ad(m) == model {
                                        v.extend(self.sinif(di, pc));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        v.sort();
        v.dedup();
        v
    }
}

/// İfadenin kısa kaynak metni (kod ifadeleri için)
fn ifade_metni(x: &Ifade) -> String {
    match x {
        Ifade::Metin(t) | Ifade::Sayi(t) | Ifade::Ad(t) => t.clone(),
        Ifade::Sinif(s) => format!("{s}::class"),
        Ifade::Degisken(v) => format!("${v}"),
        Ifade::Dizi(v, _) => {
            v.iter().map(|(a, e)| format!("{}{}", a.as_ref().map(|a| format!("{} => ", ifade_metni(a))).unwrap_or_default(), ifade_metni(e))).collect::<Vec<_>>().join(", ")
        }
        Ifade::Diger(t, _) => t.clone(),
        _ => String::new(),
    }
}

// ---- Adaptör ----

/// Laravel yolu -> Sezgi yolu: "/posts/{post}" -> "/posts/:post", "{id?}" -> ":id", "{post:slug}" -> ":post". Parçanın
/// içindeki isteğe bağlı parametre (`ping{format?}`) atılır: yol onsuz da eşleşir
fn sezgi_yolu(uri: &str) -> String {
    let y = uri.split('/').map(|p| match p.strip_prefix('{').and_then(|x| x.strip_suffix('}')).filter(|a| !a.contains('{')) {
        Some(a) => format!(":{}", param_adi(a)),
        None => ic_parametresiz(p),
    });
    let y = y.collect::<Vec<_>>().join("/");
    if y.is_empty() {
        "/".into()
    } else {
        y
    }
}

/// `{post:slug?}` -> "post"
fn param_adi(a: &str) -> String {
    a.trim_end_matches('?').split(':').next().unwrap_or(a).to_string()
}

/// Parça içindeki isteğe bağlı parametreler atılır: "ping{format?}" -> "ping"
fn ic_parametresiz(p: &str) -> String {
    let mut o = String::new();
    let mut kalan = p;
    while let Some(i) = kalan.find('{') {
        let Some(j) = kalan[i..].find('}') else { break };
        o.push_str(&kalan[..i]);
        if !kalan[i..i + j].ends_with('?') {
            o.push_str(&kalan[i..=i + j]);
        }
        kalan = &kalan[i + j + 1..];
    }
    o.push_str(kalan);
    o
}

fn yol_parametreleri(uri: &str) -> Vec<String> {
    uri.split('/').filter_map(|p| p.strip_prefix('{').and_then(|x| x.strip_suffix('}'))).map(param_adi).collect()
}

/// Rota dosyası mı: `routes/` (ya da modüllerdeki `Routes/`) klasöründe ve kanal / konsol tanımı değil
fn rota_dosyasi_mi(goreli: &str) -> bool {
    let k = goreli.to_lowercase();
    (k.starts_with("routes/") || k.contains("/routes/")) && !k.ends_with("/channels.php") && !k.ends_with("/console.php")
}

impl<'a> Baglam<'a> {
    /// Olay ve iş düğümleri: `$listen` dizisi, `Event::listen(...)`, `app/Listeners` keşfi (handle metodunun olay tipi) ve
    /// kuyruk işleri (`handle()` metodu olan Job / ShouldQueue sınıfları)
    fn olaylar(&self) -> BTreeMap<String, Value> {
        let mut dinleyiciler: Vec<(&'static str, String, Yer, &'a Metot)> = vec![];
        let ekle = |v: &mut Vec<(&'static str, String, Yer, &'a Metot)>, tur: &'static str, ad: String, y: Yer, m: &'a Metot| {
            if !v.iter().any(|x| x.0 == tur && x.1 == ad && x.2 == y && std::ptr::eq(x.3, m)) {
                v.push((tur, ad, y, m));
            }
        };
        for (di, d) in self.dosyalar.iter().enumerate() {
            for (ci, s) in d.siniflar.iter().enumerate() {
                let y = (di, ci);
                // $listen = [Olay::class => [Dinleyici::class, 'Dinleyici@metot']]
                if let Some(Ifade::Dizi(v, _)) = s.ozellikler.iter().find(|o| o.ad == "listen").and_then(|o| o.deger.as_ref()) {
                    for (a, e) in v {
                        let Some(Ifade::Sinif(olay)) = a else { continue };
                        let liste: Vec<&Ifade> = match e {
                            Ifade::Dizi(l, _) => l.iter().map(|(_, x)| x).collect(),
                            x => vec![x],
                        };
                        for x in liste {
                            let (l, m) = match x {
                                Ifade::Sinif(l) => (l.clone(), "handle".to_string()),
                                Ifade::Metin(t) => match t.split_once('@') {
                                    Some((l, m)) => (l.to_string(), m.to_string()),
                                    None => (t.clone(), "handle".into()),
                                },
                                _ => continue,
                            };
                            if let Some((u, mt)) = self.sinif(di, &l).and_then(|ly| self.metot(ly, &m).or_else(|| self.metot(ly, "__invoke"))) {
                                ekle(&mut dinleyiciler, "event", kisa_ad(olay).to_string(), u, mt);
                            }
                        }
                    }
                }
                // Event::listen(Olay::class, Dinleyici::class | [Dinleyici::class, 'metot'])
                for mt in &s.metotlar {
                    for x in &mt.govde {
                        let (kok, h) = halkalar(x);
                        let (Some(Ifade::Ad(e)), Some(c)) = (kok, h.first()) else { continue };
                        if kisa_ad(e) != "Event" || c.ad != "listen" {
                            continue;
                        }
                        let Some(Ifade::Sinif(olay)) = c.arglar.first().map(|a| &a.deger) else { continue };
                        let hedef = match c.arglar.get(1).map(|a| &a.deger) {
                            Some(Ifade::Sinif(l)) => self.sinif(di, l).and_then(|ly| self.metot(ly, "handle").or_else(|| self.metot(ly, "__invoke"))),
                            Some(Ifade::Dizi(v, _)) => match v.as_slice() {
                                [(None, Ifade::Sinif(l)), (None, Ifade::Metin(m))] => self.sinif(di, l).and_then(|ly| self.metot(ly, m)),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some((u, m)) = hedef {
                            ekle(&mut dinleyiciler, "event", kisa_ad(olay).to_string(), u, m);
                        }
                    }
                }
                if s.tur != "class" {
                    continue;
                }
                let tam = self.tam(y);
                // Keşif: App\Listeners altındaki handle*/__invoke metodunun ilk parametresinin tipi olaydır
                if tam.contains("\\Listeners\\") {
                    for m in s.metotlar.iter().filter(|m| m.ad.starts_with("handle") || m.ad == "__invoke") {
                        if let Some(t) = m.parametreler.first().and_then(|p| p.tip.as_deref()) {
                            ekle(&mut dinleyiciler, "event", kisa_ad(&self.tam_ad(di, t)).to_string(), y, m);
                        }
                    }
                }
                // Kuyruk işleri
                if (tam.contains("\\Jobs\\") || self.uygular(y, "ShouldQueue") || self.trait_tam(y, "Dispatchable").is_some_and(|t| t.contains("Bus")))
                    && !tam.contains("\\Listeners\\")
                    && self.olay_turu(y) == "job"
                {
                    if let Some(m) = s.metotlar.iter().find(|m| m.ad == "handle") {
                        ekle(&mut dinleyiciler, "job", s.ad.clone(), y, m);
                    }
                }
            }
        }
        let mut v: BTreeMap<String, Value> = BTreeMap::new();
        for (tur, ad, y, m) in dinleyiciler {
            let (ozet, _) = self.zincir(Govde::Metot(y, m), &[], &BTreeSet::new());
            let dugum = v.entry(format!("{tur}:{ad}")).or_insert_with(|| json!({ "tur": tur, "ad": ad, "dinleyiciler": [], "tetikler": [], "silinen": [] }));
            if let Some(l) = dugum["dinleyiciler"].as_array_mut() {
                l.push(json!({ "fonksiyon": format!("{}.{}", self.sinif_(y).ad, m.ad), "kanit": format!("{}:{}", self.goreli(y.0), m.satir) }));
            }
            for alan in ["tetikler", "silinen"] {
                let ekler = ozet[alan].as_array().cloned().unwrap_or_default();
                if let Some(l) = dugum[alan].as_array_mut() {
                    for e in ekler {
                        if !l.contains(&e) {
                            l.push(e);
                        }
                    }
                }
            }
        }
        v
    }

    /// Tablo modelleri: Eloquent modelleri ($table ya da varsayılan tablo adı; alanlar `$fillable` ve `$casts`'tan) ve
    /// göçlerdeki tablolar (`Schema::create` sütunları)
    fn veri_modelleri(&self) -> Value {
        let mut v = vec![];
        let goc_tablolari: HashSet<String> = self.dosyalar.iter().flat_map(|d| &d.tablolar).filter(|t| t.olustur).map(|t| t.tablo.to_lowercase()).collect();
        let kaynak = |m: &Model| if m.tablo_acik { "$table" } else { "varsayılan (sınıf adının snake_case çoğulu)" };
        for (ad, m) in &self.modeller {
            let s = self.sinif_(m.yer);
            let mut alanlar: Vec<Value> = vec![];
            for o in s.ozellikler.iter().filter(|o| (o.ad == "fillable" || o.ad == "casts") && !goc_tablolari.contains(&m.tablo.to_lowercase())) {
                if let Some(Ifade::Dizi(d, _)) = &o.deger {
                    for (a, e) in d {
                        let alan = match (a, e) {
                            (Some(Ifade::Metin(a)), _) => a.clone(),
                            (None, Ifade::Metin(e)) => e.clone(),
                            _ => continue,
                        };
                        if !alanlar.iter().any(|x| x["ad"] == alan.as_str()) {
                            alanlar.push(json!({ "ad": alan, "tip": null, "satir": o.satir }));
                        }
                    }
                }
            }
            v.push(json!({
                "ad": ad, "tablo": m.tablo,
                "tablo_kaynagi": kaynak(m),
                "dosya": self.goreli(m.yer.0), "satir": s.satir, "alanlar": alanlar,
            }));
        }
        for (di, d) in self.dosyalar.iter().enumerate() {
            for t in &d.tablolar {
                // Modeli olan tablonun adı ve kaynağı modelinkiyle aynı yazılır (iki kanıt aynı önermeyi destekler)
                let model = self.tablolar.get(&t.tablo.to_lowercase()).and_then(|a| self.modeller.get(a).map(|m| (a.clone(), kaynak(m))));
                let (ad, tablo_kaynagi) = model.unwrap_or_else(|| (t.tablo.clone(), if t.olustur { "göç (Schema::create)" } else { "göç (Schema::table)" }));
                let alanlar: Vec<Value> = t.sutunlar.iter().map(|(a, tip, s)| json!({ "ad": a, "tip": tip, "satir": s })).collect();
                v.push(json!({ "ad": ad, "tablo": t.tablo, "tablo_kaynagi": tablo_kaynagi, "dosya": self.goreli(di), "satir": t.satir, "alanlar": alanlar }));
            }
        }
        json!(v)
    }

    /// Göçlerdeki kaskad silmeler: [ebeveyn, çocuk, "dosya:satır"] (tablo adları modele çevrilir)
    fn kaskadlar(&self) -> Vec<Value> {
        let model_ya_da_tablo = |t: &str| self.tablolar.get(&t.to_lowercase()).cloned().unwrap_or_else(|| t.to_string());
        let mut v = vec![];
        for (di, d) in self.dosyalar.iter().enumerate() {
            for t in &d.tablolar {
                for y in t.yabancilar.iter().filter(|y| y.kaskad) {
                    let Some(e) = &y.ebeveyn else { continue };
                    let ebeveyn = match e.strip_prefix("sinif:") {
                        Some(s) => {
                            let k = kisa_ad(s).to_string();
                            if self.modeller.contains_key(&k) {
                                k
                            } else {
                                model_ya_da_tablo(&varsayilan_tablo(&k))
                            }
                        }
                        None => model_ya_da_tablo(e),
                    };
                    let cocuk = model_ya_da_tablo(&t.tablo);
                    if !ebeveyn.eq_ignore_ascii_case(&cocuk) {
                        v.push(json!([ebeveyn, cocuk, format!("{}:{}", self.goreli(di), y.satir)]));
                    }
                }
            }
        }
        v
    }
}

/// Çatının yetki denetimi mi; öyleyse reddeder mi: `$this->authorize(...)` ve `Gate::authorize(...)` istisna atar,
/// `->can()` ve `Gate::allows / denies / check` yalnızca sonuç döndürür (kararı çağıran kod verir)
fn cati_yetkisi(c: &Cagri) -> Option<bool> {
    match (&c.alici, c.statik) {
        (Some(Ifade::Degisken(t)), false) if t == "this" && c.ad == "authorize" => Some(true),
        (Some(Ifade::Ad(s)), true) if kisa_ad(s) == "Gate" && c.ad == "authorize" => Some(true),
        (Some(Ifade::Ad(s)), true) if kisa_ad(s) == "Gate" && matches!(c.ad.as_str(), "allows" | "denies" | "check") => Some(false),
        (Some(_), false) if matches!(c.ad.as_str(), "can" | "cannot") => Some(false),
        _ => None,
    }
}

impl<'a> Baglam<'a> {
    /// Çağrının projedeki tanımı: `$this->m(...)` (sınıf ve ataları) ya da dosya düzeyindeki fonksiyon `f(...)`
    fn yardimci(&self, sinif: Option<Yer>, c: &Cagri) -> Option<(Option<Yer>, &'a Metot)> {
        match &c.alici {
            Some(Ifade::Degisken(t)) if t == "this" && !c.statik => sinif.and_then(|y| self.metot(y, &c.ad)).map(|(u, m)| (Some(u), m)),
            None => {
                let ad = c.ad.rsplit('\\').next().unwrap_or(&c.ad);
                self.dosyalar.iter().flat_map(|d| d.fonksiyonlar.iter()).find(|f| f.ad.eq_ignore_ascii_case(ad)).map(|f| (None, f))
            }
            _ => None,
        }
    }

    /// Projenin izin yardımcısı: gövde parametrelerinden birini çatının yetki denetimine ya da başka bir izin yardımcısına
    /// veriyorsa o parametrenin sırası (`checkPermission($permission)` -> `user()->can($permission)`; en çok 3 kat)
    fn izin_parametresi(&self, sinif: Option<Yer>, m: &Metot, d: usize) -> Option<usize> {
        let sira = |x: Option<&Ifade>| match x {
            Some(Ifade::Degisken(v)) => m.parametreler.iter().position(|p| p.ad == *v),
            _ => None,
        };
        kosullu_dugumler(&m.govde).into_iter().find_map(|y| {
            let Ifade::Cagri(c) = y else { return None };
            if cati_yetkisi(c).is_some() {
                sira(c.arglar.first().map(|a| &a.deger))
            } else if d < 3 {
                self.yardimci(sinif, c).filter(|(_, mm)| !std::ptr::eq(*mm, m)).and_then(|(u, mm)| {
                    let i = self.izin_parametresi(u, mm, d + 1)?;
                    sira(arg(c, &mm.parametreler[i].ad, i))
                })
            } else {
                None
            }
        })
    }
}

/// Gövdenin bütün düğümleri (kapanışlar dahil), `if` / `elseif` koşul ifadeleriyle birlikte: yetki denetimi çoğu
/// zaman koşuldadır (`if (!user()->can($izin)) abort(403);`)
fn kosullu_dugumler(govde: &[Ifade]) -> Vec<&Ifade> {
    let mut v = vec![];
    let mut kosullar = vec![];
    for x in govde {
        ifade_gez(
            x,
            &mut |y| {
                v.push(y);
                if let Ifade::Kosul(_, _, _, Some(k)) = y {
                    kosullar.push(k.as_ref());
                }
            },
            0,
        );
    }
    for k in kosullar {
        ifade_gez(k, &mut |y| v.push(y), 0);
    }
    v
}

/// İşleyicinin yetki çağrıları: `$this->authorize('yetenek', $model)`, `Gate::authorize(...)`, `->can('yetenek', X::class)`
/// ve projenin izin yardımcıları (`$this->checkPermission(Permission::SettingsManage)`, `userCan('x')`); yetenek adı
/// metin ya da projedeki enum durumu / sınıf sabiti olabilir. Dönen: (yetenek, model adı, kesin mi). Kesin yalnızca
/// gövdenin üst düzeyinde (koşulsuz) reddeden çatı denetimidir; sonuç döndüren denetim, koşul altındaki çağrı ve
/// kararı proje kodunda olan yardımcı yorumdur
fn yetki_cagrilari(b: &Baglam, g: Govde) -> Vec<(String, Option<String>, bool)> {
    let vars = b.degiskenler(g);
    let mut v = vec![];
    for y in kosullu_dugumler(g.ifadeler()) {
        let Ifade::Cagri(c) = y else { continue };
        match cati_yetkisi(c) {
            Some(reddeder) => {
                let Some(a) = c.arglar.first().and_then(|a| b.izin_adi(g.di(), &a.deger)) else { continue };
                let model = match c.arglar.get(1).map(|a| &a.deger) {
                    Some(Ifade::Sinif(s)) => Some(kisa_ad(s).to_string()),
                    Some(Ifade::Dizi(d, _)) => d.iter().find_map(|(_, e)| if let Ifade::Sinif(s) = e { Some(kisa_ad(s).to_string()) } else { None }),
                    Some(x) => b.ifade_sinifi(g.di(), g.ctx(), &vars, x, 0).and_then(|y| b.model_adi(y)),
                    None => None,
                };
                // Gövdenin üst düzeyindeki deyim koşulsuz çalışır
                let ust = g.ifadeler().iter().any(|x| std::ptr::eq(x, y));
                v.push((a, model, reddeder && ust));
            }
            None => {
                let Some((u, m)) = b.yardimci(g.ctx(), c) else { continue };
                let Some(i) = b.izin_parametresi(u, m, 0) else { continue };
                if let Some(a) = arg(c, &m.parametreler[i].ad, i).and_then(|x| b.izin_adi(g.di(), x)) {
                    v.push((a, None, false));
                }
            }
        }
    }
    v
}

/// Rotanın işleyicisi: (gövde, kontrolcü, fonksiyon, dosya, satır, sınıf, özet)
type Isleyici<'a> = (Option<Govde<'a>>, String, Option<String>, String, usize, Option<Yer>, Option<String>);

pub fn kapilar(kok: &Path, dosyalar: &[Dosya]) -> (Vec<Kapi>, Value) {
    let b = Baglam::yeni(kok, dosyalar);
    let ortam = b.ara_ortami();
    let mut m = RotaMotoru::yeni(&b);
    let bos = Degerler::new();
    // 1) Yükleyiciler: bootstrap/app.php withRouting(...) ve servis sağlayıcıları (RouteServiceProvider boot()/map(),
    //    loadRoutesFrom)
    for (di, d) in dosyalar.iter().enumerate() {
        if rota_dosyasi_mi(&b.goreli(di)) {
            continue;
        }
        if govde_ara(&d.ifadeler, &mut |c| c.ad == "withRouting") {
            m.deyimler(di, None, &d.ifadeler, &Grup::default(), &bos, 0);
        }
    }
    for (di, d) in dosyalar.iter().enumerate() {
        for ci in 0..d.siniflar.len() {
            let y = (di, ci);
            if !b.turer(y, &["Illuminate\\Support\\ServiceProvider", "Illuminate\\Foundation\\Support\\Providers\\RouteServiceProvider"]) {
                continue;
            }
            for ad in ["boot", "map"] {
                if b.sinif_(y).metotlar.iter().any(|x| x.ad.eq_ignore_ascii_case(ad)) {
                    m.metot_degerlendir(di, Some(y), y, ad, None, &Grup::default(), &bos, 0);
                }
            }
        }
    }
    // 2) Yükleyicisi bulunamayan rota dosyaları: hiç yükleyici yoksa Laravel'in varsayılanı (web.php öneksiz, api.php
    //    /api öneki ve api grubu); varsa bağlanmamış dosyanın kapıları ikincildir
    let yukleyici_var = m.yukleyici.is_some() || !m.yuklemeler.is_empty();
    let mut sirali: Vec<usize> = (0..dosyalar.len()).filter(|di| rota_dosyasi_mi(&b.goreli(*di))).collect();
    sirali.sort_by_key(|di| (!matches!(b.goreli(*di).as_str(), "routes/web.php" | "routes/api.php"), b.goreli(*di)));
    for di in sirali {
        if m.yuklemeler.iter().any(|y| y.0 == di) {
            continue;
        }
        let g = b.goreli(di);
        let mut grup = Grup { dosya: Some(di), ..Grup::default() };
        match (yukleyici_var, g.as_str()) {
            (false, "routes/web.php") => grup.ara.push((Ara::Takma("web".into()), format!("{g}:1"))),
            (false, "routes/api.php") => {
                grup.onek = vec!["api".into()];
                grup.ara.push((Ara::Takma("api".into()), format!("{g}:1")));
                m.api_oneki.get_or_insert("/api".into());
            }
            _ => grup.ana = false,
        }
        m.yigin.push(di);
        m.deyimler(di, None, &dosyalar[di].ifadeler, &grup, &bos, 0);
        m.yigin.pop();
    }
    // Aynı yöntem ve yol iki kez kaydedildiyse sonraki geçerlidir (Laravel'in rota koleksiyonu)
    let mut son: HashMap<(String, String), usize> = HashMap::new();
    for (i, r) in m.rotalar.iter().enumerate() {
        for y in &r.yontemler {
            son.insert((y.clone(), sezgi_yolu(&r.uri)), i);
        }
    }
    let mut kapilar = vec![];
    let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
    let mut kosullu_kapilar = vec![];
    let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
    let mut islem_kimlikleri: BTreeMap<String, Option<String>> = BTreeMap::new();
    for (i, r) in m.rotalar.iter().enumerate() {
        let yol = sezgi_yolu(&r.uri);
        let yontemler: Vec<&String> = r.yontemler.iter().filter(|y| son.get(&((*y).clone(), yol.clone())) == Some(&i)).collect();
        if yontemler.is_empty() {
            continue;
        }
        let rota_dosyasi = b.goreli(r.grup.dosya.unwrap_or(r.di));
        let dosya_kok = pascal(rota_dosyasi.rsplit('/').next().unwrap_or("").trim_end_matches(".php"));
        let yol_adi = |y: &str| format!("{}{}", y.to_lowercase(), pascal(&parcalar(&r.uri).iter().filter(|p| !p.starts_with('{')).cloned().collect::<Vec<_>>().join(" ")));
        // İşleyici: controller metodu ya da kapanış
        let (isleyici, kontrolcu, fonksiyon, dosya, satir, sinif_yeri, ozet): Isleyici = match &r.eylem {
            Eylem::Metot { di, sinif, tam, metot } => {
                let y = if *tam { b.tam_sinif(sinif).or_else(|| b.sinif(*di, kisa_ad(sinif))) } else { b.sinif(*di, sinif) };
                match y {
                    Some(y) => match b.metot(y, metot) {
                        Some((u, mm)) => (Some(Govde::Metot(u, mm)), b.sinif_(y).ad.clone(), Some(mm.ad.clone()), b.goreli(u.0), mm.satir, Some(y), mm.ozet.clone()),
                        None => (None, b.sinif_(y).ad.clone(), Some(metot.clone()), b.goreli(y.0), b.sinif_(y).satir, Some(y), None),
                    },
                    None => (None, kisa_ad(sinif).to_string(), Some(metot.clone()), b.goreli(r.di), r.satir, None, None),
                }
            }
            Eylem::Kapanis(di, k) => (Some(Govde::Kapanis(*di, k)), dosya_kok.clone(), None, b.goreli(*di), k.satir, None, None),
            Eylem::Gorunum => (None, "View".into(), None, b.goreli(r.di), r.satir, None, None),
            Eylem::Saglik => (None, "Health".into(), Some("up".into()), b.goreli(r.di), r.satir, None, None),
        };
        let konum = format!("{dosya}:{satir}");
        // Ara katmanlar: grup + rota + controller; genel ara katmanlar ayrıca
        let mut liste = r.grup.ara.clone();
        if let (Some(y), Eylem::Metot { metot, .. }) = (sinif_yeri, &r.eylem) {
            liste.extend(b.kontrolcu_arasi(y, metot));
        }
        let acik = b.ara_ac(&ortam, &liste, &r.grup.haric);
        let genel: Vec<(Ara, String)> = ortam.genel.iter().map(|a| (a.clone(), String::new())).collect();
        let genel = b.ara_ac(&ortam, &genel, &r.grup.haric);
        let (mut kesin, mut tahmin, mut bilinmeyen) = (false, false, false);
        let mut roller: BTreeSet<String> = BTreeSet::new();
        let mut roller_kesin = true;
        // (yetenek, model, kesin mi)
        let mut yetkiler: Vec<(String, Option<String>, bool)> = vec![];
        let mut bagimliliklar: Vec<Govde> = vec![];
        for (j, (a, _)) in acik.iter().chain(genel.iter()).enumerate() {
            let (tur, yer) = b.ara_turu(&ortam, a);
            // Rota düzeyindeki proje ara katmanlarının gövdesi davranış zincirine (bağımlılık) girer
            if j < acik.len() {
                if let Some((u, mm)) = yer.and_then(|y| b.metot(y, "handle")) {
                    bagimliliklar.push(Govde::Metot(u, mm));
                }
            }
            match tur {
                AraTur::Kimlik(true) => kesin = true,
                AraTur::Kimlik(false) => tahmin = true,
                AraTur::Rol(r, k) => {
                    if k {
                        kesin = true;
                    } else {
                        tahmin = true;
                        roller_kesin = false;
                    }
                    roller.extend(r);
                }
                AraTur::Yetki(p, k) => {
                    tahmin = true;
                    let mut x = p.split(',');
                    let yetenek = x.next().unwrap_or("").to_string();
                    let model = x.next().map(|m| {
                        // can:update,post -> rota parametresinin bağlandığı model ya da adı
                        let m = m.trim();
                        isleyici
                            .and_then(|g| g.parametreler().iter().find(|p| p.ad == m).and_then(|p| p.tip.as_deref()).and_then(|t| b.sinif(g.di(), t)).and_then(|y| b.model_adi(y)))
                            .unwrap_or_else(|| if m.contains('\\') { kisa_ad(m).to_string() } else { pascal(m) })
                    });
                    yetkiler.push((yetenek, model, k));
                }
                AraTur::Misafir | AraTur::Notr => {}
                AraTur::Bilinmeyen => bilinmeyen = true,
            }
        }
        // Girdiler
        let girdi = b.girdi(isleyici);
        if let Some(g) = isleyici {
            let cagrilar = yetki_cagrilari(&b, g);
            if !cagrilar.is_empty() {
                tahmin = true;
            }
            yetkiler.extend(cagrilar);
            let denetim = match g {
                Govde::Metot(_, mm) => mm.giris_denetimi,
                Govde::Kapanis(_, k) => k.giris_denetimi,
            };
            if denetim {
                tahmin = true;
            }
        }
        if let Some((u, fm)) = girdi.form_istegi.and_then(|y| b.metot(y, "authorize")) {
            if fm.kimlik_izi {
                tahmin = true;
            }
            yetkiler.extend(yetki_cagrilari(&b, Govde::Metot(u, fm)));
        }
        // Gate / politika gövdesinden okunan roller yorumdur. Rolü okunamayan, modelsiz yetenek bir izin adıdır
        // (`can:settings-manage`, `$this->checkPermission(Permission::X)`): rol / izin olarak sayılır; kaynakta yazılı ve
        // çatının denetimiyse kesin, değilse yorum
        for (yetenek, model, k) in &yetkiler {
            let r = b.yetki_rolleri(yetenek, model.as_deref());
            if !r.is_empty() {
                roller_kesin = false;
                roller.extend(r);
            } else if model.is_none() && !yetenek.is_empty() && !yetenek.contains('…') {
                roller_kesin &= *k;
                roller.insert(yetenek.clone());
            }
        }
        let (giris, kanit) = if kesin {
            ("gerekli", "kesin")
        } else if tahmin {
            ("gerekli", if bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" })
        } else if bilinmeyen {
            ("herkese_acik", "varsayim_bilinmeyen_guard")
        } else {
            ("herkese_acik", "varsayim")
        };
        let mut guardlar: Vec<String> = vec![];
        for (a, _) in &liste {
            let t = a.metin();
            if !guardlar.contains(&t) {
                guardlar.push(t);
            }
        }
        let roller: Vec<String> = roller.into_iter().collect();
        // Yol parametreleri: desenden (`where`, `whereNumber`, `Route::pattern`) ya da işleyici parametresinin tipinden;
        // modele bağlanan parametre (route model binding) modelin tablosunu okur
        let mut ek_okuma: BTreeSet<String> = girdi.tablolar.clone();
        let parametreler: Vec<Girdi> = yol_parametreleri(&r.uri)
            .into_iter()
            .map(|p| {
                let desen = r.grup.desenler.iter().chain(m.desenler.iter()).rev().find(|(a, _)| *a == p).map(|(_, d)| d.as_str());
                let mut tip = desen.map(|d| if matches!(d, "[0-9]+" | "\\d+" | "[0-9]*" | "\\d*" | "^[0-9]+$") { "integer" } else { "string" }.to_string());
                if let Some(g) = isleyici {
                    if let Some(t) = g.parametreler().iter().find(|x| x.ad == p).and_then(|x| x.tip.as_deref()) {
                        match b.sinif(g.di(), t).and_then(|y| b.model_adi(y)) {
                            Some(md) => {
                                ek_okuma.insert(md.clone());
                                tip = Some(md);
                            }
                            None if tip.is_none() => {
                                tip = match t {
                                    "int" => Some("integer".into()),
                                    "string" => Some("string".into()),
                                    "float" => Some("number".into()),
                                    _ => None,
                                }
                            }
                            None => {}
                        }
                    }
                }
                Girdi { ad: Some(p), tip, sema: None }
            })
            .collect();
        // Davranış zinciri ve veri bağı
        let (dna, bag) = match isleyici {
            Some(g) => b.zincir(g, &bagimliliklar, &ek_okuma),
            None => (Value::Null, VeriBag { okur: ek_okuma.iter().cloned().collect(), yazar: vec![], eksik_olabilir: false }),
        };
        let veri = (!b.modeller.is_empty() || !bag.okur.is_empty() || !bag.yazar.is_empty() || bag.eksik_olabilir).then_some(bag);
        if !dna.is_null() && !crate::dna::bos(&dna) {
            dna_haritasi.entry(konum.clone()).or_insert(dna);
        }
        let yanit = isleyici.and_then(|g| b.yanit(g));
        if !girdi.notlar.is_empty() {
            let n = denetlenemeyen.entry(konum.clone()).or_default();
            for x in &girdi.notlar {
                if !n.contains(x) {
                    n.push(x.clone());
                }
            }
        }
        kapi_routerlari.entry(konum.clone()).or_insert_with(|| rota_dosyasi.clone());
        if let Some(a) = &r.ad {
            match islem_kimlikleri.get(&konum) {
                Some(Some(e)) if e != a => {
                    islem_kimlikleri.insert(konum.clone(), None);
                }
                Some(_) => {}
                None => {
                    islem_kimlikleri.insert(konum.clone(), Some(a.clone()));
                }
            }
        }
        for y in yontemler {
            let okuma = matches!(y.as_str(), "GET" | "HEAD");
            let id = format!("{y} {yol}");
            if !r.grup.kosullar.is_empty() || r.grup.dongu {
                kosullu_kapilar.push(id.clone());
            }
            let (govde, sorgu) = match &girdi.sema {
                Some(s) => {
                    let g = vec![Girdi { ad: None, tip: Some(s.dto.clone()), sema: Some(s.clone()) }];
                    if okuma {
                        (vec![], g)
                    } else {
                        (g, vec![])
                    }
                }
                None => (vec![], vec![]),
            };
            kapilar.push(Kapi {
                id,
                yontem: y.clone(),
                yol: yol.clone(),
                kontrolcu: kontrolcu.clone(),
                fonksiyon: fonksiyon.clone().unwrap_or_else(|| yol_adi(y)),
                dosya: dosya.clone(),
                satir,
                giris: giris.into(),
                guardlar: guardlar.clone(),
                roller: roller.clone(),
                roller_kaniti: (!roller.is_empty() && !roller_kesin).then(|| "varsayim".into()),
                ozet: ozet.clone(),
                govde,
                sorgu,
                parametreler: parametreler.clone(),
                kullanimdan_kalkmis: false,
                yerine: None,
                surum: None,
                uygulama: (!r.grup.ana).then(|| rota_dosyasi.clone()),
                ana: r.grup.ana,
                giris_kaniti: kanit.into(),
                veri: veri.clone(),
                yanit: yanit.clone(),
            });
        }
    }
    kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
    kosullu_kapilar.sort();
    kosullu_kapilar.dedup();
    // Rota dosyaları grafı: her rota dosyası ve onu yükleyen (bootstrap/app.php, RouteServiceProvider, require)
    let mut routerlar: BTreeMap<String, Value> = BTreeMap::new();
    for (h, y, satir, kosul) in &m.yuklemeler {
        let (hid, yid) = (b.goreli(*h), b.goreli(*y));
        routerlar.entry(yid.clone()).or_insert_with(|| json!({ "id": yid, "dosya": yid, "satir": 1, "ustler": [] }));
        let d = routerlar.entry(hid.clone()).or_insert_with(|| json!({ "id": hid, "dosya": hid, "satir": 1, "ustler": [] }));
        if let Some(l) = d["ustler"].as_array_mut() {
            l.push(json!({ "id": yid, "icerme": format!("{yid}:{satir}"), "kosul": kosul, "kosullu": kosul.is_some() }));
        }
    }
    for v in kapi_routerlari.values() {
        routerlar.entry(v.clone()).or_insert_with(|| json!({ "id": v, "dosya": v, "satir": 1, "ustler": [] }));
    }
    let ana = m.yukleyici.map(|di| b.goreli(di));
    let genel_guardlar: Vec<String> = ortam.genel.iter().map(Ara::metin).collect();
    let bilgi = json!({
        "genel_onek": m.api_oneki.clone().unwrap_or_default(), "genel_guardlar": genel_guardlar, "ana_uygulama": ana, "surum_secimi": null,
        "kosullu_kapilar": kosullu_kapilar, "denetlenemeyen": denetlenemeyen, "dna": dna_haritasi, "kaskadlar": b.kaskadlar(),
        "routerlar": routerlar.into_values().collect::<Vec<_>>(), "kapi_routerlari": kapi_routerlari, "veri_modelleri": b.veri_modelleri(),
        "islem_kimlikleri": islem_kimlikleri.into_iter().filter_map(|(a, k)| k.map(|k| (a, k))).collect::<BTreeMap<_, _>>(),
        "olaylar": b.olaylar(), "dinamik_kayitlar": m.dinamik,
    });
    (kapilar, bilgi)
}

/// Laravel uygulaması mı: composer.json'da laravel/framework (ya da Lumen)
pub fn laravel_mi(kok: &Path) -> bool {
    guvenli::oku(&kok.join("composer.json")).is_some_and(|m| m.contains("\"laravel/framework\"") || m.contains("\"laravel/lumen-framework\""))
}

/// Uygulamanın adı ve sürümü: composer.json
pub fn paket(kok: &Path) -> Value {
    let p: Value = guvenli::oku(&kok.join("composer.json")).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_default();
    json!({ "name": p["name"].as_str(), "version": p["version"].as_str() })
}

/// Keşif, paralel ayrıştırma ve Laravel adaptörü
pub(crate) fn topla(kok: &Path, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<Dosya>>, t0: Instant) -> crate::Toplam {
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
        // Bağımlılıklar, önbellek, testler, görünümler ve dil dosyaları uygulamanın API'si değildir
        let goreli = guvenli::goreli(kok, p);
        let bilesenler: Vec<&str> = goreli.split('/').collect();
        if bilesenler.iter().any(|c| matches!(*c, "vendor" | "node_modules" | "storage" | "tests" | "test" | "lang" | ".git"))
            || goreli.starts_with("bootstrap/cache/")
            || goreli.starts_with("public/")
            || goreli.starts_with("resources/views/")
            || goreli.ends_with(".blade.php")
        {
            atlanan += 1;
            continue;
        }
        if p.extension().and_then(|e| e.to_str()) == Some("php") {
            if g.metadata().is_ok_and(|m| m.len() > crate::EN_BUYUK_DOSYA) {
                buyuk.push(goreli);
                continue;
            }
            yollar.push(p.to_path_buf());
        } else {
            atlanan += 1;
        }
    }
    let t_kesif = t0.elapsed();
    let (sonuclar, okunan, ayristirilan) = crate::kesif::ayristir_hepsi(&yollar, onbellek, |p, b| ayristir_metin(p, &String::from_utf8(b).ok()?));
    let dosyalar: Vec<Dosya> = sonuclar.into_iter().map(|(_, d)| d).collect();
    let t_ayristirma = t0.elapsed();
    let (kapilar, bilgi) = kapilar(kok, &dosyalar);
    let t_toplam = t0.elapsed();
    // Tablo tanımlayan dosyalar: Eloquent modelleri ve göçler
    let mut veri_dosyalari: Vec<String> = bilgi["veri_modelleri"].as_array().into_iter().flatten().filter_map(|m| m["dosya"].as_str().map(String::from)).collect();
    veri_dosyalari.sort();
    veri_dosyalari.dedup();
    crate::Toplam {
        kapilar,
        bilgi,
        adaptor: format!("laravel@{SURUM}"),
        dosya_sayisi: dosyalar.len(),
        satir: dosyalar.iter().map(|d| d.satir_sayisi).sum(),
        kismi: dosyalar.iter().filter(|d| d.hatali).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        atlanan,
        buyuk,
        ek_paketler: vec![],
        veri_dosyalari,
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}
