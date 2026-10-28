//! S3 Doğrulanmış insan rolleri: rol isteyen yazma, insanın doğrulanmış rolleri kapının rollerinden birini içeriyorsa
//! (ve öteki şartlar sağlanıyorsa) onaysız gider; rol eşleşmiyorsa önizleme 403 diye reddeder ve istek iletilmez; rol
//! bilinmiyorsa onaya düşer. Roller ajandan değil insandan gelir: elle oturumda açık beyan (`--rol`), OIDC'de imzası
//! doğrulanmış kimlik belirtecinin claim'leri, ya da uygulamanın "ben" ucundan insanın oturumuyla okunan roller.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Uygulama: GET /api/me insanın rollerini döner; öteki her isteğe 201. Döner: (port, /api/me dışındaki istek sayısı)
fn uygulama(roller: &'static str) -> (u16, Arc<AtomicUsize>) {
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
            let (kod, cevap) = if ilk.starts_with("GET /api/me ") {
                (200, format!("{{\"kullanici\":{{\"roller\":{roller}}}}}"))
            } else {
                s2.fetch_add(1, Ordering::SeqCst);
                (201, "{\"id\":\"1\"}".to_string())
            };
            let mut s = s;
            let _ = write!(s, "HTTP/1.1 {kod} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cevap}", cevap.len());
        }
    });
    (port, sayac)
}

fn kur(ad: &str, roller: &'static str) -> (Ev, Proje, u16, Arc<AtomicUsize>, String) {
    let ev = Ev::yeni(ad);
    let p = ev.proje("s3_rol", "a");
    let (port, sayac) = uygulama(roller);
    p.ok(&["hedef", &format!("http://127.0.0.1:{port}"), "--test-kopyasi"]);
    p.ok(&["mod", "yazma"]);
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    (ev, p, port, sayac, t)
}

fn oturum(p: &Proje, ek: &[&str]) -> String {
    let mut a = vec!["oturum", "ac"];
    a.extend(ek);
    let (ok, m) = p.sahte_terminalde(&a, "OTURUM-ALICE\n");
    assert!(ok, "{m}");
    m
}

#[test]
fn kart_rol_isteyen_yazmada_onay_ister() {
    let ev = Ev::yeni("s3-kart");
    let p = ev.proje("s3_rol", "a");
    let kart = p.kart();
    let yorum = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "POST" && o["entry"]["path"] == "/api/v1/yorumlar").unwrap().clone();
    assert_eq!(yorum["roles"], serde_json::json!(["EDITOR"]), "{yorum}");
    assert_eq!(yorum["requires_approval"], true, "statik kısım değişmez: rol isteyen yazma kartta onay ister");
}

#[test]
fn rol_bilinmiyorsa_onay_ajan_beyani_sayilmaz() {
    let (_ev, p, _port, sayac, t) = kur("s3-bilinmiyor", "[]");
    oturum(&p, &[]);
    let mut m = p.mcp(&t);
    let mut a = yorum_yaz();
    a["roller"] = serde_json::json!(["EDITOR"]);
    let (s, hata) = m.cagir(a);
    assert!(!hata && s.contains("ONAY BEKLİYOR") && s.contains("rol"), "ajanın rol beyanı doğrulanmış sayılmaz: {s}");
    assert_eq!(sayac.load(Ordering::SeqCst), 0);
}

#[test]
fn insanin_beyan_ettigi_rol_eslesirse_onaysiz_eslesmezse_ret() {
    let (_ev, p, _port, sayac, t) = kur("s3-beyan", "[]");
    let m = oturum(&p, &["--rol", "EDITOR"]);
    assert!(m.contains("EDITOR"), "{m}");
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(sayac.load(Ordering::SeqCst), 1);
    // Kayıt: rol beyanı insanın kimliğiyle
    let k = p.ok(&["kayit", "--json"]);
    assert!(k.contains("insan_beyani") && k.contains("EDITOR"), "{k}");
    // Rol eşleşmiyor: önizleme 403 diye reddeder, iletilmez
    oturum(&p, &["--rol", "OKUR"]);
    let mut a = yorum_yaz();
    a["tekrar_anahtari"] = serde_json::json!("ikinci");
    let (s, hata) = p.mcp(&t).cagir(a);
    assert!(hata && s.contains("403") && s.contains("İLETİLMEDİ"), "{s}");
    assert_eq!(sayac.load(Ordering::SeqCst), 1);
}

#[test]
fn rol_ucundan_okunan_rol_kullanilir() {
    let (_ev, p, _port, sayac, t) = kur("s3-rol-ucu", "[\"EDITOR\"]");
    let m = oturum(&p, &["--rol-ucu", "/api/me", "--rol-yolu", "kullanici.roller"]);
    assert!(m.contains("EDITOR"), "{m}");
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(sayac.load(Ordering::SeqCst), 1);
}

// ---- OIDC: imzası doğrulanmış kimlik belirteci ----

fn b64(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

struct Anahtar {
    cift: ring::signature::EcdsaKeyPair,
    rng: ring::rand::SystemRandom,
}

impl Anahtar {
    fn yeni() -> Anahtar {
        use ring::signature::{EcdsaKeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let cift = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng).unwrap();
        Anahtar { cift, rng }
    }
    fn jwks(&self) -> serde_json::Value {
        use ring::signature::KeyPair;
        let n = self.cift.public_key().as_ref();
        serde_json::json!({ "keys": [{ "kty": "EC", "crv": "P-256", "kid": "k1", "alg": "ES256", "x": b64(&n[1..33]), "y": b64(&n[33..65]) }] })
    }
    fn imzala(&self, govde: &serde_json::Value) -> String {
        let baslik = b64(br#"{"alg":"ES256","kid":"k1","typ":"JWT"}"#);
        let girdi = format!("{baslik}.{}", b64(govde.to_string().as_bytes()));
        let imza = self.cift.sign(&self.rng, girdi.as_bytes()).unwrap();
        format!("{girdi}.{}", b64(imza.as_ref()))
    }
}

/// Cihaz akışını hemen onaylayan sağlayıcı; verilen kimlik belirtecini döner
fn saglayici(kimlik: impl Fn(&str) -> String + Send + 'static, jwks: serde_json::Value) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let kok = format!("http://127.0.0.1:{port}");
    let k2 = kok.clone();
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
            let govde = String::from_utf8_lossy(&g).to_string();
            let cevap = if ilk.contains("/.well-known/openid-configuration") {
                serde_json::json!({ "issuer": k2, "token_endpoint": format!("{k2}/token"), "device_authorization_endpoint": format!("{k2}/cihaz"), "jwks_uri": format!("{k2}/jwks") })
            } else if ilk.contains("/jwks") {
                jwks.clone()
            } else if ilk.contains("/cihaz") {
                serde_json::json!({ "device_code": "dc", "user_code": "ABCD", "verification_uri": format!("{k2}/e"), "interval": 0, "expires_in": 60 })
            } else if govde.contains("device_code") {
                serde_json::json!({ "access_token": "erisim-1", "expires_in": 3600, "id_token": kimlik(&k2) })
            } else {
                serde_json::json!({ "error": "invalid_grant" })
            };
            let c = cevap.to_string();
            let mut s = s;
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{c}", c.len());
        }
    });
    kok
}

/// Sağlayıcının verdiği kimlik belirtecini üretir (issuer alır)
type Uretici = Box<dyn Fn(&str) -> String + Send>;

fn simdi() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

#[test]
fn oidc_imzali_kimlik_belirtecindeki_rol_kullanilir() {
    let (_ev, p, _port, sayac, t) = kur("s3-oidc", "[]");
    let a = Arc::new(Anahtar::yeni());
    let a2 = a.clone();
    let issuer = saglayici(
        move |iss| a2.imzala(&serde_json::json!({ "iss": iss, "aud": "sezgi-deneme", "exp": simdi() + 3600, "email": "oidc@ornek.test", "realm_access": { "roles": ["EDITOR"] } })),
        a.jwks(),
    );
    let c = p.sezgi(&["oturum", "ac", "--oidc", &issuer, "--istemci", "sezgi-deneme"], None);
    let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(c.status.success() && m.contains("EDITOR"), "{m}");
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(sayac.load(Ordering::SeqCst), 1);
}

#[test]
fn oidc_imzasiz_ya_da_baska_kitleye_ait_belirtecin_rolu_kullanilmaz() {
    let a = Arc::new(Anahtar::yeni());
    // alg none (imzasız), başka hedef kitle, imzası bozuk
    let durumlar: Vec<Uretici> = vec![
        Box::new(|iss: &str| {
            format!(
                "{}.{}.",
                b64(br#"{"alg":"none"}"#),
                b64(serde_json::json!({ "iss": iss, "aud": "sezgi-deneme", "exp": simdi() + 3600, "roles": ["EDITOR"] }).to_string().as_bytes())
            )
        }),
        {
            let a = a.clone();
            Box::new(move |iss: &str| a.imzala(&serde_json::json!({ "iss": iss, "aud": "baska-istemci", "exp": simdi() + 3600, "roles": ["EDITOR"] })))
        },
        {
            let a = a.clone();
            Box::new(move |iss: &str| {
                let j = a.imzala(&serde_json::json!({ "iss": iss, "aud": "sezgi-deneme", "exp": simdi() + 3600, "roles": ["OKUR"] }));
                let (bas, imza) = j.rsplit_once('.').unwrap();
                let govde = b64(serde_json::json!({ "iss": iss, "aud": "sezgi-deneme", "exp": simdi() + 3600, "roles": ["EDITOR"] }).to_string().as_bytes());
                format!("{}.{govde}.{imza}", bas.split('.').next().unwrap())
            })
        },
    ];
    for (i, kimlik) in durumlar.into_iter().enumerate() {
        let (_ev, p, _port, sayac, t) = kur(&format!("s3-oidc-ret-{i}"), "[]");
        let issuer = saglayici(kimlik, a.jwks());
        let c = p.sezgi(&["oturum", "ac", "--oidc", &issuer, "--istemci", "sezgi-deneme"], None);
        assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
        let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
        assert!(!hata && s.contains("ONAY BEKLİYOR"), "durum {i}: doğrulanmamış belirtecin rolü kullanılmamalı: {s}");
        assert_eq!(sayac.load(Ordering::SeqCst), 0);
    }
}
