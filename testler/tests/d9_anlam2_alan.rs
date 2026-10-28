//! D9 anlam2: iş alanı sınıflandırmasının sağlamlığı. Uygulamanın kendi beyanı (README başlığı / ilk paragrafı, paket
//! açıklaması) tablo ve yol adlarından ağır basar; yalnızca kod adlarından gelen zayıf ya da yarışan kanıtla karar
//! verilmez (alan `unknown`, adaylar notta). "Problem" yalnızca alan biliniyorsa ya da belgede anlatım alıntısı varsa
//! yazılır; güvenlik, kurulum ve teknik paragraflar alıntı olmaz.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};

fn kanit(p: &Proje) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kanit.json")).unwrap()).unwrap()
}

fn onermeler<'a>(k: &'a Value, varlik: &str, yuklem: &str) -> Vec<&'a Value> {
    k["onermeler"].as_array().unwrap().iter().filter(|o| o["varlik"] == varlik && o["yuklem"] == yuklem).collect()
}

fn uygulama_onermeleri<'a>(k: &'a Value, yuklem: &str) -> Vec<&'a Value> {
    k["onermeler"].as_array().unwrap().iter().filter(|o| o["varlik"].as_str().unwrap().starts_with("app:") && o["yuklem"] == yuklem).collect()
}

#[test]
fn kisisel_finans_mesajlasma_sanilmaz() {
    let ev = Ev::yeni("d9-anlam2-finans");
    let p = ev.proje("d9_anlam2_kumbara", "kumbara");
    let k = kanit(&p);
    let alan = onermeler(&k, "app:kumbara", "ALAN");
    assert_eq!(alan.len(), 1, "{alan:?}");
    assert_eq!(alan[0]["deger"]["id"], "personal-finance", "{}", alan[0]);
    assert_eq!(alan[0]["hesap"]["guven"], "inferred");
    // Problem: alanın tanımı ve README'deki anlatım; güvenlik paragrafı (CSRF) alıntı olmaz
    let problem = onermeler(&k, "app:kumbara", "PROBLEM");
    assert!(problem.iter().any(|o| o["deger"]["dayanak"] == "alan" && o["deger"]["alan"] == "personal-finance"), "{problem:?}");
    for o in &problem {
        let a = o["deger"]["alinti"].as_str().unwrap_or("");
        assert!(!a.contains("CSRF") && !a.contains("chat assistant"), "{o}");
    }
    let r = p.ok(&["rapor"]);
    assert!(r.contains("ANLAM        [yorum] alan: kişisel finans ve bütçe"), "{r}");
    assert!(!r.contains("mesajlaşma ·") && !r.contains("Kişiler ve gruplar arasında mesajlaşmak"), "{r}");
    let mut m = p.mcp("");
    let (t, _) = m.arac("kimsin", json!({}));
    assert!(t.contains("Ne: kişisel finans ve bütçe [Yorum]") && t.contains("README README.md:3 'finance'"), "{t}");
    let mut c = p.komut(&["--lang", "en", "rapor"]);
    let o = c.output().unwrap();
    let en = String::from_utf8_lossy(&o.stdout);
    assert!(en.contains("[inferred] domain: personal finance and budgeting"), "{en}");
}

#[test]
fn izleme_servisi_taninir() {
    let ev = Ev::yeni("d9-anlam2-izleme");
    let p = ev.proje("d9_anlam2_izleme", "nabiz");
    let k = kanit(&p);
    let alan = uygulama_onermeleri(&k, "ALAN");
    assert_eq!(alan[0]["deger"]["id"], "monitoring", "{}", alan[0]);
    let r = p.ok(&["rapor"]);
    assert!(r.contains("alan: izleme ve uyarı (uptime / cron)"), "{r}");
    // Anlamsız yetenek adı (kesik çoğul) üretilmez: /oups -> "oups"
    assert!(!r.contains("oup (") && !r.contains(" oup,"), "{r}");
}

#[test]
fn yalnizca_kod_adlarindan_yarisan_kanitla_bilinmiyor() {
    let ev = Ev::yeni("d9-anlam2-zayif");
    let p = ev.proje("d9_anlam2_kumbara", "kumbara");
    // Beyan yok: alan yalnızca tablo ve yol adlarından (sohbet + finans tabloları yarışır) sınıflandırılmaz
    std::fs::remove_file(p.klasor.join("README.md")).unwrap();
    p.ok(&["init", "."]);
    let k = kanit(&p);
    let alan = uygulama_onermeleri(&k, "ALAN");
    assert_eq!(alan.len(), 1);
    assert_eq!(alan[0]["hesap"]["guven"], "unknown", "{}", alan[0]);
    let not = alan[0]["notlar"][0].as_str().unwrap();
    assert!(not.contains("sınıflandırılamadı") && not.contains("adaylar:"), "{not}");
    // Alan bilinmiyor ve belgede anlatım yok: problem uydurulmaz
    let problem = uygulama_onermeleri(&k, "PROBLEM");
    assert!(problem.iter().all(|o| o["hesap"]["guven"] == "unknown"), "{problem:?}");
    let r = p.ok(&["rapor"]);
    assert!(r.contains("ANLAM        [bilinmiyor] alan:"), "{r}");
    assert!(!r.contains("[yorum] problem"), "{r}");
}

#[test]
fn iot_alani_ve_sozluk() {
    let ev = Ev::yeni("d9-anlam2-iot");
    let p = ev.proje("d9_anlam2_flask", "sayac");
    let k = kanit(&p);
    let alan = uygulama_onermeleri(&k, "ALAN");
    assert_eq!(alan[0]["deger"]["id"], "iot", "{}", alan[0]);
}
