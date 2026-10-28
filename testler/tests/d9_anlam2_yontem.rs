//! D9 anlam2: Django işlev görünümlerinde yöntem kısıtları. Yerleşik `@require_POST` / `@require_GET` /
//! `@require_http_methods`, DRF `@api_view([...])`, projedeki yöntem süzen dekoratör (`request.method` karşılaştırıp 405
//! dönen ve yöntem adlarıyla çağrılan) ve gövdedeki `request.method != "POST"` -> 405 koruması kabul edilen yöntemleri
//! belirler; kısıtlı görünüm GET diye listelenmez.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::Value;

fn idler(p: &Proje) -> Vec<String> {
    let h: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    h["endpointler"].as_array().unwrap().iter().map(|k| k["id"].as_str().unwrap().to_string()).collect()
}

#[test]
fn django_yontem_kisitlari() {
    let ev = Ev::yeni("d9-anlam2-yontem");
    let p = ev.proje("d9_anlam2_izleme", "nabiz");
    let v = idler(&p);
    let var = |id: &str| v.iter().any(|x| x == id);
    // Projedeki yöntem süzen dekoratör
    assert!(var("POST /api/v1/checks/:code/pause") && !var("GET /api/v1/checks/:code/pause"), "{v:?}");
    assert!(var("POST /api/v1/checks/:code/resume") && !var("GET /api/v1/checks/:code/resume"), "{v:?}");
    assert!(var("POST /api/v1/checks/:code") && var("DELETE /api/v1/checks/:code") && !var("GET /api/v1/checks/:code"), "{v:?}");
    assert!(var("GET /api/v1/checks/") && !var("POST /api/v1/checks/"), "{v:?}");
    // Gövdedeki koruma: POST dışı 405
    assert!(var("POST /integrations/:code/test/") && !var("GET /integrations/:code/test/"), "{v:?}");
    // Yerleşik dekoratörler ve DRF
    assert!(var("POST /checks/:code/remove/") && !var("GET /checks/:code/remove/"), "{v:?}");
    assert!(var("GET /checks/") && !var("POST /checks/"), "{v:?}");
    assert!(var("GET /integrations/") && var("POST /integrations/"), "{v:?}");
    assert!(var("POST /checks/:code/transfer/") && !var("GET /checks/:code/transfer/"), "{v:?}");
    // Kısıtsız görünüm GET kalır
    assert!(var("GET /ping/:code/"), "{v:?}");
}
