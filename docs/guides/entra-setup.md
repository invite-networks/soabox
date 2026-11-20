# Microsoft Entra ID Setup

Status: **Planned, not yet executed** against a live tenant.

## In The Entra Admin Center

1. Go to **App registrations** and choose **New registration**.
2. Name it, for example `SOAbox`. Choose **Accounts in this organizational directory only**.
3. Add a **Web** redirect URI: `https://<soabox-host>/auth/oidc/callback`.
4. Under **Certificates & secrets**, create a client secret and copy its value.
5. Under **API permissions**, keep `openid`, `profile`, and `email` (Microsoft Graph, delegated).
6. Copy the **Application (client) ID** and **Directory (tenant) ID** from the overview.

## In SOAbox

```
SOABOX_OIDC_ISSUER=https://login.microsoftonline.com/<tenant-id>/v2.0
SOABOX_OIDC_CLIENT_ID=<application id>
SOABOX_OIDC_CLIENT_SECRET=<secret value>
SOABOX_OIDC_BUTTON_LABEL=Sign in with Microsoft
```

Restart SOAbox. The sign-in page shows the button.

## After The First Sign-In

New users start with the User role and no zones, or wait for approval, depending on the **New OIDC users** setting. Admins assign roles, accounts, and zones in SOAbox. Entra groups and app roles are not used.

## Other Providers

Any OpenID Connect provider works the same way. Set its issuer URL. SOAbox matches users on issuer and subject.
