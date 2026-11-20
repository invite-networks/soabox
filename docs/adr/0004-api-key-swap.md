# ADR-0004: Always Swap Client Keys For The SOAbox PowerDNS Key

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: FR-KEY-01, NFR-SEC-01

## Context

A toggle to pass client keys through to PowerDNS was considered.

## Alternatives

- Always swap: clients hold SOAbox keys, SOAbox holds the PowerDNS key.
- Pass through: PowerDNS validates client keys, SOAbox cannot scope.
- Mixed mode: recognize SOAbox keys, pass others through.

## Decision

SOAbox always validates the client key and replaces it with its own PowerDNS key. Client keys are never forwarded. The PowerDNS key is rejected if a client sends it.

## Rationale

One model is simpler to reason about and audit. Scoping and revocation only work if SOAbox owns authentication.

## Consequences

Migrating clients must be issued SOAbox keys. The PowerDNS key can be rotated without touching clients.
