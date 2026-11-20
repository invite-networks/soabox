# Security Policy

## Reporting a Vulnerability

Please report vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/invite-networks/soabox/security/advisories/new).
Do not open a public issue.

Include the SOAbox version or commit, how to reproduce the problem, and what an attacker could do with it. We aim to acknowledge reports within three business days.

## Supported Versions

SOAbox is in early development (v0.x). Fixes are made on `main` and in the next release.

## How SOAbox Is Checked

- Every pull request runs `cargo audit`, `cargo deny`, `pnpm audit`, a 72 hour release cooldown on new dependencies, a gitleaks scan of the git history, and a Trivy scan of the container image.
- A security review of the code and its dependencies was completed before the first release, and a second source review of 0.2.0 was completed in November 2026. Its findings are fixed or recorded as accepted risks in [ADR-0016](docs/adr/0016-audit-hardening-2026-11.md). Dependency risks are recorded with their reasons in `backend/deny.toml` and `backend/.cargo/audit.toml`.
