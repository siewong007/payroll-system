# Payroll testing

Backend suites (all under `src/tests/`, run with `cargo test` — need
`DATABASE_URL` + `JWT_SECRET`):

| Suite | Covers |
| --- | --- |
| `payroll_tests` | engine correctness, population, period resolution |
| `payroll_lifecycle_tests` | submit/approve/lock, return-for-changes, **cancel** (preparer/approver split, paid-terminal, mandatory reason, audit, re-run after cancel) |
| `statutory_tests` | EPF categories/ceilings/fail-closed, SOCSO, EIS, PCB brackets/additional remuneration/zakat |
| `approval_flow_tests` | four-eyes separation |
| `route_auth_tests` | permission gates on payroll routes |
| `schema_invariant_tests` | constraints, FK scoping |

Frontend: Vitest suites cover api modules, roles, CSV helpers and page flows.
`statusMeta.ts` constants are intentionally dumb and untested.

Gaps worth filling next (tracked in `../enhancement-plan.md`): export golden
files for the statutory/payment CSVs, concurrency tests on the unique-index
path, and payslip PDF snapshot tests.
