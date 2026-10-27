//! `sezgi rapor <harita>`: kapsam raporu. Haritada neyin bilindiğini ve neyin BİLİNMEDİĞİNİ gösterir:
//! çözülemeyen şemalar, kısmen okunan dosyalar, kontrol edilemeyen kurallar, veri bağı olmayan kapılar,
//! kullanımdan kalkmış kapılar, ikincil uygulamalar ve kartın imza durumu. Tablo bağları ve güven sayıları kanıt
//! modelinden (`kanit`, `graf`): `tani` ve Ayna ile aynı kaynak.

use crate::kural::{self, Kural};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

fn sayilar<'a>(it: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for x in it {
        *m.entry(x.to_string()).or_insert(0) += 1;
    }
    m
}

/// Rapor verisini üretir (hem ekran hem --json için)
pub fn uret(klasor: &Path) -> Result<Value, String> {
    let oku = |ad: &str| -> Result<Value, String> {
        let yol = klasor.join(ad);
        let metin = crate::guvenli::oku_harita(&yol).ok_or_else(|| tf!("{} okunamadı (önce `sezgi init`)", "could not read {} (run `sezgi init` first)", yol.display()))?;
        serde_json::from_str(&metin).map_err(|e| tf!("{ad} bozuk: {e}", "{ad} is corrupt: {e}"))
    };
    let harita = oku("kapilar.json")?;
    let kart = oku("benlik.json").unwrap_or(Value::Null);
    let ozet = &harita["ozet"];
    let kapilar: Vec<&Value> = harita["endpointler"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let id = |k: &Value| k["id"].as_str().unwrap_or("").to_string();

    let ana: Vec<&&Value> = kapilar.iter().filter(|k| k["ana"].as_bool() != Some(false)).collect();
    let ikincil = sayilar(kapilar.iter().filter(|k| k["ana"].as_bool() == Some(false)).map(|k| k["uygulama"].as_str().unwrap_or("?")));
    let giris = sayilar(ana.iter().map(|k| k["giris"].as_str().unwrap_or("?")));

    // Girdiler ve kurallar
    let (mut girdili, mut cozulemeyen, mut alan, mut kural_sayisi, mut belirsiz_alan) = (0usize, Vec::new(), 0usize, 0usize, 0usize);
    let mut denetlenemeyen: BTreeMap<String, usize> = BTreeMap::new();
    for k in &ana {
        let bolumler: Vec<&Value> = ["govde", "sorgu"].iter().flat_map(|b| k[*b].as_array().into_iter().flatten()).collect();
        if !bolumler.is_empty() {
            girdili += 1;
        }
        for g in bolumler {
            let sema = &g["sema"];
            if !sema.is_object() {
                continue;
            }
            if sema["bulunamadi"].as_bool() == Some(true) {
                cozulemeyen.push(json!({ "kapi": id(k), "sema": sema["dto"] }));
                continue;
            }
            for a in sema["alanlar"].as_object().into_iter().flat_map(|m| m.values()) {
                alan += 1;
                if a["belirsiz"].as_bool() == Some(true) {
                    belirsiz_alan += 1;
                }
                let tip = a["tip"].as_str().unwrap_or("");
                for r in a["kurallar"].as_array().into_iter().flatten().filter_map(|r| r.as_str()) {
                    kural_sayisi += 1;
                    for kr in kural::coz(r, tip) {
                        if let Kural::Bilinmiyor(m) = kr {
                            let ad = m.split('(').next().unwrap_or(&m).to_string();
                            *denetlenemeyen.entry(ad).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }

    // Veri bağları: işlemin READS/WRITES kenarı var mı (kanıt modeli; yoksa haritanın aynı alanları). Şema dosyası
    // bulunamasa da sorgudan çıkarılan tablo bağı bağdır (Express, ham SQL): "tablo bağı yok" denmez
    let model = crate::kanit::Model::yukle(klasor);
    let bagli_mi = |k: &Value| -> bool {
        match &model {
            Some(m) => m.graf.giden(&crate::graf::api_kimligi(k)).iter().any(|&i| matches!(m.depo.onermeler[i].yuklem.as_str(), "READS" | "WRITES")),
            None => ["okur", "yazar"].iter().any(|a| k["veri"][a].as_array().is_some_and(|x| !x.is_empty())),
        }
    };
    let veri_semasi = ozet["veri_semasi"].as_array().cloned().unwrap_or_default();
    let tablo_bagi = ana.iter().any(|k| bagli_mi(k));
    let veri_yok = veri_semasi.is_empty() && !tablo_bagi;
    let veri_bagsiz: Vec<String> = if veri_yok { vec![] } else { ana.iter().filter(|k| !bagli_mi(k)).map(|k| id(k)).collect() };
    let kalkmis: Vec<Value> = ana.iter().filter(|k| k["kullanimdan_kalkmis"].as_bool() == Some(true)).map(|k| json!({ "kapi": id(k), "yerine": k["yerine"] })).collect();
    let imza = &kart["signature"];
    // Kod sürümü okunamadıysa nedeni: harita projenin `.sezgi` klasöründeyse proje bir git deposunda mı
    let proje = std::fs::canonicalize(klasor).ok().filter(|k| k.file_name().is_some_and(|a| a == ".sezgi")).and_then(|k| k.parent().map(Path::to_path_buf));
    let git = proje.map(|p| crate::git_koku(&p).is_some());
    let imzali = imza["value"].as_str().map(|v| v != "UNSIGNED-DEV-BUILD").unwrap_or(false);
    let okunamayan = crate::adaptor::okunamayan_rota_kaynaklari(ozet);

    Ok(json!({
        "uygulama": ozet["uygulama"],
        "uretici": ozet["uretici"],
        "adaptor": ozet["adaptor"],
        "kaynak_ref": ozet["kaynak_ref"],
        "git": git,
        // Sayı alanları türü denetlenerek aktarılır: başka bir yerden gelmiş haritada sayı yerine metin olabilir
        "okunan": { "dosya": ozet["dosya"].as_u64(), "satir": ozet["satir"].as_u64(), "kardes_paketler": ozet["ek_paketler"], "kismen_okunan_dosyalar": ozet["kismen_okunan_dosya"] },
        "kapilar": { "ana": ana.len(), "ikincil_uygulamalar": ikincil, "giris": giris, "rol_kisitli": ana.iter().filter(|k| k["roller"].as_array().map(|r| !r.is_empty()).unwrap_or(false)).count(), "cozulemeyen_rotalar": ozet["cozulemeyen_rotalar"],
            // Rota kaynağı bulunup okunamayan yerler (bütün adaptörler): boş değilse işlem listesi eksik, kapsam kısmen
            "okunamayan_rota_kaynaklari": okunamayan, "kapsam": if okunamayan.is_empty() { "tam" } else { "kismen" } },
        "girdiler": { "girdili_kapi": girdili, "alan": alan, "kural": kural_sayisi, "zorunlulugu_belirsiz_alan": belirsiz_alan, "cozulemeyen_semalar": cozulemeyen, "onizlemede_kontrol_edilemeyen_kurallar": denetlenemeyen },
        "veri": {
            "sema_dosyalari": veri_semasi,
            "bagli_kapi": if veri_yok { Value::Null } else { json!(ana.len() - veri_bagsiz.len()) },
            "bagsiz_kapilar": veri_bagsiz,
            // Tablo adlarının kaynağı: veri şeması dosyası ya da yalnızca sorgular (o zaman tablonun varlığı da yorum)
            "tablo_kaynagi": if !veri_semasi.is_empty() { json!("sema") } else if tablo_bagi { json!("sorgu") } else { Value::Null },
        },
        // Önermelerin güven, geçerlilik ve kapsam sayıları (kanıt deposu)
        "kanit": model.as_ref().map(|m| m.sayim()).unwrap_or(Value::Null),
        // Çağrı grafiği ve derin servis analizi: grafiğin sayıları, zincirle tablo etkisi bulunan ve servis katmanında nesne
        // düzeyi yetki denetimi olan işlemler
        "cagri": cagri_ozeti(&harita, model.as_ref()),
        // Uygulamanın anlamı (alan, amaç alıntısı, problem, kullanıcı tipleri, iş yetenekleri): en fazla yorum
        "anlam": model.as_ref().map(|m| crate::anlam::ozet(&m.depo)).unwrap_or(Value::Null),
        "kullanimdan_kalkmis": kalkmis,
        // Adaptörün haritaya almadığı URL kayıtları (yer, neden)
        "haritalanmayan": ozet.get("haritalanmayan").cloned().unwrap_or_else(|| json!([])),
        // Yolu çalışma anında kurulan rota kayıtları ("dosya:satır ifade"): haritada yoktur
        "dinamik_kayitlar": ozet["dinamik_kayitlar"].as_array().cloned().unwrap_or_default(),
        // Dış etki, geri alınabilirlik ve ön koşullar (davranış özeti ve dış çağrı envanteri)
        "dis": dis_ozeti(ozet, &ana),
        "benlik_karti": if kart.is_null() { Value::Null } else { json!({ "imzali": imzali, "alg": imza["alg"], "key_id": imza["key_id"], "islem": kart["operations"].as_array().map(|a| a.len()), "tam": kart["coverage"]["operations_complete"] }) },
        "sure_ms": ozet["sure_ms"]["toplam"].as_u64(),
    }))
}

/// Dış etki bölümü: dış sistemler ve çağrılar, dış etkili işlemler, geri alınabilirlik dağılımı (yazma işlemlerinde),
/// koşullu red (ön koşul) sayısı ve dış etki ya da geri alınamazlık yüzünden onay isteyen işlemler
fn dis_ozeti(ozet: &Value, ana: &[&&Value]) -> Value {
    let d = &ozet["dis"];
    if d.is_null() {
        return Value::Null;
    }
    let dna = |k: &Value| &ozet["dna"][format!("{}:{}", k["dosya"].as_str().unwrap_or(""), k["satir"]).as_str()];
    let mut geri: BTreeMap<String, usize> = ["evet", "kismen", "hayir", "bilinmiyor"].iter().map(|x| (x.to_string(), 0)).collect();
    let (mut etkili, mut on_kosul_islem, mut on_kosul, mut onay) = (0usize, 0usize, 0usize, 0usize);
    // Çağrı zincirinde sınıflandırılamayan çağrı yüzünden dış etkisi bilinmeyen işlemler (yazma, okuma)
    let (mut bilinmiyor_yazma, mut bilinmiyor_okuma) = (0usize, 0usize);
    for k in ana {
        let v = dna(k);
        if v["dis"].as_array().is_some_and(|a| !a.is_empty()) {
            etkili += 1;
        }
        let n = v["hatalar"].as_array().into_iter().flatten().filter(|h| h["kosul"].as_str().is_some_and(|x| x != "catch" && x != "except")).count();
        if n > 0 {
            on_kosul_islem += 1;
            on_kosul += n;
        }
        if !matches!(k["yontem"].as_str(), Some("GET" | "HEAD" | "OPTIONS")) {
            *geri.entry(v["geri_alinabilir"].as_str().unwrap_or("bilinmiyor").to_string()).or_insert(0) += 1;
        }
        // Yalnızca dış etkisi bilinmediği için onay isteyen yazma ayrı sayılır (aşağıda; Ayna'nın 14. sorusuyla aynı)
        let yalniz_bilinmeyen = k["yontem"] != "DELETE" && k["etki_onayi"].as_str().is_some_and(|g| g.starts_with("işlemin dış etkisi bilinmiyor"));
        if k["etki_onayi"].is_string() && !yalniz_bilinmeyen {
            onay += 1;
        }
        if v["dis_etki"] == "bilinmiyor" {
            if matches!(k["yontem"].as_str(), Some("GET" | "HEAD" | "OPTIONS")) {
                bilinmiyor_okuma += 1;
            } else {
                bilinmiyor_yazma += 1;
            }
        }
    }
    let mut sistemler: Vec<Value> = d["sistemler"].as_array().cloned().unwrap_or_default();
    sistemler.sort_by_key(|s| std::cmp::Reverse(s["islemler"].as_array().map(Vec::len).unwrap_or(0)));
    json!({
        "cagri": d["istatistik"]["cagri"], "isleme_bagli": d["istatistik"]["isleme_bagli"], "taranan_dosya": d["istatistik"]["dosya"],
        "sistemler": sistemler, "dinleyiciler": d["dinleyiciler"].as_array().map(Vec::len).unwrap_or(0),
        "dis_etkili_islem": etkili, "geri_alinabilirlik": geri, "on_kosullu_islem": on_kosul_islem, "on_kosul": on_kosul,
        "etki_yuzunden_onay": onay, "eklenen_on_kosul": d["istatistik"]["eklenen_on_kosul"],
        "dis_etki_bilinmiyor_yazma": bilinmiyor_yazma, "dis_etki_bilinmiyor_okuma": bilinmiyor_okuma,
    })
}

/// Çağrı grafiğinin rapordaki özeti (kanıt deposunun yan bilgisinden ve haritanın davranış özetinden)
fn cagri_ozeti(harita: &Value, model: Option<&crate::kanit::Model>) -> Value {
    let Some(c) = model.map(|m| &m.depo.ek["cagri"]).filter(|c| c.is_object()) else { return Value::Null };
    let mut v = json!({});
    for a in ["dosya", "fonksiyon", "kenar", "kesin", "yorum", "dinamik", "ulasilan_fonksiyon", "ulasilan_kenar", "model"] {
        v[a] = c[a].clone();
    }
    let (mut tablo, mut nesne, mut eksik) = (0usize, 0usize, 0usize);
    for k in harita["endpointler"].as_array().into_iter().flatten().filter(|k| k["ana"].as_bool() != Some(false)) {
        let d = &harita["ozet"]["dna"][format!("{}:{}", k["dosya"].as_str().unwrap_or(""), k["satir"]).as_str()];
        tablo += usize::from(d["tablo_etkisi"].as_array().is_some_and(|t| !t.is_empty()));
        nesne += usize::from(
            d["erisim_denetimi"].as_array().is_some_and(|e| e.iter().any(|x| x["nesne_duzeyi"] == true)) || d["sahiplik_denetimi"].as_array().is_some_and(|s| !s.is_empty()),
        );
        eksik += usize::from(d["cagri"]["dinamik"].as_u64().unwrap_or(0) > 0 || d["cagri"]["kesildi"] == true);
    }
    v["zincirle_tablo_bulunan_islem"] = json!(tablo);
    v["nesne_duzeyi_denetimli_islem"] = json!(nesne);
    v["zinciri_eksik_islem"] = json!(eksik);
    v
}

fn yuzde(a: usize, b: usize) -> String {
    if b == 0 {
        "-".into()
    } else {
        tf!("%{:.1}", "{:.1}%", 100.0 * a as f64 / b as f64)
    }
}

/// Ekrana okunur rapor
pub fn yazdir(r: &Value) {
    use crate::dil::cogul;
    let n = |v: &Value| v.as_u64().unwrap_or(0) as usize;
    let ana = n(&r["kapilar"]["ana"]);
    tp!("◆ Sezgi kapsam raporu · {}", "◆ Sezgi coverage report · {}", r["uygulama"].as_str().unwrap_or(""));
    let kod = match (r["kaynak_ref"].as_str().unwrap_or("unknown"), r["git"].as_bool()) {
        ("unknown", Some(false)) => t!("kod sürümü yok (git yok)", "no code version (no git)").to_string(),
        ("unknown", Some(true)) => t!("kod sürümü okunamadı (git commit'i çözülemedi)", "code version unreadable (git commit could not be resolved)").to_string(),
        ("unknown", None) => t!("kod sürümü bilinmiyor", "code version unknown").to_string(),
        (c, _) => tf!("kod {c}", "code {c}"),
    };
    tp!("  Sürüm        {} · {} · {kod}", "  Version      {} · {} · {kod}", r["uretici"].as_str().unwrap_or(""), r["adaptor"].as_str().unwrap_or(""));
    tp!("  Okunan       {} dosya · {} satır", "  Read         {} {f} · {} {l}", n(&r["okunan"]["dosya"]), n(&r["okunan"]["satir"]); f = cogul(&r["okunan"]["dosya"], "file", "files"), l = cogul(&r["okunan"]["satir"], "line", "lines"));
    let kardes: Vec<&str> = r["okunan"]["kardes_paketler"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
    if !kardes.is_empty() {
        tp!("  Kardeş paket {}", "  Siblings     {}", kardes.join(", "));
    }
    let giris: Vec<String> = r["kapilar"]["giris"].as_object().into_iter().flatten().map(|(k, v)| format!("{v} {}", crate::yardim::giris_metni(k))).collect();
    // Anlam: her satır kendi damgasıyla (en fazla yorum); alıntı uygulamanın kendi beyanıdır
    for (i, l) in crate::anlam::rapor_satirlari(&r["anlam"]).iter().enumerate() {
        println!("{}{l}", if i == 0 { t!("\n  ANLAM        ", "\n  MEANING      ") } else { "               " });
    }
    tp!(
        "\n  İŞLEMLER     {} ana uygulamada · {} · {} rol kısıtlı",
        "\n  OPERATIONS   {} in the main app · {} · {} role-restricted",
        ana,
        giris.join(", "),
        n(&r["kapilar"]["rol_kisitli"])
    );
    for (ad, sayi) in r["kapilar"]["ikincil_uygulamalar"].as_object().into_iter().flatten() {
        tp!("  ↳ ikincil   {ad}: {sayi} işlem (Benlik Kartı'na girmez)", "  ↳ secondary {ad}: {sayi} {i} (not in the Benlik Card)"; i = cogul(sayi, "operation", "operations"));
    }
    let okunamayan: Vec<&str> = r["kapilar"]["okunamayan_rota_kaynaklari"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
    if !okunamayan.is_empty() {
        tp!(
            "  ! kapsam kısmen · okunamayan rota kaynağı: {} yer; bu yerlerin işlemleri haritada yok (liste eksik, sayı kesin değil):",
            "  ! coverage partial · unreadable route source: {} {p}; their operations are not in the map (the list is incomplete, the count is not certain):",
            okunamayan.len();
            p = cogul(okunamayan.len(), "place", "places")
        );
        for x in okunamayan.iter().take(10) {
            println!("      {}", crate::adaptor::okunamayan_rota_satiri(x));
        }
        if okunamayan.len() > 10 {
            tp!("      … {} yer daha (tamamı: --json)", "      … {} more (all: --json)", okunamayan.len() - 10);
        }
    }
    let rotasiz: Vec<&str> = r["kapilar"]["cozulemeyen_rotalar"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
    if !rotasiz.is_empty() {
        tp!(
            "  ? {} rota tanımı işleme çevrilemedi (dinamik kayıt, Rack uygulaması ya da denetleyicisi / eylemi olmayan rota):",
            "  ? {} route {t} could not become operations (dynamic registration, Rack app, or a route without a controller / action):",
            rotasiz.len();
            t = cogul(rotasiz.len(), "definition", "definitions")
        );
        for x in rotasiz.iter().take(5) {
            println!("      {x}");
        }
        if rotasiz.len() > 5 {
            tp!("      … {} tanım daha (tamamı: --json)", "      … {} more {t} (all: --json)", rotasiz.len() - 5; t = cogul(rotasiz.len() - 5, "definition", "definitions"));
        }
    }

    let g = &r["girdiler"];
    let coz: Vec<&Value> = g["cozulemeyen_semalar"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    tp!(
        "\n  GİRDİLER     {} girdili işlem · {} alan · {} kural",
        "\n  INPUTS       {} {i} with inputs · {} {a} · {} {k}",
        n(&g["girdili_kapi"]),
        n(&g["alan"]),
        n(&g["kural"]);
        i = cogul(&g["girdili_kapi"], "operation", "operations"),
        a = cogul(&g["alan"], "field", "fields"),
        k = cogul(&g["kural"], "rule", "rules")
    );
    let belirsiz = n(&g["zorunlulugu_belirsiz_alan"]);
    if belirsiz > 0 {
        tp!(
            "  ? {belirsiz} alanın zorunluluğu koddan çıkarılamadı (doğrulayıcısı yok ya da özel dekoratörün tanımı izlenemedi)",
            "  ? whether {belirsiz} {a} required could not be read from the code (no validator, or a custom decorator could not be followed)";
            a = cogul(belirsiz, "field is", "fields are")
        );
    }
    if coz.is_empty() {
        tp!("  ✓ bütün girdi tanımları bulundu", "  ✓ all input definitions found");
    } else {
        tp!("  ? {} girdi tanımı bulunamadı (bu işlemlere yazma önerilmez):", "  ? {} input {t} not found (writing to these operations is not recommended):", coz.len(); t = cogul(coz.len(), "definition", "definitions"));
        for c in &coz {
            println!("      {} · {}", c["kapi"].as_str().unwrap_or(""), c["sema"].as_str().unwrap_or(""));
        }
    }
    let den: Vec<(String, usize)> =
        g["onizlemede_kontrol_edilemeyen_kurallar"].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0) as usize)).collect();
    if !den.is_empty() {
        let toplam: usize = den.iter().map(|(_, v)| v).sum();
        let mut sirali = den.clone();
        sirali.sort_by_key(|a| std::cmp::Reverse(a.1));
        let ilk: Vec<String> = sirali.iter().take(8).map(|(k, v)| format!("{k} {v}")).collect();
        tp!(
            "  ? {} kural önizlemede kontrol edilemez (özel doğrulayıcı ya da sabit adlı sınır): {}",
            "  ? {} {k} cannot be checked by the preview (custom validator or a bound given by a named constant): {}",
            toplam,
            ilk.join(", ");
            k = cogul(toplam, "rule", "rules")
        );
    }

    let v = &r["veri"];
    let semalar: Vec<&str> = v["sema_dosyalari"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
    if v["bagli_kapi"].is_null() {
        tp!(
            "\n  VERİ         veritabanı tanımı ve tablo bağı bulunamadı (desteklenen: Prisma, SQLModel / SQLAlchemy, Django modelleri, JPA / Spring Data, Exposed, ActiveRecord ve db/schema.rb, Eloquent / Laravel göçleri, Entity Framework Core, Express sorguları, GORM, Go SQL sorguları)",
            "\n  DATA         no database definition or table link found (supported: Prisma, SQLModel / SQLAlchemy, Django models, JPA / Spring Data, Exposed, ActiveRecord and db/schema.rb, Eloquent / Laravel migrations, Entity Framework Core, Express queries, GORM, Go SQL queries)"
        );
    } else {
        let bagli = n(&v["bagli_kapi"]);
        // Çok sayıda tanım dosyası (ör. her Eloquent modeli ayrı dosya) ilk birkaçı ve sayısıyla; tamamı --json'da
        let kaynak = match semalar.len() {
            0 => t!("tablo adları sorgulardan çıkarıldı, şema dosyası yok [Yorum]", "table names taken from queries, no schema file [Inferred]").to_string(),
            1..=4 => tf!("tanım dosyası: {}", "definition file: {}", semalar.join(", ")),
            s => tf!("{s} tanım dosyası: {} ve {} dosya daha (tamamı: --json)", "{s} definition files: {} and {} more (all: --json)", semalar[..3].join(", "), s - 3),
        };
        tp!(
            "\n  VERİ         {} / {} işlem bir tabloya bağlı ({}) · {kaynak}",
            "\n  DATA         {} / {} {i} linked to a table ({}) · {kaynak}",
            bagli,
            ana,
            yuzde(bagli, ana);
            i = cogul(ana, "operation", "operations")
        );
        let bagsiz: Vec<&str> = v["bagsiz_kapilar"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
        if !bagsiz.is_empty() {
            tp!(
                "  · {} işlemde tablo bağı yok (dış servis çağrısı, derlenmiş kütüphane ya da izlenemeyen çağrı olabilir):",
                "  · {} {i} no table link (an external service call, a compiled library or a call that could not be followed):",
                bagsiz.len();
                i = cogul(bagsiz.len(), "operation has", "operations have")
            );
            for b in bagsiz.iter().take(10) {
                println!("      {b}");
            }
            if bagsiz.len() > 10 {
                tp!("      … {} işlem daha (tamamı: --json)", "      … {} more {i} (all: --json)", bagsiz.len() - 10; i = cogul(bagsiz.len() - 10, "operation", "operations"));
            }
        }
    }

    let c = &r["cagri"];
    if c.is_object() {
        tp!(
            "\n  ÇAĞRI        {} fonksiyon · {} çağrı kenarı ({} kesin, {} yorum) · işlemlerden {} fonksiyona iniliyor",
            "\n  CALLS        {} {f} · {} call {k} ({} certain, {} inferred) · operations reach {} {u}",
            n(&c["fonksiyon"]),
            n(&c["kenar"]),
            n(&c["kesin"]),
            n(&c["yorum"]),
            n(&c["ulasilan_fonksiyon"]);
            f = cogul(&c["fonksiyon"], "function", "functions"),
            k = cogul(&c["kenar"], "edge", "edges"),
            u = cogul(&c["ulasilan_fonksiyon"], "function", "functions")
        );
        tp!(
            "  · {} işlemin tablo etkisi servis / depo katmanından · {} işlemde nesne düzeyi yetki denetimi (erişim denetimi ya da sahiplik)",
            "  · {} {z} table effects from the service / repository layer · {} {d} object-level authorization checks (access check or ownership)",
            n(&c["zincirle_tablo_bulunan_islem"]),
            n(&c["nesne_duzeyi_denetimli_islem"]);
            z = cogul(&c["zincirle_tablo_bulunan_islem"], "operation has", "operations have"),
            d = cogul(&c["nesne_duzeyi_denetimli_islem"], "operation has", "operations have")
        );
        if n(&c["dinamik"]) > 0 {
            tp!(
                "  ? {} çağrının hedefi koddan bilinmez (dizin erişimi, yansıma, parametre olarak gelen fonksiyon); {} işlemin zinciri eksik olabilir",
                "  ? the target of {} {k} is unknown from the code (index access, reflection, a function passed as a parameter); the chain of {} {i} may be incomplete",
                n(&c["dinamik"]),
                n(&c["zinciri_eksik_islem"]);
                k = cogul(&c["dinamik"], "call", "calls"),
                i = cogul(&c["zinciri_eksik_islem"], "operation", "operations")
            );
        }
    }

    let dinamik: Vec<&str> = r["dinamik_kayitlar"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
    if !dinamik.is_empty() {
        tp!(
            "\n  DİNAMİK      {} rota kaydının yolu çalışma anında kuruluyor; bu rotalar haritada yok:",
            "\n  DYNAMIC      {} route registrations build their path at run time; these routes are not in the map:",
            dinamik.len()
        );
        for d in dinamik.iter().take(10) {
            println!("      {d}");
        }
    }

    let dis = &r["dis"];
    if !dis.is_null() {
        let sis: Vec<&Value> = dis["sistemler"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        tp!(
            "
        DIŞ ETKİ     {} giden çağrı ({} dosya tarandı) · {} işleme bağlı · {} dış sistem · {} işlem dışarıya dokunuyor",
            "
        EXTERNAL     {} outgoing {c} ({} {f} scanned) · {} tied to operations · {} external {s} · {} {i} outside",
            n(&dis["cagri"]),
            n(&dis["taranan_dosya"]),
            n(&dis["isleme_bagli"]),
            sis.len(),
            n(&dis["dis_etkili_islem"]);
            c = cogul(&dis["cagri"], "call", "calls"),
            f = cogul(&dis["taranan_dosya"], "file", "files"),
            s = cogul(sis.len(), "system", "systems"),
            i = cogul(&dis["dis_etkili_islem"], "operation reaches", "operations reach")
        );
        for s in sis.iter().take(8) {
            let hedef = match s["hedef_turu"].as_str() {
                Some("ortam") => tf!("adres ortam değişkeninden: {} [Yorum]", "address from an environment variable: {} [Inferred]", s["hedef"].as_str().unwrap_or("")),
                Some("url") => tf!("{} [Kesin]", "{} [Certain]", s["hedef"].as_str().unwrap_or("")),
                Some("sdk") => format!("SDK {}", s["hedef"].as_str().unwrap_or("")),
                _ => t!("adres çözülemedi [Bilinmiyor]", "address not resolved [Unknown]").to_string(),
            };
            tp!(
                "      {} · {hedef} · {} çağrı, {} işlem",
                "      {} · {hedef} · {} {c}, {} {i}",
                s["tur"].as_str().unwrap_or(""),
                n(&s["cagri"]),
                s["islemler"].as_array().map(Vec::len).unwrap_or(0);
                c = cogul(&s["cagri"], "call", "calls"),
                i = cogul(s["islemler"].as_array().map(Vec::len).unwrap_or(0), "operation", "operations")
            );
        }
        if sis.len() > 8 {
            tp!("      … {} sistem daha (tamamı: --json)", "      … {} more {s} (all: --json)", sis.len() - 8; s = cogul(sis.len() - 8, "system", "systems"));
        }
        // Çağrı zincirinde sınıflandırılamayan kütüphane / dinamik çağrı ya da kesilen zincir: yazma onaysız gidemez
        if dis["dis_etki_bilinmiyor_yazma"].as_u64().unwrap_or(0) + dis["dis_etki_bilinmiyor_okuma"].as_u64().unwrap_or(0) > 0 {
            tp!(
                "      dış etkisi bilinmeyen: {} yazma (onay ister) · {} okuma (bilgi) [Bilinmiyor]: zincirde sınıflandırılamayan çağrı",
                "      external effect unknown: {} {y} (need approval) · {} {o} (information) [Unknown]: an unclassified call in the chain",
                n(&dis["dis_etki_bilinmiyor_yazma"]),
                n(&dis["dis_etki_bilinmiyor_okuma"]);
                y = crate::dil::cogul(&dis["dis_etki_bilinmiyor_yazma"], "write", "writes"),
                o = crate::dil::cogul(&dis["dis_etki_bilinmiyor_okuma"], "read", "reads")
            );
        }
        let g = &dis["geri_alinabilirlik"];
        tp!(
            "  GERİ ALMA    yazma işlemleri: {} evet · {} kısmen · {} hayır · {} bilinmiyor [Yorum] · {} işlem dış etki ya da geri alınamazlık yüzünden her seferinde onay ister",
            "  UNDO         writes: {} yes · {} partly · {} no · {} unknown [Inferred] · {} {i} approval every time because of an external effect or because {k} be undone",
            n(&g["evet"]),
            n(&g["kismen"]),
            n(&g["hayir"]),
            n(&g["bilinmiyor"]),
            n(&dis["etki_yuzunden_onay"]);
            i = cogul(&dis["etki_yuzunden_onay"], "operation needs", "operations need"),
            k = cogul(&dis["etki_yuzunden_onay"], "it cannot", "they cannot")
        );
        tp!(
            "  ÖN KOŞUL     {} işlemde {} koşullu red (iş durumu ön koşulu); önizleme bunları denetleyemez, GEÇER demez",
            "  PRECONDITION {} {i} with {} conditional {k} (business-state preconditions); the preview cannot check them and does not say PASS",
            n(&dis["on_kosullu_islem"]),
            n(&dis["on_kosul"]);
            i = cogul(&dis["on_kosullu_islem"], "operation", "operations"),
            k = cogul(&dis["on_kosul"], "rejection", "rejections")
        );
    }

    let kalkmis: Vec<&Value> = r["kullanimdan_kalkmis"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if !kalkmis.is_empty() {
        tp!("\n  KALKMIŞ      {} işlem kullanımdan kaldırılmış:", "\n  DEPRECATED   {} {i} deprecated:", kalkmis.len(); i = cogul(kalkmis.len(), "operation is", "operations are"));
        for k in &kalkmis {
            println!("      {}{}", k["kapi"].as_str().unwrap_or(""), k["yerine"].as_str().map(|y| format!(" → {y}")).unwrap_or_default());
        }
    }

    let haritasiz: Vec<&Value> = r["haritalanmayan"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if !haritasiz.is_empty() {
        tp!(
            "\n  HARİTASIZ    {} URL kaydı haritaya alınmadı (yönetim paneli, proje dışı yapılandırma ya da çözülemeyen görünüm):",
            "\n  UNMAPPED     {} URL {k} not in the map (admin panel, configuration outside the project or an unresolved view):",
            haritasiz.len();
            k = cogul(haritasiz.len(), "registration", "registrations")
        );
        for h in haritasiz.iter().take(10) {
            println!("      {} · {}", h["yer"].as_str().unwrap_or(""), h["neden"].as_str().unwrap_or(""));
        }
        if haritasiz.len() > 10 {
            tp!("      … {} kayıt daha (tamamı: --json)", "      … {} more {k} (all: --json)", haritasiz.len() - 10; k = cogul(haritasiz.len() - 10, "registration", "registrations"));
        }
    }

    let kismi: Vec<&str> = r["okunan"]["kismen_okunan_dosyalar"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
    if !kismi.is_empty() {
        tp!(
            "\n  OKUMA        {} dosya kısmen okundu (ayrıştırıcı hata buldu; bu dosyalardaki bilgi eksik olabilir):",
            "\n  PARSING      {} {f} partly read (the parser found errors; what these files say may be incomplete):",
            kismi.len();
            f = cogul(kismi.len(), "file", "files")
        );
        for d in kismi.iter().take(10) {
            println!("      {d}");
        }
    }

    let k = &r["benlik_karti"];
    if k.is_null() {
        tp!("\n  KART         benlik.json yok", "\n  CARD         no benlik.json");
    } else {
        tp!(
            "\n  KART         {} işlem · {} tam{} · {}",
            "\n  CARD         {} {i} · {} complete{} · {}",
            n(&k["islem"]),
            n(&k["tam"]),
            if r["kapilar"]["kapsam"] == "kismen" { t!(" · işlem listesi kısmen (okunamayan rota kaynağı var)", " · operation list partial (unreadable route sources)") } else { "" },
            if k["imzali"].as_bool() == Some(true) {
                tf!("imzalı ({}, anahtar '{}')", "signed ({}, key '{}')", k["alg"].as_str().unwrap_or(""), k["key_id"].as_str().unwrap_or(""))
            } else {
                t!("imzasız", "unsigned").into()
            };
            i = cogul(&k["islem"], "operation", "operations")
        );
    }
    let kn = &r["kanit"];
    if !kn.is_null() {
        let g = &kn["guven"];
        tp!(
            "\n  KANIT        {} önerme · {} kesin · {} gözlem · {} yorum · {} bilinmiyor · {} kanıt · rev {}",
            "\n  EVIDENCE     {} {o} · {} certain · {} observed · {} inferred · {} unknown · {} evidence · rev {}",
            n(&kn["onerme"]),
            n(&g["certain"]),
            n(&g["observed"]),
            n(&g["inferred"]),
            n(&g["unknown"]),
            n(&kn["kanit"]),
            n(&kn["revizyon"]);
            o = cogul(&kn["onerme"], "claim", "claims")
        );
        if n(&kn["eski"]) > 0 {
            tp!("  ⚠ {} önerme eski: dayandığı dosya init'ten sonra değişti → sezgi init", "  ⚠ {} {o} stale: {d} file changed after init → sezgi init", n(&kn["eski"]); o = cogul(&kn["eski"], "claim is", "claims are"), d = cogul(&kn["eski"], "its", "their"));
        }
        if n(&kn["celiski"]) > 0 {
            tp!("  ✗ {} önermede çelişen kanıt var (güven en fazla yorum)", "  ✗ {} {o} contradicting evidence (trust at most inferred)", n(&kn["celiski"]); o = cogul(&kn["celiski"], "claim has", "claims have"));
        }
        if kn["haritadan"].as_bool() == Some(true) {
            tp!("  · kanit.json yok: haritadan hesaplandı (sezgi init yazar)", "  · no kanit.json: computed from the map (sezgi init writes it)");
        }
    }
    tp!("\n  Temiz bir rapor \"bilinen eksik yok\" demektir, eksiksizlik kanıtı değildir.", "\n  A clean report means \"no known gaps\"; it is not proof of completeness.");
}
