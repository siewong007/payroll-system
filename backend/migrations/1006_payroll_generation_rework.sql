-- Payroll generation rework: run-level calculation provenance, and support for
-- the payslip breakdown that `payroll_item_details` was created for but never
-- received a write path.

-- Which verified statutory rule sets, and which company overtime configuration,
-- produced a run's figures. Both inputs are mutable and effective-dated: a later
-- rule import or a change to `overtime_multiplier_*` silently changes what the
-- engine would compute today, leaving no way to explain a historical payslip.
-- Recorded at commit time so the run stays self-describing.
ALTER TABLE payroll_runs
    ADD COLUMN IF NOT EXISTS calculation_snapshot jsonb;

-- Every read of the breakdown is by payslip item, and deleting a run's details
-- joins back through payroll_items. Unindexed until now because nothing wrote
-- the table.
CREATE INDEX IF NOT EXISTS idx_payroll_item_details_item
    ON payroll_item_details (payroll_item_id);
