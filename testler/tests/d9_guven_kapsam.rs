//! D9 güven (kapsam): rota kaynağı bulunup okunamayan yer uyarısız geçmez. Adaptör, uygulamaya bağlanan ama rotası
//! okunamayan router'ı, çözülemeyen rota yolunu ve tanınmayan rota dekoratörünü sayar; `init`, `rapor`, `doktor`, Ayna
//! kapsam kartı ve Benlik Kartı "okunamayan rota kaynağı: N yer (dosya:satır)" der ve kapsamı "tam" değil "kısmen"
//! sayar. FastAPI sınıf tabanlı görünümleri (`@controller(router)`, fastapi-utils `cbv`) rota olarak okunur: yol öneki
//! router'dan, sınıfın `Depends(...)` öznitelikleri her ucun bağımlılığıdır.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d9guvenk-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = sezgi(&["init", ornek(ad).to_str().unwrap(), "--cikti", cikti.to_str().unwrap(), "--paylasimsiz"]);
    assert!(c.0, "init: {}", c.1);
    cikti
}

/// (başarılı mı, stdout + stderr)
fn sezgi(args: &[&str]) -> (bool, String) {
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(args).env("SEZGI_ANAHTARLIK", "kapali").output().unwrap();
    (c.status.success(), format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr)))
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn kapi<'a>(m: &'a Value, id: &str) -> &'a Value {
    m["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).unwrap_or_else(|| {
        panic!("{id} yok: {:?}", m["endpointler"].as_array().unwrap().iter().map(|k| k["id"].as_str().unwrap()).collect::<Vec<_>>())
    })
}

#[test]
fn sinif_tabanli_gorunum_yontemleri_rota_olarak_okunur() {
    let h = haritala("d9_guven_cbv");
    let m = json_oku(&h.join("kapilar.json"));
    for id in ["GET /api/recipes", "POST /api/recipes", "DELETE /api/recipes/:slug"] {
        let k = kapi(&m, id);
        // Sınıfın taban sınıfındaki `user: User = Depends(get_current_user)`: OAuth2PasswordBearer'a ulaşır, giriş kesin
        assert_eq!(k["giris"], "gerekli", "{id}: {k:#}");
        assert_eq!(k["giris_kaniti"], "kesin", "{id}: {k:#}");
        assert_eq!(k["dosya"], "app/routes/recipes.py", "{k:#}");
    }
    // Gövde: yöntemin `data: RecipeIn` parametresi (self girdi değildir)
    let p = kapi(&m, "POST /api/recipes");
    assert_eq!(p["govde"][0]["sema"]["dto"], "RecipeIn", "{p:#}");
    assert!(!p.to_string().contains("\"self\""), "{p:#}");
    // Rota dekoratörü olmayan yardımcı yöntem işlem değildir
    assert!(!m["endpointler"].as_array().unwrap().iter().any(|k| k["fonksiyon"] == "helper" || k["fonksiyon"] == "_helper"));
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn okunamayan_rota_kaynagi_her_yuzeyde_uyarilir() {
    let h = haritala("d9_guven_cbv");
    let m = json_oku(&h.join("kapilar.json"));
    let o: Vec<&str> = m["ozet"]["okunamayan_rotalar"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert!(o.iter().any(|x| x.starts_with("app/routes/dinamik.py:")), "döngüyle kurulan rota okunamadı diye sayılmalı: {o:?}");
    // rapor: kapsam kısmen, yer listesi
    let (ok, r) = sezgi(&["rapor", h.to_str().unwrap()]);
    assert!(ok, "{r}");
    assert!(r.contains("kapsam kısmen") && r.contains("okunamayan rota kaynağı: ") && r.contains("app/routes/dinamik.py:"), "{r}");
    let (_, rj) = sezgi(&["rapor", h.to_str().unwrap(), "--json"]);
    let rj: Value = serde_json::from_str(&rj).unwrap();
    assert_eq!(rj["kapilar"]["kapsam"], "kismen", "{:#}", rj["kapilar"]);
    assert!(!rj["kapilar"]["okunamayan_rota_kaynaklari"].as_array().unwrap().is_empty());
    // İngilizce rapor
    let (_, re) = sezgi(&["--lang", "en", "rapor", h.to_str().unwrap()]);
    assert!(re.contains("coverage partial") && re.contains("unreadable route source"), "{re}");
    // Benlik Kartı: listede olmayan işlem "yok" demek değildir
    let kart = json_oku(&h.join("benlik.json"));
    let x = &kart["x-unread_route_sources"];
    assert!(x["count"].as_u64().unwrap() >= 1, "{x:#}");
    assert_eq!(x["coverage"], "partial");
    assert!(x["sources"].as_array().unwrap().iter().any(|s| s.as_str().unwrap().starts_with("app/routes/dinamik.py:")), "{x:#}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn init_ve_doktor_kapsami_kismen_der() {
    let kok = std::env::temp_dir().join(format!("sezgi-d9guvenk-proje-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    kopyala(&ornek("d9_guven_cbv"), &kok);
    let (ok, i) = sezgi(&["init", kok.to_str().unwrap(), "--paylasimsiz"]);
    assert!(ok, "{i}");
    assert!(i.contains("okunamayan rota kaynağı: ") && i.contains("app/routes/dinamik.py:"), "init özeti uyarmalı: {i}");
    let (_, d) = sezgi(&["doktor", kok.to_str().unwrap()]);
    let harita = d.lines().find(|l| l.contains("Harita")).unwrap_or_else(|| panic!("{d}"));
    assert!(harita.trim_start().starts_with('!') && harita.contains("kapsam kısmen"), "doktor ✓ demez: {d}");
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn okunabilen_uygulamada_kapsam_tam() {
    let h = haritala("d9_dogruluk_fastapi");
    let m = json_oku(&h.join("kapilar.json"));
    assert!(m["ozet"]["okunamayan_rotalar"].as_array().unwrap().is_empty(), "{:#}", m["ozet"]["okunamayan_rotalar"]);
    let (_, rj) = sezgi(&["rapor", h.to_str().unwrap(), "--json"]);
    let rj: Value = serde_json::from_str(&rj).unwrap();
    assert_eq!(rj["kapilar"]["kapsam"], "tam");
    assert!(json_oku(&h.join("benlik.json"))["x-unread_route_sources"].is_null());
    let _ = std::fs::remove_dir_all(&h);
}

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        let p = g.path();
        let h = hedef.join(g.file_name());
        if p.is_dir() {
            kopyala(&p, &h);
        } else {
            std::fs::copy(&p, &h).unwrap();
        }
    }
}
