from sqlmodel import Session, select, text

from app.db import Odunc, Raf


def raf_bul(*, session: Session, ad: str) -> Raf | None:
    return session.exec(select(Raf).where(Raf.ad == ad)).first()


def odunc_ver(*, session: Session, raf_id: int, kisi: str) -> Odunc:
    kayit = Odunc(raf_id=raf_id, kisi=kisi)
    session.add(kayit)
    session.commit()
    return kayit


def rapor(*, session: Session) -> int:
    return session.execute(text("SELECT count(*) FROM odunc")).scalar()
