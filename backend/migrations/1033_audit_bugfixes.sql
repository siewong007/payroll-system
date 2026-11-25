-- Audit remediation migration:
--   1. Relax salary_history_approval_pair_check so ON DELETE SET NULL on
--      approved_by (migration 1030) does not violate migration 1025's check.
--   2. Add revoked_at to refresh_tokens for accurate token-level rotation grace.
--   3. Add taxable_gross to payroll_items so YTD PCB and EA Form exclude
--      unpaid leave and non-taxable allowances.
--   4. Add overtime_application_id and leave_request_id FKs to payroll_entries
--      so cancellations never delete unrelated same-day entries.
--   5. Drop DEFAULT 'A' on employees.epf_category so omitted/cleared categories
--      auto-derive by age, residency, and date.
--   6. Add positive/non-negative CHECK constraints on claims and tp3_records.
--   7. Add unique index on oauth2_accounts (user_id, provider) when clean.
--   8. Add missing retention/health/listing indexes.

-- 1. salary_history approval pair check compatible with ON DELETE SET NULL.
ALTER TABLE public.salary_history
    DROP CONSTRAINT IF EXISTS salary_history_approval_pair_check;

ALTER TABLE public.salary_history
    ADD CONSTRAINT salary_history_approval_pair_check
    CHECK (approved_by IS NULL OR approved_at IS NOT NULL);

-- 2. refresh_tokens revocation timestamp.
ALTER TABLE public.refresh_tokens
    ADD COLUMN IF NOT EXISTS revoked_at TIMESTAMPTZ;

-- 3. payroll_items taxable gross.
ALTER TABLE public.payroll_items
    ADD COLUMN IF NOT EXISTS taxable_gross BIGINT;

-- 4. payroll_entries explicit linkage to overtime_applications / leave_requests.
ALTER TABLE public.payroll_entries
    ADD COLUMN IF NOT EXISTS overtime_application_id UUID REFERENCES public.overtime_applications(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS leave_request_id UUID REFERENCES public.leave_requests(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_payroll_entries_overtime_app
    ON public.payroll_entries (overtime_application_id)
    WHERE overtime_application_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_payroll_entries_leave_req
    ON public.payroll_entries (leave_request_id)
    WHERE leave_request_id IS NOT NULL;

-- 5. Drop DEFAULT 'A' on employees.epf_category.
ALTER TABLE public.employees
    ALTER COLUMN epf_category DROP DEFAULT;

-- 6. Claims and TP3 domain invariants (NOT VALID + guarded VALIDATE).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'claims_amount_positive_check'
          AND conrelid = 'public.claims'::regclass
    ) THEN
        ALTER TABLE public.claims
            ADD CONSTRAINT claims_amount_positive_check
            CHECK (amount > 0) NOT VALID;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'tp3_records_tax_year_check'
          AND conrelid = 'public.tp3_records'::regclass
    ) THEN
        ALTER TABLE public.tp3_records
            ADD CONSTRAINT tp3_records_tax_year_check
            CHECK (tax_year BETWEEN 2000 AND 2200) NOT VALID;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'tp3_records_ytd_non_negative_check'
          AND conrelid = 'public.tp3_records'::regclass
    ) THEN
        ALTER TABLE public.tp3_records
            ADD CONSTRAINT tp3_records_ytd_non_negative_check
            CHECK (
                previous_income_ytd >= 0
                AND previous_epf_ytd >= 0
                AND previous_pcb_ytd >= 0
                AND previous_socso_ytd >= 0
                AND previous_zakat_ytd >= 0
            ) NOT VALID;
    END IF;
END $$;

DO $$
DECLARE
    bad bigint;
BEGIN
    SELECT count(*) INTO bad FROM public.claims WHERE amount <= 0;
    IF bad = 0 THEN
        ALTER TABLE public.claims VALIDATE CONSTRAINT claims_amount_positive_check;
    ELSE
        RAISE WARNING 'claims_amount_positive_check left NOT VALID: % row(s)', bad;
    END IF;

    SELECT count(*) INTO bad FROM public.tp3_records WHERE tax_year < 2000 OR tax_year > 2200;
    IF bad = 0 THEN
        ALTER TABLE public.tp3_records VALIDATE CONSTRAINT tp3_records_tax_year_check;
    ELSE
        RAISE WARNING 'tp3_records_tax_year_check left NOT VALID: % row(s)', bad;
    END IF;

    SELECT count(*) INTO bad FROM public.tp3_records
    WHERE previous_income_ytd < 0
       OR previous_epf_ytd < 0
       OR previous_pcb_ytd < 0
       OR previous_socso_ytd < 0
       OR previous_zakat_ytd < 0;
    IF bad = 0 THEN
        ALTER TABLE public.tp3_records VALIDATE CONSTRAINT tp3_records_ytd_non_negative_check;
    ELSE
        RAISE WARNING 'tp3_records_ytd_non_negative_check left NOT VALID: % row(s)', bad;
    END IF;
END $$;

-- 7. Single OAuth2 link per (user_id, provider).
DO $$
DECLARE
    dups bigint;
BEGIN
    SELECT count(*) INTO dups FROM (
        SELECT 1 FROM public.oauth2_accounts
        GROUP BY user_id, provider
        HAVING count(*) > 1
    ) t;
    IF dups = 0 THEN
        CREATE UNIQUE INDEX IF NOT EXISTS idx_oauth2_accounts_user_provider
            ON public.oauth2_accounts (user_id, provider);
    ELSE
        RAISE WARNING 'idx_oauth2_accounts_user_provider skipped: % duplicate (user_id, provider) group(s)', dups;
    END IF;
END $$;

-- 8. Retention cleanup, health, and listing indexes.
CREATE INDEX IF NOT EXISTS idx_email_logs_company_created
    ON public.email_logs (company_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_email_logs_created_at
    ON public.email_logs (created_at);

CREATE INDEX IF NOT EXISTS idx_email_logs_failed_created
    ON public.email_logs (created_at)
    WHERE status = 'failed';

CREATE INDEX IF NOT EXISTS idx_user_sessions_revoked_at
    ON public.user_sessions (revoked_at)
    WHERE revoked_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_user_sessions_last_seen_at
    ON public.user_sessions (last_seen_at);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_created_at
    ON public.refresh_tokens (created_at);

CREATE INDEX IF NOT EXISTS idx_bulk_import_sessions_expires_at
    ON public.bulk_import_sessions (expires_at);

CREATE INDEX IF NOT EXISTS idx_passkey_challenges_expires_at
    ON public.passkey_challenges (expires_at);

CREATE INDEX IF NOT EXISTS idx_background_jobs_status_created
    ON public.background_jobs (status, created_at);

CREATE INDEX IF NOT EXISTS idx_background_jobs_finished_at
    ON public.background_jobs (finished_at)
    WHERE finished_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_password_reset_requests_requested_at
    ON public.password_reset_requests (requested_at);
