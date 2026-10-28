from django.db.models.signals import post_delete, post_save
from django.dispatch import receiver

from .models import Kitap, Yorum


@receiver(post_save, sender=Kitap)
def kitap_kaydedildi(sender, instance, created, **kwargs):
    if created:
        Yorum.objects.filter(kitap=instance).delete()


def yorum_silindi(sender, instance, **kwargs):
    pass


post_delete.connect(yorum_silindi, sender=Yorum)
