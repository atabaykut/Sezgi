// dis.rs birim testleri: her dilin desenleri derlenir ve bilinen kalıplar bulunur; hedef çözümü ve güven damgası
use super::*;

fn tara1(dil: Dil, m: &str) -> DosyaSonuc {
    dosya_tara(dil, m)
}

#[test]
fn her_dilin_desenleri_derlenir_ve_calisir() {
    // Bir desen derlenemezse tarama iş parçacığı çöker ve proje "0 dosya" görünür: her dil burada çalıştırılır
    let ornekler: [(Dil, &str); 7] = [
        (Dil::Js, "import axios from 'axios';\nexport async function f() {\n  return axios.post('https://api.x.com/v1/a', {});\n}\n"),
        (Dil::Py, "import requests\n\ndef f():\n    return requests.get('https://api.x.com/a')\n"),
        (Dil::Jvm, "class A {\n  private RestTemplate restTemplate;\n  public String f() {\n    return restTemplate.getForObject(\"https://api.x.com/a\", String.class);\n  }\n}\n"),
        (Dil::Cs, "using System.Net.Http;\nclass A {\n  public async Task F() {\n    await _httpClient.GetAsync(\"https://api.x.com/a\");\n  }\n}\n"),
        (Dil::Go, "package a\nfunc F() {\n\thttp.Get(\"https://api.x.com/a\")\n}\n"),
        (Dil::Php, "<?php\nclass A {\n  public function f() {\n    return Http::post('https://api.x.com/a', []);\n  }\n}\n"),
        (Dil::Rb, "class A\n  def f\n    Net::HTTP.get(URI('https://api.x.com/a'))\n  end\nend\n"),
    ];
    for (dil, m) in ornekler {
        let s = tara1(dil, m);
        assert!(!s.bloklar.is_empty(), "{dil:?}: blok yok");
        let d = s.disler.first().unwrap_or_else(|| panic!("{dil:?}: dış çağrı yok"));
        assert_eq!(d.tur, "http", "{dil:?}");
        if dil != Dil::Rb {
            assert_eq!((d.hedef.as_str(), d.hedef_turu, d.guven), ("api.x.com", "url", "certain"), "{dil:?}: {d:?}");
        }
    }
}

#[test]
fn hedef_cozumu_sabit_ortam_dinamik() {
    let dosya = "const { PAYPAL_API_URL } = process.env;\nconst TABAN = 'https://api.ornek.com';\n";
    assert_eq!(hedef_coz("`${PAYPAL_API_URL}/v1/oauth2/token`", "", dosya, 0), ("PAYPAL_API_URL".to_string(), "ortam", Some("/v1/oauth2/token".to_string()), "inferred"));
    assert_eq!(hedef_coz("TABAN + '/x'", "", dosya, 0).0, "api.ornek.com");
    assert_eq!(hedef_coz("TABAN + '/x'", "", dosya, 0).3, "certain");
    assert_eq!(hedef_coz("process.env.UCUS_API", "", "", 0).1, "ortam");
    assert_eq!(hedef_coz("f\"{settings.SMTP_HOST}/a\"", "", "", 0).0, "SMTP_HOST");
    assert_eq!(hedef_coz("adres", "", "", 0).1, "dinamik");
    assert_eq!(hedef_coz("'/api/x'", "", "", 0).1, "goreli", "göreli adres uygulamanın kendisidir");
}

#[test]
fn sdk_kuyruk_ve_eposta() {
    let js = "const { Kafka } = require('kafkajs');\nasync function f() {\n  await producer.send({ topic: 'siparis', messages: [] });\n  await posta.sendMail({ to: 'a' });\n}\n";
    let s = tara1(Dil::Js, js);
    assert!(s.disler.iter().any(|d| d.tur == "kuyruk" && d.kanal.as_deref() == Some("siparis")), "{:?}", s.disler);
    assert!(s.disler.iter().any(|d| d.tur == "eposta"), "{:?}", s.disler);
    let php = "<?php\nclass A {\n  public function f() {\n    Mail::to($u)->queue(new Hosgeldin($u));\n  }\n}\n";
    let s = tara1(Dil::Php, php);
    assert!(s.disler.iter().any(|d| d.tur == "eposta" && d.ertelenmis), "{:?}", s.disler);
    let rb = "class A\n  def f\n    UserMailer.welcome(u).deliver_later\n  end\nend\n";
    assert!(tara1(Dil::Rb, rb).disler.iter().any(|d| d.tur == "eposta" && d.ertelenmis));
}

#[test]
fn on_kosul_ve_silme_desenleri() {
    let php = "<?php\nclass A {\n  public function f($p) {\n    abort_unless($p->acik, 403);\n    if ($p->kilitli) {\n      throw new ConflictHttpException('x');\n    }\n    $p->forceDelete();\n  }\n}\n";
    let s = tara1(Dil::Php, php);
    assert!(s.retler.iter().any(|r| r.durum == Some(403) && r.kosul.as_deref() == Some("!($p->acik)")), "{:?}", s.retler);
    assert!(s.retler.iter().any(|r| r.durum == Some(409) && r.kosul.as_deref() == Some("$p->kilitli")), "{:?}", s.retler);
    assert!(s.silmeler.iter().any(|x| x.yontem == "forceDelete"));
    // Koleksiyon silmesi veri silmesi değildir; depo silmesi öyledir
    let js = "async function f(id) {\n  seen.delete(id);\n  this.service.delete(id);\n  await this.notRepository.delete(id);\n}\n";
    let s = tara1(Dil::Js, js);
    assert_eq!(s.silmeler.iter().map(|x| x.alici.as_str()).collect::<Vec<_>>(), ["notRepository"]);
}

#[test]
fn yumusak_silme_modelleri_ve_kalitim() {
    let php = "<?php\nclass Entity extends Model {\n  use SoftDeletes;\n}\nclass Page extends Entity {}\n";
    let s = tara1(Dil::Php, php);
    assert!(s.yumusak_modeller.iter().any(|m| m.0 == "Entity"));
    assert!(s.ustler.iter().any(|u| u.0 == "Page" && u.1 == "Entity"));
    let go = "package m\ntype GVA_MODEL struct {\n\tDeletedAt gorm.DeletedAt\n}\ntype SysApi struct {\n\tglobal.GVA_MODEL\n\tPath string\n}\n";
    let s = tara1(Dil::Go, go);
    assert!(s.yumusak_modeller.iter().any(|m| m.0 == "GVA_MODEL"));
    assert!(s.ustler.iter().any(|u| u.0 == "SysApi" && u.1 == "GVA_MODEL"), "{:?}", s.ustler);
    assert_eq!(go_silme_tipi("err = global.GVA_DB.Delete(&entity).Error", &["\tvar entity system.SysApi"]), Some("SysApi".into()));
}

#[test]
fn feign_istemcisi_dis_cagridir() {
    let java = "@FeignClient(name = \"stok\", url = \"${stok.url}\")\npublic interface StokIstemcisi {\n  @PostMapping(\"/rezerve\")\n  Sonuc rezerve(Istek i);\n}\n";
    let s = tara1(Dil::Jvm, java);
    let d = s.disler.first().expect("feign");
    assert_eq!((d.istemci.as_str(), d.hedef.as_str(), d.hedef_turu, d.yontem.as_deref()), ("Feign", "stok.url", "ortam", Some("POST")));
    assert!(s.bloklar.iter().any(|b| b.0 == "rezerve"));
}

#[test]
fn di_taban_adresi_typed_httpclient() {
    // Kayıt (Program / Extensions) ve typed client ayrı dosyalarda: göreli adres kaydın tabanıyla çözülür
    let kayit = "builder.Services.AddHttpClient<OrderingService>(o => o.BaseAddress = new(\"https+http://ordering-api\"));\n";
    let istemci = "using System.Net.Http;\npublic class OrderingService(HttpClient httpClient)\n{\n    public Task<O[]> Al()\n    {\n        return httpClient.GetFromJsonAsync<O[]>(\"/api/Orders/\")!;\n    }\n}\n";
    let mut d = vec![
        Dosya { yol: "Extensions.cs".into(), mutlak: PathBuf::new(), ozet: String::new(), sonuc: tara1(Dil::Cs, kayit) },
        Dosya { yol: "OrderingService.cs".into(), mutlak: PathBuf::new(), ozet: String::new(), sonuc: tara1(Dil::Cs, istemci) },
    ];
    assert_eq!(d[0].sonuc.tabanlar.first().map(|t| t.1 .0.as_str()), Some("ordering-api"));
    assert_eq!(d[1].sonuc.disler.first().map(|x| x.hedef_turu), Some("goreli"));
    tabanlari_uygula(&mut d);
    let x = d[1].sonuc.disler.first().expect("typed client çağrısı");
    assert_eq!((x.hedef.as_str(), x.hedef_turu, x.yol.as_deref()), ("ordering-api", "url", Some("/api/Orders/")));
    // Kaydı olmayan göreli çağrı uygulamanın kendisidir: atılır
    let mut tek = vec![Dosya { yol: "A.cs".into(), mutlak: PathBuf::new(), ozet: String::new(), sonuc: tara1(Dil::Cs, istemci) }];
    tabanlari_uygula(&mut tek);
    assert!(tek[0].sonuc.disler.is_empty());
}

#[test]
fn gecmis_isaretleri() {
    assert!(gecmis_yazmasi("    $this->revisionRepo->storeNewForPage($page, $summary);").is_some());
    assert!(gecmis_yazmasi("    $revision = new PageRevision();").is_some());
    assert!(gecmis_yazmasi("    $page->save();").is_none());
    let rb = "class Article < ApplicationRecord\n  has_paper_trail\nend\n";
    assert_eq!(tara1(Dil::Rb, rb).gecmis_modeller.first().map(|x| x.0.as_str()), Some("Article"));
    let java = "@Entity\n@Audited\npublic class Siparis {\n}\n";
    assert_eq!(tara1(Dil::Jvm, java).gecmis_modeller.first().map(|x| x.0.as_str()), Some("Siparis"));
}

#[test]
fn kimlik_bilgisi_onay_gerekcesi_ingilizceye_cevrilir() {
    let tr = "işlem kimlik bilgisi değiştiriyor (parola, token, anahtar ya da iki adımlı doğrulama) ve geri alınabilirliği kanıtlanmadı";
    assert_eq!(
        gerekce_cevir(tr, true),
        "the operation changes credentials (password, token, key or two-factor authentication) and its reversibility is not proven"
    );
    assert_eq!(gerekce_cevir(tr, false), tr);
}
