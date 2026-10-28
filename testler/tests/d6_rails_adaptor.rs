//! Dalga 6b · Rails adaptörü: config/routes.rb (resources / resource, only / except, iç içe ve shallow kaynaklar,
//! member / collection, namespace / scope / module, concern, constraints, draw, fiil rotaları, Devise), filtre
//! zincirinden giriş kararı (Devise, HTTP token, Rails 8 kimlik doğrulama), Pundit / CanCanCan rolleri, strong
//! parameters gövdesi (şema tipleri, model doğrulamaları, enum), jbuilder / serileştirici yanıtları, ActiveRecord veri
//! bağı, kaskadlar, geri çağrılar ve iş / posta tetikleri. Kart, OpenAPI, kanıt ve doktor dilden bağımsız çalışır.
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap().flatten() {
        let p = g.path();
        if p.is_dir() {
            kopyala(&p, &hedef.join(g.file_name()));
        } else {
            std::fs::copy(&p, hedef.join(g.file_name())).unwrap();
        }
    }
}

/// Örneği geçici klasöre kopyalar ve init çalıştırır. Dönen: (uygulama kökü, harita)
fn haritala(ad: &str) -> (PathBuf, Value) {
    let kok = std::env::temp_dir().join(format!("sezgi-d6-rails-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    let ornek = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("d6_rails_blog");
    kopyala(&ornek, &kok.join("blog"));
    std::fs::create_dir_all(kok.join("ev")).unwrap();
    let uygulama = kok.join("blog");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", "--paylasimsiz"])
        .current_dir(&uygulama)
        .env("HOME", kok.join("ev"))
        .env("APPDATA", kok.join("ev"))
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    let h = serde_json::from_str(&std::fs::read_to_string(uygulama.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    (uygulama, h)
}

fn uc<'a>(h: &'a Value, id: &str) -> &'a Value {
    h["endpointler"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("{id} yok"))
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn rotalar_kaynaklar_ve_kapsamlar() {
    let (kok, h) = haritala("rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    let beklenen = [
        "GET /",
        "GET /about",
        // draw :admin (constraints altında): except: [:new, :edit] -> beş eylem, update PATCH ve PUT
        "GET /admin/rapor",
        "GET /admin/users",
        "POST /admin/users",
        "DELETE /admin/users/:id",
        "GET /admin/users/:id",
        "PATCH /admin/users/:id",
        "PUT /admin/users/:id",
        // shallow: true -> üye eylemi üst kaynak olmadan
        "DELETE /api/v1/likes/:id",
        // new / edit: denetleyicide eylem ya da şablon yok -> kapı değil
        "GET /api/v1/posts",
        "POST /api/v1/posts",
        "DELETE /api/v1/posts/:id",
        "GET /api/v1/posts/:id",
        "PATCH /api/v1/posts/:id",
        "PUT /api/v1/posts/:id",
        "POST /api/v1/posts/:id/publish",
        // concern :commentable
        "GET /api/v1/posts/:post_id/comments",
        "POST /api/v1/posts/:post_id/comments",
        "POST /api/v1/posts/:post_id/likes",
        "GET /api/v1/posts/search",
        // resource :profile (tekil): ProfilesController
        "GET /api/v1/profile",
        "PATCH /api/v1/profile",
        "PUT /api/v1/profile",
        "DELETE /comments/:id",
        // scope "/hesap", module: "hesap"
        "GET /hesap/ozet",
        "DELETE /session",
        "POST /session",
        "GET /session/new",
        // Rails'in kendi sağlık ucu
        "GET /up",
        // devise_for :users (database_authenticatable, registerable, recoverable)
        "DELETE /users",
        "PATCH /users",
        "POST /users",
        "PUT /users",
        "GET /users/cancel",
        "GET /users/edit",
        "PATCH /users/password",
        "POST /users/password",
        "PUT /users/password",
        "GET /users/password/edit",
        "GET /users/password/new",
        "GET /users/sign_in",
        "POST /users/sign_in",
        "DELETE /users/sign_out",
        "GET /users/sign_up",
        // match via: [:post, :put]
        "POST /webhooks/:kaynak",
        "PUT /webhooks/:kaynak",
    ];
    assert_eq!(idler, beklenen);
    assert_eq!(h["ozet"]["adaptor"], "rails@0.1.0");
    assert_eq!(h["ozet"]["uygulama"], "blog", "uygulamanın adı config/application.rb'deki modülden");
    assert_eq!(h["ozet"]["ana_uygulama"], "Blog");
    // spec/ okunmaz (SahteController)
    assert!(!h["ozet"]["okunan_dosyalar"].as_array().unwrap().iter().any(|d| d["yol"].as_str().unwrap().starts_with("spec/")));
    assert!(h["ozet"]["okunan_dosyalar"].as_array().unwrap().iter().any(|d| d["yol"] == "Gemfile"));
    assert_eq!(h["ozet"]["veri_semasi"], serde_json::json!(["db/schema.rb"]));
    // constraints altındaki rotalar koşullu
    let kosullu: Vec<&str> = h["ozet"]["kosullu_kapilar"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
    assert!(kosullu.contains(&"GET /admin/rapor") && kosullu.contains(&"DELETE /admin/users/:id"), "{kosullu:?}");
    assert!(!kosullu.contains(&"GET /api/v1/posts"));
    // Kapıya çevrilemeyenler bildirilir: döngüde kurulan rota (hedefi değişken), eylemi olmayan new / edit;
    // redirect rotası ve mount edilen motor işlem değildir, listede yok
    let cozulemeyen: Vec<&str> = h["ozet"]["cozulemeyen_rotalar"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
    assert!(cozulemeyen.iter().any(|x| x.contains("get sayfa") && x.contains("hedef çözülemedi")), "{cozulemeyen:?}");
    assert!(cozulemeyen.iter().any(|x| x.contains("GET /api/v1/posts/new -> api/v1/posts#new")), "{cozulemeyen:?}");
    assert!(!cozulemeyen.iter().any(|x| x.contains("eski") || x.contains("letter_opener")), "{cozulemeyen:?}");
    let e = uc(&h, "GET /api/v1/posts/:id");
    assert_eq!(e["kontrolcu"], "Api::V1::PostsController");
    assert_eq!(e["fonksiyon"], "show");
    assert_eq!(e["dosya"], "app/controllers/api/v1/posts_controller.rb");
    assert_eq!(e["parametreler"][0]["ad"], "id");
    assert_eq!(uc(&h, "GET /api/v1/posts")["ozet"], "Yayımlanmış yazılar", "eylemin üstündeki yorum");
    assert_eq!(uc(&h, "GET /hesap/ozet")["kontrolcu"], "Hesap::PanolarController");
    assert_eq!(uc(&h, "POST /users/sign_in")["kontrolcu"], "Devise::SessionsController");
    // Kart, OpenAPI ve graf dilden bağımsız
    let s = kok.join(".sezgi");
    let kart = json_oku(&s.join("benlik.json"));
    let islemler: Vec<&str> = kart["operations"].as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap()).collect();
    assert!(islemler.contains(&"api-v1-posts.show") && islemler.contains(&"devise-sessions.create"), "{islemler:?}");
    let oa = json_oku(&s.join("openapi.json"));
    assert!(oa["paths"]["/api/v1/posts/{post_id}/comments"]["post"].is_object());
    let kanit = json_oku(&s.join("kanit.json"));
    let onermeler = kanit["onermeler"].as_array().unwrap();
    assert!(onermeler.iter().all(|o| o["hesap"]["celiski"].as_array().unwrap().is_empty()), "çelişen kanıt olmamalı");
    // Rota kapsamları graf düğümü; constraints ortam koşuludur
    assert!(onermeler.iter().any(|o| o["varlik"].as_str().unwrap().starts_with("mod:config/routes.rb") && o["yuklem"] == "DEPENDS_ON"));
    assert!(onermeler.iter().any(|o| o["varlik"] == "api:GET /admin/rapor" && o["hedef"].as_str().is_some_and(|h| h.starts_with("env:subdomain"))), "constraints -> env düğümü");
    // Tablolar ve sütunlar db/schema.rb'den
    assert!(onermeler.iter().any(|o| o["varlik"] == "db:ArsivKaydi" && o["yuklem"] == "TABLO" && o["deger"]["tablo"] == "arsiv_kayitlari"));
    assert!(onermeler.iter().any(|o| o["varlik"] == "field:Post.title" && o["deger"]["tip"] == "string"));
    let _ = std::fs::remove_dir_all(kok.parent().unwrap());
}

#[test]
fn giris_ve_roller_filtre_zincirinden() {
    let (kok, h) = haritala("giris");
    let karar = |id: &str| (uc(&h, id)["giris"].as_str().unwrap().to_string(), uc(&h, id)["giris_kaniti"].as_str().unwrap().to_string());
    let beklenen: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        // ApplicationController: before_action :authenticate_user! (Devise) -> kesin
        ("GET /hesap/ozet", ("gerekli", "kesin")),
        ("GET /admin/rapor", ("gerekli", "kesin")),
        // skip_before_action :authenticate_user!
        ("GET /about", ("herkese_acik", "varsayim")),
        // BaseController: authenticate_or_request_with_http_token; only: [:index, :show, :search] atlanır
        ("POST /api/v1/posts", ("gerekli", "kesin")),
        ("GET /api/v1/posts", ("herkese_acik", "varsayim")),
        ("GET /api/v1/posts/:id", ("herkese_acik", "varsayim")),
        ("GET /api/v1/posts/:post_id/comments", ("herkese_acik", "varsayim")),
        ("POST /api/v1/posts/:post_id/comments", ("gerekli", "kesin")),
        // Rails 8 Authentication concern: included do before_action :require_authentication; allow_unauthenticated_access
        ("DELETE /session", ("gerekli", "kesin")),
        ("POST /session", ("herkese_acik", "varsayim")),
        // Devise: oturum açma açık, hesap düzenleme authenticate_scope!
        ("POST /users/sign_in", ("herkese_acik", "varsayim")),
        ("PATCH /users", ("gerekli", "kesin")),
        // Filtre yok
        ("POST /webhooks/:kaynak", ("herkese_acik", "varsayim")),
    ]);
    for (id, (g, k)) in &beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string()), "{id}");
    }
    let roller = |id: &str| -> Vec<String> { uc(&h, id)["roller"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect() };
    // before_action :require_admin -> current_user.admin? (rol enum'u)
    assert_eq!(roller("GET /admin/users"), ["admin"]);
    assert_eq!(uc(&h, "GET /admin/users")["roller_kaniti"], "varsayim");
    assert!(uc(&h, "GET /admin/users")["guardlar"].as_array().unwrap().iter().any(|g| g == "require_admin"));
    // Pundit: authorize @post, :publish? -> PostPolicy#publish?
    assert_eq!(roller("POST /api/v1/posts/:id/publish"), ["admin", "editor"]);
    assert!(uc(&h, "POST /api/v1/posts/:id/publish")["guardlar"].as_array().unwrap().iter().any(|g| g == "Pundit PostPolicy#publish?"));
    assert_eq!(roller("DELETE /api/v1/posts/:id"), ["admin"]);
    // CanCanCan: load_and_authorize_resource; Ability'de destroy yalnızca `if user.admin?` altında (manage :all)
    assert_eq!(roller("DELETE /comments/:id"), ["admin"]);
    assert!(uc(&h, "DELETE /comments/:id")["guardlar"].as_array().unwrap().iter().any(|g| g.as_str().unwrap().starts_with("CanCanCan")));
    // Rolsüz uçlar
    assert!(roller("POST /api/v1/posts").is_empty());
    assert_eq!(h["ozet"]["genel_guardlar"], serde_json::json!(["authenticate_user!"]));
    let _ = std::fs::remove_dir_all(kok.parent().unwrap());
}

#[test]
fn girdiler_strong_parameters_ve_dogrulamalar() {
    let (kok, h) = haritala("girdi");
    let e = uc(&h, "POST /api/v1/posts");
    let sema = &e["govde"][0]["sema"];
    assert_eq!(sema["dto"], "post_params");
    let post = &sema["alanlar"]["post"];
    assert_eq!(post["zorunlu"], true, "params.require(:post): yoksa 400");
    assert_eq!(post["tip"], "object");
    let ic = &post["ic"];
    // Tipler db/schema.rb'den, zorunluluk ve sınır doğrulamalardan, değerler enum'dan
    assert_eq!(ic["title"]["tip"], "string");
    assert_eq!(ic["title"]["zorunlu"], true);
    assert!(ic["title"]["kurallar"].as_array().unwrap().iter().any(|k| k == "MaxLength(120)"));
    assert_eq!(ic["body"]["zorunlu"], true);
    assert_eq!(ic["status"]["degerler"], serde_json::json!(["draft", "published", "archived"]));
    assert_eq!(ic["status"]["zorunlu"], false);
    assert_eq!(ic["tag_ids"]["tip"], "array");
    assert_eq!(ic["meta"]["tip"], "object");
    assert!(ic["meta"]["ic"]["keywords"].is_object());
    // Güncellemede presence zorunluluk değildir
    let g = &uc(&h, "PATCH /api/v1/posts/:id")["govde"][0]["sema"]["alanlar"]["post"]["ic"];
    assert_eq!(g["title"]["zorunlu"], false);
    // Okuma ucunda params[:q] ve params.fetch(:limit, 20) sorgudur; zorunluluğu yorumdur
    let s = &uc(&h, "GET /api/v1/posts/search")["sorgu"][0]["sema"]["alanlar"];
    assert_eq!(s["q"]["belirsiz"], true);
    assert!(s["limit"].is_object());
    assert!(uc(&h, "GET /api/v1/posts/search")["govde"].as_array().unwrap().is_empty());
    // params.permit(:email_address, :password): gömülü olmayan alanlar
    let o = &uc(&h, "POST /session")["govde"][0]["sema"]["alanlar"];
    assert!(o["email_address"].is_object() && o["password"].is_object());
    // Devise oturum açma gövdesi: { user: { email, password } }
    let d = &uc(&h, "POST /users/sign_in")["govde"][0]["sema"]["alanlar"]["user"]["ic"];
    assert_eq!(d["email"]["zorunlu"], true);
    // Önizlemenin sınayamadıkları: özel doğrulama ve Active Storage eki
    let den = &h["ozet"]["denetlenemeyen"];
    assert!(den["app/controllers/api/v1/posts_controller.rb:21"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("yasakli_kelime_yok")));
    assert!(den["app/controllers/api/v1/profiles_controller.rb:7"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("'avatar'")));
    let _ = std::fs::remove_dir_all(kok.parent().unwrap());
}

#[test]
fn yanitlar_jbuilder_ve_serilestirici() {
    let (kok, h) = haritala("yanit");
    // Örtük şablon: index.json.jbuilder -> json.array! ..., partial: -> _post.json.jbuilder
    let y = &uc(&h, "GET /api/v1/posts")["yanit"];
    assert_eq!(y["kaynak"], "jbuilder");
    assert_eq!(y["kesin"], true);
    assert_eq!(y["sema"]["dizi"], true);
    assert_eq!(y["sema"]["alanlar"]["id"]["tip"], "int");
    assert_eq!(y["sema"]["alanlar"]["author"]["ic"]["name"]["tip"], "string", "json.author do json.name post.user.name end");
    // render :show, status: :created
    let y = &uc(&h, "POST /api/v1/posts")["yanit"];
    assert_eq!(y["durum"], 201);
    assert_eq!(y["sema"]["alanlar"]["comments"]["tip"], "array");
    // AMS: render json: @comments -> CommentSerializer (belongs_to :user -> UserSerializer)
    let y = &uc(&h, "GET /api/v1/posts/:post_id/comments")["yanit"];
    assert_eq!(y["kaynak"], "serializer");
    assert_eq!(y["sema"]["dto"], "CommentSerializer");
    assert_eq!(y["sema"]["dizi"], true);
    assert!(y["sema"]["alanlar"]["user"]["ic"]["email"].is_object());
    // Açık serializer: kesin
    assert_eq!(uc(&h, "GET /api/v1/profile")["yanit"]["kesin"], true);
    // render json: @post (serileştiricisi yok): modelin sütunları, yorum
    let y = &uc(&h, "POST /api/v1/posts/:id/publish")["yanit"];
    assert_eq!(y["kaynak"], "render_json");
    assert_eq!(y["kesin"], false);
    assert!(y["sema"]["alanlar"]["published"].is_object());
    // head :no_content: şema yok
    assert!(uc(&h, "DELETE /api/v1/posts/:id")["yanit"].is_null());
    let _ = std::fs::remove_dir_all(kok.parent().unwrap());
}

#[test]
fn veri_kaskad_geri_cagri_ve_isler() {
    let (kok, h) = haritala("veri");
    let veri = |id: &str, a: &str| -> Vec<String> { uc(&h, id)["veri"][a].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect() };
    // current_user.posts.build(...) + @post.save
    assert!(veri("POST /api/v1/posts", "yazar").contains(&"Post".to_string()));
    // set_post (before_action) okur; before_destroy :arsivle -> ArsivKaydi.create!
    let y = veri("DELETE /api/v1/posts/:id", "yazar");
    assert!(y.contains(&"Post".to_string()) && y.contains(&"ArsivKaydi".to_string()), "{y:?}");
    assert!(veri("GET /api/v1/posts/:id", "okur").contains(&"Post".to_string()));
    // Kaskadlar: has_many dependent: :destroy / :delete_all ve şemadaki on_delete: :cascade (tekrarsız)
    let kaskadlar: Vec<(String, String)> = h["ozet"]["kaskadlar"].as_array().unwrap().iter().map(|k| (k[0].as_str().unwrap().into(), k[1].as_str().unwrap().into())).collect();
    for (e, c) in [("Post", "Comment"), ("Post", "Like"), ("User", "Post"), ("User", "Session")] {
        assert_eq!(kaskadlar.iter().filter(|(a, b)| a == e && b == c).count(), 1, "{e} -> {c}: {kaskadlar:?}");
    }
    assert!(!kaskadlar.iter().any(|(a, b)| a == "User" && b == "Comment"), "dependent: :nullify kaskad değil");
    let dna = &h["ozet"]["dna"];
    let sil = &dna["app/controllers/api/v1/posts_controller.rb:40"];
    let dolayli: Vec<&str> = sil["dolayli_yazar"].as_array().unwrap().iter().map(|d| d["tablo"].as_str().unwrap()).collect();
    assert!(dolayli.contains(&"Comment") && dolayli.contains(&"Like"), "{dolayli:?}");
    // Hatalar: Pundit (403, politikanın metniyle), set_post'taki find (404, bağımlılık)
    let hatalar = sil["hatalar"].as_array().unwrap();
    assert!(hatalar.iter().any(|x| x["durum"] == 403 && x["kosul"].as_str().unwrap().contains("PostPolicy#destroy?")));
    assert!(hatalar.iter().any(|x| x["durum"] == 404 && x["kaynak"] == "bagimlilik" && x["guven"] == "inferred"));
    // Sahiplik: PostPolicy#update? -> record.user_id == user.id
    assert!(!dna["app/controllers/api/v1/posts_controller.rb:31"]["sahiplik_denetimi"].as_array().unwrap().is_empty());
    // render json: ..., status: :unprocessable_entity (else kolu)
    let olustur = &dna["app/controllers/api/v1/posts_controller.rb:21"];
    assert!(olustur["hatalar"].as_array().unwrap().iter().any(|x| x["durum"] == 422 && x["kosul"] == "!(@post.save)"));
    // Tetikler: deliver_later (eylem) ve after_create_commit { TakipcilereBildirJob.perform_later } (geri çağrı)
    let tetik: Vec<&str> = olustur["tetikler"].as_array().unwrap().iter().map(|t| t["ad"].as_str().unwrap()).collect();
    assert!(tetik.contains(&"PostMailer.yeni_yazi") && tetik.contains(&"TakipcilereBildirJob"), "{tetik:?}");
    // İş düğümleri: perform zinciri (tetikler, sildikleri)
    let ol = &h["ozet"]["olaylar"];
    assert_eq!(ol["job:TakipcilereBildirJob"]["tetikler"][0]["ad"], "PostMailer.yeni_yazi");
    assert_eq!(ol["job:WebhookIslemeJob"]["silinen"], serde_json::json!(["Like"]));
    assert!(ol["job:PostMailer.yeni_yazi"].is_object());
    // Kart: olay düğümleri ve veri bağları
    let kart = json_oku(&kok.join(".sezgi").join("benlik.json"));
    assert!(kart.to_string().contains("job:TakipcilereBildirJob"));
    let _ = std::fs::remove_dir_all(kok.parent().unwrap());
}

#[test]
fn doktor_ve_rapor_rails_der() {
    let (kok, _) = haritala("doktor");
    let ev = kok.parent().unwrap().join("ev");
    let calistir = |args: &[&str]| -> String {
        let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(args).current_dir(&kok).env("HOME", &ev).env("APPDATA", &ev).env("SEZGI_ANAHTARLIK", "kapali").output().unwrap();
        format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr))
    };
    let d = calistir(&["doktor"]);
    assert!(d.contains("Ruby on Rails"), "{d}");
    assert!(d.contains("blog"), "{d}");
    let r = calistir(&["rapor"]);
    assert!(r.contains("rails@0.1.0") && r.contains("db/schema.rb"), "{r}");
    assert!(r.contains("rota tanımı işleme çevrilemedi"), "{r}");
    let _ = std::fs::remove_dir_all(kok.parent().unwrap());
}

#[test]
fn rails_olmayan_klasor_mesaji_rails_der() {
    let kok = std::env::temp_dir().join(format!("sezgi-d6-rails-bos-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    std::fs::create_dir_all(kok.join("api").join("config")).unwrap();
    std::fs::write(kok.join("api").join("Gemfile"), "gem 'rails'\n").unwrap();
    std::fs::write(kok.join("api").join("config").join("routes.rb"), "Rails.application.routes.draw do\nend\n").unwrap();
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", "--deneme"]).current_dir(&kok).env("SEZGI_ANAHTARLIK", "kapali").output().unwrap();
    let m = String::from_utf8_lossy(&c.stderr).to_string();
    assert!(!c.status.success());
    assert!(m.contains("Gemfile'da rails") && m.contains("sezgi init api") && m.contains("(Rails)"), "{m}");
    let _ = std::fs::remove_dir_all(&kok);
}
