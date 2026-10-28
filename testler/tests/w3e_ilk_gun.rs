//! W3 uygulamalar arası varlık eşlemesi ve değişiklik planı ("şirkette ilk gün" minyatürü). İki uygulama santralde:
//! rezervasyon (misafir, rezervasyon, varış tarihi) ve transfer (transfer, yolcu, araç kapasitesi).
//! - Eşleme bir önermedir: insan tanımı (USER_CONFIRMED) kaynak kanıtı olmadan en fazla observed; öneri motoru (ad, tip,
//!   şema benzerliği) hiçbir zaman certain üretmez. Depolama santral yapılandırma klasöründe JSON.
//! - Santral MCP: `varliklar`; `etki` uygulamalar arası (eşlenmiş varlığa MAY_AFFECT, asla certain); `plan` iki uygulamadaki
//!   işlemleri tek değişiklik planı olarak kaydeder, YÜRÜTMEZ.
//! - İnsan `sezgi plan goster` ile gövdeleri görür, `sezgi plan onayla --adim n --ozet <özet>` ile adım adım onaylar; her
//!   adım canlı modun politikasından geçer (token izni, rol, bayatlık, mod, test kopyası, önizleme, içerik özeti).
//!   Ajanın rol beyanı ya da plan argümanı kapıyı aşamaz.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

/// Santral MCP oturumu: istekler sırayla, cevaplar (metin, isError)
struct Santral {
    p: std::process::Child,
    stdin: std::process::ChildStdin,
    satirlar: Box<dyn Iterator<Item = std::io::Result<String>>>,
    id: u64,
}

impl Drop for Santral {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
    }
}

impl Santral {
    fn ac(p: &Proje, token: &str, ek: &[&str]) -> Santral {
        let mut a = vec!["mcp", "--santral"];
        a.extend_from_slice(ek);
        let mut c = p.komut(&a).env("SEZGI_TOKEN", token).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let stdin = c.stdin.take().unwrap();
        let satirlar = Box::new(BufReader::new(c.stdout.take().unwrap()).lines());
        Santral { p: c, stdin, satirlar, id: 0 }
    }
    fn istek(&mut self, yontem: &str, params: Value) -> Value {
        self.id += 1;
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": self.id, "method": yontem, "params": params })).unwrap();
        self.stdin.flush().unwrap();
        serde_json::from_str::<Value>(&self.satirlar.next().unwrap().unwrap()).unwrap()["result"].clone()
    }
    fn arac(&mut self, ad: &str, a: Value) -> (String, bool) {
        let r = self.istek("tools/call", json!({ "name": ad, "arguments": a }));
        (r["content"][0]["text"].as_str().unwrap_or("").to_string(), r["isError"] == true)
    }
}

/// İki uygulama, santral, canlı mod (test kopyası hedefi, yazma, insan oturumu) ve ajan token'ları
struct Kurulum {
    ev: Ev,
    rez: Proje,
    tr: Proje,
    rez_app: Uygulama,
    tr_app: Uygulama,
    token: String,
}

fn kur(ad: &str, tr_rolu: &[&str]) -> Kurulum {
    let ev = Ev::yeni(ad);
    let rez = ev.proje("w3e_rezervasyon", "rez");
    let tr = ev.proje("w3e_transfer", "tr");
    rez.ok(&["santral", "ekle", ".sezgi", "--ad", "rezervasyon"]);
    tr.ok(&["santral", "ekle", ".sezgi", "--ad", "transfer"]);
    let (rez_app, tr_app) = (Uygulama::yeni(), Uygulama::yeni());
    rez.ok(&["hedef", &rez_app.adres(), "--test-kopyasi"]);
    rez.ok(&["mod", "yazma"]);
    rez.oturum("OTURUM-REZ");
    tr.ok(&["hedef", &tr_app.adres(), "--test-kopyasi"]);
    tr.ok(&["mod", "yazma"]);
    tr.oturum_rollu("OTURUM-TR", tr_rolu);
    let t1 = rez.token("ajan", &["rezervasyon.*:yaz"], &[]);
    let t2 = tr.token("ajan", &["transfer.*:yaz"], &[]);
    Kurulum { ev, rez, tr, rez_app, tr_app, token: format!("{t1},{t2}") }
}

fn plan_kimligi(t: &str) -> String {
    t.split("PLAN ").nth(1).unwrap_or_else(|| panic!("plan kimliği yok: {t}")).split_whitespace().next().unwrap().to_string()
}

/// `plan goster` çıktısından adımın özeti (`--adim n --ozet <özet>`)
fn adim_ozeti(goster: &str, n: usize) -> String {
    let anahtar = format!("--adim {n} --ozet ");
    goster.split(&anahtar).nth(1).unwrap_or_else(|| panic!("adım {n} özeti yok: {goster}")).split_whitespace().next().unwrap().to_string()
}

fn yazmalar(u: &Uygulama) -> Vec<Gelen> {
    u.gelenler.lock().unwrap().iter().filter(|g| g.yontem != "GET").cloned().collect()
}

fn iki_adimli_plan() -> Value {
    json!({
        "baslik": "Yarın gelenlerin transferi",
        "adimlar": [
            { "uygulama": "rezervasyon", "kapi": "PATCH /api/rezervasyonlar/r1", "govde": { "transferNotu": "Havalimanı 10:00, 3 kişi" } },
            { "uygulama": "transfer", "kapi": "POST /api/transferler", "govde": { "rezervasyonRef": "R-2026-001", "yolcuId": "y1", "aracId": "a1", "yolcuSayisi": 3 } }
        ]
    })
}

#[test]
fn varlik_onerileri_ve_insan_tanimi() {
    let k = kur("w3e-varlik", &["OPERASYON"]);
    // Öneriler: ad ve tip benzerliği, dış kimlik adı; hiçbiri certain değil
    let v: Value = serde_json::from_str(&k.rez.ok(&["santral", "varliklar", "--json"])).unwrap();
    let oneriler = v["oneriler"].as_array().unwrap_or_else(|| panic!("{v}"));
    let var = |a: &str, b: &str| oneriler.iter().any(|o| (o["a"] == a && o["b"] == b) || (o["a"] == b && o["b"] == a));
    assert!(var("rezervasyon:Misafir.pasaportNo", "transfer:Yolcu.pasaportNo"), "aynı adlı ve tipli alan önerilir: {v:#}");
    assert!(var("rezervasyon:Rezervasyon.rezervasyonNo", "transfer:Transfer.rezervasyonRef"), "dış kimlik adı önerilir: {v:#}");
    assert!(oneriler.iter().all(|o| o["guven"] == "inferred"), "öneri motoru certain üretmez: {v:#}");
    assert!(v["eslemeler"].as_array().unwrap().is_empty());
    // Hatalı tanımlar reddedilir
    for (a, b) in
        [("yok:Misafir.pasaportNo", "transfer:Yolcu.pasaportNo"), ("rezervasyon:Hayalet.x", "transfer:Yolcu.pasaportNo"), ("rezervasyon:Misafir", "transfer:Yolcu.pasaportNo")]
    {
        let (ok, m) = k.rez.cikti(&["santral", "bagla-varlik", a, b]);
        assert!(!ok, "{a} ↔ {b} reddedilmeli: {m}");
    }
    // İnsan tanımı: USER_CONFIRMED, kaynak kanıtı olmadan en fazla observed
    k.rez.ok(&["santral", "bagla-varlik", "rezervasyon:Misafir.pasaportNo", "transfer:Yolcu.pasaportNo"]);
    k.tr.ok(&["santral", "bagla-varlik", "transfer:Transfer.rezervasyonRef", "rezervasyon:Rezervasyon.rezervasyonNo", "--anahtar", "rezervasyon-no"]);
    let v: Value = serde_json::from_str(&k.rez.ok(&["santral", "varliklar", "--json"])).unwrap();
    let e = v["eslemeler"].as_array().unwrap();
    assert_eq!(e.len(), 2, "{v:#}");
    for x in e {
        assert_eq!(x["guven"], "observed", "insan tanımı tek başına observed: {x}");
        assert!(x["kanitlar"].as_array().unwrap().iter().any(|k| k["kaynak_turu"] == "USER_CONFIRMED"), "{x}");
        assert_eq!(x["gecerlilik"], "gecerli", "{x}");
    }
    // Eşlenen öneri artık öneri listesinde değil; değer saklanmaz (yalnızca şema)
    let oneriler = v["oneriler"].as_array().unwrap();
    assert!(!oneriler.iter().any(|o| o["a"] == "rezervasyon:Misafir.pasaportNo" || o["b"] == "rezervasyon:Misafir.pasaportNo"), "{v:#}");
    let depo = std::fs::read_to_string(k.ev.ayar().join("sezgi").join("santral-varliklar.json")).unwrap_or_else(|_| {
        // Yapılandırma klasörünün adı sisteme göre değişir: ayar altında ara
        fn bul(d: &std::path::Path) -> Option<std::path::PathBuf> {
            for g in std::fs::read_dir(d).ok()?.flatten() {
                let p = g.path();
                if p.file_name().is_some_and(|a| a == "santral-varliklar.json") {
                    return Some(p);
                }
                if p.is_dir() {
                    if let Some(x) = bul(&p) {
                        return Some(x);
                    }
                }
            }
            None
        }
        std::fs::read_to_string(bul(&k.ev.kok).expect("santral-varliklar.json yok")).unwrap()
    });
    assert!(depo.contains("USER_CONFIRMED") && depo.contains("pasaportNo"), "{depo}");
    // Dayanak değişince eşleme eski olur (şema dosyası değişti)
    let sema = k.tr.klasor.join("prisma").join("schema.prisma");
    let mut m = std::fs::read_to_string(&sema).unwrap();
    m.push_str("\n// değişti\n");
    std::fs::write(&sema, m).unwrap();
    let v: Value = serde_json::from_str(&k.rez.ok(&["santral", "varliklar", "--json"])).unwrap();
    let x = v["eslemeler"].as_array().unwrap().iter().find(|x| x["a"].as_str().unwrap().contains("pasaportNo")).unwrap().clone();
    assert_eq!(x["gecerlilik"], "eski", "{x}");
    assert_eq!(x["guven"], "unknown", "{x}");
    // Metin çıktısı güven etiketleriyle
    let t = k.rez.ok(&["santral", "varliklar"]);
    assert!(t.contains("[Bilinmiyor]") || t.contains("eski"), "{t}");
}

#[test]
fn santral_mcp_varliklar_etki_ve_kapasite_kurali() {
    let k = kur("w3e-mcp", &["OPERASYON"]);
    k.rez.ok(&["santral", "bagla-varlik", "rezervasyon:Misafir.pasaportNo", "transfer:Yolcu.pasaportNo"]);
    k.rez.ok(&["santral", "bagla-varlik", "rezervasyon:Rezervasyon.rezervasyonNo", "transfer:Transfer.rezervasyonRef"]);
    let mut s = Santral::ac(&k.rez, &k.token, &[]);
    let araclar = s.istek("tools/list", json!({}));
    let adlar: Vec<&str> = araclar["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for ad in ["varliklar", "plan", "etki"] {
        assert!(adlar.contains(&ad), "{ad} yok: {adlar:?}");
    }
    let plan = araclar["tools"].as_array().unwrap().iter().find(|t| t["name"] == "plan").unwrap();
    assert_eq!(plan["annotations"]["destructiveHint"], false, "plan yürütmez: {plan}");
    // Yarın gelenler: okuma, rezervasyon uygulamasına gider
    let (t, hata) = s.arac("cagir", json!({ "kapi": "rezervasyon: GET /api/rezervasyonlar", "sorgu": { "varis": "2026-10-22" } }));
    assert!(!hata && t.starts_with("OKUNDU"), "{t}");
    assert!(k.rez_app.gelenler.lock().unwrap().iter().any(|g| g.yol.contains("varis=2026-10-22")), "okuma rezervasyon uygulamasına gitmeli");
    // Eşlemeler güvenleriyle
    let (t, hata) = s.arac("varliklar", json!({}));
    assert!(!hata && t.contains("Misafir.pasaportNo") && t.contains("Yolcu.pasaportNo") && t.contains("[Gözlem]"), "{t}");
    // Uygulamalar arası etki: rezervasyonu güncellemek eşlenmiş transfer varlığını etkileyebilir (MAY_AFFECT, asla kesin)
    let (t, hata) = s.arac("etki", json!({ "kapi": "rezervasyon: PATCH /api/rezervasyonlar/:id" }));
    assert!(!hata, "{t}");
    let satir = t.lines().find(|l| l.contains("transfer:Transfer")).unwrap_or_else(|| panic!("eşlenmiş varlık etkide yok: {t}"));
    assert!(satir.contains("MAY_AFFECT") && !satir.contains("[Kesin]"), "{satir}");
    assert!(t.contains("GET /api/transferler"), "eşlenmiş varlığı okuyan işlem: {t}");
    // Kapasite iş kuralı: transfer oluşturmanın kuralları (kart DNA'sı) görünür
    let (t, _) = s.arac("tani", json!({ "kapi": "transfer: POST /api/transferler" }));
    assert!(t.contains("yolcuSayisi") && t.contains("Max(8)"), "{t}");
    // Token görünürlüğü: yalnızca rezervasyon token'ı verilirse transfer tarafı gizli kalır
    let t1 = k.token.split(',').next().unwrap().to_string();
    let mut s1 = Santral::ac(&k.rez, &t1, &[]);
    let (t, _) = s1.arac("etki", json!({ "kapi": "rezervasyon: PATCH /api/rezervasyonlar/:id" }));
    assert!(!t.contains("GET /api/transferler"), "gizli uygulamanın işlemi görünmez: {t}");
}

#[test]
fn plan_yurutmez_insan_adim_adim_onaylar() {
    let k = kur("w3e-plan", &["OPERASYON"]);
    k.rez.ok(&["santral", "bagla-varlik", "rezervasyon:Rezervasyon.rezervasyonNo", "transfer:Transfer.rezervasyonRef"]);
    let mut s = Santral::ac(&k.rez, &k.token, &[]);
    let (t, hata) = s.arac("plan", iki_adimli_plan());
    assert!(!hata, "{t}");
    let id = plan_kimligi(&t);
    assert!(t.contains("transfer:Transfer"), "adımın etkilediği eşlenmiş varlık: {t}");
    assert!(yazmalar(&k.rez_app).is_empty() && yazmalar(&k.tr_app).is_empty(), "plan yürütmez");
    // İnsan gövdeleri görür
    let g = k.rez.ok(&["plan", "goster", &id]);
    for parca in ["Havalimanı 10:00", "yolcuSayisi", "rezervasyon", "transfer", "PATCH", "POST", "yolcuSayisi: Max(8)"] {
        assert!(g.contains(parca), "{parca}: {g}");
    }
    let (o1, o2) = (adim_ozeti(&g, 1), adim_ozeti(&g, 2));
    // Terminal yok, özet yok: gönderilmez. Yanlış özet: gönderilmez. Sıra atlanamaz
    assert!(!k.rez.cikti(&["plan", "onayla", &id, "--adim", "1"]).0);
    assert!(!k.rez.cikti(&["plan", "onayla", &id, "--adim", "1", "--ozet", "00000000"]).0);
    assert!(!k.rez.cikti(&["plan", "onayla", &id, "--adim", "2", "--ozet", &o2]).0, "önceki adım bitmeden sonraki onaylanmaz");
    assert!(yazmalar(&k.rez_app).is_empty() && yazmalar(&k.tr_app).is_empty());
    // Adım 1: rezervasyon test kopyasına, insanın oturumuyla
    k.rez.ok(&["plan", "onayla", &id, "--adim", "1", "--ozet", &o1]);
    let y = yazmalar(&k.rez_app);
    assert_eq!(y.len(), 1, "{y:?}");
    assert!(y[0].yontem == "PATCH" && y[0].yol == "/rezervasyonlar/r1" || y[0].yol.ends_with("/rezervasyonlar/r1"), "{y:?}");
    assert!(y[0].govde.contains("Havalimanı 10:00") && y[0].yetki.as_deref().unwrap_or("").contains("OTURUM-REZ"), "{y:?}");
    // Aynı adım ikinci kez yapılmaz
    assert!(!k.rez.cikti(&["plan", "onayla", &id, "--adim", "1", "--ozet", &o1]).0);
    assert_eq!(yazmalar(&k.rez_app).len(), 1);
    // Adım 2: transfer test kopyasına
    k.tr.ok(&["plan", "onayla", &id, "--adim", "2", "--ozet", &o2]);
    let y = yazmalar(&k.tr_app);
    assert_eq!(y.len(), 1, "{y:?}");
    assert!(y[0].govde.contains("R-2026-001") && y[0].yetki.as_deref().unwrap_or("").contains("OTURUM-TR"), "{y:?}");
    let l = k.rez.ok(&["plan", "listele"]);
    assert!(l.contains(&id) && l.contains("tamam"), "{l}");
    // Kayıt: her iki uygulamanın zincirli kaydında plan adımı
    assert!(k.rez.ok(&["kayit", "--json"]).contains("plan"), "rezervasyon kaydı");
}

#[test]
fn plan_politikayi_atlatamaz() {
    // İnsanın doğrulanmış rolü SOFOR: OPERASYON isteyen güncelleme önizlemede reddedilir
    let k = kur("w3e-atlatma", &["SOFOR"]);
    let mut s = Santral::ac(&k.rez, &k.token, &[]);
    // Tokensiz plan yok (yürütme token'ın izinleriyle sınırlıdır)
    let mut tokensiz = Santral::ac(&k.rez, "", &[]);
    let (t, hata) = tokensiz.arac("plan", iki_adimli_plan());
    assert!(hata && t.contains("token"), "{t}");
    // Kural çiğneyen gövde (kapasite: en çok 8 yolcu) plana alınmaz
    let (t, hata) = s.arac("plan", json!({ "baslik": "x", "adimlar": [{ "uygulama": "transfer", "kapi": "POST /api/transferler", "govde": { "rezervasyonRef": "R", "yolcuId": "y", "aracId": "a", "yolcuSayisi": 20 } }] }));
    assert!(hata && t.contains("yolcuSayisi"), "{t}");
    // Token'ın açmadığı işlem plana alınmaz
    let t1 = k.token.split(',').next().unwrap().to_string();
    let mut s1 = Santral::ac(&k.rez, &t1, &[]);
    let (t, hata) = s1.arac("plan", iki_adimli_plan());
    assert!(hata, "transfer için token yok: {t}");
    // Ajanın rol beyanı ve oturum argümanı yok sayılır: insanın rolü uyuşmuyorsa onayla da gitmez
    let (t, hata) = s.arac(
        "plan",
        json!({ "baslik": "rol", "adimlar": [{ "uygulama": "transfer", "kapi": "PATCH /api/transferler/t1", "govde": { "yolcuSayisi": 4 }, "roller": ["OPERASYON"], "oturum": true }] }),
    );
    // Plan kurulabilir ya da önizlemede reddedilir; kurulduysa onay da göndermez
    if !hata {
        let id = plan_kimligi(&t);
        let g = k.tr.ok(&["plan", "goster", &id]);
        let (ok, m) = k.tr.cikti(&["plan", "onayla", &id, "--adim", "1", "--ozet", &adim_ozeti(&g, 1)]);
        assert!(!ok && (m.contains("rol") || m.contains("REDDED") || m.contains("403")), "{m}");
    } else {
        assert!(t.contains("OPERASYON") || t.contains("rol") || t.contains("403"), "{t}");
    }
    assert!(yazmalar(&k.tr_app).is_empty(), "rol kapısı aşılmamalı");
    // Bayatlık: plan kurulduktan sonra kaynak değişirse adım gönderilmez
    let (t, hata) = s.arac("plan", iki_adimli_plan());
    assert!(!hata, "{t}");
    let id = plan_kimligi(&t);
    let g = k.rez.ok(&["plan", "goster", &id]);
    let (o1, o2) = (adim_ozeti(&g, 1), adim_ozeti(&g, 2));
    let dosya = k.rez.klasor.join("src").join("rezervasyon").join("rezervasyon.controller.ts");
    let eski = std::fs::read_to_string(&dosya).unwrap();
    std::fs::write(&dosya, format!("{eski}\n// değişti\n")).unwrap();
    let (ok, m) = k.rez.cikti(&["plan", "onayla", &id, "--adim", "1", "--ozet", &o1]);
    assert!(!ok && m.contains("bayat"), "{m}");
    std::fs::write(&dosya, eski).unwrap();
    // Plan dosyası değiştirilirse (yol) içerik özeti tutmaz: gönderilmez
    let planlar = bul_klasor(&k.ev.kok, "santral-planlar").expect("plan klasörü yok");
    let pd = planlar.join(format!("{id}.json"));
    let mut pv: Value = serde_json::from_str(&std::fs::read_to_string(&pd).unwrap()).unwrap();
    pv["adimlar"][0]["yol"] = json!("/rezervasyonlar/baska");
    std::fs::write(&pd, pv.to_string()).unwrap();
    let (ok, m) = k.rez.cikti(&["plan", "onayla", &id, "--adim", "1", "--ozet", &o1]);
    assert!(!ok, "değiştirilmiş plan gönderilmemeli: {m}");
    // Gözlem modunda gönderilmez; token iptal edilince gönderilmez
    let (t, _) = s.arac("plan", iki_adimli_plan());
    let id = plan_kimligi(&t);
    let g = k.rez.ok(&["plan", "goster", &id]);
    let o1 = adim_ozeti(&g, 1);
    k.rez.ok(&["mod", "gozlem"]);
    assert!(!k.rez.cikti(&["plan", "onayla", &id, "--adim", "1", "--ozet", &o1]).0);
    k.rez.ok(&["mod", "yazma"]);
    k.rez.ok(&["token", "iptal", "ajan"]);
    let (ok, m) = k.rez.cikti(&["plan", "onayla", &id, "--adim", "1", "--ozet", &o1]);
    assert!(!ok && m.contains("token"), "{m}");
    let _ = o2;
    assert!(yazmalar(&k.rez_app).is_empty() && yazmalar(&k.tr_app).is_empty(), "hiçbir atlatma yazmamalı");
}

fn bul_klasor(d: &std::path::Path, ad: &str) -> Option<std::path::PathBuf> {
    for g in std::fs::read_dir(d).ok()?.flatten() {
        let p = g.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|a| a == ad) {
                return Some(p);
            }
            if let Some(x) = bul_klasor(&p, ad) {
                return Some(x);
            }
        }
    }
    None
}

#[test]
fn yardim_yeni_komutlari_tanir() {
    let ev = Ev::yeni("w3e-yardim");
    let p = ev.proje("w3e_rezervasyon", "rez");
    let y = p.ok(&["plan", "--help"]);
    assert!(y.contains("plan goster") && y.contains("plan onayla"), "{y}");
    let y = p.ok(&["santral", "--help"]);
    assert!(y.contains("bagla-varlik") && y.contains("varliklar"), "{y}");
    let (ok, m) = p.cikti(&["plan", "uydur"]);
    assert!(!ok, "{m}");
    let (ok, m) = p.cikti(&["plan", "onayla", "x", "--bilinmeyen"]);
    assert!(!ok, "{m}");
}
