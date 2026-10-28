//! `sezgi olc`: kullanıcının kendi uygulamasında bağlam, hız ve (OpenAPI varsa) doğruluk ölçümü. Yapay zekâ çalıştırmaz,
//! ağa çıkmaz, uygulamaya yazmaz.
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn hazirla(ad: &str) -> PathBuf {
    let k = std::env::temp_dir().join(format!("sezgi-olc-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &k);
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", k.to_str().unwrap()]).output().unwrap().status.success());
    k
}

fn olc(k: &Path, ek: &[&str]) -> serde_json::Value {
    let mut a = vec!["olc", k.to_str().unwrap(), "--json"];
    a.extend_from_slice(ek);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(&a).output().unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    serde_json::from_slice(&c.stdout).unwrap()
}

#[test]
fn baglam_ve_hiz_olculur() {
    let k = hazirla("baglam");
    let v = olc(&k, &[]);
    let kod = v["baglam"]["kod_ortanca_token"].as_u64().unwrap();
    let kart = v["baglam"]["kart_ortanca_token"].as_u64().unwrap();
    assert!(kod > 0 && kart > 0, "{v}");
    assert!(kart < kod, "kart koddan küçük olmalı: {v}");
    assert!(v["hiz"]["p95_ms"].as_f64().unwrap() < 50.0, "{v}");
    assert!(v["dogruluk"].is_null(), "OpenAPI verilmedi: {v}");
    // İnsan çıktısı yapay zekâ çalıştırmadığını söyler
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["olc", k.to_str().unwrap()]).output().unwrap();
    let m = String::from_utf8_lossy(&c.stdout);
    assert!(m.contains("Bağlam") && m.contains("yapay zekâ çalıştırmaz"), "{m}");
    let _ = std::fs::remove_dir_all(&k);
}

#[test]
fn openapi_ile_dogruluk() {
    let k = hazirla("openapi");
    let harita: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(k.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    // Haritadaki her kapı + Sezgi'nin bilmediği bir kapı
    let mut paths = serde_json::Map::new();
    for e in harita["endpointler"].as_array().unwrap() {
        let yol = e["yol"].as_str().unwrap().split('/').map(|p| p.strip_prefix(':').map(|x| format!("{{{x}}}")).unwrap_or(p.to_string())).collect::<Vec<_>>().join("/");
        let guvenlik = if e["giris"] == "herkese_acik" { serde_json::json!([]) } else { serde_json::json!([{ "bearer": [] }]) };
        let girdi = paths.entry(yol).or_insert(serde_json::json!({}));
        girdi[e["yontem"].as_str().unwrap().to_lowercase()] = serde_json::json!({ "responses": { "200": {} }, "security": guvenlik });
    }
    paths.insert("/yok-boyle-kapi".into(), serde_json::json!({ "get": { "responses": { "200": {} }, "security": [] } }));
    let n = harita["endpointler"].as_array().unwrap().len();
    let dosya = k.join("openapi.json");
    std::fs::write(&dosya, serde_json::json!({ "openapi": "3.0.0", "paths": paths }).to_string()).unwrap();
    let v = olc(&k, &["--openapi", dosya.to_str().unwrap()]);
    let d = &v["dogruluk"];
    assert_eq!(d["resmi"].as_u64().unwrap() as usize, n + 1, "{d}");
    assert_eq!(d["bulunan"].as_u64().unwrap() as usize, n, "{d}");
    assert!(d["eksik"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("yok-boyle-kapi")), "{d}");
    assert_eq!(d["giris_dogru"].as_u64().unwrap() as usize, n, "{d}");
    let _ = std::fs::remove_dir_all(&k);
}

#[test]
fn haritasiz_uygulama() {
    let k = std::env::temp_dir().join(format!("sezgi-olc-bos-{}", std::process::id()));
    std::fs::create_dir_all(&k).unwrap();
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["olc", k.to_str().unwrap()]).output().unwrap();
    assert!(!c.status.success());
    assert!(String::from_utf8_lossy(&c.stderr).contains("sezgi init"));
    let _ = std::fs::remove_dir_all(&k);
}
