//! dil.rs birim testleri: öncelik kuralı, etiket ve yerel ayar çözümlemesi, Windows dil kimliği, çeviri makroları.
use super::*;

#[test]
fn oncelik_bayrak_ortam_sistem_varsayilan() {
    // Bayrak her şeyin önünde
    assert_eq!(belirle(Some("en"), Some("tr"), Some(Dil::Tr)), Dil::En);
    assert_eq!(belirle(Some("tr"), Some("en"), Some(Dil::En)), Dil::Tr);
    // Bayrak yoksa SEZGI_LANG
    assert_eq!(belirle(None, Some("en"), Some(Dil::Tr)), Dil::En);
    assert_eq!(belirle(None, Some("tr_TR"), Some(Dil::En)), Dil::Tr);
    // Tanınmayan SEZGI_LANG sistem diline bırakır
    assert_eq!(belirle(None, Some("de"), Some(Dil::En)), Dil::En);
    assert_eq!(belirle(None, Some(""), Some(Dil::En)), Dil::En);
    // Sistem dili
    assert_eq!(belirle(None, None, Some(Dil::En)), Dil::En);
    // Hiçbiri belirlenemezse Türkçe
    assert_eq!(belirle(None, None, None), Dil::Tr);
    assert_eq!(belirle(None, Some("xx"), None), Dil::Tr);
}

#[test]
fn yerel_ayar_cozumlemesi() {
    assert_eq!(yerel_ayardan("tr_TR.UTF-8"), Some(Dil::Tr));
    assert_eq!(yerel_ayardan("tr"), Some(Dil::Tr));
    assert_eq!(yerel_ayardan("en_US.UTF-8"), Some(Dil::En));
    assert_eq!(yerel_ayardan("de_DE@euro"), Some(Dil::En));
    assert_eq!(yerel_ayardan("fr"), Some(Dil::En));
    // Belirlenemeyenler
    assert_eq!(yerel_ayardan("C"), None);
    assert_eq!(yerel_ayardan("C.UTF-8"), None);
    assert_eq!(yerel_ayardan("POSIX"), None);
    assert_eq!(yerel_ayardan(""), None);
    assert_eq!(yerel_ayardan("  "), None);
}

#[test]
fn windows_dil_kimligi() {
    assert_eq!(windows_dili(0x041f), Some(Dil::Tr)); // tr-TR
    assert_eq!(windows_dili(0x0409), Some(Dil::En)); // en-US
    assert_eq!(windows_dili(0x0407), Some(Dil::En)); // de-DE
    assert_eq!(windows_dili(0), None);
}

#[test]
fn makrolar_bicimle_uyumlu() {
    let n = 3;
    let ad = "x";
    let ttr = format!("{n} işlem {ad} {}", 7);
    let ten = format!("{n} operations {ad} {}", 7);
    let secilen = tf!("{n} işlem {ad} {}", "{n} operations {ad} {}", 7);
    assert!(secilen == ttr || secilen == ten);
    let s: &str = t!("evet", "yes");
    assert_eq!(s == "yes", en());
    assert_eq!(sec("a", "b") == "b", en());
}
