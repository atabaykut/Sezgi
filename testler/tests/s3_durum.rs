//! S3: durum klasörü izinleri (Unix'te klasör 0700, dosya 0600), Linux'ta kalıcı kayıt anahtarı ve oturum (oturum
//! kapsamlı çekirdek anahtarlığına güvenilmez), `*` desenli yazma token'ı için açık onay, `canli kur` ikinci kez aynı
//! ayarlarla çalıştırılınca "zaten kurulu".
mod p0_g1_ortak;
use p0_g1_ortak::*;

fn durum_klasoru(ev: &Ev) -> std::path::PathBuf {
    std::fs::read_dir(ev.ayar().join("sezgi").join("uygulamalar")).unwrap().next().unwrap().unwrap().path()
}

#[cfg(unix)]
#[test]
fn durum_klasoru_yalnizca_kullaniciya_acik() {
    use std::os::unix::fs::PermissionsExt;
    let ev = Ev::yeni("s3-izin");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    p.kur(&u, "OTURUM-ALICE");
    p.ok(&["hafiza", "ac", "--oturumlu"]);
    let t = p.token("ajan", &["kitap.*:yikici"], &[]);
    let mut m = p.mcp(&t);
    m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    m.cagir(serde_json::json!({ "kapi": "DELETE /api/v1/kitaplar/3" }));
    let kok = durum_klasoru(&ev);
    fn gez(p: &std::path::Path, hatalar: &mut Vec<String>) {
        let kip = std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        if p.is_dir() {
            if kip != 0o700 {
                hatalar.push(format!("{} {kip:o}", p.display()));
            }
            for g in std::fs::read_dir(p).unwrap().flatten() {
                gez(&g.path(), hatalar);
            }
        } else if kip != 0o600 {
            hatalar.push(format!("{} {kip:o}", p.display()));
        }
    }
    let mut hatalar = vec![];
    gez(&kok, &mut hatalar);
    gez(&ev.ayar().join("sezgi").join("projeler"), &mut hatalar);
    assert!(hatalar.is_empty(), "yalnızca kullanıcıya açık olmalı: {hatalar:?}");
}

#[cfg(target_os = "linux")]
#[test]
fn linux_kayit_anahtari_ve_oturum_yeniden_baslatmada_kalir() {
    // Anahtarlık açık bırakılır (SEZGI_ANAHTARLIK yok): Linux çekirdek anahtarlığı oturum kapsamlıdır, yeniden
    // başlatmada silinir. Kayıt anahtarı ve oturum kullanıcıya özel dosyada olmalı.
    let ev = Ev::yeni("s3-kalici");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    let calis = |args: &[&str], girdi: Option<&str>| {
        let mut c = p.komut(args);
        c.env_remove("SEZGI_ANAHTARLIK").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let mut s = c.spawn().unwrap();
        if let Some(g) = girdi {
            use std::io::Write;
            s.stdin.as_mut().unwrap().write_all(g.as_bytes()).unwrap();
        }
        drop(s.stdin.take());
        let o = s.wait_with_output().unwrap();
        assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
        format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
    };
    calis(&["hedef", &u.adres(), "--test-kopyasi"], None);
    calis(&["mod", "okuma"], None);
    calis(&["oturum", "ac"], Some("OTURUM-KALICI\n"));
    let kok = durum_klasoru(&ev);
    assert!(kok.join("kayit.anahtar").exists(), "kayıt anahtarı kullanıcıya özel dosyada olmalı");
    let o = std::fs::read_to_string(kok.join("oturum.json")).unwrap();
    assert!(!o.contains("\"anahtarlik\":true"), "oturum yeniden başlatmada silinen anahtarlığa yazılmamalı: {o}");
    // Yeniden başlatma: yeni süreç; kayıt çözülür, oturumla okuma gider
    let t = calis(&["token", "olustur", "--ad", "ajan", "--izin", "kitap.*:oku"], None).split_whitespace().find(|w| w.starts_with("szg_")).unwrap().to_string();
    let mut m = p.mcp(&t);
    let (s, hata) = m.cagir(serde_json::json!({ "kapi": "GET /api/v1/kitaplar" }));
    assert!(!hata && s.contains("OKUNDU"), "{s}");
    assert!(calis(&["kayit", "--dogrula"], None).contains("sağlam"));
}

#[test]
fn yildiz_desenli_yazma_tokeni_acik_onay_ister() {
    let ev = Ev::yeni("s3-yildiz");
    let p = ev.proje("nestjs-temel", "a");
    let (ok, m) = p.cikti(&["token", "olustur", "--ad", "a", "--izin", "*:yaz"]);
    assert!(!ok && m.contains("--tum-islemler"), "{m}");
    assert!(!p.ok(&["token", "listele"]).contains(" a "), "onaysız token üretilmemeli");
    p.ok(&["token", "olustur", "--ad", "b", "--izin", "*:yaz", "--tum-islemler"]);
    // Yalnızca okuma için * onay istemez
    p.ok(&["token", "olustur", "--ad", "c", "--izin", "*:oku"]);
}

#[test]
fn canli_kur_ayni_ayarlarla_zaten_kurulu() {
    let ev = Ev::yeni("s3-kur");
    let u = Uygulama::yeni();
    let p = ev.proje("nestjs-temel", "a");
    let a = ["canli", "kur", "--hedef", &u.adres(), "--mod", "okuma", "--izin", "kitap.*:oku"];
    p.ok(&a);
    let (ok, m) = p.cikti(&a);
    assert!(ok && m.contains("zaten kurulu"), "{m}");
    assert_eq!(p.ok(&["token", "listele"]).matches("geçerli").count(), 1, "yeni token üretilmemeli");
    // Farklı istek: durur, hiçbir şeyi değiştirmez
    let (ok, m) = p.cikti(&["canli", "kur", "--hedef", &u.adres(), "--mod", "okuma", "--izin", "kitap.*:yaz"]);
    assert!(!ok && m.contains("--token-ad"), "{m}");
}
