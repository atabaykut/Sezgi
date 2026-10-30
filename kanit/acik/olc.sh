#!/usr/bin/env bash
# Açık kanıt: Sezgi haritasını uygulamaların kendi resmî OpenAPI dosyalarıyla ve bağımsız hakemlerle karşılaştırır.
# Public benchmark: compares Sezgi's map with each app's official OpenAPI file and with independent referees.
#
#   ./getir.sh        # uygulamaları sabit commit'lerinde indir (bir kez)
#   npm ci --ignore-scripts   # hakem araçları (TypeScript derleyicisi, ajv), yalnızca bu klasöre
#   ./olc.sh          # ölç; sonuçlar sonuc/ klasörüne
set -euo pipefail
cd "$(dirname "$0")"
# Windows (Git Bash) için yerel yol: node /c/... biçimini anlamaz
yol() { (cd "$1" && (pwd -W 2>/dev/null || pwd)); }
ACIK=$(yol .)
KANIT=$(yol ..)
# Yayınlanan depoda kanit/ Sezgi'nin kökündedir (../..); geliştirme düzeninde yan klasördür (../../sezgi)
if [ -f ../../Cargo.toml ]; then SEZGI_KOK=$(yol ../..); else SEZGI_KOK=$(yol ../../sezgi); fi
# Benlik Card şeması: BENLIK_SEMA ortam değişkeni, yoksa yayınlanan depodaki benlik/ klasörü ya da yan klasördeki benlik-spec
SEMA="${BENLIK_SEMA:-}"
for aday in "$SEZGI_KOK/benlik/schema/benlik-card.schema.json" "$SEZGI_KOK/../benlik-spec/schema/benlik-card.schema.json" "$KANIT/../benlik-spec/schema/benlik-card.schema.json"; do
  [ -z "$SEMA" ] && [ -f "$aday" ] && SEMA="$aday"
done
# Git Bash "/api" gibi argümanları yola çevirmesin
export MSYS_NO_PATHCONV=1
[ -d depolar/immich ] || { echo "Önce ./getir.sh"; exit 1; }
[ -d node_modules/typescript ] || { echo "Önce: npm ci --ignore-scripts"; exit 1; }

echo "• Sezgi derleniyor"
(cd "$SEZGI_KOK" && cargo build --release --locked --quiet)
SEZGI="$SEZGI_KOK/target/release/sezgi"
[ -x "$SEZGI" ] || SEZGI="$SEZGI.exe"

mkdir -p sonuc
export TS_YOLU="$ACIK/node_modules/typescript"
export AJV_YOLU="$ACIK/node_modules"
RAPOR=sonuc/OZET.md
{
  echo "# Açık kanıt sonuçları / Public benchmark results"
  echo
  # İşletim sistemi ailesi ve mimari (sürüm/derleme numarası yazılmaz)
  echo "Sezgi $("$SEZGI" --version) · $(date -u +%Y-%m-%dT%H:%MZ) · $(uname -s | cut -d_ -f1 | cut -d- -f1) $(uname -m)"
  echo
} > "$RAPOR"

olc() { # ad uygulama-klasörü openapi [--onek x]
  local ad=$1 kok=$2 openapi=$3; shift 3
  echo "• $ad: harita"
  "$SEZGI" init "$kok" --cikti "sonuc/$ad" | tee "sonuc/$ad.init.txt"
  # İkinci çalıştırma: dosyalar artık işletim sisteminin önbelleğinde; iki süre ayrı ayrı kaydedilir
  "$SEZGI" init "$kok" --cikti "sonuc/$ad" > "sonuc/$ad.init2.txt"
  "$SEZGI" rapor "sonuc/$ad" > "sonuc/$ad.rapor.txt"
  {
    echo "## $ad"
    echo
    echo '```'
    grep -E "İşlemler|Tablolar" "sonuc/$ad.init.txt" || true
    sed -n 's/^  Süre  */  Süre, ilk çalıştırma    /p' "sonuc/$ad.init.txt"
    sed -n 's/^  Süre  */  Süre, ikinci çalıştırma /p' "sonuc/$ad.init2.txt"
    echo
    echo "Resmî OpenAPI ile karşılaştırma / vs. official OpenAPI:"
    # pipefail altında "| head" erken kapanan boru yüzünden betiği durdurabilir: önce dosyaya, sonra ilk satırlar
    node "$KANIT/karsilastir-openapi.js" "sonuc/$ad/kapilar.json" "$openapi" "$@" > "sonuc/$ad.karsilastirma.txt"
    sed -n 1,6p "sonuc/$ad.karsilastirma.txt"
    echo
    echo "Benlik Kartı şema doğrulaması / card schema validation:"
    if [ -n "$SEMA" ]; then node "$KANIT/kart-dogrula.js" "$SEMA" "sonuc/$ad/benlik.json"; else echo "(şema bulunamadı: BENLIK_SEMA=<benlik-card.schema.json> verin; depodaki benlik/schema/)"; fi
    echo '```'
    echo
  } >> "$RAPOR"
}

olc immich depolar/immich/server depolar/immich/open-api/immich-openapi-specs.json --onek /api
olc calcom depolar/cal.com/apps/api/v2 depolar/cal.com/docs/api-reference/v2/openapi.json

echo "• calcom: veri bağları (TypeScript tip denetleyicili bağımsız hakem)"
node "$KANIT/hakem/veri-hakem.js" depolar/cal.com/apps/api/v2 sonuc/calcom.veri-hakem.json
{
  echo "## calcom: veri bağları / data links (vs. type-checker referee)"
  echo
  echo '```'
  node "$KANIT/karsilastir-veri.js" sonuc/calcom.veri-hakem.json sonuc/calcom/kapilar.json > sonuc/calcom.veri-karsilastirma.txt
  sed -n 1,3p sonuc/calcom.veri-karsilastirma.txt
  echo '```'
  echo
} >> "$RAPOR"

echo "• imza: Rust ile imzala, bağımsız JS (RFC 8785 canonicalize + noble ML-DSA) ile doğrula"
GECICI=$(yol "$(mktemp -d)")
ANAHTAR_ORTAM=(env APPDATA="$GECICI" XDG_CONFIG_HOME="$GECICI")
"${ANAHTAR_ORTAM[@]}" "$SEZGI" anahtar olustur --ad olcum > /dev/null
"${ANAHTAR_ORTAM[@]}" "$SEZGI" init depolar/immich/server --cikti "$GECICI/harita" --imzala olcum > /dev/null
{
  echo "## Benlik Kartı imzası / signature interoperability"
  echo
  echo '```'
  node "$KANIT/imza-dogrula.mjs" "$GECICI/harita/benlik.json" "$GECICI/harita/benlik-keys.json"
  echo '```'
  echo
} >> "$RAPOR"
rm -rf "$GECICI"

echo
cat "$RAPOR"
