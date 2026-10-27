//! Tek keşif: çatı seçimi, çatı arayüzü, dil bağımsız ayrıştırma önbelleği ve adaptörün okuduğu dosyaların içerik
//! özetleri.
//! - **Çatı seçimi tek yerde** (`cati_sec`): `init`, doktor ve paylaşım dosyası aynı adaptörü seçer.
//! - **Çatı arayüzü** (`Cati`): NestJS, Express / Fastify / Next.js, FastAPI, Django, Flask, Spring Boot, Ktor, Rails, ASP.NET Core ve Go toplayıcıları aynı arayüzden çalışır;
//!   her biri keşif, ayrıştırma ve adaptörü yapar, okuduğu dosyaları içerik özetleriyle döner.
//! - **Önbellek** (`Onbellek`): dosya başına (parmak izi, içerik özeti, ayrıştırılmış model). Parmak izi (değişim zamanı
//!   ve boyut) aynıysa dosya okunmaz; değiştiyse okunur ve içerik özeti aynıysa yine yeniden ayrıştırılmaz. Süreç içidir
//!   (`sezgi calistir`); `init` boş önbellekle başlar.
//! - **Okunan dosyalar:** haritanın özetine `okunan_dosyalar: [{yol, ozet}]` olarak yazılır. Bayatlık (izleme, yazma
//!   kapısı, doktor) ve paylaşım dosyası değişim zamanına değil bu özetlere bakar.
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::{adaptor, aspnet, django, express, fastapi, flask, gohttp, golang, guvenli, kotlin, ktor, laravel, model, python, rails, spring, Toplam};

// Kaynaksız keşif: masaüstü klasörü triyajı, PE ve SQLite okuyucuları, sürümü sabit REA köprüsü (`sezgi kesfet`) ve
// REA'nın Sezgi'nin önekine kurulması (`sezgi rea`)
pub(crate) mod bagla;
pub(crate) mod electron;
pub(crate) mod genis;
pub(crate) mod masaustu;
pub(crate) mod pe;
pub(crate) mod rea;
pub(crate) mod rea_genis;
pub(crate) mod rea_kur;
pub(crate) mod rea_tablo;
pub(crate) mod veritabani;

/// `init`'in uygulama için seçtiği adaptör
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Secim {
    NestJs,
    FastApi,
    /// Django (Django REST Framework dahil)
    Django,
    /// Flask (Flask-RESTful, Flask-Smorest, Flask-RESTX dahil)
    Flask,
    /// Express, Fastify ya da Next.js
    Express,
    /// Spring Boot (Java ve Kotlin)
    Spring,
    /// Ktor (Kotlin; yapı dosyasında io.ktor sunucusu)
    Ktor,
    /// Ruby on Rails
    Rails,
    /// ASP.NET Core (C#, `.csproj` Web SDK)
    AspNet,
    /// Go: net/http, Gin, Echo, Chi, Fiber, gorilla/mux
    Go,
    /// PHP + Laravel (composer.json'da laravel/framework)
    Laravel,
}

impl Secim {
    /// Python uygulaması mı (adı ve sürümü pyproject.toml'dan okunur)
    pub(crate) fn python_mi(self) -> bool {
        matches!(self, Secim::FastApi | Secim::Django | Secim::Flask)
    }
}

/// `init`'in adaptör seçimi. Doktor ve paylaşım dosyası da bunu kullanır: bir uygulama için tek adaptör kimliği
pub(crate) fn cati_sec(kok: &Path, pkg: &str, kurallar: &adaptor::Kurallar) -> Option<Secim> {
    if pkg.contains(&format!("\"{}\"", kurallar.adaptor.isaret_bagimlilik)) {
        Some(Secim::NestJs)
    } else if python::fastapi_mi(kok) {
        Some(Secim::FastApi)
    } else if python::django_mi(kok) {
        Some(Secim::Django)
    } else if python::flask_mi(kok) {
        Some(Secim::Flask)
    } else if spring::spring_mi(kok) {
        Some(Secim::Spring)
    } else if ktor::ktor_mi(kok) {
        Some(Secim::Ktor)
    } else if rails::rails_mi(kok) {
        Some(Secim::Rails)
    } else if laravel::laravel_mi(kok) {
        Some(Secim::Laravel)
    } else if express::destekli_mi(pkg) {
        Some(Secim::Express)
    } else if aspnet::aspnet_mi(kok) {
        Some(Secim::AspNet)
    } else if golang::go_mi(kok) {
        Some(Secim::Go)
    } else {
        None
    }
}

/// Seçilen adaptörün kimliği (haritanın `ozet.adaptor` alanı)
pub(crate) fn adaptor_kimligi(secim: Secim, kurallar: &adaptor::Kurallar) -> String {
    match secim {
        Secim::NestJs => format!("{}@{}", kurallar.adaptor.ad, kurallar.adaptor.surum),
        Secim::FastApi => format!("fastapi@{}", fastapi::SURUM),
        Secim::Django => format!("django@{}", django::SURUM),
        Secim::Flask => format!("flask@{}", flask::SURUM),
        Secim::Express => format!("express@{}", express::SURUM),
        Secim::Spring => format!("spring@{}", spring::SURUM),
        Secim::Ktor => format!("ktor@{}", ktor::SURUM),
        Secim::Rails => format!("rails@{}", rails::SURUM),
        Secim::AspNet => format!("aspnet@{}", aspnet::SURUM),
        Secim::Go => format!("go@{}", gohttp::SURUM),
        Secim::Laravel => format!("laravel@{}", laravel::SURUM),
    }
}

/// Klasördeki uygulamanın çatısı (Sezgi'nin okuyabildikleri)
pub(crate) fn cati(k: &Path) -> Option<&'static str> {
    let pkg = guvenli::oku(&k.join("package.json")).unwrap_or_default();
    let var = |ad: &str| pkg.contains(&format!("\"{ad}\""));
    if var("@nestjs/core") {
        Some("NestJS")
    } else if python::fastapi_mi(k) {
        Some("FastAPI")
    } else if python::django_mi(k) {
        Some("Django")
    } else if python::flask_mi(k) {
        Some("Flask")
    } else if spring::spring_mi(k) {
        Some("Spring Boot")
    } else if ktor::ktor_mi(k) {
        Some("Ktor")
    } else if rails::rails_mi(k) {
        Some("Rails")
    } else if laravel::laravel_mi(k) {
        Some("Laravel")
    } else if var("next") {
        Some("Next.js")
    } else if var("fastify") {
        Some("Fastify")
    } else if var("express") {
        Some("Express")
    } else if aspnet::aspnet_mi(k) {
        Some("ASP.NET Core")
    } else if golang::go_mi(k) {
        Some(golang::cati_adi(k))
    } else {
        None
    }
}

/// Bir çatının toplayıcısı: keşif, ayrıştırma (önbellekli) ve adaptör
pub(crate) trait Cati {
    /// Haritanın `ozet.adaptor` alanı
    fn kimlik(&self) -> String;
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam;
}

struct NestJs<'a> {
    kurallar: &'a adaptor::Kurallar,
    pkg: &'a str,
}

struct Express;

struct FastApi;

/// Django ve Flask: Python modeli üstünde kendi adaptörleri
struct PyCatisi(python::PyCati);

struct Spring;

struct Ktor;

struct Rails;

struct AspNet;

struct Go;

struct Laravel;

impl Cati for NestJs<'_> {
    fn kimlik(&self) -> String {
        adaptor_kimligi(Secim::NestJs, self.kurallar)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        crate::ts_topla(kok, self.kurallar, self.pkg, &mut onbellek.ts, t0)
    }
}

impl Cati for Express {
    fn kimlik(&self) -> String {
        format!("express@{}", express::SURUM)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        express::topla(kok, &mut onbellek.js, t0)
    }
}

impl Cati for FastApi {
    fn kimlik(&self) -> String {
        format!("fastapi@{}", fastapi::SURUM)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        python::topla(kok, python::PyCati::FastApi, &mut onbellek.py, t0)
    }
}

impl Cati for PyCatisi {
    fn kimlik(&self) -> String {
        match self.0 {
            python::PyCati::Django => format!("django@{}", django::SURUM),
            python::PyCati::Flask => format!("flask@{}", flask::SURUM),
            python::PyCati::FastApi => format!("fastapi@{}", fastapi::SURUM),
        }
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        python::topla(kok, self.0, &mut onbellek.py, t0)
    }
}

impl Cati for Spring {
    fn kimlik(&self) -> String {
        format!("spring@{}", spring::SURUM)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        spring::topla(kok, &mut onbellek.java, t0)
    }
}

impl Cati for Ktor {
    fn kimlik(&self) -> String {
        format!("ktor@{}", ktor::SURUM)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        ktor::topla(kok, &mut onbellek.kt, t0)
    }
}

impl Cati for Rails {
    fn kimlik(&self) -> String {
        format!("rails@{}", rails::SURUM)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        rails::topla(kok, &mut onbellek.rb, t0)
    }
}

impl Cati for AspNet {
    fn kimlik(&self) -> String {
        format!("aspnet@{}", aspnet::SURUM)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        aspnet::topla(kok, &mut onbellek.cs, t0)
    }
}

impl Cati for Go {
    fn kimlik(&self) -> String {
        format!("go@{}", gohttp::SURUM)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        golang::topla(kok, &mut onbellek.go, t0)
    }
}

impl Cati for Laravel {
    fn kimlik(&self) -> String {
        format!("laravel@{}", laravel::SURUM)
    }
    fn topla(&self, kok: &Path, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
        laravel::topla(kok, &mut onbellek.php, t0)
    }
}

/// Seçimin toplayıcısı
pub(crate) fn cati_nesnesi<'a>(secim: Secim, kurallar: &'a adaptor::Kurallar, pkg: &'a str) -> Box<dyn Cati + 'a> {
    match secim {
        Secim::NestJs => Box::new(NestJs { kurallar, pkg }),
        Secim::Express => Box::new(Express),
        Secim::FastApi => Box::new(FastApi),
        Secim::Django => Box::new(PyCatisi(python::PyCati::Django)),
        Secim::Flask => Box::new(PyCatisi(python::PyCati::Flask)),
        Secim::Spring => Box::new(Spring),
        Secim::Ktor => Box::new(Ktor),
        Secim::Rails => Box::new(Rails),
        Secim::AspNet => Box::new(AspNet),
        Secim::Go => Box::new(Go),
        Secim::Laravel => Box::new(Laravel),
    }
}

/// Çatı seçimini belirleyen yapılandırma dosyaları (`cati_sec` ve `python::fastapi_mi` / `django_mi` / `flask_mi`, `rails::rails_mi`, `golang::go_mi`, `laravel::laravel_mi` bunları okur)
const YAPILANDIRMA: [&str; 11] =
    ["package.json", "pyproject.toml", "requirements.txt", "requirements.in", "setup.py", "setup.cfg", "Pipfile", "manage.py", "Gemfile", "go.mod", "composer.json"];

/// Seçilen çatıyla toplar. Okunan dosyalara çatı seçimini belirleyen yapılandırma dosyaları eklenir; liste sıralı ve
/// tekrarsızdır
pub(crate) fn topla(secim: Secim, kok: &Path, kurallar: &adaptor::Kurallar, pkg: &str, onbellek: &mut Onbellek, t0: Instant) -> Toplam {
    let c = cati_nesnesi(secim, kurallar, pkg);
    let mut t = c.topla(kok, onbellek, t0);
    t.adaptor = c.kimlik();
    for ad in YAPILANDIRMA {
        let p = kok.join(ad);
        if let Some(b) = guvenli::oku_bayt(&p) {
            t.okunan.push((p, icerik_ozeti(&b)));
        }
    }
    t.okunan.sort();
    t.okunan.dedup_by(|a, b| a.0 == b.0);
    t
}

// ---- Önbellek ----

/// Dosyanın parmak izi: değişim zamanı ve boyutu
pub type Parmak = (Option<SystemTime>, u64);

pub fn parmak(p: &Path) -> Option<Parmak> {
    p.metadata().ok().map(|m| (m.modified().ok(), m.len()))
}

/// Bir dosyanın önbellek kaydı: parmak izi, içerik özeti ve ayrıştırılmış model
#[derive(Clone)]
pub struct Kayit<T> {
    parmak: Option<Parmak>,
    ozet: String,
    dosya: T,
}

/// Dil bağımsız ayrıştırma önbelleği (süreç içi; `sezgi calistir`)
#[derive(Default)]
pub struct Onbellek {
    pub(crate) ts: HashMap<PathBuf, Kayit<model::Dosya>>,
    pub(crate) js: HashMap<PathBuf, Kayit<express::Dosya>>,
    pub(crate) py: HashMap<PathBuf, Kayit<python::Dosya>>,
    pub(crate) rb: HashMap<PathBuf, Kayit<rails::Dosya>>,
    pub(crate) cs: HashMap<PathBuf, Kayit<aspnet::Dosya>>,
    pub(crate) go: HashMap<PathBuf, Kayit<golang::Dosya>>,
    /// PHP (Laravel)
    pub(crate) php: HashMap<PathBuf, Kayit<laravel::Dosya>>,
    /// Ön yüz adaptörü
    pub(crate) web: HashMap<PathBuf, Kayit<crate::web::Dosya>>,
    /// Spring Boot (Java ve Kotlin)
    pub(crate) java: HashMap<PathBuf, Kayit<spring::Dosya>>,
    /// Ktor (Kotlin; yönlendirme DSL'iyle)
    pub(crate) kt: HashMap<PathBuf, Kayit<kotlin::KtDosya>>,
    /// Çağrı grafiği için dilden bağımsız dosya özetleri
    pub(crate) cagri: HashMap<PathBuf, Kayit<crate::cagri::DosyaOzeti>>,
}

impl Onbellek {
    pub fn is_empty(&self) -> bool {
        self.ts.is_empty()
            && self.js.is_empty()
            && self.py.is_empty()
            && self.rb.is_empty()
            && self.cs.is_empty()
            && self.go.is_empty()
            && self.php.is_empty()
            && self.web.is_empty()
            && self.java.is_empty()
            && self.kt.is_empty()
            && self.cagri.is_empty()
    }
}

/// İçerik özeti: SHA-256, küçük harfli onaltılık. Okunan dosyalar, paylaşım dosyası ve kanıt kayıtları aynı özeti
/// kullanır (dış doğrulayıcılar standart bir araçla yeniden hesaplayabilsin)
pub fn icerik_ozeti(b: &[u8]) -> String {
    // Kartın dayanak özetleri (x-basis) ve kanıt deposu aynı fonksiyonu kullanır: tek SHA-256 kaynağı
    crate::benlik::sha256_hex(b)
}

/// `ayristir_hepsi` sonucu: yollar sırasıyla (yol, model), okunan dosyalar (yol, içerik özeti), yeniden ayrıştırılan sayısı
pub(crate) type Ayrisma<T> = (Vec<(PathBuf, T)>, Vec<(PathBuf, String)>, usize);

/// Dosyalar paralel, her biri bir kez: parmak izi değişmeyen dosya okunmaz, içeriği değişmeyen dosya yeniden
/// ayrıştırılmaz. Dönen: yollar sırasıyla (yol, model), okunan dosyalar (yol, içerik özeti) ve yeniden ayrıştırılan sayısı.
/// Önbellek bu dosyalarla yenilenir (silinen dosyalar düşer).
pub(crate) fn ayristir_hepsi<T: Clone + Send + Sync>(
    yollar: &[PathBuf],
    onbellek: &mut HashMap<PathBuf, Kayit<T>>,
    ayristir: impl Fn(&Path, Vec<u8>) -> Option<T> + Sync,
) -> Ayrisma<T> {
    let eski = &*onbellek;
    let sonuclar: Vec<(PathBuf, Kayit<T>, bool)> = yollar
        .par_iter()
        .filter_map(|p| {
            let onceki = eski.get(p);
            let iz = parmak(p);
            if let Some(o) = onceki.filter(|o| iz.is_some() && o.parmak == iz) {
                return Some((p.clone(), o.clone(), false));
            }
            let b = guvenli::oku_bayt(p)?;
            let ozet = icerik_ozeti(&b);
            if let Some(o) = onceki.filter(|o| o.ozet == ozet) {
                return Some((p.clone(), Kayit { parmak: iz, ..o.clone() }, false));
            }
            let dosya = ayristir(p, b)?;
            Some((p.clone(), Kayit { parmak: iz, ozet, dosya }, true))
        })
        .collect();
    let yeni = sonuclar.iter().filter(|s| s.2).count();
    onbellek.clear();
    let mut dosyalar = Vec::with_capacity(sonuclar.len());
    let mut okunan = Vec::with_capacity(sonuclar.len());
    for (p, k, _) in sonuclar {
        dosyalar.push((p.clone(), k.dosya.clone()));
        okunan.push((p.clone(), k.ozet.clone()));
        onbellek.insert(p, k);
    }
    (dosyalar, okunan, yeni)
}

// ---- Okunan dosyaların yolları ----

/// `..` ve `.` bileşenlerini sözcüksel olarak çözer (dosyanın var olması gerekmez)
pub(crate) fn duzlestir(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// Dosyanın uygulama köküne göre yolu, `/` ile. Kökün dışındaki (monorepo kardeş paketi) dosya için `../` kullanılır
pub(crate) fn goreli_yol(kok: &Path, p: &Path) -> Option<String> {
    let birlestir = |b: &mut dyn Iterator<Item = Component>| b.map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/");
    if let Ok(g) = p.strip_prefix(kok) {
        return Some(birlestir(&mut g.components()));
    }
    let (k, a) = (duzlestir(kok), duzlestir(p));
    let ortak = k.components().zip(a.components()).take_while(|(x, y)| x == y).count();
    if ortak == 0 {
        return None;
    }
    let yukari = k.components().count() - ortak;
    Some(format!("{}{}", "../".repeat(yukari), birlestir(&mut a.components().skip(ortak))))
}
