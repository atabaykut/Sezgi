//! Python + Django (Django REST Framework dahil): ROOT_URLCONF'tan include zinciri, path dönüştürücüleri ve re_path,
//! DRF router'ları ve @action, sınıf ve fonksiyon görünümlerinin yöntemleri, DRF izinleri ve Django giriş işaretleri
//! (kesinlik ilkesi), serializer / form şemaları, ORM veri bağları, on_delete kaskadları, sinyaller ve göç dosyaları.
//! Kart, OpenAPI, kanıt grafiği, önizleme ve canlı mod dilden bağımsız çalışır.
mod p0_g1_ortak;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn harita(ad: &str) -> (serde_json::Value, PathBuf) {
    let cikti = std::env::temp_dir().join(format!("sezgi-d6-django-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("d6_python_django");
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

fn kurallar(a: &serde_json::Value) -> Vec<String> {
    a["kurallar"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect()
}

#[test]
fn rotalar_include_router_ve_kosullu_kayit() {
    let (h, cikti) = harita("rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    let beklenen = [
        // DefaultRouter'ın kök görünümü
        "GET /api/",
        "GET /api/kitaplar/",
        "POST /api/kitaplar/",
        "DELETE /api/kitaplar/:pk/",
        "GET /api/kitaplar/:pk/",
        "PATCH /api/kitaplar/:pk/",
        "PUT /api/kitaplar/:pk/",
        // @action(detail=True, url_path="yorum-ekle") ve @action(detail=False)
        "POST /api/kitaplar/:pk/yorum-ekle/",
        "GET /api/kitaplar/populer/",
        "GET /api/v1/ara/",
        "POST /api/v1/ara/",
        "GET /api/v1/kitaplar/:kitap_id/yorumlar/",
        "POST /api/v1/kitaplar/:kitap_id/yorumlar/",
        "GET /api/v1/kitaplar/:pk/dinamik/",
        "POST /api/v1/kitaplar/:pk/yayinla/",
        "GET /api/v1/toplu-sil/",
        "POST /api/v1/toplu-sil/",
        // ReadOnlyModelViewSet: yalnızca liste ve ayrıntı
        "GET /api/yazarlar/",
        "GET /api/yazarlar/:pk/",
        // if settings.DEBUG: urlpatterns += [...]
        "GET /debug/saglik/",
        // re_path(r"^eski/(?P<yil>[0-9]{4})/$") ve @require_http_methods(["GET", "POST"])
        "GET /eski/:yil/",
        "POST /eski/:yil/",
        // http_method_names = ["get"]: tanımlı post süzülür
        "GET /hesap/ayarlar/",
        "GET /hesap/hakkinda/",
        // FormView: GET, POST, PUT
        "GET /hesap/kayit/",
        "POST /hesap/kayit/",
        "PUT /hesap/kayit/",
        "GET /hesap/profil/",
        "POST /hesap/profil/",
    ];
    assert_eq!(idler, beklenen, "admin.site.urls haritaya alınmaz");
    assert_eq!(h["ozet"]["adaptor"], "django@0.1.0");
    assert_eq!(h["ozet"]["uygulama"], "kitaplik");
    assert_eq!(h["ozet"]["ana_uygulama"], "kitaplik.urls", "ayar modülü manage.py'deki DJANGO_SETTINGS_MODULE, ROOT_URLCONF yıldızlı içe aktarmadan");
    assert_eq!(h["ozet"]["kosullu_kapilar"], serde_json::json!(["GET /debug/saglik/"]));
    // Yol parametresinin tipi dönüştürücüden
    let p = &uc(&h, "POST /api/v1/kitaplar/:pk/yayinla/")["parametreler"][0];
    assert_eq!((p["ad"].as_str(), p["tip"].as_str()), (Some("pk"), Some("int")));
    // İşlem adları: ViewSet eylemi ve fonksiyon görünümünün uygulaması
    let ad = |id: &str| format!("{}.{}", uc(&h, id)["kontrolcu"].as_str().unwrap(), uc(&h, id)["fonksiyon"].as_str().unwrap());
    assert_eq!(ad("PATCH /api/kitaplar/:pk/"), "Kitap.partialUpdate");
    assert_eq!(ad("GET /debug/saglik/"), "Kitaplar.saglik");
    // Kart ve OpenAPI dilden bağımsız
    let kart: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("benlik.json")).unwrap()).unwrap();
    assert_eq!(kart["app"]["version"], "2.1.0");
    assert!(kart["operations"].as_array().unwrap().iter().any(|o| o["id"] == "kitap.partial-update"), "{}", kart["operations"]);
    let oa: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("openapi.json")).unwrap()).unwrap();
    assert!(oa["paths"]["/api/kitaplar/{pk}/"]["delete"].is_object());
    // Haritaya alınmayan kayıt (yönetim paneli) kapsam raporunda nedeniyle
    let r = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["rapor", cikti.to_str().unwrap()]).output().unwrap();
    let m = String::from_utf8_lossy(&r.stdout);
    assert!(m.contains("HARİTASIZ") && m.contains("admin"), "{m}");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn giris_ve_rol_kararlari() {
    let (h, cikti) = harita("giris");
    let karar = |id: &str| (uc(&h, id)["giris"].as_str().unwrap().to_string(), uc(&h, id)["giris_kaniti"].as_str().unwrap().to_string());
    let beklenen: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        // IsAuthenticatedOrReadOnly: okuma açık, yazma giriş ister; ikisi de tanımdan kesin
        ("GET /api/kitaplar/", ("herkese_acik", "kesin")),
        ("POST /api/kitaplar/", ("gerekli", "kesin")),
        ("DELETE /api/kitaplar/:pk/", ("gerekli", "kesin")),
        // @action(permission_classes=[IsAuthenticated]) sınıfınkini ezer
        ("POST /api/kitaplar/:pk/yorum-ekle/", ("gerekli", "kesin")),
        // @action dekoratörü giriş işareti değildir
        ("GET /api/kitaplar/populer/", ("herkese_acik", "kesin")),
        // AllowAny açık beyan
        ("GET /api/yazarlar/", ("herkese_acik", "kesin")),
        // permission_classes yok: ayarlardaki DEFAULT_PERMISSION_CLASSES (IsAuthenticated)
        ("GET /api/v1/kitaplar/:kitap_id/yorumlar/", ("gerekli", "kesin")),
        ("GET /api/", ("gerekli", "kesin")),
        // Projedeki izin sınıfının gövdesinden: çıkarım
        ("POST /api/v1/kitaplar/:pk/yayinla/", ("gerekli", "varsayim")),
        // get_permissions çalışma anında farklı sınıflar döndürüyor: bilinmiyor
        ("GET /api/v1/kitaplar/:pk/dinamik/", ("bilinmiyor", "varsayim_bilinmeyen_guard")),
        // @api_view + @permission_classes([AllowAny])
        ("POST /api/v1/ara/", ("herkese_acik", "kesin")),
        // Django işaretleri: @login_required, @permission_required, LoginRequiredMixin, urls.py'deki login_required(...)
        ("GET /eski/:yil/", ("gerekli", "kesin")),
        ("POST /api/v1/toplu-sil/", ("gerekli", "kesin")),
        ("POST /hesap/profil/", ("gerekli", "kesin")),
        ("GET /hesap/ayarlar/", ("gerekli", "kesin")),
        // İşaret yok: açık sayılır ama yokluktan çıkarım
        ("GET /debug/saglik/", ("herkese_acik", "varsayim")),
        ("POST /hesap/kayit/", ("herkese_acik", "varsayim")),
    ]);
    for (id, (g, k)) in beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string()), "{id}");
    }
    assert_eq!(uc(&h, "POST /api/v1/toplu-sil/")["roller"], serde_json::json!(["kitaplar.delete_kitap"]));
    assert!(uc(&h, "POST /api/v1/toplu-sil/")["roller_kaniti"].is_null(), "metin olarak verilen izin kesin");
    assert_eq!(uc(&h, "POST /api/v1/kitaplar/:pk/yayinla/")["roller"], serde_json::json!(["staff"]));
    assert_eq!(uc(&h, "POST /api/v1/kitaplar/:pk/yayinla/")["roller_kaniti"], "varsayim");
    // Kanıt deposunda: kesin karar certain, çıkarım inferred, çalışma anı kararı unknown
    let kanit = std::fs::read_to_string(cikti.join("kanit.json")).unwrap();
    assert!(kanit.contains("\"bilinmiyor\""), "çözülemeyen giriş kararı kapsamı bilinmiyor");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn serializer_form_ve_sorgu_alanlari() {
    let (h, cikti) = harita("alan");
    let g = alanlar(uc(&h, "POST /api/kitaplar/"), "govde");
    let zorunlu = |a: &str| g[a]["zorunlu"].as_bool().unwrap();
    // ModelSerializer: modelin alanları (blank / default isteğe bağlı), bildirilen alanlar; read_only ve otomatik alanlar girdi değil
    assert!(zorunlu("baslik") && zorunlu("yazar_id"));
    assert!(!zorunlu("ozet") && !zorunlu("sayfa") && !zorunlu("durum") && !zorunlu("etiketler"));
    for yok in ["id", "olusturma", "yazar"] {
        assert!(g.get(yok).is_none(), "{yok} girdi değil: {g:?}");
    }
    assert!(kurallar(&g["baslik"]).contains(&"MaxLength(200)".to_string()));
    assert!(kurallar(&g["sayfa"]).contains(&"Max(5000)".to_string()), "extra_kwargs");
    assert_eq!(g["durum"]["degerler"], serde_json::json!(["taslak", "yayinda"]), "TextChoices");
    // PATCH: bütün alanlar isteğe bağlı
    assert!(alanlar(uc(&h, "PATCH /api/kitaplar/:pk/"), "govde").values().all(|a| a["zorunlu"] == false));
    // @action(serializer_class=YorumSerializer)
    let y = alanlar(uc(&h, "POST /api/kitaplar/:pk/yorum-ekle/"), "govde");
    assert!(y["metin"]["zorunlu"].as_bool().unwrap() && !y["kaynak"]["zorunlu"].as_bool().unwrap());
    assert!(kurallar(&y["puan"]).contains(&"Min(1)".to_string()) && kurallar(&y["puan"]).contains(&"Max(5)".to_string()));
    assert_eq!(y["kaynak"]["degerler"], serde_json::json!(["web", "mobil"]));
    // Django formu (FormView.form_class)
    let f = alanlar(uc(&h, "POST /hesap/kayit/"), "govde");
    assert!(f["kullanici_adi"]["zorunlu"].as_bool().unwrap() && !f["yas"]["zorunlu"].as_bool().unwrap());
    assert!(kurallar(&f["eposta"]).contains(&"IsEmail()".to_string()));
    // Sorgu: search_fields ve sayfalama (ayarlardaki DEFAULT_PAGINATION_CLASS), gövdede okunan parametre
    let s = alanlar(uc(&h, "GET /api/kitaplar/"), "sorgu");
    assert!(s.contains_key("search") && s.contains_key("page"), "{s:?}");
    assert!(alanlar(uc(&h, "GET /api/v1/ara/"), "sorgu").contains_key("q"));
    // Yanıt: serializer'ın okunur alanları; liste dizi ve sayfalı (kesin değil), oluşturma 201
    let yanit = &uc(&h, "POST /api/kitaplar/")["yanit"];
    assert_eq!((yanit["kesin"].as_bool(), yanit["durum"].as_u64()), (Some(true), Some(201)));
    assert!(yanit["sema"]["alanlar"]["olusturma"].is_object() && yanit["sema"]["alanlar"].get("yazar_id").is_none(), "write_only yanıtta yok");
    assert_eq!(uc(&h, "GET /api/kitaplar/")["yanit"]["sema"]["dizi"], true);
    // Önizlemenin denetleyemediği kurallar
    let den = &h["ozet"]["denetlenemeyen"]["kitaplar/views.py:16"];
    assert!(den.to_string().contains("validate_baslik"), "{den}");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn veri_kaskad_sinyal_ve_graf() {
    let (h, cikti) = harita("veri");
    let veri = |id: &str| (uc(&h, id)["veri"]["okur"].clone(), uc(&h, id)["veri"]["yazar"].clone());
    assert_eq!(veri("GET /api/kitaplar/"), (serde_json::json!(["Kitap"]), serde_json::json!([])));
    assert_eq!(veri("POST /api/kitaplar/"), (serde_json::json!([]), serde_json::json!(["Kitap"])));
    assert_eq!(veri("POST /api/kitaplar/:pk/yorum-ekle/").1, serde_json::json!(["Yorum"]));
    assert_eq!(veri("POST /api/v1/kitaplar/:pk/yayinla/"), (serde_json::json!(["Kitap"]), serde_json::json!(["Kitap"])), "get_object_or_404 + save");
    // on_delete=CASCADE (modelden) ve göç dosyasındaki ForeignKey
    let kaskadlar = h["ozet"]["kaskadlar"].to_string();
    assert!(kaskadlar.contains(r#"["Yazar","Kitap","kitaplar/models.py:19"]"#) && kaskadlar.contains(r#"["Kitap","Yorum","kitaplar/models.py:27"]"#), "{kaskadlar}");
    assert!(kaskadlar.contains("0001_initial.py"), "Django göç dosyası: {kaskadlar}");
    // Silme: perform_destroy'daki ret koşulu ve kaskadla dolaylı silinen Yorum
    let dna = &h["ozet"]["dna"]["kitaplar/views.py:36"];
    assert!(dna["hatalar"].to_string().contains("PermissionDenied"), "{dna}");
    assert!(dna["dolayli_yazar"].to_string().contains("Yorum"), "{dna}");
    // Sinyal: post_save dinleyicisi olay düğümüdür, Kitap yazan işlem onu tetikler
    assert!(h["ozet"]["olaylar"]["event:post_save:Kitap"]["dinleyiciler"].to_string().contains("kitap_kaydedildi"));
    assert!(h["ozet"]["dna"]["kitaplar/views.py:58"]["tetikler"].to_string().contains("post_save:Kitap"));
    // Kanıt grafiği: tablo adı Meta.db_table, yabancı anahtar sütunu, URL yapılandırması ve DEBUG koşulu
    let graf = std::fs::read_to_string(cikti.join("graf.json")).unwrap();
    let kanit = std::fs::read_to_string(cikti.join("kanit.json")).unwrap();
    assert!(kanit.contains("\"tablo\":\"kitap\"") && kanit.contains("\"sutun\":\"yazar_id\"") && kanit.contains("\"tablo\":\"kitaplar_yorum\""), "tablo ve sütun adları");
    assert!(graf.contains("mod:kitaplar/urls.py:urlpatterns") && graf.contains("mod:kitaplik/urls.py:router"), "URL yapılandırması ve router düğümleri");
    assert!(graf.contains("env:settings.DEBUG"), "koşullu kayıt ortam düğümüne bağlı");
    // Kart: olay düğümü ve tetik ilişkisi
    let kart: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("benlik.json")).unwrap()).unwrap();
    assert!(kart["x-nodes"].to_string().contains("post_save:Kitap"));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn onizleme_serializer_kurallariyla() {
    let (_, cikti) = harita("onizle");
    let mut p =
        Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", cikti.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    use std::io::{BufRead, Write};
    let istek = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": {
        "kapi": "POST /api/kitaplar/", "oturum": true, "govde": { "baslik": "", "sayfa": 9000, "durum": "silindi" } } } });
    writeln!(p.stdin.as_mut().unwrap(), "{istek}").unwrap();
    let mut satir = String::new();
    std::io::BufReader::new(p.stdout.take().unwrap()).read_line(&mut satir).unwrap();
    let _ = p.kill();
    let _ = p.wait();
    let v: serde_json::Value = serde_json::from_str(&satir).unwrap();
    let t = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(t.contains("REDDEDİLİR"), "{t}");
    assert!(t.contains("yazar_id") && t.contains("sayfa") && t.contains("durum"), "eksik zorunlu alan, üst sınır ve izinsiz değer: {t}");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn doktor_cati_ve_canli_mod_yol_parametresi() {
    use p0_g1_ortak::*;
    let ev = Ev::yeni("d6-django");
    let u = Uygulama::yeni();
    let p = ev.proje("d6_python_django", "a");
    let (_, d) = p.cikti(&["doktor"]);
    assert!(d.contains("Django"), "{d}");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    let (s, hata) = p.mcp(&t).cagir(serde_json::json!({ "kapi": "GET /api/kitaplar/:pk/", "parametreler": { "pk": "7" } }));
    assert!(!hata, "{s}");
    let g = u.gelenler.lock().unwrap();
    assert_eq!(g.last().map(|x| (x.yontem.as_str(), x.yol.as_str())), Some(("GET", "/api/kitaplar/7/")), "yol parametresi yerleştirildi, sondaki eğik çizgi korundu");
}
