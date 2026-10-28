//! P0.1 Yerel proje kimliği: canlı mod durumu (mod, hedef, oturum, token, kayıt, hafıza) depodan gelen `app.id`'ye
//! göre değil, haritanın kanonik yoluna bağlı yerel bir kimliğe göre tutulur. Aynı paket adlı iki proje durum
//! paylaşmaz; eski (`uygulamalar/<app.id>`) durum kendiliğinden taşınmaz, yalnızca `sezgi canli tasi` ile.
mod p0_g1_ortak;
use p0_g1_ortak::*;

#[test]
fn ayni_adli_iki_proje_durum_ve_token_paylasmaz() {
    let ev = Ev::yeni("iki-proje");
    let u = Uygulama::yeni();
    let a = ev.proje("nestjs-temel", "a");
    let b = ev.proje("nestjs-temel", "b");
    assert_eq!(a.kart()["app"]["id"], b.kart()["app"]["id"], "örnek: aynı paket adı");
    a.kur(&u, "OTURUM-A");
    let t = a.token("ajan", &["kitap.*:oku"], &[]);
    // B projesinde hiçbir canlı kurulum yok
    let m = b.ok(&["mod"]);
    assert!(m.contains("Mod: gozlem") && m.contains("Hedef: yok"), "B, A'nın modunu ve hedefini görmemeli: {m}");
    let l = b.ok(&["token", "listele"]);
    assert!(!l.contains("ajan"), "B, A'nın token'ını görmemeli: {l}");
    // A'nın token'ı B'nin haritasıyla hiçbir şey yaptıramaz
    let mut mb = b.mcp(&t);
    let (s, hata) = mb.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(hata && s.contains("Token geçersiz"), "{s}");
    assert_eq!(u.sayi(), 0, "B üzerinden A'nın oturumuyla istek gitmemeli");
    // Depodaki benlik.json'un app.id'si değiştirilse de A'nın durumuna erişilemez
    let yol = b.klasor.join(".sezgi").join("benlik.json");
    let mut kart = b.kart();
    kart["app"]["id"] = serde_json::json!("baska-ad");
    std::fs::write(&yol, kart.to_string()).unwrap();
    assert!(b.ok(&["mod"]).contains("Mod: gozlem"));
    // A kendi durumunu korur
    let mut ma = a.mcp(&t);
    let (s, hata) = ma.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(!hata && s.contains("OKUNDU"), "{s}");
    assert_eq!(u.gelenler.lock().unwrap()[0].yetki.as_deref(), Some("Bearer OTURUM-A"));
}

#[test]
fn eski_durum_kendiliginden_tasinmaz_tasi_ile_gelir() {
    let ev = Ev::yeni("eski-durum");
    let u = Uygulama::yeni();
    let a = ev.proje("nestjs-temel", "a");
    let app_id = a.kart()["app"]["id"].as_str().unwrap().to_string();
    // Önceki sürümün yazdığı durum: uygulamalar/<app.id>/ayar.json
    let eski = ev.ayar().join("sezgi").join("uygulamalar").join(&app_id);
    std::fs::create_dir_all(&eski).unwrap();
    std::fs::write(eski.join("ayar.json"), serde_json::json!({ "mod": "okuma", "hedef": { "url": u.adres(), "test_kopyasi": true } }).to_string()).unwrap();
    let (ok, m) = a.cikti(&["mod"]);
    assert!(ok && m.contains("Mod: gozlem"), "eski durum kendiliğinden kullanılmamalı: {m}");
    assert!(m.contains("sezgi canli tasi"), "kullanıcıya eski durum ve taşıma yolu söylenmeli: {m}");
    assert!(eski.join("ayar.json").exists(), "eski klasör yerinde kalır");
    let t = a.ok(&["canli", "tasi"]);
    assert!(t.contains("taşındı"), "{t}");
    let m = a.ok(&["mod"]);
    assert!(m.contains("Mod: okuma") && m.contains(&u.adres()), "{m}");
    assert!(!eski.exists(), "taşınan eski klasör kalmaz");
    // İkinci kez taşınacak bir şey yok
    let (ok, m) = a.cikti(&["canli", "tasi"]);
    assert!(!ok && m.contains("eski durum yok"), "{m}");
}
