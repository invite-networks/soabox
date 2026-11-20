# Getting Started

## Prerequisites

- Docker Desktop with Compose v2
- Rust stable (edition 2024) with `cargo-nextest`, `cargo-llvm-cov`, `cargo-audit`, `cargo-deny`
- Node.js 22 or later with pnpm
- `pre-commit`

## First Run

```sh
git clone https://github.com/invite-networks/soabox.git
cd soabox
pre-commit install
cp .env.example .env          # edit values, see configuration.md
docker compose up --build     # SOAbox on http://localhost:8080
```

The development stack starts PowerDNS Authoritative 5 with a SQLite backend and SOAbox in `dev` mode. Data for both is bind mounted under `data/`. A one-shot `pdns-init` service creates the PowerDNS database on first start.

If ports 8080 or 8081 are taken on your machine, set `SOABOX_HOST_PORT` and `PDNS_API_HOST_PORT` in `.env`. The PowerDNS API port is published on localhost only, for the compatibility tests.

Create an API key and try the passthrough:

```sh
docker compose exec soabox soabox apikey create --role admin --description local
curl -H "X-API-Key: <key>" http://localhost:8080/api/v1/servers/localhost/zones
```

## Loading Demo Data

`scripts/seed-demo.py` loads sample forward, reverse, signed, and secondary zones through the PowerDNS-compatible API, so each change also appears in history. Every name and address comes from reserved documentation ranges.

```sh
docker compose exec soabox soabox account create platform
docker compose exec soabox soabox account create marketing
docker compose exec soabox soabox account create it
docker compose exec soabox soabox apikey create --role admin --description demo-seed
SOABOX_URL=http://localhost:8080 SOABOX_API_KEY=<key> uv run scripts/seed-demo.py
```

Executed on 2026-11-19 against the development stack. Delete the seed key afterward from API keys.

## Working On The Backend

```sh
cd backend
cargo run -- serve            # reads ../.env
cargo nextest run
```

## Working On The Frontend

```sh
cd frontend
pnpm install
pnpm dev                      # Vite on http://localhost:5173, proxies /api, /v1, /auth to :8080
```

## Creating The First Admin

Set `SOABOX_BOOTSTRAP_ADMIN_USERNAME` and `SOABOX_BOOTSTRAP_ADMIN_PASSWORD` before the first start, or run:

```sh
docker compose exec soabox soabox create-admin --username admin
```
