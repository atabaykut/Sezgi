use super::*;
use ring::signature::KeyPair;

fn deneme_capasi(parola: &str) -> (Capa, Anahtar) {
    let tuz = [7u8; 16];
    let cift = turet(parola, &tuz, 1000).unwrap();
    let acik = cift.public_key().as_ref().to_vec();
    let c = Capa { yol: PathBuf::from("/yok"), izi: parmak_izi(&acik), acik, tuz: tuz.to_vec(), yineleme: 1000, kuruldu: String::new() };
    let a = ac(&c, parola).expect("doğru parola anahtarı açar");
    (c, a)
}

#[test]
fn parola_acik_anahtari_verir_yanlisi_vermez() {
    let (c, _) = deneme_capasi("uzun-bir-insan-parolasi");
    assert!(ac(&c, "uzun-bir-insan-parolasi").is_some());
    assert!(ac(&c, "uzun-bir-insan-parolasI").is_none());
    assert!(ac(&c, "").is_none());
    // Aynı parola, başka tuz: başka anahtar
    let baska = turet("uzun-bir-insan-parolasi", &[8u8; 16], 1000).unwrap();
    assert_ne!(baska.public_key().as_ref(), c.acik.as_slice());
}

#[test]
fn imza_kayda_ve_ture_baglidir() {
    let (c, a) = deneme_capasi("uzun-bir-insan-parolasi");
    let v = json!({ "proje": "p-1", "url": "http://127.0.0.1:1", "gercek_veri": true });
    let i = imzala(&a, "hedef", &v);
    assert!(dogrula(&c, "hedef", &v, &i));
    // Tek alan değişirse, başka bir tür kayıtta ya da başka bir anahtarla tutmaz
    assert!(!dogrula(&c, "hedef", &json!({ "proje": "p-1", "url": "http://127.0.0.1:2", "gercek_veri": true }), &i));
    assert!(!dogrula(&c, "token", &v, &i));
    let (c2, _) = deneme_capasi("baska-bir-insan-parolasi");
    assert!(!dogrula(&c2, "hedef", &v, &i));
    // İmzasız ya da bozuk imza
    assert!(!dogrula(&c, "hedef", &v, &Value::Null));
    assert!(!dogrula(&c, "hedef", &v, &json!({ "imza": "AAAA" })));
}

#[test]
fn capa_dosya_adi_guvenli() {
    assert_eq!(dosya_adi("1000"), "insan-1000.json");
    assert_eq!(dosya_adi("../x y"), "insan-.._x_y.json");
}

// ---- d9 cila: `--kullanici <ad>` (Unix) ve Windows çapa izinleri ----

#[test]
fn d9_cila_passwd_adindan_uid() {
    let passwd = "# yorum\nroot:x:0:0:root:/root:/bin/bash\nnode:x:1000:1000::/home/node:/bin/bash\nbozuk:x::1:\nnobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin\n";
    assert_eq!(uid_bul(passwd, "root").as_deref(), Some("0"));
    assert_eq!(uid_bul(passwd, "node").as_deref(), Some("1000"));
    assert_eq!(uid_bul(passwd, "nobody").as_deref(), Some("65534"));
    // Önek eşleşmesi, boş uid ve olmayan ad kabul edilmez
    assert_eq!(uid_bul(passwd, "nod"), None);
    assert_eq!(uid_bul(passwd, "bozuk"), None);
    assert_eq!(uid_bul(passwd, "yok"), None);
    assert_eq!(uid_bul(passwd, "# yorum"), None);
}

/// Windows: çapanın izin listesi (yönetici gerektiren kurulum yolu otomatik denenemez; listenin üretimi, okunması ve
/// kullanıcıya ait bir klasöre uygulanması denenir)
#[cfg(windows)]
mod d9_cila_pencere {
    use super::super::pencere::*;
    use windows_sys::Win32::Security::{WinBuiltinAdministratorsSid, WinBuiltinUsersSid, WinLocalSystemSid};

    #[test]
    fn siki_izin_yalnizca_yoneticiye_yazma_verir() {
        assert_eq!(OKUMA & DEGISTIRIR, 0, "kullanıcının okuma izni değiştirme hakkı içermemeli");
        assert_ne!(TAM & DEGISTIRIR, 0);
        for klasor in [true, false] {
            let a = siki_dacl(klasor).unwrap();
            // SAFETY: a geçerli bir izin listesi
            assert_eq!(unsafe { yabanci_yazan(a.ptr()) }, None);
        }
        // Users'a yazma veren liste yabancı yazan sayılır; liste yoksa herkes tam denetim
        let (y, s, u) = (Sid::bilinen(WinBuiltinAdministratorsSid).unwrap(), Sid::bilinen(WinLocalSystemSid).unwrap(), Sid::bilinen(WinBuiltinUsersSid).unwrap());
        let gevsek = dacl_kur(&[(&y, TAM), (&s, TAM), (&u, OKUMA | 0x2)], true).unwrap();
        // SAFETY: gevsek geçerli bir izin listesi
        assert_eq!(unsafe { yabanci_yazan(gevsek.ptr()) }.as_deref(), Some("S-1-5-32-545"));
        // SAFETY: boş liste (NULL DACL)
        assert!(unsafe { yabanci_yazan(std::ptr::null()) }.is_some());
    }

    #[test]
    fn kullanicinin_klasoru_sikilasinca_yazilamaz() {
        let k = std::env::temp_dir().join(format!("sezgi-d9-cila-dacl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&k);
        std::fs::create_dir_all(&k).unwrap();
        // Kullanıcının oluşturduğu klasör: sahibi kullanıcı, kullanıcıya yazma hakkı (miras)
        let (sahip_yonetici, _, yabanci) = guvenlik_oku(&k).unwrap();
        assert!(!sahip_yonetici && yabanci.is_some(), "{yabanci:?}");
        assert!(izin_denetimi(&k).is_err());
        // Sıkı izin (sahip değiştirilmeden; yönetici olmayan süreç sahibi Administrators yapamaz)
        izin_yaz(&k, &siki_dacl(true).unwrap(), None).unwrap();
        let (sahip_yonetici, _, yabanci) = guvenlik_oku(&k).unwrap();
        assert_eq!(yabanci, None, "miras kesilmeli, yalnızca Administrators ve SYSTEM yazabilmeli");
        let e = izin_denetimi(&k).unwrap_err();
        assert!(!sahip_yonetici && (e.contains("sahibi") || e.contains("owner")), "{e}");
        // Kullanıcı artık klasöre dosya ekleyemez
        assert!(std::fs::write(k.join("insan-x.json"), "{}").is_err(), "sıkı klasöre kullanıcı dosya eklememeli");
        // Sahibi yönetici değilken sıkılaştırma sahibi değiştirmeye çalışır; yönetici olmayan süreçte açık hata verir
        if let Err(e) = sikilastir(&k, true) {
            assert!(e.contains("takeown"), "{e}");
        }
        // Temizlik: sahip olarak izni geri aç
        let u = Sid::bilinen(WinBuiltinUsersSid).unwrap();
        izin_yaz(&k, &dacl_kur(&[(&u, TAM)], true).unwrap(), None).unwrap();
        std::fs::remove_dir_all(&k).unwrap();
    }
}
