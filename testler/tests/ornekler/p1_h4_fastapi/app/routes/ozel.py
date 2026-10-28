from fastapi import APIRouter

router = APIRouter(tags=["ozel"], prefix="/ozel")


@router.post("/kullanicilar")
def kullanici_olustur():
    return {}
