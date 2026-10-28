from fastapi import APIRouter, Depends

from app.api.deps import RolDenetimi, rol_iste, sayfa_boyu
from app.core.config import settings


def rotalari_kur() -> APIRouter:
    router = APIRouter(prefix="/fabrika")

    @router.get("/durum")
    def durum(boyut: int = Depends(sayfa_boyu(20))):
        return {"ok": True}

    @router.post("/{ad}")
    def ekle(ad: str, adet: int = 1):
        return {"ad": ad, "adet": adet}

    @router.delete("/{ad}", dependencies=[Depends(rol_iste("admin"))])
    def sil(ad: str):
        return None

    @router.put("/{ad}")
    def guncelle(ad: str, user=Depends(RolDenetimi(["editor", "admin"]))):
        return {"ad": ad}

    # İki kat iç içe: router dış fabrikanın kapsamında
    def raporlari_ekle():
        @router.get("/rapor")
        def rapor():
            return []

    raporlari_ekle()

    # Koşul altında kaydedilen rota: koşullu kapı
    if settings.DENEME_ACIK:

        @router.post("/deneme")
        def deneme():
            return None

    return router
