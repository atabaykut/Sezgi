//! D8 İngilizce (MCP): `sezgi mcp --lang en` ile initialize, tools/list ve bütün genel araçların çıktısı İngilizcedir;
//! önizlemenin ayrıntı satırları, davranış özeti (çağrı zinciri, dış etki, geri alma gerekçesi), anlam özeti ve görev
//! planı dahil. Uygulamanın kendi verisi (yol, alan, kod ifadesi) ve Sezgi komut adları (`sezgi init`) olduğu gibi kalır.
//! Türkçe çıktı değişmez; `SEZGI_LANG=en` de aynı İngilizce sunucuyu açar.
use regex::Regex;
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
        let kok = std::env::temp_dir().join(format!("sezgi-d8ing-mcp-{ad}-{}-{n}", std::process::id()));
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
        c.args(args)
            .current_dir(self.kok.join("app"))
            .env("HOME", &ev)
            .env("APPDATA", &ev)
            .env("XDG_CONFIG_HOME", &ev)
            .env("SEZGI_ANAHTARLIK", "kapali")
            .env_remove("SEZGI_LANG")
            .env_remove("SEZGI_TOKEN");
        c
    }
    /// MCP oturumu: istekler sırayla; her biri için `result`
    fn mcp_ortamla(&self, ek: &[&str], ortam: &[(&str, &str)], istekler: &[Value]) -> Vec<Value> {
        let mut args = vec!["mcp", ".sezgi"];
        args.extend_from_slice(ek);
        let mut k = self.komut(&args);
        for (a, d) in ortam {
            k.env(a, d);
        }
        let mut p = k.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
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
    fn mcp(&self, ek: &[&str], istekler: &[Value]) -> Vec<Value> {
        self.mcp_ortamla(ek, &[], istekler)
    }
    /// Birden fazla araç çağrısı tek oturumda: [(metin, isError)]
    fn cagrilar(&self, ek: &[&str], cagrilar: &[(&str, Value)]) -> Vec<(String, bool)> {
        let istekler: Vec<Value> = cagrilar.iter().map(|(ad, a)| json!({ "method": "tools/call", "params": { "name": ad, "arguments": a } })).collect();
        self.mcp(ek, &istekler).iter().map(|r| (r["content"][0]["text"].as_str().unwrap_or("").to_string(), r["isError"] == true)).collect()
    }
    fn harita(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.kok.join("app").join(".sezgi").join("kapilar.json")).unwrap()).unwrap()
    }
}

/// Sezgi'nin Türkçe çıktısında geçen, İngilizce metinde ve bu örneklerin verisinde bulunmayan sözcükler (aksansız, küçük)
const TURKCE: [&str; 47] = [
    "kaskad", "silinince", "veritabani",
    "kapi", "kapilar", "islem", "islemi", "islemler", "giris", "gerekli", "bilinmiyor", "yorum", "kesin", "zorunlu", "yetki", "sonuc", "reddedilir",
    "gecer", "degil", "olmali", "verildi", "bulunamadi", "icin", "daha", "govde", "sorgu", "tablolar", "roller", "surum", "guven", "onay", "ekran",
    "yazar", "okur", "kanit", "onerme", "uygulama", "uygulamanin", "alani", "adim", "gozlem", "ya", "ve", "ile", "gibi", "uygulamasi", "kontrol",
];

fn sade(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ç' | 'Ç' => 'c',
            'ğ' | 'Ğ' => 'g',
            'ı' | 'I' | 'İ' => 'i',
            'ö' | 'Ö' => 'o',
            'ş' | 'Ş' => 's',
            'ü' | 'Ü' => 'u',
            x => x.to_ascii_lowercase(),
        })
        .collect()
}

/// Metinde kalan Türkçe: Türkçe harfler ve Türkçe sözcükler. Kod parçaları (ters tırnak), Sezgi komutları (`sezgi init`,
/// `sezgi mod okuma`) ve uygulamanın verisi (`veri` listesindeki metinler) sayılmaz
fn turkce_kalinti(metin: &str, veri: &[String]) -> Vec<String> {
    let mut m = metin.to_string();
    let veri_tum = veri;
    let mut veri: Vec<&String> = veri.iter().filter(|v| v.chars().count() >= 3).collect();
    veri.sort_by_key(|v| std::cmp::Reverse(v.len()));
    for v in veri {
        m = m.replace(v.as_str(), " ");
    }
    let kod = Regex::new(r"`[^`]*`").unwrap();
    let komut = Regex::new(r"sezgi( [a-z0-9_.<>|:\-]+)+").unwrap();
    // Tırnak içindeki alıntılar (kullanıcının isteği, koddaki özet) ve işlem kimlikleri (`yorum.toplu-sil`) uygulamanın verisidir
    let alinti = Regex::new(r#""[^"]*"|“[^”]*”|«[^»]*»|[\w-]+\.[\w.-]+"#).unwrap();
    let m = komut.replace_all(&kod.replace_all(&m, " "), " ").to_string();
    let m = alinti.replace_all(&m, " ").to_string();
    let mut v = vec![];
    for c in m.chars().filter(|c| "çğıöşüÇĞİÖŞÜ".contains(*c)) {
        v.push(c.to_string());
    }
    // Uygulamanın kendi sözcükleri (yol parçası, modül adı: `surum`, `yorum`) Türkçe sayılmaz
    let uygulama_sozcukleri: std::collections::HashSet<String> =
        veri_tum.iter().flat_map(|v| v.split(|c: char| !c.is_alphanumeric()).map(sade).collect::<Vec<_>>()).collect();
    for w in m.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        if TURKCE.contains(&sade(w).as_str()) && !uygulama_sozcukleri.contains(&sade(w)) {
            v.push(w.to_string());
        }
    }
    v.sort();
    v.dedup();
    v
}

/// Fonksiyon adının okunur hâli (Sezgi özeti olmayan işlemi böyle anlatır): postApiNotlar -> "post api notlar"
fn addan(ad: &str) -> String {
    let mut s = String::new();
    let mut onceki_kucuk = false;
    for c in ad.chars() {
        if !c.is_alphanumeric() {
            s.push(' ');
            onceki_kucuk = false;
            continue;
        }
        if c.is_uppercase() && onceki_kucuk {
            s.push(' ');
        }
        onceki_kucuk = c.is_lowercase() || c.is_ascii_digit();
        s.extend(c.to_lowercase());
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Uygulamanın kendi verisi: haritadaki kapıların kimliği, yolu, işleyici ve modül adları, alan ve rol adları, kod
/// konumları
fn uygulama_verisi(h: &Value) -> Vec<String> {
    let mut v = vec![];
    for k in h["endpointler"].as_array().into_iter().flatten() {
        for a in ["id", "yol", "kontrolcu", "fonksiyon", "ozet", "dosya", "uygulama"] {
            if let Some(s) = k[a].as_str() {
                v.push(s.to_string());
            }
        }
        v.push(k["kontrolcu"].as_str().unwrap_or("").trim_end_matches("Controller").to_string());
        v.push(addan(k["fonksiyon"].as_str().unwrap_or("")));
        for r in k["roller"].as_array().into_iter().flatten().chain(k["guardlar"].as_array().into_iter().flatten()).filter_map(Value::as_str) {
            v.push(r.to_string());
        }
    }
    if let Some(a) = h["ozet"]["uygulama"].as_str() {
        v.push(a.to_string());
    }
    v
}

/// JSON'daki bütün metin değerleri (açıklamalar), satır satır
fn json_metinleri(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{s}
"),
        Value::Array(a) => a.iter().map(json_metinleri).collect(),
        Value::Object(m) => m.values().map(json_metinleri).collect(),
        _ => String::new(),
    }
}

fn kontrol(adim: &str, metin: &str, veri: &[String]) {
    let k = turkce_kalinti(metin, veri);
    assert!(k.is_empty(), "{adim}: Türkçe kaldı {k:?}\n{metin}");
}

/// Örnek uygulamanın bütün kapıları için describe, preview (bozuk gövdeyle), impact, evidence ve unknowns; ayrıca app,
/// search, search task, call ve hata metinleri
fn butun_araclar_ingilizce(ornek: &str, arama: &str, gorev: &str) {
    let o = Ortam::yeni(&format!("tum-{ornek}"), ornek);
    let h = o.harita();
    // Uygulamanın verisi ve kullanıcının kendi isteği (görev planı isteği alıntılar)
    let mut veri = uygulama_verisi(&h);
    veri.extend([arama.to_string(), gorev.to_string()]);
    let en = ["--lang", "en"];
    // initialize ve tools/list
    let r = o.mcp(&en, &[json!({ "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }), json!({ "method": "tools/list" })]);
    kontrol("initialize", r[0]["instructions"].as_str().unwrap(), &veri);
    for t in r[1]["tools"].as_array().unwrap() {
        kontrol(&format!("tools/list {}", t["name"]), &json_metinleri(t), &veri);
    }
    let mut cagrilar: Vec<(String, Value)> = vec![
        ("app".into(), json!({})),
        ("search".into(), json!({ "query": arama })),
        ("search".into(), json!({ "query": gorev, "task": true })),
        ("search".into(), json!({ "query": "zzzhicyok" })),
        ("describe".into(), json!({ "endpoint": "GET /yok/boyle" })),
        ("preview".into(), json!({ "endpoint": "GET /yok/boyle" })),
        ("unknowns".into(), json!({})),
        ("impact".into(), json!({ "entity": "db:yok" })),
        ("call".into(), json!({ "endpoint": "GET /yok/boyle" })),
        ("approval_status".into(), json!({ "id": "yok" })),
        ("describe".into(), json!({})),
    ];
    for k in h["endpointler"].as_array().unwrap() {
        let id = k["id"].as_str().unwrap();
        let islem = json!(null);
        cagrilar.push(("describe".into(), json!({ "endpoint": id, "operation": islem })));
        // Bozuk gövde, oturumsuz istek, yanlış rol: önizlemenin red ve denetlenemeyen satırları
        cagrilar.push(("preview".into(), json!({ "endpoint": id, "body": { "zzz": 1 }, "logged_in": false, "roles": ["zzz"] })));
        cagrilar.push(("preview".into(), json!({ "endpoint": id })));
        cagrilar.push(("impact".into(), json!({ "endpoint": id })));
        cagrilar.push(("evidence".into(), json!({ "endpoint": id })));
        cagrilar.push(("unknowns".into(), json!({ "endpoint": id })));
    }
    let istek: Vec<(&str, Value)> = cagrilar.iter().map(|(a, v)| (a.as_str(), v.clone())).collect();
    for ((ad, a), (metin, _)) in istek.iter().zip(o.cagrilar(&en, &istek)) {
        kontrol(&format!("{ornek} {ad} {a}"), &metin, &veri);
    }
}

#[test]
fn cagri_ve_tablo_ornegi_ingilizce() {
    butun_araclar_ingilizce("d8_cagri_nest", "delete album", "delete the album and then list the albums");
}

#[test]
fn dis_etki_ve_geri_alma_ornegi_ingilizce() {
    butun_araclar_ingilizce("d8_dis_express", "create order", "create an order and delete the note");
}

#[test]
fn anlam_ornegi_ingilizce() {
    butun_araclar_ingilizce("d8_anlam_foto", "delete user", "delete the user and hand their albums to someone else");
}

#[test]
fn davranis_ozeti_ornegi_ingilizce() {
    butun_araclar_ingilizce("w2dna_nest", "delete", "create and delete");
}

#[test]
fn kurallar_ornegi_ingilizce() {
    butun_araclar_ingilizce("nestjs-temel", "create book", "create a book and delete the comment");
}

/// Önizleme sonucu ve red satırları İngilizce; Türkçe sonuç birebir eskisi gibi
#[test]
fn onizleme_ayrintisi_iki_dilde() {
    let o = Ortam::yeni("onizle", "nestjs-temel");
    let istek = json!({ "endpoint": "POST /api/v1/kitaplar", "body": { "sayfa": "cok" }, "logged_in": false });
    let r = o.cagrilar(&["--lang", "en"], &[("preview", istek)]);
    let t = &r[0].0;
    assert!(t.starts_with("PREVIEW POST /api/v1/kitaplar · nothing was sent"), "{t}");
    assert!(t.contains("result: REJECTED") && t.contains("required field missing") && t.contains("Note: the application has the final say"), "{t}");
    let r = o.cagrilar(&[], &[("onizle", json!({ "kapi": "POST /api/v1/kitaplar", "govde": { "sayfa": "cok" }, "oturum": false }))]);
    let t = &r[0].0;
    assert!(t.starts_with("ÖNİZLEME POST /api/v1/kitaplar · hiçbir istek gönderilmedi"), "{t}");
    assert!(t.contains("sonuç: REDDEDİLİR") && t.contains("zorunlu alan eksik") && t.contains("Not: son karar uygulamanındır"), "{t}");
}

/// İngilizce talimat artık "ayrıntılar Türkçe" demez; damgalar certain/observed/inferred/unknown
#[test]
fn talimat_turkce_ayrinti_sinirini_soylemez() {
    let o = Ortam::yeni("talimat", "nestjs-temel");
    let r = o.mcp(&["--lang", "en"], &[json!({ "method": "initialize", "params": {} })]);
    let t = r[0]["instructions"].as_str().unwrap();
    assert!(!t.contains("Turkish \\(") && !t.contains("[Kesin]") && !t.contains("still in Turkish"), "{t}");
    assert!(t.contains("[Certain]") && t.contains("[Observed]") && t.contains("[Inferred]") && t.contains("[Unknown]"), "{t}");
}

/// `SEZGI_LANG=en` bayraksız da İngilizce sunucu açar; bayrak ortamdan önce gelir
#[test]
fn ortam_degiskeniyle_ingilizce() {
    let o = Ortam::yeni("ortam", "nestjs-temel");
    let liste = |ek: &[&str], ortam: &[(&str, &str)]| -> Vec<String> {
        o.mcp_ortamla(ek, ortam, &[json!({ "method": "tools/list" })])[0]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
    };
    assert!(liste(&[], &[("SEZGI_LANG", "en")]).contains(&"describe".to_string()));
    assert!(liste(&["--lang", "tr"], &[("SEZGI_LANG", "en")]).contains(&"tani".to_string()));
    assert!(liste(&[], &[("SEZGI_LANG", "tr")]).contains(&"tani".to_string()));
}

/// Türkçe çıktı değişmedi: kimsin, tani, etki ve görev planı eski sözcüklerle
#[test]
fn turkce_cikti_degismedi() {
    let o = Ortam::yeni("tr", "d8_cagri_nest");
    let h = o.harita();
    let sil = h["endpointler"].as_array().unwrap().iter().find(|k| k["fonksiyon"] == "deleteAlbum").unwrap()["id"].as_str().unwrap().to_string();
    let r = o.cagrilar(
        &[],
        &[
            ("kimsin", json!({})),
            ("tani", json!({ "kapi": sil })),
            ("etki", json!({ "kapi": sil })),
            ("ara", json!({ "sorgu": "albümü sil ve albümleri listele", "gorev": true })),
        ],
    );
    assert!(r[0].0.starts_with("Uygulama: ") && r[0].0.contains("Modüller (ad · kapı sayısı · ortak yol)"), "{}", r[0].0);
    assert!(r[1].0.contains("kaynak:") && r[1].0.contains("çağrı zinciri"), "{}", r[1].0);
    assert!(r[2].0.starts_with("Etki: ") && r[2].0.contains("Kapsam dışı:"), "{}", r[2].0);
    assert!(r[3].0.starts_with("GÖREV PLANI [Yorum]"), "{}", r[3].0);
}

/// Canlı modun önizleme kararını okuduğu sonuç satırları iki dilde de aynı biçimde: reddedilen istek "REJECTED",
/// bütün kuralları geçen istek "result: PASS (all known rules)"
#[test]
fn onizleme_sonuc_satirlari_sabit() {
    let o = Ortam::yeni("sonuc", "nestjs-temel");
    let gecer = json!({ "metin": "güzel", "puan": 4, "kitapId": "8a3c1e2b-1111-2222-3333-444455556666" });
    let r = o.cagrilar(
        &["--lang", "en"],
        &[
            ("preview", json!({ "endpoint": "POST /api/v1/yorumlar", "body": gecer, "logged_in": true })),
            ("preview", json!({ "endpoint": "POST /api/v1/yorumlar", "body": { "puan": 99 }, "logged_in": true })),
        ],
    );
    assert!(r[0].0.lines().any(|l| l == "result: PASS (all known rules)"), "{}", r[0].0);
    assert!(r[1].0.contains("result: REJECTED ("), "{}", r[1].0);
    let r = o.cagrilar(
        &[],
        &[
            ("onizle", json!({ "kapi": "POST /api/v1/yorumlar", "govde": gecer, "oturum": true })),
            ("onizle", json!({ "kapi": "POST /api/v1/yorumlar", "govde": { "puan": 99 }, "oturum": true })),
        ],
    );
    assert!(r[0].0.lines().any(|l| l == "sonuç: GEÇER (bilinen bütün kurallar)"), "{}", r[0].0);
    assert!(r[1].0.contains("sonuç: REDDEDİLİR ("), "{}", r[1].0);
}

/// Onay durumu metni sunucunun dilinde (metin karşılaştırmasıyla değil, çağrının diliyle)
#[test]
fn onay_durumu_iki_dilde() {
    let o = Ortam::yeni("onay", "nestjs-temel");
    let (t, hata) = o.cagrilar(&["--lang", "en"], &[("approval_status", json!({ "id": "yok-boyle" }))]).remove(0);
    assert!(hata && t.starts_with("approval not found: yok-boyle"), "{t}");
    let (t, hata) = o.cagrilar(&[], &[("onay_durumu", json!({ "id": "yok-boyle" }))]).remove(0);
    assert!(hata && t.starts_with("onay bulunamadı: yok-boyle"), "{t}");
}
