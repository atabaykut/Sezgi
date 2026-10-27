//! Kaynaksız keşif (`sezgi kesfet`): kaynak kodu olmayan bir masaüstü uygulamasının klasörü ya da tek bir dosyası.
//! - **Triyaj:** klasördeki `.exe`/`.dll`, `.apk`, `.db`/`.sqlite`, `appsettings*.json`, `app.asar`, `package.json` →
//!   hangi okuyucunun ya da hangi REA aracının çalışacağı planı.
//! - **Yerel okuyucular (Node gerekmez):** PE kimliği, APK bildirimi (BINARY_METADATA) ve SQLite şeması
//!   (DATABASE_SCHEMA).
//! - **REA (yalnızca `--rea` ile):** .NET üyeleri, imzalar, çağrı hedefleri ve P/Invoke (BINARY_METADATA); literal
//!   dizelerin SQL biçimi (BINARY_HEURISTIC). Köprü sabitlemesi tutmazsa kapalı kalır, sebebi yazılır, komut yine başarılı biter.
//!   Native ikili (ELF ve yerel PE, kullanıcının motoruyla), firmware, süreç gözlemi ve WebMCP keşfi `genis`te; onay
//!   gerektiren komutlar insanın terminalde verdiği onay ve yalıtım beyanı olmadan çalışmaz.
//! - **Çıktı:** `<çıktı>/kaynaksiz.json` (kanıtlar, plan, bilinmeyenler); `--cikti` verilmezse `<klasör>/.sezgi`. Aynı
//!   klasöre kanıt deposu, graf, boş işlem haritası ve Benlik Kartı da yazılır (`bagla`): `sezgi mcp`, santral ve
//!   `sezgi dogrula` kaynaksız uygulamada da çalışır.
//!   Yapıtın dosyalarına dokunulmaz, yapıttaki hiçbir kod çalıştırılmaz, ağa çıkılmaz.
//! - **Politika:** kaynak kodu (SOURCE_CODE/API_SCHEMA) olmadığı için bütün kanıtlar `kapsam: acik`; önizleme bu sistem
//!   için hiçbir zaman GEÇER demez, her yazma onay ister. Tavanlar: BINARY_METADATA observed, BINARY_HEURISTIC ve
//!   DECOMPILED inferred, DATABASE_SCHEMA certain (dosyadan okunan şema yine de observed yazılır).
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::{bagla, genis, pe, rea, veritabani};
use crate::guvenli;

/// Triyajın gezdiği en fazla girdi ve derinlik
const EN_FAZLA_GIRDI: usize = 5000;
const EN_DERIN: usize = 4;
/// Gezilmeyen klasörler
const ATLANAN: &[&str] = &["node_modules", ".git", ".sezgi", "__pycache__"];

/// Planın bir öğesi: dosya, türü ve onu okuyacak olan
#[derive(Debug, Clone)]
pub(crate) struct Oge {
    pub yol: PathBuf,
    pub goreli: String,
    pub tur: &'static str,
}

/// Güven sırası (certain > observed > inferred > unknown)
fn sira(g: &str) -> u8 {
    match g {
        "certain" => 3,
        "observed" => 2,
        "inferred" => 1,
        _ => 0,
    }
}

/// Kanıt türünün tavanı
pub(crate) fn tavan(tur: &str) -> &'static str {
    match tur {
        "SOURCE_CODE" | "API_SCHEMA" | "DATABASE_SCHEMA" => "certain",
        "BINARY_METADATA" | "BROWSER_OBSERVATION" | "NETWORK_CAPTURE" | "RUNTIME_TRACE" | "AUTOMATED_TEST" => "observed",
        "BINARY_HEURISTIC" | "DECOMPILED" | "INFERRED_NAME" => "inferred",
        _ => "unknown",
    }
}

/// İstenen güven tavanla sınırlanır
fn sinirla(tur: &str, istenen: &str) -> &'static str {
    let t = tavan(tur);
    if sira(istenen) >= sira(t) {
        t
    } else {
        match istenen {
            "observed" => "observed",
            "inferred" => "inferred",
            _ => "unknown",
        }
    }
}

/// Kanıt kaydı. Kimlik içerikten türetilir (aynı yapıttan aynı kanıt aynı kimliği alır). Özeti boş (dosyaya dayanmayan,
/// ör. tarayıcı gözlemi) kanıtın dayanağı yoktur.
#[allow(clippy::too_many_arguments)]
pub(crate) fn kanit(
    tur: &str,
    iddia: &str,
    guven: &str,
    yetki: &str,
    uretici: &str,
    oge: &Oge,
    sha256: &str,
    ek_konum: Option<String>,
    ozet: Value,
    kaynak: Option<String>,
) -> Value {
    let konum = match ek_konum {
        Some(e) => format!("{}#{e}", oge.goreli),
        None => oge.goreli.clone(),
    };
    let govde = json!([tur, iddia, uretici, sha256, konum, ozet]);
    let id = format!("kn_{}", &crate::benlik::sha256_hex(govde.to_string().as_bytes())[..24]);
    let mut k = json!({
        "id": id,
        "kaynak_turu": tur,
        "iddia_turu": iddia,
        "beyan": sinirla(tur, guven),
        "yetki": yetki,
        "uretici": uretici,
        "dayanak": if sha256.is_empty() { json!([]) } else { json!([{ "yol": oge.goreli, "sha256": sha256 }]) },
        "konum": [konum],
        "zaman": crate::simdi(),
        "kapsam": "acik",
        "ozet": ozet,
    });
    if let Some(e) = kaynak {
        k["kaynak_kanit"] = json!(e);
    }
    k
}

/// Dosyanın türü (uzantı ve imzayla); tanınmıyorsa `None`
fn tur(p: &Path) -> Option<&'static str> {
    let ad = p.file_name()?.to_str()?.to_lowercase();
    let uzanti = p.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    let imza = |n: usize| -> Vec<u8> {
        use std::io::Read;
        let mut b = vec![0; n];
        let k = std::fs::File::open(p).and_then(|mut f| f.read(&mut b)).unwrap_or(0);
        b.truncate(k);
        b
    };
    let elf = || imza(4) == b"\x7fELF";
    match uzanti.as_str() {
        "exe" | "dll" if imza(2) == b"MZ" => Some("pe"),
        "" | "so" | "elf" | "bin" | "out" | "axf" if elf() => Some("elf"),
        "img" | "fw" | "trx" | "chk" | "squashfs" | "ubi" | "uimage" | "bin" => Some("firmware"),
        "apk" if imza(4) == b"PK\x03\x04" => Some("apk"),
        "db" | "sqlite" | "sqlite3" | "db3" if imza(16) == b"SQLite format 3\0" => Some("sqlite"),
        _ if ad == "app.asar" => Some("electron"),
        _ if ad == "package.json" => Some("node"),
        _ if ad.starts_with("appsettings") && uzanti == "json" => Some("yapilandirma"),
        _ => None,
    }
}

/// Triyaj: klasör (en fazla 4 düzey) ya da tek dosya
pub(crate) fn plan(yapit: &Path) -> (Vec<Oge>, Vec<String>) {
    let mut ogeler = Vec::new();
    let mut notlar = Vec::new();
    if yapit.is_file() {
        if let Some(t) = tur(yapit) {
            let ad = yapit.file_name().and_then(|a| a.to_str()).unwrap_or("").to_string();
            ogeler.push(Oge { yol: yapit.to_path_buf(), goreli: ad, tur: t });
        }
        return (ogeler, notlar);
    }
    let mut yigin = vec![(yapit.to_path_buf(), 0usize)];
    let mut sayac = 0;
    while let Some((d, derinlik)) = yigin.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut girdiler: Vec<_> = rd.flatten().collect();
        girdiler.sort_by_key(|g| g.file_name());
        for g in girdiler {
            sayac += 1;
            if sayac > EN_FAZLA_GIRDI {
                notlar.push(format!("{EN_FAZLA_GIRDI} girdiden sonrası gezilmedi"));
                return (ogeler, notlar);
            }
            let Ok(ft) = g.file_type() else { continue };
            let p = g.path();
            if ft.is_dir() {
                let ad = g.file_name().to_string_lossy().to_string();
                if derinlik + 1 < EN_DERIN && !ATLANAN.contains(&ad.as_str()) {
                    yigin.push((p, derinlik + 1));
                }
            } else if ft.is_file() {
                if let Some(t) = tur(&p) {
                    ogeler.push(Oge { goreli: guvenli::goreli(yapit, &p), yol: p, tur: t });
                }
            }
        }
    }
    ogeler.sort_by(|a, b| a.goreli.cmp(&b.goreli));
    (ogeler, notlar)
}

/// Dosya özeti (SHA-256); okunamazsa boş
fn dosya_ozeti(p: &Path, sinir: u64) -> Option<(Vec<u8>, String)> {
    let m = std::fs::metadata(p).ok()?;
    if m.len() > sinir {
        return None;
    }
    let v = std::fs::read(p).ok()?;
    let o = crate::benlik::sha256_hex(&v);
    Some((v, o))
}

/// SHA-256, dosyanın içeriği belleğe alınmadan (SQLite dosyaları büyük olabilir)
fn akis_ozeti(p: &Path) -> String {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(p) else { return String::new() };
    let mut c = ring::digest::Context::new(&ring::digest::SHA256);
    let mut b = vec![0; 1 << 16];
    while let Ok(n) = f.read(&mut b) {
        if n == 0 {
            break;
        }
        c.update(&b[..n]);
    }
    c.finish().as_ref().iter().map(|x| format!("{x:02x}")).collect()
}

/// Keşfin sonucu
#[derive(Default)]
pub(crate) struct Sonuc {
    pub kanitlar: Vec<Value>,
    pub plan: Vec<Value>,
    pub bilinmeyenler: Vec<Value>,
    pub satirlar: Vec<String>,
    pub rea: Value,
}

pub(crate) fn bilinmeyen(oge: &Oge, konu: &str, neden: &str) -> Value {
    json!({ "yol": oge.goreli, "konu": konu, "neden": neden })
}

/// ELF başlığının kimliği (sınıf, bayt sırası, makine); sınır denetimli
fn elf_kimligi(b: &[u8]) -> Value {
    let makine = b.get(18..20).map(|m| if b.get(5) == Some(&2) { u16::from_be_bytes([m[0], m[1]]) } else { u16::from_le_bytes([m[0], m[1]]) });
    let makine = match makine {
        Some(0x3e) => "x86-64".to_string(),
        Some(0x03) => "x86".into(),
        Some(0xb7) => "aarch64".into(),
        Some(0x28) => "arm".into(),
        Some(0xf3) => "riscv".into(),
        Some(0x08) => "mips".into(),
        Some(x) => format!("0x{x:04x}"),
        None => "bilinmiyor".into(),
    };
    let tur = match b.get(16..18).map(|t| if b.get(5) == Some(&2) { u16::from_be_bytes([t[0], t[1]]) } else { u16::from_le_bytes([t[0], t[1]]) }) {
        Some(2) => "calistirilabilir",
        Some(3) => "paylasilan_nesne",
        Some(1) => "yer_degistirilebilir",
        Some(4) => "core",
        _ => "bilinmiyor",
    };
    json!({ "bicim": "elf", "sinif": if b.get(4) == Some(&2) { 64 } else { 32 }, "bayt_sirasi": if b.get(5) == Some(&2) { "buyuk" } else { "kucuk" }, "makine": makine, "tur": tur })
}

/// Planı yürütür: yerel okuyucular her zaman; REA yalnızca istenirse; geniş adımlar (`genis`) onay kapısıyla
pub(crate) fn kesfet(yapit: Option<&Path>, b: &mut genis::Baglam) -> Sonuc {
    let rea_iste = b.sec.rea;
    let (ogeler, notlar) = yapit.map(plan).unwrap_or_default();
    let tek_dosya = yapit.is_some_and(Path::is_file);
    let mut s = Sonuc::default();
    for oge in &ogeler {
        let mut rea_adimlari: Vec<Value> = Vec::new();
        match oge.tur {
            "pe" => {
                let Some((veri, ozet)) = dosya_ozeti(&oge.yol, pe::EN_BUYUK) else {
                    s.bilinmeyenler.push(bilinmeyen(oge, "PE kimliği", "dosya okunamadı ya da çok büyük"));
                    continue;
                };
                match pe::oku(&veri) {
                    Ok(p) => {
                        let ad = p.surum_kaynagi.get("ProductName").cloned().unwrap_or_default();
                        let surum = p.surum_kaynagi.get("FileVersion").cloned().or(p.sabit_surum.clone()).unwrap_or_default();
                        s.satirlar.push(format!(
                            "  PE      {}  {} · {}{} · {}",
                            oge.goreli,
                            p.makine,
                            if p.dotnet { format!(".NET ({})", p.clr_surumu.as_deref().unwrap_or("?")) } else { t!("yerel", "native").into() },
                            p.hedef_cati.as_deref().map(|c| format!(" {c}")).unwrap_or_default(),
                            [ad, surum, p.surum_kaynagi.get("CompanyName").cloned().unwrap_or_default()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · ")
                        ));
                        s.kanitlar.push(kanit("BINARY_METADATA", "structure", "observed", "shipped-artifact", "sezgi/pe@1", oge, &ozet, None, p.ozet(), None));
                        if let Some(c) = &p.hedef_cati {
                            s.kanitlar.push(kanit(
                                "BINARY_HEURISTIC",
                                "structure",
                                "inferred",
                                "shipped-artifact",
                                "sezgi/pe@1",
                                oge,
                                &ozet,
                                Some("hedef_cati".into()),
                                json!({ "hedef_cati": c }),
                                None,
                            ));
                        }
                        if !p.dotnet {
                            rea_adimlari.extend(genis::native(b, oge, &ozet, &mut s));
                        }
                        if p.dotnet {
                            for arac in ["inspect-managed-members", "inspect-managed-native-boundaries"] {
                                rea_adimlari.push(rea_calistir(&mut b.kopru, rea_iste, arac, oge, &ozet, &mut s));
                            }
                            s.bilinmeyenler.push(bilinmeyen(
                                oge,
                                "iş kuralları (eşik, oran, rol koşulu)",
                                "kaynak kodu yok: yalnızca bir decompiler çıktısıyla ve en fazla inferred düzeyinde gelir",
                            ));
                        }
                    }
                    Err(e) => s.bilinmeyenler.push(bilinmeyen(oge, "PE kimliği", &e)),
                }
            }
            "sqlite" => {
                let ozet = akis_ozeti(&oge.yol);
                match veritabani::oku(&oge.yol) {
                    Ok(sema) => {
                        let (tablolar, gorunumler) = veritabani::tablolar(&sema);
                        let say = |k: &str| tablolar.iter().map(|t| t[k].as_array().map_or(0, Vec::len)).sum::<usize>();
                        s.satirlar.push(tf!(
                            "  SQLite  {}  {} tablo · {} indeks · {} tetikleyici · {} görünüm{}",
                            "  SQLite  {}  {} {t} · {} {i} · {} {g} · {} {v}{}",
                            oge.goreli,
                            tablolar.len(),
                            say("indeksler"),
                            say("tetikleyiciler"),
                            gorunumler.len(),
                            if sema.sorunlar.is_empty() { String::new() } else { tf!(" · {} sorun", " · {} {p}", sema.sorunlar.len(); p = crate::dil::cogul(sema.sorunlar.len(), "problem", "problems")) };
                            t = crate::dil::cogul(tablolar.len(), "table", "tables"),
                            i = crate::dil::cogul(say("indeksler"), "index", "indexes"),
                            g = crate::dil::cogul(say("tetikleyiciler"), "trigger", "triggers"),
                            v = crate::dil::cogul(gorunumler.len(), "view", "views")
                        ));
                        s.kanitlar.push(kanit(
                            "DATABASE_SCHEMA",
                            "structure",
                            "observed",
                            "shipped-artifact",
                            "sezgi/sqlite@1",
                            oge,
                            &ozet,
                            Some("dosya".into()),
                            sema.dosya(),
                            None,
                        ));
                        for t in tablolar {
                            let ad = t["ad"].as_str().unwrap_or("").to_string();
                            s.kanitlar.push(kanit(
                                "DATABASE_SCHEMA",
                                "structure",
                                "observed",
                                "shipped-artifact",
                                "sezgi/sqlite@1",
                                oge,
                                &ozet,
                                Some(format!("tablo:{ad}")),
                                t,
                                None,
                            ));
                        }
                        for g in gorunumler {
                            let ad = g["ad"].as_str().unwrap_or("").to_string();
                            s.kanitlar.push(kanit(
                                "DATABASE_SCHEMA",
                                "structure",
                                "observed",
                                "shipped-artifact",
                                "sezgi/sqlite@1",
                                oge,
                                &ozet,
                                Some(format!("gorunum:{ad}")),
                                g,
                                None,
                            ));
                        }
                        for sorun in &sema.sorunlar {
                            s.bilinmeyenler.push(bilinmeyen(oge, "SQLite şeması", sorun));
                        }
                    }
                    Err(e) => s.bilinmeyenler.push(bilinmeyen(oge, "SQLite şeması", &e)),
                }
            }
            "apk" => match crate::android::apk_oku(&oge.yol) {
                Ok(v) => {
                    let kn = &v["kanit"];
                    let sha = v["dosya"]["sha256"].as_str().unwrap_or("").to_string();
                    let mut ozet = v.clone();
                    if let Some(o) = ozet.as_object_mut() {
                        o.remove("kanit");
                    }
                    let mut kayit = kanit(
                        kn["kaynak_turu"].as_str().unwrap_or("BINARY_METADATA"),
                        "structure",
                        kn["beyan"].as_str().unwrap_or("inferred"),
                        "shipped-artifact",
                        kn["uretici"].as_str().unwrap_or(crate::android::URETICI),
                        oge,
                        &sha,
                        None,
                        ozet,
                        None,
                    );
                    kayit["sinirlamalar"] = kn["sinirlamalar"].clone();
                    s.kanitlar.push(kayit);
                    let bilesen = v["bilesenler"].as_array().map_or(0, Vec::len);
                    let acik = v["bilesenler"].as_array().into_iter().flatten().filter(|b| b["disa_acik"] == true).count();
                    s.satirlar.push(tf!(
                        "  APK     {}  {} {} · {} bileşen ({} dışa açık) · {} izin",
                        "  APK     {}  {} {} · {} {b} ({} exported) · {} {z}",
                        oge.goreli,
                        v["paket"].as_str().unwrap_or("?"),
                        v["surum_adi"].as_str().unwrap_or(""),
                        bilesen,
                        acik,
                        v["izinler"].as_array().map_or(0, Vec::len);
                        b = crate::dil::cogul(bilesen, "component", "components"),
                        z = crate::dil::cogul(v["izinler"].as_array().map_or(0, Vec::len), "permission", "permissions")
                    ));
                    // REA'nın APK araçları süreç başlatır ve dosya sistemine yazar: izin listesinde değiller, çalıştırılmaz
                    for arac in ["inspect-android-package", "decode-android-resources"] {
                        let neden = rea::arac_izinli(arac).err().unwrap_or_default();
                        rea_adimlari.push(json!({ "arac": arac, "durum": if rea_iste { "calistirilmadi" } else { "istenmedi" }, "neden": neden }));
                    }
                    s.bilinmeyenler.push(bilinmeyen(oge, "bileşenlerin davranışı", "yalnızca bildirim okundu: kodda ne yaptıkları decompile ya da gözlem ister"));
                }
                Err(e) => s.bilinmeyenler.push(bilinmeyen(oge, "APK bildirimi", &e)),
            },
            "elf" => {
                let ozet = genis::ozet(&oge.yol);
                let bas = {
                    use std::io::Read;
                    let mut v = vec![0u8; 64];
                    let n = std::fs::File::open(&oge.yol).and_then(|mut f| f.read(&mut v)).unwrap_or(0);
                    v.truncate(n);
                    v
                };
                let kimlik = elf_kimligi(&bas);
                let elf_turu = match kimlik["tur"].as_str().unwrap_or("?") {
                    "calistirilabilir" => t!("calistirilabilir", "executable"),
                    "paylasilan_nesne" => t!("paylasilan_nesne", "shared object"),
                    "yer_degistirilebilir" => t!("yer_degistirilebilir", "relocatable"),
                    "bilinmiyor" => t!("bilinmiyor", "unknown"),
                    x => x,
                };
                s.satirlar.push(format!("  ELF     {}  {} · {}-bit · {elf_turu}", oge.goreli, kimlik["makine"].as_str().unwrap_or("?"), kimlik["sinif"]));
                s.kanitlar.push(kanit("BINARY_METADATA", "structure", "observed", "shipped-artifact", "sezgi/elf@1", oge, &ozet, None, kimlik, None));
                rea_adimlari.extend(genis::native(b, oge, &ozet, &mut s));
                if b.sec.calistir && tek_dosya {
                    rea_adimlari.extend(genis::surec(b, oge, &ozet, &mut s));
                }
            }
            "firmware" => {
                let ozet = genis::ozet(&oge.yol);
                s.satirlar.push(tf!("  Firmware {}  (bölüm envanteri yalnızca REA ve Binwalk ile)", "  Firmware {}  (section inventory only with REA and Binwalk)", oge.goreli));
                rea_adimlari.extend(genis::firmware(b, oge, &ozet, &mut s));
            }
            "electron" => rea_adimlari.extend(super::electron::incele(b, oge, &mut s)),
            "node" => s.satirlar.push(tf!("  Node    {}  (kaynak kodu var: sezgi init ile okunur)", "  Node    {}  (source code present: read with sezgi init)", oge.goreli)),
            _ => {
                s.bilinmeyenler.push(bilinmeyen(oge, "yapılandırma", "değerler okunmaz (bağlantı dizesi ve parola içerebilir)"));
                s.satirlar.push(tf!("  Ayar    {}  (değerleri okunmadı)", "  Config  {}  (values not read)", oge.goreli));
            }
        }
        let okuyucu = match oge.tur {
            "pe" => "sezgi/pe",
            "elf" => "sezgi/elf",
            "apk" => crate::android::URETICI,
            "sqlite" => "sezgi/sqlite",
            "electron" => "sezgi/asar",
            "node" => "sezgi init",
            _ => "yok",
        };
        s.plan.push(json!({ "yol": oge.goreli, "tur": oge.tur, "okuyucu": okuyucu, "rea": rea_adimlari }));
    }
    // `--calistir`: yalnızca tek bir çalıştırılabilir dosya için (klasörde hangi ikilinin çalışacağı tahmin edilmez)
    if b.sec.calistir && !(tek_dosya && ogeler.first().is_some_and(|o| o.tur == "elf")) {
        let o = Oge { yol: PathBuf::new(), goreli: ogeler.first().map(|o| o.goreli.clone()).unwrap_or_default(), tur: "surec" };
        s.bilinmeyenler.push(bilinmeyen(&o, "çalışma zamanı davranışı", "--calistir yalnızca bu işletim sisteminde çalışabilen tek bir ikiliyle (ELF) verilir"));
        s.satirlar.push(
            t!(
                "  Not     --calistir yok sayıldı: yalnızca tek bir ELF ikiliyle, REA kuruluyken ve onayla çalışır",
                "  Note    --calistir ignored: it works only with a single ELF binary, with REA installed and with approval"
            )
            .into(),
        );
    }
    genis::webmcp(b, &mut s);
    for n in notlar {
        s.bilinmeyenler.push(json!({ "yol": "", "konu": "triyaj", "neden": n }));
    }
    s.rea = match (&b.kopru, rea_iste) {
        (_, false) => json!({ "durum": "istenmedi" }),
        (None, true) => json!({ "durum": "gerekmedi", "neden": "REA gerektiren yapıt yok" }),
        (Some(Ok(k)), true) => json!({ "durum": "acik", "surum": rea::SURUM, "paket": guvenli::gorunen(&k.paket) }),
        (Some(Err(e)), true) => json!({ "durum": "kapali", "neden": e }),
    };
    s
}

/// REA adımı: istenmediyse, köprü kapalıysa ya da araç izinli değilse kanıt yerine bilinmeyen
fn rea_calistir(kopru: &mut Option<Result<rea::Kopru, String>>, iste: bool, arac: &str, oge: &Oge, ozet: &str, s: &mut Sonuc) -> Value {
    let konu = ".NET üyeleri, imzalar, çağrı hedefleri ve P/Invoke";
    if !iste {
        if arac == "inspect-managed-members" {
            s.bilinmeyenler.push(bilinmeyen(oge, konu, "REA istenmedi (sezgi kesfet <yapıt> --rea)"));
        }
        return json!({ "arac": arac, "durum": "istenmedi" });
    }
    let k = kopru.get_or_insert_with(rea::ac);
    let k = match k {
        Ok(k) => k,
        Err(e) => {
            if arac == "inspect-managed-members" {
                s.bilinmeyenler.push(bilinmeyen(oge, konu, &format!("REA kapalı: {e}")));
            }
            return json!({ "arac": arac, "durum": "kapali", "neden": e });
        }
    };
    match k.incele(arac, &oge.yol) {
        Ok(p) => {
            let sha = p.sha256.clone().filter(|x| x == ozet);
            if sha.is_none() {
                s.bilinmeyenler.push(bilinmeyen(oge, konu, "REA çıktısının yapıt özeti dosyayla aynı değil: kanıt alınmadı"));
                return json!({ "arac": arac, "durum": "reddedildi", "neden": "yapıt özeti uyuşmuyor" });
            }
            let uretici = format!("{}/{}", p.saglayici, p.islem);
            let mut n = 0;
            let kaynak = p.kaynak_kanit.clone();
            s.kanitlar.push(kanit("BINARY_METADATA", "structure", p.guven, &p.yetki, &uretici, oge, ozet, Some(p.islem.clone()), p.yapi.clone(), kaynak.clone()));
            n += 1;
            if p.cagrilar.as_array().is_some_and(|a| !a.is_empty()) {
                s.kanitlar.push(kanit(
                    "BINARY_METADATA",
                    "call_target",
                    p.guven,
                    &p.yetki,
                    &uretici,
                    oge,
                    ozet,
                    Some(format!("{}:cagrilar", p.islem)),
                    json!({ "cagrilar": p.cagrilar }),
                    kaynak.clone(),
                ));
                n += 1;
            }
            if !p.sql.is_empty() {
                s.kanitlar.push(kanit(
                    "BINARY_HEURISTIC",
                    "data_effect",
                    "inferred",
                    &p.yetki,
                    &uretici,
                    oge,
                    ozet,
                    Some(format!("{}:sql", p.islem)),
                    json!({ "sql": p.sql, "literal_sayisi": p.literal_sayisi }),
                    kaynak,
                ));
                n += 1;
            }
            json!({ "arac": arac, "durum": "calisti", "kanit": n })
        }
        Err(e) => {
            s.bilinmeyenler.push(bilinmeyen(oge, konu, &format!("REA {arac}: {e}")));
            json!({ "arac": arac, "durum": "hata", "neden": e })
        }
    }
}

/// `sezgi kesfet <yapıt> [--cikti <klasör>] [--rea] [--imzala [anahtar]] [--json]`
/// `  [--onayla --yalitim konteyner|vm|yok] [--calistir --sahibiyim] [--webmcp <loopback CDP> [--kaynak <köken>]]`
/// `sezgi kesfet --yetenekler [--json]`
pub(crate) fn komut(args: &[String]) {
    let var = |ad: &str| args.iter().any(|a| a == ad);
    let secenek = |ad: &str| args.windows(2).find(|w| w[0] == ad).map(|w| w[1].clone());
    if var("--yetenekler") {
        genis::yetenekler(var("--json"));
        return;
    }
    let yalitim = secenek("--yalitim").map(|y| {
        rea::Yalitim::coz(&y).unwrap_or_else(|| {
            te!("✗ --yalitim konteyner, vm ya da yok olmalı: {y}", "✗ --yalitim must be konteyner, vm or yok: {y}");
            std::process::exit(2)
        })
    });
    let webmcp = secenek("--webmcp");
    let sec = genis::Secenekler {
        rea: var("--rea") || var("--calistir") || webmcp.is_some(),
        onayla: var("--onayla"),
        yalitim,
        calistir: var("--calistir"),
        sahibiyim: var("--sahibiyim"),
        koken: secenek("--kaynak"),
        webmcp,
    };
    let yapit = args.get(1).filter(|a| !a.starts_with("--")).map(PathBuf::from);
    if yapit.is_none() && sec.webmcp.is_none() {
        crate::yardim::eksik("kesfet")
    }
    if let Some(y) = yapit.as_ref().filter(|y| !y.exists()) {
        te!("✗ {} yok", "✗ {} does not exist", guvenli::gorunen(y));
        std::process::exit(1);
    }
    let kok = match &yapit {
        Some(y) if y.is_dir() => y.clone(),
        Some(y) => y.parent().map(Path::to_path_buf).filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| PathBuf::from(".")),
        None => PathBuf::from("."),
    };
    let cikti = secenek("--cikti").map(PathBuf::from).unwrap_or_else(|| kok.join(".sezgi"));
    let mut baglam = genis::Baglam::yeni(sec.clone());
    let s = kesfet(yapit.as_deref(), &mut baglam);
    if s.plan.is_empty() {
        te!(
            "✗ {} içinde tanınan yapıt yok (.exe/.dll, ELF, .apk, firmware, .db/.sqlite, appsettings.json, app.asar, package.json)",
            "✗ No recognised artifact in {} (.exe/.dll, ELF, .apk, firmware, .db/.sqlite, appsettings.json, app.asar, package.json)",
            yapit.as_deref().map(guvenli::gorunen).unwrap_or_default()
        );
        std::process::exit(1);
    }
    let ad = match &yapit {
        Some(y) => std::fs::canonicalize(y).ok().and_then(|p| p.file_name().map(|a| a.to_string_lossy().to_string())).unwrap_or_default(),
        None => sec.koken.clone().unwrap_or_else(|| "web".into()).replace("://", "-").replace([':', '/'], "-"),
    };
    let belge = json!({
        "bicim": 1,
        "uretici": format!("sezgi@{}", env!("CARGO_PKG_VERSION")),
        "zaman": crate::simdi(),
        "yapit": ad,
        "kapsam": "acik",
        "onizleme": { "gecer": false, "neden": "kaynak kodu yok: kanıtlar yalnızca ikili, şema ya da REA kaynaklı; önizleme bu sistem için hiçbir zaman GEÇER demez, her yazma onay ister" },
        "rea": s.rea,
        "plan": s.plan,
        "kanitlar": s.kanitlar,
        "bilinmeyenler": s.bilinmeyenler,
    });
    let yol = cikti.join("kaynaksiz.json");
    if let Err(e) =
        std::fs::create_dir_all(&cikti).map_err(|e| e.to_string()).and_then(|_| guvenli::atomik_yaz(&yol, serde_json::to_string_pretty(&belge).unwrap_or_default().as_bytes()))
    {
        te!("✗ {} yazılamadı: {e} (başka bir klasör için: --cikti <klasör>)", "✗ Could not write {}: {e} (for another folder: --out <folder>)", guvenli::gorunen(&yol));
        std::process::exit(1);
    }
    // Kanıt deposu, graf, boş işlem haritası ve kart. Klasörde kaynak kodundan üretilmiş bir harita varsa ona dokunulmaz
    let mevcut = guvenli::oku_harita(&cikti.join("kapilar.json")).and_then(|m| serde_json::from_str::<Value>(&m).ok());
    let baska_harita = mevcut.as_ref().is_some_and(|h| h["ozet"]["adaptor"] != bagla::ADAPTOR);
    let imzala = args.iter().position(|a| a == "--imzala").map(|i| args.get(i + 1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "varsayilan".into()));
    let baglanti = if s.kanitlar.is_empty() {
        None
    } else if baska_harita {
        te!(
            "! {} kaynak kodundan üretilmiş bir harita içeriyor: kanıt deposu ve kart yazılmadı (başka klasör: --cikti <klasör>)",
            "! {} holds a map built from source code: the evidence store and card were not written (another folder: --out <folder>)",
            guvenli::gorunen(&cikti)
        );
        None
    } else {
        match bagla::yaz(&cikti, &kok, &belge, imzala.as_deref()) {
            Ok(b) => Some(b),
            Err(e) => {
                te!("✗ Kanıt deposu ve kart yazılamadı: {e}", "✗ Could not write the evidence store and card: {e}");
                std::process::exit(1);
            }
        }
    };
    if args.iter().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&belge).unwrap_or_default());
        return;
    }
    tp!("Kaynaksız keşif: {ad}", "Inspection without source code: {ad}");
    for l in &s.satirlar {
        println!("{l}");
    }
    match belge["rea"]["durum"].as_str() {
        Some("acik") => {
            tp!("REA {}: açık (sürüm, paket bütünlüğü ve araç etki tablosu doğrulandı)", "REA {}: on (version, package integrity and tool effect table verified)", rea::SURUM)
        }
        Some("kapali") => tp!("REA: kapalı — {}", "REA: off — {}", belge["rea"]["neden"].as_str().unwrap_or("")),
        Some("istenmedi") if belge["plan"].as_array().is_some_and(|p| p.iter().any(|o| o["rea"].as_array().is_some_and(|r| !r.is_empty()))) => {
            tp!(
                "REA: istenmedi (.NET üyeleri için: sezgi kesfet <yapıt> --rea; REA {} kurulu değilse önce: sezgi rea kur)",
                "REA: not requested (for .NET members: sezgi discover <artifact> --rea; if REA {} is not installed, first: sezgi rea install)",
                rea::SURUM
            )
        }
        _ => {}
    }
    if !s.kanitlar.is_empty() {
        tp!(
            "Önizleme: bu sistem için hiçbir zaman GEÇER değil (kaynak kodu yok; her yazma onay ister)",
            "Preview: never PASS for this system (no source code; every write needs approval)"
        );
    }
    tp!("Bilinmeyenler: {}", "Unknowns: {}", s.bilinmeyenler.len());
    for b in s.bilinmeyenler.iter().take(8) {
        println!("  ? {} {}: {}", b["yol"].as_str().unwrap_or(""), b["konu"].as_str().unwrap_or(""), b["neden"].as_str().unwrap_or(""));
    }
    tp!("✓ {} kanıt → {}", "✓ {} evidence records → {}", s.kanitlar.len(), guvenli::gorunen(&yol));
    if let Some((imza, dugum, kenar)) = baglanti {
        tp!(
            "✓ Kanıt deposu ve graf: {dugum} düğüm, {kenar} ilişki (kanit.json, graf.json); Benlik Kartı: benlik.json, {imza}",
            "✓ Evidence store and graph: {dugum} {d}, {kenar} {k} (kanit.json, graf.json); Benlik Card: benlik.json, {imza}";
            d = crate::dil::cogul(dugum, "node", "nodes"),
            k = crate::dil::cogul(kenar, "relation", "relations")
        );
        tp!(
            "  Yapay zekâya: sezgi mcp \"{}\"  (kanit, bilinmeyenler, etki; işlem yok)",
            "  For AI: sezgi mcp \"{}\"  (evidence, unknowns, impact; no operations)",
            guvenli::gorunen(&cikti)
        );
    }
}
