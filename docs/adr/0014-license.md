# ADR-0014: Apache-2.0 License

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: Constraints

## Context

SOAbox is published publicly.

## Alternatives

- Apache-2.0.
- MIT.

## Decision

Apache-2.0, copyright SOAbox contributors.

## Rationale

Permissive with an explicit patent grant.

## Consequences

Dependencies must have compatible licenses, enforced by `cargo deny` and a frontend license check.
