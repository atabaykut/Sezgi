//! W3 senaryo ve Electron, sahte REA (betik) ile:
//! - tarayıcı senaryosu (`sezgi gozlem senaryo`): dinamik sınıf; onay, yalıtım, --calistir ve --sahibiyim olmadan,
//!   terminal "evet" olmadan, loopback dışı CDP/başlangıçla ya da senaryoda yazılmış değerle asla çalışmaz; REA'nın
//!   adım ↔ istek bağı UI adımı → `api:` kenarını ve yolculuk adımını observed yapar (yalıtım yok → inferred); doldurulan
//!   değer yalnızca alt sürecin ortamında, hiçbir dosyada yok (bayt araması);
//! - Electron (`kesfet` + app.asar): paket kimliği, IPC zinciri (ön yüz → IPC → ana süreç işleyicisi), ağ çağrısı,
//!   pencere, fuse'lar; REA JavaScript analizi onaysız çalışmaz, onayla aynı IPC kenarlarını destekler; bozuk paket panik
//!   vermez.
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornekler() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler")
}

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            kopyala(&p, &hedef.join(g.file_name()));
        } else {
            std::fs::copy(&p, hedef.join(g.file_name())).unwrap();
        }
    }
}

struct Ortam {
    kok: PathBuf,
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-w3s-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        kopyala(&ornekler().join("w3r_sahte_rea"), &kok.join("rea"));
        let c = kok.join("rea/cevap");
        std::fs::copy(c.join("doctor.motorsuz.json"), c.join("doctor.json")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(kok.join("rea/node"), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::create_dir_all(kok.join("ayar")).unwrap();
        Ortam { kok }
    }

    fn ortam(&self, c: &mut Command) {
        c.env("SEZGI_REA", self.kok.join("rea/paket/node_modules/.bin/rea"))
            .env("SEZGI_NODE", self.kok.join("rea/node"))
            .env("W3R_IZ", self.kok.join("iz.txt"))
            .env("W3R_CEVAP", self.kok.join("rea/cevap"))
            .env("PATH", "/usr/bin:/bin")
            .env("APPDATA", self.kok.join("ayar"))
            .env("XDG_CONFIG_HOME", self.kok.join("ayar"))
            .env("HOME", self.kok.join("ayar"))
            .env("W3S_BASLIK", "GIZLI_BASLIK_DEGERI_42");
    }

    fn calis(&self, args: &[&str]) -> (Option<i32>, String) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args).stdin(Stdio::null());
        self.ortam(&mut c);
        let o = c.output().unwrap();
        (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    }

    fn terminalde(&self, args: &[&str], cevap: &str) -> (Option<i32>, String) {
        let satir = std::iter::once(env!("CARGO_BIN_EXE_sezgi").to_string()).chain(args.iter().map(|a| format!("'{a}'"))).collect::<Vec<_>>().join(" ");
        let mut c = Command::new("script");
        c.args(["-qec", &satir, "/dev/null"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        self.ortam(&mut c);
        let mut p = c.spawn().unwrap();
        let mut g = p.stdin.take().unwrap();
        let _ = g.write_all(cevap.as_bytes());
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = g.write_all(b"\x04");
        let o = p.wait_with_output().unwrap();
        drop(g);
        (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    }

    fn cagrilar(&self) -> Vec<String> {
        std::fs::read_to_string(self.kok.join("iz.txt")).unwrap_or_default().lines().map(String::from).collect()
    }

    fn cagrildi_mi(&self, komut: &str) -> bool {
        self.cagrilar().iter().any(|l| l.split_whitespace().next() == Some(komut))
    }

    fn iz_sil(&self) {
        let _ = std::fs::remove_file(self.kok.join("iz.txt"));
        let _ = std::fs::remove_file(self.kok.join("iz.txt.girdi"));
    }

    /// Kaynak kodlu uygulama (FastAPI + React): harita ve statik yolculuklar
    fn uygulama(&self) -> PathBuf {
        let u = self.kok.join("uygulama");
        kopyala(&ornekler().join("s4_fastapi_react"), &u);
        let (kod, m) = self.calis(&["init", u.join("backend").to_str().unwrap(), "--on-yuz", u.join("frontend").to_str().unwrap()]);
        assert_eq!(kod, Some(0), "{m}");
        u.join("backend/.sezgi")
    }

    fn senaryo(&self, ad: &str, v: Value) -> PathBuf {
        let p = self.kok.join(format!("{ad}.json"));
        std::fs::write(&p, v.to_string()).unwrap();
        p
    }
}

fn senaryo_govdesi() -> Value {
    serde_json::json!({
        "baslangic": "http://127.0.0.1:8000/",
        "adimlar": [
            { "ad": "doldur_baslik", "doldur": { "rol": "textbox", "ad": "Title" }, "deger": { "ortam": "W3S_BASLIK" } },
            { "ad": "kaydet", "tikla": { "rol": "button", "ad": "Save" } },
            { "ad": "bekle", "bekle_ms": 800 }
        ]
    })
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn onerme<'a>(depo: &'a Value, varlik: &str, yuklem: &str, hedef: &str) -> Option<&'a Value> {
    depo["onermeler"].as_array().unwrap().iter().find(|o| o["varlik"].as_str().unwrap_or("").starts_with(varlik) && o["yuklem"] == yuklem && o["hedef"] == hedef)
}

#[test]
fn w3s_senaryo_kapilari() {
    let o = Ortam::yeni("snr-kapi");
    let h = o.uygulama();
    let hs = h.to_str().unwrap();
    let sen = o.senaryo("s", senaryo_govdesi());
    let ss = sen.to_str().unwrap();
    let tam = ["--onayla", "--yalitim", "konteyner", "--calistir", "--sahibiyim"];
    // (bayraklar, terminal cevabı, beklenen sebep)
    let durumlar: [(&[&str], Option<&str>, &str); 4] =
        [(&[], None, "onay ister"), (&["--onayla", "--yalitim", "konteyner"], Some("evet\n"), "--sahibiyim"), (&tam, None, "terminal"), (&tam, Some("hayir\n"), "onaylamadı")];
    for (bayrak, cevap, neden) in durumlar {
        o.iz_sil();
        let mut a = vec!["gozlem", "senaryo", ss, "--cdp", "http://127.0.0.1:9222", "--harita", hs];
        a.extend_from_slice(bayrak);
        let (kod, m) = match cevap {
            Some(c) => o.terminalde(&a, c),
            None => o.calis(&a),
        };
        assert_eq!(kod, Some(1), "{bayrak:?}: {m}");
        assert!(m.contains(neden), "{bayrak:?}: '{neden}' bekleniyordu: {m}");
        assert!(o.cagrilar().is_empty() || !o.cagrildi_mi("capture-browser-scenario"), "{bayrak:?}: {:?}", o.cagrilar());
    }
    // Loopback dışı CDP ya da başlangıç; senaryoda yazılmış değer: hiçbir REA komutu çalışmaz
    o.iz_sil();
    let (kod, m) =
        o.terminalde(&["gozlem", "senaryo", ss, "--cdp", "http://192.0.2.1:9222", "--harita", hs, "--onayla", "--yalitim", "konteyner", "--calistir", "--sahibiyim"], "evet\n");
    assert!(kod == Some(1) && m.contains("bu makinede değil"), "{m}");
    let mut uzak = senaryo_govdesi();
    uzak["baslangic"] = serde_json::json!("http://ornek.test/");
    let su = o.senaryo("uzak", uzak);
    let (kod, m) = o.terminalde(
        &["gozlem", "senaryo", su.to_str().unwrap(), "--cdp", "http://127.0.0.1:9222", "--harita", hs, "--onayla", "--yalitim", "konteyner", "--calistir", "--sahibiyim"],
        "evet\n",
    );
    assert!(kod == Some(1) && m.contains("loopback"), "{m}");
    let mut duz = senaryo_govdesi();
    duz["adimlar"][0]["deger"] = serde_json::json!("acik-deger");
    let sd = o.senaryo("duz", duz);
    let (kod, m) = o.terminalde(
        &["gozlem", "senaryo", sd.to_str().unwrap(), "--cdp", "http://127.0.0.1:9222", "--harita", hs, "--onayla", "--yalitim", "konteyner", "--calistir", "--sahibiyim"],
        "evet\n",
    );
    assert!(kod == Some(1) && m.contains("senaryoda yazılmaz"), "{m}");
    assert!(o.cagrilar().is_empty(), "{:?}", o.cagrilar());
}

#[test]
fn w3s_senaryo_adim_istek_bagi_ve_gizlilik() {
    let o = Ortam::yeni("snr");
    let h = o.uygulama();
    let hs = h.to_str().unwrap();
    // Önce statik: yolculuğun UI adımı inferred
    let once = json_oku(&h.join("kanit.json"));
    let ui = "ui:src/components/Items/AddItem.tsx#button:Save";
    let adim = onerme(&once, "journey:web/items#Save", "STEP", ui).unwrap_or_else(|| panic!("statik yolculuk yok"));
    assert_eq!(adim["hesap"]["guven"], "inferred");
    let sen = o.senaryo("s", senaryo_govdesi());
    let (kod, m) = o.terminalde(
        &["gozlem", "senaryo", sen.to_str().unwrap(), "--cdp", "http://127.0.0.1:9222", "--harita", hs, "--onayla", "--yalitim", "konteyner", "--calistir", "--sahibiyim"],
        "evet\n",
    );
    assert_eq!(kod, Some(0), "{m}");
    assert!(m.contains("dinamik") && m.contains("1 UI adımı bir uca bağlandı"), "{m}");
    assert!(o.cagrildi_mi("list-browser-targets") && o.cagrildi_mi("capture-browser-scenario"), "{:?}", o.cagrilar());
    // Değer yalnızca alt sürecin ortamında: değişken adı geldi, girdi dosyasında değer yok
    assert!(o.cagrilar().iter().any(|l| l == "ortam SEZGI_SENARYO_DEGER_1"), "{:?}", o.cagrilar());
    let girdi = std::fs::read_to_string(o.kok.join("iz.txt.girdi")).unwrap();
    assert!(!girdi.contains("GIZLI_BASLIK_DEGERI_42") && girdi.contains("\"source\":\"secret\""), "{girdi}");
    assert!(!o.cagrilar().iter().any(|l| l.contains("GIZLI_BASLIK")), "komut satırında değer olmamalı");
    // Gözlem: senaryo kaynağı ve adım bağı
    let g = json_oku(&h.join("gozlem.json"));
    let k = g["kaynaklar"].as_array().unwrap().iter().find(|k| k["tur"] == "senaryo").unwrap();
    assert_eq!(k["yalitim"], "konteyner");
    let bag = &k["adim_baglari"][0];
    assert_eq!((bag["adim"].as_str(), bag["etiket"].as_str(), bag["api"].as_str()), (Some("kaydet"), Some("Save"), Some("api:POST /api/v1/items/")));
    // Kanıt: UI → uç ve yolculuk adımı observed; ekran adımı gözlenmedi (inferred)
    let depo = json_oku(&h.join("kanit.json"));
    let tetik = onerme(&depo, ui, "TRIGGERS", "api:POST /api/v1/items/").unwrap_or_else(|| panic!("UI → uç kenarı yok"));
    assert_eq!(tetik["hesap"]["guven"], "observed", "{tetik}");
    let adim = onerme(&depo, "journey:web/items#Save", "STEP", ui).unwrap();
    assert_eq!(adim["hesap"]["guven"], "observed", "{adim}");
    let ekran = onerme(&depo, "journey:web/items#Save", "STEP", "screen:web//items").unwrap();
    assert_eq!(ekran["hesap"]["guven"], "inferred");
    assert!(depo["kanitlar"].as_array().unwrap().iter().any(|k| k["kaynak_turu"] == "BROWSER_OBSERVATION"));
    // Gizlilik: değer hiçbir dosyada yok
    for g in std::fs::read_dir(&h).unwrap().flatten() {
        let b = std::fs::read(g.path()).unwrap_or_default();
        assert!(!b.windows(22).any(|w| w == b"GIZLI_BASLIK_DEGERI_42"), "{}: değer sızdı", g.path().display());
    }
    // Yalıtım yok → inferred
    let (kod, m) = o.terminalde(
        &["gozlem", "senaryo", sen.to_str().unwrap(), "--cdp", "http://127.0.0.1:9222", "--harita", hs, "--onayla", "--yalitim", "yok", "--calistir", "--sahibiyim"],
        "evet\n",
    );
    assert_eq!(kod, Some(0), "{m}");
    let depo = json_oku(&h.join("kanit.json"));
    assert_eq!(onerme(&depo, "journey:web/items#Save", "STEP", ui).unwrap()["hesap"]["guven"], "inferred");
}

fn electron_yapit(o: &Ortam) -> PathBuf {
    let y = o.kok.join("uygulama");
    kopyala(&ornekler().join("w3s_electron/yayin"), &y);
    // Sahte Electron ikilisi: ELF imzası ve fuse teli (sürüm 1, 8 fuse)
    let mut b = b"\x7fELF".to_vec();
    b.extend(vec![0u8; 2048]);
    b.extend_from_slice(b"dL7pKGdnNz796PbbjQWNKmHXBZaB9tsX");
    b.extend_from_slice(&[1, 8]);
    b.extend_from_slice(b"01r10101");
    b.extend(vec![0u8; 64]);
    std::fs::write(y.join("otel-masaustu"), b).unwrap();
    y
}

#[test]
fn w3s_electron_ipc_zinciri_ve_fuse() {
    let o = Ortam::yeni("elektron");
    let y = electron_yapit(&o);
    let (kod, m) = o.calis(&["kesfet", y.to_str().unwrap()]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(m.contains("3 IPC kanalı") && m.contains("1 ağ çağrısı"), "{m}");
    let c = y.join(".sezgi");
    let graf = json_oku(&c.join("graf.json"));
    let kenar =
        |k: &str, il: &str, h: &str| graf["kenarlar"].as_array().unwrap().iter().find(|x| x["kaynak"] == k && x["iliski"] == il && x["hedef"] == h).map(|x| x["guven"].clone());
    let a = "resources/app.asar";
    assert_eq!(kenar(&format!("client:{a}/renderer/renderer.js:3"), "CALLS", "ipc:rezervasyon:iptal"), Some(serde_json::json!("inferred")), "ön yüz → IPC");
    assert_eq!(kenar("ipc:rezervasyon:iptal", "CALLS", &format!("fn:{a}/main.js#ipc:rezervasyon:iptal")), Some(serde_json::json!("inferred")), "IPC → ana süreç");
    assert_eq!(kenar(&format!("client:{a}/renderer/renderer.js:9"), "CALLS", "api:POST /api/v1/rezervasyonlar/esitle"), Some(serde_json::json!("inferred")), "ağ çağrısı");
    let b = json_oku(&c.join("kaynaksiz.json"));
    let paket = b["kanitlar"].as_array().unwrap().iter().find(|k| k["konum"][0] == format!("{a}#paket")).unwrap();
    assert_eq!((paket["kaynak_turu"].as_str(), paket["beyan"].as_str(), paket["ozet"]["ad"].as_str()), (Some("BINARY_METADATA"), Some("observed"), Some("otel-masaustu")));
    let ipc = &b["kanitlar"].as_array().unwrap().iter().find(|k| k["konum"][0] == format!("{a}#ipc")).unwrap()["ozet"];
    assert!(ipc["web_tercihleri"].as_array().unwrap().iter().any(|t| t["ad"] == "contextIsolation" && t["deger"] == true), "{ipc}");
    assert!(ipc["kopruler"][0]["ad"] == "otel");
    let fuse = b["kanitlar"].as_array().unwrap().iter().find(|k| k["konum"][0] == "otel-masaustu#fuses").unwrap_or_else(|| panic!("{}", b["kanitlar"]));
    assert_eq!(fuse["beyan"], "observed");
    assert_eq!(fuse["ozet"]["fuselar"][0], serde_json::json!({ "ad": "RunAsNode", "durum": "kapali" }));
    assert_eq!(fuse["ozet"]["fuselar"][2]["durum"], "kaldirildi");
    let kart = json_oku(&c.join("benlik.json"));
    assert_eq!(kart["app"]["name"], "Otel Masaüstü");
    assert!(kart["x-screens"].as_array().unwrap().iter().any(|e| e["kind"] == "window"), "{}", kart["x-screens"]);
    assert_eq!(kart["operations"], serde_json::json!([]));
}

#[test]
fn w3s_electron_rea_onayli() {
    let o = Ortam::yeni("elektron-rea");
    let y = electron_yapit(&o);
    let (kod, m) = o.calis(&["kesfet", y.to_str().unwrap(), "--rea"]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(!o.cagrildi_mi("analyze-javascript-application"), "onaysız çalıştı: {:?}", o.cagrilar());
    assert!(json_oku(&y.join(".sezgi/kaynaksiz.json"))["bilinmeyenler"].to_string().contains("onay ister"));
    let (kod, m) = o.terminalde(&["kesfet", y.to_str().unwrap(), "--rea", "--onayla", "--yalitim", "konteyner"], "evet\n");
    assert_eq!(kod, Some(0), "{m}");
    assert!(o.cagrildi_mi("analyze-javascript-application"), "{:?}", o.cagrilar());
    let b = json_oku(&y.join(".sezgi/kaynaksiz.json"));
    let r = b["kanitlar"].as_array().unwrap().iter().find(|k| k["konum"][0].as_str().unwrap_or("").ends_with("#analyze_javascript_application")).unwrap();
    assert_eq!((r["kaynak_turu"].as_str(), r["beyan"].as_str()), (Some("BINARY_HEURISTIC"), Some("inferred")));
    assert!(r["ozet"]["eslesmeler"].as_array().unwrap().iter().any(|e| e["kanal"] == "rezervasyon:iptal" && e["isleyici_dosyasi"] == "main.js"));
    // Aynı IPC → işleyici kenarı iki kaynakla (desen ve REA) desteklenir
    let depo = json_oku(&y.join(".sezgi/kanit.json"));
    let on = onerme(&depo, "ipc:rezervasyon:iptal", "CALLS", "fn:resources/app.asar/main.js#ipc:rezervasyon:iptal").unwrap();
    assert_eq!(on["hesap"]["destek"].as_array().unwrap().len(), 2, "{on}");
}

/// Bozuk ve kasıtlı kötü ASAR: panik yok
#[test]
fn w3s_bozuk_asar_panik_yok() {
    let o = Ortam::yeni("asar-bozuk");
    let asar = std::fs::read(ornekler().join("w3s_electron/yayin/resources/app.asar")).unwrap();
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    for i in 0..60 {
        let y = o.kok.join(format!("k{i}/resources"));
        std::fs::create_dir_all(&y).unwrap();
        let mut v = asar.clone();
        match i % 4 {
            0 => v.truncate((i * 37) % v.len()),
            1 => v[4..8].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => {
                for _ in 0..1 + i % 20 {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    let k = (x as usize) % v.len();
                    v[k] = (x >> 8) as u8;
                }
            }
        }
        std::fs::write(y.join("app.asar"), v).unwrap();
        let (kod, m) = o.calis(&["kesfet", o.kok.join(format!("k{i}")).to_str().unwrap()]);
        assert!(matches!(kod, Some(0) | Some(1)) && !m.contains("panicked"), "{i}: {m}");
    }
    // Üst klasöre çıkan yol içeren başlık: dosya dışarı yazılmaz
    let kotu = br#"{"files":{"..":{"files":{"kacak.js":{"size":4,"offset":"0"}}},"a.js":{"size":4,"offset":"0"}}}"#;
    let mut v = vec![];
    let hs = (kotu.len() + 8) as u32;
    v.extend_from_slice(&4u32.to_le_bytes());
    v.extend_from_slice(&hs.to_le_bytes());
    v.extend_from_slice(&(kotu.len() as u32 + 4).to_le_bytes());
    v.extend_from_slice(&(kotu.len() as u32).to_le_bytes());
    v.extend_from_slice(kotu);
    v.extend_from_slice(b"x=1;");
    let y = o.kok.join("kacak/resources");
    std::fs::create_dir_all(&y).unwrap();
    std::fs::write(y.join("app.asar"), v).unwrap();
    let (kod, m) = o.calis(&["kesfet", o.kok.join("kacak").to_str().unwrap()]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(!std::env::temp_dir().join("kacak.js").exists());
}
