//! spring.rs birim testleri: yol birleştirme ve desen eşleme, SpEL kararı, yapılandırma okuma
use super::*;

#[test]
fn yol_ve_desen() {
    assert_eq!(duzgun(&yol_birlestir("/api/", "/x")), "/api/x");
    assert_eq!(duzgun(&yol_birlestir("api", "x")), "/api/x");
    assert_eq!(duzgun(&yol_birlestir("", "")), "/");
    assert_eq!(sezgi_yolu(r"/a/{id:\d+}/b/{*yol}"), "/a/:id/b/:yol");
    assert!(desen_eslesir("/api/**", "/api/a/:id"));
    assert!(desen_eslesir("/api/*/x", "/api/:id/x"));
    assert!(!desen_eslesir("/api/me", "/api/:id"));
    assert!(desen_eslesir("/api/u*", "/api/urun"));
    assert!(!desen_eslesir("/api", "/api/x"));
    assert_eq!(yilan("SiparisKalemi"), "siparis_kalemi");
    assert_eq!(yilan("URLKayit"), "url_kayit");
}

#[test]
fn spel() {
    assert_eq!(spel_karari("hasRole('ADMIN')"), Karar::Rol(vec!["ADMIN".into()]));
    assert_eq!(spel_karari("hasAnyRole('A', 'ROLE_B')"), Karar::Rol(vec!["A".into(), "B".into()]));
    assert_eq!(spel_karari("isAuthenticated()"), Karar::Giris);
    assert_eq!(spel_karari("@yetki.sahibi(#id)"), Karar::Bilinmeyen);
    assert_eq!(spel_karari("permitAll()"), Karar::Acik);
}

#[test]
fn ayarlar() {
    let d = vec![("application.yml".to_string(), "server:\n  port: 8080\n  servlet:\n    context-path: ${YOL:/magaza}\n---\nserver.servlet.context-path: /x\n".to_string())];
    assert_eq!(ayar(&d, "server.servlet.context-path").as_deref(), Some("/magaza"));
    let p = vec![("application.properties".to_string(), "# yorum\nserver.servlet.contextPath=/api\n".to_string())];
    assert_eq!(ayar(&p, "server.servlet.context-path").as_deref(), Some("/api"));
}
