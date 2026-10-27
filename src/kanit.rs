//! Kanıt ve güven modeli. Üç katman: kanıt (değişmez gözlem: kaynak türü, dayanak dosyası ve özeti, konum),
//! önerme (bir varlık hakkındaki iddia: varlık + yüklem + değer + hesaplanan güven, geçerlilik ve kapsam) ve bağ (kanıtın
//! önermeyi desteklemesi ya da onunla çelişmesi). Kanıt ve önerme `.sezgi/kanit.json`'da durur (veritabanı yok);
//! `graf.json` aynı önermelerin düğüm/kenar görünümüdür (`graf`).
//!
//! Güven kuralları: ölçek `certain > observed > inferred > unknown`. Kanıt katkısı kaynak türünün tavanı
//! ile kaynağın beyanının küçüğüdür; beyan yoksa en zayıf değer (inferred). Önerme güveni geçerli destekleyen kanıtların
//! en yükseğidir: kanıt sayısı düzeyi yükseltmez. Olgu onayı (USER_CONFIRMED) aynı önermede kaynak kodu ya da API şeması
//! kanıtı yoksa en fazla observed. Başka türden bir kaynağın geçerli çelişen kanıtı varsa en fazla inferred. Kanıtı
//! olmayan önerme unknown'dur: yokluk iddia edilmez. Hesap her yüklemede kanıtlardan yeniden yapılır; dosyaya yazılmış
//! güvene inanılmaz.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// kanit.json ve graf.json biçim sürümü
pub const BICIM: u64 = 1;
pub const DOSYA: &str = "kanit.json";
/// Web adaptörünün ön yüz bağları (ekran, eylem, istemci çağrısı); varsa grafa bağlanır
pub const WEB_DOSYA: &str = "web-baglari.json";

/// Tek güven ölçeği. Sıra: Unknown < Inferred < Observed < Certain
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Guven {
    Unknown,
    Inferred,
    Observed,
    Certain,
}

impl Guven {
    /// Ajana ve ekrana giden damga (tani'deki [Kesin]/[Yorum]/[Bilinmiyor] ile aynı sözcükler)
    pub fn etiket(self, en: bool) -> &'static str {
        match (self, en) {
            (Guven::Certain, false) => "[Kesin]",
            (Guven::Observed, false) => "[Gözlem]",
            (Guven::Inferred, false) => "[Yorum]",
            (Guven::Unknown, false) => "[Bilinmiyor]",
            (Guven::Certain, true) => "[Certain]",
            (Guven::Observed, true) => "[Observed]",
            (Guven::Inferred, true) => "[Inferred]",
            (Guven::Unknown, true) => "[Unknown]",
        }
    }
    pub fn ad(self) -> &'static str {
        match self {
            Guven::Certain => "certain",
            Guven::Observed => "observed",
            Guven::Inferred => "inferred",
            Guven::Unknown => "unknown",
        }
    }
}

/// Kanıt kaynak türleri (kanonik adlar)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KaynakTuru {
    SourceCode,
    Ast,
    ApiSchema,
    DatabaseSchema,
    BinaryMetadata,
    BinaryHeuristic,
    Decompiled,
    BrowserObservation,
    NetworkCapture,
    RuntimeTrace,
    AutomatedTest,
    UserConfirmed,
}

impl KaynakTuru {
    /// Türün izin verdiği en yüksek düzey. USER_CONFIRMED'in kaynak kanıtı şartı önerme düzeyinde uygulanır (`hesap_et`)
    pub fn tavan(self) -> Guven {
        match self {
            KaynakTuru::SourceCode | KaynakTuru::Ast | KaynakTuru::ApiSchema | KaynakTuru::DatabaseSchema | KaynakTuru::UserConfirmed => Guven::Certain,
            KaynakTuru::BinaryMetadata | KaynakTuru::BrowserObservation | KaynakTuru::NetworkCapture | KaynakTuru::RuntimeTrace | KaynakTuru::AutomatedTest => Guven::Observed,
            KaynakTuru::BinaryHeuristic | KaynakTuru::Decompiled => Guven::Inferred,
        }
    }
    pub fn ad(self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
    }
}

/// Kanıtın geçerliliğinin bağlı olduğu içerik: uygulama köküne göre yol ve SHA-256 özeti
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Dayanak {
    pub yol: String,
    pub sha256: String,
}

fn dogru() -> bool {
    true
}

/// Değişmez gözlem. Kimlik içerikten (zaman hariç) hesaplanır: aynı kod aynı kanıtı verir
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Kanit {
    pub id: String,
    pub kaynak_turu: KaynakTuru,
    pub uretici: String,
    #[serde(default)]
    pub dayanak: Vec<Dayanak>,
    /// "dosya:satır"
    #[serde(default)]
    pub konum: Vec<String>,
    #[serde(default)]
    pub zaman: String,
    /// Kaynağın kendi beyanı (ör. adaptörün `giris_kaniti`); yoksa en zayıf değer sayılır
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beyan: Option<Guven>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sinirlamalar: Vec<String>,
    /// Değer taşımayan projeksiyon (kanıtın ne olduğu; uygulamanın verisi değil)
    #[serde(default)]
    pub ozet: Value,
    /// Hesaplanır: dayanak özetleri bugün de aynı mı
    #[serde(default = "dogru")]
    pub gecerli: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eski_nedeni: Option<String>,
}

impl Kanit {
    pub fn yeni(kaynak_turu: KaynakTuru, uretici: &str, dayanak: Vec<Dayanak>, konum: Vec<String>, beyan: Option<Guven>) -> Kanit {
        let mut k = Kanit {
            id: String::new(),
            kaynak_turu,
            uretici: uretici.to_string(),
            dayanak,
            konum,
            zaman: String::new(),
            beyan,
            sinirlamalar: vec![],
            ozet: Value::Null,
            gecerli: true,
            eski_nedeni: None,
        };
        k.id = Kanit::kimlik_hesapla(&k);
        k
    }

    /// Kaydın kanonik içerik özeti (zaman ve hesaplanan alanlar hariç)
    pub fn kimlik_hesapla(k: &Kanit) -> String {
        let icerik = json!([k.kaynak_turu, k.uretici, k.dayanak, k.konum, k.beyan, k.sinirlamalar, k.ozet]);
        format!("kn_{}", &sha256_hex(icerik.to_string().as_bytes())[..24])
    }

    /// Katkı: tür tavanı ile beyanın küçüğü; beyan yoksa inferred
    pub fn katki(&self) -> Guven {
        self.kaynak_turu.tavan().min(self.beyan.unwrap_or(Guven::Inferred))
    }

    pub fn ozetli(mut self, ozet: Value, sinirlamalar: Vec<String>) -> Kanit {
        self.ozet = ozet;
        self.sinirlamalar = sinirlamalar;
        self.id = Kanit::kimlik_hesapla(&self);
        self
    }
}

/// Önermenin kapsamı: kapalı dünya (öğe listesinin tamamı kanıtlı), açık (listelenenler kanıtlı ama liste eksik
/// olabilir), bilinmiyor (bu alan için hiçbir kaynak çalışmadı)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kapsam {
    #[serde(rename = "kapali")]
    KapaliDunya,
    #[serde(rename = "acik")]
    Acik,
    #[serde(rename = "bilinmiyor")]
    Bilinmiyor,
}

impl Kapsam {
    pub fn metin(self, en: bool) -> &'static str {
        match (self, en) {
            (Kapsam::KapaliDunya, false) => "kapalı",
            (Kapsam::Acik, false) => "açık",
            (Kapsam::Bilinmiyor, false) => "kapsamı bilinmiyor",
            (Kapsam::KapaliDunya, true) => "closed",
            (Kapsam::Acik, true) => "open",
            (Kapsam::Bilinmiyor, true) => "coverage unknown",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Gecerlilik {
    Gecerli,
    Eski,
    Belirsiz,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Yon {
    Destek,
    Celiski,
}

/// Kanıt ile önerme arasındaki bağ. Kanıtın bu önerme için ileri sürdüğü değer ve kapsam da bağdadır: yön (destek ya da
/// çelişki) önermenin seçilen değerine göre hesaplanır
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bag {
    pub kanit: String,
    pub onerme: String,
    pub yon: Yon,
    #[serde(default)]
    pub deger: Value,
    pub kapsam: Kapsam,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kapsam_disi: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hesap {
    pub guven: Guven,
    pub gecerlilik: Gecerlilik,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neden: Option<String>,
    pub kapsam: Kapsam,
    #[serde(default)]
    pub destek: Vec<String>,
    #[serde(default)]
    pub celiski: Vec<String>,
    #[serde(default)]
    pub kapsam_disi: Vec<String>,
    /// Hesabın son değiştiği kanıt deposu revizyonu
    #[serde(default)]
    pub revizyon: u64,
}

impl Default for Hesap {
    fn default() -> Hesap {
        Hesap { guven: Guven::Unknown, gecerlilik: Gecerlilik::Gecerli, neden: None, kapsam: Kapsam::Bilinmiyor, destek: vec![], celiski: vec![], kapsam_disi: vec![], revizyon: 0 }
    }
}

/// Bir varlık hakkındaki iddia. Kimlik = (varlık, yüklem, hedef). Yüklem bir ilişki adıysa (CALLS, READS, ...)
/// önerme grafikte `varlik -> hedef` kenarıdır; değilse varlığın niteliğidir (VAR, GIRIS, ROLLER, GOVDE, SORGU, VERI)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Onerme {
    pub id: String,
    pub varlik: String,
    pub yuklem: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hedef: Option<String>,
    #[serde(default)]
    pub deger: Value,
    /// Üreticinin sabit notları (ör. "girdi tanımı bulunamadı"): kanıt olmasa da kapsam dışında görünür
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notlar: Vec<String>,
    #[serde(default)]
    pub hesap: Hesap,
}

/// Kurucuya verilen iddia: bir kanıtın bir önerme için söylediği
#[derive(Clone, Debug)]
pub struct Iddia {
    pub varlik: String,
    pub yuklem: String,
    pub hedef: Option<String>,
    pub deger: Value,
    pub kapsam: Kapsam,
    pub kapsam_disi: Vec<String>,
}

impl Iddia {
    pub fn yeni(varlik: &str, yuklem: &str, hedef: Option<&str>, deger: Value, kapsam: Kapsam) -> Iddia {
        Iddia { varlik: varlik.to_string(), yuklem: yuklem.to_string(), hedef: hedef.map(String::from), deger, kapsam, kapsam_disi: vec![] }
    }
    pub fn notlu(mut self, notlar: Vec<String>) -> Iddia {
        self.kapsam_disi = notlar;
        self
    }
}

pub fn onerme_kimligi(varlik: &str, yuklem: &str, hedef: Option<&str>) -> String {
    format!("on_{}", &sha256_hex(format!("{varlik}\u{0}{yuklem}\u{0}{}", hedef.unwrap_or("")).as_bytes())[..24])
}

/// Kanıt deposu (kanit.json)
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Depo {
    pub bicim: u64,
    pub revizyon: u64,
    #[serde(default)]
    pub uretici: String,
    #[serde(default)]
    pub zaman: String,
    /// Uygulama kökü, kanit.json'un klasörüne göre (ör. ".."): dayanak yolları buna görelidir. Çıktı uygulamanın
    /// dışındaysa yazılmaz (mutlak yol dosyaya girmez) ve geçerlilik izlenmez
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kok: Option<String>,
    /// Adaptörün haritaya yazılmayan yan bilgisi (modül grafiği, router içerme grafiği, tablo modelleri): depo yalnızca
    /// haritadan yeniden kurulurken (`sezgi gozlem al`) bu bilgi buradan okunur
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub ek: Value,
    #[serde(default)]
    pub kanitlar: Vec<Kanit>,
    #[serde(default)]
    pub onermeler: Vec<Onerme>,
    #[serde(default)]
    pub baglar: Vec<Bag>,
}

/// En fazla inferred olan ilişkiler: olası etki ve yönü belirsiz başvuru. Şemada açıkça tanımlı yabancı anahtar
/// (`REFERENCES` değerinde `bildirilmis: true`) yönü ve sütunları belli bir olgudur: kanıtının tavanını alır
const EN_FAZLA_YORUM: [&str; 2] = ["MAY_AFFECT", "REFERENCES"];

/// Önermeye bağlı bir kanıt: (bağın sırası, bağ, kanıt)
type BagKanit<'a> = (usize, &'a Bag, &'a Kanit);
/// Bir önermenin hesabı: (değer, hesap, bağ yönleri)
type Sonuc = (Value, Hesap, Vec<(usize, Yon)>);

fn hesap_et(o: &Onerme, baglar: &[BagKanit]) -> Sonuc {
    let mut h = Hesap { revizyon: o.hesap.revizyon, ..Hesap::default() };
    let gecerli: Vec<&BagKanit> = baglar.iter().filter(|(_, _, k)| k.gecerli).collect();
    let mut kapsam_disi: Vec<String> = o.notlar.clone();
    if gecerli.is_empty() {
        // Destek düşen önerme silinmez: unknown olur ve neden kapsam dışında görünür
        if let Some((_, _, k)) = baglar.first() {
            let neden = k.eski_nedeni.clone().unwrap_or_else(|| "dayanak değişti".into());
            kapsam_disi.push(format!("{neden}; `sezgi init` ile yenileyin"));
            h.gecerlilik = Gecerlilik::Eski;
            h.neden = Some(neden);
        }
        h.kapsam_disi = kapsam_disi;
        return (o.deger.clone(), h, vec![]);
    }
    // Değer en güçlü kanıttan (eşitlikte ilk); ondan farklı değer söyleyen, başka türden bir kaynağın (kod ↔ ağ kaydı ↔
    // veri şeması ↔ insan onayı) geçerli kanıtı çelişkidir. Aynı kaynak türünün farklı değerleri (iki işlemde farklı
    // zorunlulukla kullanılan aynı girdi adı, aynı tabloyu değiştiren iki göç) bağlama göre değişen olgudur: çelişki
    // sayılmaz, kapsam dışında not edilir
    let mut en_iyi = gecerli[0];
    for g in &gecerli {
        if g.2.katki() > en_iyi.2.katki() {
            en_iyi = g;
        }
    }
    let deger = en_iyi.1.deger.clone();
    let (destek, farkli): (Vec<&BagKanit>, Vec<&BagKanit>) = gecerli.iter().partition(|(_, b, _)| b.deger == deger);
    let (celiski, baglam): (Vec<&BagKanit>, Vec<&BagKanit>) = farkli.into_iter().partition(|(_, _, k)| k.kaynak_turu != en_iyi.2.kaynak_turu);
    if !baglam.is_empty() {
        kapsam_disi.push(format!("aynı kaynak türünden {} kanıt başka bir değer söylüyor (bağlama göre değişir; çelişki sayılmaz)", baglam.len()));
    }
    let kaynakli = destek.iter().any(|(_, _, k)| matches!(k.kaynak_turu, KaynakTuru::SourceCode | KaynakTuru::ApiSchema));
    let mut guven = destek
        .iter()
        .map(|(_, _, k)| if k.kaynak_turu == KaynakTuru::UserConfirmed && !kaynakli { k.katki().min(Guven::Observed) } else { k.katki() })
        .max()
        .unwrap_or(Guven::Unknown);
    let bildirilmis_fk = o.yuklem == "REFERENCES" && deger["bildirilmis"] == true;
    if (EN_FAZLA_YORUM.contains(&o.yuklem.as_str()) && !bildirilmis_fk) || !celiski.is_empty() {
        guven = guven.min(Guven::Inferred);
    }
    h.kapsam = en_iyi.1.kapsam;
    if !celiski.is_empty() && h.kapsam == Kapsam::KapaliDunya {
        h.kapsam = Kapsam::Acik;
    }
    for (_, b, _) in &destek {
        for n in &b.kapsam_disi {
            if !kapsam_disi.contains(n) {
                kapsam_disi.push(n.clone());
            }
        }
    }
    let eski = baglar.len() - gecerli.len();
    if eski > 0 {
        kapsam_disi.push(format!("{eski} kanıt eski (dayanağı değişti)"));
    }
    h.guven = guven;
    h.destek = destek.iter().map(|(_, _, k)| k.id.clone()).collect();
    h.celiski = celiski.iter().map(|(_, _, k)| k.id.clone()).collect();
    h.kapsam_disi = kapsam_disi;
    let yonler = destek.iter().map(|(i, _, _)| (*i, Yon::Destek)).chain(celiski.iter().map(|(i, _, _)| (*i, Yon::Celiski))).collect();
    (deger, h, yonler)
}

impl Depo {
    /// Bütün önermelerin hesabını kanıtlardan yeniden yapar (revizyon korunur)
    pub fn hesapla(&mut self) {
        let kanit_sira: HashMap<&str, usize> = self.kanitlar.iter().enumerate().map(|(i, k)| (k.id.as_str(), i)).collect();
        let mut gruplar: HashMap<&str, Vec<BagKanit>> = HashMap::new();
        for (i, b) in self.baglar.iter().enumerate() {
            if let Some(&ki) = kanit_sira.get(b.kanit.as_str()) {
                gruplar.entry(b.onerme.as_str()).or_default().push((i, b, &self.kanitlar[ki]));
            }
        }
        let sonuclar: Vec<Sonuc> = self.onermeler.iter().map(|o| hesap_et(o, gruplar.get(o.id.as_str()).map(Vec::as_slice).unwrap_or(&[]))).collect();
        for (o, (deger, hesap, yonler)) in self.onermeler.iter_mut().zip(sonuclar) {
            o.deger = deger;
            o.hesap = hesap;
            for (i, y) in yonler {
                self.baglar[i].yon = y;
            }
        }
    }

    /// Dayanak özetlerini bugünkü içerikle karşılaştırır ve kanıtları geçerli/eski işaretler. `guncel(yol)`: dosyanın
    /// bugünkü SHA-256 özeti (okunamıyorsa None). Bir kanıtın durumu değiştiyse true
    pub fn gecerlilik_isaretle(&mut self, guncel: &mut dyn FnMut(&str) -> Option<String>) -> bool {
        let mut degisti = false;
        for k in &mut self.kanitlar {
            let mut neden = None;
            for d in &k.dayanak {
                match guncel(&d.yol) {
                    Some(s) if s == d.sha256 => {}
                    Some(_) => {
                        neden = Some(format!("dayanak değişti: {}", d.yol));
                        break;
                    }
                    None => {
                        neden = Some(format!("dayanak okunamıyor: {}", d.yol));
                        break;
                    }
                }
            }
            if k.gecerli != neden.is_none() || k.eski_nedeni != neden {
                k.gecerli = neden.is_none();
                k.eski_nedeni = neden;
                degisti = true;
            }
        }
        degisti
    }

    /// Geçerliliği uygular; bir kanıtın durumu değiştiyse önermeler yeniden hesaplanır. Keşif adımının dosya özetleri (aynı
    /// algoritma: SHA-256, uygulama köküne göre `/` ayraçlı yol) doğrudan `guncel` olarak verilebilir
    pub fn gecerlilik_uygula(&mut self, guncel: &mut dyn FnMut(&str) -> Option<String>) -> bool {
        let degisti = self.gecerlilik_isaretle(guncel);
        if degisti {
            self.hesapla();
        }
        degisti
    }

    /// Yeni deponun revizyonu öncekinin bir fazlası; hesabı değişmeyen önermenin revizyonu korunur
    pub fn revizyon_ata(&mut self, onceki: Option<&Depo>) {
        self.revizyon = onceki.map(|o| o.revizyon + 1).unwrap_or(1);
        let eski: HashMap<&str, &Onerme> = onceki.map(|o| o.onermeler.iter().map(|x| (x.id.as_str(), x)).collect()).unwrap_or_default();
        for o in &mut self.onermeler {
            o.hesap.revizyon = match eski.get(o.id.as_str()) {
                Some(e) if e.deger == o.deger && Hesap { revizyon: o.hesap.revizyon, ..e.hesap.clone() } == o.hesap => e.hesap.revizyon,
                _ => self.revizyon,
            };
        }
    }

    pub fn onerme(&self, varlik: &str, yuklem: &str, hedef: Option<&str>) -> Option<&Onerme> {
        let id = onerme_kimligi(varlik, yuklem, hedef);
        self.onermeler.iter().find(|o| o.id == id)
    }
}

/// Depoyu kanıt kanıt kurar: aynı önermeye giden iddialar tek önermede birleşir
#[derive(Default)]
pub struct Kurucu {
    depo: Depo,
    onerme_sira: HashMap<String, usize>,
    kanit_var: HashSet<String>,
    /// (kanıt, önerme) çiftleri: aynı kanıt aynı önermeye bir kez bağlanır
    bag_var: HashSet<(String, String)>,
}

impl Kurucu {
    fn onerme_al(&mut self, i: &Iddia) -> usize {
        let id = onerme_kimligi(&i.varlik, &i.yuklem, i.hedef.as_deref());
        if let Some(&s) = self.onerme_sira.get(&id) {
            return s;
        }
        self.depo.onermeler.push(Onerme {
            id: id.clone(),
            varlik: i.varlik.clone(),
            yuklem: i.yuklem.clone(),
            hedef: i.hedef.clone(),
            deger: i.deger.clone(),
            notlar: vec![],
            hesap: Hesap::default(),
        });
        self.onerme_sira.insert(id, self.depo.onermeler.len() - 1);
        self.depo.onermeler.len() - 1
    }

    /// Bir kanıt ve onun desteklediği iddialar (aynı içerikli kanıt bir kez saklanır)
    pub fn ekle(&mut self, kanit: Kanit, iddialar: Vec<Iddia>) {
        for i in &iddialar {
            let s = self.onerme_al(i);
            let onerme = self.depo.onermeler[s].id.clone();
            if !self.bag_var.insert((kanit.id.clone(), onerme.clone())) {
                continue;
            }
            self.depo.baglar.push(Bag { kanit: kanit.id.clone(), onerme, yon: Yon::Destek, deger: i.deger.clone(), kapsam: i.kapsam, kapsam_disi: i.kapsam_disi.clone() });
        }
        if self.kanit_var.insert(kanit.id.clone()) {
            self.depo.kanitlar.push(kanit);
        }
    }

    /// Kanıtı olmayan önerme (ör. çözülemeyen girdi tanımı): unknown, notuyla
    pub fn bos(&mut self, iddia: Iddia, not: &str) {
        let s = self.onerme_al(&iddia);
        if !self.depo.onermeler[s].notlar.iter().any(|n| n == not) {
            self.depo.onermeler[s].notlar.push(not.to_string());
        }
    }

    pub fn bitir(mut self, zaman: &str) -> Depo {
        self.depo.bicim = BICIM;
        self.depo.zaman = zaman.to_string();
        for k in &mut self.depo.kanitlar {
            k.zaman = zaman.to_string();
        }
        self.depo.hesapla();
        self.depo
    }
}

/// SHA-256 (onaltılık): kart, keşif ve kanıt deposu tek kaynaktan
pub fn sha256_hex(b: &[u8]) -> String {
    crate::benlik::sha256_hex(b)
}

/// Bir dosyanın bugünkü içerik özeti: keşfin `okunan_dosyalar` özetiyle aynı fonksiyon (sembolik bağ izlenmez, 4 MiB
/// sınırı)
pub fn dosya_ozeti(p: &Path) -> Option<String> {
    crate::guvenli::oku_bayt(p).map(|b| crate::kesif::icerik_ozeti(&b)).or_else(|| buyuk_dosya_ozeti(p))
}

/// 4 MiB'tan büyük düzenli dosyanın (ör. kaynaksız uygulamanın ikilisi) SHA-256 özeti, belleğe alınmadan; sembolik
/// bağ izlenmez
fn buyuk_dosya_ozeti(p: &Path) -> Option<String> {
    use std::io::Read;
    let m = std::fs::symlink_metadata(p).ok()?;
    if !m.file_type().is_file() || m.len() <= crate::guvenli::EN_BUYUK_DOSYA {
        return None;
    }
    let mut f = std::fs::File::open(p).ok()?;
    let mut c = ring::digest::Context::new(&ring::digest::SHA256);
    let mut b = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut b).ok()?;
        if n == 0 {
            break;
        }
        c.update(&b[..n]);
    }
    Some(c.finish().as_ref().iter().map(|x| format!("{x:02x}")).collect())
}

fn giris_siniri(k: &str) -> String {
    match k {
        "varsayim" => "giriş kararı yokluktan çıkarıldı: giriş guard'ı bulunamadı".into(),
        "varsayim_bilinmeyen_guard" => "tanınmayan bir guard var: giriş kararı çıkarım".into(),
        "varsayim_isaret" => "herkese açık işaretinin guard'ca okunduğu doğrulanamadı".into(),
        x => format!("giriş kararı çıkarım ({x})"),
    }
}

fn metinler(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect()
}

/// Kapsam dışı notlarını kısaltır (kart ve ajan cevabı küçük kalsın)
fn kisalt(mut v: Vec<String>, n: usize) -> Vec<String> {
    if v.len() > n {
        let kalan = v.len() - n;
        v.truncate(n);
        v.push(format!("… {kalan} not daha"));
    }
    v
}

/// Girdi şeması önermesinin değeri (alan sayısı, dizi mi): gözlem karşılaştırması aynı değeri üretir
pub(crate) fn govde_degeri(sema: &Value) -> Value {
    json!({ "alan": sema["alanlar"].as_object().map(|a| a.len()).unwrap_or(0), "dizi": sema["dizi"].as_bool() == Some(true) })
}

/// Adaptör çıktısından (kapilar.json'un içeriği) kanıt deposu. Adaptörün bugünkü alanları kanıta çevrilir:
/// kapı tanımı, `giris_kaniti`, `roller_kaniti`, veri bağları, girdi şemaları, veri şeması dosyaları ve denetlenemeyen
/// notları. Ön yüz bağları (ekran, eylem, istemci çağrısı) web adaptörünün `web-baglari.json` çıktısından (`web`; yoksa
/// Null); ön yüz kanıtlarının dosyaları ön yüz klasörüne görelidir, `web_onek` o klasörün uygulama köküne göre yolu
/// (ör. "../frontend"). `ek`: adaptörün haritaya yazılmayan yan bilgisi (modül grafiği, router içerme grafiği, tablo
/// modelleri; yoksa Null; depoya da yazılır ki depo haritadan yeniden kurulurken kaybolmasın). `kok`: dayanak özetleri
/// için uygulama kökü. Çalışma zamanı gözlemleri (`gozlem`, `gozlem.json`; yoksa None) de kanıta çevrilir: ağ yakalaması
/// kanıtları, kodda bulunamayan gözlenen uçlar, zaman işareti eşlemeleri ve yolculukların gözlenen istek sırası.
/// Kullanıcı yolculukları ön yüz bağlarından kurulur
pub fn uret(harita: &Value, web: &Value, web_onek: &str, ek: &Value, kok: Option<&Path>, zaman: &str, gozlem: Option<&crate::gozlem::Gozlemler>) -> Depo {
    use crate::graf::{api_kimligi, ILISKILER};
    let ozet = &harita["ozet"];
    let uretici = format!("sezgi {} {}", env!("CARGO_PKG_VERSION"), ozet["adaptor"].as_str().unwrap_or("")).trim().to_string();
    // Dayanak özetleri keşfin okuduğu dosyaların özetleridir (`ozet.okunan_dosyalar`): ayrıca okunmaz. Keşfin
    // okumadığı dosyanın dayanağı yoktur (geçerliliği izlenemez, sınırlama olarak yazılır). Bu listeyi taşımayan eski
    // haritada (paylaşım dosyası, eski kurulum) dosyalar burada özetlenir
    let okunan = okunan_ozetleri(ozet);
    let mut ozetler: HashMap<String, Option<String>> = HashMap::new();
    let mut dayanak = |yol: &str| -> Vec<Dayanak> {
        if yol.is_empty() {
            return vec![];
        }
        let o = ozetler
            .entry(yol.to_string())
            .or_insert_with(|| match &okunan {
                Some(m) => m.get(yol).cloned(),
                None => kok.and_then(|k| dosya_ozeti(&k.join(yol))),
            })
            .clone();
        o.map(|sha256| vec![Dayanak { yol: yol.to_string(), sha256 }]).unwrap_or_default()
    };
    let okunamadi = |d: &[Dayanak]| if d.is_empty() { vec!["dayanak dosyası okunmadı: geçerliliği izlenemez".to_string()] } else { vec![] };
    let mut k = Kurucu::default();
    let semalar = metinler(&ozet["veri_semasi"]);
    let mut tablolar: BTreeMap<String, ()> = BTreeMap::new();
    let denetlenemeyen = &ozet["denetlenemeyen"];
    let kapilar = harita["endpointler"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut kimlikler: HashMap<String, String> = HashMap::new();
    for kp in kapilar {
        let api = api_kimligi(kp);
        kimlikler.entry(kp["id"].as_str().unwrap_or("").to_string()).or_insert_with(|| api.clone());
        let dosya = kp["dosya"].as_str().unwrap_or("");
        let konum = format!("{dosya}:{}", kp["satir"].as_u64().unwrap_or(0));
        let d = dayanak(dosya);
        let sin = okunamadi(&d);
        let kaynak = |beyan: Guven| Kanit::yeni(KaynakTuru::SourceCode, &uretici, d.clone(), vec![konum.clone()], Some(beyan));
        // Kapı tanımı ve uygulayan fonksiyon
        let fonk = format!("fn:{dosya}#{}", kp["fonksiyon"].as_str().unwrap_or(""));
        let tanim = json!({ "kapi": kp["id"], "yontem": kp["yontem"], "yol": kp["yol"], "kontrolcu": kp["kontrolcu"], "fonksiyon": kp["fonksiyon"], "surum": kp["surum"], "ana": kp["ana"] });
        k.ekle(
            kaynak(Guven::Certain).ozetli(json!({ "tur": "kapi_tanimi" }), sin.clone()),
            vec![Iddia::yeni(&api, "VAR", None, tanim, Kapsam::KapaliDunya), Iddia::yeni(&api, "CALLS", Some(&fonk), json!({}), Kapsam::KapaliDunya)],
        );
        // Giriş kararı: kesin işaretten certain, yokluktan ya da doğrulanamayan işaretten inferred
        let gk = kp["giris_kaniti"].as_str().unwrap_or("");
        let giris = kp["giris"].as_str().unwrap_or("bilinmiyor");
        if giris == "bilinmiyor" {
            k.bos(Iddia::yeni(&api, "GIRIS", None, Value::Null, Kapsam::Bilinmiyor), "giriş gereksinimi koddan çözülemedi");
        } else {
            let mut s = sin.clone();
            if gk != "kesin" {
                s.insert(0, giris_siniri(gk));
            }
            let beyan = if gk == "kesin" { Guven::Certain } else { Guven::Inferred };
            k.ekle(kaynak(beyan).ozetli(json!({ "tur": "giris_karari", "giris_kaniti": gk }), s), vec![Iddia::yeni(&api, "GIRIS", None, json!(giris), Kapsam::KapaliDunya)]);
        }
        // Roller: rol kararı varsayımsa ya da giriş kararı kesin değilse inferred; varsayımda liste eksik olabilir
        let roller = metinler(&kp["roller"]);
        let rol_varsayim = kp["roller_kaniti"].as_str() == Some("varsayim");
        let rol_kesin = !rol_varsayim && gk == "kesin";
        let mut s = sin.clone();
        if rol_varsayim {
            s.insert(0, "rol değeri tek anlamlı çözülemedi (ya da sınıf ve metot düzeyinde birlikte rol var)".into());
        } else if !rol_kesin {
            s.insert(0, "giriş kararı kesin değil: rol listesi de çıkarım".into());
        }
        let mut iddialar = vec![Iddia::yeni(&api, "ROLLER", None, json!(roller), if rol_varsayim { Kapsam::Acik } else { Kapsam::KapaliDunya })];
        for r in &roller {
            iddialar.push(Iddia::yeni(&api, "REQUIRES_PERMISSION", Some(&format!("role:{r}")), json!({}), Kapsam::KapaliDunya));
        }
        k.ekle(kaynak(if rol_kesin { Guven::Certain } else { Guven::Inferred }).ozetli(json!({ "tur": "rol_karari" }), s), iddialar);
        // Veri bağları: statik çıkarım, en fazla inferred; liste her zaman açık
        match kp["veri"].as_object() {
            Some(v) => {
                let (okur, yazar) = (metinler(&v["okur"]), metinler(&v["yazar"]));
                let mut notlar = vec!["iç içe yazmalar ve kütüphane içi çağrılar izlenmez".to_string()];
                if v.get("eksik_olabilir").and_then(Value::as_bool) == Some(true) {
                    notlar.insert(0, "liste eksik olabilir: ham SQL ya da izleme derinliği aşıldı".into());
                }
                let mut iddialar = vec![Iddia::yeni(&api, "VERI", None, json!({ "okur": okur, "yazar": yazar }), Kapsam::Acik).notlu(notlar)];
                for (iliski, liste) in [("READS", &okur), ("WRITES", &yazar)] {
                    for t in liste {
                        iddialar.push(Iddia::yeni(&api, iliski, Some(&format!("db:{t}")), json!({}), Kapsam::KapaliDunya));
                        tablolar.insert(t.clone(), ());
                        // Şema dosyası yoksa tablonun varlığı da yalnızca sorgudan bilinir
                        if semalar.is_empty() {
                            iddialar.push(Iddia::yeni(&format!("db:{t}"), "VAR", None, json!({ "ad": t }), Kapsam::KapaliDunya));
                        }
                    }
                }
                let mut s = vec!["statik çıkarım: doğrudan model ve sorgu çağrıları".to_string()];
                s.extend(sin.clone());
                k.ekle(kaynak(Guven::Inferred).ozetli(json!({ "tur": "veri_bagi" }), s), iddialar);
            }
            None => k.bos(Iddia::yeni(&api, "VERI", None, Value::Null, Kapsam::Bilinmiyor), "veri bağı çıkarılamadı (desteklenen ORM ya da sorgu bulunamadı)"),
        }
        // Girdi şemaları: önizlemenin denetleyemediği kural ya da zorunluluğu belirsiz alan varsa kapsam açık
        let denet = metinler(&denetlenemeyen[konum.as_str()]);
        let mut denet_eklendi = false;
        for (bolum, yuklem) in [("govde", "GOVDE"), ("sorgu", "SORGU")] {
            for g in kp[bolum].as_array().into_iter().flatten() {
                let sema = &g["sema"];
                if !sema.is_object() {
                    continue;
                }
                let dto = sema["dto"].as_str().unwrap_or("?");
                if sema["bulunamadi"].as_bool() == Some(true) {
                    k.bos(Iddia::yeni(&api, yuklem, Some(dto), Value::Null, Kapsam::Bilinmiyor), &format!("girdi tanımı bulunamadı: {dto}"));
                    continue;
                }
                let alanlar = sema["alanlar"].as_object();
                let mut notlar: Vec<String> = vec![];
                for (ad, a) in alanlar.into_iter().flatten() {
                    if a["belirsiz"].as_bool() == Some(true) {
                        notlar.push(format!("{ad}: zorunluluğu koddan çıkarılamadı"));
                    }
                    let tip = a["tip"].as_str().unwrap_or("");
                    for r in a["kurallar"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                        if crate::kural::coz(r, tip).iter().any(|x| matches!(x, crate::kural::Kural::Bilinmiyor(_))) {
                            notlar.push(format!("{ad}: {r} önizlemede kontrol edilemez"));
                        }
                    }
                }
                if !denet_eklendi {
                    notlar.extend(denet.iter().cloned());
                    denet_eklendi = true;
                }
                let adaylar = sema["adaylar"].as_array().is_some_and(|a| !a.is_empty());
                if adaylar {
                    notlar.push("girdi birkaç biçimden biri olabilir: hangisinin kabul edileceğine uygulama karar verir".into());
                }
                let sdosya = sema["dosya"].as_str().filter(|s| !s.is_empty()).unwrap_or(dosya);
                let satir = alanlar.into_iter().flatten().filter_map(|(_, a)| a["satir"].as_u64()).filter(|s| *s > 0).min().unwrap_or(kp["satir"].as_u64().unwrap_or(0));
                let sd = dayanak(sdosya);
                let s = okunamadi(&sd);
                let kapsam = if notlar.is_empty() { Kapsam::KapaliDunya } else { Kapsam::Acik };
                // Girdi alanları (`field:Dto.alan`) ve doğrulama kuralları (`rule:dosya:satır`, davranış özetindeki
                // kurallarla aynı kimlik biçimi): işlem kuralı uygular, kural alanı doğrular
                for (ad, a) in alanlar.into_iter().flatten() {
                    let alan_satir = a["satir"].as_u64().filter(|s| *s > 0).unwrap_or(satir);
                    let alan_id = format!("field:{dto}.{ad}");
                    let yer = format!("{sdosya}:{alan_satir}");
                    let kurallar = metinler(&a["kurallar"]);
                    let mut iddialar = vec![Iddia::yeni(&alan_id, "VAR", None, json!({ "tip": a["tip"], "zorunlu": a["zorunlu"], "dto": dto }), Kapsam::KapaliDunya)];
                    if !kurallar.is_empty() {
                        let kural_id = format!("rule:{yer}");
                        // Değer yalnızca kurallardır: paylaşılan şema satırı (`PinCodeResetDto` ve `PinCodeChangeDto` aynı `pinCode` kuralı)
                        // birden çok alanı doğrular; alan bağı `VALIDATES` kenarındadır, farklı alanlar çelişki sayılmaz
                        iddialar.push(Iddia::yeni(&kural_id, "KURAL", None, json!({ "kurallar": kurallar }), Kapsam::KapaliDunya));
                        iddialar.push(Iddia::yeni(&kural_id, "VALIDATES", Some(&alan_id), json!({}), Kapsam::KapaliDunya));
                        iddialar.push(Iddia::yeni(&api, "VALIDATES", Some(&kural_id), json!({ "alan": alan_id }), Kapsam::KapaliDunya));
                    }
                    let beyan = if adaylar || a["belirsiz"].as_bool() == Some(true) { Guven::Inferred } else { Guven::Certain };
                    k.ekle(
                        Kanit::yeni(KaynakTuru::SourceCode, &uretici, sd.clone(), vec![yer], Some(beyan)).ozetli(json!({ "tur": "girdi_alani", "dto": dto }), s.clone()),
                        iddialar,
                    );
                }
                let deger = govde_degeri(sema);
                k.ekle(
                    Kanit::yeni(KaynakTuru::SourceCode, &uretici, sd, vec![format!("{sdosya}:{satir}")], Some(if adaylar { Guven::Inferred } else { Guven::Certain }))
                        .ozetli(json!({ "tur": "girdi_semasi", "dto": dto }), s),
                    vec![Iddia::yeni(&api, yuklem, Some(dto), deger, kapsam).notlu(kisalt(notlar, 12))],
                );
            }
        }
        // Davranış özeti (`ozet.dna`): her öğe kendi kaynak satırıyla kanıtlanır; kapsam her zaman açık (bulunmayan red
        // koşulu yok demek değildir)
        dna_onermeleri(&mut k, &api, &ozet["dna"][konum.as_str()], &uretici, &mut dayanak);
        // Yanıt şeması: açık beyan ya da tek anlamlı tip certain (kapsam kapalı), çıkarım inferred (kapsam açık)
        let ys = &kp["yanit"]["sema"];
        if ys["alanlar"].is_object() && ys["bulunamadi"] != true {
            let kesin = kp["yanit"]["kesin"] == true;
            let ydosya = ys["dosya"].as_str().filter(|s| !s.is_empty()).unwrap_or(dosya);
            let satir =
                ys["alanlar"].as_object().into_iter().flatten().filter_map(|(_, a)| a["satir"].as_u64()).filter(|s| *s > 0).min().unwrap_or(kp["satir"].as_u64().unwrap_or(0));
            let yd = dayanak(ydosya);
            let mut s = okunamadi(&yd);
            if !kesin {
                s.insert(0, format!("yanıt şeması çıkarım ({}): gönderilen nesne farklı olabilir", kp["yanit"]["kaynak"].as_str().unwrap_or("?")));
            }
            let dto = ys["dto"].as_str().unwrap_or("?");
            k.ekle(
                Kanit::yeni(KaynakTuru::SourceCode, &uretici, yd, vec![format!("{ydosya}:{satir}")], Some(if kesin { Guven::Certain } else { Guven::Inferred }))
                    .ozetli(json!({ "tur": "yanit_semasi", "dto": dto, "kaynak": kp["yanit"]["kaynak"] }), s),
                vec![Iddia::yeni(&api, "YANIT_SEMASI", Some(dto), govde_degeri(ys), if kesin { Kapsam::KapaliDunya } else { Kapsam::Acik })],
            );
        }
        // Teknik uç iş yeteneğini gerçekleştirir: ad yoldan, fonksiyondan ve yöntemden, inferred (aynı yeteneği
        // gerçekleştiren uçlar tek `cap:` düğümünde buluşur; düğümün kendisi `anlam` modülünde)
        let yetenek = format!("cap:{}", crate::anlam::yetenek(kp).anahtar);
        k.ekle(
            kaynak(Guven::Inferred).ozetli(json!({ "tur": "adlandirma" }), vec!["yetenek adı yoldan ve fonksiyon adından çıkarıldı".into()]),
            vec![Iddia::yeni(&api, "IMPLEMENTS", Some(&yetenek), json!({}), Kapsam::KapaliDunya)],
        );
    }
    // Tablo modelleri ve sütunları: Prisma şemasından (bu modül) ve adaptörün tablo modellerinden (SQLModel, SQLAlchemy).
    // Tablo kimliği eşlemesi: model adı -> gerçek tablo adı (@@map, __tablename__, yoksa varsayılan)
    let mut modeller: Vec<Value> = ek["veri_modelleri"].as_array().cloned().unwrap_or_default();
    if let Some(kk) = kok {
        for s in semalar.iter().filter(|s| s.ends_with(".prisma")) {
            if let Some(m) = crate::guvenli::oku(&kk.join(s)) {
                modeller.extend(prisma_modelleri(&m, s));
            }
        }
    }
    for m in &modeller {
        let (Some(ad), Some(mdosya)) = (m["ad"].as_str(), m["dosya"].as_str()) else { continue };
        let d = dayanak(mdosya);
        let s = okunamadi(&d);
        let tablo = m["tablo"].as_str().unwrap_or(ad);
        k.ekle(
            Kanit::yeni(KaynakTuru::DatabaseSchema, &uretici, d.clone(), vec![format!("{mdosya}:{}", m["satir"].as_u64().unwrap_or(1))], Some(Guven::Certain))
                .ozetli(json!({ "tur": "tablo_tanimi" }), s.clone()),
            vec![
                Iddia::yeni(&format!("db:{ad}"), "VAR", None, json!({ "ad": ad }), Kapsam::KapaliDunya),
                Iddia::yeni(&format!("db:{ad}"), "TABLO", None, json!({ "tablo": tablo, "kaynak": m["tablo_kaynagi"] }), Kapsam::KapaliDunya),
            ],
        );
        for a in m["alanlar"].as_array().into_iter().flatten() {
            let Some(alan) = a["ad"].as_str() else { continue };
            let deger = json!({ "model": ad, "tablo": tablo, "sutun": a["sutun"].as_str().unwrap_or(alan), "tip": a["tip"] });
            k.ekle(
                Kanit::yeni(KaynakTuru::DatabaseSchema, &uretici, d.clone(), vec![format!("{mdosya}:{}", a["satir"].as_u64().unwrap_or(1))], Some(Guven::Certain))
                    .ozetli(json!({ "tur": "sutun" }), s.clone()),
                vec![Iddia::yeni(&format!("field:{ad}.{alan}"), "VAR", None, deger, Kapsam::KapaliDunya)],
            );
        }
    }
    bagimlilik_onermeleri(&mut k, kapilar, ek, &uretici, &mut dayanak);
    cagri_onermeleri(&mut k, &ek["cagri"], &uretici, &mut dayanak);
    // Tabloların varlığı veri şeması dosyasından (Prisma, SQLModel ...): DATABASE_SCHEMA, certain
    if !semalar.is_empty() && !tablolar.is_empty() {
        let d: Vec<Dayanak> = semalar.iter().flat_map(|s| dayanak(s)).collect();
        let s = if d.len() < semalar.len() { vec!["bir şema dosyası okunamadı: geçerliliği izlenemez".to_string()] } else { vec![] };
        let iddialar = tablolar.keys().map(|t| Iddia::yeni(&format!("db:{t}"), "VAR", None, json!({ "ad": t }), Kapsam::KapaliDunya)).collect();
        k.ekle(
            Kanit::yeni(KaynakTuru::DatabaseSchema, &uretici, d, semalar.iter().map(|s| format!("{s}:1")).collect(), Some(Guven::Certain))
                .ozetli(json!({ "tur": "veri_semasi" }), s),
            iddialar,
        );
    }
    // Ön yüz bağları (web adaptörü, `web-baglari.json`): baglar = [{kaynak, iliski DISPLAYS|TRIGGERS|CALLS, hedef, guven,
    // kanit {tur, dosya, satir}}]. Kaynak `screen:`/`ui:`/`client:` düğümü; hedef bir kapıysa ("api:YÖNTEM /yol" ya da
    // "YÖNTEM /yol") haritadaki kapının düğümüne (sürümüyle) bağlanır
    let web_uretici = web["uretici"].as_str().map(String::from).unwrap_or_else(|| format!("sezgi-web {}", env!("CARGO_PKG_VERSION")));
    let mut web_kenarlar: Vec<crate::yolculuk::Kenar> = vec![];
    for b in web["baglar"].as_array().into_iter().flatten() {
        let iliski = b["iliski"].as_str().unwrap_or("").to_ascii_uppercase();
        let (Some(kaynak), Some(hedef)) = (b["kaynak"].as_str(), b["hedef"].as_str()) else { continue };
        if !ILISKILER.contains(&iliski.as_str()) || !kaynak.contains(':') {
            continue;
        }
        let hedef = if hedef.starts_with("api:") || !hedef.contains(':') {
            let yalin = hedef.trim_start_matches("api:");
            kimlikler.get(yalin).cloned().unwrap_or_else(|| format!("api:{yalin}"))
        } else {
            hedef.to_string()
        };
        let kn = &b["kanit"];
        let dosya = match kn["dosya"].as_str().unwrap_or("") {
            "" => String::new(),
            d if web_onek.is_empty() => d.to_string(),
            d => format!("{web_onek}/{d}"),
        };
        let dosya = dosya.as_str();
        let d = dayanak(dosya);
        let s = okunamadi(&d);
        web_kenarlar.push(crate::yolculuk::Kenar {
            kaynak: kaynak.to_string(),
            iliski: iliski.clone(),
            hedef: hedef.clone(),
            dosya: dosya.to_string(),
            satir: kn["satir"].as_u64().unwrap_or(0),
        });
        // Kaynak türü ve beyan web adaptöründen; tanınmayan tür kaynak kodu, tanınmayan beyan en zayıf değer sayılır
        let tur: KaynakTuru = serde_json::from_value(kn["tur"].clone()).unwrap_or(KaynakTuru::SourceCode);
        let beyan = match b["guven"].as_str() {
            Some("kesin") => Some(Guven::Certain),
            Some("yorum") => Some(Guven::Inferred),
            _ => serde_json::from_value::<Guven>(b["guven"].clone()).ok(),
        };
        k.ekle(
            Kanit::yeni(tur, &web_uretici, d, vec![format!("{dosya}:{}", kn["satir"].as_u64().unwrap_or(0))], beyan).ozetli(json!({ "tur": "web_bagi" }), s),
            vec![Iddia::yeni(kaynak, &iliski, Some(&hedef), json!({}), Kapsam::KapaliDunya)],
        );
    }
    // Kullanıcı yolculukları: statik en fazla inferred; gözlenen istek sırası yolculuğu observed yapar (UI adımları değil)
    let uc_bilgi: HashMap<String, crate::yolculuk::UcBilgi> = kapilar
        .iter()
        .map(|kp| {
            let u = crate::yolculuk::UcBilgi {
                yontem: kp["yontem"].as_str().unwrap_or("").to_ascii_uppercase(),
                yol: kp["yol"].as_str().unwrap_or("").to_string(),
                giris: kp["giris"].as_str().unwrap_or("bilinmiyor").to_string(),
            };
            (api_kimligi(kp), u)
        })
        .collect();
    let gozlem_uretici = format!("sezgi-gozlem {}", env!("CARGO_PKG_VERSION"));
    let ui_hedef = crate::yolculuk::ui_hedefleri(&web_kenarlar);
    for y in crate::yolculuk::kur(&web_kenarlar, &uc_bilgi) {
        let mut d: Vec<Dayanak> = vec![];
        for (dosya, _) in &y.konumlar {
            for x in dayanak(dosya) {
                if !d.contains(&x) {
                    d.push(x);
                }
            }
        }
        let mut s = vec!["statik yolculuk: adımlar ön yüz kaynağından; koşullu gösterim, yönlendirme ve oturum durumu kesin bilinmez".to_string()];
        s.extend(okunamadi(&d));
        let deger = json!({ "ad": y.ad, "hedef": y.hedef, "giris": y.giris, "adimlar": y.adimlar });
        let mut iddialar = vec![Iddia::yeni(&y.id, "VAR", None, deger.clone(), Kapsam::KapaliDunya)];
        for (i, a) in y.adimlar.iter().enumerate() {
            iddialar.push(Iddia::yeni(&y.id, "STEP", Some(a), json!({ "sira": i + 1 }), Kapsam::KapaliDunya));
        }
        let konum = y.konumlar.iter().map(|(f, l)| format!("{f}:{l}")).collect();
        k.ekle(Kanit::yeni(KaynakTuru::SourceCode, &web_uretici, d, konum, Some(Guven::Inferred)).ozetli(json!({ "tur": "yolculuk" }), s), iddialar);
        for kaynak in gozlem.map(|g| g.kaynaklar.as_slice()).unwrap_or(&[]) {
            if !crate::yolculuk::sirada(&y, &kaynak.sira) {
                continue;
            }
            let gd = gozlem
                .and_then(|g| g.kayitlar.iter().find(|r| r.bicim.api.as_deref() == Some(y.hedef.as_str()) && !r.dayanak.is_empty()))
                .map(|r| r.dayanak.clone())
                .unwrap_or_default();
            let sin = vec![if y.giris.is_some() {
                "istek sırası gözlendi (giriş ucu, sonra işlem); UI adımları gözlenmedi".to_string()
            } else {
                "işlemin isteği gözlendi; UI adımları gözlenmedi".to_string()
            }];
            k.ekle(
                Kanit::yeni(KaynakTuru::NetworkCapture, &gozlem_uretici, gd, vec![kaynak.tur.clone()], Some(Guven::Observed))
                    .ozetli(json!({ "tur": "yolculuk_gozlemi", "kaynak": kaynak.tur }), sin),
                vec![Iddia::yeni(&y.id, "VAR", None, deger.clone(), Kapsam::KapaliDunya)],
            );
        }
        // Tarayıcı senaryosu: REA'nın isteği başlatan adım olarak bağladığı UI adımı (aynı etiket, statik olarak aynı uca
        // ulaşıyor, uç yolculuğun işlemi ya da giriş ucu) gözlemle desteklenir; ekran adımları ve bağlanmayan adımlar inferred kalır
        for kaynak in gozlem.map(|g| g.kaynaklar.as_slice()).unwrap_or(&[]).iter().filter(|k| k.tur == "senaryo") {
            let guven = if kaynak.yalitim.as_deref() == Some("yok") { Guven::Inferred } else { Guven::Observed };
            for (i, a) in y.adimlar.iter().enumerate().filter(|(_, a)| a.starts_with("ui:")) {
                let hedefler = ui_hedef.get(a.as_str());
                let bag = kaynak.adim_baglari.iter().find(|b| {
                    b["api"].as_str().is_some_and(|api| hedefler.is_some_and(|h| h.contains(api)) && (api == y.hedef || y.giris.as_deref() == Some(api)))
                        && b["etiket"]
                            .as_str()
                            .is_some_and(|e| !crate::gozlem::sade(e).is_empty() && crate::gozlem::sade(e) == crate::gozlem::sade(&crate::gozlem::eylem_etiketi(a)))
                });
                let Some(b) = bag else { continue };
                let gd = gozlem
                    .and_then(|g| g.kayitlar.iter().find(|r| r.kaynak == kaynak.kimlik && r.bicim.api.as_deref() == b["api"].as_str()))
                    .map(|r| r.dayanak.clone())
                    .unwrap_or_default();
                let adim = b["adim"].as_str().unwrap_or("?");
                k.ekle(
                    Kanit::yeni(KaynakTuru::BrowserObservation, &gozlem_uretici, gd, vec![format!("{}#{adim}", &kaynak.kimlik[..20.min(kaynak.kimlik.len())])], Some(guven))
                        .ozetli(
                            json!({ "tur": "yolculuk_adimi_gozlemi", "adim": adim }),
                            vec!["REA tarayıcı senaryosu: adım isteği başlattı (adım ↔ istek kimliği bağı)".to_string()],
                        ),
                    vec![Iddia::yeni(&y.id, "STEP", Some(a), json!({ "sira": i + 1 }), Kapsam::KapaliDunya)],
                );
            }
        }
    }
    if let Some(g) = gozlem {
        let kapi_haritasi: HashMap<String, &Value> = kapilar.iter().map(|kp| (api_kimligi(kp), kp)).collect();
        crate::gozlem::kanitlara(g, &mut k, &kapi_haritasi, &govde_degeri, &crate::yolculuk::ui_hedefleri(&web_kenarlar));
    }
    // Dahili olay ve iş düğümleri (`ozet.olaylar`): dinleyicinin dekoratöründen varlık (certain), zincirinden tetiklenen
    // olay / iş (TRIGGERS), sildiği tablo (WRITES) ve kaskadla dolaylı yazılan (MAY_AFFECT); hepsi inferred, kapsam açık
    for (id, o) in ozet["olaylar"].as_object().into_iter().flatten() {
        for dn in o["dinleyiciler"].as_array().into_iter().flatten() {
            let Some(konum) = dn["kanit"].as_str() else { continue };
            let dosya = konum.rsplit_once(':').map(|x| x.0).unwrap_or(konum);
            let d = dayanak(dosya);
            let s = okunamadi(&d);
            let kn = Kanit::yeni(KaynakTuru::SourceCode, &uretici, d, vec![konum.to_string()], Some(Guven::Certain)).ozetli(json!({ "tur": "dinleyici" }), s);
            k.ekle(kn, vec![Iddia::yeni(id, "VAR", None, json!({ "tur": o["tur"], "ad": o["ad"] }), Kapsam::Acik)]);
            for t in o["silinen"].as_array().into_iter().flatten().filter_map(|t| t.as_str()) {
                let d = dayanak(dosya);
                let kn = Kanit::yeni(KaynakTuru::SourceCode, &uretici, d, vec![konum.to_string()], Some(Guven::Inferred)).ozetli(json!({ "tur": "is_silmesi" }), vec![]);
                k.ekle(kn, vec![Iddia::yeni(id, "WRITES", Some(&format!("db:{t}")), json!({ "islem": "delete" }), Kapsam::Acik)]);
            }
        }
        dna_onermeleri(&mut k, id, o, &uretici, &mut dayanak);
    }
    // Anlam: iş alanı, amaç ve problem (belge alıntısı), kullanıcı tipleri, akışlar ve iş yetenekleri (en fazla inferred)
    crate::anlam::onermeler(&mut k, harita, &modeller, kok, &uretici, &mut dayanak);
    let mut depo = k.bitir(zaman);
    depo.uretici = uretici;
    // Yan bilginin yalnızca grafı kuran kısmı saklanır (harita zaten taşıdığı alanları tekrar etmez)
    let alt: serde_json::Map<String, Value> =
        ["moduller", "routerlar", "kapi_routerlari", "veri_modelleri", "cagri"].iter().filter_map(|a| ek.get(*a).map(|v| (a.to_string(), v.clone()))).collect();
    depo.ek = if alt.is_empty() { Value::Null } else { Value::Object(alt) };
    depo
}

/// Prisma şemasındaki modeller: ad, gerçek tablo adı (`@@map`, yoksa model adı), tanım satırı ve sütunlar (ad, tip,
/// satır, gerçek sütun adı `@map`). İlişki alanları (başka modele ya da listeye) sütun değildir
pub fn prisma_modelleri(metin: &str, dosya: &str) -> Vec<Value> {
    let tirnakli = |s: &str, anahtar: &str| -> Option<String> {
        let i = s.find(anahtar)?;
        let r = &s[i + anahtar.len()..];
        let r = r.trim_start().strip_prefix('(')?.trim_start();
        let r = r.strip_prefix("name:").map(str::trim_start).unwrap_or(r);
        let r = r.strip_prefix('"')?;
        Some(r[..r.find('"')?].to_string())
    };
    // (model, @@map, satır, [(alan, tip, satır, @map)])
    type Model = (String, Option<String>, usize, Vec<(String, String, usize, Option<String>)>);
    let mut modeller: Vec<Model> = vec![];
    let mut icinde = false;
    for (i, satir) in metin.lines().enumerate() {
        let s = satir.trim();
        if let Some(m) = s.strip_prefix("model ") {
            modeller.push((m.trim_end_matches('{').trim().to_string(), None, i + 1, vec![]));
            icinde = true;
            continue;
        }
        if s.starts_with('}') {
            icinde = false;
            continue;
        }
        let Some(m) = modeller.last_mut().filter(|_| icinde) else { continue };
        if s.starts_with("@@map") {
            m.1 = tirnakli(s, "@@map");
            continue;
        }
        if s.is_empty() || s.starts_with("//") || s.starts_with("@@") {
            continue;
        }
        let mut p = s.split_whitespace();
        if let (Some(alan), Some(tip)) = (p.next(), p.next()) {
            m.3.push((alan.to_string(), tip.to_string(), i + 1, tirnakli(s, "@map")));
        }
    }
    let adlar: HashSet<String> = modeller.iter().map(|m| m.0.clone()).collect();
    modeller
        .into_iter()
        .map(|(ad, tablo, satir, alanlar)| {
            let alanlar: Vec<Value> = alanlar
                .into_iter()
                .filter(|(_, t, _, _)| !t.ends_with("[]") && !adlar.contains(t.trim_end_matches('?')))
                .map(|(a, t, s, sutun)| json!({ "ad": a, "tip": t.trim_end_matches('?'), "satir": s, "sutun": sutun }))
                .collect();
            json!({ "ad": ad, "tablo": tablo.clone().unwrap_or_else(|| ad.clone()), "tablo_kaynagi": if tablo.is_some() { "@@map" } else { "varsayılan (model adı)" },
                "dosya": dosya, "satir": satir, "alanlar": alanlar })
        })
        .collect()
}

/// Ortam koşulu düğümü: koşulun kod metni (boşluklar sıkıştırılmış)
fn ortam_kimligi(kosul: &str) -> String {
    format!("env:{}", kosul.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Bağımlılık önermeleri (adaptörün yan bilgisinden): NestJS modül grafiği (`mod:Modül`, içe aktarma DEPENDS_ON) ve
/// işlemin kontrolcüsünün modülü; FastAPI router içerme grafiği (`mod:dosya:ad`, içerilen içerene DEPENDS_ON), işlemin
/// router'ı ve içerme zincirindeki ortam koşulları (`env:koşul`; koşul metni okunamadıysa inferred)
fn bagimlilik_onermeleri(k: &mut Kurucu, kapilar: &[Value], ek: &Value, uretici: &str, dayanak: &mut dyn FnMut(&str) -> Vec<Dayanak>) {
    let mut kanit = |konum: &str, beyan: Guven, ozet: Value| -> Kanit {
        let dosya = konum.rsplit_once(':').map(|x| x.0).unwrap_or(konum);
        let d = dayanak(dosya);
        let s = if d.is_empty() { vec!["dayanak dosyası okunmadı: geçerliliği izlenemez".to_string()] } else { vec![] };
        Kanit::yeni(KaynakTuru::SourceCode, uretici, d, vec![konum.to_string()], Some(beyan)).ozetli(ozet, s)
    };
    // NestJS modülleri
    let moduller = ek["moduller"].as_object();
    let mut kontrolcu_modulu: HashMap<String, (String, String)> = HashMap::new();
    for (m, v) in moduller.into_iter().flatten() {
        let yer = format!("{}:{}", v["dosya"].as_str().unwrap_or(""), v["satir"].as_u64().unwrap_or(1));
        let mut iddialar = vec![Iddia::yeni(&format!("mod:{m}"), "VAR", None, json!({ "dosya": v["dosya"] }), Kapsam::KapaliDunya)];
        for i in metinler(&v["imports"]).iter().filter(|i| moduller.is_some_and(|ms| ms.contains_key(i.as_str()))) {
            iddialar.push(Iddia::yeni(&format!("mod:{m}"), "DEPENDS_ON", Some(&format!("mod:{i}")), json!({ "tur": "imports" }), Kapsam::KapaliDunya));
        }
        k.ekle(kanit(&yer, Guven::Certain, json!({ "tur": "modul" })), iddialar);
        for c in metinler(&v["kontrolculer"]) {
            kontrolcu_modulu.insert(c, (m.clone(), yer.clone()));
        }
    }
    // FastAPI router içerme grafiği
    let routerlar: HashMap<String, &Value> = ek["routerlar"].as_array().into_iter().flatten().filter_map(|r| Some((r["id"].as_str()?.to_string(), r))).collect();
    for (id, r) in &routerlar {
        let yer = format!("{}:{}", r["dosya"].as_str().unwrap_or(""), r["satir"].as_u64().filter(|s| *s > 0).unwrap_or(1));
        k.ekle(kanit(&yer, Guven::Certain, json!({ "tur": "router" })), vec![Iddia::yeni(&format!("mod:{id}"), "VAR", None, json!({ "dosya": r["dosya"] }), Kapsam::KapaliDunya)]);
        for u in r["ustler"].as_array().into_iter().flatten() {
            let Some(ust) = u["id"].as_str() else { continue };
            let icerme = u["icerme"].as_str().unwrap_or(&yer).to_string();
            let mut iddialar = vec![Iddia::yeni(&format!("mod:{id}"), "DEPENDS_ON", Some(&format!("mod:{ust}")), json!({ "tur": "include_router" }), Kapsam::KapaliDunya)];
            let beyan = match u["kosul"].as_str() {
                Some(kosul) => {
                    let env = ortam_kimligi(kosul);
                    iddialar.push(Iddia::yeni(&env, "VAR", None, json!({ "kosul": kosul }), Kapsam::KapaliDunya));
                    iddialar.push(Iddia::yeni(&format!("mod:{id}"), "DEPENDS_ON", Some(&env), json!({ "tur": "kosullu_kayit" }), Kapsam::KapaliDunya));
                    Guven::Certain
                }
                None => Guven::Certain,
            };
            k.ekle(kanit(&icerme, beyan, json!({ "tur": "router_icerme" })), iddialar);
            if u["kosullu"] == true && u["kosul"].is_null() {
                let env = ortam_kimligi("koşul metni okunamadı");
                k.ekle(
                    kanit(&icerme, Guven::Inferred, json!({ "tur": "kosullu_kayit" })),
                    vec![Iddia::yeni(&format!("mod:{id}"), "DEPENDS_ON", Some(&env), json!({ "tur": "kosullu_kayit" }), Kapsam::Acik)],
                );
            }
        }
    }
    // Router'ın içerme zincirindeki koşullar (yukarı doğru, döngüye karşı korumalı)
    let zincir_kosullari = |rid: &str| -> Vec<(String, String)> {
        let mut v = vec![];
        let (mut yigin, mut gorulen) = (vec![rid.to_string()], HashSet::new());
        while let Some(x) = yigin.pop() {
            if !gorulen.insert(x.clone()) || gorulen.len() > 64 {
                continue;
            }
            for u in routerlar.get(&x).into_iter().flat_map(|r| r["ustler"].as_array().into_iter().flatten()) {
                if let Some(kosul) = u["kosul"].as_str() {
                    v.push((kosul.to_string(), u["icerme"].as_str().unwrap_or("").to_string()));
                }
                if let Some(ust) = u["id"].as_str() {
                    yigin.push(ust.to_string());
                }
            }
        }
        v
    };
    for kp in kapilar {
        let api = crate::graf::api_kimligi(kp);
        let konum = format!("{}:{}", kp["dosya"].as_str().unwrap_or(""), kp["satir"].as_u64().unwrap_or(0));
        if let Some((m, yer)) = kp["kontrolcu"].as_str().and_then(|c| kontrolcu_modulu.get(c)) {
            k.ekle(
                kanit(yer, Guven::Certain, json!({ "tur": "modul" })),
                vec![Iddia::yeni(&api, "DEPENDS_ON", Some(&format!("mod:{m}")), json!({ "tur": "kontrolcu" }), Kapsam::KapaliDunya)],
            );
        }
        if let Some(rid) = ek["kapi_routerlari"][konum.as_str()].as_str() {
            k.ekle(
                kanit(&konum, Guven::Certain, json!({ "tur": "rota_kaydi" })),
                vec![Iddia::yeni(&api, "DEPENDS_ON", Some(&format!("mod:{rid}")), json!({ "tur": "router" }), Kapsam::KapaliDunya)],
            );
            for (kosul, yer) in zincir_kosullari(rid) {
                let yer = if yer.is_empty() { konum.clone() } else { yer };
                k.ekle(
                    kanit(&yer, Guven::Certain, json!({ "tur": "kosullu_kayit" })),
                    vec![Iddia::yeni(&api, "DEPENDS_ON", Some(&ortam_kimligi(&kosul)), json!({ "tur": "ortam" }), Kapsam::KapaliDunya)],
                );
            }
        }
    }
}

/// Çağrı grafiği (`ek.cagri`, bkz. `cagri::uygula`): işlemlerden ulaşılan fonksiyonlar arası `fn:A -[CALLS]-> fn:B`
/// kenarları. Kanıt çağıran fonksiyon başına (ve güven / çözüm yoluna göre) tektir: konumları çağrıların satırları,
/// dayanakları çağıranın ve çağrılanların dosyaları; kenarın kendi çağrı satırı ve çözüm yolu önermenin değerindedir.
/// Tek anlamlı statik hedef certain; arayüzün tek uygulaması, kapsayıcıdan çözülen sınıf ve ad eşleşmesi inferred.
/// Hedefi bilinmeyen (dinamik) çağrılar kenar değildir: işlemin davranış özetinde sayı olarak durur
fn cagri_onermeleri(k: &mut Kurucu, c: &Value, uretici: &str, dayanak: &mut dyn FnMut(&str) -> Vec<Dayanak>) {
    // Kenar: [kaynak, hedef, "dosya:satır", güven, çözüm, hedefin tanımı "dosya:satır"]
    type Grup<'a> = (Vec<&'a str>, Vec<&'a str>, Vec<Iddia>);
    let mut gruplar: BTreeMap<(&str, bool, &str), Grup> = BTreeMap::new();
    for e in c["kenarlar"].as_array().into_iter().flatten() {
        let (Some(a), Some(b), Some(konum)) = (e[0].as_str(), e[1].as_str(), e[2].as_str()) else { continue };
        let kesin = e[3] == "certain";
        let cozum = e[4].as_str().unwrap_or("");
        let g = gruplar.entry((a, kesin, if kesin { "" } else { cozum })).or_default();
        if !g.0.contains(&konum) {
            g.0.push(konum);
        }
        if let Some(hd) = e[5].as_str().and_then(|x| x.rsplit_once(':')).map(|x| x.0) {
            if !g.1.contains(&hd) {
                g.1.push(hd);
            }
        }
        g.2.push(Iddia::yeni(a, "CALLS", Some(b), json!({ "konum": konum, "cozum": cozum }), Kapsam::KapaliDunya));
    }
    for ((_, kesin, cozum), (konumlar, hedef_dosyalari, iddialar)) in gruplar {
        let dosya = konumlar.first().and_then(|k| k.rsplit_once(':')).map(|x| x.0).unwrap_or("");
        let mut d = dayanak(dosya);
        for hd in hedef_dosyalari.into_iter().filter(|x| *x != dosya) {
            for x in dayanak(hd) {
                if !d.contains(&x) {
                    d.push(x);
                }
            }
        }
        let mut s = if d.is_empty() { vec!["dayanak dosyası okunmadı: geçerliliği izlenemez".to_string()] } else { vec![] };
        if !kesin {
            s.insert(
                0,
                match cozum {
                    "arayuz_tek_uygulama" => "hedef, arayüzün projedeki tek uygulaması: çalışma anındaki bağlama farklı olabilir".to_string(),
                    "kapsayici" => "hedef, kapsayıcıdan çözülen sınıf: kayıtlı bağlama farklı olabilir".to_string(),
                    "ad" => "hedef yalnızca ad eşleşmesiyle bulundu".to_string(),
                    x => format!("hedef birden çok adaydan biri ({x})"),
                },
            );
        }
        let kn = Kanit::yeni(KaynakTuru::SourceCode, uretici, d, konumlar.iter().map(|x| x.to_string()).collect(), Some(if kesin { Guven::Certain } else { Guven::Inferred }))
            .ozetli(json!({ "tur": "cagri" }), s);
        k.ekle(kn, iddialar);
    }
}

/// Davranış özetinden önermeler (işlem düğümü `api`):
/// - hata senaryosu: `api -[VALIDATES]-> rule:dosya:satır` (değer: HTTP kodu, istisna, koşulun kod metni, kaynak),
///   işlem gövdesindeki desen certain, servis ya da bağımlılıktaki inferred;
/// - sahiplik denetimi: `SAHIPLIK` niteliği (hedef: kanıt satırı);
/// - iki yollu koşul: `DAL` niteliği;
/// - dolaylı yazma: `api -[MAY_AFFECT]-> db:Tablo` (veri şemasındaki kaskaddan; MAY_AFFECT hiçbir zaman certain olmaz);
/// - geri alma: `GERI_ALMA` niteliği; "bilinmiyor" kanıtsızdır (unknown, notuyla).
fn dna_onermeleri(k: &mut Kurucu, api: &str, v: &Value, uretici: &str, dayanak: &mut dyn FnMut(&str) -> Vec<Dayanak>) {
    if !v.is_object() {
        return;
    }
    let mut kanit = |konum: &str, tur: KaynakTuru, beyan: Guven, ozet: Value| -> Kanit {
        let dosya = konum.rsplit_once(':').map(|x| x.0).unwrap_or(konum);
        let d = dayanak(dosya);
        let s = if d.is_empty() { vec!["dayanak dosyası okunmadı: geçerliliği izlenemez".to_string()] } else { vec![] };
        Kanit::yeni(tur, uretici, d, vec![konum.to_string()], Some(beyan)).ozetli(ozet, s)
    };
    let guven = |g: &Value| if g == "certain" { Guven::Certain } else { Guven::Inferred };
    for h in v["hatalar"].as_array().into_iter().flatten() {
        let Some(konum) = h["kanit"].as_str() else { continue };
        let deger = json!({ "durum": h["durum"], "istisna": h["istisna"], "kosul": h["kosul"], "kaynak": h["kaynak"], "sahiplik": h["sahiplik"] });
        let kn = kanit(konum, KaynakTuru::SourceCode, guven(&h["guven"]), json!({ "tur": "hata_senaryosu" }));
        k.ekle(kn, vec![Iddia::yeni(api, "VALIDATES", Some(&format!("rule:{konum}")), deger, Kapsam::Acik)]);
    }
    for x in v["sahiplik_denetimi"].as_array().into_iter().flatten() {
        let Some(konum) = x["kanit"].as_str() else { continue };
        let kn = kanit(konum, KaynakTuru::SourceCode, guven(&x["guven"]), json!({ "tur": "sahiplik_denetimi" }));
        k.ekle(kn, vec![Iddia::yeni(api, "SAHIPLIK", Some(konum), json!({ "kosul": x["kosul"], "durum": x["durum"] }), Kapsam::Acik)]);
    }
    // Erişim denetimi (çağrı zinciri boyunca bulunan `requireAccess(...)`, `authorize(...)`, `@PreAuthorize(...)`): nesne
    // düzeyindeyse `NESNE_YETKI`, değilse `YETKI_DENETIMI` (hedef: kanıt satırı)
    for x in v["erisim_denetimi"].as_array().into_iter().flatten() {
        let Some(konum) = x["kanit"].as_str() else { continue };
        let yuklem = if x["nesne_duzeyi"] == true { "NESNE_YETKI" } else { "YETKI_DENETIMI" };
        let kn = kanit(konum, KaynakTuru::SourceCode, guven(&x["guven"]), json!({ "tur": "erisim_denetimi", "fonksiyon": x["fonksiyon"] }));
        let iddia = Iddia::yeni(api, yuklem, Some(konum), json!({ "ifade": x["ifade"], "izin": x["izin"], "fonksiyon": x["fonksiyon"] }), Kapsam::Acik)
            .notlu(vec!["denetimin sonucu çalışma anındaki kayda ve kimliğe bağlıdır".into()]);
        k.ekle(kn, vec![iddia]);
    }
    // Tablo etkisi (çağrı zinciri boyunca servis ve depo katmanında): `api -[READS|WRITES]-> db:T`. Değer boş: adaptörün
    // veri bağıyla aynı önermedir, kanıtı zincirin son halkasındaki satır; statik çıkarım olduğu için en fazla inferred
    for x in v["tablo_etkisi"].as_array().into_iter().flatten() {
        let (Some(konum), Some(t)) = (x["kanit"].as_str(), x["tablo"].as_str()) else { continue };
        let yuklem = if x["islem"] == "yazar" { "WRITES" } else { "READS" };
        let kn = kanit(konum, KaynakTuru::SourceCode, Guven::Inferred, json!({ "tur": "tablo_etkisi", "kaynak": x["kaynak"], "zincir": x["zincir"] }));
        k.ekle(kn, vec![Iddia::yeni(api, yuklem, Some(&format!("db:{t}")), json!({}), Kapsam::KapaliDunya)]);
    }
    for x in v["dallar"].as_array().into_iter().flatten() {
        let Some(konum) = x["kanit"].as_str() else { continue };
        let kn = kanit(konum, KaynakTuru::SourceCode, guven(&x["guven"]), json!({ "tur": "iki_yol" }));
        let deger = json!({ "kosul": x["kosul"], "evet": x["evet"], "hayir": x["hayir"], "baglam": x["baglam"] });
        k.ekle(kn, vec![Iddia::yeni(api, "DAL", Some(konum), deger, Kapsam::Acik)]);
    }
    // Yayılan olay ve kuyruğa konan iş: `varlik -[TRIGGERS]-> event:X | job:X` (ad eşlemesi; inferred)
    for t in v["tetikler"].as_array().into_iter().flatten() {
        let (Some(konum), Some(tur), Some(ad)) = (t["kanit"].as_str(), t["tur"].as_str(), t["ad"].as_str()) else { continue };
        let kn = kanit(konum, KaynakTuru::SourceCode, Guven::Inferred, json!({ "tur": "tetik" }));
        let iddia = Iddia::yeni(api, "TRIGGERS", Some(&format!("{tur}:{ad}")), json!({}), Kapsam::Acik)
            .notlu(vec!["olay / iş adı eşlemesi; dinleyicinin çalışması çalışma anında doğrulanmadı".into()]);
        k.ekle(kn, vec![iddia]);
    }
    for x in v["dolayli_yazar"].as_array().into_iter().flatten() {
        let (Some(konum), Some(tablo)) = (x["kanit"].as_str(), x["tablo"].as_str()) else { continue };
        let kn = kanit(konum, KaynakTuru::DatabaseSchema, Guven::Inferred, json!({ "tur": "kaskad" }));
        let iddia = Iddia::yeni(api, "MAY_AFFECT", Some(&format!("db:{tablo}")), json!({ "tur": "dolayli_yazma", "neden": x["neden"] }), Kapsam::Acik)
            .notlu(vec!["silmenin bu ilişkiyi tetiklediği şemadan çıkarıldı; çalışma anında doğrulanmadı".into()]);
        k.ekle(kn, vec![iddia]);
    }
    // Dış etki: `api -[CALLS]-> external:<ana makine | ortam.AD | sdk | dinamik>`; güven hedefin çözümüne (sabit adres
    // certain, ortam değişkeni inferred, dinamik unknown) ve zincire (işleyicinin dışından ulaşılan en fazla inferred) bağlı
    // Aynı dış sisteme birden fazla çağrı tek önermedir: değer bütün çağrıların listesi (her çağrı kendi satırıyla
    // aynı önermeyi destekler; farklı değerler çelişki sayılmasın)
    let mut sistem_cagrilari: std::collections::BTreeMap<&str, Vec<Value>> = std::collections::BTreeMap::new();
    for x in v["dis"].as_array().into_iter().flatten() {
        if let Some(d) = x["dugum"].as_str() {
            sistem_cagrilari.entry(d).or_default().push(json!({ "tur": x["tur"], "istemci": x["istemci"], "yontem": x["yontem"], "hedef": x["hedef"], "hedef_turu": x["hedef_turu"], "yol": x["yol"], "kanal": x["kanal"], "yazma": x["yazma"], "zaman": x["zaman"], "kanit": x["kanit"] }));
        }
    }
    for x in v["dis"].as_array().into_iter().flatten() {
        let (Some(konum), Some(dugum)) = (x["kanit"].as_str(), x["dugum"].as_str()) else { continue };
        let g = match x["guven"].as_str() {
            Some("certain") => Guven::Certain,
            Some("inferred") => Guven::Inferred,
            _ => Guven::Unknown,
        };
        let kn = kanit(konum, KaynakTuru::SourceCode, g, json!({ "tur": "dis_etki", "istemci": x["istemci"] }));
        let deger = json!({ "cagrilar": sistem_cagrilari.get(dugum) });
        let mut notlar = vec![];
        match x["hedef_turu"].as_str() {
            Some("ortam") => notlar.push("hedef ortam değişkeninden ya da yapılandırmadan: çalışma anındaki adres bilinmiyor".to_string()),
            Some("dinamik") => notlar.push("hedef adres koddan çözülemedi".to_string()),
            _ => {}
        }
        if x["derinlik"].as_u64().unwrap_or(0) > 0 {
            notlar.push("çağrı zinciri ad eşlemesiyle izlendi (yerel, sınırlı); çalışma anında doğrulanmadı".into());
        }
        k.ekle(kn, vec![Iddia::yeni(api, "CALLS", Some(dugum), deger, Kapsam::Acik).notlu(notlar)]);
    }
    // Geri alma: evet | kismen | hayir (gerekçesi ve kanıt satırıyla, en fazla inferred); bilinmiyor kanıtsızdır
    match (v["geri_alinabilir"].as_str(), v["geri_alma_konumu"].as_str()) {
        (Some(d @ ("evet" | "kismen" | "hayir")), Some(konum)) => {
            let kn = kanit(konum, KaynakTuru::SourceCode, Guven::Inferred, json!({ "tur": "geri_alma" }));
            k.ekle(kn, vec![Iddia::yeni(api, "GERI_ALMA", None, json!({ "durum": d, "neden": v["geri_alma_gerekce"] }), Kapsam::Acik)]);
        }
        (Some(_), _) => k.bos(Iddia::yeni(api, "GERI_ALMA", None, Value::Null, Kapsam::Bilinmiyor), "geri alınabilirlik koddan çıkarılamadı"),
        _ => {}
    }
}

/// Haritanın okunan dosya özetleri (uygulama köküne göre yol -> SHA-256); harita listeyi taşımıyorsa None
pub fn okunan_ozetleri(ozet: &Value) -> Option<HashMap<String, String>> {
    let l = ozet["okunan_dosyalar"].as_array()?;
    Some(l.iter().filter_map(|d| Some((d["yol"].as_str()?.to_string(), d["ozet"].as_str()?.to_string()))).collect())
}

/// `kaynak` klasöründen `hedef`e göreli yol, `/` ayraçlı (ör. backend'den ön yüze "../frontend"); ortak kök yoksa None
pub fn goreli_yol(kaynak: &Path, hedef: &Path) -> Option<String> {
    let (k, h): (Vec<_>, Vec<_>) = (kaynak.components().collect(), hedef.components().collect());
    let ortak = k.iter().zip(&h).take_while(|(a, b)| a == b).count();
    if ortak == 0 {
        return None;
    }
    let mut parca: Vec<String> = vec!["..".to_string(); k.len() - ortak];
    parca.extend(h[ortak..].iter().map(|c| c.as_os_str().to_string_lossy().to_string()));
    Some(if parca.is_empty() { ".".into() } else { parca.join("/") })
}

/// Çıktı klasöründen uygulama köküne göreli yol (yalnızca çıktı uygulamanın içindeyse)
pub fn kok_goreli(kok: &Path, cikti: &Path) -> Option<String> {
    let ic = cikti.strip_prefix(kok).ok()?;
    let n = ic.components().count();
    Some(if n == 0 { ".".into() } else { vec![".."; n].join("/") })
}

/// `kanit.json` ve `graf.json`'u birlikte yazar. Önceki depo varsa revizyonu bir artar ve değişmeyen önermelerin
/// revizyonu korunur. İkisi aynı revizyonu taşır; graf, kanıt dosyasının SHA-256 özetini taşır (`kanit_ozeti`): okuyan
/// taraf ikisini eşleştirerek yarım kalmış bir yazımı (kanıt yeni, graf eski) ayırt eder. Her dosya atomik yazılır.
pub fn yaz(cikti: &Path, depo: &mut Depo) -> Result<(), String> {
    let onceki: Option<Depo> = crate::guvenli::oku_harita(&cikti.join(DOSYA)).and_then(|m| serde_json::from_str(&m).ok());
    depo.revizyon_ata(onceki.as_ref().filter(|o| o.bicim == BICIM));
    let metin = serde_json::to_vec(&depo).map_err(|e| e.to_string())?;
    let ozet = sha256_hex(&metin);
    let graf = crate::graf::Graf::kur(depo).json(depo, &ozet);
    let graf_metni = serde_json::to_vec(&graf).map_err(|e| e.to_string())?;
    crate::guvenli::atomik_yaz(&cikti.join(DOSYA), &metin)?;
    crate::guvenli::atomik_yaz(&cikti.join(crate::graf::DOSYA), &graf_metni)
}

/// Bellek içi okuma modeli: MCP, rapor ve Ayna sorguları buradan cevaplanır. Yüklenirken önermeler kanıtlardan
/// yeniden hesaplanır ve dayanak dosyalarının bugünkü özetleriyle geçerlilik uygulanır.
pub struct Model {
    pub depo: Depo,
    pub graf: crate::graf::Graf,
    varlik: HashMap<String, Vec<usize>>,
    kanit: HashMap<String, usize>,
    kok: Option<PathBuf>,
    /// Dayanak dosyaları, yükleme anındaki (değişim zamanı, boyut) ve özetleri: ön eleme, özet yalnızca değişende
    /// yeniden hesaplanır
    dosyalar: Vec<DayanakDosyasi>,
    /// kanit.json yoksa (eski harita) model haritadan kurulur
    pub haritadan: bool,
}

/// Dayanak dosyası: yol, (değişim zamanı, boyut) ve SHA-256 özeti (okunamıyorsa None)
type DayanakDosyasi = (String, Option<(SystemTime, u64)>, Option<String>);

fn dosya_durumu(p: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::symlink_metadata(p).ok()?;
    Some((m.modified().ok()?, m.len()))
}

impl Model {
    pub fn yukle(klasor: &Path) -> Option<Model> {
        let varsayilan_kok = klasor.parent().filter(|_| klasor.file_name().is_some_and(|a| a == ".sezgi")).map(Path::to_path_buf);
        let dosyadan = crate::guvenli::oku_harita(&klasor.join(DOSYA)).and_then(|m| serde_json::from_str::<Depo>(&m).ok()).filter(|d| d.bicim == BICIM);
        let (depo, haritadan) = match dosyadan {
            Some(d) => (d, false),
            None => {
                let mut h: Value = crate::guvenli::oku_harita(&klasor.join("kapilar.json")).and_then(|m| serde_json::from_str(&m).ok())?;
                crate::guvenli::json_temiz(&mut h);
                let web: Value = crate::guvenli::oku_harita(&klasor.join(WEB_DOSYA)).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(Value::Null);
                let mut d = uret(&h, &web, "", &Value::Null, varsayilan_kok.as_deref(), "", crate::gozlem::Gozlemler::oku(klasor).as_ref());
                d.kok = varsayilan_kok.as_ref().map(|_| "..".into());
                (d, true)
            }
        };
        let kok = match &depo.kok {
            Some(r) => Some(klasor.join(r)),
            None if haritadan => varsayilan_kok,
            None => None,
        };
        let mut m = Model::kur(depo, kok);
        m.haritadan = haritadan;
        Some(m)
    }

    /// Depodan model: hesap kanıtlardan yeniden yapılır, geçerlilik bugünkü dosyalarla
    pub fn kur(mut depo: Depo, kok: Option<PathBuf>) -> Model {
        let mut dosyalar: Vec<DayanakDosyasi> = vec![];
        if let Some(k) = &kok {
            let mut gorulen: HashSet<String> = HashSet::new();
            for x in &depo.kanitlar {
                for d in &x.dayanak {
                    if gorulen.insert(d.yol.clone()) {
                        let p = k.join(&d.yol);
                        dosyalar.push((d.yol.clone(), dosya_durumu(&p), dosya_ozeti(&p)));
                    }
                }
            }
            let ozetler: HashMap<&str, Option<String>> = dosyalar.iter().map(|(y, _, o)| (y.as_str(), o.clone())).collect();
            depo.gecerlilik_isaretle(&mut |yol: &str| ozetler.get(yol).cloned().flatten());
        }
        depo.hesapla();
        Model::dizinle(depo, kok, dosyalar)
    }

    fn dizinle(depo: Depo, kok: Option<PathBuf>, dosyalar: Vec<DayanakDosyasi>) -> Model {
        let graf = crate::graf::Graf::kur(&depo);
        let mut varlik: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, o) in depo.onermeler.iter().enumerate() {
            varlik.entry(o.varlik.clone()).or_default().push(i);
        }
        let kanit = depo.kanitlar.iter().enumerate().map(|(i, k)| (k.id.clone(), i)).collect();
        Model { depo, graf, varlik, kanit, kok, dosyalar, haritadan: false }
    }

    /// Bir dayanak dosyası yüklemeden bu yana değiştiyse geçerliliği yeniden uygulanmış yeni model
    pub fn yenilenmis(&self) -> Option<Model> {
        let k = self.kok.as_ref()?;
        let simdi: Vec<Option<(SystemTime, u64)>> = self.dosyalar.iter().map(|(yol, _, _)| dosya_durumu(&k.join(yol))).collect();
        if self.dosyalar.iter().zip(&simdi).all(|((_, d, _), s)| d == s) {
            return None;
        }
        // Yalnızca değişim zamanı ya da boyutu değişen dosya yeniden özetlenir
        let dosyalar: Vec<DayanakDosyasi> =
            self.dosyalar.iter().zip(simdi).map(|((yol, d, o), s)| (yol.clone(), s, if *d == s { o.clone() } else { dosya_ozeti(&k.join(yol)) })).collect();
        let ozetler: HashMap<&str, Option<String>> = dosyalar.iter().map(|(y, _, o)| (y.as_str(), o.clone())).collect();
        let mut depo = self.depo.clone();
        depo.gecerlilik_uygula(&mut |yol: &str| ozetler.get(yol).cloned().flatten());
        let mut m = Model::dizinle(depo, self.kok.clone(), dosyalar);
        m.haritadan = self.haritadan;
        Some(m)
    }

    /// Varlığın kendi önermeleri (varlık olarak)
    pub fn onermeler(&self, varlik: &str) -> Vec<&Onerme> {
        self.varlik.get(varlik).map(|v| v.iter().map(|i| &self.depo.onermeler[*i]).collect()).unwrap_or_default()
    }

    /// İşlemin geçtiği kullanıcı yolculukları: hedefi ya da giriş adımının ucu bu işlem olan `journey:` önermeleri
    pub fn yolculuklar(&self, api: &str) -> Vec<&Onerme> {
        let mut v: Vec<&Onerme> =
            self.depo.onermeler.iter().filter(|o| o.yuklem == "VAR" && o.varlik.starts_with("journey:") && (o.deger["hedef"] == api || o.deger["giris"] == api)).collect();
        v.sort_by(|a, b| b.hesap.guven.cmp(&a.hesap.guven).then(a.varlik.cmp(&b.varlik)));
        v
    }

    pub fn kanit(&self, id: &str) -> Option<&Kanit> {
        self.kanit.get(id).map(|i| &self.depo.kanitlar[*i])
    }

    /// Önermeler için özet sayımlar (rapor ve Ayna)
    pub fn sayim(&self) -> Value {
        let mut guven: BTreeMap<&str, usize> = ["certain", "observed", "inferred", "unknown"].iter().map(|g| (*g, 0)).collect();
        let mut kapsam: BTreeMap<&str, usize> = BTreeMap::new();
        let mut tur: BTreeMap<String, usize> = BTreeMap::new();
        let (mut eski, mut celiski) = (0usize, 0usize);
        for o in &self.depo.onermeler {
            *guven.entry(o.hesap.guven.ad()).or_default() += 1;
            *kapsam
                .entry(match o.hesap.kapsam {
                    Kapsam::KapaliDunya => "kapali",
                    Kapsam::Acik => "acik",
                    Kapsam::Bilinmiyor => "bilinmiyor",
                })
                .or_default() += 1;
            eski += usize::from(o.hesap.gecerlilik != Gecerlilik::Gecerli);
            celiski += usize::from(!o.hesap.celiski.is_empty());
        }
        for k in &self.depo.kanitlar {
            *tur.entry(k.kaynak_turu.ad()).or_default() += 1;
        }
        json!({
            "revizyon": self.depo.revizyon, "kanit": self.depo.kanitlar.len(), "onerme": self.depo.onermeler.len(),
            "guven": guven, "kapsam": kapsam, "eski": eski, "celiski": celiski, "kaynak_turleri": tur,
            "dugum": self.graf.dugumler.len(), "kenar": self.graf.kenarlar.len(), "haritadan": self.haritadan,
        })
    }
}
