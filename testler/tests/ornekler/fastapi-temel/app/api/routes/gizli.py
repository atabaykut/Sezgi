from fastapi import APIRouter

router = APIRouter(prefix="/gizli")


@router.post("/kullanici")
def kullanici_olustur():
    return {}
