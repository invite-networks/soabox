#!/usr/bin/env bash
# Runs the Playwright suite against the real stack (SOAbox image + PowerDNS).
# Requires .env with PDNS_AUTH_API_KEY, SOABOX_PDNS_API_KEY, SOABOX_SECRET_KEY,
# SOABOX_BOOTSTRAP_ADMIN_USERNAME and SOABOX_BOOTSTRAP_ADMIN_PASSWORD.
set -euo pipefail
cd "$(dirname "$0")/.."
set -a
# shellcheck disable=SC1091
. ./.env
set +a
port="${SOABOX_HOST_PORT:-8080}"
mkdir -p data/pdns data/soabox
docker compose up -d --build
for _ in $(seq 1 60); do
  if curl -fsS "http://localhost:${port}/readyz" >/dev/null 2>&1; then break; fi
  sleep 2
done
curl -fsS "http://localhost:${port}/readyz"
echo
cd frontend
pnpm exec playwright install --with-deps chromium >/dev/null
SOABOX_E2E_BASE_URL="http://localhost:${port}" \
SOABOX_E2E_USERNAME="${SOABOX_BOOTSTRAP_ADMIN_USERNAME}" \
SOABOX_E2E_PASSWORD="${SOABOX_BOOTSTRAP_ADMIN_PASSWORD}" \
  pnpm test:e2e
