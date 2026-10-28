//! D8 İngilizce (Ayna): her istek kendi dilinde cevaplanır. `?lang=en` (ya da `X-Sezgi-Lang: en`) ile `/api/sezgi`,
//! `/api/sorular`, `/api/islem`, `/api/ara` ve `/api/onizle` metinleri İngilizce; sayfa `<html lang="en">` ve İngilizce
//! sabit metinlerle gelir, dil düğmesi taşır. Dil verilmezse sürecin dili (`SEZGI_LANG`, `--lang`), o da yoksa Türkçe;
//! Türkçe cevap değişmez. Anahtar ve köken denetimi dilden bağımsızdır.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use regex::Regex;
use serde_json::{json, Value};
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
fn ayna_ortamla(p: &Proje, ek: &[&str], ortam: &[(&str, &str)]) -> Ayna {
    let mut args = vec!["ayna", "--port", "0"];
    args.extend_from_slice(ek);
    let mut k = p.komut(&args);
    k.env_remove("SEZGI_LANG");
    for (a, d) in ortam {
        k.env(a, d);
    }
    let mut c = k.stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
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

fn ayna(p: &Proje) -> Ayna {
    ayna_ortamla(p, &[], &[])
}

fn iste(a: &Ayna, yontem: &str, yol: &str, govde: &str, ek: &str) -> (String, String) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", a.port)).unwrap();
    write!(
        s,
        "{yontem} {yol} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Sezgi-Anahtar: {}\r\n{ek}\r\n{govde}",
        a.port,
        govde.len(),
        a.anahtar
    )
    .unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    let (d, g) = c.split_once("\r\n\r\n").unwrap_or((&c, ""));
    (d.lines().next().unwrap_or("").to_string(), g.to_string())
}

fn al(a: &Ayna, yol: &str) -> Value {
    let (d, g) = iste(a, "GET", yol, "", "");
    assert!(d.contains("200"), "{yol}: {d} {g}");
    serde_json::from_str(&g).unwrap()
}

fn gonder(a: &Ayna, yol: &str, govde: Value) -> Value {
    let (d, g) = iste(a, "POST", yol, &govde.to_string(), "");
    assert!(d.contains("200"), "{yol}: {d} {g}");
    serde_json::from_str(&g).unwrap()
}

/// İnsana gösterilen metin alanları (makine değerleri: durum, karar, risk, kimlik, güven sayılmaz)
const METIN_ALANLARI: [&str; 12] = ["baslik", "aciklama", "ayrinti", "metin", "soru", "bilinmeyen", "anlam", "not", "plan", "ad", "ornekler", "etki"];

fn metinler(v: &Value, anahtar: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                metinler(x, k, out);
            }
        }
        Value::Array(a) => {
            for x in a {
                metinler(x, anahtar, out);
            }
        }
        Value::String(s) if METIN_ALANLARI.contains(&anahtar) => out.push(s.clone()),
        _ => {}
    }
}

const TURKCE: [&str; 43] = [
    "kaskad", "silinince", "veritabani",
    "kapi", "islem", "islemi", "islemler", "giris", "gerekli", "bilinmiyor", "yorum", "kesin", "zorunlu", "yetki", "sonuc", "reddedilir", "gecer",
    "degil", "olmali", "bulunamadi", "icin", "daha", "govde", "sorgu", "tablolar", "roller", "surum", "guven", "onay", "ekran", "yazar", "okur",
    "kanit", "onerme", "uygulama", "uygulamanin", "alani", "adim", "ya", "ve", "ile", "gibi", "kontrol",
];

fn sade(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ç' | 'Ç' => 'c',
            'ğ' | 'Ğ' => 'g',
            'ı' | 'I' | 'İ' => 'i',
            'ö' | 'Ö' => 'o',
            'ş' | 'Ş' => 's',
            'ü' | 'Ü' => 'u',
            x => x.to_ascii_lowercase(),
        })
        .collect()
}

/// JSON cevabın insana gösterilen metinlerinde kalan Türkçe. Kod parçaları, Sezgi komutları, uygulamanın verisi (yol,
/// işleyici adı) ve kullanıcının kendi isteği (`veri`) sayılmaz
fn turkce_kalinti(v: &Value, veri: &[String]) -> Vec<String> {
    let mut m = vec![];
    metinler(v, "", &mut m);
    let kod = Regex::new(r"`[^`]*`").unwrap();
    let komut = Regex::new(r"sezgi( [a-z0-9_.<>|:\-]+)+").unwrap();
    // Tırnak içindeki alıntılar (kullanıcının isteği, koddaki özet) ve işlem kimlikleri (`yorum.toplu-sil`) uygulamanın verisidir
    let alinti = Regex::new(r#""[^"]*"|“[^”]*”|«[^»]*»|[\w-]+\.[\w.-]+"#).unwrap();
    let mut veri: Vec<&String> = veri.iter().filter(|v| v.chars().count() >= 3).collect();
    veri.sort_by_key(|v| std::cmp::Reverse(v.len()));
    // Uygulamanın kendi sözcükleri (yol parçası, modül adı) Türkçe sayılmaz
    let uygulama_sozcukleri: std::collections::HashSet<String> =
        veri.iter().flat_map(|v| v.split(|c: char| !c.is_alphanumeric()).map(sade).collect::<Vec<_>>()).collect();
    let mut out = vec![];
    for s in m {
        let mut s = s;
        for v in &veri {
            s = s.replace(v.as_str(), " ");
        }
        let s = komut.replace_all(&kod.replace_all(&s, " "), " ").to_string();
        let s = alinti.replace_all(&s, " ").to_string();
        if s.chars().any(|c| "çğıöşüÇĞİÖŞÜ".contains(c)) {
            out.push(s.clone());
            continue;
        }
        if s.split(|c: char| !c.is_alphanumeric()).any(|w| TURKCE.contains(&sade(w).as_str()) && !uygulama_sozcukleri.contains(&sade(w))) {
            out.push(s);
        }
    }
    out
}

fn ingilizce(adim: &str, v: &Value, veri: &[String]) {
    let k = turkce_kalinti(v, veri);
    assert!(k.is_empty(), "{adim}: Türkçe metin kaldı:\n{}", k.join("\n"));
}

/// Fonksiyon adının okunur hâli (özeti olmayan işlemin açıklaması): postApiNotlar -> "post api notlar"
fn addan(ad: &str) -> String {
    let mut s = String::new();
    let mut onceki_kucuk = false;
    for c in ad.chars() {
        if !c.is_alphanumeric() {
            s.push(' ');
            onceki_kucuk = false;
            continue;
        }
        if c.is_uppercase() && onceki_kucuk {
            s.push(' ');
        }
        onceki_kucuk = c.is_lowercase() || c.is_ascii_digit();
        s.extend(c.to_lowercase());
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Örnekte bütün tanıtım uçları İngilizce: kartlar, 17 soru, her işlemin ayrıntısı, arama ve plan
fn uclar_ingilizce(ornek: &str, aramalar: &[&str]) {
    let ev = Ev::yeni(&format!("d8-ing-ayna-{ornek}"));
    let p = ev.proje(ornek, "a");
    p.insan_capasi();
    let a = ayna(&p);
    let harita = al(&a, "/api/harita");
    // Uygulamanın verisi: kapı kimliği, yol, işleyici ve dosya adları
    let mut veri: Vec<String> = vec![];
    for k in harita["endpointler"].as_array().unwrap() {
        for x in ["id", "yol", "kontrolcu", "fonksiyon", "ozet", "dosya"] {
            veri.extend(k[x].as_str().map(String::from));
        }
        veri.push(addan(k["fonksiyon"].as_str().unwrap_or("")));
    }
    veri.extend(aramalar.iter().map(|q| q.to_string()));
    ingilizce("sezgi", &al(&a, "/api/sezgi?lang=en"), &veri);
    let s = al(&a, "/api/sorular?lang=en");
    assert_eq!(s["sorular"].as_array().unwrap().len(), 17);
    ingilizce("sorular", &s, &veri);
    for k in harita["endpointler"].as_array().unwrap() {
        let v = gonder(&a, "/api/islem?lang=en", json!({ "kapi": k["id"] }));
        let ids: Vec<&str> = v["bolumler"].as_array().unwrap().iter().map(|b| b["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["ne", "kim", "zincir", "veri", "dis", "onkosul", "geri", "onay"]);
        ingilizce(&format!("islem {}", k["id"]), &v, &veri);
        let o = gonder(&a, "/api/onizle?lang=en", json!({ "kapi": k["id"], "govde": { "zzz": 1 }, "oturum": false }));
        assert!(o["metin"].as_str().unwrap().starts_with("PREVIEW "), "{o}");
    }
    for q in aramalar {
        ingilizce(&format!("ara {q}"), &gonder(&a, "/api/ara?lang=en", json!({ "sorgu": q })), &veri);
    }
}

#[test]
fn cagri_ornegi_uclari_ingilizce() {
    uclar_ingilizce("d8_cagri_nest", &["delete album", "albümü sil ve albümleri listele"]);
}

#[test]
fn dis_etki_ornegi_uclari_ingilizce() {
    uclar_ingilizce("d8_dis_express", &["create order", "delete the note and then create an order"]);
}

#[test]
fn anlam_ornegi_uclari_ingilizce() {
    uclar_ingilizce("d8_anlam_foto", &["delete user", "kullanıcıyı sil ve albümlerini başkasına devret"]);
}

#[test]
fn davranis_ornegi_uclari_ingilizce() {
    uclar_ingilizce("w2dna_nest", &["delete"]);
}

/// Aynı Ayna iki dilde cevap verir; dil verilmezse Türkçe ve eskisi gibi
#[test]
fn ayni_ayna_iki_dilde() {
    let ev = Ev::yeni("d8-ing-ayna-iki");
    let p = ev.proje("d8_anlam_foto", "foto");
    let a = ayna(&p);
    let tr = al(&a, "/api/sezgi");
    let en = al(&a, "/api/sezgi?lang=en");
    let baslik = |v: &Value| v["yetenekler"][0]["baslik"].as_str().unwrap().to_string();
    assert_eq!(baslik(&tr), "Uygulamayı anlar");
    assert_eq!(baslik(&en), "Understands the application");
    // Başlıkla da seçilir
    let (_, g) = iste(&a, "GET", "/api/sezgi", "", "X-Sezgi-Lang: en\r\n");
    assert_eq!(baslik(&serde_json::from_str(&g).unwrap()), "Understands the application");
    // Anlam özeti İngilizce adlarıyla; makine değerleri aynı
    assert_eq!(en["anlam"]["alan"]["ad"], "photo and video library", "{}", en["anlam"]);
    assert_eq!(tr["anlam"]["alan"]["ad"], "fotoğraf ve video arşivi");
    let durumlar = |v: &Value| v["yetenekler"].as_array().unwrap().iter().map(|y| y["durum"].clone()).collect::<Vec<_>>();
    assert_eq!(durumlar(&tr), durumlar(&en));
    // Sorular: aynı kimlikler, aynı durum ve damgalar
    let (st, se) = (al(&a, "/api/sorular"), al(&a, "/api/sorular?lang=en"));
    assert_eq!(st["sorular"][0]["soru"], "Bu uygulama nedir?");
    assert_eq!(se["sorular"][0]["soru"], "What is this application?");
    for (x, y) in st["sorular"].as_array().unwrap().iter().zip(se["sorular"].as_array().unwrap()) {
        assert_eq!((&x["id"], &x["durum"], &x["guven"]), (&y["id"], &y["durum"], &y["guven"]));
        assert_eq!(x["maddeler"].as_array().unwrap().len(), y["maddeler"].as_array().unwrap().len(), "{}", x["id"]);
    }
    // İşlem ayrıntısı ve arama: karar aynı, metin iki dilde
    let v_tr = gonder(&a, "/api/ara", json!({ "sorgu": "kullanıcıyı sil" }));
    let v_en = gonder(&a, "/api/ara?lang=en", json!({ "sorgu": "kullanıcıyı sil" }));
    assert_eq!(v_tr["kapilar"], v_en["kapilar"]);
    assert_eq!(v_tr["sonuclar"][0]["onay"]["karar"], v_en["sonuclar"][0]["onay"]["karar"]);
    assert!(v_tr["sonuclar"][0]["onay"]["metin"].as_str().unwrap().starts_with("Silme işlemi"), "{v_tr}");
    assert!(v_en["sonuclar"][0]["onay"]["metin"].as_str().unwrap().starts_with("A delete"), "{v_en}");
    // Bilinmeyen dil değeri: sürecin dili (Türkçe)
    assert_eq!(baslik(&al(&a, "/api/sezgi?lang=de")), "Uygulamayı anlar");
}

/// Sayfa isteğin dilinde: `<html lang>`, sabit metinler ve dil düğmesi; betik iki dilin metinlerini taşır
#[test]
fn sayfa_dili_ve_dil_dugmesi() {
    let ev = Ev::yeni("d8-ing-ayna-sayfa");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    let (_, tr) = iste(&a, "GET", "/", "", "");
    assert!(tr.contains("<html lang=\"tr\">") && tr.contains("Ne yapmak istiyorsun?") && tr.contains("id=\"dil\""), "TR sayfa");
    let (_, en) = iste(&a, "GET", "/?lang=en", "", "");
    assert!(en.contains("<html lang=\"en\">") && en.contains("What do you want to do?") && en.contains("id=\"dil\""), "EN sayfa");
    assert!(!en.contains("Ne yapmak istiyorsun") && !en.contains("Kapılar"), "EN sayfada Türkçe sabit metin");
    // İki sayfanın öğe kimlikleri aynı (betik ikisinde de çalışır)
    let ids = |s: &str| Regex::new(r#"id="([^"]+)""#).unwrap().captures_iter(s).map(|c| c[1].to_string()).collect::<Vec<_>>();
    assert_eq!(ids(&tr), ids(&en));
    let (_, js) = iste(&a, "GET", "/ayna.js", "", "");
    for parca in ["const M = {", "const M_EN = {", "ayna-dil", "lang="] {
        assert!(js.contains(parca), "ayna.js: {parca}");
    }
    // Anahtarsız veri yine yok, dil ne olursa olsun
    let mut s = std::net::TcpStream::connect(("127.0.0.1", a.port)).unwrap();
    write!(s, "GET /api/sezgi?lang=en HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", a.port).unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    assert!(c.starts_with("HTTP/1.1 403") && c.contains("session key"), "{c}");
    // Başka kökenden istek dilden bağımsız reddedilir
    let (d, g) = iste(&a, "POST", "/api/islem?lang=en", "{}", "Origin: http://kotu.example\r\n");
    assert!(d.contains("403") && g.contains("another origin"), "{d} {g}");
}

/// Sürecin dili: `SEZGI_LANG=en` ya da `--lang en` ile açılan Ayna dil verilmeyen isteğe İngilizce cevap verir; terminal
/// çıktısı da İngilizce
#[test]
fn surecin_dili_varsayilan() {
    let ev = Ev::yeni("d8-ing-ayna-surec");
    let p = ev.proje("nestjs-temel", "a");
    for (ek, ortam) in [(&["--lang", "en"][..], &[][..]), (&[][..], &[("SEZGI_LANG", "en")][..])] {
        let a = ayna_ortamla(&p, ek, ortam);
        let s = al(&a, "/api/sezgi");
        assert_eq!(s["yetenekler"][0]["baslik"], "Understands the application", "{ek:?} {ortam:?}");
        let (_, sayfa) = iste(&a, "GET", "/", "", "");
        assert!(sayfa.contains("<html lang=\"en\">"));
        // İstekte Türkçe seçilebilir
        assert_eq!(al(&a, "/api/sezgi?lang=tr")["yetenekler"][0]["baslik"], "Uygulamayı anlar");
    }
}

/// Haritasız açılış da İngilizce: kartlar, 17 sorunun bilinmeyen notu ve sıradaki adım
#[test]
fn haritasiz_ingilizce() {
    let ev = Ev::yeni("d8-ing-ayna-bos");
    let kok = ev.kok.join("b");
    std::fs::create_dir_all(&kok).unwrap();
    std::fs::write(kok.join("package.json"), r#"{ "name": "b", "dependencies": { "@nestjs/core": "10.0.0" } }"#).unwrap();
    let q = Proje { klasor: kok, ev: ev.kok.join("ev") };
    let a = ayna(&q);
    let s = al(&a, "/api/sezgi?lang=en");
    ingilizce("haritasiz sezgi", &s, &[]);
    ingilizce("haritasiz sorular", &al(&a, "/api/sorular?lang=en"), &[]);
    let (d, g) = iste(&a, "GET", "/api/harita?lang=en", "", "");
    assert!(d.contains("404") && g.contains("There is no map in this folder"), "{d} {g}");
}

/// İşlem ayrıntısının ve aramanın onay kararı dilden bağımsız: her işlemde Türkçe ve İngilizce karar, damga ve
/// gerekçedeki denetlenemeyen kural türü sayısı aynı (gerekçe dile göre değişen metinden sınıflanmaz)
#[test]
fn onay_karari_dilden_bagimsiz() {
    let ev = Ev::yeni("d8-ing-ayna-karar");
    for (i, ornek) in ["d8_cagri_nest", "d8_dis_express", "w2dna_nest", "nestjs-temel"].iter().enumerate() {
        let p = ev.proje(ornek, &format!("p{i}"));
        let a = ayna(&p);
        let harita = al(&a, "/api/harita");
        for k in harita["endpointler"].as_array().unwrap() {
            let tr = gonder(&a, "/api/islem", json!({ "kapi": k["id"] }));
            let en = gonder(&a, "/api/islem?lang=en", json!({ "kapi": k["id"] }));
            assert_eq!((&tr["onay"]["karar"], &tr["onay"]["guven"], &tr["geri"], &tr["risk"]), (&en["onay"]["karar"], &en["onay"]["guven"], &en["geri"], &en["risk"]), "{ornek} {}", k["id"]);
            let tur = |v: &Value| v["onay"]["metin"].as_str().unwrap().matches("; ").count();
            assert_eq!(tur(&tr), tur(&en), "{ornek} {}: {} | {}", k["id"], tr["onay"]["metin"], en["onay"]["metin"]);
            // Önizleme kararı da aynı
            let o_tr = gonder(&a, "/api/onizle", json!({ "kapi": k["id"] }));
            let o_en = gonder(&a, "/api/onizle?lang=en", json!({ "kapi": k["id"] }));
            assert_eq!(o_tr["karar"], o_en["karar"], "{ornek} {}", k["id"]);
        }
    }
}
