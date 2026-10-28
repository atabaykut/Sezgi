from fastapi import FastAPI

from app.api import siparis

app = FastAPI()
app.include_router(siparis.router)
