//! İlk temas (PROMPT-BAGLAM-KULLANILABILIRLIK K1): argümansız `sezgi` kısa ve yol gösterir; yardım gruplu; bilinmeyen
//! komut en yakın komutu önerir; kısa adlar (`bagla`, `izle`) ve İngilizce karşılıkları çalışır.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn gecici(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let k = std::env::temp_dir().join(format!("sezgi-yardim-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    std::fs::create_dir_all(&k).unwrap();
    k
}

fn sezgi(klasor: &Path, args: &[&str]) -> Output {
    // Yapılandırma klasörü geçici: kullanıcının gerçek bağlantıları ve ayarları okunmaz
    Command::new(env!("CARGO_BIN_EXE_sezgi")).args(args).current_dir(klasor).env("HOME", klasor).env("APPDATA", klasor).env("XDG_CONFIG_HOME", klasor).output().unwrap()
}

fn metin(c: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr))
}

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            kopyala(&p, &hedef.join(g.file_name()));
        } else {
            std::fs::copy(&p, hedef.join(g.file_name())).unwrap();
        }
    }
}

#[test]
fn argumansiz_bos_klasorde_baslangici_soyler() {
    let k = gecici("bos");
    let c = sezgi(&k, &[]);
    let m = metin(&c);
    assert!(c.status.success(), "argümansız sezgi hata değildir: {m}");
    assert!(m.lines().count() <= 12, "en fazla 12 satır ({}):\n{m}", m.lines().count());
    assert!(m.contains("sezgi init"), "ilk komutu söyler: {m}");
    assert!(m.contains("sezgi yardim"), "bütün komutların yerini söyler: {m}");
}

#[test]
fn argumansiz_haritali_klasorde_durum_ve_sonraki_adim() {
    let k = gecici("harita");
    let uygulama = k.join("app");
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &uygulama);
    assert!(sezgi(&uygulama, &["init"]).status.success());
    let c = sezgi(&uygulama, &[]);
    let m = metin(&c);
    assert!(c.status.success(), "{m}");
    assert!(m.lines().count() <= 12, "en fazla 12 satır:\n{m}");
    assert!(m.contains("nestjs-temel-ornek") && m.contains("8 işlem"), "uygulama ve işlem sayısı: {m}");
    assert!(m.contains("sezgi bagla"), "bağlantı yoksa bağlanmayı önerir: {m}");
}

#[test]
fn yardim_gruplu_ve_komut_ayrintili() {
    let k = gecici("yardim");
    for a in [&["yardim"][..], &["help"], &["--help"], &["-h"]] {
        let m = metin(&sezgi(&k, a));
        for grup in ["Başla", "Yapay zekâya bağla", "Bak", "Canlı mod", "Güvenlik ve veri", "Gelişmiş"] {
            assert!(m.contains(grup), "{a:?} yardımında '{grup}' grubu yok:\n{m}");
        }
        assert!(m.contains("init") && m.contains("bagla") && m.contains("connect"), "Türkçe ve İngilizce adlar: {m}");
    }
    let m = metin(&sezgi(&k, &["yardim", "rapor"]));
    assert!(m.contains("sezgi rapor") && m.contains("--json"), "komut ayrıntısı: {m}");
    let m = metin(&sezgi(&k, &["help", "report"]));
    assert!(m.contains("sezgi rapor"), "İngilizce adla da ayrıntı: {m}");
}

#[test]
fn bilinmeyen_komut_en_yakini_onerir() {
    let k = gecici("bilinmeyen");
    let c = sezgi(&k, &["rapr"]);
    let m = metin(&c);
    assert!(!c.status.success());
    assert!(m.contains("Bilinmeyen komut: rapr") && m.contains("sezgi rapor"), "{m}");
    assert!(m.lines().count() <= 4, "kısa: {m}");
    let m = metin(&sezgi(&k, &["doctr"]));
    assert!(m.contains("sezgi doctor") || m.contains("sezgi doktor"), "İngilizce yazımda da öneri: {m}");
    let m = metin(&sezgi(&k, &["foo"]));
    assert!(!m.contains("Bunu mu"), "yakın komut yoksa öneri yok: {m}");
}

#[test]
fn kisa_adlar_calisir() {
    let k = gecici("kisa");
    let uygulama = k.join("app");
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &uygulama);
    assert!(sezgi(&uygulama, &["init"]).status.success());
    // bagla = add mcp; deneme: hiçbir şey yazılmaz
    for a in [&["bagla", "--deneme", "--arac", "claude-code"][..], &["connect", "--dry-run", "--tool", "claude-code"]] {
        let c = sezgi(&uygulama, a);
        let m = metin(&c);
        assert!(c.status.success(), "{a:?}: {m}");
        assert!(m.to_lowercase().contains("claude"), "{a:?}: {m}");
    }
}

/// Kabul ölçütü 2: init çıktısı kısa, iç terimsiz, yolu bir kez basar ve sonraki adımı söyler
#[test]
fn init_ciktisi_kisa_ve_anlasilir() {
    let k = gecici("init");
    let uygulama = k.join("app");
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &uygulama);
    let c = sezgi(&uygulama, &["init"]);
    let m = metin(&c);
    assert!(c.status.success(), "{m}");
    assert!(m.lines().count() <= 12, "en fazla 12 satır ({}):\n{m}", m.lines().count());
    let kucuk = m.to_lowercase();
    for yasak in ["kapı", "adaptör", "şema", "herkese_acik", "varsayim", "endpoint"] {
        assert!(!kucuk.contains(yasak), "iç terim '{yasak}':\n{m}");
    }
    assert!(m.contains("8 · ") && m.contains("girişsiz"), "işlem sayısı ve giriş: {m}");
    assert_eq!(m.matches(".sezgi").count(), 1, "harita yolu bir kez:\n{m}");
    assert!(m.contains("sezgi bagla") && m.contains("sezgi ayna") && m.contains("sezgi izle"), "sonraki adım: {m}");
    let e = metin(&sezgi(&uygulama, &["bagla", "--elle"]));
    assert!(e.contains("claude mcp add sezgi --") && e.contains("mcpServers"), "{e}");
}

/// Kabul ölçütü 3: harita argümansız bulunur (bulunulan klasör ya da üstleri); yoksa nasıl başlanacağı söylenir
#[test]
fn harita_argumansiz_bulunur() {
    let k = gecici("harita-bul");
    let uygulama = k.join("app");
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &uygulama);
    assert!(sezgi(&uygulama, &["init"]).status.success());
    // Alt klasörden: üstteki .sezgi bulunur
    let alt = uygulama.join("src");
    let c = sezgi(&alt, &["rapor"]);
    assert!(c.status.success() && metin(&c).contains("nestjs-temel-ornek"), "{}", metin(&c));
    let c = sezgi(&alt, &["bagla", "--elle"]);
    assert!(c.status.success() && metin(&c).contains(".sezgi"), "{}", metin(&c));
    // `mcp --lang en`: bayrak klasör sanılmaz
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["mcp", "--lang", "en"])
        .current_dir(&alt)
        .env("HOME", &k)
        .env("APPDATA", &k)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!metin(&c).contains("--lang"), "{}", metin(&c));
    // Haritasız klasör: ne yapılacağı söylenir
    let bos = gecici("harita-yok");
    let c = sezgi(&bos, &["rapor"]);
    assert!(!c.status.success() && metin(&c).contains("sezgi init"), "rapor: {}", metin(&c));
}

/// K5: kullanıcıya giden metinde iç değerler insan diliyle; JSON değişmez
#[test]
fn rapor_ve_doktor_insan_diliyle() {
    let k = gecici("insan-dili");
    let uygulama = k.join("app");
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &uygulama);
    assert!(sezgi(&uygulama, &["init"]).status.success());
    let r = metin(&sezgi(&uygulama, &["rapor"]));
    for yasak in ["herkese_acik", "istege_bagli", "varsayim", "kapı", "Kapı", "KAPI", "şema", "endpoint"] {
        assert!(!r.contains(yasak), "rapor metninde '{yasak}':\n{r}");
    }
    assert!(r.contains("girişsiz") && r.contains("giriş gerekli"), "{r}");
    let d = metin(&sezgi(&uygulama, &["doktor"]));
    assert!(!d.contains("endpoint") && d.contains("8 işlem"), "{d}");
    // JSON sözleşmesi aynı
    let j: serde_json::Value = serde_json::from_slice(&sezgi(&uygulama, &["rapor", "--json"]).stdout).unwrap();
    assert!(j["kapilar"]["giris"]["herkese_acik"].is_number(), "{j}");
}

/// Kabul ölçütü 6: desteklenmeyen klasörde neye bakıldığı ve alt klasördeki uygulama söylenir
#[test]
fn desteklenmeyen_klasorde_alt_klasor_onerilir() {
    let k = gecici("alt-oneri");
    std::fs::create_dir_all(k.join("servis")).unwrap();
    std::fs::write(k.join("servis").join("package.json"), r#"{ "name": "s", "dependencies": { "express": "4.21.0" } }"#).unwrap();
    std::fs::create_dir_all(k.join("api").join("node_modules").join("x")).unwrap();
    std::fs::write(k.join("api").join("node_modules").join("x").join("package.json"), r#"{ "dependencies": { "@nestjs/core": "10" } }"#).unwrap();
    let c = sezgi(&k, &["init"]);
    let m = metin(&c);
    assert!(!c.status.success());
    assert!(m.contains("sezgi init servis") && m.contains("Express"), "alt klasördeki uygulama önerilir: {m}");
    assert!(!m.contains("node_modules"), "bağımlılık klasörü önerilmez: {m}");
    assert!(m.contains("package.json") && m.contains("fastapi"), "neye bakıldığı söylenir: {m}");
}

/// K7: README'lerdeki komut tablosu yardımla aynı (her komut, Türkçe ve İngilizce adıyla) ve hızlı başlangıç kısa adları kullanır
#[test]
fn readme_komut_tablosu_yardimla_ayni() {
    let k = gecici("readme");
    let yardim = metin(&sezgi(&k, &["yardim"]));
    let adlar: Vec<String> = yardim.lines().filter(|l| l.starts_with("  ") && !l.starts_with("   ")).filter_map(|l| l.split_whitespace().next()).map(String::from).collect();
    assert!(adlar.len() >= 20, "yardımdan komutlar okunamadı: {adlar:?}");
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for dosya in ["README.md", "README.tr.md"] {
        let r = std::fs::read_to_string(kok.join(dosya)).unwrap();
        for a in &adlar {
            assert!(r.contains(&format!("`{a}`")) || r.contains(&format!("`sezgi {a}")), "{dosya} komut tablosunda `{a}` yok");
        }
        assert!(r.contains("sezgi bagla") && r.contains("sezgi izle"), "{dosya} hızlı başlangıcı kısa adları kullanmalı");
    }
}
