from typing import Annotated

from fastapi import Depends, HTTPException
from fastapi.security import OAuth2PasswordBearer
from sqlmodel import Session

from app.models import User

oauth2 = OAuth2PasswordBearer(tokenUrl="/api/v1/login/access-token")


def get_db():
    yield Session()


SessionDep = Annotated[Session, Depends(get_db)]


def get_current_user(session: SessionDep, token: str = Depends(oauth2)) -> User:
    user = session.get(User, token)
    if not user:
        raise HTTPException(status_code=404, detail="User not found")
    return user


CurrentUser = Annotated[User, Depends(get_current_user)]


def get_current_active_superuser(current_user: CurrentUser) -> User:
    if not current_user.is_superuser:
        raise HTTPException(status_code=403, detail="The user doesn't have enough privileges")
    return current_user
