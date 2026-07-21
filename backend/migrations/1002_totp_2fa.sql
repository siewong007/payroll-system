-- TOTP-based two-factor authentication (Google Authenticator compatible).
--
-- A row in user_totp with enabled = false represents an in-progress setup
-- (secret generated, not yet confirmed); enabled = true is the live state
-- checked at login. secret_encrypted is AES-256-GCM ciphertext, base64-
-- encoded, keyed off JWT_SECRET (see backend/src/core/crypto.rs).

CREATE TABLE public.user_totp (
    id uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    user_id uuid NOT NULL UNIQUE REFERENCES public.users(id) ON DELETE CASCADE,
    secret_encrypted text NOT NULL,
    enabled boolean DEFAULT false NOT NULL,
    confirmed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.user_totp_backup_codes (
    id uuid DEFAULT uuidv7() NOT NULL PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES public.users(id) ON DELETE CASCADE,
    code_hash character varying(255) NOT NULL,
    used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE INDEX idx_user_totp_backup_codes_user ON public.user_totp_backup_codes(user_id);
