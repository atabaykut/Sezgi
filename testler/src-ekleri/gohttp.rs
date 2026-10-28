use super::*;

#[test]
fn yol_sablonlari() {
    // net/http Go 1.22, chi, gorilla: {ad}, {ad:desen}, {yol...}, {$}
    assert_eq!(yol_normal("/urunler/{id}"), "/urunler/:id");
    assert_eq!(yol_normal("/gorevler/{id:[0-9]+}"), "/gorevler/:id");
    assert_eq!(yol_normal("/dosyalar/{yol...}"), "/dosyalar/:yol");
    assert_eq!(yol_normal("/{$}"), "/");
    assert_eq!(yol_normal("/api/{$}"), "/api/");
    // Gin / Echo / Fiber: :ad, *ad, *, :ad?, :ad<int>
    assert_eq!(yol_normal("/statik/*dosya"), "/statik/:dosya");
    assert_eq!(yol_normal("/statik/*"), "/statik/:wildcard");
    assert_eq!(yol_normal("/notlar/:id<int>"), "/notlar/:id");
    assert_eq!(yol_normal("/a/:secimli?"), "/a/:secimli");
    assert_eq!(yol_normal("api//x"), "/api/x");
    assert_eq!(yol_tamsayilari("/a/{id:[0-9]+}/{b}/:c<int>"), ["id", "c"]);
}

#[test]
fn yol_birlestirme() {
    assert_eq!(yol_birlestir("/api", "/"), "/api/");
    assert_eq!(yol_birlestir("/api/", "/x"), "/api/x");
    assert_eq!(yol_birlestir("/api", ""), "/api");
    assert_eq!(yol_birlestir("", "/x"), "/x");
}

#[test]
fn ice_aktarma_adlari() {
    assert_eq!(surumsuz("github.com/labstack/echo/v4"), "github.com/labstack/echo");
    assert_eq!(surumsuz("gorm.io/gorm"), "gorm.io/gorm");
    assert_eq!(varsayilan_ad("github.com/go-chi/chi/v5"), "chi");
    assert_eq!(varsayilan_ad("gopkg.in/yaml.v3"), "yaml");
    assert_eq!(varsayilan_ad("github.com/redis/go-redis/v9"), "redis");
    assert_eq!(varsayilan_ad("github.com/labstack/echo-jwt/v4"), "echojwt");
    assert_eq!(varsayilan_ad("github.com/appleboy/gin-jwt/v2"), "jwt");
    assert_eq!(cati_yolu("github.com/gofiber/fiber/v2"), Some(Cati::Fiber));
    assert_eq!(cati_yolu("github.com/gorilla/handlers"), None);
}

#[test]
fn gorm_adlandirmasi() {
    assert_eq!(gorm_tablo_adi("Kullanici"), "kullanicis");
    assert_eq!(gorm_tablo_adi("UserProfile"), "user_profiles");
    assert_eq!(gorm_tablo_adi("Category"), "categories");
    assert_eq!(gorm_tablo_adi("Box"), "boxes");
    assert_eq!(gorm_tablo_adi("APIKey"), "api_keys");
    assert_eq!(gorm_sutun("CreatedAt"), "created_at");
    assert_eq!(gorm_sutun("UserID"), "user_id");
    assert_eq!(gorm_sutun("Status"), "status");
}

#[test]
fn dogrulayici_etiketleri() {
    let (k, z, kos, d) = dogrulama_kurallari("required,min=2,max=50", "string");
    assert_eq!((k, z, kos, d.is_empty()), (vec!["MinLength(2)".to_string(), "MaxLength(50)".into()], true, false, true));
    let (k, z, _, _) = dogrulama_kurallari("omitempty,gte=1,lte=10", "int");
    assert_eq!((k, z), (vec!["Min(1)".to_string(), "Max(10)".into()], false));
    let (k, _, _, d) = dogrulama_kurallari("oneof=uye 'tam yetki'", "string");
    assert_eq!(d, ["uye", "tam", "yetki"], "oneof boşlukla ayrılır (tırnaklı çok sözcüklü değer desteklenmez)");
    assert!(k[0].starts_with("IsIn(["));
    let (k, _, _, _) = dogrulama_kurallari("gt=0,lt=100", "int");
    assert_eq!(k, ["Min(1)", "Max(99)"]);
    let (k, _, _, _) = dogrulama_kurallari("min=1,max=3", "array");
    assert_eq!(k, ["ArrayMinSize(1)", "ArrayMaxSize(3)"]);
    let (_, z, kos, _) = dogrulama_kurallari("required_if=Tur kurumsal", "string");
    assert!(!z && kos, "koşullu zorunluluk kesin değildir");
    let (k, _, _, _) = dogrulama_kurallari("required,dive,email", "array");
    assert!(k.is_empty(), "dive'dan sonrası öğelere uygulanır");
    let (k, _, _, _) = dogrulama_kurallari("e164", "string");
    assert_eq!(k, ["e164"], "tanınmayan kural metniyle kalır: önizleme denetleyemez");
}

#[test]
fn struct_etiketi() {
    let e = etiketler(r#"json:"ad,omitempty" binding:"required" gorm:"column:ad;size:20""#);
    assert_eq!(e["json"], "ad,omitempty");
    assert_eq!(e["binding"], "required");
    assert_eq!(e["gorm"], "column:ad;size:20");
}

#[test]
fn ara_katman_adi_kisaltilir() {
    assert_eq!(kisa_ad("middleware.JWTAuth()"), "middleware.JWTAuth()");
    assert_eq!(kisa_ad(&format!("jwtware.New(jwtware.Config{{SigningKey: {}}})", "x".repeat(60))), "jwtware.New(…)");
}
