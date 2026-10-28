//! W3 REA geniş, gerçek REA 6.7.0 ile (Node 24.15+). Varsayılan çalıştırmada atlanır (#[ignore]); test görüntüsüne
//! `/rea` (npm i rea-agents@6.7.0) ve `/n/node` bağlanınca: `cargo test --test w3r_rea -- --ignored`.
//! - WebMCP: kendi yazdığımız yerel sayfa (127.0.0.1) ve görüntüdeki başsız Chromium (WebMCP deneme özelliği açık);
//!   dış siteye istek yok. Onay terminalde (sözde terminal) "evet".
//! - Native: kurulu motor yoksa dürüst "motor kurulu değil" (analiz çalışmaz).
//! - Süreç gözlemi: kendi konteynerimizde /bin/true.
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
    let k = std::env::temp_dir().join(format!("sezgi-w3r-rea-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    std::fs::create_dir_all(&k).unwrap();
    k
}

/// Terminalde (sözde terminal) çalıştırır, `cevap` terminale yazılır
fn terminalde(args: &[&str], cevap: &str, ayar: &Path) -> String {
    let satir = std::iter::once(env!("CARGO_BIN_EXE_sezgi").to_string()).chain(args.iter().map(|a| format!("'{a}'"))).collect::<Vec<_>>().join(" ");
    let mut p = Command::new("script")
        .args(["-qec", &satir, "/dev/null"])
        .env("SEZGI_REA", REA)
        .env("SEZGI_NODE", NODE)
        .env("APPDATA", ayar)
        .env("XDG_CONFIG_HOME", ayar)
        .env("HOME", ayar)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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

fn bekle_port(port: u16) {
    let bas = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(bas.elapsed() < Duration::from_secs(20), "port {port} açılmadı");
        std::thread::sleep(Duration::from_millis(100));
    }
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

fn belge(c: &Path, ad: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(c.join(ad)).unwrap()).unwrap()
}

#[test]
#[ignore]
fn w3r_rea_webmcp_yerel_sayfa() {
    if !hazir() {
        return;
    }
    let k = gecici("webmcp");
    let sayfa = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler/w3r_sayfa");
    let (sp, cp) = (18573u16, 19322u16);
    let sunucu = Command::new("python3")
        .args(["-m", "http.server", &sp.to_string(), "--bind", "127.0.0.1"])
        .current_dir(&sayfa)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    bekle_port(sp);
    let tarayici = Command::new("chromium")
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            &format!("--user-data-dir={}", k.join("profil").display()),
            &format!("--remote-debugging-port={cp}"),
            "--enable-features=WebMCPTesting",
            &format!("http://127.0.0.1:{sp}/"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _o = Oldur(vec![sunucu, tarayici]);
    bekle_port(cp);
    std::thread::sleep(Duration::from_secs(2));
    let c = k.join("cikti");
    let koken = format!("http://127.0.0.1:{sp}");
    let m = terminalde(
        &["kesfet", "--webmcp", &format!("http://127.0.0.1:{cp}"), "--kaynak", &koken, "--cikti", c.to_str().unwrap(), "--onayla", "--yalitim", "konteyner"],
        "evet\n",
        &k,
    );
    assert!(m.contains("WebMCP") && m.contains("onaylı"), "{m}");
    let b = belge(&c, "kaynaksiz.json");
    let kn = b["kanitlar"].as_array().unwrap().iter().find(|x| x["konum"][0].as_str().unwrap_or("").ends_with("#discover_webmcp_tools")).cloned();
    let kn = kn.unwrap_or_else(|| panic!("{m}\n{}", b["bilinmeyenler"]));
    assert_eq!((kn["kaynak_turu"].as_str(), kn["beyan"].as_str(), kn["yalitim"].as_str()), (Some("BROWSER_OBSERVATION"), Some("observed"), Some("konteyner")));
    assert_eq!(kn["ozet"]["durum"], "available", "{kn}");
    let arac = kn["ozet"]["araclar"].as_array().unwrap().iter().find(|a| a["ad"] == "rezervasyon_ara").cloned().unwrap_or_else(|| panic!("{kn}"));
    assert_eq!(arac["tur"], "declarative");
    assert_eq!(arac["girdi"][0]["ad"], "tarih");
    let kart = belge(&c, "benlik.json");
    assert_eq!(kart["x-webmcp"][0]["origin"], koken.as_str());
    assert_eq!(kart["x-webmcp"][0]["tools"][0]["requires_approval"], true);
    for d in ["kaynaksiz.json", "benlik.json", "kanit.json"] {
        assert!(!std::fs::read_to_string(c.join(d)).unwrap().contains("Tarihe gore oda arar"), "{d}: sayfa açıklaması saklanmamalı");
    }
}

#[test]
#[ignore]
fn w3r_rea_native_motor_yok_ve_surec() {
    if !hazir() {
        return;
    }
    let k = gecici("native");
    let y = k.join("yapit");
    std::fs::create_dir_all(&y).unwrap();
    std::fs::copy("/bin/true", y.join("hedef")).unwrap();
    let c = k.join("cikti");
    // Native: motor (Ghidra/IDA/Hopper) yoksa analiz çalışmaz, dürüstçe söylenir
    terminalde(&["kesfet", y.to_str().unwrap(), "--cikti", c.to_str().unwrap(), "--rea", "--onayla", "--yalitim", "konteyner"], "evet\n", &k);
    let b = belge(&c, "kaynaksiz.json");
    assert!(b["bilinmeyenler"].to_string().contains("motor kurulu değil"), "{}", b["bilinmeyenler"]);
    assert!(!b["kanitlar"].to_string().contains("#inspect"));
    let m = terminalde(&["kesfet", "--yetenekler", "--json"], "", &k);
    let j = m.find('{').map(|i| &m[i..]).unwrap_or("{}");
    let v: Value = serde_json::from_str(j.trim()).unwrap_or_else(|_| panic!("{m}"));
    assert_eq!(v["rea"]["durum"], "hazir");
    assert!(v["motorlar"].as_array().unwrap().iter().any(|m| m["ad"] == "ghidra" && m["kurulu"] == false), "{}", v["motorlar"]);
    // Süreç gözlemi: tek ikili, bütün şartlar ve onay; kendi konteynerimizde
    let m =
        terminalde(&["kesfet", y.join("hedef").to_str().unwrap(), "--cikti", c.to_str().unwrap(), "--calistir", "--onayla", "--yalitim", "konteyner", "--sahibiyim"], "evet\n", &k);
    let b = belge(&c, "kaynaksiz.json");
    let kn = b["kanitlar"].as_array().unwrap().iter().find(|x| x["konum"][0].as_str().unwrap_or("").ends_with("#capture_process")).cloned();
    let kn = kn.unwrap_or_else(|| panic!("{m}\n{}", b["bilinmeyenler"]));
    assert_eq!((kn["kaynak_turu"].as_str(), kn["beyan"].as_str()), (Some("RUNTIME_TRACE"), Some("observed")));
    assert_eq!(kn["ozet"]["cikis"]["kod"], 0);
}
