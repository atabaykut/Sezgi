from typing import Annotated

from fastapi import Depends
from fastapi.security import OAuth2PasswordBearer

oauth2 = OAuth2PasswordBearer(tokenUrl="token")


def aktif_kullanici(token: Annotated[str, Depends(oauth2)]):
    return token


# Takma ad bu modülde; içindeki ad (aktif_kullanici) burada çözülmeli, kullanıldığı modülde değil
Kullanici = Annotated[str, Depends(aktif_kullanici)]
