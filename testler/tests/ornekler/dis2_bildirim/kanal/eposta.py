from threading import Thread

from django.conf import settings
from django.core.mail import EmailMultiAlternatives as Mesaj
from django.template.loader import render_to_string as cizim


class PostaIsi(Thread):
    def __init__(self, mesaj: Mesaj) -> None:
        Thread.__init__(self)
        self.mesaj = mesaj

    def run(self) -> None:
        self.mesaj.send()


def gonder(mesaj: Mesaj) -> None:
    PostaIsi(mesaj).start()


def hazirla(ad: str, alici: str, ctx: dict) -> Mesaj:
    govde = cizim(f"posta/{ad}.txt", ctx)
    return Mesaj("Izleme", govde, settings.DEFAULT_FROM_EMAIL, [alici])


def giris_baglantisi(alici: str) -> None:
    gonder(hazirla("giris", alici, {}))


def uyari(alici: str, metin: str) -> None:
    gonder(hazirla("uyari", alici, {"metin": metin}))
