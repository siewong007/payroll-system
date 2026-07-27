-- `claims` becomes the single authority for claim payment.
--
-- Two payment paths existed and disagreed. Approving a claim staged a
-- `payroll_entries` row keyed to the APPROVAL month, but every entry read
-- excludes `item_type = 'claim_reimbursement'`, so that row never reached gross
-- or net. What actually paid was a period-bounded sweep of `claims` keyed on
-- `expense_date BETWEEN period_start AND period_end` — so a claim approved after
-- its own expense month had closed could never be paid by any run, while the
-- staged row was still flipped to `is_processed = TRUE` by the next run and made
-- the claim uncancellable. The employee had already been emailed that it would
-- be in their next payroll.
--
-- The fix is carry-forward selection (`expense_date <= period_end` AND not yet
-- paid) plus an explicit link to the run that paid it, and retirement of the
-- parallel staging.

ALTER TABLE public.claims
    ADD COLUMN IF NOT EXISTS payroll_run_id uuid;

-- Composite tenant FK, matching payroll_entries_run_tenant_fkey. No ON DELETE
-- action on purpose: `payroll_service::delete_run` must revert a run's claims
-- before deleting it, and a forgotten revert should fail loudly rather than
-- leave a 'processed' claim pointing at nothing.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'claims_run_tenant_fkey'
          AND conrelid = 'public.claims'::regclass
    ) THEN
        ALTER TABLE public.claims
            ADD CONSTRAINT claims_run_tenant_fkey
            FOREIGN KEY (payroll_run_id, company_id)
            REFERENCES public.payroll_runs(id, company_id) NOT VALID;
    END IF;
END $$;

-- Backfill already-paid claims so `revert_for_run` works for runs that exist
-- today. One active run per (company, payroll_group, year, month) and one group
-- per employee make the match unique; DISTINCT ON keeps it deterministic anyway.
UPDATE public.claims c
SET payroll_run_id = m.run_id
FROM (
    SELECT DISTINCT ON (c2.id) c2.id AS claim_id, pr.id AS run_id
    FROM public.claims c2
    JOIN public.payroll_runs pr
      ON pr.company_id = c2.company_id
     AND c2.expense_date BETWEEN pr.period_start AND pr.period_end
    JOIN public.payroll_items pi
      ON pi.payroll_run_id = pr.id
     AND pi.employee_id = c2.employee_id
    WHERE c2.status = 'processed'
      AND c2.payroll_run_id IS NULL
    ORDER BY c2.id, pr.period_year, pr.period_month, pr.id
) m
WHERE c.id = m.claim_id;

-- Validate only when nothing already violates it. Migrations run at container
-- start and a failed one freezes the deploy; the backfill above cannot produce
-- a violating row, but a hand-repaired database could.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.claims
        WHERE payroll_run_id IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM public.payroll_runs pr
              WHERE pr.id = claims.payroll_run_id
                AND pr.company_id = claims.company_id
          )
    ) THEN
        ALTER TABLE public.claims VALIDATE CONSTRAINT claims_run_tenant_fkey;
    ELSE
        RAISE WARNING 'claims_run_tenant_fkey left NOT VALID: % existing row(s) point at a payroll_run outside their company. New writes are still rejected; correct those rows and run VALIDATE CONSTRAINT manually.',
            (SELECT count(*) FROM public.claims
              WHERE payroll_run_id IS NOT NULL
                AND NOT EXISTS (
                    SELECT 1 FROM public.payroll_runs pr
                    WHERE pr.id = claims.payroll_run_id
                      AND pr.company_id = claims.company_id
                ));
    END IF;
END $$;

-- The run's new selection predicate: approved, not yet paid, incurred on or
-- before period_end. idx_claims_approved_payroll does not know about
-- payroll_run_id, so it can no longer serve this on its own.
CREATE INDEX IF NOT EXISTS idx_claims_payable
    ON public.claims (company_id, employee_id, expense_date)
    WHERE (status)::text = 'approved' AND payroll_run_id IS NULL;

-- `revert_for_run` looks a run's claims up by run id.
CREATE INDEX IF NOT EXISTS idx_claims_payroll_run
    ON public.claims (payroll_run_id)
    WHERE payroll_run_id IS NOT NULL;

-- Retire the parallel staging. These rows never contributed to any payslip
-- figure — every payroll_entries read excludes the item type — so removing them
-- changes no historical amount. They did show as unexplained 'earning' rows on
-- the admin staged-entries screen, and they are what wrongly blocked
-- cancellation. Provenance stays in audit_logs and in `claims` itself.
DELETE FROM public.payroll_entries WHERE item_type = 'claim_reimbursement';
