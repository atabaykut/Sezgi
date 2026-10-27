//! Spring Boot adaptörü (Java): Java kaynaklarından NestJS ve FastAPI adaptörleriyle aynı kapı yapısını (`Kapi`) üretir;
//! kart, MCP, önizleme, OpenAPI, canlı mod ve santral dilden bağımsız çalışır.
//!
//! Kesinlik ilkesi diğer adaptörlerle aynıdır: anlam adlardan değil tanımlardan çıkar.
//! - **Rotalar (kesin):** `@RestController` / `@Controller` sınıfları; sınıfın `@RequestMapping` öneki ile yöntemin
//!   `@Get/Post/Put/Patch/DeleteMapping` ya da `@RequestMapping(method = ...)` yolu Spring'in birleştirdiği gibi
//!   birleşir (dizi yollar her biri bir kapı). Eşleme arayüzde ya da üst sınıfta tanımlıysa (arayüz tabanlı denetleyici)
//!   oradan okunur. Yol sabitlere (`ApiYollari.KULLANICI`) bağlıysa sabit izlenir. `server.servlet.context-path`
//!   (application.properties / .yml) bütün yolların önündedir (genel önek).
//! - **Girdiler:** `@RequestBody` DTO sınıfı ya da record gövdedir; `@PathVariable` yol, `@RequestParam` ve
//!   `@ModelAttribute` sorgudur. Jakarta Validation kısıtları (`@NotNull`, `@NotBlank`, `@Size`, `@Min`, `@Email`,
//!   `@Pattern`...) yalnızca parametre `@Valid` / `@Validated` taşıyorsa uygulanır: taşımıyorsa alanlar zorunlu sayılmaz ve
//!   bu, işlemin "denetlenemeyen" notuna yazılır. İç içe DTO'nun kısıtları yalnızca alan `@Valid` ise çalışır.
//! - **Giriş ve rol:** Spring Security `SecurityFilterChain` tanımı (`requestMatchers(...).permitAll()` /
//!   `authenticated()` / `hasRole(...)`, ilk eşleşen kural) ve yöntem güvenliği (`@PreAuthorize`, `@Secured`,
//!   `@RolesAllowed`; etkinleştiren `@EnableMethodSecurity` aranır). Kuralı tanımdan çözülen karar kesindir; çözülemeyen
//!   eşleyici ya da ifade "tanınmayan guard"dır. Güvenlik bağımlılığı var ama zincir yoksa Spring Boot varsayılanı
//!   (bütün istekler kimlik ister) tahmindir.
//! - **Veri etkisi (yorum):** işleyiciden servis zincirine (alan tipleri, arayüzün uygulaması) inerek Spring Data
//!   depolarının çağrıları (`save` / `delete*` yazma, `find*` / `count*` okuma, `@Modifying @Query` yazma) ve
//!   `EntityManager`. Kaskadlar `@OneToMany(cascade = ALL | REMOVE, orphanRemoval = true)` ve
//!   `@OnDelete(action = CASCADE)` tanımlarından; `@SQLDelete` / `@SoftDelete` yumuşak silmedir.
//! - **Davranış (DNA):** `throw new X(...)` (kodu `@ResponseStatus` ya da `@ExceptionHandler` eşlemesinden),
//!   `orElseThrow`, `ResponseEntity.status(...)`; `publishEvent`, Kafka / JMS / Rabbit gönderimleri ve `@Async`
//!   çağrıları tetikleyicidir. `@EventListener`, `@KafkaListener`, `@Scheduled`, `@Async` yöntemleri olay ve iş düğümüdür.
//! - **Kotlin:** `.kt` kaynakları Kotlin okuyucusuyla (`kotlin.rs`) aynı modele çevrilir; Kotlin'e özgü kurallar burada
//!   uygulanır: kurucu özelliğine hedefsiz yazılan Bean Validation kısıtı parametreye gider ve alanı doğrulamaz
//!   (`@field:` gerekir), jackson-module-kotlin boş olamayan ve varsayılansız alanı zorunlu kılar, `String?` ve varsayılan
//!   değerli istek parametresi isteğe bağlıdır. Exposed tabloları (`object T : Table(...)`) veri modelidir.
//! - **Kapsam dışı:** WebFlux işlevsel rotaları (`RouterFunction`, Kotlin `coRouter { }`) okunmaz.
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tree_sitter::{Node, Parser};

use crate::adaptor::{Alan, Girdi, Kapi, Sema, VeriBag, Yanit};
use crate::dna::{Dal, Firlatma, GovdeDna};
use crate::guvenli;

pub const SURUM: &str = "0.1.0";

/// Bir gövdede tutulacak en fazla öğe
const EN_FAZLA: usize = 32;
/// Rota eşleme anotasyonları ve HTTP yöntemleri
const ESLEMELER: [(&str, &str); 5] = [("GetMapping", "GET"), ("PostMapping", "POST"), ("PutMapping", "PUT"), ("PatchMapping", "PATCH"), ("DeleteMapping", "DELETE")];
/// Spring'in kendisinin verdiği parametre tipleri: girdi değildir
const ENJEKTE: [&str; 33] = [
    "HttpServletRequest",
    "HttpServletResponse",
    "ServletRequest",
    "ServletResponse",
    "HttpSession",
    "Principal",
    "Authentication",
    "Model",
    "ModelMap",
    "ModelAndView",
    "BindingResult",
    "Errors",
    "Locale",
    "WebRequest",
    "NativeWebRequest",
    "ServerWebExchange",
    "ServerHttpRequest",
    "ServerHttpResponse",
    "UriComponentsBuilder",
    "RedirectAttributes",
    "SessionStatus",
    "HttpEntity",
    "RequestEntity",
    "HttpMethod",
    "TimeZone",
    "ZoneId",
    "InputStream",
    "OutputStream",
    "Reader",
    "Writer",
    "SecurityContext",
    "Jwt",
    "UserDetails",
];
/// Parametreyi istek girdisi olmaktan çıkaran anotasyonlar (başlık, çerez, oturum, giriş yapan kullanıcı)
const GIRDI_DISI: [&str; 9] = [
    "RequestHeader",
    "CookieValue",
    "RequestAttribute",
    "SessionAttribute",
    "AuthenticationPrincipal",
    "CurrentSecurityContext",
    "Value",
    "MatrixVariable",
    "RegisteredOAuth2AuthorizedClient",
];
/// Spring Data depo tabanları: ilk tip argümanı yönetilen varlıktır
const DEPO_TABANLARI: [&str; 17] = [
    "Repository",
    "CrudRepository",
    "ListCrudRepository",
    "PagingAndSortingRepository",
    "ListPagingAndSortingRepository",
    "JpaRepository",
    "JpaSpecificationExecutor",
    "MongoRepository",
    "ReactiveCrudRepository",
    "ReactiveSortingRepository",
    "ReactiveMongoRepository",
    "R2dbcRepository",
    "CoroutineCrudRepository",
    "ElasticsearchRepository",
    "Neo4jRepository",
    "CassandraRepository",
    "RevisionRepository",
];
/// Koleksiyon tipleri (JSON dizisi)
const KOLEKSIYONLAR: [&str; 9] = ["List", "Set", "Collection", "Iterable", "ArrayList", "LinkedList", "HashSet", "SortedSet", "LinkedHashSet"];
/// Varlığın koşullu kaydı: denetleyici yalnızca bazı ortamlarda var
const KOSULLU: [&str; 6] = ["Profile", "ConditionalOnProperty", "ConditionalOnExpression", "ConditionalOnBean", "ConditionalOnMissingBean", "ConditionalOnClass"];

// ---- Model ----

/// Bir anotasyon: basit adı, argümanları ((anahtar, değer metni); konumsal argümanın anahtarı yok: Java'da `value`)
#[derive(Clone, Debug, Default)]
pub struct Anotasyon {
    pub ad: String,
    pub argumanlar: Vec<(Option<String>, String)>,
    pub satir: usize,
    /// Kotlin kullanım yeri hedefi (`@field:NotBlank` -> "field"); Java'da yok
    pub hedef: Option<String>,
}

impl Anotasyon {
    /// Anahtarlardan birinin değeri; konumsal argüman `value` sayılır
    pub(crate) fn deger(&self, anahtarlar: &[&str]) -> Option<&str> {
        self.argumanlar
            .iter()
            .find(|(a, _)| match a {
                Some(a) => anahtarlar.contains(&a.as_str()),
                None => anahtarlar.contains(&"value"),
            })
            .map(|(_, v)| v.as_str())
    }
}

fn anotasyon<'a>(l: &'a [Anotasyon], ad: &str) -> Option<&'a Anotasyon> {
    l.iter().find(|a| a.ad == ad)
}

fn var_mi(l: &[Anotasyon], ad: &str) -> bool {
    l.iter().any(|a| a.ad == ad)
}

/// Alan, parametre ya da record bileşeni
#[derive(Clone, Debug, Default)]
pub struct Degisken {
    pub ad: String,
    pub tip: String,
    pub anotasyonlar: Vec<Anotasyon>,
    pub satir: usize,
    pub statik: bool,
    /// İlk değer ifadesi (sabitler için); Kotlin parametresinin varsayılan değeri
    pub deger: Option<String>,
    /// Kotlin kurucu özelliği (`class X(val a: T)`)
    pub kurucu: bool,
    /// Kotlin: boş olamayan tip, varsayılan değer yok (jackson-module-kotlin bu alanı zorunlu kılar)
    pub kotlin_zorunlu: bool,
    /// Kotlin: boş olabilen tip (`String?`)
    pub bos_olabilir: bool,
}

/// Gövdedeki bir yöntem çağrısı: alıcı metni (`this.servis`, `repo`, boş), yöntem adı, argüman metinleri
#[derive(Clone, Debug, Default)]
pub struct Cagri {
    pub nesne: String,
    pub ad: String,
    pub argumanlar: Vec<String>,
    pub satir: usize,
    /// Kotlin tip argümanları (`call.receive<Dto>()`)
    pub tip_argumanlari: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Yontem {
    pub ad: String,
    pub donus: String,
    pub parametreler: Vec<Degisken>,
    pub anotasyonlar: Vec<Anotasyon>,
    pub satir: usize,
    /// Javadoc'un ilk cümlesi
    pub belge: Option<String>,
    /// Gövdenin metni (güvenlik zinciri ve özel durum eşlemesi bundan okunur)
    pub govde_metni: String,
    pub cagrilar: Vec<Cagri>,
    /// Yerel değişkenlerin ve parametrelerin tipleri
    pub yereller: HashMap<String, String>,
    pub dna: GovdeDna,
    pub govdeli: bool,
    /// Kotlin: tipi yazılmamış yerelin değerini veren çağrı (`val g = servis.getir(id)` -> g: ("servis", "getir"))
    pub kaynak_cagrilari: HashMap<String, (String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct Sinif {
    pub ad: String,
    /// "class", "interface", "record", "enum"
    pub tur: &'static str,
    pub anotasyonlar: Vec<Anotasyon>,
    pub ust: Option<String>,
    /// Uyguladığı (arayüz için genişlettiği) arayüzler
    pub arayuzler: Vec<String>,
    pub tip_parametreleri: Vec<String>,
    pub alanlar: Vec<Degisken>,
    pub yontemler: Vec<Yontem>,
    /// Enum sabitleri
    pub sabitler: Vec<String>,
    pub satir: usize,
    pub soyut: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Dosya {
    pub yol: PathBuf,
    pub paket: String,
    pub iceaktarmalar: Vec<String>,
    pub siniflar: Vec<Sinif>,
    pub satir_sayisi: usize,
    pub hatali_satirlar: Vec<usize>,
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
    n.named_children(&mut w).collect()
}

fn tek_satir(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn anotasyon_oku(n: Node, k: &[u8]) -> Anotasyon {
    let ad = n.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
    let ad = ad.rsplit('.').next().unwrap_or(&ad).to_string();
    let mut argumanlar = vec![];
    if let Some(l) = n.child_by_field_name("arguments") {
        for c in cocuklar(l) {
            match c.kind() {
                "line_comment" | "block_comment" => {}
                "element_value_pair" => {
                    argumanlar.push((c.child_by_field_name("key").map(|x| metin(x, k)), c.child_by_field_name("value").map(|x| metin(x, k)).unwrap_or_default()))
                }
                _ => argumanlar.push((None, metin(c, k))),
            }
        }
    }
    Anotasyon { ad, argumanlar, satir: satir(n), hedef: None }
}

/// Düğümün anotasyonları ve değiştirici sözcükleri (`static`, `abstract`, `final`...)
fn degistiriciler(n: Node, k: &[u8]) -> (Vec<Anotasyon>, Vec<String>) {
    let (mut a, mut s) = (vec![], vec![]);
    let mut w = n.walk();
    for c in n.children(&mut w) {
        match c.kind() {
            "modifiers" => {
                let mut w2 = c.walk();
                for m in c.children(&mut w2) {
                    match m.kind() {
                        "annotation" | "marker_annotation" => a.push(anotasyon_oku(m, k)),
                        x if !m.is_named() => s.push(x.to_string()),
                        _ => {}
                    }
                }
            }
            "annotation" | "marker_annotation" => a.push(anotasyon_oku(c, k)),
            _ => {}
        }
    }
    (a, s)
}

/// Java metin sabiti: "\"/api\"" -> "/api" (kaçış dizileri çözülür; metin bloğu dahil)
pub(crate) fn java_metni(s: &str) -> Option<String> {
    let s = s.trim();
    let ic = match s.strip_prefix("\"\"\"").and_then(|x| x.strip_suffix("\"\"\"")) {
        Some(x) => x.trim_start_matches(['\r', '\n']),
        None => s.strip_prefix('"')?.strip_suffix('"')?,
    };
    let mut out = String::new();
    let mut it = ic.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(x) => out.push(x),
            None => {}
        }
    }
    Some(out)
}

fn degisken_oku(n: Node, k: &[u8]) -> Option<Degisken> {
    let (anotasyonlar, soz) = degistiriciler(n, k);
    let (ad, tip) = match n.kind() {
        "formal_parameter" => (n.child_by_field_name("name").map(|x| metin(x, k))?, n.child_by_field_name("type").map(|x| metin(x, k))?),
        "spread_parameter" => {
            let c = cocuklar(n);
            let tip = c.iter().find(|x| !matches!(x.kind(), "modifiers" | "variable_declarator" | "annotation" | "marker_annotation")).map(|x| metin(*x, k))?;
            let ad = c.iter().find(|x| x.kind() == "variable_declarator").and_then(|x| x.child_by_field_name("name")).map(|x| metin(x, k))?;
            (ad, format!("{tip}[]"))
        }
        _ => return None,
    };
    Some(Degisken { ad, tip, anotasyonlar, satir: satir(n), statik: soz.iter().any(|s| s == "static"), ..Default::default() })
}

/// Javadoc'un ilk cümlesi: yöntemden hemen önceki `/** ... */`
fn javadoc(n: Node, k: &[u8]) -> Option<String> {
    let onceki = n.prev_named_sibling().filter(|p| p.kind() == "block_comment")?;
    let m = metin(onceki, k);
    let govde = m.strip_prefix("/**")?.strip_suffix("*/")?;
    let satirlar: Vec<String> = govde.lines().map(|l| l.trim().trim_start_matches('*').trim().to_string()).take_while(|l| !l.starts_with('@')).filter(|l| !l.is_empty()).collect();
    let tum = satirlar.join(" ");
    let ilk = tum.split(". ").next().unwrap_or(&tum).trim().trim_end_matches('.').to_string();
    (!ilk.is_empty()).then_some(ilk)
}

fn yontem_oku(n: Node, k: &[u8]) -> Yontem {
    let (anotasyonlar, _) = degistiriciler(n, k);
    let ad_dugum = n.child_by_field_name("name");
    let mut y = Yontem {
        ad: ad_dugum.map(|a| metin(a, k)).unwrap_or_default(),
        donus: n.child_by_field_name("type").map(|t| metin(t, k)).unwrap_or_default(),
        anotasyonlar,
        satir: ad_dugum.map(satir).unwrap_or_else(|| satir(n)),
        belge: javadoc(n, k),
        ..Default::default()
    };
    if let Some(p) = n.child_by_field_name("parameters") {
        y.parametreler = cocuklar(p).into_iter().filter_map(|c| degisken_oku(c, k)).collect();
    }
    for p in &y.parametreler {
        y.yereller.insert(p.ad.clone(), p.tip.clone());
    }
    if let Some(b) = n.child_by_field_name("body") {
        y.govdeli = true;
        y.govde_metni = metin(b, k);
        let mut yereller = std::mem::take(&mut y.yereller);
        let mut cagrilar = vec![];
        y.dna = govde_oku(b, k, &mut yereller, &mut cagrilar);
        y.yereller = yereller;
        y.cagrilar = cagrilar;
    }
    y
}

/// Sınıf gövdesinin üyeleri (enum gövdesinin bildirim bölümü dahil); iç sınıflar ayrıca listelenir
fn uyeler(govde: Node, k: &[u8], s: &mut Sinif, out: &mut Vec<Sinif>) {
    for c in cocuklar(govde) {
        match c.kind() {
            "field_declaration" | "constant_declaration" => {
                let (anotasyonlar, soz) = degistiriciler(c, k);
                // Arayüzdeki alan her zaman statik sabittir
                let statik = soz.iter().any(|x| x == "static") || c.kind() == "constant_declaration";
                let tip = c.child_by_field_name("type").map(|t| metin(t, k)).unwrap_or_default();
                for d in cocuklar(c).into_iter().filter(|d| d.kind() == "variable_declarator") {
                    let Some(ad) = d.child_by_field_name("name").map(|a| metin(a, k)) else { continue };
                    let deger = d.child_by_field_name("value").map(|v| metin(v, k));
                    s.alanlar.push(Degisken { ad, tip: tip.clone(), anotasyonlar: anotasyonlar.clone(), satir: satir(d), statik, deger, ..Default::default() });
                }
            }
            "method_declaration" => s.yontemler.push(yontem_oku(c, k)),
            "enum_constant" => {
                if let Some(a) = c.child_by_field_name("name") {
                    s.sabitler.push(metin(a, k));
                }
            }
            "enum_body_declarations" => uyeler(c, k, s, out),
            "class_declaration" | "interface_declaration" | "record_declaration" | "enum_declaration" | "annotation_type_declaration" => sinif_oku(c, k, out),
            _ => {}
        }
    }
}

fn tip_listesi(n: Node, k: &[u8]) -> Vec<String> {
    cocuklar(n).into_iter().filter(|c| c.kind() == "type_list").flat_map(cocuklar).map(|t| metin(t, k)).collect()
}

fn sinif_oku(n: Node, k: &[u8], out: &mut Vec<Sinif>) {
    let tur = match n.kind() {
        "class_declaration" => "class",
        "interface_declaration" => "interface",
        "record_declaration" => "record",
        "enum_declaration" => "enum",
        "annotation_type_declaration" => "annotation",
        _ => return,
    };
    let (anotasyonlar, soz) = degistiriciler(n, k);
    let ad_dugum = n.child_by_field_name("name");
    let mut s = Sinif {
        ad: ad_dugum.map(|a| metin(a, k)).unwrap_or_default(),
        tur,
        anotasyonlar,
        satir: ad_dugum.map(satir).unwrap_or_else(|| satir(n)),
        soyut: tur == "interface" || soz.iter().any(|x| x == "abstract"),
        ..Default::default()
    };
    if let Some(tp) = n.child_by_field_name("type_parameters") {
        s.tip_parametreleri = cocuklar(tp).into_iter().filter_map(|p| metin(p, k).split_whitespace().next().map(String::from)).collect();
    }
    if let Some(u) = n.child_by_field_name("superclass") {
        s.ust = cocuklar(u).first().map(|t| metin(*t, k));
    }
    if let Some(i) = n.child_by_field_name("interfaces") {
        s.arayuzler = tip_listesi(i, k);
    }
    for c in cocuklar(n).into_iter().filter(|c| c.kind() == "extends_interfaces") {
        s.arayuzler.extend(tip_listesi(c, k));
    }
    // Record bileşenleri alandır (Jackson bunları okur ve yazar)
    if let Some(p) = n.child_by_field_name("parameters") {
        s.alanlar.extend(cocuklar(p).into_iter().filter_map(|c| degisken_oku(c, k)));
    }
    let mut ic = vec![];
    if let Some(b) = n.child_by_field_name("body") {
        uyeler(b, k, &mut s, &mut ic);
    }
    out.push(s);
    out.extend(ic);
}

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

pub fn ayristir_metin(yol: &Path, kaynak: &str) -> Option<Dosya> {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).ok()?;
    let agac = p.parse(kaynak, None)?;
    let k = kaynak.as_bytes();
    let mut d = Dosya { yol: yol.to_path_buf(), satir_sayisi: kaynak.lines().count(), ..Default::default() };
    for c in cocuklar(agac.root_node()) {
        match c.kind() {
            "package_declaration" => {
                d.paket = cocuklar(c).iter().find(|x| matches!(x.kind(), "scoped_identifier" | "identifier")).map(|x| metin(*x, k)).unwrap_or_default();
            }
            "import_declaration" => {
                let m = metin(c, k);
                let m = m.trim().trim_start_matches("import").trim().trim_start_matches("static ").trim().trim_end_matches(';').trim().to_string();
                d.iceaktarmalar.push(m);
            }
            _ => sinif_oku(c, k, &mut d.siniflar),
        }
    }
    hatalar(agac.root_node(), &mut d.hatali_satirlar);
    Some(d)
}

// ---- Gövde: çağrılar, yerel tipler ve davranış özeti ----

/// Spring ve Spring Security'nin yerleşik özel durumları -> HTTP kodu
pub(crate) fn yerlesik_kod(ad: &str) -> Option<u16> {
    Some(match ad {
        "AccessDeniedException" => 403,
        "AuthenticationException"
        | "BadCredentialsException"
        | "UsernameNotFoundException"
        | "InsufficientAuthenticationException"
        | "AuthenticationCredentialsNotFoundException" => 401,
        "MethodArgumentNotValidException" | "ConstraintViolationException" | "HttpMessageNotReadableException" | "MissingServletRequestParameterException" => 400,
        "NoHandlerFoundException" | "NoResourceFoundException" => 404,
        "HttpRequestMethodNotSupportedException" => 405,
        _ => return None,
    })
}

/// İki yollu ifadenin bağlamı: atandığı değişken ya da içinde geçtiği çağrı
fn dal_baglami(n: Node, k: &[u8]) -> String {
    let mut x = n;
    for _ in 0..4 {
        let Some(u) = x.parent() else { break };
        match u.kind() {
            "variable_declarator" => return u.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default(),
            "assignment_expression" => return u.child_by_field_name("left").map(|a| metin(a, k)).unwrap_or_default(),
            "method_invocation" => return u.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default(),
            "return_statement" => return "return".into(),
            _ => x = u,
        }
    }
    String::new()
}

fn basit_mi(n: Node) -> bool {
    matches!(n.kind(), "identifier" | "field_access" | "string_literal" | "decimal_integer_literal" | "true" | "false" | "null_literal" | "character_literal")
}

/// Yumuşak silme alanının ayarlayıcısı (`setDeleted(true)`, `setDeletedAt(now())`)
fn yumusak_ayarlayici(ad: &str, arg: &str) -> bool {
    const ALANLAR: [&str; 7] = ["setDeleted", "setIsDeleted", "setDeletedAt", "setTrashed", "setIsTrashed", "setTrashedAt", "setRemoved"];
    let a = arg.trim();
    ALANLAR.contains(&ad) && (a == "true" || a == "Boolean.TRUE" || a.contains("now") || a.contains("Date") || a.contains("Instant"))
}

/// Oluşturulan tipin parca adı: `new com.x.Foo<>(...)` -> "Foo"
fn olusturulan(n: Node, k: &[u8]) -> String {
    temel_ad(&n.child_by_field_name("type").map(|t| metin(t, k)).unwrap_or_default())
}

/// Bir özel durumu fırlatma kaydı olarak ekler (kodu yerleşik addan ya da argümandaki `HttpStatus.X`'ten)
fn firlatma_ekle(d: &mut GovdeDna, yeni: Node, konum: Node, govde: Node, k: &[u8], kosul: Option<String>) {
    if d.firlatmalar.len() >= EN_FAZLA {
        return;
    }
    let ad = olusturulan(yeni, k);
    let args: Vec<String> = yeni.child_by_field_name("arguments").map(cocuklar).unwrap_or_default().into_iter().map(|a| metin(a, k)).collect();
    let durum = yerlesik_kod(&ad).or_else(|| args.iter().take(2).find_map(|a| crate::dna::kod_ifadesi(a)));
    let kosul = kosul.or_else(|| crate::dna::kosul_bul(konum, govde, k, false));
    let sahiplik = kosul.as_deref().is_some_and(|c| sahiplik(c, durum));
    d.firlatmalar.push(Firlatma { durum, istisna: ad, kosul, satir: satir(konum), sahiplik });
}

/// Sahiplik karşılaştırması; Java'nın `.equals(...)` karşılaştırması da sayılır
pub(crate) fn sahiplik(kosul: &str, durum: Option<u16>) -> bool {
    crate::dna::sahiplik_mi(&kosul.replace(".equals(", " == ("), durum)
}

pub(crate) fn tetik_ekle(d: &mut GovdeDna, tur: &str, ad: String, satir: usize) {
    if !ad.is_empty() && d.tetikler.len() < EN_FAZLA && !d.tetikler.iter().any(|(t, a, _)| t == tur && *a == ad) {
        d.tetikler.push((tur.to_string(), ad, satir));
    }
}

/// Yöntem gövdesini bir kez dolaşır: yerel değişken tipleri, çağrılar ve davranış özeti (fırlatmalar, iki yollu
/// ifadeler, yumuşak silme, olay ve mesaj gönderimleri). Yerel ve anonim sınıf gövdelerine girilmez.
fn govde_oku(govde: Node, k: &[u8], yereller: &mut HashMap<String, String>, cagrilar: &mut Vec<Cagri>) -> GovdeDna {
    let mut d = GovdeDna::default();
    let mut yigin = vec![govde];
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "class_body" | "class_declaration" => continue,
            "local_variable_declaration" => {
                let tip = n.child_by_field_name("type").map(|t| metin(t, k)).unwrap_or_default();
                for dk in cocuklar(n).into_iter().filter(|x| x.kind() == "variable_declarator") {
                    let Some(ad) = dk.child_by_field_name("name").map(|a| metin(a, k)) else { continue };
                    // `var x = new Foo(...)`: tip ilk değerden
                    let t = match (tip.as_str(), dk.child_by_field_name("value")) {
                        ("var", Some(v)) if v.kind() == "object_creation_expression" => olusturulan(v, k),
                        _ => tip.clone(),
                    };
                    yereller.insert(ad, t);
                }
            }
            "enhanced_for_statement" => {
                if let (Some(t), Some(a)) = (n.child_by_field_name("type"), n.child_by_field_name("name")) {
                    yereller.insert(metin(a, k), metin(t, k));
                }
            }
            "throw_statement" => {
                if let Some(y) = n.named_child(0).filter(|x| x.kind() == "object_creation_expression") {
                    firlatma_ekle(&mut d, y, n, govde, k, None);
                }
            }
            "object_creation_expression" => {
                // new ResponseEntity<>(govde, HttpStatus.NOT_FOUND)
                if olusturulan(n, k) == "ResponseEntity" {
                    let kod = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default().into_iter().find_map(|a| crate::dna::kod_ifadesi(&metin(a, k)));
                    if let Some(kod) = kod.filter(|c| *c >= 400 && d.firlatmalar.len() < EN_FAZLA) {
                        let kosul = crate::dna::kosul_bul(n, govde, k, false);
                        let sahip = kosul.as_deref().is_some_and(|c| sahiplik(c, Some(kod)));
                        d.firlatmalar.push(Firlatma { durum: Some(kod), istisna: "ResponseEntity".into(), kosul, satir: satir(n), sahiplik: sahip });
                    }
                }
            }
            "method_invocation" => {
                let ad = n.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
                let nesne: String = n.child_by_field_name("object").map(|o| metin(o, k)).unwrap_or_default().chars().filter(|c| !c.is_whitespace()).collect();
                let args: Vec<Node> =
                    n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default().into_iter().filter(|a| !matches!(a.kind(), "line_comment" | "block_comment")).collect();
                let arg_metni: Vec<String> = args.iter().take(8).map(|a| metin(*a, k).chars().take(300).collect()).collect();
                if cagrilar.len() < 256 {
                    cagrilar.push(Cagri { nesne: nesne.clone(), ad: ad.clone(), argumanlar: arg_metni.clone(), satir: satir(n), tip_argumanlari: vec![] });
                }
                let tam = if nesne.is_empty() { ad.clone() } else { format!("{nesne}.{ad}") };
                if !tam.contains('(') && d.cagrilar.len() < 64 && !d.cagrilar.contains(&tam) {
                    d.cagrilar.push(tam.clone());
                }
                // Hata yanıtları: ResponseEntity.notFound() / status(HttpStatus.FORBIDDEN), response.sendError(403)
                let kod = match (nesne.as_str(), ad.as_str()) {
                    ("ResponseEntity", "notFound") => Some(404),
                    ("ResponseEntity", "badRequest") => Some(400),
                    ("ResponseEntity", "unprocessableEntity") => Some(422),
                    ("ResponseEntity", "internalServerError") => Some(500),
                    ("ResponseEntity", "status") => arg_metni.first().and_then(|a| crate::dna::kod_ifadesi(a)),
                    (_, "sendError" | "setStatus") if !nesne.is_empty() => arg_metni.first().and_then(|a| crate::dna::kod_ifadesi(a)),
                    _ => None,
                };
                if let Some(kod) = kod.filter(|c| *c >= 400 && d.firlatmalar.len() < EN_FAZLA) {
                    let kosul = crate::dna::kosul_bul(n, govde, k, false);
                    let sahip = kosul.as_deref().is_some_and(|c| sahiplik(c, Some(kod)));
                    d.firlatmalar.push(Firlatma { durum: Some(kod), istisna: tam.clone(), kosul, satir: satir(n), sahiplik: sahip });
                }
                // `bul(...).orElseThrow(() -> new KayitYok(...))` / `orElseThrow(KayitYok::new)`: kayıt yoksa fırlatma
                if ad == "orElseThrow" {
                    // Koşul alıcının sorgu kısmı: `.map(x -> ...)` gibi dönüşümler atılır
                    let alici = tek_satir(&n.child_by_field_name("object").map(|o| metin(o, k)).unwrap_or_default());
                    let son = [".map(", ".filter(", ".flatMap("].iter().filter_map(|x| alici.find(x)).min().unwrap_or(alici.len());
                    let kosul = Some(format!("{} boş", crate::dna::kisa(&alici[..son])));
                    match args.first() {
                        Some(a) if a.kind() == "lambda_expression" => {
                            if let Some(y) = a.child_by_field_name("body").filter(|b| b.kind() == "object_creation_expression") {
                                firlatma_ekle(&mut d, y, n, govde, k, kosul);
                            }
                        }
                        Some(a) if a.kind() == "method_reference" => {
                            let istisna = temel_ad(metin(*a, k).split("::").next().unwrap_or(""));
                            if d.firlatmalar.len() < EN_FAZLA {
                                let durum = yerlesik_kod(&istisna);
                                d.firlatmalar.push(Firlatma { durum, istisna, kosul, satir: satir(n), sahiplik: false });
                            }
                        }
                        None if d.firlatmalar.len() < EN_FAZLA => {
                            d.firlatmalar.push(Firlatma { durum: None, istisna: "NoSuchElementException".into(), kosul, satir: satir(n), sahiplik: false });
                        }
                        _ => {}
                    }
                }
                if d.yumusak.is_none() && (yumusak_ayarlayici(&ad, arg_metni.first().map(String::as_str).unwrap_or("")) || ["softDelete", "softRemove"].contains(&ad.as_str())) {
                    d.yumusak = Some(satir(n));
                }
                // Olay ve mesaj: publishEvent(new X(...)) / publishEvent(olay), Kafka send("konu"), JMS / Rabbit convertAndSend
                let alici = nesne.to_lowercase();
                match ad.as_str() {
                    "publishEvent" => {
                        let olay = match args.first() {
                            Some(a) if a.kind() == "object_creation_expression" => Some(olusturulan(*a, k)),
                            Some(a) if a.kind() == "identifier" => yereller.get(&metin(*a, k)).map(|t| temel_ad(t)),
                            _ => None,
                        };
                        if let Some(o) = olay {
                            tetik_ekle(&mut d, "event", o, satir(n));
                        }
                    }
                    "send" | "sendDefault" if alici.contains("kafka") => {
                        if let Some(t) = arg_metni.first().and_then(|a| java_metni(a)) {
                            tetik_ekle(&mut d, "event", t, satir(n));
                        }
                    }
                    "convertAndSend" | "send" if alici.contains("jms") || alici.contains("rabbit") || alici.contains("amqp") => {
                        // Rabbit: (değişim, yönlendirme anahtarı, mesaj) ise anahtar; JMS ve iki argümanda hedef
                        let i = if arg_metni.len() >= 3 && !alici.contains("jms") { 1 } else { 0 };
                        if let Some(t) = arg_metni.get(i).and_then(|a| java_metni(a)) {
                            tetik_ekle(&mut d, "event", t, satir(n));
                        }
                    }
                    _ => {}
                }
            }
            "ternary_expression" if d.dallar.len() < EN_FAZLA => {
                if let (Some(c), Some(e), Some(h)) = (n.child_by_field_name("condition"), n.child_by_field_name("consequence"), n.child_by_field_name("alternative")) {
                    let kosul = metin(c, k);
                    if basit_mi(e) && basit_mi(h) && kosul.len() <= 60 && !kosul.contains('\n') {
                        d.dallar.push(Dal {
                            kosul: crate::dna::kisa(&kosul),
                            evet: crate::dna::kisa(&metin(e, k)),
                            hayir: crate::dna::kisa(&metin(h, k)),
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

// ---- Tip yardımcıları ----

/// Tip metnindeki anotasyonlar atılır: "List<@Valid Foo>" -> "List<Foo>"
pub(crate) fn anotasyonsuz(tip: &str) -> String {
    let mut out = String::new();
    let b: Vec<char> = tip.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '@' {
            i += 1;
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_' || b[i] == '.') {
                i += 1;
            }
            if i < b.len() && b[i] == '(' {
                let mut d = 0;
                while i < b.len() {
                    match b[i] {
                        '(' => d += 1,
                        ')' => {
                            d -= 1;
                            if d == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    tek_satir(&out)
}

/// Tipin parca adı: "java.util.List<Foo>" -> "List", "Foo[]" -> "Foo"
pub(crate) fn temel_ad(tip: &str) -> String {
    let t = anotasyonsuz(tip);
    let t = t.split('<').next().unwrap_or(&t).trim().trim_end_matches("...").trim_end_matches("[]").trim();
    t.rsplit('.').next().unwrap_or(t).trim().to_string()
}

/// Üst düzey virgüllerle bölünmüş parçalar (parantez, köşeli, süslü ve açılı parantez içi ve metinler bölünmez)
pub(crate) fn ust_duzey_bol(s: &str) -> Vec<String> {
    let (mut v, mut d, mut bas) = (vec![], 0i32, 0);
    let mut tirnak: Option<char> = None;
    let mut onceki = ' ';
    for (i, c) in s.char_indices() {
        match (tirnak, c) {
            (Some(t), x) if x == t && onceki != '\\' => tirnak = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => tirnak = Some(c),
            (None, '(' | '[' | '{' | '<') => d += 1,
            (None, ')' | ']' | '}' | '>') => d -= 1,
            (None, ',') if d == 0 => {
                v.push(s[bas..i].trim().to_string());
                bas = i + 1;
            }
            _ => {}
        }
        onceki = if onceki == '\\' && c == '\\' { ' ' } else { c };
    }
    if !s[bas..].trim().is_empty() {
        v.push(s[bas..].trim().to_string());
    }
    v
}

/// Generic tip argümanları: "Map<String, List<Foo>>" -> ["String", "List<Foo>"]
fn generic_argumanlar(tip: &str) -> Vec<String> {
    let t = anotasyonsuz(tip);
    let (Some(i), Some(j)) = (t.find('<'), t.rfind('>')) else { return vec![] };
    if j <= i {
        return vec![];
    }
    ust_duzey_bol(&t[i + 1..j])
}

/// Sarmalayıcı tiplerin içi: Optional / ResponseEntity / Mono ... -> (iç tip, dizi mi)
pub(crate) fn sarmalayicisiz(tip: &str) -> (String, bool) {
    let mut t = anotasyonsuz(tip);
    let mut dizi = false;
    for _ in 0..4 {
        let temel = temel_ad(&t);
        let ic = generic_argumanlar(&t).into_iter().next();
        match (temel.as_str(), ic) {
            ("Optional" | "ResponseEntity" | "HttpEntity" | "Mono" | "CompletableFuture" | "CompletionStage" | "Callable" | "DeferredResult" | "WebAsyncTask", Some(i)) => t = i,
            ("Flux" | "Stream", Some(i)) => {
                dizi = true;
                t = i;
            }
            (k, Some(i)) if KOLEKSIYONLAR.contains(&k) => {
                dizi = true;
                t = i;
            }
            _ => break,
        }
    }
    if let Some(e) = t.strip_suffix("[]") {
        return (e.trim().to_string(), true);
    }
    (t, dizi)
}

/// `@ResponseStatus` kodu: hata kodları ve başarı adları (`HttpStatus.CREATED`)
fn basari_kodu(s: &str) -> Option<u16> {
    crate::dna::kod_ifadesi(s).or_else(|| {
        Some(match s.trim().rsplit('.').next()? {
            "OK" => 200,
            "CREATED" => 201,
            "ACCEPTED" => 202,
            "NO_CONTENT" => 204,
            "RESET_CONTENT" => 205,
            "PARTIAL_CONTENT" => 206,
            _ => return None,
        })
    })
}

/// Basit (tek değerli) Java tipi mi
fn basit_tip(temel: &str) -> bool {
    matches!(
        temel,
        "String"
            | "CharSequence"
            | "char"
            | "Character"
            | "int"
            | "Integer"
            | "long"
            | "Long"
            | "short"
            | "Short"
            | "byte"
            | "Byte"
            | "BigInteger"
            | "double"
            | "Double"
            | "float"
            | "Float"
            | "BigDecimal"
            | "Number"
            | "boolean"
            | "Boolean"
            | "UUID"
            | "LocalDate"
            | "LocalDateTime"
            | "OffsetDateTime"
            | "ZonedDateTime"
            | "Instant"
            | "Date"
            | "LocalTime"
            | "YearMonth"
    )
}

/// İlkel tipler: gövdede yoksa varsayılan değeri alır, hiçbir zaman "eksik" değildir
fn ilkel(temel: &str) -> bool {
    matches!(temel, "int" | "long" | "short" | "byte" | "double" | "float" | "boolean" | "char")
}

/// Java adını Spring Boot'un varsayılan adlandırmasıyla tablo / sütun adına çevirir: SiparisKalemi -> siparis_kalemi
fn yilan(ad: &str) -> String {
    let mut s = String::new();
    let c: Vec<char> = ad.chars().collect();
    for (i, ch) in c.iter().enumerate() {
        if ch.is_uppercase() && i > 0 && (c[i - 1].is_lowercase() || c[i - 1].is_ascii_digit() || c.get(i + 1).is_some_and(|n| n.is_lowercase()) && c[i - 1].is_uppercase()) {
            s.push('_');
        }
        s.extend(ch.to_lowercase());
    }
    s
}

/// Yolu Spring'in birleştirdiği gibi birleştirir: "/api" + "kullanicilar" -> "/api/kullanicilar"
pub(crate) fn yol_birlestir(a: &str, b: &str) -> String {
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() {
        return b.to_string();
    }
    if b.is_empty() {
        return a.to_string();
    }
    format!("{}/{}", a.trim_end_matches('/'), b.trim_start_matches('/'))
}

/// Yolun parçaları (`{ad:regex}` içindeki `/` bölmez)
fn yol_parcalari(y: &str) -> Vec<String> {
    let (mut v, mut d, mut cur) = (vec![], 0i32, String::new());
    for c in y.chars() {
        match c {
            '{' => {
                d += 1;
                cur.push(c);
            }
            '}' => {
                d -= 1;
                cur.push(c);
            }
            '/' if d == 0 => v.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    v.push(cur);
    v
}

/// Tam yol, başında `/` ve çift `/` olmadan
pub(crate) fn duzgun(y: &str) -> String {
    let p = yol_parcalari(y);
    let mut s = String::new();
    let n = p.len();
    for (i, x) in p.iter().enumerate() {
        if x.is_empty() && !(i == n - 1 && i > 0) {
            continue;
        }
        s.push('/');
        s.push_str(x);
    }
    // Sondaki eğik çizgi korunur (Spring 6 "/a/" ile "/a"yı ayırır)
    if s.is_empty() {
        "/".into()
    } else {
        s
    }
}

/// Sezgi yolu: `{id}` / `{id:\d+}` / `{*yol}` -> `:id`
pub(crate) fn sezgi_yolu(y: &str) -> String {
    yol_parcalari(y)
        .iter()
        .map(|p| match p.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            Some(ic) => format!(":{}", ic.trim_start_matches('*').split(':').next().unwrap_or(ic).trim()),
            None => p.clone(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Ant / Spring yol deseni bir uç yoluyla eşleşir mi. Uçtaki değişken parça (`:id`) yalnızca joker ya da değişken
/// desen parçasıyla eşleşir
fn desen_eslesir(desen: &str, yol: &str) -> bool {
    let d: Vec<String> = yol_parcalari(desen).into_iter().filter(|x| !x.is_empty()).collect();
    let y: Vec<&str> = yol.split('/').filter(|x| !x.is_empty()).collect();
    fn parca(p: &str, s: &str) -> bool {
        if p.starts_with('{') {
            return true;
        }
        if s.starts_with(':') {
            return !p.is_empty() && p.chars().all(|c| c == '*');
        }
        if !p.contains(['*', '?']) {
            return p == s;
        }
        let mut r = String::from("^");
        for c in p.chars() {
            match c {
                '*' => r.push_str("[^/]*"),
                '?' => r.push_str("[^/]"),
                c => r.push_str(&regex::escape(&c.to_string())),
            }
        }
        r.push('$');
        regex::Regex::new(&r).is_ok_and(|re| re.is_match(s))
    }
    fn es(d: &[String], y: &[&str], adim: &mut usize) -> bool {
        *adim += 1;
        if *adim > 10_000 {
            return false;
        }
        match d.first().map(String::as_str) {
            None => y.is_empty(),
            Some("**") => (0..=y.len()).any(|i| es(&d[1..], &y[i..], adim)),
            Some(p) if p.starts_with("{*") => true,
            Some(p) => !y.is_empty() && parca(p, y[0]) && es(&d[1..], &y[1..], adim),
        }
    }
    es(&d, &y, &mut 0)
}

// ---- Yapılandırma ----

/// application.properties / application.yml anahtarı (varsayılan profil: YAML'ın ilk belgesi). Anahtarlar Spring'in
/// gevşek bağlamasıyla karşılaştırılır (`context-path` = `contextPath`). Yer tutucunun varsayılanı alınır
/// (`${YOL:/api}` -> "/api"); varsayılanı olmayan yer tutucu bilinmez
fn ayar(dosyalar: &[(String, String)], anahtar: &str) -> Option<String> {
    let norm = |s: &str| s.chars().filter(|c| *c != '-' && *c != '_').collect::<String>().to_lowercase();
    let hedef = norm(anahtar);
    let deger = |v: &str| -> Option<String> {
        let v = v.trim();
        let v = v.split(" #").next().unwrap_or(v).trim();
        let v = v.trim_matches(['"', '\'']).to_string();
        if let Some(ic) = v.strip_prefix("${").and_then(|x| x.strip_suffix('}')) {
            return ic.split_once(':').map(|(_, d)| d.to_string());
        }
        Some(v)
    };
    for (ad, m) in dosyalar {
        if ad.ends_with(".properties") {
            for l in m.lines() {
                let l = l.trim();
                if l.is_empty() || l.starts_with('#') || l.starts_with('!') {
                    continue;
                }
                let Some(i) = l.find(['=', ':']) else { continue };
                if norm(l[..i].trim()) == hedef {
                    return deger(&l[i + 1..]);
                }
            }
        } else {
            let mut yigin: Vec<(usize, String)> = vec![];
            for l in m.lines() {
                if l.trim() == "---" {
                    break;
                }
                let t = l.trim_end();
                if t.trim().is_empty() || t.trim_start().starts_with('#') || t.trim_start().starts_with('-') {
                    continue;
                }
                let girinti = t.len() - t.trim_start().len();
                let Some((k, v)) = t.trim().split_once(':') else { continue };
                while yigin.last().is_some_and(|(g, _)| *g >= girinti) {
                    yigin.pop();
                }
                let tam = yigin.iter().map(|(_, x)| x.as_str()).chain(std::iter::once(k.trim())).collect::<Vec<_>>().join(".");
                if v.trim().is_empty() {
                    yigin.push((girinti, k.trim().to_string()));
                } else if norm(&tam) == hedef {
                    return deger(v);
                }
            }
        }
    }
    None
}

/// Uygulama Kotlin ile mi yazılmış: yapı dosyasında Kotlin eklentisi (Gradle `kotlin("jvm")`,
/// `org.jetbrains.kotlin`, Maven `kotlin-maven-plugin`)
pub fn kotlin_mi(kok: &Path) -> bool {
    ["pom.xml", "build.gradle", "build.gradle.kts"]
        .iter()
        .filter_map(|d| guvenli::oku(&kok.join(d)))
        .any(|m| m.contains("org.jetbrains.kotlin") || m.contains("kotlin(\"jvm\")") || m.contains("kotlin-maven-plugin") || m.contains("kotlin(\"plugin.spring\")"))
}

/// Spring Boot uygulaması mı: Maven ya da Gradle yapı dosyası Spring Boot'u kullanıyor
pub fn spring_mi(kok: &Path) -> bool {
    ["pom.xml", "build.gradle", "build.gradle.kts"].iter().filter_map(|d| guvenli::oku(&kok.join(d))).any(|m| m.contains("spring-boot") || m.contains("org.springframework"))
}

/// Uygulamanın adı ve sürümü: pom.xml (`<parent>` dışındaki ilk artifactId / version) ya da Gradle
/// (`rootProject.name`, `version = '...'`)
pub fn paket(kok: &Path) -> Value {
    static D: std::sync::OnceLock<[regex::Regex; 5]> = std::sync::OnceLock::new();
    let [ust, artifact, surum, gradle_ad, gradle_surum] = D.get_or_init(|| {
        [
            regex::Regex::new(r"(?s)<parent>.*?</parent>").unwrap(),
            regex::Regex::new(r"<artifactId>\s*([^<\s]+)\s*</artifactId>").unwrap(),
            regex::Regex::new(r"<version>\s*([^<\s]+)\s*</version>").unwrap(),
            regex::Regex::new(r#"rootProject\.name\s*=\s*['"]([^'"]+)['"]"#).unwrap(),
            regex::Regex::new(r#"(?m)^\s*version\s*=\s*['"]([^'"]+)['"]"#).unwrap(),
        ]
    });
    let bilinen = |v: &str| (!v.starts_with("${")).then(|| v.to_string());
    if let Some(pom) = guvenli::oku(&kok.join("pom.xml")) {
        let govde = ust.replace(&pom, "");
        let son = ["<dependencies>", "<dependencyManagement>", "<build>", "<profiles>", "<modules>"].iter().filter_map(|x| govde.find(x)).min().unwrap_or(govde.len());
        let bas = &govde[..son];
        let ad = artifact.captures(bas).map(|c| c[1].to_string());
        let s = surum.captures(bas).and_then(|c| bilinen(&c[1]));
        return json!({ "name": ad, "version": s });
    }
    let ayar = ["settings.gradle", "settings.gradle.kts"].iter().find_map(|d| guvenli::oku(&kok.join(d))).unwrap_or_default();
    let yapi = ["build.gradle", "build.gradle.kts"].iter().find_map(|d| guvenli::oku(&kok.join(d))).unwrap_or_default();
    json!({
        "name": gradle_ad.captures(&ayar).map(|c| c[1].to_string()),
        "version": gradle_surum.captures(&yapi).and_then(|c| bilinen(&c[1])),
    })
}

// ---- Bağlam: ad çözümü ----

/// (dosya, sınıf)
pub(crate) type SRef = (usize, usize);
/// (dosya, sınıf, yöntem)
pub(crate) type YRef = (usize, usize, usize);

/// JSON gövdesini nesneye çeviren kitaplık: Kotlin alanlarının zorunluluğu buna göre değişir
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Serilestirici {
    /// Jackson (Spring Boot; Kotlin'de jackson-module-kotlin varsa boş olamayan ve varsayılansız alan zorunlu)
    Jackson { kotlin_modulu: bool },
    /// kotlinx.serialization: varsayılansız alan zorunlu; `explicitNulls = false` ise boş olabilen alan isteğe bağlı
    Kotlinx { acik_bos: bool },
    /// Gson ya da bilinmeyen: alanların zorunluluğu koddan okunmaz
    Bilinmeyen,
}

pub(crate) struct Baglam<'a> {
    kok: &'a Path,
    dosyalar: &'a [Dosya],
    adlar: HashMap<&'a str, Vec<SRef>>,
    /// Sınıf -> onu uygulayan / genişleten proje sınıfları (geçişli)
    uygulayanlar: HashMap<SRef, Vec<SRef>>,
    /// Depo arayüzü -> yönettiği varlık
    depolar: HashMap<SRef, String>,
    /// Özel durum adı -> HTTP kodu (`@ResponseStatus`, `@ExceptionHandler`; Ktor'da StatusPages)
    pub(crate) istisnalar: HashMap<String, u16>,
    /// Jackson adlandırması yılan biçiminde mi (spring.jackson.property-naming-strategy)
    yilan_adlar: bool,
    /// Gövde kök adla sarılı mı (spring.jackson.deserialization.UNWRAP_ROOT_VALUE): `{"user": {...}}`
    kok_sarma: bool,
    pub(crate) serilestirici: Serilestirici,
    /// Kotlin 2.2 `-Xannotation-default-target=param-property`: hedefsiz kısıt alana da yazılır
    pub(crate) kt_alan_hedefi: bool,
}

impl<'a> Baglam<'a> {
    pub(crate) fn yeni(kok: &'a Path, dosyalar: &'a [Dosya], yilan_adlar: bool, kok_sarma: bool) -> Self {
        let mut adlar: HashMap<&str, Vec<SRef>> = HashMap::new();
        for (di, d) in dosyalar.iter().enumerate() {
            for (si, s) in d.siniflar.iter().enumerate() {
                adlar.entry(s.ad.as_str()).or_default().push((di, si));
            }
        }
        let mut b = Baglam {
            kok,
            dosyalar,
            adlar,
            uygulayanlar: HashMap::new(),
            depolar: HashMap::new(),
            istisnalar: HashMap::new(),
            yilan_adlar,
            kok_sarma,
            serilestirici: Serilestirici::Jackson { kotlin_modulu: true },
            kt_alan_hedefi: false,
        };
        let mut uygulayanlar: HashMap<SRef, Vec<SRef>> = HashMap::new();
        let mut depolar = HashMap::new();
        for (di, d) in dosyalar.iter().enumerate() {
            for si in 0..d.siniflar.len() {
                for u in b.ustler((di, si)) {
                    uygulayanlar.entry(u).or_default().push((di, si));
                }
                if let Some(e) = b.depo_varligi((di, si), &[], 0) {
                    depolar.insert((di, si), e);
                }
            }
        }
        b.uygulayanlar = uygulayanlar;
        b.depolar = depolar;
        b.istisnalar = b.istisna_haritasi();
        b
    }

    pub(crate) fn sinif(&self, r: SRef) -> &'a Sinif {
        &self.dosyalar[r.0].siniflar[r.1]
    }

    pub(crate) fn yontem(&self, r: YRef) -> &'a Yontem {
        &self.dosyalar[r.0].siniflar[r.1].yontemler[r.2]
    }

    pub(crate) fn goreli(&self, di: usize) -> String {
        guvenli::goreli(self.kok, &self.dosyalar[di].yol)
    }

    /// Tip adından projedeki sınıf. Aynı ad birden çok yerdeyse: aynı dosya, içe aktarılan, aynı paket
    pub(crate) fn bul(&self, tip: &str, di: usize) -> Option<SRef> {
        let ad = temel_ad(tip);
        let adaylar = self.adlar.get(ad.as_str())?;
        if adaylar.len() == 1 {
            return Some(adaylar[0]);
        }
        let d = &self.dosyalar[di];
        adaylar
            .iter()
            .find(|(x, _)| *x == di)
            .or_else(|| {
                adaylar.iter().find(|(x, _)| {
                    let p = &self.dosyalar[*x].paket;
                    d.iceaktarmalar.iter().any(|i| *i == format!("{p}.{ad}") || *i == format!("{p}.*"))
                })
            })
            .or_else(|| adaylar.iter().find(|(x, _)| self.dosyalar[*x].paket == d.paket))
            .or(adaylar.first())
            .copied()
    }

    /// Üst sınıf zinciri ve bütün arayüzler (geçişli; kendisi hariç)
    fn ustler(&self, s: SRef) -> Vec<SRef> {
        let mut v: Vec<SRef> = vec![];
        let mut yigin = vec![s];
        while let Some(x) = yigin.pop() {
            if v.len() > 32 {
                break;
            }
            let sn = self.sinif(x);
            for t in sn.ust.iter().chain(sn.arayuzler.iter()) {
                if let Some(u) = self.bul(t, x.0).filter(|u| *u != s && !v.contains(u)) {
                    v.push(u);
                    yigin.push(u);
                }
            }
        }
        v
    }

    /// Yalnızca üst sınıf zinciri (alan arama için)
    fn ust_siniflar(&self, s: SRef) -> Vec<SRef> {
        let mut v = vec![];
        let mut x = s;
        while let Some(u) = self.sinif(x).ust.as_deref().and_then(|t| self.bul(t, x.0)) {
            if u == s || v.contains(&u) || v.len() > 16 {
                break;
            }
            v.push(u);
            x = u;
        }
        v
    }

    /// Spring Data depo arayüzünün yönettiği varlık: tabana kadar tip argümanları yerine konarak
    /// (`interface TemelDepo<T> extends JpaRepository<T, Long>`, `UrunDepo extends TemelDepo<Urun>`)
    fn depo_varligi(&self, s: SRef, argumanlar: &[String], derinlik: usize) -> Option<String> {
        let sn = self.sinif(s);
        if sn.tur != "interface" || derinlik > 6 {
            return None;
        }
        for ust in &sn.arayuzler {
            let g: Vec<String> = generic_argumanlar(ust)
                .into_iter()
                .map(|x| sn.tip_parametreleri.iter().position(|p| *p == x.trim()).and_then(|i| argumanlar.get(i).cloned()).unwrap_or(x))
                .collect();
            let temel = temel_ad(ust);
            match self.bul(ust, s.0) {
                Some(u) => {
                    if let Some(e) = self.depo_varligi(u, &g, derinlik + 1) {
                        return Some(e);
                    }
                }
                None if DEPO_TABANLARI.contains(&temel.as_str()) => return g.first().map(|e| temel_ad(e)).filter(|e| !e.is_empty()),
                None => {}
            }
        }
        None
    }

    // ---- Sabitler ----

    /// Sabit metin ya da metin dizisi: "...", SABIT, Sinif.SABIT, "a" + SABIT, {"a", SABIT}, new String[]{...}
    fn metinler(&self, di: usize, s: Option<SRef>, ifade: &str, derinlik: usize) -> Option<Vec<String>> {
        let e = ifade.trim();
        let e = e.strip_prefix("new String[]").map(str::trim).unwrap_or(e);
        if let Some(ic) = e.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            let mut v = vec![];
            for p in ust_duzey_bol(ic) {
                v.extend(self.metinler(di, s, &p, derinlik + 1)?);
            }
            return Some(v);
        }
        // Dizi sabitine atıf: alanın ilk değeri bir dizi olabilir
        if derinlik < 6 && !e.contains(['"', '+']) {
            if let Some((adi, asi, alan)) = self.sabit_bul(di, s, e) {
                let deger = alan.deger.clone()?;
                if deger.trim_start().starts_with('{') || deger.trim_start().starts_with("new String[]") {
                    return self.metinler(adi, Some((adi, asi)), &deger, derinlik + 1);
                }
            }
        }
        self.metin_degeri(di, s, e, derinlik).map(|m| vec![m])
    }

    pub(crate) fn metin_degeri(&self, di: usize, s: Option<SRef>, ifade: &str, derinlik: usize) -> Option<String> {
        let e = ifade.trim();
        if let Some(m) = java_metni(e) {
            return Some(m);
        }
        if derinlik > 6 {
            return None;
        }
        let parcalar = ust_duzey_arti(e);
        if parcalar.len() > 1 {
            return parcalar.iter().map(|p| self.metin_degeri(di, s, p, derinlik + 1)).collect::<Option<Vec<_>>>().map(|v| v.concat());
        }
        let (adi, asi, alan) = self.sabit_bul(di, s, e)?;
        self.metin_degeri(adi, Some((adi, asi)), alan.deger.as_deref()?, derinlik + 1)
    }

    /// Statik sabitin tanımı: `Sinif.AD`, kendi sınıfı ve üstleri, aynı dosya, ya da projede tek tanımlı statik ad
    /// (statik içe aktarma)
    fn sabit_bul(&self, di: usize, s: Option<SRef>, e: &str) -> Option<(usize, usize, &'a Degisken)> {
        if !e.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') || e.is_empty() {
            return None;
        }
        let statik = |r: SRef, ad: &str| self.sinif(r).alanlar.iter().find(|a| a.ad == ad && a.statik).map(|a| (r.0, r.1, a));
        if let Some((sinif, ad)) = e.rsplit_once('.') {
            let r = self.bul(sinif, di)?;
            return std::iter::once(r).chain(self.ustler(r)).find_map(|u| statik(u, ad));
        }
        if let Some(r) = s {
            if let Some(x) = std::iter::once(r).chain(self.ustler(r)).find_map(|u| statik(u, e)) {
                return Some(x);
            }
        }
        if let Some(x) = (0..self.dosyalar[di].siniflar.len()).find_map(|si| statik((di, si), e)) {
            return Some(x);
        }
        let mut bulunan = self.dosyalar.iter().enumerate().flat_map(|(i, d)| (0..d.siniflar.len()).map(move |si| (i, si))).filter_map(|r| statik(r, e));
        let ilk = bulunan.next()?;
        bulunan.next().is_none().then_some(ilk)
    }

    // ---- Özel durumlar ----

    /// Özel durum -> HTTP kodu: sınıftaki `@ResponseStatus` ve `@ExceptionHandler` yöntemleri (yöntemin
    /// `@ResponseStatus`'u, yoksa gövdesindeki `ResponseEntity.status(...)` ya da `HttpStatus.X`). Spring'de eşleyici
    /// sınıftaki anotasyondan önce gelir
    fn istisna_haritasi(&self) -> HashMap<String, u16> {
        static HS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let hs = HS.get_or_init(|| regex::Regex::new(r"HttpStatus\.([A-Z_]+)").unwrap());
        let durum = |a: &Anotasyon| a.deger(&["value", "code"]).and_then(crate::dna::kod_ifadesi);
        let mut m = HashMap::new();
        for d in self.dosyalar {
            for s in &d.siniflar {
                if let Some(k) = anotasyon(&s.anotasyonlar, "ResponseStatus").and_then(durum) {
                    m.insert(s.ad.clone(), k);
                }
            }
        }
        for d in self.dosyalar {
            for s in &d.siniflar {
                for y in &s.yontemler {
                    let Some(e) = anotasyon(&y.anotasyonlar, "ExceptionHandler") else { continue };
                    let kod = anotasyon(&y.anotasyonlar, "ResponseStatus")
                        .and_then(durum)
                        .or_else(|| y.dna.firlatmalar.iter().find_map(|f| f.durum))
                        .or_else(|| hs.captures(&y.govde_metni).and_then(|c| crate::dna::kod_ifadesi(&c[1])));
                    let Some(kod) = kod else { continue };
                    let mut adlar: Vec<String> =
                        e.deger(&["value"]).map(|v| v.trim_matches(['{', '}']).split(',').map(|x| temel_ad(x.trim().trim_end_matches(".class"))).collect()).unwrap_or_default();
                    if adlar.is_empty() {
                        adlar.extend(y.parametreler.first().map(|p| temel_ad(&p.tip)));
                    }
                    for a in adlar.into_iter().filter(|a| !a.is_empty()) {
                        m.insert(a, kod);
                    }
                }
            }
        }
        m
    }

    /// Fırlatılan özel durumun kodu: eşleme, yoksa projedeki üst sınıflarınınki, yoksa yerleşik
    pub(crate) fn istisna_kodu(&self, ad: &str, di: usize) -> Option<u16> {
        if let Some(k) = self.istisnalar.get(ad) {
            return Some(*k);
        }
        if let Some(r) = self.bul(ad, di) {
            for u in self.ust_siniflar(r) {
                if let Some(k) = self.istisnalar.get(&self.sinif(u).ad) {
                    return Some(*k);
                }
            }
            if let Some(k) = self.ust_siniflar(r).last().copied().and_then(|u| self.sinif(u).ust.as_deref()).and_then(|t| yerlesik_kod(&temel_ad(t))) {
                return Some(k);
            }
        }
        yerlesik_kod(ad)
    }

    // ---- Çağrı çözümü ----

    /// Çağrının alıcısının tipi: yerel değişken ya da parametre, sınıfın (ve üst sınıflarının) alanı, `this`, statik
    /// sınıf adı
    pub(crate) fn alici_tipi(&self, s: SRef, y: &Yontem, nesne: &str) -> Option<String> {
        let n = nesne.trim().trim_start_matches("this.");
        if n.is_empty() || n == "this" {
            return Some(self.sinif(s).ad.clone());
        }
        if n == "super" {
            return self.sinif(s).ust.clone();
        }
        if !n.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        if let Some(t) = y.yereller.get(n) {
            return Some(t.clone());
        }
        for u in std::iter::once(s).chain(self.ust_siniflar(s)) {
            if let Some(a) = self.sinif(u).alanlar.iter().find(|a| a.ad == n) {
                return Some(a.tip.clone());
            }
        }
        n.starts_with(char::is_uppercase).then(|| n.to_string())
    }

    /// Çağrının projedeki hedef yöntemleri: alıcının sınıfı (ve üst sınıfları); arayüz ya da soyut sınıfsa uygulayanlar
    pub(crate) fn hedefler(&self, s: SRef, y: &Yontem, c: &Cagri) -> Vec<YRef> {
        let Some(tip) = self.alici_tipi(s, y, &c.nesne) else { return vec![] };
        let Some(t) = self.bul(&tip, s.0) else { return vec![] };
        if self.depolar.contains_key(&t) {
            return vec![];
        }
        let mut siniflar = vec![t];
        siniflar.extend(self.ust_siniflar(t));
        if self.sinif(t).soyut {
            siniflar.extend(self.uygulayanlar.get(&t).cloned().unwrap_or_default());
        }
        let mut v = vec![];
        for r in siniflar {
            for (yi, m) in self.sinif(r).yontemler.iter().enumerate() {
                // Kotlin: varsayılan değerli parametreler verilmeyebilir
                let zorunlu_sayi = m.parametreler.iter().filter(|p| p.deger.is_none()).count();
                let sayi_uyar = (zorunlu_sayi..=m.parametreler.len()).contains(&c.argumanlar.len());
                if m.ad == c.ad && m.govdeli && (sayi_uyar || m.parametreler.last().is_some_and(|p| p.tip.ends_with("[]"))) {
                    v.push((r.0, r.1, yi));
                }
            }
            if v.len() >= 4 {
                break;
            }
        }
        v
    }

    /// Depo çağrısının etkisi: (yazma, silme). `@Modifying` / `@Query` metni, yoksa Spring Data'nın ad kuralı
    fn depo_islemi(&self, depo: SRef, ad: &str) -> (bool, bool) {
        let tanim = std::iter::once(depo).chain(self.ustler(depo)).find_map(|r| self.sinif(r).yontemler.iter().find(|m| m.ad == ad));
        if let Some(m) = tanim {
            let sorgu = anotasyon(&m.anotasyonlar, "Query").and_then(|q| q.deger(&["value"])).and_then(java_metni).unwrap_or_default().trim().to_lowercase();
            if var_mi(&m.anotasyonlar, "Modifying") || ["update", "delete", "insert"].iter().any(|x| sorgu.starts_with(x)) {
                return (true, sorgu.starts_with("delete"));
            }
            if !sorgu.is_empty() {
                return (false, false);
            }
        }
        let silme = ad.starts_with("delete") || ad.starts_with("remove");
        (silme || ad.starts_with("save") || ad.starts_with("insert") || ad == "persist" || ad == "merge", silme)
    }

    /// Exposed çağrısının etkisi: (tablo modeli, yazma mı, silme mi). Alıcı bir tablo nesnesi (`Gorevler.insert { }`,
    /// `selectAll()`, `update`, `deleteWhere`) ya da DAO varlık sınıfı (`Gorev.new { }`, `findById`, `all()`); başka
    /// çağrılar (sütun erişimi, birleştirme) etki değildir
    fn exposed_islemi(&self, t: SRef, ad: &str) -> Option<(String, bool, bool)> {
        let sn = self.sinif(t);
        if var_mi(&sn.anotasyonlar, "ExposedTable") {
            let silme = matches!(ad, "deleteWhere" | "deleteAll" | "deleteIgnoreWhere" | "deleteReturning");
            let yazma = silme
                || matches!(
                    ad,
                    "insert"
                        | "insertAndGetId"
                        | "insertIgnore"
                        | "insertIgnoreAndGetId"
                        | "insertReturning"
                        | "batchInsert"
                        | "batchUpsert"
                        | "upsert"
                        | "upsertReturning"
                        | "replace"
                        | "batchReplace"
                        | "update"
                        | "updateReturning"
                );
            let okuma = matches!(ad, "select" | "selectAll" | "selectBatched" | "selectAllBatched" | "slice");
            return (yazma || okuma).then(|| (sn.ad.clone(), yazma, silme));
        }
        let tablo = anotasyon(&sn.anotasyonlar, "ExposedEntity").and_then(|a| a.deger(&["table"])).and_then(|x| self.bul(x, t.0)).map(|r| self.sinif(r).ad.clone())?;
        match ad {
            "new" => Some((tablo, true, false)),
            "findById" | "find" | "all" | "get" | "findByIdAndUpdate" | "findSingleByAndUpdate" | "count" | "wrapRow" | "wrapRows" => Some((tablo, ad.contains("Update"), false)),
            _ => None,
        }
    }

    /// Yöntemin zenginleştirilmiş davranış özeti: kodu bilinmeyen özel durumlar eşlemeden, depo silmeleri (kaskad için)
    /// ve `@Async` yöntem çağrıları (iş)
    pub(crate) fn zengin_dna(&self, r: YRef) -> GovdeDna {
        let s = (r.0, r.1);
        let y = self.yontem(r);
        let mut d = y.dna.clone();
        for f in d.firlatmalar.iter_mut().filter(|f| f.durum.is_none()) {
            f.durum = self.istisna_kodu(&f.istisna, r.0);
            if let Some(k) = f.kosul.as_deref() {
                f.sahiplik = sahiplik(k, f.durum);
            }
        }
        for c in &y.cagrilar {
            let tip = self.alici_tipi(s, y, &c.nesne).and_then(|t| self.bul(&t, r.0));
            if let Some(e) = tip.and_then(|t| self.depolar.get(&t).map(|e| (t, e))) {
                if self.depo_islemi(e.0, &c.ad).1 && !d.silinen.contains(e.1) {
                    d.silinen.push(e.1.clone());
                }
                continue;
            }
            if let Some((tablo, _, silme)) = tip.and_then(|t| self.exposed_islemi(t, &c.ad)) {
                if silme && !d.silinen.contains(&tablo) {
                    d.silinen.push(tablo);
                }
                continue;
            }
            for h in self.hedefler(s, y, c) {
                let m = self.yontem(h);
                if var_mi(&m.anotasyonlar, "Async") {
                    tetik_ekle(&mut d, "job", format!("{}.{}", self.sinif((h.0, h.1)).ad, m.ad), c.satir);
                }
            }
        }
        d
    }

    /// İşleyiciden başlayıp servis zincirine (3 kat) inerek veri etkisi [Yorum]
    pub(crate) fn veri_bagi(&self, kok: YRef, modeller: &HashSet<String>) -> Option<VeriBag> {
        if modeller.is_empty() {
            return None;
        }
        let mut bag = VeriBag::default();
        let mut gorulen = HashSet::new();
        let mut yigin = vec![(kok, 0usize)];
        while let Some((r, derinlik)) = yigin.pop() {
            if !gorulen.insert(r) || gorulen.len() > 200 {
                continue;
            }
            let s = (r.0, r.1);
            let y = self.yontem(r);
            for c in &y.cagrilar {
                let tip = self.alici_tipi(s, y, &c.nesne);
                let t = tip.as_deref().and_then(|t| self.bul(t, r.0));
                if let Some((depo, e)) = t.and_then(|t| self.depolar.get(&t).map(|e| (t, e))) {
                    let (yazma, _) = self.depo_islemi(depo, &c.ad);
                    if yazma { &mut bag.yazar } else { &mut bag.okur }.push(e.clone());
                    continue;
                }
                if let Some((tablo, yazma, _)) = t.and_then(|t| self.exposed_islemi(t, &c.ad)).filter(|(x, ..)| modeller.contains(x)) {
                    if yazma { &mut bag.yazar } else { &mut bag.okur }.push(tablo);
                    continue;
                }
                match tip.as_deref().map(temel_ad).as_deref() {
                    Some("EntityManager") => {
                        let varlik = |a: &str| -> Option<String> {
                            let a = a.trim();
                            let t = a.strip_suffix(".class").map(String::from).or_else(|| y.yereller.get(a).cloned()).or_else(|| self.alici_tipi(s, y, a))?;
                            Some(temel_ad(&t)).filter(|x| modeller.contains(x))
                        };
                        match c.ad.as_str() {
                            "persist" | "merge" | "remove" => bag.yazar.extend(c.argumanlar.first().and_then(|a| varlik(a))),
                            "find" | "getReference" => bag.okur.extend(c.argumanlar.first().and_then(|a| varlik(a))),
                            "createQuery" | "createNativeQuery" | "createNamedQuery" | "createStoredProcedureQuery" => bag.eksik_olabilir = true,
                            _ => {}
                        }
                        continue;
                    }
                    Some(
                        "JdbcTemplate"
                        | "NamedParameterJdbcTemplate"
                        | "JdbcClient"
                        | "DSLContext"
                        | "MongoTemplate"
                        | "ReactiveMongoTemplate"
                        | "R2dbcEntityTemplate"
                        | "DatabaseClient",
                    ) => {
                        bag.eksik_olabilir = true;
                        continue;
                    }
                    _ => {}
                }
                if derinlik < 3 {
                    for h in self.hedefler(s, y, c) {
                        yigin.push((h, derinlik + 1));
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

    /// İşlemin davranış zinciri: işleyici, sonra gövdede çağrılan proje yöntemleri (2 kat). Dönen: davranış özeti
    pub(crate) fn dna_zinciri(&self, r: YRef, zengin: &HashMap<YRef, GovdeDna>) -> Value {
        let mut halkalar = vec![];
        let mut gorulen = HashSet::new();
        let mut yigin: Vec<(YRef, usize, &'static str)> = vec![(r, 0, "isleyici")];
        while let Some((x, derinlik, tur)) = yigin.pop() {
            if !gorulen.insert(x) || halkalar.len() >= 32 {
                continue;
            }
            let Some(dna) = zengin.get(&x) else { continue };
            halkalar.push(crate::dna::Halka { dosya: self.goreli(x.0), dna, derinlik, tur });
            if derinlik >= 2 {
                continue;
            }
            let y = self.yontem(x);
            let mut sonraki = vec![];
            for c in &y.cagrilar {
                for h in self.hedefler((x.0, x.1), y, c) {
                    sonraki.push((h, derinlik + 1, "servis"));
                }
            }
            yigin.extend(sonraki.into_iter().rev());
        }
        crate::dna::ozet(&halkalar)
    }

    // ---- Girdi şemaları ----

    /// Sınıfın JSON alanları: üst sınıflardakiler önce; statik ve `@JsonIgnore` alanlar hariç; ad `@JsonProperty` ya da
    /// adlandırma stratejisi (kotlinx.serialization'da `@SerialName`, `@Transient` hariç). `dogrulama`: Jakarta kısıtları
    /// bu konumda çalışıyor mu
    pub(crate) fn alanlar(&self, s: SRef, dogrulama: bool, derinlik: usize, out: &mut BTreeMap<String, Alan>) {
        let mut siniflar = self.ust_siniflar(s);
        siniflar.reverse();
        siniflar.push(s);
        let kotlinx = matches!(self.serilestirici, Serilestirici::Kotlinx { .. });
        let ad_anotasyonu = if kotlinx { "SerialName" } else { "JsonProperty" };
        for r in siniflar {
            let sn = self.sinif(r);
            let yilan_ad = self.yilan_adlar || anotasyon(&sn.anotasyonlar, "JsonNaming").is_some_and(|a| a.argumanlar.iter().any(|(_, v)| v.contains("Snake")));
            for a in sn.alanlar.iter().filter(|a| !a.statik && !var_mi(&a.anotasyonlar, "JsonIgnore") && !(kotlinx && var_mi(&a.anotasyonlar, "Transient"))) {
                let ad = anotasyon(&a.anotasyonlar, ad_anotasyonu).and_then(|j| j.deger(&["value"])).and_then(java_metni).filter(|x| !x.is_empty()).unwrap_or_else(|| {
                    if yilan_ad {
                        yilan(&a.ad)
                    } else {
                        a.ad.clone()
                    }
                });
                let etkin: Vec<Anotasyon> = a.anotasyonlar.iter().filter(|x| !self.etkisiz_kisit(r.0, a, x)).cloned().collect();
                let mut alan = self.alan(r.0, &a.tip, &etkin, a.satir, dogrulama, derinlik);
                if self.kotlin_zorunlu(a) {
                    alan.zorunlu = true;
                }
                if r != s {
                    alan.kalitimdan = Some(sn.ad.clone());
                }
                out.insert(ad, alan);
            }
        }
    }

    /// Kotlin kurucu özelliğinin serileştiriciye göre zorunluluğu (Java alanı ve gövdedeki özellik için false).
    /// jackson-module-kotlin: boş olamayan, varsayılansız ve ilkel olmayan (eksik `Int` 0 olur) alan zorunlu;
    /// kotlinx.serialization: varsayılansız alan zorunlu (`explicitNulls = false` ise boş olabilen alan isteğe bağlı)
    fn kotlin_zorunlu(&self, a: &Degisken) -> bool {
        const KT_ILKEL: [&str; 8] = ["Integer", "Long", "Short", "Byte", "Double", "Float", "Boolean", "Character"];
        if !a.kurucu {
            return false;
        }
        match self.serilestirici {
            Serilestirici::Jackson { kotlin_modulu } => kotlin_modulu && a.kotlin_zorunlu && !KT_ILKEL.contains(&temel_ad(&a.tip).as_str()),
            Serilestirici::Kotlinx { acik_bos } => a.deger.is_none() && (!a.bos_olabilir || acik_bos),
            Serilestirici::Bilinmeyen => false,
        }
    }

    /// Kısıt bu alanı doğrulamıyor mu: Kotlin kurucu özelliğine hedefsiz yazılan Bean Validation kısıtı parametreye gider
    /// (`param:`, `property:`, `set:` hedefleri de alan değildir); `field:` ve `get:` alanı doğrular
    pub(crate) fn etkisiz_kisit(&self, di: usize, a: &Degisken, x: &Anotasyon) -> bool {
        let kisit = JAKARTA.contains(&x.ad.as_str()) || self.bul(&x.ad, di).is_some_and(|r| self.sinif(r).tur == "annotation" && var_mi(&self.sinif(r).anotasyonlar, "Constraint"));
        if !kisit {
            return false;
        }
        match x.hedef.as_deref() {
            Some("field" | "get") => false,
            Some(_) => true,
            None => a.kurucu && !self.kt_alan_hedefi,
        }
    }

    /// Java tipinden ve Jakarta Validation anotasyonlarından alan (kurallar class-validator adlarıyla: kart, önizleme ve
    /// OpenAPI aynı kural çözücüyü kullanır)
    pub(crate) fn alan(&self, di: usize, tip: &str, anot: &[Anotasyon], satir: usize, dogrulama: bool, derinlik: usize) -> Alan {
        let (ic_tip, dizi) = sarmalayicisiz(tip);
        let temel = temel_ad(&ic_tip);
        let mut kurallar: Vec<String> = vec![];
        let mut degerler = vec![];
        let mut ic = None;
        let tekil = |kurallar: &mut Vec<String>, ic: &mut Option<BTreeMap<String, Alan>>, degerler: &mut Vec<String>| -> Option<&'static str> {
            Some(match temel.as_str() {
                "String" | "CharSequence" | "char" | "Character" => {
                    kurallar.push("IsString()".into());
                    "string"
                }
                "int" | "Integer" | "long" | "Long" | "short" | "Short" | "byte" | "Byte" | "BigInteger" => {
                    kurallar.push("IsInt()".into());
                    "int"
                }
                "double" | "Double" | "float" | "Float" | "BigDecimal" | "Number" => {
                    kurallar.push("IsNumber()".into());
                    "number"
                }
                "boolean" | "Boolean" => {
                    kurallar.push("IsBoolean()".into());
                    "boolean"
                }
                "UUID" => {
                    kurallar.push("IsUUID()".into());
                    "string"
                }
                "LocalDate" | "LocalDateTime" | "OffsetDateTime" | "ZonedDateTime" | "Instant" | "Date" => {
                    kurallar.push("IsDateString()".into());
                    "date"
                }
                "Map" | "HashMap" | "LinkedHashMap" | "JsonNode" | "ObjectNode" => {
                    kurallar.push("IsObject()".into());
                    "object"
                }
                _ => {
                    let r = self.bul(&ic_tip, di)?;
                    let sn = self.sinif(r);
                    if sn.tur == "enum" {
                        // @JsonValue enum'un JSON değerini değiştirir: değerler adlardan okunamaz
                        if !sn.yontemler.iter().any(|m| var_mi(&m.anotasyonlar, "JsonValue")) {
                            degerler.clone_from(&sn.sabitler);
                        }
                        kurallar.push(format!("IsEnum({})", sn.ad));
                        "enum"
                    } else if sn.tur == "interface" {
                        return None;
                    } else {
                        if derinlik < 3 {
                            let mut m = BTreeMap::new();
                            // İç içe kısıtlar yalnızca alan @Valid taşıyorsa çalışır
                            self.alanlar(r, dogrulama && var_mi(anot, "Valid"), derinlik + 1, &mut m);
                            *ic = Some(m);
                        }
                        "object"
                    }
                }
            })
        };
        let tur = if dizi {
            let mut oge_kurallari = vec![];
            let _ = tekil(&mut oge_kurallari, &mut ic, &mut degerler);
            if oge_kurallari.iter().any(|k| k.starts_with("IsEnum")) {
                kurallar.push(oge_kurallari[0].replace(')', ", { each: true })"));
            }
            kurallar.push("IsArray()".into());
            Some("array")
        } else {
            tekil(&mut kurallar, &mut ic, &mut degerler)
        };
        let mut zorunlu = false;
        if dogrulama {
            for a in anot {
                let sayi = |anahtar: &[&str]| -> Option<String> {
                    let v = a.deger(anahtar)?;
                    let v = java_metni(v).unwrap_or_else(|| v.trim().trim_end_matches(['L', 'l']).to_string());
                    v.parse::<f64>().ok().map(|_| v)
                };
                match a.ad.as_str() {
                    "NotNull" => zorunlu = true,
                    "NotBlank" => {
                        zorunlu = true;
                        kurallar.push("IsNotEmpty()".into());
                    }
                    "NotEmpty" => {
                        zorunlu = true;
                        kurallar.push(if dizi { "ArrayNotEmpty()" } else { "IsNotEmpty()" }.into());
                    }
                    "Size" | "Length" => {
                        let (en_az, en_cok) = if dizi { ("ArrayMinSize", "ArrayMaxSize") } else { ("MinLength", "MaxLength") };
                        if let Some(n) = sayi(&["min"]) {
                            kurallar.push(format!("{en_az}({n})"));
                        }
                        if let Some(n) = sayi(&["max"]) {
                            kurallar.push(format!("{en_cok}({n})"));
                        }
                    }
                    "Min" | "DecimalMin" => kurallar.extend(sayi(&["value"]).map(|n| format!("Min({n})"))),
                    "Max" | "DecimalMax" => kurallar.extend(sayi(&["value"]).map(|n| format!("Max({n})"))),
                    "Range" => {
                        kurallar.extend(sayi(&["min"]).map(|n| format!("Min({n})")));
                        kurallar.extend(sayi(&["max"]).map(|n| format!("Max({n})")));
                    }
                    "Positive" => kurallar.push("IsPositive()".into()),
                    "PositiveOrZero" => kurallar.push("Min(0)".into()),
                    "Negative" => kurallar.push("IsNegative()".into()),
                    "NegativeOrZero" => kurallar.push("Max(0)".into()),
                    "Email" => kurallar.push("IsEmail()".into()),
                    "UUID" => kurallar.push("IsUUID()".into()),
                    "URL" => kurallar.push("IsUrl()".into()),
                    "Pattern" => {
                        // Java'nın matches()'i bütün metni eşler; bayraklar (Pattern.Flag) önizlemede denetlenemez
                        match a.deger(&["regexp"]).and_then(java_metni) {
                            Some(r) if a.deger(&["flags"]).is_none() && !r.contains('/') => kurallar.push(format!("Matches(/^(?:{r})$/)")),
                            _ => kurallar.push("Pattern()".into()),
                        }
                    }
                    "Valid" if ic.is_some() => kurallar.push("ValidateNested()".into()),
                    // Önizlemenin sınayamadığı Jakarta kısıtları: kural "kontrol edilemedi" olarak bildirilir
                    "AssertTrue" | "AssertFalse" | "Past" | "PastOrPresent" | "Future" | "FutureOrPresent" | "Digits" | "Null" => kurallar.push(format!("{}()", a.ad)),
                    // Projedeki özel kısıt (@Constraint taşıyan anotasyon): doğrulayıcısı uygulamanın kodu, sınanamaz
                    x if self.bul(x, di).is_some_and(|r| self.sinif(r).tur == "annotation" && var_mi(&self.sinif(r).anotasyonlar, "Constraint")) => kurallar.push(format!("{x}()")),
                    _ => {}
                }
            }
        }
        if ilkel(&temel) && !dizi {
            zorunlu = false;
        }
        Alan { tip: tur.map(String::from), zorunlu, belirsiz: false, kurallar, satir, kalitimdan: None, degerler, ic: if dizi || tur == Some("object") { ic } else { None } }
    }

    /// DTO sınıfının şeması
    pub(crate) fn sema(&self, r: SRef, dogrulama: bool, dizi: bool, istek: bool) -> Sema {
        let mut alanlar = BTreeMap::new();
        self.alanlar(r, dogrulama, 0, &mut alanlar);
        let sn = self.sinif(r);
        // Kök adla sarma açıksa gövde `{"<kök ad>": {...}}`: kök ad @JsonRootName, yoksa sınıfın adı
        if istek && self.kok_sarma && !dizi {
            let kok_ad = anotasyon(&sn.anotasyonlar, "JsonRootName").and_then(|a| a.deger(&["value"])).and_then(java_metni).unwrap_or_else(|| sn.ad.clone());
            let ic = std::mem::take(&mut alanlar);
            let kurallar = if dogrulama { vec!["ValidateNested()".to_string()] } else { vec![] };
            alanlar
                .insert(kok_ad, Alan { tip: Some("object".into()), zorunlu: true, belirsiz: false, kurallar, satir: sn.satir, kalitimdan: None, degerler: vec![], ic: Some(ic) });
        }
        Sema { adaylar: vec![], dizi, dto: sn.ad.clone(), dosya: Some(self.goreli(r.0)), bulunamadi: false, kalitim: sn.ust.as_deref().map(temel_ad), alanlar }
    }

    /// Yanıt şeması: dönüş tipi (ResponseEntity / Optional / Mono sarmalayıcıları açılır, koleksiyonlar dizi). Tip bir
    /// proje DTO'su değilse (Object, `?`, Map, Page) şema yok
    fn yanit(&self, di: usize, y: &Yontem, durum: Option<u16>) -> Option<Yanit> {
        let (t, dizi) = sarmalayicisiz(&y.donus);
        if matches!(t.trim(), "?" | "Object" | "Void" | "void") || t.contains('?') {
            return None;
        }
        let r = self.bul(&t, di).filter(|r| matches!(self.sinif(*r).tur, "class" | "record"))?;
        Some(Yanit { kaynak: "donus_tipi".into(), kesin: true, durum, sema: self.sema(r, false, dizi, false) })
    }
}

/// Üst düzey `+` ile ayrılmış parçalar (metin ve parantez içi bölünmez)
fn ust_duzey_arti(s: &str) -> Vec<&str> {
    let (mut v, mut d, mut bas) = (vec![], 0i32, 0);
    let mut tirnak = false;
    let mut onceki = ' ';
    for (i, c) in s.char_indices() {
        match c {
            '"' if onceki != '\\' => tirnak = !tirnak,
            _ if tirnak => {}
            '(' | '[' | '{' => d += 1,
            ')' | ']' | '}' => d -= 1,
            '+' if d == 0 => {
                v.push(s[bas..i].trim());
                bas = i + 1;
            }
            _ => {}
        }
        onceki = c;
    }
    v.push(s[bas..].trim());
    v
}

// ---- Güvenlik ----

#[derive(Clone, Debug, PartialEq)]
enum Karar {
    Acik,
    Giris,
    Rol(Vec<String>),
    Yasak,
    Bilinmeyen,
}

#[derive(Clone, Debug)]
enum Desen {
    Ant(String),
    Regex(regex::Regex),
}

impl Desen {
    fn eslesir(&self, yol: &str) -> bool {
        match self {
            Desen::Ant(d) => desen_eslesir(d, yol),
            Desen::Regex(r) => r.is_match(&yol.split('/').map(|p| if p.starts_with(':') { "x" } else { p }).collect::<Vec<_>>().join("/")),
        }
    }
}

/// Eşleyici: (HTTP yöntemi, desenler, her istek mi, çözülemeyen desen var mı)
#[derive(Clone, Debug, Default)]
struct Eslesme {
    yontem: Option<String>,
    desenler: Vec<Desen>,
    her: bool,
    bilinmeyen: bool,
}

impl Eslesme {
    /// İstek bu eşleyiciye uyar mı: Some(evet/hayır), çözülemeyen desen varsa ve bilinen desenler eşleşmiyorsa None
    fn uyar(&self, yontem: &str, yol: &str) -> Option<bool> {
        if self.yontem.as_deref().is_some_and(|y| y != yontem) {
            return Some(false);
        }
        if self.her || self.desenler.iter().any(|d| d.eslesir(yol)) {
            return Some(true);
        }
        if self.bilinmeyen {
            return None;
        }
        Some(false)
    }
}

#[derive(Clone, Debug)]
struct Kural {
    eslesme: Eslesme,
    karar: Karar,
    metin: String,
}

#[derive(Clone, Debug)]
struct Zincir {
    ad: String,
    /// `securityMatcher(...)`: None ise bütün istekler
    eslesen: Option<Eslesme>,
    sira: i64,
    kurallar: Vec<Kural>,
    /// `authorizeHttpRequests` (eşleşmeyen istek reddedilir) Some(true), eski `authorizeRequests` Some(false), yetkilendirme yok None
    yetkilendirme: Option<bool>,
}

#[derive(Default)]
struct Guvenlik {
    bagimlilik: bool,
    /// Güvenlik otomatik yapılandırması dışlanmış
    kapali: bool,
    zincirler: Vec<Zincir>,
    yok_sayilan: Vec<Eslesme>,
    on_post: bool,
    secured: bool,
    jsr250: bool,
    /// Kimlik denetleyen ara katman sınıfları (HandlerInterceptor / Filter; 401 / 403 döndürenler)
    ara_katmanlar: Vec<String>,
    /// Bir `@Aspect` sınıfının kestiği proje anotasyonları (`@Around("@annotation(ApiKeySecured)")`): tanınmayan guard
    aop: Vec<String>,
}

/// Metindeki çağrılar kaynak sırasıyla: (ad, argüman metni, başlangıç, argümanların bitişi). Metin ve yorum içleri atlanır
fn cagri_dizisi(m: &str) -> Vec<(String, String, usize, usize)> {
    let b = m.as_bytes();
    // Metin ya da yorum içindeki baytlar
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
    let ad_mi = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80;
    let mut v = vec![];
    let mut i = 0;
    while i < b.len() {
        if maske[i] || !ad_mi(b[i]) || (i > 0 && ad_mi(b[i - 1])) || b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let bas = i;
        while i < b.len() && ad_mi(b[i]) {
            i += 1;
        }
        let ad = &m[bas..i];
        let mut j = i;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < b.len() && b[j] == b'(' && !maske[j] {
            let mut d = 0i32;
            let mut son = b.len();
            for (x, c) in b.iter().enumerate().skip(j) {
                if maske[x] {
                    continue;
                }
                match c {
                    b'(' => d += 1,
                    b')' => {
                        d -= 1;
                        if d == 0 {
                            son = x;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            v.push((ad.to_string(), m.get(j + 1..son).unwrap_or("").to_string(), bas, son));
        }
    }
    v
}

impl Baglam<'_> {
    /// Eşleyici argümanları: metin desenleri, `HttpMethod.X`, sabitler (dizi dahil), `antMatcher("/x")` gibi yardımcı
    /// çağrıların içindeki metinler. Statik kaynak ve uç (actuator) eşleyicileri API yolu değildir
    fn eslesme_coz(&self, di: usize, s: SRef, arg: &str, regex_mi: bool) -> Eslesme {
        static METIN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let metin_d = METIN.get_or_init(|| regex::Regex::new(r#""((?:[^"\\]|\\.)*)""#).unwrap());
        let mut e = Eslesme::default();
        let ekle = |e: &mut Eslesme, d: String| {
            if regex_mi {
                match regex::Regex::new(&d) {
                    Ok(r) => e.desenler.push(Desen::Regex(r)),
                    Err(_) => e.bilinmeyen = true,
                }
            } else {
                e.desenler.push(Desen::Ant(d));
            }
        };
        for p in ust_duzey_bol(arg) {
            let parca = p.trim();
            let yontem = parca.strip_prefix("HttpMethod.").unwrap_or(parca);
            if matches!(yontem, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS") {
                e.yontem = Some(yontem.to_string());
                continue;
            }
            if ["toStaticResources", "toH2Console", "EndpointRequest", "PathRequest"].iter().any(|x| parca.contains(x)) {
                continue;
            }
            if parca.contains('(') {
                let metinler: Vec<String> = metin_d.captures_iter(parca).filter_map(|c| java_metni(&format!("\"{}\"", &c[1]))).collect();
                if let Some(y) = ["GET", "POST", "PUT", "PATCH", "DELETE"].iter().find(|y| parca.contains(&format!("HttpMethod.{y}"))) {
                    e.yontem = Some(y.to_string());
                }
                if metinler.is_empty() {
                    e.bilinmeyen = true;
                }
                for m in metinler.into_iter().filter(|m| m.starts_with('/') || regex_mi) {
                    ekle(&mut e, m);
                }
                continue;
            }
            match self.metinler(di, Some(s), parca, 0) {
                Some(v) => {
                    for m in v {
                        ekle(&mut e, m);
                    }
                }
                None => e.bilinmeyen = true,
            }
        }
        if e.desenler.is_empty() && !e.bilinmeyen && e.yontem.is_some() {
            e.her = true;
        }
        e
    }

    /// Kural fiili ve argümanından karar: permitAll, authenticated, hasRole("X") ...
    fn kural_karari(&self, di: usize, s: SRef, fiil: &str, arg: &str) -> Option<Karar> {
        Some(match fiil {
            "permitAll" | "anonymous" => Karar::Acik,
            "authenticated" | "fullyAuthenticated" | "rememberMe" => Karar::Giris,
            "denyAll" => Karar::Yasak,
            "hasRole" | "hasAnyRole" | "hasAuthority" | "hasAnyAuthority" => {
                let mut roller = vec![];
                for p in ust_duzey_bol(arg) {
                    match self.metinler(di, Some(s), &p, 0) {
                        Some(v) => roller.extend(v.into_iter().map(|r| r.trim_start_matches("ROLE_").to_string())),
                        None => return Some(Karar::Bilinmeyen),
                    }
                }
                Karar::Rol(roller)
            }
            "access" | "hasIpAddress" => Karar::Bilinmeyen,
            _ => return None,
        })
    }

    /// Güvenlik zinciri tanımı (`@Bean SecurityFilterChain` ya da `WebSecurityConfigurerAdapter.configure(HttpSecurity)`):
    /// `securityMatcher`, kurallar sırasıyla ve yetkilendirme biçimi
    fn zincir_oku(&self, s: SRef, y: &Yontem) -> Zincir {
        let di = s.0;
        let m = &y.govde_metni;
        let cagrilar = cagri_dizisi(m);
        let mut yetkilendirme = None;
        // Kuralların bulunduğu aralıklar: lambda DSL'de argüman; eski zincirde sonraki .and()'e kadar
        let mut araliklar: Vec<(usize, usize)> = vec![];
        for (ad, arg, bas, son) in &cagrilar {
            let yeni = match ad.as_str() {
                "authorizeHttpRequests" | "authorizeExchange" => true,
                "authorizeRequests" => false,
                _ => continue,
            };
            yetkilendirme = Some(yetkilendirme.unwrap_or(false) || yeni);
            if arg.trim().is_empty() {
                let bitis = cagrilar.iter().find(|c| c.2 > *son && c.0 == "and").map(|c| c.2).unwrap_or(m.len());
                araliklar.push((*son, bitis));
            } else {
                araliklar.push((*bas, *son));
            }
        }
        let mut eslesen: Option<Eslesme> = None;
        let mut eslesen_araliklari: Vec<(usize, usize)> = vec![];
        for (ad, arg, bas, son) in &cagrilar {
            match ad.as_str() {
                "securityMatcher" => {
                    let e = self.eslesme_coz(di, s, arg, false);
                    let hedef = eslesen.get_or_insert_with(Eslesme::default);
                    hedef.desenler.extend(e.desenler);
                    hedef.bilinmeyen |= e.bilinmeyen;
                }
                "securityMatchers" => eslesen_araliklari.push((*bas, *son)),
                _ => {}
            }
        }
        for (ad, arg, bas, _) in &cagrilar {
            if ad == "requestMatchers" && eslesen_araliklari.iter().any(|(a, b)| bas > a && bas < b) {
                let e = self.eslesme_coz(di, s, arg, false);
                let hedef = eslesen.get_or_insert_with(Eslesme::default);
                hedef.desenler.extend(e.desenler);
                hedef.bilinmeyen |= e.bilinmeyen;
            }
        }
        let mut kurallar = vec![];
        let mut bekleyen: Option<(Eslesme, String)> = None;
        let mut atla = 0usize;
        let mut olumsuz = false;
        for (ad, arg, bas, son) in &cagrilar {
            if !araliklar.iter().any(|(a, b)| bas >= a && bas < b) || *bas < atla {
                continue;
            }
            let eslesme = match ad.as_str() {
                "requestMatchers" | "antMatchers" | "mvcMatchers" | "pathMatchers" => Some(self.eslesme_coz(di, s, arg, false)),
                "regexMatchers" => Some(self.eslesme_coz(di, s, arg, true)),
                "anyRequest" | "anyExchange" => Some(Eslesme { her: true, ..Default::default() }),
                // Yönlendirme türleri (FORWARD, ERROR) bir API yolu değildir
                "dispatcherTypeMatchers" => Some(Eslesme::default()),
                _ => None,
            };
            if let Some(e) = eslesme {
                bekleyen = Some((e, format!("{ad}({})", tek_satir(arg))));
                atla = *son;
                olumsuz = false;
                continue;
            }
            if ad == "not" {
                olumsuz = true;
                continue;
            }
            let Some((e, metin)) = bekleyen.take() else { continue };
            match self.kural_karari(di, s, ad, arg) {
                Some(k) => {
                    let karar = if olumsuz { Karar::Bilinmeyen } else { k };
                    kurallar.push(Kural { eslesme: e, karar, metin: crate::dna::kisa(&format!("{metin}.{ad}({})", tek_satir(arg))) });
                    atla = *son;
                    olumsuz = false;
                }
                None => bekleyen = Some((e, metin)),
            }
        }
        let sira = self
            .sinif(s)
            .yontemler
            .iter()
            .find(|x| x.ad == y.ad)
            .and_then(|x| anotasyon(&x.anotasyonlar, "Order"))
            .or_else(|| anotasyon(&self.sinif(s).anotasyonlar, "Order"))
            .and_then(|o| o.deger(&["value"]))
            .and_then(|v| v.trim().parse::<i64>().ok().or_else(|| v.contains("HIGHEST_PRECEDENCE").then_some(i64::MIN)))
            .unwrap_or(i64::MAX);
        Zincir { ad: format!("{}.{}", self.sinif(s).ad, y.ad), eslesen, sira, kurallar, yetkilendirme }
    }

    /// Uygulamanın güvenlik tanımı: bağımlılık, zincirler, yok sayılan yollar, yöntem güvenliğinin etkinliği
    fn guvenlik(&self, yapi_metinleri: &[String], ayarlar: &[(String, String)]) -> Guvenlik {
        let mut g = Guvenlik { bagimlilik: yapi_metinleri.iter().any(|m| m.contains("spring-boot-starter-security") || m.contains("spring-security-")), ..Default::default() };
        let dislanan = ayar(ayarlar, "spring.autoconfigure.exclude").unwrap_or_default();
        for (di, d) in self.dosyalar.iter().enumerate() {
            for (si, s) in d.siniflar.iter().enumerate() {
                for a in &s.anotasyonlar {
                    match a.ad.as_str() {
                        "SpringBootApplication" | "EnableAutoConfiguration" => {
                            if a.deger(&["exclude"]).is_some_and(|v| v.contains("SecurityAutoConfiguration")) {
                                g.kapali = true;
                            }
                        }
                        "EnableMethodSecurity" | "EnableReactiveMethodSecurity" => {
                            g.on_post = a.deger(&["prePostEnabled"]).is_none_or(|v| v.trim() != "false");
                            g.secured |= a.deger(&["securedEnabled"]).is_some_and(|v| v.trim() == "true");
                            g.jsr250 |= a.deger(&["jsr250Enabled"]).is_some_and(|v| v.trim() == "true");
                        }
                        "EnableGlobalMethodSecurity" => {
                            g.on_post |= a.deger(&["prePostEnabled"]).is_some_and(|v| v.trim() == "true");
                            g.secured |= a.deger(&["securedEnabled"]).is_some_and(|v| v.trim() == "true");
                            g.jsr250 |= a.deger(&["jsr250Enabled"]).is_some_and(|v| v.trim() == "true");
                        }
                        _ => {}
                    }
                }
                let adaptor_alt = s.ust.as_deref().is_some_and(|u| temel_ad(u) == "WebSecurityConfigurerAdapter");
                for y in &s.yontemler {
                    let donus = temel_ad(&y.donus);
                    let zincir = (var_mi(&y.anotasyonlar, "Bean") && matches!(donus.as_str(), "SecurityFilterChain" | "SecurityWebFilterChain"))
                        || (adaptor_alt && y.ad == "configure" && y.parametreler.first().is_some_and(|p| temel_ad(&p.tip) == "HttpSecurity"));
                    if zincir {
                        g.zincirler.push(self.zincir_oku((di, si), y));
                    }
                    // web.ignoring().requestMatchers(...): güvenlik zincirinden tamamen çıkarılan yollar
                    let ozellestirici = (var_mi(&y.anotasyonlar, "Bean") && donus == "WebSecurityCustomizer")
                        || (adaptor_alt && y.ad == "configure" && y.parametreler.first().is_some_and(|p| temel_ad(&p.tip) == "WebSecurity"));
                    if ozellestirici {
                        for (ad, arg, ..) in cagri_dizisi(&y.govde_metni) {
                            if matches!(ad.as_str(), "requestMatchers" | "antMatchers" | "mvcMatchers") {
                                g.yok_sayilan.push(self.eslesme_coz(di, (di, si), &arg, false));
                            }
                        }
                    }
                }
                // Kimlik denetleyen ara katman: Spring Security dışındaki bir engel olabilir
                let ara = s.ust.iter().chain(s.arayuzler.iter()).any(|t| matches!(temel_ad(t).as_str(), "HandlerInterceptor" | "AsyncHandlerInterceptor" | "WebFilter"));
                if ara && s.yontemler.iter().any(|y| ["401", "403", "UNAUTHORIZED", "FORBIDDEN", "SC_UNAUTHORIZED", "SC_FORBIDDEN"].iter().any(|x| y.govde_metni.contains(x))) {
                    g.ara_katmanlar.push(s.ad.clone());
                }
            }
        }
        if dislanan.contains("SecurityAutoConfiguration") {
            g.kapali = true;
        }
        let kesimler: Vec<&str> = self
            .dosyalar
            .iter()
            .flat_map(|d| &d.siniflar)
            .filter(|s| var_mi(&s.anotasyonlar, "Aspect"))
            .flat_map(|s| &s.yontemler)
            .flat_map(|y| &y.anotasyonlar)
            .filter(|a| matches!(a.ad.as_str(), "Pointcut" | "Around" | "Before" | "After" | "AfterReturning"))
            .flat_map(|a| a.argumanlar.iter().map(|(_, v)| v.as_str()))
            .collect();
        if !kesimler.is_empty() {
            for s in self.dosyalar.iter().flat_map(|d| &d.siniflar).filter(|s| s.tur == "annotation") {
                let ad_mi = |c: char| c.is_alphanumeric() || c == '_';
                let gecer = |k: &str| k.match_indices(s.ad.as_str()).any(|(i, _)| !k[..i].ends_with(ad_mi) && !k[i + s.ad.len()..].starts_with(ad_mi));
                if kesimler.iter().any(|k| gecer(k)) {
                    g.aop.push(s.ad.clone());
                }
            }
        }
        g.zincirler.sort_by_key(|z| z.sira);
        g
    }

    /// Yöntem güvenliği anotasyonu (yöntem, eşleme kaynağı, sınıf ve üstleri sırasıyla): (karar, etkin mi, metin)
    fn yontem_guvenligi(&self, g: &Guvenlik, s: SRef, yontemler: &[&Yontem]) -> Option<(Karar, bool, String)> {
        let mut listeler: Vec<&[Anotasyon]> = yontemler.iter().map(|y| y.anotasyonlar.as_slice()).collect();
        listeler.push(&self.sinif(s).anotasyonlar);
        for u in self.ustler(s) {
            listeler.push(&self.sinif(u).anotasyonlar);
        }
        for l in listeler {
            for a in l {
                let deger = || a.deger(&["value"]).map(String::from).unwrap_or_default();
                let (karar, etkin) = match a.ad.as_str() {
                    "PreAuthorize" | "PostAuthorize" => (spel_karari(&java_metni(&deger()).unwrap_or_default()), g.on_post),
                    "Secured" => {
                        let v = self.metinler(s.0, Some(s), &deger(), 0).unwrap_or_default();
                        if v.iter().any(|x| x.contains("ANONYMOUSLY")) {
                            (Karar::Acik, g.secured)
                        } else {
                            (Karar::Rol(v.iter().map(|r| r.trim_start_matches("ROLE_").to_string()).collect()), g.secured)
                        }
                    }
                    "RolesAllowed" => {
                        (Karar::Rol(self.metinler(s.0, Some(s), &deger(), 0).unwrap_or_default().iter().map(|r| r.trim_start_matches("ROLE_").to_string()).collect()), g.jsr250)
                    }
                    "PermitAll" => (Karar::Acik, g.jsr250),
                    "DenyAll" => (Karar::Yasak, g.jsr250),
                    _ => continue,
                };
                let metin = if a.argumanlar.is_empty() {
                    format!("@{}", a.ad)
                } else {
                    format!("@{}({})", a.ad, a.argumanlar.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>().join(", "))
                };
                return Some((karar, etkin, metin));
            }
        }
        None
    }
}

/// `@PreAuthorize` ifadesinin kararı. Yalnızca hasRole / hasAnyRole / hasAuthority / hasAnyAuthority, isAuthenticated,
/// permitAll, denyAll ve and / or / not birleşimleri anlamlandırılır; başka bir şey (`@bean.check(#id)`, `principal`)
/// tanınmayan guard'dır
fn spel_karari(e: &str) -> Karar {
    static D: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let [rol, cagri, metin] = D.get_or_init(|| {
        [
            regex::Regex::new(r"has(?:Any)?(?:Role|Authority)\s*\(([^)]*)\)").unwrap(),
            regex::Regex::new(r"\b(?:has(?:Any)?(?:Role|Authority)|isAuthenticated|isFullyAuthenticated|isRememberMe|isAnonymous|permitAll|denyAll)\s*\([^)]*\)|\b(?:and|or|not)\b|&&|\|\||!|[()\s]").unwrap(),
            regex::Regex::new(r#"'([^']*)'|"([^"]*)""#).unwrap(),
        ]
    });
    if !cagri.replace_all(e, "").trim().is_empty() || e.trim().is_empty() {
        return Karar::Bilinmeyen;
    }
    if e.contains("denyAll") {
        return Karar::Yasak;
    }
    let roller: Vec<String> = rol
        .captures_iter(e)
        .flat_map(|c| metin.captures_iter(&c[1]).filter_map(|m| m.get(1).or(m.get(2)).map(|x| x.as_str().trim_start_matches("ROLE_").to_string())).collect::<Vec<_>>())
        .collect();
    let giris = ["isAuthenticated", "isFullyAuthenticated", "isRememberMe"].iter().any(|x| e.contains(x));
    let veya = e.contains(" or ") || e.contains("||");
    if !roller.is_empty() && !(giris && veya) {
        return Karar::Rol(roller);
    }
    if giris || !roller.is_empty() {
        return Karar::Giris;
    }
    if e.contains("permitAll") || e.contains("isAnonymous") {
        return Karar::Acik;
    }
    Karar::Bilinmeyen
}

/// Giriş kararı: (giriş, kanıt, roller, rol kanıtı, guard metinleri)
struct GirisKarari {
    giris: &'static str,
    kanit: &'static str,
    roller: Vec<String>,
    roller_kaniti: Option<String>,
    guardlar: Vec<String>,
}

fn karar_cevir(k: &Karar, metin: &str, kesin: bool) -> GirisKarari {
    let kanit = if kesin { "kesin" } else { "varsayim" };
    let mut g = GirisKarari { giris: "gerekli", kanit, roller: vec![], roller_kaniti: None, guardlar: vec![metin.to_string()] };
    match k {
        Karar::Acik => g.giris = "herkese_acik",
        Karar::Giris | Karar::Yasak => {}
        Karar::Rol(r) => {
            g.roller = r.clone();
            g.roller_kaniti = (!kesin).then(|| "varsayim".into());
        }
        Karar::Bilinmeyen => g.kanit = "varsayim_bilinmeyen_guard",
    }
    g
}

impl Baglam<'_> {
    /// Kapının giriş kararı: önce filtre zinciri (yol bağlam yolu olmadan), sonra yöntem güvenliği
    fn giris_karari(&self, g: &Guvenlik, yontem: &str, yol: &str, s: SRef, yontemler: &[&Yontem]) -> GirisKarari {
        let mut sonuc = if g.yok_sayilan.iter().any(|e| e.uyar(yontem, yol) == Some(true)) {
            GirisKarari { giris: "herkese_acik", kanit: "kesin", roller: vec![], roller_kaniti: None, guardlar: vec!["web.ignoring()".into()] }
        } else if g.kapali || (!g.bagimlilik && g.zincirler.is_empty()) {
            GirisKarari { giris: "herkese_acik", kanit: "varsayim", roller: vec![], roller_kaniti: None, guardlar: vec![] }
        } else if g.zincirler.is_empty() {
            // Spring Boot varsayılanı: bütün istekler kimlik ister
            GirisKarari { giris: "gerekli", kanit: "varsayim", roller: vec![], roller_kaniti: None, guardlar: vec!["Spring Security varsayılanı".into()] }
        } else {
            let mut belirsiz = false;
            let zincir = g.zincirler.iter().find(|z| match z.eslesen.as_ref().map(|e| e.uyar(yontem, yol)) {
                None => true,
                Some(Some(e)) => e,
                Some(None) => {
                    belirsiz = true;
                    false
                }
            });
            match zincir {
                // Hiçbir zincir isteği kapsamıyor: güvenlik uygulanmaz
                None => GirisKarari {
                    giris: "herkese_acik",
                    kanit: if belirsiz { "varsayim_bilinmeyen_guard" } else { "kesin" },
                    roller: vec![],
                    roller_kaniti: None,
                    guardlar: vec![],
                },
                Some(z) => {
                    let mut karar = None;
                    for k in &z.kurallar {
                        match k.eslesme.uyar(yontem, yol) {
                            Some(true) => {
                                karar = Some(k);
                                break;
                            }
                            Some(false) => {}
                            None => belirsiz = true,
                        }
                    }
                    let mut r = match (karar, z.yetkilendirme) {
                        (Some(k), _) => karar_cevir(&k.karar, &format!("{}: {}", z.ad, k.metin), true),
                        // authorizeHttpRequests eşleşmeyen isteği reddeder; eski authorizeRequests geçirir
                        (None, Some(true)) => GirisKarari { giris: "gerekli", kanit: "varsayim", roller: vec![], roller_kaniti: None, guardlar: vec![z.ad.clone()] },
                        (None, _) => GirisKarari { giris: "herkese_acik", kanit: "varsayim", roller: vec![], roller_kaniti: None, guardlar: vec![z.ad.clone()] },
                    };
                    if belirsiz && r.kanit == "kesin" {
                        r.kanit = "varsayim_bilinmeyen_guard";
                    }
                    r
                }
            }
        };
        if let Some((k, etkin, metin)) = self.yontem_guvenligi(g, s, yontemler) {
            sonuc.guardlar.push(metin.clone());
            match k {
                Karar::Acik => {}
                Karar::Giris | Karar::Yasak => {
                    if sonuc.giris != "gerekli" || sonuc.kanit != "kesin" {
                        sonuc.giris = "gerekli";
                        sonuc.kanit = if etkin { "kesin" } else { "varsayim" };
                    }
                }
                Karar::Rol(r) => {
                    if sonuc.giris != "gerekli" || sonuc.kanit != "kesin" {
                        sonuc.giris = "gerekli";
                        sonuc.kanit = if etkin { "kesin" } else { "varsayim" };
                    }
                    // İki katman da uygulanır: ikisinin de izin verdiği roller
                    let ortak: Vec<String> = if sonuc.roller.is_empty() { r.clone() } else { r.iter().filter(|x| sonuc.roller.contains(x)).cloned().collect() };
                    sonuc.roller = if ortak.is_empty() { r } else { ortak };
                    sonuc.roller_kaniti = (!etkin).then(|| "varsayim".into());
                }
                Karar::Bilinmeyen => {
                    if sonuc.kanit == "kesin" && sonuc.giris != "gerekli" || sonuc.kanit == "varsayim" {
                        sonuc.kanit = "varsayim_bilinmeyen_guard";
                    }
                }
            }
        }
        // AOP ile kesilen proje anotasyonu: denetimi uygulamanın kodu
        let aop: Vec<&Anotasyon> = yontemler.iter().flat_map(|y| &y.anotasyonlar).chain(&self.sinif(s).anotasyonlar).filter(|a| g.aop.contains(&a.ad)).collect();
        if !aop.is_empty() {
            sonuc.guardlar.extend(aop.iter().map(|a| format!("@{} (AOP)", a.ad)));
            if sonuc.giris == "herkese_acik" {
                sonuc.kanit = "varsayim_bilinmeyen_guard";
            }
        }
        if !g.ara_katmanlar.is_empty() && sonuc.giris == "herkese_acik" {
            sonuc.kanit = "varsayim_bilinmeyen_guard";
            sonuc.guardlar.extend(g.ara_katmanlar.iter().cloned());
        }
        sonuc.roller.sort();
        sonuc.roller.dedup();
        sonuc
    }

    // ---- Rotalar ----

    /// Sınıfın etkin yöntemleri: kendisininkiler ve üst sınıflardan gelen (geçersiz kılınmamış) gövdeli yöntemler
    fn etkin_yontemler(&self, s: SRef) -> Vec<YRef> {
        let mut v: Vec<YRef> = vec![];
        for r in std::iter::once(s).chain(self.ust_siniflar(s)) {
            for (yi, y) in self.sinif(r).yontemler.iter().enumerate() {
                if !v.iter().any(|x| {
                    let m = self.yontem(*x);
                    m.ad == y.ad && m.parametreler.len() == y.parametreler.len()
                }) {
                    v.push((r.0, r.1, yi));
                }
            }
        }
        v
    }

    /// Yöntemin eşleme anotasyonunun kaynağı: kendisi, yoksa üst sınıf / arayüzlerdeki aynı imzalı yöntem
    fn esleme_kaynagi(&self, s: SRef, y: YRef) -> Option<YRef> {
        let m = self.yontem(y);
        if esleme_anotasyonu(&m.anotasyonlar).is_some() {
            return Some(y);
        }
        for u in self.ustler(s) {
            for (yi, x) in self.sinif(u).yontemler.iter().enumerate() {
                if x.ad == m.ad && x.parametreler.len() == m.parametreler.len() && esleme_anotasyonu(&x.anotasyonlar).is_some() {
                    return Some((u.0, u.1, yi));
                }
            }
        }
        None
    }

    /// Eşleme anotasyonu: (yöntemler, yollar, yöntem verilmemiş mi). Yol çözülemezse None
    fn esleme_coz(&self, s: SRef, a: &Anotasyon) -> Option<(Vec<String>, Vec<String>, bool)> {
        let yollar = match a.deger(&["value", "path"]) {
            None => vec![String::new()],
            Some(v) => self.metinler(s.0, Some(s), v, 0)?,
        };
        let yollar = if yollar.is_empty() { vec![String::new()] } else { yollar };
        if let Some((_, y)) = ESLEMELER.iter().find(|(x, _)| *x == a.ad) {
            return Some((vec![y.to_string()], yollar, false));
        }
        let yontemler: Vec<String> = a
            .deger(&["method"])
            .map(|m| m.trim_matches(['{', '}']).split(',').map(|x| x.trim().rsplit('.').next().unwrap_or("").to_uppercase()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default();
        let yok = yontemler.is_empty();
        Some((if yok { vec!["GET".into()] } else { yontemler }, yollar, yok))
    }
}

fn esleme_anotasyonu(l: &[Anotasyon]) -> Option<&Anotasyon> {
    l.iter().find(|a| a.ad == "RequestMapping" || ESLEMELER.iter().any(|(x, _)| *x == a.ad))
}

/// İstek parametresinin bağlama anotasyonları
const BAGLAMA: [&str; 7] = ["PathVariable", "RequestParam", "RequestBody", "ModelAttribute", "RequestPart", "Valid", "Validated"];

fn ana_sinif_mi(s: &Sinif) -> bool {
    var_mi(&s.anotasyonlar, "SpringBootApplication")
}

/// Veri tabanı modeli tanımlayan sınıf: JPA `@Entity`, Mongo `@Document`, Spring Data JDBC / R2DBC `@Table`; Exposed
/// tablo nesnesi (Kotlin okuyucusunun `ExposedTable` işareti)
pub(crate) fn varlik_mi(s: &Sinif) -> bool {
    ["Entity", "Document", "Table", "Node", "ExposedTable"].iter().any(|a| var_mi(&s.anotasyonlar, a)) && s.tur == "class"
}

pub fn kapilar(kok: &Path, dosyalar: &[Dosya], ayarlar: &[(String, String)], yapi_metinleri: &[String]) -> (Vec<Kapi>, Value) {
    let yilan_adlar = ayar(ayarlar, "spring.jackson.property-naming-strategy").is_some_and(|v| v.to_uppercase().contains("SNAKE"));
    let kok_sarma = ayar(ayarlar, "spring.jackson.deserialization.unwrap-root-value").is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
    let mut b = Baglam::yeni(kok, dosyalar, yilan_adlar, kok_sarma);
    // Kotlin: jackson-module-kotlin alanların zorunluluğunu belirler; Kotlin 2.2 seçeneği hedefsiz kısıtı alana da yazar
    let kotlin_modulu = yapi_metinleri.is_empty() || yapi_metinleri.iter().any(|m| m.contains("jackson-module-kotlin") || m.contains("jackson.module.kotlin"));
    b.serilestirici = Serilestirici::Jackson { kotlin_modulu };
    b.kt_alan_hedefi = yapi_metinleri.iter().any(|m| m.contains("annotation-default-target=param-property"));
    let b = b;
    let g = b.guvenlik(yapi_metinleri, ayarlar);
    let springdoc = yapi_metinleri.iter().any(|m| m.contains("springdoc"));
    // Genel önek: bağlam yolu ve sunucu uygulaması yolu
    let baglam_yolu = ayar(ayarlar, "server.servlet.context-path").or_else(|| ayar(ayarlar, "spring.webflux.base-path")).unwrap_or_default();
    let sunucu_yolu = ayar(ayarlar, "spring.mvc.servlet.path").unwrap_or_default();
    let genel_onek = {
        let o = duzgun(&yol_birlestir(&baglam_yolu, &sunucu_yolu));
        if o == "/" {
            String::new()
        } else {
            o
        }
    };
    // Modeller: varlık sınıfları ve depoların yönettiği tipler
    let mut modeller: HashSet<String> = dosyalar.iter().flat_map(|d| &d.siniflar).filter(|s| varlik_mi(s)).map(|s| s.ad.clone()).collect();
    modeller.extend(b.depolar.values().cloned());
    // Bütün yöntemlerin zenginleştirilmiş davranış özeti
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
    // Yumuşak silinen varlıklar: @SQLDelete / @SoftDelete -> "dosya:satır"
    let yumusak: HashMap<String, String> = dosyalar
        .iter()
        .enumerate()
        .flat_map(|(di, d)| d.siniflar.iter().map(move |s| (di, s)))
        .filter_map(|(di, s)| s.anotasyonlar.iter().find(|a| matches!(a.ad.as_str(), "SQLDelete" | "SoftDelete")).map(|a| (s.ad.clone(), format!("{}:{}", b.goreli(di), a.satir))))
        .collect();
    let ana = dosyalar.iter().enumerate().find_map(|(di, d)| d.siniflar.iter().find(|s| ana_sinif_mi(s)).map(|s| (di, s)));
    let ana_id = ana.map(|(di, s)| format!("{}:{}", b.goreli(di), s.ad));

    let mut kapilar = vec![];
    let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
    let mut kosullu_kapilar = vec![];
    let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
    let mut routerlar: Vec<Value> = vec![];
    // İşlem kimlikleri: açık @Operation(operationId) ya da springdoc varsayılanı (yöntem adı; aynı ad birden çok yöntemde
    // olursa springdoc sonek ekler: hangisinin hangisi olduğu belirsiz, yazılmaz)
    let mut islem_kimlikleri: BTreeMap<String, String> = BTreeMap::new();
    let mut ad_sayilari: HashMap<String, usize> = HashMap::new();
    let mut varsayilan_kimlikler: Vec<(String, String)> = vec![];
    if let Some((di, s)) = ana {
        routerlar.push(json!({ "id": ana_id, "dosya": b.goreli(di), "satir": s.satir, "ustler": [] }));
    }
    for (di, d) in dosyalar.iter().enumerate() {
        for (si, s) in d.siniflar.iter().enumerate() {
            if s.tur != "class" || s.soyut || !(var_mi(&s.anotasyonlar, "RestController") || var_mi(&s.anotasyonlar, "Controller")) {
                continue;
            }
            let sr = (di, si);
            let rest = var_mi(&s.anotasyonlar, "RestController") || var_mi(&s.anotasyonlar, "ResponseBody");
            // Koşullu kayıt: @Profile / @ConditionalOn...
            let kosul = s.anotasyonlar.iter().find(|a| KOSULLU.contains(&a.ad.as_str())).map(|a| {
                format!("@{}({})", a.ad, a.argumanlar.iter().map(|(k, v)| k.as_ref().map(|k| format!("{k} = {v}")).unwrap_or_else(|| v.clone())).collect::<Vec<_>>().join(", "))
            });
            let sinif_id = format!("{}:{}", b.goreli(di), s.ad);
            let ustler: Vec<Value> =
                ana_id.iter().map(|a| json!({ "id": a, "icerme": format!("{}:{}", b.goreli(di), s.satir), "kosul": kosul, "kosullu": kosul.is_some() })).collect();
            routerlar.push(json!({ "id": sinif_id, "dosya": b.goreli(di), "satir": s.satir, "ustler": ustler }));
            // Sınıf öneki: sınıfın, yoksa üstlerinin @RequestMapping'i
            let sinif_eslemesi = std::iter::once(sr).chain(b.ustler(sr)).find_map(|r| anotasyon(&b.sinif(r).anotasyonlar, "RequestMapping").map(|a| (r, a)));
            let onekler = match sinif_eslemesi {
                Some((r, a)) => match b.esleme_coz(r, a) {
                    Some((_, y, _)) => y,
                    None => continue,
                },
                None => vec![String::new()],
            };
            for yr in b.etkin_yontemler(sr) {
                let m = b.yontem(yr);
                let Some(kaynak) = b.esleme_kaynagi(sr, yr) else { continue };
                let km = b.yontem(kaynak);
                let Some(esleme) = esleme_anotasyonu(&km.anotasyonlar) else { continue };
                let Some((yontemler, yollar, yontemsiz)) = b.esleme_coz((kaynak.0, kaynak.1), esleme) else { continue };
                let konum = format!("{}:{}", b.goreli(yr.0), m.satir);
                let mut notlar: Vec<String> = vec![];
                if yontemsiz {
                    notlar.push("@RequestMapping yöntem belirtmiyor: Spring bütün HTTP yöntemlerini bu işleyiciye yönlendirir; harita GET olarak gösterir".into());
                }
                // Parametre anotasyonları: uygulamadaki, yoksa eşleme kaynağındaki (arayüz) parametreninki
                let parametreler: Vec<(Degisken, Vec<Anotasyon>)> = m
                    .parametreler
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let kendi = p.anotasyonlar.iter().any(|a| BAGLAMA.contains(&a.ad.as_str()) || GIRDI_DISI.contains(&a.ad.as_str()));
                        let a = if kendi || kaynak == yr { p.anotasyonlar.clone() } else { km.parametreler.get(i).map(|x| x.anotasyonlar.clone()).unwrap_or_default() };
                        (p.clone(), a)
                    })
                    .collect();
                let sinif_dogrulamasi = var_mi(&s.anotasyonlar, "Validated");
                let (mut govde, mut sorgu_alanlari, mut yol_param) = (vec![], BTreeMap::new(), vec![]);
                let mut form: BTreeMap<String, Alan> = BTreeMap::new();
                for (p, a) in &parametreler {
                    if a.iter().any(|x| GIRDI_DISI.contains(&x.ad.as_str())) {
                        continue;
                    }
                    let (ic_tip, dizi) = sarmalayicisiz(&p.tip);
                    let temel = temel_ad(&ic_tip);
                    let ad_arg = |x: &Anotasyon| x.deger(&["value", "name"]).and_then(java_metni).filter(|v| !v.is_empty()).unwrap_or_else(|| p.ad.clone());
                    let dogrulama = var_mi(a, "Valid") || var_mi(a, "Validated");
                    if let Some(x) = anotasyon(a, "PathVariable") {
                        yol_param.push(Girdi { ad: Some(ad_arg(x)), tip: Some(temel.clone()), sema: None });
                        continue;
                    }
                    let dosya_mi = matches!(temel.as_str(), "MultipartFile" | "Part" | "FilePart");
                    if dosya_mi || var_mi(a, "RequestPart") {
                        let ad = anotasyon(a, "RequestPart").or_else(|| anotasyon(a, "RequestParam")).map(ad_arg).unwrap_or_else(|| p.ad.clone());
                        if dosya_mi {
                            notlar.push(format!("dosya doğrulaması kontrol edilemedi: '{ad}' parametresi ({temel}); dosyanın varlığı, türü ve içeriği önizlemede sınanmaz"));
                            let zorunlu =
                                anotasyon(a, "RequestPart").or_else(|| anotasyon(a, "RequestParam")).is_none_or(|x| x.deger(&["required"]).is_none_or(|v| v.trim() != "false"));
                            form.insert(
                                ad,
                                Alan { tip: Some("object".into()), zorunlu, belirsiz: false, kurallar: vec![], satir: m.satir, kalitimdan: None, degerler: vec![], ic: None },
                            );
                        } else {
                            let mut alan = b.alan(yr.0, &p.tip, &p.anotasyonlar, m.satir, dogrulama, 0);
                            alan.zorunlu = true;
                            form.insert(ad, alan);
                        }
                        continue;
                    }
                    if let Some(x) = anotasyon(a, "RequestBody") {
                        let sinif = b.bul(&ic_tip, yr.0).filter(|r| matches!(b.sinif(*r).tur, "class" | "record"));
                        match sinif {
                            Some(r) => {
                                govde.push(Girdi { ad: Some(p.ad.clone()), tip: Some(b.sinif(r).ad.clone()), sema: Some(b.sema(r, dogrulama, dizi, true)) });
                                // Kotlin: kurucu özelliğine hedefsiz yazılan kısıt alanı doğrulamaz
                                let mut etkisiz: Vec<&str> =
                                    b.sinif(r).alanlar.iter().flat_map(|f| f.anotasyonlar.iter().filter(|x| b.etkisiz_kisit(r.0, f, x)).map(|x| x.ad.as_str())).collect();
                                etkisiz.dedup();
                                if dogrulama && !etkisiz.is_empty() {
                                    notlar.push(format!(
                                        "{} sınıfında kurucu özelliğine hedefsiz yazılan kısıtlar ({}) Kotlin'de parametreye gider, alan doğrulanmaz: {} gerekir",
                                        b.sinif(r).ad,
                                        etkisiz.iter().map(|x| format!("@{x}")).collect::<Vec<_>>().join(", "),
                                        etkisiz.iter().map(|x| format!("@field:{x}")).collect::<Vec<_>>().join(", ")
                                    ));
                                }
                                if !dogrulama && !b.sinif(r).alanlar.is_empty() {
                                    let kisitli = b.sinif(r).alanlar.iter().any(|f| f.anotasyonlar.iter().any(|x| JAKARTA.contains(&x.ad.as_str())));
                                    if kisitli {
                                        notlar.push(format!("'{}' gövdesinde @Valid yok: {} sınıfındaki Jakarta kısıtları (@NotNull, @Size...) uygulanmaz", p.ad, b.sinif(r).ad));
                                    }
                                }
                            }
                            None => {
                                let _ = x;
                                govde.push(Girdi { ad: Some(p.ad.clone()), tip: Some(anotasyonsuz(&p.tip)), sema: None });
                            }
                        }
                        continue;
                    }
                    if let Some(x) = anotasyon(a, "RequestParam") {
                        if matches!(temel.as_str(), "Map" | "MultiValueMap" | "HashMap") {
                            continue;
                        }
                        let ad = ad_arg(x);
                        let varsayilan = x.deger(&["defaultValue"]).is_some();
                        // Kotlin: boş olabilen tip ve varsayılan değer parametreyi isteğe bağlı kılar
                        let istege =
                            x.deger(&["required"]).is_some_and(|v| v.trim() == "false") || varsayilan || temel_ad(&p.tip) == "Optional" || p.bos_olabilir || p.deger.is_some();
                        let mut alan = b.alan(yr.0, &p.tip, &p.anotasyonlar, m.satir, sinif_dogrulamasi, 0);
                        alan.zorunlu = !istege;
                        sorgu_alanlari.insert(ad, alan);
                        continue;
                    }
                    if temel == "Pageable" {
                        for (ad, tip) in [("page", "int"), ("size", "int"), ("sort", "String")] {
                            sorgu_alanlari.insert(ad.to_string(), b.alan(yr.0, tip, &[], m.satir, false, 0));
                        }
                        continue;
                    }
                    if ENJEKTE.contains(&temel.as_str()) {
                        continue;
                    }
                    if basit_tip(&temel) || b.bul(&ic_tip, yr.0).is_some_and(|r| b.sinif(r).tur == "enum") {
                        // Anotasyonsuz basit parametre: isteğe bağlı sorgu parametresi
                        sorgu_alanlari.insert(p.ad.clone(), b.alan(yr.0, &p.tip, &p.anotasyonlar, m.satir, sinif_dogrulamasi, 0));
                        continue;
                    }
                    // @ModelAttribute ya da anotasyonsuz nesne: alanları sorgu (ve form) parametrelerinden bağlanır
                    if let Some(r) = b.bul(&ic_tip, yr.0).filter(|r| matches!(b.sinif(*r).tur, "class" | "record")) {
                        let mut m2 = BTreeMap::new();
                        b.alanlar(r, dogrulama, 0, &mut m2);
                        sorgu_alanlari.extend(m2);
                    }
                }
                if !form.is_empty() {
                    govde.push(Girdi {
                        ad: None,
                        tip: Some("multipart".into()),
                        sema: Some(Sema { adaylar: vec![], dizi: false, dto: "form".into(), dosya: Some(b.goreli(yr.0)), bulunamadi: false, kalitim: None, alanlar: form }),
                    });
                }
                let sorgu = if sorgu_alanlari.is_empty() {
                    vec![]
                } else {
                    vec![Girdi {
                        ad: None,
                        tip: None,
                        sema: Some(Sema {
                            adaylar: vec![],
                            dizi: false,
                            dto: "query".into(),
                            dosya: Some(b.goreli(yr.0)),
                            bulunamadi: false,
                            kalitim: None,
                            alanlar: sorgu_alanlari,
                        }),
                    }]
                };
                let durum = anotasyon(&m.anotasyonlar, "ResponseStatus")
                    .or_else(|| anotasyon(&km.anotasyonlar, "ResponseStatus"))
                    .and_then(|a| a.deger(&["value", "code"]))
                    .and_then(basari_kodu);
                let yanit = (rest || var_mi(&m.anotasyonlar, "ResponseBody")).then(|| b.yanit(yr.0, m, durum)).flatten();
                let operasyon = anotasyon(&m.anotasyonlar, "Operation").or_else(|| anotasyon(&km.anotasyonlar, "Operation"));
                let ozet = operasyon
                    .and_then(|o| o.deger(&["summary"]))
                    .and_then(java_metni)
                    .or_else(|| anotasyon(&km.anotasyonlar, "ApiOperation").and_then(|o| o.deger(&["value"])).and_then(java_metni))
                    .or_else(|| m.belge.clone())
                    .or_else(|| km.belge.clone());
                let kalkmis = var_mi(&m.anotasyonlar, "Deprecated")
                    || var_mi(&km.anotasyonlar, "Deprecated")
                    || operasyon.and_then(|o| o.deger(&["deprecated"])).is_some_and(|v| v.trim() == "true");
                match operasyon.and_then(|o| o.deger(&["operationId"])).and_then(java_metni) {
                    Some(k) => {
                        islem_kimlikleri.insert(konum.clone(), k);
                    }
                    None if springdoc => {
                        *ad_sayilari.entry(m.ad.clone()).or_default() += 1;
                        varsayilan_kimlikler.push((konum.clone(), m.ad.clone()));
                    }
                    None => {}
                }
                let veri = b.veri_bagi(yr, &modeller);
                if let std::collections::btree_map::Entry::Vacant(e) = dna_haritasi.entry(konum.clone()) {
                    let mut v = b.dna_zinciri(yr, &zengin);
                    // Yumuşak silinen varlığı silen işlem geri alınabilir
                    if v["yumusak_silme"].is_null() {
                        let silinen: Vec<String> = v["silinen"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
                        if let Some(k) = silinen.iter().find_map(|x| yumusak.get(x)) {
                            v["yumusak_silme"] = json!(k);
                        }
                    }
                    if !crate::dna::bos(&v) {
                        e.insert(v);
                    }
                }
                if !notlar.is_empty() {
                    let l = denetlenemeyen.entry(konum.clone()).or_default();
                    for n in notlar {
                        if !l.contains(&n) {
                            l.push(n);
                        }
                    }
                }
                kapi_routerlari.insert(konum.clone(), sinif_id.clone());
                let yontem_listesi: Vec<&Yontem> = if kaynak == yr { vec![m] } else { vec![m, km] };
                for onek in &onekler {
                    for yol in &yollar {
                        let uygulama_yolu = duzgun(&yol_birlestir(onek, yol));
                        let tam = if genel_onek.is_empty() { uygulama_yolu.clone() } else { duzgun(&yol_birlestir(&genel_onek, &uygulama_yolu)) };
                        let sezgi = sezgi_yolu(&tam);
                        for yontem in &yontemler {
                            let karar = b.giris_karari(&g, yontem, &sezgi_yolu(&uygulama_yolu), sr, &yontem_listesi);
                            if kosul.is_some() {
                                kosullu_kapilar.push(format!("{yontem} {sezgi}"));
                            }
                            kapilar.push(Kapi {
                                id: format!("{yontem} {sezgi}"),
                                yontem: yontem.clone(),
                                yol: sezgi.clone(),
                                kontrolcu: s.ad.clone(),
                                fonksiyon: m.ad.clone(),
                                dosya: b.goreli(yr.0),
                                satir: m.satir,
                                giris: karar.giris.into(),
                                guardlar: karar.guardlar,
                                roller: karar.roller,
                                roller_kaniti: karar.roller_kaniti,
                                ozet: ozet.clone(),
                                govde: govde.clone(),
                                sorgu: sorgu.clone(),
                                parametreler: yol_param.clone(),
                                kullanimdan_kalkmis: kalkmis,
                                yerine: None,
                                surum: None,
                                uygulama: None,
                                ana: true,
                                giris_kaniti: karar.kanit.into(),
                                veri: veri.clone(),
                                yanit: yanit.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    for (konum, ad) in varsayilan_kimlikler {
        if ad_sayilari.get(&ad) == Some(&1) {
            islem_kimlikleri.insert(konum, ad);
        }
    }
    kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
    let genel_guardlar: Vec<String> = g.zincirler.iter().map(|z| format!("SecurityFilterChain {}", z.ad)).collect();
    (
        kapilar,
        json!({
            "genel_onek": genel_onek, "genel_guardlar": genel_guardlar,
            "ana_uygulama": ana.map(|(di, s)| format!("{}.{}", dosyalar[di].paket, s.ad).trim_start_matches('.').to_string()),
            "kosullu_kapilar": kosullu_kapilar, "denetlenemeyen": denetlenemeyen, "dna": dna_haritasi,
            "kaskadlar": kaskadlar(&b), "olaylar": olaylar(&b, &zengin),
            "routerlar": routerlar, "kapi_routerlari": kapi_routerlari,
            "veri_modelleri": veri_modelleri(&b), "islem_kimlikleri": islem_kimlikleri,
        }),
    )
}

/// Jakarta Validation ve Hibernate Validator kısıtları
const JAKARTA: [&str; 24] = [
    "NotNull",
    "NotBlank",
    "NotEmpty",
    "Size",
    "Min",
    "Max",
    "DecimalMin",
    "DecimalMax",
    "Positive",
    "PositiveOrZero",
    "Negative",
    "NegativeOrZero",
    "Email",
    "Pattern",
    "Past",
    "PastOrPresent",
    "Future",
    "FutureOrPresent",
    "Digits",
    "AssertTrue",
    "AssertFalse",
    "Length",
    "Range",
    "Valid",
];

/// Varlıklardaki kaskad silmeler: [ebeveyn, çocuk, "dosya:satır"]. Ebeveyndeki `@OneToMany` / `@OneToOne` /
/// `@ManyToMany` (`cascade = ALL | REMOVE` ya da `orphanRemoval = true`) ve çocuktaki `@ManyToOne` +
/// `@OnDelete(action = CASCADE)`
pub(crate) fn kaskadlar(b: &Baglam) -> Vec<Value> {
    let mut v: Vec<Value> = vec![];
    let mut ekle = |e: String, c: String, k: String| {
        if e != c && !v.iter().any(|x| x[0] == e.as_str() && x[1] == c.as_str()) {
            v.push(json!([e, c, k]));
        }
    };
    for (di, d) in b.dosyalar.iter().enumerate() {
        for s in d.siniflar.iter().filter(|s| varlik_mi(s)) {
            for a in &s.alanlar {
                let konum = format!("{}:{}", b.goreli(di), a.satir);
                for x in &a.anotasyonlar {
                    match x.ad.as_str() {
                        "OneToMany" | "OneToOne" | "ManyToMany" => {
                            let cascade = x.deger(&["cascade"]).unwrap_or("");
                            let yetim = x.deger(&["orphanRemoval"]).is_some_and(|v| v.trim() == "true");
                            if cascade.contains("ALL") || cascade.contains("REMOVE") || yetim {
                                let (ic, _) = sarmalayicisiz(&a.tip);
                                let hedef = x.deger(&["targetEntity"]).map(|t| temel_ad(t.trim().trim_end_matches(".class"))).unwrap_or_else(|| temel_ad(&ic));
                                ekle(s.ad.clone(), hedef, konum.clone());
                            }
                        }
                        "OnDelete" if x.deger(&["action"]).is_some_and(|v| v.contains("CASCADE")) => ekle(temel_ad(&a.tip), s.ad.clone(), konum.clone()),
                        _ => {}
                    }
                }
            }
        }
    }
    v
}

/// Tablo modelleri: ad, gerçek tablo adı (`@Table(name)`, `@Entity(name)`, yoksa Spring Boot'un varsayılanı), sütunlar
pub(crate) fn veri_modelleri(b: &Baglam) -> Value {
    let mut v = vec![];
    for (di, d) in b.dosyalar.iter().enumerate() {
        for s in d.siniflar.iter().filter(|s| varlik_mi(s)) {
            let acik = ["Table", "Document"]
                .iter()
                .filter_map(|a| anotasyon(&s.anotasyonlar, a))
                .find_map(|a| a.deger(&["name", "value", "collection"]).and_then(java_metni))
                .filter(|x| !x.is_empty());
            let varlik_adi = anotasyon(&s.anotasyonlar, "Entity").and_then(|a| a.deger(&["name"])).and_then(java_metni);
            // Exposed: `Table("ad")`, yoksa nesne adı (sondaki "Table" atılır)
            let exposed = anotasyon(&s.anotasyonlar, "ExposedTable").map(|a| a.deger(&["name"]).and_then(java_metni).filter(|x| !x.is_empty()));
            let (tablo, kaynak) = match (&exposed, &acik, &varlik_adi) {
                (Some(Some(t)), ..) => (t.clone(), "Exposed Table(name)"),
                (Some(None), ..) => (s.ad.strip_suffix("Table").filter(|x| !x.is_empty()).unwrap_or(&s.ad).to_string(), "Exposed varsayılanı (nesne adı)"),
                (None, Some(t), _) => (t.clone(), "@Table"),
                (None, None, Some(e)) => (yilan(e), "@Entity(name)"),
                (None, None, None) => (yilan(&s.ad), "varsayılan (sınıf adı yılan_biçiminde)"),
            };
            let alanlar: Vec<Value> = s
                .alanlar
                .iter()
                .filter(|a| !a.statik && !var_mi(&a.anotasyonlar, "Transient") && !a.anotasyonlar.iter().any(|x| matches!(x.ad.as_str(), "OneToMany" | "ManyToMany")))
                .map(|a| {
                    let sutun = anotasyon(&a.anotasyonlar, "Column")
                        .or_else(|| anotasyon(&a.anotasyonlar, "JoinColumn"))
                        .and_then(|c| c.deger(&["name"]))
                        .and_then(java_metni)
                        .unwrap_or_else(|| {
                            if a.anotasyonlar.iter().any(|x| matches!(x.ad.as_str(), "ManyToOne" | "OneToOne")) {
                                format!("{}_id", yilan(&a.ad))
                            } else {
                                yilan(&a.ad)
                            }
                        });
                    json!({ "ad": a.ad, "tip": anotasyonsuz(&a.tip), "satir": a.satir, "sutun": sutun })
                })
                .collect();
            v.push(json!({ "ad": s.ad, "tablo": tablo, "tablo_kaynagi": kaynak, "dosya": b.goreli(di), "satir": s.satir, "alanlar": alanlar }));
        }
    }
    json!(v)
}

/// Dahili olay ve iş düğümleri: `@EventListener` / `@TransactionalEventListener` (olay tipi), `@KafkaListener`
/// (konu), `@JmsListener` / `@RabbitListener` (hedef), `@Scheduled` ve `@Async` yöntemleri (iş). Her düğüm:
/// dinleyicileri ve dinleyicinin zincirinden tetikledikleri ve sildikleri
pub(crate) fn olaylar(b: &Baglam, zengin: &HashMap<YRef, GovdeDna>) -> BTreeMap<String, Value> {
    let mut v: BTreeMap<String, Value> = BTreeMap::new();
    for (di, d) in b.dosyalar.iter().enumerate() {
        for (si, s) in d.siniflar.iter().enumerate() {
            for (yi, y) in s.yontemler.iter().enumerate() {
                if !y.govdeli {
                    continue;
                }
                let mut dugumler: Vec<(&str, String)> = vec![];
                for a in &y.anotasyonlar {
                    match a.ad.as_str() {
                        "EventListener" | "TransactionalEventListener" => {
                            let mut adlar: Vec<String> = a
                                .deger(&["value", "classes"])
                                .map(|x| x.trim_matches(['{', '}']).split(',').map(|c| temel_ad(c.trim().trim_end_matches(".class"))).filter(|c| !c.is_empty()).collect())
                                .unwrap_or_default();
                            if adlar.is_empty() {
                                adlar.extend(y.parametreler.first().map(|p| temel_ad(&p.tip)));
                            }
                            dugumler.extend(adlar.into_iter().map(|x| ("event", x)));
                        }
                        "KafkaListener" | "JmsListener" | "RabbitListener" => {
                            let anahtar: &[&str] = match a.ad.as_str() {
                                "KafkaListener" => &["topics", "value"],
                                "JmsListener" => &["destination"],
                                _ => &["queues", "value"],
                            };
                            if let Some(adlar) = a.deger(anahtar).and_then(|x| b.metinler(di, Some((di, si)), x, 0)) {
                                dugumler.extend(adlar.into_iter().filter(|x| !x.starts_with("${") && !x.starts_with("#{")).map(|x| ("event", x)));
                            }
                        }
                        "Scheduled" | "Async" => dugumler.push(("job", format!("{}.{}", s.ad, y.ad))),
                        _ => {}
                    }
                }
                if dugumler.is_empty() {
                    continue;
                }
                let ozet = b.dna_zinciri((di, si, yi), zengin);
                for (tur, ad) in dugumler {
                    let dugum = v.entry(format!("{tur}:{ad}")).or_insert_with(|| json!({ "tur": tur, "ad": ad, "dinleyiciler": [], "tetikler": [], "silinen": [] }));
                    let dinleyici = json!({ "fonksiyon": format!("{}.{}", s.ad, y.ad), "kanit": format!("{}:{}", b.goreli(di), y.satir) });
                    if let Some(l) = dugum["dinleyiciler"].as_array_mut() {
                        if !l.contains(&dinleyici) {
                            l.push(dinleyici);
                        }
                    }
                    for alan in ["tetikler", "silinen"] {
                        let ekler = ozet[alan].as_array().cloned().unwrap_or_default();
                        if let Some(l) = dugum[alan].as_array_mut() {
                            for e in ekler {
                                // Düğüm kendini tetiklemez (@Async yöntemin kendi zinciri)
                                if e["tur"] == tur && e["ad"] == ad.as_str() {
                                    continue;
                                }
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

/// Keşif, paralel ayrıştırma ve Spring adaptörü (Java ve Kotlin kaynakları). Test kaynakları (`src/test`) ve derleme
/// çıktıları okunmaz
pub(crate) fn topla(kok: &Path, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<Dosya>>, t0: Instant) -> crate::Toplam {
    let mut yollar = vec![];
    let (mut atlanan, mut buyuk) = (0usize, vec![]);
    let mut ayar_dosyalari: Vec<PathBuf> = vec![];
    let mut yapi_dosyalari: Vec<PathBuf> = vec![];
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
            ["target", "build", "out", ".gradle", ".mvn", "node_modules", "test", "tests", "testFixtures", "integrationTest"].contains(&c.as_os_str().to_str().unwrap_or(""))
        }) {
            atlanan += 1;
            continue;
        }
        let ad = p.file_name().and_then(|a| a.to_str()).unwrap_or("");
        if matches!(ad, "application.properties" | "application.yml" | "application.yaml") {
            ayar_dosyalari.push(p.to_path_buf());
            continue;
        }
        if matches!(ad, "pom.xml" | "build.gradle" | "build.gradle.kts") {
            yapi_dosyalari.push(p.to_path_buf());
            continue;
        }
        if p.extension().and_then(|e| e.to_str()).is_some_and(|e| JAVA_UZANTILARI.contains(&e)) {
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
    let (sonuclar, mut okunan, ayristirilan) = crate::kesif::ayristir_hepsi(&yollar, onbellek, |p, b| {
        let m = std::str::from_utf8(&b).ok()?;
        if p.extension().is_some_and(|e| e == "kt") {
            crate::kotlin::ayristir_metin(p, m, false).map(|d| d.model)
        } else {
            ayristir_metin(p, m)
        }
    });
    let dosyalar: Vec<Dosya> = sonuclar.into_iter().map(|(_, d)| d).collect();
    let t_ayristirma = t0.elapsed();
    // Yapılandırma: kök klasöre en yakın application.* önce (çok modüllü projede uygulamanın kendi ayarı)
    ayar_dosyalari.sort_by_key(|p| (p.components().count(), p.clone()));
    let mut oku = |p: &PathBuf| -> Option<String> {
        let m = guvenli::oku(p)?;
        okunan.push((p.clone(), crate::kesif::icerik_ozeti(m.as_bytes())));
        Some(m)
    };
    let ayarlar: Vec<(String, String)> = ayar_dosyalari.iter().filter_map(|p| Some((p.file_name()?.to_string_lossy().to_string(), oku(p)?))).collect();
    let yapi_metinleri: Vec<String> = yapi_dosyalari.iter().filter_map(&mut oku).collect();
    let (kapilar, bilgi) = kapilar(kok, &dosyalar, &ayarlar, &yapi_metinleri);
    let t_toplam = t0.elapsed();
    crate::Toplam {
        kapilar,
        bilgi,
        adaptor: format!("spring@{SURUM}"),
        dosya_sayisi: dosyalar.len(),
        satir: dosyalar.iter().map(|d| d.satir_sayisi).sum(),
        kismi: dosyalar.iter().filter(|d| !d.hatali_satirlar.is_empty()).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        atlanan,
        buyuk,
        ek_paketler: vec![],
        veri_dosyalari: dosyalar.iter().filter(|d| d.siniflar.iter().any(varlik_mi)).map(|d| guvenli::goreli(kok, &d.yol)).collect(),
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}

/// Adaptörün okuduğu uzantılar (Java ve Kotlin; Ktor adaptörü de `.kt` okur)
pub(crate) const JAVA_UZANTILARI: &[&str] = &["java", "kt"];

/// Haritayı etkileyen Spring yapılandırma dosyaları (izleme ve bayatlık)
pub(crate) const YAPILANDIRMA_DOSYALARI: &[&str] = &["pom.xml", "build.gradle", "build.gradle.kts", "application.properties", "application.yml", "application.yaml"];
