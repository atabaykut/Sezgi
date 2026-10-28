from fastapi import APIRouter

from app.api.routes import accounts, budgets, chats, imports, transactions

api_router = APIRouter()
api_router.include_router(chats.router)
api_router.include_router(accounts.router)
api_router.include_router(transactions.router)
api_router.include_router(budgets.router)
api_router.include_router(imports.router)
