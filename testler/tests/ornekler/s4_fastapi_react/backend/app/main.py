from fastapi import FastAPI

from app.api.main import api_router

app = FastAPI(title="Ornek")
app.include_router(api_router, prefix="/api/v1")
