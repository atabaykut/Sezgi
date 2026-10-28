from sqlmodel import Session

from app.models import Item


class ItemService:
    def __init__(self, session: Session):
        self.session = session

    def remove(self, item: Item) -> None:
        self.session.delete(item)
        self.session.commit()
