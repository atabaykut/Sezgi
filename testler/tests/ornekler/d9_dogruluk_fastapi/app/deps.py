from typing import Annotated

from fastapi import Depends
from fastapi.security import OAuth2PasswordBearer
from sqlmodel import Session

from app.models import User

oauth2 = OAuth2PasswordBearer(tokenUrl="/login")


def get_db():
    yield Session()


def get_current_user(token: Annotated[str, Depends(oauth2)]) -> User:
    return User(id=1, email="a@b.c")


SessionDep = Annotated[Session, Depends(get_db)]
CurrentUser = Annotated[User, Depends(get_current_user)]
