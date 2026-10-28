from fastapi import Depends, FastAPI, HTTPException, status
from fastapi.security import HTTPBearer

app = FastAPI()
bearer = HTTPBearer(auto_error=False)


def belirtecten(belirtec):
    if not belirtec:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED)
    return {"ad": "kisi"}


# Şema hata vermez, 401'i çağrılan yardımcı verir (Airflow get_user kalıbı)
def kullanici(bilgi=Depends(bearer)):
    return belirtecten(bilgi)


@app.delete("/isler/{is_id}")
def sil(is_id: str, k=Depends(kullanici)):
    return None
