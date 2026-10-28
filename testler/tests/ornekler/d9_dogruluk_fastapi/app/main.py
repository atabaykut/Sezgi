from fastapi import FastAPI

from app.routes import hesap, items, users

app = FastAPI()
app.include_router(hesap.router)
app.include_router(items.router, prefix="/items")
app.include_router(users.router, prefix="/users")
