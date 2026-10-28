from typing import Literal

from fastapi import APIRouter
from pydantic import BaseModel, Field

router = APIRouter(prefix="/gorseller")


class GorselIste(BaseModel):
    istem: str
    kalite: Literal["hizli", "standart", "yuksek"] = "standart"
    adim: int = Field(default=30, ge=1, le=150)
    # Adın parçası ve çok sözcüklü değer: performance_selection / "Extreme Speed"
    performance_selection: Literal["Speed", "Quality", "Extreme Speed"] = "Speed"
    image_number: int = Field(default=1, ge=1, le=8)
    # Hesabı değil cevabı belirler: ipucu verilmemeli
    page_size: int = 20
    seed: int = -1


@router.post("/")
def uret(istek: GorselIste):
    return {}


@router.post("/{is_id}/devam")
def devam(is_id: str):
    """Yarıda kalan üretimi kaldığı yerden sürdürür."""
    return {}
