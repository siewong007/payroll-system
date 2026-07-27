-- Indexes for the reworked payroll run population.
--
-- The run no longer selects on `is_active`. A leaver is deactivated the moment
-- HR records the resignation, and gating the population on that flag dropped
-- their final, prorated payslip with no error and no preview diagnostic —
-- statutory contributions went unremitted and the EA form came out a month
-- short. Selection is now the employment window:
--
--     deleted_at IS NULL
--     AND (is_active = TRUE OR date_resigned IS NOT NULL)
--     AND date_joined <= period_end
--     AND (date_resigned IS NULL OR date_resigned >= period_start)
--
-- `idx_employees_payroll_active` (1000_schema.sql) is partial on
-- `is_active = true`, so it can no longer cover that scan. The index below
-- carries the same leading columns without the `is_active` predicate, and
-- includes `date_resigned` so the leaver window is answered from the index
-- rather than a heap probe per row.
--
-- `idx_employees_payroll_active` is deliberately KEPT, not replaced:
-- employees::count_active and employees::count_distinct_departments still
-- filter on `is_active = TRUE` and are served by it.
--
-- ─────────────────────────────────────────────────────────────────────────────
-- PRE-DEPLOY AUDIT — run per tenant and clear the findings BEFORE this ships.
--
-- Two changes in this release compound in a way neither one models alone. This
-- one re-admits deactivated leavers into the payroll population; the statutory
-- rework blocks the entire run on any employee with no date of birth. Employees
-- deactivated years ago are precisely the cohort whose date of birth was never
-- captured, so together they can freeze a tenant's next payroll run on day one:
--
--     SELECT company_id, count(*)
--     FROM employees
--     WHERE deleted_at IS NULL
--       AND payroll_group_id IS NOT NULL
--       AND (date_resigned IS NOT NULL OR is_active = FALSE)
--       AND date_of_birth IS NULL
--     GROUP BY 1;
--
-- A non-zero count means that tenant's next run is blocked until each row is
-- resolved — set the date of birth, or clear `payroll_group_id` to leave the
-- record out of payroll deliberately.
--
-- The narrower half of the same problem, which this change introduces on its
-- own (an inactive row still holding a payroll group, with no resignation date
-- to say whether a final payslip is owed):
--
--     SELECT company_id, count(*)
--     FROM employees
--     WHERE deleted_at IS NULL
--       AND payroll_group_id IS NOT NULL
--       AND is_active IS NOT TRUE
--       AND date_resigned IS NULL
--     GROUP BY 1;
--
-- Do NOT weaken the block these rows cause into a warning; a warning reproduces
-- the original silent-drop defect.
-- ─────────────────────────────────────────────────────────────────────────────

CREATE INDEX IF NOT EXISTS idx_employees_payroll_population
    ON public.employees (company_id, payroll_group_id, date_joined, date_resigned)
    WHERE deleted_at IS NULL;

-- The run also reports employees excluded SOLELY by not being active, with no
-- resignation date, so processing blocks on them instead of silently omitting
-- them. That lookup is narrow and worth its own partial index.
--
-- `is_active IS NOT TRUE` rather than `= FALSE`: the column is nullable, and a
-- NULL fails the population's `is_active = TRUE` exactly as FALSE does. The
-- predicate matches the query's verbatim so the index is usable.
CREATE INDEX IF NOT EXISTS idx_employees_inactive_without_resignation
    ON public.employees (company_id, payroll_group_id, date_joined)
    WHERE deleted_at IS NULL AND is_active IS NOT TRUE AND date_resigned IS NULL;
