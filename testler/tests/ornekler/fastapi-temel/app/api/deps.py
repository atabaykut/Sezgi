from typing import Annotated

from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPBearer, OAuth2PasswordBearer

from app.models import User

oauth2 = OAuth2PasswordBearer(tokenUrl="token")
opsiyonel = OAuth2PasswordBearer(tokenUrl="token", auto_error=False)


def get_current_user(token: str = Depends(oauth2)) -> User:
    return User(id=1)


def maybe_user(token: Annotated[str | None, Depends(opsiyonel)]) -> User | None:
    return None


CurrentUser = Annotated[User, Depends(get_current_user)]


class JWTBearer(HTTPBearer):
    pass


# Bağımlılık fabrikası: iç fonksiyon döndürür; zincir iç fonksiyonun parametrelerinden sürer
def rol_iste(rol: str):
    def denetle(user: CurrentUser) -> User:
        if rol not in user.roller:
            raise HTTPException(status_code=403, detail="Yetki yok")
        return user

    return denetle


# __call__'lu sınıfın örneği bağımlılık olarak
class RolDenetimi:
    def __init__(self, izinli: list[str]):
        self.izinli = izinli

    def __call__(self, user: User = Depends(get_current_user)) -> User:
        if not set(user.roller) & set(self.izinli):
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN)
        return user


# Güvenlik şemasına ulaşmayan fabrika: giriş istemez
def sayfa_boyu(varsayilan: int):
    def oku(boyut: int = varsayilan) -> int:
        return boyut

    return oku
