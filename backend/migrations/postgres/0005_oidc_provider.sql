-- Single sign-on provider settings managed in the UI. One row at most.
-- The client secret is encrypted with SOABOX_SECRET_KEY.
CREATE TABLE oidc_provider (
    id BIGINT PRIMARY KEY CHECK (id = 1),
    enabled BIGINT NOT NULL DEFAULT 0,
    issuer TEXT NOT NULL DEFAULT '',
    client_id TEXT NOT NULL DEFAULT '',
    client_secret_enc BYTEA,
    button_label TEXT NOT NULL DEFAULT '',
    updated_at BIGINT NOT NULL
);
