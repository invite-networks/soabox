# Test Plan

Status: Draft for readiness review. Commands are **Planned, not yet executed** until the related milestone records results.

Related: [Requirements](requirements.md) | [Design](design.md) | [Testing guide](../dev/testing.md)

## 1. Strategy

| Tier | Location | Environment | Purpose |
|---|---|---|---|
| Unit (Rust) | inline `#[cfg(test)]` modules | No I/O | Classification, scope rules, key handling, diff, redaction, config parsing, client IP resolution. |
| Integration (Rust) | `backend/tests/` | SQLite in a temp dir, Postgres when `SOABOX_TEST_POSTGRES_URL` is set, `wiremock` for PowerDNS | Full router with real database and a mock PowerDNS. |
| Compatibility | `backend/tests/compat.rs` | Real PowerDNS 5 from `compose.yaml` or CI service, enabled by `SOABOX_COMPAT_PDNS_URL` | Same request directly and through SOAbox, compared. |
| Unit and component (frontend) | `frontend/src/**/*.test.tsx` | jsdom, MSW | Components and hooks through roles and labels. |
| End-to-end | `frontend/e2e/` | Built UI served by real SOAbox, real PowerDNS, mock OIDC provider | User workflows and axe scans. |
| Load | `backend/benches/` or a scripted run | Compose stack | NFR-PERF-01 and NFR-PERF-02. Before first release. |

All tests use synthetic data, isolated databases, and no live credentials.

## 2. Requirement Coverage

| Requirement | Planned checks | Tier |
|---|---|---|
| FR-API-01, 02, 04 | Compatibility cases for every endpoint class | Compatibility |
| FR-API-03, 05 | Error cases: bad key, missing key, wrong server id, bad JSON | Compatibility, Integration |
| FR-API-06 | PowerDNS mock refuses connections or delays past timeout | Integration |
| FR-KEY-01 | Mock asserts upstream `X-API-Key` equals configured key. Real PowerDNS key sent to SOAbox returns `401` | Integration |
| FR-KEY-02 | Stored row contains only a digest | Integration |
| FR-KEY-03, FR-SCP-03, FR-SCP-04 | Table test of role by endpoint class | Unit, Integration |
| FR-KEY-04, FR-SCP-01, FR-SCP-02 | Scoped key list filtering and missing-zone byte equality | Integration, Compatibility |
| FR-KEY-05 | Expired key, CIDR allow and deny with trusted and untrusted proxies | Unit, Integration |
| FR-KEY-06 | Scoped create sets account in PowerDNS body and SOAbox store | Integration, Compatibility |
| FR-KEY-07 | Disable, enable, delete transitions | Integration |
| FR-KEY-08 | Last-used throttling with a controlled clock | Unit |
| FR-AUD-01 to 03 | History rows and diffs after PATCH, PUT, DELETE, failure, redaction | Unit, Integration |
| FR-AUD-04 | `/v1/history` filters and permissions | Integration, End-to-end |
| FR-AUD-05 | History insert forced to fail | Integration |
| FR-AUTH-01, 02 | Password and TOTP sign-in | Integration |
| FR-AUTH-03, 04 | Mock OIDC provider: code flow, PKCE, state, nonce, create and approve modes | Integration, End-to-end |
| FR-AUTH-05 | Matrix of OIDC configured, setting, and role | Integration |
| FR-AUTH-06 | Bootstrap variables and CLI | Integration |
| FR-AUTH-07, 09 | Rotation, idle and absolute timeout, sign-out, revoke | Integration |
| FR-AUTH-08 | Rate limit trips and resets | Unit, Integration |
| FR-RBAC-01 to 05 | Access matrix for each role and setting through `/v1` and `/api` | Integration |
| FR-UI-01 to 11 | Workflow tests per screen | Component, End-to-end |
| FR-CLI-01 | CLI commands against a temp database | Integration |
| NFR-SEC-01, 02 | Log capture assertions for secrets | Integration |
| NFR-SEC-03 | Forwarded header spoofing | Unit |
| NFR-SEC-04, 05 | Cookie flags, CSRF rejection, header presence | Integration |
| NFR-SEC-06 | `cargo audit`, `cargo deny`, `pnpm audit --prod` | CI |
| NFR-SEC-07 | Stored TOTP secret is not plaintext | Integration |
| NFR-PERF-01, 02 | Load runs | Load |
| NFR-OPS-01, 02 | Config parsing including `_FILE` variants and missing mode | Unit |
| NFR-OPS-03 | JSON logs in `prod` | Unit |
| NFR-OPS-04 | Health endpoints | Integration |
| NFR-OPS-05, 06 | Container with read-only root and UID check | Container smoke |
| NFR-OPS-07, 08 | Migrations on both databases | Integration |
| NFR-QA-01 | Coverage thresholds | CI |
| NFR-A11Y-01 | axe on every page | End-to-end |

## 3. Release Gates

A release requires all of the following on the tagged revision:

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run`, `cargo test --doc`.
2. Coverage at or above 80% overall.
3. Compatibility suite green against PowerDNS 5.
4. Frontend `biome check`, `tsc`, Vitest with coverage, build, and Playwright with axe.
5. `cargo audit`, `cargo deny check`, `pnpm audit --prod` with no unresolved Critical or High.
6. Container builds, starts, passes `/readyz`, survives a restart with data intact.
7. Security and dependency audit reports reviewed and findings resolved or accepted with a recorded reason.

## 4. Performance Results

Executed 2026-11-19 with `scripts/loadtest.py` on a 10-core macOS host with Docker Desktop and PowerDNS 5.1.5 (SQLite backend).

| Requirement | Measurement | Result |
|---|---|---|
| NFR-PERF-01 | `GET /servers/localhost` at 50 requests per second for 20 s, direct p95 7.78 ms, through SOAbox p95 6.44 ms | Pass: no added latency (SOAbox reuses connections to PowerDNS) |
| NFR-PERF-02 | `GET /zones` with 10,000 zones and a scoped key, direct p95 101.20 ms, through SOAbox p95 102.76 ms | Pass: 1.56 ms added |

The host is faster than the 2 vCPU reference in the requirement. Repeat on reference hardware before the first release.
