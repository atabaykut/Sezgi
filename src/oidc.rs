//! İnsan oturumunu uygulamanın kimlik sağlayıcısından almak (OIDC): keşif belgesi, cihaz yetkilendirme akışı
//! (RFC 8628) ve yenileme belirteciyle süre uzatma (RFC 6749 §6).
//!
//! Kurallar:
//! - İnsan, giriş sayfasını kendi tarayıcısında açar ve kendi kimliğiyle onaylar; Sezgi parolayı hiç görmez.
//! - Sağlayıcı `https://` olmalıdır; yalnızca bu bilgisayardaki (127.0.0.1 / localhost) bir deneme sağlayıcısı
//!   `http://` olabilir. Keşif belgesindeki `issuer` verilen adresle aynı olmalıdır.
//! - Erişim ve yenileme belirteçleri işletim sistemi anahtarlığında tutulur (`canli` oturumuyla aynı yer).
//! - Oturumun toplam süresi `--sure` ile sınırlıdır (en çok 24 saat); yenileme bu sınırı aşmaz.
use serde_json::Value;

use crate::http;

/// Sağlayıcı adresi kabul edilir mi: https ya da bu bilgisayardaki bir deneme sağlayıcısı
fn guvenli_adres(url: &str) -> Result<http::Adres, String> {
    let a = http::adres(url)?;
    if !a.tls && !matches!(a.ana.as_str(), "127.0.0.1" | "localhost") {
        return Err(tf!("kimlik sağlayıcısı https:// olmalı ({url})", "the identity provider must use https:// ({url})"));
    }
    Ok(a)
}

fn json_al(url: &str) -> Result<Value, String> {
    let a = guvenli_adres(url)?;
    let c = http::istek(&a, "GET", if a.taban.is_empty() { "/" } else { "" }, &[("Accept", "application/json".into())], None)?;
    if c.durum != 200 {
        return Err(format!("{url}: HTTP {}", c.durum));
    }
    serde_json::from_slice(&c.govde).map_err(|e| tf!("{url}: JSON değil ({e})", "{url}: not JSON ({e})"))
}

/// Form gövdesiyle POST; cevap JSON (hata cevapları da JSON gövdeli olabilir: RFC 6749 §5.2)
fn form_gonder(url: &str, alanlar: &[(&str, &str)]) -> Result<(u16, Value), String> {
    let a = guvenli_adres(url)?;
    let govde: String = alanlar.iter().map(|(k, v)| format!("{}={}", http::kodla(k), http::kodla(v))).collect::<Vec<_>>().join("&");
    let c = http::istek(
        &a,
        "POST",
        if a.taban.is_empty() { "/" } else { "" },
        &[("Content-Type", "application/x-www-form-urlencoded".into()), ("Accept", "application/json".into())],
        Some(govde.as_bytes()),
    )?;
    let v = serde_json::from_slice(&c.govde).unwrap_or(Value::Null);
    Ok((c.durum, v))
}

/// Keşif belgesi: `<issuer>/.well-known/openid-configuration`
pub fn kesif(issuer: &str) -> Result<Value, String> {
    let issuer = issuer.trim_end_matches('/');
    let m = json_al(&format!("{issuer}/.well-known/openid-configuration"))?;
    if m["issuer"].as_str().map(|s| s.trim_end_matches('/')) != Some(issuer) {
        return Err(tf!("keşif belgesindeki issuer ({}) verilen adresle aynı değil", "the issuer in the discovery document ({}) differs from the given address", m["issuer"]));
    }
    for alan in ["token_endpoint", "device_authorization_endpoint"] {
        let u = m[alan].as_str().ok_or_else(|| {
            tf!("sağlayıcı cihaz akışını desteklemiyor: keşif belgesinde {alan} yok", "the provider does not support the device flow: no {alan} in the discovery document")
        })?;
        guvenli_adres(u)?;
    }
    Ok(m)
}

/// Alınan belirteçler
pub struct Belirtecler {
    pub erisim: String,
    pub yenileme: Option<String>,
    /// Erişim belirtecinin kalan ömrü (saniye; sağlayıcı söylemezse 1 saat sayılır)
    pub omur: u64,
    pub kimlik_belirteci: Option<String>,
}

fn belirtecler(v: &Value) -> Result<Belirtecler, String> {
    Ok(Belirtecler {
        erisim: v["access_token"]
            .as_str()
            .filter(|s| !s.is_empty() && !s.contains(char::is_whitespace))
            .ok_or(t!("cevapta access_token yok", "no access_token in the response"))?
            .to_string(),
        yenileme: v["refresh_token"].as_str().map(String::from),
        omur: v["expires_in"].as_u64().unwrap_or(3600),
        kimlik_belirteci: v["id_token"].as_str().map(String::from),
    })
}

/// Cihaz akışı: kullanıcıya adresi ve kodu gösterir, onay gelene kadar belirteç ucunu yoklar
pub fn cihaz_akisi(m: &Value, istemci: &str, kapsam: &str, goster: &mut dyn FnMut(&str, &str, Option<&str>)) -> Result<Belirtecler, String> {
    let (durum, c) = form_gonder(m["device_authorization_endpoint"].as_str().unwrap_or_default(), &[("client_id", istemci), ("scope", kapsam)])?;
    if durum != 200 {
        return Err(tf!("cihaz yetkilendirmesi reddedildi: HTTP {durum} {}", "device authorization refused: HTTP {durum} {}", c["error"].as_str().unwrap_or("")));
    }
    let cihaz_kodu = c["device_code"].as_str().ok_or(t!("cevapta device_code yok", "no device_code in the response"))?;
    let kullanici_kodu = c["user_code"].as_str().ok_or(t!("cevapta user_code yok", "no user_code in the response"))?;
    let adres = c["verification_uri"].as_str().ok_or(t!("cevapta verification_uri yok", "no verification_uri in the response"))?;
    goster(adres, kullanici_kodu, c["verification_uri_complete"].as_str());
    let mut aralik = c["interval"].as_u64().unwrap_or(5);
    let son = std::time::Instant::now() + std::time::Duration::from_secs(c["expires_in"].as_u64().unwrap_or(600).min(1800));
    let belirtec_ucu = m["token_endpoint"].as_str().unwrap_or_default();
    loop {
        if std::time::Instant::now() >= son {
            return Err(t!("kodun süresi doldu; yeniden deneyin", "the code expired; try again").into());
        }
        std::thread::sleep(std::time::Duration::from_secs(aralik));
        let (durum, v) = form_gonder(belirtec_ucu, &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("device_code", cihaz_kodu), ("client_id", istemci)])?;
        if durum == 200 {
            return belirtecler(&v);
        }
        match v["error"].as_str() {
            Some("authorization_pending") => {}
            Some("slow_down") => aralik += 5,
            Some("access_denied") => return Err(t!("giriş reddedildi", "sign-in refused").into()),
            Some("expired_token") => return Err(t!("kodun süresi doldu; yeniden deneyin", "the code expired; try again").into()),
            e => return Err(tf!("belirteç alınamadı: HTTP {durum} {}", "could not get a token: HTTP {durum} {}", e.unwrap_or(""))),
        }
    }
}

/// Yenileme belirteciyle yeni erişim belirteci (sağlayıcı yeni bir yenileme belirteci verirse o kullanılır)
pub fn yenile(belirtec_ucu: &str, istemci: &str, yenileme: &str) -> Result<Belirtecler, String> {
    let (durum, v) = form_gonder(belirtec_ucu, &[("grant_type", "refresh_token"), ("refresh_token", yenileme), ("client_id", istemci)])?;
    if durum != 200 {
        return Err(tf!("oturum yenilenemedi: HTTP {durum} {}", "could not refresh the session: HTTP {durum} {}", v["error"].as_str().unwrap_or("")));
    }
    let mut b = belirtecler(&v)?;
    if b.yenileme.is_none() {
        b.yenileme = Some(yenileme.to_string());
    }
    Ok(b)
}

/// Kimlik belirtecini (ID token) doğrular ve claim'lerini döner. Rol gibi yetki kararlarına dayanak olacağı için:
/// - imza zorunlu: `alg` RS256 ya da ES256; anahtar sağlayıcının `jwks_uri` adresinden (https, ya da bu bilgisayardaki
///   deneme sağlayıcısı) `kid` ile seçilir. `none` ve imzası tutmayan belirteç reddedilir;
/// - `iss` keşifteki sağlayıcı, `aud` bu istemci (dizi de olabilir), `exp` geçmemiş (60 sn tolerans).
pub fn kimlik_belirteci_dogrula(m: &Value, istemci: &str, jwt: &str) -> Result<Value, String> {
    use base64::Engine;
    let b64 = |s: &str| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s.trim_end_matches('='))
            .map_err(|_| t!("kimlik belirteci bozuk (base64)", "ID token is corrupt (base64)").to_string())
    };
    let mut p = jwt.split('.');
    let (Some(bas), Some(govde), Some(imza), None) = (p.next(), p.next(), p.next(), p.next()) else {
        return Err(t!("kimlik belirteci üç parçalı değil", "ID token does not have three parts").into());
    };
    let baslik: Value = serde_json::from_slice(&b64(bas)?).map_err(|_| t!("kimlik belirteci başlığı okunamadı", "could not read the ID token header"))?;
    let alg = baslik["alg"].as_str().unwrap_or("");
    if !matches!(alg, "RS256" | "ES256") {
        return Err(tf!("kimlik belirteci imzalı değil ya da algoritması desteklenmiyor ({alg})", "ID token is unsigned or its algorithm is not supported ({alg})"));
    }
    let jwks =
        json_al(m["jwks_uri"].as_str().ok_or(t!("keşif belgesinde jwks_uri yok: imza doğrulanamaz", "no jwks_uri in the discovery document: the signature cannot be verified"))?)?;
    let kid = baslik["kid"].as_str();
    let anahtar = jwks["keys"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|k| kid.is_none_or(|i| k["kid"].as_str() == Some(i)) && k["kty"].as_str() == Some(if alg == "RS256" { "RSA" } else { "EC" }))
        .ok_or(t!("imzalayan anahtar sağlayıcının anahtar listesinde yok", "the signing key is not in the provider's key list"))?;
    let girdi = format!("{bas}.{govde}");
    let imza = b64(imza)?;
    let tamam = if alg == "RS256" {
        let (n, e) = (b64(anahtar["n"].as_str().unwrap_or(""))?, b64(anahtar["e"].as_str().unwrap_or(""))?);
        ring::signature::RsaPublicKeyComponents { n: &n, e: &e }.verify(&ring::signature::RSA_PKCS1_2048_8192_SHA256, girdi.as_bytes(), &imza).is_ok()
    } else {
        if anahtar["crv"].as_str() != Some("P-256") {
            return Err(t!("EC anahtarının eğrisi P-256 değil", "the EC key's curve is not P-256").into());
        }
        let mut nokta = vec![4u8];
        nokta.extend(b64(anahtar["x"].as_str().unwrap_or(""))?);
        nokta.extend(b64(anahtar["y"].as_str().unwrap_or(""))?);
        ring::signature::UnparsedPublicKey::new(&ring::signature::ECDSA_P256_SHA256_FIXED, nokta).verify(girdi.as_bytes(), &imza).is_ok()
    };
    if !tamam {
        return Err(t!("kimlik belirtecinin imzası tutmuyor", "the ID token's signature does not match").into());
    }
    let c: Value = serde_json::from_slice(&b64(govde)?).map_err(|_| t!("kimlik belirteci gövdesi okunamadı", "could not read the ID token body"))?;
    let iss = m["issuer"].as_str().unwrap_or("").trim_end_matches('/');
    if c["iss"].as_str().map(|s| s.trim_end_matches('/')) != Some(iss) {
        return Err(t!("kimlik belirtecinin iss değeri sağlayıcıyla aynı değil", "the ID token's iss differs from the provider").into());
    }
    let kitle = match &c["aud"] {
        Value::String(a) => a == istemci,
        Value::Array(a) => a.iter().any(|x| x.as_str() == Some(istemci)),
        _ => false,
    };
    if !kitle {
        return Err(t!("kimlik belirteci bu istemci için verilmemiş (aud)", "the ID token was not issued for this client (aud)").into());
    }
    let simdi = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if c["exp"].as_u64().is_none_or(|e| e + 60 < simdi) {
        return Err(t!("kimlik belirtecinin süresi dolmuş", "the ID token has expired").into());
    }
    Ok(c)
}

/// Belirteç değişimi (RFC 8693): insanın erişim belirteci, yalnızca hedef uygulama (`audience`) için geçerli daha dar
/// bir belirteçle değiştirilir. Sağlayıcı desteklemiyorsa hata döner; Sezgi geniş belirteçle devam etmez.
pub fn degistir(belirtec_ucu: &str, istemci: &str, konu: &str, hedef_kitle: &str, kapsam: Option<&str>) -> Result<Belirtecler, String> {
    let mut alanlar = vec![
        ("grant_type", "urn:ietf:params:oauth:grant-type:token-exchange"),
        ("subject_token", konu),
        ("subject_token_type", "urn:ietf:params:oauth:token-type:access_token"),
        ("requested_token_type", "urn:ietf:params:oauth:token-type:access_token"),
        ("audience", hedef_kitle),
        ("client_id", istemci),
    ];
    if let Some(k) = kapsam {
        alanlar.push(("scope", k));
    }
    let (durum, v) = form_gonder(belirtec_ucu, &alanlar)?;
    if durum != 200 {
        return Err(tf!("belirteç değişimi reddedildi: HTTP {durum} {}", "token exchange refused: HTTP {durum} {}", v["error"].as_str().unwrap_or("")));
    }
    let mut b = belirtecler(&v)?;
    // Değiştirilen belirtecin yenilemesi yoktur: süre dolunca asıl yenileme belirteciyle yeniden alınıp yeniden değiştirilir
    b.yenileme = None;
    Ok(b)
}
