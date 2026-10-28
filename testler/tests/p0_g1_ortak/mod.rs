//! P0 G1 (canlı mod: proje kimliği, bağlam anahtarlı hafıza, onay, politika) testlerinin ortak düzeni.
//! İstekleri sayan ve kaydeden sahte uygulama, örnek uygulamanın geçici kopyası ve ortak bir ev klasörü.
#![allow(dead_code)]
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::{Arc, Mutex};

/// Testlerde insanın parolası (sistem çapası `Proje::insan_capasi` ile bu parolayla kurulur)
pub const TEST_PAROLASI: &str = "insan-parolasi-123";

#[derive(Clone, Debug)]
pub struct Gelen {
    pub yontem: String,
    pub yol: String,
    pub yetki: Option<String>,
    pub govde: String,
}

/// İstekleri kaydeden sahte uygulama: her isteğe 201 ve istek sırasını içeren JSON
pub struct Uygulama {
    pub port: u16,
    pub gelenler: Arc<Mutex<Vec<Gelen>>>,
}

impl Uygulama {
    pub fn yeni() -> Uygulama {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let gelenler = Arc::new(Mutex::new(vec![]));
        let g2 = gelenler.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut ilk = String::new();
                if r.read_line(&mut ilk).is_err() {
                    continue;
                }
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
                let n = {
                    let mut g = g2.lock().unwrap();
                    g.push(Gelen { yontem: p[0].into(), yol: p[1].into(), yetki: yetki.clone(), govde: String::from_utf8_lossy(&govde).into() });
                    g.len()
                };
                let cevap = format!("{{\"sira\":{n},\"kim\":\"{}\"}}", yetki.unwrap_or_default().len());
                let mut s = s;
                let _ = write!(s, "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cevap}", cevap.len());
                let _ = s.shutdown(std::net::Shutdown::Write);
            }
        });
        Uygulama { port, gelenler }
    }
    pub fn sayi(&self) -> usize {
        self.gelenler.lock().unwrap().len()
    }
    pub fn adres(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
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

/// Bir ev klasörü (kullanıcı yapılandırması) ve içinde istenen sayıda proje
pub struct Ev {
    pub kok: PathBuf,
}

impl Drop for Ev {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

impl Ev {
    pub fn yeni(ad: &str) -> Ev {
        let kok = std::env::temp_dir().join(format!("sezgi-p0g1-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("ev").join("ayar")).unwrap();
        Ev { kok }
    }
    pub fn ayar(&self) -> PathBuf {
        self.kok.join("ev").join("ayar")
    }
    /// Örnek uygulamayı `ad` klasörüne kopyalar ve haritalar
    pub fn proje(&self, ornek: &str, ad: &str) -> Proje {
        let klasor = self.kok.join(ad);
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ornek), &klasor);
        let p = Proje { klasor, ev: self.kok.join("ev") };
        p.ok(&["init", "."]);
        p
    }
}

pub struct Proje {
    pub klasor: PathBuf,
    pub ev: PathBuf,
}

impl Proje {
    pub fn komut(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args)
            .current_dir(&self.klasor)
            .env("HOME", &self.ev)
            .env("USERPROFILE", &self.ev)
            .env("APPDATA", self.ev.join("ayar"))
            .env("XDG_CONFIG_HOME", self.ev.join("ayar"))
            .env_remove("SEZGI_TOKEN")
            .env("SEZGI_ANAHTARLIK", "kapali")
            .env("SEZGI_CAPA_KLASORU", self.capa_klasoru());
        c
    }
    /// Bu testin sistem çapası klasörü (kök sahipli; yalnızca `insan_capasi` kurar). Testler birbirinin çapasını görmez.
    pub fn capa_klasoru(&self) -> PathBuf {
        self.ev.join("capa")
    }
    /// Sistem çapasını (insan anahtarı) test parolasıyla kurar: `sezgi insan kur --sistem` kök olarak, sahte terminalde.
    /// Bu komutu bilmeyen sürümde sessizce geçer (paralel dallarda aynı testler çalışır).
    pub fn insan_capasi(&self) {
        if self.capa_klasoru().join("insan-0.json").exists() {
            return;
        }
        let p = format!("{TEST_PAROLASI}\n{TEST_PAROLASI}\n");
        let _ = self.sahte_terminalde_ham(&["insan", "kur", "--sistem", "--kullanici", "0"], &p, &[]);
    }
    /// Gerçek veriyi gerçek komut yoluyla açar (sistem çapası, terminalde uygulamanın adı ve insan parolası) ve yazma
    /// modunu açar. Ayar dosyasını elle düzenlemek artık gerçek veriyi açmaz (imza tutmaz).
    pub fn gercek_veri_ac(&self, adres: &str) {
        self.insan_capasi();
        let ad = self.kart()["app"]["name"].as_str().unwrap().to_string();
        let (ok, m) = self.sahte_terminalde(&["hedef", adres, "--gercek-veri"], &format!("{ad}\n"));
        assert!(ok, "{m}");
        self.ok(&["mod", "yazma"]);
    }
    pub fn sezgi(&self, args: &[&str], girdi: Option<&str>) -> Output {
        let mut p = self.komut(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        if let Some(g) = girdi {
            p.stdin.as_mut().unwrap().write_all(g.as_bytes()).unwrap();
        }
        drop(p.stdin.take());
        p.wait_with_output().unwrap()
    }
    pub fn cikti(&self, args: &[&str]) -> (bool, String) {
        let c = self.sezgi(args, None);
        (c.status.success(), format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr)))
    }
    pub fn ok(&self, args: &[&str]) -> String {
        let (b, m) = self.cikti(args);
        assert!(b, "{args:?}: {m}");
        m
    }
    /// Bekleyen isteğin `onay goster` çıktısındaki içerik özetinin ilk 8 karakteri (terminal yokken onay buna bağlanır)
    pub fn onay_ozeti(&self, id: &str) -> String {
        let g = self.ok(&["onay", "goster", id]);
        g.split("Özet: ").nth(1).unwrap_or_else(|| panic!("{g}")).chars().take(8).collect()
    }
    pub fn oturum(&self, deger: &str) {
        let c = self.sezgi(&["oturum", "ac"], Some(&format!("{deger}\n")));
        assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    }
    pub fn token(&self, ad: &str, izinler: &[&str], ek: &[&str]) -> String {
        let mut a = vec!["token", "olustur", "--ad", ad];
        for i in izinler {
            a.extend(["--izin", i]);
        }
        a.extend(ek);
        self.ok(&a).split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string()
    }
    /// Test kopyası hedefi, yazma modu ve insan oturumu
    pub fn kur(&self, u: &Uygulama, oturum: &str) {
        self.ok(&["hedef", &u.adres(), "--test-kopyasi"]);
        self.ok(&["mod", "yazma"]);
        self.oturum(oturum);
    }
    pub fn mcp(&self, token: &str) -> Mcp {
        let mut p = self.komut(&["mcp", ".sezgi"]).env("SEZGI_TOKEN", token).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let stdin = p.stdin.take().unwrap();
        let satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
        Mcp { p, stdin, satirlar: Box::new(satirlar), id: 0 }
    }
    pub fn kart(&self) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(self.klasor.join(".sezgi").join("benlik.json")).unwrap()).unwrap()
    }
    /// Komutu sahte bir terminalde (`script`, util-linux) çalıştırır; `girdi` terminale yazılır. (başarılı mı, çıktı)
    pub fn sahte_terminalde(&self, args: &[&str], girdi: &str) -> (bool, String) {
        self.sahte_terminalde_ortamla(args, girdi, &[])
    }
    /// `sahte_terminalde`, ek ortam değişkenleriyle. Girdinin sonuna insan parolası eklenir: sistem çapası kuruluysa
    /// insan adımları (gerçek veri, onay, rol) cevaptan sonra parolayı sorar; çapa yoksa ya da soru yoksa satır okunmaz.
    pub fn sahte_terminalde_ortamla(&self, args: &[&str], girdi: &str, ortam: &[(&str, &str)]) -> (bool, String) {
        self.sahte_terminalde_ham(args, &format!("{girdi}{TEST_PAROLASI}\n"), ortam)
    }
    /// Sahte terminal, girdi olduğu gibi (parola eklenmez: parolayı bilmeyen ajanın denemesi)
    pub fn sahte_terminalde_ham(&self, args: &[&str], girdi: &str, ortam: &[(&str, &str)]) -> (bool, String) {
        let tirnak = |a: &str| format!("'{}'", a.replace('\'', "'\\''"));
        let komut: Vec<String> = std::iter::once(env!("CARGO_BIN_EXE_sezgi")).chain(args.iter().copied()).map(tirnak).collect();
        let mut c = Command::new("script");
        c.args(["-qec", &komut.join(" "), "/dev/null"])
            .current_dir(&self.klasor)
            .env("HOME", &self.ev)
            .env("USERPROFILE", &self.ev)
            .env("APPDATA", self.ev.join("ayar"))
            .env("XDG_CONFIG_HOME", self.ev.join("ayar"))
            .env_remove("SEZGI_TOKEN")
            .env("SEZGI_ANAHTARLIK", "kapali")
            .env("SEZGI_CAPA_KLASORU", self.capa_klasoru())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (a, d) in ortam {
            c.env(a, d);
        }
        let mut s = c.spawn().expect("script (util-linux) gerekli");
        s.stdin.as_mut().unwrap().write_all(girdi.as_bytes()).unwrap();
        drop(s.stdin.take());
        let o = s.wait_with_output().unwrap();
        (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
    }
    /// İnsan oturumu, terminalde beyan edilen rollerle (terminalsiz `--rol` beyanı doğrulanmış sayılmaz)
    pub fn oturum_rollu(&self, deger: &str, roller: &[&str]) -> String {
        let mut a = vec!["oturum", "ac"];
        for r in roller {
            a.extend(["--rol", r]);
        }
        let (ok, m) = self.sahte_terminalde(&a, &format!("{deger}\n"));
        assert!(ok, "{m}");
        m
    }
}

pub struct Mcp {
    p: Child,
    stdin: ChildStdin,
    satirlar: Box<dyn Iterator<Item = std::io::Result<String>>>,
    id: u64,
}

impl Mcp {
    pub fn arac(&mut self, ad: &str, a: serde_json::Value) -> (String, bool) {
        self.id += 1;
        let istek = serde_json::json!({ "jsonrpc": "2.0", "id": self.id, "method": "tools/call", "params": { "name": ad, "arguments": a } });
        writeln!(self.stdin, "{istek}").unwrap();
        self.stdin.flush().unwrap();
        let v: serde_json::Value = serde_json::from_str(&self.satirlar.next().unwrap().unwrap()).unwrap();
        (v["result"]["content"][0]["text"].as_str().unwrap().to_string(), v["result"]["isError"] == true)
    }
    pub fn cagir(&mut self, a: serde_json::Value) -> (String, bool) {
        self.arac("cagir", a)
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.p.kill();
    }
}

/// "ONAY BEKLİYOR … Onay kimliği: <id>" metninden kimlik
pub fn onay_kimligi(t: &str) -> String {
    t.split("Onay kimliği: ").nth(1).unwrap_or_else(|| panic!("onay kimliği yok: {t}")).split_whitespace().next().unwrap().to_string()
}

/// NestJS örneğinde önizlemesi kapalı dünyada GEÇER olan kesin yazma (yorum.yaz)
pub fn yorum_yaz() -> serde_json::Value {
    serde_json::json!({ "kapi": "POST /api/v1/yorumlar", "govde": { "metin": "güzel", "puan": 4, "kitapId": "8a3c1e2b-1111-2222-3333-444455556666" } })
}

/// NestJS örneğinde kesin damgalı ama önizlemesi bir kuralı denetleyemeyen yazma (ValidateIf)
pub fn kitap_olustur() -> serde_json::Value {
    serde_json::json!({ "kapi": "POST /api/v1/kitaplar", "govde": { "baslik": "Kuyucaklı Yusuf", "sayfa": 220, "tur": "roman" } })
}
