from django.http import HttpRequest, HttpResponse
from django.shortcuts import redirect
from django.views.decorators.http import require_POST

from izleme.models import Profil


@require_POST
def giris(request: HttpRequest) -> HttpResponse:
    profil = Profil.objects.get(eposta=request.POST["eposta"])
    profil.baglanti_gonder()
    return redirect("/")
