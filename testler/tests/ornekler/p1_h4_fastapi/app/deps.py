from fastapi import Depends, HTTPException


class Kullanici:
    is_active: bool = True
    is_superuser: bool = False


def get_current_user() -> Kullanici:
    return Kullanici()


def get_current_active_superuser(current_user: Kullanici = Depends(get_current_user)) -> Kullanici:
    if not current_user.is_superuser:
        raise HTTPException(status_code=403, detail="The user doesn't have enough privileges")
    return current_user


from typing import Annotated

from sqlmodel import Session


def get_db():
    with Session() as session:
        yield session


SessionDep = Annotated[Session, Depends(get_db)]
