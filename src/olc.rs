//! `sezgi olc [uygulama=.] [--harita <klasör>] [--openapi <dosya>] [--onek /api] [--json]`: kanıt testinin kullanıcının
//! kendi uygulamasında yerelde ölçülebilen kısmı (vizyon 16). Yapay zekâ çalıştırmaz, ağa çıkmaz, hiçbir dosya yazmaz.
//! - Bağlam: bir endpoint'i anlamak için okunması gereken kod (controller ve girdi şeması dosyaları bütün olarak) ile Sezgi
//!   kartının (`tani`) boyu. Token ≈ bayt / 4 (ikisi de aynı yaklaşımla; gerçek sayı modelin token'layıcısına bağlıdır).
//! - Hız: kartların süreç içinde getirilme süresi (p50, p95).
//! - Doğruluk: uygulamanın kendi OpenAPI dosyası verilirse endpoint, giriş ve gövde alanı karşılaştırması
//!   (`kanit/karsilastir-openapi.js` ile aynı kurallar).
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

use crate::mcp::Harita;

fn secenek(args: &[String], ad: &str) -> Option<String> {
    args.iter().position(|a| a == ad).and_then(|i| args.get(i + 1)).cloned()
}

fn dur(m: &str) -> ! {
    eprintln!("✗ {m}");
    std::process::exit(1)
}

fn token(bayt: usize) -> u64 {
    bayt.div_ceil(4) as u64
}

fn ortanca(v: &mut [u64]) -> u64 {
    v.sort_unstable();
    v.get(v.len() / 2).copied().unwrap_or(0)
}

fn yuzdelik(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s.get(((q * s.len() as f64).ceil() as usize).saturating_sub(1).min(s.len().saturating_sub(1))).copied().unwrap_or(0.0)
}

/// Adres karşılaştırması için tek biçim: parametre adları {} olur, sondaki / atılır, küçük harf
fn norm(y: &str) -> String {
    let mut s = String::new();
    for (i, p) in y.split('/').enumerate() {
        if i > 0 {
            s.push('/');
        }
        if p.starts_with(':') || (p.starts_with('{') && p.ends_with('}')) {
            s.push_str("{}");
        } else {
            s.push_str(&p.to_lowercase());
        }
    }
    s.trim_end_matches('/').to_string()
}

fn dogruluk(kapilar: &[Value], doc: &Value, onek: &str) -> Value {
    let semalar = &doc["components"]["schemas"];
    let mut resmi: BTreeMap<String, (Value, Option<Value>)> = BTreeMap::new();
    if let Some(paths) = doc["paths"].as_object() {
        for (p, ops) in paths {
            for (m, o) in ops.as_object().into_iter().flatten() {
                if !o.is_object() || o.get("responses").is_none() {
                    continue;
                }
                let sema = o["requestBody"]["content"]
                    .as_object()
                    .and_then(|c| c.values().next())
                    .map(|c| &c["schema"])
                    .and_then(|s| s["$ref"].as_str().map(|r| semalar[r.rsplit('/').next().unwrap_or("")].clone()).or_else(|| s.get("properties").map(|_| s.clone())));
                resmi.insert(format!("{} {}", m.to_uppercase(), norm(&format!("{onek}{p}"))), (o.clone(), sema));
            }
        }
    }
    // Aynı adrese birden fazla kapı düşebilir: ana uygulamanınki seçilir
    let mut sezgi: BTreeMap<String, &Value> = BTreeMap::new();
    for e in kapilar {
        let k = format!("{} {}", e["yontem"].as_str().unwrap_or(""), norm(e["yol"].as_str().unwrap_or("")));
        let ana = e["ana"].as_bool() != Some(false);
        if ana || !sezgi.contains_key(&k) {
            sezgi.insert(k, e);
        }
    }
    let guvenlik_var = resmi.values().any(|(o, _)| o["security"].as_array().is_some_and(|a| !a.is_empty()));
    let (mut bulunan, mut giris, mut alan_toplam, mut alan_bulunan, mut zorunlu) = (0, 0, 0, 0, 0);
    let mut eksik = vec![];
    let mut farklar = vec![];
    for (k, (o, sema)) in &resmi {
        let Some(e) = sezgi.get(k) else {
            eksik.push(k.clone());
            continue;
        };
        bulunan += 1;
        let (r_giris, s_giris) = if guvenlik_var {
            let r = if o["security"].as_array().is_some_and(|a| !a.is_empty()) { "gerekli" } else { "herkese_acik" };
            (r.to_string(), if e["giris"] == "herkese_acik" { "herkese_acik".to_string() } else { "gerekli".to_string() })
        } else {
            let baslik = o["parameters"].as_array().into_iter().flatten().find(|q| {
                q["in"] == "header"
                    && q["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case("authorization") || (n.to_lowercase().starts_with("x-") && n.to_lowercase().ends_with("key")))
            });
            let r = match baslik {
                None => "herkese_acik",
                Some(b) if b["required"] == true => "gerekli",
                Some(_) => "istege_bagli",
            };
            (r.to_string(), e["giris"].as_str().unwrap_or("").to_string())
        };
        if r_giris == s_giris {
            giris += 1;
        } else {
            farklar.push(format!("{k} giriş: sezgi={s_giris} resmi={r_giris}"));
        }
        if let Some(props) = sema.as_ref().and_then(|s| s["properties"].as_object()) {
            let mut s_alan = serde_json::Map::new();
            for g in e["govde"].as_array().into_iter().flatten() {
                if let Some(a) = g["sema"]["alanlar"].as_object() {
                    s_alan.extend(a.clone());
                }
            }
            let gerekli: BTreeSet<&str> = sema.as_ref().unwrap()["required"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
            for ad in props.keys() {
                alan_toplam += 1;
                let Some(a) = s_alan.get(ad) else {
                    farklar.push(format!("{k} alan yok: {ad}"));
                    continue;
                };
                alan_bulunan += 1;
                if a["zorunlu"].as_bool() == Some(gerekli.contains(ad.as_str())) {
                    zorunlu += 1;
                } else {
                    farklar.push(format!("{k} {ad} zorunlu: sezgi={} resmi={}", a["zorunlu"], gerekli.contains(ad.as_str())));
                }
            }
        }
    }
    let fazla: Vec<&String> = sezgi.keys().filter(|k| !resmi.contains_key(*k)).collect();
    json!({
        "resmi": resmi.len(), "bulunan": bulunan, "fazla": fazla.len(), "giris_dogru": giris,
        "alan_toplam": alan_toplam, "alan_bulunan": alan_bulunan, "zorunlu_dogru": zorunlu,
        "eksik": eksik, "fazla_ornek": fazla.iter().take(20).collect::<Vec<_>>(), "farklar": farklar.iter().take(50).collect::<Vec<_>>(),
    })
}

pub fn calistir(args: &[String]) {
    let kok_arg = args.get(1).filter(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let kok = std::fs::canonicalize(&kok_arg).unwrap_or(kok_arg);
    let harita = secenek(args, "--harita").map(PathBuf::from).unwrap_or_else(|| kok.join(".sezgi"));
    let h = Harita::yukle(&harita).unwrap_or_else(|e| dur(&tf!("{e} → önce: sezgi init", "{e} → first run: sezgi init")));
    let ham: Value = serde_json::from_str(&std::fs::read_to_string(harita.join("kapilar.json")).unwrap_or_default()).unwrap_or_default();
    let kapilar = ham["endpointler"].as_array().cloned().unwrap_or_default();
    if kapilar.is_empty() {
        dur(t!("haritada endpoint yok", "the map has no endpoints"));
    }

    // Bağlam ve hız
    let mut boy: BTreeMap<String, usize> = BTreeMap::new();
    let mut dosya_boyu = |goreli: &str| -> usize { *boy.entry(goreli.to_string()).or_insert_with(|| std::fs::metadata(kok.join(goreli)).map(|m| m.len() as usize).unwrap_or(0)) };
    let (mut kod, mut kart, mut sureler) = (vec![], vec![], vec![]);
    for e in &kapilar {
        let mut dosyalar: BTreeSet<String> = BTreeSet::new();
        if let Some(d) = e["dosya"].as_str() {
            dosyalar.insert(d.to_string());
        }
        for g in e["govde"].as_array().into_iter().flatten().chain(e["sorgu"].as_array().into_iter().flatten()) {
            if let Some(d) = g["sema"]["dosya"].as_str() {
                dosyalar.insert(d.to_string());
            }
        }
        kod.push(token(dosyalar.iter().map(|d| dosya_boyu(d)).sum()));
        let kimlik = format!("{} {}", e["yontem"].as_str().unwrap_or(""), e["yol"].as_str().unwrap_or(""));
        let t = Instant::now();
        let (metin, _) = h.tani(&kimlik, Some(&crate::canli::islem_bilgisi(e).0));
        sureler.push(t.elapsed().as_secs_f64() * 1000.0);
        kart.push(token(metin.len()));
    }
    let (kod_toplam, kart_toplam): (u64, u64) = (kod.iter().sum(), kart.iter().sum());
    let (kod_ort, kart_ort) = (ortanca(&mut kod), ortanca(&mut kart));
    let oran = if kart_ort > 0 { kod_ort as f64 / kart_ort as f64 } else { 0.0 };

    let dogru = secenek(args, "--openapi").map(|f| {
        let doc: Value = std::fs::read_to_string(&f)
            .ok()
            .and_then(|m| serde_json::from_str(&m).ok())
            .unwrap_or_else(|| dur(&tf!("OpenAPI dosyası okunamadı (JSON olmalı): {f}", "could not read the OpenAPI file (it must be JSON): {f}")));
        (f, dogruluk(&kapilar, &doc, &secenek(args, "--onek").unwrap_or_default()))
    });

    let sonuc = json!({
        "uygulama": ham["ozet"]["uygulama"],
        "endpoint": kapilar.len(),
        "baglam": { "kod_ortanca_token": kod_ort, "kart_ortanca_token": kart_ort, "kod_toplam_token": kod_toplam, "kart_toplam_token": kart_toplam, "oran": (oran * 10.0).round() / 10.0 },
        "hiz": { "p50_ms": yuzdelik(&sureler, 0.5), "p95_ms": yuzdelik(&sureler, 0.95), "cagri": sureler.len() },
        "dogruluk": dogru.as_ref().map(|(_, d)| d.clone()),
    });
    if args.iter().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&sonuc).unwrap());
        return;
    }
    tp!("Sezgi ölçümü: {} ({} endpoint)", "Sezgi measurement: {} ({} endpoints)", ham["ozet"]["uygulama"].as_str().unwrap_or("?"), kapilar.len());
    tp!(
        "Bu ölçüm yapay zekâ çalıştırmaz ve ağa çıkmaz; yerelde ölçülebilen kısımdır.\n",
        "This measurement runs no AI and makes no network calls; it is the part that can be measured locally.\n"
    );
    tp!("Bağlam: yapay zekânın bir endpoint'i anlamak için okuması gereken (token ≈ bayt / 4)", "Context: what an AI has to read to understand one endpoint (token ≈ bytes / 4)");
    tp!(
        "  Kodu okuyarak         ortanca {kod_ort} token  (controller ve girdi şeması dosyaları bütün olarak; guard, modül, servis sayılmadı)",
        "  Reading the code      median {kod_ort} tokens  (controller and input schema files in full; guards, modules and services not counted)"
    );
    tp!("  Sezgi kartıyla        ortanca {kart_ort} token", "  With the Sezgi card   median {kart_ort} tokens");
    tp!(
        "  Fark                  {:.1} kat daha az · bütün endpoint'ler için {kod_toplam} yerine {kart_toplam} token",
        "  Difference            {:.1}x less · {kart_toplam} instead of {kod_toplam} tokens for all endpoints",
        oran
    );
    tp!("\nHız", "\nSpeed");
    tp!(
        "  Kart getirme          p50 {:.3} ms · p95 {:.3} ms ({} kart, süreç içinde)",
        "  Card lookup           p50 {:.3} ms · p95 {:.3} ms ({} cards, in process)",
        yuzdelik(&sureler, 0.5),
        yuzdelik(&sureler, 0.95),
        sureler.len()
    );
    match &dogru {
        Some((f, d)) => {
            let y = |a: &Value, b: &Value| match (a.as_f64(), b.as_f64()) {
                (Some(a), Some(b)) if b > 0.0 => format!("{:.1}%", 100.0 * a / b),
                _ => "-".into(),
            };
            tp!("\nDoğruluk (uygulamanın kendi OpenAPI dosyası: {f})", "\nAccuracy (the application's own OpenAPI file: {f})");
            tp!("  Dokümandaki işlem     {}", "  Operations in doc     {}", d["resmi"]);
            tp!("  Sezgi'nin bulduğu     {}/{} ({})", "  Found by Sezgi        {}/{} ({})", d["bulunan"], d["resmi"], y(&d["bulunan"], &d["resmi"]));
            tp!("  Dokümanda olmayan     {}", "  Not in the doc        {}", d["fazla"]);
            tp!("  Giriş durumu doğru    {}", "  Login state correct   {}", y(&d["giris_dogru"], &d["bulunan"]));
            tp!("  Gövde alanı bulma     {}/{} ({})", "  Body fields found     {}/{} ({})", d["alan_bulunan"], d["alan_toplam"], y(&d["alan_bulunan"], &d["alan_toplam"]));
            tp!("  Zorunluluk doğru      {}", "  Required correct      {}", y(&d["zorunlu_dogru"], &d["alan_bulunan"]));
            let eksik: Vec<&str> = d["eksik"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).take(10).collect();
            if !eksik.is_empty() {
                tp!("  Eksik (ilk 10)        {}", "  Missing (first 10)    {}", eksik.join(" | "));
            }
        }
        None => tp!(
            "\nDoğruluk: uygulamanın OpenAPI dosyası varsa --openapi <dosya.json> ile karşılaştırılır.",
            "\nAccuracy: if the application has an OpenAPI file, compare with --openapi <file.json>."
        ),
    }
    tp!(
        "\nGörev başarısını (ilk denemede doğru işlem, harcanan token) kendi yapay zekânızla ölçmek için: görevleri",
        "\nTo measure task success (right operation on the first try, tokens spent) with your own AI: write the tasks"
    );
    tp!(
        "Sezgi'yi bağlamadan önce yazıp kilitleyin, aynı modelle kodla ve Sezgi'yle ayrı ayrı çalıştırın",
        "and lock them before connecting Sezgi, then run them with the same model on the code and with Sezgi separately"
    );
    tp!("(yöntem: kanit/SONUC-3-immich-ajan.md, görev biçimi: kanit/immich/gorevler.json).", "(method: kanit/SONUC-3-immich-ajan.md, task format: kanit/immich/gorevler.json).");
}
