//! D7 arayüz: Ayna'nın tanıtım uçları. `/api/sezgi` yeteneklerin bu projedeki durumunu (gerçek durumdan) ve sıradaki
//! adımı, `/api/sorular` "bu uygulama hakkında ne biliyoruz" yanıtlarını güven damgalarıyla verir. İkisi de oturum
//! anahtarı ister; yanıtlar yalnızca haritadan gelir (yeni olgu yok) ve Ayna projeye yazmaz.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Stdio};

struct Ayna {
    p: Child,
    port: u16,
    anahtar: String,
}

impl Drop for Ayna {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
    }
}

#[allow(clippy::zombie_processes)]
fn ayna(p: &Proje) -> Ayna {
    let mut c = p.komut(&["ayna", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0, "Ayna adres yazmadı");
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let anahtar = k.split("#anahtar=").nth(1).unwrap().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            return Ayna { p: c, port, anahtar };
        }
    }
}

fn iste(port: u16, yol: &str, anahtar: Option<&str>) -> (String, String) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let ek = anahtar.map(|a| format!("X-Sezgi-Anahtar: {a}\r\n")).unwrap_or_default();
    write!(s, "GET {yol} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{ek}\r\n").unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    let (d, g) = c.split_once("\r\n\r\n").unwrap_or((&c, ""));
    (d.lines().next().unwrap_or("").to_string(), g.to_string())
}

fn json(port: u16, yol: &str, anahtar: &str) -> Value {
    let (d, g) = iste(port, yol, Some(anahtar));
    assert!(d.contains("200"), "{yol}: {d} {g}");
    serde_json::from_str(&g).unwrap()
}

const OLCEK: [&str; 4] = ["unknown", "inferred", "observed", "certain"];
fn sira(g: &Value) -> usize {
    OLCEK.iter().position(|x| g == *x).unwrap_or_else(|| panic!("ölçek dışı güven: {g}"))
}

/// Klasördeki bütün dosyalar ve boyutları (Ayna'nın projeye yazmadığını görmek için)
fn dosyalar(k: &std::path::Path) -> Vec<(String, u64)> {
    let mut v = vec![];
    for g in std::fs::read_dir(k).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            v.extend(dosyalar(&p));
        } else {
            v.push((p.display().to_string(), p.metadata().unwrap().len()));
        }
    }
    v.sort();
    v
}

#[test]
fn tanitim_uclari_anahtar_ister() {
    let ev = Ev::yeni("d7-arayuz-anahtar");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    for yol in ["/api/sezgi", "/api/sorular"] {
        let (d, g) = iste(a.port, yol, None);
        assert!(d.contains("403") && !g.contains("yetenekler") && !g.contains("sorular\""), "{yol} anahtarsız açılmamalı: {d} {g}");
        let (d, _) = iste(a.port, yol, Some("yanlis"));
        assert!(d.contains("403"), "{yol}: {d}");
        let (d, _) = iste(a.port, yol, Some(&a.anahtar));
        assert!(d.contains("200"), "{yol}: {d}");
    }
}

#[test]
fn yetenekler_gercek_durumdan_ve_sonraki_adim() {
    let ev = Ev::yeni("d7-arayuz-yetenek");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    let s = json(a.port, "/api/sezgi", &a.anahtar);
    assert_eq!(s["harita"], true, "{s}");
    assert_eq!(s["uygulama"], "nestjs-temel-ornek", "{s}");
    assert_eq!(s["islem"], 8, "{s}");
    assert_eq!(s["cati"], "NestJS", "{s}");
    let y = s["yetenekler"].as_array().unwrap();
    let ids: Vec<&str> = y.iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["anla", "etki", "bagla", "canli", "gor", "santral", "kaynaksiz"], "{s}");
    for x in y {
        assert!(["hazir", "kurulmamis", "uygulanamaz"].contains(&x["durum"].as_str().unwrap()), "{x}");
        assert!(x["komut"].as_str().unwrap().starts_with("sezgi "), "her kartın tek satırlık komutu: {x}");
        assert!(!x["aciklama"].as_str().unwrap().is_empty() && !x["ayrinti"].as_str().unwrap().is_empty(), "{x}");
    }
    let durum = |id: &str| y.iter().find(|x| x["id"] == id).unwrap()["durum"].as_str().unwrap().to_string();
    assert_eq!(durum("anla"), "hazir");
    assert_eq!(durum("gor"), "hazir");
    // Bağlantı yok, canlı mod kurulmamış: kurulmamış; kaynak kodlu uygulamada kaynaksız keşif uygulanamaz
    assert_eq!(durum("bagla"), "kurulmamis");
    assert_eq!(durum("canli"), "kurulmamis");
    assert_eq!(durum("kaynaksiz"), "uygulanamaz");
    assert_eq!(s["sonraki"]["komut"], "sezgi bagla", "{s}");

    // Gerçek durum değişince kart da değişir: canlı mod kurulunca (hedef, mod, token) "hazır"
    let uyg = Uygulama::yeni();
    p.ok(&["hedef", &uyg.adres(), "--test-kopyasi"]);
    p.ok(&["mod", "okuma"]);
    p.ok(&["token", "olustur", "--ad", "t", "--izin", "kitap.listele:oku"]);
    let s = json(a.port, "/api/sezgi", &a.anahtar);
    let canli = s["yetenekler"].as_array().unwrap().iter().find(|x| x["id"] == "canli").unwrap().clone();
    assert_eq!(canli["durum"], "hazir", "{canli}");
    assert!(canli["ayrinti"].as_str().unwrap().contains("okuma") && canli["ayrinti"].as_str().unwrap().contains("1 geçerli token"), "{canli}");
    // Hedefin adresi gösterilmez (yalnızca türü)
    assert!(!canli.to_string().contains(&uyg.port.to_string()), "{canli}");

    // Kod değişince harita bayat: sıradaki adım haritayı yenilemek
    let dosya = p.klasor.join("src").join("kitap").join("kitap.controller.ts");
    let eski = std::fs::read_to_string(&dosya).unwrap();
    std::fs::write(&dosya, format!("{eski}\n// degisti\n")).unwrap();
    let s = json(a.port, "/api/sezgi", &a.anahtar);
    assert!(s["degisen"].as_u64().unwrap() >= 1, "{s}");
    assert_eq!(s["sonraki"]["komut"], "sezgi init", "{s}");
}

#[test]
fn sorular_guven_damgali_ve_haritadan() {
    let ev = Ev::yeni("d7-arayuz-sorular");
    let p = ev.proje("nestjs-temel", "a");
    let harita: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let kimlikler: Vec<String> = harita["endpointler"].as_array().unwrap().iter().map(|k| k["id"].as_str().unwrap().to_string()).collect();
    let a = ayna(&p);
    let b = json(a.port, "/api/sorular", &a.anahtar);
    assert_eq!(b["harita"], true);
    let olcek: Vec<&str> = b["olcek"].as_array().unwrap().iter().map(|o| o["guven"].as_str().unwrap()).collect();
    assert_eq!(olcek, ["certain", "observed", "inferred", "unknown"], "ölçek sırası");
    let sorular = b["sorular"].as_array().unwrap();
    assert_eq!(sorular.len(), 17, "{b}");
    for s in sorular {
        let maddeler = s["maddeler"].as_array().unwrap();
        sira(&s["guven"]);
        for m in maddeler {
            sira(&m["guven"]);
            assert!(!m["metin"].as_str().unwrap().is_empty(), "{m}");
        }
        match s["durum"].as_str().unwrap() {
            // Yanıtlanamayan soru: maddesi yok, damgası bilinmiyor, nedeni yazılı
            "bilinmiyor" => {
                assert!(maddeler.is_empty() && s["guven"] == "unknown", "{s}");
                assert!(s["bilinmeyen"].as_str().is_some_and(|x| x.len() > 20), "bilinmeyenin nedeni: {s}");
            }
            "kismen" => assert!(!maddeler.is_empty() && s["bilinmeyen"].is_string(), "{s}"),
            "yanitli" => assert!(!maddeler.is_empty() && s["bilinmeyen"].is_null(), "{s}"),
            d => panic!("bilinmeyen durum {d}"),
        }
        // Yanıtın damgası en zayıf maddesininki (sayım sorusu hariç: sayımın kendisi kesin)
        if !maddeler.is_empty() && s["id"] != "guven" {
            let en_zayif = maddeler.iter().map(|m| sira(&m["guven"])).min().unwrap();
            assert_eq!(sira(&s["guven"]), en_zayif, "{s}");
        }
        if !s["komut"].is_null() {
            assert!(s["komut"]["komut"].as_str().unwrap().starts_with("sezgi "), "{s}");
        }
    }
    let soru = |id: &str| sorular.iter().find(|s| s["id"] == id).unwrap_or_else(|| panic!("{id} yok"));
    // Amaç koddan çıkarılmaz: dürüstçe bilinmiyor
    assert_eq!(soru("amac")["durum"], "bilinmiyor");
    // Gözlem yok: "çalışıyor mu" bilinmiyor, gözlem komutu önerilir
    assert_eq!(soru("calisiyor")["durum"], "bilinmiyor");
    assert!(soru("calisiyor")["komut"]["komut"].as_str().unwrap().contains("gozlem"));
    // Ön yüz okunmadı: ekran sorusu bilinmiyor, --on-yuz önerilir
    assert_eq!(soru("ekran")["durum"], "bilinmiyor");
    assert!(soru("ekran")["komut"]["komut"].as_str().unwrap().contains("--on-yuz"));
    // Uygulama nedir: işlem sayısı haritayla aynı, kesin
    let ne = soru("ne").to_string();
    assert!(ne.contains("8 işlem"), "{ne}");
    // Onay: Sezgi'nin kendi kuralı (kesin); silme işlemleri haritadaki DELETE sayısı kadar
    let onay = soru("onay");
    assert!(onay["maddeler"].as_array().unwrap().iter().any(|m| m["guven"] == "certain" && m["metin"].as_str().unwrap().contains("silme")), "{onay}");
    let silme = harita["endpointler"].as_array().unwrap().iter().filter(|k| k["yontem"] == "DELETE").count();
    assert!(onay.to_string().contains(&format!("{silme} silme işlemi her seferinde insan onayı ister")), "{onay}");
    // Tablo yazmaları statik çıkarım: en fazla yorum
    for m in soru("tablolar")["maddeler"].as_array().unwrap().iter().filter(|m| m["metin"].as_str().unwrap().contains("yazılıyor")) {
        assert!(sira(&m["guven"]) <= sira(&Value::from("inferred")), "{m}");
    }
    // Yeni olgu yok: örnek olarak gösterilen her işlem haritada var
    for id in ["onay", "geri", "yetki", "onkosul", "calisiyor"] {
        for m in soru(id)["maddeler"].as_array().unwrap() {
            for o in m["ornekler"].as_array().unwrap() {
                let o = o.as_str().unwrap();
                if o.contains(" /") {
                    assert!(kimlikler.iter().any(|k| k == o), "{id}: haritada olmayan işlem: {o}");
                }
            }
        }
    }
}

#[test]
fn davranis_verisi_olan_haritada_geri_alma_ve_sahiplik() {
    let ev = Ev::yeni("d7-arayuz-dna");
    let p = ev.proje("w2dna_nest", "a");
    let a = ayna(&p);
    let b = json(a.port, "/api/sorular", &a.anahtar);
    let soru = |id: &str| b["sorular"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap().clone();
    // Geri alınabilirlik yalnızca çıkarım (yumuşak silme ya da geri yükleme ucu): asla kesin değil
    let geri = soru("geri");
    assert!(!geri["maddeler"].as_array().unwrap().is_empty(), "{geri}");
    assert_eq!(geri["guven"], "inferred", "{geri}");
    // Kaskadla dolaylı yazma en fazla yorum
    let tablo = soru("tablolar").to_string();
    assert!(tablo.contains("kaskad"), "{tablo}");
    let yetki = soru("yetki").to_string();
    assert!(yetki.contains("kaydın sahibi denetleniyor"), "{yetki}");
}

#[test]
fn ayna_projeye_yazmaz() {
    let ev = Ev::yeni("d7-arayuz-yazmaz");
    let p = ev.proje("nestjs-temel", "a");
    let once = dosyalar(&p.klasor);
    let a = ayna(&p);
    for yol in ["/", "/ayna.js", "/ayna.css", "/api/sezgi", "/api/sorular", "/api/harita", "/api/rapor", "/api/kanit"] {
        let (d, _) = iste(a.port, yol, Some(&a.anahtar));
        assert!(d.contains("200"), "{yol}: {d}");
    }
    drop(a);
    assert_eq!(once, dosyalar(&p.klasor), "Ayna projeye yazmamalı");
}

#[test]
fn sayfa_tanitim_sekmesiyle_acilir() {
    let ev = Ev::yeni("d7-arayuz-sayfa");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    let (_, sayfa) = iste(a.port, "/", None);
    // Açılış sekmesi Sezgi; sekmeler WAI-ARIA rolleriyle, paneller bağlı
    let sezgi = sayfa.lines().find(|l| l.contains("id=\"sekme-sezgi\"")).expect("Sezgi sekmesi yok");
    assert!(sezgi.contains("role=\"tab\"") && sezgi.contains("aria-selected=\"true\"") && sezgi.contains("aria-controls=\"sezgi\""), "{sezgi}");
    assert!(sayfa.contains("id=\"sekme-bilgi\"") && sayfa.contains("aria-controls=\"bilgi\""), "Ne biliyoruz sekmesi");
    assert!(sayfa.contains("role=\"tabpanel\" aria-labelledby=\"sekme-sezgi\""));
    assert!(sayfa.contains("role=\"tablist\""));
    let (_, js) = iste(a.port, "/ayna.js", None);
    for parca in ["/api/sezgi", "/api/sorular", "ArrowRight", "Home", "End", "✓ hazır", "○ kurulmamış", "– uygulanamaz"] {
        assert!(js.contains(parca), "ayna.js içinde {parca} yok");
    }
    for yasak in ["innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval(", "new Function"] {
        assert!(!js.contains(yasak), "ayna.js içinde {yasak}");
    }
    let (_, css) = iste(a.port, "/ayna.css", None);
    assert!(css.contains(":focus-visible") && css.contains("prefers-color-scheme: dark"), "odak ve koyu tema");
}

/// "Güvenle iş yaptır" kartı hedefin gerçek veri olduğunu söyler (test kopyası ya da yalnızca "tanımlı" demez);
/// adres yine gösterilmez. Terminalin yapacağı açma test ortamında ayar dosyasına doğrudan yazılır.
#[test]
fn canli_karti_gercek_veriyi_soyler() {
    let ev = Ev::yeni("d7-arayuz-gercek-veri");
    let p = ev.proje("nestjs-temel", "a");
    let uyg = Uygulama::yeni();
    p.ok(&["hedef", &uyg.adres(), "--test-kopyasi"]);
    p.ok(&["mod", "yazma"]);
    p.token("t", &["kitap.listele:oku"], &[]);
    let k = p.ev.join("ayar").join("sezgi").join("uygulamalar");
    let durum: Vec<std::path::PathBuf> = std::fs::read_dir(&k).unwrap().flatten().map(|g| g.path()).filter(|p| p.is_dir()).collect();
    assert_eq!(durum.len(), 1, "{durum:?}");
    let yol = durum[0].join("ayar.json");
    let mut ayar: Value = serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
    ayar["hedef"] = serde_json::json!({ "url": uyg.adres(), "test_kopyasi": false, "gercek_veri": true });
    ayar["gercek_adresler"] = serde_json::json!([uyg.adres()]);
    std::fs::write(&yol, ayar.to_string()).unwrap();

    let a = ayna(&p);
    let s = json(a.port, "/api/sezgi", &a.anahtar);
    let canli = s["yetenekler"].as_array().unwrap().iter().find(|x| x["id"] == "canli").unwrap().clone();
    let ayrinti = canli["ayrinti"].as_str().unwrap();
    assert!(ayrinti.contains("GERÇEK VERİ") && ayrinti.contains("terminalde") && !ayrinti.contains("test kopyası"), "{canli}");
    assert!(canli["aciklama"].as_str().unwrap().contains("gerçek veri"), "{canli}");
    assert!(!canli.to_string().contains(&uyg.port.to_string()), "{canli}");
}
