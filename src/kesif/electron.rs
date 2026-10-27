//! Electron uygulaması (`app.asar`): kaynak kodu dağıtılmamış ama paketin içindeki JavaScript okunabilir.
//! - **ASAR okuyucu** (yeni bağımlılık yok, sınır denetimli): başlık (JSON dosya ağacı) ve dosya içerikleri; paketin
//!   `package.json`'ı (ad, sürüm, giriş dosyası) BINARY_METADATA (observed).
//! - **IPC zinciri** (desenle, BINARY_HEURISTIC, inferred): ana süreç işleyicileri (`ipcMain.handle/on`), köprü
//!   (`contextBridge.exposeInMainWorld`) ve ön yüz çağrıları (`ipcRenderer.invoke/send`, köprü işlevleri) →
//!   `ipc:<kanal>` düğümleri ve ön yüz çağrısı → IPC → ana süreç işleyicisi zinciri. Pencere ayarları
//!   (`contextIsolation`, `nodeIntegration`, `sandbox`, `webSecurity`) ve yüklenen sayfalar.
//! - **Ön yüz bağları:** paketteki JS/TS geçici bir klasöre çıkarılır ve Sezgi'nin web adaptörüyle okunur: ağ
//!   çağrıları (yöntem + yol şablonu) ve ekran/eylem bağları. Klasör sonra silinir.
//! - **Fuse'lar:** paketin yanındaki Electron ikilisinde fuse işareti varsa sigorta durumları (RunAsNode,
//!   OnlyLoadAppFromAsar …) BINARY_METADATA (observed).
//! - **REA (`--rea`):** `analyze-javascript-application` onaylı sınıftadır (süreç başlatır, dosyaya yazar): onay ve
//!   yalıtım beyanıyla; IPC kanalları, işleyici eşleşmeleri, köprü ve pencere ayarları (BINARY_HEURISTIC, inferred).
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use super::genis::Baglam;
use super::masaustu::{bilinmeyen, kanit, Oge, Sonuc};
use super::rea_genis as g;

/// Okunan en büyük başlık ve dosya; en çok dosya
const EN_BUYUK_BASLIK: u64 = 32 << 20;
const EN_BUYUK_DOSYA: u64 = 4 << 20;
const EN_COK_DOSYA: usize = 20_000;
/// Taranan JS'nin toplam boyu
const EN_COK_JS: u64 = 64 << 20;
/// Electron fuse işareti (ikilinin içinde, fuse telinin başı)
const FUSE_ISARETI: &[u8] = b"dL7pKGdnNz796PbbjQWNKmHXBZaB9tsX";
/// Fuse sırası (tel biçimi 1)
const FUSELAR: &[&str] = &[
    "RunAsNode",
    "EnableCookieEncryption",
    "EnableNodeOptionsEnvironmentVariable",
    "EnableNodeCliInspectArguments",
    "EnableEmbeddedAsarIntegrityValidation",
    "OnlyLoadAppFromAsar",
    "LoadBrowserProcessSpecificV8Snapshot",
    "GrantFileProtocolExtraPrivileges",
];

/// ASAR paketi: dosya listesi ve veri başlangıcı
pub(crate) struct Asar {
    dosya: std::fs::File,
    veri: u64,
    /// (yol, boy, konum, paketin dışında mı)
    pub girdiler: Vec<(String, u64, u64, bool)>,
}

fn u32le(b: &[u8], o: usize) -> Option<u64> {
    b.get(o..o + 4).map(|x| u64::from(u32::from_le_bytes([x[0], x[1], x[2], x[3]])))
}

impl Asar {
    pub fn ac(yol: &Path) -> Result<Asar, String> {
        let mut f = std::fs::File::open(yol).map_err(|e| format!("açılamadı: {e}"))?;
        let boy = f.metadata().map_err(|e| e.to_string())?.len();
        let mut bas = [0u8; 16];
        f.read_exact(&mut bas).map_err(|_| "ASAR başlığı kısa".to_string())?;
        let (ilk, baslik_boyu, metin_boyu) = (u32le(&bas, 0).unwrap_or(0), u32le(&bas, 4).unwrap_or(0), u32le(&bas, 12).unwrap_or(0));
        if ilk != 4 || baslik_boyu > EN_BUYUK_BASLIK || metin_boyu + 8 > baslik_boyu || 8 + baslik_boyu > boy {
            return Err("ASAR değil (başlık biçimi tutmuyor)".into());
        }
        let mut metin = vec![0u8; metin_boyu as usize];
        f.read_exact(&mut metin).map_err(|_| "ASAR başlığı kısa".to_string())?;
        let v: Value = serde_json::from_slice(&metin).map_err(|_| "ASAR başlığı JSON değil".to_string())?;
        let mut girdiler = vec![];
        fn gez(v: &Value, onek: &str, d: usize, out: &mut Vec<(String, u64, u64, bool)>) {
            if d > 32 || out.len() >= EN_COK_DOSYA {
                return;
            }
            for (ad, x) in v["files"].as_object().into_iter().flatten() {
                // Yol parçası güvenli olmalı: ayraç, üst klasör ya da boş ad yok
                if ad.is_empty() || ad == "." || ad == ".." || ad.contains(['/', '\\', '\0']) {
                    continue;
                }
                let yol = if onek.is_empty() { ad.clone() } else { format!("{onek}/{ad}") };
                if x.get("files").is_some() {
                    gez(x, &yol, d + 1, out);
                } else if let Some(boy) = x["size"].as_u64() {
                    let konum = x["offset"].as_str().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                    out.push((yol, boy, konum, x["unpacked"] == true));
                }
            }
        }
        gez(&v, "", 0, &mut girdiler);
        Ok(Asar { dosya: f, veri: 8 + baslik_boyu, girdiler })
    }

    /// Paketteki bir dosyanın içeriği (paketin dışındaki ya da çok büyük dosya okunmaz)
    pub fn oku(&mut self, yol: &str) -> Option<Vec<u8>> {
        let (_, boy, konum, disarida) = self.girdiler.iter().find(|g| g.0 == yol)?.clone();
        if disarida || boy > EN_BUYUK_DOSYA {
            return None;
        }
        self.dosya.seek(SeekFrom::Start(self.veri.checked_add(konum)?)).ok()?;
        let mut b = vec![0u8; boy as usize];
        self.dosya.read_exact(&mut b).ok()?;
        Some(b)
    }
}

fn satir(metin: &str, bayt: usize) -> usize {
    metin[..bayt.min(metin.len())].bytes().filter(|b| *b == b'\n').count() + 1
}

/// Adresin yalnızca kökeni (yol ve sorgu değer taşıyabilir)
fn koken(u: &str) -> String {
    match u.find("://") {
        Some(i) => {
            let ana = u[i + 3..].split(['/', '?', '#']).next().unwrap_or("");
            let ana = ana.rsplit('@').next().unwrap_or("");
            crate::guvenli::temiz(&format!("{}://{ana}", &u[..i]))
        }
        None => String::new(),
    }
}

fn js_mi(yol: &str) -> bool {
    let u = yol.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(u.as_str(), "js" | "cjs" | "mjs" | "ts" | "tsx" | "jsx") && !yol.split('/').any(|p| p == "node_modules")
}

/// Paketteki JS'den IPC zinciri, köprü ve pencere ayarları (desenle; değer alınmaz, yalnızca adlar)
fn ipc_cozumle(dosyalar: &[(String, String)]) -> Value {
    let isleyici = Regex::new(r#"ipcMain\s*\.\s*(handle|handleOnce|on|once)\s*\(\s*['"`]([^'"`\n]{1,128})['"`]"#).unwrap();
    let gonderen = Regex::new(r#"ipcRenderer\s*\.\s*(invoke|send|sendSync)\s*\(\s*['"`]([^'"`\n]{1,128})['"`]"#).unwrap();
    let kopru = Regex::new(r#"exposeInMainWorld\s*\(\s*['"`]([A-Za-z_$][\w$]{0,63})['"`]"#).unwrap();
    let kopru_islevi =
        Regex::new(r#"([A-Za-z_$][\w$]{0,63})\s*:\s*(?:async\s*)?\([^)]*\)\s*=>\s*ipcRenderer\s*\.\s*(?:invoke|send|sendSync)\s*\(\s*['"`]([^'"`\n]{1,128})['"`]"#).unwrap();
    let tercih = Regex::new(r"\b(contextIsolation|nodeIntegration|nodeIntegrationInWorker|sandbox|webSecurity|allowRunningInsecureContent)\s*:\s*(true|false)\b").unwrap();
    let sayfa = Regex::new(r#"\.\s*(loadFile|loadURL)\s*\(\s*(?:path\.join\([^)]*['"`]([^'"`]+\.html)['"`]\s*\)|['"`]([^'"`\n]{1,200})['"`])"#).unwrap();
    let mut kanallar: BTreeMap<String, Value> = BTreeMap::new();
    let mut kopruler = vec![];
    // köprü adı -> işlev -> kanal
    let mut islevler: HashMap<String, String> = HashMap::new();
    let mut tercihler = vec![];
    let mut sayfalar = vec![];
    let kanal = |k: &mut BTreeMap<String, Value>, ad: &str| k.entry(ad.to_string()).or_insert_with(|| json!({ "kanal": ad, "isleyiciler": [], "cagiranlar": [] })).clone();
    for (yol, metin) in dosyalar {
        for c in isleyici.captures_iter(metin) {
            let ad = crate::guvenli::temiz(&c[2]);
            let mut k = kanal(&mut kanallar, &ad);
            k["isleyiciler"].as_array_mut().unwrap().push(json!({ "dosya": yol, "satir": satir(metin, c.get(0).unwrap().start()), "yontem": &c[1] }));
            kanallar.insert(ad, k);
        }
        for c in gonderen.captures_iter(metin) {
            let ad = crate::guvenli::temiz(&c[2]);
            let mut k = kanal(&mut kanallar, &ad);
            k["cagiranlar"].as_array_mut().unwrap().push(json!({ "dosya": yol, "satir": satir(metin, c.get(0).unwrap().start()), "yontem": &c[1], "yol": "dogrudan" }));
            kanallar.insert(ad, k);
        }
        for c in kopru.captures_iter(metin) {
            kopruler.push(json!({ "ad": &c[1], "dosya": yol }));
            for f in kopru_islevi.captures_iter(metin) {
                islevler.insert(format!("{}.{}", &c[1], &f[1]), crate::guvenli::temiz(&f[2]));
            }
        }
        for c in tercih.captures_iter(metin) {
            tercihler.push(json!({ "ad": &c[1], "deger": &c[2] == "true", "dosya": yol, "satir": satir(metin, c.get(0).unwrap().start()) }));
        }
        for c in sayfa.captures_iter(metin) {
            let s = c.get(2).or(c.get(3)).map(|m| m.as_str()).unwrap_or("");
            // Uzak adres yalnızca kökeniyle (yol ve sorgu değer olabilir)
            let s = if s.starts_with("http") { koken(s) } else { crate::guvenli::temiz(s) };
            sayfalar.push(json!({ "yontem": &c[1], "sayfa": s, "dosya": yol }));
        }
    }
    // Ön yüzün köprü işlevi çağrıları: `window.<köprü>.<işlev>(` ya da `<köprü>.<işlev>(` → kanal
    if !islevler.is_empty() {
        let cagri = Regex::new(r"(?:window\s*\.\s*)?\b([A-Za-z_$][\w$]{0,63})\s*\.\s*([A-Za-z_$][\w$]{0,63})\s*\(").unwrap();
        for (yol, metin) in dosyalar {
            for c in cagri.captures_iter(metin) {
                let Some(ad) = islevler.get(&format!("{}.{}", &c[1], &c[2])) else { continue };
                let mut k = kanal(&mut kanallar, ad);
                k["cagiranlar"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({ "dosya": yol, "satir": satir(metin, c.get(0).unwrap().start()), "yontem": "kopru", "yol": format!("{}.{}", &c[1], &c[2]) }));
                kanallar.insert(ad.clone(), k);
            }
        }
    }
    json!({ "kanallar": kanallar.into_values().collect::<Vec<_>>(), "kopruler": kopruler, "web_tercihleri": tercihler, "sayfalar": sayfalar })
}

/// Paketteki JS/TS ve HTML'yi geçici klasöre çıkarır (yalnızca güvenli göreli yollar)
fn cikar(asar: &mut Asar, hedef: &Path) -> usize {
    let mut n = 0;
    let yollar: Vec<String> = asar.girdiler.iter().filter(|g| js_mi(&g.0) || g.0.ends_with(".html") || g.0 == "package.json").map(|g| g.0.clone()).collect();
    for y in yollar {
        let p = hedef.join(&y);
        if Path::new(&y).components().any(|c| !matches!(c, Component::Normal(_))) || !p.starts_with(hedef) {
            continue;
        }
        if let Some(b) = asar.oku(&y) {
            if let Some(ust) = p.parent() {
                let _ = std::fs::create_dir_all(ust);
            }
            if std::fs::write(&p, b).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// Paketin yanındaki Electron ikilisinin fuse teli (işaretten sonra: sürüm, uzunluk, durumlar '0' '1' 'r')
pub(crate) fn fuselar(yol: &Path) -> Option<Value> {
    let mut f = std::fs::File::open(yol).ok()?;
    let mut onceki: Vec<u8> = vec![];
    let mut tampon = vec![0u8; 1 << 20];
    let mut okunan = 0u64;
    loop {
        let n = f.read(&mut tampon).ok()?;
        if n == 0 || okunan > (1 << 30) {
            return None;
        }
        okunan += n as u64;
        let mut parca = std::mem::take(&mut onceki);
        parca.extend_from_slice(&tampon[..n]);
        if let Some(i) = parca.windows(FUSE_ISARETI.len()).position(|w| w == FUSE_ISARETI) {
            let mut tel = parca[i + FUSE_ISARETI.len()..].to_vec();
            let mut ek = vec![0u8; 64];
            if let Ok(m) = f.read(&mut ek) {
                tel.extend_from_slice(&ek[..m]);
            }
            let (surum, uzunluk) = (*tel.first()?, *tel.get(1)? as usize);
            let durumlar: Vec<Value> = tel
                .get(2..2 + uzunluk.min(64))?
                .iter()
                .enumerate()
                .map(|(j, c)| {
                    json!({ "ad": FUSELAR.get(j).copied().unwrap_or("bilinmeyen"), "durum": match c { b'1' => "acik", b'0' => "kapali", b'r' => "kaldirildi", _ => "bilinmiyor" } })
                })
                .collect();
            return Some(json!({ "bicim": surum, "fuselar": durumlar }));
        }
        let k = parca.len().saturating_sub(FUSE_ISARETI.len());
        onceki = parca[k..].to_vec();
    }
}

/// `app.asar`'ın Electron ikilisi: paketin `resources/` klasörünün üstündeki çalıştırılabilir dosyalar
fn electron_ikilisi(asar: &Path) -> Option<PathBuf> {
    let ust = asar.parent()?.parent()?;
    let mut adaylar: Vec<PathBuf> = std::fs::read_dir(ust).ok()?.flatten().map(|g| g.path()).filter(|p| p.is_file()).collect();
    adaylar.sort();
    adaylar.into_iter().find(|p| {
        let mut b = [0u8; 4];
        let imza = std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut b)).is_ok() && (&b == b"\x7fELF" || &b[..2] == b"MZ" || b == [0xcf, 0xfa, 0xed, 0xfe]);
        imza && std::fs::metadata(p).is_ok_and(|m| m.len() > (1 << 10))
    })
}

/// Electron paketini inceler: kanıtlar `Sonuc`a, REA adımları dönüş değerinde
pub(crate) fn incele(b: &mut Baglam, oge: &Oge, s: &mut Sonuc) -> Vec<Value> {
    let sha = super::genis::ozet(&oge.yol);
    let mut adimlar = vec![];
    let mut asar = match Asar::ac(&oge.yol) {
        Ok(a) => a,
        Err(e) => {
            s.bilinmeyenler.push(bilinmeyen(oge, "Electron paketi", &e));
            return adimlar;
        }
    };
    let pj: Value = asar.oku("package.json").and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    let js: Vec<String> = asar.girdiler.iter().filter(|g| js_mi(&g.0)).map(|g| g.0.clone()).collect();
    s.kanitlar.push(kanit(
        "BINARY_METADATA",
        "structure",
        "observed",
        "shipped-artifact",
        "sezgi/asar@1",
        oge,
        &sha,
        Some("paket".into()),
        json!({
            "bicim": "asar",
            "ad": pj["name"].as_str().map(crate::guvenli::temiz),
            "urun_adi": pj["productName"].as_str().map(crate::guvenli::temiz),
            "surum": pj["version"].as_str().map(crate::guvenli::temiz),
            "giris": pj["main"].as_str().map(crate::guvenli::temiz),
            "dosya": asar.girdiler.len(),
            "js": js.len(),
            "paket_disinda": asar.girdiler.iter().filter(|g| g.3).count(),
        }),
        None,
    ));
    // IPC zinciri (desenle)
    let mut toplam = 0u64;
    let mut dosyalar = vec![];
    for y in &js {
        if toplam > EN_COK_JS {
            break;
        }
        if let Some(b) = asar.oku(y) {
            toplam += b.len() as u64;
            dosyalar.push((y.clone(), String::from_utf8_lossy(&b).into_owned()));
        }
    }
    let ipc = ipc_cozumle(&dosyalar);
    let kanal_sayisi = ipc["kanallar"].as_array().map_or(0, Vec::len);
    s.kanitlar.push(kanit("BINARY_HEURISTIC", "call_target", "inferred", "shipped-artifact", "sezgi/asar@1", oge, &sha, Some("ipc".into()), ipc, None));
    // Ön yüz bağları: Sezgi'nin web adaptörü, çıkarılan paket üzerinde
    let mut cagri_sayisi = 0;
    match super::genis::gecici_klasor() {
        Ok(k) => {
            let n = cikar(&mut asar, &k);
            if n > 0 {
                let cagrilar: Vec<Value> = crate::web::istemci_cagrilari(&k)
                    .into_iter()
                    .take(5000)
                    .map(|c| json!({ "dosya": c.dosya, "satir": c.satir, "yontem": c.yontem, "sablon": c.sablon }))
                    .collect();
                let baglar = crate::web::baglar(&k, &[], "", &mut HashMap::new()).deger;
                let ui: Vec<Value> = baglar["baglar"].as_array().into_iter().flatten().filter(|x| x["iliski"] != "CALLS").take(5000).cloned().collect();
                cagri_sayisi = cagrilar.len();
                s.kanitlar.push(kanit(
                    "BINARY_HEURISTIC",
                    "call_target",
                    "inferred",
                    "shipped-artifact",
                    "sezgi/web",
                    oge,
                    &sha,
                    Some("web".into()),
                    json!({ "cagrilar": cagrilar, "baglar": ui }),
                    None,
                ));
            }
            let _ = std::fs::remove_dir_all(&k);
        }
        Err(e) => s.bilinmeyenler.push(bilinmeyen(oge, "Electron ön yüz bağları", &e)),
    }
    // Fuse'lar (paketin yanındaki ikili)
    if let Some(ikili) = electron_ikilisi(&oge.yol) {
        let goreli = oge.goreli.rsplit_once('/').and_then(|(u, _)| u.rsplit_once('/')).map(|(u, _)| u.to_string()).unwrap_or_default();
        let ad = ikili.file_name().map(|a| a.to_string_lossy().to_string()).unwrap_or_default();
        let io = Oge { yol: ikili.clone(), goreli: if goreli.is_empty() { ad } else { format!("{goreli}/{ad}") }, tur: "pe" };
        match fuselar(&ikili) {
            Some(f) => {
                let isha = super::genis::ozet(&ikili);
                s.kanitlar.push(kanit("BINARY_METADATA", "structure", "observed", "shipped-artifact", "sezgi/electron-fuse@1", &io, &isha, Some("fuses".into()), f, None));
            }
            None => s.bilinmeyenler.push(bilinmeyen(&io, "Electron fuse'ları", "ikilide fuse işareti bulunamadı")),
        }
    }
    s.satirlar.push(tf!(
        "  Electron {}  {} {} · {} JS · {kanal_sayisi} IPC kanalı · {cagri_sayisi} ağ çağrısı",
        "  Electron {}  {} {} · {} JS · {kanal_sayisi} IPC channels · {cagri_sayisi} network calls",
        oge.goreli,
        pj["name"].as_str().unwrap_or("?"),
        pj["version"].as_str().unwrap_or(""),
        js.len()
    ));
    // REA: onaylı sınıf (süreç başlatır, dosyaya yazar)
    if !b.sec.rea {
        adimlar.push(json!({ "arac": "analyze-javascript-application", "durum": "istenmedi" }));
        return adimlar;
    }
    let yol = oge.yol.to_string_lossy().to_string();
    match super::genis::calistir(b, "analyze-javascript-application", &[&yol], &format!("{} Electron uygulamasının statik JavaScript analizi (REA)", oge.goreli))
        .and_then(|(y, _)| g::oku::<Value>(&y, "analyze_javascript_application"))
    {
        Ok(z) => {
            let o = rea_projeksiyonu(z.normalized_result.as_ref().unwrap_or(&Value::Null));
            s.kanitlar.push(kanit(
                "BINARY_HEURISTIC",
                "call_target",
                z.guven(),
                z.authority.as_deref().unwrap_or("bilinmiyor"),
                &z.saglayici(),
                oge,
                &sha,
                Some("analyze_javascript_application".into()),
                o,
                z.evidence_id.clone(),
            ));
            adimlar.push(json!({ "arac": "analyze-javascript-application", "durum": "calisti" }));
        }
        Err(e) => {
            s.bilinmeyenler.push(bilinmeyen(oge, "Electron JavaScript anlamsal analizi (REA)", &e));
            adimlar.push(json!({ "arac": "analyze-javascript-application", "durum": "calistirilmadi", "neden": e }));
        }
    }
    adimlar
}

/// REA uygulama grafiğinin biçim projeksiyonu: IPC işleyicileri ve kanalları, işleyici eşleşmeleri, köprü, pencere ayarları
fn rea_projeksiyonu(r: &Value) -> Value {
    let dugumler = r["graph"]["nodes"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let ozellik = |n: &Value| n["observations"][0]["properties"].clone();
    let ad = |v: &Value| v.as_str().map(crate::guvenli::temiz);
    let isleyiciler: Vec<Value> = dugumler
        .iter()
        .filter(|n| n["kind"] == "ipc-handler")
        .take(5000)
        .map(|n| {
            let p = ozellik(n);
            json!({ "kanal": ad(&p["channel"]), "islem": ad(&p["operation"]), "satir": p["handler_location"]["start"]["line"] })
        })
        .collect();
    let kanallar: Vec<Value> = dugumler
        .iter()
        .filter(|n| n["kind"] == "ipc-channel")
        .take(5000)
        .map(|n| {
            let p = ozellik(n);
            json!({ "kanal": ad(&p["channel"]), "taraf": ad(&p["side"]), "islem": ad(&p["operation"]) })
        })
        .collect();
    let kopruler: Vec<Value> = dugumler
        .iter()
        .filter(|n| n["kind"] == "context-bridge-api")
        .map(|n| {
            let p = ozellik(n);
            json!({ "ad": ad(&p["api_key"]), "uyeler": p["members"].as_array().map(|m| m.iter().filter_map(&ad).collect::<Vec<_>>()) })
        })
        .collect();
    let tercihler: Vec<Value> = dugumler
        .iter()
        .filter(|n| n["kind"] == "browser-window")
        .flat_map(|n| ozellik(n)["web_preferences"].as_array().cloned().unwrap_or_default())
        .map(|w| json!({ "ad": ad(&w["name"]), "deger": w["value"]["value"].as_bool() }))
        .collect();
    let eslesmeler: Vec<Value> = r["graph"]["edges"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| matches!(e["relation"].as_str(), Some("invokes" | "sends")) && e["properties"]["handler_path"].is_string())
        .take(5000)
        .map(|e| json!({ "kanal": ad(&e["properties"]["channel"]), "isleyici_dosyasi": ad(&e["properties"]["handler_path"]), "esleme": ad(&e["properties"]["pairing_basis"]) }))
        .collect();
    json!({
        "isleyiciler": isleyiciler,
        "kanallar": kanallar,
        "eslesmeler": eslesmeler,
        "kopruler": kopruler,
        "web_tercihleri": tercihler,
        "ozet": r["summary"]["ipc"],
    })
}
