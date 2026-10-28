//! D9 güven (kalıcı silme): kayıtları kalıcı silen işlem, yöntemi ne olursa olsun (`POST /trash/clear`, `POST
//! /notes/purge`) geri alma kararında "hayır"dır, "kısmen" değil; onay kuralı onu dış etki ya da başka bir gerekçeye
//! bağlı olmadan "geri alınamaz" diye onaya düşürür. Gerekçe iki dilde doğru. Socialite atfı düzeltildikten sonra da
//! onay kalıcı silmeden gelir.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d9guvens-{ad}-{}-{n}", std::process::id()));
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

fn kapi(m: &Value, id: &str) -> Value {
    m["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).cloned().unwrap_or_else(|| panic!("{id} yok"))
}

fn dna(m: &Value, id: &str) -> Value {
    let k = kapi(m, id);
    m["ozet"]["dna"][format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]).as_str()].clone()
}

fn arac(h: &Path, dil: &str, ad: &str, arguman: Value) -> String {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["--lang", dil, "mcp", h.to_str().unwrap()])
        .env("SEZGI_ANAHTARLIK", "kapali")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

#[test]
fn delete_disi_yontemle_kalici_silme_geri_alinamaz_ve_onaya_duser() {
    let h = haritala("d9_guven_cop");
    let m = json_oku(&h.join("kapilar.json"));
    let d = dna(&m, "POST /trash/clear");
    assert_eq!(d["geri_alinabilir"], "hayir", "kalıcı silme 'kısmen' değil: {d:#}");
    let g = d["geri_alma_gerekce"].as_str().unwrap();
    assert!(g.starts_with("kalıcı silme çağrısı: ") && !g.contains("yan etki"), "{g}");
    // Onay gerekçesi kalıcı silmeden; dış etkiye ya da başka bir işlemin kuralına dayanmaz
    let k = kapi(&m, "POST /trash/clear");
    let o = k["etki_onayi"].as_str().unwrap_or_else(|| panic!("kalıcı silme onay ister: {k:#}"));
    assert!(o.starts_with("işlem geri alınamaz (kalıcı silme çağrısı: "), "{o}");
    assert!(!o.contains("Socialite") && !o.contains("dışına etki"), "{o}");
    let kart = json_oku(&h.join("benlik.json"));
    let op = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "POST" && o["entry"]["path"] == "/trash/clear").unwrap();
    assert_eq!(op["requires_approval"], true);
    assert_eq!(op["x-dna"]["reversibility"]["status"], "irreversible", "{:#}", op["x-dna"]);
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn kalici_silme_gerekcesi_iki_dilde() {
    let h = haritala("d9_guven_cop");
    let tr = arac(&h, "tr", "tani", json!({ "kapi": "POST /trash/clear" }));
    assert!(tr.contains("hayır — kalıcı silme çağrısı: ") && tr.contains("işlem yöntemi ne olursa olsun kayıtları kalıcı siliyor"), "{tr}");
    let en = arac(&h, "en", "tani", json!({ "kapi": "POST /trash/clear" }));
    assert!(en.contains("no — permanent delete call: ") && en.contains("the operation deletes records permanently, whatever its method"), "{en}");
    assert!(!en.contains("kalıcı") && !en.contains("yöntemi ne olursa"), "{en}");
    let _ = std::fs::remove_dir_all(&h);
}

/// Toplu sorgu silmesi kalıcıdır: DELETE dışı yöntemle çağrılan işlem "hayır" ve onaya düşer
fn kalici_toplu(m: &Value, id: &str) {
    let d = dna(m, id);
    assert_eq!(d["geri_alinabilir"], "hayir", "{id}: toplu sorgu silmesi kalıcıdır: {d:#}");
    assert!(d["geri_alma_gerekce"].as_str().unwrap().starts_with("kalıcı silme çağrısı: "), "{id}: {d:#}");
    let k = kapi(m, id);
    let o = k["etki_onayi"].as_str().unwrap_or_else(|| panic!("{id}: onay ister: {k:#}"));
    assert!(o.starts_with("işlem geri alınamaz (kalıcı silme çağrısı: "), "{id}: {o}");
}

#[test]
fn toplu_sorgu_silmesi_kalicidir_php_python_ts() {
    let h = haritala("d9_guven_cop");
    let m = json_oku(&h.join("kapilar.json"));
    // Eloquent `Note::where(...)->delete()`
    kalici_toplu(&m, "POST /notes/purge");
    // Yumuşak silen modelde (SoftDeletes) normal `delete()` kalıcı değildir
    let y = dna(&m, "POST /links/purge");
    assert_ne!(y["geri_alinabilir"], "hayir", "{y:#}");
    assert!(!y["geri_alma_gerekce"].as_str().unwrap_or("").contains("kalıcı silme çağrısı"), "{y:#}");
    let _ = std::fs::remove_dir_all(&h);
    // SQLAlchemy `query(Item).filter(...).delete()` ve `delete(Log).where(...)`
    let h = haritala("d9_guven_cop_py");
    let m = json_oku(&h.join("kapilar.json"));
    kalici_toplu(&m, "POST /items/purge");
    kalici_toplu(&m, "POST /logs/clear");
    let _ = std::fs::remove_dir_all(&h);
    // TypeORM `createQueryBuilder().delete().from(Note)`
    let h = haritala("d9_guven_cop_ts");
    let m = json_oku(&h.join("kapilar.json"));
    kalici_toplu(&m, "POST /api/notes/purge");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn cok_satirli_sorgu_zinciri_tek_ifade_olarak_okunur() {
    // Eloquent: `Note::where(...)` alt satırlarda `->where(...)` ve `->delete()`
    let h = haritala("d9_guven_cop");
    let m = json_oku(&h.join("kapilar.json"));
    kalici_toplu(&m, "POST /notes/clear-done");
    let _ = std::fs::remove_dir_all(&h);
    // SQLAlchemy: parantez içinde bölünmüş `filter(` ve alt satırda `).delete()`
    let h = haritala("d9_guven_cop_py");
    let m = json_oku(&h.join("kapilar.json"));
    kalici_toplu(&m, "POST /items/clear");
    let _ = std::fs::remove_dir_all(&h);
    // TypeORM: `.createQueryBuilder()` zinciri satır satır
    let h = haritala("d9_guven_cop_ts");
    let m = json_oku(&h.join("kapilar.json"));
    kalici_toplu(&m, "POST /api/notes/clear");
    let _ = std::fs::remove_dir_all(&h);
}
