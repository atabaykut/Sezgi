from fastapi import APIRouter, Depends
from sqlmodel import select

from app.deps import SessionDep, get_current_active_superuser
from app.models import Oge

router = APIRouter(prefix="/ogeler", tags=["ogeler"])


@router.get("/")
def ogeleri_oku(session: SessionDep):
    return session.exec(select(Oge)).all()


@router.delete("/{oge_id}", dependencies=[Depends(get_current_active_superuser)])
def oge_sil(oge_id: int, session: SessionDep):
    oge = session.get(Oge, oge_id)
    session.delete(oge)
    session.commit()
