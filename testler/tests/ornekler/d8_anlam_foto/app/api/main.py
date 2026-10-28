from fastapi import APIRouter

from app.api.routes import albums, login, photos, users

api_router = APIRouter()
api_router.include_router(login.router)
api_router.include_router(users.router)
api_router.include_router(albums.router)
api_router.include_router(photos.router)
