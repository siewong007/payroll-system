-- Passkey (WebAuthn) credentials
CREATE TABLE IF NOT EXISTS passkey_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_name VARCHAR(255) NOT NULL DEFAULT 'My Passkey',
    credential_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_passkey_credentials_user ON passkey_credentials(user_id);

-- Temporary challenge storage for WebAuthn ceremonies
CREATE TABLE IF NOT EXISTS passkey_challenges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    challenge_type VARCHAR(20) NOT NULL, -- 'registration' or 'authentication'
    state_json JSONB NOT NULL,
    email VARCHAR(255),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '5 minutes',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_passkey_challenges_user ON passkey_challenges(user_id);
CREATE INDEX idx_passkey_challenges_email ON passkey_challenges(email);
