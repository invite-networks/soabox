# Software Requirements Specification

Status: Draft for readiness review. Applies to v0.x.

Related: [Software Design](design.md) | [Test Plan](test-plan.md) | [Decisions](../adr/README.md) | [Diagrams](../uml/README.md)

## 1. Purpose

SOAbox is a web interface and API gateway for PowerDNS Authoritative Server 5.x. It lets people manage DNS through a browser and lets automation use the PowerDNS HTTP API with scoped credentials and a complete audit trail. This document states what SOAbox must do. The [design](design.md) states how.

## 2. Scope

In scope:

- A PowerDNS-compatible HTTP API under `/api` that sits in front of one PowerDNS Authoritative 5.x server.
- SOAbox-issued API keys with roles, zone and account scope, expiry, and network limits.
- An audit history of every change.
- A browser UI for zones, records, DNSSEC, metadata, templates, history, and administration.
- Sign-in with local accounts and OpenID Connect, with Microsoft Entra ID documented and tested.
- Roles, accounts, and zone access managed inside SOAbox.

Out of scope for v0.x:

- More than one PowerDNS server.
- PowerDNS Recursor.
- Importing data from PowerDNS-Admin.
- Identity provider bearer tokens on the API.
- Proxying the PowerDNS built-in statistics web page and `/metrics` endpoint.
- Passing a client's API key through to PowerDNS. Client keys are always replaced ([ADR-0004](../adr/0004-api-key-swap.md)).

## 3. Actors

| Actor | Description |
|---|---|
| Admin | Manages everything: zones, users, accounts, keys, settings, server views. |
| Operator | Manages all zones and templates. Reads server statistics and configuration. Cannot manage users, keys of others, or settings. |
| User | Manages only zones assigned to them directly or through an account. Extra abilities depend on settings. |
| API client | Software that speaks the PowerDNS API using a SOAbox key. Unaware of SOAbox. |
| Identity provider | An OpenID Connect provider such as Microsoft Entra ID. |
| PowerDNS | The PowerDNS Authoritative 5.x server SOAbox fronts. |
| Operator of the deployment | The person who installs, upgrades, and backs up SOAbox. Not a SOAbox role. |

## 4. Functional Requirements

Each requirement has a stable ID. IDs are never reused. Acceptance criteria are verified by the checks listed in the [test plan](test-plan.md).

### 4.1 PowerDNS-compatible API

| ID | Requirement | Acceptance criteria |
|---|---|---|
| FR-API-01 | SOAbox serves the PowerDNS Authoritative 5.x HTTP API paths under `/api`, including discovery, servers, zones, rrsets, notify, axfr-retrieve, export, rectify, cryptokeys, metadata, tsigkeys, autoprimaries, search-data, statistics, config, cache flush, views, and networks. | Every case in the compatibility suite returns the same status, `Content-Type`, and JSON body through SOAbox as directly from PowerDNS. |
| FR-API-02 | Request bodies are forwarded unchanged except where SOAbox must set the zone `account` field (FR-KEY-06). | Compatibility suite write cases produce identical zones through SOAbox and directly. |
| FR-API-03 | Errors SOAbox produces on `/api` use the PowerDNS error body `{"error": "..."}` and the status PowerDNS uses for the equivalent case. | Bad key, missing key, unknown server, out-of-scope zone, and forbidden endpoint cases match PowerDNS bodies. |
| FR-API-04 | Response `Content-Type` and `Server` headers are copied from PowerDNS. | Header comparison in the compatibility suite. |
| FR-API-05 | Only the configured server id is served. Any other `server_id` returns the PowerDNS unknown-server response. | Compatibility case with `server_id` `other`. |
| FR-API-06 | When PowerDNS is unreachable or times out, SOAbox returns `502` with a PowerDNS-shaped error body and logs the cause. | Integration test with PowerDNS stopped. |

### 4.2 API keys

| ID | Requirement | Acceptance criteria |
|---|---|---|
| FR-KEY-01 | Clients authenticate with SOAbox keys in the `X-API-Key` header. SOAbox replaces the key with its own PowerDNS key on every upstream call. A client key is never forwarded. | Wiremock test asserts the upstream request carries only the configured key. Sending the real PowerDNS key to SOAbox returns `401`. |
| FR-KEY-02 | Keys are random, shown once at creation, and stored only as a hash. | Database inspection test finds no plaintext key. |
| FR-KEY-03 | Each key has one role: `admin`, `operator`, or `scoped`. | Authorization table tests per role. |
| FR-KEY-04 | Scoped keys carry a set of zones and a set of accounts. They may act only on zones in that set or owned by those accounts. | Scope tests for direct zones and account zones. |
| FR-KEY-05 | Keys may have an expiry time and a list of allowed client networks. Expired keys and keys used from outside the allowed networks are rejected with `401`. | Expiry and CIDR tests using the resolved client IP. |
| FR-KEY-06 | A scoped key may be allowed to create zones. Created zones are assigned to the key's designated account in SOAbox, and the PowerDNS `account` field is set to that account's name. | Zone create test checks both stores. |
| FR-KEY-07 | Keys can be disabled, re-enabled, and deleted. Disabled and deleted keys are rejected immediately. | State tests per the [API key lifecycle](../uml/api-key-lifecycle.md). |
| FR-KEY-08 | Keys record a last-used time with at most one write per key per minute. | Unit test with a controlled clock. |

### 4.3 Scope enforcement

| ID | Requirement | Acceptance criteria |
|---|---|---|
| FR-SCP-01 | Zone list and search responses contain only zones in scope for the caller. | Compatibility scope cases. |
| FR-SCP-02 | Requests for an out-of-scope zone return the exact response PowerDNS gives for a zone that does not exist. | Byte comparison against PowerDNS's missing-zone response. |
| FR-SCP-03 | Endpoints a role may not use return the PowerDNS response for a rejected key. | Table tests per role and endpoint class. |
| FR-SCP-04 | Paths SOAbox does not recognize are forwarded for admin keys and rejected for other roles. | Table tests with a synthetic unknown path. |

### 4.4 Audit history

| ID | Requirement | Acceptance criteria |
|---|---|---|
| FR-AUD-01 | Every zone write through the API or UI records actor, client IP, time, operation, zone, outcome, and a before and after diff of affected RRsets. | Write cases assert a matching history row. |
| FR-AUD-02 | Failed writes are recorded with the upstream status and error and no diff. | Failure case assertions. |
| FR-AUD-03 | Server-level writes are recorded with secrets redacted, including TSIG key material and private keys. | Redaction tests. |
| FR-AUD-04 | History is viewable per zone and globally, filterable by actor, zone, operation, and date, subject to permissions. | UI and `/v1` tests. |
| FR-AUD-05 | A failure to store history does not change the response to the caller and is logged at `ERROR`. | Integration test with history storage forced to fail. |

### 4.5 Authentication

| ID | Requirement | Acceptance criteria |
|---|---|---|
| FR-AUTH-01 | Local accounts sign in with username and password. Passwords are hashed with argon2id. | Login tests. Database inspection finds no plaintext. |
| FR-AUTH-02 | Local accounts may enable TOTP as a second factor. | TOTP enrollment and login tests. |
| FR-AUTH-03 | When OpenID Connect is configured, users can sign in through it using the authorization code flow with PKCE run by the backend. Tokens never reach the browser. | End-to-end sign-in against a test provider. Browser storage inspection. |
| FR-AUTH-04 | OIDC users are matched by issuer and subject. On first sign-in, SOAbox either creates the user with the User role and no zones, or holds the user for Admin approval, depending on a setting. | Tests for both setting values. |
| FR-AUTH-05 | The setting `local_login_with_oidc` controls whether local accounts may sign in while OIDC is configured. The default is `allow`. With `deny`, only local Admin accounts may sign in locally. Without OIDC configured, local sign-in is always available. | Tests for each combination. |
| FR-AUTH-06 | The first Admin is created from bootstrap environment variables or the `soabox create-admin` command. | Startup and CLI tests. |
| FR-AUTH-07 | Sessions are server-side, rotated on sign-in and role change, and expire after configurable idle and absolute timeouts. | Session tests with a controlled clock. |
| FR-AUTH-08 | Sign-in attempts are rate limited per account and per client IP. | Rate limit tests. |
| FR-AUTH-09 | Users can sign out, and Admins can end a user's sessions. | Session revocation tests. |
| FR-AUTH-10 | Admins configure OpenID Connect in the UI: enabled, issuer URL, client ID, client secret, and button label. The UI shows the redirect URI to register with the provider. The issuer must be `https` (`http` is accepted only in `dev` mode). The client ID and secret are required when enabled. Changes apply to the next sign-in without a restart. | `/v1/settings/oidc` integration tests, including a change of client ID taking effect on the next sign-in. Component tests for the form. |
| FR-AUTH-11 | The OIDC client secret is write-only. It is stored encrypted under `SOABOX_SECRET_KEY`, never returned by the API or written to history, and re-encrypted by `soabox rotate-secrets`. Saving the settings is recorded in history without the secret. | Tests that read responses, the database, and history for the secret. Rotation test. |
| FR-AUTH-12 | When `SOABOX_OIDC_ISSUER` is set, the `SOABOX_OIDC_*` variables take precedence over the settings stored in the UI. The UI then shows them read-only, marked as set by environment, and the API refuses changes with `409`. | Precedence integration test. Component test for the read-only view. |

### 4.6 Authorization and accounts

| ID | Requirement | Acceptance criteria |
|---|---|---|
| FR-RBAC-01 | Users have one role: Admin, Operator, or User. Roles are managed only in SOAbox. | Role tests. |
| FR-RBAC-02 | Accounts group zones and users. A User reaches zones assigned directly and zones of their accounts. | Access tests. |
| FR-RBAC-03 | Settings control whether Users may create zones, delete zones, view history, manage DNSSEC, and create their own API keys. | Tests per setting. |
| FR-RBAC-04 | Record types allowed in the UI are configurable separately for forward and reverse zones. | Editor tests. |
| FR-RBAC-05 | The UI and `/v1` API apply the same authorization core as `/api`. | Shared authorization tests run against both entry points. |

### 4.7 User interface

| ID | Requirement | Acceptance criteria |
|---|---|---|
| FR-UI-01 | Zone list with search, pagination, kind, serial, DNSSEC state, and account. | Playwright tests. |
| FR-UI-02 | Zone create (Native, Primary, Secondary, optional template), delete, notify, AXFR retrieve, rectify, and export. | Playwright tests. |
| FR-UI-03 | Record editor with per-type validation, add, edit, delete, and disable, and a pending-changes review applied as one change. | Playwright tests. |
| FR-UI-04 | Optional automatic PTR creation in reverse zones the user may edit. | Playwright tests. |
| FR-UI-05 | DNSSEC: enable, disable, list keys and DS records, add, activate, deactivate, and remove keys. | Playwright tests. |
| FR-UI-06 | Zone metadata view and edit. | Playwright tests. |
| FR-UI-07 | Zone templates. | Playwright tests. |
| FR-UI-08 | History views (FR-AUD-04). | Playwright tests. |
| FR-UI-09 | Administration of users, accounts, roles, API keys, settings, and TSIG keys. | Playwright tests. |
| FR-UI-10 | Server configuration and statistics, read only. | Playwright tests. |
| FR-UI-11 | Light and dark mode following the system setting, with a manual toggle. | Visual check and Playwright tests. |

### 4.8 Command line

| ID | Requirement | Acceptance criteria |
|---|---|---|
| FR-CLI-01 | The `soabox` binary offers `serve`, `create-admin`, `apikey`, `account`, `history`, `check`, `backup`, and `rotate-secrets` commands. | CLI tests. |

## 5. Nonfunctional Requirements

| ID | Requirement | Acceptance criteria |
|---|---|---|
| NFR-SEC-01 | The PowerDNS API key exists only in SOAbox configuration. It is never logged or returned. | Log capture tests. Response scans. |
| NFR-SEC-02 | Secrets, passwords, keys, tokens, and request bodies are never logged at any level. | Log capture tests. |
| NFR-SEC-03 | `X-Forwarded-*` headers are trusted only from configured proxy networks. | Spoofing tests. |
| NFR-SEC-04 | Cookies are `HttpOnly`, `Secure`, and `SameSite=Lax`. State-changing `/v1` calls require a CSRF token. | Header and CSRF tests. |
| NFR-SEC-05 | Security headers are set on UI and `/v1` responses. The UI has a strict Content Security Policy with no third-party sources. | Header tests. |
| NFR-SEC-06 | Dependency audits pass in CI with no unresolved Critical or High findings. | CI gate. |
| NFR-SEC-07 | TOTP secrets are encrypted at rest. | Database inspection test. |
| NFR-PERF-01 | Passthrough adds no more than 20 ms at the 95th percentile for unfiltered requests at 50 requests per second on a 2 vCPU host. | Load test before first release. |
| NFR-PERF-02 | Filtering a zone list of 10,000 zones adds no more than 200 ms. | Load test before first release. |
| NFR-OPS-01 | Configuration comes only from `SOABOX_` environment variables, and each secret can be read from a file with a `_FILE` suffix. Missing or invalid required values stop startup with a clear message. | Config tests. |
| NFR-OPS-02 | `SOABOX_MODE` is required and is `dev` or `prod`. No default. | Config tests. |
| NFR-OPS-03 | Logs go to stdout: JSON in `prod`, human-readable in `dev`. | Log format tests. |
| NFR-OPS-04 | `/healthz`, `/readyz`, and `/version` are available. `/readyz` checks the database and PowerDNS. | Integration tests. |
| NFR-OPS-05 | Runtime state lives only under `data/`. | Container test with a read-only root filesystem. |
| NFR-OPS-06 | The container runs as UID and GID 1001. | Image inspection. |
| NFR-OPS-07 | Database migrations run automatically at startup and are forward-only. | Upgrade test from the previous release. |
| NFR-OPS-08 | SQLite and Postgres are both supported. | Integration suite runs on both. |
| NFR-QA-01 | Line coverage is at least 80% overall and at least 90% for authorization, key, and audit modules. | CI gate. |
| NFR-A11Y-01 | The UI is fully keyboard operable and passes axe checks with no serious or critical violations. | Playwright with axe. |
| NFR-UX-01 | The UI carries no company branding. | Review. |

## 6. Constraints

- PowerDNS Authoritative 5.x with its HTTP API enabled.
- SOAbox runs behind a TLS-terminating reverse proxy.
- PowerDNS's API should be reachable only by SOAbox, so every change passes through the audit layer.
- Apache-2.0 license. Dependencies must be license-compatible.

## 7. Assumptions And Known Limitations

- Audit diffs are computed from reads before and after a write. A change made directly on PowerDNS between those reads would appear in the diff. Restricting PowerDNS API access to SOAbox prevents this.
- `502` on an unreachable PowerDNS and `429` on rate-limited sign-in or key attempts are responses PowerDNS itself never sends. They occur only on failure or abuse.
