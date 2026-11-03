from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from uyg.ayarlar import AYARLAR, korunan_mu

app = FastAPI()


@app.middleware("http")
async def koruma(request: Request, call_next):
    if korunan_mu(request) and request.headers.get("x-api-key") != AYARLAR.anahtar:
        return JSONResponse({"hata": "yetkisiz"}, status_code=401)
    return await call_next(request)


@app.get("/notlar")
def notlari_listele():
    return []


@app.post("/notlar")
def not_ekle():
    return {}
