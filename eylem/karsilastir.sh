#!/usr/bin/env bash
# Sezgi karşılaştırma eylemi (action.yml): taban commit'in ve PR'ın haritasını kurar, `sezgi karsilastir --sert` ile
# karşılaştırır. Bozan değişiklikte (kaldırılan işlem, yeni zorunlu girdi, daralan değerler, sıkılaşan sınırlar, yeni
# giriş zorunluluğu, kısıtlanan roller) `sert` açıksa çıkış kodu 1 olur. Özet adım özetine ve PR'a tek bir yorum olarak
# yazılır; yorum sonraki çalıştırmalarda güncellenir. Depoya ve uygulama klasörüne hiçbir şey yazılmaz.
#
# Girdiler ortam değişkenleriyle gelir (iş akışı ifadeleri kabuk koduna gömülmez):
#   SEZGI_UYGULAMA  uygulamanın depo köküne göre klasörü (varsayılan .)
#   SEZGI_TABAN     karşılaştırılacak eski commit (PR'da taban commit)
#   SEZGI_SERT      true: bozan değişiklikte iş başarısız olur (varsayılan true)
#   SEZGI_YORUM     true: PR'a yorum yazılır; SEZGI_PR, GH_TOKEN ve GITHUB_REPOSITORY gerekir
#   SEZGI_IKILI     kurulu sezgi ikilisinin yolu; boşsa npm'den SEZGI_SURUM sürümü çalıştırılır
# Çıkış: 0 geçti, 1 bozan değişiklik (sert), 2 karşılaştırılamadı. Adım çıktısı: kirilma=true|false
set -euo pipefail

uyg="${SEZGI_UYGULAMA:-.}"
taban="${SEZGI_TABAN:-}"
isaret='<!-- sezgi-karsilastir -->'

if [ -z "$taban" ]; then
  echo "::error::Karşılaştırılacak taban commit yok: eylemi pull_request olayında çalıştırın ya da 'taban' girdisini verin"
  exit 2
fi
if [ -n "${SEZGI_IKILI:-}" ]; then
  sezgi=("$SEZGI_IKILI")
else
  sezgi=(npx --yes "sezgi@${SEZGI_SURUM:-0.1.0}")
fi

gecici="$(mktemp -d)"
temizle() {
  git worktree remove --force "$gecici/taban" >/dev/null 2>&1 || true
  git worktree prune >/dev/null 2>&1 || true
  rm -rf "$gecici"
}
trap temizle EXIT

cikti_yaz() { if [ -n "${GITHUB_OUTPUT:-}" ]; then echo "$1" >> "$GITHUB_OUTPUT"; fi; }
ozet_yaz() { if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then cat "$1" >> "$GITHUB_STEP_SUMMARY"; fi; }
# Koddan gelen metin kod bloğunun dışına taşmasın
blok() { echo '```'; sed 's/```/` ` `/g' "$1"; echo '```'; }

# Taban commit sığ klonda yoksa yalnızca o commit getirilir
if ! git cat-file -e "${taban}^{commit}" 2>/dev/null; then
  git fetch --no-tags --depth=1 origin "$taban"
fi
git worktree add --detach --quiet "$gecici/taban" "$taban"

if ! "${sezgi[@]}" init "$gecici/taban/$uyg" --cikti "$gecici/eski" > "$gecici/eski.log" 2>&1; then
  {
    echo "$isaret"
    echo "### Sezgi: karşılaştırma yapılmadı"
    echo
    echo "Taban commit'te (\`${taban:0:12}\`) \`$uyg\` için harita kurulamadı (yeni uygulama ya da desteklenmeyen çatı)."
    echo
    tail -n 20 "$gecici/eski.log" > "$gecici/eski.son"
    blok "$gecici/eski.son"
  } > "$gecici/yorum.md"
  cat "$gecici/yorum.md"
  ozet_yaz "$gecici/yorum.md"
  cikti_yaz "kirilma=false"
  exit 0
fi
if ! "${sezgi[@]}" init "$uyg" --cikti "$gecici/yeni" > "$gecici/yeni.log" 2>&1; then
  cat "$gecici/yeni.log"
  echo "::error::Bu PR'ın haritası kurulamadı: $uyg"
  exit 2
fi

set +e
"${sezgi[@]}" karsilastir "$gecici/eski" "$gecici/yeni" --sert > "$gecici/fark.txt" 2>&1
durum=$?
set -e
# Taban harita git worktree'den kurulur; Sezgi worktree'nin .git dosyasını izlemediği için kartta kaynak "unknown"
# görünür: başlıkta taban commit'in kendisi yazılsın
sed -i "1s|(unknown) →|(${taban:0:12}) →|" "$gecici/fark.txt"
case "$durum" in
  0) kirilma=false ;;
  1) kirilma=true ;;
  *)
    cat "$gecici/fark.txt"
    echo "::error::Haritalar karşılaştırılamadı (çıkış $durum)"
    exit 2
    ;;
esac

{
  echo "$isaret"
  if [ "$kirilma" = true ]; then
    echo "### Sezgi: bu PR bağlı yapay zekâları ve entegrasyonları bozabilir"
  else
    echo "### Sezgi: bozan değişiklik yok"
  fi
  echo
  echo "Taban \`${taban:0:12}\` → bu PR · uygulama \`$uyg\`"
  echo
  blok "$gecici/fark.txt"
} > "$gecici/yorum.md"
cat "$gecici/yorum.md"
ozet_yaz "$gecici/yorum.md"
cikti_yaz "kirilma=$kirilma"

if [ "${SEZGI_YORUM:-true}" = true ] && [ -n "${SEZGI_PR:-}" ] && [ -n "${GH_TOKEN:-}" ] && [ -n "${GITHUB_REPOSITORY:-}" ]; then
  yol="repos/$GITHUB_REPOSITORY/issues"
  var="$(gh api "$yol/$SEZGI_PR/comments" --paginate --jq '.[] | select(.body | contains("<!-- sezgi-karsilastir -->")) | .id' 2>/dev/null | head -n 1 || true)"
  if [ -n "$var" ]; then
    gh api --method PATCH "$yol/comments/$var" -F "body=@$gecici/yorum.md" >/dev/null || yazilamadi=1
  else
    gh api --method POST "$yol/$SEZGI_PR/comments" -F "body=@$gecici/yorum.md" >/dev/null || yazilamadi=1
  fi
  if [ -n "${yazilamadi:-}" ]; then
    echo "::warning::PR yorumu yazılamadı: iş akışına 'pull-requests: write' izni verin (çatallanmış depodan gelen PR'larda belirteç salt okunurdur)"
  fi
fi

if [ "$kirilma" = true ] && [ "${SEZGI_SERT:-true}" = true ]; then
  echo "::error::Bozan değişiklik var (sezgi karsilastir --sert). Bilinçliyse 'sert: false' ile yalnızca raporlayın."
  exit 1
fi
