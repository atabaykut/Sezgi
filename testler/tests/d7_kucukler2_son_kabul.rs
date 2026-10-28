//! D7 küçükler 2 (son kabul 0.1.1 listesi): `init <alt-klasör>` sonrası "Sonraki adım" bloğu hizalı ve `sezgi izle`
//! klasörü veriyor (bütün çatılarda); `rapor` tanım dosyalarını özetliyor (`--json` tam liste); Laravel izin
//! denetimleri (`can:` ara katmanı, `$this->authorize`, projenin izin yardımcıları, enum izin adları) rol / izin olarak
//! sayılıyor ve güven damgası doğru; doğrulama kuralı adlarında `?` ve `array_merge` gibi çöp yok; boş klasörde
//! `doktor` JS'e özgü değil, desteklenen çatıları ve `sezgi kesfet`'i söylüyor.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

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

struct Kok(PathBuf);

impl Drop for Kok {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn kok(etiket: &str) -> Kok {
    let k = std::env::temp_dir().join(format!("sezgi-d7k2-{etiket}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    std::fs::create_dir_all(k.join(".ev").join("ayar")).unwrap();
    Kok(k)
}

fn sezgi(k: &Kok, cwd: &Path, args: &[&str]) -> Output {
    let ev = k.0.join(".ev");
    Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", &ev)
        .env("USERPROFILE", &ev)
        .env("APPDATA", ev.join("ayar"))
        .env("XDG_CONFIG_HOME", ev.join("ayar"))
        .env("SEZGI_ANAHTARLIK", "kapali")
        .env_remove("SEZGI_TOKEN")
        .output()
        .unwrap()
}

fn metin(c: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr))
}

fn ok(k: &Kok, cwd: &Path, args: &[&str]) -> String {
    let c = sezgi(k, cwd, args);
    assert!(c.status.success(), "{args:?}: {}", metin(&c));
    metin(&c)
}

/// "Sonraki adım:" bloğunun satırları: (komut, açıklamanın başladığı karakter sütunu)
fn sonraki(m: &str) -> Vec<(String, usize)> {
    let blok: Vec<&str> = m.lines().skip_while(|l| !l.starts_with("Sonraki adım")).skip(1).take_while(|l| l.starts_with("  ")).collect();
    assert_eq!(blok.len(), 3, "{m}");
    blok.iter()
        .map(|l| {
            let c: Vec<char> = l.chars().collect();
            // Komut ile açıklama arasında en az iki boşluk
            let i = (2..c.len() - 1).find(|&i| c[i] == ' ' && c[i + 1] == ' ').unwrap_or_else(|| panic!("komut ile açıklama bitişik: {l}"));
            let aciklama = (i..c.len()).find(|&j| c[j] != ' ').unwrap();
            (c[2..i].iter().collect::<String>(), aciklama)
        })
        .collect()
}

#[test]
fn init_alt_klasor_sonraki_adim_hizali_ve_izle_klasorlu() {
    for ornek_ad in ["fastapi-temel", "nestjs-temel", "d6_laravel_temel", "d6_go_gin"] {
        let k = kok(&format!("b4-{ornek_ad}"));
        kopyala(&ornek(ornek_ad), &k.0.join("backend"));
        let m = ok(&k, &k.0, &["init", "backend"]);
        let s = sonraki(&m);
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(s[0].0, format!("sezgi bagla backend{sep}.sezgi"), "{ornek_ad}: {m}");
        assert_eq!(s[1].0, format!("sezgi ayna backend{sep}.sezgi"), "{ornek_ad}: {m}");
        assert_eq!(s[2].0, "sezgi izle backend", "{ornek_ad}: izle uygulama klasörünü verir: {m}");
        assert!(s.iter().all(|x| x.1 == s[0].1), "{ornek_ad}: açıklamalar aynı sütunda başlar: {s:?}\n{m}");
        // Önerilen izle komutu gerçekten uygulamayı bulur: kökte uygulama yok, `sezgi izle` tek başına bulamazdı
        assert!(!k.0.join("composer.json").exists() && !k.0.join("package.json").exists());

        // Bulunulan klasörde: kısa komutlar, hizalı
        let m = ok(&k, &k.0.join("backend"), &["init", "."]);
        let s = sonraki(&m);
        assert_eq!(s.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["sezgi bagla", "sezgi ayna", "sezgi izle"], "{m}");
        assert!(s.iter().all(|x| x.1 == s[0].1), "{s:?}");
    }
}

#[test]
fn init_ozel_cikti_izle_cikti_klasorunu_de_verir() {
    let k = kok("b4-cikti");
    kopyala(&ornek("fastapi-temel"), &k.0.join("api"));
    let m = ok(&k, &k.0, &["init", "api", "--cikti", "harita"]);
    let s = sonraki(&m);
    assert_eq!(s[0].0, "sezgi bagla harita", "{m}");
    assert_eq!(s[2].0, "sezgi izle api --cikti harita", "{m}");
    assert!(s.iter().all(|x| x.1 == s[0].1), "{s:?}");
}

#[test]
fn rapor_tanim_dosyalarini_ozetler_json_tam_liste() {
    let k = kok("b5");
    let uyg = k.0.join("uyg");
    kopyala(&ornek("d6_laravel_temel"), &uyg);
    ok(&k, &uyg, &["init", "."]);
    // Çok sayıda tanım dosyası olan uygulama (BookStack'te ~140): haritanın şema listesi büyütülür
    let yol = uyg.join(".sezgi").join("kapilar.json");
    let mut h: Value = serde_json::from_str(&std::fs::read_to_string(&yol).unwrap()).unwrap();
    let liste: Vec<String> = (0..40).map(|i| format!("app/Models/Model{i:02}.php")).collect();
    h["ozet"]["veri_semasi"] = serde_json::json!(liste);
    std::fs::write(&yol, serde_json::to_string_pretty(&h).unwrap()).unwrap();
    let m = ok(&k, &uyg, &["rapor"]);
    let satir = m.lines().find(|l| l.contains("VERİ")).unwrap_or_else(|| panic!("{m}"));
    let gorunen = liste.iter().filter(|d| satir.contains(d.as_str())).count();
    assert!((1..=5).contains(&gorunen), "ilk birkaç dosya: {satir}");
    assert!(satir.contains(&format!("ve {} dosya daha", 40 - gorunen)), "{satir}");
    assert!(satir.chars().count() < 260, "tek satıra dökülmez: {satir}");
    let j: Value = serde_json::from_str(&ok(&k, &uyg, &["rapor", "--json"])).unwrap();
    assert_eq!(j["veri"]["sema_dosyalari"].as_array().unwrap().len(), 40, "--json tam listeyi korur");

    // Az dosyada hepsi yazılır
    h["ozet"]["veri_semasi"] = serde_json::json!(["prisma/schema.prisma", "db/b.sql"]);
    std::fs::write(&yol, serde_json::to_string_pretty(&h).unwrap()).unwrap();
    let m = ok(&k, &uyg, &["rapor"]);
    let satir = m.lines().find(|l| l.contains("VERİ")).unwrap();
    assert!(satir.contains("prisma/schema.prisma, db/b.sql") && !satir.contains("daha"), "{satir}");
}

fn harita(k: &Kok, ornek_ad: &str) -> (PathBuf, Value) {
    let uyg = k.0.join("uyg");
    kopyala(&ornek(ornek_ad), &uyg);
    ok(k, &uyg, &["init", "."]);
    let h = serde_json::from_str(&std::fs::read_to_string(uyg.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    (uyg, h)
}

fn uc<'a>(h: &'a Value, id: &str) -> &'a Value {
    h["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).unwrap_or_else(|| panic!("{id} yok"))
}

fn roller(k: &Value) -> Vec<String> {
    k["roller"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect()
}

#[test]
fn laravel_izin_denetimleri_rol_olarak_sayilir() {
    let k = kok("b6-izin");
    let (uyg, h) = harita(&k, "d7_kucukler2_izin");
    // can:audit-view; `can` takma adı projenin ara katmanına bağlı (Kernel): izin adı statik, kararı proje kodunda -> yorum
    let a = uc(&h, "GET /audit");
    assert_eq!(roller(a), ["audit-view"], "{a}");
    assert_eq!(a["roller_kaniti"], "varsayim", "{a}");
    // $this->authorize('admin-panel') gövdenin üst düzeyinde: çatının kendi denetimi, statik -> kesin
    let a = uc(&h, "GET /admin");
    assert_eq!(roller(a), ["admin-panel"], "{a}");
    assert!(a["roller_kaniti"].is_null(), "{a}");
    // Projenin izin yardımcısı ($this->checkPermission(Permission::X)): enum değeri çözülür, karar proje kodunda -> yorum
    for (id, izin) in [("GET /settings", "settings-manage"), ("PUT /settings", "settings-manage"), ("POST /books", "book-create-all"), ("PUT /books/:book", "book-update")] {
        let a = uc(&h, id);
        assert_eq!(roller(a), [izin], "{id}: {a}");
        assert_eq!(a["roller_kaniti"], "varsayim", "{id}: {a}");
    }
    // Enum metodunun ürettiği ara katman: $this->middleware(Permission::ContentExport->middleware()) -> dinamik, yorum
    let a = uc(&h, "GET /export/:book");
    assert_eq!(roller(a), ["content-export"], "{a}");
    assert_eq!(a["roller_kaniti"], "varsayim", "{a}");
    // İzin denetimi olmayan uç rol kısıtlı değil
    assert!(roller(uc(&h, "GET /books")).is_empty());

    let m = ok(&k, &uyg, &["rapor"]);
    assert!(m.contains("· 7 rol kısıtlı"), "{m}");
    // Kart da izinleri taşır; yorum olan izin işlemi "tam" saymaz
    let kart: Value = serde_json::from_str(&std::fs::read_to_string(uyg.join(".sezgi").join("benlik.json")).unwrap()).unwrap();
    assert!(kart.to_string().contains("audit-view"), "{kart}");

    // Kernel yokken `can` çatının Authorize sınıfıdır: can:audit-view statik ve açık -> kesin
    std::fs::remove_file(uyg.join("app").join("Http").join("Kernel.php")).unwrap();
    ok(&k, &uyg, &["init", "."]);
    let h: Value = serde_json::from_str(&std::fs::read_to_string(uyg.join(".sezgi").join("kapilar.json")).unwrap()).unwrap();
    let a = uc(&h, "GET /audit");
    assert_eq!(roller(a), ["audit-view"], "{a}");
    assert!(a["roller_kaniti"].is_null(), "{a}");
    // Hesaplanan ara katman (enum metodu) çatının sınıfına çözülse de yorum kalır
    assert_eq!(uc(&h, "GET /export/:book")["roller_kaniti"], "varsayim");
}

#[test]
fn laravel_kural_adlarinda_cop_yok() {
    let k = kok("b6-kural");
    let (uyg, h) = harita(&k, "d7_kucukler2_izin");
    let g = &uc(&h, "POST /books")["govde"][0]["sema"]["alanlar"];
    let kurallar = |a: &str| -> Vec<String> { g[a]["kurallar"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect() };
    // array_merge(['nullable'], $this->getImageValidationRules()): iki taraf da açılır
    let resim = kurallar("image");
    assert!(resim.contains(&"nullable()".to_string()) && resim.iter().any(|r| r.starts_with("mimes")), "{resim:?}");
    assert!(!resim.iter().any(|r| r.contains("array_merge") || r.contains("getImageValidationRules")), "{resim:?}");
    // 'max:' . $boy: kuralın adı yazılı, değeri çalışma anında
    assert!(resim.contains(&"max:…".to_string()) && !resim.contains(&"max:".to_string()), "{resim:?}");
    // Koşula bağlı kural ($sifreli ? [...] : null): kural adı "?" değil; zorunluluğu belirsiz
    let parola = kurallar("password");
    assert!(!parola.iter().any(|r| r.starts_with('?')), "{parola:?}");
    assert!(g["password"]["belirsiz"].as_bool().unwrap() && !g["password"]["zorunlu"].as_bool().unwrap(), "{}", g["password"]);
    let notlar = h["ozet"]["denetlenemeyen"].to_string();
    assert!(notlar.contains("'password'"), "çalışma anında kurulan kural not olarak kalır: {notlar}");

    let j: Value = serde_json::from_str(&ok(&k, &uyg, &["rapor", "--json"])).unwrap();
    let den = j["girdiler"]["onizlemede_kontrol_edilemeyen_kurallar"].as_object().unwrap();
    for cop in ["?", "array_merge", "getImageValidationRules"] {
        assert!(!den.contains_key(cop), "{cop}: {den:?}");
    }
}

#[test]
fn doktor_bos_klasorde_genel_mesaj() {
    let k = kok("b11");
    let bos = k.0.join("bos");
    std::fs::create_dir_all(&bos).unwrap();
    let c = sezgi(&k, &bos, &["doktor"]);
    let m = metin(&c);
    assert!(!m.contains("package.json yok"), "JS'e özgü mesaj: {m}");
    let satir = m.lines().find(|l| l.contains("Uygulama")).unwrap_or_else(|| panic!("{m}"));
    for cati in ["NestJS", "FastAPI", "Django", "Laravel", "Spring Boot", "Rails", "ASP.NET Core", "Go"] {
        assert!(satir.contains(cati), "{cati} yok: {satir}");
    }
    assert!(satir.contains("sezgi kesfet"), "kaynaksız uygulama için kesfet önerisi: {satir}");
    assert!(satir.contains("sezgi doktor <klasör>"), "{satir}");

    // package.json'ı olan ama desteklenmeyen JS projesinde uygulama adı yine okunur
    std::fs::write(bos.join("package.json"), r#"{"name":"duz-js"}"#).unwrap();
    let m = metin(&sezgi(&k, &bos, &["doktor"]));
    assert!(m.contains("duz-js"), "{m}");
}
