# Runbooks

Operating procedures for people who run SOAbox. Each names its trigger, prerequisites, steps, expected result, failure handling, and validation status.

| Runbook | Use when |
|---|---|
| [Install](install.md) | Deploying SOAbox for the first time. |
| [Upgrade and rollback](upgrade.md) | Moving to a new version or back to the previous one. |
| [Health checks](health.md) | Monitoring, or something seems wrong. |
| [Backup and recovery](recovery.md) | Scheduled backups, restores, PowerDNS outages, lost Admin access. |
| [Key and secret rotation](key-rotation.md) | Rotating the PowerDNS key, the secret key, the OIDC secret, or revoking access. |

Validation status for every runbook: **Planned, not yet executed.** Each will be executed in a test environment before the first release and the result recorded here.
