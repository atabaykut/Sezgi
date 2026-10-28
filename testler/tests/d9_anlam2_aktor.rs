//! D9 anlam2: özel kimlik doğrulama dekoratörleri ve bağımlılıkları. İstek başlığından (ya da META, gövde) API anahtarı
//! veya belirteç okuyup 401 / 403 dönen projedeki dekoratör (Django, Flask) ya da bağımlılık (FastAPI) giriş ister;
//! karar tanımdan değil kalıptan çıkar (`inferred`). Bu işlemler "anonim ziyaretçi" değil "API anahtarı / belirteci
//! ile istemci" kullanıcı tipidir; kimsin, rapor ve kart bunu iki dilde gösterir.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};

fn kapilar(p: &Proje) -> Vec<Value> {
    let h: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    h["endpointler"].as_array().unwrap().clone()
}

fn kapi<'a>(ks: &'a [Value], id: &str) -> &'a Value {
    ks.iter().find(|k| k["id"] == id).unwrap_or_else(|| panic!("{id} yok: {:?}", ks.iter().map(|k| k["id"].clone()).collect::<Vec<_>>()))
}

#[test]
fn django_api_anahtari_dekoratoru() {
    let ev = Ev::yeni("d9-anlam2-aktor-dj");
    let p = ev.proje("d9_anlam2_izleme", "nabiz");
    let ks = kapilar(&p);
    for id in ["GET /api/v1/checks/", "POST /api/v1/checks/:code/pause", "DELETE /api/v1/checks/:code"] {
        let k = kapi(&ks, id);
        assert_eq!(k["giris"], "gerekli", "{k}");
        assert_ne!(k["giris_kaniti"], "kesin", "kalıptan çıkan karar kesin değildir: {k}");
        assert!(k["guardlar"].as_array().unwrap().iter().any(|g| g == "api_key_required [api key/token]"), "{k}");
    }
    // META'dan Authorization okuyup 403 dönen dekoratör
    let k = kapi(&ks, "GET /api/v1/badges/");
    assert_eq!(k["giris"], "gerekli", "{k}");
    // Dağıtıcı görünüm: isteği yalnızca anahtar isteyen görünümlere iletir
    for id in ["GET /api/v2/checks/", "POST /api/v2/checks/"] {
        let k = kapi(&ks, id);
        assert_eq!(k["giris"], "gerekli", "{k}");
        assert!(k["guardlar"].as_array().unwrap().iter().any(|g| g.as_str().unwrap().ends_with("-> api_key_required [api key/token]")), "{k}");
    }
    // Kimlik denetlemeyen dekoratör giriş istemez
    assert_eq!(kapi(&ks, "GET /ping/:code/")["giris"], "herkese_acik");
    // Kullanıcı tipleri: API istemcisi ayrı; anonim yalnızca gerçekten açık işlemler
    let mut m = p.mcp("");
    let (t, _) = m.arac("kimsin", json!({}));
    assert!(t.contains("API anahtarı / belirteci ile istemci (8)"), "{t}");
    assert!(t.contains("anonim ziyaretçi (3)"), "{t}");
    let r = p.ok(&["rapor"]);
    assert!(r.contains("API anahtarı / belirteci ile istemci (8)"), "{r}");
    let o = p.komut(&["--lang", "en", "rapor"]).output().unwrap();
    let en = String::from_utf8_lossy(&o.stdout);
    assert!(en.contains("API key / token client (8)"), "{en}");
    let kart = p.kart();
    assert!(kart["app"]["x-meaning"]["actors"].as_array().unwrap().iter().any(|a| a["id"] == "actor:api_client" && a["stamp"] == "inferred"), "{}", kart["app"]["x-meaning"]);
    // tani: guard ve giriş damgası
    let (t, _) = m.arac("tani", json!({ "kapi": "POST /api/v1/checks/:code/pause" }));
    assert!(t.contains("giriş:gerekli") && t.contains("api_key_required [api key/token]"), "{t}");
}

#[test]
fn fastapi_baslik_anahtari_bagimliligi() {
    let ev = Ev::yeni("d9-anlam2-aktor-fa");
    let p = ev.proje("d9_anlam2_kumbara", "kumbara");
    let ks = kapilar(&p);
    let k = kapi(&ks, "POST /api/v1/imports/");
    assert_eq!(k["giris"], "gerekli", "{k}");
    assert_ne!(k["giris_kaniti"], "kesin", "{k}");
    assert!(k["guardlar"].as_array().unwrap().iter().any(|g| g == "require_api_key [api key/token]"), "{k}");
    // OAuth2 şemasıyla korunan işlemler kesin kalır
    assert_eq!(kapi(&ks, "GET /api/v1/chats/")["giris_kaniti"], "kesin");
}

#[test]
fn flask_baslik_anahtari_dekoratoru() {
    let ev = Ev::yeni("d9-anlam2-aktor-fl");
    let p = ev.proje("d9_anlam2_flask", "sayac");
    let ks = kapilar(&p);
    let k = kapi(&ks, "POST /api/readings");
    assert_eq!(k["giris"], "gerekli", "{k}");
    assert_ne!(k["giris_kaniti"], "kesin");
    assert_eq!(kapi(&ks, "GET /api/devices")["giris"], "herkese_acik");
}
