from typing import Annotated

from fastapi import Depends, Header, HTTPException
from fastapi.security import OAuth2PasswordBearer
from sqlmodel import Session

from app.models import ApiKey, User

oauth2 = OAuth2PasswordBearer(tokenUrl="/api/v1/login/access-token")


def get_db():
    yield Session()


SessionDep = Annotated[Session, Depends(get_db)]


def get_current_user(session: SessionDep, token: str = Depends(oauth2)) -> User:
    return session.get(User, token)


CurrentUser = Annotated[User, Depends(get_current_user)]


def require_api_key(session: SessionDep, x_api_key: str = Header(default="")) -> ApiKey:
    key = session.get(ApiKey, x_api_key)
    if key is None:
        raise HTTPException(status_code=401, detail="invalid api key")
    return key
