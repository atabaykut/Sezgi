//! S5 (Benlik Kartı 0.1.1): kartın tazeliği ve bütünlüğü.
//! - `x-issued_at` / `x-expires_at` (varsayılan 30 gün, `--gecerlilik <gün>`), imza bunları kapsar;
//! - `stale` elle yazılan bayrak değil: `x-basis` (dosya → SHA-256) ile o anki dosyalardan hesaplanır;
//! - `x-revision`: işlem kümesinin kanonik özeti; `changes` commit'siz ve git olmayan projede de üretilir;
//! - yol değişikliği "breaking" sayılır (aynı işlem kimliği, farklı adres);
//! - `x-evidence` (kayıt başına JCS özeti) + `x-evidence_root` (Merkle kökü) + ayrı kanıt paketi;
//! - `sezgi dogrula` süreyi, kanıt kökünü, paketi ve bayatlığı denetler.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn kopyala(kaynak: &Path, hedef: &Path) {
    std::fs::create_dir_all(hedef).unwrap();
    for g in std::fs::read_dir(kaynak).unwrap() {
        let p = g.unwrap().path();
        let h = hedef.join(p.file_name().unwrap());
        if p.is_dir() {
            kopyala(&p, &h);
        } else {
            std::fs::copy(&p, &h).unwrap();
        }
    }
}

/// Örnek uygulamanın kopyası ve ayrı bir yapılandırma klasörü (anahtarlar, güven çapası)
fn hazirla(ad: &str) -> (PathBuf, PathBuf) {
    let k = std::env::temp_dir().join(format!("sezgi-s5-{ad}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&k);
    kopyala(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("nestjs-temel"), &k.join("uyg"));
    let ayar = k.join("ayar");
    std::fs::create_dir_all(&ayar).unwrap();
    (k.join("uyg"), ayar)
}

fn sezgi(ayar: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sezgi")).args(args).env("APPDATA", ayar).env("XDG_CONFIG_HOME", ayar).env("HOME", ayar).output().unwrap()
}

fn metin(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn json(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn yaz(p: &Path, v: &Value) {
    std::fs::write(p, serde_json::to_string_pretty(v).unwrap()).unwrap();
}

fn degistir(k: &Path, dosya: &str, eski: &str, yeni: &str) {
    let f = k.join(dosya);
    let m = std::fs::read_to_string(&f).unwrap().replace("\r\n", "\n");
    assert!(m.contains(eski), "{dosya}: {eski}");
    std::fs::write(&f, m.replacen(eski, yeni, 1)).unwrap();
}

fn sha256(b: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, b).as_ref().iter().map(|x| format!("{x:02x}")).collect()
}

/// "2026-10-20T07:00:00Z" -> Unix saniyesi
fn saniye(s: &str) -> i64 {
    let (t, h) = s.trim_end_matches('Z').split_once('T').unwrap();
    let mut p = t.split('-').map(|x| x.parse::<i64>().unwrap());
    let (y, a, g) = (p.next().unwrap(), p.next().unwrap(), p.next().unwrap());
    let mut q = h.split(':').map(|x| x.parse::<i64>().unwrap());
    let (sa, dk, sn) = (q.next().unwrap(), q.next().unwrap(), q.next().unwrap());
    let y = if a <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if a > 2 { a - 3 } else { a + 9 }) + 2) / 5 + g - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) * 86_400 + sa * 3600 + dk * 60 + sn
}

#[test]
fn s5_kart_tazelik_ve_dayanak_alanlari() {
    let (k, ayar) = hazirla("alanlar");
    let o = sezgi(&ayar, &["init", k.to_str().unwrap()]);
    assert!(o.status.success(), "{}", metin(&o));
    let kart = json(&k.join(".sezgi").join("benlik.json"));
    assert_eq!(kart["benlik"], "0.1.1");
    // Varsayılan geçerlilik 30 gün
    let (v, s) = (kart["x-issued_at"].as_str().unwrap(), kart["x-expires_at"].as_str().unwrap());
    assert_eq!(saniye(s) - saniye(v), 30 * 86_400, "{v} → {s}");
    // Revizyon: işlem kümesinin kanonik özeti
    let r = kart["x-revision"].as_str().unwrap();
    assert!(r.starts_with("sha256:") && r.len() == 7 + 64, "{r}");
    // Dayanak: kaynak dosyaların o anki özetleri; elle yazılan `stale` bayrağı yok
    let dayanak = kart["x-basis"].as_object().unwrap();
    let ctrl = "src/kitap/kitap.controller.ts";
    assert_eq!(dayanak[ctrl].as_str().unwrap(), sha256(&std::fs::read(k.join(ctrl)).unwrap()));
    for op in kart["operations"].as_array().unwrap() {
        assert!(op.get("stale").is_none(), "stale elle yazılmamalı: {}", op["id"]);
        let dna = &op["x-dna"];
        assert!(dna.is_object(), "x-dna yok: {}", op["id"]);
        assert!(["yes", "no", "optional", "unknown"].contains(&dna["auth"]["required"].as_str().unwrap()), "{dna}");
        assert!(!dna["evidence_refs"].as_array().unwrap().is_empty(), "{dna}");
    }
    // Kanıt özetleri: işlem başına bir kaynak kodu kaydı; paket ayrı dosyada, kayıt özetleri karttakiyle aynı
    let ozetler = kart["x-evidence"].as_array().unwrap();
    assert!(ozetler.len() >= kart["operations"].as_array().unwrap().len(), "işlem başına en az bir kanıt kaydı (kanıt deposunun projeksiyonu)");
    assert!(kart["x-evidence_root"].as_str().unwrap().len() == 64);
    assert_eq!(kart["x-evidence_bundle"], "benlik-evidence.json");
    let paket = json(&k.join(".sezgi").join("benlik-evidence.json"));
    assert_eq!(paket["records"].as_array().unwrap().len(), ozetler.len());
    // Gizlilik projeksiyonu: kayıtta değer yok, yalnızca biçim (dosya, satır, özet)
    let m = serde_json::to_string(&paket).unwrap();
    assert!(!m.contains(&*k.to_string_lossy()), "paket mutlak yol içeriyor");
    let _ = std::fs::remove_dir_all(k.parent().unwrap());
}

#[test]
fn s5_gecerlilik_bayragi() {
    let (k, ayar) = hazirla("gecerlilik");
    let o = sezgi(&ayar, &["init", k.to_str().unwrap(), "--gecerlilik", "7"]);
    assert!(o.status.success(), "{}", metin(&o));
    let kart = json(&k.join(".sezgi").join("benlik.json"));
    assert_eq!(saniye(kart["x-expires_at"].as_str().unwrap()) - saniye(kart["x-issued_at"].as_str().unwrap()), 7 * 86_400);
    // Geçersiz süre: kullanım hatası, hiçbir şey yazılmaz
    for kotu in ["0", "400", "abc"] {
        let o = sezgi(&ayar, &["init", k.to_str().unwrap(), "--gecerlilik", kotu]);
        assert_eq!(o.status.code(), Some(2), "{kotu}: {}", metin(&o));
    }
    let _ = std::fs::remove_dir_all(k.parent().unwrap());
}

#[test]
fn s5_imzali_kart_ve_kanit_paketi_dogrulanir() {
    let (k, ayar) = hazirla("imza");
    assert!(sezgi(&ayar, &["anahtar", "olustur", "--ad", "s5"]).status.success());
    let o = sezgi(&ayar, &["init", k.to_str().unwrap(), "--imzala", "s5"]);
    assert!(o.status.success(), "{}", metin(&o));
    let h = k.join(".sezgi");
    let (kart, anahtar, paket) = (h.join("benlik.json"), h.join("benlik-keys.json"), h.join("benlik-evidence.json"));
    // Paket kendi bağlamıyla imzalı
    assert_eq!(json(&paket)["signature"]["alg"], "ML-DSA-65");
    let o = sezgi(&ayar, &["dogrula", kart.to_str().unwrap(), anahtar.to_str().unwrap()]);
    let m = metin(&o);
    assert!(o.status.success(), "{m}");
    assert!(m.contains("Kanıt paketi") && m.contains("geçerli"), "{m}");
    assert!(m.contains("Geçerlilik"), "{m}");

    // Paketteki bir kayıt değiştirilirse (sahte gövde) doğrulama başarısız
    let asil = json(&paket);
    let mut sahte = asil.clone();
    sahte["records"][0]["locator"]["line_start"] = serde_json::json!(999);
    yaz(&paket, &sahte);
    let o = sezgi(&ayar, &["dogrula", kart.to_str().unwrap(), anahtar.to_str().unwrap()]);
    let m = metin(&o);
    assert!(!o.status.success(), "sahte kayıt kabul edildi: {m}");
    assert!(m.contains("özeti uyuşmuyor"), "{m}");

    // Eksik kayıt (paylaşılmayan) kabul edilir ama "sunulmadı" olarak bildirilir
    let mut eksik = asil.clone();
    eksik["records"].as_array_mut().unwrap().remove(0);
    eksik.as_object_mut().unwrap().remove("signature");
    yaz(&paket, &eksik);
    let o = sezgi(&ayar, &["dogrula", kart.to_str().unwrap(), anahtar.to_str().unwrap()]);
    let m = metin(&o);
    assert!(o.status.success(), "{m}");
    assert!(m.contains("sunulmadı"), "{m}");
    yaz(&paket, &asil);

    // Kaynak dosya değişince işlemler bayat görünür (dayanak özetinden hesaplanır)
    degistir(&k, "src/kitap/kitap.controller.ts", "Kitapları listele", "Kitapları sırala");
    let o = sezgi(&ayar, &["dogrula", kart.to_str().unwrap(), anahtar.to_str().unwrap()]);
    let m = metin(&o);
    assert!(m.contains("bayat") && m.contains("kitap.listele"), "{m}");
    let _ = std::fs::remove_dir_all(k.parent().unwrap());
}

#[test]
fn s5_suresi_dolmus_ve_kok_uyusmazligi_bildirilir() {
    let (k, ayar) = hazirla("sure");
    assert!(sezgi(&ayar, &["init", k.to_str().unwrap()]).status.success());
    let h = k.join(".sezgi");
    let yol = h.join("benlik.json");
    let mut kart = json(&yol);
    kart["x-issued_at"] = serde_json::json!("2020-01-01T00:00:00Z");
    kart["x-expires_at"] = serde_json::json!("2020-01-31T00:00:00Z");
    kart["x-evidence"][0]["sha256"] = serde_json::json!("0".repeat(64));
    yaz(&yol, &kart);
    let o = sezgi(&ayar, &["dogrula", yol.to_str().unwrap(), h.join("yok.json").to_str().unwrap()]);
    let m = metin(&o);
    assert!(!o.status.success(), "{m}");
    assert!(m.contains("süresi dolmuş"), "{m}");
    assert!(m.contains("Merkle kökü"), "{m}");
    let _ = std::fs::remove_dir_all(k.parent().unwrap());
}

#[test]
fn s5_changes_gitsiz_projede_uretilir_ve_yol_degisikligi_bozar() {
    let (k, ayar) = hazirla("changes");
    let ks = k.to_str().unwrap();
    assert!(sezgi(&ayar, &["init", ks]).status.success());
    let kart = || json(&k.join(".sezgi").join("benlik.json"));
    let ilk = kart();
    assert!(ilk.get("changes").is_none(), "önceki kart yokken changes yazılmaz");
    // Kod değişmeden yeniden: revizyon aynı, changes yok
    assert!(sezgi(&ayar, &["init", ks]).status.success());
    assert_eq!(kart()["x-revision"], ilk["x-revision"]);
    assert!(kart().get("changes").is_none());
    // Git yok, commit yok: bozan değişiklik yine de yazılır
    degistir(&k, "src/kitap/dto/kitap.dto.ts", "@IsOptional() @IsString() yazar?: string;", "@IsString() yazar!: string;");
    degistir(&k, "src/kitap/kitap.controller.ts", "@Patch(':id')", "@Patch(':id/duzenle')");
    assert!(sezgi(&ayar, &["init", ks]).status.success());
    let ikinci = kart();
    assert_ne!(ikinci["x-revision"], ilk["x-revision"]);
    let c = &ikinci["changes"];
    assert_eq!(c["previous_source_ref"], "unknown");
    let sebep = |s: &str| c["breaking"].as_array().unwrap().iter().any(|b| b["reason"].as_str().unwrap().contains(s));
    assert!(sebep("`yazar` is now required"), "{c}");
    // Yol değişikliği: aynı işlem kimliği yeni adreste; eklenen + kaldırılan değil, bozan değişiklik
    assert!(c["breaking"].as_array().unwrap().iter().any(|b| b["operation"] == "kitap.guncelle" && b["reason"].as_str().unwrap().contains("path changed")), "{c}");
    let listede = |a: &str| c[a].as_array().unwrap().iter().any(|x| x == "kitap.guncelle");
    assert!(!listede("added") && !listede("removed"), "{c}");
    // Kod değişmeden yeniden çalıştırma farkı silmez
    assert!(sezgi(&ayar, &["init", ks]).status.success());
    assert_eq!(&kart()["changes"], c);
    // karsilastir da yol değişikliğini bozan sayar
    let _ = std::fs::remove_dir_all(k.parent().unwrap());
}
