# Logging

- Logs go to stdout. JSON in `prod` mode, readable text in `dev` mode.
- Default level: `debug` in `dev`, `info` in `prod`. Override with `SOABOX_LOG_LEVEL`.
- Every entry has `timestamp`, `level`, `target`, and, inside a request, `request_id`, `method`, `path`, `client_ip`, and `actor`.
- `environment` is added when `SOABOX_ENVIRONMENT` is set.
- Each request logs one completion event with status and latency at `info`.
- Upstream PowerDNS failures log at `warn` with the upstream status. Failed history writes log at `error` with the cause chain.

Never logged at any level: API keys, the PowerDNS key, passwords, TOTP codes and secrets, session ids, OIDC tokens and codes, and request or response bodies.

The `X-Request-Id` header is accepted from trusted proxies or generated, and is returned on every response.
