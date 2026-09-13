# Payroll permissions

The backend matrix (`core/permission.rs`) is the single source; the frontend
reads `user.permissions` from the session and only decides what to render —
every request is re-checked server-side.

| Permission | Held by | Gates |
| --- | --- | --- |
| `view_payroll` | payroll_admin, finance, super_admin | Runs list/detail, overview, items, breakdown, audit |
| `manage_payroll_draft` | payroll_admin, super_admin | Process/preview, staged entries, delete, PCB edit, cancel draft/processed |
| `submit_payroll` | payroll_admin, super_admin | submit for approval |
| `approve_payroll` | finance, super_admin | approve, return, cancel pending/approved |
| `mark_payroll_paid` | finance, super_admin | lock as paid, payment-file export, journal preview |
| `view_salary_history` | payroll_admin, hr_manager, finance, super_admin | salary history endpoint |
| `view_statutory_exports` | payroll_admin, finance, super_admin | EPF/SOCSO/EIS/PCB files, EA form |

Invariants enforced in `permission.rs` tests: `exec` and `admin` hold no
payroll permissions; no normal role both prepares and approves (separation of
duties); `super_admin` is the superset.

Scoping: every handler resolves `company_id` from the JWT, never from the
request body. Portal payslips are employee-scoped to the caller's linked
employee record and only visible once the run is approved/paid.
