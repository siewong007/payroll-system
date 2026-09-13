# Payroll database notes

See `../database.md` for the full schema. Payroll-specific points:

## Core tables

- `payroll_runs` — header: period, group, totals, lifecycle timestamps, and
  (1025) `cancelled_by`/`cancelled_at`/`cancel_reason` with a consistency
  CHECK. One active run per `(company, group, year, month)` via the partial
  unique index `payroll_runs_one_active_period` (`status <> 'cancelled'`).
- `payroll_items` — per-employee committed figures incl. frozen `ytd_*`.
- `payroll_item_details` — explainable lines behind each figure.
- `payroll_entries` — staged manual earnings/deductions, consumed per-run and
  reverted on cancel/delete.
- `payroll_groups`, `payroll_allowances` (employee_allowances), `salary_history`.
- Statutory: `epf_rates`, `socso_rates`, `eis_rates`, `pcb_brackets`,
  `pcb_reliefs`, `tp3_records`, `statutory_rule_sets`.

## salary_history (1025)

`change_type` is a checked vocabulary (`new_hire`, `increment`, `promotion`,
`demotion`, `contract_change`, `correction`, `backdated_adjustment`,
`termination`, `adjustment`); `notes` ≤500; `approved_by`/`approved_at` are
paired-NULL anchors for a future approval workflow. Rows with `old_salary = 0`
were backfilled to `new_hire`. Backup export/import round-trips the new
columns; pre-1025 backups restore with the `adjustment` default.

The employee update path accepts `salary_change_type`/`salary_change_notes`,
validated service-side before the shared transaction writes both rows.

## Integrity

- Money columns are `bigint` sen; quantities `numeric`.
- `payroll_items` bounds checks exist (1021); run cancel columns are
  consistency-checked.
- Soft-delete (`employees.deleted_at`) is excluded from all action-queue flags.
