//! `sezgi teshis [ac|kapat]`: isteğe bağlı, yalnızca yerel tanılama kaydı (vizyon 12.4). Sezgi kullanım verisi toplamaz;
//! bir sorun yalnızca kullanıcının bilgisayarında görülürse kullanıcı bu dosyayı kendisi bir hata bildirimine ekler.
//! Kayıt yalnızca sayaç içerir: süreç türü, çalışma süresi, araç adına göre çağrı sayısı (bilinen adlar; gerisi "diger"),
//! hata sayısı, ortalama ve en uzun süre, harita yenileme sayısı, bellek (ölçülebiliyorsa). Kod, sorgu metni, adres,
//! kimlik ya da veri içermez. Hiçbir yere gönderilmez.
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

const BILINEN: [&str; 13] = ["kimsin", "ara", "tani", "onizle", "cagir", "onay_durumu", "kanit", "bilinmeyenler", "etki", "islem_araci", "initialize", "tools/list", "ping"];

fn klasor() -> Option<PathBuf> {
    crate::imza::yapilandirma_klasoru().map(|k| k.join("teshis"))
}

pub fn acik() -> bool {
    klasor().is_some_and(|k| k.join("acik").exists())
}

/// Bellek (yerleşik bayt): Linux'ta /proc'tan; Windows'ta bağımlılıksız okunamıyor
#[cfg(not(target_os = "macos"))]
fn bellek() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb: u64 = s.lines().find(|l| l.starts_with("VmRSS:"))?.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

/// Bellek (yerleşik bayt), macOS: /proc yok; sistemin kendi `ps` aracı (`-o rss=`, KiB) mutlak yoldan çalıştırılır
/// (PATH'teki başka bir `ps` değil)
#[cfg(target_os = "macos")]
fn bellek() -> Option<u64> {
    use std::process::{Command, Stdio};
    let c = Command::new("/bin/ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    let kb: u64 = String::from_utf8_lossy(&c.stdout).trim().parse().ok()?;
    Some(kb * 1024)
}

pub struct Sayac {
    tur: &'static str,
    baslangic: Instant,
    cagri: BTreeMap<&'static str, (u64, u64, f64, f64)>,
    yenileme: u64,
}

impl Sayac {
    pub fn yeni(tur: &'static str) -> Option<Sayac> {
        acik().then(|| Sayac { tur, baslangic: Instant::now(), cagri: BTreeMap::new(), yenileme: 0 })
    }
    /// Bir çağrı: ad yalnızca bilinen adlardan biriyse kaydedilir (araç adı bile serbest metin olarak yazılmaz)
    pub fn cagri(&mut self, ad: &str, hata: bool, sure_ms: f64) {
        let ad = BILINEN.iter().find(|b| **b == ad).copied().unwrap_or("diger");
        let e = self.cagri.entry(ad).or_insert((0, 0, 0.0, 0.0));
        e.0 += 1;
        e.1 += u64::from(hata);
        e.2 += sure_ms;
        e.3 = e.3.max(sure_ms);
    }
    pub fn yenilendi(&mut self) {
        self.yenileme += 1;
    }
    pub fn yaz(&self) {
        let Some(k) = klasor() else { return };
        let cagri: BTreeMap<&str, Value> = self
            .cagri
            .iter()
            .map(|(a, (n, h, t, m))| (*a, json!({ "adet": n, "hata": h, "ortalama_ms": ((t / *n as f64) * 100.0).round() / 100.0, "en_uzun_ms": (m * 100.0).round() / 100.0 })))
            .collect();
        let satir = json!({
            "zaman": crate::simdi(),
            "surum": env!("CARGO_PKG_VERSION"),
            "surec": self.tur,
            "calisma_sn": self.baslangic.elapsed().as_secs(),
            "cagri": cagri,
            "harita_yenileme": self.yenileme,
            "bellek_bayt": bellek(),
        });
        let _ = std::fs::create_dir_all(&k);
        let dosya = k.join(format!("{}.jsonl", &crate::simdi()[..10]));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dosya) {
            use std::io::Write;
            let _ = writeln!(f, "{satir}");
        }
    }
}

/// sezgi teshis [ac|kapat]
pub fn komut(args: &[String]) {
    let Some(k) = klasor() else {
        te!("✗ yapılandırma klasörü bulunamadı", "✗ config folder not found");
        std::process::exit(1)
    };
    match args.get(1).map(String::as_str) {
        Some("ac") => {
            std::fs::create_dir_all(&k).and_then(|_| std::fs::write(k.join("acik"), "")).unwrap_or_else(|e| {
                eprintln!("✗ {e}");
                std::process::exit(1)
            });
            tp!("✓ Tanılama kaydı açık: {}", "✓ Diagnostics log is on: {}", k.display());
            tp!(
                "  Yalnızca sayaçlar (çağrı sayısı, süre, hata, bellek); kod, sorgu, adres, kimlik ya da veri yok. Hiçbir yere gönderilmez.",
                "  Counters only (calls, durations, errors, memory); no code, queries, addresses, ids or data. Never sent anywhere."
            );
        }
        Some("kapat") => {
            let _ = std::fs::remove_file(k.join("acik"));
            tp!("✓ Tanılama kaydı kapalı (önceki kayıtlar {} içinde; silebilirsiniz).", "✓ Diagnostics log is off (earlier logs are in {}; you can delete them).", k.display());
        }
        _ => {
            tp!("Tanılama: {} · {}", "Diagnostics: {} · {}", if acik() { t!("açık", "on") } else { t!("kapalı", "off") }, k.display());
            let mut dosyalar: Vec<PathBuf> =
                std::fs::read_dir(&k).into_iter().flatten().flatten().map(|g| g.path()).filter(|p| p.extension().is_some_and(|e| e == "jsonl")).collect();
            dosyalar.sort();
            for d in dosyalar.iter().rev().take(3) {
                for l in std::fs::read_to_string(d).unwrap_or_default().lines().rev().take(5) {
                    let v: Value = serde_json::from_str(l).unwrap_or(Value::Null);
                    let cagri: Vec<String> = v["cagri"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(a, c)| tf!("{a} {} (hata {}, en uzun {} ms)", "{a} {} (errors {}, longest {} ms)", c["adet"], c["hata"], c["en_uzun_ms"]))
                        .collect();
                    tp!("  {} {} · {} sn · {}", "  {} {} · {} s · {}", v["zaman"].as_str().unwrap_or(""), v["surec"].as_str().unwrap_or(""), v["calisma_sn"], cagri.join(", "));
                }
            }
            tp!(
                "Bir hata bildirimine eklemek için bu klasördeki .jsonl dosyasını kendiniz ekleyin.",
                "To attach it to a bug report, add the .jsonl file from this folder yourself."
            );
        }
    }
}
