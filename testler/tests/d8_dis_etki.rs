//! D8 dış etki (soru 12), geri alınabilirlik (soru 13) ve iş durumu ön koşulları (soru 10): giden çağrılar hedefi ve
//! güveniyle işleme bağlanır (sabit adres certain, ortam değişkeni inferred, dinamik unknown; zincirden ulaşılan en
//! fazla inferred), işlem başına `geri_alinabilir` (evet | kismen | hayir | bilinmiyor) gerekçesiyle verilir,
//! servisteki koşullu redler ön koşul olur. Kanıt / graf (`external:`), kart, `tani`, `etki`, `rapor` aynı veriyi gösterir.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d8dis-{ad}-{}-{n}", std::process::id()));
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
    h["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).cloned().unwrap_or_else(|| panic!("{id} yok"))
}

fn dna(h: &Value, id: &str) -> Value {
    let k = kapi(h, id);
    let yer = format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]);
    h["ozet"]["dna"].get(&yer).cloned().unwrap_or_else(|| panic!("{id} ({yer}) için dna yok"))
}

fn arac(harita: &Path, ad: &str, arguman: Value) -> String {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", harita.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

fn dis<'a>(d: &'a Value, dugum: &str) -> &'a Value {
    d["dis"].as_array().unwrap_or_else(|| panic!("dış etki yok: {d:#}")).iter().find(|x| x["dugum"] == dugum).unwrap_or_else(|| panic!("{dugum} yok: {d:#}"))
}

#[test]
fn giden_cagrilar_hedef_guven_ve_islem_baglari() {
    let h = haritala("d8_dis_express");
    let m = json_oku(&h.join("kapilar.json"));
    // Sabit adres (dosyadaki sabitten): kesin hedef; işleyiciden iki adım ötede: bağ çıkarım
    let s = dna(&m, "POST /api/siparisler");
    let odeme = dis(&s, "external:api.odeme.example.com");
    assert_eq!((odeme["tur"].as_str(), odeme["istemci"].as_str(), odeme["yontem"].as_str()), (Some("http"), Some("axios"), Some("POST")), "{odeme}");
    assert_eq!(odeme["hedef_turu"], "url");
    assert_eq!(odeme["yol"], "/v1/charges");
    assert_eq!(odeme["guven"], "inferred", "zincirden ulaşıldı: {odeme}");
    assert_eq!(odeme["zincir"].as_array().unwrap().last().unwrap(), "odemeAl", "{odeme}");
    assert!(odeme["kanit"].as_str().unwrap().starts_with("src/siparis/siparis.service.js:"), "{odeme}");
    // E-posta ve kuyruk SDK'ları
    let posta = dis(&s, "external:smtp");
    assert_eq!((posta["tur"].as_str(), posta["istemci"].as_str()), (Some("eposta"), Some("nodemailer")));
    let kuyruk = dis(&s, "external:kafka");
    assert_eq!((kuyruk["tur"].as_str(), kuyruk["kanal"].as_str()), (Some("kuyruk"), Some("siparis-olustu")), "{kuyruk}");
    // Ortam değişkeninden hedef: yorum; GET okuma sayılır
    let u = dna(&m, "GET /api/ucuslar/:no");
    let ucus = dis(&u, "external:ortam.UCUS_API_URL");
    assert_eq!((ucus["hedef_turu"].as_str(), ucus["yazma"].as_bool(), ucus["yol"].as_str()), (Some("ortam"), Some(false), Some("/flights/{}")), "{ucus}");
    assert_eq!(ucus["guven"], "inferred");
    assert!(u.get("geri_alinabilir").is_none(), "okumada geri alma sorulmaz: {u:#}");
    // Çözülemeyen adres: bilinmiyor
    let y = dna(&m, "POST /api/yansit");
    assert_eq!(dis(&y, "external:dinamik")["guven"], "unknown");
    // Uygulama düzeyi envanter: çağrı ve ulaşan işlemler
    let d = &m["ozet"]["dis"];
    assert!(d["istatistik"]["cagri"].as_u64().unwrap() >= 5, "{d:#}");
    let c = d["cagrilar"].as_array().unwrap().iter().find(|c| c["dugum"] == "external:api.odeme.example.com").unwrap();
    assert_eq!(c["guven"], "certain", "sabit adresin kendisi kesin: {c}");
    assert!(c["islemler"].as_array().unwrap().iter().any(|i| i == "POST /api/siparisler"), "{c}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn geri_alinabilirlik_ve_on_kosullar() {
    let h = haritala("d8_dis_express");
    let m = json_oku(&h.join("kapilar.json"));
    // Ters işlem (DELETE) var ama dış etki geri alınamaz: kısmen
    let s = dna(&m, "POST /api/siparisler");
    assert_eq!(s["geri_alinabilir"], "kismen", "{s:#}");
    assert!(s["geri_alma_gerekce"].as_str().unwrap().contains("dış etki geri alınamaz"), "{s:#}");
    assert_eq!(s["geri_alma_guveni"], "inferred");
    // Kalıcı silme + kaskad: hayır
    let sil = dna(&m, "DELETE /api/siparisler/:id");
    assert_eq!(sil["geri_alinabilir"], "hayir", "{sil:#}");
    assert!(sil["geri_alma_gerekce"].as_str().unwrap().contains("Kalem"), "kaskadla dolaylı silinen: {sil:#}");
    // Yumuşak silme + geri yükleme ucu: evet; oluşturan işlemin ters işlemi var: evet
    let not = dna(&m, "DELETE /api/notlar/:id");
    assert_eq!(not["geri_alinabilir"], "evet", "{not:#}");
    assert!(not["geri_alma_gerekce"].as_str().unwrap().contains("POST /api/notlar/:id/restore"), "{not:#}");
    assert_eq!(dna(&m, "POST /api/notlar")["geri_alinabilir"], "evet");
    // Güncelleme: önceki değer bilinmiyor
    let g = dna(&m, "PUT /api/siparisler/:id");
    assert_eq!(g["geri_alinabilir"], "bilinmiyor");
    // Ön koşul: servisteki koşullu red (kodu bilinmeyen) davranış özetinde, önizleme GEÇER demez
    let r = g["hatalar"].as_array().unwrap().iter().find(|x| x["kosul"].as_str().is_some_and(|k| k.contains("kapandi"))).unwrap_or_else(|| panic!("ön koşul yok: {g:#}"));
    assert_eq!((r["kaynak"].as_str(), r["guven"].as_str()), (Some("servis"), Some("inferred")), "{r}");
    let o = arac(&h, "onizle", json!({ "kapi": "PUT /api/siparisler/:id", "oturum": true, "parametreler": { "id": "1" }, "govde": {} }));
    assert!(!o.contains("GEÇER (bilinen bütün kurallar)"), "{o}");
    assert!(o.contains("HTTP kodu bilinmiyor"), "{o}");
    // POST ön koşulu (`!girdi.urunId`) da zincirde
    assert!(s["hatalar"].as_array().unwrap().iter().any(|x| x["kosul"] == "!girdi.urunId"), "{s:#}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn kanit_graf_kart_tani_etki_ve_rapor() {
    let h = haritala("d8_dis_express");
    let m = json_oku(&h.join("kapilar.json"));
    // Onay gerekçesi kapıda (canlı modun onay kuralı okur); salt okuma işleminde yok
    assert!(kapi(&m, "POST /api/siparisler")["etki_onayi"].as_str().unwrap().contains("uygulamanın dışına etki"), "{}", kapi(&m, "POST /api/siparisler"));
    assert!(kapi(&m, "GET /api/ucuslar/:no").get("etki_onayi").is_none());
    assert!(kapi(&m, "POST /api/notlar").get("etki_onayi").is_none(), "geri alınabilir ve dış etkisiz yazma");
    // Kanıt deposu: api -[CALLS]-> external:..., değeri çağrıların listesi
    let k = json_oku(&h.join("kanit.json"));
    let o = k["onermeler"].as_array().unwrap().iter().find(|o| o["varlik"] == "api:POST /api/siparisler" && o["yuklem"] == "CALLS" && o["hedef"] == "external:api.odeme.example.com");
    let o = o.unwrap_or_else(|| panic!("CALLS önermesi yok"));
    assert_eq!(o["deger"]["cagrilar"][0]["istemci"], "axios");
    assert!(k["onermeler"].as_array().unwrap().iter().all(|o| o["hesap"]["celiski"].as_array().unwrap().is_empty()), "çelişen kanıt olmamalı");
    let g = json_oku(&h.join("graf.json"));
    assert!(g.to_string().contains("external:kafka"), "graf dış sistem düğümü");
    // Kart: x-dna yan etkileri, geri alma, onay; kökte x-external
    let kart = json_oku(&h.join("benlik.json"));
    let op = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "POST" && o["entry"]["path"] == "/api/siparisler").unwrap();
    assert_eq!(op["requires_approval"], true);
    assert_eq!(op["x-dna"]["reversibility"]["status"], "partial");
    assert!(op["x-dna"]["side_effects"].as_array().unwrap().iter().any(|s| s["target"] == "external:smtp" && s["kind"] == "email"), "{op:#}");
    assert!(kart["x-external"].as_array().unwrap().iter().any(|s| s["id"] == "external:api.odeme.example.com" && s["target_kind"] == "url"), "{}", kart["x-external"]);
    // tani ve etki
    let t = arac(&h, "tani", json!({ "kapi": "POST /api/siparisler" }));
    assert!(t.contains("dış etki: HTTP POST api.odeme.example.com/v1/charges"), "{t}");
    assert!(t.contains("geri alma: kısmen"), "{t}");
    let e = arac(&h, "etki", json!({ "kapi": "POST /api/siparisler" }));
    assert!(e.contains("dış sistemler:") && e.contains("api.odeme.example.com"), "{e}");
    // rapor
    let r = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["rapor", h.to_str().unwrap(), "--json"]).output().unwrap();
    let r: Value = serde_json::from_slice(&r.stdout).unwrap();
    assert!(r["dis"]["cagri"].as_u64().unwrap() >= 5, "{}", r["dis"]);
    assert_eq!(r["dis"]["geri_alinabilirlik"]["hayir"], 2, "kalıcı silme ve dinamik adrese POST: {}", r["dis"]);
    let r = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["rapor", h.to_str().unwrap()]).output().unwrap();
    let r = String::from_utf8_lossy(&r.stdout);
    assert!(r.contains("DIŞ ETKİ") && r.contains("GERİ ALMA") && r.contains("ÖN KOŞUL"), "{r}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn ters_islem_ve_surum_kaydi() {
    let h = haritala("d8_dis_express");
    let m = json_oku(&h.join("kapilar.json"));
    // `flag` ↔ `unflag`: ters işlem
    let f = dna(&m, "POST /api/notlar/:id/flag");
    assert_eq!(f["geri_alinabilir"], "evet", "{f:#}");
    assert!(f["geri_alma_gerekce"].as_str().unwrap().contains("ters işlem: POST /api/notlar/:id/unflag"), "{f:#}");
    // Güncellemede önceki sürüm saklanıyor ve sürüme dönüş ucu var
    let g = dna(&m, "PUT /api/notlar/:id");
    assert_eq!(g["geri_alinabilir"], "evet", "{g:#}");
    let gerekce = g["geri_alma_gerekce"].as_str().unwrap();
    assert!(gerekce.contains("önceki sürüm kaydı yazılıyor: notRevisionRepo") && gerekce.contains("revisions/:surum/restore"), "{g:#}");
    // Sürümü geri yükleyen uç kaydı silmenin tersi sayılmaz: silme yumuşak, geri yükleme ucu notun kendisi
    let s = dna(&m, "DELETE /api/notlar/:id");
    assert!(s["geri_alma_gerekce"].as_str().unwrap().contains("POST /api/notlar/:id/restore"), "{s:#}");
    let _ = std::fs::remove_dir_all(&h);
}
