//! W2 kaynaksız bağlama: `sezgi kesfet` çıktısı kanıt deposu (`kanit.json`), graf (`graf.json`), boş işlem haritası ve
//! Benlik Kartı olarak yazılır; MCP `kanit`/`bilinmeyenler`/`etki` kaynaksız uygulamada çalışır, işlem araçları işlem
//! olmadığını söyler; kart v0.1.1 ve v0.2.0 doğrulayıcılarında geçerlidir ve hiçbir şeyi yazılabilir saymaz; APK
//! (Coffee) triyaja bağlıdır; doktor kaynaksız klasörü ve REA köprüsünün durumunu söyler; santral üyesi olabilir.
//! PATH'te çağrılınca iz bırakan sahte `node`, `java`, `rea` vardır: hiçbiri çağrılmaz.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[allow(dead_code, clippy::all)]
#[path = "ornekler/w2kb_dogrulayici/v011/src/lib.rs"]
mod benlik_v011;
#[allow(dead_code, clippy::all)]
#[path = "ornekler/w2kb_dogrulayici/v020/src/lib.rs"]
mod benlik_v020;

fn ornekler() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler")
}

struct Ortam {
    kok: PathBuf,
    bin: PathBuf,
    iz: PathBuf,
    ayar: PathBuf,
}

impl Ortam {
    fn yeni(ad: &str) -> Ortam {
        let kok = std::env::temp_dir().join(format!("sezgi-w2kb-{ad}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        let bin = kok.join("tuzak-bin");
        let ayar = kok.join("ayar");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&ayar).unwrap();
        let iz = kok.join("cagrildi.txt");
        for a in ["node", "java", "rea"] {
            let p = bin.join(a);
            std::fs::write(&p, format!("#!/bin/sh\necho {a} >> '{}'\nexit 0\n", iz.display())).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        Ortam { kok, bin, iz, ayar }
    }

    fn komut(&self, args: &[&str], ortam: &[(&str, &str)]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        c.args(args)
            .env("PATH", &self.bin)
            .env("APPDATA", &self.ayar)
            .env("XDG_CONFIG_HOME", &self.ayar)
            .env("HOME", &self.ayar)
            .env("SEZGI_ANAHTARLIK", "kapali")
            .env_remove("SEZGI_REA")
            .env_remove("SEZGI_NODE");
        for (a, d) in ortam {
            c.env(a, d);
        }
        c
    }

    /// (çıkış kodu, stdout + stderr)
    fn calis(&self, args: &[&str], ortam: &[(&str, &str)]) -> (Option<i32>, String) {
        let o = self.komut(args, ortam).output().unwrap();
        (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    }

    fn ok(&self, args: &[&str]) -> String {
        let (k, m) = self.calis(args, &[]);
        assert_eq!(k, Some(0), "{args:?}: {m}");
        m
    }

    fn cagrilmadi(&self) {
        assert!(!self.iz.exists(), "node/java/rea çağrılmamalı: {:?}", std::fs::read_to_string(&self.iz));
    }

    /// MCP araç çağrıları: (metin, hata mı)
    fn mcp(&self, args: &[&str], araclar: &[(&str, Value)]) -> Vec<(String, bool)> {
        let mut p = self.komut(args, &[]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let mut stdin = p.stdin.take().unwrap();
        let mut satirlar = BufReader::new(p.stdout.take().unwrap()).lines();
        let mut out = vec![];
        for (i, (ad, a)) in araclar.iter().enumerate() {
            let istek = json!({ "jsonrpc": "2.0", "id": i + 1, "method": "tools/call", "params": { "name": ad, "arguments": a } });
            writeln!(stdin, "{istek}").unwrap();
            stdin.flush().unwrap();
            let v: Value = serde_json::from_str(&satirlar.next().unwrap().unwrap()).unwrap();
            out.push((v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string(), v["result"]["isError"] == true));
        }
        drop(stdin);
        let _ = p.wait();
        out
    }
}

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        if g.file_type().unwrap().is_file() {
            std::fs::copy(g.path(), hedef.join(g.file_name())).unwrap();
        }
    }
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn sha256(p: &Path) -> String {
    // Kart ve kanıt dayanağının özeti, testte bağımsız hesap (doğrulayıcının SHA-256'sı)
    benlik_v020::sha256(&std::fs::read(p).unwrap()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Kenar: (kaynak, ilişki, hedef) → güven
fn kenar<'a>(graf: &'a Value, kaynak: &str, iliski: &str, hedef: &str) -> Option<&'a Value> {
    graf["kenarlar"].as_array().unwrap().iter().find(|k| k["kaynak"] == kaynak && k["iliski"] == iliski && k["hedef"] == hedef).map(|k| &k["guven"])
}

/// Kartı iki doğrulayıcıyla denetler: geçerli, hiçbir işlem yazılabilir değil
fn dogrulayicilar_yazilabilir_bos(kart: &Value, paket: Option<&Value>, anahtarlar: Option<&Value>) {
    let v1 = benlik_v011::writable_operations(kart);
    assert!(v1.is_empty(), "v0.1.1 doğrulayıcısı yazılabilir buldu: {v1:?}");
    assert!(benlik_v011::has_required_members(kart), "v0.1.1: zorunlu üyeler");
    assert!(benlik_v011::spec_errors(kart).is_empty(), "v0.1.1: {:?}", benlik_v011::spec_errors(kart));
    let r = benlik_v020::validate(kart, &benlik_v020::Options { evidence_bundle: paket, public_keys: anahtarlar, trusted_keys: anahtarlar, ..Default::default() });
    assert!(r.valid, "v0.2.0 kartı geçersiz saydı: {:?}", r.errors);
    assert!(r.writable.is_empty(), "v0.2.0 doğrulayıcısı yazılabilir buldu: {:?}", r.writable);
    assert!(r.not_presented.is_empty() || paket.is_none(), "kanıt paketinde eksik kayıt: {:?}", r.not_presented);
    if anahtarlar.is_some() {
        assert_eq!(r.signature, benlik_v020::SignatureStatus::Verified, "{:?}", r.signature_detail);
    }
}

#[test]
fn w2kb_otelrez_kanit_graf_kart_ve_mcp() {
    let o = Ortam::yeni("otel");
    let y = o.kok.join("yayin");
    kopyala(&ornekler().join("w2k_otelrez/yayin"), &y);
    let m = o.ok(&["kesfet", y.to_str().unwrap()]);
    assert!(m.contains("Kanıt deposu ve graf") && m.contains("benlik.json"), "{m}");
    let c = y.join(".sezgi");
    // Kanıt deposu: S2 biçimi, yapıtın özetine kilitli, hiçbir önerme certain değil
    let depo = json_oku(&c.join("kanit.json"));
    assert_eq!(depo["bicim"], 1);
    assert_eq!(depo["kok"], "..", "dayanak yolları yapıt köküne göreli");
    for k in depo["kanitlar"].as_array().unwrap() {
        let d = &k["dayanak"][0];
        assert_eq!(d["sha256"].as_str().unwrap(), sha256(&y.join(d["yol"].as_str().unwrap())), "dayanak yapıtın özetine kilitli: {k}");
        assert!(k["sinirlamalar"][0].as_str().unwrap().starts_with("kaynak kodu yok"), "{k}");
    }
    for on in depo["onermeler"].as_array().unwrap() {
        assert_ne!(on["hesap"]["guven"], "certain", "kaynaksız önerme certain olamaz: {on}");
        assert_ne!(on["hesap"]["kapsam"], "kapali", "kaynaksız önerme kapalı dünya olamaz: {on}");
    }
    // Graf: uygulama, ikili, içe aktarılan kütüphane, tablo, sütun; FK, tetikleyici
    let graf = json_oku(&c.join("graf.json"));
    let dugumler: Vec<&str> = graf["dugumler"].as_array().unwrap().iter().map(|d| d["id"].as_str().unwrap()).collect();
    for d in [
        "app:yayin",
        "mod:OtelRez.dll",
        "mod:OtelRez.exe",
        "mod:otel.db",
        "mod:dll/kernel32.dll",
        "db:rezervasyon",
        "db:iptal_kaydi",
        "field:rezervasyon.durum",
        "db:aktif_rezervasyon",
    ] {
        assert!(dugumler.contains(&d), "{d} düğümü yok: {dugumler:?}");
    }
    assert_eq!(kenar(&graf, "app:yayin", "DEPENDS_ON", "mod:OtelRez.dll"), Some(&json!("observed")));
    assert_eq!(kenar(&graf, "mod:OtelRez.exe", "DEPENDS_ON", "mod:dll/kernel32.dll"), Some(&json!("observed")));
    assert_eq!(kenar(&graf, "db:rezervasyon", "REFERENCES", "db:oda"), Some(&json!("observed")), "şemada tanımlı FK: dosyadan okunan şemanın tavanı (certain değil)");
    assert_eq!(kenar(&graf, "db:rezervasyon", "MAY_AFFECT", "db:iptal_kaydi"), Some(&json!("inferred")), "tetikleyici: olası etki");
    assert_eq!(kenar(&graf, "db:aktif_rezervasyon", "REFERENCES", "db:rezervasyon"), Some(&json!("inferred")), "görünüm");
    // Boş işlem haritası
    let harita = json_oku(&c.join("kapilar.json"));
    assert_eq!(harita["endpointler"], json!([]));
    assert_eq!(harita["ozet"]["kaynaksiz"], true);
    assert_eq!(harita["ozet"]["uygulama"], "OtelRez Rezervasyon");
    // MCP: kanıt araçları çalışır; işlem araçları işlem olmadığını söyler
    let r = o.mcp(
        &["mcp", "--harita", c.to_str().unwrap()],
        &[
            ("kimsin", json!({})),
            ("ara", json!({ "sorgu": "rezervasyon" })),
            ("onizle", json!({ "kapi": "POST /iptal" })),
            ("kanit", json!({ "varlik": "db:rezervasyon" })),
            ("bilinmeyenler", json!({ "adet": 10 })),
            ("etki", json!({ "varlik": "db:rezervasyon" })),
            ("kanit", json!({ "varlik": "app:yayin" })),
        ],
    );
    assert!(r[0].0.contains("Kaynaksız uygulama") && !r[0].0.contains("genel önek") && r[0].0.contains("app:yayin"), "{}", r[0].0);
    assert!(r[1].1 && r[1].0.contains("kapı/işlem yok"), "ara dürüstçe işlem olmadığını söyler: {:?}", r[1]);
    assert!(r[2].1 && r[2].0.contains("hiçbir zaman GEÇER demez"), "önizleme: {:?}", r[2]);
    assert!(!r[3].1 && r[3].0.contains("DATABASE_SCHEMA") && r[3].0.contains("[Gözlem]") && r[3].0.contains("REFERENCES db:oda = ") && r[3].0.contains("[Gözlem]"), "{}", r[3].0);
    assert!(r[4].0.contains("ONIZLEME") && r[4].0.contains("iş kuralları") && r[4].0.contains("REA istenmedi"), "{}", r[4].0);
    assert!(r[5].0.contains("iptal_kaydi") && r[5].0.contains("[Yorum]"), "etki tetikleyiciyi izler: {}", r[5].0);
    assert!(r[6].0.contains("KIMLIK") && r[6].0.contains("BINARY_METADATA"), "{}", r[6].0);
    // Kart: işlem yok, yetenek ve ekranlar onaylı, önizleme açık; iki doğrulayıcıda yazılabilir yok
    let kart = json_oku(&c.join("benlik.json"));
    let paket = json_oku(&c.join("benlik-evidence.json"));
    assert_eq!(kart["operations"], json!([]));
    assert_eq!(kart["x-sourceless"], true);
    assert_eq!(kart["x-preview"]["passes"], false);
    assert_eq!(kart["app"]["name"], "OtelRez Rezervasyon");
    assert_eq!(kart["app"]["version"], "2.3.1");
    assert_eq!(kart["x-basis"]["OtelRez.dll"].as_str().unwrap(), sha256(&y.join("OtelRez.dll")));
    for e in kart["x-evidence"].as_array().unwrap() {
        assert_ne!(e["confidence"], "certain", "{e}");
    }
    dogrulayicilar_yazilabilir_bos(&kart, Some(&paket), None);
    // Doğrulayıcı gerçekten karar veriyor (test boş geçmiyor): kesin damgalı bir yazma işlemi eklenirse yazılabilir olur
    let mut sahte = kart.clone();
    sahte["signature"] = json!({ "alg": "ML-DSA-65", "key_id": "k", "public_key_url": "/k", "value": "AAAA" });
    sahte["operations"] = json!([{ "id": "x.yaz", "effect": "write", "entry": { "protocol": "http", "method": "POST", "path": "/x" }, "inputs": {}, "stamp": "certain" }]);
    assert_eq!(benlik_v020::writable_operations(&sahte), ["x.yaz"], "denetim çalışmıyor");
    assert_eq!(benlik_v011::writable_operations(&sahte), ["x.yaz"], "denetim çalışmıyor");
    // Yapıt değişince kanıt eski olur (derleme özetine kilitli): doktor söyler, bilinmeyenlerde eski görünür
    let (kod, d) = o.calis(&["doktor", y.to_str().unwrap()], &[]);
    assert!(kod == Some(0) && d.contains("kaynaksız") && d.contains("yapıtlar keşiften bu yana değişmedi") && d.contains("sezgi kesfet"), "{d}");
    assert!(!d.contains("package.json") && !d.contains("Çatı"), "kaynaksız klasörde çatı aranmaz: {d}");
    let mut db = std::fs::OpenOptions::new().append(true).open(y.join("otel.db")).unwrap();
    db.write_all(&[0u8; 16]).unwrap();
    drop(db);
    let (_, d) = o.calis(&["doktor", y.to_str().unwrap()], &[]);
    assert!(d.contains("yapıt değişti") && d.contains("otel.db"), "{d}");
    let r = o.mcp(&["mcp", c.to_str().unwrap()], &[("bilinmeyenler", json!({ "adet": 5 })), ("kanit", json!({ "varlik": "db:oda" }))]);
    assert!(r[0].0.contains("eski") && !r[0].0.contains("eski 0"), "{}", r[0].0);
    assert!(r[1].0.contains("eski") || r[1].0.contains("dayanak değişti"), "{}", r[1].0);
    o.cagrilmadi();
}

#[test]
fn w2kb_imzali_kart_dogrulanir_ve_yazilabilir_yok() {
    let o = Ortam::yeni("imza");
    let y = o.kok.join("yayin");
    kopyala(&ornekler().join("w2k_otelrez/yayin"), &y);
    o.ok(&["anahtar", "olustur", "--ad", "w2kb"]);
    let m = o.ok(&["kesfet", y.to_str().unwrap(), "--imzala", "w2kb"]);
    assert!(m.contains("ML-DSA-65 ile imzalandı"), "{m}");
    let c = y.join(".sezgi");
    let (kart, paket, anahtarlar) = (json_oku(&c.join("benlik.json")), json_oku(&c.join("benlik-evidence.json")), json_oku(&c.join("benlik-keys.json")));
    assert_eq!(kart["signature"]["alg"], "ML-DSA-65");
    dogrulayicilar_yazilabilir_bos(&kart, Some(&paket), Some(&anahtarlar));
    let d = o.ok(&["dogrula", c.join("benlik.json").to_str().unwrap(), c.join("benlik-keys.json").to_str().unwrap()]);
    assert!(d.contains("İmza geçerli"), "{d}");
    o.cagrilmadi();
}

#[test]
fn w2kb_coffee_apk_triyaj_graf_ve_kart() {
    let o = Ortam::yeni("apk");
    let a = o.kok.join("apk");
    std::fs::create_dir_all(&a).unwrap();
    let apk = "com.github.muellerma.coffee_47.apk";
    std::fs::copy(ornekler().join("w2a_coffee").join(apk), a.join(apk)).unwrap();
    std::fs::write(a.join("sahte.apk"), b"PK degil").unwrap();
    let m = o.ok(&["kesfet", a.to_str().unwrap(), "--rea"]);
    assert!(m.contains("APK") && m.contains("com.github.muellerma.coffee 2.27") && m.contains("dışa açık"), "{m}");
    let c = a.join(".sezgi");
    let b = json_oku(&c.join("kaynaksiz.json"));
    let plan = b["plan"].as_array().unwrap();
    assert_eq!(plan.len(), 1, "imzası tutmayan .apk okunmaz: {plan:?}");
    assert_eq!(plan[0]["tur"], "apk");
    // REA'nın APK araçları süreç başlatır ve diske yazar: izin listesinde değil, --rea ile de çalışmaz
    for r in plan[0]["rea"].as_array().unwrap() {
        assert_eq!(r["durum"], "calistirilmadi", "{r}");
        assert!(r["neden"].as_str().unwrap().contains("izin listesinde değil"), "{r}");
    }
    let kn = &b["kanitlar"][0];
    assert_eq!((kn["kaynak_turu"].as_str(), kn["beyan"].as_str(), kn["uretici"].as_str()), (Some("BINARY_METADATA"), Some("observed"), Some("sezgi/android@1")));
    assert!(kn["sinirlamalar"].to_string().contains("APK imzası doğrulanmadı"), "{kn}");
    let graf = json_oku(&c.join("graf.json"));
    let app = "app:android/com.github.muellerma.coffee";
    assert_eq!(kenar(&graf, app, "REQUIRES_PERMISSION", "perm:android.permission.WAKE_LOCK"), Some(&json!("observed")));
    let dugumler: Vec<&str> = graf["dugumler"].as_array().unwrap().iter().map(|d| d["id"].as_str().unwrap()).collect();
    assert!(dugumler.contains(&"screen:android/com.github.muellerma.coffee.activities.MainActivity"), "{dugumler:?}");
    assert!(dugumler.iter().any(|d| d.starts_with("mod:android/")), "servis ve alıcılar: {dugumler:?}");
    let kart = json_oku(&c.join("benlik.json"));
    assert_eq!(kart["app"]["name"], "com.github.muellerma.coffee");
    assert_eq!(kart["app"]["version"], "2.27");
    let ekran = kart["x-screens"].as_array().unwrap().iter().find(|e| e["id"] == "screen:android/com.github.muellerma.coffee.activities.MainActivity").unwrap();
    assert_eq!((ekran["kind"].as_str(), ekran["stamp"].as_str(), ekran["requires_approval"].as_bool()), (Some("activity"), Some("observed"), Some(true)));
    let yetenekler = kart["x-capabilities"].as_array().unwrap();
    assert!(!yetenekler.is_empty(), "dışa açık bileşenler yetenek olarak yazılır");
    for y in yetenekler {
        assert_eq!(y["requires_approval"], true, "{y}");
        assert!(matches!(y["stamp"].as_str(), Some("observed" | "inferred")), "{y}");
    }
    dogrulayicilar_yazilabilir_bos(&kart, Some(&json_oku(&c.join("benlik-evidence.json"))), None);
    let r = o.mcp(&["mcp", c.to_str().unwrap()], &[("kanit", json!({ "varlik": app })), ("etki", json!({ "varlik": "perm:android.permission.WAKE_LOCK" }))]);
    assert!(r[0].0.contains("REQUIRES_PERMISSION perm:android.permission.WAKE_LOCK [Gözlem]"), "{}", r[0].0);
    assert!(!r[1].1, "{}", r[1].0);
    o.cagrilmadi();
}

/// `sezgi doktor`: REA durumu yalnızca --rea ile ya da kaynaksız klasörde; yok / sürüm uyuşmuyor / sabitleme hatası
#[test]
fn w2kb_doktor_rea_durumu() {
    let o = Ortam::yeni("doktor");
    let y = o.kok.join("yayin");
    kopyala(&ornekler().join("w2k_otelrez/yayin"), &y);
    o.ok(&["kesfet", y.to_str().unwrap()]);
    let (kod, d) = o.calis(&["doktor", y.to_str().unwrap()], &[]);
    assert!(kod == Some(0) && d.contains("REA") && d.contains("kurulu değil (isteğe bağlı"), "{d}");
    let (kod, d) = o.calis(&["doktor", y.to_str().unwrap(), "--rea", "--json"], &[]);
    let v: Value = serde_json::from_str(d.trim()).unwrap_or_else(|_| panic!("{d}"));
    let rea = v["kontroller"].as_array().unwrap().iter().find(|k| k["ad"] == "rea").unwrap().clone();
    assert!(kod == Some(0) && rea["durum"] == "uyari" && rea["mesaj"].as_str().unwrap().contains("kurulu değil"), "{rea}");
    // Sahte kurulumlar: sürüm uyuşmuyor ve sabitleme (bütünlük) hatası; --rea ile hata (çıkış 1), hiçbir şey çalışmaz
    let dogru = "sha512-yhIx2ioJYYT1C/tONfYeitC6LhnKmCh+ZwRUOgYuhSc733lGhkkt73zhug9WmLgxoMdKuqIYGCoBfD4fXkOhFw==";
    for (ad, surum, butunluk, beklenen) in [("surum", "6.8.0", dogru, "sürüm uyuşmuyor"), ("butunluk", "6.7.0", "sha512-AAAA", "sabitleme hatası")] {
        let kurulum = o.kok.join(format!("kurulum-{ad}"));
        let paket = kurulum.join("node_modules/rea-agents");
        std::fs::create_dir_all(paket.join("dist/contracts")).unwrap();
        std::fs::create_dir_all(kurulum.join("node_modules/.bin")).unwrap();
        std::fs::write(paket.join("package.json"), format!(r#"{{"name":"rea-agents","version":"{surum}","bin":{{"rea":"scripts/rea.mjs"}}}}"#)).unwrap();
        std::fs::write(
            kurulum.join("node_modules/.package-lock.json"),
            format!(r#"{{"packages":{{"node_modules/rea-agents":{{"version":"{surum}","integrity":"{butunluk}"}}}}}}"#),
        )
        .unwrap();
        std::fs::copy(o.bin.join("rea"), kurulum.join("node_modules/.bin/rea")).unwrap();
        let rea = kurulum.join("node_modules/.bin/rea");
        let node = o.bin.join("node");
        let ortam = [("SEZGI_REA", rea.to_str().unwrap()), ("SEZGI_NODE", node.to_str().unwrap())];
        let (kod, d) = o.calis(&["doktor", y.to_str().unwrap(), "--rea"], &ortam);
        assert!(kod == Some(1) && d.contains(beklenen) && d.contains("köprü kapalı kalır"), "{ad}: {d}");
        let (kod, d) = o.calis(&["doktor", y.to_str().unwrap()], &ortam);
        assert!(kod == Some(0) && d.contains(beklenen), "kaynaksız klasörde --rea olmadan uyarı: {ad}: {d}");
    }
    o.cagrilmadi();
}

/// Santral üyesi: kaynaksız uygulama santrale eklenir, kanıt araçları `uygulama` argümanıyla ona gider
#[test]
fn w2kb_santral_uyesi() {
    let o = Ortam::yeni("santral");
    let y = o.kok.join("yayin");
    kopyala(&ornekler().join("w2k_otelrez/yayin"), &y);
    o.ok(&["kesfet", y.to_str().unwrap()]);
    o.ok(&["santral", "ekle", y.join(".sezgi").to_str().unwrap(), "--ad", "otel"]);
    let r = o.mcp(&["mcp", "--santral"], &[("kimsin", json!({})), ("kanit", json!({ "uygulama": "otel", "varlik": "db:oda" }))]);
    assert!(r[0].0.contains("## otel") && r[0].0.contains("Kaynaksız uygulama"), "{}", r[0].0);
    assert!(r[1].0.contains("DATABASE_SCHEMA") && r[1].0.contains("KISITLAR"), "{}", r[1].0);
    o.cagrilmadi();
}

/// Kaynak kodundan üretilmiş haritanın klasörüne kesfet bağlama yazmaz (harita ezilmez)
#[test]
fn w2kb_kaynakli_haritaya_dokunmaz() {
    let o = Ortam::yeni("kaynakli");
    let u = o.kok.join("uygulama");
    std::fs::create_dir_all(&u).unwrap();
    for g in walk(&ornekler().join("nestjs-temel")) {
        let hedef = u.join(g.strip_prefix(ornekler().join("nestjs-temel")).unwrap());
        std::fs::create_dir_all(hedef.parent().unwrap()).unwrap();
        std::fs::copy(&g, hedef).unwrap();
    }
    o.ok(&["init", u.to_str().unwrap()]);
    let once = std::fs::read(u.join(".sezgi/kapilar.json")).unwrap();
    let kart_once = std::fs::read(u.join(".sezgi/benlik.json")).unwrap();
    std::fs::copy(ornekler().join("w2k_otelrez/yayin/OtelRez.dll"), u.join("OtelRez.dll")).unwrap();
    let (kod, m) = o.calis(&["kesfet", u.to_str().unwrap()], &[]);
    assert!(kod == Some(0) && m.contains("kaynak kodundan üretilmiş bir harita"), "{m}");
    assert_eq!(std::fs::read(u.join(".sezgi/kapilar.json")).unwrap(), once, "kaynak kodlu harita ezilmemeli");
    assert_eq!(std::fs::read(u.join(".sezgi/benlik.json")).unwrap(), kart_once, "kart ezilmemeli");
    assert!(u.join(".sezgi/kaynaksiz.json").exists());
}

fn walk(d: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    for g in std::fs::read_dir(d).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            v.extend(walk(&p));
        } else {
            v.push(p);
        }
    }
    v
}
