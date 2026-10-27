//! Fonksiyon çağrı grafiği ve derin servis analizi. Uygulamanın kaynak dosyaları (TypeScript / JavaScript, Python, Java,
//! Kotlin, C#, Go, PHP, Ruby) tree-sitter ile bir kez daha okunur ve dilden bağımsız bir özete indirilir: sınıflar (üst türler,
//! tipli alanlar, kurucu parametreleri), fonksiyon ve yöntemler, gövdelerindeki çağrı yerleri, SQL dizgileri, eşitlik
//! karşılaştırmaları ve yetki işaretleri. Özetler önbelleklenir (dosya değişmediyse yeniden ayrıştırılmaz).
//!
//! Çağrı kenarı çözümü (güven ölçeği certain > observed > inferred > unknown):
//! - aynı sınıfın yöntemi (`this.m()`, `self.m()`, `$this->m()`, Go alıcısı), üst sınıf dahil; aynı dosyadaki ya da içe
//!   aktarılan fonksiyon; sınıf adıyla statik çağrı; tipi bilinen alan, parametre ya da yerel değişken üzerinden yöntem
//!   (NestJS / Spring / ASP.NET kurucu enjeksiyonu, Spring alan enjeksiyonu, FastAPI `Depends(Sinif)`, Laravel
//!   kapsayıcısı `app(Sinif::class)`, PHP yükseltilmiş kurucu parametresi) -> **certain**, yalnızca hedef statik ve tek
//!   ise;
//! - arayüz tipli alanın projedeki tek uygulaması, kapsayıcıdan çözülen sınıf ya da projede tek olan yöntem adı
//!   -> **inferred**;
//! - dizin erişimiyle, yansımayla ya da parametre olarak gelen fonksiyonla yapılan çağrı -> hedefi bilinmez:
//!   **unknown** olarak sayılır, kenar yazılmaz (yokluk iddia edilmez).
//!
//! Derin analiz: işlemin işleyici fonksiyonundan çağrı grafiği boyunca inilir; ulaşılan fonksiyonlardaki tablo etkileri
//! (Kysely, TypeORM, Sequelize, Mongoose, Prisma, SQLAlchemy / SQLModel, Django ORM, JPA / Spring Data, EF Core, GORM,
//! Eloquent, ActiveRecord, ham SQL dizgileri), erişim denetimi çağrıları (`requireAccess`, `authorize`, `@PreAuthorize`
//! ...) ve sahiplik karşılaştırmaları (kaydın sahibi ile istekteki kimlik) toplanır. Zincir güveni yoldaki en zayıf
//! kenardır; tablo etkisi statik çıkarım olduğu için en fazla inferred.
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

use crate::kanit::Guven;

/// Çağrı grafiği biçim sürümü (`ek.cagri`)
pub const BICIM: u64 = 1;
/// Bir fonksiyonda tutulacak en fazla çağrı yeri (dev fonksiyonlar belleği doldurmasın)
const EN_FAZLA_CAGRI: usize = 400;
/// İşlemden inilecek en fazla derinlik ve ziyaret edilecek en fazla fonksiyon
pub const EN_DERIN: usize = 8;
pub const EN_FAZLA_FONKSIYON: usize = 400;
/// Exposed tablo tabanları (`object Gorevler : LongIdTable("gorevler")`)
const EXPOSED: [&str; 8] = ["Table", "IdTable", "IntIdTable", "LongIdTable", "UUIDTable", "UIntIdTable", "ULongIdTable", "CompositeIdTable"];
/// Argüman metninin en fazla uzunluğu
const ARGUMAN_UZUNLUGU: usize = 160;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Dil {
    Ts,
    Py,
    Java,
    Cs,
    Go,
    Php,
    Rb,
    /// Kotlin (Spring Boot, Ktor)
    Kt,
}

impl Dil {
    pub fn uzantidan(p: &Path) -> Option<Dil> {
        let ad = p.file_name()?.to_str()?;
        if ad.ends_with(".d.ts") {
            return None;
        }
        Some(match p.extension()?.to_str()? {
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => Dil::Ts,
            "py" => Dil::Py,
            "java" => Dil::Java,
            "cs" => Dil::Cs,
            "go" => Dil::Go,
            "php" => Dil::Php,
            "rb" => Dil::Rb,
            "kt" => Dil::Kt,
            _ => return None,
        })
    }
}

/// Gövdedeki bir çağrı yeri (ham): alıcı zinciri (`this.repo.save` -> ["this", "repo"]), yöntem adı, satır ve kısaltılmış
/// argümanlar. `this` / `self` / `$this` / Go alıcısı "this" olarak yazılır; karmaşık alıcı "<ifade>"
#[derive(Clone, Debug, Default)]
pub struct HamCagri {
    pub alici: Vec<String>,
    pub ad: String,
    pub satir: usize,
    pub argumanlar: Vec<String>,
    /// Kurucu çağrısı (`new X()`, Python / Ruby `X()` / `X.new`)
    pub yeni: bool,
    /// Hedef koddan çıkarılamaz: dizin erişimi, yansıma, `send`, değişken yöntem adı
    pub dinamik: bool,
}

#[derive(Clone, Debug, Default)]
pub struct HamFonk {
    pub ad: String,
    pub sinif: Option<usize>,
    pub satir: usize,
    pub son: usize,
    /// Adsız ve hiçbir fonksiyonun içinde olmayan (ör. rota kaydına verilen) fonksiyon
    pub anonim: bool,
    /// (ad, tip); tip yoksa boş
    pub parametreler: Vec<(String, String)>,
    /// Yerel değişkenler: (ad, tip ya da atanan ifadenin başı "=X.find")
    pub yereller: Vec<(String, String)>,
    pub cagrilar: Vec<HamCagri>,
    /// Ham SQL dizgilerinden: (tablo, yazma mı, satır)
    pub sql: Vec<(String, bool, usize)>,
    /// Eşitlik karşılaştırmaları: (kısaltılmış metin, satır, koşulda mı). Yalnızca bir koşulun (`if`, `unless`, `elif`)
    /// ya da bir çağrı argümanının (sorgu süzgeci) parçası olanlar tutulur; yanıt eşlemesindeki `isOwner: a === b` sayılmaz
    pub karsilastirmalar: Vec<(String, usize, bool)>,
    /// Yöntemin yetki işaretleri (`@PreAuthorize(...)`, `[Authorize(...)]`): (metin, satır)
    pub isaretler: Vec<(String, usize)>,
}

#[derive(Clone, Debug, Default)]
pub struct HamSinif {
    pub ad: String,
    pub satir: usize,
    /// Üst sınıf ve arayüzler (ham tip metni, ör. "JpaRepository<User, Long>")
    pub ustler: Vec<String>,
    pub arayuz: bool,
    /// Tipli alanlar (kurucu enjeksiyonu dahil): (ad, tip)
    pub alanlar: Vec<(String, String)>,
    /// Açık tablo adı ve kaynağı (`@Table({ name })`, `@Entity('x')`, `__tablename__`, `$table`, `table_name`)
    pub tablo: Option<(String, &'static str)>,
    /// Sınıf düzeyindeki işaretler (dekoratör / anotasyon / öznitelik adları)
    pub isaretler: Vec<String>,
    /// Python `class X(SQLModel, table=True)`
    pub tablo_bayragi: bool,
    /// Go `TableName()` yönteminin döndürdüğü ad
    pub go_tablo: Option<String>,
    /// Rails denetleyici süzgeçleri (`before_action :x, only: [:a]`): (yöntem, only, except)
    pub on_cagrilar: Vec<(String, Vec<String>, Vec<String>)>,
}

/// Bir kaynak dosyanın dilden bağımsız özeti (önbelleklenir)
#[derive(Clone, Debug, Default)]
pub struct DosyaOzeti {
    pub dil: Option<Dil>,
    /// Java paketi, C# ad alanı, PHP ad alanı, Go paket adı
    pub paket: String,
    /// İçe aktarmalar: (yerel ad, modül / tam ad, asıl ad)
    pub iceaktarma: Vec<(String, String, String)>,
    pub siniflar: Vec<HamSinif>,
    pub fonksiyonlar: Vec<HamFonk>,
    /// Enum ve sabit metin değerleri: "Permission.AssetDelete" -> "asset.delete"
    pub sabitler: Vec<(String, String)>,
    /// Değişkene atanan model: `const User = mongoose.model('User', ...)` / `sequelize.define('user', ...)` -> (değişken,
    /// tablo, kaynak)
    pub model_atamalari: Vec<(String, String, &'static str)>,
    /// Bağımlılık kaydı: arayüz -> sınıf (ASP.NET `AddScoped<IArayuz, Sinif>()`, Laravel `$this->app->bind(I::class, C::class)`)
    pub di_kayitlari: Vec<(String, String)>,
    /// İstek yöntemine göre ayrılan dallar (`if request.method == "POST":`)
    pub yontem_dallari: Vec<YontemDali>,
}

/// İstek yöntemine bağlı bir dal: koşul satırı, gövdenin ilk ve son satırı, yöntemler (büyük harf), koşul olumlu mu
/// (`==` / `in` / `isMethod`), koşul yalnızca yöntem karşılaştırması mı, gövde `return` / `raise` / `throw` ile mi bitiyor,
/// `else` mi; aynı `if / elif / else` zincirinin dalları aynı zincir numarasını taşır
#[derive(Clone, Debug, Default)]
pub struct YontemDali {
    pub kosul: usize,
    pub bas: usize,
    pub son: usize,
    pub yontemler: Vec<String>,
    pub pozitif: bool,
    pub saf: bool,
    pub donuyor: bool,
    pub degilse: bool,
    pub zincir: usize,
}

/// Koşuldaki yöntem karşılaştırması: (yöntemler, olumlu mu, koşul yalnızca bu karşılaştırma mı). `or` / `||` ile kurulan
/// koşul belirsizdir (None)
fn yontem_kosulu(kosul: &str) -> Option<(Vec<String>, bool, bool)> {
    static D: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let [ifade, is_method, tirnak] = D.get_or_init(|| {
        [
            regex::Regex::new(
                r#"(?:\$_SERVER\[['"]REQUEST_METHOD['"]\]|(?:\$this->request|\$request|\bself\.request|\brequest|\breq)\s*(?:\.|->)\s*(?:method\b(?:\(\))?|getMethod\(\)))\s*(===|==|!==|!=|not\s+in\b|in\b)\s*([^&|]*)"#,
            )
            .unwrap(),
            regex::Regex::new(r#"(!\s*)?(?:\$request|\$this->request|\brequest)\s*->\s*isMethod\(\s*['"](\w+)['"]\s*\)"#).unwrap(),
            regex::Regex::new(r#"['"](\w+)['"]"#).unwrap(),
        ]
    });
    let k = kosul.trim();
    if k.contains(" or ") || k.contains("||") {
        return None;
    }
    let (yontemler, pozitif, eslesme) = if let Some(c) = ifade.captures(k) {
        let pozitif = matches!(c[1].trim(), "===" | "==" | "in");
        let ys: Vec<String> = tirnak.captures_iter(&c[2]).map(|t| t[1].to_ascii_uppercase()).collect();
        (ys, pozitif, c.get(0).unwrap().as_str().trim().to_string())
    } else {
        let c = is_method.captures(k)?;
        (vec![c[2].to_ascii_uppercase()], c.get(1).is_none(), c.get(0).unwrap().as_str().to_string())
    };
    if yontemler.is_empty() || !yontemler.iter().all(|y| matches!(y.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS")) {
        return None;
    }
    let kalan = k.replace(&eslesme, "");
    let saf = !kalan.contains(" and ") && !kalan.contains("&&") && kalan.trim_matches(|c: char| c == '(' || c == ')' || c.is_whitespace()).is_empty();
    Some((yontemler, pozitif, saf))
}

/// Dosyadaki istek yöntemine göre ayrılan dalları satır düzeyinde bulur (Django / Flask: `if request.method == "POST":`,
/// girintiyle). Süslü parantezli diller için de çalışır (`if (req.method === 'POST') {`) ama bu sürümde yalnızca Python
/// dosyalarına uygulanır
pub fn yontem_dallari(dil: Dil, metin: &str) -> Vec<YontemDali> {
    if dil != Dil::Py || !metin.contains("method") {
        return vec![];
    }
    let satirlar: Vec<&str> = metin.lines().collect();
    let girinti = |s: &str| s.len() - s.trim_start().len();
    let bos = |s: &str| {
        let t = s.trim();
        t.is_empty() || t.starts_with('#') || t.starts_with("//")
    };
    let donus = |s: &str| {
        let t = s.trim_start();
        ["return", "raise ", "throw ", "abort(", "abort ", "exit"].iter().any(|d| t.starts_with(d))
    };
    let mut v: Vec<YontemDali> = vec![];
    let mut zincir = 0usize;
    // Son dalın (girinti, son satır): `elif` / `else` aynı zincire ancak hemen ardından gelirse katılır
    let mut onceki: Option<(usize, usize)> = None;
    for i in 0..satirlar.len() {
        let s = satirlar[i];
        let t = s.trim_start();
        let g = girinti(s);
        let (devam, kosul, degilse) = if dil == Dil::Py {
            if let Some(r) = t.strip_prefix("elif ") {
                (true, r.rsplit_once(':').map(|x| x.0.to_string()), false)
            } else if let Some(r) = t.strip_prefix("if ") {
                (false, r.rsplit_once(':').map(|x| x.0.to_string()), false)
            } else if t.starts_with("else:") || t.starts_with("else :") {
                (true, None, true)
            } else {
                continue;
            }
        } else {
            let u = t.trim_start_matches('}').trim_start();
            let ac = u.trim_end().ends_with('{');
            if let Some(r) = u.strip_prefix("else if").or_else(|| u.strip_prefix("elseif")).filter(|_| ac) {
                (true, Some(r.trim().trim_end_matches('{').trim().to_string()), false)
            } else if let Some(r) = u.strip_prefix("if").filter(|r| ac && r.trim_start().starts_with('(')) {
                (u.len() != t.len(), Some(r.trim().trim_end_matches('{').trim().to_string()), false)
            } else if ac && u.strip_prefix("else").is_some_and(|r| r.trim() == "{") {
                (true, None, true)
            } else {
                continue;
            }
        };
        // Gövdenin son satırı (0 tabanlı)
        let son = if dil == Dil::Py {
            let mut son = i;
            for (j, x) in satirlar.iter().enumerate().skip(i + 1) {
                if bos(x) {
                    continue;
                }
                if girinti(x) <= g {
                    break;
                }
                son = j;
            }
            son
        } else {
            let mut derinlik = 0i32;
            let mut son = i;
            'd: for (j, x) in satirlar.iter().enumerate().skip(i) {
                let x = if j == i { t.trim_start_matches('}') } else { *x };
                for c in x.chars() {
                    match c {
                        '{' => derinlik += 1,
                        '}' => {
                            derinlik -= 1;
                            if derinlik == 0 {
                                son = j.saturating_sub(1).max(i);
                                break 'd;
                            }
                        }
                        _ => {}
                    }
                }
            }
            son
        };
        let zincire_katilir = devam && onceki.is_some_and(|(og, oson)| og == g && oson + 2 >= i);
        if !zincire_katilir {
            zincir += 1;
        }
        let dal = match (degilse, kosul.as_deref().and_then(yontem_kosulu)) {
            (true, _) if zincire_katilir => Some(YontemDali { degilse: true, ..Default::default() }),
            (false, Some((yontemler, pozitif, saf))) => Some(YontemDali { yontemler, pozitif, saf, ..Default::default() }),
            // Yöntemle ilgisiz koşul zinciri kırar
            _ => None,
        };
        match dal {
            Some(mut d) => {
                let son_govde = (i + 1..=son).rev().find(|j| !bos(satirlar[*j]) && satirlar[*j].trim() != "}");
                d.kosul = i + 1;
                d.bas = i + 2;
                d.son = son + 1;
                d.donuyor = son_govde.is_some_and(|j| donus(satirlar[j]));
                d.zincir = zincir;
                v.push(d);
                onceki = Some((g, son));
            }
            None => onceki = None,
        }
    }
    v
}

/// İstek yöntemi `yontem` olan işlemde fonksiyonun (ilk, son satır) aralığında çalışmayan satır aralıkları: başka bir
/// yöntemin dalı, yöntemi kesin eşleşen daldan sonraki `elif` / `else` dalları ve eşleşen dal dönüyorsa zincirden sonrası
pub fn yontem_disi(dallar: &[YontemDali], bas: usize, son: usize, yontem: &str) -> Vec<(usize, usize)> {
    let yontem = yontem.to_ascii_uppercase();
    let mut haric = vec![];
    let icinde: Vec<&YontemDali> = dallar.iter().filter(|d| d.kosul > bas && d.son <= son).collect();
    let mut zincirler: Vec<usize> = icinde.iter().map(|d| d.zincir).collect();
    zincirler.dedup();
    for z in zincirler {
        let zd: Vec<&YontemDali> = icinde.iter().copied().filter(|d| d.zincir == z).collect();
        let mut eslesen: Option<&YontemDali> = None;
        for d in &zd {
            if eslesen.is_some() {
                haric.push((d.kosul, d.son));
            } else if !d.degilse {
                if d.yontemler.contains(&yontem) != d.pozitif {
                    haric.push((d.bas, d.son));
                } else if d.saf {
                    eslesen = Some(d);
                }
            }
        }
        if let (Some(e), Some(sonuncu)) = (eslesen, zd.last()) {
            if e.donuyor && sonuncu.son < son {
                haric.push((sonuncu.son + 1, son));
            }
        }
    }
    haric
}

// ---- Yardımcılar ----

fn metin(n: Node, k: &[u8]) -> String {
    n.utf8_text(k).unwrap_or("").to_string()
}

fn satir(n: Node) -> usize {
    n.start_position().row + 1
}

fn alan<'t>(n: Node<'t>, ad: &str) -> Option<Node<'t>> {
    n.child_by_field_name(ad)
}

fn cocuklar(n: Node) -> Vec<Node> {
    let mut w = n.walk();
    n.named_children(&mut w).collect()
}

/// Tek satıra indirilmiş, kısaltılmış metin
fn kisa(s: &str, n: usize) -> String {
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s
    }
}

fn tirnaksiz(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix(['b', 'r', 'f', 'u', '@', '$']).filter(|r| r.starts_with(['"', '\''])).unwrap_or(s);
    s.trim_matches(|c| c == '\'' || c == '"' || c == '`').to_string()
}

/// Tip metninin taban adı ve tip argümanları: "Repository<User>" -> ("Repository", ["User"]), "Optional[Item]" ->
/// ("Item", []), "*service.UserService" -> ("UserService", []), "App\\Models\\User" -> ("User", [])
pub fn tip_parcala(t: &str) -> (String, Vec<String>) {
    let mut t = t.trim().trim_start_matches(':').trim().trim_start_matches(['*', '&', '?', '[', ']']).trim().to_string();
    // Python Annotated[X, Depends(...)] / Optional[X] / Union[X, None]: ilk argüman
    for sar in ["Annotated[", "Optional[", "Union[", "Mapped[", "Type[", "type["] {
        if let Some(ic) = t.strip_prefix(sar) {
            t = ic.split([',', ']']).next().unwrap_or("").trim().to_string();
        }
    }
    let t = t.split('|').next().unwrap_or("").trim().to_string();
    let t = t.trim_end_matches(['?', '!']).trim_end_matches("[]").to_string();
    let (taban, args) = match t.find(['<', '[']) {
        Some(i) => {
            let ic = t[i + 1..].trim_end_matches(['>', ']']);
            let mut args = vec![];
            let (mut derinlik, mut bas) = (0i32, 0usize);
            for (j, c) in ic.char_indices() {
                match c {
                    '<' | '[' => derinlik += 1,
                    '>' | ']' => derinlik -= 1,
                    ',' if derinlik == 0 => {
                        args.push(tip_parcala(&ic[bas..j]).0);
                        bas = j + 1;
                    }
                    _ => {}
                }
            }
            args.push(tip_parcala(&ic[bas..]).0);
            (t[..i].to_string(), args.into_iter().filter(|a| !a.is_empty()).collect())
        }
        None => (t.clone(), vec![]),
    };
    let son = taban.rsplit(['.', '\\', ':']).next().unwrap_or(&taban).trim().to_string();
    (son, args)
}

/// Ad karşılaştırması: büyük/küçük harf ve alt çizgi duyarsız ("create_item" ~ "createItem")
pub fn ad_esit(a: &str, b: &str) -> bool {
    let n = |s: &str| s.chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect::<String>();
    n(a) == n(b)
}

/// İngilizce çoğul (ORM kuralı: tablo adı model adının çoğulu)
fn cogul(s: &str) -> String {
    if s.ends_with('y') && !s.ends_with("ay") && !s.ends_with("ey") && !s.ends_with("oy") {
        format!("{}ies", &s[..s.len() - 1])
    } else if s.ends_with('s') || s.ends_with('x') || s.ends_with("ch") || s.ends_with("sh") {
        format!("{s}es")
    } else if s == "person" {
        "people".into()
    } else {
        format!("{s}s")
    }
}

/// "BlogPost" -> "blog_post"
fn yilan(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// SQL dizgisindeki tablolar: (tablo, yazma mı). Dizgi tam bir SQL deyimiyle başlamalı (`SELECT … FROM`, `INSERT INTO`,
/// `UPDATE x SET`, `DELETE FROM`, `WITH x AS (`): "Update own user" gibi düz metin SQL sayılmaz. FROM / JOIN okunan, INTO /
/// UPDATE / DELETE FROM yazılan tablodur
pub fn sql_tablolari(s: &str) -> Vec<(String, bool)> {
    static D: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let [bas, okuma, yazma] = D.get_or_init(|| {
        [
            regex::Regex::new(
                r#"(?is)^\s*\(?\s*(select\s.+?\sfrom\s|insert\s+(ignore\s+)?into\s|update\s+[\w."`\[\]]+\s+set\s|delete\s+from\s|with\s+\w+\s+as\s*\(|merge\s+into\s|replace\s+into\s)"#,
            )
            .unwrap(),
            regex::Regex::new(r#"(?i)\b(?:from|join)\s+([A-Za-z_"`\[][\w."`\[\]]*)"#).unwrap(),
            regex::Regex::new(r#"(?i)(?:\binsert\s+(?:ignore\s+)?into|\breplace\s+into|\bmerge\s+into|^\s*update|\bdelete\s+from)\s+([A-Za-z_"`\[][\w."`\[\]]*)"#).unwrap(),
        ]
    });
    if s.len() > 20_000 || !bas.is_match(s) {
        return vec![];
    }
    let temiz = |t: &str| -> Option<String> {
        let t = t.rsplit('.').next().unwrap_or(t).trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']').to_string();
        let kucuk = t.to_ascii_lowercase();
        (!t.is_empty()
            && !matches!(kucuk.as_str(), "select" | "where" | "set" | "values" | "lateral" | "unnest" | "dual" | "only" | "the" | "a" | "an" | "json_each" | "generate_series"))
        .then_some(t)
    };
    let mut v: Vec<(String, bool)> = vec![];
    for c in yazma.captures_iter(s) {
        if let Some(t) = temiz(&c[1]) {
            if !v.iter().any(|x| x.0 == t && x.1) {
                v.push((t, true));
            }
        }
    }
    for c in okuma.captures_iter(s) {
        if let Some(t) = temiz(&c[1]) {
            if !v.iter().any(|x| x.0 == t) {
                v.push((t, false));
            }
        }
    }
    v
}

/// MyBatis deyimi: (eşleyici arayüzü, deyim kimliği, tablolar (tablo, yazma mı))
pub type MybatisDeyimi = (String, String, Vec<(String, bool)>);

/// MyBatis XML eşleyicisindeki deyimler: (ad alanının son parçası = eşleyici arayüzü, deyim kimliği, tablolar).
/// `<sql id>` parçaları `<include refid>` yerine konur; `<set>` / `<where>` SQL sözcüğüne çevrilir, diğer etiketler atılır
pub fn mybatis_eslemeleri(metin: &str) -> Vec<MybatisDeyimi> {
    static D: std::sync::OnceLock<[regex::Regex; 5]> = std::sync::OnceLock::new();
    let [ad_alani, deyim, parca, dahil, etiket] = D.get_or_init(|| {
        [
            regex::Regex::new(r#"<mapper\s+namespace\s*=\s*"([^"]+)""#).unwrap(),
            regex::Regex::new(r#"(?s)<(select|insert|update|delete)\b[^>]*\bid\s*=\s*"([^"]+)"[^>]*>(.*?)</(?:select|insert|update|delete)>"#).unwrap(),
            regex::Regex::new(r#"(?s)<sql\b[^>]*\bid\s*=\s*"([^"]+)"[^>]*>(.*?)</sql>"#).unwrap(),
            regex::Regex::new(r#"<include\b[^>]*\brefid\s*=\s*"([^"]+)"[^>]*/?>"#).unwrap(),
            regex::Regex::new(r"(?s)<!\[CDATA\[|\]\]>|<!--.*?-->|<[^>]+>").unwrap(),
        ]
    });
    let Some(ns) = ad_alani.captures(metin).map(|c| c[1].rsplit('.').next().unwrap_or("").to_string()) else { return vec![] };
    let parcalar: HashMap<&str, &str> = parca.captures_iter(metin).filter_map(|c| Some((c.get(1)?.as_str(), c.get(2)?.as_str()))).collect();
    let mut v = vec![];
    for c in deyim.captures_iter(metin) {
        let mut govde = c[3].to_string();
        for _ in 0..3 {
            govde = dahil.replace_all(&govde, |m: &regex::Captures| parcalar.get(&m[1]).map(|x| format!(" {x} ")).unwrap_or_default()).into_owned();
        }
        let govde = govde.replace("<set>", " set ").replace("<where>", " where ").replace("&lt;", "<").replace("&gt;", ">");
        let duz = etiket.replace_all(&govde, " ");
        let mut tablolar = sql_tablolari(&duz);
        // `<update id>` deyimi yazmadır; okuma deyiminin tabloları okunur
        if matches!(&c[1], "insert" | "update" | "delete") && tablolar.iter().all(|t| !t.1) {
            if let Some(t) = tablolar.first_mut() {
                t.1 = true;
            }
        }
        if !tablolar.is_empty() {
            v.push((ns.clone(), c[2].to_string(), tablolar));
        }
    }
    v
}

/// Erişim denetimi çağrısı mı (ad ve alıcıyla): `requireAccess`, `checkAccess`, `authorize`, `$user->can(...)`,
/// `Gate::allows`, `can?`, `authorize!`, `AuthorizeAsync`, `checkOwnablePermission`, `has_object_permission` ...
pub fn erisim_cagrisi(alici: &[String], ad: &str) -> bool {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| {
        regex::Regex::new(r"^(require|check|assert|ensure|verify|enforce|authori[sz]e|guard|validate|user)[a-z]*(access|permission|owner|ownership|ownable|policy|ability|can|authori[sz]ation)[a-z]*$").unwrap()
    });
    let a: String = ad.trim_end_matches(['?', '!']).chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect();
    // `belongsTo` / `belongsToMany` / `belongs_to`: ORM ilişki tanımı, denetim değil
    if a.is_empty() || kimlik_saglayici_cagrisi(alici, &a) || matches!(a.as_str(), "belongsto" | "belongstomany") {
        return false;
    }
    let gate = alici.last().is_some_and(|r| matches!(r.as_str(), "Gate" | "Policy" | "Authorization"));
    match a.as_str() {
        "authorize"
        | "authorizeforuser"
        | "authorizeresource"
        | "authorizeasync"
        | "usercan"
        | "hasobjectpermission"
        | "checkobjectpermissions"
        | "haspermission"
        | "checkpermission"
        | "checkpermissions" => true,
        "can" | "cannot" => ad.ends_with('?') || !alici.is_empty(),
        "allows" | "denies" | "check" | "inspect" | "any" | "none" => gate,
        // canEditArticle / canWriteArticle / isOwner / isAuthorOf / ownsRecord
        _ if ["canread", "canwrite", "canedit", "canupdate", "candelete", "canview", "canaccess", "canmanage", "canmodify", "canremove", "cansee"]
            .iter()
            .any(|o| a.starts_with(o))
            || ["isowner", "isauthor", "iscreator", "owns", "belongsto"].iter().any(|o| a.starts_with(o)) =>
        {
            true
        }
        _ => re.is_match(&a),
    }
}

/// OAuth / kimlik sağlayıcı istemcisinin akış çağrısı (`authorize_access_token`, `authorize_redirect`,
/// `authorization_url`, `fetch_token`, `Socialite::driver()->user()`, `passport.authenticate`): kimliği alır ya da
/// yönlendirir, istekteki kimliği bir kayda karşı denetleyip reddetmez; erişim denetimi değildir. `a`: küçük harfli,
/// alt çizgisiz ad
fn kimlik_saglayici_cagrisi(alici: &[String], a: &str) -> bool {
    let alici_kimlik = alici.iter().any(|r| {
        let r = r.to_ascii_lowercase();
        ["oauth", "socialite", "passport", "provider", "openid", "oidc", "sso", "saml", "keycloak", "auth0", "okta", "cognito"].iter().any(|x| r.contains(x))
    });
    alici_kimlik
        || a.starts_with("oauth")
        || ((a.contains("authori") || a.contains("authenticat"))
            && ["token", "redirect", "url", "uri", "code", "callback", "grant", "request", "login"].iter().any(|x| a.contains(x)))
}

/// Çatının bilinen yetki API'si mi: kararı çatı verir, istekteki kimliği kayda ya da izne karşı denetler ve reddeder
/// (`$this->authorize`, `Gate::allows`, `$user->can`, `can?` / `authorize!`, `AuthorizeAsync`,
/// `has_object_permission`). Projenin kendi adlandırdığı denetim (`requireAccess`, `canEditArticle`) adla tanınır:
/// en fazla inferred
fn bilinen_yetki_api(alici: &[String], ad: &str) -> bool {
    let a: String = ad.trim_end_matches(['?', '!']).chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect();
    if kimlik_saglayici_cagrisi(alici, &a) {
        return false;
    }
    let gate = alici.last().is_some_and(|r| matches!(r.as_str(), "Gate" | "Policy" | "Authorization"));
    match a.as_str() {
        "authorize" | "authorizeforuser" | "authorizeresource" | "authorizeasync" | "hasobjectpermission" | "checkobjectpermissions" => true,
        "can" | "cannot" => ad.ends_with('?') || !alici.is_empty(),
        "allows" | "denies" | "check" | "inspect" | "any" | "none" => gate,
        _ => false,
    }
}

/// Erişim denetimi nesne düzeyinde mi: adı erişim / sahiplik söylüyor ya da argümanlarında kayıt (kimlik, nesne) var
fn nesne_duzeyi(ad: &str, argumanlar: &[String]) -> bool {
    let a = ad.to_ascii_lowercase();
    if a.contains("owner") || a.contains("ownable") || a.contains("access") || a.contains("object") {
        return true;
    }
    let sabit = |s: &str| {
        let s = s.trim();
        s.is_empty()
            || s.starts_with(['\'', '"', ':', '`'])
            || s.parse::<f64>().is_ok()
            || s.chars().next().is_some_and(char::is_uppercase) && !s.contains('(')
            || matches!(s, "auth" | "user" | "request" | "$request" | "req" | "current_user" | "User" | "self.request")
    };
    argumanlar.iter().skip(1).any(|x| !sabit(x)) || argumanlar.first().is_some_and(|x| x.contains("ids") || x.contains(" id"))
}

/// Sahiplik karşılaştırması: bir taraf kaydın sahip alanı (`item.owner_id`, `article.author.id`, `$post->user_id`,
/// `@article.user`), diğer taraf istekteki kimlik (`current_user.id`, `auth.user.id`, `req.user.id`, `auth()->id()`,
/// `Auth::id()`, `request.user`) ya da 403 / 404 ile reddedilen bir koşulda çağıranın kimliğini taşıyan ad (`userId`).
/// Kayıt kimliğinin yol parametresiyle karşılaştırılması (`existing_user.id != user_id`) ya da çağıranın kendisiyle
/// karşılaştırılması (`auth.user.id === id`, "kendi hesabını silemezsin") sahiplik değildir
pub fn sahiplik_karsilastirmasi(kosul: &str, durum: Option<u16>) -> bool {
    sahiplik_kimligi(kosul, durum).is_some()
}

/// Sahiplik karşılaştırması ve istek tarafının nasıl tanındığı: Some(true) kimlik açıkça istekten (`current_user`,
/// `auth.user`, `req.user` ...), Some(false) yalnızca adından (`userId`); sahiplik değilse None
pub fn sahiplik_kimligi(kosul: &str, durum: Option<u16>) -> Option<bool> {
    static D: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let [ayrac, sahip, istek] = D.get_or_init(|| {
        [
            regex::Regex::new(r"&&|\|\||\band\b|\bor\b|\bAND\b|\bOR\b").unwrap(),
            regex::Regex::new(r"(?i)^(owner|author|createdby|creator|createdbyid|creatorid|ownerid|authorid|userid|user|sahip|sahipid|yazar|yazarid|olusturan|kullaniciid)$").unwrap(),
            regex::Regex::new(
                r"(?i)(current_?user|currentuser|\bauth\b|auth\(\)|auth::|authuser|\breq\b\s*\.\s*user|\brequest\s*(\.|->)\s*user|\$request->user|session\s*(\.|\[)\s*.?user|principal|identity|claims?\b|getuserid|logged_?in_?user|\bviewer\b|\bme\b|actor|securitycontext|getauthentication)",
            )
            .unwrap(),
        ]
    });
    for parca in ayrac.split(kosul) {
        let p = parca.trim().trim_start_matches(['(', '!']).trim_start_matches("not ").trim_end_matches(')').trim();
        let op = ["!==", "===", "!=", "==", " is not ", " is "].iter().find_map(|o| p.find(o).map(|i| (i, o.len())));
        let Some((i, l)) = op else { continue };
        let (sol, sag) = (p[..i].trim().trim_matches(['(', ')']), p[i + l..].trim().trim_matches(['(', ')']));
        let sahip_mi = |s: &str| -> bool {
            if istek.is_match(s) {
                return false;
            }
            // Java / C# erişimcisi `getAuthor()` -> `author`
            let parcalar: Vec<String> = s
                .split(['.', '>', '-', '[', ']', '\'', '"'])
                .map(|x| x.trim_matches(['@', '$', '(', ')', ' ', '?', '!']))
                .filter(|x| !x.is_empty())
                .map(|x| match x.strip_prefix("get").filter(|r| r.starts_with(|c: char| c.is_ascii_uppercase())) {
                    Some(r) => r.to_string(),
                    None => x.to_string(),
                })
                .collect();
            if parcalar.len() < 2 {
                return false;
            }
            let sade = |x: &str| x.chars().filter(|c| *c != '_').collect::<String>();
            let son = sade(&parcalar[parcalar.len() - 1]);
            let onceki = sade(&parcalar[parcalar.len() - 2]);
            // `item.owner_id`, `article.author.id`, `article.Author.PersonId`
            sahip.is_match(&son) || (son.to_ascii_lowercase().ends_with("id") && sahip.is_match(&onceki) && parcalar.len() >= 3)
        };
        // Kimlik: açıkça istekten (Some(true)) ya da çağıranın kimliğini taşıyan yalın addan (Some(false))
        let cagiran = |s: &str| -> Option<bool> {
            if istek.is_match(s) {
                Some(true)
            } else {
                (regex_kimlik(s, durum) || dolayli_kimlik(s, durum)).then_some(false)
            }
        };
        if sahip_mi(sol) {
            if let Some(k) = cagiran(sag) {
                return Some(k);
            }
        }
        if sahip_mi(sag) {
            if let Some(k) = cagiran(sol) {
                return Some(k);
            }
        }
    }
    None
}

/// Çağırandan türetilmiş bir kaydın kimliği (`articleUserModel.ID`, `myUser.id`, `currentMember.Id`): adında kullanıcı
/// ya da çağıran geçen değişkenin `id` alanı. Yalnızca 401 / 403 ile reddedilen ya da durumu bilinmeyen koşulda sayılır
fn dolayli_kimlik(s: &str, durum: Option<u16>) -> bool {
    if !matches!(durum, None | Some(401 | 403)) {
        return false;
    }
    let s = s.trim().trim_end_matches("()");
    let Some((on, son)) = s.rsplit_once(['.', '>']) else { return false };
    let on = on.trim_end_matches('-').trim_end_matches('?').rsplit(['.', '>']).next().unwrap_or("").to_ascii_lowercase();
    son.eq_ignore_ascii_case("id") && ["user", "my", "member", "account", "person", "viewer", "kullanici", "uye"].iter().any(|x| on.contains(x))
}

/// Çağıranın kimliğini taşıyan yalın ad: `userId`, `user_id`, `uid`, `currentUserId`, `kullaniciId`. Yalın `id` ve
/// `user` / `user.id` (Pundit / CanCanCan politikasında geçerli kullanıcı) yalnızca 401/403/404 ile reddedilen koşulda
/// (karşı taraf kaydın sahip alanıyken) sayılır
fn regex_kimlik(s: &str, durum: Option<u16>) -> bool {
    let s: String = s.trim().trim_start_matches(['$', '@']).chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect();
    let red = matches!(durum, Some(401 | 403 | 404));
    matches!(s.as_str(), "userid" | "uid" | "currentuserid" | "authid" | "requesterid" | "callerid" | "loggedinuserid" | "kullaniciid" | "oturumkullaniciid")
        || (red && matches!(s.as_str(), "id" | "user" | "user.id" | "user->id" | "kullanici" | "kullanici.id"))
}

/// Karşılaştırmada işlemin yol parametrelerinden biri (`:user_id`, `{userId}`, `<int:pk>`) yalın ad olarak geçiyor mu
fn yol_parametresiyle(ifade: &str, yol: &str) -> bool {
    let sade = |x: &str| x.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    let parametreler: Vec<String> = yol
        .split('/')
        .filter_map(|p| p.strip_prefix(':').or_else(|| p.strip_prefix('{').map(|x| x.trim_end_matches('}'))).or_else(|| p.strip_prefix('<').map(|x| x.trim_end_matches('>'))))
        .map(|p| sade(p.rsplit(':').next().unwrap_or(p)))
        .filter(|p| !p.is_empty())
        .collect();
    // Yalın ad: önünde nokta / ok olmayan tanımlayıcı (`Item.owner_id` alanın adıdır, parametre değil)
    let mut onceki = ' ';
    let mut ad = String::new();
    let mut adlar: Vec<(String, bool)> = vec![];
    let mut alan = false;
    for c in ifade.chars().chain([' ']) {
        if c.is_alphanumeric() || c == '_' || c == '$' {
            if ad.is_empty() {
                alan = matches!(onceki, '.' | '>');
            }
            ad.push(c);
        } else {
            if !ad.is_empty() {
                adlar.push((std::mem::take(&mut ad), alan));
            }
            onceki = c;
        }
    }
    adlar.iter().any(|(a, alan)| !alan && parametreler.contains(&sade(a)))
}

/// Dekoratör / anotasyon argümanındaki tablo adı: `{ name: 'x' }`, `{ tableName: 'x' }`, `name = "x"`
fn ad_ozelligi(s: &str) -> Option<String> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r#"(?:\bname|\btableName)\s*[:=]\s*['"`]([^'"`]+)['"`]"#).unwrap());
    re.captures(s).map(|c| c[1].to_string())
}

/// Metindeki `Sinif.Uye` biçimli sabit atıfları (enum üyesi: `Permission.AssetDelete`)
fn sabit_atiflari(s: &str) -> Vec<&str> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r"[A-Z][A-Za-z0-9_]*\.[A-Za-z0-9_]+").unwrap());
    re.find_iter(s).map(|m| m.as_str()).collect()
}

/// Karşılaştırmanın bağlamı: bir koşulun parçasıysa Some(true), bir çağrı argümanının parçasıysa Some(false), başka bir
/// yerdeyse (atama, nesne değeri, dönüş) None
fn kosul_baglami(n: Node) -> Option<bool> {
    let mut x = n;
    for _ in 0..10 {
        let u = x.parent()?;
        match u.kind() {
            "parenthesized_expression"
            | "unary_expression"
            | "not_operator"
            | "boolean_operator"
            | "binary_expression"
            | "binary"
            | "prefix_unary_expression"
            | "parenthesized_statements"
            | "condition"
            | "comparison_operator" => x = u,
            "if_statement" | "elif_clause" | "if" | "elsif" | "unless" | "if_modifier" | "unless_modifier" | "while_statement" | "guard_statement" | "if_expression"
            | "when_entry" => return Some(true),
            "arguments" | "argument_list" | "argument" | "keyword_argument" | "value_argument" | "value_arguments" => return Some(false),
            _ => return None,
        }
    }
    None
}

// ---- Çıkarım: dil başına düğüm türleri ----

/// Dosyayı ayrıştırıp özetler (önbellek anahtarı içerik özetidir)
pub fn ayristir(yol: &Path, b: Vec<u8>) -> Option<DosyaOzeti> {
    let dil = Dil::uzantidan(yol)?;
    let mut p = Parser::new();
    let lang: tree_sitter::Language = match dil {
        Dil::Ts if yol.extension().is_some_and(|e| e == "tsx" || e == "jsx") => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Dil::Ts => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Dil::Py => tree_sitter_python::LANGUAGE.into(),
        Dil::Java => tree_sitter_java::LANGUAGE.into(),
        Dil::Cs => tree_sitter_c_sharp::LANGUAGE.into(),
        Dil::Go => tree_sitter_go::LANGUAGE.into(),
        Dil::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        Dil::Rb => tree_sitter_ruby::LANGUAGE.into(),
        Dil::Kt => tree_sitter_kotlin_ng::LANGUAGE.into(),
    };
    p.set_language(&lang).ok()?;
    let agac = p.parse(&b, None)?;
    let mut c = Cikarici { k: &b, dil, oz: DosyaOzeti { dil: Some(dil), ..Default::default() } };
    c.gez(agac.root_node());
    c.oz.yontem_dallari = yontem_dallari(dil, &String::from_utf8_lossy(&b));
    Some(c.oz)
}

struct Cikarici<'a> {
    k: &'a [u8],
    dil: Dil,
    oz: DosyaOzeti,
}

/// Gezinme bağlamı: içinde bulunulan sınıf, fonksiyon ve Go alıcısının adı
#[derive(Clone, Copy)]
struct Baglam<'t> {
    sinif: Option<usize>,
    fonk: Option<usize>,
    alici: Option<Node<'t>>,
}

impl<'a> Cikarici<'a> {
    fn m(&self, n: Node) -> String {
        metin(n, self.k)
    }

    fn sinif_ac(&mut self, ad: String, n: Node, ustler: Vec<String>, arayuz: bool) -> usize {
        self.oz.siniflar.push(HamSinif { ad, satir: satir(n), ustler, arayuz, ..Default::default() });
        self.oz.siniflar.len() - 1
    }

    fn fonk_ac(&mut self, ad: String, n: Node, sinif: Option<usize>, anonim: bool) -> usize {
        self.oz.fonksiyonlar.push(HamFonk { ad, sinif, satir: satir(n), son: n.end_position().row + 1, anonim, ..Default::default() });
        self.oz.fonksiyonlar.len() - 1
    }

    fn gez(&mut self, kok: Node) {
        let mut yigin: Vec<(Node, Baglam)> = vec![(kok, Baglam { sinif: None, fonk: None, alici: None })];
        while let Some((n, b)) = yigin.pop() {
            let mut b2 = b;
            let devam = self.dugum(n, b, &mut b2);
            if !devam {
                continue;
            }
            let bas = yigin.len();
            let mut w = n.walk();
            for c in n.named_children(&mut w) {
                yigin.push((c, b2));
            }
            yigin[bas..].reverse();
        }
    }

    /// Bir düğümü işler; çocuklarına geçilecek bağlamı `alt`a yazar. false: çocuklara inilmez
    fn dugum<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match self.dil {
            Dil::Ts => self.ts(n, b, alt),
            Dil::Py => self.py(n, b, alt),
            Dil::Java => self.java(n, b, alt),
            Dil::Cs => self.cs(n, b, alt),
            Dil::Go => self.go(n, b, alt),
            Dil::Php => self.php(n, b, alt),
            Dil::Rb => self.rb(n, b, alt),
            Dil::Kt => self.kt(n, b, alt),
        }
    }

    // ---- Ortak kayıtlar ----

    fn cagri_ekle(&mut self, b: Baglam, c: HamCagri) {
        if let Some(f) = b.fonk {
            let fk = &mut self.oz.fonksiyonlar[f];
            if fk.cagrilar.len() < EN_FAZLA_CAGRI {
                fk.cagrilar.push(c);
            }
        }
    }

    fn dizgi(&mut self, b: Baglam, n: Node) {
        let Some(f) = b.fonk else { return };
        let s = self.m(n);
        if s.len() < 12 {
            return;
        }
        let ic = tirnaksiz(&s);
        for (t, y) in sql_tablolari(&ic) {
            let fk = &mut self.oz.fonksiyonlar[f];
            if fk.sql.len() < 64 && !fk.sql.iter().any(|x| x.0 == t && x.1 == y) {
                fk.sql.push((t, y, satir(n)));
            }
        }
    }

    fn karsilastirma(&mut self, b: Baglam, n: Node) {
        self.karsilastirma_metni(b, n, metin(n, self.k));
    }

    fn karsilastirma_metni(&mut self, b: Baglam, n: Node, m: String) {
        let Some(f) = b.fonk else { return };
        let Some(kosul) = kosul_baglami(n) else { return };
        let fk = &mut self.oz.fonksiyonlar[f];
        if fk.karsilastirmalar.len() < 64 {
            fk.karsilastirmalar.push((kisa(&m, 160), satir(n), kosul));
        }
    }

    fn yerel(&mut self, b: Baglam, ad: String, tip: String) {
        if let Some(f) = b.fonk {
            if !ad.is_empty() && !tip.is_empty() && self.oz.fonksiyonlar[f].yereller.len() < 128 {
                self.oz.fonksiyonlar[f].yereller.push((ad, tip));
            }
        }
    }

    fn sinif_alani(&mut self, b: Baglam, ad: String, tip: String) {
        if let Some(s) = b.sinif {
            let ad = ad.trim_start_matches(['$', '@']).to_string();
            if !ad.is_empty() && !tip.is_empty() && !self.oz.siniflar[s].alanlar.iter().any(|x| x.0 == ad) {
                self.oz.siniflar[s].alanlar.push((ad, tip));
            }
        }
    }

    fn argumanlar(&self, n: Option<Node>) -> Vec<String> {
        n.map(|a| {
            cocuklar(a)
                .into_iter()
                .filter(|x| !x.kind().contains("comment"))
                .take(6)
                .map(|x| {
                    // C# / PHP / Ruby argüman sarmalayıcısı
                    let x = if matches!(x.kind(), "argument") { x.named_child(0).unwrap_or(x) } else { x };
                    kisa(&self.m(x), ARGUMAN_UZUNLUGU)
                })
                .collect()
        })
        .unwrap_or_default()
    }

    /// Adsız fonksiyon (geri çağırma, kapanış, blok): bir fonksiyonun içindeyse onu çevreleyen fonksiyonun parçasıdır
    /// (çağrıları ona yazılır); hiçbir fonksiyonun içinde değilse (rota kaydına verilen işleyici) kendi adsız fonksiyonudur
    fn fonksiyona_gir<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> Option<usize> {
        if b.fonk.is_some() {
            return None;
        }
        let f = self.fonk_ac(format!("<anonim:{}>", satir(n)), n, b.sinif, true);
        alt.fonk = Some(f);
        Some(f)
    }

    // ---- TypeScript / JavaScript ----

    fn ts_zincir(&self, n: Node) -> Vec<String> {
        let mut parcalar = vec![];
        let mut x = n;
        for _ in 0..12 {
            match x.kind() {
                "identifier" | "property_identifier" | "shorthand_property_identifier" | "private_property_identifier" => {
                    parcalar.push(self.m(x));
                    break;
                }
                "this" => {
                    parcalar.push("this".into());
                    break;
                }
                "super" => {
                    parcalar.push("super".into());
                    break;
                }
                "member_expression" => {
                    if let Some(p) = alan(x, "property") {
                        parcalar.push(self.m(p).trim_start_matches('#').to_string());
                    }
                    match alan(x, "object") {
                        Some(o) => x = o,
                        None => break,
                    }
                }
                "parenthesized_expression" | "non_null_expression" | "await_expression" | "as_expression" | "satisfies_expression" => match x.named_child(0) {
                    Some(i) => x = i,
                    None => break,
                },
                _ => {
                    parcalar.push("<ifade>".into());
                    break;
                }
            }
        }
        parcalar.reverse();
        parcalar
    }

    fn ts_parametreler(&mut self, f: usize, ps: Option<Node>, kurucu: bool, b: Baglam) {
        let Some(ps) = ps else { return };
        for p in cocuklar(ps) {
            if !matches!(p.kind(), "required_parameter" | "optional_parameter") {
                continue;
            }
            let ad = alan(p, "pattern").map(|x| self.m(x)).unwrap_or_default();
            let tip = alan(p, "type").map(|t| self.m(t).trim_start_matches(':').trim().to_string()).unwrap_or_default();
            // Kurucu parametre özelliği (`private readonly repo: Repo`) sınıf alanıdır
            if kurucu && cocuklar(p).iter().any(|c| c.kind() == "accessibility_modifier" || c.kind() == "readonly") {
                self.sinif_alani(b, ad.clone(), tip.clone());
            }
            self.oz.fonksiyonlar[f].parametreler.push((ad, tip));
        }
    }

    fn ts<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match n.kind() {
            "comment" => return false,
            "import_statement" => {
                let modul = alan(n, "source").map(|s| tirnaksiz(&self.m(s))).unwrap_or_default();
                let mut yigin = vec![n];
                while let Some(x) = yigin.pop() {
                    match x.kind() {
                        "import_specifier" => {
                            let asil = alan(x, "name").map(|a| self.m(a)).unwrap_or_default();
                            let yerel = alan(x, "alias").map(|a| self.m(a)).unwrap_or_else(|| asil.clone());
                            self.oz.iceaktarma.push((yerel, modul.clone(), asil));
                        }
                        "namespace_import" => {
                            if let Some(i) = x.named_child(0) {
                                self.oz.iceaktarma.push((self.m(i), modul.clone(), "*".into()));
                            }
                        }
                        "import_clause" => {
                            for c in cocuklar(x) {
                                if c.kind() == "identifier" {
                                    self.oz.iceaktarma.push((self.m(c), modul.clone(), "default".into()));
                                } else {
                                    yigin.push(c);
                                }
                            }
                        }
                        _ => yigin.extend(cocuklar(x)),
                    }
                }
                return false;
            }
            "enum_declaration" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                if let Some(g) = alan(n, "body") {
                    for u in cocuklar(g) {
                        if u.kind() == "enum_assignment" {
                            if let (Some(a), Some(v)) = (alan(u, "name"), alan(u, "value")) {
                                if matches!(v.kind(), "string" | "template_string") {
                                    self.oz.sabitler.push((format!("{ad}.{}", self.m(a)), tirnaksiz(&self.m(v))));
                                }
                            }
                        }
                    }
                }
                return false;
            }
            "class_declaration" | "abstract_class_declaration" | "class" | "interface_declaration" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                if ad.is_empty() {
                    return true;
                }
                let mut ustler = vec![];
                for c in cocuklar(n) {
                    if matches!(c.kind(), "class_heritage" | "extends_type_clause") {
                        for h in cocuklar(c) {
                            for t in cocuklar(h) {
                                ustler.push(self.m(t));
                            }
                        }
                        if c.kind() == "extends_type_clause" {
                            ustler.extend(cocuklar(c).into_iter().map(|t| self.m(t)));
                        }
                    }
                }
                let s = self.sinif_ac(ad, n, ustler, n.kind() == "interface_declaration");
                // Sınıf dekoratörleri: export ifadesindekiler dahil
                let mut dek = vec![];
                for c in cocuklar(n).into_iter().chain(n.parent().filter(|p| p.kind() == "export_statement").map(cocuklar).unwrap_or_default()) {
                    if c.kind() == "decorator" {
                        dek.push(c);
                    }
                }
                for d in dek {
                    let ic = d.named_child(0);
                    let (dad, arg) = match ic {
                        Some(c) if c.kind() == "call_expression" => {
                            (alan(c, "function").map(|f| self.m(f)).unwrap_or_default(), alan(c, "arguments").and_then(|a| a.named_child(0)).map(|a| self.m(a)).unwrap_or_default())
                        }
                        Some(c) => (self.m(c), String::new()),
                        None => continue,
                    };
                    self.oz.siniflar[s].isaretler.push(dad.clone());
                    // @Entity('ad') / @Entity({ name: 'ad' }) / @Table({ name: 'ad' }) / @Table({ tableName: 'ad' })
                    if matches!(dad.as_str(), "Entity" | "Table") && !arg.is_empty() {
                        let t = if arg.starts_with('{') { ad_ozelligi(&arg) } else { Some(tirnaksiz(&arg)).filter(|x| !x.is_empty() && !x.contains(['(', ' '])) };
                        if let Some(t) = t {
                            self.oz.siniflar[s].tablo = Some((t, if dad == "Entity" { "@Entity" } else { "@Table" }));
                        }
                    }
                }
                alt.sinif = Some(s);
                alt.fonk = None;
            }
            "method_definition" | "method_signature" | "abstract_method_signature" => {
                // Sınıf yöntemi ya da üst düzey nesne değişmezinin yöntemi (`export default { list() {...} }`)
                let ad = alan(n, "name").map(|x| self.m(x).trim_start_matches('#').to_string()).unwrap_or_default();
                if b.fonk.is_none() {
                    let f = self.fonk_ac(ad.clone(), n, b.sinif, false);
                    self.ts_parametreler(f, alan(n, "parameters"), ad == "constructor", b);
                    alt.fonk = Some(f);
                }
            }
            "pair" if b.fonk.is_none() => {
                // module.exports = { liste: async (req, res) => {...} }: anahtar fonksiyonun adıdır
                if let (Some(k), Some(v)) = (alan(n, "key"), alan(n, "value")) {
                    if matches!(v.kind(), "arrow_function" | "function_expression" | "function") {
                        let f = self.fonk_ac(tirnaksiz(&self.m(k)), n, b.sinif, false);
                        self.ts_parametreler(f, alan(v, "parameters"), false, b);
                        alt.fonk = Some(f);
                    }
                }
            }
            "public_field_definition" | "property_signature" => {
                let ad = alan(n, "name").map(|x| self.m(x).trim_start_matches('#').to_string()).unwrap_or_default();
                if let Some(t) = alan(n, "type") {
                    self.sinif_alani(b, ad.clone(), self.m(t).trim_start_matches(':').trim().to_string());
                }
                if let Some(v) = alan(n, "value") {
                    if matches!(v.kind(), "arrow_function" | "function_expression") && b.fonk.is_none() {
                        let f = self.fonk_ac(ad, n, b.sinif, false);
                        self.ts_parametreler(f, alan(v, "parameters"), false, b);
                        alt.fonk = Some(f);
                    } else if v.kind() == "new_expression" {
                        // repo = new Repo(): alanın tipi kurucudan
                        if let Some(c) = alan(v, "constructor") {
                            self.sinif_alani(b, ad, self.m(c));
                        }
                    }
                }
            }
            "function_declaration" | "generator_function_declaration" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let f = self.fonk_ac(ad, n, None, false);
                self.ts_parametreler(f, alan(n, "parameters"), false, b);
                alt.fonk = Some(f);
            }
            "variable_declarator" if alan(n, "name").is_some_and(|x| x.kind() == "object_pattern") => {
                // const { a, b: c } = require('./x'): her ad içe aktarmadır
                let modul = alan(n, "value")
                    .filter(|v| v.kind() == "call_expression" && alan(*v, "function").is_some_and(|f| self.m(f) == "require"))
                    .and_then(|v| alan(v, "arguments"))
                    .and_then(|a| a.named_child(0))
                    .map(|a| tirnaksiz(&self.m(a)));
                if let (Some(modul), Some(desen)) = (modul.filter(|_| b.fonk.is_none()), alan(n, "name")) {
                    for c in cocuklar(desen) {
                        match c.kind() {
                            "shorthand_property_identifier_pattern" | "shorthand_property_identifier" => self.oz.iceaktarma.push((self.m(c), modul.clone(), self.m(c))),
                            "pair_pattern" => {
                                if let (Some(k), Some(v)) = (alan(c, "key"), alan(c, "value")) {
                                    self.oz.iceaktarma.push((self.m(v), modul.clone(), self.m(k)));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "variable_declarator" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let mut deger = alan(n, "value");
                while let Some(d) = deger.filter(|d| matches!(d.kind(), "as_expression" | "satisfies_expression" | "parenthesized_expression" | "await_expression")) {
                    deger = d.named_child(0);
                }
                if let Some(t) = alan(n, "type") {
                    self.yerel(b, ad.clone(), self.m(t).trim_start_matches(':').trim().to_string());
                }
                if let Some(v) = deger {
                    match v.kind() {
                        "arrow_function" | "function_expression" | "function" if b.fonk.is_none() => {
                            let f = self.fonk_ac(ad, n, b.sinif, false);
                            self.ts_parametreler(f, alan(v, "parameters"), false, b);
                            alt.fonk = Some(f);
                        }
                        "new_expression" => {
                            if let Some(c) = alan(v, "constructor") {
                                self.yerel(b, ad, self.m(c));
                            }
                        }
                        "call_expression" => {
                            let fad = alan(v, "function").map(|f| self.m(f)).unwrap_or_default();
                            let args = self.argumanlar(alan(v, "arguments"));
                            // const getX = asyncHandler(async (req, res) => {...}): sarmalanan fonksiyon değişkenin adıyla
                            let sarmal =
                                alan(v, "arguments").is_some_and(|a| cocuklar(a).iter().any(|x| matches!(x.kind(), "arrow_function" | "function_expression" | "function")));
                            if sarmal && b.fonk.is_none() {
                                let f = self.fonk_ac(ad.clone(), n, b.sinif, false);
                                alt.fonk = Some(f);
                            }
                            // Model değişkeni: mongoose.model('User', ...) / sequelize.define('user', ...) / model('User', ...)
                            if b.fonk.is_none() && (fad.ends_with(".model") || fad == "model" || fad.ends_with(".define")) {
                                if let Some(t) = args.first().map(|a| tirnaksiz(a)).filter(|t| !t.is_empty() && !t.contains(['(', ' '])) {
                                    let kaynak = if fad.ends_with(".define") { "sequelize" } else { "mongoose" };
                                    self.oz.model_atamalari.push((ad.clone(), t, kaynak));
                                }
                            }
                            // const x = require('./y')
                            if fad == "require" && b.fonk.is_none() {
                                if let Some(m) = args.first() {
                                    self.oz.iceaktarma.push((ad.clone(), tirnaksiz(m), "default".into()));
                                }
                            }
                            self.yerel(b, ad, format!("={fad}"));
                        }
                        "identifier" | "member_expression" => self.yerel(b, ad, format!("={}", self.m(v))),
                        _ => {}
                    }
                }
            }
            "arrow_function" | "function_expression" | "function" | "generator_function" => {
                self.fonksiyona_gir(n, b, alt);
                if let Some(f) = alt.fonk.filter(|f| Some(*f) != b.fonk) {
                    self.ts_parametreler(f, alan(n, "parameters"), false, b);
                }
            }
            "call_expression" => {
                // Süsleyici (`@Auth() auth`, `@Body() dto`) istek anında çağrılmaz, sınıf tanımında çalışır: çağrı kenarı değildir
                if n.parent().is_some_and(|p| p.kind() == "decorator") {
                    return true;
                }
                let Some(fn_) = alan(n, "function") else { return true };
                let args = self.argumanlar(alan(n, "arguments"));
                let mut c = HamCagri { satir: satir(n), argumanlar: args, ..Default::default() };
                match fn_.kind() {
                    "identifier" => c.ad = self.m(fn_),
                    "member_expression" => {
                        let z = self.ts_zincir(fn_);
                        c.ad = z.last().cloned().unwrap_or_default();
                        c.alici = z[..z.len().saturating_sub(1)].to_vec();
                        if matches!(c.ad.as_str(), "call" | "apply" | "bind") && c.alici.last().is_some_and(|x| x == "<ifade>") {
                            c.dinamik = true;
                        }
                    }
                    "subscript_expression" => {
                        let idx = alan(fn_, "index").map(|i| i.kind() == "string").unwrap_or(false);
                        c.dinamik = !idx;
                        c.ad = alan(fn_, "index").map(|i| tirnaksiz(&self.m(i))).unwrap_or_default();
                        c.alici = alan(fn_, "object").map(|o| self.ts_zincir(o)).unwrap_or_default();
                    }
                    "super" => c.ad = "constructor".into(),
                    _ => return true,
                }
                // Parametre olarak gelen fonksiyonun çağrısı: hedef koddan bilinmez
                if c.alici.is_empty() {
                    if let Some(f) = b.fonk {
                        if self.oz.fonksiyonlar[f].parametreler.iter().any(|p| p.0 == c.ad) {
                            c.dinamik = true;
                        }
                    }
                }
                self.cagri_ekle(b, c);
            }
            "new_expression" => {
                if let Some(k) = alan(n, "constructor") {
                    let z = self.ts_zincir(k);
                    let c = HamCagri {
                        ad: z.last().cloned().unwrap_or_default(),
                        alici: z[..z.len().saturating_sub(1)].to_vec(),
                        satir: satir(n),
                        argumanlar: self.argumanlar(alan(n, "arguments")),
                        yeni: true,
                        dinamik: false,
                    };
                    self.cagri_ekle(b, c);
                }
            }
            "string" | "template_string" => {
                self.dizgi(b, n);
                return n.kind() == "template_string";
            }
            "binary_expression" if alan(n, "operator").is_some_and(|o| matches!(o.kind(), "==" | "!=" | "===" | "!==")) => self.karsilastirma(b, n),
            _ => {}
        }
        true
    }

    // ---- Python ----

    fn py_zincir(&self, n: Node) -> Vec<String> {
        let mut parcalar = vec![];
        let mut x = n;
        for _ in 0..12 {
            match x.kind() {
                "identifier" => {
                    let s = self.m(x);
                    parcalar.push(if s == "self" || s == "cls" { "this".into() } else { s });
                    break;
                }
                "attribute" => {
                    if let Some(a) = alan(x, "attribute") {
                        parcalar.push(self.m(a));
                    }
                    match alan(x, "object") {
                        Some(o) => x = o,
                        None => break,
                    }
                }
                "call" if alan(x, "function").is_some_and(|f| self.m(f) == "super") => {
                    parcalar.push("super".into());
                    break;
                }
                "parenthesized_expression" | "await" => match x.named_child(0) {
                    Some(i) => x = i,
                    None => break,
                },
                _ => {
                    parcalar.push("<ifade>".into());
                    break;
                }
            }
        }
        parcalar.reverse();
        parcalar
    }

    fn py_parametreler(&mut self, f: usize, ps: Option<Node>) {
        let Some(ps) = ps else { return };
        for p in cocuklar(ps) {
            let (ad, tip, deger) = match p.kind() {
                "identifier" => (self.m(p), String::new(), None),
                "typed_parameter" => (p.named_child(0).map(|x| self.m(x)).unwrap_or_default(), alan(p, "type").map(|t| self.m(t)).unwrap_or_default(), None),
                "default_parameter" => (alan(p, "name").map(|x| self.m(x)).unwrap_or_default(), String::new(), alan(p, "value")),
                "typed_default_parameter" => (alan(p, "name").map(|x| self.m(x)).unwrap_or_default(), alan(p, "type").map(|t| self.m(t)).unwrap_or_default(), alan(p, "value")),
                _ => continue,
            };
            // svc = Depends(ItemService) / Annotated[X, Depends(Y)]: Depends'e verilen sınıf parametrenin tipidir
            let mut tip = tip;
            let depends = |m: &str| -> Option<String> {
                let i = m.find("Depends(")?;
                let ic = m[i + 8..].split([')', ',']).next()?.trim().to_string();
                (!ic.is_empty() && ic.chars().next().is_some_and(char::is_uppercase)).then_some(ic)
            };
            if let Some(d) = deger.map(|v| self.m(v)).as_deref().and_then(depends).or_else(|| depends(&tip).filter(|_| tip.starts_with("Annotated[") && tip.contains("Depends()")))
            {
                tip = d;
            }
            self.oz.fonksiyonlar[f].parametreler.push((ad, tip));
        }
    }

    fn py<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match n.kind() {
            "comment" => return false,
            "import_from_statement" => {
                let modul = alan(n, "module_name").map(|m| self.m(m)).unwrap_or_default();
                let mut w = n.walk();
                let adlar: Vec<Node> = n.children_by_field_name("name", &mut w).collect();
                for a in adlar {
                    let (asil, yerel) = match a.kind() {
                        "aliased_import" => (alan(a, "name").map(|x| self.m(x)).unwrap_or_default(), alan(a, "alias").map(|x| self.m(x)).unwrap_or_default()),
                        _ => (self.m(a), self.m(a)),
                    };
                    self.oz.iceaktarma.push((yerel.rsplit('.').next().unwrap_or(&yerel).to_string(), modul.clone(), asil));
                }
                return false;
            }
            "import_statement" => {
                for a in cocuklar(n) {
                    let (asil, yerel) = match a.kind() {
                        "aliased_import" => (alan(a, "name").map(|x| self.m(x)).unwrap_or_default(), alan(a, "alias").map(|x| self.m(x)).unwrap_or_default()),
                        _ => (self.m(a), self.m(a)),
                    };
                    self.oz.iceaktarma.push((yerel, asil, "*".into()));
                }
                return false;
            }
            "class_definition" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let mut ustler = vec![];
                let mut tablo = false;
                if let Some(u) = alan(n, "superclasses") {
                    for c in cocuklar(u) {
                        if c.kind() == "keyword_argument" {
                            let k = alan(c, "name").map(|x| self.m(x)).unwrap_or_default();
                            let v = alan(c, "value").map(|x| self.m(x)).unwrap_or_default();
                            tablo |= k == "table" && v == "True";
                        } else {
                            ustler.push(self.m(c));
                        }
                    }
                }
                let s = self.sinif_ac(ad, n, ustler, false);
                self.oz.siniflar[s].tablo_bayragi = tablo;
                if let Some(d) = n.parent().filter(|p| p.kind() == "decorated_definition") {
                    for c in cocuklar(d).into_iter().filter(|c| c.kind() == "decorator") {
                        let t = self.m(c);
                        self.oz.siniflar[s].isaretler.push(t.trim_start_matches('@').split('(').next().unwrap_or("").to_string());
                    }
                }
                alt.sinif = Some(s);
                alt.fonk = None;
            }
            "function_definition" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                if b.fonk.is_none() {
                    let f = self.fonk_ac(ad, n, b.sinif, false);
                    self.py_parametreler(f, alan(n, "parameters"));
                    if let Some(d) = n.parent().filter(|p| p.kind() == "decorated_definition") {
                        for c in cocuklar(d).into_iter().filter(|c| c.kind() == "decorator") {
                            let t = kisa(&self.m(c), 160);
                            self.oz.fonksiyonlar[f].isaretler.push((t, satir(c)));
                        }
                    }
                    alt.fonk = Some(f);
                }
            }
            "lambda" => {
                self.fonksiyona_gir(n, b, alt);
            }
            "expression_statement" if b.fonk.is_none() && b.sinif.is_some() => {
                // Sınıf gövdesi: __tablename__ = "x"; alan: Tip; alan: Tip = ...
                if let Some(a) = n.named_child(0).filter(|a| a.kind() == "assignment") {
                    let sol = alan(a, "left").map(|x| self.m(x)).unwrap_or_default();
                    if sol == "__tablename__" {
                        if let Some(v) = alan(a, "right").filter(|v| v.kind() == "string") {
                            let s = b.sinif.unwrap();
                            self.oz.siniflar[s].tablo = Some((tirnaksiz(&self.m(v)), "__tablename__"));
                        }
                    }
                    if let Some(t) = alan(a, "type") {
                        self.sinif_alani(b, sol, self.m(t));
                    }
                }
            }
            "assignment" if b.fonk.is_some() => {
                let sol = alan(n, "left");
                let sag = alan(n, "right");
                if let Some(t) = alan(n, "type") {
                    if let Some(s) = sol {
                        self.yerel(b, self.m(s), self.m(t));
                    }
                }
                if let (Some(s), Some(v)) = (sol, sag) {
                    let tip = match v.kind() {
                        "call" => {
                            let f = alan(v, "function").map(|f| self.m(f)).unwrap_or_default();
                            let ilk = alan(v, "arguments").and_then(|a| a.named_child(0)).map(|a| self.m(a)).unwrap_or_default();
                            // session.get(Item, id) / get_object_or_404(Item, ...): modelin örneği
                            if f.ends_with(".get") || f.contains("get_object_or_404") || f.ends_with(".get_or_404") || f.ends_with(".scalar") {
                                format!("={}|{ilk}", f)
                            } else {
                                format!("={f}")
                            }
                        }
                        "identifier" | "attribute" => format!("={}", self.m(v)),
                        _ => String::new(),
                    };
                    let sol_m = self.m(s);
                    // self.repo = Repo(...) / self.repo = repo: sınıf alanı
                    if let Some(a) = sol_m.strip_prefix("self.") {
                        let f = b.fonk.unwrap();
                        let ptip = self.oz.fonksiyonlar[f].parametreler.iter().find(|p| format!("={}", p.0) == tip).map(|p| p.1.clone());
                        match ptip {
                            Some(t) if !t.is_empty() => self.sinif_alani(b, a.to_string(), t),
                            _ if tip.len() > 1 => self.sinif_alani(b, a.to_string(), tip.clone()),
                            _ => {}
                        }
                    }
                    self.yerel(b, sol_m, tip);
                }
            }
            "call" => {
                let Some(fn_) = alan(n, "function") else { return true };
                let args = self.argumanlar(alan(n, "arguments"));
                let mut c = HamCagri { satir: satir(n), argumanlar: args, ..Default::default() };
                match fn_.kind() {
                    "identifier" => {
                        c.ad = self.m(fn_);
                        if matches!(c.ad.as_str(), "getattr" | "eval" | "exec") {
                            c.dinamik = c.ad != "getattr";
                        }
                        if let Some(f) = b.fonk {
                            if self.oz.fonksiyonlar[f].parametreler.iter().any(|p| p.0 == c.ad) {
                                c.dinamik = true;
                            }
                        }
                    }
                    "attribute" => {
                        let z = self.py_zincir(fn_);
                        c.ad = z.last().cloned().unwrap_or_default();
                        c.alici = z[..z.len().saturating_sub(1)].to_vec();
                    }
                    "call" if alan(fn_, "function").is_some_and(|g| self.m(g) == "getattr") => c.dinamik = true,
                    _ => return true,
                }
                self.cagri_ekle(b, c);
            }
            "string" => {
                self.dizgi(b, n);
                return false;
            }
            "comparison_operator" => {
                let t = self.m(n);
                if t.contains("==") || t.contains("!=") || t.contains(" is ") {
                    self.karsilastirma(b, n);
                }
            }
            _ => {}
        }
        true
    }

    // ---- Java ----

    fn java_zincir(&self, n: Node) -> Vec<String> {
        let mut parcalar = vec![];
        let mut x = n;
        for _ in 0..12 {
            match x.kind() {
                "identifier" | "type_identifier" => {
                    parcalar.push(self.m(x));
                    break;
                }
                "this" => {
                    parcalar.push("this".into());
                    break;
                }
                "super" => {
                    parcalar.push("super".into());
                    break;
                }
                "field_access" => {
                    if let Some(f) = alan(x, "field") {
                        parcalar.push(self.m(f));
                    }
                    match alan(x, "object") {
                        Some(o) => x = o,
                        None => break,
                    }
                }
                "scoped_identifier" => {
                    parcalar.push(self.m(x).rsplit('.').next().unwrap_or("").to_string());
                    break;
                }
                "parenthesized_expression" => match x.named_child(0) {
                    Some(i) => x = i,
                    None => break,
                },
                _ => {
                    parcalar.push("<ifade>".into());
                    break;
                }
            }
        }
        parcalar.reverse();
        parcalar
    }

    /// Java / C# yöntem ve sınıf işaretleri (anotasyon / öznitelik metinleri)
    fn isaret_metinleri(&self, n: Node) -> Vec<(String, usize)> {
        let mut v = vec![];
        for c in cocuklar(n) {
            match c.kind() {
                "modifiers" => {
                    for a in cocuklar(c) {
                        if matches!(a.kind(), "annotation" | "marker_annotation") {
                            v.push((kisa(&self.m(a), 200), satir(a)));
                        }
                    }
                }
                "attribute_list" => {
                    for a in cocuklar(c) {
                        if a.kind() == "attribute" {
                            v.push((kisa(&self.m(a), 200), satir(a)));
                        }
                    }
                }
                _ => {}
            }
        }
        v
    }

    fn java<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match n.kind() {
            "line_comment" | "block_comment" => return false,
            "package_declaration" => {
                self.oz.paket = cocuklar(n).first().map(|x| self.m(*x)).unwrap_or_default();
                return false;
            }
            "import_declaration" => {
                let t = self.m(n).trim_start_matches("import").trim().trim_start_matches("static").trim().trim_end_matches(';').trim().to_string();
                let yerel = t.rsplit('.').next().unwrap_or("").to_string();
                self.oz.iceaktarma.push((yerel.clone(), t, yerel));
                return false;
            }
            "class_declaration" | "interface_declaration" | "record_declaration" | "enum_declaration" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let mut ustler = vec![];
                if let Some(u) = alan(n, "superclass") {
                    ustler.extend(cocuklar(u).into_iter().map(|t| self.m(t)));
                }
                for c in cocuklar(n) {
                    if matches!(c.kind(), "super_interfaces" | "extends_interfaces") {
                        for tl in cocuklar(c) {
                            ustler.extend(cocuklar(tl).into_iter().map(|t| self.m(t)));
                        }
                    }
                }
                let s = self.sinif_ac(ad, n, ustler, n.kind() == "interface_declaration");
                for (t, _) in self.isaret_metinleri(n) {
                    let ad = t.trim_start_matches('@').split('(').next().unwrap_or("").to_string();
                    if ad == "Table" || ad == "Entity" {
                        if let Some(c) = ad_ozelligi(&t) {
                            if ad == "Table" || self.oz.siniflar[s].tablo.is_none() {
                                self.oz.siniflar[s].tablo = Some((c, if ad == "Table" { "@Table" } else { "@Entity" }));
                            }
                        }
                    }
                    self.oz.siniflar[s].isaretler.push(ad);
                }
                alt.sinif = Some(s);
                alt.fonk = None;
            }
            "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let f = self.fonk_ac(ad, n, b.sinif, false);
                if let Some(ps) = alan(n, "parameters") {
                    for p in cocuklar(ps).into_iter().filter(|p| matches!(p.kind(), "formal_parameter" | "spread_parameter")) {
                        let ad = alan(p, "name").map(|x| self.m(x)).unwrap_or_default();
                        let tip = alan(p, "type").map(|x| self.m(x)).unwrap_or_default();
                        if n.kind() == "constructor_declaration" {
                            // Kurucu enjeksiyonu: parametre genellikle aynı adlı alana atanır; tip alanın tipidir
                            self.sinif_alani(b, ad.clone(), tip.clone());
                        }
                        self.oz.fonksiyonlar[f].parametreler.push((ad, tip));
                    }
                }
                self.oz.fonksiyonlar[f].isaretler = self.isaret_metinleri(n);
                alt.fonk = Some(f);
            }
            "field_declaration" if b.fonk.is_none() => {
                let tip = alan(n, "type").map(|x| self.m(x)).unwrap_or_default();
                let mut w = n.walk();
                let d: Vec<Node> = n.children_by_field_name("declarator", &mut w).collect();
                for x in d {
                    if let Some(a) = alan(x, "name") {
                        self.sinif_alani(b, self.m(a), tip.clone());
                    }
                }
            }
            "local_variable_declaration" => {
                let tip = alan(n, "type").map(|x| self.m(x)).unwrap_or_default();
                let mut w = n.walk();
                let d: Vec<Node> = n.children_by_field_name("declarator", &mut w).collect();
                for x in d {
                    if let Some(a) = alan(x, "name") {
                        self.yerel(b, self.m(a), tip.clone());
                    }
                }
            }
            "lambda_expression" => {
                self.fonksiyona_gir(n, b, alt);
            }
            "method_invocation" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let alici = alan(n, "object").map(|o| self.java_zincir(o)).unwrap_or_default();
                let args = self.argumanlar(alan(n, "arguments"));
                if ad == "equals" && !alici.is_empty() {
                    if let (Some(o), Some(a)) = (alan(n, "object"), args.first()) {
                        self.karsilastirma_metni(b, n, format!("{} == {a}", metin(o, self.k)));
                    }
                }
                let dinamik = ad == "invoke" && alici.last().is_some_and(|x| x == "<ifade>");
                self.cagri_ekle(b, HamCagri { alici, ad, satir: satir(n), argumanlar: args, yeni: false, dinamik });
            }
            "object_creation_expression" => {
                if let Some(t) = alan(n, "type") {
                    let ad = tip_parcala(&self.m(t)).0;
                    let args = self.argumanlar(alan(n, "arguments"));
                    self.cagri_ekle(b, HamCagri { alici: vec![], ad, satir: satir(n), argumanlar: args, yeni: true, dinamik: false });
                }
            }
            "string_literal" | "text_block" => {
                self.dizgi(b, n);
                return false;
            }
            "binary_expression" if alan(n, "operator").is_some_and(|o| matches!(o.kind(), "==" | "!=")) => self.karsilastirma(b, n),
            _ => {}
        }
        true
    }

    // ---- Kotlin ----

    fn kt_zincir(&self, n: Node) -> Vec<String> {
        let mut parcalar = vec![];
        let mut x = n;
        for _ in 0..12 {
            match x.kind() {
                "identifier" => {
                    parcalar.push(self.m(x));
                    break;
                }
                "this_expression" => {
                    parcalar.push("this".into());
                    break;
                }
                "super_expression" => {
                    parcalar.push("super".into());
                    break;
                }
                "navigation_expression" => {
                    let c = cocuklar(x);
                    match (c.first(), c.last().filter(|l| c.len() > 1 && l.kind() == "identifier")) {
                        (Some(o), Some(a)) => {
                            parcalar.push(self.m(*a));
                            x = *o;
                        }
                        _ => {
                            parcalar.push("<ifade>".into());
                            break;
                        }
                    }
                }
                "parenthesized_expression" | "unary_expression" => match x.named_child(0) {
                    Some(i) => x = i,
                    None => break,
                },
                _ => {
                    parcalar.push("<ifade>".into());
                    break;
                }
            }
        }
        parcalar.reverse();
        parcalar
    }

    /// Kotlin bildiriminin anotasyon metinleri (`@PreAuthorize("...")`); kullanım yeri hedefi atılır
    fn kt_isaretler(&self, n: Node) -> Vec<(String, usize)> {
        cocuklar(n)
            .into_iter()
            .filter(|c| c.kind() == "modifiers")
            .flat_map(cocuklar)
            .filter(|a| a.kind() == "annotation")
            .map(|a| {
                let t = self.m(a);
                let t = match t.split_once(':') {
                    Some((h, r)) if !h.contains('(') => format!("@{r}"),
                    _ => t,
                };
                (kisa(&t, 200), satir(a))
            })
            .collect()
    }

    /// Kotlin tip düğümünün metni (boş olabilirlik işareti atılır)
    fn kt_tip(&self, n: Node) -> Option<String> {
        cocuklar(n).into_iter().find(|c| matches!(c.kind(), "user_type" | "nullable_type" | "non_nullable_type")).map(|t| self.m(t).trim_end_matches('?').trim().to_string())
    }

    /// Çağrının parçaları: çağrılan ifade, değer argümanları, sondaki lambda
    fn kt_cagri<'t>(&self, n: Node<'t>) -> (Option<Node<'t>>, Option<Node<'t>>, Option<Node<'t>>) {
        let (mut callee, mut args, mut lambda) = (None, None, None);
        for c in cocuklar(n) {
            match c.kind() {
                "value_arguments" => args = Some(c),
                "annotated_lambda" => lambda = Some(c),
                "type_arguments" => {}
                _ => callee = Some(c),
            }
        }
        (callee, args, lambda)
    }

    fn kt<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match n.kind() {
            "line_comment" | "block_comment" => return false,
            "package_header" => {
                self.oz.paket = cocuklar(n).first().map(|x| self.m(*x)).unwrap_or_default();
                return false;
            }
            "import" => {
                let t = self.m(n).trim_start_matches("import").trim().to_string();
                let (yol, takma) = match t.split_once(" as ") {
                    Some((y, a)) => (y.trim().to_string(), a.trim().to_string()),
                    None => (t.clone(), t.rsplit('.').next().unwrap_or("").to_string()),
                };
                let asil = yol.rsplit('.').next().unwrap_or("").to_string();
                self.oz.iceaktarma.push((takma, yol, asil));
                return false;
            }
            "class_declaration" | "object_declaration" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let mut ustler = vec![];
                let mut exposed: Option<Option<String>> = None;
                for d in cocuklar(n).into_iter().filter(|c| c.kind() == "delegation_specifiers").flat_map(cocuklar) {
                    for c in cocuklar(d) {
                        match c.kind() {
                            "constructor_invocation" => {
                                let t = cocuklar(c).into_iter().find(|x| x.kind() == "user_type").map(|x| self.m(x)).unwrap_or_default();
                                if EXPOSED.contains(&tip_parcala(&t).0.as_str()) {
                                    let a = cocuklar(c).into_iter().find(|x| x.kind() == "value_arguments").and_then(|v| self.argumanlar(Some(v)).into_iter().next());
                                    exposed = Some(a.map(|a| tirnaksiz(a.trim_start_matches("name").trim().trim_start_matches('=').trim())).filter(|a| !a.is_empty()));
                                }
                                ustler.push(t);
                            }
                            "user_type" => ustler.push(self.m(c)),
                            _ => {}
                        }
                    }
                }
                let arayuz = {
                    let mut w = n.walk();
                    let v = n.children(&mut w).any(|c| c.kind() == "interface");
                    v
                };
                let s = self.sinif_ac(ad, n, ustler, arayuz);
                for (t, _) in self.kt_isaretler(n) {
                    let isaret = t.trim_start_matches('@').split('(').next().unwrap_or("").to_string();
                    if isaret == "Table" || isaret == "Entity" {
                        if let Some(c) = ad_ozelligi(&t) {
                            if isaret == "Table" || self.oz.siniflar[s].tablo.is_none() {
                                self.oz.siniflar[s].tablo = Some((c, if isaret == "Table" { "@Table" } else { "@Entity" }));
                            }
                        }
                    }
                    self.oz.siniflar[s].isaretler.push(isaret);
                }
                if let Some(Some(t)) = exposed {
                    self.oz.siniflar[s].tablo = Some((t, "Exposed Table(name)"));
                }
                // Kurucu özellikleri (`class S(private val repo: Repo)`): kurucu enjeksiyonu
                let sb = Baglam { sinif: Some(s), ..b };
                for p in cocuklar(n).into_iter().filter(|c| c.kind() == "primary_constructor").flat_map(cocuklar).flat_map(cocuklar).filter(|c| c.kind() == "class_parameter") {
                    let mut w = p.walk();
                    if !p.children(&mut w).any(|x| matches!(x.kind(), "val" | "var")) {
                        continue;
                    }
                    let ad = cocuklar(p).into_iter().find(|x| x.kind() == "identifier").map(|x| self.m(x)).unwrap_or_default();
                    if let Some(t) = self.kt_tip(p) {
                        self.sinif_alani(sb, ad, t);
                    }
                }
                alt.sinif = Some(s);
                alt.fonk = None;
            }
            "function_declaration" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let f = self.fonk_ac(ad, n, b.sinif, false);
                for ps in cocuklar(n).into_iter().filter(|c| c.kind() == "function_value_parameters") {
                    for p in cocuklar(ps).into_iter().filter(|p| p.kind() == "parameter") {
                        let ad = cocuklar(p).into_iter().find(|x| x.kind() == "identifier").map(|x| self.m(x)).unwrap_or_default();
                        let tip = self.kt_tip(p).unwrap_or_default();
                        self.oz.fonksiyonlar[f].parametreler.push((ad, tip));
                    }
                }
                self.oz.fonksiyonlar[f].isaretler = self.kt_isaretler(n);
                alt.fonk = Some(f);
            }
            "property_declaration" => {
                let vd = cocuklar(n).into_iter().find(|c| c.kind() == "variable_declaration");
                let ad = vd.and_then(|v| cocuklar(v).into_iter().find(|x| x.kind() == "identifier")).map(|x| self.m(x)).unwrap_or_default();
                let mut tip = vd.and_then(|v| self.kt_tip(v));
                if tip.is_none() {
                    // `val s = Servis()`, `val s by inject<Servis>()`, `val g = servis.getir(id)`
                    let mut w = n.walk();
                    let cocuk: Vec<Node> = n.children(&mut w).collect();
                    let ilk = cocuk
                        .iter()
                        .position(|x| x.kind() == "=")
                        .and_then(|i| cocuk.get(i + 1))
                        .copied()
                        .or_else(|| cocuk.iter().find(|x| x.kind() == "property_delegate").and_then(|d| d.named_child(0)));
                    if let Some(i) = ilk.filter(|i| i.kind() == "call_expression") {
                        let (c, _, _) = self.kt_cagri(i);
                        let m = c.map(|c| self.m(c)).unwrap_or_default();
                        let ta = cocuklar(i).into_iter().find(|x| x.kind() == "type_arguments").map(|t| self.m(t).trim_matches(['<', '>']).trim().to_string());
                        tip = match (m.as_str(), ta) {
                            ("inject" | "get", Some(t)) => Some(t),
                            (m, _) if m.starts_with(char::is_uppercase) && !m.contains('.') => Some(m.to_string()),
                            (m, _) if !m.is_empty() => Some(format!("={m}")),
                            _ => None,
                        };
                    }
                }
                if let Some(t) = tip {
                    if b.fonk.is_some() {
                        self.yerel(b, ad, t);
                    } else if !t.starts_with('=') {
                        self.sinif_alani(b, ad, t);
                    }
                }
            }
            "lambda_literal" => {
                // Ktor işleyicisi (`get("/x") { }`): kendi fonksiyonu, adı fiil; başka lambda çevreleyen fonksiyonun parçası
                let cagri = n.parent().filter(|p| p.kind() == "annotated_lambda").and_then(|p| p.parent()).filter(|p| p.kind() == "call_expression");
                let fiil = cagri.and_then(|c| {
                    let callee = self.kt_cagri(c).0?;
                    let callee = if callee.kind() == "call_expression" { self.kt_cagri(callee).0? } else { callee };
                    (callee.kind() == "identifier")
                        .then(|| self.m(callee))
                        .filter(|a| matches!(a.as_str(), "get" | "post" | "put" | "patch" | "delete" | "head" | "options" | "handle"))
                });
                match fiil {
                    Some(a) => {
                        let f = self.fonk_ac(a, n, b.sinif, false);
                        // Çevreleyen fonksiyonun parametreleri ve yerelleri işleyicide görünür
                        if let Some(u) = b.fonk {
                            let ek: Vec<(String, String)> = self.oz.fonksiyonlar[u].parametreler.iter().chain(&self.oz.fonksiyonlar[u].yereller).cloned().collect();
                            self.oz.fonksiyonlar[f].yereller.extend(ek);
                        }
                        alt.fonk = Some(f);
                    }
                    None => {
                        self.fonksiyona_gir(n, b, alt);
                    }
                }
            }
            "call_expression" => {
                let (callee, args, lambda) = self.kt_cagri(n);
                let Some(callee) = callee else { return true };
                // `get("/x") { }`: içteki çağrı ayrıca gezilir
                if callee.kind() == "call_expression" && args.is_none() {
                    return true;
                }
                let (alici, ad) = match callee.kind() {
                    "identifier" => (vec![], self.m(callee)),
                    "navigation_expression" => {
                        let z = self.kt_zincir(callee);
                        match z.split_last() {
                            Some((a, r)) => (r.to_vec(), a.clone()),
                            None => return true,
                        }
                    }
                    _ => return true,
                };
                let mut argumanlar = self.argumanlar(args);
                if let Some(l) = lambda {
                    argumanlar.push(kisa(&self.m(l), ARGUMAN_UZUNLUGU));
                }
                let yeni = alici.is_empty() && ad.starts_with(char::is_uppercase);
                let dinamik = matches!(ad.as_str(), "invoke" | "call") && alici.last().is_some_and(|x| x == "<ifade>");
                self.cagri_ekle(b, HamCagri { alici, ad, satir: satir(n), argumanlar, yeni, dinamik });
            }
            "string_literal" | "multiline_string_literal" => {
                self.dizgi(b, n);
                return false;
            }
            "binary_expression" if alan(n, "operator").is_some_and(|o| matches!(o.kind(), "==" | "!=")) => self.karsilastirma(b, n),
            _ => {}
        }
        true
    }

    // ---- C# ----

    fn cs_zincir(&self, n: Node) -> Vec<String> {
        let mut parcalar = vec![];
        let mut x = n;
        for _ in 0..12 {
            match x.kind() {
                "identifier" => {
                    parcalar.push(self.m(x));
                    break;
                }
                "generic_name" => {
                    parcalar.push(x.named_child(0).map(|i| self.m(i)).unwrap_or_default());
                    break;
                }
                "this" | "this_expression" => {
                    parcalar.push("this".into());
                    break;
                }
                "base" | "base_expression" => {
                    parcalar.push("super".into());
                    break;
                }
                "member_access_expression" => {
                    if let Some(f) = alan(x, "name") {
                        let ad = if f.kind() == "generic_name" { f.named_child(0).map(|i| self.m(i)).unwrap_or_default() } else { self.m(f) };
                        parcalar.push(ad);
                    }
                    match alan(x, "expression") {
                        Some(o) => x = o,
                        None => break,
                    }
                }
                "parenthesized_expression" | "await_expression" | "postfix_unary_expression" => match x.named_child(0) {
                    Some(i) => x = i,
                    None => break,
                },
                _ => {
                    parcalar.push("<ifade>".into());
                    break;
                }
            }
        }
        parcalar.reverse();
        parcalar
    }

    fn cs_parametreler(&mut self, f: Option<usize>, ps: Option<Node>, b: Baglam, alan_mi: bool) {
        let Some(ps) = ps else { return };
        for p in cocuklar(ps).into_iter().filter(|p| p.kind() == "parameter") {
            let ad = alan(p, "name").map(|x| self.m(x)).unwrap_or_default();
            let tip = alan(p, "type").map(|x| self.m(x)).unwrap_or_default();
            if alan_mi {
                self.sinif_alani(b, ad.clone(), tip.clone());
                self.sinif_alani(b, format!("_{ad}"), tip.clone());
            }
            if let Some(f) = f {
                self.oz.fonksiyonlar[f].parametreler.push((ad, tip));
            }
        }
    }

    fn cs<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match n.kind() {
            "comment" => return false,
            "using_directive" => {
                let t = self.m(n).trim_start_matches("global").trim().trim_start_matches("using").trim().trim_end_matches(';').trim().to_string();
                self.oz.iceaktarma.push((t.rsplit('.').next().unwrap_or("").to_string(), t, String::new()));
                return false;
            }
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                self.oz.paket = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
            }
            "class_declaration" | "interface_declaration" | "record_declaration" | "struct_declaration" | "record_struct_declaration" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let mut ustler = vec![];
                for c in cocuklar(n) {
                    if c.kind() == "base_list" {
                        for t in cocuklar(c) {
                            let t = if t.kind() == "primary_constructor_base_type" { t.named_child(0).unwrap_or(t) } else { t };
                            ustler.push(self.m(t));
                        }
                    }
                }
                let s = self.sinif_ac(ad, n, ustler, n.kind() == "interface_declaration");
                for (t, _) in self.isaret_metinleri(n) {
                    let ad = t.split('(').next().unwrap_or("").trim().to_string();
                    if ad == "Table" {
                        if let Some(i) = t.find('"') {
                            let r = &t[i + 1..];
                            if let Some(j) = r.find('"') {
                                self.oz.siniflar[s].tablo = Some((r[..j].to_string(), "[Table]"));
                            }
                        }
                    }
                    self.oz.siniflar[s].isaretler.push(ad);
                }
                alt.sinif = Some(s);
                alt.fonk = None;
                // Birincil kurucu (C# 12): parametreler sınıf alanıdır
                let ps = cocuklar(n).into_iter().find(|c| c.kind() == "parameter_list");
                self.cs_parametreler(None, ps, *alt, true);
            }
            "method_declaration" | "constructor_declaration" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let f = self.fonk_ac(ad, n, b.sinif, false);
                self.cs_parametreler(Some(f), alan(n, "parameters"), b, n.kind() == "constructor_declaration");
                self.oz.fonksiyonlar[f].isaretler = self.isaret_metinleri(n);
                alt.fonk = Some(f);
            }
            "field_declaration" | "property_declaration" if b.fonk.is_none() => {
                if n.kind() == "property_declaration" {
                    let tip = alan(n, "type").map(|x| self.m(x)).unwrap_or_default();
                    let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                    self.sinif_alani(b, ad, tip);
                } else if let Some(v) = cocuklar(n).into_iter().find(|c| c.kind() == "variable_declaration") {
                    let tip = alan(v, "type").map(|x| self.m(x)).unwrap_or_default();
                    for d in cocuklar(v).into_iter().filter(|d| d.kind() == "variable_declarator") {
                        let ad = alan(d, "name").or_else(|| d.named_child(0)).map(|x| self.m(x)).unwrap_or_default();
                        self.sinif_alani(b, ad, tip.clone());
                    }
                }
            }
            "local_declaration_statement" => {
                if let Some(v) = cocuklar(n).into_iter().find(|c| c.kind() == "variable_declaration") {
                    let tip = alan(v, "type").map(|x| self.m(x)).unwrap_or_default();
                    for d in cocuklar(v).into_iter().filter(|d| d.kind() == "variable_declarator") {
                        let ad = alan(d, "name").or_else(|| d.named_child(0)).map(|x| self.m(x)).unwrap_or_default();
                        if tip != "var" {
                            self.yerel(b, ad, tip.clone());
                        } else if let Some(o) = cocuklar(d).into_iter().find(|c| c.kind() == "object_creation_expression") {
                            let t = alan(o, "type").map(|x| self.m(x)).unwrap_or_default();
                            self.yerel(b, ad, t);
                        }
                    }
                }
            }
            "lambda_expression" | "anonymous_method_expression" | "local_function_statement" => {
                self.fonksiyona_gir(n, b, alt);
            }
            "invocation_expression" => {
                let Some(fn_) = alan(n, "function") else { return true };
                // services.AddScoped<IArayuz, Sinif>(): bağımlılık kaydı
                if let Some(g) = alan(fn_, "name").filter(|g| g.kind() == "generic_name") {
                    let ad = g.named_child(0).map(|i| self.m(i)).unwrap_or_default();
                    if ad.starts_with("Add") || ad.starts_with("TryAdd") {
                        if let Some(l) = cocuklar(g).into_iter().find(|c| c.kind() == "type_argument_list") {
                            let t: Vec<String> = cocuklar(l).into_iter().map(|x| tip_parcala(&self.m(x)).0).collect();
                            if let [a, s] = t.as_slice() {
                                self.oz.di_kayitlari.push((a.clone(), s.clone()));
                            }
                        }
                    }
                }
                let z = self.cs_zincir(fn_);
                let args = self.argumanlar(alan(n, "arguments"));
                let ad = z.last().cloned().unwrap_or_default();
                let alici = z[..z.len().saturating_sub(1)].to_vec();
                if ad == "Equals" && !alici.is_empty() {
                    if let Some(a) = args.first() {
                        self.karsilastirma_metni(b, n, format!("{} == {a}", alici.join(".")));
                    }
                }
                let dinamik = ad == "Invoke" && alici.last().is_some_and(|x| x == "<ifade>") || ad == "DynamicInvoke";
                self.cagri_ekle(b, HamCagri { alici, ad, satir: satir(n), argumanlar: args, yeni: false, dinamik });
            }
            "object_creation_expression" => {
                if let Some(t) = alan(n, "type") {
                    let ad = tip_parcala(&self.m(t)).0;
                    let args = self.argumanlar(alan(n, "arguments"));
                    self.cagri_ekle(b, HamCagri { alici: vec![], ad, satir: satir(n), argumanlar: args, yeni: true, dinamik: false });
                }
            }
            "string_literal" | "verbatim_string_literal" | "raw_string_literal" | "interpolated_string_expression" => {
                self.dizgi(b, n);
                return false;
            }
            "binary_expression" if alan(n, "operator").is_some_and(|o| matches!(o.kind(), "==" | "!=")) => self.karsilastirma(b, n),
            _ => {}
        }
        true
    }

    // ---- Go ----

    fn go_zincir(&self, n: Node, alici: Option<Node>) -> Vec<String> {
        let alici_adi = alici.map(|a| self.m(a));
        let mut parcalar = vec![];
        let mut x = n;
        for _ in 0..12 {
            match x.kind() {
                "identifier" | "package_identifier" | "field_identifier" | "type_identifier" => {
                    let s = self.m(x);
                    parcalar.push(if Some(&s) == alici_adi.as_ref() { "this".into() } else { s });
                    break;
                }
                "selector_expression" => {
                    if let Some(f) = alan(x, "field") {
                        parcalar.push(self.m(f));
                    }
                    match alan(x, "operand") {
                        Some(o) => x = o,
                        None => break,
                    }
                }
                "parenthesized_expression" => match x.named_child(0) {
                    Some(i) => x = i,
                    None => break,
                },
                _ => {
                    parcalar.push("<ifade>".into());
                    break;
                }
            }
        }
        parcalar.reverse();
        parcalar
    }

    fn go_parametreler(&mut self, f: usize, ps: Option<Node>) {
        let Some(ps) = ps else { return };
        for p in cocuklar(ps).into_iter().filter(|p| matches!(p.kind(), "parameter_declaration" | "variadic_parameter_declaration")) {
            let tip = alan(p, "type").map(|x| self.m(x)).unwrap_or_default();
            let mut w = p.walk();
            let adlar: Vec<String> = p.children_by_field_name("name", &mut w).map(|x| self.m(x)).collect();
            for ad in adlar {
                self.oz.fonksiyonlar[f].parametreler.push((ad, tip.clone()));
            }
        }
    }

    fn go<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match n.kind() {
            "comment" => return false,
            "package_clause" => {
                self.oz.paket = n.named_child(0).map(|x| self.m(x)).unwrap_or_default();
                return false;
            }
            "import_spec" => {
                let yol = alan(n, "path").map(|p| tirnaksiz(&self.m(p))).unwrap_or_default();
                let yerel = alan(n, "name").map(|x| self.m(x)).unwrap_or_else(|| yol.rsplit('/').next().unwrap_or("").to_string());
                self.oz.iceaktarma.push((yerel, yol, "*".into()));
                return false;
            }
            "type_spec" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let Some(t) = alan(n, "type") else { return false };
                let arayuz = t.kind() == "interface_type";
                if t.kind() != "struct_type" && !arayuz {
                    return false;
                }
                let s = self.sinif_ac(ad, n, vec![], arayuz);
                if let Some(l) = cocuklar(t).into_iter().find(|c| c.kind() == "field_declaration_list") {
                    for fd in cocuklar(l).into_iter().filter(|c| c.kind() == "field_declaration") {
                        let tip = alan(fd, "type").map(|x| self.m(x)).unwrap_or_default();
                        let mut w = fd.walk();
                        let adlar: Vec<String> = fd.children_by_field_name("name", &mut w).map(|x| self.m(x)).collect();
                        if adlar.is_empty() {
                            // Gömülü alan: üst tür gibi davranır
                            self.oz.siniflar[s].ustler.push(tip.clone());
                            let ad = tip_parcala(&tip).0;
                            self.oz.siniflar[s].alanlar.push((ad, tip.clone()));
                        }
                        for a in adlar {
                            self.oz.siniflar[s].alanlar.push((a, tip.clone()));
                        }
                    }
                }
                return false;
            }
            "method_declaration" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let rp = alan(n, "receiver").and_then(|r| cocuklar(r).into_iter().find(|c| c.kind() == "parameter_declaration"));
                let tip = rp.and_then(|p| alan(p, "type")).map(|x| tip_parcala(&self.m(x)).0).unwrap_or_default();
                // Alıcı tipinin sınıfı: aynı dosyada ya da paketin başka dosyasında; çözümde ad ve paketle eşlenir
                let s = match self.oz.siniflar.iter().position(|s| s.ad == tip) {
                    Some(i) => i,
                    None => {
                        let i = self.sinif_ac(tip.clone(), n, vec![], false);
                        self.oz.siniflar[i].isaretler.push("<alici>".into());
                        i
                    }
                };
                let f = self.fonk_ac(ad.clone(), n, Some(s), false);
                self.go_parametreler(f, alan(n, "parameters"));
                alt.fonk = Some(f);
                alt.alici = rp.and_then(|p| alan(p, "name"));
                // func (X) TableName() string { return "x" }
                if ad == "TableName" {
                    if let Some(g) = alan(n, "body") {
                        let t = self.m(g);
                        if let Some(i) = t.find('"') {
                            let r = &t[i + 1..];
                            if let Some(j) = r.find('"') {
                                self.oz.siniflar[s].go_tablo = Some(r[..j].to_string());
                            }
                        }
                    }
                }
            }
            "function_declaration" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let f = self.fonk_ac(ad, n, None, false);
                self.go_parametreler(f, alan(n, "parameters"));
                alt.fonk = Some(f);
            }
            "func_literal" => {
                self.fonksiyona_gir(n, b, alt);
                if let Some(f) = alt.fonk.filter(|f| Some(*f) != b.fonk) {
                    self.go_parametreler(f, alan(n, "parameters"));
                }
            }
            "var_spec" => {
                let tip = alan(n, "type").map(|x| self.m(x)).unwrap_or_default();
                let mut w = n.walk();
                let adlar: Vec<String> = n.children_by_field_name("name", &mut w).map(|x| self.m(x)).collect();
                for a in adlar {
                    self.yerel(b, a, tip.clone());
                }
            }
            "short_var_declaration" => {
                let sol: Vec<String> = alan(n, "left").map(|l| cocuklar(l).into_iter().map(|x| self.m(x)).collect()).unwrap_or_default();
                let sag: Vec<Node> = alan(n, "right").map(cocuklar).unwrap_or_default();
                for (a, v) in sol.into_iter().zip(sag) {
                    let v = if v.kind() == "unary_expression" { alan(v, "operand").unwrap_or(v) } else { v };
                    if v.kind() == "composite_literal" {
                        if let Some(t) = alan(v, "type") {
                            self.yerel(b, a, self.m(t));
                        }
                    } else if v.kind() == "call_expression" {
                        let f = alan(v, "function").map(|f| self.m(f)).unwrap_or_default();
                        self.yerel(b, a, format!("={f}"));
                    }
                }
            }
            "call_expression" => {
                let Some(fn_) = alan(n, "function") else { return true };
                let args = self.argumanlar(alan(n, "arguments"));
                let mut c = HamCagri { satir: satir(n), argumanlar: args, ..Default::default() };
                match fn_.kind() {
                    "identifier" => {
                        c.ad = self.m(fn_);
                        if let Some(f) = b.fonk {
                            if self.oz.fonksiyonlar[f].parametreler.iter().any(|p| p.0 == c.ad) {
                                c.dinamik = true;
                            }
                        }
                    }
                    "selector_expression" => {
                        let z = self.go_zincir(fn_, b.alici);
                        c.ad = z.last().cloned().unwrap_or_default();
                        c.alici = z[..z.len().saturating_sub(1)].to_vec();
                        c.dinamik = c.ad == "Call" && c.alici.last().is_some_and(|x| x == "<ifade>") || c.ad == "MethodByName";
                    }
                    _ => return true,
                }
                self.cagri_ekle(b, c);
            }
            "interpreted_string_literal" | "raw_string_literal" => {
                self.dizgi(b, n);
                return false;
            }
            "binary_expression" if alan(n, "operator").is_some_and(|o| matches!(o.kind(), "==" | "!=")) => self.karsilastirma(b, n),
            _ => {}
        }
        true
    }

    // ---- PHP ----

    fn php_zincir(&self, n: Node) -> Vec<String> {
        let mut parcalar = vec![];
        let mut x = n;
        for _ in 0..12 {
            match x.kind() {
                "variable_name" => {
                    let s = self.m(x);
                    parcalar.push(if s == "$this" { "this".into() } else { s });
                    break;
                }
                "name" | "qualified_name" => {
                    let s = self.m(x);
                    parcalar.push(s.rsplit('\\').next().unwrap_or(&s).to_string());
                    break;
                }
                "member_access_expression" | "nullsafe_member_access_expression" => {
                    if let Some(f) = alan(x, "name") {
                        parcalar.push(self.m(f));
                    }
                    match alan(x, "object") {
                        Some(o) => x = o,
                        None => break,
                    }
                }
                "function_call_expression" => {
                    // app(X::class) / resolve(X::class): kapsayıcıdan çözülen sınıf
                    let f = alan(x, "function").map(|f| self.m(f)).unwrap_or_default();
                    let ilk = alan(x, "arguments").and_then(|a| a.named_child(0)).map(|a| self.m(a)).unwrap_or_default();
                    match (f.as_str(), ilk.strip_suffix("::class")) {
                        ("app" | "resolve" | "\\app", Some(sinif)) => parcalar.push(format!("<kap:{}>", sinif.rsplit('\\').next().unwrap_or(sinif))),
                        _ => parcalar.push("<ifade>".into()),
                    }
                    break;
                }
                "parenthesized_expression" => match x.named_child(0) {
                    Some(i) => x = i,
                    None => break,
                },
                _ => {
                    parcalar.push("<ifade>".into());
                    break;
                }
            }
        }
        parcalar.reverse();
        parcalar
    }

    fn php<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match n.kind() {
            "comment" => return false,
            "namespace_definition" => {
                self.oz.paket = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
            }
            "namespace_use_declaration" => {
                for c in cocuklar(n) {
                    if c.kind() == "namespace_use_clause" {
                        let t = self.m(c);
                        let (tam, takma) = match t.split_once(" as ") {
                            Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
                            None => (t.trim().to_string(), t.trim().rsplit('\\').next().unwrap_or("").to_string()),
                        };
                        let asil = tam.rsplit('\\').next().unwrap_or("").to_string();
                        self.oz.iceaktarma.push((takma, tam, asil));
                    }
                }
                return false;
            }
            "class_declaration" | "interface_declaration" | "trait_declaration" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let mut ustler = vec![];
                for c in cocuklar(n) {
                    if matches!(c.kind(), "base_clause" | "class_interface_clause") {
                        ustler.extend(cocuklar(c).into_iter().map(|t| self.m(t)));
                    }
                }
                let s = self.sinif_ac(ad, n, ustler, n.kind() == "interface_declaration");
                alt.sinif = Some(s);
                alt.fonk = None;
            }
            "property_declaration" if b.fonk.is_none() => {
                let tip = alan(n, "type").map(|x| self.m(x)).unwrap_or_default();
                for e in cocuklar(n).into_iter().filter(|c| c.kind() == "property_element") {
                    let ad = e.named_child(0).map(|x| self.m(x)).unwrap_or_default();
                    // protected $table = 'x';
                    if ad == "$table" {
                        if let Some(v) = cocuklar(e).into_iter().find_map(|c| {
                            if c.kind() == "property_initializer" {
                                c.named_child(0)
                            } else if matches!(c.kind(), "string" | "encapsed_string") {
                                Some(c)
                            } else {
                                None
                            }
                        }) {
                            if let Some(s) = b.sinif {
                                self.oz.siniflar[s].tablo = Some((tirnaksiz(&self.m(v)), "$table"));
                            }
                        }
                    }
                    if !tip.is_empty() {
                        self.sinif_alani(b, ad, tip.clone());
                    }
                }
            }
            "method_declaration" | "function_definition" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let sinif = if n.kind() == "method_declaration" { b.sinif } else { None };
                let f = self.fonk_ac(ad.clone(), n, sinif, false);
                if let Some(ps) = alan(n, "parameters") {
                    for p in cocuklar(ps) {
                        if !matches!(p.kind(), "simple_parameter" | "property_promotion_parameter" | "variadic_parameter") {
                            continue;
                        }
                        let pad = alan(p, "name").map(|x| self.m(x)).unwrap_or_default();
                        let tip = alan(p, "type").map(|x| self.m(x)).unwrap_or_default();
                        if p.kind() == "property_promotion_parameter" {
                            self.sinif_alani(b, pad.clone(), tip.clone());
                        }
                        self.oz.fonksiyonlar[f].parametreler.push((pad, tip));
                    }
                }
                alt.fonk = Some(f);
            }
            "anonymous_function" | "anonymous_function_creation_expression" | "arrow_function" => {
                self.fonksiyona_gir(n, b, alt);
            }
            "assignment_expression" => {
                let sol = alan(n, "left").map(|x| self.m(x)).unwrap_or_default();
                if let Some(v) = alan(n, "right") {
                    let tip = match v.kind() {
                        "scoped_call_expression" => {
                            format!("={}::{}", alan(v, "scope").map(|s| self.m(s)).unwrap_or_default(), alan(v, "name").map(|s| self.m(s)).unwrap_or_default())
                        }
                        "object_creation_expression" => cocuklar(v).into_iter().find(|c| matches!(c.kind(), "name" | "qualified_name")).map(|c| self.m(c)).unwrap_or_default(),
                        _ => String::new(),
                    };
                    // $this->repo = $repo (kurucu): parametrenin tipi alanın tipi
                    if let (Some(a), Some(f)) = (sol.strip_prefix("$this->"), b.fonk) {
                        let sag = self.m(v);
                        if let Some(t) = self.oz.fonksiyonlar[f].parametreler.iter().find(|p| p.0 == sag).map(|p| p.1.clone()).filter(|t| !t.is_empty()) {
                            self.sinif_alani(b, a.to_string(), t);
                        } else if !tip.is_empty() && !tip.starts_with('=') {
                            self.sinif_alani(b, a.to_string(), tip.clone());
                        }
                    }
                    self.yerel(b, sol, tip);
                }
            }
            "member_call_expression" | "nullsafe_member_call_expression" | "scoped_call_expression" | "function_call_expression" => {
                let args = self.argumanlar(alan(n, "arguments"));
                let mut c = HamCagri { satir: satir(n), argumanlar: args, ..Default::default() };
                match n.kind() {
                    "function_call_expression" => {
                        let f = alan(n, "function");
                        c.ad = f.map(|f| self.m(f)).unwrap_or_default().trim_start_matches('\\').to_string();
                        c.dinamik = f.is_some_and(|f| f.kind() == "variable_name") || matches!(c.ad.as_str(), "call_user_func" | "call_user_func_array");
                    }
                    "scoped_call_expression" => {
                        let s = alan(n, "scope").map(|s| self.m(s)).unwrap_or_default();
                        c.alici = vec![match s.as_str() {
                            "self" | "static" => "this".into(),
                            "parent" => "super".into(),
                            _ => s.rsplit('\\').next().unwrap_or(&s).to_string(),
                        }];
                        let nm = alan(n, "name");
                        c.ad = nm.map(|x| self.m(x)).unwrap_or_default();
                        c.dinamik = nm.is_some_and(|x| x.kind() == "variable_name");
                    }
                    _ => {
                        c.alici = alan(n, "object").map(|o| self.php_zincir(o)).unwrap_or_default();
                        let nm = alan(n, "name");
                        c.ad = nm.map(|x| self.m(x)).unwrap_or_default();
                        c.dinamik = nm.is_some_and(|x| x.kind() == "variable_name");
                        // $this->app->bind(Arayuz::class, Sinif::class): bağımlılık kaydı
                        if matches!(c.ad.as_str(), "bind" | "singleton" | "scoped" | "bindIf" | "singletonIf") {
                            if let [a, s, ..] = c.argumanlar.as_slice() {
                                if let (Some(a), Some(s)) = (a.strip_suffix("::class"), s.strip_suffix("::class")) {
                                    self.oz.di_kayitlari.push((tip_parcala(a).0, tip_parcala(s).0));
                                }
                            }
                        }
                    }
                }
                self.cagri_ekle(b, c);
            }
            "object_creation_expression" => {
                if let Some(t) = cocuklar(n).into_iter().find(|c| matches!(c.kind(), "name" | "qualified_name")) {
                    let s = self.m(t);
                    let args = self.argumanlar(cocuklar(n).into_iter().find(|c| c.kind() == "arguments"));
                    self.cagri_ekle(
                        b,
                        HamCagri { alici: vec![], ad: s.rsplit('\\').next().unwrap_or(&s).to_string(), satir: satir(n), argumanlar: args, yeni: true, dinamik: false },
                    );
                }
            }
            "string" | "encapsed_string" | "heredoc" | "nowdoc" => {
                self.dizgi(b, n);
                return false;
            }
            "binary_expression" if alan(n, "operator").is_some_and(|o| matches!(o.kind(), "==" | "!=" | "===" | "!==")) => self.karsilastirma(b, n),
            _ => {}
        }
        true
    }

    // ---- Ruby ----

    fn rb_zincir(&self, n: Node) -> Vec<String> {
        let mut parcalar = vec![];
        let mut x = n;
        for _ in 0..12 {
            match x.kind() {
                "identifier" | "constant" | "instance_variable" => {
                    parcalar.push(self.m(x));
                    break;
                }
                "self" => {
                    parcalar.push("this".into());
                    break;
                }
                "scope_resolution" => {
                    parcalar.push(alan(x, "name").map(|a| self.m(a)).unwrap_or_default());
                    break;
                }
                "call" => {
                    let m = alan(x, "method").map(|a| self.m(a)).unwrap_or_default();
                    let r = alan(x, "receiver");
                    // X.new: X'in örneği
                    if m == "new" && r.is_some_and(|r| matches!(r.kind(), "constant" | "scope_resolution")) {
                        let t = r.map(|r| self.m(r)).unwrap_or_default();
                        parcalar.push(format!("<yeni:{}>", t.rsplit("::").next().unwrap_or(&t)));
                        break;
                    }
                    parcalar.push(m);
                    match r {
                        Some(o) => x = o,
                        None => break,
                    }
                }
                "parenthesized_statements" => match x.named_child(0) {
                    Some(i) => x = i,
                    None => break,
                },
                _ => {
                    parcalar.push("<ifade>".into());
                    break;
                }
            }
        }
        parcalar.reverse();
        parcalar
    }

    fn rb<'t>(&mut self, n: Node<'t>, b: Baglam<'t>, alt: &mut Baglam<'t>) -> bool {
        match n.kind() {
            "comment" => return false,
            "class" | "module" => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let ad = ad.rsplit("::").next().unwrap_or(&ad).to_string();
                let ustler = alan(n, "superclass").and_then(|u| u.named_child(0)).map(|c| vec![self.m(c)]).unwrap_or_default();
                let s = self.sinif_ac(ad, n, ustler, false);
                alt.sinif = Some(s);
                alt.fonk = None;
            }
            "method" | "singleton_method" if b.fonk.is_none() => {
                let ad = alan(n, "name").map(|x| self.m(x)).unwrap_or_default();
                let f = self.fonk_ac(ad, n, b.sinif, false);
                if let Some(ps) = alan(n, "parameters") {
                    for p in cocuklar(ps) {
                        let ad = if p.kind() == "identifier" { self.m(p) } else { alan(p, "name").map(|x| self.m(x)).unwrap_or_default() };
                        self.oz.fonksiyonlar[f].parametreler.push((ad, String::new()));
                    }
                }
                alt.fonk = Some(f);
            }
            "block" | "do_block" | "lambda" => {
                self.fonksiyona_gir(n, b, alt);
            }
            "assignment" => {
                let sol = alan(n, "left").map(|x| self.m(x)).unwrap_or_default();
                if let Some(v) = alan(n, "right") {
                    // self.table_name = "x"
                    if sol == "self.table_name" && v.kind() == "string" {
                        if let Some(s) = b.sinif {
                            self.oz.siniflar[s].tablo = Some((tirnaksiz(&self.m(v)), "table_name"));
                        }
                    }
                    let tip = if v.kind() == "call" {
                        let z = self.rb_zincir(v);
                        match z.first().map(String::as_str) {
                            Some(t) if t.starts_with("<yeni:") => t.trim_start_matches("<yeni:").trim_end_matches('>').to_string(),
                            Some(t) if t.chars().next().is_some_and(char::is_uppercase) => format!("={}", z.join(".")),
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    };
                    // @x = Model.find(...): örnek değişkeni sınıf düzeyinde (before_action ile başka yöntemde kurulur)
                    if sol.starts_with('@') && !tip.is_empty() {
                        self.sinif_alani(b, sol.clone(), tip.clone());
                    }
                    self.yerel(b, sol, tip);
                }
            }
            "call"
                if b.fonk.is_none()
                    && b.sinif.is_some()
                    && alan(n, "method").is_some_and(|m| matches!(self.m(m).as_str(), "before_action" | "prepend_before_action" | "before_filter" | "around_action")) =>
            {
                // before_action :a, :b, only: [:show], except: :index -> eylemden önce çalışan yöntemler
                let args = self.argumanlar(alan(n, "arguments"));
                let semboller =
                    |t: &str| -> Vec<String> { t.split([',', '[', ']', ' ']).filter_map(|x| x.trim().strip_prefix(':')).map(String::from).filter(|x| !x.is_empty()).collect() };
                let (mut adlar, mut only, mut except) = (vec![], vec![], vec![]);
                for a in &args {
                    if let Some(r) = a.strip_prefix("only:") {
                        only = semboller(r);
                    } else if let Some(r) = a.strip_prefix("except:") {
                        except = semboller(r);
                    } else if a.starts_with(':') {
                        adlar.extend(semboller(a));
                    }
                }
                let s = b.sinif.unwrap();
                for ad in adlar {
                    self.oz.siniflar[s].on_cagrilar.push((ad, only.clone(), except.clone()));
                }
                return false;
            }
            "call" => {
                let m = alan(n, "method").map(|x| self.m(x)).unwrap_or_default();
                let alici = alan(n, "receiver").map(|r| self.rb_zincir(r)).unwrap_or_default();
                let args = self.argumanlar(alan(n, "arguments"));
                let dinamik = matches!(m.as_str(), "send" | "public_send" | "__send__" | "instance_eval" | "instance_exec" | "method");
                let yeni = m == "new";
                let (alici, ad) = if yeni && alici.len() == 1 && alici[0].chars().next().is_some_and(char::is_uppercase) { (vec![], alici[0].clone()) } else { (alici, m) };
                self.cagri_ekle(b, HamCagri { alici, ad, satir: satir(n), argumanlar: args, yeni, dinamik });
            }
            "string" | "heredoc_body" => {
                self.dizgi(b, n);
                return false;
            }
            "binary" if alan(n, "operator").is_some_and(|o| matches!(o.kind(), "==" | "!=")) => self.karsilastirma(b, n),
            _ => {}
        }
        true
    }
}

// ---- Grafik: çözümlenmiş fonksiyonlar ve kenarlar ----

/// Grafikteki fonksiyon düğümü. Kimlik `fn:dosya#Sinif.ad` (üst düzeyde `fn:dosya#ad`; aynı adlı aşırı yüklemelerde
/// `@satır` eki); bir işlemin işleyicisiyse adaptörün verdiği kimlik (`fn:dosya#fonksiyon`) kullanılır ki uç -> işleyici
/// kenarıyla birleşsin
#[derive(Clone, Debug)]
pub struct Fonksiyon {
    pub id: String,
    pub dosya: String,
    pub sinif: Option<String>,
    pub ad: String,
    pub satir: usize,
    pub son: usize,
    pub anonim: bool,
}

impl Fonksiyon {
    /// Okunur ad: `Sinif.ad` ya da `ad`
    pub fn gorunen(&self) -> String {
        match &self.sinif {
            Some(s) if !s.is_empty() => format!("{s}.{}", self.ad),
            _ => self.ad.clone(),
        }
    }
}

/// Çağrı kenarı: kaynak ve hedef fonksiyon sıraları, çağrının satırı (kaynak fonksiyonun dosyasında), güven ve çözüm yolu
#[derive(Clone, Debug)]
pub struct Kenar {
    pub kaynak: usize,
    pub hedef: usize,
    pub satir: usize,
    pub guven: Guven,
    /// "ayni_sinif", "ust_sinif", "ayni_dosya", "iceaktarma", "paket", "alan_tipi", "parametre_tipi", "yerel_tip",
    /// "statik", "kurucu", "di_kaydi", "before_action", "arayuz_tek_uygulama", "mediatr", "kapsayici", "ad"
    pub cozum: &'static str,
}

/// Bir fonksiyondaki tablo etkisi: tablo düğümünün adı (ORM modeli ya da tablo), yazma mı, satır ve kaynağı (ORM)
#[derive(Clone, Debug)]
pub struct TabloEtkisi {
    pub tablo: String,
    pub yazma: bool,
    pub satir: usize,
    pub kaynak: &'static str,
}

/// Bir fonksiyondaki erişim denetimi: çağrı (`requireAccess(...)`), işaret (`@PreAuthorize(...)`) ya da sahiplik
/// karşılaştırması ("kosul": koşulda, "filtre": sorgu süzgecinde)
#[derive(Clone, Debug)]
pub struct ErisimDenetimi {
    pub ifade: String,
    pub izin: Option<String>,
    pub nesne: bool,
    pub satir: usize,
    pub tur: &'static str,
    /// Desen tek anlamlı mı: çağrı ve işaret evet; sahiplik karşılaştırmasında istek kimliği yalnızca adından tanındıysa hayır
    pub kesin: bool,
}

/// Bir çağrı yeri: hedefi çözülemeyen (kütüphane, dış istemci) çağrılar da durur. Dış çağrı ve geri alınabilirlik gibi
/// analizler ulaşılan fonksiyonların çağrı yerlerini buradan tarar
#[derive(Clone, Debug)]
#[allow(dead_code)] // dış çağrı ve geri alınabilirlik analizlerinin okuduğu arayüz
pub struct CagriYeri {
    /// Alıcı zinciri ve ad: "this.http.get", "axios.post", "requests.get"
    pub ifade: String,
    pub satir: usize,
    pub argumanlar: Vec<String>,
    pub hedef: Option<usize>,
    pub dinamik: bool,
    /// Hedefi projede çözülemeyen çağrının girdiği dış modül (içe aktarmayla bağlı kök ad, tipli alan / değişkenin tipi
    /// ya da sınıfın kütüphane tabanı): "django.core.mail.EmailMultiAlternatives", "pycurl", "net/http". Dış etki
    /// tespiti bu modülü sınıflandırır
    pub modul: Option<String>,
    /// Yöntem çağrısının olası hedefleri (polimorfik dağıtım): alt sınıfların aynı adlı yöntemleri ya da tipi bilinmeyen
    /// alıcıda projedeki aynı adlı yöntemler. Kenar değildir (yalnızca dış etki izlemesi kullanır), güveni en fazla inferred
    pub adaylar: Vec<usize>,
    /// Aday kümesi sınırı aştı: hedef belirsiz (dış etki için bilinmiyor)
    pub belirsiz: bool,
}

pub struct Grafik {
    pub fonksiyonlar: Vec<Fonksiyon>,
    pub kenarlar: Vec<Kenar>,
    /// Fonksiyon sırası -> giden kenar sıraları
    pub giden: Vec<Vec<usize>>,
    pub tablolar: Vec<Vec<TabloEtkisi>>,
    pub erisim: Vec<Vec<ErisimDenetimi>>,
    /// Fonksiyon başına hedefi bilinmeyen (dinamik) çağrı sayısı
    pub dinamik: Vec<usize>,
    pub cagri_yerleri: Vec<Vec<CagriYeri>>,
    /// Fonksiyonun kendisi bir erişim denetimi (ör. `requireAccess`): içine inilmez, denetim olarak sayılır
    pub denetim: Vec<bool>,
    /// Bulunan ORM modelleri, adaptörlerin `veri_modelleri` biçiminde ({ad, tablo, tablo_kaynagi, dosya, satir, alanlar})
    pub modeller: Vec<Value>,
    pub dosya_sayisi: usize,
    /// (dosya, içe aktarılan yerel ad) -> o adla içe aktarılan üst düzey fonksiyonlar: işleyici rota dosyasına içe
    /// aktarılmışsa (`router.get('/x', getOrders)`) asıl tanımı bulunur
    pub iceaktarilan: HashMap<(String, String), Vec<usize>>,
    /// Dosya -> istek yöntemine göre ayrılan dallar
    pub yontem_dallari: HashMap<String, Vec<YontemDali>>,
}

/// Kysely sorgu kurucusunun tablo alan yöntemleri: (yöntem, yazma mı)
const KYSELY: [(&str, bool); 10] = [
    ("selectFrom", false),
    ("insertInto", true),
    ("updateTable", true),
    ("deleteFrom", true),
    ("replaceInto", true),
    ("mergeInto", true),
    ("innerJoin", false),
    ("leftJoin", false),
    ("rightJoin", false),
    ("fullJoin", false),
];

/// ORM yöntem adından okuma / yazma (dil bağımsız)
fn yazma_islemi(ad: &str) -> Option<bool> {
    let a: String = ad.trim_end_matches(['!', '?']).chars().filter(|c| *c != '_').flat_map(char::to_lowercase).collect();
    const YAZ: [&str; 22] = [
        "save",
        "insert",
        "create",
        "update",
        "upsert",
        "delete",
        "destroy",
        "remove",
        "softdelete",
        "softremove",
        "restore",
        "add",
        "addrange",
        "addasync",
        "addrangeasync",
        "updaterange",
        "attach",
        "persist",
        "merge",
        "increment",
        "decrement",
        "touch",
    ];
    const YAZ_ON: [&str; 16] = [
        "delete",
        "update",
        "insert",
        "save",
        "create",
        "remove",
        "upsert",
        "destroy",
        "findoneand",
        "findbyidand",
        "firstorcreate",
        "updateorcreate",
        "getorcreate",
        "executeupdate",
        "executedelete",
        "bulk",
    ];
    const OKU_ON: [&str; 19] = [
        "find",
        "get",
        "count",
        "exist",
        "read",
        "query",
        "search",
        "stream",
        "first",
        "single",
        "last",
        "tolist",
        "include",
        "orderby",
        "aggregate",
        "paginate",
        "where",
        "select",
        "pluck",
    ];
    if YAZ.contains(&a.as_str()) || YAZ_ON.iter().any(|o| a.starts_with(o)) {
        return Some(true);
    }
    if OKU_ON.iter().any(|o| a.starts_with(o)) || matches!(a.as_str(), "all" | "any" | "anyasync" | "filter" | "filterby" | "exclude" | "values" | "scalar" | "scalars") {
        return Some(false);
    }
    None
}

/// Sınıf bir ORM modeli mi: kaynağı
fn orm_modeli(dil: Dil, s: &HamSinif) -> Option<&'static str> {
    let ust = |ad: &str| s.ustler.iter().any(|u| tip_parcala(u).0 == ad || u.trim().ends_with(ad));
    let isaret = |ad: &str| s.isaretler.iter().any(|i| i == ad);
    // Python taban sınıfı tam adıyla (`models.Model`, `Base`): `SQLModel`, `BaseModel` (Pydantic) ya da `ItemBase` (DTO
    // tabanı) adının sonu benzediği için ORM modeli sayılmaz; SQLModel tablosu `table=True` ile tanınır
    let ust_tam = |ad: &str| {
        s.ustler.iter().any(|u| {
            let t = tip_parcala(u).0;
            t == ad || t.rsplit('.').next() == Some(ad)
        })
    };
    match dil {
        Dil::Ts if isaret("Entity") => Some("typeorm"),
        Dil::Ts if isaret("Table") && s.tablo.as_ref().is_some_and(|t| t.1 == "@Table") && !ust("Model") => Some("kysely"),
        Dil::Ts if isaret("Table") || ust("Model") => Some("sequelize"),
        Dil::Py if s.tablo_bayragi || s.tablo.is_some() => Some("sqlalchemy"),
        Dil::Py if ust_tam("Model") && s.ustler.iter().any(|u| u.contains("models.")) => Some("django"),
        Dil::Py if ust_tam("Model") || ust_tam("Base") || ust_tam("DeclarativeBase") => Some("sqlalchemy"),
        Dil::Java | Dil::Kt if isaret("Entity") || isaret("Table") => Some("jpa"),
        Dil::Kt if EXPOSED.iter().any(|t| ust(t)) => Some("exposed"),
        Dil::Php if ust("Model") || ust("Authenticatable") || ust("Pivot") => Some("eloquent"),
        Dil::Rb if ust("ApplicationRecord") || s.ustler.iter().any(|u| u.contains("ActiveRecord")) => Some("activerecord"),
        Dil::Go if s.go_tablo.is_some() => Some("gorm"),
        _ => None,
    }
}

fn varsayilan_tablo(kaynak: &str, ad: &str) -> String {
    match kaynak {
        "typeorm" => yilan(ad),
        "sequelize" | "eloquent" | "activerecord" | "gorm" => cogul(&yilan(ad)),
        "sqlalchemy" | "django" => ad.to_lowercase(),
        // Exposed: nesne adı, sondaki "Table" atılır
        "exposed" => ad.strip_suffix("Table").filter(|x| !x.is_empty()).unwrap_or(ad).to_string(),
        _ => ad.to_string(),
    }
}

/// GORM yöntemleri: yazma mı
fn go_gorm(ad: &str) -> Option<bool> {
    match ad {
        "Create" | "Save" | "Delete" | "Updates" | "Update" | "UpdateColumn" | "UpdateColumns" | "FirstOrCreate" | "CreateInBatches" => Some(true),
        "First" | "Find" | "Take" | "Last" | "Count" | "Pluck" | "Scan" | "FirstOrInit" | "Model" => Some(false),
        _ => None,
    }
}

/// Yaygın yöntem adları: yalnızca ad eşleşmesiyle (alıcının tipi bilinmeden) kenar kurulmaz
fn yaygin_ad(ad: &str) -> bool {
    ad.len() < 6
        || matches!(
            ad.to_ascii_lowercase().as_str(),
            "tostring"
                | "foreach"
                | "filter"
                | "reduce"
                | "findone"
                | "findall"
                | "update"
                | "delete"
                | "create"
                | "remove"
                | "insert"
                | "values"
                | "length"
                | "execute"
                | "handle"
                | "process"
                | "validate"
                | "format"
                | "append"
                | "extend"
                | "commit"
                | "refresh"
                | "rollback"
                | "replace"
                | "render"
                | "toarray"
                | "tojson"
                | "include"
                | "includes"
                | "indexof"
                | "subscribe"
                | "resolve"
                | "reject"
                | "promise"
                | "finally"
                | "invoke"
                | "dispatch"
                | "request"
                | "response"
                | "logger"
                | "unwrap"
                | "expect"
                | "default"
                | "constructor"
                | "__init__"
                | "__construct"
                | "initialize"
                | "assert"
                | "equals"
                | "hashcode"
                | "compareto"
                | "getname"
                | "setname"
                | "dispose"
                | "should"
                | "string"
                | "number"
                | "object"
                | "boolean"
                | "message"
                | "status"
                | "select"
                | "update_all"
                | "destroy"
        )
}

/// Grafiğin kurulum bağlamı (geçici dizinler)
struct Kurulum<'a> {
    dosyalar: &'a [(String, &'a DosyaOzeti)],
    /// (dosya sırası, sınıf sırası)
    siniflar: Vec<(usize, usize)>,
    sinif_ad: HashMap<&'a str, Vec<usize>>,
    /// Go: aynı paketteki eş adlı tipler (yapı ve yöntemleri farklı dosyalarda) tek sınıftır: sınıf -> grubu
    go_grup: HashMap<usize, Vec<usize>>,
    /// (sınıf, yöntem adı) -> fonksiyonlar
    metotlar: HashMap<(usize, &'a str), Vec<usize>>,
    /// (dosya, ad) -> üst düzey fonksiyonlar
    ust_fonk: HashMap<(usize, &'a str), Vec<usize>>,
    /// ad -> adlı fonksiyonlar
    ad_dizini: HashMap<&'a str, Vec<usize>>,
    /// Fonksiyon sırası -> (dosya, fonksiyonun dosyadaki sırası)
    fonk_yeri: Vec<(usize, usize)>,
    fonk_sira: HashMap<(usize, usize), usize>,
    sinif_sira: HashMap<(usize, usize), usize>,
    /// Dosyanın uzantısız yolu -> dosya
    govde_yol: HashMap<String, usize>,
    /// Arayüz adı -> onu uygulayan sınıflar
    uygulayan: HashMap<String, Vec<usize>>,
    sabitler: HashMap<&'a str, &'a str>,
    /// Model sınıfı adı (küçük) -> (tablo düğümünün adı, kaynak)
    modeller: HashMap<String, (String, &'static str)>,
    /// Tablo adı (küçük) -> tablo düğümünün adı
    tablo_modeli: HashMap<String, String>,
    /// Prisma model erişimcisi (`user`) -> model adı
    prisma: HashMap<String, String>,
    /// Değişkene atanmış model (mongoose / sequelize): (dosya, değişken) -> tablo düğümünün adı
    model_degiskeni: HashMap<(usize, &'a str), String>,
    /// Dosya başına içe aktarmaların çözüldüğü dosyalar (içe aktarma sırasıyla): (modülün dosyaları, Python'da
    /// `modül.ad` alt modülünün dosyaları). Bir kez hesaplanır
    ice: Vec<Vec<(Vec<usize>, Vec<usize>)>>,
    /// Bağımlılık kayıtları: arayüz -> kayıtlı sınıflar
    di: HashMap<String, Vec<String>>,
    /// Uzantısız yolun her soneki (`asset.service`, `services/asset.service`, ...) -> dosyalar: yol takma adlı içe
    /// aktarmalar bir kez aranır
    sonek: HashMap<String, Vec<usize>>,
    /// MyBatis XML eşleyicileri: (eşleyici arayüzünün adı, deyim kimliği) -> SQL'in tabloları (tablo, yazma mı)
    mybatis: HashMap<(String, String), Vec<(String, bool)>>,
    /// Sınıf -> doğrudan alt sınıfları (polimorfik dağıtım için)
    alt_siniflar: HashMap<usize, Vec<usize>>,
    /// Proje dosyalarının klasör adları: çözülemeyen mutlak içe aktarma (Python / TypeScript) projeye mi ait
    klasorler: HashSet<String>,
    /// Proje dosyalarının paket / ad alanı önekleri (Java / Kotlin ilk iki parça, PHP ilk parça)
    paketler: HashSet<String>,
}

impl<'a> Kurulum<'a> {
    fn dosya_dil(&self, di: usize) -> Dil {
        self.dosyalar[di].1.dil.unwrap_or(Dil::Ts)
    }

    fn sinif(&self, ci: usize) -> &'a HamSinif {
        let (di, si) = self.siniflar[ci];
        &self.dosyalar[di].1.siniflar[si]
    }

    fn fonk(&self, fi: usize) -> &'a HamFonk {
        let (di, i) = self.fonk_yeri[fi];
        &self.dosyalar[di].1.fonksiyonlar[i]
    }

    fn klasor(&self, di: usize) -> &str {
        self.dosyalar[di].0.rsplit_once('/').map(|x| x.0).unwrap_or("")
    }

    /// İçe aktarma kaynağını (TS modül yolu, Python modülü, Go paket yolu) dosyalara çözer
    fn modul_dosyalari(&self, di: usize, modul: &str) -> Vec<usize> {
        let dil = self.dosya_dil(di);
        let mut adaylar: Vec<String> = vec![];
        match dil {
            Dil::Ts => {
                let m = modul.trim_end_matches(".js").trim_end_matches(".ts").trim_end_matches(".mjs").trim_end_matches(".cjs");
                if m.starts_with('.') {
                    let mut parca: Vec<&str> = self.klasor(di).split('/').filter(|x| !x.is_empty()).collect();
                    for p in m.split('/') {
                        match p {
                            "." | "" => {}
                            ".." => {
                                parca.pop();
                            }
                            x => parca.push(x),
                        }
                    }
                    let y = parca.join("/");
                    adaylar.push(format!("{y}/index"));
                    adaylar.push(y);
                } else {
                    // Yol takma adı (`src/...`, `@/...`, `~/...`): yolu bu sonla biten dosya; birden çoksa belirsiz
                    let m = m.trim_start_matches("@/").trim_start_matches("~/");
                    let mut son: Vec<usize> = self.sonek.get(m).into_iter().chain(self.sonek.get(&format!("{m}/index"))).flatten().copied().collect();
                    son.sort_unstable();
                    son.dedup();
                    return if son.len() <= 2 { son } else { vec![] };
                }
            }
            Dil::Py => {
                let noktalar = modul.chars().take_while(|c| *c == '.').count();
                let ad = modul[noktalar..].replace('.', "/");
                if noktalar > 0 {
                    let mut parca: Vec<&str> = self.klasor(di).split('/').filter(|x| !x.is_empty()).collect();
                    for _ in 1..noktalar {
                        parca.pop();
                    }
                    let taban = parca.join("/");
                    let y = if ad.is_empty() {
                        taban
                    } else if taban.is_empty() {
                        ad
                    } else {
                        format!("{taban}/{ad}")
                    };
                    adaylar.push(y.clone());
                    adaylar.push(format!("{y}/__init__"));
                } else {
                    let mut son: Vec<usize> = self.sonek.get(&ad).into_iter().chain(self.sonek.get(&format!("{ad}/__init__"))).flatten().copied().collect();
                    son.sort_unstable();
                    son.dedup();
                    return son;
                }
            }
            Dil::Go => {
                // Paket: içe aktarma yolunun sonuyla biten klasördeki bütün dosyalar
                let parca: Vec<&str> = modul.split('/').collect();
                for n in (1..=parca.len().min(4)).rev() {
                    let son = parca[parca.len() - n..].join("/");
                    let v: Vec<usize> =
                        (0..self.dosyalar.len()).filter(|d| self.dosya_dil(*d) == Dil::Go && (self.klasor(*d) == son || self.klasor(*d).ends_with(&format!("/{son}")))).collect();
                    if !v.is_empty() {
                        return v;
                    }
                }
                return vec![];
            }
            _ => return vec![],
        }
        adaylar.iter().filter_map(|a| self.govde_yol.get(a.as_str()).copied()).collect()
    }

    /// Tip adını sınıflara çözer: tek aday; birden çok adayda aynı dosya, içe aktarma ya da paket ile seçilir
    fn sinif_coz(&self, tip: &str, di: usize) -> Vec<usize> {
        let (taban, _) = tip_parcala(tip);
        let Some(adaylar) = self.sinif_ad.get(taban.as_str()) else { return vec![] };
        let dil = self.dosya_dil(di);
        // Java ve Kotlin aynı JVM tiplerini paylaşır
        let jvm = |d: Dil| if d == Dil::Kt { Dil::Java } else { d };
        let adaylar: Vec<usize> = adaylar.iter().copied().filter(|c| jvm(self.dosya_dil(self.siniflar[*c].0)) == jvm(dil)).collect();
        if adaylar.len() <= 1 {
            return adaylar;
        }
        let ayni: Vec<usize> = adaylar.iter().copied().filter(|c| self.siniflar[*c].0 == di).collect();
        if !ayni.is_empty() {
            return ayni;
        }
        let oz = self.dosyalar[di].1;
        for (ii, (yerel, modul, asil)) in oz.iceaktarma.iter().enumerate() {
            if *yerel == taban || *asil == taban || modul.ends_with(&format!(".{taban}")) || modul.ends_with(&format!("\\{taban}")) {
                let dosyalar = &self.ice[di][ii].0;
                let v: Vec<usize> = adaylar.iter().copied().filter(|c| dosyalar.contains(&self.siniflar[*c].0)).collect();
                if v.len() == 1 {
                    return v;
                }
                // Java / C# / PHP: tam adın paketi
                let paket = modul.rsplit_once(['.', '\\']).map(|x| x.0).unwrap_or("").replace('\\', ".");
                let v: Vec<usize> = adaylar.iter().copied().filter(|c| !paket.is_empty() && self.dosyalar[self.siniflar[*c].0].1.paket.replace('\\', ".") == paket).collect();
                if v.len() == 1 {
                    return v;
                }
            }
        }
        // Aynı paket ve klasör
        let v: Vec<usize> =
            adaylar.iter().copied().filter(|c| self.dosyalar[self.siniflar[*c].0].1.paket == oz.paket && self.klasor(self.siniflar[*c].0) == self.klasor(di)).collect();
        if !v.is_empty() {
            return v;
        }
        // Go: eş adlı tiplerin hepsi tek grup
        let gruplar: BTreeSet<usize> = adaylar.iter().map(|c| self.go_grup.get(c).and_then(|g| g.first()).copied().unwrap_or(*c)).collect();
        if gruplar.len() == 1 {
            return adaylar;
        }
        vec![]
    }

    /// Sınıfın ve üst sınıflarının (en çok 6 kat) yöntemi
    fn metot_bul(&self, ci: usize, ad: &str) -> Vec<usize> {
        let mut gorulen = HashSet::new();
        let mut yigin = vec![(ci, 0usize)];
        while let Some((c, d)) = yigin.pop() {
            if !gorulen.insert(c) || d > 6 {
                continue;
            }
            let grup = self.go_grup.get(&c).cloned().unwrap_or_else(|| vec![c]);
            let mut v = vec![];
            for g in &grup {
                if let Some(m) = self.metotlar.get(&(*g, ad)) {
                    v.extend(m.iter().copied());
                }
            }
            if !v.is_empty() {
                return v;
            }
            let di = self.siniflar[c].0;
            for g in grup {
                for u in &self.sinif(g).ustler {
                    for uc in self.sinif_coz(u, di) {
                        yigin.push((uc, d + 1));
                    }
                }
            }
        }
        vec![]
    }

    /// Sınıfın (ve üst sınıflarının) alanının tipi ve tipin bulunduğu dosya
    fn alan_tipi(&self, ci: usize, ad: &str) -> Option<(String, usize)> {
        if ad.is_empty() {
            return None;
        }
        let mut gorulen = HashSet::new();
        let mut yigin = vec![(ci, 0usize)];
        while let Some((c, d)) = yigin.pop() {
            if !gorulen.insert(c) || d > 6 {
                continue;
            }
            let grup = self.go_grup.get(&c).cloned().unwrap_or_else(|| vec![c]);
            for g in &grup {
                if let Some((_, t)) = self.sinif(*g).alanlar.iter().find(|(a, _)| a == ad) {
                    return Some((t.clone(), self.siniflar[*g].0));
                }
            }
            let di = self.siniflar[c].0;
            for g in grup {
                for u in &self.sinif(g).ustler {
                    for uc in self.sinif_coz(u, di) {
                        yigin.push((uc, d + 1));
                    }
                }
            }
        }
        None
    }

    /// Tipin yöntemi: sınıfsa doğrudan (tek hedef certain), arayüzse projedeki tek uygulaması (inferred)
    fn tipte_metot(&self, tip: &str, di: usize, ad: &str, cozum: &'static str) -> Option<(Vec<usize>, Guven, &'static str)> {
        let siniflar = self.sinif_coz(tip, di);
        let c = *siniflar.first()?;
        if siniflar.len() > 1 && !self.go_grup.get(&c).is_some_and(|g| siniflar.iter().all(|x| g.contains(x))) {
            return None;
        }
        if self.sinif(c).arayuz {
            let taban = tip_parcala(tip).0;
            // Kodda tek bağımlılık kaydı varsa hedef statik ve tektir
            if let Some([kayitli]) = self.di.get(&taban).map(Vec::as_slice) {
                if let [tek] = self.sinif_coz(kayitli, self.siniflar[c].0).as_slice() {
                    let m = self.metot_bul(*tek, ad);
                    if m.len() == 1 {
                        return Some((m, Guven::Certain, "di_kaydi"));
                    }
                }
            }
            let uyg = self.uygulayan.get(&taban)?;
            if uyg.len() != 1 {
                return None;
            }
            let m = self.metot_bul(uyg[0], ad);
            return (!m.is_empty() && m.len() <= 3).then_some((m, Guven::Inferred, "arayuz_tek_uygulama"));
        }
        let m = self.metot_bul(c, ad);
        match m.len() {
            0 => None,
            1 => Some((m, Guven::Certain, cozum)),
            2..=3 => Some((m, Guven::Inferred, cozum)),
            _ => None,
        }
    }

    /// Fonksiyonda bir adın tipi: parametre, tipli yerel değişken ya da atanan ifadeden çıkan sınıf
    fn ad_tipi(&self, fi: usize, ad: &str) -> Option<String> {
        let f = self.fonk(fi);
        let (di, _) = self.fonk_yeri[fi];
        if let Some((_, t)) = f.parametreler.iter().rev().find(|(a, t)| a == ad && !t.is_empty()) {
            return Some(t.clone());
        }
        for (a, t) in f.yereller.iter().rev() {
            if a != ad || t.is_empty() {
                continue;
            }
            match t.strip_prefix('=') {
                None => return Some(t.clone()),
                Some(ifade) => {
                    if let Some(t) = self.ifade_tipi(ifade, di) {
                        return Some(t);
                    }
                }
            }
        }
        None
    }

    /// Atanan ifadeden tip: `session.get|Item` -> Item; `User::find` / `User.find` / `User.objects.get` / `X(...)` -> X
    /// (modelin ya da sınıfın örneği); ilk ad projede bir sınıf değilse None
    fn ifade_tipi(&self, ifade: &str, di: usize) -> Option<String> {
        if let Some((_, m)) = ifade.split_once('|') {
            let m = m.trim();
            return (!m.is_empty()).then(|| m.to_string());
        }
        let ilk = ifade.split(['.', ':', '(']).next().unwrap_or("").trim();
        (!ilk.is_empty() && !self.sinif_coz(ilk, di).is_empty()).then(|| ilk.to_string())
    }

    /// Alıcı zincirinin tipi: ilk halka (this, alan, parametre, yerel, sınıf adı, kapsayıcı) ve alan alan ilerleyerek.
    /// Dönen: (tip metni, tipin bulunduğu dosya, güven, çözüm, statik mi)
    fn alici_tipi(&self, fi: usize, alici: &[String]) -> Option<(String, usize, Guven, &'static str, bool)> {
        let (di, _) = self.fonk_yeri[fi];
        let f = self.fonk(fi);
        let ci = f.sinif.map(|s| self.sinif_sira[&(di, s)]);
        let ilk = alici.first()?;
        let (mut tip, mut tdi, mut guven, mut cozum, mut statik) = if ilk == "this" {
            (self.sinif(ci?).ad.clone(), di, Guven::Certain, "ayni_sinif", false)
        } else if let Some(t) = ilk.strip_prefix("<kap:").and_then(|x| x.strip_suffix('>')) {
            (t.to_string(), di, Guven::Inferred, "kapsayici", false)
        } else if let Some(t) = ilk.strip_prefix("<yeni:").and_then(|x| x.strip_suffix('>')) {
            (t.to_string(), di, Guven::Certain, "kurucu", false)
        } else if ilk.starts_with('<') {
            return None;
        } else if let Some(t) = self.ad_tipi(fi, ilk) {
            (t, di, Guven::Certain, if f.parametreler.iter().any(|p| p.0 == *ilk) { "parametre_tipi" } else { "yerel_tip" }, false)
        } else if let Some((t, d)) = ci.and_then(|c| self.alan_tipi(c, ilk.trim_start_matches(['$', '@']))) {
            let t = match t.strip_prefix('=') {
                Some(ifade) => self.ifade_tipi(ifade, d)?,
                None => t,
            };
            (t, d, Guven::Certain, "alan_tipi", false)
        } else if !self.sinif_coz(ilk, di).is_empty() {
            (ilk.clone(), di, Guven::Certain, "statik", true)
        } else {
            return None;
        };
        for halka in &alici[1..] {
            let cs = self.sinif_coz(&tip, tdi);
            let c = *cs.first()?;
            let (t, d) = self.alan_tipi(c, halka)?;
            tip = t;
            tdi = d;
            cozum = "alan_tipi";
            statik = false;
            if cs.len() > 1 {
                guven = guven.min(Guven::Inferred);
            }
        }
        Some((tip, tdi, guven, cozum, statik))
    }

    /// Alıcısız çağrı: aynı sınıfın yöntemi (Java, C#, Ruby), aynı dosyadaki ya da içe aktarılan fonksiyon, aynı paketteki
    /// fonksiyon (Go), sınıf kurucusu (Python `X()`), PHP genel yardımcısı; son çare projede tek olan ad (inferred)
    fn yalin_cagri(&self, di: usize, ci: Option<usize>, dil: Dil, ad: &str) -> Option<(Vec<usize>, Guven, &'static str)> {
        let oz = self.dosyalar[di].1;
        if matches!(dil, Dil::Java | Dil::Cs | Dil::Rb | Dil::Kt) {
            if let Some(c) = ci {
                let m = self.metot_bul(c, ad);
                match m.len() {
                    0 => {}
                    1 => return Some((m, Guven::Certain, "ayni_sinif")),
                    2..=3 => return Some((m, Guven::Inferred, "ayni_sinif")),
                    _ => return None,
                }
            }
        }
        if let Some(m) = self.ust_fonk.get(&(di, ad)) {
            if m.len() == 1 {
                return Some((m.clone(), Guven::Certain, "ayni_dosya"));
            }
        }
        for (ii, (yerel, _, asil)) in oz.iceaktarma.iter().enumerate() {
            if yerel != ad {
                continue;
            }
            let asil = if asil == "default" || asil == "*" || asil.is_empty() { ad } else { asil.as_str() };
            let dosyalar = &self.ice[di][ii].0;
            let mut v: Vec<usize> = dosyalar.iter().filter_map(|d| self.ust_fonk.get(&(*d, asil))).flatten().copied().collect();
            v.sort_unstable();
            v.dedup();
            if v.len() == 1 {
                return Some((v, Guven::Certain, "iceaktarma"));
            }
            // İçe aktarılan sınıfın kurucusu (Python `X()`)
            if dil == Dil::Py {
                let cs: Vec<usize> = self.sinif_coz(asil, di).into_iter().filter(|c| dosyalar.contains(&self.siniflar[*c].0)).collect();
                if let [tek] = cs.as_slice() {
                    let m = self.metot_bul(*tek, "__init__");
                    if m.len() == 1 {
                        return Some((m, Guven::Certain, "kurucu"));
                    }
                }
            }
        }
        if dil == Dil::Go {
            let k = self.klasor(di);
            let v: Vec<usize> =
                self.ust_fonk.iter().filter(|((d, a), _)| *a == ad && self.klasor(*d) == k && self.dosya_dil(*d) == Dil::Go).flat_map(|(_, f)| f.iter().copied()).collect();
            if v.len() == 1 {
                return Some((v, Guven::Certain, "paket"));
            }
        }
        if dil == Dil::Py {
            if let [tek] = self.sinif_coz(ad, di).as_slice() {
                let m = self.metot_bul(*tek, "__init__");
                if m.len() == 1 {
                    return Some((m, Guven::Certain, "kurucu"));
                }
            }
        }
        if dil == Dil::Php {
            let v: Vec<usize> =
                self.ad_dizini.get(ad).into_iter().flatten().copied().filter(|f| self.fonk(*f).sinif.is_none() && self.dosya_dil(self.fonk_yeri[*f].0) == Dil::Php).collect();
            if v.len() == 1 {
                return Some((v, Guven::Certain, "ayni_dosya"));
            }
        }
        self.ad_eslesmesi(di, ad)
    }

    /// Projede tek olan ad: inferred. Alıcısız çağrı PHP, Python, TypeScript ve Go'da bir sınıfın yöntemine gidemez
    /// (`redirect()` Laravel'in küresel yardımcısıdır, aynı adlı `SocialiteController::redirect` değil): aday yalnızca
    /// sınıf dışı fonksiyonlardır
    fn ad_eslesmesi(&self, di: usize, ad: &str) -> Option<(Vec<usize>, Guven, &'static str)> {
        if yaygin_ad(ad) {
            return None;
        }
        let dil = self.dosya_dil(di);
        let yalniz_fonksiyon = matches!(dil, Dil::Php | Dil::Py | Dil::Ts | Dil::Go);
        let v: Vec<usize> = self
            .ad_dizini
            .get(ad)
            .into_iter()
            .flatten()
            .copied()
            .filter(|f| self.dosya_dil(self.fonk_yeri[*f].0) == dil && !(yalniz_fonksiyon && self.fonk(*f).sinif.is_some()))
            .collect();
        (v.len() == 1).then_some((v, Guven::Inferred, "ad"))
    }

    /// `modul.fonksiyon()`: içe aktarılan modül (TS ad alanı / varsayılan dışa aktarım, Python modülü, Go paketi)
    fn modul_cagrisi(&self, di: usize, alici: &str, ad: &str) -> Option<(Vec<usize>, Guven, &'static str)> {
        let oz = self.dosyalar[di].1;
        let dil = self.dosya_dil(di);
        for (ii, (yerel, _, _)) in oz.iceaktarma.iter().enumerate() {
            if yerel != alici {
                continue;
            }
            // Python `from app import crud`: modül app/crud.py (alt modül)
            let (m, alt) = &self.ice[di][ii];
            let dosyalar: Vec<usize> = m.iter().chain(alt.iter()).copied().collect();
            let mut v: Vec<usize> = dosyalar.iter().filter_map(|d| self.ust_fonk.get(&(*d, ad))).flatten().copied().collect();
            v.sort_unstable();
            v.dedup();
            if v.len() == 1 {
                return Some((v, Guven::Certain, if dil == Dil::Go { "paket" } else { "iceaktarma" }));
            }
            // Varsayılan dışa aktarılan nesne ya da sınıf örneği (`export default new UserService()`): adıyla yöntem
            let m: Vec<usize> = dosyalar
                .iter()
                .flat_map(|d| self.dosyalar[*d].1.fonksiyonlar.iter().enumerate().filter(|(_, f)| f.ad == ad && f.sinif.is_some()).map(|(i, _)| self.fonk_sira[&(*d, i)]))
                .collect();
            if m.len() == 1 {
                return Some((m, Guven::Inferred, "iceaktarma"));
            }
        }
        None
    }

    /// İçe aktarılan model değişkeni (`import User from '../models/user'`, mongoose / sequelize)
    fn iceaktarilan_model(&self, di: usize, ad: &str) -> Option<String> {
        let oz = self.dosyalar[di].1;
        let ii = oz.iceaktarma.iter().position(|(y, _, _)| y == ad)?;
        self.ice[di][ii].0.iter().find_map(|d| self.dosyalar[*d].1.model_atamalari.first().map(|(_, t, _)| t.clone()))
    }

    /// MediatR isteğinin işleyicisi: isteğin tipi (`new Edit.Command(...)` ya da tipli değişken) ile
    /// `IRequestHandler<Command, ...>` / `INotificationHandler<...>` uygulayan sınıfın `Handle` yöntemi. İç içe sınıflar
    /// (`Edit.Command`, `Edit.Handler`) aynı dosyada aranır; tek aday yoksa None
    fn mediatr_hedefi(&self, fi: usize, arg: &str) -> Option<Vec<usize>> {
        let a = arg.trim();
        let tip = match a.strip_prefix("new ") {
            Some(r) => r.split(['(', '{']).next().unwrap_or("").trim().to_string(),
            None => self.ad_tipi(fi, a)?,
        };
        let parca: Vec<&str> = tip.split('.').map(str::trim).collect();
        let son = *parca.last()?;
        let dis = (parca.len() >= 2).then(|| parca[parca.len() - 2]);
        let (cdi, _) = self.fonk_yeri[fi];
        let dis_dosyalari: Vec<usize> = dis.map(|d| self.sinif_coz(d, cdi).into_iter().map(|c| self.siniflar[c].0).collect()).unwrap_or_default();
        let mut v = vec![];
        for ci in 0..self.siniflar.len() {
            if self.dosya_dil(self.siniflar[ci].0) != Dil::Cs {
                continue;
            }
            let isler = self.sinif(ci).ustler.iter().any(|u| {
                let (t, args) = tip_parcala(u);
                matches!(t.as_str(), "IRequestHandler" | "INotificationHandler" | "AsyncRequestHandler" | "RequestHandler")
                    && args.first().is_some_and(|x| x == son)
                    && dis.is_none_or(|_| dis_dosyalari.contains(&self.siniflar[ci].0))
            });
            if isler {
                v.extend(self.metot_bul(ci, "Handle"));
            }
        }
        (v.len() == 1).then_some(v)
    }

    /// EF Core: DbSet özelliğinin varlık tipi (herhangi bir DbContext sınıfında)
    fn dbset_varligi(&self, ozellik: &str) -> Option<String> {
        for ci in 0..self.siniflar.len() {
            let s = self.sinif(ci);
            if !s.ustler.iter().any(|u| tip_parcala(u).0.ends_with("DbContext")) {
                continue;
            }
            if let Some((_, t)) = s.alanlar.iter().find(|(a, _)| a == ozellik) {
                let (taban, args) = tip_parcala(t);
                if taban == "DbSet" {
                    let e = args.first()?.clone();
                    return Some(self.modeller.get(&e.to_lowercase()).map(|m| m.0.clone()).unwrap_or(e));
                }
            }
        }
        None
    }

    fn model_adi(&self, ad: &str) -> Option<(String, &'static str)> {
        self.modeller.get(&tip_parcala(ad).0.to_lowercase()).cloned()
    }

    fn tablo_adi(&self, t: &str) -> String {
        let t = t.split_whitespace().next().unwrap_or(t);
        let t = t.rsplit('.').next().unwrap_or(t);
        self.tablo_modeli.get(&t.to_lowercase()).cloned().unwrap_or_else(|| t.to_string())
    }
}

/// Bir fonksiyonun çözüm sonucu
struct FonkSonucu {
    /// (hedef, satır, güven, çözüm)
    kenarlar: Vec<(usize, usize, Guven, &'static str)>,
    tablolar: Vec<TabloEtkisi>,
    erisim: Vec<ErisimDenetimi>,
    dinamik: usize,
    yerler: Vec<CagriYeri>,
}

impl FonkSonucu {
    fn tablo(&mut self, tablo: String, yazma: bool, satir: usize, kaynak: &'static str) {
        if tablo.is_empty() || tablo.starts_with('<') {
            return;
        }
        if let Some(t) = self.tablolar.iter_mut().find(|t| t.tablo == tablo) {
            if yazma && !t.yazma {
                t.yazma = true;
                t.satir = satir;
                t.kaynak = kaynak;
            }
            return;
        }
        if self.tablolar.len() < 48 {
            self.tablolar.push(TabloEtkisi { tablo, yazma, satir, kaynak });
        }
    }
}

impl<'a> Kurulum<'a> {
    fn fonksiyon_coz(&self, fi: usize) -> FonkSonucu {
        let (di, _) = self.fonk_yeri[fi];
        let f = self.fonk(fi);
        let dil = self.dosya_dil(di);
        let ci = f.sinif.map(|s| self.sinif_sira[&(di, s)]);
        let mut s = FonkSonucu { kenarlar: vec![], tablolar: vec![], erisim: vec![], dinamik: 0, yerler: vec![] };
        for c in &f.cagrilar {
            let ifade = if c.alici.is_empty() { c.ad.clone() } else { format!("{}.{}", c.alici.join("."), c.ad) };
            let mut yer = CagriYeri { ifade, satir: c.satir, argumanlar: c.argumanlar.clone(), hedef: None, dinamik: c.dinamik, modul: None, adaylar: vec![], belirsiz: false };
            // Erişim denetimi çağrısı
            if erisim_cagrisi(&c.alici, &c.ad) && s.erisim.len() < 16 {
                let izin = c.argumanlar.iter().find_map(|a| sabit_atiflari(a).into_iter().find_map(|m| self.sabitler.get(m).map(|v| v.to_string())));
                // İzin adı: enum sabitinin değeri ya da tırnaklı ilk argüman (`authorize('update', $post)`); değişken izin değildir
                let izin = izin.or_else(|| {
                    c.argumanlar
                        .first()
                        .filter(|a| a.starts_with(['\'', '"', '`']))
                        .map(|a| tirnaksiz(a))
                        .filter(|a| !a.is_empty() && !a.contains([' ', '(', '{', '$', '[']) && a.len() < 64)
                });
                let ifade = kisa(&format!("{}({})", yer.ifade, c.argumanlar.first().map(String::as_str).unwrap_or("")), 120);
                // `kesin` yalnızca çatının bilinen yetki API'sinde; adından tanınan proje denetimi en fazla inferred
                let kesin = bilinen_yetki_api(&c.alici, &c.ad);
                s.erisim.push(ErisimDenetimi { ifade, izin, nesne: nesne_duzeyi(&c.ad, &c.argumanlar), satir: c.satir, tur: "cagri", kesin });
            }
            // Sorgu süzgecinde sahiplik: where('album.ownerId', '=', auth.user.id) / filter(owner=request.user)
            if matches!(c.ad.as_str(), "where" | "andWhere" | "orWhere" | "filter" | "filter_by" | "Where") && c.argumanlar.len() >= 2 && s.erisim.len() < 16 {
                let a: Vec<String> = c.argumanlar.iter().map(|x| x.replace(['\'', '"', '`'], "")).filter(|x| x != "=").collect();
                if let Some(kesin) = sahiplik_kimligi(&a.join(" == "), None) {
                    s.erisim.push(ErisimDenetimi {
                        ifade: kisa(&format!("{}({})", c.ad, c.argumanlar.join(", ")), 120),
                        izin: None,
                        nesne: true,
                        satir: c.satir,
                        tur: "filtre",
                        kesin,
                    });
                }
            }
            if c.dinamik {
                s.dinamik += 1;
                s.yerler.push(yer);
                continue;
            }
            self.dogrudan_tablolar(fi, dil, c, &mut s);
            // MediatR: `_mediator.Send(new Edit.Command(...))` -> isteği işleyen `IRequestHandler<Command, ...>` sınıfının
            // `Handle` yöntemi (eşleme çalışma anında kayıtla yapılır: inferred)
            let mediatr = (dil == Dil::Cs && matches!(c.ad.as_str(), "Send" | "Publish") && c.alici.last().is_some_and(|a| a.to_lowercase().contains("mediator")))
                .then(|| self.mediatr_hedefi(fi, c.argumanlar.first().map(String::as_str).unwrap_or("")))
                .flatten();
            // Kenar ve tip tabanlı tablo etkisi
            let hedef: Option<(Vec<usize>, Guven, &'static str)> = if let Some(m) = mediatr {
                Some((m, Guven::Inferred, "mediatr"))
            } else if c.yeni {
                match self.sinif_coz(&c.ad, di).as_slice() {
                    [tek] => {
                        let m = ["constructor", "__init__", "__construct", "initialize", c.ad.as_str()]
                            .iter()
                            .map(|a| self.metot_bul(*tek, a))
                            .find(|m| !m.is_empty())
                            .unwrap_or_default();
                        (m.len() == 1).then_some((m, Guven::Certain, "kurucu"))
                    }
                    _ => None,
                }
            } else if c.alici.is_empty() {
                self.yalin_cagri(di, ci, dil, &c.ad)
            } else if c.alici.len() == 1 && c.alici[0] == "super" {
                ci.and_then(|c0| {
                    let ust: Vec<usize> = self.sinif(c0).ustler.iter().flat_map(|u| self.sinif_coz(u, di)).collect();
                    let m: Vec<usize> = ust.iter().flat_map(|u| self.metot_bul(*u, &c.ad)).collect();
                    (m.len() == 1).then_some((m, Guven::Certain, "ust_sinif"))
                })
            } else if let Some((tip, tdi, g0, cozum, statik)) = self.alici_tipi(fi, &c.alici) {
                self.tipli_tablo(&tip, tdi, statik, c, &mut s);
                // MyBatis eşleyici arayüzü: deyimin SQL'i XML'de (`<mapper namespace="...ArticleMapper">`)
                if let Some(v) = self.mybatis.get(&(tip_parcala(&tip).0, c.ad.clone())) {
                    for (t, y) in v {
                        s.tablo(self.tablo_adi(t), *y, c.satir, "mybatis");
                    }
                }
                self.tipte_metot(&tip, tdi, &c.ad, cozum).map(|(m, g, cz)| (m, g.min(g0), cz))
            } else if c.alici.len() == 1 {
                let r = self.modul_cagrisi(di, &c.alici[0], &c.ad);
                if r.is_none() {
                    // Mongoose / Sequelize model değişkeni (aynı dosyada ya da içe aktarılan)
                    let t = self.model_degiskeni.get(&(di, c.alici[0].as_str())).cloned().or_else(|| self.iceaktarilan_model(di, &c.alici[0]));
                    if let (Some(t), Some(y)) = (t, yazma_islemi(&c.ad)) {
                        s.tablo(t, y, c.satir, "mongoose");
                    }
                }
                r
            } else {
                // EF Core: _context.Users.Add(...): DbContext alanının DbSet özelliği
                if dil == Dil::Cs {
                    if let (Some(y), Some(e)) = (yazma_islemi(&c.ad), self.dbset_varligi(&c.alici[c.alici.len() - 1])) {
                        s.tablo(e, y, c.satir, "ef");
                    }
                }
                None
            };
            self.dis_bilgisi(fi, c, hedef.as_ref().map(|h| h.0.as_slice()), &mut yer);
            if let Some((hedefler, g, cozum)) = hedef {
                yer.hedef = hedefler.first().copied();
                for h in hedefler {
                    s.kenarlar.push((h, c.satir, g, cozum));
                }
            }
            if s.yerler.len() < EN_FAZLA_CAGRI {
                s.yerler.push(yer);
            }
        }
        // Rails: denetleyicinin (ve üst sınıflarının) before_action süzgeçleri eylemden önce çalışır
        if dil == Dil::Rb {
            if let Some(c0) = ci {
                let mut siniflar = vec![c0];
                let mut i = 0;
                while i < siniflar.len() && siniflar.len() < 8 {
                    let c = siniflar[i];
                    for u in &self.sinif(c).ustler {
                        siniflar.extend(self.sinif_coz(u, self.siniflar[c].0));
                    }
                    i += 1;
                }
                for c in siniflar {
                    for (cb, only, except) in &self.sinif(c).on_cagrilar {
                        if *cb == f.ad || (!only.is_empty() && !only.contains(&f.ad)) || except.contains(&f.ad) {
                            continue;
                        }
                        let m = self.metot_bul(c0, cb);
                        if m.len() == 1 {
                            s.kenarlar.push((m[0], self.sinif(c).satir, Guven::Certain, "before_action"));
                        }
                    }
                }
            }
        }
        for (t, y, satir) in &f.sql {
            s.tablo(self.tablo_adi(t), *y, *satir, "sql");
        }
        // Sahiplik karşılaştırmaları (koşulda ya da sorgu süzgecinde) ve yetki işaretleri
        let suzgec: HashSet<usize> =
            f.cagrilar.iter().filter(|c| matches!(c.ad.as_str(), "where" | "filter" | "filter_by" | "andWhere" | "Where" | "exclude")).map(|c| c.satir).collect();
        for (k, satir, kosul) in &f.karsilastirmalar {
            let tur = match (kosul, suzgec.contains(satir)) {
                (true, _) => "kosul",
                (false, true) => "filtre",
                (false, false) => continue,
            };
            if let Some(kesin) = sahiplik_kimligi(k, None).filter(|_| s.erisim.len() < 16 && !s.erisim.iter().any(|e| e.satir == *satir)) {
                s.erisim.push(ErisimDenetimi { ifade: k.clone(), izin: None, nesne: true, satir: *satir, tur, kesin });
            }
        }
        for (i, satir) in &f.isaretler {
            let ad = i.trim_start_matches(['@', '[']).split('(').next().unwrap_or("").trim().to_string();
            // `[Authorize]` yalnızca kimlik ister (giriş kararı adaptörde); politika ya da rol taşıyorsa yetki denetimidir
            let yetki = ad != "Authorize" || i.contains("Policy") || i.contains("Roles");
            if yetki
                && matches!(
                    ad.as_str(),
                    "PreAuthorize" | "PostAuthorize" | "Secured" | "RolesAllowed" | "Authorize" | "permission_required" | "user_passes_test" | "permission_classes"
                )
            {
                let nesne = i.contains('#') || i.contains("hasPermission") || i.contains("principal") || i.contains("Resource");
                s.erisim.push(ErisimDenetimi { ifade: i.clone(), izin: None, nesne, satir: *satir, tur: "isaret", kesin: true });
            }
        }
        s
    }

    /// Alıcının tipine bakmadan okunan tablo etkileri: Kysely tablo adı, Prisma model erişimcisi, PHP sorgu kurucusu,
    /// SQLAlchemy / SQLModel / Django / Flask-SQLAlchemy kalıpları, GORM argümanının tipi
    fn dogrudan_tablolar(&self, fi: usize, dil: Dil, c: &HamCagri, s: &mut FonkSonucu) {
        let (di, _) = self.fonk_yeri[fi];
        // Birleştirme (Exposed `Users.join(Follows, ...)` / `innerJoin(Follows)`, SQLAlchemy `.join(Item)`): birleştirilen
        // model de okunur
        if matches!(dil, Dil::Kt | Dil::Py) && matches!(c.ad.as_str(), "join" | "innerJoin" | "leftJoin" | "rightJoin" | "fullJoin" | "crossJoin" | "outerjoin" | "join_from") {
            let ilk = c.argumanlar.first().map(|a| a.trim().to_string()).unwrap_or_default();
            // Birleştirilen model ve (Exposed) birleştirmenin alıcısı olan model
            for ad in [ilk.as_str(), c.alici.last().map(String::as_str).unwrap_or("")] {
                if !ad.is_empty() && !ad.contains(['(', ' ', '{', '<']) {
                    if let Some((m, k)) = self.model_adi(ad) {
                        s.tablo(m, false, c.satir, k);
                    }
                }
            }
        }
        match dil {
            Dil::Ts if !c.alici.is_empty() => {
                if let Some((_, yaz)) = KYSELY.iter().find(|(a, _)| *a == c.ad) {
                    if let Some(t) = c.argumanlar.first().filter(|a| a.starts_with(['\'', '"', '`'])) {
                        s.tablo(self.tablo_adi(&tirnaksiz(t)), *yaz, c.satir, "kysely");
                    }
                }
                if c.alici.len() >= 2 {
                    if let (Some(m), Some(y)) = (self.prisma.get(c.alici[c.alici.len() - 1].as_str()), yazma_islemi(&c.ad)) {
                        s.tablo(m.clone(), y, c.satir, "prisma");
                    }
                }
            }
            Dil::Php => {
                if c.alici == ["DB"] && c.ad == "table" {
                    if let Some(t) = c.argumanlar.first() {
                        s.tablo(self.tablo_adi(&tirnaksiz(t)), false, c.satir, "query_builder");
                    }
                }
            }
            Dil::Py => {
                let ilk = c.argumanlar.first().map(|a| a.trim().to_string()).unwrap_or_default();
                let m_ilk = self.model_adi(&ilk).filter(|_| !ilk.is_empty() && !ilk.contains(['(', ' ']));
                match c.ad.as_str() {
                    "select" | "query" | "get" | "get_or_404" | "get_object_or_404" | "get_list_or_404" | "scalar" | "exists" => {
                        if let Some((m, k)) = m_ilk {
                            s.tablo(m, false, c.satir, k);
                        }
                    }
                    "delete" | "update" | "insert" if c.alici.is_empty() || c.alici.last().is_some_and(|x| x == "sqlalchemy" || x == "sa") => {
                        if let Some((m, k)) = m_ilk {
                            s.tablo(m, true, c.satir, k);
                        }
                    }
                    "add" | "delete" | "merge" | "add_all" | "save" | "bulk_save_objects" if !c.alici.is_empty() => {
                        let tip = if c.ad == "save" || (c.ad == "delete" && c.argumanlar.is_empty()) {
                            // obj.save() / obj.delete() (Django)
                            self.ad_tipi(fi, &c.alici[0])
                        } else {
                            self.ad_tipi(fi, &ilk).or_else(|| ilk.split('(').next().filter(|x| !x.is_empty() && !x.contains(['.', ' ', '['])).map(String::from))
                        };
                        if let Some((m, k)) = tip.as_deref().and_then(|t| self.model_adi(t)) {
                            s.tablo(m, true, c.satir, k);
                        }
                    }
                    _ => {}
                }
                if c.alici.len() >= 2 && matches!(c.alici[1].as_str(), "objects" | "query") {
                    if let Some((m, k)) = self.model_adi(&c.alici[0]) {
                        s.tablo(m, yazma_islemi(&c.ad).unwrap_or(false), c.satir, k);
                    }
                }
            }
            Dil::Go if !c.alici.is_empty() => {
                // GORM: db.Create(&x) / db.Where(...).First(&x) / db.Model(&X{}): argümandaki modelin tipi
                let Some(y) = go_gorm(&c.ad) else { return };
                for a in c.argumanlar.iter().take(2) {
                    let a = a.trim().trim_start_matches('&').trim_start_matches("[]");
                    let tip = if let Some((t, _)) = a.split_once('{') {
                        Some(t.trim().trim_start_matches("[]").to_string())
                    } else if !a.is_empty() && a.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
                        self.ad_tipi(fi, a)
                    } else {
                        None
                    };
                    if let Some(t) = tip {
                        let taban = tip_parcala(&t).0;
                        if !self.sinif_coz(&taban, di).is_empty() {
                            let m = self.model_adi(&taban).map(|m| m.0).unwrap_or_else(|| self.tablo_adi(&taban));
                            s.tablo(m, y, c.satir, "gorm");
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Alıcının tipinden tablo etkisi: TypeORM `Repository<E>`, EF `DbSet<E>`, Spring Data deposu (`JpaRepository<E, ID>`
    /// türevi arayüz), model sınıfının kendisi (Eloquent / ActiveRecord / Django / Sequelize statik çağrısı ya da örneği)
    fn tipli_tablo(&self, tip: &str, tdi: usize, statik: bool, c: &HamCagri, s: &mut FonkSonucu) {
        let Some(y) = yazma_islemi(&c.ad) else { return };
        let (taban, args) = tip_parcala(tip);
        let model: Option<(String, &'static str)> = if matches!(taban.as_str(), "Repository" | "MongoRepository" | "TreeRepository" | "DbSet" | "IQueryable" | "ModelStatic") {
            args.first().map(|a| self.model_adi(a).unwrap_or_else(|| (self.tablo_adi(a), if taban == "DbSet" { "ef" } else { "typeorm" })))
        } else if let Some(m) = self.model_adi(&taban) {
            (statik || matches!(m.1, "eloquent" | "activerecord" | "django" | "sequelize" | "mongoose" | "jpa" | "gorm" | "sqlalchemy" | "exposed")).then_some(m)
        } else {
            let mut m = None;
            for rc in self.sinif_coz(&taban, tdi) {
                for u in &self.sinif(rc).ustler {
                    let (ut, ua) = tip_parcala(u);
                    if ut.ends_with("Repository") || ut.ends_with("Repo") {
                        if let Some(e) = ua.first() {
                            m = Some(self.model_adi(e).unwrap_or_else(|| (self.tablo_adi(e), "jpa")));
                        }
                    }
                }
            }
            m
        };
        if let Some((m, kaynak)) = model {
            s.tablo(m, y, c.satir, kaynak);
        }
    }
}

// ---- Dış etki için: çağrının girdiği dış modül ve polimorfik dağıtım ----

/// Polimorfik aday kümesinin sınırı: aynı adlı yöntem bundan çoksa hedef belirsizdir (dış etki için bilinmiyor)
const EN_FAZLA_ADAY: usize = 48;

/// Tip ya da ifade metninin kök adı: "=pycurl.Curl" -> "pycurl", "transports.Transport" -> "transports", "Mesaj" -> "Mesaj"
fn kok_adi(t: &str) -> String {
    let t = t.trim().trim_start_matches('=').trim_start_matches(['*', '&', '?', '\\', ':']).trim();
    let t = t.strip_prefix("new ").unwrap_or(t);
    t.split(['.', '(', '|', '<', '[', ' ', ':', '\\', '-', '>', ',']).next().unwrap_or("").to_string()
}

impl<'a> Kurulum<'a> {
    /// Alt sınıf dizini ve proje klasörlerinin adları
    fn dis_hazirla(&mut self) {
        let mut alt: HashMap<usize, Vec<usize>> = HashMap::new();
        for ci in 0..self.siniflar.len() {
            let di = self.siniflar[ci].0;
            for u in &self.sinif(ci).ustler {
                for uc in self.sinif_coz(u, di) {
                    let v = alt.entry(uc).or_default();
                    if uc != ci && !v.contains(&ci) {
                        v.push(ci);
                    }
                }
            }
        }
        let mut klasorler = HashSet::new();
        let mut paketler = HashSet::new();
        for (yol, oz) in self.dosyalar.iter() {
            match oz.dil {
                Some(Dil::Java | Dil::Kt) if !oz.paket.is_empty() => {
                    paketler.insert(oz.paket.split('.').take(2).collect::<Vec<_>>().join("."));
                }
                Some(Dil::Php) if !oz.paket.is_empty() => {
                    paketler.insert(oz.paket.trim_start_matches('\\').split('\\').next().unwrap_or("").to_string());
                }
                _ => {}
            }
            let parca: Vec<&str> = yol.split('/').collect();
            for p in &parca[..parca.len().saturating_sub(1)] {
                klasorler.insert(p.to_string());
            }
            // Python: kökteki tek dosyalık modül (`settings.py`)
            if oz.dil == Some(Dil::Py) {
                if let Some((g, _)) = parca.last().and_then(|s| s.rsplit_once('.')) {
                    klasorler.insert(g.to_string());
                }
            }
        }
        self.alt_siniflar = alt;
        self.klasorler = klasorler;
        self.paketler = paketler;
    }

    /// İçe aktarılan adın dış modülü: içe aktarma projenin dosyalarına çözülmüyorsa "modül[.asıl ad]"; projeye aitse None
    fn dis_iceaktarma(&self, di: usize, ad: &str) -> Option<String> {
        if ad.is_empty() {
            return None;
        }
        let oz = self.dosyalar[di].1;
        let ii = oz.iceaktarma.iter().position(|(y, _, _)| y == ad)?;
        let (modul, asil) = (&oz.iceaktarma[ii].1, oz.iceaktarma[ii].2.as_str());
        let (m, alt) = &self.ice[di][ii];
        if !m.is_empty() || !alt.is_empty() {
            return None;
        }
        let ilk = |ayrac: char| modul.split(ayrac).next().unwrap_or("").to_string();
        let proje = match self.dosya_dil(di) {
            Dil::Py => modul.starts_with('.') || self.klasorler.contains(&ilk('.')),
            Dil::Ts => modul.starts_with(['.', '#', '~']) || modul.starts_with("@/") || (!modul.starts_with('@') && self.klasorler.contains(&ilk('/'))),
            Dil::Java | Dil::Kt => self.paketler.contains(&modul.split('.').take(2).collect::<Vec<_>>().join(".")),
            Dil::Php => {
                let m = modul.trim_start_matches('\\');
                m.starts_with("App\\") || self.paketler.contains(m.split('\\').next().unwrap_or(""))
            }
            // C# `using` bir ad alanıdır, tek bir ada bağlanmaz
            Dil::Cs => true,
            _ => false,
        };
        if proje {
            return None;
        }
        let tam = if matches!(asil, "*" | "default" | "") || modul.ends_with(asil) { modul.clone() } else { format!("{modul}.{asil}") };
        // PHP: ad alanı olmayan çekirdek sınıfı ayırt edilebilsin diye dil öneki (`use function` / `use const` atılır)
        Some(if self.dosya_dil(di) == Dil::Php { format!("php:{}", tam.trim_start_matches("function ").trim_start_matches("const ").trim()) } else { tam })
    }

    /// Sınıfın (üst sınıflar boyunca) ilk kütüphane tabanı: projede tanımı olmayan üst tip içe aktarılmışsa modülü
    fn kutuphane_tabani(&self, ci: usize) -> Option<String> {
        let mut gorulen = HashSet::new();
        let mut yigin = vec![(ci, 0usize)];
        while let Some((c, d)) = yigin.pop() {
            if !gorulen.insert(c) || d > 6 {
                continue;
            }
            let di = self.siniflar[c].0;
            for u in &self.sinif(c).ustler {
                let ust = self.sinif_coz(u, di);
                if ust.is_empty() {
                    if let Some(m) = self.dis_iceaktarma(di, &kok_adi(u)) {
                        return Some(m);
                    }
                }
                yigin.extend(ust.into_iter().map(|x| (x, d + 1)));
            }
        }
        None
    }

    /// Sınıfın alt sınıflarındaki (en çok 256 sınıf) aynı adlı yöntemler
    fn alt_yontemler(&self, c0: usize, ad: &str) -> Vec<usize> {
        let mut v = vec![];
        let mut gorulen = HashSet::from([c0]);
        let mut kuyruk = vec![c0];
        while let Some(c) = kuyruk.pop() {
            for &a in self.alt_siniflar.get(&c).into_iter().flatten() {
                if gorulen.len() > 256 || !gorulen.insert(a) {
                    continue;
                }
                if let Some(m) = self.metotlar.get(&(a, ad)) {
                    v.extend(m.iter().copied());
                }
                kuyruk.push(a);
            }
        }
        v
    }

    /// Parametrenin ya da yerel değişkenin ham tipi (atanan ifadenin başı dahil): "pycurl.Curl", "Kanal"
    fn ham_tip(&self, fi: usize, ad: &str) -> Option<String> {
        let f = self.fonk(fi);
        let t = f
            .parametreler
            .iter()
            .rev()
            .find(|(a, t)| a == ad && !t.is_empty())
            .or_else(|| f.yereller.iter().rev().find(|(a, t)| a == ad && t.len() > 1))
            .map(|p| p.1.trim_start_matches('=').to_string())?;
        Some(t.rsplit('|').next().unwrap_or(&t).trim().to_string())
    }

    /// Tipi bilinen alıcıda çözülemeyen yöntem: projedeki sınıfsa alt sınıflarındaki yeniden tanımlar ya da kütüphane
    /// tabanı; projede değilse tipin içe aktarıldığı modül
    fn tipli_dis(&self, tip: &str, di: usize, ad: &str, yer: &mut CagriYeri) {
        let siniflar = self.sinif_coz(tip, di);
        match siniflar.as_slice() {
            [c] => {
                let v = self.alt_yontemler(*c, ad);
                if v.is_empty() {
                    yer.modul = self.kutuphane_tabani(*c);
                } else {
                    yer.adaylar = v;
                }
            }
            [] => yer.modul = self.dis_iceaktarma(di, &kok_adi(tip)),
            _ => {}
        }
    }

    /// Tipi bilinmeyen alıcı: projedeki aynı adlı yöntemler aday kümesidir (sınırı aşarsa belirsiz)
    fn ada_gore_adaylar(&self, di: usize, ad: &str, yer: &mut CagriYeri) {
        if yaygin_ad(ad) {
            return;
        }
        let dil = self.dosya_dil(di);
        let v: Vec<usize> =
            self.ad_dizini.get(ad).into_iter().flatten().copied().filter(|f| self.fonk(*f).sinif.is_some() && self.dosya_dil(self.fonk_yeri[*f].0) == dil).collect();
        if v.len() > EN_FAZLA_ADAY {
            yer.belirsiz = true;
        } else {
            yer.adaylar = v;
        }
    }

    /// Çağrı yerinin dış etki bilgisi: çözülen yöntemin alt sınıflardaki yeniden tanımları; çözülemeyen çağrının girdiği
    /// dış modül ya da polimorfik aday kümesi
    fn dis_bilgisi(&self, fi: usize, c: &HamCagri, hedefler: Option<&[usize]>, yer: &mut CagriYeri) {
        if c.dinamik {
            return;
        }
        let (di, _) = self.fonk_yeri[fi];
        if let Some(h) = hedefler {
            let mut v = vec![];
            for &x in h {
                let (xd, xi) = self.fonk_yeri[x];
                let xf = &self.dosyalar[xd].1.fonksiyonlar[xi];
                if let Some(s) = xf.sinif {
                    v.extend(self.alt_yontemler(self.sinif_sira[&(xd, s)], &xf.ad));
                }
            }
            v.retain(|x| !h.contains(x));
            v.sort_unstable();
            v.dedup();
            if v.len() > EN_FAZLA_ADAY {
                yer.belirsiz = true;
            } else {
                yer.adaylar = v;
            }
            return;
        }
        let ci = self.fonk(fi).sinif.map(|s| self.sinif_sira[&(di, s)]);
        let Some(ilk) = c.alici.first() else {
            yer.modul = self.dis_iceaktarma(di, &c.ad);
            return;
        };
        if ilk.starts_with('<') {
            return;
        }
        if ilk == "this" {
            let Some(ci) = ci else { return };
            match c.alici.get(1) {
                // Kendi sınıfında bulunmayan yöntem: alt sınıflarda (şablon yöntem) ya da kütüphane tabanında
                None => {
                    let v = self.alt_yontemler(ci, &c.ad);
                    if v.is_empty() {
                        yer.modul = self.kutuphane_tabani(ci);
                    } else {
                        yer.adaylar = v;
                    }
                }
                Some(a) => match self.alan_tipi(ci, a.trim_start_matches(['$', '@'])) {
                    Some((t, d)) if c.alici.len() == 2 => {
                        let t = t.trim_start_matches('=').rsplit('|').next().unwrap_or("").to_string();
                        self.tipli_dis(&t, d, &c.ad, yer);
                    }
                    Some(_) => {}
                    None => self.ada_gore_adaylar(di, &c.ad, yer),
                },
            }
            return;
        }
        if let Some(m) = self.dis_iceaktarma(di, ilk) {
            yer.modul = Some(m);
            return;
        }
        if let Some(t) = self.ham_tip(fi, ilk) {
            if c.alici.len() == 1 {
                self.tipli_dis(&t, di, &c.ad, yer);
            }
            return;
        }
        if let [c0] = self.sinif_coz(ilk, di).as_slice() {
            if c.alici.len() == 1 {
                yer.modul = self.kutuphane_tabani(*c0);
            }
            return;
        }
        // Ruby: projede tanımı olmayan sabit (gem sınıfı / modülü, `Stripe::Charge`)
        if self.dosya_dil(di) == Dil::Rb && ilk.starts_with(|ch: char| ch.is_ascii_uppercase()) {
            let yol: Vec<String> = c.alici.iter().take_while(|x| x.starts_with(|ch: char| ch.is_ascii_uppercase())).cloned().collect();
            yer.modul = Some(format!("rb:{}", yol.join("::")));
            return;
        }
        // Projeden içe aktarılan modül ya da sınıf (çözülemeyen yöntemi): dış değildir
        if self.dosyalar[di].1.iceaktarma.iter().any(|(y, _, _)| y == ilk) {
            return;
        }
        self.ada_gore_adaylar(di, &c.ad, yer);
    }
}

impl Grafik {
    /// Dosya özetlerinden grafiği kurar. `dosyalar`: (uygulama köküne göre yol, özet); `ek_modeller`: adaptörün bulduğu
    /// tablo modelleri ({ad, tablo}); `prisma`: (şema dosyası, metni): Prisma şemaları ve MyBatis XML eşleyicileri (`.xml`)
    pub fn kur(dosyalar: &[(String, &DosyaOzeti)], ek_modeller: &[Value], prisma: &[(String, String)]) -> Grafik {
        let mut k = Kurulum {
            dosyalar,
            siniflar: vec![],
            sinif_ad: HashMap::new(),
            go_grup: HashMap::new(),
            metotlar: HashMap::new(),
            ust_fonk: HashMap::new(),
            ad_dizini: HashMap::new(),
            fonk_yeri: vec![],
            fonk_sira: HashMap::new(),
            sinif_sira: HashMap::new(),
            govde_yol: HashMap::new(),
            uygulayan: HashMap::new(),
            sabitler: HashMap::new(),
            modeller: HashMap::new(),
            tablo_modeli: HashMap::new(),
            prisma: HashMap::new(),
            model_degiskeni: HashMap::new(),
            ice: vec![],
            di: HashMap::new(),
            sonek: HashMap::new(),
            mybatis: HashMap::new(),
            alt_siniflar: HashMap::new(),
            klasorler: HashSet::new(),
            paketler: HashSet::new(),
        };
        for (di, (yol, oz)) in dosyalar.iter().enumerate() {
            let govde = yol.rsplit_once('.').map(|x| x.0).unwrap_or(yol);
            k.govde_yol.insert(govde.to_string(), di);
            let parca: Vec<&str> = govde.split('/').collect();
            for i in 0..parca.len() {
                k.sonek.entry(parca[i..].join("/")).or_default().push(di);
            }
            for (a, c) in &oz.di_kayitlari {
                let v = k.di.entry(a.clone()).or_default();
                if !v.contains(c) {
                    v.push(c.clone());
                }
            }
            for (si, s) in oz.siniflar.iter().enumerate() {
                let ci = k.siniflar.len();
                k.siniflar.push((di, si));
                k.sinif_sira.insert((di, si), ci);
                if !s.ad.is_empty() {
                    k.sinif_ad.entry(s.ad.as_str()).or_default().push(ci);
                }
            }
            for (a, v) in &oz.sabitler {
                k.sabitler.entry(a.as_str()).or_insert(v.as_str());
            }
        }
        k.ice = (0..dosyalar.len())
            .into_par_iter()
            .map(|di| {
                let dil = k.dosya_dil(di);
                dosyalar[di]
                    .1
                    .iceaktarma
                    .iter()
                    .map(|(_, modul, asil)| {
                        let alt = if dil == Dil::Py && asil != "*" { k.modul_dosyalari(di, &format!("{modul}.{asil}")) } else { vec![] };
                        (k.modul_dosyalari(di, modul), alt)
                    })
                    .collect()
            })
            .collect();
        // Go: aynı klasördeki eş adlı tipler tek sınıf
        let mut go: HashMap<(String, &str), Vec<usize>> = HashMap::new();
        for (ci, (di, _)) in k.siniflar.iter().enumerate() {
            if k.dosya_dil(*di) == Dil::Go {
                go.entry((k.klasor(*di).to_string(), k.sinif(ci).ad.as_str())).or_default().push(ci);
            }
        }
        for g in go.into_values().filter(|g| g.len() > 1) {
            for c in &g {
                k.go_grup.insert(*c, g.clone());
            }
        }
        // Arayüz -> uygulayanlar
        for ci in 0..k.siniflar.len() {
            let s = k.sinif(ci);
            if s.arayuz {
                continue;
            }
            for u in &s.ustler {
                let v = k.uygulayan.entry(tip_parcala(u).0).or_default();
                if !v.contains(&ci) {
                    v.push(ci);
                }
            }
        }
        k.dis_hazirla();
        // Fonksiyonlar ve kimlikleri
        let mut fonksiyonlar: Vec<Fonksiyon> = vec![];
        let mut kimlikler: HashSet<String> = HashSet::new();
        for (di, (yol, oz)) in dosyalar.iter().enumerate() {
            for (i, f) in oz.fonksiyonlar.iter().enumerate() {
                let fi = fonksiyonlar.len();
                k.fonk_yeri.push((di, i));
                k.fonk_sira.insert((di, i), fi);
                let sinif = f.sinif.map(|s| oz.siniflar[s].ad.clone()).filter(|s| !s.is_empty());
                let mut id = match &sinif {
                    Some(s) => format!("fn:{yol}#{s}.{}", f.ad),
                    None => format!("fn:{yol}#{}", f.ad),
                };
                if !kimlikler.insert(id.clone()) {
                    id = format!("{id}@{}", f.satir);
                    kimlikler.insert(id.clone());
                }
                fonksiyonlar.push(Fonksiyon { id, dosya: yol.clone(), sinif, ad: f.ad.clone(), satir: f.satir, son: f.son, anonim: f.anonim });
                match f.sinif {
                    Some(s) => k.metotlar.entry((k.sinif_sira[&(di, s)], f.ad.as_str())).or_default().push(fi),
                    None => k.ust_fonk.entry((di, f.ad.as_str())).or_default().push(fi),
                }
                if !f.anonim {
                    k.ad_dizini.entry(f.ad.as_str()).or_default().push(fi);
                }
            }
        }
        // ORM modelleri: sınıflardan (dekoratör / anotasyon / üst sınıf), değişkene atanan modellerden, Prisma
        // şemasından ve adaptörün bulduklarından. Tablo düğümünün adı model adıdır; Kysely tablo sınıfında (sorgular tablo
        // adıyla yazılır) tablo adıdır
        let mut modeller_json: Vec<Value> = vec![];
        for ci in 0..k.siniflar.len() {
            let (di, _) = k.siniflar[ci];
            let s = k.sinif(ci);
            let Some(kaynak) = orm_modeli(k.dosya_dil(di), s) else { continue };
            let (tablo, tkaynak) = match (&s.tablo, &s.go_tablo) {
                (Some((t, x)), _) => (t.clone(), *x),
                (None, Some(t)) => (t.clone(), "TableName()"),
                (None, None) => (varsayilan_tablo(kaynak, &s.ad), "varsayılan (ORM kuralı)"),
            };
            let ad = if kaynak == "kysely" { tablo.clone() } else { s.ad.clone() };
            k.modeller.insert(s.ad.to_lowercase(), (ad.clone(), kaynak));
            k.tablo_modeli.entry(tablo.to_lowercase()).or_insert_with(|| ad.clone());
            k.tablo_modeli.entry(ad.to_lowercase()).or_insert_with(|| ad.clone());
            let alanlar: Vec<Value> = s.alanlar.iter().take(64).map(|(a, t)| json!({ "ad": a, "tip": tip_parcala(t).0 })).collect();
            modeller_json.push(json!({ "ad": ad, "tablo": tablo, "tablo_kaynagi": tkaynak, "dosya": dosyalar[di].0, "satir": s.satir, "alanlar": alanlar, "orm": kaynak }));
        }
        for (di, (_, oz)) in dosyalar.iter().enumerate() {
            for (degisken, tablo, kaynak) in &oz.model_atamalari {
                k.model_degiskeni.insert((di, degisken.as_str()), tablo.clone());
                k.modeller.entry(degisken.to_lowercase()).or_insert_with(|| (tablo.clone(), kaynak));
                k.tablo_modeli.entry(tablo.to_lowercase()).or_insert_with(|| tablo.clone());
            }
        }
        for (dosya, m) in prisma.iter().filter(|(d, _)| d.ends_with(".xml")) {
            let _ = dosya;
            for (arayuz, id, tablolar) in mybatis_eslemeleri(m) {
                k.mybatis.entry((arayuz, id)).or_default().extend(tablolar);
            }
        }
        for (dosya, m) in prisma.iter().filter(|(d, _)| !d.ends_with(".xml")) {
            for v in crate::kanit::prisma_modelleri(m, dosya) {
                let (Some(ad), Some(t)) = (v["ad"].as_str(), v["tablo"].as_str()) else { continue };
                let mut erisimci = ad.to_string();
                if let Some(c) = erisimci.get_mut(0..1) {
                    c.make_ascii_lowercase();
                }
                k.prisma.insert(erisimci, ad.to_string());
                k.tablo_modeli.entry(t.to_lowercase()).or_insert_with(|| ad.to_string());
            }
        }
        for m in ek_modeller {
            let Some(ad) = m["ad"].as_str() else { continue };
            let t = m["tablo"].as_str().unwrap_or(ad);
            k.tablo_modeli.insert(t.to_lowercase(), ad.to_string());
            k.tablo_modeli.insert(ad.to_lowercase(), ad.to_string());
            match k.modeller.get_mut(&ad.to_lowercase()) {
                Some((a, _)) => *a = ad.to_string(),
                None => {
                    k.modeller.insert(ad.to_lowercase(), (ad.to_string(), "adaptor"));
                }
            }
        }
        // Kenarlar, tablo etkileri, erişim denetimleri (fonksiyon başına paralel)
        let n = fonksiyonlar.len();
        let sonuclar: Vec<FonkSonucu> = (0..n).into_par_iter().map(|fi| k.fonksiyon_coz(fi)).collect();
        let mut g = Grafik {
            fonksiyonlar,
            kenarlar: vec![],
            giden: vec![vec![]; n],
            tablolar: Vec::with_capacity(n),
            erisim: Vec::with_capacity(n),
            dinamik: Vec::with_capacity(n),
            cagri_yerleri: Vec::with_capacity(n),
            denetim: Vec::with_capacity(n),
            modeller: modeller_json,
            dosya_sayisi: dosyalar.len(),
            iceaktarilan: HashMap::new(),
            yontem_dallari: dosyalar.iter().filter(|(_, o)| !o.yontem_dallari.is_empty()).map(|(y, o)| (y.clone(), o.yontem_dallari.clone())).collect(),
        };
        for (di, (yol, oz)) in dosyalar.iter().enumerate() {
            for (ii, (yerel, _, asil)) in oz.iceaktarma.iter().enumerate() {
                let asil = if matches!(asil.as_str(), "default" | "*" | "") { yerel.as_str() } else { asil.as_str() };
                let v: Vec<usize> = k.ice[di][ii].0.iter().filter_map(|d| k.ust_fonk.get(&(*d, asil))).flatten().copied().collect();
                if !v.is_empty() {
                    g.iceaktarilan.insert((yol.clone(), yerel.clone()), v);
                }
            }
        }
        for (fi, s) in sonuclar.into_iter().enumerate() {
            for (hedef, satir, guven, cozum) in s.kenarlar {
                if hedef == fi || g.giden[fi].iter().any(|&e| g.kenarlar[e].hedef == hedef) {
                    continue;
                }
                g.giden[fi].push(g.kenarlar.len());
                g.kenarlar.push(Kenar { kaynak: fi, hedef, satir, guven, cozum });
            }
            g.tablolar.push(s.tablolar);
            g.erisim.push(s.erisim);
            g.dinamik.push(s.dinamik);
            g.cagri_yerleri.push(s.yerler);
            g.denetim.push(!g.fonksiyonlar[fi].anonim && erisim_cagrisi(&[], &g.fonksiyonlar[fi].ad));
        }
        g
    }
}

// ---- İşlem analizi ----

/// İşlemin işleyicisinden ulaşılan bir fonksiyon
#[derive(Clone, Debug)]
pub struct Ulasilan {
    pub fonk: usize,
    pub derinlik: usize,
    /// Yol boyunca en zayıf kenar
    pub guven: Guven,
    /// Geldiği kenar (işleyicide None)
    pub kenar: Option<usize>,
}

/// İşlemin derin analizi: ulaşılan fonksiyonlar, tablo etkileri, erişim denetimleri. Tablo ve erişim öğelerinin ikinci
/// alanı `ulasilan` içindeki sıradır
pub struct IslemAnalizi {
    pub ulasilan: Vec<Ulasilan>,
    pub kesildi: bool,
    pub dinamik: usize,
    pub tablolar: Vec<(TabloEtkisi, usize)>,
    pub erisim: Vec<(ErisimDenetimi, usize)>,
}

impl Grafik {
    /// İşleyici fonksiyonu: işlemin dosyasında adı eşleşen (büyük/küçük harf ve alt çizgi duyarsız) fonksiyon; birden
    /// çoksa tanımı işlemin satırına en yakın olan; dosyaya bu adla içe aktarılan fonksiyon; işlemin satırını kapsayan en
    /// içteki fonksiyon; o da yoksa kayıt satırından hemen sonra başlayan adsız fonksiyon
    pub fn isleyici_bul(&self, dosya: &str, fonksiyon: &str, satir: usize) -> Option<usize> {
        let ad = fonksiyon.rsplit(['.', '#', ':']).next().unwrap_or(fonksiyon);
        let ayni: Vec<usize> = (0..self.fonksiyonlar.len()).filter(|i| self.fonksiyonlar[*i].dosya == dosya).collect();
        let adli = ayni.iter().copied().filter(|i| !ad.is_empty() && ad_esit(&self.fonksiyonlar[*i].ad, ad));
        if let Some(i) = adli.min_by_key(|i| (self.fonksiyonlar[*i].satir as i64 - satir as i64).unsigned_abs()) {
            return Some(i);
        }
        // Rota dosyasına içe aktarılan işleyici: asıl tanımı
        if let Some([tek]) = self.iceaktarilan.get(&(dosya.to_string(), ad.to_string())).map(Vec::as_slice) {
            return Some(*tek);
        }
        let kapsayan = ayni
            .iter()
            .copied()
            .filter(|i| self.fonksiyonlar[*i].satir <= satir && self.fonksiyonlar[*i].son >= satir)
            .min_by_key(|i| self.fonksiyonlar[*i].son - self.fonksiyonlar[*i].satir);
        // Rota kaydına verilen adsız işleyici (`router.get('/x', auth, async (req, res) => {...})`): kayıt satırından sonra
        // başlayan ilk adsız fonksiyon
        kapsayan.or_else(|| {
            ayni.into_iter()
                .filter(|i| self.fonksiyonlar[*i].anonim && self.fonksiyonlar[*i].satir >= satir && self.fonksiyonlar[*i].satir <= satir + 12)
                .min_by_key(|i| self.fonksiyonlar[*i].satir)
        })
    }

    /// İşleyiciden genişlik öncelikli iniş (en çok `EN_DERIN` kat, `EN_FAZLA_FONKSIYON` fonksiyon). Erişim denetimi
    /// fonksiyonunun içine inilmez (uygulaması denetimin parçasıdır). Birden çok yöntemi tek fonksiyonda karşılayan
    /// işleyicide (`if request.method == "POST":`) başka yöntemin dalındaki çağrı, tablo ve erişim öğeleri sayılmaz
    /// `yontem`: işlemin istek yöntemi (boşsa bütün dallar sayılır)
    pub fn analiz_yontemle(&self, isleyici: usize, yontem: &str) -> IslemAnalizi {
        let fi = &self.fonksiyonlar[isleyici];
        let haric = if yontem.is_empty() { vec![] } else { self.yontem_dallari.get(&fi.dosya).map(|d| yontem_disi(d, fi.satir, fi.son, yontem)).unwrap_or_default() };
        let disarida = |f: usize, satir: usize| f == isleyici && haric.iter().any(|(a, b)| satir >= *a && satir <= *b);
        let mut ulasilan: Vec<Ulasilan> = vec![Ulasilan { fonk: isleyici, derinlik: 0, guven: Guven::Certain, kenar: None }];
        let mut gorulen: HashSet<usize> = HashSet::from([isleyici]);
        let mut kuyruk: VecDeque<usize> = VecDeque::from([0]);
        let mut kesildi = false;
        while let Some(u) = kuyruk.pop_front() {
            let (f, d, g) = (ulasilan[u].fonk, ulasilan[u].derinlik, ulasilan[u].guven);
            if self.denetim[f] && d > 0 {
                continue;
            }
            for &e in &self.giden[f] {
                let k = &self.kenarlar[e];
                if gorulen.contains(&k.hedef) || disarida(f, k.satir) {
                    continue;
                }
                if d + 1 > EN_DERIN || ulasilan.len() >= EN_FAZLA_FONKSIYON {
                    kesildi = true;
                    continue;
                }
                gorulen.insert(k.hedef);
                kuyruk.push_back(ulasilan.len());
                ulasilan.push(Ulasilan { fonk: k.hedef, derinlik: d + 1, guven: g.min(k.guven), kenar: Some(e) });
            }
        }
        let mut tablolar: Vec<(TabloEtkisi, usize)> = vec![];
        let mut erisim: Vec<(ErisimDenetimi, usize)> = vec![];
        let mut dinamik = 0;
        for (i, u) in ulasilan.iter().enumerate() {
            // Erişim denetimi fonksiyonunun kendi gövdesi (izin sorgusu, kayıt okuması) denetimin uygulamasıdır: sayılmaz
            if self.denetim[u.fonk] && u.derinlik > 0 {
                continue;
            }
            dinamik += self.dinamik[u.fonk];
            for t in self.tablolar[u.fonk].iter().filter(|t| !disarida(u.fonk, t.satir)) {
                match tablolar.iter_mut().find(|(x, _)| x.tablo == t.tablo) {
                    Some((x, j)) => {
                        if t.yazma && !x.yazma {
                            *x = t.clone();
                            *j = i;
                        }
                    }
                    None => tablolar.push((t.clone(), i)),
                }
            }
            for e in self.erisim[u.fonk].iter().filter(|e| !disarida(u.fonk, e.satir)) {
                if erisim.len() < 24 {
                    erisim.push((e.clone(), i));
                }
            }
        }
        IslemAnalizi { ulasilan, kesildi, dinamik, tablolar, erisim }
    }

    /// Ulaşılan fonksiyona işleyiciden giden yol (fonksiyon sıraları)
    pub fn yol(&self, a: &IslemAnalizi, mut i: usize) -> Vec<usize> {
        let konum: HashMap<usize, usize> = a.ulasilan.iter().enumerate().map(|(j, u)| (u.fonk, j)).collect();
        let mut v = vec![a.ulasilan[i].fonk];
        for _ in 0..=EN_DERIN {
            let Some(e) = a.ulasilan[i].kenar else { break };
            let k = self.kenarlar[e].kaynak;
            v.push(k);
            match konum.get(&k) {
                Some(j) => i = *j,
                None => break,
            }
        }
        v.reverse();
        v
    }

    /// Kenarın kanıt konumu: çağıran fonksiyonun dosyası ve çağrının satırı
    pub fn kenar_konumu(&self, e: usize) -> String {
        let k = &self.kenarlar[e];
        format!("{}:{}", self.fonksiyonlar[k.kaynak].dosya, k.satir)
    }

    /// Bütün grafiğin sayıları
    pub fn sayilar(&self) -> Value {
        let kesin = self.kenarlar.iter().filter(|k| k.guven == Guven::Certain).count();
        let mut cozum: BTreeMap<&str, usize> = BTreeMap::new();
        for k in &self.kenarlar {
            *cozum.entry(k.cozum).or_default() += 1;
        }
        json!({
            "dosya": self.dosya_sayisi,
            "fonksiyon": self.fonksiyonlar.len(),
            "kenar": self.kenarlar.len(),
            "kesin": kesin,
            "yorum": self.kenarlar.len() - kesin,
            "dinamik": self.dinamik.iter().sum::<usize>(),
            "cozum": cozum,
            "model": self.modeller.len(),
        })
    }
}

/// Derin analizin haritaya uygulanması: her işlem için işleyici bulunur ve çağrı grafiği boyunca inilir. Sonuç:
/// - işlemin davranış özetine (`bilgi.dna["dosya:satır"]`) çağrı zinciri özeti (`cagri`), tablo etkileri
///   (`tablo_etkisi`), erişim denetimleri (`erisim_denetimi`) ve zincirde bulunan sahiplik karşılaştırmaları
///   (`sahiplik_denetimi`);
/// - kapının veri bağına (`veri.okur` / `veri.yazar`) çağrı zinciriyle bulunan tablolar;
/// - `bilgi.cagri`: grafiğin sayıları ve işlemlerden ulaşılan alt grafiğin kenarları (kanıt deposu bunlardan `CALLS`
///   önermeleri yazar);
/// - `bilgi.veri_modelleri`: bulunan ORM modelleri (adaptörünkilerle aynı adda olan eklenmez).
pub fn uygula(g: &Grafik, kapilar: &mut [crate::adaptor::Kapi], bilgi: &mut Value) {
    let mut kenarlar: BTreeSet<usize> = BTreeSet::new();
    let mut dugum: BTreeSet<usize> = BTreeSet::new();
    // İşleyici fonksiyonların kimliği adaptörün verdiği kimliktir (`fn:dosya#fonksiyon`): uç -> işleyici kenarıyla birleşir
    let mut kimlik: HashMap<usize, String> = HashMap::new();
    let mut islenen = 0usize;
    if !bilgi["dna"].is_object() {
        bilgi["dna"] = json!({});
    }
    // Birden çok yöntemi tek fonksiyonda karşılayan işleyici (`if request.method == "POST": ...`): aynı satırı paylaşan
    // işlemlerin davranış özeti birleşirdi; yöntemi kesin eşleşen bir dalı olan işlem o dalın koşul satırına taşınır, her
    // işlem kendi özetini taşır (dala düşmeyen yöntem fonksiyonun satırında kalır)
    let mut ayni_yer: HashMap<(String, usize), usize> = HashMap::new();
    for k in kapilar.iter() {
        *ayni_yer.entry((k.dosya.clone(), k.satir)).or_default() += 1;
    }
    for k in kapilar.iter_mut() {
        if ayni_yer.get(&(k.dosya.clone(), k.satir)).copied().unwrap_or(0) < 2 {
            continue;
        }
        let Some(h) = g.isleyici_bul(&k.dosya, &k.fonksiyon, k.satir) else { continue };
        let fi = &g.fonksiyonlar[h];
        let Some(dal) = g
            .yontem_dallari
            .get(&fi.dosya)
            .and_then(|d| d.iter().find(|d| d.kosul > fi.satir && d.son <= fi.son && !d.degilse && d.pozitif && d.saf && d.yontemler.contains(&k.yontem.to_ascii_uppercase())))
        else {
            continue;
        };
        if fi.dosya != k.dosya {
            continue;
        }
        let eski = format!("{}:{}", k.dosya, k.satir);
        let yeni = format!("{}:{}", k.dosya, dal.kosul);
        if let Some(v) = bilgi["dna"].get(&eski).cloned() {
            bilgi["dna"][yeni.as_str()] = v;
        }
        k.satir = dal.kosul;
    }
    for k in kapilar.iter_mut() {
        let Some(h) = g.isleyici_bul(&k.dosya, &k.fonksiyon, k.satir) else { continue };
        islenen += 1;
        kimlik.entry(h).or_insert_with(|| format!("fn:{}#{}", k.dosya, k.fonksiyon));
        let a = g.analiz_yontemle(h, &k.yontem);
        for u in &a.ulasilan {
            dugum.insert(u.fonk);
            if let Some(e) = u.kenar {
                kenarlar.insert(e);
            }
        }
        // Adsız işleyici (rota kaydına verilen fonksiyon) işlemin fonksiyon adıyla gösterilir
        let gorunen = |f: usize| if f == h && g.fonksiyonlar[f].anonim { k.fonksiyon.clone() } else { g.fonksiyonlar[f].gorunen() };
        let ad = |i: usize| gorunen(a.ulasilan[i].fonk);
        let yol_adlari = |i: usize| -> Vec<String> { g.yol(&a, i).into_iter().map(gorunen).collect() };
        let konum = |i: usize, satir: usize| format!("{}:{satir}", g.fonksiyonlar[a.ulasilan[i].fonk].dosya);
        let yer = format!("{}:{}", k.dosya, k.satir);
        let mut v = bilgi["dna"].get(&yer).cloned().unwrap_or_else(|| json!({ "hatalar": [], "sahiplik_denetimi": [], "dallar": [], "yumusak_silme": null, "kapsam": "acik" }));
        // Yöntem dallı işleyicide adaptörün redleri yalnızca bu yöntemin ulaştığı fonksiyonlardan (ve işleyicinin bu yönteme
        // ait satırlarından) gelir: başka yöntemin dalından inilen servisin reddi bu işlemin ön koşulu değildir
        let fh = &g.fonksiyonlar[h];
        let haric = g.yontem_dallari.get(&fh.dosya).map(|d| yontem_disi(d, fh.satir, fh.son, &k.yontem)).unwrap_or_default();
        if !haric.is_empty() {
            let ulasir = |kanit: &str| {
                let Some((d, s)) = kanit.rsplit_once(':') else { return true };
                let Ok(s) = s.parse::<usize>() else { return true };
                if d == fh.dosya && s >= fh.satir && s <= fh.son {
                    return !haric.iter().any(|(a, b)| s >= *a && s <= *b);
                }
                a.ulasilan.iter().any(|u| {
                    let f = &g.fonksiyonlar[u.fonk];
                    f.dosya == d && s >= f.satir && s <= f.son
                })
            };
            if let Some(l) = v["hatalar"].as_array_mut() {
                l.retain(|x| x["kanit"].as_str().is_none_or(ulasir));
            }
        }
        // Zincir özeti: yazılan bir tabloya giden yol, yoksa okunan bir tabloya, yoksa en derin ulaşılan fonksiyona
        let hedef_i = a
            .tablolar
            .iter()
            .find(|(t, _)| t.yazma)
            .or_else(|| a.tablolar.first())
            .map(|(_, i)| *i)
            .unwrap_or_else(|| (0..a.ulasilan.len()).max_by_key(|i| (a.ulasilan[*i].derinlik, std::cmp::Reverse(*i))).unwrap_or(0));
        v["cagri"] = json!({
            "isleyici": kimlik[&h],
            "erisilen": a.ulasilan.len(),
            "kesin_erisilen": a.ulasilan.iter().filter(|u| u.guven == Guven::Certain).count(),
            "derinlik": a.ulasilan.iter().map(|u| u.derinlik).max().unwrap_or(0),
            "dinamik": a.dinamik,
            "kesildi": a.kesildi,
            "zincir": yol_adlari(hedef_i),
            "zincir_guveni": a.ulasilan[hedef_i].guven.ad(),
        });
        // Adaptörün yazıldığını bildiği tabloya zincirdeki okuma eklenmez (ör. `Where(...).ExecuteDeleteAsync()` silmedir)
        let adaptor_yazar: Vec<String> = k.veri.as_ref().map(|v| v.yazar.clone()).unwrap_or_default();
        let etkiler: Vec<&(TabloEtkisi, usize)> = a.tablolar.iter().filter(|(t, _)| t.yazma || !adaptor_yazar.iter().any(|y| y.eq_ignore_ascii_case(&t.tablo))).collect();
        let tablolar: Vec<Value> = etkiler
            .iter()
            .take(24)
            .map(|(t, i)| {
                json!({ "tablo": t.tablo, "islem": if t.yazma { "yazar" } else { "okur" }, "kaynak": t.kaynak, "kanit": konum(*i, t.satir),
                    "guven": a.ulasilan[*i].guven.min(Guven::Inferred).ad(), "derinlik": a.ulasilan[*i].derinlik, "zincir": yol_adlari(*i) })
            })
            .collect();
        if !tablolar.is_empty() {
            v["tablo_etkisi"] = json!(tablolar);
        }
        let mut erisim: Vec<Value> = vec![];
        // Adaptörün sahiplik listesi (fırlatmanın koşulu, aynı kuralla: `dna::sahiplik_mi`) korunur; zincirden bulunanlar eklenir
        let mut sahiplik: Vec<Value> = v["sahiplik_denetimi"].as_array().cloned().unwrap_or_default();
        for (e, i) in &a.erisim {
            // İşleyicinin kendisindeki desen certain; servis zincirindeki inferred (yolun en zayıf halkasıyla)
            let guven = if a.ulasilan[*i].derinlik == 0 && e.kesin { Guven::Certain } else { a.ulasilan[*i].guven.min(Guven::Inferred) };
            let kanit = konum(*i, e.satir);
            match e.tur {
                // İstek kimliği yalnızca adından tanınan karşılaştırmada ad işlemin yol parametresiyse (`DELETE /users/{user_id}`
                // içinde `Item.owner_id == user_id`) karşılaştırılan, istekte seçilen kaydın kimliğidir; çağıranın değil:
                // sahiplik denetimi değildir (toplu silmenin / sorgunun koşulu)
                "kosul" | "filtre" if !e.kesin && yol_parametresiyle(&e.ifade, &k.yol) => {}
                "kosul" | "filtre" => {
                    // Adaptörün aynı koşulu (fırlatmanın satırıyla) zaten kaydettiyse tekrar yazılmaz
                    let ayni = |x: &Value| {
                        x["kanit"] == kanit.as_str()
                            || x["kanit"].as_str().and_then(|k2| k2.rsplit_once(':')).is_some_and(|(d2, s2)| {
                                kanit.rsplit_once(':').is_some_and(|(d1, s1)| d1 == d2 && s1.parse::<i64>().unwrap_or(0).abs_diff(s2.parse::<i64>().unwrap_or(0)) <= 3)
                                    && x["kosul"].as_str().is_some_and(|k| k.contains(e.ifade.as_str()))
                            })
                    };
                    if !sahiplik.iter().any(ayni) {
                        sahiplik.push(json!({ "kosul": e.ifade, "durum": null, "kanit": kanit, "guven": guven.ad(), "tur": e.tur, "fonksiyon": ad(*i) }));
                    }
                }
                _ => {
                    if !erisim.iter().any(|x| x["kanit"] == kanit.as_str()) {
                        erisim.push(json!({ "ifade": e.ifade, "izin": e.izin, "nesne_duzeyi": e.nesne, "tur": e.tur, "kanit": kanit, "guven": guven.ad(), "fonksiyon": ad(*i), "zincir": yol_adlari(*i) }));
                    }
                }
            }
        }
        v["sahiplik_denetimi"] = json!(sahiplik);
        if !erisim.is_empty() {
            v["erisim_denetimi"] = json!(erisim);
        }
        bilgi["dna"][yer.as_str()] = v;
        // Veri bağı: çağrı zinciriyle bulunan tablolar eklenir
        if !etkiler.is_empty() {
            let bag = k.veri.get_or_insert_with(crate::adaptor::VeriBag::default);
            for (t, _) in &etkiler {
                let l = if t.yazma { &mut bag.yazar } else { &mut bag.okur };
                if !l.contains(&t.tablo) {
                    l.push(t.tablo.clone());
                }
            }
            if a.kesildi || a.dinamik > 0 {
                bag.eksik_olabilir = true;
            }
            bag.okur.sort();
            bag.okur.dedup();
            bag.yazar.sort();
            bag.yazar.dedup();
        }
    }
    let id = |f: usize| kimlik.get(&f).cloned().unwrap_or_else(|| g.fonksiyonlar[f].id.clone());
    // Sıkı biçim: [kaynak, hedef, "dosya:satır", güven, çözüm, hedefin tanımı "dosya:satır"]
    let kenar_json: Vec<Value> = kenarlar
        .iter()
        .map(|&e| {
            let k = &g.kenarlar[e];
            let h = &g.fonksiyonlar[k.hedef];
            json!([id(k.kaynak), id(k.hedef), g.kenar_konumu(e), k.guven.ad(), k.cozum, format!("{}:{}", h.dosya, h.satir)])
        })
        .collect();
    let mut sayilar = g.sayilar();
    sayilar["bicim"] = json!(BICIM);
    sayilar["islem"] = json!(islenen);
    sayilar["ulasilan_fonksiyon"] = json!(dugum.len());
    sayilar["ulasilan_kenar"] = json!(kenar_json.len());
    sayilar["kenarlar"] = json!(kenar_json);
    bilgi["cagri"] = sayilar;
    // Bulunan ORM modelleri: adaptörün aynı adlı modeli yoksa eklenir
    let mut liste = bilgi["veri_modelleri"].as_array().cloned().unwrap_or_default();
    let var: HashSet<String> = liste.iter().filter_map(|m| m["ad"].as_str().map(|s| s.to_lowercase())).collect();
    for m in &g.modeller {
        if m["ad"].as_str().is_some_and(|a| !var.contains(&a.to_lowercase())) {
            liste.push(m.clone());
        }
    }
    if !liste.is_empty() {
        bilgi["veri_modelleri"] = json!(liste);
    }
}

/// Kaynak dosyaları özetleyip grafiği kurar. `yollar`: adaptörün okuduğu dosyalar (mutlak); desteklenen dillerdeki
/// kaynaklar alınır, test dosyaları atlanır. Önbellek dosya başınadır (içeriği değişmeyen dosya yeniden ayrıştırılmaz).
/// `semalar`: veri şeması dosyaları (uygulama köküne göre; Prisma şemaları okunur)
pub fn topla(kok: &Path, yollar: &[PathBuf], onbellek: &mut HashMap<PathBuf, crate::kesif::Kayit<DosyaOzeti>>, ek_modeller: &[Value], semalar: &[String]) -> Grafik {
    let kaynaklar: Vec<PathBuf> = yollar
        .iter()
        .filter(|p| Dil::uzantidan(p).is_some())
        .filter(|p| {
            let y = p.to_string_lossy().replace('\\', "/");
            let ad = y.rsplit('/').next().unwrap_or("");
            !(ad.contains(".spec.")
                || ad.contains(".test.")
                || ad.ends_with("_test.go")
                || ad.ends_with("_spec.rb")
                || ad.ends_with("Test.java")
                || ad.ends_with("Tests.cs")
                || ad.starts_with("test_")
                || y.contains("/__tests__/")
                || y.contains("/node_modules/"))
        })
        .cloned()
        .collect();
    let (sonuclar, _, _) = crate::kesif::ayristir_hepsi(&kaynaklar, onbellek, ayristir);
    let goreli: Vec<(String, &DosyaOzeti)> = sonuclar.iter().map(|(p, o)| (crate::guvenli::goreli(kok, p), o)).collect();
    // Prisma şeması: adaptör bildirmediyse uygulamanın içinde aranır (Express / Fastify + Prisma)
    let mut prisma_yollari: Vec<String> = semalar.iter().filter(|s| s.ends_with(".prisma")).cloned().collect();
    if prisma_yollari.is_empty() && goreli.iter().any(|(_, o)| o.dil == Some(Dil::Ts)) {
        let mut gezgin = ignore::WalkBuilder::new(kok);
        gezgin.hidden(true).git_ignore(true).max_depth(Some(5)).filter_entry(|e| e.file_name() != "node_modules");
        for e in gezgin.build().flatten() {
            if e.file_type().is_some_and(|t| t.is_file()) && e.path().extension().is_some_and(|x| x == "prisma") {
                prisma_yollari.push(crate::guvenli::goreli(kok, e.path()));
            }
        }
    }
    let mut prisma: Vec<(String, String)> = prisma_yollari.iter().filter_map(|s| crate::guvenli::oku(&kok.join(s)).map(|m| (s.clone(), m))).collect();
    // MyBatis XML eşleyicileri (Java): SQL'i XML'de duran eşleyici arayüzlerinin tablo etkisi
    if goreli.iter().any(|(_, o)| o.dil == Some(Dil::Java)) {
        let mut gezgin = ignore::WalkBuilder::new(kok);
        gezgin.hidden(true).git_ignore(true).max_depth(Some(8)).filter_entry(|e| !matches!(e.file_name().to_str(), Some("node_modules" | "target" | "build")));
        for e in gezgin.build().flatten() {
            if e.file_type().is_some_and(|t| t.is_file()) && e.path().extension().is_some_and(|x| x == "xml") {
                if let Some(m) = crate::guvenli::oku(e.path()).filter(|m| m.contains("<mapper") && m.contains("namespace")) {
                    prisma.push((crate::guvenli::goreli(kok, e.path()), m));
                }
            }
        }
    }
    Grafik::kur(&goreli, ek_modeller, &prisma)
}
