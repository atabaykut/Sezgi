//! P1 H1: uzak hedefte https zorunlu, insan oturumu düz http'yle uzağa gitmez (B7); yol parametresinde nokta
//! segmentleri hedefe gitmez (B6); okuma modunda önizleme gerçek oturum durumunu kullanır (B24).
mod p0_g1_ortak;
use p0_g1_ortak::*;

#[test]
fn uzak_hedefte_https_zorunlu() {
    let ev = Ev::yeni("hedef-https");
    let p = ev.proje("nestjs-temel", "a");
    let (ok, m) = p.cikti(&["hedef", "http://uzak.example:8080", "--test-kopyasi"]);
    assert!(!ok && m.contains("https"), "uzak http hedefi reddedilmeli: {m}");
    assert!(p.ok(&["mod"]).contains("Hedef: yok"));
    for yerel in ["http://127.0.0.1:9", "http://localhost:9"] {
        p.ok(&["hedef", yerel, "--test-kopyasi"]);
    }
    p.ok(&["hedef", "https://uzak.example", "--test-kopyasi"]);
}

#[test]
fn eski_ayardaki_uzak_http_hedefine_oturum_gonderilmez() {
    let ev = Ev::yeni("hedef-eski");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    // Önceki sürümün kabul ettiği uzak http hedefi (ayar dosyasında kalmış)
    let uyg = std::fs::read_dir(ev.ayar().join("sezgi").join("uygulamalar")).unwrap().next().unwrap().unwrap().path();
    let mut ayar: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(uyg.join("ayar.json")).unwrap()).unwrap();
    ayar["hedef"]["url"] = serde_json::json!("http://uzak-hedef.invalid:9");
    std::fs::write(uyg.join("ayar.json"), ayar.to_string()).unwrap();
    let (s, hata) = p.mcp(&t).cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(hata && s.contains("https"), "insan oturumu düz http'yle uzağa gitmemeli: {s}");
}

#[test]
fn nokta_segmenti_yol_parametresi_olamaz() {
    let ev = Ev::yeni("hedef-nokta");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    let mut m = p.mcp(&t);
    for deger in ["..", ".", ""] {
        let (s, hata) = m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar/:id", "parametreler": { "id": deger } }));
        assert!(hata && s.contains("İLETİLMEDİ"), "'{deger}': {s}");
    }
    assert_eq!(u.sayi(), 0, "nokta segmentli yol hedefe gitmemeli");
    let (s, hata) = m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar/:id", "parametreler": { "id": "a.b" } }));
    assert!(!hata && s.contains("OKUNDU"), "{s}");
    assert_eq!(u.gelenler.lock().unwrap()[0].yol, "/api/v1/kitaplar/a.b");
}

#[test]
fn okuma_modunda_oturumsuz_giris_isteyen_okuma_anlamli_mesaj() {
    let ev = Ev::yeni("hedef-oturumsuz");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["hedef", &u.adres(), "--test-kopyasi"]);
    p.ok(&["mod", "okuma"]);
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    let mut m = p.mcp(&t);
    // Giriş isteyen okuma, oturum yokken: uygulamaya gidip 401 almaz, ne yapılacağı söylenir
    let (s, hata) = m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar/:id", "parametreler": { "id": "3" } }));
    assert!(hata && s.contains("insan oturumu yok") && s.contains("sezgi oturum ac"), "{s}");
    assert_eq!(u.sayi(), 0);
    // Herkese açık okuma oturumsuz gider
    let (s, hata) = m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(!hata && s.contains("OKUNDU"), "{s}");
    assert_eq!(u.sayi(), 1);
}
