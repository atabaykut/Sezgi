from fastapi import FastAPI

import sunucu.api as api

API_ROUTERLARI = (
    api.siparis.router,
    api.saglik.router,
    api.gorsel.router,
)


def api_uygulamasi_kur(saglik_yolu: str = "/canli") -> FastAPI:
    api_app = FastAPI(title="Fabrika API")
    for router in API_ROUTERLARI:
        api_app.include_router(router)

    # Yol fabrikanın parametresinden (varsayılanı)
    @api_app.get(saglik_yolu)
    def canli():
        return {"ok": True}

    return api_app
