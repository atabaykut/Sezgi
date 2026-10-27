"""Dosyalara bölünmüş bir OpenAPI belgesini tek JSON'a birleştirir (swagger-cli bundle'ın yaptığı gibi).

Başka dosyaya işaret eden `$ref`:
- kök belgede `components` altında tanımlı bir bileşene işaret ediyorsa belge içi işarete çevrilir
  (`#/components/schemas/Ad`; kendine işaret eden şemalar böylece sonsuz döngüye girmez);
- değilse o dosyanın (ya da içindeki parçanın) içeriğiyle değiştirilir.
Belge içi `#/...` işaretleri olduğu gibi kalır. Uygulama kodu okunmaz; yalnızca belge dosyaları.
Kullanım: python openapi-birlestir.py <kök openapi.yaml> <çıktı.json>
"""
import json
import os
import sys

import yaml

_onbellek = {}


def _yukle(yol):
    yol = os.path.normpath(yol)
    if yol not in _onbellek:
        with open(yol, encoding="utf-8") as f:
            _onbellek[yol] = yaml.safe_load(f)
    return _onbellek[yol]


def _parca(belge, isaret):
    d = belge
    for p in [x for x in isaret.split("/") if x]:
        d = d[p.replace("~1", "/").replace("~0", "~")]
    return d


def _anahtar(dosya, ref):
    hedef, _, isaret = ref.partition("#")
    return os.path.normpath(os.path.join(os.path.dirname(dosya), hedef)) + "#" + isaret


def _coz(dugum, dosya, bilesenler, yigin=()):
    if isinstance(dugum, dict):
        ref = dugum.get("$ref")
        if isinstance(ref, str) and not ref.startswith("#"):
            anahtar = _anahtar(dosya, ref)
            if anahtar in bilesenler:
                return {"$ref": bilesenler[anahtar]}
            if anahtar in yigin:
                return {"type": "object", "description": "(döngüsel $ref: " + ref + ")"}
            yol, _, isaret = anahtar.partition("#")
            return _coz(_parca(_yukle(yol), isaret), yol, bilesenler, yigin + (anahtar,))
        return {k: _coz(v, dosya, bilesenler, yigin) for k, v in dugum.items()}
    if isinstance(dugum, list):
        return [_coz(v, dosya, bilesenler, yigin) for v in dugum]
    return dugum


if __name__ == "__main__":
    kok = os.path.normpath(sys.argv[1])
    belge = _yukle(kok)
    # Kökte bileşen olarak adlandırılmış dosya parçaları: (dosya#işaret) -> #/components/<bölüm>/<ad>
    bilesenler = {}
    for bolum, ogeler in (belge.get("components") or {}).items():
        for ad, v in (ogeler or {}).items():
            if isinstance(v, dict) and isinstance(v.get("$ref"), str) and not v["$ref"].startswith("#"):
                bilesenler[_anahtar(kok, v["$ref"])] = f"#/components/{bolum}/{ad}"
    sonuc = {k: v for k, v in belge.items() if k != "components"}
    sonuc = _coz(sonuc, kok, bilesenler)
    # Bileşenlerin kendileri: kendi tanımları içeride çözülür, başka bileşenlere işaretler belge içi kalır
    sonuc["components"] = {}
    for bolum, ogeler in (belge.get("components") or {}).items():
        sonuc["components"][bolum] = {}
        for ad, v in (ogeler or {}).items():
            if isinstance(v, dict) and isinstance(v.get("$ref"), str) and not v["$ref"].startswith("#"):
                anahtar = _anahtar(kok, v["$ref"])
                yol, _, isaret = anahtar.partition("#")
                v = _coz(_parca(_yukle(yol), isaret), yol, bilesenler, (anahtar,))
            else:
                v = _coz(v, kok, bilesenler)
            sonuc["components"][bolum][ad] = v
    with open(sys.argv[2], "w", encoding="utf-8") as f:
        json.dump(sonuc, f, ensure_ascii=False)
    yontem = sum(1 for v in sonuc.get("paths", {}).values() for m in v if m in ("get", "post", "put", "patch", "delete"))
    print(f"{len(sonuc.get('paths', {}))} yol, {yontem} işlem")
