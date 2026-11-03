//! D10 risk: işlemin riski yalnızca HTTP yönteminden çıkmaz. GET ile kayıt yazan işlem okuma sayılmaz (okuma izniyle
//! ve okuma modunda gitmez). Tek bir silme görünümünün (Django `DeleteView`) POST'u aynı görünümün DELETE'iyle "geri
//! alınabilir" sayılmaz; adı silme söyleyen ve geri alınabilirliği kanıtlanmamış yazma onaysız gitmez.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};

#[test]
fn get_ile_yazan_islem_okuma_izniyle_gitmez() {
    let ev = Ev::yeni("d10-risk-get");
    let u = Uygulama::yeni();
    let p = ev.proje("d10_get_yazan", "a");
    p.ok(&["hedef", &u.adres(), "--test-kopyasi"]);
    p.ok(&["mod", "okuma"]);
    p.oturum("OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(json!({ "kapi": "GET /api/v1/kitaplar/:id/kopyala", "parametreler": { "id": "3" } }));
    assert!(hata, "GET ile yazan işlem okuma izniyle gitmemeli: {s}");
    assert_eq!(u.sayi(), 0);
    // Gerçek okuma gider
    let (s, hata) = m.cagir(json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(!hata && u.sayi() == 1, "{s}");
}

#[test]
fn silme_gorunumunun_postu_kendi_deleteiyle_geri_alinabilir_sayilmaz() {
    let ev = Ev::yeni("d10-risk-silme");
    let p = ev.proje("d10_django_silme", "a");
    let kart = p.kart();
    let islemler: Vec<&Value> =
        kart["operations"].as_array().unwrap().iter().filter(|o| o["entry"]["path"].as_str().is_some_and(|y| y.ends_with("/delete"))).collect();
    assert!(!islemler.is_empty(), "silme görünümü haritada olmalı");
    let dna = |o: &Value| o.get("x-dna").or_else(|| o.get("dna")).cloned().unwrap_or(Value::Null);
    for o in &islemler {
        let d = dna(o);
        assert_ne!(d["reversibility"]["status"], "reversible", "{}: {}", o["id"], d["reversibility"]);
    }
    let post = islemler.iter().find(|o| o["entry"]["method"] == "POST").expect("POST");
    assert_eq!(dna(post)["requires_approval"], true, "{}", dna(post));
}

#[test]
fn ham_sqlde_pasiflestirme_kalici_silme_sayilmaz() {
    let ev = Ev::yeni("d10-risk-yumusak");
    let p = ev.proje("d10_express_yumusak", "a");
    let kart = p.kart();
    let geri = |yol: &str| -> Value {
        let o = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "DELETE" && o["entry"]["path"] == yol).unwrap_or_else(|| panic!("{yol}"));
        o.get("x-dna").or_else(|| o.get("dna")).unwrap()["reversibility"].clone()
    };
    // `UPDATE yazarlar SET is_active = 0, deleted_at = NOW()`: kayıt silinmez, işaretlenir
    let y = geri("/api/kitaplar/:id/yazar");
    assert_ne!(y["status"], "irreversible", "{y}");
    assert!(y["reason"].as_str().unwrap_or("").contains("yumuşak"), "{y}");
    // `DELETE FROM kitaplar`: kalıcı kalır
    assert_eq!(geri("/api/kitaplar/:id")["status"], "irreversible");
}

#[test]
fn setmetadata_ile_izin_yazan_dekorator_rol_sayilir() {
    let ev = Ev::yeni("d10-risk-izin");
    let p = ev.proje("d10_nest_izin", "a");
    let h: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let kapi = |y: &str, yol: &str| h["endpointler"].as_array().unwrap().iter().find(|k| k["yontem"] == y && k["yol"] == yol).cloned().unwrap();
    // @HasPermission('kitapGuncelle') → SetMetadata(HAS_PERMISSION_KEY, permission)
    assert_eq!(kapi("PATCH", "/api/v1/kitaplar/:id")["roller"], json!(["kitapGuncelle"]));
    // Öteki işlemler değişmez
    assert_eq!(kapi("DELETE", "/api/v1/kitaplar/:id")["roller"], json!(["ADMIN"]));
    assert_eq!(kapi("POST", "/api/v1/kitaplar")["roller"], json!([]));
}

#[test]
fn tam_yigin_uygulamada_dis_etki_taramasi_koku_atlamaz() {
    let ev = Ev::yeni("d10-risk-next");
    let p = ev.proje("d10_next_dis", "a");
    let h: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let k = h["endpointler"].as_array().unwrap().iter().find(|k| k["yontem"] == "POST" && k["yol"] == "/api/odeme").cloned().unwrap();
    // Ön yüz kökün kendisi (app/page.tsx): ödeme SDK'sı yine görülür ve yazma onaya düşer
    assert!(k["etki_onayi"].as_str().is_some_and(|g| g.contains("stripe")), "{k}");
}

#[test]
fn tipi_bilinmeyen_alicida_gonderme_cagrisi_dis_etkiyi_bilinmez_yapar() {
    let ev = Ev::yeni("d10-risk-gonder");
    let p = ev.proje("d10_django_silme", "a");
    let h: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let k = |yol: &str| h["endpointler"].as_array().unwrap().iter().find(|k| k["yontem"] == "POST" && k["yol"] == yol).cloned().unwrap();
    // `adres.send_confirmation(request)`: alıcının tipi bilinmiyor, projede karşılığı yok; "bütün çağrılar sınıflandırıldı" denmez
    let e = k("/api/eposta/dogrula");
    assert_eq!(e["dis_etki"], "bilinmiyor", "{e}");
    // Yan etki söylemeyen çağrılar bilinmezlik üretmez
    assert_ne!(k("/api/kontroller/:code/duraklat")["dis_etki"], "bilinmiyor");
}
