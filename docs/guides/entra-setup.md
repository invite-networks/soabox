# Microsoft Entra ID Setup

Status: **Planned, not yet executed** against a live tenant.

You need an Admin account in SOAbox and permission to create app registrations in Entra.

## 1. Copy The Redirect URI From SOAbox

Sign in to SOAbox as an Admin and open **Settings**. The **Single sign-on** card shows the **Redirect URI**, for example `https://<soabox-host>/auth/oidc/callback`. Use **Copy redirect URI**. It is built from `SOABOX_PUBLIC_URL`, so check that variable if the address looks wrong.

## 2. Register The App In The Entra Admin Center

1. Go to **App registrations** and choose **New registration**.
2. Name it, for example `SOAbox`. Choose **Accounts in this organizational directory only**.
3. Add a **Web** redirect URI and paste the value from step 1.
4. Under **Certificates & secrets**, create a client secret and copy its **Value** (not the secret ID).
5. Under **API permissions**, keep `openid`, `profile`, and `email` (Microsoft Graph, delegated).
6. Copy the **Application (client) ID** and **Directory (tenant) ID** from the overview.

## 3. Enter The Settings In SOAbox

In **Settings > Single sign-on**:

| Field | Value |
|---|---|
| Single sign-on enabled | On |
| Issuer URL | `https://login.microsoftonline.com/<tenant-id>/v2.0` |
| Client ID | The application (client) ID |
| Client secret | The secret value |
| Button label | For example `Sign in with Microsoft` |

Choose **Save single sign-on**. No restart is needed. Open the sign-in page in a private window to check that the button appears, and sign in once to confirm.

After saving, the secret is never shown again. The card shows **Secret is set**. To rotate it, create a new secret in Entra, choose **Replace secret**, paste the new value, save, test sign-in, and then delete the old secret in Entra.

## After The First Sign-In

New users start with the User role and no zones, or wait for approval, depending on the **New single sign-on users** setting. Admins assign roles, accounts, and zones in SOAbox. Entra groups and app roles are not used.

If you set **Local sign-in when single sign-on is on** to deny, keep at least one local Admin so you can still reach Settings if the provider is unavailable.

## Configuring With Environment Variables Instead

Deployments managed as code can still set these variables. When `SOABOX_OIDC_ISSUER` is set, they take precedence over the UI, which then shows the values read-only with the note **Set by environment**. A restart is needed after changing them.

```
SOABOX_OIDC_ISSUER=https://login.microsoftonline.com/<tenant-id>/v2.0
SOABOX_OIDC_CLIENT_ID=<application id>
SOABOX_OIDC_CLIENT_SECRET=<secret value>
SOABOX_OIDC_BUTTON_LABEL=Sign in with Microsoft
```

The UI cannot be edited while the variables are set. To move from variables to the UI, remove the variables, restart, and then enter the same values under **Settings > Single sign-on**. Users keep their accounts as long as the issuer and client stay the same.

## Other Providers

Any OpenID Connect provider works the same way. Set its issuer URL. SOAbox matches users on issuer and subject.
