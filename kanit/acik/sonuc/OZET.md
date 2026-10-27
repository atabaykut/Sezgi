# Açık kanıt sonuçları / Public benchmark results

immich: Sezgi 0.1.0 · 2026-10-21T09:33Z · MINGW64 x86_64
calcom: Sezgi 0.1.0 (önceki derleme) · 2026-10-14T07:52Z · MINGW64 x86_64 — 2026-10-21'de yeniden ölçülmedi: depo bu ölçümde
elde değildi / not re-measured on 2026-10-21: the repository was not available for that run

## immich

```
  İşlemler     306 · 276 giriş ister · 30 girişsiz · 268 rol kısıtlı
  Süre, ilk çalıştırma    172 ms · harita: sonuc\immich
  Süre, ikinci çalıştırma 186 ms · harita: sonuc\immich

Resmî OpenAPI ile karşılaştırma / vs. official OpenAPI:
Resmî dokümandaki işlem     279
Sezgi'nin bulduğu           279/279 (100.0%)
Sezgi'de olup dokümanda yok 16
Giriş durumu doğru          100.0%
Gövde alanı bulma           437/485 (90.1%)
Zorunluluk doğru            100.0%

Benlik Kartı şema doğrulaması / card schema validation:
✓ sonuc/immich/benlik.json (Taslak 0.1.1 şeması, benlik 0.2.0 doğrulayıcısı / Draft 0.1.1 schema, benlik 0.2.0 validator)
```

## calcom

```
  Kapılar      177 endpoint · 65 girdi şemalı · 18 rol kısıtlı · 28 herkese açık
  Veri bağı    141 / 177 endpoint bir tabloya bağlandı (1 şema dosyası)
  Süre, ilk çalıştırma    204 ms (keşif 113 · ayrıştırma 76 · adaptör 15)
  Süre, ikinci çalıştırma 212 ms (keşif 109 · ayrıştırma 84 · adaptör 17)

Resmî OpenAPI ile karşılaştırma / vs. official OpenAPI:
Resmî dokümandaki işlem     121
Sezgi'nin bulduğu           121/121 (100.0%)
Sezgi'de olup dokümanda yok 41
Giriş durumu doğru          99.2%
Gövde alanı bulma           250/250 (100.0%)
Zorunluluk doğru            100.0%

Benlik Kartı şema doğrulaması / card schema validation:
✓ sonuc/calcom/benlik.json
```

## calcom: veri bağları / data links (vs. type-checker referee)

```
Metot 175 · birebir aynı 175 (%100.0)
Bağ: doğru 341 · fazla 0 · eksik 0 · kesinlik %100.0 · duyarlılık %100.0
```

## Benlik Kartı imzası / signature interoperability

```
✓ bağımsız doğrulama geçti (canonicalize + noble, 267512 bayt)
```

