# Milestone 1: Passthrough Core

Goal: SOAbox runs in front of PowerDNS Authoritative 5.x and serves the full pdns HTTP API under `/api`. API keys are issued by SOAbox and scoped by role, zone, and account. Every write is audited. A compatibility suite shows a client cannot tell SOAbox from pdns.

No UI in this milestone. API keys and accounts are managed with the `soabox` CLI until milestone 4.

See [design](../design.md) for the overall architecture.

## Exit Criteria

- `docker compose up` in `deploy/` starts pdns 5 and SOAbox, and `pdnsutil`-equivalent workflows work through SOAbox with a SOAbox key.
- The compatibility suite passes against a real pdns 5 instance.
- Scoped keys only see and change their own zones, and out-of-scope zones look like missing zones.
- Every successful and failed zone write has a history row with an RRset diff.
- CI is green: fmt, clippy (`-D warnings`), nextest, doc tests, coverage at or above 80%, `cargo audit`, `cargo deny`.

## Task 1: Repository Scaffolding

- `git init`, `.gitignore`, `LICENSE` (Apache-2.0), `NOTICE` ("Copyright SOAbox contributors"), `README.md` with project summary and quick start.
- `backend/` crate `soabox`, edition 2024, library plus `src/bin/soabox.rs`. Layout: `config.rs`, `errors.rs`, `db/`, `pdns/`, `auth/`, `authz/`, `audit/`, `api/` (passthrough), `cli/`.
- `rust-toolchain.toml` (stable), `rustfmt.toml`, clippy lints in `Cargo.toml`, `deny.toml` (license allowlist compatible with Apache-2.0).
- `.env.example` with every `SOABOX_` variable.
- `deploy/compose.yaml`: pdns-auth 5.x (SQLite backend, API enabled, tag pinned) and SOAbox, bind mounts under `deploy/data/`.
- GitHub Actions workflow: fmt, clippy, nextest, doc tests, coverage, audit, deny, and the compat suite with pdns as a service container.

## Task 2: Config, Errors, Logging, Client IP

- `Config` loaded with `envy` (`SOABOX_` prefix) and `dotenvy` in development. Fails fast with a clear message on missing or invalid values.
- Two error renderers:
  - pdns shape for `/api`: `{"error": "..."}` with the status pdns would return.
  - SOAbox shape for `/v1` (used from milestone 2): `{"detail", "code", "status"}`.
- `tracing` with JSON output in production and pretty output in development. Request spans carry method, path, status, latency, client IP, and actor. API keys and bodies are never logged.
- Client IP middleware: honor `X-Forwarded-For` and `X-Forwarded-Proto` only when the peer is in `SOABOX_TRUSTED_PROXIES`. Unit tests for spoofing attempts.
- Request body size limit and upstream timeouts (connect and total) from config.

## Task 3: Database And Migrations

- `sqlx` with SQLite (default) and Postgres. Queries are written in portable SQL. Migrations live in `migrations/sqlite/` and `migrations/postgres/` and run at startup.
- Tables for this milestone:
  - `accounts` (id, name, description, created_at)
  - `account_zones` (account_id, zone_name)
  - `api_keys` (id, prefix, hash, description, role, can_create_zones, account_id, expires_at, last_used_at, created_at, disabled)
  - `api_key_zones` (api_key_id, zone_name)
  - `api_key_networks` (api_key_id, cidr)
  - `history` (id, at, actor_type, actor_id, actor_name, client_ip, operation, zone_name, status, upstream_status, diff_json, detail_json)
- Users, sessions, and settings tables arrive in milestone 2. `history.actor_type` already allows `user` and `api_key`.
- Repository layer behind traits so services can be tested with fakes. Integration tests run against both SQLite and Postgres in CI.

## Task 4: pdns Client

- `reqwest` with `rustls`, connection pooling, timeouts, and the real API key from config.
- `forward(request)`: sends method, path, query, filtered headers, and body unchanged, and returns status, headers, and body bytes. Hop-by-hop headers are stripped in both directions.
- Typed helpers used by the authorization and audit layers: list zones, get zone (with and without rrsets), get rrsets for a name and type.
- Zone id handling matches pdns: trailing dot, escaped characters (for example `=2F`), and view variants (`zone..variant`). One normalization function, covered by table tests.
- Wiremock tests for timeouts, upstream errors, and header handling.

## Task 5: API Keys

- Key format: random 32-byte secret, base64url, with a short public prefix used for lookup. Stored as an argon2id hash. Constant-time verification.
- Roles: `admin`, `operator`, `scoped`. Scoped keys carry zones, accounts, and an optional `can_create_zones` flag tied to an account.
- Optional expiry and CIDR allowlist checked against the resolved client IP.
- `last_used_at` updated at most once a minute per key to avoid write load.
- Failed authentication is rate limited per client IP. Responses match pdns (`401` with pdns body).
- CLI: `soabox apikey create|list|disable|delete`, `soabox account create|list|add-zone|remove-zone`. The secret is printed once on creation.

## Task 6: Route Classification And Authorization

- A route table maps `(method, path pattern)` to an operation class and the zone id position:
  - Discovery: `/api`, `/api/v1`, `/api/v1/servers`, `/api/v1/servers/{id}`
  - Zone list: `GET /zones`
  - Zone create: `POST /zones`
  - Zone read and write: everything under `/zones/{zone_id}` (rrsets PATCH, PUT, DELETE, notify, axfr-retrieve, export, rectify, cryptokeys, metadata)
  - Search: `GET /search-data`
  - Server admin: config, statistics, cache flush, tsigkeys, autoprimaries, views, networks
- Policy:
  - `admin`: everything, including paths not in the table, so new pdns endpoints keep working.
  - `operator`: all zone classes, read-only server statistics and config, no tsigkey, autoprimary, or cache writes.
  - `scoped`: zone classes for zones in scope only. Zone create only with `can_create_zones`. No server admin classes.
  - Non-admin keys are denied on unknown paths.
- `server_id` other than the configured one returns pdns's `404` for an unknown server.
- Out-of-scope zones return the same `404` body pdns returns for a missing zone.
- Exhaustive table tests for every class and role.

## Task 7: Passthrough Handler

- One handler mounted on `/api` and `/api/*`. Flow: resolve key, classify, authorize, then one of:
  - **Plain forward** for reads in scope.
  - **Filtered forward** for `GET /zones` and `GET /search-data`: parse the JSON array, drop out-of-scope entries, re-serialize, fix `Content-Length`. Query parameters (`zone=`, `dnssec=`, `q=`, `max=`, `object_type=`) are passed through unchanged.
  - **Audited forward** for writes (Task 8).
  - **Zone create**: for scoped keys, set the `account` field to the key's account before forwarding, and add the zone to the account on success.
- Response status, body, `Content-Type`, and `Server` copied from upstream.

## Task 8: Audit

- Before a zone write, fetch the affected RRsets: for PATCH, the names and types in the request; for PUT and DELETE, the zone. After a successful call, fetch again and store the diff.
- Diff format: list of `{name, type, before: {ttl, records}, after: {ttl, records}}`, plus zone-level changes (kind, primaries, account, DNSSEC flags).
- Failed writes are recorded with the upstream status and error, and no diff.
- Server admin writes record the request body with secrets redacted (TSIG `key`, cryptokey `privatekey`).
- Audit failures are logged at error level and do not change the client's response, since the write already happened upstream.
- `soabox history list` CLI for this milestone.

## Task 9: Compatibility Suite

- Location: `tests/compat/`, run with nextest when `SOABOX_COMPAT_PDNS_URL` is set (CI provides pdns as a service).
- Each case runs the same request against pdns directly with the real key and against SOAbox with an admin key, then compares status, `Content-Type`, and the parsed JSON body.
- Write cases use twin zones (`a-<case>.test.` direct, `b-<case>.test.` through SOAbox) and compare the resulting zones after normalizing the zone name.
- Coverage: discovery endpoints, zone CRUD for each kind, rrset PATCH (replace, delete, extend, prune where supported), comments, notify, export, rectify, cryptokeys, metadata, tsigkeys, search, statistics, config, error cases (bad JSON, missing zone, bad key, wrong server id).
- Scope cases with a scoped key: filtered lists, `404` for out-of-scope zones matching pdns's missing-zone body byte-for-byte.
- Client smoke tests: run a short script with an existing pdns client library against SOAbox (for example the PowerDNS Python client or octoDNS) to catch anything the JSON comparisons miss.

## Task 10: Hardening For The Milestone

- Security headers on all responses that pdns would not reject.
- Graceful shutdown, `/healthz` (process) and `/readyz` (database and pdns reachable) outside `/api`.
- Container image: multi-stage build, non-root user, minimal runtime base.
- Run the security and dependency audits and fix findings before closing the milestone.

## Order Of Work

1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10. Tasks 4 and 5 can run in parallel after 3. The compat suite harness (Task 9) starts as soon as Task 7 forwards plain requests, so it can guide Tasks 7 and 8.

## Later Milestones (Outline)

2. **Auth**: users, sessions, local login with TOTP, OIDC with Entra guide, `local_login_with_oidc` setting, `/v1` API skeleton with OpenAPI and CSRF.
3. **UI foundation**: React app, theme tokens with light and dark mode, component layer, shell, sign-in, zone list, record editor with pending-change diff.
4. **Administration**: users, accounts, roles, API keys, settings, templates, history views. CLI stays for automation.
5. **DNSSEC, metadata, TSIG, server views.**
6. **Hardening and release**: security and dependency audits, Playwright with axe, release image, docs, first public tag.
