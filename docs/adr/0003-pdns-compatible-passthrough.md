# ADR-0003: PowerDNS-Compatible API Passthrough

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: FR-API-*, FR-SCP-*

## Context

Automation already speaks the PowerDNS API. A separate SOAbox API would force every tool to change, and tools would bypass SOAbox with the PowerDNS key instead.

## Alternatives

- Mirror the PowerDNS 5.x API exactly under `/api`.
- A new SOAbox-specific API only.
- Expose PowerDNS directly and audit from its logs.

## Decision

SOAbox serves the PowerDNS Authoritative 5.x API under `/api` with identical paths, bodies, status codes, and errors, adding scoping and audit invisibly.

## Rationale

Existing clients work unchanged, and there is no reason left to give anyone the PowerDNS key.

## Consequences

SOAbox must track PowerDNS API changes. A compatibility suite against real PowerDNS is required. Admin keys forward unknown paths so new PowerDNS endpoints keep working.
