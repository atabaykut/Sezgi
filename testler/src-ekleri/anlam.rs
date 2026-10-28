// d8 anlam: sözlük, yetenek adlandırma, alan sınıflandırması ve görev ayrıştırmasının birim testleri
use super::*;
use serde_json::json;

fn kapi(yontem: &str, yol: &str, fonksiyon: &str) -> Value {
    json!({ "id": format!("{yontem} {yol}"), "yontem": yontem, "yol": yol, "fonksiyon": fonksiyon, "kontrolcu": "", "giris": "gerekli" })
}

#[test]
fn turkce_ekli_kelime_kokune_iner() {
    assert_eq!(sade("Kullanıcıyı SİL"), "kullaniciyi sil");
    let (k, ek) = kavramlar("kullanıcıyı");
    assert!(k.iter().any(|x| x.id == "user"), "kullanıcıyı -> user");
    assert_eq!(ek.as_deref(), Some("yi"));
    let (k, _) = kavramlar("albümlerini");
    assert!(k.iter().any(|x| x.id == "album"));
    // Ek gibi görünmeyen devam kök sayılmaz: "araba" arama değildir
    assert!(kavramlar("araba").0.is_empty());
    assert_eq!(tekil("libraries"), "library");
    assert_eq!(tekil("boxes"), "box");
    assert_eq!(tekil("status"), "status");
    assert_eq!(tekil("people"), "person");
}

#[test]
fn turkce_istek_fiili_http_yontemine_cevrilir() {
    let s = sorgu("kullanıcı sil");
    assert!(s.yontemler.contains(&"DELETE"));
    let sil = kapi("DELETE", "/api/users/:id", "deleteUser");
    let oku = kapi("GET", "/api/users/:id", "getUser");
    assert!(puan(&sil, &s, &[]) > puan(&oku, &s, &[]), "silme okumadan önce");
    // İngilizce istek Türkçe adlı kodu bulur
    let olustur = kapi("POST", "/kitaplar/", "olustur");
    assert!(puan(&olustur, &sorgu("create book"), &[]) > 0.0);
    // Varlığı uygulamada olmayan istek fiilden sonuç üretmez
    assert_eq!(puan(&kapi("POST", "/api/memories", "createMemory"), &sorgu("sipariş oluştur"), &[]), 0.0);
}

#[test]
fn yetenek_adlari_is_dilinde() {
    let y = yetenek(&kapi("PUT", "/api/albums/:id/users", "addUsersToAlbum"));
    assert_eq!((y.anahtar.as_str(), y.ad.as_str(), y.eylem.as_str()), ("album.share", "albüm paylaş", "share"));
    let y = yetenek(&kapi("POST", "/api/assets", "uploadAsset"));
    assert_eq!((y.anahtar.as_str(), y.ad.as_str()), ("asset.upload", "medya yükle"));
    let y = yetenek(&kapi("GET", "/api/albums", "getAllAlbums"));
    assert_eq!(y.anahtar, "album.list");
    let y = yetenek(&kapi("DELETE", "/api/admin/users/:id", "deleteUserAdmin"));
    assert_eq!((y.anahtar.as_str(), y.ad.as_str()), ("user.delete", "kullanıcı sil"));
    let y = yetenek(&kapi("PUT", "/api/albums/:id/assets", "addAssetsToAlbum"));
    assert_eq!((y.anahtar.as_str(), y.ad.as_str()), ("album.add-asset", "albüm: medya ekle"));
    let y = yetenek(&kapi("POST", "/api/v1/login/access-token", "login_access_token"));
    assert_eq!((y.anahtar.as_str(), y.ad.as_str()), ("auth.login", "giriş yap"));
    // Laravel adları: store / destroy
    assert_eq!(yetenek(&kapi("DELETE", "/api/songs/:song", "destroy")).anahtar, "song.delete");
    // Yolda isim yoksa yükleme dosyanın işidir (ana sayfanın değil)
    assert_eq!(yetenek(&kapi("POST", "/api/upload", "postApiUpload")).ad, "dosya yükle");
    // Yol parçasında fiil ve isim birlikte
    assert_eq!(yetenek(&kapi("POST", "/shelves/:slug/create-book", "store")).anahtar, "shelve.add-book");
}

#[test]
fn alan_kanit_yetmezse_bilinmiyor() {
    let m = |kaynak: &'static str, ws: &[&str]| -> Metin { (kaynak, "x".into(), 1, ws.iter().map(|w| w.to_string()).collect(), None, false) };
    // Tek kaynak türü (yalnızca yol adları) yetmez
    let s = siniflandir(&[m("rota", &["article", "comment", "favorite", "follow", "tag"])]);
    assert!(kabul(&s).is_err());
    // Belge + tablo: kabul
    let s = siniflandir(&[m("belge", &["photo", "video", "library"]), m("tablo", &["album", "exif"])]);
    assert_eq!(ALANLAR[kabul(&s).unwrap()].id, "photo-video");
    // İki alan yarışıyorsa bilinmiyor
    let s = siniflandir(&[m("belge", &["photo", "video", "music", "song"]), m("tablo", &["exif", "artist"])]);
    assert!(kabul(&s).unwrap_err().contains("yarışıyor"), "yarış");
}

#[test]
fn readme_kurulum_bolumunde_biter() {
    let mut v = vec![];
    readme_satirlari("# Ad\n\n[![rozet](x)](y)\n\nAd is a photo library\nfor families.\n\n## Install\n\nThe shop cart\n", "README.md", &mut v);
    let metinler: Vec<&str> = v.iter().map(|s| s.metin.as_str()).collect();
    assert!(metinler.contains(&"Ad is a photo library for families."), "{metinler:?}");
    assert!(!metinler.iter().any(|m| m.contains("cart")), "kurulum bölümü okunmaz: {metinler:?}");
    assert_eq!(v.iter().find(|s| s.metin.starts_with("Ad is")).unwrap().satir, 5);
}

#[test]
fn gorev_cumlelere_ayrilir() {
    assert_eq!(cumleler("kullanıcıyı sil ve albümlerini başkasına devret"), vec!["kullanıcıyı sil", "albümlerini başkasına devret"]);
    let c = cumle_coz("albümlerini başkasına devret");
    assert_eq!(c.eylem.map(|e| e.id), Some("transfer"));
    assert!(c.varliklar.iter().any(|v| v.kavram.is_some_and(|k| k.id == "album") && v.iyelik));
    assert!(cok_adimli("sipariş oluştur ve öde"));
    assert!(!cok_adimli("kullanıcı sil"));
}
