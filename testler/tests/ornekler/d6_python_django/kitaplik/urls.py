from django.conf import settings
from django.contrib import admin
from django.urls import include, path, re_path
from rest_framework.routers import DefaultRouter

from kitaplar import views

router = DefaultRouter()
router.register(r"kitaplar", views.KitapViewSet, basename="kitap")
router.register(r"yazarlar", views.YazarViewSet)

urlpatterns = [
    path("admin/", admin.site.urls),
    path("api/", include(router.urls)),
    # Sürümlü uç noktalar
    path("api/v1/", include("kitaplar.urls")),
    re_path(r"^eski/(?P<yil>[0-9]{4})/$", views.eski_arsiv),
    path("hesap/", include("hesap.urls")),
]

if settings.DEBUG:
    urlpatterns += [path("debug/saglik/", views.saglik)]
