//! Çalışma zamanı gözlemi (ağ yakalaması): HAR içe aktarma (`sezgi gozlem al <dosya.har>`) ve canlı modda `cagir` ile
//! yapılan istekler. Yalnızca BİÇİM saklanır: yöntem, yol şablonu (sayısal, uuid ve değer görünümlü parçalar `:param`),
//! durum kodu, istek ve yanıt gövdesinin üst düzey alan ADLARI ve JSON tipleri, sorgu parametresi adları. Değer
//! saklanmaz: gövde ve sorgu değerleri, başlıklar (Authorization dahil), çerezler, belirteçler, e-posta. Ad gibi
//! görünmeyen anahtarlar (e-posta, belirteç, kimlik, uzun ya da rastgele görünen) yalnızca sayılır (`dinamik_anahtar`).
//! HAR dosyasının kendisi saklanmaz, yalnızca SHA-256 özeti; HAR akışla okunur (bir girdi bellekte).
//!
//! Gözlemler `.sezgi/gozlem.json`'da durur; kanıt deposu her kurulumda onlarla birlikte kurulur (`kanitlara`):
//! - haritadaki uca eşleşen istek → NETWORK_CAPTURE kanıtı (tavan observed): uç çalışıyor (`CALISIR`, 2xx/3xx), hata
//!   durumu (`HATA_DURUMU`, 4xx/5xx), yanıt biçimi (`YANIT`). Başarılı isteğin gövdesi koddaki girdi şemasıyla
//!   karşılaştırılır: zorunlu alan gönderilmeden kabul ya da tip farkı çelişkidir (önerme en fazla inferred olur);
//! - haritada olmayan gözlenen uç → bilinmeyen: "kodda bulunamayan gözlenen uç";
//! - zaman işaretiyle UI eylemi eşlemesi (`--isaretler`) yalnızca inferred: HAR adım bilgisi taşımaz.
//!
//! - tarayıcı senaryosu (`sezgi gozlem senaryo`, `senaryo` modülü): REA'nın adım ↔ istek kimliği bağı. Bağlanan UI
//!   adımı → uç `TRIGGERS` kenarı ve yolculuk adımı BROWSER_OBSERVATION kanıtıyla observed olur (yalıtım beyanı yoksa
//!   inferred); yalnızca REA'nın isteği başlatan adım olarak kaydettiği adımlar.
//!
//! Gözlem kanıtının dayanağı eşleşen ucun kaynak dosyasıdır (gözlem anındaki özetiyle): kod değişince gözlem eski sayılır.

use crate::kanit::{Dayanak, Guven, Iddia, Kanit, Kapsam, KaynakTuru, Kurucu};
use serde::de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub const DOSYA: &str = "gozlem.json";
pub const BICIM: u64 = 1;
/// Bir kaynaktan en çok okunan istek
const EN_COK_ISTEK: usize = 200_000;
/// Alanları çıkarılan en büyük gövde (bayt); daha büyüğünün yalnızca biçimi (tür) yazılır
const EN_BUYUK_GOVDE: usize = 8 << 20;
const EN_COK_ALAN: usize = 200;
/// Yolculuk gözlemi için saklanan uç sırasının uzunluğu
const EN_UZUN_SIRA: usize = 5000;
/// Zaman işaretinden sonraki isteğin eşlendiği pencere
const PENCERE_MS: i64 = 2000;

/// Bir isteğin değer taşımayan biçimi
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bicim {
    pub yontem: String,
    /// Yol şablonu: eşleşen ucun yolu ya da değer görünümlü parçaları `:param` yapılmış gözlenen yol
    pub yol: String,
    /// Eşleşen uç (`api:` kimliği); haritada yoksa None
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    pub durum: u16,
    /// Alan adı -> JSON tipi (string, number, boolean, null, object, array; form gövdesinde "form")
    #[serde(default)]
    pub istek_alanlari: BTreeMap<String, String>,
    #[serde(default)]
    pub sorgu_alanlari: Vec<String>,
    #[serde(default)]
    pub yanit_alanlari: BTreeMap<String, String>,
    #[serde(default)]
    pub yanit_dizi: bool,
    /// Ad gibi görünmediği için yazılmayan anahtar sayısı
    #[serde(default)]
    pub dinamik_anahtar: u32,
}

/// Aynı biçimdeki isteklerin toplamı
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Kayit {
    /// "har:<sha256>" ya da "canli"
    pub kaynak: String,
    #[serde(flatten)]
    pub bicim: Bicim,
    pub adet: u64,
    /// Eşleşen ucun gözlem anındaki kaynak dosyası ve özeti
    #[serde(default)]
    pub dayanak: Vec<Dayanak>,
}

/// Bir gözlem kaynağı (HAR dosyası ya da canlı mod)
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Kaynak {
    pub kimlik: String,
    pub tur: String,
    pub zaman: String,
    pub istek: u64,
    pub eslesen: u64,
    /// Başarılı (2xx/3xx) eşleşen uçların gözlenme sırası; ardışık tekrar tek (yolculuk gözlemi)
    #[serde(default)]
    pub sira: Vec<String>,
    /// Zaman işareti → istek eşlemeleri: {isaret, api, fark_ms}
    #[serde(default)]
    pub zaman_eslesmeleri: Vec<Value>,
    /// Tarayıcı senaryosu: REA'nın bağladığı adım → istek: {adim, eylem, rol, etiket, api, yontem, yol, durum}
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adim_baglari: Vec<Value>,
    /// Senaryonun yalıtım beyanı (konteyner, vm, yok)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yalitim: Option<String>,
}

/// gozlem.json
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Gozlemler {
    pub bicim: u64,
    #[serde(default)]
    pub kaynaklar: Vec<Kaynak>,
    #[serde(default)]
    pub kayitlar: Vec<Kayit>,
}

impl Gozlemler {
    pub fn oku(klasor: &Path) -> Option<Gozlemler> {
        crate::guvenli::oku_harita(&klasor.join(DOSYA)).and_then(|m| serde_json::from_str::<Gozlemler>(&m).ok()).filter(|g| g.bicim == BICIM)
    }

    pub(crate) fn yaz(&self, klasor: &Path) -> Result<(), String> {
        crate::guvenli::atomik_yaz(&klasor.join(DOSYA), serde_json::to_string_pretty(self).map_err(|e| e.to_string())?.as_bytes())
    }

    /// Kaynağın kayıtlarını ekler: aynı kaynak yeniden alınırsa öncekinin yerine geçer (aynı HAR iki kez sayılmaz)
    pub(crate) fn kaynak_koy(&mut self, k: Kaynak, kayitlar: Vec<Kayit>) {
        self.kaynaklar.retain(|x| x.kimlik != k.kimlik);
        self.kayitlar.retain(|x| x.kaynak != k.kimlik);
        self.kaynaklar.push(k);
        self.kayitlar.extend(kayitlar);
    }
}

// ---- Değer süzgeci ----

fn uuid_mu(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() == 36 && b.iter().enumerate().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { *c == b'-' } else { c.is_ascii_hexdigit() })
}

/// Yol parçası ya da anahtar bir değere mi benziyor: sayı, uuid, uzun onaltılık, e-posta, belirteç, uzun ya da harf ve
/// rakam karışık rastgele görünen dizgi
fn deger_gibi(p: &str) -> bool {
    let n = p.chars().count();
    if n == 0 {
        return false;
    }
    let rakam = p.chars().filter(char::is_ascii_digit).count();
    let harf = p.chars().filter(|c| c.is_ascii_alphabetic()).count();
    p.chars().all(|c| c.is_ascii_digit())
        || uuid_mu(p)
        || (n >= 16 && p.chars().all(|c| c.is_ascii_hexdigit()))
        || p.contains('@')
        || p.contains('%')
        || n > 40
        || (p.starts_with("eyJ") && n > 16)
        || p.matches('.').count() >= 2 && n > 20
        || (n >= 20 && rakam >= 3 && harf >= 3)
        || (n >= 10 && rakam * 2 >= n)
}

/// Alan adı olarak yazılabilir mi: kısa, tanımlayıcı karakterli ve değer gibi görünmeyen
fn ad_mi(k: &str) -> bool {
    let n = k.chars().count();
    (1..=64).contains(&n)
        && k.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && k.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '$' | '[' | ']'))
        && !deger_gibi(k)
}

/// Gözlenen yolun şablonu: değer görünümlü parçalar `:param`
pub fn sablon(yol: &str) -> String {
    let s: Vec<String> = yol.split('/').map(|p| if deger_gibi(p) { ":param".to_string() } else { crate::guvenli::temiz(p) }).collect();
    let s = s.join("/");
    if s.is_empty() {
        "/".into()
    } else {
        s
    }
}

fn tip(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// JSON gövdesinin üst düzey alan adları ve tipleri; dizi ise ilk nesne öğelerinin alanları. (alanlar, dizi mi,
/// yazılmayan anahtar)
fn json_alanlari(v: &Value) -> (BTreeMap<String, String>, bool, u32) {
    let mut alanlar = BTreeMap::new();
    let mut dinamik = 0u32;
    let mut ekle = |o: &serde_json::Map<String, Value>| {
        for (k, x) in o {
            if !ad_mi(k) {
                dinamik += 1;
            } else if alanlar.len() < EN_COK_ALAN {
                alanlar.entry(k.clone()).or_insert_with(|| tip(x).to_string());
            }
        }
    };
    match v {
        Value::Object(o) => {
            ekle(o);
            (alanlar, false, dinamik)
        }
        Value::Array(a) => {
            for o in a.iter().take(5).filter_map(Value::as_object) {
                ekle(o);
            }
            (alanlar, true, dinamik)
        }
        _ => (alanlar, false, dinamik),
    }
}

/// `a=1&b=2` biçiminin adları (değerler atılır)
fn form_adlari(metin: &str) -> (Vec<String>, u32) {
    let mut adlar = BTreeSet::new();
    let mut dinamik = 0u32;
    for p in metin.split('&').filter(|p| !p.is_empty()) {
        let ad = p.split('=').next().unwrap_or("").replace('+', " ");
        if ad_mi(&ad) {
            adlar.insert(ad);
        } else {
            dinamik += 1;
        }
    }
    (adlar.into_iter().take(EN_COK_ALAN).collect(), dinamik)
}

/// Adres: (ana makine ve kapı, yol, sorgu)
fn adres_parcala(url: &str) -> (String, String, String) {
    let (ana, kalan) = match url.find("://") {
        Some(i) => {
            let s = &url[i + 3..];
            match s.find('/') {
                Some(j) => (s[..j].to_string(), &s[j..]),
                None => (s.to_string(), "/"),
            }
        }
        None => (String::new(), url),
    };
    let kalan = kalan.split('#').next().unwrap_or("");
    let (yol, sorgu) = kalan.split_once('?').unwrap_or((kalan, ""));
    // Kullanıcı adı ve parola ana makineden atılır
    let ana = ana.rsplit('@').next().unwrap_or("").to_ascii_lowercase();
    (ana, yol.to_string(), sorgu.to_string())
}

/// Bir isteğin gövdesi (mime, metin, form parametre adları) ve yanıtı (durum, mime, metin) -> biçim alanları
struct HamIstek<'a> {
    yontem: &'a str,
    yol: &'a str,
    sorgu: &'a str,
    durum: u16,
    istek_mime: &'a str,
    istek_metni: Option<&'a [u8]>,
    istek_parametreleri: &'a [String],
    yanit_mime: &'a str,
    yanit_metni: Option<&'a [u8]>,
}

fn bicimle(h: &HamIstek, api: Option<(&str, &str)>) -> Bicim {
    let mut b = Bicim {
        yontem: h.yontem.to_ascii_uppercase(),
        yol: api.map(|(_, y)| y.to_string()).unwrap_or_else(|| sablon(h.yol)),
        api: api.map(|(a, _)| a.to_string()),
        durum: h.durum,
        ..Bicim::default()
    };
    let (sorgu, d) = form_adlari(h.sorgu);
    b.sorgu_alanlari = sorgu;
    b.dinamik_anahtar += d;
    let im = h.istek_mime.to_ascii_lowercase();
    if im.contains("json") {
        if let Some(v) = h.istek_metni.filter(|m| m.len() <= EN_BUYUK_GOVDE).and_then(|m| serde_json::from_slice::<Value>(m).ok()) {
            let (a, _, d) = json_alanlari(&v);
            b.istek_alanlari = a;
            b.dinamik_anahtar += d;
        }
    } else if im.contains("x-www-form-urlencoded") || im.contains("multipart") {
        let (adlar, d) = if !h.istek_parametreleri.is_empty() {
            let (mut a, mut d) = (BTreeSet::new(), 0u32);
            for p in h.istek_parametreleri {
                if ad_mi(p) {
                    a.insert(p.clone());
                } else {
                    d += 1;
                }
            }
            (a.into_iter().collect::<Vec<_>>(), d)
        } else if im.contains("x-www-form-urlencoded") {
            form_adlari(&String::from_utf8_lossy(h.istek_metni.unwrap_or_default()))
        } else {
            (vec![], 0)
        };
        b.istek_alanlari = adlar.into_iter().map(|a| (a, "form".to_string())).collect();
        b.dinamik_anahtar += d;
    }
    if h.yanit_mime.to_ascii_lowercase().contains("json") {
        if let Some(v) = h.yanit_metni.filter(|m| m.len() <= EN_BUYUK_GOVDE).and_then(|m| serde_json::from_slice::<Value>(m).ok()) {
            let (a, dizi, d) = json_alanlari(&v);
            b.yanit_alanlari = a;
            b.yanit_dizi = dizi;
            b.dinamik_anahtar += d;
        }
    }
    b
}

// ---- Uç eşleme ----

/// Haritanın ucu: kimlik, yöntem, yol ve parçaları, kaynak dosyası
pub(crate) struct Uc {
    pub api: String,
    yontem: String,
    yol: String,
    parcalar: Vec<String>,
    pub dosya: String,
}

pub(crate) fn uclar(harita: &Value) -> Vec<Uc> {
    harita["endpointler"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| {
            let yol = k["yol"].as_str().unwrap_or("").to_string();
            Uc {
                api: crate::graf::api_kimligi(k),
                yontem: k["yontem"].as_str().unwrap_or("").to_ascii_uppercase(),
                parcalar: crate::santral::parcalar(&yol),
                yol,
                dosya: k["dosya"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect()
}

/// Gözlenen isteğin ucu: aynı yöntem, yol parçaları eşleşen (değer ↔ parametre); birden fazlaysa en çok sabit parçası
/// birebir tutan
pub(crate) fn eslestir<'a>(uclar: &'a [Uc], yontem: &str, yol: &str) -> Option<&'a Uc> {
    let p = crate::santral::parcalar(yol);
    let mut en: Option<(usize, &Uc)> = None;
    for u in uclar.iter().filter(|u| u.yontem.eq_ignore_ascii_case(yontem)) {
        if crate::santral::eslesme(&p, &u.parcalar).is_none() {
            continue;
        }
        let sabit = p.iter().zip(&u.parcalar).filter(|(a, b)| a.eq_ignore_ascii_case(b)).count();
        if en.is_none_or(|(s, _)| sabit > s) {
            en = Some((sabit, u));
        }
    }
    en.map(|(_, u)| u)
}

/// Statik dosya yolu (betik, stil, görsel, yazı tipi): uç sayılmaz
pub(crate) fn statik_mi(yol: &str) -> bool {
    let son = yol.rsplit('/').next().unwrap_or("");
    son.rsplit_once('.').is_some_and(|(_, u)| {
        matches!(
            u.to_ascii_lowercase().as_str(),
            "js" | "mjs" | "css" | "map" | "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" | "woff" | "woff2" | "ttf" | "html" | "txt" | "json" | "webmanifest"
        )
    })
}

// ---- HAR (akışla) ----

#[derive(Deserialize, Default)]
#[serde(default)]
struct HarGirdi {
    #[serde(rename = "startedDateTime")]
    baslangic: String,
    request: HarIstek,
    response: HarYanit,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct HarIstek {
    method: String,
    url: String,
    #[serde(rename = "postData")]
    post: Option<HarGovde>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct HarGovde {
    #[serde(rename = "mimeType")]
    mime: String,
    text: Option<String>,
    params: Vec<HarParametre>,
}

/// Form parametresi: yalnızca adı okunur (değer okunmaz)
#[derive(Deserialize, Default)]
#[serde(default)]
struct HarParametre {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct HarYanit {
    status: i64,
    content: HarIcerik,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct HarIcerik {
    #[serde(rename = "mimeType")]
    mime: String,
    text: Option<String>,
    encoding: Option<String>,
}

type Isleyici<'a> = &'a mut dyn FnMut(HarGirdi) -> Result<(), String>;

/// `{"log": {"entries": [...]}}`: girdiler tek tek işlenir, bütün dosya belleğe alınmaz
struct HarKok<'a>(Isleyici<'a>);
struct HarLog<'a>(Isleyici<'a>);
struct HarGirdiler<'a>(Isleyici<'a>);

impl<'de> DeserializeSeed<'de> for HarKok<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for HarKok<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("HAR nesnesi")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<(), A::Error> {
        let f = self.0;
        while let Some(k) = m.next_key::<String>()? {
            if k == "log" {
                m.next_value_seed(HarLog(&mut *f))?;
            } else {
                m.next_value::<IgnoredAny>()?;
            }
        }
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for HarLog<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for HarLog<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("HAR log nesnesi")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<(), A::Error> {
        let f = self.0;
        while let Some(k) = m.next_key::<String>()? {
            if k == "entries" {
                m.next_value_seed(HarGirdiler(&mut *f))?;
            } else {
                m.next_value::<IgnoredAny>()?;
            }
        }
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for HarGirdiler<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for HarGirdiler<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("HAR girdi dizisi")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut s: A) -> Result<(), A::Error> {
        while let Some(g) = s.next_element::<HarGirdi>()? {
            (self.0)(g).map_err(A::Error::custom)?;
        }
        Ok(())
    }
}

/// RFC 3339 zaman -> Unix milisaniye
fn zaman_ms(m: &str) -> Option<i64> {
    let sn = crate::benlik::zaman_sn(m)?;
    let kesir: String = m.get(19..).filter(|k| k.starts_with('.')).map(|k| k[1..].chars().take_while(char::is_ascii_digit).take(3).collect()).unwrap_or_default();
    let ms = if kesir.is_empty() { 0 } else { format!("{kesir:0<3}").parse::<i64>().unwrap_or(0) };
    Some(sn * 1000 + ms)
}

/// HAR içe aktarmanın sonucu
pub struct Aktarim {
    pub kaynak: Kaynak,
    pub kayitlar: Vec<Kayit>,
    /// Haritada bulunamayan gözlenen uçlar (yöntem + şablon)
    pub bilinmeyen: BTreeSet<String>,
    pub atlanan: u64,
}

/// Zaman işareti: {ad, zaman}
#[derive(Deserialize)]
pub struct Isaret {
    pub ad: String,
    pub zaman: String,
}

/// HAR'ı okur, harita uçlarıyla eşler ve biçimlere indirger. `okunan`: haritanın okunan dosya özetleri (dayanak)
pub fn har_oku(yol: &Path, harita: &Value, isaretler: &[Isaret]) -> Result<Aktarim, String> {
    let m = std::fs::symlink_metadata(yol).map_err(|e| format!("{}: {e}", crate::guvenli::gorunen(yol)))?;
    if !m.file_type().is_file() {
        return Err(tf!("{}: düzenli dosya değil (sembolik bağ izlenmez)", "{}: not a regular file (symbolic links are not followed)", crate::guvenli::gorunen(yol)));
    }
    // Kaynağın kimliği HAR'ın SHA-256'sı: aynı dosya yeniden alınırsa öncekinin yerine geçer
    let sha = {
        use std::io::Read;
        let mut f = std::fs::File::open(yol).map_err(|e| e.to_string())?;
        let mut c = ring::digest::Context::new(&ring::digest::SHA256);
        let mut t = vec![0u8; 1 << 16];
        loop {
            let n = f.read(&mut t).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            c.update(&t[..n]);
        }
        c.finish().as_ref().iter().map(|x| format!("{x:02x}")).collect::<String>()
    };
    let kimlik = format!("har:{sha}");
    let uc = uclar(harita);
    let okunan = crate::kanit::okunan_ozetleri(&harita["ozet"]).unwrap_or_default();
    // (zaman ms, biçim) sırasıyla
    let mut gorulen: Vec<(i64, Bicim, String)> = vec![];
    let mut ana_makineler: BTreeMap<String, bool> = BTreeMap::new();
    let mut atlanan = 0u64;
    let mut sayi = 0usize;
    let mut isle = |g: HarGirdi| -> Result<(), String> {
        sayi += 1;
        if sayi > EN_COK_ISTEK {
            return Err(tf!("HAR'da {EN_COK_ISTEK}'den fazla istek var", "the HAR has more than {EN_COK_ISTEK} requests"));
        }
        let yontem = g.request.method.to_ascii_uppercase();
        if yontem.is_empty() || yontem == "OPTIONS" || g.response.status <= 0 {
            atlanan += 1;
            return Ok(());
        }
        let (ana, yol, sorgu) = adres_parcala(&g.request.url);
        let u = eslestir(&uc, &yontem, &yol);
        if u.is_none() && (statik_mi(&yol) || (matches!(yontem.as_str(), "GET" | "HEAD") && !g.response.content.mime.to_ascii_lowercase().contains("json"))) {
            atlanan += 1;
            return Ok(());
        }
        let post = g.request.post.unwrap_or_default();
        let parametreler: Vec<String> = post.params.into_iter().map(|p| p.name).collect();
        let yanit_metni: Option<Vec<u8>> = g.response.content.text.map(|t| {
            if g.response.content.encoding.as_deref() == Some("base64") {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.decode(t.trim()).unwrap_or_default()
            } else {
                t.into_bytes()
            }
        });
        let h = HamIstek {
            yontem: &yontem,
            yol: &yol,
            sorgu: &sorgu,
            durum: g.response.status.clamp(0, 999) as u16,
            istek_mime: &post.mime,
            istek_metni: post.text.as_deref().map(str::as_bytes),
            istek_parametreleri: &parametreler,
            yanit_mime: &g.response.content.mime,
            yanit_metni: yanit_metni.as_deref(),
        };
        let b = bicimle(&h, u.map(|u| (u.api.as_str(), u.yol.as_str())));
        *ana_makineler.entry(ana.clone()).or_default() |= u.is_some();
        gorulen.push((zaman_ms(&g.baslangic).unwrap_or(0), b, ana));
        Ok(())
    };
    let f = std::fs::File::open(yol).map_err(|e| e.to_string())?;
    let mut de = serde_json::Deserializer::from_reader(std::io::BufReader::new(f));
    HarKok(&mut isle).deserialize(&mut de).map_err(|e| tf!("HAR okunamadı: {e}", "could not read the HAR: {e}"))?;
    de.end().map_err(|e| tf!("HAR okunamadı: {e}", "could not read the HAR: {e}"))?;
    // Uygulamanın kökeni: en az bir eşleşen isteğin geldiği ana makineler. Başka kökenlerin (analitik, CDN) eşleşmeyen
    // istekleri uç sayılmaz
    gorulen.retain(|(_, b, ana)| b.api.is_some() || ana_makineler.get(ana).copied().unwrap_or(false));
    gorulen.sort_by_key(|(z, _, _)| *z);
    let mut sayac: BTreeMap<Bicim, u64> = BTreeMap::new();
    let mut sira: Vec<String> = vec![];
    let mut bilinmeyen = BTreeSet::new();
    let mut eslesen = 0u64;
    for (_, b, _) in &gorulen {
        *sayac.entry(b.clone()).or_default() += 1;
        match &b.api {
            Some(a) => {
                eslesen += 1;
                if b.durum < 400 && sira.last() != Some(a) && sira.len() < EN_UZUN_SIRA {
                    sira.push(a.clone());
                }
            }
            None => {
                bilinmeyen.insert(format!("{} {}", b.yontem, b.yol));
            }
        }
    }
    // Zaman işaretleri: işaretten sonraki (bir sonraki işarete ve pencereye kadar) ilk eşleşen istek
    let mut isaretler: Vec<(i64, String)> = isaretler.iter().filter_map(|i| Some((zaman_ms(&i.zaman)?, crate::guvenli::temiz(&i.ad).chars().take(64).collect()))).collect();
    isaretler.sort();
    let mut zaman_eslesmeleri = vec![];
    for (j, (z, ad)) in isaretler.iter().enumerate() {
        let son = isaretler.get(j + 1).map(|(s, _)| *s).unwrap_or(i64::MAX).min(z + PENCERE_MS);
        if let Some((zi, b, _)) = gorulen.iter().find(|(zi, b, _)| *zi >= *z && *zi < son && b.api.is_some()) {
            zaman_eslesmeleri.push(json!({ "isaret": ad, "api": b.api, "fark_ms": zi - z }));
        }
    }
    let dosyalar: HashMap<&str, &str> = uc.iter().map(|u| (u.api.as_str(), u.dosya.as_str())).collect();
    let kayitlar = sayac
        .into_iter()
        .map(|(b, adet)| {
            let dayanak =
                b.api.as_deref().and_then(|a| dosyalar.get(a)).and_then(|d| okunan.get(*d).map(|s| Dayanak { yol: d.to_string(), sha256: s.clone() })).into_iter().collect();
            Kayit { kaynak: kimlik.clone(), bicim: b, adet, dayanak }
        })
        .collect();
    let kaynak = Kaynak { kimlik, tur: "har".into(), zaman: crate::simdi(), istek: gorulen.len() as u64, eslesen, sira, zaman_eslesmeleri, ..Kaynak::default() };
    Ok(Aktarim { kaynak, kayitlar, bilinmeyen, atlanan })
}

// ---- Canlı mod ----

/// `cagir` ile gerçekleşen bir istek: aynı projeksiyonla gözlem olarak eklenir ve kanıt deposu yenilenir. İnsan ve
/// token bilgisi gözleme girmez; değer yok. Hata kanıt deposunu bozmaz (yalnızca döner)
pub fn cagri_kaydet(klasor: &Path, kapi: &Value, istek_govdesi: Option<&[u8]>, durum: u16, yanit: &[u8]) -> Result<(), String> {
    let harita: Value =
        crate::guvenli::oku_harita(&klasor.join("kapilar.json")).and_then(|m| serde_json::from_str(&m).ok()).ok_or(t!("harita okunamadı", "could not read the map"))?;
    let api = crate::graf::api_kimligi(kapi);
    let yol = kapi["yol"].as_str().unwrap_or("");
    let h = HamIstek {
        yontem: kapi["yontem"].as_str().unwrap_or("GET"),
        yol,
        sorgu: "",
        durum,
        istek_mime: if istek_govdesi.is_some() { "application/json" } else { "" },
        istek_metni: istek_govdesi,
        istek_parametreleri: &[],
        yanit_mime: "application/json",
        yanit_metni: Some(yanit),
    };
    let b = bicimle(&h, Some((&api, yol)));
    let okunan = crate::kanit::okunan_ozetleri(&harita["ozet"]).unwrap_or_default();
    let dosya = kapi["dosya"].as_str().unwrap_or("");
    let dayanak: Vec<Dayanak> = okunan.get(dosya).map(|s| Dayanak { yol: dosya.to_string(), sha256: s.clone() }).into_iter().collect();
    let mut g = Gozlemler::oku(klasor).unwrap_or(Gozlemler { bicim: BICIM, ..Gozlemler::default() });
    let kimlik = "canli".to_string();
    match g.kayitlar.iter_mut().find(|k| k.kaynak == kimlik && k.bicim == b) {
        Some(k) => {
            k.adet += 1;
            k.dayanak = dayanak;
        }
        None => g.kayitlar.push(Kayit { kaynak: kimlik.clone(), bicim: b, adet: 1, dayanak }),
    }
    if !g.kaynaklar.iter().any(|k| k.kimlik == kimlik) {
        g.kaynaklar.push(Kaynak { kimlik: kimlik.clone(), tur: "canli".into(), ..Kaynak::default() });
    }
    if let Some(k) = g.kaynaklar.iter_mut().find(|k| k.kimlik == kimlik) {
        k.zaman = crate::simdi();
        k.istek += 1;
        k.eslesen += 1;
        if durum < 400 && k.sira.last() != Some(&api) {
            k.sira.push(api);
            if k.sira.len() > EN_UZUN_SIRA {
                k.sira.remove(0);
            }
        }
    }
    g.yaz(klasor)?;
    depo_yenile(klasor)
}

// ---- Kanıt deposu ----

fn kaynaksiz_hatasi() -> &'static str {
    t!(
        "kaynaksız haritaya (sezgi kesfet) gözlem içe aktarma bu sürümde desteklenmiyor: kaynak koddan kurulan haritayla kullanın",
        "importing observations into a map built without source code (sezgi discover) is not supported in this version: use a map built from source code"
    )
}

/// Kanıt deposunu haritadan, ön yüz bağlarından ve gözlemlerden yeniden kurar ve yazar (`init` sonrası değişen tek
/// şey gözlemlerse)
pub fn depo_yenile(klasor: &Path) -> Result<(), String> {
    let mut harita: Value =
        crate::guvenli::oku_harita(&klasor.join("kapilar.json")).and_then(|m| serde_json::from_str(&m).ok()).ok_or(t!("harita okunamadı", "could not read the map"))?;
    crate::guvenli::json_temiz(&mut harita);
    // Kaynaksız haritanın kanıt deposu yapıttan kurulur (`sezgi kesfet`): kaynak haritası gibi yeniden kurulamaz
    if harita["ozet"]["kaynaksiz"] == true {
        return Err(kaynaksiz_hatasi().into());
    }
    let web: Value = crate::guvenli::oku_harita(&klasor.join(crate::kanit::WEB_DOSYA)).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null);
    let onceki: Option<crate::kanit::Depo> = crate::guvenli::oku_harita(&klasor.join(crate::kanit::DOSYA)).and_then(|m| serde_json::from_str(&m).ok());
    let kok_goreli = onceki.as_ref().and_then(|d| d.kok.clone());
    let kok = kok_goreli.as_ref().map(|r| klasor.join(r));
    let onek = harita["ozet"]["on_yuz"].as_str().filter(|o| *o != ".").unwrap_or("").to_string();
    let g = Gozlemler::oku(klasor);
    // Adaptörün yan bilgisi (modül, router ve tablo grafiği) önceki depodan: harita onu taşımaz
    let ek = onceki.as_ref().map(|d| d.ek.clone()).unwrap_or_default();
    let mut depo = crate::kanit::uret(&harita, &web, &onek, &ek, kok.as_deref(), &crate::simdi(), g.as_ref());
    depo.kok = kok_goreli;
    crate::kanit::yaz(klasor, &mut depo)
}

/// İki tipin uyuşması: koddaki tip ile gözlenen JSON tipi. Bilinmeyen kod tipi (DTO adı, tarih ...) ve null uyumlu sayılır
fn tip_uyumlu(kod: &str, gozlenen: &str) -> bool {
    match (kod, gozlenen) {
        (_, "null" | "form") => true,
        ("string", g) => g == "string",
        ("number" | "integer", g) => g == "number",
        ("boolean", g) => g == "boolean",
        ("array", g) => g == "array",
        _ => true,
    }
}

/// Gözlemleri kanıt deposuna ekler. `kapilar`: api kimliği -> kapı; `govde_degeri`: kodun girdi şeması önermesinin
/// değeri (karşılaştırma için aynı biçimde); `ui_hedefleri`: UI eylemi -> statik olarak ulaştığı uçlar (zaman işareti
/// eşlemesi için)
pub(crate) fn kanitlara(
    g: &Gozlemler,
    k: &mut Kurucu,
    kapilar: &HashMap<String, &Value>,
    govde_degeri: &dyn Fn(&Value) -> Value,
    ui_hedefleri: &BTreeMap<String, BTreeSet<String>>,
) {
    let uretici = format!("sezgi-gozlem {}", env!("CARGO_PKG_VERSION"));
    let kaynak_adi = |kaynak: &str| {
        if kaynak == "canli" {
            "canlı mod".to_string()
        } else if let Some(s) = kaynak.strip_prefix("senaryo:") {
            format!("senaryo {}", &s[..12.min(s.len())])
        } else {
            format!("HAR {}", &kaynak[4.min(kaynak.len())..16.min(kaynak.len())])
        }
    };
    for r in &g.kayitlar {
        let b = &r.bicim;
        let api = b.api.clone().unwrap_or_else(|| format!("api:{} {}", b.yontem, b.yol));
        let kapi = b.api.as_deref().and_then(|a| kapilar.get(a)).copied();
        let mut sinir = vec![format!("yalnızca gözlenen {} istek için ({}); başka girdi ve durumlar gözlenmedi", r.adet, kaynak_adi(&r.kaynak))];
        if r.kaynak.starts_with("senaryo:") {
            sinir.push("tarayıcı senaryosu (REA); gövde ve yanıt değerleri alınmadı".into());
        } else if r.kaynak != "canli" {
            sinir.push("UI → istek bağı yok: HAR adım bilgisi taşımaz".into());
        }
        if r.dayanak.is_empty() {
            sinir.push("dayanak dosyası yok: geçerliliği kodla izlenemez".into());
        }
        let ozet = json!({
            "tur": "ag_gozlemi", "kaynak": if r.kaynak == "canli" { "canli" } else { "har" }, "yontem": b.yontem, "yol": b.yol, "durum": b.durum, "adet": r.adet,
            "istek_alanlari": b.istek_alanlari.keys().collect::<Vec<_>>(), "yanit_alanlari": b.yanit_alanlari.keys().collect::<Vec<_>>(),
        });
        let konum = vec![if r.kaynak == "canli" {
            "canli".to_string()
        } else if r.kaynak.starts_with("senaryo:") {
            r.kaynak[..20.min(r.kaynak.len())].to_string()
        } else {
            format!("har:{}", &r.kaynak[4.min(r.kaynak.len())..16.min(r.kaynak.len())])
        }];
        let mut iddialar = vec![];
        if b.durum < 400 {
            iddialar.push(Iddia::yeni(&api, "CALISIR", None, json!(true), Kapsam::KapaliDunya));
        } else {
            iddialar.push(Iddia::yeni(&api, "HATA_DURUMU", Some(&b.durum.to_string()), json!(true), Kapsam::KapaliDunya));
        }
        // Başarılı yanıt koddaki yanıt şemasıyla: uyumluysa aynı değer (destek) ve gözlenen yanıt biçimi kapalı kapsam;
        // değilse çelişki
        let yanit_semasi = kapi.map(|kp| &kp["yanit"]["sema"]).filter(|s| s["alanlar"].is_object() && s["bulunamadi"] != true);
        let mut yanit_uyumlu = false;
        if let Some(ys) = yanit_semasi.filter(|_| (200..300).contains(&b.durum) && !b.yanit_alanlari.is_empty()) {
            let alanlar = ys["alanlar"].as_object().cloned().unwrap_or_default();
            let mut uyumsuz = vec![];
            if b.yanit_dizi != (ys["dizi"] == true) {
                uyumsuz.push(if b.yanit_dizi { "yanıt dizi, kodda tek nesne".to_string() } else { "yanıt tek nesne, kodda dizi".to_string() });
            }
            for a in b.yanit_alanlari.keys().filter(|a| !alanlar.contains_key(*a)) {
                uyumsuz.push(format!("yanıtta kodda olmayan alan: {a}"));
            }
            for (ad, a) in &alanlar {
                if a["zorunlu"] == true && a["belirsiz"] != true && !b.yanit_alanlari.contains_key(ad) {
                    uyumsuz.push(format!("yanıtta zorunlu `{ad}` yok"));
                }
                if let Some(t) = b.yanit_alanlari.get(ad) {
                    if !tip_uyumlu(a["tip"].as_str().unwrap_or(""), t) {
                        uyumsuz.push(format!("yanıt `{ad}`: kodda {}, gözlenen {t}", a["tip"].as_str().unwrap_or("?")));
                    }
                }
            }
            if !uyumsuz.is_empty() {
                sinir.insert(0, format!("yanıt koddaki şemayla uyuşmuyor: {}", uyumsuz.join("; ")));
            }
            yanit_uyumlu = uyumsuz.is_empty();
            let deger = if yanit_uyumlu { govde_degeri(ys) } else { json!({ "gozlem_uyumsuz": uyumsuz }) };
            iddialar.push(Iddia::yeni(&api, "YANIT_SEMASI", Some(ys["dto"].as_str().unwrap_or("?")), deger, Kapsam::Acik));
        }
        if !b.yanit_alanlari.is_empty() {
            let alanlar: Vec<&String> = b.yanit_alanlari.keys().collect();
            // Şemalı uçta ve şemayla uyumluysa gözlenen biçim kapalı kapsamdır: alan listesi kodun şemasıyla doğrulandı
            let (kapsam, not) = if yanit_uyumlu { (Kapsam::KapaliDunya, vec![]) } else { (Kapsam::Acik, vec!["yanıt biçimi yalnızca gözlenen yanıtlardan".to_string()]) };
            iddialar.push(Iddia::yeni(&api, "YANIT", Some(&b.durum.to_string()), json!({ "alanlar": alanlar, "dizi": b.yanit_dizi }), kapsam).notlu(not));
        }
        // Başarılı isteğin gövdesi koddaki girdi şemasıyla: uyumluysa aynı değer (destek), değilse çelişki
        if let Some(kp) = kapi.filter(|_| b.durum < 300 && !b.istek_alanlari.is_empty()) {
            if let Some(sema) = kp["govde"].as_array().into_iter().flatten().map(|g| &g["sema"]).find(|s| s["alanlar"].is_object() && s["bulunamadi"] != true) {
                let dto = sema["dto"].as_str().unwrap_or("?");
                let alanlar = sema["alanlar"].as_object().cloned().unwrap_or_default();
                let mut uyumsuz = vec![];
                for (ad, a) in &alanlar {
                    if a["zorunlu"] == true && !b.istek_alanlari.contains_key(ad) {
                        uyumsuz.push(format!("zorunlu `{ad}` gönderilmeden kabul edildi (HTTP {})", b.durum));
                    }
                    if let Some(t) = b.istek_alanlari.get(ad) {
                        if !tip_uyumlu(a["tip"].as_str().unwrap_or(""), t) {
                            uyumsuz.push(format!("`{ad}`: kodda {}, gözlenen {t}", a["tip"].as_str().unwrap_or("?")));
                        }
                    }
                }
                let fazla: Vec<String> = b.istek_alanlari.keys().filter(|a| !alanlar.contains_key(*a)).map(|a| format!("gözlenen ama kodda olmayan alan: {a}")).collect();
                // Uyuşmazlığın nedeni kanıtın ilk sınırlamasıdır: çelişki gösterilirken nedeni de görünür
                if !uyumsuz.is_empty() {
                    sinir.insert(0, format!("gövde koddaki şemayla uyuşmuyor: {}", uyumsuz.join("; ")));
                }
                let deger = if uyumsuz.is_empty() { govde_degeri(sema) } else { json!({ "gozlem_uyumsuz": uyumsuz }) };
                iddialar.push(Iddia::yeni(&api, "GOVDE", Some(dto), deger, Kapsam::Acik).notlu(fazla));
            }
        }
        k.ekle(Kanit::yeni(KaynakTuru::NetworkCapture, &uretici, r.dayanak.clone(), konum, Some(Guven::Observed)).ozetli(ozet, sinir), iddialar);
        if kapi.is_none() {
            k.bos(
                Iddia::yeni(&api, "VAR", None, Value::Null, Kapsam::Bilinmiyor),
                "kodda bulunamayan gözlenen uç: haritada yok (koşullu rota, başka uygulama ya da desteklenmeyen tanım)",
            );
        }
    }
    // Tarayıcı senaryosu: REA'nın isteği başlatan adım olarak kaydettiği UI adımı → uç (gözlem). Statik UI eylemi
    // aynı etiketi taşıyor ve aynı uca ulaşıyorsa onun TRIGGERS kenarı desteklenir; değilse senaryo adımı ayrı bir düğümdür
    for kaynak in g.kaynaklar.iter().filter(|k| k.tur == "senaryo") {
        let guven = if kaynak.yalitim.as_deref() == Some("yok") { Guven::Inferred } else { Guven::Observed };
        for b in &kaynak.adim_baglari {
            let (Some(api), Some(etiket)) = (b["api"].as_str(), b["etiket"].as_str()) else { continue };
            let dayanak = g.kayitlar.iter().find(|r| r.kaynak == kaynak.kimlik && r.bicim.api.as_deref() == Some(api)).map(|r| r.dayanak.clone()).unwrap_or_default();
            let statik: Vec<&String> = ui_hedefleri
                .iter()
                .filter(|(ui, h)| h.contains(api) && !sade(&eylem_etiketi(ui)).is_empty() && sade(&eylem_etiketi(ui)) == sade(etiket))
                .map(|(ui, _)| ui)
                .collect();
            let adim = b["adim"].as_str().unwrap_or("?");
            let mut sin = vec!["REA tarayıcı senaryosu: isteği başlatan adım (adım ↔ istek kimliği bağı); doldurulan ve yanıt değerleri alınmadı".to_string()];
            if guven == Guven::Inferred {
                sin.push("yalıtım beyanı: yok (gözlem en fazla inferred)".into());
            }
            let kanit = Kanit::yeni(KaynakTuru::BrowserObservation, &uretici, dayanak, vec![format!("{}#{adim}", &kaynak.kimlik[..20.min(kaynak.kimlik.len())])], Some(guven))
                .ozetli(json!({ "tur": "senaryo_adimi", "adim": adim, "eylem": b["eylem"], "rol": b["rol"] }), sin);
            let iddialar: Vec<Iddia> = if statik.is_empty() {
                let ui = format!("ui:senaryo#{}:{}", b["rol"].as_str().unwrap_or("?"), etiket);
                vec![Iddia::yeni(&ui, "TRIGGERS", Some(api), json!({}), Kapsam::Acik)]
            } else {
                statik.iter().map(|ui| Iddia::yeni(ui, "TRIGGERS", Some(api), json!({}), Kapsam::KapaliDunya)).collect()
            };
            k.ekle(kanit, iddialar);
        }
    }
    // Zaman işareti → UI eylemi: işaretin adı eylemin adıyla uyuşan ve statik olarak aynı uca ulaşan eylem; inferred
    for kaynak in &g.kaynaklar {
        for z in &kaynak.zaman_eslesmeleri {
            let (Some(isaret), Some(api)) = (z["isaret"].as_str(), z["api"].as_str()) else { continue };
            for (ui, hedefler) in ui_hedefleri {
                let etiket = ui.rsplit_once('#').map(|(_, e)| e.split_once(':').map(|(_, a)| a).unwrap_or(e)).unwrap_or("");
                if sade(etiket).is_empty() || sade(etiket) != sade(isaret) || !hedefler.contains(api) {
                    continue;
                }
                let dayanak = kapilar
                    .get(api)
                    .and_then(|kp| g.kayitlar.iter().find(|r| r.bicim.api.as_deref() == Some(api)).map(|r| (kp, r.dayanak.clone())))
                    .map(|(_, d)| d)
                    .unwrap_or_default();
                let kanit = Kanit::yeni(KaynakTuru::NetworkCapture, &uretici, dayanak, vec![format!("{}:isaret:{isaret}", kaynak.tur)], Some(Guven::Inferred)).ozetli(
                    json!({ "tur": "zaman_eslemesi", "isaret": isaret, "fark_ms": z["fark_ms"] }),
                    vec![format!("zaman penceresi eşlemesi ({} ms): UI → istek bağı tahmin", z["fark_ms"])],
                );
                k.ekle(kanit, vec![Iddia::yeni(ui, "TRIGGERS", Some(api), json!({}), Kapsam::Acik)]);
            }
        }
    }
}

/// Karşılaştırma için sade biçim: yalnızca harf ve rakam, küçük harf
pub(crate) fn sade(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// UI eylemi kimliğinin etiketi: `ui:src/A.tsx#button:Save` -> "Save"
pub(crate) fn eylem_etiketi(ui: &str) -> String {
    ui.rsplit_once('#').map(|(_, e)| e.split_once(':').map(|(_, a)| a).unwrap_or(e)).unwrap_or("").to_string()
}

// ---- Komut ----

/// `sezgi gozlem al <dosya.har> [--harita <klasör>] [--isaretler <json>]`
pub fn komut(args: &[String]) {
    // `sezgi gozlem senaryo <senaryo.json> ...`: REA tarayıcı senaryosu
    if args.get(1).map(String::as_str) == Some("senaryo") {
        return crate::senaryo::komut(args);
    }
    let dur = |m: &str| -> ! {
        eprintln!("✗ {m}");
        std::process::exit(1)
    };
    // Konumsal argümanlar: değer alan bayrakların değerleri hariç
    let mut konumlar: Vec<&String> = vec![];
    let mut i = 1;
    while i < args.len() {
        if matches!(args[i].as_str(), "--harita" | "--isaretler") {
            i += 2;
            continue;
        }
        if !args[i].starts_with("--") {
            konumlar.push(&args[i]);
        }
        i += 1;
    }
    let (Some("al"), Some(har)) = (konumlar.first().map(|s| s.as_str()), konumlar.get(1)) else {
        te!("Kullanım: sezgi gozlem al <dosya.har> [--harita <klasör>] [--isaretler <json>]", "Usage: sezgi observe import <file.har> [--map <folder>] [--isaretler <json>]");
        std::process::exit(2)
    };
    let secenek = |ad: &str| args.iter().position(|a| a == ad).and_then(|i| args.get(i + 1)).cloned();
    let klasor = crate::guvenli::harita_gerekli(secenek("--harita").as_deref());
    let harita: Value = crate::guvenli::oku_harita(&klasor.join("kapilar.json"))
        .and_then(|m| serde_json::from_str(&m).ok())
        .unwrap_or_else(|| dur(t!("harita okunamadı (önce: sezgi init)", "could not read the map (first: sezgi init)")));
    let isaretler: Vec<Isaret> = match secenek("--isaretler") {
        Some(p) => crate::guvenli::oku(Path::new(&p))
            .and_then(|m| serde_json::from_str(&m).ok())
            .unwrap_or_else(|| dur(&tf!("işaret dosyası okunamadı: {p} ([{{\"ad\", \"zaman\"}}])", "could not read the marker file: {p} ([{{\"ad\", \"zaman\"}}])"))),
        None => vec![],
    };
    if harita["ozet"]["kaynaksiz"] == true {
        dur(kaynaksiz_hatasi());
    }
    let a = har_oku(Path::new(har.as_str()), &harita, &isaretler).unwrap_or_else(|e| dur(&e));
    let mut g = Gozlemler::oku(&klasor).unwrap_or(Gozlemler { bicim: BICIM, ..Gozlemler::default() });
    let (istek, eslesen, zaman) = (a.kaynak.istek, a.kaynak.eslesen, a.kaynak.zaman_eslesmeleri.len());
    let uc_sayisi = a.kayitlar.iter().filter_map(|k| k.bicim.api.clone()).collect::<BTreeSet<_>>().len();
    g.kaynak_koy(a.kaynak, a.kayitlar);
    g.yaz(&klasor).unwrap_or_else(|e| dur(&e));
    depo_yenile(&klasor).unwrap_or_else(|e| dur(&e));
    tp!(
        "✓ Gözlem alındı: {istek} istek · {eslesen} tanesi haritadaki {uc_sayisi} uca eşleşti · {} atlandı (statik dosya, başka köken)",
        "✓ Observation imported: {istek} {i} · {eslesen} matched {uc_sayisi} {e} in the map · {} skipped (static file, other origin)",
        a.atlanan;
        i = crate::dil::cogul(istek, "request", "requests"),
        e = crate::dil::cogul(uc_sayisi, "endpoint", "endpoints")
    );
    tp!(
        "  Saklanan     yalnızca biçim: yöntem, yol şablonu, durum kodu, alan adları ve tipleri (değer, başlık, çerez, belirteç yok)",
        "  Stored       shape only: method, path template, status code, field names and types (no values, headers, cookies or tokens)"
    );
    tp!("  Kanıt        NETWORK_CAPTURE (observed) · kanit.json yenilendi", "  Evidence     NETWORK_CAPTURE (observed) · kanit.json refreshed");
    if !a.bilinmeyen.is_empty() {
        tp!("  Bilinmeyen   kodda bulunamayan {} gözlenen uç:", "  Unknown      {} observed endpoints not found in the code:", a.bilinmeyen.len());
        for b in a.bilinmeyen.iter().take(10) {
            println!("               {b}");
        }
    }
    if zaman > 0 {
        tp!(
            "  İşaret       {zaman} işaret bir isteğe eşlendi (zaman penceresi: UI bağı yalnızca tahmin)",
            "  Markers      {zaman} {m} a request (time window: the UI link is only a guess)";
            m = crate::dil::cogul(zaman, "marker matched", "markers matched")
        );
    }
    tp!("  Kart         gözlemler bir sonraki `sezgi init`te Benlik Kartı'na da girer", "  Card         observations also go into the Benlik Card at the next `sezgi init`");
}
