# Repository Layout

```
soabox/
  backend/                Rust crate (library plus soabox binary)
    src/
      bin/soabox.rs       CLI entry point
      config.rs           the only place environment variables are read
      errors.rs           error types and response mapping
      http/               router, client IP, security headers, static UI
      api/                /api PowerDNS passthrough
      v1/                 /v1 UI API
      auth/               local sign-in, TOTP, OIDC, sessions
      authz/              route classification and permission decisions
      audit/              diffs, redaction, history
      keys/               API key generation and lookup
      pdns/               PowerDNS client and zone id handling
      store/              database pool, migrations, repositories
      cli/                CLI subcommands
    migrations/sqlite/    forward-only migrations
    migrations/postgres/
    tests/                integration and compatibility tests
    openapi.json          generated /v1 spec, committed
  frontend/               React app
    src/components/ui/    component layer on React Aria Components
    src/routes/           TanStack Router file routes
    src/api/              generated schema types and client
    e2e/                  Playwright tests
  deploy/                 production Compose example and PowerDNS config
  data/                   runtime state, bind mounted, never committed
  docs/                   this documentation
  compose.yaml            development stack
  Dockerfile              multi-stage image build
```

## Boundaries

- Only `config.rs` reads environment variables.
- Only `pdns/` talks to PowerDNS.
- Only `store/` runs SQL.
- `api/` and `v1/` never decide permissions themselves. They ask `authz/` and record through `audit/`.
- The frontend talks only to `/v1` and `/auth`. It never calls `/api`.

## Extension Points

- New PowerDNS endpoints: add a row to the classification table in `authz/`.
- New settings: add a key to the settings registry with a default and a validator.
