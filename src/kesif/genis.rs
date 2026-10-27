//! Kaynaksız keşfin REA ile geniş adımları ve **onay kapısı**:
//! - **Native ikili** (kullanıcının kurduğu Ghidra / IDA / Hopper; REA `doctor` ile algılanır): genel bakış (`inspect`)
//!   → fonksiyon ve sembol listesi (BINARY_METADATA, observed); giriş fonksiyonlarının decompile özeti (DECOMPILED,
//!   inferred; kod saklanmaz). Motor yoksa dürüst bilinmeyen. Ticari motor çıktısı paylaşılamaz.
//! - **Firmware** (REA Binwalk/Unblob ile, yalnızca Linux): bölüm ve imza envanteri (BINARY_HEURISTIC, inferred).
//! - **Süreç gözlemi** (`--calistir`): hedefin kendisi çalışır; RUNTIME_TRACE (yalıtım yoksa inferred). Windows'ta
//!   REA'nın süreç örneklemesi boş döndüğü için desteklenmez.
//! - **WebMCP keşfi** (`--webmcp <loopback CDP adresi>`): kullanıcının açtığı tarayıcıdaki sayfanın ilan ettiği araçlar
//!   (BROWSER_OBSERVATION; yalıtım yoksa inferred). Sezgi tarayıcı başlatmaz, uzak adrese bağlanmaz.
//!
//! Onay kapısı: otomatik olmayan her komut insanın terminalde "evet" yazmasını, `--onayla` ve yalıtım beyanını
//! (`--yalitim konteyner|vm|yok`) ister; dinamik komut ayrıca `--calistir` ve `--sahibiyim` ister. Terminal yoksa onay
//! verilemez: komut çalışmaz, sebebi bilinmeyen olarak yazılır.
use serde_json::{json, Value};
use std::io::{BufRead, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::masaustu::{bilinmeyen, kanit, Oge, Sonuc};
use super::rea::{self, Kopru, Onay, Sinif, Yalitim};
use super::rea_genis as g;

/// Keşfin geniş seçenekleri
#[derive(Default, Clone)]
pub(crate) struct Secenekler {
    pub rea: bool,
    pub onayla: bool,
    pub yalitim: Option<Yalitim>,
    pub calistir: bool,
    pub sahibiyim: bool,
    pub webmcp: Option<String>,
    pub koken: Option<String>,
}

/// Bir keşif boyunca köprü, motorlar ve verilen onaylar (her sınıf için en çok bir soru)
#[derive(Default)]
pub(crate) struct Baglam {
    pub sec: Secenekler,
    pub kopru: Option<Result<Kopru, String>>,
    motorlar: Option<Result<Vec<rea::Motor>, String>>,
    onaylar: Vec<(Sinif, Result<Onay, String>)>,
}

fn sure() -> Duration {
    Duration::from_secs(std::env::var("SEZGI_REA_SURE").ok().and_then(|s| s.parse().ok()).unwrap_or(120))
}

impl Baglam {
    pub fn yeni(sec: Secenekler) -> Baglam {
        Baglam { sec, ..Baglam::default() }
    }

    pub fn kopru(&mut self) -> Result<&Kopru, String> {
        self.kopru.get_or_insert_with(rea::ac).as_ref().map_err(Clone::clone)
    }

    /// Onay kapısı. `ne`: ne yapılacağı (insana gösterilir)
    pub fn onay(&mut self, komut: &str, ne: &str) -> Result<Onay, String> {
        let (s, b) = rea::sinif(komut)?;
        if s == Sinif::Otomatik {
            return Ok(Onay::kapidan(s, self.sec.yalitim.unwrap_or(Yalitim::Yok)));
        }
        if let Some((_, o)) = self.onaylar.iter().find(|(x, _)| *x >= s) {
            return o.clone();
        }
        let o = self.sor(s, b, komut, ne);
        self.onaylar.push((s, o.clone()));
        o
    }

    fn sor(&self, s: Sinif, b: u16, komut: &str, ne: &str) -> Result<Onay, String> {
        let gerek = if s == Sinif::Dinamik { "--onayla --yalitim konteyner|vm|yok --calistir --sahibiyim" } else { "--onayla --yalitim konteyner|vm|yok" };
        if !self.sec.onayla {
            return Err(format!("{} komut ({komut}: {}) onay ister: {gerek} ile ve terminalde \"evet\" yazarak", s.ad(), rea::etki_metni(b)));
        }
        let Some(y) = self.sec.yalitim else { return Err(format!("yalıtım beyanı yok: {gerek}")) };
        if s == Sinif::Dinamik && !(self.sec.calistir && self.sec.sahibiyim) {
            return Err(format!("dinamik komut ({komut}) hedefin kodunu çalıştırır: {gerek} gerekli (hedef size ait olmalı)"));
        }
        if !std::io::stdin().is_terminal() {
            return Err("onay terminalde \"evet\" yazılarak verilir; terminal yok (betikte onay verilemez)".into());
        }
        te!("REA {} komut: {ne}", "REA {} command: {ne}", s.gorunen());
        eprintln!("  {komut}: {}", rea::etki_gorunen(b));
        te!(
            "  Yalıtım beyanı: {}{}",
            "  Isolation declared: {}{}",
            y.ad(),
            if y == Yalitim::Yok { t!(" (gözlem en fazla inferred sayılır)", " (the observation counts as inferred at most)") } else { "" }
        );
        if s == Sinif::Dinamik {
            te!(
                "  Hedefin kendisi çalıştırılacak; hedefin size ait olduğunu ve yalıtılmış ortamda olduğunuzu beyan ettiniz.",
                "  The target itself will run; you declared that the target is yours and that you are in an isolated environment."
            );
        }
        if crate::dil::en() {
            eprint!("Do you approve? (type yes): ");
        } else {
            eprint!("Onaylıyor musunuz? (evet yazın): ");
        }
        let mut c = String::new();
        let _ = std::io::stdin().lock().read_line(&mut c);
        let cevap = c.trim().to_lowercase();
        if cevap == "evet" || (crate::dil::en() && cevap == "yes") {
            Ok(Onay::kapidan(s, y))
        } else {
            Err("insan onaylamadı".into())
        }
    }

    fn motorlar(&mut self) -> Result<Vec<rea::Motor>, String> {
        if self.motorlar.is_none() {
            let m = self.kopru().and_then(|k| k.motorlar());
            self.motorlar = Some(m);
        }
        self.motorlar.clone().unwrap_or_else(|| Err("motorlar okunamadı".into()))
    }
}

/// Kapılı bir REA komutunu çalıştırır: köprü, onay ve çıktı dosyası
pub(crate) fn calistir(b: &mut Baglam, komut: &str, argumanlar: &[&str], ne: &str) -> Result<(PathBuf, Onay), String> {
    b.kopru()?;
    let onay = b.onay(komut, ne)?;
    let k = b.kopru()?;
    let yol = k.calistir_kapili(komut, argumanlar, Some(&onay), sure())?;
    Ok((yol, onay))
}

fn adim(arac: &str, durum: &str, neden: Option<&str>) -> Value {
    let mut v = json!({ "arac": arac, "durum": durum });
    if let Some(n) = neden {
        v["neden"] = json!(n);
    }
    v
}

/// Native ikili: motor varsa genel bakış ve giriş fonksiyonlarının decompile özeti
pub(crate) fn native(b: &mut Baglam, oge: &Oge, sha: &str, s: &mut Sonuc) -> Vec<Value> {
    let konu = "native fonksiyonlar, semboller ve çağrılar";
    if !b.sec.rea {
        s.bilinmeyenler.push(bilinmeyen(oge, konu, "REA istenmedi (--rea; kullanıcının kurduğu Ghidra/IDA/Hopper gerekir)"));
        return vec![adim("inspect", "istenmedi", None)];
    }
    let motorlar = match b.motorlar() {
        Ok(m) => m,
        Err(e) => {
            s.bilinmeyenler.push(bilinmeyen(oge, konu, &format!("REA kapalı: {e}")));
            return vec![adim("inspect", "kapali", Some(&e))];
        }
    };
    let kurulu: Vec<&rea::Motor> = motorlar.iter().filter(|m| m.1 && matches!(m.0.as_str(), "ghidra" | "ida" | "hopper")).collect();
    if kurulu.is_empty() {
        let neden = "motor kurulu değil: Ghidra 12.1 (GHIDRA_INSTALL_DIR), IDA ya da Hopper; REA motor indirmez";
        s.bilinmeyenler.push(bilinmeyen(oge, konu, neden));
        return vec![adim("inspect", "motor_yok", Some(neden))];
    }
    let yol_metni = oge.yol.to_string_lossy().to_string();
    let mut adimlar = vec![];
    let ozet = match calistir(
        b,
        "inspect",
        &[&yol_metni],
        &format!("{} native ikili analizi (motor: {})", oge.goreli, kurulu.iter().map(|m| m.0.as_str()).collect::<Vec<_>>().join(", ")),
    )
    .and_then(|(y, _)| g::oku::<Value>(&y, "binary_overview"))
    {
        Ok(z) => {
            if z.sha256().is_some_and(|x| x != sha) {
                s.bilinmeyenler.push(bilinmeyen(oge, konu, "REA çıktısının yapıt özeti dosyayla aynı değil: kanıt alınmadı"));
                return vec![adim("inspect", "reddedildi", Some("yapıt özeti uyuşmuyor"))];
            }
            let ozet = g::genel_ozet(z.normalized_result.as_ref().unwrap_or(&Value::Null));
            let mut k = kanit(
                "BINARY_METADATA",
                "structure",
                z.guven(),
                z.authority.as_deref().unwrap_or("bilinmiyor"),
                &z.saglayici(),
                oge,
                sha,
                Some("inspect".into()),
                ozet.clone(),
                z.evidence_id.clone(),
            );
            paylasim(&mut k, z.paylasilabilir());
            s.kanitlar.push(k);
            adimlar.push(adim("inspect", "calisti", None));
            ozet
        }
        Err(e) => {
            s.bilinmeyenler.push(bilinmeyen(oge, konu, &e));
            return vec![adim("inspect", "calistirilmadi", Some(&e))];
        }
    };
    for f in g::giris_fonksiyonlari(&ozet, 3) {
        match calistir(b, "decompile", &[&yol_metni, "--procedure", &f], "giriş fonksiyonlarının decompile özeti").and_then(|(y, _)| g::oku::<Value>(&y, "procedure_pseudo_code"))
        {
            Ok(z) => {
                let o = g::decompile_ozeti(&f, z.normalized_result.as_ref().unwrap_or(&Value::Null));
                let mut k = kanit("DECOMPILED", "behavior", "inferred", "analyst-inference", &z.saglayici(), oge, sha, Some(format!("decompile:{f}")), o, z.evidence_id.clone());
                paylasim(&mut k, z.paylasilabilir());
                s.kanitlar.push(k);
                adimlar.push(adim("decompile", "calisti", None));
            }
            Err(e) => adimlar.push(adim("decompile", "calistirilmadi", Some(&e))),
        }
    }
    adimlar
}

/// Ticari motor çıktısı: karta ve yayımlanan kanıt paketine girmez
fn paylasim(k: &mut Value, paylasilabilir: bool) {
    if !paylasilabilir {
        k["paylasilabilir"] = json!(false);
        k["sinirlamalar"] = json!(["ticari analiz motorunun çıktısı: lisansı gereği paylaşılamaz; yalnızca yerel kanıt deposunda"]);
    }
}

/// Firmware: Binwalk/Unblob yapılandırılmışsa (yalnızca Linux) bölüm ve imza envanteri
pub(crate) fn firmware(b: &mut Baglam, oge: &Oge, sha: &str, s: &mut Sonuc) -> Vec<Value> {
    let konu = "firmware bölümleri ve dosya sistemleri";
    if !b.sec.rea {
        s.bilinmeyenler.push(bilinmeyen(oge, konu, "REA istenmedi (--rea; Binwalk 3.1 gerekir)"));
        return vec![adim("inspect-firmware-regions", "istenmedi", None)];
    }
    if !cfg!(target_os = "linux") {
        let neden = "firmware envanteri yalnızca Linux'ta (REA Binwalk/Unblob sağlayıcısı)";
        s.bilinmeyenler.push(bilinmeyen(oge, konu, neden));
        return vec![adim("inspect-firmware-regions", "desteklenmiyor", Some(neden))];
    }
    let motor = std::env::var_os("REA_BINWALK_COMMAND").map(PathBuf::from).filter(|p| p.is_absolute() && p.is_file());
    if motor.is_none() {
        let neden = "motor kurulu değil: REA_BINWALK_COMMAND (Binwalk 3.1, mutlak yol); REA motor indirmez";
        s.bilinmeyenler.push(bilinmeyen(oge, konu, neden));
        return vec![adim("inspect-firmware-regions", "motor_yok", Some(neden))];
    }
    let yol = oge.yol.to_string_lossy().to_string();
    match calistir(b, "inspect-firmware-regions", &[&yol], &format!("{} firmware bölüm envanteri (Binwalk, çıkarma yok)", oge.goreli))
        .and_then(|(y, _)| g::oku::<Value>(&y, "inspect_firmware_regions"))
    {
        Ok(z) => {
            let ozet = g::genel_ozet(z.normalized_result.as_ref().unwrap_or(&Value::Null));
            s.kanitlar.push(kanit(
                "BINARY_HEURISTIC",
                "structure",
                "inferred",
                z.authority.as_deref().unwrap_or("bilinmiyor"),
                &z.saglayici(),
                oge,
                sha,
                Some("inspect_firmware_regions".into()),
                ozet,
                z.evidence_id,
            ));
            vec![adim("inspect-firmware-regions", "calisti", None)]
        }
        Err(e) => {
            s.bilinmeyenler.push(bilinmeyen(oge, konu, &e));
            vec![adim("inspect-firmware-regions", "calistirilmadi", Some(&e))]
        }
    }
}

/// Süreç gözlemi: hedef yalıtılmış ortamda, kullanıcının beyanı ve onayıyla çalıştırılır
pub(crate) fn surec(b: &mut Baglam, oge: &Oge, sha: &str, s: &mut Sonuc) -> Vec<Value> {
    let konu = "çalışma zamanı davranışı (süreç, dosya etkileri)";
    if cfg!(windows) {
        let neden = "süreç gözlemi Windows'ta desteklenmiyor: REA'nın süreç ağacı örneklemesi Windows'ta boş döner";
        s.bilinmeyenler.push(bilinmeyen(oge, konu, neden));
        return vec![adim("capture-process", "desteklenmiyor", Some(neden))];
    }
    // Senaryo: hedef yalnızca geçici bir çalışma klasöründe, kısa süreyle çalışır; o klasörün dosya etkileri izlenir
    let Ok(klasor) = gecici_klasor() else {
        s.bilinmeyenler.push(bilinmeyen(oge, konu, "geçici çalışma klasörü oluşturulamadı"));
        return vec![adim("capture-process", "calistirilmadi", None)];
    };
    let calisma = klasor.join("calisma");
    let _ = std::fs::create_dir_all(&calisma);
    let hedef = std::fs::canonicalize(&oge.yol).unwrap_or_else(|_| oge.yol.clone());
    let senaryo = json!({
        "executable": hedef.to_string_lossy(),
        "arguments": [],
        "working_directory": calisma.to_string_lossy(),
        "filesystem_observation_paths": [calisma.to_string_lossy()],
        "timeout_ms": 10_000,
        "idle_timeout_ms": 5_000,
    });
    let senaryo_yolu = klasor.join("senaryo.json");
    let sonuc = crate::guvenli::atomik_yaz_gizli(&senaryo_yolu, senaryo.to_string().as_bytes()).and_then(|_| {
        let y = senaryo_yolu.to_string_lossy().to_string();
        calistir(b, "capture-process", &[&y], &format!("{} hedefin kendisi çalıştırılıp gözlenecek (en çok 10 sn)", oge.goreli))
            .and_then(|(p, onay)| g::oku::<g::Surec>(&p, "capture_process_scenario").map(|z| (z, onay)))
    });
    let _ = std::fs::remove_dir_all(&klasor);
    match sonuc {
        Ok((z, onay)) => {
            let ozet = g::surec_ozeti(z.normalized_result.as_ref().unwrap_or(&g::Surec::default()));
            let guven = if onay.yalitim == Yalitim::Yok { "inferred" } else { z.guven() };
            let mut k = kanit(
                "RUNTIME_TRACE",
                "behavior",
                guven,
                z.authority.as_deref().unwrap_or("bilinmiyor"),
                &z.saglayici(),
                oge,
                sha,
                Some("capture_process".into()),
                ozet,
                z.evidence_id.clone(),
            );
            k["yalitim"] = json!(onay.yalitim.ad());
            k["rea_yalitim"] = json!(z.rea_yalitim());
            k["sinirlamalar"] = json!([format!("yalıtım beyanı: {}; terminal çıktısı saklanmadı, yalnızca sayıldı", onay.yalitim.ad())]);
            s.kanitlar.push(k);
            vec![adim("capture-process", "calisti", None)]
        }
        Err(e) => {
            s.bilinmeyenler.push(bilinmeyen(oge, konu, &e));
            vec![adim("capture-process", "calistirilmadi", Some(&e))]
        }
    }
}

pub(crate) fn gecici_klasor() -> Result<PathBuf, String> {
    for i in 0..16u32 {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let p = std::env::temp_dir().join(format!("sezgi-surec-{}-{n}-{i}", std::process::id()));
        if std::fs::create_dir(&p).is_ok() {
            let _ = crate::guvenli::gizli_klasor(&p);
            return Ok(p);
        }
    }
    Err("geçici klasör oluşturulamadı".into())
}

/// CDP adresi yalnızca bu makine (loopback): uzak tarayıcıya bağlanılmaz
pub(crate) fn loopback_mu(uc: &str) -> bool {
    let govde = uc.strip_prefix("http://").or_else(|| uc.strip_prefix("ws://")).unwrap_or("");
    let ana = govde.split(['/', '?', '#']).next().unwrap_or("");
    let ana = ana.rsplit_once('@').map(|(_, h)| h).unwrap_or(ana);
    let host = if let Some(r) = ana.strip_prefix('[') { r.split(']').next().unwrap_or("") } else { ana.split(':').next().unwrap_or("") };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// WebMCP keşfi: kullanıcının açtığı tarayıcıdaki (loopback CDP) sayfanın ilan ettiği araçlar
pub(crate) fn webmcp(b: &mut Baglam, s: &mut Sonuc) -> Option<Oge> {
    let uc = b.sec.webmcp.clone()?;
    let koken = b.sec.koken.clone();
    let oge = Oge { yol: PathBuf::new(), goreli: koken.clone().unwrap_or_else(|| "web".into()), tur: "web" };
    let konu = "sayfanın ilan ettiği WebMCP araçları";
    if !loopback_mu(&uc) {
        s.bilinmeyenler.push(bilinmeyen(&oge, konu, "CDP adresi bu makinede değil (yalnızca http://127.0.0.1:<port>, localhost, [::1]): bağlanılmadı"));
        s.plan.push(json!({ "yol": oge.goreli, "tur": "web", "okuyucu": "yok", "rea": [adim("discover-webmcp-tools", "reddedildi", Some("loopback değil"))] }));
        return None;
    }
    let mut ek: Vec<&str> = vec![];
    if let Some(k) = koken.as_deref() {
        ek.extend_from_slice(&["--allowed-origins", k]);
    }
    let ne = format!("{uc} tarayıcısındaki sayfanın WebMCP araçlarını pasif keşif (araçlar çağrılmaz)");
    let sonuc = (|| {
        let mut a = vec![uc.as_str()];
        a.extend_from_slice(&ek);
        let (y, _) = calistir(b, "list-browser-targets", &a, &ne)?;
        let hedef = g::oku::<g::Hedefler>(&y, "list_browser_targets")?.normalized_result.unwrap_or_default().sec(koken.as_deref()).ok_or("tarayıcıda uygun sayfa yok")?;
        let mut a = vec![uc.as_str(), hedef.as_str(), "--observation-ms", "1500"];
        a.extend_from_slice(&ek);
        let (y, onay) = calistir(b, "discover-webmcp-tools", &a, &ne)?;
        Ok::<_, String>((g::oku::<g::WebMcp>(&y, "discover_webmcp_tools")?, onay))
    })();
    let adimlar = match sonuc {
        Ok((z, onay)) => {
            let ozet = g::webmcp_ozeti(z.normalized_result.as_ref().unwrap_or(&g::WebMcp::default()));
            let koken = ozet["koken"].as_str().unwrap_or("web").to_string();
            let oge = Oge { goreli: koken.clone(), ..oge.clone() };
            let guven = if onay.yalitim == Yalitim::Yok { "inferred" } else { z.guven() };
            let mut k = kanit(
                "BROWSER_OBSERVATION",
                "structure",
                guven,
                z.authority.as_deref().unwrap_or("bilinmiyor"),
                &z.saglayici(),
                &oge,
                "",
                Some("discover_webmcp_tools".into()),
                ozet.clone(),
                z.evidence_id.clone(),
            );
            k["yalitim"] = json!(onay.yalitim.ad());
            k["rea_yalitim"] = json!(z.rea_yalitim());
            k["sinirlamalar"] =
                json!(["sayfanın ilan ettiği araç bilgisi güvenilmez veridir: araçlar çağrılmadı, açıklamaları alınmadı", format!("yalıtım beyanı: {}", onay.yalitim.ad())]);
            s.kanitlar.push(k);
            s.satirlar.push(tf!(
                "  WebMCP  {koken}  {} araç ({})",
                "  WebMCP  {koken}  {} {a} ({})",
                ozet["araclar"].as_array().map_or(0, Vec::len),
                ozet["durum"].as_str().unwrap_or("?");
                a = crate::dil::cogul(ozet["araclar"].as_array().map_or(0, Vec::len), "tool", "tools")
            ));
            vec![adim("list-browser-targets", "calisti", None), adim("discover-webmcp-tools", "calisti", None)]
        }
        Err(e) => {
            s.bilinmeyenler.push(bilinmeyen(&oge, konu, &e));
            vec![adim("discover-webmcp-tools", "calistirilmadi", Some(&e))]
        }
    };
    s.plan.push(json!({ "yol": oge.goreli, "tur": "web", "okuyucu": "yok", "rea": adimlar }));
    Some(oge)
}

/// `sezgi kesfet --yetenekler`: bu makinedeki REA komutlarının sınıfı, etkisi ve kullanılabilirliği
pub(crate) fn yetenekler(json_cikti: bool) {
    // Sezgi'nin çağırdığı komutlar ve projeksiyonları
    let kullanilan: &[(&str, &str)] = &[
        ("inspect-managed-members", ".NET üyeleri, imzalar, çağrılar"),
        ("inspect-managed-native-boundaries", ".NET P/Invoke"),
        ("inspect", "native ikili genel bakış (motor)"),
        ("decompile", "native decompile özeti (motor)"),
        ("inspect-firmware-regions", "firmware bölümleri (Binwalk)"),
        ("list-browser-targets", "tarayıcı sayfaları (loopback CDP)"),
        ("discover-webmcp-tools", "WebMCP araçları (loopback CDP)"),
        ("capture-process", "süreç gözlemi (hedef çalışır)"),
    ];
    let kopru = rea::ac();
    let (motorlar, ilanlar) = match &kopru {
        Ok(k) => (k.motorlar().unwrap_or_default(), k.ilanlar().unwrap_or_default()),
        Err(_) => (vec![], vec![]),
    };
    let binwalk = std::env::var_os("REA_BINWALK_COMMAND").map(PathBuf::from).is_some_and(|p| p.is_absolute() && p.is_file());
    let motor_var = |ad: &str| motorlar.iter().any(|m| m.0 == ad && m.1);
    let native_motor = ["ghidra", "ida", "hopper"].iter().any(|m| motor_var(m));
    let satirlar: Vec<Value> = kullanilan
        .iter()
        .map(|(komut, ne)| {
            let (sinif, etki) = rea::sinif(komut).map(|(s, b)| (s.ad().to_string(), rea::etki_metni(b))).unwrap_or_else(|e| ("reddedildi".into(), e));
            let araclar = rea::komut_araclari(komut);
            let ilan = ilanlar.iter().find(|(a, _, _)| araclar.contains(&a.as_str()));
            let durum = if kopru.is_err() {
                "köprü kapalı".to_string()
            } else if matches!(*komut, "inspect" | "decompile") && !native_motor {
                "motor kurulu değil (Ghidra/IDA/Hopper)".into()
            } else if *komut == "inspect-firmware-regions" && !(binwalk && cfg!(target_os = "linux")) {
                "motor kurulu değil (REA_BINWALK_COMMAND, yalnızca Linux)".into()
            } else if *komut == "capture-process" && cfg!(windows) {
                "Windows'ta desteklenmiyor".into()
            } else {
                match ilan {
                    Some((_, false, n)) => format!("kullanılamaz: {n}"),
                    _ => "kullanılabilir".into(),
                }
            };
            json!({ "komut": komut, "ne": ne, "araclar": araclar, "sinif": sinif, "etki": etki, "durum": durum })
        })
        .collect();
    let mut sayim = std::collections::BTreeMap::new();
    for k in super::rea_tablo::CLI_KOMUTLARI {
        let s = rea::sinif(k).map(|(s, _)| s.ad()).unwrap_or("tablo dışı");
        *sayim.entry(s).or_insert(0usize) += 1;
    }
    let v = json!({
        "rea": match &kopru { Ok(k) => json!({ "durum": "hazir", "surum": rea::SURUM, "paket": crate::guvenli::gorunen(&k.paket) }), Err(e) => json!({ "durum": "kapali", "neden": e }) },
        "motorlar": motorlar.iter().map(|m| json!({ "ad": m.0, "kurulu": m.1, "ticari": m.2, "aciklama": m.3 })).collect::<Vec<_>>(),
        "komutlar": satirlar,
        "butun_komutlar": sayim,
        "izin_listesi": super::rea_tablo::ETKILER.iter().filter(|(a, _)| rea::izinli(a)).count(),
        "onay": "onaylı ve dinamik komutlar: --onayla --yalitim konteyner|vm|yok ve terminalde \"evet\"; dinamik ayrıca --calistir --sahibiyim",
    });
    if json_cikti {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return;
    }
    match &kopru {
        Ok(_) => {
            tp!("REA {}: hazır (sürüm, paket bütünlüğü ve araç etki tablosu doğrulandı)", "REA {}: ready (version, package integrity and tool effect table verified)", rea::SURUM)
        }
        Err(e) => tp!("REA: kapalı — {e}", "REA: off — {e}"),
    }
    if !motorlar.is_empty() {
        tp!("Analiz motorları:", "Analysis engines:");
        for m in &motorlar {
            println!("  {} {}{}", if m.1 { "✓" } else { "·" }, m.0, if m.2 { t!(" (ticari: çıktısı paylaşılamaz)", " (commercial: its output cannot be shared)") } else { "" });
        }
    }
    tp!("Sezgi'nin kullandığı REA komutları:", "REA commands Sezgi uses:");
    for r in v["komutlar"].as_array().into_iter().flatten() {
        let komut = r["komut"].as_str().unwrap_or("");
        // Ekranda seçilen dil; --json değerleri Türkçe kalır
        let (sinif, etki) = match rea::sinif(komut) {
            Ok((s, b)) => (s.gorunen().to_string(), rea::etki_gorunen(b)),
            Err(_) => (t!("reddedildi", "rejected").to_string(), r["etki"].as_str().unwrap_or("").to_string()),
        };
        let durum = match r["durum"].as_str().unwrap_or("") {
            "köprü kapalı" => t!("köprü kapalı", "bridge off").to_string(),
            "motor kurulu değil (Ghidra/IDA/Hopper)" => t!("motor kurulu değil (Ghidra/IDA/Hopper)", "engine not installed (Ghidra/IDA/Hopper)").to_string(),
            "motor kurulu değil (REA_BINWALK_COMMAND, yalnızca Linux)" => {
                t!("motor kurulu değil (REA_BINWALK_COMMAND, yalnızca Linux)", "engine not installed (REA_BINWALK_COMMAND, Linux only)").to_string()
            }
            "Windows'ta desteklenmiyor" => t!("Windows'ta desteklenmiyor", "not supported on Windows").to_string(),
            "kullanılabilir" => t!("kullanılabilir", "available").to_string(),
            x => match x.strip_prefix("kullanılamaz: ") {
                Some(n) => tf!("kullanılamaz: {n}", "unavailable: {n}"),
                None => x.to_string(),
            },
        };
        println!("  {:<34} {:<9} {}  [{}]", komut, sinif, durum, etki);
    }
    tp!(
        "Bütün REA {} komutları: {}",
        "All REA {} commands: {}",
        rea::SURUM,
        v["butun_komutlar"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, n)| {
                let k = match k.as_str() {
                    "otomatik" => t!("otomatik", "automatic"),
                    "onaylı" => t!("onaylı", "approval"),
                    "dinamik" => t!("dinamik", "dynamic"),
                    "tablo dışı" => t!("tablo dışı", "not in table"),
                    x => x,
                };
                format!("{k} {n}")
            })
            .collect::<Vec<_>>()
            .join(" · ")
    );
    tp!(
        "Onay: {}",
        "Approval: {}",
        t!(
            v["onay"].as_str().unwrap_or(""),
            "commands needing approval and dynamic commands: --onayla --yalitim konteyner|vm|yok and typing \"yes\" in the terminal; dynamic ones also --calistir --sahibiyim"
        )
    );
}

/// Dosya özeti (SHA-256), belleğe almadan
pub(crate) fn ozet(p: &Path) -> String {
    crate::kanit::dosya_ozeti(p).unwrap_or_default()
}
