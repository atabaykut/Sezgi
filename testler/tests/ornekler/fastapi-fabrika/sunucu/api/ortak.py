from typing import Any

from fastapi import Body, Depends, Query


def SayfaBoyu() -> Any:
    """Ortak gövde parametresi: gövdede `limit`."""
    return Body(10, ge=1, le=200)


def sayfalama(skip: int = 0, limit: int = Query(100, le=500)):
    """Bağımlılığın kendi parametreleri uç noktanın sorgusuna katılır."""
    return {"skip": skip, "limit": limit}


def OffsetBody() -> Any:
    """Depends(iç fonksiyon) döndüren fabrika, parametre varsayılanı olarak: gövdede `offset`."""

    def get_offset(offset: int = Body(0, ge=0)):
        return offset

    return Depends(get_offset)
