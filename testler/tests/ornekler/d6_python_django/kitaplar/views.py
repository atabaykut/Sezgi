from django.contrib.auth.decorators import login_required, permission_required
from django.core.exceptions import PermissionDenied
from django.http import JsonResponse
from django.shortcuts import get_object_or_404
from django.views.decorators.http import require_http_methods
from rest_framework import generics, permissions, viewsets
from rest_framework.decorators import action, api_view, permission_classes
from rest_framework.response import Response
from rest_framework.views import APIView

from .models import Kitap, Yazar, Yorum
from .permissions import EditorMu
from .serializers import KitapSerializer, YazarSerializer, YorumSerializer


class KitapViewSet(viewsets.ModelViewSet):
    """Kitap kataloğu"""

    queryset = Kitap.objects.all()
    serializer_class = KitapSerializer
    permission_classes = [permissions.IsAuthenticatedOrReadOnly]
    search_fields = ["baslik"]

    @action(detail=True, methods=["post"], url_path="yorum-ekle", permission_classes=[permissions.IsAuthenticated], serializer_class=YorumSerializer)
    def yorum_ekle(self, request, pk=None):
        kitap = self.get_object()
        s = YorumSerializer(data=request.data)
        s.is_valid(raise_exception=True)
        Yorum.objects.create(kitap=kitap, **s.validated_data)
        return Response(status=201)

    @action(detail=False)
    def populer(self, request):
        return Response(KitapSerializer(Kitap.objects.order_by("-sayfa")[:5], many=True).data)

    def perform_destroy(self, instance):
        if instance.durum == "yayinda":
            raise PermissionDenied("yayındaki kitap silinemez")
        instance.delete()


class YazarViewSet(viewsets.ReadOnlyModelViewSet):
    queryset = Yazar.objects.all()
    serializer_class = YazarSerializer
    permission_classes = [permissions.AllowAny]


class YorumListesi(generics.ListCreateAPIView):
    serializer_class = YorumSerializer

    def get_queryset(self):
        return Yorum.objects.filter(kitap_id=self.kwargs["kitap_id"])


class YayinAPIView(APIView):
    permission_classes = [EditorMu]

    def post(self, request, pk):
        kitap = get_object_or_404(Kitap, pk=pk)
        kitap.durum = "yayinda"
        kitap.save()
        return Response({"durum": kitap.durum})


class DinamikView(generics.RetrieveAPIView):
    queryset = Kitap.objects.all()
    serializer_class = KitapSerializer

    def get_permissions(self):
        if self.request.method == "GET":
            return [permissions.AllowAny()]
        return [permissions.IsAuthenticated()]


@api_view(["GET", "POST"])
@permission_classes([permissions.AllowAny])
def arama(request):
    q = request.query_params.get("q")
    return Response({"sonuc": list(Kitap.objects.filter(baslik__icontains=q).values("id"))})


def saglik(request):
    return JsonResponse({"durum": "ok"})


@login_required
@require_http_methods(["GET", "POST"])
def eski_arsiv(request, yil):
    return JsonResponse({"yil": yil})


@permission_required("kitaplar.delete_kitap", raise_exception=True)
def toplu_sil(request):
    if request.method == "POST":
        Kitap.objects.filter(durum="taslak").delete()
    return JsonResponse({})
