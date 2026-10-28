use super::*;

#[test]
fn nesne_ve_dizi_ayristirma() {
    let m = "{ imports: [X.forRoot({ providers: [Ic] }), forwardRef(() => Y)], controllers: LISTE, providers: [{ useClass: G, provide: APP_GUARD }, 'a,b'], }";
    assert_eq!(dizi_cek(m, "providers"), vec!["{ useClass: G, provide: APP_GUARD }", "'a,b'"]);
    assert_eq!(dizi_cek(m, "controllers"), vec!["...LISTE"]);
    assert_eq!(dizi_cek(m, "imports"), vec!["X.forRoot({ providers: [Ic] })", "forwardRef(() => Y)"]);
    // controllers: değer dizi değil, ardındaki başka bir dizi alınmamalı
    assert_eq!(dizi_cek("{ controllers: kontrolculer(), providers: [P] }", "controllers"), Vec::<String>::new());
    assert_eq!(nesne_degeri("{ provide: APP_GUARD, useClass: G }", "useClass").as_deref(), Some("G"));
    assert_eq!(nesne_degeri("{ controllers }", "controllers").as_deref(), Some("controllers"));
    assert_eq!(dizi_ogeleri("['tr', \"e,n\"]"), Some(vec!["'tr'".to_string(), "\"e,n\"".to_string()]));
    assert!(test_dosyasi(Path::new("test/app.module.ts")) && !test_dosyasi(Path::new("src/app.module.ts")));
}

#[test]
fn turetme_ifadeleri() {
    use Turetme::*;
    let s = |x: &str| Sinif(x.into());
    let a = |v: &[&str]| Anahtarlar(Some(v.iter().map(|x| x.to_string()).collect()));
    assert_eq!(turetme_ayristir("Taban<T, Array<U>>"), Some(s("Taban")));
    assert_eq!(turetme_ayristir("PartialType(OmitType(Temel, ['id'] as const))"), Some(Cagri("PartialType".into(), vec![Cagri("OmitType".into(), vec![s("Temel"), a(&["id"])])])));
    assert_eq!(turetme_ayristir("PickType(Temel, SECILEN)"), Some(Cagri("PickType".into(), vec![s("Temel"), s("SECILEN")])));
    assert_eq!(turetme_ayristir("PickType(Temel, ['a', ...DIGER])"), Some(Cagri("PickType".into(), vec![s("Temel"), Anahtarlar(None)])));
    assert_eq!(turetme_ayristir("PartialType(Temel, { skipNullProperties: false })"), Some(Cagri("PartialType".into(), vec![s("Temel"), Diger])));
    // Bozuk ya da beklenmeyen sözdizimi: belirsiz
    assert_eq!(turetme_ayristir("PartialType(Temel"), None);
    assert_eq!(turetme_ayristir("(() => Temel)()"), None);
    // Derinlik sınırı
    assert_eq!(turetme_ayristir(&format!("{}X{}", "F(".repeat(100), ")".repeat(100))), None);
}
