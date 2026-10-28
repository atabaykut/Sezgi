//! P1 H1: Ayna'nın token ucu yalnızca Origin'e güvenmez, başlangıçta üretilen oturum anahtarını ister (B5); `*` deseni
//! yalnızca açık onayla. `sun` ve `ayna` haritadaki sembolik bağları izlemez (B9).
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Stdio};

struct Sunucu {
    p: Child,
    port: u16,
    anahtar: Option<String>,
}

impl Drop for Sunucu {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
    }
}

/// `sezgi ayna|sun` başlatır; adres satırından port ve (varsa) `#anahtar=` değeri. Süreç Drop'ta beklenir.
#[allow(clippy::zombie_processes)]
fn baslat(p: &Proje, komut: &str) -> Sunucu {
    let mut c = p.komut(&[komut, ".sezgi", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0, "adres yazılmadı");
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let anahtar = k.split("#anahtar=").nth(1).map(|a| a.chars().take_while(|c| c.is_ascii_alphanumeric()).collect());
            return Sunucu { p: c, port, anahtar };
        }
    }
}

fn istek(port: u16, yontem: &str, yol: &str, basliklar: &[(&str, &str)], govde: &str) -> (String, String) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut b = format!("{yontem} {yol} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\n", govde.len());
    for (k, v) in basliklar {
        b.push_str(&format!("{k}: {v}\r\n"));
    }
    b.push_str("\r\n");
    s.write_all(b.as_bytes()).unwrap();
    s.write_all(govde.as_bytes()).unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    let (d, g) = c.split_once("\r\n\r\n").unwrap_or((&c, ""));
    (d.lines().next().unwrap_or("").to_string(), g.to_string())
}

#[test]
fn ayna_token_ucu_oturum_anahtari_ister() {
    let ev = Ev::yeni("ayna-anahtar");
    let p = ev.proje("nestjs-temel", "a");
    let a = baslat(&p, "ayna");
    let anahtar = a.anahtar.clone().expect("Ayna adresi oturum anahtarını taşımalı (#anahtar=)");
    assert!(anahtar.len() >= 32, "{anahtar}");
    let koken = format!("http://127.0.0.1:{}", a.port);
    let govde = serde_json::json!({ "ad": "x", "izinler": ["kitap.listele:oku"] }).to_string();
    let temel = [("Origin", koken.as_str()), ("Content-Type", "application/json")];
    // Origin doğru ama anahtar yok ya da yanlış: tarayıcı dışından sahte Origin'le token basılamaz
    for ek in [vec![], vec![("X-Sezgi-Anahtar", "yanlis")]] {
        let b: Vec<(&str, &str)> = temel.iter().cloned().chain(ek).collect();
        let (d, g) = istek(a.port, "POST", "/api/token", &b, &govde);
        assert!(d.contains("403") && !g.contains("szg_"), "{d} {g}");
    }
    let b: Vec<(&str, &str)> = temel.iter().cloned().chain([("X-Sezgi-Anahtar", anahtar.as_str())]).collect();
    let (d, g) = istek(a.port, "POST", "/api/token", &b, &govde);
    assert!(d.contains("200") && g.contains("szg_"), "{d} {g}");
    // `*` deseni açık onay olmadan reddedilir
    let yildiz = serde_json::json!({ "ad": "y", "izinler": ["*:yaz"] }).to_string();
    let (_, g) = istek(a.port, "POST", "/api/token", &b, &yildiz);
    assert!(g.contains("hata") && !g.contains("szg_"), "{g}");
    let onayli = serde_json::json!({ "ad": "y", "izinler": ["*:yaz"], "tum_islemler": true }).to_string();
    let (_, g) = istek(a.port, "POST", "/api/token", &b, &onayli);
    assert!(g.contains("szg_"), "{g}");
    // İptal de anahtar ister
    let iptal = serde_json::json!({ "ad": "x" }).to_string();
    let (d, _) = istek(a.port, "POST", "/api/token/iptal", &temel, &iptal);
    assert!(d.contains("403"), "{d}");
}

#[test]
fn sun_ve_ayna_harita_baglarini_izlemez() {
    let ev = Ev::yeni("ayna-bag");
    let p = ev.proje("nestjs-temel", "a");
    let gizli = ev.kok.join("gizli.json");
    std::fs::write(&gizli, r#"{"gizli":"GIZLI-DEGER-4242","app":{"id":"x"}}"#).unwrap();
    let harita = p.klasor.join(".sezgi");
    let bagla = |hedef: &std::path::Path, ad: &str| {
        let yol = harita.join(ad);
        let _ = std::fs::remove_file(&yol);
        #[cfg(unix)]
        std::os::unix::fs::symlink(hedef, &yol).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(hedef, &yol).unwrap();
    };
    bagla(&gizli, "openapi.json");
    let s = baslat(&p, "sun");
    let (_, g) = istek(s.port, "GET", "/uygulama/openapi.json", &[], "");
    assert!(!g.contains("GIZLI-DEGER"), "sun bağı izlememeli: {g}");
    drop(s);
    bagla(&gizli, "benlik.json");
    let mut c = p.komut(&["ayna", ".sezgi", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    let mut port = 0u16;
    while r.read_line(&mut satir).unwrap_or(0) > 0 {
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            port = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            break;
        }
        satir.clear();
    }
    if port > 0 {
        let (_, g) = istek(port, "GET", "/api/kart", &[], "");
        assert!(!g.contains("GIZLI-DEGER"), "ayna bağı izlememeli: {g}");
    }
    let _ = c.kill();
    let _ = c.wait();
}
