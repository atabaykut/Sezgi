//! D6 `sezgi rea kur | durum | kaldir`: REA'nın Sezgi yapılandırma klasöründeki özel öneke kurulması. Ağ yok: sahte npm
//! (betik) sahte REA kurulumunu (gerçek etki tablosu, gerçek bütünlük kaydı) öneke kopyalar; sahte node REA'nın yerine
//! geçer. Denetlenen:
//! - kurulum terminalde açık "evet" ister; terminal yoksa ya da "evet" yazılmazsa npm hiç çağrılmaz, önek oluşmaz;
//! - kurulmadan önce paket, sürüm, kaynak, bütünlük ve hedef klasör gösterilir; npm `ci --prefix <önek>
//!   --ignore-scripts --no-audit --no-fund` ile, Sezgi'nin kilit dosyasıyla çağrılır;
//! - kurulumdan sonra köprünün sabitleme denetimleri çalışır; tutmazsa ya da npm hata verirse kurulan silinir;
//! - Node yoksa ya da sürümü uygun değilse açık mesaj;
//! - köprü REA'yı önce bu önekte bulur (`SEZGI_REA` verilmediyse); `doktor --rea` ve `kesfet --rea` REA yoksa
//!   `sezgi rea kur` önerir;
//! - `rea kaldir` yalnızca Sezgi'nin kurduğu öneki siler ve onay ister.
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const BUTUNLUK: &str = "sha512-yhIx2ioJYYT1C/tONfYeitC6LhnKmCh+ZwRUOgYuhSc733lGhkkt73zhug9WmLgxoMdKuqIYGCoBfD4fXkOhFw==";

fn ornekler() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler")
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

#[cfg(unix)]
fn calistirilabilir(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

struct Ortam {
    kok: PathBuf,
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-d6-rea-kur-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        kopyala(&ornekler().join("w3r_sahte_rea"), &kok.join("rea"));
        kopyala(&ornekler().join("d6_rea_kur_sahte_npm"), &kok.join("npm"));
        #[cfg(unix)]
        {
            calistirilabilir(&kok.join("rea/node"));
            calistirilabilir(&kok.join("npm/npm"));
        }
        std::fs::create_dir_all(kok.join("ayar")).unwrap();
        std::fs::create_dir_all(kok.join("bos")).unwrap();
        Ortam { kok }
    }

    fn onek(&self) -> PathBuf {
        self.kok.join("ayar/sezgi/rea")
    }

    fn iz(&self) -> String {
        std::fs::read_to_string(self.kok.join("npm-iz.txt")).unwrap_or_default()
    }

    fn ortam(&self, c: &mut Command) {
        c.env_remove("SEZGI_REA")
            .env("SEZGI_NODE", self.kok.join("rea/node"))
            .env("SEZGI_NPM", self.kok.join("npm/npm"))
            .env("D6_IZ", self.kok.join("npm-iz.txt"))
            .env("D6_KAYNAK", self.kok.join("rea/paket/node_modules"))
            .env("W3R_CEVAP", self.kok.join("rea/cevap"))
            .env("PATH", "/usr/bin:/bin")
            .env("APPDATA", self.kok.join("ayar"))
            .env("XDG_CONFIG_HOME", self.kok.join("ayar"))
            .env("HOME", self.kok.join("ayar"))
            .current_dir(self.kok.join("bos"));
    }

    fn calis(&self, args: &[&str], ek: &[(&str, &str)]) -> (Option<i32>, String) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args).stdin(Stdio::null());
        self.ortam(&mut c);
        for (a, d) in ek {
            c.env(a, d);
        }
        let o = c.output().unwrap();
        (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    }

    /// Sözde terminalde (`script`) çalıştırır; `cevap` terminale yazılır
    fn terminalde(&self, args: &[&str], cevap: &str, ek: &[(&str, &str)]) -> (Option<i32>, String) {
        let satir = std::iter::once(env!("CARGO_BIN_EXE_sezgi").to_string()).chain(args.iter().map(|a| format!("'{a}'"))).collect::<Vec<_>>().join(" ");
        let mut c = Command::new("script");
        c.args(["-qec", &satir, "/dev/null"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        self.ortam(&mut c);
        for (a, d) in ek {
            c.env(a, d);
        }
        let mut p = c.spawn().unwrap();
        let mut g = p.stdin.take().unwrap();
        let _ = g.write_all(cevap.as_bytes());
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = g.write_all(b"\x04");
        let o = p.wait_with_output().unwrap();
        drop(g);
        // `script -e` alt sürecin çıkış kodunu döner
        (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    }

    /// Öneki sahte npm'siz, elle kurulmuş gibi hazırlar
    fn elle_kur(&self) {
        kopyala(&self.kok.join("rea/paket/node_modules"), &self.onek().join("node_modules"));
        std::fs::write(self.onek().join("package.json"), r#"{"name":"sezgi-rea","private":true,"dependencies":{"rea-agents":"6.7.0"}}"#).unwrap();
    }
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

#[test]
fn d6_rea_kur_terminal_yoksa_kurmaz() {
    let o = Ortam::yeni("terminalsiz");
    let (kod, m) = o.calis(&["rea", "kur"], &[]);
    assert_ne!(kod, Some(0), "{m}");
    assert!(m.contains("terminal"), "{m}");
    assert!(o.iz().is_empty(), "npm çağrılmamalı: {}", o.iz());
    assert!(!o.onek().exists(), "önek oluşmamalı");
}

#[test]
fn d6_rea_kur_evet_yazilmazsa_kurmaz() {
    let o = Ortam::yeni("hayir");
    let (kod, m) = o.terminalde(&["rea", "kur"], "hayir\n", &[]);
    assert_ne!(kod, Some(0), "{m}");
    assert!(m.contains("kurulmadı"), "{m}");
    assert!(o.iz().is_empty(), "npm çağrılmamalı: {}", o.iz());
    assert!(!o.onek().exists());
}

#[test]
fn d6_rea_kur_evetle_kurar_gosterir_ve_dogrular() {
    let o = Ortam::yeni("iyi");
    let (kod, m) = o.terminalde(&["rea", "kur"], "evet\n", &[("D6_KIP", "iyi")]);
    assert_eq!(kod, Some(0), "{m}");
    // Kurulmadan önce gösterilenler
    for p in ["rea-agents", "6.7.0", "MIT", "github.com/morluto/rea", BUTUNLUK, "--ignore-scripts", "evet"] {
        assert!(m.contains(p), "'{p}' gösterilmeli: {m}");
    }
    assert!(m.contains(&o.onek().display().to_string()), "hedef klasör gösterilmeli: {m}");
    // npm: kilit dosyasıyla, betiksiz
    let iz = o.iz();
    assert!(iz.starts_with("ci "), "{iz}");
    for p in ["--prefix", &o.onek().display().to_string(), "--ignore-scripts", "--no-audit", "--no-fund"] {
        assert!(iz.contains(p), "npm argümanı '{p}' yok: {iz}");
    }
    let kilit: Value = serde_json::from_str(&std::fs::read_to_string(o.onek().join("package-lock.json")).unwrap()).unwrap();
    assert_eq!(kilit["packages"]["node_modules/rea-agents"]["integrity"], BUTUNLUK);
    let pj: Value = serde_json::from_str(&std::fs::read_to_string(o.onek().join("package.json")).unwrap()).unwrap();
    assert_eq!(pj["dependencies"]["rea-agents"], "6.7.0", "sürüm tam sabit olmalı");
    // Sabitleme denetimleri çalıştı
    assert!(m.contains("doğrulandı"), "{m}");
    // Durum: kurulu ve hazır; köprü öneki SEZGI_REA olmadan bulur
    let (kod, d) = o.calis(&["rea", "durum"], &[]);
    assert_eq!(kod, Some(0), "{d}");
    assert!(d.contains("hazır") && d.contains(&o.onek().display().to_string()), "{d}");
    let (_, j) = o.calis(&["rea", "durum", "--json"], &[]);
    let j: Value = serde_json::from_str(&j).unwrap_or_else(|e| panic!("{e}: {j}"));
    assert_eq!(j["durum"], "hazir", "{j}");
    assert_eq!(j["surum"], "6.7.0");
    assert_eq!(j["kaynak"], "onek", "{j}");
    // İkinci kurulum: zaten hazır, npm yeniden çağrılmaz
    let (kod, m2) = o.terminalde(&["rea", "kur"], "evet\n", &[("D6_KIP", "iyi")]);
    assert_eq!(kod, Some(0), "{m2}");
    assert!(m2.contains("zaten kurulu"), "{m2}");
    assert_eq!(o.iz().lines().count(), 1, "npm bir kez çağrılmalı: {}", o.iz());
}

#[test]
fn d6_rea_kur_sabitleme_tutmazsa_kurulani_siler() {
    let o = Ortam::yeni("bozuk");
    let (kod, m) = o.terminalde(&["rea", "kur"], "evet\n", &[("D6_KIP", "bozuk")]);
    assert_eq!(kod, Some(1), "{m}");
    assert!(m.contains("etki tablosu") && m.contains("silindi"), "{m}");
    assert!(!o.onek().exists(), "kurulan silinmeli");
}

#[test]
fn d6_rea_kur_npm_hata_verirse_kurulani_siler() {
    let o = Ortam::yeni("hata");
    let (kod, m) = o.terminalde(&["rea", "kur"], "evet\n", &[("D6_KIP", "hata")]);
    assert_eq!(kod, Some(1), "{m}");
    assert!(m.contains("npm") && m.contains("silindi"), "{m}");
    assert!(!o.onek().exists(), "yarım kurulum silinmeli");
}

#[test]
fn d6_rea_kur_node_yoksa_acik_mesaj() {
    let o = Ortam::yeni("nodesuz");
    let (kod, m) = o.calis(&["rea", "kur"], &[("SEZGI_NODE", ""), ("PATH", o.kok.join("bos").to_str().unwrap())]);
    assert_eq!(kod, Some(1), "{m}");
    assert!(m.contains("Node.js bulunamadı") && m.contains("nodejs.org"), "{m}");
    assert!(m.contains("22.19") && m.contains("24.11"), "uygun sürümler söylenmeli: {m}");
    assert!(o.iz().is_empty());
}

#[test]
fn d6_rea_kur_node_surumu_uygun_degilse_acik_mesaj() {
    let o = Ortam::yeni("eski-node");
    let eski = o.kok.join("eski-node");
    std::fs::write(&eski, "#!/bin/sh\necho v20.11.1\n").unwrap();
    #[cfg(unix)]
    calistirilabilir(&eski);
    let (kod, m) = o.calis(&["rea", "kur"], &[("SEZGI_NODE", eski.to_str().unwrap())]);
    assert_eq!(kod, Some(1), "{m}");
    assert!(m.contains("v20.11.1") && m.contains("uygun değil"), "{m}");
    assert!(o.iz().is_empty());
}

#[test]
fn d6_rea_node_22_kabul_edilir() {
    // REA 6.7.0'ın kendi engines alanı: ^22.19.0 || ^24.11.0 || >=26.0.0
    let o = Ortam::yeni("node22");
    o.elle_kur();
    let n22 = o.kok.join("node22");
    let govde = std::fs::read_to_string(o.kok.join("rea/node")).unwrap().replace("v24.22.0", "v22.23.3");
    std::fs::write(&n22, govde).unwrap();
    #[cfg(unix)]
    calistirilabilir(&n22);
    let (kod, d) = o.calis(&["rea", "durum"], &[("SEZGI_NODE", n22.to_str().unwrap())]);
    assert_eq!(kod, Some(0), "{d}");
    assert!(d.contains("hazır"), "{d}");
}

#[test]
fn d6_rea_durum_kurulu_degilken_kur_onerir() {
    let o = Ortam::yeni("durum-yok");
    let (kod, m) = o.calis(&["rea", "durum"], &[]);
    assert_eq!(kod, Some(1), "{m}");
    assert!(m.contains("kurulu değil") && m.contains("sezgi rea kur"), "{m}");
    let (_, j) = o.calis(&["rea", "durum", "--json"], &[]);
    let j: Value = serde_json::from_str(&j).unwrap_or_else(|e| panic!("{e}: {j}"));
    assert_eq!(j["durum"], "yok", "{j}");
    assert_eq!(j["onek_kurulu"], false);
}

#[test]
fn d6_rea_sezgi_rea_degiskeni_onekten_once_gelir() {
    let o = Ortam::yeni("degisken");
    o.elle_kur();
    let baska = o.kok.join("rea/paket/node_modules/.bin/rea");
    let (kod, cikti) = o.calis(&["rea", "durum", "--json"], &[("SEZGI_REA", baska.to_str().unwrap())]);
    assert_eq!(kod, Some(0), "{cikti}");
    // Sezgi'nin kurduğu klasörün dışındaki REA stderr'de uyarılır (içeriği doğrulanmadı); JSON ondan ayrıdır
    let (uyari, json): (Vec<&str>, Vec<&str>) = cikti.lines().partition(|l| l.starts_with("! REA"));
    assert!(uyari.iter().any(|l| l.contains("SEZGI_REA") && l.contains("içeriği doğrulanmadı")), "{cikti}");
    let j = json.join("\n");
    let j: Value = serde_json::from_str(&j).unwrap_or_else(|e| panic!("{e}: {j}"));
    assert_eq!(j["kaynak"], "SEZGI_REA", "{j}");
    assert_eq!(j["onek_kurulu"], true);
}

#[test]
fn d6_rea_doktor_ve_kesfet_rea_yoksa_kur_onerir() {
    let o = Ortam::yeni("oneri");
    let (_, d) = o.calis(&["doktor", "--rea"], &[]);
    assert!(d.contains("sezgi rea kur"), "{d}");
    let y = o.kok.join("yayin");
    kopyala(&ornekler().join("w2k_otelrez/yayin"), &y);
    let (kod, k) = o.calis(&["kesfet", y.to_str().unwrap(), "--rea"], &[]);
    assert_eq!(kod, Some(0), "{k}");
    assert!(k.contains("sezgi rea kur"), "{k}");
    // --rea verilmediyse de yol gösterilir
    let (_, k) = o.calis(&["kesfet", y.to_str().unwrap()], &[]);
    assert!(k.contains("sezgi rea kur"), "{k}");
}

#[test]
fn d6_rea_kesfet_onekteki_reayi_kullanir() {
    let o = Ortam::yeni("kesfet");
    o.elle_kur();
    let y = o.kok.join("yayin");
    kopyala(&ornekler().join("w2k_otelrez/yayin"), &y);
    let (kod, k) = o.calis(&["kesfet", y.to_str().unwrap(), "--rea"], &[]);
    assert_eq!(kod, Some(0), "{k}");
    let b: Value = serde_json::from_str(&std::fs::read_to_string(y.join(".sezgi/kaynaksiz.json")).unwrap()).unwrap();
    assert_eq!(b["rea"]["durum"], "acik", "{k}");
    assert!(b["rea"]["paket"].as_str().unwrap().starts_with(&o.onek().display().to_string()), "{}", b["rea"]);
}

#[test]
fn d6_rea_kaldir_onay_ister_ve_yalnizca_kendi_onekini_siler() {
    let o = Ortam::yeni("kaldir");
    o.elle_kur();
    let (kod, m) = o.calis(&["rea", "kaldir"], &[]);
    assert_eq!(kod, Some(2), "{m}");
    assert!(o.onek().exists(), "onaysız silinmemeli");
    let (kod, m) = o.calis(&["rea", "kaldir", "--evet"], &[]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(!o.onek().exists(), "{m}");
    let (kod, m) = o.calis(&["rea", "kaldir", "--evet"], &[]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(m.contains("kurulu değil"), "{m}");
    // Sezgi'nin kurmadığı bir klasör silinmez
    std::fs::create_dir_all(o.onek()).unwrap();
    std::fs::write(o.onek().join("package.json"), r#"{"name":"baskasi"}"#).unwrap();
    let (kod, m) = o.calis(&["rea", "kaldir", "--evet"], &[]);
    assert_eq!(kod, Some(1), "{m}");
    assert!(o.onek().join("package.json").exists());
}

#[test]
fn d6_rea_yardim_ve_alt_komutlar() {
    let o = Ortam::yeni("yardim");
    let (kod, m) = o.calis(&["yardim", "rea"], &[]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(m.contains("sezgi rea kur") && m.contains("durum") && m.contains("kaldir"), "{m}");
    let (kod, m) = o.calis(&["rea", "yukle"], &[]);
    assert_eq!(kod, Some(2), "{m}");
    let (kod, m) = o.calis(&["rea"], &[]);
    assert_eq!(kod, Some(2), "alt komut zorunlu: {m}");
    // İngilizce: install / status / uninstall
    let (kod, m) = o.calis(&["rea", "status"], &[]);
    assert_eq!(kod, Some(1), "{m}");
    assert!(m.contains("kurulu değil"), "{m}");
    let (kod, m) = o.calis(&["rea", "kur", "--bilinmeyen"], &[]);
    assert_eq!(kod, Some(2), "{m}");
}

#[test]
fn d6_rea_kilit_dosyasi_sabitlenen_paketle_ayni() {
    let kilit: Value = serde_json::from_str(&std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/kesif/rea_kilit.json")).unwrap()).unwrap();
    assert_eq!(kilit["lockfileVersion"], 3);
    assert_eq!(kilit["packages"][""]["dependencies"]["rea-agents"], "6.7.0");
    let rea = &kilit["packages"]["node_modules/rea-agents"];
    assert_eq!((rea["version"].as_str(), rea["integrity"].as_str()), (Some("6.7.0"), Some(BUTUNLUK)));
    let paketler = kilit["packages"].as_object().unwrap();
    assert!(paketler.len() > 100, "bütün bağımlılık ağacı sabit olmalı");
    for (ad, p) in paketler.iter().filter(|(ad, _)| !ad.is_empty()) {
        assert!(p["integrity"].as_str().is_some_and(|i| i.starts_with("sha512-")), "{ad}: sha512 bütünlüğü yok");
        assert!(p["resolved"].as_str().is_some_and(|r| r.starts_with("https://registry.npmjs.org/")), "{ad}: kaynak npm kaydı değil");
    }
}
