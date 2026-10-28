//! S1b: ön yüz adaptörünün okuduğu dosyalar haritanın okunan dosyalarındadır (uygulama kökünün dışında da: `../frontend`);
//! ön yüz değişikliği içerik özetiyle bayatlık sayılır, izlenir ve tek ön yüz dosyası değişince yalnızca o yeniden
//! ayrıştırılır. Ayrıca init'in "Sonraki adım" satırları uzun harita yolunda da okunur.
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

struct Ortam {
    kok: PathBuf,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
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

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-s1b-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("ev")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("s4_fastapi_react"), &kok.join("depo"));
        Ortam { kok }
    }
    fn arka(&self) -> PathBuf {
        self.kok.join("depo").join("backend")
    }
    fn on(&self) -> PathBuf {
        self.kok.join("depo").join("frontend")
    }
    fn komut(&self, args: &[&str]) -> Command {
        let ev = self.kok.join("ev");
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args).current_dir(self.arka()).env("HOME", &ev).env("USERPROFILE", &ev).env("APPDATA", &ev).env("XDG_CONFIG_HOME", &ev).env("SEZGI_ANAHTARLIK", "kapali");
        c
    }
    fn calistir(&self, args: &[&str]) -> Output {
        self.komut(args).stdin(Stdio::null()).output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let c = self.calistir(args);
        let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
        assert!(c.status.success(), "{args:?}: {m}");
        m
    }
    fn json(&self, ad: &str) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(self.arka().join(".sezgi").join(ad)).unwrap()).unwrap()
    }
    fn tazelik(&self) -> (String, String) {
        let c = self.calistir(&["doktor", "--json"]);
        let v: serde_json::Value = serde_json::from_slice(&c.stdout).unwrap();
        let k = v["kontroller"].as_array().unwrap().iter().find(|k| k["ad"] == "tazelik").unwrap().clone();
        (k["durum"].as_str().unwrap().to_string(), k["mesaj"].as_str().unwrap_or("").to_string())
    }
}

fn zaman_ver(f: &Path, z: SystemTime) {
    std::fs::File::options().write(true).open(f).unwrap().set_modified(z).unwrap();
}

/// web-baglari.json'un süreden bağımsız hâli
fn baglar_suresiz(mut v: serde_json::Value) -> serde_json::Value {
    v["istatistik"].as_object_mut().unwrap().remove("sure_ms");
    v
}

#[test]
fn on_yuz_dosyalari_okunan_dosyalarda() {
    let o = Ortam::yeni("okunan");
    o.ok(&["init", "."]);
    let yollar: Vec<String> = o.json("kapilar.json")["ozet"]["okunan_dosyalar"].as_array().unwrap().iter().map(|d| d["yol"].as_str().unwrap().to_string()).collect();
    for y in ["app/api/routes/items.py", "../frontend/src/components/Items/AddItem.tsx", "../frontend/src/client/sdk.gen.ts", "../frontend/tsconfig.json"] {
        assert!(yollar.contains(&y.to_string()), "{y} okunan dosyalarda yok: {yollar:?}");
    }
}

#[test]
fn on_yuz_degisikligi_icerik_ozetiyle_bayat() {
    let o = Ortam::yeni("bayat");
    o.ok(&["init", "."]);
    assert_eq!(o.tazelik().0, "tamam");
    let f = o.on().join("src").join("components").join("Items").join("AddItem.tsx");
    // Yalnızca dokunuldu: bayat değil
    zaman_ver(&f, SystemTime::now() + Duration::from_secs(120));
    let (d, m) = o.tazelik();
    assert_eq!(d, "tamam", "{m}");
    // İçerik değişti: bayat
    std::fs::write(&f, format!("{}\n// değişiklik\n", std::fs::read_to_string(&f).unwrap())).unwrap();
    let (d, m) = o.tazelik();
    assert_eq!(d, "uyari", "{m}");
    assert!(m.contains("AddItem.tsx"), "{m}");
    // Ön yüze yeni dosya: bayat
    o.ok(&["init", "."]);
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(o.on().join("src").join("components").join("Items").join("Yeni.tsx"), "export const Yeni = () => null;\n").unwrap();
    let (d, m) = o.tazelik();
    assert_eq!(d, "uyari", "{m}");
    assert!(m.contains("Yeni.tsx"), "{m}");
}

#[test]
fn tek_on_yuz_dosyasi_degisince_yalnizca_o_ayristirilir() {
    let o = Ortam::yeni("artimli");
    let mut c = o.komut(&["calistir", ".", "--aralik", "100", "--en-fazla", "2"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let (tx, rx) = mpsc::channel();
    let cikti = c.stdout.take().unwrap();
    std::thread::spawn(move || {
        for l in BufReader::new(cikti).lines().map_while(Result::ok) {
            let _ = tx.send(l);
        }
    });
    let mut satirlar = vec![];
    let mut bekle = |kosul: &dyn Fn(&str) -> bool| -> String {
        let son = Instant::now() + Duration::from_secs(30);
        loop {
            let l = rx.recv_timeout(son.saturating_duration_since(Instant::now())).unwrap_or_else(|_| panic!("beklenen satır gelmedi: {satirlar:?}"));
            satirlar.push(l.clone());
            if kosul(&l) {
                return l;
            }
        }
    };
    bekle(&|l| l.contains("İzleniyor"));
    let ilk = baglar_suresiz(o.json("web-baglari.json"));
    std::thread::sleep(Duration::from_millis(300));
    let f = o.on().join("src").join("components").join("Items").join("AddItem.tsx");
    // 1) Yalnızca dokunuldu: hiçbir dosya ayrıştırılmaz, bağlar aynı kalır (önbellekten kurulur)
    zaman_ver(&f, SystemTime::now() + Duration::from_secs(60));
    let l = bekle(&|l| l.contains('↻'));
    assert!(l.contains(" 0 / "), "ön yüz dosyasına dokunulması izlenmeli ve hiçbir şey ayrıştırılmamalı: {l}");
    assert_eq!(baglar_suresiz(o.json("web-baglari.json")), ilk, "önbellekten kurulan bağlar farklı");
    std::thread::sleep(Duration::from_millis(300));
    // 2) İçerik değişti: yalnızca o dosya
    std::fs::write(&f, format!("{}\n// değişiklik\n", std::fs::read_to_string(&f).unwrap())).unwrap();
    let l = bekle(&|l| l.contains('↻'));
    assert!(l.contains(" 1 / "), "tek ön yüz dosyası değişince birden fazla dosya ayrıştırıldı: {l}");
    let _ = c.kill();
    let _ = c.wait();
}

#[test]
fn sonraki_adim_uzun_harita_yolunda_ayrik() {
    let o = Ortam::yeni("sonraki");
    let cikti = o.kok.join("bambaska-bir-cikti-klasoru");
    let m = o.ok(&["init", ".", "--cikti", cikti.to_str().unwrap()]);
    let satir = m.lines().find(|l| l.trim_start().starts_with("sezgi bagla")).unwrap_or_else(|| panic!("{m}"));
    assert!(satir.contains("klasoru ") && !satir.contains("klasoruyapay"), "harita yolu ile açıklama bitişik: {satir}");
    let satir = m.lines().find(|l| l.trim_start().starts_with("sezgi ayna")).unwrap();
    assert!(!satir.contains("klasorune"), "{satir}");
    assert!(satir.contains("klasoru "), "{satir}");
}

#[test]
fn okunan_dosya_ozeti_sha256() {
    let o = Ortam::yeni("sha256");
    o.ok(&["init", "."]);
    let h = o.json("kapilar.json");
    let d = h["ozet"]["okunan_dosyalar"].as_array().unwrap().iter().find(|d| d["yol"] == "../frontend/tsconfig.json").unwrap().clone();
    let ozet = d["ozet"].as_str().unwrap();
    assert_eq!(ozet.len(), 64, "{ozet}");
    // Dış doğrulayıcının standart aracıyla aynı özet
    let c = Command::new("sha256sum").arg(o.on().join("tsconfig.json")).output().unwrap();
    let beklenen = String::from_utf8_lossy(&c.stdout).split_whitespace().next().unwrap().to_string();
    assert_eq!(ozet, beklenen);
    // Biçim: küçük harfli onaltılık
    assert!(ozet.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{ozet}");
}
