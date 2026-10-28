//! `sezgi add mcp` ve `sezgi kaldir`: izinli bağlantı, kullanıcı ayarlarının korunması ve iz bırakmadan geri alma.
//! Her test kendi sahte kullanıcı klasörüyle (HOME / USERPROFILE / APPDATA) çalışır; gerçek ayarlara dokunulmaz.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Ortam {
    kok: PathBuf,
    ev: PathBuf,
    proje: PathBuf,
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-baglan-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let ev = kok.join("ev");
        let proje = kok.join("uygulama");
        std::fs::create_dir_all(ev.join("ayar")).unwrap();
        std::fs::create_dir_all(proje.join(".sezgi")).unwrap();
        std::fs::write(proje.join(".sezgi").join("kapilar.json"), "{}").unwrap();
        std::fs::write(proje.join("package.json"), "{ \"name\": \"uygulama\" }\n").unwrap();
        Ortam { kok, ev, proje }
    }

    fn calistir(&self, argumanlar: &[&str]) -> Output {
        let bos_path = self.kok.join("path");
        std::fs::create_dir_all(&bos_path).unwrap();
        Command::new(env!("CARGO_BIN_EXE_sezgi"))
            .args(argumanlar)
            .current_dir(&self.proje)
            .env("HOME", &self.ev)
            .env("USERPROFILE", &self.ev)
            .env("APPDATA", self.ev.join("ayar"))
            .env("XDG_CONFIG_HOME", self.ev.join("ayar"))
            .env("PATH", &bos_path)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn defter(&self) -> PathBuf {
        self.ev.join("ayar").join("sezgi").join("eklemeler.json")
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

/// Klasördeki bütün dosyalar ve içerikleri (klasörler de: boş klasör kalırsa fark görünsün)
fn goruntu(k: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut m = BTreeMap::new();
    fn gez(k: &Path, taban: &Path, m: &mut BTreeMap<String, Vec<u8>>) {
        for g in std::fs::read_dir(k).unwrap() {
            let p = g.unwrap().path();
            let ad = p.strip_prefix(taban).unwrap().to_string_lossy().replace('\\', "/");
            if p.is_dir() {
                m.insert(format!("{ad}/"), vec![]);
                gez(&p, taban, m);
            } else {
                m.insert(ad, std::fs::read(&p).unwrap());
            }
        }
    }
    gez(k, k, &mut m);
    m
}

fn codex(o: &Ortam) -> PathBuf {
    o.ev.join(".codex").join("config.toml")
}

const KULLANICI_MCP: &str = "{\n    \"mcpServers\": {\n        \"baska\": {\n            \"command\": \"baska-sunucu\"\n        }\n    }\n}\n";
const KULLANICI_CODEX: &str = "model = \"o4\"\n\n[mcp_servers.baska]\ncommand = \"baska\"\n";

#[test]
fn secim_yoksa_hicbir_sey_degismez() {
    let o = Ortam::yeni("secimsiz");
    std::fs::create_dir_all(o.ev.join(".codex")).unwrap();
    let (once_proje, once_ev) = (goruntu(&o.proje), goruntu(&o.ev));
    let c = o.calistir(&["add", "mcp"]);
    let m = metin(&c);
    assert!(c.status.success(), "{m}");
    assert!(m.contains("Neden"), "gerekçe söylenmeli: {m}");
    assert!(m.contains("Codex") && m.contains("bütün projelerinizi etkiler"), "bulunan genel araç uyarıyla gösterilmeli: {m}");
    assert!(m.contains("hiçbir dosya değişmedi"), "{m}");
    assert_eq!(goruntu(&o.proje), once_proje);
    assert_eq!(goruntu(&o.ev), once_ev);
}

#[test]
fn ekler_kullanici_ayarlarini_korur_ve_kaldir_birebir_geri_getirir() {
    let o = Ortam::yeni("tam");
    std::fs::write(o.proje.join(".mcp.json"), KULLANICI_MCP).unwrap();
    std::fs::create_dir_all(o.ev.join(".codex")).unwrap();
    std::fs::write(codex(&o), KULLANICI_CODEX).unwrap();
    let (once_proje, once_ev) = (goruntu(&o.proje), goruntu(&o.ev));

    let c = o.calistir(&["add", "mcp", "--arac", "claude-code,cursor,codex"]);
    assert!(c.status.success(), "{}", metin(&c));

    // Kullanıcının girdisi ve girintisi (4 boşluk) korunur, Sezgi'ninki eklenir
    let mcp = std::fs::read_to_string(o.proje.join(".mcp.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&mcp).unwrap();
    assert_eq!(v["mcpServers"]["baska"]["command"], "baska-sunucu");
    assert_eq!(v["mcpServers"]["sezgi"]["args"][0], "mcp");
    assert!(v["mcpServers"]["sezgi"]["args"][1].as_str().unwrap().ends_with(".sezgi"));
    assert!(mcp.contains("\n    \"mcpServers\""), "girinti korunmalı:\n{mcp}");
    assert!(o.proje.join(".cursor").join("mcp.json").is_file());
    // Codex: kullanıcı metni aynen başta, Sezgi bloğu sonda; genel dosyada ad projeye özgü
    let t = std::fs::read_to_string(codex(&o)).unwrap();
    assert!(t.starts_with(KULLANICI_CODEX), "{t}");
    let tablo: toml::Table = t.parse().unwrap();
    assert_eq!(tablo["mcp_servers"]["sezgi-uygulama"]["args"][0].as_str(), Some("mcp"));
    assert_eq!(tablo["model"].as_str(), Some("o4"));
    // Kayıt proje dışında
    assert!(o.defter().is_file());
    assert!(!o.proje.join("eklemeler.json").exists());

    // İkinci çalıştırma: zaten ekli, dosyalar değişmez
    let ara = goruntu(&o.proje);
    let c = o.calistir(&["add", "mcp", "--arac", "claude-code,codex"]);
    assert!(c.status.success() && metin(&c).contains("zaten"), "{}", metin(&c));
    assert_eq!(goruntu(&o.proje), ara);

    let c = o.calistir(&["kaldir", "--evet"]);
    assert!(c.status.success(), "{}", metin(&c));
    assert_eq!(goruntu(&o.proje), once_proje, "proje kurulum öncesiyle birebir aynı olmalı");
    let mut sonra_ev = goruntu(&o.ev);
    // Sezgi'nin kendi yapılandırma klasörü (boş kayıt, yedek klasörü) kalabilir; kullanıcının dosyaları birebir aynı olmalı
    sonra_ev.retain(|k, _| !k.starts_with("ayar/sezgi"));
    assert_eq!(sonra_ev, once_ev);
}

#[test]
fn ayni_adli_kullanici_girdisine_dokunmaz() {
    let o = Ortam::yeni("cakisma");
    let kendi = "{\n  \"mcpServers\": {\n    \"sezgi\": { \"command\": \"benim-sezgim\" }\n  }\n}\n";
    std::fs::write(o.proje.join(".mcp.json"), kendi).unwrap();
    let c = o.calistir(&["add", "mcp", "--arac", "claude-code"]);
    assert!(metin(&c).contains("dokunulmadı"), "{}", metin(&c));
    assert_eq!(std::fs::read_to_string(o.proje.join(".mcp.json")).unwrap(), kendi);
}

#[test]
fn yorumlu_json_dosyasina_yazmaz() {
    let o = Ortam::yeni("jsonc");
    std::fs::create_dir_all(o.proje.join(".vscode")).unwrap();
    let jsonc = "{\n  // kullanıcının notu\n  \"servers\": {}\n}\n";
    std::fs::write(o.proje.join(".vscode").join("mcp.json"), jsonc).unwrap();
    let c = o.calistir(&["add", "mcp", "--arac", "vscode"]);
    assert!(metin(&c).contains("elle"), "{}", metin(&c));
    assert_eq!(std::fs::read_to_string(o.proje.join(".vscode").join("mcp.json")).unwrap(), jsonc);
}

#[test]
fn kaldir_sonradan_degisen_dosyada_yalnizca_kendi_girdisini_siler() {
    let o = Ortam::yeni("degisen");
    std::fs::write(o.proje.join(".mcp.json"), KULLANICI_MCP).unwrap();
    assert!(o.calistir(&["add", "mcp", "--arac", "claude-code"]).status.success());
    // Kullanıcı ekledikten sonra kendi sunucusunu ekliyor
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(o.proje.join(".mcp.json")).unwrap()).unwrap();
    v["mcpServers"]["yeni"] = serde_json::json!({ "command": "yeni-sunucu" });
    std::fs::write(o.proje.join(".mcp.json"), serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let c = o.calistir(&["kaldir", "--evet"]);
    assert!(c.status.success(), "{}", metin(&c));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(o.proje.join(".mcp.json")).unwrap()).unwrap();
    assert!(v["mcpServers"].get("sezgi").is_none(), "Sezgi girdisi silinmeli");
    assert_eq!(v["mcpServers"]["yeni"]["command"], "yeni-sunucu", "kullanıcının sonradan eklediği kalmalı");
    assert_eq!(v["mcpServers"]["baska"]["command"], "baska-sunucu");
}

#[test]
fn yonlendirme_ayri_istenir_ve_geri_alinir() {
    let o = Ortam::yeni("yonlendir");
    std::fs::write(o.proje.join("AGENTS.md"), "# Kendi talimatlarım\n").unwrap();
    let once = goruntu(&o.proje);
    assert!(o.calistir(&["add", "mcp", "--arac", "cursor"]).status.success());
    assert_eq!(std::fs::read_to_string(o.proje.join("AGENTS.md")).unwrap(), "# Kendi talimatlarım\n", "--yonlendir olmadan talimat dosyasına dokunulmaz");
    assert!(o.calistir(&["add", "mcp", "--arac", "cursor", "--yonlendir"]).status.success());
    let t = std::fs::read_to_string(o.proje.join("AGENTS.md")).unwrap();
    assert!(t.starts_with("# Kendi talimatlarım\n") && t.contains("Sezgi"), "{t}");
    assert!(o.calistir(&["kaldir", "--evet"]).status.success());
    assert_eq!(goruntu(&o.proje), once);
}

#[test]
fn deneme_hicbir_sey_yazmaz() {
    let o = Ortam::yeni("deneme");
    let (once_proje, once_ev) = (goruntu(&o.proje), goruntu(&o.ev));
    let c = o.calistir(&["add", "mcp", "--arac", "claude-code,codex", "--yonlendir", "--deneme"]);
    assert!(c.status.success() && metin(&c).contains(".mcp.json"), "{}", metin(&c));
    assert_eq!(goruntu(&o.proje), once_proje);
    assert_eq!(goruntu(&o.ev), once_ev);
}

#[test]
fn bilinmeyen_arac_ve_haritasiz_proje_reddedilir() {
    let o = Ortam::yeni("ret");
    let c = o.calistir(&["add", "mcp", "--arac", "yok-boyle-arac"]);
    assert!(!c.status.success() && metin(&c).contains("yok-boyle-arac"), "{}", metin(&c));
    std::fs::remove_dir_all(o.proje.join(".sezgi")).unwrap();
    let c = o.calistir(&["add", "mcp", "--arac", "claude-code"]);
    assert!(!c.status.success() && metin(&c).contains("sezgi init"), "{}", metin(&c));
    assert!(!o.proje.join(".mcp.json").exists());
}

#[test]
fn kaldir_haritayi_yalnizca_istenirse_siler() {
    let o = Ortam::yeni("harita");
    std::fs::write(o.proje.join(".sezgi").join("benlik.json"), "{}").unwrap();
    let c = o.calistir(&["kaldir"]);
    assert!(c.status.success(), "{}", metin(&c));
    assert!(o.proje.join(".sezgi").join("kapilar.json").exists(), "sorulmadan harita silinmez");
    // Haritada kullanıcının bıraktığı bilinmeyen bir dosya varsa o dosya ve klasör kalır
    std::fs::write(o.proje.join(".sezgi").join("notum.txt"), "x").unwrap();
    let c = o.calistir(&["kaldir", "--harita-sil"]);
    assert!(c.status.success(), "{}", metin(&c));
    assert!(!o.proje.join(".sezgi").join("kapilar.json").exists());
    assert!(!o.proje.join(".sezgi").join("benlik.json").exists());
    assert!(o.proje.join(".sezgi").join("notum.txt").exists());
    std::fs::remove_file(o.proje.join(".sezgi").join("notum.txt")).unwrap();
    std::fs::write(o.proje.join(".sezgi").join("kapilar.json"), "{}").unwrap();
    assert!(o.calistir(&["kaldir", "--harita-sil"]).status.success());
    assert!(!o.proje.join(".sezgi").exists());
}
