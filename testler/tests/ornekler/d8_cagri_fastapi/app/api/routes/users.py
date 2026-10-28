from fastapi import APIRouter, HTTPException

from app import crud
from app.api.deps import CurrentUser, SessionDep
from app.models import User, UserUpdate

router = APIRouter()


@router.patch("/{user_id}")
def update_user(session: SessionDep, current_user: CurrentUser, user_id: int, user_in: UserUpdate):
    db_user = session.get(User, user_id)
    if not db_user:
        raise HTTPException(status_code=404, detail="no user")
    existing_user = crud.get_user_by_email(session=session, email=user_in.email)
    if existing_user and existing_user.id != user_id:
        raise HTTPException(status_code=409, detail="User with this email already exists")
    return crud.update_user(session=session, db_user=db_user, email=user_in.email)
