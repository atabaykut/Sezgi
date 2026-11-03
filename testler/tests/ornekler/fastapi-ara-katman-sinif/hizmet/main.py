from fastapi import FastAPI, HTTPException, Request
from starlette.middleware.base import BaseHTTPMiddleware

ACIK_YOLLAR = {"/saglik", "/oturum/ac"}


class BelirtecAraKatmani(BaseHTTPMiddleware):
    async def dispatch(self, request: Request, call_next):
        if request.url.path in ACIK_YOLLAR:
            return await call_next(request)
        belirtec = request.headers.get("Authorization")
        if not belirtec or not belirtec.startswith("Bearer "):
            raise HTTPException(status_code=401, detail="belirtec yok")
        return await call_next(request)


app = FastAPI()
app.add_middleware(BelirtecAraKatmani)


@app.get("/saglik")
def saglik():
    return {"durum": "iyi"}


@app.post("/oturum/ac")
def oturum_ac():
    return {}


@app.get("/siparisler")
def siparisleri_listele():
    return []


@app.post("/siparisler")
def siparis_olustur():
    return {}
