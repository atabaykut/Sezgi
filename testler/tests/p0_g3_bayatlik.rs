//! P0.5 (tek dosya listesi) ve doktor çatı tanıma: saf JavaScript Express uygulamasında kod değişikliği izleme,
//! bayatlık kapısı, doktor ve paylaşım dosyası tarafından görülür; doktor Express / Fastify / Next uygulamalarını
//! init'in seçtiği çatıyla tanır.
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

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

struct Ortam {
    kok: PathBuf,
    ev: PathBuf,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

impl Ortam {
    fn yeni(ad: &str, ornek: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-p0g3-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let ev = kok.join("ev");
        std::fs::create_dir_all(ev.join("ayar")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ornek), &kok.join("app"));
        Ortam { kok, ev }
    }
    fn app(&self) -> PathBuf {
        self.kok.join("app")
    }
    fn komut(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args)
            .current_dir(self.app())
            .env("HOME", &self.ev)
            .env("USERPROFILE", &self.ev)
            .env("APPDATA", self.ev.join("ayar"))
            .env("XDG_CONFIG_HOME", self.ev.join("ayar"))
            .env_remove("SEZGI_TOKEN")
            .env("SEZGI_ANAHTARLIK", "kapali");
        c
    }
    fn sezgi(&self, args: &[&str], girdi: Option<&str>, ortam: &[(&str, &str)]) -> Output {
        let mut c = self.komut(args);
        for (a, d) in ortam {
            c.env(a, d);
        }
        c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut p = c.spawn().unwrap();
        if let Some(g) = girdi {
            p.stdin.as_mut().unwrap().write_all(g.as_bytes()).unwrap();
        }
        drop(p.stdin.take());
        p.wait_with_output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let c = self.sezgi(args, None, &[]);
        let m = metin(&c);
        assert!(c.status.success(), "{args:?}: {m}");
        m
    }
    fn rota_dosyasi(&self) -> PathBuf {
        self.app().join("backend").join("routes").join("productRoutes.js")
    }
    /// .js rota dosyasını değiştirir; değişiklik zamanı haritanınkinden kesin sonra olsun diye kısa bekler
    fn js_degistir(&self, eski: &str, yeni: &str) {
        std::thread::sleep(Duration::from_millis(50));
        let f = self.rota_dosyasi();
        let m = std::fs::read_to_string(&f).unwrap();
        assert!(m.contains(eski), "{eski} yok: {m}");
        std::fs::write(&f, m.replace(eski, yeni)).unwrap();
    }
    fn giris(&self, yontem: &str, yol: &str) -> String {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(self.app().join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
        let k = v["endpointler"].as_array().unwrap().iter().find(|k| k["yontem"] == yontem && k["yol"] == yol).unwrap_or_else(|| panic!("{yontem} {yol} yok"));
        k["giris"].as_str().unwrap().to_string()
    }
    fn doktor(&self) -> (bool, Vec<serde_json::Value>, String) {
        let c = self.sezgi(&["doktor", "--json"], None, &[]);
        let v: serde_json::Value = serde_json::from_slice(&c.stdout).unwrap_or_else(|e| panic!("{e}: {}", metin(&c)));
        (c.status.success(), v["kontroller"].as_array().unwrap().clone(), metin(&c))
    }
}

fn metin(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn durum(k: &[serde_json::Value], ad: &str) -> String {
    k.iter().find(|x| x["ad"] == ad).unwrap_or_else(|| panic!("'{ad}' kontrolü yok: {k:?}"))["durum"].as_str().unwrap().to_string()
}

const YETKILI: &str = ".post(protect, admin, createProduct)";
const YETKISIZ: &str = ".post(createProduct)";

#[test]
fn ornek_haritasi_beklendigi_gibi() {
    let o = Ortam::yeni("ornek", "p0_g3_saf_js");
    o.ok(&["init", "."]);
    assert_eq!(o.giris("POST", "/api/products"), "gerekli");
}

#[test]
fn izleme_js_rota_eklemesini_gorur() {
    let o = Ortam::yeni("izle", "p0_g3_saf_js");
    let mut p = o.komut(&["calistir", ".", "--aralik", "100", "--en-fazla", "1"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let (tx, rx) = mpsc::channel();
    let cikti = p.stdout.take().unwrap();
    std::thread::spawn(move || {
        for l in BufReader::new(cikti).lines().map_while(Result::ok) {
            let _ = tx.send(l);
        }
    });
    let son = Instant::now() + Duration::from_secs(60);
    let mut satirlar = vec![];
    while !satirlar.iter().any(|l: &String| l.contains("İzleniyor")) {
        satirlar.push(rx.recv_timeout(son.saturating_duration_since(Instant::now())).unwrap_or_else(|_| panic!("izleme başlamadı: {satirlar:?}")));
    }
    std::thread::sleep(Duration::from_millis(300));
    o.js_degistir("export default router;", "router.get('/ozel', getProducts);\n\nexport default router;");
    let son = Instant::now() + Duration::from_secs(15);
    let mut guncellendi = false;
    while Instant::now() < son {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(l) => {
                guncellendi |= l.contains('↻');
                satirlar.push(l);
                if guncellendi {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
    let _ = p.kill();
    let _ = p.wait();
    assert!(guncellendi, ".js değişikliği izlemede görülmedi: {satirlar:?}");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(o.app().join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    assert!(v["endpointler"].as_array().unwrap().iter().any(|k| k["yol"] == "/api/products/ozel"), "yeni uç haritada yok");
}

#[test]
fn js_yetki_kaldirilinca_bayat_kapisi_yazmayi_reddeder() {
    let o = Ortam::yeni("bayat", "p0_g3_saf_js");
    o.ok(&["init", "."]);
    // Hedef kullanılmayan bir yerel port: kapı açık kalırsa istek gider ve bağlantı hatası olur, "bayat" yazmaz
    o.ok(&["hedef", "http://127.0.0.1:9", "--test-kopyasi"]);
    o.ok(&["mod", "yazma"]);
    let c = o.sezgi(&["oturum", "ac"], Some("eyJhbGciOiJub25lIn0.eyJzdWIiOiIxIn0.x\n"), &[]);
    assert!(c.status.success(), "{}", metin(&c));
    let m = o.ok(&["token", "olustur", "--ad", "ajan", "--izin", "*:yaz", "--tum-islemler"]);
    let token = m.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
    o.js_degistir(YETKILI, YETKISIZ);
    let istek = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"t\",\"version\":\"0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"cagir\",\"arguments\":{\"kapi\":\"POST /api/products\",\"govde\":{\"name\":\"x\",\"price\":1}}}}\n"
    );
    let c = o.sezgi(&["mcp", ".sezgi"], Some(istek), &[("SEZGI_TOKEN", &token)]);
    let m = metin(&c);
    assert!(m.contains("harita bayat (1 dosya"), "JS değişikliğinden sonra yazma bayat haritayla denendi: {m}");
}

#[test]
fn js_degisikliginde_doktor_bayat_der() {
    let o = Ortam::yeni("doktor-bayat", "p0_g3_saf_js");
    o.ok(&["init", "."]);
    let (_, k, m) = o.doktor();
    assert_eq!(durum(&k, "tazelik"), "tamam", "{m}");
    o.js_degistir(YETKILI, YETKISIZ);
    let (_, k, m) = o.doktor();
    assert_eq!(durum(&k, "tazelik"), "uyari", "{m}");
    assert!(m.contains("productRoutes.js"), "{m}");
}

#[test]
fn js_degisikliginde_eski_paylasim_kullanilmaz() {
    let o = Ortam::yeni("paylas", "p0_g3_saf_js");
    o.ok(&["anahtar", "olustur", "--ad", "ekip"]);
    let m = o.ok(&["paylas", ".", "--imzala", "ekip"]);
    assert!(m.contains("Paylaşım dosyası"), "{m}");
    o.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    // Değişiklik yokken paylaşım kullanılır (ön koşul)
    let m = o.ok(&["init", "."]);
    assert!(m.contains("paylaşım dosyasından kuruldu"), "{m}");
    o.js_degistir(YETKILI, YETKISIZ);
    let m = o.ok(&["init", "."]);
    assert!(!m.contains("paylaşım dosyasından kuruldu"), "değişmiş JS koduyla eski paylaşım kullanıldı: {m}");
    assert!(m.contains("değişmiş"), "{m}");
    assert_eq!(o.giris("POST", "/api/products"), "herkese_acik", "harita koddan kurulmalı");
}

#[test]
fn eski_bicim_paylasim_yeniden_uretin_der() {
    let o = Ortam::yeni("bicim", "p0_g3_saf_js");
    o.ok(&["anahtar", "olustur", "--ad", "ekip"]);
    o.ok(&["paylas", ".", "--imzala", "ekip"]);
    o.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    let p = o.app().join(".sezgi").join("paylasim.bin");
    let ham = std::fs::read(&p).unwrap();
    let baslik_sonu = ham.iter().position(|b| *b == b'\n').unwrap() + 1;
    let acik = miniz_oxide::inflate::decompress_to_vec_zlib(&ham[baslik_sonu..]).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&acik).unwrap();
    v["bicim"] = serde_json::json!(1);
    let mut yeni = ham[..baslik_sonu].to_vec();
    yeni.extend(miniz_oxide::deflate::compress_to_vec_zlib(&serde_json::to_vec(&v).unwrap(), 9));
    std::fs::write(&p, yeni).unwrap();
    let m = o.ok(&["init", "."]);
    assert!(!m.contains("paylaşım dosyasından kuruldu"), "{m}");
    assert!(m.contains("yeniden üretin"), "eski biçim nedeniyle reddedildiği söylenmeli: {m}");
}

#[test]
fn doktor_express_fastify_next_catisini_tanir() {
    for ornek in ["express-temel", "fastify-temel", "next-temel", "p0_g3_saf_js"] {
        let o = Ortam::yeni(&format!("cati-{ornek}"), ornek);
        o.ok(&["init", "."]);
        let (basarili, k, m) = o.doktor();
        assert_eq!(durum(&k, "cati"), "tamam", "{ornek}: {m}");
        assert!(!m.contains("@nestjs/core"), "{ornek}: {m}");
        assert!(!k.iter().any(|x| x["ad"] == "uretici"), "{ornek}: init'ten hemen sonra sürüm uyarısı: {m}");
        assert!(basarili, "{ornek}: sorun yokken doktor başarısız: {m}");
    }
}
