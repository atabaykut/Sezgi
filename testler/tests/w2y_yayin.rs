//! W2 yayın düzeltmeleri:
//! - süresi dolmuş kart (`x-expires_at` geçmiş) canlı modda yazmayı durdurur, okumada uyarır;
//! - ön yüz kendiliğinden bulunurken arka ucun kendisi (React ile e-posta şablonu yazan NestJS uygulaması) ön yüz
//!   sayılmaz; bağı olmayan ön yüz için web-baglari.json yazılmaz ve "Ön yüz" satırı basılmaz;
//! - öneri metinleri komutların güncel adını söyler (`sezgi bagla`, `add mcp` değil).
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn kartin_suresini_bitir(p: &Proje) {
    let yol = p.klasor.join(".sezgi").join("benlik.json");
    let mut k = p.kart();
    k["x-issued_at"] = serde_json::json!("2020-01-01T00:00:00Z");
    k["x-expires_at"] = serde_json::json!("2020-01-31T00:00:00Z");
    std::fs::write(yol, serde_json::to_string_pretty(&k).unwrap()).unwrap();
}

#[test]
fn w2y_suresi_dolmus_kart_yazmayi_durdurur_okumada_uyarir() {
    let ev = Ev::yeni("w2y-sure");
    let p = ev.proje("nestjs-temel", "uyg");
    let u = Uygulama::yeni();
    p.kur(&u, "oturum-degeri");
    let t = p.token("ajan", &["yorum.*:yaz", "kitap.*:oku"], &[]);
    // Kontrol: süre içindeki kartla yazma iletilir
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.starts_with("YAZILDI"), "{s}");
    assert_eq!(u.sayi(), 1);
    kartin_suresini_bitir(&p);
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(serde_json::json!({ "kapi": "POST /api/v1/yorumlar", "govde": { "metin": "ikinci", "puan": 5, "kitapId": "8a3c1e2b-1111-2222-3333-444455556666" } }));
    assert!(hata && s.contains("kartın süresi doldu") && s.contains("sezgi init"), "{s}");
    assert_eq!(u.sayi(), 1, "süresi dolmuş kartla yazma uygulamaya gitmemeli");
    // Okuma iletilir ama uyarılır
    let (s, hata) = m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(!hata && s.contains("OKUNDU") && s.contains("kartın süresi doldu"), "{s}");
    assert_eq!(u.sayi(), 2);
}

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
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

/// Örneği geçici klasöre kopyalar, `uygulama` alt yolunda init çalıştırır: (init çıktısı, .sezgi klasörü)
fn init(ad: &str, ornek_ad: &str, uygulama: &str) -> (String, PathBuf, PathBuf) {
    let kok = std::env::temp_dir().join(format!("sezgi-w2y-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    kopyala(&ornek(ornek_ad), &kok);
    let ev = kok.join("_ev");
    std::fs::create_dir_all(&ev).unwrap();
    let k = kok.join(uygulama);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", "."])
        .current_dir(&k)
        .env("HOME", &ev)
        .env("USERPROFILE", &ev)
        .env("APPDATA", &ev)
        .env("XDG_CONFIG_HOME", &ev)
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(c.status.success(), "{m}");
    (m, k.join(".sezgi"), kok)
}

fn ozet(sezgi: &Path) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(sezgi.join("kapilar.json")).unwrap()).unwrap()["ozet"].clone()
}

#[test]
fn w2y_arka_ucun_kendisi_on_yuz_sayilmaz() {
    // NestJS arka ucu react / react-email bağımlılığı ve .tsx e-posta şablonu taşıyor (Immich server gibi)
    let (m, sezgi, kok) = init("arka-uc", "w2y_arka_uc", ".");
    assert!(!sezgi.join("web-baglari.json").exists(), "arka uç ön yüz sayıldı: {m}");
    assert!(!m.contains("Ön yüz"), "{m}");
    assert!(ozet(&sezgi)["on_yuz"].is_null(), "{}", ozet(&sezgi));
    let _ = std::fs::remove_dir_all(kok);
}

#[test]
fn w2y_bagi_olmayan_on_yuz_icin_dosya_ve_satir_yok() {
    // Yandaki web/ bir React ön yüzü ama hiçbir API çağırmıyor: kendiliğinden bulunduğunda sessiz kalınır
    let (m, sezgi, kok) = init("bos", "w2y_bos_on_yuz", "api");
    assert!(!sezgi.join("web-baglari.json").exists(), "{m}");
    assert!(!m.contains("Ön yüz"), "{m}");
    assert!(ozet(&sezgi)["on_yuz"].is_null(), "{}", ozet(&sezgi));
    let _ = std::fs::remove_dir_all(kok);
}

#[test]
fn w2y_fastapi_arka_ucu_yandaki_frontend_i_bulur() {
    let (m, sezgi, kok) = init("fastapi", "s4_fastapi_react", "backend");
    assert!(sezgi.join("web-baglari.json").exists(), "{m}");
    assert!(m.contains("Ön yüz"), "{m}");
    assert_eq!(ozet(&sezgi)["on_yuz"], "../frontend");
    let _ = std::fs::remove_dir_all(kok);
}

#[test]
fn w2y_doktor_guncel_komut_adini_onerir() {
    let (_, _, kok) = init("doktor", "nestjs-temel", ".");
    let ev = kok.join("_ev");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["doktor"])
        .current_dir(&kok)
        .env("HOME", &ev)
        .env("USERPROFILE", &ev)
        .env("APPDATA", &ev)
        .env("XDG_CONFIG_HOME", &ev)
        .output()
        .unwrap();
    let m = String::from_utf8_lossy(&c.stdout).to_string();
    assert!(m.contains("sezgi bagla") && !m.contains("add mcp"), "{m}");
    let _ = std::fs::remove_dir_all(kok);
}
