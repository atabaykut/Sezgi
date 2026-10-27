# Sonuç 17: Django ve Flask adaptörleri (geliştirme ölçümü)

Bu ölçüm holdout değildir: adaptörler bu uygulamalara bakılarak yazıldı ve düzeltildi (seviye 1). Ölçülen kod, bu belgeyle
aynı commit'teki Sezgi (`cargo build`). Uç sayıları elle sayımla, iki RealWorld uygulaması ayrıca bağımsız hakemle
(RealWorld API belgesi) karşılaştırıldı.

- **Hakem:** gothinkster/realworld `a0767b867105`, `api/openapi.yml` (19 işlem), önek `/api`.
  Karşılaştırıcı Sonuç 12'dekiyle aynı kurallar: belgede `security` yoksa ya da `{}` içeriyorsa kimliksiz istek geçer
  ("herkese açık" ile eşleşir); gövde alanları zarfın (`{"user": {...}}`) içinden sayılır; yol parametresi adları
  karşılaştırılmaz.
- **Elle sayım:** her `path` / `re_path` / `url` kaydı, router kaydı ve `@action`, her `@route` / `add_url_rule` /
  `add_resource` için kodda kabul edilen yöntemler (HEAD ve OPTIONS sayılmaz).

## Django

| Uygulama | Commit | Elle sayım | Sezgi | Not |
|---|---|---|---|---|
| gothinkster/django-realworld-example-app (DRF) | `29c9d42831fa` | 20 | **20/20** | `url()` + düzenli ifade, `DefaultRouter(trailing_slash=False)`, karışımlı ViewSet |
| encode/rest-framework-tutorial (DRF) | `8429d9572e93` | 10 | **10/10** | `ModelViewSet`, `ReadOnlyModelViewSet`, `@action`, router kökü |

RealWorld belgesine karşı (django-realworld-example-app):

| Ölçüt | Sonuç |
|---|---|
| Endpoint bulma | **18/19** (uygulamada `DELETE /api/articles/{slug}` yok: ViewSet yok etme karışımını içermiyor) |
| Belgede olmayan | 2 (`PATCH /api/user`: `RetrieveUpdateAPIView` verir; `GET /api`: `DefaultRouter`'ın kök görünümü) |
| Giriş durumu | **18/18**, kesin damgalı yanlış karar 0 |
| Gövde alanı (zarf içi) | **16/18** |
| Zorunluluk | 15/16 |

Eksik iki alan `PUT /api/user`'daki `bio` ve `image`: işleyici zarfın içini serializer'ın iç içe `profile` alanına
taşıyor; Sezgi alanları serializer'dan aday olarak ve belirsiz damgayla verir.

## Flask

| Uygulama | Commit | Elle sayım | Sezgi | Not |
|---|---|---|---|---|
| gothinkster Flask RealWorld örneği (`$F-realworld-example-app`, F=flask) | `4b95fb2227df` | 19 | **19/19** | fabrika + yardımcı fonksiyonla Blueprint kaydı, flask-apispec, marshmallow `pre_load` zarfı |
| miguelgrinberg/microblog | `a975ef648643` | 35 | **35/35** | kayıtta `url_prefix`, Flask-Login, Flask-HTTPAuth |
| miguelgrinberg/flasky | `3beedd640b91` | 50 | **50/50** | `before_request` + `@auth.login_required`, proje dekoratörüyle izin |

RealWorld belgesine karşı (Flask RealWorld örneği):

| Ölçüt | Sonuç |
|---|---|
| Endpoint bulma | **19/19** |
| Belgede olmayan | 0 |
| Giriş durumu | **19/19**, kesin damgalı yanlış karar 0 |
| Gövde alanı (zarf içi) | **18/18** |
| Zorunluluk | 9/18 |

Zorunluluk farklarının hepsi aynı nedenden: uygulamanın marshmallow şemalarında alanlar `required=True` taşımıyor,
yani kod bu alanları istemiyor; belge istiyor. Sezgi kodun davranışını yazar.

## Tekrar üretme

```sh
git clone https://github.com/gothinkster/django-realworld-example-app && git -C django-realworld-example-app checkout 29c9d42831fa
F=flask; git clone https://github.com/gothinkster/$F-realworld-example-app fl-app && git -C fl-app checkout 4b95fb2227df
git clone https://github.com/gothinkster/realworld && git -C realworld show a0767b867105:api/openapi.yml > rw.yml
sezgi init django-realworld-example-app --cikti dj
sezgi init fl-app --cikti fl
```
