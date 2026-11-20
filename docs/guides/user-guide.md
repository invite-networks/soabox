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

Each record type has its own fields. For example, MX asks for a priority and a mail server, and SOA asks for the primary name server, contact email, serial, and timers. Number fields accept only digits, and name fields accept only characters valid in a host name.

Host names follow zone file rules:

- A name ending in a dot is absolute. `mail.example.net.` stays as typed.
- `@` is the zone itself.
- Any other name is relative to the zone. In `example.com.`, `mail` becomes `mail.example.com.`.

While you type, the zone that will be appended is shown after the field. In the record table, the zone part of values inside the zone is shown in a lighter color so you can see what each name resolves to.

In templates, relative names stay relative and are completed with the new zone's name when a zone is created from the template.

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
