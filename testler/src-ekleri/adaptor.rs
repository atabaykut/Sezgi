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

#[test]
fn ic_ice_sema_butceyle_kesilir_ve_denetlenmemis_sayilir() {
    // Her katmanda 10 alan, her alan bir alt nesne: 4 katman 10.000 iç alan olurdu
    fn katman(d: usize) -> BTreeMap<String, Alan> {
        (0..10)
            .map(|i| {
                let ic = (d > 0).then(|| katman(d - 1));
                let alan = Alan { tip: Some("object".into()), zorunlu: true, belirsiz: false, kurallar: vec![], satir: 1, kalitimdan: None, degerler: vec![], ic };
                (format!("a{i}"), alan)
            })
            .collect()
    }
    let mut s = Sema { dto: "D".into(), dosya: None, bulunamadi: false, kalitim: None, alanlar: katman(4), adaylar: vec![], dizi: false };
    assert!(sema_kirp(&mut s));
    fn say(a: &BTreeMap<String, Alan>) -> usize {
        a.values().map(|x| x.ic.as_ref().map(|i| i.len() + say(i)).unwrap_or(0)).sum()
    }
    assert!(say(&s.alanlar) <= IC_ALAN_BUTCESI, "iç alan {}", say(&s.alanlar));
    // Üst düzey alanlar hep kalır; kesilen alan önizlemenin tanımadığı kuralı taşır ("kontrol edilemedi")
    assert_eq!(s.alanlar.len(), 10);
    fn kesik_var(a: &BTreeMap<String, Alan>) -> bool {
        a.values().any(|x| x.kurallar.iter().any(|k| k == KESILDI_KURALI) || x.ic.as_ref().is_some_and(kesik_var))
    }
    assert!(kesik_var(&s.alanlar));
    assert!(matches!(crate::kural::coz(KESILDI_KURALI, "object").as_slice(), [crate::kural::Kural::Bilinmiyor(_)]));
    // Bütçenin altındaki şema dokunulmadan kalır
    let mut kucuk = Sema { dto: "K".into(), dosya: None, bulunamadi: false, kalitim: None, alanlar: katman(1), adaylar: vec![], dizi: false };
    assert!(!sema_kirp(&mut kucuk));
}
