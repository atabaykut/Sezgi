from typing import Annotated

from fastapi import APIRouter, Depends
from sqlmodel import Session

from app import crud
from app.db import Raf, RafGirdisi, oturum

router = APIRouter(prefix="/raflar")
OturumDep = Annotated[Session, Depends(oturum)]
SeciliRaf = Annotated[Raf, Depends(crud.raf_bul)]


@router.get("/")
def listele(session: OturumDep):
    return crud.raf_bul(session=session, ad="a")


@router.post("/")
def olustur(session: OturumDep, girdi: RafGirdisi):
    raf = Raf.model_validate(girdi)
    session.add(raf)
    session.commit()
    return raf


@router.patch("/{ad}")
def yeniden_adlandir(session: OturumDep, ad: str, yeni: str):
    raf = crud.raf_bul(session=session, ad=ad)
    raf.ad = yeni
    session.add(raf)
    session.commit()
    return raf


@router.delete("/secili")
def secileni_sil(session: OturumDep, raf: SeciliRaf):
    session.delete(raf)
    session.commit()


@router.post("/{raf_id}/odunc")
def odunc(session: OturumDep, raf_id: int, kisi: str):
    if kisi:
        return crud.odunc_ver(session=session, raf_id=raf_id, kisi=kisi)


@router.get("/rapor")
def rapor(session: OturumDep):
    return crud.rapor(session=session)
