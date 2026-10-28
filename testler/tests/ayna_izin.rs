//! Ayna izin ekranı: işlem listesi, token üretme ve iptal. Token yalnızca Ayna'nın kendi sayfasından üretilir
//! (Origin zorunlu ve yerel, JSON zorunlu); üretilen token MCP'de yalnızca seçilen işlemleri gösterir.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

struct Ortam {
    kok: PathBuf,
    ayna: Option<Child>,
    port: u16,
    anahtar: String,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        if let Some(mut p) = self.ayna.take() {
            let _ = p.kill();
            let _ = p.wait();
        }
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

fn ortamli(c: &mut Command, ev: &Path) {
    c.env("HOME", ev).env("USERPROFILE", ev).env("APPDATA", ev.join("ayar")).env("XDG_CONFIG_HOME", ev.join("ayar")).env_remove("SEZGI_TOKEN");
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-ayna-izin-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("ev").join("ayar")).unwrap();
        let ornek = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel");
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(["init", ornek.to_str().unwrap(), "--cikti", kok.join("harita").to_str().unwrap()]);
        ortamli(&mut c, &kok.join("ev"));
        assert!(c.output().unwrap().status.success());
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(["ayna", kok.join("harita").to_str().unwrap(), "--port", "0"]).stdout(Stdio::piped());
        ortamli(&mut c, &kok.join("ev"));
        let mut p = c.spawn().unwrap();
        let mut r = BufReader::new(p.stdout.take().unwrap());
        let (mut port, mut anahtar) = (0, String::new());
        let mut satir = String::new();
        while r.read_line(&mut satir).unwrap() > 0 {
            if let Some(i) = satir.find("http://127.0.0.1:") {
                port = satir[i + 17..].split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap();
                anahtar = satir.split("#anahtar=").nth(1).unwrap_or("").split_whitespace().next().unwrap_or("").to_string();
                break;
            }
            satir.clear();
        }
        assert!(port > 0, "Ayna adres yazmadı");
        Ortam { kok, ayna: Some(p), port, anahtar }
    }

    /// Ham HTTP isteği; (durum satırı, gövde)
    fn istek(&self, yontem: &str, yol: &str, basliklar: &[(&str, String)], govde: &str) -> (String, String) {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
        let mut b = format!("{yontem} {yol} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: {}\r\n", self.port, govde.len());
        for (a, v) in basliklar {
            b.push_str(&format!("{a}: {v}\r\n"));
        }
        write!(s, "{b}\r\n{govde}").unwrap();
        let mut c = String::new();
        let _ = s.read_to_string(&mut c);
        let (bas, govde) = c.split_once("\r\n\r\n").unwrap_or((&c, ""));
        (bas.lines().next().unwrap_or("").to_string(), govde.to_string())
    }

    fn sayfadan(&self, yol: &str, govde: &Value) -> Value {
        let (d, g) = self.istek(
            "POST",
            yol,
            &[("Origin", format!("http://127.0.0.1:{}", self.port)), ("Content-Type", "application/json".into()), ("X-Sezgi-Anahtar", self.anahtar.clone())],
            &govde.to_string(),
        );
        assert!(d.contains("200"), "{d} {g}");
        serde_json::from_str(&g).unwrap()
    }

    fn izinler(&self) -> Value {
        serde_json::from_str(&self.istek("GET", "/api/izinler", &[("X-Sezgi-Anahtar", self.anahtar.clone())], "").1).unwrap()
    }

    fn mcp_liste(&self, token: &str) -> String {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(["mcp", self.kok.join("harita").to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped());
        ortamli(&mut c, &self.kok.join("ev"));
        c.env("SEZGI_TOKEN", token);
        let mut p = c.spawn().unwrap();
        writeln!(p.stdin.take().unwrap(), "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).unwrap();
        String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string()
    }

    fn mcp_ara(&self, token: &str, sorgu: &str) -> String {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(["mcp", self.kok.join("harita").to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped());
        ortamli(&mut c, &self.kok.join("ev"));
        c.env("SEZGI_TOKEN", token);
        let mut p = c.spawn().unwrap();
        let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "ara", "arguments": { "sorgu": sorgu, "adet": 50 } } });
        writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
        let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
        let v: Value = serde_json::from_str(c.lines().next().unwrap()).unwrap();
        v["result"]["content"][0]["text"].as_str().unwrap().to_string()
    }
}

#[test]
fn izin_ekrani_islemleri_ne_ise_yaradiklariyla_listeler() {
    let o = Ortam::yeni("liste");
    let v = o.izinler();
    let islemler = v["islemler"].as_array().unwrap();
    let bul = |yontem: &str, yol: &str| islemler.iter().find(|i| i["yontem"] == yontem && i["yol"] == yol).unwrap_or_else(|| panic!("{yontem} {yol} yok: {v}")).clone();
    let liste = bul("GET", "/api/v1/kitaplar");
    assert_eq!((liste["id"].as_str(), liste["risk"].as_str()), (Some("kitap.listele"), Some("oku")));
    assert_eq!(bul("POST", "/api/v1/kitaplar")["risk"], "yaz");
    assert_eq!(bul("DELETE", "/api/v1/kitaplar/:id")["risk"], "yikici");
    // Her işlemin bir açıklaması var: özet kesin, özet yoksa addan okunur ve yorum sayılır
    for i in islemler {
        assert!(!i["aciklama"].as_str().unwrap_or("").is_empty(), "açıklama yok: {i}");
    }
    assert_eq!(v["mod"], "gozlem");
    assert_eq!(v["tokenler"], json!([]));
}

#[test]
fn token_yalnizca_ayna_sayfasindan_uretilir() {
    let o = Ortam::yeni("koken");
    let govde = json!({ "ad": "x", "izinler": ["kitap.listele:oku"], "sure": 7, "onay": "yikici" }).to_string();
    let json_ = ("Content-Type", "application/json".to_string());
    let yerel = ("Origin", format!("http://127.0.0.1:{}", o.port));
    // Origin yok (tarayıcı dışı), başka köken, JSON olmayan gövde (basit form isteği): hepsi reddedilir
    for basliklar in [vec![json_.clone()], vec![("Origin", "http://kotu.example".into()), json_.clone()], vec![yerel.clone(), ("Content-Type", "text/plain".into())]] {
        let (d, g) = o.istek("POST", "/api/token", &basliklar, &govde);
        assert!(d.contains("403"), "{basliklar:?}: {d} {g}");
        assert!(!g.contains("szg_"));
    }
    assert_eq!(o.izinler()["tokenler"], json!([]), "reddedilen istek token üretmemeli");
    // Varsayılan kapalı: izinsiz token yok; geçersiz desen reddedilir
    assert!(o.sayfadan("/api/token", &json!({ "ad": "bos", "izinler": [] }))["hata"].is_string());
    assert!(o.sayfadan("/api/token", &json!({ "ad": "kotu", "izinler": ["kitap\nx:oku"] }))["hata"].is_string());
    assert!(o.sayfadan("/api/token", &json!({ "ad": "", "izinler": ["kitap.listele:oku"] }))["hata"].is_string());
}

#[test]
fn ayna_tokeni_yalnizca_secilen_islemleri_gosterir_ve_iptal_edilir() {
    let o = Ortam::yeni("uret");
    let v = o.sayfadan("/api/token", &json!({ "ad": "Claude – deneme", "izinler": ["kitap.listele:oku", "kitap.olustur:yaz"], "sure": 7, "onay": "yikici" }));
    let token = v["token"].as_str().unwrap_or_else(|| panic!("{v}")).to_string();
    assert!(token.starts_with("szg_"));
    let baglanti = v["baglanti"].to_string();
    // Yapay zekânın göreceği işlem tool'ları önizlenir (MCP ile aynı adlar)
    let araclar: Vec<&str> = v["araclar"].as_array().unwrap().iter().map(|t| t["ad"].as_str().unwrap()).collect();
    assert_eq!(araclar, ["kitap_listele", "kitap_olustur"], "{v}");
    assert!(baglanti.contains("claude mcp add sezgi -e SEZGI_TOKEN=szg_"), "{baglanti}");
    // Liste token'ı ve izinlerini gösterir, açık hâlini ya da özetini göstermez
    let liste = o.izinler()["tokenler"].clone();
    assert_eq!(liste[0]["ad"], "Claude – deneme");
    assert_eq!(liste[0]["durum"], "gecerli");
    assert!(!liste.to_string().contains(&token[4..]) && liste[0].get("ozet").is_none(), "{liste}");
    // MCP: yalnızca seçilen iki işlem görünür
    let t = o.mcp_ara(&token, "kitap kitaplar");
    assert!(t.contains("GET /api/v1/kitaplar ·") && t.contains("POST /api/v1/kitaplar ·"), "{t}");
    assert!(!t.contains("DELETE") && !t.contains("PATCH"), "{t}");
    assert!(o.mcp_liste(&token).contains("\"kitap_olustur\""));
    // Aynı adla ikinci geçerli token yok
    assert!(o.sayfadan("/api/token", &json!({ "ad": "Claude – deneme", "izinler": ["kitap.listele:oku"] }))["hata"].is_string());
    // İptal: liste ve MCP hemen görür
    assert_eq!(o.sayfadan("/api/token/iptal", &json!({ "ad": "Claude – deneme" }))["tamam"], true);
    assert_eq!(o.izinler()["tokenler"][0]["durum"], "iptal");
    assert!(o.mcp_ara(&token, "kitap").contains("Token geçersiz"));
}
