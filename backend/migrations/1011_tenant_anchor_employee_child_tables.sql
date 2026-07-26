-- Tenant-anchor the employee-owned child tables.
--
-- Backup restore remapped archive ids with an identity fallback, so a
-- hand-edited archive could write employee_allowances / salary_history /
-- tp3_records rows against another tenant's employee_id -- and
-- recurring_allowance_totals pays whatever it finds for an employee_id. The
-- importer now fails closed; these columns let the database refuse the row too.
--
-- Unlike payroll_items / leave_balances / team_members, these tables hold a
-- single tenant-rooted foreign key, so a same-company trigger has nothing to
-- compare against. The invariant is only expressible once the row states the
-- tenant it belongs to, exactly as claims, leave_requests,
-- overtime_applications and documents already do.
--
-- payroll_item_details is the fourth table with this shape and is deliberately
-- left out: its parent payroll_items carries no company_id of its own, so
-- anchoring it means first adding the column to the money table. That is its
-- own migration and its own review.

ALTER TABLE public.employee_allowances ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE public.salary_history      ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE public.tp3_records         ADD COLUMN IF NOT EXISTS company_id uuid;

-- Backfill from the parent employee. Every child row has a validated scalar FK
-- to employees and employees.company_id is immutable
-- (employees_company_immutable_trigger), so this is total and stable: no row
-- can be orphaned and none can be left NULL.
UPDATE public.employee_allowances child
   SET company_id = e.company_id
  FROM public.employees e
 WHERE e.id = child.employee_id
   AND child.company_id IS DISTINCT FROM e.company_id;

UPDATE public.salary_history child
   SET company_id = e.company_id
  FROM public.employees e
 WHERE e.id = child.employee_id
   AND child.company_id IS DISTINCT FROM e.company_id;

UPDATE public.tp3_records child
   SET company_id = e.company_id
  FROM public.employees e
 WHERE e.id = child.employee_id
   AND child.company_id IS DISTINCT FROM e.company_id;

-- NOT NULL is not cosmetic: a composite FK is MATCH SIMPLE, so a NULL
-- company_id would silently skip the tenant check entirely.
ALTER TABLE public.employee_allowances ALTER COLUMN company_id SET NOT NULL;
ALTER TABLE public.salary_history      ALTER COLUMN company_id SET NOT NULL;
ALTER TABLE public.tp3_records         ALTER COLUMN company_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_employee_allowances_company
    ON public.employee_allowances (company_id);
CREATE INDEX IF NOT EXISTS idx_salary_history_company
    ON public.salary_history (company_id);
CREATE INDEX IF NOT EXISTS idx_tp3_records_company
    ON public.tp3_records (company_id);

-- NOT VALID keeps the idempotent shape used by the baseline. The immediate
-- VALIDATE cannot fail here: the backfill above derived every existing row's
-- company_id from the very parent the constraint checks. (The baseline defers
-- VALIDATE to empty databases only because its pre-existing rows were
-- unaudited; these are not.)
DO $tenant_child_constraints$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'employee_allowances_employee_tenant_fkey' AND conrelid = 'public.employee_allowances'::regclass) THEN
        ALTER TABLE public.employee_allowances ADD CONSTRAINT employee_allowances_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) NOT VALID;
        ALTER TABLE public.employee_allowances VALIDATE CONSTRAINT employee_allowances_employee_tenant_fkey;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'salary_history_employee_tenant_fkey' AND conrelid = 'public.salary_history'::regclass) THEN
        ALTER TABLE public.salary_history ADD CONSTRAINT salary_history_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) NOT VALID;
        ALTER TABLE public.salary_history VALIDATE CONSTRAINT salary_history_employee_tenant_fkey;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'tp3_records_employee_tenant_fkey' AND conrelid = 'public.tp3_records'::regclass) THEN
        ALTER TABLE public.tp3_records ADD CONSTRAINT tp3_records_employee_tenant_fkey
            FOREIGN KEY (employee_id, company_id)
            REFERENCES public.employees(id, company_id) NOT VALID;
        ALTER TABLE public.tp3_records VALIDATE CONSTRAINT tp3_records_employee_tenant_fkey;
    END IF;
END
$tenant_child_constraints$;

-- Tenant ownership never moves after creation, same as employees /
-- payroll_groups / payroll_runs / leave_types / teams.
DROP TRIGGER IF EXISTS employee_allowances_company_immutable_trigger ON public.employee_allowances;
CREATE TRIGGER employee_allowances_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.employee_allowances
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
DROP TRIGGER IF EXISTS salary_history_company_immutable_trigger ON public.salary_history;
CREATE TRIGGER salary_history_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.salary_history
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();
DROP TRIGGER IF EXISTS tp3_records_company_immutable_trigger ON public.tp3_records;
CREATE TRIGGER tp3_records_company_immutable_trigger
    BEFORE UPDATE OF company_id ON public.tp3_records
    FOR EACH ROW EXECUTE FUNCTION public.enforce_immutable_company_id();

-- The company-qualified relationship subsumes the scalar foreign key (both are
-- NO ACTION on delete), matching the cleanup the baseline does at cutover.
ALTER TABLE public.employee_allowances DROP CONSTRAINT IF EXISTS employee_allowances_employee_id_fkey;
ALTER TABLE public.salary_history DROP CONSTRAINT IF EXISTS salary_history_employee_id_fkey;
ALTER TABLE public.tp3_records DROP CONSTRAINT IF EXISTS tp3_records_employee_id_fkey;
