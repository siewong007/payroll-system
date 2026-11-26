-- Security Audit (BugHunter) Remediations:
--   1. Exact-ID payroll run linkage and 'processed' status on overtime_applications
--      (mirroring claims), closing READ COMMITTED double-pay and post-payout
--      cancellation races (H-05).
--   2. Partial unique index on active employee emails per company so duplicate
--      decoy employees cannot shadow self-approval / self-edit guards (H-04).
--   3. Per-account password login failure tracking on users (M-08).

-- 1. Overtime exact-ID payment authority (mirrors 1017_claim_payment_authority.sql).
ALTER TABLE public.overtime_applications
    ADD COLUMN IF NOT EXISTS payroll_run_id uuid;

ALTER TABLE public.overtime_applications
    DROP CONSTRAINT IF EXISTS overtime_applications_status_check;

ALTER TABLE public.overtime_applications
    ADD CONSTRAINT overtime_applications_status_check
    CHECK (((status)::text = ANY (ARRAY[
        ('pending'::character varying)::text,
        ('approved'::character varying)::text,
        ('rejected'::character varying)::text,
        ('cancelled'::character varying)::text,
        ('processed'::character varying)::text
    ])));

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'overtime_applications_run_tenant_fkey'
          AND conrelid = 'public.overtime_applications'::regclass
    ) THEN
        ALTER TABLE public.overtime_applications
            ADD CONSTRAINT overtime_applications_run_tenant_fkey
            FOREIGN KEY (payroll_run_id, company_id)
            REFERENCES public.payroll_runs(id, company_id) NOT VALID;
    END IF;
END $$;

UPDATE public.overtime_applications oa
SET status = 'processed',
    payroll_run_id = pe.payroll_run_id
FROM public.payroll_entries pe
WHERE pe.overtime_application_id = oa.id
  AND pe.company_id = oa.company_id
  AND pe.is_processed = TRUE
  AND pe.payroll_run_id IS NOT NULL
  AND oa.status = 'approved';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.overtime_applications
        WHERE payroll_run_id IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM public.payroll_runs pr
              WHERE pr.id = overtime_applications.payroll_run_id
                AND pr.company_id = overtime_applications.company_id
          )
    ) THEN
        ALTER TABLE public.overtime_applications
            VALIDATE CONSTRAINT overtime_applications_run_tenant_fkey;
    ELSE
        RAISE WARNING 'overtime_applications_run_tenant_fkey left NOT VALID due to orphaned existing row(s).';
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_overtime_applications_payable
    ON public.overtime_applications (company_id, employee_id, ot_date)
    WHERE (status)::text = 'approved' AND payroll_run_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_overtime_applications_payroll_run
    ON public.overtime_applications (payroll_run_id)
    WHERE payroll_run_id IS NOT NULL;

-- 2. Unique normalized active email per company to prevent decoy shadowing.
CREATE UNIQUE INDEX IF NOT EXISTS idx_employees_company_email_unique
    ON public.employees (company_id, lower(btrim(email)))
    WHERE deleted_at IS NULL AND email IS NOT NULL AND btrim(email) <> '';

-- 3. Per-account password login lockout tracking.
ALTER TABLE public.users
    ADD COLUMN IF NOT EXISTS failed_login_attempts integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS login_locked_until timestamp with time zone;
