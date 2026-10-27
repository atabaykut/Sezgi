//! `sezgi add mcp` ve `sezgi kaldir`: yapay zekâ araçlarına izinli bağlantı ve iz bırakmadan geri alma (vizyon 4.4.6).
//!
//! - `init` hiçbir aracın ayarına dokunmaz; bağlantı yalnızca `add mcp` ile ve araç araç onayla eklenir.
//!   Hiçbir araç önceden seçili değildir; her araç için dosya, kapsam ve eklenecek satır sayısı gösterilir.
//! - Bilinen araçlar ve ayar dosyaları bir veri listesindedir (`araclar/mcp.toml`).
//! - Kullanıcının kendi girdilerine dokunulmaz: aynı adla başka bir girdi varsa, dosya düz JSON / TOML değilse ya da
//!   sembolik bağsa o araç atlanır ve bağlantı bilgisi elle eklensin diye gösterilir.
//! - Her değişiklikten önce dosya yedeklenir ve kayda (yapılandırma klasöründe `eklemeler.json`, proje dışında) yazılır.
//! - `kaldir`: dosya Sezgi'nin bıraktığı hâlindeyse yedekten birebir geri yüklenir (Sezgi'nin oluşturduğu dosya ve
//!   klasörler silinir); sonradan değiştirildiyse yalnızca Sezgi'nin eklediği girdi silinir; o da bulunamazsa dosyaya
//!   dokunulmaz ve kullanıcıya söylenir. Harita klasörü yalnızca açıkça istenirse silinir.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::imza;

#[derive(Deserialize, Clone)]
struct Arac {
    kimlik: String,
    ad: String,
    kapsam: String,
    dosya: String,
    bicim: String,
    #[serde(default)]
    kok: Option<String>,
    #[serde(default)]
    tip: Option<String>,
    #[serde(default)]
    belirti: Vec<String>,
    #[serde(default)]
    talimat: Option<String>,
}

#[derive(Deserialize)]
struct Liste {
    arac: Vec<Arac>,
}

fn araclar() -> Vec<Arac> {
    toml::from_str::<Liste>(include_str!("../araclar/mcp.toml")).expect("araclar/mcp.toml geçersiz").arac
}

/// Talimat dosyasına eklenen yönlendirme (işaretli blok: kaldir yalnızca bunu siler)
const TALIMAT_BASI: &str = "<!-- sezgi:basla (sezgi add mcp ekledi; sezgi kaldir siler) -->";
const TALIMAT: &str = "<!-- sezgi:basla (sezgi add mcp ekledi; sezgi kaldir siler) -->\nBefore scanning code for this application's endpoints, inputs, rules, auth or database tables, ask the Sezgi MCP tools first (kimsin, ara, tani, onizle).\n<!-- sezgi:bitir -->\n";

/// Haritada Sezgi'nin yazdığı dosyalar: `kaldir --harita-sil` yalnızca bunları siler
const HARITA_DOSYALARI: [&str; 11] = [
    "kapilar.json",
    "benlik.json",
    "benlik-keys.json",
    "benlik-evidence.json",
    "openapi.json",
    "web-baglari.json",
    "kanit.json",
    "graf.json",
    "paylasim.bin",
    ".gitattributes",
    ".gitignore",
];

// ---- Yollar ----

fn ev() -> Option<PathBuf> {
    let adlar: &[&str] = if cfg!(windows) { &["USERPROFILE", "HOME"] } else { &["HOME"] };
    adlar.iter().find_map(|a| std::env::var_os(a).filter(|v| !v.is_empty()).map(PathBuf::from))
}

fn ayar() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        ev().map(|e| e.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| ev().map(|e| e.join(".config")))
    }
}

/// "{ev}/.codex/config.toml" → mutlak yol (bölücüler platformun)
fn genislet(sablon: &str, proje: &Path) -> Option<PathBuf> {
    let mut parcalar = sablon.split('/');
    let mut p = match parcalar.next()? {
        "{proje}" => proje.to_path_buf(),
        "{ev}" => ev()?,
        "{ayar}" => ayar()?,
        _ => return None,
    };
    for x in parcalar {
        p.push(x);
    }
    Some(p)
}

fn komut_var(ad: &str) -> bool {
    let uzantilar: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|k| uzantilar.iter().any(|u| k.join(format!("{ad}{u}")).is_file()))).unwrap_or(false)
}

fn kurulu(a: &Arac, proje: &Path) -> bool {
    a.belirti.iter().any(|b| match b.strip_prefix("komut:") {
        Some(k) => komut_var(k),
        None => genislet(b, proje).is_some_and(|p| p.is_dir()),
    })
}

fn duz(p: &Path) -> String {
    p.display().to_string().trim_start_matches(r"\\?\").to_string()
}

fn mutlak(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).map(|c| PathBuf::from(duz(&c))).unwrap_or_else(|_| p.to_path_buf())
}

/// Yolu ev klasörüne göre kısaltarak gösterir (çıktıda kullanıcı adı tekrar tekrar görünmesin)
fn goster(p: &Path, proje: &Path) -> String {
    if let Ok(g) = p.strip_prefix(proje) {
        return g.display().to_string();
    }
    match ev().and_then(|e| p.strip_prefix(e).ok().map(Path::to_path_buf)) {
        Some(g) => format!("~{}{}", std::path::MAIN_SEPARATOR, g.display()),
        None => p.display().to_string(),
    }
}

// ---- Sunucu tanımı ----

struct Sunucu {
    komut: String,
    argumanlar: Vec<String>,
}

/// MCP sunucusunun komutu ve argümanları: `bagla`, `bagla --elle`, `token olustur` ve Ayna bunu kullanır. İkili bir
/// `node_modules` içindeyse (npx önbelleği ya da projenin yerel kurulumu) mutlak yolu yazılmaz: önbellek temizlenince ya
/// da `node_modules` silinince bağlantı kırılırdı. O zaman bağlantı npx üzerinden, sürümü sabitlenerek kurulur
fn sunucu(harita: &Path) -> Sunucu {
    let exe = std::env::current_exe().ok().map(|p| mutlak(&p)).unwrap_or_else(|| PathBuf::from("sezgi"));
    let harita = duz(&mutlak(harita));
    if exe.components().any(|c| c.as_os_str() == "_npx" || c.as_os_str() == "node_modules") {
        let mut argumanlar = vec!["-y".to_string(), format!("sezgi@{}", env!("CARGO_PKG_VERSION")), "mcp".into(), harita];
        if cfg!(windows) {
            argumanlar.splice(0..0, ["/c".to_string(), "npx".to_string()]);
            return Sunucu { komut: "cmd".into(), argumanlar };
        }
        return Sunucu { komut: "npx".into(), argumanlar };
    }
    Sunucu { komut: duz(&exe), argumanlar: vec!["mcp".into(), harita] }
}

/// Kabuk satırında boşluk içeren argüman tırnaklanır
fn tirnakli(x: &str) -> String {
    if x.contains(' ') {
        format!("\"{x}\"")
    } else {
        x.to_string()
    }
}

/// Claude Code'a bağlantı satırı (token verilirse ortam değişkeniyle): `claude mcp add sezgi [-e SEZGI_TOKEN=…] -- …`
pub(crate) fn claude_satiri(harita: &Path, token: Option<&str>) -> String {
    let s = sunucu(harita);
    let ortam = token.map(|t| format!("-e SEZGI_TOKEN={t} ")).unwrap_or_default();
    let argumanlar: Vec<String> = s.argumanlar.iter().map(|a| tirnakli(a)).collect();
    format!("claude mcp add sezgi {ortam}-- {} {}", tirnakli(&s.komut), argumanlar.join(" "))
}

/// Listede olmayan MCP araçları için komut ve argümanlar (okunur metin)
pub(crate) fn komut_ve_argumanlar(harita: &Path) -> (String, String) {
    let s = sunucu(harita);
    (tirnakli(&s.komut), s.argumanlar.iter().map(|a| tirnakli(a)).collect::<Vec<_>>().join(" "))
}

fn girdi_adi(a: &Arac, proje: &Path) -> String {
    if a.kapsam == "proje" {
        return "sezgi".into();
    }
    // Genel dosyada birden fazla proje olabilir: ad projeye özgü
    let ham: String = proje.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let ad = ham.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    if ad.is_empty() {
        "sezgi".into()
    } else {
        format!("sezgi-{ad}")
    }
}

// ---- Biçimler ----

/// Dosyanın girintisi (ilk girintili satırdan); bulunamazsa 2 boşluk
fn girinti(m: &str) -> String {
    m.lines()
        .find_map(|s| {
            let g: String = s.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            (!g.is_empty() && g.len() < s.len()).then_some(g)
        })
        .unwrap_or_else(|| "  ".into())
}

fn json_yaz(v: &Value, ornek: Option<&str>) -> String {
    let g = ornek.map(girinti).unwrap_or_else(|| "  ".into());
    let mut tampon = Vec::new();
    let bicim = serde_json::ser::PrettyFormatter::with_indent(g.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut tampon, bicim);
    v.serialize(&mut ser).unwrap();
    let mut s = String::from_utf8(tampon).unwrap() + "\n";
    if ornek.is_some_and(|m| m.contains("\r\n")) {
        s = s.replace('\n', "\r\n");
    }
    s
}

fn json_deger(a: &Arac, s: &Sunucu) -> Value {
    let mut d = serde_json::Map::new();
    if let Some(t) = &a.tip {
        d.insert("type".into(), json!(t));
    }
    d.insert("command".into(), json!(s.komut));
    d.insert("args".into(), json!(s.argumanlar));
    Value::Object(d)
}

fn toml_metin(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

fn ozet(b: &[u8]) -> String {
    // FNV-1a 64: yalnızca "dosya Sezgi'nin bıraktığı gibi mi" sorusu için (güvenlik özeti değil)
    let mut h: u64 = 0xcbf29ce484222325;
    for x in b {
        h ^= u64::from(*x);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}-{}", b.len())
}

// ---- Plan ----

enum Plan {
    Ekle { yeni: String, satir: usize, ekli_metin: Option<String>, kok_olusturuldu: bool, deger: Option<Value> },
    ZatenVar,
    Engel(String),
}

fn mevcut(dosya: &Path) -> Result<Option<String>, String> {
    if dosya.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(t!("dosya sembolik bağ; bağlantıyı elle ekleyin", "the file is a symbolic link; add the connection by hand").into());
    }
    match std::fs::read_to_string(dosya) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(tf!("okunamadı ({e}); bağlantıyı elle ekleyin", "could not be read ({e}); add the connection by hand")),
    }
}

fn mcp_plani(a: &Arac, dosya: &Path, ad: &str, s: &Sunucu) -> Plan {
    let eski = match mevcut(dosya) {
        Ok(m) => m,
        Err(e) => return Plan::Engel(e),
    };
    if a.bicim == "codex-toml" {
        if let Some(m) = &eski {
            match m.parse::<toml::Table>() {
                Ok(t) => {
                    if let Some(v) = t.get("mcp_servers").and_then(|v| v.get(ad)) {
                        let ayni = v.get("command").and_then(|c| c.as_str()) == Some(s.komut.as_str())
                            && v.get("args").and_then(|x| x.as_array()).map(|x| x.iter().filter_map(|y| y.as_str()).collect::<Vec<_>>())
                                == Some(s.argumanlar.iter().map(String::as_str).collect());
                        return if ayni {
                            Plan::ZatenVar
                        } else {
                            Plan::Engel(tf!("'{ad}' adlı başka bir girdi var; dokunulmadı", "another entry named '{ad}' exists; left untouched"))
                        };
                    }
                }
                Err(_) => return Plan::Engel(t!("dosya geçerli TOML değil; bağlantıyı elle ekleyin", "the file is not valid TOML; add the connection by hand").into()),
            }
        }
        let blok = format!(
            "# sezgi add mcp ekledi; geri almak için: sezgi kaldir\n[mcp_servers.{ad}]\ncommand = {}\nargs = [{}]\n",
            toml_metin(&s.komut),
            s.argumanlar.iter().map(|x| toml_metin(x)).collect::<Vec<_>>().join(", ")
        );
        let onceki = eski.clone().unwrap_or_default();
        let ayrac = if onceki.is_empty() {
            ""
        } else if onceki.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        let ekli = format!("{ayrac}{blok}");
        let yeni = format!("{onceki}{ekli}");
        if yeni.parse::<toml::Table>().is_err() {
            return Plan::Engel(
                t!(
                    "eklenen blok dosyanın yapısıyla çakışıyor (ör. satır içi mcp_servers); bağlantıyı elle ekleyin",
                    "the added block conflicts with the file's structure (e.g. inline mcp_servers); add the connection by hand"
                )
                .into(),
            );
        }
        return Plan::Ekle { yeni, satir: blok.lines().count(), ekli_metin: Some(ekli), kok_olusturuldu: false, deger: None };
    }

    let mut kok: Value = match eski.as_deref() {
        None => json!({}),
        Some(m) if m.trim().is_empty() => json!({}),
        Some(m) => match serde_json::from_str::<Value>(m) {
            Ok(v) if v.is_object() => v,
            _ => {
                return Plan::Engel(
                    t!(
                        "dosya düz JSON değil (yorum ya da sondaki virgül olabilir); bağlantıyı elle ekleyin",
                        "the file is not plain JSON (it may have comments or a trailing comma); add the connection by hand"
                    )
                    .into(),
                )
            }
        },
    };
    let deger = json_deger(a, s);
    let kok_ad = a.kok.as_deref().unwrap_or("mcpServers");
    let nesne = kok.as_object_mut().unwrap();
    let kok_olusturuldu = !nesne.contains_key(kok_ad);
    let Some(kap) = nesne.entry(kok_ad).or_insert(json!({})).as_object_mut() else {
        return Plan::Engel(tf!("'{kok_ad}' bir nesne değil; dokunulmadı", "'{kok_ad}' is not an object; left untouched"));
    };
    if let Some(v) = kap.get(ad) {
        return if *v == deger { Plan::ZatenVar } else { Plan::Engel(tf!("'{ad}' adlı başka bir girdi var; dokunulmadı", "another entry named '{ad}' exists; left untouched")) };
    }
    kap.insert(ad.to_string(), deger.clone());
    let yeni = json_yaz(&kok, eski.as_deref().filter(|m| !m.trim().is_empty()));
    let satir = serde_json::to_string_pretty(&json!({ ad: deger })).unwrap().lines().count() - 2;
    Plan::Ekle { yeni, satir, ekli_metin: None, kok_olusturuldu, deger: Some(deger) }
}

fn talimat_plani(dosya: &Path) -> Plan {
    let eski = match mevcut(dosya) {
        Ok(m) => m.unwrap_or_default(),
        Err(e) => return Plan::Engel(e),
    };
    if eski.contains(TALIMAT_BASI) {
        return Plan::ZatenVar;
    }
    let ayrac = if eski.is_empty() {
        ""
    } else if eski.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let ekli = format!("{ayrac}{TALIMAT}");
    Plan::Ekle { yeni: format!("{eski}{ekli}"), satir: TALIMAT.lines().count(), ekli_metin: Some(ekli), kok_olusturuldu: false, deger: None }
}

// ---- Kayıt ----

#[derive(Serialize, Deserialize, Clone)]
struct Kayit {
    proje: String,
    arac: String,
    tur: String,
    dosya: String,
    bicim: String,
    #[serde(default)]
    kok: Option<String>,
    #[serde(default)]
    ad: Option<String>,
    #[serde(default)]
    deger: Option<Value>,
    #[serde(default)]
    ekli_metin: Option<String>,
    onceden_vardi: bool,
    kok_olusturuldu: bool,
    olusturulan_klasorler: Vec<String>,
    yedek: Option<String>,
    sonra_ozet: String,
}

fn defter_yolu() -> Result<PathBuf, String> {
    imza::yapilandirma_klasoru().map(|k| k.join("eklemeler.json")).ok_or_else(|| {
        t!("yapılandırma klasörü bulunamadı (APPDATA / XDG_CONFIG_HOME / HOME tanımlı değil)", "config folder not found (APPDATA / XDG_CONFIG_HOME / HOME is not set)").into()
    })
}

fn defter_oku() -> Result<Vec<Kayit>, String> {
    let yol = defter_yolu()?;
    match std::fs::read_to_string(&yol) {
        Ok(m) => serde_json::from_str::<Value>(&m)
            .ok()
            .and_then(|v| serde_json::from_value(v["kayitlar"].clone()).ok())
            .ok_or_else(|| tf!("kayıt okunamadı: {}", "could not read the record: {}", yol.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(tf!("kayıt okunamadı ({e}): {}", "could not read the record ({e}): {}", yol.display())),
    }
}

fn defter_yaz(k: &[Kayit]) -> Result<(), String> {
    let yol = defter_yolu()?;
    std::fs::create_dir_all(yol.parent().unwrap()).map_err(|e| e.to_string())?;
    let metin = serde_json::to_string_pretty(&json!({ "surum": 1, "kayitlar": k })).unwrap();
    atomik_yaz(&yol, metin.as_bytes())
}

fn atomik_yaz(yol: &Path, icerik: &[u8]) -> Result<(), String> {
    let gecici = yol.with_file_name(format!(".{}.sezgi-gecici", yol.file_name().unwrap().to_string_lossy()));
    std::fs::write(&gecici, icerik).map_err(|e| format!("{}: {e}", yol.display()))?;
    std::fs::rename(&gecici, yol).map_err(|e| {
        let _ = std::fs::remove_file(&gecici);
        format!("{}: {e}", yol.display())
    })
}

struct Is {
    arac: String,
    tur: &'static str,
    dosya: PathBuf,
    bicim: String,
    kok: Option<String>,
    ad: Option<String>,
    plan: Plan,
}

fn uygula(proje: &Path, is: Is) -> Result<Kayit, String> {
    let Plan::Ekle { yeni, ekli_metin, kok_olusturuldu, deger, .. } = is.plan else { unreachable!() };
    let onceden_vardi = is.dosya.exists();
    let mut olusturulan = vec![];
    let mut k = is.dosya.parent();
    while let Some(d) = k {
        if d.exists() {
            break;
        }
        olusturulan.push(duz(d));
        k = d.parent();
    }
    let yedek = if onceden_vardi {
        let klasor = imza::yapilandirma_klasoru().ok_or(t!("yapılandırma klasörü yok", "no config folder"))?.join("yedekler");
        std::fs::create_dir_all(&klasor).map_err(|e| e.to_string())?;
        let an = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let y = klasor.join(format!("{an}-{}-{}", is.arac, is.dosya.file_name().unwrap().to_string_lossy()));
        std::fs::copy(&is.dosya, &y).map_err(|e| tf!("yedek alınamadı ({e}); dosyaya dokunulmadı", "could not make a backup ({e}); the file was left untouched"))?;
        Some(duz(&y))
    } else {
        None
    };
    std::fs::create_dir_all(is.dosya.parent().unwrap()).map_err(|e| e.to_string())?;
    atomik_yaz(&is.dosya, yeni.as_bytes())?;
    Ok(Kayit {
        proje: duz(proje),
        arac: is.arac,
        tur: is.tur.into(),
        dosya: duz(&is.dosya),
        bicim: is.bicim,
        kok: is.kok,
        ad: is.ad,
        deger,
        ekli_metin,
        onceden_vardi,
        kok_olusturuldu,
        olusturulan_klasorler: olusturulan,
        yedek,
        sonra_ozet: ozet(yeni.as_bytes()),
    })
}

// ---- Komutlar ----

fn secenek(args: &[String], ad: &str) -> Option<String> {
    args.iter().position(|a| a == ad).and_then(|i| args.get(i + 1)).cloned()
}

/// Yalnızca açık evet onaydır; boş Enter ve başka her şey hayır
fn sor(soru: &str) -> bool {
    if crate::dil::en() {
        print!("  {soru} (yes/N) ");
    } else {
        print!("  {soru} (evet/H) ");
    }
    let _ = std::io::stdout().flush();
    let mut cevap = String::new();
    if std::io::stdin().lock().read_line(&mut cevap).is_err() {
        return false;
    }
    matches!(cevap.trim().to_lowercase().as_str(), "e" | "evet" | "y" | "yes")
}

/// Geri alınamaz işlem için onay: `--evet` verildiyse evet; terminalde sorulur; terminal yoksa hayır
pub fn onaylandi(args: &[String], soru: &str) -> bool {
    args.iter().any(|a| a == "--evet") || (std::io::stdin().is_terminal() && sor(soru))
}

/// Komutun önerisinde proje yolu: bulunulan klasörse yazılmaz
fn proje_eki(proje: &Path) -> String {
    let burasi = std::env::current_dir().ok().map(|c| mutlak(&c));
    if burasi.as_deref() == Some(proje) {
        String::new()
    } else {
        format!(" {}", duz(proje))
    }
}

fn dur(mesaj: &str, kod: i32) -> ! {
    eprintln!("✗ {mesaj}");
    std::process::exit(kod)
}

/// sezgi add mcp [harita-klasörü=.sezgi] [--proje <klasör>] [--arac a,b] [--yonlendir] [--deneme]
pub fn ekle_mcp(args: &[String]) {
    let harita = crate::guvenli::harita_bul(args.get(2).filter(|a| !a.starts_with("--")).map(String::as_str));
    if !harita.join("kapilar.json").is_file() {
        dur(&tf!("Harita bulunamadı: {} (önce uygulamayı haritalayın: sezgi init)", "Map not found: {} (map the application first: sezgi init)", harita.display()), 1);
    }
    let harita = mutlak(&harita);
    let proje = match secenek(args, "--proje") {
        Some(p) => mutlak(Path::new(&p)),
        None if harita.file_name().is_some_and(|a| a == ".sezgi") => harita.parent().unwrap().to_path_buf(),
        None => mutlak(Path::new(".")),
    };
    let deneme = args.iter().any(|a| a == "--deneme");
    let liste = araclar();
    let istenen: Option<Vec<String>> = secenek(args, "--arac").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect());
    if let Some(i) = &istenen {
        let bilinmeyen: Vec<&String> = i.iter().filter(|k| !liste.iter().any(|a| &a.kimlik == *k)).collect();
        if !bilinmeyen.is_empty() || i.is_empty() {
            dur(
                &tf!(
                    "Bilinmeyen araç: {} (bilinenler: {})",
                    "Unknown tool: {} (known: {})",
                    bilinmeyen.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
                    liste.iter().map(|a| a.kimlik.as_str()).collect::<Vec<_>>().join(", ")
                ),
                2,
            );
        }
    }
    let s = sunucu(&harita);
    // --elle: hiçbir dosyaya yazmadan, kullanıcının kendisinin ekleyeceği satırlar
    if args.iter().any(|a| a == "--elle") {
        tp!("Elle bağlamak için (Sezgi hiçbir dosyaya yazmaz):", "To connect by hand (Sezgi writes no file):");
        println!("  Claude Code:  {}", claude_satiri(&harita, None));
        tp!("  Diğer araçlar (ayar dosyasındaki mcpServers'a):", "  Other tools (into mcpServers in the settings file):");
        println!("    {}", serde_json::json!({ "sezgi": { "command": s.komut, "args": s.argumanlar } }));
        return;
    }

    tp!(
        "Neden: Yapay zekâ araçları hangi MCP sunucularına bağlanacaklarını kendi ayar dosyalarında tutar.",
        "Why: AI tools keep the MCP servers they connect to in their own settings files."
    );
    tp!(
        "Sezgi bu listeye eklenmezse araç Sezgi'yi göremez. İsterseniz bağlantıyı sizin yerinize ekleyebilirim.",
        "If Sezgi is not on that list, the tool cannot see Sezgi. If you like, I can add the connection for you."
    );
    tp!(
        "Hiçbir araç önceden seçili değildir; her ekleme yedeklenir ve `sezgi kaldir` ile geri alınır.\n",
        "No tool is preselected; every change is backed up and `sezgi remove` undoes it.\n"
    );

    // Plan: istenen araçlar ya da bu bilgisayarda bulunanlar
    let mut adaylar: Vec<(Arac, PathBuf, String, Plan)> = vec![];
    for a in &liste {
        let secili = istenen.as_ref().is_some_and(|i| i.contains(&a.kimlik));
        if !secili && (istenen.is_some() || !kurulu(a, &proje)) {
            continue;
        }
        let Some(dosya) = genislet(&a.dosya, &proje) else { continue };
        let ad = girdi_adi(a, &proje);
        let plan = mcp_plani(a, &dosya, &ad, &s);
        adaylar.push((a.clone(), dosya, ad, plan));
    }
    if istenen.is_none() {
        if adaylar.is_empty() {
            tp!("Bu bilgisayarda listedeki araçlardan hiçbiri bulunamadı.", "None of the listed tools was found on this computer.");
        } else {
            tp!("Bu bilgisayarda MCP destekleyen {} araç bulundu:\n", "Found {} {a} with MCP support on this computer:\n", adaylar.len(); a = crate::dil::cogul(adaylar.len(), "tool", "tools"));
        }
    }
    for (a, dosya, _, plan) in &adaylar {
        let kapsam = if a.kapsam == "genel" { t!("tüm projeler", "all projects") } else { t!("bu proje", "this project") };
        let durum = match plan {
            Plan::Ekle { satir, .. } => tf!("{satir} satır eklenecek", "{satir} {l} will be added"; l = crate::dil::cogul(satir, "line", "lines")),
            Plan::ZatenVar => t!("zaten ekli", "already added").into(),
            Plan::Engel(e) => tf!("eklenemez: {e}", "cannot be added: {e}"),
        };
        println!("  {:<18} → {} ({kapsam})  {durum}", a.ad, goster(dosya, &proje));
        if a.kapsam == "genel" {
            tp!("  {:<18}   ⚠ Bu dosya bütün projelerinizi etkiler.", "  {:<18}   ⚠ This file affects all your projects.", "");
        }
    }
    tp!("\n  Listede olmayan araçlar için MCP sunucusu (stdio):", "\n  MCP server (stdio) for tools not on the list:");
    tp!("    komut       {}", "    command     {}", s.komut);
    tp!("    argümanlar  {}", "    arguments   {}", s.argumanlar.join("  "));

    // Seçim: --arac açık seçimdir; yoksa etkileşimli terminalde her araç tek tek sorulur
    let etkilesimli = std::io::stdin().is_terminal() && !deneme;
    let mut secilen: Vec<usize> = vec![];
    for (i, (a, dosya, _, plan)) in adaylar.iter().enumerate() {
        if !matches!(plan, Plan::Ekle { .. }) {
            continue;
        }
        if istenen.is_some() || (etkilesimli && sor(&tf!("{} → {} eklensin mi?", "Add {} → {}?", a.ad, goster(dosya, &proje)))) {
            secilen.push(i);
        }
    }
    if istenen.is_none() && !etkilesimli {
        tp!(
            "\nSeçim yapılmadı: hiçbir dosya değişmedi. Eklemek için: sezgi bagla {} --arac <{}>",
            "\nNothing selected: no file changed. To add: sezgi connect {} --tool <{}>",
            duz(&harita),
            liste.iter().map(|a| a.kimlik.as_str()).collect::<Vec<_>>().join("|")
        );
        return;
    }

    // Yönlendirme: ayrı sorulur (ya da --yonlendir), talimat dosyası başına bir kez
    let yonlendir = args.iter().any(|a| a == "--yonlendir");
    let mut talimatlar: Vec<(String, PathBuf, Plan)> = vec![];
    // Bağlantısı zaten ekli araçlara da yönlendirme eklenebilir
    let zaten: Vec<usize> =
        (0..adaylar.len()).filter(|&i| matches!(adaylar[i].3, Plan::ZatenVar) && (etkilesimli || istenen.as_ref().is_some_and(|x| x.contains(&adaylar[i].0.kimlik)))).collect();
    for &i in secilen.iter().chain(zaten.iter()) {
        let a = &adaylar[i].0;
        let Some(t) = &a.talimat else { continue };
        let dosya = proje.join(t);
        if talimatlar.iter().any(|(_, d, _)| d == &dosya) {
            continue;
        }
        let plan = talimat_plani(&dosya);
        if !matches!(plan, Plan::Ekle { .. }) {
            continue;
        }
        if yonlendir
            || (etkilesimli && {
                tp!(
                    "\n  Neden: Bağlı olsa bile yapay zekâ alışkanlıkla kodu taramaya başlayabilir. {t} dosyasına tek bir",
                    "\n  Why: even when connected, the AI may start scanning the code out of habit. If a single"
                );
                tp!(
                    "  \"önce Sezgi'ye sor\" yönergesi eklenirse önce Sezgi'ye sorar. Kendi yazdıklarınıza dokunulmaz.",
                    "  \"ask Sezgi first\" instruction is added to {t}, it asks Sezgi first. What you wrote stays untouched."
                );
                sor(&tf!("{} için {t} dosyasına yönlendirme eklensin mi?", "Add the instruction to {t} for {}?", a.ad))
            })
        {
            talimatlar.push((a.kimlik.clone(), dosya, plan));
        }
    }

    if deneme {
        tp!("\nDeneme: hiçbir dosya yazılmadı. Yazılacaklar:", "\nDry run: no file written. Would write:");
        for &i in &secilen {
            println!("  {} → {}", adaylar[i].0.ad, goster(&adaylar[i].1, &proje));
        }
        for (_, d, _) in &talimatlar {
            tp!("  yönlendirme → {}", "  instruction → {}", goster(d, &proje));
        }
        return;
    }

    let mut defter = defter_oku().unwrap_or_else(|e| dur(&e, 1));
    let mut isler: Vec<(String, Is)> = vec![];
    let mut adaylar = adaylar;
    for i in secilen.into_iter().rev() {
        let (a, dosya, ad, plan) = adaylar.remove(i);
        let is = Is { arac: a.kimlik.clone(), tur: "mcp", dosya, bicim: a.bicim.clone(), kok: a.kok.clone(), ad: Some(ad), plan };
        isler.insert(0, (a.ad.clone(), is));
    }
    for (kimlik, dosya, plan) in talimatlar {
        isler.push((t!("yönlendirme", "instruction").into(), Is { arac: kimlik, tur: "talimat", dosya, bicim: "metin".into(), kok: None, ad: None, plan }));
    }
    println!();
    let mut hata = false;
    let mut eklenen = 0usize;
    for (ad, is) in isler {
        let gorunen = goster(&is.dosya, &proje);
        let satir = match &is.plan {
            Plan::Ekle { satir, .. } => *satir,
            _ => 0,
        };
        match uygula(&proje, is) {
            Ok(k) => {
                let yedek = if k.yedek.is_some() { t!(", yedek alındı", ", backed up") } else { t!(", yeni dosya", ", new file") };
                defter.push(k);
                // Her değişiklikten hemen sonra kayıt: yarıda kesilse bile kaldir neyi geri alacağını bilir
                if let Err(e) = defter_yaz(&defter) {
                    dur(&tf!("Kayıt yazılamadı ({e}); {gorunen} değişti, elle geri alın", "Could not write the record ({e}); {gorunen} changed, undo it by hand"), 1);
                }
                eklenen += 1;
                tp!("  ✓ {ad} → {gorunen} (+{satir} satır{yedek})", "  ✓ {ad} → {gorunen} (+{satir} {l}{yedek})"; l = crate::dil::cogul(satir, "line", "lines"));
            }
            Err(e) => {
                hata = true;
                println!("  ✗ {ad} → {gorunen}: {e}");
            }
        }
    }
    for (a, dosya, _, plan) in &adaylar {
        if let (Plan::Engel(e), true) = (plan, istenen.as_ref().is_some_and(|i| i.contains(&a.kimlik))) {
            hata = true;
            println!("  ✗ {} → {}: {e}", a.ad, goster(dosya, &proje));
        } else if matches!(plan, Plan::ZatenVar) && istenen.as_ref().is_some_and(|i| i.contains(&a.kimlik)) {
            tp!("  · {} → {}: zaten ekli", "  · {} → {}: already added", a.ad, goster(dosya, &proje));
        }
    }
    // Sonuç olduğu gibi söylenir: hiçbir şey eklenmediyse "yeniden başlatın" denmez
    let zaten_ekli = adaylar.iter().any(|(_, _, _, p)| matches!(p, Plan::ZatenVar));
    if eklenen > 0 {
        tp!("\nAracı yeniden başlatın. Geri almak için: sezgi kaldir{}", "\nRestart the tool. To undo: sezgi remove{}", proje_eki(&proje));
    } else if hata {
        tp!("\nHiçbir bağlantı eklenmedi. Elle eklemek için: sezgi bagla --elle", "\nNo connection added. To add by hand: sezgi connect --manual");
    } else if zaten_ekli {
        tp!(
            "\nDeğişiklik yok: bağlantı zaten ekli. Araç Sezgi'yi görmüyorsa durumu görmek için: sezgi doktor",
            "\nNo change: the connection is already there. If the tool does not see Sezgi, check the state with: sezgi doctor"
        );
    } else if adaylar.is_empty() {
        tp!(
            "\nHiçbir bağlantı eklenmedi: listedeki araçlardan hiçbiri bulunamadı. Elle eklemek için: sezgi bagla --elle",
            "\nNo connection added: none of the listed tools was found. To add by hand: sezgi connect --manual"
        );
        std::process::exit(1);
    } else {
        tp!("\nHiçbir bağlantı eklenmedi; hiçbir dosya değişmedi.", "\nNo connection added; no file changed.");
    }
    if hata {
        std::process::exit(1);
    }
}

fn bos_klasorleri_sil(k: &Kayit) {
    // En derinden başlayarak: yalnızca Sezgi'nin oluşturduğu ve boş kalan klasörler
    for d in &k.olusturulan_klasorler {
        let _ = std::fs::remove_dir(d);
    }
}

/// Bir eklemeyi geri alır; sonuç cümlesi ve kaydın silinip silinmeyeceği döner
fn geri_al(k: &Kayit, deneme: bool) -> (String, bool) {
    let yol = PathBuf::from(&k.dosya);
    if yol.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        return (t!("sonradan sembolik bağ olmuş; dokunulmadı", "became a symbolic link later; left untouched").into(), false);
    }
    let simdi = match std::fs::read(&yol) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (t!("dosya zaten yok", "the file is already gone").into(), true),
        Err(e) => return (tf!("okunamadı ({e}); dokunulmadı", "could not be read ({e}); left untouched"), false),
    };
    if ozet(&simdi) == k.sonra_ozet {
        if deneme {
            return (t!("birebir geri yüklenecek", "will be restored exactly").into(), false);
        }
        let sonuc = match &k.yedek {
            Some(y) => std::fs::copy(y, &yol).map(|_| {
                let _ = std::fs::remove_file(y);
            }),
            None => std::fs::remove_file(&yol).map(|_| bos_klasorleri_sil(k)),
        };
        return match sonuc {
            Ok(()) if k.yedek.is_some() => (t!("birebir geri yüklendi", "restored exactly").into(), true),
            Ok(()) => (t!("silindi (Sezgi oluşturmuştu)", "deleted (Sezgi had created it)").into(), true),
            Err(e) => (tf!("geri yüklenemedi ({e})", "could not be restored ({e})"), false),
        };
    }
    // Dosya sonradan değişmiş: yalnızca Sezgi'nin eklediği kısım çıkarılır
    let metin = String::from_utf8_lossy(&simdi).to_string();
    let yeni: Option<String> = if k.bicim == "json" {
        let (Some(kok_ad), Some(ad), Some(deger)) = (&k.kok, &k.ad, &k.deger) else { return (t!("kayıt eksik; dokunulmadı", "record incomplete; left untouched").into(), false) };
        let kok_ad = if kok_ad.is_empty() { "mcpServers" } else { kok_ad.as_str() };
        match serde_json::from_str::<Value>(&metin) {
            Ok(mut v) if v[kok_ad].get(ad.as_str()) == Some(deger) => {
                let kap = v[kok_ad].as_object_mut().unwrap();
                kap.remove(ad.as_str());
                if k.kok_olusturuldu && kap.is_empty() {
                    v.as_object_mut().unwrap().remove(kok_ad);
                }
                Some(if !k.onceden_vardi && v.as_object().is_some_and(|o| o.is_empty()) { String::new() } else { json_yaz(&v, Some(&metin)) })
            }
            _ => None,
        }
    } else {
        let ekli = k.ekli_metin.clone().unwrap_or_default();
        let blok = ekli.trim_start_matches(['\r', '\n']).to_string();
        [ekli, blok].into_iter().find(|p| !p.is_empty() && metin.contains(p.as_str())).map(|p| metin.replacen(&p, "", 1))
    };
    let Some(yeni) = yeni else {
        return (
            t!(
                "dosya sonradan değişmiş ve Sezgi'nin eklediği kısım bulunamadı; dokunulmadı (elle kontrol edin)",
                "the file changed later and the part Sezgi added was not found; left untouched (check it by hand)"
            )
            .into(),
            true,
        );
    };
    if deneme {
        return (t!("yalnızca Sezgi'nin eklediği kısım silinecek (dosya sonradan değişmiş)", "only the part Sezgi added will be removed (the file changed later)").into(), false);
    }
    let sonuc =
        if yeni.is_empty() && !k.onceden_vardi { std::fs::remove_file(&yol).map_err(|e| e.to_string()).map(|_| bos_klasorleri_sil(k)) } else { atomik_yaz(&yol, yeni.as_bytes()) };
    if let Some(y) = &k.yedek {
        let _ = std::fs::remove_file(y);
    }
    match sonuc {
        Ok(()) => (t!("yalnızca Sezgi'nin eklediği kısım silindi (dosya sonradan değişmiş)", "only the part Sezgi added was removed (the file changed later)").into(), true),
        Err(e) => (tf!("yazılamadı ({e})", "could not be written ({e})"), false),
    }
}

/// sezgi kaldir [uygulama-klasörü=.] [--evet] [--harita-sil] [--deneme]
/// Önce neyin geri alınacağı listelenir; onay terminalde sorulur, terminal yoksa `--evet` gerekir
pub fn kaldir(args: &[String]) {
    let proje = mutlak(Path::new(args.get(1).filter(|a| !a.starts_with("--")).map(String::as_str).unwrap_or(".")));
    let deneme = args.iter().any(|a| a == "--deneme");
    let defter = defter_oku().unwrap_or_else(|e| dur(&e, 1));
    let anahtar = duz(&proje);
    let (bu, mut kalan): (Vec<Kayit>, Vec<Kayit>) = defter.into_iter().partition(|k| k.proje == anahtar);
    if bu.is_empty() {
        tp!("Bu uygulama için Sezgi'nin eklediği bir bağlantı ya da yönlendirme yok.", "Sezgi added no connection or instruction for this application.");
    } else if !deneme {
        tp!("Geri alınacaklar ({}):", "To undo ({}):", duz(&proje));
        for k in bu.iter().rev() {
            println!("  · {} → {}: {}", k.arac, goster(Path::new(&k.dosya), &proje), geri_al(k, true).0);
        }
        if !onaylandi(args, t!("Bunlar geri alınsın mı?", "Undo these?")) {
            if std::io::stdin().is_terminal() {
                tp!("Vazgeçildi: hiçbir şey değişmedi.", "Cancelled: nothing changed.");
                return;
            }
            dur(
                &tf!(
                    "Onay yok: hiçbir şey değişmedi. Terminal dışında onaylamak için: sezgi kaldir{} --evet",
                    "No approval: nothing changed. To approve outside a terminal: sezgi remove{} --yes",
                    proje_eki(&proje)
                ),
                2,
            );
        }
    }
    let mut hata = false;
    // Son eklenenden başlayarak
    for k in bu.iter().rev() {
        let (sonuc, sil) = geri_al(k, deneme);
        let isaret = if deneme {
            "·"
        } else if sonuc.contains("dokunulmadı") || sonuc.contains("untouched") {
            "!"
        } else if sil {
            "✓"
        } else {
            "✗"
        };
        println!("  {isaret} {} → {}: {sonuc}", k.arac, goster(Path::new(&k.dosya), &proje));
        if !sil && !deneme {
            hata = true;
            kalan.push(k.clone());
        }
    }
    if !deneme && !bu.is_empty() {
        if let Err(e) = defter_yaz(&kalan) {
            dur(&tf!("Kayıt güncellenemedi: {e}", "Could not update the record: {e}"), 1);
        }
    }

    // Harita: ayrıca sorulur; yalnızca Sezgi'nin yazdığı dosyalar silinir
    let harita = proje.join(".sezgi");
    if harita.is_dir() && !harita.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        let sil = args.iter().any(|a| a == "--harita-sil")
            || (!deneme && std::io::stdin().is_terminal() && sor(&tf!("Harita da silinsin mi ({})?", "Delete the map too ({})?", goster(&harita, &proje))));
        if sil && !deneme {
            for d in HARITA_DOSYALARI {
                let _ = std::fs::remove_file(harita.join(d));
            }
            if std::fs::remove_dir(&harita).is_ok() {
                tp!("  ✓ harita silindi: {}", "  ✓ map deleted: {}", goster(&harita, &proje));
            } else {
                tp!(
                    "  · harita dosyaları silindi; klasörde Sezgi'ye ait olmayan dosyalar kaldı: {}",
                    "  · map files deleted; files not belonging to Sezgi remain in the folder: {}",
                    goster(&harita, &proje)
                );
            }
        } else {
            tp!(
                "  · harita yerinde kaldı ({}); silmek için: sezgi kaldir --harita-sil",
                "  · the map stays ({}); to delete it: sezgi remove --delete-map",
                goster(&harita, &proje)
            );
        }
    }
    if hata {
        std::process::exit(1);
    }
}

// ---- Durum (sezgi doktor için, salt okunur) ----

pub enum Durum {
    Tamam(String),
    Uyari(String),
    Hata(String),
}

/// Bu uygulama için eklenen bağlantılar hâlâ çalışır durumda mı: girdi dosyada mı, komut ve harita yerinde mi
pub fn baglanti_durumlari(proje: &Path) -> Vec<(String, String, Durum)> {
    let Ok(defter) = defter_oku() else { return vec![] };
    let proje = mutlak(proje);
    let anahtar = duz(&proje);
    defter
        .iter()
        .filter(|k| k.proje == anahtar && k.tur == "mcp")
        .map(|k| {
            let yol = PathBuf::from(&k.dosya);
            let girdi = std::fs::read_to_string(&yol).ok().and_then(|m| girdi_oku(k, &m));
            let durum = match girdi {
                None if !yol.exists() => Durum::Uyari(tf!(
                    "ayar dosyası silinmiş; bağlantı yok → sezgi kaldir {0}, ardından sezgi bagla",
                    "the settings file was deleted; no connection → sezgi remove {0}, then sezgi connect",
                    duz(&proje)
                )),
                None => Durum::Uyari(
                    t!(
                        "Sezgi girdisi dosyada yok (elle silinmiş olabilir) → yeniden eklemek için: sezgi bagla",
                        "the Sezgi entry is not in the file (it may have been deleted by hand) → to add it again: sezgi connect"
                    )
                    .into(),
                ),
                Some((komut, argumanlar)) => {
                    let harita = argumanlar.iter().position(|a| a == "mcp").and_then(|i| argumanlar.get(i + 1)).map(PathBuf::from);
                    let komut_yerinde = matches!(komut.as_str(), "npx" | "cmd") || Path::new(&komut).is_file() || (!komut.contains(['/', '\\']) && komut_var(&komut));
                    if !komut_yerinde {
                        Durum::Hata(tf!(
                            "komut bulunamadı: {komut} (Sezgi taşınmış ya da kaldırılmış) → sezgi kaldir, ardından sezgi bagla",
                            "command not found: {komut} (Sezgi was moved or removed) → sezgi remove, then sezgi connect"
                        ))
                    } else if !harita.as_ref().is_some_and(|h| h.join("kapilar.json").is_file()) {
                        Durum::Hata(
                            t!(
                                "bağlantının gösterdiği harita yok → sezgi init, ardından sezgi kaldir ve sezgi bagla",
                                "the map the connection points to is missing → sezgi init, then sezgi remove and sezgi connect"
                            )
                            .into(),
                        )
                    } else {
                        Durum::Tamam(format!("{} → {}", goster(&yol, &proje), komut))
                    }
                }
            };
            (k.arac.clone(), goster(&yol, &proje), durum)
        })
        .collect()
}

fn girdi_oku(k: &Kayit, metin: &str) -> Option<(String, Vec<String>)> {
    let ad = k.ad.as_deref()?;
    let dizi = |v: Option<Vec<String>>| v.unwrap_or_default();
    if k.bicim == "json" {
        let v: Value = serde_json::from_str(metin).ok()?;
        let g = v.get(k.kok.as_deref().unwrap_or("mcpServers"))?.get(ad)?;
        let argumanlar = g.get("args").and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect());
        Some((g.get("command")?.as_str()?.to_string(), dizi(argumanlar)))
    } else {
        let t: toml::Table = metin.parse().ok()?;
        let g = t.get("mcp_servers")?.get(ad)?;
        let argumanlar = g.get("args").and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect());
        Some((g.get("command")?.as_str()?.to_string(), dizi(argumanlar)))
    }
}
