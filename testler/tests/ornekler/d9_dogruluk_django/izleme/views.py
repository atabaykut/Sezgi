from django.core.exceptions import PermissionDenied
from django.http import HttpRequest, HttpResponse, HttpResponseForbidden, JsonResponse
from django.shortcuts import get_object_or_404
from django.views.decorators.http import require_POST

from .models import Kontrol, Proje


def _yazilabilir_proje(request: HttpRequest, code) -> Proje:
    proje = get_object_or_404(Proje, code=code)
    rw = request.user.is_superuser
    if not rw:
        raise PermissionDenied

    return proje


@require_POST
def duraklat(request: HttpRequest, code) -> HttpResponse:
    kontrol = get_object_or_404(Kontrol, code=code)
    if kontrol.proje_id != request.project.id:
        return HttpResponseForbidden()

    kontrol.duraklat()
    return JsonResponse({"durum": kontrol.durum})


@require_POST
def entegrasyon_ekle(request: HttpRequest, code) -> HttpResponse:
    proje = _yazilabilir_proje(request, code)
    if not request.POST.get("ad"):
        return JsonResponse({"hata": "ad gerekli"}, status=400)
    return JsonResponse({"proje": proje.ad})


def save(request: HttpRequest) -> HttpResponse:
    proje = _yazilabilir_proje(request, request.GET["code"])
    return JsonResponse({"proje": proje.ad})


def _guncelle(request: HttpRequest, code) -> HttpResponse:
    kontrol = get_object_or_404(Kontrol, code=code)
    if not request.POST.get("durum"):
        return JsonResponse({"hata": "durum gerekli"}, status=400)
    kontrol.durum = request.POST["durum"]
    kontrol.save()
    return JsonResponse({"durum": kontrol.durum})


def _sil(request: HttpRequest, code) -> HttpResponse:
    kontrol = get_object_or_404(Kontrol, code=code)
    if kontrol.proje_id != request.project.id:
        return HttpResponseForbidden()
    kontrol.delete()
    return HttpResponse(status=204)


def _getir(request: HttpRequest, code) -> HttpResponse:
    kontrol = get_object_or_404(Kontrol, code=code)
    return JsonResponse({"durum": kontrol.durum})


def tekil(request: HttpRequest, code) -> HttpResponse:
    if request.method == "POST":
        return _guncelle(request, code)

    if request.method == "DELETE":
        return _sil(request, code)

    return _getir(request, code)
