# Data Model

Supports [design section 4](../spec/design.md#4-data-model).

```mermaid
erDiagram
    accounts ||--o{ account_zones : owns
    accounts ||--o{ account_users : has
    users ||--o{ account_users : belongs
    users ||--o{ user_zones : assigned
    users ||--o{ sessions : has
    users ||--o{ api_keys : owns
    api_keys ||--o{ api_key_zones : scoped
    api_keys ||--o{ api_key_accounts : scoped
    accounts ||--o{ api_key_accounts : "in scope"
    api_keys ||--o{ api_key_networks : allows
    accounts |o--o{ api_keys : "creates zones in"
    templates ||--o{ template_records : contains

    accounts {
        int id PK
        text name UK
        text description
    }
    account_zones {
        int account_id FK
        text zone_name UK
    }
    users {
        int id PK
        text username UK
        text role
        text auth_source
        text oidc_issuer
        text oidc_subject
        text password_hash
        blob totp_secret_enc
        text status
    }
    sessions {
        text id_hash PK
        int user_id FK
        text csrf_token
        timestamp expires_at
    }
    api_keys {
        int id PK
        text key_hash UK
        text hint
        text role
        int create_account_id FK
        int owner_user_id FK
        timestamp expires_at
        bool disabled
    }
    api_key_zones {
        int api_key_id FK
        text zone_name
    }
    api_key_accounts {
        int api_key_id FK
        int account_id FK
    }
    api_key_networks {
        int api_key_id FK
        text cidr
    }
    user_zones {
        int user_id FK
        text zone_name
    }
    account_users {
        int account_id FK
        int user_id FK
    }
    templates {
        int id PK
        text name UK
    }
    template_records {
        int template_id FK
        text name
        text type
        int ttl
        text content
    }
    history {
        int id PK
        timestamp at
        text actor_type
        text actor_name
        text client_ip
        text operation
        text zone_name
        text outcome
        text diff_json
    }
    settings {
        text key PK
        text value
    }
```

`history` and `settings` have no foreign keys on purpose. History must survive deletion of the actor or zone it describes.
