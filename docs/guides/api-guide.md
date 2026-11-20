# API Guide

SOAbox serves the PowerDNS Authoritative 5.x HTTP API. Point any PowerDNS API client at SOAbox and use a SOAbox API key.

- Base URL: `https://<soabox-host>/api/v1`
- Header: `X-API-Key: <your SOAbox key>`
- Server id: `localhost` unless your administrator says otherwise.

The full API reference is the [PowerDNS HTTP API documentation](https://doc.powerdns.com/authoritative/http-api/).

## What A Key Can Do

| Key role | Zones | Server endpoints |
|---|---|---|
| Admin | All | All |
| Operator | All | Read statistics and config |
| Scoped | Only its zones and its accounts' zones. May create zones if allowed. | None |

A scoped key sees only its own zones in listings. Any other zone returns the same `404` as a zone that does not exist.

## Examples

```sh
curl -H "X-API-Key: $SOABOX_KEY" https://dns-admin.example.net/api/v1/servers/localhost/zones
```

### Terraform (PowerDNS provider)

```hcl
provider "powerdns" {
  server_url = "https://dns-admin.example.net"
  api_key    = var.soabox_key
}
```

### external-dns

```
--provider=pdns
--pdns-server=https://dns-admin.example.net
--pdns-api-key=<SOAbox key>
```

### octoDNS

```yaml
providers:
  powerdns:
    class: octodns_powerdns.PowerDnsProvider
    host: dns-admin.example.net
    port: 443
    scheme: https
    api_key: env/SOABOX_KEY
```

## Errors

Errors use the PowerDNS format `{"error": "..."}`. Two responses PowerDNS never sends can appear: `502` when SOAbox cannot reach PowerDNS, and `429` after repeated failed key attempts.
