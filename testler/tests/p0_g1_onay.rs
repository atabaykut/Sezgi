//! P0.6 Onay: tek seferlik kilit (eşzamanlı iki onay tek istek gönderir), onay anında token, izin, tekrar ve önizleme
//! yeniden denetlenir (iptal edilmiş token'ın isteği yürütülmez), insan gövdeyi görür (`onay goster`) ve terminal
//! yoksa onay gördüğü içeriğin özetine bağlanır (`onay ver <id> --ozet <ilk8>`).
mod p0_g1_ortak;
use p0_g1_ortak::*;

#[test]
fn iptal_edilmis_tokenin_istegi_onaylanamaz() {
    let ev = Ev::yeni("onay-iptal");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:yikici"], &[]);
    let (s, _) = p.mcp(&t).cagir(serde_json::json!({ "kapi": "DELETE /api/v1/kitaplar/7" }));
    let id = onay_kimligi(&s);
    let oz = p.onay_ozeti(&id);
    p.ok(&["token", "iptal", "ajan"]);
    let (ok, m) = p.cikti(&["onay", "ver", &id, "--ozet", &oz]);
    assert!(!ok && m.contains("iptal"), "iptal edilmiş token'ın isteği onaylanamaz: {m}");
    assert_eq!(u.sayi(), 0);
    assert!(!p.ok(&["onay", "listele"]).contains(&id), "iptal edilen token'ın bekleyen isteği kuyruktan düşer");
}

#[test]
fn eszamanli_iki_onay_tek_istek_gonderir() {
    let ev = Ev::yeni("onay-yaris");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:yikici"], &[]);
    let mut m = p.mcp(&t);
    for n in 0..4 {
        let (s, _) = m.cagir(serde_json::json!({ "kapi": format!("DELETE /api/v1/kitaplar/{n}") }));
        let id = onay_kimligi(&s);
        let once = u.sayi();
        let oz = p.onay_ozeti(&id);
        let cocuklar: Vec<_> =
            (0..3).map(|_| p.komut(&["onay", "ver", &id, "--ozet", &oz]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap()).collect();
        let basarili = cocuklar.into_iter().map(|mut c| c.wait().unwrap().success()).filter(|b| *b).count();
        assert_eq!(u.sayi() - once, 1, "deneme {n}: aynı onay bir kez yürütülür");
        assert_eq!(basarili, 1, "deneme {n}: yalnızca bir onay başarılı olur");
    }
    assert!(p.ok(&["kayit", "--dogrula"]).contains("sağlam"));
}

#[test]
fn govdeli_onay_govdeyi_gosterir_ve_ozete_baglanir() {
    let ev = Ev::yeni("onay-govde");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    // Her yazma onay ister: gövdeli bir istek kuyruğa düşer
    let t = p.token("ajan", &["kitap.*:yaz"], &["--onay", "hepsi"]);
    let (s, hata) = p.mcp(&t).cagir(kitap_olustur());
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "{s}");
    let id = onay_kimligi(&s);
    let g = p.ok(&["onay", "goster", &id]);
    assert!(g.contains("POST /api/v1/kitaplar") && g.contains("Kuyucaklı Yusuf"), "insan gövdeyi görür: {g}");
    let ozet = g.split("Özet: ").nth(1).unwrap_or_else(|| panic!("{g}")).split_whitespace().next().unwrap().to_string();
    assert!(ozet.len() >= 8, "{ozet}");
    // Terminal yok ve özet verilmedi: gönderilmez
    let (ok, m) = p.cikti(&["onay", "ver", &id]);
    assert!(!ok && m.contains("--ozet"), "{m}");
    // Yanlış özet: gönderilmez
    let (ok, m) = p.cikti(&["onay", "ver", &id, "--ozet", "00000000"]);
    assert!(!ok && m.contains("özet"), "{m}");
    assert_eq!(u.sayi(), 0);
    let m = p.ok(&["onay", "ver", &id, "--ozet", &ozet[..8]]);
    assert!(m.contains("HTTP 201"), "{m}");
    assert_eq!(u.sayi(), 1);
    assert!(u.gelenler.lock().unwrap()[0].govde.contains("Kuyucaklı Yusuf"));
}

#[test]
fn onaydan_once_tekrar_korumasi_denetlenir() {
    let ev = Ev::yeni("onay-tekrar");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:yikici"], &[]);
    let mut m = p.mcp(&t);
    // Aynı istek iki kez kuyruğa düşer (henüz hiçbiri yapılmadı)
    let (s1, _) = m.cagir(serde_json::json!({ "kapi": "DELETE /api/v1/kitaplar/9", "tekrar_anahtari": "x" }));
    let (s2, _) = m.cagir(serde_json::json!({ "kapi": "DELETE /api/v1/kitaplar/9", "tekrar_anahtari": "x" }));
    let (id1, id2) = (onay_kimligi(&s1), onay_kimligi(&s2));
    p.ok(&["onay", "ver", &id1, "--ozet", &p.onay_ozeti(&id1)]);
    assert_eq!(u.sayi(), 1);
    let (ok, c) = p.cikti(&["onay", "ver", &id2, "--ozet", &p.onay_ozeti(&id2)]);
    assert!(!ok && c.contains("son 10 dakikada"), "aynı yazma ikinci onayla tekrar yapılmaz: {c}");
    assert_eq!(u.sayi(), 1);
}

#[test]
fn govdesiz_silme_de_ozete_baglanir() {
    let ev = Ev::yeni("onay-govdesiz");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:yikici"], &[]);
    let (s, _) = p.mcp(&t).cagir(serde_json::json!({ "kapi": "DELETE /api/v1/kitaplar/5" }));
    let id = onay_kimligi(&s);
    // Terminal yok ve özet verilmedi: kabuğu olan bir ajan kendi silme isteğini tek komutla onaylayamaz
    let (ok, m) = p.cikti(&["onay", "ver", &id]);
    assert!(!ok && m.contains("--ozet"), "{m}");
    let (ok, _) = p.cikti(&["onay", "ver", &id, "--ozet", "00000000"]);
    assert!(!ok);
    assert_eq!(u.sayi(), 0);
    p.ok(&["onay", "ver", &id, "--ozet", &p.onay_ozeti(&id)]);
    assert_eq!(u.sayi(), 1);
}
