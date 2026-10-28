//! D9 güven (erişim denetimi): OAuth / kimlik sağlayıcı istemcisinin akış çağrısı (`authorize_access_token`,
//! `authorize_redirect`, `Socialite::driver()->user()`) erişim denetimi değildir; erişim denetimi istekteki kimliği bir
//! kayda ya da izne karşı denetleyip reddeden çağrıdır. `kesin` damgası yalnızca çatının bilinen yetki API'sinde
//! (`$this->authorize`, `Gate::allows` ...). ORM ilişki tanımı (`belongsToMany`) denetim değildir. Alıcısız çağrı
//! (Laravel'in küresel `redirect()` yardımcısı) aynı adlı bir sınıf yöntemine (`SocialiteController::redirect`)
//! bağlanmaz: o yöntemin 403'ü başka işlemin ön koşulu sayılmaz.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d9guvene-{ad}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", ornek(ad).to_str().unwrap(), "--cikti", cikti.to_str().unwrap(), "--paylasimsiz"])
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    assert!(c.status.success(), "init: {}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    cikti
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn onermeler(depo: &Value, varlik: &str, yuklem: &str) -> Vec<Value> {
    depo["onermeler"].as_array().unwrap().iter().filter(|o| o["varlik"] == varlik && o["yuklem"] == yuklem).cloned().collect()
}

fn dna(m: &Value, id: &str) -> Value {
    let k = m["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).cloned().unwrap_or_else(|| panic!("{id} yok"));
    m["ozet"]["dna"][format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]).as_str()].clone()
}

#[test]
fn oauth_akisi_erisim_denetimi_sayilmaz() {
    let h = haritala("d9_guven_cbv");
    let depo = json_oku(&h.join("kanit.json"));
    for api in ["api:GET /api/auth/oauth/callback", "api:GET /api/auth/oauth"] {
        for y in ["NESNE_YETKI", "YETKI_DENETIMI"] {
            let n = onermeler(&depo, api, y);
            assert!(n.is_empty(), "{api} {y}: OAuth token değişimi / yönlendirmesi erişim denetimi değildir: {n:#?}");
        }
    }
    let kart = json_oku(&h.join("benlik.json"));
    let op = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["path"] == "/api/auth/oauth/callback").unwrap();
    assert!(op["x-dna"]["auth"]["object_level"].as_array().is_none_or(|a| a.is_empty()), "{:#}", op["x-dna"]["auth"]);
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn kuresel_yardimci_ayni_adli_yonteme_baglanmaz() {
    let h = haritala("d9_guven_cop");
    let depo = json_oku(&h.join("kanit.json"));
    // TrashController.clearTrash -> redirect(): Laravel yardımcısı, SocialiteController::redirect değil
    let yanlis: Vec<&Value> = depo["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["yuklem"] == "CALLS" && o["varlik"].as_str().unwrap().contains("TrashController") && o["hedef"].as_str().unwrap_or("").contains("SocialiteController"))
        .collect();
    assert!(yanlis.is_empty(), "{yanlis:#?}");
    let m = json_oku(&h.join("kapilar.json"));
    for id in ["POST /trash/clear", "POST /notes/purge", "GET /trash"] {
        let d = dna(&m, id);
        assert!(!d.to_string().contains("SocialiteController"), "{id}: başka fonksiyonun 403'ü bu işleme atfedilmemeli: {d:#}");
    }
    // Asıl yöntemin kendi 403'ü yerinde kalır
    assert!(dna(&m, "GET /auth/:provider").to_string().contains("SocialiteController.php"), "{:#}", dna(&m, "GET /auth/:provider"));
    let _ = std::fs::remove_dir_all(&h);
}
