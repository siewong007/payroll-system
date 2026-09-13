# Payroll Operations Surface — Implementation Plan

> Executed inline (no subagents) on branch `feat/payroll-ops`.

**Goal:** close the top payroll operations gaps from the audit — an operational
overview/action queue, run cancellation, payment-file export, journal preview,
and salary-change metadata — without disturbing the existing engine.

**Stack:** Axum + sqlx macros (offline `.sqlx` cache, regenerate after query
changes) / React 19 + Vite + TanStack Query + Tailwind v4.

## Global constraints

- Money is `bigint` sen end-to-end; never `f64`. Display via `formatMYR`,
  exports via `csv_helpers::sen_to_plain_rm` + `neutralize_formula`.
- Handlers stay thin: permission gate → service → repository. All new SQL in
  `repositories/` or `repositories/reads/`, generic over `Executor`.
- Every new handler carries an explicit `require_permission` gate.
- Migrations are additive numbered files; next free number is **1025**
  (a `1024` exists on unmerged branch `83d9605` and in the dev DB).
- `paid` runs stay terminal: cancellation stops at `approved`.
- YTD/portal/report reads already exclude `cancelled` — verified.

## Tasks

### 1. Migration `1025_payroll_ops.sql`
- `payroll_runs` += `cancelled_by uuid`, `cancelled_at timestamptz`,
  `cancel_reason varchar(500)`.
- `salary_history` += `change_type varchar(30) NOT NULL DEFAULT 'adjustment'`
  (CHECK against type list), `notes varchar(500)`, `approved_by uuid`,
  `approved_at timestamptz`. Backfill `new_hire` where `old_salary = 0`.

### 2. Run cancellation
- `payroll_runs::set_cancelled` — status-guarded UPDATE
  (`draft|processed|pending_approval|approved → cancelled`).
- `payroll_lifecycle_service::cancel(pool, company, run, auth, reason, meta)`:
  mandatory trimmed reason ≤500; per-status permission
  (`ManagePayrollDraft` for draft/processed, `ApprovePayroll` for
  pending_approval/approved); refuse `processing`/`paid`; refuse when a later
  committed run exists (same rule as `delete_run`); one transaction: revert
  staged `payroll_entries` + `claims`, set cancelled, audit row.
- `PUT /payroll/runs/{id}/cancel`, handler dual-checks nothing — the service
  enforces the per-status permission via `AuthUser`.

### 3. `GET /api/payroll/overview` (ViewPayroll)
`repositories/reads/payroll_overview.rs` + `services/payroll_overview_service.rs`:
- `latest_period`: most recent `(year, month)` with a committed run
  (processed/pending_approval/approved/paid).
- `kpis`: summed run totals for that period (employees, gross, deductions,
  net, employer cost, EPF/SOCSO/EIS/PCB/zakat, overtime).
- `variance`: per-group latest committed run vs its predecessor, summed.
- `pipeline`: run counts grouped by status.
- `trend`: last 12 committed periods (sum across groups per period).
- `departments`: labour cost by department for the latest period.
- `action_queue`: employees missing bank/DOB/payroll group/statutory ids,
  inactive-without-resignation blockers, pending leave/claim/overtime counts,
  unprocessed staged entries, runs awaiting submit/approve/pay.

### 4. `GET /api/payroll/runs/{id}/payment-file` (MarkPayrollPaid)
CSV for `approved`/`paid` runs only: employee no, name, bank name, account
number, bank type, net sen→RM, reference `PAY-{YYYYMM}-{empno}`. Audited with
count + total only (never account numbers).

### 5. `GET /api/payroll/runs/{id}/journal-preview` (ViewPayroll)
Computed from committed figures, zero writes: DR wage expense per department
(gross − unpaid leave), DR employer EPF/SOCSO/EIS expense, CR statutory
payables (ee+er), CR PCB/zakat/PTPTN/Tabung Haji/other deductions payable,
CR net salary payable, CR claims payable; `balanced` flag. Documented as a
fixed default mapping until a configurable COA exists.

### 6. Salary history metadata
- `salary_history::insert` gains `change_type`/`notes`.
- `UpdateEmployeeRequest` += `salary_change_type`, `salary_change_reason`
  (validated against the CHECK list); `employee_service::update` passes them.
- `SalaryHistory` model + list query + `EmployeeDetail` display.

### 7. Frontend
- `api/payroll.ts` + `types`: overview, cancel, payment file, journal.
- `pages/payroll/PayrollOverview.tsx` (KPIs, pipeline, action queue, trend,
  departments — real data only).
- `App.tsx`: `/payroll` → Overview, `/payroll/runs` → list (rest unchanged).
- `PayrollDetail`: cancel dialog (reason), payment-file + journal actions.

### 8. Verify + docs
`cargo sqlx prepare` (against `payroll_dev`), fmt, clippy `-D warnings`,
`cargo test` (DB up), frontend lint/typecheck/test/build, `docs/payroll/`,
`features.md`, `./scripts/codegraph update .`.

---

## Execution record (2026-09-13)

All tasks implemented and verified:

- `1025_payroll_ops.sql` applied to `payroll_dev`; cancel columns + CHECK,
  salary_history metadata + backfill.
- Cancel: `payroll_runs::set_cancelled`, `payroll_lifecycle_service::cancel`,
  `PUT /payroll/runs/{id}/cancel` — 3 lifecycle tests added and passing.
- Overview: `reads/payroll_overview.rs`, `payroll_service::overview`,
  `GET /payroll/overview`, `/payroll` page (runs list → `/payroll/runs`).
- Payment file: `reads/payroll.rs::payment_file_rows`,
  `payroll_service::export_payment_file`, `GET …/payment-file`, UI button.
- Journal preview: `run_department_totals`, `journal_preview`,
  `GET …/journal-preview`, modal on run detail.
- Salary history: `change_type`/`notes` through `UpdateEmployeeRequest` →
  `employee_service` → `salary_history::insert`; backup export/import
  round-trips the new columns (`#[serde(default)]` keeps pre-1025 backups
  importable).
- Verification: `cargo fmt --check`, `clippy -D warnings`, 592 tests pass,
  `.sqlx` regenerated; `bun lint`/`typecheck`/`test` (377)/`build` all clean.
