//! P1 hızlı doğruluk düzeltmeleri (H4):
//! 1. `@IsIn(SABIT_DIZI)`: aynı adlı dizi birden fazla dosyada varsa kullanan dosyanın gördüğü dizi alınır.
//! 2. Express dosya yüklemesi (multer `upload.single(...)`): önizleme "dosya doğrulaması kontrol edilemedi" der.
//! 3. NestJS: dosya adı `.controller.ts` ile bitmeyen ama `@Controller` dekoratörlü sınıf haritaya girer.
//! 4. FastAPI: SQLModel tablo modelleri veri tanımı sayılır; rapor tablo bağlarını `tani` ile tutarlı gösterir.
//! 5. Rol: FastAPI süper kullanıcı bağımlılığı ve Express `admin` ara katmanı rol olarak görünür (çıkarım).
//! 6. OpenAPI: operationId uygulamanın kendi kimliği (FastAPI generate_unique_id_function, NestJS @ApiOperation
//!    operationId); koşullu kaydedilen rota `x-sezgi-kosullu` ile işaretlenir.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

/// Her çağrı kendi çıktı klasörünü alır: aynı süreçteki paralel testler birbirinin haritasını silmesin
fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-p1h4-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", ornek(ad).to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "init: {}", String::from_utf8_lossy(&c.stderr));
    cikti
}

fn oku(h: &Path, dosya: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(h.join(dosya)).unwrap()).unwrap()
}

fn kapi<'a>(harita: &'a Value, yontem: &str, yol: &str) -> &'a Value {
    harita["endpointler"].as_array().unwrap().iter().find(|k| k["yontem"] == yontem && k["yol"] == yol).unwrap_or_else(|| {
        let var: Vec<String> = harita["endpointler"].as_array().unwrap().iter().map(|k| format!("{} {}", k["yontem"], k["yol"])).collect();
        panic!("kapı yok: {yontem} {yol}; haritada: {var:?}")
    })
}

fn onizle(harita: &Path, arguman: Value) -> String {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", harita.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

fn islem(openapi: &Value, yontem: &str, yol: &str) -> Value {
    openapi["paths"][yol][yontem].clone()
}

#[test]
fn isin_sabit_dizisi_dosya_kapsamli() {
    let h = haritala("p1_h4_nest");
    let m = oku(&h, "kapilar.json");
    let k = kapi(&m, "POST", "/siparisler");
    let degerler = &k["govde"][0]["sema"]["alanlar"]["durum"]["degerler"];
    assert_eq!(degerler, &json!(["acik", "kapali"]), "IsIn(DURUMLAR) kendi dosyasındaki diziyle çözülmeli: {k}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn ek_uymayan_ama_dekoratorlu_controller_haritada() {
    let h = haritala("p1_h4_nest");
    let m = oku(&h, "kapilar.json");
    let k = kapi(&m, "GET", "/yonetim/durum");
    assert_eq!(k["giris"], "gerekli", "{k}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn nestjs_operation_id_uygulamanin_kendisi() {
    let h = haritala("p1_h4_nest");
    let o = oku(&h, "openapi.json");
    assert_eq!(islem(&o, "post", "/siparisler")["operationId"], "siparisOlustur", "{}", o["paths"]);
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn express_multer_dosya_dogrulamasi_bildirilir() {
    let h = haritala("p1_h4_express");
    let m = onizle(&h, json!({ "kapi": "POST /api/urunler/resim", "oturum": true, "govde": { "aciklama": "x" } }));
    assert!(m.contains("dosya doğrulaması kontrol edilemedi"), "multer yüklemesi bildirilmeli:\n{m}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn express_admin_ara_katmani_rol_olarak_gorunur() {
    let h = haritala("p1_h4_express");
    let m = oku(&h, "kapilar.json");
    let k = kapi(&m, "DELETE", "/api/urunler/:id");
    assert!(k["roller"].as_array().unwrap().iter().any(|r| r == "admin"), "admin ara katmanı rol olmalı: {k}");
    assert_eq!(k["roller_kaniti"], "varsayim", "addan çıkarılan rol kesin olamaz: {k}");
    // admin'siz rota rol istemez
    assert!(kapi(&m, "GET", "/api/urunler")["roller"].as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn fastapi_super_kullanici_rol_olarak_gorunur() {
    let h = haritala("p1_h4_fastapi");
    let m = oku(&h, "kapilar.json");
    let k = kapi(&m, "DELETE", "/ogeler/:oge_id");
    assert!(k["roller"].as_array().unwrap().iter().any(|r| r == "superuser"), "süper kullanıcı bağımlılığı rol olmalı: {k}");
    assert_eq!(k["roller_kaniti"], "varsayim", "{k}");
    assert!(kapi(&m, "GET", "/ogeler/")["roller"].as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn fastapi_rapor_tablo_baglarini_tani_ile_tutarli_gosterir() {
    let h = haritala("p1_h4_fastapi");
    let m = oku(&h, "kapilar.json");
    // tani'nın gösterdiği bağ
    assert!(kapi(&m, "DELETE", "/ogeler/:oge_id")["veri"]["yazar"].as_array().is_some_and(|y| !y.is_empty()), "{}", kapi(&m, "DELETE", "/ogeler/:oge_id"));
    let r = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["rapor", h.to_str().unwrap(), "--json"]).output().unwrap();
    let r: Value = serde_json::from_slice(&r.stdout).unwrap();
    assert!(r["veri"]["sema_dosyalari"].as_array().is_some_and(|a| !a.is_empty()), "veri tanımı (SQLModel) raporda görünmeli: {}", r["veri"]);
    assert!(r["veri"]["bagli_kapi"].as_u64().is_some_and(|n| n >= 1), "rapor bağlı kapıları saymalı: {}", r["veri"]);
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn fastapi_operation_id_ve_kosullu_rota() {
    let h = haritala("p1_h4_fastapi");
    let o = oku(&h, "openapi.json");
    assert_eq!(islem(&o, "get", "/ogeler/")["operationId"], "ogeler-ogeleri_oku", "{}", o["paths"]);
    assert_eq!(islem(&o, "delete", "/ogeler/{oge_id}")["operationId"], "ogeler-oge_sil");
    let ozel = islem(&o, "post", "/ozel/kullanicilar");
    assert_eq!(ozel["x-sezgi-kosullu"], true, "koşullu kaydedilen rota işaretlenmeli: {ozel}");
    assert!(islem(&o, "get", "/ogeler/")["x-sezgi-kosullu"].is_null());
    let _ = std::fs::remove_dir_all(&h);
}
