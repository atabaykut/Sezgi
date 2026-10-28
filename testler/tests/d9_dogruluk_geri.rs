//! D9 doğruluk: geri alma eşlemesi işlem türüne göre. Güncellemenin tersi silme değildir (aynı yolda DELETE yalnızca
//! ekleme / işaretlemenin tersidir); koleksiyonun toplu silmesi tek kaydı oluşturan işlemin tersi sayılmaz; tekil
//! kaynakta kurma ↔ silme terstir. Emin olunamayan yerde `bilinmiyor`.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d9dog-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", ornek(ad).to_str().unwrap(), "--cikti", cikti.to_str().unwrap(), "--paylasimsiz"])
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    assert!(c.status.success(), "init: {}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    cikti
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn dna(h: &Value, id: &str) -> Value {
    let k = h["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).cloned().unwrap_or_else(|| panic!("{id} yok"));
    let yer = format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]);
    h["ozet"]["dna"].get(&yer).cloned().unwrap_or_else(|| panic!("{id} ({yer}) için dna yok"))
}

fn gerekce(d: &Value) -> String {
    d["geri_alma_gerekce"].as_str().unwrap_or("").to_string()
}

#[test]
fn guncellemenin_tersi_silme_degildir() {
    let h = haritala("d9_dogruluk_geri");
    let m = json_oku(&h.join("kapilar.json"));
    for id in ["PUT /api/kisiler", "PATCH /api/varliklar", "PATCH /api/kullanicilar/me", "PUT /api/pin-kodu"] {
        let d = dna(&m, id);
        assert!(!gerekce(&d).contains("ters işlem: DELETE"), "{id}: güncelleme silmeyle geri alınmaz: {d:#}");
        assert_eq!(d["geri_alinabilir"], "bilinmiyor", "{id}: {d:#}");
    }
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn ekleme_isaretleme_ve_tekil_kaynak_tersleri_korunur() {
    let h = haritala("d9_dogruluk_geri");
    let m = json_oku(&h.join("kapilar.json"));
    let a = dna(&m, "PUT /api/albumler/:id/varliklar");
    assert_eq!(a["geri_alinabilir"], "evet", "{a:#}");
    assert!(gerekce(&a).contains("ters işlem: DELETE /api/albumler/:id/varliklar"), "{a:#}");
    let t = dna(&m, "POST /api/profiller/:ad/takip");
    assert!(gerekce(&t).contains("ters işlem: DELETE /api/profiller/:ad/takip"), "{t:#}");
    let p = dna(&m, "POST /api/pin-kodu");
    assert!(gerekce(&p).contains("DELETE /api/pin-kodu"), "{p:#}");
    assert_eq!(p["geri_alinabilir"], "evet", "{p:#}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn koleksiyonun_toplu_silmesi_olusturmanin_tersi_degildir() {
    let h = haritala("d9_dogruluk_geri");
    let m = json_oku(&h.join("kapilar.json"));
    // Tek kaydı silen uç varsa o
    let o = dna(&m, "POST /api/oturumlar");
    assert!(gerekce(&o).contains("DELETE /api/oturumlar/:id"), "{o:#}");
    assert_eq!(o["geri_alinabilir"], "evet", "{o:#}");
    // Yalnızca toplu silme varsa bilinmiyor
    let e = dna(&m, "POST /api/etiketler");
    assert!(!gerekce(&e).contains("DELETE /api/etiketler"), "{e:#}");
    assert_eq!(e["geri_alinabilir"], "bilinmiyor", "{e:#}");
    let _ = std::fs::remove_dir_all(&h);
}
