//! Kotlin çağrı grafiği ve dış etki: Spring kurucu enjeksiyonu (`class S(private val repo: Repo)`) kesin `CALLS`
//! kenarı, Spring Data deposunun tablo etkisi zincirden; Ktor işleyici lambdası kendi fonksiyonu (adı fiil), rota
//! fonksiyonunun parametresi üzerinden servis yöntemine kenar, Exposed tablo etkisi (`Gorevler.insertAndGetId`)
//! zincirden; giden HTTP: Spring `RestTemplate` ve Ktor `HttpClient`.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn haritala(ornek: &str) -> PathBuf {
    let cikti = std::env::temp_dir().join(format!("sezgi-d8-kotlin-cagri-{ornek}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ornek);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    cikti
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn kenar<'a>(depo: &'a Value, kaynak_sonu: &str, hedef_sonu: &str) -> &'a Value {
    let o = depo["onermeler"].as_array().unwrap();
    o.iter()
        .find(|o| {
            o["yuklem"] == "CALLS"
                && o["varlik"].as_str().unwrap().starts_with("fn:")
                && o["varlik"].as_str().unwrap().ends_with(kaynak_sonu)
                && o["hedef"].as_str().unwrap_or("").ends_with(hedef_sonu)
        })
        .unwrap_or_else(|| {
            let v: Vec<String> = o.iter().filter(|o| o["yuklem"] == "CALLS").map(|o| format!("{} -> {}", o["varlik"], o["hedef"])).collect();
            panic!("{kaynak_sonu} -> {hedef_sonu} yok: {v:#?}")
        })
}

fn onermeler<'a>(depo: &'a Value, varlik: &str, yuklem: &str) -> Vec<&'a Value> {
    depo["onermeler"].as_array().unwrap().iter().filter(|o| o["varlik"] == varlik && o["yuklem"] == yuklem).collect()
}

fn dna(h: &Path, id: &str) -> Value {
    let m = json_oku(&h.join("kapilar.json"));
    let e = m["endpointler"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("{id} yok")).clone();
    m["ozet"]["dna"][format!("{}:{}", e["dosya"].as_str().unwrap(), e["satir"]).as_str()].clone()
}

#[test]
fn spring_kotlin_kurucu_enjeksiyonu_ve_depo_zinciri() {
    let h = haritala("d8_kotlin_spring");
    let depo = json_oku(&h.join("kanit.json"));
    let e = kenar(&depo, "KitapController.kt#olustur", "KitapServisi.kt#KitapServisi.olustur");
    assert_eq!(e["hesap"]["guven"], "certain", "{e:#}");
    // Zincirden tablo etkisi: KitapServisi.olustur -> kitapDeposu.save (JpaRepository<Kitap, Long>)
    let w = onermeler(&depo, "api:POST /k/api/kitaplar", "WRITES");
    assert!(w.iter().any(|o| o["hedef"] == "db:Kitap"), "{w:#?}");
    let d = dna(&h, "POST /k/api/kitaplar");
    assert!(d["cagri"]["zincir"].to_string().contains("KitapServisi.olustur"), "{d:#}");
    // Giden HTTP: RestTemplate.postForObject sabit adrese
    let dis = d["dis"].as_array().unwrap_or_else(|| panic!("dış etki yok: {d:#}"));
    let a = dis.iter().find(|x| x["dugum"] == "external:arama.ornek.com").unwrap_or_else(|| panic!("{d:#}"));
    assert_eq!((a["istemci"].as_str(), a["yontem"].as_str()), (Some("RestTemplate"), Some("POST")), "{a}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn ktor_isleyici_lambdasi_exposed_ve_http_istemcisi() {
    let h = haritala("d8_kotlin_ktor");
    let depo = json_oku(&h.join("kanit.json"));
    // İşleyici lambdası kendi fonksiyonu; rota fonksiyonunun parametresi (servis: GorevServisi) üzerinden kesin kenar
    let e = kenar(&depo, "Routing.kt#post", "GorevServisi.kt#GorevServisi.olustur");
    assert_eq!(e["hesap"]["guven"], "certain", "{e:#}");
    // Exposed tablo etkisi zincirden; başka işleyicilerin etkisi bu işleme karışmaz
    let api = "api:POST /app/api/v1/gorevler";
    assert!(onermeler(&depo, api, "WRITES").iter().any(|o| o["hedef"] == "db:Gorevler"), "{:#?}", onermeler(&depo, api, "WRITES"));
    let d = dna(&h, "GET /app/api/v1/gorevler");
    assert!(!d["tablo_etkisi"].to_string().contains("\"yazma\":true"), "listeleme yazmaz: {d:#}");
    // Ktor HttpClient: işleyici -> BildirimIstemcisi.bildir -> client.post(sabit adres)
    let d = dna(&h, "POST /app/api/v1/gorevler");
    let dis = d["dis"].as_array().unwrap_or_else(|| panic!("dış etki yok: {d:#}"));
    let b = dis.iter().find(|x| x["dugum"] == "external:bildirim.ornek.com").unwrap_or_else(|| panic!("{d:#}"));
    assert_eq!((b["istemci"].as_str(), b["yontem"].as_str()), (Some("Ktor HttpClient"), Some("POST")), "{b}");
    // Göreli adres: istemcinin `defaultRequest { url(...) }` tabanı
    let a = dis.iter().find(|x| x["dugum"] == "external:arama.ornek.com").unwrap_or_else(|| panic!("defaultRequest tabanı: {d:#}"));
    assert_eq!((a["istemci"].as_str(), a["yontem"].as_str()), (Some("Ktor HttpClient"), Some("PUT")), "{a}");
    let _ = std::fs::remove_dir_all(&h);
}
