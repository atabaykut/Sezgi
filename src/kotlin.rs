//! Kotlin okuyucusu: Kotlin kaynaklarını Spring adaptörünün dil bağımsız sınıf modeline (`spring::Dosya`) çevirir.
//! Spring Boot (Kotlin) bu modelle Java ile aynı adaptörden geçer; Ktor adaptörü ayrıca yönlendirme DSL'ini
//! (`routing { route("/x") { get { } } }`) çağrı ağacı olarak okur.
//! - **Model:** sınıf, arayüz, `data class`, `enum class`, `object` ve `companion object` (üyeleri statik sayılır);
//!   kurucu özellikleri (`class X(val a: T)`) alandır. Dosyanın üst düzey fonksiyon ve özellikleri, Kotlin'in JVM'deki
//!   gibi `<Dosya>Kt` sınıfındadır. Tipler Java karşılığına çevrilir (`Int` -> `Integer`, `MutableList<T>` -> `List<T>`,
//!   `Array<T>` -> `T[]`); boş olabilirlik (`T?`) ve varsayılan değer ayrıca saklanır.
//! - **Anotasyon argümanları** Java ifadesine çevrilir: dizgi şablonu `"$API/x"` -> `API + "/x"`, `[a, b]` ve
//!   `arrayOf(...)` -> `{a, b}`, `X::class` -> `X.class`; kullanım yeri hedefi (`@field:`) ayrı tutulur.
//! - **Gövde:** yerel tipler (yazılan tip, kurucu çağrısı, `call.receive<T>()`, `by inject<T>()`), çağrılar ve davranış
//!   özeti: `throw X(...)`, `?: throw` (koşul: sol taraf boş), `orElseThrow { }`, `ResponseEntity.status(...)`, Ktor
//!   `call.respond(HttpStatusCode.X)`, yumuşak silme ataması, olay ve mesaj gönderimleri.
//! - **Spring Security Kotlin DSL'i** (`http { authorizeHttpRequests { authorize("/x", permitAll) } }`) güvenlik
//!   zincirinin okuduğu çağrı biçimine çevrilir.
//! - **Exposed:** `object T : Table("ad")` / `IntIdTable(...)` tablo modelidir; sütunlar (`varchar("ad", 50)`) ve
//!   `reference(..., onDelete = ReferenceOption.CASCADE)` kaskadı; DAO varlığı (`companion object : IntEntityClass<X>(T)`).
//! - Gramerin ayrıştıramadığı bölgeler (`ERROR`) dosyanın kısmi satırlarıdır; içlerindeki sınıf başlığı ve üyeler
//!   kurtarılır.
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Node, Parser};

use crate::dna::{Dal, Firlatma, GovdeDna};
use crate::spring::{Anotasyon, Cagri, Degisken, Dosya, Sinif, Yontem};

/// Bir gövdede tutulacak en fazla öğe
const EN_FAZLA: usize = 32;
/// Ktor HTTP fiilleri: yönlendirme DSL'inde işleyici kaydı
pub(crate) const FIILLER: [(&str, &str); 7] =
    [("get", "GET"), ("post", "POST"), ("put", "PUT"), ("patch", "PATCH"), ("delete", "DELETE"), ("head", "HEAD"), ("options", "OPTIONS")];
/// Exposed tablo tabanları
const EXPOSED_TABLOLARI: [&str; 8] = ["Table", "IdTable", "IntIdTable", "LongIdTable", "UUIDTable", "UIntIdTable", "ULongIdTable", "CompositeIdTable"];
/// Kotlin'in anahtar sözcükleri: `ad { }` biçiminde olsalar da çağrı değildir
const ANAHTAR: [&str; 19] =
    ["if", "else", "try", "catch", "finally", "when", "do", "while", "for", "fun", "object", "class", "interface", "init", "return", "companion", "constructor", "enum", "in"];

/// Ktor yönlendirme DSL'indeki bir çağrı (`route("/x") { }`, `get { }`, `authenticate("jwt") { }`, `kullaniciRotalari()`)
#[derive(Clone, Debug, Default)]
pub struct DslCagri {
    pub ad: String,
    /// Alıcı metni (`call`, `this`; yoksa boş)
    pub nesne: String,
    /// Argümanlar Java ifadesi olarak ((ad, değer); konumsal argümanın adı yok)
    pub argumanlar: Vec<(Option<String>, String)>,
    pub tip_argumanlari: Vec<String>,
    pub satir: usize,
    /// Sondaki lambdası var mı
    pub lambda: bool,
    /// Lambdanın içindeki DSL çağrıları
    pub cocuklar: Vec<DslCagri>,
    /// HTTP fiilinin işleyicisi: fonksiyonun sınıfındaki yöntemin dizini
    pub isleyici: Option<usize>,
    /// `if` / `when` / döngü içinde: kayıt koşullu
    pub kosullu: bool,
}

/// Gövdesinde DSL çağrısı olan fonksiyon (`fun Route.gorevRotalari(servis: GorevServisi) { ... }`)
#[derive(Clone, Debug, Default)]
pub struct DslFonk {
    pub ad: String,
    /// Uzantı alıcısı (`Route`, `Routing`, `Application`; yoksa boş)
    pub alici: String,
    /// Fonksiyonun bulunduğu sınıfın dizini (dosyanın `<Dosya>Kt` sınıfı dahil)
    pub sinif: usize,
    pub parametre_sayisi: usize,
    /// Fonksiyon tipli parametreler (`build: Route.() -> Unit`): sarmalayıcının içeriği bunlarla çağrılır
    pub lambda_parametreleri: Vec<String>,
    pub satir: usize,
    pub cagrilar: Vec<DslCagri>,
}

/// Kotlin dosyasının modeli ve (istenirse) yönlendirme DSL'i
#[derive(Clone, Debug, Default)]
pub struct KtDosya {
    pub model: Dosya,
    pub dsl: Vec<DslFonk>,
}

// ---- Düğüm yardımcıları ----

fn metin(n: Node, k: &[u8]) -> String {
    n.utf8_text(k).unwrap_or("").to_string()
}

fn satir(n: Node) -> usize {
    n.start_position().row + 1
}

fn yorum_mu(n: Node) -> bool {
    matches!(n.kind(), "line_comment" | "block_comment")
}

/// Adlı çocuklar (yorumlar hariç)
fn adlilar(n: Node) -> Vec<Node> {
    let mut w = n.walk();
    n.named_children(&mut w).filter(|c| !yorum_mu(*c)).collect()
}

/// Bütün çocuklar (belirteçler dahil)
fn tumu(n: Node) -> Vec<Node> {
    let mut w = n.walk();
    n.children(&mut w).collect()
}

fn tek_satir(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tip_mi(tur: &str) -> bool {
    matches!(tur, "user_type" | "nullable_type" | "function_type" | "parenthesized_type" | "non_nullable_type")
}

/// Tip adının son parçası: "org.x.Foo<Bar>" -> "Foo"
fn son_ad(t: &str) -> String {
    let t = t.split('<').next().unwrap_or(t);
    t.rsplit('.').next().unwrap_or(t).trim().to_string()
}

// ---- Tipler ----

/// Kotlin tip metnini Java karşılığına çevirir: (tip, boş olabilir mi)
pub(crate) fn tip_cevir(t: &str) -> (String, bool) {
    let t = tek_satir(t);
    let t = t.trim();
    let (t, bos) = match t.strip_suffix('?') {
        Some(x) => (x.trim(), true),
        None => (t, false),
    };
    let t = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')).filter(|x| !x.contains("->")).unwrap_or(t);
    (tip_ic(t), bos)
}

fn tip_ic(t: &str) -> String {
    let t = t.trim().trim_end_matches('?').trim();
    // Tipteki anotasyonlar (`@Valid Foo`) atılır
    let t = if t.starts_with('@') { crate::spring::anotasyonsuz(t) } else { t.to_string() };
    let t = t.trim();
    if t.contains("->") {
        return "Function".into();
    }
    let (taban, argumanlar) = match t.find('<') {
        Some(i) if t.ends_with('>') => (&t[..i], crate::spring::ust_duzey_bol(&t[i + 1..t.len() - 1])),
        _ => (t, vec![]),
    };
    let taban = taban.trim();
    let son = taban.rsplit('.').next().unwrap_or(taban);
    let ic: Vec<String> = argumanlar
        .iter()
        .map(|a| {
            let a = a.trim();
            let a = a.strip_prefix("out ").or_else(|| a.strip_prefix("in ")).unwrap_or(a);
            if a == "*" {
                "?".into()
            } else {
                tip_ic(a)
            }
        })
        .collect();
    if ic.is_empty() {
        let tekil = match son {
            "Int" => "Integer",
            "Long" | "Short" | "Byte" | "Double" | "Float" | "Boolean" => son,
            "Char" => "Character",
            "Any" => "Object",
            "Unit" | "Nothing" => "void",
            "IntArray" => "int[]",
            "LongArray" => "long[]",
            "ShortArray" => "short[]",
            "ByteArray" => "byte[]",
            "DoubleArray" => "double[]",
            "FloatArray" => "float[]",
            "BooleanArray" => "boolean[]",
            "CharArray" => "char[]",
            _ => "",
        };
        if !tekil.is_empty() {
            return tekil.into();
        }
    }
    if son == "Array" && ic.len() == 1 {
        return format!("{}[]", ic[0]);
    }
    let yeni = match son {
        "MutableList" | "ArrayList" | "Sequence" => "List",
        "MutableSet" | "HashSet" | "LinkedHashSet" => "Set",
        "MutableCollection" => "Collection",
        "MutableMap" | "HashMap" | "LinkedHashMap" => "Map",
        "MutableIterable" => "Iterable",
        // Kotlin eşyordam akışı: JSON dizisi (Flux gibi); ertelenmiş değer (Mono gibi)
        "Flow" => "Flux",
        "Deferred" => "Mono",
        _ => taban,
    };
    if ic.is_empty() {
        yeni.to_string()
    } else {
        format!("{}<{}>", yeni, ic.join(", "))
    }
}

// ---- İfadeler ----

/// Dizgi sabitini Java ifadesine çevirir: `"$API/x"` -> `API + "/x"`, `"${Yol.A}/b"` -> `Yol.A + "/b"`
fn dizgi(n: Node, k: &[u8]) -> String {
    // Kaçışlı `\$` şablon değildir: geçici bir işaretle saklanır
    const KACIS: char = '\u{E000}';
    let mut parcalar: Vec<(bool, String)> = vec![];
    let mut ham = String::new();
    fn bosalt(ham: &mut String, parcalar: &mut Vec<(bool, String)>) {
        let b: Vec<char> = ham.chars().collect();
        let mut lit = String::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == '$' && b.get(i + 1).is_some_and(|c| c.is_alphabetic() || *c == '_') {
                let mut j = i + 1;
                while j < b.len() && (b[j].is_alphanumeric() || b[j] == '_') {
                    j += 1;
                }
                if !lit.is_empty() {
                    parcalar.push((false, std::mem::take(&mut lit)));
                }
                parcalar.push((true, b[i + 1..j].iter().collect()));
                i = j;
                continue;
            }
            lit.push(if b[i] == KACIS { '$' } else { b[i] });
            i += 1;
        }
        if !lit.is_empty() {
            parcalar.push((false, lit));
        }
        ham.clear();
    }
    for c in tumu(n) {
        match c.kind() {
            "string_content" => ham.push_str(&metin(c, k)),
            "escape_sequence" => {
                let e = metin(c, k);
                let ch = match e.as_str() {
                    "\\n" => '\n',
                    "\\t" => '\t',
                    "\\r" => '\r',
                    "\\b" => '\u{8}',
                    "\\$" => KACIS,
                    x if x.starts_with("\\u") => u32::from_str_radix(&x[2..], 16).ok().and_then(char::from_u32).unwrap_or('?'),
                    x => x.chars().nth(1).unwrap_or(' '),
                };
                ham.push(ch);
            }
            "interpolation" => {
                bosalt(&mut ham, &mut parcalar);
                parcalar.push((true, adlilar(c).first().map(|x| tek_satir(&metin(*x, k))).unwrap_or_default()));
            }
            _ => {}
        }
    }
    bosalt(&mut ham, &mut parcalar);
    if parcalar.is_empty() {
        return "\"\"".into();
    }
    parcalar.into_iter().map(|(ifade, m)| if ifade { m } else { java_dizgisi(&m) }).collect::<Vec<_>>().join(" + ")
}

fn java_dizgisi(m: &str) -> String {
    format!("\"{}\"", m.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t"))
}

/// Kotlin ifadesini Spring adaptörünün çözdüğü Java ifadesine çevirir (sabit izleme, anotasyon argümanları)
pub(crate) fn java_ifadesi(n: Node, k: &[u8]) -> String {
    match n.kind() {
        "string_literal" | "multiline_string_literal" => dizgi(n, k),
        "collection_literal" => format!("{{{}}}", adlilar(n).into_iter().map(|c| java_ifadesi(c, k)).collect::<Vec<_>>().join(", ")),
        "call_expression" => {
            let p = cagri_parcalari(n);
            match (p.callee.filter(|c| c.kind() == "identifier").map(|c| metin(c, k)).as_deref(), p.argumanlar) {
                (Some("arrayOf" | "listOf" | "setOf"), Some(va)) => format!("{{{}}}", argumanlar(va, k).into_iter().map(|(_, v)| v).collect::<Vec<_>>().join(", ")),
                _ => tek_satir(&metin(n, k)),
            }
        }
        "callable_reference" => tek_satir(&metin(n, k)).replace("::class.java", ".class").replace("::class", ".class"),
        "parenthesized_expression" => adlilar(n).first().map(|c| java_ifadesi(*c, k)).unwrap_or_default(),
        "binary_expression" if n.child_by_field_name("operator").is_some_and(|o| o.kind() == "+") => {
            let (l, r) = (n.child_by_field_name("left"), n.child_by_field_name("right"));
            format!("{} + {}", l.map(|x| java_ifadesi(x, k)).unwrap_or_default(), r.map(|x| java_ifadesi(x, k)).unwrap_or_default())
        }
        _ => tek_satir(&metin(n, k)),
    }
}

/// Çağrının parçaları: çağrılan ifade, tip argümanları, değer argümanları, sondaki lambda. `get("/x") { }` iki katlı
/// çağrıdır (içteki çağrı ve lambda): tek çağrı olarak döner
#[derive(Clone, Copy, Default)]
struct Parcalar<'t> {
    callee: Option<Node<'t>>,
    tipler: Option<Node<'t>>,
    argumanlar: Option<Node<'t>>,
    lambda: Option<Node<'t>>,
}

fn cagri_parcalari(n: Node) -> Parcalar {
    let mut p = Parcalar::default();
    for c in adlilar(n) {
        match c.kind() {
            "annotated_lambda" => p.lambda = adlilar(c).into_iter().find(|x| x.kind() == "lambda_literal"),
            "value_arguments" => p.argumanlar = Some(c),
            "type_arguments" => p.tipler = Some(c),
            _ => p.callee = Some(c),
        }
    }
    if p.lambda.is_some() && p.argumanlar.is_none() && p.tipler.is_none() {
        if let Some(ic) = p.callee.filter(|c| c.kind() == "call_expression") {
            let q = cagri_parcalari(ic);
            if q.lambda.is_none() {
                return Parcalar { lambda: p.lambda, ..q };
            }
        }
    }
    p
}

/// Çağrılanın (alıcı, ad) çifti: `repo.save` -> ("repo", "save"), `get` -> ("", "get"); alıcı boşluksuz
fn cagri_adi(c: Node, k: &[u8]) -> (String, String) {
    match c.kind() {
        "identifier" => (String::new(), metin(c, k)),
        "navigation_expression" => {
            let cocuk = adlilar(c);
            let ad = cocuk.last().filter(|x| x.kind() == "identifier").map(|x| metin(*x, k)).unwrap_or_default();
            let nesne: String = cocuk.first().filter(|_| cocuk.len() > 1).map(|x| metin(*x, k)).unwrap_or_default().chars().filter(|c| !c.is_whitespace() && *c != '?').collect();
            (nesne, ad)
        }
        _ => (metin(c, k).chars().filter(|c| !c.is_whitespace()).collect(), String::new()),
    }
}

fn tip_argumanlari(ta: Option<Node>, k: &[u8]) -> Vec<String> {
    ta.map(adlilar).unwrap_or_default().into_iter().filter(|x| x.kind() == "type_projection").map(|x| tip_cevir(&metin(x, k)).0).collect()
}

/// Değer argümanları: (ad, Java ifadesi)
fn argumanlar(va: Node, k: &[u8]) -> Vec<(Option<String>, String)> {
    adlilar(va)
        .into_iter()
        .filter(|v| v.kind() == "value_argument")
        .map(|v| {
            let cocuk = adlilar(v);
            let adli = tumu(v).iter().any(|x| x.kind() == "=");
            match (adli, cocuk.first(), cocuk.last()) {
                (true, Some(a), Some(d)) if cocuk.len() >= 2 => (Some(metin(*a, k)), java_ifadesi(*d, k)),
                (_, _, Some(d)) => (None, java_ifadesi(*d, k)),
                _ => (None, String::new()),
            }
        })
        .collect()
}

/// Argümanların ham metinleri (davranış özeti için; adlı argümanın yalnızca değeri)
fn ham_argumanlar<'t>(va: Option<Node<'t>>) -> Vec<Node<'t>> {
    va.map(adlilar).unwrap_or_default().into_iter().filter(|v| v.kind() == "value_argument").filter_map(|v| adlilar(v).last().copied()).collect()
}

// ---- Anotasyonlar ----

fn anotasyon_oku(n: Node, k: &[u8]) -> Option<Anotasyon> {
    let mut a = Anotasyon { satir: satir(n), ..Default::default() };
    for c in adlilar(n) {
        match c.kind() {
            "use_site_target" => a.hedef = Some(metin(c, k).trim_end_matches(':').trim().to_string()),
            "user_type" => a.ad = son_ad(&metin(c, k)),
            "constructor_invocation" => {
                for x in adlilar(c) {
                    match x.kind() {
                        "user_type" => a.ad = son_ad(&metin(x, k)),
                        "value_arguments" => a.argumanlar = argumanlar(x, k),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    (!a.ad.is_empty()).then_some(a)
}

/// Düğümün anotasyonları ve değiştirici sözcükleri (`data`, `const`, `abstract`, `suspend`...)
fn degistiriciler(n: Node, k: &[u8]) -> (Vec<Anotasyon>, Vec<String>) {
    let (mut a, mut s) = (vec![], vec![]);
    for c in adlilar(n) {
        match c.kind() {
            "modifiers" => {
                for m in adlilar(c) {
                    if m.kind() == "annotation" {
                        a.extend(anotasyon_oku(m, k));
                    } else {
                        s.push(metin(m, k).trim().to_string());
                    }
                }
            }
            "annotation" => a.extend(anotasyon_oku(c, k)),
            _ => {}
        }
    }
    (a, s)
}

/// KDoc'un ilk cümlesi: bildirimden hemen önceki `/** ... */`
fn kdoc(n: Node, k: &[u8]) -> Option<String> {
    let onceki = n.prev_named_sibling().filter(|p| p.kind() == "block_comment")?;
    let m = metin(onceki, k);
    let govde = m.strip_prefix("/**")?.strip_suffix("*/")?;
    let satirlar: Vec<String> = govde.lines().map(|l| l.trim().trim_start_matches('*').trim().to_string()).take_while(|l| !l.starts_with('@')).filter(|l| !l.is_empty()).collect();
    let tum = satirlar.join(" ");
    let ilk = tum.split(". ").next().unwrap_or(&tum).trim().trim_end_matches('.').to_string();
    (!ilk.is_empty()).then_some(ilk)
}

// ---- Okuyucu ----

/// Çağrı ağacında lambdalı çağrı var mı
fn lambdali(cagrilar: &[DslCagri]) -> bool {
    cagrilar.iter().any(|c| c.lambda || lambdali(&c.cocuklar))
}

struct Okuyucu<'a> {
    k: &'a [u8],
    /// Ktor yönlendirme DSL'i de okunsun mu
    dsl: bool,
    siniflar: Vec<Sinif>,
    fonklar: Vec<DslFonk>,
}

impl Okuyucu<'_> {
    /// Sınıf, arayüz ya da nesne bildirimi. Dizini önceden ayrılır: işleyiciler sınıfın yöntemlerine eklenir
    fn sinif_oku(&mut self, n: Node) {
        let k = self.k;
        let (anotasyonlar, soz) = degistiriciler(n, k);
        let nesne = n.kind() == "object_declaration";
        let arayuz = tumu(n).iter().any(|c| c.kind() == "interface");
        let tur = if arayuz {
            "interface"
        } else if soz.iter().any(|x| x == "enum") {
            "enum"
        } else if soz.iter().any(|x| x == "annotation") {
            "annotation"
        } else {
            "class"
        };
        let ad_dugum = n.child_by_field_name("name");
        let i = self.siniflar.len();
        self.siniflar.push(Sinif::default());
        let mut s = Sinif {
            ad: ad_dugum.map(|a| metin(a, k)).unwrap_or_default(),
            tur,
            anotasyonlar,
            satir: ad_dugum.map(satir).unwrap_or_else(|| satir(n)),
            soyut: arayuz || soz.iter().any(|x| x == "abstract" || x == "sealed"),
            ..Default::default()
        };
        for c in adlilar(n) {
            match c.kind() {
                "type_parameters" => {
                    s.tip_parametreleri = adlilar(c).into_iter().filter_map(|p| adlilar(p).into_iter().find(|x| x.kind() == "identifier").map(|x| metin(x, k))).collect();
                }
                "primary_constructor" => self.kurucu(c, &mut s),
                "delegation_specifiers" => self.ustler(c, &mut s),
                "class_body" | "enum_class_body" => self.govde(c, &mut s, i, nesne),
                _ => {}
            }
        }
        if nesne {
            for a in &mut s.alanlar {
                a.statik = true;
            }
        }
        exposed_tablosu(&mut s);
        self.siniflar[i] = s;
    }

    /// Kurucu özellikleri (`val` / `var`) alandır
    fn kurucu(&self, n: Node, s: &mut Sinif) {
        let k = self.k;
        for cp in adlilar(n).into_iter().flat_map(adlilar).filter(|x| x.kind() == "class_parameter") {
            if !tumu(cp).iter().any(|x| matches!(x.kind(), "val" | "var")) {
                continue;
            }
            let (anotasyonlar, _) = degistiriciler(cp, k);
            let cocuk = tumu(cp);
            let Some(ad) = cocuk.iter().find(|x| x.kind() == "identifier").map(|x| metin(*x, k)) else { continue };
            let (tip, bos) = cocuk.iter().find(|x| tip_mi(x.kind())).map(|x| tip_cevir(&metin(*x, k))).unwrap_or_default();
            let deger = cocuk.iter().position(|x| x.kind() == "=").and_then(|i| cocuk.get(i + 1)).map(|x| java_ifadesi(*x, k));
            s.alanlar.push(Degisken {
                ad,
                tip,
                anotasyonlar,
                satir: satir(cp),
                kotlin_zorunlu: !bos && deger.is_none(),
                bos_olabilir: bos,
                deger,
                kurucu: true,
                ..Default::default()
            });
        }
    }

    /// Üst tipler: kurucu çağrısıyla yazılan üst sınıf, gerisi arayüz. Exposed tablosu ve DAO varlık sınıfı işaretlenir
    fn ustler(&self, n: Node, s: &mut Sinif) {
        let k = self.k;
        for d in adlilar(n).into_iter().filter(|x| x.kind() == "delegation_specifier") {
            for c in adlilar(d) {
                match c.kind() {
                    "constructor_invocation" => {
                        let tip = adlilar(c).into_iter().find(|x| x.kind() == "user_type").map(|x| tip_cevir(&metin(x, k)).0).unwrap_or_default();
                        let args = adlilar(c).into_iter().find(|x| x.kind() == "value_arguments").map(|x| argumanlar(x, k)).unwrap_or_default();
                        let temel = son_ad(&tip);
                        if EXPOSED_TABLOLARI.contains(&temel.as_str()) {
                            let ad = args.iter().find(|(a, _)| a.as_deref().is_none_or(|a| a == "name")).map(|(_, v)| v.clone());
                            s.anotasyonlar.push(Anotasyon {
                                ad: "ExposedTable".into(),
                                argumanlar: ad.map(|v| (Some("name".into()), v)).into_iter().collect(),
                                satir: s.satir,
                                hedef: None,
                            });
                        }
                        if s.tur == "interface" || s.ust.is_some() {
                            s.arayuzler.push(tip);
                        } else {
                            s.ust = Some(tip);
                        }
                    }
                    "user_type" => s.arayuzler.push(tip_cevir(&metin(c, k)).0),
                    "explicit_delegation" => s.arayuzler.extend(adlilar(c).into_iter().find(|x| tip_mi(x.kind())).map(|x| tip_cevir(&metin(x, k)).0)),
                    _ => {}
                }
            }
        }
    }

    /// Sınıf gövdesinin üyeleri; iç sınıflar ayrıca listelenir. `statik`: nesne ya da eşlik eden nesne gövdesi
    fn govde(&mut self, n: Node, s: &mut Sinif, si: usize, statik: bool) {
        let k = self.k;
        for m in adlilar(n) {
            match m.kind() {
                "function_declaration" => {
                    let y = self.fonksiyon(m, s, si);
                    s.yontemler.push(y);
                }
                "property_declaration" => s.alanlar.extend(self.ozellik(m, statik)),
                "companion_object" => {
                    // Exposed DAO: `companion object : IntEntityClass<Gorev>(Gorevler)`
                    for d in adlilar(m).into_iter().filter(|x| x.kind() == "delegation_specifiers").flat_map(adlilar) {
                        for c in adlilar(d).into_iter().filter(|x| x.kind() == "constructor_invocation") {
                            let tip = adlilar(c).into_iter().find(|x| x.kind() == "user_type").map(|x| metin(x, k)).unwrap_or_default();
                            let tablo = adlilar(c).into_iter().find(|x| x.kind() == "value_arguments").and_then(|x| argumanlar(x, k).into_iter().next()).map(|(_, v)| v);
                            if let Some(t) = tablo.filter(|_| son_ad(&tip).ends_with("EntityClass")) {
                                s.anotasyonlar.push(Anotasyon { ad: "ExposedEntity".into(), argumanlar: vec![(Some("table".into()), t)], satir: satir(m), hedef: None });
                            }
                        }
                    }
                    if let Some(g) = adlilar(m).into_iter().find(|x| x.kind() == "class_body") {
                        let bas = s.alanlar.len();
                        self.govde(g, s, si, true);
                        for a in &mut s.alanlar[bas..] {
                            a.statik = true;
                        }
                    }
                }
                "class_declaration" | "object_declaration" => self.sinif_oku(m),
                "enum_entry" => s.sabitler.extend(adlilar(m).into_iter().find(|x| x.kind() == "identifier").map(|x| metin(x, k))),
                _ => {}
            }
        }
    }

    /// Özellik bildirimi: tip yazılmamışsa ilk değerden çıkarılır
    fn ozellik(&self, n: Node, statik: bool) -> Option<Degisken> {
        let k = self.k;
        let (anotasyonlar, soz) = degistiriciler(n, k);
        let vd = adlilar(n).into_iter().find(|x| x.kind() == "variable_declaration")?;
        let ad = adlilar(vd).into_iter().find(|x| x.kind() == "identifier").map(|x| metin(x, k))?;
        let cocuk = tumu(n);
        let ilk = cocuk.iter().position(|x| x.kind() == "=").and_then(|i| cocuk.get(i + 1)).copied();
        let vekil = cocuk.iter().find(|x| x.kind() == "property_delegate").copied();
        let (tip, bos) = match adlilar(vd).into_iter().find(|x| tip_mi(x.kind())) {
            Some(t) => tip_cevir(&metin(t, k)),
            None => (ilk.or(vekil).and_then(|i| cikarim(i, k)).unwrap_or_default(), false),
        };
        Some(Degisken {
            ad,
            tip,
            anotasyonlar,
            satir: satir(vd),
            statik: statik || soz.iter().any(|x| x == "const"),
            deger: ilk.map(|i| java_ifadesi(i, k)),
            bos_olabilir: bos,
            ..Default::default()
        })
    }

    /// Fonksiyon bildirimi; Ktor DSL'i istenmişse gövdedeki yönlendirme çağrıları da okunur
    fn fonksiyon(&mut self, n: Node, s: &mut Sinif, si: usize) -> Yontem {
        let k = self.k;
        let (anotasyonlar, _) = degistiriciler(n, k);
        let ad_dugum = n.child_by_field_name("name");
        let mut y = Yontem {
            ad: ad_dugum.map(|a| metin(a, k)).unwrap_or_default(),
            anotasyonlar,
            satir: ad_dugum.map(satir).unwrap_or_else(|| satir(n)),
            belge: kdoc(n, k),
            ..Default::default()
        };
        let (mut alici, mut donus, mut govde, mut parametreler) = (String::new(), None, None, None);
        let mut ad_gecti = false;
        for c in tumu(n) {
            match c.kind() {
                "function_value_parameters" => parametreler = Some(c),
                "function_body" => govde = Some(c),
                x if tip_mi(x) => {
                    if !ad_gecti {
                        alici = son_ad(&metin(c, k));
                    } else if parametreler.is_some() {
                        donus = Some(c);
                    }
                }
                _ if Some(c.id()) == ad_dugum.map(|a| a.id()) => ad_gecti = true,
                _ => {}
            }
        }
        let mut lambda_parametreleri = vec![];
        if let Some(pn) = parametreler {
            let ps = tumu(pn);
            let mut bekleyen: Vec<Anotasyon> = vec![];
            for (i, c) in ps.iter().enumerate() {
                match c.kind() {
                    "parameter_modifiers" => bekleyen.extend(adlilar(*c).into_iter().filter(|x| x.kind() == "annotation").filter_map(|x| anotasyon_oku(x, k))),
                    "parameter" => {
                        let cocuk = adlilar(*c);
                        let ad = cocuk.iter().find(|x| x.kind() == "identifier").map(|x| metin(*x, k)).unwrap_or_default();
                        let tip_n = cocuk.iter().find(|x| tip_mi(x.kind()));
                        if tip_n.is_some_and(|t| t.kind() == "function_type") {
                            lambda_parametreleri.push(ad.clone());
                        }
                        let (tip, bos) = tip_n.map(|t| tip_cevir(&metin(*t, k))).unwrap_or_default();
                        let deger = (ps.get(i + 1).is_some_and(|x| x.kind() == "=")).then(|| ps.get(i + 2).map(|x| tek_satir(&metin(*x, k)))).flatten();
                        let anotasyonlar = std::mem::take(&mut bekleyen);
                        y.parametreler.push(Degisken {
                            ad,
                            tip,
                            anotasyonlar,
                            satir: satir(*c),
                            kotlin_zorunlu: !bos && deger.is_none(),
                            bos_olabilir: bos,
                            deger,
                            ..Default::default()
                        });
                    }
                    _ => {}
                }
            }
        }
        for p in &y.parametreler {
            y.yereller.insert(p.ad.clone(), p.tip.clone());
        }
        let ifade_govdesi = govde.and_then(|g| adlilar(g).into_iter().find(|x| x.kind() != "block"));
        y.donus = match (donus, ifade_govdesi) {
            (Some(t), _) => tip_cevir(&metin(t, k)).0,
            (None, Some(e)) => cikarim(e, k).unwrap_or_default(),
            (None, None) => "void".into(),
        };
        if let Some(b) = govde {
            y.govdeli = true;
            let m = metin(b, k);
            y.govde_metni = if m.contains("authorize") { guvenlik_dsl(&m) } else { m };
            let mut yereller = std::mem::take(&mut y.yereller);
            let (mut cagrilar, mut kaynaklar) = (vec![], HashMap::new());
            y.dna = govde_oku(b, k, &mut yereller, &mut cagrilar, &mut kaynaklar);
            y.yereller = yereller;
            y.cagrilar = cagrilar;
            y.kaynak_cagrilari = kaynaklar;
            if self.dsl {
                let cagrilar = self.dsl_oku(b, s, &y, false);
                // Yalnızca DSL kurabilecek fonksiyonlar: uzantı alıcısı ya da lambdalı çağrı
                if !cagrilar.is_empty() && (!alici.is_empty() || lambdali(&cagrilar)) {
                    self.fonklar.push(DslFonk { ad: y.ad.clone(), alici, sinif: si, parametre_sayisi: y.parametreler.len(), lambda_parametreleri, satir: y.satir, cagrilar });
                }
            }
        }
        y
    }

    // ---- Ktor yönlendirme DSL'i ----

    /// Kaptaki (blok, lambda, fonksiyon gövdesi) DSL çağrıları
    fn dsl_oku(&mut self, kap: Node, s: &mut Sinif, ust: &Yontem, kosullu: bool) -> Vec<DslCagri> {
        let mut v = vec![];
        for c in adlilar(kap) {
            self.dsl_ifade(c, s, ust, kosullu, &mut v);
        }
        v
    }

    fn dsl_ifade(&mut self, n: Node, s: &mut Sinif, ust: &Yontem, kosullu: bool, v: &mut Vec<DslCagri>) {
        match n.kind() {
            "call_expression" => {
                // `embeddedServer(...) { routing { } }.start()`: alıcıdaki çağrı da okunur
                if let Some(ic) =
                    cagri_parcalari(n).callee.filter(|c| c.kind() == "navigation_expression").and_then(|c| adlilar(c).first().copied()).filter(|c| c.kind() == "call_expression")
                {
                    self.dsl_ifade(ic, s, ust, kosullu, v);
                }
                v.extend(self.dsl_cagri(n, s, ust, kosullu));
            }
            // `exception<X> { }`, `get<Kaynak> { }`: gramer tip argümanlı lambdalı çağrıyı karşılaştırma olarak okur
            "binary_expression" => v.extend(self.dsl_genel_cagri(n, s, ust, kosullu)),
            "if_expression" | "when_expression" | "when_entry" | "for_statement" | "while_statement" | "do_while_statement" => {
                for c in adlilar(n) {
                    self.dsl_ifade(c, s, ust, true, v);
                }
            }
            "block"
            | "function_body"
            | "try_expression"
            | "catch_block"
            | "finally_block"
            | "parenthesized_expression"
            | "labeled_expression"
            | "annotated_expression"
            | "return_expression" => {
                for c in adlilar(n) {
                    self.dsl_ifade(c, s, ust, kosullu, v);
                }
            }
            _ => {}
        }
    }

    fn dsl_cagri(&mut self, n: Node, s: &mut Sinif, ust: &Yontem, kosullu: bool) -> Option<DslCagri> {
        let k = self.k;
        let p = cagri_parcalari(n);
        let (nesne, ad) = cagri_adi(p.callee?, k);
        let mut d = DslCagri {
            ad,
            nesne,
            argumanlar: p.argumanlar.map(|a| argumanlar(a, k)).unwrap_or_default(),
            tip_argumanlari: tip_argumanlari(p.tipler, k),
            satir: satir(n),
            lambda: p.lambda.is_some(),
            kosullu,
            ..Default::default()
        };
        if let Some(l) = p.lambda {
            self.lambda_isle(&mut d, l, s, ust);
        }
        Some(d)
    }

    fn dsl_genel_cagri(&mut self, n: Node, s: &mut Sinif, ust: &Yontem, kosullu: bool) -> Option<DslCagri> {
        let k = self.k;
        let (sol, op, sag) = (n.child_by_field_name("left")?, n.child_by_field_name("operator")?, n.child_by_field_name("right")?);
        if op.kind() != ">" || sag.kind() != "lambda_literal" || sol.kind() != "binary_expression" || sol.child_by_field_name("operator")?.kind() != "<" {
            return None;
        }
        let (nesne, ad) = cagri_adi(sol.child_by_field_name("left")?, k);
        let tip = tip_cevir(&metin(sol.child_by_field_name("right")?, k)).0;
        let mut d = DslCagri { ad, nesne, tip_argumanlari: vec![tip], satir: satir(n), lambda: true, kosullu, ..Default::default() };
        self.lambda_isle(&mut d, sag, s, ust);
        Some(d)
    }

    /// Lambda: HTTP fiilinde işleyici (sınıfa yöntem olarak eklenir), değilse içindeki DSL çağrıları
    fn lambda_isle(&mut self, d: &mut DslCagri, l: Node, s: &mut Sinif, ust: &Yontem) {
        let k = self.k;
        let fiil = FIILLER.iter().any(|(f, _)| *f == d.ad) || d.ad == "handle";
        if !fiil || (!d.nesne.is_empty() && d.nesne != "this") {
            d.cocuklar = self.dsl_oku(l, s, ust, false);
            return;
        }
        let mut y = Yontem { ad: format!("{}@{}", d.ad, d.satir), satir: d.satir, govdeli: true, govde_metni: metin(l, k), ..Default::default() };
        // Çevreleyen fonksiyonun parametreleri ve yerelleri işleyicide görünür
        let mut yereller = ust.yereller.clone();
        if let (Some(t), Some(lp)) = (d.tip_argumanlari.first(), adlilar(l).into_iter().find(|x| x.kind() == "lambda_parameters")) {
            if let Some(a) = adlilar(lp).into_iter().find(|x| x.kind() == "variable_declaration").and_then(|v| adlilar(v).into_iter().find(|x| x.kind() == "identifier")) {
                yereller.insert(metin(a, k), t.clone());
            }
        }
        let (mut cagrilar, mut kaynaklar) = (vec![], ust.kaynak_cagrilari.clone());
        y.dna = govde_oku(l, k, &mut yereller, &mut cagrilar, &mut kaynaklar);
        y.yereller = yereller;
        y.cagrilar = cagrilar;
        y.kaynak_cagrilari = kaynaklar;
        s.yontemler.push(y);
        d.isleyici = Some(s.yontemler.len() - 1);
    }

    /// Gramerin ayrıştıramadığı bölge: içindeki sınıf başlığı (`class X(...) : Y`), üyeler ve iç bildirimler kurtarılır;
    /// sınıf başlığı yoksa üyeler dosyanın sınıfına eklenir
    fn kurtar(&mut self, n: Node, cephe: &mut Sinif, ci: usize) {
        let k = self.k;
        let mut anotasyonlar = vec![];
        let mut acik: Option<(usize, Sinif)> = None;
        let mut ad_bekleniyor = None;
        let kapat = |o: &mut Self, acik: &mut Option<(usize, Sinif)>| {
            if let Some((i, mut s)) = acik.take() {
                exposed_tablosu(&mut s);
                o.siniflar[i] = s;
            }
        };
        for c in tumu(n) {
            match c.kind() {
                "modifiers" => anotasyonlar = adlilar(c).into_iter().filter(|m| m.kind() == "annotation").filter_map(|m| anotasyon_oku(m, k)).collect(),
                "class" | "interface" | "object" => {
                    kapat(self, &mut acik);
                    ad_bekleniyor = Some(if c.kind() == "interface" { "interface" } else { "class" });
                }
                "identifier" if ad_bekleniyor.is_some() => {
                    let i = self.siniflar.len();
                    self.siniflar.push(Sinif::default());
                    let tur = ad_bekleniyor.take().unwrap_or("class");
                    let s = Sinif { ad: metin(c, k), tur, anotasyonlar: std::mem::take(&mut anotasyonlar), satir: satir(c), soyut: tur == "interface", ..Default::default() };
                    acik = Some((i, s));
                }
                "primary_constructor" => {
                    if let Some((_, s)) = acik.as_mut() {
                        self.kurucu(c, s);
                    }
                }
                "delegation_specifiers" => {
                    if let Some((_, s)) = acik.as_mut() {
                        self.ustler(c, s);
                    }
                }
                "class_body" => {
                    if let Some((i, mut s)) = acik.take() {
                        self.govde(c, &mut s, i, false);
                        acik = Some((i, s));
                    }
                }
                "function_declaration" => match acik.take() {
                    Some((i, mut s)) => {
                        let y = self.fonksiyon(c, &mut s, i);
                        s.yontemler.push(y);
                        acik = Some((i, s));
                    }
                    None => {
                        let y = self.fonksiyon(c, cephe, ci);
                        cephe.yontemler.push(y);
                    }
                },
                "property_declaration" => match acik.as_mut() {
                    Some((_, s)) => s.alanlar.extend(self.ozellik(c, false)),
                    None => cephe.alanlar.extend(self.ozellik(c, true)),
                },
                "class_declaration" | "object_declaration" => self.sinif_oku(c),
                "ERROR" => self.kurtar(c, cephe, ci),
                _ => {}
            }
        }
        kapat(self, &mut acik);
    }
}

/// Exposed tablosunun sütunları: `varchar("ad", 50)` sütun adı ve tipi; `reference("x_id", Tablo, onDelete = CASCADE)`
/// ilişki ve kaskad (modelde JPA karşılıklarıyla: `@Column(name)`, `@ManyToOne`, `@OnDelete(action = CASCADE)`)
fn exposed_tablosu(s: &mut Sinif) {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let sutun = D.get_or_init(|| regex::Regex::new(r#"^\s*(\w+)\s*\(\s*"([^"]+)"\s*(?:,\s*([A-Za-z_][\w.]*))?"#).unwrap());
    if !s.anotasyonlar.iter().any(|a| a.ad == "ExposedTable") {
        return;
    }
    for a in &mut s.alanlar {
        let Some(d) = a.deger.clone() else { continue };
        let Some(c) = sutun.captures(&d) else { continue };
        let satir_ = a.satir;
        let yeni = |ad: &str, argumanlar: Vec<(Option<String>, String)>| Anotasyon { ad: ad.into(), argumanlar, satir: satir_, hedef: None };
        let mut ek = vec![yeni("Column", vec![(Some("name".into()), format!("\"{}\"", &c[2]))])];
        a.tip = match &c[1] {
            "reference" | "optReference" => {
                ek.push(yeni("ManyToOne", vec![]));
                if d.contains("onDelete") && d.contains("CASCADE") {
                    ek.push(yeni("OnDelete", vec![(Some("action".into()), "CASCADE".into())]));
                }
                c.get(3).map(|x| son_ad(x.as_str())).unwrap_or_default()
            }
            "varchar" | "text" | "char" | "mediumText" | "largeText" => "String".into(),
            "integer" => "Integer".into(),
            "long" => "Long".into(),
            "short" => "Short".into(),
            "bool" => "Boolean".into(),
            "double" => "Double".into(),
            "float" => "Float".into(),
            "decimal" => "BigDecimal".into(),
            "uuid" => "UUID".into(),
            "datetime" | "timestamp" => "LocalDateTime".into(),
            "date" => "LocalDate".into(),
            "binary" | "blob" => "byte[]".into(),
            _ => continue,
        };
        a.anotasyonlar.extend(ek);
        // Sütun, nesnenin statik sabiti değil tablonun alanıdır
        a.statik = false;
    }
}

// ---- Gövde ----

/// Gramerin karşılaştırma olarak okuduğu tip argümanlı çağrı: `a.getOrFail<Long>("id")` ->
/// (alıcı, ad, tip, argüman metinleri)
fn genel_cagri(n: Node, k: &[u8]) -> Option<(String, String, String, Vec<String>)> {
    let (sol, op, sag) = (n.child_by_field_name("left")?, n.child_by_field_name("operator")?, n.child_by_field_name("right")?);
    if n.kind() != "binary_expression"
        || op.kind() != ">"
        || sol.kind() != "binary_expression"
        || sol.child_by_field_name("operator")?.kind() != "<"
        || sag.kind() != "parenthesized_expression"
    {
        return None;
    }
    let (nesne, ad) = cagri_adi(sol.child_by_field_name("left")?, k);
    let tip = tip_cevir(&metin(sol.child_by_field_name("right")?, k)).0;
    let args = metin(sag, k);
    let ic = args.trim().strip_prefix('(').and_then(|x| x.strip_suffix(')')).unwrap_or("").trim();
    Some((nesne, ad, tip, if ic.is_empty() { vec![] } else { crate::spring::ust_duzey_bol(ic) }))
}

/// İlk değerden tip: kurucu çağrısı (`Foo(...)`), `call.receive<T>()`, `inject<T>()`, `x as T`, `a ?: b`, dizgi
fn cikarim(n: Node, k: &[u8]) -> Option<String> {
    match n.kind() {
        "call_expression" => {
            let p = cagri_parcalari(n);
            let (nesne, ad) = cagri_adi(p.callee?, k);
            let tipler = tip_argumanlari(p.tipler, k);
            if matches!(ad.as_str(), "receive" | "receiveNullable" | "receiveOrNull" | "inject" | "get" | "lazy") && !tipler.is_empty() {
                return tipler.into_iter().next();
            }
            (nesne.is_empty() && ad.starts_with(char::is_uppercase)).then(|| if tipler.is_empty() { ad } else { format!("{ad}<{}>", tipler.join(", ")) })
        }
        "as_expression" => n.child_by_field_name("right").map(|t| tip_cevir(&metin(t, k)).0),
        "binary_expression" if n.child_by_field_name("operator").is_some_and(|o| o.kind() == "?:") => cikarim(n.child_by_field_name("left")?, k),
        "binary_expression" => {
            genel_cagri(n, k).filter(|(_, ad, ..)| matches!(ad.as_str(), "receive" | "receiveNullable" | "receiveOrNull" | "inject" | "get")).map(|(_, _, t, _)| t)
        }
        "property_delegate" => adlilar(n).first().and_then(|x| cikarim(*x, k)),
        "string_literal" | "multiline_string_literal" => Some("String".into()),
        "parenthesized_expression" | "unary_expression" => adlilar(n).first().and_then(|x| cikarim(*x, k)),
        _ => None,
    }
}

/// Tipsiz yerelin değerini veren çağrı: `servis.getir(id)` -> ("servis", "getir"); `a ?: throw`, `a!!` açılır
fn kaynak_cagri(n: Node, k: &[u8]) -> Option<(String, String)> {
    match n.kind() {
        "call_expression" => {
            let (nesne, ad) = cagri_adi(cagri_parcalari(n).callee?, k);
            (!nesne.is_empty() && !nesne.contains('(') && !ad.is_empty()).then_some((nesne, ad))
        }
        "binary_expression" if n.child_by_field_name("operator").is_some_and(|o| o.kind() == "?:") => kaynak_cagri(n.child_by_field_name("left")?, k),
        "parenthesized_expression" | "unary_expression" => adlilar(n).first().and_then(|x| kaynak_cagri(*x, k)),
        _ => None,
    }
}

/// Ktor `HttpStatusCode.X` ya da `HttpStatusCode(404, ...)`; değilse Java / NestJS biçimi (`HttpStatus.NOT_FOUND`)
pub(crate) fn kod(s: &str) -> Option<u16> {
    let s = s.trim();
    if let Some(ic) = s.strip_prefix("HttpStatusCode(") {
        return ic.split([',', ')']).next().and_then(|x| x.trim().parse().ok());
    }
    if let Some(ad) = s.strip_prefix("HttpStatusCode.") {
        return Some(match ad {
            "OK" => 200,
            "Created" => 201,
            "Accepted" => 202,
            "NoContent" => 204,
            "BadRequest" => 400,
            "Unauthorized" => 401,
            "PaymentRequired" => 402,
            "Forbidden" => 403,
            "NotFound" => 404,
            "MethodNotAllowed" => 405,
            "NotAcceptable" => 406,
            "RequestTimeout" => 408,
            "Conflict" => 409,
            "Gone" => 410,
            "PreconditionFailed" => 412,
            "PayloadTooLarge" => 413,
            "UnsupportedMediaType" => 415,
            "UnprocessableEntity" => 422,
            "Locked" => 423,
            "TooManyRequests" => 429,
            "InternalServerError" => 500,
            "NotImplemented" => 501,
            "BadGateway" => 502,
            "ServiceUnavailable" => 503,
            "GatewayTimeout" => 504,
            _ => return None,
        });
    }
    crate::dna::kod_ifadesi(s)
}

/// Düğümün içinde bulunduğu koşul: en yakın `if` (else dalındaysa olumsuzu), `when` dalı, `catch`, `a ?: x` (a boş)
fn kosul(mut n: Node, kok: Node, k: &[u8]) -> Option<String> {
    while let Some(ust) = n.parent() {
        if n.id() == kok.id() {
            break;
        }
        match ust.kind() {
            "if_expression" => {
                let c = ust.child_by_field_name("condition")?;
                if c.id() != n.id() {
                    let kosul = crate::dna::kisa(&metin(c, k));
                    let mut else_once = false;
                    for x in tumu(ust) {
                        if x.id() == n.id() {
                            break;
                        }
                        else_once |= x.kind() == "else";
                    }
                    return Some(if else_once { format!("!({kosul})") } else { kosul });
                }
            }
            "when_entry" => return ust.child_by_field_name("condition").map(|c| crate::dna::kisa(&metin(c, k))),
            "catch_block" => return Some("catch".into()),
            "binary_expression" if ust.child_by_field_name("operator").is_some_and(|o| o.kind() == "?:") && ust.child_by_field_name("right").is_some_and(|r| r.id() == n.id()) => {
                return ust.child_by_field_name("left").map(|l| format!("{} boş", crate::dna::kisa(&metin(l, k))));
            }
            _ => {}
        }
        n = ust;
    }
    None
}

/// Fırlatma kaydı: `throw X(...)` ya da `orElseThrow { X(...) }`. Kodu yerleşik addan ya da argümandaki durumdan;
/// bilinmiyorsa adaptör eşlemeden (`@ResponseStatus`, `@ExceptionHandler`, StatusPages) tamamlar
fn firlatma_ekle(d: &mut GovdeDna, x: Node, konum: Node, govde: Node, k: &[u8], kosul_: Option<String>) {
    if d.firlatmalar.len() >= EN_FAZLA || x.kind() != "call_expression" {
        return;
    }
    let p = cagri_parcalari(x);
    let Some((_, ad)) = p.callee.map(|c| cagri_adi(c, k)) else { return };
    if !ad.starts_with(char::is_uppercase) {
        return;
    }
    let args: Vec<String> = ham_argumanlar(p.argumanlar).into_iter().map(|a| metin(a, k)).collect();
    let durum = crate::spring::yerlesik_kod(&ad).or_else(|| args.iter().take(2).find_map(|a| kod(a)));
    let kosul_ = kosul_.or_else(|| kosul(konum, govde, k));
    let sahiplik = kosul_.as_deref().is_some_and(|c| crate::spring::sahiplik(c, durum));
    d.firlatmalar.push(Firlatma { durum, istisna: ad, kosul: kosul_, satir: satir(konum), sahiplik });
}

fn basit_mi(n: Node) -> bool {
    match n.kind() {
        "identifier" | "number_literal" | "character_literal" | "float_literal" => true,
        "string_literal" => !adlilar(n).iter().any(|c| c.kind() == "interpolation"),
        "navigation_expression" => adlilar(n).iter().all(|c| matches!(c.kind(), "identifier" | "navigation_expression" | "this_expression")),
        _ => false,
    }
}

/// İki yollu ifadenin bağlamı: atandığı değişken ya da içinde geçtiği çağrı
fn dal_baglami(n: Node, k: &[u8]) -> String {
    let mut x = n;
    for _ in 0..4 {
        let Some(u) = x.parent() else { break };
        match u.kind() {
            "property_declaration" => return adlilar(u).into_iter().find(|c| c.kind() == "variable_declaration").map(|v| metin(v, k)).unwrap_or_default(),
            "assignment" => return u.child_by_field_name("left").map(|a| metin(a, k)).unwrap_or_default(),
            "call_expression" => return cagri_parcalari(u).callee.map(|c| cagri_adi(c, k).1).unwrap_or_default(),
            "return_expression" => return "return".into(),
            _ => x = u,
        }
    }
    String::new()
}

/// Gövdeyi bir kez dolaşır: yerel tipler, tipsiz yerellerin kaynak çağrıları, çağrılar (Ktor indeks erişimleri
/// `call.parameters["id"]` ad "[]" olarak) ve davranış özeti. Yerel sınıf ve nesne gövdelerine girilmez
fn govde_oku(govde: Node, k: &[u8], yereller: &mut HashMap<String, String>, cagrilar: &mut Vec<Cagri>, kaynaklar: &mut HashMap<String, (String, String)>) -> GovdeDna {
    let mut d = GovdeDna::default();
    // Tipi yazılan yerelin ilk değeri: `val g: Dto = call.receive()` alma tipini verir
    let mut ipucu: HashMap<usize, String> = HashMap::new();
    let mut yigin = vec![govde];
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "class_declaration" | "object_literal" | "object_declaration" | "class_body" => continue,
            "property_declaration" => {
                let vd = adlilar(n).into_iter().find(|x| x.kind() == "variable_declaration");
                let ad = vd.and_then(|v| adlilar(v).into_iter().find(|x| x.kind() == "identifier")).map(|x| metin(x, k));
                let tip = vd.and_then(|v| adlilar(v).into_iter().find(|x| tip_mi(x.kind()))).map(|t| tip_cevir(&metin(t, k)).0);
                let cocuk = tumu(n);
                let ilk = cocuk.iter().position(|x| x.kind() == "=").and_then(|i| cocuk.get(i + 1)).copied();
                let vekil = cocuk.iter().find(|x| x.kind() == "property_delegate").copied();
                if let Some(ad) = ad {
                    match tip {
                        Some(t) => {
                            if let Some(i) = ilk {
                                ipucu.insert(i.id(), t.clone());
                            }
                            yereller.insert(ad, t);
                        }
                        None => match ilk.or(vekil).and_then(|i| cikarim(i, k)) {
                            Some(t) => {
                                yereller.insert(ad, t);
                            }
                            None => {
                                if let Some(kc) = ilk.and_then(|i| kaynak_cagri(i, k)) {
                                    kaynaklar.insert(ad, kc);
                                }
                            }
                        },
                    }
                }
            }
            "for_statement" | "lambda_parameters" => {
                for v in adlilar(n).into_iter().filter(|x| x.kind() == "variable_declaration") {
                    let c = adlilar(v);
                    if let (Some(a), Some(t)) = (c.iter().find(|x| x.kind() == "identifier"), c.iter().find(|x| tip_mi(x.kind()))) {
                        yereller.insert(metin(*a, k), tip_cevir(&metin(*t, k)).0);
                    }
                }
            }
            "throw_expression" => {
                if let Some(x) = adlilar(n).first() {
                    firlatma_ekle(&mut d, *x, n, govde, k, None);
                }
            }
            "index_expression" => {
                // call.parameters["id"], call.request.queryParameters["q"], params["ad"]; ardından gelen dönüşüm (toLong)
                let c = adlilar(n);
                if let (2, Some(a)) = (c.len(), c.get(1).filter(|x| x.kind() == "string_literal")) {
                    let mut x = n;
                    while let Some(u) = x.parent().filter(|u| u.kind() == "unary_expression") {
                        x = u;
                    }
                    let donusum = x
                        .parent()
                        .filter(|u| u.kind() == "navigation_expression" && adlilar(*u).first().is_some_and(|f| f.id() == x.id()))
                        .and_then(|u| adlilar(u).last().map(|i| metin(*i, k)));
                    if cagrilar.len() < 256 {
                        let nesne: String = metin(c[0], k).chars().filter(|c| !c.is_whitespace() && *c != '?').collect();
                        cagrilar.push(Cagri { nesne, ad: "[]".into(), argumanlar: vec![dizgi(*a, k), donusum.unwrap_or_default()], satir: satir(n), tip_argumanlari: vec![] });
                    }
                }
            }
            "call_expression" => cagri_isle(n, govde, k, &mut d, yereller, cagrilar, &ipucu),
            "binary_expression" => {
                if let Some((nesne, ad, tip, argumanlar)) = genel_cagri(n, k).filter(|_| cagrilar.len() < 256) {
                    cagrilar.push(Cagri { nesne, ad, argumanlar, satir: satir(n), tip_argumanlari: vec![tip] });
                }
            }
            "assignment" => {
                // Yumuşak silme: `kayit.deleted = true`, `kayit.deletedAt = Instant.now()`
                if let (Some(l), Some(r)) = (n.child_by_field_name("left"), n.child_by_field_name("right")) {
                    let alan = metin(l, k);
                    let alan = alan.rsplit('.').next().unwrap_or("");
                    let deger = metin(r, k);
                    const ALANLAR: [&str; 7] = ["deleted", "isDeleted", "deletedAt", "trashed", "isTrashed", "trashedAt", "removed"];
                    if d.yumusak.is_none() && ALANLAR.contains(&alan) && (deger == "true" || deger.contains("now") || deger.contains("Date") || deger.contains("Instant")) {
                        d.yumusak = Some(satir(n));
                    }
                }
            }
            "if_expression" if d.dallar.len() < EN_FAZLA => {
                let c = tumu(n);
                let kosul_ = n.child_by_field_name("condition");
                let evet = c.iter().position(|x| x.kind() == ")").and_then(|i| c[i + 1..].iter().find(|x| x.is_named() && !yorum_mu(**x)));
                let hayir = c.iter().position(|x| x.kind() == "else").and_then(|i| c[i + 1..].iter().find(|x| x.is_named() && !yorum_mu(**x)));
                if let (Some(kn), Some(e), Some(h)) = (kosul_, evet, hayir) {
                    let km = metin(kn, k);
                    if basit_mi(*e) && basit_mi(*h) && km.len() <= 60 && !km.contains('\n') {
                        d.dallar.push(Dal {
                            kosul: crate::dna::kisa(&km),
                            evet: crate::dna::kisa(&metin(*e, k)),
                            hayir: crate::dna::kisa(&metin(*h, k)),
                            baglam: dal_baglami(n, k),
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

fn cagri_isle(n: Node, govde: Node, k: &[u8], d: &mut GovdeDna, yereller: &HashMap<String, String>, cagrilar: &mut Vec<Cagri>, ipucu: &HashMap<usize, String>) {
    let p = cagri_parcalari(n);
    let Some(callee) = p.callee else { return };
    let (nesne, ad) = cagri_adi(callee, k);
    if ad.is_empty() {
        return;
    }
    let args = ham_argumanlar(p.argumanlar);
    let mut arg_metni: Vec<String> = args.iter().take(8).map(|a| metin(*a, k).chars().take(300).collect()).collect();
    // Sondaki lambda da bir argümandır (aşırı yükleme ve varsayılan değerli parametre eşleşmesi)
    if let Some(l) = p.lambda {
        arg_metni.push(metin(l, k).chars().take(300).collect());
    }
    let mut tipler = tip_argumanlari(p.tipler, k);
    if tipler.is_empty() && matches!(ad.as_str(), "receive" | "receiveNullable" | "receiveOrNull") {
        tipler.extend(ipucu.get(&n.id()).cloned());
    }
    if cagrilar.len() < 256 {
        cagrilar.push(Cagri { nesne: nesne.clone(), ad: ad.clone(), argumanlar: arg_metni.clone(), satir: satir(n), tip_argumanlari: tipler });
    }
    let tam = if nesne.is_empty() { ad.clone() } else { format!("{nesne}.{ad}") };
    if !tam.contains('(') && d.cagrilar.len() < 64 && !d.cagrilar.contains(&tam) {
        d.cagrilar.push(tam.clone());
    }
    // Hata yanıtları: ResponseEntity.notFound() / status(...), response.sendError(403), Ktor call.respond(HttpStatusCode.X)
    let kod_ = match (nesne.as_str(), ad.as_str()) {
        ("ResponseEntity", "notFound") => Some(404),
        ("ResponseEntity", "badRequest") => Some(400),
        ("ResponseEntity", "unprocessableEntity") => Some(422),
        ("ResponseEntity", "internalServerError") => Some(500),
        ("ResponseEntity", "status") => arg_metni.first().and_then(|a| kod(a)),
        (_, "sendError" | "setStatus") if !nesne.is_empty() => arg_metni.first().and_then(|a| kod(a)),
        (x, "respond" | "respondText" | "respondNullable" | "respondBytes" | "respondRedirect") if x.ends_with("call") => arg_metni.iter().find_map(|a| kod(a)),
        (x, "status") if x.ends_with("call.response") => arg_metni.first().and_then(|a| kod(a)),
        _ => None,
    };
    if let Some(kod_) = kod_.filter(|c| *c >= 400 && d.firlatmalar.len() < EN_FAZLA) {
        let kosul_ = kosul(n, govde, k);
        let sahip = kosul_.as_deref().is_some_and(|c| crate::spring::sahiplik(c, Some(kod_)));
        d.firlatmalar.push(Firlatma { durum: Some(kod_), istisna: tam.clone(), kosul: kosul_, satir: satir(n), sahiplik: sahip });
    }
    // `bul(...).orElseThrow { KayitYok(...) }`: kayıt yoksa fırlatma
    if ad == "orElseThrow" {
        let alici = tek_satir(&nesne);
        let son = [".map(", ".filter(", ".flatMap("].iter().filter_map(|x| alici.find(x)).min().unwrap_or(alici.len());
        let kosul_ = Some(format!("{} boş", crate::dna::kisa(&alici[..son])));
        match p.lambda.and_then(|l| adlilar(l).into_iter().rfind(|x| x.kind() == "call_expression")) {
            Some(y) => firlatma_ekle(d, y, n, govde, k, kosul_),
            None if p.lambda.is_none() && args.is_empty() && d.firlatmalar.len() < EN_FAZLA => {
                d.firlatmalar.push(Firlatma { durum: None, istisna: "NoSuchElementException".into(), kosul: kosul_, satir: satir(n), sahiplik: false });
            }
            _ => {}
        }
    }
    let ilk = arg_metni.first().map(String::as_str).unwrap_or("").trim();
    let yumusak_ayarlayici = ["setDeleted", "setIsDeleted", "setDeletedAt", "setTrashed", "setIsTrashed", "setTrashedAt", "setRemoved"].contains(&ad.as_str())
        && (ilk == "true" || ilk.contains("now") || ilk.contains("Date") || ilk.contains("Instant"));
    if d.yumusak.is_none() && (yumusak_ayarlayici || ["softDelete", "softRemove"].contains(&ad.as_str())) {
        d.yumusak = Some(satir(n));
    }
    // Olay ve mesaj: publishEvent(X(...)) / publishEvent(olay), Kafka send("konu"), JMS / Rabbit convertAndSend
    let alici = nesne.to_lowercase();
    let metin_arg = |i: usize| args.get(i).filter(|a| a.kind() == "string_literal").and_then(|a| crate::spring::java_metni(&dizgi(*a, k)));
    match ad.as_str() {
        "publishEvent" => {
            let olay = match args.first() {
                Some(a) if a.kind() == "call_expression" => {
                    cagri_parcalari(*a).callee.map(|c| cagri_adi(c, k)).filter(|(ne, x)| ne.is_empty() && x.starts_with(char::is_uppercase)).map(|(_, x)| x)
                }
                Some(a) if a.kind() == "identifier" => yereller.get(&metin(*a, k)).map(|t| crate::spring::temel_ad(t)),
                _ => None,
            };
            if let Some(o) = olay {
                crate::spring::tetik_ekle(d, "event", o, satir(n));
            }
        }
        "send" | "sendDefault" if alici.contains("kafka") => {
            if let Some(t) = metin_arg(0) {
                crate::spring::tetik_ekle(d, "event", t, satir(n));
            }
        }
        "convertAndSend" | "send" if alici.contains("jms") || alici.contains("rabbit") || alici.contains("amqp") => {
            let i = if args.len() >= 3 && !alici.contains("jms") { 1 } else { 0 };
            if let Some(t) = metin_arg(i) {
                crate::spring::tetik_ekle(d, "event", t, satir(n));
            }
        }
        _ => {}
    }
}

// ---- Spring Security Kotlin DSL'i ----

/// Metin ve yorum içindeki baytlar
fn maske(b: &[u8]) -> Vec<bool> {
    let mut maske = vec![false; b.len()];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => {
                let q = b[i];
                let bas = i;
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                for x in maske.iter_mut().take((i + 1).min(b.len())).skip(bas) {
                    *x = true;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                let bas = i;
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                for x in maske.iter_mut().take(i).skip(bas) {
                    *x = true;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let bas = i;
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 1;
                for x in maske.iter_mut().take((i + 1).min(b.len())).skip(bas) {
                    *x = true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    maske
}

/// Açılan ayracın (`{` ya da `(`) eşi
fn kapanis(b: &[u8], maske: &[bool], bas: usize) -> Option<usize> {
    let (ac, kapa) = (b[bas], if b[bas] == b'{' { b'}' } else { b')' });
    let mut d = 0i32;
    for (x, c) in b.iter().enumerate().skip(bas) {
        if maske[x] {
            continue;
        }
        if *c == ac {
            d += 1;
        } else if *c == kapa {
            d -= 1;
            if d == 0 {
                return Some(x);
            }
        }
    }
    None
}

/// Spring Security Kotlin DSL'ini güvenlik zincirinin okuduğu çağrı biçimine çevirir: `ad { ... }` -> `ad({ ... })`,
/// `authorize("/x", permitAll)` -> `requestMatchers("/x").permitAll()`, `authorize(anyRequest, authenticated)` ->
/// `anyRequest().authenticated()`
pub(crate) fn guvenlik_dsl(m: &str) -> String {
    let ad_mi = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    // 1) Sondaki lambda çağrı argümanı olur
    let b = m.as_bytes();
    let mk = maske(b);
    let mut ekler: Vec<(usize, &str)> = vec![];
    let mut i = 0;
    while i < b.len() {
        if mk[i] || !ad_mi(b[i]) || (i > 0 && ad_mi(b[i - 1])) {
            i += 1;
            continue;
        }
        let bas = i;
        while i < b.len() && ad_mi(b[i]) {
            i += 1;
        }
        let mut j = i;
        while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
            j += 1;
        }
        if j < b.len() && b[j] == b'{' && !mk[j] && !ANAHTAR.contains(&&m[bas..i]) {
            if let Some(son) = kapanis(b, &mk, j) {
                ekler.push((i, "("));
                ekler.push((son + 1, ")"));
            }
        }
    }
    ekler.sort_by_key(|(x, _)| *x);
    let mut s = String::with_capacity(m.len() + ekler.len());
    let mut onceki = 0;
    for (x, e) in ekler {
        s.push_str(&m[onceki..x]);
        s.push_str(e);
        onceki = x;
    }
    s.push_str(&m[onceki..]);
    // 2) authorize(eşleyici..., kural)
    let b = s.as_bytes().to_vec();
    let mk = maske(&b);
    let mut degisimler: Vec<(usize, usize, String)> = vec![];
    let mut i = 0;
    while let Some(x) = s[i..].find("authorize(").map(|x| x + i) {
        i = x + 1;
        if mk[x] || (x > 0 && ad_mi(b[x - 1])) {
            continue;
        }
        let ac = x + "authorize".len();
        let Some(son) = kapanis(&b, &mk, ac) else { continue };
        let parcalar = crate::spring::ust_duzey_bol(&s[ac + 1..son]);
        let Some((kural, eslesme)) = parcalar.split_last() else { continue };
        let kural = kural.trim();
        let kural = if kural.contains('(') { kural.to_string() } else { format!("{kural}()") };
        let sol = if eslesme.len() == 1 && eslesme[0].trim() == "anyRequest" { "anyRequest()".to_string() } else { format!("requestMatchers({})", eslesme.join(", ")) };
        degisimler.push((x, son + 1, format!("{sol}.{kural}")));
        i = son;
    }
    for (bas, son, yeni) in degisimler.into_iter().rev() {
        s.replace_range(bas..son, &yeni);
    }
    s
}

// ---- Dosya ----

fn hatalar(n: Node, out: &mut Vec<usize>) {
    if n.is_error() || n.is_missing() {
        out.push(satir(n));
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

/// Kotlin dosyasını okur. `dsl`: Ktor yönlendirme DSL'i de okunsun mu (Spring için gerekmez)
pub(crate) fn ayristir_metin(yol: &Path, kaynak: &str, dsl: bool) -> Option<KtDosya> {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_kotlin_ng::LANGUAGE.into()).ok()?;
    let agac = p.parse(kaynak, None)?;
    let k = kaynak.as_bytes();
    let mut d = Dosya { yol: yol.to_path_buf(), satir_sayisi: kaynak.lines().count(), ..Default::default() };
    let mut o = Okuyucu { k, dsl, siniflar: vec![Sinif::default()], fonklar: vec![] };
    // Üst düzey fonksiyon ve özellikler: Kotlin'in JVM'deki dosya sınıfı (Routing.kt -> RoutingKt)
    let kok_ad = yol.file_stem().and_then(|s| s.to_str()).unwrap_or("Dosya");
    let mut harfler = kok_ad.chars();
    let cephe_adi = format!("{}{}Kt", harfler.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default(), harfler.as_str());
    let mut cephe = Sinif { ad: cephe_adi, tur: "class", satir: 1, ..Default::default() };
    for c in adlilar(agac.root_node()) {
        match c.kind() {
            "package_header" => d.paket = adlilar(c).into_iter().find(|x| x.kind() == "qualified_identifier").map(|x| metin(x, k)).unwrap_or_default(),
            "import" => {
                let m = tek_satir(&metin(c, k));
                let m = m.trim_start_matches("import").trim();
                d.iceaktarmalar.push(m.split(" as ").next().unwrap_or(m).replace(' ', ""));
            }
            "class_declaration" | "object_declaration" => o.sinif_oku(c),
            "function_declaration" => {
                let y = o.fonksiyon(c, &mut cephe, 0);
                cephe.yontemler.push(y);
            }
            "property_declaration" => cephe.alanlar.extend(o.ozellik(c, true)),
            "ERROR" => o.kurtar(c, &mut cephe, 0),
            _ => {}
        }
    }
    o.siniflar[0] = cephe;
    d.siniflar = o.siniflar;
    hatalar(agac.root_node(), &mut d.hatali_satirlar);
    Some(KtDosya { model: d, dsl: o.fonklar })
}
