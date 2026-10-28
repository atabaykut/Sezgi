//! P1 H2 (MCP araç yüzeyi): başarısız sonuçlar `isError`, bütün araçlarda `annotations`, eş adresli kapı için `islem`
//! argümanı ve açık belirsizlik, yanlış tipli argümana açık hata, İngilizce modda İngilizce parametre adları ve metin,
//! `kimsin` çıktısında okunur genel guard, canlı mod yardımında yeni alt komut ve bayraklar.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Ortam {
    kok: PathBuf,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
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

impl Ortam {
    fn yeni(ad: &str, ornek: &str) -> Ortam {
        static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let kok = std::env::temp_dir().join(format!("sezgi-p1h2-{ad}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("ev")).unwrap();
        kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ornek), &kok.join("app"));
        let o = Ortam { kok };
        let c = o.komut(&["init", "."]).output().unwrap();
        assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
        o
    }
    fn komut(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sezgi"));
        let ev = self.kok.join("ev");
        c.args(args).current_dir(self.kok.join("app")).env("HOME", &ev).env("APPDATA", &ev).env("XDG_CONFIG_HOME", &ev).env("SEZGI_ANAHTARLIK", "kapali");
        c
    }
    /// MCP oturumu: istekler sırayla; her biri için `result` (ya da hata nesnesi)
    fn mcp(&self, ek: &[&str], istekler: &[Value]) -> Vec<Value> {
        let mut args = vec!["mcp", ".sezgi"];
        args.extend_from_slice(ek);
        let mut p = self.komut(&args).env_remove("SEZGI_TOKEN").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let mut stdin = p.stdin.take().unwrap();
        for (i, v) in istekler.iter().enumerate() {
            let mut v = v.clone();
            v["jsonrpc"] = json!("2.0");
            v["id"] = json!(i + 1);
            writeln!(stdin, "{v}").unwrap();
        }
        drop(stdin);
        let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
        c.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["result"].clone()).collect()
    }
    /// Tek araç çağrısı: (metin, isError)
    fn cagir(&self, ek: &[&str], ad: &str, a: Value) -> (String, bool) {
        let r = &self.mcp(ek, &[json!({ "method": "tools/call", "params": { "name": ad, "arguments": a } })])[0];
        (r["content"][0]["text"].as_str().unwrap_or("").to_string(), r["isError"] == true)
    }
    fn araclar(&self, ek: &[&str]) -> Vec<Value> {
        self.mcp(ek, &[json!({ "method": "tools/list" })])[0]["tools"].as_array().unwrap().clone()
    }
}

fn arac<'a>(l: &'a [Value], ad: &str) -> &'a Value {
    l.iter().find(|t| t["name"] == ad).unwrap_or_else(|| panic!("{ad} yok"))
}

#[test]
fn basarisiz_sonuclar_is_error() {
    let o = Ortam::yeni("hata", "nestjs-temel");
    for (ad, a) in [
        ("ara", json!({ "sorgu": "zzzhicyok" })),
        ("tani", json!({ "kapi": "GET /yok/boyle" })),
        ("tani", json!({ "kapi": "/api/v1/kitaplar" })),
        ("onizle", json!({ "kapi": "GET /yok/boyle" })),
        ("onay_durumu", json!({ "id": "yok-boyle" })),
        ("onay_durumu", json!({})),
    ] {
        let (t, hata) = o.cagir(&[], ad, a.clone());
        assert!(hata, "{ad} {a}: başarısız sonuç isError olmalı: {t}");
    }
    let (t, _) = o.cagir(&[], "onay_durumu", json!({}));
    assert!(t.contains("`id`"), "eksik argüman adıyla söylenir: {t}");
    // Başarılı sonuçlar hata değildir
    assert!(!o.cagir(&[], "ara", json!({ "sorgu": "kitap" })).1);
    assert!(!o.cagir(&[], "tani", json!({ "kapi": "POST /api/v1/kitaplar" })).1);
}

#[test]
fn butun_araclarda_annotations() {
    let o = Ortam::yeni("ann", "nestjs-temel");
    for ek in [&[][..], &["--lang", "en"]] {
        let l = o.araclar(ek);
        assert_eq!(l.len(), 9, "{l:?}");
        for t in &l {
            let a = &t["annotations"];
            for alan in ["readOnlyHint", "destructiveHint", "idempotentHint", "openWorldHint"] {
                assert!(a[alan].is_boolean(), "{}: {alan} yok: {t}", t["name"]);
            }
            let cagir = t["name"] == "cagir" || t["name"] == "call";
            assert_eq!(a["readOnlyHint"], !cagir, "{t}");
            assert_eq!(a["destructiveHint"], cagir, "{t}");
        }
    }
}

#[test]
fn es_adresli_kapi_islem_ile_secilir_ve_belirsizlik_bildirilir() {
    let o = Ortam::yeni("islem", "nestjs-surum");
    let l = o.araclar(&[]);
    for ad in ["tani", "cagir", "onizle"] {
        assert!(arac(&l, ad)["inputSchema"]["properties"]["islem"].is_object(), "{ad} şemasında islem yok");
    }
    let l = o.araclar(&["--lang", "en"]);
    for ad in ["describe", "call", "preview"] {
        assert!(arac(&l, ad)["inputSchema"]["properties"]["operation"].is_object(), "{ad} şemasında operation yok");
    }
    let (t, hata) = o.cagir(&[], "tani", json!({ "kapi": "GET /api/siparisler" }));
    assert!(hata, "belirsiz kapı ilk eşleşenle cevaplanmaz: {t}");
    assert!(t.contains("siparis-v1.listele") && t.contains("siparis-v2.listele") && t.contains("islem"), "adaylar ve nasıl seçileceği: {t}");
    let (t, hata) = o.cagir(&[], "tani", json!({ "kapi": "GET /api/siparisler", "islem": "siparis-v2.listele" }));
    assert!(!hata && t.contains("sürüm:2"), "{t}");
    let (t, hata) = o.cagir(&["--lang", "en"], "describe", json!({ "endpoint": "GET /api/siparisler", "operation": "siparis-v1.listele" }));
    assert!(!hata && t.contains("1"), "{t}");
}

#[test]
fn yanlis_tipli_arguman_acik_hata() {
    let o = Ortam::yeni("tip", "nestjs-temel");
    for (ad, a, alan) in [
        ("onizle", json!({ "kapi": "POST /api/v1/kitaplar", "sorgu": "x=1" }), "sorgu"),
        ("onizle", json!({ "kapi": "POST /api/v1/kitaplar", "govde": "baslik=a" }), "govde"),
        ("onizle", json!({ "kapi": "POST /api/v1/kitaplar", "oturum": "evet" }), "oturum"),
        ("onizle", json!({ "kapi": "POST /api/v1/kitaplar", "roller": "ADMIN" }), "roller"),
        ("ara", json!({ "sorgu": 5 }), "sorgu"),
        ("tani", json!({ "kapi": ["POST /api/v1/kitaplar"] }), "kapi"),
    ] {
        let (t, hata) = o.cagir(&[], ad, a.clone());
        assert!(hata, "{ad} {a}: sessizce yok sayılmaz: {t}");
        assert!(t.contains(&format!("`{alan}`")), "{ad}: hatalı argüman adıyla: {t}");
    }
    let (t, hata) = o.cagir(&["--lang", "en"], "preview", json!({ "endpoint": "POST /api/v1/kitaplar", "query_params": "x=1" }));
    assert!(hata && t.contains("`query_params`") && !t.contains("sorgu"), "İngilizce parametre adı: {t}");
}

#[test]
fn ingilizce_modda_ingilizce_metin() {
    let o = Ortam::yeni("en", "nestjs-temel");
    let en = ["--lang", "en"];
    let (t, hata) = o.cagir(&en, "search", json!({}));
    assert!(hata && t.contains("`query`") && !t.contains("sorgu") && !t.contains("gerekli"), "{t}");
    let (t, hata) = o.cagir(&en, "describe", json!({}));
    assert!(hata && t.contains("`endpoint`") && !t.contains("kapi"), "{t}");
    let (t, hata) = o.cagir(&en, "describe", json!({ "endpoint": "GET /yok" }));
    assert!(hata && t.contains("not found") && !t.contains("bulunamadı"), "{t}");
    let (t, _) = o.cagir(&en, "app", json!({}));
    assert!(t.contains("Application") && t.contains("endpoints") && !t.contains("Uygulama") && !t.contains("kapı"), "{t}");
    let (t, _) = o.cagir(&en, "describe", json!({ "endpoint": "POST /api/v1/kitaplar" }));
    for tr in ["kaynak:", "gövde", "zorunlu", "giriş:", "isteğe bağlı"] {
        assert!(!t.contains(tr), "describe İngilizce: '{tr}' kaldı:\n{t}");
    }
    assert!(t.contains("source:") && t.contains("required") && t.contains("login:"), "{t}");
    let (t, hata) = o.cagir(&en, "search", json!({ "query": "zzzhicyok" }));
    assert!(hata && !t.contains("bulunamadı"), "{t}");
}

#[test]
fn kimsin_genel_guard_okunur() {
    let o = Ortam::yeni("kimsin", "nestjs-temel");
    let (t, _) = o.cagir(&[], "kimsin", json!({}));
    assert!(t.contains("genel guard JwtAuthGuard"), "{t}");
    assert!(!t.contains("[\"") && !t.contains("\"]"), "ham JSON yok: {t}");
}

#[test]
fn canli_mod_yardimi_yeni_komutlari_anlatir() {
    let o = Ortam::yeni("yardim", "nestjs-temel");
    let m = |a: &[&str]| String::from_utf8_lossy(&o.komut(a).output().unwrap().stdout).to_string();
    assert!(m(&["yardim", "canli"]).contains("canli tasi"), "{}", m(&["yardim", "canli"]));
    let onay = m(&["yardim", "onay"]);
    assert!(onay.contains("onay goster") && onay.contains("--ozet"), "{onay}");
    assert!(m(&["yardim", "hafiza"]).contains("--oturumlu"), "{}", m(&["yardim", "hafiza"]));
}
