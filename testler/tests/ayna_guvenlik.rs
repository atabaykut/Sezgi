//! Sezgi Ayna'nın kötü niyetli haritaya karşı güvenliği. Harita depoya işlenmiş bir `.sezgi/` klasöründen gelebilir:
//! içindeki her metin ve sayı yerine HTML/betik konabilir. Hiçbir betik çalışmamalı.
//!
//! Tarayıcı testi gerçek bir başsız tarayıcıyla (Chrome, Chromium ya da Edge) sayfayı yükler. Tarayıcı
//! `SEZGI_TARAYICI` ortam değişkeniyle ya da bilinen kurulum yollarından bulunur; bulunamazsa test kalır (atlanmaz).
use serde_json::json;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Ayna'nın adres satırındaki oturum anahtarı (porta göre); /api/ istekleri bunu taşır
static ANAHTARLAR: std::sync::Mutex<Vec<(u16, String)>> = std::sync::Mutex::new(Vec::new());
fn anahtari_sakla(port: u16, satir: &str) -> u16 {
    let a: String = satir.split("#anahtar=").nth(1).unwrap_or("").chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
    ANAHTARLAR.lock().unwrap().push((port, a));
    port
}
fn anahtar(port: u16) -> String {
    ANAHTARLAR.lock().unwrap().iter().rev().find(|(p, _)| *p == port).map(|(_, a)| a.clone()).unwrap_or_default()
}

/// Betik çalışırsa kök öğeye `data-xss` yazar
const YUK: &str = "<img src=x onerror=\"document.documentElement.setAttribute('data-xss','1')\"><script>document.documentElement.setAttribute('data-xss','2')</script>";

fn kotu_harita(klasor: &Path) {
    std::fs::create_dir_all(klasor).unwrap();
    let y = YUK;
    let alan = json!({ "tip": y, "zorunlu": y, "kurallar": [y], "satir": y, "degerler": [y] });
    let kapi = json!({
        "id": "GET /x-yol", "yontem": "GET", "yol": "/x-yol", "kontrolcu": y, "fonksiyon": y, "dosya": y, "satir": y,
        "giris": y, "giris_kaniti": y, "guardlar": [y], "roller": [y], "ozet": y,
        "govde": [{ "ad": y, "tip": y, "sema": { "dto": y, "dosya": y, "bulunamadi": false, "kalitim": y, "alanlar": { y: alan } } }],
        "sorgu": [{ "ad": y, "tip": y }], "parametreler": [{ "ad": y, "tip": y }],
        "kullanimdan_kalkmis": true, "yerine": y, "surum": y, "uygulama": y, "ana": false,
        "veri": { "okur": [y], "yazar": [y], "eksik_olabilir": true }
    });
    let harita = json!({
        "ozet": {
            "uretici": y, "adaptor": y, "uygulama": y, "kaynak_ref": y, "genel_onek": y, "genel_guardlar": [y], "ana_uygulama": y,
            "dosya": y, "satir": y, "ek_paketler": [y], "kismen_okunan_dosya": [y], "veri_semasi": [y], "dto_bulunamayan": [y],
            "sure_ms": { "toplam": y }
        },
        "endpointler": [kapi, { "id": "GET /ikinci", "yontem": "GET", "yol": "/ikinci", "kontrolcu": "B", "fonksiyon": "f", "dosya": "b.ts", "satir": 1,
            "giris": "gerekli", "giris_kaniti": "kesin", "guardlar": [], "roller": [], "govde": [], "sorgu": [], "parametreler": [], "ana": true }]
    });
    let kart = json!({
        "benlik": y, "operations": [{ "id": y }], "coverage": { "operations_complete": y },
        "signature": { "alg": y, "key_id": y, "value": y }
    });
    std::fs::write(klasor.join("kapilar.json"), harita.to_string()).unwrap();
    std::fs::write(klasor.join("benlik.json"), kart.to_string()).unwrap();
}

// Süreç çağırana döner; çağıran öldürüp bekler (kill + wait)
#[allow(clippy::zombie_processes)]
fn ayna_ac(klasor: &Path) -> (Child, u16) {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["ayna", klasor.to_str().unwrap(), "--port", "0"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut r = std::io::BufReader::new(p.stdout.take().unwrap());
    let mut satir = String::new();
    loop {
        satir.clear();
        if r.read_line(&mut satir).unwrap() == 0 {
            let _ = p.kill();
            let _ = p.wait();
            panic!("Ayna adres yazmadı");
        }
        if let Some(i) = satir.find("http://127.0.0.1:") {
            let port = anahtari_sakla(satir[i + 17..].split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap(), &satir);
            return (p, port);
        }
    }
}

fn http(port: u16, yol: &str) -> String {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    write!(s, "GET {yol} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Sezgi-Anahtar: {}\r\n\r\n", anahtar(port)).unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    c
}

#[test]
fn ayna_kaynaklari_satir_ici_betik_icermez() {
    let h = std::env::temp_dir().join(format!("sezgi-ayna-statik-{}", std::process::id()));
    kotu_harita(&h);
    let (mut p, port) = ayna_ac(&h);
    let sayfa = http(port, "/");
    let betik = http(port, "/ayna.js");
    let _ = p.kill();
    let _ = p.wait();
    let _ = std::fs::remove_dir_all(&h);
    let (bas, govde) = sayfa.split_once("\r\n\r\n").unwrap();
    let csp = bas.lines().find(|l| l.to_ascii_lowercase().starts_with("content-security-policy:")).expect("CSP yok");
    assert!(!csp.contains("unsafe-inline") && !csp.contains("unsafe-eval"), "{csp}");
    assert!(csp.contains("script-src 'self'"), "{csp}");
    // Sayfada satır içi betik, satır içi stil ya da olay işleyicisi yok
    let kucuk = govde.to_lowercase();
    for parca in kucuk.split("<script").skip(1) {
        assert!(parca.split('>').next().unwrap().contains("src="), "satır içi betik var");
    }
    assert!(!kucuk.contains("<style") && !kucuk.contains(" style=") && !kucuk.contains(" onclick=") && !kucuk.contains(" onerror="));
    // Betik HTML metni üretmez: dinamik içerik yalnızca DOM API ve textContent ile
    assert!(betik.starts_with("HTTP/1.1 200") && betik.contains("javascript"), "{}", &betik[..betik.len().min(200)]);
    for yasak in ["innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval(", "new Function"] {
        assert!(!betik.contains(yasak), "ayna.js içinde {yasak}");
    }
}

/// Başsız tarayıcı: SEZGI_TARAYICI ya da bilinen yollar
fn tarayici() -> PathBuf {
    if let Some(t) = std::env::var_os("SEZGI_TARAYICI") {
        return PathBuf::from(t);
    }
    let adaylar = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    adaylar.iter().map(PathBuf::from).find(|p| p.exists()).expect("başsız tarayıcı bulunamadı: SEZGI_TARAYICI=<chrome|chromium|msedge yolu> verin")
}

#[test]
fn ayna_kotu_niyetli_haritada_betik_calistirmaz() {
    let h = std::env::temp_dir().join(format!("sezgi-ayna-tarayici-{}", std::process::id()));
    kotu_harita(&h);
    let profil = std::env::temp_dir().join(format!("sezgi-ayna-profil-{}", std::process::id()));
    let (mut p, port) = ayna_ac(&h);
    // Ayrıntı ekranı da çizilsin diye kapı adresin sonunda (#) seçilir
    let adres = format!("http://127.0.0.1:{port}/#anahtar={}&GET%20%2Fx-yol", anahtar(port));
    let cikti = Command::new(tarayici())
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
    let _ = p.kill();
    let _ = p.wait();
    let _ = std::fs::remove_dir_all(&h);
    let _ = std::fs::remove_dir_all(&profil);
    let dom = String::from_utf8_lossy(&cikti.stdout).to_string();
    // Sayfa gerçekten çizildi: kapı listesi ve ayrıntı ekranı yüklendi (test boşuna geçmesin)
    assert!(dom.contains("/x-yol") && dom.contains("/ikinci"), "sayfa çizilmedi:\n{}", &dom[..dom.len().min(2000)]);
    assert!(dom.contains("Önizle"), "ayrıntı ekranı çizilmedi");
    // Hiçbir betik çalışmadı, yük öğe olarak değil metin olarak duruyor
    let kok = dom.split('>').next().unwrap_or("");
    assert!(!kok.contains("data-xss"), "betik çalıştı: {kok}");
    assert!(!dom.contains("<img") && !dom.contains("<script>document"), "yük HTML öğesi olarak yerleşti");
    assert!(dom.contains("&lt;img src=x"), "yük metin olarak görünmüyor");
}

#[test]
fn yandaki_anahtarla_dogrulanan_imza_guvenilir_sayilmaz() {
    // Kartı ve anahtarı birlikte getiren biri (depoya işlenmiş .sezgi/) yeşil rozet almamalı
    let ayar = std::env::temp_dir().join(format!("sezgi-ayna-imza-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ayar);
    let sezgi = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.env("APPDATA", &ayar).env("XDG_CONFIG_HOME", &ayar);
        c
    };
    assert!(sezgi().args(["anahtar", "olustur", "--ad", "saldirgan"]).output().unwrap().status.success());
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel");
    let h = ayar.join("harita");
    assert!(sezgi().args(["init", kok.to_str().unwrap(), "--cikti", h.to_str().unwrap(), "--imzala", "saldirgan"]).output().unwrap().status.success());
    let (mut p, port) = ayna_ac(&h);
    let imza = http(port, "/api/imza");
    let betik = http(port, "/ayna.js");
    let _ = p.kill();
    let _ = p.wait();
    let _ = std::fs::remove_dir_all(&ayar);
    let govde = imza.split_once("\r\n\r\n").unwrap().1;
    let v: serde_json::Value = serde_json::from_str(govde).unwrap();
    assert_eq!(v["durum"], "tutarli_anahtar_dogrulanmadi", "{v}");
    // Rozet yeşil (iyi) olmamalı; metin anahtarın doğrulanmadığını söylemeli
    let satir = betik.lines().find(|l| l.contains("tutarli_anahtar_dogrulanmadi:")).expect("rozet eşlemesi yok");
    assert!(!satir.contains("\"iyi\"") && betik.contains("anahtar doğrulanmadı") && betik.contains("key not verified"), "{satir}");
}

#[test]
fn yavas_istemci_baglantiyi_suresiz_tutamaz() {
    // Başlığı saniyede birkaç baytla gönderen istemci (slowloris): bağlantı başına toplam süre sınırı olmalı
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel");
    let h = std::env::temp_dir().join(format!("sezgi-ayna-yavas-{}", std::process::id()));
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", h.to_str().unwrap()]).output().unwrap().status.success());
    let (mut p, port) = ayna_ac(&h);
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(b"GET / HTTP/1.1\r\nX-Yavas: ").unwrap();
    let bas = std::time::Instant::now();
    let mut kapandi = None;
    while bas.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(400));
        if s.write_all(b"a").is_err() {
            kapandi = Some(bas.elapsed());
            break;
        }
        // Sunucu kapattıysa okuma 0 ya da hata döner
        s.set_read_timeout(Some(std::time::Duration::from_millis(10))).unwrap();
        let mut b = [0u8; 64];
        match s.read(&mut b) {
            Ok(0) => {
                kapandi = Some(bas.elapsed());
                break;
            }
            Ok(_) => {
                kapandi = Some(bas.elapsed());
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => {
                kapandi = Some(bas.elapsed());
                break;
            }
        }
    }
    let _ = p.kill();
    let _ = p.wait();
    let _ = std::fs::remove_dir_all(&h);
    let sure = kapandi.expect("yavaş istemcinin bağlantısı 20 saniyede kapanmadı");
    assert!(sure < std::time::Duration::from_secs(15), "{sure:?}");
}

#[test]
fn rapor_sayi_alanlarina_metin_tasimaz() {
    // Savunma katmanı: haritadaki sayı alanlarına konmuş metin rapora (ve Ayna'ya) taşınmaz
    let h = std::env::temp_dir().join(format!("sezgi-rapor-tur-{}", std::process::id()));
    kotu_harita(&h);
    let d = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["rapor", h.to_str().unwrap(), "--json"]).output().unwrap();
    let _ = std::fs::remove_dir_all(&h);
    assert!(d.status.success(), "{}", String::from_utf8_lossy(&d.stderr));
    let r: serde_json::Value = serde_json::from_slice(&d.stdout).unwrap();
    for (ad, v) in [("okunan.dosya", &r["okunan"]["dosya"]), ("okunan.satir", &r["okunan"]["satir"]), ("sure_ms", &r["sure_ms"])] {
        assert!(v.is_null() || v.is_u64(), "{ad} sayı değil: {v}");
    }
}

#[test]
fn ayna_standart_cikti_kapaninca_cokmez() {
    // Standart çıktı borusu Ayna daha hiçbir şey yazmadan kapatılır: println! panik verip sunucuyu düşürmemeli.
    // Adres çıktıdan okunamayacağı için sabit bir port kullanılır.
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel");
    let h = std::env::temp_dir().join(format!("sezgi-ayna-boru-{}", std::process::id()));
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", h.to_str().unwrap()]).output().unwrap().status.success());
    let port = 41000 + (std::process::id() % 2000) as u16;
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["ayna", h.to_str().unwrap(), "--port", &port.to_string()]).stdout(Stdio::piped()).spawn().unwrap();
    drop(p.stdout.take());
    let mut cevap = String::new();
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if p.try_wait().unwrap().is_some() {
            break;
        }
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            cevap = http(port, "/");
            break;
        }
    }
    let calisiyor = p.try_wait().unwrap().is_none();
    let _ = p.kill();
    let _ = p.wait();
    let _ = std::fs::remove_dir_all(&h);
    assert!(calisiyor, "Ayna standart çıktı kapanınca çöktü");
    assert!(cevap.starts_with("HTTP/1.1 200"), "{cevap}");
}

/// Geçici yapılandırma klasörüyle sezgi komutu (anahtarlar, sabitlenmiş ve görülen anahtarlar burada)
fn sezgi_ayarli(ayar: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
    c.env("APPDATA", ayar).env("XDG_CONFIG_HOME", ayar);
    c
}

/// Ayna'yı ayarlı ortamda açıp /api/imza durumunu döner
fn imza_durumu(ayar: &Path, harita: &Path) -> serde_json::Value {
    let mut p = sezgi_ayarli(ayar).args(["ayna", harita.to_str().unwrap(), "--port", "0"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut r = std::io::BufReader::new(p.stdout.take().unwrap());
    let mut satir = String::new();
    let port: u16 = loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0, "Ayna adres yazmadı");
        if let Some(i) = satir.find("http://127.0.0.1:") {
            break anahtari_sakla(satir[i + 17..].split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap(), &satir);
        }
    };
    let c = http(port, "/api/imza");
    let _ = p.kill();
    let _ = p.wait();
    serde_json::from_str(c.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[test]
fn guven_capasi_sabitlenmis_anahtar_ve_degisen_anahtar() {
    let ayar = std::env::temp_dir().join(format!("sezgi-guven-capasi-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ayar);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel");
    for ad in ["sahip", "saldirgan"] {
        assert!(sezgi_ayarli(&ayar).args(["anahtar", "olustur", "--ad", ad]).output().unwrap().status.success());
    }
    let (h_sahip, h_saldirgan) = (ayar.join("h-sahip"), ayar.join("h-saldirgan"));
    for (h, ad) in [(&h_sahip, "sahip"), (&h_saldirgan, "saldirgan")] {
        assert!(sezgi_ayarli(&ayar).args(["init", kok.to_str().unwrap(), "--cikti", h.to_str().unwrap(), "--imzala", ad]).output().unwrap().status.success());
    }
    let kart = |h: &PathBuf| [h.join("benlik.json").to_str().unwrap().to_string(), h.join("benlik-keys.json").to_str().unwrap().to_string()];

    // Sabitleme yokken: tutarlı ama anahtar doğrulanmadı
    assert_eq!(imza_durumu(&ayar, &h_sahip)["durum"], "tutarli_anahtar_dogrulanmadi");

    // İlk kullanımda güven: dogrula sahibin anahtarını görür; sonra aynı uygulamanın kartı başka anahtarla gelirse uyarır
    let d = sezgi_ayarli(&ayar).arg("dogrula").args(kart(&h_sahip)).output().unwrap();
    assert!(d.status.success(), "{}", String::from_utf8_lossy(&d.stdout));
    let d = sezgi_ayarli(&ayar).arg("dogrula").args(kart(&h_saldirgan)).output().unwrap();
    let cikti = String::from_utf8_lossy(&d.stdout).to_string();
    assert!(cikti.contains("anahtar değişti"), "{cikti}");
    assert_eq!(imza_durumu(&ayar, &h_saldirgan)["durum"], "anahtar_degisti");

    // Sahibin anahtarı sabitlenir: sahibin kartı güvenilir, saldırganınki reddedilir
    let d = sezgi_ayarli(&ayar).args(["anahtar", "sabitle"]).args(kart(&h_sahip)).output().unwrap();
    assert!(d.status.success(), "{}{}", String::from_utf8_lossy(&d.stdout), String::from_utf8_lossy(&d.stderr));
    assert_eq!(imza_durumu(&ayar, &h_sahip)["durum"], "guvenilir");
    assert_eq!(imza_durumu(&ayar, &h_saldirgan)["durum"], "sabitlenmis_anahtarla_uyusmuyor");
    assert!(!sezgi_ayarli(&ayar).arg("dogrula").args(kart(&h_saldirgan)).output().unwrap().status.success(), "sabitlenmiş anahtarla uyuşmayan kart doğrulandı");
    // Sabitleme imzası doğrulanmayan kartla yapılamaz
    let bozuk = ayar.join("bozuk.json");
    let mut k: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(h_sahip.join("benlik.json")).unwrap()).unwrap();
    k["app"]["name"] = serde_json::json!("degisti");
    std::fs::write(&bozuk, k.to_string()).unwrap();
    assert!(!sezgi_ayarli(&ayar).args(["anahtar", "sabitle", bozuk.to_str().unwrap(), h_sahip.join("benlik-keys.json").to_str().unwrap()]).output().unwrap().status.success());
    let _ = std::fs::remove_dir_all(&ayar);
}
