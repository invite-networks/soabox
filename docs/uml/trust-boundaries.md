# Data Flow And Trust Boundaries

Supports [design section 7](../spec/design.md#7-security-boundaries).

```mermaid
flowchart LR
    subgraph internet["Untrusted: clients"]
        browser["Browser"]
        client["API client"]
    end
    subgraph edge["Edge"]
        proxy["Reverse proxy<br>TLS"]
    end
    subgraph app["Trusted: SOAbox"]
        soabox["SOAbox<br>keys, sessions, authz, audit"]
    end
    subgraph private["Private network"]
        pdns["PowerDNS API"]
        db[("Store")]
    end
    idp["Identity provider"]

    browser -->|"session cookie, CSRF"| proxy
    client -->|"SOAbox API key"| proxy
    proxy -->|"X-Forwarded-For, trusted CIDRs only"| soabox
    soabox -->|"SOAbox PowerDNS key"| pdns
    soabox -->|"parameterized SQL"| db
    soabox -->|"code exchange, PKCE"| idp
```

Data that never crosses outward: the PowerDNS key, password hashes, key digests, TOTP secrets, and session ids.
