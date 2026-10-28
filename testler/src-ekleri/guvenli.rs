use super::*;

#[test]
fn gorunmez_karakter_siniflari() {
    // Her sınıf: insan görmez ya da metnin yönünü değiştirir, yapay zekâ okur
    let siniflar: [(&str, &[char]); 10] = [
        ("Unicode Tag (ASCII kaçakçılığı)", &['\u{E0001}', '\u{E0041}', '\u{E0069}', '\u{E007F}']),
        ("yumuşak tire", &['\u{00AD}']),
        ("varyasyon seçicileri", &['\u{FE00}', '\u{FE0F}', '\u{E0100}', '\u{E01EF}', '\u{180B}']),
        ("sıfır genişlikli", &['\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{2064}', '\u{FEFF}', '\u{034F}']),
        ("yön değiştirme", &['\u{202A}', '\u{202E}', '\u{2066}', '\u{2069}', '\u{061C}', '\u{200E}', '\u{200F}']),
        ("diğer biçim karakterleri (Cf)", &['\u{180E}', '\u{0600}', '\u{06DD}', '\u{FFF9}', '\u{FFFB}', '\u{1D173}', '\u{13430}']),
        ("Hangul dolgu", &['\u{115F}', '\u{1160}', '\u{3164}', '\u{FFA0}']),
        ("özel kullanım alanı (Co)", &['\u{E000}', '\u{F8FF}', '\u{F0000}', '\u{10FFFD}']),
        ("karakter olmayanlar", &['\u{FDD0}', '\u{FFFE}', '\u{1FFFF}']),
        ("satır / paragraf ayırıcı ve kontrol", &['\u{2028}', '\u{2029}', '\n', '\u{7}', '\u{85}']),
    ];
    for (ad, kar) in siniflar {
        for c in kar {
            assert_eq!(temiz(&format!("a{c}b")), "a b", "{ad}: U+{:04X} temizlenmedi", *c as u32);
        }
    }
    // Görünür metin korunur: Türkçe, aksanlı harfler, emoji, CJK
    for s in ["Kullanıcıları listele", "çğıöşü ÇĞİÖŞÜ", "café", "数据", "✓ 😀"] {
        assert_eq!(temiz(s), s);
    }
}

#[test]
fn gorunmez_karakterler() {
    assert_eq!(temiz("a\nb\u{2028}c\u{202E}d\u{200B}e"), "a b c d e");
    let mut v = serde_json::json!({ "x\u{2029}": ["satır\nsahte"] });
    json_temiz(&mut v);
    assert_eq!(v, serde_json::json!({ "x ": ["satır sahte"] }));
}
