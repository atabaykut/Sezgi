from fastapi import FastAPI

v2_app = FastAPI()


@v2_app.get("/saglik")
def saglik():
    return {"durum": "iyi"}
