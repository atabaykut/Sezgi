//! Uygulamalar arası varlık eşlemesi (EntityLink). Aynı gerçek dünya varlığının farklı uygulamalardaki kayıtları (ör.
//! rezervasyondaki misafir ve transferdeki yolcu) şema düzeyinde eşlenir: "A uygulamasının varlık.alan'ı, B
//! uygulamasının varlık.alan'ıyla eşleşir". Tek tek kayıtların eşleşmesi saklanmaz; değerler hiçbir depoya girmez.
//!
//! Eşleme bir önermedir (`LINKS` ilişkisi), güveni kanıtlarından hesaplanır (kanıt modeliyle aynı kurallar):
//! - insan tanımı (`sezgi santral bagla-varlik`): USER_CONFIRMED; aynı önermede kaynak kodu ya da API şeması kanıtı
//!   yoksa en fazla observed. Dayanağı iki tarafın şema dosyalarıdır: şema değişince eşleme eski olur;
//! - açık dış kimlik: bir uygulamanın kodu ötekine onun alanıyla istek atıyor (`GET /rezervasyonlar/:rezervasyonNo`
//!   ← `${veri.rezervasyonRef}`): SOURCE_CODE; iki taraf da tek anlamlıysa ve adres birebir tutuyorsa certain. Kanıt
//!   santral çapraz bağlarından ve kaynak satırından her okumada yeniden çıkarılır (kod değişince düşer);
//! - gözlenen veri eşleşmesi (`sezgi santral varlik-gozle`): iki taraf test kopyasından insanın oturumuyla okunur,
//!   değerler yalnızca bellekte eşleştirilir; saklanan yalnızca sayılar ve orandır (RUNTIME_TRACE, observed). Oran
//!   düşükse gözlem çelişkidir;
//! - öneri motoru (ad, tip, dış kimlik adı benzerliği): yalnızca öneri, en fazla inferred; hiçbir zaman certain değil.
//!
//! Depo santral yapılandırma klasöründe JSON'dur (`santral-varliklar.json`); biçim kanıt deposuyla aynıdır.

use crate::kanit::{Dayanak, Depo, Gecerlilik, Guven, Kanit, Kapsam, KaynakTuru, Onerme};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

pub const DOSYA: &str = "santral-varliklar.json";

/// Santral üyesi (haritası olan uygulama)
pub struct Uye {
    pub ad: String,
    pub harita: PathBuf,
}

/// Santral listesindeki haritalı üyeler
pub fn uyeler(liste: &[Value]) -> Vec<Uye> {
    liste.iter().filter_map(|u| Some(Uye { ad: u["ad"].as_str()?.to_string(), harita: PathBuf::from(u["harita"].as_str()?) })).collect()
}

/// Üyenin uygulama kökü: harita `.sezgi` klasörüyse üstü, değilse kanıt deposunun kaydettiği kök
fn kok(u: &Uye) -> Option<PathBuf> {
    if u.harita.file_name().is_some_and(|a| a == ".sezgi") {
        return u.harita.parent().map(Path::to_path_buf);
    }
    let d: Depo = crate::guvenli::oku_harita(&u.harita.join(crate::kanit::DOSYA)).and_then(|m| serde_json::from_str(&m).ok())?;
    d.kok.map(|r| u.harita.join(r))
}

/// Bir uygulamanın varlığı (tablo): alanlar (ad -> (tip, şema dosyası satırı)) ve dayanaklar (şema dosyaları, özetiyle)
#[derive(Default)]
pub struct Varlik {
    pub alanlar: BTreeMap<String, (String, Option<(String, usize)>)>,
    pub dayanak: Vec<Dayanak>,
}

/// Üyenin varlıkları (süreç içinde önbellekli): harita ve kanıt deposu dosyaları değişmedikçe yeniden okunmaz; santral MCP
/// her çağrıda bütün üyelerin kanıt deposunu ayrıştırmaz
pub fn varliklar(u: &Uye) -> std::sync::Arc<BTreeMap<String, Varlik>> {
    type Onbellek = HashMap<PathBuf, (Vec<Option<std::time::SystemTime>>, std::sync::Arc<BTreeMap<String, Varlik>>)>;
    static ONBELLEK: std::sync::OnceLock<std::sync::Mutex<Onbellek>> = std::sync::OnceLock::new();
    let imza: Vec<Option<std::time::SystemTime>> = ["kapilar.json", crate::kanit::DOSYA].iter().map(|d| u.harita.join(d).metadata().and_then(|m| m.modified()).ok()).collect();
    let mut o = ONBELLEK.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((i, v)) = o.get(&u.harita) {
        if *i == imza {
            return v.clone();
        }
    }
    let v = std::sync::Arc::new(varliklari_oku(u));
    o.insert(u.harita.clone(), (imza, v.clone()));
    v
}

/// Üyenin varlıkları kanıt grafiğinden: tablolar (`db:` düğümleri) ve sütunları (`field:Model.alan`; Prisma, SQLModel,
/// SQLAlchemy), ayrıca tabloya yazan işlemlerin girdi alanları. Dayanak: tablonun ve sütunun kanıtlarının dosyaları
fn varliklari_oku(u: &Uye) -> BTreeMap<String, Varlik> {
    let mut v: BTreeMap<String, Varlik> = BTreeMap::new();
    let Some(m) = crate::kanit::Model::yukle(&u.harita) else { return v };
    let dayanak_ekle = |x: &mut Varlik, o: &Onerme| {
        for k in o.hesap.destek.iter().filter_map(|id| m.kanit(id)) {
            for d in &k.dayanak {
                if !x.dayanak.contains(d) {
                    x.dayanak.push(d.clone());
                }
            }
        }
    };
    for o in m.depo.onermeler.iter().filter(|o| o.yuklem == "VAR" && o.varlik.starts_with("db:")) {
        dayanak_ekle(v.entry(o.varlik[3..].to_string()).or_default(), o);
    }
    // Sütunlar: değerinde modeli olan alan düğümleri
    for o in m.depo.onermeler.iter().filter(|o| o.yuklem == "VAR" && o.varlik.starts_with("field:")) {
        let Some(model) = o.deger["model"].as_str() else { continue };
        let Some(alan) = o.varlik.strip_prefix(&format!("field:{model}.")) else { continue };
        let yer = o
            .hesap
            .destek
            .iter()
            .filter_map(|id| m.kanit(id))
            .flat_map(|k| k.konum.first())
            .find_map(|k| k.rsplit_once(':').and_then(|(d, s)| Some((d.to_string(), s.parse().ok()?))));
        let x = v.entry(model.to_string()).or_default();
        x.alanlar.insert(alan.to_string(), (o.deger["tip"].as_str().unwrap_or("").to_string(), yer));
        dayanak_ekle(x, o);
    }
    // Tabloya yazan işlemin girdi alanları (şema okunamayan çatılarda da öneri yapılabilsin)
    let harita: Value = crate::guvenli::oku_harita(&u.harita.join("kapilar.json")).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_default();
    for kp in harita["endpointler"].as_array().into_iter().flatten() {
        for t in kp["veri"]["yazar"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            let Some(x) = v.get_mut(t) else { continue };
            for g in kp["govde"].as_array().into_iter().flatten() {
                for (alan, a) in g["sema"]["alanlar"].as_object().into_iter().flatten() {
                    x.alanlar.entry(alan.clone()).or_insert_with(|| (a["tip"].as_str().unwrap_or("").to_string(), None));
                }
            }
        }
    }
    v
}

/// "uygulama:Varlik.alan" -> (uygulama, Varlik, alan)
pub fn coz(r: &str) -> Result<(String, String, String), String> {
    let hata =
        || tf!("'{r}': biçim \"uygulama:Varlik.alan\" olmalı (ör. rezervasyon:Misafir.pasaportNo)", "'{r}': the form must be \"app:Entity.field\" (e.g. booking:Guest.passportNo)");
    let (u, va) = r.split_once(':').ok_or_else(hata)?;
    let (v, a) = va.split_once('.').ok_or_else(hata)?;
    if u.is_empty() || v.is_empty() || a.is_empty() || a.contains('.') {
        return Err(hata());
    }
    Ok((u.to_string(), v.to_string(), a.to_string()))
}

fn dosya() -> Result<PathBuf, String> {
    crate::imza::yapilandirma_klasoru().map(|k| k.join(DOSYA)).ok_or_else(|| t!("yapılandırma klasörü bulunamadı", "configuration folder not found").into())
}

fn yukle() -> Depo {
    dosya().ok().and_then(|p| crate::guvenli::oku_harita(&p)).and_then(|m| serde_json::from_str::<Depo>(&m).ok()).unwrap_or_else(|| Depo {
        bicim: crate::kanit::BICIM,
        uretici: format!("sezgi {}", env!("CARGO_PKG_VERSION")),
        ..Depo::default()
    })
}

/// Dayanakların bugünkü özetleri: yol "uygulama:göreli/yol" biçimindedir
fn gecerlilik(depo: &mut Depo, uyeler: &[Uye]) {
    let kokler: HashMap<&str, Option<PathBuf>> = uyeler.iter().map(|u| (u.ad.as_str(), kok(u))).collect();
    let mut onbellek: HashMap<String, Option<String>> = HashMap::new();
    depo.gecerlilik_isaretle(&mut |yol: &str| {
        onbellek
            .entry(yol.to_string())
            .or_insert_with(|| {
                let (u, g) = yol.split_once(':')?;
                crate::kanit::dosya_ozeti(&kokler.get(u)?.as_ref()?.join(g))
            })
            .clone()
    });
    depo.hesapla();
}

/// İnsan tanımı: iki tarafın varlığı ve alanı uygulamasında olmalı. Kanıt USER_CONFIRMED (beyan certain; kaynak kanıtı
/// olmadan hesap en fazla observed verir)
pub fn bagla(uyeler: &[Uye], a: &str, b: &str, anahtar: Option<&str>) -> Result<String, String> {
    let mut dayanak = vec![];
    let mut konum = vec![];
    let mut taraflar = vec![];
    for r in [a, b] {
        let (u, v, alan) = coz(r)?;
        let uye = uyeler.iter().find(|x| x.ad == u).ok_or_else(|| format!("santralde '{u}' yok (sezgi santral)"))?;
        let vs = varliklar(uye);
        let x = vs.get(&v).ok_or_else(|| {
            tf!("'{u}' uygulamasında '{v}' varlığı yok; bilinenler: {}", "app '{u}' has no entity '{v}'; known: {}", vs.keys().cloned().collect::<Vec<_>>().join(", "))
        })?;
        let (_, yer) = x.alanlar.get(&alan).ok_or_else(|| {
            tf!(
                "'{u}:{v}' varlığında '{alan}' alanı yok; bilinenler: {}",
                "entity '{u}:{v}' has no field '{alan}'; known: {}",
                x.alanlar.keys().cloned().collect::<Vec<_>>().join(", ")
            )
        })?;
        dayanak.extend(x.dayanak.iter().map(|d| Dayanak { yol: format!("{u}:{}", d.yol), sha256: d.sha256.clone() }));
        if let Some((dosya, satir)) = yer {
            konum.push(format!("{u}:{dosya}:{satir}"));
        }
        taraflar.push(format!("{u}:{v}.{alan}"));
    }
    if taraflar[0] == taraflar[1] {
        return Err(t!("bir alan kendisiyle eşlenemez", "a field cannot be mapped to itself").into());
    }
    taraflar.sort();
    let (sol, sag) = (format!("field:{}", taraflar[0]), format!("field:{}", taraflar[1]));
    let mut kanit = Kanit::yeni(KaynakTuru::UserConfirmed, &format!("sezgi {} santral", env!("CARGO_PKG_VERSION")), dayanak, konum, Some(Guven::Certain));
    kanit.ozet = json!({ "tur": "kullanici_tanimi", "tanimlayan": "yerel komut satırı" });
    kanit.zaman = crate::simdi();
    kanit.id = Kanit::kimlik_hesapla(&kanit);
    let mut depo = yukle();
    eski_bicimi_tasi(&mut depo);
    let id = crate::kanit::onerme_kimligi(&sol, "LINKS", Some(&sag));
    if !depo.onermeler.iter().any(|o| o.id == id) {
        depo.onermeler.push(Onerme {
            id: id.clone(),
            varlik: sol.clone(),
            yuklem: "LINKS".into(),
            hedef: Some(sag.clone()),
            deger: json!({}),
            notlar: vec![ESLEME_NOTU.into()],
            hesap: Default::default(),
        });
    }
    if let Some(a) = anahtar {
        if let Some(o) = depo.onermeler.iter_mut().find(|o| o.id == id) {
            o.notlar.retain(|n| !n.starts_with(ANAHTAR_NOTU));
            o.notlar.push(format!("{ANAHTAR_NOTU}{a}"));
        }
    }
    if !depo.kanitlar.iter().any(|k| k.id == kanit.id) {
        depo.baglar.push(crate::kanit::Bag {
            kanit: kanit.id.clone(),
            onerme: id.clone(),
            yon: crate::kanit::Yon::Destek,
            deger: json!({}),
            kapsam: Kapsam::KapaliDunya,
            kapsam_disi: vec![],
        });
        depo.kanitlar.push(kanit);
    }
    depo.revizyon += 1;
    gecerlilik(&mut depo, uyeler);
    kaydet(&depo)?;
    Ok(format!("{} ↔ {}", taraflar[0], taraflar[1]))
}

/// Eşleme önermesinin sabit notu
const ESLEME_NOTU: &str = "eş adlılık ve yazım farkı kayıt düzeyinde eşleşmeyi bozabilir; eşleme toplu yazmanın tek dayanağı olamaz";
/// Eşleme anahtarının (insanın verdiği ad) notu
const ANAHTAR_NOTU: &str = "anahtar: ";

fn kaydet(depo: &Depo) -> Result<(), String> {
    let p = dosya()?;
    if let Some(k) = p.parent() {
        crate::guvenli::gizli_klasor(k).map_err(|e| e.to_string())?;
    }
    crate::guvenli::atomik_yaz_gizli(&p, serde_json::to_string_pretty(depo).map_err(|e| e.to_string())?.as_bytes())
}

/// Eşleme önermesi (iki taraf sıralı): yoksa eklenir; kimliği döner
fn esleme_onermesi(depo: &mut Depo, a: &str, b: &str) -> String {
    let (sol, sag) = if a < b { (format!("field:{a}"), format!("field:{b}")) } else { (format!("field:{b}"), format!("field:{a}")) };
    let id = crate::kanit::onerme_kimligi(&sol, "LINKS", Some(&sag));
    if !depo.onermeler.iter().any(|o| o.id == id) {
        depo.onermeler.push(Onerme {
            id: id.clone(),
            varlik: sol,
            yuklem: "LINKS".into(),
            hedef: Some(sag),
            deger: json!({}),
            notlar: vec![ESLEME_NOTU.into()],
            hesap: Default::default(),
        });
    }
    id
}

/// Kanıtı eşlemeye bağlar (aynı kanıt bir kez)
fn kanit_bagla(depo: &mut Depo, onerme: &str, kanit: Kanit, deger: Value) {
    if !depo.baglar.iter().any(|b| b.kanit == kanit.id && b.onerme == onerme) {
        depo.baglar.push(crate::kanit::Bag {
            kanit: kanit.id.clone(),
            onerme: onerme.to_string(),
            yon: crate::kanit::Yon::Destek,
            deger,
            kapsam: Kapsam::KapaliDunya,
            kapsam_disi: vec![],
        });
    }
    if !depo.kanitlar.iter().any(|k| k.id == kanit.id) {
        depo.kanitlar.push(kanit);
    }
}

/// Bir satırdaki dizgiler (tırnak, tek tırnak, ters tırnak)
fn dizgiler(satir: &str) -> Vec<String> {
    let mut v = vec![];
    let b: Vec<char> = satir.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if matches!(b[i], '"' | '\'' | '`') {
            let q = b[i];
            let j = (i + 1..b.len()).find(|&j| b[j] == q && b[j - 1] != '\\').unwrap_or(b.len());
            v.push(b[i + 1..j.min(b.len())].iter().collect());
            i = j + 1;
        } else {
            i += 1;
        }
    }
    v
}

/// `${veri.rezervasyonRef}` / `{veri.rezervasyonRef}` -> "rezervasyonRef": ifadenin son tanımlayıcısı
fn ifade_alani(parca: &str) -> Option<String> {
    let ic = parca.strip_prefix("${").or_else(|| parca.strip_prefix('{'))?.strip_suffix('}')?;
    let son = ic.rsplit(['.', '[']).next()?.trim().trim_end_matches([']', '?', '!', ')', '(']).trim_matches(['"', '\'']);
    (!son.is_empty() && son.chars().all(|c| c.is_alphanumeric() || c == '_')).then(|| son.to_string())
}

/// Açık dış kimlik kanıtları: santral çapraz bağlarındaki her istekte, hedef kapının yol (ya da sorgu) parametresine
/// kaynak uygulamanın bir alanı veriliyorsa (`${veri.rezervasyonRef}`) o iki alan eşleşir. Kanıt kaynak satırından her
/// okumada yeniden çıkarılır; depoya yazılmaz
fn dis_kimlik_ekle(depo: &mut Depo, uyeler: &[Uye]) {
    let Ok(d) = dosya() else { return };
    let baglar: Value = crate::guvenli::oku_harita(&d.with_file_name("santral-baglar.json")).and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_default();
    let kaynaklar: HashMap<String, PathBuf> = crate::santral::liste().iter().filter_map(|u| Some((u["ad"].as_str()?.to_string(), PathBuf::from(u["kaynak"].as_str()?)))).collect();
    for b in baglar["baglar"].as_array().into_iter().flatten() {
        let (Some(kaynak_ad), Some((hedef_ad, kapi)), Some(dosya), Some(satir)) =
            (b["kaynak"].as_str(), b["kapi"].as_str().and_then(|k| k.split_once(": ")), b["dosya"].as_str(), b["satir"].as_u64())
        else {
            continue;
        };
        let (Some(ub), Some(ua), Some(kok_b)) = (uyeler.iter().find(|u| u.ad == kaynak_ad), uyeler.iter().find(|u| u.ad == hedef_ad), kaynaklar.get(kaynak_ad)) else { continue };
        let Some(metin) = crate::guvenli::oku(&kok_b.join(dosya)) else { continue };
        let satirlar: Vec<&str> = metin.lines().skip(satir.saturating_sub(1) as usize).take(3).collect();
        let sablon: Vec<String> = crate::santral::parcalar(kapi.split_once(' ').map(|x| x.1).unwrap_or(kapi));
        // (hedef parametre, kaynak alan) çiftleri
        let mut ciftler: Vec<(String, String)> = vec![];
        for s in satirlar.iter().flat_map(|l| dizgiler(l)) {
            let Some(aday) = crate::santral::aday(&s) else { continue };
            let parca = crate::santral::parcalar(&aday);
            // Genel önek istemcinin taban adresinde kalabilir: yol sondan hizalanır
            if parca.len() > sablon.len() || parca.is_empty() {
                continue;
            }
            for (p, t) in parca.iter().rev().zip(sablon.iter().rev()) {
                if let (Some(pa), Some(fb)) = (t.strip_prefix(':'), ifade_alani(p)) {
                    ciftler.push((pa.to_string(), fb));
                }
            }
            // Sorgu: ?ad=${ifade}
            for q in s.split_once('?').map(|x| x.1).unwrap_or("").split('&') {
                if let Some((ad, deger)) = q.split_once('=') {
                    if let Some(fb) = ifade_alani(deger) {
                        ciftler.push((ad.to_string(), fb));
                    }
                }
            }
        }
        if ciftler.is_empty() {
            continue;
        }
        let (va, vb) = (varliklar(ua), varliklar(ub));
        let kesin_bag = b["damga"] == "kesin";
        for (pa, fb) in ciftler {
            let a_aday: Vec<(String, String)> = va.iter().flat_map(|(e, x)| x.alanlar.keys().filter(|a| kucuk(a) == kucuk(&pa)).map(move |a| (e.clone(), a.clone()))).collect();
            let b_aday: Vec<(String, String)> = vb.iter().flat_map(|(e, x)| x.alanlar.keys().filter(|a| kucuk(a) == kucuk(&fb)).map(move |a| (e.clone(), a.clone()))).collect();
            // Tek anlamlı veri akışı: iki tarafta da tek aday ve adres birebir
            let tek = a_aday.len() == 1 && b_aday.len() == 1 && kesin_bag;
            for (ea, aa) in &a_aday {
                for (eb, ab) in &b_aday {
                    let ozet = crate::kanit::dosya_ozeti(&kok_b.join(dosya));
                    let dayanak = ozet.map(|o| vec![Dayanak { yol: format!("{kaynak_ad}:{dosya}"), sha256: o }]).unwrap_or_default();
                    let mut k = Kanit::yeni(
                        KaynakTuru::SourceCode,
                        &format!("sezgi {} santral", env!("CARGO_PKG_VERSION")),
                        dayanak,
                        vec![format!("{kaynak_ad}:{dosya}:{satir}")],
                        Some(if tek { Guven::Certain } else { Guven::Inferred }),
                    );
                    k.ozet = json!({ "tur": "dis_kimlik", "istek": format!("{hedef_ad}: {kapi}"), "parametre": pa, "alan": fb });
                    if !tek {
                        k.sinirlamalar.push("veri akışı tek anlamlı değil (birden fazla aday alan ya da adres tahmini)".into());
                    }
                    k.zaman = crate::simdi();
                    k.id = Kanit::kimlik_hesapla(&k);
                    let id = esleme_onermesi(depo, &format!("{hedef_ad}:{ea}.{aa}"), &format!("{kaynak_ad}:{eb}.{ab}"));
                    kanit_bagla(depo, &id, k, json!({}));
                }
            }
        }
    }
}

/// Eski biçim: anahtar bağın ve önermenin değerindeydi; artık önerme notunda
fn eski_bicimi_tasi(depo: &mut Depo) {
    for o in depo.onermeler.iter_mut().filter(|o| o.yuklem == "LINKS") {
        if let Some(a) = o.deger["anahtar"].as_str() {
            if !o.notlar.iter().any(|n| n.starts_with(ANAHTAR_NOTU)) {
                o.notlar.push(format!("{ANAHTAR_NOTU}{a}"));
            }
        }
        o.deger = json!({});
    }
    for b in depo.baglar.iter_mut() {
        if b.deger.get("anahtar").is_some() {
            b.deger = json!({});
        }
    }
}

/// Gözlenen veri eşleşmesi: iki taraf insanın oturumuyla test kopyasından okunur (tablonun okuma işlemi), alanın
/// değerleri BELLEKTE eşleştirilir; saklanan yalnızca örnek sayıları, eşleşen sayısı ve orandır. Oran 0,5 ve üstü
/// destek, altı çelişki (RUNTIME_TRACE, observed). Aynı eşlemenin önceki gözlemi yenisiyle değişir
pub fn gozle(uyeler: &[Uye], esleme: &str, ornek: usize) -> Result<String, String> {
    let (sol, sag) = esleme
        .split_once('=')
        .ok_or(t!("eşleme \"uygulama:Varlik.alan=uygulama:Varlik.alan\" biçiminde olmalı", "the mapping must have the form \"app:Entity.field=app:Entity.field\""))?;
    let mut taraflar = vec![];
    for r in [sol.trim(), sag.trim()] {
        let (u, v, alan) = coz(r)?;
        let uye = uyeler.iter().find(|x| x.ad == u).ok_or_else(|| format!("santralde '{u}' yok"))?;
        let vs = varliklar(uye);
        let x = vs.get(&v).ok_or_else(|| tf!("'{u}' uygulamasında '{v}' varlığı yok", "app '{u}' has no entity '{v}'"))?;
        if !x.alanlar.contains_key(&alan) {
            return Err(tf!("'{u}:{v}' varlığında '{alan}' alanı yok", "entity '{u}:{v}' has no field '{alan}'"));
        }
        // Tablonun okuma işlemi: tabloyu okuyan, yol parametresi olmayan GET
        let m = crate::kanit::Model::yukle(&uye.harita).ok_or(t!("kanıt deposu okunamadı (sezgi init)", "could not read the evidence store (sezgi init)"))?;
        let mut okumalar: Vec<String> = m
            .graf
            .gelen(&format!("db:{v}"))
            .iter()
            .map(|&i| &m.depo.onermeler[i])
            .filter(|o| o.yuklem == "READS")
            .map(|o| crate::graf::ad(&o.varlik).split(['@', '#']).next().unwrap_or("").to_string())
            .filter(|k| k.starts_with("GET ") && !k.contains("/:"))
            .collect();
        okumalar.sort();
        let okuma = okumalar.first().cloned().ok_or_else(|| {
            tf!("'{u}' uygulamasında {v} tablosunu okuyan parametresiz GET işlemi yok", "app '{u}' has no GET operation without parameters that reads the {v} table")
        })?;
        let h = crate::mcp::Harita::yukle(&uye.harita)?;
        let kapi = h.bul(&okuma).cloned().ok_or_else(|| format!("{okuma} haritada yok"))?;
        let dayanak: Vec<Dayanak> = x.dayanak.iter().map(|d| Dayanak { yol: format!("{u}:{}", d.yol), sha256: d.sha256.clone() }).collect();
        taraflar.push((format!("{u}:{v}.{alan}"), alan, uye, kapi, okuma, dayanak));
    }
    // Okuma ve eşleştirme: değerler bu fonksiyonun dışına çıkmaz
    let mut kumeler: Vec<Vec<String>> = vec![];
    for (_, alan, uye, kapi, _, _) in &taraflar {
        let cevap = crate::canli::insan_okumasi(&uye.harita, kapi)?;
        let mut v = vec![];
        topla(&cevap, alan, ornek, &mut v);
        if v.is_empty() {
            return Err(tf!("{}: cevapta '{alan}' alanı bulunamadı", "{}: field '{alan}' not found in the response", uye.ad));
        }
        kumeler.push(v);
    }
    let a: std::collections::HashSet<&String> = kumeler[0].iter().collect();
    let eslesen = kumeler[1].iter().filter(|x| a.contains(x)).count();
    let (na, nb) = (kumeler[0].len(), kumeler[1].len());
    let oran = eslesen as f64 / nb as f64;
    drop(kumeler);
    let mut dayanak = taraflar[0].5.clone();
    dayanak.extend(taraflar[1].5.clone());
    let mut k = Kanit::yeni(
        KaynakTuru::RuntimeTrace,
        &format!("sezgi {} santral", env!("CARGO_PKG_VERSION")),
        dayanak,
        vec![format!("{} {}", taraflar[0].0.split(':').next().unwrap_or(""), taraflar[0].4), format!("{} {}", taraflar[1].0.split(':').next().unwrap_or(""), taraflar[1].4)],
        Some(Guven::Observed),
    );
    k.ozet = json!({ "tur": "gozlenen_eslesme", "ornek_a": na, "ornek_b": nb, "eslesen": eslesen, "oran": (oran * 1000.0).round() / 1000.0,
        "okuma_a": taraflar[0].4, "okuma_b": taraflar[1].4, "ortam": "test kopyası, insanın oturumu" });
    k.sinirlamalar.push("örnekleme: yalnızca okunan kayıtlar; değerler saklanmadı".into());
    k.zaman = crate::simdi();
    k.id = Kanit::kimlik_hesapla(&k);
    let mut depo = yukle();
    eski_bicimi_tasi(&mut depo);
    let id = esleme_onermesi(&mut depo, &taraflar[0].0, &taraflar[1].0);
    // Önceki gözlem yenisiyle değişir
    let eski: Vec<String> = depo
        .baglar
        .iter()
        .filter(|b| b.onerme == id)
        .filter(|b| depo.kanitlar.iter().any(|x| x.id == b.kanit && x.ozet["tur"] == "gozlenen_eslesme"))
        .map(|b| b.kanit.clone())
        .collect();
    depo.baglar.retain(|b| !(b.onerme == id && eski.contains(&b.kanit)));
    depo.kanitlar.retain(|x| !eski.contains(&x.id) || depo.baglar.iter().any(|b| b.kanit == x.id));
    let destek = oran >= 0.5;
    kanit_bagla(&mut depo, &id, k, if destek { json!({}) } else { json!({ "eslesmedi": true }) });
    depo.revizyon += 1;
    gecerlilik(&mut depo, uyeler);
    kaydet(&depo)?;
    Ok(tf!(
        "✓ Gözlem: {} değerlerinin {eslesen}/{nb}'i (%{:.1}) {} değerleri arasında bulundu (örnek: {na} ve {nb} kayıt).\n  Kanıt: RUNTIME_TRACE (gözlem), {}. Değerler yalnızca bellekte eşleştirildi; saklanan yalnızca sayılar ve oran.\n",
        "✓ Observation: {eslesen}/{nb} of the {} values ({:.1}%) were found among the {} values (sample: {na} and {nb} records).\n  Evidence: RUNTIME_TRACE (observed), {}. Values were matched in memory only; only counts and the ratio are stored.\n",
        taraflar[1].0,
        oran * 100.0,
        taraflar[0].0,
        if destek { t!("eşlemeyi destekliyor", "supports the mapping") } else { t!("eşlemeyle çelişiyor (oran 0,5'in altında)", "contradicts the mapping (ratio below 0.5)") }
    ))
}

/// Cevaptaki `alan` değerleri (metin ya da sayı), en çok `n` tane
fn topla(v: &Value, alan: &str, n: usize, out: &mut Vec<String>) {
    if out.len() >= n {
        return;
    }
    match v {
        Value::Array(a) => a.iter().for_each(|x| topla(x, alan, n, out)),
        Value::Object(o) => {
            for (k, x) in o {
                if out.len() >= n {
                    return;
                }
                match x {
                    Value::String(s) if kucuk(k) == kucuk(alan) => out.push(s.clone()),
                    Value::Number(s) if kucuk(k) == kucuk(alan) => out.push(s.to_string()),
                    _ => topla(x, alan, n, out),
                }
            }
        }
        _ => {}
    }
}

/// Hesaplanmış eşleme
pub struct Esleme {
    pub a: String,
    pub b: String,
    pub anahtar: Option<String>,
    pub guven: Guven,
    pub gecerlilik: Gecerlilik,
    pub neden: Option<String>,
    pub json: Value,
}

impl Esleme {
    /// "uygulama:Varlik" (alan olmadan)
    pub fn varliklar(&self) -> [(String, String); 2] {
        let parca = |r: &str| coz(r).map(|(u, v, _)| (u, v)).unwrap_or_default();
        [parca(&self.a), parca(&self.b)]
    }
}

/// Eşlemeler, bugünkü dayanaklarla yeniden hesaplanmış güvenleriyle
pub fn eslemeler(uyeler: &[Uye]) -> Vec<Esleme> {
    let mut depo = yukle();
    eski_bicimi_tasi(&mut depo);
    dis_kimlik_ekle(&mut depo, uyeler);
    gecerlilik(&mut depo, uyeler);
    let kanitlar: HashMap<&str, &Kanit> = depo.kanitlar.iter().map(|k| (k.id.as_str(), k)).collect();
    depo.onermeler
        .iter()
        .filter(|o| o.yuklem == "LINKS")
        .map(|o| {
            let (a, b) = (o.varlik.trim_start_matches("field:").to_string(), o.hedef.as_deref().unwrap_or("").trim_start_matches("field:").to_string());
            let k: Vec<Value> = depo
                .baglar
                .iter()
                .filter(|g| g.onerme == o.id)
                .filter_map(|g| kanitlar.get(g.kanit.as_str()))
                .map(|k| json!({ "id": k.id, "kaynak_turu": k.kaynak_turu, "konum": k.konum, "katki": k.katki(), "gecerli": k.gecerli, "zaman": k.zaman, "ozet": k.ozet }))
                .collect();
            let anahtar = o.notlar.iter().find_map(|n| n.strip_prefix(ANAHTAR_NOTU)).map(String::from);
            Esleme {
                json: json!({ "id": o.id, "a": a, "b": b, "anahtar": anahtar, "guven": o.hesap.guven, "gecerlilik": o.hesap.gecerlilik,
                    "neden": o.hesap.neden, "kapsam_disi": o.hesap.kapsam_disi, "kanitlar": k }),
                a,
                b,
                anahtar,
                guven: o.hesap.guven,
                gecerlilik: o.hesap.gecerlilik,
                neden: o.hesap.neden.clone(),
            }
        })
        .collect()
}

/// Dilden bağımsız tip sınıfı (Prisma `String`, Python `str`, TS `string` aynı): öneri motorunun karşılaştırması için
fn tip_sinifi(t: &str) -> String {
    let t = t.replace("Optional[", "").replace("| None", "").replace(['?', ']', ' '], "");
    match kucuk(&t).as_str() {
        "string" | "str" | "text" | "varchar" | "uuid" | "char" => "metin".into(),
        "int" | "integer" | "bigint" | "float" | "number" | "decimal" | "double" => "sayi".into(),
        "datetime" | "date" | "timestamp" => "zaman".into(),
        "bool" | "boolean" => "mantik".into(),
        x => x.to_string(),
    }
}

fn kucuk(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Kimlik alanı olarak anlamsız ortak adlar: öneri yapılmaz
const GENEL_ALANLAR: [&str; 8] = ["id", "createdat", "updatedat", "olusturma", "guncelleme", "ad", "name", "durum"];
/// Başka bir varlığı adlandıran dış kimlik sonekleri (ör. rezervasyonRef, reservation_id)
const DIS_KIMLIK: [&str; 7] = ["ref", "no", "id", "kodu", "kod", "numara", "number"];

/// Öneri motoru: aynı ad ve tipte alan; başka uygulamanın varlığını adlandıran dış kimlik alanı. Yalnızca öneridir: güven
/// en fazla inferred (hiçbir zaman certain), insan onaylayınca eşleme olur
pub fn oneriler(uyeler: &[Uye]) -> Vec<Value> {
    let varlar: Vec<(String, std::sync::Arc<BTreeMap<String, Varlik>>)> = uyeler.iter().map(|u| (u.ad.clone(), varliklar(u))).collect();
    let mevcut: BTreeSet<(String, String)> = eslemeler(uyeler).into_iter().flat_map(|e| [(e.a.clone(), e.b.clone()), (e.b, e.a)]).collect();
    let eslenen: BTreeSet<String> = mevcut.iter().map(|x| x.0.clone()).collect();
    let mut sonuc: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut ekle = |a: String, b: String, neden: String| {
        if eslenen.contains(&a) || eslenen.contains(&b) {
            return;
        }
        let (x, y) = if a < b { (a, b) } else { (b, a) };
        sonuc.entry((x, y)).or_insert(neden);
    };
    for (i, (ua, va)) in varlar.iter().enumerate() {
        for (ub, vb) in varlar.iter().skip(i + 1) {
            for (ea, xa) in va.iter() {
                for (eb, xb) in vb.iter() {
                    // Aynı ad ve tip
                    for (alan, (tip, _)) in &xa.alanlar {
                        let k = kucuk(alan);
                        if k.len() < 4 || GENEL_ALANLAR.contains(&k.as_str()) {
                            continue;
                        }
                        if let Some((ad_b, (tip_b, _))) = xb.alanlar.iter().find(|(a, _)| kucuk(a) == k) {
                            if tip_sinifi(tip) == tip_sinifi(tip_b) {
                                ekle(format!("{ua}:{ea}.{alan}"), format!("{ub}:{eb}.{ad_b}"), "aynı ad ve tip".into());
                            }
                        }
                    }
                    // Dış kimlik: bir taraftaki alan öteki tarafın varlığını adlandırıyor (rezervasyonRef -> Rezervasyon)
                    for ((u1, e1, x1), (u2, e2, x2)) in [((ua, ea, xa), (ub, eb, xb)), ((ub, eb, xb), (ua, ea, xa))] {
                        let ad = kucuk(e1);
                        for alan in x2.alanlar.keys() {
                            let k = kucuk(alan);
                            let Some(sonek) = k.strip_prefix(&ad) else { continue };
                            if !DIS_KIMLIK.contains(&sonek) {
                                continue;
                            }
                            // Karşı tarafın anahtarı: aynı adlı alan, yoksa "<varlık>no", "no" ya da "id"
                            let anahtar = x1
                                .alanlar
                                .keys()
                                .find(|a| kucuk(a) == k)
                                .or_else(|| x1.alanlar.keys().find(|a| kucuk(a) == format!("{ad}no")))
                                .or_else(|| x1.alanlar.keys().find(|a| kucuk(a) == "no"))
                                .or_else(|| x1.alanlar.keys().find(|a| kucuk(a) == "id"));
                            if let Some(an) = anahtar {
                                ekle(
                                    format!("{u1}:{e1}.{an}"),
                                    format!("{u2}:{e2}.{alan}"),
                                    format!("dış kimlik adı: '{alan}' alanı {u1} uygulamasının {e1} varlığını adlandırıyor"),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    sonuc
        .into_iter()
        .map(|((a, b), neden)| json!({ "a": a, "b": b, "neden": neden, "guven": Guven::Inferred, "kaynak_turu": "DATABASE_SCHEMA", "not": "ad sezgisi: yalnızca öneri" }))
        .collect()
}

fn etiket(g: &str) -> &'static str {
    match g {
        "certain" => t!("[Kesin]", "[Certain]"),
        "observed" => t!("[Gözlem]", "[Observed]"),
        "inferred" => t!("[Yorum]", "[Inferred]"),
        _ => t!("[Bilinmiyor]", "[Unknown]"),
    }
}

/// `sezgi santral varliklar` ve santral MCP `varliklar`: eşlemeler (güven, geçerlilik, kanıt türü) ve öneriler
pub fn metin(uyeler: &[Uye]) -> String {
    let e = eslemeler(uyeler);
    let o = oneriler(uyeler);
    let mut s = tf!("Varlık eşlemeleri ({}):\n", "Entity mappings ({}):\n", e.len());
    if e.is_empty() {
        s.push_str(t!(
            "  yok (insan tanımı: sezgi santral bagla-varlik <uygulama:Varlik.alan> <uygulama:Varlik.alan>)\n",
            "  none (human definition: sezgi switchboard link-entity <app:Entity.field> <app:Entity.field>)\n"
        ));
    }
    for x in &e {
        let turler: BTreeSet<String> = x.json["kanitlar"].as_array().into_iter().flatten().filter_map(|k| k["kaynak_turu"].as_str().map(String::from)).collect();
        let durum = match x.gecerlilik {
            Gecerlilik::Gecerli => t!("geçerli", "valid").to_string(),
            _ => tf!("eski: {}", "stale: {}", x.neden.as_deref().unwrap_or(t!("dayanak değişti", "basis changed"))),
        };
        s.push_str(&tf!(
            "  {} ↔ {} {} · {durum} · kanıt: {}{}\n",
            "  {} ↔ {} {} · {durum} · evidence: {}{}\n",
            x.a,
            x.b,
            etiket(x.guven.ad()),
            turler.into_iter().collect::<Vec<_>>().join(", "),
            x.anahtar.as_ref().map(|a| tf!(" · anahtar: {a}", " · key: {a}")).unwrap_or_default()
        ));
    }
    s.push_str(&tf!(
        "Öneriler ({}; yalnızca ad, tip ve şema benzerliği: en fazla yorum, onaylamak için bagla-varlik):\n",
        "Suggestions ({}; name, type and schema similarity only: at most inferred, confirm with link-entity):\n",
        o.len()
    ));
    for x in o.iter().take(50) {
        s.push_str(&tf!(
            "  {} ↔ {} [Yorum] · {}\n",
            "  {} ↔ {} [Inferred] · {}\n",
            x["a"].as_str().unwrap_or(""),
            x["b"].as_str().unwrap_or(""),
            x["neden"].as_str().unwrap_or("")
        ));
    }
    s.push_str(t!(
        "Eşleme şema düzeyindedir: kayıt değerleri saklanmaz; kayıtları birleştirmek çalışma anında, izinli işlemlerle yapılır.\n",
        "Mappings are at schema level: no record values are stored; records are joined at run time, through permitted operations.\n"
    ));
    s
}

pub fn json_gorunum(uyeler: &[Uye]) -> Value {
    json!({ "eslemeler": eslemeler(uyeler).into_iter().map(|e| e.json).collect::<Vec<_>>(), "oneriler": oneriler(uyeler) })
}
