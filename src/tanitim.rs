//! Ayna'nın tanıtım ekranları: "Sezgi neler yapabilir" (yetenekler ve bu projedeki durumları, sıradaki adım) ve
//! "bu uygulama hakkında ne biliyoruz" (kurucu sorularının bu uygulamanın verisinden yanıtları).
//!
//! Yetenek durumu gerçek durumdan hesaplanır ve salt okunurdur: harita, bağlantı defteri, canlı mod durumu ve santral
//! yalnızca okunur, hiçbir şey kurulmaz ya da yazılmaz. Durumlar: `hazir`, `kurulmamis`, `uygulanamaz`.
//!
//! Sorular yeni olgu üretmez: her madde haritadan, kanıt deposundan ya da kapsam raporundan sayılır ve dayandığı
//! önermelerin güvenini taşır (ölçek certain > observed > inferred > unknown). Eski (dayanağı değişmiş) önerme
//! bilinmiyor sayılır. Bir yanıtın damgası en zayıf maddesininkidir. Yanıtlanamayan soru `bilinmiyor` olarak, nedeni ve
//! (varsa) yardımcı olabilecek komutla döner; bilinmeyen bir şey "yok" diye yazılmaz.

use crate::dil::cogul;
use crate::kanit::{Gecerlilik, Guven, Kapsam, Model, Onerme};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Önermenin geçerli güveni: dayanağı değişmiş önerme bilinmiyor sayılır
fn etkin(o: &Onerme) -> Guven {
    if o.hesap.gecerlilik == Gecerlilik::Gecerli {
        o.hesap.guven
    } else {
        Guven::Unknown
    }
}

/// Bir listenin en zayıf güveni; liste boşsa `bos`
fn en_zayif(it: impl IntoIterator<Item = Guven>, bos: Guven) -> Guven {
    it.into_iter().min().unwrap_or(bos)
}

fn madde(metin: impl Into<String>, guven: Guven) -> Value {
    json!({ "metin": metin.into(), "guven": guven.ad(), "ornekler": [] })
}

fn ornekli(metin: impl Into<String>, guven: Guven, ornekler: Vec<String>) -> Value {
    json!({ "metin": metin.into(), "guven": guven.ad(), "ornekler": ornekler })
}

fn komut(k: &str, aciklama: &str) -> Value {
    json!({ "komut": k, "aciklama": aciklama })
}

/// Soru yanıtı. Metni ve sıra numarası (`no`) kurucu sorularının listesinden. `durum`: maddesi yoksa bilinmiyor,
/// bilinmeyen notu varsa kısmen, yoksa yanıtlı. `guven`: verilmezse maddelerin en zayıfı
fn soru(id: &str, maddeler: Vec<Value>, bilinmeyen: Option<String>, yardim: Option<Value>, guven: Option<Guven>) -> Value {
    let (no, metin) = SORULAR.iter().enumerate().find(|(_, (k, _, _))| *k == id).map(|(i, (_, m, e))| (i + 1, t!(*m, *e))).unwrap_or((0, id));
    let durum = if maddeler.is_empty() {
        "bilinmiyor"
    } else if bilinmeyen.is_some() {
        "kismen"
    } else {
        "yanitli"
    };
    let g = guven.unwrap_or_else(|| {
        en_zayif(
            maddeler.iter().map(|m| match m["guven"].as_str() {
                Some("certain") => Guven::Certain,
                Some("observed") => Guven::Observed,
                Some("inferred") => Guven::Inferred,
                _ => Guven::Unknown,
            }),
            Guven::Unknown,
        )
    });
    json!({ "id": id, "no": no, "soru": metin, "durum": durum, "guven": if durum == "bilinmiyor" { "unknown" } else { g.ad() }, "maddeler": maddeler, "bilinmeyen": bilinmeyen, "komut": yardim.unwrap_or(Value::Null) })
}

/// Kurucunun 17 sorusu, kendi sırasıyla (haritasız açılışta da aynı liste): (kimlik, Türkçe, İngilizce). Yanıtı zayıf
/// olan soru da listede kalır: neyin bilinmediği de bir yanıttır
const SORULAR: [(&str, &str, &str); 17] = [
    ("ne", "Bu uygulama nedir?", "What is this application?"),
    ("amac", "Neden var?", "Why does it exist?"),
    ("problem", "Hangi iş problemini çözer?", "What business problem does it solve?"),
    ("kullanicilar", "Kullanıcıları kim?", "Who are its users?"),
    ("yapabilir", "Kullanıcı ne yapabilir?", "What can a user do?"),
    ("calisiyor", "Hangi özellik gerçekten çalışıyor?", "Which features actually work?"),
    ("ekran", "Hangi işlem hangi ekrandan başlar?", "Which operation starts from which screen?"),
    ("cagri", "Hangi fonksiyonlar birbirini çağırır?", "Which functions call each other?"),
    ("tablolar", "Hangi tablolar etkilenir?", "Which tables are affected?"),
    ("onkosul", "Ön koşullar neler?", "What are the preconditions?"),
    ("yetki", "Kim hangi işleme yetkili?", "Who is authorized to do what?"),
    ("dis", "Başka uygulamaları etkiler mi?", "Does it affect other applications?"),
    ("geri", "Geri alınabilir mi?", "Can it be undone?"),
    ("onay", "İnsan onayı gerekir mi?", "Does it need human approval?"),
    ("guven", "Ne kesin, ne tahmin, ne bilinmiyor?", "What is certain, what is inferred, what is unknown?"),
    ("degisim", "Değişince ne geçersiz olur?", "What becomes invalid when the code changes?"),
    ("gorev", "Doğal dil görevi için hangi yetenekler gerekir?", "Which capabilities does a natural-language task need?"),
];

/// Ölçeğin ekrandaki karşılığı (Ayna'nın lejantı)
fn olcek() -> Value {
    json!([
        { "guven": "certain", "ad": t!("kesin", "certain"), "anlam": t!("koddan ya da şemadan tek anlamlı çözümleme", "resolved unambiguously from code or schema") },
        { "guven": "observed", "ad": t!("gözlem", "observed"), "anlam": t!("çalışma anında görüldü (ağ kaydı, tarayıcı senaryosu, test)", "seen at run time (network log, browser scenario, test)") },
        { "guven": "inferred", "ad": t!("yorum", "inferred"), "anlam": t!("çıkarım: ad, koşullu yol, birden fazla aday", "a deduction: from a name, a conditional path or several candidates") },
        { "guven": "unknown", "ad": t!("bilinmiyor", "unknown"), "anlam": t!("kanıt yok; \"yok\" demek değildir", "no evidence; this does not mean \"absent\"") },
    ])
}

/// Haritasız açılış: bütün sorular bilinmiyor, nedeni ve ilk adım
pub fn sorular_haritasiz() -> Value {
    let liste: Vec<Value> = SORULAR
        .iter()
        .map(|(id, _, _)| {
            soru(
                id,
                vec![],
                Some(
                    t!("Bu klasörde harita yok: Sezgi bu uygulamanın kodunu henüz okumadı.", "There is no map in this folder: Sezgi has not read this application's code yet.")
                        .into(),
                ),
                Some(komut("sezgi init", t!("kodu okur, haritayı çıkarır", "reads the code and builds the map"))),
                None,
            )
        })
        .collect();
    json!({ "harita": false, "olcek": olcek(), "sorular": liste })
}

/// Ana uygulamanın kapıları ve işlem düğümleri
struct Veri<'a> {
    harita: &'a Value,
    rapor: &'a Value,
    model: Option<&'a Model>,
    kapilar: Vec<(&'a Value, String)>,
    /// Kanıt deposu dosyaların değişimini izliyor mu (uygulama kökü biliniyor)
    izleniyor: bool,
    /// Canlı modun kullandığı önizleme: hangi kuralların hiçbir zaman denetlenemeyeceğini aynı koddan söyler
    onizleyici: Option<&'a crate::mcp::Harita>,
}

impl<'a> Veri<'a> {
    fn onermeler(&self, api: &str, yuklem: &str) -> Vec<&'a Onerme> {
        self.model.map(|m| m.onermeler(api).into_iter().filter(|o| o.yuklem == yuklem).collect()).unwrap_or_default()
    }
    /// Kapıların bu yüklemdeki önermelerinin en zayıf güveni
    fn guveni(&self, yuklem: &str, suz: impl Fn(&Value) -> bool) -> Guven {
        en_zayif(self.kapilar.iter().filter(|(k, _)| suz(k)).flat_map(|(_, a)| self.onermeler(a, yuklem)).map(etkin), Guven::Unknown)
    }
    fn kaynaksiz(&self) -> bool {
        self.harita["ozet"]["kaynaksiz"] == true
    }
    /// Kapının davranış özeti (haritadaki `ozet.dna`, "dosya:satır" anahtarıyla; `rapor` aynı anahtarı kullanır)
    fn dna(&self, k: &Value) -> &'a Value {
        dna(self.harita, k)
    }
}

/// Kapının davranış özeti: çağrı zinciri, tablo etkisi, erişim denetimi, dış etki, ön koşul, geri alma
fn dna<'a>(harita: &'a Value, k: &Value) -> &'a Value {
    harita["ozet"]["dna"].get(format!("{}:{}", k["dosya"].as_str().unwrap_or(""), k["satir"])).unwrap_or(&Value::Null)
}

/// Ana uygulamanın kapıları (ikincil uygulamalar sayılmaz; `rapor` ile aynı süzgeç) ve graf kimlikleri
fn ana_kapilar(harita: &Value) -> Vec<(&Value, String)> {
    harita["endpointler"].as_array().into_iter().flatten().filter(|k| k["ana"].as_bool() != Some(false)).map(|k| (k, crate::graf::api_kimligi(k))).collect()
}

/// Red koşulu iş durumu ön koşulu mu: hata yakalama blokları (catch / except) ön koşul değildir (`rapor` ile aynı kural)
fn on_kosul_mu(h: &Value) -> bool {
    h["kosul"].as_str().is_some_and(|x| x != "catch" && x != "except")
}

/// Kapıda nesne düzeyi yetki denetimi var mı: servis katmanında kaydın bu kullanıcıya açık olup olmadığı ya da kaydın
/// sahibi denetleniyor (`rapor`un `nesne_duzeyi_denetimli_islem` sayımıyla aynı kural)
fn nesne_denetimli(d: &Value) -> bool {
    d["erisim_denetimi"].as_array().is_some_and(|e| e.iter().any(|x| x["nesne_duzeyi"] == true)) || d["sahiplik_denetimi"].as_array().is_some_and(|s| !s.is_empty())
}

/// Yazma işleminin onay sınıfı: canlı modun kuralıyla aynı sıra (önce statik kısım, çelişkili kanıt, sonra önizlemenin
/// hiçbir zaman denetleyemediği kurallar). Okuma onaya hiç düşmez
enum OnaySinifi {
    Okuma,
    Silme,
    /// Dış sisteme etki ya da geri alınamaz yazma (haritada `etki_onayi`): gerekçesi
    DisEtki(String),
    KesinDegil,
    Celiskili,
    /// Önizlemenin isteğin değerinden bağımsız olarak hiç denetleyemediği kurallar
    Onizlemesiz(Vec<String>),
    Rol,
    Kosullu,
}

fn onay_sinifi(v: &Veri, k: &Value) -> OnaySinifi {
    let r = crate::canli::islem_bilgisi(k).1;
    if r == "oku" {
        return OnaySinifi::Okuma;
    }
    if r == "yikici" {
        return OnaySinifi::Silme;
    }
    if let Some(g) = k["etki_onayi"].as_str() {
        return OnaySinifi::DisEtki(g.to_string());
    }
    let statik = crate::canli::statik_onay(k);
    if statik.is_some() && crate::benlik::islem_damgasi(k) != "certain" {
        return OnaySinifi::KesinDegil;
    }
    if v.model.is_some_and(|md| crate::canli::celiskili_onermeler(md, k) > 0) {
        return OnaySinifi::Celiskili;
    }
    let hep = v.onizleyici.map(|h| h.hep_denetlenemeyen(k)).unwrap_or_default();
    if !hep.is_empty() {
        return OnaySinifi::Onizlemesiz(hep);
    }
    if statik.is_some() {
        OnaySinifi::Rol
    } else {
        OnaySinifi::Kosullu
    }
}

/// Onay sınıfının tek işlem için düz dildeki karşılığı: (karar, metin, güven). Karar: `gerekmez`, `her_seferinde`,
/// `kosullu`. Onay isteyenler Sezgi'nin kendi kuralından (kesin); onaysız gidebilmesi yalnızca koşullu bir olasılık (yorum)
fn onay_metni(s: &OnaySinifi, k: &Value) -> (&'static str, String, Guven) {
    match s {
        OnaySinifi::Okuma => ("gerekmez", t!(METIN_OKUMA_ONAYSIZ, METIN_OKUMA_ONAYSIZ_EN).to_string(), Guven::Certain),
        OnaySinifi::Silme => ("her_seferinde", t!("Silme işlemi: her seferinde insan onayı ister.", "A delete: needs human approval every time.").into(), Guven::Certain),
        OnaySinifi::DisEtki(g) => {
            let g = crate::dis::gerekce_metni(g);
            ("her_seferinde", tf!("Her seferinde insan onayı ister: {g}.", "Needs human approval every time: {g}."), Guven::Certain)
        }
        OnaySinifi::KesinDegil => (
            "her_seferinde",
            t!(
                "Onay ister: dayandığı kanıt kesin değil (giriş, girdi şeması ya da rol kararı koddan kesin çıkarılamadı).",
                "Needs approval: the evidence behind it is not certain (the login, input schema or role decision could not be derived with certainty from the code)."
            )
            .into(),
            Guven::Certain,
        ),
        OnaySinifi::Celiskili => (
            "her_seferinde",
            t!(
                "Onay ister: kanıtı çelişkili (kaynaklar birbirini tutmuyor); çelişki giderilene kadar.",
                "Needs approval: its evidence is contradictory (the sources disagree), until the contradiction is resolved."
            )
            .into(),
            Guven::Certain,
        ),
        OnaySinifi::Onizlemesiz(n) => {
            let turler = denetlenemeyen_turleri(n).join("; ");
            (
                "her_seferinde",
                tf!(
                    "Her istekte onay ister: önizleme bu işlemde şunları hiçbir zaman denetleyemez: {turler}.",
                    "Needs approval on every request: for this operation the preview can never check {turler}."
                ),
                Guven::Certain,
            )
        }
        OnaySinifi::Rol => {
            let roller = k["roller"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(", ");
            (
                "her_seferinde",
                tf!(
                    "Rol istiyor ({roller}): insanın doğrulanmış rolü yoksa onay ister (yapay zekânın rol beyanı doğrulanmış sayılmaz).",
                    "Needs a role ({roller}): without the human's verified role it needs approval (a role the AI claims is never treated as verified)."
                ),
                Guven::Certain,
            )
        }
        OnaySinifi::Kosullu => ("kosullu", t!(METIN_KOSULLU, METIN_KOSULLU_EN).to_string(), Guven::Inferred),
    }
}

const METIN_OKUMA_ONAYSIZ: &str = "Okuma işlemi: insan onayı gerekmez; yine de yalnızca token'ın açtığı işlemler çağrılabilir.";
const METIN_OKUMA_ONAYSIZ_EN: &str = "A read: no human approval needed; still, only the operations the token opens can be called.";
const METIN_KOSULLU: &str = "Koşullar sağlanırsa onaysız gidebilir; canlı mod bunu her istekte yeniden denetler: token işlemi açmış olmalı, girdi bilinen bütün kuralları geçmeli, token bütün yazmalara onay istememeli ve hedef gerçek veri olmamalı (gerçek veride her yazma onaylanır).";
const METIN_KOSULLU_EN: &str = "May go without approval if the conditions hold; live mode checks them again on every request: the token must open the operation, the input must pass every known rule, the token must not require approval for all writes and the target must not be real data (on real data every write is approved).";

/// Bir işlem düğümünden geriye (ön yüz zinciri: istemci çağrısı → UI eylemi → ekran) en çok üç adımda ulaşılan
/// ekranlar; yol boyunca en zayıf halkanın güveniyle (ekran başına en iyi yol)
fn ekranlar(m: &Model, api: &str) -> BTreeMap<String, Guven> {
    let mut bulunan: BTreeMap<String, Guven> = BTreeMap::new();
    let mut sinir: Vec<(String, Guven)> = vec![(api.to_string(), Guven::Certain)];
    for _ in 0..3 {
        let mut yeni = vec![];
        for (d, g) in &sinir {
            for &i in m.graf.gelen(d) {
                let o = &m.depo.onermeler[i];
                if !matches!(o.yuklem.as_str(), "CALLS" | "TRIGGERS" | "DISPLAYS") {
                    continue;
                }
                let tur = crate::graf::tur(&o.varlik);
                let gy = (*g).min(etkin(o));
                match tur {
                    "screen" => {
                        let e = bulunan.entry(o.varlik.clone()).or_insert(Guven::Unknown);
                        *e = (*e).max(gy);
                    }
                    "ui" | "comp" | "client" => yeni.push((o.varlik.clone(), gy)),
                    _ => {}
                }
            }
        }
        sinir = yeni;
    }
    bulunan
}

/// "Bu uygulama hakkında ne biliyoruz": kurucu sorularının bu haritadan yanıtları
pub fn sorular(harita: &Value, rapor: &Value, model: Option<&Model>, izleniyor: bool, onizleyici: Option<&crate::mcp::Harita>) -> Value {
    let v = Veri { harita, rapor, model, kapilar: ana_kapilar(harita), izleniyor, onizleyici };
    let liste = vec![
        ne(&v),
        amac(&v),
        problem(&v),
        kullanicilar(&v),
        yapabilir(&v),
        calisiyor(&v),
        ekran(&v),
        cagri(&v),
        tablolar(&v),
        onkosul(&v),
        yetki(&v),
        dis(&v),
        geri(&v),
        onay(&v),
        guven_sayimi(&v),
        degisim(&v),
        gorev(&v),
    ];
    json!({ "harita": true, "olcek": olcek(), "sorular": liste })
}

/// Ekran düğümünün kullanıcının gördüğü adı: web ekranı yolu (`screen:web//items` -> `/items`), masaüstü ve mobil
/// ekranlar türüyle
fn ekran_adi(id: &str) -> String {
    let a = crate::graf::ad(id);
    match a.split_once('/') {
        Some(("web", "")) => "/".into(),
        Some(("web", y)) if y.starts_with('/') => y.into(),
        Some(("web", y)) => format!("/{y}"),
        Some(("electron", y)) => format!("{y} (Electron)"),
        Some(("win", y)) => format!("{y} (Windows)"),
        Some(("android", y)) => format!("{y} (Android)"),
        _ => a.into(),
    }
}

fn modul(k: &Value) -> String {
    let m = k["kontrolcu"].as_str().unwrap_or("").trim_end_matches("Controller");
    if m.is_empty() {
        t!("(adsız)", "(unnamed)").into()
    } else {
        m.to_string()
    }
}

fn ne(v: &Veri) -> Value {
    let ozet = &v.harita["ozet"];
    let mut m = vec![];
    let ad = ozet["uygulama"].as_str().unwrap_or("");
    if v.kaynaksiz() {
        // Kaynaksız uygulamanın kimliği ikilinin beyan ettiği metadata'dan: güveni uygulama düğümünün önermesinden
        let dugum = ozet["uygulama_dugumu"].as_str().unwrap_or("");
        let g = en_zayif(v.model.map(|md| md.onermeler(dugum).into_iter().filter(|o| o.yuklem == "VAR").map(etkin).collect::<Vec<_>>()).unwrap_or_default(), Guven::Unknown);
        m.push(madde(tf!("Kaynak kodu olmayan bir uygulama: {ad}", "An application without source code: {ad}"), g));
    } else if !ad.is_empty() {
        m.push(madde(tf!("Adı {ad} (paket tanımından)", "Named {ad} (from the package definition)"), Guven::Certain));
    }
    if let Some(a) = ozet["adaptor"].as_str().filter(|a| !a.is_empty()) {
        m.push(madde(tf!("Okuyan adaptör: {a}", "Read by the {a} adapter"), Guven::Certain));
    }
    if !v.kapilar.is_empty() {
        let mut moduller: BTreeMap<String, usize> = BTreeMap::new();
        for (k, _) in &v.kapilar {
            *moduller.entry(modul(k)).or_default() += 1;
        }
        let mut sirali: Vec<(String, usize)> = moduller.into_iter().collect();
        sirali.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let g = v.guveni("VAR", |_| true);
        m.push(ornekli(
            tf!("{} işlem, {} modülde", "{} {i} in {} {m}", v.kapilar.len(), sirali.len(); i = cogul(v.kapilar.len(), "operation", "operations"), m = cogul(sirali.len(), "module", "modules")),
            g,
            sirali.iter().take(10).map(|(a, n)| format!("{a} ({n})")).collect(),
        ));
    }
    let ikincil: Vec<String> = v.rapor["kapilar"]["ikincil_uygulamalar"].as_object().into_iter().flatten().map(|(a, n)| format!("{a} ({n})")).collect();
    if !ikincil.is_empty() {
        m.push(ornekli(tf!("Aynı depoda {} ikincil uygulama daha var", "The same repository holds {} more secondary applications", ikincil.len()), Guven::Certain, ikincil));
    }
    if let Some(md) = v.model {
        let ekran: Vec<&Onerme> = md.depo.onermeler.iter().filter(|o| o.yuklem == "VAR" && o.varlik.starts_with("screen:")).collect();
        if !ekran.is_empty() {
            m.push(ornekli(
                tf!("Ön yüzde {} ekran okundu", "{} {e} read in the front end", ekran.len(); e = cogul(ekran.len(), "screen", "screens")),
                en_zayif(ekran.iter().map(|o| etkin(o)), Guven::Unknown),
                ekran.iter().take(8).map(|o| ekran_adi(&o.varlik)).collect(),
            ));
        }
    }
    // İş alanı ve türü: sözlük tabanlı sınıflandırma, kanıtlarıyla (en fazla yorum)
    let a = anlam_ozeti(v);
    let alan = &a["alan"];
    if alan["id"].is_string() {
        m.push(ornekli(tf!("İş alanı: {}", "Business domain: {}", ad_dilde(alan)), guven_degeri(&alan["guven"]), crate::anlam::kanit_ornekleri(alan, 8)));
    }
    if a["tur"]["id"].is_string() {
        m.push(ornekli(tf!("Türü: {}", "Kind: {}", ad_dilde(&a["tur"])), guven_degeri(&a["tur"]["guven"]), crate::anlam::kanit_ornekleri(&a["tur"], 4)));
    }
    let not = if alan["id"].is_string() {
        t!(
            "İş alanı README, paket tanımı, tablo, yol ve rol adlarındaki terimlerin sözlükle sayımından çıkarılır (yorum); uygulamanın sahibi doğrulamalı.",
            "The business domain is derived by counting dictionary terms in the README, the package definition and the table, path and role names (inferred); the app's owner should confirm it."
        )
        .to_string()
    } else {
        let neden = crate::anlam::siniflama_notu(alan["not"].as_str());
        tf!(
            "İş alanı belirlenemedi ({neden}); yanlış bir alan söylemektense bilinmiyor denir. Yalnızca yapısı (işlemler, modüller, ekranlar) bilinir.",
            "The business domain could not be determined ({neden}); Sezgi says unknown rather than name a wrong one. Only the structure (operations, modules, screens) is known."
        )
    };
    soru("ne", m, Some(not), None, None)
}

/// Anlam özeti (kanıt deposundaki anlam önermeleri; depo yoksa Null)
fn anlam_ozeti(v: &Veri) -> Value {
    v.model.map(|m| crate::anlam::ozet(&m.depo)).unwrap_or(Value::Null)
}

/// Anlam katmanının adı seçilen dilde (`ad` / `ad_en`)
fn ad_dilde(v: &Value) -> &str {
    let tr = v["ad"].as_str().unwrap_or("");
    if crate::dil::en() {
        v["ad_en"].as_str().unwrap_or(tr)
    } else {
        tr
    }
}

fn guven_degeri(g: &Value) -> Guven {
    match g.as_str() {
        Some("certain") => Guven::Certain,
        Some("observed") => Guven::Observed,
        Some("inferred") => Guven::Inferred,
        _ => Guven::Unknown,
    }
}

/// Belgeden alıntı maddesi: "README.md:3 alıntısı: “…”"
fn alinti(x: &Value) -> String {
    let (konum, metin) = (x["konum"].as_str().unwrap_or(""), x["alinti"].as_str().unwrap_or(""));
    tf!("{konum} alıntısı: “{metin}”", "Quoted from {konum}: “{metin}”")
}

/// Neden var: uygulamanın kendi beyanı (README, paket tanımı, OpenAPI başlığı), alıntı olarak; doğrulanmış sayılmaz
fn amac(v: &Veri) -> Value {
    let a = anlam_ozeti(v);
    let m: Vec<Value> = a["amac"].as_array().into_iter().flatten().map(|x| madde(alinti(x), guven_degeri(&x["guven"]))).collect();
    if m.is_empty() {
        return soru(
            "amac",
            vec![],
            Some(
                t!(
                    "README'de, paket tanımında ya da OpenAPI belgesinde uygulamayı anlatan bir cümle bulunamadı; Sezgi amacı koddan uydurmaz. Uygulamanın sahibine sorun.",
                    "No sentence describing the application was found in the README, the package definition or the OpenAPI document; Sezgi does not invent a purpose from code. Ask the app's owner."
                )
                .into(),
            ),
            None,
            None,
        );
    }
    soru(
        "amac",
        m,
        Some(
            t!(
                "Alıntı uygulamanın kendi beyanıdır (belge ya da paket tanımı); Sezgi bunu doğrulamaz, yorum sayar.",
                "The quote is the application's own statement (a document or the package definition); Sezgi does not verify it and treats it as inferred."
            )
            .into(),
        ),
        None,
        None,
    )
}

/// Alan ya da tür tanımından gelen problem metni seçilen dilde (`metin` / `metin_en`)
fn problem_metni(p: &Value) -> &str {
    let tr = p["metin"].as_str().unwrap_or("");
    if crate::dil::en() {
        p["metin_en"].as_str().unwrap_or(tr)
    } else {
        tr
    }
}

/// Hangi iş problemini çözer: sözlükteki alan ya da tür tanımı ve belgede fayda anlatan cümle (alıntı)
fn problem(v: &Veri) -> Value {
    let a = anlam_ozeti(v);
    let m: Vec<Value> = a["problem"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let metin = problem_metni(p);
            match p["dayanak"].as_str() {
                Some("alinti") => madde(alinti(p), guven_degeri(&p["guven"])),
                Some("tur") => madde(tf!("{metin} (uygulamanın türünden)", "{metin} (from the kind of application)"), guven_degeri(&p["guven"])),
                _ => madde(tf!("{metin} (iş alanından)", "{metin} (from the business domain)"), guven_degeri(&p["guven"])),
            }
        })
        .collect();
    if m.is_empty() {
        return soru(
            "problem",
            vec![],
            Some(
                t!(
                    "Çözdüğü iş problemi bilinmiyor: iş alanı sınıflandırılamadı ve belgede fayda anlatan bir cümle yok. \"Kullanıcı ne yapabilir?\" yanıtı bir başlangıçtır; uygulamanın sahibine sorun.",
                    "The business problem it solves is unknown: the domain could not be classified and no document sentence describes a benefit. The answer to \"What can a user do?\" is a start; ask the app's owner."
                )
                .into(),
            ),
            None,
            None,
        );
    }
    soru(
        "problem",
        m,
        Some(
            t!(
                "İş problemi alan sınıflandırmasından ve uygulamanın kendi beyanından çıkarılır (yorum); gerçek iş ihtiyacını uygulamanın sahibi doğrulamalı.",
                "The business problem is derived from the domain classification and the application's own statement (inferred); the app's owner should confirm the real business need."
            )
            .into(),
        ),
        None,
        None,
    )
}

fn kullanicilar(v: &Veri) -> Value {
    let mut m = vec![];
    let mut roller: BTreeMap<String, usize> = BTreeMap::new();
    for (k, _) in &v.kapilar {
        for r in k["roller"].as_array().into_iter().flatten().filter_map(|r| r.as_str()) {
            *roller.entry(r.to_string()).or_default() += 1;
        }
    }
    let rollu = |k: &Value| k["roller"].as_array().is_some_and(|r| !r.is_empty());
    if !roller.is_empty() {
        m.push(ornekli(
            tf!("Kodda {} rol ya da izin adı geçiyor", "The code names {} {r}", roller.len(); r = cogul(roller.len(), "role or permission", "roles or permissions")),
            v.guveni("ROLLER", rollu),
            roller.keys().take(12).cloned().collect(),
        ));
    } else if !v.kapilar.is_empty() {
        let g = v.guveni("ROLLER", |_| true);
        if g > Guven::Unknown {
            m.push(madde(t!("Hiçbir işlemde rol ya da izin kısıtı bulunmadı", "No operation is restricted by a role or permission"), g));
        }
    }
    let giris: Vec<String> =
        v.rapor["kapilar"]["giris"].as_object().into_iter().flatten().filter(|(a, _)| a.as_str() != "bilinmiyor").map(|(a, n)| format!("{n} {}", giris_adi(a))).collect();
    if !giris.is_empty() {
        m.push(madde(tf!("Giriş: {}", "Login: {}", giris.join(", ")), v.guveni("GIRIS", |k| k["giris"] != "bilinmiyor")));
    }
    let a = anlam_ozeti(v);
    for x in a["aktorler"].as_array().into_iter().flatten() {
        let ornekler: Vec<String> = x["ornekler"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
        m.push(ornekli(
            tf!("Kullanıcı tipi: {} ({} işlem)", "User type: {} ({} {i})", ad_dilde(x), x["islem_sayisi"]; i = cogul(&x["islem_sayisi"], "operation", "operations")),
            guven_degeri(&x["guven"]),
            ornekler,
        ));
    }
    for x in a["akislar"].as_array().into_iter().flatten() {
        let ornekler: Vec<String> = x["islemler"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
        m.push(ornekli(tf!("Akış: {}", "Flow: {}", ad_dilde(x)), guven_degeri(&x["guven"]), ornekler));
    }
    soru(
        "kullanicilar",
        m,
        Some(
            t!(
                "Kullanıcı tipleri giriş kararından, yol adlarından (admin) ve rol adlarından çıkarılır (yorum); uygulamanın kendi kullanıcı modeli farklı olabilir.",
                "User types are derived from the login decision, path names (admin) and role names (inferred); the application's own user model may differ."
            )
            .into(),
        ),
        None,
        None,
    )
}

/// Giriş kararının okunur adı (sayımda: "12 giriş gerekli")
fn giris_adi(g: &str) -> &str {
    match g {
        "gerekli" => t!("giriş gerekli", "login required"),
        "herkese_acik" => t!("girişsiz", "public"),
        "istege_bagli" => t!("giriş isteğe bağlı", "login optional"),
        x => x,
    }
}

fn yapabilir(v: &Veri) -> Value {
    if v.kapilar.is_empty() {
        let neden = if v.kaynaksiz() {
            t!(
                "Kaynaksız keşifte çağrılabilir işlem çıkarılmaz: yalnızca ikilinin beyan ettiği yapı bilinir.",
                "Discovery without source code yields no callable operations: only the structure the binary declares is known."
            )
        } else {
            t!("Haritada hiç işlem bulunamadı.", "The map has no operations.")
        };
        return soru(
            "yapabilir",
            vec![],
            Some(neden.into()),
            (!v.kaynaksiz()).then(|| komut("sezgi rapor", t!("neyin okunamadığını listeler", "lists what could not be read"))),
            None,
        );
    }
    let mut m = vec![];
    let mut moduller: BTreeMap<String, usize> = BTreeMap::new();
    let mut risk: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut kesin_ozet, mut addan) = (0usize, 0usize);
    for (k, _) in &v.kapilar {
        *moduller.entry(modul(k)).or_default() += 1;
        *risk.entry(crate::canli::islem_bilgisi(k).1).or_default() += 1;
        if crate::mcp::aciklama(k).1 {
            kesin_ozet += 1;
        } else {
            addan += 1;
        }
    }
    let mut sirali: Vec<(String, usize)> = moduller.into_iter().collect();
    sirali.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    // Okunamayan rota kaynağı varsa liste eksiktir: sayı kesin değil (en fazla yorum), eksiklik ayrı madde
    let okunamayan = crate::adaptor::okunamayan_rota_kaynaklari(&v.harita["ozet"]);
    let sayi_guveni = if okunamayan.is_empty() { v.guveni("VAR", |_| true) } else { v.guveni("VAR", |_| true).min(Guven::Inferred) };
    m.push(ornekli(
        if okunamayan.is_empty() {
            tf!("{} işlem tanımlı", "{} {i} defined", v.kapilar.len(); i = cogul(v.kapilar.len(), "operation", "operations"))
        } else {
            tf!("en az {} işlem tanımlı (liste kısmen)", "at least {} {i} defined (partial list)", v.kapilar.len(); i = cogul(v.kapilar.len(), "operation", "operations"))
        },
        sayi_guveni,
        sirali.iter().take(10).map(|(a, n)| tf!("{a}: {n} işlem", "{a}: {n} {i}"; i = cogul(n, "operation", "operations"))).collect(),
    ));
    if let Some(u) = crate::adaptor::okunamayan_rota_uyarisi(&okunamayan) {
        m.push(ornekli(u, Guven::Unknown, okunamayan.iter().take(6).map(|x| crate::adaptor::okunamayan_rota_satiri(x)).collect()));
    }
    let say = |r: &str| risk.get(r).copied().unwrap_or(0);
    m.push(madde(
        tf!("{} okuma, {} yazma, {} silme (türü HTTP yönteminden)", "{} {o}, {} {y}, {} {s} (kind from the HTTP method)", say("oku"), say("yaz"), say("yikici"); o = cogul(say("oku"), "read", "reads"), y = cogul(say("yaz"), "write", "writes"), s = cogul(say("yikici"), "delete", "deletes")),
        Guven::Inferred,
    ));
    if kesin_ozet > 0 {
        m.push(madde(tf!("{kesin_ozet} işlemin ne yaptığı koddaki özetinden", "What {kesin_ozet} {i} {d} comes from their summary in the code"; i = cogul(kesin_ozet, "operation", "operations"), d = cogul(kesin_ozet, "does", "do")), Guven::Certain));
    }
    if addan > 0 {
        m.push(madde(tf!("{addan} işlemin açıklaması yalnızca fonksiyon adından okundu", "{addan} {i} {b} described only from their function name"; i = cogul(addan, "operation", "operations"), b = cogul(addan, "is", "are")), Guven::Inferred));
    }
    let yolculuk: Vec<&Onerme> = v.model.map(|md| md.depo.onermeler.iter().filter(|o| o.yuklem == "VAR" && o.varlik.starts_with("journey:")).collect()).unwrap_or_default();
    if !yolculuk.is_empty() {
        m.push(ornekli(
            tf!("{} kullanıcı yolculuğu (ekran → işlem adımları)", "{} user journeys (screen → operation steps)", yolculuk.len()),
            en_zayif(yolculuk.iter().map(|o| etkin(o)), Guven::Unknown),
            yolculuk.iter().take(6).map(|o| o.deger["ad"].as_str().unwrap_or(crate::graf::ad(&o.varlik)).to_string()).collect(),
        ));
    }
    let a = anlam_ozeti(v);
    let gruplar = crate::anlam::kaynak_gruplari(&a, crate::dil::en());
    if !gruplar.is_empty() {
        let n = a["yetenekler"].as_array().map(Vec::len).unwrap_or(0);
        let g = en_zayif(a["yetenekler"].as_array().into_iter().flatten().map(|y| guven_degeri(&y["guven"])), Guven::Unknown);
        m.push(ornekli(
            tf!(
                "{n} iş yeteneği, {} kaynakta (işlemler iş senaryolarına gruplandı)",
                "{n} business {y} over {} {k} (operations grouped into business scenarios)",
                gruplar.len();
                y = cogul(n, "capability", "capabilities"),
                k = cogul(gruplar.len(), "resource", "resources")
            ),
            g,
            gruplar
                .iter()
                .take(8)
                .map(|(k, ads, say)| {
                    let ads = ads.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
                    tf!("{k}: {ads} ({say} işlem)", "{k}: {ads} ({say} {i})"; i = cogul(say, "operation", "operations"))
                })
                .collect(),
        ));
    }
    soru(
        "yapabilir",
        m,
        Some(
            t!(
                "İş yetenekleri yoldan, fonksiyon adından ve HTTP yönteminden çıkarılır (yorum); birden fazla işlemden oluşan senaryoların sırası ancak bir görev planında kurulur.",
                "Business capabilities are derived from the path, the function name and the HTTP method (inferred); the order of a scenario made of several operations is set only in a task plan."
            )
            .into(),
        ),
        None,
        None,
    )
}

fn calisiyor(v: &Veri) -> Value {
    let gozlenen: Vec<&str> = v
        .kapilar
        .iter()
        .filter(|(_, a)| v.model.is_some_and(|md| md.onermeler(a).iter().any(|o| etkin(o) == Guven::Observed)))
        .map(|(k, _)| k["id"].as_str().unwrap_or(""))
        .collect();
    if gozlenen.is_empty() {
        return soru(
            "calisiyor",
            vec![],
            Some(
                t!(
                    "Haritadaki bilgi koddan çıkarıldı: bir işlemin var olduğunu söyler, çalıştığını söylemez. Bu uygulama için çalışma anı gözlemi (ağ kaydı, tarayıcı senaryosu) yok.",
                    "The map comes from the code: it says an operation exists, not that it works. There is no run-time observation (network log, browser scenario) for this application."
                )
                .into(),
            ),
            Some(komut("sezgi gozlem al <dosya.har>", t!("tarayıcının ağ kaydını gözlem kanıtı olarak alır", "imports the browser's network log as observed evidence"))),
            None,
        );
    }
    let kalan = v.kapilar.len() - gozlenen.len();
    soru(
        "calisiyor",
        vec![ornekli(
            tf!("{} işlem çalışma anında gözlendi", "{} {i} observed at run time", gozlenen.len(); i = cogul(gozlenen.len(), "operation", "operations")),
            Guven::Observed,
            gozlenen.iter().take(8).map(|s| s.to_string()).collect(),
        )],
        (kalan > 0).then(|| tf!("Gözlenmeyen {kalan} işlemin çalıştığı bilinmiyor.", "Whether the {kalan} unobserved operations work is unknown.")),
        (kalan > 0).then(|| komut("sezgi gozlem al <dosya.har>", t!("tarayıcının ağ kaydını gözlem kanıtı olarak alır", "imports the browser's network log as observed evidence"))),
        None,
    )
}

fn ekran(v: &Veri) -> Value {
    let Some(md) = v.model else {
        return soru("ekran", vec![], Some(depo_yok()), Some(komut("sezgi init", yeniden_cikarir())), None);
    };
    if !md.graf.tur_var(&["screen", "ui", "comp", "client"]) {
        return soru(
            "ekran",
            vec![],
            Some(
                t!(
                    "Ön yüz kodu okunmadı: ekranlar ve hangi düğmenin hangi işlemi çağırdığı bilinmiyor.",
                    "The front-end code was not read: the screens, and which button calls which operation, are unknown."
                )
                .into(),
            ),
            Some(komut(
                t!("sezgi init --on-yuz <ön yüz klasörü>", "sezgi init --on-yuz <front-end folder>"),
                t!("ön yüz kodunu da okur, ekran → işlem bağlarını çıkarır", "also reads the front-end code and links screens to operations"),
            )),
            None,
        );
    }
    let (mut bagli, mut ekransiz): (Vec<(String, Guven)>, usize) = (vec![], 0);
    for (k, a) in &v.kapilar {
        let e = ekranlar(md, a);
        match e.iter().max_by_key(|(_, g)| **g) {
            Some((s, g)) => bagli.push((tf!("{} ekranı → {}", "{} screen → {}", ekran_adi(s), k["id"].as_str().unwrap_or("")), *g)),
            None if md.graf.gelen(a).iter().any(|&i| matches!(crate::graf::tur(&md.depo.onermeler[i].varlik), "client" | "ui")) => ekransiz += 1,
            None => {}
        }
    }
    let mut m = vec![];
    if !bagli.is_empty() {
        m.push(ornekli(
            tf!("{} işlemin başladığı ekran bulundu", "The starting screen was found for {} {i}", bagli.len(); i = cogul(bagli.len(), "operation", "operations")),
            en_zayif(bagli.iter().map(|(_, g)| *g), Guven::Unknown),
            bagli.iter().take(8).map(|(s, _)| s.clone()).collect(),
        ));
    }
    if ekransiz > 0 {
        m.push(madde(
            tf!("{ekransiz} işlem ön yüz kodundan çağrılıyor ama ekranı bulunamadı", "{ekransiz} {i} called from front-end code but {e} screen was not found"; i = cogul(ekransiz, "operation is", "operations are"), e = cogul(ekransiz, "its", "their")),
            Guven::Inferred,
        ));
    }
    let bagsiz = v.kapilar.len() - bagli.len() - ekransiz;
    soru(
        "ekran",
        m,
        Some(if bagsiz > 0 {
            tf!(
                "{bagsiz} işlemin ön yüzle bağı bulunamadı (yokluk kanıtı değildir). Ekran ve düğme bağları içe aktarma ve olay işleyicisinden çıkarılır: en fazla yorum.",
                "No front-end link was found for {bagsiz} {i} (not proof of absence). Screen and button links are derived from imports and event handlers: at most inferred.";
                i = cogul(bagsiz, "operation", "operations")
            )
        } else {
            t!(
                "Ekran ve düğme bağları içe aktarma ve olay işleyicisinden çıkarılır: en fazla yorum.",
                "Screen and button links are derived from imports and event handlers: at most inferred."
            )
            .into()
        }),
        None,
        None,
    )
}

fn yetki(v: &Veri) -> Value {
    let mut m = vec![];
    let mut bilinmeyen = vec![];
    let (mut kesin, mut yorum, mut yok) = (0usize, 0usize, 0usize);
    for (_, a) in &v.kapilar {
        match v.onermeler(a, "GIRIS").iter().map(|o| etkin(o)).max() {
            Some(Guven::Certain) => kesin += 1,
            Some(Guven::Observed | Guven::Inferred) => yorum += 1,
            _ => yok += 1,
        }
    }
    if kesin > 0 {
        m.push(madde(
            tf!("{kesin} işlemde giriş gereksinimi koddaki açık işaretten", "For {kesin} {i} the login requirement comes from an explicit marker in the code"; i = cogul(kesin, "operation", "operations")),
            Guven::Certain,
        ));
    }
    if yorum > 0 {
        m.push(madde(
            tf!(
                "{yorum} işlemde giriş kararı çıkarım (açık bir işaret yok ya da doğrulanamadı)",
                "For {yorum} {i} the login decision is inferred (no explicit marker, or it could not be verified)";
                i = cogul(yorum, "operation", "operations")
            ),
            Guven::Inferred,
        ));
    }
    if yok > 0 {
        bilinmeyen.push(tf!("{yok} işlemin giriş gereksinimi çözülemedi.", "The login requirement of {yok} {i} could not be resolved."; i = cogul(yok, "operation", "operations")));
    }
    let mut roller: BTreeMap<String, (usize, Guven)> = BTreeMap::new();
    for (k, a) in &v.kapilar {
        let g = en_zayif(v.onermeler(a, "ROLLER").iter().map(|o| etkin(o)), Guven::Unknown);
        for r in k["roller"].as_array().into_iter().flatten().filter_map(|r| r.as_str()) {
            let e = roller.entry(r.to_string()).or_insert((0, Guven::Certain));
            e.0 += 1;
            e.1 = e.1.min(g);
        }
    }
    if !roller.is_empty() {
        // En çok işlemi kısıtlayan roller önce
        let mut sirali: Vec<(&String, &(usize, Guven))> = roller.iter().collect();
        sirali.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));
        let rollu_sayi = v.kapilar.iter().filter(|(k, _)| k["roller"].as_array().is_some_and(|r| !r.is_empty())).count();
        m.push(ornekli(
            tf!(
                "{} işlem rol ya da izin istiyor ({} farklı rol ya da izin)",
                "{} {i} a role or permission ({} distinct {r})",
                rollu_sayi,
                roller.len();
                i = cogul(rollu_sayi, "operation requires", "operations require"),
                r = cogul(roller.len(), "role or permission", "roles or permissions")
            ),
            en_zayif(roller.values().map(|x| x.1), Guven::Unknown),
            sirali.iter().take(10).map(|(r, (n, _))| tf!("{r}: {n} işlem", "{r}: {n} {i}"; i = cogul(n, "operation", "operations"))).collect(),
        ));
    }
    // Nesne düzeyi yetki: servis katmanındaki erişim denetimi (çağrı grafiğinden: requireAccess, authorize($kayit), ...)
    // ya da kaydın sahibinin karşılaştırılması. Sayım haritanın davranış özetinden (`rapor` ile aynı kural)
    let mut nesne: Vec<(&str, Guven)> = vec![];
    let mut sahiplik: Vec<(&str, Guven)> = vec![];
    let mut erisim: Vec<(String, Guven)> = vec![];
    for (k, _) in &v.kapilar {
        let d = v.dna(k);
        if !nesne_denetimli(d) {
            continue;
        }
        let id = k["id"].as_str().unwrap_or("");
        let e: Vec<&Value> = d["erisim_denetimi"].as_array().into_iter().flatten().filter(|x| x["nesne_duzeyi"] == true).collect();
        let s: Vec<&Value> = d["sahiplik_denetimi"].as_array().into_iter().flatten().collect();
        let ge = en_zayif(e.iter().map(|x| guven_degeri(&x["guven"])), Guven::Unknown);
        let gs = en_zayif(s.iter().map(|x| guven_degeri(&x["guven"])), Guven::Unknown);
        if let Some(x) = e.first() {
            erisim.push((format!("{id}: {}", x["izin"].as_str().or(x["ifade"].as_str()).unwrap_or("")), ge));
        }
        if !s.is_empty() {
            sahiplik.push((id, gs));
        }
        nesne.push((
            id,
            if e.is_empty() {
                gs
            } else if s.is_empty() {
                ge
            } else {
                ge.min(gs)
            },
        ));
    }
    if nesne.is_empty() {
        bilinmeyen.push(
            t!(
                "Nesne düzeyi yetki (kayıt bu kullanıcının mı, ona açık mı) denetimi bulunamadı; yokluk kanıtı değildir.",
                "No object-level authorization check (is the record this user's, is it open to them) was found; this is not proof of absence."
            )
            .into(),
        );
    } else {
        m.push(ornekli(
            tf!(
                "{} işlemde nesne düzeyi yetki denetleniyor: kayıt bu kullanıcıya açık mı ya da onun mu",
                "{} {i} object-level authorization: whether the record is open to this user or belongs to them",
                nesne.len();
                i = cogul(nesne.len(), "operation checks", "operations check")
            ),
            en_zayif(nesne.iter().map(|x| x.1), Guven::Unknown),
            nesne.iter().take(8).map(|x| x.0.to_string()).collect(),
        ));
        if !erisim.is_empty() {
            m.push(ornekli(
                tf!(
                    "{} işlemde servis katmanında nesne düzeyi erişim denetimi çağrılıyor (izniyle)",
                    "{} {i} an object-level access check in the service layer (with its permission)",
                    erisim.len();
                    i = cogul(erisim.len(), "operation calls", "operations call")
                ),
                en_zayif(erisim.iter().map(|x| x.1), Guven::Unknown),
                erisim.iter().take(6).map(|x| x.0.clone()).collect(),
            ));
        }
        if !sahiplik.is_empty() {
            m.push(ornekli(
                tf!("{} işlemde kaydın sahibi denetleniyor", "{} {i} the record's owner", sahiplik.len(); i = cogul(sahiplik.len(), "operation checks", "operations check")),
                en_zayif(sahiplik.iter().map(|x| x.1), Guven::Unknown),
                sahiplik.iter().take(8).map(|x| x.0.to_string()).collect(),
            ));
        }
    }
    let yardim = if m.is_empty() && !v.kaynaksiz() { Some(komut("sezgi rapor", t!("neyin okunamadığını listeler", "lists what could not be read"))) } else { None };
    if v.kaynaksiz() && m.is_empty() {
        bilinmeyen = vec![t!("Kaynaksız keşifte işlem ve yetki kararı çıkarılmaz.", "Discovery without source code yields no operations or authorization decisions.").into()];
    }
    soru("yetki", m, (!bilinmeyen.is_empty()).then(|| bilinmeyen.join(" ")), yardim, None)
}

fn tablolar(v: &Veri) -> Value {
    let veri = &v.rapor["veri"];
    if veri["bagli_kapi"].is_null() {
        return soru(
            "tablolar",
            vec![],
            Some(
                t!(
                    "Veritabanı tanımı ve tablo bağı bulunamadı: desteklenen bir ORM ya da sorgu görülmedi.",
                    "No database definition or table link was found: no supported ORM or query was seen."
                )
                .into(),
            ),
            Some(komut("sezgi rapor", t!("neyin bilinmediğini listeler", "lists what is unknown"))),
            None,
        );
    }
    let mut m = vec![];
    let mut yazan: BTreeMap<String, (usize, Guven)> = BTreeMap::new();
    let mut okunan: BTreeSet<String> = BTreeSet::new();
    let (mut dolayli, mut dolayli_g) = (0usize, Guven::Certain);
    for (_, a) in &v.kapilar {
        for o in v.onermeler(a, "WRITES") {
            let e = yazan.entry(crate::graf::ad(o.hedef.as_deref().unwrap_or("")).to_string()).or_insert((0, Guven::Certain));
            e.0 += 1;
            e.1 = e.1.min(etkin(o));
        }
        for o in v.onermeler(a, "READS") {
            okunan.insert(crate::graf::ad(o.hedef.as_deref().unwrap_or("")).to_string());
        }
        let d = v.onermeler(a, "MAY_AFFECT");
        if !d.is_empty() {
            dolayli += 1;
            dolayli_g = dolayli_g.min(en_zayif(d.iter().map(|o| etkin(o)), Guven::Unknown));
        }
    }
    if !yazan.is_empty() {
        let mut sirali: Vec<(&String, &(usize, Guven))> = yazan.iter().collect();
        sirali.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));
        m.push(ornekli(
            tf!("{} tabloya yazılıyor", "{} {t} written", yazan.len(); t = cogul(yazan.len(), "table is", "tables are")),
            en_zayif(yazan.values().map(|x| x.1), Guven::Unknown),
            sirali.iter().take(10).map(|(t, (n, _))| tf!("{t}: {n} işlem yazar", "{t}: written by {n} {i}"; i = cogul(n, "operation", "operations"))).collect(),
        ));
    }
    if !okunan.is_empty() {
        m.push(ornekli(
            tf!("{} tablo okunuyor", "{} {t} read", okunan.len(); t = cogul(okunan.len(), "table is", "tables are")),
            v.guveni("READS", |_| true),
            okunan.iter().take(10).cloned().collect(),
        ));
    }
    if dolayli > 0 {
        m.push(madde(
            tf!(
                "{dolayli} işlem silme kaskadıyla başka tablolara da dokunabilir (şemadaki ilişkiden)",
                "{dolayli} {i} may also touch other tables through a delete cascade (from the schema relation)";
                i = cogul(dolayli, "operation", "operations")
            ),
            dolayli_g,
        ));
    }
    // Servis ve depo katmanına inilerek (çağrı grafiği) bulunan tablo etkileri
    let zincirli: Vec<String> = v
        .kapilar
        .iter()
        .filter_map(|(k, _)| {
            let d = &v.harita["ozet"]["dna"][format!("{}:{}", k["dosya"].as_str().unwrap_or(""), k["satir"]).as_str()];
            let t = d["tablo_etkisi"].as_array().filter(|t| !t.is_empty())?;
            let ilk = t.iter().find(|x| x["islem"] == "yazar").unwrap_or(&t[0]);
            let zincir: Vec<&str> = ilk["zincir"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
            Some(format!("{} → {} ({})", k["id"].as_str().unwrap_or(""), ilk["tablo"].as_str().unwrap_or(""), zincir.join(" › ")))
        })
        .collect();
    if !zincirli.is_empty() {
        m.push(ornekli(
            tf!(
                "{} işlemde tablo etkisi servis ve depo katmanına inilerek bulundu (çağrı zinciri)",
                "For {} {i} the table effect was found by following calls into the service and repository layers (call chain)",
                zincirli.len();
                i = cogul(zincirli.len(), "operation", "operations")
            ),
            Guven::Inferred,
            zincirli.into_iter().take(4).collect(),
        ));
    }
    match veri["tablo_kaynagi"].as_str() {
        Some("sema") => m.push(ornekli(
            t!("Tablo tanımları veri şemasından", "Table definitions come from the data schema"),
            Guven::Certain,
            veri["sema_dosyalari"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect(),
        )),
        Some("sorgu") => m.push(madde(t!("Şema dosyası yok: tablo adları yalnızca sorgulardan", "No schema file: table names come from queries only"), Guven::Inferred)),
        _ => {}
    }
    let bagsiz = veri["bagsiz_kapilar"].as_array().map(Vec::len).unwrap_or(0);
    let mut not = t!(
        "Çağrı zinciri en çok 8 kat izlenir; kütüphane içi ve hedefi bilinmeyen (dinamik) çağrılar izlenmez: liste eksik olabilir.",
        "The call chain is followed at most 8 levels deep; calls inside libraries and calls with an unknown (dynamic) target are not followed: the list may be incomplete."
    )
    .to_string();
    if bagsiz > 0 {
        not = tf!(
            "{bagsiz} işlemde tablo bağı bulunamadı (dış servis ya da izlenemeyen çağrı olabilir). {not}",
            "No table link was found for {bagsiz} {i} (an external service or an untraceable call, perhaps). {not}";
            i = cogul(bagsiz, "operation", "operations")
        );
    }
    soru("tablolar", m, Some(not), None, None)
}

fn onkosul(v: &Veri) -> Value {
    let g = &v.rapor["girdiler"];
    let mut m = vec![];
    let (alan, kural) = (g["alan"].as_u64().unwrap_or(0), g["kural"].as_u64().unwrap_or(0));
    if alan > 0 {
        // Çözülemeyen şema bilinmeyende ayrıca söylenir: çözülen alanların damgasını ezmez
        let gv = en_zayif(
            v.kapilar.iter().flat_map(|(_, a)| v.onermeler(a, "GOVDE").into_iter().chain(v.onermeler(a, "SORGU"))).map(etkin).filter(|g| *g > Guven::Unknown),
            Guven::Unknown,
        );
        m.push(madde(
            tf!(
                "{} işlemin girdisi var: {alan} alan, {kural} doğrulama kuralı",
                "{} {i} input: {alan} {a}, {kural} validation {k}",
                g["girdili_kapi"].as_u64().unwrap_or(0);
                i = cogul(&g["girdili_kapi"], "operation takes", "operations take"),
                a = cogul(alan, "field", "fields"),
                k = cogul(kural, "rule", "rules")
            ),
            gv,
        ));
    }
    // İş durumu ön koşulları: işlemin ve çağırdığı servislerin koşullu redleri (hata yakalama blokları hariç). Sayım
    // haritanın davranış özetinden, `rapor`un `on_kosullu_islem` / `on_kosul` sayımıyla aynı kural
    let mut tur: BTreeMap<&str, (BTreeSet<&str>, usize, Guven)> = BTreeMap::new();
    let (mut islem, mut toplam, mut toplam_g) = (vec![], 0usize, Guven::Certain);
    for (k, _) in &v.kapilar {
        let redler: Vec<&Value> = v.dna(k)["hatalar"].as_array().into_iter().flatten().filter(|h| on_kosul_mu(h)).collect();
        if redler.is_empty() {
            continue;
        }
        islem.push(k["id"].as_str().unwrap_or(""));
        toplam += redler.len();
        for h in redler {
            let gh = guven_degeri(&h["guven"]);
            toplam_g = toplam_g.min(gh);
            let ad = match h["durum"].as_u64() {
                Some(404) => t!("kayıt bulunmalı (404)", "the record must exist (404)"),
                Some(409) => t!("çakışma olmamalı (409)", "there must be no conflict (409)"),
                Some(400 | 412 | 422) => t!("durum ya da değer uygun olmalı (400 / 412 / 422)", "the state or value must be valid (400 / 412 / 422)"),
                Some(401 | 403) => t!("yetki ya da kaydın sahibi (401 / 403)", "authorization or the record's owner (401 / 403)"),
                Some(_) => t!("diğer red kodları", "other rejection codes"),
                None => t!("red kodu koddan çıkmayan koşul", "a condition whose rejection code is not in the code"),
            };
            let e = tur.entry(ad).or_insert((BTreeSet::new(), 0, Guven::Certain));
            e.0.insert(k["id"].as_str().unwrap_or(""));
            e.1 += 1;
            e.2 = e.2.min(gh);
        }
    }
    if toplam > 0 {
        m.push(ornekli(
            tf!(
                "{} işlemde {toplam} ön koşul (koşul sağlanmazsa istek reddedilir)",
                "{toplam} {k} in {} {i} (the request is rejected if a condition fails)",
                islem.len();
                k = cogul(toplam, "precondition", "preconditions"),
                i = cogul(islem.len(), "operation", "operations")
            ),
            toplam_g,
            islem.iter().take(8).map(|s| s.to_string()).collect(),
        ));
    }
    let mut turler: Vec<_> = tur.iter().collect();
    turler.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(a.0.cmp(b.0)));
    for (ad, (islemler, n, g)) in turler {
        m.push(ornekli(tf!("{} işlemde {n} koşul: {ad}", "{n} {k} in {} {i}: {ad}", islemler.len(); k = cogul(n, "condition", "conditions"), i = cogul(islemler.len(), "operation", "operations")), *g, islemler.iter().take(6).map(|s| s.to_string()).collect()));
    }
    let dal = v.kapilar.iter().filter(|(_, a)| !v.onermeler(a, "DAL").is_empty()).count();
    if dal > 0 {
        m.push(madde(
            tf!("{dal} işlemde sonuç bir koşula göre değişiyor (iki yollu koşul)", "In {dal} {i} the outcome depends on a condition (a two-way branch)"; i = cogul(dal, "operation", "operations")),
            v.guveni("DAL", |_| true),
        ));
    }
    let mut not = vec![];
    let denetlenemeyen: u64 = g["onizlemede_kontrol_edilemeyen_kurallar"].as_object().into_iter().flatten().filter_map(|(_, n)| n.as_u64()).sum();
    if denetlenemeyen > 0 {
        not.push(tf!("{denetlenemeyen} kural önizlemede denetlenemiyor (özel doğrulayıcı).", "{denetlenemeyen} {k} cannot be checked by the preview (custom validator)."; k = cogul(denetlenemeyen, "rule", "rules")));
    }
    let coz = g["cozulemeyen_semalar"].as_array().map(Vec::len).unwrap_or(0);
    if coz > 0 {
        not.push(tf!("{coz} girdi şeması çözülemedi: alanları bilinmiyor.", "{coz} input schemas could not be resolved: their fields are unknown."));
    }
    not.push(
        t!(
            "Ön koşullar (ör. kayıt var mı, stok yeterli mi) işlemde ve çağırdığı servislerde koşullu red olarak görünüyorsa listelenir; önizleme bunları denetleyemez ve böyle bir işlemde GEÇER demez. Listede olmayan koşul \"yok\" demek değildir.",
            "Preconditions (e.g. does the record exist, is there enough stock) are listed when they appear as conditional rejections in the operation or the services it calls; the preview cannot check them and never says PASS for such an operation. A condition missing from the list does not mean \"none\"."
        )
        .into(),
    );
    soru("onkosul", m, Some(not.join(" ")), None, None)
}

fn geri(v: &Veri) -> Value {
    let yazma: Vec<&(&Value, String)> = v.kapilar.iter().filter(|(k, _)| crate::canli::islem_bilgisi(k).1 != "oku").collect();
    if yazma.is_empty() {
        let neden = if v.kapilar.is_empty() {
            t!("Haritada işlem yok.", "The map has no operations.")
        } else {
            t!("Haritada yazma işlemi yok (türü HTTP yönteminden).", "The map has no write operations (kind from the HTTP method).")
        };
        return soru("geri", vec![], Some(neden.into()), None, None);
    }
    // Karar başına işlemler: evet (yumuşak silme + geri yükleme ucu, ters işlem), kismen, hayir (kalıcı silme, geri
    // alınamaz dış etki); hepsi çıkarım. Eski önerme bilinmiyor sayılır
    let mut kararlar: BTreeMap<&str, Vec<(&str, Guven)>> = BTreeMap::new();
    for (k, a) in &yazma {
        let Some((d, g)) = v.onermeler(a, "GERI_ALMA").iter().filter_map(|o| Some((o.deger["durum"].as_str()?, etkin(o)))).filter(|(_, g)| *g > Guven::Unknown).max_by_key(|x| x.1)
        else {
            continue;
        };
        kararlar.entry(d).or_default().push((k["id"].as_str().unwrap_or(""), g));
    }
    let mut m = vec![];
    for (d, metin) in [
        ("evet", t!("işlem geri alınabilir: yumuşak silme ve geri yükleme ucu ya da etkisini silen ters işlem var", "operations can be undone: a soft delete with a restore endpoint, or a reverse operation that removes the effect")),
        ("kismen", t!("işlem kısmen geri alınabilir: kayıt işaretlenir ama geri yükleme ucu yok, bir koşulda kalıcı ya da dış etkisi geri alınamaz", "operations can be partly undone: the record is flagged but there is no restore endpoint, it is permanent under a condition, or its external effect cannot be undone")),
        ("hayir", t!("işlem geri alınamaz: kalıcı silme (kaskad dahil) ya da uygulama dışına geri alınamaz etki (e-posta, ödeme, giden yazma)", "operations cannot be undone: a permanent delete (cascades included) or an effect outside the application that cannot be undone (e-mail, payment, outgoing write)")),
    ] {
        if let Some(l) = kararlar.get(d) {
            m.push(ornekli(format!("{} {metin}", l.len()), en_zayif(l.iter().map(|x| x.1), Guven::Unknown), l.iter().take(8).map(|x| x.0.to_string()).collect()));
        }
    }
    let kalan = yazma.len() - kararlar.values().map(Vec::len).sum::<usize>();
    let not = (kalan > 0).then(|| {
        tf!(
            "{kalan} yazma işleminin geri alınıp alınamayacağı koddan çıkarılamadı (ör. güncellemede önceki değerin saklanıp saklanmadığı). Karar yumuşak silme, geri yükleme ucu, kalıcı silme çağrısı, kaskad ve dış etkiden çıkarılır; en fazla yorumdur. Geri alınamaz ya da dış etkili yazma onaysız gitmez.",
            "Whether {kalan} write operations can be undone could not be derived from the code (e.g. whether an update keeps the previous value). The decision comes from soft deletes, restore endpoints, permanent delete calls, cascades and external effects; it is at most inferred. A write that cannot be undone or has an external effect never goes without approval."
        )
    });
    soru("geri", m, not, None, None)
}

/// Önizlemenin hiçbir zaman denetleyemediği kuralların kullanıcıya söylenen türleri
fn denetlenemeyen_turleri(maddeler: &[String]) -> Vec<&'static str> {
    let mut t: Vec<&'static str> = vec![];
    for m in maddeler {
        // Maddeler önizlemenin dilinde: iki dilin öneki de tanınır
        let ad = if m.starts_with("yetki (nesne düzeyi)") || m.starts_with("authorization (object level)") {
            t!("kaydın sahibi mi", "whether the caller owns the record")
        } else if m.starts_with("iş kuralı") || m.starts_with("business rule") {
            t!("servis içindeki iş kuralları (ör. kayıt var mı, benzersiz mi)", "business rules inside services (e.g. does the record exist, is it unique)")
        } else if m.starts_with("yetki") || m.starts_with("authorization") {
            t!("koddan kesinleşmeyen giriş ya da rol kararı", "a login or role decision the code does not settle")
        } else if m.contains("şeması çözülemedi") || m.contains("schema could not be resolved") {
            t!("çözülemeyen girdi şeması", "an unresolved input schema")
        } else {
            t!("özel doğrulama ya da dosya kuralları", "custom validation or file rules")
        };
        if !t.contains(&ad) {
            t.push(ad);
        }
    }
    t
}

fn onay(v: &Veri) -> Value {
    let (mut yikici, mut kesin_degil, mut celiskili, mut onizlemesiz, mut rol, mut serbest) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut nedenler: Vec<String> = vec![];
    // Dış sisteme etki eden ya da geri alınamaz işlemler (haritada `etki_onayi`; silmeler dahil, `rapor` ile aynı sayım)
    let (mut etkili, mut etkili_silme): (Vec<String>, usize) = (vec![], 0);
    // Dış etkisi bilinmeyen yazmalar (zincirde sınıflandırılamayan çağrı): fail-closed onay
    let mut bilinmeyen: Vec<String> = vec![];
    for (k, _) in &v.kapilar {
        let id = k["id"].as_str().unwrap_or("").to_string();
        if k["dis_etki"] == "bilinmiyor" && k["etki_onayi"].as_str().is_none_or(|g| g.starts_with("işlemin dış etkisi bilinmiyor")) && crate::canli::islem_bilgisi(k).1 == "yaz"
        {
            bilinmeyen.push(id);
            continue;
        }
        if k["etki_onayi"].is_string() {
            etkili_silme += usize::from(crate::canli::islem_bilgisi(k).1 == "yikici");
            etkili.push(id.clone());
        }
        match onay_sinifi(v, k) {
            OnaySinifi::Okuma | OnaySinifi::DisEtki(_) => {}
            OnaySinifi::Silme => yikici.push(id),
            OnaySinifi::KesinDegil => kesin_degil.push(id),
            OnaySinifi::Celiskili => celiskili.push(id),
            OnaySinifi::Onizlemesiz(n) => {
                nedenler.extend(n);
                onizlemesiz.push(id);
            }
            OnaySinifi::Rol => rol.push(id),
            OnaySinifi::Kosullu => serbest.push(id),
        }
    }
    if yikici.len() + etkili.len() + bilinmeyen.len() + kesin_degil.len() + celiskili.len() + onizlemesiz.len() + rol.len() + serbest.len() == 0 {
        let neden = if v.kapilar.is_empty() {
            t!("Haritada işlem yok.", "The map has no operations.")
        } else {
            t!("Haritada yazma işlemi yok: okumalar hiçbir zaman onaya düşmez.", "The map has no write operations: reads never need approval.")
        };
        return soru("onay", vec![], Some(neden.into()), None, None);
    }
    // Onay isteyenler Sezgi'nin kendi kuralından (canlı modun kullandığı fonksiyonlar): kesin. Onaysız gidebilecekler
    // yalnızca koşullu bir olasılıktır: karar her istekte yeniden verilir, en fazla yorum
    let mut m = vec![];
    let ornek = |x: &Vec<String>| x.iter().take(10).cloned().collect::<Vec<_>>();
    if !yikici.is_empty() {
        m.push(ornekli(
            tf!("{} silme işlemi her seferinde insan onayı ister", "{} delete operations need human approval every time", yikici.len()),
            Guven::Certain,
            ornek(&yikici),
        ));
    }
    if !etkili.is_empty() {
        let ek =
            if etkili_silme > 0 { tf!(" ({etkili_silme} silme dahil)", " ({etkili_silme} {s} included)"; s = cogul(etkili_silme, "delete", "deletes")) } else { String::new() };
        // Silme olmayanlar örnekte önce: silmeler zaten bir üstteki maddede
        let mut sirali: Vec<String> = etkili.iter().filter(|id| !id.starts_with("DELETE ")).cloned().collect();
        sirali.extend(etkili.iter().filter(|id| id.starts_with("DELETE ")).cloned());
        m.push(ornekli(
            tf!(
                "{} işlem{ek} her seferinde insan onayı ister: uygulamanın dışına etki ediyor ya da geri alınamaz",
                "{} {i}{ek} {n} human approval every time: {o} something outside the application or cannot be undone",
                etkili.len();
                i = cogul(etkili.len(), "operation", "operations"),
                n = cogul(etkili.len(), "needs", "need"),
                o = cogul(etkili.len(), "it affects", "they affect")
            ),
            Guven::Certain,
            ornek(&sirali),
        ));
    }
    if !bilinmeyen.is_empty() {
        m.push(ornekli(
            tf!(
                "{} yazma işlemi her seferinde insan onayı ister: dış etkisi bilinmiyor (zincirde sınıflandırılamayan çağrı)",
                "{} {i} human approval every time: {s} external effect is unknown (an unclassified call in the chain)",
                bilinmeyen.len();
                i = crate::dil::cogul(bilinmeyen.len(), "write operation needs", "write operations need"),
                s = crate::dil::cogul(bilinmeyen.len(), "its", "their")
            ),
            Guven::Certain,
            ornek(&bilinmeyen),
        ));
    }
    if !kesin_degil.is_empty() {
        m.push(ornekli(
            tf!("{} yazma işlemi onay ister: dayandığı kanıt kesin değil", "{} write operations need approval: the evidence behind them is not certain", kesin_degil.len()),
            Guven::Certain,
            ornek(&kesin_degil),
        ));
    }
    if !celiskili.is_empty() {
        m.push(ornekli(
            tf!(
                "{} yazma işlemi onay ister: kanıtı çelişkili (kaynaklar birbirini tutmuyor; çelişki giderilene kadar)",
                "{} write operations need approval: their evidence is contradictory (the sources disagree; until it is resolved)",
                celiskili.len()
            ),
            Guven::Certain,
            ornek(&celiskili),
        ));
    }
    if !onizlemesiz.is_empty() {
        m.push(ornekli(
            tf!(
                "{} yazma işlemi her istekte onay ister: önizleme bu işlemlerde şunları hiçbir zaman denetleyemez: {}",
                "{} write operations need approval on every request: for them the preview can never check {}",
                onizlemesiz.len(),
                denetlenemeyen_turleri(&nedenler).join("; ")
            ),
            Guven::Certain,
            ornek(&onizlemesiz),
        ));
    }
    if !rol.is_empty() {
        m.push(ornekli(
            tf!(
                "{} yazma işlemi rol istiyor: insanın doğrulanmış rolü yoksa onay ister (yapay zekânın rol beyanı doğrulanmış sayılmaz)",
                "{} write operations need a role: without the human's verified role they need approval (a role the AI claims is never treated as verified)",
                rol.len()
            ),
            Guven::Certain,
            ornek(&rol),
        ));
    }
    if !serbest.is_empty() {
        let kosul = ilk_harf_kucuk(t!(METIN_KOSULLU, METIN_KOSULLU_EN));
        let kosul = kosul.trim_end_matches('.');
        m.push(ornekli(tf!("{} yazma işlemi {kosul}", "{} write operations {kosul}", serbest.len()), Guven::Inferred, ornek(&serbest)));
    }
    // Onayı kimin verdiği nasıl kanıtlanır: bu bilgisayardaki sistem çapasından (Sezgi'nin kendi kuralı, kesin)
    m.push(madde(insan_kaniti_metni(), Guven::Certain));
    soru("onay", m, None, None, None)
}

/// Onayı veren insanın nasıl kanıtlandığı: bu bilgisayarın sistem çapasından (salt okuma)
fn insan_kaniti_metni() -> String {
    match crate::insan::gecerli_capa() {
        Some(c) => tf!(
            "Onayı veren insan gerçek veride sistem çapasındaki insan parolasıyla kanıtlanır (çapa {}; aynı hesaptaki yapay zekâ çapayı değiştiremez, parolayı bilmeden onaylayamaz); test kopyası bildirimi de imzalıdır",
            "On real data the approving human is proven by the human password in the system anchor (anchor {}; an AI on the same account cannot change the anchor or approve without the password); the test-copy declaration is signed too",
            c.izi
        ),
        None => t!(
            "Onayı veren insan için sistem çapası kurulu değil: gerçek veride yazma kapalı; test kopyasında insan kanıtı yalnızca terminal denetimidir (kabuk erişimi olan bir yapay zekâ sahte terminalle geçebilir)",
            "No system anchor is set up for the approving human: writes on real data are off; on a test copy the proof of a human is only a terminal check (an AI with shell access can pass it with a fake terminal)"
        )
        .to_string(),
    }
}

fn ilk_harf_kucuk(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().chain(c).collect()).unwrap_or_default()
}

fn dis(v: &Veri) -> Value {
    let mut tetik: BTreeMap<String, Guven> = BTreeMap::new();
    for (_, a) in &v.kapilar {
        for o in v.onermeler(a, "TRIGGERS") {
            if let Some(h) = o.hedef.as_deref().filter(|h| matches!(crate::graf::tur(h), "event" | "job")) {
                let e = tetik.entry(h.to_string()).or_insert(Guven::Unknown);
                *e = (*e).max(etkin(o));
            }
        }
    }
    // Dış sistemler: işlemden `external:` düğümüne CALLS kenarları; sistem başına en iyi güven ve işlem sayısı. Damga
    // hedefin çözümünden: sabit adres kesin, ortam değişkeni / yapılandırma yorum, çözülemeyen adres bilinmiyor
    // Sistem başına: en iyi kenar güveni, çağıran işlemler, türler ve hedefin türü (sabit adres, ortam, SDK, dinamik)
    type Sistem<'a> = (Guven, BTreeSet<&'a str>, BTreeSet<String>, String);
    let mut sistem: BTreeMap<String, Sistem> = BTreeMap::new();
    for (k, a) in &v.kapilar {
        for o in v.onermeler(a, "CALLS") {
            let Some(h) = o.hedef.as_deref().filter(|h| crate::graf::tur(h) == "external") else { continue };
            let e = sistem.entry(h.to_string()).or_insert((Guven::Unknown, BTreeSet::new(), BTreeSet::new(), String::new()));
            e.0 = e.0.max(etkin(o));
            e.1.insert(k["id"].as_str().unwrap_or(""));
            for c in o.deger["cagrilar"].as_array().into_iter().flatten() {
                if let Some(t) = c["tur"].as_str() {
                    e.2.insert(t.to_string());
                }
                if let Some(t) = c["hedef_turu"].as_str() {
                    e.3 = t.to_string();
                }
            }
        }
    }
    // Damga: gruptaki kenarların en zayıfı (sabit adres bile zincirden ulaşılınca yorumdur)
    let mut m = vec![];
    // Toplam: uygulamanın dışına etki eden işlemler (haritanın davranış özetinden; `rapor`un `dis_etkili_islem` sayımı)
    let etkili: Vec<(&str, Guven)> = v
        .kapilar
        .iter()
        .filter_map(|(k, _)| {
            let d = v.dna(k)["dis"].as_array().filter(|a| !a.is_empty())?;
            // Hedefi çözülemeyen çağrı da bir dış etkidir: varlığı en fazla yorum, hedefi ayrı maddede bilinmiyor
            Some((k["id"].as_str().unwrap_or(""), en_zayif(d.iter().map(|x| guven_degeri(&x["guven"]).max(Guven::Inferred)), Guven::Inferred)))
        })
        .collect();
    if !etkili.is_empty() {
        m.push(ornekli(
            tf!(
                "{} işlem uygulamanın dışına etki ediyor (giden HTTP, kuyruk, e-posta, ödeme ya da bulut)",
                "{} {i} something outside the application (outgoing HTTP, queue, e-mail, payment or cloud)",
                etkili.len();
                i = cogul(etkili.len(), "operation affects", "operations affect")
            ),
            en_zayif(etkili.iter().map(|x| x.1), Guven::Unknown),
            etkili.iter().take(8).map(|x| x.0.to_string()).collect(),
        ));
    }
    // Çağrı zincirinde sınıflandırılamayan çağrı yüzünden dış etkisi bilinmeyen işlemler (yazmalar onay ister)
    let bilinmeyen: Vec<(&str, bool)> = v
        .kapilar
        .iter()
        .filter(|(k, _)| v.dna(k)["dis_etki"] == "bilinmiyor")
        .map(|(k, _)| (k["id"].as_str().unwrap_or(""), !matches!(k["yontem"].as_str(), Some("GET" | "HEAD" | "OPTIONS"))))
        .collect();
    if !bilinmeyen.is_empty() {
        let yazma = bilinmeyen.iter().filter(|x| x.1).count();
        let mut sirali: Vec<String> = bilinmeyen.iter().filter(|x| x.1).map(|x| x.0.to_string()).collect();
        sirali.extend(bilinmeyen.iter().filter(|x| !x.1).map(|x| x.0.to_string()));
        m.push(ornekli(
            tf!(
                "{} işlemin dış etkisi bilinmiyor ({yazma} yazma onay ister): zincirde sınıflandırılamayan kütüphane ya da dinamik çağrı var, ya da zincir sınırda kesildi",
                "{} {i} an unknown external effect ({yazma} {y} approval): the chain has an unclassified library or dynamic call, or was cut at the limit",
                bilinmeyen.len();
                i = crate::dil::cogul(bilinmeyen.len(), "operation has", "operations have"),
                y = crate::dil::cogul(yazma, "write needs", "writes need")
            ),
            Guven::Unknown,
            sirali.into_iter().take(8).collect(),
        ));
    }
    for (tur, baslik) in [
        ("url", t!("sabit adresli dış sistem", "external systems with a fixed address")),
        (
            "sdk",
            t!("hazır istemci kütüphanesiyle çağrılan dış sistem (ödeme, e-posta, kuyruk, bulut)", "external systems called through a client SDK (payment, e-mail, queue, cloud)"),
        ),
        ("ortam", t!("adresi ortam değişkeninden ya da yapılandırmadan gelen dış sistem", "external systems whose address comes from an environment variable or configuration")),
        ("dinamik", t!("adresi koddan çözülemeyen dış çağrı", "external calls whose address the code does not resolve")),
    ] {
        let grup: Vec<(&String, &Sistem)> = sistem.iter().filter(|(_, x)| x.3 == tur).collect();
        if grup.is_empty() {
            continue;
        }
        let l: Vec<String> = grup
            .iter()
            .map(|(id, x)| tf!("{} ({}, {} işlem)", "{} ({}, {} {i})", crate::graf::ad(id), x.2.iter().map(|t| etki_turu(t)).collect::<Vec<_>>().join("/"), x.1.len(); i = cogul(x.1.len(), "operation", "operations")))
            .collect();
        let islem: BTreeSet<&str> = grup.iter().flat_map(|(_, x)| x.1.iter().copied()).collect();
        m.push(ornekli(
            tf!("{} {baslik}, {} işlemden çağrılıyor", "{} {baslik}, called from {} {i}", l.len(), islem.len(); i = cogul(islem.len(), "operation", "operations")),
            en_zayif(grup.iter().map(|(_, x)| x.0), Guven::Unknown),
            l.into_iter().take(8).collect(),
        ));
    }
    if !tetik.is_empty() {
        m.push(ornekli(
            tf!("{} olay ya da arka plan işi tetikleniyor (ad eşlemesiyle)", "{} {o} triggered (matched by name)", tetik.len(); o = cogul(tetik.len(), "event or background job is", "events or background jobs are")),
            en_zayif(tetik.values().copied(), Guven::Unknown),
            tetik.keys().take(8).cloned().collect(),
        ));
    }
    let d = &v.harita["ozet"]["dis"];
    let mut not = vec![];
    match d["istatistik"]["cagri"].as_u64() {
        Some(n) => {
            let bagli = d["istatistik"]["isleme_bagli"].as_u64().unwrap_or(0);
            if n > bagli {
                not.push(tf!(
                    "{} giden çağrı hiçbir işleme bağlanamadı (zamanlanmış iş, başlangıç kodu ya da izlenemeyen çağrı zinciri).",
                    "{} outgoing calls could not be tied to any operation (a scheduled job, start-up code or an untraceable call chain).",
                    n - bagli
                ));
            }
            if n == 0 {
                not.push(tf!(
                    "Taranan {} dosyada giden çağrı bulunamadı; bu \"yok\" demek değildir.",
                    "No outgoing call was found in the {} {f} scanned; this does not mean \"none\".",
                    d["istatistik"]["dosya"].as_u64().unwrap_or(0);
                    f = cogul(&d["istatistik"]["dosya"], "file", "files")
                ));
            }
        }
        None => not.push(harita_dis_taramasiz().into()),
    }
    not.push(
        t!(
            "Giden çağrılar (HTTP istemcileri, kuyruklar, e-posta, ödeme ve bulut SDK'ları) satır desenleriyle ve çağrı grafiği üzerinden (polimorfik adaylar, kütüphane çağrısının paketi) bulunur; listede olmayan çağrı \"yok\" demek değildir. Zincirinde sınıflandırılamayan çağrı olan yazma onaysız gidemez. Başka bir uygulamanın tabanına giden çağrı, iki uygulama santrale eklenince uygulamalar arası bağ olur.",
            "Outgoing calls (HTTP clients, queues, e-mail, payment and cloud SDKs) are found by line patterns and through the call graph (polymorphic candidates, the package of a library call); a call missing from the list does not mean \"none\". A write whose chain has an unclassified call cannot go without approval. A call to another application's base becomes a cross-application link once both are added to the switchboard."
        )
        .into(),
    );
    soru(
        "dis",
        m,
        Some(not.join(" ")),
        Some(komut("sezgi santral baglar", t!("kayıtlı uygulamalar arasındaki çağrıları bağlar", "links the calls between registered applications"))),
        None,
    )
}

/// Dış etki türünün okunur adı
fn etki_turu(t: &str) -> &str {
    match t {
        "http" => "HTTP",
        "eposta" => t!("e-posta", "e-mail"),
        "odeme" => t!("ödeme", "payment"),
        "kuyruk" => t!("kuyruk", "queue"),
        "bulut" => t!("bulut", "cloud"),
        "bildirim" => t!("bildirim", "notification"),
        x => x,
    }
}

/// Kanıt deposu okunamadığında yanıtın notu
fn depo_yok() -> String {
    t!("Kanıt deposu okunamadı.", "The evidence store could not be read.").into()
}

/// `sezgi init` komutunun kısa açıklaması
fn yeniden_cikarir() -> &'static str {
    t!("haritayı yeniden çıkarır", "builds the map again")
}

/// `sezgi init` ile yenileme komutunun açıklaması
fn yeniler() -> &'static str {
    t!("haritayı yeniler (ya da sürekli: sezgi izle)", "refreshes the map (or continuously: sezgi izle)")
}

/// Haritada dış çağrı taraması yoksa söylenen not
fn harita_dis_taramasiz() -> &'static str {
    t!("Bu harita giden çağrı taraması olmadan çıkarılmış: sezgi init ile yenileyin.", "This map was built without the outgoing-call scan: refresh it with sezgi init.")
}

/// İki sürümü karşılaştırma komutu
fn karsilastir_komutu() -> Value {
    komut(
        t!("sezgi karsilastir <eski-harita> <yeni-harita>", "sezgi karsilastir <old-map> <new-map>"),
        t!("iki sürüm arasındaki bozucu değişiklikleri bulur", "finds breaking changes between two versions"),
    )
}

fn degisim(v: &Veri) -> Value {
    let Some(md) = v.model else {
        return soru("degisim", vec![], Some(depo_yok()), Some(komut("sezgi init", yeniden_cikarir())), None);
    };
    if !v.izleniyor {
        return soru(
            "degisim",
            vec![],
            Some(
                t!(
                    "Harita uygulamanın klasörünün dışında: dayandığı dosyaların değişimi izlenmiyor.",
                    "The map is outside the application's folder: changes to the files it rests on are not tracked."
                )
                .into(),
            ),
            Some(karsilastir_komutu()),
            None,
        );
    }
    let eski = md.depo.onermeler.iter().filter(|o| o.hesap.gecerlilik != Gecerlilik::Gecerli).count();
    let dosya = md.depo.kanitlar.iter().flat_map(|k| k.dayanak.iter().map(|d| d.yol.as_str())).collect::<BTreeSet<_>>().len();
    // Önermeler dayandıkları dosyanın içerik özetine bağlıdır: değişen dosya bugün yeniden özetlenerek bulunur (kesin)
    let mut m = vec![madde(
        tf!(
            "{} önerme {dosya} dosyanın içerik özetine bağlı: dosya değişince ona dayananlar eskir",
            "{} {o} tied to the content digest of {dosya} {f}: when a file changes, the claims resting on it go stale",
            md.depo.onermeler.len();
            o = cogul(md.depo.onermeler.len(), "claim is", "claims are"),
            f = cogul(dosya, "file", "files")
        ),
        Guven::Certain,
    )];
    if eski == 0 {
        m.push(madde(
            t!("Bugün eski önerme yok: dayanılan dosyalar haritadan bu yana değişmedi", "No claim is stale today: the files they rest on have not changed since the map"),
            Guven::Certain,
        ));
        soru("degisim", m, None, Some(karsilastir_komutu()), None)
    } else {
        let mut dosyalar: BTreeSet<&str> = BTreeSet::new();
        for k in md.depo.kanitlar.iter().filter(|k| !k.gecerli) {
            dosyalar.extend(k.dayanak.iter().map(|d| d.yol.as_str()));
        }
        m.push(ornekli(
            tf!("{eski} önerme eski: dayandığı dosya haritadan sonra değişti", "{eski} {o} stale: the file {d} on changed after the map"; o = cogul(eski, "claim is", "claims are"), d = cogul(eski, "it rests", "they rest")),
            Guven::Certain,
            dosyalar.iter().take(8).map(|s| s.to_string()).collect(),
        ));
        soru(
            "degisim",
            m,
            Some(t!("Eski önermelerin bugün doğru olup olmadığı bilinmiyor.", "Whether the stale claims still hold today is unknown.").into()),
            Some(komut("sezgi init", yeniler())),
            None,
        )
    }
}

fn guven_sayimi(v: &Veri) -> Value {
    let Some(md) = v.model else {
        return soru("guven", vec![], Some(depo_yok()), Some(komut("sezgi init", yeniden_cikarir())), None);
    };
    let mut say: BTreeMap<Guven, usize> = BTreeMap::new();
    let (mut celiski, mut acik) = (0usize, 0usize);
    for o in &md.depo.onermeler {
        *say.entry(etkin(o)).or_default() += 1;
        celiski += usize::from(!o.hesap.celiski.is_empty());
        acik += usize::from(o.hesap.kapsam == Kapsam::Acik);
    }
    // Her madde kendi düzeyinin damgasını taşır (ölçeği gösterir); sayımın kendisi kanıt deposundan kesindir
    let mut m: Vec<Value> = [
        (Guven::Certain, t!("kesin", "certain")),
        (Guven::Observed, t!("gözlem", "observed")),
        (Guven::Inferred, t!("yorum", "inferred")),
        (Guven::Unknown, t!("bilinmiyor ya da eski", "unknown or stale")),
    ]
    .iter()
    .map(|(g, ad)| madde(tf!("{} önerme {ad}", "{} {o} {ad}", say.get(g).copied().unwrap_or(0); o = cogul(say.get(g).copied().unwrap_or(0), "claim", "claims")), *g))
    .collect();
    if celiski > 0 {
        m.push(madde(
            tf!("{celiski} önermede çelişen kanıt var: güveni en fazla yorum", "{celiski} {o} contradicting evidence: {t} trust is at most inferred"; o = cogul(celiski, "claim has", "claims have"), t = cogul(celiski, "its", "their")),
            Guven::Inferred,
        ));
    }
    if acik > 0 {
        m.push(madde(
            tf!(
                "{acik} önermenin listesi açık: listelenenler kanıtlı, liste eksik olabilir",
                "{acik} {o} an open list: what is listed has evidence, the list may be incomplete";
                o = cogul(acik, "claim has", "claims have")
            ),
            Guven::Inferred,
        ));
    }
    soru(
        "guven",
        m,
        Some(
            t!(
                "Bilinmeyenler Sezgi'nin bildiği bilinmeyenlerdir; temiz bir sonuç eksiksizlik kanıtı değildir.",
                "These are the unknowns Sezgi knows of; a clean result is not proof of completeness."
            )
            .into(),
        ),
        Some(komut("sezgi rapor", t!("neyin bilinmediğini listeler", "lists what is unknown"))),
        Some(Guven::Certain),
    )
}

fn cagri(v: &Veri) -> Value {
    let Some(md) = v.model else {
        return soru("cagri", vec![], Some(depo_yok()), Some(komut("sezgi init", yeniden_cikarir())), None);
    };
    let mut fonk: Vec<(String, Guven)> = vec![];
    for o in md.depo.onermeler.iter().filter(|o| o.yuklem == "CALLS") {
        let hedef = o.hedef.as_deref().unwrap_or("");
        if (crate::graf::tur(&o.varlik), crate::graf::tur(hedef)) == ("fn", "fn") {
            fonk.push((format!("{} → {}", fonk_adi(&o.varlik), fonk_adi(hedef)), etkin(o)));
        }
    }
    // İşleyicisi bilinen ana işlemler: işlemden işleyici fonksiyona giden çağrı kenarı (ikincil uygulamalar sayılmaz)
    let isleyici: Vec<Guven> = v
        .kapilar
        .iter()
        .filter_map(|(_, a)| v.onermeler(a, "CALLS").into_iter().filter(|o| o.hedef.as_deref().is_some_and(|h| crate::graf::tur(h) == "fn")).map(etkin).max())
        .collect();
    let c = &md.depo.ek["cagri"];
    let mut m = vec![];
    if !isleyici.is_empty() {
        m.push(madde(
            tf!(
                "{} / {} işlemin işleyici fonksiyonu (çağrının girdiği yer) koddan bulundu",
                "The handler function (where the request enters) was found in the code for {} / {} {i}",
                isleyici.len(),
                v.kapilar.len();
                i = cogul(v.kapilar.len(), "operation", "operations")
            ),
            en_zayif(isleyici, Guven::Unknown),
        ));
    }
    if c.is_object() {
        m.push(madde(
            tf!(
                "{} dosyada {} fonksiyon ve yöntem, aralarında {} çağrı kenarı; işlemlerden {} fonksiyona iniliyor",
                "{1} {f} and methods in {0} {d}, with {2} call {k} between them; the operations reach {3} {u}",
                c["dosya"].as_u64().unwrap_or(0),
                c["fonksiyon"].as_u64().unwrap_or(0),
                c["kenar"].as_u64().unwrap_or(0),
                c["ulasilan_fonksiyon"].as_u64().unwrap_or(0);
                f = cogul(&c["fonksiyon"], "function", "functions"),
                d = cogul(&c["dosya"], "file", "files"),
                k = cogul(&c["kenar"], "edge", "edges"),
                u = cogul(&c["ulasilan_fonksiyon"], "function", "functions")
            ),
            Guven::Certain,
        ));
    }
    let kesin: Vec<&(String, Guven)> = fonk.iter().filter(|x| x.1 == Guven::Certain).collect();
    let yorum: Vec<&(String, Guven)> = fonk.iter().filter(|x| x.1 != Guven::Certain).collect();
    if !kesin.is_empty() {
        m.push(ornekli(
            tf!(
                "{} çağrı tek anlamlı çözüldü (aynı sınıf, içe aktarma, enjekte edilen servisin tipi)",
                "{} {c} resolved unambiguously (same class, import, type of the injected service)",
                kesin.len();
                c = cogul(kesin.len(), "call was", "calls were")
            ),
            Guven::Certain,
            kesin.iter().take(6).map(|x| x.0.clone()).collect(),
        ));
    }
    if !yorum.is_empty() {
        m.push(ornekli(
            tf!(
                "{} çağrının hedefi adla, arayüzün tek uygulamasıyla ya da kapsayıcıdan eşlendi",
                "The target of {} {c} was matched by name, by the only implementation of an interface or from the container",
                yorum.len();
                c = cogul(yorum.len(), "call", "calls")
            ),
            en_zayif(yorum.iter().map(|x| x.1), Guven::Unknown),
            yorum.iter().take(4).map(|x| x.0.clone()).collect(),
        ));
    }
    // Örnek zincirler: tabloya yazan, tek anlamlı çözülmüş ve kısa olanlar önce (en uzun zincir en az güvenilir olandır)
    let mut zincirler: Vec<(u8, usize, String, Guven)> = vec![];
    let mut eksik = 0usize;
    for (k, _) in &v.kapilar {
        let d = v.dna(k);
        let c = &d["cagri"];
        eksik += usize::from(c["dinamik"].as_u64().unwrap_or(0) > 0 || c["kesildi"] == true);
        let z: Vec<&str> = c["zincir"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).collect();
        if z.len() < 3 {
            continue;
        }
        let yazar = d["tablo_etkisi"].as_array().is_some_and(|t| t.iter().any(|x| x["islem"] == "yazar"));
        let g = if c["zincir_guveni"] == "certain" { Guven::Certain } else { Guven::Inferred };
        let oncelik = u8::from(!yazar) + u8::from(g != Guven::Certain) * 2;
        zincirler.push((oncelik, z.len(), format!("{}: {}", k["id"].as_str().unwrap_or(""), z.join(" → ")), g));
    }
    zincirler.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    if !zincirler.is_empty() {
        let ornek: Vec<&(u8, usize, String, Guven)> = zincirler.iter().take(3).collect();
        m.push(ornekli(
            t!("Örnek çağrı zincirleri (işlemden veri katmanına)", "Example call chains (from the operation to the data layer)"),
            en_zayif(ornek.iter().map(|x| x.3), Guven::Unknown),
            ornek.iter().map(|x| x.2.clone()).collect(),
        ));
    }
    let mut not = vec![];
    if !c.is_object() {
        not.push(
            t!(
                "Çağrı grafiği bu haritada yok (eski bir sürümle çıkarılmış): sezgi init ile yenileyin.",
                "This map has no call graph (built with an older version): refresh it with sezgi init."
            )
            .to_string(),
        );
    }
    if let Some(n) = c["dinamik"].as_u64().filter(|n| *n > 0) {
        not.push(tf!(
            "{n} çağrının hedefi koddan bilinmez (dizin erişimi, yansıma, parametre olarak gelen fonksiyon): bu çağrılar kenar olarak yazılmadı.",
            "The target of {n} {c} cannot be known from the code (index access, reflection, a function passed as a parameter): {k} not written as edges.";
            c = cogul(n, "call", "calls"),
            k = cogul(n, "it was", "they were")
        ));
    }
    if eksik > 0 {
        not.push(tf!(
            "{eksik} işlemin zinciri eksik olabilir (hedefi bilinmeyen çağrı ya da derinlik sınırı).",
            "The chain of {eksik} {i} may be incomplete (a call with an unknown target, or the depth limit).";
            i = cogul(eksik, "operation", "operations")
        ));
    }
    not.push(
        t!(
            "Kütüphane içindeki fonksiyonlar ve çalışma anında kurulan bağlar izlenmez; zincir en çok 8 kat iner.",
            "Functions inside libraries and links made at run time are not followed; the chain goes at most 8 levels deep."
        )
        .into(),
    );
    soru("cagri", m, Some(not.join(" ")), None, None)
}

/// En çok işlemi olan iş yetenekleri (seçilen dildeki adları): örnek olarak alfabetik ilkler değil, uygulamanın asıl işi görünsün
fn one_cikan_yetenekler(a: &Value, sinir: usize) -> Vec<String> {
    let mut y: Vec<&Value> = a["yetenekler"].as_array().into_iter().flatten().collect();
    y.sort_by(|p, q| {
        let n = |v: &Value| v["islemler"].as_array().map(Vec::len).unwrap_or(0);
        n(q).cmp(&n(p)).then(p["ad"].as_str().cmp(&q["ad"].as_str()))
    });
    y.iter().map(|x| ad_dilde(x).to_string()).filter(|a| !a.is_empty()).take(sinir).collect()
}

/// Fonksiyon düğümünün okunur adı: `fn:dosya#Sinif.ad` -> `Sinif.ad`
fn fonk_adi(id: &str) -> &str {
    id.rsplit_once('#').map(|x| x.1).unwrap_or(crate::graf::ad(id))
}

fn gorev(v: &Veri) -> Value {
    let a = anlam_ozeti(v);
    let mut m = vec![];
    let yetenekler: Vec<&Value> = a["yetenekler"].as_array().into_iter().flatten().collect();
    if !yetenekler.is_empty() {
        m.push(ornekli(
            tf!(
                "{} iş yeteneği Türkçe ya da İngilizce istekle bulunur (sözlük ve fiil → HTTP yöntemi eşlemesi)",
                "{} business {y} can be found with a Turkish or English request (dictionary and verb → HTTP method mapping)",
                yetenekler.len();
                y = cogul(yetenekler.len(), "capability", "capabilities")
            ),
            en_zayif(yetenekler.iter().map(|y| guven_degeri(&y["guven"])), Guven::Unknown),
            one_cikan_yetenekler(&a, 8),
        ));
    }
    let kapilar: Vec<Value> = v.kapilar.iter().map(|(k, _)| (*k).clone()).collect();
    if let Some((istek, kapi)) = crate::anlam::ornek_istek(&kapilar, &crate::mcp::alan_adlari) {
        m.push(madde(tf!("Örnek: “{istek}” → {kapi}", "Example: “{istek}” → {kapi}"), Guven::Inferred));
    }
    soru(
        "gorev",
        m,
        Some(
            t!(
                "Görev planı bir öneridir (yorum): adımlar sözlükle bulunur, sıra kuraldır (okuma → yazma → silme), her adımın rolü ve onay gereği haritadan gelir; Sezgi planı yürütmez. Bağlı yapay zekâ aracı ara aracını gorev: true ile çağırır.",
                "A task plan is a suggestion (inferred): the steps are found with a dictionary, the order is a rule (read → write → delete), and each step's role and approval come from the map; Sezgi does not run the plan. A connected AI tool calls the search tool with task: true."
            )
            .into(),
        ),
        Some(komut(
            "sezgi bagla",
            t!(
                "yapay zekâ aracınıza bağlar; ajan görevi ara (gorev: true) ile adımlara ayırır, tani ile tanır",
                "connects your AI tool; the agent splits the task into steps with search (task: true) and inspects each with describe"
            ),
        )),
        None,
    )
}

// ---- İşlem ayrıntısı ve arama ----
// Ayna'nın kapı ayrıntısı ve "Ne yapmak istiyorsun?" araması: haritadaki davranış özetinin (çağrı zinciri, tablo etkisi,
// nesne düzeyi yetki, dış etki, ön koşul, geri alma) ve canlı modun onay kuralının tek işlem için düz dildeki hâli (Türkçe
// ya da İngilizce). Yeni olgu üretilmez; her madde dayandığı kanıtın damgasını taşır.

/// Ayrıntı bölümü: başlık, maddeler ve (varsa) bilinmeyen notu
fn bolum(id: &str, baslik: &str, maddeler: Vec<Value>, bilinmeyen: Option<String>) -> Value {
    json!({ "id": id, "baslik": baslik, "maddeler": maddeler, "bilinmeyen": bilinmeyen })
}

/// Kanıt konumu kısa: "src/services/album.service.ts:168" -> "album.service.ts:168"
fn kisa_konum(k: &Value) -> String {
    let t = k.as_str().unwrap_or("");
    t.rsplit_once('/').map(|x| x.1).unwrap_or(t).to_string()
}

fn risk_adi(k: &Value) -> &'static str {
    match crate::canli::islem_bilgisi(k).1 {
        "oku" => "okuma",
        "yikici" => "silme",
        _ => "yazma",
    }
}

/// Geri alma kararı ve damgası (haritanın davranış özetinden; okumada yazma yok)
fn geri_karari(k: &Value, d: &Value) -> (&'static str, Guven) {
    if crate::canli::islem_bilgisi(k).1 == "oku" {
        return ("okuma", Guven::Inferred);
    }
    match d["geri_alinabilir"].as_str() {
        Some("evet") => ("evet", Guven::Inferred),
        Some("kismen") => ("kismen", Guven::Inferred),
        Some("hayir") => ("hayir", Guven::Inferred),
        _ => ("bilinmiyor", Guven::Unknown),
    }
}

/// Bir işlemin iş yetenekleri (anlam katmanının gruplaması; yorum)
fn islem_yetenekleri(a: &Value, id: &str) -> Vec<String> {
    a["yetenekler"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|y| y["islemler"].as_array().is_some_and(|l| l.iter().any(|x| x == id)))
        .map(|y| ad_dilde(y).to_string())
        .filter(|a| !a.is_empty())
        .collect()
}

/// İşlemin etkisinin kısa satırları (arama sonucu): yazdığı tablolar, kaskad, dış etki
fn etki_ozeti(k: &Value, d: &Value) -> Vec<String> {
    let mut yazar: Vec<String> =
        d["tablo_etkisi"].as_array().into_iter().flatten().filter(|x| x["islem"] == "yazar").filter_map(|x| x["tablo"].as_str().map(String::from)).collect();
    if yazar.is_empty() {
        yazar = k["veri"]["yazar"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    }
    yazar.dedup();
    let mut v = vec![];
    if !yazar.is_empty() {
        v.push(tf!("yazar: {}", "writes: {}", yazar.join(", ")));
    }
    let kaskad = d["dolayli_yazar"].as_array().map(Vec::len).unwrap_or(0);
    if kaskad > 0 {
        v.push(tf!("kaskadla {kaskad} tablo daha", "{kaskad} more {t} by cascade"; t = cogul(kaskad, "table", "tables")));
    }
    let dis: Vec<String> = d["dis"].as_array().into_iter().flatten().map(|x| crate::dis::etki_adi(x).trim().to_string()).collect();
    if !dis.is_empty() {
        v.push(tf!("dış etki: {}", "external effect: {}", dis.into_iter().take(3).collect::<Vec<_>>().join(", ")));
    }
    v
}

/// Kapılar ekranındaki işlem ayrıntısı (`POST /api/islem`). Kapı yoksa Null
pub fn islem(harita: &Value, model: Option<&Model>, onizleyici: Option<&crate::mcp::Harita>, kapi: &str) -> Value {
    let Some(k) = harita["endpointler"].as_array().into_iter().flatten().find(|k| k["id"] == kapi) else { return Value::Null };
    let v = Veri { harita, rapor: &Value::Null, model, kapilar: vec![], izleniyor: false, onizleyici };
    let api = crate::graf::api_kimligi(k);
    let d = v.dna(k);
    let id = k["id"].as_str().unwrap_or("");
    let a = anlam_ozeti(&v);
    let mut bolumler = vec![];

    // Ne yapar
    let (aciklama, kesin) = crate::mcp::aciklama(k);
    let mut m = vec![if kesin {
        madde(tf!("Koddaki özeti: “{aciklama}”", "Its summary in the code: “{aciklama}”"), Guven::Certain)
    } else {
        madde(tf!("Kodda özeti yok; fonksiyon adından: “{aciklama}”", "No summary in the code; from the function name: “{aciklama}”"), Guven::Inferred)
    }];
    let yet = islem_yetenekleri(&a, id);
    if !yet.is_empty() {
        m.push(madde(tf!("İş yeteneği: {}", "Business capability: {}", yet.join(", ")), Guven::Inferred));
    }
    m.push(madde(
        tf!(
            "Kodda: {}.{} ({}:{})",
            "In the code: {}.{} ({}:{})",
            k["kontrolcu"].as_str().unwrap_or(""),
            k["fonksiyon"].as_str().unwrap_or(""),
            k["dosya"].as_str().unwrap_or(""),
            k["satir"]
        ),
        en_zayif(v.onermeler(&api, "VAR").into_iter().map(etkin), Guven::Certain),
    ));
    bolumler.push(bolum("ne", t!("Ne yapar?", "What does it do?"), m, None));

    // Kim yapabilir: giriş, rol, nesne düzeyi yetki
    let mut m = vec![];
    let gg = v.onermeler(&api, "GIRIS").into_iter().map(etkin).max().unwrap_or(Guven::Unknown);
    let giris = match k["giris"].as_str() {
        Some("gerekli") => t!("Giriş yapmış kullanıcı gerekir", "Needs a logged-in user"),
        Some("herkese_acik") => t!("Herkese açık: giriş gerekmez", "Public: no login needed"),
        Some("istege_bagli") => t!("Giriş isteğe bağlı", "Login optional"),
        _ => t!("Giriş gereksinimi çözülemedi", "The login requirement could not be resolved"),
    };
    m.push(madde(giris, if k["giris"] == "bilinmiyor" { Guven::Unknown } else { gg }));
    let roller: Vec<&str> = k["roller"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !roller.is_empty() {
        m.push(madde(tf!("Rol ya da izin: {}", "Role or permission: {}", roller.join(", ")), en_zayif(v.onermeler(&api, "ROLLER").into_iter().map(etkin), Guven::Unknown)));
    }
    for x in d["erisim_denetimi"].as_array().into_iter().flatten().take(4) {
        let izin = x["izin"].as_str().map(|i| tf!(" (izin {i})", " (permission {i})")).unwrap_or_default();
        let ne = if x["nesne_duzeyi"] == true {
            t!("Kaydın bu kullanıcıya açık olduğu denetleniyor", "Checks that the record is open to this user")
        } else {
            t!("Yetki denetimi", "Authorization check")
        };
        m.push(ornekli(format!("{ne}{izin}: {}", kisa_konum(&x["kanit"])), guven_degeri(&x["guven"]), x["ifade"].as_str().map(|i| vec![i.to_string()]).unwrap_or_default()));
    }
    for x in d["sahiplik_denetimi"].as_array().into_iter().flatten().take(3) {
        m.push(ornekli(
            tf!("Kaydın sahibi denetleniyor: {}", "Checks the record's owner: {}", kisa_konum(&x["kanit"])),
            guven_degeri(&x["guven"]),
            x["kosul"].as_str().map(|i| vec![i.to_string()]).unwrap_or_default(),
        ));
    }
    let not = (!nesne_denetimli(d) && crate::canli::islem_bilgisi(k).1 != "oku").then(|| {
        t!(
            "Kaydın bu kullanıcıya ait ya da açık olup olmadığını denetleyen bir kod bulunamadı; yokluk kanıtı değildir.",
            "No code was found that checks whether the record belongs to or is open to this user; this is not proof of absence."
        )
        .to_string()
    });
    bolumler.push(bolum("kim", t!("Kim yapabilir?", "Who can do it?"), m, not));

    // Çağrı zinciri
    let c = &d["cagri"];
    let mut m = vec![];
    let zincir: Vec<&str> = c["zincir"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !zincir.is_empty() {
        m.push(madde(zincir.join(" → "), if c["zincir_guveni"] == "certain" { Guven::Certain } else { Guven::Inferred }));
        m.push(madde(
            tf!(
                "{} fonksiyona ulaşıldı, en çok {} kat derinde",
                "{} {f} reached, at most {} {d} deep",
                c["erisilen"].as_u64().unwrap_or(0),
                c["derinlik"].as_u64().unwrap_or(0);
                f = cogul(&c["erisilen"], "function", "functions"),
                d = cogul(&c["derinlik"], "level", "levels")
            ),
            Guven::Certain,
        ));
    }
    if let Some(n) = c["dinamik"].as_u64().filter(|n| *n > 0) {
        m.push(madde(tf!("{n} çağrının hedefi koddan bilinmiyor", "The target of {n} {c} is unknown from the code"; c = cogul(n, "call", "calls")), Guven::Unknown));
    }
    let mut not = vec![];
    if c["kesildi"] == true {
        not.push(t!("Derinlik ya da boyut sınırı aşıldı: zincir eksik olabilir.", "The depth or size limit was reached: the chain may be incomplete.").to_string());
    }
    if zincir.is_empty() {
        not.push(
            if c.is_null() {
                t!(
                    "Bu işlem için çağrı zinciri yok (çağrı grafiği çıkarılmamış ya da işleyici bulunamadı).",
                    "There is no call chain for this operation (no call graph was built, or the handler was not found)."
                )
            } else {
                t!("İşleyici başka bir fonksiyon çağırmıyor ya da çağrılar izlenemedi.", "The handler calls no other function, or its calls could not be traced.")
            }
            .to_string(),
        );
    }
    bolumler.push(bolum("zincir", t!("Hangi fonksiyonlardan geçer?", "Which functions does it go through?"), m, (!not.is_empty()).then(|| not.join(" "))));

    // Veri etkisi: çağrı zincirinden tablo etkisi, yoksa işleyicinin doğrudan veri bağı; kaskad
    let mut m = vec![];
    let te: Vec<&Value> = d["tablo_etkisi"].as_array().into_iter().flatten().collect();
    for (tur, ad) in [("yazar", t!("Yazar", "Writes")), ("okur", t!("Okur", "Reads"))] {
        let l: Vec<&&Value> = te.iter().filter(|x| x["islem"] == tur).collect();
        if l.is_empty() {
            continue;
        }
        let mut tablolar: Vec<&str> = l.iter().filter_map(|x| x["tablo"].as_str()).collect();
        tablolar.dedup();
        m.push(ornekli(
            format!("{ad}: {}", tablolar.join(", ")),
            en_zayif(l.iter().map(|x| guven_degeri(&x["guven"])), Guven::Unknown),
            l.iter()
                .take(4)
                .map(|x| {
                    format!("{} ← {}", x["tablo"].as_str().unwrap_or(""), x["zincir"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(" › "))
                })
                .collect(),
        ));
    }
    if te.is_empty() {
        for (alan, ad) in [("yazar", t!("Yazar", "Writes")), ("okur", t!("Okur", "Reads"))] {
            let l: Vec<&str> = k["veri"][alan].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            if !l.is_empty() {
                m.push(madde(format!("{ad}: {}", l.join(", ")), Guven::Inferred));
            }
        }
    }
    let kaskad: Vec<&Value> = d["dolayli_yazar"].as_array().into_iter().flatten().collect();
    if !kaskad.is_empty() {
        m.push(ornekli(
            tf!("Silinince kaskadla etkilenir: {}", "Affected by cascade when deleted: {}", kaskad.iter().filter_map(|x| x["tablo"].as_str()).collect::<Vec<_>>().join(", ")),
            Guven::Inferred,
            kaskad.iter().take(4).map(|x| format!("{} ({})", crate::dis::gerekce_metni(x["neden"].as_str().unwrap_or("")), kisa_konum(&x["kanit"]))).collect(),
        ));
    }
    let not = m.is_empty().then(|| {
        t!(
            "Tablo bağı bulunamadı (dış servis, kütüphane içi ya da izlenemeyen çağrı olabilir); yokluk kanıtı değildir.",
            "No table link was found (an external service, a library or an untraceable call, perhaps); this is not proof of absence."
        )
        .to_string()
    });
    bolumler.push(bolum("veri", t!("Hangi tabloları etkiler?", "Which tables does it affect?"), m, not));

    // Dış etki ve tetiklenen olaylar
    let mut m = vec![];
    for x in d["dis"].as_array().into_iter().flatten().take(6) {
        let zaman = if x["zaman"] == "ertelenmis" { t!(" (ertelenmiş)", " (deferred)") } else { "" };
        let baslik = format!("{}{zaman} · {} ({})", crate::dis::etki_adi(x).trim(), x["istemci"].as_str().unwrap_or(""), kisa_konum(&x["kanit"]));
        // Zincir: işleyiciden dış çağrıya giden fonksiyonlar (çağrı grafiği ya da ad eşlemesi)
        let zincir: Vec<&str> = x["zincir"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if zincir.len() > 1 {
            m.push(ornekli(baslik, guven_degeri(&x["guven"]), vec![zincir.join(" › ")]));
        } else {
            m.push(madde(baslik, guven_degeri(&x["guven"])));
        }
    }
    // Zincirde sınıflandırılamayan çağrı: dış etki bilinmiyor (yazmada onay ister)
    if d["dis_etki"] == "bilinmiyor" {
        let nedenler: Vec<String> = d["dis_etki_bilinmeyen"]
            .as_array()
            .into_iter()
            .flatten()
            .take(4)
            .map(|x| {
                let z: Vec<&str> = x["zincir"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                let ifade = x["ifade"].as_str().unwrap_or("");
                if z.is_empty() {
                    format!("{ifade} ({})", kisa_konum(&x["kanit"]))
                } else {
                    format!("{ifade} ({}) ← {}", kisa_konum(&x["kanit"]), z.join(" › "))
                }
            })
            .collect();
        m.push(ornekli(
            tf!(
                "Dış etkisi bilinmiyor: zincirde {} sınıflandırılamayan çağrı (kütüphane, dinamik çağrı ya da kesilen zincir); yazmaysa onay ister",
                "External effect unknown: {} {c} in the chain (a library, a dynamic call or a cut chain); a write needs approval",
                d["dis_etki_bilinmeyen_sayisi"].as_u64().unwrap_or(0);
                c = crate::dil::cogul(&d["dis_etki_bilinmeyen_sayisi"], "unclassified call", "unclassified calls")
            ),
            Guven::Unknown,
            nedenler,
        ));
    }
    let tetik: Vec<String> = d["tetikler"].as_array().into_iter().flatten().map(|t| t["ad"].as_str().unwrap_or("").to_string()).collect();
    if !tetik.is_empty() {
        m.push(ornekli(
            tf!("{} olay ya da arka plan işi tetikliyor", "Triggers {} {o}", tetik.len(); o = cogul(tetik.len(), "event or background job", "events or background jobs")),
            Guven::Inferred,
            tetik.into_iter().take(6).collect(),
        ));
    }
    let not = m.is_empty().then(|| {
        if harita["ozet"]["dis"].is_null() {
            harita_dis_taramasiz().to_string()
        } else {
            t!(
                "Bu işlemden giden çağrı (HTTP, kuyruk, e-posta, ödeme) bulunamadı; \"yok\" demek değildir.",
                "No outgoing call (HTTP, queue, e-mail, payment) was found from this operation; this does not mean \"none\"."
            )
            .to_string()
        }
    });
    bolumler.push(bolum("dis", t!("Uygulamanın dışına etki eder mi?", "Does it affect anything outside the application?"), m, not));

    // Ön koşullar: koşullu redler (kod, koşul, konum)
    let redler: Vec<&Value> = d["hatalar"].as_array().into_iter().flatten().filter(|h| on_kosul_mu(h)).collect();
    let m: Vec<Value> = redler
        .iter()
        .take(8)
        .map(|h| {
            let kod = h["durum"]
                .as_u64()
                .map(|c| tf!("{c} ile reddedilir", "Rejected with {c}"))
                .unwrap_or_else(|| t!("Reddedilir (red kodu koddan çıkmıyor)", "Rejected (the code does not give the status code)").into());
            ornekli(format!("{kod}: {}", kisa_konum(&h["kanit"])), guven_degeri(&h["guven"]), h["kosul"].as_str().map(|x| vec![x.to_string()]).unwrap_or_default())
        })
        .collect();
    let mut not = vec![];
    if redler.len() > 8 {
        not.push(
            tf!("{} koşul daha var.", "There {v} {} more {k}.", redler.len() - 8; v = cogul(redler.len() - 8, "is", "are"), k = cogul(redler.len() - 8, "condition", "conditions")),
        );
    }
    not.push(
        t!(
            "Önizleme bu koşulları denetleyemez; listelenmeyen koşul \"yok\" demek değildir.",
            "The preview cannot check these conditions; a condition not listed does not mean \"none\"."
        )
        .to_string(),
    );
    bolumler.push(bolum("onkosul", t!("Hangi durumda reddedilir?", "When is it rejected?"), m, Some(not.join(" "))));

    // Geri alınabilir mi
    let (karar, gg) = geri_karari(k, d);
    let gerekce = d["geri_alma_gerekce"].as_str().filter(|g| !g.is_empty()).map(|g| format!(": {}", crate::dis::gerekce_metni(g))).unwrap_or_default();
    let (m, not) = match karar {
        "okuma" => (
            vec![],
            Some(
                t!(
                    "Okuma yöntemiyle (GET) çağrılıyor: geri alma kararı yazma işlemleri için verilir.",
                    "Called with a read method (GET): reversibility is decided for write operations."
                )
                .to_string(),
            ),
        ),
        "bilinmiyor" => (
            vec![],
            Some(tf!(
                "Koddan çıkarılamadı{gerekce}. Geri alınamaz ya da dış etkili yazma onaysız gitmez.",
                "Could not be derived from the code{gerekce}. A write that cannot be undone or has an external effect never goes without approval."
            )),
        ),
        "evet" => (vec![madde(tf!("Evet{gerekce}", "Yes{gerekce}"), gg)], None),
        "kismen" => (vec![madde(tf!("Kısmen{gerekce}", "Partly{gerekce}"), gg)], None),
        _ => (vec![madde(tf!("Hayır{gerekce}", "No{gerekce}"), gg)], None),
    };
    bolumler.push(bolum("geri", t!("Geri alınabilir mi?", "Can it be undone?"), m, not));

    // Onay: canlı modun kuralı
    let sinif = onay_sinifi(&v, k);
    let (onay_karar, onay_m, onay_g) = onay_metni(&sinif, k);
    let mut m = vec![madde(onay_m.clone(), onay_g)];
    if !matches!(sinif, OnaySinifi::Okuma) {
        m.push(madde(
            t!("Gerçek veride (deneysel) her yazma insan parolasıyla onaylanır", "On real data (experimental) every write is approved with the human password"),
            Guven::Certain,
        ));
    }
    bolumler.push(bolum("onay", t!("Yapay zekâ bunu onaysız yapabilir mi?", "Can the AI do this without approval?"), m, None));

    json!({
        "id": id, "yontem": k["yontem"], "yol": k["yol"], "aciklama": aciklama, "aciklama_kesin": kesin, "risk": risk_adi(k),
        "geri": { "karar": karar, "guven": gg.ad() },
        "onay": { "karar": onay_karar, "metin": onay_m, "guven": onay_g.ad() },
        "bolumler": bolumler,
    })
}

/// "Ne yapmak istiyorsun?" araması (`POST /api/ara`): Türkçe ya da İngilizce istek MCP `ara` ile aynı sözlükle
/// işlemlere çevrilir. `kapilar`: puan sırasıyla bütün eşleşenler; `sonuclar`: ilk birkaçı etkisi, geri alma ve onay
/// kararıyla; birden çok eylemli istekte `plan` (MCP `ara` `gorev: true` ile aynı plan; yürütülmez)
pub fn ara(harita: &Value, model: Option<&Model>, onizleyici: Option<&crate::mcp::Harita>, sorgu: &str) -> Value {
    let tum: Vec<Value> = harita["endpointler"].as_array().cloned().unwrap_or_default();
    let ids = crate::anlam::ara_kimlikleri(&tum, sorgu, 100);
    let v = Veri { harita, rapor: &Value::Null, model, kapilar: vec![], izleniyor: false, onizleyici };
    let sonuclar: Vec<Value> = ids
        .iter()
        .take(6)
        .filter_map(|id| tum.iter().find(|k| k["id"] == id.as_str()))
        .map(|k| {
            let d = v.dna(k);
            let (aciklama, kesin) = crate::mcp::aciklama(k);
            let (geri, gg) = geri_karari(k, d);
            let (karar, metin, g) = onay_metni(&onay_sinifi(&v, k), k);
            json!({
                "id": k["id"], "yontem": k["yontem"], "yol": k["yol"], "aciklama": aciklama, "aciklama_kesin": kesin, "risk": risk_adi(k),
                "etki": etki_ozeti(k, d), "geri": { "karar": geri, "guven": gg.ad() }, "onay": { "karar": karar, "metin": metin, "guven": g.ad() },
            })
        })
        .collect();
    let plan = crate::anlam::cok_adimli(sorgu).then(|| crate::anlam::plan(sorgu, &tum, &crate::mcp::alan_adlari, crate::dil::en()).0);
    json!({ "kapilar": ids, "sonuclar": sonuclar, "plan": plan })
}

// ---- Yetenekler ----
// Dört ana yetenek, kullanıcının sorduğu sırayla: (1) uygulamayı anlar, (2) her işlemin neyi etkilediğini bilir, (3) bunu
// yapay zekâya kanıtla anlatır, (4) yapay zekâya güvenli iş yaptırır. Ardından yardımcı yetenekler (gör, çoklu uygulama,
// kaynaksız keşif). Her kartın durumu ve alt durumları bu bilgisayardaki gerçek durumdan hesaplanır.

/// Kart: `grup` ana (1-4, `adim`) ya da diger; `durumlar`: kartın alt durumları (metin ve durum)
fn yetenek(id: &str, durum: &str, ayrinti: String, k: &str) -> Value {
    let (baslik, aciklama) = KARTLAR.iter().find(|x| x.0 == id).map(|x| (t!(x.1, x.2), t!(x.3, x.4))).unwrap_or((id, ""));
    let (grup, adim) = match id {
        "anla" => ("ana", 1),
        "etki" => ("ana", 2),
        "bagla" => ("ana", 3),
        "canli" => ("ana", 4),
        _ => ("diger", 0),
    };
    json!({ "id": id, "baslik": baslik, "aciklama": aciklama, "durum": durum, "ayrinti": ayrinti, "komut": k, "grup": grup, "adim": adim, "durumlar": [] })
}

/// Kartın alt durumu: (metin, durum: hazir / kurulmamis / uygulanamaz / bilinmiyor)
fn alt(metin: impl Into<String>, durum: &str) -> Value {
    json!({ "metin": metin.into(), "durum": durum })
}

fn durumlu(mut y: Value, durumlar: Vec<Value>) -> Value {
    y["durumlar"] = json!(durumlar);
    y
}

/// Yetenek kartlarının başlığı ve açıklaması: (kimlik, başlık, başlık İngilizce, açıklama, açıklama İngilizce)
const KARTLAR: [(&str, &str, &str, &str, &str); 7] = [
    (
        "anla",
        "Uygulamayı anlar",
        "Understands the application",
        "Kodu okur: uygulamanın ne olduğunu, neden var olduğunu, kullanıcı tiplerini ve iş yeteneklerini (ör. \"albüm paylaş\") kanıtıyla çıkarır. Amaç uygulamanın kendi cümlesinden alıntılanır, uydurulmaz.",
        "Reads the code and derives, with evidence, what the application is, why it exists, its user types and its business capabilities (e.g. \"share an album\"). The purpose is quoted from the application's own words, never invented.",
    ),
    (
        "etki",
        "Her işlemin neyi etkilediğini bilir",
        "Knows what each operation affects",
        "Fonksiyon çağrılarını izleyerek her işlemin hangi tablolara yazdığını, hangi dış sistemlere (ödeme, e-posta, kuyruk, başka uygulama) gittiğini, geri alınıp alınamayacağını ve kimin yapabileceğini (giriş, rol, kaydın sahibi) bulur.",
        "Follows function calls to find which tables each operation writes, which external systems it reaches (payment, e-mail, queue, another application), whether it can be undone and who may run it (login, role, record owner).",
    ),
    (
        "bagla",
        "Yapay zekâya kanıtla anlatır",
        "Explains it to AI with evidence",
        "Haritayı MCP ile yapay zekâ aracınıza bağlar: yapay zekâ kodu okumadan uygulamayı tanır, işlemi Türkçe ya da İngilizce arar ve her bilginin güven damgasını ve kaynağını (dosya:satır) görür. Benlik Kartı aynı bilgiyi imzalı taşır.",
        "Connects the map to your AI tool over MCP: the AI knows the application without reading its code, searches for operations in English or Turkish and sees each fact's trust stamp and source (file:line). The Benlik Card carries the same facts, signed.",
    ),
    (
        "canli",
        "Yapay zekâya güvenli iş yaptırır",
        "Lets AI work safely",
        "Yapay zekâ yalnızca token'ın açtığı işlemleri insanın kendi oturumuyla çağırır ve her istek önce önizlenir. Silme, dış etki ve geri alınamaz yazma her seferinde insan onayı ister. Yazma test kopyasına gider; gerçek veriye (deneysel) yalnızca insan bunu terminalde insan parolasıyla açarsa ve her yazmayı orada onaylarsa. Her işlem zincirli kayda yazılır.",
        "The AI calls only the operations its token opens, with the human's own session, and every request is previewed first. Deletes, external effects and writes that cannot be undone need human approval every time. Writes go to a test copy; to real data (experimental) only if a human opens it in a terminal with the human password and approves each write there. Every operation is written to a chained log.",
    ),
    (
        "gor",
        "Gör",
        "Look",
        "Haritayı tarayıcıda (Ayna), neyin bilinmediğiyle birlikte terminalde (rapor) ve iki sürüm arasındaki bozucu farkla (karşılaştır) gösterir.",
        "Shows the map in the browser (Ayna), in the terminal together with what is unknown (report), and as the breaking difference between two versions (diff).",
    ),
    (
        "santral",
        "Çoklu uygulama",
        "Several applications",
        "Birden fazla uygulamayı tek bağlantıda toplar; uygulamalar arası çağrıları ve insanın tanımladığı varlık eşlemelerini gösterir.",
        "Brings several applications under one connection; shows the calls between them and the entity mappings a human defined.",
    ),
    (
        "kaynaksiz",
        "Kaynaksız keşif",
        "Discovery without source code",
        "Kaynak kodu olmayan uygulamayı (.exe/.dll, APK, SQLite) inceler; neyi bildiğini ve neyi bilemeyeceğini açıkça söyler, tam keşif vaat etmez.",
        "Inspects an application without source code (.exe/.dll, APK, SQLite); says plainly what it knows and what it cannot know, and does not promise full discovery.",
    ),
];

/// Çağrılabilir işlemi olmayan haritada kartın ayrıntısı
fn islem_yok() -> String {
    t!("Bu haritada çağrılabilir işlem yok.", "This map has no callable operations.").into()
}

/// Kaynaksız keşif komutu (yer tutucu seçilen dilde)
fn kesfet_komutu() -> &'static str {
    t!("sezgi kesfet <klasör|dosya>", "sezgi kesfet <folder|file>")
}

/// İnsan parolasının (sistem çapası) bu bilgisayardaki durumu: kartın alt durumu
fn insan_durumu() -> Value {
    match crate::insan::capa() {
        crate::insan::CapaDurumu::Var(c) => alt(
            tf!("İnsan parolası kurulu (çapa {}): gerçek veride onayı yalnızca insan verir", "Human password set up (anchor {}): on real data only the human approves", c.izi),
            "hazir",
        ),
        crate::insan::CapaDurumu::Yok(_) => alt(
            tf!(
                "İnsan parolası kurulu değil: gerçek veride yazma kapalı (kurmak için {})",
                "Human password not set up: writes on real data are off (to set it up: {})",
                crate::insan::kur_komutu()
            ),
            "kurulmamis",
        ),
        crate::insan::CapaDurumu::Gecersiz(..) => alt(
            tf!("İnsan parolası geçersiz: yönetici yeniden kurmalı ({})", "Human password invalid: an administrator must set it up again ({})", crate::insan::kur_komutu()),
            "kurulmamis",
        ),
    }
}

/// Sezgi sekmesinin üstündeki anlam özeti: iş alanı, amaç alıntısı, kullanıcı tipleri ve iş yetenekleri, damgalarıyla
fn anlam_karti(model: Option<&Model>) -> Value {
    let a = model.map(|m| crate::anlam::ozet(&m.depo)).unwrap_or(Value::Null);
    if a.is_null() {
        return Value::Null;
    }
    let alan = &a["alan"];
    let alan = if alan["ad"].is_string() {
        json!({ "ad": ad_dilde(alan), "guven": alan["guven"], "kanit": crate::anlam::kanit_ornekleri(alan, 3) })
    } else {
        json!({ "ad": null, "guven": "unknown", "not": t!("kanıt yetmedi: yanlış bir alan söylemektense bilinmiyor denir", "not enough evidence: Sezgi says unknown rather than name a wrong domain") })
    };
    let amac = a["amac"].as_array().and_then(|l| l.first()).map(|x| json!({ "alinti": x["alinti"], "konum": x["konum"], "guven": x["guven"] })).unwrap_or(Value::Null);
    let problem =
        a["problem"].as_array().and_then(|l| l.iter().find(|p| p["dayanak"] != "alinti")).map(|p| json!({ "metin": problem_metni(p), "guven": p["guven"] })).unwrap_or(Value::Null);
    let aktorler: Vec<Value> = a["aktorler"].as_array().into_iter().flatten().map(|x| json!({ "ad": ad_dilde(x), "islem": x["islem_sayisi"], "guven": x["guven"] })).collect();
    let n = a["yetenekler"].as_array().map(Vec::len).unwrap_or(0);
    let gy = en_zayif(a["yetenekler"].as_array().into_iter().flatten().map(|y| guven_degeri(&y["guven"])), Guven::Unknown);
    json!({
        "alan": alan, "tur": if a["tur"]["ad"].is_string() { json!({ "ad": ad_dilde(&a["tur"]), "guven": a["tur"]["guven"] }) } else { Value::Null },
        "amac": amac, "problem": problem, "aktorler": aktorler,
        "yetenek": { "sayi": n, "guven": gy.ad(), "ornekler": one_cikan_yetenekler(&a, 6) },
    })
}

/// Yeteneklerin bu projedeki durumu, anlam özeti ve sıradaki adım. `proje`: uygulamanın klasörü; `harita`: haritanın
/// klasörü (yoksa haritasız açılış); `model`: kanıt deposu. Salt okunur.
pub fn yetenekler(proje: &Path, harita: Option<(&Path, &Value)>, model: Option<&Model>) -> Value {
    let cati = crate::cati(proje);
    let surum = env!("CARGO_PKG_VERSION");
    let Some((klasor, h)) = harita else {
        let (anla, kaynaksiz, sonraki) = match cati {
            Some(c) => (
                yetenek(
                    "anla",
                    "kurulmamis",
                    tf!("Bu klasörde bir {c} uygulaması var; haritası henüz çıkarılmadı.", "This folder holds a {c} application; its map has not been built yet."),
                    "sezgi init",
                ),
                yetenek(
                    "kaynaksiz",
                    "uygulanamaz",
                    t!("Bu klasörde kaynak kod var: Sezgi onu doğrudan okur.", "This folder has source code: Sezgi reads it directly.").into(),
                    kesfet_komutu(),
                ),
                komut(
                    "sezgi init",
                    t!(
                        "kodu okur, haritayı ve Benlik Kartı'nı çıkarır (birkaç saniye; uygulamayı çalıştırmaz)",
                        "reads the code and builds the map and the Benlik Card (a few seconds; the application is not run)"
                    ),
                ),
            ),
            None => (
                yetenek(
                    "anla",
                    "uygulanamaz",
                    t!(
                        "Bu klasörde Sezgi'nin okuyabildiği bir çatı bulunamadı: uygulamanın klasöründe açın ya da klasörünü verin.",
                        "No framework Sezgi can read was found in this folder: open it in the application's folder or pass that folder."
                    )
                    .into(),
                    t!("sezgi init <uygulama klasörü>", "sezgi init <app folder>"),
                ),
                yetenek(
                    "kaynaksiz",
                    "kurulmamis",
                    t!("Kaynak kodu yoksa uygulamanın dosyalarını inceleyebilir.", "Without source code it can inspect the application's files.").into(),
                    kesfet_komutu(),
                ),
                komut(
                    t!("sezgi init <uygulama klasörü>", "sezgi init <app folder>"),
                    t!("uygulamanın kodunu okur; kaynak kodu yoksa: sezgi kesfet <klasör|dosya>", "reads the application's code; without source code: sezgi kesfet <folder|file>"),
                ),
            ),
        };
        let once = t!("Önce harita gerekir (sezgi init).", "A map is needed first (sezgi init).").to_string();
        return json!({
            "surum": surum, "harita": false, "klasor": klasor_adi(proje), "cati": cati, "uygulama": Value::Null, "islem": 0, "kaynaksiz": false, "anlam": Value::Null,
            "yetenekler": [
                anla,
                yetenek("etki", "kurulmamis", once.clone(), "sezgi init"),
                yetenek("bagla", "kurulmamis", once.clone(), "sezgi bagla"),
                durumlu(yetenek("canli", "kurulmamis", once.clone(), "sezgi canli kur"), vec![insan_durumu()]),
                yetenek("gor", "kurulmamis", t!("Ayna açık ama gösterecek harita yok.", "Ayna is open but there is no map to show.").into(), "sezgi rapor"),
                yetenek("santral", "kurulmamis", once, "sezgi santral ekle .sezgi"),
                kaynaksiz,
            ],
            "sonraki": sonraki,
        });
    };
    let ozet = &h["ozet"];
    let kaynaksiz = ozet["kaynaksiz"] == true;
    let kapilar = ana_kapilar(h);
    let islem = kapilar.len();
    let varsayilan = klasor.file_name().is_some_and(|a| a == ".sezgi");
    // Tazelik: içerik özetleriyle değişen dosyalar (bayatlık kapısıyla aynı karar); yalnızca uygulamanın kendi haritasında
    let degisen = if varsayilan && !kaynaksiz { crate::izle::degisen_dosyalar(proje, klasor).len() } else { 0 };
    let ek = if varsayilan { String::new() } else { format!(" {}", crate::guvenli::gorunen(klasor)) };
    let mut sonraki: Option<Value> = None;

    let kart = crate::guvenli::oku_harita(&klasor.join("benlik.json")).and_then(|m| serde_json::from_str::<Value>(&m).ok()).unwrap_or(Value::Null);
    let imzali = kart["signature"]["alg"].as_str().is_some_and(|a| a != "none") && kart["signature"]["value"].as_str() != Some("UNSIGNED-DEV-BUILD");
    let kart_durumu = if kart.is_null() {
        alt(t!("Benlik Kartı yok", "No Benlik Card"), "kurulmamis")
    } else if imzali {
        alt(t!("Benlik Kartı imzalı", "Benlik Card signed"), "hazir")
    } else {
        alt(
            t!(
                "Benlik Kartı imzasız (imzalamak için: sezgi anahtar olustur, sonra sezgi init --imzala)",
                "Benlik Card unsigned (to sign it: sezgi anahtar olustur, then sezgi init --imzala)"
            ),
            "kurulmamis",
        )
    };
    let kart_metni = kart_durumu["metin"].as_str().unwrap_or("").split(" (").next().unwrap_or("").to_string();

    // 1. Uygulamayı anlar: harita, tazelik, iş alanı, kullanıcı tipleri, iş yetenekleri
    let anlam = anlam_karti(model);
    let anla = if kaynaksiz {
        durumlu(
            yetenek("anla", "hazir", tf!("Kaynaksız keşif haritası · {kart_metni}.", "Map from discovery without source code · {kart_metni}."), kesfet_komutu()),
            vec![alt(t!("Kaynak kodu yok: yalnızca ikilinin beyan ettiği yapı bilinir", "No source code: only the structure the binary declares is known"), "uygulanamaz")],
        )
    } else {
        let mut d = vec![if degisen > 0 {
            alt(
                tf!("{islem} işlem · {degisen} dosya haritadan sonra değişti: yenileyin", "{islem} {i} · {degisen} {f} changed after the map: refresh it"; i = cogul(islem, "operation", "operations"), f = cogul(degisen, "file", "files")),
                "kurulmamis",
            )
        } else {
            alt(tf!("{islem} işlem · harita güncel", "{islem} {i} · map up to date"; i = cogul(islem, "operation", "operations")), "hazir")
        }];
        if anlam.is_null() {
            d.push(alt(
                t!(
                    "Anlam özeti yok (eski bir sürümle çıkarılmış harita): sezgi init ile yenileyin",
                    "No meaning summary (map built with an older version): refresh it with sezgi init"
                ),
                "kurulmamis",
            ));
        } else {
            d.push(match anlam["alan"]["ad"].as_str() {
                Some(a) => alt(tf!("İş alanı: {a} (yorum)", "Business domain: {a} (inferred)"), "hazir"),
                None => alt(t!("İş alanı belirlenemedi (bilinmiyor denir, uydurulmaz)", "Business domain not determined (said to be unknown, not invented)"), "bilinmiyor"),
            });
            let ak = anlam["aktorler"].as_array().map(Vec::len).unwrap_or(0);
            d.push(alt(
                tf!("{ak} kullanıcı tipi · {} iş yeteneği", "{ak} user {t} · {} business {y}", anlam["yetenek"]["sayi"]; t = cogul(ak, "type", "types"), y = cogul(&anlam["yetenek"]["sayi"], "capability", "capabilities")),
                if ak > 0 { "hazir" } else { "bilinmiyor" },
            ));
        }
        let ayrinti = if degisen > 0 {
            sonraki = Some(komut("sezgi init", yeniler()));
            tf!(
                "{islem} işlem · {degisen} dosya haritadan sonra değişti: yenileyin · {kart_metni}.",
                "{islem} {i} · {degisen} {f} changed after the map: refresh it · {kart_metni}.";
                i = cogul(islem, "operation", "operations"),
                f = cogul(degisen, "file", "files")
            )
        } else {
            tf!("{islem} işlem · harita güncel · {kart_metni}.", "{islem} {i} · map up to date · {kart_metni}."; i = cogul(islem, "operation", "operations"))
        };
        durumlu(yetenek("anla", "hazir", ayrinti, "sezgi init"), d)
    };

    // 2. Her işlemin neyi etkilediğini bilir: çağrı grafiği, tablo etkisi, dış sistemler, geri alma, nesne düzeyi yetki
    let cagri = model.map(|m| &m.depo.ek["cagri"]).filter(|c| c.is_object());
    let etki = if kaynaksiz || islem == 0 {
        yetenek("etki", "uygulanamaz", islem_yok(), "sezgi rapor")
    } else {
        let dnalar: Vec<(&Value, &Value)> = kapilar.iter().map(|(k, _)| (*k, dna(h, k))).collect();
        let tablo =
            dnalar.iter().filter(|(k, d)| d["tablo_etkisi"].as_array().is_some_and(|t| !t.is_empty()) || k["veri"]["yazar"].as_array().is_some_and(|t| !t.is_empty())).count();
        let dis = dnalar.iter().filter(|(_, d)| d["dis"].as_array().is_some_and(|a| !a.is_empty())).count();
        let yazma: Vec<&(&Value, &Value)> = dnalar.iter().filter(|(k, _)| crate::canli::islem_bilgisi(k).1 != "oku").collect();
        let geri = yazma.iter().filter(|(_, d)| matches!(d["geri_alinabilir"].as_str(), Some("evet" | "kismen" | "hayir"))).count();
        let nesne = dnalar.iter().filter(|(_, d)| nesne_denetimli(d)).count();
        let mut d = vec![match cagri {
            Some(c) => alt(
                tf!("Çağrı grafiği: {} fonksiyon, {} çağrı", "Call graph: {} {f}, {} {k}", c["fonksiyon"].as_u64().unwrap_or(0), c["kenar"].as_u64().unwrap_or(0); f = cogul(&c["fonksiyon"], "function", "functions"), k = cogul(&c["kenar"], "call", "calls")),
                "hazir",
            ),
            None => alt(
                t!(
                    "Çağrı grafiği yok (eski bir sürümle çıkarılmış harita): sezgi init ile yenileyin",
                    "No call graph (map built with an older version): refresh it with sezgi init"
                ),
                "kurulmamis",
            ),
        }];
        d.push(alt(
            tf!("Tablo etkisi {tablo} / {islem} işlemde", "Table effect known for {tablo} / {islem} {i}"; i = cogul(islem, "operation", "operations")),
            if tablo > 0 { "hazir" } else { "bilinmiyor" },
        ));
        d.push(if ozet["dis"].is_null() {
            alt(t!("Dış sistem taraması yok: sezgi init ile yenileyin", "No external-system scan: refresh it with sezgi init"), "kurulmamis")
        } else {
            alt(tf!("{dis} işlem uygulamanın dışına etki ediyor", "{dis} {i} something outside the application"; i = cogul(dis, "operation affects", "operations affect")), "hazir")
        });
        d.push(alt(
            tf!("Geri alınabilirlik {geri} / {} yazma işleminde biliniyor", "Reversibility known for {geri} / {} write {i}", yazma.len(); i = cogul(yazma.len(), "operation", "operations")),
            if geri > 0 { "hazir" } else { "bilinmiyor" },
        ));
        d.push(alt(
            tf!("Nesne düzeyi yetki {nesne} işlemde denetleniyor", "Object-level authorization checked in {nesne} {i}"; i = cogul(nesne, "operation", "operations")),
            if nesne > 0 { "hazir" } else { "bilinmiyor" },
        ));
        let durum = if cagri.is_some() {
            "hazir"
        } else {
            sonraki.get_or_insert_with(|| {
                komut(
                    "sezgi init",
                    t!(
                        "haritayı bu sürümle yeniler: çağrı grafiği, dış etki ve anlam özeti eklenir",
                        "refreshes the map with this version: the call graph, external effects and the meaning summary are added"
                    ),
                )
            });
            "kurulmamis"
        };
        durumlu(
            yetenek(
                "etki",
                durum,
                tf!(
                    "{tablo} işlemde tablo etkisi · {dis} işlemde dış etki · {nesne} işlemde nesne düzeyi yetki.",
                    "Table effect in {tablo} {i} · external effect in {dis} · object-level authorization in {nesne}.";
                    i = cogul(tablo, "operation", "operations")
                ),
                &format!("sezgi rapor{ek}"),
            ),
            d,
        )
    };

    // 3. Yapay zekâya kanıtla anlatır: MCP bağlantısı ve Benlik Kartı
    let baglantilar = crate::baglan::baglanti_durumlari(proje);
    let tamam: Vec<&str> = baglantilar.iter().filter(|(_, _, d)| matches!(d, crate::baglan::Durum::Tamam(_))).map(|(a, _, _)| a.as_str()).collect();
    let sorunlu = baglantilar.len() - tamam.len();
    let bagla = if !tamam.is_empty() {
        let s = if sorunlu > 0 {
            tf!(" · {sorunlu} bağlantıda sorun var (sezgi doktor)", " · {sorunlu} {b} a problem (sezgi doktor)"; b = cogul(sorunlu, "connection has", "connections have"))
        } else {
            String::new()
        };
        durumlu(
            yetenek("bagla", "hazir", tf!("Bağlı: {}{s}.", "Connected: {}{s}.", tamam.join(", ")), &format!("sezgi bagla{ek}")),
            vec![alt(tf!("Bağlı yapay zekâ aracı: {}", "Connected AI tools: {}", tamam.join(", ")), "hazir"), kart_durumu.clone()],
        )
    } else {
        sonraki.get_or_insert_with(|| {
            komut(
                &format!("sezgi bagla{ek}"),
                t!("yapay zekâ aracınıza bağlar; her aracı tek tek sorar, geri alınır", "connects your AI tool; asks about each tool separately and can be undone"),
            )
        });
        let m = if sorunlu > 0 {
            t!("Bağlantı var ama çalışmıyor: ayrıntı için sezgi doktor.", "A connection exists but does not work: see sezgi doktor.")
        } else {
            t!("Henüz bağlı bir yapay zekâ aracı yok.", "No AI tool is connected yet.")
        };
        durumlu(yetenek("bagla", "kurulmamis", m.into(), &format!("sezgi bagla{ek}")), vec![alt(m.trim_end_matches('.'), "kurulmamis"), kart_durumu.clone()])
    };

    // 4. Yapay zekâya güvenli iş yaptırır: canlı mod, token, insan parolası
    let canli = if kaynaksiz || islem == 0 {
        durumlu(yetenek("canli", "uygulanamaz", islem_yok(), "sezgi canli kur"), vec![insan_durumu()])
    } else {
        match crate::canli::Durum::bul(klasor) {
            None => {
                sonraki.get_or_insert_with(|| {
                    komut(
                        "sezgi canli kur",
                        t!("isteğe bağlı: hedef, mod, oturum ve ilk token'ı sırayla kurar", "optional: sets up the target, mode, session and first token in turn"),
                    )
                });
                durumlu(
                    yetenek(
                        "canli",
                        "kurulmamis",
                        t!(
                            "Gözlem modunda: yapay zekâ işlemleri görür ve önizler, uygulamaya istek gitmez.",
                            "In observe mode: the AI sees and previews operations; no request reaches the application."
                        )
                        .into(),
                        "sezgi canli kur",
                    ),
                    vec![
                        alt(t!("Gözlem modu: uygulamaya istek gitmez", "Observe mode: no request reaches the application"), "kurulmamis"),
                        alt(t!("Token yok", "No token"), "kurulmamis"),
                        insan_durumu(),
                    ],
                )
            }
            Some(c) => {
                let ayar = c.ayar_oku();
                let mod_ = c.mod_();
                let hedef = ayar["hedef"]["url"].is_string();
                let test = ayar["hedef"]["test_kopyasi"] == true;
                let gecerli = crate::canli::token_listesi(&c).iter().filter(|t| t["durum"] == "gecerli").count();
                let mod_metni = match mod_.as_str() {
                    "okuma" => t!("okuma", "read"),
                    "yazma" => t!("yazma", "write"),
                    _ => t!("gözlem", "observe"),
                };
                // Gerçek veri: insan terminalde açtı; her yazma terminalde onaylanır, saatlik sınır vardır
                let hedef_metni = match (hedef, test, c.gercek_veri()) {
                    (true, _, true) => t!(
                        "hedef: GERÇEK VERİ, deneysel (her yazma terminalde insan parolasıyla onaylanır, saatlik sınır)",
                        "target: REAL DATA, experimental (every write is approved in a terminal with the human password, hourly limit)"
                    ),
                    (true, true, false) => t!("hedef: test kopyası", "target: test copy"),
                    (true, false, false) => t!("hedef tanımlı", "target set"),
                    _ => t!("hedef yok", "no target"),
                };
                let ayrinti = tf!("Mod: {mod_metni} · {hedef_metni} · {gecerli} geçerli token.", "Mode: {mod_metni} · {hedef_metni} · {gecerli} valid tokens.");
                let d = vec![
                    alt(tf!("Mod: {mod_metni}", "Mode: {mod_metni}"), if mod_ == "gozlem" { "kurulmamis" } else { "hazir" }),
                    alt(hedef_metni.split(" (").next().unwrap_or(hedef_metni).to_string(), if hedef { "hazir" } else { "kurulmamis" }),
                    alt(tf!("{gecerli} geçerli token", "{gecerli} valid tokens"), if gecerli > 0 { "hazir" } else { "kurulmamis" }),
                    insan_durumu(),
                ];
                let y = if mod_ != "gozlem" && hedef && gecerli > 0 {
                    yetenek("canli", "hazir", ayrinti, "sezgi canli")
                } else {
                    sonraki.get_or_insert_with(|| {
                        komut("sezgi canli kur", t!("isteğe bağlı: eksik adımları (hedef, mod, token) tamamlar", "optional: completes the missing steps (target, mode, token)"))
                    });
                    yetenek("canli", "kurulmamis", ayrinti, "sezgi canli kur")
                };
                durumlu(y, d)
            }
        }
    };

    let gor = yetenek(
        "gor",
        "hazir",
        t!("Şu an Ayna'dasınız; aynı bilgi terminalde de var.", "You are in Ayna now; the same facts are in the terminal too.").into(),
        &format!("sezgi rapor{ek}"),
    );

    // Santral dosyası kullanıcının yapılandırma klasöründe; klasör yoksa okunmaz (santral modülü o durumda çıkar)
    let uyeler = if crate::imza::yapilandirma_klasoru().is_some() { crate::santral::liste() } else { vec![] };
    let bu = crate::canli::mutlak_yol(klasor);
    let santral_komutu = format!("sezgi santral ekle {}", if varsayilan { ".sezgi" } else { ek.trim() });
    let santral = if uyeler.iter().any(|u| u["harita"].as_str() == Some(bu.as_str())) {
        yetenek(
            "santral",
            "hazir",
            tf!("Santralde {} uygulama; bu uygulama da içinde.", "{} {u} in the switchboard, this one included.", uyeler.len(); u = cogul(uyeler.len(), "application", "applications")),
            "sezgi santral",
        )
    } else if uyeler.is_empty() {
        yetenek("santral", "kurulmamis", t!("Santral boş: tek uygulama için gerekmez.", "The switchboard is empty: not needed for a single application.").into(), &santral_komutu)
    } else {
        yetenek(
            "santral",
            "kurulmamis",
            tf!("Santralde {} uygulama var; bu uygulama yok.", "{} {u} in the switchboard; this one is not among them.", uyeler.len(); u = cogul(uyeler.len(), "application", "applications")),
            &santral_komutu,
        )
    };

    let kesfet = if kaynaksiz {
        yetenek("kaynaksiz", "hazir", t!("Bu harita kaynaksız keşiften.", "This map comes from discovery without source code.").into(), kesfet_komutu())
    } else {
        yetenek(
            "kaynaksiz",
            "uygulanamaz",
            t!(
                "Bu uygulamanın kaynak kodu okundu; kaynaksız keşif kaynak kodu olmayan uygulamalar içindir.",
                "This application's source code was read; discovery without source code is for applications that have none."
            )
            .into(),
            kesfet_komutu(),
        )
    };

    json!({
        "surum": surum, "harita": true, "klasor": klasor_adi(proje), "cati": cati, "uygulama": ozet["uygulama"], "islem": islem, "kaynaksiz": kaynaksiz,
        "degisen": degisen, "anlam": anlam,
        "yetenekler": [anla, etki, bagla, canli, gor, santral, kesfet],
        "sonraki": sonraki,
    })
}

fn klasor_adi(p: &Path) -> String {
    std::fs::canonicalize(p).ok().as_deref().unwrap_or(p).file_name().map(|a| a.to_string_lossy().to_string()).unwrap_or_default()
}
