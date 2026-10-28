from kanal import eposta
from ortak import curl


class TasimaHatasi(Exception):
    pass


class Tasima:
    def __init__(self, kanal):
        self.kanal = kanal

    def bildirim_gonder(self, metin: str) -> None:
        raise NotImplementedError()


class HttpTasima(Tasima):
    def gonder(self, url: str, veri: dict) -> None:
        yanit = curl.istek("post", url, veri)
        if yanit.durum >= 400:
            raise TasimaHatasi("gönderilemedi")


class Slack(HttpTasima):
    def bildirim_gonder(self, metin: str) -> None:
        self.gonder(self.kanal.adres, {"text": metin})


class Eposta(Tasima):
    def bildirim_gonder(self, metin: str) -> None:
        eposta.uyari(self.kanal.adres, metin)
