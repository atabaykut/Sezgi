from django.urls import path

from . import views

urlpatterns = [
    path("giris/", views.giris),
]
