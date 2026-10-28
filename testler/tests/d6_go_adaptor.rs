//! Dalga 6b: Go HTTP adaptörü (net/http, Gin, Echo, Chi, Fiber, gorilla/mux).
//! Rotalar (gruplar, alt yönlendiriciler, Mount, StripPrefix, Go 1.22 desenleri, tablo tabanlı kayıt), ara katmandan
//! giriş ve rol kararı (kitaplık doğrulayıcısı kesin, proje ara katmanı çıkarım, sıra kuralları), gövde bağlama ve
//! `binding` / `validate` etiketleri, yanıt tipleri, GORM / SQL veri bağı ve kaskadlar, davranış özeti, kart, OpenAPI,
//! kanıt grafı, rapor, doktor ve canlı mod.
mod p0_g1_ortak;

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

/// Her çağrı kendi çıktı klasörünü alır: paralel testler birbirinin haritasını silmesin
fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d6go-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", ornek(ad).to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "init: {}", String::from_utf8_lossy(&c.stderr));
    cikti
}

fn oku(h: &Path, dosya: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(h.join(dosya)).unwrap()).unwrap()
}

fn idler(m: &Value) -> Vec<String> {
    m["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap().to_string()).collect()
}

fn uc<'a>(m: &'a Value, id: &str) -> &'a Value {
    m["endpointler"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("{id} yok: {:?}", idler(m)))
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

fn karar(e: &Value) -> (String, String) {
    (e["giris"].as_str().unwrap().to_string(), e["giris_kaniti"].as_str().unwrap().to_string())
}

fn kurallar(a: &Value) -> Vec<String> {
    a["kurallar"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect()
}

fn onizle(harita: &Path, arguman: Value) -> String {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", harita.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

#[test]
fn gin_gruplar_ara_katman_ve_cok_paket() {
    let h = haritala("d6_go_gin");
    let m = oku(&h, "kapilar.json");
    assert_eq!(
        idler(&m),
        [
            "GET /api/v1/kitaplar",
            "POST /api/v1/kitaplar",
            "GET /api/v1/kitaplar/:id",
            "PUT /api/v1/kitaplar/:id",
            "DELETE /api/v1/yonetim/kitaplar/:id",
            "GET /api/v1/yorumlar",
            "POST /api/v1/yorumlar/:kitap_id",
            "GET /hata-ayikla",
            "GET /saglik",
            "GET /sahipsiz",
        ],
        "sabit + metin önek, iç içe gruplar, router parametresi alan fonksiyon (başka paket), bağlanmamış router"
    );
    let o = &m["ozet"];
    assert_eq!(o["adaptor"], "go@0.1.0");
    assert_eq!(o["uygulama"], "kitapci", "modül yolunun son parçası");
    assert_eq!(o["ana_uygulama"], "cmd/api/main.go:r");
    assert_eq!(o["kosullu_kapilar"], json!(["GET /hata-ayikla"]), "if gin.Mode() == ... altındaki rota");
    // Giriş: proje ara katmanı 401 ile reddediyor (tanımdan çıkarım); gruplar kayıt anındaki ara katmanları alır
    assert_eq!(karar(uc(&m, "GET /api/v1/kitaplar")), ("herkese_acik".into(), "varsayim".into()), "gin.Logger yetki değildir");
    assert_eq!(karar(uc(&m, "POST /api/v1/kitaplar")), ("gerekli".into(), "varsayim".into()));
    assert_eq!(karar(uc(&m, "POST /api/v1/yorumlar/:kitap_id")), ("gerekli".into(), "varsayim".into()), "rota düzeyi ara katman");
    assert_eq!(karar(uc(&m, "GET /api/v1/yorumlar")), ("herkese_acik".into(), "varsayim".into()));
    let sil = uc(&m, "DELETE /api/v1/yonetim/kitaplar/:id");
    assert_eq!(sil["roller"], json!(["admin", "editor"]), "403 döndüren ara katmanın metin argümanları");
    assert_eq!(sil["roller_kaniti"], "varsayim");
    assert_eq!(sil["kullanimdan_kalkmis"], true, "// Deprecated:");
    // Kontrolcü alıcı tipinden, fonksiyon metot adından; @Summary ve @ID swag açıklamaları
    let l = uc(&m, "GET /api/v1/kitaplar");
    assert_eq!((l["kontrolcu"].as_str().unwrap(), l["fonksiyon"].as_str().unwrap()), ("KitapHandler", "Listele"));
    assert_eq!(l["ozet"], "Kitapları listele");
    assert_eq!(o["islem_kimlikleri"]["internal/handlers/kitap.go:44"], "kitap-listele");
    // Fabrikanın döndürdüğü kapanış: fonksiyon adı fabrikanın adı
    assert_eq!(uc(&m, "GET /api/v1/yorumlar")["fonksiyon"], "yorumlariListele");
    let s = uc(&m, "GET /sahipsiz");
    assert_eq!((s["ana"].as_bool(), s["uygulama"].as_str()), (Some(false), Some("bağlanmamış router (önek bilinmiyor)")));
    // Satır içi işleyici: yöntemle yoldan ad
    assert_eq!(uc(&m, "GET /saglik")["fonksiyon"], "getSaglik");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn gin_govde_binding_etiketleri_ve_yanit() {
    let h = haritala("d6_go_gin");
    let m = oku(&h, "kapilar.json");
    let e = uc(&m, "POST /api/v1/kitaplar");
    assert_eq!(e["govde"][0]["tip"], "KitapGirdisi");
    assert_eq!(e["govde"][0]["sema"]["dosya"], "internal/handlers/kitap.go");
    let g = alanlar(e, "govde");
    assert!(g.get("Gizli").is_none() && g.get("gizli").is_none(), "json:\"-\" alanı gövdede değildir: {g:?}");
    let zorunlu = |a: &str| g[a]["zorunlu"].as_bool().unwrap();
    assert!(zorunlu("baslik") && zorunlu("sayfa") && zorunlu("tur") && zorunlu("yazar_id"));
    assert!(!zorunlu("eposta") && !zorunlu("etiketler") && !zorunlu("yayinci"));
    assert!(g.values().all(|a| a.get("belirsiz").is_none()), "ShouldBindJSON binding etiketlerini doğrular: kesin");
    assert_eq!(kurallar(&g["baslik"]), ["IsString()", "MinLength(2)", "MaxLength(200)"]);
    assert_eq!(kurallar(&g["sayfa"]), ["IsInt()", "Min(1)", "Max(5000)"]);
    assert_eq!(g["tur"]["degerler"], json!(["roman", "siir"]));
    assert!(kurallar(&g["eposta"]).contains(&"IsEmail()".to_string()));
    assert!(kurallar(&g["yazar_id"]).contains(&"Min(0)".to_string()), "uint");
    assert!(kurallar(&g["yayinci"]).contains(&"nullable()".to_string()), "işaretçi");
    assert_eq!(g["yayinci"]["ic"]["ad"]["zorunlu"], true, "iç içe yapı da binding ile doğrulanır");
    // Yanıt: statik tip kesin; omitempty alan her yanıtta yoktur
    let y = &e["yanit"];
    assert_eq!((y["kaynak"].as_str(), y["kesin"].as_bool(), y["durum"].as_u64()), (Some("c.JSON"), Some(true), Some(201)));
    assert_eq!(y["sema"]["dto"], "KitapYaniti");
    assert_eq!((y["sema"]["alanlar"]["id"]["zorunlu"].as_bool(), y["sema"]["alanlar"]["ozet"]["zorunlu"].as_bool()), (Some(true), Some(false)));
    let l = &uc(&m, "GET /api/v1/kitaplar")["yanit"]["sema"];
    assert_eq!((l["dto"].as_str(), l["dizi"].as_bool()), (Some("Kitap"), Some(true)), "[]models.Kitap");
    // gin.H: anahtarlar yorum
    let s = &uc(&m, "GET /saglik")["yanit"];
    assert_eq!(s["kesin"], false);
    assert_eq!(s["sema"]["alanlar"].as_object().unwrap().keys().cloned().collect::<Vec<_>>(), ["durum", "surum"]);
    // Sorgu: DefaultQuery isteğe bağlı (kesin), Query kullanımdan (belirsiz); strconv ile tamsayı
    let q = alanlar(uc(&m, "GET /api/v1/kitaplar"), "sorgu");
    assert_eq!((q["sayfa"]["tip"].as_str(), q["sayfa"]["zorunlu"].as_bool(), q["sayfa"].get("belirsiz")), (Some("int"), Some(false), None));
    assert_eq!(q["q"]["belirsiz"], true);
    let p = &uc(&m, "GET /api/v1/kitaplar/:id")["parametreler"][0];
    assert_eq!((p["ad"].as_str(), p["tip"].as_str()), (Some("id"), Some("int")), "strconv.Atoi(c.Param(\"id\"))");
    // Önizleme binding kurallarını uygular
    let r = onizle(&h, json!({ "kapi": "POST /api/v1/kitaplar", "oturum": true, "govde": { "baslik": "a", "sayfa": 3, "tur": "roman", "yazar_id": 1 } }));
    assert!(r.lines().any(|l| l.starts_with("sonuç: REDDEDİLİR")) && r.contains("baslik"), "MinLength(2) ihlali bildirilmeli:\n{r}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn gin_gorm_veri_kaskad_ve_davranis() {
    let h = haritala("d6_go_gin");
    let m = oku(&h, "kapilar.json");
    let veri = |id: &str| uc(&m, id)["veri"].clone();
    assert_eq!(veri("GET /api/v1/kitaplar"), json!({ "okur": ["Kitap"], "yazar": [] }), "Find(&kitaplar); c.Query sorgu çağrısı değildir");
    assert_eq!(veri("PUT /api/v1/kitaplar/:id"), json!({ "okur": ["Kitap"], "yazar": ["Kitap"] }), "Model(&Kitap{{}}).Updates");
    assert_eq!(veri("DELETE /api/v1/yonetim/kitaplar/:id"), json!({ "okur": [], "yazar": ["Kitap"] }), "yardımcı metot zinciri");
    assert_eq!(veri("POST /api/v1/yorumlar/:kitap_id"), json!({ "okur": [], "yazar": ["Yorum"] }), "ham SQL tablo adı TableName() ile modele eşlenir");
    let o = &m["ozet"];
    assert_eq!(o["kaskadlar"], json!([["Kitap", "Yorum", "internal/models/models.go:17"], ["Yazar", "Kitap", "internal/models/models.go:8"]]));
    assert_eq!(o["veri_semasi"], json!(["internal/models/models.go"]));
    let d = &o["dna"]["internal/handlers/kitap.go:97"];
    assert_eq!(d["silinen"], json!(["Kitap"]));
    assert_eq!(d["yumusak_silme"], "internal/handlers/kitap.go:103", "gorm.Model gömen modelin silinmesi yumuşaktır");
    assert_eq!(d["dolayli_yazar"][0]["tablo"], "Yorum");
    assert!(d["hatalar"].as_array().unwrap().iter().any(|x| x["durum"] == 401 && x["kaynak"] == "bagimlilik" && x["guven"] == "inferred"), "{d}");
    let g = &o["dna"]["internal/handlers/kitap.go:78"];
    assert!(g["sahiplik_denetimi"].as_array().is_some_and(|s| s.iter().any(|x| x["durum"] == 403)), "k.OwnerID != userID: {g}");
    // Tablo modelleri kanıt deposunda: gerçek tablo adı ve sütunları
    let k = oku(&h, "kanit.json");
    let tablo = k["onermeler"].as_array().unwrap().iter().find(|x| x["varlik"] == "db:Yorum" && x["yuklem"] == "TABLO").unwrap();
    assert_eq!(tablo["deger"]["tablo"], "yorumlar");
    assert!(k["onermeler"].as_array().unwrap().iter().any(|x| x["varlik"] == "field:Kitap.Baslik" && x["deger"]["sutun"] == "baslik"));
    assert!(k["onermeler"].as_array().unwrap().iter().any(|x| x["varlik"] == "field:Kitap.CreatedAt" && x["deger"]["sutun"] == "created_at"), "gorm.Model alanları");
    // Router grafı: grup grubun içinde, işlem router'ına bağlı
    assert!(k["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["varlik"] == "mod:cmd/api/main.go:yonetim" && x["yuklem"] == "DEPENDS_ON" && x["hedef"] == "mod:cmd/api/main.go:yetkili"));
    assert!(k["onermeler"].as_array().unwrap().iter().any(|x| x["varlik"] == "api:DELETE /api/v1/yonetim/kitaplar/:id" && x["hedef"] == "mod:cmd/api/main.go:yonetim"));
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn nethttp_go122_desenleri_stripprefix_ve_sarmalayici() {
    let h = haritala("d6_go_nethttp");
    let m = oku(&h, "kapilar.json");
    assert_eq!(
        idler(&m),
        [
            "GET /api/",
            "GET /api/dosyalar/:yol",
            "GET /api/eski/arama",
            "POST /api/eski/arama",
            "POST /api/raporlar",
            "GET /api/urunler",
            "POST /api/urunler",
            "DELETE /api/urunler/:id",
            "GET /api/urunler/:id",
            "GET /saglik",
            "ALL /varsayilan",
        ],
        "yöntem + desen, {{$}}, {{yol...}}, StripPrefix ile bağlanan alt mux, yöntemsiz desende işleyicinin r.Method dalları"
    );
    assert_eq!(m["ozet"]["ana_uygulama"], "main.go:kok");
    assert_eq!(karar(uc(&m, "POST /api/urunler")), ("gerekli".into(), "varsayim".into()), "s.kimlikIste(http.HandlerFunc(...)) sarmalayıcısı");
    assert_eq!(uc(&m, "POST /api/urunler")["guardlar"], json!(["s.kimlikIste"]));
    assert_eq!(uc(&m, "DELETE /api/urunler/:id")["roller"], json!(["yonetici"]), "rol != \"yonetici\" -> 403");
    assert_eq!(karar(uc(&m, "GET /api/urunler")), ("herkese_acik".into(), "varsayim".into()), "gunluk(kok) sarmalayıcısı yetki değildir");
    // Kök mux'a bağlanmayan varsayılan mux ikincil
    let v = uc(&m, "ALL /varsayilan");
    assert_eq!((v["ana"].as_bool(), v["uygulama"].as_str()), (Some(false), Some("main.go:http.DefaultServeMux")));
    // Doğrulama yok: alanların zorunluluğu koddan çıkarılamaz
    let g = alanlar(uc(&m, "POST /api/urunler"), "govde");
    assert!(g.values().all(|a| a["belirsiz"] == true && a["zorunlu"] == false), "{g:?}");
    assert!(kurallar(&g["stok"]).contains(&"nullable()".to_string()));
    // Yanıt: WriteHeader(201) + Encode(urun{...})
    let y = &uc(&m, "POST /api/urunler")["yanit"];
    assert_eq!((y["kaynak"].as_str(), y["durum"].as_u64(), y["sema"]["dto"].as_str()), (Some("json.Encode"), Some(201), Some("urun")));
    assert_eq!(uc(&m, "GET /api/urunler")["yanit"]["sema"]["dizi"], true);
    // Sorgu: q := r.URL.Query(); q.Get("sayfa") + strconv
    assert_eq!(alanlar(uc(&m, "GET /api/urunler"), "sorgu")["sayfa"]["tip"], "int");
    // FormValue: GET'te sorgu, POST'ta form gövdesi
    assert!(alanlar(uc(&m, "GET /api/eski/arama"), "sorgu").contains_key("q"));
    assert!(alanlar(uc(&m, "POST /api/eski/arama"), "govde").contains_key("q"));
    assert_eq!(uc(&m, "GET /api/urunler/:id")["parametreler"][0]["tip"], "int", "strconv.Atoi(r.PathValue(\"id\"))");
    // SQL: JOIN, sabit sorgu, INSERT, DELETE; çalışma anında kurulan sorgu listeyi eksik bırakabilir
    assert_eq!(uc(&m, "GET /api/urunler")["veri"], json!({ "okur": ["kategoriler", "urunler"], "yazar": [] }));
    assert_eq!(uc(&m, "GET /api/urunler/:id")["veri"]["okur"], json!(["urunler"]), "const urunSorgusu");
    assert_eq!(uc(&m, "DELETE /api/urunler/:id")["veri"]["yazar"], json!(["urunler"]));
    assert_eq!(uc(&m, "POST /api/raporlar")["veri"]["eksik_olabilir"], true);
    let d = &m["ozet"]["dna"]["urun.go:83"];
    assert!(d["tetikler"].as_array().unwrap().iter().any(|t| t["tur"] == "job" && t["ad"] == "raporUret"), "go raporUret(...): {d}");
    assert!(m["ozet"]["denetlenemeyen"]["urun.go:83"][0].as_str().unwrap().contains("'ek' alanı"));
    // OpenAPI: {param} biçimi; yöntemsiz kayıt (ALL) belgeye girmez
    let oa = oku(&h, "openapi.json");
    assert!(oa["paths"]["/api/urunler/{id}"]["delete"].is_object());
    assert!(oa["paths"].get("/varsayilan").is_none());
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn echo_kitaplik_jwt_kesin_bind_ve_validate() {
    let h = haritala("d6_go_echo");
    let m = oku(&h, "kapilar.json");
    assert_eq!(idler(&m), ["GET /api/siparisler/:id", "PUT /api/siparisler/:id", "DELETE /api/yonetim/kullanicilar/:id", "GET /kamu/:dosya", "POST /kayit"]);
    assert_eq!(m["ozet"]["uygulama"], "uyelik", "modül yolunun /v2 eki atılır");
    assert_eq!(karar(uc(&m, "GET /api/siparisler/:id")), ("gerekli".into(), "kesin".into()), "echojwt.JWT kitaplık doğrulayıcısı");
    assert_eq!(karar(uc(&m, "POST /kayit")), ("herkese_acik".into(), "varsayim".into()), "Logger / Recover yetki değildir");
    assert_eq!(karar(uc(&m, "GET /kamu/:dosya")), ("herkese_acik".into(), "varsayim".into()), "rota düzeyi ara katman (işleyiciden sonra) 401 vermiyor");
    let y = uc(&m, "DELETE /api/yonetim/kullanicilar/:id");
    assert_eq!((karar(y), y["roller"].clone()), (("gerekli".into(), "kesin".into()), json!(["yonetici"])));
    // Bind + Validate: validate etiketleri uygulanır
    let g = alanlar(uc(&m, "POST /kayit"), "govde");
    assert_eq!((g["eposta"]["zorunlu"].as_bool(), g["sifre"]["zorunlu"].as_bool(), g["rol"]["zorunlu"].as_bool()), (Some(true), Some(true), Some(false)));
    assert!(kurallar(&g["sifre"]).contains(&"MinLength(8)".to_string()));
    assert!(kurallar(&g["yas"]).contains(&"Min(18)".to_string()), "gt=17 tamsayıda Min(18)");
    assert_eq!(g["rol"]["degerler"], json!(["uye", "yonetici"]));
    // Bind: param etiketi yol, query etiketi sorgu, kalanı gövde (yalnızca gövdeli yöntemde); doğrulama yok
    let get = uc(&m, "GET /api/siparisler/:id");
    assert_eq!(get["parametreler"][0]["tip"], "int");
    assert!(alanlar(get, "sorgu").contains_key("durum"));
    assert!(get["govde"].as_array().unwrap().is_empty());
    let put = alanlar(uc(&m, "PUT /api/siparisler/:id"), "govde");
    assert_eq!(put.keys().cloned().collect::<Vec<_>>(), ["not"]);
    assert_eq!(put["not"]["belirsiz"], true, "c.Validate çağrılmadı");
    let r = &uc(&m, "POST /kayit")["yanit"];
    assert_eq!((r["sema"]["dto"].as_str(), r["durum"].as_u64(), r["kesin"].as_bool()), (Some("Kullanici"), Some(201), Some(true)));
    let d = &m["ozet"]["dna"]["main.go:61"];
    assert!(d["hatalar"].as_array().unwrap().iter().any(|x| x["durum"] == 400 && x["istisna"] == "echo.ErrBadRequest"), "{d}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn chi_route_group_with_mount_ve_jwtauth() {
    let h = haritala("d6_go_chi");
    let m = oku(&h, "kapilar.json");
    assert_eq!(
        idler(&m),
        ["GET /", "GET /akis", "GET /makaleler/", "POST /makaleler/", "GET /makaleler/:makaleID/", "PUT /makaleler/:makaleID/", "GET /yonetim/", "DELETE /yonetim/makaleler/:id"]
    );
    assert_eq!(karar(uc(&m, "PUT /makaleler/:makaleID/")), ("gerekli".into(), "kesin".into()), "With(Verifier, Authenticator)");
    assert_eq!(karar(uc(&m, "GET /akis")), ("istege_bagli".into(), "varsayim".into()), "yalnızca Verifier: belirteci okur, reddetmez");
    assert_eq!(karar(uc(&m, "GET /makaleler/:makaleID/")), ("herkese_acik".into(), "varsayim".into()));
    let y = uc(&m, "DELETE /yonetim/makaleler/:id");
    assert_eq!((karar(y), y["roller"].clone()), (("gerekli".into(), "varsayim".into()), json!(["yonetici"])), "Mount edilen router'ın ara katmanı");
    assert_eq!(uc(&m, "GET /makaleler/:makaleID/")["parametreler"][0]["tip"], "int", "{{makaleID:[0-9]+}}");
    assert_eq!(uc(&m, "GET /makaleler/")["veri"], json!({ "okur": ["makaleler"], "yazar": [] }), "sqlx Select; depo metodu zinciri");
    assert_eq!(uc(&m, "DELETE /yonetim/makaleler/:id")["veri"]["yazar"], json!(["makaleler", "yorumlar"]));
    let r = &uc(&m, "GET /makaleler/:makaleID/")["yanit"];
    assert_eq!((r["kaynak"].as_str(), r["sema"]["dto"].as_str()), (Some("render.JSON"), Some("Makale")));
    assert_eq!(r["sema"]["alanlar"]["govde"]["zorunlu"], false, "omitempty");
    assert!(alanlar(uc(&m, "POST /makaleler/"), "govde").contains_key("baslik"), "render.DecodeJSON");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn fiber_sira_kurali_kisit_ve_dogrulama() {
    let h = haritala("d6_go_fiber");
    let m = oku(&h, "kapilar.json");
    assert_eq!(idler(&m), ["POST /api/v2/giris", "POST /api/v2/notlar/", "DELETE /api/v2/notlar/:id", "GET /api/v2/notlar/:id", "GET /api/v2/profil", "GET /durum"]);
    // Fiber ara katmanı kayıt sırasıyla uygular: Use(jwtware)'den önce kaydedilen giriş ucu açık
    assert_eq!(karar(uc(&m, "POST /api/v2/giris")), ("herkese_acik".into(), "varsayim".into()));
    assert_eq!(karar(uc(&m, "GET /api/v2/profil")), ("gerekli".into(), "kesin".into()));
    assert_eq!(karar(uc(&m, "GET /api/v2/notlar/:id")), ("gerekli".into(), "kesin".into()), "sonradan kurulan grup");
    assert_eq!(uc(&m, "GET /api/v2/notlar/:id")["parametreler"][0], json!({ "ad": "id", "tip": "int" }), ":id<int>");
    let g = alanlar(uc(&m, "POST /api/v2/notlar/"), "govde");
    assert_eq!((g["baslik"]["zorunlu"].as_bool(), g["icerik"]["zorunlu"].as_bool()), (Some(true), Some(false)), "BodyParser + validator.Struct");
    assert!(kurallar(&g["etiketler"]).contains(&"ArrayMaxSize(5)".to_string()));
    assert_eq!(alanlar(uc(&m, "POST /api/v2/notlar/"), "sorgu")["etiket"]["belirsiz"], Value::Null, "c.Query(ad, varsayılan) isteğe bağlı, kesin");
    let r = &uc(&m, "POST /api/v2/notlar/")["yanit"];
    assert_eq!((r["sema"]["dto"].as_str(), r["durum"].as_u64()), (Some("Not"), Some(201)), "c.Status(201).JSON(...)");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn gorilla_subrouter_methods_tablo_ve_dinamik_kayit() {
    let h = haritala("d6_go_mux");
    let m = oku(&h, "kapilar.json");
    assert_eq!(
        idler(&m),
        [
            "GET /api/v1/gorevler",
            "POST /api/v1/gorevler",
            "DELETE /api/v1/gorevler/:id",
            "GET /api/v1/gorevler/:id",
            "POST /geri-bildirim",
            "GET /saglik",
            "GET /surum",
            "ALL /yetim"
        ],
        "PathPrefix().Subrouter(), .Methods(), Methods().Path().HandlerFunc(), tablo tabanlı rota listesi"
    );
    assert_eq!(karar(uc(&m, "DELETE /api/v1/gorevler/:id")), ("gerekli".into(), "varsayim".into()), "alt router'ın Use'u");
    assert_eq!(karar(uc(&m, "GET /saglik")), ("herkese_acik".into(), "varsayim".into()), "handlers.LoggingHandler sarmalaması yetki değildir");
    assert_eq!(uc(&m, "GET /api/v1/gorevler/:id")["parametreler"][0]["tip"], "int", "{{id:[0-9]+}}");
    assert_eq!(uc(&m, "ALL /yetim")["ana"], false);
    assert_eq!(m["ozet"]["dinamik_kayitlar"], json!(["main.go:33 os.Getenv(\"EK_YOL\")"]), "yolu çalışma anında kurulan kayıt uydurulmaz");
    let r = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["rapor", h.to_str().unwrap()]).output().unwrap();
    let r = String::from_utf8_lossy(&r.stdout);
    assert!(r.contains("DİNAMİK") && r.contains("os.Getenv"), "{r}");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn kart_kanit_ve_doktor() {
    let h = haritala("d6_go_gin");
    let kart = oku(&h, "benlik.json");
    let ops: Vec<&str> = kart["operations"].as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap()).collect();
    assert!(ops.contains(&"kitap-handler.getir") && ops.contains(&"yorum.yorum-ekle"), "{ops:?}");
    assert!(!ops.iter().any(|o| o.contains("sahipsiz")), "ikincil uç karta girmez");
    assert_eq!(kart["generator"]["adapters"], json!(["go@0.1.0"]));
    let k = oku(&h, "kanit.json");
    let giris = k["onermeler"].as_array().unwrap().iter().find(|x| x["varlik"] == "api:POST /api/v1/kitaplar" && x["yuklem"] == "GIRIS").unwrap();
    assert_eq!(giris["hesap"]["guven"], "inferred", "proje ara katmanının kararı çıkarımdır");
    let _ = std::fs::remove_dir_all(&h);
    let e = haritala("d6_go_echo");
    let k = oku(&e, "kanit.json");
    let giris = k["onermeler"].as_array().unwrap().iter().find(|x| x["varlik"] == "api:GET /api/siparisler/:id" && x["yuklem"] == "GIRIS").unwrap();
    assert_eq!(giris["hesap"]["guven"], "certain", "kitaplık doğrulayıcısı kesin");
    let _ = std::fs::remove_dir_all(&e);
    let d = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["doktor", ornek("d6_go_fiber").to_str().unwrap()]).output().unwrap();
    let d = String::from_utf8_lossy(&d.stdout);
    assert!(d.contains("Fiber (Go, go.mod)") && d.contains("notlar"), "{d}");
}

#[test]
fn canli_mod_yol_parametresi() {
    use p0_g1_ortak::*;
    let ev = Ev::yeni("d6-go");
    let u = Uygulama::yeni();
    let p = ev.proje("d6_go_gin", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap-handler.*:oku"], &[]);
    let (s, hata) = p.mcp(&t).cagir(json!({ "kapi": "GET /api/v1/kitaplar/:id", "parametreler": { "id": 42 } }));
    assert!(!hata, "{s}");
    let g = u.gelenler.lock().unwrap();
    assert_eq!(g.last().map(|x| (x.yontem.as_str(), x.yol.as_str())), Some(("GET", "/api/v1/kitaplar/42")));
}

/// Kesinlik değişmezleri Go örneklerinde de geçerli (kesinlik.rs yalnızca package.json / pyproject.toml olan örnekleri
/// gezer): kartta "certain" işlemin giriş kararı ve bütün girdileri kesin; haritada belirsiz alan kartta certain olamaz
#[test]
fn go_orneklerinde_kesinlik_degismezleri() {
    let mut ihlaller = vec![];
    for ad in ["d6_go_gin", "d6_go_nethttp", "d6_go_echo", "d6_go_chi", "d6_go_fiber", "d6_go_mux"] {
        let h = haritala(ad);
        let (m, kart) = (oku(&h, "kapilar.json"), oku(&h, "benlik.json"));
        let kapilar = m["endpointler"].as_array().unwrap();
        for op in kart["operations"].as_array().unwrap() {
            let (yontem, yol) = (op["entry"]["method"].as_str().unwrap(), op["entry"]["path"].as_str().unwrap());
            let Some(k) = kapilar.iter().find(|k| k["yontem"] == yontem && k["yol"] == yol && k["ana"] != false) else {
                ihlaller.push(format!("{ad}: kartta {yontem} {yol} var, haritada yok"));
                continue;
            };
            for g in k["govde"].as_array().into_iter().chain(k["sorgu"].as_array()).flatten() {
                for (alan, a) in g["sema"]["alanlar"].as_object().into_iter().flatten() {
                    if a["belirsiz"] == true && op["inputs"][alan]["stamp"] == "certain" {
                        ihlaller.push(format!("{ad}: {yontem} {yol} {alan} haritada belirsiz, kartta certain"));
                    }
                }
            }
            if op["stamp"] != "certain" {
                continue;
            }
            if k["giris_kaniti"] != "kesin" {
                ihlaller.push(format!("{ad}: işlem {} certain ama giriş kararı {}", op["id"], k["giris_kaniti"]));
            }
            for (g, v) in op["inputs"].as_object().unwrap() {
                if v["stamp"] != "certain" {
                    ihlaller.push(format!("{ad}: işlem {} certain ama girdi {g} {}", op["id"], v["stamp"]));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&h);
    }
    assert!(ihlaller.is_empty(), "{}", ihlaller.join("\n"));
}
