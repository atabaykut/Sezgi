"""requests benzeri, PycURL üzerine ince bir katman."""

import json
from io import BytesIO

import pycurl


class Yanit:
    def __init__(self, durum: int, icerik: bytes) -> None:
        self.durum = durum
        self.icerik = icerik


def istek(yontem: str, url: str, veri: dict) -> Yanit:
    tampon = BytesIO()
    c = pycurl.Curl()
    c.setopt(pycurl.URL, url.encode())
    c.setopt(pycurl.POSTFIELDS, json.dumps(veri))
    c.setopt(pycurl.WRITEDATA, tampon)
    c.perform()
    durum = c.getinfo(pycurl.RESPONSE_CODE)
    c.close()
    return Yanit(durum, tampon.getvalue())
