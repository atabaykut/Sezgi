//! C# + ASP.NET Core (kaynak kodlu): controller rotaları ([Route] + [HttpX], [controller] / [action] yer değiştirmesi,
//! `~/` mutlak şablon, [ApiVersion]), Minimal API (iç içe MapGroup, uzantı metodu, metot grubu, koşullu kayıt),
//! girdiler ve DataAnnotations (çatı doğrulamıyorsa kısıt yok), yanıt şeması, giriş ve rol (Authorize / Policy /
//! AllowAnonymous / FallbackPolicy; kesinlik ilkesi), EF Core veri etkisi ve kaskadlar, MediatR olayları ve
//! BackgroundService işleri. Kart, OpenAPI, kanıt grafiği, önizleme, doktor, rapor ve canlı mod dilden bağımsız çalışır.
mod p0_g1_ortak;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const ORNEK: &str = "d6_aspnet_magaza";

fn harita(ad: &str) -> (Value, PathBuf) {
    let cikti = std::env::temp_dir().join(format!("sezgi-d6-aspnet-{ad}-{}", std::process::id()));
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
    h["endpointler"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("{id} yok"))
}

fn dna<'a>(h: &'a Value, id: &str) -> &'a Value {
    let e = uc(h, id);
    &h["ozet"]["dna"][format!("{}:{}", e["dosya"].as_str().unwrap(), e["satir"]).as_str()]
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
fn rotalar_gruplar_ve_kosullu_kayit() {
    let (h, cikti) = harita("rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    let beklenen = [
        // [Route("api/[controller]/[action]")], [ActionName], rota kısıtı {yil:int:min(2000)}, "~/" mutlak şablon
        "GET /api/Raporlar/Gunluk",
        "GET /api/Raporlar/Yillik/:yil",
        "GET /api/Urunler",
        "POST /api/Urunler",
        "DELETE /api/Urunler/:id",
        "GET /api/Urunler/:id",
        "PUT /api/Urunler/:id",
        "POST /api/disa-aktar",
        // Uzantı metodu (app.MapKampanyaUclari()) içindeki grup ve metot grubu işleyici
        "GET /api/kampanyalar/:kod",
        "POST /api/kampanyalar/:kod/katil",
        // İç içe MapGroup ("/api/v1" + "/notlar")
        "GET /api/v1/notlar/",
        "POST /api/v1/notlar/",
        "DELETE /api/v1/notlar/:id",
        "GET /api/v1/notlar/:id",
        "GET /api/v1/notlar/ara",
        // [ApiVersion("2.0")] + {version:apiVersion}
        "POST /api/v2/Siparisler",
        "GET /api/v2/Siparisler/:id",
        "POST /api/v2/Siparisler/:id/iptal",
        "GET /gelistirme/ayarlar",
        "GET /saglik",
    ];
    assert_eq!(idler, beklenen, "test projesi (xunit) ve obj/ altındaki kod uygulamanın API'si değildir");
    assert_eq!(h["ozet"]["adaptor"], "aspnet@0.1.0");
    assert_eq!(h["ozet"]["uygulama"], "Magaza.Api", "AssemblyName");
    assert_eq!(h["ozet"]["ana_uygulama"], "Program.cs");
    assert_eq!(h["ozet"]["kosullu_kapilar"], json!(["GET /gelistirme/ayarlar"]), "if (app.Environment.IsDevelopment()) altında");
    // Yol parametresinin tipi işleyiciden; sürüm, kullanımdan kalkma, belge yorumu ve işlem kimlikleri
    assert_eq!(uc(&h, "GET /api/v2/Siparisler/:id")["parametreler"], json!([{ "ad": "id", "tip": "Guid" }]));
    assert_eq!(uc(&h, "GET /api/v2/Siparisler/:id")["surum"], "2.0");
    assert_eq!(uc(&h, "POST /api/v2/Siparisler/:id/iptal")["kullanimdan_kalkmis"], true, "[Obsolete]");
    assert_eq!(uc(&h, "GET /api/Urunler")["ozet"], "Ürünleri listeler", "/// <summary>");
    assert_eq!(uc(&h, "GET /api/Urunler/:id")["fonksiyon"], "getir", "Async eki atılır");
    assert_eq!(uc(&h, "DELETE /api/v1/notlar/:id")["dosya"], "Uclar/NotUclari.cs", "metot grubu işleyici kendi dosyasında");
    let kimlikler = &h["ozet"]["islem_kimlikleri"];
    let kimlik = |id: &str| {
        let e = uc(&h, id);
        kimlikler[format!("{}:{}", e["dosya"].as_str().unwrap(), e["satir"]).as_str()].clone()
    };
    assert_eq!(kimlik("GET /api/Urunler/:id"), "UrunGetir", "[HttpGet(..., Name = ...)]");
    assert_eq!(kimlik("POST /api/v1/notlar/"), "NotOlustur", ".WithName(...)");
    // Kart ve OpenAPI dilden bağımsız
    let kart = json_oku(&cikti.join("benlik.json"));
    assert_eq!(kart["app"]["version"], "2.3.0", "<Version>");
    assert!(kart["operations"].as_array().unwrap().iter().any(|o| o["id"] == "urunler.olustur"), "{}", kart["operations"]);
    let oa = json_oku(&cikti.join("openapi.json"));
    assert!(oa["paths"]["/api/Urunler/{id}"]["delete"].is_object());
    assert_eq!(oa["paths"]["/api/Urunler/{id}"]["get"]["operationId"], "UrunGetir");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn giris_ve_rol_kararlari() {
    let (h, cikti) = harita("giris");
    let karar = |id: &str| {
        let e = uc(&h, id);
        (
            e["giris"].as_str().unwrap().to_string(),
            e["giris_kaniti"].as_str().unwrap().to_string(),
            e["roller"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect::<Vec<_>>(),
        )
    };
    let beklenen: BTreeMap<&str, (&str, &str, Vec<&str>)> = BTreeMap::from([
        // [AllowAnonymous] genel FallbackPolicy'yi geçersiz kılar
        ("GET /api/Urunler", ("herkese_acik", "kesin", vec![])),
        ("POST /api/Urunler", ("gerekli", "kesin", vec!["Editor", "Yonetici"])),
        // İşaretsiz eylem: genel FallbackPolicy (RequireAuthenticatedUser) tanımı
        ("PUT /api/Urunler/:id", ("gerekli", "kesin", vec![])),
        // Policy = Politikalar.Yonetici -> AddPolicy(Politikalar.Yonetici, p => p.RequireRole(Roller.Yonetici)), sabitler çözülür
        ("DELETE /api/Urunler/:id", ("gerekli", "kesin", vec!["Yonetici"])),
        // Sınıf düzeyinde talep (claim) politikası: rol değildir
        ("GET /api/v2/Siparisler/:id", ("gerekli", "kesin", vec![])),
        ("POST /api/v2/Siparisler", ("gerekli", "kesin", vec!["Musteri"])),
        // Grup RequireAuthorization() ve uçta RequireAuthorization(politika)
        ("GET /api/v1/notlar/", ("gerekli", "kesin", vec![])),
        ("DELETE /api/v1/notlar/:id", ("gerekli", "kesin", vec!["Yonetici"])),
        ("GET /saglik", ("herkese_acik", "kesin", vec![])),
        // Grupta AllowAnonymous(): uçtaki [Authorize(Roles = "Musteri")] etkisizdir (ASP.NET Core kuralı)
        ("POST /api/kampanyalar/:kod/katil", ("herkese_acik", "kesin", vec![])),
        ("GET /api/Raporlar/Gunluk", ("gerekli", "kesin", vec![])),
    ]);
    for (id, (g, k, r)) in beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string(), r.iter().map(|x| x.to_string()).collect::<Vec<_>>()), "{id}");
    }
    assert!(uc(&h, "PUT /api/Urunler/:id")["guardlar"].to_string().contains("FallbackPolicy"));
    assert_eq!(h["ozet"]["genel_guardlar"], json!(["FallbackPolicy (RequireAuthenticatedUser)"]));
    assert!(uc(&h, "POST /api/Urunler")["roller_kaniti"].is_null(), "tek rol koşulu: kesin");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn girdiler_dataannotations_ve_yanit() {
    let (h, cikti) = harita("alan");
    // [ApiController] + positional record: DataAnnotations kural, varsayılanlı parametre isteğe bağlı, camelCase JSON adları
    let g = alanlar(uc(&h, "POST /api/Urunler"), "govde");
    assert_eq!(g.keys().cloned().collect::<Vec<_>>(), ["ad", "durum", "fiyat", "iletisimEposta", "kategoriId"]);
    assert!(g["ad"]["zorunlu"].as_bool().unwrap() && g["ad"]["belirsiz"].is_null());
    for k in ["MaxLength(100)", "MinLength(2)", "IsNotEmpty()"] {
        assert!(kurallar(&g["ad"]).contains(&k.to_string()), "{k}: {:?}", kurallar(&g["ad"]));
    }
    assert!(kurallar(&g["fiyat"]).contains(&"Min(0.01)".to_string()) && kurallar(&g["fiyat"]).contains(&"Max(100000)".to_string()));
    assert!(kurallar(&g["iletisimEposta"]).contains(&"IsEmail()".to_string()) && !g["iletisimEposta"]["zorunlu"].as_bool().unwrap());
    assert!(!g["kategoriId"]["zorunlu"].as_bool().unwrap(), "[Required]'sız değer tipi zorunlu değil");
    assert!(!g["durum"]["zorunlu"].as_bool().unwrap(), "varsayılan değerli parametre");
    assert_eq!(g["durum"]["degerler"], json!(["Taslak", "Yayinda", "Arsiv"]), "JsonStringEnumConverter kayıtlı");
    // Sınıf DTO'su: Nullable bağlamında boş olamayan metin örtük [Required]; [JsonPropertyName]; [RegularExpression] bütün değer
    let g = alanlar(uc(&h, "PUT /api/Urunler/:id"), "govde");
    assert!(g["aciklama"]["zorunlu"].as_bool().unwrap(), "örtük [Required]");
    assert!(!g["etiketler"]["zorunlu"].as_bool().unwrap());
    assert!(kurallar(&g["sku_kodu"]).contains(&r"Matches(/^[A-Z]{3}-\d{4}$/)".to_string()), "{:?}", kurallar(&g["sku_kodu"]));
    assert!(kurallar(&g["stok"]).contains(&"Max(1000)".to_string()));
    // Değer tipinde [Required] eksik alanı yakalamaz: zorunluluk tahmin
    let s = alanlar(uc(&h, "POST /api/v2/Siparisler"), "govde");
    assert_eq!((s["urunId"]["zorunlu"].as_bool(), s["urunId"]["belirsiz"].as_bool()), (Some(true), Some(true)));
    // Minimal API: `required` (System.Text.Json) kesin; DataAnnotations doğrulanmaz (AddValidation yok): kısıt yok, tahmin
    let n = alanlar(uc(&h, "POST /api/v1/notlar/"), "govde");
    assert!(n["baslik"]["zorunlu"].as_bool().unwrap() && n["baslik"]["belirsiz"].is_null());
    let k = alanlar(uc(&h, "POST /api/kampanyalar/:kod/katil"), "govde");
    assert_eq!(k["eposta"]["belirsiz"], true);
    assert!(!kurallar(&k["eposta"]).contains(&"IsEmail()".to_string()) && !kurallar(&k["adet"]).iter().any(|r| r.starts_with("Max")), "{k:?}");
    // Sorgu: [FromQuery] basit tipler, [AsParameters] (FromQuery(Name = "q")); DI servisi girdi değildir
    let q = alanlar(uc(&h, "GET /api/Urunler"), "sorgu");
    assert_eq!(q.keys().cloned().collect::<Vec<_>>(), ["ara", "sayfa"]);
    assert!(!q["sayfa"]["zorunlu"].as_bool().unwrap());
    let q = alanlar(uc(&h, "GET /api/v1/notlar/ara"), "sorgu");
    assert_eq!(q.keys().cloned().collect::<Vec<_>>(), ["Sayfa", "q"]);
    assert!(uc(&h, "GET /api/v1/notlar/")["govde"].as_array().unwrap().is_empty(), "MagazaDb parametresi DI'dan gelir");
    // [FromHeader] girdi değil; IFormFile önizlemede sınanamaz
    assert!(alanlar(uc(&h, "POST /api/v2/Siparisler/:id/iptal"), "sorgu").is_empty());
    let not = &h["ozet"]["denetlenemeyen"][format!("Controllers/RaporlarController.cs:{}", uc(&h, "POST /api/disa-aktar")["satir"]).as_str()];
    assert!(not.to_string().contains("dosya doğrulaması kontrol edilemedi"), "{not}");
    // Yanıt: ActionResult<List<T>> dizi, [ProducesResponseType(typeof(T), 201)], Results<Ok<T>, NotFound>
    let y = &uc(&h, "GET /api/Urunler")["yanit"];
    assert_eq!((y["sema"]["dto"].as_str(), y["sema"]["dizi"].as_bool()), (Some("UrunDto"), Some(true)));
    let y = &uc(&h, "POST /api/Urunler")["yanit"];
    assert_eq!((y["kaynak"].as_str(), y["durum"].as_u64()), (Some("ProducesResponseType"), Some(201)));
    assert_eq!(uc(&h, "GET /api/v1/notlar/:id")["yanit"]["sema"]["dto"], "NotDto");
    let y = &uc(&h, "GET /api/v1/notlar/")["yanit"];
    assert_eq!((y["sema"]["dto"].as_str(), y["sema"]["dizi"].as_bool()), (Some("Not"), Some(true)), "TypedResults.Ok<List<Not>>(...)");
    assert!(uc(&h, "GET /api/v1/notlar/:id")["yanit"]["sema"]["alanlar"]["baslik"].is_object());
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn ef_core_veri_kaskad_olay_ve_graf() {
    let (h, cikti) = harita("veri");
    let veri = |id: &str| (uc(&h, id)["veri"]["okur"].clone(), uc(&h, id)["veri"]["yazar"].clone());
    // Arayüzden uygulamaya (IUrunServisi -> UrunServisi), DbContext.Add(değişken), MediatR Send -> IRequestHandler
    assert_eq!(veri("POST /api/Urunler"), (json!(["Kategori"]), json!(["Urun"])));
    assert_eq!(veri("DELETE /api/Urunler/:id"), (json!(["Urun"]), json!(["Urun"])));
    assert_eq!(veri("POST /api/v2/Siparisler"), (json!([]), json!(["Siparis"])));
    assert_eq!(veri("POST /api/v2/Siparisler/:id/iptal"), (json!([]), json!(["Siparis"])), "ExecuteDeleteAsync");
    assert_eq!(veri("GET /api/v1/notlar/"), (json!(["Not"]), json!([])));
    // Kaskad: OnDelete(DeleteBehavior.Cascade); Restrict ve boş olabilen yabancı anahtar kaskad değildir
    let kaskad = |e: &str, c: &str| h["ozet"]["kaskadlar"].as_array().unwrap().iter().any(|k| k[0] == e && k[1] == c);
    assert!(kaskad("Kategori", "Urun") && kaskad("Urun", "Yorum"), "{}", h["ozet"]["kaskadlar"]);
    assert!(!kaskad("Urun", "Siparis"));
    let d = dna(&h, "DELETE /api/Urunler/:id");
    assert_eq!(d["dolayli_yazar"][0]["tablo"], "Yorum", "{d:#}");
    // Sahiplik denetimi servis zincirinde (inferred); red sonucu işleyicide (certain)
    assert_eq!(d["sahiplik_denetimi"][0]["kosul"], "urun.SahipId != kullanici");
    let hata = |d: &Value, kod: u64| d["hatalar"].as_array().unwrap().iter().find(|x| x["durum"] == kod).cloned();
    let n = hata(dna(&h, "GET /api/Urunler/:id"), 404).expect("return NotFound()");
    assert_eq!((n["kosul"].as_str(), n["guven"].as_str()), (Some("urun == null"), Some("certain")));
    assert_eq!(hata(dna(&h, "DELETE /api/v1/notlar/:id"), 404).unwrap()["kosul"], "not is null");
    // Olaylar: MediatR Publish (işleyicide ve Send ile ulaşılan işleyicide), INotificationHandler, BackgroundService
    let tetik = |id: &str| dna(&h, id)["tetikler"].as_array().unwrap().iter().map(|t| format!("{}:{}", t["tur"].as_str().unwrap(), t["ad"].as_str().unwrap())).collect::<Vec<_>>();
    assert_eq!(tetik("POST /api/Urunler"), ["event:UrunOlusturuldu"]);
    assert_eq!(tetik("POST /api/v2/Siparisler"), ["event:SiparisVerildi"]);
    let ol = &h["ozet"]["olaylar"];
    assert_eq!(ol["event:SiparisVerildi"]["dinleyiciler"][0]["fonksiyon"], "StokDusur.Handle");
    assert_eq!(ol["event:SiparisVerildi"]["silinen"], json!(["Urun"]));
    assert_eq!(ol["event:SiparisVerildi"]["dolayli_yazar"][0]["tablo"], "Yorum");
    assert_eq!(ol["job:StokTemizleyici"]["silinen"], json!(["Yorum"]));
    // Kanıt: tablo adı [Table] / ToTable / DbSet adından; işlem yazar, tetikler; graf düğümleri
    let depo = json_oku(&cikti.join("kanit.json"));
    let onermeler = depo["onermeler"].as_array().unwrap();
    let var = |v: &str, y: &str, f: &dyn Fn(&Value) -> bool| onermeler.iter().any(|o| o["varlik"] == v && o["yuklem"] == y && f(o));
    assert!(var("db:Urun", "TABLO", &|o| o["deger"]["tablo"] == "urun_tablosu"));
    assert!(var("db:Siparis", "TABLO", &|o| o["deger"]["tablo"] == "siparisler"));
    assert!(var("db:Kategori", "TABLO", &|o| o["deger"]["tablo"] == "Kategoriler"));
    assert!(var("api:DELETE /api/Urunler/:id", "WRITES", &|o| o["hedef"] == "db:Urun"));
    assert!(var("api:DELETE /api/Urunler/:id", "MAY_AFFECT", &|o| o["hedef"] == "db:Yorum"));
    assert!(var("api:POST /api/Urunler", "TRIGGERS", &|o| o["hedef"] == "event:UrunOlusturuldu"));
    assert!(var("api:GET /api/Urunler", "DEPENDS_ON", &|o| o["hedef"] == "mod:Controllers/UrunlerController.cs:UrunlerController"));
    assert!(var("mod:Program.cs:/api/v1/notlar", "DEPENDS_ON", &|o| o["hedef"] == "mod:Program.cs:/api/v1"));
    let kart = json_oku(&cikti.join("benlik.json"));
    assert!(kart["x-nodes"].to_string().contains("job:StokTemizleyici"));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn onizleme_dataannotations_kurallariyla() {
    let (_, cikti) = harita("onizle");
    let mut p =
        Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", cikti.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    use std::io::{BufRead, Write};
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": {
        "kapi": "POST /api/Urunler", "oturum": true, "govde": { "ad": "x", "fiyat": 0, "kategoriId": 1 } } } });
    writeln!(p.stdin.as_mut().unwrap(), "{istek}").unwrap();
    let mut satir = String::new();
    std::io::BufReader::new(p.stdout.take().unwrap()).read_line(&mut satir).unwrap();
    let _ = p.kill();
    let _ = p.wait();
    let v: Value = serde_json::from_str(&satir).unwrap();
    let t = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(t.contains("REDDEDİLİR"), "{t}");
    assert!(t.contains("ad") && t.contains("fiyat"), "en az 2 karakter ve en az 0.01: {t}");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn doktor_rapor_ve_canli_mod_yol_parametresi() {
    use p0_g1_ortak::*;
    let ev = Ev::yeni("d6-aspnet");
    let u = Uygulama::yeni();
    let p = ev.proje(ORNEK, "a");
    let (_, d) = p.cikti(&["doktor"]);
    assert!(d.contains("ASP.NET Core (C#)") && d.contains("Magaza.Api"), "{d}");
    let r = p.ok(&["rapor", ".sezgi"]);
    assert!(r.contains("aspnet@0.1.0"), "{r}");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["urunler.*:oku"], &[]);
    let (s, hata) = p.mcp(&t).cagir(json!({ "kapi": "GET /api/Urunler/:id", "parametreler": { "id": "7" }, "sorgu": {} }));
    assert!(!hata, "{s}");
    let g = u.gelenler.lock().unwrap();
    assert_eq!(g.last().map(|x| (x.yontem.as_str(), x.yol.as_str())), Some(("GET", "/api/Urunler/7")), "rota kısıtlı yol parametresi yerleştirilir");
}

/// Geçici bir ASP.NET Core projesi yazar ve haritalar
fn proje(ad: &str, dosyalar: &[(&str, &str)]) -> (Value, PathBuf) {
    let kok = std::env::temp_dir().join(format!("sezgi-d6-aspnet-gecici-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    for (yol, m) in dosyalar {
        let p = kok.join(yol);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, m).unwrap();
    }
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", kok.join(".sezgi").to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    (json_oku(&kok.join(".sezgi").join("kapilar.json")), kok)
}

const WEB_PROJESI: &str = "<Project Sdk=\"Microsoft.NET.Sdk.Web\">\n  <PropertyGroup>\n    <TargetFramework>net8.0</TargetFramework>\n  </PropertyGroup>\n</Project>\n";

#[test]
fn geleneksel_rota_ara_katman_ve_genel_filtre() {
    // Çözüm klasörü: proje alt klasörde; geleneksel rota; işaretsiz uç açık ama kesin değil; 401 üreten ara katman
    let (h, kok) = proje(
        "geleneksel",
        &[
            ("src/Vitrin/Vitrin.csproj", WEB_PROJESI),
            (
                "src/Vitrin/Program.cs",
                "var builder = WebApplication.CreateBuilder(args);\nbuilder.Services.AddControllersWithViews();\nvar app = builder.Build();\napp.UseMiddleware<AnahtarDenetimi>();\napp.MapControllerRoute(name: \"default\", pattern: \"{controller=Home}/{action=Index}/{id?}\");\napp.MapGet(\"/acik\", () => \"x\");\napp.Run();\n",
            ),
            (
                "src/Vitrin/AnahtarDenetimi.cs",
                "public class AnahtarDenetimi(RequestDelegate sonraki)\n{\n    public async Task InvokeAsync(HttpContext c)\n    {\n        if (!c.Request.Headers.ContainsKey(\"X-Anahtar\")) { c.Response.StatusCode = 401; return; }\n        await sonraki(c);\n    }\n}\n",
            ),
            (
                "src/Vitrin/Controllers/HomeController.cs",
                "using Microsoft.AspNetCore.Mvc;\npublic class HomeController : Controller\n{\n    public IActionResult Index() => View();\n    public IActionResult Detay(int id) => View();\n    [HttpPost]\n    public IActionResult Kaydet(string ad) => RedirectToAction(nameof(Index));\n}\n",
            ),
        ],
    );
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(idler, ["GET /Home/Detay/:id", "GET /Home/Index", "POST /Home/Kaydet", "GET /acik"]);
    let e = uc(&h, "GET /acik");
    assert_eq!((e["giris"].as_str(), e["giris_kaniti"].as_str()), (Some("herkese_acik"), Some("varsayim_bilinmeyen_guard")), "401 üreten ara katman");
    assert!(h["ozet"]["genel_guardlar"].to_string().contains("UseMiddleware<AnahtarDenetimi>"));
    assert_eq!(h["ozet"]["uygulama"], "Vitrin");
    // [ApiController] yok: çatı doğrulamaz, zorunluluk tahmin
    assert_eq!(alanlar(uc(&h, "POST /Home/Kaydet"), "sorgu")["ad"]["zorunlu"], false);
    let _ = std::fs::remove_dir_all(&kok);

    // MVC genel AuthorizeFilter ve MapControllers().RequireAuthorization(); özel yetki filtresi; ara katman yok
    let (h, kok) = proje(
        "filtre",
        &[
            ("Api.csproj", WEB_PROJESI),
            (
                "Program.cs",
                "var builder = WebApplication.CreateBuilder(args);\nbuilder.Services.AddControllers(o => o.Filters.Add(new AuthorizeFilter()));\nvar app = builder.Build();\napp.MapControllers();\napp.MapGet(\"/durum\", () => \"ok\");\napp.Run();\n",
            ),
            (
                "AnahtarGerekli.cs",
                "public class AnahtarGerekliAttribute : Attribute, IAuthorizationFilter\n{\n    public void OnAuthorization(AuthorizationFilterContext c) { c.Result = new UnauthorizedResult(); }\n}\n",
            ),
            (
                "Controllers/HesapController.cs",
                "[ApiController]\n[Route(\"[controller]\")]\npublic class HesapController : ControllerBase\n{\n    [HttpGet]\n    public string Ben() => \"\";\n    [HttpGet(\"acik\")]\n    [AllowAnonymous]\n    public string Acik() => \"\";\n}\n",
            ),
            (
                "Controllers/AnahtarController.cs",
                "[Route(\"anahtar\")]\npublic class AnahtarController : ControllerBase\n{\n    [HttpGet, AnahtarGerekli]\n    public string Al() => \"\";\n}\n",
            ),
        ],
    );
    let karar = |id: &str| (uc(&h, id)["giris"].as_str().unwrap().to_string(), uc(&h, id)["giris_kaniti"].as_str().unwrap().to_string());
    assert_eq!(karar("GET /Hesap"), ("gerekli".into(), "kesin".into()), "MVC genel AuthorizeFilter");
    assert_eq!(karar("GET /Hesap/acik"), ("herkese_acik".into(), "kesin".into()));
    assert_eq!(karar("GET /anahtar"), ("gerekli".into(), "kesin".into()), "genel filtre özel filtreden önce gelir");
    assert_eq!(karar("GET /durum"), ("herkese_acik".into(), "varsayim".into()), "MVC filtresi Minimal API'ye uygulanmaz");
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn katmanli_cozum_ayni_adli_ic_tipler_ve_donen_grup() {
    // Çözüm klasörü (.sln): web projesi Api/, altyapı katmanı ../Altyapi (ProjectReference); her özellikte `Create.Command`
    // (RealWorld düzeni) ve grubu döndüren uzantı metodu (`app.MapSiparisler().RequireAuthorization()`, eShop düzeni)
    let kok = std::env::temp_dir().join(format!("sezgi-d6-aspnet-katman-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    let dosyalar: [(&str, &str); 7] = [
        ("Magaza.sln", "Microsoft Visual Studio Solution File, Format Version 12.00\n"),
        (
            "Api/Api.csproj",
            "<Project Sdk=\"Microsoft.NET.Sdk.Web\">\n  <ItemGroup>\n    <ProjectReference Include=\"..\\Altyapi\\Altyapi.csproj\" />\n  </ItemGroup>\n</Project>\n",
        ),
        (
            "Api/Program.cs",
            "var builder = WebApplication.CreateBuilder(args);\nvar app = builder.Build();\napp.MapControllers();\napp.MapSiparisler().RequireAuthorization();\napp.Run();\n\npublic static class SiparisUclari\n{\n    public static RouteGroupBuilder MapSiparisler(this IEndpointRouteBuilder app)\n    {\n        var g = app.MapGroup(\"/siparisler\");\n        g.MapGet(\"/\", () => Results.Ok());\n        return g;\n    }\n}\n",
        ),
        (
            "Api/Ozellikler/Kullanicilar.cs",
            "namespace Api.Ozellikler.Kullanicilar;\n\n[ApiController]\n[Route(\"kullanicilar\")]\npublic class KullanicilarController(IMediator mediator) : ControllerBase\n{\n    [HttpPost]\n    public Task<int> Olustur([FromBody] Create.Command komut) => mediator.Send(komut);\n}\n\npublic class Create\n{\n    public record Veri(string? Ad, string? Eposta);\n    public record Command(Veri Kullanici) : IRequest<int>;\n    public class Isleyici(MagazaDb db) : IRequestHandler<Command, int>\n    {\n        public async Task<int> Handle(Command k, CancellationToken ct)\n        {\n            db.Kullanicilar.Add(new Kullanici());\n            return await db.SaveChangesAsync(ct);\n        }\n    }\n}\n",
        ),
        (
            "Api/Ozellikler/Makaleler.cs",
            "namespace Api.Ozellikler.Makaleler;\n\n[ApiController]\n[Route(\"makaleler\")]\npublic class MakalelerController(IMediator mediator) : ControllerBase\n{\n    [HttpPost]\n    public Task<int> Olustur([FromBody] Create.Command komut) => mediator.Send(komut);\n}\n\npublic class Create\n{\n    public record Veri(string? Baslik, string? Govde, string[]? Etiketler);\n    public record Command(Veri Makale) : IRequest<int>;\n    public class Isleyici(IMakaleDeposu depo) : IRequestHandler<Command, int>\n    {\n        public Task<int> Handle(Command k, CancellationToken ct) => depo.Ekle(k.Makale.Baslik!);\n    }\n}\n",
        ),
        ("Altyapi/Altyapi.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n"),
        (
            "Altyapi/Veri.cs",
            "public class Kullanici { public int Id { get; set; } }\npublic class Makale { public int Id { get; set; } public string Baslik { get; set; } = \"\"; }\npublic class MagazaDb : DbContext\n{\n    public DbSet<Kullanici> Kullanicilar => Set<Kullanici>();\n    public DbSet<Makale> Makaleler => Set<Makale>();\n}\npublic interface IMakaleDeposu { Task<int> Ekle(string baslik); }\npublic class MakaleDeposu(MagazaDb db) : IMakaleDeposu\n{\n    public async Task<int> Ekle(string baslik)\n    {\n        db.Makaleler.Add(new Makale { Baslik = baslik });\n        return await db.SaveChangesAsync();\n    }\n}\n",
        ),
    ];
    for (yol, m) in dosyalar {
        let p = kok.join(yol);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, m).unwrap();
    }
    let api = kok.join("Api");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", api.to_str().unwrap(), "--cikti", api.join(".sezgi").to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    assert!(String::from_utf8_lossy(&c.stdout).contains("kardeş paket: Altyapi"), "{}", String::from_utf8_lossy(&c.stdout));
    let h = json_oku(&api.join(".sezgi").join("kapilar.json"));
    let ic = |id: &str, dis: &str| alanlar(uc(&h, id), "govde")[dis]["ic"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert_eq!(ic("POST /kullanicilar", "kullanici"), ["ad", "eposta"], "kendi ad alanındaki Create.Command ve kardeş Veri");
    assert_eq!(ic("POST /makaleler", "makale"), ["baslik", "etiketler", "govde"]);
    // Send -> kendi ad alanındaki işleyici; altyapı katmanındaki DbContext ve arayüzün uygulaması
    assert_eq!(uc(&h, "POST /kullanicilar")["veri"]["yazar"], json!(["Kullanici"]));
    assert_eq!(uc(&h, "POST /makaleler")["veri"]["yazar"], json!(["Makale"]));
    let e = uc(&h, "GET /siparisler/");
    assert_eq!((e["giris"].as_str(), e["giris_kaniti"].as_str()), (Some("gerekli"), Some("kesin")), "uzantının döndürdüğü gruba RequireAuthorization");
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn web_sdk_algilama_ve_alt_klasordeki_proje() {
    let kok = std::env::temp_dir().join(format!("sezgi-d6-aspnet-ust-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    std::fs::create_dir_all(kok.join("Sunucu")).unwrap();
    std::fs::write(kok.join("Sunucu").join("Sunucu.csproj"), WEB_PROJESI).unwrap();
    std::fs::create_dir_all(kok.join("Kutuphane")).unwrap();
    std::fs::write(kok.join("Kutuphane").join("Kutuphane.csproj"), "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n").unwrap();
    let ic = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.join("Kutuphane").to_str().unwrap(), "--deneme"]).output().unwrap();
    assert!(!ic.status.success(), "Web SDK'sız sınıf kütüphanesi bir uygulama değildir");
    assert!(String::from_utf8_lossy(&ic.stderr).contains("Microsoft.NET.Sdk.Web"), "{}", String::from_utf8_lossy(&ic.stderr));
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--deneme"]).output().unwrap();
    let m = String::from_utf8_lossy(&c.stdout).to_string();
    assert!(c.status.success(), "üst klasördeki çözümde web projesi alt klasörde: {m}{}", String::from_utf8_lossy(&c.stderr));
    let _ = std::fs::remove_dir_all(&kok);
}
