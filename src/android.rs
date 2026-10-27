//! Android paketi (APK): kaynak kod olmadan uygulamanın bildirimini (AndroidManifest.xml) okur. APK bir ZIP'tir;
//! bildirim içinde ikili XML (AXML) olarak durur. Çıkanlar: paket adı, sürüm, SDK düzeyleri, istenen ve tanımlanan
//! izinler, uygulama ayarları (ağ güvenlik yapılandırması, açık metin trafiği, hata ayıklama, yedekleme) ve bileşenler
//! (activity, activity-alias, service, receiver, provider: ad, dışa açıklık, izin, intent filtreleri, başlatıcı).
//!
//! Kanıt türü BINARY_METADATA; tavanı `observed` (güven ölçeği certain > observed > inferred > unknown): bildirimde
//! yazanı gözler, kodun ne yaptığını söylemez. Kaynak kimlikleri (`resources.arsc`) çözülmez, `@0x7f…` olarak kalır.
//!
//! Girdi güvenilmezdir (kasıtlı bozulmuş APK'lar analiz araçlarını kırmak için yapılır): her okuma sınır denetimlidir,
//! hiçbir girdi paniğe yol açmaz. Sınırlar: merkezi dizin 64 MiB, bildirim sıkıştırılmış ve açılmış 8 MiB (ZIP bombası),
//! öğe ve öznitelik sayısı, iç içelik derinliği. ZIP64 desteklenmez (APK'larda kullanılmaz).
use serde_json::{json, Map, Value};
#[cfg(test)]
use std::io::Cursor;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Kanıtı üreten okuyucunun kimliği
pub(crate) const URETICI: &str = "sezgi/android@1";
/// Çıktının biçim sürümü
pub(crate) const BICIM: u64 = 1;

const EN_BUYUK_MERKEZI_DIZIN: u64 = 64 << 20;
const EN_COK_GIRDI: usize = 500_000;
/// Bildirimin sıkıştırılmış ve açılmış en büyük boyu
const EN_BUYUK_BILDIRIM: usize = 8 << 20;
const EN_DERIN: usize = 128;
const EN_COK_OGE: usize = 50_000;
const EN_COK_OZNITELIK: usize = 200_000;
/// Bir dizginin okunan en çok karakteri (çıktıda `guvenli::temiz` ayrıca 300 karaktere indirir)
const EN_UZUN_DIZGI: usize = 4096;

// ---- Sınır denetimli okuma ----

fn u16_al(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i.checked_add(2)?)?.try_into().ok()?))
}

fn u32_al(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i.checked_add(4)?)?.try_into().ok()?))
}

// ---- ZIP ----

/// Merkezi dizindeki bir girdi
struct Girdi {
    ad: String,
    yontem: u16,
    sikistirilmis: u64,
    acik: u64,
    yerel: u64,
}

fn tam_oku<R: Read + Seek>(r: &mut R, konum: u64, uzunluk: usize) -> Result<Vec<u8>, String> {
    r.seek(SeekFrom::Start(konum)).map_err(|e| format!("okunamadı: {e}"))?;
    let mut v = vec![0u8; uzunluk];
    r.read_exact(&mut v).map_err(|_| "dosya beklenenden kısa (kesik ZIP)".to_string())?;
    Ok(v)
}

/// Merkezi dizinin sonu (EOCD) ve merkezi dizin
fn merkezi_dizin<R: Read + Seek>(r: &mut R, boyut: u64) -> Result<Vec<Girdi>, String> {
    let kuyruk_boyu = boyut.min(22 + 0xFFFF);
    let kuyruk = tam_oku(r, boyut - kuyruk_boyu, kuyruk_boyu as usize)?;
    let i = (0..kuyruk.len().saturating_sub(21)).rev().find(|&i| kuyruk.get(i..i + 4) == Some(b"PK\x05\x06")).ok_or("ZIP değil: merkezi dizin sonu bulunamadı")?;
    let sayi = u16_al(&kuyruk, i + 10).ok_or("merkezi dizin sonu kesik")?;
    let dizin_boyu = u32_al(&kuyruk, i + 12).ok_or("merkezi dizin sonu kesik")?;
    let dizin_konumu = u32_al(&kuyruk, i + 16).ok_or("merkezi dizin sonu kesik")?;
    if sayi == 0xFFFF || dizin_boyu == 0xFFFF_FFFF || dizin_konumu == 0xFFFF_FFFF {
        return Err("ZIP64 desteklenmiyor".into());
    }
    let (dizin_boyu, dizin_konumu) = (u64::from(dizin_boyu), u64::from(dizin_konumu));
    if dizin_boyu > EN_BUYUK_MERKEZI_DIZIN {
        return Err(format!("merkezi dizin çok büyük ({dizin_boyu} bayt)"));
    }
    if dizin_konumu.checked_add(dizin_boyu).is_none_or(|s| s > boyut) {
        return Err("merkezi dizin dosyanın dışını gösteriyor".into());
    }
    let d = tam_oku(r, dizin_konumu, dizin_boyu as usize)?;
    let mut girdiler = vec![];
    let mut j = 0usize;
    while j + 46 <= d.len() {
        if d.get(j..j + 4) != Some(b"PK\x01\x02") {
            return Err("merkezi dizin bozuk (girdi imzası yok)".into());
        }
        let alan = |k: usize| u32_al(&d, j + k).map(u64::from).ok_or("merkezi dizin girdisi kesik");
        let yontem = u16_al(&d, j + 10).ok_or("merkezi dizin girdisi kesik")?;
        let (sikistirilmis, acik, yerel) = (alan(20)?, alan(24)?, alan(42)?);
        let n = usize::from(u16_al(&d, j + 28).ok_or("merkezi dizin girdisi kesik")?);
        let ek = usize::from(u16_al(&d, j + 30).ok_or("merkezi dizin girdisi kesik")?);
        let yorum = usize::from(u16_al(&d, j + 32).ok_or("merkezi dizin girdisi kesik")?);
        let ad = d.get(j + 46..j + 46 + n).ok_or("merkezi dizin girdisi kesik (ad)")?;
        girdiler.push(Girdi { ad: String::from_utf8_lossy(ad).into_owned(), yontem, sikistirilmis, acik, yerel });
        if girdiler.len() > EN_COK_GIRDI {
            return Err("ZIP'te çok fazla girdi".into());
        }
        j += 46 + n + ek + yorum;
    }
    Ok(girdiler)
}

/// Girdinin açılmış içeriği. Bilinmeyen sıkıştırma yöntemi Android'in yaptığı gibi saklanmış (sıkıştırmasız) sayılır
/// ve `notlar`a yazılır (araçları kırmak için kullanılan bir hile)
fn girdi_oku<R: Read + Seek>(r: &mut R, boyut: u64, g: &Girdi, notlar: &mut Vec<String>) -> Result<Vec<u8>, String> {
    let yerel = tam_oku(r, g.yerel, 30).map_err(|_| format!("{}: yerel başlık dosyanın dışında", g.ad))?;
    if yerel.get(0..4) != Some(b"PK\x03\x04") {
        return Err(format!("{}: yerel başlık imzası yok", g.ad));
    }
    let n = u64::from(u16_al(&yerel, 26).unwrap_or(0));
    let ek = u64::from(u16_al(&yerel, 28).unwrap_or(0));
    let veri = g.yerel + 30 + n + ek;
    if g.sikistirilmis > EN_BUYUK_BILDIRIM as u64 {
        return Err(format!("{}: sıkıştırılmış boyu sınırı aşıyor ({} bayt)", g.ad, g.sikistirilmis));
    }
    if veri.checked_add(g.sikistirilmis).is_none_or(|s| s > boyut) {
        return Err(format!("{}: veri dosyanın dışını gösteriyor", g.ad));
    }
    let ham = tam_oku(r, veri, g.sikistirilmis as usize)?;
    match g.yontem {
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(&ham, EN_BUYUK_BILDIRIM)
            .map_err(|_| format!("{}: açılamadı ya da açılmış boyu {} MiB sınırını aşıyor (ZIP bombası?)", g.ad, EN_BUYUK_BILDIRIM >> 20)),
        0 => Ok(ham),
        y => {
            notlar.push(format!("{}: bilinmeyen sıkıştırma yöntemi ({y}); Android gibi sıkıştırmasız sayıldı", g.ad));
            Ok(ham)
        }
    }
}

// ---- AXML (ikili XML) ----

/// Ayrıştırılmış bir XML öğesi. Öznitelik adları önekisizdir (`android:name` -> `name`)
#[derive(Debug, Default, Clone)]
pub(crate) struct Oge {
    pub ad: String,
    pub oznitelikler: Vec<(String, Value)>,
    pub cocuklar: Vec<Oge>,
}

impl Oge {
    fn al(&self, ad: &str) -> Option<&Value> {
        self.oznitelikler.iter().find(|(a, _)| a == ad).map(|(_, d)| d)
    }
    fn metin(&self, ad: &str) -> Option<String> {
        self.al(ad).and_then(|d| d.as_str().map(String::from))
    }
    fn mantik(&self, ad: &str) -> Option<bool> {
        self.al(ad).and_then(|d| d.as_bool().or_else(|| d.as_str().and_then(|s| s.parse().ok())))
    }
    fn cocuk<'a>(&'a self, ad: &'a str) -> impl Iterator<Item = &'a Oge> + 'a {
        self.cocuklar.iter().filter(move |c| c.ad == ad)
    }
}

/// Dizgi havuzu: dizgiler istendiğinde çözülür (çok sayıda girdinin aynı uzun dizgiyi göstermesi belleği şişirmesin)
struct Havuz<'a> {
    parca: &'a [u8],
    ofsetler: usize,
    sayi: usize,
    baslangic: usize,
    utf8: bool,
}

impl Havuz<'_> {
    fn bos() -> Havuz<'static> {
        Havuz { parca: &[], ofsetler: 0, sayi: 0, baslangic: 0, utf8: true }
    }

    fn kur(p: &[u8], baslik: usize) -> Result<Havuz<'_>, String> {
        let sayi = u32_al(p, 8).ok_or("dizgi havuzu kesik")? as usize;
        let bayrak = u32_al(p, 16).ok_or("dizgi havuzu kesik")?;
        let baslangic = u32_al(p, 20).ok_or("dizgi havuzu kesik")? as usize;
        if sayi > p.len().saturating_sub(baslik) / 4 {
            return Err("dizgi havuzu: dizgi sayısı parçaya sığmıyor".into());
        }
        Ok(Havuz { parca: p, ofsetler: baslik, sayi, baslangic, utf8: bayrak & 0x100 != 0 })
    }

    /// `i`. dizgi; havuzda yoksa ya da bozuksa `None`
    fn al(&self, i: u32) -> Option<String> {
        let i = i as usize;
        if i >= self.sayi {
            return None;
        }
        let ofset = u32_al(self.parca, self.ofsetler + i * 4)? as usize;
        let k = self.baslangic.checked_add(ofset)?;
        let p = self.parca;
        if self.utf8 {
            // Uzunluk önekleri: karakter sayısı, sonra bayt sayısı (her biri 1 ya da 2 bayt)
            let uzunluk = |k: usize| -> Option<(usize, usize)> {
                let a = usize::from(*p.get(k)?);
                if a & 0x80 != 0 {
                    Some((((a & 0x7F) << 8) | usize::from(*p.get(k + 1)?), 2))
                } else {
                    Some((a, 1))
                }
            };
            let (_, a) = uzunluk(k)?;
            let (bayt, b) = uzunluk(k + a)?;
            let bas = k + a + b;
            let veri = p.get(bas..bas.checked_add(bayt.min(EN_UZUN_DIZGI * 4))?.min(p.len()))?;
            Some(String::from_utf8_lossy(veri).chars().take(EN_UZUN_DIZGI).collect())
        } else {
            let a = usize::from(u16_al(p, k)?);
            let (uzun, bas) = if a & 0x8000 != 0 { (((a & 0x7FFF) << 16) | usize::from(u16_al(p, k + 2)?), k + 4) } else { (a, k + 2) };
            let birim: Vec<u16> = (0..uzun.min(EN_UZUN_DIZGI)).map_while(|j| u16_al(p, bas + j * 2)).collect();
            Some(String::from_utf16_lossy(&birim))
        }
    }
}

/// Android çerçeve özniteliklerinin kaynak kimlikleri. Android öznitelikleri adlarıyla değil bu kimliklerle tanır:
/// ad dizgisi değiştirilmiş (karartılmış) bir bildirimde de doğru öznitelik okunur
fn bilinen_oznitelik(kimlik: u32) -> Option<&'static str> {
    Some(match kimlik {
        0x0101_0001 => "label",
        0x0101_0003 => "name",
        0x0101_0006 => "permission",
        0x0101_0009 => "protectionLevel",
        0x0101_000E => "enabled",
        0x0101_000F => "debuggable",
        0x0101_0010 => "exported",
        0x0101_0018 => "authorities",
        0x0101_0024 => "value",
        0x0101_0026 => "mimeType",
        0x0101_0027 => "scheme",
        0x0101_0028 => "host",
        0x0101_0029 => "port",
        0x0101_002A => "path",
        0x0101_002B => "pathPrefix",
        0x0101_002C => "pathPattern",
        0x0101_0202 => "targetActivity",
        0x0101_020C => "minSdkVersion",
        0x0101_021B => "versionCode",
        0x0101_021C => "versionName",
        0x0101_0270 => "targetSdkVersion",
        0x0101_0271 => "maxSdkVersion",
        0x0101_0280 => "allowBackup",
        0x0101_028E => "required",
        0x0101_04EC => "usesCleartextTraffic",
        0x0101_0527 => "networkSecurityConfig",
        0x0101_0599 => "foregroundServiceType",
        _ => return None,
    })
}

/// Öznitelik değeri. Kaynak kimlikleri `@0x7f…` olarak bırakılır (kaynak tablosu çözülmez)
fn deger(havuz: &Havuz, ham: u32, tur: u8, veri: u32) -> Value {
    match tur {
        0x03 => json!(havuz.al(if ham != u32::MAX { ham } else { veri }).map(|s| crate::guvenli::temiz(&s)).unwrap_or_default()),
        0x01 => json!(format!("@0x{veri:08x}")),
        0x02 => json!(format!("?0x{veri:08x}")),
        0x10 | 0x11 => json!(veri as i32),
        0x12 => json!(veri != 0),
        0x1C..=0x1F => json!(format!("#{veri:08x}")),
        0x04 => serde_json::Number::from_f64(f64::from(f32::from_bits(veri))).map(Value::Number).unwrap_or(Value::Null),
        0x00 => Value::Null,
        _ => match havuz.al(ham) {
            Some(s) if ham != u32::MAX => json!(crate::guvenli::temiz(&s)),
            _ => json!(format!("0x{veri:08x}")),
        },
    }
}

/// İkili XML'i öğe ağacına çevirir; kök öğeyi döner
pub(crate) fn axml_coz(b: &[u8]) -> Result<Oge, String> {
    if u16_al(b, 0) != Some(0x0003) {
        return Err("ikili XML değil".into());
    }
    let baslik = usize::from(u16_al(b, 2).ok_or("ikili XML kesik")?);
    // Bildirilen boy dosyadan büyükse (araçları kırmak için) dosya boyu kullanılır
    let son = (u32_al(b, 4).ok_or("ikili XML kesik")? as usize).min(b.len());
    if baslik < 8 || baslik > son {
        return Err("ikili XML başlığı geçersiz".into());
    }
    let mut havuz = Havuz::bos();
    let mut kimlikler: &[u8] = &[];
    let mut yigin: Vec<Oge> = vec![];
    let mut kokler: Vec<Oge> = vec![];
    let (mut oge_sayisi, mut oznitelik_sayisi) = (0usize, 0usize);
    let mut i = baslik;
    while i + 8 <= son {
        let tur = u16_al(b, i).unwrap_or(0);
        let hb = usize::from(u16_al(b, i + 2).unwrap_or(0));
        let boy = u32_al(b, i + 4).unwrap_or(0) as usize;
        if boy < 8 || hb < 8 || hb > boy || i.checked_add(boy).is_none_or(|s| s > son) {
            return Err(format!("ikili XML parçası geçersiz (konum {i})"));
        }
        let p = &b[i..i + boy];
        match tur {
            // Dizgi havuzu (ilki geçerli)
            0x0001 if havuz.sayi == 0 => havuz = Havuz::kur(p, hb)?,
            // Öznitelik adı -> kaynak kimliği eşlemesi
            0x0180 => kimlikler = &p[hb..],
            // Öğe başlangıcı
            0x0102 => {
                if yigin.len() >= EN_DERIN {
                    return Err("ikili XML çok derin".into());
                }
                oge_sayisi += 1;
                if oge_sayisi > EN_COK_OGE {
                    return Err("ikili XML'de çok fazla öğe".into());
                }
                let ad = havuz.al(u32_al(p, hb + 4).ok_or("öğe başlığı kesik")?).map(|s| crate::guvenli::temiz(&s)).unwrap_or_default();
                let oz_bas = usize::from(u16_al(p, hb + 8).ok_or("öğe başlığı kesik")?);
                let oz_boy = usize::from(u16_al(p, hb + 10).ok_or("öğe başlığı kesik")?);
                let oz_sayi = usize::from(u16_al(p, hb + 12).ok_or("öğe başlığı kesik")?);
                if oz_sayi > 0 && oz_boy < 20 {
                    return Err("öznitelik boyu geçersiz".into());
                }
                oznitelik_sayisi += oz_sayi;
                if oznitelik_sayisi > EN_COK_OZNITELIK {
                    return Err("ikili XML'de çok fazla öznitelik".into());
                }
                let mut oge = Oge { ad, ..Default::default() };
                for j in 0..oz_sayi {
                    let a = hb + oz_bas + j * oz_boy;
                    let (Some(ad_i), Some(ham), Some(veri)) = (u32_al(p, a + 4), u32_al(p, a + 8), u32_al(p, a + 16)) else {
                        return Err("öznitelik parçanın dışında".into());
                    };
                    let tur = *p.get(a + 15).ok_or("öznitelik parçanın dışında")?;
                    let kimlik = u32_al(kimlikler, ad_i as usize * 4);
                    let ad = match kimlik.and_then(bilinen_oznitelik) {
                        Some(a) => a.to_string(),
                        None => match havuz.al(ad_i).filter(|s| !s.is_empty()) {
                            Some(s) => crate::guvenli::temiz(&s),
                            None => kimlik.map(|k| format!("0x{k:08x}")).unwrap_or_default(),
                        },
                    };
                    oge.oznitelikler.push((ad, deger(&havuz, ham, tur, veri)));
                }
                yigin.push(oge);
            }
            // Öğe sonu
            0x0103 => {
                if let Some(o) = yigin.pop() {
                    match yigin.last_mut() {
                        Some(ust) => ust.cocuklar.push(o),
                        None => kokler.push(o),
                    }
                }
            }
            _ => {}
        }
        i += boy;
    }
    // Kapanmamış öğeler (kesik dosya) ağaca eklenir
    while let Some(o) = yigin.pop() {
        match yigin.last_mut() {
            Some(ust) => ust.cocuklar.push(o),
            None => kokler.push(o),
        }
    }
    kokler.into_iter().next().ok_or_else(|| "ikili XML'de öğe yok".into())
}

// ---- Bildirimden üst veri ----

/// `.Ana` -> `paket.Ana`; noktasız ad paketin içindedir
fn sinif(paket: &str, ad: &str) -> String {
    if ad.starts_with('.') {
        format!("{paket}{ad}")
    } else if !ad.contains('.') && !paket.is_empty() {
        format!("{paket}.{ad}")
    } else {
        ad.to_string()
    }
}

fn sayi_ya_da_metin(d: Option<&Value>) -> Value {
    match d {
        Some(Value::String(s)) => s.parse::<i64>().map(|n| json!(n)).unwrap_or_else(|_| json!(s)),
        Some(v) => v.clone(),
        None => Value::Null,
    }
}

fn intent_filtresi(f: &Oge) -> Value {
    let adlar = |tur: &str| f.cocuk(tur).filter_map(|c| c.metin("name")).collect::<Vec<_>>();
    let veri: Vec<Value> = f.cocuk("data").map(|d| Value::Object(d.oznitelikler.iter().map(|(a, v)| (a.clone(), v.clone())).collect::<Map<_, _>>())).collect();
    let mut v = json!({ "eylemler": adlar("action"), "kategoriler": adlar("category") });
    if !veri.is_empty() {
        v["veri"] = json!(veri);
    }
    v
}

fn bilesen(o: &Oge, paket: &str, hedef_sdk: Option<i64>) -> Value {
    let filtreler: Vec<Value> = o.cocuk("intent-filter").map(intent_filtresi).collect();
    let icerir = |f: &Value, alan: &str, deger: &str| f[alan].as_array().is_some_and(|a| a.iter().any(|x| x == deger));
    let baslatici = matches!(o.ad.as_str(), "activity" | "activity-alias")
        && filtreler.iter().any(|f| icerir(f, "eylemler", "android.intent.action.MAIN") && icerir(f, "kategoriler", "android.intent.category.LAUNCHER"));
    let belirtilen = o.mantik("exported");
    // Belirtilmemişse Android'in varsayılanı: provider için hedef SDK 17 ve sonrasında kapalı; diğerlerinde intent
    // filtresi varsa açık
    let varsayilan = if o.ad == "provider" { hedef_sdk.is_some_and(|s| s < 17) } else { !filtreler.is_empty() };
    let mut v = json!({
        "tur": o.ad,
        "ad": sinif(paket, &o.metin("name").unwrap_or_default()),
        "disa_acik": belirtilen.unwrap_or(varsayilan),
        "disa_acik_belirtilmis": belirtilen.is_some(),
        "etkin": o.mantik("enabled").unwrap_or(true),
        "izin": o.metin("permission"),
        "baslatici": baslatici,
        "intent_filtreleri": filtreler,
    });
    if let Some(a) = o.metin("authorities") {
        v["yetkililer"] = json!(a.split(';').map(str::to_string).collect::<Vec<_>>());
    }
    if let Some(h) = o.metin("targetActivity") {
        v["hedef"] = json!(sinif(paket, &h));
    }
    if let Some(t) = o.al("foregroundServiceType") {
        v["on_plan_turu"] = t.clone();
    }
    v
}

/// Bildirim ağacından üst veri (kanıt ve dosya bilgisi hariç)
pub(crate) fn bildirim(kok: &Oge) -> Result<Value, String> {
    if kok.ad != "manifest" {
        return Err(format!("kök öğe manifest değil: {}", kok.ad));
    }
    let paket = kok.metin("package").unwrap_or_default();
    let sdk = kok.cocuk("uses-sdk").next();
    let sdk_al = |ad: &str| sayi_ya_da_metin(sdk.and_then(|s| s.al(ad)));
    let hedef_sdk = sdk_al("targetSdkVersion").as_i64();
    let izinler: Vec<Value> = kok
        .cocuklar
        .iter()
        .filter(|c| c.ad == "uses-permission" || c.ad == "uses-permission-sdk-23")
        .map(|c| {
            let mut v = json!({ "ad": c.metin("name").unwrap_or_default() });
            if let Some(m) = c.al("maxSdkVersion") {
                v["en_fazla_sdk"] = sayi_ya_da_metin(Some(m));
            }
            v
        })
        .collect();
    let tanimlanan: Vec<Value> =
        kok.cocuk("permission").map(|c| json!({ "ad": c.metin("name").unwrap_or_default(), "koruma_duzeyi": c.al("protectionLevel").cloned().unwrap_or(Value::Null) })).collect();
    let ozellikler: Vec<Value> = kok.cocuk("uses-feature").map(|c| json!({ "ad": c.metin("name"), "zorunlu": c.mantik("required").unwrap_or(true) })).collect();
    let uyg = kok.cocuk("application").next();
    let uygulama = uyg.map(|a| {
        json!({
            "sinif": a.metin("name").map(|n| sinif(&paket, &n)),
            "etiket": a.al("label").cloned(),
            "hata_ayiklanabilir": a.mantik("debuggable").unwrap_or(false),
            "yedeklenebilir": a.mantik("allowBackup"),
            "acik_metin_trafigi": a.mantik("usesCleartextTraffic"),
            "ag_guvenlik_yapilandirmasi": a.al("networkSecurityConfig").cloned(),
            "izin": a.metin("permission"),
        })
    });
    let bilesenler: Vec<Value> = uyg
        .into_iter()
        .flat_map(|a| a.cocuklar.iter())
        .filter(|c| matches!(c.ad.as_str(), "activity" | "activity-alias" | "service" | "receiver" | "provider"))
        .map(|c| bilesen(c, &paket, hedef_sdk))
        .collect();
    Ok(json!({
        "paket": paket,
        "surum_kodu": sayi_ya_da_metin(kok.al("versionCode")),
        "surum_adi": kok.al("versionName").cloned(),
        "sdk": { "en_az": sdk_al("minSdkVersion"), "hedef": sdk_al("targetSdkVersion"), "en_fazla": sdk_al("maxSdkVersion") },
        "izinler": izinler,
        "tanimlanan_izinler": tanimlanan,
        "ozellikler": ozellikler,
        "uygulama": uygulama,
        "bilesenler": bilesenler,
    }))
}

// ---- APK ----

fn coz<R: Read + Seek>(r: &mut R, boyut: u64, ad: &str, sha256: String) -> Result<Value, String> {
    let girdiler = merkezi_dizin(r, boyut)?;
    let mut notlar = vec![];
    let mut adaylar = girdiler.iter().filter(|g| g.ad == "AndroidManifest.xml");
    let g = adaylar.next().ok_or("APK değil: AndroidManifest.xml yok")?;
    if adaylar.next().is_some() {
        notlar.push("birden fazla AndroidManifest.xml girdisi; ilki okundu".to_string());
    }
    if g.acik > EN_BUYUK_BILDIRIM as u64 {
        return Err(format!("AndroidManifest.xml: açılmış boyu sınırı aşıyor ({} bayt; ZIP bombası?)", g.acik));
    }
    let bayt = girdi_oku(r, boyut, g, &mut notlar)?;
    let mut v = bildirim(&axml_coz(&bayt)?)?;
    let dex = girdiler.iter().filter(|g| g.ad.starts_with("classes") && g.ad.ends_with(".dex") && !g.ad.contains('/')).count();
    let mut mimariler: Vec<String> = girdiler.iter().filter_map(|g| g.ad.strip_prefix("lib/").and_then(|s| s.split_once('/')).map(|(m, _)| crate::guvenli::temiz(m))).collect();
    mimariler.sort();
    mimariler.dedup();
    let imza_dosyasi = girdiler.iter().any(|g| g.ad.starts_with("META-INF/") && [".RSA", ".DSA", ".EC"].iter().any(|u| g.ad.to_ascii_uppercase().ends_with(u)));
    let mut sinirlamalar = vec![
        "APK imzası doğrulanmadı".to_string(),
        "kaynak kimlikleri çözülmedi: @0x… olarak bırakıldı (kaynak tablosu okunmaz)".to_string(),
        "yalnızca bildirim okundu: bileşenin kodda ne yaptığı ya da gerçekten kullanıldığı gözlenmedi".to_string(),
    ];
    sinirlamalar.extend(notlar);
    let o = v.as_object_mut().ok_or("iç hata")?;
    let mut sonuc = Map::new();
    sonuc.insert("bicim".into(), json!(BICIM));
    sonuc.insert("tur".into(), json!("android-apk"));
    sonuc.insert(
        "kanit".into(),
        json!({
            "kaynak_turu": "BINARY_METADATA",
            "tavan": "observed",
            "beyan": "observed",
            "uretici": URETICI,
            "dayanak": [{ "yol": ad, "sha256": sha256 }],
            "konum": ["AndroidManifest.xml"],
            "sinirlamalar": sinirlamalar,
        }),
    );
    sonuc.insert(
        "dosya".into(),
        json!({ "ad": ad, "boyut": boyut, "sha256": sha256, "girdi_sayisi": girdiler.len(), "dex_sayisi": dex, "yerel_kutuphane_mimarileri": mimariler, "imza_dosyasi_var": imza_dosyasi }),
    );
    sonuc.append(o);
    sonuc.insert("guven".into(), json!("observed"));
    Ok(Value::Object(sonuc))
}

/// Bellekteki APK (testler için; ürün dosyadan okur: `apk_oku`). Birim testleri depo dışında tutulduğundan depodaki
/// test hedefinde çağıran yoktur
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn apk_coz(bayt: &[u8], ad: &str) -> Result<Value, String> {
    coz(&mut Cursor::new(bayt), bayt.len() as u64, ad, crate::benlik::sha256_hex(bayt))
}

/// APK dosyasının üst verisi. Sembolik bağ izlenmez; dosyanın tamamı yalnızca SHA-256 için akış olarak okunur,
/// ayrıştırma merkezi dizin ve bildirimle sınırlıdır
pub(crate) fn apk_oku(yol: &Path) -> Result<Value, String> {
    let m = std::fs::symlink_metadata(yol).map_err(|e| format!("{}: {e}", crate::guvenli::gorunen(yol)))?;
    if !m.file_type().is_file() {
        return Err(format!("{}: düzenli dosya değil (sembolik bağ izlenmez)", crate::guvenli::gorunen(yol)));
    }
    let mut f = std::fs::File::open(yol).map_err(|e| format!("{}: {e}", crate::guvenli::gorunen(yol)))?;
    let mut ozet = ring::digest::Context::new(&ring::digest::SHA256);
    let mut tampon = vec![0u8; 1 << 16];
    let mut boyut = 0u64;
    loop {
        let n = f.read(&mut tampon).map_err(|e| format!("okunamadı: {e}"))?;
        if n == 0 {
            break;
        }
        ozet.update(&tampon[..n]);
        boyut += n as u64;
    }
    let sha256: String = ozet.finish().as_ref().iter().map(|x| format!("{x:02x}")).collect();
    let ad = yol.file_name().map(|a| crate::guvenli::temiz(&a.to_string_lossy())).unwrap_or_default();
    coz(&mut f, boyut, &ad, sha256)
}
