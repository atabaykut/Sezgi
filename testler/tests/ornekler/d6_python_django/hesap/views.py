from django import forms
from django.contrib.auth.mixins import LoginRequiredMixin
from django.http import JsonResponse
from django.views import View
from django.views.generic.edit import FormView


class ProfilView(LoginRequiredMixin, View):
    def get(self, request):
        return JsonResponse({"ad": request.user.username})

    def post(self, request):
        return JsonResponse({})


class AyarlarView(View):
    http_method_names = ["get"]

    def get(self, request):
        return JsonResponse({})

    def post(self, request):
        return JsonResponse({})


class KayitFormu(forms.Form):
    kullanici_adi = forms.CharField(max_length=30)
    eposta = forms.EmailField()
    yas = forms.IntegerField(min_value=13, required=False)


class KayitView(FormView):
    form_class = KayitFormu
    template_name = "kayit.html"
