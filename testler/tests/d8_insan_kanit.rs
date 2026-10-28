//! D8 insan: gerçek insan kanıtı. Aynı kullanıcı hesabında kabuk erişimi olan ajan (sahte terminal + Sezgi'nin
//! dosyalarını düzenleme) gerçek veriyi açamaz, gerçek-veri token'ı üretemez, onay veremez, rol beyan edemez.
//! - Sistem çapası (yönetici sahipli dosya) + yalnızca insanın bildiği parola; kayıtlar parolanın anahtarıyla imzalı.
//! - Ajan: `sahte_terminalde_ham` (parolayı bilmez), terminalsiz komut ve dosya düzenleme.
//! - İnsan: `sahte_terminalde` (girdinin sonuna test parolası eklenir) ve `gercek_veri_ac`.
//! - Test kopyası akışı çapa yokken değişmez.
#![cfg(unix)]
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn durum_klasoru(p: &Proje) -> PathBuf {
    p.ok(&["mod"]);
    let k = p.ev.join("ayar").join("sezgi").join("uygulamalar");
    let v: Vec<PathBuf> = std::fs::read_dir(&k).unwrap().flatten().map(|g| g.path()).filter(|p| p.is_dir()).collect();
    assert_eq!(v.len(), 1, "{v:?}");
    v[0].clone()
}

fn json_oku(p: &std::path::Path) -> Value {
    std::fs::read_to_string(p).ok().and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null)
}

/// Ajanın dosya düzenlemesi: durum klasöründeki JSON dosyasını değiştirir
fn duzenle(p: &Proje, dosya: &str, f: impl FnOnce(&mut Value)) {
    let yol = durum_klasoru(p).join(dosya);
    let mut v = json_oku(&yol);
    f(&mut v);
    std::fs::write(&yol, v.to_string()).unwrap();
}

fn imzali_proje(ev: &Ev, ornek: &str) -> Proje {
    let p = ev.proje(ornek, "a");
    p.ok(&["anahtar", "olustur"]);
    p.ok(&["init", ".", "--imzala"]);
    // Gerçek veride yazma sabitlenmiş anahtar ister
    p.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    p
}

fn ad(p: &Proje) -> String {
    p.kart()["app"]["name"].as_str().unwrap().to_string()
}

fn kayit(p: &Proje) -> Vec<Value> {
    p.ok(&["kayit", "--json"]).lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn insan_tokeni(p: &Proje, ad: &str, izin: &str) -> String {
    let (ok, c) = p.sahte_terminalde(&["token", "olustur", "--ad", ad, "--izin", izin, "--gercek-veri"], "evet\n");
    assert!(ok, "{c}");
    c.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string()
}

fn capa_dosyasi(p: &Proje) -> PathBuf {
    p.capa_klasoru().join("insan-0.json")
}

// ---- ENGEL-2: sahte terminal + dosya düzenleme ----

/// Vizyon denetimindeki ENGEL-2 adımları: önce (d7) beşi de geçiyordu; şimdi parolasız her adım reddedilir ve gerçek
/// veriye istek gitmez. İnsan aynı adımları parolasıyla yapınca istek gider ve kayıtta insan kanıtı görünür.
#[test]
fn engel2_adimlari_parolasiz_gecmez() {
    let ev = Ev::yeni("d8i-engel2");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "nestjs-temel");
    p.insan_capasi();
    // 1. Ajan sahte terminalde gerçek veriyi açar: uygulamanın adını bilir, parolayı bilmez
    let (ok, c) = p.sahte_terminalde_ham(&["hedef", &u.adres(), "--gercek-veri"], &format!("{}\n", ad(&p)), &[]);
    assert!(!ok && c.contains("parola"), "{c}");
    let (ok, c) = p.sahte_terminalde_ham(&["hedef", &u.adres(), "--gercek-veri"], &format!("{}\nyanlis-parola-99\n", ad(&p)), &[]);
    assert!(!ok && c.contains("yanlış"), "{c}");
    assert!(!p.ok(&["hedef"]).contains("GERÇEK VERİ"), "açılmamalı");
    // İnsan açar (adı ve parolasıyla)
    p.gercek_veri_ac(&u.adres());
    p.oturum_rollu("OTURUM-ALICE", &["ADMIN"]);
    assert!(p.ok(&["hedef"]).contains("yazma açık"), "{}", p.ok(&["hedef"]));
    // 2. Ajan gerçek-veri token'ı üretir: "evet" yetmez, parola gerekir
    let (ok, c) = p.sahte_terminalde_ham(&["token", "olustur", "--ad", "ajan", "--izin", "yorum.*:yaz", "--gercek-veri"], "evet\n", &[]);
    assert!(!ok && c.contains("parola"), "{c}");
    assert!(!p.ok(&["token", "listele"]).contains("ajan"), "token üretilmemeli");
    // Ajan sıradan token üretip dosyada gerçek-veri işaretini kendisi yazar: imza yok, gerçek veride yalnızca okur
    let sahte = p.token("ajan", &["yorum.*:yaz"], &[]);
    duzenle(&p, "tokenler.json", |l| {
        for t in l.as_array_mut().unwrap() {
            t["gercek_veri"] = json!(true);
        }
    });
    let (s, hata) = p.mcp(&sahte).cagir(yorum_yaz());
    assert!(hata && s.contains("imzasını taşımıyor") && s.contains("yalnızca okur"), "{s}");
    // İnsanın token'ı: istek kuyruğa düşer (3–4. adımlar)
    let t = insan_tokeni(&p, "insan-verdi", "yorum.*:yaz");
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR") && s.contains("parola"), "{s}");
    let id = onay_kimligi(&s);
    // 5. Ajan sahte terminalde onaylar: "evet" yazar, parolayı bilmez → gönderilmez
    let (ok, c) = p.sahte_terminalde_ham(&["onay", "ver", &id], "evet\n", &[]);
    assert!(!ok && c.contains("parola") && c.contains("gönderilmedi"), "{c}");
    let (ok, c) = p.sahte_terminalde_ham(&["onay", "ver", &id], "evet\nyanlis-parola-99\n", &[]);
    assert!(!ok && c.contains("yanlış"), "{c}");
    assert_eq!(u.sayi(), 0, "gerçek veriye istek gitmemeli");
    // İnsan parolasıyla onaylar: gider, kayıtta insan kanıtı (parola ve çapanın izi)
    let (ok, c) = p.sahte_terminalde(&["onay", "ver", &id], "evet\n");
    assert!(ok && c.contains("GERÇEK VERİ"), "{c}");
    assert_eq!(u.sayi(), 1);
    let k = kayit(&p);
    assert!(k.iter().any(|v| v["durum"] == "onaylanip_iletildi" && v["insan_kaniti"] == "parola" && v["capa"].is_string()), "{k:?}");
    assert!(k.iter().any(|v| v["durum"] == "hedef" && v["gercek_veri"] == true && v["insan_kaniti"] == "parola"), "{k:?}");
    assert!(p.ok(&["kayit", "--dogrula"]).contains("sağlam"));
}

/// Ajan ayar dosyasını düzenler: hedefi başka sunucuya çevirir, sınırı yükseltir, gerçek veriyi test kopyası yapar.
/// İmza tutmaz: yazma kapalıya düşer, hedef ve doktor nedenini söyler; insan yeniden açınca düzelir.
#[test]
fn dosya_duzenlemesi_kapaliya_duser() {
    let ev = Ev::yeni("d8i-duzenleme");
    let u = Uygulama::yeni();
    let baska = Uygulama::yeni();
    let p = imzali_proje(&ev, "nestjs-temel");
    p.gercek_veri_ac(&u.adres());
    p.oturum("OTURUM-ALICE");
    let t = insan_tokeni(&p, "ajan", "yorum.*:yaz");
    let mut m = p.mcp(&t);
    let asil = json_oku(&durum_klasoru(&p).join("ayar.json"));
    // Başka sunucu (gerçek veri işareti duruyor)
    duzenle(&p, "ayar.json", |a| a["hedef"]["url"] = json!(baska.adres()));
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(hata && s.contains("GERÇEK VERİ") && s.contains("imzasını taşımıyor"), "{s}");
    assert!(p.ok(&["hedef"]).contains("yazma KAPALI"), "{}", p.ok(&["hedef"]));
    assert!(p.ok(&["doktor", "."]).contains("yazma kapalı"));
    // Saatlik sınır
    std::fs::write(durum_klasoru(&p).join("ayar.json"), asil.to_string()).unwrap();
    duzenle(&p, "ayar.json", |a| a["saatlik_yazma"] = json!(200));
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(hata && s.contains("imzasını taşımıyor"), "{s}");
    // Gerçek veriyi test kopyası diye işaretleyip onaysız yazmak (düşürme)
    duzenle(&p, "ayar.json", |a| {
        a["hedef"] = json!({ "url": u.adres(), "test_kopyasi": true });
        a["gercek_adresler"] = json!([]);
        a["saatlik_yazma"] = Value::Null;
    });
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(hata && s.contains("test kopyası bildirimi insanın imzasını taşımıyor"), "{s}");
    // Komutla düşürme: terminalsiz ve sahte terminalde parolasız reddedilir
    let (ok, c) = p.cikti(&["hedef", &baska.adres(), "--test-kopyasi"]);
    assert!(!ok && c.contains("terminal"), "{c}");
    let (ok, c) = p.sahte_terminalde_ham(&["hedef", &baska.adres(), "--test-kopyasi"], "", &[]);
    assert!(!ok && c.contains("parola"), "{c}");
    assert_eq!(u.sayi() + baska.sayi(), 0, "hiçbir yere yazılmamalı");
    // İmzalı asıl kayıt geri gelince (insanın açtığı hâl) yazma yeniden kuyruğa düşer
    std::fs::write(durum_klasoru(&p).join("ayar.json"), asil.to_string()).unwrap();
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "{s}");
}

/// Çapa yoksa gerçek veride yazma yoktur: terminal ve parola da açamaz, ayar dosyası da. Test kopyası çapasız
/// eskisi gibi çalışır (parola sorulmaz).
#[test]
fn capasiz_gercek_veri_kapali_test_kopyasi_degismez() {
    let ev = Ev::yeni("d8i-capasiz");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "nestjs-temel");
    let (ok, c) = p.sahte_terminalde(&["hedef", &u.adres(), "--gercek-veri"], &format!("{}\n", ad(&p)));
    assert!(!ok && c.contains("sistem çapası yok"), "{c}");
    let (ok, c) = p.sahte_terminalde(&["token", "olustur", "--ad", "x", "--izin", "yorum.*:yaz", "--gercek-veri"], "evet\n");
    assert!(!ok && c.contains("sistem çapası yok"), "{c}");
    // Ayar dosyasıyla (d7'de testlerin de yaptığı gibi)
    p.oturum("OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    duzenle(&p, "ayar.json", |a| {
        a["hedef"] = json!({ "url": u.adres(), "test_kopyasi": false, "gercek_veri": true });
        a["mod"] = json!("yazma");
    });
    duzenle(&p, "tokenler.json", |l| l[0]["gercek_veri"] = json!(true));
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(hata && s.contains("sistem çapası yok"), "{s}");
    assert_eq!(u.sayi(), 0);
    let i = p.ok(&["insan"]);
    assert!(i.contains("Sistem çapası  yok") && i.contains("sudo sezgi insan kur --sistem"), "{i}");
    assert!(p.ok(&["canli"]).contains("sistem çapası yok"));
    // Test kopyası: çapasız, parolasız, eskisi gibi
    let tk = Uygulama::yeni();
    p.ok(&["hedef", &tk.adres(), "--test-kopyasi"]);
    p.ok(&["mod", "yazma"]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(tk.sayi(), 1);
}

/// Çapa yalnızca yönetici sahipli ve başkasının yazamadığı dosyadır: grup/başkaları yazabiliyorsa ya da sahibi kök
/// değilse sayılmaz (ajanın kendi kurduğu "çapa" gerçek veriyi açmaz). Yönetici olmayan kullanıcı çapayı kuramaz,
/// değiştiremez; şimdiki parola bilinmeden parola değişmez.
#[test]
fn sahte_ya_da_korunmayan_capa_sayilmaz() {
    let ev = Ev::yeni("d8i-sahte-capa");
    let u = Uygulama::yeni();
    let p = imzali_proje(&ev, "nestjs-temel");
    p.insan_capasi();
    let dosya = capa_dosyasi(&p);
    let klasor = p.capa_klasoru();
    assert!(p.ok(&["insan"]).contains("iz "), "{}", p.ok(&["insan"]));
    // Klasör herkese yazılabilir: geçersiz, gerçek veri açılmaz
    std::fs::set_permissions(&klasor, std::fs::Permissions::from_mode(0o777)).unwrap();
    let i = p.ok(&["insan"]);
    assert!(i.contains("geçersiz") && i.contains("yazılabilir"), "{i}");
    let (ok, c) = p.sahte_terminalde(&["hedef", &u.adres(), "--gercek-veri"], &format!("{}\n", ad(&p)));
    assert!(!ok && c.contains("çapası geçersiz"), "{c}");
    std::fs::set_permissions(&klasor, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Sahibi kök değil (ajanın yazdığı dosya)
    std::os::unix::fs::chown(&dosya, Some(65534), Some(65534)).unwrap();
    let i = p.ok(&["insan"]);
    assert!(i.contains("geçersiz") && i.contains("kök"), "{i}");
    std::os::unix::fs::chown(&dosya, Some(0), Some(0)).unwrap();
    // Sembolik bağ çapa sayılmaz
    let asil = klasor.join("asil.json");
    std::fs::rename(&dosya, &asil).unwrap();
    std::os::unix::fs::symlink(&asil, &dosya).unwrap();
    assert!(p.ok(&["insan"]).contains("sembolik"));
    std::fs::remove_file(&dosya).unwrap();
    std::fs::rename(&asil, &dosya).unwrap();
    // Yönetici olmayan kullanıcı (ajan) çapayı ne kurabilir ne değiştirebilir
    let icerik = std::fs::read_to_string(&dosya).unwrap();
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["insan", "kur", "--sistem", "--kullanici", "0"])
        .env("SEZGI_CAPA_KLASORU", &klasor)
        .current_dir("/")
        .uid(65534)
        .gid(65534)
        .output()
        .unwrap();
    let m = String::from_utf8_lossy(&c.stderr).to_string();
    assert!(!c.status.success() && m.contains("yönetici"), "{m}");
    let c = Command::new("sh").args(["-c", &format!("echo x >> '{}'", dosya.display())]).uid(65534).gid(65534).output().unwrap();
    assert!(!c.status.success(), "kök olmayan kullanıcı çapayı değiştirememeli");
    // Kök olarak bile şimdiki parola bilinmeden parola değişmez
    let (ok, c) = p.sahte_terminalde_ham(&["insan", "kur", "--sistem", "--kullanici", "0"], "yanlis-parola-99\nyeni-parola-12345\nyeni-parola-12345\n", &[]);
    assert!(!ok && c.contains("yanlış"), "{c}");
    assert_eq!(std::fs::read_to_string(&dosya).unwrap(), icerik, "çapa değişmemeli");
    // Kısa parola kabul edilmez (yeni çapa)
    std::fs::remove_file(&dosya).unwrap();
    let (ok, c) = p.sahte_terminalde_ham(&["insan", "kur", "--sistem", "--kullanici", "0"], "kisa\nkisa\n", &[]);
    assert!(!ok && c.contains("en az 12"), "{c}");
    assert!(!dosya.exists());
}

/// Çapa varken rol beyanı insan parolasıyla imzalanır: sahte terminalde parolasız beyan ve oturum dosyasına elle
/// yazılan rol doğrulanmış sayılmaz (onaysız yazma yok); insanın parolalı beyanı sayılır.
#[test]
fn rol_beyani_capa_ile_parola_ister() {
    let ev = Ev::yeni("d8i-rol");
    let u = Uygulama::yeni();
    let p = ev.proje("s3_rol", "a");
    p.insan_capasi();
    let (ok, c) = p.sahte_terminalde(&["hedef", &u.adres(), "--test-kopyasi"], "");
    assert!(ok, "{c}");
    p.ok(&["mod", "yazma"]);
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    // Ajan: sahte terminal, parolasız
    let (ok, c) = p.sahte_terminalde_ham(&["oturum", "ac", "--rol", "EDITOR"], "OTURUM-ALICE\n", &[]);
    assert!(ok && c.contains("doğrulanmış sayılmadı") && !c.contains("Doğrulanmış roller"), "{c}");
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("ONAY BEKLİYOR") && s.contains("rol"), "{s}");
    // Ajan: oturum dosyasına rol yazar
    duzenle(&p, "oturum.json", |o| {
        o["roller"] = json!(["EDITOR"]);
        o["rol_kaynaklari"] = json!(["insan_beyani"]);
    });
    let mut y = yorum_yaz();
    y["tekrar_anahtari"] = json!("ikinci");
    let (s, hata) = p.mcp(&t).cagir(y);
    assert!(!hata && s.contains("ONAY BEKLİYOR"), "elle yazılan rol sayılmaz: {s}");
    assert_eq!(u.sayi(), 0);
    // İnsan: parolasıyla
    let m = p.oturum_rollu("OTURUM-ALICE", &["EDITOR"]);
    assert!(m.contains("Doğrulanmış roller") && m.contains("EDITOR"), "{m}");
    let mut y = yorum_yaz();
    y["tekrar_anahtari"] = json!("ucuncu");
    let (s, hata) = p.mcp(&t).cagir(y);
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(u.sayi(), 1);
}

/// Çapa varken test kopyası bildirimi de insan imzalıdır: terminalsiz ya da parolasız bildirilemez; ajanın hedefi
/// ayar dosyasında başka bir sunucuya çevirmesi yazmayı kapatır
#[test]
fn test_kopyasi_capa_ile_imzali() {
    let ev = Ev::yeni("d8i-test-kopyasi");
    let u = Uygulama::yeni();
    let baska = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.insan_capasi();
    let (ok, c) = p.cikti(&["hedef", &u.adres(), "--test-kopyasi"]);
    assert!(!ok && c.contains("terminal"), "{c}");
    let (ok, c) = p.sahte_terminalde_ham(&["hedef", &u.adres(), "--test-kopyasi"], "", &[]);
    assert!(!ok && c.contains("parola"), "{c}");
    let (ok, c) = p.sahte_terminalde(&["hedef", &u.adres(), "--test-kopyasi"], "");
    assert!(ok && c.contains("imzalandı"), "{c}");
    p.ok(&["mod", "yazma"]);
    p.oturum("OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    assert_eq!(u.sayi(), 1);
    duzenle(&p, "ayar.json", |a| a["hedef"]["url"] = json!(baska.adres()));
    let mut y = yorum_yaz();
    y["tekrar_anahtari"] = json!("ikinci");
    let (s, hata) = p.mcp(&t).cagir(y);
    assert!(hata && s.contains("imzasını taşımıyor"), "{s}");
    assert_eq!(baska.sayi(), 0);
    let (ok, _) = p.cikti(&["mod", "yazma"]);
    assert!(!ok, "imzasız hedefle yazma modu açılmaz");
}

// ---- Görünürlük ve metinler (Y1, Y2) ----

fn mcp_satiri(p: &Proje, token: Option<&str>, istekler: &[Value]) -> Vec<Value> {
    let mut c = p.komut(&["mcp", ".sezgi"]);
    if let Some(t) = token {
        c.env("SEZGI_TOKEN", t);
    }
    let mut s = c.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut girdi = s.stdin.take().unwrap();
    let mut satirlar = BufReader::new(s.stdout.take().unwrap()).lines();
    let mut cevaplar = vec![];
    for i in istekler {
        writeln!(girdi, "{i}").unwrap();
        girdi.flush().unwrap();
        cevaplar.push(serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap());
    }
    let _ = s.kill();
    let _ = s.wait();
    cevaplar
}

#[test]
fn metinler_kurucu_yok_kaynak_turkce_insan_gorunur() {
    let ev = Ev::yeni("d8i-metin");
    let p = ev.proje("nestjs-temel", "a");
    // Y1: cagir aracının açıklaması ve geçersiz token mesajı "kurucu" demez
    let c = mcp_satiri(&p, Some("szg_yanlis"), &[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })]);
    let araclar = c[0].to_string();
    assert!(araclar.contains("uygulamanın sahibi") && !araclar.contains("kurucu"), "{araclar}");
    let c = mcp_satiri(&p, Some("szg_yanlis"), &[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "ara", "arguments": { "sorgu": "yorum" } } })]);
    assert!(c[0].to_string().contains("uygulamanın sahibinden") && !c[0].to_string().contains("kurucu"), "{}", c[0]);
    // Y2: Ayna başlığı rapor ile aynı metni kullanır (git olmayan klasör: "kod sürümü yok")
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    assert_eq!(r["git"], false, "{r}");
    assert!(p.ok(&["rapor"]).contains("kod sürümü yok (git yok)"));
    let js = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join("ayna").join("ayna.js")).unwrap();
    assert!(js.contains("kod sürümü yok (git yok)") && !js.contains("kaynak ${metin(o.kaynak_ref)}"), "Ayna başlığı");
    // Yeni komut yardımda ve İngilizce adıyla
    assert!(p.ok(&["yardim"]).contains("insan (human)"));
    assert!(p.ok(&["human", "status"]).contains("İnsan anahtarı"));
    let (ok, c) = p.cikti(&["insan", "kur"]);
    assert!(!ok && c.contains("--sistem"), "{c}");
    // Çapa kurulunca durum, canlı ve Ayna'nın onay sorusu söyler
    p.insan_capasi();
    assert!(p.ok(&["insan"]).contains("Koruma"));
    assert!(p.ok(&["canli"]).contains("sistem çapası var"));
}
