from typing import Annotated

from fastapi import APIRouter, Depends, File, Form, Header, Query, UploadFile
from dis_kutuphane import dis_kontrol

from app.api.deps import CurrentUser, JWTBearer, maybe_user
from app.models import KitapOlustur, User

router = APIRouter(prefix="/kitaplar", tags=["kitaplar"])


@router.get("/")
def listele(q: Annotated[str | None, Query(max_length=50)] = None, sayfa: int = 1):
    """Kitapları listeler."""
    return []


@router.post("/", status_code=201)
async def olustur(kitap: KitapOlustur, user: CurrentUser):
    return kitap


@router.get("/{kitap_id}")
def getir(kitap_id: int, user: Annotated[User | None, Depends(maybe_user)]):
    return {}


@router.delete("/{kitap_id}", dependencies=[Depends(JWTBearer())])
def sil(kitap_id: int):
    return None


@router.put("/{kitap_id}/kapak")
def kapak(kitap_id: int, dosya: UploadFile = File(...), aciklama: str = Form(...), x_istek: str = Header(...)):
    return {}


@router.get("/dis/kontrol", dependencies=[Depends(dis_kontrol)])
def dis():
    return {}
