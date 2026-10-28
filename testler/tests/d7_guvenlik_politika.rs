//! Güvenlik (d7): onaysız yazma politikası.
//! - Çelişkili kanıt: işlemin bir önermesine çelişen kanıt varsa (ör. ağ kaydında koddaki şemaya uymayan başarılı gövde)
//!   yazma onaysız gitmez, onay kuyruğuna düşer ve gerekçe çelişkiyi söyler.
//! - Terminalsiz `oturum ac --rol` beyanı (betik ya da ajan) doğrulanmış insan rolü sayılmaz: rol isteyen yazma onaya
//!   düşer. Terminalde beyan edilen rol sayılır.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Koddaki YorumDto ile çelişen başarılı istek: zorunlu `metin` ve `kitapId` yok, `puan` metin
const CELISKI_HAR: &str = r#"{"log":{"version":"1.2","entries":[
{"startedDateTime":"2026-10-25T10:00:00Z","request":{"method":"POST","url":"http://127.0.0.1:8001/api/v1/yorumlar","headers":[{"name":"Content-Type","value":"application/json"}],"queryString":[],"postData":{"mimeType":"application/json","text":"{\"puan\":\"cok\"}"}},"response":{"status":201,"headers":[],"content":{"mimeType":"application/json","text":"{\"id\":\"1\"}"}}}
]}}"#;

#[test]
fn celiskili_kanitla_onaysiz_yazma_yok() {
    let ev = Ev::yeni("d7g-celiski");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    std::fs::write(p.klasor.join("celiski.har"), CELISKI_HAR).unwrap();
    p.ok(&["gozlem", "al", "celiski.har"]);
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let mut m = p.mcp(&t);
    let (s, _) = m.arac("tani", serde_json::json!({ "kapi": "POST /api/v1/yorumlar" }));
    assert!(s.contains("çelişkili"), "kurulum: kanıt çelişkili olmalı: {s}");
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR") && s.contains("çelişk"), "çelişkili kanıtla onaysız yazılmaz: {s}");
    assert_eq!(u.sayi(), 0);
}

#[test]
fn celiskisiz_kesin_yazma_onaysiz_gider() {
    let ev = Ev::yeni("d7g-celiskisiz");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(u.sayi(), 1);
}

/// Her isteğe 201 dönen uygulama (rol ucu yok). Döner: (port, istek sayısı)
fn uygulama() -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let sayac = Arc::new(AtomicUsize::new(0));
    let s2 = sayac.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut ilk = String::new();
            r.read_line(&mut ilk).unwrap();
            let mut boy = 0usize;
            loop {
                let mut b = String::new();
                r.read_line(&mut b).unwrap();
                if b.trim().is_empty() {
                    break;
                }
                if let Some(v) = b.to_ascii_lowercase().strip_prefix("content-length:") {
                    boy = v.trim().parse().unwrap();
                }
            }
            let mut g = vec![0u8; boy];
            r.read_exact(&mut g).unwrap();
            s2.fetch_add(1, Ordering::SeqCst);
            let cevap = "{\"id\":\"1\"}";
            let mut s = s;
            let _ = write!(s, "HTTP/1.1 201 X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cevap}", cevap.len());
        }
    });
    (port, sayac)
}

#[test]
fn terminalsiz_rol_beyani_dogrulanmis_sayilmaz() {
    let ev = Ev::yeni("d7g-rol-betik");
    let p = ev.proje("s3_rol", "a");
    let (port, sayac) = uygulama();
    p.ok(&["hedef", &format!("http://127.0.0.1:{port}"), "--test-kopyasi"]);
    p.ok(&["mod", "yazma"]);
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    // Betik: oturum değeri ve rol standart girdiden, terminal yok
    let c = p.sezgi(&["oturum", "ac", "--rol", "EDITOR"], Some("OTURUM-ALICE\n"));
    let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(c.status.success(), "oturum yine açılır: {m}");
    assert!(!m.contains("Doğrulanmış roller") && m.contains("doğrulanmış sayılmaz"), "{m}");
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR") && s.contains("rol"), "betikle beyan edilen rol 8. şartı sağlamaz: {s}");
    assert_eq!(sayac.load(Ordering::SeqCst), 0);
    let k = p.ok(&["kayit", "--json"]);
    assert!(!k.contains("\"insan_beyani\""), "doğrulanmamış beyan kaynak olarak yazılmaz: {k}");
}

#[cfg(unix)]
#[test]
fn terminalde_rol_beyani_sayilir() {
    let ev = Ev::yeni("d7g-rol-terminal");
    let p = ev.proje("s3_rol", "a");
    let (port, sayac) = uygulama();
    p.ok(&["hedef", &format!("http://127.0.0.1:{port}"), "--test-kopyasi"]);
    p.ok(&["mod", "yazma"]);
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let m = p.oturum_rollu("OTURUM-ALICE", &["EDITOR"]);
    assert!(m.contains("Doğrulanmış roller") && m.contains("EDITOR"), "{m}");
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(sayac.load(Ordering::SeqCst), 1);
}
