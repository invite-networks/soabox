# User Guide

## Signing In

Use your local username and password, or the single sign-on button if your administrator enabled it. If your account needs approval, an Admin will enable it.

## What You Can See

- Admins and Operators see every zone.
- Users see zones assigned to them and zones of their accounts.

## Managing Records

1. Open a zone from the Zones list.
2. Add, edit, disable, or delete records in the table. Changes are collected as pending.
3. Review the pending changes and apply them. They are sent to PowerDNS as one change and recorded in history.

## Creating And Deleting Zones

Admins and Operators can always create and delete zones. Users can if the settings allow it. A zone can start from a template.

## DNSSEC

From a zone's DNSSEC tab, enable signing, view DS records to give to your registrar, and manage keys.

## History

Each zone has a History tab showing who changed what and when. Admins and Operators also see a global history page.

## API Keys

If allowed, create your own API keys under your profile. A key is shown once. Copy it then.

## Limitations

- One PowerDNS server.
- Changes made directly on PowerDNS, not through SOAbox, are not attributed to a person.
