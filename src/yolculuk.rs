//! Kullanıcı yolculukları: ön yüz bağlarından (ekran → UI eylemi → istemci çağrısı → API) ve yönlendirmeden
//! `Journey` düğümleri ve `STEP` ilişkileri. Örnek: "/login › Log In → /items › Save → POST /api/v1/items/".
//!
//! - Adımlar: işlemin ekranı ve eylemi; işlem giriş istiyorsa önce giriş ekranı ve giriş eylemi (giriş ucu: herkese açık,
//!   POST, yolu login/signin/token/auth/session içeren; ona ulaşan UI eylemi ve onu gösteren ekran).
//! - Güven: statik yolculuk ve adımları en fazla inferred (koşullu gösterim, yönlendirme ve oturum durumu kaynaktan
//!   kesin bilinmez). Gözlem, yolculuğun istek sırasını (giriş ucu, sonra işlem) gördüyse yolculuğun kendisi observed
//!   olur; UI adımları gözlenmediği için inferred kalır (HAR adım bilgisi taşımaz).
//! - Yolculuk kimliği anlam anahtarından: ekran, eylem ve işlem aynı kaldıkça aynıdır.

use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Çözülmüş ön yüz bağı: dosya uygulama köküne göre
#[derive(Clone, Debug)]
pub(crate) struct Kenar {
    pub kaynak: String,
    pub iliski: String,
    pub hedef: String,
    pub dosya: String,
    pub satir: u64,
}

/// Haritanın ucu: yöntem, yol, giriş gereksinimi
pub(crate) struct UcBilgi {
    pub yontem: String,
    pub yol: String,
    pub giris: String,
}

pub(crate) struct Yolculuk {
    pub id: String,
    pub ad: String,
    /// Yolculuğun işlemi (`api:`)
    pub hedef: String,
    /// Giriş ucu (işlem giriş istiyorsa ve giriş eylemi bulunduysa)
    pub giris: Option<String>,
    /// Adımlar sırasıyla (ekran ve UI eylemi düğümleri)
    pub adimlar: Vec<String>,
    /// Yolculuğu kuran bağların kaynakları (dosya, satır)
    pub konumlar: Vec<(String, u64)>,
}

/// Yolculuk sayısı sınırı (büyük uygulamada kart ve graf şişmesin)
const EN_COK: usize = 300;

/// UI eylemi -> statik olarak ulaştığı uçlar (TRIGGERS ile istemci çağrısı, CALLS ile uç)
pub(crate) fn ui_hedefleri(kenarlar: &[Kenar]) -> BTreeMap<String, BTreeSet<String>> {
    let mut cagri: HashMap<&str, Vec<&str>> = HashMap::new();
    for k in kenarlar.iter().filter(|k| k.iliski == "CALLS" && k.hedef.starts_with("api:")) {
        cagri.entry(k.kaynak.as_str()).or_default().push(k.hedef.as_str());
    }
    let mut v: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for k in kenarlar.iter().filter(|k| k.iliski == "TRIGGERS" && k.kaynak.starts_with("ui:")) {
        if k.hedef.starts_with("api:") {
            v.entry(k.kaynak.clone()).or_default().insert(k.hedef.clone());
        }
        for a in cagri.get(k.hedef.as_str()).into_iter().flatten() {
            v.entry(k.kaynak.clone()).or_default().insert(a.to_string());
        }
    }
    v
}

/// Ekranın okunur adı: `screen:web//items` -> "/items"
fn ekran_adi(id: &str) -> String {
    let a = id.strip_prefix("screen:").unwrap_or(id);
    let a = a.split_once('/').map(|(_, y)| y).unwrap_or(a);
    if a.is_empty() {
        "/".into()
    } else {
        a.to_string()
    }
}

/// Eylemin okunur adı: `ui:src/A.tsx#button:Save` -> "Save"
fn eylem_adi(id: &str) -> String {
    let e = id.rsplit_once('#').map(|(_, e)| e).unwrap_or(id);
    e.split_once(':').map(|(_, a)| a).unwrap_or(e).to_string()
}

/// Yükleme (ekran açılışında çağrılan sorgu) eylemi yolculuk adımı sayılmaz
fn yukleme_mi(ui: &str) -> bool {
    ui.rsplit_once('#').is_some_and(|(_, e)| e.starts_with("yukleme:"))
}

fn giris_ucu_mu(u: &UcBilgi) -> bool {
    let y = u.yol.to_ascii_lowercase();
    u.yontem == "POST" && u.giris != "gerekli" && ["login", "signin", "sign-in", "sign_in", "token", "auth", "session"].iter().any(|s| y.contains(s))
}

/// Statik yolculuklar
pub(crate) fn kur(kenarlar: &[Kenar], uclar: &HashMap<String, UcBilgi>) -> Vec<Yolculuk> {
    let hedefler = ui_hedefleri(kenarlar);
    // Ekran -> gösterdiği eylemler; eylem -> onu gösteren ekranlar ve kanıt konumu
    let mut gosteren: BTreeMap<&str, Vec<(&str, &Kenar)>> = BTreeMap::new();
    for k in kenarlar.iter().filter(|k| k.iliski == "DISPLAYS" && k.kaynak.starts_with("screen:") && k.hedef.starts_with("ui:")) {
        gosteren.entry(k.hedef.as_str()).or_default().push((k.kaynak.as_str(), k));
    }
    let konum_bul = |iliski: &str, kaynak: &str| kenarlar.iter().filter(|k| k.iliski == iliski && k.kaynak == kaynak).map(|k| (k.dosya.clone(), k.satir)).collect::<Vec<_>>();
    // Giriş adımı: giriş ucuna ulaşan ilk UI eylemi ve onu gösteren ilk ekran (sıralı, belirlenimci)
    let giris: Option<(&str, &str, &str, &Kenar)> = hedefler.iter().find_map(|(ui, apiler)| {
        let api = apiler.iter().find(|a| uclar.get(a.as_str()).is_some_and(giris_ucu_mu))?;
        let (ekran, k) = gosteren.get(ui.as_str())?.first()?;
        Some((*ekran, ui.as_str(), api.as_str(), *k))
    });
    let gecisler: Vec<&Kenar> = kenarlar.iter().filter(|k| k.iliski == "NAVIGATES" && k.kaynak.starts_with("screen:") && k.hedef.starts_with("screen:")).collect();
    let mut v = vec![];
    for (ui, apiler) in &hedefler {
        if yukleme_mi(ui) {
            continue;
        }
        for (ekran, gk) in gosteren.get(ui.as_str()).into_iter().flatten() {
            for api in apiler {
                let Some(u) = uclar.get(api.as_str()) else { continue };
                let mut adimlar = vec![];
                let mut konumlar = vec![];
                let mut adlar = vec![];
                let mut giris_ucu = None;
                if let Some((ge, gu, ga, gkk)) = giris.filter(|g| u.giris == "gerekli" && g.0 != *ekran && g.1 != ui.as_str()) {
                    adimlar.extend([ge.to_string(), gu.to_string()]);
                    konumlar.push((gkk.dosya.clone(), gkk.satir));
                    konumlar.extend(konum_bul("TRIGGERS", gu));
                    adlar.push(format!("{} › {}", ekran_adi(ge), eylem_adi(gu)));
                    giris_ucu = Some(ga.to_string());
                    // Girişten işlemin ekranına ekran geçişleriyle (en kısa yol): aradaki ekranlar da adımdır
                    for (ara, k) in gecis_yolu(&gecisler, ge, ekran) {
                        adimlar.push(ara.to_string());
                        konumlar.push((k.dosya.clone(), k.satir));
                        adlar.push(ekran_adi(ara));
                    }
                }
                adimlar.extend([ekran.to_string(), ui.clone()]);
                konumlar.push((gk.dosya.clone(), gk.satir));
                konumlar.extend(konum_bul("TRIGGERS", ui));
                adlar.push(format!("{} › {}", ekran_adi(ekran), eylem_adi(ui)));
                adlar.push(format!("{} {}", u.yontem, u.yol));
                konumlar.sort();
                konumlar.dedup();
                let anahtar = crate::kanit::sha256_hex(format!("{ekran}\u{0}{ui}\u{0}{api}").as_bytes());
                let id = format!("journey:web{}#{}@{}", ekran_adi(ekran), eylem_adi(ui), &anahtar[..8]);
                v.push(Yolculuk { id, ad: adlar.join(" → "), hedef: api.clone(), giris: giris_ucu, adimlar, konumlar });
                if v.len() >= EN_COK {
                    return v;
                }
            }
        }
    }
    v
}

/// İki ekran arasında ekran geçişleriyle en kısa yolun ara ekranları (başlangıç ve hedef hariç) ve son kenara kadar
/// her ara ekrana giren kenar; yol yoksa ya da 4 adımdan uzunsa boş
fn gecis_yolu<'a>(gecisler: &[&'a Kenar], bas: &'a str, hedef: &'a str) -> Vec<(&'a str, &'a Kenar)> {
    let mut onceki: HashMap<&str, (&str, &Kenar)> = HashMap::new();
    let mut sira: std::collections::VecDeque<(&str, usize)> = std::collections::VecDeque::from([(bas, 0)]);
    let mut bulundu = false;
    while let Some((x, d)) = sira.pop_front() {
        if x == hedef {
            bulundu = true;
            break;
        }
        if d >= 4 {
            continue;
        }
        for k in gecisler.iter().filter(|k| k.kaynak == x) {
            let h = k.hedef.as_str();
            if h != bas && !onceki.contains_key(h) {
                onceki.insert(h, (x, k));
                sira.push_back((h, d + 1));
            }
        }
    }
    if !bulundu {
        return vec![];
    }
    // Hedeften geriye: her ara ekran ve ona giren kenar
    let mut yol = vec![];
    let mut x = hedef;
    while let Some((o, k)) = onceki.get(x) {
        if x != hedef {
            yol.push((x, *k));
        }
        x = o;
    }
    yol.reverse();
    yol
}

/// Gözlenen istek sırası yolculuğu içeriyor mu: (giriş ucu varsa önce o) sonra işlem
pub(crate) fn sirada(y: &Yolculuk, sira: &[String]) -> bool {
    match &y.giris {
        Some(g) => sira.iter().position(|a| a == g).is_some_and(|i| sira[i + 1..].contains(&y.hedef)),
        None => sira.contains(&y.hedef),
    }
}
