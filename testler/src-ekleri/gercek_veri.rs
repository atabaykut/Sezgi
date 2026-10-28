use super::*;
use serde_json::json;

/// Gerçek veride saatlik yazma sınırı: yalnızca son bir saatteki yazmalar sayılır; sınıra ulaşınca red
#[test]
fn hiz_siniri_son_saat() {
    let simdi = 1_000_000;
    assert_eq!(hiz_siniri(&[], simdi, 20), Ok(0));
    let eski: Vec<u64> = (0..50).map(|i| simdi - 3600 - i).collect();
    assert_eq!(hiz_siniri(&eski, simdi, 1), Ok(0), "bir saatten eski yazmalar sayılmaz");
    let yeni: Vec<u64> = (0..19).map(|i| simdi - i * 10).collect();
    assert_eq!(hiz_siniri(&yeni, simdi, 20), Ok(19));
    let dolu: Vec<u64> = (0..20).map(|i| simdi - i * 10).collect();
    let e = hiz_siniri(&dolu, simdi, 20).unwrap_err();
    assert!(e.contains("GERÇEK VERİ") && e.contains("sınır 20") && e.contains("--saatlik-yazma"), "{e}");
    assert!(hiz_siniri(&[simdi], simdi, 1).is_err());
}

/// Onay cevabı: yazmada yalnızca tam "evet"; yıkıcıda yöntem ve yol aynen
#[test]
fn onay_cevabi() {
    for c in ["evet", " evet \n"] {
        assert!(gercek_onay_cevabi("yaz", "POST", "/api/v1/yorumlar", c), "{c:?}");
    }
    for c in ["e", "E", "y", "yes", "Evet", "EVET", "", "evet evet"] {
        assert!(!gercek_onay_cevabi("yaz", "POST", "/api/v1/yorumlar", c), "kısaltma ya da başka cevap kabul edilmemeli: {c:?}");
    }
    assert!(gercek_onay_cevabi("yikici", "DELETE", "/api/v1/kitaplar/7", "DELETE /api/v1/kitaplar/7"));
    assert!(gercek_onay_cevabi("yikici", "delete", "/api/v1/kitaplar/7", "  DELETE   /api/v1/kitaplar/7 "));
    for c in ["evet", "e", "DELETE", "/api/v1/kitaplar/7", "DELETE /api/v1/kitaplar/8", "delete /api/v1/kitaplar/7"] {
        assert!(!gercek_onay_cevabi("yikici", "DELETE", "/api/v1/kitaplar/7", c), "yıkıcıda yöntem ve yol aynen yazılmalı: {c:?}");
    }
}

/// Gerçek veriyi açmak: uygulamanın adı aynen
#[test]
fn acma_cevabi() {
    assert!(gercek_ac_cevabi("kitapci", "kitapci\n"));
    for c in ["", "evet", "Kitapci", "kitapc", "kitapci2"] {
        assert!(!gercek_ac_cevabi("kitapci", c), "{c:?}");
    }
    assert!(!gercek_ac_cevabi("", ""), "adsız uygulama boş cevapla açılmaz");
}

/// Eski (bayraksız) token gerçek veride yazamaz
#[test]
fn token_bayragi() {
    assert!(token_gercek_veri(&json!({ "ad": "eski" })).unwrap_err().contains("yalnızca okur"));
    assert!(token_gercek_veri(&json!({ "ad": "eski", "gercek_veri": "true" })).is_err());
    assert!(token_gercek_veri(&json!({ "ad": "yeni", "gercek_veri": true })).is_ok());
}
