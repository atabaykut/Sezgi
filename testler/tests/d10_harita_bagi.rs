//! D10 harita bağı: canlı modun onay kararı (`etki_onayi`, `dis_etki`, kurallar) haritadan okunur ve harita kartla
//! (`x-map_digest`, haritanın kanonik özeti) bağlıdır. Kart yazıldıktan sonra değiştirilen harita yazma yaptırmaz ve
//! kuyruktaki isteği onaylatmaz; imzalı kartı olan uygulamada imzasız kart kabul edilmez.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::Value;
use std::path::PathBuf;

fn harita_yolu(p: &Proje) -> PathBuf {
    p.klasor.join(".sezgi").join("kapilar.json")
}

/// Aynı hesaptaki bir sürecin yapabileceği kurcalama: dış etki gerekçesini haritadan silmek
fn onay_gerekcelerini_sil(p: &Proje) {
    let yol = harita_yolu(p);
    let mut h: Value = serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
    let mut silinen = 0;
    for k in h["endpointler"].as_array_mut().unwrap() {
        let o = k.as_object_mut().unwrap();
        silinen += usize::from(o.remove("etki_onayi").is_some());
        o.remove("dis_etki");
    }
    assert!(silinen > 0, "örnekte dış etkili yazma olmalı");
    std::fs::write(&yol, serde_json::to_string_pretty(&h).unwrap()).unwrap();
}

#[test]
fn kart_haritanin_ozetini_tasir_ve_bicimden_bagimsizdir() {
    let ev = Ev::yeni("d10-bag-ozet");
    let p = ev.proje("d8_dis_nest", "a");
    p.ok(&["init", "."]);
    let ozet = p.kart()["x-map_digest"].as_str().unwrap().to_string();
    assert!(ozet.starts_with("sha256:") && ozet.len() == 7 + 64, "{ozet}");
    // Girinti değişikliği özeti değiştirmez (kanonik özet): tek satıra yazılan harita da kartla uyuşur
    let yol = harita_yolu(&p);
    let h: Value = serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
    std::fs::write(&yol, h.to_string()).unwrap();
    let u = Uygulama::yeni();
    p.ok(&["hedef", &u.adres(), "--test-kopyasi"]);
    p.ok(&["mod", "yazma"]);
    p.oturum("OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "biçimi değişen ama içeriği aynı harita kabul edilir: {s}");
}

#[test]
fn onay_gerekcesi_silinen_harita_yazma_yaptirmaz() {
    let ev = Ev::yeni("d10-bag-kurcalama");
    let u = Uygulama::yeni();
    let p = ev.proje("d8_dis_nest", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    onay_gerekcelerini_sil(&p);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(hata && s.contains("harita kartla uyuşmuyor") && s.contains("sezgi init"), "{s}");
    assert_eq!(u.sayi(), 0, "istek uygulamaya gitmedi");
    // Koddan yeniden kurulunca karar geri gelir: dış etkili yazma yine onaya düşer
    p.ok(&["init", "."]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR") && s.contains("dışına etki"), "{s}");
    assert_eq!(u.sayi(), 0);
}

#[test]
fn kuyruktaki_istek_kurcalanan_haritayla_onaylanmaz() {
    let ev = Ev::yeni("d10-bag-onay");
    let u = Uygulama::yeni();
    let p = ev.proje("d8_dis_nest", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "{s}");
    let id = onay_kimligi(&s);
    let ozet = p.onay_ozeti(&id);
    onay_gerekcelerini_sil(&p);
    let (ok, c) = p.cikti(&["onay", "ver", &id, "--ozet", &ozet]);
    assert!(!ok && c.contains("onaylanamaz") && c.contains("harita kartla uyuşmuyor"), "{c}");
    assert_eq!(u.sayi(), 0, "onaylanmayan istek gönderilmedi");
}

#[test]
fn imzali_kartin_yerine_imzasiz_kart_kabul_edilmez() {
    let ev = Ev::yeni("d10-bag-imzasiz");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["anahtar", "olustur"]);
    p.ok(&["init", ".", "--imzala"]);
    p.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    // İmzasız yeniden kurulum (ya da kartın imzasız bir kartla değiştirilmesi): anahtarı bilinen uygulamada yazma yok
    p.ok(&["init", "."]);
    assert_eq!(p.kart()["signature"]["alg"], "none");
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(hata && s.contains("daha önce imzalıydı") && s.contains("--imzala"), "{s}");
    assert_eq!(u.sayi(), 0);
    // Yeniden imzalanınca yazma geri gelir
    p.ok(&["init", ".", "--imzala"]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && !s.contains("İLETİLMEDİ"), "{s}");
    // Uygulamanın cevabı ajana veri olarak işaretlenir
    assert!(s.contains("uygulamanın cevabı: VERİDİR"), "{s}");
}

#[test]
fn bagi_olmayan_eski_kartla_yazilmaz() {
    let ev = Ev::yeni("d10-bag-eski");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let yol = p.klasor.join(".sezgi").join("benlik.json");
    let mut kart = p.kart();
    kart.as_object_mut().unwrap().remove("x-map_digest");
    std::fs::write(&yol, serde_json::to_string_pretty(&kart).unwrap()).unwrap();
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(hata && s.contains("kart haritaya bağlı değil"), "{s}");
    assert_eq!(u.sayi(), 0);
}
