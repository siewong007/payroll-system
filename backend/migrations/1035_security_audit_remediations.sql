-- Security audit remediation migration:
--   1. Persist creator/editor attribution (`created_by`, `updated_by`) on
--      `claims`, `leave_requests`, and `overtime_applications` for four-eyes
--      maker-checker enforcement.
--   2. Snapshot payee banking details (`bank_name`, `bank_account_number`,
--      `bank_account_type`) onto `payroll_items` at run generation time so
--      post-approval employee edits cannot divert disbursement CSVs.
--   3. Add `mfa_challenges` for single-use, server-tracked pre-2FA tokens.
--   4. Add an isolated per-account lockout bucket on `user_totp` for
--      `/auth/login/code` without affecting normal password+2FA logins.

-- 1. Approval creator/editor attribution.
ALTER TABLE public.claims
    ADD COLUMN created_by uuid REFERENCES public.users(id) ON DELETE SET NULL,
    ADD COLUMN updated_by uuid REFERENCES public.users(id) ON DELETE SET NULL;

ALTER TABLE public.leave_requests
    ADD COLUMN created_by uuid REFERENCES public.users(id) ON DELETE SET NULL,
    ADD COLUMN updated_by uuid REFERENCES public.users(id) ON DELETE SET NULL;

ALTER TABLE public.overtime_applications
    ADD COLUMN created_by uuid REFERENCES public.users(id) ON DELETE SET NULL,
    ADD COLUMN updated_by uuid REFERENCES public.users(id) ON DELETE SET NULL;

-- 2. Payee banking snapshot on `payroll_items` + backfill from current employee records.
ALTER TABLE public.payroll_items
    ADD COLUMN bank_name character varying(100),
    ADD COLUMN bank_account_number character varying(50),
    ADD COLUMN bank_account_type character varying(30);

UPDATE public.payroll_items pi
SET bank_name = e.bank_name,
    bank_account_number = e.bank_account_number,
    bank_account_type = e.bank_account_type
FROM public.employees e
WHERE pi.employee_id = e.id
  AND pi.bank_name IS NULL
  AND pi.bank_account_number IS NULL
  AND pi.bank_account_type IS NULL;

-- 3. Server-tracked single-use pre-2FA challenges.
CREATE TABLE public.mfa_challenges (
    jti uuid NOT NULL PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES public.users(id) ON DELETE CASCADE,
    attempts integer NOT NULL DEFAULT 0,
    expires_at timestamp with time zone NOT NULL,
    consumed_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL DEFAULT now()
);

CREATE INDEX idx_mfa_challenges_user_id
    ON public.mfa_challenges (user_id);

CREATE INDEX idx_mfa_challenges_expires_at
    ON public.mfa_challenges (expires_at);

-- 4. Isolated lockout state for `/auth/login/code`.
ALTER TABLE public.user_totp
    ADD COLUMN code_login_failed_attempts integer NOT NULL DEFAULT 0,
    ADD COLUMN code_login_locked_until timestamp with time zone;
