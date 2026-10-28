//! D7 arayüz: haritasız açılış ve ilk temas. Bulunulan klasörde harita yoksa `sezgi ayna` hata vermez, Sezgi'yi
//! tanıtan sayfayı açar ve `sezgi init` adımını gösterir; hiçbir şey yazmaz. Güvenlik modeli aynıdır (anahtar, Host,
//! Origin, CSP). Argümansız `sezgi` Ayna'yı önerir. Sayfa gerçek bir başsız tarayıcıda da çizilir.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

struct Ayna {
    p: Child,
    port: u16,
    anahtar: String,
    cikti: String,
}

impl Drop for Ayna {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
    }
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

/// Haritası çıkarılmamış proje (örnek uygulamanın kopyası ya da boş klasör)
fn haritasiz(ev: &Ev, ornek: Option<&str>, ad: &str) -> Proje {
    let klasor = ev.kok.join(ad);
    match ornek {
        Some(o) => kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(o), &klasor),
        None => std::fs::create_dir_all(&klasor).unwrap(),
    }
    Proje { klasor, ev: ev.kok.join("ev") }
}

#[allow(clippy::zombie_processes)]
fn ayna(p: &Proje) -> Ayna {
    let mut c = p.komut(&["ayna", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut cikti = String::new();
    let mut satir = String::new();
    let (port, anahtar) = loop {
        satir.clear();
        if r.read_line(&mut satir).unwrap() == 0 {
            let mut hata = String::new();
            let _ = c.stderr.take().unwrap().read_to_string(&mut hata);
            let _ = c.kill();
            let _ = c.wait();
            panic!("Ayna haritasız açılmadı: {cikti}{hata}");
        }
        cikti.push_str(&satir);
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port: u16 = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let a: String = k.split("#anahtar=").nth(1).unwrap().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            break (port, a);
        }
    };
    // Kalan başlangıç satırları: son satır "Durdurmak için Ctrl+C"
    while !cikti.contains("Durdurmak") {
        satir.clear();
        if r.read_line(&mut satir).unwrap_or(0) == 0 {
            break;
        }
        cikti.push_str(&satir);
    }
    Ayna { p: c, port, anahtar, cikti }
}

fn iste(port: u16, yontem: &str, yol: &str, basliklar: &[(&str, &str)], govde: &str) -> (String, String, String) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let host = basliklar.iter().find(|(k, _)| *k == "Host").map(|(_, v)| v.to_string()).unwrap_or(format!("127.0.0.1:{port}"));
    let mut b = format!("{yontem} {yol} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\n", govde.len());
    for (k, v) in basliklar.iter().filter(|(k, _)| *k != "Host") {
        b.push_str(&format!("{k}: {v}\r\n"));
    }
    b.push_str("\r\n");
    s.write_all(b.as_bytes()).unwrap();
    s.write_all(govde.as_bytes()).unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    let (d, g) = c.split_once("\r\n\r\n").unwrap_or((&c, ""));
    (d.lines().next().unwrap_or("").to_string(), d.to_string(), g.to_string())
}

fn json(a: &Ayna, yol: &str) -> Value {
    let (d, _, g) = iste(a.port, "GET", yol, &[("X-Sezgi-Anahtar", &a.anahtar)], "");
    assert!(d.contains("200"), "{yol}: {d} {g}");
    serde_json::from_str(&g).unwrap()
}

fn yetenek(s: &Value, id: &str) -> Value {
    s["yetenekler"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap_or_else(|| panic!("{id} yok: {s}")).clone()
}

#[test]
fn haritasiz_ayna_tanitimi_acar_ve_init_onerir() {
    let ev = Ev::yeni("d7-arayuz-haritasiz");
    let p = haritasiz(&ev, Some("nestjs-temel"), "a");
    let a = ayna(&p);
    assert!(a.cikti.contains("harita yok") && a.cikti.contains("sezgi init"), "başlangıç çıktısı ilk adımı söyler:\n{}", a.cikti);
    // Anahtar yine zorunlu
    for yol in ["/api/sezgi", "/api/sorular", "/api/harita"] {
        let (d, _, _) = iste(a.port, "GET", yol, &[], "");
        assert!(d.contains("403"), "{yol} anahtarsız: {d}");
    }
    let s = json(&a, "/api/sezgi");
    assert_eq!(s["harita"], false, "{s}");
    assert_eq!(s["cati"], "NestJS", "{s}");
    assert_eq!(yetenek(&s, "anla")["durum"], "kurulmamis", "{s}");
    assert_eq!(yetenek(&s, "anla")["komut"], "sezgi init", "{s}");
    assert_eq!(s["sonraki"]["komut"], "sezgi init", "{s}");
    for id in ["bagla", "gor", "canli", "santral"] {
        assert_eq!(yetenek(&s, id)["durum"], "kurulmamis", "{id}: {s}");
    }
    assert_eq!(yetenek(&s, "kaynaksiz")["durum"], "uygulanamaz", "kaynak kod var: {s}");
    // Sorular: hepsi dürüstçe bilinmiyor, nedeni ve init komutuyla
    let b = json(&a, "/api/sorular");
    assert_eq!(b["harita"], false);
    let sorular = b["sorular"].as_array().unwrap();
    assert_eq!(sorular.len(), 17);
    for q in sorular {
        assert!(q["durum"] == "bilinmiyor" && q["guven"] == "unknown" && q["maddeler"].as_array().unwrap().is_empty(), "{q}");
        assert_eq!(q["komut"]["komut"], "sezgi init", "{q}");
    }
    // Harita uçları "harita yok" der; token üretilemez
    let (d, _, g) = iste(a.port, "GET", "/api/harita", &[("X-Sezgi-Anahtar", &a.anahtar)], "");
    assert!(d.contains("404") && g.contains("sezgi init"), "{d} {g}");
    let koken = format!("http://127.0.0.1:{}", a.port);
    let govde = serde_json::json!({ "ad": "x", "izinler": ["kitap.listele:oku"] }).to_string();
    let (d, _, g) = iste(a.port, "POST", "/api/token", &[("Origin", &koken), ("Content-Type", "application/json"), ("X-Sezgi-Anahtar", &a.anahtar)], &govde);
    assert!(!d.contains("200") && !g.contains("szg_"), "haritasız token yok: {d} {g}");
    // Host denetimi ve CSP aynı
    let (d, _, _) = iste(a.port, "GET", "/", &[("Host", "kotu.example:80")], "");
    assert!(d.contains("403"), "yabancı Host: {d}");
    let (d, bas, _) = iste(a.port, "GET", "/", &[], "");
    assert!(d.contains("200"), "{d}");
    let csp = bas.lines().find(|l| l.to_ascii_lowercase().starts_with("content-security-policy:")).expect("CSP yok");
    assert!(csp.contains("script-src 'self'") && !csp.contains("unsafe-inline"), "{csp}");
    // Hiçbir şey yazılmadı
    assert!(!p.klasor.join(".sezgi").exists(), "haritasız Ayna .sezgi yazmamalı");
}

#[test]
fn haritasiz_aynada_init_sonrasi_harita_yuklenir() {
    let ev = Ev::yeni("d7-arayuz-sonra-init");
    let p = haritasiz(&ev, Some("nestjs-temel"), "a");
    let a = ayna(&p);
    assert_eq!(json(&a, "/api/sezgi")["harita"], false);
    p.ok(&["init"]);
    let s = json(&a, "/api/sezgi");
    assert_eq!(s["harita"], true, "init sonrası Ayna haritayı yüklemeli: {s}");
    assert_eq!(s["islem"], 8, "{s}");
    let (d, _, _) = iste(a.port, "GET", "/api/harita", &[("X-Sezgi-Anahtar", &a.anahtar)], "");
    assert!(d.contains("200"), "{d}");
    assert_eq!(json(&a, "/api/sorular")["harita"], true);
}

#[test]
fn catisiz_klasorde_kaynaksiz_kesif_onerilir() {
    let ev = Ev::yeni("d7-arayuz-catisiz");
    let p = haritasiz(&ev, None, "bos");
    let a = ayna(&p);
    let s = json(&a, "/api/sezgi");
    assert!(s["cati"].is_null(), "{s}");
    assert_eq!(yetenek(&s, "anla")["durum"], "uygulanamaz", "{s}");
    assert_eq!(yetenek(&s, "kaynaksiz")["durum"], "kurulmamis", "{s}");
    assert!(s["sonraki"]["aciklama"].as_str().unwrap().contains("sezgi kesfet"), "{s}");
}

#[test]
fn verilen_klasorde_harita_yoksa_hata() {
    // Yazım hatası sessizce tanıtıma düşmez: açıkça verilen klasörde harita yoksa hata
    let ev = Ev::yeni("d7-arayuz-verilen");
    let p = haritasiz(&ev, None, "bos");
    let (b, m) = p.cikti(&["ayna", "yok-boyle-klasor", "--port", "0"]);
    assert!(!b && m.contains("sezgi init"), "{m}");
}

#[test]
fn argumansiz_sezgi_aynayi_onerir() {
    let ev = Ev::yeni("d7-arayuz-ilk-temas");
    let p = haritasiz(&ev, None, "bos");
    let m = p.ok(&[]);
    assert!(m.lines().count() <= 12, "en fazla 12 satır:\n{m}");
    let satir = m.lines().find(|l| l.contains("sezgi ayna")).unwrap_or_else(|| panic!("Ayna önerilmeli:\n{m}"));
    assert!(satir.contains("yetenek"), "{satir}");
    assert!(m.contains("harita gerekmez"), "haritasız da açıldığı söylenir:\n{m}");
    assert!(m.contains("sezgi init") && m.contains("sezgi yardim"), "{m}");
    let q = ev.proje("nestjs-temel", "b");
    let m = q.ok(&[]);
    assert!(m.lines().count() <= 12, "{m}");
    assert!(m.lines().any(|l| l.contains("sezgi ayna") && l.contains("neler yapabildiğini")), "{m}");
    let y = q.ok(&["yardim", "ayna"]);
    assert!(y.contains("neler yapabildiği") && y.contains("harita yoksa tanıtım"), "{y}");
}

/// Başsız tarayıcı: SEZGI_TARAYICI ya da bilinen yollar
fn tarayici() -> PathBuf {
    if let Some(t) = std::env::var_os("SEZGI_TARAYICI") {
        return PathBuf::from(t);
    }
    let adaylar = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    adaylar.iter().map(PathBuf::from).find(|p| p.exists()).expect("başsız tarayıcı bulunamadı: SEZGI_TARAYICI=<chrome|chromium|msedge yolu> verin")
}

fn dom(a: &Ayna, ad: &str) -> String {
    let profil = std::env::temp_dir().join(format!("sezgi-d7-arayuz-{ad}-{}", std::process::id()));
    let adres = format!("http://127.0.0.1:{}/#anahtar={}", a.port, a.anahtar);
    let c = Command::new(tarayici())
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-first-run",
            "--no-default-browser-check",
            &format!("--user-data-dir={}", profil.display()),
            "--virtual-time-budget=8000",
            "--dump-dom",
            &adres,
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&profil);
    String::from_utf8_lossy(&c.stdout).to_string()
}

#[test]
fn tarayicida_tanitim_ve_sorular_cizilir() {
    let ev = Ev::yeni("d7-arayuz-tarayici");
    let p = ev.proje("nestjs-temel", "a");
    let a = ayna(&p);
    let d = dom(&a, "haritali");
    assert!(d.contains("Sezgi neler yapabilir?"), "tanıtım çizilmedi:\n{}", &d[..d.len().min(2000)]);
    for parca in ["Uygulamayı anlar", "Yapay zekâya kanıtla anlatır", "Yapay zekâya güvenli iş yaptırır", "Kaynaksız keşif", "✓ hazır", "○ kurulmamış", "– uygulanamaz", "Sıradaki adım", "sezgi bagla"] {
        assert!(d.contains(parca), "sayfada '{parca}' yok");
    }
    for parca in ["Bu uygulama nedir?", "İnsan onayı gerekir mi?", "Neden var?", "Hangi iş problemini çözer?", "Bilinmiyor: "] {
        assert!(d.contains(parca), "sorularda '{parca}' yok");
    }
    // Harita sekmeleri görünür, kapı listesi de yüklendi
    assert!(d.contains("/kitaplar"), "kapı listesi");
    drop(a);

    let q = haritasiz(&ev, Some("nestjs-temel"), "b");
    let a = ayna(&q);
    let d = dom(&a, "haritasiz");
    assert!(d.contains("○ harita yok") && d.contains("sezgi init"), "haritasız tanıtım:\n{}", &d[..d.len().min(2000)]);
    // Harita sekmeleri gizli
    let i = d.find("id=\"sekme-kapilar\"").expect("Kapılar sekmesi");
    let etiket = &d[d[..i].rfind("<button").unwrap()..i + d[i..].find('>').unwrap()];
    assert!(etiket.contains("hidden"), "Kapılar sekmesi haritasızda gizli olmalı: {etiket}");
}
