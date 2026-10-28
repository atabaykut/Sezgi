//! W2-android: APK bildirimi (ikili AXML) okuyucusu. Kabul: F-Droid Coffee 2.27 APK'sının bileşenleri yapısal çıkar;
//! karartılmış öznitelik adları kaynak kimliğinden çözülür; bozuk ya da kasıtlı kötü APK/AXML girdileri (ZIP bombası
//! dahil) paniğe yol açmaz, açık bir hata ya da not verir.
use super::*;
use proptest::prelude::*;
use std::path::PathBuf;

const COFFEE_SHA256: &str = "ae4688fe48e751517e7a1d56aed27bff10b87daac0d7e28993fb193a7dc8f223";

fn coffee_yolu() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join("w2a_coffee").join("com.github.muellerma.coffee_47.apk")
}

fn coffee() -> Vec<u8> {
    std::fs::read(coffee_yolu()).unwrap()
}

/// Coffee'nin açılmış ikili bildirimi
fn coffee_bildirimi() -> Vec<u8> {
    let b = coffee();
    let mut r = Cursor::new(&b[..]);
    let g = merkezi_dizin(&mut r, b.len() as u64).unwrap();
    let m = g.iter().find(|g| g.ad == "AndroidManifest.xml").unwrap();
    girdi_oku(&mut r, b.len() as u64, m, &mut vec![]).unwrap()
}

fn bilesen_bul<'a>(v: &'a Value, ad: &str) -> &'a Value {
    v["bilesenler"].as_array().unwrap().iter().find(|b| b["ad"] == ad).unwrap_or_else(|| panic!("{ad} yok: {:#}", v["bilesenler"]))
}

// ---- Sentetik AXML ve ZIP ----

fn le16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn le32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// UTF-8 dizgi havuzu
fn havuz(d: &[&str]) -> Vec<u8> {
    let mut veri = vec![];
    let mut ofset = vec![];
    for s in d {
        ofset.push(veri.len() as u32);
        assert!(s.len() < 128);
        veri.push(s.chars().count() as u8);
        veri.push(s.len() as u8);
        veri.extend_from_slice(s.as_bytes());
        veri.push(0);
    }
    while veri.len() % 4 != 0 {
        veri.push(0);
    }
    let bas = 28 + 4 * d.len() as u32;
    let mut p = vec![];
    le16(&mut p, 0x0001);
    le16(&mut p, 28);
    le32(&mut p, bas + veri.len() as u32);
    le32(&mut p, d.len() as u32);
    le32(&mut p, 0);
    le32(&mut p, 0x100);
    le32(&mut p, bas);
    le32(&mut p, 0);
    for o in ofset {
        le32(&mut p, o);
    }
    p.extend(veri);
    p
}

fn kimlik_haritasi(k: &[u32]) -> Vec<u8> {
    let mut p = vec![];
    le16(&mut p, 0x0180);
    le16(&mut p, 8);
    le32(&mut p, 8 + 4 * k.len() as u32);
    for x in k {
        le32(&mut p, *x);
    }
    p
}

/// Öznitelik: (ad dizgisi, değer türü, değer, ham dizgi ya da u32::MAX)
type Oz = (u32, u8, u32, u32);

fn bas(ad: u32, oz: &[Oz]) -> Vec<u8> {
    let mut p = vec![];
    le16(&mut p, 0x0102);
    le16(&mut p, 16);
    le32(&mut p, 36 + 20 * oz.len() as u32);
    le32(&mut p, 1);
    le32(&mut p, u32::MAX);
    le32(&mut p, u32::MAX);
    le32(&mut p, ad);
    le16(&mut p, 20);
    le16(&mut p, 20);
    le16(&mut p, oz.len() as u16);
    le16(&mut p, 0);
    le16(&mut p, 0);
    le16(&mut p, 0);
    for (a, tur, veri, ham) in oz {
        le32(&mut p, u32::MAX);
        le32(&mut p, *a);
        le32(&mut p, *ham);
        le16(&mut p, 8);
        p.push(0);
        p.push(*tur);
        le32(&mut p, *veri);
    }
    p
}

fn son(ad: u32) -> Vec<u8> {
    let mut p = vec![];
    le16(&mut p, 0x0103);
    le16(&mut p, 16);
    le32(&mut p, 24);
    le32(&mut p, 1);
    le32(&mut p, u32::MAX);
    le32(&mut p, u32::MAX);
    le32(&mut p, ad);
    p
}

fn axml(parcalar: &[Vec<u8>]) -> Vec<u8> {
    let govde: Vec<u8> = parcalar.concat();
    let mut p = vec![];
    le16(&mut p, 0x0003);
    le16(&mut p, 8);
    le32(&mut p, 8 + govde.len() as u32);
    p.extend(govde);
    p
}

/// (ad, yöntem, girdide duran veri, bildirilen açık boy)
fn zip(girdiler: &[(&str, u16, Vec<u8>, u32)]) -> Vec<u8> {
    let mut z = vec![];
    let mut dizin = vec![];
    for (ad, yontem, veri, acik) in girdiler {
        let yerel = z.len() as u32;
        le32(&mut z, 0x0403_4b50);
        le16(&mut z, 20);
        le16(&mut z, 0);
        le16(&mut z, *yontem);
        le32(&mut z, 0);
        le32(&mut z, 0);
        le32(&mut z, veri.len() as u32);
        le32(&mut z, *acik);
        le16(&mut z, ad.len() as u16);
        le16(&mut z, 0);
        z.extend_from_slice(ad.as_bytes());
        z.extend_from_slice(veri);
        le32(&mut dizin, 0x0201_4b50);
        le16(&mut dizin, 20);
        le16(&mut dizin, 20);
        le16(&mut dizin, 0);
        le16(&mut dizin, *yontem);
        le32(&mut dizin, 0);
        le32(&mut dizin, 0);
        le32(&mut dizin, veri.len() as u32);
        le32(&mut dizin, *acik);
        le16(&mut dizin, ad.len() as u16);
        le16(&mut dizin, 0);
        le16(&mut dizin, 0);
        le16(&mut dizin, 0);
        le16(&mut dizin, 0);
        le32(&mut dizin, 0);
        le32(&mut dizin, yerel);
        dizin.extend_from_slice(ad.as_bytes());
    }
    let konum = z.len() as u32;
    z.extend_from_slice(&dizin);
    le32(&mut z, 0x0605_4b50);
    le16(&mut z, 0);
    le16(&mut z, 0);
    le16(&mut z, girdiler.len() as u16);
    le16(&mut z, girdiler.len() as u16);
    le32(&mut z, dizin.len() as u32);
    le32(&mut z, konum);
    le16(&mut z, 0);
    z
}

/// Öznitelik adları karartılmış ("zz0"...), Android'in kullandığı kaynak kimlikleri doğru bir bildirim
fn karartilmis_bildirim() -> Vec<u8> {
    // 0-8: karartılmış öznitelik adları (kaynak kimlikleri k'de); 9-25: öğe adları ve değerler
    let d: Vec<&str> = "zz0 zz1 zz2 zz3 zz4 zz5 zz6 zz7 zz8 manifest package com.ornek uses-sdk application activity .Ana intent-filter action \
        android.intent.action.MAIN category android.intent.category.LAUNCHER provider Saglayici com.ornek.a;com.ornek.b activity-alias .Takma"
        .split_whitespace()
        .collect();
    let k = [0x0101_0003, 0x0101_021B, 0x0101_020C, 0x0101_0270, 0x0101_0010, 0x0101_04EC, 0x0101_0527, 0x0101_0018, 0x0101_0202];
    let s = |i: u32| (0u32, 0x03u8, i, i);
    axml(&[
        havuz(&d),
        kimlik_haritasi(&k),
        bas(9, &[(10, 0x03, 11, 11), (1, 0x10, 7, u32::MAX)]),
        bas(12, &[(2, 0x10, 14, u32::MAX), (3, 0x10, 16, u32::MAX)]),
        son(12),
        bas(13, &[(5, 0x12, 0xFFFF_FFFF, u32::MAX), (6, 0x01, 0x7f15_0001, u32::MAX)]),
        bas(14, &[s(15)]),
        bas(16, &[]),
        bas(17, &[s(18)]),
        son(17),
        bas(19, &[s(20)]),
        son(19),
        son(16),
        son(14),
        bas(21, &[s(22), (7, 0x03, 23, 23)]),
        son(21),
        bas(24, &[s(25), (8, 0x03, 15, 15)]),
        son(24),
        son(13),
        son(9),
    ])
}

fn bildirimli_zip(bildirim: Vec<u8>) -> Vec<u8> {
    let n = bildirim.len() as u32;
    zip(&[("AndroidManifest.xml", 0, bildirim, n), ("classes.dex", 0, b"dex\n035\0".to_vec(), 8)])
}

// ---- Kabul: Coffee 2.27 ----

#[test]
fn w2a_coffee_bildirimi_yapisal_cikar() {
    let v = apk_oku(&coffee_yolu()).unwrap();
    assert_eq!(v["paket"], "com.github.muellerma.coffee");
    assert_eq!(v["surum_kodu"], 47);
    assert_eq!(v["surum_adi"], "2.27");
    assert_eq!(v["sdk"]["en_az"], 23);
    assert_eq!(v["sdk"]["hedef"], 34);
    assert_eq!(v["guven"], "observed");
    assert_eq!(v["kanit"]["kaynak_turu"], "BINARY_METADATA");
    assert_eq!(v["kanit"]["tavan"], "observed");
    assert_eq!(v["kanit"]["dayanak"][0]["sha256"], COFFEE_SHA256);
    assert_eq!(v["dosya"]["sha256"], COFFEE_SHA256);
    assert_eq!(v["dosya"]["dex_sayisi"], 1);
    assert!(v["kanit"]["sinirlamalar"].to_string().contains("imzası doğrulanmadı"), "{}", v["kanit"]);

    let izinler: Vec<&str> = v["izinler"].as_array().unwrap().iter().map(|i| i["ad"].as_str().unwrap()).collect();
    assert_eq!(izinler.len(), 7, "{izinler:?}");
    for i in ["android.permission.FOREGROUND_SERVICE", "android.permission.WAKE_LOCK", "android.permission.RECEIVE_BOOT_COMPLETED", "android.permission.POST_NOTIFICATIONS"] {
        assert!(izinler.contains(&i), "{i}: {izinler:?}");
    }
    assert_eq!(v["tanimlanan_izinler"][0]["ad"], "com.github.muellerma.coffee.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION");
    assert_eq!(v["tanimlanan_izinler"][0]["koruma_duzeyi"], 2);

    assert_eq!(v["uygulama"]["sinif"], "com.github.muellerma.coffee.CoffeeApplication");
    assert_eq!(v["uygulama"]["yedeklenebilir"], true);
    assert_eq!(v["uygulama"]["hata_ayiklanabilir"], false);
    assert!(v["uygulama"]["etiket"].as_str().unwrap().starts_with("@0x7f"), "{}", v["uygulama"]);
    assert_eq!(v["uygulama"]["acik_metin_trafigi"], Value::Null);
    assert_eq!(v["uygulama"]["ag_guvenlik_yapilandirmasi"], Value::Null);

    // Bileşenler: 4 activity, 5 service, 2 receiver, 1 provider
    let b = v["bilesenler"].as_array().unwrap();
    let say = |t: &str| b.iter().filter(|x| x["tur"] == t).count();
    assert_eq!((say("activity"), say("service"), say("receiver"), say("provider")), (4, 5, 2, 1), "{:#}", v["bilesenler"]);

    let ana = bilesen_bul(&v, "com.github.muellerma.coffee.activities.MainActivity");
    assert_eq!(ana["disa_acik"], true);
    assert_eq!(ana["baslatici"], true);
    assert_eq!(ana["intent_filtreleri"].as_array().unwrap().len(), 2);
    assert_eq!(b.iter().filter(|x| x["baslatici"] == true).count(), 1);

    let gorunmez = bilesen_bul(&v, "com.github.muellerma.coffee.activities.CoffeeInvisibleActivity");
    assert_eq!((gorunmez["disa_acik"].clone(), gorunmez["disa_acik_belirtilmis"].clone()), (json!(false), json!(false)));

    let acilis = bilesen_bul(&v, "com.github.muellerma.coffee.BootReceiver");
    assert_eq!(acilis["tur"], "receiver");
    assert_eq!(acilis["disa_acik"], false);
    assert_eq!(acilis["intent_filtreleri"][0]["eylemler"], json!(["android.intent.action.BOOT_COMPLETED"]));

    let kutucuk = bilesen_bul(&v, "com.github.muellerma.coffee.tiles.ToggleTile");
    assert_eq!(kutucuk["izin"], "android.permission.BIND_QUICK_SETTINGS_TILE");
    assert_eq!(kutucuk["disa_acik"], true);
    assert_eq!(kutucuk["intent_filtreleri"][0]["eylemler"], json!(["android.service.quicksettings.action.QS_TILE"]));

    let on_plan = bilesen_bul(&v, "com.github.muellerma.coffee.ForegroundService");
    assert_eq!(on_plan["disa_acik"], false);
    assert!(on_plan.get("on_plan_turu").is_some(), "{on_plan}");

    let saglayici = bilesen_bul(&v, "androidx.startup.InitializationProvider");
    assert_eq!(saglayici["yetkililer"], json!(["com.github.muellerma.coffee.androidx-startup"]));
    assert_eq!(saglayici["disa_acik"], false);

    let profil = bilesen_bul(&v, "androidx.profileinstaller.ProfileInstallReceiver");
    assert_eq!(profil["izin"], "android.permission.DUMP");
    assert_eq!(profil["intent_filtreleri"].as_array().unwrap().len(), 4);
}

#[test]
fn w2a_bellekten_ve_dosyadan_ayni() {
    let a = apk_oku(&coffee_yolu()).unwrap();
    let b = apk_coz(&coffee(), "com.github.muellerma.coffee_47.apk").unwrap();
    assert_eq!(a, b);
}

// ---- Sentetik bildirimler ----

#[test]
fn w2a_karartilmis_oznitelik_adlari_kaynak_kimliginden() {
    let v = apk_coz(&bildirimli_zip(karartilmis_bildirim()), "ornek.apk").unwrap();
    assert_eq!(v["paket"], "com.ornek");
    assert_eq!(v["surum_kodu"], 7);
    assert_eq!(v["sdk"]["en_az"], 14);
    assert_eq!(v["sdk"]["hedef"], 16);
    assert_eq!(v["uygulama"]["acik_metin_trafigi"], true);
    assert_eq!(v["uygulama"]["ag_guvenlik_yapilandirmasi"], "@0x7f150001");
    let ana = bilesen_bul(&v, "com.ornek.Ana");
    assert_eq!(ana["baslatici"], true);
    // exported yazılmamış, intent filtresi var: açık
    assert_eq!((ana["disa_acik"].clone(), ana["disa_acik_belirtilmis"].clone()), (json!(true), json!(false)));
    // Hedef SDK 17'den küçük: provider varsayılan olarak açık
    let s = bilesen_bul(&v, "com.ornek.Saglayici");
    assert_eq!(s["disa_acik"], true);
    assert_eq!(s["yetkililer"], json!(["com.ornek.a", "com.ornek.b"]));
    let takma = bilesen_bul(&v, "com.ornek.Takma");
    assert_eq!(takma["tur"], "activity-alias");
    assert_eq!(takma["hedef"], "com.ornek.Ana");
    assert_eq!(v["dosya"]["dex_sayisi"], 1);
}

#[test]
fn w2a_bilinmeyen_yontem_android_gibi_sikistirmasiz() {
    let b = karartilmis_bildirim();
    let n = b.len() as u32;
    let v = apk_coz(&zip(&[("AndroidManifest.xml", 99, b, n)]), "x.apk").unwrap();
    assert_eq!(v["paket"], "com.ornek");
    assert!(v["kanit"]["sinirlamalar"].to_string().contains("bilinmeyen sıkıştırma yöntemi"), "{}", v["kanit"]);
}

#[test]
fn w2a_iki_bildirim_girdisi_not_edilir() {
    let b = karartilmis_bildirim();
    let n = b.len() as u32;
    let v = apk_coz(&zip(&[("AndroidManifest.xml", 0, b.clone(), n), ("AndroidManifest.xml", 0, b, n)]), "x.apk").unwrap();
    assert!(v["kanit"]["sinirlamalar"].to_string().contains("birden fazla AndroidManifest.xml"), "{}", v["kanit"]);
}

// ---- Bozuk ve kötü girdiler ----

fn hata(r: Result<Value, String>) -> String {
    r.expect_err("hata bekleniyordu")
}

#[test]
fn w2a_zip_bombasi_sinirda_durur() {
    let sifirlar = vec![0u8; 64 << 20];
    let bomba = miniz_oxide::deflate::compress_to_vec(&sifirlar, 6);
    assert!(bomba.len() < 1 << 20);
    // Açık boy yalan söylüyor: açma sınırı yakalar
    let t = std::time::Instant::now();
    let h = hata(apk_coz(&zip(&[("AndroidManifest.xml", 8, bomba.clone(), 1000)]), "b.apk"));
    assert!(h.contains("sınırını aşıyor"), "{h}");
    assert!(t.elapsed() < std::time::Duration::from_secs(10), "{:?}", t.elapsed());
    // Açık boy doğru: açmadan reddedilir
    let h = hata(apk_coz(&zip(&[("AndroidManifest.xml", 8, bomba, 64 << 20)]), "b.apk"));
    assert!(h.contains("ZIP bombası"), "{h}");
}

#[test]
fn w2a_bozuk_zip_acik_hata() {
    assert!(hata(apk_coz(b"", "x")).contains("merkezi dizin sonu"));
    assert!(hata(apk_coz(b"PK\x05\x06", "x")).contains("merkezi dizin sonu"));
    // Merkezi dizin dosyanın dışında
    let mut z = bildirimli_zip(karartilmis_bildirim());
    let n = z.len();
    z[n - 6..n - 2].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
    assert!(hata(apk_coz(&z, "x")).contains("dışını"));
    // ZIP64 işareti
    let mut z = bildirimli_zip(karartilmis_bildirim());
    z[n - 6..n - 2].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(hata(apk_coz(&z, "x")).contains("ZIP64"));
    // Bildirim yok
    assert!(hata(apk_coz(&zip(&[("classes.dex", 0, vec![1, 2, 3], 3)]), "x")).contains("AndroidManifest.xml yok"));
    // Bildirim ikili XML değil (düz metin XML)
    let duz = b"<manifest package=\"a\"/>".to_vec();
    assert!(hata(apk_coz(&zip(&[("AndroidManifest.xml", 0, duz, 24)]), "x")).contains("ikili XML değil"));
    // Sıkıştırılmış boy sınırın üstünde
    let mut z = zip(&[("AndroidManifest.xml", 8, vec![0; 16], 16)]);
    let cd = z.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    z[cd + 20..cd + 24].copy_from_slice(&(64u32 << 20).to_le_bytes());
    assert!(hata(apk_coz(&z, "x")).contains("sınırı aşıyor"));
}

#[test]
fn w2a_bozuk_axml_acik_hata() {
    let d = ["manifest", "a"];
    // Çok derin iç içelik
    let mut p = vec![havuz(&d)];
    p.extend((0..EN_DERIN + 5).map(|_| bas(1, &[])));
    assert!(axml_coz(&axml(&p)).unwrap_err().contains("derin"));
    // Dizgi sayısı parçaya sığmıyor
    let mut h = havuz(&d);
    h[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(axml_coz(&axml(&[h, bas(0, &[]), son(0)])).unwrap_err().contains("dizgi"));
    // Öznitelik boyu 20'den küçük
    let mut e = bas(0, &[(1, 0x03, 1, 1)]);
    e[26..28].copy_from_slice(&4u16.to_le_bytes());
    assert!(axml_coz(&axml(&[havuz(&d), e, son(0)])).unwrap_err().contains("öznitelik"));
    // Parça boyu dosyayı aşıyor
    let mut e = bas(0, &[]);
    e[4..8].copy_from_slice(&0x7FFF_FFFFu32.to_le_bytes());
    assert!(axml_coz(&axml(&[havuz(&d), e])).unwrap_err().contains("parçası geçersiz"));
    // Kök manifest değil
    assert!(bildirim(&axml_coz(&axml(&[havuz(&d), bas(1, &[]), son(1)])).unwrap()).unwrap_err().contains("manifest değil"));
    // Dizgi kimliği havuzun dışında: ad boş, panik yok
    let o = axml_coz(&axml(&[havuz(&d), bas(999, &[(777, 0x03, 555, 555)]), son(999)])).unwrap();
    assert_eq!(o.ad, "");
    // Kapanmamış öğeler (kesik dosya) yine ağaç verir
    let o = axml_coz(&axml(&[havuz(&d), bas(0, &[]), bas(1, &[])])).unwrap();
    assert_eq!((o.ad.as_str(), o.cocuklar.len()), ("manifest", 1));
}

#[test]
fn w2a_gercek_bildirimin_her_kesigi_paniksiz() {
    let b = coffee_bildirimi();
    for n in (0..b.len()).step_by(7) {
        let _ = axml_coz(&b[..n]).map(|o| bildirim(&o));
    }
    // Gerçek APK'nın her kesiği (merkezi dizin ve bildirim çevresi)
    let a = coffee();
    for n in (0..a.len()).step_by(a.len() / 300) {
        let _ = apk_coz(&a[..n], "k.apk");
    }
}

#[cfg(unix)]
#[test]
fn w2a_sembolik_bag_izlenmez() {
    let d = std::env::temp_dir().join(format!("sezgi-w2a-bag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::os::unix::fs::symlink(coffee_yolu(), d.join("bag.apk")).unwrap();
    assert!(apk_oku(&d.join("bag.apk")).unwrap_err().contains("sembolik bağ"));
    let _ = std::fs::remove_dir_all(&d);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn w2a_rastgele_bayt_paniksiz(b in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = axml_coz(&b).map(|o| bildirim(&o));
        let _ = apk_coz(&b, "r.apk");
        let mut ax = vec![3, 0, 8, 0];
        ax.extend_from_slice(&(b.len() as u32 + 8).to_le_bytes());
        ax.extend_from_slice(&b);
        let _ = axml_coz(&ax).map(|o| bildirim(&o));
    }

    #[test]
    fn w2a_bozulan_gercek_bildirim_paniksiz(degisiklik in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..24)) {
        let mut b = coffee_bildirimi();
        for (i, x) in &degisiklik {
            let n = i.index(b.len());
            b[n] = *x;
        }
        let _ = axml_coz(&b).map(|o| bildirim(&o));
        let n = b.len() as u32;
        let _ = apk_coz(&zip(&[("AndroidManifest.xml", 0, b, n)]), "m.apk");
    }

    #[test]
    fn w2a_bozulan_merkezi_dizin_paniksiz(degisiklik in proptest::collection::vec((0usize..65536, any::<u8>()), 1..16)) {
        let mut a = coffee();
        let n = a.len();
        for (i, x) in &degisiklik {
            a[n - 1 - (i % n.min(65536))] = *x;
        }
        let _ = apk_coz(&a, "m.apk");
    }
}
