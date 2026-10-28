//! `sezgi doktor`: neyin eksik ya da bozuk olduğunu söyler, nasıl düzeltileceğini yazar; hiçbir şeyi değiştirmez.
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Ortam {
    kok: PathBuf,
    ev: PathBuf,
    proje: PathBuf,
}

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap() {
        let p = g.unwrap().path();
        let h = hedef.join(p.file_name().unwrap());
        if p.is_dir() {
            kopyala(&p, &h);
        } else {
            std::fs::copy(&p, &h).unwrap();
        }
    }
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-doktor-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let ev = kok.join("ev");
        let proje = kok.join("uygulama");
        std::fs::create_dir_all(ev.join("ayar")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &proje);
        Ortam { kok, ev, proje }
    }

    fn calistir(&self, argumanlar: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sezgi"))
            .args(argumanlar)
            .current_dir(&self.proje)
            .env("HOME", &self.ev)
            .env("USERPROFILE", &self.ev)
            .env("APPDATA", self.ev.join("ayar"))
            .env("XDG_CONFIG_HOME", self.ev.join("ayar"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

fn metin(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn kontroller(o: &Ortam) -> Vec<serde_json::Value> {
    let c = o.calistir(&["doktor", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&c.stdout).unwrap_or_else(|e| panic!("{e}: {}", metin(&c)));
    v["kontroller"].as_array().unwrap().clone()
}

fn durum(k: &[serde_json::Value], ad: &str) -> String {
    k.iter().find(|x| x["ad"] == ad).unwrap_or_else(|| panic!("'{ad}' kontrolü yok: {k:?}"))["durum"].as_str().unwrap().to_string()
}

#[test]
fn saglikli_uygulama() {
    let o = Ortam::yeni("saglikli");
    assert!(o.calistir(&["init"]).status.success());
    let c = o.calistir(&["doktor"]);
    assert!(c.status.success(), "{}", metin(&c));
    let k = kontroller(&o);
    for ad in ["uygulama", "cati", "harita", "tazelik", "kart"] {
        assert_eq!(durum(&k, ad), "tamam", "{ad}: {k:?}");
    }
}

#[test]
fn haritasiz_uygulama_init_onerir() {
    let o = Ortam::yeni("haritasiz");
    let c = o.calistir(&["doktor"]);
    assert!(!c.status.success());
    assert!(metin(&c).contains("sezgi init"), "{}", metin(&c));
    assert_eq!(durum(&kontroller(&o), "harita"), "hata");
}

#[test]
fn bayat_harita_uyarilir() {
    let o = Ortam::yeni("bayat");
    assert!(o.calistir(&["init"]).status.success());
    let ts = walk(&o.proje.join("src")).into_iter().find(|p| p.extension().is_some_and(|e| e == "ts")).unwrap();
    std::fs::write(&ts, format!("{}\n// değişiklik\n", std::fs::read_to_string(&ts).unwrap())).unwrap();
    let c = o.calistir(&["doktor"]);
    assert!(c.status.success(), "bayatlık uyarıdır, hata değil: {}", metin(&c));
    assert!(metin(&c).contains("1 dosya"), "{}", metin(&c));
    assert_eq!(durum(&kontroller(&o), "tazelik"), "uyari");
}

fn walk(k: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    for g in std::fs::read_dir(k).unwrap() {
        let p = g.unwrap().path();
        if p.is_dir() {
            v.extend(walk(&p));
        } else {
            v.push(p);
        }
    }
    v
}

#[test]
fn nestjs_olmayan_uygulama() {
    let o = Ortam::yeni("nestjssiz");
    std::fs::write(o.proje.join("package.json"), "{ \"name\": \"x\", \"dependencies\": { \"koa\": \"2\" } }").unwrap();
    let c = o.calistir(&["doktor"]);
    assert!(!c.status.success());
    assert!(metin(&c).contains("@nestjs/core"), "{}", metin(&c));
    assert_eq!(durum(&kontroller(&o), "cati"), "hata");
}

#[test]
fn bozulan_baglanti_bulunur() {
    let o = Ortam::yeni("baglanti");
    assert!(o.calistir(&["init"]).status.success());
    assert!(o.calistir(&["add", "mcp", "--arac", "claude-code"]).status.success());
    assert_eq!(durum(&kontroller(&o), "baglanti:claude-code"), "tamam");
    // İkili taşınmış gibi: bağlantıdaki komut artık yok
    let yol = o.proje.join(".mcp.json");
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
    v["mcpServers"]["sezgi"]["command"] = serde_json::json!(o.kok.join("yok").join("sezgi.exe").to_string_lossy());
    std::fs::write(&yol, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    let c = o.calistir(&["doktor"]);
    assert!(!c.status.success());
    assert!(metin(&c).contains("sezgi bagla"), "düzeltme yolu söylenmeli: {}", metin(&c));
    assert_eq!(durum(&kontroller(&o), "baglanti:claude-code"), "hata");
    // Girdi elle silinmiş
    v["mcpServers"].as_object_mut().unwrap().remove("sezgi");
    std::fs::write(&yol, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    assert_eq!(durum(&kontroller(&o), "baglanti:claude-code"), "uyari");
}

#[test]
fn doktor_hicbir_sey_degistirmez() {
    let o = Ortam::yeni("salt-okur");
    assert!(o.calistir(&["init"]).status.success());
    let once: Vec<(PathBuf, Vec<u8>)> = walk(&o.kok).into_iter().map(|p| (p.clone(), std::fs::read(&p).unwrap())).collect();
    o.calistir(&["doktor"]);
    o.calistir(&["doktor", "--json"]);
    let sonra: Vec<(PathBuf, Vec<u8>)> = walk(&o.kok).into_iter().map(|p| (p.clone(), std::fs::read(&p).unwrap())).collect();
    assert_eq!(once, sonra);
}

#[test]
fn imzali_kart_dogrulanir_degistirilen_kart_hata() {
    let o = Ortam::yeni("imza");
    assert!(o.calistir(&["anahtar", "olustur", "--ad", "test"]).status.success());
    assert!(o.calistir(&["init", "--imzala", "test"]).status.success());
    let k = kontroller(&o);
    assert_eq!(durum(&k, "kart"), "tamam", "{k:?}");
    let kart_yolu = o.proje.join(".sezgi").join("benlik.json");
    let mut kart: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&kart_yolu).unwrap()).unwrap();
    kart["app"]["name"] = serde_json::json!("baskasi");
    std::fs::write(&kart_yolu, serde_json::to_string_pretty(&kart).unwrap()).unwrap();
    assert_eq!(durum(&kontroller(&o), "kart"), "hata");
    assert!(!o.calistir(&["doktor"]).status.success());
}
