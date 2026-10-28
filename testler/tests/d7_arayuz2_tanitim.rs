//! D7 arayüz 2: Ayna tanıtımının dürüstlüğü. "Ne biliyoruz" kurucunun 17 sorusunu kendi sırasıyla sorar (zayıf sorular
//! saklanmaz, veri yoksa nedeniyle "bilinmiyor"); "onaysız gidebilir" listesi canlı modun kullandığı önizleme
//! kurallarıyla aynı kaynaktan hesaplanır ve kesin damga taşımaz; tanıtım metni politikayı abartmaz; ekran adları ve
//! önizleme koşulları düz Türkçe; dar ekranda sekmeler sarılır, ölçüm kutuları tanıtımın önüne geçmez.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
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

fn iste(port: u16, yontem: &str, yol: &str, anahtar: Option<&str>, govde: &str) -> (String, String) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let ek = anahtar.map(|a| format!("X-Sezgi-Anahtar: {a}\r\n")).unwrap_or_default();
    write!(s, "{yontem} {yol} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{ek}\r\n{govde}", govde.len()).unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    let (d, g) = c.split_once("\r\n\r\n").unwrap_or((&c, ""));
    (d.lines().next().unwrap_or("").to_string(), g.to_string())
}

fn json(a: &Ayna, yol: &str) -> Value {
    let (d, g) = iste(a.port, "GET", yol, Some(&a.anahtar), "");
    assert!(d.contains("200"), "{yol}: {d} {g}");
    serde_json::from_str(&g).unwrap()
}

fn onizle(a: &Ayna, istek: Value) -> String {
    let (d, g) = iste(a.port, "POST", "/api/onizle", Some(&a.anahtar), &istek.to_string());
    assert!(d.contains("200"), "{d} {g}");
    let v: Value = serde_json::from_str(&g).unwrap();
    v["metin"].as_str().unwrap_or("").to_string()
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

/// Örnek uygulamanın alt klasöründe haritalanan proje (ör. fastapi-template'in backend'i); `init` yoksa haritasız
fn proje(ev: &Ev, ornek: &str, ad: &str, alt: &str, init: bool) -> Proje {
    let kok = ev.kok.join(ad);
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ornek), &kok);
    let p = Proje { klasor: if alt.is_empty() { kok } else { kok.join(alt) }, ev: ev.kok.join("ev") };
    if init {
        p.ok(&["init", "."]);
    }
    p
}

/// Kurucunun 17 sorusu, kendi sırasıyla
const KIMLIKLER: [&str; 17] =
    ["ne", "amac", "problem", "kullanicilar", "yapabilir", "calisiyor", "ekran", "cagri", "tablolar", "onkosul", "yetki", "dis", "geri", "onay", "guven", "degisim", "gorev"];

fn soru<'a>(b: &'a Value, id: &str) -> &'a Value {
    b["sorular"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap_or_else(|| panic!("{id} yok: {b}"))
}

fn on_yedi_soru(b: &Value) {
    let sorular = b["sorular"].as_array().unwrap();
    let ids: Vec<&str> = sorular.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids, KIMLIKLER, "17 soru, kurucunun sırasıyla");
    for (i, s) in sorular.iter().enumerate() {
        assert_eq!(s["no"], i + 1, "{s}");
    }
    assert_eq!(soru(b, "amac")["soru"], "Neden var?");
    assert_eq!(soru(b, "problem")["soru"], "Hangi iş problemini çözer?");
    assert_eq!(soru(b, "cagri")["soru"], "Hangi fonksiyonlar birbirini çağırır?");
    assert_eq!(soru(b, "gorev")["soru"], "Doğal dil görevi için hangi yetenekler gerekir?");
    // Zayıf sorular saklanmaz: yanıtı eksikse nedeni yazılı
    for id in ["amac", "problem", "cagri", "gorev"] {
        let s = soru(b, id);
        assert_ne!(s["durum"], "yanitli", "{s}");
        assert!(s["bilinmeyen"].as_str().is_some_and(|x| x.chars().count() > 30), "nedeni: {s}");
    }
}

#[test]
fn ne_biliyoruz_on_yedi_soru_sorar() {
    let ev = Ev::yeni("d7-arayuz2-17");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    let b = json(&a, "/api/sorular");
    on_yedi_soru(&b);
    // Fonksiyon çağrıları: fonksiyonlar arası çağrı grafiği yok; varsa işlem → işleyici bağı gösterilir, kesin değilse dürüstçe
    let cagri = soru(&b, "cagri");
    assert!(cagri["bilinmeyen"].as_str().unwrap().contains("fonksiyon"), "{cagri}");
    // Doğal dil görevi: planlama yok, anahtar kelimeli arama önerilir
    let gorev = soru(&b, "gorev").to_string();
    assert!(gorev.contains("ara"), "{gorev}");
    drop(a);

    let q = proje(&ev, "nestjs-temel", "b", "", false);
    let a = ayna(&q);
    let b = json(&a, "/api/sorular");
    assert_eq!(b["harita"], false);
    on_yedi_soru(&b);
    for s in b["sorular"].as_array().unwrap() {
        assert!(s["durum"] == "bilinmiyor" && s["komut"]["komut"] == "sezgi init", "{s}");
    }
}

/// "Onaysız gidebilir" listesi: canlı modun önizleme kuralıyla aynı kaynaktan. Önizlemesi hiçbir zaman bütün kuralları
/// denetleyemeyen yazma (sahiplik, servis içi kural) listede yer almaz; liste kesin damga taşımaz.
#[test]
fn onaysiz_listesi_abartmaz() {
    let ev = Ev::yeni("d7-arayuz2-onay");
    let p = proje(&ev, "w2g_fastapi_template", "a", "backend", true);
    let harita: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let a = ayna(&p);
    let b = json(&a, "/api/sorular");
    let onay = soru(&b, "onay");
    let maddeler = onay["maddeler"].as_array().unwrap();
    for m in maddeler {
        let t = m["metin"].as_str().unwrap();
        if t.contains("onaysız") {
            assert_eq!(m["guven"], "inferred", "onaysız iddiası en fazla yorum: {m}");
            assert!(t.contains("koşullar sağlanırsa") && t.contains("her istekte"), "{m}");
        }
    }
    let serbest: Vec<&str> =
        maddeler.iter().filter(|m| m["metin"].as_str().unwrap().contains("onaysız")).flat_map(|m| m["ornekler"].as_array().unwrap().iter().map(|o| o.as_str().unwrap())).collect();
    let onizlemeli = maddeler.iter().find(|m| m["metin"].as_str().unwrap().contains("önizleme")).unwrap_or_else(|| panic!("önizleme gerekçeli madde yok: {onay}"));
    let hep_onay: Vec<&str> = onizlemeli["ornekler"].as_array().unwrap().iter().map(|o| o.as_str().unwrap()).collect();
    for id in ["PUT /api/v1/items/:id", "PATCH /api/v1/users/me"] {
        assert!(!serbest.contains(&id), "{id} onaysız listesinde olmamalı: {onay}");
    }
    assert!(hep_onay.contains(&"PUT /api/v1/items/:id"), "{onay}");
    // Aynı kaynak: "her istekte onaya düşer" denen her işlemin önizlemesi, insan oturumla ve kapının rolüyle gelse bile geçmez
    let kapilar = harita["endpointler"].as_array().unwrap();
    for id in &hep_onay {
        let k = kapilar.iter().find(|k| k["id"] == *id).unwrap();
        let m = onizle(&a, serde_json::json!({ "kapi": id, "oturum": true, "roller": k["roller"] }));
        assert!(!m.contains("GEÇER (bilinen bütün kurallar)"), "{id}: {m}");
    }
}

#[test]
fn tanitim_metni_politikayi_abartmaz() {
    let ev = Ev::yeni("d7-arayuz2-metin");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    let (_, sayfa) = iste(a.port, "GET", "/", None, "");
    assert!(!sayfa.contains("tek bir kuraldan") && !sayfa.contains("kanıtın düzeyine bağlı"), "abartılı cümle kaldı");
    for parca in ["politika denetiminden", "önizleme", "token", "güven damgası", "insan onayı"] {
        assert!(sayfa.contains(parca), "tanıtımda '{parca}' yok");
    }
}

/// Ekran adları iç biçimde (`web//items`) değil, kullanıcının gördüğü yol olarak
#[test]
fn ekran_adlari_duz() {
    let ev = Ev::yeni("d7-arayuz2-ekran");
    let p = proje(&ev, "w2g_fastapi_template", "a", "backend", true);
    let a = ayna(&p);
    let b = json(&a, "/api/sorular");
    let ekran = soru(&b, "ekran");
    let ornekler: Vec<&str> =
        ekran["maddeler"].as_array().unwrap().iter().flat_map(|m| m["ornekler"].as_array().unwrap().iter().map(|o| o.as_str().unwrap())).collect();
    assert!(!ornekler.is_empty(), "ekran bağı bulunmalı: {ekran}");
    for o in &ornekler {
        assert!(!o.contains("//") && !o.starts_with("web/") && !o.contains("screen:"), "iç biçimli ekran adı: {o}");
    }
    assert!(ornekler.iter().any(|o| o.starts_with("/items ekranı")), "{ornekler:?}");
    let ne = soru(&b, "ne").to_string();
    assert!(!ne.contains("web//"), "{ne}");
}

/// MCP aracının sonucu (token'sız gözlem modu)
fn arac(p: &Proje, ad: &str, arguman: Value) -> String {
    let mut c = p.komut(&["mcp", ".sezgi"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let istek = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": arguman } });
    writeln!(c.stdin.take().unwrap(), "{istek}").unwrap();
    let cikti = String::from_utf8_lossy(&c.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(cikti.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

/// Red koşulu kod parçası (`except`) olarak değil düz Türkçeyle; kaynak satırı kalır
#[test]
fn red_kosulu_duz_turkce() {
    let ev = Ev::yeni("d7-arayuz2-kosul");
    let p = proje(&ev, "w2g_fastapi_template", "a", "backend", true);
    let harita: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    assert!(harita["ozet"]["dna"].to_string().contains("\"except\""), "örnekte except koşulu olmalı");
    let t = arac(&p, "tani", serde_json::json!({ "kapi": "DELETE /api/v1/users/me" }));
    assert!(!t.contains("`except`"), "{t}");
    assert!(t.contains("red 403") && t.contains("hata yakalanınca") && t.contains("deps.py:37"), "{t}");
}

#[test]
fn dar_ekranda_sekmeler_sarilir_olcumler_one_gecmez() {
    let ev = Ev::yeni("d7-arayuz2-dar");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    let (_, css) = iste(a.port, "GET", "/ayna.css", None, "");
    let dar = css.split("@media (max-width: 600px)").nth(1).expect("dar ekran kuralı");
    assert!(dar.contains("flex-wrap: wrap"), "sekme çubuğu dar ekranda sarılmalı");
    assert!(css.contains("body[data-sekme=\"sezgi\"] .sayilar"), "Sezgi sekmesinde ölçümler dar ekranda geri çekilmeli");
    let (_, js) = iste(a.port, "GET", "/ayna.js", None, "");
    assert!(js.contains("document.body.dataset.sekme"), "seçili sekme gövdeye yazılmalı");
    let (_, sayfa) = iste(a.port, "GET", "/", None, "");
    assert!(sayfa.contains("<body data-sekme=\"sezgi\">"), "açılışta gövde Sezgi sekmesini bilmeli");
}

/// `sezgi ayna --adres` haritasız açılıp sonradan haritası çıkarılan Ayna'yı gerçek durumuyla listeler (kayıt bayat
/// etiket taşımaz)
#[test]
fn adres_listesi_haritanin_gercek_durumunu_soyler() {
    let ev = Ev::yeni("d7-arayuz2-adres");
    let p = proje(&ev, "nestjs-temel", "a", "", false);
    let a = ayna(&p);
    let liste = p.ok(&["ayna", "--adres"]);
    let satir = liste.lines().find(|l| l.contains(&format!(":{}/", a.port))).unwrap_or_else(|| panic!("{liste}"));
    assert!(satir.contains("(harita yok)"), "{satir}");
    p.ok(&["init", "."]);
    let liste = p.ok(&["ayna", "--adres"]);
    let satir = liste.lines().find(|l| l.contains(&format!(":{}/", a.port))).unwrap_or_else(|| panic!("{liste}"));
    assert!(!satir.contains("harita yok") && satir.contains(".sezgi"), "init sonrası etiket güncel olmalı: {satir}");
    // Ayna da haritayı yüklemiş olmalı
    assert_eq!(json(&a, "/api/sezgi")["harita"], true);
}

/// Koddaki YorumDto ile çelişen başarılı istek (ağ kaydı): işlemin kanıtı çelişkili olur
const CELISKI_HAR: &str = r#"{"log":{"version":"1.2","entries":[
{"startedDateTime":"2026-10-25T10:00:00Z","request":{"method":"POST","url":"http://127.0.0.1:8001/api/v1/yorumlar","headers":[{"name":"Content-Type","value":"application/json"}],"queryString":[],"postData":{"mimeType":"application/json","text":"{\"puan\":\"cok\"}"}},"response":{"status":201,"headers":[],"content":{"mimeType":"application/json","text":"{\"id\":\"1\"}"}}}
]}}"#;

/// Canlı modun politikası çelişkili kanıtla onaysız yazmaz: Ayna da aynı kaynaktan bu işlemi "onaysız gidebilir"
/// listesinde göstermez, gerekçesiyle onay isteyenlerde gösterir
#[test]
fn celiskili_kanitli_islem_onaysiz_listesinde_yok() {
    let ev = Ev::yeni("d7-arayuz2-celiski");
    let p = ev.proje("nestjs-temel", "a");
    let serbest = |b: &Value| -> Vec<String> {
        soru(b, "onay")["maddeler"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["metin"].as_str().unwrap().contains("onaysız"))
            .flat_map(|m| m["ornekler"].as_array().unwrap().iter().map(|o| o.as_str().unwrap().to_string()))
            .collect()
    };
    let a = ayna(&p);
    assert!(serbest(&json(&a, "/api/sorular")).iter().any(|x| x == "POST /api/v1/yorumlar"), "kurulum: çelişkisizken onaysız gidebilir");
    drop(a);
    std::fs::write(p.klasor.join("celiski.har"), CELISKI_HAR).unwrap();
    p.ok(&["gozlem", "al", "celiski.har"]);
    let a = ayna(&p);
    let b = json(&a, "/api/sorular");
    assert!(!serbest(&b).iter().any(|x| x == "POST /api/v1/yorumlar"), "{}", soru(&b, "onay"));
    let celiski = soru(&b, "onay")["maddeler"].as_array().unwrap().iter().find(|m| m["metin"].as_str().unwrap().contains("çelişkili")).cloned();
    let celiski = celiski.unwrap_or_else(|| panic!("çelişki gerekçeli madde yok: {}", soru(&b, "onay")));
    assert!(celiski["ornekler"].as_array().unwrap().iter().any(|o| o == "POST /api/v1/yorumlar"), "{celiski}");
}

/// Gerçek veride yazma deneysel: "Güvenle iş yaptır" kartı bunu söyler
#[test]
fn canli_karti_gercek_veride_deneysel_der() {
    let ev = Ev::yeni("d7-arayuz2-deneysel");
    let p = ev.proje("nestjs-temel", "a");
    let uyg = Uygulama::yeni();
    p.ok(&["hedef", &uyg.adres(), "--test-kopyasi"]);
    p.ok(&["mod", "yazma"]);
    p.token("t", &["kitap.listele:oku"], &[]);
    let k = p.ev.join("ayar").join("sezgi").join("uygulamalar");
    let durum: Vec<PathBuf> = std::fs::read_dir(&k).unwrap().flatten().map(|g| g.path()).filter(|p| p.is_dir()).collect();
    let yol = durum[0].join("ayar.json");
    let mut ayar: Value = serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
    ayar["hedef"] = serde_json::json!({ "url": uyg.adres(), "test_kopyasi": false, "gercek_veri": true });
    ayar["gercek_adresler"] = serde_json::json!([uyg.adres()]);
    std::fs::write(&yol, ayar.to_string()).unwrap();
    let a = ayna(&p);
    let s = json(&a, "/api/sezgi");
    let canli = s["yetenekler"].as_array().unwrap().iter().find(|x| x["id"] == "canli").unwrap().clone();
    assert!(canli["ayrinti"].as_str().unwrap().contains("deneysel"), "{canli}");
    assert!(canli["aciklama"].as_str().unwrap().contains("deneysel"), "{canli}");
}
