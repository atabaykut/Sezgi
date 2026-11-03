from fastapi import FastAPI

app = FastAPI()


@app.post("/auth/logout")
def cikis():
    return {}


@app.get("/titles")
def basliklar():
    return []


@app.get("/{fname:path}")
def statik(fname: str):
    return ""
