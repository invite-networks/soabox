# Health Checks

- Validation status: **Executed 2026-11-19** on Docker Desktop (macOS) by the authoring agent, against the development stack and the production Compose example with Postgres. Independent execution by someone other than the author is still required before the first release.

| Endpoint | Meaning of `200` |
|---|---|
| `/healthz` | The process is running. Use for liveness. |
| `/readyz` | The database answers and PowerDNS answers with the configured key. Use for readiness and alerting. |
| `/version` | Returns the version and git revision. |

## Triage

1. `/healthz` fails: the process is down. Check `docker compose ps` and logs.
2. `/readyz` fails: the JSON body names the failing dependency (`database` or `pdns`).
3. API clients get `502`: PowerDNS is unreachable or slow. Check PowerDNS and `SOABOX_PDNS_TIMEOUT_SECS`.
4. API clients get `401` unexpectedly: check the key is active and not expired, and that the client address is in its allowed networks. If behind a proxy, check `SOABOX_TRUSTED_PROXIES` so the real client address is used.
