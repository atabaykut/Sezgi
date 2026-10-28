from fastapi import APIRouter, Depends

from servis.guvenlik import kullanici_al
from servis.rotalar.isler import isler_router
from servis.rotalar.vitrin import vitrin_router

genel_router = APIRouter(prefix="/api/v2")

girisli_router = APIRouter(dependencies=[Depends(kullanici_al)])
girisli_router.include_router(isler_router)

genel_router.include_router(girisli_router)
genel_router.include_router(vitrin_router)
