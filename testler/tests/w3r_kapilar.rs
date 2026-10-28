//! W3 REA geniş: sahte REA (betik) ile izin, onay ve yalıtım kapıları. Sahte kurulum gerçek etki tablosunu taşır
//! (sabitleme tutar); `node` betiği çağrılan her REA komutunu kaydeder ve hazır çıktıları döner. Denetlenen:
//! - onaysız onaylı/dinamik komut asla çalışmaz (onay bayrağı, yalıtım beyanı, terminal "evet", dinamikte --calistir ve
//!   --sahibiyim); terminal olmadan onay verilemez;
//! - native analiz motor yoksa çalışmaz ve "motor kurulu değil" der; ticari motor çıktısı karta girmez;
//! - WebMCP yalnızca loopback CDP'ye; sayfanın açıklaması saklanmaz; yalıtımsız gözlem inferred;
//! - firmware motor yoksa çalışmaz; süreç gözlemi RUNTIME_TRACE (yalıtım yoksa inferred);
//! - `--yetenekler` sınıfları sabitlenen tablodan verir.
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler/w3r_sahte_rea")
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

struct Ortam {
    kok: PathBuf,
}

impl Ortam {
    /// `doktor`: doctor.<ad>.json (motorsuz, ghidra, ida); `inspect`: inspect.<ad>.json
    fn yeni(ad: &str, doktor: &str, inspect: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-w3r-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        kopyala(&ornek(), &kok.join("rea"));
        let c = kok.join("rea/cevap");
        std::fs::copy(c.join(format!("doctor.{doktor}.json")), c.join("doctor.json")).unwrap();
        std::fs::copy(c.join(format!("inspect.{inspect}.json")), c.join("inspect.json")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(kok.join("rea/node"), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::create_dir_all(kok.join("ayar")).unwrap();
        Ortam { kok }
    }

    fn ortam(&self, c: &mut Command) {
        c.env("SEZGI_REA", self.kok.join("rea/paket/node_modules/.bin/rea"))
            .env("SEZGI_NODE", self.kok.join("rea/node"))
            .env("W3R_IZ", self.kok.join("iz.txt"))
            .env("W3R_CEVAP", self.kok.join("rea/cevap"))
            .env("PATH", "/usr/bin:/bin")
            .env("APPDATA", self.kok.join("ayar"))
            .env("XDG_CONFIG_HOME", self.kok.join("ayar"))
            .env("HOME", self.kok.join("ayar"))
            .env_remove("REA_BINWALK_COMMAND");
    }

    /// Terminalsiz çalıştırma: (kod, stdout + stderr)
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

    /// Terminalde (sözde terminal, `script`) çalıştırma; `cevap` terminale yazılır
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
        // Soru gelene kadar beklemeden yazılır: terminal satırı tamponlar
        let _ = g.write_all(cevap.as_bytes());
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = g.write_all(b"\x04");
        let o = p.wait_with_output().unwrap();
        drop(g);
        (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    }

    /// Sahte REA'ya iletilen komutlar
    fn cagrilar(&self) -> Vec<String> {
        std::fs::read_to_string(self.kok.join("iz.txt")).unwrap_or_default().lines().map(String::from).collect()
    }

    fn cagrildi_mi(&self, komut: &str) -> bool {
        self.cagrilar().iter().any(|l| l.split_whitespace().next() == Some(komut))
    }

    fn iz_sil(&self) {
        let _ = std::fs::remove_file(self.kok.join("iz.txt"));
    }

    /// Çalıştırılabilir bir ELF (test görüntüsündeki /bin/true) içeren yapıt
    fn elf(&self) -> PathBuf {
        let y = self.kok.join("yapit");
        std::fs::create_dir_all(&y).unwrap();
        std::fs::copy("/bin/true", y.join("hedef")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(y.join("hedef"), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        y.join("hedef")
    }

    fn belge(&self, cikti: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(cikti.join("kaynaksiz.json")).unwrap()).unwrap()
    }
}

fn bilinmeyen_var(b: &Value, parca: &str) -> bool {
    b["bilinmeyenler"].as_array().unwrap().iter().any(|x| x["neden"].as_str().unwrap_or("").contains(parca))
}

fn kanit<'a>(b: &'a Value, son: &str) -> Option<&'a Value> {
    b["kanitlar"].as_array().unwrap().iter().find(|k| k["konum"][0].as_str().unwrap_or("").ends_with(son))
}

/// Çıktının JSON kısmı: Sezgi'nin kurduğu klasörün dışındaki REA için stderr'e yazılan uyarı satırları (`! REA ...`) ayıklanır
fn json_kismi(m: &str) -> String {
    m.lines().filter(|l| !l.starts_with("! REA")).collect::<Vec<_>>().join("\n")
}

#[test]
fn w3r_yetenekler_siniflari_tablodan() {
    let o = Ortam::yeni("yetenek", "ghidra", "ghidra");
    let (kod, m) = o.calis(&["kesfet", "--yetenekler", "--json"], &[]);
    assert_eq!(kod, Some(0), "{m}");
    let v: Value = serde_json::from_str(&json_kismi(&m)).unwrap();
    assert_eq!(v["rea"]["durum"], "hazir", "{m}");
    let sinif = |k: &str| v["komutlar"].as_array().unwrap().iter().find(|x| x["komut"] == k).unwrap()["sinif"].as_str().unwrap().to_string();
    assert_eq!(sinif("inspect-managed-members"), "otomatik");
    assert_eq!(sinif("discover-webmcp-tools"), "onaylı");
    assert_eq!(sinif("list-browser-targets"), "onaylı");
    assert_eq!(sinif("inspect"), "onaylı", "motor oturumu süreç başlatır");
    assert_eq!(sinif("inspect-firmware-regions"), "onaylı");
    assert_eq!(sinif("capture-process"), "dinamik");
    assert!(v["motorlar"].as_array().unwrap().iter().any(|m| m["ad"] == "ghidra" && m["kurulu"] == true));
    assert!(v["motorlar"].as_array().unwrap().iter().any(|m| m["ad"] == "ida" && m["ticari"] == true));
    let say = &v["butun_komutlar"];
    assert!(say["otomatik"].as_u64().unwrap() > 0 && say["onaylı"].as_u64().unwrap() > 0 && say["dinamik"].as_u64().unwrap() > 0, "{say}");
    // Yalnızca tanılama komutları çağrıldı (sürüm, yetenekler, doctor)
    for c in o.cagrilar() {
        assert!(matches!(c.split_whitespace().next(), Some("--version" | "capabilities" | "doctor")), "beklenmeyen çağrı: {c}");
    }
    let (_, m) = o.calis(&["kesfet", "--yetenekler"], &[]);
    assert!(m.contains("capture-process") && m.contains("dinamik") && m.contains("--sahibiyim"), "{m}");
}

/// Dinamik komut (süreç gözlemi) hiçbir eksik şartla çalışmaz; hepsiyle çalışır, yalıtım yoksa inferred
#[test]
fn w3r_onaysiz_dinamik_asla_calismaz() {
    let o = Ortam::yeni("dinamik", "motorsuz", "ghidra");
    let hedef = o.elf();
    let h = hedef.to_str().unwrap();
    let c = o.kok.join("cikti");
    let cs = c.to_str().unwrap();
    let durumlar: [(&[&str], Option<&str>, &str); 5] = [
        (&["--calistir"], None, "onay ister"),
        (&["--calistir", "--onayla"], None, "yalıtım beyanı yok"),
        (&["--calistir", "--onayla", "--yalitim", "konteyner"], Some("evet\n"), "--sahibiyim"),
        (&["--calistir", "--onayla", "--yalitim", "konteyner", "--sahibiyim"], None, "terminal"),
        (&["--calistir", "--onayla", "--yalitim", "konteyner", "--sahibiyim"], Some("hayir\n"), "onaylamadı"),
    ];
    for (bayrak, cevap, neden) in durumlar {
        o.iz_sil();
        let mut a = vec!["kesfet", h, "--cikti", cs];
        a.extend_from_slice(bayrak);
        let (kod, m) = match cevap {
            Some(c) => o.terminalde(&a, c, &[]),
            None => o.calis(&a, &[]),
        };
        assert_eq!(kod, Some(0), "{bayrak:?}: {m}");
        assert!(!o.cagrildi_mi("capture-process"), "{bayrak:?}: onaysız süreç gözlemi çalıştı: {:?}", o.cagrilar());
        let b = o.belge(&c);
        assert!(bilinmeyen_var(&b, neden), "{bayrak:?}: '{neden}' bekleniyordu: {}", b["bilinmeyenler"]);
        assert!(kanit(&b, "#capture_process").is_none());
    }
    // Bütün şartlar ve terminalde "evet": çalışır; yalıtım beyanı konteyner → observed, yok → inferred
    for (yalitim, guven) in [("konteyner", "observed"), ("yok", "inferred")] {
        o.iz_sil();
        let (kod, m) = o.terminalde(&["kesfet", h, "--cikti", cs, "--calistir", "--onayla", "--yalitim", yalitim, "--sahibiyim"], "evet\n", &[]);
        assert_eq!(kod, Some(0), "{m}");
        assert!(m.contains("dinamik") && m.contains("evet"), "insan soruyu görür: {m}");
        assert!(o.cagrildi_mi("capture-process"), "{:?}", o.cagrilar());
        let b = o.belge(&c);
        let k = kanit(&b, "#capture_process").unwrap_or_else(|| panic!("{}", b["bilinmeyenler"]));
        assert_eq!((k["kaynak_turu"].as_str(), k["beyan"].as_str(), k["yalitim"].as_str()), (Some("RUNTIME_TRACE"), Some(guven), Some(yalitim)));
        assert!(k["ozet"]["terminal_cerceve"].as_u64().unwrap() >= 1);
        let metin = std::fs::read_to_string(c.join("kaynaksiz.json")).unwrap();
        assert!(!metin.contains("merhaba"), "terminal çıktısı saklanmamalı");
    }
}

/// Native: motor yoksa çalışmaz ("motor kurulu değil"); motor varsa yalnızca onayla; kod ve dize değeri saklanmaz
#[test]
fn w3r_native_motor_ve_onay() {
    let o = Ortam::yeni("native-yok", "motorsuz", "ghidra");
    let hedef = o.elf();
    let c = o.kok.join("cikti");
    let (kod, m) = o.terminalde(&["kesfet", hedef.to_str().unwrap(), "--cikti", c.to_str().unwrap(), "--rea", "--onayla", "--yalitim", "konteyner"], "evet\n", &[]);
    assert_eq!(kod, Some(0), "{m}");
    let b = o.belge(&c);
    assert!(bilinmeyen_var(&b, "motor kurulu değil"), "{}", b["bilinmeyenler"]);
    assert!(!o.cagrildi_mi("inspect") && !o.cagrildi_mi("decompile"), "{:?}", o.cagrilar());
    assert_eq!(kanit(&b, "hedef").unwrap()["ozet"]["bicim"], "elf", "ELF kimliği yerel okunur");

    let o = Ortam::yeni("native", "ghidra", "ghidra");
    let hedef = o.elf();
    let c = o.kok.join("cikti");
    // Onaysız: motor olsa da çalışmaz
    let (kod, _) = o.calis(&["kesfet", hedef.to_str().unwrap(), "--cikti", c.to_str().unwrap(), "--rea"], &[]);
    assert_eq!(kod, Some(0));
    assert!(!o.cagrildi_mi("inspect"), "{:?}", o.cagrilar());
    assert!(bilinmeyen_var(&o.belge(&c), "onay ister"));
    // Onayla
    let (kod, m) = o.terminalde(&["kesfet", hedef.to_str().unwrap(), "--cikti", c.to_str().unwrap(), "--rea", "--onayla", "--yalitim", "vm"], "evet\n", &[]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(o.cagrildi_mi("inspect") && o.cagrildi_mi("decompile"), "{:?}", o.cagrilar());
    let b = o.belge(&c);
    let gb = kanit(&b, "#inspect").unwrap();
    assert_eq!((gb["kaynak_turu"].as_str(), gb["beyan"].as_str()), (Some("BINARY_METADATA"), Some("observed")));
    assert_eq!(gb["ozet"]["strings"]["sayi"], 2, "dizeler yalnızca sayılır");
    let dk = kanit(&b, "#decompile:main").unwrap();
    assert_eq!((dk["kaynak_turu"].as_str(), dk["beyan"].as_str()), (Some("DECOMPILED"), Some("inferred")));
    assert_eq!(dk["ozet"]["satir"], 3);
    for d in ["cikti/kaynaksiz.json", "cikti/kanit.json", "cikti/benlik.json", "cikti/benlik-evidence.json"] {
        let metin = std::fs::read_to_string(o.kok.join(d)).unwrap();
        for gizli in ["GIZLI_KOD_PARCASI", "GIZLI_DIZE_DEGERI"] {
            assert!(!metin.contains(gizli), "{d}: {gizli} saklanmamalı");
        }
    }
    let graf: Value = serde_json::from_str(&std::fs::read_to_string(c.join("graf.json")).unwrap()).unwrap();
    let kenar = graf["kenarlar"].as_array().unwrap().iter().any(|k| k["kaynak"] == "fn:hedef#main" && k["iliski"] == "CALLS" && k["hedef"] == "fn:hedef#yardimci");
    assert!(kenar, "{}", graf["kenarlar"]);
}

/// Ticari motor (IDA) çıktısı paylaşılamaz: yerel kanıt deposunda var, kartta ve kanıt paketinde yok
#[test]
fn w3r_ticari_cikti_karta_girmez() {
    let o = Ortam::yeni("ticari", "ida", "ida");
    let hedef = o.elf();
    let c = o.kok.join("cikti");
    let (kod, m) = o.terminalde(&["kesfet", hedef.to_str().unwrap(), "--cikti", c.to_str().unwrap(), "--rea", "--onayla", "--yalitim", "konteyner"], "evet\n", &[]);
    assert_eq!(kod, Some(0), "{m}");
    let b = o.belge(&c);
    let k = kanit(&b, "#inspect").unwrap_or_else(|| panic!("{}", b["bilinmeyenler"]));
    assert_eq!(k["paylasilabilir"], false);
    let depo: Value = serde_json::from_str(&std::fs::read_to_string(c.join("kanit.json")).unwrap()).unwrap();
    let ticari: Vec<String> = depo["kanitlar"].as_array().unwrap().iter().filter(|k| k["ozet"]["paylasilabilir"] == false).map(|k| k["id"].as_str().unwrap().to_string()).collect();
    assert!(!ticari.is_empty(), "ticari kanıt yerel depoda olmalı");
    let kart = std::fs::read_to_string(c.join("benlik.json")).unwrap();
    let paket = std::fs::read_to_string(c.join("benlik-evidence.json")).unwrap();
    for id in &ticari {
        assert!(!kart.contains(id.as_str()) && !paket.contains(id.as_str()), "ticari kanıt {id} karta ya da pakete girdi");
    }
    assert!(!paket.contains("\"ida\""), "ticari sağlayıcı paket kaydında görünmemeli");
}

/// WebMCP: yalnızca loopback; onaysız çalışmaz; açıklama saklanmaz; yalıtımsız inferred; kartta x-webmcp, grafta ext/cap
#[test]
fn w3r_webmcp_kapilari_ve_projeksiyon() {
    let o = Ortam::yeni("webmcp", "motorsuz", "ghidra");
    let c = o.kok.join("cikti");
    let cs = c.to_str().unwrap();
    // Uzak adres reddedilir, hiçbir REA komutu çalışmaz
    let (kod, _) = o.terminalde(&["kesfet", "--webmcp", "http://192.0.2.10:9222", "--cikti", cs, "--onayla", "--yalitim", "konteyner"], "evet\n", &[]);
    assert_eq!(kod, Some(0));
    assert!(bilinmeyen_var(&o.belge(&c), "bu makinede değil"), "{}", o.belge(&c)["bilinmeyenler"]);
    assert!(!o.cagrildi_mi("list-browser-targets") && !o.cagrildi_mi("discover-webmcp-tools"), "{:?}", o.cagrilar());
    // Onaysız
    let (_, _) = o.calis(&["kesfet", "--webmcp", "http://127.0.0.1:9222", "--cikti", cs], &[]);
    assert!(!o.cagrildi_mi("discover-webmcp-tools"), "{:?}", o.cagrilar());
    // Onaylı, yalıtım yok
    let (kod, m) =
        o.terminalde(&["kesfet", "--webmcp", "http://127.0.0.1:9222", "--kaynak", "http://127.0.0.1:8000", "--cikti", cs, "--onayla", "--yalitim", "yok"], "evet\n", &[]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(o.cagrildi_mi("list-browser-targets") && o.cagrildi_mi("discover-webmcp-tools"), "{:?}", o.cagrilar());
    assert!(o.cagrilar().iter().any(|l| l.contains("--allowed-origins http://127.0.0.1:8000")), "{:?}", o.cagrilar());
    let b = o.belge(&c);
    let k = kanit(&b, "#discover_webmcp_tools").unwrap_or_else(|| panic!("{}", b["bilinmeyenler"]));
    assert_eq!((k["kaynak_turu"].as_str(), k["beyan"].as_str(), k["dayanak"].as_array().map(Vec::len)), (Some("BROWSER_OBSERVATION"), Some("inferred"), Some(0)));
    let arac = &k["ozet"]["araclar"][0];
    assert_eq!(arac["ad"], "rezervasyon_ara");
    assert_eq!(arac["girdi"][0], serde_json::json!({ "ad": "tarih", "tur": "string", "zorunlu": true }));
    for d in ["kaynaksiz.json", "benlik.json", "kanit.json"] {
        let metin = std::fs::read_to_string(c.join(d)).unwrap();
        assert!(!metin.contains("Tarihe gore oda arar") && !metin.contains("YYYY-MM-DD"), "{d}: sayfanın açıklaması saklanmamalı");
    }
    let kart: Value = serde_json::from_str(&std::fs::read_to_string(c.join("benlik.json")).unwrap()).unwrap();
    let w = &kart["x-webmcp"][0];
    assert_eq!(w["origin"], "http://127.0.0.1:8000");
    assert_eq!(w["tools"][0]["name"], "rezervasyon_ara");
    assert_eq!((w["tools"][0]["stamp"].as_str(), w["tools"][0]["requires_approval"].as_bool()), (Some("inferred"), Some(true)));
    assert_eq!(kart["operations"], serde_json::json!([]));
    let graf: Value = serde_json::from_str(&std::fs::read_to_string(c.join("graf.json")).unwrap()).unwrap();
    assert!(graf["kenarlar"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k["kaynak"] == "ext:web/http://127.0.0.1:8000" && k["iliski"] == "IMPLEMENTS" && k["hedef"] == "cap:webmcp/http://127.0.0.1:8000#rezervasyon_ara"));
}

/// Firmware: motor (Binwalk) yoksa çalışmaz; varsa onayla bölüm envanteri (inferred), içerik saklanmaz
#[test]
fn w3r_firmware_motor_ve_envanter() {
    let o = Ortam::yeni("firmware", "motorsuz", "ghidra");
    let y = o.kok.join("fw");
    std::fs::create_dir_all(&y).unwrap();
    std::fs::write(y.join("cihaz.bin"), vec![0x27u8; 4096]).unwrap();
    let c = o.kok.join("cikti");
    let a = ["kesfet", y.to_str().unwrap(), "--cikti", c.to_str().unwrap(), "--rea", "--onayla", "--yalitim", "konteyner"];
    let (kod, m) = o.terminalde(&a, "evet\n", &[]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(bilinmeyen_var(&o.belge(&c), "REA_BINWALK_COMMAND"), "{}", o.belge(&c)["bilinmeyenler"]);
    assert!(!o.cagrildi_mi("inspect-firmware-regions"));
    let (kod, m) = o.terminalde(&a, "evet\n", &[("REA_BINWALK_COMMAND", "/bin/true")]);
    assert_eq!(kod, Some(0), "{m}");
    assert!(o.cagrildi_mi("inspect-firmware-regions"), "{:?}", o.cagrilar());
    let b = o.belge(&c);
    let k = kanit(&b, "#inspect_firmware_regions").unwrap_or_else(|| panic!("{}", b["bilinmeyenler"]));
    assert_eq!((k["kaynak_turu"].as_str(), k["beyan"].as_str()), (Some("BINARY_HEURISTIC"), Some("inferred")));
    assert_eq!(k["ozet"]["regions"]["ogeler"][1]["description"], "Squashfs filesystem");
    assert!(!std::fs::read_to_string(c.join("kaynaksiz.json")).unwrap().contains("GIZLI_ICERIK"));
}

/// Tablo dışı komutlar sınıflanmaz (reddedilir); izin listesi sabitlenen tablodan 83 araç
#[test]
fn w3r_tablo_disi_ve_izin_listesi() {
    let o = Ortam::yeni("tablo", "motorsuz", "ghidra");
    let (kod, m) = o.calis(&["kesfet", "--yetenekler", "--json"], &[]);
    assert_eq!(kod, Some(0), "{m}");
    let v: Value = serde_json::from_str(&json_kismi(&m)).unwrap();
    assert_eq!(v["izin_listesi"], 83);
    // setup / update gibi aracı etki tablosunda olmayan komutlar "tablo dışı" sayılır (çalıştırılmaz)
    assert!(v["butun_komutlar"]["tablo dışı"].as_u64().unwrap() >= 2, "{}", v["butun_komutlar"]);
}

/// CDP adresi yalnızca bu makine: uzak, şemasız ya da kılık değiştirmiş adreslere hiç bağlanılmaz
#[test]
fn w3r_cdp_yalnizca_loopback() {
    let o = Ortam::yeni("loopback", "motorsuz", "ghidra");
    let c = o.kok.join("cikti");
    let cs = c.to_str().unwrap();
    for uzak in ["http://192.0.2.1:9222", "https://127.0.0.1:9222", "http://127.0.0.1.ornek.test:9222", "http://kullanici@ornek.test:9222", "127.0.0.1:9222"] {
        o.iz_sil();
        let (kod, m) = o.calis(&["kesfet", "--webmcp", uzak, "--cikti", cs], &[]);
        assert_eq!(kod, Some(0), "{uzak}: {m}");
        assert!(bilinmeyen_var(&o.belge(&c), "bu makinede değil"), "{uzak}: {}", o.belge(&c)["bilinmeyenler"]);
        assert!(!o.cagrildi_mi("list-browser-targets"), "{uzak}");
    }
    for yerel in ["http://127.0.0.1:9222", "http://localhost:9222/json", "ws://127.0.0.1:9222/devtools/browser/x", "http://[::1]:9222"] {
        let (kod, _) = o.calis(&["kesfet", "--webmcp", yerel, "--cikti", cs], &[]);
        assert_eq!(kod, Some(0));
        let b = o.belge(&c);
        assert!(!bilinmeyen_var(&b, "bu makinede değil") && bilinmeyen_var(&b, "onay ister"), "{yerel}: {}", b["bilinmeyenler"]);
    }
}
