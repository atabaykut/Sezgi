from django.urls import include, path

urlpatterns = [
    path("api/", include("izleme.urls")),
    path("hesap/", include("hesap.urls")),
]
