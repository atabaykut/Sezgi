import uuid
from typing import Any

from fastapi import APIRouter, Depends

from app.api.deps import SessionDep, get_current_active_superuser
from app.models import User, UserRegister

router = APIRouter(prefix="/users", tags=["users"])


@router.post("/signup")
def register_user(session: SessionDep, user_in: UserRegister) -> Any:
    """
    Create a new user without logging in.
    """
    user = User(email=user_in.email)
    session.add(user)
    return user


@router.delete("/{user_id}", dependencies=[Depends(get_current_active_superuser)])
def delete_user(session: SessionDep, user_id: uuid.UUID) -> Any:
    """
    Delete a user.
    """
    user = session.get(User, user_id)
    session.delete(user)
    return {"ok": True}
