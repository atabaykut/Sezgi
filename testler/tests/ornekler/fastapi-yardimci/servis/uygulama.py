from fastapi import FastAPI


def gorunumleri_kur(app: FastAPI) -> None:
    """Router'lar yardımcı fonksiyonda, fonksiyon içi içe aktarmayla bağlanır (Airflow init_views kalıbı)."""
    from servis.rotalar import genel_router

    app.include_router(genel_router)


def uygulama_kur() -> FastAPI:
    app = FastAPI(title="Yardımcı")
    gorunumleri_kur(app)
    return app
