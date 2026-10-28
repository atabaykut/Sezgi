#!/usr/bin/env bash
# Sezgi'nin test katmanını çalıştırır: ürün kaynağının GEÇİCİ bir kopyasına testleri ekler ve orada `cargo test`
# çalıştırır; depoya hiçbir dosya yazmaz.
#   testler/tests/               bütünleşik testler ve örnek uygulamalar (tests/ornekler)
#   testler/src-ekleri/<ad>.rs   src/<ad>.rs için birim testleri: kopyada dosyanın sonuna #[path] ile bağlanır
#   testler/src-ekleri/ozellik_testleri.rs   özellik (proptest) testleri, main.rs'e #[cfg(test)] modül olarak eklenir
#   testler/dev-bagimliliklar.toml           kopyadaki Cargo.toml'a eklenir
#
# Kullanım: bash testler/calistir.sh [cargo test argümanları...]   ör. bash testler/calistir.sh --test d10_harita_bagi
# Gerekenler: rust-toolchain.toml'daki Rust sürümü; `script` (util-linux; sahte terminal testleri, yalnızca Linux'ta
# çalışır) ve başsız bir Chrome/Chromium (Ayna tarayıcı testleri: SEZGI_TARAYICI ya da bilinen kurulum yolları; bulunamazsa
# bu testler başarısız olur). Testlerin tamamı Linux'ta koşar; CI da Linux'ta çalıştırır.
# Ortam: SEZGI_KOMUT (varsayılan test; ör. clippy), CARGO_TARGET_DIR (varsayılan <depo>/target/testler).
set -euo pipefail
KOK="$(cd "$(dirname "$0")/.." && pwd)"
KATMAN="$KOK/testler"
GECICI="$(mktemp -d)"
trap 'rm -rf "$GECICI"' EXIT
# Ürün kaynağı (derleme çıktısı, git ve test katmanı hariç)
(cd "$KOK" && tar --exclude=./target --exclude=./.git --exclude=./testler -cf - .) | (cd "$GECICI" && tar -xf -)
cp -r "$KATMAN/tests" "$GECICI/tests"
[ -d "$KATMAN/proptest-regressions" ] && cp -r "$KATMAN/proptest-regressions" "$GECICI/"
for ek in "$KATMAN"/src-ekleri/*.rs; do
  ad="$(basename "$ek" .rs)"
  if [ "$ad" = ozellik_testleri ]; then
    cp "$ek" "$GECICI/src/ozellik_testleri.rs"
    printf '\n#[cfg(test)]\nmod ozellik_testleri;\n' >> "$GECICI/src/main.rs"
  else
    [ -f "$GECICI/src/$ad.rs" ] || { echo "! src/$ad.rs yok; src-ekleri/$ad.rs atlandı" >&2; continue; }
    cp "$ek" "$GECICI/src/masa_testleri_$ad.rs"
    printf '\n#[cfg(test)]\n#[path = "masa_testleri_%s.rs"]\nmod testler;\n' "$ad" >> "$GECICI/src/$ad.rs"
  fi
done
printf '\n' >> "$GECICI/Cargo.toml"
cat "$KATMAN/dev-bagimliliklar.toml" >> "$GECICI/Cargo.toml"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$KOK/target/testler}"
# Testler işletim sistemi anahtarlığına yazmaz: oturum ve kayıt anahtarı test klasörlerindeki dosyalarda tutulur
export SEZGI_ANAHTARLIK=kapali
cd "$GECICI"
cargo "${SEZGI_KOMUT:-test}" "$@"
