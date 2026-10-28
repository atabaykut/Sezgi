import requests

BASE = "http://127.0.0.1:3000"


def yorumlar(kitap_id: str):
    return requests.get(f"{BASE}/api/v1/kitaplar/{kitap_id}/detay")
