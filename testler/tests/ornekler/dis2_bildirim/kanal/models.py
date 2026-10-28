from importlib import import_module

from django.db import models

from kanal import tasimalar

TASIMALAR = {"slack": "kanal.tasimalar.Slack", "eposta": "kanal.tasimalar.Eposta"}


class Kanal(models.Model):
    code = models.UUIDField()
    name = models.CharField(max_length=100)
    kind = models.CharField(max_length=20)
    adres = models.CharField(max_length=200)

    @property
    def tasima(self) -> tasimalar.Tasima:
        modul, sinif = TASIMALAR[self.kind].rsplit(".", 1)
        cls = getattr(import_module(modul), sinif)
        return cls(self)

    def bildirim_gonder(self, metin: str) -> str:
        try:
            self.tasima.bildirim_gonder(metin)
        except tasimalar.TasimaHatasi as e:
            return str(e)
        return ""
