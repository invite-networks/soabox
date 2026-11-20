# Architecture Decision Records

Each ADR records one consequential decision: its context, the alternatives, what was chosen, and why. Start a new ADR from the [template](template.md).

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-rust-axum-backend.md) | Rust and Axum backend | Accepted |
| [0002](0002-react-spa-frontend.md) | React single-page frontend served by the backend | Accepted |
| [0003](0003-pdns-compatible-passthrough.md) | PowerDNS-compatible API passthrough | Accepted |
| [0004](0004-api-key-swap.md) | Always swap client keys for the SOAbox PowerDNS key | Accepted |
| [0005](0005-database.md) | SQLite by default, Postgres supported, through SQLx | Accepted |
| [0006](0006-authentication.md) | Local and OIDC sign-in with backend sessions | Accepted |
| [0007](0007-local-roles.md) | Roles and zone access managed in SOAbox | Accepted |
| [0008](0008-single-pdns-server.md) | One PowerDNS server | Accepted |
| [0009](0009-audit-model.md) | Audit by reading before and after each write | Accepted |
| [0010](0010-generic-ui-theme.md) | Generic, unbranded UI theme | Accepted |
| [0011](0011-configuration-and-runtime-mode.md) | `SOABOX_` configuration and runtime mode | Accepted |
| [0012](0012-distribution.md) | Container image and Compose examples | Accepted |
| [0013](0013-api-key-hashing.md) | SHA-256 digests for API keys | Accepted |
| [0014](0014-license.md) | Apache-2.0 license | Accepted |
| [0015](0015-oidc-settings-in-ui.md) | OIDC provider settings managed in the UI, environment overrides | Accepted |
| [0016](0016-audit-hardening-2026-11.md) | Hardening from the November 2026 security audit | Accepted |

Maintenance rules: accepted ADRs are never edited to change the decision. A changed decision gets a new ADR that supersedes the old one, and both index rows are updated.
