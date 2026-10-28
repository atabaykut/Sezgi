import grafikcizer
from django.http import HttpResponse, JsonResponse
from django.shortcuts import get_object_or_404
from django.views import View

from kanal import eposta
from kanal.models import Kanal


def kanal_getir(code):
    kanal = get_object_or_404(Kanal, code=code)
    return kanal, True


class DenemeBildirimi(View):
    def post(self, request, code):
        # Kanalın türüne göre seçilen taşıma: Slack (HTTP) ya da e-posta
        kanal, _yaz = kanal_getir(code)
        hata = kanal.bildirim_gonder("deneme")
        return JsonResponse({"hata": hata})


class AdDegistir(View):
    def post(self, request, code):
        kanal = get_object_or_404(Kanal, code=code)
        kanal.name = request.POST.get("ad", "")
        kanal.save()
        return HttpResponse(status=204)


class Giris(View):
    def post(self, request):
        eposta.giris_baglantisi(request.POST.get("eposta", ""))
        return HttpResponse(status=202)


class Rapor(View):
    def get(self, request):
        return JsonResponse({"surum": grafikcizer.surum()})

    def post(self, request):
        grafikcizer.yayinla(request.POST.get("baslik", ""))
        return HttpResponse(status=202)
