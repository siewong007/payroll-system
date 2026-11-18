# Payroll workflows

## Run lifecycle

```
draft → processing → processed → pending_approval → approved → paid
                       └──────────────┬──────────────────┘
                                  cancelled
```

| Transition       | From → to                             | Permission           | Notes                                                                                                    |
| ---------------- | ------------------------------------- | -------------------- | -------------------------------------------------------------------------------------------------------- |
| process          | — → processing → processed            | `ManagePayrollDraft` | Async via `background_jobs` (`202` + `GET /api/jobs/{id}`); transactional; snapshots statutory rule sets |
| submit           | processed → pending_approval          | `SubmitPayroll`      | Records `submitted_by`                                                                                   |
| approve          | pending_approval → approved           | `ApprovePayroll`     | Four-eyes: approver ≠ preparer and ≠ submitter                                                           |
| return           | pending_approval → processed          | `ApprovePayroll`     | Reopens for PCB edits                                                                                    |
| lock / mark paid | approved → paid                       | `MarkPayrollPaid`    | Sets `locked_*`                                                                                          |
| **cancel**       | draft/processed → cancelled           | `ManagePayrollDraft` | Preparer withdraws own run                                                                               |
| **cancel**       | pending_approval/approved → cancelled | `ApprovePayroll`     | Approver cancels under review                                                                            |
| **reverse**      | paid → cancelled                      | `ApprovePayroll`     | Finance reverses a paid run with mandatory reason; reverts staged entries/claims                         |
| delete           | draft/processed → (removed)           | `ManagePayrollDraft` | Hard delete; reverts staged inputs                                                                       |

Cancellation (`PUT /payroll/runs/{id}/cancel`, migration 1025) and Paid-Run Reversal (`PUT /payroll/runs/{id}/reverse`):

- Reason is mandatory (≤500 chars), stored on the run and in the audit row.
- `processing` is refused; `paid` runs cannot use `cancel` and must explicitly call `reverse` (`ApprovePayroll`) so an accidental pre-payment cancel never hits a disbursed run.
- A later committed run blocks both cancellation and reversal (same corruption rule as delete — YTD and PCB annualisation were frozen from this run's figures).
- One transaction reverts staged `payroll_entries` and `claims`, then the status-guarded `UPDATE` re-checks state atomically.
- The row stays — it is the audit record of what was computed. `cancelled` is exempt from the one-active-period unique index, so the period can be re-run immediately.

## Immutability

`approved`/`paid` runs reject edits: PCB edits only apply while `processed`,
deletion stops at `pending_approval`, and no endpoint mutates payslip lines on
a locked run. Corrections after payment are future supplemental/adjustment
runs — the schema groundwork (`payroll_entries` staging, version counter)
exists; a dedicated off-cycle run type does not yet.

## Adjustments today

Manual `payroll_entries` stage extra earnings/deductions before a run is
processed and are consumed by it (`is_processed` flag, reverted on
delete/cancel). They have no approval state of their own — a staged entry is
trusted once the run it feeds is approved. Per-entry approval is listed as
future work in `../enhancement-plan.md`.

## Payment output

`GET /payroll/runs/{id}/payment-file` (approved/paid only,
`MarkPayrollPaid`) streams a CSV of employee number, name, bank, account
number/type and net pay. Formula-neutralised via `csv_helpers`. The audit row
records the export and row count — never the account numbers.

## Journal preview

`GET /payroll/runs/{id}/journal-preview` (approved/paid, `MarkPayrollPaid`)
returns a balanced DR/CR preview computed from the run's stored totals:
department-allocated gross wages plus employer statutory expenses against EPF /
SOCSO / EIS / PCB / zakat payables and net salaries payable. Account codes are
fixed defaults; nothing is posted to a ledger.
