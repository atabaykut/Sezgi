//! D10 kuyruk kurcalama: onay kuyruğu ve plan kayıtları ajanın erişebildiği düz metindir. Onay anında risk, kapı,
//! yöntem ve işlem kayıttan değil haritadan kurulur; kayıttaki `risk` ya da `kapi`/`islem` değiştirilerek gerçek
//! verinin kapıları atlanamaz ve insanın gördüğünden farklı bir yöntem gönderilemez.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Ev klasöründe `<id>.json` adlı onay kaydı
fn onay_kaydi(kok: &Path, id: &str) -> Option<PathBuf> {
    for g in std::fs::read_dir(kok).ok()?.flatten() {
        let y = g.path();
        if y.is_dir() {
            if let Some(b) = onay_kaydi(&y, id) {
                return Some(b);
            }
        } else if y.file_name().is_some_and(|a| a == format!("{id}.json").as_str()) && y.parent().is_some_and(|p| p.ends_with("onay")) {
            return Some(y);
        }
    }
    None
}

fn kurcala(p: &Proje, id: &str, f: impl Fn(&mut Value)) {
    let y = onay_kaydi(&p.ev, id).expect("onay kaydı");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&y).unwrap()).unwrap();
    f(&mut v);
    std::fs::write(&y, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

fn gercek_token(p: &Proje, izinler: &[&str]) -> String {
    let mut a = vec!["token", "olustur", "--ad", "ajan"];
    for i in izinler {
        a.extend(["--izin", i]);
    }
    a.push("--gercek-veri");
    let (ok, c) = p.sahte_terminalde(&a, "evet\n");
    assert!(ok, "{c}");
    c.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string()
}

#[test]
fn gercek_veride_risk_oku_yapilan_kayit_terminalsiz_onaylanamaz() {
    let ev = Ev::yeni("d10-kuyruk-risk");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["anahtar", "olustur"]);
    p.ok(&["init", ".", "--imzala"]);
    p.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    p.insan_capasi();
    p.oturum_rollu("OTURUM-ALICE", &["ADMIN"]);
    p.gercek_veri_ac(&u.adres());
    let t = gercek_token(&p, &["kitap.*:yikici"]);
    let (s, _) = p.mcp(&t).cagir(json!({ "kapi": "DELETE /api/v1/kitaplar/:id", "parametreler": { "id": "7" } }));
    let id = onay_kimligi(&s);
    let ozet = p.onay_ozeti(&id);
    // Ajan kaydı "okuma" diye işaretler ve gerçek veri işaretini siler
    kurcala(&p, &id, |v| {
        v["risk"] = json!("oku");
        v.as_object_mut().unwrap().remove("gercek_veri");
    });
    let (ok, c) = p.cikti(&["onay", "ver", &id, "--ozet", &ozet]);
    assert!(!ok, "{c}");
    assert_eq!(u.sayi(), 0, "kurcalanmış kayıt gerçek veriye gitmemeli: {c}");
}

#[test]
fn kayittaki_kapi_degistirilince_baska_yontem_gonderilmez() {
    let ev = Ev::yeni("d10-kuyruk-kapi");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:yaz"], &["--onay", "hepsi"]);
    let (s, _) = p.mcp(&t).cagir(json!({ "kapi": "PATCH /api/v1/kitaplar/:id", "parametreler": { "id": "5" }, "govde": { "baslik": "Yeni" } }));
    let id = onay_kimligi(&s);
    let ozet = p.onay_ozeti(&id);
    // İnsanın gördüğü PATCH; ajan kaydın kapısını ve işlemini silmeye çevirir
    kurcala(&p, &id, |v| {
        v["kapi"] = json!("DELETE /api/v1/kitaplar/:id");
        v["islem"] = json!("kitap.sil");
    });
    let (ok, c) = p.cikti(&["onay", "ver", &id, "--ozet", &ozet]);
    assert!(!ok && c.contains("uyuşmuyor"), "{c}");
    assert!(u.gelenler.lock().unwrap().iter().all(|g| g.yontem != "DELETE"), "silme gönderilmemeli");
    assert_eq!(u.sayi(), 0);
    // Değiştirilmemiş kayıt hâlâ onaylanabilir
    let (s, _) = p.mcp(&t).cagir(json!({ "kapi": "PATCH /api/v1/kitaplar/:id", "parametreler": { "id": "6" }, "govde": { "baslik": "Yeni" } }));
    let id = onay_kimligi(&s);
    p.ok(&["onay", "ver", &id, "--ozet", &p.onay_ozeti(&id)]);
    assert_eq!(u.sayi(), 1);
    assert_eq!(u.gelenler.lock().unwrap()[0].yontem, "PATCH");
}

#[test]
fn kartin_uygulama_kimligi_degistirilerek_imza_guvencesi_atlanamaz() {
    let ev = Ev::yeni("d10-kart-kimligi");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.ok(&["anahtar", "olustur"]);
    p.ok(&["init", ".", "--imzala"]);
    p.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(yorum_tekrarli("1"));
    assert!(!hata && u.sayi() == 1, "imzalı kartla kesin yazma gider: {s}");
    // Ajan kartı imzasız yeniden üretir ve uygulama kimliğini değiştirir (sabitleme eski kimliğe bağlıydı)
    p.ok(&["init", "."]);
    let yol = p.klasor.join(".sezgi").join("benlik.json");
    let mut kart: Value = serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
    kart["app"]["id"] = json!("baska-uygulama");
    std::fs::write(&yol, serde_json::to_string_pretty(&kart).unwrap()).unwrap();
    let (s, _) = p.mcp(&t).cagir(yorum_tekrarli("2"));
    assert_eq!(u.sayi(), 1, "kimliği değişmiş imzasız kartla yazılmamalı: {s}");
    // İnsan kartı yeniden imzalayıp hedefi yeniden bildirince yazma döner
    p.ok(&["init", ".", "--imzala"]);
    p.ok(&["hedef", &u.adres(), "--test-kopyasi"]);
    let (s, hata) = p.mcp(&t).cagir(yorum_tekrarli("3"));
    assert!(!hata && u.sayi() == 2, "{s}");
}

fn yorum_tekrarli(anahtar: &str) -> Value {
    let mut a = yorum_yaz();
    a["tekrar_anahtari"] = json!(anahtar);
    a
}
