//! Ruby on Rails adaptörü (tree-sitter-ruby): NestJS ve FastAPI adaptörleriyle aynı kapı yapısını (`Kapi`) üretir;
//! kart, MCP, önizleme, OpenAPI ve canlı mod dilden bağımsız çalışır.
//!
//! Neyin nereden okunduğu (anlam adlardan değil tanımlardan):
//! - **Rotalar (kesin):** `config/routes.rb` (ve `draw :ad` ile `config/routes/ad.rb`). `resources` / `resource`
//!   yedi eylemi (`only` / `except`, `path`, `param`, `controller`, `module`, `shallow`), iç içe kaynaklar
//!   (`/posts/:post_id/comments`), `member` / `collection` ve `on:`, `namespace` / `scope` / `module` / `controller`,
//!   `concern` / `concerns`, `constraints` (koşullu kapı), fiil rotaları (`get 'yol', to: 'c#a'`, `'yol' => 'c#a'`,
//!   `match via:`), `root`, `devise_for` (modelin `devise` modüllerine göre Devise uçları). Her rota için denetleyici
//!   sınıfı ve eylemi (metot ya da şablon) bulunmalıdır: bulunamayan eylemin rotası kapı sayılmaz (Rails onu da 404 /
//!   ActionNotFound ile reddeder). `mount` edilen motorlar ve `redirect` rotaları işlem değildir.
//! - **Giriş:** denetleyicinin ata zincirindeki ve dahil ettiği modüllerdeki filtreler (`before_action` / `skip_before_action`,
//!   `only` / `except` / `if` / `unless`; Rails'in sırasıyla). Filtrenin tanımı izlenir: Devise `authenticate_<kapsam>!`,
//!   `authenticate_or_request_with_http_token`, `head :unauthorized`, giriş sayfasına yönlendirme ya da 401 fırlatan bir
//!   zincire ulaşırsa giriş gerekir ve karar kesindir. Koşullu filtre, yalnızca rol denetleyen filtre ve izlenemeyen
//!   filtre çıkarımdır. Rol: filtre gövdesindeki rol yüklemleri (`current_user.admin?`, `has_role?(:x)`), Pundit
//!   politikasının metodu (`authorize @post` -> `PostPolicy#update?`), CanCanCan `Ability` kuralları.
//! - **Girdiler:** strong parameters (`params.require(:post).permit(:title, tags: [])`, `params.expect(...)`) gövdenin
//!   tanımıdır; alan tipleri `db/schema.rb` sütunlarından, zorunluluk ve sınırlar modelin doğrulamalarından
//!   (`validates :title, presence: true, length: { maximum: 120 }`, `enum`). Okuma uçlarında `params[:x]` sorgudur.
//! - **Yanıt:** jbuilder şablonu (eylemin örtük şablonu ya da `render :show`), serileştirici (ActiveModel::Serializer,
//!   jsonapi-serializer, Blueprinter) ya da `render json:` ifadesi.
//! - **Veri ve DNA:** ActiveRecord çağrıları (okuma / yazma), `has_many ... dependent: :destroy` ve şemadaki
//!   `on_delete: :cascade` kaskadları, yazılan modelin geri çağrıları (`after_create_commit`, `before_destroy`),
//!   ActiveJob (`perform_later`) ve ActionMailer (`deliver_later`) tetikleri; `raise`, `head :forbidden`,
//!   `render status: :unprocessable_entity`, `find` (404), `save!` (422), Pundit / CanCanCan (403).
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;
use tree_sitter::{Node, Parser};

use crate::adaptor::{Alan, Girdi, Kapi, Sema, VeriBag, Yanit};
use crate::dna::{Dal, Firlatma, GovdeDna};
use crate::guvenli;

pub const SURUM: &str = "0.1.0";

/// Bir gövdede tutulacak en fazla öğe (dev fonksiyonlar belleği doldurmasın)
const EN_FAZLA: usize = 32;
/// Bir metotta tutulacak en fazla çağrı
const EN_FAZLA_CAGRI: usize = 600;

// ---- Ruby modeli ----

/// Çağrı argümanı: sembol, metin, dizi, sözlük ya da çözülemeyen ifade (kaynak metniyle)
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Arg {
    Sembol(String),
    Metin(String),
    Dizi(Vec<Arg>),
    Hash(Vec<(Arg, Arg)>),
    Ham(String),
}

impl Arg {
    /// Sembol ya da metnin değeri
    fn ad(&self) -> Option<&str> {
        match self {
            Arg::Sembol(s) | Arg::Metin(s) => Some(s),
            _ => None,
        }
    }

    /// Tek ad ya da ad dizisi (`:index`, `[:index, :show]`, `%i[index show]`)
    fn adlar(&self) -> Vec<String> {
        match self {
            Arg::Dizi(v) => v.iter().filter_map(|a| a.ad().map(String::from)).collect(),
            a => a.ad().map(|s| vec![s.to_string()]).unwrap_or_default(),
        }
    }

    /// Kaynak koddaki biçimine yakın metin
    fn metin(&self) -> String {
        match self {
            Arg::Sembol(s) => format!(":{s}"),
            Arg::Metin(s) => format!("\"{s}\""),
            Arg::Ham(s) => s.clone(),
            Arg::Dizi(v) => format!("[{}]", v.iter().map(Arg::metin).collect::<Vec<_>>().join(", ")),
            Arg::Hash(v) => format!("{{ {} }}", v.iter().map(|(a, b)| format!("{}: {}", a.ad().unwrap_or(""), b.metin())).collect::<Vec<_>>().join(", ")),
        }
    }

    fn anahtar(&self, k: &str) -> Option<&Arg> {
        match self {
            Arg::Hash(v) => v.iter().find(|(a, _)| matches!(a, Arg::Sembol(s) if s == k)).map(|(_, b)| b),
            _ => None,
        }
    }
}

/// Bir çağrı: alıcı, ad, argümanlar, anahtarlı argümanlar, bloğun ifadeleri (DSL: rotalar, şema, sınıf gövdesi)
#[derive(Debug, Clone, Default)]
pub(crate) struct Cagri {
    pub alici: Option<String>,
    pub ad: String,
    pub args: Vec<Arg>,
    pub anahtarli: Vec<(Arg, Arg)>,
    /// Bloğun ifade düzeyindeki çağrıları (yalnızca DSL için doldurulur)
    pub blok: Vec<Cagri>,
    pub blok_var: bool,
    pub satir: usize,
    /// En yakın koşul (`if` / `unless` / `else` kolu, `a || b`'nin sağı için `!(a)`)
    pub kosul: Option<String>,
    /// Çağrının kısaltılmış kaynak metni
    pub metin: String,
}

impl Cagri {
    fn anahtar(&self, k: &str) -> Option<&Arg> {
        self.anahtarli.iter().find(|(a, _)| matches!(a, Arg::Sembol(s) if s == k)).map(|(_, b)| b)
    }
    fn semboller(&self) -> Vec<String> {
        self.args.iter().flat_map(Arg::adlar).collect()
    }
}

/// Metot: gövdenin bütün çağrıları (her derinlikte, koşullarıyla), atamaları ve davranış özeti
#[derive(Debug, Clone, Default)]
pub(crate) struct Metot {
    pub ad: String,
    pub satir: usize,
    pub sinif_metodu: bool,
    pub ozel: bool,
    pub govde: String,
    pub cagrilar: Vec<Cagri>,
    /// Gövdede geçen yalın adlar (argümansız metot çağrısı olabilir: `authenticate_user!`, `post_params`)
    pub adlar: Vec<String>,
    /// (sol, sağ ifade metni, satır): `@post = Post.find(params[:id])`
    pub atamalar: Vec<(String, String, usize)>,
    pub dna: GovdeDna,
    /// Metodun hemen üstündeki yorum satırları
    pub yorum: Option<String>,
}

/// Sınıf ya da modül (tam adıyla: `Api::V1::PostsController`)
#[derive(Debug, Clone, Default)]
pub(crate) struct Sinif {
    pub ad: String,
    pub ust: Option<String>,
    pub satir: usize,
    /// Sınıf gövdesindeki ifade düzeyindeki çağrılar (`before_action`, `has_many`, `validates`, `include` ...)
    pub cagrilar: Vec<Cagri>,
    pub metotlar: Vec<Metot>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Dosya {
    /// Uygulama köküne göre yol (`/` ile)
    pub goreli: String,
    pub siniflar: Vec<Sinif>,
    /// Sınıf dışındaki ifade düzeyindeki çağrılar (rotalar, şema, jbuilder)
    pub ust: Vec<Cagri>,
    pub satir_sayisi: usize,
    pub hatali_satirlar: Vec<usize>,
}

// ---- Ayrıştırma ----

fn metin(n: Node, k: &[u8]) -> String {
    n.utf8_text(k).unwrap_or("").to_string()
}

fn cocuklar(n: Node) -> Vec<Node> {
    let mut w = n.walk();
    n.named_children(&mut w).collect()
}

fn satir(n: Node) -> usize {
    n.start_position().row + 1
}

/// Kod metninin kısaltılmışı (tek satır, en çok 120 karakter)
fn kisa(s: &str) -> String {
    let ilk: String = s.chars().take(600).collect();
    crate::dna::kisa(&ilk)
}

/// Metin düğümünün içeriği: ara değer (`#{...}`) içeriyorsa None
fn metin_icerigi(n: Node, k: &[u8]) -> Option<String> {
    let c = cocuklar(n);
    if c.iter().any(|x| x.kind() == "interpolation") {
        return None;
    }
    if c.is_empty() {
        let t = metin(n, k);
        return Some(t.trim_start_matches(':').trim_matches(['"', '\'']).to_string());
    }
    Some(c.iter().map(|x| metin(*x, k)).collect())
}

/// Ağaçta çok derin düğüm: özyinelemeli okuyucular (argümanlar, DSL blokları, iç içe sınıflar) burada durur, kötü
/// niyetli iç içe girdi yığını taşırmaz
const EN_DERIN: usize = 128;

fn cok_derin(n: Node) -> bool {
    let mut d = 0;
    let mut x = n;
    while let Some(u) = x.parent() {
        d += 1;
        if d > EN_DERIN {
            return true;
        }
        x = u;
    }
    false
}

fn arg_coz(n: Node, k: &[u8]) -> Arg {
    if cok_derin(n) {
        return Arg::Ham(String::new());
    }
    match n.kind() {
        "simple_symbol" => Arg::Sembol(metin(n, k).trim_start_matches(':').to_string()),
        "hash_key_symbol" | "bare_symbol" => Arg::Sembol(metin(n, k)),
        "delimited_symbol" => metin_icerigi(n, k).map(Arg::Sembol).unwrap_or_else(|| Arg::Ham(metin(n, k))),
        "string" | "bare_string" => metin_icerigi(n, k).map(Arg::Metin).unwrap_or_else(|| Arg::Ham(metin(n, k))),
        "array" | "symbol_array" | "string_array" => {
            let mut v = vec![];
            let mut sozluk = vec![];
            for c in cocuklar(n) {
                match c.kind() {
                    "comment" => {}
                    "pair" => {
                        if let (Some(a), Some(b)) = (c.child_by_field_name("key"), c.child_by_field_name("value")) {
                            sozluk.push((arg_coz(a, k), arg_coz(b, k)));
                        }
                    }
                    _ => v.push(arg_coz(c, k)),
                }
            }
            if !sozluk.is_empty() {
                v.push(Arg::Hash(sozluk));
            }
            Arg::Dizi(v)
        }
        "hash" => Arg::Hash(
            cocuklar(n)
                .into_iter()
                .filter(|c| c.kind() == "pair")
                .filter_map(|c| Some((arg_coz(c.child_by_field_name("key")?, k), arg_coz(c.child_by_field_name("value")?, k))))
                .collect(),
        ),
        "parenthesized_statements" if n.named_child_count() == 1 => n.named_child(0).map(|c| arg_coz(c, k)).unwrap_or_else(|| Arg::Ham(metin(n, k))),
        _ => Arg::Ham(metin(n, k)),
    }
}

/// Çağrı düğümünden `Cagri`; `dsl` ise bloğun ifadeleri de okunur
fn cagri_coz(n: Node, k: &[u8], kosul: Option<String>, dsl: bool) -> Cagri {
    let mut c = Cagri {
        ad: n.child_by_field_name("method").map(|m| metin(m, k)).unwrap_or_default(),
        alici: n.child_by_field_name("receiver").map(|r| metin(r, k)),
        satir: satir(n),
        kosul,
        metin: kisa(&metin(n, k)),
        ..Default::default()
    };
    if let Some(a) = n.child_by_field_name("arguments") {
        for x in cocuklar(a) {
            match x.kind() {
                "pair" => {
                    if let (Some(ka), Some(va)) = (x.child_by_field_name("key"), x.child_by_field_name("value")) {
                        c.anahtarli.push((arg_coz(ka, k), arg_coz(va, k)));
                    }
                }
                "comment" => {}
                _ => c.args.push(arg_coz(x, k)),
            }
        }
    }
    if let Some(b) = n.child_by_field_name("block") {
        c.blok_var = true;
        if dsl {
            if let Some(g) = b.child_by_field_name("body") {
                ifadeler(g, k, None, &mut c.blok);
            }
        }
    }
    c
}

/// İfade düzeyindeki çağrılar (DSL): koşul kolları koşullarıyla, `begin` / parantez içleri açılarak
fn ifadeler(n: Node, k: &[u8], kosul: Option<String>, out: &mut Vec<Cagri>) {
    for x in cocuklar(n) {
        ifade(x, k, kosul.clone(), out);
    }
}

fn ifade(x: Node, k: &[u8], kosul: Option<String>, out: &mut Vec<Cagri>) {
    if out.len() > 5000 || cok_derin(x) {
        return;
    }
    let kosul_metni = |x: Node| x.child_by_field_name("condition").map(|c| kisa(&metin(c, k))).unwrap_or_default();
    match x.kind() {
        "call" => out.push(cagri_coz(x, k, kosul, true)),
        "identifier" | "constant" => out.push(Cagri { ad: metin(x, k), satir: satir(x), kosul, metin: metin(x, k), ..Default::default() }),
        "assignment" => {
            if let (Some(l), Some(r)) = (x.child_by_field_name("left"), x.child_by_field_name("right")) {
                out.push(Cagri { ad: "=".into(), alici: Some(metin(l, k)), args: vec![arg_coz(r, k)], satir: satir(x), kosul, metin: kisa(&metin(x, k)), ..Default::default() });
            }
        }
        "if" | "elsif" | "unless" => {
            let kd = kosul_metni(x);
            let (evet, hayir) = if x.kind() == "unless" { (format!("!({kd})"), kd) } else { (kd.clone(), format!("!({kd})")) };
            if let Some(c) = x.child_by_field_name("consequence") {
                ifadeler(c, k, Some(evet), out);
            }
            if let Some(a) = x.child_by_field_name("alternative") {
                if a.kind() == "elsif" {
                    ifade(a, k, Some(hayir), out);
                } else {
                    ifadeler(a, k, Some(hayir), out);
                }
            }
        }
        "if_modifier" | "unless_modifier" => {
            let kd = kosul_metni(x);
            let kd = if x.kind() == "unless_modifier" { format!("!({kd})") } else { kd };
            if let Some(b) = x.child_by_field_name("body") {
                ifade(b, k, Some(kd), out);
            }
        }
        "begin" | "parenthesized_statements" | "then" | "else" | "body_statement" | "block_body" => ifadeler(x, k, kosul, out),
        _ => {}
    }
}

/// Düğümün içinde bulunduğu koşul: en yakın `if` / `unless` / `elsif` (else kolunda olumsuzu), değiştirici
/// (`x if y`), üçlü ifade, `a || b` (sağ taraf `!(a)` koşulunda çalışır), `rescue`
fn kosul_rb(mut n: Node, kok: Node, k: &[u8]) -> Option<String> {
    let olumsuz = |s: String| format!("!({s})");
    for _ in 0..64 {
        if n.id() == kok.id() {
            return None;
        }
        let ust = n.parent()?;
        let alan = |a: &str| ust.child_by_field_name(a).is_some_and(|c| c.id() == n.id());
        let kosul = || ust.child_by_field_name("condition").map(|c| kisa(&metin(c, k)));
        match ust.kind() {
            "if" | "elsif" | "conditional" if alan("consequence") => return kosul(),
            "if" | "elsif" | "conditional" if alan("alternative") => return kosul().map(olumsuz),
            "unless" if alan("consequence") => return kosul().map(olumsuz),
            "unless" if alan("alternative") => return kosul(),
            "if_modifier" if alan("body") => return kosul(),
            "unless_modifier" if alan("body") => return kosul().map(olumsuz),
            "binary" if alan("right") => {
                let op = ust.child_by_field_name("operator").map(|o| metin(o, k)).unwrap_or_default();
                let sol = ust.child_by_field_name("left").map(|l| kisa(&metin(l, k)))?;
                match op.as_str() {
                    "||" | "or" => return Some(olumsuz(sol)),
                    "&&" | "and" => return Some(sol),
                    _ => {}
                }
            }
            "rescue" => return Some("rescue".into()),
            _ => {}
        }
        n = ust;
    }
    None
}

/// Metodun gövdesi: bütün çağrılar (kaynak sırasıyla), yalın adlar, atamalar ve davranış özeti
fn govde_doldur(mt: &mut Metot, g: Node, k: &[u8]) {
    mt.govde = metin(g, k);
    let mut adlar = BTreeSet::new();
    let mut yigin = vec![g];
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "call" if mt.cagrilar.len() < EN_FAZLA_CAGRI => mt.cagrilar.push(cagri_coz(n, k, kosul_rb(n, g, k), false)),
            "identifier" => {
                let sol = n.parent().is_some_and(|p| matches!(p.kind(), "assignment" | "operator_assignment") && p.child_by_field_name("left").is_some_and(|l| l.id() == n.id()));
                let parametre = n.parent().is_some_and(|p| p.kind().ends_with("parameters") || p.kind().ends_with("parameter"));
                if !sol && !parametre && adlar.len() < 400 {
                    adlar.insert(metin(n, k));
                }
            }
            "assignment" | "operator_assignment" => {
                if let (Some(l), Some(r)) = (n.child_by_field_name("left"), n.child_by_field_name("right")) {
                    if matches!(l.kind(), "instance_variable" | "identifier") && mt.atamalar.len() < 200 {
                        mt.atamalar.push((metin(l, k), metin(r, k), satir(n)));
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
    mt.adlar = adlar.into_iter().collect();
    mt.dna = ruby_dna(g, k);
}

fn metot_coz(m: Node, k: &[u8], ozel: bool, yorum: Option<String>) -> Metot {
    let mut mt = Metot {
        ad: m.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default(),
        satir: satir(m),
        sinif_metodu: m.kind() == "singleton_method",
        ozel,
        yorum,
        ..Default::default()
    };
    if let Some(g) = m.child_by_field_name("body") {
        govde_doldur(&mut mt, g, k);
    }
    mt
}

/// Bloklu geri çağrı / filtre makroları (`after_create_commit { ... }`, `before_action do ... end`): blok adsız bir
/// metot olarak saklanır, makronun ilk argümanı o metodun adıdır
fn geri_cagri_makrosu(ad: &str) -> bool {
    ad.starts_with("before_") || ad.starts_with("after_") || ad.starts_with("around_")
}

fn sinif_coz(x: Node, k: &[u8], kapsam: &[String], d: &mut Dosya) {
    if cok_derin(x) {
        return;
    }
    let ad_ham = x.child_by_field_name("name").map(|a| metin(a, k)).unwrap_or_default();
    let mut tam: Vec<String> = kapsam.to_vec();
    tam.extend(ad_ham.trim_start_matches("::").split("::").filter(|p| !p.is_empty()).map(String::from));
    let mut s = Sinif {
        ad: tam.join("::"),
        ust: x.child_by_field_name("superclass").and_then(|u| u.named_child(0)).map(|c| metin(c, k).trim_start_matches("::").to_string()),
        satir: satir(x),
        ..Default::default()
    };
    if let Some(g) = x.child_by_field_name("body") {
        let mut ozel = false;
        let mut yorum: Vec<String> = vec![];
        let mut son_yorum_satiri = 0usize;
        for c in cocuklar(g) {
            let ust_yorum = |yorum: &mut Vec<String>, son: usize, c: Node| -> Option<String> {
                let y = (son + 1 == satir(c) && !yorum.is_empty()).then(|| yorum.join(" "));
                yorum.clear();
                y
            };
            match c.kind() {
                "comment" => {
                    if son_yorum_satiri + 1 != satir(c) {
                        yorum.clear();
                    }
                    yorum.push(metin(c, k).trim_start_matches('#').trim().to_string());
                    son_yorum_satiri = satir(c);
                }
                "method" | "singleton_method" => {
                    let y = ust_yorum(&mut yorum, son_yorum_satiri, c);
                    s.metotlar.push(metot_coz(c, k, ozel, y));
                }
                "class" | "module" => sinif_coz(c, k, &tam, d),
                "identifier" => {
                    match metin(c, k).as_str() {
                        "private" | "protected" => ozel = true,
                        "public" => ozel = false,
                        _ => {}
                    }
                    ifade(c, k, None, &mut s.cagrilar);
                }
                "call" => {
                    let ad = c.child_by_field_name("method").map(|m| metin(m, k)).unwrap_or_default();
                    let ic_metot = c.child_by_field_name("arguments").and_then(|a| cocuklar(a).into_iter().find(|x| matches!(x.kind(), "method" | "singleton_method")));
                    if let (true, Some(m)) = (matches!(ad.as_str(), "private" | "protected" | "public" | "private_class_method"), ic_metot) {
                        s.metotlar.push(metot_coz(m, k, ad != "public", None));
                        continue;
                    }
                    let mut cg = cagri_coz(c, k, None, true);
                    if matches!(ad.as_str(), "private" | "protected") && c.child_by_field_name("receiver").is_none() {
                        // private :a, :b
                        let adlar = cg.semboller();
                        for m in s.metotlar.iter_mut().filter(|m| adlar.contains(&m.ad)) {
                            m.ozel = true;
                        }
                    }
                    if cg.blok_var && geri_cagri_makrosu(&ad) {
                        if let Some(b) = c.child_by_field_name("block") {
                            let anonim = format!("{ad}@{}", cg.satir);
                            let mut mt = Metot { ad: anonim.clone(), satir: cg.satir, ozel: true, ..Default::default() };
                            if let Some(gv) = b.child_by_field_name("body") {
                                govde_doldur(&mut mt, gv, k);
                            }
                            s.metotlar.push(mt);
                            cg.args.insert(0, Arg::Sembol(anonim));
                        }
                    }
                    s.cagrilar.push(cg);
                }
                _ => ifade(c, k, None, &mut s.cagrilar),
            }
        }
    }
    d.siniflar.push(s);
}

fn gez(n: Node, k: &[u8], d: &mut Dosya) {
    for x in cocuklar(n) {
        match x.kind() {
            "class" | "module" => sinif_coz(x, k, &[], d),
            _ => ifade(x, k, None, &mut d.ust),
        }
    }
}

/// Sözdizimi hatalı satırlar (özyinelemesiz)
fn hatalar(n: Node, out: &mut Vec<usize>) {
    let mut yigin = vec![n];
    while let Some(n) = yigin.pop() {
        if n.is_error() || n.is_missing() {
            if out.len() < 1000 {
                out.push(n.start_position().row + 1);
            }
            continue;
        }
        if n.has_error() {
            let mut w = n.walk();
            yigin.extend(n.children(&mut w));
        }
    }
    out.sort();
    out.dedup();
}

pub(crate) fn ayristir_metin(goreli: &str, kaynak: &str) -> Option<Dosya> {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_ruby::LANGUAGE.into()).ok()?;
    let agac = p.parse(kaynak, None)?;
    let k = kaynak.as_bytes();
    let mut d = Dosya { goreli: goreli.to_string(), satir_sayisi: kaynak.lines().count(), ..Default::default() };
    gez(agac.root_node(), k, &mut d);
    hatalar(agac.root_node(), &mut d.hatali_satirlar);
    Some(d)
}

// ---- Davranış özeti (Ruby gövdesi) ----

/// Rack durum sembolü ya da sayı -> HTTP kodu
fn durum_kodu(s: &str) -> Option<u16> {
    let s = s.trim().trim_start_matches(':').trim_matches(['"', '\'']);
    if let Ok(n) = s.parse::<u16>() {
        return (100..600).contains(&n).then_some(n);
    }
    Some(match s {
        "ok" => 200,
        "created" => 201,
        "accepted" => 202,
        "no_content" => 204,
        "moved_permanently" => 301,
        "found" => 302,
        "see_other" => 303,
        "not_modified" => 304,
        "bad_request" => 400,
        "unauthorized" => 401,
        "payment_required" => 402,
        "forbidden" => 403,
        "not_found" => 404,
        "method_not_allowed" => 405,
        "not_acceptable" => 406,
        "request_timeout" => 408,
        "conflict" => 409,
        "gone" => 410,
        "precondition_failed" => 412,
        "payload_too_large" | "request_entity_too_large" => 413,
        "unsupported_media_type" => 415,
        "unprocessable_entity" | "unprocessable_content" => 422,
        "locked" => 423,
        "too_many_requests" => 429,
        "internal_server_error" => 500,
        "not_implemented" => 501,
        "bad_gateway" => 502,
        "service_unavailable" => 503,
        "gateway_timeout" => 504,
        _ => return None,
    })
}

/// Rails ve yaygın gem istisnaları -> Rails'in varsayılan HTTP karşılığı
fn istisna_kodu(ad: &str) -> Option<u16> {
    let son = ad.trim_start_matches("::").rsplit("::").next().unwrap_or(ad);
    Some(match son {
        "RecordNotFound" | "RoutingError" | "ActionNotFound" => 404,
        "ParameterMissing" | "BadRequest" | "UnpermittedParameters" => 400,
        "RecordInvalid" | "RecordNotSaved" | "RecordNotDestroyed" | "InvalidAuthenticityToken" => 422,
        "NotAuthorizedError" | "AccessDenied" | "Forbidden" => 403,
        "Unauthorized" | "NotAuthenticated" | "AuthenticationError" | "InvalidToken" => 401,
        "UnknownFormat" => 406,
        "StaleObjectError" | "Conflict" => 409,
        "TooManyRequests" => 429,
        _ => return None,
    })
}

const YUMUSAK_ALAN: [&str; 6] = ["deleted_at", "discarded_at", "archived_at", "trashed_at", "is_deleted", "deleted"];
const YUMUSAK_CAGRI: [&str; 5] = ["discard", "discard!", "soft_delete", "soft_destroy", "trash"];
/// Kayıt bulunamazsa ActiveRecord::RecordNotFound (404) fırlatan bulucular
const BULUCU: [&str; 4] = ["find", "find_by!", "find_sole_by", "sole"];
/// Doğrulama başarısızsa ActiveRecord::RecordInvalid (422) fırlatan yazıcılar
const UNLEMLI_YAZMA: [&str; 5] = ["save!", "create!", "update!", "validate!", "find_or_create_by!"];

/// Basit dal ifadesi: ad, sabit, metin, sembol, sayı
fn basit_mi(n: Node) -> bool {
    matches!(n.kind(), "identifier" | "constant" | "instance_variable" | "string" | "simple_symbol" | "integer" | "float" | "true" | "false" | "nil" | "scope_resolution")
}

/// Ruby gövdesinden davranış öğeleri: fırlatmalar (koşulu ve HTTP koduyla), iki yollu ifadeler, yumuşak silme ve
/// tetikler (ActiveJob, ActionMailer, ActiveSupport::Notifications)
fn ruby_dna(g: Node, k: &[u8]) -> GovdeDna {
    let mut d = GovdeDna::default();
    let mut yigin = vec![g];
    let ekle = |d: &mut GovdeDna, n: Node, durum: Option<u16>, istisna: String, kosul: Option<String>| {
        if d.firlatmalar.len() < EN_FAZLA {
            let sahiplik = kosul.as_deref().is_some_and(|c| crate::dna::sahiplik_mi(c, durum));
            d.firlatmalar.push(Firlatma { durum, istisna, kosul, satir: satir(n), sahiplik });
        }
    };
    while let Some(n) = yigin.pop() {
        match n.kind() {
            "call" => {
                let ad = n.child_by_field_name("method").map(|m| metin(m, k)).unwrap_or_default();
                let alici = n.child_by_field_name("receiver").map(|r| metin(r, k));
                let args: Vec<Node> = n.child_by_field_name("arguments").map(cocuklar).unwrap_or_default();
                let anahtar = |a: &str| {
                    args.iter().filter(|x| x.kind() == "pair").find_map(|p| {
                        let ka = metin(p.child_by_field_name("key")?, k);
                        (ka.trim_start_matches(':') == a).then(|| p.child_by_field_name("value").map(|v| metin(v, k)))?
                    })
                };
                match (alici.as_deref(), ad.as_str()) {
                    (None, "raise" | "fail") => {
                        let ilk = args.first().map(|a| metin(*a, k)).unwrap_or_default();
                        let istisna = if ilk.starts_with(['"', '\'']) || ilk.is_empty() {
                            "RuntimeError".to_string()
                        } else {
                            ilk.split([',', '(', ' ']).next().unwrap_or("").trim_end_matches(".new").to_string()
                        };
                        let kosul = kosul_rb(n, g, k);
                        ekle(&mut d, n, istisna_kodu(&istisna), istisna, kosul);
                    }
                    (None, "head") => {
                        if let Some(durum) = args.first().and_then(|a| durum_kodu(&metin(*a, k))).filter(|d| *d >= 400) {
                            let kosul = kosul_rb(n, g, k);
                            ekle(&mut d, n, Some(durum), "head".into(), kosul);
                        }
                    }
                    (None, "render") => {
                        if let Some(durum) = anahtar("status").and_then(|s| durum_kodu(&s)).filter(|d| *d >= 400) {
                            let kosul = kosul_rb(n, g, k);
                            ekle(&mut d, n, Some(durum), "render".into(), kosul);
                        }
                    }
                    (None, "authorize") => ekle(&mut d, n, Some(403), "Pundit::NotAuthorizedError".into(), Some(kisa(&metin(n, k)))),
                    (None, "authorize!") => ekle(&mut d, n, Some(403), "CanCan::AccessDenied".into(), Some(kisa(&metin(n, k)))),
                    (Some("params"), "require" | "expect") => ekle(&mut d, n, Some(400), "ActionController::ParameterMissing".into(), Some(kisa(&metin(n, k)))),
                    (Some(a), m) if BULUCU.contains(&m) && a != "params" && n.child_by_field_name("block").is_none() && !args.iter().any(|x| x.kind() == "block_argument") => {
                        ekle(&mut d, n, Some(404), "ActiveRecord::RecordNotFound".into(), Some(kisa(&metin(n, k))))
                    }
                    (Some(_), m) if UNLEMLI_YAZMA.contains(&m) => ekle(&mut d, n, Some(422), "ActiveRecord::RecordInvalid".into(), Some(kisa(&metin(n, k)))),
                    _ => {}
                }
                if d.yumusak.is_none() && (YUMUSAK_CAGRI.contains(&ad.as_str()) && alici.is_some()) {
                    d.yumusak = Some(satir(n));
                }
                if d.yumusak.is_none() && matches!(ad.as_str(), "update" | "update!" | "update_column" | "update_columns" | "update_attribute") {
                    let m = metin(n, k);
                    if YUMUSAK_ALAN.iter().any(|a| m.contains(&format!("{a}:")) || m.contains(&format!(":{a},"))) && (m.contains("Time") || m.contains("now") || m.contains("true"))
                    {
                        d.yumusak = Some(satir(n));
                    }
                }
                if d.tetikler.len() < EN_FAZLA {
                    if let Some(t) = tetik(alici.as_deref(), &ad, &args.iter().map(|a| metin(*a, k)).collect::<Vec<_>>()) {
                        if !d.tetikler.iter().any(|x| x.0 == t.0 && x.1 == t.1) {
                            d.tetikler.push((t.0, t.1, satir(n)));
                        }
                    }
                }
            }
            "conditional" if d.dallar.len() < EN_FAZLA => {
                if let (Some(c), Some(e), Some(h)) = (n.child_by_field_name("condition"), n.child_by_field_name("consequence"), n.child_by_field_name("alternative")) {
                    if basit_mi(e) && basit_mi(h) {
                        let baglam = n
                            .parent()
                            .and_then(|u| match u.kind() {
                                "pair" => u.child_by_field_name("key").map(|a| metin(a, k)),
                                "assignment" => u.child_by_field_name("left").map(|a| metin(a, k)),
                                _ => None,
                            })
                            .unwrap_or_default();
                        d.dallar.push(Dal { kosul: kisa(&metin(c, k)), evet: metin(e, k), hayir: metin(h, k), baglam, satir: satir(n) });
                    }
                }
            }
            "assignment" if d.yumusak.is_none() => {
                if let (Some(l), Some(r)) = (n.child_by_field_name("left"), n.child_by_field_name("right")) {
                    let sol = metin(l, k);
                    let sag = metin(r, k);
                    if YUMUSAK_ALAN.iter().any(|a| sol.ends_with(&format!(".{a}"))) && (sag.contains("Time") || sag.contains("now") || sag == "true") {
                        d.yumusak = Some(satir(n));
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

/// Çağrı bir iş ya da olay tetikliyor mu: `XJob.perform_later` / `.set(...).perform_later` / Sidekiq `perform_async`
/// -> ("job", "XJob"); `XMailer.with(...).m.deliver_later` / `XMailer.m(...).deliver_now` -> ("job", "XMailer.m");
/// `ActiveSupport::Notifications.instrument("ad")` -> ("event", "ad")
fn tetik(alici: Option<&str>, ad: &str, args: &[String]) -> Option<(String, String)> {
    static D: OnceLock<[Regex; 2]> = OnceLock::new();
    let [is_alici, posta] = D.get_or_init(|| {
        [Regex::new(r"^((?:[A-Z]\w*::)*[A-Z]\w*)(?:\.set\(.*\))?$").unwrap(), Regex::new(r"^((?:[A-Z]\w*::)*[A-Z]\w*Mailer)(?:\.with\(.*?\))?\.(\w+)(?:\(.*\))?$").unwrap()]
    });
    let a = alici?.trim();
    match ad {
        "perform_later" | "perform_now" | "perform_async" | "perform_in" | "perform_at" | "perform_bulk" | "perform_all_later" => {
            is_alici.captures(a).map(|c| ("job".to_string(), c[1].to_string()))
        }
        "deliver_later" | "deliver_now" | "deliver_later!" | "deliver_now!" => {
            posta.captures(&a.split_whitespace().collect::<String>()).map(|c| ("job".to_string(), format!("{}.{}", &c[1], &c[2])))
        }
        "instrument" if a.ends_with("Notifications") => {
            args.first().map(|x| x.trim_matches(['"', '\'', ':']).to_string()).filter(|x| !x.is_empty()).map(|x| ("event".to_string(), x))
        }
        _ => None,
    }
}

// ---- Çekim (Rails'in İngilizce çekim kuralları, en sık kullanılanlar) ----

const SAYILAMAYAN: [&str; 11] = ["equipment", "information", "rice", "money", "species", "series", "fish", "sheep", "jeans", "police", "news"];
const DUZENSIZ: [(&str, &str); 7] = [("person", "people"), ("man", "men"), ("woman", "women"), ("child", "children"), ("sex", "sexes"), ("move", "moves"), ("zombie", "zombies")];

fn kural_listesi(kurallar: &[(&str, &str)]) -> Vec<(Regex, String)> {
    kurallar.iter().map(|(d, y)| (Regex::new(&format!("(?i){d}")).unwrap(), y.to_string())).collect()
}

/// Tekil biçim (snake_case adlarda son sözcük): posts -> post, categories -> category, statuses -> status
pub(crate) fn tekil(s: &str) -> String {
    if let Some((bas, son)) = s.rsplit_once('_') {
        return format!("{bas}_{}", tekil(son));
    }
    let a = s.to_lowercase();
    if SAYILAMAYAN.contains(&a.as_str()) || DUZENSIZ.iter().any(|(t, _)| *t == a) {
        return s.to_string();
    }
    if let Some((t, _)) = DUZENSIZ.iter().find(|(_, c)| *c == a) {
        return t.to_string();
    }
    static K: OnceLock<Vec<(Regex, String)>> = OnceLock::new();
    // Rails'in sırasının tersi: sonra tanımlanan önce denenir
    let kurallar = K.get_or_init(|| {
        kural_listesi(&[
            (r"(database)s$", "$1"),
            (r"(quiz)zes$", "$1"),
            (r"(matr)ices$", "${1}ix"),
            (r"(vert|ind)ices$", "${1}ex"),
            (r"^(ox)en", "$1"),
            (r"(alias|status)(es)?$", "$1"),
            (r"(octop|vir)(us|i)$", "${1}us"),
            (r"^(a)x[ie]s$", "${1}xis"),
            (r"(cris|test)(is|es)$", "${1}is"),
            (r"(shoe)s$", "$1"),
            (r"(o)es$", "$1"),
            (r"(bus)(es)?$", "$1"),
            (r"^(m|l)ice$", "${1}ouse"),
            (r"(x|ch|ss|sh)es$", "$1"),
            (r"(m)ovies$", "${1}ovie"),
            (r"(s)eries$", "${1}eries"),
            (r"([^aeiouy]|qu)ies$", "${1}y"),
            (r"([lr])ves$", "${1}f"),
            (r"(tive)s$", "$1"),
            (r"(hive)s$", "$1"),
            (r"([^f])ves$", "${1}fe"),
            (r"(^analy)(sis|ses)$", "${1}sis"),
            (r"([ti])a$", "${1}um"),
            (r"(n)ews$", "${1}ews"),
            (r"(ss)$", "$1"),
            (r"s$", ""),
        ])
    });
    for (d, y) in kurallar {
        if d.is_match(s) {
            return d.replace(s, y.as_str()).to_string();
        }
    }
    s.to_string()
}

/// Çoğul biçim (son sözcük): post -> posts, category -> categories, status -> statuses, person -> people
pub(crate) fn cogul(s: &str) -> String {
    if let Some((bas, son)) = s.rsplit_once('_') {
        return format!("{bas}_{}", cogul(son));
    }
    let a = s.to_lowercase();
    if SAYILAMAYAN.contains(&a.as_str()) || DUZENSIZ.iter().any(|(_, c)| *c == a) {
        return s.to_string();
    }
    if let Some((_, c)) = DUZENSIZ.iter().find(|(t, _)| *t == a) {
        return c.to_string();
    }
    static K: OnceLock<Vec<(Regex, String)>> = OnceLock::new();
    let kurallar = K.get_or_init(|| {
        kural_listesi(&[
            (r"(quiz)$", "${1}zes"),
            (r"^(oxen)$", "$1"),
            (r"^(ox)$", "${1}en"),
            (r"^(m|l)ice$", "${1}ice"),
            (r"^(m|l)ouse$", "${1}ice"),
            (r"(matr|vert|ind)(?:ix|ex)$", "${1}ices"),
            (r"(x|ch|ss|sh)$", "${1}es"),
            (r"([^aeiouy]|qu)y$", "${1}ies"),
            (r"(hive)$", "${1}s"),
            (r"(?:([^f])fe|([lr])f)$", "${1}${2}ves"),
            (r"sis$", "ses"),
            (r"([ti])a$", "${1}a"),
            (r"([ti])um$", "${1}a"),
            (r"(buffal|tomat)o$", "${1}oes"),
            (r"(bu)s$", "${1}ses"),
            (r"(alias|status)$", "${1}es"),
            (r"(octop|vir)i$", "${1}i"),
            (r"(octop|vir)us$", "${1}i"),
            (r"^(ax|test)is$", "${1}es"),
            (r"s$", "s"),
            (r"$", "s"),
        ])
    });
    for (d, y) in kurallar {
        if d.is_match(s) {
            return d.replacen(s, 1, y.as_str()).to_string();
        }
    }
    format!("{s}s")
}

/// snake_case / yol -> CamelCase / modül: "api/v1/blog_posts" -> "Api::V1::BlogPosts"
pub(crate) fn camelize(s: &str) -> String {
    s.split('/')
        .map(|p| {
            p.split('_')
                .filter(|x| !x.is_empty())
                .map(|w| {
                    let mut c = w.chars();
                    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("::")
}

/// CamelCase -> snake_case: "ArsivKaydi" -> "arsiv_kaydi", "APIKey" -> "api_key"
pub(crate) fn underscore(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, ch) in c.iter().enumerate() {
        if ch.is_uppercase() {
            let onceki_kucuk = i > 0 && (c[i - 1].is_lowercase() || c[i - 1].is_ascii_digit());
            let sonraki_kucuk = i > 0 && c[i - 1].is_uppercase() && c.get(i + 1).is_some_and(|n| n.is_lowercase());
            if onceki_kucuk || sonraki_kucuk {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(*ch);
        }
    }
    out
}

/// Model sınıfı adı: "comments" -> "Comment"
fn sinif_adi(s: &str) -> String {
    camelize(&tekil(s))
}

/// snake_case -> camelCase (işlem kimliği için): mark_read -> markRead
fn snake_camel(s: &str) -> String {
    let mut out = String::new();
    let mut buyuk = false;
    for c in s.chars() {
        if c == '_' {
            buyuk = !out.is_empty();
        } else if !(c.is_alphanumeric()) {
            continue;
        } else if buyuk {
            out.extend(c.to_uppercase());
            buyuk = false;
        } else {
            out.push(c);
        }
    }
    out
}

// ---- Keşif ----

/// Gemfile'daki gem adları (yorum satırları hariç)
pub(crate) fn gem_listesi(m: &str) -> Vec<String> {
    static D: OnceLock<Regex> = OnceLock::new();
    let d = D.get_or_init(|| Regex::new(r#"^\s*gem\s*\(?\s*["']([\w\-.]+)["']"#).unwrap());
    m.lines().filter_map(|l| d.captures(l).map(|c| c[1].to_string())).collect()
}

/// Ruby on Rails uygulaması mı: Gemfile'da rails (ya da railties / actionpack) ve config/routes.rb var
pub fn rails_mi(kok: &Path) -> bool {
    kok.join("config").join("routes.rb").is_file()
        && guvenli::oku(&kok.join("Gemfile")).is_some_and(|g| gem_listesi(&g).iter().any(|x| matches!(x.as_str(), "rails" | "railties" | "actionpack")))
}

/// Uygulamanın adı: config/application.rb'deki modül (`module Blog` -> "blog"); Rails uygulamasının sürümü yoktur
pub fn paket(kok: &Path) -> Value {
    static D: OnceLock<Regex> = OnceLock::new();
    let d = D.get_or_init(|| Regex::new(r"(?m)^\s*module\s+([A-Z]\w*)").unwrap());
    let ad = guvenli::oku(&kok.join("config").join("application.rb")).and_then(|m| d.captures(&m).map(|c| underscore(&c[1])));
    json!({ "name": ad, "version": null })
}

/// Adaptörün okuduğu dosya mı (uygulama köküne göre yol): app/ altındaki Ruby ve jbuilder dosyaları, rotalar, şema ve
/// lib/ (görevler hariç)
fn okunur(goreli: &str) -> bool {
    let rb = goreli.ends_with(".rb");
    (goreli.starts_with("app/") && (rb || goreli.ends_with(".jbuilder")))
        || goreli == "config/routes.rb"
        || (goreli.starts_with("config/routes/") && rb)
        || goreli == "db/schema.rb"
        || (goreli.starts_with("lib/") && rb && !goreli.starts_with("lib/tasks/") && !goreli.starts_with("lib/generators/"))
}

/// Uygulamanın API'si olmayan klasörler: bağımlılıklar, geçici dosyalar, testler, göçler
const ATLANAN: [&str; 12] = ["vendor", "node_modules", "tmp", "log", "coverage", "public", "storage", ".bundle", "spec", "test", "features", "migrate"];

/// Keşif, paralel ayrıştırma (önbellekli) ve Rails adaptörü
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
        let goreli = guvenli::goreli(kok, p);
        if goreli.split('/').any(|c| ATLANAN.contains(&c)) || !okunur(&goreli) {
            atlanan += 1;
            continue;
        }
        if g.metadata().is_ok_and(|m| m.len() > crate::EN_BUYUK_DOSYA) {
            buyuk.push(goreli);
            continue;
        }
        yollar.push(p.to_path_buf());
    }
    let t_kesif = t0.elapsed();
    let (sonuclar, mut okunan, ayristirilan) = crate::kesif::ayristir_hepsi(&yollar, onbellek, |p, b| {
        let m = String::from_utf8(b).ok()?;
        ayristir_metin(&guvenli::goreli(kok, p), &m)
    });
    let dosyalar: Vec<Dosya> = sonuclar.into_iter().map(|(_, d)| d).collect();
    let t_ayristirma = t0.elapsed();
    let (kapilar, bilgi) = kapilar(kok, &dosyalar);
    // Uygulamanın adı config/application.rb'den okunur: bayatlık onu da izlesin
    let uygulama = kok.join("config").join("application.rb");
    if let Some(b) = guvenli::oku_bayt(&uygulama) {
        okunan.push((uygulama, crate::kesif::icerik_ozeti(&b)));
    }
    let sema = dosyalar.iter().any(|d| d.goreli == "db/schema.rb");
    let t_toplam = t0.elapsed();
    crate::Toplam {
        kapilar,
        adaptor: format!("rails@{SURUM}"),
        dosya_sayisi: dosyalar.len(),
        satir: dosyalar.iter().map(|d| d.satir_sayisi).sum(),
        kismi: dosyalar.iter().filter(|d| !d.hatali_satirlar.is_empty()).map(|d| d.goreli.clone()).collect(),
        atlanan,
        buyuk,
        ek_paketler: vec![],
        // Tablo tanımı: db/schema.rb; yoksa model dosyaları
        veri_dosyalari: if sema {
            vec!["db/schema.rb".to_string()]
        } else {
            bilgi["veri_modelleri"].as_array().into_iter().flatten().filter_map(|m| m["dosya"].as_str().map(String::from)).collect::<BTreeSet<_>>().into_iter().collect()
        },
        bilgi,
        ayristirilan,
        okunan,
        t_kesif,
        t_ayristirma,
        t_toplam,
    }
}

// ---- Proje: sınıflar, modeller, şema ----

/// Sınıf adının arama anahtarı: büyük-küçük harf ve alt çizgi duyarsız (Zeitwerk kısaltmaları: API / Api)
fn anahtar(ad: &str) -> String {
    ad.trim_start_matches("::").to_lowercase().replace('_', "")
}

struct Tablo {
    ad: String,
    satir: usize,
    /// (sütun, tip, null: false, satır)
    sutunlar: Vec<(String, String, bool, usize)>,
}

#[derive(Clone, Debug)]
struct Dogrulama {
    tur: String,
    secenek: Option<Arg>,
    /// `if:` / `unless:` ile: zorunluluk kesin değil
    kosullu: bool,
    /// `on: :update` olmayan (oluşturmada geçerli)
    olusturmada: bool,
}

#[derive(Default)]
struct Model {
    ad: String,
    sinif: (usize, usize),
    tablo: String,
    acik_tablo: bool,
    /// ilişki adı -> (model sınıfının tam adı, çoğul mu)
    iliskiler: HashMap<String, (String, bool)>,
    dogrulamalar: BTreeMap<String, Vec<Dogrulama>>,
    ozel_dogrulamalar: Vec<String>,
    enumlar: HashMap<String, Vec<String>>,
    ekler: Vec<String>,
    devise: Vec<String>,
    kapsamlar: HashSet<String>,
}

struct Proje<'a> {
    dosyalar: &'a [Dosya],
    /// sınıf anahtarı -> (dosya, sınıf) listesi (Ruby sınıfları birden çok dosyada yeniden açılabilir)
    siniflar: HashMap<String, Vec<(usize, usize)>>,
    gemler: HashSet<String>,
    tablolar: BTreeMap<String, Tablo>,
    sema: Option<usize>,
    /// model sınıfının anahtarı -> model
    modeller: BTreeMap<String, Model>,
    /// app/views altındaki dosyalar (göreli, `/` ile)
    gorunumler: BTreeSet<String>,
    /// jbuilder şablonları: app/views'e göre yol -> dosya indeksi
    sablonlar: HashMap<String, usize>,
}

/// Kolon tipinden (db/schema.rb) Sezgi alan tipi ve kuralı
fn sutun_tipi(tip: &str) -> (Option<&'static str>, Option<&'static str>) {
    match tip {
        "string" | "text" | "citext" | "inet" | "cidr" | "macaddr" | "binary" | "char" | "varchar" => (Some("string"), Some("IsString()")),
        "uuid" => (Some("string"), Some("IsUUID()")),
        "integer" | "bigint" | "smallint" | "references" | "belongs_to" | "serial" | "bigserial" => (Some("int"), Some("IsInt()")),
        "float" | "decimal" | "numeric" | "real" | "double" | "money" => (Some("number"), Some("IsNumber()")),
        "boolean" => (Some("boolean"), Some("IsBoolean()")),
        "date" | "datetime" | "time" | "timestamp" | "timestamptz" => (Some("date"), Some("IsDateString()")),
        "json" | "jsonb" | "hstore" => (Some("object"), Some("IsObject()")),
        _ => (None, None),
    }
}

const SUTUN_TIPLERI: [&str; 26] = [
    "string",
    "text",
    "citext",
    "integer",
    "bigint",
    "smallint",
    "float",
    "decimal",
    "numeric",
    "boolean",
    "date",
    "datetime",
    "time",
    "timestamp",
    "timestamptz",
    "json",
    "jsonb",
    "uuid",
    "binary",
    "references",
    "belongs_to",
    "hstore",
    "inet",
    "money",
    "virtual",
    "column",
];

impl<'a> Proje<'a> {
    fn yeni(kok: &'a Path, dosyalar: &'a [Dosya]) -> Self {
        let mut siniflar: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        let mut sablonlar = HashMap::new();
        for (di, d) in dosyalar.iter().enumerate() {
            for (si, s) in d.siniflar.iter().enumerate() {
                siniflar.entry(anahtar(&s.ad)).or_default().push((di, si));
            }
            if let Some(g) = d.goreli.strip_prefix("app/views/").filter(|g| g.ends_with(".jbuilder")) {
                sablonlar.insert(g.to_string(), di);
            }
        }
        let gemler = guvenli::oku(&kok.join("Gemfile")).map(|g| gem_listesi(&g).into_iter().collect()).unwrap_or_default();
        let mut gorunumler = BTreeSet::new();
        let views = kok.join("app").join("views");
        if views.is_dir() {
            for g in ignore::WalkBuilder::new(&views).hidden(true).git_ignore(true).build().flatten() {
                if g.path().is_file() && !g.path_is_symlink() {
                    gorunumler.insert(guvenli::goreli(&views, g.path()));
                }
            }
        }
        let mut p = Proje { dosyalar, siniflar, gemler, tablolar: BTreeMap::new(), sema: None, modeller: BTreeMap::new(), gorunumler, sablonlar };
        p.sema_oku();
        p.modelleri_kur();
        p
    }

    fn sinif(&self, x: (usize, usize)) -> &'a Sinif {
        &self.dosyalar[x.0].siniflar[x.1]
    }

    /// Sınıfın bütün açılışları
    fn acilislar(&self, anahtar_: &str) -> &[(usize, usize)] {
        self.siniflar.get(anahtar_).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Sabit adını, başvuran sınıfın ad alanından dışarı doğru çözer: `BaseController` (Api::V1::PostsController içinde)
    /// -> api::v1::basecontroller. Dönen: sınıf anahtarı
    fn coz(&self, ad: &str, baglam: &str) -> Option<String> {
        let ad = ad.trim().trim_start_matches("::");
        if ad.is_empty() || !ad.starts_with(|c: char| c.is_uppercase()) {
            return None;
        }
        let parcalar: Vec<&str> = baglam.split("::").filter(|p| !p.is_empty()).collect();
        for i in (0..=parcalar.len()).rev() {
            let aday = if i == 0 { ad.to_string() } else { format!("{}::{ad}", parcalar[..i].join("::")) };
            let a = anahtar(&aday);
            if self.siniflar.contains_key(&a) {
                return Some(a);
            }
        }
        None
    }

    /// Sınıfın tam adı (ilk açılıştan)
    fn tam_ad(&self, a: &str) -> String {
        self.acilislar(a).first().map(|x| self.sinif(*x).ad.clone()).unwrap_or_default()
    }

    /// Üst sınıfın anahtarı (projede tanımlıysa) ve ham adı
    fn ust(&self, a: &str) -> (Option<String>, Option<String>) {
        for x in self.acilislar(a) {
            let s = self.sinif(*x);
            if let Some(u) = &s.ust {
                return (self.coz(u, &s.ad), Some(u.clone()));
            }
        }
        (None, None)
    }

    /// Ata zinciri (kendisi dahil, yapraktan köke) ve zincirin projede olmayan ilk atası (ör. "ActiveRecord::Base")
    fn zincir(&self, a: &str) -> (Vec<String>, Option<String>) {
        let mut v = vec![a.to_string()];
        let mut dis = None;
        while v.len() < 16 {
            let (u, ham) = self.ust(v.last().unwrap());
            match u {
                Some(u) if !v.contains(&u) => v.push(u),
                _ => {
                    dis = ham;
                    break;
                }
            }
        }
        (v, dis)
    }

    /// Zincirdeki herhangi bir sınıfın üstü ya da dahil ettiği modül hedeflerden biri mi (ham adla)
    fn soyundan(&self, a: &str, hedefler: &[&str]) -> bool {
        let (z, dis) = self.zincir(a);
        dis.as_deref().is_some_and(|d| hedefler.contains(&d.trim_start_matches("::")))
            || z.iter().any(|x| {
                self.acilislar(x).iter().any(|y| {
                    let s = self.sinif(*y);
                    s.ust.as_deref().is_some_and(|u| hedefler.contains(&u))
                        || s.cagrilar
                            .iter()
                            .any(|c| c.ad == "include" && c.alici.is_none() && c.args.iter().any(|a| matches!(a, Arg::Ham(h) if hedefler.contains(&h.trim_start_matches("::")))))
                })
            })
    }

    /// Sınıfın dahil ettiği proje modülleri (`include X`), kaynak sırasıyla
    fn dahil(&self, a: &str) -> Vec<String> {
        let mut v = vec![];
        for x in self.acilislar(a) {
            let s = self.sinif(*x);
            for c in s.cagrilar.iter().filter(|c| matches!(c.ad.as_str(), "include" | "prepend") && c.alici.is_none()) {
                for arg in &c.args {
                    if let Arg::Ham(h) = arg {
                        if let Some(m) = self.coz(h, &s.ad) {
                            if !v.contains(&m) {
                                v.push(m);
                            }
                        }
                    }
                }
            }
        }
        v
    }

    /// Metodu sınıfta, dahil ettiği modüllerde ve atalarında arar (Ruby'nin sırasıyla). Dönen: (dosya, sınıf anahtarı, metot)
    fn metot(&self, a: &str, ad: &str, sinif_metodu: bool) -> Option<(usize, String, &'a Metot)> {
        let (z, _) = self.zincir(a);
        for c in &z {
            if let Some(r) = self.kendi_metodu(c, ad, sinif_metodu, 0) {
                return Some(r);
            }
        }
        None
    }

    fn kendi_metodu(&self, a: &str, ad: &str, sinif_metodu: bool, derinlik: usize) -> Option<(usize, String, &'a Metot)> {
        for x in self.acilislar(a) {
            if let Some(m) = self.sinif(*x).metotlar.iter().find(|m| m.ad == ad && m.sinif_metodu == sinif_metodu) {
                return Some((x.0, a.to_string(), m));
            }
        }
        if derinlik < 4 {
            for m in self.dahil(a).iter().rev() {
                if let Some(r) = self.kendi_metodu(m, ad, sinif_metodu, derinlik + 1) {
                    return Some(r);
                }
            }
        }
        None
    }

    /// db/schema.rb: create_table blokları ve sütunları
    fn sema_oku(&mut self) {
        let Some(di) = self.dosyalar.iter().position(|d| d.goreli == "db/schema.rb") else { return };
        self.sema = Some(di);
        let mut yigin: Vec<&Cagri> = self.dosyalar[di].ust.iter().collect();
        while let Some(c) = yigin.pop() {
            if c.ad == "create_table" {
                let Some(ad) = c.args.first().and_then(Arg::ad) else { continue };
                let mut t = Tablo { ad: ad.to_string(), satir: c.satir, sutunlar: vec![] };
                if c.anahtar("id").is_none_or(|i| !matches!(i, Arg::Ham(h) if h == "false")) {
                    let tip = c.anahtar("id").and_then(Arg::ad).unwrap_or("bigint").to_string();
                    t.sutunlar.push(("id".into(), tip, true, c.satir));
                }
                for s in &c.blok {
                    if s.ad == "timestamps" {
                        t.sutunlar.push(("created_at".into(), "datetime".into(), true, s.satir));
                        t.sutunlar.push(("updated_at".into(), "datetime".into(), true, s.satir));
                        continue;
                    }
                    if !SUTUN_TIPLERI.contains(&s.ad.as_str()) {
                        continue;
                    }
                    let null_degil = matches!(s.anahtar("null"), Some(Arg::Ham(h)) if h == "false");
                    for a in s.args.iter().filter_map(Arg::ad) {
                        if matches!(s.ad.as_str(), "references" | "belongs_to") {
                            t.sutunlar.push((format!("{a}_id"), "bigint".into(), null_degil, s.satir));
                        } else {
                            t.sutunlar.push((a.to_string(), s.ad.clone(), null_degil, s.satir));
                        }
                    }
                }
                self.tablolar.insert(t.ad.clone(), t);
            } else {
                yigin.extend(c.blok.iter());
            }
        }
    }

    /// ActiveRecord modelleri: tablo adı, ilişkiler, doğrulamalar, enum'lar, ekler, Devise modülleri
    fn modelleri_kur(&mut self) {
        let mut anahtarlar: Vec<String> = self.siniflar.keys().cloned().collect();
        anahtarlar.sort();
        for a in anahtarlar {
            if !self.soyundan(&a, &["ActiveRecord::Base"]) {
                continue;
            }
            let acilislar = self.acilislar(&a).to_vec();
            let soyut = acilislar.iter().any(|x| {
                self.sinif(*x).cagrilar.iter().any(|c| {
                    c.ad == "primary_abstract_class"
                        || (c.ad == "=" && c.alici.as_deref() == Some("self.abstract_class") && matches!(c.args.first(), Some(Arg::Ham(h)) if h == "true"))
                })
            });
            if soyut {
                continue;
            }
            let tam = self.tam_ad(&a);
            let son = tam.rsplit("::").next().unwrap_or(&tam).to_string();
            let mut m = Model { ad: tam.clone(), sinif: acilislar[0], tablo: cogul(&underscore(&son)), ..Default::default() };
            for x in &acilislar {
                let s = self.sinif(*x);
                for c in &s.cagrilar {
                    self.model_cagrisi(&mut m, s, c);
                }
            }
            self.modeller.insert(a, m);
        }
    }

    fn model_cagrisi(&self, m: &mut Model, s: &Sinif, c: &Cagri) {
        let kosullu = c.anahtar("if").is_some() || c.anahtar("unless").is_some() || c.kosul.is_some();
        let olusturmada = c.anahtar("on").and_then(Arg::ad).is_none_or(|o| o != "update");
        match c.ad.as_str() {
            "=" if c.alici.as_deref() == Some("self.table_name") => {
                if let Some(t) = c.args.first().and_then(Arg::ad) {
                    m.tablo = t.to_string();
                    m.acik_tablo = true;
                }
            }
            "has_many" | "has_one" | "belongs_to" | "has_and_belongs_to_many" => {
                let Some(ad) = c.args.first().and_then(Arg::ad) else { return };
                let cogul_ = matches!(c.ad.as_str(), "has_many" | "has_and_belongs_to_many");
                let sinif = c.anahtar("class_name").and_then(Arg::ad).map(String::from).unwrap_or_else(|| if cogul_ { sinif_adi(ad) } else { camelize(ad) });
                let hedef = self.coz(&sinif, &s.ad).unwrap_or_else(|| anahtar(&sinif));
                m.iliskiler.insert(ad.to_string(), (hedef, cogul_));
                if c.ad == "belongs_to" && !matches!(c.anahtar("optional"), Some(Arg::Ham(h)) if h == "true") {
                    let d = Dogrulama { tur: "presence".into(), secenek: None, kosullu, olusturmada: true };
                    m.dogrulamalar.entry(format!("{ad}_id")).or_default().push(d);
                }
            }
            "devise" => m.devise = c.semboller(),
            "has_one_attached" | "has_many_attached" => m.ekler.extend(c.semboller()),
            "scope" => m.kapsamlar.extend(c.args.first().and_then(Arg::ad).map(String::from)),
            "enum" => {
                // enum :durum, { a: 0 } | enum :durum, [:a, :b] | enum durum: { a: 0 } | enum durum: [:a, :b]
                let degerler = |a: &Arg| -> Vec<String> {
                    match a {
                        Arg::Hash(v) => v.iter().filter_map(|(k, _)| k.ad().map(String::from)).collect(),
                        Arg::Dizi(_) => a.adlar(),
                        _ => vec![],
                    }
                };
                if let (Some(ad), Some(d)) = (c.args.first().and_then(Arg::ad), c.args.get(1)) {
                    m.enumlar.insert(ad.to_string(), degerler(d));
                }
                for (k, v) in &c.anahtarli {
                    if let Some(ad) = k.ad().filter(|a| !a.starts_with('_') && !matches!(*a, "prefix" | "suffix" | "default" | "scopes" | "validate" | "instance_methods")) {
                        let d = degerler(v);
                        if !d.is_empty() {
                            m.enumlar.insert(ad.to_string(), d);
                        }
                    }
                }
            }
            "validate" => m.ozel_dogrulamalar.extend(c.semboller()),
            "validates" | "validates!" => {
                for alan in c.semboller() {
                    for (k, v) in &c.anahtarli {
                        let Some(tur) = k.ad().filter(|t| !matches!(*t, "if" | "unless" | "on" | "allow_nil" | "allow_blank" | "strict" | "message")) else { continue };
                        if matches!(v, Arg::Ham(h) if h == "false") {
                            continue;
                        }
                        m.dogrulamalar.entry(alan.clone()).or_default().push(Dogrulama { tur: tur.to_string(), secenek: Some(v.clone()), kosullu, olusturmada });
                    }
                }
            }
            ad if ad.starts_with("validates_") && ad.ends_with("_of") => {
                let tur = ad.trim_start_matches("validates_").trim_end_matches("_of").to_string();
                let secenek = Arg::Hash(c.anahtarli.clone());
                for alan in c.semboller() {
                    m.dogrulamalar.entry(alan).or_default().push(Dogrulama { tur: tur.clone(), secenek: Some(secenek.clone()), kosullu, olusturmada });
                }
            }
            _ => {}
        }
    }

    /// Model adından (kısa ya da tam) model; baglam: başvuran sınıf
    fn model(&self, ad: &str, baglam: &str) -> Option<&Model> {
        let a = self.coz(ad, baglam)?;
        self.modeller.get(&a)
    }

    /// Kullanıcı modeli (Devise kapsamı `user` ya da `User` sınıfı)
    fn kullanici_modeli(&self) -> Option<String> {
        self.modeller.iter().find(|(_, m)| !m.devise.is_empty()).map(|(a, _)| a.clone()).or_else(|| self.modeller.contains_key("user").then(|| "user".to_string()))
    }

    fn tablo(&self, m: &Model) -> Option<&Tablo> {
        self.tablolar.get(&m.tablo)
    }

    /// Kaskad silmeler: `has_many / has_one ... dependent: :destroy | :delete_all | :destroy_async` ve şemadaki
    /// `add_foreign_key ..., on_delete: :cascade`. Dönen: [ebeveyn modeli, çocuk modeli, "dosya:satır"]
    fn kaskadlar(&self) -> Vec<Value> {
        let mut v: Vec<(String, String, String)> = vec![];
        for m in self.modeller.values() {
            for x in self.acilislar(&anahtar(&m.ad)) {
                let s = self.sinif(*x);
                for c in s.cagrilar.iter().filter(|c| matches!(c.ad.as_str(), "has_many" | "has_one")) {
                    let bagimli = c.anahtar("dependent").and_then(Arg::ad).unwrap_or("");
                    if !matches!(bagimli, "destroy" | "delete_all" | "destroy_async" | "delete") || c.anahtar("through").is_some() {
                        continue;
                    }
                    let Some(ad) = c.args.first().and_then(Arg::ad) else { continue };
                    let Some((hedef, _)) = m.iliskiler.get(ad) else { continue };
                    let cocuk = self.modeller.get(hedef).map(|h| h.ad.clone()).unwrap_or_else(|| sinif_adi(ad));
                    v.push((m.ad.clone(), cocuk, format!("{}:{}", self.dosyalar[x.0].goreli, c.satir)));
                }
            }
        }
        // Şemadaki yabancı anahtarlar (tablo adları modellere çevrilir)
        if let Some(di) = self.sema {
            let tablo_modeli = |t: &str| self.modeller.values().find(|m| m.tablo == t).map(|m| m.ad.clone()).unwrap_or_else(|| sinif_adi(t));
            let mut yigin: Vec<&Cagri> = self.dosyalar[di].ust.iter().collect();
            while let Some(c) = yigin.pop() {
                yigin.extend(c.blok.iter());
                if c.ad != "add_foreign_key" || c.anahtar("on_delete").and_then(Arg::ad) != Some("cascade") {
                    continue;
                }
                if let (Some(cocuk), Some(eb)) = (c.args.first().and_then(Arg::ad), c.args.get(1).and_then(Arg::ad)) {
                    let (e, cc) = (tablo_modeli(eb), tablo_modeli(cocuk));
                    if !v.iter().any(|(a, b, _)| a.eq_ignore_ascii_case(&e) && b.eq_ignore_ascii_case(&cc)) {
                        v.push((e, cc, format!("{}:{}", self.dosyalar[di].goreli, c.satir)));
                    }
                }
            }
        }
        v.into_iter().map(|(a, b, c)| json!([a, b, c])).collect()
    }

    /// Tablo modelleri (kanıt deposu için): ad, gerçek tablo adı, şemadaki tanımı ve sütunları
    fn veri_modelleri(&self) -> Value {
        let mut v = vec![];
        for m in self.modeller.values() {
            let (dosya, satir, alanlar) = match (self.tablo(m), self.sema) {
                (Some(t), Some(di)) => {
                    (self.dosyalar[di].goreli.clone(), t.satir, t.sutunlar.iter().map(|(a, tip, _, s)| json!({ "ad": a, "tip": tip, "satir": s })).collect::<Vec<_>>())
                }
                _ => (self.dosyalar[m.sinif.0].goreli.clone(), self.sinif(m.sinif).satir, vec![]),
            };
            v.push(json!({
                "ad": m.ad, "tablo": m.tablo,
                "tablo_kaynagi": if m.acik_tablo { "self.table_name" } else { "varsayılan (sınıf adının çoğulu)" },
                "dosya": dosya, "satir": satir, "alanlar": alanlar,
            }));
        }
        json!(v)
    }
}

// ---- Rotalar (config/routes.rb) ----

/// Kaynak bağlamı (`resources :posts do ... end` içi)
#[derive(Clone, Debug)]
struct Kaynak {
    /// Denetleyici yolu: "api/v1/posts"
    kontrolcu: String,
    /// Koleksiyon yolu: "/api/v1/posts"
    koleksiyon: String,
    /// Üye yolu: "/api/v1/posts/:id" (tekil kaynakta koleksiyonla aynı; shallow'da üst kaynak olmadan)
    uye: String,
    /// İç içe kaynakların öneki: "/api/v1/posts/:post_id"
    ic: String,
}

/// Rota kapsamı: yol öneki, shallow öneki, denetleyici modülü, koşullar, kapsam düğümü
#[derive(Clone, Debug, Default)]
struct Kapsam {
    yol: String,
    /// Shallow üye rotalarının öneki: üst kaynaklar olmadan (namespace / scope önekleri)
    sig: String,
    modul: String,
    kontrolcu: Option<String>,
    kosul: Vec<String>,
    dugum: usize,
    shallow: bool,
    kaynak: Option<Kaynak>,
    /// member (true) / collection (false) bloğu içinde
    uye: Option<bool>,
    /// Rotaların bulunduğu dosya
    di: usize,
    /// Devise kapsamı (`devise_scope :user do ... end`)
    devise: bool,
}

#[derive(Clone, Debug)]
struct Rota {
    yontem: String,
    yol: String,
    /// Denetleyici yolu ("api/v1/posts") ve eylem
    kontrolcu: String,
    eylem: String,
    di: usize,
    satir: usize,
    kosul: Vec<String>,
    dugum: usize,
    /// Devise'ın ürettiği rota: kullanıcı modeli kapsamı
    devise: Option<String>,
}

/// Rota kapsam düğümü (namespace / scope / constraints / draw): bağımlılık grafiğinde `mod:` düğümü
struct Dugum {
    etiket: String,
    di: usize,
    satir: usize,
    ust: Option<usize>,
    kosul: Option<String>,
}

#[derive(Default)]
struct Rotalar {
    rotalar: Vec<Rota>,
    dugumler: Vec<Dugum>,
    konsernler: HashMap<String, Vec<Cagri>>,
    /// (hedef, yol, "dosya:satır"): mount edilen motorlar (işlem değildir)
    monteler: Vec<(String, String, String)>,
    /// Çözülemeyen rota tanımları ("dosya:satır metin")
    cozulemeyen: Vec<String>,
}

/// Yol birleştirme: isteğe bağlı parçalar (`(.:format)`, `(:locale)`) atılır, `*glob` `:glob` olur
fn yol_ekle(a: &str, b: &str) -> String {
    static D: OnceLock<Regex> = OnceLock::new();
    let d = D.get_or_init(|| Regex::new(r"\([^()]*\)").unwrap());
    let mut b = b.to_string();
    while d.is_match(&b) {
        b = d.replace_all(&b, "").to_string();
    }
    let b = b.trim_matches('/');
    let b: Vec<String> = b.split('/').filter(|p| !p.is_empty()).map(|p| if let Some(g) = p.strip_prefix('*') { format!(":{g}") } else { p.to_string() }).collect();
    let a = a.trim_end_matches('/');
    if b.is_empty() {
        return a.to_string();
    }
    format!("{a}/{}", b.join("/"))
}

fn modul_ekle(a: &str, b: &str) -> String {
    let b = b.trim_matches('/');
    match (a.is_empty(), b.is_empty()) {
        (_, true) => a.to_string(),
        (true, false) => b.to_string(),
        _ => format!("{a}/{b}"),
    }
}

/// `only` / `except` süzgeci
fn secili(c: &Cagri, eylem: &str) -> bool {
    if let Some(o) = c.anahtar("only") {
        return o.adlar().iter().any(|x| x == eylem);
    }
    if let Some(e) = c.anahtar("except") {
        return !e.adlar().iter().any(|x| x == eylem);
    }
    true
}

/// Kısıtların metni: `subdomain: "admin"`, `AdminKisiti.new`
fn kisit_metni(c: &Cagri) -> Option<String> {
    let mut p: Vec<String> = c.args.iter().map(Arg::metin).collect();
    p.extend(c.anahtarli.iter().map(|(k, v)| format!("{}: {}", k.ad().unwrap_or("?"), v.metin())));
    let m = p.join(", ");
    (!m.is_empty()).then_some(m)
}

const FIILLER: [&str; 8] = ["get", "post", "put", "patch", "delete", "options", "head", "match"];

impl Rotalar {
    fn dugum(&mut self, etiket: String, ks: &Kapsam, satir: usize, kosul: Option<String>) -> usize {
        self.dugumler.push(Dugum { etiket, di: ks.di, satir, ust: Some(ks.dugum), kosul });
        self.dugumler.len() - 1
    }

    #[allow(clippy::too_many_arguments)]
    fn ekle(&mut self, ks: &Kapsam, yontemler: &[String], yol: String, kontrolcu: String, eylem: String, satir: usize, ek_kosul: Option<String>) {
        let mut kosul = ks.kosul.clone();
        kosul.extend(ek_kosul);
        let yol = if yol.is_empty() { "/".to_string() } else { yol };
        for y in yontemler {
            self.rotalar.push(Rota {
                yontem: y.clone(),
                yol: yol.clone(),
                kontrolcu: kontrolcu.clone(),
                eylem: eylem.clone(),
                di: ks.di,
                satir,
                kosul: kosul.clone(),
                dugum: ks.dugum,
                devise: None,
            });
        }
    }

    fn gez(&mut self, p: &Proje, cagrilar: &[Cagri], ks: &Kapsam, derinlik: usize) {
        if derinlik > 24 {
            return;
        }
        for c in cagrilar {
            let mut ks = ks.clone();
            if let Some(k) = &c.kosul {
                ks.kosul.push(k.clone());
            }
            let ilk = c.args.first().and_then(Arg::ad).map(String::from);
            match c.ad.as_str() {
                "draw" if c.alici.is_some() => self.gez(p, &c.blok, &ks, derinlik + 1),
                "draw" => {
                    // draw :admin -> config/routes/admin.rb (aynı kapsamda)
                    let Some(ad) = ilk else { continue };
                    let hedef = format!("config/routes/{ad}.rb");
                    if let Some(di) = p.dosyalar.iter().position(|d| d.goreli == hedef) {
                        let mut ks2 = ks.clone();
                        ks2.di = di;
                        ks2.dugum = self.dugum(format!("draw {ad}"), &ks, c.satir, None);
                        self.gez(p, &p.dosyalar[di].ust, &ks2, derinlik + 1);
                    } else {
                        self.cozulemeyen.push(format!("{}:{} {} (hedef çözülemedi)", p.dosyalar[ks.di].goreli, c.satir, c.metin));
                    }
                }
                "namespace" => {
                    let Some(ad) = ilk else { continue };
                    let yol = c.anahtar("path").and_then(Arg::ad).map(String::from).unwrap_or_else(|| ad.clone());
                    let modul = c.anahtar("module").and_then(Arg::ad).map(String::from).unwrap_or_else(|| ad.clone());
                    let mut ks2 = ks.clone();
                    ks2.yol = yol_ekle(&ks.yol, &yol);
                    ks2.sig = yol_ekle(&ks.sig, &yol);
                    ks2.modul = modul_ekle(&ks.modul, &modul);
                    ks2.kaynak = None;
                    ks2.uye = None;
                    ks2.dugum = self.dugum(format!("namespace {}", ks2.modul), &ks, c.satir, None);
                    self.gez(p, &c.blok, &ks2, derinlik + 1);
                }
                "scope" => {
                    let yol = c.args.first().and_then(Arg::ad).or_else(|| c.anahtar("path").and_then(Arg::ad)).unwrap_or("").to_string();
                    let mut ks2 = ks.clone();
                    ks2.yol = yol_ekle(&ks.yol, &yol);
                    ks2.sig = yol_ekle(&ks.sig, &yol);
                    if let Some(m) = c.anahtar("module").and_then(Arg::ad) {
                        ks2.modul = modul_ekle(&ks.modul, m);
                    }
                    if let Some(k) = c.anahtar("controller").and_then(Arg::ad) {
                        ks2.kontrolcu = Some(k.to_string());
                    }
                    let kisit = c.anahtar("constraints").map(Arg::metin);
                    if let Some(k) = &kisit {
                        ks2.kosul.push(k.clone());
                    }
                    let etiket = if yol.is_empty() { format!("scope {}", ks2.modul) } else { format!("scope {}", ks2.yol) };
                    ks2.dugum = self.dugum(etiket, &ks, c.satir, kisit);
                    self.gez(p, &c.blok, &ks2, derinlik + 1);
                }
                "controller" => {
                    let mut ks2 = ks.clone();
                    ks2.kontrolcu = ilk.clone();
                    self.gez(p, &c.blok, &ks2, derinlik + 1);
                }
                "constraints" => {
                    let mut ks2 = ks.clone();
                    let kisit = kisit_metni(c).unwrap_or_else(|| "constraints".into());
                    ks2.kosul.push(kisit.clone());
                    ks2.dugum = self.dugum(format!("constraints {kisit}"), &ks, c.satir, Some(kisit));
                    self.gez(p, &c.blok, &ks2, derinlik + 1);
                }
                "defaults" | "with_options" | "as" | "localized" => self.gez(p, &c.blok, &ks, derinlik + 1),
                "devise_scope" => {
                    let mut ks2 = ks.clone();
                    ks2.devise = true;
                    self.gez(p, &c.blok, &ks2, derinlik + 1);
                }
                "shallow" => {
                    let mut ks2 = ks.clone();
                    ks2.shallow = true;
                    self.gez(p, &c.blok, &ks2, derinlik + 1);
                }
                "concern" => {
                    if let Some(ad) = ilk {
                        self.konsernler.insert(ad, c.blok.clone());
                    }
                }
                "concerns" => {
                    for ad in c.semboller() {
                        if let Some(b) = self.konsernler.get(&ad).cloned() {
                            self.gez(p, &b, &ks, derinlik + 1);
                        }
                    }
                }
                "resources" | "resource" => {
                    // Değişkenle verilen kaynak adı (dinamik kayıt) kapıya çevrilemez: bildirilir
                    if c.args.iter().any(|a| matches!(a, Arg::Ham(_))) {
                        self.cozulemeyen.push(format!("{}:{} {} (dinamik kaynak adı)", p.dosyalar[ks.di].goreli, c.satir, c.metin));
                    }
                    for ad in c.args.iter().filter_map(Arg::ad).map(String::from).collect::<Vec<_>>() {
                        self.kaynak(p, c, &ad, c.ad == "resource", &ks, derinlik);
                    }
                }
                "member" | "collection" if ks.kaynak.is_some() => {
                    let mut ks2 = ks.clone();
                    ks2.uye = Some(c.ad == "member");
                    self.gez(p, &c.blok, &ks2, derinlik + 1);
                }
                "root" => self.fiil(p, c, &ks),
                f if FIILLER.contains(&f) && c.alici.is_none() => self.fiil(p, c, &ks),
                "devise_for" => self.devise_for(p, c, &ks),
                "mount" => {
                    let (hedef, yol) = match (c.args.first(), c.anahtarli.first()) {
                        (Some(a), _) => (a.metin(), c.anahtar("at").and_then(Arg::ad).unwrap_or("").to_string()),
                        (None, Some((k, v))) => (k.metin(), v.ad().unwrap_or("").to_string()),
                        _ => continue,
                    };
                    self.monteler.push((hedef, yol_ekle(&ks.yol, &yol), format!("{}:{}", p.dosyalar[ks.di].goreli, c.satir)));
                }
                // Devise oturum kısıtları (`authenticate :user do ... end`) ve tanınmayan bloklu çağrılar (döngü,
                // yardımcı): içindeki rotalar koşullu okunur, çözülemeyenler bildirilir
                _ if c.blok_var && !c.blok.is_empty() => {
                    let mut ks2 = ks.clone();
                    ks2.kosul.push(c.metin.split(" do").next().unwrap_or(&c.metin).to_string());
                    self.gez(p, &c.blok, &ks2, derinlik + 1);
                }
                _ => {}
            }
        }
    }

    /// `resources :posts` / `resource :profile`: yedi (tekilde altı) eylem, iç içe kaynaklar, member / collection
    fn kaynak(&mut self, p: &Proje, c: &Cagri, ad: &str, tekil_: bool, ks: &Kapsam, derinlik: usize) {
        let yol_parca = c.anahtar("path").and_then(Arg::ad).unwrap_or(ad).to_string();
        let modul = match c.anahtar("module").and_then(Arg::ad) {
            Some(m) => modul_ekle(&ks.modul, m),
            None => ks.modul.clone(),
        };
        let kontrolcu_adi = c.anahtar("controller").and_then(Arg::ad).map(String::from).unwrap_or_else(|| if tekil_ { cogul(ad) } else { ad.to_string() });
        let kontrolcu = modul_ekle(&modul, &kontrolcu_adi);
        let param = c.anahtar("param").and_then(Arg::ad).unwrap_or("id").to_string();
        let ic_ice = ks.kaynak.is_some();
        // Üst kaynak varsa onun iç öneki (member / collection içinde de)
        let taban = match (&ks.kaynak, ks.uye) {
            (Some(k), Some(true)) => k.uye.clone(),
            (Some(k), Some(false)) => k.koleksiyon.clone(),
            (Some(k), None) => k.ic.clone(),
            (None, _) => ks.yol.clone(),
        };
        let shallow = ks.shallow || matches!(c.anahtar("shallow"), Some(Arg::Ham(h)) if h == "true");
        let koleksiyon = yol_ekle(&taban, &yol_parca);
        let uye_tam = if tekil_ { koleksiyon.clone() } else { yol_ekle(&koleksiyon, &format!(":{param}")) };
        let uye = if shallow && ic_ice && !tekil_ { yol_ekle(&yol_ekle(&ks.sig, &yol_parca), &format!(":{param}")) } else { uye_tam.clone() };
        let ad_yeni = c.anahtar("path_names").and_then(|x| x.anahtar("new")).and_then(Arg::ad).unwrap_or("new").to_string();
        let ad_duzenle = c.anahtar("path_names").and_then(|x| x.anahtar("edit")).and_then(Arg::ad).unwrap_or("edit").to_string();
        let ek_kosul = c.anahtar("constraints").map(Arg::metin);
        let eylemler: Vec<(&str, Vec<&str>, String)> = if tekil_ {
            vec![
                ("create", vec!["POST"], koleksiyon.clone()),
                ("new", vec!["GET"], yol_ekle(&koleksiyon, &ad_yeni)),
                ("edit", vec!["GET"], yol_ekle(&koleksiyon, &ad_duzenle)),
                ("show", vec!["GET"], koleksiyon.clone()),
                ("update", vec!["PATCH", "PUT"], koleksiyon.clone()),
                ("destroy", vec!["DELETE"], koleksiyon.clone()),
            ]
        } else {
            vec![
                ("index", vec!["GET"], koleksiyon.clone()),
                ("create", vec!["POST"], koleksiyon.clone()),
                ("new", vec!["GET"], yol_ekle(&koleksiyon, &ad_yeni)),
                ("edit", vec!["GET"], yol_ekle(&uye, &ad_duzenle)),
                ("show", vec!["GET"], uye.clone()),
                ("update", vec!["PATCH", "PUT"], uye.clone()),
                ("destroy", vec!["DELETE"], uye.clone()),
            ]
        };
        for (eylem, yontemler, yol) in eylemler {
            if secili(c, eylem) {
                let y: Vec<String> = yontemler.iter().map(|x| x.to_string()).collect();
                self.ekle(ks, &y, yol, kontrolcu.clone(), eylem.to_string(), c.satir, ek_kosul.clone());
            }
        }
        // İç içe kaynakların öneki: /posts/:post_id (param: :slug ise :post_slug); tekil kaynakta kaynağın kendi yolu
        let ic = if tekil_ { koleksiyon.clone() } else { yol_ekle(&koleksiyon, &format!(":{}_{param}", tekil(ad))) };
        let mut ks2 = ks.clone();
        ks2.kaynak = Some(Kaynak { kontrolcu, koleksiyon, uye, ic });
        ks2.uye = None;
        ks2.modul = modul;
        ks2.shallow = shallow;
        if let Some(k) = ek_kosul {
            ks2.kosul.push(k);
        }
        for k in c.anahtar("concerns").map(Arg::adlar).unwrap_or_default() {
            if let Some(b) = self.konsernler.get(&k).cloned() {
                self.gez(p, &b, &ks2, derinlik + 1);
            }
        }
        self.gez(p, &c.blok, &ks2, derinlik + 1);
    }

    /// Fiil rotası: `get 'yol', to: 'c#a'`, `get 'yol' => 'c#a'`, `match 'yol', via: [...]`, `root 'c#a'`, kaynak
    /// içinde `get :preview` (member / collection / iç içe)
    fn fiil(&mut self, p: &Proje, c: &Cagri, ks: &Kapsam) {
        let yontemler: Vec<String> = match c.ad.as_str() {
            "root" => vec!["GET".into()],
            "match" => {
                let via = c.anahtar("via").map(Arg::adlar).unwrap_or_default();
                if via.iter().any(|v| v == "all") {
                    ["GET", "POST", "PUT", "PATCH", "DELETE"].iter().map(|x| x.to_string()).collect()
                } else {
                    via.iter().map(|v| v.to_uppercase()).collect()
                }
            }
            f => vec![f.to_uppercase()],
        };
        if yontemler.is_empty() {
            self.cozulemeyen.push(format!("{}:{} {} (hedef çözülemedi)", p.dosyalar[ks.di].goreli, c.satir, c.metin));
            return;
        }
        // Yol ve hedef: `'yol' => 'c#a'` (anahtarı metin olan çift), ilk argüman, `to:`
        let ok = c.anahtarli.iter().find(|(k, _)| matches!(k, Arg::Metin(_)));
        let (mut yol, mut hedef): (Option<String>, Option<Arg>) = match ok {
            Some((Arg::Metin(y), h)) => (Some(y.clone()), Some(h.clone())),
            _ => (c.args.first().and_then(Arg::ad).map(String::from), c.anahtar("to").cloned()),
        };
        if c.ad == "root" {
            if let Some(Arg::Metin(h)) = c.args.first() {
                hedef = Some(Arg::Metin(h.clone()));
            }
            yol = None;
        }
        let yonlendirme = matches!(&hedef, Some(Arg::Ham(h)) if h.starts_with("redirect"));
        if yonlendirme {
            return;
        }
        let (mut kontrolcu, mut eylem): (Option<String>, Option<String>) = (None, None);
        match &hedef {
            Some(Arg::Metin(h)) if h.contains('#') => {
                let (k, e) = h.split_once('#').unwrap();
                kontrolcu = Some(modul_ekle(&ks.modul, k));
                eylem = Some(e.to_string());
            }
            Some(Arg::Metin(h)) | Some(Arg::Sembol(h)) => eylem = Some(h.clone()),
            Some(_) => {
                // Rack uygulaması ya da lambda: işlem değil
                self.cozulemeyen.push(format!("{}:{} {} (hedef çözülemedi)", p.dosyalar[ks.di].goreli, c.satir, c.metin));
                return;
            }
            None => {}
        }
        if let Some(k) = c.anahtar("controller").and_then(Arg::ad) {
            kontrolcu = Some(modul_ekle(&ks.modul, k));
        }
        if let Some(e) = c.anahtar("action").and_then(Arg::ad) {
            eylem = Some(e.to_string());
        }
        let yol_m = yol.clone().unwrap_or_default();
        // Yolun ilk durağan parçası eylem adı olur (`get :preview`, `get 'archive/:year'`)
        let durgun: Vec<&str> = yol_m.trim_matches('/').split('/').filter(|x| !x.is_empty() && !x.starts_with([':', '*', '('])).collect();
        let on = c.anahtar("on").and_then(Arg::ad).map(|o| o == "member").or(ks.uye);
        let tam;
        match &ks.kaynak {
            Some(kn) => {
                let taban = match on {
                    Some(true) => &kn.uye,
                    Some(false) => &kn.koleksiyon,
                    None => &kn.ic,
                };
                tam = yol_ekle(taban, &yol_m);
                if kontrolcu.is_none() {
                    kontrolcu = Some(kn.kontrolcu.clone());
                }
                if eylem.is_none() {
                    eylem = durgun.first().map(|x| x.replace(['-', '.'], "_"));
                }
            }
            None => {
                tam = if c.ad == "root" { ks.yol.clone() } else { yol_ekle(&ks.yol, &yol_m) };
                if kontrolcu.is_none() {
                    if let Some(k) = &ks.kontrolcu {
                        kontrolcu = Some(modul_ekle(&ks.modul, k));
                        if eylem.is_none() {
                            eylem = durgun.last().map(|x| x.replace(['-', '.'], "_"));
                        }
                    } else if durgun.len() >= 2 && eylem.is_none() {
                        // get 'photos/search' -> photos#search
                        kontrolcu = Some(modul_ekle(&ks.modul, &durgun[..durgun.len() - 1].join("/")));
                        eylem = durgun.last().map(|x| x.replace(['-', '.'], "_"));
                    }
                }
            }
        }
        let (Some(kontrolcu), Some(eylem)) = (kontrolcu, eylem) else {
            self.cozulemeyen.push(format!("{}:{} {} (hedef çözülemedi)", p.dosyalar[ks.di].goreli, c.satir, c.metin));
            return;
        };
        let ek = c.anahtar("constraints").map(Arg::metin);
        self.ekle(ks, &yontemler, tam, kontrolcu, eylem, c.satir, ek);
    }

    /// `devise_for :users`: modelin `devise` modüllerine göre Devise uçları; `controllers:` ile ezilen denetleyiciler,
    /// `path:`, `skip:` / `only:`
    fn devise_for(&mut self, p: &Proje, c: &Cagri, ks: &Kapsam) {
        for kapsam in c.args.iter().filter_map(Arg::ad).map(String::from).collect::<Vec<_>>() {
            let sinif = c.anahtar("class_name").and_then(Arg::ad).map(String::from).unwrap_or_else(|| sinif_adi(&kapsam));
            let moduller = p.model(&sinif, "").map(|m| m.devise.clone()).unwrap_or_default();
            if moduller.is_empty() {
                self.cozulemeyen.push(format!("{}:{} {} (hedef çözülemedi)", p.dosyalar[ks.di].goreli, c.satir, c.metin));
                continue;
            }
            let taban = yol_ekle(&ks.yol, c.anahtar("path").and_then(Arg::ad).unwrap_or(&kapsam));
            let atla = c.anahtar("skip").map(Arg::adlar).unwrap_or_default();
            let yalniz = c.anahtar("only").map(Arg::adlar);
            let kontrolculer = c.anahtar("controllers");
            let var = |m: &str| moduller.iter().any(|x| x == m);
            // path_names: { sign_in: :login, registration: "kayit" }
            let ad = |k: &str| c.anahtar("path_names").and_then(|x| x.anahtar(k)).and_then(Arg::ad).unwrap_or(k).to_string();
            let kayit = yol_ekle(&taban, c.anahtar("path_names").and_then(|x| x.anahtar("registration")).and_then(Arg::ad).unwrap_or(""));
            // (modül, denetleyici, eylem, yöntem, yol)
            let mut v: Vec<(&str, &str, &str, &str, String)> = vec![];
            if var("database_authenticatable") {
                v.push(("sessions", "sessions", "new", "GET", yol_ekle(&taban, &ad("sign_in"))));
                v.push(("sessions", "sessions", "create", "POST", yol_ekle(&taban, &ad("sign_in"))));
                v.push(("sessions", "sessions", "destroy", "DELETE", yol_ekle(&taban, &ad("sign_out"))));
            }
            if var("recoverable") {
                let y = yol_ekle(&taban, &ad("password"));
                v.push(("passwords", "passwords", "new", "GET", yol_ekle(&y, "new")));
                v.push(("passwords", "passwords", "edit", "GET", yol_ekle(&y, "edit")));
                v.push(("passwords", "passwords", "update", "PATCH", y.clone()));
                v.push(("passwords", "passwords", "update", "PUT", y.clone()));
                v.push(("passwords", "passwords", "create", "POST", y));
            }
            if var("registerable") {
                v.push(("registrations", "registrations", "cancel", "GET", yol_ekle(&kayit, &ad("cancel"))));
                v.push(("registrations", "registrations", "new", "GET", yol_ekle(&kayit, &ad("sign_up"))));
                v.push(("registrations", "registrations", "edit", "GET", yol_ekle(&kayit, &ad("edit"))));
                v.push(("registrations", "registrations", "update", "PATCH", kayit.clone()));
                v.push(("registrations", "registrations", "update", "PUT", kayit.clone()));
                v.push(("registrations", "registrations", "destroy", "DELETE", kayit.clone()));
                v.push(("registrations", "registrations", "create", "POST", kayit.clone()));
            }
            if var("confirmable") {
                let y = yol_ekle(&taban, &ad("confirmation"));
                v.push(("confirmations", "confirmations", "new", "GET", yol_ekle(&y, "new")));
                v.push(("confirmations", "confirmations", "show", "GET", y.clone()));
                v.push(("confirmations", "confirmations", "create", "POST", y));
            }
            if var("lockable") {
                let y = yol_ekle(&taban, &ad("unlock"));
                v.push(("unlocks", "unlocks", "new", "GET", yol_ekle(&y, "new")));
                v.push(("unlocks", "unlocks", "show", "GET", y.clone()));
                v.push(("unlocks", "unlocks", "create", "POST", y));
            }
            for (modul, kontrolcu, eylem, yontem, yol) in v {
                if atla.iter().any(|a| a == modul) || yalniz.as_ref().is_some_and(|y| !y.iter().any(|a| a == modul)) {
                    continue;
                }
                let k = kontrolculer.and_then(|x| x.anahtar(modul)).and_then(Arg::ad).map(String::from).unwrap_or_else(|| format!("devise/{kontrolcu}"));
                self.rotalar.push(Rota {
                    yontem: yontem.into(),
                    yol,
                    kontrolcu: k,
                    eylem: eylem.into(),
                    di: ks.di,
                    satir: c.satir,
                    kosul: ks.kosul.clone(),
                    dugum: ks.dugum,
                    devise: Some(sinif.clone()),
                });
            }
        }
    }
}

/// Uygulamanın rotaları: config/routes.rb'deki `Rails.application.routes.draw` bloğu
fn rotalari_oku(p: &Proje) -> Rotalar {
    let mut r = Rotalar::default();
    let Some(di) = p.dosyalar.iter().position(|d| d.goreli == "config/routes.rb") else { return r };
    r.dugumler.push(Dugum { etiket: "routes".into(), di, satir: 1, ust: None, kosul: None });
    let ks = Kapsam { di, ..Default::default() };
    r.gez(p, &p.dosyalar[di].ust, &ks, 0);
    r
}

// ---- Denetleyiciler: filtreler, giriş ve rol ----

/// Eyleme uygulanan bir filtre (`before_action :ad`)
#[derive(Clone, Debug)]
struct Filtre {
    ad: String,
    /// `if:` / `unless:` ile: her istekte çalışmayabilir
    kosullu: bool,
}

const FILTRE: [&str; 7] = ["before_action", "prepend_before_action", "append_before_action", "before_filter", "around_action", "prepend_around_action", "append_around_action"];
const FILTRE_ATLA: [&str; 4] = ["skip_before_action", "skip_before_filter", "skip_around_action", "skip_action_callback"];

/// Bir filtrenin (ya da zincirinin) izlenmesinin sonucu
#[derive(Default, Clone, Debug)]
struct Iz {
    /// Tanımı izlenen bir kimlik doğrulamaya ulaştı (kesin)
    giris: bool,
    /// Rol denetimi (giriş gerektirdiği çıkarımdır)
    roller: Vec<String>,
    /// Tanımı projede olmayan, adı kimlik doğrulamayı andıran filtre
    bilinmeyen: bool,
    /// Kimlik yalnızca bir koşulda isteniyor (giriş isteğe bağlı)
    istege: bool,
    /// Kimlik doğrulama / rol denetimi mi (guard olarak listelenir)
    guard: bool,
}

impl Iz {
    fn birlestir(&mut self, o: Iz) {
        self.giris |= o.giris;
        self.istege |= o.istege;
        self.bilinmeyen |= o.bilinmeyen;
        self.guard |= o.guard;
        for r in o.roller {
            if !self.roller.contains(&r) {
                self.roller.push(r);
            }
        }
    }
}

fn desenler() -> &'static [Regex; 4] {
    static D: OnceLock<[Regex; 4]> = OnceLock::new();
    D.get_or_init(|| {
        [
            // Kimlik doğrulayan çağrılar ve 401 yanıtı
            Regex::new(
                r"authenticate_or_request_with_http_(?:token|basic|digest)|request_http_(?:token|basic|digest)_authentication|warden\.authenticate!|\bauthenticate_[a-z_]+!|doorkeeper_authorize!|head\s*\(?\s*(?::unauthorized|401)\b|status:\s*(?::unauthorized|401)\b",
            )
            .unwrap(),
            // Giriş sayfasına yönlendirme
            Regex::new(r#"redirect_to\s*\(?\s*(?:main_app\.)?(?:new_\w*session\w*_(?:path|url)|\w*(?:login|log_in|sign_in|signin)\w*_(?:path|url)|["'][^"']*/(?:login|sign_in|signin|sessions?)(?:/new)?["'])"#).unwrap(),
            // Kimlik doğrulamayı andıran filtre adları (tanımı izlenemediğinde)
            Regex::new(r"(?i)auth|login|logged|signed_in|session|token|admin|permission|authoriz|require_user|current_user|access").unwrap(),
            // Kullanıcının yokluğunu sınayan koşul: `!(@user)`, `!(current_user)`, `!(user_signed_in?)`, `current_user.nil?`
            Regex::new(r"(?i)^!\(\s*(?:@?current_user|@user|user|Current\.user|Current\.session|@?current_user\.present\?|user_signed_in\?|signed_in\?|logged_in\?|authenticated\?|session\[:\w*user\w*\])\s*\)$|^(?:@?current_user|@user|Current\.user)\.(?:nil|blank)\?$").unwrap(),
        ]
    })
}

/// Metindeki rol yüklemleri: `current_user.admin?`, `user.has_role?(:editor)`, `user.role == "admin"`
fn rol_yuklemleri(t: &str, izinli: &HashSet<String>) -> Vec<String> {
    static D: OnceLock<[Regex; 5]> = OnceLock::new();
    let d = D.get_or_init(|| {
        [
            Regex::new(r"(?:current_user|\buser|@user|@current_user|Current\.user)\s*&?\.\s*(\w+)\?").unwrap(),
            Regex::new(r#"has_(?:any_)?role\?\s*\(?\s*:?["']?(\w+)"#).unwrap(),
            Regex::new(r#"\.role\s*(?:==|===|\.eql\?)\s*\(?\s*:?["'](\w+)"#).unwrap(),
            Regex::new(r#"\.role\?\s*\(?\s*:?["']?(\w+)"#).unwrap(),
            Regex::new(r#"\.roles?\.(?:include\?|exists\?|where)\s*\(?\s*(?:name:\s*)?:?["']?(\w+)"#).unwrap(),
        ]
    });
    static ROL: OnceLock<Regex> = OnceLock::new();
    let rol = ROL.get_or_init(|| {
        Regex::new(r"(?i)^(?:\w*admin\w*|editor|\w*moderator\w*|manager|staff|superuser|super_user|owner|author|reviewer|operator|support|root|writer|publisher|agent|employee|teacher|instructor|student|member|guest|viewer|vip|premium)$").unwrap()
    });
    let mut v: Vec<String> = vec![];
    for (i, r) in d.iter().enumerate() {
        for c in r.captures_iter(t) {
            let ad = c[1].trim_start_matches("is_").to_string();
            // Yüklem biçiminde yalnızca rol adları (izinli: kullanıcı modelinin rol enum değerleri)
            if (i == 0 && !izinli.contains(&ad) && !rol.is_match(&ad)) || matches!(ad.as_str(), "present" | "nil" | "blank" | "persisted" | "any" | "none") {
                continue;
            }
            if !v.contains(&ad) {
                v.push(ad);
            }
        }
    }
    v
}

/// Rails eylemi -> CanCanCan eylemi ve takma adları
fn cancan_eylemi(e: &str) -> &str {
    match e {
        "index" | "show" => "read",
        "new" | "create" => "create",
        "edit" | "update" => "update",
        _ => e,
    }
}

fn cancan_eslesir(kural: &str, eylem: &str) -> bool {
    kural == "manage"
        || kural == eylem
        || cancan_eylemi(eylem) == kural
        || (kural == "read" && matches!(eylem, "index" | "show" | "read"))
        || (kural == "create" && eylem == "new")
        || (kural == "update" && eylem == "edit")
}

type Tipler = HashMap<String, (String, bool)>;

/// İfadeyi noktalarından böler (parantez, köşeli parantez, süslü parantez ve tırnak içi hariç); `&.` de ayraçtır
fn noktali_parcalar(s: &str) -> Vec<String> {
    let mut v = vec![];
    let mut d = 0i32;
    let mut tirnak: Option<char> = None;
    let mut cur = String::new();
    for ch in s.chars() {
        match tirnak {
            Some(t) => {
                cur.push(ch);
                if ch == t {
                    tirnak = None;
                }
                continue;
            }
            None if ch == '"' || ch == '\'' => {
                tirnak = Some(ch);
                cur.push(ch);
                continue;
            }
            None => {}
        }
        match ch {
            '(' | '[' | '{' => d += 1,
            ')' | ']' | '}' => d -= 1,
            '.' if d == 0 => {
                v.push(cur.trim().trim_end_matches('&').to_string());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    v.push(cur.trim().to_string());
    v.into_iter().filter(|x| !x.is_empty()).collect()
}

/// Parçanın yöntem adı: `find(params[:id])` -> "find", `where(x: 1) { ... }` -> "where"
fn parca_adi(p: &str) -> &str {
    let son = p.find(['(', ' ', '{', '[']).unwrap_or(p.len());
    p[..son].trim()
}

const TEKIL_YONTEM: [&str; 18] = [
    "find",
    "find_by",
    "find_by!",
    "first",
    "first!",
    "last",
    "last!",
    "take",
    "take!",
    "new",
    "build",
    "create",
    "create!",
    "find_or_create_by",
    "find_or_create_by!",
    "find_or_initialize_by",
    "sole",
    "find_sole_by",
];
const COGUL_YONTEM: [&str; 26] = [
    "where",
    "all",
    "order",
    "includes",
    "joins",
    "left_joins",
    "left_outer_joins",
    "preload",
    "eager_load",
    "limit",
    "offset",
    "page",
    "per",
    "not",
    "distinct",
    "select",
    "group",
    "reorder",
    "unscoped",
    "rewhere",
    "references",
    "none",
    "or",
    "merge",
    "readonly",
    "lock",
];
/// Yazan ActiveRecord çağrıları
const YAZMA: [&str; 33] = [
    "create",
    "create!",
    "insert",
    "insert!",
    "insert_all",
    "insert_all!",
    "upsert",
    "upsert_all",
    "update",
    "update!",
    "update_all",
    "update_attribute",
    "update_attributes",
    "update_column",
    "update_columns",
    "save",
    "save!",
    "destroy",
    "destroy!",
    "destroy_all",
    "destroy_by",
    "delete",
    "delete_all",
    "delete_by",
    "increment!",
    "decrement!",
    "toggle!",
    "touch",
    "find_or_create_by",
    "find_or_create_by!",
    "create_or_find_by",
    "first_or_create",
    "first_or_create!",
];
const SILME: [&str; 7] = ["destroy", "destroy!", "destroy_all", "destroy_by", "delete", "delete_all", "delete_by"];

/// Geri çağrıları tetikleyen yazma türü: oluşturma, güncelleme, kaydetme (ikisinden biri), silme. update_all /
/// delete_all / insert_all / update_column geri çağrı çalıştırmaz
fn yazma_turu(ad: &str) -> Option<&'static str> {
    Some(match ad {
        "create" | "create!" | "find_or_create_by" | "find_or_create_by!" | "create_or_find_by" | "first_or_create" | "first_or_create!" => "create",
        "save" | "save!" => "save",
        "update" | "update!" | "update_attribute" | "update_attributes" | "toggle!" | "increment!" | "decrement!" => "update",
        "destroy" | "destroy!" | "destroy_all" | "destroy_by" => "destroy",
        _ => return None,
    })
}

/// Zincirin bir halkası: metot, bulunduğu dosya, `self`in sınıfı, işlemden uzaklığı ve türü
#[derive(Clone)]
struct Uye<'a> {
    di: usize,
    sinif: String,
    metot: &'a Metot,
    derinlik: usize,
    tur: &'static str,
}

impl<'a> Proje<'a> {
    /// Eyleme uygulanan filtreler: ata zinciri kökten yaprağa, dahil edilen modüllerin `included` blokları dahil
    /// edildikleri yerde; aynı adlı filtre yeniden tanımlanınca eskisinin yerini alır (Rails). İkinci dönen: CanCanCan
    /// makroları (`load_and_authorize_resource`)
    fn filtreler(&self, kontrolcu: &str, eylem: &str) -> (Vec<Filtre>, Vec<Cagri>) {
        let (mut z, _) = self.zincir(kontrolcu);
        z.reverse();
        let mut v = vec![];
        let mut cancan = vec![];
        for s in z {
            for x in self.acilislar(&s) {
                let sinif = self.sinif(*x);
                self.filtre_cagrilari(&sinif.cagrilar, &sinif.ad, eylem, &mut v, &mut cancan, 0);
            }
        }
        (v, cancan)
    }

    fn filtre_cagrilari(&self, cagrilar: &[Cagri], baglam: &str, eylem: &str, v: &mut Vec<Filtre>, cancan: &mut Vec<Cagri>, derinlik: usize) {
        for c in cagrilar.iter().filter(|c| c.alici.is_none()) {
            let uygulanir = secili(c, eylem);
            let kosullu = c.anahtar("if").is_some() || c.anahtar("unless").is_some() || c.kosul.is_some();
            let atla = |v: &mut Vec<Filtre>, ad: &str| {
                if uygulanir {
                    if kosullu {
                        v.iter_mut().filter(|f| f.ad == ad).for_each(|f| f.kosullu = true);
                    } else {
                        v.retain(|f| f.ad != ad);
                    }
                }
            };
            match c.ad.as_str() {
                "include" if derinlik < 4 => {
                    for a in &c.args {
                        let Arg::Ham(h) = a else { continue };
                        let Some(m) = self.coz(h, baglam) else { continue };
                        for x in self.acilislar(&m) {
                            let ms = self.sinif(*x);
                            for inc in ms.cagrilar.iter().filter(|i| i.ad == "included" && i.alici.is_none()) {
                                self.filtre_cagrilari(&inc.blok, &ms.ad, eylem, v, cancan, derinlik + 1);
                            }
                        }
                    }
                }
                a if FILTRE.contains(&a) => {
                    for ad in c.semboller() {
                        v.retain(|f| f.ad != ad);
                        if uygulanir {
                            let f = Filtre { ad, kosullu };
                            if a.starts_with("prepend") {
                                v.insert(0, f);
                            } else {
                                v.push(f);
                            }
                        }
                    }
                }
                a if FILTRE_ATLA.contains(&a) => {
                    for ad in c.semboller() {
                        atla(v, &ad);
                    }
                }
                // Rails 8 kimlik doğrulama üreticisi: `skip_before_action :require_authentication, **options`
                "allow_unauthenticated_access" => atla(v, "require_authentication"),
                "http_basic_authenticate_with" | "http_digest_authenticate_with" if uygulanir => v.push(Filtre { ad: c.ad.clone(), kosullu }),
                "load_and_authorize_resource" | "authorize_resource" if uygulanir => cancan.push(c.clone()),
                "load_resource" if uygulanir => cancan.push(c.clone()),
                _ => {}
            }
        }
    }

    /// Kullanıcı modelinin rol enum değerleri (rol yüklemlerini tanımak için)
    fn rol_degerleri(&self) -> HashSet<String> {
        self.kullanici_modeli()
            .and_then(|k| self.modeller.get(&k))
            .map(|m| m.enumlar.iter().filter(|(a, _)| a.contains("role") || a.contains("rol")).flat_map(|(_, d)| d.iter().cloned()).collect())
            .unwrap_or_default()
    }

    /// Tanımı projede olmayan, bilinen kütüphane filtreleri (Devise, Doorkeeper, Sorcery / Clearance, HTTP Basic)
    fn bilinen_filtre(&self, ad: &str) -> Option<Iz> {
        let devise = self.gemler.contains("devise") || self.modeller.values().any(|m| !m.devise.is_empty());
        let giris = Iz { giris: true, guard: true, ..Default::default() };
        if ad.starts_with("authenticate_") && ad.ends_with('!') && devise {
            return Some(giris);
        }
        match ad {
            "doorkeeper_authorize!" if self.gemler.contains("doorkeeper") => Some(giris),
            "require_login" if self.gemler.contains("sorcery") || self.gemler.contains("clearance") => Some(giris),
            "http_basic_authenticate_with" | "http_digest_authenticate_with" => Some(giris),
            _ => None,
        }
    }

    /// Filtrenin tanımını izler (en çok 4 kat): kimlik doğrulama, rol denetimi ya da izlenemeyen filtre
    fn izle(&self, sinif: &str, ad: &str, derinlik: usize, gorulen: &mut HashSet<String>, izinli: &HashSet<String>) -> Iz {
        if derinlik > 4 || !gorulen.insert(ad.to_string()) {
            return Iz::default();
        }
        let [giris_d, yonlendirme_d, ad_d, kullanici_yok] = desenler();
        match self.metot(sinif, ad, false) {
            Some((_, _, m)) => {
                let t = &m.govde;
                let mut iz = Iz::default();
                // Reddetme (401, giriş sayfasına yönlendirme) koşul altında da girişi gerektirir: koşul kimlik denetimidir;
                // `rescue` kolundaki reddetme yalnızca geçersiz kimlikte çalışır.
                // Kimlik isteyen çağrı (`authenticate_or_request_with_http_token`) yalnızca bir koşulda yapılıyorsa
                // (`if request.headers["Authorization"].present?`) giriş isteğe bağlıdır
                let kurtarma = |k: &Option<String>| k.as_deref() == Some("rescue");
                // Kullanıcı yokken reddetme (400-403) ya da yönlendirme de girişi gerektirir: `render status: 400 unless @user`
                let kullanicisiz = |k: &Option<String>| k.as_deref().is_some_and(|k| kullanici_yok.is_match(k));
                let reddetme = m
                    .dna
                    .firlatmalar
                    .iter()
                    .any(|f| (f.durum == Some(401) && !kurtarma(&f.kosul)) || (f.durum.is_some_and(|d| (400..=403).contains(&d)) && kullanicisiz(&f.kosul)))
                    || m.cagrilar.iter().any(|c| c.alici.is_none() && c.ad == "redirect_to" && kullanicisiz(&c.kosul))
                    || m.cagrilar.iter().any(|c| {
                        c.alici.is_none()
                            && !kurtarma(&c.kosul)
                            && ((c.ad == "head" || c.ad == "render") && giris_d.is_match(&c.metin) || c.ad == "redirect_to" && yonlendirme_d.is_match(&c.metin))
                    });
                let isteyen: Vec<&Cagri> = m
                    .cagrilar
                    .iter()
                    .filter(|c| c.ad != "head" && c.ad != "render" && giris_d.is_match(&format!("{}{}", c.alici.as_deref().map(|a| format!("{a}.")).unwrap_or_default(), c.ad)))
                    .collect();
                if reddetme || isteyen.iter().any(|c| c.kosul.is_none()) {
                    iz.giris = true;
                    iz.guard = true;
                } else if !isteyen.is_empty() {
                    iz.istege = true;
                    iz.guard = true;
                }
                let roller = rol_yuklemleri(t, izinli);
                let reddeder =
                    m.dna.firlatmalar.iter().any(|f| matches!(f.durum, Some(401) | Some(403))) || t.contains("redirect_to") || t.contains("head ") || t.contains("render");
                if !roller.is_empty() && reddeder {
                    iz.roller = roller;
                    iz.guard = true;
                }
                let adlar: BTreeSet<String> = m.cagrilar.iter().filter(|c| c.alici.is_none()).map(|c| c.ad.clone()).chain(m.adlar.iter().cloned()).collect();
                for n in adlar {
                    if n == ad {
                        continue;
                    }
                    if self.metot(sinif, &n, false).is_some() {
                        let alt = self.izle(sinif, &n, derinlik + 1, gorulen, izinli);
                        iz.giris |= alt.giris;
                        iz.istege |= alt.istege;
                        iz.guard |= alt.giris || alt.istege;
                        for r in alt.roller {
                            if !iz.roller.contains(&r) {
                                iz.roller.push(r);
                            }
                        }
                    } else if let Some(b) = self.bilinen_filtre(&n) {
                        iz.birlestir(b);
                    }
                }
                iz
            }
            None => self.bilinen_filtre(ad).unwrap_or_else(|| Iz { bilinmeyen: ad_d.is_match(ad), guard: ad_d.is_match(ad), ..Default::default() }),
        }
    }

    /// İfadenin modeli (çoğul mu): `Post.find(x)` -> (Post, false), `current_user.posts` -> (Post, true),
    /// `@post.comments.where(...)` -> (Comment, true)
    fn ifade_modeli(&self, metin: &str, tipler: &Tipler, baglam: &str) -> Option<(String, bool)> {
        let parcalar = noktali_parcalar(metin.trim());
        let ilk = parcalar.first()?;
        let ilk_ad = parca_adi(ilk);
        let (mut model, mut cogul_, mut i) = if ilk_ad.starts_with(|c: char| c.is_uppercase()) {
            if ilk_ad == "Current" && parcalar.get(1).map(|p| parca_adi(p)) == Some("user") {
                (self.kullanici_modeli()?, false, 2)
            } else {
                let a = self.coz(ilk_ad, baglam)?;
                self.modeller.contains_key(&a).then_some(())?;
                (a, true, 1)
            }
        } else if ilk_ad == "current_user" && !tipler.contains_key("current_user") {
            (self.kullanici_modeli()?, false, 1)
        } else if ilk_ad == "self" {
            let a = anahtar(baglam);
            self.modeller.contains_key(&a).then_some(())?;
            (a, false, 1)
        } else {
            let (m, c) = tipler.get(ilk_ad)?.clone();
            (m, c, 1)
        };
        while i < parcalar.len() {
            let ad = parca_adi(&parcalar[i]);
            let m = self.modeller.get(&model)?;
            if let Some((h, c)) = m.iliskiler.get(ad) {
                model = h.clone();
                cogul_ = *c;
                self.modeller.contains_key(&model).then_some(())?;
            } else if TEKIL_YONTEM.contains(&ad) {
                cogul_ = false;
            } else if COGUL_YONTEM.contains(&ad) || m.kapsamlar.contains(ad) {
                cogul_ = true;
            } else {
                return None;
            }
            i += 1;
        }
        Some((model, cogul_))
    }

    /// Atamalardan değişken tipleri (`@post = Post.find(...)`)
    fn tipleri_ekle(&self, m: &Metot, baglam: &str, tipler: &mut Tipler) {
        for (sol, sag, _) in &m.atamalar {
            if let Some(t) = self.ifade_modeli(sag, tipler, baglam) {
                tipler.entry(sol.clone()).or_insert(t);
            }
        }
    }

    /// Modelin yazma türüne uyan geri çağrı metotları
    fn geri_cagrilar(&self, model: &str, tur: &str) -> Vec<(usize, &'a Metot)> {
        let uyar = |ad: &str, c: &Cagri| -> bool {
            let on = c.anahtar("on").map(Arg::adlar).unwrap_or_default();
            let on_uyar = on.is_empty() || on.iter().any(|o| o == tur || (tur == "save" && (o == "create" || o == "update")));
            let olay = ad.trim_start_matches("before_").trim_start_matches("after_").trim_start_matches("around_");
            let olay = olay.trim_end_matches("_commit");
            match olay {
                "validation" | "save" => tur != "destroy",
                "create" => tur == "create" || tur == "save",
                "update" => tur == "update" || tur == "save",
                "destroy" => tur == "destroy",
                "commit" | "" => on_uyar,
                _ => false,
            }
        };
        let mut v = vec![];
        let (z, _) = self.zincir(model);
        for s in &z {
            for x in self.acilislar(s) {
                for c in self.sinif(*x).cagrilar.iter().filter(|c| c.alici.is_none() && geri_cagri_makrosu(&c.ad) && uyar(&c.ad, c)) {
                    for ad in c.semboller() {
                        if let Some((di, _, m)) = self.metot(model, &ad, false) {
                            v.push((di, m));
                        }
                    }
                }
            }
        }
        v
    }

    /// İşlemin davranış zinciri: eylem, filtreler, gövdede çağrılan proje metotları (kendi sınıfı, servis nesneleri,
    /// model metotları; 3 kat) ve yazılan modellerin geri çağrıları. Değişken tipleri zincir boyunca toplanır
    fn zincir_kur(&self, baslangic: Vec<Uye<'a>>, tipler: &mut Tipler) -> Vec<Uye<'a>> {
        static YENI: OnceLock<Regex> = OnceLock::new();
        let yeni = YENI.get_or_init(|| Regex::new(r"^((?:[A-Z]\w*::)*[A-Z]\w*)\.new\b").unwrap());
        let mut v: Vec<Uye<'a>> = vec![];
        let mut gorulen = HashSet::new();
        let mut kuyruk: std::collections::VecDeque<Uye<'a>> = baslangic.into();
        while let Some(u) = kuyruk.pop_front() {
            if v.len() >= EN_FAZLA || !gorulen.insert((u.di, u.metot.satir, u.metot.ad.clone())) {
                continue;
            }
            self.tipleri_ekle(u.metot, &u.sinif, tipler);
            if u.derinlik < 3 {
                let tur = if u.tur == "bagimlilik" { "bagimlilik" } else { "servis" };
                let mut sonraki = vec![];
                let yalin: BTreeSet<&str> = u
                    .metot
                    .cagrilar
                    .iter()
                    .filter(|c| c.alici.is_none() || c.alici.as_deref() == Some("self"))
                    .map(|c| c.ad.as_str())
                    .chain(u.metot.adlar.iter().map(String::as_str))
                    .collect();
                for ad in yalin {
                    if let Some((di, _, m)) = self.metot(&u.sinif, ad, false) {
                        sonraki.push(Uye { di, sinif: u.sinif.clone(), metot: m, derinlik: u.derinlik + 1, tur });
                    }
                }
                for c in u.metot.cagrilar.iter().filter(|c| c.alici.is_some()) {
                    let r = c.alici.as_deref().unwrap_or("");
                    if let Some(cap) = yeni.captures(r) {
                        if let Some(k) = self.coz(&cap[1], &u.sinif) {
                            if let Some((di, sk, m)) = self.metot(&k, &c.ad, false) {
                                sonraki.push(Uye { di, sinif: sk, metot: m, derinlik: u.derinlik + 1, tur });
                            }
                        }
                        continue;
                    }
                    if r.starts_with(|ch: char| ch.is_uppercase()) && !r.contains('.') {
                        if let Some(k) = self.coz(r, &u.sinif) {
                            let bulunan = self
                                .metot(&k, &c.ad, true)
                                .or_else(|| (c.ad == "call" || c.ad == "perform_now").then(|| self.metot(&k, if c.ad == "call" { "call" } else { "perform" }, false)).flatten());
                            if let Some((di, _, m)) = bulunan {
                                sonraki.push(Uye { di, sinif: k.clone(), metot: m, derinlik: u.derinlik + 1, tur });
                            }
                        }
                    }
                    // Model örneğinin metodu (`@post.publish!`) ve yazmanın geri çağrıları
                    if let Some((mk, _)) = self.ifade_modeli(r, tipler, &u.sinif) {
                        if let Some((di, _, m)) = self.metot(&mk, &c.ad, false) {
                            sonraki.push(Uye { di, sinif: mk.clone(), metot: m, derinlik: u.derinlik + 1, tur });
                        }
                        if let Some(yt) = yazma_turu(&c.ad) {
                            for (di, m) in self.geri_cagrilar(&mk, yt) {
                                sonraki.push(Uye { di, sinif: mk.clone(), metot: m, derinlik: u.derinlik + 1, tur: "servis" });
                            }
                        }
                    }
                }
                kuyruk.extend(sonraki);
            }
            v.push(u);
        }
        v
    }

    /// Çağrının veri etkisi: (model adı, okuma / yazma / silme)
    fn cagri_verisi(&self, c: &Cagri, tipler: &Tipler, baglam: &str) -> Option<(String, u8)> {
        let model = match c.alici.as_deref() {
            Some(r) => self.ifade_modeli(r, tipler, baglam)?.0,
            None => {
                let a = anahtar(baglam);
                self.modeller.contains_key(&a).then_some(a)?
            }
        };
        let m = self.modeller.get(&model)?;
        if let Some((h, _)) = m.iliskiler.get(&c.ad) {
            return self.modeller.get(h).map(|x| (x.ad.clone(), 0));
        }
        if c.alici.is_none() && !YAZMA.contains(&c.ad.as_str()) {
            return None;
        }
        let etki = if SILME.contains(&c.ad.as_str()) {
            2
        } else if YAZMA.contains(&c.ad.as_str()) {
            1
        } else if matches!(c.ad.as_str(), "new" | "build" | "errors" | "valid?" | "invalid?" | "assign_attributes" | "attributes=") {
            return None;
        } else {
            0
        };
        Some((m.ad.clone(), etki))
    }

    /// Zincirin veri bağı: okunan ve yazılan modeller; ham SQL görülürse eksik olabilir
    fn veri_bagi(&self, zincir: &[Uye], tipler: &Tipler) -> Option<VeriBag> {
        if self.modeller.is_empty() {
            return None;
        }
        let mut bag = VeriBag::default();
        for u in zincir {
            for c in &u.metot.cagrilar {
                if matches!(c.ad.as_str(), "find_by_sql" | "execute" | "exec_query" | "select_all" | "exec_update" | "exec_delete") {
                    bag.eksik_olabilir = true;
                }
                if let Some((m, etki)) = self.cagri_verisi(c, tipler, &u.sinif) {
                    if etki == 0 {
                        bag.okur.push(m);
                    } else {
                        bag.yazar.push(m);
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

    /// Zincirden davranış özeti: halkaların gövde özetleri, silinen modeller ve Pundit politikalarının metni
    fn dna_ozeti(&self, zincir: &[Uye], tipler: &Tipler, politikalar: &HashMap<usize, (String, bool)>) -> Value {
        let mut dnalar: Vec<(String, GovdeDna, usize, &'static str)> = vec![];
        for u in zincir {
            let mut d = u.metot.dna.clone();
            for c in &u.metot.cagrilar {
                if let Some((m, 2)) = self.cagri_verisi(c, tipler, &u.sinif) {
                    if !d.silinen.contains(&m) {
                        d.silinen.push(m);
                    }
                }
            }
            if u.derinlik == 0 {
                for f in d.firlatmalar.iter_mut().filter(|f| f.istisna.starts_with("Pundit")) {
                    if let Some((metin_, sahiplik)) = politikalar.get(&f.satir) {
                        f.kosul = Some(metin_.clone());
                        f.sahiplik = *sahiplik;
                    }
                }
            }
            dnalar.push((self.dosyalar[u.di].goreli.clone(), d, u.derinlik, u.tur));
        }
        let halkalar: Vec<crate::dna::Halka> =
            dnalar.iter().map(|(dosya, d, derinlik, tur)| crate::dna::Halka { dosya: dosya.clone(), dna: d, derinlik: *derinlik, tur }).collect();
        crate::dna::ozet(&halkalar)
    }
}

// ---- Girdiler ve yanıt ----

/// Model sütunundan ve doğrulamalarından alan: tip ve kurallar sütundan (db/schema.rb), zorunluluk (yalnızca
/// oluşturmada) ve sınırlar doğrulamalardan; izinli değerler enum'dan
fn model_alani(p: &Proje, model: Option<&Model>, alan: &str, olusturma: bool, satir_: usize, notlar: &mut Vec<String>) -> Alan {
    let mut a = Alan { tip: None, zorunlu: false, belirsiz: false, kurallar: vec![], satir: satir_, kalitimdan: None, degerler: vec![], ic: None };
    let Some(m) = model else { return a };
    if let Some((_, tip, _, _)) = p.tablo(m).and_then(|t| t.sutunlar.iter().find(|x| x.0 == alan)) {
        let (t, k) = sutun_tipi(tip);
        a.tip = t.map(String::from);
        a.kurallar.extend(k.map(String::from));
    }
    if let Some(d) = m.enumlar.get(alan) {
        a.tip = Some("enum".into());
        a.kurallar.retain(|k| !k.starts_with("IsInt") && !k.starts_with("IsString"));
        a.kurallar.push(format!("IsIn([{}])", d.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
        a.degerler = d.clone();
    }
    for d in m.dogrulamalar.get(alan).into_iter().flatten() {
        let s = d.secenek.as_ref();
        let sayi = |k: &str| {
            s.and_then(|x| x.anahtar(k)).and_then(|v| match v {
                Arg::Ham(h) => h.replace('_', "").parse::<f64>().ok().map(|n| n.to_string()),
                _ => None,
            })
        };
        match d.tur.as_str() {
            "presence" => {
                if olusturma && d.olusturmada {
                    a.zorunlu = true;
                    a.belirsiz |= d.kosullu;
                    if !a.kurallar.iter().any(|k| k == "IsNotEmpty()") {
                        a.kurallar.push("IsNotEmpty()".into());
                    }
                }
            }
            "length" => {
                for (k, kural) in [("maximum", "MaxLength"), ("minimum", "MinLength")] {
                    if let Some(n) = sayi(k) {
                        a.kurallar.push(format!("{kural}({n})"));
                    }
                }
                if let Some(n) = sayi("is") {
                    a.kurallar.push(format!("Length({n}, {n})"));
                }
            }
            "numericality" => {
                if matches!(s.and_then(|x| x.anahtar("only_integer")), Some(Arg::Ham(h)) if h == "true") {
                    a.kurallar.push("IsInt()".into());
                } else if !a.kurallar.iter().any(|k| k == "IsInt()") {
                    a.kurallar.push("IsNumber()".into());
                }
                for (k, kural) in [("greater_than_or_equal_to", "Min"), ("less_than_or_equal_to", "Max")] {
                    if let Some(n) = sayi(k) {
                        a.kurallar.push(format!("{kural}({n})"));
                    }
                }
                for (k, kural) in [("greater_than", "gt"), ("less_than", "lt")] {
                    if let Some(n) = sayi(k) {
                        a.kurallar.push(format!("{kural}({n})"));
                    }
                }
            }
            "inclusion" => {
                let degerler = s.and_then(|x| x.anahtar("in")).map(Arg::adlar).unwrap_or_default();
                if !degerler.is_empty() {
                    a.kurallar.push(format!("IsIn([{}])", degerler.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
                    a.degerler = degerler;
                }
            }
            "format" => {
                if let Some(Arg::Ham(re)) = s.and_then(|x| x.anahtar("with")) {
                    a.kurallar.push(format!("Matches({re})"));
                }
            }
            "uniqueness" => {
                let n = format!("'{alan}' benzersizliği (uniqueness, {}) veritabanına bakar; önizlemede kontrol edilemez", m.ad);
                if !notlar.contains(&n) {
                    notlar.push(n);
                }
            }
            _ => {}
        }
    }
    if m.ekler.iter().any(|e| e == alan) {
        a.tip = None;
        let n = format!("dosya doğrulaması kontrol edilemedi: '{alan}' parametresi (Active Storage eki); dosyanın varlığı, türü ve içeriği önizlemede sınanmaz");
        if !notlar.contains(&n) {
            notlar.push(n);
        }
    }
    a
}

impl<'a> Proje<'a> {
    /// Strong parameters izin listesinden alanlar: `:title` (tekil değer), `tags: []` (değer dizisi),
    /// `meta: [:a, :b]` (iç nesne), `items_attributes: [...]` (ilişkinin iç içe nitelikleri), `settings: {}` (serbest)
    fn izinli_alanlar(&self, spec: &[Arg], model: Option<&Model>, olusturma: bool, satir_: usize, notlar: &mut Vec<String>) -> BTreeMap<String, Alan> {
        let mut v = BTreeMap::new();
        for a in spec {
            match a {
                Arg::Sembol(s) | Arg::Metin(s) => {
                    v.insert(s.clone(), model_alani(self, model, s, olusturma, satir_, notlar));
                }
                Arg::Hash(ciftler) => {
                    for (k, deger) in ciftler {
                        let Some(ad) = k.ad() else { continue };
                        let mut alan = Alan { tip: None, zorunlu: false, belirsiz: false, kurallar: vec![], satir: satir_, kalitimdan: None, degerler: vec![], ic: None };
                        match deger {
                            // Kural kimliği satırla kurulur: aynı satırdaki alanlar ayrı kural taşımasın diye tip yeter
                            Arg::Dizi(ic) if ic.is_empty() => alan.tip = Some("array".into()),
                            Arg::Dizi(ic) => {
                                let alt = ad.strip_suffix("_attributes").and_then(|i| model?.iliskiler.get(i)).and_then(|(h, _)| self.modeller.get(h));
                                alan.tip = Some("object".into());
                                alan.ic = Some(self.izinli_alanlar(ic, alt, olusturma, satir_, notlar));
                            }
                            _ => alan.tip = Some("object".into()),
                        }
                        v.insert(ad.to_string(), alan);
                    }
                }
                _ => {}
            }
        }
        v
    }

    /// Eylemin strong parameters çağrıları: eylem gövdesinde ve çağırdığı yardımcı metotlarda (`post_params`).
    /// Dönen: (yardımcı adı, çağrı)
    fn strong_params(&self, kontrolcu: &str, eylem: &'a Metot) -> Vec<(String, &'a Cagri)> {
        let mut v = vec![];
        let mut kuyruk: Vec<(&str, &'a Metot, usize)> = vec![("params", eylem, 0)];
        let mut gorulen = HashSet::new();
        while let Some((ad, m, d)) = kuyruk.pop() {
            if !gorulen.insert(m.ad.clone()) {
                continue;
            }
            for c in &m.cagrilar {
                let params = c.alici.as_deref().is_some_and(|a| a == "params" || a.starts_with("params."));
                let yalniz_require = c.ad == "require" && !m.cagrilar.iter().any(|x| x.ad == "permit" && x.alici.as_deref().is_some_and(|a| a.contains("require")));
                if params && (matches!(c.ad.as_str(), "permit" | "expect") || yalniz_require) {
                    v.push((if d == 0 { "params".to_string() } else { ad.to_string() }, c));
                }
            }
            if d < 2 {
                let adlar: BTreeSet<&str> = m.cagrilar.iter().filter(|c| c.alici.is_none()).map(|c| c.ad.as_str()).chain(m.adlar.iter().map(String::as_str)).collect();
                for n in adlar {
                    if let Some((_, _, alt)) = self.metot(kontrolcu, n, false) {
                        if alt.govde.contains("params") {
                            kuyruk.push((&alt.ad, alt, d + 1));
                        }
                    }
                }
            }
        }
        v
    }

    /// Gövde, sorgu ve yol parametreleri; önizlemenin sınayamadığı kurallar (notlar)
    #[allow(clippy::too_many_arguments)]
    fn girdiler(
        &self,
        kontrolcu: &str,
        eylem: Option<&'a Metot>,
        filtreler: &[&'a Metot],
        yontem: &str,
        yol: &str,
        dosya: &str,
        notlar: &mut Vec<String>,
    ) -> (Vec<Girdi>, Vec<Girdi>, Vec<Girdi>) {
        static D: OnceLock<Regex> = OnceLock::new();
        let gevsek = D.get_or_init(|| Regex::new(r#"params(?:\.fetch|\.dig|\[)\s*\(?\s*(?::(\w+)|["'](\w+)["'])\s*(,)?"#).unwrap());
        let yol_adlari: Vec<String> = yol.split('/').filter_map(|p| p.strip_prefix(':')).map(String::from).collect();
        let parametreler: Vec<Girdi> = yol_adlari.iter().map(|a| Girdi { ad: Some(a.clone()), tip: Some("string".into()), sema: None }).collect();
        let okuma = matches!(yontem, "GET" | "HEAD" | "DELETE" | "OPTIONS");
        let olusturma = yontem == "POST";
        let mut govde_alanlari: BTreeMap<String, Alan> = BTreeMap::new();
        let mut sorgu_alanlari: BTreeMap<String, Alan> = BTreeMap::new();
        let mut dto: Option<String> = None;
        let mut gomulu: HashSet<String> = HashSet::new();
        static R: OnceLock<Regex> = OnceLock::new();
        let r = R.get_or_init(|| Regex::new(r#"require\(\s*:?["']?(\w+)"#).unwrap());
        if let Some(e) = eylem {
            for (yardimci, c) in self.strong_params(kontrolcu, e) {
                dto.get_or_insert(yardimci);
                // require / expect anahtarı (ParameterMissing -> 400: zorunlu, kesin) ve izin listesi
                let mut ciftler: Vec<(String, Vec<Arg>, bool)> = vec![];
                match c.ad.as_str() {
                    "permit" => {
                        let mut spec = c.args.clone();
                        if !c.anahtarli.is_empty() {
                            spec.push(Arg::Hash(c.anahtarli.clone()));
                        }
                        match c.alici.as_deref().and_then(|a| r.captures(a)) {
                            Some(cap) => ciftler.push((cap[1].to_string(), spec, true)),
                            None => ciftler.push((String::new(), spec, false)),
                        }
                    }
                    "expect" => {
                        for (k, v) in &c.anahtarli {
                            let spec = match v {
                                Arg::Dizi(x) => x.iter().flat_map(|y| if let Arg::Dizi(z) = y { z.clone() } else { vec![y.clone()] }).collect(),
                                x => vec![x.clone()],
                            };
                            ciftler.push((k.ad().unwrap_or("").to_string(), spec, true));
                        }
                        for a in &c.args {
                            ciftler.push((String::new(), vec![a.clone()], false));
                        }
                    }
                    _ => {
                        if let Some(k) = c.args.first().and_then(Arg::ad) {
                            ciftler.push((k.to_string(), vec![], true));
                        }
                    }
                }
                for (anahtar_, spec, zorunlu) in ciftler {
                    let model = (!anahtar_.is_empty()).then(|| self.model(&sinif_adi(&anahtar_), kontrolcu)).flatten();
                    if let Some(m) = model.filter(|_| !okuma) {
                        for o in &m.ozel_dogrulamalar {
                            let n = format!("özel doğrulama '{o}' ({}) önizlemede kontrol edilemez", m.ad);
                            if !notlar.contains(&n) {
                                notlar.push(n);
                            }
                        }
                    }
                    let alanlar = self.izinli_alanlar(&spec, model, olusturma, c.satir, notlar);
                    let hedef = if okuma { &mut sorgu_alanlari } else { &mut govde_alanlari };
                    if anahtar_.is_empty() {
                        hedef.extend(alanlar);
                    } else {
                        gomulu.insert(anahtar_.clone());
                        hedef.insert(
                            anahtar_,
                            Alan {
                                tip: Some("object".into()),
                                zorunlu,
                                belirsiz: false,
                                kurallar: vec![],
                                satir: c.satir,
                                kalitimdan: None,
                                degerler: vec![],
                                ic: (!spec.is_empty()).then_some(alanlar),
                            },
                        );
                    }
                }
            }
        }
        // `params[:x]`, `params.fetch(:x, varsayılan)`: zorunluluğu koddan çıkarılamaz (yorum)
        let metinler: Vec<&str> = eylem.into_iter().chain(filtreler.iter().copied()).map(|m| m.govde.as_str()).collect();
        for t in metinler {
            for c in gevsek.captures_iter(t) {
                let ad = c.get(1).or(c.get(2)).map(|m| m.as_str().to_string()).unwrap_or_default();
                if ad.is_empty() || yol_adlari.contains(&ad) || gomulu.contains(&ad) || matches!(ad.as_str(), "controller" | "action" | "format") {
                    continue;
                }
                let hedef = if okuma { &mut sorgu_alanlari } else { &mut govde_alanlari };
                hedef.entry(ad).or_insert(Alan { tip: None, zorunlu: false, belirsiz: true, kurallar: vec![], satir: 0, kalitimdan: None, degerler: vec![], ic: None });
            }
        }
        let sema = |dto: &str, alanlar: BTreeMap<String, Alan>| Sema {
            dto: dto.to_string(),
            dosya: Some(dosya.to_string()),
            bulunamadi: false,
            kalitim: None,
            alanlar,
            adaylar: vec![],
            dizi: false,
        };
        let govde = if govde_alanlari.is_empty() {
            vec![]
        } else {
            let d = dto.clone().unwrap_or_else(|| "params".into());
            vec![Girdi { ad: None, tip: Some(d.clone()), sema: Some(sema(&d, govde_alanlari)) }]
        };
        let sorgu = if sorgu_alanlari.is_empty() { vec![] } else { vec![Girdi { ad: None, tip: None, sema: Some(sema("query", sorgu_alanlari)) }] };
        (govde, sorgu, parametreler)
    }

    /// Modelin bütün sütunları (`render json: @post`: ActiveRecord'un `as_json`'ı)
    fn model_semasi(&self, model: &Model, dizi: bool) -> Sema {
        let mut alanlar = BTreeMap::new();
        if let Some(t) = self.tablo(model) {
            for (ad, tip, null_degil, s) in &t.sutunlar {
                if ad.contains("password") || ad.contains("token") || ad.starts_with("encrypted_") {
                    continue;
                }
                let (tt, k) = sutun_tipi(tip);
                let mut kurallar: Vec<String> = k.map(String::from).into_iter().collect();
                if !null_degil {
                    kurallar.push("nullable()".into());
                }
                alanlar.insert(ad.clone(), Alan { tip: tt.map(String::from), zorunlu: true, belirsiz: false, kurallar, satir: *s, kalitimdan: None, degerler: vec![], ic: None });
            }
        }
        Sema { dto: model.ad.clone(), dosya: self.sema.map(|d| self.dosyalar[d].goreli.clone()), bulunamadi: alanlar.is_empty(), kalitim: None, alanlar, adaylar: vec![], dizi }
    }

    /// Yanıt alanı: model sütunundan tip
    fn yanit_alani(&self, model: Option<&Model>, ad: &str, satir_: usize) -> Alan {
        let mut a = Alan { tip: None, zorunlu: true, belirsiz: false, kurallar: vec![], satir: satir_, kalitimdan: None, degerler: vec![], ic: None };
        if let Some((_, tip, null_degil, _)) = model.and_then(|m| self.tablo(m)).and_then(|t| t.sutunlar.iter().find(|x| x.0 == ad)) {
            let (t, k) = sutun_tipi(tip);
            a.tip = t.map(String::from);
            a.kurallar.extend(k.map(String::from));
            if !null_degil {
                a.kurallar.push("nullable()".into());
            }
        }
        a
    }

    /// Serileştirici sınıfının alanları: ActiveModel::Serializer / jsonapi-serializer `attributes`, `attribute`,
    /// ilişkiler; Blueprinter `identifier`, `fields`, `field`, `association`. Dönen: (alanlar, jsonapi mi)
    fn serilestirici(&self, s: &str, model: Option<&Model>, derinlik: usize) -> Option<(BTreeMap<String, Alan>, bool)> {
        let (z, _) = self.zincir(s);
        let mut v = BTreeMap::new();
        let jsonapi = self.soyundan(s, &["JSONAPI::Serializer", "FastJsonapi::ObjectSerializer"]);
        for x in z.iter().rev().flat_map(|k| self.acilislar(k).to_vec()) {
            let sinif = self.sinif(x);
            for c in sinif.cagrilar.iter().filter(|c| c.alici.is_none()) {
                match c.ad.as_str() {
                    "attributes" | "fields" => {
                        for a in c.semboller() {
                            v.insert(a.clone(), self.yanit_alani(model, &a, c.satir));
                        }
                    }
                    "attribute" | "field" | "identifier" => {
                        if let Some(a) = c.args.first().and_then(Arg::ad) {
                            v.insert(a.to_string(), self.yanit_alani(model, a, c.satir));
                        }
                    }
                    "has_many" | "has_one" | "belongs_to" | "association" => {
                        let Some(a) = c.args.first().and_then(Arg::ad) else { continue };
                        let cogul_ = c.ad == "has_many" || model.and_then(|m| m.iliskiler.get(a)).is_some_and(|(_, c)| *c);
                        let hedef = model.and_then(|m| m.iliskiler.get(a)).and_then(|(h, _)| self.modeller.get(h));
                        let ic = hedef.and_then(|h| {
                            let son = h.ad.rsplit("::").next().unwrap_or(&h.ad);
                            let ser = self.coz(&format!("{son}Serializer"), &sinif.ad)?;
                            (derinlik < 2).then(|| self.serilestirici(&ser, Some(h), derinlik + 1)).flatten().map(|x| x.0)
                        });
                        let kurallar = if cogul_ { vec!["IsArray()".to_string()] } else { vec!["IsObject()".to_string()] };
                        v.insert(
                            a.to_string(),
                            Alan {
                                tip: Some(if cogul_ { "array" } else { "object" }.into()),
                                zorunlu: true,
                                belirsiz: false,
                                kurallar,
                                satir: c.satir,
                                kalitimdan: None,
                                degerler: vec![],
                                ic,
                            },
                        );
                    }
                    _ => {}
                }
            }
        }
        (!v.is_empty()).then_some((v, jsonapi))
    }

    /// jbuilder çağrılarından alanlar. `yerel`: şablon değişkenleri -> model; dönen: (alanlar, kök dizi mi)
    fn jbuilder(&self, cagrilar: &[Cagri], dizin: &str, yerel: &Tipler, derinlik: usize) -> (BTreeMap<String, Alan>, bool) {
        let mut v = BTreeMap::new();
        let mut dizi = false;
        if derinlik > 6 {
            return (v, dizi);
        }
        let model_of = |ifade: &str| -> Option<(String, bool)> { self.ifade_modeli(ifade, yerel, "") };
        for c in cagrilar.iter().filter(|c| c.alici.as_deref() == Some("json")) {
            let ilk = c.args.first().map(Arg::metin);
            match c.ad.as_str() {
                "extract!" | "call" => {
                    let model = ilk.as_deref().and_then(model_of).and_then(|(m, _)| self.modeller.get(&m));
                    for a in c.args.iter().skip(1).filter_map(Arg::ad) {
                        v.insert(a.to_string(), self.yanit_alani(model, a, c.satir));
                    }
                }
                "partial!" => {
                    let ad = c.args.first().and_then(Arg::ad).or_else(|| c.anahtar("partial").and_then(Arg::ad));
                    if let Some(ad) = ad {
                        let mut yerel2 = yerel.clone();
                        let kaynak = c.anahtar("locals").map(|l| match l {
                            Arg::Hash(h) => h.clone(),
                            _ => vec![],
                        });
                        for (k, d) in kaynak.unwrap_or_else(|| c.anahtarli.clone()) {
                            if let (Some(k), Some(t)) = (k.ad(), model_of(&d.metin())) {
                                yerel2.insert(k.to_string(), t);
                            }
                        }
                        let (a, _) = self.kismi(ad, dizin, &yerel2, derinlik);
                        v.extend(a);
                    }
                }
                "array!" => {
                    dizi = true;
                    let oge = ilk.as_deref().and_then(model_of).map(|(m, _)| (m, false));
                    if let Some(ad) = c.anahtar("partial").and_then(Arg::ad) {
                        let mut yerel2 = yerel.clone();
                        let degisken = c.anahtar("as").and_then(Arg::ad).map(String::from).unwrap_or_else(|| ad.rsplit('/').next().unwrap_or(ad).to_string());
                        if let Some(o) = oge.clone() {
                            yerel2.insert(degisken, o);
                        }
                        v.extend(self.kismi(ad, dizin, &yerel2, derinlik).0);
                    } else {
                        let model = oge.as_ref().and_then(|(m, _)| self.modeller.get(m));
                        for a in c.args.iter().skip(1).filter_map(Arg::ad) {
                            v.insert(a.to_string(), self.yanit_alani(model, a, c.satir));
                        }
                    }
                    if c.blok_var {
                        // json.array! @posts do |post| ... end: blok parametresi öğenin modelidir
                        v.extend(self.jbuilder(&c.blok, dizin, &blok_yereli(c, yerel, oge), derinlik + 1).0);
                    }
                }
                "set!" => {
                    if let Some(a) = c.args.first().and_then(Arg::ad) {
                        v.insert(a.to_string(), Alan { tip: None, zorunlu: true, belirsiz: false, kurallar: vec![], satir: c.satir, kalitimdan: None, degerler: vec![], ic: None });
                    }
                }
                "merge!" | "key_format!" | "ignore_nil!" | "cache!" | "cache_if!" | "child!" => {}
                ad => {
                    let ad = ad.to_string();
                    let alan = if c.blok_var {
                        // json.author do ... end (iç nesne) ya da json.comments @post.comments do |c| ... end (nesne dizisi)
                        let oge = ilk.as_deref().and_then(model_of);
                        let (ic, _) = self.jbuilder(&c.blok, dizin, &blok_yereli(c, yerel, oge.clone()), derinlik + 1);
                        let dizi_ = oge.is_some_and(|(_, c)| c);
                        Alan {
                            tip: Some(if dizi_ { "array" } else { "object" }.into()),
                            zorunlu: true,
                            belirsiz: false,
                            kurallar: vec![if dizi_ { "IsArray()" } else { "IsObject()" }.into()],
                            satir: c.satir,
                            kalitimdan: None,
                            degerler: vec![],
                            ic: (!ic.is_empty()).then_some(ic),
                        }
                    } else if c.args.len() > 1 {
                        // json.comments @post.comments, :id, :body
                        let model = ilk.as_deref().and_then(model_of).and_then(|(m, _)| self.modeller.get(&m));
                        let ic: BTreeMap<String, Alan> = c.args.iter().skip(1).filter_map(Arg::ad).map(|a| (a.to_string(), self.yanit_alani(model, a, c.satir))).collect();
                        Alan {
                            tip: Some("array".into()),
                            zorunlu: true,
                            belirsiz: false,
                            kurallar: vec!["IsArray()".into()],
                            satir: c.satir,
                            kalitimdan: None,
                            degerler: vec![],
                            ic: Some(ic),
                        }
                    } else {
                        // json.title post.title: değerin sütun tipi
                        let tip = ilk.as_deref().and_then(|i| {
                            let (onu, son) = i.rsplit_once('.')?;
                            let (m, _) = model_of(onu)?;
                            Some(self.yanit_alani(self.modeller.get(&m), son, c.satir))
                        });
                        tip.unwrap_or(Alan { tip: None, zorunlu: true, belirsiz: false, kurallar: vec![], satir: c.satir, kalitimdan: None, degerler: vec![], ic: None })
                    };
                    v.insert(ad, alan);
                }
            }
        }
        (v, dizi)
    }

    /// Kısmi şablon: "posts/post" -> app/views/posts/_post.json.jbuilder; eğik çizgisizse aynı klasörde
    fn kismi(&self, ad: &str, dizin: &str, yerel: &Tipler, derinlik: usize) -> (BTreeMap<String, Alan>, bool) {
        let (klasor, dosya) = match ad.rsplit_once('/') {
            Some((k, d)) => (k.to_string(), d.to_string()),
            None => (dizin.to_string(), ad.to_string()),
        };
        let yol = format!("{klasor}/_{dosya}.json.jbuilder");
        match self.sablonlar.get(&yol) {
            Some(di) => {
                let mut y = yerel.clone();
                // Kısmi şablonun yerel değişkeni, şablon adıyla da gelir (`post`)
                if !y.contains_key(&dosya) {
                    if let Some(m) = self.model(&camelize(&dosya), "") {
                        y.insert(dosya.clone(), (anahtar(&m.ad), false));
                    }
                }
                self.jbuilder(&self.dosyalar[*di].ust, &klasor, &y, derinlik + 1)
            }
            None => (BTreeMap::new(), false),
        }
    }

    /// Başarılı yanıtın şeması: `render json:` (serileştirici, model, sözlük), `render :sablon`, örtük jbuilder şablonu
    fn yanit(&self, kontrolcu_yolu: &str, kontrolcu: &str, eylem_adi: &str, eylem: Option<&Metot>, tipler: &Tipler) -> Option<Yanit> {
        let ams = self.gemler.contains("active_model_serializers");
        let durum = |c: &Cagri| c.anahtar("status").and_then(|s| durum_kodu(&s.metin()));
        let renderlar: Vec<&Cagri> = eylem.map(|e| e.cagrilar.iter().filter(|c| c.alici.is_none() && c.ad == "render").collect()).unwrap_or_default();
        let basarili = |c: &&&Cagri| durum(c).is_none_or(|d| d < 400);
        let sablon_yaniti = |sablon: &str, d: Option<u16>| -> Option<Yanit> {
            let yol = format!("{kontrolcu_yolu}/{sablon}.json.jbuilder");
            let di = *self.sablonlar.get(&yol)?;
            let (alanlar, dizi) = self.jbuilder(&self.dosyalar[di].ust, kontrolcu_yolu, tipler, 0);
            Some(Yanit {
                kaynak: "jbuilder".into(),
                kesin: true,
                durum: Some(d.unwrap_or(200)),
                sema: Sema { dto: yol.clone(), dosya: Some(self.dosyalar[di].goreli.clone()), bulunamadi: alanlar.is_empty(), kalitim: None, alanlar, adaylar: vec![], dizi },
            })
        };
        static S: OnceLock<Regex> = OnceLock::new();
        let s = S.get_or_init(|| {
            Regex::new(r"^((?:[A-Z]\w*::)*[A-Z]\w*(?:Serializer|Blueprint|Blueprinter|Resource|Presenter))\.(?:new|render|render_as_hash|render_as_json)\s*\(\s*([^,)]*)").unwrap()
        });
        for c in renderlar.iter().filter(basarili) {
            let d = durum(c);
            if let Some(j) = c.anahtar("json") {
                let ifade = j.metin();
                let acik = c.anahtar("serializer").or(c.anahtar("each_serializer")).and_then(|s| if let Arg::Ham(h) = s { Some(h.clone()) } else { None });
                // XSerializer.new(@x) / XBlueprint.render(@x)
                let (ser, veri) = match s.captures(&ifade) {
                    Some(cap) => (Some(cap[1].to_string()), cap[2].trim().to_string()),
                    None => (acik.clone(), ifade.clone()),
                };
                let model = self.ifade_modeli(&veri, tipler, kontrolcu);
                let dizi = model.as_ref().is_some_and(|(_, c)| *c) || c.anahtar("each_serializer").is_some();
                let m = model.as_ref().and_then(|(k, _)| self.modeller.get(k));
                // Açık serileştirici, yoksa AMS'in örtük `XSerializer`'ı
                let ser_anahtari = ser.as_deref().and_then(|x| self.coz(x, kontrolcu)).or_else(|| {
                    let m = m.filter(|_| ams)?;
                    let son = m.ad.rsplit("::").next().unwrap_or(&m.ad);
                    self.coz(&format!("{son}Serializer"), kontrolcu)
                });
                if let Some((alanlar, jsonapi)) = ser_anahtari.as_deref().and_then(|x| self.serilestirici(x, m, 0)) {
                    let ad = self.tam_ad(ser_anahtari.as_deref().unwrap_or(""));
                    let (alanlar, dizi) = if jsonapi {
                        // jsonapi-serializer: { data: { id, type, attributes: {...} } }
                        let mut ozn = alanlar;
                        ozn.remove("id");
                        let veri_alani = |ic: BTreeMap<String, Alan>| Alan {
                            tip: Some("object".into()),
                            zorunlu: true,
                            belirsiz: false,
                            kurallar: vec!["IsObject()".into()],
                            satir: 0,
                            kalitimdan: None,
                            degerler: vec![],
                            ic: Some(ic),
                        };
                        let mut ic = BTreeMap::new();
                        ic.insert(
                            "id".to_string(),
                            Alan { tip: Some("string".into()), zorunlu: true, belirsiz: false, kurallar: vec![], satir: 0, kalitimdan: None, degerler: vec![], ic: None },
                        );
                        ic.insert(
                            "type".to_string(),
                            Alan { tip: Some("string".into()), zorunlu: true, belirsiz: false, kurallar: vec![], satir: 0, kalitimdan: None, degerler: vec![], ic: None },
                        );
                        ic.insert("attributes".to_string(), veri_alani(ozn));
                        let mut kok = BTreeMap::new();
                        let mut data = veri_alani(ic);
                        if dizi {
                            data.tip = Some("array".into());
                            data.kurallar = vec!["IsArray()".into()];
                        }
                        kok.insert("data".to_string(), data);
                        (kok, false)
                    } else {
                        (alanlar, dizi)
                    };
                    let dosya = self.acilislar(ser_anahtari.as_deref().unwrap_or("")).first().map(|x| self.dosyalar[x.0].goreli.clone());
                    return Some(Yanit {
                        kaynak: "serializer".into(),
                        kesin: ser.is_some(),
                        durum: Some(d.unwrap_or(200)),
                        sema: Sema { dto: ad, dosya, bulunamadi: false, kalitim: None, alanlar, adaylar: vec![], dizi },
                    });
                }
                if let Some(m) = m {
                    return Some(Yanit { kaynak: "render_json".into(), kesin: false, durum: Some(d.unwrap_or(200)), sema: self.model_semasi(m, dizi) });
                }
                if let Arg::Hash(h) = j {
                    let alanlar: BTreeMap<String, Alan> = h
                        .iter()
                        .filter_map(|(k, v)| {
                            let a = k.ad()?.to_string();
                            let tip = match v {
                                Arg::Metin(_) => Some("string".to_string()),
                                Arg::Ham(x) if x == "true" || x == "false" => Some("boolean".into()),
                                Arg::Ham(x) if x.parse::<f64>().is_ok() => Some("number".into()),
                                Arg::Dizi(_) => Some("array".into()),
                                Arg::Hash(_) => Some("object".into()),
                                _ => None,
                            };
                            Some((a, Alan { tip, zorunlu: true, belirsiz: true, kurallar: vec![], satir: c.satir, kalitimdan: None, degerler: vec![], ic: None }))
                        })
                        .collect();
                    if !alanlar.is_empty() {
                        return Some(Yanit {
                            kaynak: "render_json".into(),
                            kesin: false,
                            durum: Some(d.unwrap_or(200)),
                            sema: Sema { dto: "json".into(), dosya: None, bulunamadi: false, kalitim: None, alanlar, adaylar: vec![], dizi: false },
                        });
                    }
                }
                continue;
            }
            // render :show / render "show" / render action: :show / render template: "posts/show"
            let sablon = c.args.first().and_then(Arg::ad).or_else(|| c.anahtar("action").and_then(Arg::ad)).map(String::from);
            if let Some(s) = sablon {
                if let Some(y) = sablon_yaniti(s.rsplit('/').next().unwrap_or(&s), d) {
                    return Some(y);
                }
            }
        }
        // Açık render yoksa eylemin örtük şablonu
        if renderlar.is_empty() || renderlar.iter().all(|c| !basarili(&c)) {
            let yonlendirir = eylem.is_some_and(|e| e.cagrilar.iter().any(|c| c.alici.is_none() && matches!(c.ad.as_str(), "redirect_to" | "head" | "send_data" | "send_file")));
            if !yonlendirir {
                return sablon_yaniti(eylem_adi, None);
            }
        }
        None
    }
}

/// Blok parametresinin modeli: `json.array! @posts do |post|` -> post: Post
fn blok_yereli(c: &Cagri, yerel: &Tipler, oge: Option<(String, bool)>) -> Tipler {
    let mut y = yerel.clone();
    if let (Some((m, _)), Some(p)) = (oge, c.metin.split('|').nth(1)) {
        let p = p.trim();
        if !p.is_empty() && p.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
            y.insert(p.to_string(), (m, false));
        }
    }
    y
}

// ---- Yetkilendirme: Pundit ve CanCanCan ----

/// Bir yetkilendirme denetiminin sonucu: guard adı, roller, girişi gerektiriyor mu (tanımdan), politikanın metni ve
/// sahiplik karşılaştırması (DNA), denetim satırı
struct Yetki {
    guard: String,
    roller: Vec<String>,
    giris: bool,
    metin: Option<String>,
    sahiplik: bool,
    satir: usize,
}

impl<'a> Proje<'a> {
    /// Pundit: `authorize @post` / `authorize Post` / `authorize @post, :publish?` / `authorize [:admin, @post]` ->
    /// `PostPolicy#update?` (eylem adı + `?`). Roller politikanın metodundan; politika `initialize`'da kullanıcı yoksa
    /// fırlatıyorsa giriş gerekir
    fn pundit(&self, kontrolcu: &str, eylem: &str, metotlar: &[&Metot], tipler: &Tipler, izinli: &HashSet<String>) -> Vec<Yetki> {
        let mut v = vec![];
        for m in metotlar {
            for c in m.cagrilar.iter().filter(|c| c.alici.is_none() && c.ad == "authorize") {
                let Some(ilk) = c.args.first() else { continue };
                let (ad_alani, nesne) = match ilk {
                    Arg::Dizi(x) => (x.iter().take(x.len().saturating_sub(1)).filter_map(Arg::ad).map(camelize).collect::<Vec<_>>().join("::"), x.last().cloned()),
                    x => (String::new(), Some(x.clone())),
                };
                let model = match &nesne {
                    Some(Arg::Sembol(s)) => Some(camelize(s)),
                    Some(Arg::Ham(h)) if h.starts_with(|c: char| c.is_uppercase()) => Some(h.rsplit("::").next().unwrap_or(h).to_string()),
                    Some(Arg::Ham(h)) => {
                        self.ifade_modeli(h, tipler, kontrolcu).and_then(|(k, _)| self.modeller.get(&k)).map(|m| m.ad.rsplit("::").next().unwrap_or(&m.ad).to_string())
                    }
                    _ => None,
                };
                let Some(model) = model else { continue };
                let politika_adi = c.anahtar("policy_class").and_then(|p| if let Arg::Ham(h) = p { Some(h.clone()) } else { None }).unwrap_or_else(|| {
                    if ad_alani.is_empty() {
                        format!("{model}Policy")
                    } else {
                        format!("{ad_alani}::{model}Policy")
                    }
                });
                let sorgu = c.args.get(1).and_then(Arg::ad).map(String::from).unwrap_or_else(|| format!("{eylem}?"));
                let mut y = Yetki { guard: format!("Pundit {politika_adi}#{sorgu}"), roller: vec![], giris: false, metin: None, sahiplik: false, satir: c.satir };
                if let Some(p) = self.coz(&politika_adi, kontrolcu) {
                    if let Some((_, _, pm)) = self.metot(&p, &sorgu, false) {
                        let govde = kisa(&pm.govde);
                        y.roller = rol_yuklemleri(&pm.govde, izinli);
                        y.sahiplik = crate::dna::sahiplik_mi(&govde, Some(403));
                        y.metin = Some(format!("!{politika_adi}#{sorgu}: {govde}"));
                    }
                    // ApplicationPolicy#initialize: `raise Pundit::NotAuthorizedError unless user`
                    if let Some((_, _, init)) = self.metot(&p, "initialize", false) {
                        y.giris = init.dna.firlatmalar.iter().any(|f| f.kosul.as_deref().is_some_and(|k| k.contains("user")));
                    }
                }
                v.push(y);
            }
        }
        v
    }

    /// CanCanCan: `load_and_authorize_resource` / `authorize_resource` (denetleyicinin modeli) ve `authorize! :eylem, X`.
    /// Roller `Ability#initialize`'daki eşleşen `can` kurallarının koşullarından; koşulsuz (ya da olumsuz koşullu) bir
    /// kural eşleşirse rol kısıtı yoktur
    fn cancan(&self, kontrolcu: &str, eylem: &str, makrolar: &[Cagri], metotlar: &[&Metot], tipler: &mut Tipler, izinli: &HashSet<String>) -> Vec<Yetki> {
        let mut istekler: Vec<(String, String, usize)> = vec![];
        let kontrolcu_modeli = {
            let son = self.tam_ad(kontrolcu);
            let son = son.rsplit("::").next().unwrap_or(&son).trim_end_matches("Controller").to_string();
            sinif_adi(&underscore(&son))
        };
        for c in makrolar {
            let model = c
                .anahtar("class")
                .and_then(|x| match x {
                    Arg::Ham(h) => Some(h.clone()),
                    x => x.ad().map(camelize),
                })
                .or_else(|| c.args.first().and_then(Arg::ad).map(camelize))
                .unwrap_or_else(|| kontrolcu_modeli.clone());
            // load_*: @post / @posts
            if let Some(m) = self.model(&model, kontrolcu) {
                let a = anahtar(&m.ad);
                let tekil_ = underscore(m.ad.rsplit("::").next().unwrap_or(&m.ad));
                tipler.entry(format!("@{tekil_}")).or_insert((a.clone(), false));
                tipler.entry(format!("@{}", cogul(&tekil_))).or_insert((a, true));
            }
            if c.ad != "load_resource" {
                istekler.push((eylem.to_string(), model, c.satir));
            }
        }
        for m in metotlar {
            for c in m.cagrilar.iter().filter(|c| c.alici.is_none() && c.ad == "authorize!") {
                let Some(e) = c.args.first().and_then(Arg::ad) else { continue };
                let model = match c.args.get(1) {
                    Some(Arg::Ham(h)) if h.starts_with(|c: char| c.is_uppercase()) => h.clone(),
                    Some(Arg::Ham(h)) => match self.ifade_modeli(h, tipler, kontrolcu).and_then(|(k, _)| self.modeller.get(&k)) {
                        Some(m) => m.ad.clone(),
                        None => continue,
                    },
                    Some(Arg::Sembol(s)) => s.clone(),
                    _ => continue,
                };
                istekler.push((e.to_string(), model, c.satir));
            }
        }
        if istekler.is_empty() {
            return vec![];
        }
        let kurallar: Vec<&Cagri> = self
            .coz("Ability", "")
            .and_then(|a| self.metot(&a, "initialize", false))
            .map(|(_, _, m)| m.cagrilar.iter().filter(|c| c.alici.is_none() && c.ad == "can").collect())
            .unwrap_or_default();
        let mut v = vec![];
        for (e, model, satir_) in istekler {
            let son = model.rsplit("::").next().unwrap_or(&model).to_string();
            let mut acik = false;
            let mut roller = vec![];
            for k in &kurallar {
                let eylemler = k.args.first().map(Arg::adlar).unwrap_or_default();
                let konular: Vec<String> = match k.args.get(1) {
                    Some(Arg::Dizi(x)) => x.iter().map(|y| y.ad().map(String::from).unwrap_or_else(|| y.metin())).collect(),
                    Some(Arg::Sembol(s)) => vec![s.clone()],
                    Some(x) => vec![x.metin()],
                    None => vec![],
                };
                let konu = konular.iter().any(|x| x == "all" || x.rsplit("::").next() == Some(son.as_str()) || camelize(x) == son);
                if !konu || !eylemler.iter().any(|x| cancan_eslesir(x, &e)) {
                    continue;
                }
                match k.kosul.as_deref() {
                    Some(kosul) if !kosul.starts_with("!(") => {
                        let r = rol_yuklemleri(kosul, izinli);
                        if r.is_empty() {
                            acik = true;
                        }
                        roller.extend(r);
                    }
                    _ => acik = true,
                }
            }
            if acik {
                roller.clear();
            }
            roller.sort();
            roller.dedup();
            v.push(Yetki { guard: format!("CanCanCan can :{}, {son}", cancan_eylemi(&e)), roller, giris: false, metin: None, sahiplik: false, satir: satir_ });
        }
        v
    }
}

// ---- Kapılar ----

/// Rails'in kendi denetleyicileri (projede tanımlı değildir)
fn yerlesik(kontrolcu: &str) -> Option<&'static str> {
    match kontrolcu {
        "rails/health" => Some("Rails::HealthController"),
        "rails/pwa" => Some("Rails::PwaController"),
        _ => None,
    }
}

/// Devise denetleyicisinin varsayılan giriş kararı: oturum açma / kayıt / parola sıfırlama uçları açık; hesap
/// düzenleme ve silme (`authenticate_scope!`) giriş ister
fn devise_girisi(kontrolcu: &str, eylem: &str) -> bool {
    kontrolcu.ends_with("registrations") && matches!(eylem, "edit" | "update" | "destroy" | "cancel")
}

/// Devise uçlarının gövdesi (kapsam anahtarıyla gömülü): oturum açma, kayıt, parola
fn devise_govdesi(kapsam: &str, kontrolcu: &str, eylem: &str, satir_: usize) -> Vec<Girdi> {
    let alanlar: Vec<(&str, bool)> = match (kontrolcu.rsplit('/').next().unwrap_or(""), eylem) {
        ("sessions", "create") => vec![("email", true), ("password", true), ("remember_me", false)],
        ("registrations", "create") => vec![("email", true), ("password", true), ("password_confirmation", false)],
        ("registrations", "update") => vec![("email", false), ("password", false), ("password_confirmation", false), ("current_password", true)],
        ("passwords", "create") | ("confirmations", "create") | ("unlocks", "create") => vec![("email", true)],
        ("passwords", "update") => vec![("reset_password_token", true), ("password", true), ("password_confirmation", false)],
        _ => return vec![],
    };
    let ic: BTreeMap<String, Alan> = alanlar
        .into_iter()
        .map(|(a, z)| {
            let kurallar = if a == "remember_me" { vec!["IsBoolean()".to_string()] } else { vec!["IsString()".to_string()] };
            (
                a.to_string(),
                Alan {
                    tip: Some(if a == "remember_me" { "boolean" } else { "string" }.into()),
                    zorunlu: z,
                    belirsiz: false,
                    kurallar,
                    satir: satir_,
                    kalitimdan: None,
                    degerler: vec![],
                    ic: None,
                },
            )
        })
        .collect();
    let mut kok = BTreeMap::new();
    kok.insert(
        kapsam.to_string(),
        Alan {
            tip: Some("object".into()),
            zorunlu: true,
            belirsiz: false,
            kurallar: vec!["ValidateNested()".into()],
            satir: satir_,
            kalitimdan: None,
            degerler: vec![],
            ic: Some(ic),
        },
    );
    vec![Girdi {
        ad: None,
        tip: Some("devise".into()),
        sema: Some(Sema { dto: format!("devise {kapsam}"), dosya: None, bulunamadi: false, kalitim: None, alanlar: kok, adaylar: vec![], dizi: false }),
    }]
}

pub(crate) fn kapilar(kok: &Path, dosyalar: &[Dosya]) -> (Vec<Kapi>, Value) {
    let p = Proje::yeni(kok, dosyalar);
    let r = rotalari_oku(&p);
    let izinli = p.rol_degerleri();
    let mut kapilar: Vec<Kapi> = vec![];
    let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
    let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut kosullu_kapilar = vec![];
    let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
    let mut eylemsiz: Vec<String> = vec![];
    let mut genel_guardlar: Vec<String> = vec![];
    if let Some(a) = p.coz("ApplicationController", "") {
        let (f, _) = p.filtreler(&a, "");
        for x in f {
            if p.izle(&a, &x.ad, 0, &mut HashSet::new(), &izinli).guard {
                genel_guardlar.push(x.ad);
            }
        }
    }
    let dugum_kimligi = |i: usize| -> String {
        let d = &r.dugumler[i];
        format!("{}:{}", dosyalar[d.di].goreli, if i == 0 { "routes".to_string() } else { format!("{} {}", d.satir, d.etiket) })
    };
    for rota in &r.rotalar {
        let rota_yeri = format!("{}:{}", dosyalar[rota.di].goreli, rota.satir);
        let sinif_adi_ = format!("{}Controller", camelize(&rota.kontrolcu));
        let kontrolcu = p.siniflar.contains_key(&anahtar(&sinif_adi_)).then(|| anahtar(&sinif_adi_));
        let eylem: Option<(usize, String, &Metot)> = kontrolcu.as_deref().and_then(|k| p.metot(k, &rota.eylem, false)).filter(|(_, _, m)| !m.ozel);
        let sablon = p.gorunumler.iter().any(|g| g.starts_with(&format!("{}/{}.", rota.kontrolcu, rota.eylem)));
        let devise = rota.devise.is_some() && (rota.kontrolcu.starts_with("devise/") || eylem.is_none());
        let yerlesik_ = yerlesik(&rota.kontrolcu).filter(|_| kontrolcu.is_none());
        if !devise && yerlesik_.is_none() && (kontrolcu.is_none() || (eylem.is_none() && !sablon)) {
            eylemsiz.push(format!("{rota_yeri} {} {} -> {}#{} (denetleyici ya da eylem yok)", rota.yontem, rota.yol, rota.kontrolcu, rota.eylem));
            continue;
        }
        let gorunen_sinif = kontrolcu.as_deref().map(|k| p.tam_ad(k)).or(yerlesik_.map(String::from)).unwrap_or_else(|| sinif_adi_.clone());
        // Dosya ve satır: eylemin metodu, yoksa denetleyici sınıfı, yoksa rota
        let (dosya, satir_) = match (&eylem, kontrolcu.as_deref().and_then(|k| p.acilislar(k).first())) {
            (Some((di, _, m)), _) => (dosyalar[*di].goreli.clone(), m.satir),
            (None, Some(x)) if !devise => (dosyalar[x.0].goreli.clone(), p.sinif(*x).satir),
            _ => (dosyalar[rota.di].goreli.clone(), rota.satir),
        };
        let yer = format!("{dosya}:{satir_}");
        let mut guardlar: Vec<String> = vec![];
        let mut roller: Vec<String> = vec![];
        let mut notlar: Vec<String> = vec![];
        let (giris, kanit, govde, sorgu, parametreler, veri, yanit, ozet);
        if let (Some(k), false) = (kontrolcu.as_deref(), devise) {
            let (filtreler, cancan_makrolari) = p.filtreler(k, &rota.eylem);
            let mut iz = Iz::default();
            let mut kosullu_giris = false;
            let mut filtre_metotlari: Vec<(usize, &Metot)> = vec![];
            for f in &filtreler {
                let x = p.izle(k, &f.ad, 0, &mut HashSet::new(), &izinli);
                if let Some((di, _, m)) = p.metot(k, &f.ad, false) {
                    filtre_metotlari.push((di, m));
                }
                if x.guard {
                    guardlar.push(f.ad.clone());
                }
                if f.kosullu && x.giris {
                    kosullu_giris = true;
                    let mut x = x;
                    x.giris = false;
                    iz.birlestir(x);
                } else {
                    iz.birlestir(x);
                }
            }
            // Tipler: kullanıcı, CanCanCan'in yüklediği kaynak, filtrelerin ve eylemin atamaları
            let mut tipler: Tipler = HashMap::new();
            let metotlar: Vec<&Metot> = filtre_metotlari.iter().map(|x| x.1).chain(eylem.as_ref().map(|e| e.2)).collect();
            let yetkiler_cancan = p.cancan(k, &rota.eylem, &cancan_makrolari, &metotlar, &mut tipler, &izinli);
            for m in &metotlar {
                p.tipleri_ekle(m, &p.tam_ad(k), &mut tipler);
            }
            let yetkiler_pundit = p.pundit(k, &rota.eylem, &metotlar, &tipler, &izinli);
            let mut politikalar: HashMap<usize, (String, bool)> = HashMap::new();
            let mut politika_girisi = false;
            for y in yetkiler_pundit.iter().chain(yetkiler_cancan.iter()) {
                guardlar.push(y.guard.clone());
                roller.extend(y.roller.iter().cloned());
                politika_girisi |= y.giris;
                if let Some(m) = &y.metin {
                    politikalar.insert(y.satir, (m.clone(), y.sahiplik));
                }
            }
            roller.extend(iz.roller.iter().cloned());
            roller.sort();
            roller.dedup();
            // Giriş kararı (FastAPI / NestJS ile aynı ölçek): tanımı izlenen kimlik doğrulama kesin; koşullu filtre, rol
            // denetimi ve izlenemeyen filtre çıkarım
            let govde_metni: String = metotlar.iter().map(|m| m.govde.as_str()).collect::<Vec<_>>().join("\n");
            let kullanici_okur = ["current_user", "user_signed_in?", "Current.user", "Current.session"].iter().any(|x| govde_metni.contains(x));
            (giris, kanit) = if iz.giris || politika_girisi {
                ("gerekli", if iz.bilinmeyen { "varsayim_bilinmeyen_guard" } else { "kesin" })
            } else if !iz.roller.is_empty() || kosullu_giris {
                ("gerekli", if iz.bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" })
            } else if iz.bilinmeyen {
                ("herkese_acik", "varsayim_bilinmeyen_guard")
            } else if iz.istege || kullanici_okur {
                ("istege_bagli", "varsayim")
            } else {
                ("herkese_acik", "varsayim")
            };
            let filtre_ref: Vec<&Metot> = filtre_metotlari.iter().map(|x| x.1).collect();
            (govde, sorgu, parametreler) = p.girdiler(k, eylem.as_ref().map(|e| e.2), &filtre_ref, &rota.yontem, &rota.yol, &dosya, &mut notlar);
            // Davranış zinciri: eylem (işleyici), filtreler (bağımlılık), çağrılanlar ve geri çağrılar
            let sinif_tam = p.tam_ad(k);
            let mut baslangic: Vec<Uye> = eylem.iter().map(|(di, _, m)| Uye { di: *di, sinif: k.to_string(), metot: m, derinlik: 0, tur: "isleyici" }).collect();
            baslangic.extend(filtre_metotlari.iter().map(|(di, m)| Uye { di: *di, sinif: k.to_string(), metot: m, derinlik: 1, tur: "bagimlilik" }));
            let zincir = p.zincir_kur(baslangic, &mut tipler);
            veri = p.veri_bagi(&zincir, &tipler);
            let d = p.dna_ozeti(&zincir, &tipler, &politikalar);
            if !crate::dna::bos(&d) {
                dna_haritasi.entry(yer.clone()).or_insert(d);
            }
            yanit = p.yanit(&rota.kontrolcu, &sinif_tam, &rota.eylem, eylem.as_ref().map(|e| e.2), &tipler);
            ozet = eylem.as_ref().and_then(|e| e.2.yorum.clone());
        } else {
            // Devise ya da Rails'in kendi denetleyicisi: davranış kütüphaneden bilinir
            let gerekli = devise && devise_girisi(&rota.kontrolcu, &rota.eylem);
            if gerekli {
                guardlar.push("authenticate_scope!".into());
            }
            (giris, kanit) = if gerekli { ("gerekli", "kesin") } else { ("herkese_acik", "varsayim") };
            let kapsam = rota.devise.as_deref().map(|m| underscore(m.rsplit("::").next().unwrap_or(m))).unwrap_or_default();
            govde = if devise { devise_govdesi(&kapsam, &rota.kontrolcu, &rota.eylem, rota.satir) } else { vec![] };
            sorgu = vec![];
            parametreler = vec![];
            veri = rota.devise.as_deref().and_then(|m| p.model(m, "")).map(|m| VeriBag {
                okur: if matches!(rota.eylem.as_str(), "create" | "update" | "show" | "edit" | "destroy") { vec![m.ad.clone()] } else { vec![] },
                yazar: if matches!(rota.eylem.as_str(), "update" | "destroy") || (rota.eylem == "create" && !rota.kontrolcu.ends_with("sessions")) {
                    vec![m.ad.clone()]
                } else {
                    vec![]
                },
                eksik_olabilir: false,
            });
            yanit = None;
            ozet = Some(if devise { format!("Devise {}#{}", rota.kontrolcu.rsplit('/').next().unwrap_or(""), rota.eylem) } else { format!("{}#{}", rota.kontrolcu, rota.eylem) });
        }
        guardlar.dedup();
        if !notlar.is_empty() {
            let l = denetlenemeyen.entry(yer.clone()).or_default();
            for n in notlar {
                if !l.contains(&n) {
                    l.push(n);
                }
            }
        }
        let id = format!("{} {}", rota.yontem, rota.yol);
        if !rota.kosul.is_empty() && !kosullu_kapilar.contains(&id) {
            kosullu_kapilar.push(id.clone());
        }
        kapi_routerlari.entry(yer.clone()).or_insert_with(|| dugum_kimligi(rota.dugum));
        let kalkmis = ozet.as_deref().is_some_and(|o| o.to_lowercase().contains("deprecated"));
        kapilar.push(Kapi {
            id,
            yontem: rota.yontem.clone(),
            yol: rota.yol.clone(),
            kontrolcu: gorunen_sinif,
            fonksiyon: snake_camel(&rota.eylem),
            dosya,
            satir: satir_,
            giris: giris.into(),
            guardlar,
            roller_kaniti: (!roller.is_empty()).then(|| "varsayim".into()),
            roller,
            ozet: ozet.or_else(|| Some(format!("{}#{}", rota.kontrolcu, rota.eylem))),
            govde,
            sorgu,
            parametreler,
            kullanimdan_kalkmis: kalkmis,
            yerine: None,
            surum: None,
            uygulama: None,
            ana: true,
            giris_kaniti: kanit.into(),
            veri,
            yanit,
        });
    }
    // Aynı yöntem ve yol iki kez tanımlandıysa Rails ilkini kullanır
    let mut gorulen = HashSet::new();
    kapilar.retain(|k| gorulen.insert(k.id.clone()));
    kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
    let routerlar: Vec<Value> = r
        .dugumler
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let ustler: Vec<Value> = d
                .ust
                .map(|u| json!({ "id": dugum_kimligi(u), "icerme": format!("{}:{}", dosyalar[d.di].goreli, d.satir), "kosul": d.kosul, "kosullu": d.kosul.is_some() }))
                .into_iter()
                .collect();
            json!({ "id": dugum_kimligi(i), "dosya": dosyalar[d.di].goreli, "satir": d.satir, "ustler": ustler })
        })
        .collect();
    let ana = paket(kok)["name"].as_str().map(camelize);
    (
        kapilar,
        json!({
            "genel_onek": "", "genel_guardlar": genel_guardlar, "ana_uygulama": ana, "kosullu_kapilar": kosullu_kapilar,
            "denetlenemeyen": denetlenemeyen, "dna": dna_haritasi, "kaskadlar": p.kaskadlar(),
            "routerlar": routerlar, "kapi_routerlari": kapi_routerlari, "veri_modelleri": p.veri_modelleri(),
            "islem_kimlikleri": {}, "olaylar": olaylar(&p),
            "monteler": r.monteler.iter().map(|(h, y, k)| json!({ "hedef": h, "yol": y, "kanit": k })).collect::<Vec<_>>(),
            // Kapıya çevrilemeyen rota tanımları: dinamik kayıt, Rack uygulaması, denetleyicisi ya da eylemi olmayan rota
            "cozulemeyen_rotalar": r.cozulemeyen.iter().chain(eylemsiz.iter()).collect::<Vec<_>>(),
        }),
    )
}

/// Dahili iş düğümleri: ActiveJob / Sidekiq işleri (`job:XJob`, dinleyici `perform`) ve ActionMailer metotları
/// (`job:XMailer.metot`; `deliver_later` bir iştir). Zincirlerinden tetikledikleri ve sildikleri
fn olaylar(p: &Proje) -> BTreeMap<String, Value> {
    let mut v: BTreeMap<String, Value> = BTreeMap::new();
    let mut anahtarlar: Vec<&String> = p.siniflar.keys().collect();
    anahtarlar.sort();
    for a in anahtarlar {
        let is_ = p.soyundan(a, &["ActiveJob::Base", "Sidekiq::Worker", "Sidekiq::Job", "ApplicationJob"]);
        let posta = p.soyundan(a, &["ActionMailer::Base"]);
        if !is_ && !posta {
            continue;
        }
        let tam = p.tam_ad(a);
        let metotlar: Vec<(usize, &Metot)> = p
            .acilislar(a)
            .iter()
            .flat_map(|x| p.sinif(*x).metotlar.iter().map(move |m| (x.0, m)))
            .filter(|(_, m)| if is_ { m.ad == "perform" } else { !m.ozel && !m.sinif_metodu && m.ad != "initialize" })
            .collect();
        for (di, m) in metotlar {
            let id = if is_ { tam.clone() } else { format!("{tam}.{}", m.ad) };
            let mut tipler = HashMap::new();
            let zincir = p.zincir_kur(vec![Uye { di, sinif: a.clone(), metot: m, derinlik: 0, tur: "isleyici" }], &mut tipler);
            let ozet = p.dna_ozeti(&zincir, &tipler, &HashMap::new());
            v.insert(
                format!("job:{id}"),
                json!({
                    "tur": "job", "ad": id,
                    "dinleyiciler": [{ "fonksiyon": format!("{tam}.{}", m.ad), "kanit": format!("{}:{}", p.dosyalar[di].goreli, m.satir) }],
                    "tetikler": ozet["tetikler"], "silinen": ozet["silinen"],
                }),
            );
        }
    }
    v
}
