//! `sezgi paylas`: ekibin yeniden kurulum yapmadan kullanacağı harita dosyası (.sezgi/paylasim.bin).
//! Depodan gelen dosya güvenilmezdir: yalnızca kullanıcının sabitlediği anahtarla imzalıysa, sürümü uyuyorsa ve kod
//! paylaşıldığı andakiyle birebir aynıysa kullanılır; yoksa normal kurulum yapılır.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Ortam {
    kok: PathBuf,
    ev: PathBuf,
}

fn kopyala(kaynak: &Path, hedef: &Path, atla: &dyn Fn(&Path) -> bool) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap() {
        let p = g.unwrap().path();
        if atla(&p) {
            continue;
        }
        let h = hedef.join(p.file_name().unwrap());
        if p.is_dir() {
            kopyala(&p, &h, atla);
        } else {
            std::fs::copy(&p, &h).unwrap();
        }
    }
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-paylas-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let ev = kok.join("ev");
        std::fs::create_dir_all(ev.join("ayar")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &kok.join("a"), &|_| false);
        Ortam { kok, ev }
    }
    fn calistir(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sezgi"))
            .args(args)
            .env("HOME", &self.ev)
            .env("USERPROFILE", &self.ev)
            .env("APPDATA", self.ev.join("ayar"))
            .env("XDG_CONFIG_HOME", self.ev.join("ayar"))
            .output()
            .unwrap()
    }
    fn a(&self) -> String {
        self.kok.join("a").to_string_lossy().to_string()
    }
    /// Takım arkadaşının bilgisayarı: depoya girenler (harita çıktısı hariç, paylaşım dosyası dahil)
    fn klon(&self) -> String {
        let b = self.kok.join("b");
        let _ = std::fs::remove_dir_all(&b);
        kopyala(&self.kok.join("a"), &b, &|p: &Path| {
            let ad = p.file_name().unwrap().to_string_lossy().to_string();
            p.parent().is_some_and(|u| u.ends_with(".sezgi")) && !matches!(ad.as_str(), "paylasim.bin" | ".gitignore" | ".gitattributes")
        });
        b.to_string_lossy().to_string()
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

fn endpointler(k: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(Path::new(k).join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    v["endpointler"].clone()
}

/// İmzalı paylaşım: anahtar oluştur, kartı imzala, anahtarı sabitle, paylaş
fn imzali_paylas(o: &Ortam) {
    assert!(o.calistir(&["anahtar", "olustur", "--ad", "ekip"]).status.success());
    assert!(o.calistir(&["init", &o.a(), "--imzala", "ekip"]).status.success());
    let h = Path::new(&o.a()).join(".sezgi");
    let c = o.calistir(&["anahtar", "sabitle", h.join("benlik.json").to_str().unwrap(), h.join("benlik-keys.json").to_str().unwrap()]);
    assert!(c.status.success(), "{}", metin(&c));
    let c = o.calistir(&["paylas", &o.a(), "--imzala", "ekip"]);
    assert!(c.status.success(), "{}", metin(&c));
}

#[test]
fn imzasiz_paylasim_kullanilmaz() {
    let o = Ortam::yeni("imzasiz");
    let c = o.calistir(&["paylas", &o.a()]);
    assert!(c.status.success(), "{}", metin(&c));
    let sezgi = Path::new(&o.a()).join(".sezgi");
    assert!(sezgi.join("paylasim.bin").is_file());
    // Depoya girebilsin: .gitignore paylaşım dosyasını dışarıda bırakmaz; çakışmada birleştirme denenmez
    let gi = std::fs::read_to_string(sezgi.join(".gitignore")).unwrap();
    assert!(gi.contains("!paylasim.bin"), "{gi}");
    assert!(std::fs::read_to_string(sezgi.join(".gitattributes")).unwrap().contains("paylasim.bin"));
    // Paylaşım dosyası sıkıştırılmış: haritadan küçük
    assert!(std::fs::metadata(sezgi.join("paylasim.bin")).unwrap().len() < std::fs::metadata(sezgi.join("kapilar.json")).unwrap().len() / 2);
    let b = o.klon();
    let c = o.calistir(&["init", &b]);
    assert!(c.status.success());
    assert!(metin(&c).contains("imzasız"), "{}", metin(&c));
    assert!(!metin(&c).contains("paylaşım dosyasından kuruldu"));
    assert_eq!(endpointler(&b), endpointler(&o.a()));
}

#[test]
fn imzali_ve_sabitli_paylasim_ayristirmadan_kullanilir() {
    let o = Ortam::yeni("imzali");
    imzali_paylas(&o);
    let b = o.klon();
    let c = o.calistir(&["init", &b]);
    assert!(c.status.success(), "{}", metin(&c));
    assert!(metin(&c).contains("paylaşım dosyasından kuruldu"), "{}", metin(&c));
    assert_eq!(endpointler(&b), endpointler(&o.a()));
    // Kart ve imzası da gelir; doğrulanır
    let h = Path::new(&b).join(".sezgi");
    let c = o.calistir(&["dogrula", h.join("benlik.json").to_str().unwrap(), h.join("benlik-keys.json").to_str().unwrap()]);
    assert!(c.status.success() && metin(&c).contains("güvenilir"), "{}", metin(&c));
}

#[test]
fn kod_degistiyse_kullanilmaz() {
    let o = Ortam::yeni("degisti");
    imzali_paylas(&o);
    let b = o.klon();
    let f = Path::new(&b).join("src").join("kitap").join("kitap.controller.ts");
    let m = std::fs::read_to_string(&f).unwrap();
    std::fs::write(&f, format!("{m}\n// değişiklik\n")).unwrap();
    let c = o.calistir(&["init", &b]);
    assert!(c.status.success());
    assert!(metin(&c).contains("değişmiş"), "{}", metin(&c));
    assert!(!metin(&c).contains("paylaşım dosyasından kuruldu"));
}

#[test]
fn bozuk_dosya_kullanilmaz() {
    let o = Ortam::yeni("bozuk");
    imzali_paylas(&o);
    let b = o.klon();
    let p = Path::new(&b).join(".sezgi").join("paylasim.bin");
    let mut v = std::fs::read(&p).unwrap();
    let n = v.len();
    v[n - 10] ^= 0xff;
    std::fs::write(&p, v).unwrap();
    let c = o.calistir(&["init", &b]);
    assert!(c.status.success(), "{}", metin(&c));
    assert!(metin(&c).contains("kullanılmadı"), "{}", metin(&c));
    assert!(!metin(&c).contains("paylaşım dosyasından kuruldu"));
    assert_eq!(endpointler(&b), endpointler(&o.a()));
}

#[test]
fn sabitlenmemis_anahtar_kullanilmaz() {
    let o = Ortam::yeni("sabitsiz");
    imzali_paylas(&o);
    let b = o.klon();
    // Başka bir bilgisayar: sabitlenmiş anahtar yok
    std::fs::remove_file(o.ev.join("ayar").join("sezgi").join("guvenilen-anahtarlar.json")).unwrap();
    let c = o.calistir(&["init", &b]);
    assert!(c.status.success());
    assert!(metin(&c).contains("sabitlenmemiş"), "{}", metin(&c));
    assert!(!metin(&c).contains("paylaşım dosyasından kuruldu"));
}

#[test]
fn sahte_harita_imzadan_gecmez() {
    let o = Ortam::yeni("sahte");
    imzali_paylas(&o);
    let b = o.klon();
    let p = Path::new(&b).join(".sezgi").join("paylasim.bin");
    let ham = std::fs::read(&p).unwrap();
    let baslik = b"SEZGI-PAYLASIM/1\n";
    let acik = miniz_oxide::inflate::decompress_to_vec_zlib(&ham[baslik.len()..]).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&acik).unwrap();
    // Saldırgan: giriş isteyen bir ucu "herkese açık" gösterir, dosyayı geçerli biçimde yeniden paketler
    let uc = v["kapilar"]["endpointler"].as_array_mut().unwrap().iter_mut().find(|e| e["giris"] == "gerekli").expect("giriş isteyen uç");
    uc["giris"] = serde_json::json!("herkese_acik");
    let mut yeni = baslik.to_vec();
    yeni.extend(miniz_oxide::deflate::compress_to_vec_zlib(&serde_json::to_vec(&v).unwrap(), 9));
    std::fs::write(&p, yeni).unwrap();
    let c = o.calistir(&["init", &b]);
    assert!(c.status.success());
    assert!(metin(&c).contains("imza"), "{}", metin(&c));
    assert!(!metin(&c).contains("paylaşım dosyasından kuruldu"));
    assert_eq!(endpointler(&b), endpointler(&o.a()), "harita koddan kurulmalı");
}
