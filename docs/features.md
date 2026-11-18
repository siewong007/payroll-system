# Implemented feature catalogue

This catalogue distinguishes code that is usable end to end from code that is
present but not fully wired. It is the feature source of truth for the current
repository.

## Status definitions

| Status | Meaning |
| --- | --- |
| End to end | Backend route, persistence/business logic, and a reachable frontend flow exist |
| API only | Backend capability exists, but no reachable product page currently exposes it |
| Conditional | Implemented but requires external configuration or infrastructure |
| Informational | UI explains or displays policy but does not persist that policy |
| Unwired | Code exists on one side of the stack but is not a reachable, complete workflow |
| Known limitation | A user-visible flow exists with a material security or completeness caveat |

## Authentication and administration

| Capability | Status | Notes |
| --- | --- | --- |
| Password login, JWT access, refresh rotation, logout | End to end | Access tokens stay in memory; refresh uses an httpOnly cookie |
| Forgot/reset/change password | End to end | Forgot/reset requires SMTP; password change revokes all active sessions and refresh tokens |
| TOTP 2FA, backup recovery codes, and code-login | End to end | Dedicated `TOTP_ENCRYPTION_KEY`, replay & lockout guards, `super_admin` recovery reset |
| Active session management | End to end | Device/IP tracking and per-session revocation in Settings / Portal |
| WebAuthn passkey registration, sign-in, rename, delete | End to end | Relying-party origin and ID must match the deployed host |
| Google OAuth2 authorize, callback, link, unlink | Conditional | Routed at `/oauth2/callback` and `/oauth2/link`; requires Google OAuth2 client credentials |
| Cloudflare Turnstile bot protection | Conditional | Enforced fail-closed on unauthenticated credential-free endpoints when `TURNSTILE_SECRET_KEY` is set |
| Multi-company membership and active-company switching | End to end | Switching reissues the JWT with a new company context |
| Company create/update/delete | End to end | Super-admin operation |
| One-time initial administrator bootstrap | End to end | Explicit `bootstrap_admin` CLI; no database-seeded credential |
| User create/update/deactivate and company assignment | End to end | Super-admin page and APIs with search and pagination |
| Additive company-scoped user groups & permissions | End to end | `UserGroups` UI and `PermissionGuard` routing backed by per-request permission union |
| Role matrix page | Informational | Displays built-in role baseline alongside customizable user groups |
| Password-reset administration page | Unwired | Legacy unrouted client calls; admin 2FA reset and email reset links cover recovery |

## Company and employee management

| Capability | Status | Notes |
| --- | --- | --- |
| Company profile, payroll defaults, attendance settings, summary statistics | End to end | Company-scoped |
| Employee list/search/filter, create, edit, soft delete | End to end | Search is backed by a trigram expression index |
| Employment, bank, statutory, compensation, and contact fields | End to end | Sensitive fields require appropriate HR/payroll access |
| Salary history | End to end | Maintained with employee compensation changes |
| TP3 records | End to end | Backend and employee detail flow |
| Employee portal-account linking/provisioning | End to end | Consolidated data migration also repairs eligible legacy links |
| CSV/XLSX import template, validation preview, confirmation | End to end | Validation sessions expire; confirmation runs asynchronously via `background_jobs` (`202` + `GET /api/jobs/{id}`) |
| Default payroll group selection | End to end | Every company is provisioned with a usable group; management CRUD is not implemented |
| Work schedules and employee schedule overrides | End to end | Company defaults plus per-employee assignment |

## Payroll and statutory processing

| Capability | Status | Notes |
| --- | --- | --- |
| Manual payroll entries | End to end | Earnings/deductions can be added, edited, and removed before processing |
| Monthly payroll calculation & preview | Conditional | Preview surfaces diagnostics (incl. Minimum Wages Order 2024 RM1,700 check & KWSP Parts A–F derivation); runs execute asynchronously via `background_jobs`, and production runs fail closed until official EPF/SOCSO/EIS data is verified and PCB passes LHDN conformance |
| One active run per group and period | End to end | Enforced by service logic and partial unique indexes on `payroll_runs` and `background_jobs` |
| Run detail and employee item review | End to end | Tenant-scoped read models plus line-by-line explainable payslip drawer |
| Manual PCB adjustment before approval | Conditional | Recomputes totals under row locks for an existing non-production/test run; it does not enable production PCB |
| Submit, approve, return, mark paid, delete eligible run | End to end | Explicit permission, four-eyes maker-checker, and state-transition checks |
| Cancel a run before payment | End to end | Mandatory reason; preparer cancels draft/processed, approver cancels pending/approved; staged entries/claims revert in-transaction |
| Reverse a paid run | End to end | `PUT /api/payroll/runs/{id}/reverse` (`ApprovePayroll` / finance); mandatory reason, blocked if a later run exists, reverts staged entries/claims and records audit provenance |
| Payroll overview dashboard | End to end | `/payroll`: committed period KPIs, status pipeline, variance vs prior period, 12-period trend, department labour cost, live action queue |
| Bank payment file export | End to end | CSV for approved/paid runs, `mark_payroll_paid`-gated, formula-neutralised, audited without account numbers |
| Journal preview | End to end | Balanced DR/CR from committed figures with department allocation; fixed default account codes, nothing posted to a ledger |
| Salary change classification | End to end | `change_type` vocabulary + notes on `salary_history`; round-trips through backup/restore |
| Individual and batch payslip PDFs | End to end | Generated by the backend |
| EPF, SOCSO, EIS, and PCB reports/exports | Conditional | Report/export paths exist; source data and PCB output require independent verification first |
| EA form data and export | End to end | Available through report endpoints/UI |

## Attendance, kiosk, and geofence

| Capability | Status | Notes |
| --- | --- | --- |
| Admin-generated QR and employee QR check-in | End to end | One QR can serve many employees during its server-provided TTL; the admin console only mints while the QR is actually displayed |
| Public kiosk credentials and rotating QR display | End to end | Kiosk secrets are shown once and stored hashed; expired unreferenced tokens are purged daily |
| Check-out including overnight shifts | End to end | Matches the newest open record within 24 hours; an older open session is reported as such instead of "please check in first". Never blocked by geofence — an off-site check-out is recorded and flagged |
| Employee current-day/history view | End to end | Portal flow; fetch failures surface as retryable errors rather than an empty "not checked in" state |
| HR list, manual entry, correction, filters | End to end | Company-scoped and audit logged. Manual entry picks employees by name/number and rejects cross-company ids; corrections require a reason and support explicit clearing of check-out/notes |
| Open-session triage | End to end | `open_only` filter plus a "Still Checked In" tile for finding sessions nobody closed |
| Per-employee attendance summary and CSV export | End to end | Includes employees with zero matching rows. Counts are per distinct local day (a split shift is one day; a superseded absence is not double-counted). Export defaults to the current month when no range is given |
| Locations and `none`/`warn`/`enforce` geofence modes | End to end | Coordinate/radius validity is enforced by the schema; clients skip the GPS wait when the mode is `none` |
| Office-network binding (`none`/`learn`/`warn`/`enforce`) | End to end | Evaluates server-observed egress IP prefixes against admin-approved `company_networks`; learning aggregates anchored candidates without exposing `employee_id → IP` mappings |
| Automatic absence marking | End to end | In-process scheduler; safe for the current single API instance. Runs as a catch-up from the last bookmarked success, so a missed window is recovered rather than skipped, and HR can re-run a past date on demand |
| Face ID check-in | End to end | A dedicated challenge is issued per check-in and the WebAuthn assertion is verified server-side against the employee's registered passkeys before the record is written |

## Employee self-service and approvals

| Capability | Status | Notes |
| --- | --- | --- |
| Profile and payslip history/download | End to end | Employee-linked account required |
| Leave types, balances, requests, cancellation, ICS export | End to end | Standard types are provisioned; type-management CRUD is not implemented |
| Claims create/edit/submit/cancel/delete | End to end | Receipt upload supported |
| Overtime create/edit/submit/cancel/delete | End to end | Type and hours feed payroll when approved |
| Leave, claim, and overtime approval workbench | End to end | Admin/HR create, review, approve, reject, and edit flows |
| Team calendar and public holidays | End to end | Company-aware calendar projections |
| Notifications, unread count, mark read | End to end | Portal UI and API |

## Documents, communication, and collaboration

| Capability | Status | Notes |
| --- | --- | --- |
| Document metadata, categories, expiry tracking | End to end | Company and optional employee association |
| File upload/download | End to end | Stored on local API disk (`uploaded_files`), gated by extension, 10 MB cap, magic-byte validation, canonical path containment, and per-user reverse-reference authorization (`404` on denial) |
| Letter/email templates, preview, send, delivery logs | Conditional | SMTP is required for delivery; initial passwords redacted from stored `email_logs` |
| Holiday CRUD and working-day configuration | End to end | Includes `.ics` file and SSRF-hardened URL import (`core::http_client`) |
| Teams and team membership | End to end | Used by calendars and organizational views |

## Reporting, localization, legal, and operations

| Capability | Status | Notes |
| --- | --- | --- |
| Payroll, department, leave, claims, statutory reports | End to end | Reachable reports page and formula-neutralised export actions |
| Workspace dashboard (`/dashboard`) | End to end | Headcount, pending approvals, attendance exceptions, YTD statutory breakdown, and shortcuts |
| Four-locale internationalization (`en`, `ms`, `zh-CN`, `zh-TW`) | End to end | Full UI + server-error translation parity, script-purity tests, and CI hardcoded-string scanner |
| Dark/light/system theme & global command palette (`⌘K`) | End to end | Accessible across Admin and Portal shells |
| Terms, Privacy/PDPA Notice, and 2026 Statutory Guide | End to end | Public & in-app routes (`/terms`, `/privacy`, `/compliance`) plus `docs/legal/*` |
| Audit trail & PDPA retention purges | End to end | Filtered administrative view plus automated daily retention purges (7-year `2,555`-day audit horizon) |
| Company JSON backup/export and restore/import | End to end | Tenant-bound restore with `upload_path` filename sanitization and magic-byte verification |
| Settings and platform attendance defaults | End to end | Company and super-admin flows |
| Liveness and database-readiness probes | End to end | Readiness checks connectivity and migration presence |
| PostgreSQL schema/reference & forward migrations | End to end | Consolidated `1000`/`1001` baseline plus immutable forward chain `1002`–`1030`; no demo credentials |
| Docker Compose and GitHub Actions | End to end | Both pin PostgreSQL 19 Beta 2 and enforce forward-only migration immutability |

## Explicit non-features

- The product has no published OpenAPI contract yet.
- The "Face ID" label triggers device-local W3C WebAuthn passkey verification; it is not a server-side biometric recognition service.
- Uploads live on the API container's local disk rather than external object storage (S3).
- Redis is not part of the application runtime; background tasks run in-process for a single API instance.
- Statutory reference data is not automatically synchronized with Malaysian authorities and must be verified against official KWSP, PERKESO, and LHDN publications before live production use.
