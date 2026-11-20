# ADR-0008: One PowerDNS Server

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: FR-API-05

## Context

PowerDNS-Admin supports one server. Multiple servers would show up as extra `server_id` values.

## Alternatives

- One server now, server id kept as a parameter internally.
- Multiple servers from the start.

## Decision

v0.x fronts one PowerDNS server, exposed under its configured server id (default `localhost`).

## Rationale

Keeps the first release small. The internal parameter leaves room to add servers later.

## Consequences

Requests for any other server id return the PowerDNS unknown-server response.
