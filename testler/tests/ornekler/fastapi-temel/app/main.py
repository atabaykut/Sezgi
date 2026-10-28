from fastapi import FastAPI

from app.api.main import api_router
from app.core.config import settings
from app.v2 import v2_app

app = FastAPI(title="Kitapçı")
app.include_router(api_router, prefix=settings.API_PREFIX)
app.mount("/v2", v2_app)
