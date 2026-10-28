import uuid

from sqlmodel import Field, SQLModel


class User(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    email: str
    is_superuser: bool = False


class Album(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    title: str
    owner_id: uuid.UUID = Field(foreign_key="user.id")


class Photo(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    album_id: uuid.UUID = Field(foreign_key="album.id")
    exif: str | None = None


class Exif(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    camera: str


class AlbumCreate(SQLModel):
    title: str = Field(min_length=1, max_length=100)


class AlbumUpdate(SQLModel):
    title: str | None = None
    owner_id: uuid.UUID | None = None


class AlbumUsers(SQLModel):
    user_ids: list[uuid.UUID]


class UserRegister(SQLModel):
    email: str
    password: str = Field(min_length=8)
