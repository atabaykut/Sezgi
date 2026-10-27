//! Doğrulama kurallarını (class-validator dekoratörleri, zod halkaları) kontrol edilebilir biçime çevirir
//! ve bir değeri bu kurallara karşı sınar. Hiçbir şeyi çalıştırmaz, uygulamayı çağırmaz.
//! Çözülemeyen kural (ör. sabit adlı sınır, özel doğrulayıcı) "kontrol edilemedi" olarak bildirilir; tahmin edilmez.

use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Derlenmiş desen önbelleği: aynı desen süreç boyunca bir kez derlenir (Regex kopyası ucuzdur, içi paylaşımlıdır).
/// Geçersiz desen derleme hatasıdır (sabit ya da kural dosyasından gelir); kullanıcı deseni için [`derle_dene`].
pub fn derle(d: &str) -> Regex {
    derle_dene(d).unwrap_or_else(|| panic!("geçersiz desen: {d}"))
}

/// Önbellekli derleme; geçersiz desende None. Önbellek sınırlıdır (kod kaynaklı desenler sınırsız çoğalabilir).
pub fn derle_dene(d: &str) -> Option<Regex> {
    static ONBELLEK: OnceLock<Mutex<HashMap<String, Regex>>> = OnceLock::new();
    let o = ONBELLEK.get_or_init(Default::default);
    if let Some(r) = o.lock().ok()?.get(d) {
        return Some(r.clone());
    }
    let r = Regex::new(d).ok()?;
    let mut m = o.lock().ok()?;
    if m.len() >= 4096 {
        m.clear();
    }
    m.insert(d.to_string(), r.clone());
    Some(r)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kural {
    /// zod: uzunluk UTF-16 kod birimiyle (bir emoji 2)
    MinUzunluk(f64),
    MaxUzunluk(f64),
    /// class-validator (validator.js isLength): kod noktası, VS15/VS16 seçicileri sayılmaz (bir emoji 1)
    MinKarakter(f64),
    MaxKarakter(f64),
    Min(f64),
    Max(f64),
    DiziMin(f64),
    DiziMax(f64),
    BosOlmaz,
    BosMetinOlmaz,
    Pozitif,
    Negatif,
    Tamsayi,
    Eposta,
    Uuid,
    Url,
    Desen(String),
    /// ISO 8601 tarih / tarih-saat metni (IsDateString, IsISO8601, zod date() / datetime())
    IsoTarih(IsoTur),
    /// Tip kuralları (IsString, IsBoolean...) alan tipinde karşılanır
    Tip,
    /// Yalnızca anlam taşır, değeri kısıtlamaz (IsOptional, ValidateNested, Type...)
    Etkisiz,
    /// Dizi öğesinin tipi: z.array(X) -> Oge("X")
    Oge(String),
    /// null kabul edilir
    BosDeger,
    /// Kontrol edilemedi: metni olduğu gibi bildirilir
    Bilinmiyor(String),
}

/// Hangi ISO 8601 biçimi bekleniyor
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IsoTur {
    /// class-validator IsDateString / IsISO8601: tarih ya da tarih-saat, ofset isteğe bağlı
    Herhangi,
    /// zod `date()`: yalnızca YYYY-AA-GG
    YalnizTarih,
    /// zod `datetime()`: saat zorunlu; varsayılanda yalnızca "Z" (offset: true ile ±SS:DD, local: true ile bölgesiz)
    TarihSaat { ofset: bool, yerel: bool },
}

/// Ay, gün, saat aralıklarıyla ve yalnızca ASCII rakamlarla ISO 8601 denetimi
fn iso_uygun(s: &str, tur: IsoTur) -> bool {
    let re = derle(
        r"^([0-9]{4})-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])(?:[Tt ]([01][0-9]|2[0-3]):([0-5][0-9])(?::([0-5][0-9]|60)(?:\.[0-9]+)?)?([Zz]|[+-](?:[01][0-9]|2[0-3]):?[0-5][0-9])?)?$",
    );
    let Some(c) = re.captures(s) else { return false };
    let saat_var = c.get(4).is_some();
    let bolge = c.get(7).map(|m| m.as_str());
    match tur {
        IsoTur::Herhangi => true,
        IsoTur::YalnizTarih => !saat_var,
        IsoTur::TarihSaat { ofset, yerel } => {
            saat_var
                && match bolge {
                    Some("Z") | Some("z") => true,
                    Some(_) => ofset,
                    None => yerel,
                }
        }
    }
}

/// Kuralın değeri bir dizinin her öğesine mi uygulanıyor? (`{ each: true }`)
pub fn her_oge(kural: &str) -> bool {
    kural.contains("each: true")
}

/// Tip belirten class-validator kuralının tipi: IsString -> string (tip_sina'nın tip adları)
pub fn tip_kurali(kural: &str) -> Option<&'static str> {
    match kural.split('(').next().unwrap_or("") {
        "IsString" => Some("string"),
        "IsNumber" => Some("number"),
        "IsInt" => Some("integer"),
        "IsBoolean" => Some("boolean"),
        "IsDateString" | "IsISO8601" => Some("string"),
        "IsObject" => Some("object"),
        _ => None,
    }
}

fn ilk_sayi(arg: &str) -> Option<f64> {
    arg.split(',').next()?.trim().replace('_', "").parse().ok()
}

fn sayilar(arg: &str) -> Vec<Option<f64>> {
    arg.split(',').map(|a| a.trim().replace('_', "").parse().ok()).collect()
}

/// "MaxLength(50)" ya da "min(1)" -> kurallar. `tip` zod'da min/max'ın anlamını belirler (metin, sayı, dizi).
pub fn coz(kural: &str, tip: &str) -> Vec<Kural> {
    let (ad, arg) = match kural.find('(') {
        Some(i) => (&kural[..i], kural[i + 1..].trim_end().strip_suffix(')').unwrap_or(&kural[i + 1..])),
        None => (kural, ""),
    };
    let sayi = |f: fn(f64) -> Kural| ilk_sayi(arg).map(|n| vec![f(n)]).unwrap_or_else(|| vec![Kural::Bilinmiyor(kural.to_string())]);
    let t = tip.to_lowercase();
    let metin_mi = t.contains("string") || t == "str";
    let dizi_mi = t.contains("array") || t.ends_with("[]");
    match ad {
        // class-validator
        "MinLength" => sayi(Kural::MinKarakter),
        "MaxLength" => sayi(Kural::MaxKarakter),
        "Length" => match sayilar(arg).as_slice() {
            [Some(a), Some(b), ..] => vec![Kural::MinKarakter(*a), Kural::MaxKarakter(*b)],
            [Some(a)] => vec![Kural::MinKarakter(*a)],
            _ => vec![Kural::Bilinmiyor(kural.to_string())],
        },
        "Min" => sayi(Kural::Min),
        "Max" => sayi(Kural::Max),
        "ArrayMinSize" => sayi(Kural::DiziMin),
        "ArrayMaxSize" => sayi(Kural::DiziMax),
        "ArrayNotEmpty" => vec![Kural::BosOlmaz],
        // class-validator IsNotEmpty: yalnızca "", null, undefined reddedilir; boş dizi geçer
        "IsNotEmpty" => vec![Kural::BosMetinOlmaz],
        "IsPositive" => vec![Kural::Pozitif],
        "IsNegative" => vec![Kural::Negatif],
        "IsInt" => vec![Kural::Tamsayi],
        "IsEmail" => vec![Kural::Eposta],
        "IsUUID" => vec![Kural::Uuid],
        "IsUrl" => vec![Kural::Url],
        "Matches" => desen(arg).map(|d| vec![Kural::Desen(d)]).unwrap_or_else(|| vec![Kural::Bilinmiyor(kural.to_string())]),
        "IsDateString" | "IsISO8601" => vec![Kural::IsoTarih(IsoTur::Herhangi)],
        "IsString" | "IsBoolean" | "IsNumber" | "IsArray" | "IsObject" | "IsDate" | "IsEnum" | "IsIn" => vec![Kural::Tip],
        "IsOptional" | "ValidateIf" | "ValidateNested" | "IsDefined" | "Type" | "Transform" => vec![Kural::Etkisiz],
        // zod halkaları
        "min" | "max" | "length" if ilk_sayi(arg).is_none() => vec![Kural::Bilinmiyor(kural.to_string())],
        "min" if metin_mi => sayi(Kural::MinUzunluk),
        "max" if metin_mi => sayi(Kural::MaxUzunluk),
        "min" if dizi_mi => sayi(Kural::DiziMin),
        "max" if dizi_mi => sayi(Kural::DiziMax),
        "min" | "gte" => sayi(Kural::Min),
        "max" | "lte" => sayi(Kural::Max),
        "length" if metin_mi => {
            let n = ilk_sayi(arg).unwrap_or(0.0);
            vec![Kural::MinUzunluk(n), Kural::MaxUzunluk(n)]
        }
        "nonempty" => vec![Kural::BosOlmaz],
        "date" if metin_mi => vec![Kural::IsoTarih(IsoTur::YalnizTarih)],
        "datetime" if metin_mi => vec![Kural::IsoTarih(IsoTur::TarihSaat { ofset: arg.contains("offset: true"), yerel: arg.contains("local: true") })],
        "of" => vec![Kural::Oge(arg.trim().to_string())],
        "nullable" => vec![Kural::BosDeger],
        "positive" => vec![Kural::Pozitif],
        "negative" => vec![Kural::Negatif],
        "int" => vec![Kural::Tamsayi],
        "email" => vec![Kural::Eposta],
        "uuid" => vec![Kural::Uuid],
        "url" => vec![Kural::Url],
        "regex" => desen(arg).map(|d| vec![Kural::Desen(d)]).unwrap_or_else(|| vec![Kural::Bilinmiyor(kural.to_string())]),
        "string" | "number" | "boolean" | "array" | "object" | "date" | "optional" | "nullish" | "default" | "trim" | "toLowerCase" | "toUpperCase" | "describe" | "openapi" => {
            vec![Kural::Etkisiz]
        }
        _ => vec![Kural::Bilinmiyor(kural.to_string())],
    }
}

/// zod 4'te biçim tipin kendisidir: z.email(), z.uuid(), z.url()
pub fn tip_kurallari(ham_tip: &str) -> Vec<Kural> {
    match ham_tip {
        "email" => vec![Kural::Eposta],
        "uuid" | "uuidv4" | "uuidv7" | "guid" => vec![Kural::Uuid],
        "url" => vec![Kural::Url],
        _ => vec![],
    }
}

/// /^abc$/i -> "(?i)^abc$"; değişken adı (ör. pinCodeRegex) çözülemez
fn desen(arg: &str) -> Option<String> {
    let a = arg.split(", {").next()?.trim();
    let a = a.strip_prefix('/')?;
    let son = a.rfind('/')?;
    let (govde, bayrak) = (&a[..son], &a[son + 1..]);
    let bayrak = bayrak.trim_end_matches(|c: char| c == ',' || c.is_whitespace());
    // JS bayrakları: i, m, s Rust'ta karşılığı olanlar; g ve d eşleşmeyi değiştirmez; u ve v farklı anlam taşır
    // y (sticky) eşleşmeyi başa sabitler, Rust karşılığı yok: kontrol edilemedi
    if bayrak.chars().any(|c| !matches!(c, 'i' | 'm' | 's' | 'g' | 'd')) {
        return None;
    }
    let on: String = bayrak.chars().filter(|c| matches!(c, 'i' | 'm' | 's')).collect();
    let on = if on.is_empty() { String::new() } else { format!("(?{on})") };
    let d = format!("{on}{}", js_sinif_ascii(govde)?);
    // Geri başvuru ve ileri/geri bakış Rust'ta yok: derlenemezse kural "kontrol edilemedi" olur
    derle_dene(&d).map(|_| d)
}

/// JS'de \d, \w, \b ASCII'dir; Rust'ta Unicode. Anlam aynı kalsın diye ASCII karşılıklarına çevrilir.
fn js_sinif_ascii(g: &str) -> Option<String> {
    let mut s = String::new();
    let mut it = g.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            s.push(c);
            continue;
        }
        let n = it.next()?;
        match n {
            'd' => s.push_str("[0-9]"),
            'D' => s.push_str("[^0-9]"),
            'w' => s.push_str("[A-Za-z0-9_]"),
            'W' => s.push_str("[^A-Za-z0-9_]"),
            'b' => s.push_str("(?-u:\\b)"),
            'B' => s.push_str("(?-u:\\B)"),
            // \1 gibi geri başvurular ve \k<ad> Rust'ta yok
            '1'..='9' | 'k' => return None,
            _ => {
                s.push('\\');
                s.push(n);
            }
        }
    }
    Some(s)
}

/// zod (JS `string.length`): UTF-16 kod birimi, bir emoji 2
fn uzunluk(v: &Value) -> Option<f64> {
    v.as_str().map(|s| s.encode_utf16().count() as f64)
}

/// validator.js `isLength`: UTF-16 uzunluğundan vekil çiftleri ve VS15/VS16 seçicileri düşülür
fn karakter(v: &Value) -> Option<f64> {
    v.as_str().map(|s| s.chars().filter(|c| !matches!(c, '\u{FE0E}' | '\u{FE0F}')).count() as f64)
}

/// Sorgu ve yol parametreleri metin olarak gelir; sayı bekleyen alanlarda metin sayıya çevrilerek sınanır
pub fn sayi_degeri(v: &Value, donustur: bool) -> Option<f64> {
    v.as_f64().or_else(|| if donustur { v.as_str().and_then(|s| s.trim().parse().ok()) } else { None })
}

/// Bir değeri tek bir kurala karşı sınar. Ok(()) geçti, Err(neden) kaldı.
pub fn sina(k: &Kural, v: &Value, donustur: bool) -> Result<(), String> {
    let sayi = sayi_degeri(v, donustur);
    match k {
        Kural::MinUzunluk(n) => match uzunluk(v) {
            Some(u) if u < *n => Err(tf!("en az {n} karakter olmalı ({u} verildi)", "must be at least {n} {c} ({u} given)"; c = crate::dil::cogul(n, "character", "characters"))),
            _ => Ok(()),
        },
        Kural::MaxUzunluk(n) => match uzunluk(v) {
            Some(u) if u > *n => Err(tf!("en fazla {n} karakter olmalı ({u} verildi)", "must be at most {n} {c} ({u} given)"; c = crate::dil::cogul(n, "character", "characters"))),
            _ => Ok(()),
        },
        Kural::MinKarakter(n) => match karakter(v) {
            Some(u) if u < *n => Err(tf!("en az {n} karakter olmalı ({u} verildi)", "must be at least {n} {c} ({u} given)"; c = crate::dil::cogul(n, "character", "characters"))),
            _ => Ok(()),
        },
        Kural::MaxKarakter(n) => match karakter(v) {
            Some(u) if u > *n => Err(tf!("en fazla {n} karakter olmalı ({u} verildi)", "must be at most {n} {c} ({u} given)"; c = crate::dil::cogul(n, "character", "characters"))),
            _ => Ok(()),
        },
        Kural::Min(n) => match sayi {
            Some(x) if x < *n => Err(tf!("en az {n} olmalı ({x} verildi)", "must be at least {n} ({x} given)")),
            _ => Ok(()),
        },
        Kural::Max(n) => match sayi {
            Some(x) if x > *n => Err(tf!("en fazla {n} olmalı ({x} verildi)", "must be at most {n} ({x} given)")),
            _ => Ok(()),
        },
        Kural::DiziMin(n) => match v.as_array() {
            Some(a) if (a.len() as f64) < *n => {
                Err(tf!("en az {n} öğe olmalı ({} verildi)", "must have at least {n} {o} ({} given)", a.len(); o = crate::dil::cogul(n, "item", "items")))
            }
            _ => Ok(()),
        },
        Kural::DiziMax(n) => match v.as_array() {
            Some(a) if (a.len() as f64) > *n => {
                Err(tf!("en fazla {n} öğe olmalı ({} verildi)", "must have at most {n} {o} ({} given)", a.len(); o = crate::dil::cogul(n, "item", "items")))
            }
            _ => Ok(()),
        },
        Kural::BosMetinOlmaz => {
            if v.is_null() || v.as_str() == Some("") {
                Err(t!("boş olamaz", "cannot be empty").into())
            } else {
                Ok(())
            }
        }
        Kural::BosOlmaz => {
            let bos = v.is_null() || v.as_str().map(|s| s.is_empty()).unwrap_or(false) || v.as_array().map(|a| a.is_empty()).unwrap_or(false);
            if bos {
                Err(t!("boş olamaz", "cannot be empty").into())
            } else {
                Ok(())
            }
        }
        Kural::Pozitif => match sayi {
            Some(x) if x <= 0.0 => Err(tf!("pozitif olmalı ({x} verildi)", "must be positive ({x} given)")),
            _ => Ok(()),
        },
        Kural::Negatif => match sayi {
            Some(x) if x >= 0.0 => Err(tf!("negatif olmalı ({x} verildi)", "must be negative ({x} given)")),
            _ => Ok(()),
        },
        Kural::Tamsayi => match sayi {
            Some(x) if x.fract() != 0.0 => Err(tf!("tam sayı olmalı ({x} verildi)", "must be an integer ({x} given)")),
            None if !v.is_null() => Err(t!("tam sayı olmalı", "must be an integer").into()),
            _ => Ok(()),
        },
        Kural::Eposta => match v.as_str() {
            Some(s) if !derle(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").is_match(s) => Err(tf!("geçerli bir e-posta değil ({s})", "not a valid e-mail address ({s})")),
            _ => Ok(()),
        },
        Kural::Uuid => match v.as_str() {
            Some(s) if !derle(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").is_match(s) => Err(tf!("UUID değil ({s})", "not a UUID ({s})")),
            _ => Ok(()),
        },
        Kural::Url => match v.as_str() {
            Some(s) if !derle(r"^(?i)(https?://)?[\w.-]+\.[a-z]{2,}(:\d+)?(/\S*)?$").is_match(s) => {
                Err(tf!("geçerli bir adres (URL) değil ({s})", "not a valid address (URL) ({s})"))
            }
            _ => Ok(()),
        },
        Kural::Desen(d) => match v.as_str() {
            Some(s) if !derle_dene(d).map(|r| r.is_match(s)).unwrap_or(true) => Err(tf!("biçime uymuyor: /{d}/", "does not match the pattern: /{d}/")),
            _ => Ok(()),
        },
        Kural::IsoTarih(tur) => match v.as_str() {
            Some(s) if !iso_uygun(s, *tur) => Err(if *tur == IsoTur::YalnizTarih {
                tf!("ISO 8601 tarih (YYYY-AA-GG) değil ({s})", "not an ISO 8601 date (YYYY-MM-DD) ({s})")
            } else {
                tf!("ISO 8601 tarih-saat değil ({s})", "not an ISO 8601 date-time ({s})")
            }),
            _ => Ok(()),
        },
        Kural::Tip | Kural::Etkisiz | Kural::Oge(_) | Kural::BosDeger | Kural::Bilinmiyor(_) => Ok(()),
    }
}

/// Alan tipine (alan_tipi çıktısı) göre değerin türünü sınar
pub fn tip_sina(tip: &str, v: &Value, donustur: bool) -> Result<(), String> {
    let ok = match tip {
        "string" | "datetime" | "date" => v.is_string(),
        "integer" => sayi_degeri(v, donustur).map(|x| x.fract() == 0.0).unwrap_or(false),
        "number" => sayi_degeri(v, donustur).is_some(),
        "boolean" => v.is_boolean() || (donustur && matches!(v.as_str(), Some("true") | Some("false"))),
        "array" => v.is_array() || (donustur && v.is_string()),
        "object" => v.is_object(),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(tf!("{tip} olmalı ({} verildi)", "must be {tip} ({} given)", tur_adi(v)))
    }
}

pub fn tur_adi(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
