//! Django adaptörü (Django REST Framework dahil): Python modelinden (`python.rs`) NestJS ve FastAPI adaptörleriyle aynı
//! kapı yapısını (`Kapi`) üretir; kart, MCP, önizleme, OpenAPI ve canlı mod dilden bağımsız çalışır.
//!
//! Kesinlik ilkesi FastAPI'dekiyle aynıdır: anlam adlardan değil tanımlardan çıkar.
//! - **Rotalar:** ayarların `ROOT_URLCONF`'u (ayar modülü manage.py / wsgi.py'deki `DJANGO_SETTINGS_MODULE`'dan) ve oradan
//!   `include(...)` zinciri: `path` (dönüştürücüler `<int:pk>` tipli yol parametresidir), `re_path` / `url` (adlı gruplar
//!   parametredir; düzenli ifadenin tamamı yola çevrilemezse işlem "denetlenemeyen" notu taşır), `urlpatterns += ...` ve
//!   `if settings.DEBUG:` altındaki kayıtlar (koşullu kapı). DRF router'ları (`DefaultRouter`, `SimpleRouter`,
//!   `trailing_slash`, iç içe router'lar) ViewSet eylemlerine ve `@action`'lara açılır.
//! - **Yöntemler:** sınıf görünümünde tanımlı HTTP yöntemleri ve tabandan gelenler (`ListCreateAPIView` -> GET, POST;
//!   `CreateView` -> GET, POST, PUT), `http_method_names` ile süzülür; `@api_view([...])`, `@require_http_methods`.
//!   Yöntem kısıtı olmayan fonksiyon görünümü her yöntemi kabul eder: yöntemler gövdedeki `request.method`
//!   karşılaştırmalarından çıkarılır ve bu not edilir.
//! - **Giriş ve rol:** DRF `permission_classes` (görünüm, `@action`, `@permission_classes`, `as_view(...)`, yoksa
//!   ayarlardaki `DEFAULT_PERMISSION_CLASSES`, o da yoksa DRF'nin varsayılanı AllowAny) bileşimleriyle (`&`, `|`)
//!   değerlendirilir; Django'da `@login_required`, `@permission_required`, `@staff_member_required`, `LoginRequiredMixin`,
//!   `PermissionRequiredMixin`, `@method_decorator(...)` ve urls.py'deki sarmalayıcılar. Kesin yalnızca tanımı bilinen
//!   işaret; projedeki izin sınıfının gövdesinden okunan karar çıkarımdır; `get_permissions` gibi çalışma anında verilen
//!   karar çözülemezse giriş "bilinmiyor"dur.
//! - **Girdiler ve yanıt:** serializer'lar (`serializer_class`, `@action(serializer_class=...)`, gövdede
//!   `XSerializer(data=request.data)`), ModelSerializer'da modelin alanları; Django formları (`form_class`, `XForm(request.POST)`).
//! - **Veri:** Django modelleri (tablo adı `Meta.db_table` ya da `uygulama_model`), ORM okuma / yazma (Yorum),
//!   `on_delete=CASCADE` kaskadları ve sinyaller (`post_save`, `pre_delete` ... dinleyicileri olay düğümüdür).
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::adaptor::{Alan, Girdi, Kapi, Sema, VeriBag, Yanit};
use crate::fastapi::{metin_sabiti, pascal, snake_camel, Baglam, Yer};
use crate::guvenli;
use crate::py_ortak::{bol, cagri_ayir, donusturuculu_yol, liste_ogeleri, metin_listesi, parcalar, satir_bul, yol_ekle, yontem_dna, MetinCagri, Zincir};
use crate::python::{Dosya, Fonksiyon, Sinif};

pub const SURUM: &str = "0.1.0";

/// DRF ViewSet tabanları
const VIEWSET: [&str; 6] = ["ViewSet", "GenericViewSet", "ModelViewSet", "ReadOnlyModelViewSet", "ViewSetMixin", "ViewSetMixin"];
/// DRF APIView tabanları (generics dahil)
const APIVIEW: [&str; 12] = [
    "APIView",
    "GenericAPIView",
    "CreateAPIView",
    "ListAPIView",
    "RetrieveAPIView",
    "DestroyAPIView",
    "UpdateAPIView",
    "ListCreateAPIView",
    "RetrieveUpdateAPIView",
    "RetrieveDestroyAPIView",
    "RetrieveUpdateDestroyAPIView",
    "ObtainAuthToken",
];
/// Yöntem kısıtı ve önbellek gibi girişe etkisi olmayan bilinen dekoratörler
const ETKISIZ: [&str; 35] = [
    "action",
    "detail_route",
    "list_route",
    "extend_schema",
    "extend_schema_view",
    "swagger_auto_schema",
    "cache_response",
    "csrf_exempt",
    "csrf_protect",
    "ensure_csrf_cookie",
    "requires_csrf_token",
    "never_cache",
    "cache_page",
    "cache_control",
    "vary_on_headers",
    "vary_on_cookie",
    "xframe_options_exempt",
    "xframe_options_deny",
    "xframe_options_sameorigin",
    "gzip_page",
    "condition",
    "etag",
    "last_modified",
    "require_http_methods",
    "require_GET",
    "require_POST",
    "require_safe",
    "sensitive_post_parameters",
    "sensitive_variables",
    "atomic",
    "api_view",
    "renderer_classes",
    "parser_classes",
    "throttle_classes",
    "schema",
];

/// Bir sınıf görünümü tabanının kendiliğinden verdiği (yöntem, eylem) çiftleri
fn taban_yontemleri(ad: &str) -> &'static [(&'static str, &'static str)] {
    match ad {
        "CreateAPIView" => &[("POST", "create")],
        "ListAPIView" => &[("GET", "list")],
        "RetrieveAPIView" => &[("GET", "retrieve")],
        "DestroyAPIView" => &[("DELETE", "destroy")],
        "UpdateAPIView" => &[("PUT", "update"), ("PATCH", "partial_update")],
        "ListCreateAPIView" => &[("GET", "list"), ("POST", "create")],
        "RetrieveUpdateAPIView" => &[("GET", "retrieve"), ("PUT", "update"), ("PATCH", "partial_update")],
        "RetrieveDestroyAPIView" => &[("GET", "retrieve"), ("DELETE", "destroy")],
        "RetrieveUpdateDestroyAPIView" => &[("GET", "retrieve"), ("PUT", "update"), ("PATCH", "partial_update"), ("DELETE", "destroy")],
        "ObtainAuthToken" => &[("POST", "post")],
        // Django genel görünümleri
        "TemplateView"
        | "ListView"
        | "DetailView"
        | "ArchiveIndexView"
        | "YearArchiveView"
        | "MonthArchiveView"
        | "WeekArchiveView"
        | "DayArchiveView"
        | "TodayArchiveView"
        | "DateDetailView"
        | "PasswordResetDoneView"
        | "PasswordResetCompleteView"
        | "PasswordChangeDoneView" => &[("GET", "get")],
        "RedirectView" => &[("GET", "get")],
        "FormView" | "CreateView" | "UpdateView" | "LoginView" | "PasswordChangeView" | "PasswordResetView" | "PasswordResetConfirmView" => {
            &[("GET", "get"), ("POST", "post"), ("PUT", "put")]
        }
        "DeleteView" => &[("GET", "get"), ("POST", "post"), ("DELETE", "delete")],
        "LogoutView" => &[("POST", "post")],
        _ => &[],
    }
}

/// ViewSet tabanlarının ve karışımlarının eylemleri
fn viewset_eylemleri(ad: &str) -> &'static [&'static str] {
    match ad {
        "ModelViewSet" => &["list", "create", "retrieve", "update", "partial_update", "destroy"],
        "ReadOnlyModelViewSet" => &["list", "retrieve"],
        "ListModelMixin" => &["list"],
        "CreateModelMixin" => &["create"],
        "RetrieveModelMixin" => &["retrieve"],
        "UpdateModelMixin" => &["update", "partial_update"],
        "DestroyModelMixin" => &["destroy"],
        _ => &[],
    }
}

/// Router'ın liste ve ayrıntı rotalarında yöntem -> eylem eşlemesi
const LISTE_ESLEME: [(&str, &str); 2] = [("GET", "list"), ("POST", "create")];
const AYRINTI_ESLEME: [(&str, &str); 4] = [("GET", "retrieve"), ("PUT", "update"), ("PATCH", "partial_update"), ("DELETE", "destroy")];

fn guvenli_yontem(y: &str) -> bool {
    matches!(y, "GET" | "HEAD" | "OPTIONS")
}

/// Bir kaydın görünümü
#[derive(Clone, Debug)]
enum Hedef {
    /// Fonksiyon görünümü: (dosya, fonksiyon adı)
    Fonksiyon(usize, String),
    /// Projedeki sınıf görünümü: (dosya, sınıf adı, açık (yöntem, eylem) eşlemesi, `as_view(...)` anahtarlı argümanları)
    Sinif(usize, String, Option<Vec<(String, String)>>, Vec<(String, String)>),
    /// Dışarıdan gelen sınıf görünümü (`TemplateView.as_view(...)`, `LoginView.as_view()`): tam adı
    DisSinif(String),
    /// DRF DefaultRouter'ın kök görünümü: (router'ın dosyası, tanım satırı)
    ApiKok(usize, usize),
}

/// URL kaydı (include zinciri boyunca biriken önek, koşul ve sarmalayıcılarla)
#[derive(Clone, Debug)]
struct Rota {
    yol: String,
    parametreler: Vec<(String, String)>,
    hedef: Hedef,
    kayit_di: usize,
    kayit_satir: usize,
    kosullu: bool,
    /// Düzenli ifadenin tamamı yola çevrilemedi
    duzenli: bool,
    /// urls.py'de görünümü saran çağrılar (`login_required(...)`)
    sarmalayicilar: Vec<(usize, String)>,
    /// Kaydı yapan URL yapılandırması ya da router (bağımlılık grafiği)
    kaynak: String,
    ana: bool,
}

#[derive(Clone, Debug, Default)]
struct Onek {
    yol: String,
    parametreler: Vec<(String, String)>,
    kosul: Option<String>,
    kosullu: bool,
    duzenli: bool,
    kaynak: String,
    ana: bool,
}

/// URL yapılandırması ya da router düğümü: (dosya, satır, üstler [(id, içerme yeri, koşul, koşullu)])
type GrafDugumu = (String, usize, Vec<(String, String, Option<String>, bool)>);
/// Modelin serializer'a girebilecek alanı: (ad, model alanı (çağrı, satır, dosya); otomatik `id` için None)
type ModelAlanAdayi = (String, Option<(MetinCagri, usize, usize)>);
/// Rotanın görünümü: (kontrolcü, dosya, satır, sınıf zinciri, DRF mi, fonksiyon görünümü)
type GorunumBilgisi<'a> = (String, usize, usize, Option<Zincir<'a>>, bool, Option<(usize, &'a Fonksiyon)>);

/// Giriş durumu
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Giris {
    Acik,
    Gerekli,
    Bilinmeyen,
}

/// Bir izin işaretinin ya da bileşiminin kararı
#[derive(Clone, Debug)]
pub(crate) struct Karar {
    pub(crate) giris: Giris,
    pub(crate) kesin: bool,
    pub(crate) roller: Vec<String>,
    pub(crate) rol_kesin: bool,
    /// Tanınmayan bir işaret var (karar kesin olamaz)
    pub(crate) bilinmeyen: bool,
}

impl Karar {
    pub(crate) fn yeni(giris: Giris, kesin: bool) -> Karar {
        Karar { giris, kesin, roller: vec![], rol_kesin: true, bilinmeyen: giris == Giris::Bilinmeyen }
    }
    pub(crate) fn rollu(mut self, roller: Vec<String>, kesin: bool) -> Karar {
        self.roller = roller;
        self.rol_kesin = kesin;
        self
    }
    /// Hepsi geçmeli (liste, `&`)
    pub(crate) fn ve(self, o: Karar) -> Karar {
        let mut roller = self.roller.clone();
        roller.extend(o.roller.iter().cloned());
        let rol_kesin = self.rol_kesin && o.rol_kesin;
        let bilinmeyen = self.bilinmeyen || o.bilinmeyen;
        let (giris, kesin) = if self.giris == Giris::Gerekli || o.giris == Giris::Gerekli {
            (Giris::Gerekli, (self.giris == Giris::Gerekli && self.kesin) || (o.giris == Giris::Gerekli && o.kesin))
        } else if self.giris == Giris::Bilinmeyen || o.giris == Giris::Bilinmeyen {
            (Giris::Bilinmeyen, false)
        } else {
            (Giris::Acik, self.kesin && o.kesin)
        };
        Karar { giris, kesin, roller, rol_kesin, bilinmeyen }
    }
    /// Biri yeter (`|`)
    pub(crate) fn veya(self, o: Karar) -> Karar {
        let mut roller = self.roller.clone();
        roller.extend(o.roller.iter().cloned());
        let bilinmeyen = self.bilinmeyen || o.bilinmeyen;
        let (giris, kesin) = match (self.giris, o.giris) {
            (Giris::Gerekli, Giris::Gerekli) => (Giris::Gerekli, self.kesin && o.kesin),
            (Giris::Acik, _) if self.kesin => (Giris::Acik, true),
            (_, Giris::Acik) if o.kesin => (Giris::Acik, true),
            (Giris::Acik, _) | (_, Giris::Acik) => (Giris::Acik, false),
            _ => (Giris::Bilinmeyen, false),
        };
        // Rollerden biri yeter: liste rol şartı değil seçenektir
        Karar { giris, kesin, roller: if giris == Giris::Gerekli { roller } else { vec![] }, rol_kesin: false, bilinmeyen }
    }
}

/// Bayrak ve izin denetimleri metinden: `is_staff` -> "staff", `has_perm("a.b")` -> "a.b", `groups.filter(name="x")` -> "x"
pub(crate) fn metindeki_roller(t: &str) -> Vec<String> {
    static D: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let [bayrak, izin, grup] = D.get_or_init(|| {
        [
            regex::Regex::new(r"\.is_(staff|superuser|admin)\b").unwrap(),
            regex::Regex::new(r#"\.has_perms?\(\s*\[?\s*["']([\w.]+)["']"#).unwrap(),
            regex::Regex::new(r#"groups\.filter\(\s*name\s*=\s*["']([\w .-]+)["']"#).unwrap(),
        ]
    });
    let mut v: Vec<String> = bayrak.captures_iter(t).map(|c| c[1].to_string()).collect();
    v.extend(izin.captures_iter(t).map(|c| c[1].to_string()));
    v.extend(grup.captures_iter(t).map(|c| c[1].to_string()));
    v.sort();
    v.dedup();
    v
}

/// Gövde giriş istiyor gibi mi: kimlik denetimi ve ret (yönlendirme, 401 / 403)
fn giris_isteyen_govde(t: &str) -> bool {
    (t.contains("is_authenticated") || t.contains("is_anonymous") || t.contains("request.user and") || t.contains("request.auth"))
        && ["redirect", "PermissionDenied", "HttpResponseForbidden", "401", "403", "NotAuthenticated", "raise", "return False", "login"].iter().any(|x| t.contains(x))
}

/// Ayar modülü ve değerleri
struct Ayarlar {
    /// Ayar modülleri (tek bir modül seçilemezse birden fazla)
    moduller: Vec<usize>,
    /// Seçim kesin mi (manage.py / wsgi.py'deki DJANGO_SETTINGS_MODULE ya da tek aday)
    pub(crate) kesin: bool,
}

pub(crate) struct Django<'a> {
    b: Baglam<'a>,
    kok: &'a Path,
    ayarlar: Ayarlar,
    /// Model sınıfları: ad -> (dosya, sınıf)
    modeller: HashMap<String, (usize, &'a Sinif)>,
    rotalar: Vec<Rota>,
    /// URL yapılandırması ve router grafiği: id -> (dosya, satır, üstler [(id, içerme yeri, koşul, koşullu)])
    graf: BTreeMap<String, GrafDugumu>,
    /// Çözülemeyen ya da projenin dışındaki kayıtlar: (yer, açıklama)
    disaridakiler: Vec<(String, String)>,
    adim: usize,
}

/// Modül adı (dosya modülünün son parçası olmadan): uygulama etiketi için
fn uygulama_etiketi(d: &Dosya) -> String {
    let p: Vec<&str> = d.modul.split('.').collect();
    // app/models.py -> app; app/models/kitap.py -> app
    match p.iter().rposition(|x| *x == "models") {
        Some(i) if i > 0 => p[i - 1].to_string(),
        _ => p.iter().rev().nth(1).or(p.last()).map(|s| s.to_string()).unwrap_or_default(),
    }
}

impl<'a> Django<'a> {
    fn yeni(kok: &'a Path, dosyalar: &'a [Dosya]) -> Django<'a> {
        let b = Baglam::yeni(dosyalar);
        let mut d = Django {
            b,
            kok,
            ayarlar: Ayarlar { moduller: vec![], kesin: false },
            modeller: HashMap::new(),
            rotalar: vec![],
            graf: BTreeMap::new(),
            disaridakiler: vec![],
            adim: 0,
        };
        d.ayarlari_bul();
        d.modelleri_bul();
        d
    }

    fn dosyalar(&self) -> &'a [Dosya] {
        self.b.dosyalar
    }

    fn goreli(&self, di: usize) -> String {
        guvenli::goreli(self.kok, &self.dosyalar()[di].yol)
    }

    // ---- Ayarlar ----

    /// Ayar modülü: manage.py / wsgi.py / asgi.py'deki `os.environ.setdefault("DJANGO_SETTINGS_MODULE", "x.settings")`,
    /// yoksa `ROOT_URLCONF` tanımlayan modüller
    fn ayarlari_bul(&mut self) {
        let mut adaylar: Vec<String> = vec![];
        for d in self.dosyalar() {
            let son = d.modul.rsplit('.').next().unwrap_or("");
            if !matches!(son, "manage" | "wsgi" | "asgi") {
                continue;
            }
            let cagrilar = d.cagrilar.iter().chain(d.fonksiyonlar.iter().flat_map(|f| f.ic_cagrilar.iter()));
            for c in cagrilar {
                if (c.ad.ends_with("environ.setdefault") || c.ad.ends_with("environ.__setitem__"))
                    && c.argumanlar.first().and_then(|a| metin_sabiti(a)).as_deref() == Some("DJANGO_SETTINGS_MODULE")
                {
                    if let Some(m) = c.argumanlar.get(1).and_then(|a| metin_sabiti(a)) {
                        if !adaylar.contains(&m) {
                            adaylar.push(m);
                        }
                    }
                }
            }
        }
        let mut moduller: Vec<usize> = adaylar.iter().filter_map(|m| self.b_modul(m)).collect();
        moduller.dedup();
        if moduller.len() == 1 {
            self.ayarlar = Ayarlar { moduller, kesin: true };
            return;
        }
        let tanimlayan: Vec<usize> =
            self.dosyalar().iter().enumerate().filter(|(_, d)| d.atamalar.contains_key("ROOT_URLCONF") || d.atamalar.contains_key("INSTALLED_APPS")).map(|(i, _)| i).collect();
        if moduller.is_empty() {
            moduller = tanimlayan;
        }
        let kesin = moduller.len() == 1;
        self.ayarlar = Ayarlar { moduller, kesin };
    }

    fn b_modul(&self, ad: &str) -> Option<usize> {
        self.dosyalar().iter().position(|d| d.modul == ad)
    }

    /// Ayar değeri: (modül, ifade metni). Yıldızlı içe aktarmalar (`from .base import *`) sırayla izlenir; sonraki atama
    /// öncekini ezer, modülün kendi ataması en sonda değerlendirilir. Birden fazla ayar modülü farklı değer veriyorsa
    /// ikinci öğe false (seçim kesin değil)
    fn ayar(&self, ad: &str) -> Option<((usize, String), bool)> {
        let mut sonuclar: Vec<(usize, String)> = vec![];
        for &m in &self.ayarlar.moduller {
            if let Some(x) = self.ayar_modulde(m, ad, 0) {
                sonuclar.push(x);
            }
        }
        let ilk = sonuclar.first()?.clone();
        let tek = sonuclar.iter().all(|x| x.1 == ilk.1);
        Some((ilk, tek && self.ayarlar.kesin))
    }

    fn ayar_modulde(&self, m: usize, ad: &str, derinlik: usize) -> Option<(usize, String)> {
        if derinlik > 4 {
            return None;
        }
        let d = &self.dosyalar()[m];
        if let Some(v) = d.atamalar.get(ad) {
            return Some((m, v.clone()));
        }
        d.yildizlar.iter().rev().filter_map(|y| self.b.modul_bul(m, y)).find_map(|m2| self.ayar_modulde(m2, ad, derinlik + 1))
    }

    /// `REST_FRAMEWORK = {"ANAHTAR": değer}` sözlüğündeki değer
    fn drf_ayari(&self, anahtar: &str) -> Option<((usize, String), bool)> {
        let ((m, sozluk), kesin) = self.ayar("REST_FRAMEWORK")?;
        sozluk_degeri(&sozluk, anahtar).map(|v| ((m, v), kesin))
    }

    // ---- Modeller ----

    /// Sınıf bir Django modeli mi (zincir `django.db.models.Model`'e ulaşıyor mu)
    fn model_mi(&self, di: usize, s: &'a Sinif) -> bool {
        let z = Zincir::kur(&self.b, di, s);
        z.dis.iter().any(|d| d.starts_with("django") && d.ends_with(".Model")) || z.dis.iter().any(|d| d == "models.Model")
    }

    fn soyut_mu(s: &Sinif) -> bool {
        s.ic_siniflar.iter().any(|m| m.ad == "Meta" && m.alanlar.iter().any(|a| a.ad == "abstract" && a.varsayilan.as_deref().map(str::trim) == Some("True")))
    }

    fn modelleri_bul(&mut self) {
        let dosyalar = self.dosyalar();
        for (di, d) in dosyalar.iter().enumerate() {
            for s in &d.siniflar {
                if self.model_mi(di, s) && !Self::soyut_mu(s) {
                    self.modeller.entry(s.ad.clone()).or_insert((di, s));
                }
            }
        }
    }

    /// Modelin tablo adı ve kaynağı
    fn tablo_adi(&self, di: usize, s: &Sinif) -> (String, &'static str) {
        let meta = s.ic_siniflar.iter().find(|m| m.ad == "Meta");
        if let Some(t) = meta.and_then(|m| m.alanlar.iter().find(|a| a.ad == "db_table")).and_then(|a| a.varsayilan.as_deref()).and_then(metin_sabiti) {
            return (t, "Meta.db_table");
        }
        let etiket = meta
            .and_then(|m| m.alanlar.iter().find(|a| a.ad == "app_label"))
            .and_then(|a| a.varsayilan.as_deref())
            .and_then(metin_sabiti)
            .unwrap_or_else(|| uygulama_etiketi(&self.dosyalar()[di]));
        (format!("{}_{}", etiket.to_lowercase(), s.ad.to_lowercase()), "varsayılan (uygulama etiketi_model adı)")
    }

    /// Modelin alanları (soyut tabanlardakiler dahil): (ad, çağrı, satır)
    fn model_alanlari(&self, di: usize, s: &'a Sinif) -> Vec<(String, MetinCagri, usize, usize)> {
        let z = Zincir::kur(&self.b, di, s);
        let mut v: Vec<(String, MetinCagri, usize, usize)> = vec![];
        for (d2, s2) in z.siniflar.iter().rev() {
            for a in &s2.alanlar {
                let Some(c) = a.varsayilan.as_deref().and_then(cagri_ayir) else { continue };
                if !self.model_alani_mi(*d2, &c) {
                    continue;
                }
                v.retain(|x| x.0 != a.ad);
                v.push((a.ad.clone(), c, a.satir, *d2));
            }
        }
        v
    }

    /// Çağrı bir model alanı mı (`models.CharField(...)`, projedeki alan alt sınıfı)
    fn model_alani_mi(&self, di: usize, c: &MetinCagri) -> bool {
        let tur = self.alan_turu(di, &c.ad);
        tur.ends_with("Field") || matches!(tur.as_str(), "ForeignKey" | "ManyToManyField" | "OneToOneField" | "GenericForeignKey")
    }

    /// Alan çağrısının adının dışarıdaki asıl adı (projedeki alt sınıf zinciri izlenir): "CharField"
    fn alan_turu(&self, di: usize, ad: &str) -> String {
        self.b.taban_dis_ad(di, ad, 0).unwrap_or_else(|| ad.rsplit('.').next().unwrap_or(ad).to_string())
    }

    /// İlişki alanının hedef modeli: sınıf adı, `"app.Model"`, `"self"`, `settings.AUTH_USER_MODEL`
    fn iliski_hedefi(&self, di: usize, sahip: &str, arg: &str) -> Option<String> {
        let a = arg.trim();
        if let Some(m) = metin_sabiti(a) {
            let ad = m.rsplit('.').next().unwrap_or(&m).to_string();
            return Some(if ad == "self" { sahip.to_string() } else { ad });
        }
        if a.ends_with("AUTH_USER_MODEL") || a.starts_with("get_user_model") {
            let kullanici = self.ayar("AUTH_USER_MODEL").and_then(|((_, v), _)| metin_sabiti(&v)).map(|m| m.rsplit('.').next().unwrap_or(&m).to_string());
            return Some(kullanici.unwrap_or_else(|| "User".into()));
        }
        match self.b.ifade(di, a) {
            Some(y @ Yer::Ad(..)) => self.b.sinif(&y).map(|(_, s)| s.ad.clone()),
            Some(Yer::Dis(_, ad)) => Some(ad.rsplit('.').next().unwrap_or(&ad).to_string()),
            _ => Some(a.rsplit('.').next().unwrap_or(a).to_string()),
        }
    }

    fn veri_modelleri(&self) -> Value {
        let mut v = vec![];
        let mut adlar: Vec<&String> = self.modeller.keys().collect();
        adlar.sort();
        for ad in adlar {
            let (di, s) = self.modeller[ad];
            let (tablo, kaynak) = self.tablo_adi(di, s);
            let mut alanlar = vec![];
            // Açık birincil anahtar yoksa Django `id` sütununu ekler
            let alanlar_ham = self.model_alanlari(di, s);
            if !alanlar_ham.iter().any(|(_, c, _, _)| c.anahtar("primary_key").map(str::trim) == Some("True")) {
                alanlar.push(json!({ "ad": "id", "tip": "AutoField", "satir": s.satir, "sutun": "id" }));
            }
            for (a, c, satir, _) in &alanlar_ham {
                let tur = self.alan_turu(di, &c.ad);
                if tur == "ManyToManyField" || tur == "GenericForeignKey" {
                    continue;
                }
                let sutun = c.anahtar("db_column").and_then(metin_sabiti).unwrap_or_else(|| {
                    if matches!(tur.as_str(), "ForeignKey" | "OneToOneField") {
                        format!("{a}_id")
                    } else {
                        a.clone()
                    }
                });
                alanlar.push(json!({ "ad": a, "tip": tur, "satir": satir, "sutun": sutun }));
            }
            v.push(json!({ "ad": ad, "tablo": tablo, "tablo_kaynagi": kaynak, "dosya": self.goreli(di), "satir": s.satir, "alanlar": alanlar }));
        }
        json!(v)
    }

    /// `on_delete=CASCADE` kaskadları: [ebeveyn, çocuk, "dosya:satır"]
    fn kaskadlar(&self) -> Vec<Value> {
        let mut v = vec![];
        let mut adlar: Vec<&String> = self.modeller.keys().collect();
        adlar.sort();
        for ad in adlar {
            let (di, s) = self.modeller[ad];
            for (_, c, satir, adi) in self.model_alanlari(di, s) {
                let tur = self.alan_turu(adi, &c.ad);
                if !matches!(tur.as_str(), "ForeignKey" | "OneToOneField" | "ParentalKey") {
                    continue;
                }
                let silme = c.anahtar("on_delete").or_else(|| c.arg(1)).unwrap_or("");
                if !silme.trim().ends_with("CASCADE") {
                    continue;
                }
                let Some(hedef) = c.arg(0).or_else(|| c.anahtar("to")).and_then(|a| self.iliski_hedefi(adi, ad, a)) else { continue };
                if hedef != *ad {
                    v.push(json!([hedef, ad, format!("{}:{satir}", self.goreli(adi))]));
                }
            }
        }
        v
    }

    // ---- URL yapılandırması ----

    fn rotalari_topla(&mut self) -> Option<String> {
        let kok_modul = self.ayar("ROOT_URLCONF").and_then(|((_, v), _)| metin_sabiti(&v));
        let kok_di = kok_modul.as_deref().and_then(|m| self.b_modul(m));
        // Kök bilinmiyorsa: hiçbir yerden içerilmeyen URL yapılandırmaları kök sayılır
        let mut kokler: Vec<(usize, bool)> = match kok_di {
            Some(k) => vec![(k, true)],
            None => vec![],
        };
        let mut gezilenler: HashSet<usize> = HashSet::new();
        for (di, ana) in kokler.clone() {
            let onek = Onek { yol: "/".into(), kaynak: self.urlconf_kimligi(di), ana, ..Default::default() };
            self.urlconf(di, &onek, 0, &mut gezilenler);
        }
        // Erişilmeyen URL yapılandırmaları: ikincil (kök bilinmiyorsa en çok rota bulunan ana sayılır). Başka bir erişilmeyen
        // yapılandırmanın içerdiği modül kendi başına gezilmez (öneki yanlış olurdu): önce kimin kimi içerdiği bulunur
        let yetim: Vec<usize> = self.dosyalar().iter().enumerate().filter(|(i, d)| !gezilenler.contains(i) && d.atamalar.contains_key("urlpatterns")).map(|(i, _)| i).collect();
        let (graf, disarida, rota_sayisi) = (self.graf.clone(), self.disaridakiler.len(), self.rotalar.len());
        let mut icerilen: HashSet<usize> = HashSet::new();
        for &di in &yetim {
            let mut g = HashSet::new();
            self.urlconf(di, &Onek { yol: "/".into(), ..Default::default() }, 0, &mut g);
            g.remove(&di);
            icerilen.extend(g);
        }
        self.graf = graf;
        self.disaridakiler.truncate(disarida);
        self.rotalar.truncate(rota_sayisi);
        let mut yetim_sonuclari: Vec<(usize, Vec<Rota>)> = vec![];
        for di in yetim.into_iter().filter(|d| !icerilen.contains(d)) {
            if gezilenler.contains(&di) {
                continue;
            }
            let once = self.rotalar.len();
            let onek = Onek { yol: "/".into(), kaynak: self.urlconf_kimligi(di), ana: false, ..Default::default() };
            self.urlconf(di, &onek, 0, &mut gezilenler);
            let yeni: Vec<Rota> = self.rotalar.drain(once..).collect();
            yetim_sonuclari.push((di, yeni));
        }
        if kok_di.is_none() {
            if let Some(en) = yetim_sonuclari.iter().enumerate().max_by_key(|(_, (_, r))| r.len()).map(|(i, _)| i) {
                for r in &mut yetim_sonuclari[en].1 {
                    r.ana = true;
                }
                kokler.push((yetim_sonuclari[en].0, true));
            }
        }
        for (_, r) in yetim_sonuclari {
            self.rotalar.extend(r);
        }
        kokler.first().map(|(di, _)| self.dosyalar()[*di].modul.clone())
    }

    fn urlconf_kimligi(&self, di: usize) -> String {
        format!("{}:urlpatterns", self.goreli(di))
    }

    /// Kaydı grafa ekler: `id` düğümü `ust`'ten içerildi
    fn graf_ekle(&mut self, id: &str, di: usize, satir: usize, ust: Option<(&str, String, Option<String>, bool)>) {
        let dosya = self.goreli(di);
        let dugum = self.graf.entry(id.to_string()).or_insert((dosya, satir, vec![]));
        if let Some((u, yer, kosul, kosullu)) = ust {
            if u != id && !dugum.2.iter().any(|x| x.0 == u && x.1 == yer) {
                dugum.2.push((u.to_string(), yer, kosul, kosullu));
            }
        }
    }

    /// Bir URL yapılandırma modülünün `urlpatterns`'ı: atama, `+=` artırımları, `append` / `extend` / `insert` çağrıları
    fn urlconf(&mut self, di: usize, onek: &Onek, derinlik: usize, gezilen: &mut HashSet<usize>) {
        gezilen.insert(di);
        if derinlik > 12 {
            return;
        }
        let d = &self.dosyalar()[di];
        let kimlik = self.urlconf_kimligi(di);
        let satir = d.atama_satirlari.get("urlpatterns").copied().unwrap_or(1);
        self.graf_ekle(&kimlik, di, satir, None);
        let onek = Onek { kaynak: kimlik, ..onek.clone() };
        if let Some(m) = d.atamalar.get("urlpatterns") {
            self.ifade(di, m, satir, &onek, derinlik, gezilen);
        }
        for a in d.artirimlar.iter().filter(|a| a.ad == "urlpatterns") {
            let mut o = Onek { kosul: a.kosul.clone().or(onek.kosul.clone()), kosullu: onek.kosullu || a.kosullu, ..onek.clone() };
            if a.kosullu {
                // `if settings.DEBUG: urlpatterns += [...]`: koşullu blok kendi düğümüdür, koşul ortam düğümüne bağlanır
                let blok = format!("{}@{}", o.kaynak, a.satir);
                let yer = format!("{}:{}", self.goreli(di), a.satir);
                let ust = o.kaynak.clone();
                self.graf_ekle(&blok, di, a.satir, Some((&ust, yer, a.kosul.clone(), true)));
                o.kaynak = blok;
            }
            self.ifade(di, &a.ifade, a.satir, &o, derinlik, gezilen);
        }
        for c in d.cagrilar.iter().filter(|c| c.ad.starts_with("urlpatterns.")) {
            let o = Onek { kosul: c.kosul.clone().or(onek.kosul.clone()), kosullu: onek.kosullu || c.kosullu, ..onek.clone() };
            match c.ad.as_str() {
                "urlpatterns.append" => {
                    if let Some(a) = c.argumanlar.first() {
                        self.oge(di, a, c.satir, &o, derinlik, gezilen);
                    }
                }
                "urlpatterns.insert" => {
                    if let Some(a) = c.argumanlar.get(1) {
                        self.oge(di, a, c.satir, &o, derinlik, gezilen);
                    }
                }
                "urlpatterns.extend" => {
                    if let Some(a) = c.argumanlar.first() {
                        self.ifade(di, a, c.satir, &o, derinlik, gezilen);
                    }
                }
                _ => {}
            }
        }
    }

    /// Desen listesi ifadesi: `[...] + router.urls`, değişken, `format_suffix_patterns(...)`, `i18n_patterns(...)`
    fn ifade(&mut self, di: usize, metin: &str, satir: usize, onek: &Onek, derinlik: usize, gezilen: &mut HashSet<usize>) {
        self.adim += 1;
        if derinlik > 12 || self.adim > 20_000 {
            return;
        }
        for (o, p) in bol(metin, '+') {
            let s = satir_bul(metin, o, satir);
            let p = p.trim_start_matches('*').trim();
            if let Some(ogeler) = liste_ogeleri(p) {
                for (o2, x) in ogeler {
                    self.oge(di, &x, satir_bul(p, o2, s), onek, derinlik, gezilen);
                }
                continue;
            }
            if let Some(alici) = p.strip_suffix(".urls") {
                if self.router_dugumu(di, alici).is_some() {
                    self.router(di, alici, s, onek);
                    continue;
                }
            }
            if let Some(c) = cagri_ayir(p) {
                match c.kisa_ad() {
                    "format_suffix_patterns" => {
                        if let Some(a) = c.arg(0).or(c.anahtar("urlpatterns")) {
                            self.ifade(di, a, s, onek, derinlik + 1, gezilen);
                        }
                    }
                    "i18n_patterns" => {
                        let o2 = Onek {
                            yol: yol_ekle(&onek.yol, ":language/"),
                            parametreler: [onek.parametreler.clone(), vec![("language".into(), "str".into())]].concat(),
                            ..onek.clone()
                        };
                        for (ao, a) in c.arglar.clone() {
                            self.oge(di, &a, satir_bul(p, ao, s), &o2, derinlik + 1, gezilen);
                        }
                    }
                    "patterns" => {
                        for (ao, a) in c.arglar.iter().skip(1).cloned().collect::<Vec<_>>() {
                            self.oge(di, &a, satir_bul(p, ao, s), onek, derinlik + 1, gezilen);
                        }
                    }
                    "static" | "staticfiles_urlpatterns" | "debug_toolbar_urls" => {}
                    _ => self.disaridakiler.push((format!("{}:{s}", self.goreli(di)), format!("URL deseni ifadesi çözülemedi: {}", kisalt(p)))),
                }
                continue;
            }
            // Değişken: aynı modülde ya da içe aktarılmış liste
            match self.b.ifade(di, p) {
                Some(Yer::Ad(d2, ad)) => {
                    let dd = &self.dosyalar()[d2];
                    let Some(m) = dd.atamalar.get(&ad) else { continue };
                    let s2 = dd.atama_satirlari.get(&ad).copied().unwrap_or(1);
                    let artirimlar: Vec<_> = dd.artirimlar.iter().filter(|a| a.ad == ad).cloned().collect();
                    self.ifade(d2, m, s2, onek, derinlik + 1, gezilen);
                    for a in artirimlar {
                        let o = Onek { kosul: a.kosul.clone().or(onek.kosul.clone()), kosullu: onek.kosullu || a.kosullu, ..onek.clone() };
                        self.ifade(d2, &a.ifade, a.satir, &o, derinlik + 1, gezilen);
                    }
                }
                Some(Yer::Modul(m)) => {
                    let o = Onek { ..onek.clone() };
                    self.urlconf(m, &o, derinlik + 1, gezilen);
                }
                _ => {}
            }
        }
    }

    /// Tek bir desen öğesi: `path(...)`, `re_path(...)`, `url(...)`
    fn oge(&mut self, di: usize, oge: &str, satir: usize, onek: &Onek, derinlik: usize, gezilen: &mut HashSet<usize>) {
        let oge = oge.trim();
        if oge.starts_with('*') {
            self.ifade(di, oge.trim_start_matches('*'), satir, onek, derinlik + 1, gezilen);
            return;
        }
        let Some(c) = cagri_ayir(oge) else {
            self.ifade(di, oge, satir, onek, derinlik + 1, gezilen);
            return;
        };
        let tur = c.kisa_ad().to_string();
        if !matches!(tur.as_str(), "path" | "re_path" | "url") {
            self.disaridakiler.push((format!("{}:{satir}", self.goreli(di)), format!("tanınmayan URL deseni: {}", kisalt(oge))));
            return;
        }
        let yer = format!("{}:{satir}", self.goreli(di));
        let Some(ham) = c.arg(0).or(c.anahtar("route")).or(c.anahtar("regex")).map(String::from) else { return };
        let Some(desen) = self.b.deger(di, &ham, 0) else {
            self.disaridakiler.push((yer, format!("yol değeri çözülemedi: {}", kisalt(&ham))));
            return;
        };
        let (parca, params, tam) = if tur == "path" {
            let (y, p) = donusturuculu_yol(&desen);
            (y, p, true)
        } else {
            duzenli_yol(&desen)
        };
        let yeni = Onek { yol: yol_birlestir_ham(&onek.yol, &parca), parametreler: [onek.parametreler.clone(), params].concat(), duzenli: onek.duzenli || !tam, ..onek.clone() };
        let Some(gorunum) = c.arg(1).or(c.anahtar("view")).map(String::from) else { return };
        // include(...)
        if let Some(ic) = cagri_ayir(&gorunum).filter(|x| x.kisa_ad() == "include") {
            self.include(di, &ic, satir, &yeni, derinlik, gezilen);
            return;
        }
        if gorunum.trim().ends_with("site.urls") {
            self.disaridakiler.push((yer, format!("{} Django yönetim paneli (admin) rotaları haritaya alınmadı", yeni.yol)));
            return;
        }
        let (hedef, sarmalayicilar) = match self.gorunum_coz(di, &gorunum, 0) {
            Some(x) => x,
            None => {
                self.disaridakiler.push((yer, format!("görünüm çözülemedi: {}", kisalt(&gorunum))));
                return;
            }
        };
        self.rotalar.push(Rota {
            yol: yeni.yol.clone(),
            parametreler: yeni.parametreler.clone(),
            hedef,
            kayit_di: di,
            kayit_satir: satir,
            kosullu: yeni.kosullu,
            duzenli: yeni.duzenli,
            sarmalayicilar,
            kaynak: yeni.kaynak.clone(),
            ana: yeni.ana,
        });
    }

    /// `include("app.urls")`, `include(("app.urls", "app"), namespace=...)`, `include(router.urls)`, `include([...])`,
    /// `include(modul)`
    fn include(&mut self, di: usize, c: &MetinCagri, satir: usize, onek: &Onek, derinlik: usize, gezilen: &mut HashSet<usize>) {
        let Some(mut arg) = c.arg(0).or(c.anahtar("arg")).map(String::from) else { return };
        // Demet: (desenler, uygulama adı)
        if let Some(ogeler) = liste_ogeleri(&arg).filter(|_| arg.trim_start().starts_with('(')) {
            if let Some((_, ilk)) = ogeler.first() {
                arg = ilk.clone();
            }
        }
        let yer = format!("{}:{satir}", self.goreli(di));
        if let Some(m) = metin_sabiti(&arg) {
            match self.b_modul(&m) {
                Some(m2) => {
                    let kimlik = self.urlconf_kimligi(m2);
                    let ust = onek.kaynak.clone();
                    let s2 = self.dosyalar()[m2].atama_satirlari.get("urlpatterns").copied().unwrap_or(1);
                    self.graf_ekle(&kimlik, m2, s2, Some((&ust, yer, onek.kosul.clone(), onek.kosullu)));
                    self.urlconf(m2, onek, derinlik + 1, gezilen);
                }
                None => self.disaridakiler.push((yer, format!("{} projenin dışındaki URL yapılandırması ({m}) haritaya alınmadı", onek.yol))),
            }
            return;
        }
        if let Some(alici) = arg.trim().strip_suffix(".urls") {
            if self.router_dugumu(di, alici).is_some() {
                self.router(di, alici, satir, onek);
                return;
            }
        }
        if arg.trim_start().starts_with('[') {
            self.ifade(di, &arg, satir, onek, derinlik + 1, gezilen);
            return;
        }
        match self.b.ifade(di, arg.trim()) {
            Some(Yer::Modul(m2)) => {
                let kimlik = self.urlconf_kimligi(m2);
                let ust = onek.kaynak.clone();
                let s2 = self.dosyalar()[m2].atama_satirlari.get("urlpatterns").copied().unwrap_or(1);
                self.graf_ekle(&kimlik, m2, s2, Some((&ust, yer, onek.kosul.clone(), onek.kosullu)));
                self.urlconf(m2, onek, derinlik + 1, gezilen);
            }
            Some(Yer::Ad(..)) => self.ifade(di, &arg, satir, onek, derinlik + 1, gezilen),
            _ => self.disaridakiler.push((yer, format!("include hedefi çözülemedi: {}", kisalt(&arg)))),
        }
    }

    /// Görünüm ifadesi: sarmalayıcılar açılır (`login_required(views.x)`, `cache_page(60)(V)`), `X.as_view(...)` sınıftır
    fn gorunum_coz(&self, di: usize, metin: &str, derinlik: usize) -> Option<(Hedef, Vec<(usize, String)>)> {
        if derinlik > 6 {
            return None;
        }
        let mut sarmalayicilar = vec![];
        let mut v = metin.trim().to_string();
        for _ in 0..8 {
            let Some(c) = cagri_ayir(&v) else { break };
            if let Some(sinif) = c.ad.strip_suffix(".as_view") {
                let eslesme = c.arg(0).or(c.anahtar("actions")).and_then(eylem_eslemesi);
                let kw: Vec<(String, String)> = c.anahtarli.iter().filter(|(k, _, _)| k != "actions").map(|(k, v, _)| (k.clone(), v.clone())).collect();
                return match self.b.ifade(di, sinif) {
                    Some(y @ Yer::Ad(..)) => {
                        let (sdi, s) = self.b.sinif(&y)?;
                        Some((Hedef::Sinif(sdi, s.ad.clone(), eslesme, kw), sarmalayicilar))
                    }
                    Some(Yer::Dis(m, a)) => Some((Hedef::DisSinif(format!("{m}.{a}")), sarmalayicilar)),
                    _ => Some((Hedef::DisSinif(sinif.to_string()), sarmalayicilar)),
                };
            }
            let ic = c.arg(0)?.to_string();
            sarmalayicilar.push((di, c.ad.clone()));
            v = ic;
        }
        match self.b.ifade(di, &v)? {
            y @ Yer::Ad(..) => {
                if let Some((fdi, f)) = self.b.fonksiyon(&y) {
                    return Some((Hedef::Fonksiyon(fdi, f.ad.clone()), sarmalayicilar));
                }
                // Modül düzeyinde atanmış görünüm: `kitap_listesi = KitapListesi.as_view()`
                let (atama, _) = self.b.atama(&y)?;
                let Yer::Ad(adi, _) = y else { return None };
                let (h, mut s2) = self.gorunum_coz(adi, atama, derinlik + 1)?;
                sarmalayicilar.append(&mut s2);
                Some((h, sarmalayicilar))
            }
            _ => None,
        }
    }

    // ---- Router'lar ----

    /// Router örneği: (dosya, ad, kurulum çağrısı)
    fn router_dugumu(&self, di: usize, alici: &str) -> Option<(usize, String, &'a crate::python::Cagri)> {
        let Some(Yer::Ad(rdi, rad)) = self.b.ifade(di, alici.trim()) else { return None };
        let c = self.dosyalar()[rdi].atama_cagrilari.get(&rad)?;
        let tur = self.b.taban_dis_ad(rdi, &c.ad, 0)?;
        tur.contains("Router").then_some((rdi, rad, c))
    }

    /// Router'ın kayıtları: (kayıt dosyası, satır, önek ifadesi, ViewSet ifadesi, koşullu); `registry.extend` ile
    /// eklenen başka router'ın kayıtları dahil
    fn router_kayitlari(&self, rdi: usize, rad: &str, derinlik: usize) -> Vec<(usize, usize, String, String, bool)> {
        let mut v = vec![];
        if derinlik > 4 {
            return v;
        }
        for (di, d) in self.dosyalar().iter().enumerate() {
            let cagrilar = d.cagrilar.iter().chain(d.fonksiyonlar.iter().flat_map(|f| f.ic_cagrilar.iter()));
            for c in cagrilar {
                if let Some(alici) = c.ad.strip_suffix(".register") {
                    if matches!(self.b.ifade(di, alici), Some(Yer::Ad(a, ref b)) if a == rdi && b == rad) {
                        let onek = c.argumanlar.first().cloned().or_else(|| c.anahtarli.iter().find(|(k, _)| k == "prefix").map(|(_, v)| v.clone()));
                        let vs = c.argumanlar.get(1).cloned().or_else(|| c.anahtarli.iter().find(|(k, _)| k == "viewset").map(|(_, v)| v.clone()));
                        if let (Some(o), Some(vs)) = (onek, vs) {
                            v.push((di, c.satir, o, vs, c.kosullu));
                        }
                    }
                } else if let Some(alici) = c.ad.strip_suffix(".registry.extend") {
                    if matches!(self.b.ifade(di, alici), Some(Yer::Ad(a, ref b)) if a == rdi && b == rad) {
                        if let Some(diger) = c.argumanlar.first().and_then(|x| x.strip_suffix(".registry")) {
                            if let Some((d2, a2, _)) = self.router_dugumu(di, diger) {
                                v.extend(self.router_kayitlari(d2, &a2, derinlik + 1));
                            }
                        }
                    }
                }
            }
        }
        v
    }

    fn router(&mut self, di: usize, alici: &str, satir: usize, onek: &Onek) {
        let Some((rdi, rad, c)) = self.router_dugumu(di, alici) else { return };
        let tur = self.b.taban_dis_ad(rdi, &c.ad, 0).unwrap_or_default();
        let kimlik = format!("{}:{rad}", self.goreli(rdi));
        let ust = onek.kaynak.clone();
        let yer = format!("{}:{satir}", self.goreli(di));
        self.graf_ekle(&kimlik, rdi, c.satir, Some((&ust, yer, onek.kosul.clone(), onek.kosullu)));
        let ts = match c.anahtarli.iter().find(|(k, _)| k == "trailing_slash").map(|(_, v)| v.trim()) {
            Some("False") => String::new(),
            Some(v) => metin_sabiti(v).unwrap_or_else(|| "/".into()),
            None => "/".into(),
        };
        let ts = if ts.contains('?') { String::new() } else { ts };
        // İç içe router (drf-nested-routers): NestedSimpleRouter(ust_router, r"alanlar", lookup="alan")
        let mut taban = onek.clone();
        taban.kaynak = kimlik.clone();
        if tur.starts_with("Nested") {
            let ust_onek = c.argumanlar.get(1).and_then(|x| self.b.deger(rdi, x, 0)).unwrap_or_default();
            let lookup = c.anahtarli.iter().find(|(k, _)| k == "lookup").and_then(|(_, v)| metin_sabiti(v)).unwrap_or_else(|| "parent".into());
            let ad = format!("{lookup}_pk");
            taban.yol = yol_birlestir_ham(&taban.yol, &format!("{}/:{ad}/", duzenli_yol(&ust_onek).0.trim_end_matches('/')));
            taban.parametreler.push((ad, "str".into()));
        }
        if tur.contains("DefaultRouter") {
            self.rotalar.push(Rota {
                yol: taban.yol.clone(),
                parametreler: taban.parametreler.clone(),
                hedef: Hedef::ApiKok(rdi, c.satir),
                kayit_di: rdi,
                kayit_satir: c.satir,
                kosullu: taban.kosullu,
                duzenli: taban.duzenli,
                sarmalayicilar: vec![],
                kaynak: kimlik.clone(),
                ana: taban.ana,
            });
        }
        for (kdi, ksatir, onek_ham, vs, kosullu) in self.router_kayitlari(rdi, &rad, 0) {
            let Some(onek_metni) = self.b.deger(kdi, &onek_ham, 0) else {
                self.disaridakiler.push((format!("{}:{ksatir}", self.goreli(kdi)), format!("router öneki çözülemedi: {}", kisalt(&onek_ham))));
                continue;
            };
            let Some((sdi, s)) = self.b.ifade(kdi, vs.trim()).and_then(|y| self.b.sinif(&y)) else {
                self.disaridakiler.push((format!("{}:{ksatir}", self.goreli(kdi)), format!("ViewSet çözülemedi: {}", kisalt(&vs))));
                continue;
            };
            let (parca, params, tam) = duzenli_yol(&onek_metni);
            let parca = parca.trim_end_matches('/').to_string();
            let z = Zincir::kur(&self.b, sdi, s);
            let lookup = z
                .oznitelik("lookup_url_kwarg")
                .or_else(|| z.oznitelik("lookup_field"))
                .and_then(|(_, a)| a.varsayilan.as_deref().and_then(metin_sabiti))
                .unwrap_or_else(|| "pk".into());
            let liste_yolu = yol_birlestir_ham(&taban.yol, &format!("{parca}{ts}"));
            let ayrinti_yolu = yol_birlestir_ham(&taban.yol, &format!("{parca}/:{lookup}{ts}"));
            let mut p0 = taban.parametreler.clone();
            p0.extend(params);
            let mut p1 = p0.clone();
            p1.push((lookup.clone(), "str".into()));
            let eylemler = self.viewset_eylem_listesi(&z);
            let liste: Vec<(String, String)> = LISTE_ESLEME.iter().filter(|(_, e)| eylemler.contains(&e.to_string())).map(|(y, e)| (y.to_string(), e.to_string())).collect();
            let ayrinti: Vec<(String, String)> = AYRINTI_ESLEME.iter().filter(|(_, e)| eylemler.contains(&e.to_string())).map(|(y, e)| (y.to_string(), e.to_string())).collect();
            let ekle = |yol: &str, ps: &Vec<(String, String)>, eslesme: Vec<(String, String)>, rotalar: &mut Vec<Rota>| {
                if eslesme.is_empty() {
                    return;
                }
                rotalar.push(Rota {
                    yol: yol.to_string(),
                    parametreler: ps.clone(),
                    hedef: Hedef::Sinif(sdi, s.ad.clone(), Some(eslesme), vec![]),
                    kayit_di: kdi,
                    kayit_satir: ksatir,
                    kosullu: taban.kosullu || kosullu,
                    duzenli: taban.duzenli || !tam,
                    sarmalayicilar: vec![],
                    kaynak: kimlik.clone(),
                    ana: taban.ana,
                });
            };
            ekle(&liste_yolu, &p0, liste, &mut self.rotalar);
            ekle(&ayrinti_yolu, &p1, ayrinti, &mut self.rotalar);
            // @action: ayrıntı (detail=True) ya da liste rotasının altında
            for (m, ayrinti_mi, url_yolu, yontemler) in self.ek_eylemler(&z) {
                let yol = if ayrinti_mi {
                    yol_birlestir_ham(&taban.yol, &format!("{parca}/:{lookup}/{url_yolu}{ts}"))
                } else {
                    yol_birlestir_ham(&taban.yol, &format!("{parca}/{url_yolu}{ts}"))
                };
                let eslesme = yontemler.into_iter().map(|y| (y, m.clone())).collect();
                ekle(&yol, if ayrinti_mi { &p1 } else { &p0 }, eslesme, &mut self.rotalar);
            }
        }
    }

    /// ViewSet'in standart eylemleri: tabanlardan (ModelViewSet, karışımlar) ve projede tanımlı eylem yöntemleri
    fn viewset_eylem_listesi(&self, z: &Zincir) -> Vec<String> {
        let mut v: Vec<String> = vec![];
        for ad in z.dis_adlar() {
            v.extend(viewset_eylemleri(ad).iter().map(|x| x.to_string()));
        }
        for e in ["list", "create", "retrieve", "update", "partial_update", "destroy"] {
            if z.metot(e).is_some() && !v.contains(&e.to_string()) {
                v.push(e.into());
            }
        }
        if let Some(izinli) = self.http_yontemleri(z) {
            let yontem = |e: &str| AYRINTI_ESLEME.iter().chain(LISTE_ESLEME.iter()).find(|(_, x)| *x == e).map(|(y, _)| y.to_lowercase());
            v.retain(|e| yontem(e).is_none_or(|y| izinli.contains(&y)));
        }
        v
    }

    /// `http_method_names = ["get", "post"]` (çözülebildiyse)
    fn http_yontemleri(&self, z: &Zincir) -> Option<Vec<String>> {
        z.oznitelik("http_method_names").and_then(|(_, a)| a.varsayilan.as_deref().and_then(metin_listesi)).map(|l| l.iter().map(|x| x.to_lowercase()).collect())
    }

    /// `@action(detail=..., methods=[...], url_path=...)` yöntemleri ve `@x.mapping.delete` ek yöntemleri:
    /// (yöntem adı, ayrıntı mı, url parçası, HTTP yöntemleri)
    fn ek_eylemler(&self, z: &Zincir) -> Vec<(String, bool, String, Vec<String>)> {
        let mut v: Vec<(String, bool, String, Vec<String>)> = vec![];
        let mut gorulen = HashSet::new();
        for (_, s) in &z.siniflar {
            for m in &s.metotlar {
                if !gorulen.insert(m.ad.clone()) {
                    continue;
                }
                for dek in &m.dekoratorler {
                    let kisa = dek.ad.rsplit('.').next().unwrap_or(&dek.ad);
                    if !matches!(kisa, "action" | "detail_route" | "list_route") {
                        continue;
                    }
                    let anahtar = |k: &str| dek.anahtarli.iter().find(|(a, _)| a == k).map(|(_, v)| v.trim().to_string());
                    let ayrinti = match kisa {
                        "detail_route" => true,
                        "list_route" => false,
                        _ => anahtar("detail").as_deref() == Some("True"),
                    };
                    let url = anahtar("url_path").and_then(|u| metin_sabiti(&u)).unwrap_or_else(|| m.ad.clone());
                    let yontemler: Vec<String> =
                        anahtar("methods").and_then(|l| metin_listesi(&l)).map(|l| l.iter().map(|x| x.to_uppercase()).collect()).unwrap_or_else(|| vec!["GET".into()]);
                    v.push((m.ad.clone(), ayrinti, url, yontemler));
                }
            }
        }
        // @eylem.mapping.delete: aynı rotaya başka yöntem
        for (_, s) in &z.siniflar {
            for m in &s.metotlar {
                for dek in &m.dekoratorler {
                    let p: Vec<&str> = dek.ad.split('.').collect();
                    if let [eylem, "mapping", yontem] = p.as_slice() {
                        if let Some(x) = v.iter_mut().find(|x| x.0 == *eylem) {
                            let y = yontem.to_uppercase();
                            if !x.3.contains(&y) {
                                x.3.push(y);
                            }
                        }
                    }
                }
            }
        }
        v
    }

    // ---- İzinler ----

    /// DRF izin ifadesi (sınıf, örnek, `A & B`, `A | B`, `~A`, ayarlardaki noktalı ad)
    fn drf_izni(&self, di: usize, ifade: &str, yontem: &str, model: Option<&str>, derinlik: usize) -> Karar {
        let t = ifade.trim();
        if derinlik > 8 {
            return Karar::yeni(Giris::Bilinmeyen, false);
        }
        let t = parantezsiz(t);
        let veya = parcalar(t, '|');
        if veya.len() > 1 {
            return veya.iter().map(|p| self.drf_izni(di, p, yontem, model, derinlik + 1)).reduce(Karar::veya).unwrap();
        }
        let ve = parcalar(t, '&');
        if ve.len() > 1 {
            return ve.iter().map(|p| self.drf_izni(di, p, yontem, model, derinlik + 1)).reduce(Karar::ve).unwrap();
        }
        if t.starts_with('~') {
            return Karar::yeni(Giris::Bilinmeyen, false);
        }
        let ad = t.split('(').next().unwrap_or(t).trim();
        let yer = match metin_sabiti(ad) {
            Some(nokta) => match nokta.rsplit_once('.') {
                Some((m, a)) => match self.b_modul(m) {
                    Some(m2) => self.b.coz(m2, a, 0),
                    None => Some(Yer::Dis(m.to_string(), a.to_string())),
                },
                None => None,
            },
            None => self.b.ifade(di, ad),
        };
        match yer {
            Some(Yer::Dis(m, a)) => yerlesik_izin(&m, a.rsplit('.').next().unwrap_or(&a), yontem, model),
            Some(y @ Yer::Ad(..)) => match self.b.sinif(&y) {
                Some((sdi, s)) => self.proje_izni(sdi, s, yontem, model, t),
                None => Karar::yeni(Giris::Bilinmeyen, false),
            },
            _ => yerlesik_izin("", ad.rsplit('.').next().unwrap_or(ad), yontem, model),
        }
    }

    /// Projedeki izin sınıfı: `has_permission` gövdesi okunur (çıkarım); yoksa tabandaki yerleşik sınıfın anlamı
    fn proje_izni(&self, di: usize, s: &'a Sinif, yontem: &str, model: Option<&str>, ifade: &str) -> Karar {
        let z = Zincir::kur(&self.b, di, s);
        let Some((_, f, _)) = z.metot("has_permission") else {
            // Yerleşik izin sınıfının alt sınıfı (gövde değişmedi) ya da yalnızca nesne düzeyi denetim
            for d in &z.dis {
                let (m, a) = d.rsplit_once('.').unwrap_or(("", d));
                let k = yerlesik_izin(m, a, yontem, model);
                if !(k.giris == Giris::Bilinmeyen) && a != "BasePermission" {
                    return k;
                }
            }
            // BasePermission.has_permission her zaman True: giriş istemez; nesne düzeyi denetim ayrıca yapılabilir
            return Karar::yeni(Giris::Acik, false);
        };
        let t = &f.govde_metni;
        let roller = metindeki_roller(t);
        // Argümanlı izin örneği: HasRole("admin")
        let mut roller = roller;
        if let Some(c) = cagri_ayir(ifade) {
            roller.extend(c.arglar.iter().filter_map(|(_, a)| metin_sabiti(a)));
        }
        if t.contains("SAFE_METHODS") && guvenli_yontem(yontem) && t.contains("return True") {
            return Karar::yeni(Giris::Acik, false);
        }
        if t.contains("is_authenticated") || t.contains("request.user and") || t.contains("request.auth") || t.contains("IsAuthenticated") || !roller.is_empty() {
            return Karar::yeni(Giris::Gerekli, false).rollu(roller, false);
        }
        if t.trim() == "return True" {
            return Karar::yeni(Giris::Acik, false);
        }
        Karar::yeni(Giris::Bilinmeyen, false)
    }

    /// Görünümün DRF izin kararı: eylemin `permission_classes`'ı, `as_view(...)`, sınıf özniteliği, `get_permissions`,
    /// fonksiyon görünümünde `@permission_classes`, yoksa ayarlar ve DRF varsayılanı
    fn drf_karari(&self, kaynak: Option<(usize, String)>, dinamik: Option<&Fonksiyon>, z_di: usize, yontem: &str, model: Option<&str>) -> (Karar, Vec<String>, Vec<String>) {
        let mut notlar = vec![];
        if let Some(f) = dinamik {
            // get_permissions: çalışma anında seçilir; gövdedeki bütün izin sınıfları aynı kararı veriyorsa o karar çıkarımdır
            static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
            let re = D.get_or_init(|| regex::Regex::new(r"\b([A-Z]\w*(?:\.[A-Z]\w*)?)\s*\(\s*\)").unwrap());
            let adlar: Vec<String> = re.captures_iter(&f.govde_metni).map(|c| c[1].to_string()).collect();
            notlar.push("izinler çalışma anında seçiliyor (get_permissions): giriş kararı koddan kesin çıkarılamaz".to_string());
            let kararlar: Vec<Karar> = adlar.iter().map(|a| self.drf_izni(z_di, a, yontem, model, 0)).collect();
            let mut guardlar = adlar.clone();
            guardlar.push("get_permissions".into());
            if kararlar.is_empty() {
                return (Karar::yeni(Giris::Bilinmeyen, false), guardlar, notlar);
            }
            let ilk = kararlar[0].giris;
            let k = if kararlar.iter().all(|k| k.giris == ilk) {
                let mut k = Karar::yeni(ilk, false);
                k.bilinmeyen = true;
                k
            } else {
                Karar::yeni(Giris::Bilinmeyen, false)
            };
            return (k, guardlar, notlar);
        }
        let (di, liste, ayardan, ayar_kesin) = match kaynak {
            Some((di, l)) => (di, l, false, true),
            None => match self.drf_ayari("DEFAULT_PERMISSION_CLASSES") {
                Some(((m, l), kesin)) => (m, l, true, kesin),
                None => {
                    // DRF varsayılanı AllowAny: ayar yokluğundan, çıkarım
                    return (Karar::yeni(Giris::Acik, false), vec!["AllowAny (DRF varsayılanı)".into()], notlar);
                }
            },
        };
        let ogeler: Vec<String> = match liste_ogeleri(&liste) {
            Some(o) => o.into_iter().map(|(_, x)| x).collect(),
            None => vec![liste.clone()],
        };
        if ogeler.is_empty() {
            // Boş liste: izin denetimi yok (açık beyan)
            return (Karar::yeni(Giris::Acik, !ayardan || ayar_kesin), vec![], notlar);
        }
        let mut k = ogeler.iter().map(|o| self.drf_izni(di, o, yontem, model, 0)).reduce(Karar::ve).unwrap();
        if ayardan && !ayar_kesin {
            k.kesin = false;
            notlar.push("ayar modülü tek anlamlı seçilemedi: varsayılan izinler çıkarım".into());
        }
        let guardlar = ogeler.iter().map(|o| metin_sabiti(o).map(|m| m.rsplit('.').next().unwrap_or(&m).to_string()).unwrap_or_else(|| o.clone())).collect();
        (k, guardlar, notlar)
    }

    /// Django işaretleri: dekoratör, `method_decorator(...)`, urls.py sarmalayıcısı. Girişe etkisi yoksa None
    fn django_isareti(&self, di: usize, metin: &str, derinlik: usize) -> Option<Karar> {
        let t = metin.trim();
        let c = cagri_ayir(t);
        let ad = c.as_ref().map(|c| c.ad.clone()).unwrap_or_else(|| t.to_string());
        // Zincirli fabrika: permission_required("x")(V) sarmalayıcısı "permission_required('x')" olarak gelir
        let (ad, c) = match cagri_ayir(&ad) {
            Some(ic) if c.is_some() && ad.ends_with(')') => (ic.ad.clone(), Some(ic)),
            _ => (ad, c),
        };
        let kisa = ad.rsplit('.').next().unwrap_or(&ad).to_string();
        if kisa == "method_decorator" {
            let ic = c.as_ref()?.arg(0)?.to_string();
            if let Some(l) = liste_ogeleri(&ic).filter(|_| ic.trim_start().starts_with(['[', '('])) {
                return l.iter().filter_map(|(_, x)| self.django_isareti(di, x, derinlik + 1)).reduce(Karar::ve);
            }
            return self.django_isareti(di, &ic, derinlik + 1);
        }
        let yer = self.b.ifade(di, &ad);
        let proje = matches!(yer, Some(Yer::Ad(..)));
        if !proje {
            match kisa.as_str() {
                "login_required" | "fresh_login_required" => return Some(Karar::yeni(Giris::Gerekli, true)),
                "permission_required" => {
                    let roller = c.as_ref().and_then(|c| c.arg(0).or(c.anahtar("perm"))).map(|a| metin_listesi(a).unwrap_or_default()).unwrap_or_default();
                    let kesin = !roller.is_empty();
                    return Some(Karar::yeni(Giris::Gerekli, true).rollu(roller, kesin));
                }
                "staff_member_required" => return Some(Karar::yeni(Giris::Gerekli, true).rollu(vec!["staff".into()], true)),
                "superuser_required" => return Some(Karar::yeni(Giris::Gerekli, true).rollu(vec!["superuser".into()], true)),
                "user_passes_test" => {
                    let roller = c.as_ref().and_then(|c| c.arg(0)).map(|a| self.test_rolleri(di, a)).unwrap_or_default();
                    return Some(Karar::yeni(Giris::Gerekli, false).rollu(roller, false));
                }
                "login_not_required" => {
                    let mut k = Karar::yeni(Giris::Acik, true);
                    k.rol_kesin = true;
                    return Some(k);
                }
                "permission_classes" | "authentication_classes" => return None,
                x if ETKISIZ.contains(&x) => return None,
                _ => {}
            }
            if matches!(yer, Some(Yer::Dis(..))) || yer.is_none() {
                // Tanınmayan dış dekoratör: giriş isteyip istemediği bilinmiyor
                let mut k = Karar::yeni(Giris::Acik, false);
                k.bilinmeyen = true;
                return Some(k);
            }
        }
        // Projedeki dekoratör: tanımı (ve iç sarmalayıcısı) okunur
        if derinlik > 3 {
            return None;
        }
        let (fdi, f) = yer.as_ref().and_then(|y| self.b.fonksiyon(y))?;
        let ic_metin: String = std::iter::once(f.govde_metni.as_str())
            .chain(self.dosyalar()[fdi].fonksiyonlar.iter().filter(|g| g.kapsam.as_deref() == Some(f.ad.as_str())).map(|g| g.govde_metni.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        // Başka bir giriş işaretini saran dekoratör: `return login_required(f)` / `@login_required` iç fonksiyonda
        for g in self.dosyalar()[fdi].fonksiyonlar.iter().filter(|g| g.kapsam.as_deref() == Some(f.ad.as_str())) {
            for d in &g.dekoratorler {
                if let Some(k) = self.django_isareti(fdi, &d.ad, derinlik + 1).filter(|k| k.giris == Giris::Gerekli) {
                    let mut k = k;
                    k.kesin = false;
                    return Some(k);
                }
            }
        }
        if ic_metin.contains("login_required(") || ic_metin.contains("user_passes_test(") || giris_isteyen_govde(&ic_metin) || crate::py_ortak::kimlik_bilgisi_denetimi(&ic_metin) {
            return Some(Karar::yeni(Giris::Gerekli, false).rollu(metindeki_roller(&ic_metin), false));
        }
        // Yalnızca yöntem süzen dekoratör (405): girişe etkisi yok, tanınmayan guard da değil
        if crate::py_ortak::yontem_suzen_govde(&ic_metin) {
            return None;
        }
        let mut k = Karar::yeni(Giris::Acik, false);
        k.bilinmeyen = true;
        Some(k)
    }

    /// Dağıtıcı görünüm: gövdesi isteği (`f(request, ...)`) projedeki başka görünümlere iletiyor ve iletilen her görünüm
    /// bir dekoratörle giriş istiyor: giriş gerekli, ama çağrıdan çıkarıldığı için kesin değil. Guard adı "çağrılan ->
    /// dekoratör" biçimindedir
    fn dagitici_karari(&self, di: usize, f: &Fonksiyon) -> Option<(Karar, Vec<String>)> {
        static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = D.get_or_init(|| regex::Regex::new(r"\b([A-Za-z_][\w.]*)\(\s*request\b").unwrap());
        let mut adlar: Vec<String> = vec![];
        let mut sayi = 0;
        for c in re.captures_iter(&f.govde_metni) {
            let ad = &c[1];
            let Some((gdi, g)) = self.b.ifade(di, ad).and_then(|y| self.b.fonksiyon(&y)) else { continue };
            if g.ad == f.ad {
                continue;
            }
            sayi += 1;
            let guard = g.dekoratorler.iter().find(|dk| self.django_isareti(gdi, &dekorator_metni(dk), 1).is_some_and(|k| k.giris == Giris::Gerekli))?;
            let etiket = if self.kimlik_dekoratoru(gdi, &guard.ad) { crate::anlam::ANAHTAR_GUARD_ETIKETI } else { "" };
            let a = format!("{} -> {}{etiket}", g.ad, guard.ad);
            if !adlar.contains(&a) {
                adlar.push(a);
            }
        }
        (sayi > 0).then(|| (Karar::yeni(Giris::Gerekli, false), adlar))
    }

    /// Projedeki dekoratör istekten API anahtarı / belirteç okuyup 401 / 403 dönüyor mu (guard adı etiketlenir)
    fn kimlik_dekoratoru(&self, di: usize, ad: &str) -> bool {
        self.b.ifade(di, ad).and_then(|y| self.b.fonksiyon(&y)).is_some_and(|(_, f)| crate::py_ortak::kimlik_bilgisi_denetimi(&f.govde_metni))
    }

    /// user_passes_test / UserPassesTestMixin testindeki bayraklar
    fn test_rolleri(&self, di: usize, test: &str) -> Vec<String> {
        let t = test.trim();
        if t.starts_with("lambda") {
            return metindeki_roller(t);
        }
        self.b.ifade(di, t).and_then(|y| self.b.fonksiyon(&y)).map(|(_, f)| metindeki_roller(&f.govde_metni)).unwrap_or_default()
    }

    /// Sınıf görünümünün Django işaretleri: karışımlar, sınıf dekoratörleri, `dispatch` ve işleyici yöntem dekoratörleri
    fn sinif_isaretleri(&self, z: &Zincir, isleyici: Option<&Fonksiyon>) -> (Vec<Karar>, Vec<String>) {
        let mut v = vec![];
        let mut adlar = vec![];
        for d in &z.dis {
            let kisa = d.rsplit('.').next().unwrap_or(d);
            let k = match kisa {
                "LoginRequiredMixin" => Some(Karar::yeni(Giris::Gerekli, true)),
                "PermissionRequiredMixin" => {
                    let roller = z.oznitelik("permission_required").and_then(|(_, a)| a.varsayilan.as_deref().and_then(metin_listesi)).unwrap_or_default();
                    let kesin = !roller.is_empty();
                    Some(Karar::yeni(Giris::Gerekli, true).rollu(roller, kesin))
                }
                "UserPassesTestMixin" => {
                    let roller = z.metot("test_func").map(|(_, f, _)| metindeki_roller(&f.govde_metni)).unwrap_or_default();
                    Some(Karar::yeni(Giris::Gerekli, false).rollu(roller, false))
                }
                "StaffuserRequiredMixin" | "StaffRequiredMixin" => Some(Karar::yeni(Giris::Gerekli, true).rollu(vec!["staff".into()], true)),
                "SuperuserRequiredMixin" => Some(Karar::yeni(Giris::Gerekli, true).rollu(vec!["superuser".into()], true)),
                _ => None,
            };
            if let Some(k) = k {
                adlar.push(kisa.to_string());
                v.push(k);
            }
        }
        for (sdi, s) in &z.siniflar {
            for dek in &s.dekoratorler {
                let metin = dekorator_metni(dek);
                // Sınıf dekoratörü `method_decorator(x, name="dispatch")` ya da işleyicinin adını verir
                let hedef = dek.anahtarli.iter().find(|(k, _)| k == "name").and_then(|(_, v)| metin_sabiti(v));
                if hedef.as_deref().is_some_and(|h| h != "dispatch" && isleyici.is_some_and(|f| f.ad != h)) {
                    continue;
                }
                if let Some(k) = self.django_isareti(*sdi, &metin, 0) {
                    adlar.push(dek.ad.clone());
                    v.push(k);
                }
            }
            // dispatch ezilmişse gövdesi
            if let Some(f) = s.metotlar.iter().find(|m| m.ad == "dispatch") {
                for dek in &f.dekoratorler {
                    if let Some(k) = self.django_isareti(*sdi, &dekorator_metni(dek), 0) {
                        adlar.push(dek.ad.clone());
                        v.push(k);
                    }
                }
                if giris_isteyen_govde(&f.govde_metni) {
                    adlar.push(format!("{}.dispatch", s.ad));
                    v.push(Karar::yeni(Giris::Gerekli, false).rollu(metindeki_roller(&f.govde_metni), false));
                }
            }
        }
        if let Some(f) = isleyici {
            for dek in &f.dekoratorler {
                if let Some(k) = self.django_isareti(z.siniflar.first().map(|x| x.0).unwrap_or(0), &dekorator_metni(dek), 0) {
                    adlar.push(dek.ad.clone());
                    v.push(k);
                }
            }
        }
        (v, adlar)
    }

    /// MIDDLEWARE: Django'nun LoginRequiredMiddleware'i (bütün görünümler giriş ister) ve giriş denetleyen proje ara katmanları
    fn ara_katman(&self) -> (bool, bool) {
        let Some(((_, liste), _)) = self.ayar("MIDDLEWARE").or_else(|| self.ayar("MIDDLEWARE_CLASSES")) else { return (false, false) };
        let mut zorunlu = false;
        let mut bilinmeyen = false;
        for o in metin_listesi(&liste).unwrap_or_default() {
            let (mod_adi, ad) = o.rsplit_once('.').unwrap_or(("", &o));
            if ad == "LoginRequiredMiddleware" && mod_adi.starts_with("django") {
                zorunlu = true;
                continue;
            }
            if let Some(m2) = self.b_modul(mod_adi) {
                let y = self.b.coz(m2, ad, 0);
                let govde = y.as_ref().and_then(|y| self.b.sinif(y)).map(|(_, s)| s.metotlar.iter().map(|f| f.govde_metni.as_str()).collect::<Vec<_>>().join("\n"));
                let govde = govde.or_else(|| {
                    y.as_ref().and_then(|y| self.b.fonksiyon(y)).map(|(d, f)| {
                        let mut t = f.govde_metni.clone();
                        for g in self.dosyalar()[d].fonksiyonlar.iter().filter(|g| g.kapsam.as_deref() == Some(f.ad.as_str())) {
                            t.push('\n');
                            t.push_str(&g.govde_metni);
                        }
                        t
                    })
                });
                if govde.as_deref().is_some_and(giris_isteyen_govde) {
                    bilinmeyen = true;
                }
            }
        }
        (zorunlu, bilinmeyen)
    }

    // ---- Şemalar ----

    /// Serializer / form sınıfının şeması. `girdi`: yalnızca yazılabilir alanlar (read_only ve HiddenField hariç);
    /// değilse yanıt alanları (write_only hariç). `kismi`: PATCH (bütün alanlar isteğe bağlı). Dönen: şema ve
    /// önizlemenin denetleyemediği kurallar
    fn sema(&self, di: usize, ifade: &str, girdi: bool, kismi: bool, derinlik: usize) -> Option<(Sema, Vec<String>)> {
        let ad = ifade.trim().split('(').next().unwrap_or(ifade).trim();
        let (sdi, s) = self.b.ifade(di, ad).and_then(|y| self.b.sinif(&y))?;
        let z = Zincir::kur(&self.b, sdi, s);
        let form = z.dis.iter().any(|d| d.contains("forms."));
        let model_tabanli = z.dis_var(&["ModelSerializer", "HyperlinkedModelSerializer", "ModelForm"]);
        let mut notlar = vec![];
        let mut alanlar: BTreeMap<String, Alan> = BTreeMap::new();
        let meta = z.ic_sinif("Meta");
        let meta_deger = |k: &str| meta.and_then(|(_, m)| m.alanlar.iter().find(|a| a.ad == k)).and_then(|a| a.varsayilan.clone());
        let salt_okunur: Vec<String> = meta_deger("read_only_fields").and_then(|v| metin_listesi(&v)).unwrap_or_default();
        let ek_ayarlar = meta_deger("extra_kwargs").unwrap_or_default();
        let derinlik_ayari: usize = meta_deger("depth").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        // Modelden gelen alanlar (ModelSerializer / ModelForm)
        let mut tam = true;
        if model_tabanli {
            let model_ad = meta_deger("model");
            let model = model_ad.as_deref().and_then(|m| self.b.ifade(meta.map(|x| x.0).unwrap_or(sdi), m.trim())).and_then(|y| self.b.sinif(&y));
            let fields = meta_deger("fields");
            let exclude: Vec<String> = meta_deger("exclude").and_then(|v| metin_listesi(&v)).unwrap_or_default();
            match model {
                Some((mdi, ms)) => {
                    let ham = self.model_alanlari(mdi, ms);
                    let tumu = fields.as_deref().is_some_and(|f| metin_sabiti(f).as_deref() == Some("__all__")) || fields.is_none() && !exclude.is_empty();
                    let secili: Option<Vec<String>> = if tumu { None } else { fields.as_deref().and_then(metin_listesi) };
                    if fields.is_some() && !tumu && secili.is_none() {
                        tam = false;
                    }
                    let id_var = ham.iter().any(|(_, c, _, _)| c.anahtar("primary_key").map(str::trim) == Some("True"));
                    let mut adaylar: Vec<ModelAlanAdayi> = vec![];
                    if !id_var {
                        adaylar.push(("id".into(), None));
                    }
                    adaylar.extend(ham.into_iter().map(|(a, c, sat, d)| (a, Some((c, sat, d)))));
                    let istenen: Vec<String> = match &secili {
                        Some(l) => l.clone(),
                        None => adaylar.iter().map(|(a, _)| a.clone()).filter(|a| !exclude.contains(a)).collect(),
                    };
                    for a in &istenen {
                        let Some((_, kaynak)) = adaylar.iter().find(|(x, _)| x == a) else { continue };
                        let mut alan = match kaynak {
                            None => Alan {
                                tip: Some("int".into()),
                                zorunlu: false,
                                belirsiz: false,
                                kurallar: vec!["IsInt()".into()],
                                satir: ms.satir,
                                kalitimdan: None,
                                degerler: vec![],
                                ic: None,
                            },
                            Some((c, sat, d)) => self.model_alani(*d, c, *sat, form, &mut notlar, a),
                        };
                        let salt = kaynak.is_none() || kaynak.as_ref().is_some_and(|(c, _, d)| model_alani_salt_okunur(c, &self.alan_turu(*d, &c.ad))) || salt_okunur.contains(a);
                        let iliski = kaynak.as_ref().is_some_and(|(c, _, d)| matches!(self.alan_turu(*d, &c.ad).as_str(), "ForeignKey" | "OneToOneField" | "ManyToManyField"));
                        let salt = salt || (iliski && derinlik_ayari > 0);
                        ek_kwargs_uygula(&ek_ayarlar, a, &mut alan);
                        let (salt, yalniz_yaz) = ek_okuma_yazma(&ek_ayarlar, a, salt);
                        if girdi && salt || !girdi && yalniz_yaz {
                            continue;
                        }
                        if !girdi {
                            alan.zorunlu = true;
                        }
                        alanlar.insert(a.clone(), alan);
                    }
                }
                None => {
                    tam = false;
                    notlar.push(format!("{}: Meta.model çözülemedi; modelden gelen alanlar eksik", s.ad));
                }
            }
        }
        // Tanımlanan alanlar (tabandan alt sınıfa: alt sınıf ezer)
        for (d2, s2) in z.siniflar.iter().rev() {
            for a in &s2.alanlar {
                let Some(c) = a.varsayilan.as_deref().and_then(cagri_ayir) else { continue };
                let Some(tanim) = self.bildirilen_alan(*d2, &c, a.satir, girdi, derinlik, &mut notlar, &a.ad) else { continue };
                let (alan, salt, yalniz_yaz, gizli) = tanim;
                // Model tabanlıda `fields` listesinde olmayan bildirilen alan da serializer'a girer (DRF bunu ister)
                alanlar.remove(&a.ad);
                if (girdi && (salt || gizli)) || (!girdi && (yalniz_yaz || gizli)) {
                    continue;
                }
                let mut alan = alan;
                ek_kwargs_uygula(&ek_ayarlar, &a.ad, &mut alan);
                if !girdi {
                    alan.zorunlu = true;
                }
                alanlar.insert(a.ad.clone(), alan);
            }
        }
        if kismi {
            for a in alanlar.values_mut() {
                a.zorunlu = false;
            }
        }
        // Nesne düzeyi doğrulama: validate(), validate_<alan>(), clean(), clean_<alan>()
        if girdi {
            for (_, s2) in &z.siniflar {
                for m in &s2.metotlar {
                    if m.ad == "validate" || m.ad == "clean" {
                        notlar.push(format!("nesne düzeyi doğrulama ({}.{}) önizlemede çalıştırılamaz", s2.ad, m.ad));
                    } else if let Some(alan) = m.ad.strip_prefix("validate_").or_else(|| m.ad.strip_prefix("clean_")) {
                        notlar.push(format!("'{alan}' alanının doğrulaması ({}.{}) önizlemede çalıştırılamaz", s2.ad, m.ad));
                    }
                }
            }
            if meta_deger("validators").is_some() {
                notlar.push(format!("{}: Meta.validators (ör. birlikte benzersizlik) önizlemede çalıştırılamaz", s.ad));
            }
        }
        if !tam {
            notlar.push(format!("{}: Meta.fields koddan çözülemedi; alan listesi eksik olabilir", s.ad));
        }
        Some((Sema { dto: s.ad.clone(), dosya: Some(self.goreli(sdi)), bulunamadi: false, kalitim: s.tabanlar.first().cloned(), alanlar, adaylar: vec![], dizi: false }, notlar))
    }

    /// Modelin alanından serializer / form alanı (DRF ModelSerializer kuralları: varsayılanı, blank ya da null olan alan
    /// isteğe bağlıdır)
    fn model_alani(&self, di: usize, c: &MetinCagri, satir: usize, form: bool, notlar: &mut Vec<String>, ad: &str) -> Alan {
        let tur = self.alan_turu(di, &c.ad);
        let mut alan = alan_tipi(&tur, c, satir);
        let bos = c.anahtar("blank").map(str::trim) == Some("True");
        let null = c.anahtar("null").map(str::trim) == Some("True");
        let varsayilan = c.anahtar("default").is_some();
        alan.zorunlu = !(bos || varsayilan || (!form && null));
        if null {
            alan.kurallar.push("nullable()".into());
        }
        if !bos && alan.tip.as_deref() == Some("string") && !alan.kurallar.iter().any(|k| k.starts_with("Is") && k != "IsString()") {
            alan.kurallar.push("IsNotEmpty()".into());
        }
        if let Some(ch) = c.anahtar("choices") {
            if let Some(d) = self.secenekler(di, ch) {
                alan.degerler = d;
                alan.kurallar.push(format!("IsIn([{}])", alan.degerler.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
            }
        }
        if c.anahtar("unique").map(str::trim) == Some("True") {
            notlar.push(format!("'{ad}' benzersiz olmalı: veritabanında denetlenir, önizlemede sınanmaz"));
        }
        if c.anahtar("validators").is_some() {
            notlar.push(format!("'{ad}' alanının doğrulayıcıları (validators) önizlemede çalıştırılamaz"));
        }
        if matches!(tur.as_str(), "FileField" | "ImageField") {
            notlar.push(format!("dosya doğrulaması kontrol edilemedi: '{ad}' alanı ({tur}); dosyanın varlığı, türü ve içeriği önizlemede sınanmaz"));
        }
        alan
    }

    /// `choices=` değerleri: `[("a", "A"), ...]`, aynı modüldeki liste değişkeni ya da `Durum.choices` (TextChoices)
    fn secenekler(&self, di: usize, ifade: &str) -> Option<Vec<String>> {
        let t = ifade.trim();
        if let Some(ogeler) = liste_ogeleri(t) {
            // Öğe (değer, etiket) demeti ya da yalnızca değer (DRF ChoiceField(choices=["a", "b"]))
            let v: Option<Vec<String>> = ogeler
                .iter()
                .map(|(_, o)| liste_ogeleri(o).and_then(|ic| ic.first().map(|x| x.1.clone())).unwrap_or_else(|| o.clone()))
                .map(|x| metin_sabiti(&x).or_else(|| sayi_mi(&x)))
                .collect();
            return v.filter(|v| !v.is_empty());
        }
        if let Some(sinif) = t.strip_suffix(".choices") {
            let (_, s) = self.b.ifade(di, sinif).and_then(|y| self.b.sinif(&y))?;
            let v: Vec<String> = s
                .alanlar
                .iter()
                .filter(|a| a.ad.chars().next().is_some_and(char::is_uppercase))
                .filter_map(|a| a.varsayilan.as_deref())
                .filter_map(|v| {
                    let ilk = parcalar(v, ',').into_iter().next()?;
                    let ilk = parantezsiz(&ilk).to_string();
                    let ilk = parcalar(&ilk, ',').into_iter().next().unwrap_or(ilk);
                    metin_sabiti(&ilk).or_else(|| sayi_mi(&ilk))
                })
                .collect();
            return (!v.is_empty()).then_some(v);
        }
        let y = self.b.ifade(di, t)?;
        let (atama, _) = self.b.atama(&y)?;
        let Yer::Ad(adi, _) = y else { return None };
        if atama.trim() == t {
            return None;
        }
        self.secenekler(adi, atama)
    }

    /// Serializer'da ya da formda bildirilen alan: (alan, salt okunur, yalnız yazılır, gizli). Alan değilse None
    #[allow(clippy::too_many_arguments)]
    fn bildirilen_alan(&self, di: usize, c: &MetinCagri, satir: usize, girdi: bool, derinlik: usize, notlar: &mut Vec<String>, ad: &str) -> Option<(Alan, bool, bool, bool)> {
        let tur = self.alan_turu(di, &c.ad);
        let anahtar = |k: &str| c.anahtar(k).map(str::trim);
        let dogru = |k: &str| anahtar(k) == Some("True");
        // İç içe serializer: ProfilSerializer(many=True, read_only=True)
        let ic_sinif = self.b.ifade(di, c.ad.trim()).and_then(|y| self.b.sinif(&y)).filter(|(d2, s2)| {
            let z = Zincir::kur(&self.b, *d2, s2);
            z.dis_var(&["Serializer", "ModelSerializer", "HyperlinkedModelSerializer", "Form", "ModelForm"])
        });
        let (mut alan, salt, yaz, gizli) = if let Some((d2, s2)) = ic_sinif {
            let coklu = dogru("many");
            let ic = (derinlik < 3).then(|| self.sema(d2, &s2.ad, girdi, false, derinlik + 1)).flatten();
            let mut a = Alan {
                tip: Some(if coklu { "array" } else { "object" }.into()),
                zorunlu: true,
                belirsiz: false,
                kurallar: vec![if coklu { "IsArray()".into() } else { "ValidateNested()".into() }],
                satir,
                kalitimdan: None,
                degerler: vec![],
                ic: None,
            };
            if let Some((sema, n)) = ic {
                a.ic = Some(sema.alanlar);
                notlar.extend(n);
            }
            (a, dogru("read_only"), dogru("write_only"), false)
        } else {
            if !(tur.ends_with("Field") || tur.ends_with("RelatedField") || matches!(tur.as_str(), "ListSerializer" | "Field")) {
                return None;
            }
            let salt_tur = matches!(tur.as_str(), "SerializerMethodField" | "ReadOnlyField" | "HyperlinkedIdentityField");
            let gizli = tur == "HiddenField";
            (alan_tipi(&tur, c, satir), salt_tur || dogru("read_only") || dogru("disabled"), dogru("write_only"), gizli)
        };
        // Zorunluluk: required açıkça verilmişse o; varsayılanı olan alan isteğe bağlı
        alan.zorunlu = match anahtar("required") {
            Some("False") => false,
            Some("True") => true,
            _ => c.anahtar("default").is_none(),
        };
        if dogru("allow_null") {
            alan.kurallar.push("nullable()".into());
        }
        if alan.tip.as_deref() == Some("string") && !dogru("allow_blank") && !alan.kurallar.iter().any(|k| k == "IsNotEmpty()") && !matches!(tur.as_str(), "SlugRelatedField") {
            alan.kurallar.push("IsNotEmpty()".into());
        }
        if let Some(ch) = c.anahtar("choices") {
            if let Some(d) = self.secenekler(di, ch) {
                alan.kurallar.push(format!("IsIn([{}])", d.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ")));
                alan.degerler = d;
                alan.tip = Some("enum".into());
            }
        }
        if girdi && c.anahtar("validators").is_some() {
            notlar.push(format!("'{ad}' alanının doğrulayıcıları (validators) önizlemede çalıştırılamaz"));
        }
        if girdi && matches!(tur.as_str(), "FileField" | "ImageField") {
            notlar.push(format!("dosya doğrulaması kontrol edilemedi: '{ad}' alanı ({tur}); dosyanın varlığı, türü ve içeriği önizlemede sınanmaz"));
        }
        Some((alan, salt, yaz, gizli))
    }

    // ---- Veri bağları ----

    /// Sınıf görünümünün modeli: `queryset = Kitap.objects...`, `model = Kitap`, serializer'ın Meta.model'i
    fn gorunum_modeli(&self, z: &Zincir) -> Option<String> {
        if let Some((d, a)) = z.oznitelik("queryset").or_else(|| z.oznitelik("model")) {
            let v = a.varsayilan.as_deref()?;
            let ilk = v.split(['.', '(']).next()?.trim();
            if let Some((_, s)) = self.b.ifade(d, ilk).and_then(|y| self.b.sinif(&y)) {
                if self.modeller.contains_key(&s.ad) {
                    return Some(s.ad.clone());
                }
            }
        }
        let (d, a) = z.oznitelik("serializer_class")?;
        self.serializer_modeli(d, a.varsayilan.as_deref()?)
    }

    fn serializer_modeli(&self, di: usize, ifade: &str) -> Option<String> {
        let (sdi, s) = self.b.ifade(di, ifade.trim()).and_then(|y| self.b.sinif(&y))?;
        let z = Zincir::kur(&self.b, sdi, s);
        let (mdi, meta) = z.ic_sinif("Meta")?;
        let m = meta.alanlar.iter().find(|a| a.ad == "model")?.varsayilan.as_deref()?;
        let (_, ms) = self.b.ifade(mdi, m.trim()).and_then(|y| self.b.sinif(&y))?;
        self.modeller.contains_key(&ms.ad).then(|| ms.ad.clone())
    }

    /// Gövdeden (ve 3 kat proje çağrısından) Django ORM kullanımı [Yorum]: `M.objects.<okuma>` okur;
    /// `.create / .update / .delete / .bulk_create / get_or_create / update_or_create` yazar; `M(...)` + `.save()`,
    /// alınan nesnenin `.save()` / `.delete()`'i yazar; `get_object_or_404(M, ...)` okur; `x.save()` serializer'ı Meta.model'e
    /// yazar; `.raw(` / `cursor.execute(` listeyi eksik yapar
    fn orm(&self, z: Option<&Zincir>, baslangic: Vec<(usize, &'a Fonksiyon)>, bag: &mut VeriBag) {
        static D: std::sync::OnceLock<[regex::Regex; 7]> = std::sync::OnceLock::new();
        let [objects, kisayol, olusturma, degisken, kaydet, ham, cagri] = D.get_or_init(|| {
            [
                regex::Regex::new(r"\b([A-Z]\w*)\.(?:objects|_default_manager|all_objects)\b([^\n]*)").unwrap(),
                regex::Regex::new(r"\bget_(?:object|list)_or_404\(\s*([A-Z]\w*)").unwrap(),
                regex::Regex::new(r"\b([A-Z]\w*)\(").unwrap(),
                regex::Regex::new(r"\b(\w+)\s*=\s*(?:get_object_or_404\(\s*([A-Z]\w*)|([A-Z]\w*)\.objects\b|([A-Z]\w*)\()").unwrap(),
                regex::Regex::new(r"\b(\w+)\.(save|delete)\(").unwrap(),
                regex::Regex::new(r"\.raw\(|cursor\(\)|\.execute\(").unwrap(),
                regex::Regex::new(r"(?:^|[^\w.])((?:self\.)?[A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)\(").unwrap(),
            ]
        });
        const YAZMA: [&str; 9] = [".create(", ".update(", ".delete(", ".bulk_create(", ".bulk_update(", ".get_or_create(", ".update_or_create(", ".add(", ".remove("];
        let model = |m: &str| self.modeller.contains_key(m).then(|| m.to_string());
        let mut yigin: Vec<(usize, &Fonksiyon, usize)> = baslangic.into_iter().map(|(d, f)| (d, f, 0)).collect();
        let mut gorulen = HashSet::new();
        while let Some((di, f, derinlik)) = yigin.pop() {
            if !gorulen.insert((di, f.ad.clone(), f.satir)) || gorulen.len() > 64 {
                continue;
            }
            let t = &f.govde_metni;
            for c in objects.captures_iter(t) {
                let Some(m) = model(&c[1]) else { continue };
                if YAZMA.iter().any(|y| c[2].contains(y)) {
                    bag.yazar.push(m.clone());
                    if c[2].contains(".get_or_create(") || c[2].contains(".update(") {
                        bag.okur.push(m);
                    }
                } else {
                    bag.okur.push(m);
                }
            }
            bag.okur.extend(kisayol.captures_iter(t).filter_map(|c| model(&c[1])));
            // Değişken -> model: get_object_or_404(M), M.objects..., M(...)
            let mut degiskenler: HashMap<String, String> = HashMap::new();
            for c in degisken.captures_iter(t) {
                if let Some(m) = c.get(2).or(c.get(3)).or(c.get(4)).and_then(|x| model(x.as_str())) {
                    degiskenler.insert(c[1].to_string(), m);
                }
            }
            for c in kaydet.captures_iter(t) {
                let v = &c[1];
                if let Some(m) = degiskenler.get(v) {
                    bag.yazar.push(m.clone());
                    continue;
                }
                // serializer.save(): serializer'ın modeli
                if &c[2] == "save" {
                    if let Some(sm) = self.degisken_serializer_modeli(di, t, v) {
                        bag.yazar.push(sm);
                    }
                }
            }
            if t.contains(".save(") {
                bag.yazar.extend(olusturma.captures_iter(t).filter_map(|c| model(&c[1])).filter(|m| degiskenler.values().any(|x| x == m)));
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
                    if matches!(m, "get_object" | "get_queryset") {
                        if let Some(gm) = z.and_then(|z| self.gorunum_modeli(z)) {
                            bag.okur.push(gm);
                        }
                    }
                    continue;
                }
                if let Some((d2, g)) = self.b.ifade(di, ad).and_then(|y| self.b.fonksiyon(&y)) {
                    yigin.push((d2, g, derinlik + 1));
                }
            }
        }
    }

    /// Gövdedeki `x = XSerializer(...)` atamasının serializer modeli
    fn degisken_serializer_modeli(&self, di: usize, govde: &str, degisken: &str) -> Option<String> {
        let desen = regex::Regex::new(&format!(r"\b{}\s*=\s*([A-Za-z_][\w.]*)\(", regex::escape(degisken))).ok()?;
        let ad = desen.captures(govde)?[1].to_string();
        self.serializer_modeli(di, &ad)
    }

    // ---- Sinyaller ----

    /// Sinyal dinleyicileri: `@receiver(post_save, sender=Kitap)` ve `post_save.connect(f, sender=Kitap)`. Dönen:
    /// "event:post_save:Kitap" -> {tur, ad, dinleyiciler, tetikler, silinen}
    fn sinyaller(&self) -> BTreeMap<String, Value> {
        let mut v: BTreeMap<String, Value> = BTreeMap::new();
        let mut ekle = |sinyal: &str, gonderen: Option<String>, fdi: usize, f: &Fonksiyon| {
            let ad = match &gonderen {
                Some(g) => format!("{sinyal}:{g}"),
                None => sinyal.to_string(),
            };
            let dugum = v.entry(format!("event:{ad}")).or_insert_with(|| json!({ "tur": "event", "ad": ad, "dinleyiciler": [], "tetikler": [], "silinen": [] }));
            if let Some(l) = dugum["dinleyiciler"].as_array_mut() {
                l.push(json!({ "fonksiyon": f.ad, "kanit": format!("{}:{}", guvenli::goreli(self.kok, &self.dosyalar()[fdi].yol), f.satir) }));
            }
            let mut bag = VeriBag::default();
            self.orm(None, vec![(fdi, f)], &mut bag);
            bag.yazar.sort();
            bag.yazar.dedup();
            if let Some(l) = dugum["silinen"].as_array_mut() {
                if f.govde_metni.contains(".delete(") {
                    for y in bag.yazar {
                        if !l.contains(&json!(y)) {
                            l.push(json!(y));
                        }
                    }
                }
            }
        };
        for (di, d) in self.dosyalar().iter().enumerate() {
            for f in &d.fonksiyonlar {
                for dek in f.dekoratorler.iter().filter(|x| x.ad.rsplit('.').next() == Some("receiver")) {
                    let gonderen = dek.anahtarli.iter().find(|(k, _)| k == "sender").and_then(|(_, s)| self.gonderen_adi(di, s));
                    let sinyaller: Vec<String> = match dek.argumanlar.first() {
                        Some(a) if a.trim_start().starts_with('[') => liste_ogeleri(a).unwrap_or_default().into_iter().map(|(_, x)| x).collect(),
                        Some(a) => vec![a.clone()],
                        None => vec![],
                    };
                    for s in sinyaller {
                        ekle(s.rsplit('.').next().unwrap_or(&s).trim(), gonderen.clone(), di, f);
                    }
                }
            }
            let cagrilar = d.cagrilar.iter().chain(d.fonksiyonlar.iter().flat_map(|f| f.ic_cagrilar.iter()));
            for c in cagrilar {
                let Some(sinyal) = c.ad.strip_suffix(".connect") else { continue };
                let Some(hedef) = c.argumanlar.first().or_else(|| c.anahtarli.iter().find(|(k, _)| k == "receiver").map(|(_, v)| v)) else { continue };
                let Some((fdi, f)) = self.b.ifade(di, hedef).and_then(|y| self.b.fonksiyon(&y)) else { continue };
                let gonderen = c.anahtarli.iter().find(|(k, _)| k == "sender").and_then(|(_, s)| self.gonderen_adi(di, s));
                ekle(sinyal.rsplit('.').next().unwrap_or(sinyal), gonderen, fdi, f);
            }
        }
        v
    }

    fn gonderen_adi(&self, di: usize, s: &str) -> Option<String> {
        let t = s.trim();
        if let Some(m) = metin_sabiti(t) {
            return Some(m.rsplit('.').next().unwrap_or(&m).to_string());
        }
        match self.b.ifade(di, t)? {
            y @ Yer::Ad(..) => self.b.sinif(&y).map(|(_, s)| s.ad.clone()),
            Yer::Dis(_, a) => Some(a.rsplit('.').next().unwrap_or(&a).to_string()),
            _ => None,
        }
    }
}

/// Dekoratör çağrısının metni: `login_required`, `permission_required('a.b')`
fn dekorator_metni(dek: &crate::python::Cagri) -> String {
    if dek.argumanlar.is_empty() && dek.anahtarli.is_empty() {
        return dek.ad.clone();
    }
    let mut p: Vec<String> = dek.argumanlar.clone();
    p.extend(dek.anahtarli.iter().map(|(k, v)| format!("{k}={v}")));
    format!("{}({})", dek.ad, p.join(", "))
}

/// Model alanı serializer'da salt okunur mu: otomatik birincil anahtar, `auto_now`, `editable=False`
fn model_alani_salt_okunur(c: &MetinCagri, tur: &str) -> bool {
    tur.ends_with("AutoField")
        || c.anahtar("auto_now").map(str::trim) == Some("True")
        || c.anahtar("auto_now_add").map(str::trim) == Some("True")
        || c.anahtar("editable").map(str::trim) == Some("False")
}

/// `extra_kwargs = {"parola": {"write_only": True, "required": False}}`
fn ek_kwargs_uygula(ek: &str, ad: &str, alan: &mut Alan) {
    let Some(ayar) = sozluk_degeri(ek, ad) else { return };
    match sozluk_degeri(&ayar, "required").as_deref().map(str::trim) {
        Some("False") => alan.zorunlu = false,
        Some("True") => alan.zorunlu = true,
        _ => {}
    }
    for (k, kural) in [("max_length", "MaxLength"), ("min_length", "MinLength"), ("min_value", "Min"), ("max_value", "Max")] {
        if let Some(n) = sozluk_degeri(&ayar, k).filter(|n| n.trim().parse::<f64>().is_ok()) {
            alan.kurallar.push(format!("{kural}({})", n.trim()));
        }
    }
    if sozluk_degeri(&ayar, "allow_blank").as_deref().map(str::trim) == Some("True") {
        alan.kurallar.retain(|k| k != "IsNotEmpty()");
    }
}

/// extra_kwargs'taki read_only / write_only: (salt okunur, yalnız yazılır)
fn ek_okuma_yazma(ek: &str, ad: &str, salt: bool) -> (bool, bool) {
    let Some(ayar) = sozluk_degeri(ek, ad) else { return (salt, false) };
    let d = |k: &str| sozluk_degeri(&ayar, k).map(|v| v.trim() == "True");
    (d("read_only").unwrap_or(salt), d("write_only").unwrap_or(false))
}

/// Sözlük metninde üst düzey anahtarın değeri: `{"A": 1, 'B': [..]}` ya da `dict(A=1)`
pub(crate) fn sozluk_degeri(sozluk: &str, anahtar: &str) -> Option<String> {
    let t = sozluk.trim();
    if let Some(c) = cagri_ayir(t).filter(|c| c.ad == "dict") {
        return c.anahtar(anahtar).map(String::from);
    }
    let ic = t.strip_prefix('{')?.strip_suffix('}')?;
    for (_, oge) in bol(ic, ',') {
        let p = bol(&oge, ':');
        let Some((_, k)) = p.first() else { continue };
        if metin_sabiti(k).as_deref() == Some(anahtar) {
            let (o, _) = p.get(1)?;
            return Some(oge[*o..].trim().to_string());
        }
    }
    None
}

pub(crate) fn sayi_mi(s: &str) -> Option<String> {
    let t = s.trim();
    t.parse::<f64>().ok().map(|_| t.to_string())
}

/// Dıştaki tek parantez çifti (bütün ifadeyi sarıyorsa) atılır
pub(crate) fn parantezsiz(s: &str) -> &str {
    let t = s.trim();
    if let Some(ic) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        let mut d = 0i32;
        for c in ic.chars() {
            match c {
                '(' => d += 1,
                ')' => {
                    d -= 1;
                    if d < 0 {
                        return t;
                    }
                }
                _ => {}
            }
        }
        return ic.trim();
    }
    t
}

pub(crate) fn kisalt(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > 80 {
        format!("{}…", t.chars().take(80).collect::<String>())
    } else {
        t
    }
}

/// `{"get": "list", "post": "create"}` -> [("GET", "list"), ("POST", "create")]
fn eylem_eslemesi(s: &str) -> Option<Vec<(String, String)>> {
    let t = s.trim();
    let ic = t.strip_prefix('{')?.strip_suffix('}')?;
    let mut v = vec![];
    for oge in parcalar(ic, ',') {
        let p = parcalar(&oge, ':');
        let (Some(y), Some(e)) = (p.first().and_then(|x| metin_sabiti(x)), p.get(1).and_then(|x| metin_sabiti(x))) else { continue };
        v.push((y.to_uppercase(), e));
    }
    (!v.is_empty()).then_some(v)
}

/// Yerleşik (kütüphane) izin sınıfının anlamı
fn yerlesik_izin(modul: &str, ad: &str, yontem: &str, model: Option<&str>) -> Karar {
    let model_izni = |eylem: &str| -> Vec<String> { model.map(|m| vec![format!("{eylem}_{}", m.to_lowercase())]).unwrap_or_default() };
    let model_rolu = || match yontem {
        "POST" => model_izni("add"),
        "PUT" | "PATCH" => model_izni("change"),
        "DELETE" => model_izni("delete"),
        _ => vec![],
    };
    let drf = modul.is_empty() || modul.starts_with("rest_framework");
    match ad {
        "AllowAny" if drf => Karar::yeni(Giris::Acik, true),
        "IsAuthenticated" if drf => Karar::yeni(Giris::Gerekli, true),
        "IsAdminUser" if drf => Karar::yeni(Giris::Gerekli, true).rollu(vec!["staff".into()], true),
        "IsAuthenticatedOrReadOnly" if drf => {
            if guvenli_yontem(yontem) {
                Karar::yeni(Giris::Acik, true)
            } else {
                Karar::yeni(Giris::Gerekli, true)
            }
        }
        "DjangoModelPermissions" | "DjangoObjectPermissions" if drf => Karar::yeni(Giris::Gerekli, true).rollu(model_rolu(), false),
        "DjangoModelPermissionsOrAnonReadOnly" if drf => {
            if guvenli_yontem(yontem) {
                Karar::yeni(Giris::Acik, true)
            } else {
                Karar::yeni(Giris::Gerekli, true).rollu(model_rolu(), false)
            }
        }
        // django-oauth-toolkit ve djangorestframework-api-key: belirteç / anahtar ister
        "TokenHasScope" | "TokenHasReadWriteScope" | "TokenHasResourceScope" | "TokenMatchesOASRequirements" | "IsAuthenticatedOrTokenHasScope" | "HasAPIKey" => {
            Karar::yeni(Giris::Gerekli, true)
        }
        _ => Karar::yeni(Giris::Bilinmeyen, false),
    }
}

/// Alan tipinden (DRF serializer, Django form ya da model alanı adı) Sezgi alanı
fn alan_tipi(tur: &str, c: &MetinCagri, satir: usize) -> Alan {
    let mut kurallar: Vec<String> = vec![];
    let sayi = |k: &str| c.anahtar(k).map(str::trim).filter(|n| n.parse::<f64>().is_ok()).map(String::from);
    let tip: Option<&str> = match tur {
        "CharField" | "TextField" | "SlugField" | "RegexField" | "IPAddressField" | "GenericIPAddressField" | "FilePathField" | "PasswordField" => {
            kurallar.push("IsString()".into());
            if tur == "SlugField" {
                kurallar.push("Matches(/^[-a-zA-Z0-9_]+$/)".into());
            }
            if tur == "RegexField" {
                if let Some(d) = c.arg(0).or(c.anahtar("regex")).and_then(metin_sabiti) {
                    kurallar.push(format!("Matches(/{d}/)"));
                }
            }
            Some("string")
        }
        "EmailField" => {
            kurallar.push("IsEmail()".into());
            Some("string")
        }
        "URLField" | "HyperlinkedRelatedField" | "HyperlinkedIdentityField" => {
            kurallar.push("IsUrl()".into());
            Some("string")
        }
        "UUIDField" => {
            kurallar.push("IsUUID()".into());
            Some("string")
        }
        "IntegerField"
        | "BigIntegerField"
        | "SmallIntegerField"
        | "PositiveIntegerField"
        | "PositiveSmallIntegerField"
        | "PositiveBigIntegerField"
        | "AutoField"
        | "BigAutoField" => {
            kurallar.push("IsInt()".into());
            if tur.starts_with("Positive") {
                kurallar.push("Min(0)".into());
            }
            Some("int")
        }
        "FloatField" | "DecimalField" => {
            kurallar.push("IsNumber()".into());
            Some("number")
        }
        "BooleanField" | "NullBooleanField" => {
            kurallar.push("IsBoolean()".into());
            Some("boolean")
        }
        "DateField" | "DateTimeField" | "TimeField" | "DurationField" => {
            kurallar.push("IsDateString()".into());
            Some("date")
        }
        "ChoiceField" | "TypedChoiceField" => Some("enum"),
        "MultipleChoiceField" | "ListField" | "ManyToManyField" | "ModelMultipleChoiceField" | "ArrayField" => {
            kurallar.push("IsArray()".into());
            Some("array")
        }
        "DictField" | "HStoreField" => {
            kurallar.push("IsObject()".into());
            Some("object")
        }
        "SlugRelatedField" | "StringRelatedField" => Some("string"),
        "FileField" | "ImageField" => Some("string"),
        // İlişki (birincil anahtar), JSON ve tanınmayan alanların tipi bilinmez
        _ => None,
    };
    for (k, kural) in [("max_length", "MaxLength"), ("min_length", "MinLength"), ("min_value", "Min"), ("max_value", "Max")] {
        if let Some(n) = sayi(k) {
            kurallar.push(format!("{kural}({n})"));
        }
    }
    Alan { tip: tip.map(String::from), zorunlu: true, belirsiz: tip.is_none(), kurallar, satir, kalitimdan: None, degerler: vec![], ic: None }
}

/// Düzenli ifade deseninden yol: adlı gruplar `:ad`, adsız gruplar `:argN`, `^` ve `$` atılır, `/?` (isteğe bağlı eğik
/// çizgi) çizgisiz yazılır. Dönen: (yol, parametreler, tamamı çevrildi mi)
fn duzenli_yol(r: &str) -> (String, Vec<(String, String)>, bool) {
    let mut s = r.trim();
    s = s.strip_prefix('^').unwrap_or(s);
    s = s.strip_suffix('$').or_else(|| s.strip_suffix("\\Z")).unwrap_or(s);
    let b: Vec<char> = s.chars().collect();
    let (mut out, mut params, mut tam, mut adsiz) = (String::new(), vec![], true, 0);
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            '(' => {
                // Grubun sonu
                let mut d = 0;
                let mut j = i;
                while j < b.len() {
                    match b[j] {
                        '\\' => j += 1,
                        '(' => d += 1,
                        ')' => {
                            d -= 1;
                            if d == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                let ic: String = b[i + 1..j.min(b.len())].iter().collect();
                if let Some(adli) = ic.strip_prefix("?P<") {
                    let ad = adli.split('>').next().unwrap_or("").to_string();
                    let desen = adli.split_once('>').map(|x| x.1).unwrap_or("");
                    let tip = if desen == "[0-9]+" || desen == "\\d+" || desen == "[\\d]+" { "int" } else { "str" };
                    out.push(':');
                    out.push_str(&ad);
                    params.push((ad, tip.to_string()));
                } else if ic.starts_with("?:") || ic.starts_with("?=") || ic.starts_with("?!") {
                    tam = false;
                } else {
                    adsiz += 1;
                    let ad = format!("arg{adsiz}");
                    out.push(':');
                    out.push_str(&ad);
                    params.push((ad, "str".into()));
                }
                i = j + 1;
                // Gruptan sonra niceleyici
                if i < b.len() && matches!(b[i], '?' | '*' | '+') {
                    tam = false;
                    i += 1;
                }
                continue;
            }
            '\\' if i + 1 < b.len() => {
                let n = b[i + 1];
                if n.is_ascii_alphanumeric() {
                    tam = false;
                } else {
                    out.push(n);
                }
                i += 2;
                continue;
            }
            '/' if b.get(i + 1) == Some(&'?') => {
                // İsteğe bağlı eğik çizgi: çizgisiz biçim
                i += 2;
                continue;
            }
            '?' | '*' | '+' | '[' | ']' | '{' | '}' | '|' | '.' => {
                tam = false;
                if c == '[' {
                    while i < b.len() && b[i] != ']' {
                        i += 1;
                    }
                }
                if c == '{' {
                    while i < b.len() && b[i] != '}' {
                        i += 1;
                    }
                }
                i += 1;
                continue;
            }
            _ => out.push(c),
        }
        i += 1;
    }
    (out, params, tam)
}

/// Django desen birleştirmesi: önek ve parça doğrudan birleşir (Django yolları eğik çizgisiz başlar); `//` olmaz
fn yol_birlestir_ham(onek: &str, parca: &str) -> String {
    let p = parca.trim_start_matches('/');
    let mut s = format!("{}{}", if onek.is_empty() { "/" } else { onek }, p);
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '/' && out.ends_with('/') {
            continue;
        }
        out.push(c);
    }
    out
}

/// Görünüm sınıfının işlem adındaki eki atılır: KitapViewSet -> Kitap
fn kontrolcu_adi(s: &str) -> String {
    for ek in ["ViewSet", "APIView", "View", "Viewset", "Api"] {
        if let Some(k) = s.strip_suffix(ek).filter(|k| !k.is_empty()) {
            return k.to_string();
        }
    }
    s.to_string()
}

/// Fonksiyon görünümünün kontrolcü adı: modül views / api / urls ise bir üst paketin adı
fn modul_kontrolcusu(d: &Dosya) -> String {
    let p: Vec<&str> = d.modul.split('.').collect();
    let son = p.last().copied().unwrap_or("");
    let ad = if matches!(son, "views" | "api" | "urls" | "viewsets" | "endpoints") && p.len() > 1 { p[p.len() - 2] } else { son };
    pascal(ad)
}

/// Gövdedeki serializer kullanımı: `XSerializer(data=...)`, `self.get_serializer(data=...)` (`serializer_class`),
/// `self.serializer_class(data=...)`. Dönen: (serializer ifadesi, data argümanı, kısmi mi)
fn govdedeki_serializer(t: &str) -> Option<(String, String, bool)> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r"\b((?:self\.)?[A-Za-z_][\w.]*)\(([^()]*(?:\([^()]*\)[^()]*)*)\)").unwrap());
    for c in re.captures_iter(t) {
        let ad = &c[1];
        let arglar = &c[2];
        if !arglar.contains("data=") && !arglar.contains("data =") {
            continue;
        }
        let son = ad.rsplit('.').next().unwrap_or(ad);
        if !(son.ends_with("Serializer") || son == "get_serializer" || son == "serializer_class" || son.ends_with("Form")) {
            continue;
        }
        let mc = cagri_ayir(&format!("f({arglar})"))?;
        let veri = mc.anahtar("data").map(String::from).unwrap_or_default();
        let kismi = mc.anahtar("partial").map(str::trim) == Some("True");
        return Some((ad.to_string(), veri, kismi));
    }
    None
}

/// Gövdede adı geçen serializer ve form sınıfları (`get_serializer_class` adayları)
fn serializer_adlari(t: &str) -> Vec<String> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r"\b([A-Z]\w*(?:Serializer|Form))\b").unwrap());
    let mut v: Vec<String> = re.captures_iter(t).map(|c| c[1].to_string()).collect();
    v.dedup();
    v
}

/// `x = request.data.get("anahtar", {})` / `request.data["anahtar"]`: zarf anahtarı
fn zarf_anahtari(t: &str, degisken: &str) -> Option<String> {
    let degisken = degisken.trim();
    if let Some(a) = istek_anahtari(degisken) {
        return Some(a);
    }
    let desen = regex::Regex::new(&format!(r"\b{}\s*=\s*([^\n]+)", regex::escape(degisken))).ok()?;
    let sag = desen.captures(t)?[1].trim().to_string();
    istek_anahtari(&sag)
}

/// Gövdede `x = request.data.get("anahtar", {})` ataması: zarf anahtarı (ilk)
fn zarf_ara(t: &str) -> Option<String> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r#"\b\w+\s*=\s*(?:self\.)?request\.data(?:\.get\(\s*|\[\s*)["']([\w-]+)["']"#).unwrap());
    re.captures(t).map(|c| c[1].to_string())
}

fn istek_anahtari(ifade: &str) -> Option<String> {
    static D: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = D.get_or_init(|| regex::Regex::new(r#"^(?:self\.)?request\.data(?:\.get\(\s*|\[\s*)["']([\w-]+)["']"#).unwrap());
    re.captures(ifade.trim()).map(|c| c[1].to_string())
}

/// Gövdede okunan istek alanları: `request.data["x"]` (zorunlu gibi), `request.data.get("x")`, `request.POST.get("x")`;
/// sorgu: `request.query_params.get("x")`, `request.GET.get("x")`. Dönen: (gövde [(ad, köşeli mi)], sorgu)
fn istek_alanlari(t: &str) -> (Vec<(String, bool)>, Vec<String>) {
    static D: std::sync::OnceLock<[regex::Regex; 2]> = std::sync::OnceLock::new();
    let [govde, sorgu] = D.get_or_init(|| {
        [
            regex::Regex::new(r#"request\.(?:data|POST)(\.get\(\s*|\[\s*)["']([\w-]+)["']"#).unwrap(),
            regex::Regex::new(r#"request\.(?:query_params|GET)(?:\.get\(\s*|\.getlist\(\s*|\[\s*)["']([\w-]+)["']"#).unwrap(),
        ]
    });
    let mut g: Vec<(String, bool)> = vec![];
    for c in govde.captures_iter(t) {
        if !g.iter().any(|x| x.0 == c[2]) {
            g.push((c[2].to_string(), c[1].starts_with('[')));
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

pub(crate) fn serbest_alan(satir: usize, zorunlu: bool) -> Alan {
    Alan { tip: None, zorunlu, belirsiz: true, kurallar: vec![], satir, kalitimdan: None, degerler: vec![], ic: None }
}

/// Model tanımlayan dosyalar (rapor tablo bağlarını bunlara göre sayar)
pub(crate) fn model_dosyalari(dosyalar: &[Dosya]) -> Vec<&Dosya> {
    let d = Django::yeni(Path::new(""), dosyalar);
    let mut v: Vec<usize> = d.modeller.values().map(|(di, _)| *di).collect();
    v.sort();
    v.dedup();
    v.into_iter().map(|i| &dosyalar[i]).collect()
}

/// İşlem planı: bir rotanın bir yöntemi
struct Islem<'a> {
    yontem: String,
    /// Anlamsal eylem: list, create, retrieve, update, partial_update, destroy ya da yöntem / eylem adı
    eylem: String,
    /// İşleyici (projede tanımlıysa)
    isleyici: Option<(usize, &'a Fonksiyon)>,
    /// `@action(...)` dekoratörü (varsa)
    aksiyon: Option<&'a crate::python::Cagri>,
}

pub fn kapilar(kok: &Path, dosyalar: &[Dosya]) -> (Vec<Kapi>, Value) {
    let mut dj = Django::yeni(kok, dosyalar);
    let ana_modul = dj.rotalari_topla();
    let (ara_zorunlu, ara_bilinmeyen) = dj.ara_katman();
    let sinyaller = dj.sinyaller();
    let mut kapilar: Vec<Kapi> = vec![];
    let mut denetlenemeyen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dna_haritasi: BTreeMap<String, Value> = BTreeMap::new();
    let mut kosullu_kapilar: Vec<String> = vec![];
    let mut kapi_routerlari: BTreeMap<String, String> = BTreeMap::new();
    let mut gorulen_idler: HashSet<String> = HashSet::new();
    let mut ek_disarida: Vec<(String, String)> = vec![];
    let b = &dj.b;
    let rotalar = std::mem::take(&mut dj.rotalar);
    let varsayilan_sayfalama = dj.drf_ayari("DEFAULT_PAGINATION_CLASS").map(|((_, v), _)| v);
    let ozel_cizici = dj.drf_ayari("DEFAULT_RENDERER_CLASSES").is_some_and(|((_, v), _)| metin_listesi(&v).is_some_and(|l| l.iter().any(|x| !x.starts_with("rest_framework."))));
    for r in &rotalar {
        // Rotanın işlemleri, görünüm türüne göre
        let mut islemler: Vec<Islem> = vec![];
        let mut notlar: Vec<String> = vec![];
        let (kontrolcu, dosya_di, sinif_satiri, z, drf, fonk): GorunumBilgisi;
        let mut as_view_kw: Vec<(String, String)> = vec![];
        match &r.hedef {
            Hedef::Fonksiyon(fdi, ad) => {
                let Some((fdi, f)) = b.fonksiyon(&Yer::Ad(*fdi, ad.clone())) else { continue };
                let api = f.dekoratorler.iter().find(|d| d.ad.rsplit('.').next() == Some("api_view"));
                let kisit = f.dekoratorler.iter().find_map(|d| match d.ad.rsplit('.').next().unwrap_or("") {
                    "require_http_methods" => d.argumanlar.first().and_then(|a| metin_listesi(a)),
                    "require_GET" => Some(vec!["GET".into()]),
                    "require_POST" => Some(vec!["POST".into()]),
                    "require_safe" => Some(vec!["GET".into()]),
                    // Projedeki yöntem süzen dekoratör: `@allow_methods("POST")`, gövdesi request.method'a bakıp 405 döner
                    _ => {
                        let yontemler: Vec<String> = d.argumanlar.iter().filter_map(|a| metin_sabiti(a)).map(|m| m.to_uppercase()).collect();
                        let hepsi_yontem = !yontemler.is_empty()
                            && yontemler.len() == d.argumanlar.len()
                            && yontemler.iter().all(|y| matches!(y.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"));
                        b.ifade(fdi, &d.ad)
                            .and_then(|y| b.fonksiyon(&y))
                            .filter(|(_, g)| hepsi_yontem && crate::py_ortak::yontem_suzen_govde(&g.govde_metni))
                            .map(|_| yontemler.clone())
                    }
                });
                // Dekoratör yoksa gövdenin başındaki koruma (`if request.method != "POST": ... 405`)
                let kisit = kisit.or_else(|| crate::py_ortak::govdedeki_yontem_kisiti(&f.govde_metni));
                let yontemler: Vec<String> = match (api, kisit) {
                    (Some(a), _) => a
                        .argumanlar
                        .first()
                        .or_else(|| a.anahtarli.iter().find(|(k, _)| k == "http_method_names").map(|(_, v)| v))
                        .and_then(|l| metin_listesi(l))
                        .unwrap_or_else(|| vec!["GET".into()]),
                    (None, Some(k)) => k,
                    (None, None) => {
                        let mut v = vec!["GET".to_string()];
                        for y in crate::py_ortak::govdedeki_yontemler(&f.govde_metni) {
                            if !v.contains(&y) {
                                v.push(y);
                            }
                        }
                        notlar.push("yöntem kısıtı yok: işleyici her HTTP yöntemini kabul eder; yöntemler gövdedeki request.method karşılaştırmalarından çıkarıldı".into());
                        v
                    }
                };
                for y in yontemler.iter().map(|y| y.to_uppercase()).filter(|y| !matches!(y.as_str(), "HEAD" | "OPTIONS")) {
                    islemler.push(Islem { yontem: y.clone(), eylem: f.ad.clone(), isleyici: Some((fdi, f)), aksiyon: None });
                }
                kontrolcu = modul_kontrolcusu(&dosyalar[fdi]);
                dosya_di = fdi;
                sinif_satiri = f.satir;
                z = None;
                drf = api.is_some();
                fonk = Some((fdi, f));
            }
            Hedef::Sinif(sdi, ad, eslesme, kw) => {
                let Some((sdi, s)) = b.sinif(&Yer::Ad(*sdi, ad.clone())) else { continue };
                let zz = Zincir::kur(b, sdi, s);
                as_view_kw = kw.clone();
                let viewset = zz.dis_var(&VIEWSET);
                let drf_mi = viewset || zz.dis_var(&APIVIEW);
                match eslesme {
                    Some(e) => {
                        for (y, eylem) in e {
                            let isleyici = zz.metot(eylem).map(|(d, f, _)| (d, f));
                            let aksiyon =
                                isleyici.and_then(|(_, f)| f.dekoratorler.iter().find(|d| matches!(d.ad.rsplit('.').next(), Some("action" | "detail_route" | "list_route"))));
                            // Varsayılan eylemi ezmeyen ViewSet'te perform_* kancası işleyicidir
                            let isleyici = isleyici.or_else(|| {
                                let kanca = match eylem.as_str() {
                                    "create" => "perform_create",
                                    "update" | "partial_update" => "perform_update",
                                    "destroy" => "perform_destroy",
                                    _ => "",
                                };
                                zz.metot(kanca).map(|(d, f, _)| (d, f))
                            });
                            // @x.mapping.post yöntemi
                            let isleyici = isleyici.or_else(|| {
                                zz.siniflar.iter().find_map(|(d, s2)| {
                                    s2.metotlar.iter().find(|m| m.dekoratorler.iter().any(|dk| dk.ad == format!("{eylem}.mapping.{}", y.to_lowercase()))).map(|m| (*d, m))
                                })
                            });
                            islemler.push(Islem { yontem: y.clone(), eylem: eylem.clone(), isleyici, aksiyon });
                        }
                    }
                    None => {
                        let mut ciftler: Vec<(String, String)> = vec![];
                        for ad in zz.dis_adlar() {
                            for (y, e) in taban_yontemleri(ad) {
                                if !ciftler.iter().any(|(y2, _)| y2 == y) {
                                    ciftler.push((y.to_string(), e.to_string()));
                                }
                            }
                        }
                        for y in ["get", "post", "put", "patch", "delete"] {
                            if zz.metot(y).is_some() {
                                let yy = y.to_uppercase();
                                match ciftler.iter_mut().find(|(y2, _)| *y2 == yy) {
                                    Some(c) if !drf_mi => c.1 = y.to_string(),
                                    Some(_) => {}
                                    None => ciftler.push((yy, y.to_string())),
                                }
                            }
                        }
                        if let Some(izinli) = dj.http_yontemleri(&zz) {
                            ciftler.retain(|(y, _)| izinli.contains(&y.to_lowercase()));
                        }
                        // RedirectView her yöntemi yönlendirir; yalnızca GET işlem sayılır
                        for (y, e) in ciftler {
                            let isleyici = zz.metot(&y.to_lowercase()).or_else(|| zz.metot(&e)).map(|(d, f, _)| (d, f));
                            islemler.push(Islem { yontem: y, eylem: e, isleyici, aksiyon: None });
                        }
                    }
                }
                kontrolcu = kontrolcu_adi(&s.ad);
                dosya_di = sdi;
                sinif_satiri = s.satir;
                drf = drf_mi;
                fonk = None;
                z = Some(zz);
            }
            Hedef::DisSinif(tam) => {
                let ad = tam.rsplit('.').next().unwrap_or(tam).to_string();
                for (y, e) in taban_yontemleri(&ad) {
                    islemler.push(Islem { yontem: y.to_string(), eylem: e.to_string(), isleyici: None, aksiyon: None });
                }
                if islemler.is_empty() {
                    ek_disarida.push((format!("{}:{}", dj.goreli(r.kayit_di), r.kayit_satir), format!("{} dışarıdaki görünümün yöntemleri bilinmiyor: {tam}", r.yol)));
                    continue;
                }
                kontrolcu = kontrolcu_adi(&ad);
                dosya_di = r.kayit_di;
                sinif_satiri = r.kayit_satir;
                z = None;
                drf = false;
                fonk = None;
            }
            Hedef::ApiKok(rdi, satir) => {
                islemler.push(Islem { yontem: "GET".into(), eylem: "api_root".into(), isleyici: None, aksiyon: None });
                kontrolcu = "ApiRoot".into();
                dosya_di = *rdi;
                sinif_satiri = *satir;
                z = None;
                drf = true;
                fonk = None;
            }
        }
        if r.duzenli {
            notlar.push(format!("yol düzenli ifadeyle tanımlı ve tamamı yola çevrilemedi: {} yaklaşık bir biçimdir", r.yol));
        }
        let sezgi_yolu = r.yol.clone();
        let model = z.as_ref().and_then(|z| dj.gorunum_modeli(z));
        for is in &islemler {
            let id = format!("{} {sezgi_yolu}", is.yontem);
            if !gorulen_idler.insert(id.clone()) {
                // Django ilk eşleşen deseni kullanır: aynı yol ve yöntemin sonraki kaydı gölgededir
                continue;
            }
            let (idi, satir) = match is.isleyici {
                Some((d, f)) => (d, f.satir),
                None => (dosya_di, sinif_satiri),
            };
            let yer = format!("{}:{satir}", guvenli::goreli(kok, &dosyalar[idi].yol));
            let mut is_notlari = notlar.clone();
            // ---- Giriş ve rol ----
            let mut kararlar: Vec<Karar> = vec![];
            let mut guardlar: Vec<String> = vec![];
            if drf {
                let aksiyon_izni = is.aksiyon.and_then(|a| a.anahtarli.iter().find(|(k, _)| k == "permission_classes")).map(|(_, v)| (idi, v.clone()));
                let fonk_izni = fonk.and_then(|(d, f)| {
                    f.dekoratorler.iter().find(|x| x.ad.rsplit('.').next() == Some("permission_classes")).and_then(|x| x.argumanlar.first()).map(|a| (d, a.clone()))
                });
                let view_kw = as_view_kw.iter().find(|(k, _)| k == "permission_classes").map(|(_, v)| (r.kayit_di, v.clone()));
                let sinif_izni = z.as_ref().and_then(|z| z.oznitelik("permission_classes")).and_then(|(d, a)| a.varsayilan.clone().map(|v| (d, v)));
                let dinamik = z.as_ref().and_then(|z| z.metot("get_permissions")).filter(|_| aksiyon_izni.is_none()).map(|(_, f, _)| f);
                let kaynak = aksiyon_izni.or(fonk_izni).or(view_kw).or(sinif_izni.clone());
                let dinamik = if kaynak.is_some() && dinamik.is_none() { None } else { dinamik };
                let (k, g, n) =
                    dj.drf_karari(if dinamik.is_some() { None } else { kaynak }, dinamik, z.as_ref().map(|z| z.siniflar[0].0).unwrap_or(dosya_di), &is.yontem, model.as_deref());
                kararlar.push(k);
                guardlar.extend(g);
                is_notlari.extend(n);
                // DRF kimlik doğrulama sınıfları (yalnızca bilgi)
                let kimlik = z
                    .as_ref()
                    .and_then(|z| z.oznitelik("authentication_classes"))
                    .and_then(|(_, a)| a.varsayilan.clone())
                    .or_else(|| dj.drf_ayari("DEFAULT_AUTHENTICATION_CLASSES").map(|((_, v), _)| v));
                if let Some(l) = kimlik.and_then(|k| liste_ogeleri(&k)) {
                    for (_, x) in l {
                        let a = metin_sabiti(&x).unwrap_or(x);
                        let a = a.rsplit('.').next().unwrap_or(&a).to_string();
                        if !guardlar.contains(&a) {
                            guardlar.push(a);
                        }
                    }
                }
            }
            if let Some(z) = &z {
                let (k, adlar) = dj.sinif_isaretleri(z, is.isleyici.map(|x| x.1));
                kararlar.extend(k);
                guardlar.extend(adlar);
            }
            if let Some((d, f)) = fonk {
                for dek in &f.dekoratorler {
                    if let Some(k) = dj.django_isareti(d, &dekorator_metni(dek), 0) {
                        kararlar.push(k);
                        let etiket = if dj.kimlik_dekoratoru(d, &dek.ad) { crate::anlam::ANAHTAR_GUARD_ETIKETI } else { "" };
                        guardlar.push(format!("{}{etiket}", dek.ad));
                    }
                }
                // Dağıtıcı görünüm: kendisi işaretsiz, isteği yalnızca giriş isteyen görünümlere iletiyor
                // (`if request.method == "POST": return create_check(request)`; `return get_checks(request)`)
                if !kararlar.iter().any(|k| k.giris == Giris::Gerekli) {
                    if let Some((k, adlar)) = dj.dagitici_karari(d, f) {
                        kararlar.push(k);
                        guardlar.extend(adlar);
                    }
                }
            }
            for (sdi, s) in &r.sarmalayicilar {
                if let Some(k) = dj.django_isareti(*sdi, &format!("{s}(x)"), 0).or_else(|| dj.django_isareti(*sdi, s, 0)) {
                    kararlar.push(k);
                    guardlar.push(s.split('(').next().unwrap_or(s).to_string());
                }
            }
            let muaf = kararlar.iter().any(|k| k.giris == Giris::Acik && k.kesin && !drf);
            let mut karar = kararlar.into_iter().reduce(Karar::ve).unwrap_or_else(|| Karar::yeni(Giris::Acik, false));
            if ara_zorunlu && !muaf {
                karar = karar.ve(Karar::yeni(Giris::Gerekli, true));
                guardlar.push("LoginRequiredMiddleware".into());
            }
            if ara_bilinmeyen {
                karar.bilinmeyen = true;
            }
            let (giris, kanit) = match karar.giris {
                Giris::Gerekli if karar.kesin => ("gerekli", "kesin"),
                Giris::Gerekli => ("gerekli", if karar.bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" }),
                Giris::Acik if karar.kesin && !karar.bilinmeyen => ("herkese_acik", "kesin"),
                Giris::Acik => ("herkese_acik", if karar.bilinmeyen { "varsayim_bilinmeyen_guard" } else { "varsayim" }),
                Giris::Bilinmeyen => ("bilinmiyor", "varsayim_bilinmeyen_guard"),
            };
            let mut roller = karar.roller.clone();
            roller.sort();
            roller.dedup();
            guardlar.dedup();
            // ---- Girdiler ----
            let mut govde: Vec<Girdi> = vec![];
            let mut sorgu_alanlari: BTreeMap<String, Alan> = BTreeMap::new();
            let yazma = matches!(is.yontem.as_str(), "POST" | "PUT" | "PATCH");
            let kismi = is.eylem == "partial_update" || (is.yontem == "PATCH" && drf);
            let isleyici_metni = is.isleyici.map(|(_, f)| f.govde_metni.clone()).unwrap_or_default();
            if yazma {
                // Serializer: @action(serializer_class=...), gövdedeki XSerializer(data=...), sınıfın serializer_class'ı
                let aksiyon_ser = is.aksiyon.and_then(|a| a.anahtarli.iter().find(|(k, _)| k == "serializer_class")).map(|(_, v)| (idi, v.clone()));
                let govdeki = govdedeki_serializer(&isleyici_metni);
                let sinif_ser =
                    z.as_ref().and_then(|z| z.oznitelik("serializer_class").or_else(|| z.oznitelik("form_class"))).and_then(|(d, a)| a.varsayilan.clone().map(|v| (d, v)));
                let dinamik_ser = z.as_ref().and_then(|z| z.metot("get_serializer_class").or_else(|| z.metot("get_form_class")));
                let standart = matches!(is.eylem.as_str(), "create" | "update" | "partial_update" | "post" | "put")
                    && is.isleyici.is_none_or(|(_, f)| govdedeki_serializer(&f.govde_metni).is_none());
                let (ser, zarf, kismi2) = match (&aksiyon_ser, &govdeki) {
                    (Some(a), _) => (Some(a.clone()), None, false),
                    (None, Some((ad, veri, k))) => {
                        let ad = if ad.ends_with("get_serializer") || ad.ends_with("serializer_class") { sinif_ser.as_ref().map(|x| x.1.clone()) } else { Some(ad.clone()) };
                        // Zarf: data=request.data.get("user") ya da önce bir sözlüğe dönüştürülen request.data.get("user")
                        let zarf = zarf_anahtari(&isleyici_metni, veri)
                            .map(|z| (z, false))
                            .or_else(|| (!veri.contains("request.data")).then(|| zarf_ara(&isleyici_metni)).flatten().map(|z| (z, true)));
                        (ad.map(|a| (is.isleyici.map(|x| x.0).unwrap_or(idi), a)).or(sinif_ser.clone()), zarf, *k)
                    }
                    (None, None) if standart => (sinif_ser.clone(), None, false),
                    _ => (None, None, false),
                };
                let ser = ser.map(|(d, a)| {
                    // sınıf özniteliğinden gelen ad sınıfın modülünde çözülür
                    let d = if sinif_ser.as_ref().is_some_and(|x| x.1 == a) { sinif_ser.as_ref().unwrap().0 } else { d };
                    (d, a)
                });
                if let (Some(dm), true) = (dinamik_ser.filter(|_| aksiyon_ser.is_none()), ser.is_none() || standart) {
                    // get_serializer_class: çalışma anında seçilir; gövdesindeki serializer'lar aday
                    let (ddi, f, _) = dm;
                    let adaylar: Vec<Sema> = serializer_adlari(&f.govde_metni).iter().filter_map(|a| dj.sema(ddi, a, true, kismi, 0).map(|x| x.0)).collect();
                    is_notlari.push("serializer çalışma anında seçiliyor (get_serializer_class): gövde şeması adaylardan biridir".into());
                    govde.push(Girdi {
                        ad: None,
                        tip: Some("serializer".into()),
                        sema: Some(Sema {
                            dto: "get_serializer_class".into(),
                            dosya: Some(guvenli::goreli(kok, &dosyalar[ddi].yol)),
                            bulunamadi: true,
                            kalitim: None,
                            alanlar: BTreeMap::new(),
                            adaylar,
                            dizi: false,
                        }),
                    });
                } else if let Some((sdi2, ad)) = ser {
                    match dj.sema(sdi2, &ad, true, kismi || kismi2, 0) {
                        Some((mut sema, n)) => {
                            is_notlari.extend(n);
                            if let Some((z, donusturulmus)) = &zarf {
                                // Gövde zarflı: {"user": {...}}. Zarf yoksa `{}` kullanılır: iç şemada zorunlu alan varsa
                                // istek yine reddedilir
                                let mut ic = std::mem::take(&mut sema.alanlar);
                                if *donusturulmus {
                                    // Zarfın içi serializer'a verilmeden önce başka bir sözlüğe dönüştürülüyor: alanlar aday
                                    for a in ic.values_mut() {
                                        a.belirsiz = true;
                                        a.zorunlu = false;
                                    }
                                    is_notlari.push(format!("gövde ('{z}' zarfı) serializer'a verilmeden önce dönüştürülüyor: iç alanlar serializer'dan aday olarak alındı"));
                                }
                                let zorunlu = ic.values().any(|a| a.zorunlu);
                                sema.alanlar.insert(
                                    z.clone(),
                                    Alan {
                                        tip: Some("object".into()),
                                        zorunlu,
                                        belirsiz: *donusturulmus,
                                        kurallar: vec!["ValidateNested()".into()],
                                        satir,
                                        kalitimdan: None,
                                        degerler: vec![],
                                        ic: Some(ic),
                                    },
                                );
                            }
                            govde.push(Girdi { ad: None, tip: Some(sema.dto.clone()), sema: Some(sema) });
                        }
                        None => {
                            let ad = ad.trim().to_string();
                            govde.push(Girdi {
                                ad: None,
                                tip: Some(ad.clone()),
                                sema: Some(Sema { dto: ad, dosya: None, bulunamadi: true, kalitim: None, alanlar: BTreeMap::new(), adaylar: vec![], dizi: false }),
                            });
                        }
                    }
                } else {
                    // Gövdede doğrudan okunan alanlar (serializer yok): tip ve zorunluluk koddan çıkmaz
                    let (alanlar, _) = istek_alanlari(&isleyici_metni);
                    if !alanlar.is_empty() {
                        let m: BTreeMap<String, Alan> = alanlar.into_iter().map(|(a, koseli)| (a, serbest_alan(satir, koseli))).collect();
                        govde.push(Girdi {
                            ad: None,
                            tip: Some("request.data".into()),
                            sema: Some(Sema {
                                dto: "body".into(),
                                dosya: Some(guvenli::goreli(kok, &dosyalar[idi].yol)),
                                bulunamadi: false,
                                kalitim: None,
                                alanlar: m,
                                adaylar: vec![],
                                dizi: false,
                            }),
                        });
                        is_notlari.push("gövde alanları işleyicinin okuduğu anahtarlardan çıkarıldı; tipleri ve doğrulamaları bilinmiyor".into());
                    }
                }
            }
            // Sorgu: gövdede okunan parametreler, liste eyleminde filtre / arama / sıralama / sayfalama
            // Liste ve ayrıntı eyleminde get_queryset'in okuduğu sorgu parametreleri de
            let mut sorgu_metni = isleyici_metni.clone();
            if matches!(is.eylem.as_str(), "list" | "retrieve") {
                if let Some((_, f, _)) = z.as_ref().and_then(|z| z.metot("get_queryset")) {
                    sorgu_metni.push('\n');
                    sorgu_metni.push_str(&f.govde_metni);
                }
            }
            let (_, sorgular) = istek_alanlari(&sorgu_metni);
            for q in sorgular {
                sorgu_alanlari.insert(q, serbest_alan(satir, false));
            }
            if is.eylem == "list" {
                if let Some(z) = &z {
                    for (anahtar, param) in [("search_fields", "search"), ("ordering_fields", "ordering")] {
                        if z.oznitelik(anahtar).is_some() {
                            sorgu_alanlari.insert(
                                param.into(),
                                Alan {
                                    tip: Some("string".into()),
                                    zorunlu: false,
                                    belirsiz: false,
                                    kurallar: vec!["IsString()".into()],
                                    satir,
                                    kalitimdan: None,
                                    degerler: vec![],
                                    ic: None,
                                },
                            );
                        }
                    }
                    if let Some(l) = z.oznitelik("filterset_fields").or_else(|| z.oznitelik("filter_fields")).and_then(|(_, a)| a.varsayilan.as_deref().and_then(metin_listesi)) {
                        for f in l {
                            sorgu_alanlari.insert(f, serbest_alan(satir, false));
                        }
                    }
                    let sayfalama = z.oznitelik("pagination_class").and_then(|(_, a)| a.varsayilan.clone()).or(varsayilan_sayfalama.clone());
                    if let Some(s) = sayfalama.filter(|s| s.trim() != "None") {
                        let s = metin_sabiti(&s).unwrap_or(s);
                        let s = s.rsplit('.').next().unwrap_or(&s);
                        let params: &[&str] = if s.contains("LimitOffset") {
                            &["limit", "offset"]
                        } else if s.contains("Cursor") {
                            &["cursor"]
                        } else {
                            &["page"]
                        };
                        for p in params {
                            sorgu_alanlari.entry(p.to_string()).or_insert(Alan {
                                tip: Some(if *p == "cursor" { "string" } else { "int" }.into()),
                                zorunlu: false,
                                belirsiz: false,
                                kurallar: vec![],
                                satir,
                                kalitimdan: None,
                                degerler: vec![],
                                ic: None,
                            });
                        }
                    }
                }
            }
            let sorgu = if sorgu_alanlari.is_empty() {
                vec![]
            } else {
                vec![Girdi {
                    ad: None,
                    tip: None,
                    sema: Some(Sema {
                        dto: "query".into(),
                        dosya: Some(guvenli::goreli(kok, &dosyalar[idi].yol)),
                        bulunamadi: false,
                        kalitim: None,
                        alanlar: sorgu_alanlari,
                        adaylar: vec![],
                        dizi: false,
                    }),
                }]
            };
            let parametreler: Vec<Girdi> = r.parametreler.iter().map(|(a, t)| Girdi { ad: Some(a.clone()), tip: Some(t.clone()), sema: None }).collect();
            // ---- Yanıt ----
            let yanit = if drf && is.yontem != "DELETE" {
                let ser = is
                    .aksiyon
                    .and_then(|a| a.anahtarli.iter().find(|(k, _)| k == "serializer_class"))
                    .map(|(_, v)| (idi, v.clone()))
                    .or_else(|| z.as_ref().and_then(|z| z.oznitelik("serializer_class")).and_then(|(d, a)| a.varsayilan.clone().map(|v| (d, v))));
                let standart = matches!(is.eylem.as_str(), "list" | "retrieve" | "create" | "update" | "partial_update");
                ser.filter(|_| standart || is.aksiyon.is_some()).and_then(|(d, a)| dj.sema(d, &a, false, false, 0)).map(|(mut sema, _)| {
                    sema.dizi = is.eylem == "list";
                    let sayfali = z
                        .as_ref()
                        .and_then(|z| z.oznitelik("pagination_class"))
                        .map(|(_, a)| a.varsayilan.as_deref().map(str::trim) != Some("None"))
                        .unwrap_or(varsayilan_sayfalama.is_some());
                    let cizici = z.as_ref().is_some_and(|z| z.oznitelik("renderer_classes").is_some()) || ozel_cizici;
                    Yanit {
                        kaynak: "serializer_class".into(),
                        // Generic eylem serializer'la döner; özel çizici ya da sayfalama zarfı varsa biçim kesin değil
                        kesin: standart && is.isleyici.is_none() && !cizici && !(is.eylem == "list" && sayfali),
                        durum: Some(if is.eylem == "create" { 201 } else { 200 }),
                        sema,
                    }
                })
            } else {
                None
            };
            // ---- Veri ----
            let mut bag = VeriBag::default();
            if let Some(m) = &model {
                if drf && is.isleyici.is_none()
                    || matches!(is.eylem.as_str(), "list" | "retrieve" | "create" | "update" | "partial_update" | "destroy")
                        && z.as_ref().is_some_and(|z| z.dis_var(&VIEWSET) || z.dis_var(&APIVIEW))
                {
                    match is.eylem.as_str() {
                        "list" | "retrieve" => bag.okur.push(m.clone()),
                        "create" => bag.yazar.push(m.clone()),
                        "update" | "partial_update" => {
                            bag.okur.push(m.clone());
                            bag.yazar.push(m.clone());
                        }
                        "destroy" => {
                            bag.okur.push(m.clone());
                            bag.yazar.push(m.clone());
                        }
                        _ => {}
                    }
                }
                // Django genel görünümleri
                if !drf && z.is_some() {
                    match is.yontem.as_str() {
                        "GET" => bag.okur.push(m.clone()),
                        "POST" | "PUT" if z.as_ref().is_some_and(|z| z.dis_var(&["CreateView", "UpdateView", "DeleteView"])) => {
                            if z.as_ref().is_some_and(|z| !z.dis_var(&["CreateView"])) {
                                bag.okur.push(m.clone());
                            }
                            bag.yazar.push(m.clone());
                        }
                        "DELETE" => bag.yazar.push(m.clone()),
                        _ => {}
                    }
                }
            }
            let mut baslangic: Vec<(usize, &Fonksiyon)> = is.isleyici.into_iter().collect();
            if let Some(zz) = &z {
                for kanca in match is.eylem.as_str() {
                    "create" => vec!["perform_create"],
                    "update" | "partial_update" => vec!["perform_update"],
                    "destroy" => vec!["perform_destroy"],
                    "list" | "retrieve" => vec!["get_queryset"],
                    _ => vec![],
                } {
                    if let Some((d, f, _)) = zz.metot(kanca) {
                        baslangic.push((d, f));
                    }
                }
            }
            dj.orm(z.as_ref(), baslangic, &mut bag);
            bag.okur.sort();
            bag.okur.dedup();
            bag.yazar.sort();
            bag.yazar.dedup();
            let veri = (!dj.modeller.is_empty()).then_some(bag);
            // ---- Davranış özeti ----
            if let std::collections::btree_map::Entry::Vacant(e) = dna_haritasi.entry(yer.clone()) {
                let mut v = match (&z, is.isleyici) {
                    (Some(zz), Some(h)) => yontem_dna(b, kok, zz, Some(h)),
                    (None, Some((d, f))) => crate::fastapi::dna_zinciri(b, kok, d, f, &[]),
                    _ => json!({}),
                };
                // Sinyaller: yazılan modelin kaydetme / silme sinyalleri tetiklenir (Yorum). Silme: DELETE ya da gövdede
                // yalnızca silme çağrısı
                let mut tetikler: Vec<Value> = v["tetikler"].as_array().cloned().unwrap_or_default();
                let silme = is.yontem == "DELETE" || (isleyici_metni.contains(".delete(") && !isleyici_metni.contains(".save(") && !isleyici_metni.contains(".create("));
                if let Some(vb) = &veri {
                    for m in &vb.yazar {
                        let adlar: &[&str] = if silme { &["pre_delete", "post_delete"] } else { &["pre_save", "post_save"] };
                        for s in adlar {
                            let ad = format!("{s}:{m}");
                            if sinyaller.contains_key(&format!("event:{ad}")) && !tetikler.iter().any(|t| t["ad"] == ad.as_str()) {
                                tetikler.push(json!({ "tur": "event", "ad": ad, "kanit": yer, "guven": "inferred" }));
                            }
                        }
                    }
                }
                if !tetikler.is_empty() {
                    if v.as_object().is_none_or(|o| o.is_empty()) {
                        v = json!({ "hatalar": [], "sahiplik_denetimi": [], "dallar": [], "yumusak_silme": null, "silinen": [], "tetikler": [], "kapsam": "acik" });
                    }
                    v["tetikler"] = json!(tetikler);
                }
                if v.as_object().is_some_and(|o| !o.is_empty()) && !crate::dna::bos(&v) {
                    e.insert(v);
                }
            }
            if !is_notlari.is_empty() {
                let l = denetlenemeyen.entry(yer.clone()).or_default();
                for n in is_notlari {
                    if !l.contains(&n) {
                        l.push(n);
                    }
                }
            }
            if r.kosullu {
                kosullu_kapilar.push(id.clone());
            }
            kapi_routerlari.entry(yer.clone()).or_insert_with(|| r.kaynak.clone());
            let ozet = is
                .isleyici
                .and_then(|(_, f)| f.belge.clone())
                .or_else(|| fonk.and_then(|(_, f)| f.belge.clone()))
                .unwrap_or_else(|| crate::fastapi::varsayilan_ozet(&format!("{}_{}", is.eylem, kontrolcu.to_lowercase())));
            kapilar.push(Kapi {
                id,
                yontem: is.yontem.clone(),
                yol: sezgi_yolu.clone(),
                kontrolcu: kontrolcu.clone(),
                fonksiyon: snake_camel(&is.eylem),
                dosya: guvenli::goreli(kok, &dosyalar[idi].yol),
                satir,
                giris: giris.into(),
                guardlar: guardlar.clone(),
                roller_kaniti: (!roller.is_empty() && !karar.rol_kesin).then(|| "varsayim".into()),
                roller,
                ozet: Some(ozet),
                govde,
                sorgu,
                parametreler,
                kullanimdan_kalkmis: false,
                yerine: None,
                surum: None,
                uygulama: None,
                ana: r.ana,
                giris_kaniti: kanit.into(),
                veri,
                yanit,
            });
        }
    }
    kapilar.sort_by(|a, b| a.yol.cmp(&b.yol).then(a.yontem.cmp(&b.yontem)));
    let routerlar: Vec<Value> = dj
        .graf
        .iter()
        .map(|(id, (dosya, satir, ustler))| {
            let u: Vec<Value> = ustler.iter().map(|(ust, icerme, kosul, kosullu)| json!({ "id": ust, "icerme": icerme, "kosul": kosul, "kosullu": kosullu })).collect();
            json!({ "id": id, "dosya": dosya, "satir": satir, "ustler": u })
        })
        .collect();
    let disarida: Vec<Value> = dj.disaridakiler.iter().chain(ek_disarida.iter()).map(|(y, a)| json!({ "yer": y, "neden": a })).collect();
    (
        kapilar,
        json!({ "genel_onek": "", "genel_guardlar": [], "ana_uygulama": ana_modul, "kosullu_kapilar": kosullu_kapilar, "denetlenemeyen": denetlenemeyen,
        "dna": dna_haritasi, "kaskadlar": dj.kaskadlar(), "olaylar": sinyaller,
        "routerlar": routerlar, "kapi_routerlari": kapi_routerlari, "veri_modelleri": dj.veri_modelleri(),
        "haritalanmayan": disarida }),
    )
}
