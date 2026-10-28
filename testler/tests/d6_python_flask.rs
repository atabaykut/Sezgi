//! Python + Flask: uygulama fabrikası ve yardımcı fonksiyonla Blueprint kaydı (kayıttaki url_prefix Blueprint'inkini
//! ezer), koşullu kayıt, MethodView / add_url_rule, Flask-RESTful kaynakları ve reqparse, Flask-Smorest sınıfları,
//! giriş işaretleri (Flask-Login, Flask-JWT-Extended, proje dekoratörü, before_request kancası), marshmallow şemaları,
//! istek anahtarları, Flask-SQLAlchemy veri bağları ve kaskadlar. Kart, OpenAPI, önizleme ve canlı mod dilden bağımsız.
mod p0_g1_ortak;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn harita(ad: &str) -> (serde_json::Value, PathBuf) {
    let cikti = std::env::temp_dir().join(format!("sezgi-d6-flask-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("d6_python_flask");
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
fn rotalar_blueprint_zinciri() {
    let (h, cikti) = harita("rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    let beklenen = [
        // register_blueprint(url_prefix="/api/urunler") Blueprint'in kendi önekini ("/eski") ezer; @bp.get / @bp.delete
        "GET /api/urunler/",
        "POST /api/urunler/",
        "DELETE /api/urunler/:urun_id",
        "GET /api/urunler/:urun_id",
        // Flask-RESTful Api(api_bp), yardımcı fonksiyonda kaydedilen Blueprint
        "POST /api/v2/siparisler",
        "DELETE /api/v2/siparisler/:siparis_id",
        "GET /api/v2/siparisler/:siparis_id",
        "GET /hesap/giris",
        "POST /hesap/giris",
        // MethodView + add_url_rule
        "GET /hesap/profil",
        "PUT /hesap/profil",
        // Fabrikanın içindeki rota
        "GET /saglik",
        // Flask-Smorest sınıfı, if altında kaydedilen Blueprint
        "GET /yonetim/kategoriler",
        "POST /yonetim/kategoriler",
    ];
    assert_eq!(idler, beklenen);
    assert_eq!(h["ozet"]["adaptor"], "flask@0.1.0");
    assert_eq!(h["ozet"]["uygulama"], "magaza");
    assert_eq!(h["ozet"]["ana_uygulama"], "magaza:create_app().app");
    assert_eq!(h["ozet"]["kosullu_kapilar"], serde_json::json!(["GET /yonetim/kategoriler", "POST /yonetim/kategoriler"]));
    let p = &uc(&h, "GET /api/urunler/:urun_id")["parametreler"][0];
    assert_eq!((p["ad"].as_str(), p["tip"].as_str()), (Some("urun_id"), Some("int")));
    let ad = |id: &str| format!("{}.{}", uc(&h, id)["kontrolcu"].as_str().unwrap(), uc(&h, id)["fonksiyon"].as_str().unwrap());
    assert_eq!(ad("GET /api/urunler/:urun_id"), "Urunler.getir");
    assert_eq!(ad("PUT /hesap/profil"), "Profil.put");
    let kart: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("benlik.json")).unwrap()).unwrap();
    assert_eq!(kart["app"]["version"], "0.3.0");
    let oa: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("openapi.json")).unwrap()).unwrap();
    assert!(oa["paths"]["/api/urunler/{urun_id}"]["delete"].is_object());
    // Bağımlılık grafiği: Blueprint kaydı ve koşulu
    let graf = std::fs::read_to_string(cikti.join("graf.json")).unwrap();
    assert!(graf.contains("mod:magaza/yonetim.py:yonetim_bp") && graf.contains("env:app.config.get(\\\"YONETIM\\\")"), "koşullu Blueprint kaydı");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn giris_ve_rol_kararlari() {
    let (h, cikti) = harita("giris");
    let karar = |id: &str| (uc(&h, id)["giris"].as_str().unwrap().to_string(), uc(&h, id)["giris_kaniti"].as_str().unwrap().to_string());
    let beklenen: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        // Flask-JWT-Extended
        ("POST /api/urunler/", ("gerekli", "kesin")),
        ("GET /api/urunler/:urun_id", ("istege_bagli", "kesin")),
        // Projedeki dekoratörün gövdesinden: çıkarım
        ("DELETE /api/urunler/:urun_id", ("gerekli", "varsayim")),
        // MethodView.decorators = [login_required]
        ("PUT /hesap/profil", ("gerekli", "kesin")),
        // Flask-RESTful method_decorators
        ("GET /api/v2/siparisler/:siparis_id", ("gerekli", "kesin")),
        // @yonetim_bp.before_request + @login_required: Blueprint'in bütün rotaları
        ("POST /yonetim/kategoriler", ("gerekli", "kesin")),
        // İşaret yok: yokluktan çıkarım
        ("GET /api/urunler/", ("herkese_acik", "varsayim")),
        ("POST /api/v2/siparisler", ("herkese_acik", "varsayim")),
        ("GET /saglik", ("herkese_acik", "varsayim")),
    ]);
    for (id, (g, k)) in beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string()), "{id}");
    }
    assert_eq!(uc(&h, "DELETE /api/urunler/:urun_id")["roller"], serde_json::json!(["admin"]));
    assert_eq!(uc(&h, "DELETE /api/urunler/:urun_id")["roller_kaniti"], "varsayim");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn sema_ve_istek_alanlari() {
    let (h, cikti) = harita("alan");
    // marshmallow: Schema().load(request.get_json()); required, Length, Range, OneOf; dump_only girdi değil, load_only girdi
    let g = alanlar(uc(&h, "POST /api/urunler/"), "govde");
    assert!(g["ad"]["zorunlu"].as_bool().unwrap() && g["fiyat"]["zorunlu"].as_bool().unwrap() && !g["kategori"]["zorunlu"].as_bool().unwrap());
    let k = |a: &str| g[a]["kurallar"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert!(k("ad").contains(&"MinLength(2)".to_string()) && k("ad").contains(&"MaxLength(120)".to_string()));
    assert!(k("fiyat").contains(&"Min(0)".to_string()));
    assert_eq!(g["kategori"]["degerler"], serde_json::json!(["kitap", "oyun"]));
    assert!(g.get("id").is_none() && g.contains_key("notlar"));
    // Flask-RESTful reqparse
    let r = alanlar(uc(&h, "POST /api/v2/siparisler"), "govde");
    assert!(r["adet"]["zorunlu"].as_bool().unwrap() && r["adet"]["tip"] == "int" && !r["not"]["zorunlu"].as_bool().unwrap());
    // Flask-Smorest @blp.arguments ve @blp.response
    let s = uc(&h, "POST /yonetim/kategoriler");
    assert!(alanlar(s, "govde")["ad"]["zorunlu"].as_bool().unwrap());
    assert_eq!((s["yanit"]["durum"].as_u64(), s["yanit"]["kesin"].as_bool()), (Some(201), Some(true)));
    assert_eq!(uc(&h, "GET /yonetim/kategoriler")["yanit"]["sema"]["dizi"], true, "KategoriSema(many=True)");
    // İşleyicinin okuduğu anahtarlar: tipleri bilinmez (belirsiz)
    let p = alanlar(uc(&h, "PUT /hesap/profil"), "govde");
    assert!(p["ad"]["belirsiz"].as_bool().unwrap() && p.contains_key("telefon"));
    assert!(alanlar(uc(&h, "GET /api/urunler/"), "sorgu").contains_key("q"));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn veri_ve_kaskad() {
    let (h, cikti) = harita("veri");
    let veri = |id: &str| (uc(&h, id)["veri"]["okur"].clone(), uc(&h, id)["veri"]["yazar"].clone());
    assert_eq!(veri("GET /api/urunler/"), (serde_json::json!(["Urun"]), serde_json::json!([])));
    assert_eq!(veri("POST /api/urunler/").1, serde_json::json!(["Urun"]));
    assert_eq!(veri("DELETE /api/urunler/:urun_id"), (serde_json::json!(["Urun"]), serde_json::json!(["Urun"])));
    assert_eq!(veri("POST /api/v2/siparisler").1, serde_json::json!(["Siparis"]));
    let kaskadlar = h["ozet"]["kaskadlar"].as_array().unwrap();
    let ciftler: Vec<(String, String)> = kaskadlar.iter().map(|k| (k[0].as_str().unwrap().to_string(), k[1].as_str().unwrap().to_string())).collect();
    assert_eq!(ciftler, vec![("Kategori".to_string(), "Urun".to_string()), ("Urun".to_string(), "Siparis".to_string())], "relationship ve ForeignKey(ondelete) tek kaskad");
    // Ürün silinince sipariş kaskadla dolaylı silinir
    assert!(h["ozet"]["dna"]["magaza/urunler/views.py:37"]["dolayli_yazar"].to_string().contains("Siparis"));
    // Tablo adı: __tablename__ ya da Flask-SQLAlchemy varsayılanı (yılan biçimi)
    let kanit = std::fs::read_to_string(cikti.join("kanit.json")).unwrap();
    assert!(kanit.contains("\"tablo\":\"urunler\"") && kanit.contains("\"tablo\":\"siparis\""));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn canli_mod_yol_parametresi_ve_doktor() {
    use p0_g1_ortak::*;
    let ev = Ev::yeni("d6-flask");
    let u = Uygulama::yeni();
    let p = ev.proje("d6_python_flask", "a");
    let (_, d) = p.cikti(&["doktor"]);
    assert!(d.contains("Flask"), "{d}");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["urunler.*:oku"], &[]);
    let (s, hata) = p.mcp(&t).cagir(serde_json::json!({ "kapi": "GET /api/urunler/:urun_id", "parametreler": { "urun_id": 42 } }));
    assert!(!hata, "{s}");
    let g = u.gelenler.lock().unwrap();
    assert_eq!(g.last().map(|x| (x.yontem.as_str(), x.yol.as_str())), Some(("GET", "/api/urunler/42")));
}
