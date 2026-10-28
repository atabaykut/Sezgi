//! Aynı adreste birden fazla sürüm, iç içe nesneler ve birleşim gövdeler (tests/ornekler/nestjs-surum).
//! - Her sürüm kendi tool'unu, kendi şemasıyla alır; çağrı doğru sürümün başlığıyla gider.
//! - Uygulama sürümü koddan okunamayan bir yolla seçiyorsa Sezgi o adresteki işlemleri iletmez.
//! - Aynı adreste birden fazla işlem varken `cagir` hangisi olduğu belirtilmeden iletmez.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// Gelen isteklerin ilk satırı ve başlıkları (küçük harf)
type Gelenler = Arc<Mutex<Vec<(String, Vec<(String, String)>)>>>;

fn sahte_uygulama() -> (u16, Gelenler) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let gelenler: Gelenler = Arc::new(Mutex::new(vec![]));
    let g2 = gelenler.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut ilk = String::new();
            r.read_line(&mut ilk).unwrap();
            let (mut basliklar, mut boy) = (vec![], 0usize);
            loop {
                let mut l = String::new();
                r.read_line(&mut l).unwrap();
                let l = l.trim_end().to_string();
                if l.is_empty() {
                    break;
                }
                let (k, v) = l.split_once(':').unwrap();
                let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
                if k == "content-length" {
                    boy = v.parse().unwrap();
                }
                basliklar.push((k, v));
            }
            let mut govde = vec![0u8; boy];
            r.read_exact(&mut govde).unwrap();
            g2.lock().unwrap().push((ilk.trim().to_string(), basliklar));
            let mut s = s;
            write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]").unwrap();
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    (port, gelenler)
}

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

struct Ortam {
    kok: PathBuf,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-surum-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("ev").join("ayar")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-surum"), &kok.join("app"));
        let o = Ortam { kok };
        o.ok(&["init", "."]);
        o
    }
    fn komut(&self, args: &[&str]) -> Command {
        let ev = self.kok.join("ev");
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args)
            .current_dir(self.kok.join("app"))
            .env("HOME", &ev)
            .env("USERPROFILE", &ev)
            .env("APPDATA", ev.join("ayar"))
            .env("XDG_CONFIG_HOME", ev.join("ayar"))
            .env_remove("SEZGI_TOKEN");
        c
    }
    fn ok(&self, args: &[&str]) -> String {
        let c = self.komut(args).output().unwrap();
        let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
        assert!(c.status.success(), "{args:?}: {m}");
        m
    }
    fn token(&self) -> String {
        self.ok(&["token", "olustur", "--ad", "t", "--izin", "*:yikici", "--tum-islemler"]).split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string()
    }
    /// MCP istekleri; cevapların `result` kısımları
    fn mcp(&self, token: &str, istekler: &[Value]) -> Vec<Value> {
        let mut p = self.komut(&["mcp", ".sezgi"]).env("SEZGI_TOKEN", token).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let mut stdin = p.stdin.take().unwrap();
        for (i, v) in istekler.iter().enumerate() {
            let mut v = v.clone();
            v["jsonrpc"] = json!("2.0");
            v["id"] = json!(i + 1);
            writeln!(stdin, "{v}").unwrap();
        }
        drop(stdin);
        let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
        c.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["result"].clone()).collect()
    }
}

fn araclar(o: &Ortam, token: &str) -> Vec<Value> {
    o.mcp(token, &[json!({ "method": "tools/list" })])[0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| !["kimsin", "ara", "tani", "onizle", "cagir", "onay_durumu", "kanit", "bilinmeyenler", "etki"].contains(&t["name"].as_str().unwrap_or("")))
        .cloned()
        .collect()
}

fn arac<'a>(l: &'a [Value], aciklama: &str) -> &'a Value {
    l.iter().find(|t| t["description"].as_str().unwrap().contains(aciklama)).unwrap_or_else(|| panic!("{aciklama} yok: {l:?}"))
}

fn cagri(ad: &str, a: Value) -> Value {
    json!({ "method": "tools/call", "params": { "name": ad, "arguments": a } })
}

#[test]
fn surumler_ic_ice_nesneler_ve_birlesim_govde() {
    let o = Ortam::yeni("sema");
    let t = o.token();
    let l = araclar(&o, &t);
    assert_eq!(l.len(), 4, "iki sürüm × iki işlem: {l:?}");
    let v1 = arac(&l, "işlem siparis-v1.olustur");
    let v2 = arac(&l, "işlem siparis-v2.olustur");
    assert_ne!(v1["name"], v2["name"]);
    assert!(v1["description"].as_str().unwrap().contains("Sürüm 1") && v1["description"].as_str().unwrap().contains("sürüm başlığını gönderir"), "{v1}");
    // v1: iç içe nesne ve nesne dizisi (ValidateNested)
    let g = &v1["inputSchema"]["properties"]["govde"];
    assert_eq!(g["properties"]["adres"]["properties"]["sehir"]["type"], "string", "{g}");
    assert_eq!(g["properties"]["adres"]["required"], json!(["sehir"]));
    assert_eq!(g["properties"]["kalemler"]["items"]["properties"]["adet"]["type"], "integer", "{g}");
    assert_eq!(g["properties"]["kalemler"]["items"]["properties"]["adet"]["minimum"].as_f64(), Some(1.0));
    // v2: birleşim gövde, iki biçim, yorum damgalı
    let g = &v2["inputSchema"]["properties"]["govde"];
    assert_eq!(g["oneOf"].as_array().map(|a| a.len()), Some(2), "{g}");
    assert!(v2["description"].as_str().unwrap().contains("2 biçimden biri") && v2["description"].as_str().unwrap().contains("Yorum"), "{v2}");
}

#[test]
fn surum_basligi_gonderilir_ve_islem_secilmeden_iletilmez() {
    let o = Ortam::yeni("baslik");
    let (port, gelenler) = sahte_uygulama();
    o.ok(&["hedef", &format!("http://127.0.0.1:{port}"), "--test-kopyasi"]);
    o.ok(&["mod", "okuma"]);
    let t = o.token();
    let l = araclar(&o, &t);
    let v2 = arac(&l, "işlem siparis-v2.listele")["name"].as_str().unwrap().to_string();
    let c = o.mcp(&t, &[cagri(&v2, json!({})), cagri("cagir", json!({ "kapi": "GET /api/siparisler" }))]);
    assert!(c[0]["content"][0]["text"].as_str().unwrap().contains("OKUNDU"), "{:?}", c[0]);
    let g = gelenler.lock().unwrap().clone();
    assert_eq!(g.len(), 1, "yalnızca işlem tool'u iletildi: {g:?}");
    assert!(g[0].1.contains(&("x-api-surum".into(), "2".into())), "v2 başlığı: {:?}", g[0]);
    let m = c[1]["content"][0]["text"].as_str().unwrap();
    assert!(m.contains("İLETİLMEDİ") && m.contains("2 işlem var") && m.contains("islem"), "{m}");
}

#[test]
fn surum_secimi_okunamazsa_ayni_adresteki_islem_iletilmez() {
    let o = Ortam::yeni("ozel");
    // Sürüm özel bir fonksiyonla seçiliyor: koddan okunamaz
    let main = o.kok.join("app").join("src").join("main.ts");
    let m = std::fs::read_to_string(&main).unwrap().replace("{ type: VersioningType.HEADER, header: 'x-api-surum' }", "{ type: VersioningType.CUSTOM, extractor: (r) => sec(r) }");
    std::fs::write(&main, m).unwrap();
    o.ok(&["init", "."]);
    let (port, gelenler) = sahte_uygulama();
    o.ok(&["hedef", &format!("http://127.0.0.1:{port}"), "--test-kopyasi"]);
    o.ok(&["mod", "okuma"]);
    let t = o.token();
    let l = araclar(&o, &t);
    let v1 = arac(&l, "işlem siparis-v1.listele");
    assert!(v1["description"].as_str().unwrap().contains("Sezgi bu işlemi çağıramaz"), "{v1}");
    let c = o.mcp(&t, &[cagri(v1["name"].as_str().unwrap(), json!({}))]);
    let m = c[0]["content"][0]["text"].as_str().unwrap();
    assert!(m.contains("İLETİLMEDİ") && m.contains("sürüm"), "{m}");
    assert!(gelenler.lock().unwrap().is_empty());
}
