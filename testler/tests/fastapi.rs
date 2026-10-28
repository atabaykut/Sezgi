//! Python + FastAPI: rotalar ve önekler, bağımlılık zincirinden giriş kararı (kesinlik ilkesi), Pydantic alanları,
//! Query / Header / Form ayrımı. Kart, OpenAPI ve önizleme dilden bağımsız çalışır.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn harita(ad: &str) -> (serde_json::Value, PathBuf) {
    let cikti = std::env::temp_dir().join(format!("sezgi-fastapi-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("fastapi-temel");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    (serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap(), cikti)
}

fn uc<'a>(h: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    h["endpointler"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("{id} yok"))
}

fn alanlar(e: &serde_json::Value, bolum: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    for g in e[bolum].as_array().unwrap() {
        if let Some(a) = g["sema"]["alanlar"].as_object() {
            m.extend(a.clone());
        }
    }
    m
}

#[test]
fn rotalar_ve_onekler() {
    let (h, cikti) = harita("rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    let beklenen = [
        "DELETE /api/fabrika/:ad",
        "POST /api/fabrika/:ad",
        "PUT /api/fabrika/:ad",
        "POST /api/fabrika/deneme",
        "GET /api/fabrika/durum",
        "GET /api/fabrika/rapor",
        "POST /api/gizli/kullanici",
        "GET /api/kitaplar/",
        "POST /api/kitaplar/",
        "DELETE /api/kitaplar/:kitap_id",
        "GET /api/kitaplar/:kitap_id",
        "PUT /api/kitaplar/:kitap_id/kapak",
        "GET /api/kitaplar/dis/kontrol",
        "GET /api/raflar/",
        "POST /api/raflar/",
        "PATCH /api/raflar/:ad",
        "POST /api/raflar/:raf_id/odunc",
        "GET /api/raflar/rapor",
        "DELETE /api/raflar/secili",
        "GET /api/yonetim/rapor",
        "POST /api/yonetim/temizle",
        "GET /v2/saglik",
    ];
    assert_eq!(idler, beklenen, "uygulama öneki (settings.API_PREFIX), router önekleri, mount ve add_api_route");
    assert_eq!(h["ozet"]["adaptor"], "fastapi@0.1.0");
    assert_eq!(h["ozet"]["uygulama"], "kitapci");
    assert!(h["ozet"]["kosullu_kapilar"].as_array().unwrap().iter().any(|k| k == "POST /api/gizli/kullanici"), "{}", h["ozet"]);
    // Kart ve OpenAPI dilden bağımsız üretilir
    let kart: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("benlik.json")).unwrap()).unwrap();
    assert_eq!(kart["app"]["version"], "1.2.0");
    assert!(kart["operations"].as_array().unwrap().iter().any(|o| o["id"] == "kitaplar.olustur"));
    let oa: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("openapi.json")).unwrap()).unwrap();
    assert!(oa["paths"]["/api/kitaplar/{kitap_id}"]["delete"].is_object());
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn giris_bagimlilik_zincirinden() {
    let (h, cikti) = harita("giris");
    let karar = |id: &str| (uc(&h, id)["giris"].as_str().unwrap().to_string(), uc(&h, id)["giris_kaniti"].as_str().unwrap().to_string());
    let beklenen: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        // CurrentUser = Annotated[User, Depends(get_current_user)] -> Depends(oauth2) -> OAuth2PasswordBearer
        ("POST /api/kitaplar/", ("gerekli", "kesin")),
        // OAuth2PasswordBearer(auto_error=False): şema hata vermez; girişin isteğe bağlı olduğu tanımdan kanıtlanamaz
        // (hatayı başka bir fonksiyon verebilir, Airflow: kanit/SONUC-15), karar tahmindir
        ("GET /api/kitaplar/:kitap_id", ("istege_bagli", "varsayim")),
        // class JWTBearer(HTTPBearer), Depends(JWTBearer())
        ("DELETE /api/kitaplar/:kitap_id", ("gerekli", "kesin")),
        // router düzeyinde dependencies=[Depends(get_current_user)], add_api_route dahil
        ("GET /api/yonetim/rapor", ("gerekli", "kesin")),
        ("POST /api/yonetim/temizle", ("gerekli", "kesin")),
        // Güvenlik şeması yok: açık sayılır ama kesin değil (ara katman kimlik isteyebilir)
        ("GET /api/kitaplar/", ("herkese_acik", "varsayim")),
        // İzlenemeyen dış bağımlılık: kesin "açık" denemez
        ("GET /api/kitaplar/dis/kontrol", ("herkese_acik", "varsayim_bilinmeyen_guard")),
    ]);
    for (id, (g, k)) in beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string()), "{id}");
    }
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn pydantic_alanlari_ve_parametreler() {
    let (h, cikti) = harita("alan");
    let g = alanlar(uc(&h, "POST /api/kitaplar/"), "govde");
    let zorunlu = |a: &str| g[a]["zorunlu"].as_bool().unwrap();
    // Taban sınıftan gelenler dahil; varsayılanı olmayan alan zorunlu
    assert!(zorunlu("baslik") && zorunlu("sayfa") && zorunlu("tur") && zorunlu("ISBN"));
    assert!(!zorunlu("yazar") && !zorunlu("dil") && !zorunlu("iletisim"));
    assert!(g.get("isbn").is_none(), "alias kullanılır");
    let kural = |a: &str| g[a]["kurallar"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert!(kural("baslik").contains(&"MinLength(2)".to_string()) && kural("baslik").contains(&"MaxLength(200)".to_string()), "{:?}", kural("baslik"));
    assert!(kural("sayfa").contains(&"Min(1)".to_string()) && kural("sayfa").contains(&"Max(5000)".to_string()));
    assert!(kural("iletisim").contains(&"IsEmail()".to_string()));
    assert_eq!(g["tur"]["degerler"], serde_json::json!(["roman", "siir"]), "Literal");
    assert_eq!(g["dil"]["degerler"], serde_json::json!(["tr", "en"]), "Enum");
    // Sorgu: Annotated[..., Query(max_length=50)] ve basit tip; Header sorgu değildir; Form gövdedir
    let s = alanlar(uc(&h, "GET /api/kitaplar/"), "sorgu");
    assert!(!s["q"]["zorunlu"].as_bool().unwrap() && s["q"]["kurallar"].as_array().unwrap().iter().any(|r| r == "MaxLength(50)"));
    assert!(!s["sayfa"]["zorunlu"].as_bool().unwrap());
    let kapak = uc(&h, "PUT /api/kitaplar/:kitap_id/kapak");
    assert!(alanlar(kapak, "sorgu").is_empty(), "x_istek bir başlıktır");
    assert!(alanlar(kapak, "govde")["aciklama"]["zorunlu"].as_bool().unwrap());
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn onizle_python_kurallariyla() {
    let (_, cikti) = harita("onizle");
    let mut p =
        Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", cikti.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    use std::io::{BufRead, Write};
    let istek = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": {
        "kapi": "POST /api/kitaplar/", "oturum": true, "govde": { "baslik": "X", "sayfa": 0, "tur": "deneme", "ISBN": "1" } } } });
    writeln!(p.stdin.as_mut().unwrap(), "{istek}").unwrap();
    let mut satir = String::new();
    std::io::BufReader::new(p.stdout.take().unwrap()).read_line(&mut satir).unwrap();
    let _ = p.kill();
    let _ = p.wait();
    let v: serde_json::Value = serde_json::from_str(&satir).unwrap();
    let t = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(t.contains("REDDEDİLİR"), "{t}");
    assert!(t.contains("baslik") && t.contains("sayfa") && t.contains("tur"), "kısa başlık, alt sınırın altındaki sayfa ve izinsiz tür: {t}");
    let _ = std::fs::remove_dir_all(&cikti);
}

/// Prefect holdout'unda bulunan kalıplar (kanit/SONUC-10): fabrika fonksiyonu içinde kurulan uygulama, router'ların
/// listeden döngüyle dahil edilmesi, alt modül erişimi, modül yoluyla yazılmış gövde tipi; aynı depodaki küçük
/// yardımcı uygulama ana uygulama sanılmamalı.
#[test]
fn fabrika_uygulamasi_ve_modul_yollu_tipler() {
    let cikti = std::env::temp_dir().join(format!("sezgi-fastapi-fabrika-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("fastapi-fabrika");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    let h: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap();
    assert_eq!(h["ozet"]["ana_uygulama"], "sunucu.uygulama:api_uygulamasi_kur().api_app", "fabrika içindeki uygulama ana");
    for id in ["POST /siparisler/", "GET /saglik/"] {
        assert_eq!(uc(&h, id)["ana"], true, "{id} ana uygulamaya ait (döngüyle dahil edilen router)");
    }
    assert_eq!(uc(&h, "POST /basarili")["ana"], false, "yardımcı uygulama ikincil");
    let g = alanlar(uc(&h, "POST /siparisler/"), "govde");
    assert!(g["urun"]["zorunlu"].as_bool().unwrap() && !g["adet"]["zorunlu"].as_bool().unwrap(), "modül yoluyla yazılmış tip çözülmeli: {g:?}");
    // Prefect'te bulunan iki kalıp: varsayılanı `Depends(iç fonksiyon)` döndüren fabrika (gövdede offset) ve yolu
    // fabrikanın parametre varsayılanından gelen rota
    assert!(alanlar(uc(&h, "POST /siparisler/kaydir"), "govde").contains_key("offset"), "{}", uc(&h, "POST /siparisler/kaydir"));
    assert_eq!(uc(&h, "GET /canli")["ana"], true);
    let _ = std::fs::remove_dir_all(&cikti);
}

/// FastAPI kuralları: birden fazla gövde parametresi gövdeyi parametre adlarıyla gömer; Field(default=...) zorunludur
#[test]
fn gomulu_govde_ve_ellipsis() {
    let cikti = std::env::temp_dir().join(format!("sezgi-fastapi-gomulu-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("fastapi-fabrika");
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap().status.success());
    let h: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap();
    let g = alanlar(uc(&h, "POST /siparisler/filtre"), "govde");
    let mut adlar: Vec<&String> = g.keys().collect();
    adlar.sort();
    assert_eq!(adlar, ["sayfa", "siparisler"], "gövde parametre adlarıyla gömülür: {g:?}");
    assert!(!g["siparisler"]["zorunlu"].as_bool().unwrap() && !g["sayfa"]["zorunlu"].as_bool().unwrap());
    assert!(g["sayfa"]["kurallar"].as_array().unwrap().iter().any(|r| r == "Min(0)"), "{:?}", g["sayfa"]);
    let o = alanlar(uc(&h, "POST /siparisler/"), "govde");
    assert!(o["kod"]["zorunlu"].as_bool().unwrap(), "Field(default=...) zorunlu");
    assert!(!o["adet"]["zorunlu"].as_bool().unwrap());
    let _ = std::fs::remove_dir_all(&cikti);
}

/// Varsayılanı Body(...) döndüren projedeki bir yardımcı fonksiyonsa parametre gövdededir (ör. Prefect LimitBody())
#[test]
fn yardimci_fonksiyondan_isaret() {
    let cikti = std::env::temp_dir().join(format!("sezgi-fastapi-yardimci-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("fastapi-fabrika");
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap().status.success());
    let h: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap();
    let e = uc(&h, "POST /siparisler/sayfa");
    let g = alanlar(e, "govde");
    assert!(g.contains_key("limit") && alanlar(e, "sorgu").is_empty(), "limit gövdede: {g:?}");
    let k: Vec<&str> = g["limit"]["kurallar"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert!(k.contains(&"Min(1)") && k.contains(&"Max(200)") && !g["limit"]["zorunlu"].as_bool().unwrap(), "{k:?}");
    let _ = std::fs::remove_dir_all(&cikti);
}

/// Bağımlılık fonksiyonunun kendi parametreleri (Query, Body...) uç noktanın girdilerine katılır (FastAPI kuralı)
#[test]
fn bagimliligin_parametreleri_girdidir() {
    let cikti = std::env::temp_dir().join(format!("sezgi-fastapi-altbag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("fastapi-fabrika");
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap().status.success());
    let h: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap();
    let s = alanlar(uc(&h, "GET /siparisler/liste"), "sorgu");
    let mut adlar: Vec<&String> = s.keys().collect();
    adlar.sort();
    assert_eq!(adlar, ["limit", "skip"], "{s:?}");
    assert!(s["limit"]["kurallar"].as_array().unwrap().iter().any(|r| r == "Max(500)"));
    let _ = std::fs::remove_dir_all(&cikti);
}

/// GPU israfı (vizyon 10): hafif seçenekler ve kaldığı yerden devam ucu kartta görünür; addan çıkarıldığı söylenir
#[test]
fn hesap_ipuclari_tani_kartinda() {
    let cikti = std::env::temp_dir().join(format!("sezgi-fastapi-ipucu-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("fastapi-fabrika");
    assert!(Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap().status.success());
    let mut p =
        Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", cikti.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    use std::io::{BufRead, Write};
    let istek = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "tani", "arguments": { "kapi": "POST /gorseller/" } } });
    writeln!(p.stdin.as_mut().unwrap(), "{istek}").unwrap();
    let mut satir = String::new();
    std::io::BufReader::new(p.stdout.take().unwrap()).read_line(&mut satir).unwrap();
    let _ = p.kill();
    let _ = p.wait();
    let v: serde_json::Value = serde_json::from_str(&satir).unwrap();
    let t = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(t.contains("Hesap ipuçları") && t.contains("hizli") && t.contains("kalite"), "{t}");
    assert!(t.contains("POST /gorseller/:is_id/devam"), "kaldığı yerden devam ucu: {t}");
    assert!(t.contains("addan"), "çıkarım olduğu söylenmeli: {t}");
    // Ad parçalarıyla (Fooocus-API ölçümünden, kanit/SONUC-14): çok sözcüklü hafif seçenek ve üretilen şeyin sayısı
    assert!(t.contains("`performance_selection` alanında hafif seçenek: Speed, Extreme Speed"), "{t}");
    assert!(t.contains("`image_number` hesabı"), "{t}");
    assert!(!t.contains("`page_size`") && !t.contains("`seed`"), "sayfalama ve tohum hesabı belirlemez: {t}");
    let _ = std::fs::remove_dir_all(&cikti);
}

/// Veri bağları (SQLModel): tablo sınıfları (table=True), select / get okuma, add / delete yazma; değişkenin modeli
/// parametre tipinden, takma addan (Annotated[Raf, ...]) ya da çağrılan fonksiyonun dönüş tipinden; proje içi
/// çağrılar `return` içinde de izlenir; ham SQL listeyi eksik işaretler. Tablo olmayan şema (RafGirdisi) sayılmaz.
#[test]
fn veri_baglari_sqlmodel() {
    let (h, cikti) = harita("veri");
    let _ = std::fs::remove_dir_all(&cikti);
    let v = |id: &str| uc(&h, id)["veri"].clone();
    assert_eq!(v("GET /api/raflar/"), serde_json::json!({ "okur": ["Raf"], "yazar": [] }), "return içindeki crud çağrısı");
    assert_eq!(v("POST /api/raflar/"), serde_json::json!({ "okur": [], "yazar": ["Raf"] }), "Raf.model_validate + add");
    assert_eq!(v("PATCH /api/raflar/:ad"), serde_json::json!({ "okur": ["Raf"], "yazar": ["Raf"] }), "dönüş tipi -> Raf | None");
    assert_eq!(v("DELETE /api/raflar/secili")["yazar"], serde_json::json!(["Raf"]), "takma ad Annotated[Raf, ...]");
    assert_eq!(v("POST /api/raflar/:raf_id/odunc")["yazar"], serde_json::json!(["Odunc"]), "koşul içinde dönen çağrı");
    assert_eq!(v("GET /api/raflar/rapor")["eksik_olabilir"], true, "ham SQL");
    // Tabloya dokunmayan uç: boş liste (null değil)
    assert_eq!(v("GET /api/kitaplar/")["okur"], serde_json::json!([]));
}

/// Fabrika içinde kaydedilen rotalar: `def kur(): router = APIRouter(prefix=...); @router.get(...)`;
/// `include_router(kur())` fabrikanın döndürdüğü router'a bağlanır
#[test]
fn fabrika_icindeki_rotalar() {
    let (h, cikti) = harita("ic-rota");
    let _ = std::fs::remove_dir_all(&cikti);
    let ekle = uc(&h, "POST /api/fabrika/:ad");
    assert_eq!(ekle["fonksiyon"], "ekle");
    assert!(alanlar(ekle, "sorgu").contains_key("adet"), "{ekle}");
    assert_eq!(uc(&h, "GET /api/fabrika/durum")["fonksiyon"], "durum");
    // Bağımlılık fabrikası ve __call__'lu sınıf: zincir döndürülen fonksiyondan güvenlik şemasına ulaşır (kesin);
    // roller fabrikanın metin argümanlarından, 403 döndüren denetimde (Yorum)
    let sil = uc(&h, "DELETE /api/fabrika/:ad");
    assert_eq!((sil["giris"].as_str(), sil["giris_kaniti"].as_str()), (Some("gerekli"), Some("kesin")), "{sil}");
    assert_eq!(sil["roller"], serde_json::json!(["admin"]));
    assert_eq!(sil["roller_kaniti"], "varsayim");
    let guncelle = uc(&h, "PUT /api/fabrika/:ad");
    assert_eq!(guncelle["giris"], "gerekli");
    assert_eq!(guncelle["roller"], serde_json::json!(["admin", "editor"]));
    // Güvenlik şemasına ulaşmayan fabrika: açık ve tanınmayan guard sayılmaz
    let durum = uc(&h, "GET /api/fabrika/durum");
    assert_eq!((durum["giris"].as_str(), durum["giris_kaniti"].as_str()), (Some("herkese_acik"), Some("varsayim")), "{durum}");
    // FastAPI alt bağımlılığın parametresini ucun sorgusuna açar (fabrikanın döndürdüğü fonksiyon dahil)
    assert!(alanlar(durum, "sorgu").contains_key("boyut"), "{durum}");
    // İki kat iç içe tanımlanan rota dış fabrikanın router'ını bulur; koşul altında kaydedilen rota koşullu kapıdır
    assert_eq!(uc(&h, "GET /api/fabrika/rapor")["fonksiyon"], "rapor");
    assert!(h["ozet"]["kosullu_kapilar"].as_array().unwrap().iter().any(|k| k == "POST /api/fabrika/deneme"), "{}", h["ozet"]);
}

/// Airflow holdout'unun bulduğu kalıplar (kanit/SONUC-15): uygulama router'ları yardımcı fonksiyonda ve fonksiyon içi
/// içe aktarmayla bağlar (`gorunumleri_kur(app)`); güvenlik şemaları `auto_error=False`, 401'i şemayı kullanan
/// fonksiyonun çağırdığı yardımcı fırlatır. `auto_error=False` girişin isteğe bağlı olduğunu kanıtlamaz: karar tahmindir.
#[test]
fn yardimci_fonksiyonla_baglama_ve_auto_error_false() {
    let cikti = std::env::temp_dir().join(format!("sezgi-fastapi-baglama-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("fastapi-yardimci");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    let h: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&cikti);
    // Önek zinciri: uygulama -> (yardımcı fonksiyon) genel_router /api/v2 -> girisli_router -> isler_router /isler
    let sil = uc(&h, "DELETE /api/v2/isler/:is_id");
    assert_eq!(sil["ana"], true, "{sil}");
    // 401'i çağrılan yardımcı fırlatıyor: giriş gerekli, ama tanımdan kanıtlanmadığı için tahmin
    assert_eq!(sil["giris"], "gerekli", "{sil}");
    assert_ne!(sil["giris_kaniti"], "kesin", "auto_error=False zincirinde karar kesin olamaz: {sil}");
    assert_eq!(uc(&h, "GET /api/v2/isler")["giris"], "gerekli");
    // Metin toplamayla kurulan yol
    assert_eq!(uc(&h, "GET /api/v2/isler/:is_id/denemeler/:deneme_no")["fonksiyon"], "denemeyiGetir");
    // Hata fırlatmayan isteğe bağlı kullanıcı: isteğe bağlı, yine tahmin
    let vitrin = uc(&h, "GET /api/v2/vitrin");
    assert_eq!(vitrin["giris"], "istege_bagli", "{vitrin}");
    assert_ne!(vitrin["giris_kaniti"], "kesin", "{vitrin}");
}
