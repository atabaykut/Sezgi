//! İnsan anahtarı ve sistem çapası: gerçek veride "bunu bir insan yaptı" kanıtı.
//!
//! Sorun: aynı kullanıcı hesabında kabuk erişimi olan bir kodlama ajanı sahte terminal açabilir (`script`) ve Sezgi'nin
//! kullanıcı klasöründeki ayar ve token dosyalarını düzenleyebilir; "terminal var mı" denetimi insanı kanıtlamaz.
//!
//! Kanıt iki parçadır:
//! - **Parola:** yalnızca insanın bildiği bir sır. Parola → PBKDF2-HMAC-SHA256 (16 bayt tuz, en az 600.000 yineleme)
//!   → 32 bayt → Ed25519 tohumu → imza anahtar çifti. Parola ve gizli anahtar hiçbir yere yazılmaz.
//! - **Sistem çapası:** parolanın tuzu, yineleme sayısı ve açık anahtarı, ajanın (aynı kullanıcı, yönetici değil)
//!   yazamadığı bir dosyada: Unix'te `/etc/sezgi/insan-<uid>.json` (kök sahipli; dosya ve klasör grup ya da başkaları
//!   için yazılamaz), Windows'ta `<ProgramData>\sezgi\insan-<kullanıcı>.json` (dosya ve klasörün sahibi Administrators
//!   ya da SYSTEM, izin listesi başka kimseye değiştirme hakkı vermiyor ve bu süreç yazamıyor; kurulum klasörü
//!   sahiplenir, mirası keser: Administrators ve SYSTEM tam, Users yalnızca okuma). Çapayı yalnızca yönetici kurar ve
//!   değiştirir: `sudo sezgi insan kur --sistem` ya da yönetici terminalinde `sezgi insan kur --sistem`. Kullanıcı
//!   klasöründeki bir dosya çapa sayılmaz: ajan onu silip kendi parolasıyla yeniden kurabilirdi.
//!
//! Kullanım (canlı mod): gerçek veride her insan adımı (açma, gerçek-veri token'ı, her onay, saatlik sınırın
//! değişmesi) parolayı terminalde ister ve çapadaki açık anahtarla doğrular. Bu adımların bıraktığı kayıtlar (hedef,
//! gerçek-veri token'ı, çapa varken rol beyanı) parolanın gizli anahtarıyla imzalanır; doğrulama açık anahtarla yapılır
//! (parola gerekmez). Kayıt sonradan elle değiştirilirse imza tutmaz ve o yetki yok sayılır: yazma kapalıya düşer.
//!
//! Sınır: ajan parolayı öğrenirse (tuş kaydı, insanın paylaşması), yönetici/kök yetkisiyle çalışıyorsa ya da Sezgi'nin
//! sakladığı oturum belirtecini okuyup uygulamaya Sezgi'siz giderse bu kanıt korumaz.
use serde_json::{json, Value};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// Yeni çapada PBKDF2 yineleme sayısı; daha azını taşıyan çapa kabul edilmez
const YINELEME: u32 = 600_000;
/// Parolanın en kısa uzunluğu (karakter): çapa herkesçe okunabilir, açık anahtara karşı çevrimdışı deneme yapılabilir
const EN_KISA: usize = 12;
/// Çapa dosyasının türü (başka bir JSON dosyası çapa sayılmaz)
const TUR: &str = "sezgi-insan-capasi";
/// Varsayılan yerde çapa yoksa bakılan ikinci klasör: yalnızca yönetici sahipli ve yazılamayan klasör kabul edilir
/// (ajan bu değişkenle kendi yazabildiği bir yeri gösteremez; varsayılan yerdeki çapa her zaman önce gelir)
const KLASOR_DEGISKENI: &str = "SEZGI_CAPA_KLASORU";

/// Sistem çapası: bu kullanıcının insan anahtarının açık kısmı
pub struct Capa {
    pub yol: PathBuf,
    acik: Vec<u8>,
    tuz: Vec<u8>,
    yineleme: u32,
    pub izi: String,
    pub kuruldu: String,
}

/// Çapa araması: yok, var ama geçersiz (korunmuyor ya da bozuk), geçerli
pub enum CapaDurumu {
    Yok(Option<PathBuf>),
    Gecersiz(PathBuf, String),
    Var(Capa),
}

/// Parolayla açılmış insan anahtarı (yalnızca bellekte)
pub struct Anahtar {
    cift: ring::signature::Ed25519KeyPair,
    pub izi: String,
}

fn b64(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn b64_coz(s: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).ok()
}

/// Açık anahtarın kısa parmak izi (insana gösterilir, kayda yazılır)
fn parmak_izi(acik: &[u8]) -> String {
    crate::canli::ozet_metni(acik)[..16].to_string()
}

/// Paroladan imza anahtar çifti
fn turet(parola: &str, tuz: &[u8], yineleme: u32) -> Result<ring::signature::Ed25519KeyPair, String> {
    let n = std::num::NonZeroU32::new(yineleme).ok_or(t!("yineleme sayısı geçersiz", "invalid iteration count"))?;
    let mut k = [0u8; 32];
    ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA256, n, tuz, parola.as_bytes(), &mut k);
    let tohum = ring::hmac::sign(&ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &k), b"sezgi/insan/imza-tohumu/v1");
    ring::signature::Ed25519KeyPair::from_seed_unchecked(tohum.as_ref()).map_err(|_| t!("anahtar türetilemedi", "could not derive the key").to_string())
}

// ---- Bu kullanıcı ve çapanın yeri ----

/// Bu sürecin kullanıcısı (ortamdan değil, işletim sisteminden): Unix'te etkin uid, Windows'ta kullanıcı adı
pub fn kullanici() -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Linux: /proc/self sürecin kullanıcısına aittir; başka yerde yeni oluşturulan dosyanın sahibi
        if let Ok(m) = std::fs::metadata("/proc/self") {
            return Some(m.uid().to_string());
        }
        let yol = std::env::temp_dir().join(format!(".sezgi-kimlik-{}", std::process::id()));
        let uid = std::fs::File::create(&yol).and_then(|f| f.metadata()).map(|m| m.uid()).ok();
        let _ = std::fs::remove_file(&yol);
        uid.map(|u| u.to_string())
    }
    #[cfg(windows)]
    {
        pencere::kullanici_adi()
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Varsayılan çapa klasörü (ortamdan bağımsız): Unix'te /etc/sezgi, Windows'ta sistem sürücüsündeki ProgramData\sezgi
fn varsayilan_klasor() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        Some(PathBuf::from("/etc/sezgi"))
    }
    #[cfg(windows)]
    {
        pencere::sistem_suruculu("ProgramData").map(|p| p.join("sezgi"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

fn dosya_adi(k: &str) -> String {
    let guvenli: String = k.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' }).collect();
    format!("insan-{guvenli}.json")
}

/// Dosya ve klasörü bu kullanıcı (yönetici değilse) değiştiremez mi; değiştirebiliyorsa neden
fn korunuyor(yol: &Path) -> Result<(), String> {
    let m = std::fs::symlink_metadata(yol).map_err(|e| tf!("okunamıyor: {e}", "cannot be read: {e}"))?;
    if !m.file_type().is_file() {
        return Err(t!("düz dosya değil (sembolik bağ çapa sayılmaz)", "not a regular file (a symbolic link is not an anchor)").into());
    }
    let klasor = yol.parent().ok_or(t!("klasörü yok", "it has no folder"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let k = std::fs::metadata(klasor).map_err(|e| tf!("klasör okunamıyor: {e}", "the folder cannot be read: {e}"))?;
        if m.uid() != 0 || k.uid() != 0 {
            return Err(t!("dosya ya da klasörü kök (root) kullanıcısına ait değil", "the file or its folder is not owned by root").into());
        }
        if m.mode() & 0o022 != 0 || k.mode() & 0o022 != 0 {
            return Err(t!("dosya ya da klasörü grup ya da başkaları için yazılabilir", "the file or its folder is writable by the group or others").into());
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        if !pencere::yonetici_sahipli(yol)? {
            return Err(t!("dosyanın sahibi Administrators ya da SYSTEM değil", "the file's owner is not Administrators or SYSTEM").into());
        }
        // Klasör de yöneticinin olmalı ve başkasına değiştirme hakkı vermemeli: kullanıcının önceden oluşturduğu klasörde
        // çapa silinip yeniden yazılabilir; dosyanın izni de açıkça denetlenir (yönetici süreçte yazma denemesi her zaman
        // başarılıdır)
        windows_izinleri(klasor, yol)?;
        // Sahibi yönetici olsa da bu süreç yazabiliyorsa (yükseltilmiş süreç ya da gevşek izin) ajan da yazabilir
        if std::fs::OpenOptions::new().write(true).open(yol).is_ok() {
            return Err(t!(
                "bu süreç dosyayı değiştirebiliyor (yönetici olarak çalışan terminal ya da gevşek dosya izni): çapa yalnızca yönetici olmayan süreçte geçerlidir",
                "this process can change the file (a terminal running as administrator, or loose file permissions): the anchor is valid only in a non-administrator process"
            )
            .into());
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (m, klasor);
        Err(t!("bu işletim sisteminde çapa denetlenemiyor", "the anchor cannot be checked on this operating system").into())
    }
}

fn capa_oku(yol: &Path, kullanici: &str) -> Result<Capa, String> {
    korunuyor(yol)?;
    let v: Value = std::fs::read_to_string(yol).ok().and_then(|m| serde_json::from_str(&m).ok()).ok_or(t!("JSON okunamıyor", "cannot read JSON"))?;
    if v["tur"] != TUR || v["surum"] != 1 || v["kdf"] != "pbkdf2-hmac-sha256" {
        return Err(t!("çapa biçimi tanınmıyor", "anchor format not recognized").into());
    }
    if v["kullanici"].as_str() != Some(kullanici) {
        return Err(tf!("çapa başka bir kullanıcıya ait ({})", "the anchor belongs to another user ({})", v["kullanici"].as_str().unwrap_or("?")));
    }
    let yineleme = v["yineleme"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n >= YINELEME)
        .ok_or(tf!("yineleme sayısı {YINELEME}'den az", "iteration count below {YINELEME}"))?;
    let tuz = v["tuz"].as_str().and_then(b64_coz).filter(|t| t.len() >= 16).ok_or(t!("tuz geçersiz", "invalid salt"))?;
    let acik = v["acik_anahtar"].as_str().and_then(b64_coz).filter(|a| a.len() == 32).ok_or(t!("açık anahtar geçersiz", "invalid public key"))?;
    Ok(Capa { yol: yol.to_path_buf(), izi: parmak_izi(&acik), acik, tuz, yineleme, kuruldu: v["kuruldu"].as_str().unwrap_or("?").to_string() })
}

/// Bu kullanıcının sistem çapası. Önce varsayılan yer; orada dosya yoksa (yalnızca o zaman) `SEZGI_CAPA_KLASORU`.
pub fn capa() -> CapaDurumu {
    let Some(k) = kullanici() else { return CapaDurumu::Yok(None) };
    let ad = dosya_adi(&k);
    let adaylar: Vec<PathBuf> =
        varsayilan_klasor().into_iter().chain(std::env::var_os(KLASOR_DEGISKENI).filter(|v| !v.is_empty()).map(PathBuf::from)).map(|d| d.join(&ad)).collect();
    for yol in &adaylar {
        if std::fs::symlink_metadata(yol).is_ok() {
            return match capa_oku(yol, &k) {
                Ok(c) => CapaDurumu::Var(c),
                Err(e) => CapaDurumu::Gecersiz(yol.clone(), e),
            };
        }
    }
    CapaDurumu::Yok(adaylar.into_iter().next())
}

/// Geçerli çapa (yoksa ya da geçersizse None)
pub fn gecerli_capa() -> Option<Capa> {
    match capa() {
        CapaDurumu::Var(c) => Some(c),
        _ => None,
    }
}

/// Çapa yoksa ya da geçersizse insana söylenecek neden ve yapılacak
pub fn capa_eksik_nedeni() -> Option<String> {
    match capa() {
        CapaDurumu::Var(_) => None,
        CapaDurumu::Yok(_) => Some(tf!(
            "sistem çapası yok: gerçek veride yazma, yöneticinin kurduğu insan anahtarı ister ({})",
            "no system anchor: writing on real data needs a human key set up by an administrator ({})",
            kur_komutu()
        )),
        CapaDurumu::Gecersiz(y, e) => Some(tf!(
            "sistem çapası geçersiz ({}: {e}); yönetici yeniden kurmalı ({})",
            "system anchor invalid ({}: {e}); an administrator must set it up again ({})",
            crate::guvenli::gorunen(&y),
            kur_komutu()
        )),
    }
}

/// Çapayı kuran komut (bu işletim sisteminde)
pub fn kur_komutu() -> &'static str {
    if cfg!(windows) {
        t!("yönetici terminalinde: sezgi insan kur --sistem", "in an administrator terminal: sezgi human setup --system")
    } else {
        t!("sudo sezgi insan kur --sistem", "sudo sezgi human setup --system")
    }
}

// ---- Parola ----

/// Terminalde yankısız okuma: standart girdi ve hata çıktısı terminal değilse parola sorulmaz
pub fn parola_sor(soru: &str) -> Result<String, String> {
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return Err(t!("insan parolası yalnızca terminalde sorulur (terminal yok)", "the human password is asked only in a terminal (no terminal)").into());
    }
    eprint!("{soru}: ");
    let _ = std::io::stderr().flush();
    let yanki = Yanki::kapat();
    let mut s = String::new();
    let okundu = std::io::stdin().lock().read_line(&mut s);
    drop(yanki);
    eprintln!();
    okundu.map_err(|e| tf!("parola okunamadı: {e}", "could not read the password: {e}"))?;
    Ok(s.trim_end_matches(['\r', '\n']).to_string())
}

/// Terminal yankısını kapatır; bırakılınca açar
struct Yanki {
    #[cfg(windows)]
    eski: Option<u32>,
}

impl Yanki {
    fn kapat() -> Yanki {
        #[cfg(unix)]
        {
            let _ = stty("-echo");
            Yanki {}
        }
        #[cfg(windows)]
        {
            Yanki { eski: pencere::yanki_kapat() }
        }
        #[cfg(not(any(unix, windows)))]
        {
            Yanki {}
        }
    }
}

impl Drop for Yanki {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = stty("echo");
        }
        #[cfg(windows)]
        {
            if let Some(m) = self.eski {
                pencere::kip_geri(m);
            }
        }
    }
}

/// `stty` standart girdinin terminaline uygulanır (yankı yalnızca insanın yazdığını ekranda göstermemek için kapatılır)
#[cfg(unix)]
fn stty(a: &str) -> std::io::Result<std::process::ExitStatus> {
    let yol = ["/bin/stty", "/usr/bin/stty"].into_iter().find(|p| Path::new(p).exists()).unwrap_or("stty");
    std::process::Command::new(yol).arg(a).stdin(std::process::Stdio::inherit()).stderr(std::process::Stdio::null()).status()
}

/// Gerçek veride insan adımı: geçerli sistem çapası olmalı ve insan parolasını terminalde yazmalı. `ne`: insana
/// gösterilen adım (ör. "gerçek veriyi açmak"). Dönen: imza anahtarı (yalnızca bu süreçte, bellekte)
pub fn kanit(ne: &str) -> Result<Anahtar, String> {
    let c = match capa() {
        CapaDurumu::Var(c) => c,
        _ => return Err(capa_eksik_nedeni().unwrap_or_default()),
    };
    let p = parola_sor(&tf!("İnsan parolası ({ne}; çapa {})", "Human password ({ne}; anchor {})", c.izi))?;
    ac(&c, &p).ok_or_else(|| t!("insan parolası yanlış: adım yapılmadı", "wrong human password: the step was not done").to_string())
}

/// Parola çapadaki açık anahtarı veriyorsa anahtar
fn ac(c: &Capa, parola: &str) -> Option<Anahtar> {
    use ring::signature::KeyPair;
    let cift = turet(parola, &c.tuz, c.yineleme).ok()?;
    (cift.public_key().as_ref() == c.acik.as_slice()).then(|| Anahtar { cift, izi: c.izi.clone() })
}

// ---- İmza ----

fn mesaj(tur: &str, veri: &Value) -> Vec<u8> {
    format!("sezgi/insan/{tur}/v1\n{}", serde_json::to_string(veri).unwrap_or_default()).into_bytes()
}

/// Kaydı insanın anahtarıyla imzalar: `{"izi": <çapanın izi>, "imza": <base64>}`. `tur` imzanın ne için olduğunu
/// söyler (bir kaydın imzası başka bir tür kayıtta geçmez)
pub fn imzala(a: &Anahtar, tur: &str, veri: &Value) -> Value {
    json!({ "izi": a.izi, "imza": b64(a.cift.sign(&mesaj(tur, veri)).as_ref()) })
}

/// İmza bu çapanın açık anahtarıyla bu kayıt için mi
pub fn dogrula(c: &Capa, tur: &str, veri: &Value, imza: &Value) -> bool {
    let Some(s) = imza["imza"].as_str().and_then(b64_coz) else { return false };
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &c.acik).verify(&mesaj(tur, veri), &s).is_ok()
}

// ---- Komut ----

fn dur(m: &str) -> ! {
    eprintln!("✗ {m}");
    std::process::exit(1)
}

/// sezgi insan [durum] | sezgi insan kur --sistem [--kullanici <uid|ad>]
pub fn insan_komutu(args: &[String]) {
    match args.get(1).filter(|a| !a.starts_with("--")).map(String::as_str) {
        None | Some("durum") => durum_yaz(),
        Some("kur") => kur(args),
        Some(x) => crate::yardim::kullanim_hatasi("insan", &tf!("bilinmeyen: insan {x} (durum, kur)", "unknown: human {x} (status, setup)")),
    }
}

fn durum_yaz() {
    tp!("İnsan anahtarı (gerçek veride insanın kanıtı)", "Human key (proof of a human on real data)");
    match capa() {
        CapaDurumu::Var(c) => {
            tp!("  Sistem çapası  {} · iz {} · kuruldu {}", "  System anchor  {} · fingerprint {} · set up {}", crate::guvenli::gorunen(&c.yol), c.izi, c.kuruldu);
            tp!(
                "  Koruma         dosya yöneticiye ait; bu kullanıcı (ve aynı hesaptaki ajan) değiştiremez",
                "  Protection     the file belongs to an administrator; this user (and an agent on the same account) cannot change it"
            );
            tp!(
                "  Gerçek veride açma, gerçek-veri token'ı, her onay ve saatlik sınır değişikliği bu parolayı ister.",
                "  Opening real data, a real-data token, every approval and every change to the hourly limit need this password."
            );
        }
        CapaDurumu::Yok(y) => {
            tp!("  Sistem çapası  yok{}", "  System anchor  none{}", y.map(|y| format!(" ({})", crate::guvenli::gorunen(&y))).unwrap_or_default());
            tp!("  Gerçek veride yazma kapalı. Kurmak için: {}", "  Writing on real data is off. To set up: {}", kur_komutu());
        }
        CapaDurumu::Gecersiz(y, e) => {
            tp!("  Sistem çapası  geçersiz: {} ({e})", "  System anchor  invalid: {} ({e})", crate::guvenli::gorunen(&y));
            tp!("  Gerçek veride yazma kapalı. Yönetici yeniden kurmalı: {}", "  Writing on real data is off. An administrator must set it up again: {}", kur_komutu());
        }
    }
    tp!(
        "  Sınır: parolayı bilen ya da yönetici/kök yetkisiyle çalışan bir program korunmaz; ayrıntı: SECURITY.md",
        "  Limit: it does not protect against a program that knows the password or runs with administrator/root rights; details: SECURITY.md"
    );
}

/// Çapayı kurar ya da parolayı değiştirir: yalnızca yönetici (Unix'te kök, Windows'ta yönetici terminali)
fn kur(args: &[String]) {
    if !args.iter().any(|a| a == "--sistem") {
        crate::yardim::kullanim_hatasi(
            "insan",
            t!(
                "çapa yalnızca sistem klasörüne kurulur: sezgi insan kur --sistem (kullanıcı klasöründeki bir dosyayı ajan silip yeniden kurabilir)",
                "the anchor is set up only in a system folder: sezgi human setup --system (an agent could delete and recreate a file in the user's folder)"
            ),
        );
    }
    let secenek = |ad: &str| args.iter().position(|a| a == ad).and_then(|i| args.get(i + 1)).cloned();
    let hedef_kullanici = kur_kullanicisi(secenek("--kullanici"));
    let klasor = std::env::var_os(KLASOR_DEGISKENI)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(varsayilan_klasor)
        .unwrap_or_else(|| dur(t!("çapa klasörü bulunamadı", "anchor folder not found")));
    let yol = klasor.join(dosya_adi(&hedef_kullanici));
    // Var olan çapanın parolası değişecekse önce eski parola
    if std::fs::symlink_metadata(&yol).is_ok() {
        match capa_oku(&yol, &hedef_kullanici) {
            Ok(c) => {
                let p = parola_sor(&tf!("Şimdiki insan parolası (çapa {})", "Current human password (anchor {})", c.izi)).unwrap_or_else(|e| dur(&e));
                if ac(&c, &p).is_none() {
                    dur(t!(
                        "şimdiki parola yanlış: çapa değiştirilmedi (parola unutulduysa yönetici dosyayı silip yeniden kurar)",
                        "wrong current password: the anchor was not changed (if the password is forgotten, an administrator deletes the file and sets it up again)"
                    ));
                }
            }
            Err(e) => te!("! var olan çapa geçersiz ({e}); yenisi yazılacak", "! the existing anchor is invalid ({e}); a new one will be written"),
        }
    }
    te!(
        "Gerçek veride her insan adımı bu parolayı isteyecek. Yalnızca siz bilin: yapay zekâ ajanına, betiğe ya da",
        "Every human step on real data will ask for this password. Keep it to yourself: do not give it to an AI agent, a script or"
    );
    te!(
        "ortam değişkenine vermeyin; parolayı bilen her program sizin yerinize onay verebilir. En az {EN_KISA} karakter.",
        "an environment variable; any program that knows the password can approve in your place. At least {EN_KISA} characters."
    );
    let p1 = parola_sor(t!("Yeni insan parolası", "New human password")).unwrap_or_else(|e| dur(&e));
    if p1.chars().count() < EN_KISA {
        dur(&tf!("parola en az {EN_KISA} karakter olmalı", "the password must be at least {EN_KISA} characters"));
    }
    let p2 = parola_sor(t!("Yeniden", "Again")).unwrap_or_else(|e| dur(&e));
    if p1 != p2 {
        dur(t!("parolalar aynı değil: çapa yazılmadı", "the passwords differ: the anchor was not written"));
    }
    let mut tuz = [0u8; 16];
    getrandom::fill(&mut tuz).unwrap_or_else(|e| dur(&tf!("rastgele sayı üretilemedi: {e}", "could not generate random bytes: {e}")));
    let cift = turet(&p1, &tuz, YINELEME).unwrap_or_else(|e| dur(&e));
    use ring::signature::KeyPair;
    let acik = cift.public_key().as_ref().to_vec();
    let v = json!({ "tur": TUR, "surum": 1, "kullanici": hedef_kullanici, "kdf": "pbkdf2-hmac-sha256", "yineleme": YINELEME, "tuz": b64(&tuz),
        "acik_anahtar": b64(&acik), "kuruldu": crate::simdi() });
    capa_yaz(&klasor, &yol, &serde_json::to_string_pretty(&v).unwrap()).unwrap_or_else(|e| dur(&e));
    // Yazılan dosya gerçekten korunuyor mu (ör. Windows'ta yükseltilmemiş terminal: sahibi kullanıcı olur)
    if let Err(e) = kurulum_denetimi(&yol) {
        let _ = std::fs::remove_file(&yol);
        dur(&tf!("çapa korunmuyor ({e}); dosya silindi. {}", "the anchor is not protected ({e}); the file was deleted. {}", kur_komutu()));
    }
    tp!("✓ Sistem çapası kuruldu: {} · iz {}", "✓ System anchor set up: {} · fingerprint {}", crate::guvenli::gorunen(&yol), parmak_izi(&acik));
    tp!(
        "  Bu kullanıcı ({hedef_kullanici}) gerçek veride her insan adımında bu parolayı yazar; çapayı yalnızca yönetici değiştirir.",
        "  This user ({hedef_kullanici}) types this password at every human step on real data; only an administrator changes the anchor."
    );
    tp!(
        "  Önceki parolayla imzalanmış kayıtlar (gerçek-veri hedefi, token, rol) artık geçmez: hedefi ve token'ı terminalde yeniden açın.",
        "  Records signed with the previous password (real-data target, token, roles) are no longer valid: open the target and the token again in a terminal."
    );
}

/// Çapanın kimin için kurulacağı: `--kullanici`, yoksa sudo'yu çağıran kullanıcı (SUDO_UID) ya da bu kullanıcı.
/// Unix'te yalnızca kök kurar; `--kullanici` uid ya da `/etc/passwd`'deki ad olabilir (ad uid'ye çevrilir).
fn kur_kullanicisi(acik: Option<String>) -> String {
    #[cfg(unix)]
    {
        if kullanici().as_deref() != Some("0") {
            dur(&tf!("çapayı yalnızca yönetici kurar (kök yetkisi gerekir): {}", "only an administrator sets up the anchor (root rights needed): {}", kur_komutu()));
        }
        let k = acik.or_else(|| std::env::var("SUDO_UID").ok()).unwrap_or_else(|| {
            dur(t!(
                "hangi kullanıcı için? sezgi insan kur --sistem --kullanici <uid|ad> (sudo ile çalıştırınca kendiliğinden)",
                "for which user? sezgi human setup --system --user <uid|name> (automatic when run with sudo)"
            ))
        });
        if k.is_empty() {
            dur(t!("--kullanici boş olamaz: kullanıcı kimliği (uid) ya da adı", "--user cannot be empty: a user id (uid) or name"));
        }
        if k.chars().all(|c| c.is_ascii_digit()) {
            return k;
        }
        // Ad verildiyse kullanıcı veritabanından uid (çapanın adı uid'dir; süreç kullanıcısı uid ile aranır)
        let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
        uid_bul(&passwd, &k).unwrap_or_else(|| {
            dur(&tf!(
                "kullanıcı bulunamadı: {k} (/etc/passwd'de yok; dizin hizmetindeki kullanıcı için uid verin: --kullanici $(id -u {k}))",
                "user not found: {k} (not in /etc/passwd; for a directory-service user give the uid: --user $(id -u {k}))"
            ))
        })
    }
    #[cfg(not(unix))]
    {
        acik.or_else(kullanici).unwrap_or_else(|| dur(t!("kullanıcı adı okunamadı: --kullanici <ad>", "could not read the user name: --user <name>")))
    }
}

/// `/etc/passwd` biçimindeki metinde adın uid'si (`ad:x:uid:gid:...`); yoksa ya da uid sayı değilse None
#[cfg_attr(not(unix), allow(dead_code))]
fn uid_bul(passwd: &str, ad: &str) -> Option<String> {
    passwd.lines().filter(|l| !l.starts_with('#')).find_map(|l| {
        let mut p = l.split(':');
        if p.next() != Some(ad) {
            return None;
        }
        p.nth(1).filter(|u| !u.is_empty() && u.chars().all(|c| c.is_ascii_digit())).map(String::from)
    })
}

fn capa_yaz(klasor: &Path, yol: &Path, icerik: &str) -> Result<(), String> {
    if !klasor.exists() {
        std::fs::create_dir_all(klasor).map_err(|e| tf!("{} oluşturulamadı: {e}", "could not create {}: {e}", crate::guvenli::gorunen(klasor)))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(klasor, std::fs::Permissions::from_mode(0o755)).map_err(|e| tf!("klasör izni verilemedi: {e}", "could not set the folder permission: {e}"))?;
    }
    // Windows: klasör (kullanıcı önceden oluşturmuş olabilir) yöneticiye geçer, mirası kesilir ve Users yalnızca okur;
    // yeni dosya bu izni miras alır
    #[cfg(windows)]
    pencere::sikilastir(klasor, true)?;
    let gecici = klasor.join(format!(".{}.yeni", yol.file_name().unwrap_or_default().to_string_lossy()));
    let _ = std::fs::remove_file(&gecici);
    std::fs::write(&gecici, icerik).map_err(|e| tf!("çapa yazılamadı: {e}", "could not write the anchor: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gecici, std::fs::Permissions::from_mode(0o644)).map_err(|e| tf!("çapa izni verilemedi: {e}", "could not set the anchor permission: {e}"))?;
    }
    std::fs::rename(&gecici, yol).map_err(|e| tf!("çapa yazılamadı: {e}", "could not write the anchor: {e}"))?;
    // Taşınan dosya geçici adındaki izni taşır: çapanın izni ayrıca, miras kesilerek yazılır
    #[cfg(windows)]
    pencere::sikilastir(yol, false)?;
    Ok(())
}

/// Windows'ta çapa klasörünün ve dosyasının sahibi ve izin listesi: ikisi de yöneticinin ve başkası değiştiremiyor
#[cfg(windows)]
fn windows_izinleri(klasor: &Path, yol: &Path) -> Result<(), String> {
    let kur = kur_komutu();
    pencere::izin_denetimi(klasor)
        .map_err(|e| tf!("çapa klasörü korunmuyor: {e} ({kur} izinleri sıkılaştırır)", "the anchor folder is not protected: {e} ({kur} tightens the permissions)"))?;
    pencere::izin_denetimi(yol)
        .map_err(|e| tf!("çapa dosyası korunmuyor: {e} ({kur} izinleri sıkılaştırır)", "the anchor file is not protected: {e} ({kur} tightens the permissions)"))
}

/// Kurulumdan sonra: dosya ve klasör yöneticiye ait ve başkası değiştiremiyor mu (kurulan süreç yönetici olduğu için
/// "bu süreç yazabiliyor" sayılmaz)
fn kurulum_denetimi(yol: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        korunuyor(yol)
    }
    #[cfg(windows)]
    {
        if !pencere::yonetici_sahipli(yol)? {
            return Err(t!(
                "dosyanın sahibi Administrators değil: terminal yönetici olarak açılmamış",
                "the file's owner is not Administrators: the terminal was not opened as administrator"
            )
            .into());
        }
        windows_izinleri(yol.parent().ok_or(t!("klasörü yok", "it has no folder"))?, yol)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = yol;
        Err(t!("bu işletim sisteminde çapa kurulamaz", "the anchor cannot be set up on this operating system").into())
    }
}

// ---- Windows: kullanıcı adı, sistem sürücüsü, dosya sahibi ve izinleri, konsol yankısı ----

#[cfg(windows)]
mod pencere {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        AclSizeInformation, AddAccessAllowedAceEx, CreateWellKnownSid, GetAce, GetAclInformation, GetLengthSid, InitializeAcl, IsWellKnownSid, WinBuiltinAdministratorsSid,
        WinBuiltinUsersSid, WinLocalSystemSid, ACE_HEADER, ACL, ACL_REVISION, ACL_SIZE_INFORMATION, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, INHERIT_ONLY_ACE,
        OBJECT_INHERIT_ACE, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, WELL_KNOWN_SID_TYPE,
    };
    use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_ECHO_INPUT, STD_INPUT_HANDLE};
    use windows_sys::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
    use windows_sys::Win32::System::WindowsProgramming::GetUserNameW;

    /// Oturumun kullanıcı adı (USERNAME ortam değişkeni değil)
    pub fn kullanici_adi() -> Option<String> {
        let mut b = [0u16; 257];
        let mut n = b.len() as u32;
        // SAFETY: tampon ve uzunluğu doğru; işlev en çok n karakter (sonlandırıcı dahil) yazar
        let ok = unsafe { GetUserNameW(b.as_mut_ptr(), &mut n) };
        (ok != 0 && n > 1).then(|| String::from_utf16_lossy(&b[..n as usize - 1]))
    }

    /// Sistem sürücüsünün kökünde bir klasör (ör. C:\ProgramData); Windows klasöründen, ortam değişkeninden değil
    pub fn sistem_suruculu(ad: &str) -> Option<PathBuf> {
        let mut b = [0u16; 260];
        // SAFETY: tampon ve uzunluğu doğru; dönen değer yazılan karakter sayısı
        let n = unsafe { GetSystemWindowsDirectoryW(b.as_mut_ptr(), b.len() as u32) } as usize;
        if n == 0 || n >= b.len() {
            return None;
        }
        let w = PathBuf::from(std::ffi::OsString::from_wide(&b[..n]));
        let kok: PathBuf = w.components().take(2).collect();
        Some(kok.join(ad))
    }

    /// Dosyanın sahibi Administrators grubu ya da SYSTEM mi
    pub fn yonetici_sahipli(yol: &Path) -> Result<bool, String> {
        let genis: Vec<u16> = yol.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut sahip: PSID = std::ptr::null_mut();
        let mut tanim: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: yol sıfırla biten geniş dizgi; çıkış işaretçileri geçerli; tanım LocalFree ile bırakılır
        let r = unsafe {
            GetNamedSecurityInfoW(
                genis.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut sahip,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tanim,
            )
        };
        if r != ERROR_SUCCESS {
            return Err(tf!("dosyanın sahibi okunamadı (hata {r})", "could not read the file's owner (error {r})"));
        }
        // SAFETY: sahip, tanımın içinde geçerli bir SID
        let yonetici = unsafe { IsWellKnownSid(sahip, WinBuiltinAdministratorsSid) != 0 || IsWellKnownSid(sahip, WinLocalSystemSid) != 0 };
        // SAFETY: tanım GetNamedSecurityInfoW'nun ayırdığı bellek
        unsafe { LocalFree(tanim as _) };
        Ok(yonetici)
    }

    // ---- İzinler (DACL) ----
    // Çapa klasörü kullanıcı tarafından önceden oluşturulmuşsa (ProgramData altında yeni klasör kullanıcıya dosya ekleme
    // hakkı ve CREATOR OWNER tam denetim mirası verir) klasör ya da çapa kullanıcıya yazılabilir kalabilir. Kurulum
    // klasörün ve dosyanın sahibini Administrators yapar, mirası keser ve izni açıkça yazar; doğrulama hem sahibi hem
    // izin listesini denetler.

    /// Tam denetim (FILE_ALL_ACCESS)
    pub const TAM: u32 = 0x001F_01FF;
    /// Yalnızca okuma ve klasörde gezinme (FILE_GENERIC_READ | FILE_GENERIC_EXECUTE)
    pub const OKUMA: u32 = 0x0012_00A9;
    /// Bu bitlerden birini veren izin nesneyi değiştirme hakkı sayılır: yazma ya da dosya ekleme (0x2), ekleme ya da alt
    /// klasör (0x4), genişletilmiş öznitelik (0x10), alt öğe silme (0x40), öznitelik (0x100), DELETE, WRITE_DAC,
    /// WRITE_OWNER, MAXIMUM_ALLOWED, GENERIC_ALL, GENERIC_WRITE
    pub const DEGISTIRIR: u32 =
        0x0000_0002 | 0x0000_0004 | 0x0000_0010 | 0x0000_0040 | 0x0000_0100 | 0x0001_0000 | 0x0004_0000 | 0x0008_0000 | 0x0200_0000 | 0x1000_0000 | 0x4000_0000;

    /// Bilinen bir SID (en çok SECURITY_MAX_SID_SIZE = 68 bayt; 4 bayt hizalı)
    pub struct Sid([u32; 17]);

    impl Sid {
        pub fn bilinen(tur: WELL_KNOWN_SID_TYPE) -> Result<Sid, String> {
            let mut s = Sid([0; 17]);
            let mut n = 68u32;
            // SAFETY: tampon 68 bayt ve uzunluğu n'de; alan SID'i gerekmeyen yerel bir tür
            let ok = unsafe { CreateWellKnownSid(tur, std::ptr::null_mut(), s.0.as_mut_ptr() as PSID, &mut n) };
            if ok == 0 {
                return Err(tf!("bilinen SID üretilemedi ({tur})", "could not build a well-known SID ({tur})"));
            }
            Ok(s)
        }
        pub fn ptr(&self) -> PSID {
            self.0.as_ptr() as PSID
        }
    }

    /// İzin listesi (ACL) tamponu: 4 bayt hizalı
    pub struct Acl(Vec<u32>);

    impl Acl {
        pub fn ptr(&self) -> *const ACL {
            self.0.as_ptr() as *const ACL
        }
    }

    /// Verilen (SID, erişim) çiftlerinden izin listesi. `klasor`: izinler alt dosya ve klasörlere de geçer
    pub fn dacl_kur(girdiler: &[(&Sid, u32)], klasor: bool) -> Result<Acl, String> {
        // ACL başlığı 8 bayt; her ACCESS_ALLOWED_ACE: başlık 4 + maske 4 + SID
        // SAFETY: SID'ler CreateWellKnownSid ile kurulmuş geçerli SID'ler
        let boy: u32 = 8 + girdiler.iter().map(|(s, _)| 8 + unsafe { GetLengthSid(s.ptr()) }).sum::<u32>();
        let mut a = Acl(vec![0u32; boy.div_ceil(4) as usize]);
        let bayrak = if klasor { OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE } else { 0 };
        // SAFETY: tampon en az `boy` bayt; girişler bu boya sığar
        unsafe {
            if InitializeAcl(a.0.as_mut_ptr() as *mut ACL, boy, ACL_REVISION) == 0 {
                return Err(t!("izin listesi kurulamadı", "could not build the access list").into());
            }
            for (sid, erisim) in girdiler {
                if AddAccessAllowedAceEx(a.0.as_mut_ptr() as *mut ACL, ACL_REVISION, bayrak, *erisim, sid.ptr()) == 0 {
                    return Err(t!("izin listesine giriş eklenemedi", "could not add an entry to the access list").into());
                }
            }
        }
        Ok(a)
    }

    /// Çapanın izni: Administrators ve SYSTEM tam denetim, Users (bu kullanıcı dahil) yalnızca okuma
    pub fn siki_dacl(klasor: bool) -> Result<Acl, String> {
        let (y, s, u) = (Sid::bilinen(WinBuiltinAdministratorsSid)?, Sid::bilinen(WinLocalSystemSid)?, Sid::bilinen(WinBuiltinUsersSid)?);
        dacl_kur(&[(&y, TAM), (&s, TAM), (&u, OKUMA)], klasor)
    }

    fn sid_metni(sid: PSID) -> String {
        let mut m: *mut u16 = std::ptr::null_mut();
        // SAFETY: sid geçerli; dönen dizgi sıfırla biter ve LocalFree ile bırakılır
        unsafe {
            if ConvertSidToStringSidW(sid, &mut m) == 0 || m.is_null() {
                return "?".into();
            }
            let n = (0..).take_while(|&i| *m.add(i) != 0).count();
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(m, n));
            LocalFree(m as _);
            s
        }
    }

    fn yonetici_sid(sid: PSID) -> bool {
        // SAFETY: sid geçerli bir SID
        unsafe { IsWellKnownSid(sid, WinBuiltinAdministratorsSid) != 0 || IsWellKnownSid(sid, WinLocalSystemSid) != 0 }
    }

    /// İzin listesinde Administrators ve SYSTEM dışında birine nesneyi değiştirme hakkı veren giriş: varsa kimin (SID).
    /// Liste yoksa (NULL DACL) herkes tam denetime sahiptir. Yalnızca alt öğelere geçen (INHERIT_ONLY) girişler nesnenin
    /// kendisine uygulanmaz; reddeden girişler hakkı yalnızca kısar; tanınmayan izin veren türler yabancı sayılır.
    ///
    /// # Safety
    /// `acl` boş ya da geçerli bir izin listesi olmalı
    pub unsafe fn yabanci_yazan(acl: *const ACL) -> Option<String> {
        if acl.is_null() {
            return Some(t!("izin listesi yok (herkes tam denetim)", "no access list (everyone has full control)").into());
        }
        let mut bilgi = ACL_SIZE_INFORMATION::default();
        // SAFETY: acl geçerli; bilgi yapısı doğru boyda
        if unsafe { GetAclInformation(acl, &mut bilgi as *mut _ as *mut _, std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32, AclSizeInformation) } == 0 {
            return Some(t!("izin listesi okunamadı", "could not read the access list").into());
        }
        for i in 0..bilgi.AceCount {
            let mut ace: *mut core::ffi::c_void = std::ptr::null_mut();
            // SAFETY: i < AceCount; ace listenin içindeki girişi gösterir
            if unsafe { GetAce(acl, i, &mut ace) } == 0 || ace.is_null() {
                return Some(t!("izin listesi okunamadı", "could not read the access list").into());
            }
            // SAFETY: her giriş ACE_HEADER ile başlar
            let baslik = unsafe { *(ace as *const ACE_HEADER) };
            // 1: ACCESS_DENIED_ACE_TYPE
            if u32::from(baslik.AceFlags) & INHERIT_ONLY_ACE != 0 || baslik.AceType == 1 {
                continue;
            }
            // 0: ACCESS_ALLOWED_ACE_TYPE
            if baslik.AceType != 0 {
                return Some(tf!("tanınmayan izin girişi (tür {})", "unrecognized access entry (type {})", baslik.AceType));
            }
            // SAFETY: ACCESS_ALLOWED_ACE: başlık (4 bayt), maske (4 bayt), ardından SID
            let (maske, sid) = unsafe { ((ace as *const u8).add(4).cast::<u32>().read_unaligned(), (ace as *mut u8).add(8) as PSID) };
            if maske & DEGISTIRIR != 0 && !yonetici_sid(sid) {
                return Some(sid_metni(sid));
            }
        }
        None
    }

    /// Dosya ya da klasörün güvenlik bilgisi: (sahibi Administrators ya da SYSTEM mi, sahibin SID'i, izin listesinde
    /// değiştirme hakkı olan yabancı)
    pub fn guvenlik_oku(yol: &Path) -> Result<(bool, String, Option<String>), String> {
        let genis: Vec<u16> = yol.as_os_str().encode_wide().chain(Some(0)).collect();
        let (mut sahip, mut dacl, mut tanim): (PSID, *mut ACL, PSECURITY_DESCRIPTOR) = (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: yol sıfırla biten geniş dizgi; çıkış işaretçileri geçerli; tanım LocalFree ile bırakılır
        let r = unsafe {
            GetNamedSecurityInfoW(
                genis.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut sahip,
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut tanim,
            )
        };
        if r != ERROR_SUCCESS {
            return Err(tf!("izinleri okunamadı (hata {r})", "could not read its permissions (error {r})"));
        }
        // SAFETY: sahip ve dacl tanımın içinde (dacl boş olabilir)
        let sonuc = (yonetici_sid(sahip), sid_metni(sahip), unsafe { yabanci_yazan(dacl) });
        // SAFETY: tanım GetNamedSecurityInfoW'nun ayırdığı bellek
        unsafe { LocalFree(tanim as _) };
        Ok(sonuc)
    }

    /// Dosya ya da klasörün sahibi ve izinleri: sahibi Administrators ya da SYSTEM ve başka kimse değiştiremiyor.
    /// Değilse neden (insana gösterilir)
    pub fn izin_denetimi(yol: &Path) -> Result<(), String> {
        match guvenlik_oku(yol)? {
            (false, sahip, _) => Err(tf!("sahibi Administrators ya da SYSTEM değil ({sahip})", "its owner is not Administrators or SYSTEM ({sahip})")),
            (true, _, Some(kim)) => Err(tf!("izni {kim} için değiştirme hakkı veriyor", "its permissions let {kim} change it")),
            (true, _, None) => Ok(()),
        }
    }

    /// İzin listesini mirası keserek yazar; `sahip` verilirse sahibi de değiştirir. Hata: Win32 kodu
    pub fn izin_yaz(yol: &Path, acl: &Acl, sahip: Option<&Sid>) -> Result<(), u32> {
        let genis: Vec<u16> = yol.as_os_str().encode_wide().chain(Some(0)).collect();
        let bilgi = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION | if sahip.is_some() { OWNER_SECURITY_INFORMATION } else { 0 };
        // SAFETY: yol sıfırla biten geniş dizgi; acl ve sid geçerli tamponlar
        let r = unsafe {
            SetNamedSecurityInfoW(genis.as_ptr(), SE_FILE_OBJECT, bilgi, sahip.map_or(std::ptr::null_mut(), Sid::ptr), std::ptr::null_mut(), acl.ptr(), std::ptr::null())
        };
        if r == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(r)
        }
    }

    /// Çapa klasörünü ya da dosyasını sıkılaştırır (yönetici süreçte): sahibi Administrators ya da SYSTEM değilse
    /// Administrators yapılır; miras kesilir, izin [`siki_dacl`]. Yapılamazsa insana ne yapacağı söylenir
    pub fn sikilastir(yol: &Path, klasor: bool) -> Result<(), String> {
        let acl = siki_dacl(klasor)?;
        let yonetici = Sid::bilinen(WinBuiltinAdministratorsSid)?;
        let sahip = (!yonetici_sahipli(yol)?).then_some(&yonetici);
        izin_yaz(yol, &acl, sahip).map_err(|r| {
            let y = yol.display();
            tf!(
                "{y} sıkılaştırılamadı (hata {r}: sahibi Administrators yapılamadı ya da izni yazılamadı). Yönetici terminalinde sahiplenip yeniden deneyin: takeown /F \"{y}\" /A /R; ya da klasörü silip yeniden kurun",
                "could not tighten {y} (error {r}: could not make Administrators its owner or write its permissions). In an administrator terminal take ownership and try again: takeown /F \"{y}\" /A /R; or delete the folder and set it up again"
            )
        })
    }

    /// Konsol yankısını kapatır; eski kip (konsol değilse None)
    pub fn yanki_kapat() -> Option<u32> {
        // SAFETY: standart girdinin tanıtıcısı; kip yalnızca okunur ve yankı biti kaldırılarak yazılır
        unsafe {
            let h = GetStdHandle(STD_INPUT_HANDLE);
            if h.is_null() || h == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut kip = 0u32;
            if GetConsoleMode(h, &mut kip) == 0 {
                return None;
            }
            SetConsoleMode(h, kip & !ENABLE_ECHO_INPUT);
            Some(kip)
        }
    }

    pub fn kip_geri(kip: u32) {
        // SAFETY: standart girdinin tanıtıcısına önceki kip yazılır
        unsafe {
            let h = GetStdHandle(STD_INPUT_HANDLE);
            if !h.is_null() && h != INVALID_HANDLE_VALUE {
                SetConsoleMode(h, kip);
            }
        }
    }
}
