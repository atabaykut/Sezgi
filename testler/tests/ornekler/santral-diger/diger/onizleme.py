import requests


def simge(taban: str):
    # Kendi statik dosyası: catch-all rotası karşılar
    return requests.get("/favicon.ico")
