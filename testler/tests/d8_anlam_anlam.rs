//! D8 anlam: Sezgi incelediği uygulamanın ne olduğunu (iş alanı), neden var olduğunu (kendi beyanının alıntısı), hangi
//! problemi çözdüğünü, kullanıcılarını (kullanıcı tipleri, kayıt ve giriş akışları) ve iş düzeyinde yeteneklerini
//! kanıtla yanıtlar; hiçbiri `certain` değildir, kanıt yetmezse `unknown`dır. Türkçe ve İngilizce arama aynı sözlükle
//! işlemleri bulur; doğal dil görevi sıralı adımlara (işlem, rol, onay) ayrılır ve yürütülmez.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Stdio};

fn kanit(p: &Proje) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("kanit.json")).unwrap()).unwrap()
}

fn graf(p: &Proje) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join("graf.json")).unwrap()).unwrap()
}

fn onermeler<'a>(k: &'a Value, varlik: &str, yuklem: &str) -> Vec<&'a Value> {
    k["onermeler"].as_array().unwrap().iter().filter(|o| o["varlik"] == varlik && o["yuklem"] == yuklem).collect()
}

fn kanitlari<'a>(k: &'a Value, o: &Value) -> Vec<&'a Value> {
    let destek: Vec<&str> = o["hesap"]["destek"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    k["kanitlar"].as_array().unwrap().iter().filter(|x| destek.contains(&x["id"].as_str().unwrap())).collect()
}

/// Anlam önermeleri: uygulama düğümü, kullanıcı tipleri ve iş yetenekleri
fn anlam_onermeleri(k: &Value) -> Vec<&Value> {
    k["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| {
            let v = o["varlik"].as_str().unwrap();
            (v.starts_with("app:") && ["ALAN", "TUR", "AMAC", "PROBLEM", "GIRIS_AKISI"].contains(&o["yuklem"].as_str().unwrap()))
                || v.starts_with("actor:")
                || (v.starts_with("cap:") && o["yuklem"] == "VAR")
        })
        .collect()
}

#[test]
fn uygulama_nedir_neden_var_kim_kullanir() {
    let ev = Ev::yeni("d8-anlam-ne");
    let p = ev.proje("d8_anlam_foto", "foto");
    let k = kanit(&p);
    let app = "app:fotokutu";
    // 1. İş alanı: README ve tablo adlarından, kanıtlarıyla; en fazla inferred
    let alan = onermeler(&k, app, "ALAN");
    assert_eq!(alan.len(), 1, "{alan:?}");
    assert_eq!(alan[0]["deger"]["id"], "photo-video", "{}", alan[0]);
    assert_eq!(alan[0]["hesap"]["guven"], "inferred");
    let kn = kanitlari(&k, alan[0]);
    let konumlar: Vec<&str> = kn.iter().flat_map(|x| x["konum"].as_array().unwrap().iter().map(|y| y.as_str().unwrap())).collect();
    assert!(konumlar.iter().any(|x| x.starts_with("README.md:")), "README kanıtı: {konumlar:?}");
    assert!(kn.iter().any(|x| x["ozet"]["kaynak"] == "tablo"), "tablo kanıtı: {kn:?}");
    assert!(kn.iter().all(|x| x["beyan"] == "inferred"), "kanıt beyanı en fazla inferred");
    // Kurulum bölümündeki "order", "cart" sınıflandırmaya girmez
    assert!(!kn.iter().any(|x| x["ozet"]["terim"] == "cart"), "{kn:?}");
    // 2. Neden var: README'nin kendi cümlesi, alıntı olarak, dosya:satırıyla
    let amac = onermeler(&k, app, "AMAC");
    let readme = amac.iter().find(|o| o["hedef"].as_str().unwrap().starts_with("README.md:")).unwrap_or_else(|| panic!("{amac:?}"));
    assert_eq!(readme["hedef"], "README.md:5");
    assert!(readme["deger"]["alinti"].as_str().unwrap().starts_with("Fotokutu is a self-hosted photo and video library for families."), "{readme}");
    assert_eq!(readme["hesap"]["guven"], "inferred");
    assert!(amac.iter().any(|o| o["hedef"].as_str().unwrap().starts_with("pyproject.toml:") && o["deger"]["alinti"] == "Self-hosted photo and video library"), "{amac:?}");
    // 3. Problem: alanın tanımı ve belgede fayda anlatan cümle (alıntı)
    let problem = onermeler(&k, app, "PROBLEM");
    assert!(problem.iter().any(|o| o["deger"]["dayanak"] == "alan"), "{problem:?}");
    assert!(problem.iter().any(|o| o["deger"]["alinti"].as_str().is_some_and(|a| a.contains("without handing them to a cloud provider"))), "{problem:?}");
    // 4. Kullanıcılar: anonim, giriş yapmış, yönetici; kayıt ve giriş akışları
    let aktor = |id: &str| onermeler(&k, id, "VAR").first().map(|o| o["deger"].clone()).unwrap_or(Value::Null);
    assert!(aktor("actor:anonymous")["ornekler"].as_array().unwrap().iter().any(|x| x == "POST /api/v1/users/signup"), "{}", aktor("actor:anonymous"));
    assert_eq!(aktor("actor:admin")["ornekler"], json!(["DELETE /api/v1/users/:user_id"]));
    assert!(aktor("actor:user")["islem_sayisi"].as_u64().unwrap() >= 5);
    assert_eq!(onermeler(&k, app, "GIRIS_AKISI").iter().map(|o| o["hedef"].as_str().unwrap()).collect::<Vec<_>>().len(), 2);
    assert_eq!(onermeler(&k, app, "GIRIS_AKISI").iter().find(|o| o["hedef"] == "kayit").unwrap()["deger"]["islemler"], json!(["POST /api/v1/users/signup"]));
    assert_eq!(onermeler(&k, app, "GIRIS_AKISI").iter().find(|o| o["hedef"] == "giris").unwrap()["deger"]["islemler"], json!(["POST /api/v1/login/access-token"]));
    // 5. İş yetenekleri: işlemler iş diline gruplanır; uç IMPLEMENTS yetenek kenarıyla bağlanır
    let paylas = onermeler(&k, "cap:album.share", "VAR");
    assert_eq!(paylas[0]["deger"]["ad"], "albüm paylaş");
    assert_eq!(paylas[0]["deger"]["islemler"], json!(["PUT /api/v1/albums/:id/users"]));
    assert_eq!(onermeler(&k, "cap:photo.upload", "VAR")[0]["deger"]["ad"], "fotoğraf yükle");
    let g = graf(&p);
    assert!(
        g["kenarlar"].as_array().unwrap().iter().any(|e| e["kaynak"] == "api:PUT /api/v1/albums/:id/users" && e["iliski"] == "IMPLEMENTS" && e["hedef"] == "cap:album.share" && e["guven"] == "inferred"),
        "IMPLEMENTS kenarı"
    );
    // Hiçbir anlam önermesi certain ya da observed değildir
    for o in anlam_onermeleri(&k) {
        assert!(matches!(o["hesap"]["guven"].as_str(), Some("inferred" | "unknown")), "{o}");
    }
}

#[test]
fn kanit_yetmezse_alan_bilinmiyor() {
    let ev = Ev::yeni("d8-anlam-bilinmiyor");
    // Kitap ve raf: ne wiki ne kitapçı diye sınıflandırılmaz (README yok, tek kaynak türü)
    for ornek in ["nestjs-temel", "fastapi-temel"] {
        let p = ev.proje(ornek, ornek);
        let k = kanit(&p);
        let alan: Vec<&Value> = k["onermeler"].as_array().unwrap().iter().filter(|o| o["varlik"].as_str().unwrap().starts_with("app:") && o["yuklem"] == "ALAN").collect();
        assert_eq!(alan.len(), 1, "{ornek}");
        assert_eq!(alan[0]["hesap"]["guven"], "unknown", "{ornek}: {}", alan[0]);
        assert!(alan[0]["notlar"][0].as_str().unwrap().contains("sınıflandırılamadı"), "{}", alan[0]);
        let mut m = p.mcp("");
        let (t, _) = m.arac("kimsin", json!({}));
        assert!(t.contains("Ne: bilinmiyor [Bilinmiyor]"), "{t}");
        let rapor = p.ok(&["rapor"]);
        assert!(rapor.contains("ANLAM        [bilinmiyor] alan:"), "{rapor}");
        assert!(rapor.contains("[bilinmiyor] amaç"), "{rapor}");
    }
}

#[test]
fn kart_ve_kimsin_anlami_tasir() {
    let ev = Ev::yeni("d8-anlam-kart");
    let p = ev.proje("d8_anlam_foto", "foto");
    let kart = p.kart();
    // Şema `app` altında x- üyelerine izin verir; damgalar en fazla inferred
    let x = &kart["app"]["x-meaning"];
    assert_eq!(x["domain"]["id"], "photo-video", "{x}");
    assert_eq!(x["domain"]["stamp"], "inferred");
    assert!(x["domain"]["evidence"].as_array().unwrap().iter().any(|e| e["file"] == "README.md"), "{x}");
    assert_eq!(x["stamp_ceiling"], "inferred");
    assert!(x["capabilities"].as_array().unwrap().iter().any(|c| c["id"] == "cap:album.share" && c["endpoints"] == json!(["PUT /api/v1/albums/:id/users"])));
    assert!(x["actors"].as_array().unwrap().iter().any(|a| a["id"] == "actor:admin"));
    assert!(x["auth_flows"].as_array().unwrap().iter().any(|a| a["kind"] == "kayit"));
    for v in [&x["purpose"], &x["problem"], &x["actors"], &x["auth_flows"], &x["capabilities"]] {
        for i in v.as_array().unwrap() {
            assert!(i["stamp"] == "inferred" || i["stamp"] == "unknown", "{i}");
        }
    }
    // Uygulamanın kendi açıklaması kartın `app.description` alanına alıntı olarak (stampedText, inferred)
    assert_eq!(kart["app"]["description"]["stamp"], "inferred");
    assert_eq!(kart["app"]["description"]["sources"][0], json!({ "file": "README.md", "line_start": 5 }));
    // İmzalı kart kendi doğrulayıcısından geçer (x-meaning imzanın kapsamında)
    p.ok(&["anahtar", "olustur", "--ad", "varsayilan"]);
    p.ok(&["init", ".", "--imzala"]);
    p.ok(&["dogrula", ".sezgi/benlik.json", ".sezgi/benlik-keys.json"]);
    // MCP kimsin: ne, neden (alıntı), problem, kullanıcılar, akışlar, yetenekler; damga Yorum
    let mut m = p.mcp("");
    let (t, hata) = m.arac("kimsin", json!({}));
    assert!(!hata);
    for parca in [
        "Ne: fotoğraf ve video arşivi [Yorum]",
        "Neden (README.md:5 alıntısı) [Yorum]: \"Fotokutu is a self-hosted photo and video library for families.",
        "Problem: Fotoğraf ve videoları tek yerde saklamak",
        "Kullanıcılar [Yorum]: anonim ziyaretçi",
        "kayıt (POST /api/v1/users/signup)",
        "albüm paylaş",
        "gorev=true",
    ] {
        assert!(t.contains(parca), "{parca} yok:\n{t}");
    }
    assert!(!t.contains("[Kesin]: \"") && !t.contains("Ne: fotoğraf ve video arşivi [Kesin]"), "{t}");
    // İngilizce
    let mut c = p.komut(&["mcp", ".sezgi", "--lang", "en"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "app", "arguments": {} } });
    writeln!(c.stdin.take().unwrap(), "{istek}").unwrap();
    let cikti = String::from_utf8_lossy(&c.wait_with_output().unwrap().stdout).to_string();
    assert!(cikti.contains("What it is: photo and video library [Inferred]"), "{cikti}");
    // Rapor bölümü
    let r = p.ok(&["rapor"]);
    assert!(r.contains("ANLAM        [yorum] alan: fotoğraf ve video arşivi"), "{r}");
    assert!(r.contains("[yorum] amaç (README.md:5 alıntısı)"), "{r}");
    assert!(r.contains("iş yetenekleri:"), "{r}");
}

#[test]
fn turkce_ve_ingilizce_arama() {
    let ev = Ev::yeni("d8-anlam-ara");
    let p = ev.proje("d8_anlam_foto", "foto");
    let mut m = p.mcp("");
    let ilk = |m: &mut Mcp, q: &str| -> (String, bool) {
        let (t, h) = m.arac("ara", json!({ "sorgu": q, "adet": 3 }));
        (t.lines().find(|l| !l.starts_with('(')).unwrap_or("").to_string(), h)
    };
    for (q, beklenen) in [
        ("kullanıcı sil", "DELETE /api/v1/users/:user_id"),
        ("kullanıcıyı sil", "DELETE /api/v1/users/:user_id"),
        ("fotoğraf yükle", "POST /api/v1/photos/"),
        ("albüm paylaş", "PUT /api/v1/albums/:id/users"),
        ("albüm oluştur", "POST /api/v1/albums/"),
        ("delete user", "DELETE /api/v1/users/:user_id"),
        ("upload photo", "POST /api/v1/photos/"),
        ("kayıt ol", "POST /api/v1/users/signup"),
    ] {
        let (l, h) = ilk(&mut m, q);
        assert!(!h && l.starts_with(beklenen), "{q}: {l}");
    }
    // Uygulamada olmayan varlık: fiilden sonuç uydurulmaz
    let (t, h) = m.arac("ara", json!({ "sorgu": "sipariş oluştur" }));
    assert!(h && t.contains("bulunamadı"), "{t}");
    // İngilizce istek Türkçe adlı kodda
    let q = ev.proje("fastapi-temel", "kitap");
    let mut m2 = q.mcp("");
    let (t, _) = m2.arac("ara", json!({ "sorgu": "delete book", "adet": 1 }));
    assert!(t.starts_with("DELETE /api/kitaplar/:kitap_id"), "{t}");
    let (t, _) = m2.arac("ara", json!({ "sorgu": "kitap oluştur", "adet": 1 }));
    assert!(t.starts_with("POST /api/kitaplar"), "{t}");
    // Çok adımlı istek: ara plan önerir
    let (t, _) = m.arac("ara", json!({ "sorgu": "kullanıcıyı sil ve albümlerini başkasına devret" }));
    assert!(t.contains("gorev: true"), "{t}");
}

#[test]
fn dogal_dil_gorevi_plan() {
    let ev = Ev::yeni("d8-anlam-plan");
    let p = ev.proje("d8_anlam_foto", "foto");
    let mut m = p.mcp("");
    let (t, hata) = m.arac("ara", json!({ "sorgu": "kullanıcıyı sil ve albümlerini başkasına devret", "gorev": true }));
    assert!(!hata, "{t}");
    assert!(t.starts_with("GÖREV PLANI [Yorum]") && t.contains("YÜRÜTÜLMEZ"), "{t}");
    // Sıra: devret (listele + sahibini değiştir) önce, silme en sonda
    let i_liste = t.find("GET /api/v1/albums/").expect(&t);
    let i_sahip = t.find("PATCH /api/v1/albums/:id").expect(&t);
    let i_sil = t.find("DELETE /api/v1/users/:user_id").expect(&t);
    assert!(i_liste < i_sahip && i_sahip < i_sil, "{t}");
    assert!(t.contains("Sıra değişti"), "{t}");
    assert!(t.contains("sahibini değiştir (gövdede owner alanı)"), "{t}");
    assert!(t.contains("devralacak kullanıcı belirtilmeli"), "{t}");
    // İzin ve onay: silme yıkıcı, süper kullanıcı ister
    assert!(t.contains("rol/izin: superuser") && t.contains("insan onayı gerekir (yıkıcı işlem)"), "{t}");
    assert!(t.contains("Gereken rol/izinler: superuser"), "{t}");
    // Uygulamada karşılığı olmayan adım dürüstçe yazılır
    let (t, _) = m.arac("ara", json!({ "sorgu": "sipariş oluştur ve öde", "gorev": true }));
    assert!(t.contains("bu uygulamada 'sipariş' ile ilgili işlem bulunamadı"), "{t}");
    // İngilizce argüman adı
    let (t, _) = m.arac("search", json!({ "query": "delete the user and then upload a photo", "task": true }));
    assert!(t.contains("POST /api/v1/photos/") && t.contains("DELETE /api/v1/users/:user_id"), "{t}");
}

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

#[test]
fn ayna_ne_biliyoruz_ve_turkce_arama() {
    let ev = Ev::yeni("d8-anlam-ayna");
    let p = ev.proje("d8_anlam_foto", "foto");
    let a = ayna(&p);
    // Türkçe arama: sunucu sözlükle sıralar
    let (d, g) = iste(&a, "POST", "/api/ara", &json!({ "sorgu": "kullanıcıyı sil" }).to_string(), None);
    assert!(d.contains("200"), "{d} {g}");
    let v: Value = serde_json::from_str(&g).unwrap();
    assert_eq!(v["kapilar"][0], "DELETE /api/v1/users/:user_id", "{v}");
    // Başka kökenden arama isteği reddedilir
    let (d, _) = iste(&a, "POST", "/api/ara", "{}", Some("http://kotu.example"));
    assert!(d.contains("403"), "{d}");
    // "Ne biliyoruz": 1, 2, 3, 4, 5 ve 17 bu uygulamanın verisinden
    let (d, g) = iste(&a, "GET", "/api/sorular", "", None);
    assert!(d.contains("200"));
    let b: Value = serde_json::from_str(&g).unwrap();
    let soru = |id: &str| b["sorular"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap().clone();
    let ne = soru("ne").to_string();
    assert!(ne.contains("İş alanı: fotoğraf ve video arşivi") && ne.contains("README README.md:"), "{ne}");
    let amac = soru("amac");
    assert_eq!(amac["durum"], "kismen", "{amac}");
    assert_eq!(amac["guven"], "inferred");
    assert!(amac.to_string().contains("README.md:5 alıntısı: “Fotokutu is a self-hosted photo and video library"), "{amac}");
    let problem = soru("problem").to_string();
    assert!(problem.contains("(iş alanından)") && problem.contains("cloud provider"), "{problem}");
    let kullanicilar = soru("kullanicilar").to_string();
    assert!(kullanicilar.contains("Kullanıcı tipi: yönetici (1 işlem)") && kullanicilar.contains("Akış: kayıt"), "{kullanicilar}");
    let yapabilir = soru("yapabilir").to_string();
    assert!(yapabilir.contains("iş yeteneği") && yapabilir.contains("albüm paylaş"), "{yapabilir}");
    let gorev = soru("gorev");
    assert!(gorev.to_string().contains("Örnek: “"), "{gorev}");
    assert_eq!(gorev["durum"], "kismen");
    assert!(gorev["bilinmeyen"].as_str().unwrap().contains("yürütmez"), "{gorev}");
    // Hiçbir anlam yanıtı kesin değildir
    for id in ["amac", "problem", "gorev"] {
        assert_ne!(soru(id)["guven"], "certain", "{id}");
    }
}

#[test]
fn token_gormedigi_yetenegi_kimsin_gostermez() {
    let ev = Ev::yeni("d8-anlam-token");
    let p = ev.proje("d8_anlam_foto", "foto");
    let tok = p.token("okur", &["*:oku"], &[]);
    let mut m = p.mcp(&tok);
    let (t, _) = m.arac("kimsin", json!({}));
    assert!(!t.contains("albüm paylaş"), "gizli işlemin yeteneği görünmemeli:\n{t}");
    // Plan da yalnızca görünen işlemlerle kurulur
    let (t, _) = m.arac("ara", json!({ "sorgu": "kullanıcıyı sil ve albümlerini başkasına devret", "gorev": true }));
    assert!(!t.contains("DELETE /api/v1/users/:user_id"), "{t}");
}
