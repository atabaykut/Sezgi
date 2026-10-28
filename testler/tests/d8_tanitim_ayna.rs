//! D8 tanıtım: Ayna bu sürümün dört yeteneğini tutarlı gösterir.
//! - "Sezgi" sekmesi: dört ana yetenek (anlar, etkiyi bilir, kanıtla anlatır, güvenle iş yaptırır) ve yardımcılar; alt
//!   durumlar gerçek durumdan (çağrı grafiği, dış etki taraması, insan parolası); üstte anlam özeti (alan, amaç alıntısı,
//!   kullanıcı tipleri; hiçbiri kesin değil).
//! - "Ne yapmak istiyorsun?": `/api/ara` Türkçe / İngilizce istekten işlem, etkisi, geri alma ve onay kararı; çok
//!   adımlı istekte plan.
//! - Kapı ayrıntısı: `/api/islem` çağrı zinciri, tablo etkisi, nesne düzeyi yetki, dış etki, ön koşul, geri alma ve onay
//!   gerekçesi; haritanın davranış özetiyle ve canlı modun onay kuralıyla aynı.
//! - "Ne biliyoruz": 17 soru sıralı, tek üslupta; sayılar `rapor --json` ile aynı.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
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

fn iste(a: &Ayna, yontem: &str, yol: &str, govde: &str, koken: Option<&str>) -> (String, String) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", a.port)).unwrap();
    let k = koken.map(|o| format!("Origin: {o}\r\n")).unwrap_or_default();
    write!(
        s,
        "{yontem} {yol} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Sezgi-Anahtar: {}\r\n{k}\r\n{govde}",
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
    let (d, g) = iste(a, "GET", yol, "", None);
    assert!(d.contains("200"), "{yol}: {d} {g}");
    serde_json::from_str(&g).unwrap()
}

fn gonder(a: &Ayna, yol: &str, govde: Value) -> Value {
    let (d, g) = iste(a, "POST", yol, &govde.to_string(), None);
    assert!(d.contains("200"), "{yol}: {d} {g}");
    serde_json::from_str(&g).unwrap()
}

fn kart<'a>(s: &'a Value, id: &str) -> &'a Value {
    s["yetenekler"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap_or_else(|| panic!("{id} yok: {s}"))
}

fn kopyala(kaynak: &std::path::Path, hedef: &std::path::Path) {
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

/// Alt klasörü haritalanan örnek (fastapi-template'in backend'i)
fn alt_proje(ev: &Ev, ornek: &str, ad: &str, alt: &str) -> Proje {
    let kok = ev.kok.join(ad);
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ornek), &kok);
    let p = Proje { klasor: kok.join(alt), ev: ev.kok.join("ev") };
    p.ok(&["init", "."]);
    p
}

#[test]
fn sezgi_sekmesi_dort_ana_yetenek_gercek_durumdan() {
    let ev = Ev::yeni("d8-tanitim-kart");
    let p = ev.proje("d8_cagri_nest", "a");
    let a = ayna(&p);
    let s = al(&a, "/api/sezgi");
    let ids: Vec<&str> = s["yetenekler"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["anla", "etki", "bagla", "canli", "gor", "santral", "kaynaksiz"], "{s}");
    for (i, id) in ["anla", "etki", "bagla", "canli"].iter().enumerate() {
        let k = kart(&s, id);
        assert_eq!((k["grup"].as_str(), k["adim"].as_u64()), (Some("ana"), Some(i as u64 + 1)), "{k}");
        assert!(!k["durumlar"].as_array().unwrap().is_empty(), "ana kartın alt durumları gerçek durumdan: {k}");
        for d in k["durumlar"].as_array().unwrap() {
            assert!(["hazir", "kurulmamis", "uygulanamaz", "bilinmiyor"].contains(&d["durum"].as_str().unwrap()), "{d}");
        }
    }
    for id in ["gor", "santral", "kaynaksiz"] {
        assert_eq!(kart(&s, id)["grup"], "diger");
    }
    // Etki kartı: çağrı grafiği bu haritada var; sayılar rapor ile aynı
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    let etki = kart(&s, "etki");
    assert_eq!(etki["durum"], "hazir", "{etki}");
    let metin = etki["durumlar"].to_string();
    assert!(metin.contains(&format!("Çağrı grafiği: {} fonksiyon, {} çağrı", r["cagri"]["fonksiyon"], r["cagri"]["kenar"])), "{metin}");
    assert!(metin.contains(&format!("Nesne düzeyi yetki {} işlemde", r["cagri"]["nesne_duzeyi_denetimli_islem"])), "{metin}");
    // İnsan parolası: kurulmamışken söylenir, kurulunca kart değişir (Ayna yeniden başlatılmadan)
    let canli = kart(&s, "canli").to_string();
    assert!(canli.contains("İnsan parolası kurulu değil") && canli.contains("deneysel"), "{canli}");
    p.insan_capasi();
    let s = al(&a, "/api/sezgi");
    let canli = kart(&s, "canli")["durumlar"].to_string();
    assert!(canli.contains("İnsan parolası kurulu (çapa"), "{canli}");
    // Eski harita (çağrı grafiği yok): etki kartı kurulmamış, sıradaki adım haritayı yenilemek
    drop(a);
    let kanit = p.klasor.join(".sezgi").join("kanit.json");
    let mut k: Value = serde_json::from_str(&std::fs::read_to_string(&kanit).unwrap()).unwrap();
    k["ek"].as_object_mut().map(|e| e.remove("cagri"));
    std::fs::write(&kanit, k.to_string()).unwrap();
    let a = ayna(&p);
    let s = al(&a, "/api/sezgi");
    if kart(&s, "etki")["durum"] == "kurulmamis" {
        assert_eq!(s["sonraki"]["komut"], "sezgi init", "{s}");
        assert!(kart(&s, "etki")["durumlar"].to_string().contains("Çağrı grafiği yok"), "{s}");
    }
}

#[test]
fn anlam_ozeti_kanitli_ve_kesin_degil() {
    let ev = Ev::yeni("d8-tanitim-anlam");
    let p = ev.proje("d8_anlam_foto", "foto");
    let a = ayna(&p);
    let s = al(&a, "/api/sezgi");
    let an = &s["anlam"];
    assert_eq!(an["alan"]["ad"], "fotoğraf ve video arşivi", "{an}");
    assert!(an["amac"]["alinti"].as_str().unwrap().contains("Fotokutu"), "{an}");
    assert!(an["amac"]["konum"].as_str().unwrap().contains("README.md:"), "{an}");
    assert!(!an["aktorler"].as_array().unwrap().is_empty() && an["yetenek"]["sayi"].as_u64().unwrap() > 0, "{an}");
    // Anlam en fazla yorum: hiçbir damga kesin değil
    assert!(!an.to_string().contains("\"certain\""), "{an}");
    // Haritasız açılışta anlam yok
    let ev2 = Ev::yeni("d8-tanitim-anlam-bos");
    let kok = ev2.kok.join("b");
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &kok);
    let q = Proje { klasor: kok, ev: ev2.kok.join("ev") };
    let b = ayna(&q);
    let s = al(&b, "/api/sezgi");
    assert!(s["anlam"].is_null() && kart(&s, "etki")["durum"] == "kurulmamis", "{s}");
}

#[test]
fn ne_yapmak_istiyorsun_aramasi_etki_ve_onay_soyler() {
    let ev = Ev::yeni("d8-tanitim-ara");
    let p = ev.proje("d8_anlam_foto", "foto");
    let a = ayna(&p);
    for sorgu in ["kullanıcıyı sil", "delete user"] {
        let v = gonder(&a, "/api/ara", json!({ "sorgu": sorgu }));
        assert_eq!(v["kapilar"][0], "DELETE /api/v1/users/:user_id", "{sorgu}: {v}");
        let s = &v["sonuclar"][0];
        assert_eq!(s["id"], "DELETE /api/v1/users/:user_id");
        assert_eq!((s["risk"].as_str(), s["onay"]["karar"].as_str()), (Some("silme"), Some("her_seferinde")), "{s}");
        assert_eq!(s["onay"]["guven"], "certain", "{s}");
        assert!(s["geri"]["karar"].is_string() && s["etki"].is_array(), "{s}");
        assert!(v["plan"].is_null(), "tek eylemli istekte plan yok: {v}");
    }
    // Çok adımlı istek: plan (öneri, yürütülmez)
    let v = gonder(&a, "/api/ara", json!({ "sorgu": "kullanıcıyı sil ve albümlerini başkasına devret" }));
    assert!(v["plan"].as_str().is_some_and(|x| x.contains("DELETE /api/v1/users/:user_id")), "{v}");
    // Okuma onaysız; onaysız gidebilecek yazma yalnızca koşullu (yorum)
    let v = gonder(&a, "/api/ara", json!({ "sorgu": "albümleri listele" }));
    for s in v["sonuclar"].as_array().unwrap() {
        match s["onay"]["karar"].as_str().unwrap() {
            "gerekmez" => assert_eq!(s["risk"], "okuma", "{s}"),
            "kosullu" => assert_eq!(s["onay"]["guven"], "inferred", "{s}"),
            "her_seferinde" => assert_eq!(s["onay"]["guven"], "certain", "{s}"),
            k => panic!("bilinmeyen karar {k}"),
        }
    }
    // Başka kökenden istek reddedilir
    for yol in ["/api/ara", "/api/islem"] {
        let (d, _) = iste(&a, "POST", yol, "{}", Some("http://kotu.example"));
        assert!(d.contains("403"), "{yol}: {d}");
    }
}

#[test]
fn islem_ayrintisi_zincir_tablo_yetki_dis_geri_onay() {
    let ev = Ev::yeni("d8-tanitim-islem");
    let p = ev.proje("d8_cagri_nest", "a");
    let harita: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let a = ayna(&p);
    let sil = harita["endpointler"].as_array().unwrap().iter().find(|k| k["fonksiyon"] == "deleteAlbum").expect("deleteAlbum");
    let id = sil["id"].as_str().unwrap();
    let v = gonder(&a, "/api/islem", json!({ "kapi": id }));
    let bolum = |b: &str| v["bolumler"].as_array().unwrap().iter().find(|x| x["id"] == b).unwrap_or_else(|| panic!("{b} yok: {v:#}")).clone();
    let ids: Vec<&str> = v["bolumler"].as_array().unwrap().iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["ne", "kim", "zincir", "veri", "dis", "onkosul", "geri", "onay"], "{v:#}");
    for b in v["bolumler"].as_array().unwrap() {
        assert!(!b["maddeler"].as_array().unwrap().is_empty() || b["bilinmeyen"].is_string(), "boş bölüm nedenini söyler: {b}");
        for m in b["maddeler"].as_array().unwrap() {
            assert!(["certain", "observed", "inferred", "unknown"].contains(&m["guven"].as_str().unwrap()), "{m}");
        }
    }
    let zincir = bolum("zincir").to_string();
    assert!(zincir.contains("AlbumService.delete"), "{zincir}");
    let kim = bolum("kim").to_string();
    assert!(kim.contains("Kaydın bu kullanıcıya açık olduğu denetleniyor") && kim.contains("album.delete"), "{kim}");
    assert_eq!(v["onay"]["karar"], "her_seferinde", "{v:#}");
    assert!(bolum("onay").to_string().contains("Silme işlemi"), "{v:#}");
    // Geri alma: haritanın davranış özetiyle aynı karar
    let d = &harita["ozet"]["dna"][format!("{}:{}", sil["dosya"].as_str().unwrap(), sil["satir"]).as_str()];
    let beklenen = d["geri_alinabilir"].as_str().unwrap_or("bilinmiyor");
    assert_eq!(v["geri"]["karar"], beklenen, "{v:#}");
    assert_ne!(v["geri"]["guven"], "certain", "geri alma en fazla yorum");
    // Olmayan kapı
    assert!(gonder(&a, "/api/islem", json!({ "kapi": "GET /yok" })).is_null());
    drop(a);

    // Dış etki: ödeme sistemine giden yazma her seferinde onay ister, gerekçesiyle
    let q = ev.proje("d8_dis_express", "b");
    let a = ayna(&q);
    let v = gonder(&a, "/api/islem", json!({ "kapi": "POST /api/siparisler" }));
    let dis = v["bolumler"].as_array().unwrap().iter().find(|x| x["id"] == "dis").unwrap();
    assert!(!dis["maddeler"].as_array().unwrap().is_empty() && dis.to_string().contains("axios"), "{dis}");
    assert_eq!(v["onay"]["karar"], "her_seferinde", "{v:#}");
    assert!(v["onay"]["metin"].as_str().unwrap().contains("dışına etki"), "{v:#}");
    let v = gonder(&a, "/api/islem", json!({ "kapi": "PUT /api/siparisler/:id" }));
    let on = v["bolumler"].as_array().unwrap().iter().find(|x| x["id"] == "onkosul").unwrap();
    assert!(on["maddeler"].as_array().unwrap().iter().any(|m| m["metin"].as_str().unwrap().to_lowercase().contains("reddedilir")), "{on}");
}

/// Kurucunun 17 sorusu
const KIMLIKLER: [&str; 17] =
    ["ne", "amac", "problem", "kullanicilar", "yapabilir", "calisiyor", "ekran", "cagri", "tablolar", "onkosul", "yetki", "dis", "geri", "onay", "guven", "degisim", "gorev"];

/// Soru metinlerinde bir ifade (madde metni ya da örnek) geçiyor mu
fn geciyor(s: &Value, ifade: &str) -> bool {
    s["maddeler"].as_array().unwrap().iter().any(|m| m["metin"].as_str().unwrap().contains(ifade))
}

fn tutarlilik(p: &Proje) {
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    let a = ayna(p);
    let b = al(&a, "/api/sorular");
    let sorular = b["sorular"].as_array().unwrap();
    let ids: Vec<&str> = sorular.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids, KIMLIKLER);
    let soru = |id: &str| sorular.iter().find(|s| s["id"] == id).unwrap();
    let n = |v: &Value| v.as_u64().unwrap_or(0);
    let ad = r["uygulama"].as_str().unwrap_or("");
    let ac = |id: &str, ifade: String| assert!(geciyor(soru(id), &ifade), "{ad} soru {id}: '{ifade}' yok: {:#}", soru(id));
    // Tek üslup: madde sayıyla ya da adla başlar, nokta ile bitmez; bilinmeyen notu tam cümledir
    for s in sorular {
        for m in s["maddeler"].as_array().unwrap() {
            let t = m["metin"].as_str().unwrap();
            assert!(!t.ends_with('.') && !t.starts_with("ön koşul ·") && !t.contains("SDK"), "{ad} {}: üslup: {t}", s["id"]);
        }
        if let Some(x) = s["bilinmeyen"].as_str() {
            assert!(x.ends_with('.'), "{ad} {}: bilinmeyen tam cümle: {x}", s["id"]);
        }
    }
    // Sayılar rapor ile aynı
    let ana = n(&r["kapilar"]["ana"]);
    ac("ne", format!("{ana} işlem"));
    ac("yapabilir", format!("{ana} işlem tanımlı"));
    if n(&r["kapilar"]["rol_kisitli"]) > 0 {
        ac("yetki", format!("{} işlem rol ya da izin istiyor", n(&r["kapilar"]["rol_kisitli"])));
    }
    if let Some(y) = r["anlam"]["yetenekler"].as_array().filter(|y| !y.is_empty()) {
        ac("gorev", format!("{} iş yeteneği", y.len()));
        ac("yapabilir", format!("{} iş yeteneği", y.len()));
    }
    let c = &r["cagri"];
    if c.is_object() {
        ac("cagri", format!("{} fonksiyon ve yöntem, aralarında {} çağrı kenarı", n(&c["fonksiyon"]), n(&c["kenar"])));
        if n(&c["zincirle_tablo_bulunan_islem"]) > 0 {
            ac("tablolar", format!("{} işlemde tablo etkisi", n(&c["zincirle_tablo_bulunan_islem"])));
        }
        if n(&c["nesne_duzeyi_denetimli_islem"]) > 0 {
            ac("yetki", format!("{} işlemde nesne düzeyi yetki denetleniyor", n(&c["nesne_duzeyi_denetimli_islem"])));
        }
    }
    let d = &r["dis"];
    if d.is_object() {
        if n(&d["on_kosul"]) > 0 {
            ac("onkosul", format!("{} işlemde {} ön koşul", n(&d["on_kosullu_islem"]), n(&d["on_kosul"])));
        }
        if n(&d["dis_etkili_islem"]) > 0 {
            ac("dis", format!("{} işlem uygulamanın dışına etki ediyor", n(&d["dis_etkili_islem"])));
        }
        for (k, metin) in [("evet", "işlem geri alınabilir:"), ("kismen", "işlem kısmen geri alınabilir"), ("hayir", "işlem geri alınamaz")] {
            let x = n(&d["geri_alinabilirlik"][k]);
            if x > 0 {
                ac("geri", format!("{x} {metin}"));
            }
        }
        if n(&d["etki_yuzunden_onay"]) > 0 {
            let x = n(&d["etki_yuzunden_onay"]);
            assert!(
                soru("onay")["maddeler"].as_array().unwrap().iter().any(|m| {
                    let t = m["metin"].as_str().unwrap();
                    t.starts_with(&format!("{x} işlem")) && t.contains("uygulamanın dışına etki ediyor ya da geri alınamaz")
                }),
                "{ad}: {:#}",
                soru("onay")
            );
        }
    }
    let k = &r["kanit"];
    if k.is_object() {
        for (g, metin) in [("certain", "kesin"), ("inferred", "yorum")] {
            ac("guven", format!("{} önerme {metin}", n(&k["guven"][g])));
        }
    }
}

#[test]
fn ne_biliyoruz_rapor_ile_tutarli_ve_tek_uslupta() {
    let ev = Ev::yeni("d8-tanitim-tutarlilik");
    for (i, ornek) in ["d8_cagri_nest", "d8_dis_express", "d8_anlam_foto", "nestjs-temel", "w2dna_nest"].iter().enumerate() {
        tutarlilik(&ev.proje(ornek, &format!("p{i}")));
    }
    tutarlilik(&alt_proje(&ev, "w2g_fastapi_template", "fat", "backend"));
}

#[test]
fn sayfa_arama_kutusu_ve_metinler_tek_yerde() {
    let ev = Ev::yeni("d8-tanitim-sayfa");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    let (_, sayfa) = iste(&a, "GET", "/", "", None);
    assert!(sayfa.contains("role=\"search\"") && sayfa.contains("id=\"ara-gorev\"") && sayfa.contains("Ne yapmak istiyorsun?"), "arama kutusu");
    assert!(sayfa.contains("id=\"diger-yetenekler\""), "yardımcı yetenekler ayrı");
    let (_, js) = iste(&a, "GET", "/ayna.js", "", None);
    for parca in ["/api/islem", "/api/ara", "const M = {", "aria-live"] {
        assert!(js.contains(parca), "ayna.js içinde {parca} yok");
    }
    for yasak in ["innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval(", "new Function"] {
        assert!(!js.contains(yasak), "ayna.js içinde {yasak}");
    }
    // Ölçüm kutuları her genişlikte tanıtımın önüne geçmez
    let (_, css) = iste(&a, "GET", "/ayna.css", "", None);
    assert!(css.contains("
body[data-sekme=\"sezgi\"] .sayilar"), "Sezgi sekmesinde ölçüm kutuları her genişlikte gizli");
}

/// Süsleyici çağrısı (`@Auth() auth` parametresi) çağrı kenarı değildir: istek anında değil sınıf tanımında çalışır.
/// İşleyiciden servise giden gerçek kenar kalır; 8. sorunun örnekleri süsleyiciye giden kenar göstermez
#[test]
fn susleyici_cagri_kenari_degil() {
    let ev = Ev::yeni("d8-tanitim-suslu");
    let p = ev.proje("d8_tanitim_suslu", "a");
    let depo: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kanit.json")).unwrap()).unwrap();
    let kenarlar: Vec<(String, String)> = depo["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["yuklem"] == "CALLS" && o["varlik"].as_str().is_some_and(|v| v.ends_with("#deleteAlbum")))
        .map(|o| (o["varlik"].as_str().unwrap().to_string(), o["hedef"].as_str().unwrap_or("").to_string()))
        .collect();
    assert!(kenarlar.iter().any(|(_, h)| h.ends_with("AlbumService.delete")), "gerçek kenar kalmalı: {kenarlar:?}");
    assert!(!kenarlar.iter().any(|(_, h)| h.ends_with("#Auth")), "süsleyiciye kenar olmamalı: {kenarlar:?}");
    let a = ayna(&p);
    let b = al(&a, "/api/sorular");
    let cagri = b["sorular"].as_array().unwrap().iter().find(|s| s["id"] == "cagri").unwrap().to_string();
    assert!(!cagri.contains("→ Auth\""), "{cagri}");
}

/// İş yeteneği adı nitelendiren yol parçasından (`defaults`, `mine`) ya da dosya adından (`playlist.m3u8`) kurulmaz
#[test]
fn yetenek_adlari_anlamsiz_yol_parcasindan_kurulmaz() {
    let ev = Ev::yeni("d8-tanitim-yetenek-ad");
    let p = ev.proje("d8_tanitim_suslu", "a");
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    let adlar: Vec<&str> = r["anlam"]["yetenekler"].as_array().unwrap().iter().map(|y| y["ad"].as_str().unwrap()).collect();
    assert!(!adlar.is_empty(), "{r:#}");
    for kotu in ["default", "mine", "m3u8", "playlist"] {
        assert!(!adlar.iter().any(|a| a.contains(kotu)), "'{kotu}' yetenek adında: {adlar:?}");
    }
    // Bu işlemler yine bir yeteneğe bağlı (listeden düşmez)
    for id in ["/albums/config/defaults", "/albums/orders/mine", "/albums/:id/video/playlist.m3u8"] {
        assert!(
            r["anlam"]["yetenekler"].as_array().unwrap().iter().any(|y| y["islemler"].as_array().unwrap().iter().any(|i| i.as_str().unwrap().ends_with(id))),
            "{id} yeteneksiz kaldı: {adlar:?}"
        );
    }
}

/// Ktor uygulaması tanıtımda çatısıyla görünür; haritası çıkınca dört ana kart hazırdır
#[test]
fn ktor_catisi_kartta_gorunur() {
    let ev = Ev::yeni("d8-tanitim-ktor");
    let kok = ev.kok.join("k");
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("d8_kotlin_ktor"), &kok);
    let p = Proje { klasor: kok, ev: ev.kok.join("ev") };
    let a = ayna(&p);
    let s = al(&a, "/api/sezgi");
    assert_eq!(s["cati"], "Ktor", "{s}");
    assert!(kart(&s, "anla")["ayrinti"].as_str().unwrap().contains("Ktor"), "{s}");
    drop(a);
    p.ok(&["init", "."]);
    let a = ayna(&p);
    let s = al(&a, "/api/sezgi");
    assert_eq!(kart(&s, "anla")["durum"], "hazir", "{s}");
    assert!(s["islem"].as_u64().unwrap() > 0, "{s}");
}
