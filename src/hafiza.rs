//! Deneyim hafızası (vizyon 9.4, 10): aynı iş ikinci kez uygulamaya (ve onun GPU'suna) gitmez, hafızadan gelir.
//! Yeni bir buluş değildir; adı önbellektir. Farkı, nerede güvenli olduğunu Sezgi'nin koddan bilmesidir.
//!
//! Kurallar:
//! - **Varsayılan kapalı** (`sezgi hafiza ac`); müşteri verisi sınıfındadır (veri defteri), saklama süresi ve disk
//!   sınırı vardır, `sezgi unut --hepsi` siler.
//! - **Yalnızca yan etkisiz okumalar:** GET / HEAD; koddan bir tabloya yazdığı biliniyorsa (veri bağı) hatırlanmaz.
//!   Yazma işlemlerinin sonucu hiçbir zaman hatırlanmaz.
//! - **Parmak izi (bağlam):** proje, token (özeti; adı değil), insan oturumu, hedef adresi, haritanın içerik özeti,
//!   işlem, yöntem, istek adresi ve gövdesi. Token'lar, insanlar ve hedefler birbirinin sonucunu görmez; kod değişip
//!   harita yeniden üretilince eski sonuçlar kullanılmaz. Oturum ya da hedef değişince ve token iptal edilince hafıza
//!   temizlenir.
//! - **Oturumlu okumalar varsayılan olarak hatırlanmaz:** insanın oturumuyla (Authorization) alınan cevap kişiye
//!   özeldir; yalnızca `sezgi hafiza ac --oturumlu` ile açıkça istenirse hatırlanır (oturum yine parmak izindedir).
//! - **Kişisel veri:** cevapta e-posta adresi görünürse hatırlanmaz. Yalnızca başarılı (2xx) cevaplar hatırlanır.
//! - **Silme:** süre dolan sonuç silinir; disk sınırı aşılınca en uzun süredir kullanılmayanlar silinir.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::canli::Durum;

fn simdi_sn() -> u64 {
    SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn klasor(d: &Durum) -> PathBuf {
    d.kok.join("hafiza")
}

fn ayar(d: &Durum) -> Value {
    let a = d.ayar_oku();
    a.get("hafiza").cloned().unwrap_or_else(|| json!({ "acik": false }))
}

pub fn acik(d: &Durum) -> bool {
    ayar(d)["acik"] == true
}

pub fn sure_gun(d: &Durum) -> u64 {
    ayar(d)["sure_gun"].as_u64().unwrap_or(7)
}

fn sinir_bayt(d: &Durum) -> u64 {
    ayar(d)["sinir_mb"].as_u64().unwrap_or(100) * 1024 * 1024
}

/// Oturumla (Authorization) alınan cevaplar da hatırlansın mı (`sezgi hafiza ac --oturumlu`; varsayılan hayır)
pub fn oturumlu(d: &Durum) -> bool {
    ayar(d)["oturumlu"] == true
}

/// Hatırlanan cevabın bağlamı: bunlardan biri değişirse eski sonuç kullanılmaz
pub struct Baglam<'a> {
    /// Token kaydının özeti (adı değil: aynı adla yeniden üretilen token eski sonucu görmez)
    pub token_ozeti: &'a str,
    /// İnsan oturumunun değeri (parmak izine özeti girer); oturumsuz okumada None
    pub oturum: Option<&'a str>,
    pub islem: &'a str,
    pub yontem: &'a str,
    pub yol: &'a str,
    pub govde: Option<&'a [u8]>,
}

/// Parmak izi: proje, token özeti, insan oturumunun özeti, hedef adresi, haritanın içerik özeti, işlem, yöntem, adres
/// (sorgu dahil) ve gövde
pub fn anahtar(d: &Durum, harita: &Path, b: &Baglam) -> String {
    let ozet = crate::canli::ozet_metni;
    let proje = d.kok.file_name().map(|a| a.to_string_lossy().to_string()).unwrap_or_default();
    let insan = b.oturum.map(|o| ozet(o.as_bytes())).unwrap_or_default();
    let hedef = d.hedef_url().unwrap_or_default();
    let harita_ozeti = std::fs::read(harita.join("kapilar.json")).map(|m| ozet(&m)).unwrap_or_default();
    let mut m = format!("{proje}\n{}\n{insan}\n{hedef}\n{harita_ozeti}\n{}\n{}\n{}\n", b.token_ozeti, b.islem, b.yontem, b.yol).into_bytes();
    m.extend_from_slice(b.govde.unwrap_or_default());
    ozet(&m)
}

/// İşlem hatırlanabilir mi: yan etkisiz okuma (GET / HEAD) ve koddan bilinen bir tablo yazması yok
pub fn hatirlanabilir(k: &Value) -> bool {
    matches!(k["yontem"].as_str().unwrap_or(""), "GET" | "HEAD") && k["veri"]["yazar"].as_array().is_none_or(|y| y.is_empty())
}

fn kisisel(govde: &[u8]) -> bool {
    let re = regex::bytes::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap();
    re.is_match(govde)
}

/// Hafızadaki sonuç: (durum kodu, gövde, üretildiği zaman)
pub fn al(d: &Durum, anahtar: &str) -> Option<(u16, Vec<u8>, String)> {
    let yol = klasor(d).join(format!("{anahtar}.json"));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&yol).ok()?).ok()?;
    if v["bitis"].as_u64().is_some_and(|b| b <= simdi_sn()) {
        let _ = std::fs::remove_file(&yol);
        return None;
    }
    // Kullanım zamanı: disk sınırında en uzun süredir kullanılmayan silinir
    if let Ok(f) = std::fs::File::options().write(true).open(&yol) {
        let _ = f.set_modified(SystemTime::now());
    }
    // Şifreli (eski sürümlerin base64 düz metni de okunur)
    let govde = match &v["govde"] {
        Value::String(g) => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.decode(g).ok()?
        }
        m => d.muhur_ac(m)?,
    };
    Some((v["durum"].as_u64()? as u16, govde, v["zaman"].as_str().unwrap_or("").to_string()))
}

pub fn koy(d: &Durum, anahtar: &str, durum: u16, govde: &[u8]) {
    if !(200..300).contains(&durum) || kisisel(govde) {
        return;
    }
    let k = klasor(d);
    if crate::guvenli::gizli_klasor(&k).is_err() {
        return;
    }
    // Cevap müşteri verisidir: diskte şifreli; şifrelenemezse hatırlanmaz
    let Ok(muhur) = d.muhurle(govde) else { return };
    let v = json!({ "durum": durum, "zaman": crate::simdi(), "bitis": simdi_sn() + sure_gun(d) * 86_400, "govde": muhur });
    // Yazma ve disk sınırı temizliği durum klasörünün kilidi altında (aynı anda çalışan sunucular birbirinin dosyasını silmesin)
    let Ok(_k) = d.kilitle() else { return };
    let _ = crate::guvenli::atomik_yaz_gizli(&k.join(format!("{anahtar}.json")), v.to_string().as_bytes());
    temizle(d, false);
}

/// Süresi dolanları ve (disk sınırı aşılınca) en uzun süredir kullanılmayanları siler; `hepsi` ise bütününü
pub fn temizle(d: &Durum, hepsi: bool) -> usize {
    let k = klasor(d);
    let mut dosyalar: Vec<(PathBuf, SystemTime, u64)> = std::fs::read_dir(&k)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|g| g.metadata().ok().map(|m| (g.path(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len())))
        .collect();
    let mut silinen = 0;
    if hepsi {
        for (p, _, _) in &dosyalar {
            silinen += usize::from(std::fs::remove_file(p).is_ok());
        }
        return silinen;
    }
    let simdi = simdi_sn();
    dosyalar.retain(|(p, _, _)| {
        let dolmus = std::fs::read_to_string(p).ok().and_then(|m| serde_json::from_str::<Value>(&m).ok()).and_then(|v| v["bitis"].as_u64()).is_some_and(|b| b <= simdi);
        if dolmus {
            silinen += usize::from(std::fs::remove_file(p).is_ok());
        }
        !dolmus
    });
    let mut toplam: u64 = dosyalar.iter().map(|x| x.2).sum();
    dosyalar.sort_by_key(|x| x.1);
    for (p, _, boy) in dosyalar {
        if toplam <= sinir_bayt(d) {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            toplam -= boy;
            silinen += 1;
        }
    }
    silinen
}

/// (sonuç sayısı, toplam bayt)
pub fn durum(d: &Durum) -> (usize, u64) {
    let v: Vec<u64> = std::fs::read_dir(klasor(d)).into_iter().flatten().flatten().filter_map(|g| g.metadata().ok().map(|m| m.len())).collect();
    (v.len(), v.iter().sum())
}

/// sezgi hafiza [ac [--sure <gün>] [--sinir <MB>] [--oturumlu] | kapat | temizle]
pub fn komut(args: &[String]) {
    let harita = crate::guvenli::harita_bul(args.iter().position(|a| a == "--harita").and_then(|i| args.get(i + 1)).map(String::as_str));
    let d = Durum::yukle(&harita).unwrap_or_else(|e| {
        eprintln!("✗ {e}");
        std::process::exit(1)
    });
    let secenek = |ad: &str| args.iter().position(|a| a == ad).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok());
    match args.get(1).filter(|a| !a.starts_with("--")).map(String::as_str) {
        Some("ac") => {
            let sure = secenek("--sure").unwrap_or(7).clamp(1, 365);
            let sinir = secenek("--sinir").unwrap_or(100).clamp(1, 100_000);
            let oturumlu = args.iter().any(|a| a == "--oturumlu");
            d.ayar_yaz_genel("hafiza", json!({ "acik": true, "sure_gun": sure, "sinir_mb": sinir, "oturumlu": oturumlu })).unwrap_or_else(|e| {
                eprintln!("✗ {e}");
                std::process::exit(1)
            });
            let _ = d.kaydet(json!({ "durum": "hafiza_acildi", "sure_gun": sure, "sinir_mb": sinir, "oturumlu": oturumlu }));
            tp!(
                "✓ Deneyim hafızası açık: yan etkisiz okumaların sonucu {sure} gün, en fazla {sinir} MB tutulur.",
                "✓ Experience memory is on: results of side-effect-free reads are kept {sure} {g}, at most {sinir} MB.";
                g = crate::dil::cogul(sure, "day", "days")
            );
            tp!(
                "  Aynı istek ikinci kez uygulamaya gitmez. Yazmalar ve e-posta adresi içeren cevaplar hatırlanmaz; token'lar,",
                "  The same request does not reach the application twice. Writes and answers containing e-mail addresses are not kept; tokens,"
            );
            tp!(
                "  insanlar ve hedefler birbirinin sonucunu görmez; kod değişip harita yenilenince eski sonuçlar kullanılmaz.",
                "  people and targets never see each other's results; when the code changes and the map is refreshed, old results are not used."
            );
            if oturumlu {
                tp!(
                    "  Oturumla alınan cevaplar da hatırlanır (insan başına ayrı). Silmek: sezgi unut --hepsi",
                    "  Answers fetched with a session are kept too (separately per person). To delete: sezgi forget --all"
                );
            } else {
                tp!(
                    "  İnsanın oturumuyla alınan cevaplar hatırlanmaz (açmak: --oturumlu). Silmek: sezgi unut --hepsi",
                    "  Answers fetched with a person's session are not kept (to turn on: --oturumlu). To delete: sezgi forget --all"
                );
            }
        }
        Some("kapat") => {
            let silinen = temizle(&d, true);
            let _ = d.ayar_yaz_genel("hafiza", json!({ "acik": false }));
            let _ = d.kaydet(json!({ "durum": "hafiza_kapandi", "silinen": silinen }));
            tp!("✓ Deneyim hafızası kapalı; {silinen} sonuç silindi.", "✓ Experience memory is off; {silinen} {o} deleted."; o = crate::dil::cogul(silinen, "result", "results"));
        }
        Some("temizle") => tp!("✓ {} silindi.", "✓ {} deleted.", crate::dil::sayili(temizle(&d, true), "sonuç", "result", "results")),
        _ => {
            let (n, bayt) = durum(&d);
            if acik(&d) {
                tp!("Deneyim hafızası: açık · {n} sonuç · {} KB · saklama {} gün", "Experience memory: on · {n} {o} · {} KB · kept {} {g}", bayt.div_ceil(1024), sure_gun(&d); o = crate::dil::cogul(n, "result", "results"), g = crate::dil::cogul(sure_gun(&d), "day", "days"));
            } else {
                tp!("Deneyim hafızası: kapalı · {n} sonuç (açmak için: sezgi hafiza ac)", "Experience memory: off · {n} {o} (to turn on: sezgi memory on)"; o = crate::dil::cogul(n, "result", "results"));
            }
        }
    }
}
