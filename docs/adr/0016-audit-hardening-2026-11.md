# ADR-0016: Hardening From The November 2026 Security Audit

- Status: Accepted
- Date: 2026-11-20
- Deciders: Project maintainer
- Affected requirements: FR-API-01, FR-KEY-06, FR-AUTH-02, FR-AUTH-03, NFR-SEC-*

## Context

A source-level security review of SOAbox 0.2.0 reported four Medium and four Low findings (SBX-001 to SBX-008) and no Critical or High findings. Most fixes are straightforward. Three needed a decision because they touch PowerDNS compatibility, developer workflow, or supply-chain policy.

## Decisions

**SBX-001, SBX-002, SBX-003, SBX-005: fixed as recommended.**

- A User-owned API key may create zones only in an account its owner still belongs to. Membership is checked on every request.
- Wrong and replayed TOTP codes count against the account-wide sign-in limiter as well as the client limiter.
- A TOTP time step is consumed with a conditional update (`WHERE totp_last_step < step`), so a code is accepted once even under concurrent requests.
- When an ID token is signed with a key SOAbox does not have, it fetches the provider's discovery document and keys again once and retries. Forced refreshes happen at most once a minute. A failed refresh keeps the known document and rejects the sign-in.

**SBX-004: production requires an https OIDC issuer from every source.** Issuers from `SOABOX_OIDC_ISSUER` follow the same rule as issuers saved in the UI: `https`, with `http` allowed only in `dev` mode. Production refuses to start otherwise.

**SBX-006: `OPTIONS` under `/api` is answered by SOAbox and never forwarded.** It carries no API key, and forwarding it sent an unauthenticated request to PowerDNS with the SOAbox PowerDNS key. SOAbox returns the same status and headers PowerDNS 5 returns for each route (`Access-Control-Allow-Headers`, `Access-Control-Allow-Methods`, `Access-Control-Max-Age`, and `404` for unknown routes). The compatibility suite compares every route with a real PowerDNS.

**SBX-008: production is never configured from a `.env` file.** SOAbox still loads `.env` when `SOABOX_MODE` is not `prod` in the process environment, so local development is unchanged. If a `.env` file was loaded and the resulting mode is `prod`, startup fails. Values that Compose passes with `env_file` are ordinary process environment and are not affected.

**SBX-007: accepted risk. Supporting images track `latest`.** The CI service and tool images (mock OIDC server, Gitleaks, Trivy) and the SOAbox image in the production Compose example use `latest` instead of digests. Postgres stays on major version `18` because a major upgrade needs a manual migration. The Dockerfile base images and the PowerDNS and Postgres images in the Compose files stay digest pinned.

## Rationale

Answering `OPTIONS` locally removes the only path where an unauthenticated request reached PowerDNS with the privileged key, without changing anything a client can observe. The `.env` rule closes the production gap while keeping `cargo run` working without extra exports.

For SBX-007, the maintainer prefers current scanner rules and test tooling over the reproducibility of pinned digests. The CI jobs already run with read-only repository permissions, and GitHub Actions themselves remain pinned to commit hashes.

## Consequences

- A compromised or broken `latest` tag of a CI tool image can affect CI runs until it is noticed. A tool release can also change CI results without a change in this repository.
- Deployments that copy the production example follow the newest SOAbox release on each pull. Operators who need a fixed release use a version tag.
- Browser pages on other origins still cannot call `/api`, as before: SOAbox never sends `Access-Control-Allow-Origin`, matching PowerDNS.
