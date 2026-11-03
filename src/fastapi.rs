//! FastAPI adaptörü: Python modelinden (`python.rs`) NestJS adaptörüyle aynı kapı yapısını (`Kapi`) üretir; kart, MCP,
//! önizleme, OpenAPI ve canlı mod dilden bağımsız çalışır.
//!
//! Kesinlik ilkesi NestJS'tekiyle aynıdır: anlam adlardan değil tanımlardan çıkar.
//! - **Rotalar:** `@<router>.get("/yol")` ve benzerleri; tam yol, uygulamadan (FastAPI()) router'a kadar `include_router`
//!   zinciri izlenerek kurulur (FastAPI'nin sırasıyla: include öneki + router öneki + ... + rota yolu).
//! - **Giriş:** bağımlılıklar (`Depends` / `Security`, `Annotated[..., Depends(f)]`, router ve uygulama düzeyindeki
//!   `dependencies=[...]`) tanımlarından izlenir. Zincir bir güvenlik şemasına (OAuth2PasswordBearer, HTTPBearer,
//!   APIKeyHeader...) ulaşırsa giriş gerekir (`auto_error=False` ise isteğe bağlı) ve karar kesindir. Hiç güvenlik şeması
//!   bulunamazsa uç açık sayılır ama bu çıkarımdır; izlenemeyen bir bağımlılık varsa ayrıca "tanınmayan guard" olarak
//!   bildirilir. Uygulama düzeyi ara katman (`@app.middleware("http")`, `app.add_middleware(...)`) bir kimlik bilgisine
//!   bakıp 401 / 403 döndürüyorsa kapsadığı uçlar giriş ister (çıkarım); kapsamı çözülemeyen uçta giriş "bilinmiyor".
//! - **Girdiler:** Pydantic / SQLModel sınıfı tipli parametre gövdedir; yol şablonundaki ad yol parametresidir;
//!   kalan basit tipli parametreler sorgudur. Pydantic alanları her zaman doğrular: varsayılanı olmayan alan zorunludur
//!   (kesin); `Field(...)` sınırları kurallara çevrilir.
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::adaptor::{Alan, Girdi, Kapi, Sema};
use crate::guvenli;
use crate::python::{Cagri, Dosya, Fonksiyon, Sinif};

pub const SURUM: &str = "0.1.0";

const YONTEMLER: [&str; 7] = ["get", "post", "put", "patch", "delete", "options", "head"];
/// fastapi.security sınıfları: istek bu şemadan geçmeden işleyiciye ulaşmaz (auto_error=False değilse)
const GUVENLIK_SEMALARI: [&str; 11] = [
    "OAuth2PasswordBearer",
    "OAuth2AuthorizationCodeBearer",
    "OAuth2",
    "HTTPBearer",
    "HTTPBasic",
    "HTTPDigest",
    "APIKeyHeader",
    "APIKeyQuery",
    "APIKeyCookie",
    "OpenIdConnect",
    "HTTPBase",
];
/// Çatının kendisinin enjekte ettiği tipler: girdi değildir
const ENJEKTE: [&str; 7] = ["Request", "Response", "BackgroundTasks", "WebSocket", "HTTPConnection", "SecurityScopes", "UploadFile"];
const PYDANTIC_TABAN: [&str; 3] = ["BaseModel", "SQLModel", "RootModel"];

/// Bir adın çözümlendiği yer
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Yer {
    Modul(usize),
    /// (dosya, ad): o modüldeki üst düzey ad
    Ad(usize, String),
    /// Projede bulunamayan, dışarıdan içe aktarılmış ad: (modül, ad), ör. ("fastapi", "Depends")
    Dis(String, String),
}

/// Bağımlılık zincirinin giriş sonucu
#[derive(Clone, Copy, Default, Debug)]
struct Guvenlik {
    /// Zincir hata veren (`auto_error` açık) bir güvenlik şemasına ulaştı: giriş gerekli, kesin
    gerekli: bool,
    /// Zincir `auto_error=False` bir şemaya ulaştı ve kimse hata fırlatmıyor görünüyor: isteğe bağlı (tahmin)
    istege: bool,
    /// İzlenemeyen bağımlılık
    bilinmeyen: bool,
    /// `auto_error=False` şemayı kullanan fonksiyon 401 / 403 fırlatıyor: büyük olasılıkla giriş gerekli (tahmin)
    tahmini_gerekli: bool,
}

impl Guvenlik {
    fn sema(auto_error_kapali: bool) -> Self {
        Guvenlik { gerekli: !auto_error_kapali, istege: auto_error_kapali, ..Default::default() }
    }
    fn bilinmeyen() -> Self {
        Guvenlik { bilinmeyen: true, ..Default::default() }
    }
    fn birlestir(self, o: Guvenlik) -> Self {
        Guvenlik {
            gerekli: self.gerekli || o.gerekli,
            istege: self.istege || o.istege,
            bilinmeyen: self.bilinmeyen || o.bilinmeyen,
            tahmini_gerekli: self.tahmini_gerekli || o.tahmini_gerekli,
        }
    }
}

pub(crate) struct Baglam<'a> {
    pub(crate) dosyalar: &'a [Dosya],
    moduller: HashMap<&'a str, usize>,
}

/// Çağrı metninin içindeki ilk argüman: "Depends(get_db)" -> "get_db"
pub(crate) fn ic(ifade: &str, ad: &str) -> Option<String> {
    let i = ifade.find(&format!("{ad}("))?;
    let bas = i + ad.len() + 1;
    let mut derinlik = 0;
    for (j, c) in ifade[bas..].char_indices() {
        match c {
            '(' | '[' => derinlik += 1,
            ')' | ']' if derinlik > 0 => derinlik -= 1,
            ')' | ',' if derinlik == 0 => return Some(ifade[bas..bas + j].trim().to_string()),
            _ => {}
        }
    }
    None
}

/// "Annotated[X, Depends(f), ...]" -> ("X", ["Depends(f)", ...])
pub(crate) fn annotated(tip: &str) -> Option<(String, Vec<String>)> {
    let g = tip.trim().strip_prefix("Annotated[")?.strip_suffix(']')?;
    let parcalar = ust_duzey_bol(g);
    let (ilk, kalan) = parcalar.split_first()?;
    Some((ilk.clone(), kalan.to_vec()))
}

/// Virgülle ayrılmış üst düzey parçalar (parantez ve köşeli parantez içi bölünmez)
pub(crate) fn ust_duzey_bol(s: &str) -> Vec<String> {
    let (mut v, mut d, mut bas) = (vec![], 0i32, 0);
    let mut tirnak: Option<char> = None;
    for (i, c) in s.char_indices() {
        match (tirnak, c) {
            (Some(t), x) if x == t => tirnak = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => tirnak = Some(c),
            (None, '(' | '[' | '{') => d += 1,
            (None, ')' | ']' | '}') => d -= 1,
            (None, ',') if d == 0 => {
                v.push(s[bas..i].trim().to_string());
                bas = i + 1;
            }
            _ => {}
        }
    }
    if !s[bas..].trim().is_empty() {
        v.push(s[bas..].trim().to_string());
    }
    v
}

/// Üst düzey `+` ile ayrılmış parçalar (tırnak ve parantez içi bölünmez)
fn ust_duzey_arti(s: &str) -> Vec<&str> {
    let (mut v, mut d, mut bas) = (vec![], 0i32, 0);
    let mut tirnak: Option<char> = None;
    for (i, c) in s.char_indices() {
        match (tirnak, c) {
            (Some(t), x) if x == t => tirnak = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => tirnak = Some(c),
            (None, '(' | '[' | '{') => d += 1,
            (None, ')' | ']' | '}') => d -= 1,
            (None, '+') if d == 0 => {
                v.push(s[bas..i].trim());
                bas = i + 1;
            }
            _ => {}
        }
    }
    v.push(s[bas..].trim());
    v
}

/// Metin sabiti: "'/items'" -> "/items" (f-string'de yer tutucu varsa çözülemez)
pub(crate) fn metin_sabiti(s: &str) -> Option<String> {
    let s = s.trim();
    let govde = s.trim_start_matches(['r', 'R', 'b', 'B', 'u', 'U', 'f', 'F']);
    let bicimli = s.len() != govde.len() && s[..s.len() - govde.len()].to_lowercase().contains('f');
    for q in ["\"\"\"", "'''", "\"", "'"] {
        if let Some(ic) = govde.strip_prefix(q).and_then(|x| x.strip_suffix(q)) {
            if bicimli && ic.contains('{') {
                return None;
            }
            return Some(ic.to_string());
        }
    }
    None
}

/// "list[Depends(a), Depends(b)]" ya da "[Depends(a)]" -> bağımlılık hedefleri
fn bagimlilik_listesi(s: &str) -> Vec<String> {
    let g = s.trim().trim_start_matches('[').trim_end_matches(']');
    ust_duzey_bol(g).iter().filter_map(|x| ic(x, "Depends").or_else(|| ic(x, "Security"))).collect()
}

pub(crate) fn snake_camel(s: &str) -> String {
    let mut out = String::new();
    let mut buyuk = false;
    for c in s.chars() {
        if c == '_' {
            buyuk = !out.is_empty();
        } else if buyuk {
            out.extend(c.to_uppercase());
            buyuk = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn pascal(s: &str) -> String {
    let c = snake_camel(s);
    let mut k = c.chars();
    k.next().map(|f| f.to_uppercase().collect::<String>() + k.as_str()).unwrap_or_default()
}

/// FastAPI'nin varsayılan özeti: read_items -> "Read Items"
pub(crate) fn varsayilan_ozet(ad: &str) -> String {
    ad.split('_').filter(|p| !p.is_empty()).map(pascal).collect::<Vec<_>>().join(" ")
}

impl<'a> Baglam<'a> {
    pub(crate) fn yeni(dosyalar: &'a [Dosya]) -> Self {
        Baglam { dosyalar, moduller: dosyalar.iter().enumerate().map(|(i, d)| (d.modul.as_str(), i)).collect() }
    }

    /// Göreli içe aktarmayı mutlak modül adına çevirir: ".deps" (app.api.routes.items içinde) -> "app.api.routes.deps"
    pub(crate) fn mutlak(&self, di: usize, modul: &str) -> String {
        let nokta = modul.chars().take_while(|c| *c == '.').count();
        if nokta == 0 {
            return modul.to_string();
        }
        let d = &self.dosyalar[di];
        let paket_mi = d.yol.file_stem().is_some_and(|s| s == "__init__");
        let mut parcalar: Vec<&str> = d.modul.split('.').collect();
        // Modülün kendisi bir paket değilse ilk nokta bulunduğu paketi gösterir
        let cik = if paket_mi { nokta - 1 } else { nokta };
        for _ in 0..cik {
            parcalar.pop();
        }
        let kalan = &modul[nokta..];
        if !kalan.is_empty() {
            parcalar.push(kalan);
        }
        parcalar.join(".")
    }

    /// Modül adının (göreli olabilir: `.base`) projedeki dosyası
    pub(crate) fn modul_bul(&self, di: usize, modul: &str) -> Option<usize> {
        self.moduller.get(self.mutlak(di, modul).as_str()).copied()
    }

    /// Bir modüldeki adı çözer (tanımlıysa orası, içe aktarılmışsa kaynağı), 6 adıma kadar
    pub(crate) fn coz(&self, di: usize, ad: &str, derinlik: usize) -> Option<Yer> {
        if derinlik > 6 {
            return None;
        }
        let d = &self.dosyalar[di];
        // İç fonksiyonlar (fabrikanın içindekiler) modül düzeyinde ad değildir
        if d.atamalar.contains_key(ad) || d.fonksiyonlar.iter().any(|f| f.ad == ad && f.kapsam.is_none()) || d.siniflar.iter().any(|s| s.ad == ad) {
            return Some(Yer::Ad(di, ad.to_string()));
        }
        let Some((modul, oge)) = d.iceaktarmalar.get(ad) else {
            // Yıldızlı içe aktarma (`from .views import *`): ad o modüllerden birinde tanımlı olabilir
            return d.yildizlar.iter().filter_map(|y| self.moduller.get(self.mutlak(di, y).as_str()).copied()).find_map(|m| self.coz(m, ad, derinlik + 1));
        };
        let modul = self.mutlak(di, modul);
        match oge {
            None => self.moduller.get(modul.as_str()).map(|&m| Yer::Modul(m)).or(Some(Yer::Dis(modul, ad.to_string()))),
            Some(oge) => {
                // `from paket import modul` bir alt modül de olabilir
                if let Some(&m) = self.moduller.get(format!("{modul}.{oge}").as_str()) {
                    return Some(Yer::Modul(m));
                }
                match self.moduller.get(modul.as_str()) {
                    Some(&m) => self.coz(m, oge, derinlik + 1).or(Some(Yer::Dis(modul, oge.clone()))),
                    None => Some(Yer::Dis(modul, oge.clone())),
                }
            }
        }
    }

    /// Noktalı ifadeyi çözer: "items.router" (modül özniteliği) ya da düz ad
    pub(crate) fn ifade(&self, di: usize, ifade: &str) -> Option<Yer> {
        let ifade = ifade.trim();
        let mut parcalar = ifade.split('.');
        let ilk = parcalar.next()?;
        let mut yer = self.coz(di, ilk, 0)?;
        for p in parcalar {
            yer = match yer {
                // Paket alt modülü kendisi içe aktarmasa da `paket.alt_modul` erişilebilir
                Yer::Modul(m) => match self.coz(m, p, 0) {
                    Some(y) => y,
                    None => Yer::Modul(*self.moduller.get(format!("{}.{p}", self.dosyalar[m].modul).as_str())?),
                },
                Yer::Dis(m, a) => Yer::Dis(m, format!("{a}.{p}")),
                Yer::Ad(..) => return None,
            };
        }
        Some(yer)
    }

    pub(crate) fn atama(&self, y: &Yer) -> Option<(&'a str, Option<&'a Cagri>)> {
        let Yer::Ad(di, ad) = y else { return None };
        let d = &self.dosyalar[*di];
        Some((d.atamalar.get(ad)?.as_str(), d.atama_cagrilari.get(ad)))
    }

    pub(crate) fn fonksiyon(&self, y: &Yer) -> Option<(usize, &'a Fonksiyon)> {
        let Yer::Ad(di, ad) = y else { return None };
        self.dosyalar[*di].fonksiyonlar.iter().find(|f| &f.ad == ad && f.kapsam.is_none()).map(|f| (*di, f))
    }

    pub(crate) fn sinif(&self, y: &Yer) -> Option<(usize, &'a Sinif)> {
        let Yer::Ad(di, ad) = y else { return None };
        self.dosyalar[*di].siniflar.iter().find(|s| &s.ad == ad).map(|s| (*di, s))
    }

    /// Metin değeri: sabit, sabite atıf, `settings.X` (Settings sınıfındaki varsayılan), yer tutucuları çözülebilen f-string
    pub(crate) fn deger(&self, di: usize, ifade: &str, derinlik: usize) -> Option<String> {
        if let Some(s) = metin_sabiti(ifade) {
            return Some(s);
        }
        if derinlik > 4 {
            return None;
        }
        let ifade = ifade.trim();
        // f"{settings.API_V1_STR}/items": her {ifade} çözülmeli, biri çözülemezse değer bilinmez
        if let Some(govde) = ifade
            .strip_prefix('f')
            .or_else(|| ifade.strip_prefix('F'))
            .and_then(|g| g.strip_prefix('"').and_then(|x| x.strip_suffix('"')).or_else(|| g.strip_prefix('\'').and_then(|x| x.strip_suffix('\''))))
        {
            let mut out = String::new();
            let mut kalan = govde;
            while let Some(i) = kalan.find('{') {
                out.push_str(&kalan[..i]);
                let j = kalan[i..].find('}')? + i;
                out.push_str(&self.deger(di, &kalan[i + 1..j], derinlik + 1)?);
                kalan = &kalan[j + 1..];
            }
            out.push_str(kalan);
            return Some(out);
        }
        // Metin toplama: `task_instances_prefix + "/{task_id}"` (her parça çözülmeli)
        let parcalar = ust_duzey_arti(ifade);
        if parcalar.len() > 1 {
            return parcalar.iter().map(|p| self.deger(di, p, derinlik + 1)).collect::<Option<Vec<_>>>().map(|v| v.concat());
        }
        if let Some((nesne, oznitelik)) = ifade.rsplit_once('.') {
            // settings.API_V1_STR: settings = Settings(); class Settings: API_V1_STR: str = "/api/v1"
            let yer = self.ifade(di, nesne)?;
            let (_, cagri) = self.atama(&yer)?;
            let Yer::Ad(adi, _) = &yer else { return None };
            let sinif_yeri = self.coz(*adi, cagri?.ad.as_str(), 0)?;
            let (si, s) = self.sinif(&sinif_yeri)?;
            let alan = s.alanlar.iter().find(|a| a.ad == oznitelik)?;
            return self.deger(si, alan.varsayilan.as_deref()?, derinlik + 1);
        }
        let yer = self.coz(di, ifade, 0)?;
        let (atama, _) = self.atama(&yer)?;
        let Yer::Ad(adi, _) = yer else { return None };
        self.deger(adi, atama, derinlik + 1)
    }

    /// Sınıf Pydantic modeli mi (projedeki taban zinciri BaseModel / SQLModel'e ulaşıyor mu)
    pub(crate) fn pydantic_mi(&self, di: usize, s: &Sinif, derinlik: usize) -> bool {
        if derinlik > 8 {
            return false;
        }
        s.tabanlar.iter().any(|t| {
            let t = t.split('[').next().unwrap_or(t);
            match self.ifade(di, t) {
                Some(Yer::Dis(_, a)) => PYDANTIC_TABAN.contains(&a.rsplit('.').next().unwrap_or(&a)),
                Some(y) => self.sinif(&y).is_some_and(|(d2, s2)| self.pydantic_mi(d2, s2, derinlik + 1)),
                None => PYDANTIC_TABAN.contains(&t),
            }
        })
    }

    /// Modelin alanları (taban sınıflardakiler önce, alt sınıf geçersiz kılar)
    pub(crate) fn alanlar(&self, di: usize, s: &Sinif, derinlik: usize, out: &mut BTreeMap<String, Alan>) {
        if derinlik > 8 {
            return;
        }
        for t in &s.tabanlar {
            if let Some((d2, s2)) = self.ifade(di, t.split('[').next().unwrap_or(t)).and_then(|y| self.sinif(&y)) {
                self.alanlar(d2, s2, derinlik + 1, out);
            }
        }
        for a in &s.alanlar {
            // Tipsiz sınıf özniteliği Pydantic alanı değildir
            let Some(tip) = a.tip.clone() else { continue };
            if a.ad.starts_with('_') || a.ad == "model_config" || tip.starts_with("ClassVar") {
                continue;
            }
            // Field(alias="x"): API'deki ad takma addır
            let ad = a
                .varsayilan
                .as_deref()
                .and_then(|v| v.trim().strip_prefix("Field(").and_then(|x| x.strip_suffix(')')).map(ust_duzey_bol))
                .and_then(|p| p.iter().find_map(|x| x.split_once('=').filter(|(k, _)| matches!(k.trim(), "alias" | "validation_alias")).and_then(|(_, v)| metin_sabiti(v))))
                .unwrap_or_else(|| a.ad.clone());
            out.insert(ad, self.alan(di, &tip, a.varsayilan.as_deref(), a.satir));
        }
    }

    /// Sınıf hiyerarşisini dışarıdan gelen bir tabana kadar izler: projedeki `class Router(APIRouter)` ya da
    /// `class JWTBearer(HTTPBearer)` için dışarıdaki asıl adı ("APIRouter", "HTTPBearer") verir
    pub(crate) fn taban_dis_ad(&self, di: usize, ad: &str, derinlik: usize) -> Option<String> {
        if derinlik > 8 {
            return None;
        }
        match self.ifade(di, ad.split('(').next().unwrap_or(ad))? {
            Yer::Dis(_, a) => Some(a.rsplit('.').next().unwrap_or(&a).to_string()),
            y => {
                let (d2, s) = self.sinif(&y)?;
                s.tabanlar.iter().find_map(|t| self.taban_dis_ad(d2, t.split('[').next().unwrap_or(t), derinlik + 1))
            }
        }
    }

    /// Python tipinden (ve varsayılandan) Sezgi alanı: tip, zorunluluk, kurallar (class-validator adlarıyla, böylece
    /// kart, önizleme ve OpenAPI aynı kural çözücüyü kullanır), izin verilen değerler
    pub(crate) fn alan(&self, di: usize, tip: &str, varsayilan: Option<&str>, satir: usize) -> Alan {
        let mut kurallar = vec![];
        let (mut tip_s, nullable) = tip_coz(tip);
        let mut degerler: Vec<String> = vec![];
        let tam_ad = tip_s.split('[').next().unwrap_or(&tip_s).trim().to_string();
        let temel = tam_ad.rsplit('.').next().unwrap_or("").to_string();
        let ad_tip = match temel.as_str() {
            "str" | "constr" => {
                kurallar.push("IsString()".to_string());
                "string"
            }
            "EmailStr" | "NameEmail" => {
                kurallar.push("IsEmail()".to_string());
                "string"
            }
            "UUID" | "UUID4" | "UUID1" => {
                kurallar.push("IsUUID()".to_string());
                "string"
            }
            "HttpUrl" | "AnyUrl" | "AnyHttpUrl" => {
                kurallar.push("IsUrl()".to_string());
                "string"
            }
            "int" | "conint" | "PositiveInt" | "NonNegativeInt" => {
                kurallar.push("IsInt()".to_string());
                if temel == "PositiveInt" {
                    kurallar.push("Min(1)".to_string());
                }
                "int"
            }
            "float" | "Decimal" | "confloat" => {
                kurallar.push("IsNumber()".to_string());
                "number"
            }
            "bool" => {
                kurallar.push("IsBoolean()".to_string());
                "boolean"
            }
            "datetime" | "date" => {
                kurallar.push("IsDateString()".to_string());
                "date"
            }
            "list" | "List" | "set" | "Set" | "tuple" | "Sequence" => {
                kurallar.push("IsArray()".to_string());
                "array"
            }
            "dict" | "Dict" | "Json" => {
                kurallar.push("IsObject()".to_string());
                "object"
            }
            "Literal" => {
                degerler = ust_duzey_bol(tip_s.trim_start_matches("Literal[").trim_end_matches(']')).iter().filter_map(|v| metin_sabiti(v)).collect();
                kurallar.push(format!("IsIn([{}])", degerler.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
                "enum"
            }
            _ => match self.ifade(di, &tam_ad).and_then(|y| self.sinif(&y).map(|(d2, s2)| (d2, s2.clone()))) {
                // Projedeki Enum: metin değerleri izinli değerlerdir
                Some((d2, s)) if s.tabanlar.iter().any(|t| t.contains("Enum")) => {
                    degerler = s.alanlar.iter().filter_map(|a| a.varsayilan.as_deref().and_then(metin_sabiti)).collect();
                    let _ = d2;
                    // Değerleri sınıf gövdesindeki atamalardan da okunur (tipsiz: ROMAN = "roman")
                    kurallar.push(format!("IsEnum({temel})"));
                    "enum"
                }
                Some((d2, s)) if self.pydantic_mi(d2, &s, 0) => {
                    kurallar.push("ValidateNested()".to_string());
                    "object"
                }
                _ => {
                    tip_s = String::new();
                    "unknown"
                }
            },
        };
        if nullable {
            kurallar.push("nullable()".to_string());
        }
        // Zorunluluk: varsayılan yoksa ya da `...` ise zorunlu; Field(...) içinde default / default_factory yoksa da
        let mut zorunlu = varsayilan.is_none();
        if let Some(v) = varsayilan.map(str::trim) {
            if v == "..." {
                zorunlu = true;
            } else if let Some(arglar) = v.strip_prefix("Field(").and_then(|x| x.strip_suffix(')')) {
                let parcalar = ust_duzey_bol(arglar);
                let konumsal = parcalar.iter().find(|p| !p.contains('='));
                let anahtar = |a: &str| parcalar.iter().find_map(|p| p.split_once('=').filter(|(k, _)| k.trim() == a).map(|(_, v)| v.trim().to_string()));
                // `...` (Ellipsis) varsayılan değil "zorunlu" demektir: Field(...) ve Field(default=...)
                zorunlu = match konumsal {
                    Some(k) => k.trim() == "...",
                    None => anahtar("default_factory").is_none() && anahtar("default").is_none_or(|d| d == "..."),
                };
                for (k, kural) in [("min_length", "MinLength"), ("max_length", "MaxLength"), ("ge", "Min"), ("le", "Max")] {
                    if let Some(n) = anahtar(k).filter(|n| n.parse::<f64>().is_ok()) {
                        kurallar.push(format!("{kural}({n})"));
                    }
                }
                for (k, kural) in [("gt", "gt"), ("lt", "lt"), ("pattern", "Matches"), ("regex", "Matches")] {
                    if let Some(n) = anahtar(k) {
                        kurallar.push(format!("{kural}({n})"));
                    }
                }
            }
        }
        Alan { tip: (!tip_s.is_empty()).then(|| ad_tip.to_string()), zorunlu, belirsiz: false, kurallar, satir, kalitimdan: None, degerler, ic: None }
    }

    /// Bağımlılık hedefinin güvenlik sonucu: (gerekli şema, isteğe bağlı şema, izlenemeyen bağımlılık)
    fn guvenlik(&self, di: usize, hedef: &str, ziyaret: &mut HashSet<String>, derinlik: usize) -> Guvenlik {
        let anahtar = format!("{di}:{hedef}");
        if derinlik > 8 || !ziyaret.insert(anahtar) {
            return Guvenlik::default();
        }
        // Sınıf örneği bağımlılık: Depends(JWTBearer()) / Depends(HTTPBearer(auto_error=False))
        if let Some((sinif_adi, arglar)) = hedef.trim().strip_suffix(')').and_then(|h| h.split_once('(')) {
            let istege = ust_duzey_bol(arglar).iter().any(|a| a.replace(' ', "") == "auto_error=False");
            if self.taban_dis_ad(di, sinif_adi, 0).is_some_and(|d| GUVENLIK_SEMALARI.contains(&d.as_str())) {
                return Guvenlik::sema(istege);
            }
            // Projedeki fabrika (`rol_iste("admin")` iç fonksiyon döndürür) ya da __call__'lu sınıfın örneği
            return match self.cagrilan(di, sinif_adi) {
                Some((fdi, f)) => self.fonksiyon_guvenligi(fdi, f, ziyaret, derinlik),
                None => Guvenlik::bilinmeyen(),
            };
        }
        let Some(yer) = self.ifade(di, hedef) else { return Guvenlik::bilinmeyen() };
        // Şema örneği: reusable_oauth2 = OAuth2PasswordBearer(tokenUrl=...) ya da şema alt sınıfının örneği
        if let Some((_, Some(c))) = self.atama(&yer) {
            let Yer::Ad(adi, _) = &yer else { unreachable!() };
            if let Some(dis) = self.taban_dis_ad(*adi, &c.ad, 0) {
                if GUVENLIK_SEMALARI.contains(&dis.as_str()) {
                    return Guvenlik::sema(c.anahtarli.iter().any(|(k, v)| k == "auto_error" && v.trim() == "False"));
                }
            }
            return Guvenlik::bilinmeyen();
        }
        if let Some((fdi, f)) = self.fonksiyon(&yer) {
            return self.fonksiyon_guvenligi(fdi, f, ziyaret, derinlik);
        }
        if let Yer::Dis(_, a) = &yer {
            let a = a.rsplit('.').next().unwrap_or(a);
            if GUVENLIK_SEMALARI.contains(&a) {
                return Guvenlik::sema(false);
            }
        }
        Guvenlik::bilinmeyen()
    }

    /// Bağımlılık fonksiyonunun parametrelerindeki zincir. Zincir `auto_error=False` bir şemaya ulaştıysa şema hata
    /// vermez; belirteç yoksa hatayı bu fonksiyon (ya da çağırdığı bir proje fonksiyonu) verebilir: 401 / 403
    /// fırlatıyorsa giriş büyük olasılıkla gereklidir. İkisi de tanımdan kanıtlanamaz: tahmin.
    fn fonksiyon_guvenligi(&self, fdi: usize, f: &Fonksiyon, ziyaret: &mut HashSet<String>, derinlik: usize) -> Guvenlik {
        let mut sonuc = Guvenlik::default();
        for p in &f.parametreler {
            for (hdi, h) in self.param_bagimliliklari(fdi, p.tip.as_deref(), p.varsayilan.as_deref()) {
                sonuc = sonuc.birlestir(self.guvenlik(hdi, &h, ziyaret, derinlik + 1));
            }
        }
        if sonuc.istege && !sonuc.gerekli && self.yetki_hatasi(fdi, f, 2) {
            sonuc.istege = false;
            sonuc.tahmini_gerekli = true;
        }
        // Güvenlik şeması olmadan başlıktan (ya da çerezden, sorgudan) anahtar / belirteç okuyup 401 / 403 fırlatan
        // bağımlılık: `def require_api_key(x_api_key: str = Header(...))`. Giriş büyük olasılıkla gerekli (tahmin)
        if !sonuc.gerekli && self.anahtar_okuyan(f) && self.yetki_hatasi(fdi, f, 2) {
            sonuc.istege = false;
            sonuc.tahmini_gerekli = true;
        }
        sonuc
    }

    /// Fonksiyonun bir parametresi başlık / çerez / sorgu olarak kimlik bilgisi (anahtar, belirteç) alıyor mu
    fn anahtar_okuyan(&self, f: &Fonksiyon) -> bool {
        f.parametreler.iter().any(|p| {
            let ad = p.ad.to_ascii_lowercase();
            let kimlik = ["key", "token", "authorization", "secret", "signature"].iter().any(|k| ad.contains(k));
            kimlik && isaret(p.tip.as_deref(), p.varsayilan.as_deref()).is_some_and(|(i, _)| matches!(i.as_str(), "Header" | "Cookie" | "Query"))
        })
    }

    /// Bağımlılık hedefi, başlıktan anahtar / belirteç okuyup 401 / 403 fırlatan bir proje fonksiyonu mu (guard adı etiketlenir)
    fn anahtar_bagimliligi(&self, di: usize, hedef: &str) -> bool {
        self.ifade(di, hedef).and_then(|y| self.fonksiyon(&y)).is_some_and(|(fdi, f)| self.anahtar_okuyan(f) && self.yetki_hatasi(fdi, f, 2))
    }

    /// Fonksiyon (ya da gövdesinde çağırdığı proje fonksiyonları, `derinlik` kat) 401 / 403 fırlatıyor mu
    fn yetki_hatasi(&self, fdi: usize, f: &Fonksiyon, derinlik: usize) -> bool {
        const ISARETLER: [&str; 8] = ["401", "403", "UNAUTHORIZED", "FORBIDDEN", "Unauthorized", "Forbidden", "NotAuthenticated", "PermissionDenied"];
        if ISARETLER.iter().any(|x| f.govde_metni.contains(x)) {
            return true;
        }
        if derinlik == 0 {
            return false;
        }
        let cagri = crate::kural::derle(r"\b([A-Za-z_][\w.]*)\(");
        let adlar: Vec<String> = cagri.captures_iter(&f.govde_metni).map(|c| c[1].to_string()).take(64).collect();
        adlar.iter().filter_map(|a| self.ifade(fdi, a).and_then(|y| self.fonksiyon(&y))).filter(|(_, g)| g.ad != f.ad).any(|(gdi, g)| self.yetki_hatasi(gdi, g, derinlik - 1))
    }

    /// `Depends(X(...))` çağrısında FastAPI'nin çağıracağı fonksiyon: X projede bir fabrikaysa döndürdüğü iç fonksiyon
    /// (`def rol_iste(r): def denetle(u = Depends(...)): ...; return denetle`), bir sınıfsa `__call__` yöntemi
    fn cagrilan(&self, di: usize, ad: &str) -> Option<(usize, &'a Fonksiyon)> {
        let yer = self.ifade(di, ad)?;
        if let Some((fdi, f)) = self.fonksiyon(&yer) {
            // `return denetle` ya da `return Depends(get_limit)` (parametre varsayılanı olarak kullanılan fabrika)
            let donus = f.donus.as_deref()?.trim();
            let donus = ic(donus, "Depends").filter(|x| !x.is_empty()).unwrap_or_else(|| donus.to_string());
            let donus = donus.as_str();
            return self.dosyalar[fdi].fonksiyonlar.iter().find(|g| g.kapsam.as_deref() == Some(f.ad.as_str()) && g.ad == donus).map(|g| (fdi, g));
        }
        let (sdi, s) = self.sinif(&yer)?;
        s.cagri_yontemi.as_deref().map(|c| (sdi, c))
    }

    /// Rol fabrikasının argümanları [Yorum]: çağrılan fonksiyon 403 / Forbidden döndürüyorsa metin argümanları
    /// (liste içindekiler dahil) rol sayılır: `Depends(rol_iste("admin"))`, `Depends(RolDenetimi(["admin", "editor"]))`
    fn roller(&self, di: usize, hedef: &str) -> Vec<String> {
        let Some((ad, arglar)) = hedef.trim().strip_suffix(')').and_then(|h| h.split_once('(')) else { return self.bayrak_rolleri(di, hedef) };
        let Some((_, f)) = self.cagrilan(di, ad) else { return vec![] };
        if !["403", "FORBIDDEN", "Forbidden", "PermissionDenied"].iter().any(|x| f.govde_metni.contains(x)) {
            return vec![];
        }
        let mut v = vec![];
        for a in ust_duzey_bol(arglar) {
            let a = a.split_once('=').filter(|(k, _)| !k.contains(['"', '\''])).map(|(_, d)| d.trim().to_string()).unwrap_or(a);
            let ic = a.trim().trim_start_matches(['[', '(', '{']).trim_end_matches([']', ')', '}']);
            v.extend(ust_duzey_bol(ic).iter().filter_map(|x| metin_sabiti(x)));
        }
        v
    }

    /// Argümansız bağımlılığın denetlediği yetki bayrağı [Yorum]: fonksiyon 403 / Forbidden döndürüyor ve kullanıcının
    /// `is_superuser` / `is_admin` / `is_staff` bayrağını denetliyorsa bayrağın adı rol sayılır
    /// (`get_current_active_superuser` deseni). Bayrak bulunamazsa rol uydurulmaz.
    fn bayrak_rolleri(&self, di: usize, hedef: &str) -> Vec<String> {
        static BAYRAK: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let bayrak = BAYRAK.get_or_init(|| regex::Regex::new(r"\bnot\s+[\w.]+\.is_(superuser|superadmin|admin|staff)\b").unwrap());
        let Some((_, f)) = self.ifade(di, hedef.trim()).and_then(|y| self.fonksiyon(&y)) else { return vec![] };
        if !["403", "FORBIDDEN", "Forbidden", "PermissionDenied"].iter().any(|x| f.govde_metni.contains(x)) {
            return vec![];
        }
        let mut v: Vec<String> = bayrak.captures_iter(&f.govde_metni).map(|c| c[1].to_string()).collect();
        v.dedup();
        v
    }

    /// `X(...)` çağrısı projede `return Depends(iç)` yapan bir fabrika mı
    fn depends_fabrikasi(&self, di: usize, ifade: &str) -> bool {
        let Some((ad, _)) = ifade.trim().strip_suffix(')').and_then(|h| h.split_once('(')) else { return false };
        self.ifade(di, ad).and_then(|y| self.fonksiyon(&y)).is_some_and(|(_, f)| f.donus.as_deref().is_some_and(|d| d.trim_start().starts_with("Depends(")))
            && self.cagrilan(di, ad).is_some()
    }

    /// Parametrenin bağımlılık hedefleri: varsayılan Depends(f) / Security(f), Annotated[..., Depends(f)] ya da bu
    /// biçimde bir tip takma adı (CurrentUser = Annotated[User, Depends(get_current_user)]). Her hedef, adının
    /// çözüleceği modülle döner: takma adın içindeki ad, takma adın tanımlandığı modülde çözülür.
    fn param_bagimliliklari(&self, di: usize, tip: Option<&str>, varsayilan: Option<&str>) -> Vec<(usize, String)> {
        let mut v = vec![];
        let hedef = |e: &str, ilk: &str| ic(e, "Depends").or_else(|| ic(e, "Security")).map(|h| if h.is_empty() { ilk.to_string() } else { h });
        if let Some(h) = varsayilan.and_then(|d| hedef(d, tip.unwrap_or(""))) {
            v.push((di, h));
        } else if let Some(d) = varsayilan.filter(|d| self.depends_fabrikasi(di, d)) {
            // `limit: int = LimitBody()`: varsayılan, Depends(iç fonksiyon) döndüren fabrika
            v.push((di, d.trim().to_string()));
        }
        if let Some(t) = tip {
            if let Some((ilk, ekler)) = annotated(t) {
                v.extend(ekler.iter().filter_map(|e| hedef(e, &ilk)).map(|h| (di, h)));
            } else if let Some(y @ Yer::Ad(adi, _)) = self.coz(di, t.trim(), 0) {
                if let Some((ilk, ekler)) = self.atama(&y).and_then(|(a, _)| annotated(a)) {
                    v.extend(ekler.iter().filter_map(|e| hedef(e, &ilk)).map(|h| (adi, h)));
                }
            }
        }
        v
    }
}

const ISARETLER: [&str; 7] = ["Query", "Path", "Header", "Cookie", "Body", "Form", "File"];

/// Varsayılan değerdeki parametre işareti: "Query(None, alias='q')" -> ("Query", ["None", "alias='q'"])
fn isaret_varsayilanda(v: &str) -> Option<(String, Vec<String>)> {
    let v = v.trim();
    let (ad, kalan) = v.split_once('(')?;
    let ad = ad.rsplit('.').next().unwrap_or(ad).trim();
    ISARETLER.contains(&ad).then(|| (ad.to_string(), ust_duzey_bol(kalan.strip_suffix(')').unwrap_or(kalan))))
}

/// Parametrenin işareti: önce Annotated[T, Query(...)] içindeki, sonra varsayılan değerdeki
fn isaret(tip: Option<&str>, varsayilan: Option<&str>) -> Option<(String, Vec<String>)> {
    tip.and_then(annotated).and_then(|(_, ekler)| ekler.iter().find_map(|e| isaret_varsayilanda(e))).or_else(|| varsayilan.and_then(isaret_varsayilanda))
}

/// Annotated[T, ...] -> T
fn tip_ham_annotated(tip: &str) -> String {
    annotated(tip).map(|(t, _)| t).unwrap_or_else(|| tip.to_string())
}

/// "str | None" / "Optional[str]" / "Annotated[str, Field(...)]" -> ("str", nullable)
pub(crate) fn tip_coz(tip: &str) -> (String, bool) {
    let mut t = tip.trim().to_string();
    if let Some((ilk, _)) = annotated(&t) {
        t = ilk;
    }
    let mut nullable = false;
    if let Some(ic) = t.strip_prefix("Optional[").and_then(|x| x.strip_suffix(']')) {
        return (ic.trim().to_string(), true);
    }
    let parcalar: Vec<String> = t.split('|').map(|p| p.trim().to_string()).collect();
    if parcalar.len() > 1 {
        nullable = parcalar.iter().any(|p| p == "None");
        let kalan: Vec<&String> = parcalar.iter().filter(|p| *p != "None").collect();
        if kalan.len() == 1 {
            return (kalan[0].clone(), nullable);
        }
        return (String::new(), nullable);
    }
    (t, nullable)
}

/// OAuth2PasswordRequestForm: FastAPI'nin kendi form bağımlılığı (alanları belgelidir)
/// Yanıt modeli tipi: `list[X]` / `List[X]` / `Sequence[X]` dizi, `Optional[X]` / `X | None` X
pub(crate) fn yanit_tipi(tip: &str) -> (String, bool) {
    let (t, _) = tip_coz(tip);
    for onek in ["list[", "List[", "Sequence[", "typing.List[", "typing.Sequence["] {
        if let Some(ic) = t.strip_prefix(onek).and_then(|x| x.strip_suffix(']')) {
            return (tip_coz(ic).0, true);
        }
    }
    (t, false)
}

/// Dekoratörün başarı durum kodu: `status_code=201` ya da `status.HTTP_201_CREATED`
fn durum_kodu(anahtarli: &[(String, String)]) -> Option<u16> {
    let v = anahtarli.iter().find(|(k, _)| k == "status_code")?.1.trim();
    let rakam: String = v.rsplit("HTTP_").next().unwrap_or(v).chars().take_while(char::is_ascii_digit).collect();
    rakam.parse().ok().filter(|d| (100..600).contains(d))
}

impl Baglam<'_> {
    /// Yanıt şeması: `response_model=` açık beyanı, yoksa dönüş tipi bildirimi (FastAPI ikisinden de yanıtı süzer ve
    /// belgeler). Tip bir Pydantic/SQLModel modeli değilse (Any, dict, Response ...) şema yok: bilinmiyor
    fn yanit(&self, kok: &Path, dosyalar: &[Dosya], di: usize, anahtarli: &[(String, String)], f: &Fonksiyon) -> Option<crate::adaptor::Yanit> {
        let (ham, kaynak) = match anahtarli.iter().find(|(k, _)| k == "response_model") {
            Some((_, v)) if v.trim() != "None" => (v.clone(), "response_model"),
            Some(_) => return None,
            None => (f.donus_tipi.clone()?, "donus_tipi"),
        };
        let (tip, dizi) = yanit_tipi(&ham);
        let tam_ad = tip.split('[').next().unwrap_or(&tip).trim().to_string();
        let (d2, s) = self.ifade(di, &tam_ad).and_then(|y| self.sinif(&y)).filter(|(d2, s)| self.pydantic_mi(*d2, s, 0))?;
        let mut alanlar = BTreeMap::new();
        self.alanlar(d2, s, 0, &mut alanlar);
        Some(crate::adaptor::Yanit {
            kaynak: kaynak.into(),
            kesin: true,
            durum: durum_kodu(anahtarli),
            sema: Sema {
                adaylar: vec![],
                dizi,
                dto: s.ad.clone(),
                dosya: Some(guvenli::goreli(kok, &dosyalar[d2].yol)),
                bulunamadi: false,
                kalitim: s.tabanlar.first().cloned(),
                alanlar,
            },
        })
    }
}

fn oauth2_formu(satir: usize) -> BTreeMap<String, Alan> {
    let alan =
        |zorunlu: bool| Alan { tip: Some("string".into()), zorunlu, belirsiz: false, kurallar: vec!["IsString()".into()], satir, kalitimdan: None, degerler: vec![], ic: None };
    BTreeMap::from([
        ("username".to_string(), alan(true)),
        ("password".to_string(), alan(true)),
        ("scope".to_string(), alan(false)),
        ("grant_type".to_string(), alan(false)),
        ("client_id".to_string(), alan(false)),
        ("client_secret".to_string(), alan(false)),
    ])
}

/// (dosya, ad): bir uygulama ya da router örneği
type Dugum = (usize, String);
/// Router içerme kenarı (bağımlılık grafiği): (içeren, içerilen, çağrının modülü, satırı, koşul metni, koşullu)
type Icerme = (Dugum, Dugum, usize, usize, Option<String>, bool);
/// include_router kenarı: (hedef router, önek, bağımlılıklar, koşullu)
type Kenar = (Dugum, String, Vec<String>, bool);
/// Router -> (tam önek, bağımlılıklar, koşullu)
type Erisim = HashMap<Dugum, (String, Vec<Dugum>, bool)>;
/// Rota kaydı: (kaydın modülü, alıcı, yöntemler, kayıt çağrısı, fonksiyonun modülü, fonksiyon, sınıf bağımlılıkları).
/// Sınıf tabanlı görünümde (`@controller(router)`, fastapi-utils `cbv`) sınıfın `Depends(...)` öznitelikleri her uca
/// parametre gibi eklenir: (özniteliğin modülü, parametre)
type RotaKaydi<'a> = (usize, String, Vec<String>, Cagri, usize, &'a Fonksiyon, Vec<(usize, crate::python::Parametre)>);

struct Router {
    di: usize,
    onek: String,
    bagimliliklar: Vec<String>,
    /// `APIRouter(tags=["items"])`: işlem kimliği (operationId) etiketle üretiliyorsa ilk etiket
    etiketler: Vec<String>,
}

/// `tags=["a", "b"]` gibi bir anahtarlı argümanın metin öğeleri
fn etiket_listesi(c: &Cagri) -> Vec<String> {
    c.anahtarli
        .iter()
        .find(|(k, _)| k == "tags")
        .map(|(_, v)| ust_duzey_bol(v.trim().trim_start_matches(['[', '(']).trim_end_matches([']', ')'])).iter().filter_map(|x| metin_sabiti(x)).collect())
        .unwrap_or_default()
}

/// Uygulamanın işlem kimliği (operationId) üretimi: FastAPI'nin varsayılanı ya da `generate_unique_id_function` ile
/// verilen `f"{route.tags[0]}<ayraç>{route.name}"` deseni. Başka bir fonksiyon koddan anlamlandırılamaz.
#[derive(Clone, PartialEq)]
enum KimlikUretimi {
    /// `{ad}{yol}` (\W -> _) + `_{yöntem}`
    Varsayilan,
    /// `{ilk etiket}{ayraç}{ad}`
    Etiket(String),
}

/// ORM model sınıfları: SQLModel `class X(..., table=True)` ya da SQLAlchemy `__tablename__` taşıyan sınıflar
/// Veritabanı tablosu tanımlayan sınıf: SQLModel `table=True` ya da SQLAlchemy `__tablename__`
pub(crate) fn tablo_modeli_mi(s: &crate::python::Sinif) -> bool {
    s.anahtarli.iter().any(|(k, v)| k == "table" && v == "True") || s.alanlar.iter().any(|a| a.ad == "__tablename__")
}

fn veri_modelleri(dosyalar: &[Dosya]) -> std::collections::HashSet<&str> {
    dosyalar.iter().flat_map(|d| &d.siniflar).filter(|s| tablo_modeli_mi(s)).map(|s| s.ad.as_str()).collect()
}

/// Metinde geçen ilk model adı (tip bildirimi ya da takma ad metni: `User | None`, `Annotated[User, Depends(...)]`)
fn metindeki_model(metin: &str, modeller: &std::collections::HashSet<&str>) -> Option<String> {
    metin.split(|c: char| !(c.is_alphanumeric() || c == '_')).find(|p| modeller.contains(p)).map(String::from)
}

/// Oturuma eklenen ya da silinen değişkenin modeli, tanımdan: parametrenin tip bildirimi (takma ad dahil:
/// `CurrentUser = Annotated[User, ...]`), değişkeni üreten çağrı (`session.get(M, ...)`, `M(...)`) ya da çağrılan
/// proje fonksiyonunun dönüş tipi (`-> User | None`)
fn degisken_modeli(b: &Baglam, modeller: &std::collections::HashSet<&str>, di: usize, f: &Fonksiyon, ad: &str) -> Option<String> {
    if let Some(p) = f.parametreler.iter().find(|p| p.ad == ad) {
        let tip = p.tip.as_deref()?;
        if let Some(m) = metindeki_model(tip, modeller) {
            return Some(m);
        }
        // Takma ad: modül düzeyindeki atamanın metni
        let takma = tip.trim();
        return b.dosyalar.iter().find_map(|d| d.atamalar.get(takma)).and_then(|t| metindeki_model(t, modeller));
    }
    let c = f.ic_atama_cagrilari.get(ad)?;
    let kurucu = c.ad.trim_end_matches(".model_validate");
    if modeller.contains(kurucu) {
        return Some(kurucu.to_string());
    }
    if c.ad.ends_with(".get") {
        if let Some(m) = c.argumanlar.first().and_then(|a| metindeki_model(a, modeller)) {
            return Some(m);
        }
    }
    let (_, g) = b.ifade(di, &c.ad).and_then(|y| b.fonksiyon(&y))?;
    metindeki_model(g.donus_tipi.as_deref()?, modeller)
}

/// Uç fonksiyonundan başlayıp proje içi çağrılara (3 kat) inerek ORM kullanımını toplar [Yorum: metinden çıkarım].
/// Okuma: select(M), .select_from(M), .get(M, ...), .query(M). Yazma: update/insert/delete(M); oturuma eklenen
/// (`.add`, `.add_all`, `.merge`) ya da silinen (`.delete`) fonksiyonda oluşturulan veya alınan modeller.
/// Ham SQL (`text(...)`, metinle `execute`) görülürse liste eksik olabilir.
pub(crate) fn veri_bagi(b: &Baglam, modeller: &std::collections::HashSet<&str>, fdi: usize, f: &Fonksiyon) -> Option<crate::adaptor::VeriBag> {
    static DESEN: std::sync::OnceLock<[regex::Regex; 7]> = std::sync::OnceLock::new();
    let [okuma, cekirdek, olusturma, alma, oturum_hedefi, ham_sql, cagri_adi] = DESEN.get_or_init(|| {
        [
            regex::Regex::new(r"(?:\bselect|\.select_from|\.query)\(\s*([A-Za-z_]\w*)").unwrap(),
            regex::Regex::new(r"\b(?:update|insert|delete)\(\s*([A-Za-z_]\w*)\s*\)").unwrap(),
            regex::Regex::new(r"\b([A-Z]\w*)(?:\.model_validate)?\(").unwrap(),
            regex::Regex::new(r"\.get\(\s*([A-Za-z_]\w*)\s*,").unwrap(),
            regex::Regex::new(r"\.(?:add|delete|merge)\(\s*([A-Za-z_]\w*)\s*\)").unwrap(),
            regex::Regex::new(r#"\btext\(\s*[fbru]*["']|\.execute\(\s*[fbru]*["']"#).unwrap(),
            regex::Regex::new(r"(?:^|[^\w.])([A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)\(").unwrap(),
        ]
    });
    if modeller.is_empty() {
        return None;
    }
    let mut bag = crate::adaptor::VeriBag::default();
    let mut gorulen = std::collections::HashSet::new();
    let mut yigin = vec![(fdi, f, 0usize)];
    while let Some((di, f, derinlik)) = yigin.pop() {
        if !gorulen.insert((di, f.ad.as_str(), f.satir)) {
            continue;
        }
        let t = &f.govde_metni;
        let model = |m: &str| modeller.contains(m).then(|| m.to_string());
        let alinan: Vec<String> = alma.captures_iter(t).filter_map(|c| model(&c[1])).collect();
        bag.okur.extend(okuma.captures_iter(t).filter_map(|c| model(&c[1])));
        bag.okur.extend(alinan.iter().cloned());
        let cekirdek_yazma: Vec<String> = cekirdek.captures_iter(t).filter_map(|c| model(&c[1])).collect();
        bag.yazar.extend(cekirdek_yazma);
        if [".add(", ".add_all(", ".merge(", ".delete("].iter().any(|x| t.contains(x)) {
            bag.yazar.extend(olusturma.captures_iter(t).filter_map(|c| model(&c[1])));
            bag.yazar.extend(alinan);
            // Eklenen ya da silinen değişkenin modeli: parametre tipi, takma ad ya da üreten çağrının dönüş tipi
            bag.yazar.extend(oturum_hedefi.captures_iter(t).filter_map(|c| degisken_modeli(b, modeller, di, f, &c[1])));
        }
        if ham_sql.is_match(t) {
            bag.eksik_olabilir = true;
        }
        if derinlik < 3 {
            // Gövdedeki bütün çağrılar, konumundan bağımsız (`return crud.bul(...)`, koşul içinde, argüman içinde)
            let adlar: std::collections::BTreeSet<&str> = cagri_adi.captures_iter(t).filter_map(|c| c.get(1)).map(|m| m.as_str()).collect();
            for ad in adlar {
                if let Some((gdi, g)) = b.ifade(di, ad).and_then(|y| b.fonksiyon(&y)) {
                    yigin.push((gdi, g, derinlik + 1));
                }
            }
        }
    }
    bag.okur.sort();
    bag.okur.dedup();
    bag.yazar.sort();
    bag.yazar.dedup();
    Some(bag)
}

/// İşlemin davranış zinciri: uç fonksiyonu, bağımlılıkları (`Depends`, kendi bağımlılıklarıyla), router ve uygulama
/// düzeyindeki bağımlılıklar (`APIRouter(dependencies=[...])`, `include_router(dependencies=[...])`, dekoratörün
/// `dependencies=`; `ek`: (modül, ifade)) ve gövdede adıyla çağrılan proje fonksiyonları (2 kat). Dönen: davranış özeti
pub(crate) fn dna_zinciri(b: &Baglam, kok: &Path, fdi: usize, f: &Fonksiyon, ek: &[(usize, String)]) -> Value {
    let mut halkalar = vec![];
    let mut gorulen = HashSet::new();
    let mut yigin: Vec<(usize, &Fonksiyon, usize, &'static str)> = vec![(fdi, f, 0, "isleyici")];
    while let Some((di, g, derinlik, tur)) = yigin.pop() {
        if !gorulen.insert((di, g.ad.as_str(), g.satir)) || halkalar.len() >= 32 {
            continue;
        }
        halkalar.push(crate::dna::Halka { dosya: guvenli::goreli(kok, &b.dosyalar[di].yol), dna: &g.dna, derinlik, tur });
        if derinlik >= 3 {
            continue;
        }
        let mut sonraki = vec![];
        let mut bagimliliklar: Vec<(usize, String)> = g.parametreler.iter().flat_map(|p| b.param_bagimliliklari(di, p.tip.as_deref(), p.varsayilan.as_deref())).collect();
        if derinlik == 0 {
            bagimliliklar.extend(ek.iter().cloned());
        }
        {
            for (hdi, h) in bagimliliklar {
                let bulunan = match h.split_once('(') {
                    Some((ad, _)) => b.cagrilan(hdi, ad.trim()),
                    None => b.ifade(hdi, h.trim()).and_then(|y| b.fonksiyon(&y)),
                };
                if let Some((gdi, g2)) = bulunan {
                    sonraki.push((gdi, g2, derinlik + 1, "bagimlilik"));
                }
            }
        }
        if derinlik < 2 {
            for ad in &g.dna.cagrilar {
                if let Some((gdi, g2)) = b.ifade(di, ad).and_then(|y| b.fonksiyon(&y)) {
                    sonraki.push((gdi, g2, derinlik + 1, if tur == "bagimlilik" { "bagimlilik" } else { "servis" }));
                }
            }
        }
        // Yığın sondan alır: önce kaynak sırasıyla gezilsin
        yigin.extend(sonraki.into_iter().rev());
    }
    crate::dna::ozet(&halkalar)
}

/// Sınıf tabanlı görünümün bağımlılık öznitelikleri (`user: User = Depends(get_current_user)`): önce projedeki taban
/// sınıflar, alt sınıf aynı adı geçersiz kılar. `_` ile başlayan ve `ClassVar` öznitelikler enjekte edilmez
fn sinif_bagimliliklari(b: &Baglam, di: usize, s: &Sinif, derinlik: usize, out: &mut BTreeMap<String, (usize, crate::python::Parametre)>) {
    if derinlik > 8 {
        return;
    }
    for t in &s.tabanlar {
        if let Some((d2, s2)) = b.ifade(di, t.split('[').next().unwrap_or(t)).and_then(|y| b.sinif(&y)) {
            sinif_bagimliliklari(b, d2, s2, derinlik + 1, out);
        }
    }
    for a in &s.alanlar {
        if a.ad.starts_with('_') || a.tip.as_deref().is_some_and(|t| t.starts_with("ClassVar")) {
            continue;
        }
        let p = crate::python::Parametre { ad: a.ad.clone(), tip: a.tip.clone(), varsayilan: a.varsayilan.clone() };
        if !b.param_bagimliliklari(di, p.tip.as_deref(), p.varsayilan.as_deref()).is_empty() {
            out.insert(a.ad.clone(), (di, p));
        }
    }
}

/// Uyarı metninde kod parçası: tek satır, en çok 60 karakter
fn kisalt(s: &str) -> String {
    let t: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > 60 {
        format!("{}…", t.chars().take(60).collect::<String>())
    } else {
        t
    }
}

/// Router ve uygulama örneklerinin tanım satırı: modül düzeyi atama ya da fabrika fonksiyonundaki yerel atama
fn ornek_satiri(dosyalar: &[Dosya], d: &Dugum) -> usize {
    let dosya = &dosyalar[d.0];
    match d.1.split_once("().") {
        Some((f, x)) => dosya.fonksiyonlar.iter().find(|g| g.ad == f).and_then(|g| g.ic_atama_cagrilari.get(x)).map(|c| c.satir).unwrap_or(0),
        None => dosya.atama_cagrilari.get(&d.1).map(|c| c.satir).unwrap_or(0),
    }
}

/// Router içerme grafiği (bağımlılık grafiği için yan bilgi): her router ve uygulama örneği, onu içeren örnek, içerme
/// satırı ve koşulu (`if settings.X:` altındaki `include_router`)
fn router_grafi(kok: &Path, dosyalar: &[Dosya], icerme: &[Icerme], routerlar: &HashMap<Dugum, Router>, uygulamalar: &[(usize, String, Cagri)]) -> Value {
    let kimlik = |d: &Dugum| format!("{}:{}", guvenli::goreli(kok, &dosyalar[d.0].yol), d.1);
    let mut v: Vec<Value> = vec![];
    let mut ornekler: Vec<Dugum> = routerlar.keys().cloned().chain(uygulamalar.iter().map(|(u, a, _)| (*u, a.clone()))).collect();
    ornekler.sort();
    for o in ornekler {
        let ustler: Vec<Value> = icerme
            .iter()
            .filter(|(_, alt, ..)| *alt == o)
            .map(|(ust, _, di, satir, kosul, kosullu)| json!({ "id": kimlik(ust), "icerme": format!("{}:{satir}", guvenli::goreli(kok, &dosyalar[*di].yol)), "kosul": kosul, "kosullu": kosullu }))
            .collect();
        v.push(json!({ "id": kimlik(&o), "dosya": guvenli::goreli(kok, &dosyalar[o.0].yol), "satir": ornek_satiri(dosyalar, &o), "ustler": ustler }));
    }
    json!(v)
}

/// Tablo modelleri (SQLModel `table=True`, SQLAlchemy `__tablename__`): ad, gerçek tablo adı, sütunları (ad, tip, satır)
pub(crate) fn tablo_modelleri(kok: &Path, dosyalar: &[Dosya]) -> Value {
    let mut v = vec![];
    for d in dosyalar {
        for s in d.siniflar.iter().filter(|s| tablo_modeli_mi(s)) {
            let acik = s.alanlar.iter().find(|a| a.ad == "__tablename__").and_then(|a| a.varsayilan.as_deref()).map(|x| x.trim().trim_matches(['"', '\'']).to_string());
            let alanlar: Vec<Value> = s
                .alanlar
                .iter()
                .filter(|a| !a.ad.starts_with("__") && a.tip.as_deref().is_some_and(|t| !t.contains("ClassVar")))
                // İlişki alanı (Relationship) sütun değildir
                .filter(|a| !a.varsayilan.as_deref().is_some_and(|x| x.trim_start().starts_with("Relationship(") || x.trim_start().starts_with("relationship(")))
                .map(|a| json!({ "ad": a.ad, "tip": a.tip, "satir": a.satir }))
                .collect();
            let satir = s.alanlar.iter().map(|a| a.satir).filter(|s| *s > 0).min().map(|s| s.saturating_sub(1).max(1)).unwrap_or(1);
            v.push(json!({
                "ad": s.ad, "tablo": acik.clone().unwrap_or_else(|| s.ad.to_lowercase()),
                "tablo_kaynagi": if acik.is_some() { "__tablename__" } else { "varsayılan (sınıf adının küçük harfi)" },
                "dosya": guvenli::goreli(kok, &d.yol), "satir": satir, "alanlar": alanlar,
            }));
        }
    }
    json!(v)
}

/// Tablo modellerindeki kaskad silmeler: [ebeveyn, çocuk, "dosya:satır"]
pub(crate) fn kaskadlar(kok: &Path, dosyalar: &[Dosya]) -> Vec<Value> {
    let mut tablolar = BTreeMap::new();
    for s in dosyalar.iter().flat_map(|d| &d.siniflar).filter(|s| tablo_modeli_mi(s)) {
        let ad = s.alanlar.iter().find(|a| a.ad == "__tablename__").and_then(|a| a.varsayilan.as_deref()).map(|v| v.trim().trim_matches(['"', '\'']).to_lowercase());
        tablolar.insert(ad.unwrap_or_else(|| s.ad.to_lowercase()), s.ad.clone());
    }
    let mut v = vec![];
    for d in dosyalar {
        for s in d.siniflar.iter().filter(|s| tablo_modeli_mi(s)) {
            for a in &s.alanlar {
                if let Some((e, c)) = crate::dna::python_kaskadi(&s.ad, a.tip.as_deref(), a.varsayilan.as_deref().unwrap_or(""), &tablolar) {
                    v.push(json!([e, c, format!("{}:{}", guvenli::goreli(kok, &d.yol), a.satir)]));
                }
            }
        }
    }
    v
}

pub fn kapilar(kok: &Path, dosyalar: &[Dosya]) -> (Vec<Kapi>, Value) {
    let b = Baglam::yeni(dosyalar);
    let modeller = veri_modelleri(dosyalar);
    // Uygulama ve router örnekleri: modül düzeyinde ve fabrika fonksiyonlarının içinde (`def create_app(): app = FastAPI()`).
    // Fonksiyon içindeki örneğin adı "fonksiyon().değişken" biçimindedir.
    let mut uygulamalar = vec![];
    let mut routerlar: HashMap<(usize, String), Router> = HashMap::new();
    for (di, d) in dosyalar.iter().enumerate() {
        let mut ornekler: Vec<(String, &Cagri)> = d.atama_cagrilari.iter().map(|(a, c)| (a.clone(), c)).collect();
        for f in &d.fonksiyonlar {
            ornekler.extend(f.ic_atama_cagrilari.iter().map(|(a, c)| (format!("{}().{a}", f.ad), c)));
        }
        for (ad, c) in ornekler {
            // Projedeki alt sınıflar (class Router(APIRouter)) da sayılır
            match b.taban_dis_ad(di, &c.ad, 0).as_deref() {
                Some("FastAPI") => uygulamalar.push((di, ad, c.clone())),
                Some("APIRouter") => {
                    let onek = c.anahtarli.iter().find(|(k, _)| k == "prefix").and_then(|(_, v)| b.deger(di, v, 0)).unwrap_or_default();
                    let bag = c.anahtarli.iter().find(|(k, _)| k == "dependencies").map(|(_, v)| bagimlilik_listesi(v)).unwrap_or_default();
                    routerlar.insert((di, ad), Router { di, onek, bagimliliklar: bag, etiketler: etiket_listesi(c) });
                }
                _ => {}
            }
        }
    }
    // Bir çağrının hedef(ler)i: döngü değişkeniyse gezilen listenin her öğesi (`for r in API_ROUTERS: app.include_router(r)`)
    let hedefler = |di: usize, arg: &str, dongu: &Option<(String, String)>| -> Vec<Yer> {
        let arg = arg.split('(').next().unwrap_or(arg).trim();
        if let Some((_, ifade)) = dongu.as_ref().filter(|(v, _)| v == arg) {
            let (ldi, metin) = match b.coz(di, ifade.trim(), 0) {
                Some(y @ Yer::Ad(ldi, _)) => (ldi, b.atama(&y).map(|(a, _)| a.to_string()).unwrap_or_default()),
                _ => (di, ifade.to_string()),
            };
            let ic = metin.trim().trim_start_matches(['[', '(']).trim_end_matches([']', ')']);
            return ust_duzey_bol(ic).iter().filter_map(|e| b.ifade(ldi, e)).collect();
        }
        b.ifade(di, arg).into_iter().collect()
    };
    // Alıcı: fonksiyonun yerel örneği ("f().app") ya da modül düzeyindeki ad
    let alici_bul = |di: usize, f: Option<&Fonksiyon>, alici: &str| -> Option<Dugum> {
        if let Some(f) = f.filter(|f| f.ic_atama_cagrilari.contains_key(alici)) {
            return Some((di, format!("{}().{alici}", f.ad)));
        }
        match b.ifade(di, alici)? {
            Yer::Ad(adi, aad) => Some((adi, aad)),
            _ => None,
        }
    };
    // include_router ve mount kenarları: (alıcı) -> (router ya da alt uygulama, önek, bağımlılıklar, koşullu)
    let mut kenarlar: HashMap<Dugum, Vec<Kenar>> = HashMap::new();
    // İçerme kenarları (bağımlılık grafiği için): (içeren, içerilen, çağrının modülü, satırı, koşul metni, koşullu)
    let mut icerme: Vec<Icerme> = vec![];
    // include_router(tags=[...]) etiketleri: (alıcı, hedef) -> etiketler; FastAPI bunları rotanın etiketlerinin başına ekler
    let mut kenar_etiketleri: HashMap<(Dugum, Dugum), Vec<String>> = HashMap::new();
    let bilinen = |d: &Dugum| routerlar.contains_key(d) || uygulamalar.iter().any(|(u, a, _)| *u == d.0 && *a == d.1);
    // Bağlama çağrıları: (çağrının modülü, çağrının fonksiyonu, çağrı, alıcı düğümü verilmişse o). Uygulamayı ya da router'ı
    // argüman alan yardımcı fonksiyonun (`def gorunumleri_kur(app): app.include_router(r)`, çağrı `gorunumleri_kur(app)`)
    // içindeki bağlamalar argümana bağlanır (bir kat)
    let mut baglamalar: Vec<(usize, Option<&Fonksiyon>, &Cagri, Option<Dugum>)> = vec![];
    for (di, d) in dosyalar.iter().enumerate() {
        let mut cagrilar: Vec<(Option<&Fonksiyon>, &Cagri)> = d.cagrilar.iter().map(|c| (None, c)).collect();
        for f in &d.fonksiyonlar {
            cagrilar.extend(f.ic_cagrilar.iter().map(|c| (Some(f), c)));
        }
        for (f, c) in cagrilar {
            if c.ad.ends_with(".include_router") || c.ad.ends_with(".mount") {
                baglamalar.push((di, f, c, None));
                continue;
            }
            let Some((gdi, g)) = b.ifade(di, &c.ad).and_then(|y| b.fonksiyon(&y)) else { continue };
            for (i, arg) in c.argumanlar.iter().enumerate() {
                let Some(dugum) = alici_bul(di, f, arg.trim()).filter(|d| bilinen(d)) else { continue };
                let Some(p) = g.parametreler.get(i) else { continue };
                for gc in g.ic_cagrilar.iter().filter(|gc| gc.ad == format!("{}.include_router", p.ad) || gc.ad == format!("{}.mount", p.ad)) {
                    baglamalar.push((gdi, Some(g), gc, Some(dugum.clone())));
                }
            }
        }
    }
    {
        for (di, f, c, verilen) in baglamalar {
            if let Some(alici) = c.ad.strip_suffix(".include_router") {
                let (Some(dugum), Some(hedef)) = (verilen.clone().or_else(|| alici_bul(di, f, alici)), c.argumanlar.first()) else { continue };
                let onek = c.anahtarli.iter().find(|(k, _)| k == "prefix").and_then(|(_, v)| b.deger(di, v, 0)).unwrap_or_default();
                let bag = c.anahtarli.iter().find(|(k, _)| k == "dependencies").map(|(_, v)| bagimlilik_listesi(v)).unwrap_or_default();
                for y in hedefler(di, hedef, &c.dongu) {
                    if let Yer::Ad(hdi, had) = y {
                        // include_router(kur()): fabrikanın döndürdüğü yerel router ("kur().router")
                        let had = match b.fonksiyon(&Yer::Ad(hdi, had.clone())) {
                            Some((_, g)) if hedef.trim_end().ends_with(')') => match g.donus.as_deref().map(str::trim).filter(|d| g.ic_atama_cagrilari.contains_key(*d)) {
                                Some(d) => format!("{}().{d}", g.ad),
                                None => had,
                            },
                            _ => had,
                        };
                        icerme.push((dugum.clone(), (hdi, had.clone()), di, c.satir, c.kosul.clone(), c.kosullu));
                        let etiketler = etiket_listesi(c);
                        if !etiketler.is_empty() {
                            kenar_etiketleri.insert((dugum.clone(), (hdi, had.clone())), etiketler);
                        }
                        kenarlar.entry(dugum.clone()).or_default().push(((hdi, had), onek.clone(), bag.clone(), c.kosullu));
                    }
                }
            } else if let Some(alici) = c.ad.strip_suffix(".mount") {
                // Alt uygulama: app.mount("/v1", alt_uygulama)
                let (Some(dugum), Some(yol), Some(hedef)) = (verilen.or_else(|| alici_bul(di, f, alici)), c.argumanlar.first(), c.argumanlar.get(1)) else { continue };
                let Some(Yer::Ad(hdi, had)) = b.ifade(di, hedef) else { continue };
                if b.atama(&Yer::Ad(hdi, had.clone())).and_then(|(_, c)| c).is_some_and(|c| b.taban_dis_ad(hdi, &c.ad, 0).as_deref() == Some("FastAPI")) {
                    icerme.push((dugum.clone(), (hdi, had.clone()), di, c.satir, c.kosul.clone(), c.kosullu));
                    kenarlar.entry(dugum).or_default().push(((hdi, had), b.deger(di, yol, 0).unwrap_or_default(), vec![], c.kosullu));
                }
            }
        }
    }
    // Ana uygulama: en çok rotaya ulaşan FastAPI örneği. Router -> (tam önek, bağımlılıklar, koşullu) haritası
    let ulas = |kok_: Dugum, app_bag: Vec<String>| -> Erisim {
        let mut sonuc = HashMap::new();
        let mut kuyruk = vec![(kok_.clone(), String::new(), app_bag.iter().map(|x| (kok_.0, x.clone())).collect::<Vec<_>>(), false)];
        sonuc.insert(kok_.clone(), (String::new(), kuyruk[0].2.clone(), false));
        let mut adim = 0;
        while let Some((dugum, onek, bag, kosullu)) = kuyruk.pop() {
            adim += 1;
            if adim > 1000 {
                break;
            }
            for (hedef, iek, ibag, ikos) in kenarlar.get(&dugum).into_iter().flatten() {
                if sonuc.contains_key(hedef) {
                    continue;
                }
                let r = routerlar.get(hedef);
                let tam = format!("{onek}{iek}{}", r.map(|r| r.onek.as_str()).unwrap_or(""));
                let mut b2 = bag.clone();
                b2.extend(ibag.iter().map(|x| (dugum.0, x.clone())));
                if let Some(r) = r {
                    b2.extend(r.bagimliliklar.iter().map(|x| (r.di, x.clone())));
                }
                sonuc.insert(hedef.clone(), (tam.clone(), b2.clone(), kosullu || *ikos));
                kuyruk.push((hedef.clone(), tam, b2, kosullu || *ikos));
            }
        }
        sonuc
    };
    let mut en_iyi: Option<(Dugum, Erisim)> = None;
    for (di, ad, c) in &uygulamalar {
        let bag = c.anahtarli.iter().find(|(k, _)| k == "dependencies").map(|(_, v)| bagimlilik_listesi(v)).unwrap_or_default();
        let h = ulas((*di, ad.clone()), bag);
        if en_iyi.as_ref().is_none_or(|(_, e)| h.len() > e.len()) {
            en_iyi = Some(((*di, ad.clone()), h));
        }
    }
    let (ana_kok, erisim) = en_iyi.unwrap_or(((usize::MAX, String::new()), HashMap::new()));
    // Router'a ana uygulamadan ulaşırken eklenen include etiketleri, dıştan içe (rotanın `tags[0]`'ı bunlardan ilki)
    let mut include_etiketleri: HashMap<Dugum, Vec<String>> = HashMap::new();
    {
        let mut kuyruk = vec![(ana_kok.clone(), Vec::<String>::new())];
        let mut gorulen: HashSet<Dugum> = HashSet::new();
        while let Some((dugum, etiket)) = kuyruk.pop() {
            if !gorulen.insert(dugum.clone()) || gorulen.len() > 1000 {
                continue;
            }
            for (hedef, _, _, _) in kenarlar.get(&dugum).into_iter().flatten() {
                let mut e = etiket.clone();
                e.extend(kenar_etiketleri.get(&(dugum.clone(), hedef.clone())).cloned().unwrap_or_default());
                include_etiketleri.entry(hedef.clone()).or_insert_with(|| e.clone());
                kuyruk.push((hedef.clone(), e));
            }
        }
    }

    let mut kapilar = vec![];
    // Önizlemenin sınayamadığı girdiler (dosya yüklemesi), kapının "dosya:satır" anahtarıyla
    let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
    let mut kosullu_kapilar = vec![];
    // İşlemin router'ı ("dosya:satır" -> router kimliği): bağımlılık grafiği
    let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
    // Rota kayıtları: dekoratörler (@router.get) ve router.add_api_route(...) çağrıları
    // (kaydın modülü, alıcı, yöntemler, kayıt çağrısı, fonksiyonun modülü, fonksiyon)
    let yontem_listesi = |c: &Cagri| -> Vec<String> {
        c.anahtarli
            .iter()
            .find(|(k, _)| k == "methods")
            .map(|(_, v)| ust_duzey_bol(v.trim_matches(['[', ']', '(', ')', '{', '}'])).iter().filter_map(|m| metin_sabiti(m)).map(|m| m.to_uppercase()).collect())
            .unwrap_or_else(|| vec!["GET".into()])
    };
    // İşlem kimliği üretimi: bütün uygulamalar aynı yolla üretiyorsa o. Etiket deseninde `tags[0]`, FastAPI'nin sırasıyla:
    // dıştan içe include_router(tags=...) etiketleri, APIRouter(tags=...), rota dekoratörünün tags=...
    let ayrac_deseni = crate::kural::derle(r"\{\s*route\.tags\[0\]\s*\}([^{}]*)\{\s*route\.name\s*\}");
    let uretimler: Vec<Option<KimlikUretimi>> = uygulamalar
        .iter()
        .map(|(di, _, c)| match c.anahtarli.iter().find(|(k, _)| k == "generate_unique_id_function") {
            None => Some(KimlikUretimi::Varsayilan),
            Some((_, v)) => {
                let (_, g) = b.ifade(*di, v.trim()).and_then(|y| b.fonksiyon(&y))?;
                let ayrac = ayrac_deseni.captures(g.donus.as_deref().unwrap_or(""))?[1].to_string();
                Some(KimlikUretimi::Etiket(ayrac))
            }
        })
        .collect();
    let kimlik_uretimi = match uretimler.as_slice() {
        [ilk, kalan @ ..] if kalan.iter().all(|x| x == ilk) => ilk.clone(),
        _ => None,
    };
    // Uygulamanın kendi işlem kimlikleri, kapının "dosya:satır" anahtarıyla; aynı fonksiyona iki farklı kimlik düşerse yazılmaz
    let mut islem_kimlikleri: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut kayitlar: Vec<RotaKaydi> = vec![];
    // Rota kaynağı olduğu görülen ama okunamayan yerler: (modül, satır, alıcı, neden). Haritada bu rotalar yoktur;
    // rapor, doktor, Ayna ve kart kapsamı "kısmen" der
    let mut okunamayan: Vec<(usize, usize, String, String)> = vec![];
    // Uygulama düzeyi ara katmanlar: (uygulama düğümü, ara katman)
    let mut ara_katmanlar: Vec<(Dugum, AraKatman)> = vec![];
    for (di, d) in dosyalar.iter().enumerate() {
        for f in &d.fonksiyonlar {
            for dek in &f.dekoratorler {
                let Some((alici, yontem)) = dek.ad.rsplit_once('.') else { continue };
                let yontemler = if YONTEMLER.contains(&yontem) {
                    vec![yontem.to_uppercase()]
                } else if yontem == "api_route" {
                    yontem_listesi(dek)
                } else if yontem == "middleware" {
                    // @app.middleware("http"): fonksiyonun ilk parametresi istektir
                    let http = dek.argumanlar.first().and_then(|a| metin_sabiti(a)).as_deref() == Some("http");
                    let uygulama =
                        kapsam_zinciri(d, f).iter().map(|k| (di, format!("{k}().{alici}"))).find(|a| uygulamalar.iter().any(|(u, x, _)| *u == a.0 && *x == a.1)).or_else(
                            || match b.ifade(di, alici) {
                                Some(Yer::Ad(adi, aad)) if uygulamalar.iter().any(|(u, x, _)| *u == adi && *x == aad) => Some((adi, aad)),
                                _ => None,
                            },
                        );
                    if let (true, Some(u), Some(p)) = (http, uygulama, f.parametreler.first()) {
                        ara_katmanlar.extend(b.ara_katman(di, f.ad.clone(), f, &p.ad).map(|a| (u, a)));
                    }
                    continue;
                } else {
                    // Router ya da uygulama örneğinin tanınmayan rota dekoratörü (`@router.route(...)`); yaşam döngüsü,
                    // websocket ve hata işleyicisi HTTP işlemi değildir
                    let rota_disi = ["websocket", "websocket_route", "on_event", "middleware", "exception_handler", "lifespan"].contains(&yontem);
                    let ornek = matches!(b.ifade(di, alici), Some(Yer::Ad(adi, aad)) if routerlar.contains_key(&(adi, aad.clone())) || uygulamalar.iter().any(|(u, a, _)| *u == adi && *a == aad));
                    if ornek && !rota_disi {
                        okunamayan.push((di, f.satir, alici.to_string(), format!("@{}: tanınmayan rota dekoratörü", dek.ad)));
                    }
                    continue;
                };
                kayitlar.push((di, alici.to_string(), yontemler, dek.clone(), di, f, vec![]));
            }
        }
        // Sınıf tabanlı görünüm: sınıf dekoratörünün argümanı bir router (`@controller(router)`, `@cbv(router)`); sınıftaki
        // `@router.get(...)` yöntemleri o router'ın uçlarıdır, yol öneki router'dan. `self` FastAPI'nin bağımlılık
        // enjeksiyonuyla kurulur: sınıfın (ve projedeki taban sınıflarının) `Depends(...)` öznitelikleri her ucun bağımlılığıdır
        for s in &d.siniflar {
            let cbv: Vec<&str> = s
                .dekoratorler
                .iter()
                .filter_map(|c| c.argumanlar.first())
                .map(|a| a.trim())
                .filter(|a| matches!(b.ifade(di, a), Some(Yer::Ad(adi, aad)) if routerlar.contains_key(&(adi, aad.clone()))))
                .collect();
            if cbv.is_empty() {
                continue;
            }
            let mut oznitelikler: BTreeMap<String, (usize, crate::python::Parametre)> = BTreeMap::new();
            sinif_bagimliliklari(&b, di, s, 0, &mut oznitelikler);
            let sinif_p: Vec<(usize, crate::python::Parametre)> = oznitelikler.into_values().collect();
            for m in &s.metotlar {
                for dek in &m.dekoratorler {
                    let Some((alici, yontem)) = dek.ad.rsplit_once('.') else { continue };
                    if !cbv.contains(&alici) {
                        continue;
                    }
                    let yontemler = if YONTEMLER.contains(&yontem) {
                        vec![yontem.to_uppercase()]
                    } else if yontem == "api_route" {
                        yontem_listesi(dek)
                    } else {
                        continue;
                    };
                    kayitlar.push((di, alici.to_string(), yontemler, dek.clone(), di, m, sinif_p.clone()));
                }
            }
        }
        for c in d.cagrilar.iter().filter(|c| c.ad.ends_with(".add_api_route")) {
            let alici = c.ad.trim_end_matches(".add_api_route").to_string();
            let Some(hedef) = c.argumanlar.get(1).cloned().or_else(|| c.anahtarli.iter().find(|(k, _)| k == "endpoint").map(|(_, v)| v.clone())) else {
                okunamayan.push((di, c.satir, alici.clone(), format!("{alici}.add_api_route: işleyici okunamadı")));
                continue;
            };
            let Some((fdi, f)) = b.ifade(di, &hedef).and_then(|y| b.fonksiyon(&y)) else {
                okunamayan.push((di, c.satir, alici.clone(), format!("{alici}.add_api_route: işleyici okunamadı ({})", kisalt(&hedef))));
                continue;
            };
            kayitlar.push((di, alici, yontem_listesi(c), c.clone(), fdi, f, vec![]));
        }
    }
    // app.add_middleware(S): sınıfın `dispatch` yöntemi (BaseHTTPMiddleware), istek ikinci parametresidir
    for (di, d) in dosyalar.iter().enumerate() {
        let mut cagrilar: Vec<(Option<&Fonksiyon>, &Cagri)> = d.cagrilar.iter().map(|c| (None, c)).collect();
        for f in &d.fonksiyonlar {
            cagrilar.extend(f.ic_cagrilar.iter().map(|c| (Some(f), c)));
        }
        for (f, c) in cagrilar {
            let Some(alici) = c.ad.strip_suffix(".add_middleware") else { continue };
            let Some(u) = alici_bul(di, f, alici).filter(|u| uygulamalar.iter().any(|(x, a, _)| *x == u.0 && *a == u.1)) else { continue };
            let Some((sdi, s)) = c.argumanlar.first().and_then(|a| b.ifade(di, a.trim())).and_then(|y| b.sinif(&y)) else { continue };
            let Some(m) = s.metotlar.iter().find(|m| m.ad == "dispatch") else { continue };
            if let Some(p) = m.parametreler.iter().find(|p| p.ad != "self") {
                ara_katmanlar.extend(b.ara_katman(sdi, s.ad.clone(), m, &p.ad).map(|a| (u.clone(), a)));
            }
        }
    }
    // Ara katmanı olan her uygulamanın ulaştığı router'lar (ana uygulamanınki zaten hesaplı)
    let mut ara_erisim: HashMap<Dugum, Erisim> = HashMap::new();
    for (u, _) in &ara_katmanlar {
        if !ara_erisim.contains_key(u) {
            let e = if *u == ana_kok { erisim.clone() } else { ulas(u.clone(), vec![]) };
            ara_erisim.insert(u.clone(), e);
        }
    }
    // En az bir rotası okunan router ve uygulama örnekleri
    let mut okunan_routerlar: HashSet<Dugum> = HashSet::new();
    for (kdi, alici, yontemler, dek, di, f, sinif_p) in kayitlar {
        let d = &dosyalar[di];
        let dek = &dek;
        {
            {
                // Fabrikanın içindeki rota fonksiyonu: alıcı fabrikanın yerel router'ı ("kur().router")
                // İç içe fabrikada ad dışarı doğru aranır (Python kapanışı): önce tanımlandığı fonksiyon, sonra onun üstü
                let mut kapsamlar = vec![];
                let mut k = f.kapsam.clone();
                while let Some(ad) = k.filter(|_| kapsamlar.len() < 16) {
                    k = d.fonksiyonlar.iter().find(|g| g.ad == ad).and_then(|g| g.kapsam.clone());
                    kapsamlar.push(ad);
                }
                let yerel =
                    kapsamlar.iter().map(|k| (kdi, format!("{k}().{alici}"))).find(|a| routerlar.contains_key(a) || uygulamalar.iter().any(|(u, x, _)| *u == a.0 && *x == a.1));
                let (adi, aad) = match yerel {
                    Some(a) => a,
                    None => {
                        let Some(Yer::Ad(adi, aad)) = b.ifade(kdi, &alici) else { continue };
                        (adi, aad)
                    }
                };
                let anahtar = (adi, aad.clone());
                kapi_routerlari.insert(format!("{}:{}", guvenli::goreli(kok, &d.yol), f.satir), format!("{}:{}", guvenli::goreli(kok, &dosyalar[adi].yol), aad));
                let router = routerlar.get(&anahtar);
                let uygulama_mi = uygulamalar.iter().any(|(u, a, _)| *u == adi && *a == aad);
                if router.is_none() && !uygulama_mi {
                    continue;
                }
                let yol_ham = dek.argumanlar.first().cloned().or_else(|| dek.anahtarli.iter().find(|(k, _)| k == "path").map(|(_, v)| v.clone())).unwrap_or_default();
                // Yol, fabrikanın parametresi olabilir (`def create_api_app(health_check_path: str = "/health")`): varsayılanı
                let kapsam_parametresi = || {
                    kapsamlar.iter().find_map(|k| {
                        let g = d.fonksiyonlar.iter().find(|g| &g.ad == k)?;
                        g.parametreler.iter().find(|p| p.ad == yol_ham.trim()).and_then(|p| p.varsayilan.as_deref()).and_then(metin_sabiti)
                    })
                };
                let Some(yol) = b.deger(kdi, &yol_ham, 0).or_else(kapsam_parametresi) else {
                    okunamayan.push((kdi, dek.satir, alici.clone(), format!("{}: yol çözülemedi ({})", dek.ad, kisalt(&yol_ham))));
                    continue;
                };
                okunan_routerlar.insert(anahtar.clone());
                let (onek, mut bag, kosullu_erisim, ana) = match erisim.get(&anahtar) {
                    Some((o, bg, k)) => (o.clone(), bg.clone(), *k, true),
                    // Uygulamaya bağlanmamış router: kendi öneki, ikincil
                    None => (
                        router.map(|r| r.onek.clone()).unwrap_or_default(),
                        router.map(|r| r.bagimliliklar.iter().map(|x| (r.di, x.clone())).collect()).unwrap_or_default(),
                        false,
                        false,
                    ),
                };
                // Koşul altında tanımlanan rota fonksiyonu da koşullu kapıdır
                let kosullu = kosullu_erisim || f.kosullu_tanim;
                // Rotanın kendi önekiyle: erişim haritasındaki önek router'ın kendi önekini zaten içerir
                let tam = format!("{onek}{yol}");
                let tam = if tam.is_empty() { "/".to_string() } else { tam };
                bag.extend(dek.anahtarli.iter().find(|(k, _)| k == "dependencies").map(|(_, v)| bagimlilik_listesi(v)).unwrap_or_default().into_iter().map(|x| (kdi, x)));
                // Parametreler
                let yol_adlari: Vec<String> =
                    tam.split('/').filter_map(|p| p.strip_prefix('{').and_then(|x| x.strip_suffix('}'))).map(|x| x.split(':').next().unwrap_or(x).to_string()).collect();
                let (mut govde, mut sorgu_alanlari) = (vec![], BTreeMap::new());
                // Gövde parametreleri (model ya da Body): (ad, gömülü hâldeki alan, tek başınaysa girdi, embed=True)
                let mut govde_paramlari: Vec<(String, Alan, Option<Girdi>, bool)> = vec![];
                // UploadFile / File(): dosyanın varlığı, türü ve içeriği önizlemede sınanmaz
                let mut dosya_notlari: Vec<String> = vec![];
                let dosya_notu =
                    |ad: &str, neden: &str| format!("dosya doğrulaması kontrol edilemedi: '{ad}' parametresi ({neden}); dosyanın varlığı, türü ve içeriği önizlemede sınanmaz");
                let mut parametreler = vec![];
                // Uç noktanın parametreleri ve (FastAPI kuralı) bağımlılık fonksiyonlarının kendi parametreleri: her biri
                // kendi modülünde çözülür
                let mut kuyruk: Vec<(usize, &crate::python::Parametre)> = f.parametreler.iter().filter(|p| p.ad != "self" && p.ad != "cls").map(|p| (di, p)).collect();
                kuyruk.extend(sinif_p.iter().map(|(d2, p)| (*d2, p)));
                let mut gezilen: HashSet<(usize, String)> = HashSet::new();
                let mut sira = 0;
                while sira < kuyruk.len() && sira < 500 {
                    let (pdi, p) = kuyruk[sira];
                    sira += 1;
                    let bagimliliklar = b.param_bagimliliklari(pdi, p.tip.as_deref(), p.varsayilan.as_deref());
                    let (tip_ham, _) = tip_coz(p.tip.as_deref().unwrap_or(""));
                    let temel = tip_ham.split('[').next().unwrap_or(&tip_ham).rsplit('.').next().unwrap_or("").to_string();
                    if !bagimliliklar.is_empty() {
                        // Depends() sınıfla: OAuth2PasswordRequestForm gibi form girdisi
                        if bagimliliklar.iter().any(|(_, h)| h.ends_with("OAuth2PasswordRequestForm") || h.ends_with("OAuth2PasswordRequestFormStrict")) {
                            govde.push(Girdi {
                                ad: None,
                                tip: Some("OAuth2PasswordRequestForm".into()),
                                sema: Some(Sema {
                                    adaylar: vec![],
                                    dizi: false,
                                    dto: "OAuth2PasswordRequestForm".into(),
                                    dosya: None,
                                    bulunamadi: false,
                                    kalitim: None,
                                    alanlar: oauth2_formu(f.satir),
                                }),
                            });
                        } else {
                            for (hdi, h) in &bagimliliklar {
                                // Fabrika ya da __call__'lu sınıf örneği: FastAPI döndürülen fonksiyonun parametrelerini okur
                                let cagri = h.trim().strip_suffix(')').and_then(|x| x.split_once('(')).and_then(|(ad, _)| b.cagrilan(*hdi, ad));
                                if let Some((fdi, f2)) = cagri.or_else(|| b.ifade(*hdi, h).and_then(|y| b.fonksiyon(&y))) {
                                    if gezilen.insert((fdi, f2.ad.clone())) {
                                        kuyruk.extend(f2.parametreler.iter().filter(|p| p.ad != "self" && p.ad != "cls").map(|p| (fdi, p)));
                                    }
                                }
                            }
                            bag.extend(bagimliliklar);
                        }
                        continue;
                    }
                    if tip_ham.contains("UploadFile") {
                        let n = dosya_notu(&p.ad, "UploadFile");
                        if !dosya_notlari.contains(&n) {
                            dosya_notlari.push(n);
                        }
                    }
                    if ENJEKTE.contains(&temel.as_str()) {
                        continue;
                    }
                    if yol_adlari.contains(&p.ad) {
                        parametreler.push(Girdi { ad: Some(p.ad.clone()), tip: Some(tip_ham.clone()), sema: None });
                        continue;
                    }
                    // Parametre işareti: Query / Path / Header / Cookie / Body / Form / File (varsayılan ya da Annotated içinde)
                    // Varsayılan, işaret döndüren projedeki bir yardımcı olabilir: def LimitBody(): return Body(10, ge=1)
                    let yardimci = p.varsayilan.as_deref().filter(|v| isaret_varsayilanda(v).is_none()).and_then(|v| {
                        let (_, f2) = b.ifade(pdi, v.trim().split('(').next()?).and_then(|y| b.fonksiyon(&y))?;
                        f2.donus.clone().filter(|r| isaret_varsayilanda(r).is_some())
                    });
                    let isaret = isaret(p.tip.as_deref(), yardimci.as_deref().or(p.varsayilan.as_deref()));
                    let tur = isaret.as_ref().map(|(t, _)| t.as_str()).unwrap_or("");
                    if tur == "File" && !tip_ham.contains("UploadFile") {
                        dosya_notlari.push(dosya_notu(&p.ad, "File()"));
                    }
                    if matches!(tur, "Header" | "Cookie") {
                        continue;
                    }
                    // Tip modül yoluyla yazılmış olabilir (schemas.actions.FlowCreate): tam yol çözülür
                    let tam_ad = tip_ham.split('[').next().unwrap_or(&tip_ham).trim().to_string();
                    let model = b.ifade(pdi, &tam_ad).and_then(|y| b.sinif(&y)).filter(|(d2, s)| b.pydantic_mi(*d2, s, 0));
                    let gomulsun = isaret.as_ref().is_some_and(|(t, a)| t == "Body" && a.iter().any(|x| x.replace(' ', "") == "embed=True"));
                    if let Some((d2, s)) = model.filter(|_| !matches!(tur, "Query" | "Form" | "File")) {
                        let mut alanlar = BTreeMap::new();
                        b.alanlar(d2, s, 0, &mut alanlar);
                        // Gömülü hâlde: parametre adıyla bir nesne alanı; varsayılanı yoksa (ya da ...) zorunlu
                        let varsayilan = match &isaret {
                            Some((_, a)) => a
                                .iter()
                                .find(|x| !x.contains('='))
                                .cloned()
                                .or_else(|| a.iter().find_map(|x| x.split_once('=').filter(|(k, _)| k.trim() == "default").map(|(_, v)| v.trim().to_string()))),
                            None => p.varsayilan.clone(),
                        };
                        let (_, nullable) = tip_coz(p.tip.as_deref().unwrap_or(""));
                        let mut kurallar = vec!["ValidateNested()".to_string()];
                        if nullable {
                            kurallar.push("nullable()".into());
                        }
                        let gomulu = Alan {
                            tip: Some("object".into()),
                            zorunlu: varsayilan.as_deref().is_none_or(|v| v == "..."),
                            belirsiz: false,
                            kurallar,
                            satir: f.satir,
                            kalitimdan: None,
                            degerler: vec![],
                            ic: None,
                        };
                        govde_paramlari.push((
                            p.ad.clone(),
                            gomulu,
                            Some(Girdi {
                                ad: Some(p.ad.clone()),
                                tip: Some(s.ad.clone()),
                                sema: Some(Sema {
                                    adaylar: vec![],
                                    dizi: false,
                                    dto: s.ad.clone(),
                                    dosya: Some(guvenli::goreli(kok, &dosyalar[d2].yol)),
                                    bulunamadi: false,
                                    kalitim: s.tabanlar.first().cloned(),
                                    alanlar,
                                }),
                            }),
                            gomulsun,
                        ));
                        continue;
                    }
                    // İşaretin argümanları Field(...) gibi okunur: varsayılan, alias ve sınırlar
                    let (alan_adi, alan_varsayilani) = match &isaret {
                        Some((_, arglar)) => {
                            let konumsal = arglar.iter().find(|x| !x.contains('='));
                            let anahtar = |a: &str| arglar.iter().find_map(|x| x.split_once('=').filter(|(k, _)| k.trim() == a).map(|(_, v)| v.trim().to_string()));
                            let varsayilan =
                                konumsal.cloned().or_else(|| anahtar("default")).or_else(|| p.varsayilan.clone().filter(|v| !v.contains('(') || isaret_varsayilanda(v).is_none()));
                            let sinirlar: Vec<String> =
                                arglar.iter().filter(|x| x.contains('=') && !x.trim_start().starts_with("default") && !x.trim_start().starts_with("alias")).cloned().collect();
                            let alan = match varsayilan.as_deref() {
                                None | Some("...") => format!("Field(..., {})", sinirlar.join(", ")),
                                Some(d) => format!("Field(default={d}, {})", sinirlar.join(", ")),
                            };
                            (anahtar("alias").and_then(|a| metin_sabiti(&a)).unwrap_or_else(|| p.ad.clone()), Some(alan))
                        }
                        None => (p.ad.clone(), p.varsayilan.clone()),
                    };
                    let a = b.alan(pdi, &tip_ham_annotated(p.tip.as_deref().unwrap_or("")), alan_varsayilani.as_deref(), f.satir);
                    match tur {
                        "Body" => govde_paramlari.push((alan_adi, a, None, gomulsun)),
                        "Form" | "File" => match govde.iter_mut().find(|g| g.ad.is_none() && g.sema.as_ref().is_some_and(|s| s.dto == "form")) {
                            Some(g) => {
                                g.sema.as_mut().unwrap().alanlar.insert(alan_adi, a);
                            }
                            None => govde.push(Girdi {
                                ad: None,
                                tip: Some(tur.to_string()),
                                sema: Some(Sema {
                                    adaylar: vec![],
                                    dizi: false,
                                    dto: "form".into(),
                                    dosya: Some(guvenli::goreli(kok, &d.yol)),
                                    bulunamadi: false,
                                    kalitim: None,
                                    alanlar: BTreeMap::from([(alan_adi, a)]),
                                }),
                            }),
                        },
                        "Path" => parametreler.push(Girdi { ad: Some(alan_adi), tip: Some(tip_ham.clone()), sema: None }),
                        _ => {
                            sorgu_alanlari.insert(alan_adi, a);
                        }
                    }
                }
                // FastAPI: tek gövde parametresi gövdenin kendisidir; birden fazlaysa (ya da embed=True) gövde parametre
                // adlarıyla gömülür: {"flows": {...}, "limit": 10}
                if govde_paramlari.len() > 1 || govde_paramlari.iter().any(|x| x.3) {
                    let alanlar = govde_paramlari.into_iter().map(|(ad, a, _, _)| (ad, a)).collect();
                    govde.push(Girdi {
                        ad: None,
                        tip: Some("Body".into()),
                        sema: Some(Sema { adaylar: vec![], dizi: false, dto: "body".into(), dosya: Some(guvenli::goreli(kok, &d.yol)), bulunamadi: false, kalitim: None, alanlar }),
                    });
                } else if let Some((ad, a, girdi, _)) = govde_paramlari.pop() {
                    govde.push(girdi.unwrap_or(Girdi { ad: Some(ad), tip: a.tip.clone(), sema: None }));
                }
                let sorgu = if sorgu_alanlari.is_empty() {
                    vec![]
                } else {
                    vec![Girdi {
                        ad: None,
                        tip: None,
                        sema: Some(Sema {
                            adaylar: vec![],
                            dizi: false,
                            dto: "query".into(),
                            dosya: Some(guvenli::goreli(kok, &d.yol)),
                            bulunamadi: false,
                            kalitim: None,
                            alanlar: sorgu_alanlari,
                        }),
                    }]
                };
                // Giriş kararı: bağımlılık zinciri
                let mut g = Guvenlik::default();
                for (bdi, h) in &bag {
                    g = g.birlestir(b.guvenlik(*bdi, h, &mut HashSet::new(), 0));
                }
                let bilinmeyen = g.bilinmeyen;
                // Kesin yalnızca hata veren bir güvenlik şemasına ulaşan zincir; auto_error=False ile kurulan kararlar tahmindir
                let (giris, kanit) = if g.gerekli {
                    ("gerekli", "kesin")
                } else if g.tahmini_gerekli {
                    ("gerekli", if bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" })
                } else if g.istege {
                    ("istege_bagli", if bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" })
                } else if bilinmeyen {
                    ("herkese_acik", "varsayim_bilinmeyen_guard")
                } else {
                    ("herkese_acik", "varsayim")
                };
                let ozet =
                    dek.anahtarli.iter().find(|(k, _)| k == "summary").and_then(|(_, v)| metin_sabiti(v)).or_else(|| f.belge.clone()).or_else(|| Some(varsayilan_ozet(&f.ad)));
                let kalkmis = dek.anahtarli.iter().any(|(k, v)| k == "deprecated" && v.trim() == "True");
                let yanit = b.yanit(kok, dosyalar, di, &dek.anahtarli, f);
                let sezgi_yolu = tam
                    .split('/')
                    .map(|p| p.strip_prefix('{').and_then(|x| x.strip_suffix('}')).map(|x| format!(":{}", x.split(':').next().unwrap_or(x))).unwrap_or_else(|| p.to_string()))
                    .collect::<Vec<_>>()
                    .join("/");
                let stem = d.yol.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let mut guardlar: Vec<String> =
                    bag.iter().map(|(bdi, h)| if b.anahtar_bagimliligi(*bdi, h) { format!("{h}{}", crate::anlam::ANAHTAR_GUARD_ETIKETI) } else { h.clone() }).collect();
                guardlar.dedup();
                let mut roller: Vec<String> = bag.iter().flat_map(|(bdi, h)| b.roller(*bdi, h)).collect();
                roller.sort();
                roller.dedup();
                // Bu router'a ulaşan uygulamaların ara katmanları
                let ara: Vec<&AraKatman> = ara_katmanlar.iter().filter(|(u, _)| ara_erisim.get(u).is_some_and(|e| e.contains_key(&anahtar))).map(|(_, a)| a).collect();
                for yontem in &yontemler {
                    let sonuclar: Vec<(&str, AraSonuc)> = ara.iter().map(|a| (a.ad.as_str(), a.sonuc(&b, yontem, &tam))).collect();
                    let (giris, kanit, ara_guardlari) = ara_katman_karari(giris, kanit, &sonuclar);
                    let mut guardlar = guardlar.clone();
                    guardlar.extend(ara_guardlari);
                    if kosullu {
                        kosullu_kapilar.push(format!("{yontem} {sezgi_yolu}"));
                    }
                    let kimlik = match &kimlik_uretimi {
                        // Birden fazla yönteme bağlı rotada FastAPI ilk yöntemi kümeden seçer: hangisi olduğu belirsiz
                        Some(KimlikUretimi::Varsayilan) if yontemler.len() == 1 => {
                            let ham = format!("{}{tam}", f.ad);
                            let ham: String = ham.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
                            Some(format!("{ham}_{}", yontem.to_lowercase()))
                        }
                        Some(KimlikUretimi::Etiket(ayrac)) => include_etiketleri
                            .get(&anahtar)
                            .and_then(|e| e.first().cloned())
                            .or_else(|| router.and_then(|r| r.etiketler.first().cloned()))
                            .or_else(|| etiket_listesi(dek).into_iter().next())
                            .map(|e| format!("{e}{ayrac}{}", f.ad)),
                        _ => None,
                    };
                    if let Some(kimlik) = kimlik {
                        let anahtar = format!("{}:{}", guvenli::goreli(kok, &d.yol), f.satir);
                        match islem_kimlikleri.get(&anahtar) {
                            Some(Some(eski)) if *eski != kimlik => {
                                islem_kimlikleri.insert(anahtar, None);
                            }
                            Some(_) => {}
                            None => {
                                islem_kimlikleri.insert(anahtar, Some(kimlik));
                            }
                        }
                    }
                    if !dosya_notlari.is_empty() {
                        let notlar = denetlenemeyen.entry(format!("{}:{}", guvenli::goreli(kok, &d.yol), f.satir)).or_default();
                        for n in &dosya_notlari {
                            if !notlar.contains(n) {
                                notlar.push(n.clone());
                            }
                        }
                    }
                    let yer = format!("{}:{}", guvenli::goreli(kok, &d.yol), f.satir);
                    if let std::collections::btree_map::Entry::Vacant(e) = dna_haritasi.entry(yer) {
                        let v = dna_zinciri(&b, kok, di, f, &bag);
                        if !crate::dna::bos(&v) {
                            e.insert(v);
                        }
                    }
                    kapilar.push(Kapi {
                        id: format!("{yontem} {sezgi_yolu}"),
                        yontem: yontem.clone(),
                        yol: sezgi_yolu.clone(),
                        kontrolcu: pascal(&stem),
                        fonksiyon: snake_camel(&f.ad),
                        dosya: guvenli::goreli(kok, &d.yol),
                        satir: f.satir,
                        giris: giris.into(),
                        guardlar,
                        roller: roller.clone(),
                        roller_kaniti: (!roller.is_empty()).then(|| "varsayim".into()),
                        ozet: ozet.clone(),
                        govde: govde.clone(),
                        sorgu: sorgu.clone(),
                        parametreler: parametreler.clone(),
                        kullanimdan_kalkmis: kalkmis,
                        yerine: None,
                        surum: None,
                        uygulama: None,
                        ana,
                        giris_kaniti: kanit.into(),
                        veri: veri_bagi(&b, &modeller, di, f),
                        yanit: yanit.clone(),
                    });
                }
            }
        }
    }
    kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
    // Uygulamaya bağlanan ama hiçbir rotası okunamayan router: içi başka router içermiyorsa rotaları koddan okunamayan
    // bir yolla kaydediliyor (sınıf tabanlı görünüm, döngüyle kurulan yol, tanınmayan dekoratör). Kaydı ayrıca
    // okunamayan router için tek yer yeter
    for (anahtar, r) in &routerlar {
        let bagli = erisim.contains_key(anahtar) || icerme.iter().any(|(_, alt, ..)| alt == anahtar);
        let ic_router = kenarlar.get(anahtar).is_some_and(|k| !k.is_empty());
        let kaydi_var = okunamayan.iter().any(|(odi, _, oalici, _)| matches!(b.ifade(*odi, oalici), Some(Yer::Ad(adi, aad)) if adi == anahtar.0 && aad == anahtar.1));
        if bagli && !ic_router && !kaydi_var && !okunan_routerlar.contains(anahtar) {
            let satir = ornek_satiri(dosyalar, anahtar);
            okunamayan.push((r.di, satir, anahtar.1.clone(), format!("{}: router uygulamaya bağlı ama rotası okunamadı", anahtar.1)));
        }
    }
    let mut okunamayan_rotalar: Vec<String> = okunamayan.iter().map(|(odi, satir, _, neden)| format!("{}:{satir} {neden}", guvenli::goreli(kok, &dosyalar[*odi].yol))).collect();
    okunamayan_rotalar.sort();
    okunamayan_rotalar.dedup();
    let ana = (ana_kok.0 != usize::MAX).then(|| format!("{}:{}", dosyalar[ana_kok.0].modul, ana_kok.1));
    // Ana uygulamanın kimlik denetleyen ara katmanları (gösterim: kapsamı her işlemin kendi kararındadır)
    let genel_guardlar: Vec<String> = ara_katmanlar.iter().filter(|(u, _)| *u == ana_kok).map(|(_, a)| format!("{} (ara katman)", a.ad)).collect();
    (
        kapilar,
        json!({ "genel_onek": "", "genel_guardlar": genel_guardlar, "ana_uygulama": ana, "kosullu_kapilar": kosullu_kapilar, "denetlenemeyen": denetlenemeyen,
        "dna": dna_haritasi, "kaskadlar": kaskadlar(kok, dosyalar),
        "routerlar": router_grafi(kok, dosyalar, &icerme, &routerlar, &uygulamalar), "kapi_routerlari": kapi_routerlari,
        "veri_modelleri": tablo_modelleri(kok, dosyalar), "okunamayan_rotalar": okunamayan_rotalar,
        "islem_kimlikleri": islem_kimlikleri.into_iter().filter_map(|(a, k)| k.map(|k| (a, k))).collect::<BTreeMap<_, _>>() }),
    )
}

// ---- Uygulama düzeyi ara katman ----
//
// `@app.middleware("http")` fonksiyonu ya da `app.add_middleware(S)` ile eklenen sınıfın `dispatch` yöntemi her
// isteği işleyiciden önce görür. Gövdesi bir kimlik bilgisine (anahtar, belirteç, oturum taşıyan başlık, çerez ya da
// sorgu) bakıp 401 / 403 döndürüyorsa kapsadığı işlemler girişsiz değildir. Kapsam gövdenin koşullarından okunur:
// yöntem koşulu (`request.method not in (...)`), yol istisnaları (`startswith`, sabit küme, tek `return`lük yardımcı
// fonksiyon) ve başka bir korumaya devir. Çözülemeyen koşul kapsamı "bilinmiyor" yapar; karar hiçbir zaman kesin değildir.

/// Ara katmanın bir işleme etkisi
#[derive(Clone, Copy, PartialEq, Debug)]
enum AraSonuc {
    /// İstek kimlik denetiminden geçmeden işleyiciye ulaşmaz: giriş gerekli (tahmin)
    Gerekli,
    /// Denetim bu işleme uygulanmaz
    Degil,
    /// Kapsam (yöntem koşulu, yol istisnası, başka korumaya devir) çözülemedi
    Bilinmiyor,
}

/// Koşulun üç değerli sonucu; `Kimlik`: sonuç isteğin kimlik bilgisine bağlı
#[derive(Clone, Copy, PartialEq, Debug)]
enum Deger {
    Dogru,
    Yanlis,
    Kimlik,
    Belirsiz,
}

impl Deger {
    fn degil(self) -> Deger {
        match self {
            Deger::Dogru => Deger::Yanlis,
            Deger::Yanlis => Deger::Dogru,
            x => x,
        }
    }
    fn ve(self, o: Deger) -> Deger {
        use Deger::*;
        match (self, o) {
            (Yanlis, _) | (_, Yanlis) => Yanlis,
            (Belirsiz, _) | (_, Belirsiz) => Belirsiz,
            (Kimlik, _) | (_, Kimlik) => Kimlik,
            _ => Dogru,
        }
    }
    fn veya(self, o: Deger) -> Deger {
        use Deger::*;
        match (self, o) {
            (Dogru, _) | (_, Dogru) => Dogru,
            (Belirsiz, _) | (_, Belirsiz) => Belirsiz,
            (Kimlik, _) | (_, Kimlik) => Kimlik,
            _ => Yanlis,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum OlayTuru {
    /// 401 / 403 döndürür ya da fırlatır
    Red,
    /// İsteği işleyiciye geçirir (`await call_next(request)`)
    Gecis,
    /// İsteği başka bir korumaya devreder (`return await koruma(request, call_next)`)
    Devir,
}

/// Gövdeden okunan olay ve gerçekleştiği koşullar (dıştan içe, hepsi doğruysa). `BELIRSIZ_KOSUL`: çözülemeyen koşul
/// (except bloğu, döngü); `False`: hiç çalışmayan gövde (iç fonksiyon)
struct AraOlay {
    kosullar: Vec<String>,
    tur: OlayTuru,
}

const BELIRSIZ_KOSUL: &str = "<?>";
/// Başlık, çerez ya da sorgu adında geçtiğinde değeri kimlik bilgisi sayılan parçalar
const KIMLIK_ADLARI: [&str; 11] = ["auth", "token", "key", "secret", "session", "signature", "jwt", "bearer", "cookie", "credential", "password"];
const RED_ISARETLERI: [&str; 6] = ["401", "403", "UNAUTHORIZED", "FORBIDDEN", "Unauthorized", "Forbidden"];

/// Bir uygulama düzeyi ara katman: adı, modülü, istek parametresinin adı, olayları ve gövdedeki yerel takma adlar
pub(crate) struct AraKatman {
    ad: String,
    di: usize,
    istek: String,
    olaylar: Vec<AraOlay>,
    /// `yol = request.url.path`
    yol_adlari: Vec<String>,
    /// `yontem = request.method`
    yontem_adlari: Vec<String>,
    /// `belirtec = request.headers.get("Authorization")`
    kimlik_adlari: Vec<String>,
}

/// Koşul değerlendirme bağlamı: ifadenin modülü ve isteğe bağlanan adlar (yardımcı fonksiyona girildiğinde parametreler)
struct AraBaglam<'k> {
    di: usize,
    istek: &'k str,
    yol_adlari: Vec<String>,
    yontem_adlari: Vec<String>,
    kimlik_adlari: Vec<String>,
}

/// Gövde metninin mantıksal satırları: (girinti, metin). Parantez içinde ve `\` ile süren satırlar birleşir, yorumlar
/// atılır, üçlü tırnaklı dizgiler (belge dizgisi) boş dizgi olur. `sutun`: ilk satırın kaynaktaki sütunu
fn mantiksal_satirlar(govde: &str, sutun: usize) -> Vec<(usize, String)> {
    // Windows satır sonu atılır: aksi hâlde `\` ile süren satırın sonu `\r` olur ve devam görülmez
    let c: Vec<char> = govde.chars().filter(|c| *c != '\r').collect();
    let mut out = vec![];
    let mut cur = String::new();
    let (mut girinti, mut derinlik, mut i) = (sutun, 0usize, 0usize);
    let (mut satir_basi, mut devam) = (false, false);
    while i < c.len() {
        if satir_basi {
            let mut n = 0;
            while i < c.len() && (c[i] == ' ' || c[i] == '\t') {
                n += if c[i] == '\t' { 8 } else { 1 };
                i += 1;
            }
            satir_basi = false;
            if !devam {
                girinti = n;
            }
            continue;
        }
        let ch = c[i];
        match ch {
            '#' => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '"' | '\'' => {
                let uclu = i + 2 < c.len() && c[i + 1] == ch && c[i + 2] == ch;
                let bas = i;
                i += if uclu { 3 } else { 1 };
                while i < c.len() {
                    if c[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    if uclu && i + 2 < c.len() && c[i] == ch && c[i + 1] == ch && c[i + 2] == ch {
                        i += 3;
                        break;
                    }
                    if !uclu && (c[i] == ch || c[i] == '\n') {
                        i += usize::from(c[i] == ch);
                        break;
                    }
                    i += 1;
                }
                if uclu {
                    cur.push_str("\"\"");
                } else {
                    cur.extend(&c[bas..i.min(c.len())]);
                }
                continue;
            }
            '(' | '[' | '{' => derinlik += 1,
            ')' | ']' | '}' => derinlik = derinlik.saturating_sub(1),
            '\\' if c.get(i + 1) == Some(&'\n') => {
                cur.push(' ');
                i += 2;
                (satir_basi, devam) = (true, true);
                continue;
            }
            '\n' => {
                i += 1;
                satir_basi = true;
                if derinlik > 0 {
                    cur.push(' ');
                    devam = true;
                    continue;
                }
                if !cur.trim().is_empty() {
                    out.push((girinti, cur.trim().to_string()));
                }
                cur.clear();
                devam = false;
                continue;
            }
            _ => {}
        }
        cur.push(ch);
        i += 1;
    }
    if !cur.trim().is_empty() {
        out.push((girinti, cur.trim().to_string()));
    }
    out
}

/// İfadenin, parantez ve dizgi dışında sözcük olarak geçen anahtar sözcükten (`or`, `and`) bölünmüş parçaları
fn sozcukle_bol<'s>(s: &'s str, sozcuk: &str) -> Vec<&'s str> {
    let b = s.as_bytes();
    let (mut v, mut bas, mut derinlik, mut dizgi) = (vec![], 0usize, 0i32, None::<u8>);
    let mut i = 0;
    while i < b.len() {
        let ch = b[i];
        match dizgi {
            Some(q) => {
                if ch == b'\\' {
                    i += 1;
                } else if ch == q {
                    dizgi = None;
                }
            }
            None => match ch {
                b'"' | b'\'' => dizgi = Some(ch),
                b'(' | b'[' | b'{' => derinlik += 1,
                b')' | b']' | b'}' => derinlik -= 1,
                _ if derinlik == 0
                    && s[i..].starts_with(sozcuk)
                    && i > 0
                    && (b[i - 1].is_ascii_whitespace() || b[i - 1] == b')')
                    && s[i + sozcuk.len()..].starts_with([' ', '(']) =>
                {
                    v.push(s[bas..i].trim());
                    bas = i + sozcuk.len();
                    i = bas;
                    continue;
                }
                _ => {}
            },
        }
        i += 1;
    }
    v.push(s[bas..].trim());
    v
}

/// Dıştaki parantez bütün ifadeyi sarıyorsa atılır: "(a or b)" -> "a or b"
fn dis_parantez(s: &str) -> &str {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') {
        return s;
    }
    let mut derinlik = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => derinlik += 1,
            ')' => {
                derinlik -= 1;
                if derinlik == 0 && i + 1 < s.len() {
                    return s;
                }
            }
            _ => {}
        }
    }
    dis_parantez(&s[1..s.len() - 1])
}

/// Blok başlığındaki, parantez ve dizgi dışındaki ilk `:` konumu
fn ust_duzey_iki_nokta(s: &str) -> Option<usize> {
    let (mut derinlik, mut dizgi) = (0i32, None::<char>);
    for (i, ch) in s.char_indices() {
        match dizgi {
            Some(q) if ch == q => dizgi = None,
            Some(_) => {}
            None => match ch {
                '"' | '\'' => dizgi = Some(ch),
                '(' | '[' | '{' => derinlik += 1,
                ')' | ']' | '}' => derinlik -= 1,
                ':' if derinlik == 0 => return Some(i),
                _ => {}
            },
        }
    }
    None
}

/// Yol şablonu (`/a/{b}/{c:path}`) bir metinle başlıyor mu: sabit önek yetiyorsa kesin; parametre öneke karışıyorsa belirsiz
fn yol_baslar(sablon: &str, onek: &str) -> Deger {
    match sablon.find('{') {
        None if sablon.starts_with(onek) => Deger::Dogru,
        None => Deger::Yanlis,
        Some(i) if sablon[..i].starts_with(onek) => Deger::Dogru,
        Some(i) if onek.starts_with(&sablon[..i]) => Deger::Belirsiz,
        Some(_) => Deger::Yanlis,
    }
}

/// Yol şablonu bir metne eşit mi: parametreli şablon metne uyabiliyorsa belirsiz
fn yol_esit(sablon: &str, yol: &str) -> Deger {
    if !sablon.contains('{') {
        return if sablon == yol { Deger::Dogru } else { Deger::Yanlis };
    }
    let desen: String = sablon
        .split('/')
        .map(|p| match p.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            Some(x) if x.ends_with(":path") => ".+".to_string(),
            Some(_) => "[^/]+".to_string(),
            None => regex::escape(p),
        })
        .collect::<Vec<_>>()
        .join("/");
    match regex::Regex::new(&format!("^{desen}$")) {
        Ok(r) if !r.is_match(yol) => Deger::Yanlis,
        _ => Deger::Belirsiz,
    }
}

impl AraBaglam<'_> {
    fn yol_ifadesi(&self, s: &str) -> bool {
        let s = s.trim();
        (!self.istek.is_empty() && s == format!("{}.url.path", self.istek)) || self.yol_adlari.iter().any(|a| a == s)
    }
    fn yontem_ifadesi(&self, s: &str) -> bool {
        let s = s.trim();
        let s = s.strip_suffix(".upper()").unwrap_or(s);
        (!self.istek.is_empty() && s == format!("{}.method", self.istek)) || self.yontem_adlari.iter().any(|a| a == s)
    }
}

impl<'a> Baglam<'a> {
    /// Metin listesi: dizgi, demet / liste / küme sabiti (`frozenset({...})` dahil) ya da bunlardan birine çözülen ad
    fn metinler(&self, di: usize, s: &str, derinlik: usize) -> Option<Vec<String>> {
        let s = s.trim();
        if let Some(m) = metin_sabiti(s) {
            return Some(vec![m]);
        }
        let ic = ["frozenset(", "set(", "tuple(", "list("].iter().find_map(|o| s.strip_prefix(o).and_then(|x| x.strip_suffix(')'))).unwrap_or(s).trim();
        if let Some(g) = ic.strip_prefix(['(', '[', '{']).and_then(|x| x.strip_suffix([')', ']', '}'])) {
            return ust_duzey_bol(g).iter().filter(|x| !x.trim().is_empty()).map(|x| metin_sabiti(x)).collect();
        }
        if derinlik > 4 || s.is_empty() || !s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
            return None;
        }
        let y = self.ifade(di, s)?;
        let Yer::Ad(adi, _) = &y else { return None };
        let (metin, _) = self.atama(&y)?;
        self.metinler(*adi, metin, derinlik + 1)
    }

    /// Uygulama düzeyi ara katmanın olayları; kimlik bilgisine bakıp reddetmiyorsa None
    pub(crate) fn ara_katman(&self, di: usize, ad: String, f: &Fonksiyon, istek: &str) -> Option<AraKatman> {
        let mut a = AraKatman { ad, di, istek: istek.to_string(), olaylar: vec![], yol_adlari: vec![], yontem_adlari: vec![], kimlik_adlari: vec![] };
        // Açık bloklar: (girinti, koşullar); aynı girintideki if zincirinin koşulları (elif / else öncekileri olumsuzlar)
        let mut yigin: Vec<(usize, Vec<String>)> = vec![];
        let mut zincir: HashMap<usize, Vec<String>> = HashMap::new();
        let mut satirlar = mantiksal_satirlar(&f.govde_metni, f.govde_sutunu);
        let mut i = 0;
        while i < satirlar.len() {
            let (g, metin) = satirlar[i].clone();
            i += 1;
            while yigin.last().is_some_and(|(bg, _)| *bg >= g) {
                yigin.pop();
            }
            zincir.retain(|zg, _| *zg <= g);
            let ilk = metin.split([' ', ':', '(']).next().unwrap_or("");
            if !matches!(ilk, "elif" | "else") {
                zincir.remove(&g);
            }
            // Blok başlığı: koşulu ve aynı satırdaki gövdesi (`if x: return y`)
            let baslik = |onek: &str| -> Option<(String, String)> {
                let kalan = metin.strip_prefix(onek)?;
                let son = ust_duzey_iki_nokta(kalan)?;
                Some((kalan[..son].trim().to_string(), kalan[son + 1..].trim().to_string()))
            };
            let blok: Option<(Vec<String>, String)> = if let Some((k, govde)) = baslik("if ") {
                zincir.insert(g, vec![k.clone()]);
                Some((vec![k], govde))
            } else if let Some((k, govde)) = baslik("elif ") {
                let onceki = zincir.get(&g).cloned();
                let mut v: Vec<String> = onceki.iter().flatten().map(|p| format!("not ({p})")).collect();
                if onceki.is_none() {
                    v.push(BELIRSIZ_KOSUL.into());
                }
                v.push(k.clone());
                zincir.entry(g).or_default().push(k);
                Some((v, govde))
            } else if let Some((_, govde)) = baslik("else") {
                let v = match zincir.remove(&g) {
                    Some(onceki) => onceki.iter().map(|p| format!("not ({p})")).collect(),
                    // try / for / while bloğunun else'i
                    None => vec![BELIRSIZ_KOSUL.into()],
                };
                Some((v, govde))
            } else if let Some((_, govde)) = ["try", "finally", "with ", "async with "].iter().find_map(|o| baslik(o)) {
                Some((vec![], govde))
            } else if let Some((_, govde)) = ["except", "for ", "async for ", "while ", "match ", "case "].iter().find_map(|o| baslik(o)) {
                Some((vec![BELIRSIZ_KOSUL.into()], govde))
            } else if ["def ", "async def ", "class "].iter().any(|o| metin.starts_with(o)) {
                // İç fonksiyon ve sınıf gövdesi ara katmanın akışında çalışmaz
                Some((vec!["False".into()], String::new()))
            } else {
                None
            };
            if let Some((kosullar, govde)) = blok {
                yigin.push((g, kosullar));
                if !govde.is_empty() {
                    satirlar.insert(i, (g + 1, govde));
                }
                continue;
            }
            let kosullar: Vec<String> = yigin.iter().flat_map(|(_, k)| k.clone()).collect();
            if let Some(x) = metin.strip_prefix("return ").or_else(|| metin.strip_prefix("raise ")) {
                let x = x.trim().trim_start_matches("await ").trim();
                let tur = if RED_ISARETLERI.iter().any(|r| x.contains(r)) {
                    Some(OlayTuru::Red)
                } else if metin.starts_with("raise") {
                    None
                } else if x.starts_with("call_next(") {
                    Some(OlayTuru::Gecis)
                } else if x.contains("call_next") {
                    Some(OlayTuru::Devir)
                } else {
                    None
                };
                if let Some(tur) = tur {
                    a.olaylar.push(AraOlay { kosullar, tur });
                }
                continue;
            }
            // Atama: yerel takma adlar ve işleyiciye geçiş (`yanit = await call_next(request)`)
            let Some((sol, sag)) = metin.split_once('=').filter(|(s, d)| !s.contains(['(', '[', '!', '<', '>', '=']) && !d.starts_with('=')) else { continue };
            let sol = sol.split(':').next().unwrap_or("").trim().to_string();
            let sag = sag.trim().trim_start_matches("await ").trim();
            if sag.starts_with("call_next(") {
                a.olaylar.push(AraOlay { kosullar, tur: OlayTuru::Gecis });
                continue;
            }
            if sol.is_empty() || !sol.chars().all(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let bg = a.baglam();
            if bg.yol_ifadesi(sag) {
                a.yol_adlari.push(sol);
            } else if bg.yontem_ifadesi(sag) {
                a.yontem_adlari.push(sol);
            } else if self.kimlik_iceriyor(&bg, sag) {
                a.kimlik_adlari.push(sol);
            }
        }
        let bg = a.baglam();
        let denetim = a.olaylar.iter().any(|o| o.tur == OlayTuru::Red && o.kosullar.iter().any(|k| self.kimlik_iceriyor(&bg, k)));
        denetim.then_some(a)
    }

    /// İfadede bir kimlik bilgisi okunuyor mu: başlık, çerez ya da sorgu (adı anahtar / belirteç / oturum taşıyan) ya da
    /// böyle okunmuş bir yerel ad
    fn kimlik_iceriyor(&self, bg: &AraBaglam, s: &str) -> bool {
        if bg.kimlik_adlari.iter().any(|a| crate::kural::derle(&format!(r"\b{}\b", regex::escape(a))).is_match(s)) {
            return true;
        }
        if bg.istek.is_empty() {
            return false;
        }
        let okuma = crate::kural::derle(&format!(r"\b{}\.(headers|cookies|query_params)\s*(?:\.get\(\s*([^,)]+)|\[\s*([^\]]+)\])", regex::escape(bg.istek)));
        let kimlik = okuma.captures_iter(s).any(|c| {
            let anahtar = c.get(2).or(c.get(3)).map(|m| m.as_str().trim()).unwrap_or("");
            let ad = metin_sabiti(anahtar).or_else(|| self.deger(bg.di, anahtar, 0)).unwrap_or_else(|| anahtar.to_string()).to_ascii_lowercase();
            KIMLIK_ADLARI.iter().any(|k| ad.contains(k))
        });
        kimlik
    }

    /// Koşul ifadesinin bir işlem (yöntem, FastAPI yol şablonu) için değeri
    fn kosul_degeri(&self, bg: &AraBaglam, s: &str, yontem: &str, sablon: &str, derinlik: usize) -> Deger {
        let s = dis_parantez(s);
        if s == BELIRSIZ_KOSUL || derinlik > 4 {
            return Deger::Belirsiz;
        }
        let parcalar = sozcukle_bol(s, "or");
        if parcalar.len() > 1 {
            return parcalar.iter().map(|p| self.kosul_degeri(bg, p, yontem, sablon, derinlik)).fold(Deger::Yanlis, Deger::veya);
        }
        let parcalar = sozcukle_bol(s, "and");
        if parcalar.len() > 1 {
            return parcalar.iter().map(|p| self.kosul_degeri(bg, p, yontem, sablon, derinlik)).fold(Deger::Dogru, Deger::ve);
        }
        if let Some(x) = s.strip_prefix("not ").or_else(|| s.strip_prefix("not(").map(|_| &s[3..])) {
            return self.kosul_degeri(bg, x, yontem, sablon, derinlik).degil();
        }
        match s {
            "True" => return Deger::Dogru,
            "False" => return Deger::Yanlis,
            _ => {}
        }
        if self.kimlik_iceriyor(bg, s) {
            return Deger::Kimlik;
        }
        // Karşılaştırma ve üyelik: yöntem ya da yol
        if let Some(c) = crate::kural::derle(r"^(.+?)\s+(not\s+in|in|==|!=)\s+(.+)$").captures(s) {
            let (sol, sag) = (c[1].trim(), c[3].trim());
            let isl = c[2].split_whitespace().collect::<Vec<_>>().join(" ");
            let liste = if isl.ends_with("in") { self.metinler(bg.di, sag, 0) } else { metin_sabiti(sag).map(|m| vec![m]) };
            let Some(liste) = liste else { return Deger::Belirsiz };
            let d = if bg.yontem_ifadesi(sol) {
                if liste.iter().any(|m| m.eq_ignore_ascii_case(yontem)) {
                    Deger::Dogru
                } else {
                    Deger::Yanlis
                }
            } else if bg.yol_ifadesi(sol) {
                liste.iter().map(|m| yol_esit(sablon, m)).fold(Deger::Yanlis, Deger::veya)
            } else {
                return Deger::Belirsiz;
            };
            return if isl == "not in" || isl == "!=" { d.degil() } else { d };
        }
        if let Some(c) = crate::kural::derle(r"^(.+?)\.startswith\((.+)\)$").captures(s) {
            if !bg.yol_ifadesi(&c[1]) {
                return Deger::Belirsiz;
            }
            let ilk = ust_duzey_bol(&c[2]).into_iter().next().unwrap_or_default();
            return match self.metinler(bg.di, &ilk, 0) {
                Some(l) => l.iter().map(|o| yol_baslar(sablon, o)).fold(Deger::Yanlis, Deger::veya),
                None => Deger::Belirsiz,
            };
        }
        // Tek `return`lük proje fonksiyonu: argümanlar parametrelere bağlanır, dönüş ifadesi değerlendirilir
        if let Some(c) = crate::kural::derle(r"^([A-Za-z_][\w.]*)\((.*)\)$").captures(s) {
            let Some((fdi, f)) = self.ifade(bg.di, &c[1]).and_then(|y| self.fonksiyon(&y)) else { return Deger::Belirsiz };
            let satirlar = mantiksal_satirlar(&f.govde_metni, f.govde_sutunu);
            let deyimler: Vec<&String> = satirlar.iter().map(|(_, m)| m).filter(|m| m.as_str() != "\"\"").collect();
            let [donus] = deyimler.as_slice() else { return Deger::Belirsiz };
            let Some(ifade) = donus.strip_prefix("return ") else { return Deger::Belirsiz };
            let mut istek_adi = String::new();
            let (mut yol_adlari, mut yontem_adlari) = (vec![], vec![]);
            for (p, arg) in f.parametreler.iter().zip(ust_duzey_bol(&c[2])) {
                let arg = arg.trim();
                if arg == bg.istek {
                    istek_adi = p.ad.clone();
                } else if bg.yol_ifadesi(arg) {
                    yol_adlari.push(p.ad.clone());
                } else if bg.yontem_ifadesi(arg) {
                    yontem_adlari.push(p.ad.clone());
                }
            }
            let ic = AraBaglam { di: fdi, istek: &istek_adi, yol_adlari, yontem_adlari, kimlik_adlari: vec![] };
            return self.kosul_degeri(&ic, ifade, yontem, sablon, derinlik + 1);
        }
        Deger::Belirsiz
    }
}

impl AraKatman {
    fn baglam(&self) -> AraBaglam<'_> {
        AraBaglam { di: self.di, istek: &self.istek, yol_adlari: self.yol_adlari.clone(), yontem_adlari: self.yontem_adlari.clone(), kimlik_adlari: self.kimlik_adlari.clone() }
    }

    /// Ara katmanın bir işleme etkisi. Olaylar program sırasıyla gezilir: koşulu doğru olan ilk geçiş ya da devir isteği
    /// denetimden önce bırakır; koşulu kimlik bilgisine bağlı red denetimin kendisidir. Koşulu belirsiz bir geçiş ya da
    /// devirden sonra gelen denetim kesinleşemez
    fn sonuc(&self, b: &Baglam, yontem: &str, sablon: &str) -> AraSonuc {
        let bg = self.baglam();
        let (mut belirsiz_gecis, mut belirsiz_devir) = (false, false);
        for o in &self.olaylar {
            let d = o.kosullar.iter().fold(Deger::Dogru, |a, k| a.ve(b.kosul_degeri(&bg, k, yontem, sablon, 0)));
            match (o.tur, d) {
                (OlayTuru::Red, Deger::Kimlik) => return if belirsiz_gecis || belirsiz_devir { AraSonuc::Bilinmiyor } else { AraSonuc::Gerekli },
                (OlayTuru::Red, Deger::Belirsiz) if o.kosullar.iter().any(|k| b.kimlik_iceriyor(&bg, k)) => return AraSonuc::Bilinmiyor,
                (OlayTuru::Red, _) | (_, Deger::Yanlis) => {}
                (OlayTuru::Gecis, Deger::Dogru) => return if belirsiz_devir { AraSonuc::Bilinmiyor } else { AraSonuc::Degil },
                (OlayTuru::Devir, Deger::Dogru) => return AraSonuc::Bilinmiyor,
                (OlayTuru::Gecis, _) => belirsiz_gecis = true,
                (OlayTuru::Devir, _) => belirsiz_devir = true,
            }
        }
        if belirsiz_devir {
            AraSonuc::Bilinmiyor
        } else {
            AraSonuc::Degil
        }
    }
}

/// Bağımlılık zincirinin kararına ara katmanların etkisi: (giriş, kanıt, eklenecek guard adları). Kesin karar
/// değişmez. Kimlik denetimi kapsıyorsa giriş gerekli (tahmin); kapsam çözülemiyorsa girişsiz ya da isteğe bağlı karar
/// "bilinmiyor" olur
fn ara_katman_karari(giris: &'static str, kanit: &'static str, sonuclar: &[(&str, AraSonuc)]) -> (&'static str, &'static str, Vec<String>) {
    let adlar = |s: AraSonuc| -> Vec<String> { sonuclar.iter().filter(|(_, x)| *x == s).map(|(a, _)| format!("{a} (ara katman)")).collect() };
    if kanit == "kesin" {
        return (giris, kanit, vec![]);
    }
    let gerekli = adlar(AraSonuc::Gerekli);
    if !gerekli.is_empty() {
        return ("gerekli", if kanit == "varsayim_bilinmeyen_guard" { kanit } else { "varsayim" }, gerekli);
    }
    let bilinmiyor = adlar(AraSonuc::Bilinmiyor);
    if !bilinmiyor.is_empty() && giris != "gerekli" {
        return ("bilinmiyor", "varsayim_bilinmeyen_guard", bilinmiyor);
    }
    (giris, kanit, vec![])
}

/// Fonksiyonun tanımlandığı fabrikalar, içten dışa (Python kapanışı: ad önce en içteki fonksiyonda aranır)
fn kapsam_zinciri(d: &Dosya, f: &Fonksiyon) -> Vec<String> {
    let mut kapsamlar = vec![];
    let mut k = f.kapsam.clone();
    while let Some(ad) = k.filter(|_| kapsamlar.len() < 16) {
        k = d.fonksiyonlar.iter().find(|g| g.ad == ad).and_then(|g| g.kapsam.clone());
        kapsamlar.push(ad);
    }
    kapsamlar
}
