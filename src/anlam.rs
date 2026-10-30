//! Anlam katmanı: uygulamanın ne olduğu (iş alanı ve türü), neden var olduğu (belgedeki beyanın alıntısı), hangi iş
//! problemini çözdüğü, kullanıcıları (kullanıcı tipleri, kayıt ve giriş akışları, yönetici ve herkese açık işlemler) ve
//! iş düzeyindeki yetenekleri (işlemlerin "albüm paylaş", "sipariş oluştur" gibi gruplanması).
//!
//! Kaynaklar: README ve paket tanımı (açıklama, ad, anahtar kelimeler), OpenAPI başlığı ve açıklaması, tablo ve model
//! adları, yol ve modül adları, rol adları. Yerel ve deterministiktir: ağ ya da dil modeli yok; alan sınıflandırması
//! sözlük tabanlıdır ve kanıt sayımına dayanır.
//!
//! Güven: anlam çıkarımı en fazla inferred'dir; hiçbir zaman certain denmez. Belgeden alınan cümle uygulamanın kendi
//! beyanıdır: alıntı olarak (dosya:satır) gösterilir ve doğrulanmış sayılmaz. Kanıt yetmiyorsa ya da iki alan yarışıyorsa
//! alan `unknown` kalır: yanlış bir sınıflandırma, damgası ne olursa olsun, "bilinmiyor"dan kötüdür.
//!
//! Aynı sözlük aramayı da besler: Türkçe ya da İngilizce istek ("kullanıcı sil", "delete user") kodun dilindeki
//! kelimelere ve HTTP yöntemine çevrilir. Doğal dil görevi adımlarına ayrılır; her adım için işlem, yetki ve onay gereği
//! bulunur. Plan yürütülmez.

use crate::kanit::{Dayanak, Depo, Gecerlilik, Guven, Iddia, Kanit, Kapsam, KaynakTuru, Kurucu, Onerme};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

// ---- Sözlük ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tur {
    Isim,
    Eylem,
}

/// Bir kavram: kodda geçen İngilizce biçimleri (tekil), Türkçe biçimleri (aksansız, küçük harf), okunur adları ve
/// eylemse işlemin olası HTTP yöntemleri
struct Kavram {
    id: &'static str,
    tur: Tur,
    en: &'static [&'static str],
    tr: &'static [&'static str],
    ad_tr: &'static str,
    ad_en: &'static str,
    yontem: &'static [&'static str],
}

const fn isim(id: &'static str, en: &'static [&'static str], tr: &'static [&'static str], ad_tr: &'static str, ad_en: &'static str) -> Kavram {
    Kavram { id, tur: Tur::Isim, en, tr, ad_tr, ad_en, yontem: &[] }
}

const fn eylem(id: &'static str, en: &'static [&'static str], tr: &'static [&'static str], ad_tr: &'static str, ad_en: &'static str, yontem: &'static [&'static str]) -> Kavram {
    Kavram { id, tur: Tur::Eylem, en, tr, ad_tr, ad_en, yontem }
}

const OKU: &[&str] = &["GET"];
const YAZ: &[&str] = &["POST", "PUT", "PATCH"];

/// Kavram sözlüğü. Sıra önemlidir: bir kelime birden fazla kavramda geçiyorsa okunur ad için ilki kullanılır
static KAVRAMLAR: &[Kavram] = &[
    // Eylemler
    eylem("create", &["create", "add", "new", "insert", "make", "store", "save"], &["olustur", "ekle", "yarat", "yeni", "kaydet"], "oluştur", "create", &["POST", "PUT"]),
    eylem("upload", &["upload", "import"], &["yukle"], "yükle", "upload", &["POST", "PUT"]),
    eylem("download", &["download", "export"], &["indir"], "indir", "download", OKU),
    eylem("list", &["list", "all", "index", "browse", "getall", "findall"], &["listele", "hepsi", "tum"], "listele", "list", OKU),
    eylem(
        "view",
        &["get", "show", "view", "read", "detail", "info", "fetch", "find", "retrieve", "one"],
        &["goster", "gor", "goruntule", "oku", "getir", "detay", "kontrol", "incele"],
        "görüntüle",
        "view",
        OKU,
    ),
    eylem("search", &["search", "query", "lookup", "filter"], &["ara", "bul", "sorgula"], "ara", "search", &["GET", "POST"]),
    eylem(
        "update",
        &["update", "edit", "change", "modify", "set", "rename", "patch", "put", "replace"],
        &["guncelle", "duzenle", "degistir"],
        "güncelle",
        "update",
        &["PUT", "PATCH", "POST"],
    ),
    eylem("delete", &["delete", "remove", "destroy", "erase", "purge", "drop", "del"], &["sil", "kaldir"], "sil", "delete", &["DELETE"]),
    eylem("share", &["share", "shared", "invite", "collaborate"], &["paylas", "davet"], "paylaş", "share", YAZ),
    eylem("transfer", &["reassign", "handover", "assign", "move"], &["devret", "aktar", "tasi", "ata"], "devret", "transfer", YAZ),
    eylem("login", &["login", "signin", "authenticate", "logon"], &["giris"], "giriş yap", "log in", &["POST"]),
    eylem("logout", &["logout", "signout", "logoff"], &["cikis"], "çıkış yap", "log out", &["POST", "DELETE", "GET"]),
    eylem("register", &["register", "signup", "registration"], &["kayit", "kaydol", "uyeol"], "kayıt ol", "sign up", &["POST"]),
    eylem("reset", &["reset", "forgot", "recover", "recovery"], &["sifirla", "unut"], "sıfırla", "reset", &["POST", "PUT"]),
    eylem("restore", &["restore", "undelete", "untrash", "undo"], &["kurtar", "geriyukle", "geri"], "geri yükle", "restore", YAZ),
    eylem("archive", &["archive"], &["arsivle"], "arşivle", "archive", YAZ),
    eylem("approve", &["approve", "accept", "confirm", "verify"], &["onayla", "dogrula"], "onayla", "approve", YAZ),
    eylem("reject", &["reject", "deny", "decline"], &["reddet"], "reddet", "reject", YAZ),
    eylem("pay", &["pay", "purchase", "buy"], &["ode", "satinal"], "öde", "pay", YAZ),
    eylem("cancel", &["cancel", "revoke", "abort"], &["iptal"], "iptal et", "cancel", &["POST", "PUT", "PATCH", "DELETE"]),
    eylem("refund", &["refund", "chargeback", "reimburse"], &["iade", "geriode"], "iade et", "refund", YAZ),
    eylem("play", &["play", "stream", "listen", "scrobble"], &["cal", "oynat", "dinle"], "çal", "play", &["GET", "POST"]),
    eylem("send", &["send", "notify", "mail"], &["gonder", "yolla"], "gönder", "send", &["POST"]),
    eylem("empty", &["empty"], &["bosalt"], "boşalt", "empty", &["POST", "DELETE"]),
    eylem("copy", &["copy", "duplicate", "clone"], &["kopyala", "cogalt"], "kopyala", "copy", &["POST", "PUT"]),
    eylem("pause", &["pause", "suspend", "mute", "snooze"], &["duraklat", "askiya", "sustur", "beklet"], "duraklat", "pause", YAZ),
    eylem("resume", &["resume", "unpause", "unmute", "reactivate"], &["devam", "surdur"], "devam ettir", "resume", YAZ),
    eylem("deactivate", &["deactivate", "disable", "close"], &["kapat", "pasiflestir"], "kapat", "close", &["POST", "PUT", "PATCH", "DELETE"]),
    eylem("follow", &["follow", "subscribe"], &["takip"], "takip et", "follow", &["POST", "PUT"]),
    eylem("favorite", &["favorite", "favourite", "like", "star", "bookmark"], &["begen"], "favorilere ekle", "favorite", &["POST", "PUT"]),
    // Varlıklar
    isim("user", &["user", "account", "member", "usr"], &["kullanici", "hesap", "uye"], "kullanıcı", "user"),
    isim("person", &["person", "people"], &["kisi"], "kişi", "person"),
    isim("face", &["face"], &["yuz"], "yüz", "face"),
    isim("album", &["album"], &["album"], "albüm", "album"),
    isim("asset", &["asset", "media"], &["medya", "varlik"], "medya", "asset"),
    isim("photo", &["photo", "image", "picture", "asset", "thumbnail", "img"], &["fotograf", "resim", "gorsel", "foto"], "fotoğraf", "photo"),
    isim("video", &["video", "asset", "movie", "clip"], &["video", "film"], "video", "video"),
    isim("file", &["file", "attachment"], &["dosya", "ek"], "dosya", "file"),
    isim("document", &["document", "doc"], &["belge", "dokuman"], "belge", "document"),
    isim("order", &["order", "purchase"], &["siparis"], "sipariş", "order"),
    isim("product", &["product", "goods", "sku"], &["urun"], "ürün", "product"),
    isim("item", &["item"], &["oge", "kalem", "madde"], "öğe", "item"),
    isim("cart", &["cart", "basket"], &["sepet"], "sepet", "cart"),
    isim("payment", &["payment", "paypal", "stripe", "checkout"], &["odeme"], "ödeme", "payment"),
    isim("invoice", &["invoice", "bill"], &["fatura"], "fatura", "invoice"),
    isim("receipt", &["receipt"], &["makbuz", "fis", "dekont"], "makbuz", "receipt"),
    isim("transaction", &["transaction", "entry"], &["islem", "hareket"], "işlem", "transaction"),
    isim("budget", &["budget"], &["butce"], "bütçe", "budget"),
    isim("customer", &["customer", "client"], &["musteri"], "müşteri", "customer"),
    isim("article", &["article", "story"], &["makale", "yazi", "gonderi", "haber"], "makale", "article"),
    isim("comment", &["comment", "reply"], &["yorum", "cevap"], "yorum", "comment"),
    isim("tag", &["tag", "label"], &["etiket"], "etiket", "tag"),
    isim("favorite", &["favorite", "favourite", "like", "star"], &["favori", "begeni"], "favori", "favorite"),
    isim("profile", &["profile"], &["profil"], "profil", "profile"),
    // Yazar bir kullanıcının profilidir: "yazarı takip et" profil işlemlerinde aranır
    isim("author", &["author", "writer", "creator", "profile"], &["yazar"], "yazar", "author"),
    isim("follow", &["follow", "follower", "following", "subscription"], &["takip", "abone"], "takip", "follow"),
    isim("book", &["book"], &["kitap"], "kitap", "book"),
    isim("page", &["page"], &["sayfa"], "sayfa", "page"),
    isim("chapter", &["chapter"], &["bolum"], "bölüm", "chapter"),
    isim("shelf", &["shelf", "shelve", "bookshelf", "bookshelve"], &["raf"], "raf", "shelf"),
    isim("song", &["song", "track", "music", "audio"], &["sarki", "parca", "muzik"], "şarkı", "song"),
    isim("playlist", &["playlist"], &["calma", "playlist"], "çalma listesi", "playlist"),
    isim("artist", &["artist"], &["sanatci"], "sanatçı", "artist"),
    isim("podcast", &["podcast", "episode"], &["podcast", "bolum"], "podcast", "podcast"),
    isim("reservation", &["reservation", "booking"], &["rezervasyon"], "rezervasyon", "reservation"),
    isim("guest", &["guest"], &["misafir"], "misafir", "guest"),
    isim("room", &["room"], &["oda"], "oda", "room"),
    isim("hotel", &["hotel"], &["otel"], "otel", "hotel"),
    isim("transfer", &["transfer"], &["transfer"], "transfer", "transfer"),
    isim("flight", &["flight"], &["ucus"], "uçuş", "flight"),
    isim("message", &["message", "msg"], &["mesaj"], "mesaj", "message"),
    isim("chat", &["chat", "conversation"], &["sohbet"], "sohbet", "chat"),
    isim("notification", &["notification"], &["bildirim"], "bildirim", "notification"),
    isim("role", &["role"], &["rol"], "rol", "role"),
    isim("permission", &["permission", "grant"], &["izin", "yetki"], "izin", "permission"),
    isim("password", &["password", "passwd", "pwd"], &["parola", "sifre"], "parola", "password"),
    isim("session", &["session", "token", "auth"], &["oturum"], "oturum", "session"),
    isim("setting", &["setting", "config", "configuration", "preference", "option"], &["ayar", "tercih"], "ayar", "setting"),
    isim("team", &["team", "group"], &["takim", "ekip", "grup"], "ekip", "team"),
    isim("organization", &["organization", "organisation", "org", "company", "tenant"], &["kurulus", "sirket", "firma"], "kuruluş", "organization"),
    isim("project", &["project"], &["proje"], "proje", "project"),
    isim("task", &["task", "todo", "issue", "ticket"], &["gorev"], "görev", "task"),
    isim("owner", &["owner"], &["sahip"], "sahip", "owner"),
    isim("category", &["category"], &["kategori"], "kategori", "category"),
    isim("review", &["review", "rating"], &["degerlendirme", "puan"], "değerlendirme", "review"),
    isim("address", &["address"], &["adres"], "adres", "address"),
    isim("shipping", &["shipping", "shipment", "delivery"], &["kargo", "teslimat"], "teslimat", "shipping"),
    isim("stock", &["stock", "inventory"], &["stok", "envanter"], "stok", "stock"),
    isim("event", &["event"], &["etkinlik"], "etkinlik", "event"),
    isim("library", &["library"], &["kutuphane"], "kütüphane", "library"),
    isim("trash", &["trash", "recycle"], &["cop"], "çöp kutusu", "trash"),
    isim("backup", &["backup"], &["yedek"], "yedek", "backup"),
    isim("key", &["key", "apikey"], &["anahtar"], "anahtar", "key"),
    isim("link", &["link", "url"], &["baglanti", "link"], "bağlantı", "link"),
    isim("memory", &["memory"], &["ani"], "anı", "memory"),
    isim("location", &["location", "map", "place"], &["konum", "harita"], "konum", "location"),
    isim("pet", &["pet", "animal"], &["hayvan", "evcil"], "evcil hayvan", "pet"),
    isim("vet", &["vet", "veterinarian"], &["veteriner"], "veteriner", "vet"),
    isim("visit", &["visit", "appointment"], &["ziyaret", "randevu"], "ziyaret", "visit"),
    isim("employee", &["employee", "staff"], &["calisan", "personel"], "çalışan", "employee"),
    isim("course", &["course", "lesson"], &["kurs", "ders"], "kurs", "course"),
    isim("student", &["student"], &["ogrenci"], "öğrenci", "student"),
    isim("email", &["email"], &["eposta"], "e-posta", "email"),
    isim("server", &["server"], &["sunucu"], "sunucu", "server"),
    isim("job", &["job", "queue"], &["kuyruk"], "iş kuyruğu", "job"),
    isim("statistic", &["statistic", "stat", "count"], &["istatistik"], "istatistik", "statistic"),
    isim("home", &["home", "root", "dashboard"], &["anasayfa"], "ana sayfa", "home"),
];

/// Aramada ve görev ayrıştırmada anlam taşımayan kelimeler (aksansız)
const DOLGU: &[&str] = &[
    "ve",
    "ile",
    "bir",
    "bu",
    "su",
    "icin",
    "de",
    "da",
    "sonra",
    "once",
    "ardindan",
    "butun",
    "her",
    "olan",
    "gibi",
    "the",
    "a",
    "an",
    "and",
    "of",
    "to",
    "for",
    "with",
    "then",
    "after",
    "before",
    "it",
    "its",
    "their",
    "his",
    "her",
    "them",
    "from",
    "into",
    "in",
    "on",
    "by",
    "baskasina",
    "baska",
    "diger",
    "digerine",
    "else",
    "someone",
    "another",
    "other",
    "yeniden",
    "lutfen",
    "please",
    "et",
    "ettir",
    "edin",
    "yap",
    "ol",
    "daha",
    "that",
    "this",
];

/// Adaptörün guard adına eklediği etiket: projedeki dekoratör ya da bağımlılık istekten API anahtarı / belirteç okuyup
/// 401 / 403 dönüyor (kalıptan çıkarım). Bu işlemlerin kullanıcısı "API anahtarı / belirteci ile istemci"dir
pub const ANAHTAR_GUARD_ETIKETI: &str = " [api key/token]";

/// Yolda kaynak adı sayılmayan parçalar
/// Yolda kaynağı nitelendiren ama kendisi kaynak olmayan parçalar (`/config/defaults`, `/orders/mine`): yetenek adı üst
/// kaynaktan kurulur
const NITELEYICI: &[&str] = &["default", "defaults", "mine", "my", "all", "latest", "new", "info", "detail", "details"];
const YOL_DOLGU: &[&str] = &["api", "rest", "admin", "me", "self", "current", "internal", "public", "private", "by", "id", "bulk", "v", "app", "yonetim", "kamu", "ben"];

/// Türkçe yazımı aksansız, küçük harfe indirir ("Kullanıcı" -> "kullanici"); kalıp eşleşmeleri bunun üzerindedir
pub fn sade(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ç' | 'Ç' => o.push('c'),
            'ğ' | 'Ğ' => o.push('g'),
            'ı' | 'I' | 'İ' => o.push('i'),
            'ö' | 'Ö' => o.push('o'),
            'ş' | 'Ş' => o.push('s'),
            'ü' | 'Ü' => o.push('u'),
            'â' | 'Â' => o.push('a'),
            'î' | 'Î' => o.push('i'),
            'û' | 'Û' => o.push('u'),
            c => o.extend(c.to_lowercase()),
        }
    }
    o
}

/// Tanımlayıcıyı kelimelere böler (camelCase, ayraçlar) ve sadeleştirir: `addAssetsToAlbum` -> add, assets, to, album
pub fn kelimeler(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut k = String::new();
    let mut onceki_kucuk = false;
    for ch in s.chars() {
        if !ch.is_alphanumeric() {
            if !k.is_empty() {
                out.push(sade(&std::mem::take(&mut k)));
            }
            onceki_kucuk = false;
            continue;
        }
        if ch.is_uppercase() && onceki_kucuk && !k.is_empty() {
            out.push(sade(&std::mem::take(&mut k)));
        }
        onceki_kucuk = ch.is_lowercase() || ch.is_ascii_digit();
        k.push(ch);
    }
    if !k.is_empty() {
        out.push(sade(&k));
    }
    out
}

/// Düz yazının kelimeleri (camelCase bölünmez: "eCommerce" -> "ecommerce"); birleşik giriş/kayıt yazımları tek kelime
fn yazi_kelimeleri(s: &str) -> Vec<String> {
    let s = birlesik(&sade(s));
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}

/// "sign in", "sign-up", "log out", "geri yükle", "kayıt ol", "üye ol", "satın al", "e-commerce" tek kelime yazılır
fn birlesik(s: &str) -> String {
    let mut s = s.to_string();
    for (a, b) in [
        ("sign in", "signin"),
        ("sign-in", "signin"),
        ("sign_in", "signin"),
        ("sign up", "signup"),
        ("sign-up", "signup"),
        ("sign_up", "signup"),
        ("sign out", "signout"),
        ("sign-out", "signout"),
        ("log in", "login"),
        ("log-in", "login"),
        ("log out", "logout"),
        ("log-out", "logout"),
        ("geri yukle", "geriyukle"),
        ("kayit ol", "kaydol"),
        ("uye ol", "uyeol"),
        ("satin al", "satinal"),
        ("e-commerce", "ecommerce"),
        ("e-ticaret", "eticaret"),
        ("e-posta", "eposta"),
        ("e-mail", "email"),
        ("net worth", "networth"),
        ("net-worth", "networth"),
        ("self-hosted", "selfhosted"),
        ("self hosted", "selfhosted"),
        ("e-learning", "elearning"),
        ("health check", "healthcheck"),
        ("health-check", "healthcheck"),
    ] {
        s = s.replace(a, b);
    }
    s
}

/// İngilizce tekil biçim (kaba): albums -> album, libraries -> library, aliases -> alias, people -> person
pub fn tekil(w: &str) -> String {
    match w {
        "people" => return "person".into(),
        "children" => return "child".into(),
        "movies" => return "movie".into(),
        "cookies" => return "cookie".into(),
        "settings" => return "setting".into(),
        "series" | "news" | "status" | "media" | "data" | "analysis" => return w.into(),
        _ => {}
    }
    let n = w.len();
    if n > 4 && w.ends_with("ies") {
        return format!("{}y", &w[..n - 3]);
    }
    if n > 4 && ["sses", "xes", "zes", "ches", "shes", "uses"].iter().any(|e| w.ends_with(e)) {
        return w[..n - 2].to_string();
    }
    if n > 3 && w.ends_with('s') && !["ss", "us", "is"].iter().any(|e| w.ends_with(e)) {
        // Dört harfli kelimede yalnızca sözlükteki tekil biçime indirilir (tags -> tag); "oups" -> "oup" gibi
        // anlamsız kesik ad üretilmez
        let t = &w[..n - 1];
        if n == 4 && !KAVRAMLAR.iter().any(|k| k.en.contains(&t)) {
            return w.to_string();
        }
        return t.to_string();
    }
    w.to_string()
}

/// Türkçe ek mi: çekim ekleri (çoğul, iyelik, hâl, fiil) bu harflerle başlar; ek çok uzun değildir
fn ek_mi(s: &str) -> bool {
    !s.is_empty() && s.len() <= 8 && s.chars().all(|c| c.is_ascii_lowercase()) && s.starts_with(['a', 'e', 'i', 'o', 'u', 'l', 'm', 'n', 'd', 't', 'y', 's', 'c', 'g'])
}

/// Kelimenin kavramları ve Türkçe ekli olup olmadığı: önce tam eşleşme (İngilizce tekil ya da Türkçe), sonra Türkçe
/// ekli biçimde en uzun kök (kullanıcıyı -> kullanici, albümlerini -> album)
fn kavramlar(w: &str) -> (Vec<&'static Kavram>, Option<String>) {
    let w = sade(w);
    let t = tekil(&w);
    let tam: Vec<&Kavram> = KAVRAMLAR.iter().filter(|k| k.en.contains(&w.as_str()) || k.en.contains(&t.as_str()) || k.tr.contains(&w.as_str())).collect();
    if !tam.is_empty() {
        return (tam, None);
    }
    // Ünlüyle başlayan ekte kökün son sert ünsüzü yumuşar: hesap -> hesabı, kitap -> kitabı, renk -> rengi
    let yumusak = |f: &str| -> String {
        match f.chars().last() {
            Some('p') => format!("{}b", &f[..f.len() - 1]),
            Some('t') => format!("{}d", &f[..f.len() - 1]),
            Some('k') => format!("{}g", &f[..f.len() - 1]),
            _ => f.to_string(),
        }
    };
    let mut en_uzun = 0usize;
    let mut bulunan: Option<&str> = None;
    for k in KAVRAMLAR {
        for f in k.tr {
            for kok in [f.to_string(), yumusak(f)] {
                if f.len() >= 3 && kok.len() > en_uzun && w.len() > kok.len() && w.starts_with(kok.as_str()) && ek_mi(&w[kok.len()..]) {
                    en_uzun = kok.len();
                    bulunan = Some(f);
                }
            }
        }
    }
    let Some(kok) = bulunan else {
        return (vec![], None);
    };
    (KAVRAMLAR.iter().filter(|k| k.tr.contains(&kok)).collect(), Some(w[en_uzun..].to_string()))
}

fn kavram(id: &str) -> Option<&'static Kavram> {
    KAVRAMLAR.iter().find(|k| k.id == id)
}

/// Kod kelimesinin (tekil) bilinen ilk isim kavramı: okunur ad için
fn isim_kavrami(w: &str) -> Option<&'static Kavram> {
    let t = tekil(w);
    KAVRAMLAR.iter().find(|k| k.tur == Tur::Isim && (k.en.contains(&t.as_str()) || k.tr.contains(&w)))
}

/// Kod kelimesi bir eylem mi (yalnızca tam eşleşme: yoldaki ve fonksiyon adındaki kelime)
fn eylem_kavrami(w: &str) -> Option<&'static Kavram> {
    let w = sade(w);
    KAVRAMLAR.iter().find(|k| k.tur == Tur::Eylem && (k.en.contains(&w.as_str()) || k.tr.contains(&w.as_str())))
}

// ---- Arama ----

/// Çözülmüş arama isteği: her anlamlı kelime için (asıl kelime, eş anlamlıları), eylemden çıkan HTTP yöntemleri
pub struct Sorgu {
    /// (asıl kelime, eş anlamlılar, eylem mi)
    gruplar: Vec<(String, Vec<String>, bool)>,
    yontemler: Vec<&'static str>,
    /// İstekteki eylem kavramları (ör. delete)
    eylemler: Vec<&'static str>,
}

impl Sorgu {
    pub fn bos(&self) -> bool {
        self.gruplar.is_empty()
    }
    /// İstekteki eylemin HTTP yöntemi kapıya uyuyor mu (istekte eylem yoksa uyar)
    pub fn yontem_uyar(&self, k: &Value) -> bool {
        self.yontemler.is_empty() || self.yontemler.contains(&k["yontem"].as_str().unwrap_or("").to_ascii_uppercase().as_str())
    }
}

/// Türkçe ya da İngilizce isteği kodun dilindeki kelimelere çevirir: "kullanıcı sil" -> {kullanici, user, account …}
/// {sil, delete, remove …} ve yöntem DELETE. Ters yön de aynı sözlükten: "delete book" Türkçe adlı kodda "kitap sil"i bulur
pub fn sorgu(metin: &str) -> Sorgu {
    let tum = yazi_kelimeleri(metin);
    let anlamli: Vec<String> = tum.iter().filter(|w| !DOLGU.contains(&w.as_str())).cloned().collect();
    let kelimeler = if anlamli.is_empty() { tum } else { anlamli };
    let mut gruplar = vec![];
    let mut yontemler: Vec<&'static str> = vec![];
    let mut eylemler: Vec<&'static str> = vec![];
    for w in kelimeler {
        let (ks, _) = kavramlar(&w);
        let mut es: Vec<String> = vec![];
        let t = tekil(&w);
        if t != w {
            es.push(t);
        }
        for k in &ks {
            for f in k.en.iter().chain(k.tr.iter()) {
                if *f != w && !es.iter().any(|e| e == f) {
                    es.push(f.to_string());
                }
            }
            if k.tur == Tur::Eylem {
                eylemler.push(k.id);
                for y in k.yontem {
                    if !yontemler.contains(y) {
                        yontemler.push(y);
                    }
                }
            }
        }
        let eylem_mi = !ks.is_empty() && ks.iter().all(|k| k.tur == Tur::Eylem);
        gruplar.push((w, es, eylem_mi));
    }
    Sorgu { gruplar, yontemler, eylemler }
}

/// Ayna araması: isteğe uyan kapıların kimlikleri, puan sırasıyla (Türkçe ya da İngilizce istek)
pub fn ara_kimlikleri(kapilar: &[Value], metin: &str, adet: usize) -> Vec<String> {
    let q = sorgu(metin);
    if q.bos() {
        return vec![];
    }
    let mut v: Vec<(f64, &Value)> = kapilar.iter().map(|k| (puan(k, &q, &crate::mcp::alan_adlari(k)), k)).filter(|(p, _)| *p > 0.0).collect();
    v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    v.into_iter().take(adet).filter_map(|(_, k)| k["id"].as_str().map(String::from)).collect()
}

fn eslesir(liste: &[String], w: &str) -> bool {
    liste.iter().any(|x| x == w || (w.len() > 3 && x.len() > 3 && (x.starts_with(w) || w.starts_with(x.as_str()))))
}

/// Eş anlamlı eşleşmesi daha sıkıdır: birebir, İngilizce çoğul ya da (Türkçe biçimse) ekli hâl ("ordered" order değildir)
fn es_eslesir(liste: &[String], e: &str) -> bool {
    let tr = KAVRAMLAR.iter().any(|k| k.tr.contains(&e));
    liste.iter().any(|x| x == e || tekil(x) == e || (tr && e.len() >= 3 && x.len() > e.len() && x.starts_with(e) && ek_mi(&x[e.len()..])))
}

/// Kapının arama puanı: yol ve fonksiyon adı 3, kod özeti 2, modül 1,5, girdi alanı 1; eş anlamlıyla eşleşme bunun
/// %80'i. İstek bir eylem taşıyorsa uyan HTTP yöntemi öne çıkar. 0: eşleşme yok
pub fn puan(k: &Value, s: &Sorgu, alanlar: &[String]) -> f64 {
    let yol = kelimeler(k["yol"].as_str().unwrap_or(""));
    let fonk = kelimeler(k["fonksiyon"].as_str().unwrap_or(""));
    let ctrl = kelimeler(k["kontrolcu"].as_str().unwrap_or(""));
    let ozet = kelimeler(k["ozet"].as_str().unwrap_or(""));
    let alan: Vec<String> = alanlar.iter().flat_map(|a| kelimeler(a)).collect();
    let mut p: f64 = 0.0;
    // İstek bir varlık adı taşıyorsa (ör. "sipariş oluştur") yalnızca fiille eşleşen kapı sonuç değildir
    let isim_var = s.gruplar.iter().any(|g| !g.2);
    let mut isim_eslesti = false;
    for (asil, es, eylem_mi) in &s.gruplar {
        for (liste, agirlik) in [(&yol, 3.0), (&fonk, 3.0), (&ozet, 2.0), (&ctrl, 1.5), (&alan, 1.0)] {
            let artis = if eslesir(liste, asil) {
                agirlik
            } else if es.iter().any(|e| es_eslesir(liste, e)) {
                agirlik * 0.8
            } else {
                0.0
            };
            p += artis;
            if artis > 0.0 && !eylem_mi && agirlik >= 1.5 {
                isim_eslesti = true;
            }
        }
    }
    if isim_var && !isim_eslesti {
        return 0.0;
    }
    // İşlemin iş yeteneği isteğin varlığı ve eylemiyle aynıysa öne çıkar (DELETE /users/:id, taslak silmeden önce)
    if p > 0.0 {
        let y = yetenek(k);
        let kaynaklar: Vec<&str> = y.kaynak.split('-').collect();
        let son = kaynaklar.last().copied().unwrap_or("");
        if s.gruplar.iter().filter(|g| !g.2).any(|(a, es, _)| a == son || tekil(a) == son || es.iter().any(|e| e == son)) {
            p += 2.0;
        }
        if s.eylemler.contains(&y.eylem.as_str()) {
            p += 1.0;
        }
    }
    if p > 0.0 && !s.yontemler.is_empty() {
        let y = k["yontem"].as_str().unwrap_or("").to_ascii_uppercase();
        if s.yontemler.contains(&y.as_str()) {
            p += 1.5;
        } else {
            p = (p - 0.5).max(0.1);
        }
    }
    p
}

// ---- İş yetenekleri ----

/// İşlemin iş yeteneği: kaynak (yolun isim parçası, tekil) + eylem (yoldaki ya da fonksiyon adındaki fiil, yoksa HTTP
/// yöntemi). İç içe kaynakta üst kaynak da adda yer alır (`album.add-asset`); bir kaynağa kullanıcı eklemek paylaşmaktır
pub struct Yetenek {
    pub anahtar: String,
    pub ad: String,
    pub ad_en: String,
    pub kaynak: String,
    pub eylem: String,
}

fn kaynak_adi(kaynak: &str, en: bool) -> String {
    let parcalar: Vec<&str> = kaynak.split('-').collect();
    if let Some(k) = isim_kavrami(kaynak.replace('-', "").as_str()).or_else(|| parcalar.last().and_then(|p| isim_kavrami(p)).filter(|_| parcalar.len() == 1)) {
        return if en { k.ad_en.to_string() } else { k.ad_tr.to_string() };
    }
    kaynak.replace('-', " ")
}

pub fn yetenek(k: &Value) -> Yetenek {
    let yontem = k["yontem"].as_str().unwrap_or("GET").to_ascii_uppercase();
    let yol = k["yol"].as_str().unwrap_or("");
    // (isim mi, parça kelimeleri, önünde parametre var mı)
    let mut parcalar: Vec<(bool, String, bool)> = vec![];
    let mut param = false;
    let mut son_param = false;
    for p in yol.split('/').filter(|p| !p.is_empty()) {
        if p.starts_with([':', '{', '*', '<', '[']) {
            param = true;
            son_param = true;
            continue;
        }
        // Dosya adı (`playlist.m3u8`, `main.m3u8`) bir kaynak değil, değerdir: parametre gibi atlanır
        if p.rsplit_once('.').is_some_and(|(o, u)| !o.is_empty() && (2..=5).contains(&u.len()) && u.chars().all(|c| c.is_ascii_alphanumeric())) {
            param = true;
            son_param = true;
            continue;
        }
        son_param = false;
        let ws: Vec<String> = kelimeler(&birlesik(&sade(p))).into_iter().filter(|w| !w.chars().all(|c| c.is_ascii_digit())).collect();
        if ws.is_empty()
            || (ws.len() == 1
                && (YOL_DOLGU.contains(&ws[0].as_str()) || NITELEYICI.contains(&ws[0].as_str()) || (ws[0].starts_with('v') && ws[0][1..].chars().all(|c| c.is_ascii_digit()))))
        {
            continue;
        }
        let fiil = ws.iter().all(|w| eylem_kavrami(w).is_some());
        if fiil {
            parcalar.push((false, ws.join("-"), param));
        } else if ws.len() > 1 && eylem_kavrami(&ws[0]).is_some() && isim_kavrami(ws.last().map(String::as_str).unwrap_or("")).is_some() {
            // "create-book": önce fiil, sonra isim
            parcalar.push((true, ws[1..].iter().map(|w| tekil(w)).collect::<Vec<_>>().join("-"), param));
            parcalar.push((false, ws[0].clone(), false));
        } else {
            parcalar.push((true, ws.iter().map(|w| tekil(w)).collect::<Vec<_>>().join("-"), param));
        }
        param = false;
    }
    let isimler: Vec<&(bool, String, bool)> = parcalar.iter().filter(|p| p.0).collect();
    let yol_fiili = parcalar.iter().rev().find(|p| !p.0).and_then(|p| p.1.split('-').find_map(eylem_kavrami));
    let fonk = kelimeler(k["fonksiyon"].as_str().unwrap_or(""));
    let fonk_fiili = fonk.first().and_then(|w| eylem_kavrami(w)).map(|e| {
        // get + all / list / many: listeleme; laravel ve rails adları (index, show, store, destroy)
        if e.id == "view" && fonk.iter().skip(1).any(|w| w == "all" || w == "many" || w == "list") {
            kavram("list").unwrap_or(e)
        } else {
            e
        }
    });
    let ozel = match fonk.first().map(String::as_str) {
        Some("store") => kavram("create"),
        Some("destroy") => kavram("delete"),
        Some("index") => kavram("list"),
        _ => None,
    };
    let varsayilan = match yontem.as_str() {
        "GET" | "HEAD" if son_param => "view",
        "GET" | "HEAD" => "list",
        "DELETE" => "delete",
        "PUT" | "PATCH" => "update",
        _ => "create",
    };
    let akis = match akis_turu(k) {
        Some("kayit") => kavram("register"),
        Some("giris") => kavram("login"),
        Some("cikis") => kavram("logout"),
        Some("parola") => kavram("reset"),
        _ => None,
    };
    let mut e = akis.or(yol_fiili).or(ozel).or(fonk_fiili).map(|x| x.id).unwrap_or(varsayilan);
    // Okuma fiili yazma yöntemiyle (ör. POST /search) arama ya da yöntemin işi sayılır; yazma fiili GET ile okumaya döner
    let e_kavram = kavram(e);
    if let Some(ek) = e_kavram {
        let okuma = ek.yontem == OKU;
        if okuma && yontem != "GET" && e != "search" {
            e = varsayilan;
        }
        if !okuma && yontem == "GET" && !matches!(e, "search" | "download" | "play" | "logout" | "login") {
            e = varsayilan;
        }
    }
    // Giriş, çıkış ve kayıt oturumun; sıfırlama parolanın işidir (yoldaki isim ne olursa olsun)
    let ozel_kaynak = match e {
        "login" | "logout" | "register" => Some("auth"),
        "reset" => Some("password"),
        _ => None,
    };
    let (kaynak, ust) = match (isimler.first(), isimler.last()) {
        _ if ozel_kaynak.is_some() => (ozel_kaynak.unwrap_or_default().to_string(), None),
        (Some(u), Some(s)) if isimler.len() > 1 && s.2 => (s.1.clone(), Some(u.1.clone())),
        (_, Some(s)) => (s.1.clone(), None),
        _ => {
            // Yolda isim yok (ör. /upload): yükleme ve indirme dosyanın işidir, değilse modül adı, o da yoksa ana sayfa
            let modul = kelimeler(k["kontrolcu"].as_str().unwrap_or("").trim_end_matches("Controller"))
                .into_iter()
                .filter(|w| w != "controller" && w != "router" && w != "routes" && w != "api" && eylem_kavrami(w).is_none())
                .map(|w| tekil(&w))
                .collect::<Vec<_>>()
                .join("-");
            if modul.is_empty() && matches!(e, "upload" | "download") {
                ("file".to_string(), None)
            } else if modul.is_empty() {
                ("home".to_string(), None)
            } else {
                (modul, None)
            }
        }
    };
    let ust = ust.filter(|u| *u != kaynak);
    // Bir kaynağa kullanıcı ya da üye eklemek paylaşmaktır; çıkarmak paylaşımı kaldırmaktır
    let kisi = matches!(kaynak.as_str(), "user" | "member" | "collaborator" | "person");
    let (anahtar, ad, ad_en, eylem_id) = match (&ust, e) {
        (Some(u), "create" | "update" | "share") if kisi => {
            (format!("{u}.share"), format!("{} paylaş", kaynak_adi(u, false)), format!("share {}", kaynak_adi(u, true)), "share".to_string())
        }
        (Some(u), "delete") if kisi => {
            (format!("{u}.unshare"), format!("{} paylaşımını kaldır", kaynak_adi(u, false)), format!("unshare {}", kaynak_adi(u, true)), "unshare".to_string())
        }
        (Some(u), _) => {
            let ek = kavram(e).unwrap_or_else(|| kavram("create").unwrap());
            let (tr, en) = if e == "create" { ("ekle", "add") } else { (ek.ad_tr, ek.ad_en) };
            (
                format!("{u}.{}-{kaynak}", if e == "create" { "add" } else { e }),
                format!("{}: {} {tr}", kaynak_adi(u, false), kaynak_adi(&kaynak, false)),
                format!("{}: {en} {}", kaynak_adi(u, true), kaynak_adi(&kaynak, true)),
                e.to_string(),
            )
        }
        (None, _) => {
            let ek = kavram(e).unwrap_or_else(|| kavram("view").unwrap());
            let ad = if kaynak == "auth" { ek.ad_tr.to_string() } else { format!("{} {}", kaynak_adi(&kaynak, false), ek.ad_tr) };
            let ad_en = if kaynak == "auth" { ek.ad_en.to_string() } else { format!("{} {}", ek.ad_en, kaynak_adi(&kaynak, true)) };
            (format!("{kaynak}.{e}"), ad, ad_en, e.to_string())
        }
    };
    Yetenek { anahtar, ad, ad_en, kaynak: ust.unwrap_or(kaynak), eylem: eylem_id }
}

// ---- Alan ve tür sözlüğü ----

/// İş alanı: kimlik, okunur adlar, çözdüğü problem (sözlükteki genel tanım) ve ağırlıklı terimler
struct Alan {
    id: &'static str,
    ad: &'static str,
    ad_en: &'static str,
    problem: &'static str,
    problem_en: &'static str,
    terimler: &'static [(&'static str, u32)],
}

static ALANLAR: &[Alan] = &[
    Alan {
        id: "photo-video",
        ad: "fotoğraf ve video arşivi",
        ad_en: "photo and video library",
        problem: "Fotoğraf ve videoları tek yerde saklamak, düzenlemek ve paylaşmak",
        problem_en: "Keeping, organising and sharing photos and videos in one place",
        terimler: &[
            ("photo", 2),
            ("video", 2),
            ("image", 1),
            ("picture", 1),
            ("gallery", 2),
            ("exif", 2),
            ("face", 1),
            ("thumbnail", 1),
            ("camera", 1),
            ("album", 1),
            ("asset", 1),
            ("fotograf", 2),
            ("galeri", 2),
        ],
    },
    Alan {
        id: "music",
        ad: "müzik dinleme ve yayın",
        ad_en: "music streaming",
        problem: "Bir müzik koleksiyonunu her yerden dinlenebilir hâle getirmek",
        problem_en: "Making a music collection playable from anywhere",
        terimler: &[
            ("music", 2),
            ("song", 2),
            ("track", 1),
            ("artist", 2),
            ("playlist", 2),
            ("album", 1),
            ("podcast", 1),
            ("radio", 1),
            ("audio", 2),
            ("scrobble", 2),
            ("lyric", 2),
            ("genre", 1),
            ("muzik", 2),
            ("sarki", 2),
        ],
    },
    Alan {
        id: "ecommerce",
        ad: "e-ticaret (çevrimiçi satış)",
        ad_en: "e-commerce (online store)",
        problem: "Ürünleri çevrimiçi satmak: katalog, sepet, sipariş ve ödeme",
        problem_en: "Selling products online: catalogue, cart, orders and payment",
        terimler: &[
            ("ecommerce", 3),
            ("commerce", 2),
            ("shop", 2),
            ("shopping", 2),
            ("cart", 2),
            ("checkout", 2),
            ("product", 2),
            ("order", 1),
            ("payment", 1),
            ("paypal", 2),
            ("stripe", 1),
            ("coupon", 2),
            ("shipping", 2),
            ("eticaret", 3),
            ("urun", 2),
            ("sepet", 2),
            ("siparis", 1),
        ],
    },
    Alan {
        id: "blog",
        ad: "blog ve içerik yayını",
        ad_en: "blogging and publishing",
        problem: "Yazı yayımlamak; okurların yorum yapması, yazarları takip etmesi ve yazıları beğenmesi",
        problem_en: "Publishing articles, with readers commenting on, following and favouriting them",
        terimler: &[
            ("blog", 3),
            ("article", 2),
            ("post", 1),
            ("comment", 1),
            ("author", 1),
            ("publish", 1),
            ("feed", 1),
            ("favorite", 1),
            ("follow", 1),
            ("tag", 1),
            ("conduit", 2),
            ("makale", 2),
        ],
    },
    Alan {
        id: "wiki",
        ad: "wiki ve belge platformu",
        ad_en: "wiki and documentation platform",
        problem: "Bilgiyi ve belgeleri düzenli biçimde yazmak, saklamak ve bulmak",
        problem_en: "Writing, storing and finding information and documentation in an organised way",
        terimler: &[
            ("wiki", 3),
            ("documentation", 2),
            ("knowledge", 2),
            ("bookshelf", 2),
            ("bookshelve", 2),
            ("shelf", 1),
            ("shelve", 1),
            ("chapter", 1),
            ("book", 1),
            ("page", 1),
            ("revision", 1),
            ("dokumantasyon", 2),
        ],
    },
    Alan {
        id: "project",
        ad: "proje ve görev yönetimi",
        ad_en: "project and task management",
        problem: "Projeleri ve görevleri planlamak, atamak ve izlemek",
        problem_en: "Planning, assigning and tracking projects and tasks",
        terimler: &[("project", 1), ("task", 2), ("todo", 2), ("issue", 1), ("sprint", 2), ("kanban", 3), ("milestone", 2), ("assignee", 2), ("gorev", 2)],
    },
    Alan {
        id: "crm",
        ad: "müşteri ilişkileri yönetimi (CRM)",
        ad_en: "customer relationship management (CRM)",
        problem: "Müşteri adaylarını, görüşmeleri ve satış fırsatlarını izlemek",
        problem_en: "Tracking leads, contacts and sales opportunities",
        terimler: &[("crm", 3), ("lead", 2), ("contact", 1), ("deal", 2), ("opportunity", 2), ("pipeline", 1), ("customer", 1), ("prospect", 2), ("sales", 1), ("musteri", 1)],
    },
    Alan {
        id: "travel",
        ad: "rezervasyon ve seyahat operasyonu",
        ad_en: "booking and travel operations",
        problem: "Rezervasyonları, konaklamayı ve transferleri yönetmek",
        problem_en: "Managing reservations, stays and transfers",
        terimler: &[
            ("reservation", 2),
            ("booking", 2),
            ("hotel", 2),
            ("room", 1),
            ("guest", 1),
            ("flight", 2),
            ("transfer", 1),
            ("tour", 1),
            ("travel", 2),
            ("itinerary", 2),
            ("rezervasyon", 2),
            ("otel", 2),
            ("misafir", 1),
            ("ucus", 2),
            ("seyahat", 2),
        ],
    },
    Alan {
        id: "clinic",
        ad: "klinik ve hasta takibi",
        ad_en: "clinic and patient records",
        problem: "Hastaları, randevuları ve tedavileri takip etmek",
        problem_en: "Tracking patients, appointments and treatments",
        // "health" tek başına alınmaz: servis sağlık denetimi (healthcheck) her yerde geçer
        terimler: &[
            ("patient", 2),
            ("doctor", 2),
            ("appointment", 1),
            ("clinic", 2),
            ("prescription", 2),
            ("medical", 2),
            ("hospital", 2),
            ("healthcare", 3),
            ("ehr", 3),
            ("diagnosis", 2),
            ("nurse", 2),
            ("hasta", 2),
            ("doktor", 2),
        ],
    },
    Alan {
        id: "veterinary",
        ad: "veteriner kliniği",
        ad_en: "veterinary clinic",
        problem: "Evcil hayvanları, sahiplerini ve veteriner ziyaretlerini takip etmek",
        problem_en: "Tracking pets, their owners and vet visits",
        terimler: &[("pet", 2), ("vet", 2), ("veterinarian", 2), ("petclinic", 3), ("visit", 1), ("specialty", 1)],
    },
    Alan {
        id: "education",
        ad: "eğitim ve kurs platformu",
        ad_en: "learning and course platform",
        problem: "Kursları, dersleri ve öğrenci ilerlemesini yönetmek",
        problem_en: "Managing courses, lessons and student progress",
        terimler: &[
            ("course", 2),
            ("lesson", 2),
            ("student", 2),
            ("teacher", 2),
            ("instructor", 2),
            ("enrollment", 2),
            ("curriculum", 2),
            ("quiz", 2),
            ("classroom", 2),
            ("assignment", 1),
            ("grade", 1),
            ("lms", 3),
            ("elearning", 3),
            ("ogrenci", 2),
            ("ogretmen", 2),
        ],
    },
    Alan {
        id: "messaging",
        ad: "mesajlaşma",
        ad_en: "messaging",
        problem: "Kişiler ve gruplar arasında mesajlaşmak",
        problem_en: "Messaging between people and groups",
        // "channel" bildirim kanallarında, "message" sohbet asistanlarında da geçer: zayıf terimler alanı tek başına taşımaz
        terimler: &[("chat", 2), ("message", 1), ("conversation", 2), ("messenger", 3), ("messaging", 3), ("inbox", 1), ("sohbet", 2), ("mesajlasma", 3)],
    },
    Alan {
        id: "community",
        ad: "tartışma ve bağlantı paylaşım topluluğu",
        ad_en: "discussion and link-sharing community",
        problem: "Bir topluluğun bağlantı ve hikâye paylaşması, oylaması ve tartışması",
        problem_en: "Letting a community share, vote on and discuss links and stories",
        terimler: &[("forum", 3), ("thread", 1), ("story", 2), ("vote", 2), ("upvote", 2), ("karma", 2), ("moderation", 1), ("community", 1)],
    },
    Alan {
        id: "file-storage",
        ad: "dosya depolama ve paylaşım",
        ad_en: "file storage and sharing",
        problem: "Dosyaları saklamak, eşitlemek ve paylaşmak",
        problem_en: "Storing, syncing and sharing files",
        terimler: &[("drive", 2), ("folder", 2), ("storage", 1), ("sync", 1), ("klasor", 2)],
    },
    Alan {
        id: "hr",
        ad: "insan kaynakları",
        ad_en: "human resources",
        problem: "Çalışanları, izinleri, bordroyu ve işe alımı yönetmek",
        problem_en: "Managing employees, leave, payroll and hiring",
        terimler: &[("employee", 2), ("payroll", 3), ("department", 1), ("salary", 2), ("recruitment", 2), ("candidate", 1), ("calisan", 2), ("maas", 2)],
    },
    Alan {
        id: "accounting",
        ad: "muhasebe ve faturalama",
        ad_en: "accounting and invoicing",
        problem: "Faturaları, giderleri ve hesap kayıtlarını tutmak",
        problem_en: "Keeping invoices, expenses and ledger records",
        terimler: &[
            ("invoice", 2),
            ("ledger", 3),
            ("accounting", 3),
            ("bookkeeping", 3),
            ("receivable", 2),
            ("payable", 2),
            ("expense", 1),
            ("tax", 1),
            ("fatura", 2),
            ("muhasebe", 3),
        ],
    },
    Alan {
        id: "personal-finance",
        ad: "kişisel finans ve bütçe",
        ad_en: "personal finance and budgeting",
        problem: "Gelirleri, harcamaları, hesapları ve bütçeleri tek yerde izlemek",
        problem_en: "Tracking income, spending, accounts and budgets in one place",
        terimler: &[
            ("finance", 3),
            ("financial", 2),
            ("budget", 2),
            ("budgeting", 3),
            ("networth", 3),
            ("transaction", 1),
            ("expense", 1),
            ("income", 2),
            ("spending", 2),
            ("saving", 2),
            ("investment", 2),
            ("portfolio", 2),
            ("holding", 1),
            ("bank", 2),
            ("banking", 2),
            ("loan", 1),
            ("mortgage", 2),
            ("plaid", 2),
            ("merchant", 1),
            ("finans", 3),
            ("butce", 2),
            ("harcama", 2),
            ("gelir", 2),
        ],
    },
    Alan {
        id: "monitoring",
        ad: "izleme ve uyarı (uptime / cron)",
        ad_en: "monitoring and alerting (uptime / cron)",
        problem: "Servislerin ve zamanlanmış işlerin çalıştığını izlemek, aksayınca uyarı göndermek",
        problem_en: "Watching that services and scheduled jobs keep running and alerting when they fail",
        terimler: &[
            ("monitoring", 3),
            ("monitor", 2),
            ("uptime", 3),
            ("downtime", 2),
            ("cron", 3),
            ("heartbeat", 2),
            ("healthcheck", 3),
            ("alert", 2),
            ("alerting", 2),
            ("incident", 2),
            ("outage", 2),
            ("ping", 1),
            ("izleme", 2),
            ("uyari", 2),
        ],
    },
    Alan {
        id: "iot",
        ad: "nesnelerin interneti (cihaz ve sensör)",
        ad_en: "internet of things (devices and sensors)",
        problem: "Cihazlardan gelen sensör verisini toplamak, izlemek ve cihazları yönetmek",
        problem_en: "Collecting and watching sensor data from devices and managing the devices",
        terimler: &[("iot", 3), ("telemetry", 2), ("sensor", 2), ("mqtt", 3), ("firmware", 2), ("device", 1), ("reading", 1), ("gateway", 1), ("cihaz", 1)],
    },
    Alan {
        id: "helpdesk",
        ad: "destek talebi ve yardım masası",
        ad_en: "helpdesk and support tickets",
        problem: "Müşteri destek taleplerini almak, atamak ve çözmek",
        problem_en: "Receiving, assigning and resolving customer support requests",
        terimler: &[("helpdesk", 3), ("servicedesk", 3), ("ticket", 1), ("support", 1), ("sla", 2), ("escalation", 2), ("destek", 2)],
    },
    Alan {
        id: "inventory",
        ad: "stok ve depo yönetimi",
        ad_en: "inventory and warehouse management",
        problem: "Stokları, depoları ve tedarikçileri izlemek",
        problem_en: "Tracking stock, warehouses and suppliers",
        terimler: &[("inventory", 3), ("warehouse", 3), ("stock", 1), ("sku", 2), ("supplier", 2), ("envanter", 3), ("stok", 2)],
    },
    Alan {
        id: "newsletter",
        ad: "e-posta bülteni ve pazarlama",
        ad_en: "newsletters and e-mail marketing",
        problem: "Abone listelerine bülten ve kampanya e-postaları göndermek",
        problem_en: "Sending newsletters and campaign e-mails to subscriber lists",
        terimler: &[("newsletter", 3), ("subscriber", 2), ("campaign", 1), ("mailing", 2), ("unsubscribe", 2), ("bulten", 3)],
    },
    Alan {
        id: "food",
        ad: "yemek, tarif ve restoran",
        ad_en: "food, recipes and restaurants",
        problem: "Yemekleri, tarifleri ya da restoran siparişlerini yönetmek",
        problem_en: "Managing dishes, recipes or restaurant orders",
        terimler: &[("restaurant", 3), ("dish", 2), ("meal", 2), ("recipe", 2), ("ingredient", 2), ("restoran", 3), ("yemek", 2), ("tarif", 2)],
    },
    Alan {
        id: "events",
        ad: "etkinlik ve bilet",
        ad_en: "events and ticketing",
        problem: "Etkinlikleri düzenlemek ve bilet satmak",
        problem_en: "Organising events and selling tickets",
        terimler: &[("venue", 2), ("attendee", 2), ("concert", 2), ("festival", 2), ("bilet", 2)],
    },
    Alan {
        id: "real-estate",
        ad: "emlak ve kiralama",
        ad_en: "real estate and rentals",
        problem: "Mülkleri listelemek, kiralamak ve kiracıları izlemek",
        problem_en: "Listing and renting properties and tracking tenants",
        terimler: &[("rent", 2), ("lease", 2), ("landlord", 3), ("emlak", 3), ("kira", 2)],
    },
];

/// Uygulamanın türü (iş alanından ayrı): proje şablonu ya da örnek uygulama. Yalnızca belgenin başlığından, ilk
/// açıklamasından ve paket tanımından okunur
struct UygulamaTuru {
    id: &'static str,
    ad: &'static str,
    ad_en: &'static str,
    problem: &'static str,
    problem_en: &'static str,
    terimler: &'static [(&'static str, u32)],
}

static TURLER: &[UygulamaTuru] = &[
    UygulamaTuru {
        id: "template",
        ad: "proje şablonu (başlangıç iskeleti)",
        ad_en: "project template (starter)",
        problem: "Yeni bir projeye hazır, çalışan bir başlangıç iskeleti sağlamak",
        problem_en: "Giving a new project a ready, working starting point",
        terimler: &[("template", 3), ("boilerplate", 3), ("starter", 2), ("scaffold", 2), ("skeleton", 2), ("cookiecutter", 3), ("sablon", 3)],
    },
    UygulamaTuru {
        id: "example",
        ad: "örnek ya da gösterim uygulaması",
        ad_en: "example or demo application",
        problem: "Bir teknolojinin ya da API belirtiminin gerçekçi bir uygulamada nasıl kullanıldığını göstermek",
        problem_en: "Showing how a technology or an API specification is used in a realistic application",
        terimler: &[("example", 2), ("sample", 2), ("demo", 2), ("tutorial", 2), ("realworld", 3), ("ornek", 2)],
    },
];

// ---- Kaynaklar ----

/// Kanıt kaynağının ağırlığı: uygulamanın kendi beyanı (README başlığı ve ilk açıklaması, paket ve OpenAPI açıklaması)
/// 4, README'nin geri kalanı 2, tablo 2, yol ve modül 1, rol 1. Beyan kod adlarından ağır basar
fn agirlik(kaynak: &str, bas: bool) -> u32 {
    match kaynak {
        "paket" | "openapi" => 4,
        "belge" if bas => 4,
        "belge" | "tablo" => 2,
        _ => 1,
    }
}

/// Kanıt uygulamanın kendi beyanından mı (README başlığı / ilk açıklaması, paket ya da OpenAPI açıklaması)
fn beyan_mi(i: &Ipucu) -> bool {
    matches!(i.kaynak, "paket" | "openapi") || (i.kaynak == "belge" && i.bas)
}

/// Kanıt kaynağının kartta yazılan İngilizce adı
fn kanit_turu_en(kaynak: &str) -> &'static str {
    match kaynak {
        "belge" => "readme",
        "paket" => "package",
        "openapi" => "openapi",
        "tablo" => "table",
        "rota" => "path",
        "rol" => "role",
        _ => "other",
    }
}

fn kaynak_metni(kaynak: &str, en: bool) -> &'static str {
    match (kaynak, en) {
        ("belge", false) => "README",
        ("paket", false) => "paket tanımı",
        ("openapi", false) => "OpenAPI",
        ("tablo", false) => "tablo",
        ("rota", false) => "yol",
        ("rol", false) => "rol/izin adı",
        ("belge", true) => "README",
        ("paket", true) => "package manifest",
        ("openapi", true) => "OpenAPI",
        ("tablo", true) => "table",
        ("rota", true) => "path",
        ("rol", true) => "role/permission name",
        _ => "?",
    }
}

/// Bir kaynak satırı: tür, uygulama köküne göre dosya, satır, metin ve dayanak özeti
#[derive(Clone)]
struct Satir {
    kaynak: &'static str,
    dosya: String,
    satir: u64,
    metin: String,
    /// Başlık ya da ilk açıklama (tür yalnızca bunlardan okunur)
    bas: bool,
    /// README liste öğesi (özellik listesi amaç ya da problem alıntısı olmaz)
    liste: bool,
}

/// Belgeler: README, paket tanımı ve OpenAPI belgesi. Uygulama klasöründe, yoksa depo kökünde (en çok üç üst klasör)
/// aranır; dosyalar uygulama köküne göre adlandırılır
struct Belgeler {
    satirlar: Vec<Satir>,
    /// Dosya -> SHA-256 (geçerlilik: belge değişince alıntı da eskir)
    ozetler: HashMap<String, String>,
}

fn goreli(kok: &Path, p: &Path) -> String {
    crate::kanit::goreli_yol(kok, p).unwrap_or_else(|| p.file_name().map(|a| a.to_string_lossy().to_string()).unwrap_or_default())
}

/// Bir deponun alt projesi olduğunu gösteren klasör adları: belge bunların üstündeki depo kökünde olabilir
const ALT_PROJE: &[&str] = &["backend", "server", "api", "app", "src", "service", "services", "web", "apps", "packages", "frontend", "client", "core"];

/// Belgenin aranacağı klasörler: uygulama kökü; uygulama bir alt projeyse (klasör adı backend, server … ya da üstü bir
/// git deposu) en çok üç üst klasöre kadar depo kökü. Başka bir projenin README'si okunmasın diye tanınmayan üste çıkılmaz
fn klasorler(kok: &Path) -> Vec<PathBuf> {
    let mut v = vec![kok.to_path_buf()];
    let mut su = kok.to_path_buf();
    for _ in 0..3 {
        if su.join(".git").exists() {
            break;
        }
        let ad = su.file_name().map(|a| a.to_string_lossy().to_lowercase()).unwrap_or_default();
        let Some(ust) = su.parent().map(Path::to_path_buf) else { break };
        if !(ALT_PROJE.contains(&ad.as_str()) || ust.join(".git").exists()) {
            break;
        }
        v.push(ust.clone());
        su = ust;
    }
    v
}

fn belge_oku(kok: &Path) -> Belgeler {
    let mut b = Belgeler { satirlar: vec![], ozetler: HashMap::new() };
    let mut paket_bulundu = false;
    let kok = &std::fs::canonicalize(kok).unwrap_or_else(|_| kok.to_path_buf());
    let klasorler = klasorler(kok);
    for k in &klasorler {
        let girdiler: Vec<PathBuf> = std::fs::read_dir(k).into_iter().flatten().flatten().map(|g| g.path()).collect();
        let mut adlar: Vec<(String, PathBuf)> = girdiler.iter().filter_map(|p| Some((p.file_name()?.to_string_lossy().to_lowercase(), p.clone()))).collect();
        adlar.sort();
        {
            let readme = adlar.iter().find(|(a, _)| a == "readme.md").or_else(|| adlar.iter().find(|(a, _)| a == "readme" || a == "readme.rst" || a == "readme.txt"));
            if let Some((_, p)) = readme {
                if let Some(m) = crate::guvenli::oku(p) {
                    let ad = goreli(kok, p);
                    readme_satirlari(&m, &ad, &mut b.satirlar);
                    if let Some(o) = crate::kanit::dosya_ozeti(p) {
                        b.ozetler.insert(ad, o);
                    }
                }
            }
        }
        if !paket_bulundu {
            for (a, p) in &adlar {
                let Some(m) = (match a.as_str() {
                    "package.json" | "composer.json" | "pyproject.toml" | "pom.xml" | "cargo.toml" | "setup.cfg" => crate::guvenli::oku(p),
                    x if x.ends_with(".csproj") || x.ends_with(".gemspec") => crate::guvenli::oku(p),
                    _ => None,
                }) else {
                    continue;
                };
                let ad = goreli(kok, p);
                let once = b.satirlar.len();
                paket_satirlari(&m, a, &ad, &mut b.satirlar);
                if b.satirlar.len() > once {
                    paket_bulundu = true;
                    if let Some(o) = crate::kanit::dosya_ozeti(p) {
                        b.ozetler.insert(ad, o);
                    }
                }
            }
        }
    }
    // OpenAPI belgesi: köklerde ve bir alt klasörde adı openapi / swagger geçen JSON ya da YAML (başlık ve açıklama)
    let mut adaylar: Vec<PathBuf> = vec![];
    for k in &klasorler {
        for g in std::fs::read_dir(k).into_iter().flatten().flatten() {
            let p = g.path();
            if p.is_dir() {
                let ad = p.file_name().map(|a| a.to_string_lossy().to_lowercase()).unwrap_or_default();
                if ad.starts_with('.') || ad == "node_modules" || ad == "vendor" {
                    continue;
                }
                adaylar.extend(std::fs::read_dir(&p).into_iter().flatten().flatten().map(|x| x.path()).filter(|x| x.is_file()));
            } else {
                adaylar.push(p);
            }
        }
    }
    adaylar.sort();
    for p in adaylar {
        let ad = p.file_name().map(|a| a.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !(ad.contains("openapi") || ad.contains("swagger")) || !(ad.ends_with(".json") || ad.ends_with(".yaml") || ad.ends_with(".yml")) {
            continue;
        }
        let Some(m) = crate::guvenli::oku(&p) else { continue };
        let goreli_ad = goreli(kok, &p);
        let once = b.satirlar.len();
        openapi_satirlari(&m, &goreli_ad, &mut b.satirlar);
        if b.satirlar.len() > once {
            if let Some(o) = crate::kanit::dosya_ozeti(&p) {
                b.ozetler.insert(goreli_ad, o);
            }
            break;
        }
    }
    b
}

/// Markdown/HTML biçimini atar: bağlantı metni kalır, görseller ve rozetler gider
fn duz_metin(s: &str) -> String {
    let mut o = String::new();
    let mut etiket = false;
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '<' {
            etiket = true;
        } else if c == '>' && etiket {
            etiket = false;
        } else if !etiket {
            // ![alt](url) görsel: atılır; [metin](url) bağlantı: metin kalır
            if c == '!' && cs.get(i + 1) == Some(&'[') {
                let mut d = 0;
                while i < cs.len() {
                    if cs[i] == '(' {
                        d += 1;
                    }
                    if cs[i] == ')' {
                        d -= 1;
                        if d <= 0 {
                            break;
                        }
                    }
                    i += 1;
                }
            } else if c == ']' && cs.get(i + 1) == Some(&'(') {
                while i < cs.len() && cs[i] != ')' {
                    i += 1;
                }
            } else if !matches!(c, '[' | '*' | '`' | '_' | '#') || (c == '_' && i > 0 && cs[i - 1].is_alphanumeric()) {
                o.push(c);
            }
        }
        i += 1;
    }
    o.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Kurulum ve geliştirme bölümleri anlam taşımaz: README bunlardan birinde biter
const BITIS_BASLIKLARI: &[&str] = &[
    "install",
    "getting started",
    "usage",
    "development",
    "prerequisite",
    "setup",
    "contributing",
    "license",
    "build",
    "requirements",
    "quick start",
    "running",
    "deploy",
    "env",
    "table of contents",
    "kurulum",
    // Teknik bölümler (güvenlik, yapılandırma, mimari, veritabanı …) uygulamanın ne olduğunu anlatmaz
    "security",
    "how it works",
    "database",
    "configuration",
    "code overview",
    "architecture",
    "dependencies",
    "changelog",
    "authentication",
    "folder",
    "structure",
    "testing",
    "faq",
    "troubleshoot",
];

/// README'nin anlatım bölümü: başlıklar, paragraflar (ardışık satırlar tek paragraf, satır numarası ilk satırın) ve
/// liste öğeleri; kurulum ya da geliştirme başlığında biter. Rozet, görsel, bağlantı listesi ve kod atlanır
fn readme_satirlari(m: &str, dosya: &str, out: &mut Vec<Satir>) {
    let mut kod = false;
    let mut ilk_baslik = true;
    let mut ilk_paragraf = true;
    // Süren paragraf: (ilk satır, metin)
    let mut paragraf: Option<(u64, String)> = None;
    let baslangic = out.len();
    // GitHub uyarı kutusu (`> [!IMPORTANT]`): bakım ya da sürüm notudur, uygulamayı anlatmaz
    let mut uyari = false;
    let bitir = |p: &mut Option<(u64, String)>, out: &mut Vec<Satir>, ilk_paragraf: &mut bool| {
        if let Some((satir, metin)) = p.take() {
            if metin.split_whitespace().count() >= 3 {
                out.push(Satir { kaynak: "belge", dosya: dosya.into(), satir, metin, bas: *ilk_paragraf, liste: false });
                *ilk_paragraf = false;
            }
        }
    };
    for (i, ham) in m.lines().enumerate().take(200) {
        let s = ham.trim();
        if s.starts_with("```") {
            kod = !kod;
            bitir(&mut paragraf, out, &mut ilk_paragraf);
            continue;
        }
        if kod || s.is_empty() {
            bitir(&mut paragraf, out, &mut ilk_paragraf);
            continue;
        }
        if s.starts_with("> [!") || (uyari && s.starts_with('>')) {
            uyari = true;
            bitir(&mut paragraf, out, &mut ilk_paragraf);
            continue;
        }
        uyari = false;
        let baslik = s.starts_with('#') || s.starts_with("<h1") || s.starts_with("<h2") || s.starts_with("<h3") || s.trim_start_matches(['>', ' ']).starts_with('#');
        let metin = duz_metin(s.trim_start_matches('>').trim());
        let kucuk = sade(&metin);
        let liste =
            s.starts_with("- ") || s.starts_with("* ") || s.starts_with("+ ") || s.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if baslik || liste {
            bitir(&mut paragraf, out, &mut ilk_paragraf);
        }
        // Başlığı yalnızca görsel olan README'de ilk metin başlığı da bitiş başlığı olabilir: önce anlatım toplanmış olmalı
        if baslik && (!ilk_baslik || out.len() > baslangic) && BITIS_BASLIKLARI.iter().any(|b| kucuk.contains(b)) {
            break;
        }
        // Rozet, görsel, bağlantı ve tablo satırları anlam taşımaz (süren paragrafı da bitirir)
        if metin.is_empty() || s.starts_with('|') || s.starts_with("[![") || s.starts_with("<img") || s.starts_with("<a ") || s.starts_with("<p") {
            bitir(&mut paragraf, out, &mut ilk_paragraf);
            continue;
        }
        let kelime = metin.split_whitespace().count();
        if baslik {
            out.push(Satir { kaynak: "belge", dosya: dosya.into(), satir: i as u64 + 1, metin, bas: ilk_baslik, liste: false });
            ilk_baslik = false;
        } else if liste {
            if kelime >= 3 {
                out.push(Satir { kaynak: "belge", dosya: dosya.into(), satir: i as u64 + 1, metin, bas: false, liste: true });
            }
        } else {
            // Paragraf ardışık satırlardır: kısa devam satırı da paragrafın parçasıdır
            match paragraf.as_mut() {
                Some((_, p)) => {
                    p.push(' ');
                    p.push_str(&metin);
                }
                None => paragraf = Some((i as u64 + 1, metin)),
            }
        }
    }
    bitir(&mut paragraf, out, &mut ilk_paragraf);
}

fn paket_satirlari(m: &str, ad: &str, dosya: &str, out: &mut Vec<Satir>) {
    let satir_no = |anahtar: &str| m.lines().position(|l| l.contains(anahtar)).map(|i| i as u64 + 1).unwrap_or(1);
    let mut ekle = |metin: String, anahtar: &str| {
        let metin = metin.trim().to_string();
        // "Monorepo for X" ve boş açıklama uygulamayı anlatmaz
        if metin.is_empty() || sade(&metin).starts_with("monorepo") {
            return;
        }
        out.push(Satir { kaynak: "paket", dosya: dosya.into(), satir: satir_no(anahtar), metin, bas: true, liste: false });
    };
    if ad.ends_with(".json") {
        let Ok(v) = serde_json::from_str::<Value>(m) else { return };
        if let Some(d) = v["description"].as_str() {
            ekle(d.to_string(), "\"description\"");
        }
        let kel: Vec<&str> = v["keywords"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if !kel.is_empty() {
            ekle(kel.join(", "), "\"keywords\"");
        }
        return;
    }
    // TOML, XML, gemspec: yalnızca açıklama satırı (değer tırnak ya da etiket içinde)
    for l in m.lines() {
        let t = l.trim();
        let deger = if let Some(r) = t.strip_prefix("description").map(str::trim_start).and_then(|r| r.strip_prefix('=')) {
            Some(r.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
        } else if let Some(r) = t.strip_prefix("<description>").or_else(|| t.strip_prefix("<Description>")) {
            Some(r.split("</").next().unwrap_or("").to_string())
        } else {
            t.strip_prefix("spec.summary")
                .or_else(|| t.strip_prefix("s.summary"))
                .map(|r| r.trim_start_matches([' ', '=']).trim().trim_matches(|c| c == '"' || c == '\'').to_string())
        };
        if let Some(d) = deger {
            ekle(d, t);
            break;
        }
    }
}

fn openapi_satirlari(m: &str, dosya: &str, out: &mut Vec<Satir>) {
    let satir_no = |anahtar: &str| m.lines().position(|l| l.contains(anahtar)).map(|i| i as u64 + 1).unwrap_or(1);
    let (baslik, aciklama) = if dosya.ends_with(".json") {
        let Ok(v) = serde_json::from_str::<Value>(m) else { return };
        (v["info"]["title"].as_str().map(String::from), v["info"]["description"].as_str().map(String::from))
    } else {
        // YAML: `info:` altındaki ilk title ve description (girintili)
        let mut icinde = false;
        let (mut t, mut d) = (None, None);
        for l in m.lines() {
            if l.starts_with("info:") {
                icinde = true;
                continue;
            }
            if icinde && !l.starts_with(' ') && !l.trim().is_empty() {
                break;
            }
            let x = l.trim();
            if icinde {
                if let Some(r) = x.strip_prefix("title:") {
                    t = Some(r.trim().trim_matches(|c| c == '"' || c == '\'').to_string());
                } else if let Some(r) = x.strip_prefix("description:") {
                    d = Some(r.trim().trim_matches(|c| c == '"' || c == '\'').to_string());
                }
            }
        }
        (t, d)
    };
    for (deger, anahtar) in [(baslik, "title"), (aciklama, "description")] {
        if let Some(v) = deger.filter(|v| !v.trim().is_empty() && v.len() < 400) {
            out.push(Satir { kaynak: "openapi", dosya: dosya.into(), satir: satir_no(anahtar), metin: v, bas: true, liste: false });
        }
    }
}

// ---- Sınıflandırma ----

/// Bir kanıt parçası: terimin bulunduğu kaynak ve yer
#[derive(Clone)]
struct Ipucu {
    kaynak: &'static str,
    terim: String,
    dosya: String,
    satir: u64,
    alinti: Option<String>,
    /// Beyanın başı (README başlığı ya da ilk açıklaması)
    bas: bool,
    /// Terimin sözlükteki ağırlığı
    agirlik: u32,
}

struct Siniflama {
    /// (alan sırası, puan, ipuçları)
    puanlar: Vec<(usize, u32, Vec<Ipucu>)>,
}

/// Kanıt metinleri: (kaynak, dosya, satır, kelimeler, alıntı, beyanın başı mı)
type Metin = (&'static str, String, u64, Vec<String>, Option<String>, bool);

fn puanla(metinler: &[Metin], terimler: &[(&str, u32)]) -> (u32, Vec<Ipucu>) {
    // (kaynak, terim) bir kez sayılır: aynı tablonun yüz yolda geçmesi kanıtı büyütmez. Terim README'nin hem başında
    // hem gövdesinde geçiyorsa baştaki (ağır) sayılır
    let mut gorulen: BTreeMap<(&str, &str), Ipucu> = BTreeMap::new();
    let terim_agirligi = |t: &str| terimler.iter().find(|(x, _)| *x == t).map(|x| x.1).unwrap_or(1);
    for (kaynak, dosya, satir, ws, alinti, bas) in metinler {
        for (t, _) in terimler {
            let yukselt = gorulen.get(&(*kaynak, *t)).is_some_and(|i| *bas && !i.bas);
            if ws.iter().any(|w| w == t) && (!gorulen.contains_key(&(*kaynak, *t)) || yukselt) {
                gorulen.insert(
                    (kaynak, t),
                    Ipucu { kaynak, terim: t.to_string(), dosya: dosya.clone(), satir: *satir, alinti: alinti.clone(), bas: *bas, agirlik: terim_agirligi(t) },
                );
            }
        }
    }
    let p = gorulen.values().map(|i| agirlik(i.kaynak, i.bas) * i.agirlik).sum();
    let mut ipuclari: Vec<Ipucu> = gorulen.into_values().collect();
    let agir = |i: &Ipucu| agirlik(i.kaynak, i.bas) * i.agirlik;
    ipuclari.sort_by(|a, b| agir(b).cmp(&agir(a)).then(a.kaynak.cmp(b.kaynak)).then(a.terim.cmp(&b.terim)));
    (p, ipuclari)
}

fn siniflandir(metinler: &[Metin]) -> Siniflama {
    let mut puanlar: Vec<(usize, u32, Vec<Ipucu>)> =
        ALANLAR.iter().enumerate().map(|(i, a)| (i, puanla(metinler, a.terimler))).map(|(i, (p, ip))| (i, p, ip)).filter(|x| x.1 > 0).collect();
    puanlar.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Siniflama { puanlar }
}

/// Sınıflandırma kabul eşiği: en az 8 puan, en az iki farklı kaynak türü ve iki farklı terim; ikinci aday birincinin
/// %60'ını geçmemeli. Uygulamanın kendi beyanı (README başı, paket açıklaması) birinciyi desteklemiyorsa kanıt yalnızca
/// kod adlarındandır: daha sıkı eşik (10 puan, üç terim, ikinci aday en çok %50) ve beyan başka bir alanı açıkça
/// söylüyorsa (beyanda en az 8 puan) karar verilmez. Geçmezse alan bilinmiyor (adaylar notta)
fn kabul(s: &Siniflama) -> Result<usize, String> {
    let Some((i, p, ip)) = s.puanlar.first() else {
        return Err("alan sözlüğüyle eşleşen hiçbir kanıt yok".into());
    };
    let turler: BTreeSet<&str> = ip.iter().map(|x| x.kaynak).collect();
    let terimler: BTreeSet<&str> = ip.iter().map(|x| x.terim.as_str()).collect();
    let adaylar = s.puanlar.iter().take(3).map(|(j, q, _)| format!("{} ({q})", ALANLAR[*j].ad)).collect::<Vec<_>>().join(", ");
    if *p < 8 || turler.len() < 2 || terimler.len() < 2 {
        return Err(format!("kanıt yetersiz (en az 8 puan, iki kaynak türü ve iki terim gerekir); adaylar: {adaylar}"));
    }
    let beyanli = ip.iter().any(beyan_mi);
    let beyan_puani = |ip: &[Ipucu]| -> u32 { ip.iter().filter(|i| beyan_mi(i)).map(|i| agirlik(i.kaynak, i.bas) * i.agirlik).sum() };
    if !beyanli && s.puanlar.iter().skip(1).any(|(_, _, ip2)| beyan_puani(ip2) >= 8) {
        return Err(format!("uygulamanın beyanı ile kod adları farklı alanları gösteriyor; adaylar: {adaylar}"));
    }
    if !beyanli && (*p < 10 || terimler.len() < 3) {
        return Err(format!("kanıt yalnızca kod adlarından ve zayıf (beyan yok: en az 10 puan ve üç terim gerekir); adaylar: {adaylar}"));
    }
    let oran = if beyanli { 6 } else { 5 };
    if let Some((_, q, _)) = s.puanlar.get(1) {
        if *q * 10 > *p * oran {
            return Err(format!("iki alan yarışıyor; adaylar: {adaylar}"));
        }
    }
    Ok(*i)
}

/// Alan sınıflandırılamadığında kanıt deposuna yazılan notun okunur hâli (seçilen dilde). Not depoda Türkçe durur;
/// İngilizcede bilinen kalıplar çevrilir, aday alan adları İngilizce adlarıyla yazılır
pub fn siniflama_notu(not: Option<&str>) -> String {
    let Some(n) = not else { return t!("kanıt deposu yok", "no evidence store").into() };
    let n = n.trim_start_matches("iş alanı sınıflandırılamadı: ");
    if !crate::dil::en() {
        return n.to_string();
    }
    let adaylar = |s: &str| -> String {
        let mut s = s.to_string();
        for a in ALANLAR {
            s = s.replace(a.ad, a.ad_en);
        }
        s
    };
    if n.starts_with("alan sözlüğüyle eşleşen hiçbir kanıt yok") {
        "no evidence matches the domain dictionary".into()
    } else if let Some(a) = n.strip_prefix("kanıt yetersiz (en az 8 puan, iki kaynak türü ve iki terim gerekir); adaylar: ") {
        format!("not enough evidence (at least 8 points, two kinds of source and two terms are needed); candidates: {}", adaylar(a))
    } else if let Some(a) = n.strip_prefix("iki alan yarışıyor; adaylar: ") {
        format!("two domains compete; candidates: {}", adaylar(a))
    } else if let Some(a) = n.strip_prefix("uygulamanın beyanı ile kod adları farklı alanları gösteriyor; adaylar: ") {
        format!("the application's own description and its code names point to different domains; candidates: {}", adaylar(a))
    } else if let Some(a) = n.strip_prefix("kanıt yalnızca kod adlarından ve zayıf (beyan yok: en az 10 puan ve üç terim gerekir); adaylar: ") {
        format!("evidence comes only from code names and is weak (no self-description: at least 10 points and three terms are needed); candidates: {}", adaylar(a))
    } else {
        "the domain could not be classified".into()
    }
}

/// Kanıt deposundaki "iş alanı sınıflandırılamadı: …" notunun seçilen dildeki hâli; başka not olduğu gibi
pub fn siniflama_notu_ici(n: &str) -> String {
    match n.split_once("iş alanı sınıflandırılamadı: ") {
        Some((on, not)) if crate::dil::en() => format!("{on}the business domain could not be classified: {}", siniflama_notu(Some(not))),
        _ => n.to_string(),
    }
}

// ---- Kanıt deposuna yazım ----

/// Uygulama düğümü: `app:<ad>`
pub fn uygulama_dugumu(harita: &Value) -> String {
    format!("app:{}", harita["ozet"]["uygulama"].as_str().unwrap_or("?"))
}

/// Bir cümlenin anlatım işareti: açıklayıcı ("platform", "solution", "is a" …) ya da fayda / problem anlatan kelime
const ANLATIM: &[&str] = &["platform", "solution", "application", "app", "tool", "service", "system", "server", "codebase", "manager", "management", "is", "are"];
const PROBLEM_ISARETI: &[&str] = &[
    "without",
    "instead",
    "alternative",
    "helps",
    "allows",
    "lets",
    "easy",
    "simple",
    "selfhosted",
    "privacy",
    "control",
    "own",
    "free",
    "secure",
    "fast",
    "pleasant",
    "organising",
    "organizing",
    "storing",
];

/// Kodun okunmasından çıkan anlam önermeleri. `modeller`: tablo modelleri (ad, dosya, satır); `kok`: uygulama kökü
/// (belgeler buradan okunur; yoksa belge kanıtı yok)
pub fn onermeler(k: &mut Kurucu, harita: &Value, modeller: &[Value], kok: Option<&Path>, uretici: &str, dayanak: &mut dyn FnMut(&str) -> Vec<Dayanak>) {
    let app = uygulama_dugumu(harita);
    let kapilar: Vec<&Value> = harita["endpointler"].as_array().into_iter().flatten().filter(|k| k["ana"].as_bool() != Some(false)).collect();
    if harita["ozet"]["kaynaksiz"] == true {
        return;
    }
    let belgeler = kok.map(belge_oku).unwrap_or(Belgeler { satirlar: vec![], ozetler: HashMap::new() });
    let belge_dayanagi = |dosya: &str| -> Vec<Dayanak> { belgeler.ozetler.get(dosya).map(|o| vec![Dayanak { yol: dosya.to_string(), sha256: o.clone() }]).unwrap_or_default() };
    let konum = |kp: &Value| format!("{}:{}", kp["dosya"].as_str().unwrap_or(""), kp["satir"].as_u64().unwrap_or(0));

    // Kanıt metinleri: belge, paket, OpenAPI satırları; tablo adları; yol ve modül adları; rol adları
    let mut metinler: Vec<Metin> = vec![];
    for s in &belgeler.satirlar {
        let ws: Vec<String> = yazi_kelimeleri(&s.metin).into_iter().map(|w| tekil(&w)).collect();
        metinler.push((s.kaynak, s.dosya.clone(), s.satir, ws, Some(kisa(&s.metin, 200)), s.bas));
    }
    let mut tablolar: BTreeMap<String, String> = BTreeMap::new();
    for m in modeller {
        if let (Some(ad), Some(d)) = (m["ad"].as_str(), m["dosya"].as_str()) {
            tablolar.entry(ad.to_string()).or_insert_with(|| format!("{d}:{}", m["satir"].as_u64().unwrap_or(1)));
        }
    }
    for kp in &kapilar {
        for t in ["okur", "yazar"].iter().flat_map(|a| kp["veri"][a].as_array().into_iter().flatten()).filter_map(Value::as_str) {
            tablolar.entry(t.to_string()).or_insert_with(|| konum(kp));
        }
    }
    for (t, yer) in &tablolar {
        let (d, s) = yer.rsplit_once(':').unwrap_or((yer, "0"));
        metinler.push(("tablo", d.to_string(), s.parse().unwrap_or(0), kelimeler(t).iter().map(|w| tekil(w)).collect(), None, false));
    }
    for kp in &kapilar {
        let mut ws: Vec<String> = kp["yol"].as_str().unwrap_or("").split('/').filter(|p| !p.starts_with([':', '{'])).flat_map(kelimeler).map(|w| tekil(&w)).collect();
        ws.extend(kelimeler(kp["kontrolcu"].as_str().unwrap_or("")).iter().map(|w| tekil(w)));
        metinler.push(("rota", kp["dosya"].as_str().unwrap_or("").to_string(), kp["satir"].as_u64().unwrap_or(0), ws, None, false));
        let roller: Vec<String> = kp["roller"].as_array().into_iter().flatten().filter_map(Value::as_str).flat_map(kelimeler).map(|w| tekil(&w)).collect();
        if !roller.is_empty() {
            metinler.push(("rol", kp["dosya"].as_str().unwrap_or("").to_string(), kp["satir"].as_u64().unwrap_or(0), roller, None, false));
        }
    }

    let ipucu_kaniti = |ip: &Ipucu, tur: &str| -> Kanit {
        let d = if matches!(ip.kaynak, "belge" | "paket" | "openapi") { belge_dayanagi(&ip.dosya) } else { vec![] };
        let kt = match ip.kaynak {
            "openapi" => KaynakTuru::ApiSchema,
            "tablo" => KaynakTuru::DatabaseSchema,
            _ => KaynakTuru::SourceCode,
        };
        let mut ozet = json!({ "tur": tur, "kaynak": ip.kaynak, "terim": ip.terim });
        if let Some(a) = &ip.alinti {
            ozet["alinti"] = json!(a);
        }
        Kanit::yeni(kt, uretici, d, vec![format!("{}:{}", ip.dosya, ip.satir)], Some(Guven::Inferred))
            .ozetli(ozet, vec!["sözlük tabanlı sınıflandırma: ad ve beyan çıkarımı".into()])
    };

    // 1. Alan
    let sinif = siniflandir(&metinler);
    match kabul(&sinif) {
        Ok(i) => {
            let a = &ALANLAR[i];
            let deger = json!({ "id": a.id, "ad": a.ad, "ad_en": a.ad_en, "puan": sinif.puanlar[0].1 });
            for ip in sinif.puanlar[0].2.iter().take(16) {
                k.ekle(ipucu_kaniti(ip, "alan_ipucu"), vec![Iddia::yeni(&app, "ALAN", None, deger.clone(), Kapsam::Acik)]);
            }
        }
        Err(not) => k.bos(Iddia::yeni(&app, "ALAN", None, Value::Null, Kapsam::Bilinmiyor), &format!("iş alanı sınıflandırılamadı: {not}")),
    }
    // 2. Tür: yalnızca belgenin başlığı, ilk açıklaması ve paket tanımı
    let bas: Vec<Metin> = metinler.iter().filter(|m| m.5).cloned().collect();
    let mut tur_puan: Vec<(usize, u32, Vec<Ipucu>)> =
        TURLER.iter().enumerate().map(|(i, t)| (i, puanla(&bas, t.terimler))).map(|(i, (p, ip))| (i, p, ip)).filter(|x| x.1 >= 6).collect();
    tur_puan.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let tur = match (tur_puan.first(), tur_puan.get(1)) {
        (Some(a), Some(b)) if a.1 == b.1 => None,
        (Some(a), _) => Some(a),
        _ => None,
    };
    if let Some((i, p, ips)) = tur {
        let t = &TURLER[*i];
        let deger = json!({ "id": t.id, "ad": t.ad, "ad_en": t.ad_en, "puan": p });
        for ip in ips {
            k.ekle(ipucu_kaniti(ip, "tur_ipucu"), vec![Iddia::yeni(&app, "TUR", None, deger.clone(), Kapsam::Acik)]);
        }
    }
    // 3. Amaç: uygulamanın kendi beyanı (README'nin ilk açıklayıcı cümlesi, başlığı, paket açıklaması), alıntı olarak
    let mut amac: Vec<&Satir> = vec![];
    let aciklayici = |s: &Satir| {
        let ws = yazi_kelimeleri(&s.metin);
        !s.liste && ((s.bas && ws.len() >= 4) || (ws.len() >= 5 && ws.iter().any(|w| ANLATIM.contains(&w.as_str()))))
    };
    if let Some(s) = belgeler.satirlar.iter().find(|s| s.kaynak == "belge" && aciklayici(s)) {
        amac.push(s);
    }
    for s in belgeler.satirlar.iter().filter(|s| s.kaynak != "belge" && yazi_kelimeleri(&s.metin).len() >= 3 && !s.metin.contains(", ")) {
        if amac.len() < 3 && !amac.iter().any(|a| sade(&a.metin) == sade(&s.metin)) {
            amac.push(s);
        }
    }
    if amac.is_empty() {
        k.bos(Iddia::yeni(&app, "AMAC", None, Value::Null, Kapsam::Bilinmiyor), "README'de ya da paket tanımında uygulamayı anlatan bir cümle bulunamadı");
    }
    for s in &amac {
        let yer = format!("{}:{}", s.dosya, s.satir);
        let deger = json!({ "alinti": kisa(&s.metin, 300), "kaynak": s.kaynak });
        let kt = if s.kaynak == "openapi" { KaynakTuru::ApiSchema } else { KaynakTuru::SourceCode };
        k.ekle(
            Kanit::yeni(kt, uretici, belge_dayanagi(&s.dosya), vec![yer.clone()], Some(Guven::Inferred))
                .ozetli(json!({ "tur": "amac_alintisi", "kaynak": s.kaynak }), vec!["uygulamanın kendi beyanı: alıntıdır, doğrulanmadı".into()]),
            vec![Iddia::yeni(&app, "AMAC", Some(&yer), deger, Kapsam::Acik)],
        );
    }
    // 4. İş problemi: sözlükteki alan (ya da tür) tanımı + belgede fayda anlatan bir cümle (alıntı)
    let mut problem_var = false;
    if let Ok(i) = kabul(&sinif) {
        problem_var = true;
        let a = &ALANLAR[i];
        let deger = json!({ "metin": a.problem, "metin_en": a.problem_en, "dayanak": "alan", "alan": a.id });
        for ip in sinif.puanlar[0].2.iter().take(6) {
            k.ekle(ipucu_kaniti(ip, "problem_alan"), vec![Iddia::yeni(&app, "PROBLEM", Some("alan"), deger.clone(), Kapsam::Acik)]);
        }
    } else if let Some((i, _, ips)) = tur {
        problem_var = true;
        let t = &TURLER[*i];
        let deger = json!({ "metin": t.problem, "metin_en": t.problem_en, "dayanak": "tur", "tur": t.id });
        for ip in ips {
            k.ekle(ipucu_kaniti(ip, "problem_tur"), vec![Iddia::yeni(&app, "PROBLEM", Some("tur"), deger.clone(), Kapsam::Acik)]);
        }
    }
    let amac_satirlari: BTreeSet<(String, u64)> = amac.iter().map(|s| (s.dosya.clone(), s.satir)).collect();
    if let Some(s) = belgeler.satirlar.iter().find(|s| {
        s.kaynak == "belge" && !s.liste && !amac_satirlari.contains(&(s.dosya.clone(), s.satir)) && {
            let ws = yazi_kelimeleri(&s.metin);
            ws.len() >= 6 && ws.iter().any(|w| PROBLEM_ISARETI.contains(&w.as_str()))
        }
    }) {
        let yer = format!("{}:{}", s.dosya, s.satir);
        let deger = json!({ "alinti": kisa(&s.metin, 300), "dayanak": "alinti" });
        problem_var = true;
        k.ekle(
            Kanit::yeni(KaynakTuru::SourceCode, uretici, belge_dayanagi(&s.dosya), vec![yer.clone()], Some(Guven::Inferred))
                .ozetli(json!({ "tur": "problem_alintisi" }), vec!["uygulamanın kendi beyanı: alıntıdır, doğrulanmadı".into()]),
            vec![Iddia::yeni(&app, "PROBLEM", Some(&yer), deger, Kapsam::Acik)],
        );
    }
    if !problem_var {
        k.bos(Iddia::yeni(&app, "PROBLEM", None, Value::Null, Kapsam::Bilinmiyor), "iş alanı bilinmiyor ve belgede fayda anlatan bir cümle bulunamadı");
    }

    // 5. Kullanıcı tipleri ve akışlar
    kullanici_onermeleri(k, &app, &kapilar, uretici, dayanak);
    // 6. İş yetenekleri: aynı yeteneği gerçekleştiren işlemler tek düğümde (IMPLEMENTS kenarları kapılardan)
    let mut yetenekler: BTreeMap<String, (Yetenek, Vec<&Value>)> = BTreeMap::new();
    for kp in &kapilar {
        let y = yetenek(kp);
        yetenekler.entry(y.anahtar.clone()).or_insert_with(|| (y, vec![])).1.push(kp);
    }
    for (anahtar, (y, ks)) in &yetenekler {
        let islemler: Vec<&str> = ks.iter().filter_map(|k| k["id"].as_str()).collect();
        let deger = json!({ "ad": y.ad, "ad_en": y.ad_en, "kaynak": y.kaynak, "eylem": y.eylem, "islemler": islemler });
        let ilk = ks[0];
        let d = dayanak(ilk["dosya"].as_str().unwrap_or(""));
        k.ekle(
            Kanit::yeni(KaynakTuru::SourceCode, uretici, d, ks.iter().take(5).map(|x| konum(x)).collect(), Some(Guven::Inferred))
                .ozetli(json!({ "tur": "is_yetenegi" }), vec!["yetenek adı yoldan, fonksiyon adından ve HTTP yönteminden çıkarıldı".into()]),
            vec![Iddia::yeni(&format!("cap:{anahtar}"), "VAR", None, deger, Kapsam::Acik)],
        );
    }
}

/// Kayıt ve giriş akışlarının türleri: (kimlik, Türkçe ad, İngilizce ad)
const AKISLAR: &[(&str, &str, &str)] = &[
    ("kayit", "kayıt", "sign-up"),
    ("giris", "giriş", "log-in"),
    ("cikis", "çıkış", "log-out"),
    ("parola", "parola sıfırlama / değiştirme", "password reset / change"),
    ("dis_kimlik", "dış kimlik sağlayıcısıyla giriş (OAuth / SSO)", "external identity login (OAuth / SSO)"),
    ("dogrulama", "e-posta ya da hesap doğrulama", "e-mail or account verification"),
    ("cok_adimli", "çok adımlı doğrulama (2FA / PIN)", "multi-factor authentication (2FA / PIN)"),
];

fn akis_turu(kp: &Value) -> Option<&'static str> {
    let mut ws: Vec<String> = kelimeler(kp["yol"].as_str().unwrap_or(""));
    ws.extend(kelimeler(kp["fonksiyon"].as_str().unwrap_or("")));
    let ws: Vec<String> = yazi_kelimeleri(&ws.join(" ")).into_iter().map(|w| tekil(&w)).collect();
    let var = |l: &[&str]| ws.iter().any(|w| l.contains(&w.as_str()));
    let yontem = kp["yontem"].as_str().unwrap_or("").to_ascii_uppercase();
    if var(&["oauth", "sso", "oidc", "saml", "openid"]) {
        Some("dis_kimlik")
    } else if var(&["mfa", "2fa", "totp", "otp", "pin", "pincode"]) {
        Some("cok_adimli")
    } else if var(&["password", "passwd", "parola", "sifre"]) && var(&["reset", "recover", "recovery", "forgot", "change", "update", "sifirla", "unuttum", "degistir"]) {
        Some("parola")
    } else if var(&["signup", "register", "registration", "kayit", "kaydol", "uyeol"]) && yontem != "GET" {
        Some("kayit")
    } else if var(&["logout", "signout", "logoff", "cikis"]) {
        Some("cikis")
    } else if var(&["login", "signin", "authenticate", "giris"])
        || (yontem == "POST" && var(&["token"]) && var(&["access", "auth", "login"]))
        || (yontem == "POST"
            && (kp["yol"].as_str().unwrap_or("").trim_end_matches('/').ends_with("/auth")
                || kelimeler(kp["fonksiyon"].as_str().unwrap_or("")).first().is_some_and(|w| w == "auth")))
    {
        Some("giris")
    } else if var(&["verify", "verification", "confirm", "activate", "activation", "dogrula"]) && var(&["email", "account", "user", "mail", "eposta", "hesap"]) {
        Some("dogrulama")
    } else {
        None
    }
}

/// Yönetici işlemi: yolda `admin` parçası, fonksiyon adında admin ya da yönetici rolü
fn yonetici_mi(kp: &Value) -> bool {
    let yol = kelimeler(kp["yol"].as_str().unwrap_or(""));
    let fonk = kelimeler(kp["fonksiyon"].as_str().unwrap_or(""));
    let rol = kp["roller"].as_array().into_iter().flatten().filter_map(Value::as_str).any(|r| {
        let r = sade(r);
        ["admin", "superuser", "super_user", "root", "staff", "moderator", "owner", "yonetici", "manage"].iter().any(|x| r.contains(x))
    });
    yol.iter().any(|w| w == "admin" || w == "yonetim" || w == "yonetici") || fonk.iter().any(|w| w == "admin") || rol
}

/// Rol adı mı izin adı mı: noktalı ya da iki noktalı ad (ör. `album.create`, `items:write`) izindir
fn izin_adi_mi(r: &str) -> bool {
    const ROL: &[&str] = &[
        "admin",
        "administrator",
        "editor",
        "viewer",
        "user",
        "manager",
        "owner",
        "moderator",
        "member",
        "guest",
        "staff",
        "super",
        "superuser",
        "maintainer",
        "author",
        "reviewer",
        "yonetici",
    ];
    let ws = kelimeler(r);
    r.contains('.')
        || r.contains(':')
        || ws.iter().any(|w| eylem_kavrami(w).is_some() || ["access", "manage", "own", "embed", "collaborate", "write", "receive"].contains(&w.as_str()))
        || ((r.contains('-') || r.contains('_')) && !ws.iter().any(|w| ROL.contains(&w.as_str())))
}

/// Rol adı olarak anlamsız değer (çözülemeyen ifadeden kalan `true`, `false`, sayı)
fn anlamsiz_rol(r: &str) -> bool {
    let r = r.trim();
    r.is_empty() || r == "true" || r == "false" || r == "null" || r.chars().all(|c| c.is_ascii_digit())
}

fn kullanici_onermeleri(k: &mut Kurucu, app: &str, kapilar: &[&Value], uretici: &str, dayanak: &mut dyn FnMut(&str) -> Vec<Dayanak>) {
    let konum = |kp: &Value| format!("{}:{}", kp["dosya"].as_str().unwrap_or(""), kp["satir"].as_u64().unwrap_or(0));
    let id = |kp: &Value| kp["id"].as_str().unwrap_or("").to_string();
    let mut gruplar: Vec<(&str, &str, &str, &str, Vec<&Value>)> = vec![
        ("actor:anonymous", "anonim ziyaretçi", "anonymous visitor", "herkese açık işlemler (giriş kararı)", vec![]),
        ("actor:user", "giriş yapmış kullanıcı", "signed-in user", "giriş gerektiren, yönetici olmayan işlemler", vec![]),
        ("actor:admin", "yönetici", "administrator", "yolda ya da adda admin, yönetici rolü", vec![]),
        ("actor:api_client", "API anahtarı / belirteci ile istemci", "API key / token client", "istekten anahtar ya da belirteç okuyup 401 / 403 dönen guard", vec![]),
    ];
    for kp in kapilar {
        let anahtarli = kp["guardlar"].as_array().into_iter().flatten().filter_map(Value::as_str).any(|g| g.ends_with(ANAHTAR_GUARD_ETIKETI));
        let g = if yonetici_mi(kp) {
            2
        } else if anahtarli && kp["giris"] != "herkese_acik" {
            3
        } else {
            match kp["giris"].as_str() {
                Some("herkese_acik") => 0,
                Some("gerekli") | Some("istege_bagli") => 1,
                _ => continue,
            }
        };
        gruplar[g].4.push(kp);
    }
    for (aktor, ad, ad_en, dayanak_metni, ks) in &gruplar {
        if ks.is_empty() {
            continue;
        }
        let deger = json!({ "ad": ad, "ad_en": ad_en, "islem_sayisi": ks.len(), "ornekler": ks.iter().take(6).map(|x| id(x)).collect::<Vec<_>>(), "dayanak": dayanak_metni });
        let d = dayanak(ks[0]["dosya"].as_str().unwrap_or(""));
        k.ekle(
            Kanit::yeni(KaynakTuru::SourceCode, uretici, d, ks.iter().take(5).map(|x| konum(x)).collect(), Some(Guven::Inferred))
                .ozetli(json!({ "tur": "kullanici_tipi" }), vec!["kullanıcı tipi giriş kararından, yol ve rol adlarından çıkarıldı".into()]),
            vec![Iddia::yeni(aktor, "VAR", None, deger, Kapsam::Acik)],
        );
    }
    // Rol (izin değil) adları kendi kullanıcı tipidir
    let mut roller: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for kp in kapilar {
        for r in kp["roller"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|r| !izin_adi_mi(r) && !anlamsiz_rol(r)) {
            roller.entry(r.to_string()).or_default().push(kp);
        }
    }
    for (r, ks) in roller.iter().take(12) {
        let deger = json!({ "ad": format!("rol: {r}"), "ad_en": format!("role: {r}"), "islem_sayisi": ks.len(), "ornekler": ks.iter().take(6).map(|x| id(x)).collect::<Vec<_>>(), "dayanak": "koddaki rol adı", "rol": r });
        let d = dayanak(ks[0]["dosya"].as_str().unwrap_or(""));
        let aktor = format!("actor:role/{r}");
        k.ekle(
            Kanit::yeni(KaynakTuru::SourceCode, uretici, d, ks.iter().take(5).map(|x| konum(x)).collect(), Some(Guven::Inferred))
                .ozetli(json!({ "tur": "kullanici_tipi" }), vec!["rol adı kullanıcı tipi sayıldı".into()]),
            vec![Iddia::yeni(&aktor, "VAR", None, deger, Kapsam::Acik), Iddia::yeni(&aktor, "REFERENCES", Some(&format!("role:{r}")), json!({}), Kapsam::Acik)],
        );
    }
    // Akışlar
    let mut akislar: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for kp in kapilar {
        if let Some(t) = akis_turu(kp) {
            akislar.entry(t).or_default().push(kp);
        }
    }
    for (t, ad, ad_en) in AKISLAR {
        let Some(ks) = akislar.get(t) else { continue };
        let deger = json!({ "ad": ad, "ad_en": ad_en, "islemler": ks.iter().take(8).map(|x| id(x)).collect::<Vec<_>>() });
        let d = dayanak(ks[0]["dosya"].as_str().unwrap_or(""));
        k.ekle(
            Kanit::yeni(KaynakTuru::SourceCode, uretici, d, ks.iter().take(5).map(|x| konum(x)).collect(), Some(Guven::Inferred))
                .ozetli(json!({ "tur": "giris_akisi" }), vec!["akış yol ve fonksiyon adından çıkarıldı".into()]),
            vec![Iddia::yeni(app, "GIRIS_AKISI", Some(t), deger, Kapsam::Acik)],
        );
    }
}

fn kisa(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

// ---- Okuma: kanıt deposundan anlam özeti ----

fn etkin(o: &Onerme) -> Guven {
    if o.hesap.gecerlilik == Gecerlilik::Gecerli {
        o.hesap.guven
    } else {
        Guven::Unknown
    }
}

/// Önermenin dayandığı kanıtların yerleri ve özetleri
fn kanitlar_listesi(depo: &Depo, o: &Onerme, sinir: usize) -> Vec<Value> {
    o.hesap
        .destek
        .iter()
        .filter_map(|id| depo.kanitlar.iter().find(|k| &k.id == id))
        .take(sinir)
        .map(|k| json!({ "konum": k.konum.first(), "kaynak": k.ozet["kaynak"], "terim": k.ozet["terim"], "alinti": k.ozet["alinti"], "tur": k.kaynak_turu.ad() }))
        .collect()
}

/// Anlam özeti (kimsin, rapor, Ayna, kart): alan, tür, amaç alıntıları, problem, kullanıcı tipleri, akışlar ve iş
/// yetenekleri; her biri güveniyle (dayanağı değişmiş önerme bilinmiyor sayılır). Depo yoksa ya da kaynaksızsa Null
pub fn ozet(depo: &Depo) -> Value {
    let app = depo.onermeler.iter().find(|o| o.varlik.starts_with("app:") && matches!(o.yuklem.as_str(), "ALAN" | "AMAC" | "PROBLEM")).map(|o| o.varlik.clone());
    let Some(app) = app else { return Value::Null };
    let tek = |yuklem: &str| -> Value {
        let Some(o) = depo.onermeler.iter().find(|o| o.varlik == app && o.yuklem == yuklem) else { return Value::Null };
        let g = etkin(o);
        if o.deger.is_null() || g == Guven::Unknown {
            let neden = o.hesap.kapsam_disi.first().cloned().or_else(|| o.notlar.first().cloned());
            return json!({ "guven": "unknown", "not": neden });
        }
        let mut v = o.deger.clone();
        v["guven"] = json!(g.ad());
        v["kanitlar"] = json!(kanitlar_listesi(depo, o, 16));
        v
    };
    let coklu = |yuklem: &str| -> Vec<Value> {
        depo.onermeler
            .iter()
            .filter(|o| o.varlik == app && o.yuklem == yuklem && !o.deger.is_null())
            .map(|o| {
                let mut v = o.deger.clone();
                v["guven"] = json!(etkin(o).ad());
                v["konum"] = json!(o.hedef);
                v
            })
            .collect()
    };
    let mut problem = coklu("PROBLEM");
    // Alan / tür tanımı alıntıdan önce
    problem.sort_by_key(|p| if p["dayanak"] == "alinti" { 1 } else { 0 });
    let aktorler: Vec<Value> = depo
        .onermeler
        .iter()
        .filter(|o| o.varlik.starts_with("actor:") && o.yuklem == "VAR")
        .map(|o| {
            let mut v = o.deger.clone();
            v["id"] = json!(o.varlik);
            v["guven"] = json!(etkin(o).ad());
            v
        })
        .collect();
    let akislar: Vec<Value> = depo
        .onermeler
        .iter()
        .filter(|o| o.varlik == app && o.yuklem == "GIRIS_AKISI")
        .map(|o| {
            let mut v = o.deger.clone();
            v["tur"] = json!(o.hedef);
            v["guven"] = json!(etkin(o).ad());
            v
        })
        .collect();
    let mut yetenekler: Vec<Value> = depo
        .onermeler
        .iter()
        .filter(|o| o.varlik.starts_with("cap:") && o.yuklem == "VAR" && o.deger["islemler"].is_array())
        .map(|o| {
            let mut v = o.deger.clone();
            v["id"] = json!(o.varlik);
            v["guven"] = json!(etkin(o).ad());
            v
        })
        .collect();
    yetenekler.sort_by(|a, b| a["kaynak"].as_str().cmp(&b["kaynak"].as_str()).then(a["id"].as_str().cmp(&b["id"].as_str())));
    json!({
        "uygulama": app,
        "alan": tek("ALAN"),
        "tur": tek("TUR"),
        "amac": coklu("AMAC"),
        "problem": problem,
        "aktorler": aktorler,
        "akislar": akislar,
        "yetenekler": yetenekler,
    })
}

/// Kaynak başına yetenek grupları, en çok işlemi olan kaynak önce: [(kaynak adı, [yetenek adı …], işlem sayısı)]
pub fn kaynak_gruplari(a: &Value, en: bool) -> Vec<(String, Vec<String>, usize)> {
    let mut g: BTreeMap<String, (Vec<String>, usize)> = BTreeMap::new();
    for y in a["yetenekler"].as_array().into_iter().flatten() {
        let kaynak = y["kaynak"].as_str().unwrap_or("?");
        let grup = kaynak_adi(kaynak, en);
        // Alt kaynak yeteneğinin adı üst kaynağı önek olarak taşır ("gorevler: add yorumlar"); grubun altında yinelenmez
        let ad = y[if en { "ad_en" } else { "ad" }].as_str().unwrap_or("");
        let ad = ad.strip_prefix(&format!("{grup}: ")).unwrap_or(ad).to_string();
        let e = g.entry(grup).or_default();
        if !e.0.contains(&ad) {
            e.0.push(ad);
        }
        e.1 += y["islemler"].as_array().map(Vec::len).unwrap_or(0);
    }
    let mut v: Vec<(String, Vec<String>, usize)> = g.into_iter().map(|(k, (a, n))| (k, a, n)).collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    v
}

/// Alan kanıtının kısa anlatımı: "README.md:14 'photo', tablo exif"
pub fn kanit_ozeti(alan: &Value, en: bool, sinir: usize) -> String {
    alan["kanitlar"]
        .as_array()
        .into_iter()
        .flatten()
        .take(sinir)
        .map(|k| format!("{} {} '{}'", kaynak_metni(k["kaynak"].as_str().unwrap_or(""), en), k["konum"].as_str().unwrap_or(""), k["terim"].as_str().unwrap_or("")))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Alan ya da tür kanıtlarının okunur listesi (Ayna örnekleri): "README README.md:14 · photo"
pub fn kanit_ornekleri(v: &Value, sinir: usize) -> Vec<String> {
    v["kanitlar"]
        .as_array()
        .into_iter()
        .flatten()
        .take(sinir)
        .map(|k| format!("{} {} · {}", kaynak_metni(k["kaynak"].as_str().unwrap_or(""), crate::dil::en()), k["konum"].as_str().unwrap_or(""), k["terim"].as_str().unwrap_or("")))
        .collect()
}

fn damga(g: &str, en: bool) -> &'static str {
    match (g, en) {
        ("inferred", false) => "[Yorum]",
        ("inferred", true) => "[Inferred]",
        ("observed", false) => "[Gözlem]",
        ("observed", true) => "[Observed]",
        ("certain", false) => "[Kesin]",
        ("certain", true) => "[Certain]",
        (_, false) => "[Bilinmiyor]",
        (_, true) => "[Unknown]",
    }
}

/// `kimsin` eki: uygulama nedir, neden var, hangi problemi çözer, kullanıcıları kim, iş düzeyinde ne yapılabilir
pub fn kimsin_metni(depo: &Depo, en: bool, gorunur: &dyn Fn(&str) -> bool) -> String {
    let mut a = ozet(depo);
    if a.is_null() {
        return String::new();
    }
    // Görünmeyen işlemlerin yetenekleri ve akışları listelenmez (token görünürlüğü)
    let gorunur_mu = |v: &Value| v["islemler"].as_array().is_some_and(|l| l.iter().filter_map(Value::as_str).any(gorunur));
    for alan in ["yetenekler", "akislar"] {
        let suzulmus: Vec<Value> = a[alan].as_array().into_iter().flatten().filter(|v| gorunur_mu(v)).cloned().collect();
        a[alan] = json!(suzulmus);
    }
    let mut s = String::new();
    let alan = &a["alan"];
    if alan["guven"] == "unknown" {
        s.push_str(&if en {
            "What it is: unknown [Unknown] (not enough evidence for one business domain)\n".to_string()
        } else {
            format!("Ne: bilinmiyor [Bilinmiyor] ({})\n", alan["not"].as_str().unwrap_or("alan kanıtı yok"))
        });
    } else if !alan.is_null() {
        s.push_str(&format!(
            "{}: {} {} · {}: {}\n",
            if en { "What it is" } else { "Ne" },
            alan[if en { "ad_en" } else { "ad" }].as_str().unwrap_or(""),
            damga(alan["guven"].as_str().unwrap_or(""), en),
            if en { "evidence" } else { "kanıt" },
            kanit_ozeti(alan, en, 6)
        ));
    }
    let tur = &a["tur"];
    if tur["id"].is_string() {
        s.push_str(&format!(
            "{}: {} {} · {}\n",
            if en { "Kind" } else { "Tür" },
            tur[if en { "ad_en" } else { "ad" }].as_str().unwrap_or(""),
            damga(tur["guven"].as_str().unwrap_or(""), en),
            kanit_ozeti(tur, en, 3)
        ));
    }
    for m in a["amac"].as_array().into_iter().flatten().take(2) {
        s.push_str(&format!(
            "{} ({} {}) {}: \"{}\"\n",
            if en { "Why" } else { "Neden" },
            m["konum"].as_str().unwrap_or(""),
            if en { "quote" } else { "alıntısı" },
            damga(m["guven"].as_str().unwrap_or(""), en),
            m["alinti"].as_str().unwrap_or("")
        ));
    }
    for p in a["problem"].as_array().into_iter().flatten().take(2) {
        match p["dayanak"].as_str() {
            Some("alinti") => s.push_str(&format!(
                "{} ({} {}) {}: \"{}\"\n",
                "Problem",
                p["konum"].as_str().unwrap_or(""),
                if en { "quote" } else { "alıntısı" },
                damga(p["guven"].as_str().unwrap_or(""), en),
                p["alinti"].as_str().unwrap_or("")
            )),
            _ => s.push_str(&format!(
                "{}: {} {} ({})\n",
                "Problem",
                p[if en { "metin_en" } else { "metin" }].as_str().unwrap_or(""),
                damga(p["guven"].as_str().unwrap_or(""), en),
                if en { "from the domain classification" } else { "alan sınıflandırmasından" }
            )),
        }
    }
    let aktorler: Vec<String> =
        a["aktorler"].as_array().into_iter().flatten().map(|x| format!("{} ({})", x[if en { "ad_en" } else { "ad" }].as_str().unwrap_or(""), x["islem_sayisi"])).collect();
    if !aktorler.is_empty() {
        s.push_str(&format!("{} [{}]: {}\n", if en { "Users" } else { "Kullanıcılar" }, if en { "Inferred" } else { "Yorum" }, aktorler.join(", ")));
    }
    let akislar: Vec<String> = a["akislar"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|x| {
            let ilk = x["islemler"].as_array().into_iter().flatten().filter_map(Value::as_str).find(|i| gorunur(i)).unwrap_or("");
            format!("{} ({ilk})", x[if en { "ad_en" } else { "ad" }].as_str().unwrap_or(""))
        })
        .collect();
    if !akislar.is_empty() {
        s.push_str(&format!("{}: {}\n", if en { "Sign-up / login flows" } else { "Kayıt / giriş akışları" }, akislar.join(", ")));
    }
    let gruplar = kaynak_gruplari(&a, en);
    if !gruplar.is_empty() {
        let n = a["yetenekler"].as_array().map(Vec::len).unwrap_or(0);
        s.push_str(&format!(
            "{} [{}]: {n} {}, {} {}:\n",
            if en { "Business capabilities" } else { "İş yetenekleri" },
            if en { "Inferred" } else { "Yorum" },
            if en { "capabilities" } else { "yetenek" },
            gruplar.len(),
            if en { "resources" } else { "kaynakta" }
        ));
        for (k, ads, _) in gruplar.iter().take(10) {
            s.push_str(&format!("  {k}: {}\n", ads.iter().take(8).cloned().collect::<Vec<_>>().join(", ")));
        }
    }
    s.push_str(if en {
        "Natural-language task (e.g. \"delete the user and hand their albums to someone else\"): `search` with task=true returns the steps, permissions and approvals.\n"
    } else {
        "Doğal dil görevi (ör. \"kullanıcıyı sil ve albümlerini başkasına devret\"): `ara` gorev=true ile adımları, izinleri ve onayları döner.\n"
    });
    s
}

/// Benlik Kartı: `app.x-meaning` (şema `app` altında `x-` üyelerine izin verir) ve belgeden alınan açıklama
/// `app.description` (alıntı, inferred). Bütün damgalar en fazla inferred
pub fn kart_ekle(kart: &mut Value, depo: &Depo) {
    let a = ozet(depo);
    if a.is_null() {
        return;
    }
    let damga = |g: &Value| if g == "inferred" { "inferred" } else { "unknown" };
    let kaynaklar = |v: &Value| -> Vec<Value> {
        v["kanitlar"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|k| {
                let (f, l) = k["konum"].as_str()?.rsplit_once(':')?;
                Some(json!({ "file": f, "line": l.parse::<u64>().unwrap_or(0), "kind": kanit_turu_en(k["kaynak"].as_str().unwrap_or("")), "term": k["terim"] }))
            })
            .collect()
    };
    let alan = &a["alan"];
    let domain = if alan["id"].is_string() {
        json!({ "id": alan["id"], "label": alan["ad_en"], "stamp": damga(&alan["guven"]), "evidence": kaynaklar(alan) })
    } else {
        json!({ "stamp": "unknown", "reason": "not enough evidence for one domain: at least two kinds of evidence and a clear lead over the next candidate are needed" })
    };
    let tur = &a["tur"];
    let kind = if tur["id"].is_string() { json!({ "id": tur["id"], "label": tur["ad_en"], "stamp": damga(&tur["guven"]), "evidence": kaynaklar(tur) }) } else { Value::Null };
    let konum = |v: &Value| -> Value {
        match v["konum"].as_str().and_then(|k| k.rsplit_once(':')) {
            Some((f, l)) => json!({ "file": f, "line": l.parse::<u64>().unwrap_or(0) }),
            None => Value::Null,
        }
    };
    let purpose: Vec<Value> =
        a["amac"].as_array().into_iter().flatten().map(|m| json!({ "quote": m["alinti"], "source": konum(m), "stamp": damga(&m["guven"]), "basis": "self-description" })).collect();
    let problem: Vec<Value> = a["problem"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| match p["dayanak"].as_str() {
            Some("alinti") => json!({ "quote": p["alinti"], "source": konum(p), "stamp": damga(&p["guven"]), "basis": "self-description" }),
            _ => json!({ "text": p["metin_en"], "stamp": damga(&p["guven"]), "basis": p["dayanak"] }),
        })
        .collect();
    let actors: Vec<Value> = a["aktorler"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|x| json!({ "id": x["id"], "name": x["ad_en"], "operations": x["islem_sayisi"], "examples": x["ornekler"], "stamp": damga(&x["guven"]) }))
        .collect();
    let flows: Vec<Value> =
        a["akislar"].as_array().into_iter().flatten().map(|x| json!({ "kind": x["tur"], "name": x["ad_en"], "operations": x["islemler"], "stamp": damga(&x["guven"]) })).collect();
    // Kartın işlem kimlikleri kapı kimliğinden değil adlardan üretilir: yetenekler kapı kimliğiyle ("POST /x") listelenir
    let caps: Vec<Value> = a["yetenekler"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|y| json!({ "id": y["id"], "name": y["ad_en"], "resource": y["kaynak"], "action": y["eylem"], "endpoints": y["islemler"], "stamp": damga(&y["guven"]) }))
        .collect();
    kart["app"]["x-meaning"] = json!({
        "stamp_ceiling": "inferred",
        "domain": domain, "kind": kind, "purpose": purpose, "problem": problem,
        "actors": actors, "auth_flows": flows, "capabilities": caps,
    });
    if let Some(m) = a["amac"].as_array().and_then(|x| x.first()) {
        let mut d = json!({ "text": m["alinti"], "stamp": "inferred" });
        if let Some((f, l)) = m["konum"].as_str().and_then(|k| k.rsplit_once(':')) {
            d["sources"] = json!([{ "file": f, "line_start": l.parse::<u64>().unwrap_or(1).max(1) }]);
        }
        kart["app"]["description"] = d;
    }
}

/// `rapor` için anlam satırları (rapor JSON'undaki `anlam` alanından), seçilen dilde
pub fn rapor_satirlari(a: &Value) -> Vec<String> {
    let mut v = vec![];
    if a.is_null() {
        return v;
    }
    // İngilizce çıktıda adlar haritadaki İngilizce karşılıklarından (`ad_en`, `metin_en`)
    let en = crate::dil::en();
    let ad = if en { "ad_en" } else { "ad" };
    let alan = &a["alan"];
    v.push(if alan["id"].is_string() {
        tf!("[yorum] alan: {} · kanıt: {}", "[inferred] domain: {} · evidence: {}", alan[ad].as_str().unwrap_or(""), kanit_ozeti(alan, en, 5))
    } else if en {
        "[unknown] domain: not enough evidence for one business domain".to_string()
    } else {
        format!("[bilinmiyor] alan: {}", alan["not"].as_str().unwrap_or("kanıt yok"))
    });
    if a["tur"]["id"].is_string() {
        v.push(tf!("[yorum] tür: {}", "[inferred] kind: {}", a["tur"][ad].as_str().unwrap_or("")));
    }
    match a["amac"].as_array().and_then(|x| x.first()) {
        Some(m) => v.push(tf!(
            "[yorum] amaç ({} alıntısı): \"{}\"",
            "[inferred] purpose (quote from {}): \"{}\"",
            m["konum"].as_str().unwrap_or(""),
            kisa(m["alinti"].as_str().unwrap_or(""), 140)
        )),
        None => v.push(t!("[bilinmiyor] amaç: README ya da paket tanımında anlatım yok", "[unknown] purpose: no description in the README or package definition").into()),
    }
    for p in a["problem"].as_array().into_iter().flatten().take(2) {
        v.push(match p["dayanak"].as_str() {
            Some("alinti") => tf!(
                "[yorum] problem ({} alıntısı): \"{}\"",
                "[inferred] problem (quote from {}): \"{}\"",
                p["konum"].as_str().unwrap_or(""),
                kisa(p["alinti"].as_str().unwrap_or(""), 140)
            ),
            _ => tf!("[yorum] problem: {}", "[inferred] problem: {}", p[if en { "metin_en" } else { "metin" }].as_str().unwrap_or("")),
        });
    }
    let aktor: Vec<String> = a["aktorler"].as_array().into_iter().flatten().map(|x| format!("{} ({})", x[ad].as_str().unwrap_or(""), x["islem_sayisi"])).collect();
    if !aktor.is_empty() {
        v.push(tf!("[yorum] kullanıcılar: {}", "[inferred] users: {}", aktor.join(", ")));
    }
    let akis: Vec<&str> = a["akislar"].as_array().into_iter().flatten().filter_map(|x| x[ad].as_str()).collect();
    if !akis.is_empty() {
        v.push(tf!("[yorum] akışlar: {}", "[inferred] flows: {}", akis.join(", ")));
    }
    let g = kaynak_gruplari(a, en);
    if !g.is_empty() {
        v.push(tf!(
            "[yorum] iş yetenekleri: {} yetenek, {} kaynak · {}",
            "[inferred] business capabilities: {} {y}, {} {k} · {}",
            a["yetenekler"].as_array().map(Vec::len).unwrap_or(0),
            g.len(),
            g.iter().take(5).map(|(k, _, n)| format!("{k} ({n})")).collect::<Vec<_>>().join(", ");
            y = crate::dil::cogul(a["yetenekler"].as_array().map(Vec::len).unwrap_or(0), "capability", "capabilities"),
            k = crate::dil::cogul(g.len(), "resource", "resources")
        ));
    }
    v
}

// ---- Doğal dil görevi → plan ----

/// Görevin cümlelere bölünmesi: "ve", "sonra", "ardından", virgül, "and", "then" (cümle özgün yazımıyla kalır)
fn cumleler(metin: &str) -> Vec<String> {
    const AYRAC: &[&str] = &["ve", "sonra", "ardindan", "ayrica", "and", "then"];
    let mut v: Vec<String> = vec![];
    let mut su: Vec<&str> = vec![];
    for w in metin.split_whitespace() {
        let bitti = w.ends_with([',', ';', '.']);
        let temiz = w.trim_end_matches([',', ';', '.']);
        if AYRAC.contains(&sade(temiz).as_str()) {
            if !su.is_empty() {
                v.push(su.join(" "));
                su.clear();
            }
            continue;
        }
        if !temiz.is_empty() {
            su.push(temiz);
        }
        if bitti && !su.is_empty() {
            v.push(su.join(" "));
            su.clear();
        }
    }
    if !su.is_empty() {
        v.push(su.join(" "));
    }
    v
}

/// Bir varlığın cümledeki rolü: nesne (fiilin üzerinde çalıştığı kayıt), hedef (yönelme hâli ya da "to": eklenecek
/// yer, "albüme", "bütçeye") ya da ek (birliktelik, kaynak, tamlayan: "with a user", "of an article", "kullanıcıyla",
/// "çöp kutusundaki")
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rol {
    Nesne,
    Hedef,
    Ek,
}

/// Cümledeki bir varlık: sözlük kavramı (yoksa kodda aranacak ham kelime), yazıldığı biçim, iyelik eki, rolü ve sırası
#[derive(Clone)]
struct Varlik {
    kavram: Option<&'static Kavram>,
    kelime: String,
    iyelik: bool,
    rol: Rol,
    sira: usize,
}

impl Varlik {
    /// Aramada kullanılan kelime: kavramın kimliği ya da ham kelime
    fn arama(&self) -> String {
        self.kavram.map(|k| k.id.to_string()).unwrap_or_else(|| self.kelime.clone())
    }
    fn ad(&self, en: bool) -> String {
        match self.kavram {
            Some(k) => (if en { k.ad_en } else { k.ad_tr }).to_string(),
            None => self.kelime.clone(),
        }
    }
}

/// Bir cümlenin fiili (ve cümledeki sırası) ile varlıkları
struct Cumle {
    metin: String,
    eylem: Option<&'static Kavram>,
    eylem_sira: usize,
    varliklar: Vec<Varlik>,
    /// Uygulamada karşılığı bulunmadığı için atılan kelimeler (cümle bir öncekinin varlığını almaz)
    atilan: Vec<String>,
}

/// Sonraki varlığı hedef yapan İngilizce edatlar ve ek yapanlar (birliktelik, kaynak, tamlayan)
const EDAT_HEDEF: &[&str] = &["to", "into", "onto"];
const EDAT_EK: &[&str] = &["with", "for", "from", "of", "in", "on", "by", "about", "at"];
/// Başka bir kaydı gösteren zamirler: varlık bir önceki cümleden gelir
const ZAMIRLER: &[&str] = &["it", "them", "one", "ones", "birini", "onu", "onlari", "bunu", "bunlari", "sunu", "hepsini"];

/// Türkçe hâl ekinden rol: birliktelik (-la / -le) ve bulunma / ayrılma / tamlama (-da, -daki, -dan) ektir;
/// yönelme (-a / -e, -ya / -ye) hedeftir; belirtme ve iyelik (-ı, -ini) nesnedir
fn tr_ek_rolu(ek: &str) -> Option<Rol> {
    if ["la", "le"].iter().any(|x| ek.ends_with(x)) {
        return Some(Rol::Ek);
    }
    if ["daki", "deki", "taki", "teki", "de", "da", "te", "ta", "den", "dan", "ten", "tan"].iter().any(|x| ek.ends_with(x)) {
        return Some(Rol::Ek);
    }
    if ek.ends_with('e') || ek.ends_with('a') {
        return Some(Rol::Hedef);
    }
    None
}

fn cumle_coz(metin: &str) -> Cumle {
    let mut eylem = None;
    let mut eylem_sira = 0;
    let mut varliklar = vec![];
    let mut edat: Option<Rol> = None;
    let mut sira = 0;
    for parca in metin.split_whitespace() {
        // Kesme işaretiyle ek almış ad ("check'leri", "Slack'e") isimdir; ek ayrılır
        let (govde, kesme_eki) = match parca.split_once(['\'', '’']) {
            Some((g, e)) => (g, Some(sade(e).trim_matches(|c: char| !c.is_alphanumeric()).to_string())),
            None => (parca, None),
        };
        for w in yazi_kelimeleri(govde) {
            sira += 1;
            if EDAT_HEDEF.contains(&w.as_str()) {
                edat = Some(Rol::Hedef);
                continue;
            }
            if EDAT_EK.contains(&w.as_str()) {
                edat = Some(Rol::Ek);
                continue;
            }
            if DOLGU.contains(&w.as_str()) || ZAMIRLER.contains(&w.as_str()) {
                continue;
            }
            let (ks, ek) = kavramlar(&w);
            if kesme_eki.is_none() {
                if let Some(e) = ks.iter().find(|k| k.tur == Tur::Eylem) {
                    if eylem.is_none() {
                        eylem = Some(*e);
                        eylem_sira = sira;
                    }
                    edat = None;
                    continue;
                }
            }
            let kavram = ks.iter().find(|k| k.tur == Tur::Isim).copied();
            // Sözlükte olmayan kelime ham varlıktır (kodda aranır); çok kısa ya da sayı olan atlanır
            if kavram.is_none() && (w.chars().count() < 3 || w.chars().all(|c| c.is_ascii_digit())) {
                continue;
            }
            let ek = kesme_eki.clone().or(ek);
            let iyelik = ek.as_deref().is_some_and(|e| ["ini", "lerini", "larini", "si", "leri", "lari", "sini", "unu"].iter().any(|x| e.ends_with(x)));
            let rol = edat.take().or_else(|| ek.as_deref().and_then(tr_ek_rolu)).unwrap_or(Rol::Nesne);
            varliklar.push(Varlik { kavram, kelime: w.clone(), iyelik, rol, sira });
        }
    }
    Cumle { metin: metin.to_string(), eylem, eylem_sira, varliklar, atilan: vec![] }
}

impl Cumle {
    /// Asıl nesne: uygulamada karşılığı olan (`var_mi`) nesnelerden fiil sondaysa (Türkçe) fiilden hemen önceki,
    /// değilse (İngilizce) fiilden sonraki ilk; diğer nesneler ek sayılır. Karşılığı olmayan ham kelimeler atılır
    fn nesne_sec(&mut self, var_mi: &dyn Fn(&Varlik) -> bool) {
        let (kalan, atilan): (Vec<Varlik>, Vec<Varlik>) = std::mem::take(&mut self.varliklar).into_iter().partition(|v| v.kavram.is_some() || var_mi(v));
        self.varliklar = kalan;
        self.atilan = atilan.into_iter().map(|v| v.kelime).collect();
        let nesneler: Vec<usize> = self.varliklar.iter().enumerate().filter(|(_, v)| v.rol == Rol::Nesne).map(|(i, _)| i).collect();
        if nesneler.len() < 2 {
            return;
        }
        let fiil_sonda = self.eylem.is_some() && self.varliklar.iter().all(|v| v.sira < self.eylem_sira);
        // Sözlükteki ad ham kelimeden önce gelir ("send a test email": e-posta)
        let sozlukte = |i: &&usize| self.varliklar[**i].kavram.is_some();
        let secilen = if fiil_sonda {
            nesneler.iter().rev().find(sozlukte).or(nesneler.last()).copied()
        } else {
            let sonra: Vec<&usize> = nesneler.iter().filter(|i| self.varliklar[**i].sira > self.eylem_sira).collect();
            sonra.iter().find(|i| sozlukte(i)).or(sonra.first()).copied().or_else(|| nesneler.iter().find(sozlukte)).or(nesneler.first()).copied()
        };
        for i in nesneler {
            if Some(i) != secilen {
                self.varliklar[i].rol = Rol::Ek;
            }
        }
    }
    fn nesne(&self) -> Option<&Varlik> {
        self.varliklar.iter().find(|v| v.rol == Rol::Nesne)
    }
}

/// Görev ayrıştırılınca birden fazla eylemli adım çıkıyor mu (arama yerine plan önerilir)
pub fn cok_adimli(metin: &str) -> bool {
    cumleler(metin).iter().filter(|c| cumle_coz(c).eylem.is_some()).count() >= 2
}

fn risk_sirasi(k: &Value) -> u8 {
    match crate::canli::islem_bilgisi(k).1 {
        "oku" => 0,
        "yikici" => 2,
        _ => 1,
    }
}

/// Adım için en iyi işlem: eylemin yöntemine uyan, varlıkla eşleşen; puanı yeterli değilse yok
fn en_iyi<'a>(kapilar: &'a [Value], s: &Sorgu, alanlar: &dyn Fn(&Value) -> Vec<String>, yontemler: &[&str]) -> Option<(&'a Value, f64)> {
    let mut v: Vec<(&Value, f64)> = kapilar
        .iter()
        .filter(|k| yontemler.is_empty() || yontemler.contains(&k["yontem"].as_str().unwrap_or("").to_ascii_uppercase().as_str()))
        .map(|k| (k, puan(k, s, &alanlar(k))))
        .filter(|(_, p)| *p >= 3.0)
        .collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    v.into_iter().next()
}

/// İsteğin fiili işlemin eylemiyle ne kadar uyuşuyor
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Uyum {
    Yok,
    Yakin,
    Tam,
}

fn okuma_eylemi(e: &str) -> bool {
    matches!(e, "view" | "list" | "search" | "download")
}

/// Fiil-işlem uyumu. Tam: işlemin yolunda ya da fonksiyon adında fiilin kendisi (ya da eş anlamlısı) geçiyor, işlemin
/// eylemi aynı ya da aynı ailede (oluştur ~ yükle; gönder ~ mesaj / e-posta oluşturma). Okuma fiili yalnızca okuma
/// işlemine (GET ya da arama) eşlenir; yazma fiili okuma işlemine hiç eşlenmez. Yakın: kapatma / iptal / arşivleme
/// için silme ya da güncelleme, iç içe kaynağa ekleme için güncelleme (adım "en yakın" diye yazılır)
fn uyum(e: &Kavram, k: &Value) -> Uyum {
    let y = yetenek(k);
    let yontem = k["yontem"].as_str().unwrap_or("").to_ascii_uppercase();
    let mut ws = kelimeler(k["yol"].as_str().unwrap_or(""));
    ws.extend(kelimeler(k["fonksiyon"].as_str().unwrap_or("")));
    let fiil_var = e.en.iter().chain(e.tr.iter()).any(|f| ws.iter().any(|w| w == f));
    if okuma_eylemi(e.id) {
        let okur = yontem == "GET" || y.eylem == "search";
        return if okur && (okuma_eylemi(&y.eylem) || fiil_var) { Uyum::Tam } else { Uyum::Yok };
    }
    if okuma_eylemi(&y.eylem) || (yontem == "GET" && !matches!(e.id, "login" | "logout" | "play")) {
        return Uyum::Yok;
    }
    if fiil_var || y.eylem == e.id {
        return Uyum::Tam;
    }
    let mesaj = kelimeler(&y.anahtar).iter().any(|w| matches!(w.as_str(), "message" | "mail" | "email" | "notification" | "comment" | "sms" | "invite" | "invitation" | "reply"));
    match (e.id, y.eylem.as_str()) {
        ("create" | "upload", "create" | "upload" | "register") => Uyum::Tam,
        ("send", "create") if mesaj => Uyum::Tam,
        ("create", "update") if y.anahtar.contains('-') => Uyum::Yakin,
        ("deactivate" | "cancel" | "archive", "delete" | "update") => Uyum::Yakin,
        _ => Uyum::Yok,
    }
}

/// Adımın işlemi: fiilin yöntemlerinden (okuma fiilinde GET ya da arama), nesne ve hedef varlıkla eşleşen, fiille
/// uyuşan en iyi işlem; önce tam uyum, sonra puan
fn adim_islemi<'a>(kapilar: &'a [Value], e: &Kavram, varliklar: &[String], alanlar: &dyn Fn(&Value) -> Vec<String>) -> Option<(&'a Value, Uyum, f64)> {
    let s = sorgu(&format!("{} {}", varliklar.join(" "), e.en[0]));
    // Yolun son kaynağı asıl varlıksa öne çıkar (GET /budgets/:id, GET /budgets/:id/categories/:id'den önce)
    let son_kaynak = |k: &Value| -> bool {
        let son = k["yol"].as_str().unwrap_or("").split('/').rev().find(|p| !p.is_empty() && !p.starts_with([':', '{', '*', '<', '['])).unwrap_or("");
        let son = kelimeler(son).iter().map(|w| tekil(w)).collect::<Vec<_>>().join("");
        varliklar.last().is_some_and(|v| {
            let vk = kavram(v);
            son == *v || vk.is_some_and(|x| x.en.contains(&son.as_str()))
        })
    };
    let mut v: Vec<(&Value, f64, Uyum)> = kapilar
        .iter()
        .filter(|k| {
            let y = k["yontem"].as_str().unwrap_or("").to_ascii_uppercase();
            okuma_eylemi(e.id) || e.yontem.contains(&y.as_str())
        })
        .map(|k| (k, puan(k, &s, &alanlar(k)), uyum(e, k)))
        .filter(|(_, p, u)| *p >= 3.0 && *u != Uyum::Yok)
        .map(|(k, p, u)| (k, if son_kaynak(k) { p + 1.0 } else { p }, u))
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)));
    v.into_iter().next().map(|(k, p, u)| (k, u, p))
}

/// Doğal dil görevini işlem adımlarına çevirir: her cümlede fiil ve nesne ayrı bulunur (fiil-nesne eşlemesi); nesne
/// kaynağa, fiil işlemin eylemine eşlenir; okuma fiili yazma işlemine eşlenmez. Her adım için işlem (yöntem + yol),
/// giriş, rol / izin ve onay gereği. Zamir ("it", "birini") ya da nesnesiz cümle bir önceki cümlenin varlıklarından
/// fiile uyanı alır. Sıra kuralı: okuma, yazma, silme (silinen kaydın bağlı verisi önce taşınır). Plan yürütülmez;
/// karşılığı bulunamayan adım "bulunamadı" diye yazılır. `alanlar`: kapının girdi alan adları
pub fn plan(metin: &str, kapilar: &[Value], alanlar: &dyn Fn(&Value) -> Vec<String>, en: bool) -> (String, bool) {
    // Ham kelime uygulamada karşılığı olan bir varlık mı (bir işlemin yolunda ya da adında geçiyor)
    let var_mi = |v: &Varlik| -> bool {
        let s = sorgu(&v.kelime);
        kapilar.iter().any(|k| puan(k, &s, &alanlar(k)) >= 3.0)
    };
    let mut cs: Vec<Cumle> = cumleler(metin).iter().map(|c| cumle_coz(c)).collect();
    for c in cs.iter_mut() {
        c.nesne_sec(&var_mi);
    }
    // Eylemsiz cümle bir sonrakinin eylemini alır ("kullanıcıyı ve albümlerini sil")
    for i in (0..cs.len()).rev() {
        if cs[i].eylem.is_none() {
            cs[i].eylem = cs.get(i + 1).and_then(|c| c.eylem);
        }
    }
    cs.retain(|c| c.eylem.is_some() || !c.varliklar.is_empty());
    if cs.is_empty() {
        return (
            if en {
                format!("No action or entity recognised in '{metin}'. Use verbs such as create, delete, share, transfer and nouns from the application (see `app`).")
            } else {
                format!("'{metin}' içinde tanınan bir eylem ya da varlık yok. Oluştur, sil, paylaş, devret gibi fiiller ve uygulamanın varlık adlarıyla yazın (`kimsin`).")
            },
            true,
        );
    }
    struct Adim<'a> {
        ad: String,
        cumle: String,
        kapilar: Vec<(&'a Value, String)>,
        notlar: Vec<String>,
        sira: u8,
    }
    let mut adimlar: Vec<Adim> = vec![];
    let mut onceki: Vec<Varlik> = vec![];
    for c in &cs {
        let Some(e) = c.eylem else {
            adimlar.push(Adim {
                ad: format!("{} ?", c.varliklar.first().map(|v| v.ad(en)).unwrap_or_else(|| "?".into())),
                cumle: c.metin.clone(),
                kapilar: vec![],
                notlar: vec![if en { "no verb recognised in this step".into() } else { "bu adımda fiil tanınmadı".into() }],
                sira: 1,
            });
            continue;
        };
        // Nesne: cümlenin kendi nesnesi; yoksa bir önceki cümlenin varlıklarından (önce nesnesi) bu fiille işlemi olan
        let hedefler: Vec<String> = c.varliklar.iter().filter(|v| v.rol == Rol::Hedef).map(Varlik::arama).collect();
        let mut nesne: Option<Varlik> = c.nesne().cloned();
        // Zamir ya da nesnesiz cümle ("favorite it", "birini duraklat") öncekinden alır; tanınmayan bir ad yazılmışsa almaz
        if nesne.is_none() && c.atilan.is_empty() {
            let mut adaylar: Vec<&Varlik> = onceki.iter().filter(|v| v.rol == Rol::Nesne).collect();
            adaylar.extend(onceki.iter().filter(|v| v.rol != Rol::Nesne));
            // Fiille en iyi uyuşan aday (tam uyum, sonra puan); hiçbiri uymuyorsa öncekinin nesnesi
            let degerlen = |v: &Varlik| -> Option<(Uyum, f64)> {
                let mut ad = hedefler.clone();
                ad.push(v.arama());
                adim_islemi(kapilar, e, &ad, alanlar).map(|(_, u, p)| (u, p))
            };
            nesne = adaylar
                .iter()
                .filter_map(|v| degerlen(v).map(|d| (*v, d)))
                .max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(a.1 .1.partial_cmp(&b.1 .1).unwrap_or(std::cmp::Ordering::Equal)))
                .map(|(v, _)| v)
                .or(adaylar.first().copied())
                .map(|v| Varlik { rol: Rol::Nesne, iyelik: false, ..v.clone() });
        }
        // Nesne yoksa yönelme hedefi asıl varlıktır ("bütçeye ekle")
        let asil = nesne.clone().or_else(|| c.varliklar.iter().find(|v| v.rol == Rol::Hedef).cloned());
        let fiil = if en { e.ad_en } else { e.ad_tr };
        let hedef_varlik = c.varliklar.iter().find(|v| v.rol == Rol::Hedef).filter(|h| nesne.as_ref().is_some_and(|n| n.arama() != h.arama()));
        let ad = match (&asil, hedef_varlik) {
            // "albüme ekle" (önceki cümlenin fotoğraflarıyla): "albüm: fotoğraf ekle" / "add photo to album"
            (Some(v), Some(h)) => {
                let (tr, en_ad) = if e.id == "create" { ("ekle", "add") } else { (e.ad_tr, e.ad_en) };
                if en {
                    format!("{en_ad} {} to {}", v.ad(true), h.ad(true))
                } else {
                    format!("{}: {} {tr}", h.ad(false), v.ad(false))
                }
            }
            _ => match &asil {
                Some(v) if en => format!("{fiil} {}", v.ad(true)),
                Some(v) => format!("{} {fiil}", v.ad(false)),
                None if en => format!("{fiil} ?"),
                None => format!("? {fiil}"),
            },
        };
        let mut a = Adim { ad, cumle: c.metin.clone(), kapilar: vec![], notlar: vec![], sira: 1 };
        let mut aranan: Vec<String> = hedefler.clone();
        if let Some(n) = &nesne {
            aranan.push(n.arama());
        }
        aranan.dedup();
        if asil.is_none() && !matches!(e.id, "login" | "logout" | "register" | "reset") {
            let atilan = if c.atilan.is_empty() { String::new() } else { format!(" ({})", c.atilan.join(", ")) };
            a.notlar.push(if en {
                format!("no entity of this application recognised in this step{atilan}")
            } else {
                format!("bu adımda uygulamanın bir varlığı tanınmadı{atilan}")
            });
        }
        let varlik_var = asil.as_ref().is_none_or(|v| kapilar.iter().any(|k| puan(k, &sorgu(&v.arama()), &alanlar(k)) >= 3.0));
        let isim_metni: Vec<String> = asil.iter().map(Varlik::arama).collect();
        if !varlik_var {
            let ad = asil.as_ref().map(|v| v.ad(en)).unwrap_or_default();
            a.notlar.push(if en { format!("no operation on '{ad}' in this application") } else { format!("bu uygulamada '{ad}' ile ilgili işlem bulunamadı") });
        } else if asil.is_none() && !matches!(e.id, "login" | "logout" | "register" | "reset") {
            // Varlıksız adımda yalnızca fiille işlem seçilmez (giriş, çıkış, kayıt, parola sıfırlama dışında)
        } else if e.id == "transfer" {
            // Devretmek: önce devredilecek kayıtları listele, sonra sahibini değiştiren işlem (gövdesinde owner / sahip)
            let s = sorgu(&format!("{} {}", isim_metni.join(" "), e.en[0]));
            let liste = sorgu(&format!("{} list", isim_metni.join(" ")));
            if let Some((k, _)) = en_iyi(kapilar, &liste, alanlar, OKU) {
                a.kapilar.push((k, (if en { "list the records to hand over" } else { "devredilecek kayıtları listele" }).into()));
            }
            let sahip = kapilar.iter().filter(|k| YAZ.contains(&k["yontem"].as_str().unwrap_or(""))).filter(|k| puan(k, &s, &alanlar(k)) >= 3.0).find(|k| {
                alanlar(k).iter().any(|f| {
                    let f = sade(f);
                    f.contains("owner") || f.contains("sahip")
                })
            });
            match sahip {
                Some(k) => a.kapilar.push((k, (if en { "change the owner (body field owner)" } else { "sahibini değiştir (gövdede owner alanı)" }).into())),
                None => {
                    a.notlar.push(if en {
                        "no operation found that changes the owner: no write on this entity takes an owner field".into()
                    } else {
                        "sahipliği değiştiren işlem bulunamadı: bu varlığın yazma işlemlerinin hiçbirinde sahip (owner) alanı yok".into()
                    });
                    let paylas = sorgu(&format!("{} share", isim_metni.join(" ")));
                    if let Some((k, _)) = en_iyi(kapilar, &paylas, alanlar, YAZ).filter(|(k, _)| yetenek(k).eylem == "share") {
                        a.kapilar.push((k, (if en { "closest: share with the other user" } else { "en yakın: diğer kullanıcıyla paylaş" }).into()));
                    }
                }
            }
            if sade(metin).contains("baska") || sade(metin).contains("someone") || sade(metin).contains("another") {
                a.notlar.push(if en { "the receiving user must be named (its id)".into() } else { "devralacak kullanıcı belirtilmeli (kimliği)".into() });
            }
        } else if let Some((k, u, _)) = adim_islemi(kapilar, e, &aranan, alanlar).or_else(|| adim_islemi(kapilar, e, &isim_metni, alanlar)) {
            let not = if u == Uyum::Yakin {
                let ye = yetenek(k);
                let karsilik = kavram(&ye.eylem).map(|x| if en { x.ad_en } else { x.ad_tr }).unwrap_or(ye.eylem.as_str()).to_string();
                if en {
                    format!("closest: no operation that does '{fiil}' exactly; this one does '{karsilik}'")
                } else {
                    format!("en yakın: '{fiil}' işini birebir yapan işlem yok; bu işlem '{karsilik}' yapar")
                }
            } else {
                String::new()
            };
            a.kapilar.push((k, not));
        } else {
            let ad = asil.as_ref().map(|v| v.ad(en)).unwrap_or_default();
            a.notlar.push(if en {
                format!("no matching operation found: nothing in this application does '{fiil}' on '{ad}'")
            } else {
                format!("uyan işlem bulunamadı: bu uygulamada '{ad}' için '{fiil}' işini yapan bir işlem yok")
            });
        }
        a.sira = a.kapilar.iter().map(|(k, _)| risk_sirasi(k)).max().unwrap_or(1);
        if c.varliklar.iter().any(|v| v.iyelik && v.rol == Rol::Nesne) {
            a.sira = a.sira.min(1);
        }
        onceki = c.varliklar.clone();
        if let Some(n) = nesne {
            if !onceki.iter().any(|v| v.rol == Rol::Nesne) {
                onceki.insert(0, n);
            }
        }
        adimlar.push(a);
    }
    let yeniden = adimlar.windows(2).any(|w| w[0].sira > w[1].sira);
    adimlar.sort_by_key(|a| a.sira);
    let bulunan = adimlar.iter().filter(|a| !a.kapilar.is_empty()).count();
    let mut s = if en {
        format!("TASK PLAN [Inferred] · \"{}\" · {} step(s) · NOT EXECUTED\n", metin.trim(), adimlar.len())
    } else {
        format!("GÖREV PLANI [Yorum] · \"{}\" · {} adım · YÜRÜTÜLMEZ\n", metin.trim(), adimlar.len())
    };
    if yeniden {
        s.push_str(if en {
            "Order changed: reads first, then writes, deletes last (records tied to what is deleted are handed over first).\n"
        } else {
            "Sıra değişti: önce okuma, sonra yazma, silme en sonda (silinen kayda bağlı veriler önce devredilir).\n"
        });
    }
    let mut izinler: BTreeSet<String> = BTreeSet::new();
    let mut onayli: Vec<usize> = vec![];
    for (i, a) in adimlar.iter().enumerate() {
        s.push_str(&format!("{}. {}  ← \"{}\"\n", i + 1, a.ad, a.cumle));
        for (k, not) in &a.kapilar {
            let roller: Vec<&str> = k["roller"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            for r in &roller {
                izinler.insert(r.to_string());
            }
            let giris = match k["giris"].as_str().unwrap_or("bilinmiyor") {
                "gerekli" => {
                    if en {
                        "login required"
                    } else {
                        "giriş gerekli"
                    }
                }
                "herkese_acik" => {
                    if en {
                        "public"
                    } else {
                        "herkese açık"
                    }
                }
                "istege_bagli" => {
                    if en {
                        "login optional"
                    } else {
                        "giriş isteğe bağlı"
                    }
                }
                _ => {
                    if en {
                        "login unknown"
                    } else {
                        "giriş bilinmiyor"
                    }
                }
            };
            let onay = match crate::canli::statik_onay(k) {
                Some(n) => {
                    if !onayli.contains(&(i + 1)) {
                        onayli.push(i + 1);
                    }
                    if en {
                        format!("human approval needed ({n})")
                    } else {
                        format!("insan onayı gerekir ({n})")
                    }
                }
                None if risk_sirasi(k) == 0 => (if en { "no approval (read)" } else { "onay gerekmez (okuma)" }).to_string(),
                None => (if en {
                    "may go without approval if the run-time preview, token and role gates pass"
                } else {
                    "çalışma anındaki önizleme, token ve rol kapıları geçerse onaysız gidebilir"
                })
                .to_string(),
            };
            s.push_str(&format!(
                "   {} {} · {} · {}{} · {}{}\n",
                k["yontem"].as_str().unwrap_or(""),
                k["yol"].as_str().unwrap_or(""),
                crate::canli::islem_bilgisi(k).0,
                giris,
                if roller.is_empty() { String::new() } else { format!(" · {}: {}", if en { "roles/permissions" } else { "rol/izin" }, roller.join(", ")) },
                onay,
                if not.is_empty() { String::new() } else { format!(" · {not}") }
            ));
        }
        for n in &a.notlar {
            s.push_str(&format!("   ? {n}\n"));
        }
    }
    if !izinler.is_empty() {
        s.push_str(&format!("{}: {}\n", if en { "Roles/permissions needed" } else { "Gereken rol/izinler" }, izinler.into_iter().collect::<Vec<_>>().join(", ")));
    }
    if !onayli.is_empty() {
        s.push_str(&format!(
            "{}: {}\n",
            if en { "Steps that wait for a human" } else { "İnsan onayı bekleyecek adımlar" },
            onayli.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
        ));
    }
    s.push_str(if en {
        "Steps are found by a dictionary (Turkish and English) and ordered by rule: an inference, not a certainty. Check each with `describe` and `preview`; run them one by one with `call`. Sezgi does not execute the plan.\n"
    } else {
        "Adımlar sözlükle bulunur ve kuralla sıralanır: kesinlik değil çıkarımdır. Her adımı `tani` ve `onizle` ile denetleyin, `cagir` ile tek tek çalıştırın. Sezgi planı kendisi yürütmez.\n"
    });
    (s, bulunan == 0)
}

/// Ayna "Ne biliyoruz" 17. soru için örnek: uygulamanın kendi yeteneklerinden kurulmuş bir istek (seçilen dilde) ve
/// bulduğu işlem
pub fn ornek_istek(kapilar: &[Value], alanlar: &dyn Fn(&Value) -> Vec<String>) -> Option<(String, String)> {
    // Sözlükte Türkçe adı olan bir varlığın silme işlemi, yoksa oluşturma işlemi
    for e in ["delete", "create"] {
        for k in kapilar {
            let y = yetenek(k);
            if y.eylem != e || y.kaynak.contains('-') {
                continue;
            }
            let Some(isim) = isim_kavrami(&y.kaynak) else { continue };
            let Some(ek) = kavram(e) else { continue };
            let istek = if crate::dil::en() { format!("{} {}", ek.ad_en, isim.ad_en) } else { format!("{} {}", isim.ad_tr, ek.ad_tr) };
            if let Some((b, _)) = en_iyi(kapilar, &sorgu(&istek), alanlar, ek.yontem) {
                return Some((istek, format!("{} {}", b["yontem"].as_str().unwrap_or(""), b["yol"].as_str().unwrap_or(""))));
            }
        }
    }
    None
}
