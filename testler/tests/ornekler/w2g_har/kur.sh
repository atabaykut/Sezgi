#!/usr/bin/env bash
# Test 1 yeniden üretimi: fastapi-template konteynerde, Playwright HAR (giriş → /items → Add Item → Save → sil)
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
log() { echo "== $(date +%T) $*"; }
log apt
apt-get update -qq >/dev/null && apt-get install -y -qq postgresql >/dev/null
pg_ctlcluster 15 main start
su postgres -c "psql -q -c \"CREATE USER app WITH PASSWORD 'app-vt-1';\" -c 'CREATE DATABASE app OWNER app;'"
log kopya
mkdir -p /is && cd /kaynak && tar --exclude=node_modules --exclude=.git -cf - . | (mkdir -p /is/app && cd /is/app && tar -xf -)
log uv
pip install -q --break-system-packages uv
cd /is/app && uv python install 3.14 -q && uv sync --all-packages --frozen -q
export DATABASE_URL=postgresql://app:app-vt-1@localhost:5432/app SECRET_KEY=w2g-test-anahtari-0123456789 PROJECT_NAME=T1 FIRST_SUPERUSER=admin@example.com FIRST_SUPERUSER_PASSWORD="$T1_PAROLA"
cd /is/app/backend && uv run alembic upgrade head && uv run python app/initial_data.py
log on-yuz
cd /is/app && npm install --no-audit --no-fund --loglevel=error >/dev/null && cd frontend && npx vite build --logLevel error
log sunucu
cd /is/app/backend && (uv run fastapi run app/main.py --port 8000 > /is/sunucu.log 2>&1 &)
for i in $(seq 1 60); do curl -sf http://localhost:8000/api/v1/utils/health-check/ >/dev/null && break; sleep 1; done
log tarayici
cd /opt/rea && node node_modules/playwright-core/cli.js install --with-deps chromium-headless-shell >/dev/null 2>&1 || node node_modules/playwright-core/cli.js install chromium-headless-shell
log har
node /betik/har_kaydet.js
psql "postgresql://app:app-vt-1@localhost:5432/app" -At -c "select count(*) from item" > /cikti/db_son.txt
log bitti
