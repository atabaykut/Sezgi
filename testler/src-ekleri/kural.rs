use super::*;
use serde_json::json;

#[test]
fn class_validator_ve_zod() {
    assert_eq!(coz("MaxLength(50)", "string"), vec![Kural::MaxKarakter(50.0)]);
    assert_eq!(coz("Length(2, 10)", "string"), vec![Kural::MinKarakter(2.0), Kural::MaxKarakter(10.0)]);
    assert_eq!(coz("Min(1, { message: \"x\" })", "number"), vec![Kural::Min(1.0)]);
    assert_eq!(coz("min(8)", "string"), vec![Kural::MinUzunluk(8.0)]);
    assert_eq!(coz("min(1)", "int"), vec![Kural::Min(1.0)]);
    assert_eq!(coz("min(2)", "array"), vec![Kural::DiziMin(2.0)]);
    assert_eq!(coz("Max(MAX_SEATS)", "number"), vec![Kural::Bilinmiyor("Max(MAX_SEATS)".into())]);
    assert_eq!(coz("regex(pinCodeRegex)", "string"), vec![Kural::Bilinmiyor("regex(pinCodeRegex)".into())]);
    assert_eq!(coz("Matches(/^[a-z]+$/i)", "string"), vec![Kural::Desen("(?i)^[a-z]+$".into())]);
}

#[test]
fn iso_tarih() {
    let h = IsoTur::Herhangi;
    assert!(iso_uygun("2024-02-29", h) && iso_uygun("2024-02-29T10:00:00+03:00", h));
    // Ay, gün, saat aralıkları; Unicode rakam kabul edilmez
    assert!(!iso_uygun("2020-13-45", h) && !iso_uygun("2020-01-01T25:00", h) && !iso_uygun("٢٠٢٤-01-01", h));
    assert!(iso_uygun("2024-01-01", IsoTur::YalnizTarih) && !iso_uygun("2024-01-01T00:00Z", IsoTur::YalnizTarih));
    let zod = IsoTur::TarihSaat { ofset: false, yerel: false };
    assert!(iso_uygun("2024-01-01T00:00:00Z", zod));
    assert!(!iso_uygun("2024-01-01", zod) && !iso_uygun("2024-01-01T00:00:00+03:00", zod) && !iso_uygun("2024-01-01T00:00:00", zod));
    assert!(iso_uygun("2024-01-01T00:00:00+03:00", IsoTur::TarihSaat { ofset: true, yerel: false }));
    assert_eq!(coz("IsNotEmpty()", "string[]"), vec![Kural::BosMetinOlmaz]);
    assert!(sina(&Kural::BosMetinOlmaz, &json!([]), false).is_ok());
}

#[test]
fn sinama() {
    assert!(sina(&Kural::MaxUzunluk(3.0), &json!("abcd"), false).is_err());
    // Aynı emoji: zod 2 birim sayar, class-validator 1 karakter; VS16 (U+FE0F) class-validator'da sayılmaz
    assert!(sina(&Kural::MaxUzunluk(1.0), &json!("😀"), false).is_err());
    assert!(sina(&Kural::MaxKarakter(1.0), &json!("😀"), false).is_ok());
    assert!(sina(&Kural::MaxKarakter(1.0), &json!("❤\u{FE0F}"), false).is_ok());
    assert!(sina(&Kural::MinKarakter(2.0), &json!("😀"), false).is_err());
    assert!(sina(&Kural::Min(1.0), &json!("0"), true).is_err());
    assert!(sina(&Kural::Min(1.0), &json!("5"), true).is_ok());
    assert!(sina(&Kural::Eposta, &json!("a@b.co"), false).is_ok());
    assert!(sina(&Kural::Eposta, &json!("ab.co"), false).is_err());
    assert!(tip_sina("integer", &json!(1.5), false).is_err());
    assert!(tip_sina("integer", &json!("7"), true).is_ok());
    assert!(tip_sina("integer", &json!("7"), false).is_err());
}
