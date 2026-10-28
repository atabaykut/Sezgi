//! Ktor: `routing { route("/x") { get { } } }` DSL'i, rota fonksiyonları (`fun Route.gorevRotalari(...)`), sabitler,
//! `ktor.deployment.rootPath`, `authenticate("jwt") { }` (isteğe bağlı giriş dahil), uygulamanın kendi sarmalayıcısı
//! (`rolIle("ADMIN") { }`: tanınmayan guard, rol tahmin), koşullu kayıt, `call.receive<Dto>()` ve kotlinx.serialization
//! (`@SerialName`, varsayılansız alan zorunlu), yol / sorgu parametreleri ve tipleri, `call.respond` yanıtı ve hata
//! kodları, StatusPages eşlemesi, Exposed tablo etkisi ve kaskadları; kart, kanıt, doktor ve canlı mod.
mod p0_g1_ortak;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const ORNEK: &str = "d8_kotlin_ktor";

fn harita(ad: &str) -> (Value, PathBuf) {
    let cikti = std::env::temp_dir().join(format!("sezgi-d8-kotlin-ktor-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ORNEK);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
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

#[test]
fn rotalar_rota_fonksiyonlari_ve_kok_yolu() {
    let (h, cikti) = harita("rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(
        idler,
        [
            "GET /app/api/v1/gorevler",
            "POST /app/api/v1/gorevler",
            "GET /app/api/v1/gorevler/:id",
            "PUT /app/api/v1/gorevler/:id",
            "POST /app/api/v1/gorevler/:id/yorumlar",
            "GET /app/api/v1/gorevler/ozet",
            "DELETE /app/api/v1/yonetim/gorevler/:id",
            "GET /app/hata-ayikla",
            "GET /app/saglik",
        ]
    );
    let o = &h["ozet"];
    assert_eq!(o["adaptor"], "ktor@0.1.0");
    assert_eq!(o["uygulama"], "gorevler");
    assert_eq!(o["genel_onek"], "/app");
    // `if (gelistirme) { get(...) }`: koşullu kayıt
    let kosullu: Vec<&str> = o["kosullu_kapilar"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
    assert_eq!(kosullu, ["GET /app/hata-ayikla"]);
    // İşleyici rota fonksiyonunun içindeki lambda: konum dosya:satır
    assert_eq!(uc(&h, "GET /app/api/v1/gorevler/:id")["dosya"], "src/main/kotlin/com/ornek/gorevler/Routing.kt");
    assert_eq!(uc(&h, "GET /app/api/v1/gorevler/:id")["satir"], 53);
    // Yol parametresi tipi dönüşümden: toLongOrNull(), getOrFail<Long>
    assert_eq!(uc(&h, "GET /app/api/v1/gorevler/:id")["parametreler"], json!([{ "ad": "id", "tip": "Long" }]));
    assert_eq!(uc(&h, "PUT /app/api/v1/gorevler/:id")["parametreler"], json!([{ "ad": "id", "tip": "Long" }]));
    let kart = json_oku(&cikti.join("benlik.json"));
    assert_eq!(kart["app"]["version"], "0.3.0");
    assert!(kart["generator"]["adapters"].to_string().contains("ktor@0.1.0"));
    let oa = json_oku(&cikti.join("openapi.json"));
    assert!(oa["paths"]["/app/api/v1/gorevler/{id}"]["put"].is_object(), "{}", oa["paths"]);
    // Graf: rota fonksiyonu yönlendirici düğümü
    let depo = json_oku(&cikti.join("kanit.json"));
    let onermeler = depo["onermeler"].as_array().unwrap();
    assert!(
        onermeler.iter().any(|o| o["varlik"] == "api:GET /app/api/v1/gorevler" && o["yuklem"] == "DEPENDS_ON" && o["hedef"].as_str().is_some_and(|x| x.ends_with("Routing.kt:gorevRotalari"))),
        "{}",
        serde_json::to_string(&onermeler.iter().filter(|o| o["varlik"] == "api:GET /app/api/v1/gorevler").collect::<Vec<_>>()).unwrap()
    );
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn giris_istege_bagli_giris_ve_sarmalayici() {
    let (h, cikti) = harita("giris");
    let karar = |id: &str| {
        let e = uc(&h, id);
        (e["giris"].as_str().unwrap().to_string(), e["giris_kaniti"].as_str().unwrap().to_string())
    };
    let beklenen: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        ("GET /app/saglik", ("herkese_acik", "kesin")),
        ("GET /app/api/v1/gorevler", ("herkese_acik", "kesin")),
        ("GET /app/api/v1/gorevler/:id", ("herkese_acik", "kesin")),
        ("POST /app/api/v1/gorevler", ("gerekli", "kesin")),
        ("PUT /app/api/v1/gorevler/:id", ("gerekli", "kesin")),
        // authenticate(optional = true): kimlik varsa okunur, yoksa istek geçer
        ("GET /app/api/v1/gorevler/ozet", ("herkese_acik", "kesin")),
        // authenticate içinde uygulamanın kendi sarmalayıcısı: giriş kesin, rol tahmin
        ("DELETE /app/api/v1/yonetim/gorevler/:id", ("gerekli", "kesin")),
    ]);
    for (id, (g, k)) in beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string()), "{id}: {:?}", uc(&h, id)["guardlar"]);
    }
    assert!(uc(&h, "POST /app/api/v1/gorevler")["guardlar"].to_string().contains("authenticate(\\\"auth-jwt\\\")"), "{}", uc(&h, "POST /app/api/v1/gorevler")["guardlar"]);
    assert!(uc(&h, "GET /app/api/v1/gorevler/ozet")["guardlar"].to_string().contains("optional"));
    let sil = uc(&h, "DELETE /app/api/v1/yonetim/gorevler/:id");
    assert_eq!((sil["roller"].clone(), sil["roller_kaniti"].as_str()), (json!(["ADMIN"]), Some("varsayim")));
    assert!(sil["guardlar"].to_string().contains("rolIle"), "{}", sil["guardlar"]);
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn govde_sorgu_yanit_ve_kotlinx_serilestirme() {
    let (h, cikti) = harita("alan");
    let olustur = uc(&h, "POST /app/api/v1/gorevler");
    let g = alanlar(olustur, "govde");
    let zorunlu = |m: &serde_json::Map<String, Value>, a: &str| m[a]["zorunlu"].as_bool().unwrap();
    // kotlinx.serialization: varsayılan değeri olmayan alan zorunlu (boş olabilse bile)
    assert!(zorunlu(&g, "baslik") && zorunlu(&g, "etiketler") && !zorunlu(&g, "aciklama") && !zorunlu(&g, "oncelik"), "{g:?}");
    assert_eq!(olustur["govde"][0]["sema"]["dto"], "GorevIstegi");
    let g2 = alanlar(uc(&h, "PUT /app/api/v1/gorevler/:id"), "govde");
    assert!(g2.contains_key("bitti") && !g2.contains_key("tamamlandi"), "@SerialName: {g2:?}");
    assert!(zorunlu(&g2, "baslik") && zorunlu(&g2, "bitti"), "{g2:?}");
    // RequestValidation: doğrulayıcı uygulamanın kodu, önizlemede sınanamaz
    assert!(h["ozet"]["denetlenemeyen"][konum(&h, "POST /app/api/v1/gorevler").as_str()].to_string().contains("RequestValidation"), "{}", h["ozet"]["denetlenemeyen"]);
    // Sorgu: queryParameters["sayfa"]?.toIntOrNull() ?: 1
    let s = alanlar(uc(&h, "GET /app/api/v1/gorevler"), "sorgu");
    assert_eq!((s["sayfa"]["tip"].as_str(), s["sayfa"]["zorunlu"].as_bool()), (Some("int"), Some(false)), "{s:?}");
    // Yanıt: respond(HttpStatusCode.Created, yeni) -> servisin dönüş tipi; liste dizi; doğrudan oluşturulan nesne
    assert_eq!((olustur["yanit"]["sema"]["dto"].as_str(), olustur["yanit"]["durum"].as_u64()), (Some("GorevYaniti"), Some(201)));
    assert_eq!(uc(&h, "GET /app/api/v1/gorevler")["yanit"]["sema"]["dizi"], true);
    assert_eq!(uc(&h, "GET /app/api/v1/gorevler/:id")["yanit"]["sema"]["dto"], "GorevYaniti");
    assert_eq!(uc(&h, "GET /app/api/v1/gorevler/ozet")["yanit"]["sema"]["dto"], "GorevOzeti");
    assert!(uc(&h, "GET /app/saglik")["yanit"].is_null());
    // Denetleyici deseni: işleyici isteği `yorumlar.ekle(this.call)` ile verir; gövde, yol parametresi ve yanıt
    // denetleyicinin `ctx: ApplicationCall` parametresinden okunur
    let yorum = uc(&h, "POST /app/api/v1/gorevler/:id/yorumlar");
    assert_eq!(yorum["govde"][0]["sema"]["dto"], "YorumIstegi");
    assert_eq!(yorum["parametreler"], json!([{ "ad": "id", "tip": "Long" }]));
    assert_eq!((yorum["yanit"]["sema"]["dto"].as_str(), yorum["yanit"]["durum"].as_u64()), (Some("YorumYaniti"), Some(201)));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn exposed_veri_etkisi_kaskad_ve_davranis() {
    let (h, cikti) = harita("veri");
    let veri = |id: &str| (uc(&h, id)["veri"]["okur"].clone(), uc(&h, id)["veri"]["yazar"].clone());
    assert_eq!(veri("GET /app/api/v1/gorevler"), (json!(["Gorevler"]), json!([])));
    assert_eq!(veri("POST /app/api/v1/gorevler"), (json!([]), json!(["Gorevler"])));
    assert_eq!(veri("DELETE /app/api/v1/yonetim/gorevler/:id"), (json!([]), json!(["Gorevler"])));
    let kaskad = |e: &str, c: &str| h["ozet"]["kaskadlar"].as_array().unwrap().iter().any(|k| k[0] == e && k[1] == c);
    assert!(kaskad("Kullanicilar", "Gorevler") && kaskad("Gorevler", "EtiketTable"), "{}", h["ozet"]["kaskadlar"]);
    assert_eq!(dna(&h, "DELETE /app/api/v1/yonetim/gorevler/:id")["dolayli_yazar"][0]["tablo"], "EtiketTable", "{:#}", dna(&h, "DELETE /app/api/v1/yonetim/gorevler/:id"));
    let hata = |d: &Value, kod: u64| d["hatalar"].as_array().unwrap().iter().find(|x| x["durum"] == kod).cloned();
    let d = dna(&h, "GET /app/api/v1/gorevler/:id");
    // NotFoundException (Ktor) ve respond(HttpStatusCode.BadRequest)
    let nf = hata(d, 404).unwrap_or_else(|| panic!("{d:#}"));
    assert!(nf["kosul"].as_str().unwrap().contains("servis.getir"), "{nf}");
    assert!(hata(d, 400).is_some(), "{d:#}");
    // StatusPages: exception<YetkiYok> -> 403 (servis zincirinde)
    assert!(hata(dna(&h, "PUT /app/api/v1/gorevler/:id"), 403).is_some(), "{:#}", dna(&h, "PUT /app/api/v1/gorevler/:id"));
    let depo = json_oku(&cikti.join("kanit.json"));
    let onermeler = depo["onermeler"].as_array().unwrap();
    let var = |v: &str, y: &str, f: &dyn Fn(&Value) -> bool| onermeler.iter().any(|o| o["varlik"] == v && o["yuklem"] == y && f(o));
    assert!(var("db:Gorevler", "TABLO", &|o| o["deger"]["tablo"] == "gorevler"));
    // Exposed'ın varsayılan adı: nesne adı, sondaki "Table" atılır
    assert!(var("db:EtiketTable", "TABLO", &|o| o["deger"]["tablo"] == "Etiket"));
    assert!(var("field:Gorevler.sahip", "VAR", &|o| o["deger"]["sutun"] == "sahip_id"));
    assert!(var("api:POST /app/api/v1/gorevler", "WRITES", &|o| o["hedef"] == "db:Gorevler"));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn doktor_rapor_ve_canli_mod() {
    use p0_g1_ortak::*;
    let ev = Ev::yeni("d8-kotlin-ktor");
    let u = Uygulama::yeni();
    let p = ev.proje(ORNEK, "a");
    let (_, d) = p.cikti(&["doktor"]);
    assert!(d.contains("Ktor (Kotlin)") && d.contains("gorevler"), "{d}");
    let r = p.ok(&["rapor", ".sezgi"]);
    assert!(r.contains("ktor@0.1.0"), "{r}");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["gorevler.*:oku"], &[]);
    let (s, hata) = p.mcp(&t).cagir(json!({ "kapi": "GET /app/api/v1/gorevler/:id", "parametreler": { "id": "7" }, "sorgu": {} }));
    assert!(!hata, "{s}");
    let g = u.gelenler.lock().unwrap();
    assert_eq!(g.last().map(|x| (x.yontem.as_str(), x.yol.as_str())), Some(("GET", "/app/api/v1/gorevler/7")), "kök yolu ve yol parametresi");
}

#[test]
fn init_kotlin_ve_ktor_cati_algilama() {
    // Desteklenmeyen klasörde alt klasördeki Ktor uygulaması aday olarak gösterilir
    let kok = std::env::temp_dir().join(format!("sezgi-d8-kotlin-aday-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    std::fs::create_dir_all(kok.join("sunucu")).unwrap();
    std::fs::write(kok.join("sunucu").join("build.gradle.kts"), "dependencies { implementation(\"io.ktor:ktor-server-core-jvm\") }\n").unwrap();
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap()]).output().unwrap();
    let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(!c.status.success() && m.contains("Ktor") && m.contains("sunucu"), "{m}");
    let _ = std::fs::remove_dir_all(&kok);
}
