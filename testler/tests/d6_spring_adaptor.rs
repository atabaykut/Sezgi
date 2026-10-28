//! Java + Spring Boot: @RestController / @Controller ve @RequestMapping birleşimi (sabitler, arayüz tabanlı eşleme,
//! context-path), Jakarta Validation alanları (@Valid olmadan kısıt yok), SecurityFilterChain ve yöntem güvenliğinden
//! giriş ve rol kararı (kesinlik ilkesi), Spring Data veri etkisi, JPA kaskadları, @SQLDelete, olay ve iş düğümleri.
//! Kart, OpenAPI, kanıt grafiği, önizleme, doktor, rapor ve canlı mod dilden bağımsız çalışır.
mod p0_g1_ortak;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const ORNEK: &str = "d6_spring_magaza";

fn harita(ad: &str) -> (Value, PathBuf) {
    let cikti = std::env::temp_dir().join(format!("sezgi-d6-spring-{ad}-{}", std::process::id()));
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

fn konum(h: &Value, id: &str) -> String {
    let e = uc(h, id);
    format!("{}:{}", e["dosya"].as_str().unwrap(), e["satir"])
}

fn dna<'a>(h: &'a Value, id: &str) -> &'a Value {
    &h["ozet"]["dna"][konum(h, id).as_str()]
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
fn rotalar_onekler_ve_kosullu_kayit() {
    let (h, cikti) = harita("rota");
    let idler: Vec<&str> = h["endpointler"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    let beklenen = [
        "POST /magaza/api/auth/avatar",
        "POST /magaza/api/auth/giris",
        "GET /magaza/api/rapor/gunluk",
        "GET /magaza/api/saglik",
        "POST /magaza/api/siparisler",
        "DELETE /magaza/api/siparisler/:siparisNo",
        "GET /magaza/api/siparisler/:siparisNo",
        "GET /magaza/api/urunler",
        "POST /magaza/api/urunler",
        "DELETE /magaza/api/urunler/:id",
        "GET /magaza/api/urunler/:id",
        "PUT /magaza/api/urunler/:id",
        "POST /magaza/api/yonetim/onbellek/temizle",
        "GET /magaza/api/yonetim/rapor",
        "GET /magaza/eski/her",
        "GET /magaza/eski/sayfa",
        "POST /magaza/eski/sayfa",
        "GET /magaza/kotlin",
    ];
    // context-path (yer tutucunun varsayılanı, profil belgesi değil), sabitlerle sınıf öneki, dizi önek, arayüzdeki
    // eşleme, yöntemsiz ve çok yöntemli @RequestMapping; test kaynakları okunmaz, Kotlin denetleyicisi de okunur
    assert_eq!(idler, beklenen);
    let o = &h["ozet"];
    assert_eq!(o["adaptor"], "spring@0.1.0");
    assert_eq!(o["uygulama"], "magaza");
    assert_eq!(o["genel_onek"], "/magaza");
    assert_eq!(o["ana_uygulama"], "com.ornek.magaza.MagazaApplication");
    let kosullu: Vec<&str> = o["kosullu_kapilar"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
    assert!(kosullu.contains(&"GET /magaza/eski/sayfa") && !kosullu.contains(&"GET /magaza/api/saglik"), "@Profile(\"dev\"): {kosullu:?}");
    assert!(o["denetlenemeyen"][konum(&h, "GET /magaza/eski/her").as_str()].to_string().contains("bütün HTTP yöntemlerini"), "{}", o["denetlenemeyen"]);
    // Javadoc'un ilk cümlesi özet; arayüz tabanlı denetleyicide işleyici uygulamadaki yöntem
    assert_eq!(uc(&h, "GET /magaza/api/urunler")["ozet"], "Ürünleri arar");
    assert_eq!(uc(&h, "GET /magaza/api/siparisler/:siparisNo")["dosya"], "src/main/java/com/ornek/magaza/web/SiparisController.java");
    // Yol parametresi: @PathVariable("id") Long urunId, regex kısıtlı {id:\d+}
    assert_eq!(uc(&h, "PUT /magaza/api/urunler/:id")["parametreler"], json!([{ "ad": "id", "tip": "Long" }]));
    assert_eq!(uc(&h, "DELETE /magaza/api/urunler/:id")["parametreler"][0]["ad"], "id");
    // Kart, OpenAPI dilden bağımsız; uygulama sürümü pom.xml'den (<parent> sürümü değil)
    let kart = json_oku(&cikti.join("benlik.json"));
    assert_eq!(kart["app"]["version"], "2.1.0");
    assert!(kart["generator"]["adapters"].to_string().contains("spring@0.1.0"));
    assert!(kart["operations"].as_array().unwrap().iter().any(|op| op["id"] == "urun.olustur"), "{}", kart["operations"]);
    let oa = json_oku(&cikti.join("openapi.json"));
    assert!(oa["paths"]["/magaza/api/urunler/{id}"]["delete"].is_object(), "{}", oa["paths"]);
    // springdoc varsayılan işlem kimliği: yöntem adı; iki denetleyicide aynı ad (getir) belirsiz, yazılmaz
    assert_eq!(oa["paths"]["/magaza/api/urunler"]["post"]["operationId"], "olustur");
    assert!(o["islem_kimlikleri"][konum(&h, "GET /magaza/api/urunler/:id").as_str()].is_null());
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn giris_ve_rol_kararlari() {
    let (h, cikti) = harita("giris");
    let karar = |id: &str| {
        let e = uc(&h, id);
        (e["giris"].as_str().unwrap().to_string(), e["giris_kaniti"].as_str().unwrap().to_string())
    };
    let beklenen: BTreeMap<&str, (&str, &str)> = BTreeMap::from([
        // requestMatchers(HttpMethod.GET, "/api/urunler/**").permitAll(): yalnızca GET, bağlam yolu olmadan eşlenir
        ("GET /magaza/api/urunler", ("herkese_acik", "kesin")),
        ("GET /magaza/api/urunler/:id", ("herkese_acik", "kesin")),
        ("POST /magaza/api/urunler", ("gerekli", "kesin")),
        // Sabit dizi: ACIK_YOLLAR = {"/api/auth/**", ApiYollari.SAGLIK}
        ("POST /magaza/api/auth/giris", ("herkese_acik", "kesin")),
        ("GET /magaza/api/saglik", ("herkese_acik", "kesin")),
        // access(...) kuralı tanımdan anlamlandırılamaz
        ("GET /magaza/api/rapor/gunluk", ("gerekli", "varsayim_bilinmeyen_guard")),
        // anyRequest().authenticated()
        ("GET /magaza/eski/sayfa", ("gerekli", "kesin")),
        ("DELETE /magaza/api/siparisler/:siparisNo", ("gerekli", "kesin")),
        // Zincirde hasRole("ADMIN") + sınıfta @PreAuthorize
        ("GET /magaza/api/yonetim/rapor", ("gerekli", "kesin")),
        // Yöntem güvenliği: @EnableMethodSecurity ile etkin @PreAuthorize("hasRole('ADMIN')")
        ("DELETE /magaza/api/urunler/:id", ("gerekli", "kesin")),
    ]);
    for (id, (g, k)) in beklenen {
        assert_eq!(karar(id), (g.to_string(), k.to_string()), "{id}: {:?}", uc(&h, id)["guardlar"]);
    }
    let roller = |id: &str| uc(&h, id)["roller"].clone();
    assert_eq!(roller("DELETE /magaza/api/urunler/:id"), json!(["ADMIN"]));
    // İki katman birlikte: zincir ADMIN, sınıf ADMIN ya da EDITOR -> ikisinin de izin verdiği ADMIN
    assert_eq!(roller("GET /magaza/api/yonetim/rapor"), json!(["ADMIN"]));
    assert!(uc(&h, "GET /magaza/api/yonetim/rapor")["roller_kaniti"].is_null(), "kural metninden okunan rol kesin");
    assert_eq!(roller("POST /magaza/api/urunler"), json!([]));
    // Guard metni kuralın kendisi
    assert!(uc(&h, "GET /magaza/api/urunler")["guardlar"][0].as_str().unwrap().contains("permitAll"));
    assert_eq!(h["ozet"]["genel_guardlar"], json!(["SecurityFilterChain GuvenlikAyari.filterChain"]));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn dto_alanlari_jakarta_kurallari_ve_yanit() {
    let (h, cikti) = harita("alan");
    let olustur = uc(&h, "POST /magaza/api/urunler");
    let g = alanlar(olustur, "govde");
    let zorunlu = |a: &str| g[a]["zorunlu"].as_bool().unwrap();
    // @Valid: @NotBlank / @NotNull / @NotEmpty zorunlu (üst sınıftan gelen dahil); ilkel int hiçbir zaman zorunlu değil
    assert!(zorunlu("ad") && zorunlu("fiyat") && zorunlu("etiketler") && zorunlu("kanal"));
    assert!(!zorunlu("stok") && !zorunlu("oncelik") && !zorunlu("iletisim"));
    assert_eq!(g["kanal"]["kalitimdan"], "TemelIstek");
    assert!(g.get("ic").is_none() && g.get("serialVersionUID").is_none(), "@JsonIgnore ve statik alan JSON alanı değil");
    assert!(g.get("sku_kodu").is_some() && g.get("skuKodu").is_none(), "@JsonProperty adı");
    for (a, k) in [("ad", "MinLength(2)"), ("ad", "MaxLength(100)"), ("fiyat", "IsPositive()"), ("stok", "Min(0)"), ("stok", "Max(1000)"), ("iletisim", "IsEmail()"), ("etiketler", "ArrayNotEmpty()")] {
        assert!(kurallar(&g[a]).contains(&k.to_string()), "{a}: {:?}", kurallar(&g[a]));
    }
    assert!(kurallar(&g["sku_kodu"]).contains(&r"Matches(/^(?:[A-Z]{3}-\d{4})$/)".to_string()), "{:?}", kurallar(&g["sku_kodu"]));
    assert_eq!(g["kategori"]["degerler"], json!(["ELEKTRONIK", "KITAP"]));
    // İç içe @Valid liste: öğenin alanları ve kısıtları
    assert_eq!(g["etiketler"]["ic"]["ad"]["zorunlu"], true);
    assert!(kurallar(&g["etiketler"]["ic"]["ad"]).contains(&"MaxLength(20)".to_string()));
    // @Future önizlemede denetlenemez: kural olarak bildirilir
    assert!(kurallar(&g["yayinTarihi"]).contains(&"Future()".to_string()));
    // @Valid olmayan gövde: kısıtlar çalışmaz, not düşülür
    let guncelle = uc(&h, "PUT /magaza/api/urunler/:id");
    let g2 = alanlar(guncelle, "govde");
    assert!(!g2["ad"]["zorunlu"].as_bool().unwrap() && !kurallar(&g2["ad"]).contains(&"MinLength(2)".to_string()));
    assert!(h["ozet"]["denetlenemeyen"][konum(&h, "PUT /magaza/api/urunler/:id").as_str()].to_string().contains("@Valid yok"), "{}", h["ozet"]["denetlenemeyen"]);
    // Record gövde
    let giris = alanlar(uc(&h, "POST /magaza/api/auth/giris"), "govde");
    assert!(giris["eposta"]["zorunlu"].as_bool().unwrap() && kurallar(&giris["eposta"]).contains(&"IsEmail()".to_string()));
    assert!(kurallar(&giris["sifre"]).contains(&"MinLength(8)".to_string()));
    // Arayüzdeki @Valid @RequestBody
    assert_eq!(alanlar(uc(&h, "POST /magaza/api/siparisler"), "govde")["urunId"]["zorunlu"], true);
    // Sorgu: required=false, defaultValue isteğe bağlı; ad verilen required varsayılanı zorunlu
    let s = alanlar(uc(&h, "GET /magaza/api/urunler"), "sorgu");
    assert!(!s["q"]["zorunlu"].as_bool().unwrap() && !s["sayfa"]["zorunlu"].as_bool().unwrap() && s["adet"]["zorunlu"].as_bool().unwrap(), "{s:?}");
    // Dosya yüklemesi: form gövdesi, başlık girdi değil, önizlemede denetlenemez notu
    let avatar = uc(&h, "POST /magaza/api/auth/avatar");
    assert!(alanlar(avatar, "govde").contains_key("dosya") && alanlar(avatar, "sorgu").is_empty());
    assert!(h["ozet"]["denetlenemeyen"][konum(&h, "POST /magaza/api/auth/avatar").as_str()].to_string().contains("dosya doğrulaması"));
    // Yanıt: dönüş tipi (ResponseEntity açılır, liste dizi), @ResponseStatus kodu
    assert_eq!(olustur["yanit"]["sema"]["dto"], "UrunYaniti");
    assert_eq!(olustur["yanit"]["durum"], 201);
    assert_eq!(uc(&h, "GET /magaza/api/urunler")["yanit"]["sema"]["dizi"], true);
    assert_eq!(uc(&h, "GET /magaza/api/urunler/:id")["yanit"]["sema"]["dto"], "UrunYaniti");
    assert!(uc(&h, "POST /magaza/api/auth/giris")["yanit"].is_null(), "ResponseEntity<?>: şema yok");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn veri_kaskad_olay_ve_graf() {
    let (h, cikti) = harita("veri");
    let veri = |id: &str| (uc(&h, id)["veri"]["okur"].clone(), uc(&h, id)["veri"]["yazar"].clone());
    // Arayüzden uygulamaya (UrunService -> UrunServiceImpl), ara tabanlı depo (TemelRepository<Urun>)
    assert_eq!(veri("POST /magaza/api/urunler"), (json!([]), json!(["Urun"])));
    assert_eq!(veri("GET /magaza/api/urunler"), (json!(["Urun"]), json!([])));
    assert_eq!(veri("DELETE /magaza/api/siparisler/:siparisNo"), (json!(["Siparis"]), json!(["Siparis"])));
    let kaskad = |e: &str, c: &str| h["ozet"]["kaskadlar"].as_array().unwrap().iter().any(|k| k[0] == e && k[1] == c);
    assert!(kaskad("Urun", "Yorum") && kaskad("Siparis", "SiparisKalemi"), "{}", h["ozet"]["kaskadlar"]);
    // Silme kaskadı dolaylı yazar; @SQLDelete yumuşak silmedir
    let d = dna(&h, "DELETE /magaza/api/urunler/:id");
    assert_eq!(d["dolayli_yazar"][0]["tablo"], "Yorum", "{d:#}");
    let s = dna(&h, "DELETE /magaza/api/siparisler/:siparisNo");
    assert_eq!(s["geri_alinabilir"], "kismen", "{s:#}");
    assert!(s["yumusak_silme"].as_str().unwrap().contains("Siparis.java"));
    let hata = |d: &Value, kod: u64| d["hatalar"].as_array().unwrap().iter().find(|x| x["durum"] == kod).cloned();
    assert_eq!(hata(s, 404).expect("ResponseStatusException(NOT_FOUND)")["kaynak"], "isleyici");
    // orElseThrow + @ResponseStatus sınıfı; @ExceptionHandler eşlemesi (servis zincirinde)
    let g = hata(dna(&h, "GET /magaza/api/urunler/:id"), 404).expect("KayitYokException -> 404");
    assert_eq!(g["guven"], "certain");
    assert!(g["kosul"].as_str().unwrap().contains("findById"));
    let c = hata(dna(&h, "PUT /magaza/api/urunler/:id"), 409).expect("StokYokException -> 409 (@ExceptionHandler)");
    assert_eq!((c["kaynak"].as_str(), c["guven"].as_str()), (Some("servis"), Some("inferred")));
    assert!(hata(dna(&h, "POST /magaza/api/auth/giris"), 401).is_some(), "ResponseEntity.status(401)");
    // Tetikler: publishEvent, @Async çağrısı, Kafka gönderimi
    let tetik: Vec<String> =
        dna(&h, "POST /magaza/api/urunler")["tetikler"].as_array().unwrap().iter().map(|t| format!("{}:{}", t["tur"].as_str().unwrap(), t["ad"].as_str().unwrap())).collect();
    for t in ["event:UrunOlusturuldu", "job:BildirimServisi.gonder", "event:bildirimler"] {
        assert!(tetik.contains(&t.to_string()), "{t}: {tetik:?}");
    }
    // Olay ve iş düğümleri: @EventListener, @Async, @Scheduled (@Modifying sorguyla silme, kaskad)
    let ol = &h["ozet"]["olaylar"];
    assert_eq!(ol["event:UrunOlusturuldu"]["dinleyiciler"][0]["fonksiyon"], "BildirimServisi.urunOlustu");
    assert!(ol["job:BildirimServisi.gonder"].is_object());
    assert_eq!(ol["job:BildirimServisi.temizle"]["silinen"], json!(["Urun"]));
    assert_eq!(ol["job:BildirimServisi.temizle"]["dolayli_yazar"][0]["tablo"], "Yorum");
    // Kanıt: tablo adı @Table'dan, sütun @Column'dan; işlem tabloya yazar; olay tetiği
    let depo = json_oku(&cikti.join("kanit.json"));
    let onermeler = depo["onermeler"].as_array().unwrap();
    let var = |v: &str, y: &str, f: &dyn Fn(&Value) -> bool| onermeler.iter().any(|o| o["varlik"] == v && o["yuklem"] == y && f(o));
    assert!(var("db:Urun", "TABLO", &|o| o["deger"]["tablo"] == "urunler"));
    assert!(var("db:SiparisKalemi", "TABLO", &|o| o["deger"]["tablo"] == "siparis_kalemi"));
    assert!(var("field:Urun.ad", "VAR", &|o| o["deger"]["sutun"] == "urun_adi"));
    assert!(var("api:POST /magaza/api/urunler", "WRITES", &|o| o["hedef"] == "db:Urun"));
    assert!(var("api:POST /magaza/api/urunler", "TRIGGERS", &|o| o["hedef"] == "event:UrunOlusturuldu"));
    // Graf: denetleyici düğümü ve koşullu kayıt (ortam)
    assert!(var("api:GET /magaza/api/urunler", "DEPENDS_ON", &|o| o["hedef"].as_str().is_some_and(|x| x.ends_with("UrunController.java:UrunController"))));
    assert!(var("mod:src/main/java/com/ornek/magaza/web/SayfaController.java:SayfaController", "DEPENDS_ON", &|o| o["hedef"].as_str().is_some_and(|x| x.starts_with("env:"))));
    // Kart: olay ve iş düğümleri işlem değildir
    let kart = json_oku(&cikti.join("benlik.json"));
    assert!(kart["x-nodes"].to_string().contains("job:BildirimServisi.temizle"));
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn onizleme_jakarta_kurallariyla() {
    let (_, cikti) = harita("onizle");
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", cikti.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    use std::io::{BufRead, Write};
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "onizle", "arguments": {
        "kapi": "POST /magaza/api/auth/giris", "govde": { "eposta": "eposta-degil", "sifre": "kisa" } } } });
    writeln!(p.stdin.as_mut().unwrap(), "{istek}").unwrap();
    let mut satir = String::new();
    std::io::BufReader::new(p.stdout.take().unwrap()).read_line(&mut satir).unwrap();
    let _ = p.kill();
    let _ = p.wait();
    let v: Value = serde_json::from_str(&satir).unwrap();
    let t = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(t.contains("REDDEDİLİR"), "{t}");
    assert!(t.contains("eposta") && t.contains("sifre"), "e-posta biçimi ve en az 8 karakter: {t}");
    let _ = std::fs::remove_dir_all(&cikti);
}

#[test]
fn doktor_rapor_ve_canli_mod_yol_parametresi() {
    use p0_g1_ortak::*;
    let ev = Ev::yeni("d6-spring");
    let u = Uygulama::yeni();
    let p = ev.proje(ORNEK, "a");
    let (_, d) = p.cikti(&["doktor"]);
    assert!(d.contains("Spring Boot (Java)") && d.contains("magaza"), "{d}");
    let r = p.ok(&["rapor", ".sezgi"]);
    assert!(r.contains("spring@0.1.0"), "{r}");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["urun.*:oku"], &[]);
    let (s, hata) = p.mcp(&t).cagir(json!({ "kapi": "GET /magaza/api/urunler/:id", "parametreler": { "id": "7" }, "sorgu": {} }));
    assert!(!hata, "{s}");
    let g = u.gelenler.lock().unwrap();
    assert_eq!(g.last().map(|x| (x.yontem.as_str(), x.yol.as_str())), Some(("GET", "/magaza/api/urunler/7")), "bağlam yolu ve yol parametresi");
}

/// Geçici bir Gradle projesi yazar ve haritalar
fn gradle_projesi(ad: &str, guvenlik: bool, ek: &[(&str, &str)]) -> (Value, PathBuf) {
    let kok = std::env::temp_dir().join(format!("sezgi-d6-spring-gradle-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&kok);
    let yaz = |yol: &str, m: &str| {
        let p = kok.join(yol);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, m).unwrap();
    };
    yaz("settings.gradle.kts", "rootProject.name = \"gradle-ornek\"\n");
    let bagimlilik = if guvenlik { "    implementation(\"org.springframework.boot:spring-boot-starter-security\")\n" } else { "" };
    yaz(
        "build.gradle.kts",
        &format!("plugins {{ id(\"org.springframework.boot\") version \"3.3.0\" }}\nversion = \"0.4.2\"\ndependencies {{\n    implementation(\"org.springframework.boot:spring-boot-starter-web\")\n{bagimlilik}}}\n"),
    );
    yaz("src/main/resources/application.properties", "server.servlet.contextPath=/v\nspring.jackson.deserialization.UNWRAP_ROOT_VALUE=true\n");
    yaz("src/main/java/x/Kayit.java", "package x;\n@JsonRootName(\"kayit\")\npublic class Kayit {\n    @NotBlank\n    @Benzersiz\n    private String ad;\n}\n");
    yaz("src/main/java/x/Benzersiz.java", "package x;\n@Constraint(validatedBy = BenzersizDogrulayici.class)\n@Retention(RUNTIME)\npublic @interface Benzersiz {\n    String message() default \"\";\n}\n");
    yaz(
        "src/main/java/x/MerhabaController.java",
        "package x;\nimport org.springframework.web.bind.annotation.*;\n@RestController\n@RequestMapping(\"/merhaba\")\npublic class MerhabaController {\n    @GetMapping(\"/{ad}\")\n    public String selam(@PathVariable String ad, String dil) { return ad; }\n    @PostMapping\n    public void kaydet(@Valid @RequestBody Kayit k) { }\n}\n",
    );
    for (y, m) in ek {
        yaz(y, m);
    }
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["init", kok.to_str().unwrap(), "--cikti", kok.join(".sezgi").to_str().unwrap()]).output().unwrap();
    assert!(c.status.success(), "{}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    (json_oku(&kok.join(".sezgi").join("kapilar.json")), kok)
}

#[test]
fn gradle_properties_ve_guvenlik_varsayilanlari() {
    // Güvenlik bağımlılığı yok: açık sayılır ama kesin değil
    let (h, kok) = gradle_projesi("acik", false, &[]);
    assert_eq!(h["ozet"]["uygulama"], "gradle-ornek");
    let e = uc(&h, "GET /v/merhaba/:ad");
    assert_eq!((e["giris"].as_str(), e["giris_kaniti"].as_str()), (Some("herkese_acik"), Some("varsayim")));
    // Anotasyonsuz basit parametre: isteğe bağlı sorgu
    assert_eq!(alanlar(e, "sorgu")["dil"]["zorunlu"], false);
    // UNWRAP_ROOT_VALUE: gövde @JsonRootName adıyla sarılı; projedeki özel kısıt önizlemede sınanamaz kural olarak
    let kayit = &alanlar(uc(&h, "POST /v/merhaba"), "govde")["kayit"];
    assert_eq!((kayit["zorunlu"].as_bool(), kayit["ic"]["ad"]["zorunlu"].as_bool()), (Some(true), Some(true)), "{kayit}");
    assert!(kurallar(&kayit["ic"]["ad"]).contains(&"Benzersiz()".to_string()), "{kayit}");
    let kart = json_oku(&kok.join(".sezgi").join("benlik.json"));
    assert_eq!(kart["app"]["version"], "0.4.2");
    let _ = std::fs::remove_dir_all(&kok);
    // Bağımlılık var, zincir yok: Spring Boot varsayılanı bütün istekler için giriş ister (tahmin)
    let (h, kok) = gradle_projesi("varsayilan", true, &[]);
    let e = uc(&h, "GET /v/merhaba/:ad");
    assert_eq!((e["giris"].as_str(), e["giris_kaniti"].as_str()), (Some("gerekli"), Some("varsayim")));
    let _ = std::fs::remove_dir_all(&kok);
    // Eski WebSecurityConfigurerAdapter zinciri ve @Secured (securedEnabled açık değil: tahmin)
    let ayar = "package x;\npublic class Ayar extends WebSecurityConfigurerAdapter {\n    @Override\n    protected void configure(HttpSecurity http) throws Exception {\n        http.authorizeRequests().antMatchers(\"/merhaba/**\").permitAll().and().httpBasic();\n    }\n}\n";
    let gizli = "package x;\nimport org.springframework.web.bind.annotation.*;\n@RestController\npublic class GizliController {\n    @Secured(\"ROLE_YONETICI\")\n    @DeleteMapping(\"/gizli/{id}\")\n    public void sil(@PathVariable long id) { }\n}\n";
    let (h, kok) = gradle_projesi("eski", true, &[("src/main/java/x/Ayar.java", ayar), ("src/main/java/x/GizliController.java", gizli)]);
    let e = uc(&h, "GET /v/merhaba/:ad");
    assert_eq!((e["giris"].as_str(), e["giris_kaniti"].as_str()), (Some("herkese_acik"), Some("kesin")));
    // authorizeRequests eşleşmeyen isteği geçirir; @Secured etkin değil: rol ve giriş tahmin
    let e = uc(&h, "DELETE /v/gizli/:id");
    assert_eq!((e["giris"].as_str(), e["giris_kaniti"].as_str()), (Some("gerekli"), Some("varsayim")));
    assert_eq!((e["roller"].clone(), e["roller_kaniti"].as_str()), (json!(["YONETICI"]), Some("varsayim")));
    let _ = std::fs::remove_dir_all(&kok);
}
