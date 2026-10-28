//! D8 İngilizce: temel yolculuk İngilizce modda Türkçe harf ya da sözcük içermez (yol, kimlik ve uygulama verisi hariç).
//! Argümansız `sezgi` (haritasız ve haritalı), `--help`, komut yardımı, `init`, `init --deneme`, `doktor`, `rapor`,
//! `canli`, bilinmeyen komut önerisi, bilinmeyen bayrak ve eksik argüman. Aynı komutlar Türkçe modda Türkçe kalır.
mod p0_g1_ortak;
use p0_g1_ortak::*;

const TURKCE_HARF: &[char] = &['ç', 'ğ', 'ı', 'İ', 'ö', 'ş', 'ü', 'Ç', 'Ğ', 'Ö', 'Ş', 'Ü', 'â'];
/// Türkçe çıktıda sık geçen, İngilizcede olmayan sözcükler (komut ve bayrak adları hariç: bagla, ayna, rapor, hedef …)
const TURKCE_SOZCUK: &[&str] = &[
    "yok", "ve", "ya", "da", "bir", "icin", "islem", "harita", "haritayi", "uygulama", "komut", "dosya", "satir", "kural", "sonraki", "adim",
    "yardim", "bilinmiyor", "girdi", "kanit", "durum", "kapali", "acik", "gerekli", "tamam", "uyari", "hata", "eksik", "bulunamadi",
    "kart", "imzasiz", "gecerli", "okundu", "tablo", "giris",
];

/// İngilizce modda komut: `--lang en` her komutun önünde
fn en(p: &Proje, args: &[&str]) -> (bool, String) {
    let mut a = vec!["--lang", "en"];
    a.extend_from_slice(args);
    p.cikti(&a)
}

/// Türkçe harf ya da sözcük: yol ve uygulama adı çıkarıldıktan sonra
fn turkce_izler(p: &Proje, metin: &str) -> Vec<String> {
    let mut m = metin.to_string();
    for y in [p.klasor.display().to_string(), p.ev.display().to_string()] {
        m = m.replace(&y, "<yol>");
    }
    let mut izler: Vec<String> = m.chars().filter(|c| TURKCE_HARF.contains(c)).map(|c| c.to_string()).collect();
    for s in m.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()) {
        if TURKCE_SOZCUK.contains(&s.to_lowercase().as_str()) {
            izler.push(s.to_string());
        }
    }
    izler.sort();
    izler.dedup();
    izler
}

fn temiz(p: &Proje, ad: &str, metin: &str) {
    let izler = turkce_izler(p, metin);
    assert!(izler.is_empty(), "{ad}: Türkçe izler {izler:?}\n{metin}");
}

#[test]
fn ingilizce_yolculuk_turkce_icermez() {
    let ev = Ev::yeni("d8-ingilizce-yolculuk");
    let p = ev.proje("nestjs-temel", "app");
    let bos = ev.proje("nestjs-temel", "bos");
    let _ = std::fs::remove_dir_all(bos.klasor.join(".sezgi"));
    std::fs::remove_dir_all(bos.klasor.join("src")).unwrap();
    std::fs::remove_file(bos.klasor.join("package.json")).unwrap();

    let (_, m) = en(&bos, &[]);
    assert!(m.contains("sezgi init"), "{m}");
    temiz(&bos, "haritasız argümansız", &m);

    for (ad, args) in [
        ("--help", vec!["--help"]),
        ("help init", vec!["help", "init"]),
        ("init --help", vec!["init", "--help"]),
        ("live --help", vec!["live", "--help"]),
        ("init --dry-run", vec!["init", ".", "--dry-run"]),
        ("init", vec!["init", "."]),
        ("argümansız", vec![]),
        ("doctor", vec!["doctor"]),
        ("report", vec!["report"]),
        ("live", vec!["live"]),
        ("verify", vec!["verify", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]),
    ] {
        let (_, m) = en(&p, &args);
        temiz(&p, ad, &m);
    }

    // Hatalar: bilinmeyen komut, bilinmeyen bayrak, eksik argüman, uygulama olmayan klasör
    let (ok, m) = en(&p, &["reprot"]);
    assert!(!ok && m.contains("Did you mean: sezgi report"), "{m}");
    temiz(&p, "bilinmeyen komut", &m);
    let (ok, m) = en(&p, &["report", "--jsn"]);
    assert!(!ok && m.contains("Unknown flag"), "{m}");
    temiz(&p, "bilinmeyen bayrak", &m);
    let (ok, m) = en(&p, &["verify"]);
    assert!(!ok && m.contains("Missing argument"), "{m}");
    temiz(&p, "eksik argüman", &m);
    let (ok, m) = en(&bos, &["init", "."]);
    assert!(!ok && m.contains("No application Sezgi can read"), "{m}");
    temiz(&bos, "uygulama yok", &m);
}

#[test]
fn turkce_yolculuk_turkce_kalir() {
    let ev = Ev::yeni("d8-ingilizce-turkce");
    let p = ev.proje("nestjs-temel", "app");
    let (_, m) = p.cikti(&["--lang", "tr", "rapor"]);
    assert!(m.contains("Sezgi kapsam raporu") && m.contains("İŞLEMLER"), "{m}");
    let (_, m) = p.cikti(&["--lang", "tr", "init", "."]);
    assert!(m.contains("Sonraki adım:") && m.contains("sezgi bagla"), "{m}");
    let (_, m) = p.cikti(&["--lang", "tr", "--help"]);
    assert!(m.contains("Yapay zekâya bağla") && m.contains("kodu okur"), "{m}");
    let (_, m) = p.cikti(&["--lang", "en", "--help"]);
    assert!(m.contains("Connect to AI") && m.contains("reads the code") && m.contains("connect (bagla)"), "{m}");
}
