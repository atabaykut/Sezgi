//! Express, Fastify ve Next.js rota dosyaları (JavaScript / TypeScript) adaptörü, seviye 1.
//!
//! Neyin nereden okunduğu:
//! - **Rotalar (kesin):** `express()`, `express.Router()`, `Router()`, `fastify()` ile oluşturulan nesnelerin
//!   `get/post/put/patch/delete` çağrıları, `route('/x').get(...)` zincirleri; ilk parametresi `app` / `fastify` /
//!   `router` ... olan rota fonksiyonları (`export function kaydet(app) { app.post(...) }`, Fastify eklentileri).
//!   Bağlama: `app.use('/önek', ...ara, router)`, `register(eklenti, { prefix })` ve `kaydet(app)` çağrıları,
//!   dosyalar arası `require` / `import` izlenerek. Next.js: `app/**/route.(ts|js)` dosyalarının `GET/POST/...` dışa
//!   aktarımları; yol dosya konumundan.
//! - **Girdiler:** Fastify rota şeması (`schema.body/querystring`, JSON Schema) tanımdır, kesindir. Express ve Next.js'te
//!   girdi şeması çoğunlukla yoktur: alanlar işleyicinin kullanımından (`const { a, b = 1 } = req.body`, `req.query.x`,
//!   `await request.json()`) okunur; zorunlulukları **Yorum**dur.
//! - **Giriş ve rol (Yorum):** zincirdeki ara katmanlar; adı ya da tanımı (401 / Unauthorized / jwt.verify) yetki
//!   gösteriyorsa "gerekli". Rol yalnızca ara katman çağrısının metin argümanlarından (`requireRole('admin')`).
//! - **Veri bağı (Yorum):** işleyici gövdesindeki SQL metni (`FROM`, `JOIN`, `INSERT INTO`, `UPDATE`, `DELETE FROM`)
//!   ve Prisma çağrıları (`prisma.kullanici.findMany`).
//! - **Kapsam dışı:** `api.get('/x')` gibi istemci çağrıları sayılmaz: alıcı bir Express / Fastify nesnesi değildir.
use crate::adaptor::{Alan, Girdi, Kapi, Sema, VeriBag};
use crate::guvenli;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;
use tree_sitter::{Node, Parser};

pub const SURUM: &str = "0.1.0";
const YONTEMLER: [&str; 5] = ["get", "post", "put", "patch", "delete"];
/// Rota nesnesi olarak kullanılan ilk parametre adları (rota fonksiyonları, Fastify eklentileri)
const EKLENTI_PARAMETRELERI: [&str; 5] = ["fastify", "app", "server", "instance", "router"];

/// package.json Express, Fastify ya da Next.js kullanıyor mu
pub fn destekli_mi(pkg: &str) -> bool {
    ["\"express\"", "\"fastify\"", "\"next\""].iter().any(|b| pkg.contains(b))
}

#[derive(Debug, Clone)]
enum Ara {
    /// Ara katman adı (`authMiddleware`, `auth.required`)
    Ad(String),
    /// Çağrılarak kurulan ara katman (`requireRole('admin')`) ve metin argümanları
    Cagri(String, Vec<String>),
}

impl Ara {
    fn ad(&self) -> &str {
        match self {
            Ara::Ad(a) | Ara::Cagri(a, _) => a,
        }
    }
}

#[derive(Debug, Clone)]
struct Isleyici {
    ad: Option<String>,
    govde: String,
    istek: Option<String>,
    /// Davranış özeti öğeleri: fırlatmalar, `res.status(4xx)` ve koşulları, iki yollu ifadeler
    dna: crate::dna::GovdeDna,
}

#[derive(Debug, Clone)]
struct Rota {
    nesne: String,
    yontem: String,
    yol: String,
    ara: Vec<Ara>,
    /// Gömülü fonksiyon ya da çözülecek ad
    isleyici: Option<Isleyici>,
    isleyici_adi: Option<String>,
    /// Fastify rota şeması: (bölüm, alanlar)
    sema: Vec<(&'static str, BTreeMap<String, Alan>)>,
    satir: usize,
    ozet: Option<String>,
    sira: usize,
}

#[derive(Debug, Clone)]
struct Baglama {
    nesne: String,
    onek: String,
    /// Argüman metinleri (router ya da ara katman; çözümlemede ayrılır)
    hedefler: Vec<String>,
    /// Çağrı biçimindeki argümanlar (ara katman)
    cagrilar: Vec<Ara>,
    sira: usize,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Dosya {
    yol: PathBuf,
    /// Rota nesneleri: ad -> kök mü (express()/fastify(); router ise false)
    nesneler: HashMap<String, bool>,
    /// Yerel ad -> modül belirteci (`./routes/x`), adlı içe aktarmada `#ad` eki
    iceri: HashMap<String, String>,
    /// Varsayılan dışa aktarım: rota nesnesi ya da fonksiyonun adı
    disari: Option<String>,
    /// Adlı dışa aktarımlar: dışarıdaki ad -> yerel ad
    adli_disari: HashMap<String, String>,
    fonksiyonlar: HashMap<String, Isleyici>,
    rotalar: Vec<Rota>,
    baglamalar: Vec<Baglama>,
    /// Next.js rota dosyası: dışa aktarılan yöntem fonksiyonları
    next_yontemleri: Vec<(String, Isleyici, usize)>,
    /// Değişkende tutulan rota seçenekleri (`const opts = { preHandler: requireAuth }`): ara katmanlar
    secenekler: HashMap<String, Vec<Ara>>,
    /// Değişkene atanmış ara katman çağrıları (`const yazma = roleMiddleware('tanimlar')`)
    ara_takma: HashMap<String, Ara>,
    /// Dosyadaki zod nesne şemaları (`const giris = z.object({...})`): ad -> alanlar (tanımdan)
    zod: HashMap<String, BTreeMap<String, Alan>>,
    /// Zincirle kurulan rota nesnesi (`const api = Router().use(a)`, `export default Router().use('/api', api)`):
    /// zincirin başındaki oluşturma çağrısının konumu -> nesnenin adı
    zincir: HashMap<(usize, usize), String>,
    satir_sayisi: usize,
    hatali: bool,
}

fn metin(n: Node, k: &[u8]) -> String {
    n.utf8_text(k).unwrap_or("").to_string()
}

/// Metin sabiti: 'x', "x" ya da değişkensiz `x`
fn metin_sabiti(n: Node, k: &[u8]) -> Option<String> {
    match n.kind() {
        "string" => Some(metin(n, k).trim_matches(['\'', '"']).to_string()),
        "template_string" => {
            let mut w = n.walk();
            let sabit = n.named_children(&mut w).all(|c| c.kind() != "template_substitution");
            sabit.then(|| metin(n, k).trim_matches('`').to_string())
        }
        _ => None,
    }
}

fn fonksiyon_mu(n: Node) -> bool {
    matches!(n.kind(), "arrow_function" | "function_expression" | "function" | "function_declaration")
}

/// Fonksiyonun gövdesi ve ilk parametresinin adı
fn isleyici(n: Node, k: &[u8], ad: Option<String>) -> Isleyici {
    let govde = n.child_by_field_name("body").map(|b| metin(b, k).chars().take(64 * 1024).collect()).unwrap_or_default();
    let istek = n.child_by_field_name("parameters").and_then(|p| {
        let mut w = p.walk();
        let ilk = p.named_children(&mut w).next()?;
        let ad = if ilk.kind() == "identifier" { ilk } else { ilk.child_by_field_name("pattern").filter(|x| x.kind() == "identifier")? };
        Some(metin(ad, k))
    });
    // Tek parametreli ok fonksiyonu: `req => ...`
    let istek = istek.or_else(|| n.child_by_field_name("parameter").map(|p| metin(p, k)));
    let dna = n.child_by_field_name("body").map(|b| crate::dna::ts_govde(b, k)).unwrap_or_default();
    Isleyici { ad, govde, istek, dna }
}

/// Bir ifadenin hemen üstündeki yorum (rota özeti)
fn ust_yorum(n: Node, k: &[u8]) -> Option<String> {
    let onceki = n.prev_named_sibling().filter(|p| p.kind() == "comment" && p.end_position().row + 1 >= n.start_position().row)?;
    let t = metin(onceki, k);
    let satirlar: Vec<String> = t
        .lines()
        .map(|l| l.trim().trim_start_matches("/**").trim_start_matches("/*").trim_end_matches("*/").trim_start_matches("//").trim_start_matches('*').trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('@'))
        .collect();
    satirlar.first().map(|s| guvenli::temiz(s).chars().take(200).collect())
}

/// Ara katman argümanları (dizi açılır)
fn ara_katmanlar(n: Node, k: &[u8], out: &mut Vec<Ara>) {
    match n.kind() {
        "identifier" | "member_expression" => out.push(Ara::Ad(metin(n, k))),
        "call_expression" => {
            let ad = n.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
            let mut args = vec![];
            if let Some(a) = n.child_by_field_name("arguments") {
                let mut w = a.walk();
                for x in a.named_children(&mut w) {
                    if let Some(s) = metin_sabiti(x, k) {
                        args.push(s);
                    } else if x.kind() == "array" {
                        let mut w2 = x.walk();
                        args.extend(x.named_children(&mut w2).filter_map(|y| metin_sabiti(y, k)));
                    }
                }
            }
            out.push(Ara::Cagri(ad, args));
        }
        "array" => {
            let mut w = n.walk();
            for x in n.named_children(&mut w) {
                ara_katmanlar(x, k, out);
            }
        }
        _ => {}
    }
}

/// Nesne sabitindeki anahtarın değeri
fn nesne_degeri<'t>(n: Node<'t>, k: &[u8], anahtar: &str) -> Option<Node<'t>> {
    if n.kind() != "object" {
        return None;
    }
    let mut w = n.walk();
    let bulunan = n
        .named_children(&mut w)
        .find(|p| p.kind() == "pair" && p.child_by_field_name("key").map(|x| metin(x, k).trim_matches(['\'', '"']).to_string()).as_deref() == Some(anahtar));
    bulunan.and_then(|p| p.child_by_field_name("value"))
}

/// Fastify JSON Schema nesnesinden alanlar (properties, required, type, enum, minLength ...)
fn json_sema_alanlari(n: Node, k: &[u8], satir: usize) -> Option<BTreeMap<String, Alan>> {
    let ozellikler = nesne_degeri(n, k, "properties")?;
    let zorunlu: Vec<String> = nesne_degeri(n, k, "required")
        .filter(|r| r.kind() == "array")
        .map(|r| {
            let mut w = r.walk();
            r.named_children(&mut w).filter_map(|x| metin_sabiti(x, k)).collect()
        })
        .unwrap_or_default();
    let mut alanlar = BTreeMap::new();
    let mut w = ozellikler.walk();
    for p in ozellikler.named_children(&mut w).filter(|p| p.kind() == "pair") {
        let ad = p.child_by_field_name("key").map(|x| metin(x, k).trim_matches(['\'', '"']).to_string()).unwrap_or_default();
        let deger = p.child_by_field_name("value");
        let tip = deger.and_then(|d| nesne_degeri(d, k, "type")).and_then(|t| metin_sabiti(t, k));
        let mut kurallar = vec![];
        let mut degerler = vec![];
        if let Some(d) = deger {
            for (anahtar, kural) in [("minLength", "MinLength"), ("maxLength", "MaxLength"), ("minimum", "Min"), ("maximum", "Max")] {
                if let Some(v) = nesne_degeri(d, k, anahtar) {
                    kurallar.push(format!("{kural}({})", metin(v, k)));
                }
            }
            if let Some(f) = nesne_degeri(d, k, "format").and_then(|f| metin_sabiti(f, k)) {
                match f.as_str() {
                    "email" => kurallar.push("IsEmail()".into()),
                    "uuid" => kurallar.push("IsUUID()".into()),
                    "uri" => kurallar.push("IsUrl()".into()),
                    "date-time" => kurallar.push("IsDateString()".into()),
                    _ => {}
                }
            }
            if let Some(e) = nesne_degeri(d, k, "enum").filter(|e| e.kind() == "array") {
                let mut w2 = e.walk();
                degerler = e.named_children(&mut w2).filter_map(|x| metin_sabiti(x, k)).collect();
            }
        }
        // JSON Schema tipleri Sezgi tip adlarına: integer -> int
        let tip = tip.map(|t| if t == "integer" { "int".to_string() } else { t });
        alanlar.insert(ad.clone(), Alan { tip, zorunlu: zorunlu.contains(&ad), belirsiz: false, kurallar, satir, kalitimdan: None, degerler, ic: None });
    }
    Some(alanlar)
}

/// `z.object({...})` (zincirle: `.strict()`, `.describe()` ...) -> alanlar; tip ve kurallar zod zincirinden, tanımdan
fn zod_nesnesi(n: Node, k: &[u8]) -> Option<BTreeMap<String, Alan>> {
    let mut ic = n;
    let nesne = loop {
        if ic.kind() != "call_expression" {
            return None;
        }
        let f = ic.child_by_field_name("function")?;
        if metin(f, k) == "z.object" {
            break ic.child_by_field_name("arguments")?.named_child(0).filter(|o| o.kind() == "object")?;
        }
        ic = f.child_by_field_name("object")?;
    };
    let mut alanlar = BTreeMap::new();
    let mut w = nesne.walk();
    for p in nesne.named_children(&mut w).filter(|p| p.kind() == "pair") {
        let (Some(a), Some(v)) = (p.child_by_field_name("key"), p.child_by_field_name("value")) else { continue };
        let z = crate::model::zod_alan(v, k);
        let degerler = z
            .degerler_ham
            .as_deref()
            .filter(|h| h.starts_with('['))
            .map(|h| h.trim_matches(['[', ']']).split(',').map(|x| x.trim().trim_matches(['\'', '"']).to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default();
        alanlar.insert(
            metin(a, k).trim_matches(['\'', '"']).to_string(),
            Alan {
                tip: Some(z.tip),
                zorunlu: !z.istege_bagli,
                belirsiz: false,
                kurallar: z.kurallar.iter().map(|r| guvenli::temiz(r)).collect(),
                satir: z.satir,
                kalitimdan: None,
                degerler,
                ic: None,
            },
        );
    }
    Some(alanlar)
}

/// Joi nesne şeması: `Joi.object({ a: Joi.string().required(), b: Joi.string().valid('x', 'y') })` (sonda `.unknown()`
/// gibi halkalar olabilir). Joi'de alan `.required()` yoksa isteğe bağlıdır. Tanımdır: kesin.
fn joi_nesnesi(n: Node, k: &[u8]) -> Option<BTreeMap<String, Alan>> {
    let mut ic = n;
    let nesne = loop {
        if ic.kind() != "call_expression" {
            return None;
        }
        let f = ic.child_by_field_name("function")?;
        if matches!(metin(f, k).as_str(), "Joi.object" | "joi.object") {
            break ic.child_by_field_name("arguments")?.named_child(0).filter(|o| o.kind() == "object")?;
        }
        ic = f.child_by_field_name("object")?;
    };
    let tip_d = crate::kural::derle(r"^\s*[Jj]oi\.(\w+)\(");
    let deger_d = crate::kural::derle(r#"\.(?:valid|allow)\(([^)]*)\)"#);
    let metin_d = crate::kural::derle(r#"['"]([^'"]*)['"]"#);
    let mut alanlar = BTreeMap::new();
    let mut w = nesne.walk();
    for p in nesne.named_children(&mut w).filter(|p| p.kind() == "pair") {
        let (Some(a), Some(v)) = (p.child_by_field_name("key"), p.child_by_field_name("value")) else { continue };
        let ifade = metin(v, k);
        let tip = match tip_d.captures(&ifade).map(|c| c[1].to_string()).as_deref() {
            Some("string") => "string",
            Some("number") => "number",
            Some("boolean") => "boolean",
            Some("array") => "array",
            Some("object") => "object",
            Some("date") => "string",
            _ => "object",
        };
        let degerler: Vec<String> = deger_d.captures_iter(&ifade).flat_map(|c| metin_d.captures_iter(&c[1]).map(|m| m[1].to_string()).collect::<Vec<_>>()).collect();
        alanlar.insert(
            metin(a, k).trim_matches(['\'', '"']).to_string(),
            Alan {
                tip: Some(tip.into()),
                zorunlu: ifade.contains(".required()"),
                belirsiz: false,
                kurallar: vec![],
                satir: v.start_position().row + 1,
                kalitimdan: None,
                degerler,
                ic: None,
            },
        );
    }
    (!alanlar.is_empty()).then_some(alanlar)
}

/// İlk parametresi rota nesnesi olabilecek fonksiyonu kaydeder (Fastify eklentisi, `kaydet(app)`)
fn eklenti_parametresi(d: &mut Dosya, i: &Isleyici) {
    if let Some(p) = i.istek.clone().filter(|p| EKLENTI_PARAMETRELERI.contains(&p.as_str())) {
        d.nesneler.entry(p).or_insert(false);
    }
}

fn ayristir(yol: &Path, kaynak: &str) -> Option<Dosya> {
    let mut p = Parser::new();
    let dil = if yol.extension().is_some_and(|e| e == "tsx" || e == "jsx") { tree_sitter_typescript::LANGUAGE_TSX } else { tree_sitter_typescript::LANGUAGE_TYPESCRIPT };
    p.set_language(&dil.into()).ok()?;
    let k = kaynak.as_bytes();
    let agac = p.parse(k, None)?;
    let kok = agac.root_node();
    let mut d = Dosya { yol: yol.to_path_buf(), satir_sayisi: kaynak.lines().count(), hatali: kok.has_error(), ..Default::default() };
    // 1. geçiş: nesneler, içe aktarmalar, fonksiyonlar, dışa aktarımlar
    let mut yigin = vec![kok];
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "variable_declarator" => {
                if let (Some(ad), Some(deger)) = (n.child_by_field_name("name"), n.child_by_field_name("value")) {
                    let deger = if deger.kind() == "await_expression" { deger.named_child(0).unwrap_or(deger) } else { deger };
                    if ad.kind() == "identifier" {
                        let ad_m = metin(ad, k);
                        if let Some((bas, kok_mu)) = zincir_basi(deger, k) {
                            // Router().use(a).use(b): zincirdeki çağrılar bu nesnenin
                            d.nesneler.insert(ad_m.clone(), kok_mu);
                            d.zincir.insert((bas.start_byte(), bas.end_byte()), ad_m.clone());
                        } else if deger.kind() == "call_expression" {
                            let f = deger.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                            match f.as_str() {
                                "express" | "fastify" | "Fastify" | "express.default" => {
                                    d.nesneler.insert(ad_m.clone(), true);
                                }
                                "express.Router" | "Router" | "require('express').Router" | "require(\"express\").Router" => {
                                    d.nesneler.insert(ad_m.clone(), false);
                                }
                                "require" => {
                                    if let Some(s) = deger.child_by_field_name("arguments").and_then(|a| a.named_child(0)).and_then(|a| metin_sabiti(a, k)) {
                                        d.iceri.insert(ad_m.clone(), s);
                                    }
                                }
                                // Sarmalanmış işleyici: `const kayit = asyncHandler(async (req, res) => {...})`,
                                // catchAsync / wrapAsync: gövde ve girdiler içteki fonksiyondan okunur
                                _ if !f.contains('.') && sarili_fonksiyon(deger).is_some() => {
                                    if let Some(ic) = sarili_fonksiyon(deger) {
                                        d.fonksiyonlar.insert(ad_m.clone(), isleyici(ic, k, Some(ad_m.clone())));
                                    }
                                }
                                _ => {
                                    if let Some(alanlar) = zod_nesnesi(deger, k).or_else(|| joi_nesnesi(deger, k)) {
                                        d.zod.insert(ad_m.clone(), alanlar);
                                    } else {
                                        let mut ara = vec![];
                                        ara_katmanlar(deger, k, &mut ara);
                                        if let Some(a) = ara.pop() {
                                            d.ara_takma.insert(ad_m.clone(), a);
                                        }
                                    }
                                }
                            }
                        } else if fonksiyon_mu(deger) {
                            d.fonksiyonlar.insert(ad_m.clone(), isleyici(deger, k, Some(ad_m.clone())));
                        } else if deger.kind() == "object" {
                            let mut ara = vec![];
                            for kanca in ["preHandler", "onRequest", "preValidation"] {
                                if let Some(v) = nesne_degeri(deger, k, kanca) {
                                    ara_katmanlar(v, k, &mut ara);
                                }
                            }
                            if !ara.is_empty() {
                                d.secenekler.insert(ad_m.clone(), ara);
                            }
                        }
                    } else if ad.kind() == "object_pattern" && deger.kind() == "call_expression" && deger.child_by_field_name("function").is_some_and(|f| metin(f, k) == "require")
                    {
                        // const { a, b: c } = require('./x')
                        if let Some(s) = deger.child_by_field_name("arguments").and_then(|a| a.named_child(0)).and_then(|a| metin_sabiti(a, k)) {
                            let mut w = ad.walk();
                            for x in ad.named_children(&mut w) {
                                match x.kind() {
                                    "shorthand_property_identifier_pattern" => {
                                        d.iceri.insert(metin(x, k), format!("{s}#{}", metin(x, k)));
                                    }
                                    "pair_pattern" => {
                                        if let (Some(a), Some(v)) = (x.child_by_field_name("key"), x.child_by_field_name("value")) {
                                            d.iceri.insert(metin(v, k), format!("{s}#{}", metin(a, k)));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            "function_declaration" => {
                if let Some(ad) = n.child_by_field_name("name") {
                    d.fonksiyonlar.insert(metin(ad, k), isleyici(n, k, Some(metin(ad, k))));
                }
            }
            "import_statement" => {
                let kaynak_ad = n.child_by_field_name("source").and_then(|s| metin_sabiti(s, k)).unwrap_or_default();
                let mut w = n.walk();
                for c in n.named_children(&mut w).filter(|c| c.kind() == "import_clause") {
                    let mut w2 = c.walk();
                    for x in c.named_children(&mut w2) {
                        match x.kind() {
                            "identifier" => {
                                d.iceri.insert(metin(x, k), kaynak_ad.clone());
                            }
                            "named_imports" => {
                                let mut w3 = x.walk();
                                for s in x.named_children(&mut w3).filter(|s| s.kind() == "import_specifier") {
                                    let ad = s.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
                                    let yerel = s.child_by_field_name("alias").map(|a| metin(a, k)).unwrap_or_else(|| ad.clone());
                                    d.iceri.insert(yerel, format!("{kaynak_ad}#{ad}"));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "assignment_expression" => {
                let sol = n.child_by_field_name("left").map(|l| metin(l, k)).unwrap_or_default();
                if let Some(sag) = n.child_by_field_name("right") {
                    if sol == "module.exports" {
                        if fonksiyon_mu(sag) {
                            let i = isleyici(sag, k, Some("default".into()));
                            eklenti_parametresi(&mut d, &i);
                            d.fonksiyonlar.insert("default".into(), i);
                            d.disari = Some("default".into());
                        } else if sag.kind() == "object" {
                            let mut w = sag.walk();
                            for x in sag.named_children(&mut w) {
                                match x.kind() {
                                    "shorthand_property_identifier" => {
                                        d.adli_disari.insert(metin(x, k), metin(x, k));
                                    }
                                    "pair" => {
                                        if let (Some(a), Some(v)) = (x.child_by_field_name("key"), x.child_by_field_name("value")) {
                                            let a = metin(a, k);
                                            if fonksiyon_mu(v) {
                                                d.fonksiyonlar.insert(a.clone(), isleyici(v, k, Some(a.clone())));
                                                d.adli_disari.insert(a.clone(), a);
                                            } else {
                                                d.adli_disari.insert(a, metin(v, k));
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        } else if let Some((bas, kok_mu)) = zincir_basi(sag, k) {
                            // module.exports = express.Router().use('/v2', ic)
                            d.nesneler.insert("default".into(), kok_mu);
                            d.zincir.insert((bas.start_byte(), bas.end_byte()), "default".into());
                            d.disari = Some("default".into());
                        } else {
                            d.disari = Some(metin(sag, k));
                        }
                    } else if let Some(ad) = sol.strip_prefix("exports.").or_else(|| sol.strip_prefix("module.exports.")) {
                        if fonksiyon_mu(sag) {
                            d.fonksiyonlar.insert(ad.to_string(), isleyici(sag, k, Some(ad.to_string())));
                        }
                        d.adli_disari.insert(ad.to_string(), if fonksiyon_mu(sag) { ad.to_string() } else { metin(sag, k) });
                    }
                }
            }
            "export_statement" => {
                let varsayilan = (0..n.child_count()).filter_map(|i| n.child(i)).any(|c| c.kind() == "default");
                if let Some(bildirim) = n.child_by_field_name("declaration") {
                    if bildirim.kind() == "function_declaration" {
                        let ad = bildirim.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_else(|| "default".into());
                        let i = isleyici(bildirim, k, Some(ad.clone()));
                        if varsayilan {
                            eklenti_parametresi(&mut d, &i);
                            d.disari = Some(ad.clone());
                        }
                        d.adli_disari.insert(ad.clone(), ad.clone());
                        d.fonksiyonlar.insert(ad, i);
                    } else if bildirim.kind() == "lexical_declaration" {
                        let mut w = bildirim.walk();
                        for x in bildirim.named_children(&mut w).filter(|x| x.kind() == "variable_declarator") {
                            if let (Some(a), Some(v)) = (x.child_by_field_name("name"), x.child_by_field_name("value")) {
                                let a = metin(a, k);
                                if fonksiyon_mu(v) {
                                    d.fonksiyonlar.insert(a.clone(), isleyici(v, k, Some(a.clone())));
                                }
                                d.adli_disari.insert(a.clone(), a);
                            }
                        }
                    }
                } else if varsayilan {
                    if let Some(v) = n.child_by_field_name("value") {
                        if fonksiyon_mu(v) {
                            let i = isleyici(v, k, Some("default".into()));
                            eklenti_parametresi(&mut d, &i);
                            d.fonksiyonlar.insert("default".into(), i);
                            d.disari = Some("default".into());
                        } else if let Some((bas, kok_mu)) = zincir_basi(v, k) {
                            // export default Router().use('/api', api)
                            d.nesneler.insert("default".into(), kok_mu);
                            d.zincir.insert((bas.start_byte(), bas.end_byte()), "default".into());
                            d.disari = Some("default".into());
                        } else {
                            d.disari = Some(metin(v, k));
                        }
                    }
                }
            }
            _ => {}
        }
        let mut w = n.walk();
        let mut cocuklar: Vec<Node> = n.named_children(&mut w).collect();
        cocuklar.reverse();
        yigin.extend(cocuklar);
    }
    // Rota tanımlayan fonksiyonlar: ilk parametresi app / fastify / router ... olan ve gövdesinde rota çağrısı bulunan
    // (`export function registerAuthRoutes(app) { app.post(...) }`); parametre o dosyada rota nesnesidir
    let eklentiler: Vec<String> = d
        .fonksiyonlar
        .values()
        .filter_map(|f| f.istek.clone().filter(|p| EKLENTI_PARAMETRELERI.contains(&p.as_str()) && YONTEMLER.iter().any(|y| f.govde.contains(&format!("{p}.{y}(")))))
        .collect();
    for p in eklentiler {
        d.nesneler.entry(p).or_insert(false);
    }
    // 2. geçiş: rotalar ve bağlamalar (kaynak sırasıyla)
    let mut sira = 0usize;
    let mut yigin = vec![kok];
    while let Some(n) = yigin.pop() {
        if n.kind() == "call_expression" {
            sira += 1;
            rota_ya_da_baglama(n, k, &mut d, sira);
        }
        let mut w = n.walk();
        let mut cocuklar: Vec<Node> = n.named_children(&mut w).collect();
        cocuklar.reverse();
        yigin.extend(cocuklar);
    }
    // Next.js rota dosyası: dışa aktarılan yöntem fonksiyonları
    if yol.file_stem().is_some_and(|s| s == "route") {
        for y in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
            if let Some(f) = d.fonksiyonlar.get(y).filter(|_| d.adli_disari.contains_key(y)) {
                let satir = kaynak.lines().position(|l| l.contains("export") && l.contains(y)).map(|i| i + 1).unwrap_or(1);
                d.next_yontemleri.push((y.to_string(), f.clone(), satir));
            }
        }
    }
    Some(d)
}

/// Rota nesnesi oluşturan çağrı mı: (kök uygulama mı)
fn olusturma(f: &str) -> Option<bool> {
    match f {
        "express" | "fastify" | "Fastify" | "express.default" => Some(true),
        "express.Router" | "Router" | "require('express').Router" | "require(\"express\").Router" => Some(false),
        _ => None,
    }
}

/// `Router().use(a).get(...)` zincirinin başındaki oluşturma çağrısı (zincir en az bir halkalıysa)
fn zincir_basi<'t>(n: Node<'t>, k: &[u8]) -> Option<(Node<'t>, bool)> {
    let mut ic = n;
    let mut halka = 0;
    while ic.kind() == "call_expression" && halka < 64 {
        let f = ic.child_by_field_name("function")?;
        // Halka: nesnesi bir çağrı olan üye (`Router().use`); `express.Router` ise oluşturma çağrısının adıdır
        if let Some(o) = (f.kind() == "member_expression").then(|| f.child_by_field_name("object")).flatten().filter(|o| o.kind() == "call_expression") {
            ic = o;
            halka += 1;
            continue;
        }
        return olusturma(&metin(f, k)).filter(|_| halka > 0).map(|kok| (ic, kok));
    }
    None
}

/// Sarmalayıcı çağrının (`asyncHandler(fn)`, `catchAsync(fn)`) argümanlarındaki ilk fonksiyon
fn sarili_fonksiyon(n: Node) -> Option<Node> {
    let a = n.child_by_field_name("arguments")?;
    let mut w = a.walk();
    let bulunan = a.named_children(&mut w).find(|x| fonksiyon_mu(*x));
    bulunan
}

/// Bir çağrı ifadesi: rota (`router.get('/x', ...)`), bağlama (`app.use`, `register`) ya da `kaydet(app)`
fn rota_ya_da_baglama(n: Node, k: &[u8], d: &mut Dosya, sira: usize) {
    let Some(f) = n.child_by_field_name("function") else { return };
    let args: Vec<Node> = n
        .child_by_field_name("arguments")
        .map(|a| {
            let mut w = a.walk();
            a.named_children(&mut w).filter(|x| x.kind() != "comment").collect()
        })
        .unwrap_or_default();
    // registerAuthRoutes(app): rota nesnesini argüman alan fonksiyon çağrısı bir bağlamadır
    if matches!(f.kind(), "identifier" | "member_expression") {
        let fad = metin(f, k);
        let nesne_arg = args.iter().find(|x| x.kind() == "identifier" && d.nesneler.contains_key(&metin(**x, k))).map(|x| metin(*x, k));
        let ozellik = f.child_by_field_name("property").map(|p| metin(p, k)).unwrap_or_default();
        if let Some(nesne) = nesne_arg.filter(|_| !matches!(ozellik.as_str(), "use" | "register" | "get" | "post" | "put" | "patch" | "delete" | "listen") && fad != "require") {
            d.baglamalar.push(Baglama { nesne, onek: String::new(), hedefler: vec![format!("fonksiyon:{fad}")], cagrilar: vec![], sira });
        }
    }
    if f.kind() != "member_expression" {
        return;
    }
    let yontem = f.child_by_field_name("property").map(|p| metin(p, k)).unwrap_or_default();
    // router.route('/x').get(...): nesne ve yol zincirden
    let (nesne, zincir_yol) = match f.child_by_field_name("object") {
        Some(o) if o.kind() == "call_expression" => {
            let mut ic = o;
            loop {
                if let Some(ad) = d.zincir.get(&(ic.start_byte(), ic.end_byte())) {
                    break (ad.clone(), None);
                }
                let Some(f2) = ic.child_by_field_name("function").filter(|f| f.kind() == "member_expression") else { break (String::new(), None) };
                let ozellik = f2.child_by_field_name("property").map(|p| metin(p, k)).unwrap_or_default();
                if ozellik == "route" {
                    let yol = ic.child_by_field_name("arguments").and_then(|a| a.named_child(0)).and_then(|a| metin_sabiti(a, k));
                    break (f2.child_by_field_name("object").map(|x| metin(x, k)).unwrap_or_default(), yol);
                }
                match f2.child_by_field_name("object") {
                    Some(x) if x.kind() == "call_expression" => ic = x,
                    _ => break (String::new(), None),
                }
            }
        }
        Some(o) => (metin(o, k), None),
        None => (String::new(), None),
    };
    if !d.nesneler.contains_key(&nesne) {
        return;
    }
    if YONTEMLER.contains(&yontem.as_str()) {
        let yol_ve_kalan = match &zincir_yol {
            Some(y) => Some((y.clone(), &args[..])),
            None => args.first().and_then(|a| metin_sabiti(*a, k)).map(|y| (y, &args[1..])),
        };
        let Some((yol, kalan)) = yol_ve_kalan else { return };
        let mut ara = vec![];
        let mut sema = vec![];
        let mut isleyici_d = None;
        let mut isleyici_adi = None;
        // İşleyici: son fonksiyon argümanı (satır içi ya da `asyncHandler(fn)`); ondan sonrakiler cevabı yazan ara
        // katmandır (`router.post('/x', asyncHandler(fn), respond)`). Fonksiyon argümanı yoksa son ad işleyicidir.
        let fonksiyon_sirasi = kalan.iter().rposition(|a| fonksiyon_mu(*a) || (a.kind() == "call_expression" && sarili_fonksiyon(*a).is_some()));
        let kalan = &kalan[..fonksiyon_sirasi.map(|s| s + 1).unwrap_or(kalan.len())];
        for (i, a) in kalan.iter().enumerate() {
            let son = i + 1 == kalan.len();
            if son && fonksiyon_mu(*a) {
                isleyici_d = Some(isleyici(*a, k, a.child_by_field_name("name").map(|x| metin(x, k))));
            } else if son && matches!(a.kind(), "identifier" | "member_expression") {
                isleyici_adi = Some(metin(*a, k));
            } else if let Some(ic) = (son && a.kind() == "call_expression").then(|| sarili_fonksiyon(*a)).flatten() {
                // asyncHandler(async (req, res) => {...}): sarmalayıcının içindeki fonksiyon işleyicidir
                isleyici_d = Some(isleyici(ic, k, None));
            } else if a.kind() == "object" {
                // Fastify rota seçenekleri: schema ve ara katmanlar
                if let Some(s) = nesne_degeri(*a, k, "schema") {
                    for (bolum, anahtar) in [("govde", "body"), ("sorgu", "querystring")] {
                        if let Some(alanlar) = nesne_degeri(s, k, anahtar).and_then(|b| json_sema_alanlari(b, k, n.start_position().row + 1)) {
                            sema.push((bolum, alanlar));
                        }
                    }
                }
                for kanca in ["preHandler", "onRequest", "preValidation"] {
                    if let Some(v) = nesne_degeri(*a, k, kanca) {
                        ara_katmanlar(v, k, &mut ara);
                    }
                }
                if let Some(h) = nesne_degeri(*a, k, "handler") {
                    if fonksiyon_mu(h) {
                        isleyici_d = Some(isleyici(h, k, None));
                    } else {
                        isleyici_adi = Some(metin(h, k));
                    }
                }
            } else if let Some(s) = (a.kind() == "identifier").then(|| d.secenekler.get(&metin(*a, k))).flatten() {
                // Değişkende tutulan seçenek nesnesi: içindeki ara katmanlar
                ara.extend(s.iter().cloned());
            } else {
                ara_katmanlar(*a, k, &mut ara);
            }
        }
        let ifade = n.parent().filter(|p| p.kind() == "expression_statement").unwrap_or(n);
        let rota = Rota {
            nesne,
            yontem: yontem.to_uppercase(),
            yol: yol.clone(),
            ara,
            isleyici: isleyici_d,
            isleyici_adi,
            sema,
            satir: n.start_position().row + 1,
            ozet: ust_yorum(ifade, k),
            sira,
        };
        // Express'te `:ad?` isteğe bağlı parametredir: rota parametreli ve parametresiz iki adresi karşılar
        let kisa: String = yol.split('/').filter(|p| !(p.starts_with(':') && p.ends_with('?'))).collect::<Vec<_>>().join("/");
        if kisa != yol && kisa.split('/').any(|p| p.starts_with(':') || !p.is_empty()) {
            d.rotalar.push(Rota { yol: if kisa.is_empty() { "/".into() } else { kisa }, ..rota.clone() });
        }
        d.rotalar.push(rota);
    } else if yontem == "use" || yontem == "register" {
        let (mut onek, kalan): (String, &[Node]) = match args.first().and_then(|a| metin_sabiti(*a, k)) {
            Some(o) if yontem == "use" => (o, &args[1..]),
            _ => (String::new(), &args[..]),
        };
        let mut hedefler = vec![];
        let mut cagrilar = vec![];
        for a in kalan {
            match a.kind() {
                "identifier" | "member_expression" => hedefler.push(metin(*a, k)),
                "call_expression" => {
                    let fa = a.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                    let ilk = a.child_by_field_name("arguments").and_then(|x| x.named_child(0)).and_then(|x| metin_sabiti(x, k));
                    match (fa.as_str(), ilk) {
                        ("require", Some(s)) => hedefler.push(format!("require:{s}")),
                        _ => ara_katmanlar(*a, k, &mut cagrilar),
                    }
                }
                // fastify.register(plugin, { prefix: '/x' })
                "object" if yontem == "register" => {
                    if let Some(p) = nesne_degeri(*a, k, "prefix").and_then(|p| metin_sabiti(p, k)) {
                        onek = p;
                    }
                }
                "array" => ara_katmanlar(*a, k, &mut cagrilar),
                _ => {}
            }
        }
        d.baglamalar.push(Baglama { nesne, onek, hedefler, cagrilar, sira });
    }
}

// ---- Proje düzeyi çözümleme ----

struct Proje<'a> {
    kok: &'a Path,
    dosyalar: &'a [Dosya],
    dizin: HashMap<PathBuf, usize>,
}

impl<'a> Proje<'a> {
    /// `./routes/x` -> dosya; uzantısız, `.js` yazılmış `.ts` kaynağı ve index dosyası denenir
    fn modul(&self, kimden: usize, belirtec: &str) -> Option<usize> {
        // `@/lib/x` takma yolu (Next.js ve tsconfig alışkanlığı): önce src/, sonra kök
        if let Some(ic) = belirtec.strip_prefix("@/") {
            return [self.kok.join("src").join(ic), self.kok.join(ic)].iter().find_map(|t| self.dosya_bul(t));
        }
        if !belirtec.starts_with('.') {
            return None;
        }
        self.dosya_bul(&self.dosyalar[kimden].yol.parent()?.join(belirtec))
    }

    fn dosya_bul(&self, taban: &Path) -> Option<usize> {
        let taban = taban.to_path_buf();
        // Uzantı eklenir, değiştirilmez: `./tag/tag.controller` -> tag.controller.ts (with_extension ".controller"ı silerdi)
        let ekli = |u: &str| {
            let mut s = taban.clone().into_os_string();
            s.push(format!(".{u}"));
            PathBuf::from(s)
        };
        let mut adaylar = vec![taban.clone()];
        adaylar.extend(["js", "ts", "mjs", "cjs", "tsx", "jsx"].map(ekli));
        // `.js` yazılmış `.ts` kaynağı (ESM TypeScript alışkanlığı): ./x.js -> x.ts
        if taban.extension().is_some_and(|e| matches!(e.to_str(), Some("js" | "mjs" | "cjs" | "jsx"))) {
            adaylar.extend([taban.with_extension("ts"), taban.with_extension("tsx")]);
        }
        adaylar.extend([taban.join("index.js"), taban.join("index.ts")]);
        adaylar.iter().find_map(|a| self.dizin.get(&normal(a)).copied())
    }

    /// Bir argüman adını (dosya, ad) olarak çözer: yerel ad, içe aktarılmış modülün dışa aktarımı, `require:` metni
    /// ya da `fonksiyon:ad` (rota fonksiyonunun rota parametresi)
    fn coz(&self, di: usize, ad: &str, derinlik: usize) -> Option<(usize, String)> {
        if derinlik > 6 {
            return None;
        }
        let d = &self.dosyalar[di];
        if let Some(f) = ad.strip_prefix("fonksiyon:") {
            let (m, yerel) = self.coz(di, f, derinlik + 1)?;
            let isl = self.dosyalar[m].fonksiyonlar.get(&yerel)?;
            let p = isl.istek.clone().filter(|p| self.dosyalar[m].nesneler.contains_key(p))?;
            return Some((m, p));
        }
        if let Some(b) = ad.strip_prefix("require:") {
            let m = self.modul(di, b)?;
            return self.varsayilan(m, derinlik + 1);
        }
        if d.nesneler.contains_key(ad) || d.fonksiyonlar.contains_key(ad) {
            return Some((di, ad.to_string()));
        }
        // controller.list: içe aktarılmış modülün adlı dışa aktarımı
        if let Some((nesne, uye)) = ad.split_once('.') {
            let b = d.iceri.get(nesne).filter(|b| !b.contains('#'))?;
            let m = self.modul(di, b)?;
            let yerel = self.dosyalar[m].adli_disari.get(uye).cloned().unwrap_or_else(|| uye.to_string());
            return self.coz(m, &yerel, derinlik + 1);
        }
        let b = d.iceri.get(ad)?;
        match b.split_once('#') {
            Some((modul, uye)) => {
                let m = self.modul(di, modul)?;
                let yerel = self.dosyalar[m].adli_disari.get(uye).cloned().unwrap_or_else(|| uye.to_string());
                self.coz(m, &yerel, derinlik + 1)
            }
            None => {
                let m = self.modul(di, b)?;
                self.varsayilan(m, derinlik + 1)
            }
        }
    }

    /// Modülün varsayılan dışa aktarımı: rota nesnesi ya da eklenti fonksiyonunun rota parametresi
    fn varsayilan(&self, m: usize, derinlik: usize) -> Option<(usize, String)> {
        let d = &self.dosyalar[m];
        let ad = d.disari.clone()?;
        if let Some(f) = d.fonksiyonlar.get(&ad) {
            if let Some(p) = f.istek.as_ref().filter(|p| d.nesneler.contains_key(*p)) {
                return Some((m, p.clone()));
            }
        }
        self.coz(m, &ad, derinlik)
    }

    /// Hedef bir rota nesnesine mi çözülüyor
    fn rota_nesnesi(&self, di: usize, ad: &str) -> Option<(usize, String)> {
        self.coz(di, ad, 0).filter(|(m, a)| self.dosyalar[*m].nesneler.contains_key(a))
    }

    fn fonksiyon(&self, di: usize, ad: &str) -> Option<&'a Isleyici> {
        let (m, yerel) = self.coz(di, ad, 0)?;
        self.dosyalar[m].fonksiyonlar.get(&yerel)
    }
}

fn normal(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// Rota yolu ara katman önekine uyuyor mu (`:ad` her parçaya uyar)
fn yol_uyar(onek: &str, yol: &str) -> bool {
    let o: Vec<&str> = onek.split('/').filter(|x| !x.is_empty()).collect();
    let y: Vec<&str> = yol.split('/').filter(|x| !x.is_empty()).collect();
    o.len() <= y.len() && o.iter().zip(&y).all(|(a, b)| a == b || a.starts_with(':') || *a == "*")
}

fn yol_birlestir(a: &str, b: &str) -> String {
    let parcalar: Vec<&str> = a.split('/').chain(b.split('/')).filter(|p| !p.is_empty()).collect();
    format!("/{}", parcalar.join("/"))
}

/// Express yol sözdizimi: `:id?` -> `:id`, `{id}` -> `:id`
fn yol_normal(y: &str) -> String {
    y.split('/')
        .map(|p| {
            if p.starts_with(':') {
                p.trim_end_matches(['?', '*', '+']).to_string()
            } else if p.starts_with('{') && p.ends_with('}') {
                format!(":{}", &p[1..p.len() - 1])
            } else {
                p.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Ara katman yetki gösteriyor mu (adı ya da tanımı) ve rol argümanları
fn yetki(p: &Proje, di: usize, a: &Ara) -> (bool, Vec<String>) {
    // Değişkene atanmış ara katman: atandığı çağrı (`const yazma = roleMiddleware('x')`)
    let takma = match a {
        Ara::Ad(ad) => p.dosyalar[di].ara_takma.get(ad).cloned(),
        _ => None,
    };
    if let Some(t) = takma {
        let (y, r) = yetki_asil(p, di, &t);
        let (y2, _) = yetki_asil(p, di, a);
        return (y || y2, r);
    }
    yetki_asil(p, di, a)
}

/// İsteğe bağlı kimlik ara katmanı: adı (`auth.optional`, `optionalAuth`) ya da tanımı (`credentialsRequired: false`)
fn istege_bagli_mi(p: &Proje, di: usize, a: &Ara) -> bool {
    let ad = a.ad();
    crate::kural::derle(r"(?i)optional|opsiyonel|istege|maybe|soft").is_match(ad)
        || p.fonksiyon(di, ad).is_some_and(|f| f.govde.replace(' ', "").contains("credentialsRequired:false"))
}

/// Yetki gösteren ara katman / yardımcı adları
fn yetki_ad_deseni() -> &'static Regex {
    static AD: OnceLock<Regex> = OnceLock::new();
    AD.get_or_init(|| {
        Regex::new(r"(?i)auth|jwt|token|protect|logged|login|session|oturum|verify|guard|require(d)?(user|role|admin|permission|can)|admin|role|permission|izin|yetki").unwrap()
    })
}

fn yetki_asil(p: &Proje, di: usize, a: &Ara) -> (bool, Vec<String>) {
    let ad_desen = yetki_ad_deseni();
    let tanim = p.fonksiyon(di, a.ad()).map(|f| f.govde.as_str()).unwrap_or("");
    let tanim_yetki = tanim.contains("401") || tanim.contains("Unauthorized") || tanim.contains("jwt.verify") || tanim.contains("verifyToken");
    let mut roller = match a {
        Ara::Cagri(_, args) if ad_desen.is_match(a.ad()) => args.clone(),
        _ => vec![],
    };
    // Adı yalnızca bir rolü söyleyen ara katman (`admin`, `isAdmin`, `requireSuperuser`): rol adından çıkarılır [Yorum]
    if roller.is_empty() {
        roller.extend(rol_adi(a.ad()));
    }
    (ad_desen.is_match(a.ad()) || tanim_yetki, roller)
}

/// Ara katmanın adı tek başına bir rolü söylüyorsa o rol: `admin`, `isAdmin`, `adminOnly`, `requireSuperuser`,
/// `auth.admin`. Adı kimlik denetimi söyleyen (`protect`, `auth`) ya da birden çok şey söyleyen ad rol değildir.
fn rol_adi(ad: &str) -> Option<String> {
    static ROL: OnceLock<Regex> = OnceLock::new();
    let re = ROL.get_or_init(|| {
        Regex::new(r"(?i)^(?:is_?|require_?|requires_?|ensure_?|only_?|check_?)?(super_?admin|superuser|admin|staff)(?:_?only|_?required|_?guard|_?middleware|_?check)?$").unwrap()
    });
    let son = ad.rsplit('.').next().unwrap_or(ad);
    re.captures(son).map(|c| c[1].to_lowercase().replace('_', ""))
}

/// Dosya yüklemesi: multer (`upload.single('x')`, `multer().array(...)`), işleyicide `req.file(s)` ya da Fastify
/// multipart (`request.file()`, `.parts()`, `saveRequestFiles`). Dosya gövde şemasının parçası değildir; önizleme sınayamaz.
fn dosya_notu(ara: &[(usize, Ara)], isleyici: Option<&Isleyici>) -> Option<String> {
    static D: OnceLock<[Regex; 2]> = OnceLock::new();
    let [yukleme, kullanim] = D.get_or_init(|| {
        [
            Regex::new(r"(?i)(?:multer|upload)[\w.()'\x22`{}:, -]*\.(?:single|array|fields|any)$").unwrap(),
            Regex::new(r"\b(?:req|request)\.files?\b|\.(?:parts|saveRequestFiles)\(|\bformData\(\)").unwrap(),
        ]
    });
    let neden = ara
        .iter()
        .find(|(_, a)| yukleme.is_match(a.ad()))
        .map(|(_, a)| format!("ara katman {}", guvenli::temiz(a.ad())))
        .or_else(|| isleyici.and_then(|i| kullanim.find(&i.govde)).map(|m| format!("işleyicide {}", guvenli::temiz(m.as_str()))))?;
    Some(format!("dosya doğrulaması kontrol edilemedi: {neden}; dosyanın varlığı, türü ve içeriği önizlemede sınanmaz"))
}

fn bolum_adi(s: &str) -> &'static str {
    match s {
        "body" => "govde",
        "query" => "sorgu",
        _ => "parametre",
    }
}

/// Yapı bozumu metninden alanlar: `a, b = 1, c: d` -> (a, None), (b, Some("1")), (c, None)
fn yapi_alanlari(metin: &str) -> Vec<(String, Option<String>)> {
    let mut out = vec![];
    for parca in metin.split(',') {
        let parca = parca.trim();
        if parca.is_empty() || parca.starts_with("...") {
            continue;
        }
        let (sol, varsayilan) = match parca.split_once('=') {
            Some((s, v)) => (s.trim(), Some(v.trim().to_string())),
            None => (parca, None),
        };
        let ad = sol.split(':').next().unwrap_or("").trim();
        if !ad.is_empty() && ad.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
            out.push((ad.to_string(), varsayilan));
        }
    }
    out
}

/// İşleyici gövdesindeki girdi kullanımı: (bölüm, alan adı, varsayılan değer metni)
fn kullanim_girdileri(i: &Isleyici) -> Vec<(&'static str, String, Option<String>)> {
    static D: OnceLock<[Regex; 3]> = OnceLock::new();
    let [yapi, uye, koseli] = D.get_or_init(|| {
        [
            Regex::new(r"(?:const|let|var)\s*\{([^}]*)\}\s*=\s*([A-Za-z_$][\w$]*)\.(body|query|params)\b").unwrap(),
            Regex::new(r"\b([A-Za-z_$][\w$]*)\.(body|query|params)\.([A-Za-z_$][\w$]*)").unwrap(),
            Regex::new(r#"\b([A-Za-z_$][\w$]*)\.(body|query|params)\[\s*['"]([^'"]+)['"]\s*\]"#).unwrap(),
        ]
    });
    let Some(r) = i.istek.as_deref() else { return vec![] };
    let mut out = vec![];
    for c in yapi.captures_iter(&i.govde).filter(|c| &c[2] == r) {
        let bolum = bolum_adi(&c[3]);
        out.extend(yapi_alanlari(&c[1]).into_iter().map(|(a, v)| (bolum, a, v)));
    }
    for c in uye.captures_iter(&i.govde).chain(koseli.captures_iter(&i.govde)).filter(|c| &c[1] == r) {
        // Yalnızca denetlenen (`if (req.body.x)`, `typeof req.body.x`) ya da yedeği olan (`req.body.x || ...`, `?? ...`)
        // alan isteğe bağlıdır; boş varsayılan bunu işaretler (Yorum olarak kalır)
        let (b, a) = (regex::escape(&c[2]), regex::escape(&c[3]));
        let kosullu =
            crate::kural::derle(&format!(r"if\s*\(\s*!?\s*{r}\.{b}\.{a}\b|typeof\s+{r}\.{b}\.{a}\b|{r}\.{b}\.{a}\s*(?:\|\||\?\?)|{r}\.{b}\?\.{a}\b", r = regex::escape(r)))
                .is_match(&i.govde);
        out.push((bolum_adi(&c[2]), c[3].to_string(), kosullu.then(String::new)));
    }
    out
}

/// İşleyicinin isteği argüman verdiği yerel yardımcı fonksiyonların okuduğu alanlar (`function kip(req) { req.body.mode }`)
fn yardimci_girdileri(p: &Proje, di: usize, i: &Isleyici) -> Vec<(&'static str, String, Option<String>)> {
    let Some(r) = i.istek.as_deref() else { return vec![] };
    let cagri = crate::kural::derle(&format!(r"\b([A-Za-z_$][\w$]*)\(\s*{}\s*[,)]", regex::escape(r)));
    let mut out = vec![];
    let mut gorulen = HashSet::new();
    for c in cagri.captures_iter(&i.govde) {
        let ad = c[1].to_string();
        if !gorulen.insert(ad.clone()) {
            continue;
        }
        if let Some(f) = p.fonksiyon(di, &ad).filter(|f| f.istek.is_some()) {
            out.extend(kullanim_girdileri(f));
        }
    }
    out
}

/// İşleyicide zod şemasıyla doğrulanan girdiler: (bölüm, şema adı, alanlar). Şema projedeki bir `z.object` sabitidir.
fn zod_girdileri(p: &Proje, i: &Isleyici) -> Vec<(&'static str, String, BTreeMap<String, Alan>)> {
    static D: OnceLock<Regex> = OnceLock::new();
    let d =
        D.get_or_init(|| Regex::new(r"\b([A-Za-z_$][\w$]*)\.(?:parse|safeParse|parseAsync|safeParseAsync|validate|validateAsync)\(\s*([A-Za-z_$][\w$]*)\.(body|query)\b").unwrap());
    let Some(r) = i.istek.as_deref() else { return vec![] };
    d.captures_iter(&i.govde)
        .filter(|c| &c[2] == r)
        .filter_map(|c| {
            let alanlar = p.dosyalar.iter().find_map(|d| d.zod.get(&c[1]))?.clone();
            Some((bolum_adi(&c[3]), c[1].to_string(), alanlar))
        })
        .collect()
}

/// Next.js işleyicisinin girdileri: `await request.json()` yapı bozumu ve `searchParams.get('x')`
fn next_girdileri(i: &Isleyici) -> Vec<(&'static str, String, Option<String>)> {
    static D: OnceLock<[Regex; 3]> = OnceLock::new();
    let [yapi, degisken, sorgu] = D.get_or_init(|| {
        [
            Regex::new(r"(?:const|let|var)\s*\{([^}]*)\}\s*=\s*await\s+([A-Za-z_$][\w$]*)\.json\(\)").unwrap(),
            Regex::new(r"(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*await\s+([A-Za-z_$][\w$]*)\.json\(\)").unwrap(),
            Regex::new(r#"searchParams\.get\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap(),
        ]
    });
    let mut out = vec![];
    if let Some(r) = i.istek.as_deref() {
        for c in yapi.captures_iter(&i.govde).filter(|c| &c[2] == r) {
            out.extend(yapi_alanlari(&c[1]).into_iter().map(|(a, v)| ("govde", a, v)));
        }
        // const body = await req.json(); body.x
        for c in degisken.captures_iter(&i.govde).filter(|c| &c[2] == r) {
            let onek = format!("{}.", &c[1]);
            let mut kalan = i.govde.as_str();
            while let Some(j) = kalan.find(&onek) {
                let once_harf = kalan[..j].chars().last().is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '$' || ch == '.');
                let sonra: String = kalan[j + onek.len()..].chars().take_while(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '$').collect();
                if !once_harf && !sonra.is_empty() {
                    out.push(("govde", sonra, None));
                }
                kalan = &kalan[j + onek.len()..];
            }
        }
    }
    for c in sorgu.captures_iter(&i.govde) {
        out.push(("sorgu", c[1].to_string(), None));
    }
    out
}

/// Varsayılan değerin metninden tip (yalnızca sabit değerler)
fn varsayilan_tipi(v: &str) -> Option<String> {
    let v = v.trim();
    if v.parse::<f64>().is_ok() {
        Some("number".into())
    } else if v == "true" || v == "false" {
        Some("boolean".into())
    } else if v.starts_with(['\'', '"', '`']) {
        Some("string".into())
    } else if v.starts_with('[') {
        Some("array".into())
    } else {
        None
    }
}

/// Gövdedeki SQL ve Prisma kullanımı [Yorum]
fn veri_bagi(i: &Isleyici, prisma: &HashMap<String, String>) -> Option<VeriBag> {
    static D: OnceLock<[Regex; 3]> = OnceLock::new();
    let [okuma, yazma, prisma_d] = D.get_or_init(|| {
        [
            Regex::new(r#"(?i)\b(?:FROM|JOIN)\s+["`\[]?([A-Za-z_][\w]*)"#).unwrap(),
            Regex::new(r#"(?i)\b(?:INSERT\s+(?:OR\s+\w+\s+)?INTO|UPDATE|DELETE\s+FROM|REPLACE\s+INTO)\s+["`\[]?([A-Za-z_][\w]*)"#).unwrap(),
            Regex::new(r"\bprisma\.([a-zA-Z_]\w*)\.(\w+)\(").unwrap(),
        ]
    });
    // SQL anahtar kelimeleri ve sık JavaScript sözcükleri tablo adı sayılmaz
    let anahtar = ["select", "where", "set", "values", "the", "a", "an", "to", "in", "on", "and", "or", "if", "require", "import", "react"];
    let mut bag = VeriBag::default();
    // Yorumlardaki düz metin ("get the items from our database") SQL sayılmaz: satır ve blok yorumları atılır
    static Y: OnceLock<Regex> = OnceLock::new();
    let yorumsuz = Y.get_or_init(|| Regex::new(r"(?s)/\*.*?\*/|(?m)(?:^|[^:])//[^\n]*").unwrap()).replace_all(&i.govde, " ");
    let govde = yorumsuz.as_ref();
    for c in yazma.captures_iter(govde) {
        if !anahtar.contains(&c[1].to_lowercase().as_str()) {
            bag.yazar.push(c[1].to_string());
        }
    }
    for c in okuma.captures_iter(govde) {
        // DELETE FROM'daki FROM okuma sayılmaz
        let once = &govde[..c.get(0).map(|m| m.start()).unwrap_or(0)];
        if once.trim_end().to_uppercase().ends_with("DELETE") || anahtar.contains(&c[1].to_lowercase().as_str()) {
            continue;
        }
        bag.okur.push(c[1].to_string());
    }
    for c in prisma_d.captures_iter(govde) {
        let model = prisma.get(&c[1]).cloned().unwrap_or_else(|| c[1].to_string());
        if c[2].starts_with("find") || ["count", "aggregate", "groupBy"].contains(&&c[2]) {
            bag.okur.push(model);
        } else {
            bag.yazar.push(model);
        }
    }
    if bag.okur.is_empty() && bag.yazar.is_empty() {
        return None;
    }
    // SQL metinden okunur: dinamik sorgular (birleştirilen tablo adları) eksik kalabilir
    bag.eksik_olabilir = true;
    bag.okur.sort();
    bag.okur.dedup();
    bag.yazar.sort();
    bag.yazar.dedup();
    Some(bag)
}

/// Kullanımdan çıkan alanlar -> girdiler (zorunluluk Yorum)
fn kullanim_semasi(alanlar: &[(&'static str, String, Option<String>)], bolum: &str, dosya: &str, satir: usize) -> Vec<Girdi> {
    let mut m: BTreeMap<String, Alan> = BTreeMap::new();
    for (_, ad, v) in alanlar.iter().filter(|(b, _, _)| *b == bolum) {
        let tip = v.as_deref().and_then(varsayilan_tipi);
        m.entry(ad.clone()).or_insert(Alan { tip, zorunlu: v.is_none(), belirsiz: true, kurallar: vec![], satir, kalitimdan: None, degerler: vec![], ic: None });
    }
    if m.is_empty() {
        return vec![];
    }
    let dto = if bolum == "govde" { "istek gövdesi (kullanımdan)" } else { "sorgu (kullanımdan)" };
    vec![Girdi {
        ad: None,
        tip: None,
        sema: Some(Sema { adaylar: vec![], dizi: false, dto: dto.into(), dosya: Some(dosya.to_string()), bulunamadi: false, kalitim: None, alanlar: m }),
    }]
}

fn fonksiyon_adi(yontem: &str, yol: &str) -> String {
    let mut s = yontem.to_lowercase();
    for p in yol.split('/').filter(|p| !p.is_empty()) {
        let p = p.trim_start_matches(':');
        let mut ilk = true;
        for c in p.chars().filter(|c| c.is_alphanumeric()) {
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

/// Next.js `app/` altındaki dosya yolundan rota: (grup) atlanır, [id] -> :id, [...x] -> :x
fn next_yolu(kok: &Path, yol: &Path) -> Option<String> {
    let goreli = yol.strip_prefix(kok).ok()?;
    let parcalar: Vec<String> = goreli.parent()?.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    let i = parcalar.iter().rposition(|p| p == "app")?;
    let mut out = vec![];
    for p in &parcalar[i + 1..] {
        if (p.starts_with('(') && p.ends_with(')')) || p.starts_with('@') {
            continue;
        }
        if p.starts_with('[') && p.ends_with(']') {
            out.push(format!(":{}", p.trim_matches(['[', ']']).trim_start_matches("...")));
        } else {
            out.push(p.clone());
        }
    }
    Some(format!("/{}", out.join("/")))
}

/// Next.js ara katmanı (`middleware.ts`; Next 16'da `proxy.ts`), kökte ya da `src/` altında. Her rota dosyasından önce
/// çalışır; `config.matcher` hangi yollarda çalıştığını söyler (yoksa her yolda).
struct NextAra {
    dosya: String,
    /// Gövde kimlik denetimi yapıyor görünüyor [Yorum]
    yetki: bool,
    /// Gövde yolu kendisi ayırıyor (`pathname.startsWith(...)`, `createRouteMatcher`): kapsam belirlenemez
    kosullu: bool,
    /// None: matcher okunamadı (değişken, has / missing koşulu, çözülemeyen desen): hiçbir rota için karar verilmez
    eslesme: Option<Vec<Eslesme>>,
}

enum Eslesme {
    Desen(Regex),
    /// `/((?!api|_next/static|favicon.ico).*)`: önekten sonra dışlananlardan biriyle başlamayan her yol
    Haric(String, Vec<Regex>),
}

impl NextAra {
    fn bul(kok: &Path) -> Option<NextAra> {
        let yol = ["", "src"]
            .iter()
            .flat_map(|t| ["middleware", "proxy"].iter().flat_map(move |a| ["ts", "js", "mjs"].iter().map(move |u| kok.join(t).join(format!("{a}.{u}")))))
            .find(|p| p.is_file())?;
        let metin = guvenli::oku(&yol)?;
        let yetki = [
            "next-auth/middleware",
            "withAuth(",
            "getToken(",
            ".protect(",
            "jwtVerify(",
            "jwt.verify(",
            "verifyToken(",
            "as middleware",
            "as proxy",
            "export default auth",
            "status: 401",
            "Unauthorized",
        ]
        .iter()
        .any(|x| metin.contains(x))
            || (metin.contains("redirect(") && crate::kural::derle(r"(?i)['`/](login|signin|sign-in|giris|auth)\b").is_match(&metin));
        let kosullu = ["startsWith(", "createRouteMatcher(", "matchesPath", ".test(pathname", "pathname.includes(", "pathname ==="].iter().any(|x| metin.contains(x));
        Some(NextAra { dosya: guvenli::goreli(kok, &yol), yetki, kosullu, eslesme: Self::eslesmeler(&metin) })
    }

    /// `export const config = { matcher: ... }`: metin, metin dizisi ya da `{ source }` nesneleri
    fn eslesmeler(metin: &str) -> Option<Vec<Eslesme>> {
        let Some(i) = metin.find("matcher") else { return Some(vec![]) };
        let kalan = metin[i + "matcher".len()..].trim_start().strip_prefix(':')?.trim_start();
        let deger = if kalan.starts_with('[') {
            let mut derinlik = 0;
            let son = kalan.char_indices().find(|(_, c)| {
                match c {
                    '[' | '{' => derinlik += 1,
                    ']' | '}' => derinlik -= 1,
                    _ => {}
                }
                derinlik == 0
            })?;
            &kalan[1..son.0]
        } else {
            kalan
        };
        // Koşullu eşleşme (has / missing) isteğe bağlıdır: kapsam belirlenemez
        if deger.contains("has:") || deger.contains("missing:") {
            return None;
        }
        let dizgi = crate::kural::derle(r#"^\s*(?:\{\s*source\s*:\s*)?(?:'([^']*)'|"([^"]*)"|`([^`$]*)`)\s*\}?\s*$"#);
        let parcalar: Vec<&str> = if kalan.starts_with('[') { deger.split(',').filter(|p| !p.trim().is_empty()).collect() } else { vec![deger.split([',', '}', '\n']).next()?] };
        parcalar
            .iter()
            .map(|p| {
                let c = dizgi.captures(p)?;
                Self::desen(c.get(1).or(c.get(2)).or(c.get(3))?.as_str())
            })
            .collect()
    }

    /// path-to-regexp kaynağı: `/api/:yol*`, `/a/:id`, `/:x+`, `/:x?`, `/((?!api|_next).*)`
    fn desen(s: &str) -> Option<Eslesme> {
        let s = s.replace("\\\\", "\\");
        if let Some(i) = s.find("((?!") {
            let govde = s[i + 4..].strip_suffix(").*)")?;
            // Yalnızca üst düzey `|` ayırır: `.*\.(?:png|jpg)` tek seçenektir
            let (mut secenekler, mut derinlik, mut bas) = (vec![], 0i32, 0);
            for (i, c) in govde.char_indices() {
                match c {
                    '(' => derinlik += 1,
                    ')' => derinlik -= 1,
                    '|' if derinlik == 0 => {
                        secenekler.push(&govde[bas..i]);
                        bas = i + 1;
                    }
                    _ => {}
                }
            }
            secenekler.push(&govde[bas..]);
            let haric = secenekler.iter().map(|a| crate::kural::derle_dene(&format!("^(?:{a})"))).collect::<Option<Vec<_>>>()?;
            return Some(Eslesme::Haric(s[..i].to_string(), haric));
        }
        let mut d = String::from("^");
        let k: Vec<char> = s.chars().collect();
        let mut j = 0;
        while j < k.len() {
            if k[j] == '/' && k.get(j + 1) == Some(&':') {
                j += 2;
                while j < k.len() && (k[j].is_alphanumeric() || k[j] == '_') {
                    j += 1;
                }
                match k.get(j) {
                    Some('*') => d.push_str("(?:/.*)?"),
                    Some('+') => d.push_str("/.+"),
                    Some('?') => d.push_str("(?:/[^/]+)?"),
                    _ => {
                        d.push_str("/[^/]+");
                        continue;
                    }
                }
                j += 1;
            } else if k[j] == '(' {
                // Ham düzenli ifade grubu olduğu gibi
                let mut derinlik = 0;
                while j < k.len() {
                    d.push(k[j]);
                    match k[j] {
                        '(' => derinlik += 1,
                        ')' => derinlik -= 1,
                        _ => {}
                    }
                    j += 1;
                    if derinlik == 0 {
                        break;
                    }
                }
            } else {
                d.push_str(&regex::escape(&k[j].to_string()));
                j += 1;
            }
        }
        d.push_str("/?$");
        crate::kural::derle_dene(&d).map(Eslesme::Desen)
    }

    /// Rota yolu bu ara katmanın kapsamında mı (None: belirlenemedi). Dinamik bölüm örnek değerle sınanır.
    fn kapsar(&self, yol: &str) -> Option<bool> {
        let es = self.eslesme.as_ref()?;
        if es.is_empty() {
            return Some(true);
        }
        let ornek: String = yol.split('/').map(|p| if p.starts_with(':') { "x" } else { p }).collect::<Vec<_>>().join("/");
        Some(es.iter().any(|e| match e {
            Eslesme::Desen(r) => r.is_match(&ornek),
            Eslesme::Haric(onek, haric) => ornek.strip_prefix(onek.as_str()).is_some_and(|k| !haric.iter().any(|h| h.is_match(k))),
        }))
    }

    /// Kimlik denetimi bu rotaya uygulanıyor mu: yalnızca yetki görünen, yolu gövdede ayırmayan ve kapsamı okunan ara katman
    fn giris_ister(&self, yol: &str) -> bool {
        self.yetki && !self.kosullu && self.kapsar(yol) == Some(true)
    }

    fn ozet(&self) -> String {
        match (self.kosullu, self.eslesme.is_some()) {
            (false, true) => format!("{} [Yorum]", self.dosya),
            (true, _) => format!("{} [Yorum; yolu kendisi ayırıyor, kapsam belirlenemedi]", self.dosya),
            (false, false) => format!("{} [Yorum; matcher okunamadı, kapsam belirlenemedi]", self.dosya),
        }
    }
}

pub(crate) fn topla(kok: &Path, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<Dosya>>, t0: Instant) -> crate::Toplam {
    let mut yollar = vec![];
    let (mut atlanan, mut buyuk) = (0usize, vec![]);
    let mut prisma_metinleri = vec![];
    let mut okunan_sema = vec![];
    let mut kaskadlar: Vec<Value> = vec![];
    for g in ignore::WalkBuilder::new(kok).hidden(true).git_ignore(true).build().flatten() {
        let p = g.path();
        if g.path_is_symlink() || !p.is_file() {
            if g.path_is_symlink() {
                atlanan += 1;
            }
            continue;
        }
        let goreli = p.strip_prefix(kok).unwrap_or(p);
        let ad = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let test = ad.contains(".test.") || ad.contains(".spec.") || ad.ends_with(".d.ts");
        if test
            || goreli
                .components()
                .any(|c| ["node_modules", "dist", "build", ".next", "coverage", "__tests__", "test", "tests", "public", "out"].contains(&c.as_os_str().to_str().unwrap_or("")))
        {
            atlanan += 1;
            continue;
        }
        match p.extension().and_then(|e| e.to_str()) {
            Some(e) if crate::izle::JS_UZANTILARI.contains(&e) => {
                if g.metadata().is_ok_and(|m| m.len() > crate::EN_BUYUK_DOSYA) {
                    buyuk.push(guvenli::goreli(kok, p));
                } else {
                    yollar.push(p.to_path_buf());
                }
            }
            Some("prisma") => {
                let m = guvenli::oku(p).unwrap_or_default();
                okunan_sema.push((p.to_path_buf(), crate::kesif::icerik_ozeti(m.as_bytes())));
                kaskadlar.extend(crate::dna::prisma_kaskadlari(&guvenli::goreli(kok, p), &m).into_iter().map(|(e, c, k)| json!([e, c, k])));
                prisma_metinleri.push(m);
            }
            _ => atlanan += 1,
        }
    }
    let t_kesif = t0.elapsed();
    // Değişmeyen dosya önbellekten (sezgi calistir): yalnızca değişen dosya yeniden ayrıştırılır
    let (sonuclar, mut okunan, ayristirilan) = crate::kesif::ayristir_hepsi(&yollar, onbellek, |p, b| ayristir(p, std::str::from_utf8(&b).ok()?));
    let dosyalar: Vec<Dosya> = sonuclar.into_iter().map(|(_, d)| d).collect();
    okunan.extend(okunan_sema);
    let t_ayristirma = t0.elapsed();
    // Prisma: istemci erişimcisi (kullanici) -> model adı (Kullanici)
    let mut prisma: HashMap<String, String> = HashMap::new();
    let model_d = Regex::new(r"(?m)^\s*model\s+([A-Za-z_]\w*)\s*\{").unwrap();
    for m in &prisma_metinleri {
        for c in model_d.captures_iter(m) {
            let ad = c[1].to_string();
            let mut erisimci: String = ad.chars().take(1).flat_map(|c| c.to_lowercase()).collect();
            erisimci.push_str(&ad[ad.chars().next().map(|c| c.len_utf8()).unwrap_or(0)..]);
            prisma.insert(erisimci, ad);
        }
    }
    let (kapilar, mut bilgi) = kapilar(kok, &dosyalar, &prisma);
    bilgi["kaskadlar"] = json!(kaskadlar);
    let t_toplam = t0.elapsed();
    crate::Toplam {
        kapilar,
        bilgi,
        adaptor: format!("express@{SURUM}"),
        dosya_sayisi: dosyalar.len(),
        satir: dosyalar.iter().map(|d| d.satir_sayisi).sum(),
        kismi: dosyalar.iter().filter(|d| d.hatali).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        atlanan,
        buyuk,
        ek_paketler: vec![],
        veri_dosyalari: vec![],
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}

/// Gezilecek rota nesnesi: (dosya, nesne, önek, miras ara katmanlar, ana uygulamada mı)
type Ziyaret = (usize, String, String, Vec<(usize, Ara)>, bool);

fn kapilar(kok: &Path, dosyalar: &[Dosya], prisma: &HashMap<String, String>) -> (Vec<Kapi>, Value) {
    let p = Proje { kok, dosyalar, dizin: dosyalar.iter().enumerate().map(|(i, d)| (normal(&d.yol), i)).collect() };
    // Bağlanan router'lar: (dosya, nesne)
    let mut baglanan: HashSet<(usize, String)> = HashSet::new();
    for (di, d) in dosyalar.iter().enumerate() {
        for b in &d.baglamalar {
            for h in &b.hedefler {
                if let Some(x) = p.rota_nesnesi(di, h) {
                    baglanan.insert(x);
                }
            }
        }
    }
    // Kökler: express()/fastify() nesneleri; hiç yoksa bağlanmamış bütün router'lar (önek bilinmez)
    let mut kokler: Vec<(usize, String)> = dosyalar.iter().enumerate().flat_map(|(di, d)| d.nesneler.iter().filter(|(_, kok)| **kok).map(move |(a, _)| (di, a.clone()))).collect();
    kokler.sort();
    let kok_var = !kokler.is_empty();
    let mut kuyruk: Vec<Ziyaret> = kokler.iter().map(|(di, a)| (*di, a.clone(), String::new(), vec![], true)).collect();
    if !kok_var {
        for (di, d) in dosyalar.iter().enumerate() {
            for a in d.nesneler.keys() {
                if !baglanan.contains(&(di, a.clone())) {
                    kuyruk.push((di, a.clone(), String::new(), vec![], true));
                }
            }
        }
    }
    let mut ulasilan: HashSet<(usize, String, String)> = HashSet::new();
    let mut ulasilan_nesne: HashSet<(usize, String)> = HashSet::new();
    let mut kapilar = vec![];
    // Önizlemenin sınayamadığı girdiler (dosya yüklemesi), kapının "dosya:satır" anahtarıyla
    let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
    let mut dna_ekle = |k: &Kapi, yer: Option<(usize, &Isleyici)>| {
        let anahtar = format!("{}:{}", k.dosya, k.satir);
        if let Some((di, i)) = yer.filter(|_| !dna_haritasi.contains_key(&anahtar)) {
            let v = dna_zinciri(&p, di, i);
            if !crate::dna::bos(&v) {
                dna_haritasi.insert(anahtar, v);
            }
        }
    };
    let mut not_ekle = |k: &Kapi, not: Option<String>| {
        if let Some(n) = not {
            let l = denetlenemeyen.entry(format!("{}:{}", k.dosya, k.satir)).or_default();
            if !l.contains(&n) {
                l.push(n);
            }
        }
    };
    while let Some((di, nesne, onek, miras, ana)) = kuyruk.pop() {
        // Aynı nesne aynı önekle bir kez gezilir (döngü ve tekrar bağlamaya karşı)
        if !ulasilan.insert((di, nesne.clone(), onek.clone())) || ulasilan.len() > 10_000 {
            continue;
        }
        ulasilan_nesne.insert((di, nesne.clone()));
        let d = &dosyalar[di];
        // Nesne düzeyinde yolsuz `use(mw)`: kendisinden sonraki rotalara uygulanır
        let genel: Vec<&Baglama> = d.baglamalar.iter().filter(|b| b.nesne == nesne && b.onek.is_empty()).collect();
        let ara_oncesi = |sira: usize| -> Vec<(usize, Ara)> {
            let mut v = miras.clone();
            for b in genel.iter().filter(|b| b.sira < sira) {
                v.extend(b.cagrilar.iter().cloned().map(|a| (di, a)));
                v.extend(b.hedefler.iter().filter(|h| !h.starts_with("fonksiyon:") && p.rota_nesnesi(di, h).is_none()).map(|h| (di, Ara::Ad(h.clone()))));
            }
            v
        };
        for r in d.rotalar.iter().filter(|r| r.nesne == nesne) {
            let mut ara = ara_oncesi(r.sira);
            // Yollu ara katman (`router.use('/:type', authMiddleware)`): yolu öneke uyan sonraki rotalara uygulanır
            for b in d.baglamalar.iter().filter(|b| b.nesne == nesne && !b.onek.is_empty() && b.sira < r.sira && yol_uyar(&b.onek, &r.yol)) {
                if b.hedefler.iter().all(|h| p.rota_nesnesi(di, h).is_none()) {
                    ara.extend(b.cagrilar.iter().cloned().map(|a| (di, a)));
                    ara.extend(b.hedefler.iter().filter(|h| !h.starts_with("fonksiyon:")).map(|h| (di, Ara::Ad(h.clone()))));
                }
            }
            ara.extend(r.ara.iter().cloned().map(|a| (di, a)));
            let isleyici = r.isleyici.clone().or_else(|| r.isleyici_adi.as_deref().and_then(|a| p.fonksiyon(di, a)).cloned());
            let yol = yol_normal(&yol_birlestir(&onek, &r.yol));
            let k = kapi_kur(&p, di, r.yontem.clone(), yol, &ara, isleyici.as_ref(), &r.sema, r.satir, r.ozet.clone(), r.isleyici_adi.clone(), ana, prisma);
            dna_ekle(&k, isleyici_yeri(&p, di, r));
            not_ekle(&k, dosya_notu(&ara, isleyici.as_ref()));
            kapilar.push(k);
        }
        // Bağlamalar: hedef router ise yeni önekle kuyruğa
        for b in d.baglamalar.iter().filter(|b| b.nesne == nesne) {
            let mut ara = ara_oncesi(b.sira);
            ara.extend(b.cagrilar.iter().cloned().map(|a| (di, a)));
            for h in &b.hedefler {
                match p.rota_nesnesi(di, h) {
                    Some((m, a)) => kuyruk.push((m, a, yol_birlestir(&onek, &b.onek), ara.clone(), ana)),
                    None if !h.starts_with("fonksiyon:") => ara.push((di, Ara::Ad(h.clone()))),
                    None => {}
                }
            }
        }
    }
    // Kök varken hiçbir kökten ulaşılamayan router'lar: ikincil (önekleri bilinmiyor)
    if kok_var {
        for (di, d) in dosyalar.iter().enumerate() {
            for a in d.nesneler.keys() {
                if ulasilan_nesne.contains(&(di, a.clone())) {
                    continue;
                }
                for r in d.rotalar.iter().filter(|r| &r.nesne == a) {
                    let isleyici = r.isleyici.clone().or_else(|| r.isleyici_adi.as_deref().and_then(|x| p.fonksiyon(di, x)).cloned());
                    let ara: Vec<(usize, Ara)> = r.ara.iter().cloned().map(|x| (di, x)).collect();
                    let mut k = kapi_kur(
                        &p,
                        di,
                        r.yontem.clone(),
                        yol_normal(&yol_birlestir("", &r.yol)),
                        &ara,
                        isleyici.as_ref(),
                        &r.sema,
                        r.satir,
                        r.ozet.clone(),
                        r.isleyici_adi.clone(),
                        false,
                        prisma,
                    );
                    k.uygulama = Some("bağlanmamış router (önek bilinmiyor)".into());
                    dna_ekle(&k, isleyici_yeri(&p, di, r));
                    not_ekle(&k, dosya_notu(&ara, isleyici.as_ref()));
                    kapilar.push(k);
                }
            }
        }
    }
    // Next.js rota dosyaları; ara katman (middleware.ts) yalnızca rota dosyası varsa aranır
    let next_ara = dosyalar.iter().any(|d| !d.next_yontemleri.is_empty()).then(|| NextAra::bul(kok)).flatten();
    for (di, d) in dosyalar.iter().enumerate() {
        let Some(yol) = (!d.next_yontemleri.is_empty()).then(|| next_yolu(kok, &d.yol)).flatten() else { continue };
        for (y, f, satir) in &d.next_yontemleri {
            let girdiler = next_girdileri(f);
            let dosya = guvenli::goreli(kok, &d.yol);
            let (cagri_yetkisi, roller) = next_yetki(&p, di, f);
            // İşleyicinin kendi 401'i yetki kanıtı değildir (giriş ucu yanlış şifrede 401 döner): yalnızca yetki yardımcıları
            let yetkili = cagri_yetkisi || ["getServerSession", "auth()", "getToken", "currentUser"].iter().any(|x| f.govde.contains(x));
            // Ara katmanın kimlik denetimi kapsadığı yola uygulanır (Yorum)
            let yetkili = yetkili || next_ara.as_ref().is_some_and(|a| a.giris_ister(&yol));
            let not = dosya_notu(&[], Some(f));
            let k = Kapi {
                id: format!("{y} {yol}"),
                yontem: y.clone(),
                yol: yol.clone(),
                kontrolcu: next_kontrolcu(&yol),
                fonksiyon: y.to_lowercase(),
                dosya: dosya.clone(),
                satir: *satir,
                giris: if yetkili { "gerekli".into() } else { "herkese_acik".into() },
                guardlar: vec![],
                roller,
                roller_kaniti: Some("varsayim".into()),
                ozet: None,
                govde: kullanim_semasi(&girdiler, "govde", &dosya, *satir),
                sorgu: kullanim_semasi(&girdiler, "sorgu", &dosya, *satir),
                parametreler: yol.split('/').filter_map(|p| p.strip_prefix(':')).map(|a| Girdi { ad: Some(a.to_string()), tip: Some("string".into()), sema: None }).collect(),
                kullanimdan_kalkmis: false,
                yerine: None,
                surum: None,
                uygulama: None,
                ana: true,
                giris_kaniti: if yetkili { "varsayim_bilinmeyen_guard".into() } else { "varsayim".into() },
                veri: veri_bagi(f, prisma),
                yanit: yanit_cikarimi(&f.govde, &dosya),
            };
            dna_ekle(&k, Some((di, f)));
            not_ekle(&k, not);
            kapilar.push(k);
        }
    }
    kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
    kapilar.dedup_by(|a, b| a.id == b.id && a.fonksiyon == b.fonksiyon && a.dosya == b.dosya);
    let kok_dosya = kokler.first().map(|(di, a)| format!("{}:{a}", guvenli::goreli(kok, &dosyalar[*di].yol)));
    let genel: Vec<String> = next_ara.iter().filter(|a| a.yetki).map(NextAra::ozet).collect();
    (kapilar, json!({ "genel_onek": "", "genel_guardlar": genel, "ana_uygulama": kok_dosya, "surum_secimi": null, "denetlenemeyen": denetlenemeyen, "dna": dna_haritasi }))
}

/// İşlemin davranış zinciri: işleyici ve gövdesinde adıyla çağrılan proje fonksiyonları (`deleteArticle(...)`,
/// `service.sil(...)`; 3 kat). `di`: işleyicinin tanımlandığı dosya
fn dna_zinciri(p: &Proje, di: usize, i: &Isleyici) -> Value {
    let mut halkalar = vec![];
    let mut gorulen: HashSet<(usize, usize)> = HashSet::new();
    let mut yigin: Vec<(usize, &Isleyici, usize)> = vec![(di, i, 0)];
    while let Some((di, i, derinlik)) = yigin.pop() {
        if !gorulen.insert((di, i as *const Isleyici as usize)) || halkalar.len() >= 32 {
            continue;
        }
        halkalar.push(crate::dna::Halka { dosya: guvenli::goreli(p.kok, &p.dosyalar[di].yol), dna: &i.dna, derinlik, tur: if derinlik == 0 { "isleyici" } else { "servis" } });
        if derinlik >= 3 {
            continue;
        }
        let mut sonraki = vec![];
        for ad in &i.dna.cagrilar {
            if let Some((m, yerel)) = p.coz(di, ad, 0) {
                if let Some(g) = p.dosyalar[m].fonksiyonlar.get(&yerel) {
                    sonraki.push((m, g, derinlik + 1));
                }
            }
        }
        yigin.extend(sonraki.into_iter().rev());
    }
    crate::dna::ozet(&halkalar)
}

/// İşleyicinin tanımlandığı dosya ve kendisi: gömülü fonksiyon ya da adıyla çözülen
fn isleyici_yeri<'a>(p: &Proje<'a>, di: usize, r: &'a Rota) -> Option<(usize, &'a Isleyici)> {
    match &r.isleyici {
        Some(i) => Some((di, i)),
        None => {
            let (m, yerel) = p.coz(di, r.isleyici_adi.as_deref()?, 0)?;
            p.dosyalar[m].fonksiyonlar.get(&yerel).map(|i| (m, i))
        }
    }
}

/// Next.js işleyicisinin içindeki yetki çağrıları (`await requireCan("DUZENLE")`): adı yetki gösteren ya da tanımı
/// 401 / 403 / Unauthorized döndüren yardımcı; metin argümanları rol sayılır [Yorum]
fn next_yetki(p: &Proje, di: usize, f: &Isleyici) -> (bool, Vec<String>) {
    static D: OnceLock<[Regex; 2]> = OnceLock::new();
    let [cagri, metin_arg] = D.get_or_init(|| [Regex::new(r"await\s+([A-Za-z_$][\w$]*)\(([^)]*)\)").unwrap(), Regex::new(r#"['"]([^'"]+)['"]"#).unwrap()]);
    let ad_desen = yetki_ad_deseni();
    let mut yetkili = false;
    let mut roller = vec![];
    for c in cagri.captures_iter(&f.govde) {
        let tanim = p.fonksiyon(di, &c[1]).map(|g| g.govde.as_str()).unwrap_or("");
        let tanim_yetki = ["401", "403", "Unauthorized", "Forbidden"].iter().any(|x| tanim.contains(x));
        if ad_desen.is_match(&c[1]) || tanim_yetki {
            yetkili = true;
            roller.extend(metin_arg.captures_iter(&c[2]).map(|m| m[1].to_string()));
        }
    }
    roller.sort();
    roller.dedup();
    (yetkili, roller)
}

fn next_kontrolcu(yol: &str) -> String {
    let ilk = yol.split('/').find(|p| !p.is_empty() && !p.starts_with(':') && *p != "api").unwrap_or("api");
    ilk.chars().filter(|c| c.is_alphanumeric()).collect()
}

#[allow(clippy::too_many_arguments)]
fn kapi_kur(
    p: &Proje,
    di: usize,
    yontem: String,
    yol: String,
    ara: &[(usize, Ara)],
    isleyici: Option<&Isleyici>,
    sema: &[(&'static str, BTreeMap<String, Alan>)],
    satir: usize,
    ozet: Option<String>,
    isleyici_adi: Option<String>,
    ana: bool,
    prisma: &HashMap<String, String>,
) -> Kapi {
    let d = &p.dosyalar[di];
    let dosya = guvenli::goreli(p.kok, &d.yol);
    let (mut giris, mut istege) = (false, false);
    let mut roller = vec![];
    let mut guardlar = vec![];
    for (adi, a) in ara {
        let (y, r) = yetki(p, *adi, a);
        // İsteğe bağlı kimlik (`auth.optional`, express-jwt `credentialsRequired: false`): istek kimliksiz de geçer
        if y && istege_bagli_mi(p, *adi, a) {
            istege = true;
        } else {
            giris |= y;
        }
        roller.extend(r);
        guardlar.push(guvenli::temiz(a.ad()));
    }
    roller.sort();
    roller.dedup();
    let mut girdiler = isleyici.map(kullanim_girdileri).unwrap_or_default();
    if let Some(i) = isleyici {
        girdiler.extend(yardimci_girdileri(p, di, i));
    }
    let mut govde = kullanim_semasi(&girdiler, "govde", &dosya, satir);
    let mut sorgu = kullanim_semasi(&girdiler, "sorgu", &dosya, satir);
    // Zod ile doğrulanan girdi (`girisSema.parse(req.body)`): şema tanımdır, kullanımdan çıkanın yerine geçer
    if let Some(i) = isleyici {
        for (bolum, ad, alanlar) in zod_girdileri(p, i) {
            let g = vec![Girdi {
                ad: None,
                tip: None,
                sema: Some(Sema { adaylar: vec![], dizi: false, dto: ad, dosya: Some(dosya.clone()), bulunamadi: false, kalitim: None, alanlar }),
            }];
            if bolum == "govde" {
                govde = g;
            } else {
                sorgu = g;
            }
        }
    }
    // Fastify rota şeması tanımdır: kullanımdan çıkanın yerine geçer
    for (bolum, alanlar) in sema {
        let g = vec![Girdi {
            ad: None,
            tip: None,
            sema: Some(Sema {
                adaylar: vec![],
                dizi: false,
                dto: format!("schema.{}", if *bolum == "govde" { "body" } else { "querystring" }),
                dosya: Some(dosya.clone()),
                bulunamadi: false,
                kalitim: None,
                alanlar: alanlar.clone(),
            }),
        }];
        if *bolum == "govde" {
            govde = g;
        } else {
            sorgu = g;
        }
    }
    let kontrolcu: String = d.yol.file_stem().and_then(|s| s.to_str()).unwrap_or("app").chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
    let kontrolcu = if kontrolcu == "index" { d.yol.parent().and_then(|x| x.file_name()).and_then(|s| s.to_str()).unwrap_or("app").to_string() } else { kontrolcu };
    let fonksiyon = isleyici_adi
        .as_deref()
        .map(|a| a.rsplit('.').next().unwrap_or(a).to_string())
        .or_else(|| isleyici.and_then(|i| i.ad.clone()).filter(|a| a != "default"))
        .unwrap_or_else(|| fonksiyon_adi(&yontem, &yol));
    let yanit = isleyici.and_then(|i| yanit_cikarimi(&i.govde, &dosya));
    Kapi {
        id: format!("{yontem} {yol}"),
        parametreler: yol.split('/').filter_map(|x| x.strip_prefix(':')).map(|a| Girdi { ad: Some(a.to_string()), tip: Some("string".into()), sema: None }).collect(),
        yontem,
        yol,
        kontrolcu,
        fonksiyon,
        dosya,
        satir,
        giris: if giris {
            "gerekli".into()
        } else if istege {
            "istege_bagli".into()
        } else {
            "herkese_acik".into()
        },
        guardlar,
        roller,
        roller_kaniti: Some("varsayim".into()),
        ozet,
        govde,
        sorgu,
        kullanimdan_kalkmis: false,
        yerine: None,
        surum: None,
        uygulama: None,
        ana,
        // Ara katmanın yetkisi addan ya da tanımdaki 401'den çıkarılır: kesin değildir
        giris_kaniti: if giris || istege { "varsayim_bilinmeyen_guard".into() } else { "varsayim".into() },
        veri: isleyici.and_then(|i| veri_bagi(i, prisma)),
        yanit,
    }
}

/// Yanıt biçimi, yalnızca yorum: işleyicideki başarılı `res.json({ a, b: x })` / `res.status(201).json({...})` nesne
/// sabitinin anahtarları (değer tipi bilinmez). Birden fazla farklı başarılı yanıt nesnesi varsa tek anlamlı değildir: şema yok
fn yanit_cikarimi(govde: &str, dosya: &str) -> Option<crate::adaptor::Yanit> {
    static DESEN: OnceLock<Regex> = OnceLock::new();
    let re = DESEN.get_or_init(|| Regex::new(r"res(?:\.status\(\s*(\d{3})\s*\))?\s*\.(?:json|send)\(\s*\{([^{}]*)\}\s*\)").expect("desen"));
    let mut bulunan: Vec<(Option<u16>, BTreeMap<String, Alan>)> = vec![];
    for c in re.captures_iter(govde) {
        let durum = c.get(1).and_then(|s| s.as_str().parse::<u16>().ok());
        if durum.is_some_and(|d| !(200..300).contains(&d)) {
            continue;
        }
        let mut alanlar = BTreeMap::new();
        for p in c[2].split(',').map(str::trim).filter(|p| !p.is_empty()) {
            if p.starts_with("...") {
                return None;
            }
            let ad = p.split(':').next().unwrap_or(p).trim().trim_matches(|c| c == '"' || c == '\'');
            if !ad.is_empty() && ad.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
                alanlar.insert(ad.to_string(), Alan { tip: None, zorunlu: true, belirsiz: true, kurallar: vec![], satir: 0, kalitimdan: None, degerler: vec![], ic: None });
            }
        }
        if !alanlar.is_empty() && !bulunan.iter().any(|(_, a)| a.keys().eq(alanlar.keys())) {
            bulunan.push((durum, alanlar));
        }
    }
    if bulunan.len() != 1 {
        return None;
    }
    let (durum, alanlar) = bulunan.pop()?;
    Some(crate::adaptor::Yanit {
        kaynak: "res.json".into(),
        kesin: false,
        durum,
        sema: Sema { adaylar: vec![], dizi: false, dto: "res.json".into(), dosya: Some(dosya.to_string()), bulunamadi: false, kalitim: None, alanlar },
    })
}
