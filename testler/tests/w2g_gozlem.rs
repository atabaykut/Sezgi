//! W2-gözlem (P2.3 + P2.4): HAR içe aktarma, canlı mod gözlemi ve kullanıcı yolculukları.
//! - `sezgi gozlem al <har>`: yalnızca biçim (yöntem, yol şablonu, durum, alan adları ve tipleri); değer yok. Kanıt
//!   deposunda JWT, e-posta, parola ve gövde değeri bayt araması 0.
//! - Eşleşen istek → NETWORK_CAPTURE (observed); `kanit` aracı createItem için SOURCE_CODE + NETWORK_CAPTURE gösterir.
//! - Başarılı isteğin gövdesi koddaki şemayla çelişirse önerme çelişkili (en fazla inferred).
//! - Haritada olmayan gözlenen uç `bilinmeyenler`de: "kodda bulunamayan gözlenen uç".
//! - Zaman işareti → UI eylemi yalnızca inferred.
//! - Canlı mod `cagir` isteği gözlem kanıtı olur; insan ve token bilgisi kanıta girmez.
//! - Yolculuk: "/login › Log In → /items › Save → POST /api/v1/items/"; statik adımlar inferred, gözlenen istek sırası
//!   yolculuğu observed yapar; kartta `x-journeys`; `tani` ve `etki` işlemin geçtiği yolculukları gösterir.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
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

/// fastapi-template kopyası (backend + frontend); uygulama kökü backend, ön yüz yanında
fn sablon(ev: &Ev, ad: &str) -> Proje {
    let kok = ev.kok.join(ad);
    kopyala(&ornek("w2g_fastapi_template"), &kok);
    let p = Proje { klasor: kok.join("backend"), ev: ev.kok.join("ev") };
    p.ok(&["init", "."]);
    p
}

fn oku(p: &Proje, ad: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join(ad)).unwrap_or_else(|e| panic!("{ad}: {e}"))).unwrap()
}

fn onerme<'a>(k: &'a Value, varlik: &str, yuklem: &str, hedef: Option<&str>) -> Option<&'a Value> {
    k["onermeler"].as_array().unwrap().iter().find(|o| o["varlik"] == varlik && o["yuklem"] == yuklem && o["hedef"].as_str() == hedef)
}

fn kanit_turu(k: &Value, id: &str) -> String {
    k["kanitlar"].as_array().unwrap().iter().find(|x| x["id"] == id).map(|x| x["kaynak_turu"].as_str().unwrap().to_string()).unwrap_or_default()
}

fn destek_turleri(k: &Value, o: &Value) -> Vec<String> {
    o["hesap"]["destek"].as_array().unwrap().iter().map(|i| kanit_turu(k, i.as_str().unwrap())).collect()
}

/// .sezgi altındaki bütün dosyaların baytları
fn sezgi_baytlari(p: &Proje) -> Vec<(String, Vec<u8>)> {
    std::fs::read_dir(p.klasor.join(".sezgi"))
        .unwrap()
        .flatten()
        .filter(|g| g.path().is_file())
        .map(|g| (g.file_name().to_string_lossy().to_string(), std::fs::read(g.path()).unwrap()))
        .collect()
}

fn icerir(b: &[u8], aranan: &str) -> bool {
    b.windows(aranan.len()).any(|w| w == aranan.as_bytes())
}

/// Değer sızıntısı: JWT, e-posta (açık ve URL kodlu), parola, gövde değeri ve oluşturulan kaydın kimliği
fn sizinti_yok(p: &Proje, ek: &[&str]) {
    let mut aranan = vec!["eyJhbGci", "admin@example.com", "admin%40example.com", "@example.com", "Bearer ", "HAR kaydi", "access_token\":\"ey"];
    aranan.extend_from_slice(ek);
    for (ad, b) in sezgi_baytlari(p) {
        for a in &aranan {
            assert!(!icerir(&b, a), "{ad} değer sızdırıyor: {a}");
        }
        // uuid biçimli değer (oluşturulan kaydın ya da sahibinin kimliği)
        let m = String::from_utf8_lossy(&b);
        let uuid = m
            .as_bytes()
            .windows(36)
            .any(|w| w.iter().enumerate().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { *c == b'-' } else { c.is_ascii_hexdigit() && !c.is_ascii_uppercase() }));
        assert!(!uuid, "{ad}: uuid biçimli değer var");
    }
}

fn har_al(p: &Proje, har: &str, isaret: Option<&str>) -> String {
    let h = ornek("w2g_har").join(har);
    let mut a = vec!["gozlem".to_string(), "al".into(), h.to_string_lossy().to_string()];
    if let Some(i) = isaret {
        a.extend(["--isaretler".into(), ornek("w2g_har").join(i).to_string_lossy().to_string()]);
    }
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    p.ok(&a)
}

const CREATE: &str = "api:POST /api/v1/items/";
const LOGIN: &str = "api:POST /api/v1/login/access-token";

/// Test 1'in akışı (giriş → /items → Add Item → Save → sil): iki kayıt (Ajan 2'nin özgün kaydı ve konteynerde
/// yeniden üretilen) aynı sonucu vermeli
fn har_kabul(har: &str, isaretler: &str, parola: &str) {
    let ev = Ev::yeni(&format!("w2g-har-{}", &har[..6]));
    let p = sablon(&ev, "t");
    let m = har_al(&p, har, Some(isaretler));
    assert!(m.contains("Gözlem alındı") && m.contains("değer"), "{m}");
    let k = oku(&p, "kanit.json");
    // Uç çalışıyor: observed, ağ yakalamasıyla
    let calisir = onerme(&k, CREATE, "CALISIR", None).unwrap_or_else(|| panic!("CALISIR yok"));
    assert_eq!(calisir["hesap"]["guven"], "observed", "{calisir}");
    assert_eq!(destek_turleri(&k, calisir), vec!["NETWORK_CAPTURE"]);
    assert_eq!(onerme(&k, LOGIN, "CALISIR", None).unwrap()["hesap"]["guven"], "observed");
    // createItem: kodun kanıtı certain, gözlem gövdeyi destekliyor (çelişki yok)
    let govde = onerme(&k, CREATE, "GOVDE", Some("ItemCreate")).unwrap();
    let t = destek_turleri(&k, govde);
    assert!(t.contains(&"SOURCE_CODE".to_string()) && t.contains(&"NETWORK_CAPTURE".to_string()), "{t:?}");
    assert_eq!(govde["hesap"]["guven"], "certain");
    assert!(govde["hesap"]["celiski"].as_array().unwrap().is_empty(), "{govde}");
    // Yanıt biçimi: alan adları, observed, değer yok
    let yanit = onerme(&k, CREATE, "YANIT", Some("200")).unwrap();
    let alanlar: Vec<&str> = yanit["deger"]["alanlar"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
    assert!(alanlar.contains(&"id") && alanlar.contains(&"title") && alanlar.contains(&"owner_id"), "{alanlar:?}");
    assert_eq!(yanit["hesap"]["guven"], "observed");
    // Silme isteğinin yolu şablonla eşlendi (uuid değeri değil)
    assert!(onerme(&k, "api:DELETE /api/v1/items/:id", "CALISIR", None).is_some());
    // Zaman işareti → UI eylemi: yalnızca inferred
    let tetik: Vec<&Value> =
        k["onermeler"].as_array().unwrap().iter().filter(|o| o["yuklem"] == "TRIGGERS" && o["hedef"] == CREATE && o["varlik"].as_str().unwrap().contains("Save")).collect();
    assert!(!tetik.is_empty(), "save işareti Save eylemine eşlenmedi");
    for o in &tetik {
        assert_eq!(o["hesap"]["guven"], "inferred", "{o}");
    }
    // Gizlilik: hiçbir dosyada değer yok
    sizinti_yok(&p, &[parola]);
    let g = std::fs::read_to_string(p.klasor.join(".sezgi").join("gozlem.json")).unwrap();
    assert!(g.contains("\"password\"") && g.contains("\"username\""), "form alan adları yazılmalı: {g}");
    // `kanit` aracı createItem için iki kaynak türünü gösterir
    let mut mcp = p.mcp("");
    let (s, hata) = mcp.arac("kanit", json!({ "kapi": "POST /api/v1/items/" }));
    assert!(!hata && s.contains("SOURCE_CODE") && s.contains("NETWORK_CAPTURE"), "{s}");
    // tani: gözlem ve yolculuk
    let (s, hata) = mcp.arac("tani", json!({ "kapi": "POST /api/v1/items/" }));
    assert!(!hata && s.contains("gözlem: çalışıyor [Gözlem]"), "{s}");
    // Giriş ile işlemin ekranı arasında ekran geçişleri (ara ekranlar) olabilir
    assert!(s.contains("yolculuklar") && s.contains("/login › Log In →") && s.contains("/items › Save → POST /api/v1/items/ [Gözlem]"), "{s}");
}

#[test]
fn w2g_test1_har_yeniden_uretilen() {
    har_kabul("oturum.har", "har_isaretleri.json", "w2g-Parola-482");
}

#[test]
fn w2g_test1_har_ajan2_ozgun() {
    har_kabul("test1_ajan2.har", "test1_ajan2_isaretler.json", "t1-Parola-123");
}

/// Elle yazılmış küçük HAR: istekler (yöntem, yol, istek gövdesi, durum, yanıt)
fn har_yaz(p: &Path, istekler: &[(&str, &str, Option<Value>, u16, Value)]) {
    let girdiler: Vec<Value> = istekler
        .iter()
        .enumerate()
        .map(|(i, (y, yol, govde, durum, yanit))| {
            let mut istek = json!({ "method": y, "url": format!("http://localhost:8000{yol}"), "headers": [{ "name": "Authorization", "value": "Bearer eyJhbGciOiJIUzI1NiJ9.e30.gizli" }, { "name": "Cookie", "value": "oturum=cerez-degeri" }] });
            if let Some(g) = govde {
                istek["postData"] = json!({ "mimeType": "application/json", "text": g.to_string() });
            }
            json!({ "startedDateTime": format!("2026-10-21T10:00:0{i}.000Z"), "request": istek,
                    "response": { "status": durum, "headers": [{ "name": "Set-Cookie", "value": "oturum=cerez-degeri" }], "content": { "mimeType": "application/json", "text": yanit.to_string() } } })
        })
        .collect();
    std::fs::write(p, json!({ "log": { "version": "1.2", "creator": { "name": "test" }, "entries": girdiler } }).to_string()).unwrap();
}

#[test]
fn w2g_govde_celiskisi_ve_bilinmeyen_uc() {
    let ev = Ev::yeni("w2g-celiski");
    let p = sablon(&ev, "t");
    let har = ev.kok.join("elle.har");
    har_yaz(
        &har,
        &[
            // Zorunlu `title` gönderilmeden kabul edildi: koddaki şemayla çelişki
            ("POST", "/api/v1/items/", Some(json!({ "description": "gizli-aciklama-degeri" })), 200, json!({ "id": 7, "title": "x" })),
            // Haritada olmayan uç
            ("GET", "/api/v1/raporlar/aylik/2026", None, 200, json!({ "toplam": 3, "kisi@ornek.com": 1 })),
            // Hata durumu
            ("DELETE", "/api/v1/items/42", None, 404, json!({ "detail": "Item not found" })),
        ],
    );
    p.ok(&["gozlem", "al", har.to_str().unwrap()]);
    let k = oku(&p, "kanit.json");
    let govde = onerme(&k, CREATE, "GOVDE", Some("ItemCreate")).unwrap();
    assert!(!govde["hesap"]["celiski"].as_array().unwrap().is_empty(), "çelişki bekleniyordu: {govde}");
    assert_eq!(govde["hesap"]["guven"], "inferred", "çelişkide en fazla inferred");
    // Hata durumu observed
    assert_eq!(onerme(&k, "api:DELETE /api/v1/items/:id", "HATA_DURUMU", Some("404")).unwrap()["hesap"]["guven"], "observed");
    // Bilinmeyen uç: şablon (yıl değeri :param), değer görünümlü anahtar yazılmaz
    let bilinmeyen = onerme(&k, "api:GET /api/v1/raporlar/aylik/:param", "VAR", None).unwrap();
    assert_eq!(bilinmeyen["hesap"]["guven"], "unknown");
    let mut mcp = p.mcp("");
    let (s, _) = mcp.arac("bilinmeyenler", json!({ "adet": 200 }));
    assert!(s.contains("kodda bulunamayan gözlenen uç") && s.contains("GET /api/v1/raporlar/aylik/:param"), "{s}");
    let (s, _) = mcp.arac("kanit", json!({ "kapi": "POST /api/v1/items/" }));
    assert!(s.contains("çelişki") && s.contains("zorunlu `title` gönderilmeden kabul edildi"), "{s}");
    sizinti_yok(&p, &["gizli-aciklama-degeri", "cerez-degeri", "kisi@ornek.com", "e30.gizli"]);
}

#[test]
fn w2g_ayni_har_iki_kez_alininca_tekrar_sayilmaz() {
    let ev = Ev::yeni("w2g-tekrar");
    let p = sablon(&ev, "t");
    let har = ev.kok.join("elle.har");
    har_yaz(&har, &[("GET", "/api/v1/items/?skip=0&limit=100", None, 200, json!({ "data": [], "count": 0 }))]);
    p.ok(&["gozlem", "al", har.to_str().unwrap()]);
    p.ok(&["gozlem", "al", har.to_str().unwrap()]);
    let g = oku(&p, "gozlem.json");
    assert_eq!(g["kaynaklar"].as_array().unwrap().len(), 1);
    assert_eq!(g["kayitlar"].as_array().unwrap().len(), 1);
    assert_eq!(g["kayitlar"][0]["adet"], 1);
    assert_eq!(g["kayitlar"][0]["sorgu_alanlari"], json!(["limit", "skip"]));
    // init gözlemleri korur: kanıt deposu yeniden kurulduğunda gözlem kanıtı yerinde
    p.ok(&["init", "."]);
    let k = oku(&p, "kanit.json");
    assert_eq!(onerme(&k, "api:GET /api/v1/items/", "CALISIR", None).unwrap()["hesap"]["guven"], "observed");
}

#[test]
fn w2g_yolculuklar_kartta_ve_etkide() {
    let ev = Ev::yeni("w2g-yolculuk");
    let p = sablon(&ev, "t");
    let k = oku(&p, "kanit.json");
    // Statik yolculuk: giriş adımıyla, en fazla inferred
    let yol: Vec<&Value> = k["onermeler"].as_array().unwrap().iter().filter(|o| o["yuklem"] == "VAR" && o["varlik"].as_str().unwrap().starts_with("journey:")).collect();
    let save = yol
        .iter()
        .find(|o| o["deger"]["ad"].as_str().is_some_and(|a| a.starts_with("/login › Log In →") && a.ends_with("/items › Save → POST /api/v1/items/")))
        .unwrap_or_else(|| panic!("yolculuk yok: {:?}", yol.iter().map(|o| o["deger"]["ad"].clone()).collect::<Vec<_>>()));
    assert_eq!(save["hesap"]["guven"], "inferred");
    assert_eq!(save["deger"]["giris"], LOGIN);
    let adimlar: Vec<&Value> = k["onermeler"].as_array().unwrap().iter().filter(|o| o["yuklem"] == "STEP" && o["varlik"] == save["varlik"]).collect();
    assert!(adimlar.len() >= 4, "{adimlar:?}");
    assert!(adimlar.iter().all(|o| o["hesap"]["guven"] == "inferred"));
    // Kartta x-journeys (şemadaki biçimde)
    let kart = p.kart();
    let kj = kart["x-journeys"].as_array().unwrap_or_else(|| panic!("x-journeys yok"));
    let j = kj.iter().find(|j| j["id"] == save["varlik"]).unwrap();
    assert_eq!(j["stamp"], "inferred");
    // Giriş ekranı ve eylemi, (varsa) ara ekranlar, işlemin ekranı ve eylemi
    let turler: Vec<&str> = j["steps"].as_array().unwrap().iter().map(|s| s["kind"].as_str().unwrap()).collect();
    assert_eq!((&turler[..2], &turler[turler.len() - 2..]), (&["screen", "ui"][..], &["screen", "ui"][..]), "{turler:?}");
    assert!(turler[2..turler.len() - 2].iter().all(|t| *t == "screen"), "{turler:?}");
    assert!(kart["operations"].as_array().unwrap().iter().any(|o| o["id"] == j["operation"]));
    // Gözlem: giriş ucu, sonra işlem → yolculuk observed; UI adımları inferred kalır. Kart bir sonraki init'te
    let har = ev.kok.join("sira.har");
    har_yaz(
        &har,
        &[
            ("POST", "/api/v1/login/access-token", None, 200, json!({ "access_token": "x", "token_type": "bearer" })),
            ("POST", "/api/v1/items/", Some(json!({ "title": "t" })), 200, json!({ "id": 1 })),
        ],
    );
    p.ok(&["gozlem", "al", har.to_str().unwrap()]);
    p.ok(&["init", "."]);
    let kart = p.kart();
    let j = kart["x-journeys"].as_array().unwrap().iter().find(|j| j["id"] == save["varlik"]).unwrap().clone();
    assert_eq!(j["stamp"], "observed", "{j}");
    assert!(j["steps"].as_array().unwrap().iter().all(|s| s["stamp"] == "inferred"), "{j}");
    // MCP: etki ve tani işlemin geçtiği yolculukları gösterir (giriş ucu da)
    let mut mcp = p.mcp("");
    let (s, _) = mcp.arac("etki", json!({ "kapi": "POST /api/v1/items/" }));
    assert!(s.contains("Yolculuklar") && s.contains("/items › Save → POST /api/v1/items/ [Gözlem]"), "{s}");
    let (s, _) = mcp.arac("tani", json!({ "kapi": "POST /api/v1/login/access-token" }));
    assert!(s.contains("yolculuklar") && s.contains("/login › Log In →"), "{s}");
}

#[test]
fn w2g_canli_cagri_gozlem_kaniti_olur() {
    let ev = Ev::yeni("w2g-canli");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE-GIZLI");
    let t = p.token("ajan-gizli-ad", &["yorum.*:yaz"], &[]);
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(yorum_yaz());
    assert!(!hata && s.contains("YAZILDI"), "{s}");
    let k = oku(&p, "kanit.json");
    let o = k["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["varlik"].as_str().unwrap().starts_with("api:POST /api/v1/yorumlar") && o["yuklem"] == "CALISIR")
        .unwrap_or_else(|| panic!("canlı çağrı gözlem kanıtı olmadı"));
    assert_eq!(o["hesap"]["guven"], "observed");
    let kid = o["hesap"]["destek"][0].as_str().unwrap();
    let kn = k["kanitlar"].as_array().unwrap().iter().find(|x| x["id"] == kid).unwrap();
    assert_eq!(kn["kaynak_turu"], "NETWORK_CAPTURE");
    assert_eq!(kn["konum"], json!(["canli"]));
    // Yanıt alan adları (sahte uygulama: sira, kim), değerler yok; insan ve token bilgisi yok
    let y = k["onermeler"].as_array().unwrap().iter().find(|x| x["varlik"] == o["varlik"] && x["yuklem"] == "YANIT").unwrap();
    assert_eq!(y["deger"]["alanlar"], json!(["kim", "sira"]));
    sizinti_yok(&p, &["OTURUM-ALICE-GIZLI", "ajan-gizli-ad", "szg_", "güzel", "8a3c1e2b"]);
    // Aynı oturumdaki MCP sunucusu yeni kanıtı görür (kanit.json değişince model yenilenir)
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let (s, _) = m.arac("kanit", json!({ "kapi": "POST /api/v1/yorumlar" }));
    assert!(s.contains("NETWORK_CAPTURE"), "{s}");
}

#[test]
fn w2g_bozuk_har_acik_hata() {
    let ev = Ev::yeni("w2g-bozuk");
    let p = sablon(&ev, "t");
    let har = ev.kok.join("bozuk.har");
    std::fs::write(&har, "{\"log\": {\"entries\": [ {\"request\": ").unwrap();
    let (b, m) = p.cikti(&["gozlem", "al", har.to_str().unwrap()]);
    assert!(!b && m.contains("HAR okunamadı"), "{m}");
    assert!(!p.klasor.join(".sezgi").join("gozlem.json").exists());
    let (b, m) = p.cikti(&["gozlem", "al"]);
    assert!(!b && m.contains("Kullanım"), "{m}");
}
