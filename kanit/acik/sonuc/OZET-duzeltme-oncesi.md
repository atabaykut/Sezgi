# Düzeltme turundan ÖNCEKİ kodla (temel durum 06af7d7, Sezgi 0.1.0 adayı) üretilen özet. Kayıt için saklanır;
# güncel sonuçlar OZET.md'de. Ayrıntı: ../../DUZELTME-NOTU-1.md §4

# Açık kanıt sonuçları / Public benchmark results

Sezgi 0.1.0 · 2026-10-13T23:38Z · MINGW64 x86_64

## immich

```
  Kapılar      306 endpoint · 127 girdi şemalı · 268 rol kısıtlı · 30 herkese açık
  Süre         1215 ms (keşif 43 · ayrıştırma 1164 · adaptör 8)

Resmî OpenAPI ile karşılaştırma / vs. official OpenAPI:
Resmî dokümandaki işlem     279
Sezgi'nin bulduğu           279/279 (100.0%)
Sezgi'de olup dokümanda yok 16
Giriş durumu doğru          100.0%
Gövde alanı bulma           436/485 (89.9%)
Zorunluluk doğru            100.0%

Benlik Kartı şema doğrulaması / card schema validation:
✓ sonuc/immich/benlik.json
```

## calcom

```
  Kapılar      177 endpoint · 65 girdi şemalı · 18 rol kısıtlı · 28 herkese açık
  Veri bağı    141 / 177 endpoint bir tabloya bağlandı (1 şema dosyası)
  Süre         1899 ms (keşif 211 · ayrıştırma 1660 · adaptör 27)

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
✓ bağımsız doğrulama geçti (canonicalize + noble, 265761 bayt)
```

