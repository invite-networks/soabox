# Upgrade And Rollback

- Trigger: a new release.
- Prerequisites: the release notes read, a fresh backup taken.
- Validation status: **Executed 2026-11-19** on Docker Desktop (macOS) by the authoring agent, against the development stack and the production Compose example with Postgres. Independent execution by someone other than the author is still required before the first release.

## Upgrade

1. Take a backup per [Backup and recovery](recovery.md).
2. Images are published to `ghcr.io/invite-networks/soabox` as `X.Y.Z`, `X.Y`, and `latest` (see [Releasing](../dev/releasing.md)). If `compose.yaml` uses `latest`, pulling picks up the new release. To hold a deployment on one release, set the version tag, for example `ghcr.io/invite-networks/soabox:0.2.1`.
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
