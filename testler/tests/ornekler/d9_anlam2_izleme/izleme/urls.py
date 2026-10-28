from django.urls import path

from izleme import views

urlpatterns = [
    path("api/v1/checks/", views.list_checks),
    path("api/v1/checks/<uuid:code>", views.single),
    path("api/v1/checks/<uuid:code>/pause", views.pause),
    path("api/v1/checks/<uuid:code>/resume", views.resume),
    path("api/v1/badges/", views.badges),
    path("ping/<uuid:code>/", views.ping),
    path("checks/", views.checks_page),
    path("checks/<uuid:code>/remove/", views.remove_check),
    path("checks/<uuid:code>/transfer/", views.transfer_check),
    path("integrations/", views.channels),
    path("integrations/<uuid:code>/test/", views.send_test_notification),
    path("oups", views.oups),
    path("api/v2/checks/", views.checks),
]
