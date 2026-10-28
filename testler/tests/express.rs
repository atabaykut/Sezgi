//! Express, Fastify ve Next.js adaptörü (seviye 1): rotalar ve önekler, ara katmandan giriş ve rol (Yorum),
//! kullanımdan girdiler (Yorum), zod ve Fastify şemasından girdiler (tanımdan), SQL tablo bağları (Yorum).
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

fn harita(ornek: &str) -> Value {
    let kok = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ornek);
    // Her çağrı kendi klasöründe: aynı örneği okuyan testler aynı anda çalışabilir
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-express-{ornek}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", cikti.to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&cikti);
    v
}

fn uc<'a>(h: &'a Value, id: &str) -> &'a Value {
    h["endpointler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("{id} yok: {:?}", h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].clone()).collect::<Vec<_>>()))
}

fn alan<'a>(e: &'a Value, bolum: &str, ad: &str) -> &'a Value {
    &e[bolum][0]["sema"]["alanlar"][ad]
}

#[test]
fn express_rotalar_onekler_ve_yetki() {
    let h = harita("express-temel");
    let mut idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    idler.sort();
    assert_eq!(
        idler,
        [
            "DELETE /api/kitaplar/:id",
            "GET /api/kitaplar",
            "GET /api/kitaplar/:id",
            "GET /api/kitaplar/ozet/sayilar",
            "GET /paylasim/dosya/:id",
            "GET /paylasim/dosya/:id/:ad",
            "GET /saglik",
            "GET /v2/etiketler",
            "POST /api/kitaplar",
            "POST /api/kitaplar/:id/yorum",
            "POST /api/yonetim/temizle",
            "POST /paylasim/cikis",
            "POST /paylasim/giris",
            "POST /v2/etiketler",
            "PUT /api/kitaplar/:id/kapak",
        ],
        "router önekleri, route() zinciri; istemcideki api.get rota sayılmaz"
    );
    assert_eq!(uc(&h, "GET /api/kitaplar")["giris"], "herkese_acik");
    assert_eq!(uc(&h, "POST /api/kitaplar")["giris"], "gerekli", "rota ara katmanı");
    assert_eq!(uc(&h, "GET /api/kitaplar/:id")["giris"], "gerekli", "yollu use('/:id', auth)");
    assert_eq!(uc(&h, "POST /api/yonetim/temizle")["giris"], "gerekli", "bağlamadaki ara katman");
    assert_eq!(uc(&h, "DELETE /api/kitaplar/:id")["roller"], json!(["admin"]), "değişkene atanmış requireRole('admin')");
    // Ara katmandan çıkan karar kesin değildir
    assert_ne!(uc(&h, "POST /api/kitaplar")["giris_kaniti"], "kesin");
    assert_eq!(uc(&h, "GET /api/kitaplar")["ozet"], "Kitapları listele", "üstteki yorum");
    assert_eq!(uc(&h, "GET /api/kitaplar/ozet/sayilar")["fonksiyon"], "ozet", "başka modüldeki controller fonksiyonu");
    // Zincirle kurulan router (module.exports = Router().use('/v2', ic)), noktalı dosya adı, isteğe bağlı kimlik
    assert_eq!(uc(&h, "GET /v2/etiketler")["ana"], true);
    assert_eq!(uc(&h, "GET /v2/etiketler")["giris"], "istege_bagli", "optionalAuth");
    assert_eq!(uc(&h, "POST /v2/etiketler")["giris"], "gerekli");
}

#[test]
fn express_girdiler_ve_veri() {
    let h = harita("express-temel");
    let liste = uc(&h, "GET /api/kitaplar");
    assert_eq!(alan(liste, "sorgu", "sayfa")["tip"], "number", "varsayılan değerden");
    assert_eq!(alan(liste, "sorgu", "ara")["belirsiz"], true, "kullanımdan çıkan alan Yorum");
    let ekle = uc(&h, "POST /api/kitaplar");
    assert_eq!(alan(ekle, "govde", "etiketler")["tip"], "array");
    assert_eq!(alan(ekle, "govde", "sayfaSayisi")["zorunlu"], false);
    // Sarmalayıcıdaki işleyici (asyncHandler) okunur
    assert!(alan(uc(&h, "DELETE /api/kitaplar/:id"), "govde", "neden").is_object());
    // zod şeması tanımdır: kesin
    let yorum = uc(&h, "POST /api/kitaplar/:id/yorum");
    assert_eq!(yorum["govde"][0]["sema"]["dto"], "yorumSema");
    assert!(alan(yorum, "govde", "metin")["belirsiz"].is_null(), "{yorum}");
    assert_eq!(alan(yorum, "govde", "metin")["zorunlu"], true);
    assert_eq!(alan(yorum, "govde", "puan")["zorunlu"], false);
    // SQL tablo bağları (Yorum: eksik olabilir)
    assert_eq!(uc(&h, "GET /api/kitaplar/:id")["veri"]["okur"], json!(["kitaplar", "yazarlar"]));
    assert_eq!(uc(&h, "DELETE /api/kitaplar/:id")["veri"]["yazar"], json!(["kitaplar"]));
    assert_eq!(uc(&h, "DELETE /api/kitaplar/:id")["veri"]["okur"], json!([]), "DELETE FROM okuma değildir");
    assert_eq!(uc(&h, "POST /api/yonetim/temizle")["veri"]["eksik_olabilir"], true);
}

#[test]
fn fastify_eklenti_onek_sema_ve_secenek() {
    let h = harita("fastify-temel");
    let kayit = uc(&h, "POST /kayit");
    assert_eq!(kayit["govde"][0]["sema"]["dto"], "schema.body");
    assert_eq!(alan(kayit, "govde", "eposta")["zorunlu"], true);
    assert_eq!(alan(kayit, "govde", "dil")["zorunlu"], false);
    assert_eq!(alan(kayit, "govde", "dil")["degerler"], json!(["tr", "en"]));
    assert!(alan(kayit, "govde", "sifre")["belirsiz"].is_null(), "JSON Schema tanımdır");
    assert_eq!(kayit["giris"], "herkese_acik");
    assert_eq!(uc(&h, "GET /profil")["giris"], "gerekli", "değişkendeki {{ preHandler: requireAuth }}");
    // register(eklenti, { prefix }) ve eklentinin ilk parametresi
    assert_eq!(uc(&h, "GET /v1/urunler/:id")["parametreler"][0]["ad"], "id");
    assert!(alan(uc(&h, "GET /v1/urunler"), "sorgu", "kategori").is_object());
}

#[test]
fn next_rota_dosyalari() {
    let h = harita("next-temel");
    let get = uc(&h, "GET /api/urunler/:id");
    assert!(alan(get, "sorgu", "dil").is_object(), "searchParams.get");
    assert_eq!(get["giris"], "herkese_acik");
    let put = uc(&h, "PUT /api/urunler/:id");
    assert_eq!(put["giris"], "gerekli", "getServerSession / 401");
    assert!(alan(put, "govde", "fiyat").is_object(), "await request.json() yapı bozumu");
    // Rota grubu (magaza) yola girmez; değişkene alınan gövde
    let sepet = uc(&h, "POST /api/sepet");
    assert!(alan(sepet, "govde", "urunId").is_object() && alan(sepet, "govde", "adet").is_object());
    // İşleyici içindeki yetki yardımcısı (`@/lib` takma yolundan, tanımı 401/403): giriş ve rol
    let fiyat = uc(&h, "POST /api/fiyatlar");
    assert_eq!(fiyat["giris"], "gerekli");
    assert_eq!(fiyat["roller"], json!(["FIYAT_YAZ"]));
    // Kendi 401'ini döndüren giriş ucu giriş gerektirmez
    assert_eq!(uc(&h, "POST /api/giris")["giris"], "herkese_acik");
    // middleware.ts matcher'ının kapsadığı rota giriş ister (Yorum); kapsamadığı rotalar etkilenmez
    assert_eq!(uc(&h, "POST /api/sepet")["giris"], "gerekli", "middleware.ts matcher /api/sepet/:path*");
    assert_ne!(uc(&h, "POST /api/sepet")["giris_kaniti"], "kesin");
    assert_eq!(uc(&h, "GET /api/urunler/:id")["giris"], "herkese_acik", "matcher dışında");
    assert!(h["ozet"]["genel_guardlar"][0].as_str().unwrap().starts_with("middleware.ts"), "{}", h["ozet"]);
}

/// Directus holdout'unun bulduğu kalıplar (kanit/SONUC-16): Joi şeması tanımdır (zorunluluk `.required()`'dan, izinli
/// değerler `.valid()`'den); isteği yardımcı fonksiyona veren işleyicide yardımcının okuduğu alan girdidir; `:ad?`
/// isteğe bağlı parametreli rota iki adresi karşılar
#[test]
fn joi_yardimci_ve_istege_bagli_parametre() {
    let h = harita("express-temel");
    let giris = uc(&h, "POST /paylasim/giris");
    assert_eq!(alan(giris, "govde", "share")["zorunlu"], true, "{giris}");
    assert_eq!(alan(giris, "govde", "password")["zorunlu"], false);
    assert_eq!(alan(giris, "govde", "mode")["degerler"], json!(["cookie", "json"]));
    assert!(alan(giris, "govde", "share")["belirsiz"].is_null(), "Joi şeması tanımdır: kesin");
    let cikis = uc(&h, "POST /paylasim/cikis");
    assert_eq!(alan(cikis, "govde", "kip")["belirsiz"], true, "yardımcının okuduğu alan, kullanımdan (Yorum): {cikis}");
    assert_eq!(alan(cikis, "govde", "kip")["zorunlu"], false, "yalnızca `if (req.body.kip)` ile denetlenen alan isteğe bağlı");
    assert_eq!(uc(&h, "GET /paylasim/dosya/:id")["satir"], uc(&h, "GET /paylasim/dosya/:id/:ad")["satir"], "aynı rota, iki adres");
}
