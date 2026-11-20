# Key And Secret Rotation

- Validation status: **Planned, not yet executed.**

## PowerDNS API Key

Clients are not affected because they hold SOAbox keys.

1. Add the new key to PowerDNS (`api-key` accepts one value, so plan a short window) and restart PowerDNS.
2. Update `SOABOX_PDNS_API_KEY` and restart SOAbox.
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
