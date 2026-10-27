//! İlişki grafiği: kanıt deposundaki önermelerin düğüm/kenar görünümü. Kenar bir önermedir: ilişki
//! adlı yüklemi olan her önerme `varlik -[İLİŞKİ]-> hedef` kenarıdır ve güvenini, geçerliliğini önermeden alır. Düğüm
//! kimlikleri düğüm sözlüğündendir (`api:`, `db:`, `role:`, `cap:`, `fn:`, ön yüz için `ui:`, `screen:`, `client:` ...).
//! `graf.json` dışarıya (ön yüz adaptörü, kart) verilen görünümdür; bellek içi grafik her yüklemede depodan kurulur.

use crate::kanit::{Depo, Gecerlilik, Guven};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::Instant;

pub const DOSYA: &str = "graf.json";

/// İlişki sözlüğü (kartta küçük harfle yazılır)
pub const ILISKILER: [&str; 15] = [
    "DISPLAYS",
    "TRIGGERS",
    "STEP",
    "CALLS",
    "REFERENCES",
    "READS",
    "WRITES",
    "VALIDATES",
    "REQUIRES_PERMISSION",
    "DEPENDS_ON",
    "PRODUCES",
    "IMPLEMENTS",
    "MAY_AFFECT",
    "LINKS",
    "NAVIGATES",
];

/// `etki` sınırları: derinlik, ziyaret edilen düğüm ve süre
pub const EN_FAZLA_DERINLIK: usize = 3;
pub const EN_FAZLA_DUGUM: usize = 500;
pub const EN_FAZLA_SURE_MS: u128 = 200;

/// Teknik ucun kimliği: yöntem + yol şablonu, API sürümü (`@sürüm`) ve ikincil uygulamada uygulama kökü (`#uygulama`)
/// (eş adresli kapılar ayrışır). Kimlik anlam anahtarıdır, içerik özeti değil
pub fn api_kimligi(k: &Value) -> String {
    let mut s = format!("api:{} {}", k["yontem"].as_str().unwrap_or(""), k["yol"].as_str().unwrap_or(""));
    if let Some(v) = k["surum"].as_str().filter(|v| !v.is_empty()) {
        s.push('@');
        s.push_str(v);
    }
    if k["ana"].as_bool() == Some(false) {
        s.push('#');
        s.push_str(k["uygulama"].as_str().unwrap_or("?"));
    }
    s
}

/// Düğüm türü: kimliğin öneki (`api:GET /x` -> `api`)
pub fn tur(id: &str) -> &str {
    id.split_once(':').map(|(t, _)| t).unwrap_or("")
}

/// Düğümün okunur adı: önek atılmış kimlik
pub fn ad(id: &str) -> &str {
    id.split_once(':').map(|(_, a)| a).unwrap_or(id)
}

pub struct Graf {
    /// Düğüm kimliği -> türü
    pub dugumler: BTreeMap<String, String>,
    /// Kenar olan önermelerin depodaki sırası
    pub kenarlar: Vec<usize>,
    ileri: HashMap<String, Vec<usize>>,
    geri: HashMap<String, Vec<usize>>,
}

/// `etki` sonucundaki bir düğüm
pub struct EtkiDugumu {
    pub id: String,
    /// Yol boyunca en zayıf halka
    pub guven: Guven,
    pub derinlik: usize,
    /// Nereden gelindi: (ilişki, komşu düğüm)
    pub uzerinden: (String, String),
}

pub struct Etki {
    pub bulunan: Vec<EtkiDugumu>,
    pub ziyaret: usize,
    pub derinlik: usize,
    /// Aşılan sınır: "düğüm (500)", "süre (200 ms)"; derinlik sınırında kesilen yol ayrıca `derinlik_kesildi`
    pub sinir: Option<String>,
    pub derinlik_kesildi: bool,
    /// Eski (dayanağı değişmiş) olduğu için izlenmeyen kenar sayısı
    pub eski_kenar: usize,
    pub sure_ms: f64,
}

/// Yayılım kipi: ileri (X neyi değiştirir), geri (X'i kim kullanır), tablo (içeriği değişen tabloyu okuyanlar)
const ILERI: u8 = 1;
const GERI: u8 = 2;
const TABLO: u8 = 4;

impl Graf {
    pub fn kur(depo: &Depo) -> Graf {
        let mut g = Graf { dugumler: BTreeMap::new(), kenarlar: vec![], ileri: HashMap::new(), geri: HashMap::new() };
        for (i, o) in depo.onermeler.iter().enumerate() {
            g.dugumler.entry(o.varlik.clone()).or_insert_with(|| tur(&o.varlik).to_string());
            let Some(h) = o.hedef.as_ref().filter(|_| ILISKILER.contains(&o.yuklem.as_str())) else { continue };
            g.dugumler.entry(h.clone()).or_insert_with(|| tur(h).to_string());
            g.kenarlar.push(i);
            g.ileri.entry(o.varlik.clone()).or_default().push(i);
            g.geri.entry(h.clone()).or_default().push(i);
        }
        g
    }

    pub fn var_mi(&self, id: &str) -> bool {
        self.dugumler.contains_key(id)
    }

    /// Düğüme gelen kenarlar (önerme sıraları)
    pub fn gelen(&self, id: &str) -> &[usize] {
        self.geri.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Düğümden çıkan kenarlar
    pub fn giden(&self, id: &str) -> &[usize] {
        self.ileri.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Bu türde düğüm var mı (ör. ön yüz kanıtı haritada mı)
    pub fn tur_var(&self, turler: &[&str]) -> bool {
        self.dugumler.values().any(|t| turler.contains(&t.as_str()))
    }

    /// graf.json görünümü
    pub fn json(&self, depo: &Depo, kanit_ozeti: &str) -> Value {
        let dugumler: Vec<Value> = self.dugumler.iter().map(|(id, t)| json!({ "id": id, "tur": t, "ad": ad(id) })).collect();
        let kenarlar: Vec<Value> = self
            .kenarlar
            .iter()
            .map(|&i| {
                let o = &depo.onermeler[i];
                json!({ "onerme": o.id, "kaynak": o.varlik, "iliski": o.yuklem, "hedef": o.hedef, "guven": o.hesap.guven, "gecerlilik": o.hesap.gecerlilik, "nitelik": o.deger })
            })
            .collect();
        json!({ "bicim": crate::kanit::BICIM, "revizyon": depo.revizyon, "kanit_ozeti": kanit_ozeti, "dugumler": dugumler, "kenarlar": kenarlar })
    }

    /// Etki sorgusu: başlangıç düğümünden değişikliğin dokunabileceği tablolar, roller, ekranlar, işlemler ve
    /// yetenekler. İleri: DISPLAYS, TRIGGERS ve CALLS boyunca; WRITES ile tabloya ulaşılırsa o tabloyu okuyan ve ona yazan
    /// işlemler; geri: çağıran ve gösteren ön yüz öğeleri. Roller ve yetenekler yapraktır. Güven, yol boyunca en zayıf
    /// halkadır. Eski kenar izlenmez. `gorunur`: token'ın göremediği işlem düğümü hiç ziyaret edilmez.
    pub fn etki(&self, depo: &Depo, baslangic: &str, derinlik: usize, gorunur: &dyn Fn(&str) -> bool) -> Etki {
        let t0 = Instant::now();
        let derinlik = derinlik.clamp(1, EN_FAZLA_DERINLIK);
        let ilk_kip = match tur(baslangic) {
            "db" | "field" => TABLO,
            "role" | "perm" => GERI,
            _ => ILERI | GERI,
        };
        let mut kip: HashMap<String, u8> = HashMap::from([(baslangic.to_string(), ilk_kip)]);
        let mut bulunan: HashMap<String, EtkiDugumu> = HashMap::new();
        let mut sira: VecDeque<(String, usize, Guven, u8)> = VecDeque::from([(baslangic.to_string(), 0, Guven::Certain, ilk_kip)]);
        let (mut sinir, mut derinlik_kesildi, mut eski_kenar, mut adim) = (None, false, 0usize, 0usize);
        // Rol ve izin başlangıçta, kendisini isteyen işlemlere doğru açılır; sonra yapraktır
        let rol_baslangic = matches!(tur(baslangic), "role" | "perm");
        'dis: while let Some((d, der, guven, k)) = sira.pop_front() {
            let mut adaylar: Vec<(usize, bool, u8)> = vec![];
            // (önerme sırası, ileri mi, komşunun kipi)
            if k & ILERI != 0 {
                for &i in self.giden(&d) {
                    let yeni = match depo.onermeler[i].yuklem.as_str() {
                        "TRIGGERS" | "CALLS" | "DISPLAYS" | "STEP" => ILERI,
                        "WRITES" | "MAY_AFFECT" => TABLO,
                        _ => 0,
                    };
                    adaylar.push((i, true, yeni));
                }
            }
            if k & GERI != 0 {
                for &i in self.gelen(&d) {
                    if matches!(depo.onermeler[i].yuklem.as_str(), "TRIGGERS" | "CALLS" | "DISPLAYS" | "STEP") {
                        adaylar.push((i, false, GERI));
                    }
                }
                for &i in self.giden(&d) {
                    if matches!(depo.onermeler[i].yuklem.as_str(), "REQUIRES_PERMISSION" | "IMPLEMENTS") && k & ILERI == 0 {
                        adaylar.push((i, true, 0));
                    }
                }
            }
            if k & TABLO != 0 {
                for &i in self.gelen(&d) {
                    if matches!(depo.onermeler[i].yuklem.as_str(), "READS" | "WRITES") {
                        adaylar.push((i, false, GERI));
                    }
                }
                // Tablodan tabloya olası etki (tetikleyici, ON DELETE kaskadı)
                for &i in self.giden(&d) {
                    if depo.onermeler[i].yuklem == "MAY_AFFECT" {
                        adaylar.push((i, true, TABLO));
                    }
                }
            }
            if der == 0 && rol_baslangic {
                for &i in self.gelen(&d) {
                    if depo.onermeler[i].yuklem == "REQUIRES_PERMISSION" {
                        adaylar.push((i, false, GERI));
                    }
                }
            }
            for (i, ileri, yeni_kip) in adaylar {
                adim += 1;
                if adim.is_multiple_of(64) && t0.elapsed().as_millis() > EN_FAZLA_SURE_MS {
                    sinir = Some(format!("süre ({EN_FAZLA_SURE_MS} ms)"));
                    break 'dis;
                }
                let o = &depo.onermeler[i];
                if o.hesap.gecerlilik == Gecerlilik::Eski {
                    eski_kenar += 1;
                    continue;
                }
                let komsu = if ileri { o.hedef.clone().unwrap_or_default() } else { o.varlik.clone() };
                if komsu == baslangic || !gorunur(&komsu) {
                    continue;
                }
                if der + 1 > derinlik {
                    derinlik_kesildi = true;
                    continue;
                }
                let g = guven.min(o.hesap.guven);
                let onceki = kip.get(&komsu).copied().unwrap_or(0);
                if !bulunan.contains_key(&komsu) {
                    if bulunan.len() >= EN_FAZLA_DUGUM {
                        sinir = Some(format!("düğüm ({EN_FAZLA_DUGUM})"));
                        break 'dis;
                    }
                    bulunan.insert(komsu.clone(), EtkiDugumu { id: komsu.clone(), guven: g, derinlik: der + 1, uzerinden: (o.yuklem.clone(), d.clone()) });
                } else if let Some(b) = bulunan.get_mut(&komsu) {
                    if g > b.guven && der < b.derinlik {
                        b.guven = g;
                        b.uzerinden = (o.yuklem.clone(), d.clone());
                    } else if g == b.guven && b.uzerinden == ("READS".to_string(), d.clone()) && o.yuklem == "WRITES" {
                        // Aynı işlem tabloyu hem okuyup hem yazıyorsa etki yazmadır (davranış özetindeki veri bağıyla aynı)
                        b.uzerinden.0 = o.yuklem.clone();
                    }
                }
                if yeni_kip & !onceki != 0 {
                    kip.insert(komsu.clone(), onceki | yeni_kip);
                    sira.push_back((komsu, der + 1, g, yeni_kip));
                }
            }
        }
        let mut bulunan: Vec<EtkiDugumu> = bulunan.into_values().collect();
        bulunan.sort_by(|a, b| b.guven.cmp(&a.guven).then(a.derinlik.cmp(&b.derinlik)).then(a.id.cmp(&b.id)));
        Etki { ziyaret: bulunan.len(), bulunan, derinlik, sinir, derinlik_kesildi, eski_kenar, sure_ms: t0.elapsed().as_secs_f64() * 1000.0 }
    }
}
