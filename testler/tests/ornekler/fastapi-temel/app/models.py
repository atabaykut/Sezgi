from enum import Enum
from typing import Literal

from pydantic import BaseModel, EmailStr, Field


class Dil(str, Enum):
    TR = "tr"
    EN = "en"


class KitapTemel(BaseModel):
    baslik: str = Field(min_length=2, max_length=200)
    sayfa: int = Field(ge=1, le=5000)


class KitapOlustur(KitapTemel):
    yazar: str | None = None
    tur: Literal["roman", "siir"]
    dil: Dil = Dil.TR
    isbn: str = Field(alias="ISBN")
    iletisim: EmailStr | None = None


class User(BaseModel):
    id: int
