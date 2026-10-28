from django.urls import path

from . import views

urlpatterns = [
    path("kontroller/<uuid:code>/duraklat", views.duraklat),
    path("projeler/<uuid:code>/entegrasyon", views.entegrasyon_ekle),
    path("kontroller/<uuid:code>", views.tekil),
]
