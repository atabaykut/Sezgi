//! Sürümler arası fark (SPEC §7, vizyon 7 `karsilastir`): iki Benlik Kartının işlemlerini karşılaştırır.
//! İşlemler çağıranların gördüğü kimlikle eşleştirilir (HTTP yöntemi + yol). Bağlı yapay zekâları bozacak değişiklikler:
//! kaldırılan işlem, yeni zorunlu girdi, isteğe bağlıyken zorunlu olan girdi, kaldırılan girdi, değişen tip, daralan
//! izin verilen değerler, sıkılaşan sınırlar, yeni giriş zorunluluğu, kısıtlanan roller. Bunların dışındakiler
//! (yeni işlem, yeni isteğe bağlı girdi, gevşeyen kural) bozmayan değişikliktir.
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn anahtar(o: &Value) -> String {
    format!("{} {}", o["entry"]["method"].as_str().unwrap_or(""), o["entry"]["path"].as_str().unwrap_or(""))
}

fn ad(o: &Value) -> String {
    let id = o["id"].as_str().unwrap_or("");
    format!("{id} ({})", anahtar(o))
}

fn herkese_acik(o: &Value) -> bool {
    o["rules"].as_array().into_iter().flatten().any(|r| r["text"].as_str().is_some_and(|t| t.starts_with("No login required") || t.contains("treated as public")))
}

/// 10.0 -> "10", 2.5 -> "2.5"
fn sayi(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        x.to_string()
    }
}

fn degerler(g: &Value) -> Option<BTreeSet<String>> {
    g["enum"].as_array().map(|a| a.iter().map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).collect())
}

/// Bir işlemin iki sürümü: (bozan sebepler, bozmayan değişiklikler)
fn islem_farki(e: &Value, y: &Value) -> (Vec<String>, Vec<String>) {
    let (mut bozan, mut bozmayan) = (vec![], vec![]);
    let bos = serde_json::Map::new();
    let (ei, yi) = (e["inputs"].as_object().unwrap_or(&bos), y["inputs"].as_object().unwrap_or(&bos));
    for (a, g) in yi {
        let zorunlu = g["required"] == true;
        match ei.get(a) {
            None if zorunlu => bozan.push(format!("new input `{a}` is required")),
            None => bozmayan.push(format!("new optional input `{a}`")),
            Some(eg) => {
                if zorunlu && eg["required"] != true {
                    bozan.push(format!("input `{a}` is now required"));
                } else if !zorunlu && eg["required"] == true {
                    bozmayan.push(format!("input `{a}` is now optional"));
                }
                let (et, yt) = (eg["type"].as_str().unwrap_or("unknown"), g["type"].as_str().unwrap_or("unknown"));
                if et != yt && et != "unknown" && yt != "unknown" {
                    bozan.push(format!("input `{a}` changed type from {et} to {yt}"));
                }
                match (degerler(eg), degerler(g)) {
                    (Some(ed), Some(yd)) => {
                        let giden: Vec<&String> = ed.difference(&yd).collect();
                        if !giden.is_empty() {
                            bozan.push(format!("input `{a}` no longer accepts: {}", giden.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
                        }
                        if yd.difference(&ed).next().is_some() {
                            bozmayan.push(format!("input `{a}` accepts more values"));
                        }
                    }
                    (None, Some(_)) => bozan.push(format!("input `{a}` is now limited to fixed values")),
                    _ => {}
                }
                for (alan, yon, ad_) in [("min", 1.0, "minimum"), ("min_length", 1.0, "minimum length"), ("max", -1.0, "maximum"), ("max_length", -1.0, "maximum length")] {
                    let (ev, yv) = (eg[alan].as_f64(), g[alan].as_f64());
                    let sert = match (ev, yv) {
                        (Some(a), Some(b)) => (b - a) * yon > 0.0,
                        (None, Some(_)) => true,
                        _ => false,
                    };
                    if sert {
                        let yeni = sayi(yv.unwrap_or_default());
                        match ev {
                            Some(eski) => {
                                let fiil = if yon > 0.0 { "raised" } else { "lowered" };
                                bozan.push(format!("input `{a}` {ad_} {fiil} from {} to {yeni}", sayi(eski)));
                            }
                            None => bozan.push(format!("input `{a}` now has a {ad_} of {yeni}")),
                        }
                    }
                }
                if g["format"] != eg["format"] && !g["format"].is_null() {
                    bozan.push(format!("input `{a}` now requires format {}", g["format"].as_str().unwrap_or("")));
                }
            }
        }
    }
    for a in ei.keys().filter(|a| !yi.contains_key(*a)) {
        bozan.push(format!("input `{a}` was removed"));
    }
    if herkese_acik(e) && !herkese_acik(y) {
        bozan.push("login is now required".into());
    } else if !herkese_acik(e) && herkese_acik(y) {
        bozmayan.push("login is no longer required".into());
    }
    let roller = |o: &Value| -> BTreeSet<String> { o["roles"].as_array().into_iter().flatten().filter_map(|r| r.as_str().map(String::from)).collect() };
    let (er, yr) = (roller(e), roller(y));
    if er != yr {
        // Önceden rol istemeyen işlem rol istiyorsa ya da önceden yetkili bir rol listeden çıktıysa bazı çağıranlar düşer
        if (er.is_empty() && !yr.is_empty()) || er.difference(&yr).next().is_some() && !yr.is_empty() {
            bozan.push(format!("roles changed from [{}] to [{}]", er.iter().cloned().collect::<Vec<_>>().join(", "), yr.iter().cloned().collect::<Vec<_>>().join(", ")));
        } else {
            bozmayan.push(format!("roles changed from [{}] to [{}]", er.iter().cloned().collect::<Vec<_>>().join(", "), yr.iter().cloned().collect::<Vec<_>>().join(", ")));
        }
    }
    (bozan, bozmayan)
}

/// İki kartın farkı. Dönen nesne kartın `changes` alanının biçimindedir; ek olarak `non_breaking` (yalnızca rapor için).
/// Adresi ya da yöntemi değişen işlem (aynı kimlik, başka giriş) eklenmiş/kaldırılmış sayılmaz: çağıranın bildiği adres
/// artık cevap vermediği için bozan değişikliktir ve girdi farkları da ayrıca yazılır.
pub fn fark(eski: &Value, yeni: &Value) -> Value {
    let harita = |k: &Value| -> BTreeMap<String, Value> { k["operations"].as_array().into_iter().flatten().map(|o| (anahtar(o), o.clone())).collect() };
    let (e, y) = (harita(eski), harita(yeni));
    let yalniz = |a: &BTreeMap<String, Value>, b: &BTreeMap<String, Value>| -> Vec<Value> { a.iter().filter(|(k, _)| !b.contains_key(*k)).map(|(_, o)| o.clone()).collect() };
    let (yeni_yalniz, eski_yalniz) = (yalniz(&y, &e), yalniz(&e, &y));
    let (mut bozan, mut bozmayan) = (vec![], vec![]);
    // Taşınan işlemler: yalnızca bir tarafta görünen ve kimliği öbür tarafta da yalnız olan
    let mut tasinan = BTreeSet::new();
    for yo in &yeni_yalniz {
        let id = yo["id"].as_str().unwrap_or("");
        if let Some(eo) = eski_yalniz.iter().find(|o| !id.is_empty() && o["id"] == yo["id"]) {
            tasinan.insert(id.to_string());
            let (em, ey) = (eo["entry"]["method"].as_str().unwrap_or(""), eo["entry"]["path"].as_str().unwrap_or(""));
            let (ym, yy) = (yo["entry"]["method"].as_str().unwrap_or(""), yo["entry"]["path"].as_str().unwrap_or(""));
            let sebep = if em == ym { format!("path changed from {ey} to {yy}") } else { format!("entry changed from {em} {ey} to {ym} {yy}") };
            bozan.push(json!({ "operation": yo["id"], "reason": sebep }));
            let (b, n) = islem_farki(eo, yo);
            bozan.extend(b.into_iter().map(|r| json!({ "operation": yo["id"], "reason": r })));
            bozmayan.extend(n.into_iter().map(|r| format!("{}: {r}", ad(yo))));
        }
    }
    let kimlik = |o: &Value| o["id"].as_str().unwrap_or("").to_string();
    let eklenen: Vec<String> = yeni_yalniz.iter().map(kimlik).filter(|i| !tasinan.contains(i)).collect();
    let kaldirilan: Vec<String> = eski_yalniz.iter().map(kimlik).filter(|i| !tasinan.contains(i)).collect();
    for (k, yo) in &y {
        if let Some(eo) = e.get(k) {
            let (b, n) = islem_farki(eo, yo);
            bozan.extend(b.into_iter().map(|r| json!({ "operation": yo["id"], "reason": r })));
            bozmayan.extend(n.into_iter().map(|r| format!("{}: {r}", ad(yo))));
        }
    }
    json!({
        "previous_source_ref": eski["app"]["source_ref"].as_str().unwrap_or("unknown"),
        "added": eklenen,
        "removed": kaldirilan,
        "breaking": bozan,
        "non_breaking": bozmayan,
    })
}

/// `init` için: önceki kartla farkı kartın `changes` alanı olarak verir. Fark commit'e değil işlemlerin kendisine
/// bakar: commit'siz değişiklikte ve git olmayan projede de üretilir (`previous_source_ref` "unknown" olabilir).
/// İşlemler değişmediyse (aynı commit'te ya da kodda yeniden çalıştırma) önceki kartın farkı korunur; farklı
/// uygulamanın kartıyla fark yazılmaz.
pub fn kart_degisiklikleri(onceki: &Value, yeni: &Value) -> Option<Value> {
    if onceki["app"]["id"] != yeni["app"]["id"] {
        return None;
    }
    let mut f = fark(onceki, yeni);
    let bos = |a: &str| f[a].as_array().is_none_or(|x| x.is_empty());
    if bos("added") && bos("removed") && bos("breaking") && bos("non_breaking") {
        return onceki.get("changes").cloned();
    }
    f.as_object_mut().unwrap().remove("non_breaking");
    Some(f)
}

fn kart_oku(p: &Path) -> Value {
    let yol = if p.is_dir() { p.join("benlik.json") } else { p.to_path_buf() };
    std::fs::read_to_string(&yol).ok().and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_else(|| {
        te!("✗ Benlik Kartı okunamadı: {} (harita klasörü ya da benlik.json verin)", "✗ Could not read the Benlik Card: {} (give a map folder or a benlik.json)", yol.display());
        std::process::exit(1)
    })
}

/// sezgi karsilastir <eski> <yeni> [--json] [--sert]
pub fn calistir(args: &[String]) {
    let yollar: Vec<PathBuf> = args.iter().skip(1).filter(|a| !a.starts_with("--")).map(PathBuf::from).collect();
    let [eski_yol, yeni_yol] = yollar.as_slice() else {
        te!(
            "Kullanım: sezgi karsilastir <eski-harita|benlik.json> <yeni-harita|benlik.json> [--json] [--sert]",
            "Usage: sezgi diff <old-map|benlik.json> <new-map|benlik.json> [--json] [--strict]"
        );
        std::process::exit(2)
    };
    let (eski, yeni) = (kart_oku(eski_yol), kart_oku(yeni_yol));
    let f = fark(&eski, &yeni);
    let kirilma = !f["removed"].as_array().unwrap().is_empty() || !f["breaking"].as_array().unwrap().is_empty();
    if args.iter().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&f).unwrap());
    } else {
        tp!(
            "Karşılaştırma: {} ({}) → {} ({})",
            "Comparison: {} ({}) → {} ({})",
            eski["app"]["name"].as_str().unwrap_or("?"),
            eski["app"]["source_ref"].as_str().unwrap_or("?"),
            yeni["app"]["name"].as_str().unwrap_or("?"),
            yeni["app"]["source_ref"].as_str().unwrap_or("?")
        );
        let liste = |v: &Value| v.as_array().unwrap().iter().filter_map(|x| x.as_str().map(String::from)).collect::<Vec<_>>();
        let (eklenen, kaldirilan, bozmayan) = (liste(&f["added"]), liste(&f["removed"]), liste(&f["non_breaking"]));
        tp!("  Eklenen     {}{}", "  Added       {}{}", eklenen.len(), if eklenen.is_empty() { String::new() } else { format!(": {}", eklenen.join(", ")) });
        tp!(
            "  Kaldırılan  {}{}",
            "  Removed     {}{}",
            kaldirilan.len(),
            if kaldirilan.is_empty() {
                String::new()
            } else {
                tf!(": {} (bu işlemi çağıran yapay zekâlar bozulur)", ": {} (AIs calling these operations will break)", kaldirilan.join(", "))
            }
        );
        let bozan = f["breaking"].as_array().unwrap();
        tp!("  Bozan       {}", "  Breaking    {}", bozan.len());
        for b in bozan {
            println!("    ! {}: {}", b["operation"].as_str().unwrap_or(""), b["reason"].as_str().unwrap_or(""));
        }
        tp!("  Bozmayan    {}", "  Compatible  {}", bozmayan.len());
        for b in bozmayan.iter().take(20) {
            println!("    · {b}");
        }
        if kirilma {
            tp!(
                "\nBu sürüm, bağlı yapay zekâları ve entegrasyonları bozabilir. Yayından önce gözden geçirin.",
                "\nThis version may break connected AIs and integrations. Review it before release."
            );
        }
    }
    if kirilma && args.iter().any(|a| a == "--sert") {
        std::process::exit(1);
    }
}
