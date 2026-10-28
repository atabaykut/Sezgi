from sqlmodel import Field, SQLModel


class OdaKaydi(SQLModel, table=True):
    __tablename__ = "oda_kayitlari"
    id: int | None = Field(default=None, primary_key=True)
    voucher: str
    pasaport_no: str
    oda: str


class OdaKaydiGirdisi(SQLModel):
    voucher: str
    pasaport_no: str
    oda: str
