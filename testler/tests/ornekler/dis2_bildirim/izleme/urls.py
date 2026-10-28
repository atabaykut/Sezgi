from django.urls import path

from kanal import views

urlpatterns = [
    path("kanallar/<uuid:code>/dene/", views.DenemeBildirimi.as_view()),
    path("kanallar/<uuid:code>/ad/", views.AdDegistir.as_view()),
    path("giris/", views.Giris.as_view()),
    path("rapor/", views.Rapor.as_view()),
]
