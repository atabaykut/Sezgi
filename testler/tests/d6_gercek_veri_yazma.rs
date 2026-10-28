//! Gerçek veride yazma: test kopyasından sıkı kapılarla.
//! - Açma (`sezgi hedef <adres> --gercek-veri`), gerçek-veri token'ı (`token olustur --gercek-veri`) ve saatlik sınır
//!   (`--saatlik-yazma`) yalnızca terminalde: ajan (terminalsiz) açamaz.
//! - Otomatik yazma yok: politika ne derse desin her yazma onay kuyruğuna düşer; `--ozet` ile terminalsiz onay reddedilir.
//! - Kart imzalı, harita bayat değil, önizleme bütün kuralları denetlemiş olmalı; eski token yalnızca okur; hız sınırı.
//! - Kayıtta `gercek_veri: true`, MCP cevabında ve `onay goster`'de "GERÇEK VERİ". Santral planları aynı kurallara uyar.
//! - Test kopyasının davranışı değişmez.
//!   Terminal gerektiren yollar ayar dosyası doğrudan yazılarak, uçtan uca yollar ise sahte terminalle (`script`) sınanır.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn durum_klasoru(p: &Proje) -> PathBuf {
    p.ok(&["mod"]);
    let k = p.ev.join("ayar").join("sezgi").join("uygulamalar");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&k).unwrap().flatten().map(|g| g.path()).filter(|p| p.is_dir()).collect();
    assert_eq!(v.len(), 1, "{v:?}");
    v.remove(0)
}

fn json_oku(p: &std::path::Path) -> Value {
    std::fs::read_to_string(p).ok().and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null)
}

/// Terminalin yapacağını test ortamında doğrudan ayar dosyasına yazar: hedef gerçek veri, mod yazma
fn gercek_ac(p: &Proje, u: &Uygulama) {
    // d8 insan: ayar dosyasını elle yazmak gerçek veriyi artık açmaz (insan imzası tutmaz); gerçek komut yoluyla
    p.gercek_veri_ac(&u.adres());
}

/// Terminalde `--gercek-veri` ile üretilmiş token (d8 insan: gerçek komut yoluyla, insan parolasıyla)
fn gercek_token(p: &Proje, ad: &str, izinler: &[&str]) -> String {
    let mut a = vec!["token", "olustur", "--ad", ad];
    for i in izinler {
        a.extend(["--izin", i]);
    }
    a.push("--gercek-veri");
    let (ok, c) = p.sahte_terminalde(&a, "evet\n");
    assert!(ok, "{c}");
    c.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string()
}

/// İnsan oturumu, doğrulanmış ADMIN rolüyle (silme ADMIN ister)
fn admin_oturumu(p: &Proje) {
    p.insan_capasi(); // d8 insan: çapa varken rol insan parolasıyla imzalanır (çapadan önceki beyan sayılmaz)
    p.oturum_rollu("OTURUM-ALICE", &["ADMIN"]);
}

/// İmzalı kartla haritalanmış proje
fn imzali_proje(ev: &Ev, ad: &str) -> Proje {
    let p = ev.proje("nestjs-temel", ad);
    p.ok(&["anahtar", "olustur"]);
    p.ok(&["init", ".", "--imzala"]);
    // Gerçek veride yazma sabitlenmiş anahtar ister
    p.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    assert_ne!(p.kart()["signature"]["alg"], "none");
    p
}

fn kayit(p: &Proje) -> Vec<Value> {
    p.ok(&["kayit", "--json"]).lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn simdi() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

/// Komutu sahte bir terminalde (`script`) çalıştırır; `girdi` terminale yazılır. (başarılı mı, çıktı)
fn terminalde(p: &Proje, args: &[&str], girdi: &str) -> (bool, String) {
    // d8 insan: gerçek veri sistem çapası ister; insan adımlarında cevaptan sonra insan parolası
    p.insan_capasi();
    let girdi = &format!("{girdi}{TEST_PAROLASI}\n");
    let tirnak = |a: &str| format!("'{}'", a.replace('\'', "'\\''"));
    let komut: Vec<String> = std::iter::once(env!("CARGO_BIN_EXE_sezgi")).chain(args.iter().copied()).map(tirnak).collect();
    let mut c = Command::new("script");
    c.args(["-qec", &komut.join(" "), "/dev/null"])
        .current_dir(&p.klasor)
        .env("HOME", &p.ev)
        .env("USERPROFILE", &p.ev)
        .env("APPDATA", p.ev.join("ayar"))
        .env("XDG_CONFIG_HOME", p.ev.join("ayar"))
        .env_remove("SEZGI_TOKEN")
        .env("SEZGI_ANAHTARLIK", "kapali")
        .env("SEZGI_CAPA_KLASORU", p.capa_klasoru())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut s = c.spawn().expect("script (util-linux) gerekli");
    s.stdin.as_mut().unwrap().write_all(girdi.as_bytes()).unwrap();
    drop(s.stdin.take());
    let o = s.wait_with_output().unwrap();
    (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

// ---- Ajan açamaz ----

#[test]
fn gercek_veri_terminalsiz_acilamaz() {
    let ev = Ev::yeni("gv-acma");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    // Terminal yok (ajanın kabuğu gibi): uygulamanın adı standart girdiden verilse de açılmaz
    let ad = p.kart()["app"]["name"].as_str().unwrap().to_string();
    let c = p.sezgi(&["hedef", &u.adres(), "--gercek-veri"], Some(&format!("{ad}\n")));
    let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(!c.status.success() && m.contains("yalnızca terminalde"), "{m}");
    assert!(p.ok(&["hedef"]).contains("Hedef yok"), "ayar değişmemeli");
    // İki işaret birlikte verilemez (kullanım hatası, çıkış 2)
    let c = p.sezgi(&["hedef", &u.adres(), "--test-kopyasi", "--gercek-veri"], None);
    assert_eq!(c.status.code(), Some(2), "{}", String::from_utf8_lossy(&c.stderr));
    // Gerçek veride yazan token ve saatlik sınır da yalnızca terminalde
    let (ok, m) = p.cikti(&["token", "olustur", "--ad", "ajan", "--izin", "yorum.*:yaz", "--gercek-veri"]);
    assert!(!ok && m.contains("yalnızca terminalde"), "{m}");
    assert!(!p.ok(&["token", "listele"]).contains("ajan"), "token üretilmemeli");
    let (ok, m) = p.cikti(&["hedef", "--saatlik-yazma", "50"]);
    assert!(!ok && m.contains("terminal"), "{m}");
    let (ok, _) = p.cikti(&["hedef", "--saatlik-yazma", "500"]);
    assert!(!ok, "aralık dışı sınır reddedilir");
    // Test kopyası olmayan hedefte yazma modu açılmaz
    p.ok(&["hedef", &u.adres()]);
    let (ok, m) = p.cikti(&["mod", "yazma"]);
    assert!(!ok && m.contains("--gercek-veri"), "{m}");
    assert_eq!(u.sayi(), 0);
}

// ---- Eski token yalnızca okur ----

#[test]
fn gercek_veride_eski_token_yalnizca_okur() {
    let ev = Ev::yeni("gv-eski");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "a");
    p.oturum("OTURUM-ALICE");
    gercek_ac(&p, &u);
    let t = p.token("eski", &["kitap.*:yaz", "yorum.*:yaz"], &[]);
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(hata && s.contains("GERÇEK VERİ") && s.contains("yalnızca okur") && s.contains("--gercek-veri"), "{s}");
    assert_eq!(u.sayi(), 0);
    let (s, hata) = m.cagir(json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(!hata && s.contains("OKUNDU"), "eski token gerçek veride okur: {s}");
    assert_eq!(u.sayi(), 1);
}

// ---- Otomatik yazma yok; terminalsiz onay reddedilir ----

#[test]
fn gercek_veride_her_yazma_kuyruga_duser_ve_ozetle_onaylanamaz() {
    let ev = Ev::yeni("gv-kuyruk");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "a");
    admin_oturumu(&p);
    gercek_ac(&p, &u);
    let t = gercek_token(&p, "ajan", &["yorum.*:yaz", "kitap.*:yikici"]);
    let mut m = p.mcp(&t);
    // yorum.yaz test kopyasında onaysız gider (kesin, önizleme geçer); gerçek veride kuyruğa düşer
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(!hata && s.contains("GERÇEK VERİ") && s.contains("ONAY BEKLİYOR") && s.contains("gerçek veride her yazma insan onayı ister"), "{s}");
    assert_eq!(u.sayi(), 0, "gerçek veride otomatik yazma yok");
    let id = onay_kimligi(&s);
    let (s, _) = m.arac("onay_durumu", json!({ "id": id }));
    assert!(s.contains("GERÇEK VERİ"), "{s}");
    // onay goster uyarır ve --ozet önermez
    let g = p.ok(&["onay", "goster", &id]);
    assert!(g.contains("GERÇEK VERİ") && !g.contains(&format!("{id} --ozet")), "{g}");
    // Geri alma: koddan (en fazla tahmin) ve Sezgi'nin yedek almadığı açıkça söylenir
    assert!(g.contains("Geri alma: ") && g.contains("yedek"), "{g}");
    let ozet = p.onay_ozeti(&id);
    // Terminalsiz onay: doğru özetle de reddedilir, istek gitmez
    let (ok, c) = p.cikti(&["onay", "ver", &id, "--ozet", &ozet]);
    assert!(!ok && c.contains("GERÇEK VERİ") && c.contains("--ozet"), "{c}");
    let (ok, c) = p.cikti(&["onay", "ver", &id]);
    assert!(!ok && c.contains("terminal"), "{c}");
    assert_eq!(u.sayi(), 0);
    // Reddedilen onay hâlâ bekliyor (kilit bırakıldı)
    assert!(p.ok(&["onay", "listele"]).contains(&id));
    // Kayıt: kuyruğa alma satırı gerçek veri işaretli
    let k = kayit(&p);
    assert!(k.iter().any(|v| v["durum"] == "onay_bekliyor" && v["gercek_veri"] == true && v["islem"] == "yorum.yaz"), "{k:?}");
    // Yıkıcı işlem de kuyruğa
    let (s, hata) = m.cagir(json!({ "kapi": "DELETE /api/v1/kitaplar/:id", "parametreler": { "id": "7" } }));
    assert!(!hata && s.contains("GERÇEK VERİ") && s.contains("ONAY BEKLİYOR"), "{s}");
    assert_eq!(u.sayi(), 0);
}

// ---- Kart, önizleme ve hız kapıları ----

#[test]
fn gercek_veride_imzasiz_kart_denetlenemeyen_onizleme_ve_hiz_siniri() {
    let ev = Ev::yeni("gv-kapilar");
    let u = Uygulama::yeni();
    // İmzasız kart: kuyruğa bile alınmaz, gerekçe açık
    let p = ev.proje("nestjs-temel", "imzasiz");
    p.oturum("OTURUM-ALICE");
    gercek_ac(&p, &u);
    let t = gercek_token(&p, "ajan", &["yorum.*:yaz", "kitap.*:yaz"]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(hata && s.contains("kart imzasız") && s.contains("--imzala"), "{s}");
    assert!(p.ok(&["onay", "listele"]).contains("Onay bekleyen istek yok"));

    let ev = Ev::yeni("gv-kapilar-imzali");
    let p = imzali_proje(&ev, "imzali");
    p.oturum("OTURUM-ALICE");
    gercek_ac(&p, &u);
    let t = gercek_token(&p, "ajan", &["yorum.*:yaz", "kitap.*:yaz"]);
    let mut m = p.mcp(&t);
    // Önizlemesi bir kuralı denetleyemeyen yazma (ValidateIf): gerçek veride onaylanamaz
    let (s, hata) = m.cagir(kitap_olustur());
    assert!(hata && s.contains("GERÇEK VERİ") && s.contains("denetleyemedi"), "{s}");
    // İmzadan sonra değişen kart: imza geçersiz
    let kart_yolu = p.klasor.join(".sezgi").join("benlik.json");
    let asil = std::fs::read_to_string(&kart_yolu).unwrap();
    let mut kart: Value = serde_json::from_str(&asil).unwrap();
    kart["app"]["version"] = json!("9.9.9");
    std::fs::write(&kart_yolu, kart.to_string()).unwrap();
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(hata && s.contains("imza"), "{s}");
    std::fs::write(&kart_yolu, asil).unwrap();
    // Hız sınırı: son bir saatte sınır kadar gerçek-veri yazması yapılmışsa yeni yazma kuyruğa da alınmaz
    let k = durum_klasoru(&p);
    let n = simdi();
    std::fs::write(k.join("gercek_yazmalar.json"), json!((0..20).map(|i| n - i).collect::<Vec<u64>>()).to_string()).unwrap();
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(hata && s.contains("saatlik yazma sınırı doldu") && s.contains("sınır 20"), "{s}");
    // Daha yüksek sınır terminalde verilir: yeniden açılır (d8 insan: ayarı elle yazmak imzayı bozar, gerçek komut)
    let (ok, c) = terminalde(&p, &["hedef", "--saatlik-yazma", "21"], "");
    assert!(ok, "{c}");
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "{s}");
    // Aralık dışı değer varsayılana düşer (20): sınır yine dolu (d8 insan: elle yazılan değer imzayı da bozar)
    let mut a = json_oku(&k.join("ayar.json"));
    a["saatlik_yazma"] = json!(100_000);
    std::fs::write(k.join("ayar.json"), a.to_string()).unwrap();
    let mut y = yorum_yaz();
    y["tekrar_anahtari"] = json!("ikinci");
    let (s, hata) = m.cagir(y);
    assert!(hata && (s.contains("sınır 20") || s.contains("imzasını taşımıyor")), "{s}");
    assert_eq!(u.sayi(), 0);
}

// ---- Görünürlük: hedef, mod, doktor ----

#[test]
fn gercek_veri_hedef_ve_doktorda_gorunur() {
    let ev = Ev::yeni("gv-gorunur");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    gercek_ac(&p, &u);
    let h = p.ok(&["hedef"]);
    assert!(h.contains("GERÇEK VERİ") && h.contains("saatte en çok 20"), "{h}");
    assert!(p.ok(&["mod"]).contains("GERÇEK VERİ"));
    assert!(p.ok(&["canli"]).contains("GERÇEK VERİ"));
    let d = p.ok(&["doktor", "."]);
    assert!(d.contains("GERÇEK VERİ") && d.contains(&u.adres()), "{d}");
    let j: Value = serde_json::from_str(&p.ok(&["doktor", ".", "--json"])).unwrap();
    assert!(j.to_string().contains("GERÇEK VERİ"), "{j}");
    // Bayraksız hedef gerçek veriyi kapatır (terminal gerekmez: güvenli yön)
    p.ok(&["hedef", &u.adres()]);
    let h = p.ok(&["hedef"]);
    assert!(!h.contains("GERÇEK VERİ") && h.contains("yalnızca okuma"), "{h}");
    assert_eq!(p.ok(&["mod"]).lines().next().unwrap(), "Mod: okuma", "yazma modu düşer");
}

// ---- Test kopyası değişmedi ----

#[test]
fn test_kopyasi_davranisi_degismedi() {
    let ev = Ev::yeni("gv-test-kopyasi");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz", "kitap.*:yikici"], &[]);
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI") && !s.contains("GERÇEK VERİ"), "test kopyasında kesin yazma onaysız gider: {s}");
    assert_eq!(u.sayi(), 1);
    // Yıkıcı onay terminalsiz --ozet ile verilir (imzasız kartla da)
    let (s, _) = m.cagir(json!({ "kapi": "DELETE /api/v1/kitaplar/:id", "parametreler": { "id": "7" } }));
    let id = onay_kimligi(&s);
    let ozet = p.onay_ozeti(&id);
    p.ok(&["onay", "ver", &id, "--ozet", &ozet]);
    assert_eq!(u.sayi(), 2);
    assert!(kayit(&p).iter().all(|v| v["gercek_veri"].is_null()), "test kopyası yazmaları gerçek veri sayılmaz");
    assert!(p.ok(&["hedef"]).contains("(test kopyası)"));
}

// ---- Uçtan uca: sahte terminalde açma, token, onay ----

#[cfg(unix)]
#[test]
fn gercek_veri_terminalde_uctan_uca() {
    let ev = Ev::yeni("gv-uctan-uca");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "a");
    let ad = p.kart()["app"]["name"].as_str().unwrap().to_string();
    // Yanlış ad: açılmaz
    let (ok, c) = terminalde(&p, &["hedef", &u.adres(), "--gercek-veri"], "baska-uygulama\n");
    assert!(!ok && c.contains("adı yanlış"), "{c}");
    assert!(!p.ok(&["hedef"]).contains("GERÇEK VERİ"));
    // Doğru ad: açılır, ayarda kalıcı
    let (ok, c) = terminalde(&p, &["hedef", &u.adres(), "--gercek-veri", "--saatlik-yazma", "2"], &format!("{ad}\n"));
    assert!(ok && c.contains("GERÇEK VERİ"), "{c}");
    assert!(p.ok(&["hedef"]).contains("saatte en çok 2"));
    p.ok(&["mod", "yazma"]);
    admin_oturumu(&p);
    // Token: terminalde "evet" ile
    let (ok, c) = terminalde(&p, &["token", "olustur", "--ad", "ajan", "--izin", "yorum.*:yaz", "--izin", "kitap.*:yikici", "--gercek-veri"], "e\n");
    assert!(!ok && c.contains("üretilmedi"), "kısaltma kabul edilmez: {c}");
    let (ok, c) = terminalde(&p, &["token", "olustur", "--ad", "ajan", "--izin", "yorum.*:yaz", "--izin", "kitap.*:yikici", "--gercek-veri"], "evet\n");
    assert!(ok, "{c}");
    let t = c.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
    assert!(p.ok(&["token", "listele"]).contains("GERÇEK VERİ"));
    let mut m = p.mcp(&t);
    let (s, _) = m.cagir(yorum_yaz());
    let id = onay_kimligi(&s);
    // Terminalde "e" yetmez; "evet" ile gider
    let (ok, c) = terminalde(&p, &["onay", "ver", &id], "e\n");
    assert!(!ok && c.contains("onaylanmadı"), "{c}");
    assert_eq!(u.sayi(), 0);
    let (ok, c) = terminalde(&p, &["onay", "ver", &id], "evet\n");
    assert!(ok && c.contains("GERÇEK VERİ"), "{c}");
    assert_eq!(u.sayi(), 1);
    assert_eq!(u.gelenler.lock().unwrap()[0].yetki.as_deref(), Some("Bearer OTURUM-ALICE"));
    let k = kayit(&p);
    assert!(k.iter().any(|v| v["durum"] == "onaylanip_iletildi" && v["gercek_veri"] == true), "{k:?}");
    assert!(p.ok(&["kayit", "--dogrula"]).contains("sağlam"));
    // Yıkıcı: "evet" yetmez, yöntem ve yol aynen
    let (s, _) = m.cagir(json!({ "kapi": "DELETE /api/v1/kitaplar/:id", "parametreler": { "id": "7" } }));
    let id = onay_kimligi(&s);
    let (ok, _) = terminalde(&p, &["onay", "ver", &id], "evet\n");
    assert!(!ok);
    assert_eq!(u.sayi(), 1);
    let (ok, c) = terminalde(&p, &["onay", "ver", &id], "DELETE /api/v1/kitaplar/7\n");
    assert!(ok, "{c}");
    assert_eq!(u.sayi(), 2);
    // Saatlik sınır (2) doldu: yeni yazma kuyruğa alınmaz
    let mut y = yorum_yaz();
    y["tekrar_anahtari"] = json!("ucuncu");
    let (s, hata) = m.cagir(y);
    assert!(hata && s.contains("saatlik yazma sınırı doldu"), "{s}");
    assert_eq!(u.sayi(), 2);
}

// ---- Santral planları aynı kurallara uyar ----

struct Santral {
    p: std::process::Child,
    stdin: std::process::ChildStdin,
    satirlar: Box<dyn Iterator<Item = std::io::Result<String>>>,
}

impl Drop for Santral {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
    }
}

impl Santral {
    fn ac(p: &Proje, token: &str) -> Santral {
        let mut c = p.komut(&["mcp", "--santral"]).env("SEZGI_TOKEN", token).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let stdin = c.stdin.take().unwrap();
        let satirlar = Box::new(BufReader::new(c.stdout.take().unwrap()).lines());
        Santral { p: c, stdin, satirlar }
    }
    fn plan(&mut self, a: Value) -> (String, bool) {
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "plan", "arguments": a } })).unwrap();
        self.stdin.flush().unwrap();
        let r = serde_json::from_str::<Value>(&self.satirlar.next().unwrap().unwrap()).unwrap()["result"].clone();
        (r["content"][0]["text"].as_str().unwrap_or("").to_string(), r["isError"] == true)
    }
}

fn plan_argumani() -> Value {
    json!({ "baslik": "yorum", "adimlar": [ { "uygulama": "kitapci", "kapi": "POST /api/v1/yorumlar", "govde": yorum_yaz()["govde"] } ] })
}

#[test]
fn santral_plani_gercek_veride_ayni_kurallara_uyar() {
    let ev = Ev::yeni("gv-plan");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "a");
    p.ok(&["santral", "ekle", ".sezgi", "--ad", "kitapci"]);
    p.oturum("OTURUM-ALICE");
    gercek_ac(&p, &u);
    // Eski token: gerçek veride yazma adımı plana girmez
    let eski = p.token("eski", &["yorum.*:yaz"], &[]);
    let (s, hata) = Santral::ac(&p, &eski).plan(plan_argumani());
    assert!(hata && s.contains("yalnızca okur"), "{s}");
    let t = gercek_token(&p, "ajan", &["yorum.*:yaz"]);
    let (s, hata) = Santral::ac(&p, &t).plan(plan_argumani());
    assert!(!hata && s.contains("GERÇEK VERİ"), "{s}");
    let id = s.split("PLAN ").nth(1).unwrap().split_whitespace().next().unwrap().to_string();
    let g = p.ok(&["plan", "goster", &id]);
    assert!(g.contains("GERÇEK VERİ") && !g.contains("--adim 1 --ozet"), "{g}");
    let ozet = g.split("Özet: ").nth(1).unwrap().chars().take(8).collect::<String>();
    let (ok, c) = p.cikti(&["plan", "onayla", &id, "--adim", "1", "--ozet", &ozet]);
    assert!(!ok && c.contains("GERÇEK VERİ") && c.contains("--ozet"), "{c}");
    let (ok, c) = p.cikti(&["plan", "onayla", &id, "--adim", "1"]);
    assert!(!ok && c.contains("terminal"), "{c}");
    assert_eq!(u.sayi(), 0);
    #[cfg(unix)]
    {
        let (ok, c) = terminalde(&p, &["plan", "onayla", &id, "--adim", "1"], "evet\n");
        assert!(ok, "{c}");
        assert_eq!(u.sayi(), 1);
        assert!(kayit(&p).iter().any(|v| v["durum"] == "plan_onaylanip_iletildi" && v["gercek_veri"] == true));
    }
}

// ---- Gerçek veri test kopyası diye yeniden bildirilip kapılar atlanamaz ----

#[test]
fn gercek_veri_test_kopyasi_diye_yeniden_bildirilemez() {
    let ev = Ev::yeni("gv-dusurme");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "a");
    admin_oturumu(&p);
    gercek_ac(&p, &u);
    let t = gercek_token(&p, "ajan", &["yorum.*:yaz"]);
    let mut m = p.mcp(&t);
    let (s, _) = m.cagir(yorum_yaz());
    let id = onay_kimligi(&s);
    // Terminalsiz: açık gerçek veri hedefi test kopyası diye bildirilemez (sondaki / fark etmez)
    let (ok, c) = p.cikti(&["hedef", &format!("{}/", u.adres()), "--test-kopyasi"]);
    assert!(!ok && c.contains("gerçek veri olarak açılmıştı") && c.contains("terminal"), "{c}");
    assert!(p.ok(&["hedef"]).contains("GERÇEK VERİ"), "hedef değişmemeli");
    // Önce kapatıp (güvenli yön, terminalsiz) sonra test kopyası demek de olmaz: adres hatırlanır
    p.ok(&["hedef", &u.adres()]);
    let (ok, c) = p.cikti(&["hedef", &u.adres(), "--test-kopyasi"]);
    assert!(!ok && c.contains("gerçek veri olarak açılmıştı"), "{c}");
    // Başka bir adres test kopyası olabilir
    let test_kopyasi = Uygulama::yeni();
    // d8 insan: sistem çapası kuruluyken test kopyası bildirimi de insan parolasıyla, terminalde
    let (ok, c) = terminalde(&p, &["hedef", &test_kopyasi.adres(), "--test-kopyasi"], "");
    assert!(ok, "{c}");
    p.ok(&["mod", "yazma"]);
    // Gerçek veride kuyruğa alınmış istek, hedef artık test kopyası olsa da terminalsiz (--ozet) onaylanamaz
    let g = p.ok(&["onay", "goster", &id]);
    assert!(g.contains("GERÇEK VERİ"), "{g}");
    let ozet = p.onay_ozeti(&id);
    let (ok, c) = p.cikti(&["onay", "ver", &id, "--ozet", &ozet]);
    assert!(!ok && c.contains("GERÇEK VERİ") && c.contains("--ozet"), "{c}");
    assert_eq!(u.sayi() + test_kopyasi.sayi(), 0);
    #[cfg(unix)]
    {
        // Terminalde, insan uygulamanın adını yazarak: bildirebilir
        let ad = p.kart()["app"]["name"].as_str().unwrap().to_string();
        let (ok, c) = terminalde(&p, &["hedef", &u.adres(), "--test-kopyasi"], "yanlis\n");
        assert!(!ok && c.contains("adı yanlış"), "{c}");
        let (ok, c) = terminalde(&p, &["hedef", &u.adres(), "--test-kopyasi"], &format!("{ad}\n"));
        assert!(ok && c.contains("test kopyası"), "{c}");
    }
}
