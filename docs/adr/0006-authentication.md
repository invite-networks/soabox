# ADR-0006: Local And OIDC Sign-In With Backend Sessions

- Status: Accepted
- Date: 2026-11-19
- Deciders: Project maintainer
- Affected requirements: FR-AUTH-*

## Context

Organizations want Entra ID sign-in. Small installs and emergencies need local accounts.

## Alternatives

- Local accounts only.
- OIDC only.
- Both, with a setting to restrict local sign-in when OIDC is configured.
- Tokens held in the browser.

## Decision

Both methods. OIDC uses the authorization code flow with PKCE run by the backend, which then issues its own server-side session cookie. The setting `local_login_with_oidc` defaults to `allow`. With `deny`, local Admins can still sign in. Any standard OIDC provider works. Entra ID is documented and tested.

## Rationale

Backend sessions keep tokens out of the browser. Local Admin sign-in is an emergency path if the provider is down.

## Consequences

SOAbox holds a client secret. Session storage is in the database.
