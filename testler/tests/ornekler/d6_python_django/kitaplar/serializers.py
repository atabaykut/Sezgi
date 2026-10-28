from rest_framework import serializers

from .models import Kitap, Yazar


class YazarSerializer(serializers.ModelSerializer):
    class Meta:
        model = Yazar
        fields = ["id", "ad", "eposta"]


class KitapSerializer(serializers.ModelSerializer):
    yazar = YazarSerializer(read_only=True)
    yazar_id = serializers.IntegerField(write_only=True)
    etiketler = serializers.ListField(child=serializers.CharField(), required=False)

    class Meta:
        model = Kitap
        fields = ["id", "baslik", "ozet", "sayfa", "durum", "yazar", "yazar_id", "etiketler", "olusturma"]
        read_only_fields = ["olusturma"]
        extra_kwargs = {"sayfa": {"min_value": 1, "max_value": 5000}}

    def validate_baslik(self, deger):
        if deger.lower() == "isimsiz":
            raise serializers.ValidationError("başlık gerekli")
        return deger


class YorumSerializer(serializers.Serializer):
    metin = serializers.CharField(max_length=500, min_length=2)
    puan = serializers.IntegerField(min_value=1, max_value=5)
    kaynak = serializers.ChoiceField(choices=["web", "mobil"], required=False)
