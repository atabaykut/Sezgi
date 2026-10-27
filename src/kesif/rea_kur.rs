//! `sezgi rea kur | durum | kaldir`: sürümü sabitlenmiş REA'nın (npm `rea-agents` 6.7.0) Sezgi'nin yapılandırma
//! klasöründeki özel öneke (`<yapılandırma>/sezgi/rea`) kurulması, durumu ve kaldırılması.
//!
//! **Kurulum.** Önce ne kurulacağı gösterilir: paket, sürüm, lisans, kaynak, bütünlük, bağımlılık sayısı, hedef klasör
//! ve çalışacak komut. Terminalde açık "evet" yazılmadan hiçbir şey indirilmez ve yazılmaz; terminal yoksa kurulum
//! yapılmaz. Önek iki dosyayla hazırlanır: tam sürüme sabitli `package.json` ve Sezgi'nin içindeki kilit dosyası (bütün
//! bağımlılık ağacı, her paket sha512 bütünlüğüyle). npm `ci --prefix <önek> --ignore-scripts --no-audit --no-fund` ile
//! çalışır: kilit dosyasından sapamaz, paketlerin kurulum betikleri çalışmaz. Ağ yalnızca bu adımda, npm'in
//! yapılandırılmış kaydından kullanılır. Ardından köprünün sabitleme denetimleri (ad ve sürüm, npm bütünlüğü, etki
//! tablosu özeti, `rea --version` ve kullanılan araçların ilanı) bu önek üzerinde çalışır; npm hata verirse ya da
//! sabitleme tutmazsa kurulan silinir.
//!
//! **Kaldırma.** Yalnızca Sezgi'nin kurduğu önek silinir (adı `sezgi-rea` olan `package.json`); onay ister.
use serde_json::{json, Value};
use std::ffi::OsString;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::rea;
use crate::guvenli::gorunen;

/// Önekin `package.json` dosyası: tek bağımlılık, tam sürüm
const PAKET_JSON: &str = "{\n  \"name\": \"sezgi-rea\",\n  \"private\": true,\n  \"dependencies\": {\n    \"rea-agents\": \"6.7.0\"\n  }\n}\n";
/// Bütün bağımlılık ağacının kilit dosyası (npm lockfileVersion 3; her paket sha512 bütünlüğüyle)
const KILIT: &str = include_str!("rea_kilit.json");
const ONEK_ADI: &str = "sezgi-rea";
const LISANS: &str = "MIT";
const DEPO: &str = "https://github.com/morluto/rea";

fn dur(mesaj: &str, kod: i32) -> ! {
    eprintln!("✗ {mesaj}");
    std::process::exit(kod)
}

/// `sezgi rea <kur|durum|kaldir>`
pub(crate) fn komut(args: &[String]) {
    match args.get(1).map(String::as_str) {
        Some("kur") => kur(),
        Some("durum") => durum(args.iter().any(|a| a == "--json")),
        Some("kaldir") => kaldir(args),
        _ => crate::yardim::eksik("rea"),
    }
}

/// Önek Sezgi'nin mi: yok, boş ya da `package.json` adı `sezgi-rea`
fn onek_bizim(onek: &Path) -> bool {
    if !onek.exists() {
        return true;
    }
    if std::fs::read_dir(onek).is_ok_and(|mut d| d.next().is_none()) {
        return true;
    }
    std::fs::read_to_string(onek.join("package.json")).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).is_some_and(|v| v["name"] == ONEK_ADI)
}

/// npm: `SEZGI_NPM`, Node'un yanındaki npm (`npm-cli.js`, kabuksuz Node ile), ya da PATH. Dönüş: program ve baştaki
/// argümanlar
fn npm_bul(node: &Path) -> Option<(PathBuf, Vec<OsString>)> {
    let betikle = |p: PathBuf| -> (PathBuf, Vec<OsString>) {
        if p.extension().is_some_and(|e| e == "js") {
            (node.to_path_buf(), vec![p.into_os_string()])
        } else {
            (p, vec![])
        }
    };
    if let Some(v) = std::env::var_os("SEZGI_NPM").filter(|v| !v.is_empty()) {
        return Some(betikle(PathBuf::from(v)));
    }
    let mut klasorler: Vec<PathBuf> = node.parent().map(Path::to_path_buf).into_iter().collect();
    if let Some(d) = std::fs::canonicalize(node).ok().and_then(|g| g.parent().map(Path::to_path_buf)) {
        klasorler.push(d);
    }
    for d in klasorler {
        // Windows: <node>/node_modules/npm; Unix: <node>/../lib/node_modules/npm
        for aday in [d.join("node_modules/npm/bin/npm-cli.js"), d.join("../lib/node_modules/npm/bin/npm-cli.js")] {
            if aday.is_file() {
                return Some(betikle(aday));
            }
        }
    }
    rea::yolda(if cfg!(windows) { &["npm.cmd", "npm.exe"] } else { &["npm"] }).map(betikle)
}

fn evet_mi() -> bool {
    if crate::dil::en() {
        print!("Install it? (type yes): ");
    } else {
        print!("Kurulsun mu? (evet yazın): ");
    }
    let _ = std::io::stdout().flush();
    let mut c = String::new();
    let _ = std::io::stdin().lock().read_line(&mut c);
    let cevap = c.trim().to_lowercase();
    cevap == "evet" || (crate::dil::en() && cevap == "yes")
}

/// Kurulan önekte sabitleme denetimleri ve Sezgi'nin kullandığı araçların ilanı
fn dogrula(paket: PathBuf) -> Result<(), String> {
    let k = rea::ac_paketten(paket).map_err(|(_, e)| e)?;
    for a in rea::PROJEKSIYONLU {
        k.ilan_ediliyor(a)?;
    }
    Ok(())
}

fn yapilandirma_yok() -> &'static str {
    t!("yapılandırma klasörü bulunamadı (APPDATA / XDG_CONFIG_HOME / HOME tanımlı değil)", "config folder not found (APPDATA / XDG_CONFIG_HOME / HOME is not set)")
}

fn kur() {
    let onek = rea::onek().unwrap_or_else(|| dur(yapilandirma_yok(), 1));
    let paket = onek.join("node_modules").join("rea-agents");
    let Some(node) = rea::node_yolu().filter(|n| n.is_file()) else {
        dur(
            &tf!(
                "Node.js bulunamadı: REA {} Node.js {} ister (https://nodejs.org); yolu SEZGI_NODE ile de verebilirsiniz",
                "Node.js not found: REA {} needs Node.js {} (https://nodejs.org); you can also give its path with SEZGI_NODE",
                rea::SURUM,
                rea::NODE_SURUMLERI
            ),
            1,
        )
    };
    let node_surumu = rea::node_surumu(&node).unwrap_or_else(|e| dur(&format!("{e} → https://nodejs.org"), 1));
    if !onek_bizim(&onek) {
        dur(&tf!("{} Sezgi'nin kurduğu bir REA öneki değil: dokunulmadı", "{} is not a REA prefix installed by Sezgi: left untouched", gorunen(&onek)), 1);
    }
    // Zaten kurulu ve sabitleme tutuyorsa yeniden indirilmez
    let mevcut = paket.join("package.json").is_file().then(|| dogrula(paket.clone()));
    if let Some(Ok(())) = mevcut {
        tp!("✓ REA {} zaten kurulu ve doğrulandı: {}", "✓ REA {} is already installed and verified: {}", rea::SURUM, gorunen(&onek));
        return;
    }
    let Some((npm, npm_on)) = npm_bul(&node) else {
        dur(
            t!(
                "npm bulunamadı (Node.js ile birlikte gelir; yolu SEZGI_NPM ile de verebilirsiniz)",
                "npm not found (it comes with Node.js; you can also give its path with SEZGI_NPM)"
            ),
            1,
        )
    };
    let kilit: Value = serde_json::from_str(KILIT).unwrap_or_default();
    let bagimlilik = kilit["packages"].as_object().map(|p| p.len().saturating_sub(1)).unwrap_or(0);
    let komut_metni = |o: &Path| format!("npm ci --prefix {} --ignore-scripts --no-audit --no-fund", gorunen(o));
    tp!(
        "REA kurulumu (kaynak kodu olmayan .NET, native ve firmware analizi için isteğe bağlı araç)",
        "REA install (optional tool for analysing .NET, native and firmware without source code)"
    );
    tp!("  Paket      rea-agents {} ({LISANS}; {DEPO})", "  Package    rea-agents {} ({LISANS}; {DEPO})", rea::SURUM);
    tp!(
        "  Kaynak     npm kaydı (npm yapılandırmanızdaki kayıt; varsayılan registry.npmjs.org)",
        "  Source     npm registry (the registry in your npm config; default registry.npmjs.org)"
    );
    tp!("  Bütünlük   {}", "  Integrity  {}", rea::BUTUNLUK);
    tp!(
        "  Bağımlılık {bagimlilik} paket, her biri Sezgi'nin kilit dosyasında sha512 ile sabit",
        "  Deps       {bagimlilik} {p}, each pinned with sha512 in Sezgi's lock file";
        p = crate::dil::cogul(bagimlilik, "package", "packages")
    );
    tp!("  Hedef      {}", "  Target     {}", gorunen(&onek));
    tp!("  Komut      {}", "  Command    {}", komut_metni(&onek));
    println!("  Node.js    {node_surumu} ({})", gorunen(&node));
    if let Some(Err(e)) = &mevcut {
        tp!("  Mevcut     sabitleme tutmuyor ({e}): silinip yeniden kurulacak", "  Existing   pinning does not hold ({e}): it will be removed and reinstalled");
    }
    if std::env::var_os("SEZGI_REA").is_some_and(|v| !v.is_empty()) {
        tp!("  Not        SEZGI_REA tanımlı: köprü önce onu kullanır", "  Note       SEZGI_REA is set: the bridge uses it first");
    }
    tp!(
        "Kurulum betikleri çalıştırılmaz (--ignore-scripts); ağ yalnızca bu kurulumda kullanılır (yaklaşık 150 MB).",
        "Install scripts are not run (--ignore-scripts); the network is used only for this install (about 150 MB)."
    );
    tp!(
        "Kurulumdan sonra sürüm, paket bütünlüğü ve araç etki tablosu doğrulanır; tutmazsa kurulan silinir.",
        "After installing, the version, package integrity and tool effect table are verified; if they do not hold, the install is removed."
    );
    if !std::io::stdin().is_terminal() {
        dur(t!("onay terminalde \"evet\" yazılarak verilir; terminal yok: kurulmadı", "approval is given by typing \"yes\" in a terminal; no terminal: not installed"), 2);
    }
    if !evet_mi() {
        dur(t!("REA kurulmadı", "REA was not installed"), 2);
    }
    let sil = |neden: String| -> ! {
        let silindi = std::fs::remove_dir_all(&onek).is_ok() || !onek.exists();
        dur(
            &format!(
                "{neden}; {}",
                if silindi {
                    t!("kurulan silindi", "the install was removed")
                } else {
                    t!("kurulan silinemedi: sezgi rea kaldir --evet", "the install could not be removed: sezgi rea uninstall --yes")
                }
            ),
            1,
        )
    };
    if onek.exists() {
        if let Err(e) = std::fs::remove_dir_all(&onek) {
            dur(&tf!("mevcut kurulum silinemedi ({}): {e}", "could not remove the existing install ({}): {e}", gorunen(&onek)), 1);
        }
    }
    if let Err(e) =
        std::fs::create_dir_all(&onek).and_then(|_| std::fs::write(onek.join("package.json"), PAKET_JSON)).and_then(|_| std::fs::write(onek.join("package-lock.json"), KILIT))
    {
        sil(tf!("önek hazırlanamadı ({}): {e}", "could not prepare the prefix ({}): {e}", gorunen(&onek)));
    }
    tp!("{} çalışıyor...", "{} running...", komut_metni(&onek));
    // Kabuksuz; npm'in sürüm denetimi (ayrı bir ağ isteği) kapalı
    let sonuc = Command::new(&npm)
        .args(&npm_on)
        .args(["ci", "--prefix"])
        .arg(&onek)
        .args(["--ignore-scripts", "--no-audit", "--no-fund"])
        .env("npm_config_update_notifier", "false")
        .current_dir(&onek)
        .stdin(Stdio::null())
        .status();
    match sonuc {
        Ok(d) if d.success() => {}
        Ok(d) => sil(tf!("npm hata verdi ({d})", "npm failed ({d})")),
        Err(e) => sil(tf!("npm çalıştırılamadı ({}): {e}", "could not run npm ({}): {e}", gorunen(&npm))),
    }
    if let Err(e) = dogrula(paket) {
        sil(tf!("sabitleme tutmadı: {e}", "pinning does not hold: {e}"));
    }
    tp!(
        "✓ REA {} kuruldu ve doğrulandı (sürüm, paket bütünlüğü, araç etki tablosu, rea --version, araç ilanı): {}",
        "✓ REA {} installed and verified (version, package integrity, tool effect table, rea --version, tool announcement): {}",
        rea::SURUM,
        gorunen(&onek)
    );
    tp!(
        "  Kullanım: sezgi kesfet <klasör> --rea   ·   Durum: sezgi rea durum   ·   Kaldırma: sezgi rea kaldir",
        "  Usage: sezgi discover <folder> --rea   ·   Status: sezgi rea status   ·   Remove: sezgi rea uninstall"
    );
}

fn durum(json_cikti: bool) {
    let onek = rea::onek();
    let onek_kurulu = rea::onek_paketi().is_some();
    let konum = rea::konum();
    let (d, mesaj) = rea::durum();
    let v = json!({
        "durum": d,
        "surum": rea::SURUM,
        "butunluk": rea::BUTUNLUK,
        "onek": onek.as_deref().map(gorunen),
        "onek_kurulu": onek_kurulu,
        "kaynak": konum.as_ref().ok().map(|(_, k)| *k),
        "paket": konum.as_ref().ok().map(|(p, _)| gorunen(p)),
        "mesaj": mesaj,
    });
    if json_cikti {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    } else {
        tp!("REA (rea-agents {}, sabitlenmiş)", "REA (rea-agents {}, pinned)", rea::SURUM);
        tp!(
            "  Önek       {} ({})",
            "  Prefix     {} ({})",
            onek.as_deref().map(gorunen).unwrap_or_else(|| t!("yapılandırma klasörü yok", "no config folder").into()),
            if onek_kurulu { t!("kurulu", "installed") } else { t!("kurulu değil", "not installed") }
        );
        if let Ok((p, k)) = &konum {
            tp!("  Kullanılan {} ({})", "  In use     {} ({})", gorunen(p), if *k == rea::KAYNAK_ONEK { t!("Sezgi'nin öneki", "Sezgi's prefix") } else { k });
        }
        tp!("  Bütünlük   {}", "  Integrity  {}", rea::BUTUNLUK);
        if d == "hazir" {
            tp!("✓ Köprü hazır: {mesaj}", "✓ Bridge ready: {mesaj}");
        } else {
            tp!("✗ Köprü kapalı: {mesaj}", "✗ Bridge off: {mesaj}");
            if !mesaj.contains("sezgi rea kur") {
                tp!("  Kurmak ya da onarmak için: sezgi rea kur", "  To install or repair: sezgi rea install");
            }
        }
    }
    if d != "hazir" {
        std::process::exit(1);
    }
}

fn kaldir(args: &[String]) {
    let onek = rea::onek().unwrap_or_else(|| dur(yapilandirma_yok(), 1));
    if !onek.exists() {
        tp!("REA Sezgi'nin önekinde kurulu değil ({})", "REA is not installed in Sezgi's prefix ({})", gorunen(&onek));
        return;
    }
    if !onek_bizim(&onek) {
        dur(&tf!("{} Sezgi'nin kurduğu bir REA öneki değil: silinmedi", "{} is not a REA prefix installed by Sezgi: not removed", gorunen(&onek)), 1);
    }
    if !crate::baglan::onaylandi(args, &tf!("REA öneki silinsin mi? ({})", "Remove the REA prefix? ({})", gorunen(&onek))) {
        dur(t!("Silinmedi. Terminal dışında onay için: sezgi rea kaldir --evet", "Not removed. To approve outside a terminal: sezgi rea uninstall --yes"), 2);
    }
    match std::fs::remove_dir_all(&onek) {
        Ok(()) => tp!("✓ REA kaldırıldı: {}", "✓ REA removed: {}", gorunen(&onek)),
        Err(e) => dur(&tf!("{} silinemedi: {e}", "could not remove {}: {e}", gorunen(&onek)), 1),
    }
}
