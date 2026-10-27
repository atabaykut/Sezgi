//! Haritadan uygulamanın OpenAPI 3.1 belgesi (`.sezgi/openapi.json`): elle yazılmış dokümanı olmayan uygulamanın
//! güncel API dokümanı kendiliğinden oluşur; kod değişince `sezgi init` / `sezgi calistir` ile yenilenir.
//! Yalnızca koddan okunanlar yazılır: tip bilinmiyorsa şema boş bırakılır (her değer), çıkarım damgası
//! `x-sezgi-*` alanlarında görünür. Giriş gereken işlemler `bearerAuth` güvenlik şemasıyla işaretlenir; bu bir
//! varsayımdır (uygulama başka bir kimlik yöntemi kullanıyor olabilir) ve `x-sezgi-giris` alanında yazılıdır.
use serde_json::{json, Map, Value};

use crate::adaptor::{Alan, Girdi, Kapi};
use crate::benlik;

/// Kartın alan özelliklerinden JSON Schema
pub(crate) fn sema(a: &Alan) -> Value {
    let o = benlik::alan_ozellikleri(a);
    let mut s = Map::new();
    match o["type"].as_str().unwrap_or("unknown") {
        "datetime" => {
            s.insert("type".into(), json!("string"));
            s.insert("format".into(), json!("date-time"));
        }
        "enum" => match o.get("enum").and_then(|e| e.as_array()) {
            // Değerler metin (sayıya benzemeyen): tip de metindir. Sayısal enum'da tip yazılmaz (değerler metin olarak tutulur)
            Some(d) if d.iter().all(|x| x.as_str().is_some_and(|v| v.parse::<f64>().is_err())) => {
                s.insert("type".into(), json!("string"));
            }
            Some(_) => {}
            // Değerler çözülemedi: tip bilinmiyor
            None => {
                s.insert("description".into(), json!("allowed values could not be resolved from code"));
            }
        },
        "unknown" => {}
        t => {
            s.insert("type".into(), json!(t));
        }
    }
    let dizi = o["type"] == "array";
    for (k, v) in [("enum", "enum"), ("min", "minimum"), ("max", "maximum"), ("min_length", "minLength"), ("max_length", "maxLength"), ("format", "format")] {
        if let Some(x) = o.get(k) {
            // Dizide en az / en çok öğe sayısıdır (zod .min(1), class-validator ArrayMinSize)
            let v = match (dizi, v) {
                (true, "minimum" | "minLength") => "minItems",
                (true, "maximum" | "maxLength") => "maxItems",
                _ => v,
            };
            s.insert(v.into(), x.clone());
        }
    }
    if dizi {
        if let Some(o) = oge_semasi(a) {
            s.insert("items".into(), o);
        }
        // Öğe sayısı sınırları: zod .min(n) / .max(n) / .length(n), class-validator ArrayMinSize / ArrayMaxSize
        for r in a.kurallar.iter().filter(|r| !crate::kural::her_oge(r)) {
            let sayi = |on: &str| r.strip_prefix(on).and_then(|x| x.strip_suffix(')')).and_then(|x| x.trim().parse::<u64>().ok());
            if let Some(n) = sayi("min(").or_else(|| sayi("ArrayMinSize(")).or_else(|| sayi("nonempty(").map(|_| 1)) {
                s.insert("minItems".into(), json!(n));
            }
            if let Some(n) = sayi("max(").or_else(|| sayi("ArrayMaxSize(")) {
                s.insert("maxItems".into(), json!(n));
            }
            if let Some(n) = sayi("length(") {
                s.insert("minItems".into(), json!(n));
                s.insert("maxItems".into(), json!(n));
            }
        }
    }
    // ISO tarih biçimi kuraldan (zod .datetime() / .date(), z.iso.*): kartta tip metindir, belgede biçim yazılır
    if s.get("type") == Some(&json!("string")) && !s.contains_key("format") {
        let ham = a.tip.clone().unwrap_or_default();
        for k in a.kurallar.iter().flat_map(|r| crate::kural::coz(r, &ham)) {
            // Herhangi (IsDateString: tarih ya da tarih-saat) tek bir biçime indirgenmez
            match k {
                crate::kural::Kural::IsoTarih(crate::kural::IsoTur::YalnizTarih) => s.insert("format".into(), json!("date")),
                crate::kural::Kural::IsoTarih(crate::kural::IsoTur::TarihSaat { .. }) => s.insert("format".into(), json!("date-time")),
                _ => None,
            };
        }
    }
    // İç içe yapı (tanımdan çözüldüyse): nesnenin alanları ya da nesne dizisinin öğesi
    if let Some(ic) = &a.ic {
        let ozellik: Map<String, Value> = ic.iter().map(|(ad, alt)| (ad.clone(), sema(alt))).collect();
        let zorunlu: Vec<&String> = ic.iter().filter(|(_, alt)| alt.zorunlu && !alt.belirsiz).map(|(ad, _)| ad).collect();
        let mut nesne = json!({ "type": "object", "properties": ozellik });
        if !zorunlu.is_empty() {
            nesne["required"] = json!(zorunlu);
        }
        if dizi {
            s.insert("items".into(), nesne);
        } else if let Value::Object(n) = nesne {
            s.extend(n);
        }
    }
    if a.belirsiz {
        s.insert("x-sezgi-required".into(), json!("inferred"));
    }
    Value::Object(s)
}

/// Dizi öğesinin şeması, koddaki tanımdan: zod `z.array(z.uuidv4())` (kural `of(z.uuidv4())`), class-validator
/// `{ each: true }` kuralları ya da TypeScript `string[]`. Öğe başka bir şemaya atıfsa (`of(KisiSchema)`) ya da
/// tip belirsizse None: öğe tipi uydurulmaz.
fn oge_semasi(a: &Alan) -> Option<Value> {
    let tip_semasi = |tip: &str, kurallar: &[String]| -> Option<Value> {
        let t = crate::adaptor::alan_tipi(&Some(tip.to_string()), kurallar);
        let mut s = match t {
            "datetime" => json!({ "type": "string", "format": "date-time" }),
            "unknown" | "enum" => return None,
            t => json!({ "type": t }),
        };
        for k in crate::kural::tip_kurallari(tip) {
            match k {
                crate::kural::Kural::Uuid => s["format"] = json!("uuid"),
                crate::kural::Kural::Eposta => s["format"] = json!("email"),
                crate::kural::Kural::Url => s["format"] = json!("uri"),
                _ => {}
            }
        }
        Some(s)
    };
    // zod: of(z.tip(...)...)
    if let Some(ic) = a.kurallar.iter().find_map(|r| r.strip_prefix("of(").and_then(|r| r.strip_suffix(')'))) {
        let tip = ic.strip_prefix("z.")?.split(['(', '.']).next()?;
        return tip_semasi(tip, &[]);
    }
    // class-validator: öğeye uygulanan tip kuralı (IsUUID('4', { each: true }), IsString({ each: true }))
    let her: Vec<&String> = a.kurallar.iter().filter(|r| crate::kural::her_oge(r)).collect();
    if her.iter().any(|r| r.starts_with("IsUUID(")) {
        return Some(json!({ "type": "string", "format": "uuid" }));
    }
    if let Some(t) = her.iter().find_map(|r| crate::kural::tip_kurali(r)) {
        return Some(json!({ "type": t }));
    }
    // TypeScript: string[] / number[]
    a.tip.as_deref().and_then(|t| t.strip_suffix("[]")).and_then(|t| tip_semasi(t, &[]))
}

/// Girdi listesinden (gövde ya da sorgu) nesne şeması: özellikler ve zorunlular; şema çözülemediyse None
/// Dizi şemasının öğesi (bileşen adı öğe şemasına verilir), nesne şemasının kendisi
fn nesne_semasi(s: &Value) -> Value {
    if s["type"] == "array" {
        s["items"].clone()
    } else {
        s.clone()
    }
}

fn nesne(girdiler: &[Girdi]) -> Option<(Map<String, Value>, Vec<String>)> {
    let mut ozellik = Map::new();
    let mut zorunlu = vec![];
    let mut bilinen = false;
    for g in girdiler {
        match &g.sema {
            Some(s) if !s.bulunamadi => {
                bilinen = true;
                for (ad, a) in &s.alanlar {
                    ozellik.insert(ad.clone(), sema(a));
                    if a.zorunlu {
                        zorunlu.push(ad.clone());
                    }
                }
            }
            _ => {
                if let Some(ad) = &g.ad {
                    ozellik.insert(ad.clone(), crate::islem_arac::tek_girdi_semasi(g.tip.as_deref()));
                    bilinen = true;
                }
            }
        }
    }
    bilinen.then_some((ozellik, zorunlu))
}

/// `ozet`: haritanın özeti; uygulamanın kendi işlem kimlikleri (`islem_kimlikleri`, "dosya:satır" -> operationId) ve
/// yalnızca bazı ortamlarda kaydedilen kapılar (`kosullu_kapilar`, "YÖNTEM /yol") buradan okunur
pub fn uret(kapilar: &[Kapi], uygulama: &str, surum: &str, ozet: &Value) -> Value {
    let mut yollar: Map<String, Value> = Map::new();
    let mut semalar: Map<String, Value> = Map::new();
    let mut kimlikler = std::collections::HashSet::new();
    // Aynı adres ve yönteme birden fazla API sürümü düşebilir (sürüm başlıkla seçilir). OpenAPI bir adres ve yöntem için
    // tek işlem yazar: en yeni sürüm yazılır, diğerleri x-sezgi-diger-surumler alanında listelenir.
    let mut sirali: Vec<&Kapi> = kapilar.iter().filter(|k| k.ana).collect();
    sirali.sort_by(|a, b| (&a.yol, &a.yontem).cmp(&(&b.yol, &b.yontem)).then_with(|| b.surum.cmp(&a.surum)));
    for k in sirali {
        let yol = k.yol.split('/').map(|p| p.strip_prefix(':').map(|x| format!("{{{x}}}")).unwrap_or_else(|| p.to_string())).collect::<Vec<_>>().join("/");
        // Uygulamanın kendi işlem kimliği (FastAPI generate_unique_id_function, @ApiOperation operationId) koddan
        // okunabildiyse o; değilse Sezgi'nin kimliği. Çakışan kimlik ekle ayrıştırılır.
        let uygulamanin = ozet["islem_kimlikleri"][format!("{}:{}", k.dosya, k.satir).as_str()].as_str().map(String::from);
        let taban = uygulamanin.clone().unwrap_or_else(|| benlik::islem_kimligi_adlardan(&k.kontrolcu, &k.fonksiyon));
        let mut id = taban.clone();
        let mut n = 2;
        while !kimlikler.insert(id.clone()) {
            id = format!("{taban}-{n}");
            n += 1;
        }
        let mut parametreler: Vec<Value> =
            k.parametreler.iter().filter_map(|p| p.ad.as_ref()).map(|ad| json!({ "name": ad, "in": "path", "required": true, "schema": { "type": "string" } })).collect();
        // Yol şablonundaki her parametre yazılır (koddaki @Param adı olmasa bile OpenAPI şartı)
        for p in k.yol.split('/').filter_map(|p| p.strip_prefix(':')) {
            if !parametreler.iter().any(|x| x["name"] == p) {
                parametreler.push(json!({ "name": p, "in": "path", "required": true, "schema": { "type": "string" } }));
            }
        }
        if let Some((ozellik, zorunlu)) = nesne(&k.sorgu) {
            for (ad, s) in ozellik {
                parametreler.push(json!({ "name": ad, "in": "query", "required": zorunlu.contains(&ad), "schema": s }));
            }
        }
        let mut islem = json!({
            "operationId": id,
            "summary": k.ozet.clone().unwrap_or_else(|| k.fonksiyon.clone()),
            "parameters": parametreler,
            "responses": { "default": { "description": "Response of the application (not derived by Sezgi)" } },
            "x-sezgi-giris": k.giris,
            "x-sezgi-giris-kaniti": k.giris_kaniti,
            "x-sezgi-kaynak": format!("{}:{}", k.dosya, k.satir),
        });
        if uygulamanin.is_some() {
            islem["x-sezgi-islem-kimligi"] = json!(benlik::islem_kimligi_adlardan(&k.kontrolcu, &k.fonksiyon));
        }
        // Yalnızca bazı ortamlarda kaydedilen kapı (ör. `if settings.ENV == "local": include_router(...)`)
        if ozet["kosullu_kapilar"].as_array().is_some_and(|l| l.iter().any(|x| x.as_str() == Some(k.id.as_str()))) {
            islem["x-sezgi-kosullu"] = json!(true);
        }
        if !k.govde.is_empty() {
            // Gövde şeması components.schemas altında, DTO adıyla (aynı ad farklı şemaysa işlem kimliğiyle)
            let govde = match nesne(&k.govde) {
                Some((ozellik, zorunlu)) => {
                    let nesne = json!({ "type": "object", "properties": ozellik, "required": zorunlu });
                    // Gövdenin kendisi dizi (z.array(z.object(...))): öğe şeması items
                    let s = if k.govde.iter().any(|g| g.sema.as_ref().is_some_and(|s| s.dizi)) { json!({ "type": "array", "items": nesne }) } else { nesne };
                    let dto = k.govde.iter().find_map(|g| g.sema.as_ref().map(|s| s.dto.clone())).filter(|d| d.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
                    let ad = match dto {
                        Some(d) if semalar.get(&d).is_none_or(|v| *v == s) => d,
                        _ => format!("{}Body", id.replace(['.', '-'], "_")),
                    };
                    semalar.insert(ad.clone(), s);
                    json!({ "$ref": format!("#/components/schemas/{ad}") })
                }
                None => json!({ "description": "request body schema could not be resolved from code" }),
            };
            islem["requestBody"] = json!({ "required": true, "content": { "application/json": { "schema": govde } } });
        }
        // Başarılı yanıtın şeması (koddan): components.schemas altında, DTO adıyla
        if let Some(y) = &k.yanit {
            let mut ozellik = Map::new();
            let mut zorunlu = vec![];
            for (ad, a) in &y.sema.alanlar {
                ozellik.insert(ad.clone(), sema(a));
                if a.zorunlu && !a.belirsiz {
                    zorunlu.push(ad.clone());
                }
            }
            let nesne = json!({ "type": "object", "properties": ozellik, "required": zorunlu });
            let s = if y.sema.dizi { json!({ "type": "array", "items": nesne }) } else { nesne };
            let dto = Some(y.sema.dto.clone()).filter(|d| d.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            let ad = match dto {
                Some(d) if semalar.get(&d).is_none_or(|v| *v == nesne_semasi(&s)) => d,
                _ => format!("{}Response", id.replace(['.', '-'], "_")),
            };
            let tek = nesne_semasi(&s);
            semalar.insert(ad.clone(), tek);
            let ref_ = json!({ "$ref": format!("#/components/schemas/{ad}") });
            let govde = if y.sema.dizi { json!({ "type": "array", "items": ref_ }) } else { ref_ };
            let durum = y.durum.unwrap_or(200).to_string();
            islem["responses"] = json!({
                durum: { "description": "Successful response (schema from code)", "content": { "application/json": { "schema": govde } } },
                "default": { "description": "Other responses of the application (not derived by Sezgi)" },
            });
            islem["x-sezgi-yanit"] = json!({ "kaynak": y.kaynak, "kesin": y.kesin });
        }
        if let Some(s) = &k.surum {
            islem["x-sezgi-surum"] = json!(s);
        }
        islem["security"] = match k.giris.as_str() {
            "herkese_acik" => json!([]),
            "istege_bagli" => json!([{}, { "bearerAuth": [] }]),
            _ => json!([{ "bearerAuth": [] }]),
        };
        if !k.roller.is_empty() {
            islem["x-sezgi-roller"] = json!(k.roller);
        }
        if k.kullanimdan_kalkmis {
            islem["deprecated"] = json!(true);
        }
        if let Some(v) = &k.veri {
            islem["x-sezgi-veri"] = json!({ "okur": v.okur, "yazar": v.yazar });
        }
        let yontem = k.yontem.to_lowercase();
        if yontem == "all" {
            continue;
        }
        let girdi = yollar.entry(yol).or_insert_with(|| json!({}));
        match girdi.get_mut(&yontem) {
            None => girdi[yontem] = islem,
            Some(var) => {
                let liste = var.as_object_mut().unwrap().entry("x-sezgi-diger-surumler").or_insert_with(|| json!([]));
                liste.as_array_mut().unwrap().push(json!({ "surum": k.surum, "operationId": islem["operationId"], "x-sezgi-giris": k.giris }));
            }
        }
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": uygulama,
            "version": surum,
            "description": format!("Generated by Sezgi {} from the application's code. Stamps and sources are in x-sezgi-* fields; the application has the last word.", env!("CARGO_PKG_VERSION"))
        },
        "paths": yollar,
        "components": { "schemas": semalar, "securitySchemes": { "bearerAuth": { "type": "http", "scheme": "bearer" } } }
    })
}
