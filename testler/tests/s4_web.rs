//! S4 (P2.1): ön yüz kaynak adaptörü. Ekran → UI eylemi (rol + ad) → istemci çağrısı → API bağları statik olarak
//! kurulur ve `.sezgi/web-baglari.json` dosyasına kanıtıyla yazılır; `santral baglar` aynı adaptörü kullanır.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Ortam {
    kok: PathBuf,
}

impl Drop for Ortam {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.kok);
    }
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

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

impl Ortam {
    fn yeni(ad: &str, ornek_ad: &str) -> Ortam {
        static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let kok = std::env::temp_dir().join(format!("sezgi-s4-{ad}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(kok.join("ev")).unwrap();
        kopyala(&ornek(ornek_ad), &kok.join("depo"));
        Ortam { kok }
    }
    fn depo(&self) -> PathBuf {
        self.kok.join("depo")
    }
    fn calistir(&self, klasor: &Path, args: &[&str]) -> Output {
        let ev = self.kok.join("ev");
        Command::new(env!("CARGO_BIN_EXE_sezgi"))
            .args(args)
            .current_dir(klasor)
            .env("HOME", &ev)
            .env("APPDATA", &ev)
            .env("XDG_CONFIG_HOME", &ev)
            .env("SEZGI_ANAHTARLIK", "kapali")
            .output()
            .unwrap()
    }
    fn ok(&self, klasor: &Path, args: &[&str]) -> String {
        let c = self.calistir(klasor, args);
        let m = format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
        assert!(c.status.success(), "{args:?}: {m}");
        m
    }
}

fn baglar(harita: &Path) -> Value {
    let m = std::fs::read_to_string(harita.join("web-baglari.json")).unwrap_or_else(|e| panic!("web-baglari.json yok ({e})"));
    serde_json::from_str(&m).unwrap()
}

/// Bir eylemi (rol, ad) bul
fn eylem<'a>(w: &'a Value, rol: &str, ad: &str) -> &'a Value {
    w["eylemler"].as_array().unwrap().iter().find(|e| e["rol"] == rol && e["ad"] == ad).unwrap_or_else(|| panic!("{rol}:{ad} eylemi yok: {w:#}"))
}

/// İstemci çağrısını kimliğiyle bul
fn cagri<'a>(w: &'a Value, id: &str) -> &'a Value {
    w["istemci_cagrilari"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("{id} yok: {w:#}"))
}

#[test]
fn altin_zincir_save_dugmesinden_backend_fonksiyonuna() {
    let o = Ortam::yeni("altin", "s4_fastapi_react");
    let backend = o.depo().join("backend");
    // Ön yüz monorepoda kendiliğinden bulunur (../frontend)
    let m = o.ok(&backend, &["init"]);
    assert!(m.contains("Ön yüz"), "init özeti ön yüzü söyler: {m}");
    let w = baglar(&backend.join(".sezgi"));
    // UI eylemi: form onSubmit → içindeki submit düğmesi "Save" (REA locator anahtarı: role=button, name=Save)
    let save = eylem(&w, "button", "Save");
    assert!(save["ekranlar"].as_array().unwrap().iter().any(|e| e == "screen:web//items"), "ekran /items: {save}");
    assert!(save["cagrilar"].as_array().unwrap().iter().any(|c| c == "client:ItemsService.createItem"), "{save}");
    assert_eq!(save["guven"], "inferred", "UI → istemci bağı statikte en fazla inferred: {save}");
    assert!(save["kanit"]["dosya"].as_str().unwrap().ends_with("AddItem.tsx") && save["kanit"]["satir"].as_u64().unwrap() > 0, "{save}");
    // İstemci çağrısı → API → backend fonksiyonu
    let c = cagri(&w, "client:ItemsService.createItem");
    assert_eq!(c["yontem"], "POST", "{c}");
    assert_eq!(c["api"], "POST /api/v1/items/", "{c}");
    assert_eq!(c["guven"], "certain", "tek anlamlı: açık yöntem, sabit yol, tek uç: {c}");
    assert!(c["fonksiyon"].as_str().unwrap_or("").to_lowercase().contains("createitem") || c["fonksiyon"] == "create_item", "{c}");
    assert!(c["kanit"]["dosya"].as_str().unwrap().ends_with("sdk.gen.ts"), "{c}");
    // Silme: simge düğmesi aria-label ile adlanır; yol şablonu {id} → :id
    let sil = eylem(&w, "button", "Delete item");
    assert!(sil["cagrilar"].as_array().unwrap().iter().any(|c| c == "client:ItemsService.deleteItem"), "{sil}");
    assert_eq!(cagri(&w, "client:ItemsService.deleteItem")["api"], "DELETE /api/v1/items/:id");
    // Ekran yüklemesi (useSuspenseQuery → readItems)
    assert!(
        w["eylemler"].as_array().unwrap().iter().any(|e| e["olay"] == "yukleme" && e["cagrilar"].as_array().unwrap().iter().any(|c| c == "client:ItemsService.readItems")),
        "ekran yüklemesi: {w:#}"
    );
    // Bağlar kanıtlı ve 03 sözlüğüyle
    let b = w["baglar"].as_array().unwrap();
    for (kaynak, iliski, hedef) in [
        ("screen:web//items", "DISPLAYS", save["id"].as_str().unwrap()),
        (save["id"].as_str().unwrap(), "TRIGGERS", "client:ItemsService.createItem"),
        ("client:ItemsService.createItem", "CALLS", "api:POST /api/v1/items/"),
    ] {
        let bag = b.iter().find(|x| x["kaynak"] == kaynak && x["iliski"] == iliski && x["hedef"] == hedef).unwrap_or_else(|| panic!("{kaynak} {iliski} {hedef} yok: {b:?}"));
        assert!(bag["kanit"]["tur"] == "SOURCE_CODE" && bag["kanit"]["dosya"].is_string() && bag["guven"].is_string(), "{bag}");
    }
    // Yanlış pozitif yok: yönlendirme ve çeviri çağrıları istemci çağrısı değildir; vazgeç düğmesi API çağırmaz
    for x in w["istemci_cagrilari"].as_array().unwrap() {
        assert!(x["kanit"]["dosya"].as_str().unwrap().ends_with("sdk.gen.ts"), "yalnızca üretilmiş istemcideki çağrılar: {x}");
    }
    assert!(!w["eylemler"].as_array().unwrap().iter().any(|e| e["olay"] != "yukleme" && (e["ad"] == "Cancel" || e["ad"] == "Items")), "API çağırmayan eylem yazılmaz: {w:#}");
}

#[test]
fn rtk_query_sabit_cozumleme_ve_react_router() {
    let o = Ortam::yeni("rtk", "s4_rtk");
    let depo = o.depo();
    o.ok(&depo, &["init"]);
    let w = baglar(&depo.join(".sezgi"));
    let olustur = eylem(&w, "button", "Create Product");
    assert!(olustur["ekranlar"].as_array().unwrap().iter().any(|e| e == "screen:web//admin/productlist"), "{olustur}");
    assert!(olustur["cagrilar"].as_array().unwrap().iter().any(|c| c == "client:createProduct"), "{olustur}");
    let c = cagri(&w, "client:createProduct");
    assert_eq!(c["api"], "POST /api/products", "sabit (PRODUCTS_URL) başka dosyadan çözülür: {c}");
    let sil = cagri(&w, "client:deleteProduct");
    assert_eq!(sil["api"], "DELETE /api/products/:id", "{sil}");
    // Varsayılan yöntem (RTK: GET) ve ekran yüklemesi
    assert_eq!(cagri(&w, "client:getProducts")["api"], "GET /api/products");
    // Düzen bileşenindeki (App → Header) eylem kök ekranda
    let cikis = eylem(&w, "button", "Logout");
    assert!(cikis["cagrilar"].as_array().unwrap().iter().any(|c| c == "client:logout"), "{cikis}");
    assert_eq!(cagri(&w, "client:logout")["api"], "POST /api/users/logout");
    let baglanan = w["istemci_cagrilari"].as_array().unwrap().iter().filter(|c| c["api"].is_string()).count();
    assert!(baglanan >= 4, "{w:#}");
}

#[test]
fn on_yuz_bayragi_ve_hatasi() {
    let o = Ortam::yeni("bayrak", "s4_fastapi_react");
    let backend = o.depo().join("backend");
    // Kendiliğinden bulunmayan bir yerdeki ön yüz: --on-yuz ile
    std::fs::rename(o.depo().join("frontend"), o.kok.join("istemci")).unwrap();
    o.ok(&backend, &["init"]);
    assert!(!backend.join(".sezgi").join("web-baglari.json").exists(), "ön yüz yoksa dosya yazılmaz");
    let yol = o.kok.join("istemci");
    o.ok(&backend, &["init", "--on-yuz", yol.to_str().unwrap()]);
    let w = baglar(&backend.join(".sezgi"));
    assert_eq!(cagri(&w, "client:ItemsService.createItem")["api"], "POST /api/v1/items/");
    let c = o.calistir(&backend, &["init", "--on-yuz", o.kok.join("yok").to_str().unwrap()]);
    assert!(!c.status.success(), "olmayan klasör hata verir");
    assert!(String::from_utf8_lossy(&c.stdout).contains("--on-yuz") || String::from_utf8_lossy(&c.stderr).contains("--on-yuz"));
    let y = o.ok(&backend, &["yardim", "init"]);
    assert!(y.contains("--on-yuz"), "{y}");
}

#[test]
fn santral_uretilmis_istemciyi_ve_rtk_yu_baglar() {
    let o = Ortam::yeni("santral", "s4_fastapi_react");
    let backend = o.depo().join("backend");
    o.ok(&backend, &["init"]);
    let harita = backend.join(".sezgi");
    o.ok(&o.depo(), &["santral", "ekle", harita.to_str().unwrap(), "--ad", "api"]);
    o.ok(&o.depo(), &["santral", "ekle", "--istemci", o.depo().join("frontend").to_str().unwrap(), "--ad", "web"]);
    let j: Value = serde_json::from_str(&o.ok(&o.depo(), &["santral", "baglar", "--json"])).unwrap();
    let b = j["baglar"].as_array().unwrap();
    let bul = |kapi: &str| b.iter().find(|x| x["kapi"] == kapi && x["dosya"].as_str().unwrap().ends_with("sdk.gen.ts")).unwrap_or_else(|| panic!("{kapi} yok: {j}"));
    assert_eq!(bul("api: POST /api/v1/items/")["damga"], "kesin");
    assert_eq!(bul("api: DELETE /api/v1/items/:id")["damga"], "kesin");
    assert!(!b.iter().any(|x| x["dosya"].as_str().unwrap().ends_with("items.tsx")), "yönlendirme çağrısı bağ değildir: {j}");
}
