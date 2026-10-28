//! Benlik Kartı imzası: ML-DSA-65 (NIST FIPS 204, kuantuma dayanıklı).
//!
//! İmzalanan bayt dizisi: kartın `signature` üyesi çıkarılmış hâlinin kanonik JSON'u
//! (RFC 8785 JCS: anahtarlar UTF-16 sırasıyla, boşluksuz, ECMAScript sayı biçimi).
//! Bağlam dizgisi (FIPS 204 context) `BAGLAM`: aynı anahtarın başka bir mesaj türü için verdiği imza karta taşınamaz.
//! Gizli anahtar proje klasörüne asla yazılmaz; kullanıcının yapılandırma klasöründe durur.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ml_dsa::signature::Keypair;
use ml_dsa::{EncodedSignature, EncodedVerifyingKey, MlDsa65, Seed, Signature, SigningKey, VerifyingKey};
use serde_json::{json, Value};
use std::path::PathBuf;

pub const ALG: &str = "ML-DSA-65";
/// ML-DSA imza bağlamı (alan ayrımı). Sürüm içermez: kartın sürümü zaten imzalanan gövdede (`benlik`).
pub const BAGLAM: &[u8] = b"benlik-card";
/// Paylaşılan harita dosyası (`sezgi paylas`) için ayrı bağlam: paylaşım imzası kart imzası yerine geçemez, tersi de
pub const PAYLASIM_BAGLAMI: &[u8] = b"sezgi-paylasim";
/// Kanıt paketi (benlik-evidence.json) için ayrı bağlam: paket imzası kart imzası yerine geçemez, tersi de
pub const KANIT_BAGLAMI: &[u8] = b"benlik-evidence";

/// RFC 8785 (JCS) kanonik biçim: anahtarlar UTF-16 kod birimlerine göre sıralı, boşluksuz,
/// sayılar ECMAScript `Number.prototype.toString` kuralıyla (ör. 0.0 -> "0", 1e30 -> "1e+30").
pub fn kanonik(v: &Value) -> String {
    let mut s = String::new();
    yaz(v, &mut s);
    s
}

fn yaz(v: &Value, s: &mut String) {
    match v {
        Value::Object(m) => {
            let mut anahtarlar: Vec<&String> = m.keys().collect();
            anahtarlar.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            s.push('{');
            for (i, k) in anahtarlar.into_iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&serde_json::to_string(k).unwrap());
                s.push(':');
                yaz(&m[k], s);
            }
            s.push('}');
        }
        Value::Array(a) => {
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                yaz(x, s);
            }
            s.push(']');
        }
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            // 2^53 üstü tamsayılar JS'te kesirli sayı gibi yazılır
            (Some(i), _, _) if i.unsigned_abs() > 1 << 53 => s.push_str(&es_sayi(i as f64)),
            (_, Some(u), _) if u > 1 << 53 => s.push_str(&es_sayi(u as f64)),
            (Some(i), _, _) => s.push_str(&i.to_string()),
            (_, Some(u), _) => s.push_str(&u.to_string()),
            (_, _, Some(f)) => s.push_str(&es_sayi(f)),
            _ => s.push_str(&n.to_string()),
        },
        // Metin kaçışı (serde_json): yalnızca tırnak, ters bölü ve kontrol karakterleri; küçük harfli \u00xx. JCS ile aynı.
        _ => s.push_str(&serde_json::to_string(v).unwrap()),
    }
}

/// ECMAScript sayı biçimi (RFC 8785 bölüm 3.2.2.3). JSON'da NaN ve sonsuz olmaz.
fn es_sayi(f: f64) -> String {
    if f == 0.0 {
        return "0".into();
    }
    let isaret = if f < 0.0 { "-" } else { "" };
    // Rust'ın `{:e}` biçimi en kısa geri dönüşlü basamakları verir: "1.5e300", "1e-7"
    let us = format!("{:e}", f.abs());
    let (mantis, us) = us.split_once('e').unwrap();
    let basamak: String = mantis.chars().filter(|c| *c != '.').collect();
    let (k, n) = (basamak.len() as i32, us.parse::<i32>().unwrap() + 1);
    let govde = if k <= n && n <= 21 {
        format!("{basamak}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &basamak[..n as usize], &basamak[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{basamak}", "0".repeat((-n) as usize))
    } else {
        let us = n - 1;
        let us_isaret = if us >= 0 { "+" } else { "-" };
        let kalan = if k > 1 { format!(".{}", &basamak[1..]) } else { String::new() };
        format!("{}{kalan}e{us_isaret}{}", &basamak[..1], us.abs())
    };
    format!("{isaret}{govde}")
}

fn imzasiz(kart: &Value) -> Value {
    let mut k = kart.clone();
    if let Some(m) = k.as_object_mut() {
        m.remove("signature");
    }
    k
}

/// Kullanıcının Sezgi yapılandırma klasörü (APPDATA / XDG_CONFIG_HOME / ~/.config altında `sezgi`); yoksa None
pub fn yapilandirma_klasoru() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|t| t.join("sezgi"))
}

pub fn anahtar_klasoru() -> PathBuf {
    yapilandirma_klasoru()
        .unwrap_or_else(|| {
            // Çalışma klasörüne (belki bir projeye) gizli anahtar yazmaktansa dur
            te!("✗ Yapılandırma klasörü bulunamadı (APPDATA / XDG_CONFIG_HOME / HOME tanımlı değil).", "✗ Config folder not found (APPDATA / XDG_CONFIG_HOME / HOME is not set).");
            std::process::exit(1)
        })
        .join("anahtarlar")
}

// ---- Güven çapası ----
// Kartı, yanında gelen anahtarla doğrulamak yalnızca bütünlüğü kanıtlar: kartı ve anahtarı birlikte getiren biri de
// tutarlı imza üretebilir. Kimlik için kullanıcının kendi yapılandırmasında bir çapa gerekir:
// - sabitlenmiş anahtarlar (guvenilen-anahtarlar.json): kullanıcı `sezgi anahtar sabitle` ile bir uygulama kimliği için
//   açık anahtarı açıkça sabitler; yeşil "güvenilir" yalnızca bu anahtarla eşleşen imzaya verilir;
// - görülen anahtarlar (gorulen-anahtarlar.json, ilk kullanımda güven): `sezgi dogrula` bir uygulama için ilk gördüğü
//   anahtarı kaydeder, sonra başka anahtar gelirse "anahtar değişti" uyarır. İlk karşılaşmayı korumaz.

const SABIT_DOSYA: &str = "guvenilen-anahtarlar.json";
const GORULEN_DOSYA: &str = "gorulen-anahtarlar.json";

/// İmza doğrulandıktan sonra imzalayan anahtarın kullanıcının güven çapasına göre durumu
#[derive(Debug, PartialEq)]
pub enum Guven {
    /// Uygulama için sabitlenmiş bir anahtarla eşleşiyor
    Sabit,
    /// Uygulama için anahtar sabitlenmiş ama imzalayan anahtar onlardan biri değil
    SabitUyusmaz,
    /// Sabitleme yok; daha önce bu uygulama için başka bir anahtar görülmüş
    Degisti,
    /// Ne sabitleme ne farklı bir görülmüş anahtar: tutarlı ama kimlik doğrulanmadı
    Bilinmiyor,
}

fn guven_oku(ad: &str) -> Value {
    yapilandirma_klasoru().and_then(|k| std::fs::read_to_string(k.join(ad)).ok()).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_else(|| json!({}))
}

fn guven_yaz(ad: &str, v: &Value) -> Result<(), String> {
    let k = yapilandirma_klasoru().ok_or(t!("yapılandırma klasörü bulunamadı (APPDATA / XDG_CONFIG_HOME / HOME)", "config folder not found (APPDATA / XDG_CONFIG_HOME / HOME)"))?;
    klasor_olustur(&k).map_err(|e| tf!("yapılandırma klasörü oluşturulamadı: {e}", "could not create the config folder: {e}"))?;
    let yol = k.join(ad);
    if std::fs::symlink_metadata(&yol).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        return Err(tf!("{} sembolik bağ, yazılmadı", "{} is a symbolic link, not written", yol.display()));
    }
    std::fs::write(&yol, serde_json::to_string_pretty(v).unwrap()).map_err(|e| tf!("{} yazılamadı: {e}", "could not write {}: {e}", yol.display()))
}

/// Kartı imzalayan anahtarın açık anahtar metni (dogrula'nın bulduğu key_id ile)
fn imzalayan(kart: &Value, anahtarlar: &Value) -> Option<String> {
    let kimlik = kart["signature"]["key_id"].as_str()?;
    anahtarlar["keys"].as_array()?.iter().find(|k| k["key_id"].as_str() == Some(kimlik))?["public_key"].as_str().map(String::from)
}

/// Doğrulanmış bir kartın güven durumu (dosyalara yazmaz)
pub fn guven_durumu(kart: &Value, anahtarlar: &Value) -> Guven {
    let (Some(uygulama), Some(acik)) = (kart["app"]["id"].as_str(), imzalayan(kart, anahtarlar)) else { return Guven::Bilinmiyor };
    let sabit = guven_oku(SABIT_DOSYA);
    if let Some(liste) = sabit[uygulama].as_array().filter(|l| !l.is_empty()) {
        return if liste.iter().any(|k| k["public_key"].as_str() == Some(acik.as_str())) { Guven::Sabit } else { Guven::SabitUyusmaz };
    }
    match guven_oku(GORULEN_DOSYA)[uygulama]["public_key"].as_str() {
        Some(g) if g != acik => Guven::Degisti,
        _ => Guven::Bilinmiyor,
    }
}

/// Uygulama için sabitlenmiş ya da daha önce görülmüş bir anahtar var mı: varsa kartı artık imzasız olamaz (imzalı
/// kartın imzasız bir kartla değiştirilmesi)
pub fn bilinen_anahtar(uygulama: &str) -> bool {
    guven_oku(SABIT_DOSYA)[uygulama].as_array().is_some_and(|l| !l.is_empty()) || guven_oku(GORULEN_DOSYA).get(uygulama).is_some()
}

/// İlk kullanımda güven: uygulama için anahtar ilk kez görülüyorsa kaydeder
pub fn gorulen_kaydet(kart: &Value, anahtarlar: &Value) -> Result<(), String> {
    let (Some(uygulama), Some(acik)) = (kart["app"]["id"].as_str(), imzalayan(kart, anahtarlar)) else { return Ok(()) };
    let mut gorulen = guven_oku(GORULEN_DOSYA);
    if gorulen.get(uygulama).is_none() {
        gorulen[uygulama] = json!({ "key_id": kart["signature"]["key_id"], "public_key": acik });
        guven_yaz(GORULEN_DOSYA, &gorulen)?;
    }
    Ok(())
}

/// Kartın imzasını doğrular ve imzalayan anahtarı kartın uygulama kimliği için sabitler; (uygulama, key_id) döner
pub fn anahtar_sabitle(kart: &Value, anahtarlar: &Value) -> Result<(String, String), String> {
    let kimlik = dogrula(kart, anahtarlar)?;
    let uygulama = kart["app"]["id"].as_str().filter(|u| !u.is_empty()).ok_or(t!("kartta app.id yok", "the card has no app.id"))?.to_string();
    let acik = imzalayan(kart, anahtarlar).ok_or(t!("imzalayan anahtar bulunamadı", "signing key not found"))?;
    let mut sabit = guven_oku(SABIT_DOSYA);
    let liste = sabit[uygulama.as_str()].as_array().cloned().unwrap_or_default();
    if !liste.iter().any(|k| k["public_key"].as_str() == Some(acik.as_str())) {
        let mut yeni = liste;
        yeni.push(json!({ "key_id": kimlik, "alg": ALG, "public_key": acik }));
        sabit[uygulama.as_str()] = json!(yeni);
        guven_yaz(SABIT_DOSYA, &sabit)?;
    }
    Ok((uygulama, kimlik))
}

/// Anahtar adı dosya adına dönüşür: yalnızca harf, rakam, '-', '_', '.'; '.' ile başlayamaz (yol geçişi olmasın)
pub fn kimlik_gecerli(kimlik: &str) -> Result<(), String> {
    let ok = !kimlik.is_empty() && kimlik.len() <= 64 && !kimlik.starts_with('.') && kimlik.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(tf!(
            "geçersiz anahtar adı '{kimlik}': yalnızca harf, rakam, '-', '_', '.' (en fazla 64, '.' ile başlamaz)",
            "invalid key name '{kimlik}': only letters, digits, '-', '_', '.' (at most 64, not starting with '.')"
        ))
    }
}

/// Gizli tohumu yalnızca sahibinin okuyabileceği biçimde, var olan dosyanın üstüne yazmadan oluşturur
/// Anahtar klasörü: Unix'te yalnızca sahibi girebilir (0700)
fn klasor_olustur(yol: &std::path::Path) -> std::io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(yol)
}

pub(crate) fn gizli_yaz(yol: &std::path::Path, icerik: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    // Windows'ta %APPDATA% kullanıcıya özel izinlerle korunur
    o.open(yol)?.write_all(icerik.as_bytes())
}

fn gizli_yolu(kimlik: &str) -> PathBuf {
    anahtar_klasoru().join(format!("{kimlik}.tohum"))
}

fn anahtar_yukle(kimlik: &str) -> Result<SigningKey<MlDsa65>, String> {
    kimlik_gecerli(kimlik)?;
    let metin = std::fs::read_to_string(gizli_yolu(kimlik))
        .map_err(|_| tf!("'{kimlik}' anahtarı bulunamadı. Önce: sezgi anahtar olustur --ad {kimlik}", "key '{kimlik}' not found. First: sezgi key create --name {kimlik}"))?;
    let bayt = B64.decode(metin.trim()).map_err(|_| t!("anahtar dosyası bozuk", "key file is corrupt").to_string())?;
    let dizi: [u8; 32] = bayt.try_into().map_err(|_| t!("anahtar dosyası bozuk", "key file is corrupt").to_string())?;
    Ok(SigningKey::<MlDsa65>::from_seed(&Seed::from(dizi)))
}

/// Açık anahtar belgesi: /.well-known/benlik-keys.json olarak yayınlanır.
pub fn acik_anahtar_belgesi(kimlik: &str, sk: &SigningKey<MlDsa65>) -> Value {
    json!({ "keys": [{ "key_id": kimlik, "alg": ALG, "public_key": B64.encode(sk.verifying_key().encode().as_slice()) }] })
}

pub fn anahtar_olustur(kimlik: &str) -> Result<(PathBuf, Value), String> {
    kimlik_gecerli(kimlik)?;
    let yol = gizli_yolu(kimlik);
    if yol.exists() {
        return Err(tf!("'{kimlik}' adlı anahtar zaten var: {}", "a key named '{kimlik}' already exists: {}", yol.display()));
    }
    let mut tohum = [0u8; 32];
    getrandom::fill(&mut tohum).map_err(|e| tf!("rastgele sayı üretilemedi: {e}", "could not generate random bytes: {e}"))?;
    klasor_olustur(&anahtar_klasoru()).map_err(|e| tf!("anahtar klasörü oluşturulamadı: {e}", "could not create the key folder: {e}"))?;
    gizli_yaz(&yol, &B64.encode(tohum)).map_err(|e| tf!("gizli anahtar yazılamadı: {e}", "could not write the secret key: {e}"))?;
    let sk = SigningKey::<MlDsa65>::from_seed(&Seed::from(tohum));
    let belge = acik_anahtar_belgesi(kimlik, &sk);
    let acik_yol = anahtar_klasoru().join(format!("{kimlik}.benlik-keys.json"));
    std::fs::write(&acik_yol, serde_json::to_string_pretty(&belge).unwrap()).map_err(|e| e.to_string())?;
    Ok((acik_yol, belge))
}

pub fn imzala(kart: &mut Value, kimlik: &str) -> Result<(), String> {
    imzala_baglamla(kart, kimlik, BAGLAM)
}

pub fn imzala_baglamla(kart: &mut Value, kimlik: &str, baglam: &[u8]) -> Result<(), String> {
    let sk = anahtar_yukle(kimlik)?;
    let mesaj = kanonik(&imzasiz(kart));
    let imza: Signature<MlDsa65> = sk.expanded_key().sign_deterministic(mesaj.as_bytes(), baglam).map_err(|e| tf!("imzalanamadı: {e}", "could not sign: {e}"))?;
    kart["signature"] = json!({
        "alg": ALG,
        "key_id": kimlik,
        "public_key_url": "/.well-known/benlik-keys.json",
        "value": B64.encode(imza.encode().as_slice()),
    });
    Ok(())
}

/// Bu kullanıcıda bu adla imza anahtarı var mı (yapılandırma klasörü yoksa yok sayılır, süreç durmaz)
pub fn anahtar_var(kimlik: &str) -> bool {
    kimlik_gecerli(kimlik).is_ok() && yapilandirma_klasoru().is_some_and(|k| k.join("anahtarlar").join(format!("{kimlik}.tohum")).is_file())
}

/// Kartı imzalamak için önerilecek komut: varsayılan anahtar yoksa önce anahtar oluşturma adımı
pub fn imzalama_ipucu(komut: &str) -> String {
    if anahtar_var("varsayilan") {
        format!("{komut} --imzala")
    } else {
        tf!("önce sezgi anahtar olustur, sonra {komut} --imzala", "first sezgi key create, then {komut} --sign")
    }
}

pub fn acik_anahtar_dosyasi(kimlik: &str) -> PathBuf {
    anahtar_klasoru().join(format!("{kimlik}.benlik-keys.json"))
}

/// Kartın imzasını açık anahtar belgesiyle doğrular.
pub fn dogrula(kart: &Value, anahtarlar: &Value) -> Result<String, String> {
    dogrula_baglamla(kart, anahtarlar, BAGLAM)
}

pub fn dogrula_baglamla(kart: &Value, anahtarlar: &Value, baglam: &[u8]) -> Result<String, String> {
    let s = &kart["signature"];
    if s["alg"].as_str() != Some(ALG) {
        return Err(tf!("desteklenmeyen ya da eksik imza algoritması: {}", "unsupported or missing signature algorithm: {}", s["alg"]));
    }
    let kimlik = s["key_id"].as_str().unwrap_or("");
    let anahtar = anahtarlar["keys"]
        .as_array()
        .and_then(|a| a.iter().find(|k| k["key_id"].as_str() == Some(kimlik)))
        .ok_or_else(|| tf!("'{kimlik}' anahtarı açık anahtar belgesinde yok", "key '{kimlik}' is not in the public key document"))?;
    if anahtar["alg"].as_str() != Some(ALG) {
        return Err(tf!("anahtar belgesindeki algoritma {} değil: {}", "the algorithm in the key document is not {}: {}", ALG, anahtar["alg"]));
    }
    let ab = B64.decode(anahtar["public_key"].as_str().unwrap_or("")).map_err(|_| t!("açık anahtar bozuk", "public key is corrupt"))?;
    let enc_vk = EncodedVerifyingKey::<MlDsa65>::try_from(ab.as_slice()).map_err(|_| t!("açık anahtar uzunluğu yanlış", "public key has the wrong length"))?;
    let vk = VerifyingKey::<MlDsa65>::decode(&enc_vk);
    let ib = B64.decode(s["value"].as_str().unwrap_or("")).map_err(|_| t!("imza bozuk", "signature is corrupt"))?;
    let enc_sig = EncodedSignature::<MlDsa65>::try_from(ib.as_slice()).map_err(|_| t!("imza uzunluğu yanlış", "signature has the wrong length"))?;
    let imza = Signature::<MlDsa65>::decode(&enc_sig).ok_or(t!("imza çözülemedi", "signature could not be decoded"))?;
    let mesaj = kanonik(&imzasiz(kart));
    if !vk.verify_with_context(mesaj.as_bytes(), baglam, &imza) {
        return Err(t!("İMZA GEÇERSİZ: kart imzalandıktan sonra değiştirilmiş ya da yanlış anahtar", "SIGNATURE INVALID: the card was changed after signing, or the key is wrong")
            .to_string());
    }
    Ok(kimlik.to_string())
}
