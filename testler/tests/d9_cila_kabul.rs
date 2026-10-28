//! D9 cila: son kabulün sonraki sürüme bırakılan küçük bulguları, bu sürümde.
//! - S2: sistem çapası kuruluyken terminalsiz `--test-kopyasi` reddi yeni sınırı (insan parolası) söyler; çapa yokken
//!   terminal denetiminin sınırı kalır. İki dilde.
//! - S3: `insan kur --sistem --kullanici <ad>` Unix'te adı `/etc/passwd`'den uid'ye çevirir.
//! - S4: Ktor `kimsin`/`app`: genel önek tek eğik çizgiyle, alt kaynak yeteneği grubunun adını yinelemez.
//! - S5: İngilizce çıktıda 1 için tekil ("1 file", "1 token", "1 request"); `approve list` seviye adları İngilizce.
#![cfg(unix)]
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

fn imzali_proje(ev: &Ev, ornek: &str) -> Proje {
    let p = ev.proje(ornek, "a");
    p.ok(&["anahtar", "olustur"]);
    p.ok(&["init", ".", "--imzala"]);
    // Gerçek veride yazma sabitlenmiş anahtar ister
    p.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    p
}

/// MCP araç çağrıları (seçilen dilde); cevap metinleri
fn mcp_metinleri(p: &Proje, dil: &str, cagrilar: &[(&str, Value)]) -> Vec<String> {
    let mut s = p.komut(&["--lang", dil, "mcp", ".sezgi"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut girdi = s.stdin.take().unwrap();
    let mut satirlar = BufReader::new(s.stdout.take().unwrap()).lines();
    let mut v = vec![];
    for (i, (ad, a)) in cagrilar.iter().enumerate() {
        let istek = json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": ad, "arguments": a } });
        writeln!(girdi, "{istek}").unwrap();
        girdi.flush().unwrap();
        let c: Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
        v.push(c["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string());
    }
    let _ = s.kill();
    let _ = s.wait();
    v
}

const ESKI_SINIR_TR: &str = "kabuk erişimi olan bir ajanı ya da betiği durdurmaz";
const ESKI_SINIR_EN: &str = "it does not stop an agent or script with shell access";

#[test]
fn s2_capa_varken_test_kopyasi_reddi_parola_sinirini_soyler() {
    let ev = Ev::yeni("d9c-s2");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "nestjs-temel");
    p.gercek_veri_ac(&u.adres());
    // Çapa kurulu: terminalsiz bildirim reddedilir, sınır insan parolasıdır (eski "terminal denetimi" metni yok)
    let (ok, c) = p.cikti(&["hedef", &u.adres(), "--test-kopyasi"]);
    assert!(!ok && c.contains("terminal yok") && c.contains("insan parolasını ister") && !c.contains(ESKI_SINIR_TR), "{c}");
    let (ok, c) = p.cikti(&["--lang", "en", "hedef", &u.adres(), "--test-kopyasi"]);
    assert!(!ok && c.contains("no terminal") && c.contains("human password") && !c.contains(ESKI_SINIR_EN), "{c}");
    // Çapa yoksa (yönetici silmiş) terminal denetiminin sınırı söylenir
    std::fs::remove_file(p.capa_klasoru().join("insan-0.json")).unwrap();
    let (ok, c) = p.cikti(&["hedef", &u.adres(), "--test-kopyasi"]);
    assert!(!ok && c.contains(ESKI_SINIR_TR), "{c}");
    let (ok, c) = p.cikti(&["--lang", "en", "hedef", &u.adres(), "--test-kopyasi"]);
    assert!(!ok && c.contains(ESKI_SINIR_EN), "{c}");
    assert_eq!(u.sayi(), 0);
}

#[test]
fn s3_kullanici_adi_uid_ye_cevrilir() {
    let ev = Ev::yeni("d9c-s3");
    let p = ev.proje("nestjs-temel", "a");
    let parola = format!("{TEST_PAROLASI}\n{TEST_PAROLASI}\n");
    // Ad (/etc/passwd): root → 0, nobody → 65534
    let (ok, c) = p.sahte_terminalde_ham(&["insan", "kur", "--sistem", "--kullanici", "root"], &parola, &[]);
    assert!(ok, "{c}");
    assert!(p.capa_klasoru().join("insan-0.json").exists(), "{c}");
    assert!(p.ok(&["insan"]).contains("iz "), "{}", p.ok(&["insan"]));
    let (ok, c) = p.sahte_terminalde_ham(&["insan", "kur", "--sistem", "--kullanici", "nobody"], &parola, &[]);
    assert!(ok && c.contains("(65534)"), "{c}");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p.capa_klasoru().join("insan-65534.json")).unwrap()).unwrap();
    assert_eq!(v["kullanici"], "65534");
    // Bilinmeyen ad: açık hata, dosya yazılmaz; uid yine kabul edilir
    let (ok, c) = p.cikti(&["insan", "kur", "--sistem", "--kullanici", "boyle-biri-yok"]);
    assert!(!ok && c.contains("kullanıcı bulunamadı") && c.contains("id -u"), "{c}");
    let (ok, c) = p.cikti(&["--lang", "en", "insan", "kur", "--sistem", "--kullanici", "boyle-biri-yok"]);
    assert!(!ok && c.contains("user not found"), "{c}");
    assert!(!p.capa_klasoru().join("insan-boyle-biri-yok.json").exists());
    let (ok, c) = p.cikti(&["insan", "kur", "--sistem", "--kullanici", ""]);
    assert!(!ok && c.contains("boş"), "{c}");
}

#[test]
fn s4_ktor_kimsin_onek_ve_yetenek_tekrarsiz() {
    let ev = Ev::yeni("d9c-s4");
    let p = ev.proje("d8_kotlin_ktor", "a");
    for (dil, arac, onek) in [("tr", "kimsin", "genel önek /app ·"), ("en", "app", "global prefix /app ·")] {
        let s = &mcp_metinleri(&p, dil, &[(arac, json!({}))])[0];
        assert!(s.contains(onek) && !s.contains("//app"), "{s}");
        assert!(!s.contains("gorevler: gorevler:"), "{s}");
        let satir = s.lines().find(|l| l.trim_start().starts_with("gorevler: ")).unwrap_or_else(|| panic!("{s}"));
        assert!(satir.contains("yorumlar"), "{satir}");
    }
    // Eğik çizgisiz önek (NestJS "api/v1") aynı biçimde gösterilir
    let ev2 = Ev::yeni("d9c-s4-nest");
    let n = ev2.proje("nestjs-temel", "a");
    let s = &mcp_metinleri(&n, "en", &[("app", json!({}))])[0];
    assert!(s.contains("global prefix /api/v1 ·") && !s.contains("prefix //"), "{s}");
}

#[test]
fn s5_ingilizce_tekil_cogul_ve_seviye_adlari() {
    // Tek dosyalı, tek işlemli uygulama: "1 file", "1 operation", "1 capability"
    let ev = Ev::yeni("d9c-s5");
    let p = ev.proje("d8_dis_ucus", "a");
    let i = p.ok(&["--lang", "en", "init", "."]);
    assert!(i.contains("Read         1 file ·") && !i.contains("1 files"), "{i}");
    let r = p.ok(&["--lang", "en", "rapor"]);
    for beklenen in ["Read         1 file ·", "1 capability, 1 resource", "CALLS        1 function ·", "CARD         1 operation ·"] {
        assert!(r.contains(beklenen), "{beklenen}: {r}");
    }
    for yanlis in ["1 files", "1 capabilities", "1 resources", "1 functions", "1 operations"] {
        assert!(!r.contains(yanlis), "{yanlis}: {r}");
    }
    // Türkçe değişmez
    let r = p.ok(&["rapor"]);
    assert!(r.contains("Okunan       1 dosya ·"), "{r}");

    // Canlı mod: 1 token, onay bekleyen 1 istek; approve list seviye adı İngilizce
    let u = Uygulama::yeni();
    let q = ev.proje("nestjs-temel", "b");
    q.kur(&u, "OTURUM-ALICE");
    let t = q.token("tek", &["yorum.*:yaz"], &["--onay", "hepsi"]);
    let (s, hata) = q.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "{s}");
    let c = q.ok(&["--lang", "en", "canli"]);
    assert!(c.contains("Token      1 token (") && c.contains("1 request awaiting") && !c.contains("1 tokens") && !c.contains("1 requests"), "{c}");
    let c = q.ok(&["canli"]);
    assert!(c.contains("Token      1 token (") && c.contains("1 istek insan onayı bekliyor"), "{c}");
    let l = q.ok(&["--lang", "en", "onay", "listele"]);
    assert!(l.contains("(write)") && !l.contains("(yaz)"), "{l}");
    let l = q.ok(&["onay", "listele"]);
    assert!(l.contains("(yaz)"), "{l}");

    // describe: "when" bağlacı yinelenmez (except bloğu)
    let ev3 = Ev::yeni("d9c-s5-tani");
    let f = ev3.proje("w2g_fastapi_template/backend", "a");
    let s = &mcp_metinleri(&f, "en", &[("describe", json!({ "kapi": "POST /api/v1/users/" }))])[0];
    assert!(s.contains("when an error is caught") && !s.contains("when when"), "{s}");
}
