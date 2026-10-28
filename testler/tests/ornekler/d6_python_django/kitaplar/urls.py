from django.urls import path

from . import views

urlpatterns = [
    path("kitaplar/<int:kitap_id>/yorumlar/", views.YorumListesi.as_view(), name="yorumlar"),
    path("kitaplar/<int:pk>/yayinla/", views.YayinAPIView.as_view()),
    path("kitaplar/<int:pk>/dinamik/", views.DinamikView.as_view()),
    path("ara/", views.arama),
    path("toplu-sil/", views.toplu_sil),
]
