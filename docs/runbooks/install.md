# Install

- Trigger: first deployment.
- Prerequisites: a container host with Docker Compose v2, a reverse proxy with TLS, PowerDNS Authoritative 5.x (or use the bundled example), a DNS name for SOAbox.
- Validation status: **Executed 2026-11-19** on Docker Desktop (macOS) by the authoring agent, against the development stack and the production Compose example with Postgres. Independent execution by someone other than the author is still required before the first release.

## Steps

1. Copy `deploy/compose.prod.yaml` and `.env.example` to the host directory that will hold the deployment, and rename them `compose.yaml` and `.env`.
2. Create `data/` next to them, owned by UID and GID 1001: `mkdir -p data/soabox && sudo chown -R 1001:1001 data/soabox`.
3. Set in `.env`: `SOABOX_MODE=prod`, `SOABOX_PUBLIC_URL`, `SOABOX_PDNS_URL`, `SOABOX_PDNS_API_KEY`, `SOABOX_SECRET_KEY` (`openssl rand -base64 32`), `SOABOX_TRUSTED_PROXIES` (the proxy's address), and the bootstrap Admin variables. Restrict the file: `chmod 600 .env`.
4. Make sure the PowerDNS API (`webserver-address`, `api=yes`) is reachable only from SOAbox. Do not publish its port.
5. `docker compose up -d`. A one-shot `pdns-init` service creates the PowerDNS database on first start.

The example uses the Compose project name `soabox`. Run only one SOAbox project per host under that name, or change `name:` in the file.
6. Point the reverse proxy at SOAbox port 8080 and pass `X-Forwarded-For` and `X-Forwarded-Proto`.
7. Remove the bootstrap password from `.env` after the first sign-in and change the password in the UI.

## Expected Result

`curl https://<public-url>/readyz` returns `200`. The sign-in page loads.

## If It Fails

- Startup stops with a message naming a variable: fix that value.
- `/readyz` fails on PowerDNS: check `SOABOX_PDNS_URL`, the key, and network reachability with `docker compose exec soabox soabox check`.
