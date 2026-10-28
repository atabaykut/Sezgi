//! `sezgi sun`: HTTP + OpenAPI köprüsü. Araç çağrısı, MCP JSON-RPC, iki OpenAPI belgesi ve yerel güvenlik
//! (Host, Origin, içerik türü).
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap() {
        let p = g.unwrap().path();
        let h = hedef.join(p.file_name().unwrap());
        if p.is_dir() {
            kopyala(&p, &h);
        } else {
            std::fs::copy(&p, &h).unwrap();
        }
    }
}

struct Sun {
    p: Child,
    port: u16,
    kok: PathBuf,
}

impl Drop for Sun {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

fn baslat(ad: &str) -> Sun {
    let kok = std::env::temp_dir().join(format!("sezgi-sun-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &kok);
    let ayar = kok.join("ayar");
    std::fs::create_dir_all(&ayar).unwrap();
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success());
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["sun", kok.join(".sezgi").to_str().unwrap(), "--port", "0"])
        .env("APPDATA", &ayar)
        .env("XDG_CONFIG_HOME", &ayar)
        .env("HOME", &ayar)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
    let port = satirlar.by_ref().map(|s| s.unwrap()).find_map(|s| s.split("http://127.0.0.1:").nth(1).and_then(|r| r.split('/').next()).and_then(|x| x.parse().ok())).unwrap();
    Sun { p, port, kok }
}

/// (durum kodu, gövde)
fn iste(port: u16, yontem: &str, yol: &str, basliklar: &[(&str, &str)], govde: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut b = format!("{yontem} {yol} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n", govde.len());
    if !basliklar.iter().any(|(k, _)| *k == "Host") {
        b.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    for (k, v) in basliklar {
        b.push_str(&format!("{k}: {v}\r\n"));
    }
    b.push_str("\r\n");
    s.write_all(b.as_bytes()).unwrap();
    s.write_all(govde.as_bytes()).unwrap();
    let mut ham = Vec::new();
    let _ = s.read_to_end(&mut ham);
    let m = String::from_utf8_lossy(&ham).to_string();
    let kod = m.split_whitespace().nth(1).unwrap().parse().unwrap();
    (kod, m.split_once("\r\n\r\n").map(|x| x.1.to_string()).unwrap_or_default())
}

const JSON: (&str, &str) = ("Content-Type", "application/json");

#[test]
fn araclar_http_ve_mcp_uzerinden() {
    let s = baslat("arac");
    let (kod, g) = iste(s.port, "GET", "/openapi.json", &[], "");
    assert_eq!(kod, 200);
    let belge: serde_json::Value = serde_json::from_str(&g).unwrap();
    assert_eq!(belge["openapi"], "3.1.0");
    assert!(belge["paths"]["/araclar/tani"]["post"]["requestBody"].is_object(), "{belge}");
    assert!(belge["paths"]["/araclar/cagir"]["post"]["security"].is_array());
    let (kod, g) = iste(s.port, "POST", "/araclar/tani", &[JSON], r#"{"kapi":"POST /api/v1/kitaplar"}"#);
    assert_eq!(kod, 200);
    let v: serde_json::Value = serde_json::from_str(&g).unwrap();
    assert_eq!(v["hata"], false);
    assert!(v["metin"].as_str().unwrap().contains("baslik"), "{v}");
    let (kod, g) = iste(s.port, "POST", "/mcp", &[JSON], r#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#);
    assert_eq!(kod, 200);
    let v: serde_json::Value = serde_json::from_str(&g).unwrap();
    assert_eq!(v["id"], 7);
    assert!(v["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "onizle"));
    // Bildirim: cevap gövdesi yok
    assert_eq!(iste(s.port, "POST", "/mcp", &[JSON], r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).0, 202);
    // Canlı mod kapalı: cagir HTTP'de de aynı kurallardan geçer
    let (_, g) = iste(s.port, "POST", "/araclar/cagir", &[JSON], r#"{"kapi":"GET /api/v1/kitaplar"}"#);
    assert!(g.contains("gözlem"), "{g}");
}

#[test]
fn uygulamanin_openapi_belgesi() {
    let s = baslat("uygulama");
    let (kod, g) = iste(s.port, "GET", "/uygulama/openapi.json", &[], "");
    assert_eq!(kod, 200);
    let v: serde_json::Value = serde_json::from_str(&g).unwrap();
    let k: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(s.kok.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let islem: usize = v["paths"].as_object().unwrap().values().map(|p| p.as_object().unwrap().keys().filter(|m| !m.starts_with("x-")).count()).sum();
    let ana = k["endpointler"].as_array().unwrap().iter().filter(|e| e["ana"] != false && e["yontem"] != "ALL").count();
    assert_eq!(islem, ana);
    let post = &v["paths"]["/api/v1/kitaplar"]["post"];
    let sema_adi = post["requestBody"]["content"]["application/json"]["schema"]["$ref"].as_str().unwrap().rsplit('/').next().unwrap();
    let sema = &v["components"]["schemas"][sema_adi];
    assert!(sema["required"].as_array().unwrap().iter().any(|r| r == "baslik"), "{sema}");
    assert_eq!(sema["properties"]["sayfa"]["minimum"], 1.0);
    assert_eq!(sema["properties"]["dil"]["enum"], serde_json::json!(["tr", "en"]));
    assert_eq!(v["paths"]["/api/v1/kitaplar"]["get"]["security"], serde_json::json!([]), "herkese açık uç");
}

#[test]
fn yerel_guvenlik() {
    let s = baslat("guvenlik");
    // DNS yeniden bağlama: başka bir ad
    assert_eq!(iste(s.port, "GET", "/openapi.json", &[("Host", "kotu.example.com")], "").0, 403);
    // Tarayıcıdan basit form isteği (JSON değil)
    assert_eq!(iste(s.port, "POST", "/araclar/kimsin", &[("Content-Type", "text/plain")], "{}").0, 415);
    // Başka bir kökenden istek
    assert_eq!(iste(s.port, "POST", "/araclar/kimsin", &[JSON, ("Origin", "https://kotu.example.com")], "{}").0, 403);
    // Yerel köken kabul edilir
    let yerel = format!("http://127.0.0.1:{}", s.port);
    assert_eq!(iste(s.port, "POST", "/araclar/kimsin", &[JSON, ("Origin", yerel.as_str())], "{}").0, 200);
    // CORS başlığı verilmez
    let mut c = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    write!(c, "GET /openapi.json HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: https://kotu.example.com\r\n\r\n", s.port).unwrap();
    let mut ham = String::new();
    let _ = c.read_to_string(&mut ham);
    assert!(!ham.to_ascii_lowercase().contains("access-control-allow-origin"));
}

/// İşlem tool'ları HTTP'de de: Bearer token'ın açtığı işlemler tools/list'te, /araclar/<ad> ve OpenAPI belgesinde
#[test]
fn islem_araclari_http_uzerinden() {
    let s = baslat("islem");
    let ayar = s.kok.join("ayar");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["token", "olustur", "--ad", "http", "--izin", "kitap.listele:oku", "--harita", s.kok.join(".sezgi").to_str().unwrap()])
        .env("APPDATA", &ayar)
        .env("XDG_CONFIG_HOME", &ayar)
        .env("HOME", &ayar)
        .output()
        .unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    let token = String::from_utf8_lossy(&c.stdout).split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
    let yetki = format!("Bearer {token}");
    let (_, g) = iste(s.port, "POST", "/mcp", &[JSON, ("Authorization", &yetki)], r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
    assert!(g.contains("\"kitap_listele\""), "{g}");
    // Tokensiz istemci aynı sunucuda işlem tool'u görmez
    let (_, g) = iste(s.port, "POST", "/mcp", &[JSON], r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    assert!(!g.contains("kitap_listele"), "{g}");
    let (_, g) = iste(s.port, "GET", "/openapi.json", &[("Authorization", &yetki)], "");
    assert!(g.contains("/araclar/kitap_listele"), "{g}");
    let (_, g) = iste(s.port, "POST", "/araclar/kitap_listele", &[JSON, ("Authorization", &yetki)], "{}");
    assert!(g.contains("GÖZLEM MODU"), "{g}");
}
