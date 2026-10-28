from sqlmodel import Field, SQLModel


class User(SQLModel, table=True):
    id: int | None = Field(default=None, primary_key=True)
    email: str


class Item(SQLModel, table=True):
    id: int | None = Field(default=None, primary_key=True)
    title: str
    owner_id: int = Field(foreign_key="user.id")


class Profil:
    def __init__(self, user_id: int):
        self.user_id = user_id
        self.token = ""

    def baglanti_gonder(self) -> None:
        self.token = "x"
        self.save()
