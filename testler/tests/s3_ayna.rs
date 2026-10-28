//! S3: Ayna'nın okuma uçları da oturum anahtarı ister; çalışan Ayna'nın anahtarlı adresi `sezgi ayna --adres` ile
//! yeniden yazdırılabilir.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::Stdio;

fn iste(port: u16, yol: &str, anahtar: Option<&str>) -> (String, String) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let ek = anahtar.map(|a| format!("X-Sezgi-Anahtar: {a}\r\n")).unwrap_or_default();
    write!(s, "GET {yol} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{ek}\r\n").unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    let (d, g) = c.split_once("\r\n\r\n").unwrap_or((&c, ""));
    (d.lines().next().unwrap_or("").to_string(), g.to_string())
}

#[test]
fn ayna_okuma_uclari_anahtar_ister_adres_yeniden_yazdirilir() {
    let ev = Ev::yeni("s3-ayna");
    let p = ev.proje("nestjs-temel", "a");
    let mut c = p.komut(&["ayna", ".sezgi", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    let (port, anahtar) = loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0);
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port: u16 = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let a: String = k.split("#anahtar=").nth(1).unwrap().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            break (port, a);
        }
    };
    for yol in ["/api/harita", "/api/kart", "/api/rapor", "/api/imza", "/api/izinler"] {
        let (d, _) = iste(port, yol, None);
        assert!(d.contains("403"), "{yol} anahtarsız açılmamalı: {d}");
        let (d, _) = iste(port, yol, Some("yanlis"));
        assert!(d.contains("403"), "{yol}: {d}");
        let (d, _) = iste(port, yol, Some(&anahtar));
        assert!(d.contains("200"), "{yol}: {d}");
    }
    // Sayfanın kendisi anahtarsız açılır (anahtar adres parçasındadır); sayfa anahtarsız açılışta ne yapılacağını söyler
    let (d, g) = iste(port, "/", None);
    assert!(d.contains("200") && g.contains("ayna.js"), "{d}");
    let js = iste(port, "/ayna.js", None).1;
    assert!(js.contains("sezgi ayna --adres"), "anahtarsız açılışta yol gösterilmeli");
    // Çalışan Ayna'nın anahtarlı adresi yeniden yazdırılır
    let m = p.ok(&["ayna", "--adres"]);
    assert!(m.contains(&format!("http://127.0.0.1:{port}/#anahtar={anahtar}")), "{m}");
    let _ = c.kill();
    let _ = c.wait();
}
