//! P0.4 önizleme dürüstlüğü (DNA D0): önizlemenin sınayamadığı kurallar sessizce atlanmaz.
//! - Nesne düzeyi zod kuralı (superRefine / refine / transform / check): "GEÇER" denmez, "nesne düzeyi kural" bildirilir
//!   (Immich shared-links `{type:"ALBUM"}` albumId'siz deseni).
//! - Dosya yükleme (UploadedFile(s), ParseFilePipe, dosya interceptor'ı; FastAPI UploadFile / File): "dosya doğrulaması
//!   kontrol edilemedi" bildirilir (Immich dosyasız asset upload deseni).
//! - "Kontrol edilemedi" listesi en fazla 5 madde gösterir, yürütmeyi etkileyenler (yetki) önce, aynı nedenler birleşik.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

/// Her çağrı kendi çıktı klasörünü alır: aynı süreçteki paralel testler birbirinin haritasını silmesin
fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-p0g2-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", ornek(ad).to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "init: {}", String::from_utf8_lossy(&c.stderr));
    cikti
}

fn onizle(harita: &Path, arguman: Value) -> String {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", harita.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

fn sonuc(metin: &str) -> &str {
    metin.lines().find_map(|l| l.strip_prefix("sonuç: ")).unwrap_or("")
}

fn soru_satirlari(metin: &str) -> Vec<&str> {
    metin.lines().filter(|l| l.starts_with("  ? ")).collect()
}

#[test]
fn nesne_duzeyi_zod_kurali_gecer_dedirtmez() {
    let h = haritala("p0_g2_onizle");
    let m = onizle(&h, json!({ "kapi": "POST /paylasimlar", "oturum": true, "govde": { "type": "ALBUM" } }));
    assert!(!sonuc(&m).starts_with("GEÇER"), "superRefine sınanmadan GEÇER denmemeli:\n{m}");
    assert!(m.contains("nesne düzeyi kural"), "nesne düzeyi kural bildirilmeli:\n{m}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn dosya_yuklemesi_gecer_dedirtmez() {
    let h = haritala("p0_g2_onizle");
    // Gövde alanları eksiksiz, dosya yok
    let m = onizle(&h, json!({ "kapi": "POST /varliklar", "oturum": true, "govde": { "deviceId": "telefon" } }));
    assert!(!sonuc(&m).starts_with("GEÇER"), "dosya sınanmadan GEÇER denmemeli:\n{m}");
    assert!(m.contains("dosya doğrulaması kontrol edilemedi"), "dosya doğrulaması bildirilmeli:\n{m}");
    // Gövdesiz, yalnızca dosya alan kapı
    let m = onizle(&h, json!({ "kapi": "POST /varliklar/tek", "oturum": true }));
    assert!(!sonuc(&m).starts_with("GEÇER"), "dosya sınanmadan GEÇER denmemeli:\n{m}");
    assert!(m.contains("dosya doğrulaması kontrol edilemedi"), "dosya doğrulaması bildirilmeli:\n{m}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn fastapi_dosya_parametresi_bildirilir() {
    let h = haritala("p0_g2_fastapi_dosya");
    for kapi in ["POST /yukle", "POST /ham"] {
        let m = onizle(&h, json!({ "kapi": kapi, "oturum": true }));
        assert!(!sonuc(&m).starts_with("GEÇER"), "{kapi}: dosya sınanmadan GEÇER denmemeli:\n{m}");
        assert!(m.contains("dosya doğrulaması kontrol edilemedi"), "{kapi}: dosya doğrulaması bildirilmeli:\n{m}");
    }
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn kontrol_edilemedi_listesi_kisa_ve_oncelikli() {
    let h = haritala("p0_g2_onizle");
    let govde: serde_json::Map<String, Value> = ["a", "b", "c", "d", "e", "f", "g"].iter().map(|a| (a.to_string(), json!("x"))).collect();
    // Oturum belirtilmedi: yetki maddesi (yürütmeyi etkiler) listenin başında olmalı
    let m = onizle(&h, json!({ "kapi": "POST /kayitlar", "govde": govde }));
    let sorular = soru_satirlari(&m);
    assert!(sorular.len() <= 5, "en fazla 5 madde gösterilmeli, {} gösterildi:\n{m}", sorular.len());
    assert!(sorular.first().is_some_and(|s| s.contains("yetki")), "yetki maddesi önce gelmeli:\n{m}");
    assert!(m.contains("madde daha"), "gösterilmeyen maddeler sayıyla belirtilmeli:\n{m}");
    assert!(!sonuc(&m).starts_with("GEÇER"), "{m}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn ayni_nedenler_birlesir() {
    let h = haritala("p0_g2_onizle");
    // Oturum ve roller verilmemiş iki kez aynı neden üretmez; alan düzeyinde aynı neden tek satırda birleşir
    let m = onizle(&h, json!({ "kapi": "POST /paylasimlar", "oturum": true, "govde": { "type": "INDIVIDUAL" } }));
    let sorular = soru_satirlari(&m);
    let mut gorulen = std::collections::HashSet::new();
    for s in &sorular {
        let neden = s.split_once(": ").map(|x| x.1).unwrap_or(s);
        assert!(gorulen.insert(neden), "aynı neden iki satırda:\n{m}");
    }
    let _ = std::fs::remove_dir_all(&h);
}
