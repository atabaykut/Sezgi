from sqlmodel import Field, SQLModel


class Oge(SQLModel, table=True):
    id: int | None = Field(default=None, primary_key=True)
    ad: str
