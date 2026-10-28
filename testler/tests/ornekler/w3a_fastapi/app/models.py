import uuid

from sqlmodel import Field, SQLModel


class Varlik(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)


class Etiket(SQLModel, table=True):
    id: uuid.UUID = Field(default_factory=uuid.uuid4, primary_key=True)
    varlik_id: uuid.UUID = Field(foreign_key="varlik.id")
