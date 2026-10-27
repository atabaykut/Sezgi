//! Tarayıcı senaryosu (`sezgi gozlem senaryo <senaryo.json> --cdp <loopback> ...`): insanın tanımladığı bir senaryo
//! (git, tıkla, doldur, bas, bekle) REA'nın `capture-browser-scenario` aracıyla, kullanıcının açtığı tarayıcıda
//! (loopback CDP) çalıştırılır. REA her isteği onu başlatan adıma bağlar (adım ↔ istek kimliği); bu bağ gözlem olur:
//! UI adımı → istek → `api:` zinciri, yolculuk adımları gözlemle `observed` (yalıtım beyanı yoksa inferred).
//!
//! - **Sınıf:** araç hedefi değiştirir ve sayfanın kodunu çalıştırır: dinamik. `--onayla`, `--yalitim konteyner|vm|yok`,
//!   `--calistir`, `--sahibiyim` ve terminalde "evet" ister. Hedef ve CDP adresi yalnızca bu makine (loopback).
//! - **Değer yok:** doldurulan değerler senaryoda yazılmaz; ortam değişkeninden (`{"ortam": "AD"}`) ya da dosyadan
//!   (`{"dosya": "yol"}`) okunur ve REA'ya yalnızca alt sürecin ortamında gizli değer olarak verilir (senaryo
//!   dosyasında, komut satırında ve kanıtta görünmez). İstek ve yanıt gövdeleri, başlık değerleri, çerezler, sorgu
//!   değerleri alınmaz: yalnızca yöntem, yol şablonu, durum kodu ve isteği başlatan adım.
//!
//! Senaryo biçimi:
//! ```json
//! { "baslangic": "http://127.0.0.1:5173/login",
//!   "adimlar": [ { "ad": "eposta", "doldur": { "rol": "textbox", "ad": "Email" }, "deger": { "ortam": "SEZGI_EPOSTA" } },
//!                { "ad": "parola", "doldur": { "test_id": "password-input" }, "deger": { "dosya": "parola.txt" } },
//!                { "ad": "baslik", "doldur": { "css": "input[name=title]" }, "deger": { "ortam": "SEZGI_BASLIK" } },
//!                { "ad": "giris", "tikla": { "rol": "button", "ad": "Log In" } },
//!                { "ad": "bekle", "bekle": { "rol": "button", "ad": "Add Item" } },
//!                { "ad": "dur", "bekle_ms": 500 },
//!                { "ad": "git", "git": "http://127.0.0.1:5173/items" } ] }
//! ```
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Duration;

use crate::gozlem::{Bicim, Gozlemler, Kayit, Kaynak};
use crate::kanit::Dayanak;
use crate::kesif::genis::{loopback_mu, Baglam, Secenekler};
use crate::kesif::rea::Yalitim;

/// REA'nın tanıdığı erişilebilirlik rolleri
const ROLLER: &[&str] = &["button", "checkbox", "combobox", "dialog", "link", "listbox", "menuitem", "option", "radio", "slider", "spinbutton", "switch", "tab", "textbox"];
/// Bir senaryodaki en çok adım
const EN_COK_ADIM: usize = 200;

fn dur(m: &str) -> ! {
    eprintln!("✗ {m}");
    std::process::exit(1)
}

/// Adresin ana makinesi bu makine mi (http/https, kimlik bilgisi yok)
fn yerel_adres(u: &str) -> bool {
    let govde = u.strip_prefix("http://").or_else(|| u.strip_prefix("https://")).unwrap_or("");
    let ana = govde.split(['/', '?', '#']).next().unwrap_or("");
    if ana.contains('@') {
        return false;
    }
    let host = if let Some(r) = ana.strip_prefix('[') { r.split(']').next().unwrap_or("") } else { ana.split(':').next().unwrap_or("") };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// Adresin kökeni ve yolu (sorgu ve parça atılır)
fn koken_yol(u: &str) -> (String, String) {
    let Some(i) = u.find("://") else { return (String::new(), u.to_string()) };
    let s = &u[i + 3..];
    let (ana, yol) = match s.find('/') {
        Some(j) => (&s[..j], &s[j..]),
        None => (s, "/"),
    };
    let yol = yol.split(['?', '#']).next().unwrap_or("/");
    (format!("{}://{}", &u[..i], ana.to_ascii_lowercase()), yol.to_string())
}

/// Adım kimliği REA'nın tanımlayıcı biçiminde
fn kimlik(s: &str, i: usize) -> String {
    let t: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(48).collect();
    if t.is_empty() || !t.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        format!("adim_{i}")
    } else {
        t
    }
}

/// Bulucu: rol + erişilebilir ad (tam eşleşme), test kimliği (`{"test_id": "..."}`; ör. erişilebilir rolü olmayan parola
/// alanı) ya da CSS seçici (`{"css": "input[name=title]"}`)
fn bulucu(v: &Value, adim: &str) -> Result<(Value, String, String), String> {
    if let Some(t) = v["test_id"].as_str().filter(|t| !t.is_empty()) {
        return Ok((json!({ "kind": "test_id", "value": t }), "test_id".into(), crate::guvenli::temiz(t)));
    }
    if let Some(c) = v["css"].as_str().filter(|c| !c.is_empty() && c.len() <= 300) {
        return Ok((json!({ "kind": "css", "selector": c }), "css".into(), crate::guvenli::temiz(c)));
    }
    let rol = v["rol"].as_str().ok_or(tf!("{adim}: bulucu rol ister", "{adim}: the locator needs a rol (role)"))?;
    let ad = v["ad"].as_str().filter(|a| !a.is_empty()).ok_or(tf!("{adim}: bulucu ad (erişilebilir ad) ister", "{adim}: the locator needs an ad (accessible name)"))?;
    if !ROLLER.contains(&rol) {
        return Err(tf!("{adim}: rol {rol} desteklenmiyor ({})", "{adim}: role {rol} is not supported ({})", ROLLER.join(", ")));
    }
    Ok((json!({ "kind": "role", "role": rol, "name": ad }), rol.to_string(), crate::guvenli::temiz(ad)))
}

/// Adımın zaman aşımı (ms): verilmezse 15 sn; bulunamayan öğe senaryoyu sonsuza dek bekletmez
fn zaman_asimi(a: &Value) -> u64 {
    a["sure_ms"].as_u64().unwrap_or(15_000).clamp(100, 120_000)
}

/// Çevrilmiş senaryo: REA girdisi, gizli değerler (alt sürecin ortamına), adım etiketleri
struct Ceviri {
    rea: Value,
    ortam: Vec<(String, String)>,
    /// REA adım kimliği -> (eylem, rol, etiket)
    adimlar: HashMap<String, (String, String, String)>,
    koken: String,
}

fn cevir(s: &Value, klasor: &Path) -> Result<Ceviri, String> {
    let baslangic = s["baslangic"].as_str().ok_or(t!("senaryo `baslangic` adresi ister", "the scenario needs a `baslangic` (start) address"))?;
    if !yerel_adres(baslangic) {
        return Err(tf!(
            "başlangıç adresi bu makinede değil ({baslangic}): senaryo yalnızca loopback hedefte çalışır",
            "the start address is not on this machine ({baslangic}): a scenario runs only against a loopback target"
        ));
    }
    let adimlar = s["adimlar"].as_array().filter(|a| !a.is_empty()).ok_or(t!("senaryo `adimlar` ister", "the scenario needs `adimlar` (steps)"))?;
    if adimlar.len() > EN_COK_ADIM {
        return Err(tf!("en çok {EN_COK_ADIM} adım", "at most {EN_COK_ADIM} steps"));
    }
    let mut eylemler = vec![];
    let mut gizliler = vec![];
    let mut ortam = vec![];
    let mut etiketler = HashMap::new();
    let mut gorulen = std::collections::HashSet::new();
    for (i, a) in adimlar.iter().enumerate() {
        let mut id = kimlik(a["ad"].as_str().unwrap_or(""), i + 1);
        if !gorulen.insert(id.clone()) {
            id = format!("{id}_{}", i + 1);
            gorulen.insert(id.clone());
        }
        let e = if let Some(b) = a.get("tikla") {
            let (l, rol, ad) = bulucu(b, &id)?;
            etiketler.insert(id.clone(), ("click".to_string(), rol, ad));
            json!({ "step_id": id, "action": "click", "locator": l, "timeout_ms": zaman_asimi(a) })
        } else if let Some(b) = a.get("doldur") {
            let (l, rol, ad) = bulucu(b, &id)?;
            // Değer yalnızca ortamdan ya da dosyadan; senaryoda düz değer kabul edilmez
            let d = &a["deger"];
            let deger = if let Some(o) = d["ortam"].as_str() {
                std::env::var(o).map_err(|_| tf!("{id}: ortam değişkeni {o} yok", "{id}: environment variable {o} is not set"))?
            } else if let Some(f) = d["dosya"].as_str() {
                let p = if Path::new(f).is_absolute() { Path::new(f).to_path_buf() } else { klasor.join(f) };
                crate::guvenli::oku(&p).map(|m| m.trim_end_matches(['\n', '\r']).to_string()).ok_or(tf!("{id}: değer dosyası okunamadı", "{id}: could not read the value file"))?
            } else {
                return Err(tf!(
                    "{id}: doldurulan değer senaryoda yazılmaz; {{\"ortam\": \"AD\"}} ya da {{\"dosya\": \"yol\"}} verin",
                    "{id}: a filled value is never written in the scenario; give {{\"ortam\": \"NAME\"}} or {{\"dosya\": \"path\"}}"
                ));
            };
            let gizli = format!("g{}", i + 1);
            let degisken = format!("SEZGI_SENARYO_DEGER_{}", i + 1);
            gizliler.push(json!({ "secret_id": gizli, "environment_variable": degisken }));
            ortam.push((degisken, deger));
            etiketler.insert(id.clone(), ("fill".to_string(), rol, ad));
            json!({ "step_id": id, "action": "fill", "locator": l, "value": { "source": "secret", "secret_id": gizli }, "timeout_ms": zaman_asimi(a) })
        } else if let Some(b) = a.get("bas") {
            let (l, rol, ad) = bulucu(b, &id)?;
            let tus = a["tus"].as_str().unwrap_or("Enter");
            etiketler.insert(id.clone(), ("press".to_string(), rol, ad));
            json!({ "step_id": id, "action": "press", "locator": l, "key": tus, "timeout_ms": zaman_asimi(a) })
        } else if let Some(b) = a.get("bekle") {
            let (l, _, _) = bulucu(b, &id)?;
            json!({ "step_id": id, "action": "wait_for", "locator": l, "state": "visible", "timeout_ms": zaman_asimi(a) })
        } else if let Some(ms) = a["bekle_ms"].as_u64() {
            json!({ "step_id": id, "action": "wait_for_timeout", "duration_ms": ms.clamp(1, 60_000) })
        } else if let Some(u) = a["git"].as_str() {
            if !yerel_adres(u) {
                return Err(tf!("{id}: adres bu makinede değil ({u})", "{id}: the address is not on this machine ({u})"));
            }
            json!({ "step_id": id, "action": "goto", "destination": { "url": u }, "wait_until": "load", "timeout_ms": zaman_asimi(a) })
        } else {
            return Err(tf!("{id}: adım türü yok (tikla, doldur, bas, bekle, bekle_ms, git)", "{id}: no step type (tikla, doldur, bas, bekle, bekle_ms, git)"));
        };
        eylemler.push(e);
    }
    let koken = koken_yol(baslangic).0;
    let rea = json!({
        "start_url": { "url": baslangic },
        "actions": eylemler,
        "secrets": gizliler,
        "capture": { "events": ["network"] },
    });
    Ok(Ceviri { rea, ortam, adimlar: etiketler, koken })
}

// ---- REA çıktısı (yalnızca gereken alanlar) ----

#[derive(Deserialize, Default)]
#[serde(default)]
struct Sonuc {
    steps: Vec<ReaAdim>,
    events: Olaylar,
    completeness: Option<Value>,
}

#[derive(Deserialize)]
struct ReaAdim {
    step_index: u64,
    step_id: Option<String>,
    action: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Olaylar {
    items: Vec<Olay>,
}

#[derive(Deserialize)]
struct Olay {
    kind: Option<String>,
    step_index: Option<u64>,
    transaction_id: Option<String>,
    method: Option<String>,
    url: Option<OlayAdres>,
    status: Option<u16>,
    resource_type: Option<String>,
}

#[derive(Deserialize)]
struct OlayAdres {
    url: Option<String>,
}

fn sure() -> Duration {
    Duration::from_secs(std::env::var("SEZGI_REA_SURE").ok().and_then(|s| s.parse().ok()).unwrap_or(180))
}

/// `sezgi gozlem senaryo <senaryo.json> --cdp <loopback> [--harita <klasör>] --onayla --yalitim … --calistir --sahibiyim`
pub fn komut(args: &[String]) {
    let secenek = |ad: &str| args.windows(2).find(|w| w[0] == ad).map(|w| w[1].clone());
    let var = |ad: &str| args.iter().any(|a| a == ad);
    let Some(dosya) = args.get(2).filter(|a| !a.starts_with("--")) else {
        te!(
            "Kullanım: sezgi gozlem senaryo <senaryo.json> --cdp <http://127.0.0.1:port> [--harita <klasör>] --onayla --yalitim konteyner|vm|yok --calistir --sahibiyim",
            "Usage: sezgi observe scenario <scenario.json> --cdp <http://127.0.0.1:port> [--map <folder>] --onayla --yalitim konteyner|vm|yok --calistir --sahibiyim"
        );
        std::process::exit(2)
    };
    let cdp = secenek("--cdp").unwrap_or_else(|| {
        crate::yardim::kullanim_hatasi(
            "gozlem",
            t!(
                "--cdp <http://127.0.0.1:port> gerekli: senaryo kullanıcının açtığı tarayıcıda çalışır",
                "--cdp <http://127.0.0.1:port> is required: the scenario runs in a browser the user opened"
            ),
        )
    });
    if !loopback_mu(&cdp) {
        dur(&tf!(
            "CDP adresi bu makinede değil ({cdp}): yalnızca http://127.0.0.1:<port>, localhost ya da [::1]",
            "the CDP address is not on this machine ({cdp}): only http://127.0.0.1:<port>, localhost or [::1]"
        ));
    }
    let yalitim = secenek("--yalitim").map(|y| {
        Yalitim::coz(&y)
            .unwrap_or_else(|| crate::yardim::kullanim_hatasi("gozlem", &tf!("--yalitim konteyner, vm ya da yok olmalı: {y}", "--yalitim must be konteyner, vm or yok: {y}")))
    });
    let klasor = crate::guvenli::harita_gerekli(secenek("--harita").as_deref());
    let harita: Value = crate::guvenli::oku_harita(&klasor.join("kapilar.json"))
        .and_then(|m| serde_json::from_str(&m).ok())
        .unwrap_or_else(|| dur(t!("harita okunamadı (önce: sezgi init)", "could not read the map (first: sezgi init)")));
    if harita["ozet"]["kaynaksiz"] == true {
        dur(t!(
            "kaynaksız haritaya senaryo gözlemi bu sürümde desteklenmiyor: kaynak koddan kurulan haritayla kullanın",
            "scenario observation on a map built without source code is not supported in this version: use a map built from source code"
        ));
    }
    let yol = Path::new(dosya.as_str());
    let metin = crate::guvenli::oku(yol).unwrap_or_else(|| dur(&tf!("senaryo okunamadı: {dosya}", "could not read the scenario: {dosya}")));
    let s: Value = serde_json::from_str(&metin).unwrap_or_else(|e| dur(&tf!("senaryo JSON değil: {e}", "the scenario is not JSON: {e}")));
    let c = cevir(&s, yol.parent().unwrap_or(Path::new("."))).unwrap_or_else(|e| dur(&e));
    let mut b = Baglam::yeni(Secenekler { rea: true, onayla: var("--onayla"), yalitim, calistir: var("--calistir"), sahibiyim: var("--sahibiyim"), ..Secenekler::default() });
    let adim_sayisi = c.rea["actions"].as_array().map_or(0, Vec::len);
    let onay = b.onay(
        "capture-browser-scenario",
        &tf!(
            "{adim_sayisi} adımlı tarayıcı senaryosu {} üzerinde (sayfa değişebilir, veri yazılabilir)",
            "{adim_sayisi}-step browser scenario on {} (the page may change, data may be written)",
            c.koken
        ),
    );
    let onay = onay.unwrap_or_else(|e| dur(&tf!("senaryo çalıştırılmadı: {e}", "the scenario was not run: {e}")));
    let kopru = b.kopru().unwrap_or_else(|e| dur(&tf!("REA kapalı: {e}", "REA is off: {e}")));
    // Hedef sekme: başlangıç kökenindeki sayfa
    let y = kopru.calistir_kapili("list-browser-targets", &[&cdp, "--allowed-origins", &c.koken], Some(&onay), sure()).unwrap_or_else(|e| dur(&e));
    let hedefler = crate::kesif::rea_genis::oku::<crate::kesif::rea_genis::Hedefler>(&y, "list_browser_targets").unwrap_or_else(|e| dur(&e));
    let hedef = hedefler
        .normalized_result
        .unwrap_or_default()
        .sec(Some(&c.koken))
        .unwrap_or_else(|| dur(&tf!("tarayıcıda {} kökeninde açık sekme yok", "no open tab at origin {} in the browser", c.koken)));
    let mut girdi = c.rea.clone();
    girdi["browser"] = json!({ "mode": "connect", "cdp_endpoint": cdp, "target_id": hedef });
    let gecici = std::env::temp_dir().join(format!("sezgi-senaryo-{}-{}.json", std::process::id(), crate::simdi_sn()));
    crate::guvenli::atomik_yaz_gizli(&gecici, girdi.to_string().as_bytes()).unwrap_or_else(|e| dur(&e));
    let sonuc = kopru.calistir_kapili_ortamli("capture-browser-scenario", &[gecici.to_str().unwrap_or("")], Some(&onay), sure(), &c.ortam);
    let _ = std::fs::remove_file(&gecici);
    let yol_cikti = sonuc.unwrap_or_else(|e| dur(&e));
    let z = crate::kesif::rea_genis::oku::<Sonuc>(&yol_cikti, "capture_browser_scenario").unwrap_or_else(|e| dur(&e));
    let r = z.normalized_result.unwrap_or_default();
    // İstekler: bu makinedeki kökenlerin (ön yüz ve ayrı porttaki arka uç) API çağrıları (fetch/xhr), statik dosyalar
    // hariç; dış kökenler (analitik, CDN) sayılmaz
    let uclar = crate::gozlem::uclar(&harita);
    let okunan = crate::kanit::okunan_ozetleri(&harita["ozet"]).unwrap_or_default();
    let adim_adi: BTreeMap<u64, (String, String)> = r.steps.iter().map(|a| (a.step_index, (a.step_id.clone().unwrap_or_default(), a.action.clone().unwrap_or_default()))).collect();
    let mut durum: HashMap<String, u16> = HashMap::new();
    for o in r.events.items.iter().filter(|o| o.kind.as_deref() == Some("response")) {
        if let (Some(t), Some(d)) = (&o.transaction_id, o.status) {
            durum.insert(t.clone(), d);
        }
    }
    let mut sayac: BTreeMap<Bicim, u64> = BTreeMap::new();
    let mut baglar = vec![];
    let mut sira: Vec<String> = vec![];
    let (mut istek, mut eslesen) = (0u64, 0u64);
    for o in r.events.items.iter().filter(|o| o.kind.as_deref() == Some("request") && matches!(o.resource_type.as_deref(), Some("fetch" | "xhr"))) {
        let (koken, yol) = koken_yol(o.url.as_ref().and_then(|u| u.url.as_deref()).unwrap_or(""));
        if !yerel_adres(&format!("{koken}/")) || crate::gozlem::statik_mi(&yol) {
            continue;
        }
        istek += 1;
        let yontem = o.method.clone().unwrap_or_default().to_ascii_uppercase();
        let uc = crate::gozlem::eslestir(&uclar, &yontem, &yol);
        let d = o.transaction_id.as_ref().and_then(|t| durum.get(t)).copied().unwrap_or(0);
        let api = uc.map(|u| u.api.clone());
        let b = Bicim {
            yontem: yontem.clone(),
            yol: api.as_deref().and_then(|a| a.split_once(' ').map(|(_, y)| y.to_string())).unwrap_or_else(|| crate::gozlem::sablon(&yol)),
            api: api.clone(),
            durum: d,
            ..Bicim::default()
        };
        *sayac.entry(b.clone()).or_default() += 1;
        if let Some(a) = &api {
            eslesen += 1;
            if d < 400 && sira.last() != Some(a) {
                sira.push(a.clone());
            }
        }
        // REA'nın bağı: isteği başlatan adım; yalnızca etkileşim adımları (tıkla, doldur, bas) UI adımıdır
        let (adim, eylem) = o.step_index.and_then(|i| adim_adi.get(&i)).cloned().unwrap_or_default();
        if let Some((_, rol, etiket)) = c.adimlar.get(&adim).filter(|(e, _, _)| e == &eylem) {
            baglar.push(json!({ "adim": adim, "eylem": eylem, "rol": rol, "etiket": etiket, "api": api, "yontem": yontem, "yol": b.yol, "durum": d }));
        }
    }
    let kimlik = format!("senaryo:{}", crate::kanit::sha256_hex(metin.as_bytes()));
    let dosyalar: HashMap<&str, &str> = uclar.iter().map(|u| (u.api.as_str(), u.dosya.as_str())).collect();
    let kayitlar: Vec<Kayit> = sayac
        .into_iter()
        .map(|(b, adet)| {
            let dayanak =
                b.api.as_deref().and_then(|a| dosyalar.get(a)).and_then(|d| okunan.get(*d).map(|s| Dayanak { yol: d.to_string(), sha256: s.clone() })).into_iter().collect();
            Kayit { kaynak: kimlik.clone(), bicim: b, adet, dayanak }
        })
        .collect();
    let bagli = baglar.iter().filter(|b| b["api"].is_string()).count();
    let kaynak =
        Kaynak { kimlik, tur: "senaryo".into(), zaman: crate::simdi(), istek, eslesen, sira, adim_baglari: baglar, yalitim: Some(onay.yalitim.ad().into()), ..Kaynak::default() };
    let mut g = Gozlemler::oku(&klasor).unwrap_or(Gozlemler { bicim: crate::gozlem::BICIM, ..Gozlemler::default() });
    g.kaynak_koy(kaynak, kayitlar);
    g.yaz(&klasor).unwrap_or_else(|e| dur(&e));
    crate::gozlem::depo_yenile(&klasor).unwrap_or_else(|e| dur(&e));
    let tamam = r.steps.iter().filter(|a| a.status.as_deref() == Some("completed")).count();
    tp!(
        "✓ Senaryo çalıştı: {tamam}/{} adım tamamlandı · {istek} API isteği · {eslesen} tanesi haritadaki uçlara eşleşti",
        "✓ Scenario ran: {tamam}/{} {a} completed · {istek} API {i} · {eslesen} matched {e} in the map",
        r.steps.len();
        a = crate::dil::cogul(r.steps.len(), "step", "steps"),
        i = crate::dil::cogul(istek, "request", "requests"),
        e = crate::dil::cogul(eslesen, "endpoint", "endpoints")
    );
    tp!(
        "  Adım → istek  {bagli} UI adımı bir uca bağlandı (REA: isteği başlatan adım)",
        "  Step → request {bagli} UI steps linked to an endpoint (REA: the step that started the request)"
    );
    tp!(
        "  Kanıt         BROWSER_OBSERVATION ({}) · kanit.json yenilendi",
        "  Evidence      BROWSER_OBSERVATION ({}) · kanit.json refreshed",
        if onay.yalitim == Yalitim::Yok { t!("inferred: yalıtım beyanı yok", "inferred: no isolation declared") } else { "observed" }
    );
    tp!(
        "  Saklanan      yalnızca biçim: yöntem, yol şablonu, durum kodu, adım; doldurulan değer, gövde, başlık, çerez yok",
        "  Stored        shape only: method, path template, status code, step; no filled values, bodies, headers or cookies"
    );
    if r.completeness.as_ref().is_some_and(|c| c["status"] != "complete") {
        tp!(
            "  ! REA kapsamı tam değil ({}): görülmeyen istekler olabilir",
            "  ! REA coverage is not complete ({}): there may be unseen requests",
            r.completeness.as_ref().map(|c| c["status"].to_string()).unwrap_or_default()
        );
    }
}
