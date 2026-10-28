//! PHP + Laravel adaptörü: rotalar (gruplar, önekler, kaynaklar, controller biçimleri, yükleyiciler), giriş ve rol
//! (ara katman takma adları, Spatie, Gate / politika), Form Request ve satır içi doğrulama, API Resource yanıtı,
//! Eloquent veri bağları, göç kaskadları, olaylar ve işler, kanıt grafı, kart, doktor ve bayatlık; önizleme, canlı mod
//! ve kesinlik değişmezleri dilden bağımsız çalışır.
mod p0_g1_ortak;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

/// Örnek uygulamayı geçici bir klasöre kopyalar (doktor ve bayatlık için çıktı uygulamanın içinde)
fn kopya(ad: &str, etiket: &str) -> PathBuf {
    let hedef = std::env::temp_dir().join(format!("sezgi-d6-laravel-{etiket}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&hedef);
    fn kopyala(a: &Path, b: &Path) {
        std::fs::create_dir_all(b).unwrap();
        for g in std::fs::read_dir(a).unwrap().flatten() {
            let p = g.path();
            if p.is_dir() {
                kopyala(&p, &b.join(g.file_name()));
            } else {
                std::fs::copy(&p, b.join(g.file_name())).unwrap();
            }
        }
    }
    kopyala(&ornek(ad), &hedef);
    hedef
}

fn sezgi(args: &[&str], ev: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(args)
        .env("HOME", ev)
        .env("USERPROFILE", ev)
        .env("APPDATA", ev.join("ayar"))
        .env("XDG_CONFIG_HOME", ev.join("ayar"))
        .env("SEZGI_ANAHTARLIK", "kapali")
        .env_remove("SEZGI_TOKEN")
        .output()
        .unwrap()
}

/// Haritayı kurar: (harita, çıktı klasörü)
fn harita(ad: &str, etiket: &str) -> (Value, PathBuf) {
    let kok = kopya(ad, etiket);
    let cikti = kok.join(".sezgi");
    let c = sezgi(&["init", kok.to_str().unwrap()], &kok.join(".ev"));
    assert!(c.status.success(), "{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    (serde_json::from_str(&std::fs::read_to_string(cikti.join("kapilar.json")).unwrap()).unwrap(), cikti)
}

fn temizle(cikti: &Path) {
    if let Some(k) = cikti.parent() {
        let _ = std::fs::remove_dir_all(k);
    }
}

fn uc<'a>(h: &'a Value, id: &str) -> &'a Value {
    h["endpointler"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("{id} yok"))
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

fn metinler(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

#[test]
fn rotalar_onekler_kaynaklar_ve_yukleyiciler() {
    let (h, cikti) = harita("d6_laravel_temel", "rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    let beklenen = [
        "GET /",
        "GET /admin/health",
        "GET /api/v1/beta",
        "POST /api/v1/legacy",
        "POST /api/v1/login",
        "GET /api/v1/me",
        "POST /api/v1/orders",
        "POST /api/v1/orders/:order/ship",
        "GET /api/v1/ping",
        "GET /api/v1/posts",
        "POST /api/v1/posts",
        "DELETE /api/v1/posts/:post",
        "GET /api/v1/posts/:post",
        "PATCH /api/v1/posts/:post",
        "PUT /api/v1/posts/:post",
        "GET /api/v1/posts/:post/comments",
        "POST /api/v1/posts/:post/comments",
        "DELETE /api/v1/posts/:post/force",
        "POST /api/v1/register",
        "GET /api/v1/reports/daily",
        "GET /api/v1/tags",
        "GET /api/v1/tags/:tag",
        "PUT /api/v1/tags/:tag",
        "GET /api/v1/team/stats",
        "GET /dashboard",
        "GET /login",
        "POST /login",
        "POST /logout",
        "GET /up",
    ];
    // withRouting(apiPrefix: 'api/v1'), apiResource (update = PUT + PATCH), iç içe kaynak + only, prefix/name grupları,
    // Route::controller(...)->group, require ile yüklenen auth.php, health: '/up'; testler ve console.php rota değildir
    assert_eq!(idler, beklenen);
    let o = &h["ozet"];
    assert_eq!(o["adaptor"], "laravel@0.1.0");
    assert_eq!(o["uygulama"], "acme/kitapci");
    assert_eq!(o["genel_onek"], "/api/v1");
    assert_eq!(o["ana_uygulama"], "bootstrap/app.php");
    assert_eq!(o["kosullu_kapilar"], json!(["GET /api/v1/beta"]), "if (config(...)) altındaki kayıt");
    // Yükleyicisi olmayan rota dosyası ikincil
    let admin = uc(&h, "GET /admin/health");
    assert_eq!(admin["ana"], false);
    assert_eq!(admin["uygulama"], "routes/admin.php");
    // Controller biçimleri: [C::class, 'm'], invokable, Route::controller + 'm', kapanış
    let me = uc(&h, "GET /api/v1/me");
    assert_eq!((me["kontrolcu"].as_str(), me["fonksiyon"].as_str()), (Some("ProfileController"), Some("__invoke")));
    assert_eq!(me["ozet"], "Oturumdaki kullanıcı.", "PHPDoc özeti");
    assert_eq!(uc(&h, "GET /api/v1/tags/:tag")["fonksiyon"], "show");
    assert_eq!(uc(&h, "GET /api/v1/ping")["dosya"], "routes/api.php");
    // Yol parametreleri: model bağlama ve whereNumber
    assert_eq!(uc(&h, "GET /api/v1/posts/:post")["parametreler"], json!([{ "ad": "post", "tip": "Post" }]));
    assert_eq!(uc(&h, "DELETE /api/v1/posts/:post/force")["parametreler"][0]["tip"], "integer");
    // Rota adları uygulamanın işlem kimliğidir (OpenAPI operationId)
    let oa: Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("openapi.json")).unwrap()).unwrap();
    assert_eq!(oa["paths"]["/api/v1/posts/{post}"]["get"]["operationId"], "posts.show");
    assert_eq!(oa["paths"]["/api/v1/posts"]["post"]["operationId"], "posts.store");
    // Kart: composer.json sürümü, işlemler
    let kart: Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("benlik.json")).unwrap()).unwrap();
    assert_eq!(kart["app"]["version"], "1.4.0");
    let islemler: Vec<&str> = kart["operations"].as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap()).collect();
    assert!(islemler.contains(&"post.store") && islemler.contains(&"tag.update"), "{islemler:?}");
    assert!(!islemler.iter().any(|i| i.contains("admin")), "ikincil kapı karta girmez: {islemler:?}");
    temizle(&cikti);
}

#[test]
fn giris_ve_roller_ara_katmandan_ve_politikadan() {
    let (h, cikti) = harita("d6_laravel_temel", "giris");
    let karar = |id: &str| (uc(&h, id)["giris"].as_str().unwrap().to_string(), uc(&h, id)["giris_kaniti"].as_str().unwrap().to_string());
    let beklenen: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        // auth:sanctum çatının Authenticate sınıfına çözülür: kesin
        ("GET /api/v1/me", ("gerekli", "kesin")),
        ("POST /api/v1/posts", ("gerekli", "kesin")),
        // web.php: auth + verified
        ("GET /dashboard", ("gerekli", "kesin")),
        ("POST /logout", ("gerekli", "kesin")),
        // guest grubu ve ara katmansız uçlar: açık sayılır ama yalnızca çıkarım
        ("GET /login", ("herkese_acik", "varsayim")),
        ("POST /api/v1/login", ("herkese_acik", "varsayim")),
        ("GET /api/v1/tags", ("herkese_acik", "varsayim")),
        ("GET /up", ("herkese_acik", "varsayim")),
    ]);
    for (id, (g, k)) in beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string()), "{id}");
    }
    // Spatie role ara katmanı (bootstrap/app.php alias) roller kesin; politika rolü (TagPolicy hasRole) yorum
    let rapor = uc(&h, "GET /api/v1/reports/daily");
    assert_eq!(metinler(&rapor["roller"]), ["admin", "editor"]);
    assert!(rapor["roller_kaniti"].is_null(), "{rapor}");
    let etiket = uc(&h, "PUT /api/v1/tags/:tag");
    assert_eq!(metinler(&etiket["roller"]), ["admin", "curator", "editor"]);
    assert_eq!(etiket["roller_kaniti"], "varsayim");
    assert!(metinler(&etiket["guardlar"]).contains(&"can:update,tag".to_string()));
    // $this->authorize('update', $post) -> PostPolicy::update -> hasAnyRole(['editor', 'admin'])
    let guncelle = uc(&h, "PUT /api/v1/posts/:post");
    assert_eq!(metinler(&guncelle["roller"]), ["admin", "editor"]);
    assert_eq!(guncelle["roller_kaniti"], "varsayim");
    // Kayıtsız takma ad (legacy.check) tanınmayan guard olarak kalır; girişin kesinliği auth:sanctum'dan
    assert!(metinler(&uc(&h, "POST /api/v1/legacy")["guardlar"]).contains(&"legacy.check".to_string()));
    temizle(&cikti);
}

#[test]
fn form_request_ve_satir_ici_dogrulama() {
    let (h, cikti) = harita("d6_laravel_temel", "girdi");
    let g = alanlar(uc(&h, "POST /api/v1/posts"), "govde");
    assert_eq!(uc(&h, "POST /api/v1/posts")["govde"][0]["sema"]["dto"], "StorePostRequest");
    let zorunlu = |m: &serde_json::Map<String, Value>, a: &str| m[a]["zorunlu"].as_bool().unwrap();
    assert!(zorunlu(&g, "title") && zorunlu(&g, "body") && zorunlu(&g, "status"));
    assert!(!zorunlu(&g, "published_at") && !zorunlu(&g, "category") && !zorunlu(&g, "tags"));
    assert_eq!(kurallar(&g["title"]), ["IsString()", "MaxLength(200)"]);
    // new Enum(PostStatus::class): backed enum değerleri; Rule::in([...])
    assert_eq!(g["status"]["degerler"], json!(["draft", "published"]));
    assert_eq!(g["category"]["degerler"], json!(["news", "blog"]));
    assert_eq!(g["published_at"]["tip"], "datetime");
    // tags.* kuralları her öğeye
    assert!(kurallar(&g["tags"]).contains(&"IsInt({ each: true })".to_string()), "{:?}", kurallar(&g["tags"]));
    // UpdatePostRequest: array_merge(parent::rules(), [...]) ile üst sınıfın kuralları ve 'sometimes'
    let u = alanlar(uc(&h, "PUT /api/v1/posts/:post"), "govde");
    assert!(!zorunlu(&u, "title") && zorunlu(&u, "body") && u.contains_key("pinned"));
    assert_eq!(u["pinned"]["tip"], "boolean");
    // $request->validate([...]): iç içe nesne ve nesne dizisi, dosya ve özel kural notları
    let y = alanlar(uc(&h, "POST /api/v1/posts/:post/comments"), "govde");
    assert_eq!(kurallar(&y["body"]), ["IsString()", "MinLength(3)"]);
    assert_eq!(kurallar(&y["rating"]), ["nullable()", "IsInt()", "Min(1)", "Max(5)"]);
    assert_eq!(y["meta"]["tip"], "object");
    assert!(y["meta"]["zorunlu"].as_bool().unwrap(), "zorunlu alt alan nesneyi zorunlu yapar");
    assert_eq!(y["meta"]["ic"]["source"]["degerler"], json!(["web", "mobile"]));
    assert_eq!(kurallar(&y["meta"]["ic"]["lang"]), ["IsString()", "MinLength(2)", "MaxLength(2)"]);
    assert_eq!(y["attachments"]["tip"], "array");
    assert!(y["attachments"]["ic"]["url"]["zorunlu"].as_bool().unwrap());
    assert_eq!(y["avatar"]["tip"], "file");
    assert_eq!(kurallar(&y["author_id"]), ["exists:users,id"], "çevrilemeyen kural olduğu gibi kalır");
    let notlar = metinler(&h["ozet"]["denetlenemeyen"]["app/Http/Controllers/Api/CommentController.php:17"]);
    assert!(notlar.iter().any(|n| n.contains("'avatar'")) && notlar.iter().any(|n| n.contains("Uppercase")), "{notlar:?}");
    // required_if belirsiz; GET'te doğrulanan alanlar sorgudur
    let k = alanlar(uc(&h, "POST /api/v1/register"), "govde");
    assert!(k["role"]["belirsiz"].as_bool().unwrap() && !zorunlu(&k, "role"));
    let s = alanlar(uc(&h, "GET /api/v1/posts"), "sorgu");
    assert_eq!(kurallar(&s["per_page"]), ["IsInt()", "Min(1)", "Max(100)"]);
    assert!(uc(&h, "GET /api/v1/posts")["govde"].as_array().unwrap().is_empty());
    // Doğrulamasız okunan alan: zorunluluğu belirsiz
    let o = alanlar(uc(&h, "POST /api/v1/orders"), "govde");
    assert_eq!(o["total"]["tip"], "integer");
    assert!(o["total"]["belirsiz"].as_bool().unwrap());
    temizle(&cikti);
}

#[test]
fn api_resource_yaniti() {
    let (h, cikti) = harita("d6_laravel_temel", "yanit");
    let y = &uc(&h, "POST /api/v1/posts")["yanit"];
    assert_eq!(y["kaynak"], "JsonResource");
    assert_eq!(y["durum"], 201, "->response()->setStatusCode(201)");
    let veri = &y["sema"]["alanlar"]["data"];
    assert_eq!(veri["tip"], "object", "JsonResource 'data' sarmalı");
    assert_eq!(veri["ic"]["id"]["tip"], "integer");
    assert_eq!(veri["ic"]["author"]["zorunlu"], false, "whenLoaded koşullu");
    assert!(veri["ic"]["author"]["ic"]["email"].is_object(), "iç kaynak (UserResource)");
    // ResourceCollection ($collects): dizi
    let liste = &uc(&h, "GET /api/v1/posts")["yanit"]["sema"]["alanlar"]["data"];
    assert_eq!(liste["tip"], "array");
    assert!(liste["ic"]["title"].is_object());
    // response()->json([...], 201): yorum
    let j = &uc(&h, "POST /api/v1/register")["yanit"];
    assert_eq!((j["kaynak"].as_str(), j["kesin"].as_bool(), j["durum"].as_u64()), (Some("response()->json"), Some(false), Some(201)));
    temizle(&cikti);
}

#[test]
fn eloquent_veri_baglari_kaskadlar_ve_dna() {
    let (h, cikti) = harita("d6_laravel_temel", "veri");
    let veri = |id: &str| (metinler(&uc(&h, id)["veri"]["okur"]), metinler(&uc(&h, id)["veri"]["yazar"]));
    // $request->user()->posts()->create(...) ve ->tags()->sync(...) (pivot); exists:tags,id okuma
    assert_eq!(veri("POST /api/v1/posts"), (vec!["Tag".to_string()], vec!["Post".to_string(), "post_tag".to_string()]));
    // Servis zinciri: $this->posts->remove($post) -> comments()->delete(), $post->delete()
    let (_, yazar) = veri("DELETE /api/v1/posts/:post");
    assert_eq!(yazar, ["Comment", "Post"]);
    assert_eq!(veri("POST /api/v1/orders").1, ["Order"]);
    assert!(uc(&h, "GET /api/v1/reports/daily")["veri"]["eksik_olabilir"].as_bool().unwrap(), "DB::select ham SQL");
    // Göçlerdeki kaskadlar model adlarıyla
    let kaskad: Vec<(String, String)> =
        h["ozet"]["kaskadlar"].as_array().unwrap().iter().map(|k| (k[0].as_str().unwrap().to_string(), k[1].as_str().unwrap().to_string())).collect();
    for (e, c) in [("User", "Post"), ("Post", "Comment"), ("Post", "post_tag")] {
        assert!(kaskad.contains(&(e.to_string(), c.to_string())), "{e} -> {c}: {kaskad:?}");
    }
    // DNA: sahiplik denetimi, yumuşak silme (SoftDeletes), kaskadla dolaylı yazma, servisteki olay
    let dna = &h["ozet"]["dna"]["app/Http/Controllers/Api/PostController.php:57"];
    assert_eq!(dna["sahiplik_denetimi"][0]["durum"], 403);
    assert_eq!(dna["sahiplik_denetimi"][0]["guven"], "certain");
    assert_eq!(dna["yumusak_silme"], "app/Services/PostService.php:13");
    assert_eq!(dna["geri_alinabilir"], "kismen");
    assert_eq!(dna["dolayli_yazar"][0]["tablo"], "post_tag");
    assert_eq!(dna["tetikler"][0]["ad"], "PostRemoved");
    // abort_unless, ValidationException::withMessages, $this->authorize, iki yollu ifade
    assert_eq!(h["ozet"]["dna"]["routes/web.php:7"]["hatalar"][0]["kosul"], "!(auth()->check())");
    assert_eq!(h["ozet"]["dna"]["app/Http/Controllers/Api/AuthController.php:14"]["hatalar"][0]["durum"], 422);
    assert_eq!(h["ozet"]["dna"]["app/Http/Controllers/Api/PostController.php:49"]["hatalar"][0]["istisna"], "AuthorizationException");
    let ship = &h["ozet"]["dna"]["app/Http/Controllers/Api/OrderController.php:21"];
    assert_eq!(ship["dallar"][0]["kosul"], "$order->paid");
    assert_eq!(ship["tetikler"][0], json!({ "tur": "event", "ad": "OrderShipped", "kanit": "app/Http/Controllers/Api/OrderController.php:24", "guven": "inferred" }));
    assert_eq!(h["ozet"]["dna"]["app/Http/Controllers/Api/OrderController.php:13"]["tetikler"][0]["tur"], "job");
    // Olay ve iş düğümleri: dinleyici (keşif) ve kuyruk işi; işin tetiklediği olay
    let olay = &h["ozet"]["olaylar"]["event:OrderShipped"];
    assert_eq!(olay["dinleyiciler"][0]["fonksiyon"], "SendShipmentNotification.handle");
    assert_eq!(olay["silinen"], json!(["Comment"]));
    assert_eq!(h["ozet"]["olaylar"]["job:ProcessOrder"]["tetikler"][0]["ad"], "OrderShipped");
    // Kart: olay / iş düğümleri
    let kart: Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("benlik.json")).unwrap()).unwrap();
    let dugumler: Vec<&str> = kart["x-nodes"].as_array().unwrap().iter().map(|d| d["id"].as_str().unwrap()).collect();
    assert!(dugumler.contains(&"event:OrderShipped") && dugumler.contains(&"job:ProcessOrder"), "{dugumler:?}");
    temizle(&cikti);
}

#[test]
fn kanit_grafi_tablolar_ve_rota_dosyalari() {
    let (_h, cikti) = harita("d6_laravel_temel", "kanit");
    let depo: Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("kanit.json")).unwrap()).unwrap();
    let onermeler = depo["onermeler"].as_array().unwrap();
    let var = |varlik: &str, yuklem: &str| onermeler.iter().any(|o| o["varlik"] == varlik && o["yuklem"] == yuklem);
    // Tablo ve sütunlar: model ($table / varsayılan ad) ve göç (Schema::create sütunları)
    assert!(var("db:Post", "TABLO") && var("db:Order", "TABLO"));
    assert!(var("field:Post.title", "VAR"));
    let tablo = onermeler.iter().find(|o| o["varlik"] == "db:Order" && o["yuklem"] == "TABLO").unwrap();
    assert_eq!(tablo["deger"]["tablo"], "shop_orders");
    // Rota dosyası grafı: routes/api.php bootstrap/app.php'ye, auth.php web.php'ye bağlı
    let bagli = |a: &str, b: &str| onermeler.iter().any(|o| o["varlik"] == a && o["yuklem"] == "DEPENDS_ON" && o["hedef"] == b);
    assert!(bagli("mod:routes/api.php", "mod:bootstrap/app.php"));
    assert!(bagli("mod:routes/auth.php", "mod:routes/web.php"));
    assert!(bagli("api:GET /login", "mod:routes/auth.php"));
    // Çelişen kanıt yok: aynı tablo ve alan iki kaynaktan (model, göç) aynı değerle desteklenir
    let celiski: Vec<&Value> = onermeler.iter().filter(|o| o["hesap"]["celiski"].as_array().is_some_and(|c| !c.is_empty())).collect();
    assert!(celiski.is_empty(), "{celiski:?}");
    temizle(&cikti);
}

#[test]
fn laravel_10_kernel_routeserviceprovider_ve_ad_alani() {
    let (h, cikti) = harita("d6_laravel_eski", "eski");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    for id in [
        "GET /api/v2/courses",
        "PATCH /api/v2/courses/:id",
        "PUT /api/v2/courses/:id",
        "GET /api/v2/teachers/create",
        "DELETE /api/v2/users/:user",
        "GET /api/webhook",
        "DELETE /api/webhook",
        "GET /courses/create",
        "GET /courses/:course/edit",
        "GET /about",
    ] {
        assert!(idler.contains(&id), "{id}: {idler:?}");
    }
    // Route::resource('courses', 'CourseController')->except(['destroy']) ve Route::any (5 yöntem)
    assert!(!idler.contains(&"DELETE /courses/:course"));
    assert_eq!(idler.iter().filter(|i| i.ends_with("/api/webhook")).count(), 5);
    assert_eq!(h["ozet"]["genel_onek"], "/api");
    assert_eq!(h["ozet"]["ana_uygulama"], "app/Providers/RouteServiceProvider.php");
    assert_eq!(h["ozet"]["genel_guardlar"], json!(["TrustProxies", "HandleCors"]));
    // 'CourseController@index' grup ad alanıyla ($this->namespace) çözülür; Admin alt ad alanı
    assert_eq!(uc(&h, "GET /api/v2/courses")["dosya"], "app/Http/Controllers/CourseController.php");
    assert_eq!(uc(&h, "DELETE /api/v2/users/:user")["dosya"], "app/Http/Controllers/Admin/UserController.php");
    // Kernel takma adı: auth -> App\Http\Middleware\Authenticate (çatınınkinden türer): kesin
    assert_eq!(uc(&h, "GET /api/v2/courses")["giris_kaniti"], "kesin");
    // Yapıcıdaki $this->middleware('auth')->except(['index', 'show'])
    assert_eq!(uc(&h, "GET /courses")["giris"], "herkese_acik");
    assert_eq!((uc(&h, "GET /courses/create")["giris"].as_str(), uc(&h, "GET /courses/create")["giris_kaniti"].as_str()), (Some("gerekli"), Some("kesin")));
    // Gate::define('manage-users', fn ($user) => $user->role === 'admin') ve $policies -> CoursePolicy hasRole
    assert_eq!(metinler(&uc(&h, "DELETE /api/v2/users/:user")["roller"]), ["admin"]);
    assert_eq!(metinler(&uc(&h, "PUT /courses/:course")["roller"]), ["teacher"]);
    // Route::pattern('id', '[0-9]+')
    assert_eq!(uc(&h, "PUT /api/v2/courses/:id")["parametreler"][0]["tip"], "integer");
    // $this->validate($request, [...]); belongsToMany pivot yazması; $listen dinleyicisi
    assert!(alanlar(uc(&h, "POST /courses"), "govde")["title"]["zorunlu"].as_bool().unwrap());
    assert_eq!(metinler(&uc(&h, "POST /api/v2/courses/:id/enroll")["veri"]["yazar"]), ["enrollments"]);
    assert_eq!(h["ozet"]["olaylar"]["event:StudentEnrolled"]["dinleyiciler"][0]["fonksiyon"], "NotifyTeacher.handle");
    let kaskad: Vec<String> = h["ozet"]["kaskadlar"].as_array().unwrap().iter().map(|k| format!("{}>{}", k[0].as_str().unwrap(), k[1].as_str().unwrap())).collect();
    assert!(kaskad.contains(&"Course>enrollments".to_string()) && kaskad.contains(&"User>enrollments".to_string()), "{kaskad:?}");
    temizle(&cikti);
}

#[test]
fn doktor_rapor_ve_bayatlik() {
    let (_, cikti) = harita("d6_laravel_temel", "doktor");
    let kok = cikti.parent().unwrap().to_path_buf();
    let ev = kok.join(".ev");
    let d = sezgi(&["doktor", kok.to_str().unwrap()], &ev);
    let m = String::from_utf8_lossy(&d.stdout).to_string() + &String::from_utf8_lossy(&d.stderr);
    assert!(m.contains("Laravel (PHP)") && m.contains("acme/kitapci") && m.contains("harita koddan yeni"), "{m}");
    let r = sezgi(&["rapor", cikti.to_str().unwrap()], &ev);
    let rm = String::from_utf8_lossy(&r.stdout).to_string();
    assert!(rm.contains("laravel@0.1.0") && rm.contains("routes/admin.php: 1 işlem"), "{rm}");
    // PHP dosyası değişince harita bayattır
    let yol = kok.join("app/Http/Controllers/Api/TagController.php");
    let eski = std::fs::read_to_string(&yol).unwrap();
    std::fs::write(&yol, eski.replace("return Tag::all();", "return Tag::query()->get();")).unwrap();
    let d = sezgi(&["doktor", kok.to_str().unwrap()], &ev);
    let m = String::from_utf8_lossy(&d.stdout).to_string() + &String::from_utf8_lossy(&d.stderr);
    assert!(m.contains("TagController.php"), "{m}");
    temizle(&cikti);
}

#[test]
fn desteklenmeyen_klasor_laravel_adayini_gosterir() {
    let kok = std::env::temp_dir().join(format!("sezgi-d6-laravel-aday-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    std::fs::create_dir_all(kok.join("arka")).unwrap();
    std::fs::write(kok.join("arka/composer.json"), r#"{"require": {"laravel/framework": "^11.0"}}"#).unwrap();
    let c = sezgi(&["init", kok.to_str().unwrap()], &kok.join(".ev"));
    let m = String::from_utf8_lossy(&c.stderr).to_string();
    assert!(!c.status.success());
    assert!(m.contains("composer.json'da laravel/framework") && m.contains("arka") && m.contains("(Laravel)"), "{m}");
    let _ = std::fs::remove_dir_all(&kok);
}

/// Önizleme Form Request kurallarıyla reddeder: zorunlu alan eksik, `in` dışındaki değer
#[test]
fn onizleme_form_request_kurallariyla() {
    let (_, cikti) = harita("d6_laravel_temel", "onizle");
    let mut p =
        Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", cikti.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    use std::io::{BufRead, Write};
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": {
        "kapi": "POST /api/v1/posts", "govde": { "title": "Merhaba", "category": "reklam" } } } });
    writeln!(p.stdin.as_mut().unwrap(), "{istek}").unwrap();
    let mut satir = String::new();
    std::io::BufReader::new(p.stdout.take().unwrap()).read_line(&mut satir).unwrap();
    let _ = p.kill();
    let _ = p.wait();
    let v: Value = serde_json::from_str(&satir).unwrap();
    let t = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(t.contains("REDDEDİLİR"), "{t}");
    assert!(t.contains("body") && t.contains("status"), "zorunlu body ve status eksik: {t}");
    temizle(&cikti);
}

/// Canlı mod: Laravel `{post}` yol parametresi isteğe yerleşir, `whereNumber` kısıtlı yol da çağrılır
#[test]
fn canli_mod_yol_parametresi() {
    use p0_g1_ortak::*;
    let ev = Ev::yeni("d6-laravel");
    let u = Uygulama::yeni();
    let p = ev.proje("d6_laravel_temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["post.*:oku"], &[]);
    let (s, hata) = p.mcp(&t).cagir(json!({ "kapi": "GET /api/v1/posts/:post", "parametreler": { "post": 7 } }));
    assert!(!hata, "{s}");
    let g = u.gelenler.lock().unwrap();
    assert_eq!(g.last().map(|x| (x.yontem.as_str(), x.yol.as_str())), Some(("GET", "/api/v1/posts/7")), "genel önek ve yol parametresi");
}

/// Kesinlik değişmezleri Laravel örneklerinde de geçerli (kesinlik.rs yalnızca package.json / pyproject.toml olan
/// örnekleri gezer): kartta "certain" işlemin giriş kararı ve bütün girdileri kesin; haritada belirsiz alan kartta
/// certain olamaz
#[test]
fn laravel_orneklerinde_kesinlik_degismezleri() {
    let mut ihlaller = vec![];
    for ad in ["d6_laravel_temel", "d6_laravel_eski"] {
        let (h, cikti) = harita(ad, &format!("kesinlik-{ad}"));
        let kart: Value = serde_json::from_str(&std::fs::read_to_string(cikti.join("benlik.json")).unwrap()).unwrap();
        let kapilar = h["endpointler"].as_array().unwrap();
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
        temizle(&cikti);
    }
    assert!(ihlaller.is_empty(), "{}", ihlaller.join("\n"));
}

/// Ayna tanıtımı ve ilk temas Laravel projesini tanır: haritasız açılışta çatı "Laravel", sıradaki adım `sezgi init`
#[test]
fn haritasiz_ayna_laravel_projesini_tanir() {
    use p0_g1_ortak::*;
    use std::io::{BufRead, BufReader, Read, Write};
    let ev = Ev::yeni("d6-laravel-tanitim");
    let p = Proje { klasor: kopya("d6_laravel_temel", "tanitim"), ev: ev.kok.join("ev") };
    let mut c = p.komut(&["ayna", "--port", "0"]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    let (port, anahtar) = loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0, "Ayna haritasız açılmadı");
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port: u16 = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let a: String = k.split("#anahtar=").nth(1).unwrap().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            break (port, a);
        }
    };
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "GET /api/sezgi HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Sezgi-Anahtar: {anahtar}\r\nContent-Length: 0\r\n\r\n").unwrap();
    let mut cevap = String::new();
    let _ = s.read_to_string(&mut cevap);
    let _ = c.kill();
    let _ = c.wait();
    let govde = cevap.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("");
    let v: Value = serde_json::from_str(govde).unwrap_or_else(|_| panic!("{cevap}"));
    assert_eq!(v["harita"], false, "{v}");
    assert_eq!(v["cati"], "Laravel", "{v}");
    assert_eq!(v["sonraki"]["komut"], "sezgi init", "{v}");
    assert!(!p.klasor.join(".sezgi").exists(), "haritasız açılış yazmaz");
    let _ = std::fs::remove_dir_all(&p.klasor);
}
