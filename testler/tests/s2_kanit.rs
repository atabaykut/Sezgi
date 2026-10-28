//! S2 (P1.1 + P1.3) kanıt ve graf. `init` sonunda `.sezgi/kanit.json` ve `.sezgi/graf.json` yazılır: ikisi aynı
//! revizyonu taşır, graf kanıt dosyasının SHA-256 özetini taşır (çift tutarlılığı). Kenar = önerme; düğüm ve ilişki adları
//! 03 §2–§3 sözlüğünden. Dayanak dosyası değişince kanıt eski olur, yalnızca ona dayanan önerme bilinmiyor'a düşer.
//! MCP'de salt okunur `kanit`, `bilinmeyenler`, `etki`; `tani` güven özeti taşır; `rapor`, `tani` ve Ayna aynı tablo
//! bağını söyler; Ayna harita yenilenince bayat kalmaz. Tavan ve çelişki kuralları elle yazılmış bir kanıt deposuyla
//! sınanır: okuma modeli güveni dosyadaki hesaptan değil kanıtlardan yeniden hesaplar (dosyaya yazılan güvene inanılmaz).
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Stdio};

fn oku(p: &Proje, ad: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi").join(ad)).unwrap_or_else(|e| panic!("{ad}: {e}"))).unwrap()
}

fn sha256(b: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, b).as_ref().iter().map(|x| format!("{x:02x}")).collect()
}

fn onerme<'a>(k: &'a Value, varlik: &str, yuklem: &str, hedef: Option<&str>) -> &'a Value {
    k["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["varlik"] == varlik && o["yuklem"] == yuklem && o["hedef"].as_str() == hedef)
        .unwrap_or_else(|| panic!("önerme yok: {varlik} {yuklem} {hedef:?}"))
}

fn kenarlar<'a>(g: &'a Value, kaynak: &str, iliski: &str) -> Vec<&'a Value> {
    g["kenarlar"].as_array().unwrap().iter().filter(|e| e["kaynak"] == kaynak && e["iliski"] == iliski).collect()
}

fn kenar<'a>(g: &'a Value, kaynak: &str, iliski: &str, hedef: &str) -> &'a Value {
    kenarlar(g, kaynak, iliski).into_iter().find(|e| e["hedef"] == hedef).unwrap_or_else(|| panic!("kenar yok: {kaynak} {iliski} {hedef}"))
}

fn kanit<'a>(k: &'a Value, id: &str) -> &'a Value {
    k["kanitlar"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap_or_else(|| panic!("kanıt yok: {id}"))
}

const POST: &str = "api:POST /api/v1/kitaplar@2026-01-01";
const SIL: &str = "api:DELETE /api/v1/kitaplar/:id@2026-01-01";
const LISTE: &str = "api:GET /api/v1/kitaplar@2026-01-01";

#[test]
fn init_kanit_ve_graf_yazar() {
    let ev = Ev::yeni("s2-yaz");
    let p = ev.proje("nestjs-temel", "n");
    let ham = std::fs::read(p.klasor.join(".sezgi").join("kanit.json")).unwrap();
    let (k, g) = (oku(&p, "kanit.json"), oku(&p, "graf.json"));
    assert_eq!(k["bicim"], 1);
    assert_eq!(k["revizyon"], 1, "ilk kurulum revizyon 1");
    assert_eq!(g["revizyon"], k["revizyon"], "graf ve kanıt aynı revizyonda");
    assert_eq!(g["kanit_ozeti"].as_str().unwrap(), sha256(&ham), "graf kanıt dosyasının özetini taşır (çift tutarlılığı)");
    // Kanıtlar: tür, konum ve dayanak özeti
    let turler = [
        "SOURCE_CODE",
        "AST",
        "API_SCHEMA",
        "DATABASE_SCHEMA",
        "BINARY_METADATA",
        "BINARY_HEURISTIC",
        "DECOMPILED",
        "BROWSER_OBSERVATION",
        "NETWORK_CAPTURE",
        "RUNTIME_TRACE",
        "AUTOMATED_TEST",
        "USER_CONFIRMED",
    ];
    for x in k["kanitlar"].as_array().unwrap() {
        assert!(turler.contains(&x["kaynak_turu"].as_str().unwrap()), "{x}");
        assert!(x["id"].as_str().unwrap().starts_with("kn_"), "{x}");
        assert!(x["zaman"].as_str().is_some_and(|z| z.len() >= 10), "{x}");
        for d in x["dayanak"].as_array().unwrap() {
            let o = d["sha256"].as_str().unwrap();
            assert_eq!(o, sha256(&std::fs::read(p.klasor.join(d["yol"].as_str().unwrap())).unwrap()), "{x}");
        }
        assert!(x["konum"].as_array().unwrap().iter().all(|k| k.as_str().unwrap().rsplit_once(':').is_some_and(|(_, s)| s.parse::<usize>().is_ok())), "konum dosya:satır: {x}");
    }
    // Her önermenin destek ve çelişki kanıtları depoda; bağ yönü önermeyle tutarlı
    for o in k["onermeler"].as_array().unwrap() {
        for id in o["hesap"]["destek"].as_array().unwrap().iter().chain(o["hesap"]["celiski"].as_array().unwrap()) {
            kanit(&k, id.as_str().unwrap());
        }
        assert!(["certain", "observed", "inferred", "unknown"].contains(&o["hesap"]["guven"].as_str().unwrap()), "{o}");
        assert!(["kapali", "acik", "bilinmiyor"].contains(&o["hesap"]["kapsam"].as_str().unwrap()), "{o}");
        assert_eq!(o["hesap"]["gecerlilik"], "gecerli", "{o}");
    }
    for b in k["baglar"].as_array().unwrap() {
        assert!(b["yon"] == "destek" || b["yon"] == "celiski", "{b}");
    }
    // Giriş kararı: işaretten (kesin) certain, yokluktan ya da doğrulanamayan işaretten inferred (§6.3)
    let giris = onerme(&k, POST, "GIRIS", None);
    assert_eq!((giris["deger"].as_str(), giris["hesap"]["guven"].as_str()), (Some("gerekli"), Some("certain")), "{giris}");
    let x = kanit(&k, giris["hesap"]["destek"][0].as_str().unwrap());
    assert_eq!(x["kaynak_turu"], "SOURCE_CODE");
    assert!(x["konum"][0].as_str().unwrap().starts_with("src/kitap/kitap.controller.ts:"), "{x}");
    assert_eq!(onerme(&k, LISTE, "GIRIS", None)["hesap"]["guven"], "inferred");
    // Düğümler ve kenarlar (03 sözlüğü): kenar bir önermedir
    let dugum = |id: &str| g["dugumler"].as_array().unwrap().iter().find(|d| d["id"] == id).unwrap_or_else(|| panic!("düğüm yok: {id}")).clone();
    assert_eq!(dugum(POST)["tur"], "api");
    assert_eq!(dugum("db:Kitap")["tur"], "db");
    assert_eq!(dugum("role:ADMIN")["tur"], "role");
    let e = kenar(&g, SIL, "REQUIRES_PERMISSION", "role:ADMIN");
    assert_eq!(e["guven"], "certain", "{e}");
    assert_eq!(e["onerme"], onerme(&k, SIL, "REQUIRES_PERMISSION", Some("role:ADMIN"))["id"]);
    // Statik veri çıkarımı en fazla inferred (03 §3.1)
    assert_eq!(kenar(&g, POST, "WRITES", "db:Kitap")["guven"], "inferred");
    assert_eq!(kenar(&g, LISTE, "READS", "db:Kitap")["guven"], "inferred");
    // Teknik uç ile iş yeteneği ayrı: IMPLEMENTS addan, inferred
    let imp = kenarlar(&g, POST, "IMPLEMENTS");
    assert!(imp.len() == 1 && imp[0]["hedef"].as_str().unwrap().starts_with("cap:") && imp[0]["guven"] == "inferred", "{imp:?}");
    assert_eq!(dugum(imp[0]["hedef"].as_str().unwrap())["tur"], "cap");
    let cag = kenarlar(&g, POST, "CALLS");
    assert!(cag.len() == 1 && cag[0]["hedef"].as_str().unwrap().starts_with("fn:src/kitap/kitap.controller.ts#") && cag[0]["guven"] == "certain", "{cag:?}");
    // Tablonun varlığı şema dosyasından: DATABASE_SCHEMA, certain, dayanağı schema.prisma
    let tablo = onerme(&k, "db:Kitap", "VAR", None);
    assert_eq!(tablo["hesap"]["guven"], "certain", "{tablo}");
    assert!(
        tablo["hesap"]["destek"].as_array().unwrap().iter().any(|id| {
            let x = kanit(&k, id.as_str().unwrap());
            x["kaynak_turu"] == "DATABASE_SCHEMA" && x["dayanak"].as_array().unwrap().iter().any(|d| d["yol"] == "prisma/schema.prisma")
        }),
        "{tablo}"
    );
    // Yeniden kurulum: revizyon artar, değişmeyen önermenin revizyonu korunur
    p.ok(&["init", "."]);
    let (k2, g2) = (oku(&p, "kanit.json"), oku(&p, "graf.json"));
    assert_eq!((k2["revizyon"].as_u64(), g2["revizyon"].as_u64()), (Some(2), Some(2)));
    assert_eq!(onerme(&k2, POST, "GIRIS", None)["hesap"]["revizyon"], 1, "değişmeyen önerme yeniden yazılmış sayılmaz");
}

#[test]
fn kaynaksiz_tablo_ve_bilinmeyen_veri() {
    let ev = Ev::yeni("s2-express");
    let p = ev.proje("express-temel", "e");
    let (k, g) = (oku(&p, "kanit.json"), oku(&p, "graf.json"));
    // Şema dosyası yok: tablo sorgudan çıkarıldı, en fazla inferred
    let t = onerme(&k, "db:kitaplar", "VAR", None);
    assert_eq!(t["hesap"]["guven"], "inferred", "{t}");
    assert!(t["hesap"]["destek"].as_array().unwrap().iter().all(|id| kanit(&k, id.as_str().unwrap())["kaynak_turu"] != "DATABASE_SCHEMA"));
    assert_eq!(kenar(&g, "api:GET /api/kitaplar/:id", "READS", "db:yazarlar")["guven"], "inferred");
    // Veri bağı hiç çıkarılamayan işlem: önerme bilinmiyor, kapsamı bilinmiyor (yokluk iddia edilmez)
    let v = onerme(&k, "api:PUT /api/kitaplar/:id/kapak", "VERI", None);
    assert_eq!((v["hesap"]["guven"].as_str(), v["hesap"]["kapsam"].as_str()), (Some("unknown"), Some("bilinmiyor")), "{v}");
    assert!(v["hesap"]["destek"].as_array().unwrap().is_empty());
    // Listesi eksik olabilen veri bağı: kapsam açık ve nedeni yazılı
    let v = onerme(&k, "api:GET /api/kitaplar", "VERI", None);
    assert_eq!(v["hesap"]["kapsam"], "acik", "{v}");
    assert!(!v["hesap"]["kapsam_disi"].as_array().unwrap().is_empty(), "{v}");
    // Rol kararı varsayımsa (Express) inferred ve kapsam açık
    let r = onerme(&k, "api:DELETE /api/kitaplar/:id", "ROLLER", None);
    assert_eq!((r["hesap"]["guven"].as_str(), r["hesap"]["kapsam"].as_str()), (Some("inferred"), Some("acik")), "{r}");
    assert_eq!(kenar(&g, "api:DELETE /api/kitaplar/:id", "REQUIRES_PERMISSION", "role:admin")["guven"], "inferred");
}

#[test]
fn rapor_ve_tani_ayni_tablo_bagini_soyler() {
    let ev = Ev::yeni("s2-rapor");
    let p = ev.proje("express-temel", "e");
    let harita = oku(&p, "kapilar.json");
    let bagli: Vec<String> = harita["endpointler"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|k| ["okur", "yazar"].iter().any(|a| k["veri"][a].as_array().is_some_and(|x| !x.is_empty())))
        .map(|k| k["id"].as_str().unwrap().to_string())
        .collect();
    assert!(bagli.len() >= 5, "{bagli:?}");
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    assert_eq!(r["veri"]["bagli_kapi"].as_u64(), Some(bagli.len() as u64), "rapor, işlemlerin tablo bağlarını saymalı: {}", r["veri"]);
    let bagsiz: Vec<&str> = r["veri"]["bagsiz_kapilar"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    let mut m = p.mcp("");
    for k in harita["endpointler"].as_array().unwrap() {
        let id = k["id"].as_str().unwrap();
        assert_eq!(bagsiz.contains(&id), !bagli.contains(&id.to_string()), "{id}");
        let (t, _) = m.arac("tani", json!({ "kapi": id }));
        for tablo in ["okur", "yazar"].iter().flat_map(|a| k["veri"][a].as_array().cloned().unwrap_or_default()) {
            assert!(t.contains(tablo.as_str().unwrap()), "tani {id} {tablo} bağını göstermeli: {t}");
        }
    }
    let metin = p.ok(&["rapor"]);
    assert!(!metin.contains("tablo bağları yok"), "rapor tani ile çelişmemeli: {metin}");
    // Güven sayıları kanıt deposundaki önermelerden
    let k = oku(&p, "kanit.json");
    let say = |g: &str| k["onermeler"].as_array().unwrap().iter().filter(|o| o["hesap"]["guven"] == g).count() as u64;
    assert_eq!(r["kanit"]["onerme"].as_u64(), Some(k["onermeler"].as_array().unwrap().len() as u64), "{}", r["kanit"]);
    for g in ["certain", "observed", "inferred", "unknown"] {
        assert_eq!(r["kanit"]["guven"][g].as_u64(), Some(say(g)), "{g}: {}", r["kanit"]);
    }
    assert_eq!(r["kanit"]["revizyon"], k["revizyon"]);
    assert!(metin.contains("KANIT"), "{metin}");
}

#[test]
fn dayanak_degisince_kanit_eski() {
    let ev = Ev::yeni("s2-eski");
    let p = ev.proje("nestjs-temel", "n");
    // Oturum boyunca da fark edilir: dosya değişince bir sonraki çağrıda (en geç ~1 sn sonra) eski
    let mut m = p.mcp("");
    let (t, hata) = m.arac("kanit", json!({ "kapi": "POST /api/v1/kitaplar" }));
    assert!(!hata && t.contains("[Kesin]") && !t.contains("eski"), "{t}");
    let dosya = p.klasor.join("src").join("kitap").join("kitap.controller.ts");
    let mut f = std::fs::OpenOptions::new().append(true).open(&dosya).unwrap();
    writeln!(f, "\n// değişti").unwrap();
    drop(f);
    std::thread::sleep(std::time::Duration::from_millis(1300));
    let (t, hata) = m.arac("kanit", json!({ "kapi": "POST /api/v1/kitaplar" }));
    assert!(!hata, "{t}");
    assert!(t.contains("eski") && t.contains("src/kitap/kitap.controller.ts"), "dayanak değişince eski ve nedeni: {t}");
    let giris = t.lines().find(|l| l.starts_with("giriş")).unwrap_or_else(|| panic!("{t}"));
    assert!(giris.contains("[Bilinmiyor]") && giris.contains("eski"), "{giris}");
    // Gövde şeması başka dosyada (dto): etkilenmez
    let govde = t.lines().find(|l| l.starts_with("gövde")).unwrap_or_else(|| panic!("{t}"));
    assert!(govde.contains("[Kesin]") && !govde.contains("eski"), "{govde}");
    // Yeni oturum, tani ve rapor aynı şeyi söyler
    let (t, _) = p.mcp("").arac("tani", json!({ "kapi": "POST /api/v1/kitaplar" }));
    assert!(t.contains("kanıt:") && t.contains("eski"), "{t}");
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    assert!(r["kanit"]["eski"].as_u64().unwrap() > 0, "{}", r["kanit"]);
    // Yeniden kurulum: geçerli, revizyon artmış
    p.ok(&["init", "."]);
    let (t, _) = p.mcp("").arac("kanit", json!({ "kapi": "POST /api/v1/kitaplar" }));
    assert!(!t.contains("eski"), "{t}");
    assert_eq!(oku(&p, "kanit.json")["revizyon"], 2);
}

fn adlar(l: &Value) -> Vec<String> {
    l.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
}

fn liste(p: &Proje, ek: &[&str]) -> Value {
    let mut a = vec!["mcp", ".sezgi"];
    a.extend_from_slice(ek);
    let mut c = p.komut(&a).env("SEZGI_TOKEN", "").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    writeln!(c.stdin.take().unwrap(), "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).unwrap();
    let o = c.wait_with_output().unwrap();
    serde_json::from_str::<Value>(String::from_utf8_lossy(&o.stdout).lines().next().unwrap()).unwrap()["result"]["tools"].clone()
}

/// İngilizce oturumda tek araç çağrısı
fn en_arac(p: &Proje, ad: &str, a: Value) -> (String, bool) {
    let mut c = p.komut(&["mcp", ".sezgi", "--lang", "en"]).env("SEZGI_TOKEN", "").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    writeln!(c.stdin.take().unwrap(), "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": a } })).unwrap();
    let o = c.wait_with_output().unwrap();
    let v: Value = serde_json::from_str(String::from_utf8_lossy(&o.stdout).lines().next().unwrap()).unwrap();
    (v["result"]["content"][0]["text"].as_str().unwrap().to_string(), v["result"]["isError"] == true)
}

#[test]
fn mcp_kanit_bilinmeyenler_etki_araclari() {
    let ev = Ev::yeni("s2-mcp");
    let p = ev.proje("nestjs-temel", "n");
    for (ek, yeni) in [(&[][..], ["kanit", "bilinmeyenler", "etki"]), (&["--lang", "en"][..], ["evidence", "unknowns", "impact"])] {
        let l = liste(&p, ek);
        let a = adlar(&l);
        assert_eq!(a.len(), 9, "{a:?}");
        for ad in yeni {
            let t = l.as_array().unwrap().iter().find(|t| t["name"] == ad).unwrap_or_else(|| panic!("{ad} yok: {a:?}"));
            assert_eq!(t["annotations"]["readOnlyHint"], true, "{t}");
            assert_eq!(t["annotations"]["destructiveHint"], false, "{t}");
            assert_eq!(t["annotations"]["idempotentHint"], true, "{t}");
            assert_eq!(t["annotations"]["openWorldHint"], false, "{t}");
        }
    }
    let mut m = p.mcp("");
    // Hatalar isError: hedef yok, bulunamadı, yanlış tür
    for (ad, a) in [
        ("kanit", json!({})),
        ("kanit", json!({ "kapi": "GET /yok/boyle" })),
        ("kanit", json!({ "kapi": 5 })),
        ("kanit", json!({ "varlik": "db:Yok" })),
        ("etki", json!({})),
        ("etki", json!({ "varlik": "db:Yok" })),
        ("etki", json!({ "kapi": "GET /yok/boyle" })),
        ("etki", json!({ "kapi": "DELETE /api/v1/kitaplar/:id", "derinlik": "iki" })),
        ("bilinmeyenler", json!({ "kapi": "GET /yok/boyle" })),
        ("bilinmeyenler", json!({ "adet": -1 })),
    ] {
        let (t, hata) = m.arac(ad, a.clone());
        assert!(hata, "{ad} {a}: isError olmalı: {t}");
    }
    assert!(m.arac("kanit", json!({})).0.contains("`kapi`"), "eksik hedef adıyla söylenir");
    // kanit: önerme başına güven, geçerlilik, kapsam ve kaynak türü
    let (t, hata) = m.arac("kanit", json!({ "kapi": "POST /api/v1/kitaplar" }));
    assert!(!hata, "{t}");
    for parca in ["giriş", "SOURCE_CODE", "src/kitap/kitap.controller.ts:", "geçerli", "WRITES db:Kitap", "[Yorum]", "IMPLEMENTS cap:"] {
        assert!(t.contains(parca), "{parca}: {t}");
    }
    // Varlık olarak tablo: ona yazan ve onu okuyan işlemler
    let (t, hata) = m.arac("kanit", json!({ "varlik": "db:Kitap" }));
    assert!(!hata && t.contains("DATABASE_SCHEMA") && t.contains("prisma/schema.prisma") && t.contains("POST /api/v1/kitaplar"), "{t}");
    // etki: kesin ve tahmini ayrı; rol, tablo ve aynı tabloyu okuyan işlem
    let (t, hata) = m.arac("etki", json!({ "kapi": "DELETE /api/v1/kitaplar/:id" }));
    assert!(!hata, "{t}");
    let (kesin, tahmini) = t.split_once("Tahmini").unwrap_or_else(|| panic!("kesin ve tahmini ayrı listelenmeli: {t}"));
    assert!(kesin.contains("ADMIN"), "rol kesin: {t}");
    for parca in ["Gunluk", "Kitap", "GET /api/v1/kitaplar"] {
        assert!(tahmini.contains(parca), "{parca}: {t}");
    }
    assert!(t.contains("ekran"), "ekran bağı bilinmiyorsa açıkça söylenir: {t}");
    let (t, hata) = m.arac("etki", json!({ "varlik": "db:Kitap" }));
    assert!(!hata && t.contains("POST /api/v1/kitaplar") && t.contains("PATCH /api/v1/kitaplar/:id") && t.contains("GET /api/v1/kitaplar"), "{t}");
    // Derinlik 3'ten fazla istenemez; açıkça söylenir
    let (t, hata) = m.arac("etki", json!({ "varlik": "db:Kitap", "derinlik": 9 }));
    assert!(!hata && t.contains("3"), "{t}");
    // bilinmeyenler: öncelikli liste; kapıya göre süzülebilir
    let (t, hata) = m.arac("bilinmeyenler", json!({}));
    assert!(!hata && t.lines().count() > 1, "{t}");
    let (t, hata) = m.arac("bilinmeyenler", json!({ "kapi": "POST /api/v1/kitaplar" }));
    assert!(!hata, "{t}");
    assert!(!t.contains("DELETE /api/v1/kitaplar") && !t.contains("GET /api/v1/kitaplar "), "yalnızca istenen kapı: {t}");
    // İngilizce
    let (t, hata) = en_arac(&p, "impact", json!({ "endpoint": "DELETE /api/v1/kitaplar/:id" }));
    assert!(!hata && t.contains("ADMIN") && t.contains("Inferred"), "{t}");
    let (t, hata) = en_arac(&p, "evidence", json!({ "endpoint": "POST /api/v1/kitaplar" }));
    assert!(!hata && t.contains("login") && t.contains("[Certain]") && t.contains("valid"), "{t}");
    let (t, hata) = en_arac(&p, "unknowns", json!({}));
    assert!(!hata && !t.is_empty(), "{t}");
}

#[test]
fn bilinmeyenler_once_bilinmeyeni_gosterir() {
    let ev = Ev::yeni("s2-bilinmeyen");
    let p = ev.proje("express-temel", "e");
    let (t, hata) = p.mcp("").arac("bilinmeyenler", json!({ "adet": 50 }));
    assert!(!hata, "{t}");
    assert!(t.contains("PUT /api/kitaplar/:id/kapak"), "veri bağı çıkarılamayan işlem listede: {t}");
    let satirlar: Vec<&str> = t.lines().filter(|l| l.trim_start().starts_with(|c: char| c.is_ascii_digit())).collect();
    assert!(satirlar.len() > 3, "{t}");
    // Öncelik: bilinmiyor olanlar, açık kapsamlılardan önce
    let ilk_acik = satirlar.iter().position(|l| l.contains("[Yorum]") || l.contains("[Kesin]")).unwrap_or(satirlar.len());
    let son_bilinmeyen = satirlar.iter().rposition(|l| l.contains("[Bilinmiyor]")).unwrap();
    assert!(son_bilinmeyen < ilk_acik, "bilinmeyenler önce gelmeli: {t}");
}

#[test]
fn etki_siniri_asilinca_acikca_soyler() {
    let ev = Ev::yeni("s2-sinir");
    let p = ev.proje("express-temel", "e");
    // 600 işlem aynı tabloya yazar: tablodan çıkan etki düğüm sınırını (500) aşar
    let mut govde = String::from("const express = require('express');\nconst app = express();\n");
    for i in 0..600 {
        govde.push_str(&format!("app.post('/r{i}', (req, res) => {{ db.prepare('INSERT INTO kalabalik (x) VALUES (?)').run(1); }});\n"));
    }
    govde.push_str("app.listen(3000);\n");
    std::fs::write(p.klasor.join("app.js"), govde).unwrap();
    p.ok(&["init", "."]);
    let (t, hata) = p.mcp("").arac("etki", json!({ "varlik": "db:kalabalik" }));
    assert!(!hata, "kısmi sonuç hata değildir: {t}");
    assert!(t.contains("sınır aşıldı") && t.contains("500"), "sınır açıkça söylenmeli: {}", &t[..t.len().min(600)]);
}

#[test]
fn tani_guven_ozeti() {
    let ev = Ev::yeni("s2-tani");
    let p = ev.proje("nestjs-temel", "n");
    let (t, _) = p.mcp("").arac("tani", json!({ "kapi": "POST /api/v1/kitaplar" }));
    let satir = t.lines().find(|l| l.starts_with("kanıt:")).unwrap_or_else(|| panic!("tani güven özeti taşımalı: {t}"));
    assert!(satir.contains("giriş [Kesin]") && satir.contains("veri [Yorum"), "{satir}");
    let (t, _) = en_arac(&p, "describe", json!({ "endpoint": "POST /api/v1/kitaplar" }));
    let satir = t.lines().find(|l| l.starts_with("evidence:")).unwrap_or_else(|| panic!("{t}"));
    assert!(satir.contains("login [Certain]"), "{satir}");
}

#[test]
fn token_gorunurlugu_kanit_ve_etkide_korunur() {
    let ev = Ev::yeni("s2-token");
    let p = ev.proje("nestjs-temel", "n");
    let token = p.token("okur", &["kitap.listele:oku"], &[]);
    let mut m = p.mcp(&token);
    let (t, hata) = m.arac("etki", json!({ "varlik": "db:Kitap" }));
    assert!(!hata && t.contains("GET /api/v1/kitaplar"), "{t}");
    assert!(!t.contains("POST /api/v1/kitaplar") && !t.contains("DELETE"), "gizli işlem etki listesinde görünmez: {t}");
    let (t, hata) = m.arac("kanit", json!({ "kapi": "POST /api/v1/kitaplar" }));
    assert!(hata, "gizli işlemin kanıtı verilmez: {t}");
    let (t, _) = m.arac("kanit", json!({ "varlik": "db:Kitap" }));
    assert!(!t.contains("POST /api/v1/kitaplar"), "{t}");
    let (t, _) = m.arac("bilinmeyenler", json!({ "adet": 100 }));
    assert!(!t.contains("POST /api/v1/kitaplar") && !t.contains("DELETE"), "{t}");
}

struct Ayna {
    p: Child,
    port: u16,
    anahtar: String,
}

impl Drop for Ayna {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
    }
}

#[allow(clippy::zombie_processes)]
fn ayna(p: &Proje) -> Ayna {
    let mut c = p.komut(&["ayna", ".sezgi", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0, "adres yazılmadı");
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let anahtar = k.split("#anahtar=").nth(1).map(|a| a.chars().take_while(|c| c.is_ascii_alphanumeric()).collect()).expect("adres oturum anahtarını taşımalı");
            return Ayna { p: c, port, anahtar };
        }
    }
}

/// Ayna okuma ucu, oturum anahtarıyla
fn al(a: &Ayna, yol: &str) -> Value {
    let port = a.port;
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "GET {yol} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Sezgi-Anahtar: {}\r\n\r\n", a.anahtar).unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    serde_json::from_str(c.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("")).unwrap_or(Value::Null)
}

#[test]
fn ayna_ayni_modelden_ve_tazelenir() {
    let ev = Ev::yeni("s2-ayna");
    let p = ev.proje("express-temel", "e");
    let a = ayna(&p);
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    let ar = al(&a, "/api/rapor");
    assert_eq!(ar["veri"], r["veri"], "Ayna ile rapor aynı veri bağını söyler");
    assert_eq!(ar["kanit"]["revizyon"], 1);
    // Kapı başına güven: Ayna'nın kapı ayrıntısı aynı modelden
    let kn = al(&a, "/api/kanit");
    let x = kn["GET /api/kitaplar/:id"].as_array().unwrap_or_else(|| panic!("{kn}"));
    assert!(x.iter().any(|o| o["yuklem"] == "READS" && o["hedef"] == "db:yazarlar" && o["guven"] == "inferred"), "{x:?}");
    // Harita yenilenince Ayna bayat kalmaz
    p.ok(&["init", "."]);
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(al(&a, "/api/rapor")["kanit"]["revizyon"], 2, "Ayna yeni haritayı görmeli");
}

/// Elle yazılmış kanıt: dayanaksız (her zaman geçerli) ya da verilen dosya ve özetle
fn el_kaniti(id: &str, tur: &str, beyan: Option<&str>, dayanak: Option<(&str, &str)>) -> Value {
    let mut k = json!({ "id": id, "kaynak_turu": tur, "uretici": "test", "dayanak": [], "konum": [format!("{id}.txt:1")], "zaman": "2026-01-01T00:00:00Z", "ozet": {} });
    if let Some(b) = beyan {
        k["beyan"] = json!(b);
    }
    if let Some((yol, ozet)) = dayanak {
        k["dayanak"] = json!([{ "yol": yol, "sha256": ozet }]);
    }
    k
}

/// (varlık, [(kanıt türü, beyan, değer)], beklenen etiket)
type Durum<'a> = (&'a str, Vec<(&'a str, Option<&'a str>, &'a str)>, &'a str);

#[test]
fn tavanlar_celiski_ve_varsayilan_kanit_deposundan_hesaplanir() {
    let ev = Ev::yeni("s2-kural");
    let p = ev.proje("nestjs-temel", "n");
    let sema = sha256(&std::fs::read(p.klasor.join("prisma").join("schema.prisma")).unwrap());
    let durumlar: Vec<Durum> = vec![
        ("db:c1", vec![("SOURCE_CODE", Some("certain"), "a")], "[Kesin]"),
        // Eksik beyan en zayıf değer (05 §1.5)
        ("db:c2", vec![("SOURCE_CODE", None, "a")], "[Yorum]"),
        ("db:c3", vec![("DECOMPILED", Some("certain"), "a")], "[Yorum]"),
        ("db:c4", vec![("BINARY_METADATA", Some("certain"), "a")], "[Gözlem]"),
        ("db:c4b", vec![("BINARY_HEURISTIC", Some("certain"), "a")], "[Yorum]"),
        ("db:c4c", vec![("RUNTIME_TRACE", Some("certain"), "a")], "[Gözlem]"),
        ("db:c4d", vec![("API_SCHEMA", Some("certain"), "a")], "[Kesin]"),
        // K2: olgu onayı tek başına en fazla observed; kaynak kanıtıyla certain; yalnızca decompile ile observed
        ("db:c5", vec![("USER_CONFIRMED", Some("certain"), "a")], "[Gözlem]"),
        ("db:c6", vec![("USER_CONFIRMED", Some("certain"), "a"), ("SOURCE_CODE", Some("inferred"), "a")], "[Kesin]"),
        ("db:c7", vec![("USER_CONFIRMED", Some("certain"), "a"), ("DECOMPILED", Some("certain"), "a")], "[Gözlem]"),
        // Çelişki en fazla inferred
        ("db:c8", vec![("SOURCE_CODE", Some("certain"), "a"), ("NETWORK_CAPTURE", Some("observed"), "b")], "[Yorum]"),
        // Çoklu kanıt düzeyi yükseltmez
        ("db:c9", vec![("DECOMPILED", Some("certain"), "a"), ("BINARY_HEURISTIC", Some("certain"), "a"), ("SOURCE_CODE", Some("inferred"), "a")], "[Yorum]"),
    ];
    let (mut kanitlar, mut onermeler, mut baglar) = (vec![], vec![], vec![]);
    for (varlik, ks, _) in &durumlar {
        let on = format!("on_{}", &varlik[3..]);
        // Dosyadaki hesap yanlış (hep certain): okuma modeli buna inanmamalı
        onermeler.push(json!({ "id": on, "varlik": varlik, "yuklem": "VAR", "deger": "a",
            "hesap": { "guven": "certain", "gecerlilik": "gecerli", "kapsam": "kapali", "destek": [], "celiski": [], "kapsam_disi": [], "revizyon": 1 } }));
        for (i, (tur, beyan, deger)) in ks.iter().enumerate() {
            let id = format!("kn_{}_{i}", &varlik[3..]);
            kanitlar.push(el_kaniti(&id, tur, *beyan, None));
            baglar.push(json!({ "kanit": id, "onerme": on, "yon": "destek", "deger": deger, "kapsam": "kapali" }));
        }
    }
    // Kanıtsız ama dosyada certain yazılmış önerme: bilinmiyor
    onermeler.push(json!({ "id": "on_c10", "varlik": "db:c10", "yuklem": "VAR", "deger": "a",
        "hesap": { "guven": "certain", "gecerlilik": "gecerli", "kapsam": "kapali", "destek": ["kn_yok"], "celiski": [], "kapsam_disi": [], "revizyon": 1 } }));
    // MAY_AFFECT asla certain (03 §3.1)
    onermeler.push(json!({ "id": "on_c11", "varlik": "db:c11", "yuklem": "MAY_AFFECT", "hedef": "db:c1", "deger": {}, "hesap": { "guven": "certain", "gecerlilik": "gecerli", "kapsam": "acik", "destek": [], "celiski": [], "kapsam_disi": [], "revizyon": 1 } }));
    kanitlar.push(el_kaniti("kn_c11", "DATABASE_SCHEMA", Some("certain"), Some(("prisma/schema.prisma", &sema))));
    baglar.push(json!({ "kanit": "kn_c11", "onerme": "on_c11", "yon": "destek", "deger": {}, "kapsam": "acik" }));
    // Dayanağı değişmiş kanıt: eski
    onermeler.push(json!({ "id": "on_c12", "varlik": "db:c12", "yuklem": "VAR", "deger": "a", "hesap": { "guven": "certain", "gecerlilik": "gecerli", "kapsam": "kapali", "destek": [], "celiski": [], "kapsam_disi": [], "revizyon": 1 } }));
    kanitlar.push(el_kaniti("kn_c12", "DATABASE_SCHEMA", Some("certain"), Some(("prisma/schema.prisma", "00"))));
    baglar.push(json!({ "kanit": "kn_c12", "onerme": "on_c12", "yon": "destek", "deger": "a", "kapsam": "kapali" }));
    let depo =
        json!({ "bicim": 1, "revizyon": 7, "uretici": "test", "zaman": "2026-01-01T00:00:00Z", "kok": "..", "kanitlar": kanitlar, "onermeler": onermeler, "baglar": baglar });
    std::fs::write(p.klasor.join(".sezgi").join("kanit.json"), depo.to_string()).unwrap();
    let mut m = p.mcp("");
    let etiket = |t: &str, bas: &str| t.lines().find(|l| l.starts_with(bas)).unwrap_or_else(|| panic!("{bas}: {t}")).to_string();
    for (varlik, _, beklenen) in &durumlar {
        let (t, hata) = m.arac("kanit", json!({ "varlik": varlik }));
        assert!(!hata, "{varlik}: {t}");
        let l = etiket(&t, "var");
        assert!(l.contains(beklenen), "{varlik}: {beklenen} bekleniyordu: {t}");
    }
    let (t, _) = m.arac("kanit", json!({ "varlik": "db:c8" }));
    assert!(t.contains("çelişki") && t.contains("NETWORK_CAPTURE"), "çelişkinin iki tarafı görünür: {t}");
    let (t, _) = m.arac("kanit", json!({ "varlik": "db:c10" }));
    assert!(etiket(&t, "var").contains("[Bilinmiyor]"), "{t}");
    let (t, _) = m.arac("kanit", json!({ "varlik": "db:c11" }));
    assert!(etiket(&t, "MAY_AFFECT").contains("[Yorum]"), "{t}");
    let (t, _) = m.arac("kanit", json!({ "varlik": "db:c12" }));
    let l = etiket(&t, "var");
    assert!(l.contains("[Bilinmiyor]") && l.contains("eski"), "{t}");
    // Bilinmeyenler listesi de aynı hesaptan: eski ve bilinmiyor olanlar başta
    let (t, _) = m.arac("bilinmeyenler", json!({ "adet": 3 }));
    assert!(t.contains("db:c12") && t.contains("db:c10"), "{t}");
}

#[test]
fn web_baglari_grafa_baglanir() {
    // Ön yüz adaptörünün çıktısı (web-baglari.json) aynı init'te grafa girer: ekran -> düğme -> istemci çağrısı -> kapı
    let ev = Ev::yeni("s2-web");
    let d = ev.kok.join("depo");
    let kaynak = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("s4_fastapi_react");
    for g in dosyalar(&kaynak) {
        let h = d.join(g.strip_prefix(&kaynak).unwrap());
        std::fs::create_dir_all(h.parent().unwrap()).unwrap();
        std::fs::copy(&g, &h).unwrap();
    }
    let p = Proje { klasor: d.join("backend"), ev: ev.kok.join("ev") };
    p.ok(&["init", "."]);
    let (k, g) = (oku(&p, "kanit.json"), oku(&p, "graf.json"));
    let api = "api:POST /api/v1/items/";
    let e = kenar(&g, "client:ItemsService.createItem", "CALLS", api);
    assert_eq!(e["guven"], "certain", "{e}");
    let t = kenarlar(&g, "ui:src/components/Items/AddItem.tsx#button:Save", "TRIGGERS");
    assert!(t.iter().any(|x| x["hedef"] == "client:ItemsService.createItem" && x["guven"] == "inferred"), "{t:?}");
    // Ön yüz kanıtının dayanağı ön yüz dosyası: yol uygulama köküne göre, özet dosyanın kendisinden
    let x = kanit(&k, onerme(&k, "client:ItemsService.createItem", "CALLS", Some(api))["hesap"]["destek"][0].as_str().unwrap());
    let dy = &x["dayanak"][0];
    let yol = dy["yol"].as_str().unwrap();
    assert!(yol.starts_with("../frontend/src/"), "{x}");
    assert_eq!(dy["sha256"].as_str().unwrap(), sha256(&std::fs::read(p.klasor.join(yol)).unwrap()));
    // Etki: kapının değişmesi düğmeyi ve ekranı etkiler; düğmeden kapıya
    let mut m = p.mcp("");
    let (t, _) = m.arac("etki", json!({ "kapi": "POST /api/v1/items/" }));
    assert!(t.contains("button:Save") && t.contains("web//items") && !t.contains("ekran bağı bilinmiyor"), "{t}");
    let (t, _) = m.arac("etki", json!({ "varlik": "ui:src/components/Items/AddItem.tsx#button:Save" }));
    assert!(t.contains("POST /api/v1/items/"), "düğme neyi değiştirir: {t}");
    // Ön yüz dosyası değişince bağ eski olur
    let f = p.klasor.join(yol);
    let mut icerik = std::fs::read_to_string(&f).unwrap();
    icerik.push_str("\n// değişti\n");
    std::fs::write(&f, icerik).unwrap();
    let (t, _) = p.mcp("").arac("kanit", json!({ "varlik": "client:ItemsService.createItem" }));
    assert!(t.contains("eski"), "{t}");
}

fn dosyalar(d: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = vec![];
    for g in std::fs::read_dir(d).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            v.extend(dosyalar(&p));
        } else {
            v.push(p);
        }
    }
    v
}
