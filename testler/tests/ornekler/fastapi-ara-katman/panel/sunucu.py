"""Yerel panel: durum değiştiren her istek arayüz anahtarı ister (uygulama düzeyi ara katman)."""
import hmac
import secrets

from fastapi import FastAPI, Request
from fastapi.responses import PlainTextResponse

from panel import oturum

_ANAHTAR = secrets.token_urlsafe(32)
ANAHTAR_BASLIGI = "x-panel-token"
_IMZALI_YOLLAR = frozenset({"/hooks/odeme"})
_IMZALI_ONEKLER = ("/rutin/webhook/",)


def _imzali(yol: str) -> bool:
    return yol in _IMZALI_YOLLAR or yol.startswith(_IMZALI_ONEKLER)


def _anahtar_dogru(deger: str | None) -> bool:
    return bool(deger) and hmac.compare_digest(str(deger), _ANAHTAR)


def _host_dogru(host: str | None) -> bool:
    return host in ("127.0.0.1", "localhost")


def uygulama_kur():
    app = FastAPI(title="Panel")

    @app.middleware("http")
    async def _yerel_koruma(request: Request, call_next):
        if request.url.path.startswith("/web/"):
            return await oturum.koruma(request, call_next)
        if not _host_dogru(request.headers.get("host")):
            return PlainTextResponse("gecersiz host", status_code=403)
        if request.method not in ("GET", "HEAD", "OPTIONS"):
            if not request.url.path.startswith("/v1/") and not _imzali(request.url.path) \
                    and not _anahtar_dogru(request.headers.get(ANAHTAR_BASLIGI)):
                return PlainTextResponse("yetkisiz istek", status_code=403)
        return await call_next(request)

    @app.middleware("http")
    async def _guvenlik_basliklari(request: Request, call_next):
        yanit = await call_next(request)
        yanit.headers.setdefault("X-Frame-Options", "SAMEORIGIN")
        return yanit

    @app.get("/api/kayitlar")
    async def kayitlari_listele():
        return []

    @app.post("/api/kayitlar")
    async def kayit_ekle(request: Request):
        return {}

    @app.delete("/api/kayitlar/{kayit_id}")
    async def kayit_sil(kayit_id: str):
        return {}

    @app.post("/v1/sohbet")
    async def sohbet(request: Request):
        return {}

    @app.post("/hooks/odeme")
    async def odeme_bildirimi(request: Request):
        return {}

    @app.post("/rutin/webhook/{rutin_id}")
    async def rutin_tetikle(rutin_id: str):
        return {}

    @app.post("/web/giris")
    async def web_giris(request: Request):
        return {}

    @app.post("/{alan}/yukle")
    async def alana_yukle(alan: str):
        return {}

    @app.get("/{dosya:path}")
    async def statik(dosya: str):
        return PlainTextResponse("")

    return app
