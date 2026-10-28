//! P0.2 Bağlam anahtarlı hafıza: hatırlanan cevap yalnızca aynı proje, token, insan oturumu, hedef ve harita içeriği
//! için kullanılır. Oturum ya da hedef değişince ve token iptal edilince hafıza temizlenir. Oturumla (Authorization)
//! yapılan okumalar varsayılan olarak hatırlanmaz; `sezgi hafiza ac --oturumlu` ile açılır.
mod p0_g1_ortak;
use p0_g1_ortak::*;

fn listele() -> serde_json::Value {
    serde_json::json!({ "kapi": "GET /api/v1/kitaplar/:id", "parametreler": { "id": "3" } })
}

#[test]
fn oturumlu_okuma_varsayilan_hatirlanmaz() {
    let ev = Ev::yeni("hafiza-varsayilan");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    p.ok(&["hafiza", "ac"]);
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    let mut m = p.mcp(&t);
    m.cagir(listele());
    let (s, _) = m.cagir(listele());
    assert!(!s.contains("HAFIZADAN"), "{s}");
    assert_eq!(u.sayi(), 2, "oturumla alınan cevap varsayılan olarak hatırlanmaz");
}

#[test]
fn oturum_degisince_baskasinin_cevabi_donmez() {
    let ev = Ev::yeni("hafiza-oturum");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    p.ok(&["hafiza", "ac", "--oturumlu"]);
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    {
        let mut m = p.mcp(&t);
        m.cagir(listele());
        let (s, _) = m.cagir(listele());
        assert!(s.contains("HAFIZADAN"), "aynı insan, aynı istek: hafızadan ({s})");
    }
    assert_eq!(u.sayi(), 1);
    p.oturum("OTURUM-BOB");
    let mut m = p.mcp(&t);
    let (s, _) = m.cagir(listele());
    assert!(!s.contains("HAFIZADAN"), "Bob, Alice'in cevabını almamalı: {s}");
    assert_eq!(u.sayi(), 2);
    assert_eq!(u.gelenler.lock().unwrap()[1].yetki.as_deref(), Some("Bearer OTURUM-BOB"));
}

#[test]
fn hedef_degisince_eski_hedefin_cevabi_donmez() {
    let ev = Ev::yeni("hafiza-hedef");
    let u1 = Uygulama::yeni();
    let u2 = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u1, "OTURUM-ALICE");
    p.ok(&["hafiza", "ac", "--oturumlu"]);
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    p.mcp(&t).cagir(listele());
    assert_eq!(u1.sayi(), 1);
    p.ok(&["hedef", &u2.adres(), "--test-kopyasi"]);
    let (s, _) = p.mcp(&t).cagir(listele());
    assert!(!s.contains("HAFIZADAN"), "{s}");
    assert_eq!(u2.sayi(), 1, "yeni hedefe gerçekten gidilmeli");
}

#[test]
fn token_iptalinde_hafiza_temizlenir() {
    let ev = Ev::yeni("hafiza-iptal");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    p.ok(&["hafiza", "ac", "--oturumlu"]);
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    p.mcp(&t).cagir(listele());
    assert!(p.ok(&["hafiza"]).contains("1 sonuç"));
    p.ok(&["token", "iptal", "ajan"]);
    assert!(p.ok(&["hafiza"]).contains("0 sonuç"), "iptal edilen token'ın hatırlanan sonuçları silinir");
    // Aynı adla yeniden üretilen token eski sonucu miras almaz
    let t2 = p.token("ajan", &["kitap.*:oku"], &[]);
    let (s, _) = p.mcp(&t2).cagir(listele());
    assert!(!s.contains("HAFIZADAN"), "{s}");
    assert_eq!(u.sayi(), 2);
}
