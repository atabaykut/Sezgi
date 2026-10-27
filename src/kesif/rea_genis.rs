//! REA köprüsünün geniş araçları: WebMCP keşfi, native ikili analizi (kullanıcının kurduğu motorla), firmware bölüm
//! envanteri ve süreç gözlemi. Hepsi aynı köprüden (sabitleme, etki tablosu, kapı) geçer; burada yalnızca çıktıların
//! **biçim projeksiyonu** yapılır: araç, fonksiyon, bölüm ve dosya adları, şema iskeleti, sayılar. Değer alınmaz:
//! sayfanın araç açıklaması, dize içerikleri, terminal çıktısı, decompile edilmiş kod saklanmaz (kodun yalnızca satır
//! sayısı ve özeti).
//! - Sayfanın ilan ettiği WebMCP araçları güvenilmez veridir: adlar temizlenir ve kısaltılır, hiçbiri çağrılmaz.
//! - Ticari motorların (IDA, Hopper, JEB) çıktısı paylaşılamaz: kanıt `paylasilabilir: false` taşır, karta ve kanıt
//!   paketine girmez.
use serde::de::IgnoredAny;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// Bir listede projekte edilen en çok öğe
const EN_FAZLA: usize = 2000;

/// REA zarfının ortak alanları
#[derive(Deserialize)]
pub(crate) struct Zarf<T> {
    pub evidence_id: Option<String>,
    pub operation: Option<String>,
    pub confidence: Option<String>,
    pub authority: Option<String>,
    pub provider: Option<Saglayici>,
    pub subject: Option<Konu>,
    pub environment: Option<Ortam>,
    pub normalized_result: Option<T>,
}

#[derive(Deserialize)]
pub(crate) struct Saglayici {
    pub id: Option<String>,
    pub version: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Konu {
    pub digest: Option<Ozet>,
}

#[derive(Deserialize)]
pub(crate) struct Ozet {
    pub sha256: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Ortam {
    pub isolation: Option<String>,
}

impl<T> Zarf<T> {
    pub fn saglayici(&self) -> String {
        let p = self.provider.as_ref();
        format!("rea@{}/{}@{}", super::rea::SURUM, p.and_then(|p| p.id.clone()).unwrap_or_else(|| "?".into()), p.and_then(|p| p.version.clone()).unwrap_or_else(|| "?".into()))
    }
    /// REA güveni (eksik ya da tanınmayan: en zayıf)
    pub fn guven(&self) -> &'static str {
        match self.confidence.as_deref() {
            Some("certain") => "certain",
            Some("observed") => "observed",
            _ => "inferred",
        }
    }
    pub fn sha256(&self) -> Option<String> {
        self.subject.as_ref().and_then(|s| s.digest.as_ref()).and_then(|d| d.sha256.clone())
    }
    /// REA'nın kendi yalıtım bildirimi (ör. tarayıcı gözleminde "none")
    pub fn rea_yalitim(&self) -> Option<String> {
        self.environment.as_ref().and_then(|e| e.isolation.clone())
    }
    pub fn paylasilabilir(&self) -> bool {
        !super::rea::ticari_mi(self.provider.as_ref().and_then(|p| p.id.as_deref()).unwrap_or(""))
    }
}

/// Geçici çıktı dosyasını akışla okur ve siler
pub(crate) fn oku<T: for<'de> Deserialize<'de>>(yol: &Path, islem: &str) -> Result<Zarf<T>, String> {
    let z: Result<Zarf<T>, String> = std::fs::File::open(yol)
        .map_err(|e| e.to_string())
        .and_then(|f| serde_json::from_reader(std::io::BufReader::new(f)).map_err(|e| format!("REA çıktısı beklenen biçimde değil: {e}")));
    let _ = std::fs::remove_file(yol);
    let z = z?;
    if z.operation.as_deref() != Some(islem) {
        return Err(format!("REA çıktısının işlemi '{}' (beklenen {islem})", z.operation.as_deref().unwrap_or("?")));
    }
    Ok(z)
}

/// Sayfadan ya da ikiliden gelen ad: görünmez karakterler temizlenir, kısaltılır
fn ad(s: &str) -> String {
    let t = crate::guvenli::temiz(s);
    if t.chars().count() > 120 {
        t.chars().take(120).collect::<String>() + "…"
    } else {
        t
    }
}

// ---- WebMCP ----

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Hedefler {
    targets: Vec<Hedef>,
}

#[derive(Deserialize)]
struct Hedef {
    target_id: Option<String>,
    origin: Option<String>,
    #[serde(rename = "type")]
    tur: Option<String>,
}

impl Hedefler {
    /// İstenen kökenden (verilmediyse ilk) sayfa hedefinin kimliği
    pub fn sec(&self, koken: Option<&str>) -> Option<String> {
        self.targets.iter().filter(|t| t.tur.as_deref() == Some("page")).find(|t| koken.is_none_or(|k| t.origin.as_deref() == Some(k))).and_then(|t| t.target_id.clone())
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct WebMcp {
    status: Option<String>,
    target: Option<WebHedef>,
    tools: WebAraclar,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct WebHedef {
    origin: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct WebAraclar {
    total: Option<u64>,
    items: Vec<WebArac>,
}

#[derive(Deserialize)]
struct WebArac {
    name: Option<String>,
    declaration_kind: Option<String>,
    owner_origin: Option<String>,
    input_schema: Option<Value>,
    input_schema_sha256: Option<String>,
    annotations: Option<Value>,
}

/// Girdi şemasının iskeleti: alan adı, tür, zorunluluk (açıklama, örnek, izinli değerler alınmaz)
fn sema_iskeleti(s: &Value) -> Value {
    let zorunlu: Vec<&str> = s["required"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let alanlar: Vec<Value> = s["properties"]
        .as_object()
        .into_iter()
        .flatten()
        .take(100)
        .map(|(k, v)| json!({ "ad": ad(k), "tur": v["type"].as_str().map(ad).unwrap_or_else(|| "?".into()), "zorunlu": zorunlu.contains(&k.as_str()) }))
        .collect();
    json!(alanlar)
}

/// WebMCP keşfinin projeksiyonu
pub(crate) fn webmcp_ozeti(w: &WebMcp) -> Value {
    let araclar: Vec<Value> = w
        .tools
        .items
        .iter()
        .take(200)
        .map(|t| {
            let a = t.annotations.clone().unwrap_or(Value::Null);
            json!({
                "ad": ad(t.name.as_deref().unwrap_or("?")),
                "tur": t.declaration_kind.as_deref().map(ad),
                "koken": t.owner_origin.as_deref().map(ad),
                "girdi": t.input_schema.as_ref().map(sema_iskeleti).unwrap_or_else(|| json!([])),
                "girdi_sha256": t.input_schema_sha256,
                "salt_okunur": a["read_only"].as_bool(),
                "sonuclu": a["consequential"].as_bool(),
            })
        })
        .collect();
    json!({
        "durum": w.status,
        "koken": w.target.as_ref().and_then(|t| t.origin.as_deref()).map(ad),
        "toplam": w.tools.total,
        "araclar": araclar,
    })
}

// ---- Süreç gözlemi ----

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Surec {
    exit: Option<Value>,
    settlement: Option<Value>,
    /// Terminal çıktısı: yalnızca sayılır, içerik okunmaz
    frames: Vec<IgnoredAny>,
    process_samples: Vec<IgnoredAny>,
    filesystem_effects: Vec<DosyaEtkisi>,
    truncated: Option<bool>,
}

#[derive(Deserialize)]
struct DosyaEtkisi {
    path: Option<String>,
    status: Option<String>,
}

pub(crate) fn surec_ozeti(s: &Surec) -> Value {
    let etkiler: Vec<Value> = s
        .filesystem_effects
        .iter()
        .filter(|e| e.status.as_deref() != Some("unchanged"))
        .take(EN_FAZLA)
        .map(|e| json!({ "yol": e.path.as_deref().map(ad), "durum": e.status }))
        .collect();
    json!({
        "cikis": { "kod": s.exit.as_ref().and_then(|e| e["code"].as_i64()), "neden": s.exit.as_ref().and_then(|e| e["reason"].as_str()).map(ad) },
        "yerlesme": s.settlement.as_ref().and_then(|e| e["state"].as_str()).map(ad),
        "terminal_cerceve": s.frames.len(),
        "surec_ornegi": s.process_samples.len(),
        "dosya_etkileri": etkiler,
        "kesildi": s.truncated,
    })
}

// ---- Firmware ve native (çıktı biçimi motora göre değişir: izinli anahtarlarla projeksiyon) ----

/// Listelerde tutulan anahtarlar (yapı); diğerleri (içerik, dize değeri, kod) atılır
const IZINLI_ANAHTARLAR: &[&str] = &[
    "name",
    "address",
    "offset",
    "start",
    "end",
    "size",
    "length",
    "kind",
    "type",
    "format",
    "section",
    "segment",
    "library",
    "signature",
    "entry",
    "caller",
    "callee",
    "from",
    "to",
    "description",
    "confidence",
];

/// Genel projeksiyon: sonucun içindeki nesne listelerinden yalnızca yapısal anahtarlar; dize listeleri yalnızca sayılır
pub(crate) fn genel_ozet(v: &Value) -> Value {
    let mut listeler = serde_json::Map::new();
    fn gez(v: &Value, yol: &str, d: usize, out: &mut serde_json::Map<String, Value>) {
        if d > 4 || out.len() > 32 {
            return;
        }
        match v {
            Value::Object(m) => {
                for (k, x) in m.iter().take(64) {
                    gez(x, &if yol.is_empty() { k.clone() } else { format!("{yol}.{k}") }, d + 1, out);
                }
            }
            Value::Array(a) if !a.is_empty() => {
                let dize = yol.to_lowercase().contains("string") || a.iter().all(Value::is_string);
                if dize {
                    out.insert(yol.to_string(), json!({ "sayi": a.len() }));
                } else if a.iter().any(Value::is_object) {
                    let ogeler: Vec<Value> = a
                        .iter()
                        .filter_map(Value::as_object)
                        .take(EN_FAZLA)
                        .map(|o| {
                            let m: serde_json::Map<String, Value> = o
                                .iter()
                                .filter(|(k, x)| IZINLI_ANAHTARLAR.contains(&k.as_str()) && (x.is_number() || x.is_boolean() || x.is_string()))
                                .map(|(k, x)| (k.clone(), if let Some(s) = x.as_str() { json!(ad(s)) } else { x.clone() }))
                                .collect();
                            Value::Object(m)
                        })
                        .collect();
                    out.insert(yol.to_string(), json!({ "sayi": a.len(), "ogeler": ogeler }));
                }
            }
            _ => {}
        }
    }
    gez(v, "", 0, &mut listeler);
    Value::Object(listeler)
}

/// Decompile çıktısı: kod saklanmaz; yalnızca satır sayısı ve SHA-256 özeti
pub(crate) fn decompile_ozeti(fonksiyon: &str, v: &Value) -> Value {
    let mut kod = String::new();
    fn topla(v: &Value, k: &str, out: &mut String) {
        match v {
            Value::Object(m) => {
                for (a, x) in m {
                    topla(x, a, out);
                }
            }
            Value::String(s) if ["code", "pseudo", "pseudocode", "pseudo_code", "text", "source"].iter().any(|x| k.to_lowercase().contains(x)) => out.push_str(s),
            _ => {}
        }
    }
    topla(v, "", &mut kod);
    json!({ "fonksiyon": ad(fonksiyon), "satir": kod.lines().count(), "sha256": if kod.is_empty() { Value::Null } else { json!(crate::benlik::sha256_hex(kod.as_bytes())) } })
}

/// Native genel bakıştan fonksiyon adları (giriş noktası önce): decompile edilecek ilk birkaç fonksiyon
pub(crate) fn giris_fonksiyonlari(ozet: &Value, n: usize) -> Vec<String> {
    let mut adlar: Vec<String> = ozet
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| ["procedure", "function", "symbol"].iter().any(|x| k.to_lowercase().contains(x)))
        .flat_map(|(_, v)| v["ogeler"].as_array().cloned().unwrap_or_default())
        .filter_map(|o| o["name"].as_str().map(String::from))
        .collect();
    adlar.sort_by_key(|a| !matches!(a.as_str(), "main" | "_main" | "entry" | "_start" | "start" | "WinMain" | "wmain"));
    adlar.dedup();
    adlar.truncate(n);
    adlar
}
