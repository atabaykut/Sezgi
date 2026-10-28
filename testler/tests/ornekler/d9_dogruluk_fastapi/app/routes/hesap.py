from fastapi import APIRouter

from app.deps import CurrentUser
from app.entegrasyon import save
from app.models import Profil

router = APIRouter()


@router.post("/giris")
def giris(user_id: int) -> dict:
    profil = Profil(user_id)
    profil.baglanti_gonder()
    return {"ok": True}


@router.post("/entegrasyon")
def entegrasyon_kaydet(current_user: CurrentUser, rw: bool) -> dict:
    save({"user": current_user.id}, rw)
    return {"ok": True}
