-- Anti-buddy-punching hardening for attendance:
--   1. Combined QR + Face ID method ('qr_and_face_id') and Kiosk Photo ('kiosk_photo').
--   2. Company-level device binding and photo audit capture policies.
--   3. Per-employee WebCrypto P-256 bound devices, short-lived challenges, and
--      single-use personal kiosk pass QR tokens.
--   4. Forensic session/device/cookie/passkey/UA metadata, anomaly flags, and
--      auto-purged photo audit columns on attendance_records.
--   5. Fast expiry index on attendance_qr_tokens for 30s kiosk rotation cleanup.

-- ─── 1. Extend attendance method CHECK constraints ───

ALTER TABLE public.companies
    DROP CONSTRAINT IF EXISTS companies_attendance_method_check;

ALTER TABLE public.companies
    ADD CONSTRAINT companies_attendance_method_check
    CHECK (attendance_method IN ('qr_code', 'face_id', 'qr_and_face_id'));

ALTER TABLE public.attendance_records
    DROP CONSTRAINT IF EXISTS attendance_records_method_check;

ALTER TABLE public.attendance_records
    ADD CONSTRAINT attendance_records_method_check
    CHECK (method IN ('qr_code', 'face_id', 'qr_and_face_id', 'kiosk_photo', 'manual'));

-- ─── 2. Company-level attendance security policies ───

ALTER TABLE public.companies
    ADD COLUMN IF NOT EXISTS attendance_device_mode character varying(20) DEFAULT 'none' NOT NULL,
    ADD COLUMN IF NOT EXISTS attendance_device_auto_approve_first boolean DEFAULT true NOT NULL,
    ADD COLUMN IF NOT EXISTS attendance_photo_mode character varying(20) DEFAULT 'none' NOT NULL,
    ADD COLUMN IF NOT EXISTS attendance_photo_retention_days integer DEFAULT 30 NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'companies_attendance_device_mode_check'
          AND conrelid = 'public.companies'::regclass
    ) THEN
        ALTER TABLE public.companies
            ADD CONSTRAINT companies_attendance_device_mode_check
            CHECK (attendance_device_mode IN ('none', 'warn', 'enforce'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'companies_attendance_photo_mode_check'
          AND conrelid = 'public.companies'::regclass
    ) THEN
        ALTER TABLE public.companies
            ADD CONSTRAINT companies_attendance_photo_mode_check
            CHECK (attendance_photo_mode IN ('none', 'optional', 'required'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'companies_attendance_photo_retention_days_check'
          AND conrelid = 'public.companies'::regclass
    ) THEN
        ALTER TABLE public.companies
            ADD CONSTRAINT companies_attendance_photo_retention_days_check
            CHECK (attendance_photo_retention_days BETWEEN 7 AND 90);
    END IF;
END
$$;

-- ─── 3. Employee bound attendance devices & challenges ───

CREATE TABLE IF NOT EXISTS public.employee_attendance_devices (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    employee_id uuid NOT NULL,
    user_id uuid NOT NULL,
    public_key_raw text NOT NULL,
    public_key_fingerprint character varying(64) NOT NULL,
    device_label character varying(150) NOT NULL,
    user_agent character varying(500),
    cookie_device_id uuid,
    status character varying(20) DEFAULT 'pending' NOT NULL,
    approved_by uuid,
    approved_at timestamp with time zone,
    revoked_by uuid,
    revoked_at timestamp with time zone,
    revoke_reason character varying(400),
    last_used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT employee_attendance_devices_pkey PRIMARY KEY (id),
    CONSTRAINT employee_attendance_devices_company_id_fkey
        FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE,
    CONSTRAINT employee_attendance_devices_employee_id_fkey
        FOREIGN KEY (employee_id) REFERENCES public.employees(id) ON DELETE CASCADE,
    CONSTRAINT employee_attendance_devices_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE,
    CONSTRAINT employee_attendance_devices_approved_by_fkey
        FOREIGN KEY (approved_by) REFERENCES public.users(id) ON DELETE SET NULL,
    CONSTRAINT employee_attendance_devices_revoked_by_fkey
        FOREIGN KEY (revoked_by) REFERENCES public.users(id) ON DELETE SET NULL,
    CONSTRAINT employee_attendance_devices_status_check
        CHECK (status IN ('pending', 'approved', 'revoked'))
);

CREATE UNIQUE INDEX IF NOT EXISTS employee_attendance_devices_one_approved_per_employee
    ON public.employee_attendance_devices (employee_id)
    WHERE status = 'approved';

CREATE UNIQUE INDEX IF NOT EXISTS employee_attendance_devices_unique_approved_key_per_company
    ON public.employee_attendance_devices (company_id, public_key_fingerprint)
    WHERE status = 'approved';

CREATE INDEX IF NOT EXISTS idx_employee_attendance_devices_company_status
    ON public.employee_attendance_devices (company_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_employee_attendance_devices_employee
    ON public.employee_attendance_devices (employee_id, created_at DESC);

CREATE TABLE IF NOT EXISTS public.attendance_device_challenges (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    user_id uuid NOT NULL,
    employee_id uuid NOT NULL,
    nonce character varying(64) NOT NULL,
    purpose character varying(30) NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT attendance_device_challenges_pkey PRIMARY KEY (id),
    CONSTRAINT attendance_device_challenges_company_id_fkey
        FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE,
    CONSTRAINT attendance_device_challenges_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE,
    CONSTRAINT attendance_device_challenges_employee_id_fkey
        FOREIGN KEY (employee_id) REFERENCES public.employees(id) ON DELETE CASCADE,
    CONSTRAINT attendance_device_challenges_purpose_check
        CHECK (purpose IN ('check_in', 'check_out', 'personal_qr'))
);

CREATE INDEX IF NOT EXISTS idx_attendance_device_challenges_expires_at
    ON public.attendance_device_challenges (expires_at);

-- ─── 4. Personal rotating QR tokens (for kiosk camera check-in) ───

CREATE TABLE IF NOT EXISTS public.attendance_personal_qr_tokens (
    id uuid DEFAULT uuidv7() NOT NULL,
    company_id uuid NOT NULL,
    employee_id uuid NOT NULL,
    user_id uuid NOT NULL,
    token_hash character varying(64) NOT NULL,
    action character varying(20) NOT NULL,
    device_id uuid,
    cookie_device_id uuid,
    session_id uuid,
    passkey_id uuid,
    user_agent character varying(500),
    used boolean DEFAULT false NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT attendance_personal_qr_tokens_pkey PRIMARY KEY (id),
    CONSTRAINT attendance_personal_qr_tokens_token_hash_key UNIQUE (token_hash),
    CONSTRAINT attendance_personal_qr_tokens_company_id_fkey
        FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE,
    CONSTRAINT attendance_personal_qr_tokens_employee_id_fkey
        FOREIGN KEY (employee_id) REFERENCES public.employees(id) ON DELETE CASCADE,
    CONSTRAINT attendance_personal_qr_tokens_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE,
    CONSTRAINT attendance_personal_qr_tokens_action_check
        CHECK (action IN ('check_in', 'check_out'))
);

CREATE INDEX IF NOT EXISTS idx_attendance_personal_qr_expires
    ON public.attendance_personal_qr_tokens (expires_at);

-- ─── 5. Forensic, anomaly, and photo audit columns on attendance_records ───
-- Bare UUID columns (no FKs to user_sessions / passkey_credentials /
-- employee_attendance_devices) so retention purges or credential revocations
-- never break or erase historical attendance evidence.

ALTER TABLE public.attendance_records
    ADD COLUMN IF NOT EXISTS check_in_session_id uuid,
    ADD COLUMN IF NOT EXISTS check_out_session_id uuid,
    ADD COLUMN IF NOT EXISTS check_in_device_id uuid,
    ADD COLUMN IF NOT EXISTS check_out_device_id uuid,
    ADD COLUMN IF NOT EXISTS check_in_cookie_id uuid,
    ADD COLUMN IF NOT EXISTS check_out_cookie_id uuid,
    ADD COLUMN IF NOT EXISTS check_in_passkey_id uuid,
    ADD COLUMN IF NOT EXISTS check_in_user_agent character varying(500),
    ADD COLUMN IF NOT EXISTS check_out_user_agent character varying(500),
    ADD COLUMN IF NOT EXISTS is_untrusted_device boolean,
    ADD COLUMN IF NOT EXISTS anomaly_flags text[] DEFAULT '{}'::text[] NOT NULL,
    ADD COLUMN IF NOT EXISTS check_in_photo_url text,
    ADD COLUMN IF NOT EXISTS check_out_photo_url text,
    ADD COLUMN IF NOT EXISTS check_in_photo_purged_at timestamp with time zone,
    ADD COLUMN IF NOT EXISTS check_out_photo_purged_at timestamp with time zone;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'attendance_records_check_in_photo_url_no_traversal'
          AND conrelid = 'public.attendance_records'::regclass
    ) THEN
        ALTER TABLE public.attendance_records
            ADD CONSTRAINT attendance_records_check_in_photo_url_no_traversal
            CHECK (
                check_in_photo_url IS NULL
                OR (
                    check_in_photo_url ~ '^/api/uploads/[A-Za-z0-9._-]+$'
                    AND check_in_photo_url !~ '\.\.'
                )
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'attendance_records_check_out_photo_url_no_traversal'
          AND conrelid = 'public.attendance_records'::regclass
    ) THEN
        ALTER TABLE public.attendance_records
            ADD CONSTRAINT attendance_records_check_out_photo_url_no_traversal
            CHECK (
                check_out_photo_url IS NULL
                OR (
                    check_out_photo_url ~ '^/api/uploads/[A-Za-z0-9._-]+$'
                    AND check_out_photo_url !~ '\.\.'
                )
            );
    END IF;
END
$$;

CREATE INDEX IF NOT EXISTS idx_attendance_records_company_anomalies
    ON public.attendance_records (company_id, check_in_at DESC)
    WHERE cardinality(anomaly_flags) > 0;

CREATE INDEX IF NOT EXISTS idx_attendance_records_company_cookie_recent
    ON public.attendance_records (company_id, check_in_cookie_id, check_in_at DESC)
    WHERE check_in_cookie_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_attendance_records_company_checkout_cookie
    ON public.attendance_records (company_id, check_out_cookie_id, check_out_at DESC)
    WHERE check_out_cookie_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_attendance_records_check_in_photo
    ON public.attendance_records (company_id, check_in_photo_url)
    WHERE check_in_photo_url IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_attendance_records_check_out_photo
    ON public.attendance_records (company_id, check_out_photo_url)
    WHERE check_out_photo_url IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_attendance_qr_tokens_expires_at
    ON public.attendance_qr_tokens (expires_at);
