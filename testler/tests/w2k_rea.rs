//! W2 REA kabul testi (gerçek rea-agents@6.7.0 ile). Varsayılan çalıştırmada atlanır (#[ignore]): REA ve Node 24.15+
//! gerektirir. Çalıştırma: test görüntüsüne `/rea` (npm i rea-agents@6.7.0 yapılmış klasör) ve `/n/node` bağlanır,
//! `cargo test --test w2k_rea -- --ignored`.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const REA: &str = "/rea/node_modules/.bin/rea";
const NODE: &str = "/n/node";

fn hazir() -> bool {
    let var = Path::new(REA).exists() && Path::new(NODE).exists();
    if !var {
        eprintln!("REA ya da Node bağlı değil ({REA}, {NODE}): atlandı");
    }
    var
}

fn yayin(ad: &str) -> PathBuf {
    let k = std::env::temp_dir().join(format!("sezgi-w2k-rea-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    std::fs::create_dir_all(&k).unwrap();
    let kaynak = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler/w2k_otelrez/yayin");
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        std::fs::copy(g.path(), k.join(g.file_name())).unwrap();
    }
    k
}

fn kesfet(y: &Path, ortam: &[(&str, &str)]) -> (Value, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
    c.args(["kesfet", y.to_str().unwrap(), "--rea"]).env("SEZGI_REA", REA).env("SEZGI_NODE", NODE);
    for (a, d) in ortam {
        c.env(a, d);
    }
    let o = c.output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "{out}");
    (serde_json::from_str(&std::fs::read_to_string(y.join(".sezgi/kaynaksiz.json")).unwrap()).unwrap(), out)
}

#[test]
#[ignore]
fn w2k_rea_net_uyeleri_gelir_onizleme_asla_gecer_degil() {
    if !hazir() {
        return;
    }
    let y = yayin("acik");
    let (b, out) = kesfet(&y, &[]);
    assert_eq!(b["rea"]["durum"], "acik", "{out}");
    assert_eq!(b["rea"]["surum"], "6.7.0");
    let rea: Vec<&Value> = b["kanitlar"].as_array().unwrap().iter().filter(|k| k["uretici"].as_str().unwrap().starts_with("rea@6.7.0/")).collect();
    assert!(rea.len() >= 3, "{out}");
    let bul = |konum: &str| rea.iter().find(|k| k["konum"][0] == konum).unwrap_or_else(|| panic!("{konum} yok"));
    // Üyeler ve imzalar (BINARY_METADATA, observed)
    let yapi = bul("OtelRez.dll#inspect_managed_members");
    assert_eq!((yapi["kaynak_turu"].as_str(), yapi["beyan"].as_str()), (Some("BINARY_METADATA"), Some("observed")));
    assert!(yapi["kaynak_kanit"].as_str().unwrap().starts_with("ev_"));
    let tipler = yapi["ozet"]["tipler"].to_string();
    for t in ["OtelRez.Rol", "OtelRez.Rezervasyon", "OtelRez.IptalKurali", "OtelRez.Depo", "OtelRez.Program"] {
        assert!(tipler.contains(t), "{t}: {tipler}");
    }
    assert!(yapi["ozet"]["uyeler"].as_array().unwrap().iter().any(|u| u["ad"] == "OtelRez.IptalKurali.IptalEt" && u["parametreler"].as_array().unwrap().len() == 4));
    // Kesin çağrı hedefi: Main → IptalEt
    let cagri = bul("OtelRez.dll#inspect_managed_members:cagrilar");
    assert_eq!(cagri["iddia_turu"], "call_target");
    assert!(cagri["ozet"]["cagrilar"].as_array().unwrap().iter().any(|c| c["cagiran"] == "OtelRez.Program.Main" && c["hedef"] == "OtelRez.IptalKurali.IptalEt"));
    // SQL biçimi (BINARY_HEURISTIC, inferred): işlem ve tablo var, değer yok
    let sql = bul("OtelRez.dll#inspect_managed_members:sql");
    assert_eq!((sql["kaynak_turu"].as_str(), sql["beyan"].as_str()), (Some("BINARY_HEURISTIC"), Some("inferred")));
    assert!(sql["ozet"]["sql"].as_array().unwrap().iter().any(|s| s["islem"] == "UPDATE" && s["tablolar"][0] == "rezervasyon"));
    // P/Invoke
    let sinir = bul("OtelRez.dll#inspect_managed_native_boundaries");
    assert_eq!(sinir["ozet"]["pinvoke"][0]["dll"], "e_sqlite3");
    assert_eq!(sinir["ozet"]["pinvoke"][0]["giris"], "sqlite3_open");
    // Politika: açık kapsam, hiçbiri certain değil, önizleme asla GEÇER değil
    for k in b["kanitlar"].as_array().unwrap() {
        assert_eq!(k["kapsam"], "acik");
        assert_ne!(k["beyan"], "certain");
    }
    assert_eq!(b["onizleme"]["gecer"], false);
    assert!(out.contains("hiçbir zaman GEÇER değil"));
    // Değer projeksiyonu: literal değerleri ve REA'nın yerel yolları kanıta girmez; ham çıktı saklanmaz
    let metin = std::fs::read_to_string(y.join(".sezgi/kaynaksiz.json")).unwrap();
    for yok in ["yalnizca kendi rezervasyonunuz", "rezervasyon aktif degil", "WHERE id = $id", "local_path", "ORNEK_GIZLI_DEGER"] {
        assert!(!metin.contains(yok), "{yok} kanıtta olmamalı");
    }
    let artik: Vec<_> = std::fs::read_dir(std::env::temp_dir()).unwrap().flatten().filter(|g| g.file_name().to_string_lossy().starts_with("sezgi-rea-")).collect();
    assert!(artik.is_empty(), "geçici REA çıktıları silinmeli: {artik:?}");
    assert!(!b["bilinmeyenler"].to_string().contains("REA kapalı"), "{}", b["bilinmeyenler"]);
    // Yerel PE (apphost) için native analiz motoru yok: dürüstçe söylenir (W3)
    assert!(b["bilinmeyenler"].to_string().contains("motor kurulu değil"), "{}", b["bilinmeyenler"]);
    assert!(b["bilinmeyenler"].to_string().contains("iş kuralları"), "iş kuralları decompile olmadan bilinmiyor");
}

#[test]
#[ignore]
fn w2k_rea_zaman_siniri_asilinca_durdurulur() {
    if !hazir() {
        return;
    }
    let y = yayin("sure");
    // Kendi geçici klasörü: öbür testin "geçici çıktı kalmadı" denetimini etkilemesin
    let tmp = y.join("tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let (b, out) = kesfet(&y, &[("SEZGI_REA_SURE", "0"), ("TMPDIR", tmp.to_str().unwrap())]);
    assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0, "durdurulan sürecin çıktısı silinmeli");
    assert!(b["rea"]["durum"] == "kapali" || b["bilinmeyenler"].to_string().contains("durduruldu"), "{out}");
    assert!(b["kanitlar"].as_array().unwrap().iter().all(|k| !k["uretici"].as_str().unwrap().starts_with("rea")));
    assert_eq!(b["kanitlar"].as_array().unwrap().iter().find(|k| k["konum"][0] == "OtelRez.dll").unwrap()["ozet"]["dotnet"], true);
}
