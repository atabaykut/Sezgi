from typing import Annotated

from fastapi import Depends, FastAPI
from fastapi.security import OAuth2PasswordBearer

app = FastAPI()
istege_bagli = OAuth2PasswordBearer(tokenUrl="token", auto_error=False)


# auto_error=False: oturumsuz istek reddedilmez, token None gelir
@app.get("/vitrin")
def vitrin(token: Annotated[str | None, Depends(istege_bagli)]):
    return []
