//! D9 doğruluk: sahte çelişki yok, sayılar her yüzeyde aynı.
//! - Tablo olmayan Pydantic / SQLModel sınıfları (`ItemUpdate(ItemBase)`, `BaseModel`) veri şeması sayılmaz: girdi alanı
//!   "DATABASE_SCHEMA" ile çelişmez.
//! - Paylaşılan doğrulama satırı (`KitapGuncelleDto extends PartialType(KitapOlusturDto)`) birden çok alanı doğrular;
//!   aynı kural farklı alanlar için çelişki değildir.
//! - Aynı kaynak türünün farklı değerleri (aynı girdi adı iki işlemde farklı zorunlulukla) bağlama göre değişen
//!   olgudur: çelişki sayılmaz, kapsam dışında not edilir. Çelişki farklı kaynak türleri arasındadır.
//! - Gerçek çelişki (ağ kaydında koddaki şemaya uymayan başarılı gövde) Ayna, `rapor` ve MCP `bilinmeyenler`'de aynı
//!   sayıdır. `etki` ile davranış özeti aynı tabloyu aynı yönde (yazma) söyler.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Stdio};

/// Koddaki YorumDto ile çelişen başarılı istek
const CELISKI_HAR: &str = r#"{"log":{"version":"1.2","entries":[
{"startedDateTime":"2026-10-25T10:00:00Z","request":{"method":"POST","url":"http://127.0.0.1:8001/api/v1/yorumlar","headers":[{"name":"Content-Type","value":"application/json"}],"queryString":[],"postData":{"mimeType":"application/json","text":"{\"puan\":\"cok\"}"}},"response":{"status":201,"headers":[],"content":{"mimeType":"application/json","text":"{\"id\":\"1\"}"}}}
]}}"#;

struct Ayna(Child, u16, String);

impl Drop for Ayna {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[allow(clippy::zombie_processes)]
fn ayna(p: &Proje) -> Ayna {
    let mut c = p.komut(&["ayna", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0, "Ayna adres yazmadı");
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let anahtar = k.split("#anahtar=").nth(1).unwrap().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            return Ayna(c, port, anahtar);
        }
    }
}

fn sorular(a: &Ayna) -> Value {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", a.1)).unwrap();
    write!(s, "GET /api/sorular HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Sezgi-Anahtar: {}\r\n\r\n", a.1, a.2).unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    serde_json::from_str(c.split_once("\r\n\r\n").unwrap().1).unwrap()
}

fn kopyala(kaynak: &std::path::Path, hedef: &std::path::Path) {
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

/// MCP aracı (token'sız, salt okuma)
fn arac(p: &Proje, ad: &str, a: Value) -> String {
    let mut c = p.komut(&["mcp", ".sezgi"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": a } });
    writeln!(c.stdin.take().unwrap(), "{istek}").unwrap();
    let o = String::from_utf8_lossy(&c.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(o.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

/// (rapor, Ayna, MCP) çelişkili önerme sayıları
fn celiski_sayilari(p: &Proje) -> (u64, u64, u64) {
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    let rapor = r["kanit"]["celiski"].as_u64().unwrap_or(0);
    let a = ayna(p);
    let b = sorular(&a);
    let guven = b["sorular"].as_array().unwrap().iter().find(|s| s["id"] == "guven").unwrap().clone();
    let ayna_sayi = guven["maddeler"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["metin"].as_str())
        .find_map(|t| t.split_once(" önermede çelişen kanıt").and_then(|(n, _)| n.trim().parse::<u64>().ok()))
        .unwrap_or(0);
    let m = arac(p, "bilinmeyenler", json!({}));
    let mcp = m.split("çelişkili ").nth(1).and_then(|x| x.split_whitespace().next()).and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
    (rapor, ayna_sayi, mcp)
}

#[test]
fn temiz_depoda_sahte_celiski_yok() {
    let ev = Ev::yeni("d9dog-temiz");
    // Paylaşılan doğrulama satırı (PartialType)
    let n = ev.proje("nestjs-temel", "n");
    assert_eq!(celiski_sayilari(&n), (0, 0, 0));
    // Tablo olmayan SQLModel / Pydantic sınıfları
    let kok = ev.kok.join("f");
    kopyala(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("w2g_fastapi_template"), &kok);
    let f = Proje { klasor: kok.join("backend"), ev: ev.kok.join("ev") };
    f.ok(&["init", "."]);
    assert_eq!(celiski_sayilari(&f), (0, 0, 0));
}

#[test]
fn ayni_kaynak_turunun_baglami_celiski_degildir() {
    let ev = Ev::yeni("d9dog-baglam");
    let p = ev.proje("d9_dogruluk_govde", "a");
    assert_eq!(celiski_sayilari(&p), (0, 0, 0));
    let k: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kanit.json")).unwrap()).unwrap();
    let o = k["onermeler"].as_array().unwrap().iter().find(|o| o["varlik"].as_str().is_some_and(|v| v.starts_with("field:") && v.ends_with(".eposta"))).unwrap().clone();
    assert!(o["hesap"]["kapsam_disi"].to_string().contains("bağlama göre değişir"), "{o:#}");
}

#[test]
fn gercek_celiski_her_yuzeyde_ayni_sayi() {
    let ev = Ev::yeni("d9dog-celiski");
    let p = ev.proje("nestjs-temel", "a");
    std::fs::write(p.klasor.join("celiski.har"), CELISKI_HAR).unwrap();
    p.ok(&["gozlem", "al", "celiski.har"]);
    let (r, a, m) = celiski_sayilari(&p);
    assert!(r > 0, "ağ kaydı çelişkisi sayılmalı");
    assert_eq!((r, a, m), (r, r, r), "rapor / Ayna / MCP aynı sayı");
    let t = arac(&p, "tani", json!({ "kapi": "POST /api/v1/yorumlar" }));
    assert!(t.contains("çelişkili"), "{t}");
}

#[test]
fn etki_ve_davranis_ozeti_ayni_yon() {
    let ev = Ev::yeni("d9dog-etki");
    let p = ev.proje("d9_dogruluk_fastapi", "a");
    let h: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let k = h["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == "DELETE /users/:user_id").unwrap().clone();
    assert!(k["veri"]["yazar"].as_array().unwrap().iter().any(|t| t == "User"), "{k:#}");
    let e = arac(&p, "etki", json!({ "kapi": "DELETE /users/:user_id" }));
    let tablolar = e.lines().find(|l| l.contains("tablolar:")).unwrap_or_else(|| panic!("{e}"));
    let user = tablolar.split("), ").find(|x| x.trim_start().starts_with("tablolar: User") || x.trim_start().starts_with("User")).unwrap_or_else(|| panic!("{tablolar}"));
    assert!(user.contains("writes"), "okuyup silen işlem tabloya yazar: {tablolar}");
}
