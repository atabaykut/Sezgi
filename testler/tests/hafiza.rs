//! Deneyim hafızası (vizyon 9.4, 10): aynı okuma işi ikinci kez uygulamaya gitmez. Varsayılan kapalı; yalnızca yan
//! etkisiz okumalar; token'lar birbirinin sonucunu görmez; harita değişince eski sonuç kullanılmaz; kişisel veri
//! içeren cevap ve yazma sonucu hatırlanmaz.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// İstek sayan sahte uygulama; /kisi adresine e-posta içeren cevap verir
fn sahte_uygulama() -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let sayac = Arc::new(AtomicUsize::new(0));
    let s2 = sayac.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            s2.fetch_add(1, Ordering::SeqCst);
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut ilk = String::new();
            r.read_line(&mut ilk).unwrap();
            let mut boy = 0usize;
            loop {
                let mut l = String::new();
                r.read_line(&mut l).unwrap();
                if l.trim().is_empty() {
                    break;
                }
                if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                    boy = v.trim().parse().unwrap();
                }
            }
            let mut g = vec![0u8; boy];
            r.read_exact(&mut g).unwrap();
            let cevap = if ilk.contains("/kitaplar/7") { "{\"yazar\":\"kisi@ornek.test\"}".to_string() } else { format!("{{\"sira\":{}}}", s2.load(Ordering::SeqCst)) };
            let mut s = s;
            write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cevap}", cevap.len()).unwrap();
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    (port, sayac)
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
    ev: PathBuf,
    sayac: Arc<AtomicUsize>,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-hafiza-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let ev = kok.join("ev");
        std::fs::create_dir_all(ev.join("ayar")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &kok.join("app"));
        let (port, sayac) = sahte_uygulama();
        let o = Ortam { kok, ev, sayac };
        o.ok(&["init", "."]);
        o.ok(&["hedef", &format!("http://127.0.0.1:{port}"), "--test-kopyasi"]);
        o.ok(&["mod", "yazma"]);
        let c = o.sezgi(&["oturum", "ac"], Some("eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0.x\n"));
        assert!(c.status.success());
        o
    }
    fn komut(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args)
            .current_dir(self.kok.join("app"))
            .env("HOME", &self.ev)
            .env("USERPROFILE", &self.ev)
            .env("APPDATA", self.ev.join("ayar"))
            .env("XDG_CONFIG_HOME", self.ev.join("ayar"))
            .env_remove("SEZGI_TOKEN")
            // Oturum anahtarlığa değil dosyaya: testler gerçek kimlik deposuna yazmaz
            .env("SEZGI_ANAHTARLIK", "kapali");
        c
    }
    fn sezgi(&self, args: &[&str], girdi: Option<&str>) -> Output {
        let mut p = self.komut(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        if let Some(g) = girdi {
            p.stdin.as_mut().unwrap().write_all(g.as_bytes()).unwrap();
        }
        drop(p.stdin.take());
        p.wait_with_output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let c = self.sezgi(args, None);
        let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
        assert!(c.status.success(), "{args:?}: {m}");
        m
    }
    fn token(&self, ad: &str) -> String {
        let m = self.ok(&["token", "olustur", "--ad", ad, "--izin", "kitap.*:yaz", "--izin", "yorum.*:yaz"]);
        m.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string()
    }
    /// Verilen token'la MCP üzerinden araç çağrıları
    fn cagir(&self, token: &str, kapilar: &[&str]) -> Vec<String> {
        let mut p = self.komut(&["mcp", ".sezgi"]).env("SEZGI_TOKEN", token).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let mut stdin = p.stdin.take().unwrap();
        let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
        let mut out = vec![];
        for (i, k) in kapilar.iter().enumerate() {
            let (yontem, _) = k.split_once(' ').unwrap();
            let arg = if yontem == "POST" {
                serde_json::json!({ "kapi": k, "govde": { "metin": "Kuyucaklı Yusuf", "puan": 4, "kitapId": "8a3c1e2b-1111-2222-3333-444455556666" }, "tekrar_anahtari": format!("t{i}") })
            } else {
                serde_json::json!({ "kapi": k })
            };
            let istek = serde_json::json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": "cagir", "arguments": arg } });
            writeln!(stdin, "{istek}").unwrap();
            stdin.flush().unwrap();
            let v: serde_json::Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
            out.push(v["result"]["content"][0]["text"].as_str().unwrap().to_string());
        }
        drop(stdin);
        let _ = p.wait();
        out
    }
    fn n(&self) -> usize {
        self.sayac.load(Ordering::SeqCst)
    }
}

#[test]
fn varsayilan_kapali() {
    let o = Ortam::yeni("kapali");
    let t = o.token("ajan");
    o.cagir(&t, &["GET /api/v1/kitaplar", "GET /api/v1/kitaplar"]);
    assert_eq!(o.n(), 2, "hafıza kapalıyken her istek uygulamaya gider");
}

#[test]
fn ayni_okuma_hafizadan_gelir() {
    let o = Ortam::yeni("acik");
    o.ok(&["hafiza", "ac", "--oturumlu"]);
    let t = o.token("ajan");
    let c = o.cagir(&t, &["GET /api/v1/kitaplar", "GET /api/v1/kitaplar", "GET /api/v1/kitaplar/3"]);
    assert_eq!(o.n(), 2, "aynı okuma ikinci kez gitmez, farklı parametre gider");
    assert!(c[1].contains("HAFIZADAN") && c[1].contains("\"sira\":1"), "aynı sonuç: {}", c[1]);
    assert!(!c[2].contains("HAFIZADAN"));
    let k = o.ok(&["kayit"]);
    assert!(k.contains("hafizadan"), "hafızadan verilen cevap da kayda geçer: {k}");
    // Hatırlanan cevap müşteri verisidir: diskte şifreli (ne düz ne base64)
    let uyg = std::fs::read_dir(o.ev.join("ayar").join("sezgi").join("uygulamalar")).unwrap().next().unwrap().unwrap().path();
    for g in std::fs::read_dir(uyg.join("hafiza")).unwrap().flatten() {
        let m = std::fs::read_to_string(g.path()).unwrap();
        // Yapısal denetim (base64 metninde alt dizgi aramak rastgele eşleşebilir): yalnızca şifreli yük, yük düz JSON değil
        // Kayıt `{durum, zaman, bitis, govde: {s, v}}`: gövde yalnızca şifreli yüktür
        let d: serde_json::Value = serde_json::from_str(m.trim()).unwrap();
        let v = &d["govde"];
        assert!(v.as_object().unwrap().keys().all(|k| k == "s" || k == "v") && v["s"] == "c20p1", "{m}");
        use base64::Engine;
        let yuk = base64::engine::general_purpose::STANDARD.decode(v["v"].as_str().unwrap()).unwrap();
        assert!(serde_json::from_slice::<serde_json::Value>(&yuk).is_err() && !m.contains("\"sira\""), "{m}");
    }
    // Başka token aynı sonucu görmez (izolasyon)
    let t2 = o.token("baska");
    let c = o.cagir(&t2, &["GET /api/v1/kitaplar"]);
    assert!(!c[0].contains("HAFIZADAN"));
    assert_eq!(o.n(), 3);
}

#[test]
fn yazma_ve_kisisel_veri_hatirlanmaz() {
    let o = Ortam::yeni("yazma");
    o.ok(&["hafiza", "ac"]);
    let t = o.token("ajan");
    o.cagir(&t, &["POST /api/v1/yorumlar", "POST /api/v1/yorumlar"]);
    assert_eq!(o.n(), 2, "yazma sonucu hiçbir zaman hatırlanmaz");
    o.cagir(&t, &["GET /api/v1/kitaplar/7", "GET /api/v1/kitaplar/7"]);
    assert_eq!(o.n(), 4, "e-posta içeren cevap hatırlanmaz");
}

#[test]
fn harita_degisince_eski_sonuc_kullanilmaz() {
    let o = Ortam::yeni("tazelik");
    o.ok(&["hafiza", "ac", "--oturumlu"]);
    let t = o.token("ajan");
    o.cagir(&t, &["GET /api/v1/kitaplar"]);
    // Kod değişti, harita yeniden üretildi
    let f = o.kok.join("app").join("src").join("kitap").join("kitap.controller.ts");
    let m = std::fs::read_to_string(&f).unwrap();
    std::fs::write(&f, format!("{m}\n// değişiklik\n")).unwrap();
    o.ok(&["init", "."]);
    let c = o.cagir(&t, &["GET /api/v1/kitaplar"]);
    assert!(!c[0].contains("HAFIZADAN"), "{}", c[0]);
    assert_eq!(o.n(), 2);
}

#[test]
fn hafiza_temizlenir_ve_defterde_gorunur() {
    let o = Ortam::yeni("defter");
    o.ok(&["hafiza", "ac", "--sure", "3", "--oturumlu"]);
    let t = o.token("ajan");
    o.cagir(&t, &["GET /api/v1/kitaplar"]);
    let d = o.ok(&["defter"]);
    assert!(d.contains("Deneyim hafızası") && d.contains("3 gün"), "{d}");
    let h = o.ok(&["hafiza"]);
    assert!(h.contains("1 sonuç"), "{h}");
    o.ok(&["unut", "--hepsi", "--evet"]);
    assert!(o.ok(&["hafiza"]).contains("0 sonuç"));
    o.ok(&["hafiza", "kapat"]);
    o.cagir(&t, &["GET /api/v1/kitaplar", "GET /api/v1/kitaplar"]);
    assert_eq!(o.n(), 3);
}
