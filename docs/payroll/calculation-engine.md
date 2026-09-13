# Calculation engine

`services/payroll_engine.rs` is the only payroll calculator. There is no second
engine in handlers, jobs, or the frontend.

## Money and precision

- All amounts are integer **sen** (`bigint` / `i64`) end-to-end. No `f64`
  anywhere in a money path.
- Quantities (hours, days, divisors) are `rust_decimal::Decimal`.
- Rounding funnels through `round_sen`; exports use
  `csv_helpers::sen_to_plain_rm`.
- Serialised `Decimal` fields travel as strings (`serde-with-str`) — the TS
  types mirror that.

## Structure

1. `RunPeriod::resolve` — period bounds in the **company timezone**.
2. `gather_run_inputs` — bulk prefetch into `BulkPayrollData` (employees in the
   group, recurring allowance windows as interval overlaps, staged entries,
   approved OT/claims/unpaid leave, YTD, statutory tables).
3. `compute_payslip` — **pure** per-employee calculation: prorated basic
   (EA1955 s.18B calendar days), allowances, OT, claims, unpaid-leave
   deduction, EPF/SOCSO/EIS/PCB, YTD accumulation.
4. `persist_payslip` — writes `payroll_items` + `payroll_item_details`
   (the explainable line breakdown) inside the run transaction.
5. `preview_payroll` runs the same computation without persisting and returns
   diagnostics instead of aborting on the first failure.

## Explainability

Each payslip line carries type/code/description/amount and is stored in
`payroll_item_details`; `GET /payroll/runs/{run}/items/{emp}/breakdown` renders
it. The run keeps a `calculation_snapshot` of the exact statutory rule sets and
OT settings used, so a historical payslip can be re-read against the rules that
produced it even after newer rule versions exist.

## Input traceability

- OT lines derive from approved `overtime_applications` rows.
- Claims lines derive from approved `claims` rows (consumed per-run, reverted
  on cancel/delete).
- Unpaid leave derives from approved `leave_requests` classified unpaid.
- Staged `payroll_entries` carry the entry id into the run.
