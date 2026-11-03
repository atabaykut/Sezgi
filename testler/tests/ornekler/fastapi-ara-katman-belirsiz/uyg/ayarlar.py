import os


class Ayarlar:
    anahtar = os.environ.get("ANAHTAR", "")


AYARLAR = Ayarlar()


def korunan_mu(request) -> bool:
    return request.method in os.environ.get("KORUNAN", "POST").split(",")
