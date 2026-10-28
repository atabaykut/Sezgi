//! D8 çağrı: fonksiyon çağrı grafiği ve derin servis analizi (vizyon soruları 8, 9, 11).
//! - Kenar `fn:A -[CALLS]-> fn:B` kanıt deposuna dosya:satır kanıtıyla girer. Tek anlamlı statik hedef (aynı sınıf, içe
//!   aktarma, enjekte edilen servisin tipi, bağımlılık kaydı) certain; arayüzün tek uygulaması, kapsayıcı, MediatR ve ad
//!   eşleşmesi inferred; hedefi bilinmeyen (dinamik) çağrı kenar değildir, sayı olarak kalır.
//! - İşlemden servis ve depo katmanına inilir: tablo etkisi (Kysely, Prisma, SQLAlchemy, Eloquent, ActiveRecord, GORM,
//!   JPA, EF Core, ham SQL), erişim denetimi (`requireAccess`, `authorize`, `@PreAuthorize`) ve sahiplik karşılaştırması.
//! - Sahiplik yalnızca kaydın sahip alanı ile istekteki kimliğin karşılaştırılmasıdır: 409 benzersizlik kontrolü ve
//!   "kendi hesabını silemezsin" sahiplik değildir.
//! - Görünürlük: `tani` çağrı zinciri, `onizle` nesne düzeyi maddeleri, `etki` fonksiyon kümesi, `rapor`, Ayna soruları.
mod p0_g1_ortak;
use p0_g1_ortak::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ornek(ad: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ornekler").join(ad)
}

fn haritala(ad: &str) -> PathBuf {
    static SAYAC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SAYAC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let cikti = std::env::temp_dir().join(format!("sezgi-d8cagri-{ad}-{}-{n}", std::process::id()));
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

/// Kapının kimliği (yöntem ve yolun parçasıyla bulunur): (kapı, `api:` düğümü)
fn kapi(h: &Path, yontem: &str, yol_parcasi: &str) -> (Value, String) {
    let harita = json_oku(&h.join("kapilar.json"));
    let k = harita["endpointler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["yontem"] == yontem && k["yol"].as_str().unwrap().contains(yol_parcasi))
        .unwrap_or_else(|| panic!("{yontem} {yol_parcasi} yok: {:#}", harita["endpointler"].as_array().unwrap().iter().map(|k| k["id"].clone()).collect::<Vec<_>>().len()))
        .clone();
    let api = format!("api:{} {}", k["yontem"].as_str().unwrap(), k["yol"].as_str().unwrap());
    (k, api)
}

fn dna(h: &Path, k: &Value) -> Value {
    let harita = json_oku(&h.join("kapilar.json"));
    harita["ozet"]["dna"][format!("{}:{}", k["dosya"].as_str().unwrap(), k["satir"]).as_str()].clone()
}

fn onermeler(depo: &Value, varlik: &str, yuklem: &str) -> Vec<Value> {
    depo["onermeler"].as_array().unwrap().iter().filter(|o| o["varlik"] == varlik && o["yuklem"] == yuklem).cloned().collect()
}

/// fn -> fn CALLS önermesi (kaynak ve hedef kimliğinin sonuyla)
fn kenar(depo: &Value, kaynak_sonu: &str, hedef_sonu: &str) -> Value {
    depo["onermeler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| {
            o["yuklem"] == "CALLS"
                && o["varlik"].as_str().unwrap().starts_with("fn:")
                && o["varlik"].as_str().unwrap().ends_with(kaynak_sonu)
                && o["hedef"].as_str().unwrap_or("").ends_with(hedef_sonu)
        })
        .cloned()
        .unwrap_or_else(|| {
            let v: Vec<String> = depo["onermeler"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["yuklem"] == "CALLS" && o["varlik"].as_str().unwrap().starts_with("fn:"))
                .map(|o| format!("{} -> {}", o["varlik"], o["hedef"]))
                .collect();
            panic!("{kaynak_sonu} -> {hedef_sonu} kenarı yok; kenarlar: {v:#?}")
        })
}

/// Önermeyi destekleyen kanıtlar
fn kanitlar<'a>(depo: &'a Value, o: &Value) -> Vec<&'a Value> {
    let ids: Vec<&str> = o["hesap"]["destek"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    depo["kanitlar"].as_array().unwrap().iter().filter(|k| ids.contains(&k["id"].as_str().unwrap())).collect()
}

fn arac(h: &Path, ad: &str, arguman: Value) -> String {
    let mut p = Command::new(env!("CARGO_BIN_EXE_sezgi")).args(["mcp", h.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let istek = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": ad, "arguments": arguman } });
    writeln!(p.stdin.take().unwrap(), "{istek}").unwrap();
    let c = String::from_utf8_lossy(&p.wait_with_output().unwrap().stdout).to_string();
    let v: Value = serde_json::from_str(c.lines().next().unwrap_or("{}")).unwrap();
    v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

#[test]
fn nest_kysely_cagri_grafigi_ve_tablo_etkisi() {
    let h = haritala("d8_cagri_nest");
    let depo = json_oku(&h.join("kanit.json"));
    // Denetleyici -> servis: kurucu enjeksiyonu (alanın tipi), tek hedef, certain; kanıt çağrının satırı
    let e = kenar(&depo, "album.controller.ts#deleteAlbum", "src/services/album.service.ts#AlbumService.delete");
    assert_eq!(e["hesap"]["guven"], "certain", "{e:#}");
    assert_eq!(e["deger"]["konum"], "src/controllers/album.controller.ts:18", "{e:#}");
    assert_eq!(e["deger"]["cozum"], "alan_tipi", "{e:#}");
    let k = kanitlar(&depo, &e);
    assert!(k.iter().any(|k| k["konum"].as_array().unwrap().iter().any(|x| x == "src/controllers/album.controller.ts:18") && k["kaynak_turu"] == "SOURCE_CODE"), "{k:#?}");
    // Servis -> depo: üst sınıfın kurucusunda enjekte edilen alan; servis -> üst sınıfın yöntemi
    assert_eq!(kenar(&depo, "AlbumService.delete", "album.repository.ts#AlbumRepository.delete")["hesap"]["guven"], "certain");
    assert_eq!(kenar(&depo, "AlbumService.delete", "base.service.ts#BaseService.requireAccess")["hesap"]["guven"], "certain");
    // Tablo etkisi: Kysely deleteFrom('album') -> WRITES db:album; tablo tanımı @Table({ name: 'album' })
    let (_, api) = kapi(&h, "DELETE", "/albums/");
    let w = onermeler(&depo, &api, "WRITES");
    assert!(w.iter().any(|o| o["hedef"] == "db:album" && o["hesap"]["guven"] == "inferred"), "{w:#?}");
    let t = onermeler(&depo, "db:album", "TABLO");
    assert!(t.iter().any(|o| o["deger"]["tablo"] == "album"), "Kysely tablo sınıfı tablo düğümü olmalı: {t:#?}");
    // Erişim denetimi fonksiyonunun içi (izin sorgusu) işlemin tablo etkisi sayılmaz
    assert!(onermeler(&depo, &api, "READS").iter().all(|o| o["hedef"] != "db:album_user"), "requireAccess'in içi izlenmemeli");
    let (_, oku) = kapi(&h, "GET", "/albums/");
    let r = onermeler(&depo, &oku, "READS");
    assert!(r.iter().any(|o| o["hedef"] == "db:album") && r.iter().any(|o| o["hedef"] == "db:album_user"), "{r:#?}");
    // Yan bilgi ve graf görünümü
    let c = &depo["ek"]["cagri"];
    assert_eq!(c["bicim"], 1, "{c:#}");
    assert!(c["kenar"].as_u64().unwrap() >= 6 && c["kesin"].as_u64().unwrap() >= 6, "{c:#}");
    let graf = json_oku(&h.join("graf.json"));
    assert!(graf["kenarlar"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k["iliski"] == "CALLS" && k["kaynak"].as_str().unwrap().starts_with("fn:") && k["hedef"].as_str().unwrap().starts_with("fn:")));
    // Davranış özetinde zincir
    let (k, _) = kapi(&h, "DELETE", "/albums/");
    let d = dna(&h, &k);
    let z: Vec<&str> = d["cagri"]["zincir"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert_eq!(z, ["AlbumController.deleteAlbum", "AlbumService.delete", "AlbumRepository.delete"], "{d:#}");
    assert_eq!(d["cagri"]["zincir_guveni"], "certain");
    // Benlik kartı: nesne düzeyi yetki ve çağrı zinciri `x-dna`'da
    let kart = json_oku(&h.join("benlik.json"));
    let op = kart["operations"].as_array().unwrap().iter().find(|o| o["entry"]["method"] == "DELETE" && o["entry"]["path"] == "/api/albums/:id").unwrap();
    let ol = op["x-dna"]["auth"]["object_level"].as_array().unwrap_or_else(|| panic!("{:#}", op["x-dna"]));
    assert!(ol.iter().any(|x| x["kind"] == "access_check" && x["permission"] == "album.delete" && x["stamp"] == "inferred"), "{ol:#?}");
    assert_eq!(op["x-dna"]["call_chain"]["functions"][1], "AlbumService.delete", "{:#}", op["x-dna"]);
}

#[test]
fn nest_requireaccess_nesne_yetkisi_tani_onizle_etki() {
    let h = haritala("d8_cagri_nest");
    let depo = json_oku(&h.join("kanit.json"));
    let (_, api) = kapi(&h, "DELETE", "/albums/");
    let n = onermeler(&depo, &api, "NESNE_YETKI");
    let o = n.iter().find(|o| o["deger"]["izin"] == "album.delete").unwrap_or_else(|| panic!("requireAccess nesne yetkisi yok: {n:#?}"));
    assert_eq!(o["hedef"], "src/services/album.service.ts:17", "{o:#}");
    assert_eq!(o["hesap"]["guven"], "inferred", "servis zincirindeki denetim inferred: {o:#}");
    let id = api.trim_start_matches("api:").replace(":id", "a1");
    let on = arac(&h, "onizle", json!({ "kapi": id }));
    assert!(on.contains("yetki (nesne düzeyi)") && on.contains("requireAccess"), "{on}");
    assert!(!on.contains("GEÇER (bilinen bütün kurallar)"), "{on}");
    let t = arac(&h, "tani", json!({ "kapi": api.trim_start_matches("api:") }));
    assert!(t.contains("çağrı zinciri [Kesin]: AlbumController.deleteAlbum → AlbumService.delete → AlbumRepository.delete"), "{t}");
    assert!(t.contains("nesne düzeyi erişim denetimi") && t.contains("izin album.delete"), "{t}");
    let e = arac(&h, "etki", json!({ "kapi": api.trim_start_matches("api:") }));
    assert!(e.contains("fonksiyonlar (çağrı zinciri)") && e.contains("album"), "{e}");
}

#[test]
fn kendi_hesabini_silemezsin_sahiplik_degil_sahip_alani_karsilastirmasi_sahiplik() {
    let h = haritala("d8_cagri_nest");
    let depo = json_oku(&h.join("kanit.json"));
    // "Cannot delete your own account": çağıranın kendisiyle karşılaştırma, sahiplik değil
    let (k, api) = kapi(&h, "DELETE", "/admin/users/");
    assert!(onermeler(&depo, &api, "SAHIPLIK").is_empty(), "{:#?}", onermeler(&depo, &api, "SAHIPLIK"));
    let d = dna(&h, &k);
    let red = d["hatalar"].as_array().unwrap().iter().find(|x| x["durum"] == 403).unwrap_or_else(|| panic!("403 yok: {d:#}"));
    assert_eq!(red["sahiplik"], false, "{red}");
    assert!(onermeler(&depo, &api, "WRITES").iter().any(|o| o["hedef"] == "db:user"));
    // Kaydın sahip alanı ile istekteki kimlik: sahiplik (servis zincirinde inferred); yanıt eşlemesindeki karşılaştırma sayılmaz
    let (_, api) = kapi(&h, "PATCH", "/albums/");
    let s = onermeler(&depo, &api, "SAHIPLIK");
    assert!(!s.is_empty(), "sahiplik bulunmalı");
    assert!(s.iter().all(|o| !o["hedef"].as_str().unwrap().ends_with(":26")), "nesne değerindeki karşılaştırma sahiplik denetimi değil: {s:#?}");
}

#[test]
fn dinamik_cagri_bilinmiyor_ve_nesne_yetkisi_bilinmeyen_yazma_onaya_duser() {
    let h = haritala("d8_cagri_nest");
    let (k, _) = kapi(&h, "POST", "/copy");
    let d = dna(&h, &k);
    assert!(d["cagri"]["dinamik"].as_u64().unwrap() >= 1, "{d:#}");
    let on = arac(&h, "onizle", json!({ "kapi": "POST /api/albums/a1/copy" }));
    assert!(on.contains("çağrı zinciri tam çözülemedi"), "{on}");
    // Canlı mod: nesne düzeyi yetkisi bilinmeyen yazma onaya düşer, uygulamaya istek gitmez
    let ev = Ev::yeni("d8-cagri-onay");
    let u = Uygulama::yeni();
    let p = ev.proje("d8_cagri_nest", "a");
    p.kur(&u, "OTURUM-ALICE");
    let t = p.token("ajan", &["*:yaz"], &["--tum-islemler"]);
    let (s, hata) = p.mcp(&t).cagir(json!({ "kapi": "POST /api/albums/a1/copy" }));
    assert!(!hata && s.contains("ONAY BEKLİYOR") && (s.contains("nesne düzeyi") || s.contains("dış etkisi bilinmiyor")), "{s}");
    assert_eq!(u.sayi(), 0);
}

#[test]
fn fastapi_409_benzersizlik_sahiplik_degil_depends_sinifi_kesin() {
    let h = haritala("d8_cagri_fastapi");
    let depo = json_oku(&h.join("kanit.json"));
    // 409: kaydın kimliği yol parametresiyle karşılaştırılıyor; sahiplik değil
    let (k, api) = kapi(&h, "PATCH", "/users/");
    assert!(onermeler(&depo, &api, "SAHIPLIK").is_empty(), "{:#?}", onermeler(&depo, &api, "SAHIPLIK"));
    let d = dna(&h, &k);
    let r409 = d["hatalar"].as_array().unwrap().iter().find(|x| x["durum"] == 409).unwrap_or_else(|| panic!("409 yok: {d:#}"));
    assert_eq!(r409["sahiplik"], false, "{r409}");
    // `from app import crud` ile modül fonksiyonu: certain
    let e = kenar(&depo, "users.py#updateUser", "app/crud.py#update_user");
    assert_eq!(e["hesap"]["guven"], "certain", "{e:#}");
    // Gerçek sahiplik: certain
    let (_, api) = kapi(&h, "DELETE", "/items/");
    let s = onermeler(&depo, &api, "SAHIPLIK");
    assert!(s.iter().any(|o| o["hesap"]["guven"] == "certain"), "{s:#?}");
    // Depends(ItemService): parametrenin tipi sınıf, tek hedef
    let e = kenar(&depo, "items.py#deleteItem", "app/services/items.py#ItemService.remove");
    assert_eq!(e["hesap"]["guven"], "certain", "{e:#}");
    assert_eq!(e["deger"]["cozum"], "parametre_tipi", "{e:#}");
    // Serviste silinen model (parametre tipi Item): tablo etkisi zincirden
    let w = onermeler(&depo, &api, "WRITES");
    let o = w.iter().find(|o| o["hedef"] == "db:Item").unwrap_or_else(|| panic!("{w:#?}"));
    assert!(kanitlar(&depo, o).iter().any(|k| k["ozet"]["tur"] == "tablo_etkisi" && k["ozet"]["zincir"].as_array().unwrap().iter().any(|z| z == "ItemService.remove")), "{o:#}");
}

#[test]
fn express_satir_ici_isleyici_prisma_ham_sql_ve_dinamik() {
    let h = haritala("d8_cagri_express");
    let depo = json_oku(&h.join("kanit.json"));
    let (k, api) = kapi(&h, "DELETE", "/articles/");
    let d = dna(&h, &k);
    assert_eq!(d["cagri"]["isleyici"], format!("fn:{}#{}", k["dosya"].as_str().unwrap(), k["fonksiyon"].as_str().unwrap()), "{d:#}");
    assert!(d["cagri"]["dinamik"].as_u64().unwrap() >= 1, "{d:#}");
    let z = d["cagri"]["zincir"].as_array().unwrap();
    assert!(z.first().unwrap() == k["fonksiyon"].as_str().unwrap() && z.iter().any(|x| x == "deleteArticle"), "{d:#}");
    let w = onermeler(&depo, &api, "WRITES");
    assert!(w.iter().any(|o| o["hedef"] == "db:comments"), "ham SQL DELETE FROM comments: {w:#?}");
    let r = onermeler(&depo, &api, "READS");
    assert!(r.iter().any(|o| o["hedef"] == "db:Article"), "Prisma article.findUnique: {r:#?}");
    // İstekteki kimlik yalnızca adından (userId): sahiplik inferred
    let s = onermeler(&depo, &api, "SAHIPLIK");
    assert!(!s.is_empty() && s.iter().all(|o| o["hesap"]["guven"] == "inferred"), "{s:#?}");
}

#[test]
fn laravel_authorize_eloquent_kapsayici_sorgu_kurucusu() {
    let h = haritala("d8_cagri_laravel");
    let depo = json_oku(&h.join("kanit.json"));
    let (_, guncelle) = kapi(&h, "PUT", "/posts/");
    let n = onermeler(&depo, &guncelle, "NESNE_YETKI");
    assert!(n.iter().any(|o| o["hesap"]["guven"] == "certain" && o["deger"]["ifade"].as_str().unwrap().contains("authorize")), "{n:#?}");
    assert_eq!(kenar(&depo, "PostController.php#update", "PostService.php#PostService.rename")["hesap"]["guven"], "certain");
    assert!(onermeler(&depo, &guncelle, "WRITES").iter().any(|o| o["hedef"] == "db:Post"), "{:#?}", onermeler(&depo, &guncelle, "WRITES"));
    let (_, sil) = kapi(&h, "DELETE", "/posts/");
    assert!(onermeler(&depo, &sil, "SAHIPLIK").iter().any(|o| o["hesap"]["guven"] == "certain"), "{:#?}", onermeler(&depo, &sil, "SAHIPLIK"));
    // app(AuditService::class): kapsayıcıdan çözülen sınıf, inferred
    assert_eq!(kenar(&depo, "PostController.php#destroy", "AuditService.php#AuditService.record")["hesap"]["guven"], "inferred");
    assert!(onermeler(&depo, &sil, "WRITES").iter().any(|o| o["hedef"] == "db:AuditLog"));
    let (_, stats) = kapi(&h, "GET", "/stats");
    assert!(onermeler(&depo, &stats, "READS").iter().any(|o| o["hedef"] == "db:post_stats"), "{:#?}", onermeler(&depo, &stats, "READS"));
}

#[test]
fn rails_before_action_ve_activerecord() {
    let h = haritala("d8_cagri_rails");
    let depo = json_oku(&h.join("kanit.json"));
    // before_action :set_article (only) ve üst sınıfın before_action :authenticate_user!
    assert_eq!(kenar(&depo, "#update", "articles_controller.rb#ArticlesController.set_article")["hesap"]["guven"], "certain");
    assert_eq!(kenar(&depo, "#update", "application_controller.rb#ApplicationController.authenticate_user!")["hesap"]["guven"], "certain");
    let harita = json_oku(&h.join("kapilar.json"));
    let guncelle = harita["endpointler"].as_array().unwrap().iter().find(|k| k["fonksiyon"] == "update").unwrap().clone();
    let api = format!("api:{} {}", guncelle["yontem"].as_str().unwrap(), guncelle["yol"].as_str().unwrap());
    assert!(onermeler(&depo, &api, "SAHIPLIK").iter().any(|o| o["hesap"]["guven"] == "certain"), "{:#?}", onermeler(&depo, &api, "SAHIPLIK"));
    assert!(onermeler(&depo, &api, "WRITES").iter().any(|o| o["hedef"] == "db:Article"), "{:#?}", onermeler(&depo, &api, "WRITES"));
    // ArticleCleanup.new(@article).call: kurucudan tipi bilinen nesne
    assert_eq!(kenar(&depo, "#destroy", "article_cleanup.rb#ArticleCleanup.call")["hesap"]["guven"], "certain");
    let (_, sil) = kapi(&h, "DELETE", "/articles/");
    assert!(onermeler(&depo, &sil, "WRITES").iter().any(|o| o["hedef"] == "db:Comment"), "{:#?}", onermeler(&depo, &sil, "WRITES"));
}

#[test]
fn go_gin_gorm_servis_alani() {
    let h = haritala("d8_cagri_go");
    let depo = json_oku(&h.join("kanit.json"));
    let (_, api) = kapi(&h, "DELETE", "/items/");
    assert_eq!(kenar(&depo, "#Delete", "internal/service/item.go#ItemService.Delete")["hesap"]["guven"], "certain");
    let w = onermeler(&depo, &api, "WRITES");
    assert!(w.iter().any(|o| o["hedef"] == "db:Item"), "{w:#?}");
    assert!(onermeler(&depo, "db:Item", "TABLO").iter().any(|o| o["deger"]["tablo"] == "esyalar"), "TableName() tablo adı");
    let s = onermeler(&depo, &api, "SAHIPLIK");
    assert!(!s.is_empty() && s.iter().all(|o| o["hesap"]["guven"] == "inferred"), "{s:#?}");
}

#[test]
fn spring_jpa_preauthorize_ve_arayuzun_tek_uygulamasi() {
    let h = haritala("d8_cagri_spring");
    let depo = json_oku(&h.join("kanit.json"));
    let (_, api) = kapi(&h, "DELETE", "/notes/");
    // Arayüz tipli alan: projedeki tek uygulama, inferred
    let e = kenar(&depo, "NoteController.java#delete", "NoteServiceImpl.java#NoteServiceImpl.delete");
    assert_eq!(e["hesap"]["guven"], "inferred", "{e:#}");
    assert_eq!(e["deger"]["cozum"], "arayuz_tek_uygulama", "{e:#}");
    // MyBatis XML eşleyicisi: eşleyici arayüzünün deyiminin SQL'i
    let w = onermeler(&depo, &api, "WRITES");
    let o = w.iter().find(|o| o["hedef"] == "db:note_audit").unwrap_or_else(|| panic!("MyBatis insert: {w:#?}"));
    assert!(kanitlar(&depo, o).iter().any(|k| k["ozet"]["kaynak"] == "mybatis"), "{o:#}");
    let w = onermeler(&depo, &api, "WRITES");
    assert!(w.iter().any(|o| o["hedef"] == "db:Note"), "JpaRepository<Note, Long>.delete: {w:#?}");
    assert!(onermeler(&depo, "db:Note", "TABLO").iter().any(|o| o["deger"]["tablo"] == "notlar"));
    let n = onermeler(&depo, &api, "NESNE_YETKI");
    assert!(n.iter().any(|o| o["deger"]["ifade"].as_str().unwrap().contains("PreAuthorize")), "{n:#?}");
    assert!(!onermeler(&depo, &api, "SAHIPLIK").is_empty(), "getOwnerId().equals(currentUserId())");
}

#[test]
fn aspnet_bagimlilik_kaydi_kesin_ef_ve_mediatr() {
    let h = haritala("d8_cagri_aspnet");
    let depo = json_oku(&h.join("kanit.json"));
    // AddScoped<INotServisi, NotServisi>(): kodda tek kayıt, hedef statik ve tek
    let e = kenar(&depo, "NotlarController.cs#sil", "Veri/NotServisi.cs#NotServisi.SilAsync");
    assert_eq!(e["hesap"]["guven"], "certain", "{e:#}");
    assert_eq!(e["deger"]["cozum"], "di_kaydi", "{e:#}");
    let (_, sil) = kapi(&h, "DELETE", "/Notlar/");
    assert!(onermeler(&depo, &sil, "WRITES").iter().any(|o| o["hedef"] == "db:Not"), "{:#?}", onermeler(&depo, &sil, "WRITES"));
    assert!(onermeler(&depo, &sil, "YETKI_DENETIMI").iter().any(|o| o["deger"]["ifade"].as_str().unwrap().contains("Policy")));
    // MediatR: isteğin işleyicisi, inferred
    let e = kenar(&depo, "NotlarController.cs#arsivle", "ArsivleIstegi.cs#Isleyici.Handle");
    assert_eq!(e["hesap"]["guven"], "inferred", "{e:#}");
    let (_, arsiv) = kapi(&h, "POST", "/arsivle");
    assert!(onermeler(&depo, &arsiv, "WRITES").iter().any(|o| o["hedef"] == "db:Not"));
}

// ---- rapor ve Ayna ----

struct Ayna {
    p: std::process::Child,
    port: u16,
    anahtar: String,
}

impl Drop for Ayna {
    fn drop(&mut self) {
        let _ = self.p.kill();
        let _ = self.p.wait();
    }
}

#[allow(clippy::zombie_processes)]
fn ayna(p: &Proje) -> Ayna {
    let mut c = p.komut(&["ayna", "--port", "0"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let mut satir = String::new();
    loop {
        satir.clear();
        assert!(r.read_line(&mut satir).unwrap() > 0, "Ayna adres yazmadı");
        if let Some(k) = satir.split("http://127.0.0.1:").nth(1) {
            let port = k.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let anahtar = k.split("#anahtar=").nth(1).unwrap().chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            return Ayna { p: c, port, anahtar };
        }
    }
}

fn sorular(a: &Ayna) -> Value {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", a.port)).unwrap();
    write!(s, "GET /api/sorular HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Sezgi-Anahtar: {}\r\n\r\n", a.port, a.anahtar).unwrap();
    let mut c = String::new();
    let _ = s.read_to_string(&mut c);
    serde_json::from_str(c.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[test]
fn rapor_ve_ayna_sorulari_cagri_tablolar_yetki() {
    let ev = Ev::yeni("d8-cagri-ayna");
    let p = ev.proje("d8_cagri_nest", "a");
    let r: Value = serde_json::from_str(&p.ok(&["rapor", "--json"])).unwrap();
    let c = &r["cagri"];
    assert!(c["kenar"].as_u64().unwrap() >= 6 && c["nesne_duzeyi_denetimli_islem"].as_u64().unwrap() >= 2 && c["zincirle_tablo_bulunan_islem"].as_u64().unwrap() >= 4, "{c:#}");
    let metin = p.ok(&["rapor"]);
    assert!(metin.contains("ÇAĞRI") && metin.contains("çağrı kenarı"), "{metin}");
    let a = ayna(&p);
    let b = sorular(&a);
    let soru = |id: &str| b["sorular"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap().clone();
    let metinler = |s: &Value| s["maddeler"].as_array().unwrap().iter().map(|m| format!("{} {}", m["metin"], m["ornekler"])).collect::<Vec<_>>().join(" | ");
    let s8 = soru("cagri");
    assert_ne!(s8["durum"], "bilinmiyor", "{s8:#}");
    assert!(metinler(&s8).contains("çağrı kenarı") && metinler(&s8).contains("deleteAlbum → AlbumService.delete"), "{s8:#}");
    let s9 = soru("tablolar");
    assert!(metinler(&s9).contains("çağrı zinciri"), "{s9:#}");
    let s11 = soru("yetki");
    assert!(metinler(&s11).contains("nesne düzeyi erişim denetimi") && metinler(&s11).contains("album.delete"), "{s11:#}");
}
