from sqlmodel import Field, Session, SQLModel


class Raf(SQLModel, table=True):
    id: int | None = Field(default=None, primary_key=True)
    ad: str


class Odunc(SQLModel, table=True):
    id: int | None = Field(default=None, primary_key=True)
    raf_id: int
    kisi: str


# Tablo değil: yalnızca girdi şeması
class RafGirdisi(SQLModel):
    ad: str


def oturum() -> Session:
    return Session()
