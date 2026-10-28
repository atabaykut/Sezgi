from fastapi import APIRouter
from pydantic import BaseModel

from app.base import BaseUserController
from app.controller import controller

router = APIRouter(prefix="/recipes")


class RecipeIn(BaseModel):
    name: str


@controller(router)
class RecipeController(BaseUserController):
    def _helper(self) -> None:
        pass

    @router.get("")
    def get_all(self, q: str | None = None):
        return []

    @router.post("", status_code=201)
    def create_one(self, data: RecipeIn) -> str:
        return data.name

    @router.delete("/{slug}")
    def delete_one(self, slug: str):
        return None
