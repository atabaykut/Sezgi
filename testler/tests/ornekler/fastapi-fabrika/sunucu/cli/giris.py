from fastapi import FastAPI

giris_api = FastAPI()


@giris_api.post("/basarili")
def basarili():
    return {}
