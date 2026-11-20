# Key And Secret Rotation

- Validation status: **Executed 2026-11-19** on Docker Desktop (macOS) by the authoring agent, against the development stack and the production Compose example with Postgres. Independent execution by someone other than the author is still required before the first release.

## PowerDNS API Key

Clients are not affected because they hold SOAbox keys.

1. Add the new key to PowerDNS (`api-key` accepts one value, so plan a short window) and restart PowerDNS.
2. Update `SOABOX_PDNS_API_KEY` and restart SOAbox with `docker compose up -d`.

Compose gives variables already exported in your shell priority over `.env`. If you have sourced `.env` into your shell, unset the old values first or the containers keep the old key.
3. Confirm `/readyz`.

## Revoking An API Key

Disable or delete it in the UI or with `soabox apikey disable <id>`. It stops working immediately.

## SOABOX_SECRET_KEY

1. Move the current value to `SOABOX_SECRET_KEY_PREVIOUS` and set a new `SOABOX_SECRET_KEY`.
2. Restart SOAbox. Run `soabox rotate-secrets` to re-encrypt stored TOTP secrets with the new key.
3. Remove `SOABOX_SECRET_KEY_PREVIOUS` and restart.

## OIDC Client Secret

Create a new secret in the provider, update `SOABOX_OIDC_CLIENT_SECRET`, restart, test sign-in, then delete the old secret in the provider.

## Removing A Person's Access

Disable the user in SOAbox, which ends their sessions, and disable any API keys they own. Also remove them in the identity provider.
