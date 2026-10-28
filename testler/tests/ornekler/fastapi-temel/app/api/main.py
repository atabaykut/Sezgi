from fastapi import APIRouter

from app.api.routes import gizli, kitaplar, raflar, yonetim
from app.api.routes.fabrika import rotalari_kur
from app.core.config import settings

api_router = APIRouter()
api_router.include_router(kitaplar.router)
api_router.include_router(yonetim.router)
api_router.include_router(raflar.router)
api_router.include_router(rotalari_kur())

if settings.DEBUG:
    api_router.include_router(gizli.router)
