//! 0.2 yazma hattı uçtan uca: mod, hedef (test kopyası), token izinleri, insan oturumu, önizleme, tekrar koruması,
//! onay kuyruğu ve zincirli kayıt. Uygulama yerine istekleri kaydeden küçük bir HTTP sunucusu kullanılır.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
struct Gelen {
    yontem: String,
    yol: String,
    yetki: Option<String>,
    govde: String,
}

/// İstekleri kaydeden sahte uygulama (her isteğe 201 + JSON)
fn sahte_uygulama() -> (u16, Arc<Mutex<Vec<Gelen>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let gelenler = Arc::new(Mutex::new(vec![]));
    let g2 = gelenler.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut ilk = String::new();
            r.read_line(&mut ilk).unwrap();
            let (mut boy, mut yetki) = (0usize, None);
            loop {
                let mut l = String::new();
                r.read_line(&mut l).unwrap();
                let l = l.trim_end().to_string();
                if l.is_empty() {
                    break;
                }
                let (k, v) = l.split_once(':').unwrap();
                match k.to_ascii_lowercase().as_str() {
                    "content-length" => boy = v.trim().parse().unwrap(),
                    "authorization" => yetki = Some(v.trim().to_string()),
                    _ => {}
                }
            }
            let mut govde = vec![0u8; boy];
            r.read_exact(&mut govde).unwrap();
            let p: Vec<&str> = ilk.split_whitespace().collect();
            g2.lock().unwrap().push(Gelen { yontem: p[0].into(), yol: p[1].into(), yetki, govde: String::from_utf8_lossy(&govde).into() });
            let cevap = "{\"id\":\"k1\"}";
            let mut s = s;
            write!(s, "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cevap}", cevap.len()).unwrap();
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    (port, gelenler)
}

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
    port: u16,
    gelenler: Arc<Mutex<Vec<Gelen>>>,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-canli-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let ev = kok.join("ev");
        std::fs::create_dir_all(ev.join("ayar")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &kok.join("app"));
        let (port, gelenler) = sahte_uygulama();
        let o = Ortam { kok, ev, port, gelenler };
        assert!(o.sezgi(&["init", "."], None).status.success());
        o
    }
    fn komut(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args)
            .current_dir(self.kok.join("app"))
            .env("HOME", &self.ev)
            .env("USERPROFILE", &self.ev)
            .env("APPDATA", self.ev.join("ayar"))
            .env("XDG_CONFIG_HOME", self.ev.join("ayar"))
            .env_remove("SEZGI_TOKEN")
            // Oturum anahtarlığa değil dosyaya: testler gerçek kimlik deposuna yazmaz
            .env("SEZGI_ANAHTARLIK", "kapali");
        c
    }
    fn sezgi(&self, args: &[&str], girdi: Option<&str>) -> Output {
        let mut c = self.komut(args);
        c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut p = c.spawn().unwrap();
        if let Some(g) = girdi {
            p.stdin.as_mut().unwrap().write_all(g.as_bytes()).unwrap();
        }
        drop(p.stdin.take());
        p.wait_with_output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let c = self.sezgi(args, None);
        let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
        assert!(c.status.success(), "{args:?}: {m}");
        m
    }
    /// Bekleyen isteğin `onay goster` çıktısındaki içerik özetinin ilk 8 karakteri (terminal yokken onay buna bağlanır)
    fn onay_ozeti(&self, id: &str) -> String {
        let g = self.ok(&["onay", "goster", id]);
        g.split("Özet: ").nth(1).unwrap_or_else(|| panic!("{g}")).chars().take(8).collect()
    }
    /// Test kopyası, yazma modu, oturum ve verilen izinlerle token; token döner
    fn kur(&self, izinler: &[&str]) -> String {
        self.ok(&["hedef", &format!("http://127.0.0.1:{}", self.port), "--test-kopyasi"]);
        self.ok(&["mod", "yazma"]);
        // JWT biçiminde oturum: yükteki e-posta kayda "insan" olarak geçer
        let c = self.sezgi(&["oturum", "ac"], Some("eyJhbGciOiJub25lIn0.eyJlbWFpbCI6Imtpc2lAb3JuZWsudGVzdCJ9.x\n"));
        assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
        let mut a = vec!["token", "olustur", "--ad", "ajan"];
        for i in izinler {
            a.extend(["--izin", i]);
        }
        let m = self.ok(&a);
        m.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string()
    }
    fn mcp(&self, token: Option<&str>) -> Mcp {
        self.mcp_ortamli(token, &[])
    }
    fn mcp_ortamli(&self, token: Option<&str>, ortam: &[(&str, &str)]) -> Mcp {
        let mut c = self.komut(&["mcp", ".sezgi"]);
        if let Some(t) = token {
            c.env("SEZGI_TOKEN", t);
        }
        for (k, v) in ortam {
            c.env(k, v);
        }
        let mut p = c.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let stdin = p.stdin.take().unwrap();
        let satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
        Mcp { p, stdin, satirlar: Box::new(satirlar), id: 0 }
    }
    fn gelen_sayisi(&self) -> usize {
        self.gelenler.lock().unwrap().len()
    }
}

struct Mcp {
    p: Child,
    stdin: ChildStdin,
    satirlar: Box<dyn Iterator<Item = std::io::Result<String>>>,
    id: u64,
}

impl Mcp {
    fn arac(&mut self, ad: &str, a: serde_json::Value) -> (String, bool) {
        self.id += 1;
        let istek = serde_json::json!({ "jsonrpc": "2.0", "id": self.id, "method": "tools/call", "params": { "name": ad, "arguments": a } });
        writeln!(self.stdin, "{istek}").unwrap();
        self.stdin.flush().unwrap();
        let v: serde_json::Value = serde_json::from_str(&self.satirlar.next().unwrap().unwrap()).unwrap();
        (v["result"]["content"][0]["text"].as_str().unwrap().to_string(), v["result"]["isError"] == true)
    }
    /// tools/list
    fn liste(&mut self) -> Vec<serde_json::Value> {
        self.id += 1;
        writeln!(self.stdin, "{}", serde_json::json!({ "jsonrpc": "2.0", "id": self.id, "method": "tools/list" })).unwrap();
        self.stdin.flush().unwrap();
        let v: serde_json::Value = serde_json::from_str(&self.satirlar.next().unwrap().unwrap()).unwrap();
        v["result"]["tools"].as_array().unwrap().clone()
    }
}

fn token_uret(o: &Ortam, ad: &str, izinler: &[&str]) -> String {
    let mut a = vec!["token", "olustur", "--ad", ad];
    for i in izinler {
        a.extend(["--izin", i]);
    }
    o.ok(&a).split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string()
}

fn adlar(liste: &[serde_json::Value]) -> Vec<String> {
    liste.iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.p.kill();
    }
}

const POST: &str = "POST /api/v1/kitaplar";
fn gecerli() -> serde_json::Value {
    serde_json::json!({ "kapi": POST, "govde": { "baslik": "Kuyucaklı Yusuf", "sayfa": 220, "tur": "roman" } })
}
/// Kesin damgalı ve önizlemesi kapalı dünyada geçen yazma (P0.7: onaysız gidebilen tek tür); kitap.olustur
/// önizlemede bir kuralı (ValidateIf) denetleyemediği için insan onayına düşer
fn yorum() -> serde_json::Value {
    serde_json::json!({ "kapi": "POST /api/v1/yorumlar", "govde": { "metin": "Kuyucaklı Yusuf", "puan": 4, "kitapId": "8a3c1e2b-1111-2222-3333-444455556666" } })
}

#[test]
fn gozlem_modunda_hicbir_istek_gitmez() {
    let o = Ortam::yeni("gozlem");
    let mut m = o.mcp(None);
    let (t, hata) = m.arac("cagir", gecerli());
    assert!(hata && t.contains("gözlem"), "{t}");
    assert_eq!(o.gelen_sayisi(), 0);
}

#[test]
fn yazma_insanin_oturumuyla_gider_ve_tekrar_edilmez() {
    let o = Ortam::yeni("yazma");
    let token = o.kur(&["kitap.*:yaz", "yorum.*:yaz"]);
    let mut m = o.mcp(Some(&token));
    let (t, hata) = m.arac("cagir", yorum());
    assert!(!hata && t.contains("YAZILDI") && t.contains("HTTP 201"), "{t}");
    let g = o.gelenler.lock().unwrap().clone();
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].yontem, "POST");
    assert_eq!(g[0].yol, "/api/v1/yorumlar");
    assert_eq!(g[0].yetki.as_deref(), Some("Bearer eyJhbGciOiJub25lIn0.eyJlbWFpbCI6Imtpc2lAb3JuZWsudGVzdCJ9.x"), "insanın oturumu iletilmeli");
    assert!(g[0].govde.contains("Kuyucaklı Yusuf"));
    // Aynı istek ikinci kez: uygulamaya gitmez
    let (t, hata) = m.arac("cagir", yorum());
    assert!(hata && t.contains("ikinci kayıt açılmadı"), "{t}");
    assert_eq!(o.gelen_sayisi(), 1);
    // Bilerek tekrar: farklı anahtarla
    let mut a = yorum();
    a["tekrar_anahtari"] = serde_json::json!("ikinci-kopya");
    assert!(!m.arac("cagir", a).1);
    assert_eq!(o.gelen_sayisi(), 2);
    // Kayıt: insan, token, işlem; parametre değeri yok; zincir sağlam
    let k = o.ok(&["kayit"]);
    assert!(k.contains("kisi@ornek.test") && k.contains("token ajan") && k.contains("yorum.yaz"), "{k}");
    let ham = std::fs::read_to_string(std::fs::read_dir(o.ev.join("ayar").join("sezgi").join("uygulamalar")).unwrap().next().unwrap().unwrap().path().join("kayit.jsonl")).unwrap();
    assert!(!ham.contains("Kuyucaklı"), "parametre değerleri kayda yazılmaz");
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam"));
}

#[test]
fn reddedilen_istekler_uygulamaya_gitmez() {
    let o = Ortam::yeni("ret");
    let token = o.kur(&["kitap.listele:oku", "kitap.olustur:yaz"]);
    let mut m = o.mcp(Some(&token));
    // Önizlemede reddedilen: zorunlu alan eksik
    let (t, hata) = m.arac("cagir", serde_json::json!({ "kapi": POST, "govde": { "baslik": "X" } }));
    assert!(hata && t.contains("önizlemede reddedildi"), "{t}");
    // Token izni yok: güncelleme açılmadı, bu token'la görünmez bile
    let (t, hata) = m.arac("cagir", serde_json::json!({ "kapi": "PATCH /api/v1/kitaplar/5", "govde": {} }));
    assert!(hata && t.contains("bulunamadı"), "{t}");
    assert_eq!(o.gelen_sayisi(), 0);
    // Tokensiz
    let mut m2 = o.mcp(None);
    let (t, hata) = m2.arac("cagir", gecerli());
    assert!(hata && t.contains("token"), "{t}");
    // İnsan oturumu yok
    o.ok(&["oturum", "kapat"]);
    let (t, hata) = m.arac("cagir", gecerli());
    assert!(hata && t.contains("insan oturumu yok"), "{t}");
    // Okuma modunda yazma yok
    o.ok(&["mod", "okuma"]);
    let (t, hata) = m.arac("cagir", gecerli());
    assert!(hata && t.contains("yazma kapalı"), "{t}");
    // Okuma işlemi okuma modunda gider (oturumsuz da)
    let (t, hata) = m.arac("cagir", serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(!hata && t.contains("OKUNDU"), "{t}");
    assert_eq!(o.gelen_sayisi(), 1);
    assert_eq!(o.gelenler.lock().unwrap()[0].yontem, "GET");
    // İptal edilen token
    o.ok(&["token", "iptal", "ajan"]);
    let (t, hata) = m.arac("cagir", serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(hata && t.contains("iptal"), "{t}");
    assert_eq!(o.gelen_sayisi(), 1);
}

/// Token görünürlüğü: token'la bağlanan ajan yalnızca token'ın çağırabileceği işlemleri görür (gözlem modunda da)
#[test]
fn token_yalnizca_izinli_islemleri_gosterir() {
    let o = Ortam::yeni("gorunurluk");
    let token = o.ok(&["token", "olustur", "--ad", "okur", "--izin", "kitap.listele:oku"]);
    let token = token.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
    let mut m = o.mcp(Some(&token));
    let (t, _) = m.arac("ara", serde_json::json!({ "sorgu": "kitap kitaplar" }));
    assert!(t.contains("GET /api/v1/kitaplar ·"), "{t}");
    assert!(!t.contains("POST") && !t.contains("DELETE") && !t.contains("PATCH"), "izin verilmeyen işlemler görünmemeli:\n{t}");
    for kapi in [POST, "DELETE /api/v1/kitaplar/:id", "PATCH /api/v1/kitaplar/:id"] {
        let (t, _) = m.arac("tani", serde_json::json!({ "kapi": kapi }));
        assert!(t.contains("bulunamadı"), "{kapi}: {t}");
        assert!(m.arac("onizle", serde_json::json!({ "kapi": kapi })).0.contains("bulunamadı"));
    }
    let (t, _) = m.arac("kimsin", serde_json::json!({}));
    assert!(t.contains("Token 'okur': 1 kapı görünür"), "{t}");
    // Yazma seviyesi okumayı da kapsar; yıkıcı ayrıca
    let y = o.ok(&["token", "olustur", "--ad", "yazar", "--izin", "kitap.*:yaz"]);
    let y = y.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
    let (t, _) = o.mcp(Some(&y)).arac("ara", serde_json::json!({ "sorgu": "kitap kitaplar", "adet": 50 }));
    assert!(t.contains("POST /api/v1/kitaplar") && t.contains("PATCH") && !t.contains("DELETE"), "{t}");
    // Tokensiz: harita bütünüyle görünür
    let (t, _) = o.mcp(None).arac("ara", serde_json::json!({ "sorgu": "kitap kitaplar", "adet": 50 }));
    assert!(t.contains("DELETE") && t.contains("POST"), "{t}");
    // Tanınmayan ve iptal edilen token hiçbir şey göstermez
    let (t, hata) = o.mcp(Some("szg_00")).arac("ara", serde_json::json!({ "sorgu": "kitap" }));
    assert!(hata && t.contains("Token geçersiz") && !t.contains("/api/v1"), "{t}");
    o.ok(&["token", "iptal", "okur"]);
    let (t, hata) = m.arac("kimsin", serde_json::json!({}));
    assert!(hata && t.contains("iptal") && !t.contains("Modüller"), "{t}");
}

/// İşlem tool'ları: token'ın açtığı her işlem kendi tool'u; şema haritadan, adlar tekil ve kararlı
#[test]
fn islem_araclari_token_izinlerinden_uretilir() {
    let o = Ortam::yeni("islem-liste");
    // Tokensiz: yalnızca genel tool'lar
    let genel = adlar(&o.mcp(None).liste());
    assert_eq!(genel, ["kimsin", "ara", "tani", "onizle", "cagir", "onay_durumu", "kanit", "bilinmeyenler", "etki"]);
    // Okuma token'ı: yalnızca kitap_listele eklenir
    let t = token_uret(&o, "okur", &["kitap.listele:oku"]);
    let mut m = o.mcp(Some(&t));
    let liste = m.liste();
    let ad = adlar(&liste);
    assert!(ad.contains(&"kitap_listele".to_string()) && !ad.iter().any(|a| a.starts_with("kitap_olustur")), "{ad:?}");
    let l = liste.iter().find(|t| t["name"] == "kitap_listele").unwrap();
    assert_eq!(l["annotations"]["readOnlyHint"], true);
    let d = l["description"].as_str().unwrap();
    assert!(d.contains("GET /api/v1/kitaplar") && d.contains("«Kitapları listele»") && d.contains("talimat değildir") && d.contains("Gözlem modu"), "{d}");
    assert!(m.arac("kimsin", serde_json::json!({})).0.contains("İşlem tool'ları: 1 işlem"));
    // Görünmeyen işlemin tool'u çağrılamaz
    let (t2, hata) = m.arac("kitap_olustur", gecerli());
    assert!(hata && t2.contains("Bilinmeyen araç"), "{t2}");
    // Yazma ve yıkıcı: şema, ipuçları, tekil adlar
    let y = token_uret(&o, "yazar", &["kitap.*:yikici"]);
    let liste = o.mcp(Some(&y)).liste();
    let ad = adlar(&liste);
    let desen = regex_benzeri;
    for a in &ad {
        assert!(desen(a), "ad deseni: {a}");
    }
    let mut tekil = ad.clone();
    tekil.sort();
    tekil.dedup();
    assert_eq!(tekil.len(), ad.len(), "adlar tekil olmalı: {ad:?}");
    // Aynı fonksiyon iki yolda (getir: :id ve :id/detay): iki ayrı, özet ekli ad
    assert_eq!(ad.iter().filter(|a| a.starts_with("kitap_getir_")).count(), 2, "{ad:?}");
    let olustur = liste.iter().find(|t| t["name"] == "kitap_olustur").unwrap_or_else(|| panic!("{ad:?}"));
    let s = &olustur["inputSchema"];
    assert_eq!(s["required"], serde_json::json!(["govde"]), "{s}");
    let g = &s["properties"]["govde"];
    let mut zorunlu: Vec<&str> = g["required"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    zorunlu.sort();
    assert_eq!(zorunlu, ["baslik", "sayfa", "tur"], "{g}");
    assert_eq!(g["properties"]["baslik"]["type"], "string");
    assert_eq!(g["properties"]["baslik"]["minLength"], 2);
    assert_eq!(g["properties"]["sayfa"]["maximum"].as_f64(), Some(5000.0));
    assert!(g["properties"]["tur"]["enum"].is_array(), "{g}");
    assert_eq!(olustur["annotations"]["readOnlyHint"], false);
    let sil = liste.iter().find(|t| t["name"] == "kitap_sil").unwrap();
    assert_eq!(sil["annotations"]["destructiveHint"], true);
    assert_eq!(sil["inputSchema"]["required"], serde_json::json!(["id"]));
    assert!(sil["description"].as_str().unwrap().contains("YIKICI"));
    // Kullanımdan kalkmış işlem açıklamada işaretli
    let getir = liste.iter().find(|t| t["name"].as_str().unwrap().starts_with("kitap_getir_")).unwrap();
    assert!(getir["description"].as_str().unwrap().contains("Kullanımdan kaldırılmış"), "{getir}");
    // Aynı harita yeniden açıldığında adlar aynı (sıraya bağlı değil)
    assert_eq!(adlar(&o.mcp(Some(&y)).liste()), ad);
    // Gözlem modunda çağrı yalnızca önizler
    let (t3, hata) = o.mcp(Some(&y)).arac("kitap_olustur", serde_json::json!({ "govde": { "baslik": "X" } }));
    assert!(!hata && t3.contains("GÖZLEM MODU") && t3.contains("REDDEDİLİR"), "{t3}");
    assert_eq!(o.gelen_sayisi(), 0);
}

/// MCP tool adı deseni: ^[a-zA-Z0-9_-]{1,64}$
fn regex_benzeri(a: &str) -> bool {
    !a.is_empty() && a.len() <= 64 && a.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[test]
fn islem_araci_cagir_ile_ayni_kapilardan_gecer() {
    let o = Ortam::yeni("islem-cagri");
    let token = o.kur(&["kitap.*:yikici", "yorum.*:yaz"]);
    let mut m = o.mcp(Some(&token));
    let ad = adlar(&m.liste());
    assert!(ad.contains(&"kitap_olustur".to_string()) && ad.contains(&"yorum_yaz".to_string()), "{ad:?}");
    // Yazma modunda: test kopyasına, insanın oturumuyla
    let (t, hata) = m.arac("yorum_yaz", serde_json::json!({ "govde": yorum()["govde"] }));
    assert!(!hata && t.contains("YAZILDI"), "{t}");
    let g = o.gelenler.lock().unwrap().clone();
    assert_eq!((g.len(), g[0].yontem.as_str(), g[0].yol.as_str()), (1, "POST", "/api/v1/yorumlar"));
    assert!(g[0].yetki.as_deref().unwrap_or("").starts_with("Bearer eyJ"));
    // Tekrar koruması aynı
    let (t, hata) = m.arac("yorum_yaz", serde_json::json!({ "govde": yorum()["govde"] }));
    assert!(hata && t.contains("ikinci kayıt açılmadı"), "{t}");
    // Yol parametresi en üstte; yıkıcı işlem onay bekler
    let (t, hata) = m.arac("kitap_sil", serde_json::json!({ "id": "7" }));
    assert!(!hata && t.contains("ONAY BEKLİYOR"), "{t}");
    assert_eq!(o.gelen_sayisi(), 1);
    // Önizlemede reddedilen gitmez
    let (t, hata) = m.arac("kitap_olustur", serde_json::json!({ "govde": { "baslik": "X" } }));
    assert!(hata && t.contains("önizlemede reddedildi"), "{t}");
    assert_eq!(o.gelen_sayisi(), 1);
    // Token iptal: işlem tool'u da çalışmaz
    o.ok(&["token", "iptal", "ajan"]);
    let (t, hata) = m.arac("kitap_listele", serde_json::json!({}));
    assert!(hata && t.contains("Token geçersiz"), "{t}");
    assert_eq!(o.gelen_sayisi(), 1);
}

#[test]
fn esik_asilinca_islem_araci_verilmez() {
    let o = Ortam::yeni("islem-esik");
    let t = token_uret(&o, "iki", &["kitap.listele:oku", "kitap.olustur:yaz"]);
    let mut m = o.mcp_ortamli(Some(&t), &[("SEZGI_ISLEM_ESIGI", "1")]);
    assert_eq!(adlar(&m.liste()).len(), 9, "eşik aşıldı: yalnızca genel tool'lar");
    let k = m.arac("kimsin", serde_json::json!({})).0;
    assert!(k.contains("eşiği (1) aşıyor"), "{k}");
    // Eşikte (2 = 2) verilir
    let mut m = o.mcp_ortamli(Some(&t), &[("SEZGI_ISLEM_ESIGI", "2")]);
    assert_eq!(adlar(&m.liste()).len(), 11);
    // 0: kapalı
    let mut m = o.mcp_ortamli(Some(&t), &[("SEZGI_ISLEM_ESIGI", "0")]);
    assert_eq!(adlar(&m.liste()).len(), 9);
    assert!(m.arac("kimsin", serde_json::json!({})).0.contains("kapalı"));
}

/// İnsan oturumu işletim sistemi anahtarlığında: dosyada anahtar yok, kapatınca anahtarlıktan da silinir.
/// Anahtarlığı olmayan ortamda (ör. kapsayıcıdaki Linux) uyarıyla dosyaya düşülür; o durumda yalnızca uyarı sınanır.
#[test]
fn oturum_isletim_sistemi_anahtarliginda() {
    let o = Ortam::yeni("anahtarlik");
    let gercek = |args: &[&str], girdi: Option<&str>| {
        let mut c = o.komut(args);
        c.env_remove("SEZGI_ANAHTARLIK").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut p = c.spawn().unwrap();
        if let Some(g) = girdi {
            p.stdin.as_mut().unwrap().write_all(g.as_bytes()).unwrap();
        }
        drop(p.stdin.take());
        let c = p.wait_with_output().unwrap();
        assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
        format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr))
    };
    let gizli = "anahtarlik-deneme-degeri-1234567890";
    let m = gercek(&["oturum", "ac"], Some(&format!("{gizli}\n")));
    let klasor = std::fs::read_dir(o.ev.join("ayar").join("sezgi").join("uygulamalar")).unwrap().next().unwrap().unwrap().path();
    let dosya = std::fs::read_to_string(klasor.join("oturum.json")).unwrap();
    if m.contains("anahtarlığında") {
        assert!(!dosya.contains(gizli), "anahtar dosyaya yazılmamalı: {dosya}");
        // Okuma modunda okuma gider; yazma oturum ister: oturum anahtarlıktan okunur
        o.ok(&["hedef", &format!("http://127.0.0.1:{}", o.port), "--test-kopyasi"]);
        o.ok(&["mod", "yazma"]);
        let token = o.ok(&["token", "olustur", "--ad", "t", "--izin", "kitap.*:yaz"]).split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
        let mut c = o.komut(&["mcp", ".sezgi"]);
        c.env_remove("SEZGI_ANAHTARLIK").env("SEZGI_TOKEN", &token);
        let mut p = c.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let istek = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "cagir", "arguments": gecerli() } });
        writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
        let cikti = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
        assert!(cikti.contains("YAZILDI"), "{cikti}");
        assert_eq!(o.gelenler.lock().unwrap()[0].yetki.as_deref(), Some(format!("Bearer {gizli}").as_str()), "anahtarlıktaki oturumla");
        gercek(&["oturum", "kapat"], None);
        // Kapatıldıktan sonra anahtarlıkta kalmaz: yeni açılmış gibi davranılmaz
        assert!(!klasor.join("oturum.json").exists());
    } else {
        assert!(m.contains("anahtarlık kullanılamadı"), "anahtarlık yoksa söylenmeli: {m}");
        gercek(&["oturum", "kapat"], None);
    }
}

/// Sorgudaki dizi tekrarlanan anahtar olarak gider (?etiket=a&etiket=b)
#[test]
fn sorgu_dizisi_tekrarlanan_anahtar() {
    let o = Ortam::yeni("sorgu-dizi");
    let token = o.kur(&["kitap.listele:oku"]);
    o.ok(&["mod", "okuma"]);
    let mut m = o.mcp(Some(&token));
    let (t, hata) = m.arac("cagir", serde_json::json!({ "kapi": "GET /api/v1/kitaplar", "sorgu": { "etiket": ["roman", "klasik"], "ara": "yusuf", "sayfa": null } }));
    assert!(!hata && t.contains("OKUNDU"), "{t}");
    assert_eq!(o.gelenler.lock().unwrap()[0].yol, "/api/v1/kitaplar?etiket=roman&etiket=klasik&ara=yusuf");
}

#[test]
fn test_kopyasi_olmayan_hedefe_yazma_yok() {
    let o = Ortam::yeni("gercek");
    let token = o.kur(&["kitap.*:yaz"]);
    o.ok(&["hedef", &format!("http://127.0.0.1:{}", o.port)]);
    let c = o.sezgi(&["mod", "yazma"], None);
    assert!(!c.status.success(), "test kopyası olmadan yazma modu açılmaz");
    let mut m = o.mcp(Some(&token));
    let (t, hata) = m.arac("cagir", gecerli());
    assert!(hata, "{t}");
    assert_eq!(o.gelen_sayisi(), 0);
}

#[test]
fn yikici_islem_insan_onayi_bekler() {
    let o = Ortam::yeni("onay");
    let token = o.kur(&["kitap.*:yikici"]);
    let mut m = o.mcp(Some(&token));
    let (t, hata) = m.arac("cagir", serde_json::json!({ "kapi": "DELETE /api/v1/kitaplar/7" }));
    assert!(!hata && t.contains("ONAY BEKLİYOR"), "{t}");
    assert_eq!(o.gelen_sayisi(), 0, "onaysız gitmez");
    let id = t.split("Onay kimliği: ").nth(1).unwrap().split_whitespace().next().unwrap().to_string();
    assert!(o.ok(&["onay", "listele"]).contains(&id));
    assert!(m.arac("onay_durumu", serde_json::json!({ "id": id })).0.contains("bekliyor"));
    let c = o.ok(&["onay", "ver", &id, "--ozet", &o.onay_ozeti(&id)]);
    assert!(c.contains("HTTP 201"), "{c}");
    let g = o.gelenler.lock().unwrap().clone();
    assert_eq!((g.len(), g[0].yontem.as_str(), g[0].yol.as_str()), (1, "DELETE", "/api/v1/kitaplar/7"));
    assert!(g[0].yetki.is_some(), "onaylayan insanın oturumuyla");
    assert!(m.arac("onay_durumu", serde_json::json!({ "id": id })).0.contains("yapildi"));
    // Reddedilen onay gitmez
    let (t, _) = m.arac("cagir", serde_json::json!({ "kapi": "DELETE /api/v1/kitaplar/8" }));
    let id2 = t.split("Onay kimliği: ").nth(1).unwrap().split_whitespace().next().unwrap().to_string();
    o.ok(&["onay", "reddet", &id2]);
    assert_eq!(o.gelen_sayisi(), 1);
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam"));
}

#[test]
fn bayat_harita_yazma_yaptirmaz() {
    let o = Ortam::yeni("bayat");
    let token = o.kur(&["kitap.*:yaz"]);
    let f = o.kok.join("app").join("src").join("kitap").join("kitap.controller.ts");
    std::fs::write(&f, format!("{}\n// değişiklik\n", std::fs::read_to_string(&f).unwrap())).unwrap();
    let mut m = o.mcp(Some(&token));
    let (t, hata) = m.arac("cagir", gecerli());
    assert!(hata && t.contains("bayat"), "{t}");
    assert_eq!(o.gelen_sayisi(), 0);
}

#[test]
fn kayit_degistirilirse_dogrulama_bozulur() {
    let o = Ortam::yeni("kayit");
    let token = o.kur(&["kitap.*:yaz"]);
    let mut m = o.mcp(Some(&token));
    assert!(!m.arac("cagir", gecerli()).1);
    let klasor = std::fs::read_dir(o.ev.join("ayar").join("sezgi").join("uygulamalar")).unwrap().next().unwrap().unwrap().path();
    let yol = klasor.join("kayit.jsonl");
    let metin = std::fs::read_to_string(&yol).unwrap();
    // Diskte kimlik, token adı ve işlem okunmaz: her satır yalnızca şifreli yüktür ({"s":"c20p1","v":base64}); yük düz
    // JSON değildir. Base64 metninde alt dizgi aramak rastgele eşleşebilir (ör. "ajan"): denetim yapısaldır
    for l in metin.lines() {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert!(v.as_object().unwrap().keys().all(|k| k == "s" || k == "v") && v["s"] == "c20p1", "{l}");
        use base64::Engine;
        let yuk = base64::engine::general_purpose::STANDARD.decode(v["v"].as_str().unwrap()).unwrap();
        assert!(serde_json::from_slice::<serde_json::Value>(&yuk).is_err(), "yük düz JSON: {l}");
    }
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam"));
    // Şifreli metnin tek bir harfini değiştirmek: doğrulanmış şifreleme çözmeyi bozar
    let ilk = metin.lines().next().unwrap();
    let v: serde_json::Value = serde_json::from_str(ilk).unwrap();
    let s = v["v"].as_str().unwrap();
    let orta = s.len() / 2;
    let degisik = format!("{}{}{}", &s[..orta], if &s[orta..orta + 1] == "A" { "B" } else { "A" }, &s[orta + 1..]);
    std::fs::write(&yol, metin.replacen(s, &degisik, 1)).unwrap();
    let c = o.sezgi(&["kayit", "--dogrula"], None);
    assert!(!c.status.success(), "{}", String::from_utf8_lossy(&c.stdout));
}

#[test]
fn kayit_sifreli_anahtarsiz_okunmaz() {
    let o = Ortam::yeni("sifre");
    let token = o.kur(&["kitap.*:yaz"]);
    let mut m = o.mcp(Some(&token));
    assert!(!m.arac("cagir", gecerli()).1);
    // Denetçiye: çözülmüş satırlar
    let json = o.ok(&["kayit", "--json"]);
    let satirlar: Vec<serde_json::Value> = json.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(satirlar.iter().any(|v| v["islem"] == "kitap.olustur" && v["token"] == "ajan"), "{json}");
    assert!(o.ok(&["defter"]).contains("kayıt şifreli"));
    // Anahtar yoksa satırlar çözülemez: doğrulama bozuk der, yeni kayıt zinciri çatallamaz
    let klasor = std::fs::read_dir(o.ev.join("ayar").join("sezgi").join("uygulamalar")).unwrap().next().unwrap().unwrap().path();
    std::fs::rename(klasor.join("kayit.anahtar"), klasor.join("yedek.anahtar")).unwrap();
    assert!(!o.sezgi(&["kayit", "--dogrula"], None).status.success());
    let (t, hata) = m.arac("cagir", gecerli());
    assert!(hata && t.contains("kayda geçemeyeceği için iletilmedi"), "kayda yazılamayan istek iletilmemeli: {t}");
    std::fs::rename(klasor.join("yedek.anahtar"), klasor.join("kayit.anahtar")).unwrap();
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam"));
}

/// Deneme kimlik sağlayıcısı (OIDC): keşif belgesi, cihaz yetkilendirmesi, belirteç ucu. İlk yoklamada
/// `authorization_pending`, sonra 1 saniye ömürlü erişim belirteci; yenileme isteğine yeni belirteç. Gelen
/// isteklerin özeti döner (grant türleri).
fn sahte_saglayici() -> (u16, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let gelen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let g2 = gelen.clone();
    std::thread::spawn(move || {
        let mut yoklama = 0;
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut ilk = String::new();
            r.read_line(&mut ilk).unwrap();
            let mut boy = 0usize;
            loop {
                let mut b = String::new();
                r.read_line(&mut b).unwrap();
                if b.trim().is_empty() {
                    break;
                }
                if let Some(v) = b.to_ascii_lowercase().strip_prefix("content-length:") {
                    boy = v.trim().parse().unwrap();
                }
            }
            let mut govde = vec![0u8; boy];
            r.read_exact(&mut govde).unwrap();
            let govde = String::from_utf8(govde).unwrap();
            let kok = format!("http://127.0.0.1:{port}");
            // E-postalı kimlik belirteci (imzasız deneme): {"email":"oidc@ornek.test"}
            let kimlik = "eyJhbGciOiJub25lIn0.eyJlbWFpbCI6Im9pZGNAb3JuZWsudGVzdCJ9.x";
            let (kod, cevap) = if ilk.contains("/.well-known/openid-configuration") {
                (200, serde_json::json!({ "issuer": kok, "token_endpoint": format!("{kok}/token"), "device_authorization_endpoint": format!("{kok}/cihaz") }))
            } else if ilk.contains("/cihaz") {
                (200, serde_json::json!({ "device_code": "dc-1", "user_code": "ABCD-EFGH", "verification_uri": format!("{kok}/etkinlestir"), "interval": 0, "expires_in": 60 }))
            } else if govde.contains("device_code") {
                g2.lock().unwrap().push("cihaz".into());
                yoklama += 1;
                if yoklama == 1 {
                    (400, serde_json::json!({ "error": "authorization_pending" }))
                } else {
                    (200, serde_json::json!({ "access_token": "ilk-erisim", "refresh_token": "yenile-1", "expires_in": 1, "id_token": kimlik }))
                }
            } else if govde.contains("token-exchange") {
                // RFC 8693: yalnızca hedef kitle "kitapci" için daha dar belirteç
                g2.lock().unwrap().push("degisim".into());
                let konu = govde.split('&').find_map(|x| x.strip_prefix("subject_token=")).unwrap_or("").to_string();
                if govde.contains("audience=kitapci") {
                    (
                        200,
                        serde_json::json!({ "access_token": format!("dar-{konu}"), "issued_token_type": "urn:ietf:params:oauth:token-type:access_token", "expires_in": if konu == "ilk-erisim" { 1 } else { 3600 } }),
                    )
                } else {
                    (400, serde_json::json!({ "error": "invalid_target" }))
                }
            } else if govde.contains("grant_type=refresh_token") && govde.contains("refresh_token=yenile-1") {
                g2.lock().unwrap().push("yenileme".into());
                (200, serde_json::json!({ "access_token": "yeni-erisim", "refresh_token": "yenile-2", "expires_in": 3600 }))
            } else {
                (400, serde_json::json!({ "error": "invalid_grant" }))
            };
            let c = cevap.to_string();
            let mut s = s;
            write!(s, "HTTP/1.1 {kod} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{c}", c.len()).unwrap();
        }
    });
    (port, gelen)
}

#[test]
fn oidc_cihaz_akisiyla_oturum_ve_yenileme() {
    let o = Ortam::yeni("oidc");
    let token = o.kur(&["kitap.*:yaz", "yorum.*:yaz"]);
    let (sp, gelen) = sahte_saglayici();
    let issuer = format!("http://127.0.0.1:{sp}");
    let c = o.sezgi(&["oturum", "ac", "--oidc", &issuer, "--istemci", "sezgi-deneme"], None);
    let cikti = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(c.status.success(), "{cikti}");
    assert!(cikti.contains("ABCD-EFGH") && cikti.contains("/etkinlestir"), "kullanıcıya adres ve kod gösterilir: {cikti}");
    assert!(cikti.contains("oidc@ornek.test"), "kimlik, kimlik belirtecinden: {cikti}");
    // Erişim belirtecinin ömrü 1 sn: ilk yazmada yenileme belirteciyle yenilenir, istek yeni belirteçle gider
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let mut m = o.mcp(Some(&token));
    let (t, hata) = m.arac("cagir", yorum());
    assert!(!hata, "{t}");
    assert_eq!(o.gelenler.lock().unwrap().last().unwrap().yetki.as_deref(), Some("Bearer yeni-erisim"));
    assert_eq!(*gelen.lock().unwrap(), ["cihaz", "cihaz", "yenileme"], "bir bekleme, bir onay, bir yenileme");
    let k = o.ok(&["kayit"]);
    assert!(k.contains("oturum_yenilendi") && k.contains("oidc@ornek.test"), "{k}");
    // Güvenli olmayan adres reddedilir: uzak bir http:// sağlayıcı
    let c = o.sezgi(&["oturum", "ac", "--oidc", "http://kimlik.ornek.test", "--istemci", "x"], None);
    assert!(!c.status.success() && String::from_utf8_lossy(&c.stderr).contains("https://"));
}

#[test]
fn oidc_belirtec_degisimiyle_dar_belirtec() {
    let o = Ortam::yeni("degisim");
    let token = o.kur(&["kitap.*:yaz", "yorum.*:yaz"]);
    let (sp, gelen) = sahte_saglayici();
    let issuer = format!("http://127.0.0.1:{sp}");
    let c = o.sezgi(&["oturum", "ac", "--oidc", &issuer, "--istemci", "sezgi-deneme", "--hedef-kitle", "kitapci"], None);
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    // Dar belirtecin ömrü 1 sn: yenileme belirteciyle yeni belirteç alınır ve yeniden daraltılır
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let mut m = o.mcp(Some(&token));
    let (t, hata) = m.arac("cagir", yorum());
    assert!(!hata, "{t}");
    assert_eq!(o.gelenler.lock().unwrap().last().unwrap().yetki.as_deref(), Some("Bearer dar-yeni-erisim"), "uygulamaya geniş belirteç gitmez");
    assert_eq!(*gelen.lock().unwrap(), ["cihaz", "cihaz", "degisim", "yenileme", "degisim"]);
    // Sağlayıcı değişimi reddederse oturum açılmaz (geniş belirteçle devam edilmez)
    let c = o.sezgi(&["oturum", "ac", "--oidc", &issuer, "--istemci", "sezgi-deneme", "--hedef-kitle", "baska"], None);
    assert!(!c.status.success() && String::from_utf8_lossy(&c.stderr).contains("değişimi reddedildi"), "{}", String::from_utf8_lossy(&c.stderr));
}

/// Kabul ölçütü 4 (PROMPT-BAGLAM K4): canlı mod tek komutla kurulur, her kapı açıkça geçilir; durum ekranı sonraki
/// adımı söyler
#[test]
fn canli_mod_tek_komutla_kurulur() {
    let o = Ortam::yeni("canli-kur");
    let d = o.ok(&["canli"]);
    assert!(d.contains("gozlem") && d.contains("sezgi canli kur"), "kurulmamışken durum ve sonraki adım: {d}");
    // Yazma test kopyası olmadan açılmaz
    let c = o.sezgi(&["canli", "kur", "--hedef", &format!("http://127.0.0.1:{}", o.port), "--mod", "yazma"], None);
    assert!(!c.status.success() && String::from_utf8_lossy(&c.stderr).contains("test kopyası"), "{}", String::from_utf8_lossy(&c.stderr));
    // Tek komut: hedef (test kopyası), mod, insan oturumu (standart girdiden), ilk token
    let c = o.sezgi(
        &["canli", "kur", "--hedef", &format!("http://127.0.0.1:{}", o.port), "--test-kopyasi", "--mod", "yazma", "--izin", "*:yaz", "--tum-islemler"],
        Some("eyJhbGciOiJub25lIn0.eyJlbWFpbCI6Imtpc2lAb3JuZWsudGVzdCJ9.x\n"),
    );
    let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(c.status.success(), "{m}");
    for adim in ["1/4", "2/4", "3/4", "4/4"] {
        assert!(m.contains(adim), "adım {adim} görünür: {m}");
    }
    let token = m.split_whitespace().find(|w| w.starts_with("szg_")).expect("token gösterilir").to_string();
    assert!(m.contains("claude mcp add sezgi -e SEZGI_TOKEN="), "bağlantı satırı: {m}");
    // Durum: her kapı yerinde
    let d = o.ok(&["canli"]);
    assert!(d.contains("yazma") && d.contains("test kopyası") && d.contains("kisi@ornek.test") && d.contains("1 token") && d.contains("sağlam"), "{d}");
    // Kurulan düzenle yazma gerçekten gider
    let mut m = o.mcp(Some(&token));
    let (t, hata) = m.arac("cagir", yorum());
    assert!(!hata && t.contains("YAZILDI"), "{t}");
    // Varsayılan izin en kısıtlısı: --izin verilmezse yalnızca okuma
    let o2 = Ortam::yeni("canli-kur-varsayilan");
    let m2 = o2.ok(&["canli", "kur", "--hedef", &format!("http://127.0.0.1:{}", o2.port)]);
    assert!(
        m2.contains("☑ * : oku") && m2.contains("Mod: okuma") && !m2.lines().any(|l| l.contains("☑") && (l.contains("yaz") || l.contains("yikici"))),
        "varsayılan: okuma modu ve yalnızca okuma izni: {m2}"
    );
}
