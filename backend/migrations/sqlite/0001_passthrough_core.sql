-- Milestone 1: accounts, API keys, history, settings.
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE TABLE account_zones (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    zone_name TEXT NOT NULL UNIQUE,
    PRIMARY KEY (account_id, zone_name)
);

CREATE TABLE api_keys (
    id INTEGER PRIMARY KEY,
    key_hash TEXT NOT NULL UNIQUE,
    hint TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    role TEXT NOT NULL CHECK (role IN ('admin', 'operator', 'scoped')),
    create_account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    owner_user_id INTEGER,
    expires_at INTEGER,
    last_used_at INTEGER,
    created_at INTEGER NOT NULL,
    disabled INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE api_key_zones (
    api_key_id INTEGER NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    zone_name TEXT NOT NULL,
    PRIMARY KEY (api_key_id, zone_name)
);

CREATE TABLE api_key_accounts (
    api_key_id INTEGER NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    PRIMARY KEY (api_key_id, account_id)
);

CREATE TABLE api_key_networks (
    api_key_id INTEGER NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    cidr TEXT NOT NULL,
    PRIMARY KEY (api_key_id, cidr)
);

CREATE TABLE history (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id INTEGER,
    actor_name TEXT NOT NULL,
    client_ip TEXT NOT NULL DEFAULT '',
    operation TEXT NOT NULL,
    zone_name TEXT,
    outcome TEXT NOT NULL,
    upstream_status INTEGER,
    diff_json TEXT,
    detail_json TEXT
);
CREATE INDEX history_at ON history (at);
CREATE INDEX history_zone ON history (zone_name, at);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
