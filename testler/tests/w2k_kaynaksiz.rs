//! W2 kaynaksız keşif (`sezgi kesfet`): PE kimliği ve SQLite şeması Node'suz, REA köprüsü fail-closed, triyaj planı,
//! bozuk girdilerde panik yok. Test görüntüsünde node, java ve rea yoktur; ayrıca PATH'e çağrılınca iz bırakan sahte
//! `node`, `java`, `rea` konur: hiçbirinin çağrılmadığı denetlenir.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ornek() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ornekler/w2k_otelrez")
}

fn gecici(ad: &str) -> PathBuf {
    let k = std::env::temp_dir().join(format!("sezgi-w2k-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    std::fs::create_dir_all(&k).unwrap();
    k
}

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        if g.file_type().unwrap().is_file() {
            std::fs::copy(g.path(), hedef.join(g.file_name())).unwrap();
        }
    }
}

/// Çağrılınca iz dosyası bırakan sahte araçlar (node, java, rea) ve yalnızca onları içeren PATH
fn tuzak(k: &Path) -> (PathBuf, PathBuf) {
    let bin = k.join("tuzak-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let iz = k.join("cagrildi.txt");
    for a in ["node", "java", "rea"] {
        let p = bin.join(a);
        std::fs::write(&p, format!("#!/bin/sh\necho {a} >> '{}'\nexit 0\n", iz.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    (bin, iz)
}

/// (başarılı mı, çıkış kodu, stdout, stderr)
fn calis(args: &[&str], path: &Path, ortam: &[(&str, &str)]) -> (Option<i32>, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
    c.args(args).env("PATH", path).env_remove("SEZGI_REA").env_remove("SEZGI_NODE");
    for (a, d) in ortam {
        c.env(a, d);
    }
    let o = c.output().unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn belge(k: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(k.join(".sezgi/kaynaksiz.json")).unwrap()).unwrap()
}

fn kanit<'a>(b: &'a Value, konum: &str) -> &'a Value {
    b["kanitlar"].as_array().unwrap().iter().find(|k| k["konum"][0] == konum).unwrap_or_else(|| panic!("{konum} kanıtı yok"))
}

#[test]
fn w2k_nodesuz_pe_kimligi_ve_sqlite_semasi() {
    let k = gecici("nodesuz");
    let y = k.join("yayin");
    kopyala(&ornek().join("yayin"), &y);
    let db_once = std::fs::read(y.join("otel.db")).unwrap();
    let (bin, iz) = tuzak(&k);
    let (kod, out, err) = calis(&["kesfet", y.to_str().unwrap()], &bin, &[]);
    assert_eq!(kod, Some(0), "{out}{err}");
    assert!(out.contains("hiçbir zaman GEÇER değil"), "{out}");
    assert!(!iz.exists(), "node/java/rea çağrılmamalı: {:?}", std::fs::read_to_string(&iz));
    assert_eq!(std::fs::read(y.join("otel.db")).unwrap(), db_once, "veritabanı dosyası değişmemeli");
    let b = belge(&y);
    assert_eq!(b["onizleme"]["gecer"], false);
    assert_eq!(b["rea"]["durum"], "istenmedi");
    // PE kimliği (BINARY_METADATA, en fazla observed)
    let dll = kanit(&b, "OtelRez.dll");
    assert_eq!(dll["kaynak_turu"], "BINARY_METADATA");
    assert_eq!(dll["beyan"], "observed");
    let o = &dll["ozet"];
    assert_eq!(o["dotnet"], true);
    assert_eq!(o["clr_surumu"], "v4.0.30319");
    assert_eq!(o["makine"], "x64");
    assert_eq!(o["surum_kaynagi"]["ProductName"], "OtelRez Rezervasyon");
    assert_eq!(o["surum_kaynagi"]["CompanyName"], "Ornek Otelcilik");
    assert_eq!(o["surum_kaynagi"]["FileVersion"], "2.3.1.0");
    assert_eq!(o["dosya_surumu"], "2.3.1.0");
    let cati = kanit(&b, "OtelRez.dll#hedef_cati");
    assert_eq!(cati["kaynak_turu"], "BINARY_HEURISTIC");
    assert_eq!(cati["beyan"], "inferred");
    assert_eq!(cati["ozet"]["hedef_cati"], ".NETCoreApp,Version=v8.0");
    let exe = &kanit(&b, "OtelRez.exe")["ozet"];
    assert_eq!(exe["dotnet"], false);
    assert!(exe["importlar"].as_array().unwrap().iter().any(|i| i == "KERNEL32.dll"), "{exe}");
    // SQLite şeması: CHECK, FK, indeks, tetikleyici, görünüm
    let rez = kanit(&b, "otel.db#tablo:rezervasyon");
    assert_eq!(rez["kaynak_turu"], "DATABASE_SCHEMA");
    let r = &rez["ozet"];
    let kontroller: Vec<&str> = r["kontroller"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert!(kontroller.contains(&"tutar >= 0") && kontroller.contains(&"durum IN ('aktif', 'iptal')"), "{kontroller:?}");
    let fk: Vec<(&str, &str)> = r["yabanci_anahtarlar"].as_array().unwrap().iter().map(|f| (f["tablo"].as_str().unwrap(), f["kaynak_sutunlar"][0].as_str().unwrap())).collect();
    assert_eq!(fk, [("oda", "oda_no"), ("misafir", "misafir_id")]);
    assert_eq!(r["yabanci_anahtarlar"][0]["silinince"], "RESTRICT");
    assert_eq!(r["indeksler"][0]["ad"], "ix_rez_oda");
    assert_eq!(r["indeksler"][0]["sutunlar"], serde_json::json!(["oda_no", "giris"]));
    let t = &r["tetikleyiciler"][0];
    assert_eq!((t["ad"].as_str(), t["zaman"].as_str(), t["olay"].as_str()), (Some("trg_iptal"), Some("AFTER"), Some("UPDATE")));
    assert_eq!(t["calistirilmadi"], true);
    assert!(t["metin"].as_str().unwrap().contains("INSERT INTO iptal_kaydi"));
    let durum = r["sutunlar"].as_array().unwrap().iter().find(|s| s["ad"] == "durum").unwrap();
    assert_eq!((durum["bos_olamaz"].as_bool(), durum["varsayilan"].as_bool()), (Some(true), Some(true)));
    let oda = &kanit(&b, "otel.db#tablo:oda")["ozet"];
    assert_eq!(oda["kontroller"][0], "tip IN ('tek', 'cift', 'suit')");
    assert_eq!(oda["birincil_anahtar"], serde_json::json!(["no"]));
    let misafir = &kanit(&b, "otel.db#tablo:misafir")["ozet"];
    assert!(misafir["indeksler"].as_array().unwrap().iter().any(|i| i["ad"] == "ux_misafir_eposta" && i["benzersiz"] == true), "{misafir}");
    assert!(misafir["indeksler"].as_array().unwrap().iter().any(|i| i["otomatik"] == true), "UNIQUE sütunun otomatik indeksi: {misafir}");
    assert_eq!(kanit(&b, "otel.db#gorunum:aktif_rezervasyon")["ozet"]["calistirilmadi"], true);
    // Taşma sayfalarına yayılan uzun şema satırları da okunur (40 ek tablo)
    assert_eq!(b["kanitlar"].as_array().unwrap().iter().filter(|k| k["konum"][0].as_str().unwrap().starts_with("otel.db#tablo:ek_")).count(), 40);
    // Politika: hepsi açık kapsamlı, hiçbiri certain değil; yapılandırma değerleri okunmaz
    for kn in b["kanitlar"].as_array().unwrap() {
        assert_eq!(kn["kapsam"], "acik", "{kn}");
        assert_ne!(kn["beyan"], "certain", "{kn}");
        assert!(kn["id"].as_str().unwrap().starts_with("kn_"));
    }
    let metin = std::fs::read_to_string(y.join(".sezgi/kaynaksiz.json")).unwrap();
    assert!(!metin.contains("ORNEK_GIZLI_DEGER"), "yapılandırma değeri kanıta girmemeli");
    let bilinmeyen = b["bilinmeyenler"].to_string();
    assert!(bilinmeyen.contains("REA istenmedi") && bilinmeyen.contains("iş kuralları"), "{bilinmeyen}");
    // Kanıt kimliği içerikten: ikinci çalıştırma aynı kimlikleri üretir
    let (kod, _, _) = calis(&["kesfet", y.to_str().unwrap()], &bin, &[]);
    assert_eq!(kod, Some(0));
    let kimlikler = |b: &Value| b["kanitlar"].as_array().unwrap().iter().map(|k| k["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(kimlikler(&b), kimlikler(&belge(&y)));
}

/// REA köprüsü sabitleme tutmadıkça hiçbir şey çalıştırmaz, sebebini söyler, komut başarılı biter
#[test]
fn w2k_rea_sabitleme_tutmazsa_kapali() {
    let k = gecici("kapali");
    let y = k.join("yayin");
    kopyala(&ornek().join("yayin"), &y);
    let (bin, iz) = tuzak(&k);
    let dogru = "sha512-yhIx2ioJYYT1C/tONfYeitC6LhnKmCh+ZwRUOgYuhSc733lGhkkt73zhug9WmLgxoMdKuqIYGCoBfD4fXkOhFw==";
    // (ad, paket sürümü, kilit dosyasındaki bütünlük, etki tablosu içeriği, beklenen sebep parçası)
    let durumlar: [(&str, &str, Option<&str>, &str, &str); 4] = [
        ("surum", "6.8.0", Some(dogru), "x", "beklenen 6.7.0"),
        ("kilitsiz", "6.7.0", None, "x", "integrity"),
        ("butunluk", "6.7.0", Some("sha512-AAAA"), "x", "bütünlüğü"),
        ("etki", "6.7.0", Some(dogru), "export const TOOL_EFFECTS = {};", "etki tablosu"),
    ];
    for (ad, surum, butunluk, etki, beklenen) in durumlar {
        let kurulum = k.join(format!("kurulum-{ad}"));
        let paket = kurulum.join("node_modules/rea-agents");
        std::fs::create_dir_all(paket.join("dist/contracts")).unwrap();
        std::fs::create_dir_all(kurulum.join("node_modules/.bin")).unwrap();
        std::fs::write(paket.join("package.json"), format!(r#"{{"name":"rea-agents","version":"{surum}","bin":{{"rea":"scripts/rea.mjs"}}}}"#)).unwrap();
        std::fs::write(paket.join("dist/contracts/toolEffects.js"), etki).unwrap();
        std::fs::copy(bin.join("rea"), kurulum.join("node_modules/.bin/rea")).unwrap();
        if let Some(b) = butunluk {
            let kilit = format!(r#"{{"packages":{{"node_modules/rea-agents":{{"version":"{surum}","integrity":"{b}"}}}}}}"#);
            std::fs::write(kurulum.join("node_modules/.package-lock.json"), kilit).unwrap();
        }
        let rea = kurulum.join("node_modules/.bin/rea");
        let node = bin.join("node");
        let (kod, out, err) = calis(&["kesfet", y.to_str().unwrap(), "--rea"], &bin, &[("SEZGI_REA", rea.to_str().unwrap()), ("SEZGI_NODE", node.to_str().unwrap())]);
        assert_eq!(kod, Some(0), "{ad}: {out}{err}");
        let b = belge(&y);
        assert_eq!(b["rea"]["durum"], "kapali", "{ad}: {}", b["rea"]);
        assert!(b["rea"]["neden"].as_str().unwrap().contains(beklenen), "{ad}: {}", b["rea"]);
        assert!(out.contains("REA: kapalı"), "{ad}: {out}");
        assert!(b["bilinmeyenler"].to_string().contains("REA kapalı"), "{ad}");
        assert!(b["kanitlar"].as_array().unwrap().iter().all(|k| !k["uretici"].as_str().unwrap().starts_with("rea")), "{ad}");
        // Yerel okuyucular yine çalışır
        assert_eq!(kanit(&b, "OtelRez.dll")["ozet"]["dotnet"], true);
        assert!(!iz.exists(), "{ad}: sabitleme tutmadan node/rea çalıştırılmamalı: {:?}", std::fs::read_to_string(&iz));
    }
    // rea hiç yok (PATH'te de)
    let bos = k.join("bos-bin");
    std::fs::create_dir_all(&bos).unwrap();
    let (kod, out, _) = calis(&["kesfet", y.to_str().unwrap(), "--rea"], &bos, &[]);
    assert_eq!(kod, Some(0));
    assert!(belge(&y)["rea"]["neden"].as_str().unwrap().contains("rea bulunamadı"), "{out}");
}

/// Klasör triyajı: hangi dosyayı hangi okuyucu / REA aracı okur
#[test]
fn w2k_masaustu_triyaj_plani() {
    let k = gecici("triyaj");
    let y = k.join("uygulama");
    kopyala(&ornek().join("yayin"), &y);
    std::fs::create_dir_all(y.join("resources")).unwrap();
    std::fs::write(y.join("resources/app.asar"), b"\x04\x00\x00\x00asar").unwrap();
    std::fs::write(y.join("appsettings.Development.json"), "{}").unwrap();
    std::fs::create_dir_all(y.join("runtimes/win-x64/native")).unwrap();
    std::fs::copy(y.join("OtelRez.exe"), y.join("runtimes/win-x64/native/yardimci.dll")).unwrap();
    std::fs::create_dir_all(y.join("node_modules/x")).unwrap();
    std::fs::copy(y.join("OtelRez.exe"), y.join("node_modules/x/atla.dll")).unwrap();
    std::fs::write(y.join("sahte.dll"), b"PK degil aslinda").unwrap();
    std::fs::write(y.join("metin.db"), b"bu bir veritabani degil").unwrap();
    let (bin, iz) = tuzak(&k);
    let (kod, out, err) = calis(&["discover", y.to_str().unwrap()], &bin, &[]);
    assert_eq!(kod, Some(0), "{out}{err}");
    let b = belge(&y);
    let plan: Vec<(String, String)> = b["plan"].as_array().unwrap().iter().map(|o| (o["yol"].as_str().unwrap().to_string(), o["tur"].as_str().unwrap().to_string())).collect();
    for beklenen in [
        ("OtelRez.dll", "pe"),
        ("OtelRez.exe", "pe"),
        ("otel.db", "sqlite"),
        ("appsettings.json", "yapilandirma"),
        ("appsettings.Development.json", "yapilandirma"),
        ("resources/app.asar", "electron"),
        ("runtimes/win-x64/native/yardimci.dll", "pe"),
    ] {
        assert!(plan.iter().any(|(a, t)| a == beklenen.0 && t == beklenen.1), "{beklenen:?} planda yok: {plan:?}");
    }
    assert!(!plan.iter().any(|(a, _)| a.contains("node_modules")), "bağımlılık klasörü gezilmez: {plan:?}");
    // İmzası tutmayan dosya (uzantısı .dll / .db olsa da) okunmaz
    assert!(!plan.iter().any(|(a, _)| a == "sahte.dll" || a == "metin.db"), "{plan:?}");
    let o = |yol: &str| b["plan"].as_array().unwrap().iter().find(|o| o["yol"] == yol).unwrap().clone();
    assert_eq!(o("OtelRez.dll")["rea"][0]["arac"], "inspect-managed-members");
    // Yerel ikili: .NET aracı planlanmaz; native analiz (motorla, onaylı) planlanır, --rea yoksa istenmedi (W3)
    assert_eq!(o("OtelRez.exe")["rea"], serde_json::json!([{ "arac": "inspect", "durum": "istenmedi" }]));
    // Bozuk app.asar (W3: paket okunur): ASAR değil diye bilinmeyen; REA adımı planlanmaz
    let asar = o("resources/app.asar");
    assert_eq!(asar["tur"], "electron");
    assert!(b["bilinmeyenler"].to_string().contains("ASAR başlığı kısa"), "{}", b["bilinmeyenler"]);
    assert!(!iz.exists());
    // Tek dosya ve --cikti; --json çıktısı
    let c = k.join("cikti");
    let (kod, out, _) = calis(&["kesfet", y.join("otel.db").to_str().unwrap(), "--cikti", c.to_str().unwrap(), "--json"], &bin, &[]);
    assert_eq!(kod, Some(0));
    let j: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(j["plan"].as_array().unwrap().len(), 1);
    assert!(c.join("kaynaksiz.json").exists());
    // Tanınan yapıt yok / yol yok: 1; bilinmeyen bayrak: 2
    let bos = k.join("bos");
    std::fs::create_dir_all(&bos).unwrap();
    assert_eq!(calis(&["kesfet", bos.to_str().unwrap()], &bin, &[]).0, Some(1));
    assert_eq!(calis(&["kesfet", k.join("yok").to_str().unwrap()], &bin, &[]).0, Some(1));
    assert_eq!(calis(&["kesfet", y.to_str().unwrap(), "--reaa"], &bin, &[]).0, Some(2));
    let (kod, out, _) = calis(&["kesfet", "--help"], &bin, &[]);
    assert!(kod == Some(0) && out.contains("--rea"), "{out}");
}

/// UTF-16 metin kodlaması, tırnaklı adlar, WITHOUT ROWID, ON DELETE/UPDATE ve WAL uyarısı
#[test]
fn w2k_sqlite_utf16_ve_wal() {
    let k = gecici("utf16");
    let y = k.join("v");
    kopyala(&ornek().join("utf16"), &y);
    std::fs::write(y.join("utf16.db-wal"), vec![1u8; 64]).unwrap();
    let (bin, _) = tuzak(&k);
    let (kod, out, err) = calis(&["kesfet", y.to_str().unwrap()], &bin, &[]);
    assert_eq!(kod, Some(0), "{out}{err}");
    let b = belge(&y);
    assert_eq!(kanit(&b, "utf16.db#dosya")["ozet"]["kodlama"], "utf-16le");
    let m = &kanit(&b, "utf16.db#tablo:müşteri")["ozet"];
    let adlar: Vec<&str> = m["sutunlar"].as_array().unwrap().iter().map(|s| s["ad"].as_str().unwrap()).collect();
    assert_eq!(adlar, ["no", "ad soyad", "puan"]);
    assert_eq!(m["kontroller"][0], "puan BETWEEN 0 AND 5");
    let s = &kanit(&b, "utf16.db#tablo:siparis")["ozet"];
    assert_eq!(s["rowidsiz"], true);
    let fk = &s["yabanci_anahtarlar"][0];
    assert_eq!((fk["tablo"].as_str(), fk["silinince"].as_str(), fk["guncellenince"].as_str()), (Some("müşteri"), Some("CASCADE"), Some("NO ACTION")));
    assert!(b["bilinmeyenler"].to_string().contains("WAL"), "{}", b["bilinmeyenler"]);
}

/// Basit, tohumlu sözde rastgele üreteç (xorshift)
struct Rastgele(u64);
impl Rastgele {
    fn sonraki(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn alt(&mut self, n: usize) -> usize {
        (self.sonraki() % n.max(1) as u64) as usize
    }
}

fn yaz_u32le(v: &mut [u8], o: usize, x: u32) {
    if o + 4 <= v.len() {
        v[o..o + 4].copy_from_slice(&x.to_le_bytes());
    }
}
fn yaz_u32be(v: &mut [u8], o: usize, x: u32) {
    if o + 4 <= v.len() {
        v[o..o + 4].copy_from_slice(&x.to_be_bytes());
    }
}

/// Bozuk ve kasıtlı kötü PE / SQLite dosyaları: panik yok, komut biter
#[test]
fn w2k_bozuk_pe_ve_sqlite_panik_yok() {
    let k = gecici("bozuk");
    let y = k.join("kotu");
    std::fs::create_dir_all(&y).unwrap();
    let dll = std::fs::read(ornek().join("yayin/OtelRez.dll")).unwrap();
    let exe = std::fs::read(ornek().join("yayin/OtelRez.exe")).unwrap();
    let db = std::fs::read(ornek().join("yayin/otel.db")).unwrap();
    let u16db = std::fs::read(ornek().join("utf16/utf16.db")).unwrap();
    let mut r = Rastgele(0x5eed_1234_abcd_0001);
    let mut n = 0;
    let mut yaz = |ad: String, v: Vec<u8>| {
        std::fs::write(y.join(ad), v).unwrap();
        n += 1;
    };
    // Kırpılmış dosyalar (imza korunur)
    for (i, kaynak) in [&dll, &exe].into_iter().enumerate() {
        for kes in [2, 0x40, 0x80, 0x100, 0x180, 0x200, 0x400, 0x800, 0x1000, kaynak.len() / 2, kaynak.len() - 1] {
            yaz(format!("kes{i}_{kes}.dll"), kaynak[..kes.min(kaynak.len())].to_vec());
        }
    }
    for kes in [16, 100, 120, 512, 1000, 4096, 5000, 8192, db.len() / 3, db.len() - 1] {
        yaz(format!("kes_{kes}.db"), db[..kes].to_vec());
    }
    // Rastgele bayt değişiklikleri (imzanın sonrası)
    for i in 0..120 {
        let mut v = if i % 2 == 0 { dll.clone() } else { exe.clone() };
        for _ in 0..1 + r.alt(40) {
            let o = 2 + r.alt(v.len().min(0x2000) - 2);
            v[o] = r.sonraki() as u8;
        }
        yaz(format!("rast{i}.dll"), v);
    }
    for i in 0..120 {
        let mut v = if i % 3 == 0 { u16db.clone() } else { db.clone() };
        for _ in 0..1 + r.alt(30) {
            let o = 16 + r.alt(v.len().min(3 * 4096) - 16);
            v[o] = r.sonraki() as u8;
        }
        yaz(format!("rast{i}.db"), v);
    }
    // Kasıtlı kötü PE: e_lfanew taşması, dev bölüm sayısı, kaynak dizini kendini gösteriyor, import adı dosya dışı
    let pe = u32::from_le_bytes(dll[0x3c..0x40].try_into().unwrap()) as usize;
    let mut v = dll.clone();
    yaz_u32le(&mut v, 0x3c, 0xffff_fff0);
    yaz("lfanew.dll".into(), v);
    let mut v = dll.clone();
    v[pe + 6] = 0xff;
    v[pe + 7] = 0xff;
    yaz("bolumler.dll".into(), v);
    let mut v = dll.clone();
    v[pe + 20] = 0xff; // isteğe bağlı başlık boyu
    v[pe + 21] = 0xff;
    yaz("secenek.dll".into(), v);
    let mut v = exe.clone();
    // Bütün olası kaynak dizini girdileri yüksek bitli (alt dizin) ve 0'ı gösteriyor: döngü
    for o in (0..v.len()).step_by(4).skip(64).take(2000) {
        if o % 8 == 4 {
            yaz_u32le(&mut v, o, 0x8000_0000);
        }
    }
    yaz("dongu.dll".into(), v);
    // Kasıtlı kötü SQLite: sayfa 1 iç sayfa ve kendini gösteriyor; dev hücre sayısı; geçersiz sayfa boyutu;
    // taşma zinciri döngüsü; dev kayıt başlığı
    let mut v = db.clone();
    v[100] = 0x05;
    yaz_u32be(&mut v, 108, 1);
    yaz("ic_dongu.db".into(), v);
    let mut v = db.clone();
    v[103] = 0xff;
    v[104] = 0xff;
    yaz("hucre.db".into(), v);
    let mut v = db.clone();
    v[16] = 0x03;
    v[17] = 0x00;
    yaz("sayfa_boyu.db".into(), v);
    let mut v = db.clone();
    v[20] = 0xff; // ayrılan alan sayfadan büyük
    yaz("ayrilan.db".into(), v);
    let sayfa = 4096usize;
    let mut v = db.clone();
    for p in 1..(v.len() / sayfa) {
        yaz_u32be(&mut v, p * sayfa, (p + 1) as u32); // her taşma sayfası kendini izleyen sayfayı gösterir
    }
    let son = (v.len() / sayfa - 1) * sayfa;
    yaz_u32be(&mut v, son, 2);
    yaz("tasma_dongu.db".into(), v);
    let mut v = db.clone();
    for o in (108..sayfa).step_by(2) {
        v[o] = 0xff;
    }
    yaz("hucre_konum.db".into(), v);
    let mut v = vec![0u8; 8192];
    v[..16].copy_from_slice(b"SQLite format 3\0");
    v[16] = 0x10;
    v[100] = 0x0d;
    v[103] = 0;
    v[104] = 1;
    v[108] = 0x0f;
    v[109] = 0xf0;
    for b in v.iter_mut().skip(0xff0).take(16) {
        *b = 0xff;
    }
    yaz("varint.db".into(), v);
    assert!(n > 270, "{n}");
    let (bin, _) = tuzak(&k);
    let (kod, out, err) = calis(&["kesfet", y.to_str().unwrap()], &bin, &[]);
    assert!(!err.contains("panicked") && kod != Some(101), "panik: {err}");
    assert_eq!(kod, Some(0), "{out}{err}");
    let b = belge(&y);
    assert!(!b["bilinmeyenler"].as_array().unwrap().is_empty());
    // Tek tek de: her kötü dosya ayrı ayrı panik vermeden biter
    for g in std::fs::read_dir(&y).unwrap().flatten().filter(|g| g.file_type().unwrap().is_file()) {
        let c = k.join("tek");
        let (kod, _, err) = calis(&["kesfet", g.path().to_str().unwrap(), "--cikti", c.to_str().unwrap()], &bin, &[]);
        assert!(!err.contains("panicked") && matches!(kod, Some(0) | Some(1)), "{}: {kod:?} {err}", g.path().display());
    }
}
