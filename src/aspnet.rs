//! ASP.NET Core adaptörü: kaynak kodlu C# uygulamaları (`.csproj`'u Web SDK'lı). Kaynağı olmayan .NET ikilileri ayrı
//! bir yoldan (kaynaksız keşif) okunur. C# kaynağı tree-sitter-c-sharp ile okunur; NestJS ve FastAPI adaptörleriyle
//! aynı kapı yapısı (`Kapi`) üretilir, kart, MCP, önizleme, OpenAPI ve canlı mod dilden bağımsız çalışır.
//!
//! Kesinlik ilkesi aynıdır: anlam adlardan değil tanımlardan çıkar.
//! - **Rotalar (kesin):** controller'lar (`[Route("api/[controller]")]` + `[HttpGet("{id:int}")]`, `[controller]` /
//!   `[action]` / `[area]` yer değiştirmesi, `~/` ile mutlak şablon, `[ActionName]`, `Async` eki atılır,
//!   `[ApiVersion]` ile `{version:apiVersion}`), geleneksel rota (`MapControllerRoute`, `MapDefaultControllerRoute`)
//!   ve Minimal API (`MapGet/MapPost/...`, `MapMethods`, iç içe `MapGroup`, grubu ya da uygulamayı alan uzantı
//!   metotları). Rota kısıtı (`{id:int}`) yol parametresinin tipidir.
//! - **Girdiler:** `[FromBody]` / `[FromQuery]` / `[FromRoute]` / `[FromForm]` / `[AsParameters]` ve çatının çıkarım
//!   kuralları (basit tip sorgu, karmaşık tip gövde, DI servisi girdi değil). Alanlar DTO / record tanımından:
//!   DataAnnotations (`[Required]`, `[StringLength]`, `[Range]`, `[EmailAddress]`, `[RegularExpression]`, ...), C#
//!   tipi, `required` anahtar sözcüğü ve Nullable bağlamındaki örtük zorunluluk. Doğrulamayı `[ApiController]`'da çatı
//!   yapar (kesin); Minimal API'de (`AddValidation()` yoksa) ve `[ApiController]` olmayan controller'da yapmaz:
//!   zorunluluk orada tahmindir. JSON adları System.Text.Json'ın web varsayılanıyla (camelCase, `[JsonPropertyName]`).
//! - **Yanıt:** `ActionResult<T>`, düz `T`, `Results<Ok<T>, ...>`, `[ProducesResponseType(typeof(T), ...)]`,
//!   `.Produces<T>()`.
//! - **Giriş ve rol:** `[Authorize]`, `Roles = ...`, `Policy = ...` (politikanın `AddPolicy(..., p =>
//!   p.RequireRole(...))` tanımından), `[AllowAnonymous]` (her yetki işaretini geçersiz kılar), `RequireAuthorization` /
//!   `AllowAnonymous` (uç, grup, `MapControllers()`), MVC genel `AuthorizeFilter` ve genel `FallbackPolicy`: kesin.
//!   İşaretsiz uç, tanınmayan yetki filtresi ya da kimlik denetleyen ara katman: çıkarım.
//! - **Veri etkisi (Yorum):** Entity Framework Core `DbSet` erişimleri, `Add` / `Update` / `Remove`,
//!   `ExecuteDelete` / `ExecuteUpdate`; işleyiciden servislere ve MediatR işleyicilerine (`Send`) inilerek. Kaskadlar
//!   `OnDelete(DeleteBehavior.Cascade)` tanımından ve EF Core'un zorunlu ilişki kuralından. Ham SQL görülürse liste
//!   eksik olabilir.
//! - **DNA:** fırlatmalar ve red sonuçları (`NotFound()`, `Forbid()`, `Results.Problem(...)`) koşullarıyla, iki yollu
//!   ifadeler, yumuşak silme; MediatR `Publish` olayları (`INotificationHandler<T>` dinleyicileri) ve
//!   `BackgroundService` işleri olay / iş düğümleridir.
use crate::adaptor::{Alan, Girdi, Kapi, Sema, VeriBag};
use crate::dna::{Dal, Firlatma, GovdeDna};
use crate::guvenli;
use regex::Regex;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;
use tree_sitter::{Node, Parser};

pub const SURUM: &str = "0.1.0";

/// Minimal API rota kayıtları: yöntemi adından belli olanlar
const MAP_YONTEMLERI: [(&str, &str); 5] = [("MapGet", "GET"), ("MapPost", "POST"), ("MapPut", "PUT"), ("MapDelete", "DELETE"), ("MapPatch", "PATCH")];
/// Controller eylem işaretleri
const HTTP_ISARETLERI: [(&str, &str); 7] =
    [("HttpGet", "GET"), ("HttpPost", "POST"), ("HttpPut", "PUT"), ("HttpDelete", "DELETE"), ("HttpPatch", "PATCH"), ("HttpHead", "HEAD"), ("HttpOptions", "OPTIONS")];
/// Çatının işleyiciye kendisinin verdiği tipler: girdi değildir
const ENJEKTE: [&str; 16] = [
    "HttpContext",
    "HttpRequest",
    "HttpResponse",
    "CancellationToken",
    "ClaimsPrincipal",
    "IServiceProvider",
    "ILogger",
    "ILoggerFactory",
    "Stream",
    "PipeReader",
    "IConfiguration",
    "LinkGenerator",
    "IWebHostEnvironment",
    "IHostEnvironment",
    "TimeProvider",
    "IHttpClientFactory",
];
/// Basit (sorgu ve yol parametresi olabilen) tipler
const BASIT: [&str; 26] = [
    "string",
    "int",
    "long",
    "short",
    "byte",
    "sbyte",
    "uint",
    "ulong",
    "ushort",
    "bool",
    "decimal",
    "double",
    "float",
    "char",
    "Guid",
    "DateTime",
    "DateTimeOffset",
    "DateOnly",
    "TimeOnly",
    "TimeSpan",
    "Int32",
    "Int64",
    "String",
    "Boolean",
    "Decimal",
    "Double",
];
/// Dosya yükleme tipleri: önizleme dosyanın varlığını, türünü ve içeriğini sınamaz
const DOSYA_TIPLERI: [&str; 3] = ["IFormFile", "IFormFileCollection", "IFormCollection"];
/// EF Core yazma çağrıları (DbSet ya da DbContext üzerinde)
const EF_YAZMA: [&str; 12] =
    ["Add", "AddAsync", "AddRange", "AddRangeAsync", "Update", "UpdateRange", "Remove", "RemoveRange", "Attach", "ExecuteDelete", "ExecuteDeleteAsync", "ExecuteUpdate"];
/// Yumuşak silme alanları (C# adlarıyla); yalnızca `true` ya da zaman atanınca işaret sayılır
const YUMUSAK_ALAN: [&str; 8] = ["IsDeleted", "DeletedAt", "DeletedOn", "DeletedDate", "IsTrashed", "TrashedAt", "IsRemoved", "Deleted"];

// ---- Model ----

/// Öznitelik: `[Authorize(Roles = "A")]` -> ad "Authorize", anahtarlı [("Roles", "\"A\"")]; `[ProducesResponseType<T>(200)]`
/// -> genel "T". Ad son parçadır, `Attribute` eki atılır.
#[derive(Clone, Debug, Default)]
pub(crate) struct Oznitelik {
    ad: String,
    genel: Option<String>,
    konumsal: Vec<String>,
    anahtarli: Vec<(String, String)>,
}

impl Oznitelik {
    fn arg(&self, ad: &str) -> Option<&str> {
        self.anahtarli.iter().find(|(k, _)| k == ad).map(|(_, v)| v.as_str())
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Parametre {
    ad: String,
    tip: String,
    oznitelikler: Vec<Oznitelik>,
    varsayilan: Option<String>,
    /// Uzantı metodunun `this` parametresi
    bu: bool,
    satir: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Ozellik {
    ad: String,
    tip: String,
    oznitelikler: Vec<Oznitelik>,
    /// C# 11 `required`: System.Text.Json eksik alanı reddeder
    zorunlu_kw: bool,
    statik: bool,
    satir: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Govde {
    metin: String,
    dna: GovdeDna,
    satir: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Metot {
    ad: String,
    donus: String,
    parametreler: Vec<Parametre>,
    oznitelikler: Vec<Oznitelik>,
    satir: usize,
    govde: Option<Govde>,
    statik: bool,
    acik: bool,
    belge: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Sinif {
    ad: String,
    /// "class", "record", "struct", "interface", "enum"; üst düzey ifadelerin yerel fonksiyonları için "ust"
    tur: String,
    tabanlar: Vec<String>,
    oznitelikler: Vec<Oznitelik>,
    ozellikler: Vec<Ozellik>,
    /// Alanlar: (ad, tip, ilk değer, sabit)
    alanlar: Vec<(String, String, Option<String>, bool)>,
    metotlar: Vec<Metot>,
    /// Birincil kurucu, record konumsal parametreleri ve kurucu metotların parametreleri (DI alan tipleri)
    kurucu: Vec<Parametre>,
    /// Record'un konumsal parametreleri (alanlarıdır)
    konumsal: bool,
    enum_uyeleri: Vec<String>,
    /// İç içe tipin içinde bulunduğu sınıf (`Create.Command` -> "Create")
    ust: Option<String>,
    /// Ad alanı (`namespace Conduit.Features.Articles;`): aynı adlı tipler (her özellikte bir `List`) bununla ayrılır
    ad_alani: Option<String>,
    satir: usize,
    statik: bool,
    soyut: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Lambda {
    oznitelikler: Vec<Oznitelik>,
    parametreler: Vec<Parametre>,
    donus: Option<String>,
    govde: Govde,
}

#[derive(Clone, Debug)]
pub(crate) struct Arguman {
    ad: Option<String>,
    metin: String,
    lambda: Option<Lambda>,
}

/// Çağrı zincirinin bir halkası: `.MapGet("/x", ...)`
#[derive(Clone, Debug)]
pub(crate) struct Halka {
    ad: String,
    genel: Option<String>,
    argumanlar: Vec<Arguman>,
    satir: usize,
}

/// Bir çağrı zinciri: `app.MapGroup("/a").RequireAuthorization()`; kök alıcı, halkalar kaynak sırasıyla
#[derive(Clone, Debug)]
pub(crate) struct Zincir {
    kok: String,
    halkalar: Vec<Halka>,
    /// `var api = app.MapGroup(...)`: atandığı değişken
    atanan: Option<String>,
    sinif: Option<String>,
    metot: Option<String>,
    /// Kayıt bir koşulun altında (`if (app.Environment.IsDevelopment())`): koşulun kod metni
    kosul: Option<String>,
    satir: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Dosya {
    yol: PathBuf,
    satir_sayisi: usize,
    hatali: bool,
    siniflar: Vec<Sinif>,
    zincirler: Vec<Zincir>,
    /// Yapılandırma (politikalar, genel filtreler, JSON ayarları) ve EF Core şeması metinden okunur
    metin: String,
}

// ---- Ayrıştırma ----

fn metin(n: Node, k: &[u8]) -> String {
    n.utf8_text(k).unwrap_or("").to_string()
}

fn tek_satir(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn satir(n: Node) -> usize {
    n.start_position().row + 1
}

fn cocuklar(n: Node) -> Vec<Node> {
    let mut w = n.walk();
    n.named_children(&mut w).collect()
}

/// Tipin temel adı: `List<Urun>?` -> "List", `Magaza.Models.Urun` -> "Urun", `global::X` -> "X"
fn temel(t: &str) -> String {
    let t = t.trim().trim_end_matches('?').trim_start_matches("global::");
    let t = t.split('<').next().unwrap_or(t).trim_end_matches("[]");
    t.rsplit('.').next().unwrap_or(t).trim().to_string()
}

/// Genel tipin argümanları: `Dictionary<string, List<int>>` -> ["string", "List<int>"]
fn genel_argumanlar(t: &str) -> Vec<String> {
    let t = t.trim().trim_end_matches('?');
    let Some(i) = t.find('<') else { return vec![] };
    let Some(ic) = t[i + 1..].strip_suffix('>') else { return vec![] };
    ust_duzey_bol(ic, ',')
}

/// Koleksiyon tipinin öğesi: `List<X>`, `IEnumerable<X>`, `X[]` ... -> X
fn koleksiyon_ogesi(t: &str) -> Option<String> {
    let t = t.trim().trim_end_matches('?');
    if let Some(o) = t.strip_suffix("[]") {
        return Some(o.to_string());
    }
    const KOLEKSIYON: [&str; 12] = [
        "List",
        "IList",
        "ICollection",
        "IEnumerable",
        "IReadOnlyList",
        "IReadOnlyCollection",
        "HashSet",
        "ISet",
        "Collection",
        "IAsyncEnumerable",
        "ImmutableArray",
        "ImmutableList",
    ];
    KOLEKSIYON.contains(&temel(t).as_str()).then(|| genel_argumanlar(t).into_iter().next()).flatten()
}

/// Üst düzey ayraçla bölme (parantez, köşeli, süslü ve açılı parantez ile tırnak içi bölünmez)
fn ust_duzey_bol(s: &str, ayrac: char) -> Vec<String> {
    let (mut v, mut d, mut bas) = (vec![], 0i32, 0);
    let mut tirnak = false;
    let mut onceki = ' ';
    for (i, c) in s.char_indices() {
        match c {
            '"' if onceki != '\\' => tirnak = !tirnak,
            _ if tirnak => {}
            '(' | '[' | '{' | '<' => d += 1,
            ')' | ']' | '}' | '>' => d -= 1,
            c if c == ayrac && d == 0 => {
                v.push(s[bas..i].trim().to_string());
                bas = i + c.len_utf8();
            }
            _ => {}
        }
        onceki = c;
    }
    if !s[bas..].trim().is_empty() {
        v.push(s[bas..].trim().to_string());
    }
    v
}

/// C# metin sabiti: "x", @"x", """x""" -> x; yer tutuculu `$"..."` çözülmez
fn metin_sabiti(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(g) = s.strip_prefix("\"\"\"").and_then(|x| x.strip_suffix("\"\"\"")) {
        return Some(g.trim().to_string());
    }
    if let Some(g) = s.strip_prefix("@\"").and_then(|x| x.strip_suffix('"')) {
        return Some(g.replace("\"\"", "\""));
    }
    if let Some(g) = s.strip_prefix("$\"").and_then(|x| x.strip_suffix('"')) {
        return (!g.contains('{')).then(|| g.to_string());
    }
    let g = s.strip_prefix('"')?.strip_suffix('"')?;
    Some(g.replace("\\\"", "\"").replace("\\\\", "\\"))
}

fn oznitelik_adi(n: Node, k: &[u8]) -> (String, Option<String>) {
    let (ad, genel) = if n.kind() == "generic_name" {
        let ad = cocuklar(n).first().map(|c| metin(*c, k)).unwrap_or_default();
        let genel = cocuklar(n).iter().find(|c| c.kind() == "type_argument_list").map(|t| metin(*t, k).trim_start_matches('<').trim_end_matches('>').trim().to_string());
        (ad, genel)
    } else {
        (metin(n, k), None)
    };
    let ad = ad.rsplit('.').next().unwrap_or(&ad).to_string();
    (ad.strip_suffix("Attribute").filter(|a| !a.is_empty()).map(String::from).unwrap_or(ad), genel)
}

/// Düğümün `[...]` öznitelikleri
fn oznitelikler(n: Node, k: &[u8]) -> Vec<Oznitelik> {
    let mut v = vec![];
    for l in cocuklar(n).into_iter().filter(|c| c.kind() == "attribute_list") {
        for a in cocuklar(l).into_iter().filter(|c| c.kind() == "attribute") {
            let Some(ad_d) = a.child_by_field_name("name") else { continue };
            let (ad, genel) = oznitelik_adi(ad_d, k);
            let mut o = Oznitelik { ad, genel, ..Default::default() };
            for liste in cocuklar(a).into_iter().filter(|c| c.kind() == "attribute_argument_list") {
                for arg in cocuklar(liste).into_iter().filter(|c| c.kind() == "attribute_argument") {
                    let ic = cocuklar(arg);
                    let Some(deger) = ic.last() else { continue };
                    match arg.child_by_field_name("name").filter(|x| x.id() != deger.id()) {
                        Some(ad) => o.anahtarli.push((metin(ad, k), tek_satir(&metin(*deger, k)))),
                        None => o.konumsal.push(tek_satir(&metin(*deger, k))),
                    }
                }
            }
            v.push(o);
        }
    }
    v
}

fn degistiriciler(n: Node, k: &[u8]) -> Vec<String> {
    cocuklar(n).into_iter().filter(|c| c.kind() == "modifier").map(|c| metin(c, k)).collect()
}

fn parametre(n: Node, k: &[u8]) -> Parametre {
    let tip = n.child_by_field_name("type");
    let ad = n.child_by_field_name("name");
    let varsayilan =
        cocuklar(n).into_iter().rfind(|c| !matches!(c.kind(), "attribute_list" | "modifier") && Some(c.id()) != tip.map(|t| t.id()) && Some(c.id()) != ad.map(|t| t.id()));
    Parametre {
        ad: ad.map(|a| metin(a, k)).unwrap_or_else(|| metin(n, k)),
        tip: tip.map(|t| tek_satir(&metin(t, k))).unwrap_or_default(),
        oznitelikler: oznitelikler(n, k),
        varsayilan: varsayilan.map(|v| tek_satir(&metin(v, k))),
        bu: degistiriciler(n, k).iter().any(|m| m == "this"),
        satir: satir(n),
    }
}

fn parametreler(n: Option<Node>, k: &[u8]) -> Vec<Parametre> {
    match n {
        Some(l) if l.kind() == "parameter_list" => cocuklar(l).into_iter().filter(|c| c.kind() == "parameter").map(|p| parametre(p, k)).collect(),
        // `x => ...`: tipsiz tek parametre
        Some(p) if p.kind() == "implicit_parameter" || p.kind() == "identifier" => vec![Parametre { ad: metin(p, k), satir: satir(p), ..Default::default() }],
        _ => vec![],
    }
}

/// Bildirimin üstündeki `/// <summary>...</summary>` belge yorumu (etiketsiz, tek satır)
fn belge(n: Node, k: &[u8]) -> Option<String> {
    let mut satirlar = vec![];
    let mut x = n.prev_named_sibling();
    while let Some(c) = x.filter(|c| c.kind() == "comment") {
        let m = metin(c, k);
        if !m.starts_with("///") {
            break;
        }
        satirlar.push(m.trim_start_matches('/').trim().to_string());
        x = c.prev_named_sibling();
    }
    satirlar.reverse();
    let tum = satirlar.join(" ");
    let ozet = tum.split("<summary>").nth(1).map(|s| s.split("</summary>").next().unwrap_or(s)).unwrap_or(&tum);
    static ETIKET: OnceLock<Regex> = OnceLock::new();
    let ozet = ETIKET.get_or_init(|| Regex::new(r"<[^>]+>").unwrap()).replace_all(ozet, "");
    let ozet = tek_satir(&ozet);
    (!ozet.is_empty()).then_some(ozet)
}

fn govde(n: Node, k: &[u8]) -> Govde {
    Govde { metin: metin(n, k), dna: cs_govde(n, k), satir: satir(n) }
}

fn metot(n: Node, k: &[u8]) -> Metot {
    let mods = degistiriciler(n, k);
    let ad = n.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
    let donus = n.child_by_field_name("returns").or_else(|| n.child_by_field_name("type")).map(|t| tek_satir(&metin(t, k))).unwrap_or_default();
    Metot {
        ad,
        donus,
        parametreler: parametreler(n.child_by_field_name("parameters"), k),
        oznitelikler: oznitelikler(n, k),
        satir: n.child_by_field_name("name").map(satir).unwrap_or_else(|| satir(n)),
        govde: n.child_by_field_name("body").map(|b| govde(b, k)),
        statik: mods.iter().any(|m| m == "static"),
        acik: mods.iter().any(|m| m == "public"),
        belge: belge(n, k),
    }
}

/// Sınıf benzeri bildirim (iç içe tipler ayrı sınıf olarak `out`'a eklenir)
fn sinif(n: Node, k: &[u8], ust: Option<&str>, out: &mut Vec<Sinif>) {
    let mods = degistiriciler(n, k);
    let tur = match n.kind() {
        "class_declaration" => "class",
        "record_declaration" | "record_struct_declaration" => "record",
        "struct_declaration" => "struct",
        "interface_declaration" => "interface",
        _ => "enum",
    };
    let mut s = Sinif {
        ad: n.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default(),
        tur: tur.into(),
        oznitelikler: oznitelikler(n, k),
        satir: n.child_by_field_name("name").map(satir).unwrap_or_else(|| satir(n)),
        statik: mods.iter().any(|m| m == "static"),
        soyut: mods.iter().any(|m| m == "abstract"),
        ust: ust.map(String::from),
        ..Default::default()
    };
    for c in cocuklar(n) {
        match c.kind() {
            "base_list" => {
                for t in cocuklar(c) {
                    // `: Taban(args)` birincil kurucu tabanı
                    let t = if t.kind() == "primary_constructor_base_type" { t.child_by_field_name("type").unwrap_or(t) } else { t };
                    if t.kind() != "argument_list" {
                        s.tabanlar.push(tek_satir(&metin(t, k)));
                    }
                }
            }
            "parameter_list" => {
                s.kurucu = parametreler(Some(c), k);
                s.konumsal = tur == "record";
            }
            _ => {}
        }
    }
    let mut icler = vec![];
    if let Some(g) = n.child_by_field_name("body") {
        for u in cocuklar(g) {
            match u.kind() {
                "method_declaration" => s.metotlar.push(metot(u, k)),
                "constructor_declaration" => s.kurucu.extend(parametreler(u.child_by_field_name("parameters"), k)),
                "property_declaration" => {
                    let mods = degistiriciler(u, k);
                    s.ozellikler.push(Ozellik {
                        ad: u.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default(),
                        tip: u.child_by_field_name("type").map(|t| tek_satir(&metin(t, k))).unwrap_or_default(),
                        oznitelikler: oznitelikler(u, k),
                        zorunlu_kw: mods.iter().any(|m| m == "required"),
                        statik: mods.iter().any(|m| m == "static") || !mods.iter().any(|m| m == "public") && tur != "interface",
                        satir: satir(u),
                    });
                }
                "field_declaration" => {
                    let mods = degistiriciler(u, k);
                    let sabit = mods.iter().any(|m| m == "const") || (mods.iter().any(|m| m == "static") && mods.iter().any(|m| m == "readonly"));
                    for vd in cocuklar(u).into_iter().filter(|c| c.kind() == "variable_declaration") {
                        let tip = vd.child_by_field_name("type").map(|t| tek_satir(&metin(t, k))).unwrap_or_default();
                        for d in cocuklar(vd).into_iter().filter(|c| c.kind() == "variable_declarator") {
                            let ad = d.child_by_field_name("name");
                            let deger = cocuklar(d).into_iter().rfind(|c| Some(c.id()) != ad.map(|a| a.id())).map(|v| tek_satir(&metin(v, k)));
                            s.alanlar.push((ad.map(|a| metin(a, k)).unwrap_or_default(), tip.clone(), deger, sabit));
                        }
                    }
                }
                "enum_member_declaration" => {
                    if let Some(a) = u.child_by_field_name("name") {
                        s.enum_uyeleri.push(metin(a, k));
                    }
                }
                "class_declaration" | "record_declaration" | "record_struct_declaration" | "struct_declaration" | "interface_declaration" | "enum_declaration" => icler.push(u),
                _ => {}
            }
        }
    }
    let ad = s.ad.clone();
    out.push(s);
    for i in icler {
        sinif(i, k, Some(&ad), out);
    }
}

fn lambda(n: Node, k: &[u8]) -> Lambda {
    Lambda {
        oznitelikler: oznitelikler(n, k),
        parametreler: parametreler(n.child_by_field_name("parameters"), k),
        donus: n.child_by_field_name("type").map(|t| tek_satir(&metin(t, k))),
        govde: n.child_by_field_name("body").map(|b| govde(b, k)).unwrap_or_default(),
    }
}

fn argumanlar(n: Option<Node>, k: &[u8]) -> Vec<Arguman> {
    let Some(l) = n else { return vec![] };
    cocuklar(l)
        .into_iter()
        .filter(|c| c.kind() == "argument")
        .filter_map(|a| {
            let ic = cocuklar(a);
            let deger = *ic.last()?;
            let ad = a.child_by_field_name("name").filter(|x| x.id() != deger.id()).map(|x| metin(x, k));
            Some(Arguman { ad, metin: tek_satir(&metin(deger, k)), lambda: (deger.kind() == "lambda_expression").then(|| lambda(deger, k)) })
        })
        .collect()
}

/// Düğümün içinde bulunduğu koşul: en yakın `if` (else dalındaysa olumsuzu), üç terimli ifade, `?? throw`, `catch`
fn cs_kosul(mut n: Node, kok: Node, k: &[u8]) -> Option<String> {
    while let Some(ust) = n.parent() {
        if n.id() == kok.id() {
            break;
        }
        match ust.kind() {
            "if_statement" => {
                let c = ust.child_by_field_name("condition")?;
                if c.id() != n.id() {
                    let kosul = crate::dna::kisa(&metin(c, k));
                    let olumlu = ust.child_by_field_name("consequence").is_some_and(|x| x.id() == n.id());
                    return Some(if olumlu { kosul } else { format!("!({kosul})") });
                }
            }
            "conditional_expression" => {
                let c = ust.child_by_field_name("condition")?;
                if c.id() != n.id() {
                    let kosul = crate::dna::kisa(&metin(c, k));
                    let olumlu = ust.child_by_field_name("consequence").is_some_and(|x| x.id() == n.id());
                    return Some(if olumlu { kosul } else { format!("!({kosul})") });
                }
            }
            "binary_expression" if ust.child_by_field_name("right").is_some_and(|r| r.id() == n.id()) && metin(ust, k).contains("??") => {
                return ust.child_by_field_name("left").map(|l| format!("{} is null", crate::dna::kisa(&metin(l, k))));
            }
            "catch_clause" => return Some("catch".into()),
            _ => {}
        }
        n = ust;
    }
    None
}

/// Red sonucu üreten yardımcı (`ControllerBase` ya da `Results` / `TypedResults`): HTTP kodu
fn red_kodu(ad: &str) -> Option<u16> {
    Some(match ad {
        "BadRequest" | "ValidationProblem" | "BadRequestObjectResult" => 400,
        "Unauthorized" | "Challenge" => 401,
        "Forbid" => 403,
        "NotFound" | "NotFoundObjectResult" => 404,
        "Conflict" => 409,
        "UnprocessableEntity" => 422,
        "InternalServerError" => 500,
        _ => return None,
    })
}

/// İfadeden HTTP kodu: `403`, `StatusCodes.Status403Forbidden`, `HttpStatusCode.Forbidden`
fn kod_ifadesi(s: &str) -> Option<u16> {
    static D: OnceLock<Regex> = OnceLock::new();
    let s = s.trim();
    let s = s.split_once(':').filter(|(a, _)| a.trim().chars().all(|c| c.is_alphanumeric())).map(|(_, b)| b.trim()).unwrap_or(s);
    if let Ok(n) = s.parse::<u16>() {
        return (100..600).contains(&n).then_some(n);
    }
    if let Some(c) = D.get_or_init(|| Regex::new(r"Status(\d{3})").unwrap()).captures(s) {
        return c[1].parse().ok();
    }
    // `HttpStatusCode.NotFound`
    let ad = s.strip_prefix("HttpStatusCode.").or_else(|| s.strip_prefix("System.Net.HttpStatusCode."))?;
    Some(match ad {
        "BadRequest" => 400,
        "Unauthorized" => 401,
        "Forbidden" => 403,
        "NotFound" => 404,
        "Conflict" => 409,
        "Gone" => 410,
        "UnprocessableEntity" | "UnprocessableContent" => 422,
        "TooManyRequests" => 429,
        "InternalServerError" => 500,
        _ => return None,
    })
}

fn basit_ifade(n: Node) -> bool {
    matches!(
        n.kind(),
        "identifier" | "member_access_expression" | "string_literal" | "verbatim_string_literal" | "integer_literal" | "real_literal" | "boolean_literal" | "null_literal"
    )
}

/// C# fonksiyon gövdesinin davranış öğeleri (tree-sitter-c-sharp düğümü): fırlatmalar, red sonuçları, iki yollu
/// ifadeler, yumuşak silme ve adıyla çağrılan fonksiyonlar. Olaylar ve silinen tablolar çözümlemede eklenir (tipler
/// ve EF Core şeması dosyalar arasıdır)
fn cs_govde(g: Node, k: &[u8]) -> GovdeDna {
    let mut d = GovdeDna::default();
    let mut yigin = vec![g];
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "throw_statement" | "throw_expression" if d.firlatmalar.len() < 32 => {
                if let Some(y) = n.named_child(0).filter(|x| x.kind() == "object_creation_expression") {
                    let ad = y.child_by_field_name("type").map(|t| temel(&metin(t, k))).unwrap_or_default();
                    let durum = y.child_by_field_name("arguments").map(cocuklar).unwrap_or_default().iter().take(2).find_map(|a| kod_ifadesi(&metin(*a, k)));
                    let kosul = cs_kosul(n, g, k);
                    let sahiplik = kosul.as_deref().is_some_and(|c| crate::dna::sahiplik_mi(c, durum.or(Some(403))));
                    d.firlatmalar.push(Firlatma { durum, istisna: ad, kosul, satir: satir(n), sahiplik });
                }
            }
            "invocation_expression" => {
                let f = n.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                let f: String = f.chars().filter(|c| !c.is_whitespace()).collect();
                let (alici, son) = f.rsplit_once('.').unwrap_or(("", &f));
                let son = son.split('<').next().unwrap_or(son);
                let args: Vec<String> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default().iter().map(|a| metin(*a, k)).collect();
                if matches!(alici, "" | "this" | "base" | "Results" | "TypedResults") && d.firlatmalar.len() < 32 {
                    let kod = match son {
                        "StatusCode" => args.first().and_then(|a| kod_ifadesi(a)),
                        "Problem" => Some(args.iter().find(|a| a.contains("statusCode")).and_then(|a| kod_ifadesi(a)).unwrap_or(500)),
                        _ => red_kodu(son),
                    };
                    if let Some(kod) = kod.filter(|c| *c >= 400) {
                        let kosul = cs_kosul(n, g, k);
                        let sahiplik = kosul.as_deref().is_some_and(|c| crate::dna::sahiplik_mi(c, Some(kod)));
                        d.firlatmalar.push(Firlatma { durum: Some(kod), istisna: f.clone(), kosul, satir: satir(n), sahiplik });
                    }
                }
                if !f.contains('(') && d.cagrilar.len() < 64 && !d.cagrilar.contains(&f) {
                    d.cagrilar.push(f);
                }
            }
            "conditional_expression" if d.dallar.len() < 32 => {
                if let (Some(c), Some(e), Some(h)) = (n.child_by_field_name("condition"), n.child_by_field_name("consequence"), n.child_by_field_name("alternative")) {
                    let kosul = metin(c, k);
                    if basit_ifade(e) && basit_ifade(h) && kosul.len() <= 60 && !kosul.contains('\n') {
                        let baglam = n
                            .parent()
                            .filter(|u| matches!(u.kind(), "assignment_expression" | "variable_declarator"))
                            .and_then(|u| u.child_by_field_name("left").or_else(|| u.child_by_field_name("name")))
                            .map(|a| metin(a, k))
                            .unwrap_or_default();
                        d.dallar.push(Dal {
                            kosul: crate::dna::kisa(&kosul),
                            evet: crate::dna::kisa(&metin(e, k)),
                            hayir: crate::dna::kisa(&metin(h, k)),
                            baglam,
                            satir: satir(n),
                        });
                    }
                }
            }
            "assignment_expression" if d.yumusak.is_none() => {
                let sol = n.child_by_field_name("left").map(|a| metin(a, k)).unwrap_or_default();
                let sag = n.child_by_field_name("right").map(|a| metin(a, k)).unwrap_or_default();
                let alan = sol.rsplit('.').next().unwrap_or(&sol).trim();
                if YUMUSAK_ALAN.contains(&alan) && (sag.trim() == "true" || sag.contains("DateTime") || sag.contains("Now") || sag.contains("GetUtcNow")) {
                    d.yumusak = Some(satir(n));
                }
            }
            _ => {}
        }
        let bas = yigin.len();
        yigin.extend(cocuklar(n));
        yigin[bas..].reverse();
    }
    d
}

/// Düğümün bulunduğu sınıf ve metot (ya da yerel fonksiyon) adları
fn kapsam(mut n: Node, k: &[u8]) -> (Option<String>, Option<String>) {
    let (mut s, mut m) = (None, None);
    while let Some(u) = n.parent() {
        match u.kind() {
            "method_declaration" | "local_function_statement" | "constructor_declaration" if m.is_none() => m = u.child_by_field_name("name").map(|a| metin(a, k)),
            "class_declaration" | "record_declaration" | "struct_declaration" | "interface_declaration" if s.is_none() => s = u.child_by_field_name("name").map(|a| metin(a, k)),
            _ => {}
        }
        n = u;
    }
    (s, m)
}

/// Çağrı zincirlerini toplar: rota kayıtları (`Map*`), ara katmanlar (`Use*`) ve deyim düzeyindeki tek halkalı
/// çağrılar (uzantı metodu çağrısı olabilir: `app.MapKampanyalar()`, `KayitEt(app)`)
fn zincirleri_topla(kok: Node, k: &[u8], out: &mut Vec<Zincir>) {
    let mut yigin = vec![kok];
    while let Some(n) = yigin.pop() {
        if n.kind() == "invocation_expression" {
            let ic = n.parent().is_some_and(|p| {
                p.kind() == "member_access_expression"
                    && p.child_by_field_name("expression").is_some_and(|e| e.id() == n.id())
                    && p.parent().is_some_and(|pp| pp.kind() == "invocation_expression" && pp.child_by_field_name("function").is_some_and(|f| f.id() == p.id()))
            });
            if !ic {
                let mut halkalar = vec![];
                let mut x = n;
                let mut kok_metin = String::new();
                while x.kind() == "invocation_expression" {
                    let Some(f) = x.child_by_field_name("function") else { break };
                    let args = argumanlar(x.child_by_field_name("arguments"), k);
                    match f.kind() {
                        "member_access_expression" => {
                            let Some(ad) = f.child_by_field_name("name") else { break };
                            let (a, genel) = oznitelik_adi(ad, k);
                            halkalar.push(Halka { ad: a, genel, argumanlar: args, satir: satir(ad) });
                            match f.child_by_field_name("expression") {
                                Some(e) if e.kind() == "invocation_expression" => x = e,
                                Some(e) => {
                                    kok_metin = metin(e, k).chars().filter(|c| !c.is_whitespace()).collect();
                                    break;
                                }
                                None => break,
                            }
                        }
                        "identifier" | "generic_name" => {
                            let (a, genel) = oznitelik_adi(f, k);
                            halkalar.push(Halka { ad: a, genel, argumanlar: args, satir: satir(f) });
                            break;
                        }
                        _ => break,
                    }
                }
                halkalar.reverse();
                let deyim = n
                    .parent()
                    .is_some_and(|p| p.kind() == "expression_statement" || (p.kind() == "await_expression" && p.parent().is_some_and(|q| q.kind() == "expression_statement")));
                let ilgili = halkalar.iter().any(|h| h.ad.starts_with("Map") || matches!(h.ad.as_str(), "Use" | "UseMiddleware" | "UseEndpoints"))
                    || (deyim && halkalar.len() == 1 && halkalar[0].ad.chars().next().is_some_and(char::is_uppercase));
                if ilgili && !halkalar.is_empty() && out.len() < 20_000 {
                    let atanan = n.parent().and_then(|p| match p.kind() {
                        "variable_declarator" => p.child_by_field_name("name").map(|a| metin(a, k)),
                        "assignment_expression" => p.child_by_field_name("left").map(|a| metin(a, k)),
                        _ => None,
                    });
                    let (s, m) = kapsam(n, k);
                    let mut kosul = None;
                    let mut u = n;
                    while let Some(p) = u.parent() {
                        if matches!(p.kind(), "method_declaration" | "local_function_statement" | "lambda_expression" | "compilation_unit") {
                            break;
                        }
                        if p.kind() == "if_statement" {
                            if let Some(c) = p.child_by_field_name("condition") {
                                let olumlu = p.child_by_field_name("consequence").is_some_and(|x| x.id() == u.id());
                                let km = crate::dna::kisa(&metin(c, k));
                                kosul = Some(if olumlu { km } else { format!("!({km})") });
                                break;
                            }
                        }
                        u = p;
                    }
                    out.push(Zincir { kok: kok_metin, halkalar, atanan, sinif: s, metot: m, kosul, satir: satir(n) });
                }
            }
        }
        yigin.extend(cocuklar(n));
    }
}

fn bildirimleri_gez(n: Node, k: &[u8], d: &mut Dosya, ust: &mut Sinif, ad_alani: Option<String>) {
    let mut ad_alani = ad_alani;
    for c in cocuklar(n) {
        match c.kind() {
            "namespace_declaration" => {
                let ic = c.child_by_field_name("name").map(|a| tek_satir(&metin(a, k)));
                let tam = match (&ad_alani, ic) {
                    (Some(u), Some(i)) => Some(format!("{u}.{i}")),
                    (u, i) => i.or(u.clone()),
                };
                if let Some(g) = c.child_by_field_name("body") {
                    bildirimleri_gez(g, k, d, ust, tam);
                }
            }
            // `namespace X;`: dosyanın kalanı bu ad alanındadır
            "file_scoped_namespace_declaration" => ad_alani = c.child_by_field_name("name").map(|a| tek_satir(&metin(a, k))),
            "class_declaration" | "record_declaration" | "record_struct_declaration" | "struct_declaration" | "interface_declaration" | "enum_declaration" => {
                let bas = d.siniflar.len();
                sinif(c, k, None, &mut d.siniflar);
                for s in &mut d.siniflar[bas..] {
                    s.ad_alani = ad_alani.clone();
                }
            }
            "global_statement" => {
                for x in cocuklar(c).into_iter().filter(|x| x.kind() == "local_function_statement") {
                    ust.metotlar.push(metot(x, k));
                }
            }
            _ => {}
        }
    }
}

fn ayristir(yol: &Path, kaynak: &str) -> Option<Dosya> {
    // UTF-8 BOM (Visual Studio'nun yazdığı dosyalarda yaygın) ayrıştırıcıyı şaşırtır
    let kaynak = kaynak.strip_prefix('\u{feff}').unwrap_or(kaynak);
    let mut p = Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).ok()?;
    let agac = p.parse(kaynak, None)?;
    let k = kaynak.as_bytes();
    let kok = agac.root_node();
    let mut d = Dosya { yol: yol.to_path_buf(), satir_sayisi: kaynak.lines().count(), hatali: kok.has_error(), siniflar: vec![], zincirler: vec![], metin: kaynak.to_string() };
    let mut ust = Sinif { tur: "ust".into(), ..Default::default() };
    bildirimleri_gez(kok, k, &mut d, &mut ust, None);
    if !ust.metotlar.is_empty() {
        d.siniflar.push(ust);
    }
    zincirleri_topla(kok, k, &mut d.zincirler);
    Some(d)
}

// ---- Algılama ----

/// Proje dosyaları (`*.csproj`), kökten en fazla 4 kat; derleme ve bağımlılık klasörleri hariç
fn proje_dosyalari(kok: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    let mut kuyruk = vec![(kok.to_path_buf(), 0)];
    while let Some((k, derinlik)) = kuyruk.pop() {
        let Ok(oge) = std::fs::read_dir(&k) else { continue };
        for g in oge.flatten() {
            let p = g.path();
            let ad = g.file_name().to_string_lossy().to_string();
            if g.file_type().is_ok_and(|t| t.is_dir()) {
                if derinlik < 4 && !ad.starts_with('.') && !matches!(ad.as_str(), "bin" | "obj" | "node_modules" | "packages" | "artifacts") {
                    kuyruk.push((p, derinlik + 1));
                }
            } else if ad.ends_with(".csproj") && g.file_type().is_ok_and(|t| t.is_file()) {
                v.push(p);
            }
        }
    }
    v.sort();
    v
}

/// Test projesi mi (xUnit / NUnit / MSTest ya da Microsoft.NET.Test.Sdk)
fn test_projesi(m: &str) -> bool {
    ["xunit", "nunit", "MSTest", "Microsoft.NET.Test.Sdk", "<IsTestProject>true"].iter().any(|x| m.contains(x))
}

/// ASP.NET Core web projesi mi: Web SDK'sı ya da Microsoft.AspNetCore.App çerçeve başvurusu (test projesi değil)
fn web_projesi(m: &str) -> bool {
    !test_projesi(m) && (m.contains("Microsoft.NET.Sdk.Web") || m.contains("\"Microsoft.AspNetCore.App\""))
}

/// Klasörde (ya da alt klasörlerinde) kaynak kodlu bir ASP.NET Core uygulaması var mı
pub fn aspnet_mi(kok: &Path) -> bool {
    proje_dosyalari(kok).iter().filter_map(|p| guvenli::oku(p)).any(|m| web_projesi(&m))
}

/// Uygulamanın adı ve sürümü: web projesinin `<AssemblyName>` (yoksa dosya adı) ve `<Version>` değeri
pub fn paket(kok: &Path) -> Value {
    let etiket = |m: &str, ad: &str| -> Option<String> {
        let bas = m.find(&format!("<{ad}>"))? + ad.len() + 2;
        let son = m[bas..].find(&format!("</{ad}>"))? + bas;
        Some(m[bas..son].trim().to_string()).filter(|x| !x.is_empty() && !x.contains("$("))
    };
    // Önce Web SDK'lı projeler: yalnızca çerçeve başvurusu olan yardımcı proje (ör. Aspire ServiceDefaults) uygulamanın
    // kendisi değildir
    let mut projeler: Vec<(PathBuf, String)> = proje_dosyalari(kok).into_iter().filter_map(|p| guvenli::oku(&p).map(|m| (p, m))).collect();
    projeler.sort_by_key(|(_, m)| !m.contains("Microsoft.NET.Sdk.Web") || m.contains("<IsAspireSharedProject>true"));
    for (p, m) in projeler {
        if web_projesi(&m) {
            let ad = etiket(&m, "AssemblyName").unwrap_or_else(|| p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
            return json!({ "name": ad, "version": etiket(&m, "Version").or_else(|| etiket(&m, "VersionPrefix")) });
        }
    }
    json!({ "name": null, "version": null })
}

/// Proje dosyasının başvurduğu projeler (`<ProjectReference Include="../X/X.csproj" />`): mutlak, sözcüksel olarak
/// düzleştirilmiş yollar
fn proje_baglari(csproj: &Path, metin: &str) -> Vec<PathBuf> {
    static D: OnceLock<Regex> = OnceLock::new();
    let re = D.get_or_init(|| Regex::new(r#"<ProjectReference\s+Include\s*=\s*"([^"]+)""#).unwrap());
    let Some(klasor) = csproj.parent() else { return vec![] };
    re.captures_iter(metin).map(|c| crate::kesif::duzlestir(&klasor.join(c[1].replace('\\', "/")))).collect()
}

/// Uygulama kökünün dışında kalan başvurulan projelerin klasörleri (katmanlı çözüm: `Ordering.API` -> `../Ordering.Infrastructure`).
/// Yalnızca kökü içeren depo ya da çözüm klasörünün (`.git`, `*.sln`, `*.slnx`) içindekiler izlenir; dönen: (proje adı, klasör)
fn dis_projeler(kok: &Path) -> Vec<(String, PathBuf)> {
    let sinir = kok.ancestors().skip(1).take(6).find(|a| {
        a.join(".git").exists()
            || std::fs::read_dir(a)
                .is_ok_and(|mut o| o.any(|g| g.is_ok_and(|g| g.file_name().to_string_lossy().ends_with(".sln") || g.file_name().to_string_lossy().ends_with(".slnx"))))
    });
    let Some(sinir) = sinir else { return vec![] };
    let kok_d = crate::kesif::duzlestir(kok);
    let mut kuyruk: Vec<(PathBuf, usize)> = proje_dosyalari(kok).into_iter().map(|p| (p, 0)).collect();
    let mut gorulen: HashSet<PathBuf> = kuyruk.iter().map(|(p, _)| crate::kesif::duzlestir(p)).collect();
    let mut v = vec![];
    while let Some((p, derinlik)) = kuyruk.pop() {
        let Some(m) = guvenli::oku(&p) else { continue };
        if test_projesi(&m) || derinlik > 4 {
            continue;
        }
        for b in proje_baglari(&p, &m) {
            if !gorulen.insert(b.clone()) || !b.starts_with(sinir) || !b.is_file() {
                continue;
            }
            if let Some(k) = b.parent().filter(|k| !k.starts_with(&kok_d)) {
                v.push((b.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), k.to_path_buf()));
            }
            kuyruk.push((b, derinlik + 1));
        }
    }
    v.sort();
    v.dedup();
    v
}

/// Keşif, paralel ayrıştırma ve ASP.NET Core adaptörü
pub(crate) fn topla(kok: &Path, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<Dosya>>, t0: Instant) -> crate::Toplam {
    let ek = dis_projeler(kok);
    let mut kokler: Vec<PathBuf> = vec![kok.to_path_buf()];
    kokler.extend(ek.iter().map(|(_, k)| k.clone()));
    // Test projelerinin klasörleri okunmaz (WebApplicationFactory ile kurulan uçlar uygulamanın API'si değildir)
    let mut test_klasorleri = vec![];
    let mut okunan_proje = vec![];
    for p in kokler.iter().flat_map(|k| proje_dosyalari(k)) {
        let Some(m) = guvenli::oku(&p) else { continue };
        if test_projesi(&m) {
            if let Some(k) = p.parent() {
                test_klasorleri.push(k.to_path_buf());
            }
        } else {
            okunan_proje.push((p.clone(), crate::kesif::icerik_ozeti(m.as_bytes())));
        }
    }
    let mut yollar = vec![];
    let (mut atlanan, mut buyuk) = (0usize, vec![]);
    for k in &kokler {
        for g in ignore::WalkBuilder::new(k).hidden(true).git_ignore(true).build().flatten() {
            let p = g.path();
            if g.path_is_symlink() || !p.is_file() {
                if g.path_is_symlink() {
                    atlanan += 1;
                }
                continue;
            }
            let goreli = p.strip_prefix(k).unwrap_or(p);
            let ad = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Derleme çıktısı, üretilmiş kod, göçler (şema anlık görüntüsü) ve testler uygulamanın API'si değildir
            let uretilmis = ad.ends_with(".Designer.cs") || ad.ends_with(".g.cs") || ad.ends_with(".AssemblyInfo.cs") || ad.ends_with("ModelSnapshot.cs");
            if uretilmis
                || test_klasorleri.iter().any(|t| p.starts_with(t))
                || goreli.components().any(|c| ["bin", "obj", "node_modules", "Migrations", "tests", "test", "wwwroot"].contains(&c.as_os_str().to_str().unwrap_or("")))
            {
                atlanan += 1;
                continue;
            }
            if p.extension().is_some_and(|e| e == "cs") {
                if g.metadata().is_ok_and(|m| m.len() > crate::EN_BUYUK_DOSYA) {
                    buyuk.push(guvenli::goreli(kok, p));
                    continue;
                }
                yollar.push(p.to_path_buf());
            } else {
                atlanan += 1;
            }
        }
    }
    let t_kesif = t0.elapsed();
    let (sonuclar, mut okunan, ayristirilan) = crate::kesif::ayristir_hepsi(&yollar, onbellek, |p, b| ayristir(p, std::str::from_utf8(&b).ok()?));
    let dosyalar: Vec<Dosya> = sonuclar.into_iter().map(|(_, d)| d).collect();
    okunan.extend(okunan_proje);
    let t_ayristirma = t0.elapsed();
    let nullable = proje_dosyalari(kok).iter().filter_map(|p| guvenli::oku(p)).any(|m| web_projesi(&m) && m.contains("<Nullable>enable</Nullable>"));
    let p = Proje::yeni(kok, &dosyalar, nullable);
    let (kapilar, bilgi) = p.kapilar();
    let t_toplam = t0.elapsed();
    crate::Toplam {
        kapilar,
        bilgi,
        adaptor: format!("aspnet@{SURUM}"),
        dosya_sayisi: dosyalar.len(),
        satir: dosyalar.iter().map(|d| d.satir_sayisi).sum(),
        kismi: dosyalar.iter().filter(|d| d.hatali).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        atlanan,
        buyuk,
        // Kökün dışındaki başvurulan projeler (altyapı, alan katmanı)
        ek_paketler: ek.into_iter().map(|(a, _)| a).collect(),
        // DbContext ve varlık sınıflarının dosyaları: rapor tablo bağlarını bunlara göre sayar
        veri_dosyalari: p.veri_dosyalari(),
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}

// ---- Çözümleme ----

/// Yetkilendirme politikası: istediği roller ve rollerin tamamı çözülebildi mi
#[derive(Clone, Debug, Default)]
struct Politika {
    roller: Vec<String>,
    cozuldu: bool,
    ozet: String,
}

/// Uca uygulanan yetki işareti (genelden özele)
#[derive(Clone, Debug)]
enum Isaret {
    /// `[Authorize]`, `RequireAuthorization(...)`: politika adları, `Roles` ifadesi, satır içi politika metni
    Yetki { politikalar: Vec<String>, roller: Option<String>, ic: Option<String>, kaynak: String },
    /// `[AllowAnonymous]`, `AllowAnonymous()`
    Anonim(String),
    /// Projede tanımlı, 401 / 403 üreten yetki filtresi (tanımı çatıya ait değil)
    OzelGerekli(String),
    /// Adı yetkiyi andıran ama izlenemeyen filtre ya da öznitelik
    Ozel(String),
}

/// Uç işleyicisi: controller eylemi, Minimal API lambdası ya da metot grubu
#[derive(Clone, Copy)]
struct Isleyici<'a> {
    di: usize,
    sinif: Option<&'a Sinif>,
    parametreler: &'a [Parametre],
    oznitelikler: &'a [Oznitelik],
    donus: Option<&'a str>,
    govde: Option<&'a Govde>,
    satir: usize,
    belge: Option<&'a str>,
}

/// Girdi çıkarımında bir parametre ([AsParameters] sınıfının özellikleri de): (ad, tip, öznitelikler, varsayılan, satır)
type SanalParametre = (String, String, Vec<Oznitelik>, Option<String>, usize);
/// Çağrı zincirinde gezilecek gövde: (dosya, sınıf, gövde, parametreler, derinlik)
type Gezinti<'b> = (usize, Option<&'b Sinif>, &'b Govde, &'b [Parametre], usize);

/// Ad çözümlemesinin bağlamı (bkz. `Proje::baglam`)
#[derive(Clone, Debug, Default)]
struct Baglam {
    ad_alani: Option<String>,
    sinif: Option<String>,
    ust: Option<String>,
}

/// Minimal API grubu (ya da uygulamanın kendisi)
#[derive(Clone, Debug, Default)]
struct Grup {
    onek: String,
    isaretler: Vec<Isaret>,
    etiketler: Vec<String>,
    /// Grafik kimliği ("dosya:önek"); uygulamanın kendisi için yok
    id: Option<String>,
    kosul: Option<String>,
    /// Asp.Versioning `HasApiVersion(1, 0)`: grubun API sürümleri
    surumler: Vec<String>,
}

/// Bir uç (çözümlenmiş rota kaydı)
struct Uc<'a> {
    yontemler: Vec<String>,
    sablon: String,
    isleyici: Isleyici<'a>,
    isaretler: Vec<Isaret>,
    kontrolcu: String,
    fonksiyon: String,
    /// Controller eylemi mi (girdi çıkarımı MVC kurallarıyla)
    mvc: bool,
    /// Çatı DataAnnotations doğrulamasını isteğe uyguluyor mu
    dogrulama: bool,
    ozet: Option<String>,
    kalkmis: bool,
    surum: Option<String>,
    ad: Option<String>,
    uretir: Vec<(Option<String>, Option<u16>)>,
    kosul: Option<String>,
    ana: bool,
    router: Option<String>,
}

struct Proje<'a> {
    kok: &'a Path,
    dosyalar: &'a [Dosya],
    siniflar: HashMap<&'a str, Vec<(usize, &'a Sinif)>>,
    sabitler: HashMap<String, String>,
    /// DbSet özellik adı -> varlık sınıfı
    dbset: HashMap<String, String>,
    dbcontextler: HashSet<String>,
    varliklar: BTreeSet<String>,
    servisler: HashSet<String>,
    politikalar: HashMap<String, Politika>,
    fallback: Option<Politika>,
    /// MVC genel `AuthorizeFilter`
    genel_filtre: bool,
    /// Kimlik denetleyen ara katmanlar (tanımı izlenemez): çıkarım
    supheli_ara: Vec<String>,
    json_enum_metin: bool,
    camel: bool,
    nullable: bool,
    minimal_dogrulama: bool,
    /// Grup ve controller grafiği (bağımlılık grafiği için yan bilgi): kimlik -> {id, dosya, satir, ustler}
    routerlar: RefCell<BTreeMap<String, Value>>,
    /// Ad çözümlemesinin bağlamı: başvuran sınıfın (ad alanı, kendi adı, içinde bulunduğu sınıf). Aynı adlı tiplerden
    /// C#'taki gibi en yakını seçilir: başvuranın içindeki ya da kardeşi olan iç tip, sonra aynı ad alanındaki
    baglam: RefCell<Baglam>,
}

/// System.Text.Json `JsonNamingPolicy.CamelCase`: baştaki büyük harf dizisini küçültür ("URL" -> "url", "IOAkis" -> "ioAkis")
fn camel(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    if c.first().is_none_or(|x| !x.is_uppercase()) {
        return s.to_string();
    }
    let mut out = c.clone();
    for i in 0..c.len() {
        if i == 1 && !c[i].is_uppercase() {
            break;
        }
        if i > 0 && i + 1 < c.len() && !c[i + 1].is_uppercase() {
            if c[i + 1] == ' ' {
                out[i] = c[i].to_lowercase().next().unwrap_or(c[i]);
            }
            break;
        }
        out[i] = c[i].to_lowercase().next().unwrap_or(c[i]);
    }
    out.into_iter().collect()
}

/// Açılan parantezin kapanışı (tırnak içi sayılmaz); `bas` açılan parantezin hemen sonrası
fn kapanis(s: &str, bas: usize) -> usize {
    let mut d = 1i32;
    let mut tirnak = false;
    let mut onceki = ' ';
    for (i, c) in s[bas..].char_indices() {
        match c {
            '"' if onceki != '\\' => tirnak = !tirnak,
            _ if tirnak => {}
            '(' => d += 1,
            ')' => {
                d -= 1;
                if d == 0 {
                    return bas + i;
                }
            }
            _ => {}
        }
        onceki = c;
    }
    s.len()
}

/// Deyimin sonu: `bas`tan sonraki üst düzey `;`
fn deyim_sonu(s: &str, bas: usize) -> usize {
    let mut d = 0i32;
    for (i, c) in s[bas..].char_indices() {
        match c {
            '(' | '{' | '[' => d += 1,
            ')' | '}' | ']' => {
                d -= 1;
                if d < 0 {
                    return bas + i;
                }
            }
            ';' if d == 0 => return bas + i,
            _ => {}
        }
    }
    s.len()
}

/// Rota şablonunun bir yol parçası: `{id:int?}` -> ("id", Some("int")), `{*yol}` -> ("yol", None)
fn yol_parcasi(p: &str) -> Option<(String, Option<String>)> {
    let ic = p.strip_prefix('{')?.strip_suffix('}')?;
    let ic = ic.trim_start_matches('*');
    let ic = ic.split('=').next().unwrap_or(ic).trim_end_matches('?');
    let mut parcalar = ic.split(':');
    let ad = parcalar.next()?.trim().to_string();
    let kisit = parcalar.next().map(|k| k.split('(').next().unwrap_or(k).to_string());
    Some((ad, kisit))
}

/// Şablon parçalarını birleştirir: "api/x" + "{id}" -> "/api/x/{id}"; sondaki eğik çizgi korunur
fn sablon_birlestir(a: &str, b: &str) -> String {
    let a = a.trim();
    let b = b.trim();
    let a = a.trim_start_matches('~');
    let mut s = String::from("/");
    s.push_str(a.trim_start_matches('/'));
    if !b.is_empty() {
        if !s.ends_with('/') {
            s.push('/');
        }
        s.push_str(b.trim_start_matches('/'));
    }
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    s
}

/// Sezgi yolu: `{id:int}` -> `:id`
fn sezgi_yolu(sablon: &str) -> String {
    let y = sablon.split('/').map(|p| yol_parcasi(p).map(|(a, _)| format!(":{a}")).unwrap_or_else(|| p.to_string())).collect::<Vec<_>>().join("/");
    if y.is_empty() {
        "/".into()
    } else {
        y
    }
}

/// `GET /api/x/:id` gibi bir uç için işleyici adı (Minimal API lambdası)
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

fn kucuk_bas(s: &str) -> String {
    let mut k = s.chars();
    k.next().map(|f| f.to_lowercase().collect::<String>() + k.as_str()).unwrap_or_default()
}

/// API sürümünün URL biçimi: "2.0" -> "2", "2.1" -> "2.1"
fn surum_url(v: &str) -> String {
    v.strip_suffix(".0").unwrap_or(v).to_string()
}

impl<'a> Proje<'a> {
    fn yeni(kok: &'a Path, dosyalar: &'a [Dosya], nullable: bool) -> Self {
        let mut siniflar: HashMap<&str, Vec<(usize, &Sinif)>> = HashMap::new();
        for (di, d) in dosyalar.iter().enumerate() {
            for s in &d.siniflar {
                siniflar.entry(s.ad.as_str()).or_default().push((di, s));
            }
        }
        let mut p = Proje {
            kok,
            dosyalar,
            siniflar,
            sabitler: HashMap::new(),
            dbset: HashMap::new(),
            dbcontextler: HashSet::new(),
            varliklar: BTreeSet::new(),
            servisler: HashSet::new(),
            politikalar: HashMap::new(),
            fallback: None,
            genel_filtre: false,
            supheli_ara: vec![],
            json_enum_metin: false,
            camel: true,
            nullable,
            minimal_dogrulama: false,
            routerlar: RefCell::new(BTreeMap::new()),
            baglam: RefCell::new(Baglam::default()),
        };
        // Sabitler: `const string X = "..."` ve `static readonly string X = "..."`; ad hem "Sinif.X" hem "X" ile
        let mut tekil: HashMap<String, Option<String>> = HashMap::new();
        for d in dosyalar {
            for s in &d.siniflar {
                for (ad, _, deger, sabit) in &s.alanlar {
                    if let Some(v) = deger.as_deref().filter(|_| *sabit).and_then(metin_sabiti) {
                        p.sabitler.insert(format!("{}.{ad}", s.ad), v.clone());
                        let e = tekil.entry(ad.clone()).or_insert_with(|| Some(v.clone()));
                        if e.as_deref() != Some(v.as_str()) {
                            *e = None;
                        }
                    }
                }
            }
        }
        for (a, v) in tekil {
            if let Some(v) = v {
                p.sabitler.entry(a).or_insert(v);
            }
        }
        // DbContext'ler ve DbSet'ler
        let baglamlar: Vec<&Sinif> = dosyalar
            .iter()
            .flat_map(|d| &d.siniflar)
            .filter(|s| p.zincir_icerir(s, &["DbContext", "IdentityDbContext", "IdentityUserContext", "ApiAuthorizationDbContext"]))
            .collect();
        for s in baglamlar {
            p.dbcontextler.insert(s.ad.clone());
            for o in &s.ozellikler {
                if temel(&o.tip) == "DbSet" {
                    if let Some(v) = genel_argumanlar(&o.tip).first() {
                        p.dbset.insert(o.ad.clone(), temel(v));
                        p.varliklar.insert(temel(v));
                    }
                }
            }
        }
        static DESEN: OnceLock<[Regex; 6]> = OnceLock::new();
        let [kayit, varlik, politika, fallback, filtre, enum_d] = DESEN.get_or_init(|| {
            [
                Regex::new(r"\.Add(?:Scoped|Transient|Singleton|DbContext|DbContextPool|DbContextFactory|HttpClient|KeyedScoped|KeyedTransient|KeyedSingleton)\s*<\s*([\w.]+)(?:\s*,\s*([\w.]+))?\s*>").unwrap(),
                Regex::new(r"\bEntity<\s*([\w.]+)\s*>").unwrap(),
                Regex::new(r"\bAddPolicy\s*\(").unwrap(),
                Regex::new(r"(?:\bFallbackPolicy\s*=|\.SetFallbackPolicy\s*\()").unwrap(),
                Regex::new(r"Filters\s*\.\s*Add\s*(?:<\s*AuthorizeFilter\s*>|\(\s*new\s+AuthorizeFilter\b)").unwrap(),
                Regex::new(r"JsonStringEnumConverter").unwrap(),
            ]
        });
        for d in dosyalar {
            let m = &d.metin;
            for c in kayit.captures_iter(m) {
                p.servisler.insert(temel(&c[1]));
                if let Some(x) = c.get(2) {
                    p.servisler.insert(temel(x.as_str()));
                }
            }
            if d.siniflar.iter().any(|s| p.dbcontextler.contains(&s.ad)) {
                for c in varlik.captures_iter(m) {
                    p.varliklar.insert(temel(&c[1]));
                }
            }
            if enum_d.is_match(m) {
                p.json_enum_metin = true;
            }
            if m.contains("PropertyNamingPolicy = null") || m.contains("PropertyNamingPolicy=null") {
                p.camel = false;
            }
            if m.contains(".AddValidation()") {
                p.minimal_dogrulama = true;
            }
            if filtre.is_match(m) {
                p.genel_filtre = true;
            }
            for c in politika.find_iter(m) {
                let son = kapanis(m, c.end());
                let govde = &m[c.end()..son];
                let args = ust_duzey_bol(govde, ',');
                let Some(ad) = args.first().and_then(|a| p.deger(a)) else { continue };
                p.politikalar.insert(ad, p.politika_metni(govde));
            }
            for c in fallback.find_iter(m) {
                let son = if c.as_str().ends_with('(') { kapanis(m, c.end()) } else { deyim_sonu(m, c.end()) };
                let ifade = &m[c.end()..son];
                if ifade.contains("Require") || ifade.contains("AddRequirements") {
                    p.fallback = Some(p.politika_metni(ifade));
                }
            }
        }
        // Kimlik denetleyen ara katmanlar: 401 / 403 üreten `UseMiddleware<T>` ya da `app.Use(...)` (tanımı çatıya ait değil)
        const RED: [&str; 6] = ["401", "Status401", "403", "Status403", "Unauthorized", "Forbid"];
        for (di, d) in dosyalar.iter().enumerate() {
            for h in d.zincirler.iter().flat_map(|z| &z.halkalar) {
                match h.ad.as_str() {
                    "UseMiddleware" => {
                        let ad =
                            h.genel.clone().or_else(|| h.argumanlar.first().map(|a| a.metin.trim_start_matches("typeof(").trim_end_matches(')').to_string())).unwrap_or_default();
                        if let Some((_, s)) = p.sinif_bul(&ad) {
                            let metin: String = s.metotlar.iter().filter_map(|m| m.govde.as_ref()).map(|g| g.metin.as_str()).collect();
                            if RED.iter().any(|x| metin.contains(x)) {
                                p.supheli_ara.push(format!("UseMiddleware<{}>", s.ad));
                            }
                        }
                    }
                    "Use" if h.argumanlar.iter().any(|a| a.lambda.as_ref().is_some_and(|l| RED.iter().take(4).any(|x| l.govde.metin.contains(x)))) => {
                        p.supheli_ara.push(format!("{}:{} (app.Use)", p.goreli(di), h.satir));
                    }
                    _ => {}
                }
            }
        }
        p.supheli_ara.dedup();
        p
    }

    /// Politika tanımının metni: `RequireRole(...)` argümanları rollerdir; başka gereksinim (claim, assertion) rol değildir
    fn politika_metni(&self, metin: &str) -> Politika {
        static ROL: OnceLock<Regex> = OnceLock::new();
        let rol = ROL.get_or_init(|| Regex::new(r"RequireRole\s*\(").unwrap());
        let mut pol = Politika { cozuldu: true, ..Default::default() };
        for c in rol.find_iter(metin) {
            let son = kapanis(metin, c.end());
            for a in ust_duzey_bol(&metin[c.end()..son], ',') {
                match self.deger(&a) {
                    Some(v) => pol.roller.extend(v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty())),
                    None => pol.cozuldu = false,
                }
            }
        }
        static GEREK: OnceLock<Regex> = OnceLock::new();
        let gerek = GEREK.get_or_init(|| Regex::new(r"(Require\w+|AddRequirements)\s*\(").unwrap());
        let mut ozet: Vec<String> = gerek.captures_iter(metin).map(|c| c[1].to_string()).collect();
        ozet.dedup();
        pol.ozet = ozet.join(", ");
        pol
    }

    /// Metin değeri: sabit, sabite atıf (`Roller.Yonetici`), `nameof(X)`, birleştirme ve yer tutucuları çözülebilen `$"..."`
    fn deger(&self, ifade: &str) -> Option<String> {
        self.deger_derin(ifade, 0)
    }

    fn deger_derin(&self, ifade: &str, derinlik: usize) -> Option<String> {
        let s = ifade.trim();
        if derinlik > 6 || s.is_empty() {
            return None;
        }
        if let Some(v) = metin_sabiti(s) {
            return Some(v);
        }
        if let Some(g) = s.strip_prefix("$\"").and_then(|x| x.strip_suffix('"')) {
            let mut out = String::new();
            let mut kalan = g;
            while let Some(i) = kalan.find('{') {
                out.push_str(&kalan[..i]);
                let j = kalan[i..].find('}')? + i;
                out.push_str(&self.deger_derin(&kalan[i + 1..j], derinlik + 1)?);
                kalan = &kalan[j + 1..];
            }
            out.push_str(kalan);
            return Some(out);
        }
        if let Some(ic) = s.strip_prefix("nameof(").and_then(|x| x.strip_suffix(')')) {
            return Some(ic.rsplit('.').next().unwrap_or(ic).trim().to_string());
        }
        let parcalar = ust_duzey_bol(s, '+');
        if parcalar.len() > 1 {
            return parcalar.iter().map(|p| self.deger_derin(p, derinlik + 1)).collect::<Option<Vec<_>>>().map(|v| v.concat());
        }
        if !s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
            return None;
        }
        let iki: Vec<&str> = s.rsplitn(3, '.').collect();
        if iki.len() >= 2 {
            if let Some(v) = self.sabitler.get(&format!("{}.{}", iki[1], iki[0])) {
                return Some(v.clone());
            }
        }
        self.sabitler.get(iki[0]).cloned()
    }

    /// Adın adayları; nitelikli adda (`Create.Command`, `Modeller.Urun`) niteleyen sınıf ya da ad alanı tutanlar
    fn adaylar(&self, ad: &str) -> Vec<(usize, &'a Sinif)> {
        let (isim, dis) = nitelikli(ad);
        let hepsi = self.siniflar.get(isim.as_str()).cloned().unwrap_or_default();
        let mut v = hepsi.clone();
        if let Some(d) = dis {
            let uyan: Vec<_> = hepsi
                .iter()
                .filter(|(_, s)| s.ust.as_deref() == Some(d.as_str()) || s.ad_alani.as_deref().is_some_and(|n| n == d || n.ends_with(&format!(".{d}"))))
                .cloned()
                .collect();
            if !uyan.is_empty() {
                v = uyan;
            }
        }
        let b = self.baglam.borrow();
        if v.len() > 1 && b.ad_alani.is_some() {
            let ayni: Vec<_> = v.iter().filter(|(_, s)| s.ad_alani == b.ad_alani).cloned().collect();
            if !ayni.is_empty() {
                v = ayni;
            }
        }
        if v.len() > 1 {
            let yakin: Vec<_> = v.iter().filter(|(_, s)| s.ust.is_some() && (s.ust == b.sinif || s.ust == b.ust)).cloned().collect();
            if !yakin.is_empty() {
                v = yakin;
            }
        }
        v
    }

    /// `f`'yi başvuran sınıfın ad alanı bağlamında çalıştırır
    fn baglamda<T>(&self, s: Option<&Sinif>, f: impl FnOnce() -> T) -> T {
        let yeni = match s {
            Some(s) => Baglam { ad_alani: s.ad_alani.clone(), sinif: Some(s.ad.clone()), ust: s.ust.clone() },
            None => self.baglam.borrow().clone(),
        };
        let eski = self.baglam.replace(yeni);
        let sonuc = f();
        self.baglam.replace(eski);
        sonuc
    }

    fn sinif_bul(&self, ad: &str) -> Option<(usize, &'a Sinif)> {
        // Kısmi sınıflarda üyesi olan parça; arayüzden önce sınıf
        self.adaylar(ad).into_iter().max_by_key(|(_, s)| (s.tur != "interface", s.metotlar.len() + s.ozellikler.len() + s.kurucu.len()))
    }

    /// Sınıfın bütün parçaları (kısmi sınıflar): aynı ad, aynı dış sınıf ve aynı ad alanı
    fn parcalar_s(&self, s: &Sinif) -> Vec<(usize, &'a Sinif)> {
        self.siniflar.get(s.ad.as_str()).into_iter().flatten().filter(|(_, x)| x.ust == s.ust && x.ad_alani == s.ad_alani).copied().collect()
    }

    /// Tip adının parçaları
    fn parcalar(&self, ad: &str) -> Vec<(usize, &'a Sinif)> {
        self.sinif_bul(ad).map(|(_, s)| self.parcalar_s(s)).unwrap_or_default()
    }

    /// Sınıfın taban zinciri (projedeki tabanlar izlenir; dışarıdakilerin adı da listede)
    fn taban_zinciri(&self, s: &Sinif) -> Vec<String> {
        let mut v = vec![];
        let mut kuyruk: Vec<String> = s.tabanlar.clone();
        while let Some(t) = kuyruk.pop() {
            if v.len() > 32 {
                break;
            }
            let ad = temel(&t);
            if v.contains(&ad) {
                continue;
            }
            v.push(ad.clone());
            if let Some((_, b)) = self.sinif_bul(&ad).filter(|(_, b)| b.ad != s.ad) {
                kuyruk.extend(b.tabanlar.iter().cloned());
            }
        }
        v
    }

    fn zincir_icerir(&self, s: &Sinif, adlar: &[&str]) -> bool {
        self.taban_zinciri(s).iter().any(|t| adlar.contains(&t.as_str()))
    }

    /// Projedeki taban sınıflar (yakından uzağa), sınıfın kendisi hariç
    fn taban_siniflari(&self, s: &Sinif) -> Vec<(usize, &'a Sinif)> {
        let mut v: Vec<(usize, &Sinif)> = vec![];
        let mut x = s.tabanlar.first().cloned();
        while let Some(t) = x {
            if v.len() > 8 {
                break;
            }
            let Some((di, b)) = self.sinif_bul(&t).filter(|(_, b)| b.tur == "class" && b.ad != s.ad) else { break };
            if v.iter().any(|(_, y)| y.ad == b.ad) {
                break;
            }
            v.push((di, b));
            x = b.tabanlar.first().cloned();
        }
        v
    }

    fn enum_mu(&self, tip: &str) -> Option<&'a Sinif> {
        self.sinif_bul(tip).map(|(_, s)| s).filter(|s| s.tur == "enum")
    }

    fn basit_mi(&self, tip: &str) -> bool {
        let t = tip.trim().trim_end_matches('?');
        let t = t.strip_prefix("Nullable<").and_then(|x| x.strip_suffix('>')).unwrap_or(t);
        if let Some(o) = koleksiyon_ogesi(t).filter(|_| t.ends_with("[]") || matches!(temel(t).as_str(), "List" | "IEnumerable" | "IList" | "ICollection" | "IReadOnlyList")) {
            return self.basit_mi(&o);
        }
        BASIT.contains(&temel(t).as_str()) || self.enum_mu(t).is_some()
    }

    fn deger_tipi(&self, tip: &str) -> bool {
        let t = temel(tip);
        !matches!(t.as_str(), "string" | "String" | "object")
            && (BASIT.contains(&t.as_str()) || self.enum_mu(tip).is_some() || self.sinif_bul(tip).is_some_and(|(_, s)| s.tur == "struct"))
            && !tip.trim().ends_with("[]")
    }

    /// Tip bir DI servisi mi (Minimal API ve [ApiController] onu servisten verir)
    fn servis_mi(&self, tip: &str) -> bool {
        let t = temel(tip);
        if ENJEKTE.contains(&t.as_str()) || self.dbcontextler.contains(&t) || self.servisler.contains(&t) {
            return true;
        }
        const BILINEN: [&str; 12] = [
            "IMediator",
            "ISender",
            "IPublisher",
            "IMapper",
            "UserManager",
            "SignInManager",
            "RoleManager",
            "IOptions",
            "IOptionsSnapshot",
            "IOptionsMonitor",
            "IMemoryCache",
            "IDistributedCache",
        ];
        if BILINEN.contains(&t.as_str()) {
            return true;
        }
        let mut c = t.chars();
        // Arayüz (`IUrunServisi`): gövdeden okunamaz, DI'dan gelir
        c.next() == Some('I') && c.next().is_some_and(char::is_uppercase)
    }

    fn goreli(&self, di: usize) -> String {
        guvenli::goreli(self.kok, &self.dosyalar[di].yol)
    }

    // ---- Şema ----

    /// Sınıfın alanları: (C# adı, tip, öznitelikler, `required`, satır, varsayılanı var mı); taban sınıflar önce. `json`: JSON
    /// alanları ([JsonIgnore] / [BindNever] hariç); değilse EF Core sütun ve gezintileri ([NotMapped] hariç)
    fn uyeler(&self, s: &'a Sinif, derinlik: usize, json: bool) -> Vec<(String, String, Vec<Oznitelik>, bool, usize, bool)> {
        let mut v = vec![];
        if derinlik > 6 {
            return v;
        }
        if let Some((_, b)) = s.tabanlar.first().and_then(|t| self.sinif_bul(t)).filter(|(_, b)| b.ad != s.ad && matches!(b.tur.as_str(), "class" | "record")) {
            v.extend(self.uyeler(b, derinlik + 1, json));
        }
        if s.konumsal {
            for p in &s.kurucu {
                v.push((p.ad.clone(), p.tip.clone(), p.oznitelikler.clone(), false, p.satir, p.varsayilan.is_some()));
            }
        }
        for (_, parca) in self.parcalar_s(s) {
            for o in parca.ozellikler.iter().filter(|o| !o.statik) {
                let atla: &[&str] = if json { &["JsonIgnore", "BindNever", "IgnoreDataMember"] } else { &["NotMapped"] };
                if o.oznitelikler.iter().any(|x| atla.contains(&x.ad.as_str())) {
                    continue;
                }
                v.retain(|x| x.0 != o.ad);
                v.push((o.ad.clone(), o.tip.clone(), o.oznitelikler.clone(), o.zorunlu_kw, o.satir, false));
            }
        }
        v
    }

    /// C# tipi ve öznitelikleri -> Sezgi alanı. Kurallar class-validator adlarıyla (kart, önizleme ve OpenAPI aynı kural
    /// çözücüyü kullanır). `dogrulama`: çatı DataAnnotations'ı isteğe uyguluyor mu (değilse zorunluluk tahmindir)
    #[allow(clippy::too_many_arguments)]
    fn alan(&self, tip: &str, ozn: &[Oznitelik], zorunlu_kw: bool, varsayilanli: bool, satir: usize, dogrulama: bool, derinlik: usize) -> Alan {
        let mut kurallar = vec![];
        let mut degerler = vec![];
        let ham = tip.trim();
        let nullable = ham.ends_with('?') || ham.starts_with("Nullable<");
        let t = ham.trim_end_matches('?');
        let t = t.strip_prefix("Nullable<").and_then(|x| x.strip_suffix('>')).unwrap_or(t);
        let mut ic = None;
        let ad_tip: Option<&str> = match temel(t).as_str() {
            _ if koleksiyon_ogesi(t).is_some() => {
                kurallar.push("IsArray()".to_string());
                if let Some((_, s)) = koleksiyon_ogesi(t).and_then(|o| self.sinif_bul(&o)).filter(|(_, s)| matches!(s.tur.as_str(), "class" | "record") && derinlik < 3) {
                    ic = Some(self.alanlar(s, dogrulama, derinlik + 1));
                }
                Some("array")
            }
            "string" | "String" | "char" => {
                kurallar.push("IsString()".to_string());
                Some("string")
            }
            "Guid" => {
                kurallar.push("IsUUID()".to_string());
                Some("string")
            }
            "Uri" => {
                kurallar.push("IsUrl()".to_string());
                Some("string")
            }
            "int" | "long" | "short" | "byte" | "sbyte" | "uint" | "ulong" | "ushort" | "Int32" | "Int64" => {
                kurallar.push("IsInt()".to_string());
                Some("int")
            }
            "decimal" | "double" | "float" | "Decimal" | "Double" | "Single" => {
                kurallar.push("IsNumber()".to_string());
                Some("number")
            }
            "bool" | "Boolean" => {
                kurallar.push("IsBoolean()".to_string());
                Some("boolean")
            }
            "DateTime" | "DateTimeOffset" | "DateOnly" => {
                kurallar.push("IsDateString()".to_string());
                Some("date")
            }
            "TimeOnly" | "TimeSpan" => {
                kurallar.push("IsString()".to_string());
                Some("string")
            }
            "Dictionary" | "IDictionary" | "IReadOnlyDictionary" | "JsonElement" | "JsonObject" | "JsonNode" => {
                kurallar.push("IsObject()".to_string());
                Some("object")
            }
            _ => match self.sinif_bul(t) {
                // Enum: System.Text.Json varsayılanı sayıdır; JsonStringEnumConverter kayıtlıysa üye adları
                Some((_, s)) if s.tur == "enum" => {
                    if self.json_enum_metin {
                        degerler = s.enum_uyeleri.clone();
                        kurallar.push(format!("IsIn([{}])", degerler.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
                        Some("enum")
                    } else {
                        kurallar.push("IsInt()".to_string());
                        Some("int")
                    }
                }
                Some((_, s)) if matches!(s.tur.as_str(), "class" | "record" | "struct") => {
                    kurallar.push("ValidateNested()".to_string());
                    if derinlik < 3 {
                        ic = Some(self.alanlar(s, dogrulama, derinlik + 1));
                    }
                    Some("object")
                }
                _ => None,
            },
        };
        let metin_mi = ad_tip == Some("string");
        let dizi_mi = ad_tip == Some("array");
        let gerekli_isaret = ozn.iter().any(|o| o.ad == "Required");
        // Kısıtlar yalnızca çatı doğrulamayı isteğe uyguluyorsa kuraldır ([ApiController], `AddValidation()`); uygulamıyorsa
        // istek kısıtı aşsa da işleyiciye ulaşır
        for o in ozn.iter().filter(|_| dogrulama) {
            let sayi = |i: usize| o.konumsal.get(i).map(|x| x.trim().trim_end_matches(['m', 'M', 'd', 'D', 'f', 'F']).to_string()).filter(|x| x.parse::<f64>().is_ok());
            match o.ad.as_str() {
                "StringLength" => {
                    if let Some(n) = sayi(0) {
                        kurallar.push(format!("MaxLength({n})"));
                    }
                    if let Some(n) = o.arg("MinimumLength").filter(|x| x.parse::<f64>().is_ok()) {
                        kurallar.push(format!("MinLength({n})"));
                    }
                }
                "MaxLength" | "MinLength" => {
                    if let Some(n) = sayi(0) {
                        let ad = match (o.ad.as_str(), dizi_mi) {
                            ("MaxLength", true) => "ArrayMaxSize",
                            ("MinLength", true) => "ArrayMinSize",
                            (a, _) => a,
                        };
                        kurallar.push(format!("{ad}({n})"));
                    }
                }
                "Length" => {
                    if let (Some(a), Some(b)) = (sayi(0), sayi(1)) {
                        if dizi_mi {
                            kurallar.push(format!("ArrayMinSize({a})"));
                            kurallar.push(format!("ArrayMaxSize({b})"));
                        } else {
                            kurallar.push(format!("MinLength({a})"));
                            kurallar.push(format!("MaxLength({b})"));
                        }
                    }
                }
                // [Range(typeof(DateTime), ...)] gibi tipli aralık sayı değildir
                "Range" if !o.konumsal.first().is_some_and(|x| x.starts_with("typeof")) => {
                    if let (Some(a), Some(b)) = (sayi(0), sayi(1)) {
                        kurallar.push(format!("Min({a})"));
                        kurallar.push(format!("Max({b})"));
                    }
                }
                "EmailAddress" => kurallar.push("IsEmail()".into()),
                "Url" => kurallar.push("IsUrl()".into()),
                // RegularExpressionAttribute eşleşmenin bütün değeri kaplamasını ister
                "RegularExpression" => {
                    if let Some(d) = o.konumsal.first().and_then(|x| metin_sabiti(x)) {
                        let capali = d.starts_with('^') && d.ends_with('$') && !d.ends_with("\\$");
                        kurallar.push(if capali { format!("Matches(/{d}/)") } else { format!("Matches(/^(?:{d})$/)") });
                    }
                }
                "AllowedValues" => {
                    let v: Vec<String> = o.konumsal.iter().filter_map(|x| metin_sabiti(x)).collect();
                    if !v.is_empty() && v.len() == o.konumsal.len() {
                        kurallar.push(format!("IsIn([{}])", v.iter().map(|x| format!("'{x}'")).collect::<Vec<_>>().join(", ")));
                        degerler = v;
                    }
                }
                _ => {}
            }
        }
        // [Required] metinde boş metni de reddeder (AllowEmptyStrings varsayılanı false); Nullable bağlamında boş olamayan
        // başvuru tipi MVC'de örtük [Required]'dır
        let deger_tipi = self.deger_tipi(t) && !nullable;
        let ortuk = self.nullable && !nullable && !deger_tipi && ad_tip.is_some() && !varsayilanli;
        if dogrulama && (gerekli_isaret || ortuk) && metin_mi && !ozn.iter().any(|o| o.ad == "Required" && o.arg("AllowEmptyStrings").is_some_and(|x| x == "true")) {
            kurallar.push("IsNotEmpty()".into());
        }
        if nullable {
            kurallar.push("nullable()".into());
        }
        let zorunlu = zorunlu_kw || gerekli_isaret || ortuk;
        // Çatı doğrulamıyorsa yalnızca `required` (System.Text.Json) kesindir; değer tipinde [Required] eksik alanı
        // yakalamaz (JSON'da yoksa varsayılan değer gelir)
        let belirsiz = zorunlu && !zorunlu_kw && (!dogrulama || (deger_tipi && gerekli_isaret));
        Alan { tip: ad_tip.map(String::from), zorunlu, belirsiz, kurallar, satir, kalitimdan: None, degerler, ic }
    }

    fn alanlar(&self, s: &'a Sinif, dogrulama: bool, derinlik: usize) -> BTreeMap<String, Alan> {
        self.baglamda(Some(s), || {
            let mut m = BTreeMap::new();
            for (ad, tip, ozn, kw, satir, varsayilanli) in self.uyeler(s, 0, true) {
                let json_ad = ozn.iter().find(|o| o.ad == "JsonPropertyName").and_then(|o| o.konumsal.first()).and_then(|x| self.deger(x)).unwrap_or_else(|| self.json_adi(&ad));
                m.insert(json_ad, self.alan(&tip, &ozn, kw, varsayilanli, satir, dogrulama, derinlik));
            }
            m
        })
    }

    fn json_adi(&self, ad: &str) -> String {
        if self.camel {
            camel(ad)
        } else {
            ad.to_string()
        }
    }

    fn sema(&self, tip: &str, dogrulama: bool) -> Sema {
        let (oge, dizi) = match koleksiyon_ogesi(tip) {
            Some(o) => (o, true),
            None => (tip.trim().trim_end_matches('?').to_string(), false),
        };
        match self.sinif_bul(&oge).filter(|(_, s)| matches!(s.tur.as_str(), "class" | "record" | "struct")) {
            Some((di, s)) => Sema {
                dto: s.ad.clone(),
                dosya: Some(self.goreli(di)),
                bulunamadi: false,
                kalitim: s.tabanlar.first().cloned(),
                alanlar: self.alanlar(s, dogrulama, 0),
                adaylar: vec![],
                dizi,
            },
            None => Sema { dto: temel(&oge), dosya: None, bulunamadi: true, kalitim: None, alanlar: BTreeMap::new(), adaylar: vec![], dizi },
        }
    }

    /// Yanıt tipi: Task<>, ActionResult<T>, Results<Ok<T>, ...>, Ok<T>, Created<T> açılır; IActionResult / IResult şema
    /// taşımaz
    fn yanit_tipi(tip: &str) -> Option<String> {
        let mut t = tip.trim().to_string();
        for _ in 0..6 {
            let tb = temel(&t);
            match tb.as_str() {
                "Task" | "ValueTask" | "ActionResult" | "Ok" | "Created" | "CreatedAtRoute" | "Accepted" => t = genel_argumanlar(&t).into_iter().next()?,
                "Results" => t = genel_argumanlar(&t).into_iter().find(|x| matches!(temel(x).as_str(), "Ok" | "Created" | "CreatedAtRoute" | "Accepted") && x.contains('<'))?,
                "IActionResult" | "IResult" | "void" | "StatusCodeHttpResult" | "NoContent" | "NotFound" | "BadRequest" | "object" | "dynamic" => return None,
                _ => return Some(t),
            }
        }
        None
    }

    fn yanit(&self, uc: &Uc<'a>) -> Option<crate::adaptor::Yanit> {
        self.baglamda(uc.isleyici.sinif, || self.yanit_(uc))
    }

    fn yanit_(&self, uc: &Uc<'a>) -> Option<crate::adaptor::Yanit> {
        let govde = uc.isleyici.govde.map(|g| g.metin.as_str()).unwrap_or("");
        let olusturma = ["CreatedAtAction(", "CreatedAtRoute(", "Created(", "Results.Created", "TypedResults.Created"].iter().any(|x| govde.contains(x));
        // Açık beyan: [ProducesResponseType(typeof(T), kod)], [ProducesResponseType<T>(kod)], .Produces<T>(kod)
        let mut acik: Vec<(String, Option<u16>)> = vec![];
        for o in uc.isleyici.oznitelikler {
            if matches!(o.ad.as_str(), "ProducesResponseType" | "Produces") {
                let tip = o.genel.clone().or_else(|| {
                    o.konumsal.iter().map(String::as_str).chain(o.arg("Type")).find_map(|x| x.strip_prefix("typeof(").and_then(|y| y.strip_suffix(')')).map(String::from))
                });
                let kod = o.konumsal.iter().map(String::as_str).chain(o.arg("StatusCode")).find_map(kod_ifadesi);
                if let Some(t) = tip {
                    acik.push((t, kod));
                }
            }
        }
        for (t, kod) in &uc.uretir {
            if let Some(t) = t {
                acik.push((t.clone(), *kod));
            }
        }
        let (tip, kaynak, durum) = match acik.into_iter().find(|(_, k)| k.is_none_or(|k| (200..300).contains(&k))) {
            Some((t, k)) => (t, "ProducesResponseType", k),
            // Dönüş tipi bildirilmemiş işleyicide açık tip argümanlı sonuç: `TypedResults.Ok<List<Not>>(...)`
            None => match uc.isleyici.donus.and_then(Self::yanit_tipi) {
                Some(t) => (t, "donus_tipi", None),
                None => {
                    let c = crate::kural::derle(r"\b(?:Typed)?Results\s*\.\s*(Ok|Created|Accepted)\s*<\s*([\w.]+(?:<[\w.,\s<>]+>)?\??)\s*>\s*\(").captures(govde)?;
                    (c[2].to_string(), "donus_tipi", (&c[1] != "Ok").then_some(if &c[1] == "Created" { 201 } else { 202 }))
                }
            },
        };
        let durum = durum.or_else(|| (olusturma && uc.yontemler.iter().any(|y| y == "POST")).then_some(201));
        let sema = self.sema(&tip, true);
        if sema.bulunamadi {
            return None;
        }
        Some(crate::adaptor::Yanit { kaynak: kaynak.into(), kesin: true, durum, sema })
    }

    // ---- Girdiler ----

    /// İşleyicinin girdileri: (gövde, sorgu, yol parametreleri, önizlemenin sınayamadıkları)
    fn girdiler(&self, uc: &Uc<'a>) -> (Vec<Girdi>, Vec<Girdi>, Vec<Girdi>, Vec<String>) {
        self.baglamda(uc.isleyici.sinif, || self.girdiler_(uc))
    }

    fn girdiler_(&self, uc: &Uc<'a>) -> (Vec<Girdi>, Vec<Girdi>, Vec<Girdi>, Vec<String>) {
        let dosya = self.goreli(uc.isleyici.di);
        let yol_adlari: Vec<(String, Option<String>)> = uc.sablon.split('/').filter_map(yol_parcasi).collect();
        let mut govde = vec![];
        let mut sorgu: BTreeMap<String, Alan> = BTreeMap::new();
        let mut form: BTreeMap<String, Alan> = BTreeMap::new();
        let mut yol = vec![];
        let mut notlar = vec![];
        // (ad, tip, öznitelikler, varsayılan, satır): [AsParameters] sınıfın özellikleri parametre gibi açılır
        let mut kuyruk: Vec<SanalParametre> =
            uc.isleyici.parametreler.iter().map(|p| (p.ad.clone(), p.tip.clone(), p.oznitelikler.clone(), p.varsayilan.clone(), p.satir)).collect();
        let mut sira = 0;
        while sira < kuyruk.len() && sira < 200 {
            let (ad, tip, ozn, varsayilan, satir) = kuyruk[sira].clone();
            sira += 1;
            let kaynak = ozn.iter().find(|o| o.ad.starts_with("From") || o.ad == "AsParameters" || o.ad == "Bind").map(|o| o.ad.clone()).unwrap_or_default();
            let bag_adi = ozn.iter().find(|o| o.ad.starts_with("From")).and_then(|o| o.arg("Name")).and_then(|x| self.deger(x)).unwrap_or_else(|| ad.clone());
            if matches!(kaynak.as_str(), "FromServices" | "FromKeyedServices" | "FromHeader") {
                continue;
            }
            let tb = temel(&tip);
            if DOSYA_TIPLERI.contains(&tb.as_str()) || koleksiyon_ogesi(&tip).is_some_and(|o| temel(&o) == "IFormFile") {
                notlar.push(format!("dosya doğrulaması kontrol edilemedi: '{ad}' parametresi ({tb}); dosyanın varlığı, türü ve içeriği önizlemede sınanmaz"));
                if tb != "IFormCollection" {
                    form.insert(bag_adi, Alan { tip: None, zorunlu: true, belirsiz: true, kurallar: vec![], satir, kalitimdan: None, degerler: vec![], ic: None });
                }
                continue;
            }
            if kaynak.is_empty() && self.servis_mi(&tip) && !self.basit_mi(&tip) {
                continue;
            }
            if kaynak == "AsParameters" {
                if let Some((_, s)) = self.sinif_bul(&tip) {
                    for (pad, ptip, pozn, _, psatir, _) in self.uyeler(s, 0, true) {
                        kuyruk.push((pad, ptip, pozn, None, psatir));
                    }
                }
                continue;
            }
            let yol_eslesen = yol_adlari.iter().find(|(a, _)| a.eq_ignore_ascii_case(&bag_adi));
            if kaynak == "FromRoute" || (kaynak.is_empty() && yol_eslesen.is_some()) {
                yol.push(Girdi { ad: Some(yol_eslesen.map(|(a, _)| a.clone()).unwrap_or(bag_adi)), tip: Some(tip.clone()), sema: None });
                continue;
            }
            let basit = self.basit_mi(&tip);
            let varsayilanli = varsayilan.is_some();
            match kaynak.as_str() {
                "FromBody" => govde.push(self.govde_girdisi(&ad, &tip, uc.dogrulama)),
                "FromForm" if !basit => {
                    if let Some((_, s)) = self.sinif_bul(&tip) {
                        for (pad, ptip, pozn, kw, psatir, pv) in self.uyeler(s, 0, true) {
                            form.insert(pad, self.alan(&ptip, &pozn, kw, pv, psatir, uc.dogrulama, 0));
                        }
                    }
                }
                "FromForm" => {
                    form.insert(bag_adi, self.alan(&tip, &ozn, false, varsayilanli, satir, uc.dogrulama, 0));
                }
                "FromQuery" if !basit => {
                    if let Some((_, s)) = self.sinif_bul(&tip) {
                        for (pad, ptip, pozn, kw, psatir, pv) in self.uyeler(s, 0, true) {
                            let qad = pozn.iter().find(|o| o.ad == "FromQuery").and_then(|o| o.arg("Name")).and_then(|x| self.deger(x)).unwrap_or(pad);
                            sorgu.insert(qad, self.alan(&ptip, &pozn, kw, pv, psatir, uc.dogrulama, 0));
                        }
                    }
                }
                _ if basit => {
                    // Basit tipli sorgu parametresi: varsayılanı yoksa ve boş olabilen değilse MVC'de zorunlu sayılmaz
                    // (eksik değer tipin varsayılanıyla bağlanır); Minimal API boş olamayan basit parametreyi zorunlu tutar
                    let mut a = self.alan(&tip, &ozn, false, varsayilanli || uc.mvc, satir, uc.dogrulama, 0);
                    if !uc.mvc && !varsayilanli && !tip.trim().ends_with('?') {
                        a.zorunlu = true;
                        a.belirsiz = false;
                    }
                    sorgu.insert(bag_adi, a);
                }
                _ => govde.push(self.govde_girdisi(&ad, &tip, uc.dogrulama)),
            }
        }
        if !form.is_empty() {
            govde.push(Girdi {
                ad: None,
                tip: Some("Form".into()),
                sema: Some(Sema { adaylar: vec![], dizi: false, dto: "form".into(), dosya: Some(dosya.clone()), bulunamadi: false, kalitim: None, alanlar: form }),
            });
        }
        let sorgu = if sorgu.is_empty() {
            vec![]
        } else {
            vec![Girdi {
                ad: None,
                tip: None,
                sema: Some(Sema { adaylar: vec![], dizi: false, dto: "query".into(), dosya: Some(dosya), bulunamadi: false, kalitim: None, alanlar: sorgu }),
            }]
        };
        // Rota şablonundaki ama işleyicide parametresi olmayan ad da yol parametresidir (tipi kısıttan)
        for (a, kisit) in &yol_adlari {
            if !yol.iter().any(|g| g.ad.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(a))) {
                yol.push(Girdi { ad: Some(a.clone()), tip: Some(kisit.clone().unwrap_or_else(|| "string".into())), sema: None });
            }
        }
        (govde, sorgu, yol, notlar)
    }

    fn govde_girdisi(&self, ad: &str, tip: &str, dogrulama: bool) -> Girdi {
        Girdi { ad: Some(ad.to_string()), tip: Some(tip.to_string()), sema: Some(self.sema(tip, dogrulama)) }
    }

    // ---- Yetki ----

    /// Projede tanımlı ya da adı yetkiyi andıran öznitelik / filtre
    fn ozel_isaret(&self, ad: &str) -> Option<Isaret> {
        const CATI: [&str; 40] = [
            "HttpGet",
            "HttpPost",
            "HttpPut",
            "HttpDelete",
            "HttpPatch",
            "HttpHead",
            "HttpOptions",
            "AcceptVerbs",
            "Route",
            "ApiController",
            "Controller",
            "ProducesResponseType",
            "Produces",
            "Consumes",
            "ApiVersion",
            "MapToApiVersion",
            "ApiExplorerSettings",
            "Obsolete",
            "ActionName",
            "NonAction",
            "NonController",
            "Area",
            "Tags",
            "EndpointSummary",
            "EndpointDescription",
            "ResponseCache",
            "EnableCors",
            "DisableCors",
            "EnableRateLimiting",
            "DisableRateLimiting",
            "RequestSizeLimit",
            "ValidateAntiForgeryToken",
            "IgnoreAntiforgeryToken",
            "AutoValidateAntiforgeryToken",
            "OutputCache",
            "SwaggerOperation",
            "Description",
            "DisplayName",
            "Authorize",
            "AllowAnonymous",
        ];
        if CATI.contains(&ad) {
            return None;
        }
        if let Some((_, s)) = self.sinif_bul(&format!("{ad}Attribute")).or_else(|| self.sinif_bul(ad)).filter(|(_, s)| s.tur == "class") {
            let zincir = self.taban_zinciri(s);
            if zincir.iter().any(|t| t == "AuthorizeAttribute") {
                return Some(Isaret::Yetki { politikalar: vec![], roller: None, ic: None, kaynak: ad.to_string() });
            }
            if zincir.iter().any(|t| matches!(t.as_str(), "IAuthorizationFilter" | "IAsyncAuthorizationFilter" | "IActionFilter" | "IAsyncActionFilter" | "ActionFilterAttribute"))
            {
                let metin: String = s.metotlar.iter().filter_map(|m| m.govde.as_ref()).map(|g| g.metin.as_str()).collect();
                let red = ["UnauthorizedResult", "Unauthorized(", "ForbidResult", "Forbid(", "401", "403", "Status401", "Status403"].iter().any(|x| metin.contains(x));
                return Some(if red { Isaret::OzelGerekli(ad.to_string()) } else { Isaret::Ozel(ad.to_string()) });
            }
            return None;
        }
        static AD: OnceLock<Regex> = OnceLock::new();
        AD.get_or_init(|| Regex::new(r"(?i)(auth|permission|role|apikey|api_key|yetki|izin|token)").unwrap()).is_match(ad).then(|| Isaret::Ozel(ad.to_string()))
    }

    fn oznitelik_isaretleri(&self, ozn: &[Oznitelik]) -> Vec<Isaret> {
        let mut v = vec![];
        for o in ozn {
            match o.ad.as_str() {
                "Authorize" => v.push(Isaret::Yetki {
                    politikalar: o.konumsal.iter().map(String::as_str).chain(o.arg("Policy")).map(String::from).collect(),
                    roller: o.arg("Roles").map(String::from),
                    ic: None,
                    kaynak: if o.konumsal.is_empty() && o.anahtarli.is_empty() {
                        "Authorize".into()
                    } else {
                        format!("Authorize({})", o.konumsal.iter().cloned().chain(o.anahtarli.iter().map(|(k, v)| format!("{k} = {v}"))).collect::<Vec<_>>().join(", "))
                    },
                }),
                "AllowAnonymous" => v.push(Isaret::Anonim("AllowAnonymous".into())),
                a => v.extend(self.ozel_isaret(a)),
            }
        }
        v
    }

    /// Zincirdeki yetki halkası: `RequireAuthorization(...)`, `AllowAnonymous()`, `AddEndpointFilter<X>()`
    fn halka_isareti(&self, h: &Halka) -> Option<Isaret> {
        match h.ad.as_str() {
            "RequireAuthorization" => {
                let mut politikalar = vec![];
                let mut roller = None;
                let mut ic = None;
                for a in &h.argumanlar {
                    if let Some(l) = &a.lambda {
                        ic = Some(l.govde.metin.clone());
                    } else if a.metin.contains("AuthorizeAttribute") || a.metin.contains("Roles") {
                        roller = crate::kural::derle(r"Roles\s*=\s*([^,}]+)").captures(&a.metin).map(|c| c[1].trim().to_string());
                        politikalar.extend(crate::kural::derle(r"Policy\s*=\s*([^,}]+)").captures(&a.metin).map(|c| c[1].trim().to_string()));
                    } else if a.metin.contains("RequireRole") || a.metin.contains("Build()") {
                        ic = Some(a.metin.clone());
                    } else {
                        politikalar.push(a.metin.clone());
                    }
                }
                let args = h.argumanlar.iter().map(|a| a.metin.clone()).collect::<Vec<_>>().join(", ");
                Some(Isaret::Yetki { politikalar, roller, ic, kaynak: if args.is_empty() { "RequireAuthorization()".into() } else { format!("RequireAuthorization({args})") } })
            }
            "AllowAnonymous" => Some(Isaret::Anonim("AllowAnonymous()".into())),
            "AddEndpointFilter" => h.genel.as_deref().and_then(|g| self.ozel_isaret(&temel(g))),
            _ => None,
        }
    }

    /// Giriş ve rol kararı: (giriş, kanıt, roller, rol kanıtı, guard özetleri)
    fn yetki_karari(&self, isaretler: &[Isaret], mvc: bool) -> (String, String, Vec<String>, Option<String>, Vec<String>) {
        let mut guardlar: Vec<String> = isaretler
            .iter()
            .map(|i| match i {
                Isaret::Yetki { kaynak, .. } | Isaret::Anonim(kaynak) | Isaret::OzelGerekli(kaynak) | Isaret::Ozel(kaynak) => kaynak.clone(),
            })
            .collect();
        guardlar.dedup();
        let supheli = !self.supheli_ara.is_empty();
        // [AllowAnonymous] bütün yetki işaretlerini geçersiz kılar (genel politika ve [Authorize] dahil)
        if isaretler.iter().any(|i| matches!(i, Isaret::Anonim(_))) {
            return ("herkese_acik".into(), if supheli { "varsayim_isaret".into() } else { "kesin".into() }, vec![], None, guardlar);
        }
        let yetkiler: Vec<&Isaret> = isaretler.iter().filter(|i| matches!(i, Isaret::Yetki { .. })).collect();
        if !yetkiler.is_empty() {
            let mut roller: Vec<String> = vec![];
            let mut cozulmeyen = false;
            let mut rollu_kume = 0;
            for y in yetkiler {
                let Isaret::Yetki { politikalar, roller: r, ic, .. } = y else { continue };
                let mut kume: Vec<String> = vec![];
                if let Some(r) = r {
                    match self.deger(r) {
                        Some(v) => kume.extend(v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty())),
                        None => cozulmeyen = true,
                    }
                }
                for p in politikalar {
                    match self.deger(p).and_then(|ad| self.politikalar.get(&ad)) {
                        Some(pol) => {
                            kume.extend(pol.roller.iter().cloned());
                            cozulmeyen |= !pol.cozuldu;
                        }
                        None => cozulmeyen = true,
                    }
                }
                if let Some(ic) = ic {
                    let pol = self.politika_metni(ic);
                    kume.extend(pol.roller);
                    cozulmeyen |= !pol.cozuldu;
                }
                if !kume.is_empty() {
                    rollu_kume += 1;
                }
                roller.extend(kume);
            }
            roller.sort();
            roller.dedup();
            // Birden fazla rol koşulu birlikte (sınıf ve eylem) hepsinin geçmesini ister: liste birleşim değil kesişimdir
            let rol_kaniti = (cozulmeyen || rollu_kume > 1).then(|| "varsayim".to_string());
            return ("gerekli".into(), "kesin".into(), roller, rol_kaniti, guardlar);
        }
        // Genel filtre ve genel politika tanımdır: özel bir filtre ([Authorize] verisi taşımaz) onları devre dışı bırakmaz
        if mvc && self.genel_filtre {
            guardlar.push("AuthorizeFilter (MVC genel filtresi)".into());
            return ("gerekli".into(), "kesin".into(), vec![], None, guardlar);
        }
        if let Some(f) = &self.fallback {
            guardlar.push(format!("FallbackPolicy ({})", f.ozet));
            return ("gerekli".into(), "kesin".into(), f.roller.clone(), (!f.cozuldu).then(|| "varsayim".into()), guardlar);
        }
        if isaretler.iter().any(|i| matches!(i, Isaret::OzelGerekli(_))) {
            return ("gerekli".into(), "varsayim_bilinmeyen_guard".into(), vec![], Some("varsayim".into()), guardlar);
        }
        if supheli || isaretler.iter().any(|i| matches!(i, Isaret::Ozel(_))) {
            return ("herkese_acik".into(), "varsayim_bilinmeyen_guard".into(), vec![], None, guardlar);
        }
        ("herkese_acik".into(), "varsayim".into(), vec![], None, guardlar)
    }

    // ---- Çağrı zinciri (servisler, MediatR) ----

    /// Alıcının tipi: işleyici parametreleri, gövdedeki yerel bildirimler, sınıfın alanları, özellikleri ve kurucu
    /// parametreleri
    fn alici_tipi(&self, ad: &str, sinif: Option<&'a Sinif>, parametreler: &[Parametre], metin: &str) -> Option<String> {
        if let Some(p) = parametreler.iter().find(|p| p.ad == ad && !p.tip.is_empty()) {
            return Some(p.tip.clone());
        }
        if let Some(t) = yerel_tip(metin, ad) {
            return Some(t);
        }
        let s = sinif?;
        for (_, parca) in self.parcalar_s(s).into_iter().chain(self.taban_siniflari(s)) {
            if let Some((_, t, _, _)) = parca.alanlar.iter().find(|(a, ..)| a == ad) {
                return Some(t.clone());
            }
            if let Some(o) = parca.ozellikler.iter().find(|o| o.ad == ad) {
                return Some(o.tip.clone());
            }
            if let Some(p) = parca.kurucu.iter().find(|p| p.ad == ad) {
                return Some(p.tip.clone());
            }
        }
        None
    }

    /// Tipin (arayüzse uygulayan sınıfların) adıyla metodu
    fn metot_bul(&self, tip: &str, ad: &str) -> Vec<(usize, &'a Sinif, &'a Metot)> {
        let t = temel(tip);
        let mut siniflar: Vec<(usize, &Sinif)> = self.parcalar(tip).into_iter().filter(|(_, s)| s.tur != "interface").collect();
        if siniflar.is_empty() {
            siniflar = self
                .dosyalar
                .iter()
                .enumerate()
                .flat_map(|(di, d)| d.siniflar.iter().map(move |s| (di, s)))
                .filter(|(_, s)| s.tur == "class" && s.tabanlar.iter().any(|b| temel(b) == t))
                .collect();
        }
        let mut v = vec![];
        for (di, s) in siniflar {
            let mut adaylar = vec![(di, s)];
            adaylar.extend(self.taban_siniflari(s));
            if let Some((mdi, ms, m)) = adaylar.into_iter().find_map(|(di, s)| s.metotlar.iter().find(|m| m.ad == ad && m.govde.is_some()).map(|m| (di, s, m))) {
                v.push((mdi, ms, m));
            }
        }
        v
    }

    /// Gövdenin çağırdığı proje metotları: servis alanları (arayüzse uygulamaları), aynı sınıf, statik sınıflar ve
    /// MediatR `Send` ile istek işleyicileri (`IRequestHandler<T>.Handle`)
    fn cagrilanlar(&self, di: usize, sinif: Option<&'a Sinif>, govde: &Govde, parametreler: &[Parametre]) -> Vec<(usize, &'a Sinif, &'a Metot)> {
        self.baglamda(sinif, || self.cagrilanlar_(di, sinif, govde, parametreler))
    }

    fn cagrilanlar_(&self, di: usize, sinif: Option<&'a Sinif>, govde: &Govde, parametreler: &[Parametre]) -> Vec<(usize, &'a Sinif, &'a Metot)> {
        let mut v = vec![];
        for c in &govde.dna.cagrilar {
            let (alici, ad) = c.rsplit_once('.').unwrap_or(("", c));
            let ad = ad.split('<').next().unwrap_or(ad);
            if matches!(ad, "Send" | "SendAsync" | "Publish" | "PublishAsync") {
                continue;
            }
            let bulunan = match alici {
                "" | "this" | "base" => match sinif {
                    Some(s) => self.metot_bul(&s.ad, ad),
                    None => self.dosyalar[di].siniflar.iter().filter(|s| s.tur == "ust").flat_map(|s| s.metotlar.iter().filter(|m| m.ad == ad).map(move |m| (di, s, m))).collect(),
                },
                a if a.contains('(') => vec![],
                // `services.Queries.Bul(...)`: ilk parçanın tipi, sonra her parça o tipin özelliği ya da alanı
                a if a.contains('.') && !a.starts_with("this.") => {
                    let mut parcalar = a.split('.');
                    let ilk = parcalar.next().unwrap_or_default();
                    let mut tip = self.alici_tipi(ilk, sinif, parametreler, &govde.metin);
                    for p in parcalar {
                        tip = tip
                            .and_then(|t| self.sinif_bul(&t))
                            .and_then(|(_, s)| s.ozellikler.iter().find(|o| o.ad == p).map(|o| o.tip.clone()).or_else(|| s.alanlar.iter().find(|x| x.0 == p).map(|x| x.1.clone())));
                    }
                    tip.map(|t| self.metot_bul(&t, ad)).unwrap_or_default()
                }
                a => match self.alici_tipi(a.trim_start_matches("this."), sinif, parametreler, &govde.metin) {
                    Some(t) => self.metot_bul(&t, ad),
                    // Statik çağrı: `NotUclari.Sil(...)`
                    None if a.chars().next().is_some_and(char::is_uppercase) => self.metot_bul(a, ad),
                    None => vec![],
                },
            };
            v.extend(bulunan);
        }
        for t in self.mesajlar(govde, parametreler, sinif, &["Send", "SendAsync"]).into_iter().map(|(t, _)| t) {
            // `Create.Command`: aynı adlı istek tipleri (her özellikte bir `Command`) içinde bulundukları sınıfla ayrılır
            let (ad, dis) = nitelikli(&t);
            let mut adaylar = vec![];
            for (hdi, d) in self.dosyalar.iter().enumerate() {
                let isleyici_mi = |s: &Sinif| {
                    s.tabanlar.iter().any(|b| {
                        ISTEK_ISLEYICI.contains(&temel(b).as_str())
                            && genel_argumanlar(b).first().is_some_and(|x| {
                                let (xad, xdis) = nitelikli(x);
                                let xdis = xdis.or_else(|| s.ust.clone());
                                xad == ad && (dis.is_none() || xdis.is_none() || dis == xdis)
                            })
                    })
                };
                for s in d.siniflar.iter().filter(|s| isleyici_mi(s)) {
                    if let Some(m) = s.metotlar.iter().find(|m| m.ad == "Handle") {
                        adaylar.push((hdi, s, m));
                    }
                }
            }
            // Birden fazla aday: çağıranın ad alanındaki (C# ad çözümlemesinde önce gelen) seçilir
            let ayni: Vec<_> = adaylar.iter().filter(|(_, s, _)| s.ad_alani.is_some() && s.ad_alani == sinif.and_then(|c| c.ad_alani.clone())).cloned().collect();
            v.extend(if adaylar.len() > 1 && !ayni.is_empty() { ayni } else { std::mem::take(&mut adaylar) });
        }
        v
    }

    /// `mediator.Send(new T(...))` / `Publish(olay)`: gönderilen mesajın tipi ve satırı
    fn mesajlar(&self, govde: &Govde, parametreler: &[Parametre], sinif: Option<&'a Sinif>, adlar: &[&str]) -> Vec<(String, usize)> {
        static D: OnceLock<Regex> = OnceLock::new();
        let re = D.get_or_init(|| Regex::new(r"\.(Send|SendAsync|Publish|PublishAsync)\s*(?:<\s*([\w.]+)\s*>)?\(\s*(?:new\s+([\w.]+)|([A-Za-z_]\w*))").unwrap());
        let mut v = vec![];
        for c in re.captures_iter(&govde.metin) {
            if !adlar.contains(&&c[1]) {
                continue;
            }
            let tip = c.get(2).or(c.get(3)).map(|m| m.as_str().to_string()).or_else(|| c.get(4).and_then(|a| self.alici_tipi(a.as_str(), sinif, parametreler, &govde.metin)));
            if let Some(t) = tip {
                let satir = govde.satir + govde.metin[..c.get(0).map(|m| m.start()).unwrap_or(0)].matches('\n').count();
                v.push((t, satir));
            }
        }
        v
    }

    // ---- Veri (EF Core) ----

    fn baglam_alicisi(&self, alici: &str, sinif: Option<&'a Sinif>, parametreler: &[Parametre], metin: &str) -> bool {
        if let Some(t) = self.alici_tipi(alici, sinif, parametreler, metin) {
            return self.dbcontextler.contains(&temel(&t));
        }
        static AD: OnceLock<Regex> = OnceLock::new();
        AD.get_or_init(|| Regex::new(r"(?i)^_?(db|ctx|context|dbcontext|\w*db|\w*context)$").unwrap()).is_match(alici)
    }

    /// Değişkenin varlık modeli: `var u = new Urun {...}`, `var u = await db.Urunler.FindAsync(...)`, tipli bildirim,
    /// parametre tipi
    fn degisken_varligi(&self, ad: &str, metin: &str, parametreler: &[Parametre], sinif: Option<&'a Sinif>) -> Option<String> {
        let atama = Regex::new(&format!(r"\b{}\s*=\s*([^;]*)", regex::escape(ad))).ok()?;
        for c in atama.captures_iter(metin) {
            let sag = &c[1];
            if let Some(y) = crate::kural::derle(r"\bnew\s+([\w.]+)").captures(sag) {
                let t = temel(&y[1]);
                if self.varliklar.contains(&t) {
                    return Some(t);
                }
            }
            for (dbset, varlik) in &self.dbset {
                if sag.contains(&format!(".{dbset}.")) || sag.contains(&format!(".{dbset})")) {
                    return Some(varlik.clone());
                }
            }
        }
        let t = temel(&self.alici_tipi(ad, sinif, parametreler, metin)?);
        self.varliklar.contains(&t).then_some(t)
    }

    /// Gövdenin EF Core kullanımı: (okunan, yazılan, silinen varlıklar, ham SQL görüldü mü) [Yorum]
    fn ef_kullanimi(&self, metin: &str, parametreler: &[Parametre], sinif: Option<&'a Sinif>) -> (Vec<String>, Vec<String>, Vec<String>, bool) {
        static D: OnceLock<[Regex; 4]> = OnceLock::new();
        let [erisim, kume, alici_yazma, ham] = D.get_or_init(|| {
            [
                Regex::new(r"\.\s*([A-Z]\w*)\b").unwrap(),
                Regex::new(r"\b([A-Za-z_]\w*)\s*\.\s*Set\s*<\s*([\w.]+)\s*>\s*\(\s*\)").unwrap(),
                Regex::new(r"\b([A-Za-z_]\w*)\s*\.\s*(Add|AddAsync|AddRange|AddRangeAsync|Update|UpdateRange|Remove|RemoveRange|Attach)\s*\(\s*(?:new\s+([\w.]+)|([A-Za-z_]\w*))").unwrap(),
                Regex::new(r"\b(FromSqlRaw|FromSqlInterpolated|FromSql|ExecuteSqlRaw|ExecuteSqlRawAsync|ExecuteSqlInterpolated|ExecuteSqlInterpolatedAsync|ExecuteSql|ExecuteSqlAsync|SqlQuery|SqlQueryRaw)\s*[<(]|\.Query(?:Async)?\s*<|\.Execute(?:Async)?\s*\(\s*[@$]?\x22").unwrap(),
            ]
        });
        let (mut okur, mut yazar, mut silinen) = (vec![], vec![], vec![]);
        let mut isle = |varlik: String, bas: usize| {
            let son = deyim_sonu(metin, bas).min(bas + 600);
            let deyim = &metin[bas..son];
            let yazma = EF_YAZMA.iter().any(|y| deyim.contains(&format!(".{y}(")) || deyim.contains(&format!(".{y}<"))) || deyim.contains(".ExecuteUpdateAsync(");
            let silme = [".Remove(", ".RemoveRange(", ".ExecuteDelete(", ".ExecuteDeleteAsync("].iter().any(|x| deyim.contains(x));
            if silme {
                silinen.push(varlik.clone());
            }
            if yazma {
                yazar.push(varlik);
            } else {
                okur.push(varlik);
            }
        };
        // `.Urunler` erişimi ve hemen önündeki alıcı (`db.Urunler`, `services.Context.CatalogItems` içindeki `Context`)
        for c in erisim.captures_iter(metin) {
            let Some(varlik) = self.dbset.get(&c[1]) else { continue };
            let bas = c.get(0).map(|m| m.start()).unwrap_or(0);
            let once = metin[..bas].trim_end();
            let alici: String = once.chars().rev().take_while(|ch| ch.is_alphanumeric() || *ch == '_').collect::<Vec<_>>().into_iter().rev().collect();
            if alici.is_empty() || !self.baglam_alicisi(&alici, sinif, parametreler, metin) {
                continue;
            }
            isle(varlik.clone(), c.get(1).map(|m| m.end()).unwrap_or(0));
        }
        for c in kume.captures_iter(metin) {
            let t = temel(&c[2]);
            if self.varliklar.contains(&t) {
                isle(t, c.get(0).map(|m| m.end()).unwrap_or(0));
            }
        }
        for c in alici_yazma.captures_iter(metin) {
            if !self.baglam_alicisi(&c[1], sinif, parametreler, metin) {
                continue;
            }
            let varlik = match (c.get(3), c.get(4)) {
                (Some(t), _) => Some(temel(t.as_str())).filter(|t| self.varliklar.contains(t)),
                (_, Some(a)) => self.degisken_varligi(a.as_str(), metin, parametreler, sinif),
                _ => None,
            };
            if let Some(v) = varlik {
                if c[2].starts_with("Remove") {
                    silinen.push(v.clone());
                }
                yazar.push(v);
            }
        }
        // Değişiklik izleme: DbSet'ten yüklenen varlığın alanına atanıp SaveChanges çağrılıyorsa varlık yazılır
        if metin.contains("SaveChanges") {
            static YUKLEME: OnceLock<Regex> = OnceLock::new();
            let yukleme = YUKLEME.get_or_init(|| Regex::new(r"\b(?:var\s+)?([a-z_]\w*)\s*=\s*(?:await\s+)?([^;]*)").unwrap());
            for c in yukleme.captures_iter(metin) {
                let (degisken, sag) = (&c[1], &c[2]);
                let Some(varlik) = self.dbset.iter().find(|(d, _)| sag.contains(&format!(".{d}.")) || sag.contains(&format!(".{d})"))).map(|(_, v)| v.clone()) else { continue };
                let atama = format!("{degisken}.");
                let degisti = metin.match_indices(&atama).any(|(i, _)| {
                    let kalan = &metin[i + atama.len()..];
                    let ad_son = kalan.find(|ch: char| !(ch.is_alphanumeric() || ch == '_')).unwrap_or(kalan.len());
                    let sonra = kalan[ad_son..].trim_start();
                    ad_son > 0 && sonra.starts_with('=') && !sonra.starts_with("==") && !sonra.starts_with("=>")
                });
                if degisti && (i_onceki(metin, c.get(0).map(|m| m.start()).unwrap_or(0))) {
                    yazar.push(varlik);
                }
            }
        }
        (okur, yazar, silinen, ham.is_match(metin))
    }

    /// İşleyiciden başlayıp çağrılan proje metotlarına (3 kat) inerek veri bağı [Yorum: metinden çıkarım]
    fn veri_bagi(&self, i: &Isleyici<'a>) -> Option<VeriBag> {
        if self.dbset.is_empty() {
            return None;
        }
        let mut bag = VeriBag::default();
        let mut gorulen: HashSet<(usize, usize)> = HashSet::new();
        let Some(g) = i.govde else { return Some(bag) };
        let mut yigin: Vec<Gezinti> = vec![(i.di, i.sinif, g, i.parametreler, 0)];
        while let Some((di, s, g, ps, derinlik)) = yigin.pop() {
            if !gorulen.insert((di, g as *const Govde as usize)) || gorulen.len() > 200 {
                continue;
            }
            let (okur, yazar, _, ham) = self.ef_kullanimi(&g.metin, ps, s);
            bag.okur.extend(okur);
            bag.yazar.extend(yazar);
            bag.eksik_olabilir |= ham;
            if derinlik < 3 {
                for (cdi, cs, m) in self.cagrilanlar(di, s, g, ps) {
                    if let Some(cg) = &m.govde {
                        yigin.push((cdi, Some(cs), cg, &m.parametreler, derinlik + 1));
                    }
                }
            }
        }
        bag.okur.sort();
        bag.okur.dedup();
        bag.yazar.sort();
        bag.yazar.dedup();
        Some(bag)
    }

    // ---- DNA ----

    /// Gövdenin davranış özeti öğeleri, dosyalar arası olanlarla tamamlanmış: yayılan olaylar (MediatR `Publish`),
    /// kuyruğa konan işler (Hangfire `BackgroundJob.Enqueue`) ve EF Core ile silinen varlıklar
    fn tam_dna(&self, g: &Govde, ps: &[Parametre], s: Option<&'a Sinif>) -> GovdeDna {
        let mut d = g.dna.clone();
        for (t, satir) in self.mesajlar(g, ps, s, &["Publish", "PublishAsync"]).into_iter().map(|(t, satir)| (temel(&t), satir)) {
            if !d.tetikler.iter().any(|(a, b, _)| a == "event" && *b == t) {
                d.tetikler.push(("event".into(), t, satir));
            }
        }
        static IS: OnceLock<Regex> = OnceLock::new();
        let is = IS.get_or_init(|| Regex::new(r"BackgroundJob\s*\.\s*(?:Enqueue|Schedule)\s*(?:<\s*([\w.]+)\s*>)?\s*\(\s*(?:\([^)]*\)|\w+)\s*=>\s*(?:\w+\s*\.\s*)?(\w+)").unwrap());
        for c in is.captures_iter(&g.metin) {
            let ad = c.get(1).map(|m| format!("{}.{}", temel(m.as_str()), &c[2])).unwrap_or_else(|| c[2].to_string());
            let satir = g.satir + g.metin[..c.get(0).map(|m| m.start()).unwrap_or(0)].matches('\n').count();
            if !d.tetikler.iter().any(|(a, b, _)| a == "job" && *b == ad) {
                d.tetikler.push(("job".into(), ad, satir));
            }
        }
        let (_, _, silinen, _) = self.ef_kullanimi(&g.metin, ps, s);
        for v in silinen {
            if !d.silinen.contains(&v) {
                d.silinen.push(v);
            }
        }
        d
    }

    /// İşlemin davranış zinciri: işleyici ve çağırdığı servis / MediatR işleyicileri (3 kat)
    fn dna_zinciri(&self, i: &Isleyici<'a>) -> Value {
        let Some(g) = i.govde else { return Value::Null };
        let mut parcalar: Vec<(String, GovdeDna, usize, &'static str)> = vec![];
        let mut gorulen: HashSet<usize> = HashSet::new();
        let mut yigin: Vec<Gezinti> = vec![(i.di, i.sinif, g, i.parametreler, 0)];
        while let Some((di, s, g, ps, derinlik)) = yigin.pop() {
            if !gorulen.insert(g as *const Govde as usize) || parcalar.len() >= 32 {
                continue;
            }
            parcalar.push((self.goreli(di), self.tam_dna(g, ps, s), derinlik, if derinlik == 0 { "isleyici" } else { "servis" }));
            if derinlik >= 3 {
                continue;
            }
            let sonraki: Vec<_> = self
                .cagrilanlar(di, s, g, ps)
                .into_iter()
                .filter_map(|(cdi, cs, m)| m.govde.as_ref().map(|cg| (cdi, Some(cs), cg, m.parametreler.as_slice(), derinlik + 1)))
                .collect();
            yigin.extend(sonraki.into_iter().rev());
        }
        let halkalar: Vec<crate::dna::Halka> =
            parcalar.iter().map(|(dosya, dna, derinlik, tur)| crate::dna::Halka { dosya: dosya.clone(), dna, derinlik: *derinlik, tur }).collect();
        crate::dna::ozet(&halkalar)
    }

    /// Olay ve iş düğümleri: MediatR `INotificationHandler<T>` dinleyicileri (`event:T`) ve arka plan servisleri
    /// (`BackgroundService` / `IHostedService`: `job:Sinif`)
    fn olaylar(&self) -> BTreeMap<String, Value> {
        let mut v: BTreeMap<String, Value> = BTreeMap::new();
        for (di, d) in self.dosyalar.iter().enumerate() {
            for s in d.siniflar.iter().filter(|s| s.tur == "class" && !s.soyut) {
                let mut dinlenen: Vec<(&str, String, &str)> =
                    s.tabanlar.iter().filter(|b| temel(b) == "INotificationHandler").filter_map(|b| genel_argumanlar(b).first().map(|t| ("event", temel(t), "Handle"))).collect();
                let zincir = self.taban_zinciri(s);
                if zincir.iter().any(|t| t == "BackgroundService") {
                    dinlenen.push(("job", s.ad.clone(), "ExecuteAsync"));
                } else if zincir.iter().any(|t| t == "IHostedService") {
                    dinlenen.push(("job", s.ad.clone(), "StartAsync"));
                }
                for (tur, ad, metot_adi) in dinlenen {
                    let Some(m) = s.metotlar.iter().find(|m| m.ad == metot_adi) else { continue };
                    let i = Isleyici {
                        di,
                        sinif: Some(s),
                        parametreler: &m.parametreler,
                        oznitelikler: &m.oznitelikler,
                        donus: None,
                        govde: m.govde.as_ref(),
                        satir: m.satir,
                        belge: None,
                    };
                    let ozet = self.dna_zinciri(&i);
                    let dugum = v.entry(format!("{tur}:{ad}")).or_insert_with(|| json!({ "tur": tur, "ad": ad, "dinleyiciler": [], "tetikler": [], "silinen": [] }));
                    if let Some(l) = dugum["dinleyiciler"].as_array_mut() {
                        l.push(json!({ "fonksiyon": format!("{}.{}", s.ad, m.ad), "kanit": format!("{}:{}", self.goreli(di), m.satir) }));
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
            }
        }
        v
    }

    // ---- EF Core şeması ----

    /// Varlığın tablo adı: `ToTable("x")`, `[Table("x")]`, yoksa DbSet özelliğinin adı (EF Core varsayılanı), o da yoksa
    /// sınıf adı. Dönen: (tablo, kaynağı)
    fn tablo_adi(&self, varlik: &str) -> (String, &'static str) {
        static TABLO: OnceLock<Regex> = OnceLock::new();
        let tablo = TABLO.get_or_init(|| Regex::new(r#"\bToTable\s*\(\s*(?:name\s*:\s*)?"([^"]+)""#).unwrap());
        for d in self.dosyalar {
            let m = &d.metin;
            for (i, _) in m.match_indices(&format!("Entity<{varlik}>")) {
                let son = deyim_sonu(m, i).min(i + 2000);
                if let Some(c) = tablo.captures(&m[i..son]) {
                    return (c[1].to_string(), "ToTable");
                }
            }
            for s in
                d.siniflar.iter().filter(|s| s.tabanlar.iter().any(|b| temel(b) == "IEntityTypeConfiguration" && genel_argumanlar(b).first().is_some_and(|x| temel(x) == varlik)))
            {
                for m in &s.metotlar {
                    if let Some(c) = m.govde.as_ref().and_then(|g| tablo.captures(&g.metin)) {
                        return (c[1].to_string(), "ToTable");
                    }
                }
            }
        }
        if let Some(t) = self.sinif_bul(varlik).and_then(|(_, s)| s.oznitelikler.iter().find(|o| o.ad == "Table").and_then(|o| o.konumsal.first()).and_then(|x| self.deger(x))) {
            return (t, "[Table]");
        }
        match self.dbset.iter().find(|(_, v)| v.as_str() == varlik) {
            Some((ad, _)) => (ad.clone(), "DbSet özelliğinin adı (EF Core varsayılanı)"),
            None => (varlik.to_string(), "varsayılan (sınıf adı)"),
        }
    }

    fn veri_modelleri(&self) -> Value {
        let mut v = vec![];
        for varlik in &self.varliklar {
            let Some((di, s)) = self.sinif_bul(varlik) else { continue };
            let (tablo, kaynak) = self.tablo_adi(varlik);
            let alanlar: Vec<Value> = self
                .uyeler(s, 0, false)
                .into_iter()
                .filter(|(_, t, o, ..)| self.basit_mi(t) && !o.iter().any(|x| x.ad == "NotMapped"))
                .map(|(ad, tip, _, _, satir, _)| json!({ "ad": ad, "tip": tip, "satir": satir }))
                .collect();
            v.push(json!({ "ad": varlik, "tablo": tablo, "tablo_kaynagi": kaynak, "dosya": self.goreli(di), "satir": s.satir, "alanlar": alanlar }));
        }
        json!(v)
    }

    fn veri_dosyalari(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .dosyalar
            .iter()
            .enumerate()
            .filter(|(_, d)| d.siniflar.iter().any(|s| self.dbcontextler.contains(&s.ad) || (self.varliklar.contains(&s.ad) && s.tur != "interface")))
            .map(|(di, _)| self.goreli(di))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Kaskad silmeler: [ebeveyn, çocuk, "dosya:satır"]. Açık yapılandırma (`HasOne(...).WithMany(...)
    /// .OnDelete(DeleteBehavior.Cascade)`, `HasMany(...).WithOne(...).OnDelete(...)`) ve EF Core kuralı: boş olamayan
    /// yabancı anahtarlı (zorunlu) ilişki varsayılan olarak kaskadla silinir; açık `Restrict` / `NoAction` / `SetNull`
    /// kuralı geçersiz kılar
    fn kaskadlar(&self) -> Vec<Value> {
        static D: OnceLock<[Regex; 3]> = OnceLock::new();
        let [iliski, gezinti, davranis] = D.get_or_init(|| {
            [
                Regex::new(r"\b(HasOne|HasMany)\s*(?:<\s*([\w.]+)\s*>)?\s*\(").unwrap(),
                Regex::new(r"=>\s*\w+\s*\.\s*(\w+)").unwrap(),
                Regex::new(r"\.OnDelete\s*\(\s*DeleteBehavior\s*\.\s*(\w+)").unwrap(),
            ]
        });
        // (ebeveyn, çocuk) -> (davranış, "dosya:satır")
        let mut acik: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
        let varlik_d = Regex::new(r"\bEntity<\s*([\w.]+)\s*>").unwrap();
        for (di, d) in self.dosyalar.iter().enumerate() {
            // Yapılandırma metinleri: DbContext.OnModelCreating ve IEntityTypeConfiguration<T>.Configure
            let mut metinler: Vec<(Option<String>, &Govde)> = vec![];
            for s in &d.siniflar {
                let yapilandirma = s.tabanlar.iter().find(|b| temel(b) == "IEntityTypeConfiguration").and_then(|b| genel_argumanlar(b).first().map(|x| temel(x)));
                for m in s.metotlar.iter().filter(|m| m.ad == "OnModelCreating" || (m.ad == "Configure" && yapilandirma.is_some())) {
                    if let Some(g) = &m.govde {
                        metinler.push((yapilandirma.clone(), g));
                    }
                }
            }
            for (varsayilan, g) in metinler {
                let m = &g.metin;
                for c in iliski.captures_iter(m) {
                    let bas = c.get(0).map(|x| x.start()).unwrap_or(0);
                    let varlik = varlik_d.captures_iter(&m[..bas]).last().map(|x| temel(&x[1])).or_else(|| varsayilan.clone());
                    let Some(varlik) = varlik else { continue };
                    let son = deyim_sonu(m, bas).min(bas + 1500);
                    let deyim = &m[bas..son];
                    let ac_son = kapanis(m, c.get(0).map(|x| x.end()).unwrap_or(bas));
                    let arg = &m[c.get(0).map(|x| x.end()).unwrap_or(bas)..ac_son.max(c.get(0).map(|x| x.end()).unwrap_or(bas))];
                    let nav = gezinti.captures(arg).map(|x| x[1].to_string()).or_else(|| metin_sabiti(arg));
                    let nav_tipi = nav.as_deref().and_then(|n| self.sinif_bul(&varlik).and_then(|(_, s)| self.uyeler(s, 0, false).into_iter().find(|u| u.0 == n).map(|u| u.1)));
                    let hedef = c.get(2).map(|x| x.as_str().to_string()).or(nav_tipi);
                    let Some(hedef) = hedef else { continue };
                    let (ebeveyn, cocuk) =
                        if &c[1] == "HasOne" { (temel(&hedef), varlik.clone()) } else { (temel(&koleksiyon_ogesi(&hedef).unwrap_or(hedef.clone())), varlik.clone()) };
                    if let Some(dv) = davranis.captures(deyim) {
                        let satir = g.satir + m[..bas + dv.get(0).map(|x| x.start()).unwrap_or(0)].matches('\n').count();
                        acik.insert((ebeveyn, cocuk), (dv[1].to_string(), format!("{}:{satir}", self.goreli(di))));
                    }
                }
            }
        }
        let mut v: Vec<Value> = acik.iter().filter(|(_, (d, _))| d == "Cascade").map(|((e, c), (_, k))| json!([e, c, k])).collect();
        // Kural: çocukta ebeveyne başvuru (gezinti özelliği) ve boş olamayan `<Gezinti>Id` / `<Ebeveyn>Id` yabancı anahtarı
        for cocuk in &self.varliklar {
            let Some((di, s)) = self.sinif_bul(cocuk) else { continue };
            let uyeler = self.uyeler(s, 0, false);
            for (ad, tip, ozn, ..) in &uyeler {
                let eb = temel(tip);
                if !self.varliklar.contains(&eb) || koleksiyon_ogesi(tip).is_some() || &eb == cocuk {
                    continue;
                }
                let fk = uyeler.iter().find(|u| u.0 == format!("{ad}Id") || u.0 == format!("{eb}Id"));
                let zorunlu = fk.is_some_and(|f| !f.1.trim().ends_with('?') && self.deger_tipi(&f.1)) || ozn.iter().any(|o| o.ad == "Required");
                if !zorunlu || acik.contains_key(&(eb.clone(), cocuk.clone())) {
                    continue;
                }
                let satir = fk.map(|f| f.4).unwrap_or(s.satir);
                v.push(json!([eb, cocuk, format!("{}:{satir}", self.goreli(di))]));
            }
        }
        v
    }

    // ---- Uçlar ----

    fn isleyici_metot(&self, di: usize, s: Option<&'a Sinif>, m: &'a Metot) -> Isleyici<'a> {
        Isleyici {
            di,
            sinif: s,
            parametreler: &m.parametreler,
            oznitelikler: &m.oznitelikler,
            donus: Some(m.donus.as_str()),
            govde: m.govde.as_ref(),
            satir: m.satir,
            belge: m.belge.as_deref(),
        }
    }

    /// Sınıf ve projedeki taban sınıflarının öznitelikleri (taban önce: ASP.NET öznitelikleri kalıtılır)
    fn kalitimli_oznitelikler(&self, s: &'a Sinif) -> Vec<Oznitelik> {
        let mut v = vec![];
        for (_, b) in self.taban_siniflari(s).into_iter().rev() {
            v.extend(b.oznitelikler.iter().cloned());
        }
        for (_, p) in self.parcalar_s(s) {
            v.extend(p.oznitelikler.iter().cloned());
        }
        v
    }

    fn controller_mi(&self, s: &Sinif) -> bool {
        if s.tur != "class" || s.soyut || s.statik || s.oznitelikler.iter().any(|o| o.ad == "NonController") {
            return false;
        }
        s.ad.ends_with("Controller")
            || s.oznitelikler.iter().any(|o| matches!(o.ad.as_str(), "ApiController" | "Controller"))
            || self.zincir_icerir(s, &["Controller", "ControllerBase"])
    }

    /// Controller eylemleri
    fn controller_uclari(&self, genel: &[Isaret], geleneksel: &[String], denetim_var: bool) -> Vec<Uc<'a>> {
        let mut v = vec![];
        let assembly_api = self.dosyalar.iter().any(|d| d.metin.contains("[assembly: ApiController]"));
        let mut gorulen: HashSet<&str> = HashSet::new();
        for (di, d) in self.dosyalar.iter().enumerate() {
            for s in d.siniflar.iter().filter(|s| self.controller_mi(s)) {
                if !gorulen.insert(s.ad.as_str()) {
                    continue;
                }
                let ozn = self.kalitimli_oznitelikler(s);
                let api = assembly_api || ozn.iter().any(|o| o.ad == "ApiController");
                let ctrl_ad = s.ad.strip_suffix("Controller").unwrap_or(&s.ad).to_string();
                let alan = ozn.iter().find(|o| o.ad == "Area").and_then(|o| o.konumsal.first()).and_then(|x| self.deger(x));
                // Sınıfın kendi [Route]'u varsa o, yoksa en yakın tabanınki
                let mut sinif_rotalari: Vec<String> = s
                    .oznitelikler
                    .iter()
                    .filter(|o| o.ad == "Route")
                    .filter_map(|o| o.konumsal.first().map(String::as_str).or(o.arg("template")).and_then(|x| self.deger(x)))
                    .collect();
                if sinif_rotalari.is_empty() {
                    for (_, b) in self.taban_siniflari(s) {
                        sinif_rotalari = b.oznitelikler.iter().filter(|o| o.ad == "Route").filter_map(|o| o.konumsal.first().and_then(|x| self.deger(x))).collect();
                        if !sinif_rotalari.is_empty() {
                            break;
                        }
                    }
                }
                let surumler: Vec<String> =
                    ozn.iter().filter(|o| o.ad == "ApiVersion").filter_map(|o| o.konumsal.first().and_then(|x| self.deger(x).or_else(|| Some(x.clone())))).collect();
                let sinif_isaretleri = self.oznitelik_isaretleri(&ozn);
                let metotlar: Vec<(usize, &Metot)> = self.parcalar_s(s).into_iter().flat_map(|(pdi, p)| p.metotlar.iter().map(move |m| (pdi, m))).collect();
                for (mdi, m) in metotlar {
                    if !m.acik || m.statik || m.oznitelikler.iter().any(|o| o.ad == "NonAction") || m.ad == "Dispose" {
                        continue;
                    }
                    let eylem = m
                        .oznitelikler
                        .iter()
                        .find(|o| o.ad == "ActionName")
                        .and_then(|o| o.konumsal.first())
                        .and_then(|x| self.deger(x))
                        .unwrap_or_else(|| m.ad.strip_suffix("Async").filter(|a| !a.is_empty()).unwrap_or(&m.ad).to_string());
                    // (yöntem, şablon, rota adı)
                    let mut rotalar: Vec<(String, Option<String>, Option<String>)> = vec![];
                    let mut sablonsuz_yontemler = vec![];
                    for o in &m.oznitelikler {
                        if let Some((_, y)) = HTTP_ISARETLERI.iter().find(|(a, _)| *a == o.ad) {
                            let sablon = o.konumsal.first().map(String::as_str).or(o.arg("template")).and_then(|x| self.deger(x));
                            let ad = o.arg("Name").and_then(|x| self.deger(x));
                            match sablon {
                                Some(t) => rotalar.push((y.to_string(), Some(t), ad)),
                                None => sablonsuz_yontemler.push((y.to_string(), ad)),
                            }
                        } else if o.ad == "AcceptVerbs" {
                            for x in o.konumsal.iter().filter_map(|x| self.deger(x)) {
                                sablonsuz_yontemler.push((x.to_uppercase(), None));
                            }
                        }
                    }
                    let eylem_rotalari: Vec<(String, Option<String>)> = m
                        .oznitelikler
                        .iter()
                        .filter(|o| o.ad == "Route")
                        .filter_map(|o| {
                            o.konumsal.first().map(String::as_str).or(o.arg("template")).and_then(|x| self.deger(x)).map(|t| (t, o.arg("Name").and_then(|x| self.deger(x))))
                        })
                        .collect();
                    let yontemler_: Vec<(String, Option<String>)> =
                        if sablonsuz_yontemler.is_empty() && rotalar.is_empty() { vec![("GET".into(), None)] } else { sablonsuz_yontemler.clone() };
                    for (t, ad) in &eylem_rotalari {
                        for (y, yad) in &yontemler_ {
                            rotalar.push((y.clone(), Some(t.clone()), ad.clone().or(yad.clone())));
                        }
                    }
                    if eylem_rotalari.is_empty() {
                        for (y, ad) in &sablonsuz_yontemler {
                            rotalar.push((y.clone(), None, ad.clone()));
                        }
                        // Yöntem işareti olmayan eylem her yönteme açıktır: GET olarak listelenir
                        if rotalar.is_empty() {
                            rotalar.push(("GET".into(), None, None));
                        }
                    }
                    // Tam şablonlar: (yöntem, şablon, rota adı)
                    let mut tamlar: Vec<(String, String, Option<String>)> = vec![];
                    for (y, t, ad) in &rotalar {
                        match t {
                            Some(t) if t.starts_with('/') || t.starts_with("~/") => tamlar.push((y.clone(), sablon_birlestir(t, ""), ad.clone())),
                            _ if !sinif_rotalari.is_empty() => {
                                for sr in &sinif_rotalari {
                                    tamlar.push((y.clone(), sablon_birlestir(sr, t.as_deref().unwrap_or("")), ad.clone()));
                                }
                            }
                            Some(t) => tamlar.push((y.clone(), sablon_birlestir(t, ""), ad.clone())),
                            // Öznitelik rotası yok: geleneksel rota (MapControllerRoute) şablonu
                            None => {
                                for g in geleneksel {
                                    tamlar.push((y.clone(), self.geleneksel_yol(g, &ctrl_ad, &eylem, alan.as_deref(), &m.parametreler), ad.clone()));
                                }
                            }
                        }
                    }
                    let yer_degistir = |t: &str| -> String {
                        let mut t = t.to_string();
                        for (a, b) in [("[controller]", ctrl_ad.as_str()), ("[action]", eylem.as_str()), ("[area]", alan.as_deref().unwrap_or(""))] {
                            while let Some(i) = t.to_ascii_lowercase().find(a) {
                                t.replace_range(i..i + a.len(), b);
                            }
                        }
                        t
                    };
                    let mut isaretler = genel.to_vec();
                    isaretler.extend(sinif_isaretleri.iter().cloned());
                    isaretler.extend(self.oznitelik_isaretleri(&m.oznitelikler));
                    let kalkmis = m.oznitelikler.iter().chain(ozn.iter()).any(|o| o.ad == "Obsolete");
                    let surumler_m: Vec<Option<String>> = {
                        let eylem_s: Vec<String> =
                            m.oznitelikler.iter().filter(|o| o.ad == "MapToApiVersion").filter_map(|o| o.konumsal.first().and_then(|x| self.deger(x))).collect();
                        let l = if eylem_s.is_empty() { surumler.clone() } else { eylem_s };
                        if l.is_empty() {
                            vec![None]
                        } else {
                            l.into_iter().map(Some).collect()
                        }
                    };
                    for (y, t, ad) in tamlar {
                        let t = yer_degistir(&t);
                        for sv in &surumler_m {
                            let sablon = match sv {
                                Some(sv) => crate::kural::derle(r"\{[A-Za-z_]+:apiVersion\}").replace_all(&t, surum_url(sv).as_str()).to_string(),
                                None => t.clone(),
                            };
                            v.push(Uc {
                                yontemler: vec![y.clone()],
                                sablon,
                                isleyici: self.isleyici_metot(mdi, Some(s), m),
                                isaretler: isaretler.clone(),
                                kontrolcu: s.ad.clone(),
                                fonksiyon: kucuk_bas(&eylem),
                                mvc: true,
                                dogrulama: api || m.govde.as_ref().is_some_and(|g| g.metin.contains("ModelState.IsValid")),
                                ozet: m.belge.clone(),
                                kalkmis,
                                surum: sv.clone(),
                                ad: ad.clone(),
                                uretir: vec![],
                                kosul: None,
                                ana: denetim_var,
                                router: Some(format!("{}:{}", self.goreli(di), s.ad)),
                            });
                        }
                    }
                }
            }
        }
        v
    }

    /// Geleneksel rota şablonundan yol: `{controller=Home}/{action=Index}/{id?}` -> `/Urunler/Detay/{id}` (isteğe bağlı
    /// parça yalnızca eylemde o adla parametre varsa)
    fn geleneksel_yol(&self, sablon: &str, ctrl: &str, eylem: &str, alan: Option<&str>, ps: &[Parametre]) -> String {
        let mut parcalar = vec![];
        for p in sablon.split('/').filter(|p| !p.is_empty()) {
            match yol_parcasi(p) {
                Some((a, _)) if a == "controller" => parcalar.push(ctrl.to_string()),
                Some((a, _)) if a == "action" => parcalar.push(eylem.to_string()),
                Some((a, _)) if a == "area" => {
                    if let Some(x) = alan {
                        parcalar.push(x.to_string());
                    }
                }
                Some((a, _)) if p.contains('?') || p.contains('=') => {
                    if ps.iter().any(|x| x.ad.eq_ignore_ascii_case(&a)) {
                        parcalar.push(format!("{{{a}}}"));
                    }
                }
                _ => parcalar.push(p.to_string()),
            }
        }
        sablon_birlestir(&parcalar.join("/"), "")
    }

    /// Bir değişkenin (grup ya da uygulama) çözümü: atandığı zincir, uzantı metodunun `this` parametresi ya da
    /// metot parametresi (çağrı yerlerinden); hiçbiri değilse uygulamanın kendisi
    fn grup_coz(&self, di: usize, z: &Zincir, ifade: &str, derinlik: usize) -> Vec<Grup> {
        let kok = vec![Grup::default()];
        if derinlik > 8 || ifade.is_empty() || !ifade.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return kok;
        }
        let d = &self.dosyalar[di];
        let ayni = |x: &Zincir| x.sinif == z.sinif && x.metot == z.metot;
        if let Some(atama) = d.zincirler.iter().filter(|x| ayni(x) && x.atanan.as_deref() == Some(ifade) && x.satir <= z.satir).max_by_key(|x| x.satir) {
            let mut gruplar = self.zincir_degeri(di, atama, atama.halkalar.len(), derinlik + 1);
            // Ayrı deyimle gruba eklenen işaretler: `g.RequireAuthorization();` (grup işaretleri sıradan bağımsızdır)
            for x in d.zincirler.iter().filter(|x| ayni(x) && x.kok == ifade && x.halkalar.iter().all(|h| !h.ad.starts_with("Map"))) {
                for g in gruplar.iter_mut() {
                    for h in &x.halkalar {
                        self.isaret_uygula(g, h);
                    }
                }
            }
            // Grubu döndüren uzantı metodu (`return api;`): çağrı yerinde ardından gelen işaretler (`app.MapSiparisler()
            // .RequireAuthorization()`) bu gruba uygulanır
            let donduren = z.metot.as_deref().filter(|m| {
                d.siniflar
                    .iter()
                    .filter(|s| z.sinif.as_deref().is_none_or(|a| a == s.ad))
                    .flat_map(|s| &s.metotlar)
                    .any(|x| &x.ad == m && x.govde.as_ref().is_some_and(|g| g.metin.contains(&format!("return {ifade};"))))
            });
            if let Some(m) = donduren {
                for c in self.dosyalar.iter().flat_map(|cd| &cd.zincirler) {
                    for hi in c.halkalar.iter().enumerate().filter(|(_, h)| h.ad == m).map(|(hi, _)| hi) {
                        for x in c.halkalar[hi + 1..].iter().take_while(|x| !x.ad.starts_with("Map")) {
                            for g in gruplar.iter_mut() {
                                self.isaret_uygula(g, x);
                            }
                        }
                    }
                }
            }
            return gruplar;
        }
        // Metot parametresi: uzantı metodu (`this IEndpointRouteBuilder app`) ya da düz parametre
        let Some(metot_adi) = z.metot.as_deref() else { return kok };
        let Some(m) = d.siniflar.iter().filter(|s| z.sinif.as_deref().is_none_or(|a| a == s.ad) || s.tur == "ust").flat_map(|s| &s.metotlar).find(|m| m.ad == metot_adi) else {
            return kok;
        };
        let Some(pi) = m.parametreler.iter().position(|p| p.ad == ifade) else { return kok };
        let bu = m.parametreler[pi].bu;
        let mut v = vec![];
        for (cdi, cd) in self.dosyalar.iter().enumerate() {
            for c in &cd.zincirler {
                for (hi, h) in c.halkalar.iter().enumerate() {
                    if h.ad != metot_adi {
                        continue;
                    }
                    if bu {
                        v.extend(self.zincir_degeri(cdi, c, hi, derinlik + 1));
                    } else {
                        let ai = pi - m.parametreler.iter().take(pi).filter(|p| p.bu).count();
                        if let Some(a) = h.argumanlar.get(ai) {
                            v.extend(self.grup_coz(cdi, c, &a.metin, derinlik + 1));
                        }
                    }
                }
            }
        }
        if v.is_empty() {
            // Çağrı yeri bulunamadı: önek bilinmiyor
            return vec![Grup { kosul: Some("çağrı yeri bulunamadı".into()), ..Default::default() }];
        }
        v
    }

    fn isaret_uygula(&self, g: &mut Grup, h: &Halka) {
        if let Some(i) = self.halka_isareti(h) {
            g.isaretler.push(i);
        }
        match h.ad.as_str() {
            "WithTags" => g.etiketler.extend(h.argumanlar.iter().filter_map(|a| self.deger(&a.metin))),
            "HasApiVersion" | "MapToApiVersion" => g.surumler.extend(api_surumu(&h.argumanlar.iter().map(|a| a.metin.as_str()).collect::<Vec<_>>().join(","))),
            _ => {}
        }
    }

    /// Zincirin ilk `upto` halkasından sonraki değer: grup(lar)
    fn zincir_degeri(&self, di: usize, z: &Zincir, upto: usize, derinlik: usize) -> Vec<Grup> {
        let mut gruplar = if z.kok.is_empty() { vec![Grup::default()] } else { self.grup_coz(di, z, &z.kok, derinlik) };
        for h in z.halkalar.iter().take(upto) {
            if h.ad == "MapGroup" {
                let onek = h.argumanlar.first().and_then(|a| self.deger(&a.metin)).unwrap_or_default();
                for g in gruplar.iter_mut() {
                    let ust = g.id.clone();
                    g.onek = sablon_birlestir(&g.onek, &onek).trim_end_matches('/').to_string();
                    let id = format!("{}:{}", self.goreli(di), g.onek);
                    if g.kosul.is_none() {
                        g.kosul = z.kosul.clone();
                    }
                    let icerme = format!("{}:{}", self.goreli(di), h.satir);
                    let ustler: Vec<Value> = ust.iter().map(|u| json!({ "id": u, "icerme": icerme, "kosul": z.kosul, "kosullu": z.kosul.is_some() })).collect();
                    let mut r = self.routerlar.borrow_mut();
                    let kayit = r.entry(id.clone()).or_insert_with(|| json!({ "id": id, "dosya": self.goreli(di), "satir": h.satir, "ustler": [] }));
                    if let Some(l) = kayit["ustler"].as_array_mut() {
                        for u in ustler {
                            if !l.contains(&u) {
                                l.push(u);
                            }
                        }
                    }
                    g.id = Some(id);
                }
            } else {
                for g in gruplar.iter_mut() {
                    self.isaret_uygula(g, h);
                }
            }
        }
        gruplar
    }

    /// Minimal API uçları ve grup grafiği
    fn minimal_uclari(&self) -> Vec<Uc<'a>> {
        let mut v = vec![];
        for (di, d) in self.dosyalar.iter().enumerate() {
            for z in &d.zincirler {
                for (hi, h) in z.halkalar.iter().enumerate() {
                    let yontemler: Vec<String> = match MAP_YONTEMLERI.iter().find(|(a, _)| *a == h.ad) {
                        Some((_, y)) => vec![y.to_string()],
                        None if h.ad == "MapMethods" => {
                            let re = crate::kural::derle(r#""([A-Za-z]+)""#);
                            h.argumanlar.get(1).map(|a| re.captures_iter(&a.metin).map(|c| c[1].to_uppercase()).collect()).unwrap_or_default()
                        }
                        None if matches!(h.ad.as_str(), "MapHealthChecks" | "MapPrometheusScrapingEndpoint") => vec!["GET".into()],
                        _ => continue,
                    };
                    if yontemler.is_empty() {
                        continue;
                    }
                    let arg = |ad: &str, i: usize| h.argumanlar.iter().find(|a| a.ad.as_deref() == Some(ad)).or_else(|| h.argumanlar.iter().filter(|a| a.ad.is_none()).nth(i));
                    let Some(sablon) = arg("pattern", 0).and_then(|a| self.deger(&a.metin)).or_else(|| (h.ad == "MapPrometheusScrapingEndpoint").then(|| "/metrics".into())) else {
                        continue;
                    };
                    // Sağlık denetimi ve ölçüm uçlarının işleyicisi çatınındır (ikinci argüman seçeneklerdir)
                    let isleyici_arg = arg("handler", if h.ad == "MapMethods" { 2 } else { 1 }).filter(|_| MAP_YONTEMLERI.iter().any(|(a, _)| *a == h.ad) || h.ad == "MapMethods");
                    let bos: &[Parametre] = &[];
                    let isleyici: Isleyici = match isleyici_arg {
                        Some(Arguman { lambda: Some(l), .. }) => Isleyici {
                            di,
                            sinif: z.sinif.as_deref().and_then(|s| d.siniflar.iter().find(|x| x.ad == s)),
                            parametreler: &l.parametreler,
                            oznitelikler: &l.oznitelikler,
                            donus: l.donus.as_deref(),
                            govde: Some(&l.govde),
                            satir: h.satir,
                            belge: None,
                        },
                        Some(a) => {
                            let (sinif_adi, metot_adi) = match a.metin.rsplit_once('.') {
                                Some((s, m)) => (Some(s.to_string()), m.to_string()),
                                None => (z.sinif.clone(), a.metin.clone()),
                            };
                            let bulunan = match &sinif_adi {
                                Some(s) => self.parcalar(s).into_iter().find_map(|(mdi, x)| x.metotlar.iter().find(|m| m.ad == metot_adi).map(|m| (mdi, x, m))),
                                None => d.siniflar.iter().filter(|x| x.tur == "ust").find_map(|x| x.metotlar.iter().find(|m| m.ad == metot_adi).map(|m| (di, x, m))),
                            };
                            match bulunan {
                                Some((mdi, x, m)) => self.isleyici_metot(mdi, Some(x).filter(|x| x.tur != "ust"), m),
                                None => Isleyici { di, sinif: None, parametreler: bos, oznitelikler: &[], donus: None, govde: None, satir: h.satir, belge: None },
                            }
                        }
                        None => Isleyici { di, sinif: None, parametreler: bos, oznitelikler: &[], donus: None, govde: None, satir: h.satir, belge: None },
                    };
                    let gruplar = self.zincir_degeri(di, z, hi, 0);
                    let sonraki: Vec<&Halka> = z.halkalar[hi + 1..].iter().take_while(|x| !x.ad.starts_with("Map")).collect();
                    let mut ad = None;
                    let mut ozet = None;
                    let mut uretir = vec![];
                    let mut etiketler = vec![];
                    let mut uc_isaretleri = vec![];
                    let mut surumler: Vec<String> = vec![];
                    for x in &sonraki {
                        match x.ad.as_str() {
                            "HasApiVersion" | "MapToApiVersion" => surumler.extend(api_surumu(&x.argumanlar.iter().map(|a| a.metin.as_str()).collect::<Vec<_>>().join(","))),
                            "WithName" => ad = x.argumanlar.first().and_then(|a| self.deger(&a.metin)),
                            "WithSummary" | "WithDescription" if ozet.is_none() => ozet = x.argumanlar.first().and_then(|a| self.deger(&a.metin)),
                            "WithTags" => etiketler.extend(x.argumanlar.iter().filter_map(|a| self.deger(&a.metin))),
                            "Produces" => {
                                let tip = x
                                    .genel
                                    .clone()
                                    .or_else(|| x.argumanlar.iter().find_map(|a| a.metin.strip_prefix("typeof(").and_then(|y| y.strip_suffix(')')).map(String::from)));
                                let kod = x.argumanlar.iter().find_map(|a| kod_ifadesi(&a.metin));
                                uretir.push((tip, kod));
                            }
                            _ => uc_isaretleri.extend(self.halka_isareti(x)),
                        }
                    }
                    for g in gruplar {
                        let mut isaretler = g.isaretler.clone();
                        isaretler.extend(uc_isaretleri.iter().cloned());
                        isaretler.extend(self.oznitelik_isaretleri(isleyici.oznitelikler));
                        let tam = sablon_birlestir(&g.onek, &sablon);
                        let kontrolcu = etiketler
                            .first()
                            .or(g.etiketler.first())
                            .cloned()
                            .or_else(|| isleyici.sinif.map(|s| s.ad.clone()))
                            .unwrap_or_else(|| d.yol.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
                        let fonksiyon = match isleyici_arg {
                            Some(a) if a.lambda.is_none() => kucuk_bas(a.metin.rsplit('.').next().unwrap_or(&a.metin)),
                            _ => ad.clone().map(|a| kucuk_bas(&a)).unwrap_or_else(|| fonksiyon_adi(&yontemler[0], &sezgi_yolu(&tam))),
                        };
                        v.push(Uc {
                            yontemler: yontemler.clone(),
                            sablon: tam,
                            isleyici,
                            isaretler,
                            kontrolcu,
                            fonksiyon,
                            mvc: false,
                            dogrulama: self.minimal_dogrulama,
                            ozet: ozet.clone().or_else(|| isleyici.belge.map(String::from)),
                            kalkmis: isleyici.oznitelikler.iter().any(|o| o.ad == "Obsolete"),
                            // Uç ya da grup tek bir API sürümüne bağlıysa o (aynı adrese birden fazla sürüm cevap verebilir)
                            surum: match (surumler.as_slice(), g.surumler.as_slice()) {
                                ([tek], _) | ([], [tek]) => Some(tek.clone()),
                                _ => None,
                            },
                            ad: ad.clone(),
                            uretir: uretir.clone(),
                            kosul: z.kosul.clone().or(g.kosul.clone()),
                            ana: true,
                            router: g.id.clone(),
                        });
                    }
                }
            }
        }
        v
    }

    fn kapilar(&self) -> (Vec<Kapi>, Value) {
        // Uygulama düzeyi: MapControllers() zincirindeki işaretler, geleneksel rotalar, kimlik denetleyen ara katmanlar
        let mut genel = vec![];
        let mut geleneksel = vec![];
        let mut denetim_var = false;
        let mut ana_uygulama = None;
        for (di, d) in self.dosyalar.iter().enumerate() {
            if ana_uygulama.is_none() && (d.metin.contains("WebApplication.CreateBuilder") || d.metin.contains("WebApplication.Create(") || d.metin.contains("CreateHostBuilder")) {
                ana_uygulama = Some(self.goreli(di));
            }
            for z in &d.zincirler {
                for (hi, h) in z.halkalar.iter().enumerate() {
                    match h.ad.as_str() {
                        "MapControllers" | "MapDefaultControllerRoute" | "MapControllerRoute" | "MapAreaControllerRoute" => {
                            denetim_var = true;
                            for x in &z.halkalar[hi + 1..] {
                                genel.extend(self.halka_isareti(x));
                            }
                            if h.ad == "MapDefaultControllerRoute" {
                                geleneksel.push("{controller=Home}/{action=Index}/{id?}".to_string());
                            } else if h.ad != "MapControllers" {
                                let i = if h.ad == "MapAreaControllerRoute" { 2 } else { 1 };
                                if let Some(p) = h
                                    .argumanlar
                                    .iter()
                                    .find(|a| a.ad.as_deref() == Some("pattern"))
                                    .or_else(|| h.argumanlar.iter().filter(|a| a.ad.is_none()).nth(i))
                                    .and_then(|a| self.deger(&a.metin))
                                {
                                    geleneksel.push(p);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let p = self;
        let mut uclar = p.controller_uclari(&genel, &geleneksel, denetim_var);
        for u in &uclar {
            if let Some(r) = &u.router {
                let satir = u.isleyici.sinif.map(|s| s.satir).unwrap_or(1);
                p.routerlar
                    .borrow_mut()
                    .entry(r.clone())
                    .or_insert_with(|| json!({ "id": r, "dosya": r.rsplit_once(':').map(|x| x.0).unwrap_or(""), "satir": satir, "ustler": [] }));
            }
        }
        uclar.extend(p.minimal_uclari());

        let mut kapilar = vec![];
        let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
        let mut islem_kimlikleri: BTreeMap<String, String> = BTreeMap::new();
        let mut kosullu = vec![];
        let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
        for u in &uclar {
            let dosya = p.goreli(u.isleyici.di);
            let yer = format!("{dosya}:{}", u.isleyici.satir);
            let (govde, sorgu, parametreler, notlar) = p.girdiler(u);
            let (giris, kanit, roller, roller_kaniti, guardlar) = p.yetki_karari(&u.isaretler, u.mvc);
            let yol = sezgi_yolu(&u.sablon);
            if !notlar.is_empty() {
                let l = denetlenemeyen.entry(yer.clone()).or_default();
                for n in notlar {
                    if !l.contains(&n) {
                        l.push(n);
                    }
                }
            }
            if let std::collections::btree_map::Entry::Vacant(e) = dna_haritasi.entry(yer.clone()) {
                let v = p.dna_zinciri(&u.isleyici);
                if !v.is_null() && !crate::dna::bos(&v) {
                    e.insert(v);
                }
            }
            if let Some(ad) = &u.ad {
                islem_kimlikleri.insert(yer.clone(), ad.clone());
            }
            if let Some(r) = &u.router {
                kapi_routerlari.insert(yer.clone(), r.clone());
            }
            let yanit = p.yanit(u);
            let veri = p.veri_bagi(&u.isleyici);
            for y in &u.yontemler {
                if u.kosul.is_some() {
                    kosullu.push(format!("{y} {yol}"));
                }
                kapilar.push(Kapi {
                    id: format!("{y} {yol}"),
                    yontem: y.clone(),
                    yol: yol.clone(),
                    kontrolcu: u.kontrolcu.clone(),
                    fonksiyon: u.fonksiyon.clone(),
                    dosya: dosya.clone(),
                    satir: u.isleyici.satir,
                    giris: giris.clone(),
                    guardlar: guardlar.clone(),
                    roller: roller.clone(),
                    roller_kaniti: roller_kaniti.clone(),
                    ozet: u.ozet.clone(),
                    govde: govde.clone(),
                    sorgu: sorgu.clone(),
                    parametreler: parametreler.clone(),
                    kullanimdan_kalkmis: u.kalkmis,
                    yerine: None,
                    surum: u.surum.clone(),
                    uygulama: (!u.ana).then(|| "bağlanmamış controller (MapControllers bulunamadı)".into()),
                    ana: u.ana,
                    giris_kaniti: kanit.clone(),
                    veri: veri.clone(),
                    yanit: yanit.clone(),
                });
            }
        }
        kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
        kapilar.dedup_by(|a, b| a.id == b.id && a.fonksiyon == b.fonksiyon && a.dosya == b.dosya);
        let mut genel_guardlar: Vec<String> = vec![];
        if let Some(f) = &p.fallback {
            genel_guardlar.push(format!("FallbackPolicy ({})", f.ozet));
        }
        if p.genel_filtre {
            genel_guardlar.push("AuthorizeFilter (MVC genel filtresi)".into());
        }
        genel_guardlar.extend(p.supheli_ara.iter().cloned());
        let routerlar: Vec<Value> = p.routerlar.borrow().values().cloned().collect();
        let olaylar = p.olaylar();
        (
            kapilar,
            json!({ "genel_onek": "", "genel_guardlar": genel_guardlar, "ana_uygulama": ana_uygulama, "surum_secimi": null, "kosullu_kapilar": kosullu,
                "denetlenemeyen": denetlenemeyen, "dna": dna_haritasi, "kaskadlar": p.kaskadlar(), "olaylar": olaylar,
                "routerlar": routerlar, "kapi_routerlari": kapi_routerlari, "veri_modelleri": p.veri_modelleri(), "islem_kimlikleri": islem_kimlikleri }),
        )
    }
}

/// `HasApiVersion(1, 0)`, `HasApiVersion(2.0)`, `HasApiVersion(new ApiVersion(1, 0))`, `HasApiVersion("1.0")` -> "1.0"
fn api_surumu(args: &str) -> Option<String> {
    static D: OnceLock<Regex> = OnceLock::new();
    let sayilar: Vec<&str> = D.get_or_init(|| Regex::new(r"\d+(?:\.\d+)?").unwrap()).find_iter(args).map(|m| m.as_str()).collect();
    match sayilar.as_slice() {
        [a, b] if !a.contains('.') && !b.contains('.') => Some(format!("{a}.{b}")),
        [a] if a.contains('.') => Some(a.to_string()),
        [a] => Some(format!("{a}.0")),
        _ => None,
    }
}

/// Eşleşmenin önündeki karakter bir üye erişimi değil (`x.y = ...` içindeki `y = ...` sayılmaz)
fn i_onceki(metin: &str, i: usize) -> bool {
    metin[..i].chars().next_back().is_none_or(|c| c != '.')
}

/// MediatR istek işleyicisi arayüzleri (MediatR ve Mediator kaynak üreteci)
const ISTEK_ISLEYICI: [&str; 4] = ["IRequestHandler", "ICommandHandler", "IQueryHandler", "IStreamRequestHandler"];

/// Nitelikli tip adı: "Create.Command" -> ("Command", Some("Create")), "Command" -> ("Command", None)
fn nitelikli(t: &str) -> (String, Option<String>) {
    let t = t.trim().trim_end_matches('?').split('<').next().unwrap_or(t).trim_start_matches("global::");
    let mut p = t.rsplit('.');
    let ad = p.next().unwrap_or(t).to_string();
    (ad, p.next().map(String::from))
}

/// Gövdedeki yerel değişkenin tipi: `var x = new T(...)`, `var x = ...GetRequiredService<T>()`, `T x = ...`
fn yerel_tip(metin: &str, ad: &str) -> Option<String> {
    let a = regex::escape(ad);
    let atama = Regex::new(&format!(r"\bvar\s+{a}\s*=\s*([^;]*)")).ok()?;
    if let Some(c) = atama.captures(metin) {
        let sag = &c[1];
        static D: OnceLock<[Regex; 2]> = OnceLock::new();
        let [yeni, servis] = D.get_or_init(|| [Regex::new(r"^\s*(?:await\s+)?new\s+([\w.<>]+)").unwrap(), Regex::new(r"Get(?:Required)?Service\s*<\s*([\w.<>]+)\s*>").unwrap()]);
        if let Some(y) = yeni.captures(sag).or_else(|| servis.captures(sag)) {
            return Some(y[1].to_string());
        }
    }
    let bildirim = Regex::new(&format!(r"\b([A-Z][\w.]*(?:<[\w.,\s<>]+>)?\??)\s+{a}\s*[=;]")).ok()?;
    bildirim.captures(metin).map(|c| c[1].to_string())
}
