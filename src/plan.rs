//! Değişiklik planı: yapay zekâ birden fazla uygulamadaki işlemleri tek bir plan olarak sunar (santral MCP `plan`).
//! Plan YÜRÜTMEZ. İnsan `sezgi plan goster <id>` ile her adımın uygulamasını, işlemini, gövdesini, etkilediği eşlenmiş
//! varlıkları ve iş kurallarını görür; `sezgi plan onayla <id> --adim <n> --ozet <özet>` ile adımları sırayla onaylar.
//! Her adım canlı modun onay yolundan geçer (token izni, insan oturumu ve doğrulanmış rolü, bayatlık, mod, test kopyası
//! ya da gerçek veri, bugünkü haritayla önizleme, içerik özeti, tekrar koruması, zincirli kayıt): plan politikayı atlayan
//! bir yol değildir. Gerçek veride adım da yalnızca terminalde onaylanır ve aynı ek kapılardan geçer.
//!
//! Planlar santral yapılandırma klasöründedir (`santral-planlar/<id>.json`, yalnızca kullanıcıya açık). Adımların gövdesi
//! ve argümanları müşteri verisidir: her uygulamanın kendi durum anahtarıyla şifrelidir.

use serde_json::{json, Value};
use std::path::PathBuf;

pub const KLASOR: &str = "santral-planlar";

fn dur(m: &str) -> ! {
    eprintln!("✗ {m}");
    std::process::exit(1)
}

/// Eksik ya da geçersiz argüman: kullanım satırıyla, çıkış kodu 2
fn kullanim(m: &str) -> ! {
    crate::yardim::kullanim_hatasi("plan", m)
}

fn klasor() -> Result<PathBuf, String> {
    crate::imza::yapilandirma_klasoru().map(|k| k.join(KLASOR)).ok_or_else(|| t!("yapılandırma klasörü bulunamadı", "configuration folder not found").into())
}

fn kimlik_gecerli(id: &str) -> bool {
    (8..=32).contains(&id.len()) && id.chars().all(|c| c.is_ascii_hexdigit())
}

fn yol(id: &str) -> Result<PathBuf, String> {
    if !kimlik_gecerli(id) {
        return Err(tf!("geçersiz plan kimliği: {id}", "invalid plan id: {id}"));
    }
    Ok(klasor()?.join(format!("{id}.json")))
}

fn oku(id: &str) -> Result<Value, String> {
    let p = yol(id)?;
    crate::guvenli::oku_harita(&p)
        .and_then(|m| serde_json::from_str(&m).ok())
        .ok_or_else(|| tf!("plan bulunamadı: {id} (sezgi plan listele)", "plan not found: {id} (sezgi plan listele)"))
}

fn yaz(plan: &Value) -> Result<(), String> {
    let k = klasor()?;
    crate::guvenli::gizli_klasor(&k).map_err(|e| format!("{}: {e}", k.display()))?;
    crate::guvenli::atomik_yaz_gizli(&yol(plan["id"].as_str().unwrap_or(""))?, serde_json::to_string_pretty(plan).map_err(|e| e.to_string())?.as_bytes())
}

/// Yeni planı kaydeder; kimliği döner
pub fn kaydet(mut plan: Value) -> Result<String, String> {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).map_err(|e| tf!("rastgele sayı üretilemedi: {e}", "could not generate a random number: {e}"))?;
    let id: String = b.iter().map(|x| format!("{x:02x}")).collect();
    plan["id"] = json!(id);
    plan["zaman"] = json!(crate::simdi());
    plan["durum"] = json!("bekliyor");
    yaz(&plan)?;
    Ok(id)
}

fn adim_harita(a: &Value) -> PathBuf {
    PathBuf::from(a["harita"].as_str().unwrap_or(""))
}

fn secenek(args: &[String], ad: &str) -> Option<String> {
    args.iter().position(|a| a == ad).and_then(|i| args.get(i + 1)).cloned()
}

/// Bir adımın insana gösterilen hâli: uygulama, istek (gövdesiyle), etkilenen eşlenmiş varlıklar, iş kuralları, onay komutu
fn adim_metni(id: &str, n: usize, toplam: usize, a: &Value) -> String {
    let t = crate::guvenli::temiz;
    let mut s = tf!(
        "\n── Adım {n}/{toplam} · {} · {} · {}\n",
        "\n── Step {n}/{toplam} · {} · {} · {}\n",
        t(a["uygulama"].as_str().unwrap_or("")),
        t(a["islem"].as_str().unwrap_or("")),
        t(durum_adi(a["durum"].as_str().unwrap_or("")))
    );
    match crate::canli::plan_adimi_metni(&adim_harita(a), a) {
        Ok((m, icerik)) => {
            s.push_str(&m);
            let etkilenen: Vec<String> = a["etkilenen"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(t)).collect();
            let etkilenen = if etkilenen.is_empty() { t!("yok (ya da eşlenmemiş)", "none (or not mapped)").to_string() } else { etkilenen.join("; ") };
            s.push_str(&tf!("Etkilenen eşlenmiş varlıklar: {etkilenen}\n", "Mapped entities affected: {etkilenen}\n"));
            let kurallar: Vec<String> = a["is_kurallari"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(t)).collect();
            let kurallar =
                if kurallar.is_empty() { t!("bilinmiyor (işlemin kartında kural yok)", "unknown (the operation's card has no rules)").to_string() } else { kurallar.join("; ") };
            s.push_str(&tf!("İş kuralları (kart): {kurallar}\n", "Business rules (card): {kurallar}\n"));
            if a["durum"] == "bekliyor" && a["risk"] != "oku" && (a["gercek_veri"] == true || crate::canli::gercek_veri_mi(&adim_harita(a))) {
                s.push_str(&tf!(
                    "Onay (GERÇEK VERİ, yalnızca terminalde): sezgi plan onayla {id} --adim {n}\n",
                    "Approval (REAL DATA, in a terminal only): sezgi plan onayla {id} --adim {n}\n"
                ));
            } else if a["durum"] == "bekliyor" {
                s.push_str(&tf!("Onay: sezgi plan onayla {id} --adim {n} --ozet {}\n", "Approval: sezgi plan onayla {id} --adim {n} --ozet {}\n", &icerik[..8.min(icerik.len())]));
            }
        }
        Err(e) => s.push_str(&tf!("(gösterilemedi: {e})\n", "(could not be shown: {e})\n")),
    }
    s
}

/// Plan ya da adım durumunun okunur adı (dosyadaki değer Türkçe kalır)
fn durum_adi(d: &str) -> &str {
    match d {
        "bekliyor" => t!("bekliyor", "pending"),
        "yapildi" => t!("yapildi", "done"),
        "reddedildi" => t!("reddedildi", "rejected"),
        "tamam" => t!("tamam", "complete"),
        "hata" => t!("hata", "failed"),
        x => x,
    }
}

/// sezgi plan listele | goster <id> | onayla <id> [--adim <n>] [--ozet <ilk8>] | reddet <id>
pub fn komut(args: &[String]) {
    match args.get(1).map(String::as_str) {
        Some("listele") | None => {
            let k = klasor().unwrap_or_else(|e| dur(&e));
            let mut planlar: Vec<Value> = std::fs::read_dir(&k)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|g| g.path().extension().is_some_and(|e| e == "json"))
                .filter_map(|g| crate::guvenli::oku_harita(&g.path()).and_then(|m| serde_json::from_str(&m).ok()))
                .collect();
            planlar.sort_by(|a: &Value, b: &Value| a["zaman"].as_str().cmp(&b["zaman"].as_str()));
            if planlar.is_empty() {
                tp!(
                    "Plan yok. Planları yapay zekâ santral MCP'deki `plan` aracıyla hazırlar; plan yürütülmez, burada onaylanır.",
                    "No plans. The AI prepares plans with the `plan` tool of the switchboard MCP; a plan is never executed, it is approved here."
                );
                return;
            }
            for p in planlar {
                let adimlar = p["adimlar"].as_array().cloned().unwrap_or_default();
                let yapilan = adimlar.iter().filter(|a| a["durum"] == "yapildi").count();
                tp!(
                    "  {} · {} · {} · {yapilan}/{} adım · {}",
                    "  {} · {} · {} · {yapilan}/{} {a} · {}",
                    p["id"].as_str().unwrap_or(""),
                    crate::guvenli::temiz(p["baslik"].as_str().unwrap_or("")),
                    durum_adi(p["durum"].as_str().unwrap_or("")),
                    adimlar.len(),
                    p["zaman"].as_str().unwrap_or("");
                    a = crate::dil::cogul(adimlar.len(), "step", "steps")
                );
            }
        }
        Some("goster") => {
            let id = args.get(2).filter(|a| !a.starts_with("--")).unwrap_or_else(|| kullanim(kimlik_gerekli()));
            let p = oku(id).unwrap_or_else(|e| dur(&e));
            let adimlar = p["adimlar"].as_array().cloned().unwrap_or_default();
            print!(
                "{}",
                tf!(
                    "Plan {} · {} · {} · {} adım · {}\nPlan yürütülmez: her adım ayrı onaylanır ve canlı modun bütün kapılarından geçer.\n",
                    "Plan {} · {} · {} · {} {a} · {}\nThe plan is never executed: each step is approved separately and passes every gate of live mode.\n",
                    id,
                    crate::guvenli::temiz(p["baslik"].as_str().unwrap_or("")),
                    durum_adi(p["durum"].as_str().unwrap_or("")),
                    adimlar.len(),
                    p["zaman"].as_str().unwrap_or("");
                    a = crate::dil::cogul(adimlar.len(), "step", "steps")
                )
            );
            for (i, a) in adimlar.iter().enumerate() {
                print!("{}", adim_metni(id, i + 1, adimlar.len(), a));
            }
        }
        Some("onayla") => onayla(args),
        Some("reddet") => {
            let id = args.get(2).filter(|a| !a.starts_with("--")).unwrap_or_else(|| kullanim(kimlik_gerekli()));
            let mut p = oku(id).unwrap_or_else(|e| dur(&e));
            p["durum"] = json!("reddedildi");
            for a in p["adimlar"].as_array_mut().into_iter().flatten() {
                if a["durum"] == "bekliyor" {
                    a["durum"] = json!("reddedildi");
                }
                a["govde"] = Value::Null;
                a["argumanlar"] = Value::Null;
            }
            yaz(&p).unwrap_or_else(|e| dur(&e));
            tp!("✓ Plan reddedildi: {id}", "✓ Plan rejected: {id}");
        }
        Some(x) => kullanim(&tf!("bilinmeyen: plan {x} (listele, goster, onayla, reddet)", "unknown: plan {x} (listele, goster, onayla, reddet)")),
    }
}

fn kimlik_gerekli() -> &'static str {
    t!("plan kimliği gerekli", "a plan id is required")
}

/// Tek adımı onaylar ve yapar. Adımlar sırayla: önceki adım yapılmadan sonraki onaylanmaz. Gövdeli adım, insanın
/// gördüğü içerik özetiyle onaylanır (terminalde açık "evet", terminal yoksa `--ozet`). Gerçek veride onay yalnızca
/// terminalde verilir: `--ozet` reddedilir.
fn onayla(args: &[String]) {
    let id = args.get(2).filter(|a| !a.starts_with("--")).unwrap_or_else(|| kullanim(kimlik_gerekli()));
    let mut p = oku(id).unwrap_or_else(|e| dur(&e));
    if p["durum"] == "reddedildi" {
        dur(&tf!("plan reddedilmiş: {id}", "the plan was rejected: {id}"));
    }
    let adimlar = p["adimlar"].as_array().cloned().unwrap_or_default();
    let n = match secenek(args, "--adim") {
        Some(x) => x
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=adimlar.len()).contains(n))
            .unwrap_or_else(|| kullanim(&tf!("--adim 1 ile {} arasında olmalı", "--adim must be between 1 and {}", adimlar.len()))),
        None => adimlar.iter().position(|a| a["durum"] == "bekliyor").map(|i| i + 1).unwrap_or_else(|| dur(t!("bekleyen adım yok", "no pending step"))),
    };
    let a = &adimlar[n - 1];
    if a["durum"] != "bekliyor" {
        dur(&tf!("adım {n} bekliyor değil ({})", "step {n} is not pending ({})", durum_adi(a["durum"].as_str().unwrap_or(""))));
    }
    if let Some(k) = adimlar[..n - 1].iter().position(|x| x["durum"] != "yapildi") {
        dur(&tf!("adımlar sırayla onaylanır: önce adım {} (sezgi plan goster {id})", "steps are approved in order: step {} first (sezgi plan goster {id})", k + 1));
    }
    let (_, icerik) = crate::canli::plan_adimi_metni(&adim_harita(a), a).unwrap_or_else(|e| dur(&e));
    print!("{}", adim_metni(id, n, adimlar.len(), a));
    // Onay canlı modun tek yolundan: test kopyasında `--ozet` ya da terminalde "evet"; gerçek veride yalnızca terminal
    let (onay, ozet) = crate::canli::onay_al(
        &adim_harita(a),
        a,
        secenek(args, "--ozet").as_deref(),
        &icerik,
        t!("Bu adım bu gövdeyle gönderilsin mi?", "Send this step with this body?"),
        &format!("sezgi plan onayla {id} --adim {n}"),
        &format!("sezgi plan goster {id}"),
    )
    .unwrap_or_else(|e| dur(&e));
    // Tek seferlik kilit: aynı adımı iki süreç aynı anda yapamaz; gönderildiyse kilit kalır
    let kilit = klasor().unwrap_or_else(|e| dur(&e)).join(format!("{id}-{n}.kilit"));
    let mut o = crate::guvenli::gizli_secenekler();
    o.write(true).create_new(true);
    if o.open(&kilit).is_err() {
        dur(&tf!("adım {n} şu anda başka bir süreçte işleniyor ya da işlendi", "step {n} is being processed by another process right now, or already was"));
    }
    match crate::canli::plan_adimi_yap(&adim_harita(a), a, &onay, &ozet, id, n) {
        Ok((durum, sira)) => {
            let s = &mut p["adimlar"][n - 1];
            s["durum"] = json!("yapildi");
            s["sonuc_kodu"] = json!(durum);
            s["govde"] = Value::Null;
            s["argumanlar"] = Value::Null;
            if p["adimlar"].as_array().is_some_and(|l| l.iter().all(|x| x["durum"] == "yapildi")) {
                p["durum"] = json!("tamam");
            }
            yaz(&p).unwrap_or_else(|e| dur(&e));
            tp!("✓ Adım {n} yapıldı: HTTP {durum} · {} kaydı #{sira}", "✓ Step {n} done: HTTP {durum} · {} log #{sira}", p["adimlar"][n - 1]["uygulama"].as_str().unwrap_or(""));
        }
        Err((gonderildi, e)) => {
            if gonderildi {
                p["adimlar"][n - 1]["durum"] = json!("hata");
                p["adimlar"][n - 1]["hata"] = json!(e);
                let _ = yaz(&p);
            } else {
                let _ = std::fs::remove_file(&kilit);
            }
            dur(&e);
        }
    }
}
