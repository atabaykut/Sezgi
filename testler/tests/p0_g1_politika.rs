//! P0.7 Kart ↔ yürütme tutarlılığı: onaysız yazma kararını tek fonksiyon verir. Yıkıcı işlem, damgası kesin olmayan
//! işlem (Express'te giriş kararı hep çıkarım) ya da önizlemesi bir kuralı denetleyemeyen istek insan onayına düşer.
//! Kartın `requires_approval` alanı aynı kuralın statik kısmından türer; `identity.model` yazma modelini doğru söyler.
mod p0_g1_ortak;
use p0_g1_ortak::*;

#[test]
fn express_cikarim_damgali_yazma_onaya_duser() {
    let ev = Ev::yeni("politika-express");
    let u = Uygulama::yeni();
    let p = ev.proje("express-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let kart = p.kart();
    let islem = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "POST" && o["entry"]["path"] == "/api/kitaplar").unwrap().clone();
    assert_ne!(islem["stamp"], "certain", "{islem}");
    assert_eq!(islem["requires_approval"], true);
    let t = p.token("ajan", &["kitaplar.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(serde_json::json!({ "kapi": "POST /api/kitaplar", "govde": { "ad": "x" } }));
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "kesin olmayan kanıtla onaysız yazma yok: {s}");
    assert_eq!(u.sayi(), 0);
}

#[test]
fn kesin_ve_kapali_dunyada_gecen_yazma_onaysiz_gider() {
    let ev = Ev::yeni("politika-nest");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let kart = p.kart();
    let bul = |y: &str, yol: &str| kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == y && o["entry"]["path"] == yol).unwrap().clone();
    let yorum = bul("POST", "/api/v1/yorumlar");
    assert_eq!((yorum["stamp"].as_str(), yorum["requires_approval"].as_bool()), (Some("certain"), Some(false)), "{yorum}");
    assert_eq!(bul("DELETE", "/api/v1/kitaplar/:id")["requires_approval"], true, "yıkıcı işlem her zaman onay ister");
    assert_eq!(bul("GET", "/api/v1/kitaplar/:id")["requires_approval"], false);
    assert_eq!(kart["identity"]["model"], "user-session", "yazma insanın kendi oturumuyla yapılır");
    let t = p.token("ajan", &["yorum.*:yaz", "kitap.*:yaz"], &[]);
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(u.sayi(), 1);
    // Kesin damgalı ama önizlemesi bir kuralı denetleyemiyor (ValidateIf): onaya düşer
    let (s, hata) = m.cagir(kitap_olustur());
    assert!(!hata && s.contains("ONAY BEKLİYOR") && s.contains("denetle"), "{s}");
    assert_eq!(u.sayi(), 1);
}

#[test]
fn token_onay_hepsi_yalnizca_sikilastirir() {
    let ev = Ev::yeni("politika-hepsi");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &["--onay", "hepsi"]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "{s}");
    assert_eq!(u.sayi(), 0);
}
