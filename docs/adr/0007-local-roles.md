# ADR-0007: Roles And Zone Access Managed In SOAbox

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: FR-RBAC-*, FR-AUTH-04

## Context

Roles could come from identity provider claims such as Entra app roles or groups.

## Alternatives

- Map roles from provider claims.
- Manage roles in SOAbox.

## Decision

Roles (Admin, Operator, User), accounts, and zone access are managed only in SOAbox, whichever way a user signs in.

## Rationale

Works the same for local and OIDC users and any provider. No directory changes needed to grant DNS access.

## Consequences

Admins assign access in SOAbox. New OIDC users start as User with no zones, or pending approval.
