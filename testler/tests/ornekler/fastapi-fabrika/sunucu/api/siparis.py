from fastapi import APIRouter, Body, Depends

import sunucu.semalar as semalar
from sunucu.api import ortak

router = APIRouter(prefix="/siparisler")


# Tip modül yoluyla yazılmış: semalar.eylemler.SiparisOlustur
@router.post("/")
def olustur(siparis: semalar.eylemler.SiparisOlustur):
    return siparis


# Birden fazla gövde parametresi: FastAPI gövdeyi parametre adlarıyla gömer {"siparisler": {...}, "sayfa": 0}
@router.post("/filtre")
def filtre(siparisler: semalar.eylemler.SiparisFiltre = None, sayfa: int = Body(0, ge=0)):
    return []


# Varsayılanı, Body(...) döndüren projedeki bir yardımcı: parametre gövdededir
@router.post("/sayfa")
def sayfa(siparisler: semalar.eylemler.SiparisFiltre = None, limit: int = ortak.SayfaBoyu()):
    return []


@router.get("/liste")
def liste(sayfa: dict = Depends(ortak.sayfalama)):
    return []


@router.post("/kaydir")
def kaydir(siparisler: semalar.eylemler.SiparisFiltre = None, offset: int = ortak.OffsetBody()):
    return []
