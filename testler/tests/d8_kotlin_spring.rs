//! Kotlin + Spring Boot: aynı Spring adaptörü `.kt` kaynaklarını Kotlin okuyucusuyla okur. Sabitler ve dizgi şablonları
//! (`"$API/auth"`), Kotlin güvenlik DSL'i (`http { authorizeHttpRequests { authorize(...) } }`), data class DTO'lar
//! (`@field:NotBlank` kullanım yeri hedefi; hedefsiz kısıt kurucu parametresine gider ve doğrulanmaz), Kotlin
//! boş olabilirliği (`String?` isteğe bağlı, varsayılan değer isteğe bağlı, jackson-module-kotlin ile boş olamayan alan
//! zorunlu), `?: throw` ve `orElseThrow`, JPA varlıkları, kaskad, @SQLDelete, olaylar; kart, kanıt, doktor ve canlı mod.
mod p0_g1_ortak;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const ORNEK: &str = "d8_kotlin_spring";

fn harita(ad: &str) -> (Value, PathBuf) {
    let cikti = std::env::temp_dir().join(format!("sezgi-d8-kotlin-spring-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ORNEK);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    (json_oku(&cikti.join("kapilar.json")), cikti)
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn uc<'a>(h: &'a Value, id: &str) -> &'a Value {
    h["endpointler"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("{id} yok: {}", h["endpointler"]))
}

fn konum(h: &Value, id: &str) -> String {
    let e = uc(h, id);
    format!("{}:{}", e["dosya"].as_str().unwrap(), e["satir"])
}

fn dna<'a>(h: &'a Value, id: &str) -> &'a Value {
    &h["ozet"]["dna"][konum(h, id).as_str()]
}

fn alanlar(e: &Value, bolum: &str) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    for g in e[bolum].as_array().unwrap() {
        if let Some(a) = g["sema"]["alanlar"].as_object() {
            m.extend(a.clone());
        }
    }
    m
}

fn kurallar(a: &Value) -> Vec<String> {
    a["kurallar"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect()
}

#[test]
fn rotalar_sabitler_ve_sablonlar() {
    let (h, cikti) = harita("rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    // context-path, `object` sabiti ve dizgi şablonu ("$API/kitaplar", "$API/auth"); test kaynakları okunmaz
    assert_eq!(
        idler,
        [
            "GET /k/api/auth/ben",
            "POST /k/api/auth/giris",
            "GET /k/api/kitaplar",
            "POST /k/api/kitaplar",
            "DELETE /k/api/kitaplar/:id",
            "GET /k/api/kitaplar/:id",
            "PUT /k/api/kitaplar/:id",
            "GET /k/api/yonetim/rapor",
            "DELETE /k/api/yonetim/yazarlar/:id",
        ]
    );
    let o = &h["ozet"];
    assert_eq!(o["adaptor"], "spring@0.1.0");
    assert_eq!(o["uygulama"], "kitaplik");
    assert_eq!(o["ana_uygulama"], "com.ornek.kitaplik.KitaplikApplication");
    // KDoc'un ilk cümlesi özet; @PathVariable("id") adı; Kotlin tipi Java karşılığıyla
    assert_eq!(uc(&h, "GET /k/api/kitaplar")["ozet"], "Kitapları başlığa göre arar");
    assert_eq!(uc(&h, "DELETE /k/api/kitaplar/:id")["parametreler"], json!([{ "ad": "id", "tip": "Long" }]));
    assert_eq!(uc(&h, "GET /k/api/kitaplar/:id")["dosya"], "src/main/kotlin/com/ornek/kitaplik/web/KitapController.kt");
    let kart = json_oku(&cikti.join("benlik.json"));
    assert_eq!(kart["app"]["version"], "1.2.0");
    assert!(kart["operations"].as_array().unwrap().iter().any(|op| op["id"] == "kitap.olustur"), "{}", kart["operations"]);
    let oa = json_oku(&cikti.join("openapi.json"));
    assert!(oa["paths"]["/k/api/kitaplar/{id}"]["delete"].is_object(), "{}", oa["paths"]);
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn kotlin_guvenlik_dsli_ve_yontem_guvenligi() {
    let (h, cikti) = harita("giris");
    let karar = |id: &str| {
        let e = uc(&h, id);
        (e["giris"].as_str().unwrap().to_string(), e["giris_kaniti"].as_str().unwrap().to_string())
    };
    let beklenen: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        // authorize(HttpMethod.GET, "/api/kitaplar/**", permitAll): yalnızca GET
        ("GET /k/api/kitaplar", ("herkese_acik", "kesin")),
        ("GET /k/api/kitaplar/:id", ("herkese_acik", "kesin")),
        ("POST /k/api/kitaplar", ("gerekli", "kesin")),
        ("PUT /k/api/kitaplar/:id", ("gerekli", "kesin")),
        ("POST /k/api/auth/giris", ("herkese_acik", "kesin")),
        // Zincir açık bırakıyor ama yöntemde AOP kesitinin anotasyonu var: denetim uygulamanın kodunda
        ("GET /k/api/auth/ben", ("herkese_acik", "varsayim_bilinmeyen_guard")),
        // authorize(anyRequest, authenticated) + @PreAuthorize("hasRole('ADMIN')")
        ("DELETE /k/api/kitaplar/:id", ("gerekli", "kesin")),
        ("GET /k/api/yonetim/rapor", ("gerekli", "kesin")),
    ]);
    for (id, (g, k)) in beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string()), "{id}: {:?}", uc(&h, id)["guardlar"]);
    }
    assert_eq!(uc(&h, "DELETE /k/api/kitaplar/:id")["roller"], json!(["ADMIN"]));
    // Zincir ADMIN, sınıf ADMIN ya da EDITOR: ikisinin de izin verdiği ADMIN
    assert_eq!(uc(&h, "GET /k/api/yonetim/rapor")["roller"], json!(["ADMIN"]));
    assert!(uc(&h, "GET /k/api/kitaplar")["guardlar"][0].as_str().unwrap().contains("permitAll"), "{}", uc(&h, "GET /k/api/kitaplar")["guardlar"]);
    assert_eq!(h["ozet"]["genel_guardlar"], json!(["SecurityFilterChain GuvenlikAyari.filterChain"]));
    assert!(uc(&h, "GET /k/api/auth/ben")["guardlar"].to_string().contains("@ApiAnahtari (AOP)"), "{}", uc(&h, "GET /k/api/auth/ben")["guardlar"]);
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn data_class_alanlari_bos_olabilirlik_ve_kullanim_yeri_hedefi() {
    let (h, cikti) = harita("alan");
    let olustur = uc(&h, "POST /k/api/kitaplar");
    let g = alanlar(olustur, "govde");
    let zorunlu = |a: &str| g[a]["zorunlu"].as_bool().unwrap();
    // jackson-module-kotlin: boş olamayan ve varsayılanı olmayan alan zorunlu; Int (ilkel) eksikse 0 olur
    assert!(zorunlu("baslik") && zorunlu("yazarAdi"), "{g:?}");
    assert!(!zorunlu("sayfaSayisi") && !zorunlu("isbn_kodu") && !zorunlu("etiketler") && !zorunlu("tur"), "{g:?}");
    assert!(g.get("isbn").is_none(), "@JsonProperty adı");
    for (a, k) in [("baslik", "IsNotEmpty()"), ("baslik", "MinLength(2)"), ("baslik", "MaxLength(200)"), ("sayfaSayisi", "IsPositive()"), ("etiketler", "IsArray()")] {
        assert!(kurallar(&g[a]).contains(&k.to_string()), "{a}: {:?}", kurallar(&g[a]));
    }
    assert!(kurallar(&g["isbn_kodu"]).contains(&r"Matches(/^(?:\d{13})$/)".to_string()), "{:?}", kurallar(&g["isbn_kodu"]));
    assert_eq!(g["tur"]["degerler"], json!(["ROMAN", "SIIR"]));
    assert_eq!(g["sayfaSayisi"]["tip"], "int");
    // Hedefsiz @NotBlank kurucu parametresine gider: alan doğrulanmaz, not düşülür
    let guncelle = uc(&h, "PUT /k/api/kitaplar/:id");
    let g2 = alanlar(guncelle, "govde");
    assert!(!g2["baslik"]["zorunlu"].as_bool().unwrap() && !kurallar(&g2["baslik"]).contains(&"IsNotEmpty()".to_string()), "{g2:?}");
    let not = h["ozet"]["denetlenemeyen"][konum(&h, "PUT /k/api/kitaplar/:id").as_str()].to_string();
    assert!(not.contains("@field:NotBlank"), "{}", h["ozet"]["denetlenemeyen"]);
    let giris = alanlar(uc(&h, "POST /k/api/auth/giris"), "govde");
    assert!(kurallar(&giris["eposta"]).contains(&"IsEmail()".to_string()) && kurallar(&giris["sifre"]).contains(&"MinLength(8)".to_string()));
    // Sorgu: String? ve varsayılan değer isteğe bağlı, boş olamayan Int zorunlu (Spring Kotlin kuralı)
    let s = alanlar(uc(&h, "GET /k/api/kitaplar"), "sorgu");
    assert!(!s["q"]["zorunlu"].as_bool().unwrap() && !s["sayfa"]["zorunlu"].as_bool().unwrap() && s["adet"]["zorunlu"].as_bool().unwrap(), "{s:?}");
    // Yanıt: dönüş tipi (List dizi), @ResponseStatus(HttpStatus.CREATED)
    assert_eq!(olustur["yanit"]["sema"]["dto"], "KitapYaniti");
    assert_eq!(olustur["yanit"]["durum"], 201);
    assert_eq!(uc(&h, "GET /k/api/kitaplar")["yanit"]["sema"]["dizi"], true);
    assert!(uc(&h, "POST /k/api/auth/giris")["yanit"].is_null(), "ResponseEntity<*>: şema yok");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn veri_davranis_olay_ve_kanit() {
    let (h, cikti) = harita("veri");
    let veri = |id: &str| (uc(&h, id)["veri"]["okur"].clone(), uc(&h, id)["veri"]["yazar"].clone());
    assert_eq!(veri("POST /k/api/kitaplar"), (json!(["Yazar"]), json!(["Kitap"])));
    assert_eq!(veri("GET /k/api/kitaplar"), (json!(["Kitap"]), json!([])));
    assert_eq!(veri("DELETE /k/api/kitaplar/:id"), (json!([]), json!(["Kitap"])));
    let kaskad = |e: &str, c: &str| h["ozet"]["kaskadlar"].as_array().unwrap().iter().any(|k| k[0] == e && k[1] == c);
    assert!(kaskad("Kitap", "Yorum"), "{}", h["ozet"]["kaskadlar"]);
    assert_eq!(dna(&h, "DELETE /k/api/kitaplar/:id")["dolayli_yazar"][0]["tablo"], "Yorum");
    // @SQLDelete: yazar silme geri alınabilir
    let y = dna(&h, "DELETE /k/api/yonetim/yazarlar/:id");
    assert!(y["yumusak_silme"].as_str().is_some_and(|x| x.contains("Varliklar.kt")), "{y:#}");
    let hata = |d: &Value, kod: u64| d["hatalar"].as_array().unwrap().iter().find(|x| x["durum"] == kod).cloned();
    // `?: throw KitapBulunamadi(id)`: @ResponseStatus(NOT_FOUND) sınıfı, koşul sorgunun boş olması
    let g = hata(dna(&h, "GET /k/api/kitaplar/:id"), 404).unwrap_or_else(|| panic!("{:#}", dna(&h, "GET /k/api/kitaplar/:id")));
    assert!(g["kosul"].as_str().unwrap().contains("findByIdOrNull"), "{g}");
    assert_eq!(g["kaynak"], "servis");
    assert!(hata(dna(&h, "POST /k/api/kitaplar"), 400).is_some(), "ResponseStatusException(BAD_REQUEST)");
    assert!(hata(dna(&h, "POST /k/api/auth/giris"), 401).is_some(), "ResponseEntity.status(UNAUTHORIZED)");
    let tetik: Vec<String> =
        dna(&h, "POST /k/api/kitaplar")["tetikler"].as_array().unwrap().iter().map(|t| format!("{}:{}", t["tur"].as_str().unwrap(), t["ad"].as_str().unwrap())).collect();
    assert!(tetik.contains(&"event:KitapEklendi".to_string()), "{tetik:?}");
    assert_eq!(h["ozet"]["olaylar"]["event:KitapEklendi"]["dinleyiciler"][0]["fonksiyon"], "KitapDinleyici.kitapEklendi");
    let depo = json_oku(&cikti.join("kanit.json"));
    let onermeler = depo["onermeler"].as_array().unwrap();
    let var = |v: &str, y: &str, f: &dyn Fn(&Value) -> bool| onermeler.iter().any(|o| o["varlik"] == v && o["yuklem"] == y && f(o));
    assert!(var("db:Kitap", "TABLO", &|o| o["deger"]["tablo"] == "kitaplar"));
    assert!(var("db:Yazar", "TABLO", &|o| o["deger"]["tablo"] == "yazar"));
    assert!(var("field:Kitap.baslik", "VAR", &|o| o["deger"]["sutun"] == "kitap_basligi"));
    assert!(var("api:POST /k/api/kitaplar", "WRITES", &|o| o["hedef"] == "db:Kitap"));
    assert!(var("api:POST /k/api/kitaplar", "TRIGGERS", &|o| o["hedef"] == "event:KitapEklendi"));
    assert!(var("api:GET /k/api/kitaplar", "DEPENDS_ON", &|o| o["hedef"].as_str().is_some_and(|x| x.ends_with("KitapController.kt:KitapController"))));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn doktor_rapor_canli_mod_ve_izleme() {
    use p0_g1_ortak::*;
    let ev = Ev::yeni("d8-kotlin-spring");
    let u = Uygulama::yeni();
    let p = ev.proje(ORNEK, "a");
    let (_, d) = p.cikti(&["doktor"]);
    assert!(d.contains("Spring Boot (Kotlin)") && d.contains("kitaplik"), "{d}");
    let r = p.ok(&["rapor", ".sezgi"]);
    assert!(r.contains("spring@0.1.0"), "{r}");
    // Kotlin kaynakları haritanın okunan dosyalarıdır (bayatlık ve izleme)
    let ozet = json_oku(&p.klasor.join(".sezgi").join("kapilar.json"));
    assert!(ozet["ozet"]["okunan_dosyalar"].to_string().contains("KitapController.kt"), "{}", ozet["ozet"]["okunan_dosyalar"]);
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    let (s, hata) = p.mcp(&t).cagir(json!({ "kapi": "GET /k/api/kitaplar/:id", "parametreler": { "id": "7" }, "sorgu": {} }));
    assert!(!hata, "{s}");
    let g = u.gelenler.lock().unwrap();
    assert_eq!(g.last().map(|x| (x.yontem.as_str(), x.yol.as_str())), Some(("GET", "/k/api/kitaplar/7")), "bağlam yolu ve yol parametresi");
}
