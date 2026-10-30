//! D10 kapsam: Sezgi az bulduğunda ya da haritalamadığı bir API türü gördüğünde sessiz kalmaz (init, rapor, kimsin);
//! ayardan gelen Django öneki (`path(settings.BASE_PATH, include(...))`) rotaları ikincil bırakmaz.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::Value;

fn yaz(p: &Proje, yol: &str, metin: &str) {
    let d = p.klasor.join(yol);
    std::fs::create_dir_all(d.parent().unwrap()).unwrap();
    std::fs::write(d, metin).unwrap();
}

fn rapor(p: &Proje) -> Value {
    serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap()
}

/// NetBox gibi: bütün rotalar `settings.BASE_PATH` önekinin altında, önek yapılandırmadan okunuyor
const KOK_URLS: &str = r#"from django.conf import settings
from django.urls import include, path
from rest_framework.routers import DefaultRouter

from kitaplar import views

router = DefaultRouter()
router.register(r"kitaplar", views.KitapViewSet, basename="kitap")

_patterns = [
    path("api/", include(router.urls)),
    path("api/v1/", include("kitaplar.urls")),
    path("hesap/", include("hesap.urls")),
]

urlpatterns = [
    path(settings.BASE_PATH, include(_patterns)),
]
"#;

#[test]
fn ayardan_gelen_django_oneki_rotalari_ikincil_birakmaz() {
    let ev = Ev::yeni("d10-kapsam-onek");
    let p = ev.proje("d6_python_django", "a");
    yaz(&p, "kitaplik/urls.py", KOK_URLS);
    let ayar = std::fs::read_to_string(p.klasor.join("kitaplik/settings/temel.py")).unwrap();
    yaz(&p, "kitaplik/settings/temel.py", &format!("{ayar}\nBASE_PATH = trailing_slash(getattr(configuration, \"BASE_PATH\", \"\"))\n"));
    p.ok(&["init", "."]);
    let h: Value = serde_json::from_str(&std::fs::read_to_string(p.klasor.join(".sezgi/kapilar.json")).unwrap()).unwrap();
    let ops = h["endpointler"].as_array().unwrap();
    assert!(ops.iter().any(|e| e["yol"] == "/api/kitaplar/"), "{:?}", ops.iter().map(|e| &e["id"]).collect::<Vec<_>>());
    assert!(ops.iter().all(|e| e["ana"] == true), "ayardan gelen önekin altındaki rotalar ana: {:?}", ops.iter().filter(|e| e["ana"] != true).map(|e| &e["id"]).collect::<Vec<_>>());
    // Önek kurulumda farklı olabilir: rotalar koşullu
    assert!(h["ozet"]["kosullu_kapilar"].as_array().unwrap().iter().any(|x| x == "GET /api/kitaplar/"), "{}", h["ozet"]["kosullu_kapilar"]);
    assert!(!p.kart()["operations"].as_array().unwrap().is_empty(), "kart boş kalmaz");

    // Hiç çözülemeyen önek: rotalar yine öneksiz yazılır ve koşullu kalır (kart boş değil)
    yaz(&p, "kitaplik/settings/temel.py", &format!("{ayar}\nBASE_PATH = os.environ[\"NETBOX_ONEK\"]\n"));
    p.ok(&["init", "."]);
    assert!(!p.kart()["operations"].as_array().unwrap().is_empty(), "çözülemeyen önekte de kart boş kalmaz");
}

#[test]
fn graphql_bagimliligi_kapsam_disi_diye_soylenir() {
    let ev = Ev::yeni("d10-kapsam-graphql");
    let p = ev.proje("nestjs-temel", "a");
    let pj = std::fs::read_to_string(p.klasor.join("package.json")).unwrap();
    let mut v: Value = serde_json::from_str(&pj).unwrap();
    v["dependencies"]["@nestjs/graphql"] = serde_json::json!("^12.0.0");
    v["dependencies"]["socket.io"] = serde_json::json!("^4.7.0");
    yaz(&p, "package.json", &serde_json::to_string_pretty(&v).unwrap());
    let c = p.ok(&["init", "."]);
    assert!(c.contains("Kapsam") && c.contains("GraphQL (@nestjs/graphql") && c.contains("WebSocket (socket.io"), "{c}");
    let r = rapor(&p);
    assert_eq!(r["kapilar"]["kapsam"], "kismen", "{r}");
    assert!(r["kapilar"]["kapsam_uyarilari"].to_string().contains("GraphQL"), "{r}");
    // Yapay zekâ da bilir: kimsin kapsamı söyler
    let t = p.token("ajan", &["kitap.*:oku"], &[]);
    let (s, _) = p.mcp(&t).arac("kimsin", serde_json::json!({}));
    assert!(s.contains("KAPSAM") && s.contains("GraphQL"), "{s}");
}

#[test]
fn hic_islem_bulunmayan_uygulamada_kapsam_dusuk_denir() {
    let ev = Ev::yeni("d10-kapsam-sifir");
    let p = ev.proje("nestjs-temel", "a");
    // Rotalar Sezgi'nin okumadığı bir yolla, çalışma anında kurulur: hiç denetleyici yok
    let mut yigin = vec![p.klasor.join("src")];
    while let Some(k) = yigin.pop() {
        for g in std::fs::read_dir(k).unwrap().flatten() {
            if g.file_type().unwrap().is_dir() {
                yigin.push(g.path());
            } else if g.file_name().to_string_lossy().ends_with(".controller.ts") {
                std::fs::remove_file(g.path()).unwrap();
            }
        }
    }
    let (_, c) = p.cikti(&["init", "."]);
    assert!(c.contains("DÜŞÜK"), "{c}");
}

#[test]
fn normal_uygulamada_kapsam_uyarisi_yok() {
    let ev = Ev::yeni("d10-kapsam-normal");
    let p = ev.proje("nestjs-temel", "a");
    let c = p.ok(&["init", "."]);
    assert!(!c.contains("Kapsam") && !c.contains("DÜŞÜK"), "{c}");
    assert_eq!(rapor(&p)["kapilar"]["kapsam"], "tam");
}
