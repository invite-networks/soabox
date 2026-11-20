# ADR-0012: Distribution As A Container Image And Compose Examples

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: NFR-OPS-05, NFR-OPS-06

## Context

Users need a simple way to run SOAbox. Organization-specific deployment tooling does not belong in a public repository.

## Alternatives

- Container image with Compose examples.
- Include organization-specific deploy files.
- Native packages.

## Decision

Publish a multi-stage, distroless image running as UID 1001 to GitHub Container Registry. The repository includes a development `compose.yaml` and a production example with Postgres in `deploy/`. Organization-specific deployment files live in private repositories. Runtime data lives under a top-level `data/` directory.

## Rationale

One artifact that works anywhere containers run.

## Consequences

Kubernetes manifests may be added later.
