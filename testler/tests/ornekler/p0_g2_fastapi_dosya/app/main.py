from fastapi import FastAPI, File, UploadFile

app = FastAPI()


@app.post("/yukle")
async def yukle(dosya: UploadFile):
    return {"ad": dosya.filename}


@app.post("/ham")
async def ham(icerik: bytes = File(...)):
    return {"boy": len(icerik)}
