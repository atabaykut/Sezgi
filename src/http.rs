//! En küçük HTTP/1.1 istemcisi: `http://` ve `https://` (TLS: rustls, sertifikalar işletim sisteminin güven
//! deposundan; kendi imzalı test kopyası için `SEZGI_CA_DOSYASI` ile ek PEM sertifikası verilebilir).
//! Sertifika doğrulaması kapatılamaz.
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

pub struct Cevap {
    pub durum: u16,
    pub govde: Vec<u8>,
}

pub struct Adres {
    pub ana: String,
    pub port: u16,
    pub taban: String,
    pub tls: bool,
}

/// "http(s)://host:port/taban" -> (host, port, taban yol, tls)
pub fn adres(url: &str) -> Result<Adres, String> {
    let (kalan, tls) = match (url.strip_prefix("http://"), url.strip_prefix("https://")) {
        (Some(k), _) => (k, false),
        (_, Some(k)) => (k, true),
        _ => return Err(t!("adres http:// ya da https:// ile başlamalı", "the address must start with http:// or https://").into()),
    };
    let (ana_port, taban) = match kalan.find('/') {
        Some(i) => (&kalan[..i], kalan[i..].trim_end_matches('/').to_string()),
        None => (kalan, String::new()),
    };
    if ana_port.is_empty() || ana_port.contains('@') {
        return Err(t!("adres geçersiz", "invalid address").into());
    }
    let (ana, port) = match ana_port.rsplit_once(':') {
        Some((a, p)) if !a.contains(':') => (a.to_string(), p.parse::<u16>().map_err(|_| t!("port geçersiz", "invalid port"))?),
        Some(_) => return Err(t!("IPv6 adresleri henüz desteklenmiyor", "IPv6 addresses are not supported yet").into()),
        None => (ana_port.to_string(), if tls { 443 } else { 80 }),
    };
    Ok(Adres { ana, port, taban, tls })
}

/// URL bileşeni kodlama (RFC 3986 ayrılmamış karakterler dışında her bayt %XX)
pub fn kodla(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// TLS ayarı: işletim sisteminin güvendiği kök sertifikalar, varsa `SEZGI_CA_DOSYASI` içindeki PEM sertifikaları
fn tls_ayari() -> Result<Arc<rustls::ClientConfig>, String> {
    use rustls::pki_types::{pem::PemObject, CertificateDer};
    let mut kokler = rustls::RootCertStore::empty();
    for c in rustls_native_certs::load_native_certs().certs {
        let _ = kokler.add(c);
    }
    if let Some(dosya) = std::env::var_os("SEZGI_CA_DOSYASI") {
        let mut n = 0;
        for c in CertificateDer::pem_file_iter(&dosya).map_err(|e| tf!("SEZGI_CA_DOSYASI okunamadı: {e}", "could not read SEZGI_CA_DOSYASI: {e}"))? {
            kokler
                .add(c.map_err(|e| tf!("SEZGI_CA_DOSYASI bozuk: {e}", "SEZGI_CA_DOSYASI is corrupt: {e}"))?)
                .map_err(|e| tf!("sertifika eklenemedi: {e}", "could not add the certificate: {e}"))?;
            n += 1;
        }
        if n == 0 {
            return Err(t!("SEZGI_CA_DOSYASI içinde sertifika yok", "no certificate in SEZGI_CA_DOSYASI").into());
        }
    }
    if kokler.is_empty() {
        return Err(t!(
            "güvenilir kök sertifika bulunamadı (işletim sistemi deposu boş; SEZGI_CA_DOSYASI ile verin)",
            "no trusted root certificate found (the operating system store is empty; give one with SEZGI_CA_DOSYASI)"
        )
        .into());
    }
    let ayar = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(kokler)
        .with_no_client_auth();
    Ok(Arc::new(ayar))
}

pub fn istek(a: &Adres, yontem: &str, yol: &str, basliklar: &[(&str, String)], govde: Option<&[u8]>) -> Result<Cevap, String> {
    // Ad birden fazla adrese çözülebilir (localhost: önce ::1, sonra 127.0.0.1): hepsi sırayla denenir
    let adresler: Vec<_> = (a.ana.as_str(), a.port).to_socket_addrs().map_err(|e| tf!("adres çözülemedi: {e}", "could not resolve the address: {e}"))?.collect();
    let mut son_hata = String::from(t!("adres çözülemedi", "could not resolve the address"));
    let mut baglanti = None;
    for hedef in &adresler {
        match TcpStream::connect_timeout(hedef, Duration::from_secs(5)) {
            Ok(s) => {
                baglanti = Some(s);
                break;
            }
            Err(e) => son_hata = tf!("bağlanılamadı: {e}", "could not connect: {e}"),
        }
    }
    let s = baglanti.ok_or(son_hata)?;
    s.set_read_timeout(Some(Duration::from_secs(30))).ok();
    s.set_write_timeout(Some(Duration::from_secs(30))).ok();
    let mut b = format!("{yontem} {}{yol} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\nUser-Agent: sezgi/{}\r\n", a.taban, a.ana, a.port, env!("CARGO_PKG_VERSION"));
    for (k, v) in basliklar {
        if v.contains(['\r', '\n']) || k.contains(['\r', '\n', ':']) {
            return Err(t!("başlık geçersiz", "invalid header").into());
        }
        b.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(g) = govde {
        b.push_str(&format!("Content-Length: {}\r\n", g.len()));
    }
    b.push_str("\r\n");
    if a.tls {
        let ad = rustls::pki_types::ServerName::try_from(a.ana.clone()).map_err(|_| t!("TLS için sunucu adı geçersiz", "invalid server name for TLS"))?;
        let baglanti = rustls::ClientConnection::new(tls_ayari()?, ad).map_err(|e| tf!("TLS kurulamadı: {e}", "could not set up TLS: {e}"))?;
        gonder_al(rustls::StreamOwned::new(baglanti, s), b.as_bytes(), govde)
    } else {
        gonder_al(s, b.as_bytes(), govde)
    }
}

/// İsteği yazar, cevabı okur (düz TCP ya da TLS akışı)
fn gonder_al<S: Read + Write>(mut s: S, bas: &[u8], govde: Option<&[u8]>) -> Result<Cevap, String> {
    s.write_all(bas).map_err(|e| e.to_string())?;
    if let Some(g) = govde {
        s.write_all(g).map_err(|e| e.to_string())?;
    }
    s.flush().map_err(|e| e.to_string())?;
    // Sunucu cevabı yazıp bağlantıyı sert kapatabilir (Windows'ta RST): cevap tamamsa sonradan gelen sıfırlama hata sayılmaz
    let mut ham = Vec::new();
    let mut tampon = [0u8; 16 * 1024];
    loop {
        match s.read(&mut tampon) {
            Ok(0) => break,
            Ok(n) => {
                ham.extend_from_slice(&tampon[..n]);
                if ham.len() > 16 * 1024 * 1024 {
                    return Err(t!("cevap çok büyük", "response too large").into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if tamam(&ham) => {
                let _ = e;
                break;
            }
            Err(e) => return Err(tf!("cevap okunamadı: {e}", "could not read the response: {e}")),
        }
    }
    let bitis = ham.windows(4).position(|w| w == b"\r\n\r\n").ok_or(t!("cevap başlığı eksik", "response header missing"))?;
    let baslik = String::from_utf8_lossy(&ham[..bitis]).to_string();
    let durum: u16 = baslik.split_whitespace().nth(1).and_then(|d| d.parse().ok()).ok_or(t!("durum satırı okunamadı", "could not read the status line"))?;
    let mut govde = ham[bitis + 4..].to_vec();
    if baslik.lines().any(|l| l.to_ascii_lowercase().replace(' ', "") == "transfer-encoding:chunked") {
        govde = parcali_coz(&govde)?;
    }
    Ok(Cevap { durum, govde })
}

/// Cevap tamamen alındı mı: başlık bitmiş ve Content-Length kadar gövde gelmiş (ya da parçalı cevabın sonu gelmiş)
fn tamam(ham: &[u8]) -> bool {
    let Some(bitis) = ham.windows(4).position(|w| w == b"\r\n\r\n") else { return false };
    let baslik = String::from_utf8_lossy(&ham[..bitis]).to_ascii_lowercase();
    let govde = ham.len() - bitis - 4;
    if let Some(boy) = baslik.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())) {
        return govde >= boy;
    }
    baslik.lines().any(|l| l.replace(' ', "") == "transfer-encoding:chunked") && ham.ends_with(b"0\r\n\r\n")
}

/// Parçalı (chunked) gövdeyi çözer. Sunucu bağlantıyı erken kapatabilir ya da bozuk cevap verebilir: her dilim sınır
/// denetimiyle alınır, eksik ya da bozuk veri panik değil hata olur (bir hedef Sezgi'yi düşüremez).
fn parcali_coz(v: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        let kalan = v.get(i..).ok_or(t!("parçalı cevap eksik", "chunked response incomplete"))?;
        let son = kalan.windows(2).position(|w| w == b"\r\n").ok_or(t!("parçalı cevap bozuk ya da eksik", "chunked response corrupt or incomplete"))? + i;
        let boy = usize::from_str_radix(String::from_utf8_lossy(&v[i..son]).split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| t!("parçalı cevap bozuk", "chunked response corrupt"))?;
        if boy == 0 {
            return Ok(out);
        }
        let bas = son + 2;
        let bit = bas.checked_add(boy).ok_or(t!("parçalı cevap bozuk", "chunked response corrupt"))?;
        out.extend_from_slice(v.get(bas..bit).ok_or(t!("parçalı cevap eksik", "chunked response incomplete"))?);
        // Her parçanın ardından CRLF gelmeli; gelmediyse cevap kesik
        if v.get(bit..bit + 2) != Some(b"\r\n".as_slice()) {
            return Err(t!("parçalı cevap eksik (bağlantı parçanın ortasında kapandı)", "chunked response incomplete (the connection closed in the middle of a chunk)").into());
        }
        i = bit + 2;
    }
}
