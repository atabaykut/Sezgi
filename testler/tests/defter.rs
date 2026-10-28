//! Veri defteri (vizyon 9.6) ve telemetrisiz tanılama (12.4): müşteri verisi yazılmaz, işletim kaydının saklama süresi
//! uygulanır, bir kişinin silme talebi kaydın doğrulanabilirliğini bozmadan kapanır, kart ne tutulduğunu söyler,
//! tanılama yalnızca sayaç tutar.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const GIZLI: &str = "GIZLI-MUSTERI-DEGERI-7731";

/// Her isteğe gövdesinde müşteri verisi olan bir cevap veren sahte uygulama
fn sahte_uygulama() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut boy = 0usize;
            let mut satir = String::new();
            r.read_line(&mut satir).unwrap();
            loop {
                let mut l = String::new();
                r.read_line(&mut l).unwrap();
                if l.trim().is_empty() {
                    break;
                }
                if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                    boy = v.trim().parse().unwrap();
                }
            }
            let mut g = vec![0u8; boy];
            r.read_exact(&mut g).unwrap();
            let cevap = format!("{{\"musteri\":\"{GIZLI}\"}}");
            let mut s = s;
            write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cevap}", cevap.len()).unwrap();
            let _ = s.shutdown(std::net::Shutdown::Write);
        }
    });
    port
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
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
}

fn jwt(eposta: &str) -> String {
    use base64::Engine;
    let y = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!("{{\"email\":\"{eposta}\"}}"));
    format!("eyJhbGciOiJub25lIn0.{y}.x")
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-defter-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let ev = kok.join("ev");
        std::fs::create_dir_all(ev.join("ayar")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &kok.join("app"));
        let o = Ortam { kok, ev };
        o.ok(&["init", "."]);
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
        let mut p = self.komut(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
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
    fn uygulama_klasoru(&self) -> PathBuf {
        std::fs::read_dir(self.ev.join("ayar").join("sezgi").join("uygulamalar")).unwrap().next().unwrap().unwrap().path()
    }
    /// Yazma modu, oturum (verilen kişi), token; MCP'yle verilen araç çağrıları
    fn canli(&self, eposta: &str, cagrilar: &[serde_json::Value]) -> Vec<String> {
        let port = sahte_uygulama();
        self.ok(&["hedef", &format!("http://127.0.0.1:{port}"), "--test-kopyasi"]);
        self.ok(&["mod", "yazma"]);
        assert!(self.sezgi(&["oturum", "ac"], Some(&format!("{}\n", jwt(eposta)))).status.success());
        let _ = self.sezgi(&["token", "iptal", "ajan"], None);
        let m = self.ok(&["token", "olustur", "--ad", "ajan", "--izin", "kitap.*:yikici", "--izin", "yorum.*:yaz"]);
        let token = m.split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
        let mut p = self.komut(&["mcp", ".sezgi"]).env("SEZGI_TOKEN", &token).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let mut stdin = p.stdin.take().unwrap();
        let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
        let mut out = vec![];
        for (i, a) in cagrilar.iter().enumerate() {
            let istek = serde_json::json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": a["arac"], "arguments": a["arg"] } });
            writeln!(stdin, "{istek}").unwrap();
            stdin.flush().unwrap();
            let v: serde_json::Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
            out.push(v["result"]["content"][0]["text"].as_str().unwrap().to_string());
        }
        drop(stdin);
        let _ = p.wait();
        out
    }
}

fn hepsini_oku(k: &Path) -> String {
    let mut s = String::new();
    for g in std::fs::read_dir(k).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            s.push_str(&hepsini_oku(&p));
        } else {
            s.push_str(&String::from_utf8_lossy(&std::fs::read(&p).unwrap()));
        }
    }
    s
}

fn ozet(b: &[u8]) -> String {
    use shake::{ExtendableOutput, Shake256, Update, XofReader};
    let mut h = Shake256::default();
    h.update(b);
    let mut r = h.finalize_xof();
    let mut o = [0u8; 32];
    r.read(&mut o);
    o.iter().map(|x| format!("{x:02x}")).collect()
}

/// İlk `n` kayıt satırının zamanını eskitir ve zinciri Sezgi'nin kuralıyla yeniden özetler
/// (özet: `ozet` ve `insan` alanları çıkarılmış satır; kişinin kimliği yalnızca `insan_ozet` ile zincire girer)
/// Kayıt anahtarı (testler anahtarlığı kapatır: yapılandırma klasöründeki dosya)
fn anahtar(kayit: &Path) -> ring::aead::LessSafeKey {
    let s = std::fs::read_to_string(kayit.with_file_name("kayit.anahtar")).unwrap();
    let k: Vec<u8> = (0..32).map(|i| u8::from_str_radix(&s.trim()[i * 2..i * 2 + 2], 16).unwrap()).collect();
    ring::aead::LessSafeKey::new(ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &k).unwrap())
}

/// Şifreli kayıt satırını çözer (Sezgi'nin biçimi: {"s":"c20p1","v":base64(sayı ‖ şifreli ‖ etiket)})
fn coz(kayit: &Path, l: &str) -> serde_json::Value {
    use base64::Engine;
    let v: serde_json::Value = serde_json::from_str(l).unwrap();
    assert_eq!(v["s"], "c20p1", "satır şifreli olmalı: {l}");
    let mut b = base64::engine::general_purpose::STANDARD.decode(v["v"].as_str().unwrap()).unwrap();
    let mut govde = b.split_off(12);
    let tek = ring::aead::Nonce::assume_unique_for_key(b.try_into().unwrap());
    let duz = anahtar(kayit).open_in_place(tek, ring::aead::Aad::from(&b"c20p1"[..]), &mut govde).unwrap().to_vec();
    serde_json::from_slice(&duz).unwrap()
}

fn sifrele(kayit: &Path, v: &serde_json::Value) -> String {
    use base64::Engine;
    let tek: [u8; 12] = std::array::from_fn(|i| i as u8 ^ 0x5a);
    let mut b = serde_json::to_vec(v).unwrap();
    anahtar(kayit).seal_in_place_append_tag(ring::aead::Nonce::assume_unique_for_key(tek), ring::aead::Aad::from(&b"c20p1"[..]), &mut b).unwrap();
    let mut yuk = tek.to_vec();
    yuk.extend(b);
    serde_json::json!({ "s": "c20p1", "v": base64::engine::general_purpose::STANDARD.encode(yuk) }).to_string()
}

/// Kaydın çözülmüş metni
fn duz(kayit: &Path) -> String {
    std::fs::read_to_string(kayit)
        .unwrap()
        .lines()
        .map(|l| {
            coz(kayit, l).to_string()
                + "
"
        })
        .collect()
}

/// Kayıt mührü (Sezgi'nin biçimi): kayıt anahtarıyla HMAC-SHA256("sezgi-kayit|<etiket>|<değer>")
fn muhur(kayit: &Path, etiket: &str, deger: &str) -> String {
    let s = std::fs::read_to_string(kayit.with_file_name("kayit.anahtar")).unwrap();
    let k: Vec<u8> = (0..32).map(|i| u8::from_str_radix(&s.trim()[i * 2..i * 2 + 2], 16).unwrap()).collect();
    let a = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &k);
    ring::hmac::sign(&a, format!("sezgi-kayit|{etiket}|{deger}").as_bytes()).as_ref().iter().map(|x| format!("{x:02x}")).collect()
}

/// Zinciri yeniden özetler; `anahtarli` ise satırları ve son satırı kayıt anahtarıyla yeniden mühürler (Sezgi'nin
/// kendi yazdığı geçerli bir kayıt gibi), değilse mühürler eski kalır (anahtarı bilmeyen bir sürecin kurcalaması)
fn yeniden_zincirle(kayit: &Path, anahtarli: bool, mut degistir: impl FnMut(usize, &mut serde_json::Value)) {
    let mut onceki = String::new();
    let mut out = String::new();
    let mut son = (0u64, String::new());
    for (i, l) in std::fs::read_to_string(kayit).unwrap().lines().enumerate() {
        let mut v = coz(kayit, l);
        degistir(i, &mut v);
        v["onceki"] = serde_json::json!(onceki);
        let mut k = v.clone();
        for alan in ["ozet", "muhur", "insan"] {
            k.as_object_mut().unwrap().remove(alan);
        }
        let o = ozet(serde_json::to_string(&k).unwrap().as_bytes());
        v["ozet"] = serde_json::json!(o);
        if anahtarli {
            v["muhur"] = serde_json::json!(muhur(kayit, "satir", &o));
        }
        son = (v["sira"].as_u64().unwrap(), o.clone());
        onceki = o;
        out.push_str(&sifrele(kayit, &v));
        out.push('\n');
    }
    std::fs::write(kayit, out).unwrap();
    if anahtarli {
        let deger = format!("{}|{}", son.0, son.1);
        let j = serde_json::json!({ "sira": son.0, "ozet": son.1, "muhur": muhur(kayit, "son", &deger) });
        std::fs::write(kayit.with_file_name("kayit_son.json"), j.to_string()).unwrap();
    }
}

/// İlk n satırın zamanını eskitir ve zinciri yeniden özetleyip mühürler (geçerli ama eski bir kayıt); satırlar şifreli kalır
fn eskit(kayit: &Path, n: usize, zaman: &str) {
    yeniden_zincirle(kayit, true, |i, v| {
        if i < n {
            v["zaman"] = serde_json::json!(zaman);
        }
    });
}

fn yaz_cagrisi() -> serde_json::Value {
    serde_json::json!({ "arac": "cagir", "arg": { "kapi": "POST /api/v1/yorumlar", "govde": { "metin": "Gizli Başlık Değeri", "puan": 4, "kitapId": "8a3c1e2b-1111-2222-3333-444455556666" } } })
}

#[test]
fn musteri_verisi_diske_yazilmaz() {
    let o = Ortam::yeni("musteri");
    let sil = serde_json::json!({ "arac": "cagir", "arg": { "kapi": "DELETE /api/v1/kitaplar/3" } });
    let c = o.canli("kisi@ornek.test", &[yaz_cagrisi(), sil]);
    assert!(c[0].contains(GIZLI), "cevap ajana verilir: {}", c[0]);
    let id = c[1].split("Onay kimliği: ").nth(1).unwrap().split_whitespace().next().unwrap().to_string();
    o.ok(&["onay", "ver", &id, "--ozet", &o.onay_ozeti(&id)]);
    // Yapılandırma klasöründe ne parametre değeri ne cevap gövdesi var
    let disk = hepsini_oku(&o.ev.join("ayar").join("sezgi"));
    assert!(!disk.contains("Gizli Başlık Değeri"), "parametre değeri diske yazılmış");
    assert!(!disk.contains(GIZLI), "cevap gövdesi diske yazılmış");
}

#[test]
fn kisinin_silme_talebi_kaydi_bozmadan_kapanir() {
    let o = Ortam::yeni("unut");
    o.canli("kisi@ornek.test", &[yaz_cagrisi()]);
    o.canli("baska@example.com", &[serde_json::json!({ "arac": "cagir", "arg": { "kapi": "GET /api/v1/kitaplar" } })]);
    let kayit = o.uygulama_klasoru().join("kayit.jsonl");
    // Diskte kimlik okunmaz (şifreli); çözülünce görünür
    assert!(!std::fs::read_to_string(&kayit).unwrap().contains("kisi@ornek.test"));
    assert!(duz(&kayit).contains("kisi@ornek.test"));
    let m = o.ok(&["unut", "--kisi", "kisi@ornek.test"]);
    assert!(m.contains("silindi"), "{m}");
    let metin = duz(&kayit);
    assert!(!metin.contains("kisi@ornek.test"), "kişinin kimliği kayıtta kalmamalı");
    assert!(metin.contains("baska@example.com"), "başkasının kaydı kalır");
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam"), "silme zinciri bozmamalı");
    assert!(metin.contains("\"durum\":\"unut\""), "silmenin kendisi kayda geçer");
}

#[test]
fn saklama_suresi_dolan_kayit_silinir_zincir_saglam_kalir() {
    let o = Ortam::yeni("saklama");
    o.canli("kisi@ornek.test", &[yaz_cagrisi()]);
    let kayit = o.uygulama_klasoru().join("kayit.jsonl");
    // İlk satırları 40 gün öncesine çek (yeniden özetleyerek: geçerli ama eski bir zincir)
    let satirlar: Vec<serde_json::Value> = std::fs::read_to_string(&kayit).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let n = satirlar.len();
    assert!(n >= 3);
    eskit(&kayit, 2, "2020-01-01T00:00:00Z");
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam"), "yeniden özetlenen zincir geçerli olmalı");
    // Süre 30 gün: bir sonraki kayıtta eski iki satır silinir, zincir çapayla doğrulanır
    o.ok(&["mod", "okuma"]);
    let kalan = std::fs::read_to_string(&kayit).unwrap().lines().count();
    assert_eq!(kalan, n - 2 + 1, "iki eski satır silinmeli, mod satırı eklenmeli");
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam"));
    // Süre ayarı 7–365 gün
    assert!(!o.sezgi(&["defter", "--saklama", "3"], None).status.success());
    assert!(o.ok(&["defter", "--saklama", "90"]).contains("90"));
    let d = o.ok(&["defter"]);
    assert!(d.contains("Kod türevi") && d.contains("İşletim kaydı") && d.contains("Müşteri verisi") && d.contains("90 gün"), "{d}");
}

#[test]
fn kart_ne_tutuldugunu_soyler() {
    let o = Ortam::yeni("kart");
    let kart = |o: &Ortam| -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(o.kok.join("app").join(".sezgi").join("benlik.json")).unwrap()).unwrap() };
    let isletim = |k: &serde_json::Value| k["data_ledger"].as_array().unwrap().iter().find(|d| d["class"] == "operational_log").unwrap().clone();
    assert_eq!(isletim(&kart(&o))["stored"], false);
    o.canli("kisi@ornek.test", &[]);
    o.ok(&["init", "."]);
    let i = isletim(&kart(&o));
    assert_eq!(i["stored"], true);
    assert_eq!(i["retention_days"], 30);
    assert!(i["erase_command"].as_str().unwrap().contains("sezgi unut"));
    let musteri = kart(&o)["data_ledger"].as_array().unwrap().iter().find(|d| d["class"] == "customer_data").unwrap().clone();
    assert_eq!(musteri["stored"], false);
}

#[test]
fn teshis_yalnizca_sayac_tutar() {
    let o = Ortam::yeni("teshis");
    o.ok(&["teshis", "ac"]);
    o.canli("kisi@ornek.test", &[yaz_cagrisi(), serde_json::json!({ "arac": "ara", "arg": { "sorgu": "Gizli Arama Metni" } })]);
    let g = o.ok(&["teshis"]);
    assert!(g.contains("cagir") && g.contains("ara"), "{g}");
    let klasor = o.ev.join("ayar").join("sezgi").join("teshis");
    let disk = hepsini_oku(&klasor);
    assert!(!disk.is_empty());
    assert!(!disk.contains("Gizli Arama Metni") && !disk.contains("Gizli Başlık") && !disk.contains(GIZLI) && !disk.contains("kisi@ornek"), "{disk}");
    assert!(!disk.contains("kitaplar"), "işlem adresi bile yazılmaz: {disk}");
    o.ok(&["teshis", "kapat"]);
    let once = hepsini_oku(&klasor);
    o.canli("kisi@ornek.test", &[serde_json::json!({ "arac": "kimsin", "arg": {} })]);
    assert_eq!(hepsini_oku(&klasor), once, "kapalıyken yazılmaz");
}

/// Şifreleme öncesi yazılmış düz satırlar okunmaya devam eder; yeni satırlar şifreli, zincir ikisini birlikte doğrular
#[test]
fn eski_duz_kayit_sifreli_devam_eder() {
    let o = Ortam::yeni("gecis");
    let duz_yaz = |args: &[&str]| {
        let c = o.komut(args).env("SEZGI_KAYIT_SIFRELEME", "kapali").output().unwrap();
        assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    };
    duz_yaz(&["mod", "okuma"]);
    duz_yaz(&["mod", "gozlem"]);
    duz_yaz(&["mod", "okuma"]);
    let kayit = o.uygulama_klasoru().join("kayit.jsonl");
    let ham = std::fs::read_to_string(&kayit).unwrap();
    assert!(ham.contains("\"durum\":\"mod\""), "düz satırlar: {ham}");
    o.ok(&["mod", "okuma"]);
    let ham = std::fs::read_to_string(&kayit).unwrap();
    assert!(ham.lines().last().unwrap().contains("\"s\":\"c20p1\""), "yeni satır şifreli: {ham}");
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam: 4 satır"));
    assert_eq!(o.ok(&["kayit", "--json"]).lines().filter(|l| l.contains("\"durum\":\"mod\"")).count(), 4);
}

#[test]
fn anahtarsiz_yeniden_zincirlenen_kayit_bozuk_sayilir() {
    let o = Ortam::yeni("muhur");
    o.canli("kisi@ornek.test", &[yaz_cagrisi()]);
    let kayit = o.uygulama_klasoru().join("kayit.jsonl");
    assert!(o.ok(&["kayit", "--dogrula"]).contains("sağlam"));
    // Kayıt anahtarını bilmeyen bir süreç satırı değiştirip zincir özetlerini yeniden hesaplayabilir (özet anahtarsızdır),
    // ama mührü yazamaz. (Satırları şifrelemek burada yalnızca kolaylık: düz metin kayıtta da aynısı geçerli.)
    yeniden_zincirle(&kayit, false, |i, v| {
        if i == 0 {
            v["durum"] = serde_json::json!("degistirildi");
        }
    });
    let c = o.sezgi(&["kayit", "--dogrula"], None);
    assert!(!c.status.success(), "{}", String::from_utf8_lossy(&c.stdout));
    let cikti = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(cikti.contains("1. satırda bozuk"), "{cikti}");
}

#[test]
fn kaydin_sonundan_silinen_satir_gorunur() {
    let o = Ortam::yeni("son");
    o.canli("kisi@ornek.test", &[yaz_cagrisi()]);
    let kayit = o.uygulama_klasoru().join("kayit.jsonl");
    let satirlar: Vec<String> = std::fs::read_to_string(&kayit).unwrap().lines().map(String::from).collect();
    assert!(satirlar.len() >= 2);
    // Son satırı silmek zinciri kırmaz (kalanlar kendi içinde tutarlı); mühürlü son satır bunu yakalar
    std::fs::write(&kayit, satirlar[..satirlar.len() - 1].join("\n") + "\n").unwrap();
    let c = o.sezgi(&["kayit", "--dogrula"], None);
    assert!(!c.status.success(), "sondan silme görünmeli: {}", String::from_utf8_lossy(&c.stdout));
}

#[test]
fn kaydin_basindan_silinip_capasi_elle_yazilan_kayit_bozuk_sayilir() {
    let o = Ortam::yeni("capa");
    o.canli("kisi@ornek.test", &[yaz_cagrisi()]);
    let kayit = o.uygulama_klasoru().join("kayit.jsonl");
    let metin = std::fs::read_to_string(&kayit).unwrap();
    let ilk = coz(&kayit, metin.lines().next().unwrap());
    // İlk satırı sil ve çapayı (zincirin başlangıcı) mühürsüz olarak elle yaz: budama gibi görünmeye çalışır
    std::fs::write(&kayit, metin.lines().skip(1).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    let ayar_yolu = o.uygulama_klasoru().join("ayar.json");
    let mut ayar: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&ayar_yolu).unwrap()).unwrap();
    ayar["kayit_capa"] = serde_json::json!({ "onceki": ilk["ozet"], "sira": ilk["sira"].as_u64().unwrap() + 1 });
    std::fs::write(&ayar_yolu, ayar.to_string()).unwrap();
    let c = o.sezgi(&["kayit", "--dogrula"], None);
    assert!(!c.status.success(), "mühürsüz çapa kabul edilmemeli: {}", String::from_utf8_lossy(&c.stdout));
}
