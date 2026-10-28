from typing import Annotated

from fastapi import Depends, HTTPException
from sqlmodel import Session

from app.models import User


def get_db():
    yield Session()


SessionDep = Annotated[Session, Depends(get_db)]


def get_current_user(session: SessionDep, token: str = "") -> User:
    user = session.get(User, token)
    if not user:
        raise HTTPException(status_code=404, detail="User not found")
    return user


CurrentUser = Annotated[User, Depends(get_current_user)]
