from fastapi import APIRouter

from app.deps import Kullanici

router = APIRouter(prefix="/siparisler")


@router.get("/")
def listele(kullanici: Kullanici):
    return []
