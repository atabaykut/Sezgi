//! Negatif kesinlik korpusu (tests/kesinlik/): her vaka, Sezgi'nin "kesin" diyemeyeceği bir tuzak içerir.
//! Vakanın `beklenti.json` dosyası yasak iddiaları listeler; Sezgi bunlardan birini kesin damgayla söylerse test kalır.
//!
//! Kontrol türleri (`konu`):
//! - `giris`: kapının giriş durumu `yasak` değerinde ve kanıtı "kesin" olamaz.
//! - `alan`: gövde alanı `alan` için zorunluluk `yasak_zorunlu` değerinde kesin olamaz (`null`: hiçbir değer kesin olamaz).
//! - `sema`: gövde şeması kesin çözüldüyse alanları tam olarak `beklenen_alanlar` olmalı; değilse belirsiz bildirilmeli.
//!   `beklenen_alanlar: null`: şema kesin çözülemez, belirsiz bildirilmeli. `cozulmeli: true`: desteklenen bir kalıp,
//!   belirsiz demek de yetmez, doğru alanlarla çözülmeli (olumlu kontrol).
//! - `onizle`: önizleme sonucu `yasak` ile başlayamaz (ör. eksik zorunlu alanla "GEÇER"); `beklenen` verilirse onunla başlamalı.
//! - `giris_kesin`: olumlu kontrol; tanımı izlenebilen kalıpta giriş `beklenen` değerinde ve kesin olmalı.
//!
//! Her vakada ayrıca genel değişmez sınanır: kartta "certain" damgalı bir işlemin bütün girdileri de "certain" olmalı.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn korpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("kesinlik")
}

/// Vakanın uygulama klasörü (monorepo vakalarında `beklenti.json` içindeki `uygulama` alt yolu) haritalanır
fn haritala(vaka: &Path, uygulama: &Path) -> (PathBuf, Value, Value) {
    let ad = vaka.file_name().unwrap().to_string_lossy().to_string();
    let cikti = std::env::temp_dir().join(format!("sezgi-kesinlik-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let d = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", uygulama.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(d.status.success(), "{ad}: sezgi init başarısız: {}", String::from_utf8_lossy(&d.stderr));
    let oku = |f: &str| -> Value { serde_json::from_str(&std::fs::read_to_string(cikti.join(f)).unwrap()).unwrap() };
    (cikti.clone(), oku("kapilar.json"), oku("benlik.json"))
}

fn onizle(harita: &Path, arguman: &Value) -> String {
    use std::io::Write;
    let mut p =
        Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", harita.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    let istek = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

fn kapi<'a>(harita: &'a Value, id: &str) -> &'a Value {
    harita["endpointler"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("kapı yok: {id}"))
}

/// Bir vakanın bütün yasaklarını sınar; ihlalleri döner
fn vaka_sina(vaka: &Path) -> Vec<String> {
    let ad = vaka.file_name().unwrap().to_string_lossy().to_string();
    let beklenti: Value = serde_json::from_str(&std::fs::read_to_string(vaka.join("beklenti.json")).unwrap()).unwrap();
    let uygulama = vaka.join(beklenti["uygulama"].as_str().unwrap_or("."));
    let (cikti, harita, kart) = haritala(vaka, &uygulama);
    let mut ihlaller = Vec::new();
    for k in beklenti["kontroller"].as_array().unwrap() {
        let id = k["kapi"].as_str().unwrap();
        let e = kapi(&harita, id);
        match k["konu"].as_str().unwrap() {
            "giris" => {
                if e["giris"] == k["yasak"] && e["giris_kaniti"] == "kesin" {
                    ihlaller.push(format!("{ad}: {id} giriş '{}' kesin olarak söylendi", e["giris"]));
                }
            }
            "giris_kesin" => {
                if e["giris"] != k["beklenen"] || e["giris_kaniti"] != "kesin" {
                    ihlaller.push(format!("{ad}: {id} giriş {} kesin bekleniyordu, {} / {} çıktı", k["beklenen"], e["giris"], e["giris_kaniti"]));
                }
            }
            "alan" => {
                let alan = k["alan"].as_str().unwrap();
                let sema = &e["govde"][0]["sema"];
                let a = &sema["alanlar"][alan];
                let kesin = sema["bulunamadi"] == false && a.is_object() && a["belirsiz"] != true;
                let yasak = &k["yasak_zorunlu"];
                if kesin && (yasak.is_null() || a["zorunlu"] == *yasak) {
                    ihlaller.push(format!("{ad}: {id} gövde.{alan} zorunlu={} kesin olarak söylendi", a["zorunlu"]));
                }
                // Kartta da aynı iddia "certain" olamaz
                for op in
                    kart["operations"].as_array().unwrap().iter().filter(|o| format!("{} {}", o["entry"]["method"].as_str().unwrap(), o["entry"]["path"].as_str().unwrap()) == id)
                {
                    let g = &op["inputs"][alan];
                    if g["stamp"] == "certain" && (yasak.is_null() || g["required"] == *yasak) {
                        ihlaller.push(format!("{ad}: kartta {id} {alan} required={} certain", g["required"]));
                    }
                }
            }
            "sema" => {
                let sema = &e["govde"][0]["sema"];
                let mut bulunan: Vec<String> = sema["alanlar"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
                bulunan.sort();
                let kesin = sema["bulunamadi"] == false;
                match k["beklenen_alanlar"].as_array() {
                    None if kesin => ihlaller.push(format!("{ad}: {id} şeması belirsiz olmalıydı, kesin çözüldü: {bulunan:?}")),
                    None => {}
                    Some(b) => {
                        let mut beklenen: Vec<String> = b.iter().map(|x| x.as_str().unwrap().to_string()).collect();
                        beklenen.sort();
                        if kesin && bulunan != beklenen {
                            ihlaller.push(format!("{ad}: {id} şeması kesin ama yanlış: {bulunan:?} (beklenen {beklenen:?})"));
                        }
                        if !kesin && k["cozulmeli"] == true {
                            ihlaller.push(format!("{ad}: {id} desteklenen kalıp çözülemedi (beklenen {beklenen:?})"));
                        }
                    }
                }
            }
            "onizle" => {
                let metin = onizle(&cikti, &k["istek"]);
                if let Some(yasak) = k["yasak"].as_str() {
                    if metin.lines().any(|l| l.starts_with(&format!("sonuç: {yasak}"))) {
                        ihlaller.push(format!("{ad}: önizleme yasak sonucu verdi ({yasak}):\n{metin}"));
                    }
                }
                // Olumlu kontrol: kesin bilinen bir kural ihlali reddedilmeli
                if let Some(beklenen) = k["beklenen"].as_str() {
                    if !metin.lines().any(|l| l.starts_with(&format!("sonuç: {beklenen}"))) {
                        ihlaller.push(format!("{ad}: önizleme '{beklenen}' bekleniyordu:\n{metin}"));
                    }
                }
            }
            x => panic!("{ad}: bilinmeyen konu {x}"),
        }
    }
    ihlaller.extend(degismezler(&ad, &harita, &kart));
    let _ = std::fs::remove_dir_all(&cikti);
    ihlaller
}

/// Her haritada geçerli olması gereken kurallar (SPEC 3: işlemin damgası, dayandığı olguların en zayıfıdır):
/// - "certain" işlemin bütün girdileri "certain" olmalı;
/// - "certain" işlemin giriş kararı kesin olmalı (haritada `giris_kaniti: kesin`);
/// - haritada zorunluluğu belirsiz alan kartta "certain" olamaz.
fn degismezler(ad: &str, harita: &Value, kart: &Value) -> Vec<String> {
    let mut ihlaller = Vec::new();
    let kapilar = harita["endpointler"].as_array().unwrap();
    for op in kart["operations"].as_array().unwrap() {
        let (yontem, yol) = (op["entry"]["method"].as_str().unwrap(), op["entry"]["path"].as_str().unwrap());
        let Some(k) = kapilar.iter().find(|k| k["yontem"] == yontem && k["yol"] == yol && k["ana"] != false) else {
            ihlaller.push(format!("{ad}: kartta {yontem} {yol} var, haritada yok"));
            continue;
        };
        for g in k["govde"].as_array().into_iter().chain(k["sorgu"].as_array()).flatten() {
            for (alan, a) in g["sema"]["alanlar"].as_object().into_iter().flatten() {
                if a["belirsiz"] == true && op["inputs"][alan]["stamp"] == "certain" {
                    ihlaller.push(format!("{ad}: {yontem} {yol} {alan} haritada belirsiz, kartta certain"));
                }
            }
        }
        if op["stamp"] != "certain" {
            continue;
        }
        if k["giris_kaniti"] != "kesin" {
            ihlaller.push(format!("{ad}: işlem {} certain ama giriş kararı {}", op["id"], k["giris_kaniti"]));
        }
        for (g, v) in op["inputs"].as_object().unwrap() {
            if v["stamp"] != "certain" {
                ihlaller.push(format!("{ad}: işlem {} certain ama girdi {g} {}", op["id"], v["stamp"]));
            }
        }
    }
    ihlaller
}

/// Değişmezler örnek uygulamalarda da (tests/ornekler) geçerli olmalı
#[test]
fn ornek_uygulamalarda_degismezler() {
    let ornekler = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler");
    let mut ihlaller = Vec::new();
    // Yalnızca uygulamalar (bağımlılık dosyası olan); istemci örnekleri (ör. istemci-temel) haritalanmaz
    for e in std::fs::read_dir(ornekler).unwrap().flatten().filter(|e| e.path().join("package.json").exists() || e.path().join("pyproject.toml").exists()) {
        let (cikti, harita, kart) = haritala(&e.path(), &e.path());
        ihlaller.extend(degismezler(&e.file_name().to_string_lossy(), &harita, &kart));
        let _ = std::fs::remove_dir_all(&cikti);
    }
    assert!(ihlaller.is_empty(), "{}", ihlaller.join("\n"));
}

#[test]
fn kesinlik_korpusu() {
    let mut vakalar: Vec<PathBuf> = std::fs::read_dir(korpus()).unwrap().flatten().map(|e| e.path()).filter(|p| p.join("beklenti.json").exists()).collect();
    vakalar.sort();
    assert!(!vakalar.is_empty(), "korpus boş");
    let ihlaller: Vec<String> = vakalar.iter().flat_map(|v| vaka_sina(v)).collect();
    assert!(ihlaller.is_empty(), "{} yanlış kesinlik:\n{}", ihlaller.len(), ihlaller.join("\n"));
}
