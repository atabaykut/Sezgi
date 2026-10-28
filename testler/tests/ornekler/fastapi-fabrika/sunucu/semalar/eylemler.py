from pydantic import BaseModel, Field


class SiparisOlustur(BaseModel):
    urun: str = Field(min_length=1)
    adet: int = Field(default=1, ge=1)
    # default=... (Ellipsis): zorunlu
    kod: str = Field(default=..., max_length=10)


class SiparisFiltre(BaseModel):
    durum: str | None = None
