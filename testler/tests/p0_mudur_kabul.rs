//! P0 kabul testinde bulunan açıklar (müdür):
//! Y1: ajanın `cagir`'a eklediği doğrulanmamış `roller` beyanı önizlemeyi geçirip insan onayı kapısını aşmamalı.
//! Y2: rol isteyen yazma kartta da onay ister (çalışma anı kuralıyla aynı); kart dış tüketicilere iyimser sözleşme vermez.
mod p0_g1_ortak;
use p0_g1_ortak::*;

fn yorum_onayli(roller: Option<&[&str]>) -> serde_json::Value {
    let mut a = serde_json::json!({ "kapi": "POST /api/v1/yorumlar/onayli", "govde": { "metin": "güzel", "puan": 4, "kitapId": "8a3c1e2b-1111-2222-3333-444455556666" } });
    if let Some(r) = roller {
        a["roller"] = serde_json::json!(r);
    }
    a
}

#[test]
fn rol_beyani_onay_kapisini_atlatmaz() {
    let ev = Ev::yeni("mudur-rol-beyani");
    let u = Uygulama::yeni();
    let p = ev.proje("p0_mudur_rol", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(yorum_onayli(None));
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "rolü bilinmeyen çağıran onaysız yazamaz: {s}");
    let (s, hata) = m.cagir(yorum_onayli(Some(&["EDITOR"])));
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "ajanın rol beyanı doğrulanmış rol sayılmaz: {s}");
    assert_eq!(u.sayi(), 0, "onay kapısı aşılmamalı");
}

#[test]
fn rol_isteyen_yazma_kartta_onay_ister() {
    let ev = Ev::yeni("mudur-rol-kart");
    let u = Uygulama::yeni();
    let p = ev.proje("p0_mudur_rol", "a");
    p.kur(&u, "OTURUM-ALICE");
    let kart = p.kart();
    let bul = |yol: &str| kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "POST" && o["entry"]["path"] == yol).unwrap().clone();
    let onayli = bul("/api/v1/yorumlar/onayli");
    assert_eq!(onayli["requires_approval"], true, "rol isteyen yazma: {onayli}");
    assert_eq!(bul("/api/v1/yorumlar")["requires_approval"], false, "rol istemeyen kesin yazma değişmez");
}
