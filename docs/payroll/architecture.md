# Payroll architecture

## Domain boundaries

Payroll consumes operational records owned elsewhere; it does not re-own them.

| Module | Owns | Payroll touches it for |
| --- | --- | --- |
| Employees | Identity, contact, employment status, department, position, bank details, statutory IDs | Population of a run, payment-file fields, department allocation |
| Attendance / scheduling | Shifts, clock-in/out, worked hours, breaks, exceptions, OT records | Approved OT hours as an earnings input |
| Leave | Requests, approvals, balances, paid/unpaid classification | Approved unpaid leave as a deduction input |
| Claims | Claim records and their approval | Approved claims as reimbursement lines |
| Accounting | GL, journals, payables, reconciliation | Journal **preview** only — payroll writes no ledger rows |

## Request flow

`handlers/payroll.rs` (thin, permission-gated) → `services/payroll_service.rs` /
`payroll_lifecycle_service.rs` / `payroll_engine.rs` → `repositories/` +
`repositories/reads/` → Postgres. Frontend calls go through
`frontend/src/api/payroll.ts` on the shared axios client; no component talks to
the database directly.

## Read models vs. engine

- `repositories/reads/payroll.rs` — run-scoped projections used by the engine's
  bulk prefetch plus item summaries, payment-file rows and department totals.
- `repositories/reads/payroll_overview.rs` — the operational dashboard:
  per-period committed totals, status pipeline, awaiting-action runs, and the
  employee data-quality flags behind the action queue.
- `repositories/reads/reports.rs` — period/grouped reporting reused by the
  overview's department split.

"Committed" everywhere means `processed | pending_approval | approved | paid` —
the same status set `payroll_ytd` sums. `draft`, `processing` and `cancelled`
never contribute to reported figures.

## The overview endpoint

`GET /api/payroll/overview` (`payroll_service::overview`) returns, in one
payload:

- `current_period` — totals of the most recent committed period across groups;
- `variance` — deltas vs the previous committed period (percentages are `null`
  when the base is zero);
- `pipeline` — run count per lifecycle status, all statuses including draft;
- `trend` — committed period totals, oldest-first, up to 12;
- `departments` — labour cost by department for the current period;
- `recent_runs` — five newest runs regardless of status;
- `action_queue` — live checks: runs awaiting submission/approval/payment,
  unprocessed staged entries, missing DOB / payroll group / bank account /
  statutory IDs / resignation date. Codes match `PayrollDiagnostic` codes where
  the same condition also appears in preview.

Every figure is stored on committed runs or computed from live records —
nothing is estimated or projected.

## Frontend

`/payroll` is the overview (`pages/payroll/PayrollOverview.tsx`); the runs list
moved to `/payroll/runs`. Run detail (`/payroll/:id`) carries the lifecycle
actions plus the payment-file download and journal-preview modal. Status labels
and badge styles live in `pages/payroll/statusMeta.ts` because lazy-loaded page
modules may only export components.
