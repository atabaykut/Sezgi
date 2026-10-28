use super::*;

fn ara(metin: &str) -> NextAra {
    NextAra { dosya: "middleware.ts".into(), yetki: true, kosullu: false, eslesme: NextAra::eslesmeler(metin) }
}

#[test]
fn matcher_bicimleri() {
    // matcher yoksa her yol
    assert_eq!(ara("export default withAuth()").kapsar("/api/a"), Some(true));
    let a = ara("export const config = { matcher: ['/api/sepet/:path*', '/panel/:id'] }");
    assert_eq!(a.kapsar("/api/sepet"), Some(true));
    assert_eq!(a.kapsar("/api/sepet/:id/kalem"), Some(true));
    assert_eq!(a.kapsar("/api/sepetler"), Some(false));
    assert_eq!(a.kapsar("/panel/:x"), Some(true));
    assert_eq!(a.kapsar("/panel/a/b"), Some(false));
    // Tek metin ve { source } nesnesi
    assert_eq!(ara(r#"export const config = { matcher: "/api/:yol+" }"#).kapsar("/api"), Some(false));
    assert_eq!(ara(r#"export const config = { matcher: [{ source: "/api/:yol*" }] }"#).kapsar("/api/x"), Some(true));
    // Next belgelerindeki dışlama kalıbı (Rust düzenli ifadesinde ileri bakış yok: elle)
    let h = ara(r"export const config = { matcher: ['/((?!api|_next/static|_next/image|favicon.ico).*)'] }");
    assert_eq!(h.kapsar("/api/urun"), Some(false));
    assert_eq!(h.kapsar("/panel"), Some(true));
    // Grup içindeki `|` seçenek ayırmaz
    let g = ara(r#"export const config = { matcher: ["/((?!api|giris|.*\\.(?:png|svg)).*)"] }"#);
    assert_eq!(g.kapsar("/api/x"), Some(false));
    assert_eq!(g.kapsar("/logo.svg"), Some(false));
    assert_eq!(g.kapsar("/panel"), Some(true));
    // has / missing koşulu ve değişken: kapsam belirlenemez, karar verilmez
    assert_eq!(ara("export const config = { matcher: [{ source: '/a', missing: [{ type: 'header', key: 'x' }] }] }").kapsar("/a"), None);
    assert_eq!(ara("export const config = { matcher: YOLLAR }").kapsar("/a"), None);
}

#[test]
fn kosullu_ara_katman_karar_vermez() {
    let mut a = ara("export const config = { matcher: ['/api/:p*'] }");
    a.kosullu = true;
    assert!(!a.giris_ister("/api/x"));
}
