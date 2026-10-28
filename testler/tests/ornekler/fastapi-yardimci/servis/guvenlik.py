# Airflow kalıbı: şemalar auto_error=False; belirteç yoksa 401'i şemayı kullanan fonksiyonun çağırdığı yardımcı fırlatır
from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer, OAuth2PasswordBearer

oauth2_semasi = OAuth2PasswordBearer(tokenUrl="/giris", auto_error=False)
bearer_semasi = HTTPBearer(auto_error=False)


def belirtecten_kullanici(belirtec: str | None) -> dict:
    if not belirtec:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Not authenticated")
    return {"ad": "kisi"}


def kullanici_al(
    _oauth: str | None = Depends(oauth2_semasi),
    bilgi: HTTPAuthorizationCredentials | None = Depends(bearer_semasi),
) -> dict:
    return belirtecten_kullanici(bilgi.credentials if bilgi else None)


# Gerçekten isteğe bağlı: belirteç yoksa None döner, hata yok
def varsa_kullanici(bilgi: HTTPAuthorizationCredentials | None = Depends(bearer_semasi)) -> dict | None:
    return {"ad": "kisi"} if bilgi else None
