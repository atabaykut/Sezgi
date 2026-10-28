//! Güvenlik (d7): gerçek veride yazmanın kapısı.
//! - Gerçek veri olarak açılan adres, gerçek komut yoluyla (terminalde `hedef --gercek-veri`) hatırlanır: bayraksız
//!   `hedef` ile kapatıp terminalsiz `--test-kopyasi` demek kapıları kaldırmaz; MCP `cagir` gerçek sunucuya gitmez.
//! - Aynı sunucunun farklı yazımı (localhost / 127.0.0.1 / başka loopback, varsayılan port, şema, yol, sondaki `/`,
//!   büyük-küçük harf) kapıyı atlatmaz; başka port başka sunucudur.
//! - Koruma sınırı dürüstçe söylenir: terminal denetimi MCP üzerinden bağlanan ajanı durdurur, kabuk erişimi olan ajanı
//!   ya da betiği durdurmaz. Gerçek veride yazma deneyseldir (yardım, açılış, doktor, belgeler).
//! - Ebeveyn süreçlerde sahte terminal (`script`) ya da bilinen kodlama ajanı izi görülürse uyarılır (engel değil).
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::Value;
use std::path::PathBuf;

fn durum_klasoru(p: &Proje) -> PathBuf {
    p.ok(&["mod"]);
    let k = p.ev.join("ayar").join("sezgi").join("uygulamalar");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&k).unwrap().flatten().map(|g| g.path()).filter(|p| p.is_dir()).collect();
    assert_eq!(v.len(), 1, "{v:?}");
    v.remove(0)
}

fn ayar(p: &Proje) -> Value {
    std::fs::read_to_string(durum_klasoru(p).join("ayar.json")).ok().and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null)
}

fn uygulama_adi(p: &Proje) -> String {
    p.kart()["app"]["name"].as_str().unwrap().to_string()
}

/// İnsan terminalde gerçek veride yazmayı açar (gerçek komut yolu; ayar dosyası elle yazılmaz)
fn terminalde_gercek_ac(p: &Proje, adres: &str) -> String {
    p.insan_capasi(); // d8 insan: gerçek veri sistem çapası ister (parolayı sahte_terminalde ekler)
    let (ok, c) = p.sahte_terminalde(&["hedef", adres, "--gercek-veri"], &format!("{}\n", uygulama_adi(p)));
    assert!(ok && c.contains("GERÇEK VERİ"), "{c}");
    c
}

fn test_kopyasi_reddedilir(p: &Proje, adres: &str) {
    let (ok, c) = p.cikti(&["hedef", adres, "--test-kopyasi"]);
    assert!(!ok && c.contains("gerçek veri olarak açılmıştı"), "{adres}: terminalsiz test kopyası bildirimi reddedilmeli: {c}");
}

// ---- ENGEL-1: adres gerçek komut yoluyla hatırlanır ----

#[cfg(unix)]
#[test]
fn gercek_veri_adresi_gercek_komut_yoluyla_hatirlanir() {
    let ev = Ev::yeni("d7g-hatirla");
    let gercek = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    terminalde_gercek_ac(&p, &gercek.adres());
    let a = ayar(&p);
    assert!(a["gercek_adresler"].as_array().is_some_and(|l| !l.is_empty()), "adres hatırlanmalı: {a}");
    // Bayraksız hedef: gerçek veriyi kapatır (güvenli yön, terminalsiz)
    p.ok(&["hedef", &gercek.adres()]);
    // Terminalsiz test kopyası bildirimi: reddedilir, hedef değişmez
    test_kopyasi_reddedilir(&p, &gercek.adres());
    let h = p.ok(&["hedef"]);
    assert!(!h.contains("(test kopyası)") && h.contains("yalnızca okuma"), "{h}");
    // Ajanın kabuğu yazmayı açamaz; MCP `cagir` gerçek sunucuya gitmez
    let (ok, m) = p.cikti(&["mod", "yazma"]);
    assert!(!ok, "{m}");
    p.oturum("OTURUM-ALICE");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(hata, "{s}");
    assert_eq!(gercek.sayi(), 0, "gerçek sunucuya istek gitmemeli");
}

#[cfg(unix)]
#[test]
fn gercek_veri_adresinin_farkli_yazimi_kapiyi_atlatmaz() {
    let ev = Ev::yeni("d7g-yazim");
    let gercek = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    let port = gercek.port;
    terminalde_gercek_ac(&p, &gercek.adres());
    let yazimlar = [
        format!("http://localhost:{port}"),
        format!("http://LOCALHOST:{port}/"),
        format!("http://127.0.0.1:{port}/api/v1"),
        format!("https://localhost:{port}"),
        format!("http://127.0.0.2:{port}"),
        format!("https://0.0.0.0:{port}"),
        format!("https://localhost.:{port}"),
    ];
    // Gerçek veri açıkken
    for y in &yazimlar {
        test_kopyasi_reddedilir(&p, y);
    }
    assert!(p.ok(&["hedef"]).contains("GERÇEK VERİ"), "hedef değişmemeli");
    // Başka bir yazımla kapatıldıktan sonra da
    p.ok(&["hedef", &format!("http://localhost:{port}")]);
    for y in std::iter::once(&gercek.adres()).chain(&yazimlar) {
        test_kopyasi_reddedilir(&p, y);
    }
    // Başka port başka sunucudur: test kopyası olabilir
    let test_kopyasi = Uygulama::yeni();
    assert!(p.sahte_terminalde(&["hedef", &format!("http://localhost:{}", test_kopyasi.port), "--test-kopyasi"], "").0); // d8 insan: çapa varken terminalde, parolayla
    // Varsayılan port: http://localhost = :80
    let ev = Ev::yeni("d7g-port");
    let p = ev.proje("nestjs-temel", "a");
    terminalde_gercek_ac(&p, "http://localhost");
    p.ok(&["hedef", "http://localhost"]);
    test_kopyasi_reddedilir(&p, "http://127.0.0.1:80");
    test_kopyasi_reddedilir(&p, "https://127.0.0.1:80");
    // https://localhost (443) başka porttur
    assert!(p.sahte_terminalde(&["hedef", "https://localhost", "--test-kopyasi"], "").0); // d8 insan: çapa varken terminalde, parolayla
    assert_eq!(gercek.sayi() + test_kopyasi.sayi(), 0);
}

// ---- ENGEL-2: dürüst koruma sınırı, deneysel etiket ----

#[test]
fn koruma_siniri_durust_soylenir() {
    let ev = Ev::yeni("d7g-durust");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    // Terminalsiz açma reddi: mutlak "betik açamaz" iddiası yok, sınır söylenir
    let (ok, c) = p.cikti(&["hedef", &u.adres(), "--gercek-veri"]);
    assert!(!ok && c.contains("yalnızca terminalde"), "{c}");
    assert!(!c.contains("betik bunu açamaz") && c.contains("kabuk erişimi"), "{c}");
    let (ok, c) = p.cikti(&["token", "olustur", "--ad", "ajan", "--izin", "yorum.*:yaz", "--gercek-veri"]);
    assert!(!ok && !c.contains("betik bunu üretemez") && c.contains("kabuk erişimi"), "{c}");
    // Yardım: deneysel ve tehdit modeli
    let y = p.ok(&["yardim", "hedef"]);
    assert!(y.contains("deneysel") && y.contains("kabuk erişimi"), "{y}");
    // Belgeler: mutlak iddia yok, sınır ve deneysel etiket var
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let oku = |ad: &str| std::fs::read_to_string(kok.join(ad)).unwrap();
    let en = oku("README.md");
    assert!(!en.contains("so an agent or a script cannot open it") && en.contains("shell access") && en.contains("experimental"), "README.md");
    let tr = oku("README.tr.md");
    assert!(!tr.contains("yapay zekâ ya da bir betik açamaz") && tr.contains("kabuk erişimi") && tr.contains("deneysel"), "README.tr.md");
    let g = oku("SECURITY.md");
    assert!(!g.contains("so an agent cannot open it") && g.contains("shell access"), "SECURITY.md");
    let c = oku("CHANGELOG.md");
    assert!(!c.contains("so an agent cannot open it") && c.contains("shell access"), "CHANGELOG.md");
}

#[cfg(unix)]
#[test]
fn gercek_veri_acilisi_deneysel_ve_doktorda_gorunur() {
    let ev = Ev::yeni("d7g-deneysel");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    let c = terminalde_gercek_ac(&p, &u.adres());
    assert!(c.contains("DENEYSEL") && c.contains("kabuk erişimi"), "açılış mesajı tehdit modelini söyler: {c}");
    assert!(c.contains("imzalı") && c.contains("Kart imzasız"), "imzalı kart şartı söylenir: {c}");
    let d = p.ok(&["doktor", "."]);
    assert!(d.contains("GERÇEK VERİ") && d.contains("deneysel"), "{d}");
    assert!(d.contains("imzalı kart ister") && !d.contains("imzasız (isteğe bağlı"), "gerçek veri açıkken imza isteğe bağlı değildir: {d}");
}

// ---- Kullanıcıya dönük metinler ----

#[test]
fn ret_ve_kurulum_metinleri_kullaniciya_donuk() {
    let ev = Ev::yeni("d7g-metin");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    let t = p.token("ajan", &["yorum.*:yaz"], &[]);
    let (s, hata) = p.mcp(&t).cagir(yorum_yaz());
    assert!(hata && s.contains("gözlem modunda") && s.contains("Uygulamanın sahibi terminalde açabilir") && !s.contains("Kurucu"), "{s}");
    let k = p.ok(&["canli", "kur", "--hedef", &u.adres(), "--test-kopyasi", "--mod", "okuma", "--izin", "yorum.*:oku", "--token-ad", "ikinci"]);
    assert!(k.contains("2/4") && k.contains("deneysel") && !k.contains("yazma yalnızca test kopyasına)"), "{k}");
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paket = std::fs::read_to_string(kok.join("npm/sezgi/package.json")).unwrap();
    assert!(paket.contains("Laravel") && paket.contains("experimental"), "{paket}");
    let npm = std::fs::read_to_string(kok.join("npm/sezgi/README.md")).unwrap();
    assert!(npm.contains("Laravel") && npm.contains("shell access"), "npm README");
    // d8 insan: durum satırı ya eski sınırı ya da insan parolası sınırını (kabuk erişimli ajan, parola) söyler
    for (ad, iz, yeni) in [("README.md", "does not protect against an agent with shell access", "shell access under the same account cannot open"), ("README.tr.md", "kabuk erişimi olan ajana karşı korumaz", "kabuk erişimi olan bir ajan parolayı bilmeden")] {
        let r = std::fs::read_to_string(kok.join(ad)).unwrap();
        let durum = r.lines().find(|l| l.starts_with("> **")).unwrap();
        assert!(durum.contains(iz) || durum.contains(yeni), "{ad} durum satırı: {durum}");
    }
}

// ---- Ebeveyn süreç izi: bilgi, engel değil ----

#[cfg(target_os = "linux")]
#[test]
fn sahte_terminal_ve_ajan_izi_uyarilir() {
    let ev = Ev::yeni("d7g-iz");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.insan_capasi(); // d8 insan: gerçek veri sistem çapası ister
    // `script` ebeveyn zincirinde: uyarı var, açma yine yapılır (engel değil)
    let (ok, c) = p.sahte_terminalde(&["hedef", &u.adres(), "--gercek-veri"], &format!("{}\n", uygulama_adi(&p)));
    assert!(ok && c.contains("UYARI") && c.contains("script"), "{c}");
    // Kodlama ajanının ortam izi
    let (ok, c) = p.sahte_terminalde_ortamla(&["hedef", &u.adres()], "", &[("CLAUDECODE", "1")]);
    assert!(ok, "{c}");
    let (ok, c) = p.sahte_terminalde_ortamla(&["hedef", &u.adres(), "--gercek-veri"], &format!("{}\n", uygulama_adi(&p)), &[("CLAUDECODE", "1")]);
    assert!(ok && c.contains("UYARI") && c.contains("Claude Code"), "{c}");
}
