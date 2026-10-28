from django.contrib.auth.decorators import login_required
from django.urls import path
from django.views.generic import TemplateView

from . import views

urlpatterns = [
    path("profil/", views.ProfilView.as_view()),
    path("ayarlar/", login_required(views.AyarlarView.as_view())),
    path("hakkinda/", TemplateView.as_view(template_name="hakkinda.html")),
    path("kayit/", views.KayitView.as_view()),
]
