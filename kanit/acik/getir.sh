#!/usr/bin/env bash
# Ölçülen açık kaynak uygulamaları SABİT commit'lerinde, yalnızca gereken klasörlerle indirir.
# İndirilen kod yalnızca OKUNUR: bağımlılık kurulmaz, betik çalıştırılmaz, derlenmez.
# (Klasör/cone modu: listelenen klasörler ve üst klasörlerindeki dosyalar, ör. server/package.json)
# Fetches the measured open-source apps at pinned commits (sparse). The code is only read, never installed or run.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p depolar

getir() { # ad url commit klasörler...
  local ad=$1 url=$2 commit=$3; shift 3
  if [ -d "depolar/$ad/.git" ]; then
    # Var olan klon sabit commit'te olmalı: eski ya da değiştirilmiş bir klon sessizce ölçülmesin
    local head
    head="$(git -C "depolar/$ad" rev-parse HEAD 2>/dev/null || true)"
    if [ "$head" != "$commit" ]; then
      echo "✗ depolar/$ad sabit commit'te değil (${head:-HEAD yok} ≠ $commit); klasörü silip yeniden çalıştırın" >&2
      exit 1
    fi
    if [ -n "$(git -C "depolar/$ad" status --porcelain 2>/dev/null)" ]; then
      echo "✗ depolar/$ad içinde değiştirilmiş dosya var; klasörü silip yeniden çalıştırın" >&2
      exit 1
    fi
    echo "• $ad zaten var (${commit:0:12})"
    return
  fi
  echo "• $ad @ ${commit:0:12}"
  git clone --quiet --filter=blob:none --no-checkout --sparse "$url" "depolar/$ad"
  git -C "depolar/$ad" sparse-checkout set "$@"
  git -C "depolar/$ad" -c advice.detachedHead=false checkout --quiet "$commit"
}

getir immich https://github.com/immich-app/immich.git aec0dd1e6ba44d172b83cf6f3422211961aff99d \
  server/src open-api

getir cal.com https://github.com/calcom/cal.com.git 54343aa685ae8f33159d2f485ec4a57bad5c574a \
  apps/api/v2/src apps/api/v2/swagger docs/api-reference/v2 packages/platform/constants packages/platform/enums \
  packages/platform/libraries packages/platform/types packages/platform/utils packages/prisma

echo "✓ depolar hazır"
