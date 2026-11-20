# Agent And Contributor Instructions

These rules apply to anyone, human or agent, changing this repository.

- Read [docs/README.md](docs/README.md) first. The [requirements](docs/spec/requirements.md) and [design](docs/spec/design.md) are authoritative.
- Change documents before code when behavior or contracts change. Record consequential decisions as new [ADRs](docs/adr/README.md).
- SOAbox is a generic public project. No company branding, names, or company-specific configuration.
- The `/api` passthrough must stay indistinguishable from PowerDNS 5.x. Any change there needs a compatibility test.
- Only `backend/src/config.rs` reads environment variables. All variables use the `SOABOX_` prefix.
- Runtime state goes under `data/` only.
- Follow [coding standards](docs/dev/coding-standards.md) and [testing](docs/dev/testing.md). Coverage must stay at or above 80%.
- Never log or commit secrets.
