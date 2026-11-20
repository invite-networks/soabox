# ADR-0011: SOABOX_ Configuration And Runtime Mode

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: NFR-OPS-01, NFR-OPS-02, NFR-OPS-03

## Context

Configuration must be generic for public users while following a dev and prod runtime mode convention.

## Alternatives

- A company prefix such as `INVITE_`.
- A project prefix `SOABOX_`.

## Decision

All variables use `SOABOX_`. `SOABOX_MODE` is required and is `dev` or `prod`, with no default. `dev` gives readable logs at DEBUG and exposes OpenAPI docs. `prod` gives JSON logs at INFO and no debug endpoints. Secret variables also accept a `_FILE` suffix. `SOABOX_ENVIRONMENT` is an optional label for logs.

## Rationale

Neutral naming for a public project, and file-based secrets work with Docker and Kubernetes secrets.

## Consequences

Code branches only on `SOABOX_MODE`.
