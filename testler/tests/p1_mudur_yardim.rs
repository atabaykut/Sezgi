//! Yardım açıklamalarının ilk harfi Türkçe kurala göre büyür: "insan" → "İnsan" (noktasız "Insan" değil)
use std::process::Command;

#[test]
fn yardim_ilk_harf_turkce_buyur() {
    let o = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["yardim", "onay"]).output().unwrap();
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("İnsan onayı"), "{s}");
    assert!(!s.contains("Insan"), "{s}");
}
