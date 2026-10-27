//! Flask adaptörü: Python modelinden (`python.rs`) NestJS, FastAPI ve Django adaptörleriyle aynı kapı yapısını (`Kapi`)
//! üretir.
//!
//! Kesinlik ilkesi diğer adaptörlerle aynıdır: anlam adlardan değil tanımlardan çıkar.
//! - **Rotalar:** `@app.route(...)` / `@bp.route(...)`, `@bp.get(...)` gibi kısayollar, `add_url_rule(...)` (fonksiyon
//!   ya da `MethodView.as_view(...)`), Flask-RESTful `api.add_resource(...)` ve `@api.resource(...)`, Flask-Smorest
//!   `@blp.route(...)` sınıfları, Flask-RESTX `@ns.route(...)`. Tam yol uygulamadan Blueprint'e kadar
//!   `register_blueprint` zinciri izlenerek kurulur (kayıttaki `url_prefix` Blueprint'inkini ezer; iç içe Blueprint'te
//!   önekler birleşir); kayıt yardımcı fonksiyonda (`def kur(app): app.register_blueprint(bp)`) ya da fabrikada
//!   (`create_app`) olabilir. `if` altındaki kayıt koşullu kapıdır.
//! - **Giriş:** Flask-Login `@login_required`, Flask-JWT-Extended `@jwt_required()` (`optional=True` isteğe bağlı),
//!   Flask-HTTPAuth `@auth.login_required` (rol: `role=`), Flask-Security / Flask-User `@roles_required` gibi bilinen
//!   işaretler kesindir. Projedeki dekoratörün ve `before_request` kancasının gövdesinden okunan karar çıkarımdır; işaret
//!   yoksa uç açık sayılır ama bu da çıkarımdır.
//! - **Girdiler ve yanıt:** marshmallow şemaları (`@blp.arguments`, `@use_kwargs`, `Schema().load(request.json)`),
//!   Pydantic modelleri, Flask-RESTful `reqparse`, Flask-RESTX modelleri; işleyicinin okuduğu `request.json` /
//!   `request.form` / `request.args` anahtarları (tip ve zorunluluk bilinmez). Yanıt: `@blp.response`, `@marshal_with`.
//! - **Veri:** Flask-SQLAlchemy / SQLAlchemy modelleri (`db.Model`, `declarative_base`, `__tablename__`), sorgular ve
//!   oturum yazmaları (Yorum), `ondelete="CASCADE"` ve `cascade="all, delete"` kaskadları.
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::adaptor::{Alan, Girdi, Kapi, Sema, VeriBag, Yanit};
use crate::django::{metindeki_roller, serbest_alan, Giris, Karar};
use crate::fastapi::{metin_sabiti, pascal, snake_camel, Baglam, Yer};
use crate::guvenli;
use crate::py_ortak::{cagri_ayir, donusturuculu_yol, liste_ogeleri, metin_listesi, parcalar, yol_ekle, yontem_dna, MetinCagri, Zincir};
use crate::python::{Cagri, Dosya, Fonksiyon, Sinif};

pub const SURUM: &str = "0.1.0";

const HTTP: [&str; 5] = ["get", "post", "put", "patch", "delete"];

/// Bir örnek: (dosya, ad); fabrikadaki yerel örnek "fonksiyon().ad"
type Dugum = (usize, String);

/// Kökten her örneğe: (tam önek, koşul, koşullu, kökten örneğe zincir)
type Erisim = HashMap<Dugum, (String, Option<String>, bool, Vec<Dugum>)>;
/// Bağlama çağrısı: (çağrının modülü, fonksiyonu, çağrı, yardımcı fonksiyonun parametre bağları)
type Baglama<'a> = (usize, Option<&'a Fonksiyon>, &'a Cagri, HashMap<String, Dugum>);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tur {
    Uygulama,
    Blueprint,
    /// Flask-RESTful / Flask-RESTX / Flask-Smorest Api
    Api,
    /// Flask-RESTX Namespace
    Ad,
}

struct Ornek {
    tur: Tur,
    /// Kurulumdaki önek (`url_prefix=`, `prefix=`, Namespace `path=`)
    onek: String,
    satir: usize,
}

/// Bağlama kenarı: üst -> alt (önek ezilmesi, satır, koşul, koşullu)
struct Kenar {
    ust: Dugum,
    alt: Dugum,
    onek: Option<String>,
    di: usize,
    satir: usize,
    kosul: Option<String>,
    kosullu: bool,
}

/// Rota kaydı
struct Kayit<'a> {
    alici: Dugum,
    kural: String,
    yontemler: Option<Vec<String>>,
    hedef: Hedef<'a>,
    kosullu: bool,
}

enum Hedef<'a> {
    Fonksiyon(usize, &'a Fonksiyon),
    Sinif(usize, &'a Sinif),
}

struct Flask<'a> {
    b: Baglam<'a>,
    kok: &'a Path,
    ornekler: HashMap<Dugum, Ornek>,
    kenarlar: Vec<Kenar>,
    /// Model sınıfları: ad -> (dosya, sınıf)
    modeller: BTreeMap<String, (usize, &'a Sinif)>,
}

impl<'a> Flask<'a> {
    fn dosyalar(&self) -> &'a [Dosya] {
        self.b.dosyalar
    }

    fn goreli(&self, di: usize) -> String {
        guvenli::goreli(self.kok, &self.dosyalar()[di].yol)
    }

    /// Çağrının kurduğu örneğin türü (projedeki alt sınıflar dahil)
    fn ornek_turu(&self, di: usize, c: &Cagri) -> Option<Tur> {
        let ad = self.b.taban_dis_ad(di, &c.ad, 0).unwrap_or_else(|| c.ad.rsplit('.').next().unwrap_or(&c.ad).to_string());
        let modul = match self.b.ifade(di, c.ad.split('(').next().unwrap_or(&c.ad)) {
            Some(Yer::Dis(m, _)) => m,
            _ => String::new(),
        };
        match ad.as_str() {
            "Flask" | "APIFlask" => Some(Tur::Uygulama),
            "Blueprint" | "APIBlueprint" => Some(Tur::Blueprint),
            "Api" if modul.is_empty() || modul.starts_with("flask") => Some(Tur::Api),
            "Namespace" if modul.is_empty() || modul.starts_with("flask") => Some(Tur::Ad),
            _ => None,
        }
    }

    fn ornekleri_bul(&mut self) {
        for (di, d) in self.dosyalar().iter().enumerate() {
            let mut adaylar: Vec<(String, &Cagri)> = d.atama_cagrilari.iter().map(|(a, c)| (a.clone(), c)).collect();
            for f in &d.fonksiyonlar {
                adaylar.extend(f.ic_atama_cagrilari.iter().map(|(a, c)| (format!("{}().{a}", f.ad), c)));
            }
            for (ad, c) in adaylar {
                let Some(tur) = self.ornek_turu(di, c) else { continue };
                let anahtar = |k: &str| c.anahtarli.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
                let onek = match tur {
                    Tur::Blueprint => anahtar("url_prefix"),
                    Tur::Api => anahtar("prefix"),
                    Tur::Ad => anahtar("path").or_else(|| c.argumanlar.first().and_then(|a| metin_sabiti(a)).map(|n| format!("'/{n}'"))),
                    Tur::Uygulama => None,
                };
                let onek = onek.and_then(|o| self.b.deger(di, &o, 0)).unwrap_or_default();
                self.ornekler.insert((di, ad.clone()), Ornek { tur, onek, satir: c.satir });
                // Api(app) / Api(bp): Api'nin bağlandığı örnek
                if matches!(tur, Tur::Api) {
                    if let Some(hedef) = c.argumanlar.first().or_else(|| c.anahtarli.iter().find(|(k, _)| k == "app").map(|(_, v)| v)) {
                        let f = ad.split_once("().").and_then(|(f, _)| d.fonksiyonlar.iter().find(|g| g.ad == f));
                        if let Some(u) = self.alici_bul(di, f, hedef) {
                            self.kenarlar.push(Kenar { ust: u, alt: (di, ad.clone()), onek: None, di, satir: c.satir, kosul: c.kosul.clone(), kosullu: c.kosullu });
                        }
                    }
                }
            }
        }
    }

    /// Alıcı: fonksiyonun yerel örneği ("f().app") ya da modül düzeyindeki ad
    fn alici_bul(&self, di: usize, f: Option<&Fonksiyon>, alici: &str) -> Option<Dugum> {
        let alici = alici.trim();
        if let Some(f) = f {
            let mut kapsam = Some(f);
            while let Some(g) = kapsam {
                if g.ic_atama_cagrilari.contains_key(alici) {
                    return Some((di, format!("{}().{alici}", g.ad)));
                }
                kapsam = g.kapsam.as_deref().and_then(|k| self.dosyalar()[di].fonksiyonlar.iter().find(|x| x.ad == k));
            }
        }
        match self.b.ifade(di, alici)? {
            Yer::Ad(adi, aad) => Some((adi, aad)),
            _ => None,
        }
    }

    /// Bağlama çağrıları (`register_blueprint`, `add_namespace`, `init_app`) ve yardımcı fonksiyona argüman olarak verilen
    /// örnekler (bir kat)
    fn kenarlari_bul(&mut self) {
        // (çağrının modülü, fonksiyonu, çağrı, parametre bağları)
        let mut cagrilar: Vec<Baglama<'a>> = vec![];
        for (di, d) in self.dosyalar().iter().enumerate() {
            let mut hepsi: Vec<(Option<&Fonksiyon>, &Cagri)> = d.cagrilar.iter().map(|c| (None, c)).collect();
            for f in &d.fonksiyonlar {
                hepsi.extend(f.ic_cagrilar.iter().map(|c| (Some(f), c)));
            }
            for (f, c) in hepsi {
                cagrilar.push((di, f, c, HashMap::new()));
                // Yardımcı: register_blueprints(app)
                let Some((gdi, g)) = self.b.ifade(di, &c.ad).and_then(|y| self.b.fonksiyon(&y)) else { continue };
                let mut baglar = HashMap::new();
                for (i, arg) in c.argumanlar.iter().enumerate() {
                    let Some(dugum) = self.alici_bul(di, f, arg).filter(|x| self.ornekler.contains_key(x)) else { continue };
                    if let Some(p) = g.parametreler.get(i) {
                        baglar.insert(p.ad.clone(), dugum);
                    }
                }
                if !baglar.is_empty() {
                    for gc in &g.ic_cagrilar {
                        cagrilar.push((gdi, Some(g), gc, baglar.clone()));
                    }
                }
            }
        }
        for (di, f, c, baglar) in cagrilar {
            let Some((alici, yontem)) = c.ad.rsplit_once('.') else { continue };
            if !matches!(yontem, "register_blueprint" | "add_namespace" | "init_app") {
                continue;
            }
            let bul = |x: &str| baglar.get(x.trim()).cloned().or_else(|| self.alici_bul(di, f, x));
            let Some(ust) = bul(alici) else { continue };
            let Some(arg) = c.argumanlar.first() else { continue };
            let Some(alt) = bul(arg) else { continue };
            let anahtar = |k: &str| c.anahtarli.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
            let (ust, alt, onek) = match yontem {
                // api.init_app(app): Api uygulamaya bağlanır
                "init_app" => {
                    if !matches!(self.ornekler.get(&ust).map(|o| o.tur), Some(Tur::Api)) {
                        continue;
                    }
                    (alt, ust, None)
                }
                "add_namespace" => (ust, alt, anahtar("path")),
                _ => (ust, alt, anahtar("url_prefix")),
            };
            if !self.ornekler.contains_key(&ust) || !self.ornekler.contains_key(&alt) {
                continue;
            }
            let onek = onek.map(|o| self.b.deger(di, &o, 0).unwrap_or_default());
            self.kenarlar.push(Kenar { ust, alt, onek, di, satir: c.satir, kosul: c.kosul.clone(), kosullu: c.kosullu || f.is_some_and(|f| f.kosullu_tanim) });
        }
    }

    /// Kökten (uygulama) her örneğe: (tam önek, koşul, koşullu, before_request kancaları zinciri)
    fn erisim(&self, kok: &Dugum) -> Erisim {
        let mut sonuc = HashMap::new();
        sonuc.insert(kok.clone(), (String::new(), None, false, vec![kok.clone()]));
        let mut kuyruk = vec![kok.clone()];
        let mut adim = 0;
        while let Some(u) = kuyruk.pop() {
            adim += 1;
            if adim > 2000 {
                break;
            }
            let (onek, kosul, kosullu, zincir) = sonuc[&u].clone();
            for k in self.kenarlar.iter().filter(|k| k.ust == u) {
                if sonuc.contains_key(&k.alt) {
                    continue;
                }
                let kendi = self.ornekler.get(&k.alt).map(|o| o.onek.clone()).unwrap_or_default();
                // register_blueprint(url_prefix=...) Blueprint'in kendi önekini ezer; iç içe Blueprint'te önekler birleşir
                let parca = k.onek.clone().unwrap_or(kendi);
                let tam = if parca.is_empty() { onek.clone() } else { yol_ekle(&onek, &parca) };
                let mut z = zincir.clone();
                z.push(k.alt.clone());
                sonuc.insert(k.alt.clone(), (tam, k.kosul.clone().or(kosul.clone()), kosullu || k.kosullu, z));
                kuyruk.push(k.alt.clone());
            }
        }
        sonuc
    }

    fn kayitlari_bul(&self) -> Vec<Kayit<'a>> {
        let mut v = vec![];
        for (di, d) in self.dosyalar().iter().enumerate() {
            // Fonksiyon dekoratörleri: @app.route, @bp.get, @api.resource yok (sınıf)
            for f in &d.fonksiyonlar {
                let kapsam = f.kapsam.as_deref().and_then(|k| d.fonksiyonlar.iter().find(|g| g.ad == k));
                for dek in &f.dekoratorler {
                    let Some((alici, yontem)) = dek.ad.rsplit_once('.') else { continue };
                    let yontemler = match yontem {
                        "route" => dek.anahtarli.iter().find(|(k, _)| k == "methods").and_then(|(_, v)| metin_listesi(v)).map(|l| l.iter().map(|x| x.to_uppercase()).collect()),
                        y if HTTP.contains(&y) => Some(vec![y.to_uppercase()]),
                        _ => continue,
                    };
                    let Some(u) = self.alici_bul(di, kapsam, alici).filter(|u| self.ornekler.contains_key(u)) else { continue };
                    let Some(kural) = dek.argumanlar.first().or_else(|| dek.anahtarli.iter().find(|(k, _)| k == "rule").map(|(_, v)| v)).and_then(|k| self.b.deger(di, k, 0))
                    else {
                        continue;
                    };
                    v.push(Kayit { alici: u, kural, yontemler: Some(yontemler.unwrap_or_else(|| vec!["GET".into()])), hedef: Hedef::Fonksiyon(di, f), kosullu: f.kosullu_tanim });
                }
            }
            // Sınıf dekoratörleri: @api.resource("/x"), @ns.route("/x"), @blp.route("/x")
            for s in &d.siniflar {
                for dek in &s.dekoratorler {
                    let Some((alici, yontem)) = dek.ad.rsplit_once('.') else { continue };
                    if !matches!(yontem, "route" | "resource") {
                        continue;
                    }
                    let Some(u) = self.alici_bul(di, None, alici).filter(|u| self.ornekler.contains_key(u)) else { continue };
                    for k in dek.argumanlar.iter().filter_map(|k| self.b.deger(di, k, 0)) {
                        v.push(Kayit { alici: u.clone(), kural: k, yontemler: None, hedef: Hedef::Sinif(di, s), kosullu: false });
                    }
                }
            }
            // add_url_rule ve add_resource çağrıları (modül düzeyi ve fonksiyonlar)
            let mut hepsi: Vec<(Option<&Fonksiyon>, &Cagri)> = d.cagrilar.iter().map(|c| (None, c)).collect();
            for f in &d.fonksiyonlar {
                hepsi.extend(f.ic_cagrilar.iter().map(|c| (Some(f), c)));
            }
            for (f, c) in hepsi {
                let Some((alici, yontem)) = c.ad.rsplit_once('.') else { continue };
                let Some(u) = self.alici_bul(di, f, alici).filter(|u| self.ornekler.contains_key(u)) else { continue };
                let kosullu = c.kosullu || f.is_some_and(|f| f.kosullu_tanim);
                match yontem {
                    "add_url_rule" => {
                        let Some(kural) = c.argumanlar.first().or_else(|| c.anahtarli.iter().find(|(k, _)| k == "rule").map(|(_, v)| v)).and_then(|k| self.b.deger(di, k, 0))
                        else {
                            continue;
                        };
                        let Some(gorunum) = c.argumanlar.get(2).or_else(|| c.anahtarli.iter().find(|(k, _)| k == "view_func").map(|(_, v)| v)) else { continue };
                        let yontemler =
                            c.anahtarli.iter().find(|(k, _)| k == "methods").and_then(|(_, v)| metin_listesi(v)).map(|l| l.iter().map(|x| x.to_uppercase()).collect::<Vec<_>>());
                        let g = gorunum.trim();
                        if let Some(sinif) = g.split_once(".as_view(").map(|x| x.0) {
                            if let Some((sdi, s)) = self.b.ifade(di, sinif).and_then(|y| self.b.sinif(&y)) {
                                v.push(Kayit { alici: u, kural, yontemler, hedef: Hedef::Sinif(sdi, s), kosullu });
                            }
                        } else if let Some((fdi, fg)) = self.b.ifade(di, g).and_then(|y| self.b.fonksiyon(&y)) {
                            v.push(Kayit { alici: u, kural, yontemler: Some(yontemler.unwrap_or_else(|| vec!["GET".into()])), hedef: Hedef::Fonksiyon(fdi, fg), kosullu });
                        }
                    }
                    "add_resource" => {
                        let Some((sdi, s)) = c.argumanlar.first().and_then(|a| self.b.ifade(di, a)).and_then(|y| self.b.sinif(&y)) else { continue };
                        for k in c.argumanlar.iter().skip(1).filter_map(|k| self.b.deger(di, k, 0)) {
                            v.push(Kayit { alici: u.clone(), kural: k, yontemler: None, hedef: Hedef::Sinif(sdi, s), kosullu });
                        }
                    }
                    _ => {}
                }
            }
        }
        v
    }

    // ---- Modeller ----

    /// Taban ifadesi bir SQLAlchemy modeli tabanı mı: `db.Model` (db = SQLAlchemy()), `declarative_base()` sonucu,
    /// `DeclarativeBase` alt sınıfı; takma adlar (`Model = db.Model`) izlenir
    fn model_tabani_mi(&self, di: usize, taban: &str, derinlik: usize) -> bool {
        if derinlik > 6 {
            return false;
        }
        let t = taban.split('[').next().unwrap_or(taban).trim();
        if let Some(on) = t.strip_suffix(".Model") {
            if let Some(y @ Yer::Ad(..)) = self.b.ifade(di, on) {
                if let Some((_, Some(c))) = self.b.atama(&y) {
                    return c.ad.rsplit('.').next() == Some("SQLAlchemy");
                }
            }
            return on == "db";
        }
        match self.b.ifade(di, t) {
            Some(Yer::Dis(m, a)) => {
                (m.contains("sqlalchemy") && matches!(a.rsplit('.').next(), Some("Model" | "DeclarativeBase"))) || a == "Model" && m.contains("flask_sqlalchemy")
            }
            Some(y @ Yer::Ad(..)) => {
                if let Some((sdi, s)) = self.b.sinif(&y) {
                    return s.tabanlar.iter().any(|t2| self.model_tabani_mi(sdi, t2, derinlik + 1));
                }
                let Some((atama, c)) = self.b.atama(&y) else { return false };
                let Yer::Ad(adi, _) = y else { return false };
                if c.is_some_and(|c| c.ad.rsplit('.').next() == Some("declarative_base")) {
                    return true;
                }
                self.model_tabani_mi(adi, atama, derinlik + 1)
            }
            _ => false,
        }
    }

    fn modelleri_bul(&mut self) {
        for (di, d) in self.dosyalar().iter().enumerate() {
            for s in &d.siniflar {
                let soyut = s.alanlar.iter().any(|a| a.ad == "__abstract__" && a.varsayilan.as_deref().map(str::trim) == Some("True"));
                if soyut {
                    continue;
                }
                if crate::fastapi::tablo_modeli_mi(s) || s.tabanlar.iter().any(|t| self.model_tabani_mi(di, t, 0)) {
                    self.modeller.entry(s.ad.clone()).or_insert((di, s));
                }
            }
        }
    }

    /// Tablo adı: `__tablename__`, yoksa Flask-SQLAlchemy'nin varsayılanı (sınıf adının yılan biçimi)
    fn tablo_adi(s: &Sinif) -> (String, &'static str) {
        match s.alanlar.iter().find(|a| a.ad == "__tablename__").and_then(|a| a.varsayilan.as_deref()).and_then(metin_sabiti) {
            Some(t) => (t, "__tablename__"),
            None => (yilan(&s.ad), "varsayılan (sınıf adının yılan biçimi)"),
        }
    }

    /// Sütun alanları (karışımlar ve tabanlar dahil): (ad, ifade, satır, tip bildirimi)
    fn sutunlar(&self, di: usize, s: &'a Sinif) -> Vec<(String, String, usize, Option<String>)> {
        let z = Zincir::kur(&self.b, di, s);
        let mut v: Vec<(String, String, usize, Option<String>)> = vec![];
        for (d2, s2) in z.siniflar.iter().rev() {
            for a in &s2.alanlar {
                let deger = a.varsayilan.clone().unwrap_or_default();
                let c = cagri_ayir(&deger);
                let ad = c.as_ref().map(|c| c.kisa_ad().to_string()).unwrap_or_default();
                let sutun = matches!(ad.as_str(), "Column" | "mapped_column")
                    || c.as_ref().is_some_and(|c| {
                        self.b.ifade(*d2, &c.ad).and_then(|y| self.b.fonksiyon(&y)).is_some_and(|(_, f)| f.donus.as_deref().is_some_and(|r| r.contains("Column(")))
                    })
                    || a.tip.as_deref().is_some_and(|t| t.starts_with("Mapped[")) && ad != "relationship";
                if sutun {
                    v.retain(|x| x.0 != a.ad);
                    v.push((a.ad.clone(), deger, a.satir, a.tip.clone()));
                }
            }
        }
        v
    }

    fn veri_modelleri(&self) -> Value {
        let mut v = vec![];
        for (ad, (di, s)) in &self.modeller {
            let (tablo, kaynak) = Self::tablo_adi(s);
            let alanlar: Vec<Value> = self
                .sutunlar(*di, s)
                .into_iter()
                .map(|(a, deger, satir, tip)| {
                    let c = cagri_ayir(&deger);
                    let sutun = c.as_ref().and_then(|c| c.arg(0)).and_then(metin_sabiti).filter(|x| !x.contains('.')).unwrap_or_else(|| a.clone());
                    let tur = c
                        .as_ref()
                        .and_then(|c| c.arglar.iter().map(|(_, x)| x.clone()).find(|x| !x.starts_with(['\'', '"'])))
                        .map(|x| x.split('(').next().unwrap_or(&x).rsplit('.').next().unwrap_or("").to_string());
                    json!({ "ad": a, "tip": tur.or(tip), "satir": satir, "sutun": sutun })
                })
                .collect();
            v.push(json!({ "ad": ad, "tablo": tablo, "tablo_kaynagi": kaynak, "dosya": self.goreli(*di), "satir": s.satir, "alanlar": alanlar }));
        }
        json!(v)
    }

    fn kaskadlar(&self) -> Vec<Value> {
        let tablolar: BTreeMap<String, String> = self.modeller.iter().map(|(ad, (_, s))| (Self::tablo_adi(s).0.to_lowercase(), ad.clone())).collect();
        let mut v = vec![];
        for (ad, (di, s)) in &self.modeller {
            let z = Zincir::kur(&self.b, *di, s);
            for (d2, s2) in &z.siniflar {
                for a in &s2.alanlar {
                    // Modül önekleri atılır (`db.Column(db.Integer, db.ForeignKey("x.id"))` -> `Column(Integer, ForeignKey("x.id"))`)
                    let deger = a.varsayilan.clone().unwrap_or_default();
                    let duz = onek_sil(&deger);
                    if let Some((e, c)) = crate::dna::python_kaskadi(ad, a.tip.as_deref(), &duz, &tablolar) {
                        // Aynı ilişki hem yabancı anahtarda hem relationship'te tanımlı olabilir: ilk kanıt yeter
                        if !v.iter().any(|x: &Value| x[0] == e.as_str() && x[1] == c.as_str()) {
                            v.push(json!([e, c, format!("{}:{}", self.goreli(*d2), a.satir)]));
                        }
                    }
                }
            }
        }
        v
    }

    /// SQLAlchemy kullanımı [Yorum]: `M.query...` ve `select(M)` / `.query(M)` / `.get(M, ...)` / `get_or_404(M` okur;
    /// `M.query...delete()/update()`, oturuma eklenen / silinen ve `.save()` / `.delete()` / `.update()` çağrılan
    /// (CRUD karışımı) nesneler yazar; `M.create(...)` yazar; ham SQL listeyi eksik yapar. Proje çağrıları 3 kat izlenir
    fn orm(&self, z: Option<&Zincir>, baslangic: Vec<(usize, &'a Fonksiyon)>, bag: &mut VeriBag) {
        static D: std::sync::OnceLock<[regex::Regex; 8]> = std::sync::OnceLock::new();
        let [sorgu, okuma, degisken, nesne, oturum, yaratma, ham, cagri] = D.get_or_init(|| {
            [
                regex::Regex::new(r"\b([A-Z]\w*)\.query\b([^\n]*)").unwrap(),
                regex::Regex::new(r"(?:\bselect|\.select_from|\.query|\.get|get_or_404|first_or_404|one_or_404)\(\s*([A-Z]\w*)").unwrap(),
                regex::Regex::new(r"\b(\w+)\s*=\s*(?:[\w.]*(?:get|get_or_404|one_or_404)\(\s*([A-Z]\w*)|([A-Z]\w*)\.(?:query|get_by_id|get_or_404|create)\b|([A-Z]\w*)\()")
                    .unwrap(),
                regex::Regex::new(r"\b(\w+)\.(?:save|delete|update)\(").unwrap(),
                regex::Regex::new(r"\.(?:add|delete|merge)\(\s*(\w+)\s*\)").unwrap(),
                regex::Regex::new(r"\b([A-Z]\w*)\.create\(").unwrap(),
                regex::Regex::new(r#"\btext\(\s*[fbru]*["']|\.execute\(\s*[fbru]*["']"#).unwrap(),
                regex::Regex::new(r"(?:^|[^\w.])((?:self\.)?[A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)\(").unwrap(),
            ]
        });
        let model = |m: &str| self.modeller.contains_key(m).then(|| m.to_string());
        let mut yigin: Vec<(usize, &Fonksiyon, usize)> = baslangic.into_iter().map(|(d, f)| (d, f, 0)).collect();
        let mut gorulen = HashSet::new();
        while let Some((di, f, derinlik)) = yigin.pop() {
            if !gorulen.insert((di, f.ad.clone(), f.satir)) || gorulen.len() > 64 {
                continue;
            }
            let t = &f.govde_metni;
            for c in sorgu.captures_iter(t) {
                let Some(m) = model(&c[1]) else { continue };
                bag.okur.push(m.clone());
                if [".delete(", ".update("].iter().any(|y| c[2].contains(y)) {
                    bag.yazar.push(m);
                }
            }
            bag.okur.extend(okuma.captures_iter(t).filter_map(|c| model(&c[1])));
            bag.yazar.extend(yaratma.captures_iter(t).filter_map(|c| model(&c[1])));
            let mut degiskenler: HashMap<String, String> = HashMap::new();
            for c in degisken.captures_iter(t) {
                if let Some(m) = c.get(2).or(c.get(3)).or(c.get(4)).and_then(|x| model(x.as_str())) {
                    degiskenler.insert(c[1].to_string(), m);
                }
            }
            for c in nesne.captures_iter(t).chain(oturum.captures_iter(t)) {
                let v = c.get(1).map(|x| x.as_str()).unwrap_or("");
                if let Some(m) = degiskenler.get(v) {
                    bag.yazar.push(m.clone());
                } else if let Some(p) = f.parametreler.iter().find(|p| p.ad == v) {
                    if let Some(m) = p.tip.as_deref().and_then(|t| t.split(|c: char| !(c.is_alphanumeric() || c == '_')).find_map(model)) {
                        bag.yazar.push(m);
                    }
                }
            }
            if ham.is_match(t) {
                bag.eksik_olabilir = true;
            }
            if derinlik >= 3 {
                continue;
            }
            for c in cagri.captures_iter(t) {
                let ad = &c[1];
                if let Some(m) = ad.strip_prefix("self.").filter(|m| !m.contains('.')) {
                    if let Some((d2, g, _)) = z.and_then(|z| z.metot(m)) {
                        yigin.push((d2, g, derinlik + 1));
                    }
                    continue;
                }
                if let Some((d2, g)) = self.b.ifade(di, ad).and_then(|y| self.b.fonksiyon(&y)) {
                    yigin.push((d2, g, derinlik + 1));
                }
            }
        }
        bag.okur.sort();
        bag.okur.dedup();
        bag.yazar.sort();
        bag.yazar.dedup();
    }

    // ---- Giriş ----

    /// Dekoratörün giriş kararı; girişe etkisi bilinen biçimde yoksa None
    fn isaret(&self, di: usize, dek: &Cagri, derinlik: usize) -> Option<Karar> {
        let ad = dek.ad.as_str();
        let kisa = ad.rsplit('.').next().unwrap_or(ad);
        let anahtar = |k: &str| dek.anahtarli.iter().find(|(a, _)| a == k).map(|(_, v)| v.trim().to_string());
        let yer = self.b.ifade(di, ad);
        // Flask-HTTPAuth: auth = HTTPBasicAuth(); @auth.login_required(role="admin", optional=True)
        if let Some((nesne, "login_required")) = ad.rsplit_once('.') {
            let tur = self.b.ifade(di, nesne).and_then(|y| self.b.atama(&y).and_then(|(_, c)| c.map(|c| c.ad.clone())));
            if tur.as_deref().is_some_and(|t| t.contains("Auth")) {
                if anahtar("optional").as_deref() == Some("True") {
                    let mut k = Karar::yeni(Giris::Acik, false);
                    k.bilinmeyen = true;
                    return Some(k);
                }
                let roller = anahtar("role").map(|r| metin_listesi(&r).unwrap_or_default()).unwrap_or_default();
                let kesin = !roller.is_empty();
                return Some(Karar::yeni(Giris::Gerekli, true).rollu(roller, kesin));
            }
        }
        let proje = matches!(yer, Some(Yer::Ad(..)));
        if !proje {
            let metin_rolleri = || dek.argumanlar.iter().filter_map(|a| metin_sabiti(a)).collect::<Vec<_>>();
            match kisa {
                "login_required"
                | "fresh_login_required"
                | "auth_required"
                | "http_auth_required"
                | "auth_token_required"
                | "fresh_jwt_required"
                | "jwt_refresh_token_required" => {
                    return Some(Karar::yeni(Giris::Gerekli, true));
                }
                "jwt_required" | "verify_jwt_in_request" => {
                    if anahtar("optional").as_deref() == Some("True") {
                        let mut k = Karar::yeni(Giris::Acik, false);
                        k.bilinmeyen = true;
                        return Some(k);
                    }
                    return Some(Karar::yeni(Giris::Gerekli, true));
                }
                "jwt_optional" => {
                    let mut k = Karar::yeni(Giris::Acik, false);
                    k.bilinmeyen = true;
                    return Some(k);
                }
                "roles_required" | "permissions_required" => {
                    let r = metin_rolleri();
                    let kesin = !r.is_empty();
                    return Some(Karar::yeni(Giris::Gerekli, true).rollu(r, kesin));
                }
                "roles_accepted" | "permissions_accepted" => return Some(Karar::yeni(Giris::Gerekli, true).rollu(metin_rolleri(), false)),
                "route" | "get" | "post" | "put" | "patch" | "delete" | "arguments" | "response" | "doc" | "expect" | "marshal_with" | "marshal_list_with" | "use_kwargs"
                | "use_args" | "marshal_with_field" | "validate" | "cross_origin" | "cached" | "memoize" | "limit" | "exempt" | "paginate" | "alt_response" | "input"
                | "output" | "wraps" | "classmethod" | "staticmethod" | "property" | "header" | "param" | "produces" | "errorhandler" | "etag" => return None,
                _ => {}
            }
            if matches!(yer, Some(Yer::Dis(..))) || yer.is_none() {
                let mut k = Karar::yeni(Giris::Acik, false);
                k.bilinmeyen = true;
                return Some(k);
            }
        }
        if derinlik > 3 {
            return None;
        }
        // Projedeki dekoratör: gövdesi, iç sarmalayıcıları ve sardığı işaretler
        let (fdi, f) = yer.as_ref().and_then(|y| self.b.fonksiyon(y))?;
        let icler: Vec<&Fonksiyon> = self.dosyalar()[fdi].fonksiyonlar.iter().filter(|g| self.kapsamda(fdi, g, &f.ad)).collect();
        for g in &icler {
            for d in &g.dekoratorler {
                if let Some(k) = self.isaret(fdi, d, derinlik + 1).filter(|k| k.giris == Giris::Gerekli) {
                    let mut k = k;
                    k.kesin = false;
                    return Some(k);
                }
            }
        }
        let metin: String = std::iter::once(f.govde_metni.as_str()).chain(icler.iter().map(|g| g.govde_metni.as_str())).collect::<Vec<_>>().join("\n");
        let mut roller = metindeki_roller(&metin);
        roller.extend(flask_rolleri(&metin));
        // Argümanlı dekoratör fabrikası: @permission_required(Permission.ADMIN)
        if metin.contains(".can(") || metin.contains("has_role(") || metin.contains("role") {
            roller.extend(dek.argumanlar.iter().map(|a| metin_sabiti(a).unwrap_or_else(|| a.rsplit('.').next().unwrap_or(a).to_string())));
        }
        roller.sort();
        roller.dedup();
        if giris_isteyen(&metin) || !roller.is_empty() || crate::py_ortak::kimlik_bilgisi_denetimi(&metin) {
            return Some(Karar::yeni(Giris::Gerekli, false).rollu(roller, false));
        }
        // Başka bir dekoratörü uygulayan fabrika: return permission_required(X)(f)
        if let Some(r) = f.donus.as_deref().and_then(cagri_ayir) {
            // Fabrikanın argümanları (rol) da taşınır: permission_required(Permission.ADMIN)(f)
            let ic = match cagri_ayir(&r.ad) {
                Some(ic) => Cagri { ad: ic.ad.clone(), argumanlar: ic.arglar.iter().map(|(_, x)| x.clone()).collect(), ..Default::default() },
                None => Cagri { ad: r.ad.clone(), ..Default::default() },
            };
            if let Some(k) = self.isaret(fdi, &ic, derinlik + 1) {
                return Some(k);
            }
        }
        let mut k = Karar::yeni(Giris::Acik, false);
        k.bilinmeyen = true;
        Some(k)
    }

    /// Projedeki dekoratör istekten API anahtarı / belirteç okuyup 401 / 403 dönüyor mu (guard adı etiketlenir)
    fn kimlik_dekoratoru(&self, di: usize, ad: &str) -> bool {
        self.b.ifade(di, ad).and_then(|y| self.b.fonksiyon(&y)).is_some_and(|(_, f)| crate::py_ortak::kimlik_bilgisi_denetimi(&f.govde_metni))
    }

    /// `g`, `ust` fonksiyonunun içinde (her derinlikte) tanımlı mı
    fn kapsamda(&self, di: usize, g: &Fonksiyon, ust: &str) -> bool {
        let mut k = g.kapsam.clone();
        let mut adim = 0;
        while let Some(a) = k {
            if a == ust {
                return true;
            }
            adim += 1;
            if adim > 8 {
                return false;
            }
            k = self.dosyalar()[di].fonksiyonlar.iter().find(|x| x.ad == a).and_then(|x| x.kapsam.clone());
        }
        false
    }

    /// Örneğin `before_request` kancaları (`@bp.before_request`, `@bp.before_app_request` uygulamanın bütününe): karar
    fn kancalar(&self, dugum: &Dugum, uygulama_geneli: bool) -> Vec<(Karar, String)> {
        let mut v = vec![];
        for (di, d) in self.dosyalar().iter().enumerate() {
            for f in &d.fonksiyonlar {
                for dek in &f.dekoratorler {
                    let Some((alici, yontem)) = dek.ad.rsplit_once('.') else { continue };
                    let uygun = match yontem {
                        "before_request" => !uygulama_geneli,
                        "before_app_request" => uygulama_geneli,
                        _ => false,
                    };
                    if !uygun || self.alici_bul(di, None, alici).as_ref() != Some(dugum) {
                        continue;
                    }
                    // Kancanın kendi işaretleri (`@auth.login_required`) ya da gövdesi
                    let isaretler: Vec<Karar> = f.dekoratorler.iter().filter(|x| x.ad != dek.ad).filter_map(|x| self.isaret(di, x, 0)).collect();
                    let k = match isaretler.into_iter().reduce(Karar::ve) {
                        Some(k) => k,
                        None if f.govde_metni.contains("abort(401") || f.govde_metni.contains("verify_jwt_in_request(") || f.govde_metni.contains("unauthorized(") => {
                            Karar::yeni(Giris::Gerekli, false)
                        }
                        None if giris_isteyen(&f.govde_metni) => {
                            let mut k = Karar::yeni(Giris::Acik, false);
                            k.bilinmeyen = true;
                            k
                        }
                        None => continue,
                    };
                    v.push((k, format!("{}.{yontem}", f.ad)));
                }
            }
        }
        v
    }

    // ---- Şemalar ----

    /// marshmallow şeması (`class X(Schema)`, `ma.SQLAlchemyAutoSchema`): girdi (load_only dahil, dump_only hariç) ya da
    /// yanıt (dump_only dahil, load_only hariç). Dönen: şema, notlar, girdiyi / çıktıyı dönüştüren kanca var mı
    fn marshmallow(&self, di: usize, ifade: &str, girdi: bool, derinlik: usize) -> Option<(Sema, Vec<String>, bool)> {
        let t = ifade.trim();
        let c = cagri_ayir(t);
        let ad = c.as_ref().map(|c| c.ad.clone()).unwrap_or_else(|| t.to_string());
        let coklu = c.as_ref().and_then(|c| c.anahtar("many")).map(str::trim) == Some("True");
        // Modül düzeyinde örnek: article_schema = ArticleSchema()
        let (sdi, s) = match self.b.ifade(di, &ad).and_then(|y| self.b.sinif(&y)) {
            Some(x) => x,
            None => {
                let y = self.b.ifade(di, &ad)?;
                let (atama, _) = self.b.atama(&y)?;
                let Yer::Ad(adi, _) = y else { return None };
                if atama.trim() == t || derinlik > 4 {
                    return None;
                }
                return self.marshmallow(adi, atama, girdi, derinlik + 1);
            }
        };
        let z = Zincir::kur(&self.b, sdi, s);
        if !z.dis_var(&["Schema", "SQLAlchemySchema", "SQLAlchemyAutoSchema", "ModelSchema", "TableSchema"]) {
            return None;
        }
        let mut notlar = vec![];
        let meta = z.ic_sinif("Meta");
        let meta_deger = |k: &str| meta.and_then(|(_, m)| m.alanlar.iter().find(|a| a.ad == k)).and_then(|a| a.varsayilan.clone());
        let yalniz: Option<Vec<String>> = meta_deger("fields").and_then(|v| metin_listesi(&v));
        let haric: Vec<String> = meta_deger("exclude").and_then(|v| metin_listesi(&v)).unwrap_or_default();
        let load_only: Vec<String> = meta_deger("load_only").and_then(|v| metin_listesi(&v)).unwrap_or_default();
        let dump_only: Vec<String> = meta_deger("dump_only").and_then(|v| metin_listesi(&v)).unwrap_or_default();
        let mut alanlar: BTreeMap<String, Alan> = BTreeMap::new();
        // `fields.Nested("self")` alanları
        let mut kendine: Vec<String> = vec![];
        // Otomatik şema: modelin sütunları
        if z.dis_var(&["SQLAlchemyAutoSchema", "ModelSchema", "TableSchema"]) {
            if let Some((mdi, ms)) = meta_deger("model").and_then(|m| self.b.ifade(meta.map(|x| x.0).unwrap_or(sdi), m.trim())).and_then(|y| self.b.sinif(&y)) {
                for (a, deger, satir, tip) in self.sutunlar(mdi, ms) {
                    let alan = sutun_alani(&deger, tip.as_deref(), satir);
                    let (alan, birincil) = alan;
                    if !girdi || !birincil {
                        alanlar.insert(a, alan);
                    }
                }
            } else {
                notlar.push(format!("{}: Meta.model çözülemedi; otomatik alanlar eksik", s.ad));
            }
        }
        for (d2, s2) in z.siniflar.iter().rev() {
            for a in &s2.alanlar {
                let Some(c) = a.varsayilan.as_deref().and_then(cagri_ayir) else { continue };
                if c.kisa_ad() == "Nested" && c.arg(0).and_then(metin_sabiti).as_deref() == Some("self") {
                    kendine.push(a.ad.clone());
                }
                let Some((alan, load, dump, api_adi)) = self.mm_alani(*d2, &c, a.satir, girdi, derinlik, &mut notlar, &a.ad) else { continue };
                alanlar.remove(&a.ad);
                let load = load || load_only.contains(&a.ad);
                let dump = dump || dump_only.contains(&a.ad);
                if (girdi && dump) || (!girdi && load) {
                    continue;
                }
                let mut alan = alan;
                if !girdi {
                    alan.zorunlu = true;
                }
                alanlar.insert(api_adi.unwrap_or_else(|| a.ad.clone()), alan);
            }
        }
        if let Some(y) = yalniz {
            alanlar.retain(|a, _| y.contains(a));
        }
        alanlar.retain(|a, _| !haric.contains(a));
        // Kancalar: pre_load girdiyi, post_dump çıktıyı dönüştürür; validates / validates_schema önizlemede çalışmaz
        let mut donusum = false;
        let mut zarf: Option<String> = None;
        for (_, s2) in &z.siniflar {
            for m in &s2.metotlar {
                for d in &m.dekoratorler {
                    match d.ad.rsplit('.').next().unwrap_or("") {
                        "pre_load" if girdi => {
                            // Zarf açan kanca: `data = data["user"]` ya da `return data["article"]`
                            match zarf_kancasi(&m.govde_metni) {
                                Some(z) => zarf = Some(z),
                                None => {
                                    donusum = true;
                                    notlar.push(format!("{}.{} (pre_load) girdiyi şemadan önce dönüştürüyor: gövdenin biçimi şemanınkinden farklı olabilir", s2.ad, m.ad));
                                }
                            }
                        }
                        "post_dump" if !girdi => donusum = true,
                        "validates" | "validates_schema" if girdi => notlar.push(format!("{}.{} doğrulaması önizlemede çalıştırılamaz", s2.ad, m.ad)),
                        _ => {}
                    }
                }
            }
        }
        if donusum && girdi {
            for a in alanlar.values_mut() {
                a.belirsiz = true;
            }
        }
        if let (Some(z), true) = (&zarf, girdi) {
            // Gövde zarflı: {"user": {...}}; kendine iç içe alan (`fields.Nested("self")`) belge hilesidir, alan değildir
            for a in &kendine {
                alanlar.remove(a);
            }
            let ic = std::mem::take(&mut alanlar);
            let satir = s.satir;
            alanlar.insert(
                z.clone(),
                Alan {
                    tip: Some("object".into()),
                    zorunlu: true,
                    belirsiz: false,
                    kurallar: vec!["ValidateNested()".into()],
                    satir,
                    kalitimdan: None,
                    degerler: vec![],
                    ic: Some(ic),
                },
            );
        }
        Some((
            Sema { dto: s.ad.clone(), dosya: Some(self.goreli(sdi)), bulunamadi: false, kalitim: s.tabanlar.first().cloned(), alanlar, adaylar: vec![], dizi: coklu },
            notlar,
            donusum,
        ))
    }

    /// marshmallow alanı: (alan, load_only, dump_only, API adı `data_key`)
    #[allow(clippy::too_many_arguments)]
    fn mm_alani(&self, di: usize, c: &MetinCagri, satir: usize, girdi: bool, derinlik: usize, notlar: &mut Vec<String>, ad: &str) -> Option<(Alan, bool, bool, Option<String>)> {
        let kisa = c.kisa_ad();
        let dogru = |k: &str| c.anahtar(k).map(str::trim) == Some("True");
        let mut kurallar: Vec<String> = vec![];
        let mut degerler: Vec<String> = vec![];
        let mut ic = None;
        let tip: Option<&str> = match kisa {
            "Str" | "String" => {
                kurallar.push("IsString()".into());
                Some("string")
            }
            "Email" => {
                kurallar.push("IsEmail()".into());
                Some("string")
            }
            "Url" | "URL" => {
                kurallar.push("IsUrl()".into());
                Some("string")
            }
            "UUID" => {
                kurallar.push("IsUUID()".into());
                Some("string")
            }
            "Int" | "Integer" => {
                kurallar.push("IsInt()".into());
                Some("int")
            }
            "Float" | "Decimal" | "Number" => {
                kurallar.push("IsNumber()".into());
                Some("number")
            }
            "Bool" | "Boolean" => {
                kurallar.push("IsBoolean()".into());
                Some("boolean")
            }
            "DateTime" | "Date" | "Time" | "AwareDateTime" | "NaiveDateTime" | "TimeDelta" => {
                kurallar.push("IsDateString()".into());
                Some("date")
            }
            "List" | "Tuple" => {
                kurallar.push("IsArray()".into());
                Some("array")
            }
            "Dict" | "Mapping" => {
                kurallar.push("IsObject()".into());
                Some("object")
            }
            "Nested" | "Pluck" => {
                let coklu = dogru("many");
                kurallar.push(if coklu { "IsArray()".into() } else { "ValidateNested()".into() });
                if let Some(h) = c.arg(0).filter(|h| metin_sabiti(h).is_none() && derinlik < 3) {
                    if let Some((s, n, _)) = self.marshmallow(di, h, girdi, derinlik + 1) {
                        ic = Some(s.alanlar);
                        notlar.extend(n);
                    }
                }
                Some(if coklu { "array" } else { "object" })
            }
            "Enum" => Some("enum"),
            "Method" | "Function" => {
                // Yalnızca serileştirme (deserialize verilmedikçe)
                if c.anahtar("deserialize").is_none() {
                    return Some((serbest_alan(satir, false), false, true, None));
                }
                None
            }
            "Raw" | "Field" => None,
            _ => {
                let son = c.ad.rsplit('.').nth(1).unwrap_or("");
                if !(son == "fields" || son == "ma" || kisa.ends_with("Field")) {
                    return None;
                }
                None
            }
        };
        // Doğrulayıcılar: validate=validate.Length(min=1, max=50) / Range / OneOf / Regexp / Email ya da liste
        if let Some(v) = c.anahtar("validate") {
            let liste: Vec<String> = liste_ogeleri(v).map(|l| l.into_iter().map(|(_, x)| x).collect()).unwrap_or_else(|| vec![v.to_string()]);
            for d in liste {
                let Some(dc) = cagri_ayir(&d) else {
                    if girdi {
                        notlar.push(format!("'{ad}' alanının doğrulayıcısı ({}) önizlemede çalıştırılamaz", crate::django::kisalt(&d)));
                    }
                    continue;
                };
                let sayi = |k: &str| dc.anahtar(k).map(str::trim).filter(|n| n.parse::<f64>().is_ok()).map(String::from);
                match dc.kisa_ad() {
                    "Length" => {
                        if let Some(n) = sayi("min") {
                            kurallar.push(format!("MinLength({n})"));
                        }
                        if let Some(n) = sayi("max") {
                            kurallar.push(format!("MaxLength({n})"));
                        }
                        if let Some(n) = sayi("equal") {
                            kurallar.push(format!("MinLength({n})"));
                            kurallar.push(format!("MaxLength({n})"));
                        }
                    }
                    "Range" => {
                        if let Some(n) = sayi("min") {
                            kurallar.push(format!("Min({n})"));
                        }
                        if let Some(n) = sayi("max") {
                            kurallar.push(format!("Max({n})"));
                        }
                    }
                    "OneOf" => {
                        if let Some(l) = dc.arg(0).and_then(metin_listesi) {
                            kurallar.push(format!("IsIn([{}])", l.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
                            degerler = l;
                        }
                    }
                    "Email" => kurallar.push("IsEmail()".into()),
                    "URL" => kurallar.push("IsUrl()".into()),
                    "Regexp" => {
                        if let Some(d) = dc.arg(0).and_then(metin_sabiti) {
                            kurallar.push(format!("Matches(/{d}/)"));
                        }
                    }
                    _ if girdi => notlar.push(format!("'{ad}' alanının doğrulayıcısı ({}) önizlemede çalıştırılamaz", dc.kisa_ad())),
                    _ => {}
                }
            }
        }
        if dogru("allow_none") {
            kurallar.push("nullable()".into());
        }
        let api_adi = c.anahtar("data_key").or(c.anahtar("load_from")).and_then(metin_sabiti);
        let tip = if !degerler.is_empty() { Some("enum") } else { tip };
        let alan = Alan {
            tip: tip.map(String::from),
            // marshmallow'da alanlar varsayılan olarak isteğe bağlıdır
            zorunlu: dogru("required"),
            belirsiz: tip.is_none(),
            kurallar,
            satir,
            kalitimdan: None,
            degerler,
            ic,
        };
        Some((alan, dogru("load_only"), dogru("dump_only"), api_adi))
    }

    /// Pydantic modeli (FastAPI adaptörünün alan çözücüsüyle)
    fn pydantic(&self, di: usize, ifade: &str) -> Option<Sema> {
        let ad = ifade.trim().split('(').next().unwrap_or(ifade).trim();
        let (sdi, s) = self.b.ifade(di, ad).and_then(|y| self.b.sinif(&y)).filter(|(d, s)| self.b.pydantic_mi(*d, s, 0))?;
        let mut alanlar = BTreeMap::new();
        self.b.alanlar(sdi, s, 0, &mut alanlar);
        Some(Sema { dto: s.ad.clone(), dosya: Some(self.goreli(sdi)), bulunamadi: false, kalitim: s.tabanlar.first().cloned(), alanlar, adaylar: vec![], dizi: false })
    }

    /// Flask-RESTX modeli: `api.model("Ad", {"x": fields.String(required=True)})`
    fn restx_modeli(&self, di: usize, ifade: &str, derinlik: usize) -> Option<Sema> {
        let t = ifade.trim();
        let (mdi, metin) = match cagri_ayir(t) {
            Some(c) if c.kisa_ad() == "model" || c.kisa_ad() == "Model" => (di, t.to_string()),
            _ => {
                let y = self.b.ifade(di, t)?;
                let (atama, _) = self.b.atama(&y)?;
                let Yer::Ad(adi, _) = y else { return None };
                (adi, atama.to_string())
            }
        };
        let c = cagri_ayir(&metin)?;
        if !matches!(c.kisa_ad(), "model" | "Model" | "inherit") {
            return None;
        }
        let ad = c.arg(0).and_then(metin_sabiti).unwrap_or_else(|| "model".into());
        let sozluk = c.arglar.iter().map(|(_, x)| x).find(|x| x.trim_start().starts_with('{'))?;
        let ic = sozluk.trim().strip_prefix('{')?.strip_suffix('}')?;
        let satir = self.dosyalar()[mdi].atama_satirlari.values().copied().min().unwrap_or(1);
        let mut alanlar = BTreeMap::new();
        for oge in parcalar(ic, ',') {
            let p = crate::py_ortak::bol(&oge, ':');
            let (Some((_, k)), Some((o, _))) = (p.first(), p.get(1)) else { continue };
            let Some(anahtar) = metin_sabiti(k) else { continue };
            let deger = oge[*o..].trim().to_string();
            let Some(fc) = cagri_ayir(&deger) else { continue };
            let dogru = |k: &str| fc.anahtar(k).map(str::trim) == Some("True");
            let mut kurallar = vec![];
            let mut ic_alan = None;
            let tip = match fc.kisa_ad() {
                "String" | "FormattedString" => Some("string"),
                "Integer" => Some("int"),
                "Float" | "Arbitrary" | "Fixed" => Some("number"),
                "Boolean" => Some("boolean"),
                "DateTime" | "Date" => Some("date"),
                "Url" => Some("string"),
                "List" => Some("array"),
                "Nested" => {
                    if derinlik < 3 {
                        ic_alan = fc.arg(0).and_then(|m| self.restx_modeli(mdi, m, derinlik + 1)).map(|s| s.alanlar);
                    }
                    Some("object")
                }
                "Raw" => None,
                _ => None,
            };
            let sayi = |k: &str| fc.anahtar(k).map(str::trim).filter(|n| n.parse::<f64>().is_ok()).map(String::from);
            for (k, kural) in [("min_length", "MinLength"), ("max_length", "MaxLength"), ("min", "Min"), ("max", "Max")] {
                if let Some(n) = sayi(k) {
                    kurallar.push(format!("{kural}({n})"));
                }
            }
            let mut degerler = vec![];
            if let Some(l) = fc.anahtar("enum").and_then(metin_listesi) {
                kurallar.push(format!("IsIn([{}])", l.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
                degerler = l;
            }
            if let Some(d) = fc.anahtar("pattern").and_then(metin_sabiti) {
                kurallar.push(format!("Matches(/{d}/)"));
            }
            alanlar.insert(
                anahtar,
                Alan { tip: tip.map(String::from), zorunlu: dogru("required"), belirsiz: tip.is_none(), kurallar, satir, kalitimdan: None, degerler, ic: ic_alan },
            );
        }
        Some(Sema { dto: ad, dosya: Some(self.goreli(mdi)), bulunamadi: false, kalitim: None, alanlar, adaylar: vec![], dizi: false })
    }

    /// Flask-RESTful reqparse: `parser.add_argument("ad", type=str, required=True, location="json")`. Dönen: (gövde, sorgu)
    fn reqparse(&self, di: usize, f: &Fonksiyon, z: Option<&Zincir>) -> (BTreeMap<String, Alan>, BTreeMap<String, Alan>) {
        let (mut govde, mut sorgu) = (BTreeMap::new(), BTreeMap::new());
        static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = D.get_or_init(|| regex::Regex::new(r"\b((?:self\.)?\w+)\.parse_args\(").unwrap());
        for c in re.captures_iter(&f.govde_metni) {
            let degisken = c[1].to_string();
            // add_argument çağrıları: modül düzeyi, işleyici gövdesi, sınıfın __init__'i
            let mut cagrilar: Vec<&Cagri> = vec![];
            cagrilar.extend(self.dosyalar()[di].cagrilar.iter().filter(|x| x.ad == format!("{degisken}.add_argument")));
            cagrilar.extend(f.ic_cagrilar.iter().filter(|x| x.ad == format!("{degisken}.add_argument")));
            if let Some(z) = z {
                for (_, s) in &z.siniflar {
                    for m in &s.metotlar {
                        cagrilar.extend(m.ic_cagrilar.iter().filter(|x| x.ad == format!("{degisken}.add_argument")));
                    }
                }
            }
            // Modül düzeyinde içe aktarılmış ayrıştırıcı
            if cagrilar.is_empty() {
                if let Some(Yer::Ad(pdi, pad)) = self.b.ifade(di, &degisken) {
                    cagrilar.extend(self.dosyalar()[pdi].cagrilar.iter().filter(|x| x.ad == format!("{pad}.add_argument")));
                }
            }
            for a in cagrilar {
                let Some(ad) = a.argumanlar.first().and_then(|x| metin_sabiti(x)) else { continue };
                let anahtar = |k: &str| a.anahtarli.iter().find(|(x, _)| x == k).map(|(_, v)| v.trim().to_string());
                let tip = anahtar("type").map(|t| match t.as_str() {
                    "int" => ("int", "IsInt()"),
                    "float" => ("number", "IsNumber()"),
                    "bool" | "inputs.boolean" => ("boolean", "IsBoolean()"),
                    _ => ("string", "IsString()"),
                });
                let mut kurallar: Vec<String> = tip.map(|t| vec![t.1.to_string()]).unwrap_or_default();
                let mut degerler = vec![];
                if let Some(l) = anahtar("choices").and_then(|c| metin_listesi(&c)) {
                    kurallar.push(format!("IsIn([{}])", l.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
                    degerler = l;
                }
                let alan = Alan {
                    tip: tip.map(|t| t.0.to_string()),
                    zorunlu: anahtar("required").as_deref() == Some("True"),
                    belirsiz: tip.is_none(),
                    kurallar,
                    satir: a.satir,
                    kalitimdan: None,
                    degerler,
                    ic: None,
                };
                let yer = anahtar("location").and_then(|l| metin_listesi(&l)).unwrap_or_default();
                if yer.iter().any(|l| l == "args" || l == "values") && !yer.iter().any(|l| l == "json") {
                    sorgu.insert(ad, alan);
                } else {
                    govde.insert(ad, alan);
                }
            }
        }
        (govde, sorgu)
    }
}

/// SQLAlchemy sütunundan alan: (alan, birincil anahtar mı)
fn sutun_alani(deger: &str, tip: Option<&str>, satir: usize) -> (Alan, bool) {
    let c = cagri_ayir(deger);
    let metin = c.as_ref().map(|c| c.arglar.iter().map(|(_, x)| x.clone()).collect::<Vec<_>>().join(",")).unwrap_or_default() + " " + tip.unwrap_or("");
    let (t, kural) = if metin.contains("Integer") || metin.contains("int]") {
        (Some("int"), Some("IsInt()"))
    } else if metin.contains("String") || metin.contains("Text") || metin.contains("str]") {
        (Some("string"), Some("IsString()"))
    } else if metin.contains("Boolean") || metin.contains("bool]") {
        (Some("boolean"), Some("IsBoolean()"))
    } else if metin.contains("Float") || metin.contains("Numeric") {
        (Some("number"), Some("IsNumber()"))
    } else if metin.contains("DateTime") || metin.contains("Date") {
        (Some("date"), Some("IsDateString()"))
    } else {
        (None, None)
    };
    let anahtar = |k: &str| c.as_ref().and_then(|c| c.anahtar(k)).map(str::trim);
    let birincil = anahtar("primary_key") == Some("True");
    let bos = anahtar("nullable") != Some("False") && !birincil;
    let zorunlu = !bos && anahtar("default").is_none() && anahtar("server_default").is_none();
    let mut kurallar: Vec<String> = kural.map(|k| vec![k.to_string()]).unwrap_or_default();
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let uz = D.get_or_init(|| regex::Regex::new(r"String\(\s*(?:length\s*=\s*)?(\d+)").unwrap());
    if let Some(n) = uz.captures(&metin) {
        kurallar.push(format!("MaxLength({})", &n[1]));
    }
    (Alan { tip: t.map(String::from), zorunlu, belirsiz: t.is_none(), kurallar, satir, kalitimdan: None, degerler: vec![], ic: None }, birincil)
}

/// Gövdede şema kullanımı: marshmallow `X().load(...)` / `x.validate(...)` ve Pydantic `M(**request.json)` /
/// `M.model_validate(request.json)`
fn govde_sema_desenleri() -> &'static [regex::Regex; 2] {
    static D: std::sync::OnceLock<[regex::Regex; 2]> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        [
            regex::Regex::new(r"\b([A-Za-z_][\w.]*(?:\([^()]*\))?)\.(?:load|validate)\(").unwrap(),
            regex::Regex::new(r"\b([A-Z]\w*)(?:\.model_validate|\.parse_obj|\.model_validate_json)?\(\s*\*\*\s*request\.|\b([A-Z]\w*)\.(?:model_validate|parse_obj)\(\s*request\.")
                .unwrap(),
        ]
    })
}

/// pre_load kancasının açtığı zarf: `data = data["user"]` ya da `return data["article"]`
fn zarf_kancasi(t: &str) -> Option<String> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r#"(?:\bdata\s*=|\breturn)\s*data\[\s*["']([\w-]+)["']\s*\]"#).unwrap());
    re.captures(t).map(|c| c[1].to_string())
}

/// Metin sabitlerinin dışındaki modül öneklerini siler (`db.Integer` -> `Integer`); tırnak içindeki `tablo.sutun` korunur
fn onek_sil(s: &str) -> String {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let onek = D.get_or_init(|| regex::Regex::new(r"\b[a-z_]\w*\.([A-Za-z_])").unwrap());
    let mut out = String::new();
    for (i, p) in s.split(['"', '\'']).enumerate() {
        if i > 0 {
            out.push('"');
        }
        if i % 2 == 0 {
            out.push_str(&onek.replace_all(p, "$1"));
        } else {
            out.push_str(p);
        }
    }
    out
}

/// CamelCase -> camel_case (Flask-SQLAlchemy'nin varsayılan tablo adı)
fn yilan(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Gövde giriş istiyor mu: `current_user.is_authenticated` / `abort(401)` / `verify_jwt_in_request` ve ret
fn giris_isteyen(t: &str) -> bool {
    t.contains("verify_jwt_in_request(")
        || t.contains("abort(401")
        || t.contains("login_required(")
        || ((t.contains("is_authenticated") || t.contains("is_anonymous") || t.contains("current_user"))
            && ["abort(", "redirect(", "unauthorized", "forbidden", "401", "403", "raise"].iter().any(|x| t.contains(x)))
}

/// Flask'ta yaygın rol denetimleri: `current_user.has_role("x")`, `.is_administrator()`
fn flask_rolleri(t: &str) -> Vec<String> {
    static D: std::sync::OnceLock<[regex::Regex; 2]> = std::sync::OnceLock::new();
    let [rol, yonetici] = D.get_or_init(|| [regex::Regex::new(r#"has_role\(\s*["']([\w .-]+)["']"#).unwrap(), regex::Regex::new(r"\bis_(administrator|admin)\(").unwrap()]);
    let mut v: Vec<String> = rol.captures_iter(t).map(|c| c[1].to_string()).collect();
    v.extend(yonetici.captures_iter(t).map(|c| c[1].to_string()));
    v
}

/// Fonksiyon görünümünün kontrolcü adı: modül views / routes / api / __init__ ise bir üst paketin adı
fn modul_kontrolcusu(d: &Dosya) -> String {
    let p: Vec<&str> = d.modul.split('.').collect();
    let son = p.last().copied().unwrap_or("");
    let ad = if matches!(son, "views" | "routes" | "api" | "resources" | "endpoints" | "controllers") && p.len() > 1 { p[p.len() - 2] } else { son };
    pascal(ad)
}

fn kontrolcu_adi(s: &str) -> String {
    for ek in ["Resource", "View", "API", "Api"] {
        if let Some(k) = s.strip_suffix(ek).filter(|k| !k.is_empty()) {
            return k.to_string();
        }
    }
    s.to_string()
}

/// İşleyicinin okuduğu istek anahtarları: gövde `request.json["x"]`, `request.get_json()["x"]`, `data = request.get_json();
/// data["x"]` / `data.get("x")`, `request.form[...]`; sorgu `request.args.get("x")`
fn istek_alanlari(t: &str) -> (Vec<(String, bool)>, Vec<String>) {
    static D: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let [dogrudan, degisken, sorgu] = D.get_or_init(|| {
        [
            regex::Regex::new(r#"request\.(?:json|get_json\([^)]*\)|form|values)(\.get\(\s*|\[\s*)["']([\w-]+)["']"#).unwrap(),
            regex::Regex::new(r"\b(\w+)\s*=\s*request\.(?:json|get_json\([^)]*\)|form)\s*(?:or\s*\{\})?\s*$").unwrap(),
            regex::Regex::new(r#"request\.args(?:\.get\(\s*|\.getlist\(\s*|\[\s*)["']([\w-]+)["']"#).unwrap(),
        ]
    });
    let mut g: Vec<(String, bool)> = vec![];
    let mut ekle = |ad: &str, koseli: bool| {
        if !g.iter().any(|x| x.0 == ad) {
            g.push((ad.to_string(), koseli));
        }
    };
    for c in dogrudan.captures_iter(t) {
        ekle(&c[2], c[1].starts_with('['));
    }
    for satir in t.lines() {
        if let Some(c) = degisken.captures(satir.trim()) {
            let v = regex::escape(&c[1]);
            if let Ok(re) = regex::Regex::new(&format!(r#"\b{v}(\.get\(\s*|\[\s*)["']([\w-]+)["']"#)) {
                for c2 in re.captures_iter(t) {
                    ekle(&c2[2], c2[1].starts_with('['));
                }
            }
        }
    }
    let mut s: Vec<String> = vec![];
    for c in sorgu.captures_iter(t) {
        if !s.contains(&c[1].to_string()) {
            s.push(c[1].to_string());
        }
    }
    (g, s)
}

/// Model tanımlayan dosyalar (rapor tablo bağlarını bunlara göre sayar)
pub(crate) fn model_dosyalari(dosyalar: &[Dosya]) -> Vec<&Dosya> {
    let mut f = Flask { b: Baglam::yeni(dosyalar), kok: Path::new(""), ornekler: HashMap::new(), kenarlar: vec![], modeller: BTreeMap::new() };
    f.modelleri_bul();
    let mut v: Vec<usize> = f.modeller.values().map(|(di, _)| *di).collect();
    v.sort();
    v.dedup();
    v.into_iter().map(|i| &dosyalar[i]).collect()
}

/// Bir kapının girdi kaynakları: dekoratörler (`@blp.arguments`, `@use_kwargs`, `@ns.expect`, flask-pydantic
/// `@validate`) ve gövde (`Schema().load(...)`, Pydantic, reqparse, istek anahtarları)
struct Girdiler {
    govde: Vec<Girdi>,
    sorgu: BTreeMap<String, Alan>,
    yanit: Option<Yanit>,
    notlar: Vec<String>,
}

pub fn kapilar(kok: &Path, dosyalar: &[Dosya]) -> (Vec<Kapi>, Value) {
    let mut fl = Flask { b: Baglam::yeni(dosyalar), kok, ornekler: HashMap::new(), kenarlar: vec![], modeller: BTreeMap::new() };
    fl.ornekleri_bul();
    fl.kenarlari_bul();
    fl.modelleri_bul();
    let fl = fl;
    let b = &fl.b;
    // Ana uygulama: en çok örneğe ulaşan Flask örneği
    let uygulamalar: Vec<Dugum> = {
        let mut u: Vec<Dugum> = fl.ornekler.iter().filter(|(_, o)| o.tur == Tur::Uygulama).map(|(d, _)| d.clone()).collect();
        u.sort();
        u
    };
    let mut en_iyi: Option<(Dugum, Erisim)> = None;
    for u in &uygulamalar {
        let e = fl.erisim(u);
        if en_iyi.as_ref().is_none_or(|(_, x)| e.len() > x.len()) {
            en_iyi = Some((u.clone(), e));
        }
    }
    let (ana_kok, erisim) = en_iyi.map(|(a, e)| (Some(a), e)).unwrap_or((None, HashMap::new()));
    // Uygulama geneli kancalar (before_app_request bir Blueprint'te tanımlansa da bütün uygulamaya uygulanır)
    let mut genel_kancalar: Vec<(Karar, String)> = vec![];
    for d in erisim.keys() {
        genel_kancalar.extend(fl.kancalar(d, true));
    }
    if let Some(a) = &ana_kok {
        genel_kancalar.extend(fl.kancalar(a, false));
    }
    let mut kapilar: Vec<Kapi> = vec![];
    let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
    let mut kosullu_kapilar: Vec<String> = vec![];
    let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
    let mut gorulen: HashSet<String> = HashSet::new();
    let dugum_kimligi = |d: &Dugum| format!("{}:{}", guvenli::goreli(kok, &dosyalar[d.0].yol), d.1);
    for k in fl.kayitlari_bul() {
        // Önek: uygulamadan erişim, yoksa (bağlanmamış Blueprint) kendi öneki, ikincil
        // Koşul metni grafikteki kayıt kenarındadır; burada yalnızca koşullu olup olmadığı gerekir
        let (onek, kosullu_erisim, zincir, ana) = match erisim.get(&k.alici) {
            Some((o, _, kk, z)) => (o.clone(), *kk, z.clone(), true),
            None => (fl.ornekler.get(&k.alici).map(|o| o.onek.clone()).unwrap_or_default(), false, vec![k.alici.clone()], false),
        };
        let (kural, params) = donusturuculu_yol(&k.kural);
        let tam = if onek.is_empty() { yol_ekle("", &kural) } else { yol_ekle(&onek, &kural) };
        let tam = if k.kural.ends_with('/') && !tam.ends_with('/') { format!("{tam}/") } else { tam };
        let kosullu = kosullu_erisim || k.kosullu;
        // İşlemler: fonksiyon (yöntem listesi) ya da sınıf (tanımlı HTTP yöntemleri)
        let mut islemler: Vec<(String, usize, &Fonksiyon, Option<&Sinif>, usize)> = vec![];
        let mut z_sinif: Option<Zincir> = None;
        let kontrolcu: String;
        match &k.hedef {
            Hedef::Fonksiyon(fdi, f) => {
                for y in k.yontemler.clone().unwrap_or_else(|| vec!["GET".into()]) {
                    if !matches!(y.as_str(), "HEAD" | "OPTIONS") {
                        islemler.push((y, *fdi, f, None, f.satir));
                    }
                }
                kontrolcu = modul_kontrolcusu(&dosyalar[*fdi]);
            }
            Hedef::Sinif(sdi, s) => {
                let z = Zincir::kur(b, *sdi, s);
                let izinli: Option<Vec<String>> = k
                    .yontemler
                    .clone()
                    .or_else(|| z.oznitelik("methods").and_then(|(_, a)| a.varsayilan.as_deref().and_then(metin_listesi)).map(|l| l.iter().map(|x| x.to_uppercase()).collect()));
                for y in HTTP {
                    let Some((d2, f, s2)) = z.metot(y) else { continue };
                    let yy = y.to_uppercase();
                    if izinli.as_ref().is_some_and(|l| !l.contains(&yy)) {
                        continue;
                    }
                    islemler.push((yy, d2, f, Some(s2), f.satir));
                }
                // View.dispatch_request: methods listesindeki her yöntem aynı işleyiciye
                if islemler.is_empty() {
                    if let (Some((d2, f, s2)), Some(l)) = (z.metot("dispatch_request"), izinli.clone()) {
                        for y in l.into_iter().filter(|y| !matches!(y.as_str(), "HEAD" | "OPTIONS")) {
                            islemler.push((y, d2, f, Some(s2), f.satir));
                        }
                    }
                }
                kontrolcu = kontrolcu_adi(&s.ad);
                z_sinif = Some(z);
            }
        }
        for (yontem, fdi, f, sinif, satir) in islemler {
            let id = format!("{yontem} {}", sezgi_yolu(&tam));
            if !gorulen.insert(id.clone()) {
                continue;
            }
            let yer = format!("{}:{satir}", guvenli::goreli(kok, &dosyalar[fdi].yol));
            let mut notlar: Vec<String> = vec![];
            // ---- Giriş ----
            let mut kararlar: Vec<Karar> = vec![];
            let mut guardlar: Vec<String> = vec![];
            for dek in &f.dekoratorler {
                if let Some(kr) = fl.isaret(fdi, dek, 0) {
                    kararlar.push(kr);
                    let etiket = if fl.kimlik_dekoratoru(fdi, &dek.ad) { crate::anlam::ANAHTAR_GUARD_ETIKETI } else { "" };
                    guardlar.push(format!("{}{etiket}", dek.ad));
                }
            }
            // Sınıfın `decorators = [...]` / `method_decorators = [...]` listesi
            if let Some(z) = &z_sinif {
                for anahtar in ["decorators", "method_decorators"] {
                    let Some((d2, a)) = z.oznitelik(anahtar) else { continue };
                    let deger = a.varsayilan.clone().unwrap_or_default();
                    let ogeler: Vec<String> = match liste_ogeleri(&deger) {
                        Some(l) => l.into_iter().map(|(_, x)| x).collect(),
                        // method_decorators = {"post": [login_required]}
                        None => crate::django::sozluk_degeri(&deger, &yontem.to_lowercase())
                            .and_then(|v| liste_ogeleri(&v))
                            .map(|l| l.into_iter().map(|(_, x)| x).collect())
                            .unwrap_or_default(),
                    };
                    for o in ogeler {
                        let c = cagri_ayir(&o)
                            .map(|c| Cagri {
                                ad: c.ad.clone(),
                                argumanlar: c.arglar.iter().map(|(_, x)| x.clone()).collect(),
                                anahtarli: c.anahtarli.iter().map(|(k, v, _)| (k.clone(), v.clone())).collect(),
                                ..Default::default()
                            })
                            .unwrap_or(Cagri { ad: o.clone(), ..Default::default() });
                        if let Some(kr) = fl.isaret(d2, &c, 0) {
                            kararlar.push(kr);
                            guardlar.push(c.ad.clone());
                        }
                    }
                }
                for (sdi, s) in &z.siniflar {
                    for dek in &s.dekoratorler {
                        if dek.ad.ends_with(".route") || dek.ad.ends_with(".resource") {
                            continue;
                        }
                        if let Some(kr) = fl.isaret(*sdi, dek, 0) {
                            kararlar.push(kr);
                            guardlar.push(dek.ad.clone());
                        }
                    }
                }
            }
            // Gövdede çağrılan doğrulama: verify_jwt_in_request()
            if f.govde_metni.contains("verify_jwt_in_request(") && !f.govde_metni.contains("optional=True") {
                kararlar.push(Karar::yeni(Giris::Gerekli, false));
                guardlar.push("verify_jwt_in_request".into());
            }
            // before_request kancaları: erişim zincirindeki her örnek
            for d in &zincir {
                for (kr, ad) in fl.kancalar(d, false) {
                    if Some(d) != ana_kok.as_ref() || !genel_kancalar.iter().any(|(_, g)| *g == ad) {
                        kararlar.push(kr);
                        guardlar.push(ad);
                    }
                }
            }
            if ana {
                for (kr, ad) in &genel_kancalar {
                    kararlar.push(kr.clone());
                    guardlar.push(ad.clone());
                }
            }
            let karar = kararlar.into_iter().reduce(Karar::ve).unwrap_or_else(|| Karar::yeni(Giris::Acik, false));
            // jwt_optional / optional=True: girişsiz de geçer, kimlik varsa kullanılır
            let istege = f.dekoratorler.iter().any(|d| {
                let kisa = d.ad.rsplit('.').next().unwrap_or(&d.ad);
                kisa == "jwt_optional" || (matches!(kisa, "jwt_required" | "login_required") && d.anahtarli.iter().any(|(k, v)| k == "optional" && v.trim() == "True"))
            });
            let (giris, kanit) = match karar.giris {
                Giris::Gerekli if karar.kesin => ("gerekli", "kesin"),
                Giris::Gerekli => ("gerekli", if karar.bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" }),
                Giris::Acik if istege => ("istege_bagli", "kesin"),
                Giris::Acik if karar.kesin && !karar.bilinmeyen => ("herkese_acik", "kesin"),
                Giris::Acik => ("herkese_acik", if karar.bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" }),
                Giris::Bilinmeyen => ("bilinmiyor", "varsayim_bilinmeyen_guard"),
            };
            let mut roller = karar.roller.clone();
            roller.sort();
            roller.dedup();
            guardlar.dedup();
            // ---- Girdiler ve yanıt ----
            let mut g = Girdiler { govde: vec![], sorgu: BTreeMap::new(), yanit: None, notlar: vec![] };
            let yazma = matches!(yontem.as_str(), "POST" | "PUT" | "PATCH");
            let mut sinif_dekleri: Vec<&Cagri> = vec![];
            if let Some(s) = sinif {
                sinif_dekleri.extend(s.dekoratorler.iter());
            }
            for dek in f.dekoratorler.iter().chain(sinif_dekleri) {
                let kisa = dek.ad.rsplit('.').next().unwrap_or(&dek.ad);
                let anahtar = |k: &str| dek.anahtarli.iter().find(|(a, _)| a == k).map(|(_, v)| v.trim().to_string());
                match kisa {
                    // Flask-Smorest @blp.arguments(Sema, location="query"), flask-apispec / webargs @use_kwargs(Sema)
                    "arguments" | "use_kwargs" | "use_args" | "input" => {
                        let Some(s) = dek.argumanlar.first() else { continue };
                        let konum = anahtar("location").or_else(|| anahtar("locations")).and_then(|l| metin_listesi(&l)).unwrap_or_default();
                        let sorgu_mu = konum.iter().any(|l| matches!(l.as_str(), "query" | "querystring")) || (konum.is_empty() && !yazma);
                        let sema = if s.trim_start().starts_with('{') {
                            // @use_kwargs({"tag": fields.Str()})
                            let ic = s.trim().trim_start_matches('{').trim_end_matches('}');
                            let mut alanlar = BTreeMap::new();
                            for oge in parcalar(ic, ',') {
                                let p = crate::py_ortak::bol(&oge, ':');
                                let (Some((_, k)), Some((o, _))) = (p.first(), p.get(1)) else { continue };
                                let Some(ad) = metin_sabiti(k) else { continue };
                                if let Some(c) = cagri_ayir(&oge[*o..]) {
                                    if let Some((a, _, _, api)) = fl.mm_alani(fdi, &c, dek.satir, true, 0, &mut g.notlar, &ad) {
                                        alanlar.insert(api.unwrap_or(ad), a);
                                    }
                                }
                            }
                            Some(Sema {
                                dto: "kwargs".into(),
                                dosya: Some(guvenli::goreli(kok, &dosyalar[fdi].yol)),
                                bulunamadi: false,
                                kalitim: None,
                                alanlar,
                                adaylar: vec![],
                                dizi: false,
                            })
                        } else {
                            fl.marshmallow(fdi, s, true, 0).map(|(sm, n, _)| {
                                g.notlar.extend(n);
                                sm
                            })
                        };
                        match (sema, sorgu_mu) {
                            (Some(sm), true) => g.sorgu.extend(sm.alanlar),
                            (Some(sm), false) => g.govde.push(Girdi { ad: None, tip: Some(sm.dto.clone()), sema: Some(sm) }),
                            (None, _) => g.govde.push(Girdi {
                                ad: None,
                                tip: Some(s.clone()),
                                sema: Some(Sema {
                                    dto: s.trim().to_string(),
                                    dosya: None,
                                    bulunamadi: true,
                                    kalitim: None,
                                    alanlar: BTreeMap::new(),
                                    adaylar: vec![],
                                    dizi: false,
                                }),
                            }),
                        }
                    }
                    // Flask-RESTX @ns.expect(model)
                    "expect" => {
                        if let Some(sm) = dek.argumanlar.first().and_then(|m| fl.restx_modeli(fdi, m, 0)) {
                            if anahtar("validate").as_deref() != Some("True") {
                                g.notlar.push("Flask-RESTX modeli validate=True olmadan doğrulanmaz: alan kuralları belge niteliğinde".into());
                            }
                            g.govde.push(Girdi { ad: None, tip: Some(sm.dto.clone()), sema: Some(sm) });
                        }
                    }
                    // Yanıt: @blp.response(200, Sema), @marshal_with(Sema), @ns.marshal_with(model)
                    "response" | "marshal_with" | "marshal_list_with" | "output" => {
                        let (durum, s) = match kisa {
                            "response" => (dek.argumanlar.first().and_then(|d| d.trim().parse::<u16>().ok()), dek.argumanlar.get(1).cloned().or_else(|| anahtar("schema"))),
                            _ => (anahtar("code").and_then(|d| d.parse().ok()), dek.argumanlar.first().cloned()),
                        };
                        let Some(s) = s else { continue };
                        let sonuc = fl.marshmallow(fdi, &s, false, 0).map(|(sm, _, donusum)| (sm, !donusum)).or_else(|| fl.restx_modeli(fdi, &s, 0).map(|sm| (sm, true)));
                        if let Some((mut sm, kesin)) = sonuc {
                            sm.dizi |= kisa == "marshal_list_with" || anahtar("as_list").as_deref() == Some("True");
                            g.yanit = Some(Yanit { kaynak: kisa.into(), kesin, durum: durum.or(Some(200)), sema: sm });
                        }
                    }
                    // flask-pydantic @validate(): gövde ve sorgu parametre tiplerinden
                    "validate" => {
                        for p in &f.parametreler {
                            let Some(t) = p.tip.as_deref() else { continue };
                            match p.ad.as_str() {
                                "body" | "form" => {
                                    if let Some(sm) = fl.pydantic(fdi, t) {
                                        g.govde.push(Girdi { ad: None, tip: Some(sm.dto.clone()), sema: Some(sm) });
                                    }
                                }
                                "query" => {
                                    if let Some(sm) = fl.pydantic(fdi, t) {
                                        g.sorgu.extend(sm.alanlar);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            let metin = &f.govde_metni;
            if yazma && g.govde.is_empty() {
                // Gövdede şema: XSchema().load(request.json) / schema.load(...) / Model(**request.json) / reqparse
                let [yukle, pyd] = govde_sema_desenleri();
                if let Some(sm) = yukle.captures_iter(metin).find_map(|c| fl.marshmallow(fdi, &c[1], true, 0)) {
                    g.notlar.extend(sm.1);
                    g.govde.push(Girdi { ad: None, tip: Some(sm.0.dto.clone()), sema: Some(sm.0) });
                } else if let Some(sm) = pyd.captures_iter(metin).find_map(|c| c.get(1).or(c.get(2)).and_then(|m| fl.pydantic(fdi, m.as_str()))) {
                    g.govde.push(Girdi { ad: None, tip: Some(sm.dto.clone()), sema: Some(sm) });
                }
            }
            let (rp_govde, rp_sorgu) = fl.reqparse(fdi, f, z_sinif.as_ref());
            if !rp_govde.is_empty() && g.govde.is_empty() {
                if yazma {
                    g.govde.push(Girdi {
                        ad: None,
                        tip: Some("reqparse".into()),
                        sema: Some(Sema {
                            dto: "reqparse".into(),
                            dosya: Some(guvenli::goreli(kok, &dosyalar[fdi].yol)),
                            bulunamadi: false,
                            kalitim: None,
                            alanlar: rp_govde,
                            adaylar: vec![],
                            dizi: false,
                        }),
                    });
                } else {
                    g.sorgu.extend(rp_govde);
                }
            }
            g.sorgu.extend(rp_sorgu);
            let (ham_govde, ham_sorgu) = istek_alanlari(metin);
            if yazma && g.govde.is_empty() && !ham_govde.is_empty() {
                let alanlar: BTreeMap<String, Alan> = ham_govde.into_iter().map(|(a, koseli)| (a, serbest_alan(satir, koseli))).collect();
                g.govde.push(Girdi {
                    ad: None,
                    tip: Some("request".into()),
                    sema: Some(Sema {
                        dto: "body".into(),
                        dosya: Some(guvenli::goreli(kok, &dosyalar[fdi].yol)),
                        bulunamadi: false,
                        kalitim: None,
                        alanlar,
                        adaylar: vec![],
                        dizi: false,
                    }),
                });
                g.notlar.push("gövde alanları işleyicinin okuduğu anahtarlardan çıkarıldı; tipleri ve doğrulamaları bilinmiyor".into());
            }
            for q in ham_sorgu {
                g.sorgu.entry(q).or_insert_with(|| serbest_alan(satir, false));
            }
            if metin.contains("request.files") {
                g.notlar.push("dosya doğrulaması kontrol edilemedi: request.files; dosyanın varlığı, türü ve içeriği önizlemede sınanmaz".into());
            }
            notlar.extend(std::mem::take(&mut g.notlar));
            let sorgu = if g.sorgu.is_empty() {
                vec![]
            } else {
                vec![Girdi {
                    ad: None,
                    tip: None,
                    sema: Some(Sema {
                        dto: "query".into(),
                        dosya: Some(guvenli::goreli(kok, &dosyalar[fdi].yol)),
                        bulunamadi: false,
                        kalitim: None,
                        alanlar: std::mem::take(&mut g.sorgu),
                        adaylar: vec![],
                        dizi: false,
                    }),
                }]
            };
            let parametreler: Vec<Girdi> = params.iter().map(|(a, t)| Girdi { ad: Some(a.clone()), tip: Some(t.clone()), sema: None }).collect();
            // ---- Veri ve davranış ----
            let mut bag = VeriBag::default();
            fl.orm(z_sinif.as_ref(), vec![(fdi, f)], &mut bag);
            let veri = (!fl.modeller.is_empty()).then_some(bag);
            if let std::collections::btree_map::Entry::Vacant(e) = dna_haritasi.entry(yer.clone()) {
                let v = match &z_sinif {
                    Some(z) => yontem_dna(b, kok, z, Some((fdi, f))),
                    None => crate::fastapi::dna_zinciri(b, kok, fdi, f, &[]),
                };
                if v.as_object().is_some_and(|o| !o.is_empty()) && !crate::dna::bos(&v) {
                    e.insert(v);
                }
            }
            if !notlar.is_empty() {
                let l = denetlenemeyen.entry(yer.clone()).or_default();
                for n in notlar {
                    if !l.contains(&n) {
                        l.push(n);
                    }
                }
            }
            if kosullu {
                kosullu_kapilar.push(id.clone());
            }
            kapi_routerlari.entry(yer.clone()).or_insert_with(|| dugum_kimligi(&k.alici));
            let ozet = f.belge.clone().unwrap_or_else(|| crate::fastapi::varsayilan_ozet(&f.ad));
            let fonksiyon = if sinif.is_some() { snake_camel(&yontem.to_lowercase()) } else { snake_camel(&f.ad) };
            kapilar.push(Kapi {
                id,
                yontem: yontem.clone(),
                yol: sezgi_yolu(&tam),
                kontrolcu: kontrolcu.clone(),
                fonksiyon,
                dosya: guvenli::goreli(kok, &dosyalar[fdi].yol),
                satir,
                giris: giris.into(),
                guardlar: guardlar.clone(),
                roller_kaniti: (!roller.is_empty() && !karar.rol_kesin).then(|| "varsayim".into()),
                roller,
                ozet: Some(ozet),
                govde: std::mem::take(&mut g.govde),
                sorgu,
                parametreler,
                kullanimdan_kalkmis: false,
                yerine: None,
                surum: None,
                uygulama: None,
                ana,
                giris_kaniti: kanit.into(),
                veri,
                yanit: g.yanit.take(),
            });
        }
    }
    kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
    // Örnek grafiği: uygulama, Blueprint, Api ve Namespace örnekleri ve bağlama kenarları (koşullarıyla)
    let mut routerlar: Vec<Value> = vec![];
    let mut adlar: Vec<&Dugum> = fl.ornekler.keys().collect();
    adlar.sort();
    for d in adlar {
        let ustler: Vec<Value> = fl
            .kenarlar
            .iter()
            .filter(|k| k.alt == *d)
            .map(|k| json!({ "id": dugum_kimligi(&k.ust), "icerme": format!("{}:{}", guvenli::goreli(kok, &dosyalar[k.di].yol), k.satir), "kosul": k.kosul, "kosullu": k.kosullu }))
            .collect();
        routerlar.push(json!({ "id": dugum_kimligi(d), "dosya": guvenli::goreli(kok, &dosyalar[d.0].yol), "satir": fl.ornekler[d].satir, "ustler": ustler }));
    }
    let ana = ana_kok.map(|a| format!("{}:{}", dosyalar[a.0].modul, a.1));
    (
        kapilar,
        json!({ "genel_onek": "", "genel_guardlar": [], "ana_uygulama": ana, "kosullu_kapilar": kosullu_kapilar, "denetlenemeyen": denetlenemeyen,
        "dna": dna_haritasi, "kaskadlar": fl.kaskadlar(), "routerlar": routerlar, "kapi_routerlari": kapi_routerlari, "veri_modelleri": fl.veri_modelleri() }),
    )
}

/// Flask yolu ("/a/:id/") Sezgi biçiminde zaten; boşsa "/"
fn sezgi_yolu(y: &str) -> String {
    if y.is_empty() {
        "/".into()
    } else {
        y.to_string()
    }
}
