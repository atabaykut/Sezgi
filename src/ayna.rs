//! Sezgi Ayna: haritanın, Benlik Kartı'nın, kapsamın ve imzanın yerel ekranı; izin ekranından token üretilir.
//! Yalnızca 127.0.0.1'e bağlanır, dışarıdan erişilemez. İnternet gerekmez (bütün dosyalar ikilinin içinde).
//! Uygulamayı çağırmaz, projeye yazmaz. Tek yazma izin ekranıdır: token üretmek ve iptal etmek, kullanıcının
//! yapılandırma klasörüne (`sezgi token` ile aynı yer ve aynı kod). Bu iki istek yalnızca Ayna'nın kendi sayfasından
//! kabul edilir: Origin zorunlu ve yerel, Content-Type JSON ve Ayna başlarken üretilen oturum anahtarı (`X-Sezgi-Anahtar`).
//! Okuma uçları (`/api/...`) da anahtar ister: harita, kart ve token listesi aynı makinedeki başka bir kullanıcıya açık
//! değildir. Sayfanın kendisi, betik ve stil anahtarsız açılır. Çalışan Ayna'nın anahtarlı adresi kullanıcının
//! yapılandırma klasöründe (yalnızca ona açık) tutulur; `sezgi ayna --adres` yeniden yazdırır.
//! Origin tarayıcı dışından (aynı makinedeki başka bir kullanıcı ya da süreç) taklit edilebilir; anahtar yalnızca Ayna'yı
//! başlatan kullanıcının gördüğü adreste (`#anahtar=`, sunucuya gönderilmeyen parça) bulunur. Bütün işlemleri açan `*`
//! deseni ayrıca açık onay ister (`tum_islemler`).
//! Harita, kart ve kanıt deposu her istekte değişim zamanlarıyla denetlenir: `sezgi init` ya da `sezgi izle` yeni harita
//! yazınca Ayna yenisini gösterir (bayat kalmaz). Kapsam ve güven sayıları `rapor` ile, kapı başına güven `tani` ile aynı
//! kanıt modelinden gelir.
//! Açılış sekmesi Sezgi'yi tanıtır: bu uygulamanın anlam özeti, dört ana yetenek ve yardımcıları, bu projedeki durumları
//! (`/api/sezgi`, her istekte gerçek durumdan, salt okuma), "Ne yapmak istiyorsun?" araması (`/api/ara`) ve "bu uygulama
//! hakkında ne biliyoruz" soruları (`/api/sorular`, güven damgalı; `tanitim`). Kapı ayrıntısı `/api/islem`den.
//! Haritasız açılış: bulunulan klasörde (ve üstlerinde) harita yoksa Ayna yine açılır; yalnızca tanıtım ve ilk adım
//! (`sezgi init`) gösterilir, harita uçları "harita yok" der. Güvenlik modeli aynıdır (anahtar, Host, Origin, CSP,
//! yalnızca 127.0.0.1) ve projeye yazılmaz. `sezgi init` haritayı yazınca Ayna onu kendiliğinden yükler.
//! Dil: her istek kendi dilinde cevaplanır. Adresteki `?lang=en|tr` (ya da `X-Sezgi-Lang` başlığı), yoksa sürecin dili
//! (`--lang`, `SEZGI_LANG`, sistem dili). Sayfa o dilde verilir (`<html lang>`), sayfadaki dil düğmesi öteki dile geçer ve
//! seçimi tarayıcıda saklar; veri uçları aynı dili `?lang=` ile ister. Güvenlik denetimleri dilden bağımsızdır.

use crate::{canli, imza, mcp, rapor, tanitim};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

const SAYFA: &str = include_str!("ayna/index.html");
const SAYFA_EN: &str = include_str!("ayna/index.en.html");
// Betik ve stil ayrı dosyalarda: CSP satır içi betiğe ve stile izin vermez (koddan gelen metin betik olamaz)
const BETIK: &str = include_str!("ayna/ayna.js");
const STIL: &str = include_str!("ayna/ayna.css");
const EN_FAZLA_GOVDE: usize = 1 << 20;

struct Durum {
    /// Oturum anahtarı: bütün `/api/` istekleri bunu `X-Sezgi-Anahtar` başlığında taşımalı
    anahtar: String,
    harita: Value,
    kart: Value,
    rapor: Value,
    imza: Value,
    /// Haritasız açılışta yok
    mcp: Option<mcp::Harita>,
    klasor: PathBuf,
    /// Uygulamanın klasörü (yetenek durumu: çatı, bağlantılar)
    proje: PathBuf,
    /// Kapı başına önermeler ve güvenleri (kapı kimliği -> [{yuklem, hedef, guven, gecerlilik, kapsam}])
    kanit: Value,
    /// "Bu uygulama hakkında ne biliyoruz" yanıtları, dil başına ilk istekte hesaplanır (Türkçe, İngilizce); harita
    /// değişince durum yeniden kurulur
    sorular: [std::sync::OnceLock<Value>; 2],
    /// Kanıt deposu dayanak dosyalarının değişimini izliyor mu (sorular için)
    izleniyor: bool,
}

impl Durum {
    fn harita_var(&self) -> bool {
        self.mcp.is_some()
    }

    /// Soruların yanıtları isteğin dilinde
    fn sorular(&self) -> &Value {
        self.sorular[usize::from(crate::dil::en())].get_or_init(|| match &self.mcp {
            Some(h) => {
                let model = h.model();
                tanitim::sorular(&self.harita, &self.rapor, model.as_deref(), self.izleniyor, Some(h))
            }
            None => tanitim::sorular_haritasiz(),
        })
    }
}

/// Haritanın klasöründen uygulamanın klasörü: `<uygulama>/.sezgi` ise üstü, değilse bulunulan klasör
fn proje_klasoru(klasor: &Path) -> PathBuf {
    let mutlak = std::fs::canonicalize(klasor).unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(klasor));
    match mutlak.parent() {
        Some(p) if mutlak.file_name().is_some_and(|a| a == ".sezgi") => p.to_path_buf(),
        _ => std::env::current_dir().unwrap_or_default(),
    }
}

/// Haritasız açılış: tanıtım ve ilk adım. Harita uçları "harita yok" der; projeye hiçbir şey yazılmaz
fn bos(klasor: &Path, proje: &Path) -> Result<Durum, String> {
    Ok(Durum {
        anahtar: yeni_anahtar()?,
        harita: Value::Null,
        kart: Value::Null,
        rapor: Value::Null,
        imza: Value::Null,
        mcp: None,
        klasor: klasor.to_path_buf(),
        proje: proje.to_path_buf(),
        kanit: Value::Null,
        sorular: Default::default(),
        izleniyor: false,
    })
}

fn yeni_anahtar() -> Result<String, String> {
    let mut tohum = [0u8; 16];
    getrandom::fill(&mut tohum).map_err(|e| tf!("rastgele sayı üretilemedi: {e}", "could not generate a random number: {e}"))?;
    Ok(tohum.iter().map(|x| format!("{x:02x}")).collect())
}

/// Kapı başına güven: kanıt modelinden, `tani`nın özetiyle aynı önermeler
fn kanit_haritasi(h: &mcp::Harita) -> Value {
    let Some(m) = h.model() else { return Value::Null };
    let mut v = serde_json::Map::new();
    for k in h.kapilar() {
        let onermeler: Vec<Value> = m
            .onermeler(&crate::graf::api_kimligi(k))
            .iter()
            .map(|o| json!({ "yuklem": o.yuklem, "hedef": o.hedef, "guven": o.hesap.guven, "gecerlilik": o.hesap.gecerlilik, "kapsam": o.hesap.kapsam }))
            .collect();
        v.entry(k["id"].as_str().unwrap_or("").to_string()).or_insert(json!(onermeler));
    }
    Value::Object(v)
}

/// Bütün bağlantıların paylaştığı durum: dosyalar değişince yeniden yüklenir; oturum anahtarı Ayna boyunca aynı kalır
struct Paylasilan {
    klasor: PathBuf,
    proje: PathBuf,
    anahtar: String,
    durum: std::sync::Mutex<(Vec<Option<std::time::SystemTime>>, std::sync::Arc<Durum>)>,
}

fn zamanlar(klasor: &Path) -> Vec<Option<std::time::SystemTime>> {
    ["kapilar.json", crate::kanit::DOSYA, "benlik.json", "benlik-keys.json"].iter().map(|d| klasor.join(d).metadata().and_then(|m| m.modified()).ok()).collect()
}

impl Paylasilan {
    /// Güncel durum: harita, kart ya da kanıt deposu değiştiyse yeniden yüklenir; yenisi okunamazsa eskisi kalır
    fn guncel(&self) -> std::sync::Arc<Durum> {
        let mut d = self.durum.lock().unwrap_or_else(|e| e.into_inner());
        let z = zamanlar(&self.klasor);
        if z != d.0 {
            if let Ok(mut y) = yukle(&self.klasor, &self.proje) {
                y.anahtar = self.anahtar.clone();
                *d = (z, std::sync::Arc::new(y));
            }
        }
        d.1.clone()
    }
}

/// İzin ekranı verisi: her işlem (kimlik, risk, ne işe yaradığı, neye dokunduğu), mod ve mevcut tokenler.
/// Token listesi her istekte yeniden okunur (komut satırından yapılan iptal hemen görünsün).
fn izinler(d: &Durum, h: &mcp::Harita) -> Value {
    let islemler: Vec<Value> = h
        .kapilar()
        .iter()
        .map(|k| {
            let (id, risk) = canli::islem_bilgisi(k);
            let (aciklama, kesin) = mcp::aciklama(k);
            json!({
                "id": id, "risk": risk, "yontem": k["yontem"], "yol": k["yol"], "kapi": k["id"],
                "modul": k["kontrolcu"].as_str().unwrap_or("").trim_end_matches("Controller"),
                "aciklama": aciklama, "aciklama_kesin": kesin,
                "giris": k["giris"], "roller": k["roller"], "okur": k["veri"]["okur"], "yazar": k["veri"]["yazar"],
                "kalkmis": k["kullanimdan_kalkmis"] == true,
            })
        })
        .collect();
    match canli::Durum::yukle(&d.klasor) {
        Ok(c) => json!({ "islemler": islemler, "mod": c.mod_(), "tokenler": canli::token_listesi(&c) }),
        Err(e) => json!({ "islemler": islemler, "mod": "gozlem", "tokenler": [], "hata": e }),
    }
}

/// İzin ekranından token üretir: { ad, izinler: ["desen:seviye"], sure, onay }
fn token_uret(d: &Durum, istek: &Value) -> Result<Value, String> {
    let h = d.mcp.as_ref().ok_or_else(harita_yok)?;
    let c = canli::Durum::yukle(&d.klasor)?;
    let izinler: Vec<String> =
        istek["izinler"].as_array().ok_or(t!("izinler listesi gerekli", "a permission list is required"))?.iter().filter_map(|i| i.as_str().map(String::from)).collect();
    if izinler.len() > 2000 {
        return Err(t!("en fazla 2000 izin", "at most 2000 permissions").into());
    }
    // `*` bütün işlemleri (gelecekte eklenecekler dahil) açar: yalnızca açık onayla
    if izinler.iter().any(|i| i.rsplit_once(':').map(|(d, _)| d).unwrap_or(i) == "*") && istek["tum_islemler"] != true {
        return Err(t!(
            "'*' deseni bütün işlemleri (sonradan eklenecekler dahil) açar: açıkça onaylanmalı (tum_islemler); işlemleri tek tek seçin",
            "the '*' pattern opens every operation (later ones included): it must be confirmed explicitly (tum_islemler); choose operations one by one"
        )
        .into());
    }
    let (acik, izinler) = canli::token_uret(&c, istek["ad"].as_str().unwrap_or(""), &izinler, istek["sure"].as_u64().unwrap_or(90), istek["onay"].as_str().unwrap_or("yikici"))?;
    let baglanti: Vec<Value> = canli::baglanti_satirlari(&d.klasor, &acik).into_iter().map(|(b, s)| json!({ "baslik": b, "satir": s })).collect();
    // Yapay zekânın göreceği işlem tool'ları (MCP sunucusuyla aynı kural ve adlar)
    let kayit = json!({ "izinler": izinler });
    let gorunen = h.suz("", |k| canli::kapi_izinli(&kayit, k));
    let (kapilar, esik) = (gorunen.kapilar(), crate::islem_arac::esik());
    let araclar: Vec<Value> = if kapilar.len() > esik {
        vec![]
    } else {
        crate::islem_arac::adlar(kapilar)
            .iter()
            .zip(kapilar)
            .map(|(ad, k)| {
                let t = crate::islem_arac::tanim(ad, k, c.mod_() == "gozlem", crate::islem_arac::surum_notu(&gorunen, k));
                json!({ "ad": t["name"], "aciklama": t["description"] })
            })
            .collect()
    };
    Ok(json!({ "token": acik, "izinler": izinler, "baglanti": baglanti, "mod": c.mod_(), "araclar": araclar, "gorunen": kapilar.len(), "esik": esik }))
}

fn harita_yok() -> String {
    t!("Bu klasörde harita yok: önce sezgi init", "There is no map in this folder: run sezgi init first").into()
}

/// Ayna oturum anahtarı eksik ya da yanlış olduğunda söylenen
fn anahtarsiz() -> &'static str {
    t!(
        "Ayna oturum anahtarı yok ya da yanlış: Ayna'yı başlatırken yazılan adresle (…/#anahtar=…) açın; adresi yeniden görmek için: sezgi ayna --adres",
        "The Ayna session key is missing or wrong: open Ayna with the address printed when it started (…/#anahtar=…); to see the address again: sezgi ayna --adres"
    )
}

/// Başka bir kökenden gelen istek
fn baska_koken() -> &'static str {
    t!("Başka bir kökenden istek kabul edilmez.", "Requests from another origin are not accepted.")
}

/// İsteğin dili: adresteki `lang=` (ya da `X-Sezgi-Lang` başlığı), yoksa sürecin dili
fn istek_dili(sorgu: &str, baslik: &str) -> crate::dil::Dil {
    sorgu.split('&').filter_map(|p| p.strip_prefix("lang=")).chain(Some(baslik).filter(|b| !b.is_empty())).find_map(crate::dil::etiketten).unwrap_or_else(crate::dil::dil)
}

fn yukle(klasor: &Path, proje: &Path) -> Result<Durum, String> {
    // Harita depodan gelmiş olabilir: klasör ya da dosyalar sembolik bağ olamaz, boyut sınırlı
    if std::fs::symlink_metadata(klasor).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(tf!("{} sembolik bağ: Ayna bağ izlemez", "{} is a symbolic link: Ayna does not follow links", klasor.display()));
    }
    let oku = |ad: &str| -> Value { crate::guvenli::oku_harita(&klasor.join(ad)).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null) };
    let mut harita = oku("kapilar.json");
    crate::guvenli::json_temiz(&mut harita);
    if harita.is_null() {
        return Err(tf!("{} içinde kapilar.json yok (önce `sezgi init`)", "{} has no kapilar.json (run `sezgi init` first)", klasor.display()));
    }
    let kart = oku("benlik.json");
    let anahtarlar = oku("benlik-keys.json");
    // İmza durumu: imzasız (alg "none"), anahtar yok, kendi içinde tutarlı ya da GEÇERSİZ (kart imzalandıktan sonra değişmiş).
    // Anahtar kartın yanındaki dosyadan gelir: kartı ve anahtarı birlikte getiren biri tutarlı imza üretebilir. Bu yüzden
    // "tutarlı" yalnızca bütünlüğü gösterir, kimliği değil; güvenilir (yeşil) rozet bir güven çapası olmadan verilmez.
    let alg = kart["signature"]["alg"].as_str().unwrap_or("none");
    let imza = if alg == "none" || kart["signature"]["value"].as_str().map(|v| v == "UNSIGNED-DEV-BUILD").unwrap_or(true) {
        json!({ "durum": "imzasiz" })
    } else if anahtarlar.is_null() {
        json!({ "durum": "anahtar_yok", "alg": kart["signature"]["alg"], "key_id": kart["signature"]["key_id"] })
    } else {
        match imza::dogrula(&kart, &anahtarlar) {
            // Güven çapası kullanıcının yapılandırmasından okunur (Ayna dosyaya yazmaz)
            Ok(k) => {
                let durum = match imza::guven_durumu(&kart, &anahtarlar) {
                    imza::Guven::Sabit => "guvenilir",
                    imza::Guven::SabitUyusmaz => "sabitlenmis_anahtarla_uyusmuyor",
                    imza::Guven::Degisti => "anahtar_degisti",
                    imza::Guven::Bilinmiyor => "tutarli_anahtar_dogrulanmadi",
                };
                json!({ "durum": durum, "alg": kart["signature"]["alg"], "key_id": k })
            }
            Err(e) => json!({ "durum": "gecersiz", "hata": e }),
        }
    };
    let anahtar = yeni_anahtar()?;
    let mcp = mcp::Harita::yukle(klasor)?;
    let kanit = kanit_haritasi(&mcp);
    let rapor = rapor::uret(klasor)?;
    let model = mcp.model();
    // Kanıt deposu dayanak dosyalarının değişimini izliyor mu: uygulama kökü depoda yazılı ya da harita `<uygulama>/.sezgi`
    let izleniyor = model.as_ref().is_some_and(|m| m.depo.kok.is_some() || (m.haritadan && klasor.file_name().is_some_and(|a| a == ".sezgi")));
    Ok(Durum { anahtar, rapor, harita, kart, imza, mcp: Some(mcp), klasor: klasor.to_path_buf(), proje: proje.to_path_buf(), kanit, sorular: Default::default(), izleniyor })
}

pub(crate) fn cevap(akis: &mut TcpStream, kod: &str, tur: &str, govde: &[u8]) {
    let bas = format!(
        "HTTP/1.1 {kod}\r\nContent-Type: {tur}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n",
        govde.len()
    );
    let _ = akis.write_all(bas.as_bytes());
    let _ = akis.write_all(govde);
}

fn json_cevap(akis: &mut TcpStream, v: &Value) {
    cevap(akis, "200 OK", "application/json; charset=utf-8", serde_json::to_string(v).unwrap_or_default().as_bytes());
}

/// Bir bağlantının isteği okumak için toplam süresi: baytları tek tek gönderen yavaş istemci (slowloris) bağlantıyı
/// süresiz tutamaz. Okuma başına zaman aşımı bunu engellemez, çünkü her bayt zamanlayıcıyı sıfırlar.
pub(crate) const ISTEK_SURESI: std::time::Duration = std::time::Duration::from_secs(8);

/// Son tarihe kadar kalan süreyle okuma zaman aşımını ayarlar; süre dolduysa None
fn kalan_sure(r: &BufReader<TcpStream>, son: std::time::Instant) -> Option<()> {
    let kalan = son.checked_duration_since(std::time::Instant::now()).filter(|k| !k.is_zero())?;
    r.get_ref().set_read_timeout(Some(kalan)).ok()
}

/// Başlık satırı (8 KiB) ve başlık sayısı (64) sınırlı, toplam süre `son` ile sınırlı okunur
pub(crate) fn satir_oku(r: &mut BufReader<TcpStream>, son: std::time::Instant) -> Option<String> {
    let mut ham = Vec::new();
    loop {
        kalan_sure(r, son)?;
        let tampon = r.fill_buf().ok()?;
        if tampon.is_empty() {
            return None;
        }
        let (bitti, n) = match tampon.iter().position(|b| *b == b'\n') {
            Some(i) => (true, i + 1),
            None => (false, tampon.len()),
        };
        ham.extend_from_slice(&tampon[..n]);
        r.consume(n);
        if ham.len() > 8 * 1024 {
            return None;
        }
        if bitti {
            return String::from_utf8(ham).ok();
        }
    }
}

/// Gövdeyi toplam süre sınırı içinde okur
pub(crate) fn govde_oku(r: &mut BufReader<TcpStream>, uzunluk: usize, son: std::time::Instant) -> Option<Vec<u8>> {
    let mut govde = vec![0u8; uzunluk];
    let mut okunan = 0;
    while okunan < uzunluk {
        kalan_sure(r, son)?;
        match r.read(&mut govde[okunan..]) {
            Ok(0) | Err(_) => return None,
            Ok(n) => okunan += n,
        }
    }
    Some(govde)
}

/// Açık bağlantı sayacı: iş parçacığı nasıl biterse bitsin (panik dahil) sayaç azalır
pub(crate) struct Baglanti(pub(crate) std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl Drop for Baglanti {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

fn isle(akis: &mut TcpStream, d: &Durum, port: u16) {
    let Ok(kopya) = akis.try_clone() else { return };
    let mut okuyucu = BufReader::new(kopya);
    let son = std::time::Instant::now() + ISTEK_SURESI;
    let Some(ilk) = satir_oku(&mut okuyucu, son) else { return };
    let mut parca = ilk.split_whitespace();
    let (yontem, yol) = (parca.next().unwrap_or(""), parca.next().unwrap_or(""));
    // Adresteki sorgu yalnızca dil seçimini taşır; yönlendirme yol üzerinden
    let (yol, sorgu) = yol.split_once('?').unwrap_or((yol, ""));
    let (mut uzunluk, mut host, mut koken, mut tur, mut anahtar, mut dil) = (0usize, String::new(), None::<String>, String::new(), String::new(), String::new());
    for _ in 0..64 {
        let Some(satir) = satir_oku(&mut okuyucu, son) else { return };
        if satir.trim().is_empty() {
            break;
        }
        let (a, v) = satir.split_once(':').unwrap_or(("", ""));
        match a.trim().to_ascii_lowercase().as_str() {
            "content-length" => uzunluk = v.trim().parse().unwrap_or(usize::MAX),
            "host" => host = v.trim().to_string(),
            "origin" => koken = Some(v.trim().to_string()),
            "content-type" => tur = v.trim().to_ascii_lowercase(),
            "x-sezgi-anahtar" => anahtar = v.trim().to_string(),
            "x-sezgi-lang" => dil = v.trim().to_string(),
            _ => {}
        }
    }
    // Cevabın bütün metinleri isteğin dilinde
    crate::dil::ile(istek_dili(sorgu, &dil), || cevapla(akis, &mut okuyucu, d, port, son, yontem, yol, (uzunluk, &host, koken.as_deref(), &tur, &anahtar)));
}

/// İsteğin başlıkları: (Content-Length, Host, Origin, Content-Type, oturum anahtarı)
type Basliklar<'a> = (usize, &'a str, Option<&'a str>, &'a str, &'a str);

#[allow(clippy::too_many_arguments)]
fn cevapla(akis: &mut TcpStream, okuyucu: &mut BufReader<TcpStream>, d: &Durum, port: u16, son: std::time::Instant, yontem: &str, yol: &str, b: Basliklar) {
    let (uzunluk, host, koken, tur, anahtar) = b;
    // DNS yeniden bağlama saldırısına karşı: yalnızca yerel ad kabul edilir
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        cevap(akis, "403 Forbidden", "text/plain; charset=utf-8", t!("Ayna yalnızca yerel adresten açılır.", "Ayna opens only from the local address.").as_bytes());
        return;
    }
    // Veri uçları oturum anahtarı ister (anahtarsız açılan sayfa kullanıcıya ne yapacağını söyler)
    if yol.starts_with("/api/") && !esit(anahtar.as_bytes(), d.anahtar.as_bytes()) {
        let m = json!({ "hata": anahtarsiz() });
        return cevap(akis, "403 Forbidden", "application/json; charset=utf-8", m.to_string().as_bytes());
    }
    match (yontem, yol) {
        // Sayfa isteğin dilinde: `<html lang>` ve sabit metinler
        ("GET", "/") => cevap(akis, "200 OK", "text/html; charset=utf-8", t!(SAYFA, SAYFA_EN).as_bytes()),
        ("GET", "/ayna.js") => cevap(akis, "200 OK", "text/javascript; charset=utf-8", BETIK.as_bytes()),
        ("GET", "/ayna.css") => cevap(akis, "200 OK", "text/css; charset=utf-8", STIL.as_bytes()),
        // Tanıtım: yetenekler her istekte gerçek durumdan (bağlantı, canlı mod, santral Ayna dışından da değişir)
        ("GET", "/api/sezgi") => {
            let model = d.mcp.as_ref().and_then(|h| h.model());
            json_cevap(akis, &tanitim::yetenekler(&d.proje, d.harita_var().then_some((d.klasor.as_path(), &d.harita)), model.as_deref()))
        }
        ("GET", "/api/sorular") => json_cevap(akis, d.sorular()),
        // Haritasız açılış: harita uçları ne yapılacağını söyler
        (_, y) if y.starts_with("/api/") && !d.harita_var() => {
            let m = json!({ "hata": harita_yok(), "harita": false });
            cevap(akis, "404 Not Found", "application/json; charset=utf-8", m.to_string().as_bytes())
        }
        ("GET", "/api/harita") => json_cevap(akis, &d.harita),
        ("GET", "/api/kart") => json_cevap(akis, &d.kart),
        ("GET", "/api/rapor") => json_cevap(akis, &d.rapor),
        ("GET", "/api/imza") => json_cevap(akis, &d.imza),
        ("GET", "/api/kanit") => json_cevap(akis, &d.kanit),
        ("GET", "/api/izinler") => match &d.mcp {
            Some(h) => json_cevap(akis, &izinler(d, h)),
            None => json_cevap(akis, &json!({ "hata": harita_yok() })),
        },
        // Token üretme ve iptal: yalnızca Ayna'nın kendi sayfasından. Origin zorunlu (tarayıcı her POST'ta gönderir;
        // Origin'siz istek tarayıcı dışından gelir) ve JSON zorunlu (başka bir sitenin basit form isteği olamaz).
        ("POST", "/api/token" | "/api/token/iptal") => {
            let yerel = koken.is_some_and(|o| o == format!("http://127.0.0.1:{port}") || o == format!("http://localhost:{port}"));
            if !yerel || !tur.starts_with("application/json") {
                return cevap(
                    akis,
                    "403 Forbidden",
                    "text/plain; charset=utf-8",
                    t!("Token yalnızca Ayna sayfasından üretilir.", "Tokens are created only from the Ayna page.").as_bytes(),
                );
            }
            // Origin taklit edilebilir: Ayna'yı başlatan kullanıcının adresindeki oturum anahtarı da gerekir
            if !esit(anahtar.as_bytes(), d.anahtar.as_bytes()) {
                let m = json!({ "hata": anahtarsiz() });
                return cevap(akis, "403 Forbidden", "application/json; charset=utf-8", m.to_string().as_bytes());
            }
            if uzunluk > EN_FAZLA_GOVDE {
                return cevap(akis, "413 Payload Too Large", "text/plain; charset=utf-8", b"Buyuk");
            }
            let Some(govde) = govde_oku(okuyucu, uzunluk, son) else { return };
            let istek: Value = serde_json::from_slice(&govde).unwrap_or(Value::Null);
            let sonuc = if yol == "/api/token" {
                token_uret(d, &istek)
            } else {
                canli::Durum::yukle(&d.klasor).and_then(|c| canli::token_iptal(&c, istek["ad"].as_str().unwrap_or(""))).map(|_| json!({ "tamam": true }))
            };
            json_cevap(akis, &sonuc.unwrap_or_else(|e| json!({ "hata": e })));
        }
        // Başka bir siteden gelen istek (CSRF) reddedilir: Origin varsa yerel adres olmalı
        ("POST", "/api/onizle") if koken.is_some_and(|o| o != format!("http://127.0.0.1:{port}") && o != format!("http://localhost:{port}")) => {
            cevap(akis, "403 Forbidden", "text/plain; charset=utf-8", baska_koken().as_bytes())
        }
        ("POST", "/api/onizle") if uzunluk <= EN_FAZLA_GOVDE => {
            let Some(govde) = govde_oku(okuyucu, uzunluk, son) else { return };
            let istek: Value = serde_json::from_slice(&govde).unwrap_or(Value::Null);
            // Metin isteğin dilinde, karar dilden bağımsız
            let (metin, karar) = match d.mcp.as_ref().map(|h| h.onizle_karari(&istek)) {
                Some((m, k)) => (m, Some(k)),
                None => (harita_yok(), None),
            };
            let karar = karar.map(|k| match k {
                mcp::OnizlemeKarari::KapiYok => "kapi_yok",
                mcp::OnizlemeKarari::Reddedilir => "reddedilir",
                mcp::OnizlemeKarari::Belirsiz => "belirsiz",
                mcp::OnizlemeKarari::Gecer => "gecer",
            });
            json_cevap(akis, &json!({ "metin": metin, "karar": karar }));
        }
        // Arama ("Ne yapmak istiyorsun?"): Türkçe ya da İngilizce istek kodun kelimelerine ve HTTP yöntemine çevrilir (MCP
        // `ara` ile aynı sözlük); ilk sonuçlar etkisi, geri alma ve onay kararıyla. İşlem ayrıntısı: çağrı zinciri, tablo
        // etkisi, nesne düzeyi yetki, dış etki, geri alma ve onay gerekçesi (`tanitim`)
        ("POST", "/api/ara" | "/api/islem") if koken.is_some_and(|o| o != format!("http://127.0.0.1:{port}") && o != format!("http://localhost:{port}")) => {
            cevap(akis, "403 Forbidden", "text/plain; charset=utf-8", baska_koken().as_bytes())
        }
        ("POST", y @ ("/api/ara" | "/api/islem")) if uzunluk <= EN_FAZLA_GOVDE => {
            let Some(govde) = govde_oku(okuyucu, uzunluk, son) else { return };
            let istek: Value = serde_json::from_slice(&govde).unwrap_or(Value::Null);
            let model = d.mcp.as_ref().and_then(|h| h.model());
            let v = if y == "/api/ara" {
                tanitim::ara(&d.harita, model.as_deref(), d.mcp.as_ref(), istek["sorgu"].as_str().unwrap_or(""))
            } else {
                tanitim::islem(&d.harita, model.as_deref(), d.mcp.as_ref(), istek["kapi"].as_str().unwrap_or(""))
            };
            json_cevap(akis, &v);
        }
        _ => cevap(akis, "404 Not Found", "text/plain; charset=utf-8", b"Yok"),
    }
}

/// Çalışan Ayna'nın kaydı: yapılandırma klasöründe `ayna/<port>.json` (anahtarlı adres; yalnızca kullanıcıya açık)
fn kayit_yolu(port: u16) -> Option<PathBuf> {
    imza::yapilandirma_klasoru().map(|k| k.join("ayna").join(format!("{port}.json")))
}

/// `sezgi ayna --adres`: bu kullanıcının çalışan Ayna'larının anahtarlı adresleri (Ayna yeniden başlatılmadan). Artık
/// cevap vermeyen Ayna'nın kaydı silinir.
pub fn adresler() {
    let Some(klasor) = imza::yapilandirma_klasoru().map(|k| k.join("ayna")) else {
        tp!("Çalışan Ayna yok (başlatmak için: sezgi ayna)", "No Ayna is running (to start one: sezgi ayna)");
        return;
    };
    let mut n = 0;
    for g in std::fs::read_dir(&klasor).into_iter().flatten().flatten() {
        let Some(v) = crate::guvenli::oku(&g.path()).and_then(|m| serde_json::from_str::<Value>(&m).ok()) else { continue };
        let (Some(port), Some(anahtar)) = (v["port"].as_u64().and_then(|p| u16::try_from(p).ok()), v["anahtar"].as_str()) else { continue };
        let adres = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        if TcpStream::connect_timeout(&adres, std::time::Duration::from_millis(300)).is_err() {
            let _ = std::fs::remove_file(g.path());
            continue;
        }
        n += 1;
        println!("http://127.0.0.1:{port}/#anahtar={anahtar}  ({})", kayit_etiketi(&v));
    }
    if n == 0 {
        tp!("Çalışan Ayna yok (başlatmak için: sezgi ayna)", "No Ayna is running (to start one: sezgi ayna)");
    }
}

/// Kayıttaki Ayna'nın etiketi haritanın bugünkü durumundan: haritasız açılıp sonradan `init` edilen Ayna haritayı
/// yüklemiştir, "harita yok" denmez. Eski biçimli kayıtta yazılı etiket kullanılır
fn kayit_etiketi(v: &Value) -> String {
    match (v["klasor"].as_str(), v["proje"].as_str()) {
        (Some(k), Some(p)) => match std::fs::symlink_metadata(Path::new(k).join("kapilar.json")) {
            Ok(_) => canli::mutlak_yol(Path::new(k)),
            Err(_) => tf!("{p} (harita yok)", "{p} (no map)"),
        },
        _ => v["harita"].as_str().unwrap_or("").to_string(),
    }
}

/// Sabit süreli karşılaştırma (anahtarın baytları zamanlamadan sızmasın)
fn esit(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |f, (x, y)| f | (x ^ y)) == 0
}

/// Ayna'yı başlatır. Klasörde harita (`kapilar.json`) yoksa haritasız açılır: tanıtım ve ilk adım
pub fn calistir(klasor: &Path, port: u16) {
    let ilk_zaman = zamanlar(klasor);
    let proje = proje_klasoru(klasor);
    // Bozuk ya da sembolik bağ olan harita haritasız sayılmaz: yükleme hatası söylenir
    let haritasiz = std::fs::symlink_metadata(klasor.join("kapilar.json")).is_err();
    let d = match if haritasiz { bos(klasor, &proje) } else { yukle(klasor, &proje) } {
        Ok(d) => d,
        Err(e) => {
            eprintln!("✗ {e}");
            std::process::exit(1);
        }
    };
    let dinleyici = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            te!("✗ 127.0.0.1:{port} açılamadı: {e} (başka bir port için --port)", "✗ 127.0.0.1:{port} could not be opened: {e} (for another port: --port)");
            std::process::exit(1);
        }
    };
    // --port 0: işletim sistemi boş bir port seçer
    let port = dinleyici.local_addr().map(|a| a.port()).unwrap_or(port);
    // Anahtarlı adres kullanıcının yapılandırma klasöründe (yalnızca ona açık): `sezgi ayna --adres` yeniden yazdırır
    if let Some(k) = kayit_yolu(port) {
        let gorunen = if haritasiz { tf!("{} (harita yok)", "{} (no map)", canli::mutlak_yol(&proje)) } else { canli::mutlak_yol(klasor) };
        // Haritanın ve projenin mutlak yolu da yazılır: haritasız açılan Ayna `init`ten sonra haritayı kendisi yükler,
        // listeleme etiketi kayıttan değil haritanın bugünkü durumundan hesaplanır
        let mutlak = std::path::absolute(klasor).unwrap_or_else(|_| klasor.to_path_buf());
        let v =
            json!({ "port": port, "anahtar": d.anahtar, "harita": gorunen, "klasor": mutlak.display().to_string(), "proje": canli::mutlak_yol(&proje), "zaman": crate::simdi() });
        let _ = crate::guvenli::gizli_klasor(k.parent().unwrap()).map_err(|e| e.to_string()).and_then(|_| crate::guvenli::atomik_yaz_gizli(&k, v.to_string().as_bytes()));
    }
    // Standart çıktı kapalı bir boruya bağlı olabilir (ör. adresi okuyup boruyu kapatan bir araç): println! panik
    // verip sunucuyu düşürürdü. Yazma hataları yok sayılır, sunucu çalışmaya devam eder.
    {
        let mut c = std::io::stdout().lock();
        let ad = if haritasiz { t!("harita yok", "no map") } else { d.harita["ozet"]["uygulama"].as_str().unwrap_or("") };
        let _ = writeln!(c, "{}", tf!("◆ SEZGİ AYNA — {ad}", "◆ SEZGI AYNA — {ad}"));
        let _ = writeln!(
            c,
            "{}",
            tf!("  Adres   http://127.0.0.1:{port}/#anahtar={}  (yalnızca bu bilgisayar)", "  Address http://127.0.0.1:{port}/#anahtar={}  (this computer only)", d.anahtar)
        );
        let _ = writeln!(
            c,
            "{}",
            t!(
                "  Bu adresle açın: anahtar yalnızca sizdedir, sunucuya adres satırında gitmez (yeniden görmek için: sezgi ayna --adres)",
                "  Open it with this address: only you have the key, and it is not sent to the server in the address bar (to see it again: sezgi ayna --adres)"
            )
        );
        if haritasiz {
            let _ = writeln!(
                c,
                "{}",
                t!(
                    "  Bu klasörde harita yok: Ayna Sezgi'yi tanıtır ve ilk adımı gösterir, hiçbir şey yazmaz. Haritayı çıkarmak için: sezgi init",
                    "  There is no map in this folder: Ayna introduces Sezgi and shows the first step; it writes nothing. To build the map: sezgi init"
                )
            );
        }
        let _ = writeln!(c, "{}", t!("  Durdurmak için Ctrl+C", "  Press Ctrl+C to stop"));
        let _ = c.flush();
    }
    // Her bağlantı kendi iş parçacığında: yavaş bir istemci diğerlerini bekletemez. Durum salt okunurdur; harita
    // değişince yenisi kurulur (bağlantı başladığı andaki durumla cevaplanır)
    let d = std::sync::Arc::new(Paylasilan { klasor: klasor.to_path_buf(), proje, anahtar: d.anahtar.clone(), durum: std::sync::Mutex::new((ilk_zaman, std::sync::Arc::new(d))) });
    let acik = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for akis in dinleyici.incoming().flatten() {
        // Aynı anda en fazla 32 bağlantı; fazlası hemen kapatılır
        if acik.load(std::sync::atomic::Ordering::Relaxed) >= 32 {
            continue;
        }
        acik.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (d, sayac) = (d.clone(), Baglanti(acik.clone()));
        std::thread::spawn(move || {
            let _sayac = sayac;
            let mut akis = akis;
            let _ = akis.set_write_timeout(Some(std::time::Duration::from_secs(5)));
            isle(&mut akis, &d.guncel(), port);
        });
    }
}
