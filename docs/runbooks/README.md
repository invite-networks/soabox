# Runbooks

Operating procedures for people who run SOAbox. Each names its trigger, prerequisites, steps, expected result, failure handling, and validation status.

| Runbook | Use when |
|---|---|
| [Install](install.md) | Deploying SOAbox for the first time. |
| [Upgrade and rollback](upgrade.md) | Moving to a new version or back to the previous one. |
| [Health checks](health.md) | Monitoring, or something seems wrong. |
| [Backup and recovery](recovery.md) | Scheduled backups, restores, PowerDNS outages, lost Admin access. |
| [Key and secret rotation](key-rotation.md) | Rotating the PowerDNS key, the secret key, the OIDC secret, or revoking access. |

Every runbook was executed on 2026-11-19 by the authoring agent (results in each runbook). Before the first release, someone other than the author must execute them in a test environment and record the result here.
