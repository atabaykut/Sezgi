from django.db import models


class Proje(models.Model):
    ad = models.CharField(max_length=100)


class Kontrol(models.Model):
    proje = models.ForeignKey(Proje, on_delete=models.CASCADE)
    durum = models.CharField(max_length=20)

    def duraklat(self) -> None:
        self.durum = "paused"
        self.save()


class Profil(models.Model):
    eposta = models.CharField(max_length=200)
    belirtec = models.CharField(max_length=64)

    def baglanti_gonder(self) -> None:
        self.belirtec = "x"
        self.save()
