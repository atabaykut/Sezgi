//! Canlı mod HTTPS: hedef `https://` olabilir; sertifika işletim sisteminin güven deposundan ya da
//! `SEZGI_CA_DOSYASI` ile verilen yetkiliden doğrulanır. Doğrulanamayan sertifikaya istek gitmez (atlama seçeneği yok).
//! Sertifikalar test çalışırken üretilir; depoda özel anahtar tutulmaz.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// (yetkili PEM, sunucu ayarı)
fn sertifikalar() -> (String, Arc<rustls::ServerConfig>) {
    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
    let yetkili_anahtar = KeyPair::generate().unwrap();
    let mut yetkili = CertificateParams::new(vec![]).unwrap();
    yetkili.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    yetkili.distinguished_name.push(DnType::CommonName, "Sezgi Test CA");
    let yetkili_sertifika = yetkili.self_signed(&yetkili_anahtar).unwrap();
    let veren = Issuer::new(yetkili, yetkili_anahtar);
    let sunucu_anahtar = KeyPair::generate().unwrap();
    let sunucu = CertificateParams::new(vec!["localhost".to_string()]).unwrap().signed_by(&sunucu_anahtar, &veren).unwrap();
    let ayar = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![sunucu.der().clone()], rustls::pki_types::PrivateKeyDer::try_from(sunucu_anahtar.serialize_der()).unwrap())
        .unwrap();
    (yetkili_sertifika.pem(), Arc::new(ayar))
}

/// İstek satırlarını kaydeden HTTPS sunucusu
fn https_sunucu(ayar: Arc<rustls::ServerConfig>) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let gelenler = Arc::new(Mutex::new(vec![]));
    let g2 = gelenler.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let Ok(baglanti) = rustls::ServerConnection::new(ayar.clone()) else { continue };
            let mut akis = rustls::StreamOwned::new(baglanti, s);
            let mut r = BufReader::new(&mut akis);
            let mut ilk = String::new();
            if r.read_line(&mut ilk).is_err() {
                continue; // el sıkışma başarısız (istemci sertifikayı reddetti)
            }
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).is_err() || l.trim().is_empty() {
                    break;
                }
            }
            g2.lock().unwrap().push(ilk.trim().to_string());
            let _ = write!(akis, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]");
            akis.conn.send_close_notify();
            let _ = akis.flush();
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

#[test]
fn https_hedefe_dogrulanmis_sertifikayla_gider() {
    let kok = std::env::temp_dir().join(format!("sezgi-https-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    let ev = kok.join("ev");
    std::fs::create_dir_all(ev.join("ayar")).unwrap();
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &kok.join("app"));
    let komut = |args: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args)
            .current_dir(kok.join("app"))
            .env("HOME", &ev)
            .env("USERPROFILE", &ev)
            .env("APPDATA", ev.join("ayar"))
            .env("XDG_CONFIG_HOME", ev.join("ayar"))
            .env_remove("SEZGI_TOKEN")
            .env_remove("SEZGI_CA_DOSYASI");
        c
    };
    let ok = |args: &[&str]| {
        let c = komut(args).output().unwrap();
        let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
        assert!(c.status.success(), "{args:?}: {m}");
        m
    };
    ok(&["init", "."]);
    let (yetkili, ayar) = sertifikalar();
    let ca = kok.join("ca.pem");
    std::fs::write(&ca, yetkili).unwrap();
    let (port, gelenler) = https_sunucu(ayar);
    ok(&["hedef", &format!("https://localhost:{port}"), "--test-kopyasi"]);
    ok(&["mod", "okuma"]);
    let token = ok(&["token", "olustur", "--ad", "t", "--izin", "kitap.listele:oku"]).split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
    let cagir = |ca: Option<&Path>| -> String {
        let mut c = komut(&["mcp", ".sezgi"]);
        c.env("SEZGI_TOKEN", &token).stdin(Stdio::piped()).stdout(Stdio::piped());
        if let Some(ca) = ca {
            c.env("SEZGI_CA_DOSYASI", ca);
        }
        let mut p = c.spawn().unwrap();
        let istek = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "cagir", "arguments": { "kapi": "GET /api/v1/kitaplar" } } });
        writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
        let mut c = String::new();
        p.stdout.take().unwrap().read_to_string(&mut c).unwrap();
        let _ = p.wait();
        let v: serde_json::Value = serde_json::from_str(c.lines().next().unwrap()).unwrap();
        v["result"]["content"][0]["text"].as_str().unwrap().to_string()
    };
    // Yetkili tanıtılmadan: sertifika doğrulanamaz, istek gitmez
    let m = cagir(None);
    assert!(!m.contains("OKUNDU") && (m.contains("TLS") || m.contains("sertifika") || m.contains("certificate")), "{m}");
    assert!(gelenler.lock().unwrap().is_empty(), "doğrulanmamış sunucuya istek gitmemeli");
    // Test yetkilisiyle: doğrulanır, istek şifreli kanaldan gider
    let m = cagir(Some(&ca));
    assert!(m.contains("OKUNDU") && m.contains("HTTP 200"), "{m}");
    assert_eq!(gelenler.lock().unwrap().as_slice(), ["GET /api/v1/kitaplar HTTP/1.1"]);
    let _ = std::fs::remove_dir_all(&kok);
}
