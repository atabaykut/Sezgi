from django.db import models


class KitapDurumu(models.TextChoices):
    TASLAK = "taslak", "Taslak"
    YAYINDA = "yayinda", "Yayında"


class Yazar(models.Model):
    ad = models.CharField(max_length=100)
    eposta = models.EmailField(unique=True)


class Kitap(models.Model):
    baslik = models.CharField(max_length=200)
    ozet = models.TextField(blank=True)
    sayfa = models.PositiveIntegerField(default=1)
    durum = models.CharField(max_length=10, choices=KitapDurumu.choices, default=KitapDurumu.TASLAK)
    yazar = models.ForeignKey(Yazar, on_delete=models.CASCADE, related_name="kitaplar")
    olusturma = models.DateTimeField(auto_now_add=True)

    class Meta:
        db_table = "kitap"


class Yorum(models.Model):
    kitap = models.ForeignKey("Kitap", on_delete=models.CASCADE)
    metin = models.TextField()
    puan = models.IntegerField()
