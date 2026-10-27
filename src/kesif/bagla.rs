//! Kaynaksız keşfin bağlanması: `kaynaksiz.json` kanıtları kanıt deposuna (`kanit.json`), ilişki grafiğine
//! (`graf.json`), boş bir işlem haritasına (`kapilar.json`) ve Benlik Kartı'na projekte edilir. Böylece MCP'nin `kanit`,
//! `bilinmeyenler` ve `etki` araçları, santral ve `sezgi dogrula` kaynaksız uygulamada da çalışır.
//!
//! - **Application:** artefaktı elde olan kaynaksız uygulama tam bir uygulama düğümüdür (`app:<klasör>` ya da
//!   `app:android/<paket>`); kimliği yapıtın sürüm kaynağından ya da bildiriminden gelir.
//! - **Düğümler:** ikili ve veri dosyası `mod:<yol>`, içe aktarılan kütüphane `mod:dll/<ad>`, .NET metodu
//!   `fn:<dosya>#<Tip.Metot>`, yerel giriş `fn:<kütüphane>!<giriş>`, tablo `db:<tablo>`, sütun `field:<tablo>.<sütun>`,
//!   ekran `screen:android/<sınıf>` (activity) ya da `screen:win/<tip>` (form/window, addan), izin `perm:<ad>`,
//!   yetenek `cap:<tip>.<metot>` (addan).
//! - **İlişkiler:** DEPENDS_ON (uygulama → ikili, ikili → içe aktarılan kütüphane), CALLS (IL çağrı hedefi ve
//!   P/Invoke: observed), READS/WRITES (literal SQL'den: inferred), REFERENCES (şemada açıkça tanımlı FK: dosyadan okunan
//!   şemanın tavanı observed, sütunlar ve ON DELETE/UPDATE eylemiyle; görünümün okuduğu tablo: inferred),
//!   MAY_AFFECT (ON DELETE kaskadı, tetikleyici; hiçbir zaman certain değil), REQUIRES_PERMISSION, IMPLEMENTS (addan).
//! - **Dayanak:** her kanıt yapıtın SHA-256 özetine kilitlidir; yapıt değişince kanıt eski olur.
//! - **Geniş REA kanıtları:** ELF kimliği; native genel bakış (fonksiyon, içe aktarma, çağrı) ve decompile özeti
//!   (DECOMPILED, inferred); firmware bölümleri; süreç gözlemi (RUNTIME_TRACE); sayfanın ilan ettiği WebMCP araçları
//!   (ExternalSystem `ext:web/<köken>` → IMPLEMENTS → Capability `cap:webmcp/<köken>#<araç>`; kartta `x-webmcp`, yalnızca
//!   biçim). Ticari motor çıktısı (`paylasilabilir: false`) yalnızca yerel kanıt deposunda kalır: kartın kanıt
//!   özetlerine, dayanaklarına ve kanıt paketine girmez.
//! - **Electron:** paket (`mod:<asar>`), IPC kanalı `ipc:<kanal>`; ön yüz çağrısı `client:<asar>/<dosya>:<satır>` → CALLS →
//!   `ipc:<kanal>` → CALLS → ana süreç işleyicisi `fn:<asar>/<dosya>#ipc:<kanal>` (inferred); ağ çağrıları
//!   `client:` → CALLS → `api:<YÖNTEM> <şablon>`; pencereler `screen:electron/<sayfa>` (window); fuse'lar ve pencere
//!   ayarları ikili / paket nitelikleri.
//! - **Kapsam:** bütün önermeler açık kapsamlıdır; kaynak kodu (SOURCE_CODE/API_SCHEMA) olmadığı için önizleme bu
//!   sistem için hiçbir zaman GEÇER demez. Kartta işlem yoktur (girişi bilinen bir protokol yok); yetenek ve ekranlar
//!   `x-` üyelerinde observed/inferred damgalı ve `requires_approval: true`: hiçbir doğrulayıcıda yazılabilir çıkmaz.
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::kanit::{Dayanak, Depo, Guven, Iddia, Kanit, Kapsam, KaynakTuru, Kurucu};

/// Haritanın adaptör kimliği
pub(crate) const ADAPTOR: &str = "kaynaksiz@1";
/// Ad sezgisinden türeyen önermelerin üreticisi
const AD_URETICI: &str = "sezgi/kaynaksiz-ad@1";
/// Bütün kaynaksız kanıtların ortak sınırlaması
const KAYNAKSIZ: &str = "kaynak kodu yok: kapsam açık; önizleme bu sistem için GEÇER demez, her yazma onay ister";

fn guven(s: Option<&str>) -> Option<Guven> {
    s.and_then(|s| serde_json::from_value(json!(s)).ok())
}

/// `kaynaksiz.json` kaydından kanıt deposu kaydı. Ayrıntılı projeksiyon `kaynaksiz.json`'da kalır; kanıt deposunda
/// yalnızca türü ve kaydın kimliği
fn kanit_kaydi(k: &Value) -> Option<Kanit> {
    let tur: KaynakTuru = serde_json::from_value(k["kaynak_turu"].clone()).ok()?;
    let dayanak: Vec<Dayanak> = serde_json::from_value(k["dayanak"].clone()).unwrap_or_default();
    let konum: Vec<String> = k["konum"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    let mut kn = Kanit::yeni(tur, k["uretici"].as_str()?, dayanak, konum, guven(k["beyan"].as_str()));
    let mut sin = vec![KAYNAKSIZ.to_string()];
    sin.extend(k["sinirlamalar"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)));
    kn.sinirlamalar = sin;
    kn.ozet = json!({ "tur": k["iddia_turu"], "kaynaksiz_kanit": k["id"], "kaynak_kanit": k["kaynak_kanit"] });
    if k["paylasilabilir"] == false {
        kn.ozet["paylasilabilir"] = json!(false);
    }
    if let Some(y) = k["yalitim"].as_str() {
        kn.ozet["yalitim"] = json!(y);
    }
    kn.id = Kanit::kimlik_hesapla(&kn);
    Some(kn)
}

/// Aynı dayanaktan, ad ya da dize sezgisine dayanan önermeler için ayrı kanıt (BINARY_HEURISTIC, en fazla inferred)
fn ad_kaniti(asil: &Kanit, tur: &str) -> Kanit {
    let mut k = Kanit::yeni(KaynakTuru::BinaryHeuristic, AD_URETICI, asil.dayanak.clone(), asil.konum.clone(), Some(Guven::Inferred));
    k.sinirlamalar = vec![KAYNAKSIZ.to_string(), "addan çıkarım: tip ya da metot adı yorumlandı".into()];
    k.ozet = json!({ "tur": tur });
    k.id = Kanit::kimlik_hesapla(&k);
    k
}

fn i(varlik: &str, yuklem: &str, hedef: Option<&str>, deger: Value) -> Iddia {
    Iddia::yeni(varlik, yuklem, hedef, deger, Kapsam::Acik)
}

/// SQL tanımlayıcısı düğüm anahtarı olarak (SQLite adları büyük/küçük harf duyarsız)
fn tablo_id(t: &str) -> String {
    format!("db:{}", t.to_lowercase())
}

/// Tetikleyici gövdesindeki (BEGIN ... END) yazma hedefleri; gövde çalıştırılmaz, yalnızca metin okunur
fn govde_hedefleri(metin: &str) -> Vec<String> {
    let govde = metin.to_uppercase().find("BEGIN").map(|i| &metin[i..]).unwrap_or("");
    let k: Vec<&str> = govde.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '"' || c == '`' || c == '[' || c == ']' || c == '.')).filter(|w| !w.is_empty()).collect();
    let mut v = Vec::new();
    for (n, w) in k.iter().enumerate() {
        let w = w.to_uppercase();
        let hedef = match w.as_str() {
            "INTO" | "UPDATE" => k.get(n + 1),
            "FROM" if n > 0 && k[n - 1].eq_ignore_ascii_case("DELETE") => k.get(n + 1),
            _ => None,
        };
        if let Some(h) = hedef {
            let h = h.trim_matches(|c| matches!(c, '"' | '`' | '[' | ']')).rsplit('.').next().unwrap_or("").to_string();
            if !h.is_empty() && !matches!(h.to_uppercase().as_str(), "OR" | "SELECT") && !v.contains(&h) {
                v.push(h);
            }
        }
    }
    v
}

/// Sütunun okunur özeti: ad, tür ve sütun kısıtları ("tutar REAL NOT NULL CHECK (tutar > 0)")
fn sutun_ozeti(s: &Value) -> String {
    let mut p = vec![s["ad"].as_str().unwrap_or("?").to_string()];
    if let Some(t) = s["tur"].as_str().filter(|t| !t.is_empty()) {
        p.push(t.to_string());
    }
    for (k, m) in [("birincil", "PRIMARY KEY"), ("bos_olamaz", "NOT NULL"), ("benzersiz", "UNIQUE"), ("varsayilan", "DEFAULT")] {
        if s[k] == true {
            p.push(m.into());
        }
    }
    for c in s["kontroller"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        p.push(format!("CHECK ({c})"));
    }
    if let Some(r) = s["referans"].as_object() {
        p.push(format!("REFERENCES {}", r.get("tablo").and_then(Value::as_str).unwrap_or("?")));
    }
    p.join(" ")
}

/// Görünüm metnindeki FROM / JOIN tabloları
fn okunan_tablolar(metin: &str) -> Vec<String> {
    let k: Vec<&str> = metin.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')).filter(|w| !w.is_empty()).collect();
    let mut v = Vec::new();
    for (n, w) in k.iter().enumerate() {
        if (w.eq_ignore_ascii_case("FROM") || w.eq_ignore_ascii_case("JOIN")) && n + 1 < k.len() {
            let t = k[n + 1].rsplit('.').next().unwrap_or("").to_string();
            if !t.is_empty() && !t.eq_ignore_ascii_case("SELECT") && !v.contains(&t) {
                v.push(t);
            }
        }
    }
    v
}

/// Derleyicinin ürettiği ya da erişimci üye mi (yetenek sayılmaz)
fn yardimci_uye(ad: &str) -> bool {
    let metot = ad.rsplit('.').next().unwrap_or(ad);
    ad.contains('<') || ad.ends_with("..ctor") || ad.ends_with("..cctor") || metot.starts_with("get_") || metot.starts_with("set_") || metot == "Main"
}

/// Bağlamanın sonucu: kanıt deposu ve kartın yetenek/ekran listesi
pub(crate) struct Baglanti {
    pub depo: Depo,
    pub uygulama: String,
    pub ad: String,
    pub surum: String,
    pub yetenekler: Vec<Value>,
    pub ekranlar: Vec<Value>,
    /// Sayfaların ilan ettiği WebMCP araçları (kartta `x-webmcp`)
    pub webmcp: Vec<Value>,
}

/// `kaynaksiz.json` belgesinden kanıt deposu
pub(crate) fn depo(belge: &Value, zaman: &str) -> Baglanti {
    let kanitlar: &[Value] = belge["kanitlar"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    // Uygulama düğümü: APK'da paket adı, değilse yapıtın (klasörün) adı
    let apk = kanitlar.iter().find(|k| k["uretici"] == crate::android::URETICI);
    let (uygulama, mut ad, mut surum) = match apk {
        Some(a) => {
            let p = a["ozet"]["paket"].as_str().unwrap_or("?").to_string();
            (format!("app:android/{p}"), p, a["ozet"]["surum_adi"].as_str().unwrap_or("").to_string())
        }
        None => {
            let y = belge["yapit"].as_str().unwrap_or("uygulama").to_string();
            (format!("app:{y}"), y, String::new())
        }
    };
    let mut k = Kurucu::default();
    let mut yetenekler: BTreeMap<String, Value> = BTreeMap::new();
    let mut ekranlar: BTreeMap<String, Value> = BTreeMap::new();
    let mut webmcp: Vec<Value> = Vec::new();
    // .NET tipleri (yapıt yolu → tip adları): çağrı hedefinin uygulamanın içinde olup olmadığı
    let mut tipler: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for e in kanitlar.iter().filter(|e| e["konum"][0].as_str().is_some_and(|k| k.ends_with("#inspect_managed_members"))) {
        let yol = e["dayanak"][0]["yol"].as_str().unwrap_or("").to_string();
        tipler.entry(yol).or_default().extend(e["ozet"]["tipler"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(String::from)));
    }
    for e in kanitlar {
        let Some(kn) = kanit_kaydi(e) else { continue };
        let o = &e["ozet"];
        let yol = e["dayanak"][0]["yol"].as_str().unwrap_or("").to_string();
        let konum = e["konum"][0].as_str().unwrap_or("");
        let ek = konum.split_once('#').map(|(_, x)| x).unwrap_or("");
        let uretici = e["uretici"].as_str().unwrap_or("");
        let modul = format!("mod:{yol}");
        let mut iddialar: Vec<Iddia> = vec![];
        let mut ad_iddialari: Vec<Iddia> = vec![];
        if uretici == "sezgi/pe@1" && ek.is_empty() {
            iddialar.push(i(
                &modul,
                "VAR",
                None,
                json!({ "bicim": "pe", "makine": o["makine"], "alt_sistem": o["alt_sistem"], "dll": o["dll"], "dotnet": o["dotnet"], "clr_surumu": o["clr_surumu"], "dosya_surumu": o["dosya_surumu"] }),
            ));
            iddialar.push(i(&uygulama, "DEPENDS_ON", Some(&modul), json!({})));
            for imp in o["importlar"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                iddialar.push(i(&modul, "DEPENDS_ON", Some(&format!("mod:dll/{}", imp.to_lowercase())), json!({ "import": true })));
            }
            let sk = &o["surum_kaynagi"];
            if let Some(urun) = sk["ProductName"].as_str() {
                iddialar.push(i(&uygulama, "KIMLIK", None, json!({ "ad": urun, "surum": sk["ProductVersion"], "sirket": sk["CompanyName"] })));
                if surum.is_empty() {
                    surum = sk["ProductVersion"].as_str().or(sk["FileVersion"].as_str()).unwrap_or("").to_string();
                    ad = urun.to_string();
                }
            }
        } else if uretici == "sezgi/pe@1" && ek == "hedef_cati" {
            iddialar.push(i(&modul, "CATI", None, o["hedef_cati"].clone()));
        } else if uretici == "sezgi/sqlite@1" && ek == "dosya" {
            iddialar.push(i(&modul, "VAR", None, json!({ "bicim": "sqlite3", "kodlama": o["kodlama"], "wal": o["wal"] })));
            iddialar.push(i(&uygulama, "DEPENDS_ON", Some(&modul), json!({})));
        } else if let Some(t) = ek.strip_prefix("tablo:").filter(|_| uretici == "sezgi/sqlite@1") {
            let db = tablo_id(t);
            let sutunlar = o["sutunlar"].as_array().map(Vec::as_slice).unwrap_or(&[]);
            // Sütunlar ve kısıtlar kanıtın değerinde: ajan `kanit db:<tablo>` ile ayrıntıyı görür
            let ozet: Vec<String> = sutunlar.iter().map(sutun_ozeti).collect();
            let bayrakli = |k: &str| -> Vec<&Value> { sutunlar.iter().filter(|s| s[k] == true).map(|s| &s["ad"]).collect() };
            iddialar.push(i(&db, "VAR", None, json!({ "ad": t, "sutun": sutunlar.len(), "sutunlar": ozet, "rowidsiz": o["rowidsiz"], "dosya": yol })));
            iddialar.push(i(
                &db,
                "KISITLAR",
                None,
                json!({ "birincil": o["birincil_anahtar"], "benzersiz": o["benzersiz"], "kontroller": o["kontroller"], "bos_olamaz": bayrakli("bos_olamaz"), "benzersiz_sutunlar": bayrakli("benzersiz") }),
            ));
            let indeksler: Vec<Value> =
                o["indeksler"].as_array().into_iter().flatten().map(|x| json!({ "ad": x["ad"], "sutunlar": x["sutunlar"], "benzersiz": x["benzersiz"] })).collect();
            if !indeksler.is_empty() {
                iddialar.push(i(&db, "INDEKSLER", None, json!(indeksler)));
            }
            for s in sutunlar {
                let sad = s["ad"].as_str().unwrap_or("?");
                iddialar.push(i(
                    &format!("field:{}.{}", t.to_lowercase(), sad.to_lowercase()),
                    "VAR",
                    None,
                    json!({ "ad": sad, "tur": s["tur"], "bos_olamaz": s["bos_olamaz"], "birincil": s["birincil"], "benzersiz": s["benzersiz"], "varsayilan": s["varsayilan"], "kontroller": s["kontroller"] }),
                ));
            }
            for f in o["yabanci_anahtarlar"].as_array().into_iter().flatten() {
                let ust = tablo_id(f["tablo"].as_str().unwrap_or("?"));
                // Şemada açıkça tanımlı yabancı anahtar: yönü ve sütunları bellidir (`bildirilmis`); eylemler kanıtta
                iddialar.push(i(
                    &db,
                    "REFERENCES",
                    Some(&ust),
                    json!({ "sutunlar": f["kaynak_sutunlar"], "hedef_sutunlar": f["sutunlar"], "silinince": f["silinince"], "guncellenince": f["guncellenince"], "bildirilmis": true }),
                ));
                if let Some(e) = f["silinince"].as_str().filter(|e| matches!(*e, "CASCADE" | "SET NULL" | "SET DEFAULT")) {
                    iddialar.push(i(&ust, "MAY_AFFECT", Some(&db), json!({ "neden": format!("ON DELETE {e}") })));
                }
            }
            for tt in o["tetikleyiciler"].as_array().into_iter().flatten() {
                for h in govde_hedefleri(tt["metin"].as_str().unwrap_or("")) {
                    iddialar.push(i(&db, "MAY_AFFECT", Some(&tablo_id(&h)), json!({ "tetikleyici": tt["ad"], "olay": tt["olay"], "zaman": tt["zaman"] })));
                }
            }
        } else if let Some(g) = ek.strip_prefix("gorunum:").filter(|_| uretici == "sezgi/sqlite@1") {
            let db = tablo_id(g);
            iddialar.push(i(&db, "VAR", None, json!({ "ad": g, "gorunum": true, "dosya": yol })));
            for t in okunan_tablolar(o["metin"].as_str().unwrap_or("")) {
                iddialar.push(i(&db, "REFERENCES", Some(&tablo_id(&t)), json!({ "gorunum": true })));
            }
        } else if ek == "inspect_managed_members" {
            iddialar.push(i(&modul, "TIPLER", None, o["tipler"].clone()));
            for u in o["uyeler"].as_array().into_iter().flatten() {
                let uad = u["ad"].as_str().unwrap_or("?");
                iddialar.push(i(&format!("fn:{yol}#{uad}"), "VAR", None, json!({ "donus": u["donus"], "parametreler": u["parametreler"] })));
                if !yardimci_uye(uad) {
                    let (tip, metot) = uad.rsplit_once('.').unwrap_or(("", uad));
                    let cap = format!("cap:{}.{}", tip.rsplit('.').next().unwrap_or(tip).to_lowercase(), metot.to_lowercase());
                    ad_iddialari.push(i(&format!("fn:{yol}#{uad}"), "IMPLEMENTS", Some(&cap), json!({})));
                    yetenekler.entry(cap.clone()).or_insert_with(|| json!({ "id": cap, "implemented_by": format!("fn:{yol}#{uad}"), "kind": "managed-method" }));
                }
            }
            for t in o["tipler"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                let kisa = t.rsplit('.').next().unwrap_or(t);
                let tur = if kisa.ends_with("Form") || kisa.ends_with("Dialog") {
                    "form"
                } else if kisa.ends_with("Window") || kisa.ends_with("Page") {
                    "window"
                } else {
                    continue;
                };
                let id = format!("screen:win/{t}");
                ad_iddialari.push(i(&id, "VAR", None, json!({ "tur": tur, "tip": t })));
                ekranlar.insert(id.clone(), json!({ "id": id, "kind": tur, "stamp": "inferred" }));
            }
        } else if ek == "inspect_managed_members:cagrilar" {
            let ic = tipler.get(&yol).cloned().unwrap_or_default();
            for c in o["cagrilar"].as_array().into_iter().flatten() {
                let (Some(kaynak), Some(hedef)) = (c["cagiran"].as_str(), c["hedef"].as_str()) else { continue };
                if ic.iter().any(|t| hedef.starts_with(&format!("{t}."))) {
                    iddialar.push(i(&format!("fn:{yol}#{kaynak}"), "CALLS", Some(&format!("fn:{yol}#{hedef}")), json!({ "komut": c["komut"] })));
                }
            }
        } else if ek == "inspect_managed_native_boundaries" {
            for p in o["pinvoke"].as_array().into_iter().flatten() {
                let dll = p["dll"].as_str().unwrap_or("?").to_lowercase();
                iddialar.push(i(&modul, "CALLS", Some(&format!("fn:{dll}!{}", p["giris"].as_str().unwrap_or("?"))), json!({ "pinvoke": p["uye"] })));
                iddialar.push(i(&modul, "DEPENDS_ON", Some(&format!("mod:dll/{dll}")), json!({ "pinvoke": true })));
            }
        } else if ek == "inspect_managed_members:sql" {
            for s in o["sql"].as_array().into_iter().flatten() {
                let islem = s["islem"].as_str().unwrap_or("").to_uppercase();
                let yuklem = match islem.as_str() {
                    "SELECT" | "WITH" => "READS",
                    "INSERT" | "UPDATE" | "DELETE" | "REPLACE" => "WRITES",
                    _ => "REFERENCES",
                };
                let metotlar: Vec<String> = s["metotlar"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(|m| format!("fn:{yol}#{m}"))).collect();
                for t in s["tablolar"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    let kaynaklar = if metotlar.is_empty() { vec![modul.clone()] } else { metotlar.clone() };
                    for kaynak in kaynaklar {
                        iddialar.push(i(&kaynak, yuklem, Some(&tablo_id(t)), json!({ "islem": islem.to_lowercase() })));
                    }
                }
            }
        } else if uretici == "sezgi/elf@1" && ek.is_empty() {
            iddialar.push(i(&modul, "VAR", None, json!({ "bicim": "elf", "makine": o["makine"], "sinif": o["sinif"], "tur": o["tur"] })));
            iddialar.push(i(&uygulama, "DEPENDS_ON", Some(&modul), json!({})));
        } else if ek == "inspect" {
            // Native genel bakış: fonksiyon/sembol listeleri, içe aktarılan kütüphaneler, çağrı kenarları
            for (anahtar, liste) in o.as_object().into_iter().flatten() {
                let k = anahtar.to_lowercase();
                for x in liste["ogeler"].as_array().into_iter().flatten() {
                    if let (Some(a), Some(b)) = (x["caller"].as_str().or(x["from"].as_str()), x["callee"].as_str().or(x["to"].as_str())) {
                        iddialar.push(i(&format!("fn:{yol}#{a}"), "CALLS", Some(&format!("fn:{yol}#{b}")), json!({})));
                    } else if let Some(ad) = x["name"].as_str() {
                        if ["procedure", "function", "symbol"].iter().any(|t| k.contains(t)) {
                            iddialar.push(i(&format!("fn:{yol}#{ad}"), "VAR", None, json!({ "adres": x["address"], "boyut": x["size"] })));
                        } else if ["import", "librar"].iter().any(|t| k.contains(t)) {
                            iddialar.push(i(&modul, "DEPENDS_ON", Some(&format!("mod:dll/{}", ad.to_lowercase())), json!({ "import": true })));
                        }
                    }
                }
            }
            iddialar.push(i(&modul, "NATIVE", None, json!(o.as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v["sayi"].clone())).collect::<Map<String, Value>>()))));
        } else if let Some(f) = ek.strip_prefix("decompile:") {
            iddialar.push(i(&format!("fn:{yol}#{f}"), "PSEUDO", None, json!({ "satir": o["satir"], "sha256": o["sha256"] })));
        } else if ek == "inspect_firmware_regions" {
            iddialar.push(i(&modul, "BOLUMLER", None, o.clone()));
            iddialar.push(i(&uygulama, "DEPENDS_ON", Some(&modul), json!({})));
        } else if ek == "capture_process" {
            iddialar.push(i(&modul, "CALISMA", None, json!({ "cikis": o["cikis"], "dosya_etkisi": o["dosya_etkileri"].as_array().map_or(0, Vec::len), "yalitim": e["yalitim"] })));
        } else if ek == "discover_webmcp_tools" {
            let koken = o["koken"].as_str().unwrap_or("?").to_string();
            let ext = format!("ext:web/{koken}");
            iddialar.push(i(&ext, "VAR", None, json!({ "tur": "web", "durum": o["durum"] })));
            iddialar.push(i(&uygulama, "DEPENDS_ON", Some(&ext), json!({})));
            let mut araclar = vec![];
            for t in o["araclar"].as_array().into_iter().flatten() {
                let ad = t["ad"].as_str().unwrap_or("?");
                let cap = format!("cap:webmcp/{koken}#{ad}");
                iddialar.push(i(&cap, "VAR", None, json!({ "girdi": t["girdi"], "salt_okunur": t["salt_okunur"], "sonuclu": t["sonuclu"], "tur": t["tur"] })));
                iddialar.push(i(&ext, "IMPLEMENTS", Some(&cap), json!({})));
                araclar.push(json!({ "id": cap, "name": ad, "kind": t["tur"], "inputs": t["girdi"], "input_schema_sha256": t["girdi_sha256"], "read_only": t["salt_okunur"], "consequential": t["sonuclu"] }));
            }
            webmcp.push(json!({ "origin": koken, "status": o["durum"], "tools": araclar, "untrusted": true }));
        } else if uretici == "sezgi/asar@1" && ek == "paket" {
            iddialar.push(i(&modul, "VAR", None, json!({ "bicim": "asar", "ad": o["ad"], "surum": o["surum"], "giris": o["giris"], "js": o["js"] })));
            iddialar.push(i(&uygulama, "DEPENDS_ON", Some(&modul), json!({})));
            if let Some(a) = o["urun_adi"].as_str().or(o["ad"].as_str()) {
                iddialar.push(i(&uygulama, "KIMLIK", None, json!({ "ad": a, "surum": o["surum"], "tur": "electron" })));
                if surum.is_empty() {
                    surum = o["surum"].as_str().unwrap_or("").to_string();
                    ad = a.to_string();
                }
            }
        } else if uretici == "sezgi/asar@1" && ek == "ipc" {
            for k in o["kanallar"].as_array().into_iter().flatten() {
                let kanal = k["kanal"].as_str().unwrap_or("?");
                let ipc = format!("ipc:{kanal}");
                iddialar.push(i(&ipc, "VAR", None, json!({ "kanal": kanal })));
                for h in k["isleyiciler"].as_array().into_iter().flatten() {
                    let f = format!("fn:{yol}/{}#ipc:{kanal}", h["dosya"].as_str().unwrap_or("?"));
                    iddialar.push(i(&ipc, "CALLS", Some(&f), json!({})));
                }
                for c in k["cagiranlar"].as_array().into_iter().flatten() {
                    let cl = format!("client:{yol}/{}:{}", c["dosya"].as_str().unwrap_or("?"), c["satir"]);
                    iddialar.push(i(&cl, "CALLS", Some(&ipc), json!({})));
                }
            }
            let tercihler = o["web_tercihleri"].as_array().cloned().unwrap_or_default();
            if !tercihler.is_empty() {
                iddialar.push(i(&modul, "WEB_TERCIHLERI", None, json!(tercihler.iter().map(|t| json!({ "ad": t["ad"], "deger": t["deger"] })).collect::<Vec<_>>())));
            }
            for p in o["sayfalar"].as_array().into_iter().flatten() {
                let id = format!("screen:electron/{}", p["sayfa"].as_str().unwrap_or("?"));
                ad_iddialari.push(i(&id, "VAR", None, json!({ "tur": "window" })));
                ekranlar.insert(id.clone(), json!({ "id": id, "kind": "window", "stamp": "inferred" }));
            }
        } else if uretici == "sezgi/web" && ek == "web" {
            for c in o["cagrilar"].as_array().into_iter().flatten() {
                let cl = format!("client:{yol}/{}:{}", c["dosya"].as_str().unwrap_or("?"), c["satir"]);
                let api = format!("api:{} {}", c["yontem"].as_str().unwrap_or("*"), c["sablon"].as_str().unwrap_or("?"));
                iddialar.push(i(&cl, "CALLS", Some(&api), json!({})));
            }
            for b in o["baglar"].as_array().into_iter().flatten() {
                if let (Some(k), Some(il), Some(h)) = (b["kaynak"].as_str(), b["iliski"].as_str(), b["hedef"].as_str()) {
                    iddialar.push(i(k, il, Some(h), json!({})));
                }
            }
        } else if ek == "fuses" {
            iddialar.push(i(&modul, "FUSELAR", None, o["fuselar"].clone()));
        } else if ek == "analyze_javascript_application" {
            // REA uygulama grafiği: işleyici eşleşmeleri Sezgi'nin desen bağlarıyla aynı kenarları destekler
            for h in o["isleyiciler"].as_array().into_iter().flatten() {
                if let Some(kanal) = h["kanal"].as_str() {
                    iddialar.push(i(&format!("ipc:{kanal}"), "VAR", None, json!({ "kanal": kanal })));
                }
            }
            for m in o["eslesmeler"].as_array().into_iter().flatten() {
                let (Some(kanal), Some(d)) = (m["kanal"].as_str(), m["isleyici_dosyasi"].as_str()) else { continue };
                iddialar.push(i(&format!("ipc:{kanal}"), "CALLS", Some(&format!("fn:{yol}/{d}#ipc:{kanal}")), json!({})));
            }
            iddialar.push(i(&modul, "REA_ELECTRON", None, json!({ "kopruler": o["kopruler"], "web_tercihleri": o["web_tercihleri"], "ozet": o["ozet"] })));
        } else if uretici == crate::android::URETICI {
            iddialar.push(i(
                &uygulama,
                "VAR",
                None,
                json!({ "tur": "android-apk", "paket": o["paket"], "surum_adi": o["surum_adi"], "surum_kodu": o["surum_kodu"], "sdk": o["sdk"] }),
            ));
            iddialar.push(i(&uygulama, "AYARLAR", None, o["uygulama"].clone()));
            for p in o["izinler"].as_array().into_iter().flatten().filter_map(|p| p["ad"].as_str()) {
                iddialar.push(i(&uygulama, "REQUIRES_PERMISSION", Some(&format!("perm:{p}")), json!({})));
            }
            for p in o["tanimlanan_izinler"].as_array().into_iter().flatten() {
                iddialar.push(i(&format!("perm:{}", p["ad"].as_str().unwrap_or("?")), "VAR", None, json!({ "koruma_duzeyi": p["koruma_duzeyi"], "tanimlayan": uygulama })));
            }
            for b in o["bilesenler"].as_array().into_iter().flatten() {
                let tur = b["tur"].as_str().unwrap_or("?");
                let sinif = b["ad"].as_str().unwrap_or("?");
                let id = if tur.starts_with("activity") { format!("screen:android/{sinif}") } else { format!("mod:android/{sinif}") };
                iddialar.push(i(
                    &id,
                    "VAR",
                    None,
                    json!({ "tur": tur, "disa_acik": b["disa_acik"], "baslatici": b["baslatici"], "etkin": b["etkin"], "intent_filtresi": b["intent_filtreleri"].as_array().map_or(0, Vec::len) }),
                ));
                if let Some(p) = b["izin"].as_str() {
                    iddialar.push(i(&id, "REQUIRES_PERMISSION", Some(&format!("perm:{p}")), json!({})));
                }
                if tur.starts_with("activity") {
                    ekranlar.insert(id.clone(), json!({ "id": id, "kind": "activity", "stamp": "observed", "exported": b["disa_acik"], "launcher": b["baslatici"] }));
                }
                // Dışa açık bileşen başka uygulamaların çağırabildiği bir giriş noktasıdır: kartta yetenek (onaylı)
                if b["disa_acik"] == true {
                    let cap = format!("cap:android.{}", sinif.rsplit('.').next().unwrap_or(sinif).to_lowercase());
                    yetenekler.entry(cap.clone()).or_insert_with(|| json!({ "id": cap, "implemented_by": id, "kind": format!("android-{tur}"), "stamp": "observed" }));
                }
            }
        }
        if !ad_iddialari.is_empty() {
            k.ekle(ad_kaniti(&kn, "adlandirma"), ad_iddialari);
        }
        if !iddialar.is_empty() {
            k.ekle(kn, iddialar);
        }
    }
    // Bilinmeyenler: kanıtı olmayan, notlu önermeler (yokluk iddia edilmez)
    for b in belge["bilinmeyenler"].as_array().into_iter().flatten() {
        let yol = b["yol"].as_str().unwrap_or("");
        let varlik = if yol.is_empty() || yol.ends_with(".apk") { uygulama.clone() } else { format!("mod:{yol}") };
        let konu = b["konu"].as_str().unwrap_or("?");
        k.bos(Iddia::yeni(&varlik, "BILINMIYOR", Some(konu), Value::Null, Kapsam::Bilinmiyor), b["neden"].as_str().unwrap_or(""));
    }
    // Önizleme: kaynak kodu olmayan sistemde kapalı dünya hiçbir zaman kurulmaz
    k.bos(Iddia::yeni(&uygulama, "ONIZLEME", None, Value::Null, Kapsam::Bilinmiyor), KAYNAKSIZ);
    let mut depo = k.bitir(zaman);
    depo.uretici = format!("sezgi {} {ADAPTOR}", env!("CARGO_PKG_VERSION"));
    // Yetenek ve ekranların damgası grafikteki önermeden (adlandırma: inferred; bildirim: observed)
    let damga = |id: &str| depo.onermeler.iter().filter(|o| o.varlik == id || o.hedef.as_deref() == Some(id)).map(|o| o.hesap.guven).max().unwrap_or(Guven::Unknown);
    let yetenekler = yetenekler
        .into_values()
        .map(|mut c| {
            // Addan çıkan yetenek IMPLEMENTS kenarının damgasını (inferred), bildirimdeki bileşen kendi damgasını alır
            let g = match damga(c["id"].as_str().unwrap_or("")) {
                Guven::Unknown => damga(c["implemented_by"].as_str().unwrap_or("")),
                g => g,
            }
            .min(Guven::Observed);
            c["stamp"] = json!(g.ad());
            c["requires_approval"] = json!(true);
            c["coverage"] = json!("open");
            c
        })
        .collect();
    let ekranlar = ekranlar
        .into_values()
        .map(|mut e| {
            let g = damga(e["id"].as_str().unwrap_or("")).min(Guven::Observed);
            e["stamp"] = json!(g.ad());
            e["requires_approval"] = json!(true);
            e
        })
        .collect();
    // WebMCP araçlarının damgası grafikten (yalıtımsız gözlem inferred); hepsi onaylı
    let webmcp = webmcp
        .into_iter()
        .map(|mut w| {
            for t in w["tools"].as_array_mut().into_iter().flatten() {
                let g = damga(t["id"].as_str().unwrap_or("")).min(Guven::Observed);
                t["stamp"] = json!(g.ad());
                t["requires_approval"] = json!(true);
            }
            w
        })
        .collect();
    Baglanti { depo, uygulama, ad, surum, yetenekler, ekranlar, webmcp }
}

/// Ticari motor çıktısı paylaşılamaz (karta ve kanıt paketine girmez)
fn paylasilir(k: &Kanit) -> bool {
    k.ozet["paylasilabilir"] != false
}

/// Kanıt deposundaki kanıtların paylaşılabilir, değer taşımayan kayıtları (kartın kanıt paketi)
fn kanit_kayitlari(depo: &Depo) -> Vec<Value> {
    depo.kanitlar
        .iter()
        .filter(|k| paylasilir(k))
        .map(|k| {
            let ilk = k.dayanak.first();
            let iddia = match k.ozet["tur"].as_str() {
                Some("call_target") => "call_target",
                Some("data_effect") => "data_effect",
                _ => "structure",
            };
            json!({
                "id": k.id,
                "kind": k.kaynak_turu.ad(),
                "claim_kind": iddia,
                "subject": { "file": ilk.map(|d| d.yol.clone()), "sha256": ilk.map(|d| d.sha256.clone()) },
                "provider": { "name": k.uretici },
                "confidence": k.katki().ad(),
                "authority": if k.uretici == AD_URETICI || k.kaynak_turu == KaynakTuru::BinaryHeuristic { "analyst-inference" } else { "shipped-artifact" },
                "shareable": true,
                "contradicts": [],
            })
        })
        .collect()
}

/// Kaynaksız uygulamanın Benlik Kartı ve kanıt paketi. İşlem yok; yetenek ve ekranlar `x-` üyelerinde, hepsi onaylı
pub(crate) fn kart(b: &Baglanti, kaynak_ref: &str, verilis_sn: i64, gun: u64) -> (Value, Value) {
    let kayitlar = kanit_kayitlari(&b.depo);
    let ozetler: Vec<Value> = kayitlar
        .iter()
        .map(|k| {
            json!({ "id": k["id"], "kind": k["kind"], "claim_kind": k["claim_kind"], "confidence": k["confidence"], "authority": k["authority"], "shareable": k["shareable"], "sha256": crate::benlik::kayit_ozeti(k) })
        })
        .collect();
    let kok = crate::benlik::merkle_koku(&ozetler.iter().map(|o| o["sha256"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>());
    let mut dayanak = Map::new();
    for k in b.depo.kanitlar.iter().filter(|k| paylasilir(k)) {
        for d in &k.dayanak {
            dayanak.entry(d.yol.clone()).or_insert_with(|| json!(d.sha256));
        }
    }
    let ad = if b.ad.is_empty() { "uygulama".to_string() } else { b.ad.clone() };
    let mut kart = json!({
        "benlik": crate::benlik::SURUM,
        "app": { "id": crate::benlik::uygulama_kimligi(&ad), "name": ad, "version": b.surum, "source_ref": kaynak_ref },
        "generator": { "name": "sezgi", "version": env!("CARGO_PKG_VERSION"), "adapters": [ADAPTOR], "generated_at": crate::benlik::zaman_metni(verilis_sn) },
        // Kaynak kodu olmayan sistemde ajanın çağırabileceği, girişi bilinen bir işlem yoktur
        "operations": [],
        "coverage": { "operations_found": 0, "operations_complete": 0, "rules_by_stamp": { "certain": 0, "data": 0, "approved": 0, "inferred": 0, "unknown": 0 } },
        "identity": { "model": "user-session", "write_requires_human": true, "service_account_writes": false },
        "data_ledger": [
            { "class": "code_derived", "stored": true, "contents": "artifact identity, schema, evidence stamps", "retention_days": null },
            { "class": "operational_log", "stored": false, "retention_days": 0 },
            { "class": "customer_data", "stored": false, "retention_days": 0 }
        ],
        "x-sourceless": true,
        "x-app_node": b.uygulama,
        "x-preview": { "passes": false, "coverage": "open", "reason": "no source code (SOURCE_CODE/API_SCHEMA): binary, schema and provider evidence only; every write requires human approval" },
        "x-capabilities": b.yetenekler,
        "x-screens": b.ekranlar,
        "x-webmcp": b.webmcp,
        "x-issued_at": crate::benlik::zaman_metni(verilis_sn),
        "x-expires_at": crate::benlik::zaman_metni(verilis_sn + gun as i64 * 86_400),
        "x-basis": dayanak,
        "x-evidence": ozetler,
        "x-evidence_root": kok,
        "x-evidence_bundle": crate::benlik::KANIT_PAKETI,
        "signature": { "alg": "none", "key_id": "unsigned-dev", "public_key_url": "/.well-known/benlik-keys.json", "value": "UNSIGNED-DEV-BUILD" }
    });
    let revizyon =
        format!("sha256:{}", crate::benlik::kayit_ozeti(&json!([kart["operations"], kart["x-capabilities"], kart["x-screens"], kart["x-webmcp"], kart["x-evidence_root"]])));
    kart["x-revision"] = json!(revizyon);
    let paket = json!({ "benlik_evidence": crate::benlik::SURUM, "app_id": kart["app"]["id"], "card_revision": revizyon, "records": kayitlar });
    (kart, paket)
}

/// Boş işlem haritası: MCP, santral ve doktor kaynaksız uygulamayı da açabilsin
pub(crate) fn harita(b: &Baglanti, kaynak_ref: &str) -> Value {
    let mut okunan: BTreeMap<String, String> = BTreeMap::new();
    for k in &b.depo.kanitlar {
        for d in &k.dayanak {
            okunan.entry(d.yol.clone()).or_insert_with(|| d.sha256.clone());
        }
    }
    json!({
        "ozet": {
            "uygulama": if b.ad.is_empty() { "uygulama" } else { &b.ad },
            "uretici": format!("sezgi {}", env!("CARGO_PKG_VERSION")),
            "adaptor": ADAPTOR,
            "kaynaksiz": true,
            "uygulama_dugumu": b.uygulama,
            "kaynak_ref": kaynak_ref,
            "endpoint_sayisi": 0,
            "kanit_sayisi": b.depo.kanitlar.len(),
            "okunan_dosyalar": okunan.iter().map(|(y, o)| json!({ "yol": y, "ozet": o })).collect::<Vec<_>>(),
        },
        "endpointler": [],
    })
}

/// Bütün çıktıları yazar: kanit.json + graf.json, kapilar.json, benlik.json ve kanıt paketi. `kok`: yapıtın kökü
/// (dayanak yolları buna göreli); çıktı kökün içindeyse geçerlilik izlenir. `imzala`: anahtar adı
pub(crate) fn yaz(cikti: &Path, kok: &Path, belge: &Value, imzala: Option<&str>) -> Result<(String, usize, usize), String> {
    let verilis = crate::simdi_sn();
    let mut b = depo(belge, &crate::benlik::zaman_metni(verilis));
    b.depo.kok = std::fs::canonicalize(kok).ok().zip(std::fs::canonicalize(cikti).ok()).and_then(|(k, c)| crate::kanit::kok_goreli(&k, &c));
    // Kaynak referansı: yapıtın derleme özeti (APK ya da birincil ikili; yoksa bütün dayanakların özeti)
    let kaynak_ref = {
        let mut d: Vec<String> = b.depo.kanitlar.iter().flat_map(|k| k.dayanak.iter().map(|d| format!("{}={}", d.yol, d.sha256))).collect();
        d.sort();
        d.dedup();
        format!("sha256:{}", &crate::benlik::sha256_hex(d.join("\n").as_bytes())[..16])
    };
    crate::kanit::yaz(cikti, &mut b.depo)?;
    crate::guvenli::atomik_yaz(&cikti.join("kapilar.json"), serde_json::to_string_pretty(&harita(&b, &kaynak_ref)).unwrap_or_default().as_bytes())?;
    let (mut kart, mut paket) = kart(&b, &kaynak_ref, verilis, 30);
    let imza = match imzala {
        Some(kimlik) => {
            crate::imza::imzala(&mut kart, kimlik).and_then(|_| crate::imza::imzala_baglamla(&mut paket, kimlik, crate::imza::KANIT_BAGLAMI))?;
            std::fs::copy(crate::imza::acik_anahtar_dosyasi(kimlik), cikti.join("benlik-keys.json"))
                .map_err(|e| tf!("açık anahtar belgesi kopyalanamadı: {e}", "could not copy the public key document: {e}"))?;
            tf!("ML-DSA-65 ile imzalandı (anahtar '{kimlik}')", "signed with ML-DSA-65 (key '{kimlik}')")
        }
        None => tf!("imzasız (imzalamak için: {})", "unsigned (to sign: {})", crate::imza::imzalama_ipucu(t!("sezgi kesfet <yapıt>", "sezgi discover <artifact>"))),
    };
    crate::guvenli::atomik_yaz(&cikti.join(crate::benlik::KANIT_PAKETI), serde_json::to_string_pretty(&paket).unwrap_or_default().as_bytes())?;
    crate::guvenli::atomik_yaz(&cikti.join("benlik.json"), serde_json::to_string_pretty(&kart).unwrap_or_default().as_bytes())?;
    let graf = crate::graf::Graf::kur(&b.depo);
    Ok((imza, graf.dugumler.len(), graf.kenarlar.len()))
}
