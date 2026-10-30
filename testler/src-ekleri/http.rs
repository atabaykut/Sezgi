use super::*;

#[test]
fn adres_cozumleme() {
    let a = adres("http://127.0.0.1:3000/api").unwrap();
    assert_eq!((a.ana.as_str(), a.port, a.taban.as_str()), ("127.0.0.1", 3000, "/api"));
    let a = adres("http://localhost").unwrap();
    assert_eq!((a.port, a.taban.as_str()), (80, ""));
    let a = adres("https://ornek.test/v1").unwrap();
    assert_eq!((a.ana.as_str(), a.port, a.taban.as_str(), a.tls), ("ornek.test", 443, "/v1", true));
    assert!(adres("ftp://x").is_err());
    assert!(adres("http://kullanici:sifre@x").is_err());
}

#[test]
fn cevap_tamam_mi() {
    assert!(tamam(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nok"));
    assert!(!tamam(b"HTTP/1.1 201 Created\r\nContent-Length: 5\r\n\r\nok"));
    assert!(!tamam(b"HTTP/1.1 201 Created\r\nContent-Len"));
    assert!(tamam(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nok\r\n0\r\n\r\n"));
    assert!(!tamam(b"HTTP/1.1 200 OK\r\n\r\nbody-without-length"));
}

#[test]
fn parcali() {
    assert_eq!(parcali_coz(b"3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n").unwrap(), b"abcde");
}

#[test]
fn kodlama() {
    assert_eq!(kodla("a b/ç"), "a%20b%2F%C3%A7");
}

#[test]
fn duz_http_yalnizca_loopback_adrese_baglanir() {
    // Ad denetimden sonra başka adrese çözülse bile (DNS yeniden bağlama) düz http uzak sunucuya gitmez: kural
    // bağlanılan adreste uygulanır. 192.0.2.1 belgeleme ağıdır (TEST-NET-1), hiçbir yere bağlanılmaz.
    let a = adres("http://192.0.2.1:8080").unwrap();
    let Err(e) = istek(&a, "GET", "/", &[], None) else { panic!("uzak adrese düz http ile bağlanılmamalı") };
    assert!(e.contains("bu bilgisayardaki") || e.contains("on this computer"), "{e}");
}
