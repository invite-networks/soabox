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
docker compose up --build     # SOAbox on http://localhost:8080, PowerDNS API on the internal network
```

The development stack starts PowerDNS Authoritative 5 with a SQLite backend and SOAbox in `dev` mode. Data for both is bind mounted under `data/`.

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
