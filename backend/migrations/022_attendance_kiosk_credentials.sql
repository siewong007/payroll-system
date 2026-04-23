-- Per-company kiosk credentials. A credential is a long random secret minted by an
-- admin and embedded in a public URL (/kiosk/{secret}) that a tablet opens to display
-- the rotating attendance QR. The secret is never stored in plaintext on the server —
-- only its sha256 hash. The first 8 chars are kept as a non-secret prefix so admins can
-- recognise which credential is which in the management UI.

CREATE TABLE attendance_kiosk_credentials (
    id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id   UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    label        VARCHAR(100) NOT NULL,
    token_hash   VARCHAR(128) NOT NULL,
    token_prefix VARCHAR(12)  NOT NULL,
    created_by   UUID NOT NULL REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    last_used_ip TEXT,
    revoked_at   TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_kiosk_credentials_token_hash
    ON attendance_kiosk_credentials(token_hash);

CREATE INDEX idx_kiosk_credentials_company_active
    ON attendance_kiosk_credentials(company_id)
    WHERE revoked_at IS NULL;
