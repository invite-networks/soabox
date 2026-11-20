# Configuration Reference

All configuration comes from environment variables. In `dev` mode, a `.env` file in the working directory is also loaded. In `prod` mode it is not. Any variable marked secret can be given as a file path instead by adding `_FILE` to its name (for example `SOABOX_PDNS_API_KEY_FILE=/run/secrets/pdns_key`).

| Variable | Required | Default | Secret | Purpose |
|---|---|---|---|---|
| `SOABOX_MODE` | Yes | none | | `dev` or `prod`. See [ADR-0011](../adr/0011-configuration-and-runtime-mode.md). |
| `SOABOX_ENVIRONMENT` | No | none | | Label added to logs. |
| `SOABOX_BIND` | No | `0.0.0.0:8080` | | Listen address. |
| `SOABOX_PUBLIC_URL` | Yes | none | | External URL, for OIDC redirects and cookie security. Must be `https` in `prod` mode. |
| `SOABOX_DATABASE_URL` | No | `sqlite://data/soabox.db` | Yes | `sqlite://...` or `postgres://...`. |
| `SOABOX_PDNS_URL` | Yes | none | | PowerDNS API base URL, for example `http://pdns:8081`. |
| `SOABOX_PDNS_API_KEY` | Yes | none | Yes | The PowerDNS API key. Only SOAbox holds it. |
| `SOABOX_PDNS_SERVER_ID` | No | `localhost` | | PowerDNS server id. |
| `SOABOX_PDNS_TIMEOUT_SECS` | No | `30` | | Upstream request timeout. |
| `SOABOX_TRUSTED_PROXIES` | Behind a proxy | empty | | Comma-separated CIDRs allowed to set `X-Forwarded-*`. Without it, every client appears as the proxy and shares one rate limit. |
| `SOABOX_SECRET_KEY` | Yes | none | Yes | 32 random bytes, base64. Encrypts TOTP secrets and the OIDC client secret stored from the UI, and protects short-lived sign-in cookies. |
| `SOABOX_SECRET_KEY_PREVIOUS` | No | none | Yes | Previous key, accepted for decryption during rotation. |
| `SOABOX_OIDC_ISSUER` | No | none | | Optional. Enables OIDC from the environment and overrides the UI settings (see below). For Entra: `https://login.microsoftonline.com/<tenant-id>/v2.0`. |
| `SOABOX_OIDC_CLIENT_ID` | With issuer | none | | |
| `SOABOX_OIDC_CLIENT_SECRET` | With issuer | none | Yes | |
| `SOABOX_OIDC_BUTTON_LABEL` | No | `Sign in with SSO` | | Text on the sign-in button. |
| `SOABOX_SESSION_IDLE_MINUTES` | No | `60` | | Idle timeout. |
| `SOABOX_SESSION_MAX_HOURS` | No | `12` | | Absolute timeout. |
| `SOABOX_BOOTSTRAP_ADMIN_USERNAME` | No | none | | Creates this Admin at startup if no Admin exists. |
| `SOABOX_BOOTSTRAP_ADMIN_PASSWORD` | No | none | Yes | Password for the bootstrap Admin. |
| `SOABOX_STATIC_DIR` | No | `frontend/dist` | | Built UI location. `/app/static` in the image. |
| `SOABOX_MAX_BODY_BYTES` | No | `10485760` | | Request body limit. |
| `SOABOX_LOG_LEVEL` | No | `debug` in dev, `info` in prod | | Overrides the default level. |
| `SOABOX_HSTS` | No | `false` | | Set `Strict-Transport-Security` if the proxy does not. |

Generate a secret key with `openssl rand -base64 32`.

## Single Sign-On: UI Or Environment

The preferred way to set up OIDC is **Settings > Single sign-on** in the UI, as described in the [Entra setup guide](../guides/entra-setup.md). Those settings are stored in the database with the client secret encrypted under `SOABOX_SECRET_KEY`, and apply without a restart.

The `SOABOX_OIDC_*` variables still work for existing deployments and for automated setups. Precedence is simple: when `SOABOX_OIDC_ISSUER` is set, the four `SOABOX_OIDC_*` variables are used and the UI shows them read-only with the note "Set by environment". Any settings saved in the UI are kept but ignored until the variables are removed and SOAbox is restarted. See [ADR-0015](../adr/0015-oidc-settings-in-ui.md).
