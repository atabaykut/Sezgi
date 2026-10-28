//! D9 dış etki çağrı grafiği üzerinden (S-1): işlemin zincirindeki polimorfik dağıtım (`kanal.bildirim_gonder` ->
//! `Slack.bildirim_gonder` -> `curl.istek` -> pycurl) ve takma adla içe aktarılan kütüphane (`EmailMultiAlternatives as
//! Mesaj`) dış etki olarak bulunur; zincirde sınıflandırılamayan kütüphane çağrısı olan yazma "dış etki bilinmiyor"
//! gerekçesiyle onaysız gidemez (fail-closed), okumada yalnızca bilgidir. Yalnızca veritabanına yazan işlem değişmez.
//! Örnek: `dis2_bildirim` (Django; depo adına özel kural yok).
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d9dis2-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", ornek(ad).to_str().unwrap(), "--cikti", cikti.to_str().unwrap(), "--paylasimsiz"])
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    assert!(c.status.success(), "init: {}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    cikti
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn kapi(h: &Value, id: &str) -> Value {
    h["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).cloned().unwrap_or_else(|| {
        let ids: Vec<&str> = h["endpointler"].as_array().unwrap().iter().filter_map(|k| k["id"].as_str()).collect();
        panic!("{id} yok: {ids:?}")
    })
}

fn dna(h: &Value, id: &str) -> Value {
    let k = kapi(h, id);
    let yer = format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]);
    h["ozet"]["dna"].get(&yer).cloned().unwrap_or_else(|| panic!("{id} ({yer}) için dna yok"))
}

fn dis_hedefi<'a>(d: &'a Value, hedef: &str) -> &'a Value {
    d["dis"].as_array().unwrap_or_else(|| panic!("dış etki yok: {d:#}")).iter().find(|x| x["hedef"] == hedef).unwrap_or_else(|| panic!("{hedef} yok: {d:#}"))
}

fn arac(harita: &Path, ad: &str, arguman: Value, en: bool) -> String {
    use std::io::Write;
    let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
    if en {
        c.args(["--lang", "en"]);
    }
    let mut p = c.args(["mcp", harita.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

#[test]
fn polimorfik_bildirim_cagri_grafiginden_dis_etki() {
    let h = haritala("dis2_bildirim");
    let m = json_oku(&h.join("kapilar.json"));
    let d = dna(&m, "POST /kanallar/:code/dene/");
    // Tipi bilinmeyen alıcıda aynı adlı yöntemler aday kümesi: Slack -> HttpTasima.gonder -> curl.istek -> pycurl
    let http = dis_hedefi(&d, "pycurl");
    assert_eq!(http["tur"], "http", "{http:#}");
    assert_eq!(http["guven"], "inferred", "polimorfik aday hiçbir zaman certain değil: {http:#}");
    assert!(http["kanit"].as_str().unwrap().starts_with("ortak/curl.py:"), "{http:#}");
    let zincir = http["zincir"].to_string();
    assert!(zincir.contains("DenemeBildirimi.post") && zincir.contains("istek"), "{zincir}");
    // E-posta taşıması da aday: takma adla içe aktarılan Django posta sınıfı
    let posta = dis_hedefi(&d, "django.core.mail");
    assert_eq!(posta["tur"], "eposta", "{posta:#}");
    assert_eq!(d["dis_etki"], "var", "{d:#}");
    let k = kapi(&m, "POST /kanallar/:code/dene/");
    assert!(k["etki_onayi"].as_str().unwrap_or("").contains("dışına etki"), "dış etkili yazma onay ister: {k:#}");
}

#[test]
fn takma_adli_posta_kutuphanesi_ve_yalniz_veritabani_yazmasi() {
    let h = haritala("dis2_bildirim");
    let m = json_oku(&h.join("kapilar.json"));
    let d = dna(&m, "POST /giris/");
    assert_eq!(dis_hedefi(&d, "django.core.mail")["tur"], "eposta", "{d:#}");
    assert!(kapi(&m, "POST /giris/")["etki_onayi"].is_string());
    // Yalnızca ORM'ye yazan işlem: bütün çağrılar sınıflandırıldı, dış etki yok, onay gerekçesi eklenmez
    let a = dna(&m, "POST /kanallar/:code/ad/");
    assert!(a["dis"].is_null(), "{a:#}");
    assert_eq!(a["dis_etki"], "bulunmadi", "{a:#}");
    let ka = kapi(&m, "POST /kanallar/:code/ad/");
    assert!(ka["etki_onayi"].is_null() && ka["dis_etki"].is_null(), "{ka:#}");
}

#[test]
fn siniflandirilamayan_kutuphane_yazmada_onay_okumada_bilgi() {
    let h = haritala("dis2_bildirim");
    let m = json_oku(&h.join("kapilar.json"));
    let y = dna(&m, "POST /rapor/");
    assert_eq!(y["dis_etki"], "bilinmiyor", "{y:#}");
    let n = &y["dis_etki_bilinmeyen"][0];
    assert_eq!(n["tur"], "kutuphane", "{y:#}");
    assert_eq!(n["modul"], "grafikcizer", "{y:#}");
    let k = kapi(&m, "POST /rapor/");
    assert_eq!(k["dis_etki"], "bilinmiyor", "{k:#}");
    let g = k["etki_onayi"].as_str().unwrap_or_else(|| panic!("bilinmeyen dış etkili yazma onay ister: {k:#}"));
    assert!(g.starts_with("işlemin dış etkisi bilinmiyor (sınıflandırılamayan kütüphane çağrısı: grafikcizer.yayinla"), "{g}");
    // Okuma: bilgi olarak bilinmiyor, onay gerekçesi yok
    let o = dna(&m, "GET /rapor/");
    assert_eq!(o["dis_etki"], "bilinmiyor", "{o:#}");
    let ko = kapi(&m, "GET /rapor/");
    assert!(ko["etki_onayi"].is_null() && ko["dis_etki"].is_null(), "{ko:#}");
    // Uygulama düzeyi sayılar
    let ist = &m["ozet"]["dis"]["istatistik"];
    assert_eq!(ist["dis_etki_bilinmiyor_yazma"], 1, "{ist:#}");
    assert_eq!(ist["dis_etki_bilinmiyor_okuma"], 1, "{ist:#}");
    // Kart: yazma onay ister, okuma istemez
    let kart = json_oku(&h.join("benlik.json"));
    let op = |y: &str, p: &str| kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == y && o["entry"]["path"] == p).cloned().unwrap();
    assert_eq!(op("POST", "/rapor/")["requires_approval"], true);
    assert_ne!(op("GET", "/rapor/")["requires_approval"], true);
}

#[test]
fn bilinmeyen_dis_etkili_yazma_canli_modda_onaya_duser() {
    let ev = Ev::yeni("d9-dis2-politika");
    let u = Uygulama::yeni();
    let p = ev.proje("dis2_bildirim", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["*:yaz"], &["--tum-islemler"]);
    let mut mcp = p.mcp(&t);
    let (s, _) = mcp.cagir(json!({ "kapi": "POST /rapor/", "govde": { "baslik": "haftalık" } }));
    assert!(s.contains("ONAY BEKLİYOR") && s.contains("dış etkisi bilinmiyor"), "bilinmeyen dış etkili yazma onaysız gitmez: {s}");
    assert_eq!(u.sayi(), 0, "istek uygulamaya gitmedi");
}

#[test]
fn tani_etki_rapor_ve_ingilizce_gerekce() {
    let h = haritala("dis2_bildirim");
    let t = arac(&h, "tani", json!({ "kapi": "POST /rapor/" }), false);
    assert!(t.contains("dış etki: bilinmiyor") && t.contains("sınıflandırılamayan kütüphane çağrısı: grafikcizer.yayinla"), "{t}");
    let te = arac(&h, "tani", json!({ "kapi": "POST /rapor/" }), true);
    assert!(te.contains("external effect: unknown") && te.contains("unclassified library call: grafikcizer.yayinla"), "{te}");
    let td = arac(&h, "tani", json!({ "kapi": "POST /kanallar/:code/dene/" }), false);
    assert!(td.contains("SDK") || td.contains("pycurl"), "{td}");
    assert!(td.contains("← DenemeBildirimi.post"), "zincirle: {td}");
    let e = arac(&h, "etki", json!({ "kapi": "POST /rapor/" }), false);
    assert!(e.contains("dış etki bilinmiyor"), "{e}");
    let r = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["rapor", h.to_str().unwrap()]).output().unwrap();
    let r = String::from_utf8_lossy(&r.stdout).to_string();
    assert!(r.contains("dış etkisi bilinmeyen: 1 yazma (onay ister) · 1 okuma"), "{r}");
    let re = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["--lang", "en", "report", h.to_str().unwrap()]).output().unwrap();
    let re = String::from_utf8_lossy(&re.stdout).to_string();
    assert!(re.contains("external effect unknown: 1 write (need approval) · 1 read (information)"), "{re}");
}

/// Ayna: (port, anahtar); süreç dönen değerle birlikte yaşar
#[allow(clippy::zombie_processes)]
fn ayna(p: &Proje) -> (std::process::Child, u16, String) {
    use std::io::BufRead;
    let mut c = p.komut(&["ayna", "--port", "0"]).env_remove("SEZGI_LANG").stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let mut r = std::io::BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0, "Ayna adres yazmadı");
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let anahtar = k.split("#anahtar=").nth(1).unwrap().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            return (c, port, anahtar);
        }
    }
}

fn iste(port: u16, anahtar: &str, yontem: &str, yol: &str, govde: &str) -> Value {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        s,
        "{yontem} {yol} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Sezgi-Anahtar: {anahtar}\r\n\r\n{govde}",
        govde.len()
    )
    .unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    let g = c.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("");
    let g = &g[g.find('{').unwrap_or(0)..g.rfind('}').map(|i| i + 1).unwrap_or(g.len())];
    serde_json::from_str(g).unwrap_or_else(|e| panic!("{e}: {g}"))
}

#[test]
fn ayna_islem_ayrintisi_ve_sorular() {
    let ev = Ev::yeni("d9-dis2-ayna");
    let p = ev.proje("dis2_bildirim", "a");
    let (mut c, port, anahtar) = ayna(&p);
    let sorular = iste(port, &anahtar, "GET", "/api/sorular", "");
    let rapor = iste(port, &anahtar, "POST", "/api/islem", &json!({ "kapi": "POST /rapor/" }).to_string());
    let dene = iste(port, &anahtar, "POST", "/api/islem", &json!({ "kapi": "POST /kanallar/:code/dene/" }).to_string());
    let rapor_en = iste(port, &anahtar, "POST", "/api/islem?lang=en", &json!({ "kapi": "POST /rapor/" }).to_string());
    let _ = c.kill();
    let _ = c.wait();
    let soru = |id: &str| sorular["sorular"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap().to_string();
    // 12: dış etkisi bilinmeyen işlemler ayrı madde (yazma sayısıyla)
    assert!(soru("dis").contains("2 işlemin dış etkisi bilinmiyor (1 yazma onay ister)"), "{}", soru("dis"));
    // 14: bilinmeyen dış etkili yazma ayrı sayılır
    assert!(soru("onay").contains("1 yazma işlemi her seferinde insan onayı ister: dış etkisi bilinmiyor"), "{}", soru("onay"));
    // İşlem ayrıntısı: "Uygulamanın dışına etki eder mi?" bilinmezliği ve dış etkinin zinciri
    let bolum = |v: &Value| v.to_string();
    assert!(bolum(&rapor).contains("Dış etkisi bilinmiyor") && bolum(&rapor).contains("grafikcizer.yayinla"), "{rapor:#}");
    assert!(bolum(&rapor_en).contains("External effect unknown"), "{rapor_en:#}");
    assert!(bolum(&dene).contains("DenemeBildirimi.post ›"), "zincirle: {dene:#}");
}
