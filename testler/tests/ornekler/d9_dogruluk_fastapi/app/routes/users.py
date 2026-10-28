from fastapi import APIRouter, HTTPException
from sqlmodel import col, delete

from app.deps import CurrentUser, SessionDep
from app.models import Item, User

router = APIRouter()


@router.delete("/{user_id}")
def delete_user(session: SessionDep, current_user: CurrentUser, user_id: int) -> dict:
    user = session.get(User, user_id)
    if not user:
        raise HTTPException(status_code=404, detail="User not found")
    statement = delete(Item).where(col(Item.owner_id) == user_id)
    session.exec(statement)
    session.delete(user)
    session.commit()
    return {"ok": True}
