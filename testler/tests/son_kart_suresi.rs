//! Son kabul Y2: kartın süre alanlarını silmek ya da kartı kaldırmak süre kapısını aşmaz. Sezgi'nin ürettiği kart
//! her zaman `x-expires_at` taşır; alan ya da kart yoksa yazma yapılmaz (yeniden üretin), okuma uyarıyla geçer.
mod p0_g1_ortak;
use p0_g1_ortak::*;

fn yaz_istegi(metin: &str) -> serde_json::Value {
    serde_json::json!({ "kapi": "POST /api/v1/yorumlar", "govde": { "metin": metin, "puan": 5, "kitapId": "8a3c1e2b-1111-2222-3333-444455556666" } })
}

#[test]
fn sure_alani_silinmis_kart_yazdirmaz() {
    let ev = Ev::yeni("son-sure-alani");
    let p = ev.proje("nestjs-temel", "uyg");
    let u = Uygulama::yeni();
    p.kur(&u, "oturum-degeri");
    let t = p.token("ajan", &["yorum.*:yaz", "kitap.*:oku"], &[]);
    let yol = p.klasor.join(".sezgi").join("benlik.json");
    let mut k = p.kart();
    assert!(k.get("x-expires_at").is_some(), "Sezgi kartı süre alanı taşır");
    k.as_object_mut().unwrap().remove("x-expires_at");
    k.as_object_mut().unwrap().remove("x-issued_at");
    std::fs::write(&yol, serde_json::to_string_pretty(&k).unwrap()).unwrap();
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(yaz_istegi("birinci"));
    assert!(hata && s.contains("kartın süresi doldu") && s.contains("sezgi init"), "{s}");
    assert_eq!(u.sayi(), 0, "süre alanı olmayan kartla yazma uygulamaya gitmemeli");
    let (s, hata) = m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(!hata && s.contains("OKUNDU") && s.contains("kartın süresi doldu"), "okuma uyarıyla geçer: {s}");
}

#[test]
fn kart_yoksa_yazilmaz() {
    let ev = Ev::yeni("son-kart-yok");
    let p = ev.proje("nestjs-temel", "uyg");
    let u = Uygulama::yeni();
    p.kur(&u, "oturum-degeri");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    std::fs::remove_file(p.klasor.join(".sezgi").join("benlik.json")).unwrap();
    let (s, hata) = p.mcp(&t).cagir(yaz_istegi("kartsiz"));
    assert!(hata && !s.starts_with("YAZILDI"), "{s}");
    assert_eq!(u.sayi(), 0, "kart yokken yazma uygulamaya gitmemeli");
}
