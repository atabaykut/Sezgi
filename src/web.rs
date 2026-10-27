//! Ön yüz kaynak adaptörü (P2.1): React / Next ön yüzünün kaynağını tree-sitter ile okur ve şu zinciri kurar:
//! ekran (yönlendirici) → UI eylemi (rol + erişilebilir ad; REA tarayıcı senaryosunun locator anahtarıyla aynı) →
//! istemci çağrısı (üretilmiş OpenAPI istemcisi, axios, fetch, TanStack Query, RTK Query) → API ucu (yöntem + yol şablonu).
//!
//! Güven (Benlik güven ölçeği certain > observed > inferred > unknown; kaynak kod tavanı `certain`, yalnızca tek anlamlı çözümlemede):
//! - İstemci çağrısı → API (`CALLS`): yöntem açık (çağrı adı, `method:` ya da kütüphanenin belgelenmiş varsayılanı),
//!   yol sabit (metin ya da çözülen sabit) ve tek bir uç birebir eşleşiyorsa `certain`; değilse `inferred`.
//! - UI eylemi → istemci çağrısı (`TRIGGERS`) her zaman `inferred`: olay işleyicisi koşullu olabilir (doğrulama,
//!   onay kutusu), React olayı çerçevenin dağıtıcısından geçer, kanca adları (`useXMutation`) ad eşlemesiyle çözülür.
//! - Ekran → UI eylemi (`DISPLAYS`) her zaman `inferred`: içe aktarma grafiğinden ulaşılabilirlik; koşullu
//!   gösterim ve paylaşılan bileşenler ayırt edilmez.
//!
//! Çağrı bulunamaması "çağrı yok" demek değildir; bulunamayan bağlar yazılmaz, sayıları özetteki `istatistik`te durur.
//!
//! Çatılar: React / Next / Preact (JSX), Vue ve Svelte (betik blokları TypeScript gramerinde, şablonlar şablon
//! okuyucusuyla), Angular (`Routes` dizileri, tembel `loadChildren` / `loadComponent`, `canActivate` koruyucuları,
//! `templateUrl` ve satır içi şablonlar, `HttpClient` servisleri), Remix / React Router v7 çatı kipi (`app/routes`
//! dosya rotaları, `app/routes.ts`, loader / action, `<Form>`, `useFetcher`) ve Solid / SolidStart (`@solidjs/router`,
//! `src/routes` dosya rotaları, `createResource` / `createAsync`, `query` / `action` sarmalları, "use server").
//! Koruyucudan çıkan giriş gereksinimi addan ve kaynak metninden tahmindir: her zaman `inferred`.

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

use crate::guvenli;

/// Ön yüz dosyasının uzantıları (TS/TSX/JS/JSX); Vue ve Svelte tek dosya bileşenlerinde `<script>` blokları ve
/// şablon okunur. Angular şablonları (`.html`) yalnızca Angular uygulamasında okunur
const UZANTILAR: [&str; 8] = ["ts", "tsx", "js", "jsx", "mjs", "cjs", "vue", "svelte"];
/// Taranmayan klasörler
const ATLA: [&str; 12] = ["node_modules", "dist", "build", ".next", "out", "coverage", "__tests__", "test", "tests", "e2e", "public", ".turbo"];
/// İstemcinin yöntem adları (`axios.post`, `client.get`)
const YONTEMLER: [&str; 6] = ["get", "post", "put", "patch", "delete", "head"];
/// UI eylemi olay öznitelikleri
const OLAYLAR: [&str; 6] = ["onClick", "onSubmit", "onPress", "onDoubleClick", "onSelect", "onChange"];
/// Ekran yüklemesinde çağrılan sorgu kancaları (RTK'nın `useXQuery` kancaları ada göre ayrıca tanınır)
const SORGU_KANCALARI: [&str; 5] = ["useQuery", "useSuspenseQuery", "useInfiniteQuery", "useSuspenseInfiniteQuery", "useSWR"];
/// Solid'de ekran yüklemesinde veri getiren ilkel ve kancalar
const SOLID_YUKLEME: [&str; 4] = ["createResource", "createAsync", "createQuery", "createInfiniteQuery"];
/// Çözümlemede en fazla derinlik (işleyici → yerel fonksiyon → kanca → başka dosya ...)
const EN_DERIN: usize = 8;

/// Yol parçası: metin, ad (sabite çözülecek) ya da bilinmeyen (yol parametresi ya da taban adres)
#[derive(Clone, Debug)]
enum Parca {
    Metin(String),
    Ad(String),
    Bilinmeyen(String),
}

/// Bulunan istemci çağrısı (çözülmemiş yol ile)
#[derive(Clone, Debug)]
struct HamCagri {
    /// Çağrının içinde bulunduğu fonksiyon, sınıf metodu ya da RTK uç adı
    sahip: Option<String>,
    yontem: String,
    /// Yöntem açıkça yazılmış ya da kütüphanenin belgelenmiş varsayılanı mı (fetch, axios, RTK: GET)
    yontem_kesin: bool,
    yol: Vec<Parca>,
    satir: usize,
    taniyici: &'static str,
    /// Çağrının dosyadaki bayt konumu (satır içi olay işleyicisinin içindeki çağrılar için)
    bayt: usize,
}

/// Olay özniteliğiyle kurulan UI eylemi (ya da ekran yüklemesi)
#[derive(Clone, Debug)]
struct HamEylem {
    rol: String,
    ad: String,
    ad_kaynagi: &'static str,
    olay: String,
    satir: usize,
    /// Başvurular; `@rota:<action|loader>:<yol>` rota modülünün (Remix / React Router) action ya da loader'ıdır
    basvurular: Vec<String>,
    /// İşleyici ifadesinin bayt aralığı: içindeki istemci çağrıları doğrudan eylemindir (`onClick={() => api.post(...)}`)
    aralik: Option<(usize, usize)>,
}

/// Nesne biçimli rota (Angular `Routes`, Vue Router, React Router / Solid rota nesneleri)
#[derive(Clone, Debug, Default)]
struct NesneRota {
    /// Yazıldığı gibi yol (göreli ya da mutlak); yoksa üst rotanın yolu (index ya da düzen rotası)
    yol: Option<String>,
    satir: usize,
    /// Bileşen: yerel ad ya da tembel yüklenen modül (`import:./x`)
    bilesen: Option<String>,
    /// Koruyucular (canActivate, canMatch, canActivateChild, canLoad, Vue `beforeEnter` / `meta.requiresAuth`): (ad, kaynak metni)
    korumalar: Vec<(String, String)>,
    /// Tembel alt rotalar (`loadChildren: () => import('./x').then(m => m.Y)`): (modül belirteci, dışa aktarılan ad)
    tembel: Option<(String, Option<String>)>,
    cocuklar: Vec<NesneRota>,
    /// Yönlendirme rotası (`redirectTo`): ekran değildir
    yonlendirme: bool,
}

/// Rota modülü çağrısı (Remix / React Router: `fetcher.submit`, `useSubmit()`, `fetcher.load`)
#[derive(Clone, Debug)]
struct RotaCagrisi {
    sahip: Option<String>,
    bayt: usize,
    /// Hedef rotanın yolu; boşsa çağrının bulunduğu rota modülü
    hedef: String,
    /// "action" ya da "loader"
    tur: &'static str,
}

/// Bir ön yüz dosyasından okunanlar
#[derive(Default, Debug, Clone)]
pub(crate) struct Dosya {
    yol: PathBuf,
    goreli: String,
    /// Yerel ad -> (modül belirteci, dışa aktarılan ad; "default" ya da "*")
    iceri: HashMap<String, (String, String)>,
    /// `export * from 'x'`
    yildiz: Vec<String>,
    /// `export { a as b } from 'x'`: b -> (x, a)
    yeniden: HashMap<String, (String, String)>,
    varsayilan: Option<String>,
    sabitler: HashMap<String, Vec<Parca>>,
    /// Fonksiyon adı (sınıf metodu "Sinif.metot") -> gövdesindeki başvurular
    fonksiyonlar: HashMap<String, Vec<String>>,
    /// Kanca bağı: `const m = useMutation(...)`, `const [f] = useXMutation()` -> başvurular
    baglamalar: HashMap<String, Vec<String>>,
    cagrilar: Vec<HamCagri>,
    eylemler: Vec<HamEylem>,
    /// Bu dosyada tanımlanan ekranlar: (yol, satır, bileşenin yerel adı)
    rotalar: Vec<(String, usize, Option<String>)>,
    /// Ekran geçişleri (yönlendirme): (hedef yol, satır, tanıyıcı)
    gecisler: Vec<(String, usize, &'static str)>,
    /// Yol listesi öğesi gibi duran nesne özellikleri (`{ path: "/items" }`): dosyada ya da içe aktardığı dosyada hedefi
    /// değişken bir bağlantı varsa (`<Link to={oge.path}>`) geçiş hedefi sayılır
    yol_ozellikleri: Vec<(String, usize)>,
    /// Hedefi değişken olan bağlantı ya da yönlendirme var mı
    dinamik_gecis: bool,
    /// Nesne biçimli rota dizileri: (değişken adı, rotalar)
    rota_dizileri: Vec<(Option<String>, Vec<NesneRota>)>,
    /// Sınıf alanlarının tipleri (Angular bağımlılık enjeksiyonu: kurucu parametresi, `inject(Tip)`): "Bilesen.alan" -> "Tip"
    alan_tipleri: HashMap<String, String>,
    /// Angular bileşen sınıfları ve dış şablonları: (sınıf, templateUrl)
    bilesen_siniflari: BTreeSet<String>,
    sablon_url: Vec<(String, String)>,
    /// Bileşen seçicileri (`selector: 'app-header'`) ve şablonda kullanılan özel öğeler (`<app-header>`)
    seciciler: Vec<String>,
    etiketler: BTreeSet<String>,
    /// Uygulamanın kök bileşeni (Angular `bootstrapApplication` / NgModule `bootstrap`, Vue `createApp`): her ekranda görünür
    kok_bilesenler: Vec<String>,
    /// Hedefi göreli bağlantılar (Angular `routerLink="detail"`): bağlantıyı gösteren ekranın yoluna göre çözülür
    goreli_gecisler: Vec<(String, usize, &'static str)>,
    rota_cagrilari: Vec<RotaCagrisi>,
    /// React Router v7 rota yapılandırması (`app/routes.ts`): (yol, satır, modül, düzen modülleri)
    yapilandirma_rotalari: Vec<(String, usize, String, Vec<String>)>,
    /// `flatRoutes()`: dosya tabanlı rotalar yapılandırmayla birlikte kullanılıyor
    duz_rotalar: bool,
    /// "use server" yönergesi taşıyan fonksiyonlar (SolidStart / React sunucu fonksiyonları)
    sunucu_fonksiyonu: usize,
}

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

/// Metin sabiti: 'x', "x" ya da değişkensiz `x`
fn metin_sabiti(n: Node, k: &[u8]) -> Option<String> {
    match n.kind() {
        "string" => Some(metin(n, k).trim_matches(['\'', '"']).to_string()),
        "template_string" if cocuklar(n).iter().all(|c| c.kind() != "template_substitution") => Some(metin(n, k).trim_matches('`').to_string()),
        _ => None,
    }
}

/// Nesne literalinde anahtarın değeri
fn nesne_degeri<'t>(n: Node<'t>, k: &[u8], anahtar: &str) -> Option<Node<'t>> {
    let n = if n.kind() == "parenthesized_expression" { n.named_child(0)? } else { n };
    if n.kind() != "object" {
        return None;
    }
    cocuklar(n).into_iter().find_map(|c| {
        let ad = c.child_by_field_name("key").map(|a| metin(a, k).trim_matches(['\'', '"']).to_string());
        (c.kind() == "pair" && ad.as_deref() == Some(anahtar)).then(|| c.child_by_field_name("value")).flatten()
    })
}

/// İfadeden yol parçaları: metin, şablon, ad, `a + b`
fn yol_parcalari(n: Node, k: &[u8]) -> Option<Vec<Parca>> {
    match n.kind() {
        "string" => Some(vec![Parca::Metin(metin_sabiti(n, k)?)]),
        "template_string" => {
            let mut v = vec![];
            let govde = metin(n, k);
            let bas = n.start_byte();
            let mut son = bas + 1;
            for c in cocuklar(n).into_iter().filter(|c| c.kind() == "template_substitution") {
                v.push(Parca::Metin(govde[son - bas..c.start_byte() - bas].to_string()));
                let ic = c.named_child(0);
                v.push(match ic {
                    Some(i) if i.kind() == "identifier" => Parca::Ad(metin(i, k)),
                    // `${encodeURIComponent(id)}` -> {id}; sorgu dizgisi (`${QS.query(...)}`, `?${params}`) yolun parçası değildir
                    Some(i) if i.kind() == "call_expression" => {
                        let f = i.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default();
                        let son = f.rsplit('.').next().unwrap_or(&f).to_lowercase();
                        if son.contains("query") || son.contains("stringify") || son == "qs" {
                            Parca::Metin(String::new())
                        } else {
                            let arg = i.child_by_field_name("arguments").and_then(|a| a.named_child(0)).map(|a| metin(a, k)).unwrap_or_default();
                            Parca::Bilinmeyen(arg.rsplit('.').next().unwrap_or("").to_string())
                        }
                    }
                    // `${environment.apiUrl}`, `${this.taban}`: sabite çözülür; çözülmezse son ad (`${data.productId}` -> {productId})
                    Some(i) if i.kind() == "member_expression" && noktali_ad(i, k).is_some_and(|a| !a.starts_with('.')) => Parca::Ad(noktali_ad(i, k).unwrap_or_default()),
                    Some(i) => Parca::Bilinmeyen(metin(i, k).rsplit('.').next().unwrap_or("").to_string()),
                    None => Parca::Bilinmeyen(String::new()),
                });
                son = c.end_byte();
            }
            v.push(Parca::Metin(govde[son - bas..govde.len() - 1].to_string()));
            Some(v)
        }
        "identifier" => Some(vec![Parca::Ad(metin(n, k))]),
        // `environment.apiUrl`, `this.heroesUrl` (sınıf içinde sınıf adıyla yazılır)
        "member_expression" => noktali_ad(n, k).filter(|a| !a.starts_with('.')).map(|a| vec![Parca::Ad(a)]),
        "binary_expression" if n.child_by_field_name("operator").map(|o| metin(o, k)).as_deref() == Some("+") => {
            let mut v = yol_parcalari(n.child_by_field_name("left")?, k)?;
            v.extend(yol_parcalari(n.child_by_field_name("right")?, k)?);
            Some(v)
        }
        "parenthesized_expression" | "as_expression" | "satisfies_expression" | "non_null_expression" => yol_parcalari(n.named_child(0)?, k),
        _ => None,
    }
}

/// Çağrının adı: `a`, `a.b.c`; çağrı ya da parantezle başlayan zincirde son ad (`(x ?? client).post` -> ".post")
fn noktali_ad(n: Node, k: &[u8]) -> Option<String> {
    match n.kind() {
        "identifier" | "property_identifier" | "this" => Some(metin(n, k)),
        "member_expression" => {
            let ozellik = metin(n.child_by_field_name("property")?, k);
            match noktali_ad(n.child_by_field_name("object")?, k) {
                Some(o) => Some(format!("{o}.{ozellik}")),
                None => Some(format!(".{ozellik}")),
            }
        }
        "non_null_expression" => noktali_ad(n.named_child(0)?, k),
        _ => None,
    }
}

/// Bir düğümdeki bütün başvurular: adlar ve noktalı adlar (çözümleme bilinmeyenleri eler)
fn basvurular(n: Node, k: &[u8]) -> Vec<String> {
    let mut v = BTreeSet::new();
    let mut yigin = vec![n];
    while let Some(x) = yigin.pop() {
        match x.kind() {
            "identifier" | "shorthand_property_identifier" => {
                v.insert(metin(x, k));
            }
            "member_expression" => {
                if let Some(a) = noktali_ad(x, k).filter(|a| !a.starts_with('.')) {
                    v.insert(a);
                }
            }
            // Tip argümanları ve ek açıklamaları başvuru değildir (`useLoaderData<typeof loader>()`)
            "type_arguments" | "type_annotation" => continue,
            _ => {}
        }
        yigin.extend(cocuklar(x));
    }
    v.into_iter().collect()
}

/// Yönlendirme ve yerel durum çağrıları: yol metni taşısalar da API çağrısı değildir
fn yonlendirme_mi(ad: &str) -> bool {
    let son = ad.rsplit('.').next().unwrap_or(ad);
    let alici = ad.split('.').next().unwrap_or("");
    matches!(son, "navigate" | "push" | "replace" | "redirect" | "prefetch" | "t" | "i18n" | "translate" | "setItem" | "getItem" | "removeItem")
        || matches!(alici, "router" | "app" | "server" | "fastify" | "r" | "express" | "localStorage" | "sessionStorage" | "searchParams" | "params" | "map" | "cache")
        || alici.ends_with("Router")
}

/// Bir çağrının istemci çağrısı olup olmadığı; olduğunda yöntem, yol ve tanıyıcı
fn istemci_cagrisi(c: Node, k: &[u8]) -> Option<(String, bool, Vec<Parca>, &'static str)> {
    let f = c.child_by_field_name("function")?;
    let ad = noktali_ad(f, k).unwrap_or_default();
    let args: Vec<Node> = c.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
    let ilk = *args.first()?;
    if yonlendirme_mi(&ad) {
        return None;
    }
    let son = ad.rsplit('.').next().unwrap_or("");
    // fetch(url, { method })
    if ad == "fetch" || ad.ends_with(".fetch") {
        let yontem = args.get(1).and_then(|o| nesne_degeri(*o, k, "method")).and_then(|m| metin_sabiti(m, k)).unwrap_or_else(|| "GET".into());
        return Some((yontem.to_uppercase(), true, yol_parcalari(ilk, k)?, "fetch"));
    }
    // oazapfts (üretilmiş istemci): `oazapfts.fetchJson("/assets", oazapfts.json({ ...opts, method: "DELETE" }))`
    if matches!(son, "fetchJson" | "fetchText" | "fetchBlob") {
        let yontem = args.get(1).and_then(|o| ic_metot(*o, k)).unwrap_or_else(|| "GET".into());
        return Some((yontem.to_uppercase(), true, yol_parcalari(ilk, k)?, "uretilmis-istemci"));
    }
    // istemci.post({ url }) (hey-api) ya da axios.post(url) / ornek.get(url)
    if ad.contains('.') && YONTEMLER.contains(&son) {
        if let Some(u) = nesne_degeri(ilk, k, "url") {
            return Some((son.to_uppercase(), true, yol_parcalari(u, k)?, "uretilmis-istemci"));
        }
        let yol = yol_parcalari(ilk, k)?;
        return Some((son.to_uppercase(), true, yol, "axios"));
    }
    // axios({ url, method }), __request(OpenAPI, { method, url }) (openapi-typescript-codegen),
    // customInstance({ url, method }) (orval), request({ url, method })
    for a in &args {
        if let (Some(u), m) = (nesne_degeri(*a, k, "url"), nesne_degeri(*a, k, "method")) {
            let yontem = m.and_then(|m| metin_sabiti(m, k));
            if yontem.is_none() && ad != "axios" {
                continue;
            }
            let taniyici = if ad == "axios" || ad.ends_with(".request") { "axios" } else { "uretilmis-istemci" };
            return Some((yontem.unwrap_or_else(|| "GET".into()).to_uppercase(), true, yol_parcalari(u, k)?, taniyici));
        }
    }
    None
}

/// Argümanın içindeki (sarmalayıcı çağrılar dahil) `method: "X"` değeri
fn ic_metot(n: Node, k: &[u8]) -> Option<String> {
    let mut yigin = vec![n];
    while let Some(x) = yigin.pop() {
        if x.kind() == "pair" && x.child_by_field_name("key").is_some_and(|a| metin(a, k).trim_matches(['\'', '"']) == "method") {
            if let Some(m) = x.child_by_field_name("value").and_then(|v| metin_sabiti(v, k)) {
                return Some(m);
            }
        }
        yigin.extend(cocuklar(x));
    }
    None
}

/// Parantez, `as`, `satisfies`, `await` sarmalının içi
fn ic_ifade(n: Node) -> Node {
    match n.kind() {
        "parenthesized_expression" | "as_expression" | "satisfies_expression" | "non_null_expression" | "await_expression" => n.named_child(0).map(ic_ifade).unwrap_or(n),
        _ => n,
    }
}

/// Dinamik içe aktarmanın (`import("./x")`) belirteci: düğümün kendisinde ya da altında
fn tembel_modul(n: Node, k: &[u8]) -> Option<String> {
    let mut yigin = vec![n];
    while let Some(x) = yigin.pop() {
        if x.kind() == "call_expression" && x.child_by_field_name("function").is_some_and(|f| f.kind() == "import") {
            return x.child_by_field_name("arguments").and_then(|a| a.named_child(0)).and_then(|a| metin_sabiti(a, k));
        }
        yigin.extend(cocuklar(x));
    }
    None
}

/// `import('./x').then(m => m.Y)`: (belirteç, Y)
fn tembel_ihrac(n: Node, k: &[u8]) -> Option<(String, Option<String>)> {
    let b = tembel_modul(n, k)?;
    let mut yigin = vec![n];
    while let Some(x) = yigin.pop() {
        if x.kind() == "call_expression" && x.child_by_field_name("function").and_then(|f| noktali_ad(f, k)).is_some_and(|f| f.ends_with(".then")) {
            let govde = x.child_by_field_name("arguments").and_then(|a| a.named_child(0)).and_then(|f| f.child_by_field_name("body"));
            if let Some(o) = govde.filter(|g| g.kind() == "member_expression").and_then(|g| g.child_by_field_name("property")) {
                return Some((b, Some(metin(o, k))));
            }
        }
        yigin.extend(cocuklar(x));
    }
    Some((b, None))
}

/// Tip ek açıklamasındaki ad: `: HeroService` -> "HeroService", `: Observable<X>` -> "Observable"
fn tip_adi(t: Node, k: &[u8]) -> Option<String> {
    let s = metin(t, k);
    let s = s.trim_start_matches(':').trim().split('<').next().unwrap_or("").trim().to_string();
    (!s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')).then_some(s)
}

/// Sınıf içindeki `this.x` başvurusu sınıfın adıyla yazılır: "this.kaydet" -> "Bilesen.kaydet"
fn sinifla(v: Vec<String>, sinif: Option<&str>) -> Vec<String> {
    let Some(s) = sinif else { return v };
    v.into_iter().map(|r| r.strip_prefix("this.").map(|a| format!("{s}.{a}")).unwrap_or(r)).collect()
}

fn sinifla_yol(v: Vec<Parca>, sinif: Option<&str>) -> Vec<Parca> {
    let Some(s) = sinif else { return v };
    v.into_iter()
        .map(|p| match p {
            Parca::Ad(a) if a.starts_with("this.") => Parca::Ad(format!("{s}.{}", &a["this.".len()..])),
            x => x,
        })
        .collect()
}

/// Sınıfın dekoratörleri (sınıfta ya da sarmalayan `export` deyiminde): (ad, ilk argüman)
fn dekoratorler<'t>(sinif: Node<'t>, k: &[u8]) -> Vec<(String, Option<Node<'t>>)> {
    let mut v = cocuklar(sinif);
    if let Some(u) = sinif.parent().filter(|u| u.kind() == "export_statement") {
        v.extend(cocuklar(u));
    }
    v.into_iter()
        .filter(|d| d.kind() == "decorator")
        .filter_map(|d| {
            let c = d.named_child(0)?;
            if c.kind() != "call_expression" {
                return Some((metin(c, k), None));
            }
            Some((noktali_ad(c.child_by_field_name("function")?, k)?, c.child_by_field_name("arguments").and_then(|a| a.named_child(0))))
        })
        .collect()
}

/// Angular sınıfı: `@Component` (şablon, seçici), `@NgModule` (kök bileşen) ve alan tipleri (kurucu parametreleri,
/// `alan: Tip`, `alan = inject(Tip)`)
fn angular_sinifi(n: Node, k: &[u8], d: &mut Dosya, sinif: &str) {
    for (ad, arg) in dekoratorler(n, k) {
        let Some(o) = arg else { continue };
        match ad.as_str() {
            "Component" => {
                d.bilesen_siniflari.insert(sinif.to_string());
                if let Some(u) = nesne_degeri(o, k, "templateUrl").and_then(|u| metin_sabiti(u, k)) {
                    d.sablon_url.push((sinif.to_string(), u));
                }
                if let Some(s) = nesne_degeri(o, k, "selector").and_then(|s| metin_sabiti(s, k)) {
                    let gecerli = |x: &&str| !x.is_empty() && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
                    d.seciciler.extend(s.split(',').map(str::trim).filter(gecerli).map(str::to_ascii_lowercase));
                }
                if let Some(t) = nesne_degeri(o, k, "template").filter(|t| matches!(t.kind(), "template_string" | "string")) {
                    let ham = metin(t, k);
                    if ham.len() >= 2 {
                        let s = sablon_oku(&ham[1..ham.len() - 1], t.start_position().row + 1, SablonTuru::Angular);
                        sablon_ekle(d, s, Some(sinif));
                    }
                }
            }
            "NgModule" => {
                if let Some(b) = nesne_degeri(o, k, "bootstrap").filter(|b| b.kind() == "array") {
                    d.kok_bilesenler.extend(cocuklar(b).into_iter().filter(|x| x.kind() == "identifier").map(|x| metin(x, k)));
                }
            }
            _ => {}
        }
    }
    let Some(govde) = n.child_by_field_name("body") else { return };
    for u in cocuklar(govde) {
        match u.kind() {
            "method_definition" if u.child_by_field_name("name").is_some_and(|a| metin(a, k) == "constructor") => {
                for pr in u.child_by_field_name("parameters").map(cocuklar).unwrap_or_default() {
                    if let (Some(a), Some(t)) = (pr.child_by_field_name("pattern"), pr.child_by_field_name("type")) {
                        if let Some(t) = tip_adi(t, k) {
                            d.alan_tipleri.insert(format!("{sinif}.{}", metin(a, k)), t);
                        }
                    }
                }
            }
            "public_field_definition" => {
                let Some(a) = u.child_by_field_name("name").map(|a| metin(a, k)) else { continue };
                let tip = u.child_by_field_name("type").and_then(|t| tip_adi(t, k)).or_else(|| {
                    let v = u.child_by_field_name("value")?;
                    let f = v.child_by_field_name("function").map(|f| metin(f, k));
                    (v.kind() == "call_expression" && f.as_deref() == Some("inject")).then(|| v.child_by_field_name("arguments")?.named_child(0).map(|x| metin(x, k))).flatten()
                });
                if let Some(t) = tip {
                    d.alan_tipleri.insert(format!("{sinif}.{a}"), t);
                }
            }
            _ => {}
        }
    }
}

/// Rota dizisinin tipi: Angular `Routes` / `Route[]`, React Router `RouteObject[]`, Solid `RouteDefinition[]`, Vue `RouteRecordRaw[]`
fn rota_tipi(t: &str) -> bool {
    ["Routes", "Route[]", "RouteObject", "RouteDefinition", "RouteRecordRaw"].iter().any(|x| t.contains(x))
}

/// Nesne biçimli rota dizisi
fn rota_dizisi(dizi: Node, k: &[u8], goreli: &str) -> Vec<NesneRota> {
    const ANAHTARLAR: [&str; 10] = ["path", "component", "Component", "element", "loadComponent", "loadChildren", "children", "redirectTo", "lazy", "redirect"];
    cocuklar(dizi).into_iter().filter(|o| o.kind() == "object" && ANAHTARLAR.iter().any(|a| nesne_degeri(*o, k, a).is_some())).map(|o| nesne_rota_oku(o, k, goreli)).collect()
}

fn nesne_rota_oku(o: Node, k: &[u8], goreli: &str) -> NesneRota {
    let bilesen = ["component", "Component", "element", "loadComponent", "lazy"].iter().find_map(|a| nesne_degeri(o, k, a)).and_then(|v| bilesen_adi(v, k));
    let mut korumalar = vec![];
    for a in ["canActivate", "canMatch", "canActivateChild", "canLoad", "beforeEnter"] {
        let Some(v) = nesne_degeri(o, k, a) else { continue };
        let ogeler = if v.kind() == "array" { cocuklar(v) } else { vec![v] };
        for x in ogeler {
            let ad = match x.kind() {
                "identifier" | "member_expression" => metin(x, k),
                "call_expression" => x.child_by_field_name("function").map(|f| metin(f, k)).unwrap_or_default(),
                "new_expression" => x.child_by_field_name("constructor").map(|f| metin(f, k)).unwrap_or_default(),
                _ => format!("{goreli}:{}", satir(x)),
            };
            korumalar.push((ad, metin(x, k).chars().take(300).collect()));
        }
    }
    // Vue Router: meta: { requiresAuth: true }
    if let Some(m) = nesne_degeri(o, k, "meta") {
        for a in ["requiresAuth", "requireAuth", "authRequired", "auth"] {
            if nesne_degeri(m, k, a).is_some_and(|v| metin(v, k) == "true") {
                korumalar.push((format!("meta.{a}"), format!("{a}: true")));
            }
        }
    }
    NesneRota {
        yol: nesne_degeri(o, k, "path").and_then(|p| metin_sabiti(p, k)),
        satir: satir(o),
        bilesen,
        korumalar,
        tembel: nesne_degeri(o, k, "loadChildren").and_then(|v| tembel_ihrac(v, k)),
        cocuklar: nesne_degeri(o, k, "children").filter(|c| c.kind() == "array").map(|c| rota_dizisi(c, k, goreli)).unwrap_or_default(),
        yonlendirme: nesne_degeri(o, k, "redirectTo").is_some() || nesne_degeri(o, k, "redirect").is_some(),
    }
}

/// Rota bileşeni: ad, JSX öğesi (`<X/>`) ya da tembel modül (`() => import('./x')`, `lazy(() => import('./x'))`)
fn bilesen_adi(v: Node, k: &[u8]) -> Option<String> {
    match v.kind() {
        "identifier" | "member_expression" => Some(metin(v, k)),
        "jsx_self_closing_element" => v.child_by_field_name("name").map(|n| metin(n, k)),
        "jsx_element" => v.child_by_field_name("open_tag").and_then(|o| o.child_by_field_name("name")).map(|n| metin(n, k)),
        "arrow_function" | "function_expression" | "call_expression" => tembel_modul(v, k).map(|b| format!("import:{b}")),
        _ => None,
    }
}

/// Yol birleştirme: mutlak alt yol olduğu gibi; göreli alt yol üst yola eklenir
fn yol_birlestir(ust: &str, alt: &str) -> String {
    if alt.starts_with('/') {
        return alt.to_string();
    }
    let ust = ust.trim_end_matches('/');
    if alt.is_empty() {
        return if ust.is_empty() { "/".into() } else { ust.to_string() };
    }
    format!("{ust}/{}", alt.trim_end_matches('/'))
}

/// React Router v7 rota yapılandırması (`app/routes.ts`): `route(yol, modül, [alt])`, `index(modül)`, `layout(modül, [alt])`,
/// `prefix(yol, [alt])`
fn yapilandirma(dizi: Node, k: &[u8], onek: &str, duzenler: &[String], out: &mut Vec<(String, usize, String, Vec<String>)>) {
    for el in cocuklar(dizi) {
        let c = ic_ifade(if el.kind() == "spread_element" { el.named_child(0).unwrap_or(el) } else { el });
        if c.kind() != "call_expression" {
            continue;
        }
        let f = c.child_by_field_name("function").and_then(|f| noktali_ad(f, k)).unwrap_or_default();
        let args = c.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
        let dizgi = |i: usize| args.get(i).and_then(|a| metin_sabiti(*a, k));
        let alt = args.iter().find(|a| a.kind() == "array").copied();
        match f.as_str() {
            "index" => {
                if let Some(m) = dizgi(0) {
                    out.push((yol_birlestir(onek, ""), satir(c), m, duzenler.to_vec()));
                }
            }
            "route" => {
                if let (Some(y), Some(m)) = (dizgi(0), dizgi(1)) {
                    let y = yol_birlestir(if onek.is_empty() { "/" } else { onek }, &y);
                    out.push((y.clone(), satir(c), m.clone(), duzenler.to_vec()));
                    if let Some(a) = alt {
                        let mut d = duzenler.to_vec();
                        d.push(m);
                        yapilandirma(a, k, &y, &d, out);
                    }
                }
            }
            "layout" => {
                if let (Some(m), Some(a)) = (dizgi(0), alt) {
                    let mut d = duzenler.to_vec();
                    d.push(m);
                    yapilandirma(a, k, onek, &d, out);
                }
            }
            "prefix" => {
                if let (Some(y), Some(a)) = (dizgi(0), alt) {
                    yapilandirma(a, k, &yol_birlestir(if onek.is_empty() { "/" } else { onek }, &y), duzenler, out);
                }
            }
            _ => {}
        }
    }
}

/// Rota modülü çağrısı: `fetcher.submit(veri, { method, action })` (useFetcher), `submit(veri, { method, action })`
/// (useSubmit), `fetcher.load(yol)`. Dönen: (hedef yol; boşsa aynı rota, "action" | "loader")
fn rota_cagrisi(c: Node, k: &[u8], d: &Dosya) -> Option<(String, &'static str)> {
    let ad = noktali_ad(c.child_by_field_name("function")?, k)?;
    let (alici, son) = ad.rsplit_once('.').unwrap_or(("", ad.as_str()));
    let bag = |x: &str, kanca: &str| d.baglamalar.get(x).is_some_and(|b| b.iter().any(|r| r == kanca));
    let args = cocuklar(c.child_by_field_name("arguments")?);
    match son {
        "submit" if (alici.is_empty() && bag("submit", "useSubmit")) || (!alici.is_empty() && bag(alici, "useFetcher")) => {
            let secenek = args.get(1).copied();
            let deger = |a: &str| secenek.and_then(|o| nesne_degeri(o, k, a)).and_then(|m| metin_sabiti(m, k));
            let yontem = deger("method").unwrap_or_else(|| "get".into());
            Some((deger("action").unwrap_or_default(), if yontem.eq_ignore_ascii_case("get") { "loader" } else { "action" }))
        }
        "load" if !alici.is_empty() && bag(alici, "useFetcher") => Some((args.first().and_then(|a| metin_sabiti(*a, k))?, "loader")),
        _ => None,
    }
}

/// Program düzeyindeki nesne sabitinin alanları (`export const environment = { apiUrl: '...' }`): "environment.apiUrl"
fn nesne_sabitleri(onek: &str, o: Node, k: &[u8], out: &mut HashMap<String, Vec<Parca>>, derinlik: usize) {
    for c in cocuklar(o).into_iter().filter(|c| c.kind() == "pair") {
        let (Some(a), Some(v)) = (c.child_by_field_name("key"), c.child_by_field_name("value")) else { continue };
        let ad = format!("{onek}.{}", metin(a, k).trim_matches(['\'', '"']));
        let v = ic_ifade(v);
        if v.kind() == "object" && derinlik < 3 {
            nesne_sabitleri(&ad, v, k, out, derinlik + 1);
        } else if matches!(v.kind(), "string" | "template_string" | "binary_expression") {
            if let Some(y) = yol_parcalari(v, k) {
                out.insert(ad, y);
            }
        }
    }
}

/// Vue / Svelte tek dosya bileşeni: `<script>` bloklarının dışı boşluğa çevrilir (satır sonları ve bayt konumları
/// korunur: kanıt satırları kaynak dosyadaki satırlardır)
fn betik_ayikla(kaynak: &str) -> String {
    let mut out: Vec<u8> = kaynak.bytes().map(|b| if b == b'\n' || b == b'\r' { b } else { b' ' }).collect();
    let alt = kaynak.to_ascii_lowercase();
    let mut i = 0;
    while let Some(bas) = alt[i..].find("<script").map(|x| x + i) {
        let Some(ac) = alt[bas..].find('>').map(|x| x + bas + 1) else { break };
        let Some(son) = alt[ac..].find("</script").map(|x| x + ac) else { break };
        out[ac..son].copy_from_slice(&kaynak.as_bytes()[ac..son]);
        i = son;
    }
    String::from_utf8(out).unwrap_or_default()
}

// ---- Şablon okuyucusu (Angular `.html` ve satır içi şablonlar, Vue `<template>`, Svelte işaretlemesi) ----
// Hoşgörülü bir HTML belirteç ayırıcısı: öğeler, öznitelikler ve metin. Öznitelik ifadeleri (`(click)="kaydet()"`,
// `@click="kaydet"`, `on:click={kaydet}`) TypeScript grameriyle ayrıştırılıp başvurularına ayrılır.

#[derive(Clone, Copy, PartialEq, Debug)]
enum SablonTuru {
    Angular,
    Vue,
    Svelte,
}

/// Şablondan okunanlar
#[derive(Default)]
struct Sablon {
    eylemler: Vec<HamEylem>,
    gecisler: Vec<(String, usize, &'static str)>,
    goreli: Vec<(String, usize, &'static str)>,
    dinamik: bool,
    etiketler: BTreeSet<String>,
}

struct Etiket {
    ad: String,
    oz: Vec<(String, Option<String>)>,
    bas: usize,
    kapali: bool,
}

enum Belirtec {
    Ac(Etiket),
    Kapa(String),
    Metin(String),
}

/// Kapanış etiketi olmayan HTML öğeleri
const BOS_OGELER: [&str; 14] = ["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];

/// `{` ile başlayan dengeli süslü ayraç bloğunun sonu (tırnaklar atlanır)
fn suslu_sonu(b: &[u8], mut j: usize) -> usize {
    let mut derin = 0usize;
    let mut tirnak: Option<u8> = None;
    while j < b.len() {
        let c = b[j];
        match tirnak {
            Some(q) => {
                if c == b'\\' {
                    j += 1;
                } else if c == q {
                    tirnak = None;
                }
            }
            None => match c {
                b'"' | b'\'' | b'`' => tirnak = Some(c),
                b'{' => derin += 1,
                b'}' => {
                    derin = derin.saturating_sub(1);
                    if derin == 0 {
                        return j + 1;
                    }
                }
                _ => {}
            },
        }
        j += 1;
    }
    b.len()
}

/// İşaretlemeyi belirteçlere ayırır; `<script>` ve `<style>` içerikleri ve yorumlar atlanır
fn belirtecler(m: &str) -> Vec<Belirtec> {
    let b = m.as_bytes();
    let n = b.len();
    let bosluk = |c: u8| c.is_ascii_whitespace();
    let mut v = vec![];
    let (mut i, mut metin_bas) = (0usize, 0usize);
    let metin_ekle = |v: &mut Vec<Belirtec>, a: usize, z: usize| {
        if z > a {
            v.push(Belirtec::Metin(m[a..z].to_string()));
        }
    };
    while i < n {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let sonraki = b.get(i + 1).copied().unwrap_or(0);
        if m[i..].starts_with("<!--") {
            metin_ekle(&mut v, metin_bas, i);
            i = m[i + 4..].find("-->").map_or(n, |x| i + 4 + x + 3);
            metin_bas = i;
            continue;
        }
        if sonraki == b'/' {
            metin_ekle(&mut v, metin_bas, i);
            let ad_son = m[i..].find('>').map_or(n, |x| i + x);
            v.push(Belirtec::Kapa(m[(i + 2).min(ad_son)..ad_son].trim().to_ascii_lowercase()));
            i = (ad_son + 1).min(n);
            metin_bas = i;
            continue;
        }
        if !sonraki.is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        metin_ekle(&mut v, metin_bas, i);
        let bas = i;
        let mut j = i + 1;
        while j < n && !bosluk(b[j]) && b[j] != b'>' && b[j] != b'/' {
            j += 1;
        }
        let ad = m[i + 1..j].to_string();
        let mut oz = vec![];
        let mut kapali = false;
        loop {
            while j < n && bosluk(b[j]) {
                j += 1;
            }
            if j >= n {
                break;
            }
            match b[j] {
                b'>' => {
                    j += 1;
                    break;
                }
                b'/' if b.get(j + 1) == Some(&b'>') => {
                    kapali = true;
                    j += 2;
                    break;
                }
                b'/' => {
                    j += 1;
                    continue;
                }
                b'{' => {
                    let s = j;
                    j = suslu_sonu(b, j);
                    oz.push((m[s..j].to_string(), None));
                    continue;
                }
                _ => {}
            }
            let s = j;
            while j < n && !bosluk(b[j]) && b[j] != b'=' && b[j] != b'>' && !(b[j] == b'/' && b.get(j + 1) == Some(&b'>')) {
                j += 1;
            }
            if s == j {
                j += 1;
                continue;
            }
            let oad = m[s..j].to_string();
            let mut e = j;
            while e < n && bosluk(b[e]) {
                e += 1;
            }
            if e < n && b[e] == b'=' {
                j = e + 1;
                while j < n && bosluk(b[j]) {
                    j += 1;
                }
                if j >= n {
                    oz.push((oad, None));
                    break;
                }
                let deger = match b[j] {
                    q @ (b'"' | b'\'') => {
                        let s2 = j + 1;
                        let z = m[s2..].find(q as char).map_or(n, |x| s2 + x);
                        j = (z + 1).min(n);
                        m[s2..z].to_string()
                    }
                    b'{' => {
                        let s2 = j;
                        j = suslu_sonu(b, j);
                        m[s2..j].to_string()
                    }
                    _ => {
                        let s2 = j;
                        while j < n && !bosluk(b[j]) && b[j] != b'>' {
                            j += 1;
                        }
                        m[s2..j].to_string()
                    }
                };
                oz.push((oad, Some(deger)));
            } else {
                oz.push((oad, None));
            }
        }
        let alt = ad.to_ascii_lowercase();
        v.push(Belirtec::Ac(Etiket { ad, oz, bas, kapali }));
        i = j;
        metin_bas = i;
        if !kapali && (alt == "script" || alt == "style") {
            let kapanis = format!("</{alt}");
            i = m[i..].to_ascii_lowercase().find(&kapanis).map_or(n, |x| i + x);
            metin_bas = i;
        }
    }
    metin_ekle(&mut v, metin_bas, n);
    v
}

/// Her açılış belirtecinin öğesinin bittiği belirteç (dışlayan); kapanmayan öğeler sona kadar sürer
fn oge_sonlari(v: &[Belirtec]) -> Vec<usize> {
    let mut son: Vec<usize> = (1..=v.len()).collect();
    let mut yigin: Vec<(usize, String)> = vec![];
    for (i, b) in v.iter().enumerate() {
        match b {
            Belirtec::Ac(e) => {
                let a = e.ad.to_ascii_lowercase();
                if !e.kapali && !BOS_OGELER.contains(&a.as_str()) {
                    yigin.push((i, a));
                }
            }
            Belirtec::Kapa(a) => {
                if let Some(x) = yigin.iter().rposition(|(_, y)| y == a) {
                    for (j, _) in yigin.drain(x..) {
                        son[j] = i;
                    }
                }
            }
            Belirtec::Metin(_) => {}
        }
    }
    for (j, _) in yigin {
        son[j] = v.len();
    }
    son
}

/// Görünen metinden şablon sözdizimi atılır: `{{ ifade }}`, Svelte `{...}`, Angular `@if (koşul) {` blokları
fn metin_temizle(s: &str, tur: SablonTuru) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if s[i..].starts_with("{{") {
            i = s[i..].find("}}").map_or(b.len(), |x| i + x + 2);
            out.push(' ');
            continue;
        }
        let c = b[i];
        if tur == SablonTuru::Svelte && c == b'{' {
            i = suslu_sonu(b, i);
            out.push(' ');
            continue;
        }
        if tur == SablonTuru::Angular && c == b'@' && b.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_alphabetic() {
                j += 1;
            }
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if b.get(j) == Some(&b'(') {
                let mut derin = 0usize;
                while j < b.len() {
                    match b[j] {
                        b'(' => derin += 1,
                        b')' => {
                            derin -= 1;
                            if derin == 0 {
                                j += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
            }
            i = j;
            out.push(' ');
            continue;
        }
        if c == b'{' || c == b'}' {
            i += 1;
            out.push(' ');
            continue;
        }
        let ch = s[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&")
}

fn oz_degeri<'a>(e: &'a Etiket, ad: &str) -> Option<&'a str> {
    e.oz.iter().find(|(a, _)| a == ad).and_then(|(_, d)| d.as_deref())
}

/// Öğenin erişilebilir adı: aria-label, görünen metin, title
fn sablon_adi(v: &[Belirtec], sonlar: &[usize], i: usize, tur: SablonTuru) -> (String, &'static str) {
    let Belirtec::Ac(e) = &v[i] else { return (String::new(), "yok") };
    if let Some(a) = oz_degeri(e, "aria-label").map(str::trim).filter(|a| !a.is_empty()) {
        return (a.to_string(), "aria-label");
    }
    let ham: Vec<&str> = v[i + 1..sonlar[i].max(i + 1)]
        .iter()
        .filter_map(|b| match b {
            Belirtec::Metin(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    let g: String = metin_temizle(&ham.join(" "), tur).split_whitespace().collect::<Vec<_>>().join(" ").chars().take(60).collect();
    if !g.is_empty() {
        return (g, "metin");
    }
    match oz_degeri(e, "title").map(str::trim).filter(|a| !a.is_empty()) {
        Some(t) => (t.to_string(), "title"),
        None => (String::new(), "yok"),
    }
}

/// Şablon olayı: Angular `(click)`, Vue `@click` / `v-on:click`, Svelte `on:click` / `onclick`. Dönen: temel olay adı
fn sablon_olayi(ad: &str, tur: SablonTuru) -> Option<String> {
    let ham = match tur {
        SablonTuru::Angular => ad.strip_prefix('(')?.strip_suffix(')')?,
        SablonTuru::Vue => ad.strip_prefix('@').or_else(|| ad.strip_prefix("v-on:"))?,
        SablonTuru::Svelte => ad.strip_prefix("on:").or_else(|| ad.strip_prefix("on").filter(|x| x.chars().all(|c| c.is_ascii_lowercase())))?,
    };
    let temel = ham.split(['.', '|']).next()?.to_ascii_lowercase();
    matches!(temel.as_str(), "click" | "dblclick" | "submit" | "ngsubmit" | "change" | "select").then_some(temel)
}

/// Şablon ifadesinin başvuruları (TypeScript ifadesi olarak ayrıştırılır); bileşen metodunun adı anahtar sözcük olabilir
/// (`delete(hero)`): çağrılan adlar ayrıca metinden alınır
fn ifade_basvurulari(ifade: &str) -> Vec<String> {
    let mut v: BTreeSet<String> = BTreeSet::new();
    let mut p = Parser::new();
    if p.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()).is_ok() {
        if let Some(a) = p.parse(ifade.as_bytes(), None) {
            v.extend(basvurular(a.root_node(), ifade.as_bytes()));
        }
    }
    let b = ifade.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if (b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] == b'$')
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'.' || b[i - 1] == b'$'))
        {
            let bas = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$' || b[i] == b'.') {
                i += 1;
            }
            if b.get(i) == Some(&b'(') {
                v.insert(ifade[bas..i].trim_end_matches('.').to_string());
            }
            continue;
        }
        i += 1;
    }
    v.into_iter().collect()
}

/// Bağlantı hedefi
enum Hedef {
    Sabit(String),
    Goreli(String),
    Dinamik,
}

/// Yazılı hedef: "/x" uygulama içi yol; göreli izinliyse "x" / "./x" bağlantıyı gösteren ekrana görelidir
fn duz_hedef(s: &str, goreli_izinli: bool) -> Option<Hedef> {
    let s = s.trim();
    if s.starts_with('/') {
        return ic_yol(s).map(Hedef::Sabit);
    }
    if !goreli_izinli || s.is_empty() || s.starts_with("..") || s.contains("://") || s.starts_with('#') || s.contains(':') || s.contains('{') {
        return None;
    }
    let s = s.trim_start_matches("./").split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
    (!s.is_empty() && !s.contains(char::is_whitespace)).then(|| Hedef::Goreli(s.to_string()))
}

/// Dizi biçimli hedef (`['/article', slug]`, `router.navigate(['/x', id])`): değişken öğeler `:p` parametresi olur
fn dizi_yolu(dizi: Node, k: &[u8]) -> Option<String> {
    let mut parcalar: Vec<String> = vec![];
    for c in cocuklar(dizi) {
        match metin_sabiti(c, k) {
            Some(s) if parcalar.is_empty() => parcalar.push(s.trim_end_matches('/').to_string()),
            Some(s) => parcalar.extend(s.split('/').filter(|x| !x.is_empty()).map(String::from)),
            None if parcalar.is_empty() => return None,
            None => parcalar.push(":p".into()),
        }
    }
    let y = parcalar.join("/");
    Some(if y.is_empty() { "/".into() } else { y })
}

/// Bağlanmış hedef ifadesi: `'/x'`, `['/x', id]`; başka ifade değişken hedeftir
fn ifade_hedefi(ifade: &str, goreli_izinli: bool) -> Option<Hedef> {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()).ok()?;
    let k = ifade.as_bytes();
    let agac = p.parse(k, None)?;
    let d = agac.root_node().named_child(0).filter(|s| s.kind() == "expression_statement").and_then(|s| s.named_child(0)).map(ic_ifade);
    let Some(d) = d else { return Some(Hedef::Dinamik) };
    if let Some(s) = metin_sabiti(d, k) {
        return duz_hedef(&s, goreli_izinli);
    }
    if d.kind() == "array" {
        return match dizi_yolu(d, k) {
            Some(y) => duz_hedef(&y, goreli_izinli),
            None => Some(Hedef::Dinamik),
        };
    }
    Some(Hedef::Dinamik)
}

/// Şablonu okur: olaylı öğeler (UI eylemleri), bağlantılar (ekran geçişleri) ve özel öğeler (Angular seçicileri).
/// `bas_satir`: metnin ilk satırının dosyadaki satırı
fn sablon_oku(m: &str, bas_satir: usize, tur: SablonTuru) -> Sablon {
    let v = belirtecler(m);
    let sonlar = oge_sonlari(&v);
    let satir_bas: Vec<usize> = m.bytes().enumerate().filter(|(_, c)| *c == b'\n').map(|(i, _)| i).collect();
    let satir_of = |bayt: usize| bas_satir + satir_bas.partition_point(|&x| x < bayt);
    let mut s = Sablon::default();
    for (i, b) in v.iter().enumerate() {
        let Belirtec::Ac(e) = b else { continue };
        let etiket = e.ad.to_ascii_lowercase();
        let st = satir_of(e.bas);
        if tur == SablonTuru::Angular && etiket.contains('-') {
            s.etiketler.insert(etiket.clone());
        }
        let baglanti = etiket.ends_with("link") || etiket == "router-link";
        for (oad, deger) in &e.oz {
            let Some(d) = deger else { continue };
            // Ekran geçişi
            let hedef = match (tur, oad.as_str()) {
                (SablonTuru::Angular, "routerLink") => duz_hedef(d, true),
                (SablonTuru::Angular, "[routerLink]") => ifade_hedefi(d, true),
                (SablonTuru::Vue, "to") if baglanti => duz_hedef(d, false),
                (SablonTuru::Vue, ":to" | "v-bind:to") if baglanti => ifade_hedefi(d, false),
                (_, "href") if etiket == "a" && d.starts_with('{') => Some(Hedef::Dinamik),
                (_, "href") if etiket == "a" => duz_hedef(d, false),
                _ => None,
            };
            match hedef {
                Some(Hedef::Sabit(y)) => s.gecisler.push((y, st, "baglanti")),
                Some(Hedef::Goreli(y)) => s.goreli.push((y, st, "baglanti")),
                Some(Hedef::Dinamik) => s.dinamik = true,
                None => {}
            }
            // UI eylemi
            let Some(olay) = sablon_olayi(oad, tur) else { continue };
            let ifade = d.trim().trim_start_matches('{').trim_end_matches('}');
            let gonder = matches!(olay.as_str(), "submit" | "ngsubmit") && etiket == "form";
            let (rol_, ad, kaynak) = if gonder {
                // Gönder düğmesi: type="submit" taşıyan ya da türü yazılmamış iç <button>
                let ic = i + 1..sonlar[i].max(i + 1);
                let bul = |kosul: &dyn Fn(&Etiket) -> bool| ic.clone().find(|&j| matches!(&v[j], Belirtec::Ac(x) if kosul(x)));
                let j = bul(&|x| oz_degeri(x, "type") == Some("submit")).or_else(|| bul(&|x| x.ad.eq_ignore_ascii_case("button") && oz_degeri(x, "type").is_none()));
                match j {
                    Some(j) => {
                        let (ad, kaynak) = sablon_adi(&v, &sonlar, j, tur);
                        let dugme = match &v[j] {
                            Belirtec::Ac(x) => x.ad.clone(),
                            _ => String::new(),
                        };
                        (rol(&dugme, Some("submit")), ad, kaynak)
                    }
                    None => ("form".into(), String::new(), "yok"),
                }
            } else {
                let (ad, kaynak) = sablon_adi(&v, &sonlar, i, tur);
                (rol(&e.ad, oz_degeri(e, "type")), ad, kaynak)
            };
            s.eylemler.push(HamEylem { rol: rol_, ad, ad_kaynagi: kaynak, olay: oad.clone(), satir: st, basvurular: ifade_basvurulari(ifade), aralik: None });
        }
    }
    s
}

/// Şablondan okunanları dosyaya ekler; satır içi Angular şablonunda başvurular bileşen sınıfının adıyla yazılır
fn sablon_ekle(d: &mut Dosya, s: Sablon, sinif: Option<&str>) {
    for mut e in s.eylemler {
        if let Some(c) = sinif {
            e.basvurular = e.basvurular.into_iter().map(|r| format!("{c}.{r}")).collect();
        }
        d.eylemler.push(e);
    }
    d.gecisler.extend(s.gecisler);
    d.goreli_gecisler.extend(s.goreli);
    d.dinamik_gecis |= s.dinamik;
    d.etiketler.extend(s.etiketler);
}

/// Monorepo çalışma alanı paketleri: ön yüzün package.json'ında `workspace:` / `file:` / `link:` ile bağımlı olunan
/// paketler, deponun içinde (ön yüzün üst klasörü altında, 3 kat) adıyla aranır. Dönen: (paket adı, kaynak klasörü:
/// `src` varsa o, yoksa paketin kökü)
fn calisma_paketleri(kok: &Path) -> Vec<(String, PathBuf)> {
    let pkg: Value = guvenli::oku(&kok.join("package.json")).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_default();
    let mut adlar: Vec<String> = vec![];
    for alan in ["dependencies", "devDependencies"] {
        for (ad, surum) in pkg[alan].as_object().into_iter().flatten() {
            if surum.as_str().is_some_and(|s| s.starts_with("workspace:") || s.starts_with("file:") || s.starts_with("link:")) {
                adlar.push(ad.clone());
            }
        }
    }
    let Some(ust) = kok.parent().filter(|_| !adlar.is_empty()) else { return vec![] };
    let mut v = vec![];
    for g in ignore::WalkBuilder::new(ust).hidden(true).git_ignore(true).max_depth(Some(4)).build().flatten() {
        let p = g.path();
        if p.file_name().is_none_or(|a| a != "package.json") || p.components().any(|c| c.as_os_str() == "node_modules") || p.starts_with(kok) {
            continue;
        }
        let Some(j) = guvenli::oku(p).and_then(|m| serde_json::from_str::<Value>(&m).ok()) else { continue };
        if let (Some(ad), Some(klasor)) = (j["name"].as_str().filter(|a| adlar.iter().any(|x| x == a)), p.parent()) {
            let src = klasor.join("src");
            v.push((ad.to_string(), if src.is_dir() { src } else { klasor.to_path_buf() }));
        }
    }
    v
}

/// RTK Query ucu: `ad: builder.query({ query: (x) => ({ url, method }) })`
fn rtk_ucu(c: Node, k: &[u8]) -> Option<(String, Vec<Parca>)> {
    let f = noktali_ad(c.child_by_field_name("function")?, k)?;
    if !(f.ends_with(".query") || f.ends_with(".mutation")) || f.starts_with('.') {
        return None;
    }
    let args = cocuklar(c.child_by_field_name("arguments")?);
    let sorgu = nesne_degeri(*args.first()?, k, "query")?;
    if !matches!(sorgu.kind(), "arrow_function" | "function_expression" | "function") {
        return None;
    }
    let mut govde = sorgu.child_by_field_name("body")?;
    if govde.kind() == "statement_block" {
        govde = cocuklar(govde).into_iter().find(|s| s.kind() == "return_statement")?.named_child(0)?;
    }
    if let Some(u) = nesne_degeri(govde, k, "url") {
        let yontem = nesne_degeri(govde, k, "method").and_then(|m| metin_sabiti(m, k)).unwrap_or_else(|| "GET".into());
        return Some((yontem.to_uppercase(), yol_parcalari(u, k)?));
    }
    // query: (id) => `/posts/${id}`
    Some(("GET".into(), yol_parcalari(govde, k)?))
}

/// Etiketten rol (REA locator rolleriyle uyumlu)
fn rol(etiket: &str, tur: Option<&str>) -> String {
    let tam = etiket.to_lowercase();
    let k = tam.rsplit('.').next().unwrap_or(&tam).to_string();
    if k == "button" || k.ends_with("button") || (k == "input" && matches!(tur, Some("submit" | "button"))) {
        "button".into()
    } else if k == "a" || k.ends_with("link") {
        "link".into()
    } else if k.contains("menuitem") || (k.ends_with("item") && (tam.contains("menu") || tam.contains("dropdown"))) {
        "menuitem".into()
    } else if k == "form" {
        "form".into()
    } else if k.contains("tab") && !k.contains("table") {
        "tab".into()
    } else if k.contains("checkbox") || k == "switch" {
        "checkbox".into()
    } else {
        k
    }
}

/// JSX öğesinin etiketi ve öznitelikleri (açılış ya da kendini kapatan öğe)
fn oznitelik<'t>(acilis: Node<'t>, k: &[u8], ad: &str) -> Option<Node<'t>> {
    cocuklar(acilis).into_iter().filter(|a| a.kind() == "jsx_attribute").find_map(|a| {
        let ilk = a.named_child(0)?;
        (metin(ilk, k) == ad).then(|| a.named_child(1)).flatten()
    })
}

fn oznitelik_metni(acilis: Node, k: &[u8], ad: &str) -> Option<String> {
    let d = oznitelik(acilis, k, ad)?;
    match d.kind() {
        "string" => metin_sabiti(d, k),
        "jsx_expression" => d.named_child(0).and_then(|i| metin_sabiti(i, k)),
        _ => None,
    }
}

/// Öğenin görünen metni: alt `jsx_text`'ler
fn gorunen_metin(oge: Node, k: &[u8]) -> String {
    let mut parcalar = vec![];
    let mut yigin = vec![oge];
    while let Some(x) = yigin.pop() {
        if x.kind() == "jsx_text" {
            parcalar.push((x.start_byte(), metin(x, k)));
        }
        yigin.extend(cocuklar(x));
    }
    parcalar.sort();
    let s = parcalar.into_iter().map(|(_, t)| t).collect::<Vec<_>>().join(" ");
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    s.chars().take(60).collect()
}

/// Açılış öğesinden (rol, ad, ad kaynağı); `<form onSubmit>` içindeki gönder düğmesine bakar
fn rol_ve_ad(acilis: Node, k: &[u8], olay: &str) -> (String, String, &'static str) {
    let etiket = acilis.child_by_field_name("name").map(|n| metin(n, k)).unwrap_or_default();
    let oge = if acilis.kind() == "jsx_opening_element" { acilis.parent().unwrap_or(acilis) } else { acilis };
    if olay == "onSubmit" && etiket.to_lowercase().ends_with("form") {
        // Gönder düğmesi: type="submit" taşıyan iç öğe; yoksa türü yazılmamış ilk <button> (HTML'de varsayılan tür submit)
        let mut yigin = vec![oge];
        let mut varsayilan = None;
        let mut bulunan = None;
        while let Some(x) = yigin.pop() {
            if matches!(x.kind(), "jsx_opening_element" | "jsx_self_closing_element") && x != acilis {
                if oznitelik_metni(x, k, "type").as_deref() == Some("submit") {
                    bulunan = Some(x);
                    break;
                }
                let e = x.child_by_field_name("name").map(|n| metin(n, k)).unwrap_or_default();
                if varsayilan.is_none() && e == "button" && oznitelik(x, k, "type").is_none() {
                    varsayilan = Some(x);
                }
            }
            let mut alt = cocuklar(x);
            alt.reverse();
            yigin.extend(alt);
        }
        if let Some(x) = bulunan.or(varsayilan) {
            let e = x.child_by_field_name("name").map(|n| metin(n, k)).unwrap_or_default();
            let ic = if x.kind() == "jsx_opening_element" { x.parent().unwrap_or(x) } else { x };
            let (ad, kaynak) = ad_bul(x, ic, k);
            return (rol(&e, Some("submit")), ad, kaynak);
        }
        return ("form".into(), String::new(), "yok");
    }
    let (ad, kaynak) = ad_bul(acilis, oge, k);
    (rol(&etiket, oznitelik_metni(acilis, k, "type").as_deref()), ad, kaynak)
}

/// Erişilebilir ad: aria-label, görünen metin, title
fn ad_bul(acilis: Node, oge: Node, k: &[u8]) -> (String, &'static str) {
    if let Some(a) = oznitelik_metni(acilis, k, "aria-label").filter(|a| !a.trim().is_empty()) {
        return (a.trim().to_string(), "aria-label");
    }
    let g = gorunen_metin(oge, k);
    if !g.is_empty() {
        return (g, "metin");
    }
    match oznitelik_metni(acilis, k, "title").filter(|a| !a.trim().is_empty()) {
        Some(t) => (t.trim().to_string(), "title"),
        None => (String::new(), "yok"),
    }
}

/// TanStack Router dosya rotası: "/_layout/items" -> "/items"; "$id" -> ":id"
fn tanstack_yolu(y: &str) -> String {
    let parcalar: Vec<String> = y
        .split('/')
        .filter(|p| !p.is_empty() && !p.starts_with('_') && *p != "index")
        .map(|p| if let Some(a) = p.strip_prefix('$') { format!(":{a}") } else { p.to_string() })
        .collect();
    format!("/{}", parcalar.join("/"))
}

/// Rota ve üst düzen bileşenleri: iç içe rotada düzen bileşeni (`<Route element={<App/>}>` altındaki ekranlar)
/// alt ekranın da köküdür; düzendeki eylemler (başlık çubuğu) her alt ekranda görünür
fn rota_ekle(out: &mut Vec<(String, usize, Option<String>)>, yol: &str, satir: usize, bilesen: &Option<String>, atalar: &[String]) {
    let yol = if yol.is_empty() { "/".to_string() } else { yol.to_string() };
    if bilesen.is_some() {
        out.push((yol.clone(), satir, bilesen.clone()));
    }
    for a in atalar {
        out.push((yol.clone(), satir, Some(a.clone())));
    }
}

/// React Router `<Route path element>` ağacı
fn react_router(oge: Node, k: &[u8], ust: &str, atalar: &[String], out: &mut Vec<(String, usize, Option<String>)>) {
    let acilis = if oge.kind() == "jsx_element" { oge.child_by_field_name("open_tag") } else { Some(oge) };
    let Some(acilis) = acilis else { return };
    let etiket = acilis.child_by_field_name("name").map(|n| metin(n, k)).unwrap_or_default();
    let mut yol = ust.to_string();
    let mut alt_atalar = atalar.to_vec();
    if etiket == "Route" {
        if let Some(p) = oznitelik_metni(acilis, k, "path") {
            yol = if p.starts_with('/') { p } else { format!("{}/{p}", ust.trim_end_matches('/')) };
        }
        let bilesen = oznitelik(acilis, k, "element")
            .and_then(|e| e.named_child(0))
            .and_then(|x| match x.kind() {
                "jsx_self_closing_element" => x.child_by_field_name("name"),
                "jsx_element" => x.child_by_field_name("open_tag").and_then(|o| o.child_by_field_name("name")),
                _ => None,
            })
            .map(|n| metin(n, k))
            // component={X}, Solid component={lazy(() => import("./x"))}
            .or_else(|| oznitelik(acilis, k, "component").or_else(|| oznitelik(acilis, k, "Component")).and_then(|e| e.named_child(0)).and_then(|x| bilesen_adi(x, k)));
        rota_ekle(out, &yol, satir(acilis), &bilesen, atalar);
        alt_atalar.extend(bilesen);
    }
    if oge.kind() == "jsx_element" {
        for c in cocuklar(oge) {
            if matches!(c.kind(), "jsx_element" | "jsx_self_closing_element") {
                react_router(c, k, &yol, &alt_atalar, out);
            }
        }
    }
}

/// Dosyayı ayrıştırır
fn ayristir(yol: &Path, kok: &Path) -> Option<Dosya> {
    ayristir_metin(yol, kok, &guvenli::oku(yol)?)
}

fn ayristir_metin(yol: &Path, kok: &Path, kaynak: &str) -> Option<Dosya> {
    let uzanti = yol.extension().and_then(|e| e.to_str()).unwrap_or("");
    // Angular dış şablonu: yalnızca şablon okuyucusu; başvurular bileşen sınıfında çözülür
    if uzanti == "html" {
        let mut d = Dosya { yol: yol.to_path_buf(), goreli: guvenli::goreli(kok, yol), ..Default::default() };
        sablon_ekle(&mut d, sablon_oku(kaynak, 1, SablonTuru::Angular), None);
        return Some(d);
    }
    let tek_dosya = uzanti == "vue" || uzanti == "svelte";
    let ozgun = kaynak;
    let ayiklanan;
    let kaynak = if tek_dosya {
        ayiklanan = betik_ayikla(kaynak);
        ayiklanan.as_str()
    } else {
        kaynak
    };
    let k = kaynak.as_bytes();
    let mut p = Parser::new();
    let dil = if tek_dosya || yol.extension().is_some_and(|e| e == "ts") { tree_sitter_typescript::LANGUAGE_TYPESCRIPT } else { tree_sitter_typescript::LANGUAGE_TSX };
    p.set_language(&dil.into()).ok()?;
    let agac = p.parse(k, None)?;
    let mut d = Dosya { yol: yol.to_path_buf(), goreli: guvenli::goreli(kok, yol), ..Default::default() };
    gez(agac.root_node(), k, &mut d, None, None, true);
    if tek_dosya {
        let tur = if uzanti == "vue" { SablonTuru::Vue } else { SablonTuru::Svelte };
        sablon_ekle(&mut d, sablon_oku(ozgun, 1, tur), None);
    }
    Some(d)
}

/// Ağacı dolaşır. `sahip`: içinde bulunulan fonksiyon / metot / RTK uç adı; `ust_duzey`: program düzeyinde mi
fn gez(n: Node, k: &[u8], d: &mut Dosya, sinif: Option<&str>, sahip: Option<&str>, ust_duzey: bool) {
    let mut sahip = sahip.map(String::from);
    let mut sinif = sinif.map(String::from);
    let mut alt_ust_duzey = false;
    match n.kind() {
        "program" => alt_ust_duzey = true,
        "export_statement" => {
            alt_ust_duzey = ust_duzey;
            if let Some(kaynak) = n.child_by_field_name("source").and_then(|s| metin_sabiti(s, k)) {
                match cocuklar(n).into_iter().find(|c| c.kind() == "export_clause") {
                    Some(clause) => {
                        for s in cocuklar(clause).into_iter().filter(|s| s.kind() == "export_specifier") {
                            let ad = s.child_by_field_name("name").map(|x| metin(x, k)).unwrap_or_default();
                            let takma = s.child_by_field_name("alias").map(|x| metin(x, k)).unwrap_or_else(|| ad.clone());
                            d.yeniden.insert(takma, (kaynak.clone(), ad));
                        }
                    }
                    None => d.yildiz.push(kaynak),
                }
            }
            if let Some(v) = n.child_by_field_name("value") {
                d.varsayilan = Some(metin(v, k));
                // React Router v7 rota yapılandırması: app/routes.ts `export default [route(...), ...] satisfies RouteConfig`
                let v = ic_ifade(v);
                if v.kind() == "array" && Path::new(&d.goreli).file_stem().is_some_and(|a| a == "routes") {
                    let mut out = vec![];
                    yapilandirma(v, k, "", &[], &mut out);
                    d.yapilandirma_rotalari.extend(out);
                }
            }
            if let Some(dek) = n.child_by_field_name("declaration") {
                if metin(n, k).trim_start().starts_with("export default") {
                    d.varsayilan = dek.child_by_field_name("name").map(|a| metin(a, k));
                }
            }
        }
        "import_statement" => {
            let Some(kaynak) = n.child_by_field_name("source").and_then(|s| metin_sabiti(s, k)) else { return };
            for c in cocuklar(n).into_iter().filter(|c| c.kind() == "import_clause") {
                for x in cocuklar(c) {
                    match x.kind() {
                        "identifier" => {
                            d.iceri.insert(metin(x, k), (kaynak.clone(), "default".into()));
                        }
                        "named_imports" => {
                            for s in cocuklar(x).into_iter().filter(|s| s.kind() == "import_specifier") {
                                let ad = s.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
                                let takma = s.child_by_field_name("alias").map(|a| metin(a, k)).unwrap_or_else(|| ad.clone());
                                d.iceri.insert(takma, (kaynak.clone(), ad));
                            }
                        }
                        "namespace_import" => {
                            if let Some(a) = x.named_child(0) {
                                d.iceri.insert(metin(a, k), (kaynak.clone(), "*".into()));
                            }
                        }
                        _ => {}
                    }
                }
            }
            return;
        }
        "lexical_declaration" | "variable_declaration" => alt_ust_duzey = ust_duzey,
        "class_declaration" | "class" | "abstract_class_declaration" => {
            sinif = n.child_by_field_name("name").map(|a| metin(a, k));
            if let Some(s) = &sinif {
                angular_sinifi(n, k, d, s);
            }
        }
        "function_declaration" | "generator_function_declaration" => {
            if let (Some(ad), Some(govde)) = (n.child_by_field_name("name"), n.child_by_field_name("body")) {
                let ad = metin(ad, k);
                d.fonksiyonlar.insert(ad.clone(), basvurular(govde, k));
                yukleyici(d, &ad, n, ust_duzey);
                sahip = Some(ad);
            }
        }
        "method_definition" => {
            if let (Some(ad), Some(govde)) = (n.child_by_field_name("name"), n.child_by_field_name("body")) {
                let metot = metin(ad, k);
                let ad = match &sinif {
                    Some(s) => format!("{s}.{metot}"),
                    None => metot.clone(),
                };
                let b = sinifla(basvurular(govde, k), sinif.as_deref());
                // Angular bileşeninin açılışı (ngOnInit, kurucu): ekran yüklemesi
                if let Some(s) = sinif.as_ref().filter(|s| d.bilesen_siniflari.contains(*s) && matches!(metot.as_str(), "ngOnInit" | "constructor")) {
                    d.eylemler.push(HamEylem {
                        rol: "ekran".into(),
                        ad: s.clone(),
                        ad_kaynagi: "bilesen",
                        olay: "yukleme".into(),
                        satir: satir(n),
                        basvurular: b.clone(),
                        aralik: None,
                    });
                }
                d.fonksiyonlar.insert(ad.clone(), b);
                sahip = Some(ad);
            }
        }
        // Sınıf alanı: ok fonksiyonu metot gibi, metin sabit gibi (`private heroesUrl = 'api/heroes'`)
        "public_field_definition" => {
            if let (Some(s), Some(a), Some(v)) = (sinif.clone(), n.child_by_field_name("name"), n.child_by_field_name("value")) {
                let ad = format!("{s}.{}", metin(a, k));
                if matches!(v.kind(), "arrow_function" | "function_expression" | "function") {
                    d.fonksiyonlar.insert(ad.clone(), sinifla(basvurular(v, k), Some(&s)));
                    sahip = Some(ad);
                } else if let Some(y) = yol_parcalari(v, k) {
                    d.sabitler.insert(ad, sinifla_yol(y, Some(&s)));
                }
            }
        }
        // "use server" yönergesi (SolidStart, React sunucu fonksiyonları)
        "statement_block" => {
            if cocuklar(n).first().is_some_and(|x| x.kind() == "expression_statement" && x.named_child(0).and_then(|y| metin_sabiti(y, k)).as_deref() == Some("use server")) {
                d.sunucu_fonksiyonu += 1;
            }
        }
        "variable_declarator" => {
            if let (Some(ad), Some(deger)) = (n.child_by_field_name("name"), n.child_by_field_name("value")) {
                let deger = if deger.kind() == "await_expression" { deger.named_child(0).unwrap_or(deger) } else { deger };
                // Rota dizisi: `const routes: Routes = [...]`, `export const routes = [...] satisfies RouteObject[]`
                let ic = ic_ifade(deger);
                let tipli = matches!(deger.kind(), "satisfies_expression" | "as_expression") && deger.named_child(1).is_some_and(|t| rota_tipi(&metin(t, k)));
                if ad.kind() == "identifier" && ic.kind() == "array" {
                    let a = metin(ad, k);
                    if n.child_by_field_name("type").is_some_and(|t| rota_tipi(&metin(t, k))) || a.to_ascii_lowercase().ends_with("routes") || tipli {
                        let r = rota_dizisi(ic, k, &d.goreli);
                        if !r.is_empty() {
                            d.rota_dizileri.push((Some(a), r));
                        }
                    }
                }
                match (ad.kind(), deger.kind()) {
                    ("identifier", "arrow_function" | "function_expression" | "function") => {
                        let a = metin(ad, k);
                        d.fonksiyonlar.insert(a.clone(), basvurular(deger, k));
                        yukleyici(d, &a, n, ust_duzey);
                        sahip = Some(a);
                    }
                    ("identifier", "object") if ust_duzey => {
                        let a = metin(ad, k);
                        nesne_sabitleri(&a, deger, k, &mut d.sabitler, 0);
                    }
                    ("identifier", "call_expression") => {
                        let a = metin(ad, k);
                        let kanca = deger.child_by_field_name("function").and_then(|f| noktali_ad(f, k)).unwrap_or_default();
                        // Tembel bileşen: `const Ayarlar = lazy(() => import("./Ayarlar"))` içe aktarma gibi
                        if matches!(kanca.as_str(), "lazy" | "React.lazy" | "defineAsyncComponent" | "loadable" | "clientOnly") {
                            if let Some(b) = tembel_modul(deger, k) {
                                d.iceri.insert(a.clone(), (b, "default".into()));
                            }
                        }
                        // Sarmalanmış fonksiyon (Solid `query` / `action` / `cache`, `server$`): `const f = query(async () => ..., "ad")`
                        if matches!(kanca.as_str(), "query" | "cache" | "action" | "server$" | "createServerAction$" | "createServerData$" | "createRouteAction") {
                            let f = deger
                                .child_by_field_name("arguments")
                                .and_then(|x| x.named_child(0))
                                .filter(|f| matches!(f.kind(), "arrow_function" | "function_expression" | "function"));
                            if let Some(f) = f {
                                d.fonksiyonlar.insert(a.clone(), basvurular(f, k));
                                sahip = Some(a.clone());
                            }
                        }
                        if kanca.starts_with("use") {
                            d.baglamalar.insert(a.clone(), basvurular(deger, k));
                        }
                        // createFileRoute(...)({ component: X }): bileşen
                        if ust_duzey {
                            if let Some(y) = yol_parcalari(deger, k) {
                                d.sabitler.insert(a, y);
                            }
                        }
                    }
                    ("array_pattern", "call_expression") => {
                        let kanca = deger.child_by_field_name("function").and_then(|f| noktali_ad(f, k)).unwrap_or_default();
                        if kanca.starts_with("use") {
                            let b = basvurular(deger, k);
                            for x in cocuklar(ad).into_iter().filter(|x| x.kind() == "identifier") {
                                d.baglamalar.insert(metin(x, k), b.clone());
                            }
                        }
                    }
                    // const { loginMutation } = useAuth(): ad, kancanın içindeki aynı adlı bağa çözülür ("useAuth.loginMutation")
                    ("object_pattern", "call_expression") => {
                        let kanca = deger.child_by_field_name("function").and_then(|f| noktali_ad(f, k)).unwrap_or_default();
                        if kanca.starts_with("use") {
                            for x in cocuklar(ad) {
                                let (anahtar, yerel) = match x.kind() {
                                    "shorthand_property_identifier_pattern" => (metin(x, k), metin(x, k)),
                                    "pair_pattern" => match (x.child_by_field_name("key"), x.child_by_field_name("value")) {
                                        (Some(a), Some(v)) if v.kind() == "identifier" => (metin(a, k), metin(v, k)),
                                        _ => continue,
                                    },
                                    _ => continue,
                                };
                                d.baglamalar.insert(yerel, vec![format!("{kanca}.{anahtar}")]);
                            }
                        }
                    }
                    ("identifier", _) if ust_duzey => {
                        if let Some(y) = yol_parcalari(deger, k) {
                            d.sabitler.insert(metin(ad, k), y);
                        }
                    }
                    _ => {}
                }
            }
        }
        "pair" => {
            if let Some(y) = yol_ozelligi(n, k) {
                d.yol_ozellikleri.push((y, satir(n)));
            }
            // RTK Query ucu: anahtar uç adıdır
            if let (Some(a), Some(v)) = (n.child_by_field_name("key"), n.child_by_field_name("value")) {
                if v.kind() == "call_expression" {
                    if let Some((yontem, yol)) = rtk_ucu(v, k) {
                        let ad = metin(a, k).trim_matches(['\'', '"']).to_string();
                        d.cagrilar.push(HamCagri { sahip: Some(ad), yontem, yontem_kesin: true, yol, satir: satir(v), taniyici: "rtk-query", bayt: v.start_byte() });
                        return;
                    }
                }
            }
        }
        "call_expression" => {
            let ad = n.child_by_field_name("function").and_then(|f| noktali_ad(f, k)).unwrap_or_default();
            if let Some((yontem, kesin, yol, taniyici)) = istemci_cagrisi(n, k) {
                // Angular HttpClient (`this.http.get(...)`): dosya HttpClient'ı içe aktarıyor
                let taniyici = if taniyici == "axios" && d.iceri.get("HttpClient").is_some_and(|(m, _)| m == "@angular/common/http") { "angular-http" } else { taniyici };
                let yol = sinifla_yol(yol, sinif.as_deref());
                d.cagrilar.push(HamCagri { sahip: sahip.clone(), yontem, yontem_kesin: kesin, yol, satir: satir(n), taniyici, bayt: n.start_byte() });
            }
            if let Some((hedef, tur)) = rota_cagrisi(n, k, d) {
                d.rota_cagrilari.push(RotaCagrisi { sahip: sahip.clone(), bayt: n.start_byte(), hedef, tur });
            }
            match gecis_cagrisi(n, k) {
                Some(Gecis::Sabit(y, t)) => d.gecisler.push((y, satir(n), t)),
                Some(Gecis::Dinamik) => d.dinamik_gecis = true,
                None => {}
            }
            if ad == "createFileRoute" || ad == "createRoute" {
                let args = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
                if let Some(y) = args.first().and_then(|a| metin_sabiti(*a, k)) {
                    // Bileşen: createFileRoute("/x")({ component: X })
                    let bilesen = n
                        .parent()
                        .filter(|p| p.kind() == "call_expression")
                        .and_then(|p| p.child_by_field_name("arguments"))
                        .and_then(|a| a.named_child(0))
                        .and_then(|o| nesne_degeri(o, k, "component"))
                        .map(|c| metin(c, k));
                    d.rotalar.push((tanstack_yolu(&y), satir(n), bilesen));
                } else if let Some(o) = args.first() {
                    if let Some(y) = nesne_degeri(*o, k, "path").and_then(|p| metin_sabiti(p, k)) {
                        d.rotalar.push((y, satir(n), nesne_degeri(*o, k, "component").map(|c| metin(c, k))));
                    }
                }
            }
            let ilk_arg = n.child_by_field_name("arguments").and_then(|a| a.named_child(0));
            // Satır içi rota dizisi: React Router, Angular RouterModule / provideRouter, Vue Router createRouter({ routes })
            let dizi = match ad.as_str() {
                "createBrowserRouter"
                | "createHashRouter"
                | "createMemoryRouter"
                | "useRoutes"
                | "RouterModule.forRoot"
                | "RouterModule.forChild"
                | "provideRouter"
                | "provideRoutes" => ilk_arg,
                "createRouter" | "VueRouter" => ilk_arg.and_then(|o| nesne_degeri(o, k, "routes")),
                _ => None,
            };
            if let Some(dizi) = dizi.filter(|a| a.kind() == "array") {
                let r = rota_dizisi(dizi, k, &d.goreli);
                if !r.is_empty() {
                    d.rota_dizileri.push((None, r));
                }
            }
            // Uygulamanın kök bileşeni: bootstrapApplication(AppComponent), createApp(App)
            if matches!(ad.as_str(), "bootstrapApplication" | "createApp" | "createSSRApp") {
                d.kok_bilesenler.extend(ilk_arg.filter(|a| a.kind() == "identifier").map(|a| metin(a, k)));
            }
            if ad == "flatRoutes" {
                d.duz_rotalar = true;
            }
            // Ekran yüklemesi: sorgu kancası (useQuery, useSuspenseQuery, RTK useXQuery), Solid createResource / createAsync
            if SORGU_KANCALARI.contains(&ad.as_str()) || (ad.starts_with("use") && ad.ends_with("Query") && !ad.starts_with("useLazy")) || SOLID_YUKLEME.contains(&ad.as_str()) {
                let mut b = basvurular(n, k);
                b.push(ad.clone());
                let bilesen = sahip.clone().unwrap_or_default();
                d.eylemler.push(HamEylem {
                    rol: "ekran".into(),
                    ad: bilesen,
                    ad_kaynagi: "bilesen",
                    olay: "yukleme".into(),
                    satir: satir(n),
                    basvurular: b,
                    aralik: Some((n.start_byte(), n.end_byte())),
                });
            }
        }
        "jsx_element" | "jsx_self_closing_element" => {
            let acilis = if n.kind() == "jsx_element" { n.child_by_field_name("open_tag") } else { Some(n) };
            if let Some(acilis) = acilis {
                let etiket = acilis.child_by_field_name("name").map(|x| metin(x, k)).unwrap_or_default();
                match gecis_baglantisi(acilis, k) {
                    Some(Gecis::Sabit(y, t)) => d.gecisler.push((y, satir(acilis), t)),
                    Some(Gecis::Dinamik) => d.dinamik_gecis = true,
                    None => {}
                }
                let ust_rota = n
                    .parent()
                    .filter(|p| p.kind() == "jsx_element")
                    .and_then(|p| p.child_by_field_name("open_tag"))
                    .and_then(|o| o.child_by_field_name("name"))
                    .is_some_and(|a| metin(a, k) == "Route");
                if etiket == "Route" && !ust_rota {
                    // Solid `<Router root={App}>`: kök düzen bileşeni her rotanın atasıdır
                    let kok = n
                        .parent()
                        .filter(|p| p.kind() == "jsx_element")
                        .and_then(|p| p.child_by_field_name("open_tag"))
                        .filter(|o| o.child_by_field_name("name").is_some_and(|a| metin(a, k) == "Router"))
                        .and_then(|o| oznitelik(o, k, "root"))
                        .and_then(|r| r.named_child(0))
                        .filter(|r| r.kind() == "identifier")
                        .map(|r| metin(r, k));
                    react_router(n, k, "", &kok.into_iter().collect::<Vec<_>>(), &mut d.rotalar);
                }
                for olay in OLAYLAR {
                    if let Some(deger) = oznitelik(acilis, k, olay) {
                        let (r, ad, kaynak) = rol_ve_ad(acilis, k, olay);
                        let aralik = Some((deger.start_byte(), deger.end_byte()));
                        d.eylemler.push(HamEylem { rol: r, ad, ad_kaynagi: kaynak, olay: olay.into(), satir: satir(acilis), basvurular: basvurular(deger, k), aralik });
                    }
                }
                form_eylemi(acilis, &etiket, k, d);
            }
        }
        _ => {}
    }
    for c in cocuklar(n) {
        gez(c, k, d, sinif.as_deref(), sahip.as_deref(), alt_ust_duzey);
    }
}

/// Remix / React Router rota modülünün yükleyicisi (`export async function loader`, `clientLoader`): ekran yüklemesi
fn yukleyici(d: &mut Dosya, ad: &str, n: Node, ust_duzey: bool) {
    if ust_duzey && matches!(ad, "loader" | "clientLoader") {
        d.eylemler.push(HamEylem { rol: "ekran".into(), ad: ad.into(), ad_kaynagi: "bilesen", olay: "yukleme".into(), satir: satir(n), basvurular: vec![ad.into()], aralik: None });
    }
}

/// Rota modülüne gönderilen form: Remix / React Router `<Form method action>`, `<fetcher.Form>`, yerel `<form method>`;
/// Solid `<form action={eylem}>` ve `<button formAction={eylem}>`. `onSubmit` taşıyan form olay işleyicisiyle okunur
fn form_eylemi(acilis: Node, etiket: &str, k: &[u8], d: &mut Dosya) {
    if oznitelik(acilis, k, "onSubmit").is_some() {
        return;
    }
    let son = etiket.rsplit('.').next().unwrap_or(etiket);
    let form = son == "Form" || etiket == "form";
    let yontem = oznitelik_metni(acilis, k, "method").map(|m| m.to_ascii_lowercase());
    let hedef = oznitelik(acilis, k, if form { "action" } else { "formAction" });
    if !((form && (son == "Form" || yontem.is_some() || hedef.is_some())) || (!form && hedef.is_some())) {
        return;
    }
    let tur = if yontem.as_deref().is_some_and(|y| y != "get") { "action" } else { "loader" };
    let ic = hedef.and_then(|h| if h.kind() == "jsx_expression" { h.named_child(0) } else { Some(h) });
    let basv = match ic {
        Some(x) => match metin_sabiti(x, k) {
            Some(y) => vec![format!("@rota:{tur}:{y}")],
            None => basvurular(x, k),
        },
        None => vec![format!("@rota:{tur}:")],
    };
    let (r, ad, kaynak) = rol_ve_ad(acilis, k, if form { "onSubmit" } else { "onClick" });
    d.eylemler.push(HamEylem { rol: r, ad, ad_kaynagi: kaynak, olay: "submit".into(), satir: satir(acilis), basvurular: basv, aralik: ic.map(|x| (x.start_byte(), x.end_byte())) });
}

// ---- Ekran geçişleri (yönlendirme) ----

/// Yönlendirme tanıyıcısının sonucu: sabit hedef (yol, tanıyıcı) ya da değişken hedef
enum Gecis {
    Sabit(String, &'static str),
    Dinamik,
}

/// Uygulama içi yol: "/" ile başlar; sorgu ve parça atılır, sondaki "/" kırpılır (kök hariç)
fn ic_yol(y: &str) -> Option<String> {
    let y = y.trim();
    if !y.starts_with('/') || y.starts_with("//") || y.contains("://") || y.contains(char::is_whitespace) || y.len() > 200 {
        return None;
    }
    let y = y.split(['?', '#']).next().unwrap_or("");
    let y = if y.len() > 1 { y.trim_end_matches('/') } else { y };
    Some(y.to_string())
}

/// Yönlendirme çağrısı: `navigate("/x")` / `navigate({ to: "/x" })` (React Router ve TanStack `useNavigate`),
/// `router.push` / `router.replace` / `history.push` (Next.js, Vue Router), `redirect("/x")` / `redirect({ to })` (TanStack, Next.js, Remix,
/// Solid), Angular `this.router.navigate(['/x', id])` / `navigateByUrl('/x')`
fn gecis_cagrisi(c: Node, k: &[u8]) -> Option<Gecis> {
    let ad = c.child_by_field_name("function").and_then(|f| noktali_ad(f, k))?;
    let son = ad.rsplit('.').next().unwrap_or(&ad);
    let alici = ad.rsplit_once('.').map(|(a, _)| a.rsplit('.').next().unwrap_or(a));
    let taniyici = match (alici, son) {
        (None, "navigate" | "navigateTo") => "navigate",
        (None, "redirect") => "redirect",
        (Some(a), "push" | "replace") if a == "router" || a == "history" || a.ends_with("Router") => "router.push",
        (Some(a), "navigate" | "navigateByUrl") if a == "router" || a == "navigation" || a.ends_with("Router") => "navigate",
        _ => return None,
    };
    let arg = c.child_by_field_name("arguments")?.named_child(0)?;
    // Angular komut dizisi: göreli komut (`relativeTo`) ekran geçişi sayılmaz
    if arg.kind() == "array" {
        return match dizi_yolu(arg, k) {
            Some(y) if y.starts_with('/') => ic_yol(&y).map(|y| Gecis::Sabit(y, taniyici)),
            Some(_) => None,
            None => Some(Gecis::Dinamik),
        };
    }
    let hedef = metin_sabiti(arg, k).or_else(|| nesne_degeri(arg, k, "to").or_else(|| nesne_degeri(arg, k, "pathname")).and_then(|v| metin_sabiti(v, k)));
    match hedef {
        Some(h) => ic_yol(&h).map(|y| Gecis::Sabit(y, taniyici)),
        None => Some(Gecis::Dinamik),
    }
}

/// Bağlantı öğesi: `<Link to="/x">`, `<NavLink to>`, `<RouterLink to>`, `<Navigate to>` (React Router, TanStack), `<Link href>` (Next.js),
/// `<A href>` (Solid Router)
fn gecis_baglantisi(acilis: Node, k: &[u8]) -> Option<Gecis> {
    let etiket = acilis.child_by_field_name("name").map(|x| metin(x, k))?;
    let son = etiket.rsplit('.').next().unwrap_or(&etiket);
    if !(son.ends_with("Link") || son == "Navigate" || son == "Redirect" || son == "A") {
        return None;
    }
    let deger = oznitelik(acilis, k, "to").or_else(|| oznitelik(acilis, k, "href"))?;
    let ic = if deger.kind() == "jsx_expression" { deger.named_child(0)? } else { deger };
    match metin_sabiti(ic, k) {
        Some(h) => ic_yol(&h).map(|y| Gecis::Sabit(y, "baglanti")),
        None => Some(Gecis::Dinamik),
    }
}

/// Yol listesi öğesi: `{ path: "/items" }`, `{ to: ... }`, `{ href: ... }`, `{ url: ... }` (kenar çubuğu menüsü gibi)
fn yol_ozelligi(n: Node, k: &[u8]) -> Option<String> {
    let a = metin(n.child_by_field_name("key")?, k).trim_matches(['\'', '"']).to_string();
    if !matches!(a.as_str(), "path" | "to" | "href" | "url") {
        return None;
    }
    ic_yol(&metin_sabiti(n.child_by_field_name("value")?, k)?)
}

/// Ekran geçişi bağları (`screen -[NAVIGATES]-> screen`, inferred): geçişin bulunduğu dosyayı gösteren ekranlardan hedef
/// ekrana. Düzen (layout) rotası (`routes/_layout.tsx`, alt ekranları `routes/_layout/` altında) ve onun içe aktardığı
/// dosyalar (kenar çubuğu) alt ekranların her birinde görünür: oradaki geçiş alt ekranlardan yapılabilir
fn gecis_baglari(p: &Proje, ekranlar: &BTreeMap<String, (String, usize, BTreeSet<usize>)>, dosya_ekranlari: &HashMap<usize, BTreeSet<String>>) -> Vec<Value> {
    let yollar: HashMap<&str, &str> = ekranlar.keys().map(|id| (&id["screen:web/".len()..], id.as_str())).collect();
    let goreli_sira: HashMap<&str, usize> = p.dosyalar.iter().enumerate().map(|(i, d)| (d.goreli.as_str(), i)).collect();
    let mut erisim: HashMap<usize, BTreeSet<usize>> = HashMap::new();
    let mut duzen: HashMap<usize, BTreeSet<String>> = HashMap::new();
    for (id, (_, _, kokler)) in ekranlar {
        for &kd in kokler {
            let mut ust = Path::new(&p.dosyalar[kd].goreli).parent();
            while let Some(u) = ust.filter(|u| !u.as_os_str().is_empty()) {
                for uz in UZANTILAR {
                    let aday = u.with_extension(uz);
                    let Some(&li) = goreli_sira.get(aday.to_string_lossy().replace('\\', "/").as_str()) else { continue };
                    let ulasilan = erisim.entry(li).or_insert_with(|| {
                        let mut gorulen = BTreeSet::new();
                        let mut yigin = vec![(li, 0usize)];
                        while let Some((x, derin)) = yigin.pop() {
                            if derin > 12 || !gorulen.insert(x) {
                                continue;
                            }
                            yigin.extend(p.komsular(x).into_iter().map(|y| (y, derin + 1)));
                        }
                        gorulen
                    });
                    for x in ulasilan.iter() {
                        duzen.entry(*x).or_default().insert(id.clone());
                    }
                }
                ust = u.parent();
            }
        }
    }
    let mut gorulen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut baglar = vec![];
    for (di, d) in p.dosyalar.iter().enumerate() {
        let mut hedefler: Vec<(&str, usize, &str)> = d.gecisler.iter().map(|(y, s, t)| (y.as_str(), *s, *t)).collect();
        // Yol listesi, aynı dosyada ya da içe aktardığı bir dosyada hedefi değişken bir bağlantıyla kullanılıyorsa (kenar çubuğu
        // öğeleri ve onları çizen bileşen)
        if d.dinamik_gecis || (!d.yol_ozellikleri.is_empty() && p.komsular(di).iter().any(|x| p.dosyalar[*x].dinamik_gecis)) {
            hedefler.extend(d.yol_ozellikleri.iter().map(|(y, s)| (y.as_str(), *s, "yol_listesi")));
        }
        if hedefler.is_empty() && d.goreli_gecisler.is_empty() {
            continue;
        }
        let kaynaklar: BTreeSet<&String> = dosya_ekranlari.get(&di).into_iter().flatten().chain(duzen.get(&di).into_iter().flatten()).collect();
        // Göreli bağlantı (Angular `routerLink="detail"`): her kaynak ekranın yoluna göre çözülür
        let mut ciftler: Vec<(&String, String, usize, &str)> = vec![];
        for (y, satir, taniyici) in hedefler {
            ciftler.extend(kaynaklar.iter().map(|k| (*k, y.to_string(), satir, taniyici)));
        }
        for (y, satir, taniyici) in &d.goreli_gecisler {
            ciftler.extend(kaynaklar.iter().map(|k| (*k, yol_birlestir(&k["screen:web/".len()..], y), *satir, *taniyici)));
        }
        for (kaynak, y, satir, taniyici) in ciftler {
            let Some(hedef) = ekran_bul(&yollar, &y) else { continue };
            if kaynak.as_str() == hedef || !gorulen.insert((kaynak.clone(), hedef.to_string())) {
                continue;
            }
            baglar.push(json!({
                "kaynak": kaynak, "iliski": "NAVIGATES", "hedef": hedef, "guven": "inferred", "taniyici": taniyici,
                "kanit": { "tur": "SOURCE_CODE", "dosya": d.goreli, "satir": satir },
            }));
        }
    }
    baglar
}

/// Geçiş hedefinin ekranı: yol birebir ya da parametreli hedef (`/article/:p`) tek bir parametreli ekranla (`/article/:slug`)
fn ekran_bul<'a>(yollar: &HashMap<&str, &'a str>, y: &str) -> Option<&'a str> {
    if let Some(h) = yollar.get(y) {
        return Some(h);
    }
    if !y.contains("/:") {
        return None;
    }
    let ys: Vec<&str> = y.split('/').collect();
    let mut adaylar: Vec<&str> = yollar
        .iter()
        .filter(|(e, _)| {
            let es: Vec<&str> = e.split('/').collect();
            es.len() == ys.len() && es.iter().zip(&ys).all(|(a, b)| a == b || (a.starts_with(':') && b.starts_with(':')))
        })
        .map(|(_, h)| *h)
        .collect();
    adaylar.sort();
    adaylar.dedup();
    (adaylar.len() == 1).then(|| adaylar[0])
}

/// Ön yüzün bütün kaynak dosyaları; `html`: Angular şablonları da
fn dosyalar(kok: &Path, html: bool) -> Vec<PathBuf> {
    let mut v = vec![];
    for g in ignore::WalkBuilder::new(kok).hidden(true).git_ignore(true).build().flatten() {
        let p = g.path();
        if g.path_is_symlink() || !p.is_file() {
            continue;
        }
        let goreli = p.strip_prefix(kok).unwrap_or(p);
        if goreli.components().any(|c| ATLA.contains(&c.as_os_str().to_str().unwrap_or(""))) {
            continue;
        }
        let ad = p.file_name().and_then(|a| a.to_str()).unwrap_or("");
        if ad.ends_with(".d.ts") || ad.contains(".test.") || ad.contains(".spec.") || ad.contains(".stories.") {
            continue;
        }
        if p.extension().and_then(|e| e.to_str()).is_some_and(|e| UZANTILAR.contains(&e) || (html && e == "html")) && p.metadata().is_ok_and(|m| m.len() <= guvenli::EN_BUYUK_DOSYA)
        {
            v.push(p.to_path_buf());
        }
    }
    v.sort();
    v
}

/// tsconfig `paths` takma adları: ("@/", <klasör>); yoksa `@/` ve `~/` -> src. `baseUrl` yazılıysa ("", <taban>): yalın
/// belirteç (`src/environments/environment`) tabana göre de aranır
fn takma_adlar(kok: &Path) -> Vec<(String, PathBuf)> {
    let mut v = vec![];
    let mut tabanlar = vec![];
    let sondaki_virgul = regex::Regex::new(r",(\s*[}\]])").unwrap();
    for ad in ["tsconfig.json", "jsconfig.json", "tsconfig.app.json"] {
        let Some(m) = guvenli::oku(&kok.join(ad)) else { continue };
        // Yorum satırları, satır başındaki blok yorumlar ve sondaki virgüller atılır (tsconfig JSONC'dir)
        let mut blokta = false;
        let temiz: String = m
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                let at = blokta || t.starts_with("/*") || t.starts_with("//");
                if t.starts_with("/*") || blokta {
                    blokta = !t.contains("*/");
                }
                !at
            })
            .collect::<Vec<_>>()
            .join("\n");
        let temiz = sondaki_virgul.replace_all(&temiz, "$1").to_string();
        let Ok(j) = serde_json::from_str::<Value>(&temiz) else { continue };
        let taban = kok.join(j["compilerOptions"]["baseUrl"].as_str().unwrap_or("."));
        if j["compilerOptions"]["baseUrl"].is_string() {
            tabanlar.push(taban.clone());
        }
        for (anahtar, hedefler) in j["compilerOptions"]["paths"].as_object().into_iter().flatten() {
            if let (Some(on), Some(h)) = (anahtar.strip_suffix('*'), hedefler.get(0).and_then(|h| h.as_str()).and_then(|h| h.strip_suffix('*'))) {
                v.push((on.to_string(), taban.join(h.trim_start_matches("./"))));
            }
        }
    }
    for on in ["@/", "~/"] {
        if !v.iter().any(|(o, _)| o == on) {
            v.push((on.to_string(), kok.join("src")));
        }
    }
    v.extend(tabanlar.into_iter().take(1).map(|t| (String::new(), t)));
    v
}

/// İçe aktarma belirtecinden yerel dosya
fn modul_coz(kimden: &Path, belirtec: &str, takma: &[(String, PathBuf)]) -> Option<PathBuf> {
    let taban = if belirtec.starts_with('.') {
        kimden.parent()?.join(belirtec)
    } else {
        let (on, k) = takma.iter().find(|(on, _)| belirtec.starts_with(on.as_str()))?;
        k.join(&belirtec[on.len()..])
    };
    let adaylar = [String::new(), ".ts".into(), ".tsx".into(), ".js".into(), ".jsx".into(), ".mjs".into()];
    for u in &adaylar {
        let p = PathBuf::from(format!("{}{u}", taban.display()));
        if p.is_file() {
            return Some(p);
        }
    }
    for u in ["ts", "tsx", "js", "jsx"] {
        let p = taban.join(format!("index.{u}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Proje: bütün dosyalar ve modül çözümleri
struct Proje {
    dosyalar: Vec<Dosya>,
    sira: HashMap<PathBuf, usize>,
    takma: Vec<(String, PathBuf)>,
    /// package.json'dan çatılar: "angular", "remix" (React Router v7 çatı kipi dahil), "solidstart"
    cati: BTreeSet<&'static str>,
    /// Dosyaların yerel komşuları: içe aktarmalar, Angular dış şablonu ve şablonda seçiciyle kullanılan bileşenler
    komsu: Vec<Vec<usize>>,
    /// Dış şablon -> (bileşen dosyası, bileşen sınıfı): şablondaki başvurular sınıfta çözülür
    sablon_sahibi: HashMap<usize, (usize, String)>,
}

/// Uygulamanın package.json'ından çatılar
fn catilar(kok: &Path) -> BTreeSet<&'static str> {
    let b = bagimliliklar(kok);
    let mut v = BTreeSet::new();
    if b.contains("@angular/core") {
        v.insert("angular");
    }
    if b.iter().any(|x| x.starts_with("@remix-run/") || x == "@react-router/dev" || x == "@react-router/fs-routes") {
        v.insert("remix");
    }
    if b.contains("@solidjs/start") || b.contains("solid-start") {
        v.insert("solidstart");
    }
    if b.contains("next") || b.contains("nuxt") {
        v.insert("next");
    }
    v
}

impl Proje {
    /// Ayrıştırılmış dosyalardan proje: modül sırası, takma adlar, şablon ve seçici bağları, komşular
    fn kur(kok: &Path, dosyalar: Vec<Dosya>, paketler: Vec<(String, PathBuf)>, cati: BTreeSet<&'static str>) -> Proje {
        let kanonik = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let sira: HashMap<PathBuf, usize> = dosyalar.iter().enumerate().map(|(i, d)| (kanonik(&d.yol), i)).collect();
        let mut takma = takma_adlar(kok);
        takma.extend(paketler);
        // Tabana göre arama (boş önek) her belirtece uyar: en sona
        takma.sort_by_key(|(on, _)| on.is_empty());
        let mut p = Proje { dosyalar, sira, takma, cati, komsu: vec![], sablon_sahibi: HashMap::new() };
        let mut secici: HashMap<&str, usize> = HashMap::new();
        for (di, d) in p.dosyalar.iter().enumerate() {
            for s in &d.seciciler {
                secici.entry(s.as_str()).or_insert(di);
            }
        }
        let mut komsu: Vec<Vec<usize>> = (0..p.dosyalar.len()).map(|di| p.ice_aktarilanlar(di)).collect();
        let mut sahip = HashMap::new();
        for (di, d) in p.dosyalar.iter().enumerate() {
            for (sinif, url) in &d.sablon_url {
                let h = d.yol.parent().map(|u| kanonik(&u.join(url))).and_then(|y| p.sira.get(&y).copied());
                if let Some(h) = h {
                    komsu[di].push(h);
                    sahip.insert(h, (di, sinif.clone()));
                }
            }
            komsu[di].extend(d.etiketler.iter().filter_map(|e| secici.get(e.as_str()).copied()).filter(|h| *h != di));
            komsu[di].sort();
            komsu[di].dedup();
        }
        p.komsu = komsu;
        p.sablon_sahibi = sahip;
        p
    }

    fn oku(kok: &Path) -> Proje {
        let cati = catilar(kok);
        let html = cati.contains("angular");
        let paketler = calisma_paketleri(kok);
        let mut yollar = dosyalar(kok, html);
        for (_, k) in &paketler {
            yollar.extend(dosyalar(k, html));
        }
        let dosyalar: Vec<Dosya> = {
            use rayon::prelude::*;
            yollar.par_iter().filter_map(|p| ayristir(p, kok)).collect()
        };
        Proje::kur(kok, dosyalar, paketler, cati)
    }

    /// Önbellekten (sezgi calistir): yalnızca içeriği değişen dosya yeniden ayrıştırılır. Okunan dosyalar (kaynaklar,
    /// tsconfig / jsconfig, ön yüzü tanıtan package.json) içerik özetleriyle ve yeniden ayrıştırılan sayısıyla döner
    fn oku_onbellekli(kok: &Path, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<Dosya>>) -> (Proje, Vec<(PathBuf, String)>, usize) {
        let cati = catilar(kok);
        let html = cati.contains("angular");
        // Çalışma alanı paketleri (ör. üretilmiş API istemcisi `@immich/sdk`): ön yüzün içe aktardığı kaynak da okunur
        let paketler = calisma_paketleri(kok);
        let mut yollar = dosyalar(kok, html);
        for (_, k) in &paketler {
            yollar.extend(dosyalar(k, html));
        }
        let (sonuclar, mut okunan, yeni) = crate::kesif::ayristir_hepsi(&yollar, onbellek, |p, b| ayristir_metin(p, kok, std::str::from_utf8(&b).ok()?));
        let dosyalar: Vec<Dosya> = sonuclar.into_iter().map(|(_, d)| d).collect();
        for ad in ["package.json", "tsconfig.json", "jsconfig.json", "tsconfig.app.json"] {
            if let Some(b) = guvenli::oku_bayt(&kok.join(ad)) {
                okunan.push((kok.join(ad), crate::kesif::icerik_ozeti(&b)));
            }
        }
        (Proje::kur(kok, dosyalar, paketler, cati), okunan, yeni)
    }

    fn modul(&self, di: usize, belirtec: &str) -> Option<usize> {
        let p = modul_coz(&self.dosyalar[di].yol, belirtec, &self.takma)?;
        self.sira.get(&std::fs::canonicalize(&p).unwrap_or(p)).copied()
    }

    /// Adın dosyada dışa aktarılan tanımına git: yerel tanım, `export { a as b } from`, `export * from`
    fn disari(&self, di: usize, ad: &str, derinlik: usize) -> Option<(usize, String)> {
        if derinlik > EN_DERIN {
            return None;
        }
        let d = &self.dosyalar[di];
        let bas = ad.split('.').next().unwrap_or(ad);
        let tanimli = d.fonksiyonlar.keys().any(|f| f == ad || f.split('.').next() == Some(bas))
            || d.cagrilar.iter().any(|c| c.sahip.as_deref().is_some_and(|s| s == ad || s.split('.').next() == Some(bas)))
            || d.baglamalar.contains_key(bas)
            || d.sabitler.contains_key(bas)
            || d.sabitler.contains_key(ad)
            || d.iceri.contains_key(bas)
            || rtk_kancasi(ad).is_some_and(|u| d.cagrilar.iter().any(|c| c.sahip.as_deref() == Some(&u)));
        if tanimli {
            return Some((di, ad.to_string()));
        }
        if let Some((b, asil)) = d.yeniden.get(bas) {
            let kalan = &ad[bas.len()..];
            return self.modul(di, b).and_then(|h| self.disari(h, &format!("{asil}{kalan}"), derinlik + 1));
        }
        d.yildiz.iter().filter_map(|b| self.modul(di, b)).find_map(|h| self.disari(h, ad, derinlik + 1))
    }

    /// Sabitin yol parçaları (yerel ya da içe aktarılmış)
    fn sabit(&self, di: usize, ad: &str, derinlik: usize) -> Option<Vec<Parca>> {
        if derinlik > EN_DERIN {
            return None;
        }
        let d = &self.dosyalar[di];
        if let Some(p) = d.sabitler.get(ad) {
            return Some(self.coz_yol(di, p, derinlik + 1));
        }
        // İçe aktarılmış sabit ya da alanı (`environment.apiUrl`)
        let bas = ad.split('.').next().unwrap_or(ad);
        let kalan = &ad[bas.len()..];
        let (b, asil) = d.iceri.get(bas)?;
        let h = self.modul(di, b)?;
        let hedef = match asil.as_str() {
            "default" => format!("{}{kalan}", self.dosyalar[h].varsayilan.clone()?),
            "*" => kalan.trim_start_matches('.').to_string(),
            a => format!("{a}{kalan}"),
        };
        let (h, asil) = self.disari(h, &hedef, derinlik + 1)?;
        self.sabit(h, &asil, derinlik + 1)
    }

    fn coz_yol(&self, di: usize, p: &[Parca], derinlik: usize) -> Vec<Parca> {
        p.iter()
            .flat_map(|x| match x {
                Parca::Ad(a) => self.sabit(di, a, derinlik).unwrap_or_else(|| vec![Parca::Bilinmeyen(a.rsplit('.').next().unwrap_or(a).to_string())]),
                y => vec![y.clone()],
            })
            .collect()
    }

    /// Bir başvurunun eriştiği istemci çağrıları ve rota modülü çağrıları (dosya, sıra)
    fn coz(&self, di: usize, ad: &str, derinlik: usize, ziyaret: &mut BTreeSet<(usize, String)>, out: &mut Ulasilan) {
        if derinlik > EN_DERIN || !ziyaret.insert((di, ad.to_string())) {
            return;
        }
        let d = &self.dosyalar[di];
        let bas = ad.split('.').next().unwrap_or(ad).to_string();
        // 1. Bu adla sahiplenilmiş istemci çağrısı (fonksiyon, sınıf metodu, RTK ucu)
        for (i, c) in d.cagrilar.iter().enumerate() {
            if c.sahip.as_deref() == Some(ad) {
                out.cagrilar.insert((di, i));
            }
        }
        for (i, c) in d.rota_cagrilari.iter().enumerate() {
            if c.sahip.as_deref() == Some(ad) {
                out.rota.insert((di, i));
            }
        }
        // 2. RTK kancası: useXMutation / useXQuery / useLazyXQuery -> x ucu
        if let Some(uc) = rtk_kancasi(ad) {
            for (i, c) in d.cagrilar.iter().enumerate() {
                if c.taniyici == "rtk-query" && c.sahip.as_deref() == Some(uc.as_str()) {
                    out.cagrilar.insert((di, i));
                }
            }
        }
        // 3. Kanca bağı (mutation.mutate -> mutation)
        if let Some(b) = d.baglamalar.get(&bas) {
            for r in b.clone() {
                self.coz(di, &r, derinlik + 1, ziyaret, out);
            }
        }
        // 4. Kancanın döndürdüğü bağ ("useAuth.loginMutation"): kancanın gövdesindeki aynı adlı bağ ya da fonksiyon;
        // bütün kanca gövdesine genişletilmez (aksi halde kancanın her çağrısı her eyleme bağlanırdı)
        if let Some(kuyruk) = ad.split_once('.').map(|x| x.1).filter(|x| !x.contains('.') && d.fonksiyonlar.contains_key(&bas) && !d.fonksiyonlar.contains_key(ad)) {
            if d.baglamalar.contains_key(kuyruk) || d.fonksiyonlar.contains_key(kuyruk) {
                self.coz(di, kuyruk, derinlik + 1, ziyaret, out);
                return;
            }
        }
        // 5. Yerel fonksiyon gövdesi
        for anahtar in [ad, bas.as_str()] {
            if let Some(b) = d.fonksiyonlar.get(anahtar) {
                for r in b.clone() {
                    self.coz(di, &r, derinlik + 1, ziyaret, out);
                }
            }
        }
        // 6. Sınıf alanı üzerinden çağrı (Angular servis enjeksiyonu): "Bilesen.servis.metot" -> "Servis.metot"
        let mut uc = ad.splitn(3, '.');
        if let (Some(a), Some(b), Some(m)) = (uc.next(), uc.next(), uc.next()) {
            if let Some(t) = d.alan_tipleri.get(&format!("{a}.{b}")) {
                self.coz(di, &format!("{t}.{m}"), derinlik + 1, ziyaret, out);
            }
        }
        // 7. İçe aktarılmış ad: hedef dosyada dışa aktarılan tanım
        if let Some((belirtec, asil)) = d.iceri.get(&bas) {
            if let Some(h) = self.modul(di, belirtec) {
                let kalan = &ad[bas.len()..];
                let yeni = match asil.as_str() {
                    "default" => self.dosyalar[h].varsayilan.clone().map(|v| format!("{v}{kalan}")),
                    "*" => Some(kalan.trim_start_matches('.').to_string()).filter(|x| !x.is_empty()),
                    a => Some(format!("{a}{kalan}")),
                };
                if let Some(yeni) = yeni {
                    if let Some((h2, ad2)) = self.disari(h, &yeni, derinlik + 1) {
                        self.coz(h2, &ad2, derinlik + 1, ziyaret, out);
                    }
                }
            }
        }
    }

    /// Çağrının çözülmüş yolu; Angular HttpClient'ın göreli adresi (`'api/heroes'`) uygulama köküne görelidir
    fn cagri_yolu(&self, di: usize, c: &HamCagri) -> Vec<Parca> {
        let mut y = self.coz_yol(di, &c.yol, 0);
        if c.taniyici == "angular-http" {
            if let Some(Parca::Metin(m)) = y.first_mut() {
                if !m.is_empty() && !m.starts_with('/') && !m.contains("://") {
                    m.insert(0, '/');
                }
            }
        }
        y
    }

    /// Dosyanın yerel komşuları (ulaşılabilirlik için): içe aktarmalar ve şablon bağları
    fn komsular(&self, di: usize) -> Vec<usize> {
        self.komsu.get(di).cloned().unwrap_or_default()
    }

    fn ice_aktarilanlar(&self, di: usize) -> Vec<usize> {
        let d = &self.dosyalar[di];
        let mut v: Vec<usize> = d.iceri.values().map(|(b, _)| b.as_str()).chain(d.yildiz.iter().map(String::as_str)).filter_map(|b| self.modul(di, b)).collect();
        v.extend(d.yeniden.values().filter_map(|(b, _)| self.modul(di, b)));
        v.sort();
        v.dedup();
        v
    }
}

/// `useCreateProductMutation` -> "createProduct"
fn rtk_kancasi(ad: &str) -> Option<String> {
    let govde = ad.strip_prefix("useLazy").or_else(|| ad.strip_prefix("use"))?;
    let govde = govde.strip_suffix("Mutation").or_else(|| govde.strip_suffix("Query"))?;
    let mut c = govde.chars();
    let ilk = c.next()?;
    Some(ilk.to_lowercase().collect::<String>() + c.as_str())
}

/// Yol parçalarından şablon: baştaki bilinmeyen taban adres atılır, diğer bilinmeyenler `{ad}` olur.
/// Dönen: (şablon, tamamen çözüldü mü: yol parametresi dışında bilinmeyen yok)
fn sablon(p: &[Parca]) -> Option<(String, bool)> {
    let mut s = String::new();
    let mut tam = true;
    for (i, x) in p.iter().enumerate() {
        match x {
            Parca::Metin(m) => s.push_str(m),
            Parca::Ad(a) | Parca::Bilinmeyen(a) => {
                if i == 0 && !p.get(1).is_some_and(|y| matches!(y, Parca::Metin(m) if !m.starts_with('/'))) {
                    // Taban adres (`${API}/items`): atılır
                    continue;
                }
                let ad: String = a.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
                if !s.ends_with('/') {
                    tam = false;
                }
                s.push_str(&format!("{{{}}}", if ad.is_empty() { "p".into() } else { ad }));
            }
        }
    }
    let s = crate::santral::aday(&s)?;
    Some((s, tam))
}

/// Haritadaki bir uç
pub struct Uc {
    pub yontem: String,
    pub yol: String,
    pub fonksiyon: String,
    pub dosya: String,
    pub satir: usize,
}

/// Bir istemci çağrısı (santral için: yalnızca yöntem ve yol)
pub struct Cagri {
    pub dosya: String,
    pub satir: usize,
    pub yontem: Option<String>,
    pub sablon: String,
}

/// Bir klasördeki JS/TS istemci çağrıları (santral bağları)
pub fn istemci_cagrilari(kok: &Path) -> Vec<Cagri> {
    let p = Proje::oku(kok);
    let mut v = vec![];
    for (di, d) in p.dosyalar.iter().enumerate() {
        for c in &d.cagrilar {
            if let Some((s, _)) = sablon(&p.cagri_yolu(di, c)) {
                v.push(Cagri { dosya: d.goreli.clone(), satir: c.satir, yontem: c.yontem_kesin.then(|| c.yontem.clone()), sablon: s });
            }
        }
    }
    v
}

/// Ön yüzün bağları
pub(crate) struct Baglar {
    /// `web-baglari.json` içeriği
    pub deger: Value,
    /// Okunan dosyalar ve içerik özetleri (haritanın okunan dosyalarına eklenir)
    pub okunan: Vec<(PathBuf, String)>,
    pub dosya: usize,
    pub ayristirilan: usize,
}

/// Ön yüzün bağları: ekranlar, UI eylemleri, istemci çağrıları ve grafik sözlüğündeki bağlar (`web-baglari.json`).
/// Ayrıştırma önbellekten geçer (`sezgi calistir`: yalnızca değişen ön yüz dosyası yeniden ayrıştırılır)
pub(crate) fn baglar(kok: &Path, ucler: &[Uc], genel_onek: &str, onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<Dosya>>) -> Baglar {
    let (p, okunan, ayristirilan) = Proje::oku_onbellekli(kok, onbellek);
    let dosya = p.dosyalar.len();
    let deger = baglar_kur(&p, ucler, genel_onek);
    Baglar { deger, okunan, dosya, ayristirilan }
}

fn baglar_kur(p: &Proje, ucler: &[Uc], genel_onek: &str) -> Value {
    let onek = crate::santral::parcalar(genel_onek);
    // İstemci çağrıları: kimlik, çözülmüş şablon, API eşleşmesi
    let mut cagri_id: HashMap<(usize, usize), String> = HashMap::new();
    let mut cagrilar = vec![];
    let mut kullanilan: BTreeSet<String> = BTreeSet::new();
    let (mut eslesen, mut eslesmeyen) = (0usize, 0usize);
    for (di, d) in p.dosyalar.iter().enumerate() {
        for (i, c) in d.cagrilar.iter().enumerate() {
            let Some((s, tam)) = sablon(&p.cagri_yolu(di, c)) else { continue };
            let mut id = match &c.sahip {
                Some(a) => format!("client:{a}"),
                None => format!("client:{}:{}", d.goreli, c.satir),
            };
            if !kullanilan.insert(id.clone()) {
                id = format!("{id}@{}:{}", d.goreli, c.satir);
                kullanilan.insert(id.clone());
            }
            let ap = crate::santral::parcalar(&s);
            let mut adaylar: Vec<(&Uc, bool)> = vec![];
            for u in ucler.iter().filter(|u| u.yontem == c.yontem) {
                let tam_yol = crate::santral::parcalar(&u.yol);
                if let Some(b) = crate::santral::eslesme(&ap, &tam_yol) {
                    adaylar.push((u, b));
                } else if !onek.is_empty() && tam_yol.starts_with(&onek) && crate::santral::eslesme(&ap, &tam_yol[onek.len()..]).is_some() {
                    adaylar.push((u, false));
                }
            }
            // İstemcinin taban adresi (oazapfts `defaults.baseUrl = '/api'`, axios `baseURL`) yolda yazılmaz: eşleşme
            // yoksa uçların başındaki en çok iki sabit parça (`/api`, `/api/v1`) atılarak denenir; tahmin
            if adaylar.is_empty() && !ap.is_empty() {
                for u in ucler.iter().filter(|u| u.yontem == c.yontem) {
                    let tam_yol = crate::santral::parcalar(&u.yol);
                    let fazla = tam_yol.len().saturating_sub(ap.len());
                    if (1..=2).contains(&fazla) && tam_yol[..fazla].iter().all(|p| !crate::santral::degisken_mi(p)) && crate::santral::eslesme(&ap, &tam_yol[fazla..]).is_some() {
                        adaylar.push((u, false));
                    }
                }
            }
            // Birebir eşleşen varsa onlar; şablonun parametresi değerle eşleştiyse tahmin
            let birebir: Vec<&(&Uc, bool)> = adaylar.iter().filter(|(_, b)| *b).collect();
            let secilen: Option<&(&Uc, bool)> = if birebir.len() == 1 { birebir.first().copied() } else { adaylar.first() };
            let kesin = c.yontem_kesin && tam && birebir.len() == 1 && adaylar.len() == 1;
            if secilen.is_some() {
                eslesen += 1;
            } else {
                eslesmeyen += 1;
            }
            cagri_id.insert((di, i), id.clone());
            cagrilar.push(json!({
                "id": id,
                "yontem": c.yontem,
                "yol": s,
                "taniyici": c.taniyici,
                "api": secilen.map(|(u, _)| format!("{} {}", u.yontem, u.yol)),
                "fonksiyon": secilen.map(|(u, _)| u.fonksiyon.clone()),
                "uc_kaynagi": secilen.map(|(u, _)| format!("{}:{}", u.dosya, u.satir)),
                "adaylar": adaylar.len(),
                "guven": if secilen.is_none() { "unknown" } else if kesin { "certain" } else { "inferred" },
                "kanit": { "tur": "SOURCE_CODE", "dosya": d.goreli, "satir": c.satir },
            }));
        }
    }
    // Ekranlar: rota tanımları ve bileşen dosyaları
    let mut ekranlar: Ekranlar = BTreeMap::new();
    for (di, d) in p.dosyalar.iter().enumerate() {
        for (yol, satir, bilesen) in &d.rotalar {
            let id = format!("screen:web/{yol}");
            let e = ekranlar.entry(id).or_insert_with(|| (d.goreli.clone(), *satir, BTreeSet::new()));
            // Kök: içe aktarılan bileşenin dosyası; bileşen bu dosyadaysa (TanStack dosya rotası) rota dosyasının kendisi.
            // Rota tablosu dosyası (bütün ekranları içe aktarır) kök sayılmaz: her eylem her ekrana düşmesin
            match bilesen.as_ref().and_then(|b| b.strip_prefix("import:").map(|m| p.modul(di, m)).unwrap_or_else(|| d.iceri.get(b).and_then(|(m, _)| p.modul(di, m)))) {
                Some(b) => e.2.insert(b),
                None => e.2.insert(di),
            };
        }
    }
    // Nesne biçimli rota dizileri (Angular, Vue Router, React Router, Solid) ve koruyucuları
    let mut korumalar: Korumalar = BTreeMap::new();
    nesne_ekranlari(p, &mut ekranlar, &mut korumalar);
    // SvelteKit (src/routes/**/+page.*), Next.js (app/**/page.*, pages/**; Nuxt pages/**)
    for (di, d) in p.dosyalar.iter().enumerate() {
        if let Some(y) = next_yolu(&d.goreli, p.cati.contains("next")) {
            ekranlar.entry(format!("screen:web/{y}")).or_insert_with(|| (d.goreli.clone(), 1, BTreeSet::new())).2.insert(di);
        }
    }
    // Remix / React Router v7 ve SolidStart dosya rotaları
    let rota_modulleri = remix_ekranlari(p, &mut ekranlar);
    solid_ekranlari(p, &mut ekranlar);
    // Ulaşılabilirlik: ekranın kök dosyalarından içe aktarma grafiği
    let mut dosya_ekranlari: HashMap<usize, BTreeSet<String>> = HashMap::new();
    for (id, (_, _, kokler)) in &ekranlar {
        let mut yigin: Vec<(usize, usize)> = kokler.iter().map(|k| (*k, 0)).collect();
        let mut gorulen = BTreeSet::new();
        while let Some((x, derin)) = yigin.pop() {
            if !gorulen.insert(x) || derin > 12 {
                continue;
            }
            dosya_ekranlari.entry(x).or_default().insert(id.clone());
            yigin.extend(p.komsular(x).into_iter().map(|y| (y, derin + 1)));
        }
    }
    // UI eylemleri: yalnızca bir istemci çağrısına ulaşanlar
    let mut eylemler = vec![];
    let mut baglar = vec![];
    let (mut cagrisiz, mut adsiz) = (0usize, 0usize);
    for (di, d) in p.dosyalar.iter().enumerate() {
        // Angular dış şablonundaki başvurular bileşen sınıfında çözülür ("kaydet()" -> "HeroesComponent.kaydet")
        let (cdi, sinif) = match p.sablon_sahibi.get(&di) {
            Some((t, s)) => (*t, Some(s.as_str())),
            None => (di, None),
        };
        for e in &d.eylemler {
            let mut out = Ulasilan::default();
            let mut ziyaret = BTreeSet::new();
            let mut rota_istekleri: Vec<(usize, &'static str, String)> = vec![];
            for r in &e.basvurular {
                if let Some(x) = r.strip_prefix("@rota:") {
                    let (t, y) = x.split_once(':').unwrap_or((x, ""));
                    rota_istekleri.push((di, if t == "loader" { "loader" } else { "action" }, y.to_string()));
                    continue;
                }
                let r = match sinif {
                    Some(s) => format!("{s}.{r}"),
                    None => r.clone(),
                };
                p.coz(cdi, &r, 0, &mut ziyaret, &mut out);
            }
            // Satır içi işleyicinin içindeki çağrılar
            if let Some((a, b)) = e.aralik {
                out.cagrilar.extend(d.cagrilar.iter().enumerate().filter(|(_, c)| (a..b).contains(&c.bayt)).map(|(i, _)| (di, i)));
                out.rota.extend(d.rota_cagrilari.iter().enumerate().filter(|(_, c)| (a..b).contains(&c.bayt)).map(|(i, _)| (di, i)));
            }
            // Rota modülü çağrıları (Remix / React Router form, fetcher, submit): hedef rotanın action / loader'ı
            let mut islenen = BTreeSet::new();
            loop {
                let mut yeni: Vec<(usize, &'static str, String)> = std::mem::take(&mut rota_istekleri);
                for &(f, i) in &out.rota {
                    if islenen.insert((f, i)) {
                        let rc = &p.dosyalar[f].rota_cagrilari[i];
                        yeni.push((f, rc.tur, rc.hedef.clone()));
                    }
                }
                if yeni.is_empty() {
                    break;
                }
                for (f, tur, hedef) in yeni {
                    for t in rota_modulu(&rota_modulleri, f, &hedef) {
                        for ad in if tur == "loader" { ["loader", "clientLoader"] } else { ["action", "clientAction"] } {
                            p.coz(t, ad, 0, &mut ziyaret, &mut out);
                        }
                    }
                }
            }
            let hedefler: Vec<String> = out.cagrilar.iter().filter_map(|x| cagri_id.get(x).cloned()).collect::<BTreeSet<_>>().into_iter().collect();
            if hedefler.is_empty() {
                cagrisiz += 1;
                continue;
            }
            let (ad, kaynak) = if e.ad.is_empty() {
                adsiz += 1;
                (e.basvurular.first().cloned().unwrap_or_default(), "isleyici")
            } else {
                (e.ad.clone(), e.ad_kaynagi)
            };
            let tur = if e.olay == "yukleme" { "yukleme" } else { e.rol.as_str() };
            let id = format!("ui:{}#{tur}:{ad}", d.goreli);
            let ekr: Vec<String> = dosya_ekranlari.get(&di).map(|s| s.iter().cloned().collect()).unwrap_or_default();
            let kanit = json!({ "tur": "SOURCE_CODE", "dosya": d.goreli, "satir": e.satir });
            for s in &ekr {
                baglar.push(json!({ "kaynak": s, "iliski": "DISPLAYS", "hedef": id, "guven": "inferred", "kanit": kanit }));
            }
            for h in &hedefler {
                baglar.push(json!({ "kaynak": id, "iliski": "TRIGGERS", "hedef": h, "guven": "inferred", "kanit": kanit }));
            }
            eylemler.push(json!({
                "id": id, "rol": e.rol, "ad": ad, "ad_kaynagi": kaynak, "olay": e.olay,
                "ekranlar": ekr, "cagrilar": hedefler, "guven": "inferred", "kanit": kanit,
            }));
        }
    }
    let gecisler = gecis_baglari(p, &ekranlar, &dosya_ekranlari);
    let gecis_sayisi = gecisler.len();
    baglar.extend(gecisler);
    for c in &cagrilar {
        if let Some(api) = c["api"].as_str() {
            baglar.push(json!({ "kaynak": c["id"], "iliski": "CALLS", "hedef": format!("api:{api}"), "guven": c["guven"], "kanit": c["kanit"] }));
        }
    }
    // Koruyucular: ekranın giriş gereksinimi (addan ve kaynak metninden tahmin, inferred) ve koruyucuya bağ
    let mut ekran_listesi: Vec<Value> = vec![];
    for (id, (dosya, satir, _)) in &ekranlar {
        let mut e = json!({ "id": id, "yol": &id["screen:web/".len()..], "kanit": { "tur": "SOURCE_CODE", "dosya": dosya, "satir": satir } });
        if let Some(k) = korumalar.get(id) {
            let turler: BTreeSet<&str> = k.values().map(|(m, _, _)| koruma_turu(m)).collect();
            let giris = if turler.contains("misafir") {
                "misafir"
            } else if turler.contains("gerekli") {
                "gerekli"
            } else {
                "bilinmiyor"
            };
            e["giris"] = json!(giris);
            e["korumalar"] = json!(k.keys().collect::<Vec<_>>());
            e["giris_guven"] = json!("inferred");
            for (ad, (_, kd, ks)) in k {
                baglar.push(json!({
                    "kaynak": id, "iliski": "REQUIRES_PERMISSION", "hedef": format!("guard:web/{ad}"), "guven": "inferred",
                    "kanit": { "tur": "SOURCE_CODE", "dosya": kd, "satir": ks },
                }));
            }
        }
        ekran_listesi.push(e);
    }
    json!({
        "surum": 1,
        "uretici": format!("sezgi {}", env!("CARGO_PKG_VERSION")),
        "ekranlar": ekran_listesi,
        "eylemler": eylemler,
        "istemci_cagrilari": cagrilar,
        "baglar": baglar,
        "istatistik": {
            "dosya": p.dosyalar.len(),
            "ekran": ekranlar.len(),
            "eylem": eylemler.len(),
            "cagrisiz_eylem": cagrisiz,
            "adsiz_eylem": adsiz,
            "istemci_cagrisi": cagrilar.len(),
            "ekran_gecisi": gecis_sayisi,
            "uca_baglanan": eslesen,
            "uca_baglanmayan": eslesmeyen,
            "korunan_ekran": korumalar.len(),
            "sunucu_fonksiyonu": p.dosyalar.iter().map(|d| d.sunucu_fonksiyonu).sum::<usize>(),
        },
        "not": "Bağ yoksa bilinmiyor demektir, yok demek değildir. Güven ölçeği certain > observed > inferred > unknown; UI ve ekran bağları statikte en fazla inferred.",
    })
}

/// Ekranlar: kimlik -> (tanım dosyası, satır, kök dosyalar)
type Ekranlar = BTreeMap<String, (String, usize, BTreeSet<usize>)>;
/// Ekranın koruyucuları: kimlik -> koruyucu adı -> (kaynak metni, dosya, satır)
type Korumalar = BTreeMap<String, BTreeMap<String, (String, String, usize)>>;

/// Bir eylemin ulaştığı istemci çağrıları ve rota modülü çağrıları: (dosya, sıra)
#[derive(Default)]
struct Ulasilan {
    cagrilar: BTreeSet<(usize, usize)>,
    rota: BTreeSet<(usize, usize)>,
}

/// Koruyucunun giriş anlamı (ad ve kaynak metninden tahmin): "gerekli", "misafir" (yalnızca girişsiz: login ekranı) ya da
/// "bilinmiyor"
fn koruma_turu(metin: &str) -> &'static str {
    let m = metin.to_ascii_lowercase().replace(char::is_whitespace, "");
    // `map(girdi => !girdi)`, `() => !auth.isLoggedIn()`: olumsuzlanmış giriş durumu
    const MISAFIR: [&str; 12] = ["guest", "anon", "noauth", "unauth", "notauth", "nonauth", "loggedout", "notlogged", "!isauth", "!auth", "!islogged", "=>!"];
    const GIRIS: [&str; 9] = ["auth", "login", "logged", "signedin", "session", "token", "admin", "role", "permission"];
    if MISAFIR.iter().any(|x| m.contains(x)) {
        "misafir"
    } else if GIRIS.iter().any(|x| m.contains(x)) {
        "gerekli"
    } else {
        "bilinmiyor"
    }
}

/// Bileşen adının kök dosyası: tembel modül (`import:./x`), içe aktarılmış ad ya da bu dosyada tanımlı bileşen
fn bilesen_dosyasi(p: &Proje, di: usize, b: &str) -> Option<usize> {
    if let Some(m) = b.strip_prefix("import:") {
        return p.modul(di, m);
    }
    let d = &p.dosyalar[di];
    if let Some((m, _)) = d.iceri.get(b) {
        return p.modul(di, m);
    }
    (d.bilesen_siniflari.contains(b) || d.fonksiyonlar.contains_key(b) || d.varsayilan.as_deref() == Some(b)).then_some(di)
}

/// Tembel alt rota dizileri (`loadChildren`): hedef modüldeki diziler (dışa aktarılan adla eşleşen öncelikli); hedefte dizi
/// yoksa onun içe aktardığı dosyalardakiler (NgModule'ün yönlendirme modülü)
fn tembel_diziler(p: &Proje, di: usize, belirtec: &str, ihrac: &Option<String>) -> Vec<(usize, usize)> {
    let Some(h) = p.modul(di, belirtec) else { return vec![] };
    let secilen = |f: usize| -> Vec<(usize, usize)> {
        let d = &p.dosyalar[f];
        let adli: Vec<(usize, usize)> = d.rota_dizileri.iter().enumerate().filter(|(_, (a, _))| ihrac.is_some() && a == ihrac).map(|(i, _)| (f, i)).collect();
        if adli.is_empty() {
            (0..d.rota_dizileri.len()).map(|i| (f, i)).collect()
        } else {
            adli
        }
    };
    let v = secilen(h);
    if !v.is_empty() {
        return v;
    }
    p.komsular(h).into_iter().flat_map(secilen).collect()
}

fn tembel_hedefler(p: &Proje, di: usize, r: &NesneRota, out: &mut BTreeSet<(usize, usize)>) {
    if let Some((b, ihrac)) = &r.tembel {
        out.extend(tembel_diziler(p, di, b, ihrac));
    }
    for c in &r.cocuklar {
        tembel_hedefler(p, di, c, out);
    }
}

/// Nesne biçimli rota dizilerinden ekranlar. Tembel yüklenen alt diziler (`loadChildren`) üst rotanın yoluyla okunur;
/// uygulamanın kök bileşeni (Angular bootstrap, Vue createApp: `router-outlet` / `router-view` taşıyan) her ekranın köküdür
fn nesne_ekranlari(p: &Proje, ekranlar: &mut Ekranlar, korumalar: &mut Korumalar) {
    let mut tuketilen: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (di, d) in p.dosyalar.iter().enumerate() {
        for (_, dizi) in &d.rota_dizileri {
            for r in dizi {
                tembel_hedefler(p, di, r, &mut tuketilen);
            }
        }
    }
    let uygulama: BTreeSet<usize> = p.dosyalar.iter().enumerate().flat_map(|(di, d)| d.kok_bilesenler.iter().filter_map(move |b| bilesen_dosyasi(p, di, b))).collect();
    for (di, d) in p.dosyalar.iter().enumerate() {
        for (ai, (_, dizi)) in d.rota_dizileri.iter().enumerate() {
            if tuketilen.contains(&(di, ai)) {
                continue;
            }
            let mut gorulen = BTreeSet::from([(di, ai)]);
            for r in dizi {
                let mut b = RotaGezisi { p, uygulama: &uygulama, gorulen: &mut gorulen, ekranlar, korumalar };
                b.gez(di, r, "", &BTreeSet::new(), &[]);
            }
        }
    }
}

/// Nesne rotası ağacında gezinti durumu
struct RotaGezisi<'a> {
    p: &'a Proje,
    uygulama: &'a BTreeSet<usize>,
    gorulen: &'a mut BTreeSet<(usize, usize)>,
    ekranlar: &'a mut Ekranlar,
    korumalar: &'a mut Korumalar,
}

impl RotaGezisi<'_> {
    /// `atalar`: üst rotaların (düzen) bileşen dosyaları; `ust_korumalar`: üst rotalardan gelen koruyucular
    fn gez(&mut self, di: usize, r: &NesneRota, ust: &str, atalar: &BTreeSet<usize>, ust_korumalar: &[(String, String, String, usize)]) {
        let p = self.p;
        let d = &p.dosyalar[di];
        let yol = match &r.yol {
            Some(y) if y.contains("**") => return,
            Some(y) => yol_birlestir(ust, y),
            None => yol_birlestir(ust, ""),
        };
        let mut koruma = ust_korumalar.to_vec();
        koruma.extend(r.korumalar.iter().map(|(a, m)| (a.clone(), m.clone(), d.goreli.clone(), r.satir)));
        let kok = r.bilesen.as_ref().map(|b| bilesen_dosyasi(p, di, b).unwrap_or(di));
        if !r.yonlendirme && (kok.is_some() || (!atalar.is_empty() && r.cocuklar.is_empty() && r.tembel.is_none())) {
            let id = format!("screen:web/{yol}");
            let e = self.ekranlar.entry(id.clone()).or_insert_with(|| (d.goreli.clone(), r.satir, BTreeSet::new()));
            e.2.extend(kok);
            e.2.extend(atalar.iter().copied());
            e.2.extend(self.uygulama.iter().copied());
            for (a, m, kd, ks) in &koruma {
                self.korumalar.entry(id.clone()).or_default().entry(a.clone()).or_insert_with(|| (m.clone(), kd.clone(), *ks));
            }
        }
        let mut alt_atalar = atalar.clone();
        alt_atalar.extend(kok);
        for c in &r.cocuklar {
            self.gez(di, c, &yol, &alt_atalar, &koruma);
        }
        if let Some((b, ihrac)) = &r.tembel {
            for (f, ai) in tembel_diziler(p, di, b, ihrac) {
                if self.gorulen.insert((f, ai)) {
                    for c in &p.dosyalar[f].rota_dizileri[ai].1 {
                        self.gez(f, c, &yol, &alt_atalar, &koruma);
                    }
                }
            }
        }
    }
}

/// Remix rota kimliğinin parçaları: "." ve "/" ayırır; `[.]` kaçışı ayırmaz (ayraçlar atılır)
fn remix_parcalari(kimlik: &str) -> Vec<String> {
    let mut v = vec![];
    let mut s = String::new();
    let mut kacis = false;
    for c in kimlik.chars() {
        match c {
            '[' => kacis = true,
            ']' => kacis = false,
            '.' | '/' if !kacis => v.push(std::mem::take(&mut s)),
            _ => s.push(c),
        }
    }
    v.push(s);
    v
}

/// Remix / React Router dosya rotası (`app/routes`): düz rotalar (`concerts.$city.tsx`, `_index`, yolsuz düzen `_auth.login`,
/// iç içe olmayan `concerts_.mine`, klasör rotası `concerts.$city/route.tsx`) ve iç içe klasörler (`concerts/$city.tsx`).
/// `klasor_rotasi`: klasörde route.* var mı (varsa klasördeki öteki dosyalar rota değildir). Dönen: (yol, kimlik parçaları)
fn remix_rotasi(goreli: &str, klasor_rotasi: impl Fn(&str) -> bool) -> Option<(String, Vec<String>)> {
    let a = goreli.strip_prefix("app/routes/")?;
    let (govde, uzanti) = a.rsplit_once('.')?;
    if !matches!(uzanti, "tsx" | "ts" | "jsx" | "js") || govde.ends_with(".server") || govde.ends_with(".client") {
        return None;
    }
    let govde = match govde.rsplit_once('/') {
        Some((klasor, "route")) => klasor,
        Some((klasor, _)) if klasor_rotasi(klasor) => return None,
        _ => govde,
    };
    let parcalar = remix_parcalari(govde);
    let mut yol = vec![];
    for s in &parcalar {
        if s == "_index" || s == "index" || s.starts_with('_') || (s.starts_with('(') && s.ends_with(')')) {
            continue;
        }
        let s = s.trim_end_matches('_');
        yol.push(match s.strip_prefix('$') {
            Some("") => "*".to_string(),
            Some(x) => format!(":{x}"),
            None => s.to_string(),
        });
    }
    Some((format!("/{}", yol.join("/")), parcalar))
}

/// Remix / React Router v7 ekranları: `app/routes.ts` yapılandırması ya da dosya rotaları. Varsayılan dışa aktarımı olmayan
/// rota modülü (kaynak rotası: yalnızca loader / action) ekran değildir ama form ve fetcher hedefidir. Kök düzen
/// (`app/root.tsx`) ve üst düzen rotaları ekranın da köküdür. Dönen: rota yolu -> rota modülleri
fn remix_ekranlari(p: &Proje, ekranlar: &mut Ekranlar) -> HashMap<String, Vec<usize>> {
    let mut moduller: HashMap<String, Vec<usize>> = HashMap::new();
    if !p.cati.contains("remix") {
        return moduller;
    }
    let goreli_sira: HashMap<&str, usize> = p.dosyalar.iter().enumerate().map(|(i, d)| (d.goreli.as_str(), i)).collect();
    let kok_duzen = ["app/root.tsx", "app/root.jsx", "app/root.ts", "app/root.js"].iter().find_map(|g| goreli_sira.get(g).copied());
    let belirtec = |m: &str| if m.starts_with('.') { m.to_string() } else { format!("./{m}") };
    let yapilandirmali: Vec<usize> = p.dosyalar.iter().enumerate().filter(|(_, d)| !d.yapilandirma_rotalari.is_empty()).map(|(i, _)| i).collect();
    for &di in &yapilandirmali {
        let d = &p.dosyalar[di];
        for (yol, satir, m, duzenler) in &d.yapilandirma_rotalari {
            let Some(md) = p.modul(di, &belirtec(m)) else { continue };
            moduller.entry(yol.clone()).or_default().push(md);
            if p.dosyalar[md].varsayilan.is_none() {
                continue;
            }
            let e = ekranlar.entry(format!("screen:web/{yol}")).or_insert_with(|| (d.goreli.clone(), *satir, BTreeSet::new()));
            e.2.insert(md);
            e.2.extend(kok_duzen);
            e.2.extend(duzenler.iter().filter_map(|x| p.modul(di, &belirtec(x))));
        }
    }
    if !yapilandirmali.is_empty() && !p.dosyalar.iter().any(|d| d.duz_rotalar) {
        return moduller;
    }
    let klasor_rotasi = |k: &str| ["tsx", "ts", "jsx", "js"].iter().any(|u| goreli_sira.contains_key(format!("app/routes/{k}/route.{u}").as_str()));
    let rotalar: Vec<(usize, String, Vec<String>)> = p.dosyalar.iter().enumerate().filter_map(|(di, d)| remix_rotasi(&d.goreli, klasor_rotasi).map(|(y, k)| (di, y, k))).collect();
    let kimlikler: HashMap<String, usize> = rotalar.iter().map(|(di, _, k)| (k.join("."), *di)).collect();
    for (di, yol, parcalar) in &rotalar {
        moduller.entry(yol.clone()).or_default().push(*di);
        let d = &p.dosyalar[*di];
        // Kaynak rotası ve yolsuz düzen (`_auth.tsx`) kendi başına ekran değildir
        if d.varsayilan.is_none() || parcalar.last().is_some_and(|s| s.starts_with('_') && s != "_index") {
            continue;
        }
        let e = ekranlar.entry(format!("screen:web/{yol}")).or_insert_with(|| (d.goreli.clone(), 1, BTreeSet::new()));
        e.2.insert(*di);
        e.2.extend(kok_duzen);
        for i in 1..parcalar.len() {
            if let Some(u) = kimlikler.get(&parcalar[..i].join(".")) {
                e.2.insert(*u);
            }
        }
    }
    moduller
}

/// Rota modülü çağrısının hedef modülleri: hedef yoksa çağrının dosyası; yoksa yolu eşleşen rota modülleri
fn rota_modulu(moduller: &HashMap<String, Vec<usize>>, di: usize, hedef: &str) -> Vec<usize> {
    if hedef.is_empty() {
        return vec![di];
    }
    let Some(y) = ic_yol(hedef) else { return vec![] };
    if let Some(v) = moduller.get(&y) {
        return v.clone();
    }
    let yollar: HashMap<&str, &str> = moduller.keys().map(|k| (k.as_str(), k.as_str())).collect();
    ekran_bul(&yollar, &y).and_then(|k| moduller.get(k)).cloned().unwrap_or_default()
}

/// SolidStart dosya rotaları (`src/routes/**.tsx|jsx`, varsayılan dışa aktarımlı): `[id]` -> `:id`, `(grup)` ve `index`
/// yolda yer almaz. Kök düzen (`src/app.tsx`) ve aynı adlı üst düzen dosyası (`users.tsx` + `users/`) ekranın da köküdür
fn solid_ekranlari(p: &Proje, ekranlar: &mut Ekranlar) {
    if !p.cati.contains("solidstart") {
        return;
    }
    let goreli_sira: HashMap<&str, usize> = p.dosyalar.iter().enumerate().map(|(i, d)| (d.goreli.as_str(), i)).collect();
    let kok_duzen = ["src/app.tsx", "src/app.jsx"].iter().find_map(|g| goreli_sira.get(g).copied());
    for (di, d) in p.dosyalar.iter().enumerate() {
        let Some(a) = d.goreli.strip_prefix("src/routes/") else { continue };
        let Some((govde, uzanti)) = a.rsplit_once('.') else { continue };
        if !matches!(uzanti, "tsx" | "jsx") || d.varsayilan.is_none() {
            continue;
        }
        let e = ekranlar.entry(format!("screen:web/{}", dosya_yolu(govde))).or_insert_with(|| (d.goreli.clone(), 1, BTreeSet::new()));
        e.2.insert(di);
        e.2.extend(kok_duzen);
        let mut ust = Path::new(govde).parent();
        while let Some(u) = ust.filter(|u| !u.as_os_str().is_empty()) {
            let u_ = u.to_string_lossy().replace('\\', "/");
            e.2.extend(["tsx", "jsx"].iter().filter_map(|x| goreli_sira.get(format!("src/routes/{u_}.{x}").as_str()).copied()));
            ust = u.parent();
        }
    }
}

/// Dosya tabanlı rotanın yolu: `x/[id]` -> /x/:id, `[...slug]` -> /:slug, `[[id=eslestirici]]` -> /:id; `(grup)` ve `index` atılır
fn dosya_yolu(s: &str) -> String {
    let v: Vec<String> = s
        .split('/')
        .filter(|p| !p.is_empty() && !(p.starts_with('(') && p.ends_with(')')) && *p != "index")
        .map(|p| {
            if p.starts_with('[') && p.ends_with(']') {
                let a = p.trim_start_matches('[').trim_end_matches(']').trim_start_matches("...");
                format!(":{}", a.split('=').next().unwrap_or(a))
            } else {
                p.to_string()
            }
        })
        .collect();
    format!("/{}", v.join("/"))
}

/// Next.js dosya rotası: app/x/[id]/page.tsx -> /x/:id; pages/x.tsx -> /x (yalnızca Next / Nuxt uygulamasında: `next`);
/// SvelteKit src/routes/**/+page.svelte
fn next_yolu(goreli: &str, next: bool) -> Option<String> {
    let g = goreli.strip_prefix("src/").unwrap_or(goreli);
    let parcala = dosya_yolu;
    // SvelteKit: src/routes/**/+page.svelte
    if let Some(a) = g.strip_prefix("routes/") {
        let (klasor, dosya) = a.rsplit_once('/').unwrap_or(("", a));
        return dosya.starts_with("+page.").then(|| parcala(klasor));
    }
    if !next {
        return None;
    }
    if let Some(a) = g.strip_prefix("app/") {
        let (klasor, dosya) = a.rsplit_once('/').unwrap_or(("", a));
        return dosya.starts_with("page.").then(|| parcala(klasor));
    }
    let a = g.strip_prefix("pages/")?;
    let ad = a.rsplit_once('.').map(|x| x.0).unwrap_or(a);
    if ad.starts_with("api/") || ad.starts_with('_') || ad.contains("/_") {
        return None;
    }
    Some(parcala(ad))
}

/// Ön yüz çatıları: package.json bağımlılığı
/// (okuma kapsamıyla aynı: her biri okunur)
const ON_YUZ_PAKETLERI: [&str; 12] =
    ["react", "react-dom", "next", "preact", "vue", "nuxt", "svelte", "@sveltejs/kit", "@angular/core", "solid-js", "@remix-run/react", "@solidjs/start"];
/// Yönlendiriciler (dosya tabanlı yönlendirmeli çatılar dahil): ekranı olan bir uygulamanın kanıtı
const YONLENDIRICI_PAKETLERI: [&str; 13] = [
    "react-router",
    "react-router-dom",
    "@tanstack/react-router",
    "wouter",
    "vue-router",
    "@angular/router",
    "next",
    "@sveltejs/kit",
    "@remix-run/react",
    "@react-router/dev",
    "@solidjs/router",
    "@solidjs/start",
    "solid-app-router",
];
/// Ön yüzü sunucusuyla birlikte taşıyan çatılar (Remix, React Router v7 çatı kipi, SolidStart): sunucu bağımlılığı
/// (ör. Remix'in Express bağdaştırıcısı) uygulamayı ön yüz olmaktan çıkarmaz
const TAM_YIGIN_PAKETLERI: [&str; 4] = ["@remix-run/react", "@react-router/dev", "@solidjs/start", "solid-start"];
/// Arka uç çatıları: bunlara bağımlı uygulama kökü kendisi ön yüz sayılmaz (ör. React ile e-posta şablonu yazan NestJS)
const ARKA_UC_PAKETLERI: [&str; 7] = ["@nestjs/core", "express", "fastify", "koa", "@hapi/hapi", "hono", "@adonisjs/core"];

/// package.json'daki bağımlılık adları (dependencies, devDependencies, peerDependencies)
fn bagimliliklar(k: &Path) -> std::collections::BTreeSet<String> {
    let pkg: serde_json::Value = guvenli::oku(&k.join("package.json")).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_default();
    ["dependencies", "devDependencies", "peerDependencies"].iter().filter_map(|b| pkg[*b].as_object()).flat_map(|o| o.keys().cloned()).collect()
}

/// Klasörde arayüz dosyası (.tsx, .jsx, .vue, .svelte, Angular `.component.ts|html`) var mı; derleme çıktısı ve bağımlılıklar atlanır, arama sınırlı
fn arayuz_dosyasi_var(k: &Path) -> bool {
    const ATLA: [&str; 8] = ["node_modules", ".git", "dist", "build", ".next", "out", "coverage", ".sezgi"];
    let mut yigin = vec![(k.to_path_buf(), 0usize)];
    let mut bakilan = 0usize;
    while let Some((d, derinlik)) = yigin.pop() {
        for g in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            bakilan += 1;
            if bakilan > 5000 {
                return false;
            }
            let p = g.path();
            let Ok(tur) = g.file_type() else { continue };
            if tur.is_dir() && derinlik < 6 && !ATLA.iter().any(|a| p.file_name().is_some_and(|n| n == *a)) {
                yigin.push((p, derinlik + 1));
            } else if tur.is_file()
                && (p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e, "tsx" | "jsx" | "vue" | "svelte"))
                    || p.file_name().and_then(|a| a.to_str()).is_some_and(|a| a.ends_with(".component.html") || a.ends_with(".component.ts")))
            {
                return true;
            }
        }
    }
    false
}

/// Ön yüz klasörü mü: bir ön yüz çatısına bağımlı VE ekranı olduğuna dair kanıt var (yönlendirici bağımlılığı ya da
/// arayüz dosyası). Yalnızca bağımlılık yetmez: arka uçlar da React'i e-posta şablonu için kullanır.
pub fn on_yuz_mu(k: &Path) -> bool {
    let b = bagimliliklar(k);
    ON_YUZ_PAKETLERI.iter().any(|p| b.contains(*p)) && (YONLENDIRICI_PAKETLERI.iter().any(|p| b.contains(*p)) || arayuz_dosyasi_var(k))
}

/// Uygulama kökü bir arka uç çatısına bağımlı mı (tam yığın ön yüz çatısı değilse)
fn arka_uc_mu(k: &Path) -> bool {
    let b = bagimliliklar(k);
    ARKA_UC_PAKETLERI.iter().any(|p| b.contains(*p)) && !TAM_YIGIN_PAKETLERI.iter().any(|p| b.contains(*p))
}

/// Monorepoda ön yüzü bulur: uygulamanın kendisi, altındaki ya da yanındaki frontend / client / web / ui klasörü.
/// Uygulamanın üst klasörüne yalnızca aynı git deposunun içindeyse (ya da depo yoksa bir kat) bakılır.
pub fn on_yuz_bul(kok: &Path, sinir: Option<&Path>) -> Option<PathBuf> {
    const ADLAR: [&str; 7] = ["frontend", "client", "web", "ui", "webapp", "front", "app"];
    // Uygulamanın kendisi: arka uç çatısına bağımlıysa ön yüz sayılmaz (yalnızca altı ve yanı aranır)
    if on_yuz_mu(kok) && !arka_uc_mu(kok) {
        return Some(kok.to_path_buf());
    }
    let mut yerler = vec![kok.to_path_buf()];
    if let Some(ust) = kok.parent().filter(|u| sinir.is_none_or(|s| u.starts_with(s))) {
        yerler.push(ust.to_path_buf());
    }
    for y in yerler {
        for a in ADLAR {
            let k = y.join(a);
            if k != kok && k.is_dir() && on_yuz_mu(&k) {
                return Some(k);
            }
        }
    }
    None
}
