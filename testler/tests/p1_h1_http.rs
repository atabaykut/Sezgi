//! P1 H1 (B3): bozuk parçalı (chunked) cevap Sezgi'yi düşürmez; `sezgi sun` bir panikten sonra da cevap verir ve hedefe
//! giden yavaş bir istek salt okuma araçlarını bekletmez (ağ çağrısı kilit dışında).
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

/// Her isteğe verilen ham cevabı yazıp bekleyen ve bağlantıyı kapatan sahte hedef
fn ham_hedef(cevap: &'static str, bekle: Duration) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut r = BufReader::new(s.try_clone().unwrap());
                loop {
                    let mut l = String::new();
                    if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
                        break;
                    }
                }
                std::thread::sleep(bekle);
                let mut s = s;
                let _ = s.write_all(cevap.as_bytes());
                let _ = s.shutdown(std::net::Shutdown::Both);
            });
        }
    });
    port
}

const BOZUK: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello";
const YAVAS: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";

struct Sun {
    p: Child,
    port: u16,
}

impl Drop for Sun {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
    }
}

fn sun(p: &Proje) -> Sun {
    let mut c = p.komut(&["sun", ".sezgi", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let satirlar = BufReader::new(c.stdout.take().unwrap()).lines();
    let port = satirlar.map(|s| s.unwrap()).find_map(|s| s.split("http://127.0.0.1:").nth(1).and_then(|r| r.split('/').next()).and_then(|x| x.parse().ok())).unwrap();
    Sun { p: c, port }
}

fn iste(port: u16, yol: &str, token: &str, govde: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    write!(s, "POST {yol} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n{govde}", govde.len())
        .unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    let kod = c.split_whitespace().nth(1).and_then(|k| k.parse().ok()).unwrap_or(0);
    (kod, c.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

fn kur(ad: &str, hedef_port: u16) -> (Ev, Proje, String) {
    let ev = Ev::yeni(ad);
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["hedef", &format!("http://127.0.0.1:{hedef_port}"), "--test-kopyasi"]);
    p.ok(&["mod", "okuma"]);
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    (ev, p, t)
}

#[test]
fn bozuk_parcali_cevap_hata_verir_sunucu_ayakta_kalir() {
    let (_ev, p, t) = kur("http-bozuk", ham_hedef(BOZUK, Duration::ZERO));
    let s = sun(&p);
    for _ in 0..2 {
        let (kod, g) = iste(s.port, "/araclar/cagir", &t, r#"{"kapi":"GET /api/v1/kitaplar"}"#);
        assert_eq!(kod, 200, "cevap gelmeli: {g}");
        assert!(g.contains("parçalı") && g.contains("\"hata\":true"), "{g}");
    }
    // Sonraki istekler de cevap alır (zehirlenmiş kilit yok)
    let (kod, g) = iste(s.port, "/araclar/kimsin", &t, "{}");
    assert!(kod == 200 && g.contains("kapı"), "{kod} {g}");
    // MCP stdio: süreç düşmez, ikinci çağrı da cevaplanır
    let mut m = p.mcp(&t);
    let (s1, h1) = m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(h1 && s1.contains("parçalı"), "{s1}");
    let (s2, _) = m.arac("kimsin", serde_json::json!({}));
    assert!(s2.contains("kapı"), "{s2}");
}

#[test]
fn yavas_hedef_salt_okuma_araclarini_bekletmez() {
    let (_ev, p, t) = kur("http-yavas", ham_hedef(YAVAS, Duration::from_secs(4)));
    let s = sun(&p);
    let port = s.port;
    let t2 = t.clone();
    let yavas = std::thread::spawn(move || iste(port, "/araclar/cagir", &t2, r#"{"kapi":"GET /api/v1/kitaplar"}"#));
    std::thread::sleep(Duration::from_millis(400));
    let bas = Instant::now();
    let (kod, g) = iste(port, "/araclar/ara", &t, r#"{"sorgu":"kitap"}"#);
    let sure = bas.elapsed();
    assert_eq!(kod, 200, "{g}");
    assert!(sure < Duration::from_secs(2), "ara, yavaş hedefe giden isteği beklememeli: {sure:?}");
    let (kod, g) = yavas.join().unwrap();
    assert!(kod == 200 && g.contains("OKUNDU"), "{g}");
}
