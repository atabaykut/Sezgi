from typing import Annotated

from fastapi import APIRouter, Depends
from sqlmodel import Session, select

from app.db import oturum
from app.models import OdaKaydi, OdaKaydiGirdisi

router = APIRouter(prefix="/odalar")
OturumDep = Annotated[Session, Depends(oturum)]


@router.get("/")
def listele(session: OturumDep):
    return session.exec(select(OdaKaydi)).all()


@router.post("/")
def olustur(session: OturumDep, girdi: OdaKaydiGirdisi):
    kayit = OdaKaydi.model_validate(girdi)
    session.add(kayit)
    session.commit()
    return kayit
