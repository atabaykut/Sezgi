//! GitHub iş akışlarının güvenlik yapısı (yerelde çalıştırılamadıkları için metin olarak denetlenir):
//! - kimlik belirteci izni (id-token: write) üçüncü taraf kodun derlendiği / çalıştığı işte olmaz;
//! - yapı kanıtı (attestation) üreten iş yalnızca derlenmiş çıktıyı indirir, cargo çalıştırmaz;
//! - sürüm, CI (testler, cargo-deny, belge tutarlılığı) başarıyla geçmeden oluşturulamaz;
//! - bütün eylemler commit SHA'sıyla sabitlenmiştir.
use std::collections::BTreeMap;
use std::path::PathBuf;

fn oku(ad: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github").join("workflows").join(ad)).unwrap()
}

/// `jobs:` altındaki her işin metni (iki boşlukla girintili iş adına göre)
fn isler(metin: &str) -> BTreeMap<String, String> {
    let mut isler = BTreeMap::new();
    let mut ad: Option<String> = None;
    let mut icinde = false;
    for satir in metin.lines() {
        if satir.starts_with("jobs:") {
            icinde = true;
            continue;
        }
        if !icinde {
            continue;
        }
        if let Some(a) = satir.strip_prefix("  ").filter(|s| !s.starts_with(' ') && !s.starts_with('#') && s.trim_end().ends_with(':')) {
            ad = Some(a.trim_end().trim_end_matches(':').to_string());
            isler.insert(ad.clone().unwrap(), String::new());
            continue;
        }
        if let Some(a) = &ad {
            let m = isler.get_mut(a).unwrap();
            m.push_str(satir);
            m.push('\n');
        }
    }
    isler
}

#[test]
fn surum_is_akisi_izinleri_ayri() {
    let r = oku("release.yml");
    let isler = isler(&r);
    for (ad, metin) in &isler {
        let derleme = metin.contains("cargo ") || metin.contains("uses: ./.github/workflows/");
        assert!(!(derleme && metin.contains("id-token")), "'{ad}' işi hem kod derliyor/çalıştırıyor hem id-token izni taşıyor");
    }
    let kanit = isler.values().find(|m| m.contains("attest-build-provenance")).expect("yapı kanıtı işi yok");
    assert!(kanit.contains("id-token: write") && kanit.contains("download-artifact") && !kanit.contains("cargo "), "{kanit}");
    // Sürüm CI'ya bağlı: ci.yml yeniden kullanılabilir iş olarak çağrılır ve derleme ona bağlıdır
    let ci = isler.iter().find(|(_, m)| m.contains("uses: ./.github/workflows/ci.yml")).map(|(a, _)| a.clone()).expect("sürüm iş akışı CI'yı çağırmıyor");
    let derle = isler.values().find(|m| m.contains("cargo build")).expect("derleme işi yok");
    assert!(derle.contains(&format!("needs: {ci}")) || derle.contains(&format!("needs: [{ci}")), "derleme CI'ya bağlı değil:\n{derle}");
    assert!(oku("ci.yml").contains("workflow_call:"), "ci.yml çağrılabilir değil");
    // Taslak sürüm yapı kanıtından sonra
    let surum = isler.values().find(|m| m.contains("gh release create")).expect("sürüm işi yok");
    assert!(surum.contains("needs:") && surum.contains("attest"), "{surum}");
}

#[test]
fn eylemler_sha_ile_sabit() {
    for ad in ["ci.yml", "release.yml"] {
        for satir in oku(ad).lines().filter(|l| l.trim_start().starts_with("- uses:") || l.trim_start().starts_with("uses:")) {
            let deger = satir.split("uses:").nth(1).unwrap().trim();
            if deger.starts_with("./") {
                continue;
            }
            let sha = deger.split('@').nth(1).unwrap_or("").split_whitespace().next().unwrap_or("");
            assert!(sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()), "{ad}: SHA ile sabitlenmemiş eylem: {satir}");
        }
    }
}

#[test]
fn run_satirlari_gecerli_yaml() {
    // Tırnaksız tek satırlık `run:` değerinde ": " YAML eşlemesi sayılır ve iş akışını geçersiz kılar
    // (0.1.0 adayının release.yml dosyası bu yüzden hiç çalışmayacaktı)
    for ad in ["ci.yml", "release.yml"] {
        for (i, satir) in oku(ad).lines().enumerate() {
            let Some(deger) = satir.trim_start().strip_prefix("run:").or_else(|| satir.trim_start().strip_prefix("- run:")) else { continue };
            let deger = deger.trim();
            if deger.starts_with(['|', '>', '"', '\'']) {
                continue;
            }
            assert!(!deger.contains(": ") && !deger.contains(" #"), "{ad}:{}: tırnaksız run değerinde ': ' ya da ' #': {satir}", i + 1);
        }
    }
}
