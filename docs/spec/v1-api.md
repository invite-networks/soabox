# UI API Contract (`/v1` and `/auth`)

Status: Accepted for milestones 2 to 5. The generated `backend/openapi.json` is authoritative once implemented. This document is the contract the frontend and backend are built against.

Related: [Design](design.md) | [Requirements](requirements.md)

## Conventions

- JSON bodies. Times are Unix seconds. Zone ids in paths are the PowerDNS URL id (for example `example.com.` or `0=2F26.2.0.192.in-addr.arpa.`), percent-encoded by the client.
- Authentication is the `soabox_session` cookie (`__Host-soabox_session` when served over HTTPS). Every `POST`, `PUT`, `PATCH`, and `DELETE` under `/v1` must send `X-CSRF-Token` with the value from `GET /v1/me`.
- Errors: `{"detail": "...", "code": "SCREAMING_SNAKE", "status": 422}`. Codes used: `BAD_REQUEST`, `VALIDATION_ERROR`, `UNAUTHENTICATED`, `TOTP_REQUIRED`, `FORBIDDEN`, `NOT_FOUND`, `CONFLICT`, `RATE_LIMITED`, `UPSTREAM_ERROR`, `UPSTREAM_UNAVAILABLE`, `INTERNAL_ERROR`, `CSRF_FAILED`.
- A zone outside the caller's access returns `404 NOT_FOUND`.

## Sign-In (`/auth`)

| Method and path | Body | Response |
|---|---|---|
| `GET /auth/config` | | `{"local_enabled": bool, "oidc_enabled": bool, "oidc_label": string}` |
| `POST /auth/login` | `{"username", "password", "totp_code"?}` | `200 Me`, sets cookie. `401 UNAUTHENTICATED` for bad credentials. `401 TOTP_REQUIRED` when a code is needed. `403 FORBIDDEN` when local sign-in is not allowed for this user. `429 RATE_LIMITED`. |
| `POST /auth/logout` | | `204`, clears cookie. |
| `GET /auth/oidc/start` | | `302` to the provider. |
| `GET /auth/oidc/callback` | | `302` to `/`, or to `/login?error=<code>` with `pending_approval`, `disabled`, or `oidc_failed`. |

## Current User

`Me`:

```json
{
  "user": {"id": 1, "username": "admin", "display_name": "Admin", "email": "", "role": "admin", "auth_source": "local", "totp_enabled": false},
  "csrf_token": "...",
  "permissions": {
    "manage_users": true, "manage_settings": true, "manage_tsig": true, "view_server": true,
    "all_zones": true, "create_zones": true, "delete_zones": true, "view_history": true,
    "manage_dnssec": true, "manage_templates": true, "create_api_keys": true
  }
}
```

| Method and path | Body | Response |
|---|---|---|
| `GET /v1/me` | | `Me` or `401` |
| `PUT /v1/me` | `{"display_name", "email"}` | `Me` |
| `PUT /v1/me/password` | `{"current_password", "new_password"}` | `204`. Local users only. Minimum 12 characters. Ends the user's other sessions. |
| `POST /v1/me/totp/setup` | | `{"secret", "otpauth_url"}` |
| `POST /v1/me/totp/enable` | `{"code"}` | `204` |
| `DELETE /v1/me/totp` | `{"code", "password"}` | `204`. A code is accepted once. |

## Dashboard And Server

| Method and path | Response |
|---|---|
| `GET /v1/dashboard` | `{"zone_count", "recent_history": HistoryEntry[], "server": {"version", "uptime"} or null}` |
| `GET /v1/server` | `{"id", "daemon_type", "version"}` (view_server) |
| `GET /v1/server/statistics` | `[{"name", "type", "value"}]` as PowerDNS returns them (view_server) |
| `GET /v1/server/config` | `[{"name", "type", "value"}]` (view_server) |

## Zones

`ZoneSummary`: `{"id", "name", "kind", "serial", "dnssec", "account": {"id", "name"} or null, "masters": []}`

`Zone`: `ZoneSummary` plus `{"soa_edit_api", "api_rectify", "nsec3param", "allowed_record_types": [], "rrsets": RrSet[]}`. `allowed_record_types` comes from the forward or reverse list in settings, so every user sees the types the backend will accept.

`RrSet`: `{"name", "type", "ttl", "records": [{"content", "disabled"}], "comments": [{"content", "account", "modified_at"}]}`

| Method and path | Body | Response |
|---|---|---|
| `GET /v1/zones?q=&page=&per_page=` | | `{"items": ZoneSummary[], "total": n}`. Filtered to the caller's zones. `per_page` default 50, max 500. |
| `POST /v1/zones` | `{"name", "kind": "Native"or"Master"or"Slave", "masters": [], "nameservers": [], "template_id"?, "account_id"?, "soa_edit_api"?}` | `201 Zone` |
| `GET /v1/zones/{id}` | | `Zone` |
| `PUT /v1/zones/{id}` | any of `{"kind", "masters", "account_id" (null clears), "soa_edit_api", "api_rectify"}` | `204` |
| `DELETE /v1/zones/{id}` | | `204` |
| `PATCH /v1/zones/{id}/rrsets` | `{"rrsets": [{"name", "type", "ttl", "changetype": "REPLACE"or"DELETE", "records", "comments"?}]}` | `204`. Record types are checked against the allowed list for the zone (forward or reverse). |
| `POST /v1/zones/{id}/notify` | | `{"result"}` |
| `POST /v1/zones/{id}/rectify` | | `{"result"}` |
| `POST /v1/zones/{id}/axfr-retrieve` | | `{"result"}` |
| `GET /v1/zones/{id}/export` | | `{"zone": "<zone file text>"}` |
| `GET /v1/zones/{id}/history` | | `{"items": HistoryEntry[]}` (view_history) |

Automatic PTR: when the `auto_ptr` setting is on and a `PATCH` replaces an `A` or `AAAA` record, SOAbox also replaces the matching PTR in a reverse zone the caller can edit, if one exists. Each PTR change is a separate history entry.

## DNSSEC And Metadata (manage_dnssec)

| Method and path | Body | Response |
|---|---|---|
| `POST /v1/zones/{id}/dnssec` | `{"enabled": bool, "nsec3param"?: string}` | `204` |
| `GET /v1/zones/{id}/cryptokeys` | | `[{"id", "keytype", "active", "published", "algorithm", "bits", "dnskey", "ds": []}]` |
| `POST /v1/zones/{id}/cryptokeys` | `{"keytype": "ksk"or"zsk"or"csk", "active", "algorithm"?, "bits"?}` | `201` key |
| `PUT /v1/zones/{id}/cryptokeys/{key_id}` | `{"active", "published"}` | `204` |
| `DELETE /v1/zones/{id}/cryptokeys/{key_id}` | | `204` |
| `GET /v1/zones/{id}/metadata` | | `[{"kind", "metadata": []}]` |
| `PUT /v1/zones/{id}/metadata/{kind}` | `{"metadata": []}` | `204` |
| `DELETE /v1/zones/{id}/metadata/{kind}` | | `204` |

## History

`HistoryEntry`: `{"id", "at", "actor_type", "actor_name", "client_ip", "operation", "zone_name", "zone_id", "outcome", "upstream_status", "diff", "detail"}`

- `outcome` is `success` or `failure`. `zone_name`, `zone_id`, `upstream_status`, `diff`, and `detail` may be null.
- `diff` has one of two shapes:
  - Record changes: `[{"name", "type", "before": {"ttl", "records": [{"content", "disabled"}]} or null, "after": same or null}]`. `before` null means added, `after` null means removed.
  - Zone setting changes (`zone.update`): `{"zone": [{"field", "before", "after"}]}`.
- `detail` is the redacted request for operations without a diff, and `{"request", "error"}` for failures.

| Method and path | Response |
|---|---|
| `GET /v1/history?zone=&actor=&operation=&since=&until=&before_id=&limit=` | `{"items": HistoryEntry[]}`. Users see only their zones. Requires view_history. |

## Templates (manage_templates to change, any user to list)

`Template`: `{"id", "name", "description", "records": [{"name", "type", "ttl", "content"}]}`. Record names are relative to the zone: `@` for the apex, or a label such as `www`.

| Method and path | Body | Response |
|---|---|---|
| `GET /v1/templates` | | `Template[]` |
| `POST /v1/templates` | `Template` without id | `201 Template` |
| `PUT /v1/templates/{id}` | `Template` without id | `Template` |
| `DELETE /v1/templates/{id}` | | `204` |

## Administration (manage_users)

`User`: `{"id", "username", "display_name", "email", "role": "admin"or"operator"or"user", "auth_source": "local"or"oidc", "status": "active"or"pending"or"disabled", "totp_enabled", "created_at", "last_login_at", "zones": [], "account_ids": []}`

| Method and path | Body | Response |
|---|---|---|
| `GET /v1/users` | | `User[]` |
| `POST /v1/users` | `{"username", "display_name", "email", "role", "password"}` | `201 User` (local) |
| `PUT /v1/users/{id}` | `{"display_name", "email", "role", "status", "zones", "account_ids"}` | `User`. Changing role or disabling ends the user's sessions. |
| `POST /v1/users/{id}/password` | `{"password"}` | `204` |
| `POST /v1/users/{id}/revoke-sessions` | | `204` |
| `DELETE /v1/users/{id}/totp` | | `204` |
| `DELETE /v1/users/{id}` | | `204`. Disables the user's API keys. |

`Account`: `{"id", "name", "description", "zones": [], "user_ids": []}`

| Method and path | Body | Response |
|---|---|---|
| `GET /v1/accounts` | | `Account[]` (Admin and Operator see all; Users see their own) |
| `POST /v1/accounts` | `{"name", "description"}` | `201 Account` |
| `PUT /v1/accounts/{id}` | `{"name", "description", "zones", "user_ids"}` | `Account` |
| `DELETE /v1/accounts/{id}` | | `204` |

## API Keys

`ApiKey`: `{"id", "hint", "description", "role", "zones", "account_ids", "create_account_id", "networks", "expires_at", "last_used_at", "created_at", "disabled", "owner_user_id"}`

Admins see and manage all keys. Users with create_api_keys see and manage their own keys, which are always `scoped` and limited to zones the user can reach.

| Method and path | Body | Response |
|---|---|---|
| `GET /v1/api-keys` | | `ApiKey[]` |
| `POST /v1/api-keys` | `{"description", "role", "zones", "account_ids", "create_account_id", "networks", "expires_at"}` | `201 {"key": ApiKey, "secret": "..."}` |
| `PUT /v1/api-keys/{id}` | same as create | `ApiKey` |
| `POST /v1/api-keys/{id}/disable` | | `204` |
| `POST /v1/api-keys/{id}/enable` | | `204` |
| `DELETE /v1/api-keys/{id}` | | `204` |

## Settings (manage_settings)

`Settings`: `{"local_login_with_oidc": "allow"or"deny", "oidc_new_user": "create"or"approve", "user_can_create_zones", "user_can_delete_zones", "user_can_view_history", "user_can_manage_dnssec", "user_can_create_api_keys", "record_types_forward": [], "record_types_reverse": [], "auto_ptr"}`

| Method and path | Body | Response |
|---|---|---|
| `GET /v1/settings` | | `Settings` |
| `PUT /v1/settings` | partial `Settings` | `Settings` |
| `GET /v1/settings/oidc` | | `OidcSettings` |
| `PUT /v1/settings/oidc` | `{"enabled", "issuer", "client_id", "client_secret"?, "button_label"}` | `OidcSettings`. `409` when set by environment. `422` on validation errors. |

`OidcSettings`: `{"source": "environment"or"database", "enabled", "issuer", "client_id", "client_secret_set", "button_label", "redirect_uri"}`. The client secret is write-only: it is never returned. Omit `client_secret` or send it empty to keep the stored one. Saving is recorded in history as `settings.oidc_update` with `client_secret_changed` instead of the value.

## TSIG Keys (manage_tsig)

`TsigKey`: `{"id", "name", "algorithm", "key"}`. `key` is returned only by `GET /v1/tsigkeys/{id}` and on create.

| Method and path | Body | Response |
|---|---|---|
| `GET /v1/tsigkeys` | | `TsigKey[]` without `key` |
| `POST /v1/tsigkeys` | `{"name", "algorithm", "key"?}` | `201 TsigKey` with `key` |
| `GET /v1/tsigkeys/{id}` | | `TsigKey` with `key` |
| `PUT /v1/tsigkeys/{id}` | `{"algorithm"?, "key"?}` | `TsigKey` |
| `DELETE /v1/tsigkeys/{id}` | | `204` |
