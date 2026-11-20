# Milestones 2 To 5: Backend

Scope: the backend parts of milestones 2 to 5, built against the [UI API contract](../spec/v1-api.md). The browser UI for milestones 3 and 4 is delivered separately.

See the [design](../spec/design.md) and [requirements](../spec/requirements.md). Back to the [README](../../README.md).

## Delivered

- **Milestone 2, sign-in:** users, sessions (hashed ids, idle and absolute timeouts, rotation on sign-in, revoked on role change or disable), CSRF tokens, local sign-in with argon2id and optional TOTP (secrets encrypted with XChaCha20-Poly1305), OIDC authorization code flow with PKCE run by the backend, the `local_login_with_oidc` and `oidc_new_user` settings, bootstrap Admin, `create-admin` and `rotate-secrets` commands, and sign-in rate limits.
- **Milestone 4, administration API:** users, accounts (kept in step with the PowerDNS `account` field), API keys (users with the setting get scoped keys limited to their own zones), templates, settings, and history with filters.
- **Milestone 5, DNSSEC and server API:** DNSSEC on and off, cryptokeys, metadata, TSIG keys (key material shown only on the detail view and redacted in history), and server configuration and statistics.
- **Shared:** zone list, create (with templates and accounts), update, delete, record changes with the allowed record type lists, automatic PTR records, notify, rectify, AXFR retrieve, export, and zone history. The built UI is served with a deep-link fallback, the OpenAPI document is generated with `utoipa`, committed as `backend/openapi.json`, and checked for drift in CI.

## Results

Executed on 2026-11-19 on macOS (arm64) with PowerDNS Authoritative 5.1.5, Postgres 18, and `mock-oauth2-server` 2.1.10.

| Check | Result |
|---|---|
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | Pass |
| All backend tests, including compatibility, Postgres, OIDC, and the UI API scenario against real PowerDNS | 160 passed |
| Line coverage | 92.4% |

Changes from the design found during implementation:

- Users signing in with OIDC are matched on `iss` and `sub` rather than Entra's `oid`, so any OIDC provider works the same way. The design is updated.
- New DNSSEC keys default to the `ecdsap256sha256` algorithm, because PowerDNS requires an algorithm when one is not given.
- The Content Security Policy allows inline `style` attributes (`style-src-attr 'unsafe-inline'`) because accessible UI components use them for visually hidden text. Scripts and style sheets remain same-origin only.
