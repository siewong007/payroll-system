-- compute_payslip refuses to CREATE a negative net; update_item_pcb could edit
-- one into existence. The service bound is the fix; this is the backstop.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint
                   WHERE conname = 'payroll_items_net_salary_non_negative'
                     AND conrelid = 'public.payroll_items'::regclass) THEN
        ALTER TABLE public.payroll_items
            ADD CONSTRAINT payroll_items_net_salary_non_negative
            CHECK (net_salary >= 0) NOT VALID;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_constraint
                   WHERE conname = 'payroll_items_pcb_non_negative'
                     AND conrelid = 'public.payroll_items'::regclass) THEN
        ALTER TABLE public.payroll_items
            ADD CONSTRAINT payroll_items_pcb_non_negative
            CHECK (pcb_amount >= 0) NOT VALID;
    END IF;
END $$;

-- Validate ONLY if the existing data already satisfies it. Migrations run at
-- container start, so a bare VALIDATE against one already-corrupted row would
-- crash-loop the backend and freeze every subsequent deploy. NOT VALID still
-- enforces on every future INSERT/UPDATE.
DO $$
DECLARE bad bigint;
BEGIN
    SELECT count(*) INTO bad FROM public.payroll_items WHERE net_salary < 0;
    IF bad = 0 THEN
        ALTER TABLE public.payroll_items
            VALIDATE CONSTRAINT payroll_items_net_salary_non_negative;
    ELSE
        RAISE WARNING 'payroll_items_net_salary_non_negative left NOT VALID: % negative row(s)', bad;
    END IF;

    SELECT count(*) INTO bad FROM public.payroll_items WHERE pcb_amount < 0;
    IF bad = 0 THEN
        ALTER TABLE public.payroll_items
            VALIDATE CONSTRAINT payroll_items_pcb_non_negative;
    ELSE
        RAISE WARNING 'payroll_items_pcb_non_negative left NOT VALID: % negative row(s)', bad;
    END IF;
END $$;
