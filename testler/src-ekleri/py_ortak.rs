use super::*;

#[test]
fn bolme_ve_cagri() {
    let p = parcalar("path('a/', x),  # yorum, b\n re_path(r'^(?P<pk>[0-9]+)/$', y),", ',');
    assert_eq!(p, vec!["path('a/', x)", "re_path(r'^(?P<pk>[0-9]+)/$', y)"]);
    let c = cagri_ayir("path('k/<int:pk>/', views.detay, name='detay')").unwrap();
    assert_eq!(c.ad, "path");
    assert_eq!(c.arg(1), Some("views.detay"));
    assert_eq!(c.anahtar("name"), Some("'detay'"));
    let c = cagri_ayir("cache_page(60)(views.x)").unwrap();
    assert_eq!((c.ad.as_str(), c.arg(0)), ("cache_page(60)", Some("views.x")));
    assert!(cagri_ayir("[a, b]").is_none());
    assert_eq!(donusturuculu_yol("k/<int:pk>/<slug>/").0, "k/:pk/:slug/");
    assert_eq!(yol_ekle("/api/", "kitaplar/"), "/api/kitaplar/");
    assert_eq!(govdedeki_yontemler("if request.method == 'POST':\n  x\nelif request.method in ('PUT', 'PATCH'):"), vec!["POST", "PUT", "PATCH"]);
}
