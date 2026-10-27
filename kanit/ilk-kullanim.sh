#!/usr/bin/env bash
# Sıfırdan kullanıcı senaryosu (PROMPT-BAGLAM-KULLANILABILIRLIK K8, kabul ölçütü 8): Sezgi'yi hiç bilmeyen birinin
# ilk dakikaları. Boş bir klasörde ve örnek bir uygulamada sırayla: argümansız sezgi, init, durum, bağlantı (deneme),
# rapor, Ayna, canlı mod kurulumu (etkileşimsiz), canlı mod durumu. Her adımın başarıyla bittiği ve çıktısında iç
# terim kalmadığı denetlenir. Kullanıcının gerçek ayarlarına dokunmaz (HOME geçici).
# Kullanım: SEZGI=<sezgi ikilisi> bash kanit/ilk-kullanim.sh <NestJS / FastAPI / Express uygulamasının klasörü>
set -uo pipefail
SEZGI="${SEZGI:-sezgi}"
IS="$(mktemp -d)"
trap 'kill "${AYNA:-0}" 2>/dev/null; rm -rf "$IS"' EXIT
export HOME="$IS/ev" XDG_CONFIG_HOME="$IS/ev/ayar" APPDATA="$IS/ev/ayar" SEZGI_ANAHTARLIK=kapali
mkdir -p "$HOME" "$IS/bos"
UYG="${1:?uygulama klasörü gerekli}"
cp -r "$UYG" "$IS/app"
YASAK='kapı|Kapı|KAPI|adaptör|herkese_acik|istege_bagli|varsayim|endpoint'
gecen=0; kalan=0
adim() { # ad klasör beklenen-metin komut...
  local ad=$1 klasor=$2 bek=$3; shift 3
  local cikti kod
  cikti="$(cd "$klasor" && "$@" 2>&1)"; kod=$?
  local sorun=""
  [ $kod -ne 0 ] && sorun="çıkış kodu $kod"
  [ -n "$bek" ] && ! grep -q -- "$bek" <<<"$cikti" && sorun="${sorun:+$sorun; }'$bek' yok"
  local terim; terim="$(grep -oE "$YASAK" <<<"$cikti" | sort -u | tr '\n' ' ')"
  [ -n "$terim" ] && sorun="${sorun:+$sorun; }iç terim: $terim"
  if [ -z "$sorun" ]; then
    echo "✓ $ad ($(wc -l <<<"$cikti") satır)"; gecen=$((gecen + 1))
  else
    echo "✗ $ad: $sorun"; sed 's/^/    /' <<<"$cikti" | head -20; kalan=$((kalan + 1))
  fi
}
adim "1. argümansız, boş klasör" "$IS/bos" "sezgi init" "$SEZGI"
adim "2. init" "$IS/app" "Sonraki adım" "$SEZGI" init
adim "3. argümansız, haritalı klasör" "$IS/app" "işlem" "$SEZGI"
adim "4. bağlan (deneme)" "$IS/app" "" "$SEZGI" bagla --deneme --arac claude-code
adim "5. rapor" "$IS/app" "İŞLEMLER" "$SEZGI" rapor
# 6. Ayna: boş bir portta açılır, sayfa gelir
(cd "$IS/app" && "$SEZGI" ayna --port 0 >"$IS/ayna.txt" 2>&1) & AYNA=$!
adres=""
for _ in $(seq 1 50); do
  adres="$(grep -oE 'http://127\.0\.0\.1:[0-9]+' "$IS/ayna.txt" | head -1)"
  [ -n "$adres" ] && break
  sleep 0.1
done
if [ -n "$adres" ] && curl -fsS "$adres/" | grep -qi "<html"; then
  echo "✓ 6. Ayna ($adres)"; gecen=$((gecen + 1))
else
  echo "✗ 6. Ayna açılmadı"; sed 's/^/    /' "$IS/ayna.txt" | head; kalan=$((kalan + 1))
fi
kill "$AYNA" 2>/dev/null
adim "7. canlı mod kurulumu" "$IS/app" "szg_" "$SEZGI" canli kur --hedef http://127.0.0.1:9 --test-kopyasi --mod okuma
adim "8. canlı mod durumu" "$IS/app" "1 token" "$SEZGI" canli
echo "$gecen geçti, $kalan kaldı"
[ "$kalan" -eq 0 ]
