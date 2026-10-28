from fastapi import APIRouter

from app.api.routes import varliklar

api_router = APIRouter()
api_router.include_router(varliklar.router, tags=["depo"])
