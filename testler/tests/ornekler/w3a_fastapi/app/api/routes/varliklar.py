import uuid
from typing import Any

from fastapi import APIRouter, BackgroundTasks, Depends, HTTPException

from app.api.deps import admin_iste

router = APIRouter(prefix="/varliklar", tags=["varliklar"], dependencies=[Depends(admin_iste)])


def eposta_gonder(adres: str) -> None:
    return None


@router.delete("/{id}")
def sil(id: uuid.UUID, arka_plan: BackgroundTasks) -> Any:
    if not id:
        raise HTTPException(status_code=404, detail="Yok")
    arka_plan.add_task(eposta_gonder, "yonetici@ornek")
    return {"ok": True}
