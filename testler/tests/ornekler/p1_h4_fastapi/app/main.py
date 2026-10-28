from fastapi import FastAPI
from fastapi.routing import APIRoute

from app.ayarlar import AYARLAR
from app.routes import ogeler, ozel


def kimlik_uret(route: APIRoute) -> str:
    return f"{route.tags[0]}-{route.name}"


app = FastAPI(generate_unique_id_function=kimlik_uret)
app.include_router(ogeler.router)
if AYARLAR.ortam == "yerel":
    app.include_router(ozel.router)
