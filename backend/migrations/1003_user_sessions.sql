-- Per-device login sessions. A refresh token belongs to exactly one session;
-- token rotation keeps the session identity stable so a user can revoke a
-- particular device without ending every login.
CREATE TABLE user_sessions (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_agent text,
    ip_address inet,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz
);

CREATE INDEX idx_user_sessions_user_active
    ON user_sessions (user_id, revoked_at, expires_at);

ALTER TABLE refresh_tokens ADD COLUMN session_id uuid;

-- Preserve existing logins during rollout. Historical tokens become one
-- session each; new refresh-token rotations retain their session id.
INSERT INTO user_sessions (id, user_id, created_at, last_seen_at, expires_at, revoked_at)
SELECT id, user_id, created_at, created_at, expires_at,
       CASE WHEN revoked THEN created_at ELSE NULL END
FROM refresh_tokens;

UPDATE refresh_tokens SET session_id = id WHERE session_id IS NULL;
ALTER TABLE refresh_tokens ALTER COLUMN session_id SET NOT NULL;
ALTER TABLE refresh_tokens
    ADD CONSTRAINT refresh_tokens_session_id_fkey
    FOREIGN KEY (session_id) REFERENCES user_sessions(id) ON DELETE CASCADE;
CREATE INDEX idx_refresh_tokens_session ON refresh_tokens (session_id);
