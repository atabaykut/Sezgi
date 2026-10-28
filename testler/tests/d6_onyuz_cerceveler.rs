//! D6 ön yüz: Angular, Remix / React Router v7 çatı kipi ve Solid / SolidStart Vue / Svelte ile aynı derinlikte okunur:
//! ekranlar (rota tanımları, dosya rotaları, tembel yükleme), ekran geçişleri (NAVIGATES), UI eylemleri (şablon olayları,
//! formlar) ve istemci çağrısı → API eşlemesi; Angular koruyucuları ekranın giriş gereksinimi olur (inferred). Vue ve
//! Svelte şablonlarındaki olaylar ve bağlantılar da okunur.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

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

/// Örneği geçici klasöre kopyalar, `server` klasöründe init çalıştırır (ek argümanlarla) ve web-baglari.json'u döndürür
fn haritala(ad: &str, ek: &[&str]) -> (PathBuf, Value) {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let kok = std::env::temp_dir().join(format!("sezgi-d6-onyuz-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    kopyala(&ornek(ad), &kok.join("depo"));
    std::fs::create_dir_all(kok.join("ev")).unwrap();
    let uygulama = kok.join("depo").join("server");
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", "--paylasimsiz"])
        .args(ek)
        .current_dir(&uygulama)
        .env("HOME", kok.join("ev"))
        .env("APPDATA", kok.join("ev"))
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    assert!(c.status.success(), "init: {}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    let p = uygulama.join(".sezgi").join("web-baglari.json");
    let w: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap();
    (kok, w)
}

fn ekran<'a>(w: &'a Value, yol: &str) -> Option<&'a Value> {
    w["ekranlar"].as_array().unwrap().iter().find(|e| e["yol"] == yol)
}

fn ekran_yollari(w: &Value) -> Vec<String> {
    w["ekranlar"].as_array().unwrap().iter().map(|e| e["yol"].as_str().unwrap().to_string()).collect()
}

fn cagri<'a>(w: &'a Value, id: &str) -> &'a Value {
    w["istemci_cagrilari"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("{id} yok: {:#}", w["istemci_cagrilari"]))
}

fn bag(w: &Value, kaynak: &str, iliski: &str, hedef: &str) -> bool {
    w["baglar"].as_array().unwrap().iter().any(|b| b["kaynak"] == kaynak && b["iliski"] == iliski && b["hedef"] == hedef)
}

fn gecis(w: &Value, kaynak: &str, hedef: &str) -> bool {
    bag(w, &format!("screen:web/{kaynak}"), "NAVIGATES", &format!("screen:web/{hedef}"))
}

/// Eylem: dosya sonu ve ad; ulaştığı istemci çağrıları
fn eylem<'a>(w: &'a Value, dosya: &str, ad: &str) -> &'a Value {
    w["eylemler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kanit"]["dosya"].as_str().unwrap().ends_with(dosya) && e["ad"] == ad)
        .unwrap_or_else(|| panic!("{dosya} #{ad} yok: {:#}", w["eylemler"]))
}

fn cagrilari(e: &Value) -> Vec<&str> {
    e["cagrilar"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect()
}

#[test]
fn d6_onyuz_angular_rotalar_tembel_yukleme_ve_koruyucular() {
    let (kok, w) = haritala("d6_onyuz_angular", &[]);
    let yollar = ekran_yollari(&w);
    for y in ["/heroes", "/detail/:id", "/login", "/admin", "/admin/stats"] {
        assert!(yollar.iter().any(|x| x == y), "{y} ekranı yok: {yollar:?}");
    }
    // Yönlendirme rotası ve joker rota ekran değildir
    assert!(!yollar.iter().any(|x| x == "/" || x.contains('*')), "{yollar:?}");
    // Koruyucular: authGuard -> giriş gerekli; satır içi `!girdi` koruyucusu -> yalnızca girişsiz (login ekranı);
    // üst rotanın koruyucusu tembel yüklenen alt rotaya geçer
    let detay = ekran(&w, "/detail/:id").unwrap();
    assert_eq!(detay["giris"], "gerekli", "{detay}");
    assert_eq!(detay["giris_guven"], "inferred");
    assert!(detay["korumalar"].as_array().unwrap().iter().any(|k| k == "authGuard"), "{detay}");
    assert_eq!(ekran(&w, "/login").unwrap()["giris"], "misafir", "{}", ekran(&w, "/login").unwrap());
    assert_eq!(ekran(&w, "/admin/stats").unwrap()["giris"], "gerekli");
    assert!(ekran(&w, "/heroes").unwrap()["giris"].is_null());
    assert!(bag(&w, "screen:web//admin", "REQUIRES_PERMISSION", "guard:web/AuthGuard"), "{:#}", w["baglar"]);
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn d6_onyuz_angular_servis_cagrilari_api_eslemesi() {
    let (kok, w) = haritala("d6_onyuz_angular", &[]);
    // HttpClient servis metotları: environment.apiUrl (baseUrl'e göre içe aktarılmış) + sınıf alanı
    let liste = cagri(&w, "client:HeroService.getHeroes");
    assert_eq!(liste["api"], "GET /api/heroes", "{liste}");
    assert_eq!(liste["guven"], "certain", "{liste}");
    assert_eq!(liste["taniyici"], "angular-http");
    assert_eq!(cagri(&w, "client:HeroService.deleteHero")["api"], "DELETE /api/heroes/:id");
    assert_eq!(cagri(&w, "client:HeroService.updateHero")["api"], "PUT /api/heroes/:id");
    assert_eq!(cagri(&w, "client:AuthService.login")["api"], "POST /api/auth/login");
    // Göreli adres ('api/admin/stats') uygulama köküne göre
    assert_eq!(cagri(&w, "client:AdminDashboardComponent.load")["api"], "GET /api/admin/stats");
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn d6_onyuz_angular_sablon_olaylari_ve_gecisler() {
    let (kok, w) = haritala("d6_onyuz_angular", &[]);
    // Dış şablondaki (click): bileşen metodu -> enjekte edilen servis -> HttpClient
    let ekle = eylem(&w, "heroes/heroes.component.html", "Add hero");
    assert_eq!(ekle["rol"], "button");
    assert_eq!(ekle["olay"], "(click)");
    assert_eq!(ekle["kanit"]["satir"], 4, "satır şablon dosyasınınki: {ekle}");
    assert_eq!(cagrilari(ekle), vec!["client:HeroService.addHero"]);
    assert!(ekle["ekranlar"].as_array().unwrap().iter().any(|e| e == "screen:web//heroes"), "{ekle}");
    assert_eq!(cagrilari(eylem(&w, "heroes/heroes.component.html", "Delete hero")), vec!["client:HeroService.deleteHero"]);
    // Satır içi şablondaki (ngSubmit): gönder düğmesinin adı; inject() ile alınan servis
    let kaydet = eylem(&w, "hero-detail/hero-detail.component.ts", "Save");
    assert_eq!(cagrilari(kaydet), vec!["client:HeroService.updateHero"], "{kaydet}");
    // Türü yazılmamış <button> formun gönder düğmesidir
    assert_eq!(cagrilari(eylem(&w, "login/login.component.html", "Sign in")), vec!["client:AuthService.login"]);
    // ngOnInit: ekran yüklemesi
    let yukleme = eylem(&w, "heroes/heroes.component.ts", "HeroesComponent");
    assert_eq!(yukleme["olay"], "yukleme");
    assert_eq!(cagrilari(yukleme), vec!["client:HeroService.getHeroes"]);
    // TRIGGERS ve CALLS bağları grafik sözlüğünde
    assert!(bag(&w, ekle["id"].as_str().unwrap(), "TRIGGERS", "client:HeroService.addHero"));
    assert!(bag(&w, "client:HeroService.addHero", "CALLS", "api:POST /api/heroes"));
    // Ekran geçişleri: dizi bağlantısı ([routerLink]="['/detail', id]"), kök bileşenin seçiciyle kullandığı gezinti
    // çubuğu (her ekranda), router.navigate / navigateByUrl, göreli routerLink
    assert!(gecis(&w, "/heroes", "/detail/:id"), "{:#}", w["baglar"]);
    assert!(gecis(&w, "/detail/:id", "/heroes"));
    assert!(gecis(&w, "/login", "/admin"), "gezinti çubuğu her ekranda");
    assert!(gecis(&w, "/login", "/heroes"));
    assert!(gecis(&w, "/admin", "/admin/stats"), "göreli routerLink");
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn d6_onyuz_remix_dosya_rotalari_loader_action_form_fetcher() {
    let (kok, w) = haritala("d6_onyuz_remix", &[]);
    let yollar = ekran_yollari(&w);
    for y in ["/", "/notes", "/notes/:noteId", "/notes/new", "/login", "/notes/archive", "/settings"] {
        assert!(yollar.iter().any(|x| x == y), "{y} ekranı yok: {yollar:?}");
    }
    // Kaynak rotası (varsayılan dışa aktarımsız), klasör rotasının yardımcı dosyası ve yolsuz düzen ekran değildir
    for y in ["/sitemap.xml", "/notes/like", "/settings/form", "/_auth"] {
        assert!(!yollar.iter().any(|x| x == y), "{y} ekran sayıldı: {yollar:?}");
    }
    // loader: ekran yüklemesi; düzen rotasının (notes.tsx) loader'ı alt ekranda da görünür, `notes_.archive`'da görünmez
    let yukleme = eylem(&w, "app/routes/notes.tsx", "loader");
    assert_eq!(yukleme["olay"], "yukleme");
    assert_eq!(cagri(&w, cagrilari(yukleme)[0])["api"], "GET /api/notes");
    let ekr: Vec<&str> = yukleme["ekranlar"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    assert!(ekr.contains(&"screen:web//notes/:noteId") && !ekr.contains(&"screen:web//notes/archive"), "{ekr:?}");
    // <Form method="post">: aynı rotanın action'ı
    let sil = eylem(&w, "app/routes/notes.$noteId.tsx", "Delete");
    assert_eq!(sil["olay"], "submit");
    let apiler: Vec<&str> = cagrilari(sil).iter().map(|c| cagri(&w, c)["api"].as_str().unwrap()).collect();
    assert_eq!(apiler, vec!["DELETE /api/notes/:id"], "{sil}");
    // fetcher.submit(..., { action: "/notes/like" }): kaynak rotasının action'ı
    let begen = eylem(&w, "app/routes/notes.$noteId.tsx", "Like");
    let apiler: Vec<&str> = cagrilari(begen).iter().map(|c| cagri(&w, c)["api"].as_str().unwrap()).collect();
    assert_eq!(apiler, vec!["POST /api/notes/:id/like"], "{begen}");
    // Türü yazılmamış düğme (varsayılan submit)
    let olustur = eylem(&w, "app/routes/notes.new.tsx", "Create note");
    assert_eq!(cagri(&w, cagrilari(olustur)[0])["api"], "POST /api/notes");
    // Ekran geçişleri: kök düzendeki bağlantı her ekranda, action'daki redirect
    assert!(gecis(&w, "/notes/archive", "/notes"), "{:#}", w["baglar"]);
    assert!(gecis(&w, "/", "/notes/new"));
    assert!(gecis(&w, "/notes/:noteId", "/notes"));
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn d6_onyuz_react_router_v7_rota_yapilandirmasi() {
    let (kok, w) = haritala("d6_onyuz_remix", &["--on-yuz", "../web-rr7"]);
    let yollar = ekran_yollari(&w);
    for y in ["/", "/about", "/login", "/concerts", "/concerts/:city"] {
        assert!(yollar.iter().any(|x| x == y), "{y} ekranı yok: {yollar:?}");
    }
    // Dosya adından yol çıkarılmaz (yapılandırma varken): routes/home -> /home yok
    assert!(!yollar.iter().any(|x| x == "/home" || x == "/auth/login"), "{yollar:?}");
    let giris = eylem(&w, "app/routes/auth/login.tsx", "Giriş yap");
    assert_eq!(cagri(&w, cagrilari(giris)[0])["api"], "POST /api/notes");
    assert!(gecis(&w, "/concerts/:city", "/about"), "kök düzen bağlantısı: {:#}", w["baglar"]);
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn d6_onyuz_solidstart_dosya_rotalari_sunucu_fonksiyonlari() {
    let (kok, w) = haritala("d6_onyuz_solid", &[]);
    let yollar = ekran_yollari(&w);
    for y in ["/", "/todos", "/users/:id", "/login"] {
        assert!(yollar.iter().any(|x| x == y), "{y} ekranı yok: {yollar:?}");
    }
    assert!(!yollar.iter().any(|x| x.starts_with("/api")), "API rotası ekran değildir: {yollar:?}");
    // query / action sarmalı ("use server"): istemci çağrısının sahibi
    assert_eq!(cagri(&w, "client:getTodos")["api"], "GET /api/todos");
    assert_eq!(cagri(&w, "client:addTodo")["api"], "POST /api/todos");
    assert_eq!(cagri(&w, "client:removeTodo")["api"], "DELETE /api/todos/:id");
    assert!(w["istatistik"]["sunucu_fonksiyonu"].as_u64().unwrap() >= 3, "{}", w["istatistik"]);
    // createAsync: ekran yüklemesi; <form action={addTodo}>; useAction bağı
    assert_eq!(cagrilari(eylem(&w, "src/routes/todos/index.tsx", "Todos")), vec!["client:getTodos"]);
    assert_eq!(cagrilari(eylem(&w, "src/routes/todos/index.tsx", "Add todo")), vec!["client:addTodo"]);
    assert_eq!(cagrilari(eylem(&w, "src/routes/todos/index.tsx", "Delete")), vec!["client:removeTodo"]);
    // createResource(kaynak, getirici)
    let kullanici = eylem(&w, "src/routes/users/[id].tsx", "User");
    assert_eq!(cagri(&w, cagrilari(kullanici)[0])["api"], "GET /api/users/:id");
    // <A href>: kök düzenin (app.tsx) gezinti çubuğu her ekranda; action'daki redirect
    assert!(gecis(&w, "/users/:id", "/todos"), "{:#}", w["baglar"]);
    assert!(gecis(&w, "/users/:id", "/"));
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn d6_onyuz_solid_router_jsx_rotalari_ve_tembel_bilesen() {
    let (kok, w) = haritala("d6_onyuz_solid", &["--on-yuz", "../web-router"]);
    let yollar = ekran_yollari(&w);
    for y in ["/", "/users", "/users/:id"] {
        assert!(yollar.iter().any(|x| x == y), "{y} ekranı yok: {yollar:?}");
    }
    // lazy(() => import(...)): ekranın kökü sayfa dosyası (rota dosyası değil)
    let sil = eylem(&w, "src/pages/User.tsx", "Remove");
    assert_eq!(sil["ekranlar"], serde_json::json!(["screen:web//users/:id"]), "{sil}");
    assert_eq!(cagri(&w, cagrilari(sil)[0])["api"], "DELETE /api/todos/:id");
    let yukleme = eylem(&w, "src/pages/Users.tsx", "Users");
    assert_eq!(yukleme["ekranlar"], serde_json::json!(["screen:web//users"]), "{yukleme}");
    // <Router root={App}>: kök düzenin bağlantısı her ekranda
    assert!(gecis(&w, "/users/:id", "/users"), "{:#}", w["baglar"]);
    assert!(gecis(&w, "/users/:id", "/"));
    let _ = std::fs::remove_dir_all(&kok);
}

#[test]
fn d6_onyuz_vue_svelte_sablon_olaylari_ve_vue_router() {
    let (kok, w) = haritala("d6_onyuz_vue", &[]);
    let yollar = ekran_yollari(&w);
    assert!(yollar.iter().any(|x| x == "/items") && yollar.iter().any(|x| x == "/admin"), "{yollar:?}");
    // meta.requiresAuth: giriş gerekli (inferred)
    assert_eq!(ekran(&w, "/admin").unwrap()["giris"], "gerekli");
    // Vue şablonundaki @click ve Svelte'deki on:click
    let kaydet = eylem(&w, "src/views/ItemsView.vue", "Kaydet");
    assert_eq!(kaydet["kanit"]["satir"], 11);
    assert_eq!(cagri(&w, cagrilari(kaydet)[0])["api"], "POST /api/items");
    assert_eq!(cagri(&w, cagrilari(eylem(&w, "src/components/Sil.svelte", "Sil"))[0])["api"], "DELETE /api/items/:id");
    // <router-link :to>, kök bileşenin (App.vue) <RouterLink to>
    assert!(gecis(&w, "/items", "/admin"), "{:#}", w["baglar"]);
    assert!(gecis(&w, "/admin", "/items"));
    let _ = std::fs::remove_dir_all(&kok);
}
