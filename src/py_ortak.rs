//! Django ve Flask adaptörlerinin ortak Python yardımcıları:
//! - ifade metnini üst düzeyde bölme (metin, yorum ve parantez duyarlı; parçanın ifade içindeki konumuyla, satır için),
//! - metin olarak yazılmış çağrıyı ayrıştırma (`path("a/", views.x, name="x")`),
//! - sınıf zinciri: projedeki taban sınıflar (sırasıyla) ve dışarıdan gelen tabanların tam adları,
//! - yol dönüştürücüleri (`<int:pk>` -> `:pk`),
//! - davranış özeti için sınıf yöntemlerinin zinciri.
use std::collections::HashSet;
use std::path::Path;

use crate::fastapi::{Baglam, Yer};
use crate::guvenli;
use crate::python::{Alan as PyAlan, Fonksiyon, Sinif};

/// Üst düzey bölme: (parçanın ifade içindeki bayt konumu, parça metni). Metinler ('…', "…", '''…''', önekleriyle) ve
/// parantez içi bölünmez; yorumlar (`#` satır sonuna kadar) parçaya girmez. Boş parçalar atılır.
pub(crate) fn bol(s: &str, ayrac: char) -> Vec<(usize, String)> {
    let b: Vec<(usize, char)> = s.char_indices().collect();
    let mut v = vec![];
    let (mut parca, mut bas) = (String::new(), None::<usize>);
    let mut derinlik = 0i32;
    let mut i = 0;
    let bitir = |parca: &mut String, bas: &mut Option<usize>, v: &mut Vec<(usize, String)>| {
        let t = parca.trim().to_string();
        if let (false, Some(o)) = (t.is_empty(), *bas) {
            v.push((o, t));
        }
        parca.clear();
        *bas = None;
    };
    while i < b.len() {
        let (o, c) = b[i];
        match c {
            '#' => {
                while i < b.len() && b[i].1 != '\n' {
                    i += 1;
                }
                continue;
            }
            '\'' | '"' => {
                let uclu = i + 2 < b.len() && b[i + 1].1 == c && b[i + 2].1 == c;
                let n = if uclu { 3 } else { 1 };
                bas.get_or_insert(o);
                for _ in 0..n {
                    parca.push(c);
                }
                i += n;
                while i < b.len() {
                    let x = b[i].1;
                    if x == '\\' {
                        parca.push(x);
                        if let Some(&(_, y)) = b.get(i + 1) {
                            parca.push(y);
                        }
                        i += 2;
                        continue;
                    }
                    if x == c && (!uclu || (i + 2 < b.len() && b[i + 1].1 == c && b[i + 2].1 == c)) {
                        for _ in 0..n {
                            parca.push(c);
                        }
                        i += n;
                        break;
                    }
                    parca.push(x);
                    i += 1;
                }
                continue;
            }
            '(' | '[' | '{' => derinlik += 1,
            ')' | ']' | '}' => derinlik -= 1,
            x if x == ayrac && derinlik == 0 => {
                bitir(&mut parca, &mut bas, &mut v);
                i += 1;
                continue;
            }
            _ => {}
        }
        if !c.is_whitespace() {
            bas.get_or_insert(o);
        }
        parca.push(c);
        i += 1;
    }
    bitir(&mut parca, &mut bas, &mut v);
    v
}

/// Yalnızca metinler: `bol` sonucunun parça metinleri
pub(crate) fn parcalar(s: &str, ayrac: char) -> Vec<String> {
    bol(s, ayrac).into_iter().map(|(_, p)| p).collect()
}

/// Metin olarak yazılmış çağrı: ad, konumsal argümanlar ve anahtarlı argümanlar (konumlarıyla, çağrı metninin başına göre)
#[derive(Debug, Clone, Default)]
pub(crate) struct MetinCagri {
    pub ad: String,
    pub arglar: Vec<(usize, String)>,
    pub anahtarli: Vec<(String, String, usize)>,
}

impl MetinCagri {
    pub(crate) fn arg(&self, i: usize) -> Option<&str> {
        self.arglar.get(i).map(|(_, a)| a.as_str())
    }
    pub(crate) fn anahtar(&self, ad: &str) -> Option<&str> {
        self.anahtarli.iter().find(|(k, _, _)| k == ad).map(|(_, v, _)| v.as_str())
    }
    /// Adın son parçası: `views.x` -> `x`, `serializers.CharField` -> `CharField`
    pub(crate) fn kisa_ad(&self) -> &str {
        self.ad.rsplit('.').next().unwrap_or(&self.ad)
    }
}

/// `ad=değer` biçimindeki argümanı ayırır (`==`, `<=` sayılmaz; sol taraf bir ad olmalı)
fn anahtar_ayir(s: &str) -> Option<(String, String, usize)> {
    let b: Vec<(usize, char)> = s.char_indices().collect();
    for (i, &(o, c)) in b.iter().enumerate() {
        if c == '=' {
            let once = i.checked_sub(1).map(|j| b[j].1);
            let sonra = b.get(i + 1).map(|x| x.1);
            if matches!(once, Some('=' | '!' | '<' | '>')) || sonra == Some('=') {
                return None;
            }
            let ad = s[..o].trim();
            if ad.is_empty() || !ad.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return None;
            }
            let deger = &s[o + 1..];
            let bosluk = deger.len() - deger.trim_start().len();
            return Some((ad.to_string(), deger.trim().to_string(), o + 1 + bosluk));
        }
        if !(c.is_alphanumeric() || c == '_' || c.is_whitespace()) {
            return None;
        }
    }
    None
}

/// "path('a/', views.x, name='x')" -> MetinCagri. İfade bir çağrıyla bitmiyorsa None. Zincirli çağrıda son çağrı
/// ayrıştırılır: `cache_page(60)(views.x)` -> ad "cache_page(60)", argüman "views.x"
pub(crate) fn cagri_ayir(s: &str) -> Option<MetinCagri> {
    let bas = s.len() - s.trim_start().len();
    let t = s.trim();
    if !t.ends_with(')') {
        return None;
    }
    let b: Vec<(usize, char)> = t.char_indices().collect();
    let (mut derinlik, mut son_acilis) = (0i32, None);
    let mut i = 0;
    while i < b.len() {
        let c = b[i].1;
        match c {
            '\'' | '"' => {
                let uclu = i + 2 < b.len() && b[i + 1].1 == c && b[i + 2].1 == c;
                i += if uclu { 3 } else { 1 };
                while i < b.len() {
                    if b[i].1 == '\\' {
                        i += 2;
                        continue;
                    }
                    if b[i].1 == c && (!uclu || (i + 2 < b.len() && b[i + 1].1 == c && b[i + 2].1 == c)) {
                        i += if uclu { 3 } else { 1 };
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            '#' => {
                while i < b.len() && b[i].1 != '\n' {
                    i += 1;
                }
                continue;
            }
            '(' => {
                if derinlik == 0 {
                    son_acilis = Some(b[i].0);
                }
                derinlik += 1;
            }
            '[' | '{' => derinlik += 1,
            ')' | ']' | '}' => derinlik -= 1,
            _ => {}
        }
        i += 1;
    }
    let ac = son_acilis?;
    if derinlik != 0 || ac == 0 {
        return None;
    }
    let ad = t[..ac].trim().to_string();
    if ad.is_empty() || ad.starts_with(['[', '(', '{', '\'', '"']) {
        return None;
    }
    let ic = &t[ac + 1..t.len() - 1];
    let mut c = MetinCagri { ad, ..Default::default() };
    for (o, p) in bol(ic, ',') {
        let konum = bas + ac + 1 + o;
        if p.starts_with("**") {
            continue;
        }
        match anahtar_ayir(&p) {
            Some((k, v, vo)) => c.anahtarli.push((k, v, konum + vo)),
            None => c.arglar.push((konum, p)),
        }
    }
    Some(c)
}

/// İfade metnindeki bir konumun satırı (ifadenin başladığı satıra göre)
pub(crate) fn satir_bul(metin: &str, konum: usize, taban: usize) -> usize {
    taban + metin.get(..konum.min(metin.len())).map(|m| m.matches('\n').count()).unwrap_or(0)
}

/// Liste ya da demet metninin öğeleri: "[a, b]" / "(a, b)" / "a" -> öğeler (konumlarıyla); liste değilse None
pub(crate) fn liste_ogeleri(s: &str) -> Option<Vec<(usize, String)>> {
    let bas = s.len() - s.trim_start().len();
    let t = s.trim();
    let ic = t.strip_prefix('[').and_then(|x| x.strip_suffix(']')).or_else(|| t.strip_prefix('(').and_then(|x| x.strip_suffix(')')))?;
    Some(bol(ic, ',').into_iter().map(|(o, p)| (bas + 1 + o, p)).collect())
}

/// Metin listesi: `["GET", "POST"]` -> metinler (öğelerden biri metin değilse None)
pub(crate) fn metin_listesi(s: &str) -> Option<Vec<String>> {
    let t = s.trim();
    if let Some(m) = crate::fastapi::metin_sabiti(t) {
        return Some(vec![m]);
    }
    let ic = t
        .strip_prefix('[')
        .and_then(|x| x.strip_suffix(']'))
        .or_else(|| t.strip_prefix('(').and_then(|x| x.strip_suffix(')')))
        .or_else(|| t.strip_prefix('{').and_then(|x| x.strip_suffix('}')))?;
    parcalar(ic, ',').iter().map(|p| crate::fastapi::metin_sabiti(p)).collect()
}

/// Sınıf zinciri: projedeki sınıflar (kendisi önce, tabanlar soldan sağa, derinlik öncelikli) ve dışarıdan gelen
/// tabanların tam adları ("rest_framework.generics.ListAPIView")
pub(crate) struct Zincir<'a> {
    pub siniflar: Vec<(usize, &'a Sinif)>,
    pub dis: Vec<String>,
}

impl<'a> Zincir<'a> {
    pub(crate) fn kur(b: &Baglam<'a>, di: usize, s: &'a Sinif) -> Zincir<'a> {
        let mut z = Zincir { siniflar: vec![], dis: vec![] };
        let mut gorulen = HashSet::new();
        z.gez(b, di, s, &mut gorulen, 0);
        z
    }

    fn gez(&mut self, b: &Baglam<'a>, di: usize, s: &'a Sinif, gorulen: &mut HashSet<(usize, String)>, derinlik: usize) {
        if derinlik > 12 || !gorulen.insert((di, s.ad.clone())) {
            return;
        }
        self.siniflar.push((di, s));
        for t in &s.tabanlar {
            let t = t.split(['[', '(']).next().unwrap_or(t).trim();
            match b.ifade(di, t) {
                Some(Yer::Dis(m, a)) => self.dis.push(format!("{m}.{a}")),
                Some(y) => match b.sinif(&y) {
                    Some((d2, s2)) => self.gez(b, d2, s2, gorulen, derinlik + 1),
                    None => self.dis.push(t.to_string()),
                },
                None => self.dis.push(t.to_string()),
            }
        }
    }

    /// Dışarıdan gelen tabanlardan biri bu adlardan biri mi (adın son parçasıyla)
    pub(crate) fn dis_var(&self, adlar: &[&str]) -> bool {
        self.dis.iter().any(|d| adlar.contains(&d.rsplit('.').next().unwrap_or(d)))
    }

    /// Dışarıdan gelen tabanların son parçaları
    pub(crate) fn dis_adlar(&self) -> Vec<&str> {
        self.dis.iter().map(|d| d.rsplit('.').next().unwrap_or(d)).collect()
    }

    /// Zincirde tanımlı ilk yöntem: (dosya, yöntem, sınıf)
    pub(crate) fn metot(&self, ad: &str) -> Option<(usize, &'a Fonksiyon, &'a Sinif)> {
        self.siniflar.iter().find_map(|(di, s)| s.metotlar.iter().find(|m| m.ad == ad).map(|m| (*di, m, *s)))
    }

    /// Zincirde atanmış ilk sınıf özniteliği: (dosya, öznitelik)
    pub(crate) fn oznitelik(&self, ad: &str) -> Option<(usize, &'a PyAlan)> {
        self.siniflar.iter().find_map(|(di, s)| s.alanlar.iter().find(|a| a.ad == ad).map(|a| (*di, a)))
    }

    /// Zincirdeki ilk iç sınıf (`Meta`)
    pub(crate) fn ic_sinif(&self, ad: &str) -> Option<(usize, &'a Sinif)> {
        self.siniflar.iter().find_map(|(di, s)| s.ic_siniflar.iter().find(|i| i.ad == ad).map(|i| (*di, i)))
    }
}

/// Yol şablonundaki dönüştürücüler (`<int:pk>`, `<slug>`, Flask `<any(a, b):x>`): ":ad" biçimine çevrilmiş yol ve
/// parametreler (ad, tip)
pub(crate) fn donusturuculu_yol(yol: &str) -> (String, Vec<(String, String)>) {
    let mut out = String::new();
    let mut params = vec![];
    let mut kalan = yol;
    while let Some(i) = kalan.find('<') {
        let Some(j) = kalan[i..].find('>').map(|j| i + j) else { break };
        out.push_str(&kalan[..i]);
        let ic = &kalan[i + 1..j];
        let (donusturucu, ad) = match ic.rsplit_once(':') {
            Some((d, a)) => (d.split('(').next().unwrap_or(d).trim(), a.trim()),
            None => ("str", ic.trim()),
        };
        let tip = match donusturucu {
            "int" => "int",
            "float" => "number",
            "uuid" => "UUID",
            _ => "str",
        };
        out.push(':');
        out.push_str(ad);
        params.push((ad.to_string(), tip.to_string()));
        kalan = &kalan[j + 1..];
    }
    out.push_str(kalan);
    (out, params)
}

/// İki yol parçasını birleştirir; sonuç `/` ile başlar, `//` olmaz
pub(crate) fn yol_ekle(a: &str, b: &str) -> String {
    let mut s = format!("{}/{}", a.trim_end_matches('/'), b.trim_start_matches('/'));
    if b.is_empty() && a.ends_with('/') {
        s = a.to_string();
    }
    let s = if s.starts_with('/') { s } else { format!("/{s}") };
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '/' && out.ends_with('/') {
            continue;
        }
        out.push(c);
    }
    out
}

/// Sınıfın yöntemlerinden (ve zincirden) davranış özeti: işleyici yöntem, `self.x()` ile çağrılan yöntemler (servis) ve
/// adıyla çağrılan proje fonksiyonları (2 kat)
pub(crate) fn yontem_dna(b: &Baglam, kok: &Path, z: &Zincir, ilk: Option<(usize, &Fonksiyon)>) -> serde_json::Value {
    let Some((fdi, f)) = ilk else { return serde_json::json!({}) };
    let mut halkalar = vec![];
    let mut gorulen = HashSet::new();
    let mut yigin: Vec<(usize, &Fonksiyon, usize)> = vec![(fdi, f, 0)];
    while let Some((di, g, derinlik)) = yigin.pop() {
        if !gorulen.insert((di, g.ad.clone(), g.satir)) || halkalar.len() >= 32 {
            continue;
        }
        halkalar.push(crate::dna::Halka { dosya: guvenli::goreli(kok, &b.dosyalar[di].yol), dna: &g.dna, derinlik, tur: if derinlik == 0 { "isleyici" } else { "servis" } });
        if derinlik >= 2 {
            continue;
        }
        let mut sonraki = vec![];
        for ad in &g.dna.cagrilar {
            if let Some(m) = ad.strip_prefix("self.").filter(|m| !m.contains('.')) {
                if let Some((d2, g2, _)) = z.metot(m) {
                    sonraki.push((d2, g2, derinlik + 1));
                }
                continue;
            }
            if let Some((d2, g2)) = b.ifade(di, ad).and_then(|y| b.fonksiyon(&y)) {
                sonraki.push((d2, g2, derinlik + 1));
            }
        }
        yigin.extend(sonraki.into_iter().rev());
    }
    crate::dna::ozet(&halkalar)
}

/// Gövdede `request.method == "POST"` / `request.method in ("POST", "PUT")` karşılaştırmalarında geçen yöntemler
pub(crate) fn govdedeki_yontemler(govde: &str) -> Vec<String> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r#"request\.method\s*(?:==|!=|in|not\s+in)\s*(\([^)]*\)|\[[^\]]*\]|\{[^}]*\}|"[A-Za-z]+"|'[A-Za-z]+')"#).unwrap());
    let mut v: Vec<String> = vec![];
    for c in re.captures_iter(govde) {
        for m in metin_listesi(&c[1]).unwrap_or_default() {
            let m = m.to_uppercase();
            if matches!(m.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") && !v.contains(&m) {
                v.push(m);
            }
        }
    }
    v
}

/// Yöntem süzen gövde: `request.method` karşılaştırılıyor ve 405 (Method Not Allowed) dönülüyor. Projedeki dekoratör
/// böyleyse (`@allow_methods("POST")`) çağrı argümanlarındaki yöntemler kabul edilen yöntemlerdir
pub(crate) fn yontem_suzen_govde(t: &str) -> bool {
    t.contains("request.method") && (t.contains("405") || t.contains("NotAllowed"))
}

/// Gövdenin başındaki yöntem koruması: `if request.method != "POST": return HttpResponseNotAllowed(["POST"])` ya da
/// `if request.method not in ("GET", "POST"): ... 405` -> kabul edilen yöntemler. Koruma 405 ile bitmiyorsa None
pub(crate) fn govdedeki_yontem_kisiti(govde: &str) -> Option<Vec<String>> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r#"request\.method\s*(?:!=|not\s+in)\s*(\([^)]*\)|\[[^\]]*\]|\{[^}]*\}|"[A-Za-z]+"|'[A-Za-z]+')\s*:"#).unwrap());
    let c = re.captures(govde)?;
    let sonra: String = govde[c.get(0)?.end()..].chars().take(240).collect();
    let ret = sonra.split("\n\n").next().unwrap_or("");
    if !(ret.contains("405") || ret.contains("NotAllowed")) {
        return None;
    }
    let v: Vec<String> =
        metin_listesi(&c[1]).unwrap_or_default().iter().map(|m| m.to_uppercase()).filter(|m| matches!(m.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE")).collect();
    (!v.is_empty()).then_some(v)
}

/// Kimlik bilgisi denetleyen gövde (özel kimlik doğrulama dekoratörü): istekten (başlık, META, sorgu, gövde) API anahtarı
/// ya da belirteç okuyor ve 401 / 403 dönüyor ya da yetki hatası fırlatıyor. Kalıptan çıkan karar kesin değildir
pub(crate) fn kimlik_bilgisi_denetimi(t: &str) -> bool {
    static D: std::sync::OnceLock<[regex::Regex; 2]> = std::sync::OnceLock::new();
    let [okuma, kimlik] = D.get_or_init(|| {
        [
            regex::Regex::new(r"request\.(?:headers|META|args|GET|POST|json|query_params|data|cookies|values|form)\b|HTTP_AUTHORIZATION|\bheaders\.get\(").unwrap(),
            regex::Regex::new(r#"(?i)api[_-]?key|apikey|\btoken\b|authorization|\bsecret\b|signature|["']x-[a-z-]*key["']|[a-z]+_key\b"#).unwrap(),
        ]
    });
    okuma.is_match(t)
        && kimlik.is_match(t)
        && ["401", "403", "Unauthorized", "Forbidden", "PermissionDenied", "AuthenticationFailed", "NotAuthenticated"].iter().any(|x| t.contains(x))
}
