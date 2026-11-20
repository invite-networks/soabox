# Upgrade And Rollback

- Trigger: a new release.
- Prerequisites: the release notes read, a fresh backup taken.
- Validation status: **Planned, not yet executed.**

## Upgrade

1. Take a backup per [Backup and recovery](recovery.md).
2. Change the image tag or digest in `compose.yaml`.
3. `docker compose pull soabox && docker compose up -d soabox`
4. Migrations run automatically at startup. Watch `docker compose logs -f soabox` for `migrations complete`.

Expected result: `/readyz` returns `200` and `/version` shows the new version.

## Rollback

Migrations are forward-only. An older version may not start against a newer schema.

1. `docker compose stop soabox`
2. Restore the database backup taken before the upgrade.
3. Set the previous image tag or digest and `docker compose up -d soabox`.

Expected result: `/version` shows the previous version and `/readyz` returns `200`.

## If It Fails

If a migration fails, SOAbox does not start. Roll back as above and report the log output in an issue.
