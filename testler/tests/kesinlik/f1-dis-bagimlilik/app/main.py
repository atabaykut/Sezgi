from fastapi import Depends, FastAPI
from kurum_kimlik import kimlik_dogrula

app = FastAPI()


# Bağımlılık bir kütüphaneden: kimlik isteyip istemediği koddan izlenemez
@app.get("/raporlar", dependencies=[Depends(kimlik_dogrula)])
def raporlar():
    return []
