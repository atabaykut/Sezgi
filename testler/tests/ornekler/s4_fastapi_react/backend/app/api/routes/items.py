import uuid

from fastapi import APIRouter
from pydantic import BaseModel

router = APIRouter(prefix="/items", tags=["items"])


class ItemCreate(BaseModel):
    title: str
    description: str | None = None


@router.get("/")
def read_items(skip: int = 0, limit: int = 100):
    """Retrieve items."""
    return []


@router.post("/")
def create_item(item_in: ItemCreate):
    """Create new item."""
    return item_in


@router.delete("/{id}")
def delete_item(id: uuid.UUID):
    """Delete an item."""
    return None
