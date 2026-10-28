from fastapi import APIRouter, Depends

from servis.guvenlik import varsa_kullanici

vitrin_router = APIRouter(prefix="/vitrin")


@vitrin_router.get("")
def vitrin(kullanici: dict | None = Depends(varsa_kullanici)):
    return []
