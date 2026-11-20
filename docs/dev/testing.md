# Testing

Test tiers and requirement coverage are defined in the [test plan](../spec/test-plan.md).

## Commands

Backend commands were executed for milestone 1 on 2026-11-19 (see the [milestone 1 results](../plans/milestone-1-passthrough.md#results)). Frontend commands are **Planned, not yet executed** until milestone 3.

```sh
# backend
cd backend
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run
cargo test --doc
cargo llvm-cov nextest --test-threads 1 --fail-under-lines 80
cargo audit
cargo deny check

# compatibility suite against real PowerDNS
docker compose up -d pdns
SOABOX_COMPAT_PDNS_URL=http://localhost:8081 SOABOX_COMPAT_PDNS_KEY=<PDNS_AUTH_API_KEY> \
  cargo nextest run --test compat --test-threads 1

# Postgres integration
SOABOX_TEST_POSTGRES_URL=postgres://... cargo nextest run

# frontend
cd frontend
pnpm biome check .
pnpm typecheck
pnpm test:coverage
pnpm build
pnpm test:e2e
pnpm audit --prod
```

## CI

GitHub Actions runs on every pull request and on `main`:

1. Backend format, lint, tests with coverage gate, doc tests.
2. Compatibility suite with PowerDNS 5 as a service container.
3. Postgres integration tests with Postgres as a service container.
4. Frontend lint, types, tests with coverage, build, OpenAPI drift check.
5. End-to-end tests against the built image.
6. Supply chain: `cargo audit`, `cargo deny`, `pnpm audit --prod`.
7. Container build.
