//! Kapılardan Benlik Card (Draft 0.1) üretir. Koddan doğrudan okunan olgular "certain"; yokluktan ya da
//! addan çıkarılanlar (guard bulunamadığı için açık, HTTP yönteminden etki, tablo bağları) "inferred";
//! girdi şeması bulunamayan işlemler "unknown" damgası alır ve yazmaya açılmaz.
//!
//! Kart 0.1.1 (geriye uyumlu; yeni alanlar `x-` önekli, 0.1 doğrulayıcıları onları yok sayar):
//! - `x-issued_at` / `x-expires_at`: kartın geçerlilik süresi (imza bunları kapsar);
//! - `x-basis`: işlemlerin dayandığı kaynak dosyaların SHA-256 özetleri. `stale` elle yazılmaz, bu özetlerden hesaplanır;
//! - `x-revision`: işlem kümesinin kanonik (JCS) özeti; `changes` commit'e değil buna bağlıdır;
//! - `x-map_digest`: kartla birlikte yazılan haritanın (`kapilar.json`) kanonik (JCS) SHA-256 özeti. Canlı modun onay
//!   kararı haritadan okunur; harita karttan sonra değiştiyse yazma yapılmaz (kart imzalıysa bu bağ da imzalıdır);
//! - `x-evidence` (kayıt başına JCS özeti) + `x-evidence_root` (RFC 6962 Merkle kökü) + ayrı kanıt paketi;
//! - `operation.x-dna`: işlemin davranış özeti (giriş, hata koşulları, veri etkisi, geri alma, kapsam).

use crate::adaptor::{alan_tipi, Kapi};
use crate::kural::{self, Kural};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Kartın biçim sürümü. 0.1 deseni (`^0\.1(\.\d+)?$`) bunu kabul eder: 0.1 doğrulayıcısı kartı reddetmez
pub const SURUM: &str = "0.1.1";
/// Kanıt paketinin kartın yanındaki dosya adı
pub const KANIT_PAKETI: &str = "benlik-evidence.json";
/// Varsayılan geçerlilik süresi (gün); `sezgi init --gecerlilik <gün>` ile değişir
pub const VARSAYILAN_GECERLILIK_GUN: u64 = 30;

fn islem_kimligi(k: &Kapi) -> String {
    islem_kimligi_adlardan(&k.kontrolcu, &k.fonksiyon)
}

/// Alanın koddan okunan özellikleri: tip, izin verilen değerler, sınırlar ve biçim (kart ve OpenAPI için ortak)
pub fn alan_ozellikleri(a: &crate::adaptor::Alan) -> Value {
    let mut alan = json!({ "type": alan_tipi(&a.tip, &a.kurallar) });
    if !a.degerler.is_empty() {
        alan["enum"] = json!(a.degerler);
    }
    // Sınırlar: dizinin her öğesine uygulanan kurallar ({ each: true }) alanın kendisini sınırlamaz
    let ham_tip = a.tip.clone().unwrap_or_default();
    let cozulmus = a.kurallar.iter().filter(|r| !kural::her_oge(r)).flat_map(|r| kural::coz(r, &ham_tip));
    for k in kural::tip_kurallari(&ham_tip).into_iter().chain(cozulmus) {
        match k {
            Kural::Min(n) => alan["min"] = json!(n),
            Kural::Max(n) => alan["max"] = json!(n),
            Kural::MinUzunluk(n) | Kural::MinKarakter(n) => alan["min_length"] = json!(n as u64),
            Kural::MaxUzunluk(n) | Kural::MaxKarakter(n) => alan["max_length"] = json!(n as u64),
            Kural::Eposta => alan["format"] = json!("email"),
            Kural::Uuid => alan["format"] = json!("uuid"),
            Kural::Url => alan["format"] = json!("uri"),
            _ => {}
        }
    }
    alan
}

/// Kartın işlem kimliği (tekilleştirme eki hariç): ActivityController.getActivities -> activity.get-activities
pub fn islem_kimligi_adlardan(kontrolcu: &str, fonksiyon: &str) -> String {
    let kontrolcu = kontrolcu.trim_end_matches("Controller");
    let mut s = String::new();
    for (i, ch) in format!("{}.{}", kontrolcu, fonksiyon).chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !s.ends_with('.') {
                s.push('-');
            }
            s.extend(ch.to_lowercase());
        } else if ch.is_ascii_alphanumeric() || ch == '.' {
            s.push(ch);
        }
    }
    s
}

/// Bir metot birden fazla adrese bağlıysa (@Get(['/a', '/b'])) kimlik adresle ayrıştırılır
fn tekil(goru: &mut std::collections::HashSet<String>, id: String, yol: &str) -> String {
    if goru.insert(id.clone()) {
        return id;
    }
    // Kimlik deseni: [a-z0-9_.-]
    let ek: String = yol
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let mut aday = format!("{id}.{ek}");
    let mut n = 2;
    while !goru.insert(aday.clone()) {
        aday = format!("{id}.{ek}-{n}");
        n += 1;
    }
    aday
}

/// Değeri kısıtlamayan işaretler (kartta "reddeder" diye yazılmaz)
fn reddetmez(r: &str) -> bool {
    let ad = r.split('(').next().unwrap_or(r);
    matches!(
        ad,
        "IsOptional"
            | "ValidateIf"
            | "Type"
            | "Transform"
            | "of"
            | "nullable"
            | "optional"
            | "nullish"
            | "default"
            | "describe"
            | "openapi"
            | "meta"
            | "trim"
            | "toLowerCase"
            | "toUpperCase"
    )
}

fn etki(yontem: &str) -> &'static str {
    match yontem {
        "GET" | "HEAD" | "OPTIONS" => "read",
        "DELETE" => "destructive",
        _ => "write",
    }
}

/// "@acme/api" -> "acme.api": kimlik küçük harf, [a-z0-9_.-]
pub(crate) fn uygulama_kimligi(ad: &str) -> String {
    let s: String = ad.trim_start_matches('@').to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '.' }).collect();
    let s = s.trim_matches('.').to_string();
    if s.is_empty() {
        "app".into()
    } else {
        s
    }
}

/// İşlemin damgası (SPEC 3), haritadaki kapıdan: yazmanın dayanacağı olguların en zayıfı. Şeması bilinmeyen ya da
/// çözülemeyen girdi (dizi gövde dahil) varsa unknown; giriş kararı kesin değilse, zorunluluğu belirsiz bir alan ya da
/// kesin olmayan bir rol kararı varsa inferred; değilse certain. Kart ve canlı modun onay kuralı bu fonksiyonu kullanır.
pub fn islem_damgasi(k: &Value) -> &'static str {
    let girdiler = || k["govde"].as_array().into_iter().flatten().chain(k["sorgu"].as_array().into_iter().flatten());
    let eksik = girdiler().any(|g| match g.get("sema") {
        Some(s) => s["bulunamadi"] == true || s["dizi"] == true,
        None => true,
    });
    let belirsiz = girdiler().filter_map(|g| g.get("sema")).any(|s| s["alanlar"].as_object().into_iter().flatten().any(|(_, a)| a["belirsiz"] == true));
    if eksik {
        "unknown"
    } else if k["giris_kaniti"] != "kesin" || belirsiz || k.get("roller_kaniti").is_some_and(|r| !r.is_null()) {
        "inferred"
    } else {
        "certain"
    }
}

/// Kartın `operation.x-response` üyesi: başarılı yanıtın şeması (durum, tip adı, dizi mi, alanlar), damgası (açık beyan ya
/// da tek anlamlı tip certain, çıkarım inferred) ve kaynağı
fn yanit_karti(y: &crate::adaptor::Yanit) -> Value {
    let alanlar: Map<String, Value> =
        y.sema.alanlar.iter().map(|(ad, a)| (ad.clone(), json!({ "type": crate::adaptor::alan_tipi(&a.tip, &a.kurallar), "required": a.zorunlu && !a.belirsiz }))).collect();
    let satir = y.sema.alanlar.values().map(|a| a.satir).filter(|s| *s > 0).min().unwrap_or(1);
    let mut v = json!({
        "type": y.sema.dto,
        "array": y.sema.dizi,
        "fields": alanlar,
        "stamp": if y.kesin { "certain" } else { "inferred" },
        "basis": y.kaynak,
    });
    if let Some(d) = y.durum {
        v["status"] = json!(d);
    }
    if let Some(f) = y.sema.dosya.as_deref().filter(|f| !f.is_empty()) {
        v["sources"] = json!([kaynak(f, satir, "schema")]);
    }
    v
}

fn kaynak(dosya: &str, satir: usize, tur: &str) -> Value {
    json!({ "file": dosya, "line_start": satir, "kind": tur })
}

pub fn kart(uygulama_adi: &str, uygulama_surumu: &str, kaynak_ref: &str, adaptor: &str, kapilar: &[Kapi], uretim_zamani: &str) -> Value {
    let mut islemler = Vec::new();
    let mut kimlikler = std::collections::HashSet::new();
    // Kural sayaçları: yalnızca `rules` içindeki kurallar sayılır (işlemler değil)
    let (mut kesin, mut cikarim) = (0usize, 0usize);
    let mut tam = 0usize;
    // Kart ana uygulamanın benliğidir; aynı depodaki ikincil uygulamalar (ör. bakım sunucusu) dahil edilmez.
    for k in kapilar.iter().filter(|k| k.ana) {
        let mut girdiler = Map::new();
        let mut kurallar = Vec::new();
        let mut eksik = false;
        // Zorunluluğu koddan kesin çıkarılamayan alan: işlem en fazla "inferred" olur
        let mut cozulmeyen_adlar: Vec<String> = vec![];
        for p in &k.parametreler {
            if let Some(ad) = &p.ad {
                girdiler.insert(ad.clone(), json!({ "type": "string", "required": true, "description": "path parameter", "stamp": "certain" }));
            }
        }
        // Bölümün adı yalnızca adsız ve şemasız girdiyi (ör. `@Body() body: any`) karta yazmak için
        for (bolum, g) in k.govde.iter().map(|g| ("_body", g)).chain(k.sorgu.iter().map(|g| ("_query", g))) {
            match &g.sema {
                // Dizi gövde (öğeler şemaya uyar) kartın düz girdi listesiyle ifade edilemez: çözülemeyen gibi işlenir
                Some(s) if !s.bulunamadi && !s.dizi => {
                    let dosya = s.dosya.clone().unwrap_or_default();
                    for (ad, a) in &s.alanlar {
                        let damga = if a.belirsiz { "inferred" } else { "certain" };
                        let mut alan = alan_ozellikleri(a);
                        alan["required"] = json!(a.zorunlu);
                        alan["stamp"] = json!(damga);
                        alan["sources"] = json!([kaynak(&dosya, a.satir, "schema")]);
                        if a.belirsiz {
                            alan["description"] = json!("required flag could not be derived from validators; value is a guess");
                        }
                        // Aynı ad iki girdide (ör. iki gövde şeması) geçiyorsa zayıf damga korunur: belirsiz bir olgu
                        // sonradan gelen kesin bir olguyla örtülmez
                        if let Some(eski) = girdiler.get(ad) {
                            if eski["stamp"] != "certain" {
                                alan["stamp"] = eski["stamp"].clone();
                                if let Some(a) = eski.get("description") {
                                    alan["description"] = a.clone();
                                }
                            }
                        }
                        girdiler.insert(ad.clone(), alan);
                        // of(...) ve nullable() tip bilgisidir, reddeden kural değil
                        // Yalnızca isteği reddedebilen kurallar: IsOptional, of(...), nullable() gibi işaretler kural değildir
                        for r in a.kurallar.iter().filter(|r| !reddetmez(r)) {
                            // Hangi durum koduyla reddedildiği uygulamanın doğrulama ayarına bağlıdır: uydurulmaz
                            kurallar.push(json!({ "text": format!("{ad}: {r}"), "stamp": "certain", "sources": [kaynak(&dosya, a.satir, "schema")] }));
                            kesin += 1;
                        }
                    }
                }
                // Çözülemeyen şema: listelediği adlar (varsa) kesin sayılamaz; aynı ad başka bir şemada kesin görünse bile
                Some(s) => {
                    eksik = true;
                    cozulmeyen_adlar.extend(s.alanlar.keys().cloned());
                }
                // Şeması olmayan girdi: adı ya koddan ya bölümden, tipi ve zorunluluğu bilinmiyor.
                // Yazma bu girdiye dayanacağı için işlemin damgası da unknown olur (SPEC 3).
                None => {
                    let ad = g.ad.clone().unwrap_or_else(|| bolum.to_string());
                    let mut alan = json!({ "type": "unknown", "required": false, "stamp": "unknown" });
                    if g.ad.is_none() {
                        alan["description"] =
                            json!(format!("whole {} without a schema ({})", if bolum == "_body" { "body" } else { "query" }, g.tip.clone().unwrap_or_else(|| "no type".into())));
                    }
                    girdiler.insert(ad, alan);
                    eksik = true;
                }
            }
        }
        for ad in &cozulmeyen_adlar {
            if let Some(g) = girdiler.get_mut(ad.as_str()).filter(|g| g["stamp"] == "certain") {
                g["stamp"] = json!("inferred");
                g["description"] = json!("the same name appears in a schema that could not be resolved");
            }
        }
        if !eksik {
            tam += 1;
        }
        // İşlemin damgası ve onay gereği haritadaki kapıdan, canlı modun kullandığı aynı fonksiyonlarla: kart ile yürütme
        // hiçbir zaman ayrışmaz
        let kv = serde_json::to_value(k).unwrap_or_default();
        let damga = islem_damgasi(&kv);
        let mut islem = json!({
            "id": tekil(&mut kimlikler, islem_kimligi(k), &k.yol),
            "name": { "text": k.ozet.clone().unwrap_or_else(|| k.fonksiyon.clone()), "stamp": "certain", "sources": [kaynak(&k.dosya, k.satir, "code")] },
            "effect": etki(&k.yontem),
            // Etki yalnızca HTTP yönteminden çıkarıldı (POST /search okuyabilir, GET değiştirebilir)
            "effect_stamp": "inferred",
            "entry": { "protocol": "http", "method": k.yontem, "path": k.yol },
            "inputs": girdiler,
            "rules": kurallar,
            "roles": k.roller,
            // Canlı modun onay kuralının statik kısmı (crate::canli::statik_onay): yıkıcı ya da kanıtı kesin olmayan yazma
            // insan onayı ister. Çalışma anında yalnızca sıkılaşır (önizleme bir kuralı denetleyemezse, token "hepsi" derse)
            "requires_approval": crate::canli::statik_onay(&kv).is_some(),
            "stamp": damga,
            "sources": [kaynak(&k.dosya, k.satir, "code")],
        });
        if k.kullanimdan_kalkmis {
            islem["deprecated"] = json!(true);
            if let Some(y) = &k.yerine {
                islem["replaced_by"] = json!(y);
            }
        }
        if let Some(v) = &k.veri {
            // Bağlar ad ve alan tipi izlenerek çıkarılır (tip denetleyicisi değil): damga "inferred"
            islem["data"] = json!({ "reads": v.okur, "writes": v.yazar, "stamp": "inferred" });
            if v.eksik_olabilir {
                islem["data"]["partial"] = json!(true);
            }
        }
        if let Some(s) = &k.surum {
            islem["version"] = json!(s);
        }
        // Giriş: açık işaretten geliyorsa kesin; hiçbir guard bulunamadığı için açık sayıldıysa çıkarım
        if k.giris == "herkese_acik" {
            if k.giris_kaniti == "kesin" {
                islem["rules"].as_array_mut().unwrap().push(json!({ "text": "No login required.", "stamp": "certain" }));
                kesin += 1;
            } else {
                let metin = if k.giris_kaniti == "varsayim_isaret" {
                    "A public marker is present, but it could not be verified that the login guard honours it; treated as public."
                } else {
                    "No login guard found in code; treated as public."
                };
                islem["rules"].as_array_mut().unwrap().push(json!({ "text": metin, "stamp": "inferred" }));
                cikarim += 1;
            }
        }
        if let Some(y) = &k.yanit {
            islem["x-response"] = yanit_karti(y);
        }
        islem["x-dna"] = dna(k, &islem);
        islemler.push(islem);
    }
    json!({
        "benlik": SURUM,
        "app": { "id": uygulama_kimligi(uygulama_adi), "name": uygulama_adi, "version": uygulama_surumu, "source_ref": kaynak_ref },
        "generator": { "name": "sezgi", "version": env!("CARGO_PKG_VERSION"), "adapters": [adaptor], "generated_at": uretim_zamani },
        "operations": islemler,
        "coverage": {
            "operations_found": kapilar.iter().filter(|k| k.ana).count(),
            "operations_complete": tam,
            "rules_by_stamp": { "certain": kesin, "data": 0, "approved": 0, "inferred": cikarim, "unknown": 0 }
        },
        // Sezgi yetki vermez: yazma (canlı mod) insanın uygulamadaki kendi oturumuyla yapılır
        "identity": { "model": "user-session", "write_requires_human": true, "service_account_writes": false },
        "data_ledger": [
            { "class": "code_derived", "stored": true, "contents": "map, cards, stamps", "retention_days": null },
            { "class": "operational_log", "stored": false, "retention_days": 0 },
            { "class": "customer_data", "stored": false, "retention_days": 0 }
        ],
        "signature": { "alg": "none", "key_id": "unsigned-dev", "public_key_url": "/.well-known/benlik-keys.json", "value": "UNSIGNED-DEV-BUILD" }
    })
}

/// 0.1 görünümünün damgası: `observed` (gözlem) 0.1 ölçeğinde yoktur ve yazmaya hiçbir zaman yetmez, `inferred` yazılır
pub fn v1_damgasi(damga: &str) -> &str {
    if damga == "observed" {
        "inferred"
    } else {
        damga
    }
}

/// İşlemin davranış özeti (`x-dna`). Şimdilik kartın kendi olgularından türetilir; kapsamı açık kalan bölümler
/// `coverage` içinde "open" yazılır (yokluk kanıtı değildir). `evidence_refs` kanıt kayıtları üretilince doldurulur.
fn dna(k: &Kapi, islem: &Value) -> Value {
    let gerekli = match k.giris.as_str() {
        "gerekli" => "yes",
        "herkese_acik" => "no",
        "istege_bagli" => "optional",
        _ => "unknown",
    };
    let giris_damgasi = if k.giris_kaniti == "kesin" { "certain" } else { "inferred" };
    // Hata koşulları: isteği reddeden kurallar (giriş açıklığı notları kural değildir)
    let hatalar: Vec<Value> = islem["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r.get("sources").is_some())
        .map(|r| json!({ "condition": r["text"], "result": "rejected", "stamp": v1_damgasi(r["stamp"].as_str().unwrap_or("unknown")) }))
        .collect();
    let mut d = json!({
        "auth": { "required": gerekli, "stamp": giris_damgasi },
        "preconditions": [],
        "failure_modes": hatalar,
        // Geri alınabilirlik koddan bilinmiyor: uydurulmaz
        "reversibility": { "status": "unknown" },
        "risk": { "class": islem["effect"], "basis": "method" },
        "requires_approval": islem["requires_approval"],
        "coverage": { "auth": if giris_damgasi == "certain" { "closed" } else { "open" }, "preconditions": "open", "failure_modes": "open" },
        "evidence_refs": [],
    });
    if let Some(v) = &k.veri {
        d["data_effect"] = json!({ "reads": v.okur, "writes": v.yazar, "stamp": "inferred" });
    }
    d
}

/// "dosya:satır" kanıtından kartın kaynak listesi
fn kanit_kaynagi(kanit: Option<&str>, tur: &str) -> Value {
    match kanit.and_then(|k| k.rsplit_once(':')).and_then(|(d, s)| Some((d, s.parse::<usize>().ok()?))) {
        Some((dosya, satir)) => json!([kaynak(dosya, satir, tur)]),
        None => json!([]),
    }
}

/// Dahili olay ve işler kartın kökünde: `x-nodes` (tür, kimlik, dinleyicinin kaynağı; işlem değildir) ve
/// `x-relationships` (işlem ya da düğüm -> olay / iş `triggers`; işin sildiği tablo `writes`, kaskadla dolaylı silinen
/// `may_affect`). Hepsi statik çıkarım: `inferred`
pub fn olaylari_ekle(kart: &mut Value, dna: &Value, olaylar: &Value) {
    let mut iliskiler: Vec<Value> = vec![];
    for op in kart["operations"].as_array().into_iter().flatten() {
        let yer = format!("{}:{}", op["sources"][0]["file"].as_str().unwrap_or(""), op["sources"][0]["line_start"]);
        for t in dna[yer.as_str()]["tetikler"].as_array().into_iter().flatten() {
            let hedef = format!("{}:{}", t["tur"].as_str().unwrap_or(""), t["ad"].as_str().unwrap_or(""));
            iliskiler.push(json!({ "from": op["id"], "to": hedef, "kind": "triggers", "stamp": "inferred", "sources": kanit_kaynagi(t["kanit"].as_str(), "code") }));
        }
    }
    let mut dugumler: Vec<Value> = vec![];
    for (id, o) in olaylar.as_object().into_iter().flatten() {
        let kaynaklar: Vec<Value> =
            o["dinleyiciler"].as_array().into_iter().flatten().flat_map(|d| kanit_kaynagi(d["kanit"].as_str(), "code").as_array().cloned().unwrap_or_default()).collect();
        let isleyiciler: Vec<&Value> = o["dinleyiciler"].as_array().into_iter().flatten().map(|d| &d["fonksiyon"]).collect();
        dugumler.push(json!({ "type": o["tur"], "id": id, "name": o["ad"], "handlers": isleyiciler, "stamp": "inferred", "sources": kaynaklar }));
        for t in o["tetikler"].as_array().into_iter().flatten() {
            let hedef = format!("{}:{}", t["tur"].as_str().unwrap_or(""), t["ad"].as_str().unwrap_or(""));
            iliskiler.push(json!({ "from": id, "to": hedef, "kind": "triggers", "stamp": "inferred", "sources": kanit_kaynagi(t["kanit"].as_str(), "code") }));
        }
        for s in o["silinen"].as_array().into_iter().flatten().filter_map(|s| s.as_str()) {
            iliskiler.push(json!({ "from": id, "to": format!("db:{s}"), "kind": "writes", "stamp": "inferred" }));
        }
        for x in o["dolayli_yazar"].as_array().into_iter().flatten() {
            let hedef = format!("db:{}", x["tablo"].as_str().unwrap_or(""));
            iliskiler.push(json!({ "from": id, "to": hedef, "kind": "may_affect", "stamp": "inferred", "sources": kanit_kaynagi(x["kanit"].as_str(), "schema") }));
        }
    }
    if !dugumler.is_empty() {
        kart["x-nodes"] = json!(dugumler);
    }
    if !iliskiler.is_empty() {
        kart["x-relationships"] = json!(iliskiler);
    }
}

/// Haritadaki davranış özetini (`ozet.dna`, "dosya:satır" anahtarlı) kartın `x-dna` alanlarına taşır:
/// - `failure_modes`: koşullu fırlatmalar (koşul kodun kendisidir, değer içermez; sonuç HTTP kodu ve istisna),
/// - `data_effect.indirect`: kaskadla dolaylı silinen tablolar (en fazla `inferred`),
/// - `branches`: davranışı ikiye bölen koşullar (`force ? Deleted : Trashed`),
/// - `reversibility`: `reversible` | `partial` | `irreversible` (gerekçesiyle, `inferred`) ya da `unknown`,
/// - `side_effects`: giden çağrılar (dış sistem düğümü, tür, yöntem, damga), `transaction`: işlem sınırı,
/// - `requires_approval`: dış sisteme yazan ya da geri alınamaz yazmada `true` (canlı modla aynı gerekçe).
///
/// Bu bölümlerin kapsamı her zaman `open`'dır: bulunamayan red koşulu "yok" demek değildir.
pub fn dna_ekle(kart: &mut Value, dna: &Value) {
    for op in kart["operations"].as_array_mut().into_iter().flatten() {
        let yer = format!("{}:{}", op["sources"][0]["file"].as_str().unwrap_or(""), op["sources"][0]["line_start"]);
        let Some(v) = dna.get(&yer) else { continue };
        // Okuma işlemi aynı görünümü paylaşan yazmanın onay gerekçesini taşımaz (canlı modda okuma onay istemez)
        let okuma = matches!(op["entry"]["method"].as_str(), Some("GET" | "HEAD" | "OPTIONS"));
        let Some(d) = op.get_mut("x-dna") else { continue };
        let mut hatalar: Vec<Value> = v["hatalar"]
            .as_array()
            .into_iter()
            .flatten()
            .take(12)
            .map(|h| {
                let sonuc = match h["durum"].as_u64() {
                    Some(c) => format!("HTTP {c} ({})", h["istisna"].as_str().unwrap_or("")),
                    None => format!("error ({})", h["istisna"].as_str().unwrap_or("")),
                };
                json!({ "condition": h["kosul"].as_str().unwrap_or("unconditional"), "result": sonuc, "stamp": h["guven"].as_str().unwrap_or("inferred") })
            })
            .collect();
        if let Some(l) = d["failure_modes"].as_array_mut() {
            l.append(&mut hatalar);
        }
        // Dolaylı yazma şemadan çıkarılır: damgası hiçbir zaman certain değildir (şema da reddeder)
        let dolayli: Vec<Value> = v["dolayli_yazar"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| Some(json!({ "table": x["tablo"].as_str()?, "reason": x["neden"], "stamp": "inferred", "sources": kanit_kaynagi(x["kanit"].as_str(), "schema") })))
            .collect();
        if !dolayli.is_empty() {
            if !d["data_effect"].is_object() {
                d["data_effect"] = json!({ "reads": [], "writes": [], "stamp": "inferred" });
            }
            d["data_effect"]["indirect"] = json!(dolayli);
            d["coverage"]["data_effect"] = json!("open");
        }
        let dallar: Vec<Value> = v["dallar"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| {
                let ek = x["baglam"].as_str().filter(|b| !b.is_empty()).map(|b| format!("{b}: ")).unwrap_or_default();
                json!({
                    "condition": x["kosul"],
                    "when_true": format!("{ek}{}", x["evet"].as_str().unwrap_or("")),
                    "when_false": format!("{ek}{}", x["hayir"].as_str().unwrap_or("")),
                    "stamp": x["guven"].as_str().unwrap_or("inferred"),
                    "sources": kanit_kaynagi(x["kanit"].as_str(), "code"),
                })
            })
            .collect();
        if !dallar.is_empty() {
            d["branches"] = json!(dallar);
        }
        if let Some(g) = v["geri_alinabilir"].as_str() {
            let durum = match g {
                "evet" => "reversible",
                "kismen" => "partial",
                "hayir" => "irreversible",
                _ => "unknown",
            };
            d["reversibility"] = json!({ "status": durum, "stamp": if durum == "unknown" { "unknown" } else { "inferred" } });
            if let Some(n) = v["geri_alma_gerekce"].as_str().filter(|n| !n.is_empty()) {
                d["reversibility"]["reason"] = json!(n);
            }
            d["coverage"]["reversibility"] = json!("open");
        }
        // Nesne düzeyi yetki ve çağrı zinciri (çağrı grafiğinden): servis katmanındaki erişim denetimleri, sahiplik
        // karşılaştırmaları ve sorgu süzgeçleri
        let mut nesne: Vec<Value> = v["erisim_denetimi"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| {
                let mut o = json!({
                    "kind": if x["nesne_duzeyi"] == true { "access_check" } else { "authorization" },
                    "expression": x["ifade"].as_str().unwrap_or(""),
                    "stamp": x["guven"].as_str().unwrap_or("inferred"),
                    "sources": kanit_kaynagi(x["kanit"].as_str(), "code"),
                });
                if let Some(i) = x["izin"].as_str() {
                    o["permission"] = json!(i);
                }
                o
            })
            .collect();
        nesne.extend(v["sahiplik_denetimi"].as_array().into_iter().flatten().filter_map(|x| {
            Some(json!({
                "kind": if x["tur"] == "filtre" { "ownership_filter" } else { "ownership" },
                "expression": x["kosul"].as_str()?,
                "stamp": x["guven"].as_str().unwrap_or("inferred"),
                "sources": kanit_kaynagi(x["kanit"].as_str(), "code"),
            }))
        }));
        if !nesne.is_empty() && d["auth"].is_object() {
            nesne.truncate(12);
            d["auth"]["object_level"] = json!(nesne);
        }
        let c = &v["cagri"];
        if let Some(z) = c["zincir"].as_array().filter(|z| !z.is_empty()) {
            d["call_chain"] = json!({
                "functions": z,
                "reached": c["erisilen"].as_u64().unwrap_or(0),
                "unresolved_calls": c["dinamik"].as_u64().unwrap_or(0),
                "stamp": c["zincir_guveni"].as_str().unwrap_or("inferred"),
            });
        }
        d["coverage"]["failure_modes"] = json!("open");
        // Dış etkiler (DNA alan 6) ve işlem sınırı (dis.rs)
        let yan: Vec<Value> = v["dis"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| {
                json!({
                    "target": x["dugum"], "kind": crate::dis::tur_en(x["tur"].as_str().unwrap_or("")), "method": x["yontem"], "client": x["istemci"],
                    "writes": x["yazma"], "timing": if x["zaman"] == "ertelenmis" { "deferred" } else { "sync" },
                    "stamp": x["guven"], "sources": kanit_kaynagi(x["kanit"].as_str(), "code"),
                })
            })
            .collect();
        if !yan.is_empty() {
            d["side_effects"] = json!(yan);
            d["coverage"]["side_effects"] = json!("open");
        }
        if let Some(s) = v["islem_siniri"].as_object() {
            d["transaction"] = json!({ "kind": s["ifade"], "stamp": "inferred", "sources": kanit_kaynagi(s["kanit"].as_str(), "code") });
        }
        // Dış sisteme yazan ya da geri alınamaz yazma: canlı modun onay kuralıyla aynı gerekçe (`etki_onayi`)
        if let Some(o) = v["onay_gerekcesi"].as_str().filter(|_| !okuma) {
            d["requires_approval"] = json!(true);
            d["approval_reason"] = json!(o);
            op["requires_approval"] = json!(true);
        }
    }
}

/// Okunamayan rota kaynakları kartın kökünde: `x-unread_route_sources` (`count`, `sources`: "dosya:satır neden", en çok
/// 50; `coverage: "partial"`). Kod bu yerlerde rota kaydediyor ama Sezgi okuyamadı: `operations` eksiktir, listede
/// olmayan işlem "yok" demek değildir
pub fn okunamayan_rotalari_ekle(kart: &mut Value, liste: &[String]) {
    if liste.is_empty() {
        return;
    }
    kart["x-unread_route_sources"] = json!({ "count": liste.len(), "sources": liste.iter().take(50).collect::<Vec<_>>(), "coverage": "partial" });
}

/// Dış sistemler kartın kökünde: `x-external` (düğüm, tür, hedef ve türü, damga, çağrı sayısı) ve işlemden dış sisteme
/// `calls` ilişkileri (`x-relationships`'e eklenir). Hedefi ortam değişkeninden gelen sistemin adresi bilinmez.
pub fn dis_ekle(kart: &mut Value, dna: &Value, dis: &Value) {
    let mut iliskiler: Vec<Value> = vec![];
    for op in kart["operations"].as_array().into_iter().flatten() {
        let yer = format!("{}:{}", op["sources"][0]["file"].as_str().unwrap_or(""), op["sources"][0]["line_start"]);
        for x in dna[yer.as_str()]["dis"].as_array().into_iter().flatten() {
            iliskiler.push(json!({ "from": op["id"], "to": x["dugum"], "kind": "calls", "stamp": x["guven"], "sources": kanit_kaynagi(x["kanit"].as_str(), "code") }));
        }
    }
    let sistemler: Vec<Value> = dis["sistemler"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            json!({ "id": s["dugum"], "kind": crate::dis::tur_en(s["tur"].as_str().unwrap_or("")), "target": s["hedef"], "target_kind": s["hedef_turu"], "calls": s["cagri"], "stamp": s["guven"] })
        })
        .collect();
    if !sistemler.is_empty() {
        kart["x-external"] = json!(sistemler);
    }
    if !iliskiler.is_empty() {
        match kart["x-relationships"].as_array_mut() {
            Some(l) => l.extend(iliskiler),
            None => kart["x-relationships"] = json!(iliskiler),
        }
    }
}

/// Baytların SHA-256 özeti (küçük harf onaltılık)
pub fn sha256_hex(b: &[u8]) -> String {
    onaltilik(ring::digest::digest(&ring::digest::SHA256, b).as_ref())
}

/// Kartın haritaya bağı: haritanın kanonik (JCS) SHA-256 özeti. Biçimden (girinti, satır sonu) bağımsızdır;
/// paylaşım dosyasından yeniden yazılan harita da aynı özeti verir.
pub const HARITA_OZETI: &str = "x-map_digest";

pub fn harita_ozeti(harita: &Value) -> String {
    format!("sha256:{}", sha256_hex(crate::imza::kanonik(harita).as_bytes()))
}

fn onaltilik(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Bir kaydın özeti: RFC 8785 (JCS) kanonik biçiminin SHA-256'sı
pub fn kayit_ozeti(v: &Value) -> String {
    sha256_hex(crate::imza::kanonik(v).as_bytes())
}

/// RFC 6962 Merkle ağaç özeti: yaprak = SHA256(0x00 ‖ özet), düğüm = SHA256(0x01 ‖ sol ‖ sağ); n öğe, n'den küçük
/// en büyük ikinin kuvvetinden bölünür; boş liste SHA256("")
pub fn merkle_koku(ozetler: &[String]) -> String {
    fn ozet(b: &[u8]) -> Vec<u8> {
        ring::digest::digest(&ring::digest::SHA256, b).as_ref().to_vec()
    }
    fn mth(o: &[Vec<u8>]) -> Vec<u8> {
        match o.len() {
            0 => ozet(b""),
            1 => ozet(&[&[0u8][..], &o[0]].concat()),
            n => {
                let mut k = 1;
                while k * 2 < n {
                    k *= 2;
                }
                ozet(&[&[1u8][..], &mth(&o[..k]), &mth(&o[k..])].concat())
            }
        }
    }
    let baytlar: Vec<Vec<u8>> = ozetler.iter().map(|h| (0..h.len() / 2).filter_map(|i| h.get(2 * i..2 * i + 2).and_then(|p| u8::from_str_radix(p, 16).ok())).collect()).collect();
    onaltilik(&mth(&baytlar))
}

/// Unix saniyesinden RFC 3339 UTC zaman (takvim hesabı, bağımlılıksız)
pub fn zaman_metni(s: i64) -> String {
    let (gun, sn) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    let z = gun + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let g = doy - (153 * mp + 2) / 5 + 1;
    let a = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if a <= 2 { 1 } else { 0 };
    format!("{y:04}-{a:02}-{g:02}T{:02}:{:02}:{:02}Z", sn / 3600, sn % 3600 / 60, sn % 60)
}

/// RFC 3339 zamanı Unix saniyesine çevirir ("2026-01-02T03:04:05Z"; kesirli saniye ve +hh:mm ofset kabul edilir)
pub fn zaman_sn(m: &str) -> Option<i64> {
    let b = m.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let sayi = |a: usize, z: usize| m.get(a..z).filter(|p| p.bytes().all(|c| c.is_ascii_digit())).and_then(|p| p.parse::<i64>().ok());
    let (y, ay, g, sa, dk, sn) = (sayi(0, 4)?, sayi(5, 7)?, sayi(8, 10)?, sayi(11, 13)?, sayi(14, 16)?, sayi(17, 19)?);
    if !(1..=12).contains(&ay) || !(1..=31).contains(&g) || sa > 23 || dk > 59 || sn > 60 {
        return None;
    }
    let mut i = 19;
    if b[i] == b'.' {
        i += 1;
        let bas = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == bas {
            return None;
        }
    }
    let ofset = match m.get(i..)? {
        "Z" | "z" => 0,
        o if o.len() == 6 && (o.starts_with('+') || o.starts_with('-')) && &o[3..4] == ":" => {
            let d = sayi(i + 1, i + 3)? * 3600 + sayi(i + 4, i + 6)? * 60;
            if o.starts_with('+') {
                d
            } else {
                -d
            }
        }
        _ => return None,
    };
    // Gün sayısı (days_from_civil): zaman_metni'nin tersi
    let yy = if ay <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * (if ay > 2 { ay - 3 } else { ay + 9 }) + 2) / 5 + g - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let gun = era * 146_097 + doe - 719_468;
    Some(gun * 86_400 + sa * 3600 + dk * 60 + sn - ofset)
}

/// İşlemin dayandığı kaynak dosyalar: işlemin, adının, girdilerinin ve kurallarının kaynakları
pub fn islem_dayanaklari(op: &Value) -> BTreeSet<String> {
    let mut kaynaklar: Vec<&Value> = vec![&op["sources"], &op["name"]["sources"]];
    kaynaklar.extend(op["inputs"].as_object().into_iter().flatten().map(|(_, g)| &g["sources"]));
    kaynaklar.extend(op["rules"].as_array().into_iter().flatten().map(|r| &r["sources"]));
    kaynaklar.push(&op["x-response"]["sources"]);
    kaynaklar.into_iter().filter_map(Value::as_array).flatten().filter_map(|s| s["file"].as_str()).filter(|f| !f.is_empty()).map(String::from).collect()
}

/// Proje kökü altındaki bir dosyanın güncel özeti; kök dışına çıkan, bağlantı olan ya da okunamayan dosya için None
fn dosya_ozeti(kok: &Path, goreli: &str) -> Option<String> {
    let p = kok.join(goreli);
    if !crate::calisma_alani::icinde(kok, &p) {
        return None;
    }
    crate::guvenli::oku_bayt(&p).map(|b| sha256_hex(&b))
}

/// Bayat işlemler: dayandığı dosyalardan biri `x-basis`'teki özetten farklıysa, silindiyse ya da `x-basis`'te yoksa.
/// Kartta `x-basis` yoksa (0.1 kartı) tazelik hesaplanamaz; yalnızca açık `stale: true` bayrakları sayılır.
pub fn bayat_islemler(kart: &Value, kok: &Path) -> Vec<String> {
    let dayanak = kart.get("x-basis").and_then(Value::as_object);
    let mut guncel: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut bayat = Vec::new();
    for op in kart["operations"].as_array().into_iter().flatten() {
        let hesaplanan = dayanak.is_some_and(|d| {
            islem_dayanaklari(op).iter().any(|f| {
                let g = guncel.entry(f.clone()).or_insert_with(|| dosya_ozeti(kok, f));
                g.is_none() || d.get(f).and_then(Value::as_str) != g.as_deref()
            })
        });
        if op["stale"] == true || hesaplanan {
            bayat.push(op["id"].as_str().unwrap_or("").to_string());
        }
    }
    bayat
}

/// Kanıt kaydının iddia türü (`claim_kind`): kanıtın kanıt deposundaki türünden
fn iddia_turu(k: &crate::kanit::Kanit) -> &'static str {
    match k.ozet["tur"].as_str() {
        Some("giris_karari" | "rol_karari") => "permission",
        Some("veri_bagi") => "data_effect",
        Some("web_bagi") => "call_target",
        Some("ag_gozlemi" | "yolculuk_gozlemi" | "zaman_eslemesi") => "behavior",
        _ => "structure",
    }
}

/// Kanıt kaydının yetkisi (`authority`): kaynak kod, addan çıkarım ya da çalışma zamanı kaydı (HAR geçmiş bir kayıt,
/// canlı mod Sezgi'nin kendi isteği)
fn yetki(k: &crate::kanit::Kanit) -> &'static str {
    match (k.kaynak_turu, k.ozet["tur"].as_str(), k.ozet["kaynak"].as_str()) {
        (_, Some("adlandirma"), _) => "analyst-inference",
        (crate::kanit::KaynakTuru::NetworkCapture, _, Some("canli")) => "controlled-replay",
        (crate::kanit::KaynakTuru::NetworkCapture, _, _) => "historical-reference",
        _ => "source",
    }
}

/// Kartın `x-journeys` üyesi: kartın işlemlerine giden kullanıcı yolculukları (kanıt deposundaki `journey:` önermeleri).
/// Adımın damgası STEP önermesinin güvenidir (kaynaktan en fazla inferred); yolculuğun damgası, istek sırası
/// gözlendiyse observed
fn yolculuklar(depo: &crate::kanit::Depo, apiler: &[String], islemler: &[Value]) -> Vec<Value> {
    let islem: BTreeMap<&str, &str> = apiler.iter().zip(islemler).map(|(a, op)| (a.as_str(), op["id"].as_str().unwrap_or(""))).collect();
    let mut adimlar: BTreeMap<&str, Vec<(u64, &str, &'static str)>> = BTreeMap::new();
    for o in depo.onermeler.iter().filter(|o| o.yuklem == "STEP" && o.varlik.starts_with("journey:")) {
        if let Some(h) = &o.hedef {
            adimlar.entry(o.varlik.as_str()).or_default().push((o.deger["sira"].as_u64().unwrap_or(0), h.as_str(), o.hesap.guven.ad()));
        }
    }
    let mut v = vec![];
    for o in depo.onermeler.iter().filter(|o| o.yuklem == "VAR" && o.varlik.starts_with("journey:")) {
        let Some(op) = o.deger["hedef"].as_str().and_then(|h| islem.get(h)) else { continue };
        let mut a = adimlar.get(o.varlik.as_str()).cloned().unwrap_or_default();
        if a.is_empty() {
            continue;
        }
        a.sort();
        let steps: Vec<Value> = a.iter().enumerate().map(|(i, (_, n, g))| json!({ "order": i + 1, "node": n, "kind": crate::graf::tur(n), "stamp": g })).collect();
        v.push(json!({ "id": o.varlik, "name": o.deger["ad"], "operation": op, "stamp": o.hesap.guven.ad(), "steps": steps }));
    }
    v
}

/// Kartı 0.1.1 alanlarıyla tamamlar ve ayrı kanıt paketini döndürür: geçerlilik süresi, dayanak özetleri, kanıt
/// kayıtlarının özetleri ve Merkle kökü, işlemlerin `x-dna.evidence_refs` bağları ve işlem kümesinin özeti (`x-revision`).
/// Kanıt kayıtları ayrı bir kaynak değildir: kanıt deposunun (`kanit.json`) işlemlere dayanak olan kanıtlarının
/// paylaşılabilir, değer taşımayan projeksiyonudur (kimlik, tür, dosya, satır, dosya özeti, güven). `x-basis` aynı
/// kanıtların dayanak özetlerinden gelir. Kayıtlar yalnızca proje köküne göreli yollar taşır; paket kartla aynı
/// düzenle, kendi bağlamıyla imzalanabilir. `kapilar`: kartın üretildiği kapılar (işlemler ana kapılarla aynı sırada).
pub fn tamamla(kart: &mut Value, kapilar: &[Kapi], depo: &crate::kanit::Depo, kok: &Path, verilis_sn: i64, gun: u64) -> Value {
    kart["x-issued_at"] = json!(zaman_metni(verilis_sn));
    kart["x-expires_at"] = json!(zaman_metni(verilis_sn + gun as i64 * 86_400));
    let kanitlar: BTreeMap<&str, &crate::kanit::Kanit> = depo.kanitlar.iter().map(|k| (k.id.as_str(), k)).collect();
    let mut varliklar: BTreeMap<&str, Vec<&crate::kanit::Onerme>> = BTreeMap::new();
    for o in &depo.onermeler {
        varliklar.entry(o.varlik.as_str()).or_default().push(o);
    }
    let apiler: Vec<String> = kapilar.iter().filter(|k| k.ana).map(|k| crate::graf::api_kimligi(&serde_json::to_value(k).unwrap_or_default())).collect();
    let mut dayanak = Map::new();
    let mut kayitlar: Vec<Value> = Vec::new();
    let mut yazilan: BTreeSet<String> = BTreeSet::new();
    for (i, op) in kart["operations"].as_array_mut().into_iter().flatten().enumerate() {
        let onermeler = apiler.get(i).and_then(|a| varliklar.get(a.as_str())).cloned().unwrap_or_default();
        let mut refler: Vec<String> = vec![];
        for o in &onermeler {
            for (kid, celiski) in o.hesap.destek.iter().map(|x| (x, false)).chain(o.hesap.celiski.iter().map(|x| (x, true))) {
                let Some(k) = kanitlar.get(kid.as_str()) else { continue };
                if !refler.contains(kid) {
                    refler.push(kid.clone());
                }
                for d in &k.dayanak {
                    dayanak.entry(d.yol.clone()).or_insert_with(|| json!(d.sha256));
                }
                if !yazilan.insert(kid.clone()) {
                    continue;
                }
                let ilk = k.dayanak.first();
                let satir = k.konum.first().and_then(|x| x.rsplit_once(':')).and_then(|(_, s)| s.parse::<u64>().ok());
                kayitlar.push(json!({
                    "id": k.id,
                    "kind": k.kaynak_turu.ad(),
                    "claim_kind": iddia_turu(k),
                    "subject": { "operation": op["id"], "file": ilk.map(|d| d.yol.clone()), "sha256": ilk.map(|d| d.sha256.clone()) },
                    "locator": { "line_start": satir },
                    "provider": { "name": "sezgi", "version": env!("CARGO_PKG_VERSION") },
                    "confidence": k.katki().ad(),
                    "authority": yetki(k),
                    "shareable": true,
                    "contradicts": if celiski { json!(o.hesap.destek) } else { json!([]) },
                }));
            }
        }
        // Kanıt deposunda olmayan dayanak (ör. kural kaynağı başka dosyada): kartın tazeliği yine hesaplanabilsin
        for f in islem_dayanaklari(op) {
            if !dayanak.contains_key(&f) {
                if let Some(o) = dosya_ozeti(kok, &f) {
                    dayanak.insert(f, json!(o));
                }
            }
        }
        if let Some(d) = op.get_mut("x-dna") {
            d["evidence_refs"] = json!(refler);
        }
    }
    // Karttaki özet değer taşımayan gizlilik projeksiyonudur: yol, satır, içerik yok; yalnızca türler ve kaydın özeti
    let ozetler: Vec<Value> = kayitlar
        .iter()
        .map(|k| {
            json!({ "id": k["id"], "kind": k["kind"], "claim_kind": k["claim_kind"], "confidence": k["confidence"], "authority": k["authority"], "shareable": k["shareable"], "sha256": kayit_ozeti(k) })
        })
        .collect();
    let kok_ozeti = merkle_koku(&ozetler.iter().map(|o| o["sha256"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>());
    kart["x-basis"] = Value::Object(dayanak);
    kart["x-evidence"] = json!(ozetler);
    kart["x-evidence_root"] = json!(kok_ozeti);
    kart["x-evidence_bundle"] = json!(KANIT_PAKETI);
    let yol = yolculuklar(depo, &apiler, kart["operations"].as_array().map(Vec::as_slice).unwrap_or(&[]));
    if !yol.is_empty() {
        kart["x-journeys"] = json!(yol);
    }
    // İşlem kümesinin özeti; işlemler kanıt deposundaki kanıtlara bağlı (`evidence_refs`): kanıt değişirse revizyon da
    let revizyon = format!("sha256:{}", kayit_ozeti(&kart["operations"]));
    kart["x-revision"] = json!(revizyon);
    json!({ "benlik_evidence": SURUM, "app_id": kart["app"]["id"], "card_revision": revizyon, "records": kayitlar })
}

/// Kartın kanıt özetlerinden Merkle kökünü yeniden hesaplar ve karttakiyle karşılaştırır (kök yoksa None)
pub fn kok_tutarli(kart: &Value) -> Option<bool> {
    let kok = kart.get("x-evidence_root")?.as_str()?;
    let ozetler: Vec<String> = kart["x-evidence"].as_array().into_iter().flatten().map(|o| o["sha256"].as_str().unwrap_or("").to_string()).collect();
    Some(merkle_koku(&ozetler) == kok)
}

/// Kanıt paketini kartla karşılaştırır: her kaydın özeti kartta olmalı ve uyuşmalı; kartta olup pakette olmayanlar
/// "sunulmadı" sayılır (paket kısmi sunulabilir). Dönen: (geçerli kayıt sayısı, sunulmayan kimlikler, hatalar)
pub fn kanit_paketi_denetle(kart: &Value, paket: &Value) -> (usize, Vec<String>, Vec<String>) {
    let ozetler: BTreeMap<&str, &str> = kart["x-evidence"].as_array().into_iter().flatten().filter_map(|o| Some((o["id"].as_str()?, o["sha256"].as_str().unwrap_or("")))).collect();
    let mut hatalar = Vec::new();
    let mut gorulen = BTreeSet::new();
    if paket["card_revision"] != kart["x-revision"] {
        hatalar.push("paket başka bir kart sürümüne ait (card_revision uyuşmuyor)".to_string());
    }
    // Sunulan ama özeti tutmayan kayıt "sunulmadı" diye ikinci kez sayılmaz: yalnızca özet hatasıdır
    let mut gecerli = 0;
    for k in paket["records"].as_array().into_iter().flatten() {
        let id = k["id"].as_str().unwrap_or("");
        match ozetler.get(id) {
            None => hatalar.push(format!("{id}: kayıt kartta yok")),
            Some(_) if !gorulen.insert(id) => hatalar.push(format!("{id}: kayıt iki kez var")),
            Some(o) if *o != kayit_ozeti(k) => hatalar.push(format!("{id}: kayıt özeti uyuşmuyor")),
            Some(_) => gecerli += 1,
        }
    }
    let sunulmayan = ozetler.keys().filter(|i| !gorulen.contains(*i)).map(|i| i.to_string()).collect();
    (gecerli, sunulmayan, hatalar)
}
