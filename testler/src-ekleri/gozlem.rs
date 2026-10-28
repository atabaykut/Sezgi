//! W2-gözlem: değer süzgeci ve uç eşleme. Yol parçası ya da anahtar bir değere benziyorsa (sayı, uuid, e-posta,
//! belirteç, uzun ya da rastgele görünen) yazılmaz; alan adları yazılır.
use super::*;
use serde_json::json;

#[test]
fn w2g_yol_sablonu_degerleri_atar() {
    assert_eq!(sablon("/api/v1/items/c7af0c37-4b2f-4890-b78c-9102b1bed85d"), "/api/v1/items/:param");
    assert_eq!(sablon("/api/v1/users/42/posts/7"), "/api/v1/users/:param/posts/:param");
    assert_eq!(sablon("/api/v1/users/me"), "/api/v1/users/me");
    assert_eq!(sablon("/api/v1/users/admin@example.com"), "/api/v1/users/:param");
    assert_eq!(sablon("/reset/eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.abc"), "/reset/:param");
    assert_eq!(sablon("/files/5f2b8c9d0e1a4b3c5d6e7f80"), "/files/:param");
    assert_eq!(sablon("/kullanici/ali%40ornek.com"), "/kullanici/:param");
    assert_eq!(sablon("/api/v1/items/"), "/api/v1/items/");
}

#[test]
fn w2g_alan_adlari_degerler_yok() {
    let (a, dizi, dinamik) = json_alanlari(
        &json!({ "title": "gizli", "owner_id": "c7af0c37-4b2f-4890-b78c-9102b1bed85d", "admin@example.com": true, "c7af0c37-4b2f-4890-b78c-9102b1bed85d": 1, "count": 3 }),
    );
    assert!(!dizi);
    assert_eq!(a.keys().map(String::as_str).collect::<Vec<_>>(), vec!["count", "owner_id", "title"]);
    assert_eq!(a["title"], "string");
    assert_eq!(a["count"], "number");
    assert_eq!(dinamik, 2, "değer görünümlü anahtarlar yalnızca sayılır");
    let (a, dizi, _) = json_alanlari(&json!([{ "id": 1 }, { "name": "x" }]));
    assert!(dizi);
    assert_eq!(a.len(), 2);
    let (adlar, dinamik) = form_adlari("username=admin%40example.com&password=t1-Parola-123&admin%40example.com=1");
    assert_eq!(adlar, vec!["password", "username"]);
    assert_eq!(dinamik, 1);
}

#[test]
fn w2g_adres_parcalama_kimlik_bilgisini_atar() {
    let (ana, yol, sorgu) = adres_parcala("https://kullanici:parola@Ornek.com:8443/api/v1/x?y=1#z");
    assert_eq!((ana.as_str(), yol.as_str(), sorgu.as_str()), ("ornek.com:8443", "/api/v1/x", "y=1"));
}

#[test]
fn w2g_uc_eslemede_sabit_parca_oncelikli() {
    let harita = json!({ "endpointler": [
        { "yontem": "GET", "yol": "/api/v1/users/:user_id", "dosya": "a.py" },
        { "yontem": "GET", "yol": "/api/v1/users/me", "dosya": "a.py" },
    ] });
    let u = uclar(&harita);
    assert_eq!(eslestir(&u, "GET", "/api/v1/users/me").unwrap().api, "api:GET /api/v1/users/me");
    assert_eq!(eslestir(&u, "GET", "/api/v1/users/42").unwrap().api, "api:GET /api/v1/users/:user_id");
    assert!(eslestir(&u, "POST", "/api/v1/users/me").is_none());
}
