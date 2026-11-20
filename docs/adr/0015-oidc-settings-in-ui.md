# ADR-0015: OIDC Provider Settings Managed In The UI

- Status: Accepted
- Date: 2026-11-20
- Deciders: Project maintainer
- Affected requirements: FR-AUTH-03, FR-AUTH-10, FR-AUTH-11, FR-AUTH-12

## Context

OIDC was configured only through `SOABOX_OIDC_*` environment variables, which needs shell access to the host and a restart for every change, including routine client secret rotation. Admins asked to set up Microsoft Entra ID from the Settings page instead. Existing deployments already use the variables and must keep working.

## Alternatives

- Keep environment variables only. Simple, but every change needs host access and a restart.
- UI only, dropping the variables. Breaks existing deployments and infrastructure-as-code setups.
- UI settings with the environment as a fallback when nothing is stored. Two sources can both look active, and the effective one depends on database state that is not visible in the deployment files.
- UI settings, with the environment taking precedence when `SOABOX_OIDC_ISSUER` is set.

## Decision

The last option. When `SOABOX_OIDC_ISSUER` is set, the `SOABOX_OIDC_*` variables are used and the UI shows them read-only, marked "Set by environment". `PUT /v1/settings/oidc` answers `409`. Otherwise the settings saved in the UI apply.

UI settings live in a single-row `oidc_provider` table. The client secret is encrypted with XChaCha20-Poly1305 under `SOABOX_SECRET_KEY`, bound to the purpose `soabox/oidc-client-secret/v1`, never returned by the API, never written to history, and re-encrypted by `soabox rotate-secrets`.

The row is read on each sign-in and on `/auth/config`, so changes apply at once on every instance without a restart. The provider discovery document is cached in memory for the current issuer only, for at most one hour, and dropped when the settings are saved on that instance.

## Rationale

Environment precedence keeps deployments that are configured as code authoritative and predictable, while the UI covers the common case. A dedicated table keeps the encrypted secret as binary next to its metadata rather than inside the JSON key and value settings. Reading one row per sign-in is cheap and avoids cross-instance cache invalidation.

## Consequences

- Admins can set up and rotate the client secret without a restart.
- Losing or changing `SOABOX_SECRET_KEY` without `SOABOX_SECRET_KEY_PREVIOUS` makes the stored secret unreadable. Single sign-on then turns off and a warning is logged until an Admin enters the secret again.
- On other instances, a changed issuer is picked up immediately. A changed discovery document at the same issuer is picked up within an hour.
