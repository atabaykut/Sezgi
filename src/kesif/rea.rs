//! REA köprüsü: kaynak kodu olmayan .NET ikilileri için isteğe bağlı, sürümü sabitlenmiş, fail-closed bir dış kanıt
//! sağlayıcısı (npm `rea-agents`). Sezgi REA'yı paketlemez; kullanıcı `sezgi rea kur` ile Sezgi'nin yapılandırma
//! klasöründeki özel öneke kurar (`rea_kur`). Yalnızca CLI ve JSON dosyası kullanılır (MCP yok).
//!
//! **Arama sırası.** `SEZGI_REA` (açıkça verilen yol) → Sezgi'nin öneki (`<yapılandırma>/sezgi/rea`) → PATH. Nerede
//! bulunursa bulunsun aynı sabitleme denetimleri çalışır.
//!
//! **Sabitleme.** Sürüm dizgisi tek başına yetmez. Köprü şu dördü tutmadan hiçbir REA kodu çalıştırmaz ve sebebini
//! açıkça söyleyerek kapalı kalır:
//! 1. kurulu paketin `package.json` adı ve sürümü,
//! 2. npm'in kurulum kaydındaki paket bütünlüğü (`integrity`, sha512),
//! 3. araç etki tablosu dosyasının (`TOOL_EFFECTS`) SHA-256 özeti,
//! 4. ardından `rea --version` çıktısı ve `rea capabilities` içinde kullanılacak aracın ilanı.
//!
//! **İzin listesi ve sınıflar.** Sabitlenen etki tablosundan (`rea_tablo`) her CLI komutunun ulaştığı araçların etkileri
//! birleştirilir (motor oturumu açan komutlarda oturum açmanın etkisi de eklenir):
//! - **otomatik:** yalnızca oturuma kanıt kaydı (hedefi, dosya sistemini, ağı, süreçleri değiştirmeyen; yabancı kod
//!   çalıştırmayan). `--rea` ile çalışır.
//! - **onaylı:** süreç başlatan, dosya sistemine yazan ya da ağa (loopback tarayıcı dahil) erişen. Ancak insanın açık
//!   onayıyla (`--onayla`, terminalde "evet") ve yalıtım beyanıyla (`--yalitim konteyner|vm|yok`) çalışır.
//! - **dinamik:** hedef kodu çalıştıran ya da hedefi değiştiren. Onaylının şartlarına ek olarak `--calistir` ve hedefin
//!   kullanıcıya ait olduğu beyanı (`--sahibiyim`) ister.
//! - Tabloda olmayan komut ya da araç reddedilir. Sezgi yalnızca çıktısının biçim projeksiyonunu bildiği komutları çağırır.
//!
//! **Çalıştırma.** Alt süreç kabuk olmadan (`node <betik> <araç> <yapıt> --format json`), zaman ve bellek sınırıyla
//! başlar; standart çıktı geçici bir dosyaya yazılır ve akışla okunur. Sınır aşılırsa süreç sonlandırılır (iptal).
//! Ham çıktı saklanmaz: okunduktan sonra dosya silinir.
//!
//! **Dönüştürme.** Yalnızca biçim alınır (tip, üye, imza, çağrı hedefi, P/Invoke adı); dize literallerinin değeri
//! alınmaz, yalnızca SQL biçimi (işlem ve tablo adları) çıkarılır. REA `confidence`/`authority` eksik ya da tanınmazsa
//! en zayıf değer sayılır. Bütün REA kanıtları `kapsam: acik` taşır.
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Sabitlenen sürüm
pub(crate) const SURUM: &str = "6.7.0";
/// npm kaydındaki paket bütünlüğü (rea-agents-6.7.0.tgz)
pub(crate) const BUTUNLUK: &str = "sha512-yhIx2ioJYYT1C/tONfYeitC6LhnKmCh+ZwRUOgYuhSc733lGhkkt73zhug9WmLgxoMdKuqIYGCoBfD4fXkOhFw==";
/// `dist/contracts/toolEffects.js` dosyasının SHA-256 özeti (araç etki tablosu)
pub(crate) const ETKI_TABLOSU_OZETI: &str = "726c2b2340c9baf5606d5eb1fe3bd807d353f4867135245b93bb843a9a2c7b70";
const ETKI_TABLOSU: &str = "dist/contracts/toolEffects.js";

/// Doğrulanana kadar otomatik çalışmayan araçlar (etkileri yalnızca oturum olsa da)
const OTOMATIK_DISI: &[&str] = &["trace_native_ui_action", "trace_native_values"];

/// Etki bitleri (`rea_tablo::ETKILER`)
const HEDEF: u16 = 1;
const OTURUM: u16 = 1 << 1;
const DOSYA: u16 = 1 << 2;
const SUREC: u16 = 1 << 3;
const AG: u16 = 1 << 4;
const ARAYUZ: u16 = 1 << 5;
const SILME: u16 = 1 << 6;
const YABANCI_KOD: u16 = 1 << 7;
const CIHAZ: u16 = 1 << 8;
const ESGUCLU: u16 = 1 << 9;

/// Aracın sabitlenen etkileri (tabloda yoksa `None`)
pub(crate) fn etki(arac: &str) -> Option<u16> {
    super::rea_tablo::ETKILER.iter().find(|(a, _)| *a == arac).map(|(_, b)| *b)
}

/// İzin listesi: etkisi yalnızca oturuma kanıt kaydı olan araçlar (sabitlenen tablodan 85 aday; doğrulanmayan ikisi
/// hariç 83)
pub(crate) fn izinli(arac: &str) -> bool {
    etki(arac) == Some(OTURUM | ESGUCLU) && !OTOMATIK_DISI.contains(&arac)
}

/// Komutun sınıfı
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Sinif {
    Otomatik,
    Onayli,
    Dinamik,
}

impl Sinif {
    pub fn ad(self) -> &'static str {
        match self {
            Sinif::Otomatik => "otomatik",
            Sinif::Onayli => "onaylı",
            Sinif::Dinamik => "dinamik",
        }
    }

    /// Ekranda seçilen dildeki adı (`ad` kayıtlarda ve --json çıktısında Türkçe kalır)
    pub fn gorunen(self) -> &'static str {
        match self {
            Sinif::Otomatik => t!("otomatik", "automatic"),
            Sinif::Onayli => t!("onaylı", "approval"),
            Sinif::Dinamik => t!("dinamik", "dynamic"),
        }
    }
}

/// Analiz motoru oturumu açan komutlar: kendi etkilerine motor sürecini başlatmanın (`open_binary`) etkisi eklenir
const MOTOR_OTURUMU: &[&str] = &["analyze", "inspect", "decompile", "function", "instructions", "search", "xrefs", "trace"];

/// CLI komutunun ulaştığı araçlar: takma ad tablosundan ya da adın alt çizgili hâlinden (tabloda yoksa boş)
pub(crate) fn komut_araclari(komut: &str) -> Vec<&'static str> {
    if let Some((_, a)) = super::rea_tablo::CLI_TAKMA_ADLARI.iter().find(|(k, _)| *k == komut) {
        return a.to_vec();
    }
    let ad = komut.replace('-', "_");
    super::rea_tablo::ETKILER.iter().filter(|(a, _)| *a == ad).map(|(a, _)| *a).collect()
}

/// Komutun birleşik etkisi ve sınıfı; komut ya da aracı sabitlenen tabloda yoksa reddedilir
pub(crate) fn sinif(komut: &str) -> Result<(Sinif, u16), String> {
    if !super::rea_tablo::CLI_KOMUTLARI.contains(&komut) {
        return Err(format!("'{komut}' sabitlenen REA {SURUM} komutları arasında yok: reddedildi"));
    }
    let araclar = komut_araclari(komut);
    if araclar.is_empty() {
        return Err(format!("'{komut}' komutunun aracı REA araç etki tablosunda yok: reddedildi"));
    }
    let mut b = araclar.iter().filter_map(|a| etki(a)).fold(0u16, |x, y| x | y);
    if MOTOR_OTURUMU.contains(&komut) {
        b |= etki("open_binary").unwrap_or(SUREC);
    }
    let s = if b & (HEDEF | YABANCI_KOD | CIHAZ | SILME | ARAYUZ) != 0 {
        Sinif::Dinamik
    } else if b & (DOSYA | SUREC | AG) != 0 || araclar.iter().any(|a| !izinli(a)) {
        Sinif::Onayli
    } else {
        Sinif::Otomatik
    };
    Ok((s, b))
}

/// Etkilerin insan dilinde özeti
pub(crate) fn etki_metni(b: u16) -> String {
    let adlar = [
        (HEDEF, "hedefi değiştirir"),
        (DOSYA, "dosya sistemine yazar"),
        (SUREC, "süreç başlatır"),
        (AG, "ağa (loopback dahil) erişir"),
        (ARAYUZ, "arayüz durumunu değiştirir"),
        (SILME, "veri silebilir"),
        (YABANCI_KOD, "yabancı kod çalıştırır"),
        (CIHAZ, "cihazı değiştirir"),
    ];
    let v: Vec<&str> = adlar.iter().filter(|(x, _)| b & x != 0).map(|(_, a)| *a).collect();
    if v.is_empty() {
        "yalnızca oturuma kanıt kaydeder".into()
    } else {
        v.join(", ")
    }
}

/// `etki_metni`nin ekranda seçilen dildeki karşılığı (kayıtlarda ve --json çıktısında `etki_metni` kullanılır)
pub(crate) fn etki_gorunen(b: u16) -> String {
    if !crate::dil::en() {
        return etki_metni(b);
    }
    let adlar = [
        (HEDEF, "changes the target"),
        (DOSYA, "writes to the file system"),
        (SUREC, "starts processes"),
        (AG, "accesses the network (loopback included)"),
        (ARAYUZ, "changes UI state"),
        (SILME, "may delete data"),
        (YABANCI_KOD, "runs foreign code"),
        (CIHAZ, "changes the device"),
    ];
    let v: Vec<&str> = adlar.iter().filter(|(x, _)| b & x != 0).map(|(_, a)| *a).collect();
    if v.is_empty() {
        "only records evidence in the session".into()
    } else {
        v.join(", ")
    }
}

/// Yalıtım beyanı: yalıtımsız gözlem en fazla inferred
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Yalitim {
    Konteyner,
    Vm,
    Yok,
}

impl Yalitim {
    pub fn coz(s: &str) -> Option<Yalitim> {
        match s {
            "konteyner" | "container" => Some(Yalitim::Konteyner),
            "vm" => Some(Yalitim::Vm),
            "yok" | "none" => Some(Yalitim::Yok),
            _ => None,
        }
    }
    pub fn ad(self) -> &'static str {
        match self {
            Yalitim::Konteyner => "konteyner",
            Yalitim::Vm => "vm",
            Yalitim::Yok => "yok",
        }
    }
}

/// İnsan onayının kanıtı. Yalnızca onay kapısı (`kesif::masaustu`) üretir: terminalde "evet" yazılmadan, yalıtım
/// beyanı olmadan (dinamikte ayrıca `--calistir` ve `--sahibiyim` olmadan) oluşmaz
#[derive(Debug, Clone, Copy)]
pub(crate) struct Onay {
    pub sinif: Sinif,
    pub yalitim: Yalitim,
    _kapi: (),
}

impl Onay {
    /// Onay kapısı: terminalde verilen onaydan sonra çağrılır
    pub(crate) fn kapidan(sinif: Sinif, yalitim: Yalitim) -> Onay {
        Onay { sinif, yalitim, _kapi: () }
    }
}

/// Algılanan analiz motorları (REA `doctor`): (ad, kurulu mu, ticari mi, açıklama)
pub(crate) type Motor = (String, bool, bool, String);

/// Ticari motorlar: çıktıları paylaşılamaz (karta ve yayımlanan pakete girmez)
pub(crate) const TICARI: &[&str] = &["ida", "hopper", "jeb"];

/// Çıktısı ticari bir motordan geliyor mu (sağlayıcı kimliğinden)
pub(crate) fn ticari_mi(saglayici: &str) -> bool {
    let s = saglayici.to_lowercase();
    TICARI.iter().any(|t| s.contains(t))
}

/// Sezgi'nin çıktı projeksiyonunu bildiği araçlar (izin listesinin alt kümesi): yalnızca bunlar çağrılır
pub(crate) const PROJEKSIYONLU: &[&str] = &["inspect_managed_members", "inspect_managed_native_boundaries"];

/// Çıktı dosyasının en büyük boyu; aşılırsa süreç sonlandırılır
const EN_BUYUK_CIKTI: u64 = 512 << 20;

/// Doğrulanmış (sabitlenmiş) REA kurulumu
#[derive(Debug)]
pub(crate) struct Kopru {
    node: PathBuf,
    betik: PathBuf,
    pub paket: PathBuf,
    /// `rea capabilities` çıktısı (bir kez okunur)
    yetenekler: std::cell::OnceCell<Result<Value, String>>,
}

/// Aracın çalıştırılabilir olup olmadığı (izin listesi ve projeksiyon)
pub(crate) fn arac_izinli(arac: &str) -> Result<&'static str, String> {
    let ad = arac.replace('-', "_");
    let Some(a) = super::rea_tablo::ETKILER.iter().map(|(a, _)| *a).find(|a| *a == ad && izinli(a)) else {
        return Err(format!("'{arac}' REA araç etki tablosunun izin listesinde değil (yalnızca oturuma kanıt kaydeden araçlar çalışır)"));
    };
    if !PROJEKSIYONLU.contains(&a) {
        return Err(format!("'{arac}' için Sezgi'de çıktı projeksiyonu yok: çağrılmaz"));
    }
    Ok(a)
}

/// PATH'te ya da verilen ortam değişkeninde çalıştırılabilir dosya
fn bul(degisken: &str, adlar: &[&str]) -> Option<PathBuf> {
    if let Some(v) = std::env::var_os(degisken).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(v));
    }
    yolda(adlar)
}

/// PATH'teki ilk çalıştırılabilir dosya
pub(crate) fn yolda(adlar: &[&str]) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).flat_map(|d| adlar.iter().map(move |a| d.join(a))).find(|p| p.is_file())
}

/// Node: `SEZGI_NODE` ya da PATH
pub(crate) fn node_yolu() -> Option<PathBuf> {
    bul("SEZGI_NODE", if cfg!(windows) { &["node.exe"] } else { &["node"] })
}

/// Sezgi'nin REA öneki: `<yapılandırma>/sezgi/rea` (`sezgi rea kur` buraya kurar)
pub(crate) fn onek() -> Option<PathBuf> {
    crate::imza::yapilandirma_klasoru().map(|k| k.join("rea"))
}

/// Önekteki paket klasörü (kurulu değilse `None`)
pub(crate) fn onek_paketi() -> Option<PathBuf> {
    onek().map(|o| o.join("node_modules").join("rea-agents")).filter(|p| p.join("package.json").is_file())
}

fn oku_sinirli(p: &Path, sinir: u64) -> Option<String> {
    let m = std::fs::metadata(p).ok()?;
    if !m.is_file() || m.len() > sinir {
        return None;
    }
    std::fs::read_to_string(p).ok()
}

/// Paketin kökü: `rea` yolundan (sembolik bağ, npm `.bin` kısayolu ya da genel kurulum) `rea-agents` klasörü
fn paket_koku(rea: &Path) -> Option<PathBuf> {
    let adi_dogru = |d: &Path| oku_sinirli(&d.join("package.json"), 1 << 20).and_then(|s| serde_json::from_str::<Value>(&s).ok()).is_some_and(|v| v["name"] == "rea-agents");
    let mut adaylar: Vec<PathBuf> = Vec::new();
    if rea.is_dir() {
        adaylar.push(rea.to_path_buf());
    }
    if let Ok(g) = std::fs::canonicalize(rea) {
        adaylar.extend(g.ancestors().take(4).map(Path::to_path_buf));
    }
    if let Some(d) = rea.parent() {
        adaylar.push(d.join("node_modules").join("rea-agents"));
        adaylar.push(d.join("..").join("rea-agents"));
        adaylar.push(d.join("..").join("lib").join("node_modules").join("rea-agents"));
    }
    adaylar.into_iter().find(|d| adi_dogru(d)).map(|d| std::fs::canonicalize(&d).unwrap_or(d))
}

/// npm kurulum kaydındaki bütünlük değeri (gizli kilit dosyası ya da projenin kilit dosyası)
fn kayitli_butunluk(paket: &Path) -> Option<String> {
    let ust = paket.parent()?;
    for kilit in [ust.join(".package-lock.json"), ust.parent()?.join("package-lock.json")] {
        let Some(v) = oku_sinirli(&kilit, 64 << 20).and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { continue };
        let kayit = v["packages"].as_object().and_then(|p| p.iter().find(|(k, _)| k.ends_with("node_modules/rea-agents")).map(|(_, d)| d.clone()));
        if let Some(d) = kayit.filter(|d| d["version"] == SURUM) {
            return d["integrity"].as_str().map(String::from);
        }
    }
    None
}

/// Köprü kapalıysa sebebin türü: kurulu değil, sürüm uyuşmuyor, sabitleme (bütünlük, etki tablosu) tutmuyor, çalışmadı
pub(crate) const YOK: &str = "yok";
pub(crate) const SURUM_UYUSMUYOR: &str = "surum";
pub(crate) const SABITLEME: &str = "sabitleme";
pub(crate) const CALISMADI: &str = "calismadi";

/// Köprüyü açar: sabitlemenin her adımı tutmazsa hiçbir şey çalıştırmadan sebebiyle kapalı kalır
pub(crate) fn ac() -> Result<Kopru, String> {
    ac_turlu().map_err(|(_, e)| e)
}

/// `sezgi doktor` için: (durum, açıklama). Durum: hazir, yok, surum, sabitleme, calismadi. Ağa çıkılmaz; sabitleme
/// tutarsa yalnızca yerel `node --version` ve `rea --version` çalışır.
pub(crate) fn durum() -> (&'static str, String) {
    match ac_turlu() {
        Ok(k) => (
            "hazir",
            tf!(
                "rea-agents {SURUM} hazır ({}; sürüm, paket bütünlüğü ve araç etki tablosu doğrulandı)",
                "rea-agents {SURUM} ready ({}; version, package integrity and tool effect table verified)",
                crate::guvenli::gorunen(&k.paket)
            ),
        ),
        Err((t, e)) => (t, e),
    }
}

/// REA'nın nereden bulunduğu
pub(crate) const KAYNAK_DEGISKEN: &str = "SEZGI_REA";
pub(crate) const KAYNAK_ONEK: &str = "onek";
pub(crate) const KAYNAK_PATH: &str = "PATH";

/// Kurulu paketin kökü ve nereden bulunduğu: `SEZGI_REA` → Sezgi'nin öneki → PATH
pub(crate) fn konum() -> Result<(PathBuf, &'static str), (&'static str, String)> {
    let yok = |e: String| (YOK, e);
    if let Some(v) = std::env::var_os("SEZGI_REA").filter(|v| !v.is_empty()) {
        let rea = PathBuf::from(v);
        let paket = paket_koku(&rea).ok_or_else(|| yok(format!("SEZGI_REA rea-agents paketini göstermiyor ({})", crate::guvenli::gorunen(&rea))))?;
        return Ok((paket, KAYNAK_DEGISKEN));
    }
    if let Some(p) = onek_paketi() {
        return Ok((p, KAYNAK_ONEK));
    }
    let adlar: &[&str] = if cfg!(windows) { &["rea.cmd", "rea.exe", "rea"] } else { &["rea"] };
    let rea = yolda(adlar).ok_or_else(|| yok("rea bulunamadı (Sezgi'nin önekinde ve PATH'te yok) → kurmak için: sezgi rea kur (ya da yolu SEZGI_REA ile verin)".into()))?;
    let paket = paket_koku(&rea).ok_or_else(|| yok(format!("rea-agents paketi bulunamadı ({}): npm paketinden kurulmuş olmalı → sezgi rea kur", crate::guvenli::gorunen(&rea))))?;
    Ok((paket, KAYNAK_PATH))
}

fn ac_turlu() -> Result<Kopru, (&'static str, String)> {
    let (paket, kaynak) = konum()?;
    let kopru = ac_paketten(paket)?;
    // Sabitleme denetimleri sürümü tanır, kurulumdan sonra değiştirilmiş dosyaları değil: Sezgi'nin kendi kurduğu
    // klasörün dışındaki (SEZGI_REA ya da PATH) bir REA'nın klasörünü başka bir programın yazamadığı kullanıcıya söylenir
    if kaynak != KAYNAK_ONEK {
        eprintln!(
            "{}",
            tf!(
                "! REA Sezgi'nin kurduğu klasörden değil ({kaynak}: {}): dosyalarının içeriği doğrulanmadı; bu klasörü başka bir program (ör. bir kodlama ajanı) yazamamalı",
                "! REA is not from the folder Sezgi installed ({kaynak}: {}): its file contents are not verified; no other program (such as a coding agent) should be able to write that folder",
                crate::guvenli::gorunen(&kopru.paket)
            )
        );
    }
    Ok(kopru)
}

/// Verilen paket klasöründe sabitleme denetimleri; hepsi tutarsa köprü (yalnızca yerel `node --version` ve `rea
/// --version` çalışır)
pub(crate) fn ac_paketten(paket: PathBuf) -> Result<Kopru, (&'static str, String)> {
    let yok = |e: String| (YOK, e);
    let pj: Value = oku_sinirli(&paket.join("package.json"), 1 << 20).and_then(|s| serde_json::from_str(&s).ok()).ok_or_else(|| yok("rea-agents package.json okunamadı".into()))?;
    if pj["version"] != SURUM {
        return Err((SURUM_UYUSMUYOR, format!("rea-agents sürümü {} (beklenen {SURUM}): sürüm sabitlemesi tutmuyor", pj["version"].as_str().unwrap_or("?"))));
    }
    match kayitli_butunluk(&paket) {
        Some(b) if b == BUTUNLUK => {}
        Some(_) => return Err((SABITLEME, "rea-agents paket bütünlüğü (npm integrity) beklenen değerle aynı değil".into())),
        None => return Err((SABITLEME, "rea-agents için npm kurulum kaydı (integrity) bulunamadı: paket npm ile yerel bir klasöre kurulmalı".into())),
    }
    let etki = std::fs::read(paket.join(ETKI_TABLOSU)).map_err(|_| (SABITLEME, "REA araç etki tablosu bulunamadı".to_string()))?;
    if crate::benlik::sha256_hex(&etki) != ETKI_TABLOSU_OZETI {
        return Err((SABITLEME, "REA araç etki tablosunun özeti sabitlenen değerle aynı değil: izin listesi geçersiz".into()));
    }
    let betik_goreli = pj["bin"]["rea"].as_str().ok_or((SABITLEME, "rea-agents package.json'da rea betiği yok".to_string()))?;
    if Path::new(betik_goreli).components().any(|c| !matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir)) {
        return Err((SABITLEME, "rea betik yolu paketin dışını gösteriyor".into()));
    }
    let betik = paket.join(betik_goreli);
    let node = node_yolu().ok_or_else(|| yok(format!("Node.js bulunamadı (REA {SURUM} Node.js {NODE_SURUMLERI} ister; yolu SEZGI_NODE ile verin)")))?;
    // Sabitleme tuttu: ancak şimdi Node ve REA çalıştırılır
    node_surumu(&node).map_err(|e| (SURUM_UYUSMUYOR, e))?;
    let kopru = Kopru { node, betik, paket, yetenekler: std::cell::OnceCell::new() };
    let surum = kopru.metin_cikti(&["--version"], Duration::from_secs(30)).map_err(|e| (CALISMADI, e))?;
    if surum.trim() != SURUM {
        return Err((SURUM_UYUSMUYOR, format!("rea --version {} diyor (beklenen {SURUM})", crate::guvenli::temiz(surum.trim()))));
    }
    Ok(kopru)
}

/// Sabitlenen paketin `engines` alanı (`^22.19.0 || ^24.11.0 || >=26.0.0`), insan diliyle
pub(crate) const NODE_SURUMLERI: &str = "22.19+, 24.11+ ya da 26+";

/// Node sürümü sabitlenen paketin `engines` alanına uyuyor mu: 22.19+ (23 değil), 24.11+ (25 değil) ya da 26+
pub(crate) fn node_uygun(surum: &str) -> bool {
    let mut p = surum.trim().trim_start_matches('v').split('.').map(|x| x.parse::<u32>().unwrap_or(0));
    let (ana, ara) = (p.next().unwrap_or(0), p.next().unwrap_or(0));
    (ana == 22 && ara >= 19) || (ana == 24 && ara >= 11) || ana >= 26
}

/// `node --version`; uygun değilse sebebiyle hata. Başarıda sürüm metni
pub(crate) fn node_surumu(node: &Path) -> Result<String, String> {
    let c = Command::new(node).arg("--version").stdin(Stdio::null()).stderr(Stdio::null()).output().map_err(|e| format!("node çalıştırılamadı: {e}"))?;
    let s = crate::guvenli::temiz(String::from_utf8_lossy(&c.stdout).trim());
    if node_uygun(&s) {
        Ok(s)
    } else {
        Err(format!("Node {s} REA {SURUM} için uygun değil ({NODE_SURUMLERI})"))
    }
}

/// Geçici dosya (yalnızca kullanıcıya açık, yeni oluşturulur)
fn gecici() -> Result<(PathBuf, std::fs::File), String> {
    for i in 0..16u32 {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let p = std::env::temp_dir().join(format!("sezgi-rea-{}-{n}-{i}.json", std::process::id()));
        if let Ok(f) = crate::guvenli::gizli_secenekler().write(true).create_new(true).open(&p) {
            return Ok((p, f));
        }
    }
    Err("geçici dosya oluşturulamadı".into())
}

fn sure() -> Duration {
    Duration::from_secs(std::env::var("SEZGI_REA_SURE").ok().and_then(|s| s.parse().ok()).unwrap_or(120))
}

impl Kopru {
    /// REA'yı kabuksuz, zaman ve bellek sınırıyla çalıştırır; standart çıktı geçici dosyaya. Başarıda dosyanın yolu.
    fn calistir_dosyaya(&self, argumanlar: &[&str], sinir: Duration) -> Result<PathBuf, String> {
        self.calistir_ham(argumanlar, sinir, false, &[])
    }

    /// `her_kod`: sıfırdan farklı çıkış kodu da çıktıyı döner (ör. `doctor` sağlıksız ortamda 1 döner)
    /// `ortam`: yalnızca alt sürece verilen ortam değişkenleri (ör. senaryonun gizli değerleri: komut satırında ve
    /// dosyada görünmezler)
    fn calistir_ham(&self, argumanlar: &[&str], sinir: Duration, her_kod: bool, ortam: &[(String, String)]) -> Result<PathBuf, String> {
        let (yol, dosya) = gecici()?;
        let (hata_yolu, hata) = gecici()?;
        let bellek = std::env::var("SEZGI_REA_BELLEK").ok().and_then(|s| s.parse::<u32>().ok()).unwrap_or(2048);
        let sonuc = (|| {
            let mut c = Command::new(&self.node)
                .arg(format!("--max-old-space-size={bellek}"))
                .arg(&self.betik)
                .args(argumanlar)
                .current_dir(std::env::temp_dir())
                .env("NO_COLOR", "1")
                .envs(ortam.iter().map(|(a, d)| (a.as_str(), d.as_str())))
                .stdin(Stdio::null())
                .stdout(dosya)
                .stderr(hata)
                .spawn()
                .map_err(|e| format!("REA başlatılamadı: {e}"))?;
            let bas = Instant::now();
            loop {
                if let Some(d) = c.try_wait().map_err(|e| e.to_string())? {
                    if d.success() || her_kod {
                        return Ok(());
                    }
                    let mut m = String::new();
                    let _ = std::fs::File::open(&hata_yolu).map(|f| f.take(600).read_to_string(&mut m));
                    return Err(format!("REA hata verdi ({d}): {}", crate::guvenli::temiz(m.trim())));
                }
                let buyuk = std::fs::metadata(&yol).is_ok_and(|m| m.len() > EN_BUYUK_CIKTI);
                if bas.elapsed() > sinir || buyuk {
                    let _ = c.kill();
                    let _ = c.wait();
                    return Err(if buyuk {
                        "REA çıktısı boyut sınırını aştı: durduruldu".into()
                    } else {
                        format!("REA {} sn içinde bitmedi: durduruldu", sinir.as_secs())
                    });
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        })();
        let _ = std::fs::remove_file(&hata_yolu);
        match sonuc {
            Ok(()) => Ok(yol),
            Err(e) => {
                let _ = std::fs::remove_file(&yol);
                Err(e)
            }
        }
    }

    fn metin_cikti(&self, argumanlar: &[&str], sinir: Duration) -> Result<String, String> {
        let yol = self.calistir_dosyaya(argumanlar, sinir)?;
        let s = oku_sinirli(&yol, 1 << 20).unwrap_or_default();
        let _ = std::fs::remove_file(&yol);
        Ok(s)
    }

    /// `rea capabilities`: aracın ilan edilip kullanılabilir olduğu
    pub(crate) fn ilan_ediliyor(&self, arac: &str) -> Result<(), String> {
        let v = self
            .yetenekler
            .get_or_init(|| {
                let yol = self.calistir_dosyaya(&["capabilities", "--format", "json"], sure())?;
                let v: Option<Value> = std::fs::File::open(&yol).ok().and_then(|f| serde_json::from_reader(std::io::BufReader::new(f)).ok());
                let _ = std::fs::remove_file(&yol);
                v.ok_or_else(|| "rea capabilities çıktısı okunamadı".to_string())
            })
            .clone()?;
        let gruplar = v["capabilities"].as_array().into_iter().flatten().flat_map(|c| c["groups"].as_array().into_iter().flatten());
        let mut ilan = false;
        for g in gruplar {
            if g["operations"].as_array().is_some_and(|o| o.iter().any(|x| x == arac)) {
                ilan = true;
                if g["available"] == true {
                    return Ok(());
                }
            }
        }
        Err(if ilan { format!("REA '{arac}' aracını bu makinede kullanılamaz diye bildiriyor") } else { format!("REA capabilities '{arac}' aracını ilan etmiyor") })
    }

    /// Sınıfı onay gerektiren bir komutu çalıştırır. Kapı köprüde de denetlenir: otomatik olmayan komut onaysız,
    /// dinamik komut dinamik onaysız çalışmaz; tablo dışı komut reddedilir. Çıktı geçici JSON dosyasıdır (çağıran okur ve
    /// siler)
    pub(crate) fn calistir_kapili(&self, komut: &str, argumanlar: &[&str], onay: Option<&Onay>, sinir: Duration) -> Result<PathBuf, String> {
        self.calistir_kapili_ortamli(komut, argumanlar, onay, sinir, &[])
    }

    /// `calistir_kapili`, alt sürece ek ortam değişkenleriyle
    pub(crate) fn calistir_kapili_ortamli(&self, komut: &str, argumanlar: &[&str], onay: Option<&Onay>, sinir: Duration, ortam: &[(String, String)]) -> Result<PathBuf, String> {
        let (s, b) = sinif(komut)?;
        match (s, onay) {
            (Sinif::Otomatik, _) => {}
            (_, None) => return Err(format!("'{komut}' {} bir komut ({}): insan onayı olmadan çalışmaz", s.ad(), etki_metni(b))),
            (_, Some(o)) if o.sinif < s => return Err(format!("'{komut}' {} onay ister; verilen onay {}", s.ad(), o.sinif.ad())),
            _ => {}
        }
        if komut == "capture-process" && cfg!(windows) {
            return Err("süreç gözlemi Windows'ta desteklenmiyor: REA'nın süreç ağacı örneklemesi Windows'ta boş döner".into());
        }
        let mut a = vec![komut];
        a.extend_from_slice(argumanlar);
        a.extend_from_slice(&["--format", "json"]);
        self.calistir_ham(&a, sinir, false, ortam)
    }

    /// REA `doctor` (salt okunur tanılama): analiz motorlarının kurulu olup olmadığı
    pub(crate) fn motorlar(&self) -> Result<Vec<Motor>, String> {
        let yol = self.calistir_ham(&["doctor", "--format", "json"], sure(), true, &[])?;
        let v: Option<Value> = std::fs::File::open(&yol).ok().and_then(|f| serde_json::from_reader(std::io::BufReader::new(f)).ok());
        let _ = std::fs::remove_file(&yol);
        let v = v.ok_or("rea doctor çıktısı okunamadı")?;
        let mut m: Vec<Motor> = Vec::new();
        for c in v["checks"].as_array().into_iter().flatten() {
            let ad = c["name"].as_str().unwrap_or("").trim_end_matches("-registration").trim_end_matches("-installation").to_string();
            if !matches!(ad.as_str(), "ghidra" | "ida" | "hopper" | "jadx" | "kuna" | "jeb" | "rizin") {
                continue;
            }
            let kurulu = c["ok"] == true;
            let aciklama = crate::guvenli::temiz(c["detail"].as_str().or(c["remediation"].as_str()).unwrap_or(""));
            m.push((ad.clone(), kurulu, ticari_mi(&ad), aciklama));
        }
        Ok(m)
    }

    /// `rea capabilities` grupları: (araç, kullanılabilir mi, neden)
    pub(crate) fn ilanlar(&self) -> Result<Vec<(String, bool, String)>, String> {
        let _ = self.ilan_ediliyor("");
        let v = self.yetenekler.get().cloned().unwrap_or_else(|| Err("rea capabilities okunamadı".into()))?;
        let mut l = Vec::new();
        for c in v["capabilities"].as_array().into_iter().flatten() {
            for g in c["groups"].as_array().into_iter().flatten() {
                for o in g["operations"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    l.push((o.to_string(), g["available"] == true, g["reason"].as_str().unwrap_or("").to_string()));
                }
            }
        }
        Ok(l)
    }

    /// İzinli bir aracı yapıt üzerinde çalıştırır ve çıktının biçim projeksiyonunu döner
    pub(crate) fn incele(&self, arac: &str, yapit: &Path) -> Result<Projeksiyon, String> {
        let ad = arac_izinli(arac)?;
        self.ilan_ediliyor(ad)?;
        let komut = ad.replace('_', "-");
        let hedef = yapit.to_str().ok_or("yapıt yolu UTF-8 değil")?;
        let yol = self.calistir_dosyaya(&[&komut, hedef, "--format", "json"], sure())?;
        // Akışla okunur: tanınmayan alanlar ve literal değerleri belleğe alınmadan atlanır
        let z: Result<Zarf, String> = std::fs::File::open(&yol)
            .map_err(|e| e.to_string())
            .and_then(|f| serde_json::from_reader(std::io::BufReader::new(f)).map_err(|e| format!("REA çıktısı beklenen biçimde değil: {e}")));
        let _ = std::fs::remove_file(&yol);
        let z = z?;
        if z.operation.as_deref() != Some(ad) {
            return Err(format!("REA çıktısının işlemi '{}' (beklenen {ad})", z.operation.as_deref().unwrap_or("?")));
        }
        Ok(Projeksiyon::yeni(z))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// REA kanıt zarfı: yalnızca gereken alanlar (gerisi okunurken atlanır)

#[derive(Deserialize)]
struct Zarf {
    evidence_id: Option<String>,
    operation: Option<String>,
    confidence: Option<String>,
    authority: Option<String>,
    provider: Option<Saglayici>,
    subject: Option<Konu>,
    #[serde(default)]
    normalized_result: Sonuc,
}

#[derive(Deserialize)]
struct Saglayici {
    id: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct Konu {
    digest: Option<Ozet>,
}

#[derive(Deserialize)]
struct Ozet {
    sha256: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Sonuc {
    module: Option<Modul>,
    metadata: Option<Meta>,
    types: Vec<Tip>,
    methods: Vec<Uye>,
    fields: Vec<Uye>,
    call_edges: Vec<Kenar>,
    user_strings: Option<Dizeler>,
    module_refs: Vec<ModulRef>,
    pinvoke_imports: Vec<PInvoke>,
}

#[derive(Deserialize)]
struct Modul {
    name: Option<String>,
    mvid: Option<String>,
}

#[derive(Deserialize)]
struct Meta {
    version: Option<String>,
}

#[derive(Deserialize)]
struct Tip {
    full_name: Option<String>,
}

#[derive(Deserialize)]
struct Uye {
    declaring_type: Option<String>,
    name: Option<String>,
    signature: Option<Imza>,
    body: Option<Govde>,
}

/// Metot gövdesi: yalnızca dize yükleyen (ldstr) çapaların işleneni (literalin kimliği)
#[derive(Deserialize, Default)]
#[serde(default)]
struct Govde {
    anchors: Vec<Capa>,
}

#[derive(Deserialize)]
struct Capa {
    opcode: Option<String>,
    operand: Option<String>,
}

#[derive(Deserialize)]
struct Imza {
    return_type: Option<String>,
    #[serde(default)]
    parameter_types: Vec<String>,
    field_type: Option<String>,
}

#[derive(Deserialize)]
struct Kenar {
    caller: Option<String>,
    opcode: Option<String>,
    target_name: Option<String>,
    target_declaring_type: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Dizeler {
    literals: Vec<Literal>,
}

/// Literalin değeri okunurken SQL biçimine çevrilir; değerin kendisi saklanmaz
#[derive(Deserialize)]
struct Literal {
    token: Option<String>,
    #[serde(rename = "value", default, deserialize_with = "sql_bicimi_al")]
    sql: Option<Value>,
}

#[derive(Deserialize)]
struct ModulRef {
    token: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct PInvoke {
    member_name: Option<String>,
    import_name: Option<String>,
    import_scope_token: Option<String>,
}

fn sql_bicimi_al<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    let s: Option<String> = Option::deserialize(d)?;
    Ok(s.as_deref().and_then(sql_bicimi))
}

/// SQL dizesinin biçimi: işlem ve tablo adları (değerler alınmaz). SQL'e benzemiyorsa `None`.
pub(crate) fn sql_bicimi(s: &str) -> Option<Value> {
    let kelimeler: Vec<String> = s.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.')).filter(|w| !w.is_empty()).take(400).map(String::from).collect();
    let buyuk: Vec<String> = kelimeler.iter().map(|w| w.to_uppercase()).collect();
    let islem = buyuk.first()?.clone();
    let var = |k: &str| buyuk.iter().any(|w| w == k);
    let sql = match islem.as_str() {
        "SELECT" | "DELETE" => var("FROM"),
        "UPDATE" => var("SET"),
        "INSERT" | "REPLACE" => var("INTO"),
        "CREATE" | "ALTER" | "DROP" => var("TABLE") || var("INDEX") || var("VIEW") || var("TRIGGER"),
        "WITH" => var("SELECT"),
        _ => false,
    };
    if !sql {
        return None;
    }
    let mut tablolar: Vec<String> = Vec::new();
    for (i, w) in buyuk.iter().enumerate() {
        if matches!(w.as_str(), "FROM" | "JOIN" | "INTO" | "TABLE") || (w == "UPDATE" && i == 0) {
            let mut j = i + 1;
            while buyuk.get(j).is_some_and(|x| matches!(x.as_str(), "IF" | "NOT" | "EXISTS" | "ONLY")) {
                j += 1;
            }
            if let Some(t) = kelimeler.get(j).filter(|t| !t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.len() <= 128) {
                let t = t.rsplit('.').next().unwrap_or(t).to_string();
                if !tablolar.contains(&t) && !matches!(t.to_uppercase().as_str(), "SELECT" | "VALUES") {
                    tablolar.push(t);
                }
            }
        }
    }
    Some(json!({ "islem": islem, "tablolar": tablolar }))
}

/// REA güven değeri → güven ölçeği (certain > observed > inferred > unknown). Eksik ya da tanınmayan: en zayıf.
fn guven(c: Option<&str>) -> &'static str {
    match c {
        Some("certain") => "certain",
        Some("observed") => "observed",
        _ => "inferred",
    }
}

/// Bir REA çıktısının biçim projeksiyonu: kanıta dönüşecek parçalar
pub(crate) struct Projeksiyon {
    pub islem: String,
    pub kaynak_kanit: Option<String>,
    pub saglayici: String,
    pub guven: &'static str,
    pub yetki: String,
    pub sha256: Option<String>,
    /// BINARY_METADATA, yapı (tipler, üyeler, alanlar; P/Invoke)
    pub yapi: Value,
    /// BINARY_METADATA, kesin çağrı hedefleri
    pub cagrilar: Value,
    /// BINARY_HEURISTIC: literal dizelerden SQL biçimi (değer yok)
    pub sql: Vec<Value>,
    pub literal_sayisi: usize,
}

impl Projeksiyon {
    fn yeni(z: Zarf) -> Projeksiyon {
        let r = z.normalized_result;
        let tip_adi = |u: &Uye| format!("{}.{}", u.declaring_type.as_deref().unwrap_or("?"), u.name.as_deref().unwrap_or("?"));
        let uyeler: Vec<Value> = r
            .methods
            .iter()
            .take(50_000)
            .map(|m| {
                let i = m.signature.as_ref();
                json!({ "ad": tip_adi(m), "donus": i.and_then(|i| i.return_type.clone()), "parametreler": i.map(|i| i.parameter_types.clone()).unwrap_or_default() })
            })
            .collect();
        let alanlar: Vec<Value> = r.fields.iter().take(50_000).map(|f| json!({ "ad": tip_adi(f), "tur": f.signature.as_ref().and_then(|i| i.field_type.clone()) })).collect();
        let pinvoke: Vec<Value> = r
            .pinvoke_imports
            .iter()
            .map(|p| {
                let dll = r.module_refs.iter().find(|m| m.token.is_some() && m.token == p.import_scope_token).and_then(|m| m.name.clone());
                json!({ "uye": p.member_name, "giris": p.import_name, "dll": dll })
            })
            .collect();
        let mut yapi = json!({ "tipler": r.types.iter().filter_map(|t| t.full_name.clone()).take(50_000).collect::<Vec<_>>(), "uyeler": uyeler, "alanlar": alanlar });
        if let Some(m) = &r.module {
            yapi["modul"] = json!({ "ad": m.name, "mvid": m.mvid });
        }
        if let Some(m) = &r.metadata {
            yapi["meta_veri_surumu"] = json!(m.version);
        }
        if !pinvoke.is_empty() || !r.module_refs.is_empty() {
            yapi["pinvoke"] = json!(pinvoke);
            yapi["yerel_moduller"] = json!(r.module_refs.iter().filter_map(|m| m.name.clone()).collect::<Vec<_>>());
        }
        let cagrilar: Vec<Value> = r
            .call_edges
            .iter()
            .take(200_000)
            .map(|k| {
                json!({ "cagiran": k.caller, "hedef": format!("{}.{}", k.target_declaring_type.as_deref().unwrap_or("?"), k.target_name.as_deref().unwrap_or("?")), "komut": k.opcode })
            })
            .collect();
        // SQL biçimi, literali yükleyen metotlarla (ldstr çapası → literal kimliği); değer yine alınmaz
        let yukleyenler = |token: &str| -> Vec<String> {
            r.methods
                .iter()
                .filter(|m| m.body.as_ref().is_some_and(|g| g.anchors.iter().any(|c| c.opcode.as_deref() == Some("ldstr") && c.operand.as_deref() == Some(token))))
                .map(|m| format!("{}.{}", m.declaring_type.as_deref().unwrap_or("?"), m.name.as_deref().unwrap_or("?")))
                .collect()
        };
        let (literal_sayisi, sql) = r
            .user_strings
            .as_ref()
            .map(|d| {
                let sql = d
                    .literals
                    .iter()
                    .filter_map(|l| {
                        let mut v = l.sql.clone()?;
                        v["metotlar"] = json!(l.token.as_deref().map(yukleyenler).unwrap_or_default());
                        Some(v)
                    })
                    .collect();
                (d.literals.len(), sql)
            })
            .unwrap_or((0, Vec::new()));
        Projeksiyon {
            islem: z.operation.unwrap_or_default(),
            kaynak_kanit: z.evidence_id,
            saglayici: format!(
                "rea@{SURUM}/{}@{}",
                z.provider.as_ref().and_then(|p| p.id.clone()).unwrap_or_else(|| "?".into()),
                z.provider.as_ref().and_then(|p| p.version.clone()).unwrap_or_else(|| "?".into())
            ),
            guven: guven(z.confidence.as_deref()),
            yetki: z.authority.unwrap_or_else(|| "bilinmiyor".into()),
            sha256: z.subject.and_then(|s| s.digest).and_then(|d| d.sha256),
            yapi,
            cagrilar: json!(cagrilar),
            sql,
            literal_sayisi,
        }
    }
}
