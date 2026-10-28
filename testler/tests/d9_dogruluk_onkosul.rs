//! D9 doğruluk: ön koşul ve sahiplik atfı.
//! - Ön koşul (servis içi ret) yalnızca işlemin gerçekten ulaştığı zincirdedir: `self.save()` nesnenin kendi
//!   sınıfındaki (ya da çerçevenin taban sınıfındaki) yöntemdir; projede aynı adı taşıyan başka bir dosyadaki
//!   fonksiyonun reddi işleme atfedilmez. Django'da hata yanıtı döndürmek (`return HttpResponseForbidden()`,
//!   `JsonResponse(..., status=400)`) ön koşuldur.
//! - Sahiplik yalnızca kaydın sahip alanı ile istekteki kimliğin karşılaştırılmasıdır: yol parametresiyle süzen toplu
//!   silme (`delete(Item).where(Item.owner_id == user_id)`) sahiplik denetimi değildir.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d9dogok-{ad}-{}-{n}", std::process::id()));
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
    h["ozet"]["dna"].get(&yer).cloned().unwrap_or_default()
}

fn retler(d: &Value) -> Vec<(Option<u64>, String)> {
    d["hatalar"].as_array().into_iter().flatten().map(|h| (h["durum"].as_u64(), h["kanit"].as_str().unwrap_or("").to_string())).collect()
}

#[test]
fn baska_dosyadaki_ayni_adli_fonksiyonun_reddi_atfedilmez() {
    let h = haritala("d9_dogruluk_fastapi");
    let m = json_oku(&h.join("kapilar.json"));
    // `profil.baglanti_gonder()` -> `self.save()`: entegrasyon.py'deki `save` bu zincirde değil
    let g = dna(&m, "POST /giris");
    assert!(!retler(&g).iter().any(|(_, k)| k.starts_with("app/entegrasyon.py")), "yanlış atıf: {g:#}");
    // Gerçekten çağrılan `save(...)` (içe aktarılmış): reddi ön koşuldur
    let e = dna(&m, "POST /entegrasyon");
    assert!(retler(&e).contains(&(Some(403), "app/entegrasyon.py:6".into())), "{e:#}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn django_ret_yaniti_ve_dogru_zincir() {
    let h = haritala("d9_dogruluk_django");
    let m = json_oku(&h.join("kapilar.json"));
    // Giriş, modeldeki `self.save()` üzerinden başka bir dosyanın `save` görünümüne ve onun 403'üne bağlanmaz
    let g = dna(&m, "POST /hesap/giris/");
    assert!(retler(&g).is_empty(), "girişte ret yok: {g:#}");
    // Duraklatmanın kendi 403'ü (`return HttpResponseForbidden()`) var; başka fonksiyonun `not rw` reddi yok
    let d = dna(&m, "POST /api/kontroller/:code/duraklat");
    let r = retler(&d);
    assert!(r.contains(&(Some(403), "izleme/views.py:22".into())), "{d:#}");
    assert!(!r.iter().any(|(_, k)| k == "izleme/views.py:13"), "{d:#}");
    // Entegrasyon: yardımcının reddi (zincirde) ve `JsonResponse(..., status=400)`
    let e = dna(&m, "POST /api/projeler/:code/entegrasyon");
    let r = retler(&e);
    assert!(r.contains(&(Some(403), "izleme/views.py:13".into())) && r.contains(&(Some(400), "izleme/views.py:32".into())), "{e:#}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn yol_parametresiyle_suzme_sahiplik_degildir() {
    let h = haritala("d9_dogruluk_fastapi");
    let m = json_oku(&h.join("kapilar.json"));
    let u = dna(&m, "DELETE /users/:user_id");
    assert!(u["sahiplik_denetimi"].as_array().is_none_or(|a| a.is_empty()), "toplu silmenin koşulu sahiplik değil: {u:#}");
    // İstekteki kimlikle karşılaştırma sahipliktir (koşul ve sorgu süzgeci)
    let s = dna(&m, "DELETE /items/:id");
    assert!(s["sahiplik_denetimi"].as_array().unwrap().iter().any(|x| x["kosul"].as_str().unwrap().contains("current_user.id")), "{s:#}");
    let l = dna(&m, "GET /items/");
    assert!(l["sahiplik_denetimi"].as_array().unwrap().iter().any(|x| x["kosul"].as_str().unwrap().contains("current_user.id")), "{l:#}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn ktor_exposed_birlestirilen_tablo_okunur() {
    let h = haritala("d9_dogruluk_ktor");
    let m = json_oku(&h.join("kapilar.json"));
    let k = m["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == "GET /profiles/:username").cloned().unwrap();
    let okur: Vec<&str> = k["veri"]["okur"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(okur.contains(&"Follows") && okur.contains(&"Users"), "`Users.join(Follows, ...)`: {k:#}");
    let t = m["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == "POST /profiles/:username/follow").cloned().unwrap();
    assert_eq!(t["veri"]["yazar"], serde_json::json!(["Follows"]), "{t:#}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn yontem_dallari_ayri_islem() {
    // `if request.method == "POST": return _guncelle(...)` / `if request.method == "DELETE": return _sil(...)` /
    // `return _getir(...)`: her yöntemin ön koşulu yalnızca kendi dalından
    let h = haritala("d9_dogruluk_django");
    let m = json_oku(&h.join("kapilar.json"));
    let sil = retler(&dna(&m, "DELETE /api/kontroller/:code"));
    let guncelle = retler(&dna(&m, "POST /api/kontroller/:code"));
    let getir = retler(&dna(&m, "GET /api/kontroller/:code"));
    assert!(sil.contains(&(Some(403), "izleme/views.py:53".into())) && !sil.iter().any(|(_, k)| k == "izleme/views.py:44"), "{sil:?}");
    assert!(guncelle.contains(&(Some(400), "izleme/views.py:44".into())) && !guncelle.iter().any(|(_, k)| k == "izleme/views.py:53"), "{guncelle:?}");
    assert!(!getir.iter().any(|(_, k)| k == "izleme/views.py:44" || k == "izleme/views.py:53"), "{getir:?}");
    let _ = std::fs::remove_dir_all(&h);
}
