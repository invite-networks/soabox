# Backup And Recovery

- Validation status: **Planned, not yet executed.**

SOAbox stores keys, users, settings, and history. DNS data lives in PowerDNS and needs its own backup.

## Backup

- SQLite: `docker compose exec soabox soabox backup /app/data/backup-$(date +%F).db` writes a consistent copy using SQLite's online backup. Copy it off the host.
- Postgres: `pg_dump -Fc` on a schedule.
- Keep `.env` (or its secrets) backed up separately and securely. Without `SOABOX_SECRET_KEY`, TOTP enrollments cannot be decrypted.

## Restore

1. `docker compose stop soabox`
2. SQLite: replace `data/soabox/soabox.db` with the backup. Postgres: `pg_restore --clean`.
3. `docker compose start soabox` and confirm `/readyz`.

## PowerDNS Outage

API clients receive `502` and the UI shows PowerDNS as unavailable. No SOAbox action is needed. Recover PowerDNS, then confirm `/readyz`.

## Identity Provider Outage

Local Admins can always sign in. If `local_login_with_oidc` is `allow`, other local accounts can too.

## Lost Admin Access

`docker compose exec soabox soabox create-admin --username recovery` creates a new Admin and prints a one-time password. Remove the account after use.
