//! W3 senaryo ve Electron, gerçek REA 6.7.0 ile. Varsayılan çalıştırmada atlanır (#[ignore]); görüntüye `/rea` ve `/n/node`
//! bağlanınca: `cargo test --test w3s_rea -- --ignored --test-threads=1`.
//! - Electron: kendi Electron uygulamamızın app.asar'ı; REA `analyze-javascript-application` (onaylı).
//! - Senaryo: fastapi-template, uygulamanın kendi konteynerinde ayakta (127.0.0.1:8000 arka uç, 127.0.0.1:5173 ön yüz;
//!   test aynı ağ ad alanında: `--network container:<uygulama>`) ve kaynağı `/depo` altında. "Giriş → Kaydet"
//!   senaryosu; dış siteye istek yok. Uygulama yoksa test atlanır.
use serde_json::Value;
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const REA: &str = "/rea/node_modules/.bin/rea";
const NODE: &str = "/n/node";

fn hazir() -> bool {
    let v = Path::new(REA).exists() && Path::new(NODE).exists();
    if !v {
        eprintln!("REA ya da Node bağlı değil: atlandı");
    }
    v
}

fn gecici(ad: &str) -> PathBuf {
    let k = std::env::temp_dir().join(format!("sezgi-w3s-rea-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    std::fs::create_dir_all(&k).unwrap();
    k
}

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        let p = g.path();
        let ad = g.file_name();
        if ad == "node_modules" || ad == ".git" || ad == ".sezgi" {
            continue;
        }
        if p.is_dir() {
            kopyala(&p, &hedef.join(&ad));
        } else {
            let _ = std::fs::copy(&p, hedef.join(&ad));
        }
    }
}

fn terminalde(args: &[&str], cevap: &str, ayar: &Path, ortam: &[(&str, &str)]) -> String {
    let satir = std::iter::once(env!("CARGO_BIN_EXE_sezgi").to_string()).chain(args.iter().map(|a| format!("'{a}'"))).collect::<Vec<_>>().join(" ");
    let mut c = Command::new("script");
    c.args(["-qec", &satir, "/dev/null"])
        .env("SEZGI_REA", REA)
        .env("SEZGI_NODE", NODE)
        .env("APPDATA", ayar)
        .env("XDG_CONFIG_HOME", ayar)
        .env("HOME", ayar)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (a, d) in ortam {
        c.env(a, d);
    }
    let mut p = c.spawn().unwrap();
    let mut g = p.stdin.take().unwrap();
    let _ = g.write_all(cevap.as_bytes());
    std::thread::sleep(Duration::from_millis(300));
    let _ = g.write_all(b"\x04");
    let o = p.wait_with_output().unwrap();
    drop(g);
    let m = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{args:?}: {m}");
    m
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

struct Oldur(Vec<Child>);
impl Drop for Oldur {
    fn drop(&mut self) {
        for c in &mut self.0 {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

fn acik(port: u16) -> bool {
    TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_millis(500)).is_ok()
}

#[test]
#[ignore]
fn w3s_rea_electron_uygulamasi() {
    if !hazir() {
        return;
    }
    let k = gecici("electron");
    let y = k.join("uygulama");
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler/w3s_electron/yayin"), &y);
    let m = terminalde(&["kesfet", y.to_str().unwrap(), "--rea", "--onayla", "--yalitim", "konteyner"], "evet\n", &k, &[]);
    assert!(m.contains("onaylı") && m.contains("3 IPC kanalı"), "{m}");
    let b = json_oku(&y.join(".sezgi/kaynaksiz.json"));
    let r = b["kanitlar"].as_array().unwrap().iter().find(|x| x["konum"][0].as_str().unwrap_or("").ends_with("#analyze_javascript_application")).cloned();
    let r = r.unwrap_or_else(|| panic!("{m}\n{}", b["bilinmeyenler"]));
    assert_eq!(r["beyan"], "inferred");
    let es = r["ozet"]["eslesmeler"].as_array().unwrap();
    for kanal in ["rezervasyon:iptal", "rezervasyon:liste", "gunluk:yaz"] {
        assert!(es.iter().any(|e| e["kanal"] == kanal && e["isleyici_dosyasi"] == "main.js"), "{kanal}: {es:?}");
    }
    assert!(r["ozet"]["kopruler"][0]["ad"] == "otel");
    let depo = json_oku(&y.join(".sezgi/kanit.json"));
    let on = depo["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["varlik"] == "ipc:rezervasyon:iptal" && o["yuklem"] == "CALLS" && o["hedef"] == "fn:resources/app.asar/main.js#ipc:rezervasyon:iptal")
        .unwrap();
    assert_eq!(on["hesap"]["destek"].as_array().unwrap().len(), 2, "desen ve REA aynı kenarı destekler: {on}");
}

#[test]
#[ignore]
fn w3s_rea_fastapi_giris_kaydet_senaryosu() {
    if !hazir() {
        return;
    }
    if !(acik(8000) && acik(5173) && Path::new("/depo/backend").exists()) {
        eprintln!("fastapi-template ayakta değil (127.0.0.1:8000/5173, /depo): atlandı");
        return;
    }
    let k = gecici("fastapi");
    let u = k.join("uygulama");
    kopyala(Path::new("/depo"), &u);
    let o = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", u.join("backend").to_str().unwrap(), "--on-yuz", u.join("frontend").to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let h = u.join("backend/.sezgi");
    let cdp = 19433u16;
    let tarayici = Command::new("chromium")
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            &format!("--user-data-dir={}", k.join("profil").display()),
            &format!("--remote-debugging-port={cdp}"),
            "http://127.0.0.1:5173/login",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _o = Oldur(vec![tarayici]);
    let bas = Instant::now();
    while !acik(cdp) {
        assert!(bas.elapsed() < Duration::from_secs(20), "CDP açılmadı");
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_secs(3));
    std::fs::write(k.join("parola.txt"), "yalnizca-test-parolasi\n").unwrap();
    let senaryo = serde_json::json!({
        "baslangic": "http://127.0.0.1:5173/login",
        "adimlar": [
            { "ad": "eposta", "doldur": { "test_id": "email-input" }, "deger": { "ortam": "W3S_EPOSTA" } },
            { "ad": "parola", "doldur": { "test_id": "password-input" }, "deger": { "dosya": k.join("parola.txt").to_str().unwrap() } },
            { "ad": "giris", "tikla": { "rol": "button", "ad": "Log In" } },
            { "ad": "panel", "bekle_ms": 2000 },
            { "ad": "ogeler", "git": "http://127.0.0.1:5173/items" },
            { "ad": "ekle_dugmesi", "bekle": { "rol": "button", "ad": "Add Item" } },
            { "ad": "ekle", "tikla": { "rol": "button", "ad": "Add Item" } },
            { "ad": "baslik", "doldur": { "css": "input[name=title]" }, "deger": { "ortam": "W3S_BASLIK" } },
            { "ad": "kaydet", "tikla": { "rol": "button", "ad": "Save" } },
            { "ad": "bekle", "bekle_ms": 2000 }
        ]
    });
    let sp = k.join("senaryo.json");
    std::fs::write(&sp, senaryo.to_string()).unwrap();
    let m = terminalde(
        &[
            "gozlem",
            "senaryo",
            sp.to_str().unwrap(),
            "--cdp",
            &format!("http://127.0.0.1:{cdp}"),
            "--harita",
            h.to_str().unwrap(),
            "--onayla",
            "--yalitim",
            "konteyner",
            "--calistir",
            "--sahibiyim",
        ],
        "evet\n",
        &k,
        &[("W3S_EPOSTA", "admin@example.com"), ("W3S_BASLIK", "Sezgi senaryo ogesi 7f3a")],
    );
    let g = json_oku(&h.join("gozlem.json"));
    let kaynak = g["kaynaklar"].as_array().unwrap().iter().find(|x| x["tur"] == "senaryo").cloned().unwrap_or_else(|| panic!("{m}"));
    let bag = |adim: &str| kaynak["adim_baglari"].as_array().unwrap().iter().find(|b| b["adim"] == adim).cloned();
    let giris = bag("giris").unwrap_or_else(|| panic!("{m}\n{kaynak}"));
    assert!(giris["api"].as_str().unwrap_or("").contains("/login/access-token"), "{giris}");
    let kaydet = bag("kaydet").unwrap_or_else(|| panic!("{m}\n{kaynak}"));
    assert_eq!(kaydet["api"], "api:POST /api/v1/items/", "{kaydet}");
    assert!(kaydet["durum"].as_u64().is_some_and(|d| d < 300), "{kaydet}");
    // Yolculuk: giriş ve Kaydet UI adımları gözlemle observed; yolculuğun kendisi istek sırasıyla observed
    let depo = json_oku(&h.join("kanit.json"));
    let yolculuk = depo["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["varlik"].as_str().unwrap_or("").starts_with("journey:web/items#Save") && o["yuklem"] == "VAR")
        .cloned()
        .unwrap_or_else(|| panic!("Save yolculuğu yok"));
    assert_eq!(yolculuk["hesap"]["guven"], "observed", "{yolculuk}");
    let adimlar: Vec<&Value> = depo["onermeler"].as_array().unwrap().iter().filter(|o| o["varlik"] == yolculuk["varlik"] && o["yuklem"] == "STEP").collect();
    let ui = |parca: &str| adimlar.iter().find(|o| o["hedef"].as_str().unwrap_or("").contains(parca)).map(|o| o["hesap"]["guven"].clone());
    assert_eq!(ui("#button:Save"), Some(serde_json::json!("observed")), "{adimlar:?}");
    assert_eq!(ui("#button:Log In"), Some(serde_json::json!("observed")), "{adimlar:?}");
    assert_eq!(ui("screen:web//items"), Some(serde_json::json!("inferred")), "ekran adımı gözlenmedi");
    // Gizlilik: e-posta, parola ve doldurulan başlık hiçbir çıktıda yok
    for g in std::fs::read_dir(&h).unwrap().flatten() {
        let b = std::fs::read(g.path()).unwrap_or_default();
        for gizli in [&b"yalnizca-test-parolasi"[..], b"admin@example.com", b"Sezgi senaryo ogesi 7f3a"] {
            assert!(!b.windows(gizli.len()).any(|w| w == gizli), "{}: {} sızdı", g.path().display(), String::from_utf8_lossy(gizli));
        }
    }
}
