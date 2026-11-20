# Software Design

Status: Draft for readiness review. Applies to v0.x.

Related: [Requirements](requirements.md) | [Test Plan](test-plan.md) | [Decisions](../adr/README.md) | [Diagrams](../uml/README.md)

## 1. Overview

SOAbox is one Rust service that serves three things: a PowerDNS-compatible API under `/api`, its own API under `/v1` for the browser UI, and the built React UI itself. Both APIs call one core that handles authorization, audit, and the connection to PowerDNS. See the [context diagram](../uml/context.md) and [component diagram](../uml/components.md).

Design goals:

- A client of `/api` cannot tell SOAbox from PowerDNS 5.x (FR-API-01 to FR-API-05).
- There is no path to PowerDNS that skips authorization and audit (FR-RBAC-05).
- The UI is generic and unbranded (NFR-UX-01).
- One container, behind a reverse proxy, with state only in `data/` or Postgres.

## 2. Components

| Component | Responsibility | Requirements |
|---|---|---|
| HTTP layer | Axum router, client IP resolution, security headers, request limits, tracing spans. | NFR-SEC-03, NFR-SEC-05 |
| Passthrough (`/api`) | Key authentication, route classification, scope enforcement, list filtering, forwarding. | FR-API-*, FR-KEY-*, FR-SCP-* |
| UI API (`/v1`) | Typed JSON endpoints for the UI, sessions, CSRF, OpenAPI via `utoipa`. The contract is [v1-api.md](v1-api.md). | FR-UI-*, FR-RBAC-* |
| Auth (`/auth`) | Local sign-in, TOTP, OIDC flow, sessions. | FR-AUTH-* |
| Authorization core | Decides whether an actor may perform an operation on a zone or server resource. | FR-RBAC-*, FR-SCP-* |
| Audit core | Before and after reads, diff, history storage, redaction. | FR-AUD-* |
| PowerDNS client | Forwarding and typed calls with the SOAbox PowerDNS key. | FR-API-*, FR-KEY-01 |
| Store | SQLite or Postgres access, migrations. | NFR-OPS-07, NFR-OPS-08 |
| Static UI | Serves the built frontend with an `index.html` fallback. | FR-UI-* |
| CLI | `serve`, `create-admin`, `apikey`, `account`, `history`. | FR-CLI-01 |

## 3. Interfaces

### 3.1 Route groups

| Prefix | Auth | Purpose |
|---|---|---|
| `/api` | `X-API-Key` (SOAbox key) | PowerDNS-compatible API. |
| `/v1` | Session cookie and CSRF header | UI API. Standard error shape `{detail, code, status}`. |
| `/auth` | None, then session | Sign-in, OIDC redirect and callback, sign-out. |
| `/healthz`, `/readyz`, `/version` | None | Health and version. |
| `/v1/openapi.json`, `/v1/docs` | Session | OpenAPI and Redoc, in `dev` mode only. |
| `/*` | None | Built UI, fingerprinted assets cached long, `index.html` not cached. |

`/v1` routes are registered before the static fallback and return a JSON `404` for unknown paths.

### 3.2 Passthrough compatibility contract

- Paths, methods, bodies, and status codes follow PowerDNS Authoritative 5.x.
- Bodies are forwarded byte for byte, except filtered list responses (re-serialized) and scoped zone creation (`account` set).
- `Content-Type` and `Server` are copied from upstream. Hop-by-hop headers are dropped both ways.
- SOAbox errors use `{"error": "..."}` and the status PowerDNS would use.
- Zone ids are normalized the way PowerDNS does: trailing dot, escaped characters such as `=2F`, and view variants such as `example.com..internal`.

### 3.3 Route classification

| Class | Examples | Admin | Operator | Scoped |
|---|---|---|---|---|
| Discovery | `GET /api`, `/api/v1`, `/api/v1/servers`, `/api/v1/servers/{id}` | Yes | Yes | Yes |
| Zone list | `GET .../zones` | Yes | Yes | Filtered |
| Zone create | `POST .../zones` | Yes | Yes | If allowed (FR-KEY-06) |
| Zone resource | anything under `.../zones/{zone_id}` | Yes | Yes | In scope only |
| Search | `GET .../search-data` | Yes | Yes | Filtered |
| Server read | `GET .../statistics`, `GET .../config` | Yes | Yes | No |
| Server write | tsigkeys, autoprimaries, cache flush, views, networks, config writes | Yes | No | No |
| Unknown | any other path | Forwarded | No | No |

The same core serves UI requests, with User role permissions taken from settings (FR-RBAC-03).

Additional rules applied to writes on both `/api` and `/v1`, added after the pre-release security audit:

- Paths are split into segments before authorization. Empty, `.`, and `..` segments (plain or percent-encoded), backslashes, and encoded slashes are refused with the PowerDNS `404`, and only the checked segments are forwarded. The PowerDNS client also refuses any path that URL normalization would change.
- A key never exceeds its owner. Its level is capped at the owner's current role, a key owned by a User only reaches the owner's current zones and follows the User settings (delete zones, DNSSEC, allowed record types, zone creation), and keys of a disabled owner stop working.
- Metadata kinds that run code, make PowerDNS contact other hosts, or forward updates (for example `LUA-AXFR-SCRIPT`, `AXFR-SOURCE`, `ALSO-NOTIFY`, `FORWARD-DNSUPDATE`) are Admin only. Other roles may set the kinds in the allowlist in `authz.rs`.
- Secondary zones and primaries can be set only by Admins and Operators, because primaries make PowerDNS connect to other hosts.
- `LUA` records are Admin only unless the record type lists in settings allow them.
- A limited principal cannot create a zone equal to, above, or below a zone outside its scope.
- Refused writes are recorded in history with outcome `failure`.

### 3.4 Key handling

SOAbox always replaces the client's key ([ADR-0004](../adr/0004-api-key-swap.md)). See the [passthrough write sequence](../uml/sequence-passthrough-write.md).

- Keys are 32 random bytes from the OS CSPRNG, encoded base64url, with no recognizable prefix, so a key reveals nothing about SOAbox.
- Storage is the SHA-256 digest of the key, indexed for lookup. Keys are high-entropy, so a fast hash is safe and keeps per-request cost low. Passwords, which are low-entropy, use argon2id.
- The UI shows the last four characters as a hint.
- Expiry and allowed networks are checked against the resolved client IP.

## 4. Data Model

See the [entity relationship diagram](../uml/data-model.md). Tables arrive by milestone.

| Table | Milestone | Purpose |
|---|---|---|
| `accounts` | 1 | Named groups of zones and users. |
| `account_zones` | 1 | Zones owned by an account. |
| `api_keys` | 1 | Key hash, hint, description, role, `create_account_id`, `expires_at`, `last_used_at`, `disabled`, `owner_user_id` (from milestone 2). |
| `api_key_zones`, `api_key_accounts` | 1 | Scope of a scoped key. |
| `api_key_networks` | 1 | Allowed client CIDRs. |
| `history` | 1 | Audit rows: time, actor type and id, actor name, client IP, operation, zone, outcome, upstream status, diff, detail. |
| `settings` | 1 | Key and value pairs for Admin settings. |
| `users` | 2 | Username, display name, email, role, auth source, OIDC issuer and subject, password hash, encrypted TOTP secret, status. |
| `sessions` | 2 | Session id hash, user, CSRF token, created, last seen, expiry, client IP, user agent. |
| `account_users`, `user_zones` | 2 | Access assignments. |
| `templates`, `template_records` | 4 | Zone templates. |

Zone names are stored in PowerDNS canonical form with a trailing dot. SOAbox never copies zone contents. PowerDNS stays the source of truth for DNS data.

### Settings

| Key | Default | Requirement |
|---|---|---|
| `local_login_with_oidc` | `allow` | FR-AUTH-05 |
| `oidc_new_user` | `create` (`create` or `approve`) | FR-AUTH-04 |
| `user_can_create_zones` | `false` | FR-RBAC-03 |
| `user_can_delete_zones` | `false` | FR-RBAC-03 |
| `user_can_view_history` | `true` | FR-RBAC-03 |
| `user_can_manage_dnssec` | `false` | FR-RBAC-03 |
| `user_can_create_api_keys` | `false` | FR-RBAC-03 |
| `record_types_forward`, `record_types_reverse` | common types | FR-RBAC-04 |
| `auto_ptr` | `false` | FR-UI-04 |

## 5. Authentication Design

- Local: argon2id password hashes, optional TOTP with secrets encrypted with XChaCha20-Poly1305 under `SOABOX_SECRET_KEY`, bound to their purpose as associated data. A TOTP code is accepted at most once (the last accepted time step is stored). Turning TOTP off needs the password and a code. Changing the password ends the user's other sessions. Password and code checks after sign-in are rate limited per account.
- OIDC: backend-run authorization code flow with PKCE, `state`, and `nonce`. Single issuer. For Entra, the issuer is the tenant-specific v2.0 endpoint. Users are matched on the `iss` and `sub` claims, which Entra keeps stable for a user within one app registration. See the [sign-in sequence](../uml/sequence-oidc-sign-in.md).
- Sessions: a random 32-byte id in the cookie, SHA-256 hashed in the database. `HttpOnly`, `Secure`, `SameSite=Lax`. Rotated on sign-in and role change. Idle and absolute timeouts from configuration.
- CSRF: per-session token sent in a header on state-changing `/v1` calls.
- Rate limits: failed key attempts per client IP; sign-in failures per client IP, per client IP and username pair, and a higher limit per username across all addresses, so one attacker cannot quickly lock an account out. In memory per process, returning `429`. Behind a proxy, `SOABOX_TRUSTED_PROXIES` must be set or all clients share one limit; SOAbox warns at startup in `prod` when it is empty.
- In `prod`, `SOABOX_PUBLIC_URL` must be `https`, the secret key must look random, and `.env` files are not loaded.

## 6. Audit Design

- Writes are classified before forwarding. For a PATCH, SOAbox reads the RRsets named in the request. For PUT or DELETE on a zone, it reads the zone.
- After a successful write, it reads the same data again and stores a diff of `{name, type, before, after}` entries plus zone-level changes.
- Failed writes store the upstream status and error without a diff.
- Server writes store the request body with `key` and `privatekey` fields and similar secrets replaced by `[redacted]`.
- A failure to store history is logged at `ERROR` and does not change the client's response, because the change already happened in PowerDNS ([ADR-0009](../adr/0009-audit-model.md)).

## 7. Security Boundaries

See the [trust boundary diagram](../uml/trust-boundaries.md).

| Boundary | Control |
|---|---|
| Internet to reverse proxy | TLS, handled by the proxy. |
| Proxy to SOAbox | `X-Forwarded-*` trusted only from `SOABOX_TRUSTED_PROXIES`. |
| API client to SOAbox | SOAbox key, role, scope, expiry, networks, rate limit. |
| Browser to SOAbox | Session cookie, CSRF, CSP with same-origin assets only, security headers. |
| SOAbox to identity provider | TLS, PKCE, state, nonce, issuer and audience validation. |
| SOAbox to PowerDNS | Private network. SOAbox's own PowerDNS key. PowerDNS API not exposed to anyone else. |
| SOAbox to database | Credentials in configuration. Parameterized queries only. |

## 8. Configuration

All variables use the `SOABOX_` prefix ([ADR-0011](../adr/0011-configuration-and-runtime-mode.md)). Any secret variable can instead be given as a file path with a `_FILE` suffix, for Docker and Kubernetes secrets. The full list is in the [configuration reference](../dev/configuration.md).

## 9. Deployment

See the [deployment diagram](../uml/deployment.md).

- One container image built from a multi-stage Dockerfile. Distroless runtime, `USER 1001:1001`, version string from the crate version and short git SHA.
- The image contains the binary and the built UI.
- Persistent state in `data/` (SQLite database) or in Postgres.
- `compose.yaml` at the repository root for development starts SOAbox and PowerDNS 5 with bind mounts under `data/`.
- An example production Compose file with Postgres lives in `deploy/`.
- Release images are published to GitHub Container Registry by digest.

## 10. Failure Handling

| Failure | Behavior |
|---|---|
| PowerDNS unreachable or slow | `/api` returns `502` with a PowerDNS-shaped body after the configured timeout. `/v1` returns `502` with code `UPSTREAM_UNAVAILABLE`. `/readyz` fails. |
| Database unreachable | Requests that need it return `503`. `/readyz` fails. |
| Identity provider unreachable | OIDC sign-in shows an error. Local sign-in still works for allowed accounts. Existing sessions continue. |
| History write fails | Logged at `ERROR`. Response unchanged. |
| Invalid configuration | Startup stops with a message naming the variable. |
| Migration fails | Startup stops. The database is left as the migration tool leaves it. Restore from backup per the [upgrade runbook](../runbooks/upgrade.md). |

## 11. Dependencies

| Area | Choice |
|---|---|
| Backend | Rust 2024, `tokio`, `axum`, `tower-http`, `reqwest` with `rustls`, `sqlx` (SQLite and Postgres), `openidconnect`, `argon2`, `sha2`, `totp-rs`, an AEAD from a vetted crate, `utoipa`, `tracing`, `thiserror`, `anyhow`, `envy`, `dotenvy`, `clap` |
| Frontend | React 19, Vite, TypeScript strict, TanStack Router, Query, and Table, React Aria Components, Tailwind CSS 4, React Hook Form, Zod, `openapi-fetch`, `lucide-react`, self-hosted Inter and JetBrains Mono |
| Tests | `cargo-nextest`, `wiremock`, `rstest`, Vitest, Testing Library, MSW, Playwright, axe |
| Upstream | PowerDNS Authoritative 5.x |

## 12. Visual Design

Generic and unbranded ([ADR-0010](../adr/0010-generic-ui-theme.md)): a slate gray scale with one blue accent, semantic status colors always paired with text or an icon, light and dark mode, Inter and JetBrains Mono self-hosted. A fixed left sidebar, a top bar with zone search and the user menu, and dense tables in cards. Each destination appears once in navigation. Theme colors are CSS custom properties in one file.
