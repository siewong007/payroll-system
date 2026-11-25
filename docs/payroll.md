# Payroll module

This document covers the implemented payroll subsystem — domain boundaries, calculation engine, run lifecycle, statutory rules, permissions, schema invariants, and test coverage. Read alongside [features.md](features.md), [database.md](database.md), and [legal.md](legal.md).

## Domain boundaries and read models

Payroll consumes operational records owned elsewhere without re-owning them:

| Module                  | Owns                                                            | Payroll touches it for                                           |
| ----------------------- | --------------------------------------------------------------- | ---------------------------------------------------------------- |
| Employees               | Identity, employment dates, department, bank, statutory numbers | Run population, payment-file fields, department cost allocation  |
| Attendance / scheduling | Shifts, clock-in/out, breaks, exceptions, OT records            | Approved / derived OT hours as an earnings input                 |
| Leave                   | Requests, approvals, balances, paid/unpaid classification       | Approved unpaid leave as a deduction and statutory-base offset   |
| Claims                  | Claim records and approvals                                     | Approved claims as reimbursement lines (outside gross/statutory) |
| Accounting              | GL, journals, payables, reconciliation                          | Balanced journal **preview** only (no ledger posting)            |

Request flow: `handlers/payroll.rs` (thin, permission-gated) → `services/payroll_service.rs` / `payroll_lifecycle_service.rs` / `payroll_engine.rs` → `repositories/` + `repositories/reads/` → PostgreSQL.

Read-model modules:

- `repositories/reads/payroll.rs` — run-scoped projections for bulk engine prefetch, item summaries, payment-file rows, and department totals.
- `repositories/reads/payroll_overview.rs` — powers `GET /api/payroll/overview` (`/payroll` dashboard): committed current-period totals (`processed | pending_approval | approved | paid`), prior-period variance, status pipeline counts, 12-period trend, department split, 5 recent runs, and live `action_queue` checks (pending lifecycle actions, unprocessed staged entries, missing DOB/group/bank/statutory IDs/resignation dates).
- `repositories/reads/reports.rs` — period and department aggregations reused across reports and overview.

## Calculation engine (`services/payroll_engine.rs`)

`services/payroll_engine.rs` is the sole payroll calculator.

### Money and precision

- All money values are integer **sen** (`bigint` / `i64`) end to end — never `f64`.
- Quantities (hours, days, divisors) use `rust_decimal::Decimal`, serialized as strings (`serde-with-str`).
- Rounding funnels through `round_sen`; machine-readable statutory/payment CSV exports use `csv_helpers::sen_to_plain_rm` with formula neutralization.

### Execution structure

1. `RunPeriod::resolve` — resolves period bounds in the **company's validated IANA timezone**.
2. `gather_run_inputs` — bulk prefetches eligible employees in the payroll group, overlapping recurring allowance windows, staged `payroll_entries`, approved overtime/claims/unpaid leave, prior YTD figures (including `taxable_gross` per `1033_audit_bugfixes.sql`), and a single `StatutoryTables` snapshot (`services/statutory_tables.rs`).
3. `compute_payslip` — **pure** per-employee calculation over `BulkPayrollData` and `StatutoryTables`:
   - Prorated basic salary (`EA 1955 s.18B` calendar days) and prorated overlapping recurring allowances.
   - Overtime (`EA 1955 s.60I` 26-day ORP, `effective_hours_per_day`, and `1.5×`/`2.0×`/`3.0×` day-type multipliers).
   - Claims reimbursements (paid on top of net; excluded from gross and statutory bases).
   - Unpaid-leave deductions (reducing EPF/SOCSO/EIS and PCB taxable bases while keeping contracted gross visible).
   - **EPF** (`epf_service`): KWSP Third Schedule Parts `A`–`F` & age-75 cessation (`s.43(1)`), excluding overtime (`s.2`).
   - **SOCSO & EIS** (`socso_service`, `eis_service`): includes overtime, clamped at the **RM6,000/month** ceiling (`600_000` sen).
   - **PCB** (`pcb_calculator`): tracks `taxable_gross` (excluding unpaid leave and non-taxable allowances), Schedule 2 bonus/commission differential, Zakat offset, and YTD + TP3 accumulation.
4. `process_payroll` — computes **all** employees before persisting any row (so errors surface together with zero partial writes), then calls `persist_payslip` to write `payroll_items` and `payroll_item_details`.
5. `preview_payroll` (`POST /api/payroll/preview`) — executes the exact same computation without persisting, returning per-employee projections plus `blocking` and `warnings` diagnostics (`missing_date_of_birth`, `epf_category_override`, `below_minimum_wage` under _Minimum Wages Order 2024_ RM1,700/month, `missing_earlier_period`, `missing_bank_account`).

### Explainability and traceability

Every payslip stores itemized `payroll_item_details` lines (`GET /api/payroll/runs/{run}/items/{emp}/breakdown`), and every run records a `calculation_snapshot` capturing the verified rule-set IDs and overtime configuration used at computation time. Approved `overtime_applications`, `claims`, `leave_requests`, and staged `payroll_entries` (`overtime_application_id`, `leave_request_id` per `1033`) trace directly to the run and revert atomically if a run is deleted, cancelled, or reversed.

## Run lifecycle and workflows

```text
draft → processing → processed → pending_approval → approved → paid
                       └──────────────┬──────────────────┘
                                  cancelled
```

| Transition       | From → To                                   | Permission           | Notes                                                                                                   |
| ---------------- | ------------------------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------- |
| Process          | — → `processing` → `processed`              | `ManagePayrollDraft` | Enqueued via `background_jobs` (`202 Accepted` + `GET /api/jobs/{id}`); one active run per group/period |
| Submit           | `processed` → `pending_approval`            | `SubmitPayroll`      | Records `submitted_by` (`1030`)                                                                         |
| Approve          | `pending_approval` → `approved`             | `ApprovePayroll`     | Four-eyes check: approver ≠ preparer and ≠ submitter                                                    |
| Return           | `pending_approval` → `processed`            | `ApprovePayroll`     | Reopens run for controlled PCB adjustments                                                              |
| Lock / Mark paid | `approved` → `paid`                         | `MarkPayrollPaid`    | Sets `locked_*` timestamps and enables portal payslip visibility                                        |
| Cancel           | `draft`/`processed` → `cancelled`           | `ManagePayrollDraft` | Preparer withdrawal with mandatory reason (≤500 chars); blocked if a later committed run exists         |
| Cancel (review)  | `pending_approval`/`approved` → `cancelled` | `ApprovePayroll`     | Approver cancellation with mandatory reason; reverts staged entries/claims atomically                   |
| Reverse          | `paid` → `cancelled`                        | `ApprovePayroll`     | Finance reversal (`PUT /api/payroll/runs/{id}/reverse`) with mandatory reason; reverts staged inputs    |
| Delete           | `draft`/`processed` → removed               | `ManagePayrollDraft` | Hard delete of pre-submission run; reverts staged inputs                                                |

- **Immutability**: `approved` and `paid` runs reject PCB or line mutations; cancelled runs remain for auditability and are exempt from `payroll_runs_one_active_period`, allowing immediate re-runs.
- **Outputs**: Bank salary-payment CSV (`GET /api/payroll/runs/{id}/payment-file`, `MarkPayrollPaid`, audited without account numbers), balanced GL journal preview (`GET /api/payroll/runs/{id}/journal-preview`), individual/batch payslip PDFs (`payslip_pdf_service`), statutory export files (`statutory_export_service`), and annual Form EA (`ea_form_service`).

## Statutory rules and fail-closed gate

EPF, SOCSO, EIS, and PCB/MTD are table-driven, effective-dated, and versioned in `statutory_rule_sets` (`source_version`, `source_sha256`). See [legal.md#part-iii-malaysian-statutory-compliance-guide-2026](legal.md#part-iii-malaysian-statutory-compliance-guide-2026) for full statutory rates and thresholds.

- **Fail-closed posture**: Seeded rows in `1001_data.sql` are `status = 'prototype'` academic fixtures. Production payroll refuses to run without verified, non-overlapping rule sets across `epf`, `socso`, `eis`, and `pcb`, and automatic PCB remains disabled (`platform_settings.pcb_calculator_status <> 'enabled'`) until the calculator passes LHDN computerised-MTD conformance.
- **Age & category rules**: `epf_service::derive_category_for` derives Parts `A`–`F` (including Oct 2025 2%+2% foreign-worker Part F) and `EXEMPT` at age 75+ (`s.43(1)`), honoring explicit `employees.epf_category` overrides (`NULL` default per `1033`). `socso_service` and `eis_service` enforce the RM6,000 ceiling and fail closed on missing DOB, ages 55–59 (SOCSO), ages 57–59 (EIS), and foreign-worker PERKESO schemes lacking entry-age history.

## Permissions and separation of duties

The authoritative permission matrix lives in `backend/src/core/permission.rs`; the frontend reads `user.permissions` only for UI gating.

| Permission               | Held by                                                 | Gates                                                                 |
| ------------------------ | ------------------------------------------------------- | --------------------------------------------------------------------- |
| `view_payroll`           | `payroll_admin`, `finance`, `super_admin`               | Overview, runs list/detail, items, breakdown, run audit logs          |
| `manage_payroll_draft`   | `payroll_admin`, `super_admin`                          | Preview/run, staged entries, delete, PCB edit, cancel draft/processed |
| `submit_payroll`         | `payroll_admin`, `super_admin`                          | Submit run for approval                                               |
| `approve_payroll`        | `finance`, `super_admin`                                | Approve, return, cancel pending/approved, reverse paid                |
| `mark_payroll_paid`      | `finance`, `super_admin`                                | Mark paid/lock, payment-file CSV export, journal preview              |
| `view_salary_history`    | `payroll_admin`, `hr_manager`, `finance`, `super_admin` | Employee salary history endpoint                                      |
| `view_statutory_exports` | `payroll_admin`, `finance`, `super_admin`               | EPF, SOCSO, EIS, PCB exports and Form EA                              |

Invariants enforced by tests: `admin`, `manager`, and `exec` hold **no** payroll permissions; no standard role both submits and approves (`submit_payroll` + `approve_payroll` cannot coexist in a `user_group` or effective user grants); portal payslips are scoped to the caller's linked `employee_id` and visible only after approval/payment.

## Testing

Backend test suites under `backend/src/tests/` (run with `cargo test` with `DATABASE_URL` and `JWT_SECRET`):

- `payroll_tests` — engine correctness, proration, OT, claims, `taxable_gross`, population, and DB-free `proptest` / golden vectors
- `payroll_lifecycle_tests` — submit, approve, return, lock, cancel, paid-run reversal, and re-run after cancel
- `statutory_tests` — EPF Parts A–F & age 75 cessation, SOCSO/EIS RM6,000 ceiling & fail-closed guards, PCB brackets/bonus/zakat
- `export_and_report_tests` — statutory CSV/CP39 formatting, formula neutralization, and EA form golden checks
- `approval_flow_tests`, `route_auth_tests`, `route_inventory_tests`, and `schema_invariant_tests` — four-eyes separation, permission gates, and DB constraints

Frontend Vitest suites cover `PayrollPreview.test.tsx`, `Csv.test.ts`, `Roles.test.ts`, and `ApiModulesExtended.test.ts`.
