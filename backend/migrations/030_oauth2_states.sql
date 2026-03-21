-- Temporary storage for OAuth2 state + PKCE code verifiers (short-lived, cleaned up on use)
CREATE TABLE IF NOT EXISTS oauth2_states (
    state VARCHAR(255) PRIMARY KEY,
    code_verifier VARCHAR(128) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_oauth2_states_expires ON oauth2_states(expires_at);
