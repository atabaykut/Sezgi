from fastapi import FastAPI

from app.routes import auth, dinamik, recipes

app = FastAPI()
app.include_router(recipes.router, prefix="/api")
app.include_router(dinamik.router, prefix="/api")
app.include_router(auth.router, prefix="/api")
