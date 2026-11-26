-- Audit follow-up migration:
--   1. Persist append-only `editor_ids uuid[]` attribution on `claims`,
--      `leave_requests`, and `overtime_applications` so subsequent edits by
--      another user never erase earlier financial editors from four-eyes checks.
--   2. Replace dead `? 'payroll_run_id'` predicates on the two `audit_logs`
--      partial indexes with planner-provable `IS NOT NULL` `->>` predicates.
--   3. Restore `idx_oauth2_accounts_user` if `idx_oauth2_accounts_user_provider`
--      was skipped by migration 1033 due to legacy duplicate rows.
--   4. Drop 7 strict left-prefix duplicate indexes from `1000_schema.sql`.
--   5. Refresh planner statistics for the extended statistics added in 1036.

-- 1. Append-only editor attribution for claims, leave, and overtime.
ALTER TABLE public.claims
    ADD COLUMN IF NOT EXISTS editor_ids uuid[] NOT NULL DEFAULT '{}';

ALTER TABLE public.leave_requests
    ADD COLUMN IF NOT EXISTS editor_ids uuid[] NOT NULL DEFAULT '{}';

ALTER TABLE public.overtime_applications
    ADD COLUMN IF NOT EXISTS editor_ids uuid[] NOT NULL DEFAULT '{}';

UPDATE public.claims
SET editor_ids = ARRAY[updated_by]
WHERE updated_by IS NOT NULL
  AND cardinality(editor_ids) = 0;

UPDATE public.leave_requests
SET editor_ids = ARRAY[updated_by]
WHERE updated_by IS NOT NULL
  AND cardinality(editor_ids) = 0;

UPDATE public.overtime_applications
SET editor_ids = ARRAY[updated_by]
WHERE updated_by IS NOT NULL
  AND cardinality(editor_ids) = 0;

-- 2. Planner-usable JSONB partial indexes matching `reads/audit.rs`.
DROP INDEX IF EXISTS public.idx_audit_logs_payroll_item_run_new;
DROP INDEX IF EXISTS public.idx_audit_logs_payroll_item_run_old;

CREATE INDEX IF NOT EXISTS idx_audit_logs_payroll_item_run_new
    ON public.audit_logs (company_id, (new_values->>'payroll_run_id'))
    WHERE entity_type = 'payroll_item'
      AND (new_values->>'payroll_run_id') IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_audit_logs_payroll_item_run_old
    ON public.audit_logs (company_id, (old_values->>'payroll_run_id'))
    WHERE entity_type = 'payroll_item'
      AND (old_values->>'payroll_run_id') IS NOT NULL;

-- 3. Conditional restore of `oauth2_accounts (user_id)` index if 1033 skipped
--    `idx_oauth2_accounts_user_provider`.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_index i ON i.indexrelid = c.oid
        WHERE n.nspname = 'public'
          AND c.relname = 'idx_oauth2_accounts_user_provider'
          AND i.indisvalid
    ) THEN
        CREATE INDEX IF NOT EXISTS idx_oauth2_accounts_user
            ON public.oauth2_accounts USING btree (user_id);
    END IF;
END
$$;

-- 4. Drop strict left-prefix duplicate indexes superseded by 1036.
DROP INDEX IF EXISTS public.idx_payroll_items_employee;
DROP INDEX IF EXISTS public.idx_salary_history_employee;
DROP INDEX IF EXISTS public.idx_claims_employee;
DROP INDEX IF EXISTS public.idx_claims_status;
DROP INDEX IF EXISTS public.idx_leave_requests_employee;
DROP INDEX IF EXISTS public.idx_leave_requests_status;
DROP INDEX IF EXISTS public.idx_overtime_applications_company;

-- 5. Populate extended multivariate statistics created in 1036.
ANALYZE public.leave_requests;
ANALYZE public.claims;
ANALYZE public.overtime_applications;
ANALYZE public.payroll_entries;
ANALYZE public.notifications;
