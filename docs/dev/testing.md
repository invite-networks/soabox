# Testing

Test tiers and requirement coverage are defined in the [test plan](../spec/test-plan.md).

## Commands

**Planned, not yet executed** until milestone 1 records results.

```sh
# backend
cd backend
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run
cargo test --doc
cargo llvm-cov nextest --fail-under-lines 80
cargo audit
cargo deny check

# compatibility suite against real PowerDNS
docker compose up -d pdns
SOABOX_COMPAT_PDNS_URL=http://localhost:8081 SOABOX_COMPAT_PDNS_KEY=<dev key> cargo nextest run --test compat

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
