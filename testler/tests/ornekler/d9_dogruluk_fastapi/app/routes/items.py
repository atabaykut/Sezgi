from fastapi import APIRouter, HTTPException
from sqlmodel import select

from app.deps import CurrentUser, SessionDep
from app.models import Item

router = APIRouter()


@router.get("/")
def read_items(session: SessionDep, current_user: CurrentUser) -> list[Item]:
    return list(session.exec(select(Item).where(Item.owner_id == current_user.id)).all())


@router.delete("/{id}")
def delete_item(session: SessionDep, current_user: CurrentUser, id: int) -> dict:
    item = session.get(Item, id)
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    if item.owner_id != current_user.id:
        raise HTTPException(status_code=403, detail="Not enough permissions")
    session.delete(item)
    session.commit()
    return {"ok": True}
