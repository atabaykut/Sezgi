from authlib.integrations.starlette_client import OAuth
from fastapi import Depends
from fastapi.security import OAuth2PasswordBearer
from pydantic import BaseModel

oauth2 = OAuth2PasswordBearer(tokenUrl="/login")
oauth = OAuth()


class User(BaseModel):
    id: int
    email: str


def get_db():
    yield None


def get_current_user(token: str = Depends(oauth2)) -> User:
    return User(id=1, email="a@b.c")
