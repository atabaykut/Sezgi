//! S1 (P1.2) tek keşif: adaptörün okuduğu dosyalar ve içerik özetleri haritada; bayatlık değişim zamanına değil içeriğe
//! bakar; Express ve FastAPI'de de yalnızca değişen dosya yeniden ayrıştırılır.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

fn okunan(p: &Proje) -> Vec<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    v["ozet"]["okunan_dosyalar"]
        .as_array()
        .unwrap_or_else(|| panic!("ozet.okunan_dosyalar yok: {}", v["ozet"]))
        .iter()
        .map(|d| (d["yol"].as_str().unwrap().to_string(), d["ozet"].as_str().unwrap().to_string()))
        .collect()
}

fn doktor_tazelik(p: &Proje) -> (String, String) {
    let c = p.sezgi(&["doktor", "--json"], None);
    let v: serde_json::Value = serde_json::from_slice(&c.stdout).unwrap();
    let k = v["kontroller"].as_array().unwrap().iter().find(|k| k["ad"] == "tazelik").unwrap().clone();
    (k["durum"].as_str().unwrap().to_string(), k["mesaj"].as_str().unwrap_or("").to_string())
}

/// Dosyanın değişim zamanını verilen zamana çeker
fn zaman_ver(f: &Path, z: SystemTime) {
    std::fs::File::options().write(true).open(f).unwrap().set_modified(z).unwrap();
}

#[test]
fn okunan_dosyalar_ve_ozetleri_haritada() {
    let ev = Ev::yeni("s1-okunan");
    // Express: test dosyası ve node_modules okunmaz, listede de yoktur
    let e = ev.proje("express-temel", "e");
    std::fs::write(e.klasor.join("routes").join("kitaplar.test.js"), "test('x', () => {});\n").unwrap();
    std::fs::create_dir_all(e.klasor.join("node_modules").join("x")).unwrap();
    std::fs::write(e.klasor.join("node_modules").join("x").join("index.js"), "module.exports = 1;\n").unwrap();
    e.ok(&["init", "."]);
    let l = okunan(&e);
    let yollar: Vec<&str> = l.iter().map(|(y, _)| y.as_str()).collect();
    assert!(yollar.contains(&"routes/kitaplar.js") && yollar.contains(&"app.js") && yollar.contains(&"package.json"), "{yollar:?}");
    assert!(!yollar.iter().any(|y| y.contains("test.js") || y.contains("node_modules")), "okunmayan dosya listede: {yollar:?}");
    assert!(l.iter().all(|(_, o)| o.len() == 64 && o.chars().all(|c| c.is_ascii_hexdigit())), "{l:?}");
    // Liste sıralı ve tekrarsız
    let mut s = yollar.clone();
    s.sort();
    s.dedup();
    assert_eq!(s, yollar);
    // FastAPI ve NestJS (Prisma şeması dahil)
    let f = ev.proje("fastapi-temel", "f");
    let y: Vec<String> = okunan(&f).into_iter().map(|x| x.0).collect();
    assert!(y.contains(&"app/api/routes/kitaplar.py".to_string()) && y.contains(&"pyproject.toml".to_string()), "{y:?}");
    let n = ev.proje("nestjs-temel", "n");
    let y: Vec<String> = okunan(&n).into_iter().map(|x| x.0).collect();
    assert!(y.contains(&"src/kitap/kitap.controller.ts".to_string()) && y.contains(&"prisma/schema.prisma".to_string()), "{y:?}");
}

/// Bayatlık içerik özetine bakar: değişim zamanı değişip içerik aynıysa bayat değil; içerik değişip değişim zamanı
/// aynı kalsa da bayat (doktor ve yazma kapısı)
#[test]
fn bayatlik_icerik_ozetiyle() {
    let ev = Ev::yeni("s1-bayat");
    let p = ev.proje("nestjs-temel", "a");
    let u = Uygulama::yeni();
    p.kur(&u, "OTURUM-A");
    let t = p.token("ajan", &["kitap.*:yaz"], &[]);
    let f = p.klasor.join("src").join("kitap").join("kitap.controller.ts");
    let ilk = std::fs::metadata(&f).unwrap().modified().unwrap();
    // 1) Yalnızca dokunuldu (içerik aynı, değişim zamanı ileri)
    zaman_ver(&f, SystemTime::now() + Duration::from_secs(120));
    let (d, m) = doktor_tazelik(&p);
    assert_eq!(d, "tamam", "içerik aynıyken bayat sayıldı: {m}");
    let mut mcp = p.mcp(&t);
    let (s, _) = mcp.cagir(kitap_olustur());
    assert!(!s.contains("bayat"), "içerik aynıyken yazma bayat diye reddedildi: {s}");
    drop(mcp);
    // 2) İçerik değişti, uzunluk ve değişim zamanı aynı
    let metin = std::fs::read_to_string(&f).unwrap();
    let i = metin.find("Kitap").expect("örnekte 'Kitap' geçmeli");
    let yeni = format!("{}Kitab{}", &metin[..i], &metin[i + 5..]);
    assert_eq!(yeni.len(), metin.len());
    std::fs::write(&f, yeni).unwrap();
    zaman_ver(&f, ilk);
    let (d, m) = doktor_tazelik(&p);
    assert_eq!(d, "uyari", "içerik değiştiği hâlde bayat sayılmadı: {m}");
    assert!(m.contains("kitap.controller.ts"), "{m}");
    let mut mcp = p.mcp(&t);
    let (s, _) = mcp.cagir(kitap_olustur());
    assert!(s.contains("harita bayat"), "içerik değişmiş haritayla yazma: {s}");
}

/// Haritadan sonra eklenen, listede olmayan kaynak dosya da bayatlık sayılır
#[test]
fn yeni_kaynak_dosya_bayatlik_sayilir() {
    let ev = Ev::yeni("s1-yeni-dosya");
    let p = ev.proje("express-temel", "e");
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(
        p.klasor.join("routes").join("yeni.js"),
        "const express = require('express');\nconst r = express.Router();\nr.get('/x', (q, s) => s.json({}));\nmodule.exports = r;\n",
    )
    .unwrap();
    let (d, m) = doktor_tazelik(&p);
    assert_eq!(d, "uyari", "{m}");
    assert!(m.contains("yeni.js"), "{m}");
}

/// `sezgi calistir`: tek dosya değişince yalnızca o dosya yeniden ayrıştırılır (Express ve FastAPI); yalnızca dokunulan
/// dosya ise hiç ayrıştırılmaz
#[test]
fn express_ve_fastapi_artimli_ayristirma() {
    for (ornek, dosya, ek) in [
        ("express-temel", "routes/kitaplar.js", "\nrouter.get('/s1-yeni', (req, res) => res.json({}));\n"),
        ("fastapi-temel", "app/api/routes/kitaplar.py", "\n\n@router.get(\"/s1-yeni\")\ndef s1_yeni():\n    return {}\n"),
    ] {
        let ev = Ev::yeni(&format!("s1-artimli-{ornek}"));
        let p = ev.proje(ornek, "a");
        let mut c = p.komut(&["calistir", ".", "--aralik", "100", "--en-fazla", "2"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let (tx, rx) = mpsc::channel();
        let cikti = c.stdout.take().unwrap();
        std::thread::spawn(move || {
            for l in BufReader::new(cikti).lines().map_while(Result::ok) {
                let _ = tx.send(l);
            }
        });
        let bekle = |kosul: &dyn Fn(&str) -> bool, satirlar: &mut Vec<String>| -> String {
            let son = Instant::now() + Duration::from_secs(30);
            loop {
                let l = rx.recv_timeout(son.saturating_duration_since(Instant::now())).unwrap_or_else(|_| panic!("{ornek}: beklenen satır gelmedi: {satirlar:?}"));
                satirlar.push(l.clone());
                if kosul(&l) {
                    return l;
                }
            }
        };
        let mut satirlar = vec![];
        bekle(&|l| l.contains("İzleniyor"), &mut satirlar);
        std::thread::sleep(Duration::from_millis(300));
        let f = p.klasor.join(dosya);
        // 1) Yalnızca dokunuldu: hiçbir dosya yeniden ayrıştırılmaz
        zaman_ver(&f, SystemTime::now() + Duration::from_secs(60));
        let l = bekle(&|l| l.contains('↻'), &mut satirlar);
        assert!(l.contains(" 0 / "), "{ornek}: içerik aynıyken yeniden ayrıştırıldı: {l}");
        std::thread::sleep(Duration::from_millis(300));
        // 2) Bir dosya değişti: yalnızca o
        let m = std::fs::read_to_string(&f).unwrap();
        std::fs::write(&f, format!("{m}{ek}")).unwrap();
        let l = bekle(&|l| l.contains('↻'), &mut satirlar);
        assert!(l.contains(" 1 / "), "{ornek}: tek dosya değişikliğinde birden fazla dosya ayrıştırıldı: {l}");
        let _ = c.kill();
        let _ = c.wait();
        assert!(std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap().contains("s1-yeni"), "{ornek}: yeni uç haritada yok");
    }
}

/// Paylaşım dosyası okunan dosyaları ve haritayı etkileyebilecek yeni dosyaları kapsar
#[test]
fn paylasim_yeni_kaynak_dosyayla_kullanilmaz() {
    let ev = Ev::yeni("s1-paylas");
    let p = ev.proje("express-temel", "e");
    p.ok(&["anahtar", "olustur", "--ad", "ekip"]);
    p.ok(&["paylas", ".", "--imzala", "ekip"]);
    p.ok(&["anahtar", "sabitle", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    assert!(p.ok(&["init", "."]).contains("paylaşım dosyasından kuruldu"));
    std::fs::write(p.klasor.join("routes").join("yeni.js"), "module.exports = 1;\n").unwrap();
    let m = p.ok(&["init", "."]);
    assert!(!m.contains("paylaşım dosyasından kuruldu") && m.contains("değişmiş"), "{m}");
}
