//! D9 güven (kimlik bilgisi): parola, token ya da iki adımlı doğrulama değiştiren yazma, geri alınabilirliği
//! kanıtlanmadıkça onaysız gitmez (son tur: LinkAce `POST /settings/change-password` onaysız gidiyordu). Adı kimlik
//! bilgisi söylemeyen yazma bu gerekçeyle onaya düşmez. İngilizcesi: src-ekleri/dis.rs.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const GEREKCE: &str = "işlem kimlik bilgisi değiştiriyor";

fn haritala(ad: &str) -> PathBuf {
    let ornek = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad);
    let cikti = std::env::temp_dir().join(format!("sezgi-d9kimlik-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cikti);
    let c = Command::new(env!("CARGO_BIN_EXE_sezgi"))
        .args(["init", ornek.to_str().unwrap(), "--cikti", cikti.to_str().unwrap(), "--paylasimsiz"])
        .env("SEZGI_ANAHTARLIK", "kapali")
        .output()
        .unwrap();
    assert!(c.status.success(), "init: {}{}", String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&c.stderr));
    cikti
}

fn json_oku(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn kapi(m: &Value, id: &str) -> Value {
    m["endpointler"].as_array().unwrap().iter().find(|k| k["id"] == id).cloned().unwrap_or_else(|| panic!("{id} yok"))
}

#[test]
fn kimlik_bilgisi_degistiren_yazma_onaya_duser() {
    let h = haritala("d9_guven_kimlik");
    let m = json_oku(&h.join("kapilar.json"));
    let kart = json_oku(&h.join("benlik.json"));
    for id in ["POST /settings/change-password", "POST /settings/api-tokens", "POST /settings/two-factor"] {
        let k = kapi(&m, id);
        let o = k["etki_onayi"].as_str().unwrap_or_else(|| panic!("{id} onay ister: {k:#}"));
        assert!(o.starts_with(GEREKCE), "{id}: {o}");
        let yol = id.split(' ').nth(1).unwrap();
        let op = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "POST" && o["entry"]["path"] == yol).unwrap();
        assert_eq!(op["requires_approval"], true, "{id}");
    }
    let profil = kapi(&m, "POST /settings/profile");
    assert!(!profil["etki_onayi"].as_str().unwrap_or("").starts_with(GEREKCE), "{profil:#}");
    let _ = std::fs::remove_dir_all(&h);
}
