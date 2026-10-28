from sqlmodel import Field, SQLModel


class User(SQLModel, table=True):
    id: int | None = Field(default=None, primary_key=True)
    email: str


class Item(SQLModel, table=True):
    __tablename__ = "esya"
    id: int | None = Field(default=None, primary_key=True)
    title: str
    owner_id: int = Field(foreign_key="user.id")


class UserUpdate(SQLModel):
    email: str | None = None
