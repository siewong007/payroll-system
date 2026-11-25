-- Phase 3 hardening & reliability:
--   1. O(1) keyed SHA-256 lookup index for TOTP backup codes (Item 38b)
--   2. Email outbox retry tracking columns & due-queue index (Item 41)
--   3. Non-negative salary/allowance and chronological date CHECK constraints
--      on employees (Item 42)

-- 1. TOTP backup-code keyed lookup hash (legacy rows remain NULL and use
--    the fallback scan so pre-migration backup codes stay valid).
ALTER TABLE public.user_totp_backup_codes
    ADD COLUMN IF NOT EXISTS code_lookup_hash VARCHAR(64);

CREATE INDEX IF NOT EXISTS idx_user_totp_backup_codes_lookup
    ON public.user_totp_backup_codes (user_id, code_lookup_hash)
    WHERE used_at IS NULL AND code_lookup_hash IS NOT NULL;

-- 2. Email outbox retry state & partial due index.
ALTER TABLE public.email_logs
    ADD COLUMN IF NOT EXISTS attempts INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS max_attempts INTEGER NOT NULL DEFAULT 3,
    ADD COLUMN IF NOT EXISTS next_retry_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_email_logs_outbox_due
    ON public.email_logs (next_retry_at, created_at)
    WHERE status = 'pending' AND next_retry_at IS NOT NULL;

-- 3. Employees domain invariants (added NOT VALID then validated cleanly).
ALTER TABLE public.employees
    ADD CONSTRAINT employees_basic_salary_non_negative_check
        CHECK (basic_salary >= 0) NOT VALID,
    ADD CONSTRAINT employees_hourly_rate_non_negative_check
        CHECK (hourly_rate IS NULL OR hourly_rate >= 0) NOT VALID,
    ADD CONSTRAINT employees_daily_rate_non_negative_check
        CHECK (daily_rate IS NULL OR daily_rate >= 0) NOT VALID,
    ADD CONSTRAINT employees_num_children_range_check
        CHECK (num_children IS NULL OR (num_children >= 0 AND num_children <= 30)) NOT VALID,
    ADD CONSTRAINT employees_zakat_non_negative_check
        CHECK (zakat_monthly_amount IS NULL OR zakat_monthly_amount >= 0) NOT VALID,
    ADD CONSTRAINT employees_ptptn_non_negative_check
        CHECK (ptptn_monthly_amount IS NULL OR ptptn_monthly_amount >= 0) NOT VALID,
    ADD CONSTRAINT employees_tabung_haji_non_negative_check
        CHECK (tabung_haji_amount IS NULL OR tabung_haji_amount >= 0) NOT VALID,
    ADD CONSTRAINT employees_date_resigned_after_joined_check
        CHECK (date_resigned IS NULL OR date_resigned >= date_joined) NOT VALID,
    ADD CONSTRAINT employees_probation_dates_order_check
        CHECK (probation_start IS NULL OR probation_end IS NULL OR probation_end >= probation_start) NOT VALID,
    ADD CONSTRAINT employees_confirmation_after_joined_check
        CHECK (confirmation_date IS NULL OR confirmation_date >= date_joined) NOT VALID;

ALTER TABLE public.employees VALIDATE CONSTRAINT employees_basic_salary_non_negative_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_hourly_rate_non_negative_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_daily_rate_non_negative_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_num_children_range_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_zakat_non_negative_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_ptptn_non_negative_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_tabung_haji_non_negative_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_date_resigned_after_joined_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_probation_dates_order_check;
ALTER TABLE public.employees VALIDATE CONSTRAINT employees_confirmation_after_joined_check;
