//! P0 G4 (komut satırı): yardım hiçbir yan etki yapmaz; bilinmeyen ya da değersiz bayrak hata verir; İngilizce takma
//! adlar gerçekten çevrilir; silme işleri onay ister; çıktı borusu kapanınca panik yok; `bagla` dürüst sonuç verir.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Ortam {
    kok: PathBuf,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let kok = std::env::temp_dir().join(format!("sezgi-p0g4-{ad}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("ev")).unwrap();
        std::fs::create_dir_all(kok.join("bos")).unwrap();
        Ortam { kok }
    }
    fn ev(&self) -> PathBuf {
        self.kok.join("ev")
    }
    fn app(&self) -> PathBuf {
        self.kok.join("app")
    }
    fn komut(&self, klasor: &Path, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args).current_dir(klasor).env("HOME", self.ev()).env("APPDATA", self.ev()).env("XDG_CONFIG_HOME", self.ev()).env("SEZGI_ANAHTARLIK", "kapali").stdin(Stdio::null());
        c
    }
    fn calistir(&self, klasor: &Path, args: &[&str]) -> Output {
        self.komut(klasor, args).output().unwrap()
    }
    /// nestjs-temel kopyası + `init`
    fn haritali(&self) -> PathBuf {
        kopyala(&ornek("nestjs-temel"), &self.app());
        let c = self.calistir(&self.app(), &["init"]);
        assert!(c.status.success(), "{}", metin(&c));
        self.app()
    }
}

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
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

fn metin(c: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr))
}

/// Bir klasördeki bütün dosyaların içeriği (yan etki karşılaştırması)
fn goruntu(k: &Path) -> BTreeMap<String, Vec<u8>> {
    fn gez(k: &Path, taban: &Path, m: &mut BTreeMap<String, Vec<u8>>) {
        for g in std::fs::read_dir(k).into_iter().flatten().flatten() {
            let p = g.path();
            if p.is_dir() {
                gez(&p, taban, m);
            } else {
                m.insert(p.strip_prefix(taban).unwrap().display().to_string(), std::fs::read(&p).unwrap());
            }
        }
    }
    let mut m = BTreeMap::new();
    gez(k, k, &mut m);
    m
}

#[test]
fn yardim_bayragi_hicbir_komutta_yan_etki_yapmaz() {
    let o = Ortam::yeni("yardim");
    let app = o.haritali();
    assert!(o.calistir(&app, &["anahtar", "olustur", "--ad", "k1"]).status.success());
    let c = o.calistir(&app, &["init", "--imzala", "k1"]);
    assert!(c.status.success(), "{}", metin(&c));
    let c = o.calistir(&app, &["paylas", "--imzala", "k1"]);
    assert!(c.status.success(), "{}", metin(&c));
    let c = o.calistir(&app, &["bagla", "--arac", "claude-code"]);
    assert!(c.status.success(), "{}", metin(&c));
    assert!(app.join(".mcp.json").is_file());
    let once = goruntu(&o.kok);
    for a in [
        &["init", "--help"][..],
        &["init", "-h"],
        &["paylas", "--help"],
        &["kaldir", "--help"],
        &["kaldir", "help"],
        &["remove", "-h"],
        &["unut", "--hepsi", "--help"],
        &["bagla", "--help"],
        &["santral", "ekle", "--help"],
        &["canli", "kur", "--help"],
        &["izle", "--help"],
        &["ayna", "--help"],
    ] {
        let c = o.calistir(&app, a);
        let m = metin(&c);
        assert!(c.status.success(), "{a:?} yardımı başarı döner: {m}");
        assert!(m.contains("Kullanım: sezgi") || m.contains("sezgi "), "{a:?} kullanım satırı: {m}");
        assert!(!m.contains('✓') && !m.contains("İzleniyor") && !m.contains("SEZGİ AYNA"), "{a:?} komutu çalıştırmamalı: {m}");
        assert_eq!(goruntu(&o.kok), once, "{a:?} hiçbir dosyayı değiştirmemeli");
    }
    // Haritasız klasörde `rapor -h` harita aramaz
    let c = o.calistir(&o.kok.join("bos"), &["rapor", "-h"]);
    let m = metin(&c);
    assert!(c.status.success() && m.contains("sezgi rapor") && !m.contains("harita yok"), "{m}");
}

#[test]
fn bilinmeyen_ya_da_degersiz_bayrak_hata_verir() {
    let o = Ortam::yeni("bayrak");
    kopyala(&ornek("nestjs-temel"), &o.app());
    let c = o.calistir(&o.app(), &["init", "--imzla", "k1"]);
    let m = metin(&c);
    assert_eq!(c.status.code(), Some(2), "{m}");
    assert!(m.contains("--imzla") && m.contains("--imzala"), "yazım hatası öneriyle bildirilir: {m}");
    assert!(!o.app().join(".sezgi").exists(), "hatalı bayrakla hiçbir şey yazılmaz");
    let c = o.calistir(&o.app(), &["init", "--cikti"]);
    assert_eq!(c.status.code(), Some(2), "değer isteyen bayrak değersiz: {}", metin(&c));
    assert!(!o.app().join(".sezgi").exists());
    let bos = o.kok.join("bos");
    for (a, aranan) in [(&["ayna", "--port", "abc"][..], "--port"), (&["sun", "--port", "99999"], "--port"), (&["rapor", "--jsn"], "--json")] {
        let c = o.calistir(&bos, a);
        let m = metin(&c);
        assert_eq!(c.status.code(), Some(2), "{a:?}: {m}");
        assert!(m.contains(aranan), "{a:?}: {m}");
        assert!(!m.contains("harita yok"), "bayrak hatası önce bildirilir: {m}");
    }
}

#[test]
fn ingilizce_delete_map_ve_kaldir_onayi() {
    let o = Ortam::yeni("kaldir");
    let app = o.haritali();
    assert!(o.calistir(&app, &["bagla", "--arac", "claude-code"]).status.success());
    // Terminal yok, --evet yok: hiçbir şey silinmez, neyin silineceği söylenir, çıkış ≠ 0
    let c = o.calistir(&app, &["kaldir"]);
    let m = metin(&c);
    assert!(!c.status.success(), "{m}");
    assert!(m.contains(".mcp.json") && m.contains("--evet"), "liste ve nasıl onaylanacağı: {m}");
    assert!(app.join(".mcp.json").is_file(), "onaysız silinmez");
    // İngilizce: remove --delete-map --yes
    let c = o.calistir(&app, &["remove", "--delete-map", "--yes"]);
    let m = metin(&c);
    assert!(c.status.success(), "{m}");
    assert!(!app.join(".mcp.json").exists(), "bağlantı geri alındı: {m}");
    assert!(!app.join(".sezgi").exists(), "--delete-map haritayı siler: {m}");
}

#[test]
fn unut_hepsi_onay_ister_ve_kismi_basarisizlik_basari_sayilmaz() {
    let o = Ortam::yeni("unut");
    let bos = o.kok.join("bos");
    let c = o.calistir(&bos, &["unut", "--hepsi"]);
    let m = metin(&c);
    assert!(!c.status.success() && m.contains("--evet"), "onaysız toplu silme yok: {m}");
    let c = o.calistir(&bos, &["forget", "--all", "--yes"]);
    assert!(c.status.success(), "{}", metin(&c));
    // Çözülemeyen kayıt satırı olan uygulama atlanırsa sonuç başarı değildir
    let uyg = o.ev().join("sezgi").join("uygulamalar").join("bozuk-uygulama");
    std::fs::create_dir_all(&uyg).unwrap();
    std::fs::write(uyg.join("kayit.jsonl"), "bozuk-satir\n").unwrap();
    let c = o.calistir(&bos, &["unut", "--kisi", "kisi@ornek.test"]);
    let m = metin(&c);
    assert!(!c.status.success(), "kısmi başarısızlık çıkış ≠ 0: {m}");
    assert!(!m.contains('✓'), "kısmi başarısızlıkta ✓ yok: {m}");
}

#[test]
fn ingilizce_alt_komutlar_cevrilir_ya_da_hata_verir() {
    let o = Ortam::yeni("ingilizce");
    let app = o.haritali();
    let c = o.calistir(&app, &["memory", "on"]);
    assert!(c.status.success(), "{}", metin(&c));
    assert!(metin(&o.calistir(&app, &["hafiza"])).contains("açık"), "memory on hafızayı açar");
    let c = o.calistir(&app, &["diagnostics", "on"]);
    assert!(c.status.success(), "{}", metin(&c));
    assert!(metin(&o.calistir(&app, &["teshis"])).contains("açık"), "diagnostics on tanılamayı açar");
    let c = o.calistir(&app, &["switchboard", "add", app.join(".sezgi").to_str().unwrap(), "--name", "uyg"]);
    assert!(c.status.success(), "{}", metin(&c));
    assert!(metin(&o.calistir(&app, &["santral"])).contains("uyg"), "switchboard add santrale ekler");
    for a in [&["hafiza", "acik"][..], &["teshis", "xyz"], &["santral", "eklee"], &["token", "yap"]] {
        let c = o.calistir(&app, a);
        let m = metin(&c);
        assert_eq!(c.status.code(), Some(2), "{a:?}: {m}");
        assert!(m.contains("alt komut"), "{a:?}: {m}");
    }
    // `sezgi add` (ve `ekle`) bağlama yönlendirilir, `mod` önerilmez
    for a in [&["add"][..], &["ekle"]] {
        let m = metin(&o.calistir(&app, a));
        assert!(!m.contains("sezgi mod") && !m.contains("Bilinmeyen komut"), "{a:?}: {m}");
    }
    // Önerilen komut gerçekten çalışır: bağlantı yokken doktorun önerisi
    let d = metin(&o.calistir(&app, &["doktor"]));
    if let Some(s) = d.lines().find_map(|l| l.split("isteğe bağlı: ").nth(1)) {
        let oneri: Vec<&str> = s.trim_end_matches(')').split_whitespace().skip(1).collect();
        let c = o.calistir(&app, &oneri);
        assert!(!metin(&c).contains("Bilinmeyen komut"), "doktorun önerisi çalışır: {oneri:?}: {}", metin(&c));
    }
}

#[test]
fn cikti_borusu_kapaninca_panik_yok() {
    let o = Ortam::yeni("boru");
    let app = o.haritali();
    for a in [&["yardim"][..], &["rapor"]] {
        let mut c = o.komut(&app, a).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        drop(c.stdout.take()); // okuyan taraf kapandı
        let c = c.wait_with_output().unwrap();
        let e = String::from_utf8_lossy(&c.stderr);
        assert!(!e.contains("panicked"), "{a:?}: {e}");
        assert_eq!(c.status.code(), Some(0), "{a:?}: {e}");
    }
}

#[test]
fn bagla_hicbir_sey_eklemezse_durust_soyler() {
    let o = Ortam::yeni("bagla");
    let app = o.haritali();
    assert!(o.calistir(&app, &["bagla", "--arac", "claude-code"]).status.success());
    let c = o.calistir(&app, &["bagla", "--arac", "claude-code"]);
    let m = metin(&c);
    assert!(c.status.success(), "zaten ekli olmak hata değil: {m}");
    assert!(!m.contains("yeniden başlatın"), "hiçbir şey eklenmediyse yeniden başlatma istenmez: {m}");
    assert!(m.contains("zaten"), "{m}");
}
