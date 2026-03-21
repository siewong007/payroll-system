-- OAuth2 linked accounts: maps external provider identities to local users
CREATE TABLE IF NOT EXISTS oauth2_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider VARCHAR(50) NOT NULL,           -- 'google', 'microsoft', etc.
    provider_user_id VARCHAR(255) NOT NULL,  -- sub claim from provider
    provider_email VARCHAR(255),
    provider_name VARCHAR(255),
    avatar_url TEXT,
    access_token_hash VARCHAR(255),
    refresh_token_hash VARCHAR(255),
    token_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider, provider_user_id)
);

CREATE INDEX IF NOT EXISTS idx_oauth2_accounts_user ON oauth2_accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_oauth2_accounts_provider ON oauth2_accounts(provider, provider_user_id);
