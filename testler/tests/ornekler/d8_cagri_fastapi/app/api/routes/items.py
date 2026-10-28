from fastapi import APIRouter, Depends, HTTPException

from app.api.deps import CurrentUser, SessionDep
from app.models import Item
from app.services.items import ItemService

router = APIRouter()


@router.delete("/{id}")
def delete_item(session: SessionDep, current_user: CurrentUser, id: int, service: ItemService = Depends(ItemService)):
    item = session.get(Item, id)
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    if item.owner_id != current_user.id:
        raise HTTPException(status_code=403, detail="Not enough permissions")
    service.remove(item)
    return {"ok": True}
