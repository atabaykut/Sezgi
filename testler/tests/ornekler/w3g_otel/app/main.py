from fastapi import FastAPI

from app.core.config import settings
from app.routes import deneme, odalar

app = FastAPI(title="Otel")
app.include_router(odalar.router, prefix="/api")

if settings.ORTAM == "local":
    app.include_router(deneme.router, prefix="/api")
