-- Audit fix: remember the last accepted TOTP time step to stop code replay.
ALTER TABLE users ADD COLUMN totp_last_step BIGINT NOT NULL DEFAULT 0;
