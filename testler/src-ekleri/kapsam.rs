use super::*;

#[test]
fn dusuk_kapsam_kurallari() {
    assert!(dusuk(500, 0, 0), "hiç işlem yok");
    assert!(dusuk(5_000, 2, 15), "15 rota kaydı benzeri, 2 işlem");
    assert!(!dusuk(5_000, 14, 14), "kayıtlar kadar işlem");
    assert!(!dusuk(5_000, 2, 5), "az sayıda ifade karar vermez");
    assert!(dusuk(346_000, 7, 0), "büyük kodda çok az işlem");
    assert!(!dusuk(160_000, 955, 900));
}

#[test]
fn bagimlilik_adi_kendi_basina_eslesir() {
    assert!(geciyor(Dosya::Npm, r#"{"dependencies":{"ws":"^8"}}"#, "ws"));
    assert!(!geciyor(Dosya::Npm, r#"{"dependencies":{"isomorphic-ws":"^5"}}"#, "ws"));
    assert!(geciyor(Dosya::Py, "Django==5.0\ngraphene>=3\n", "graphene"));
    assert!(!geciyor(Dosya::Py, "graphene-django==3\n", "graphene"));
    assert!(geciyor(Dosya::Gem, "gem 'graphql', '~> 2.0'\n", "graphql"));
    assert!(geciyor(Dosya::Go, "require (\n\tgoogle.golang.org/grpc v1.60.0\n)\n", "google.golang.org/grpc"));
    assert!(geciyor(Dosya::Nuget, r#"<PackageReference Include="HotChocolate.AspNetCore" Version="13" />"#, "HotChocolate"));
}
