class SilmeKarisimi:
    """
    Silme görünümlerinin ortak davranışı
    """

    def delete(self, request, *args, **kwargs):
        return super().delete(request, *args, **kwargs)
