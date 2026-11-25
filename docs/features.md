# Implemented feature catalogue

This catalogue distinguishes capabilities usable end to end from those gated by external configuration or left on the project roadmap. It is the feature source of truth for the repository.

## Status definitions

| Status           | Meaning                                                                        |
| ---------------- | ------------------------------------------------------------------------------ |
| End to end       | Backend route, persistence/business logic, and a reachable frontend flow exist |
| Conditional      | Implemented but requires external configuration or infrastructure              |
| Informational    | UI explains or displays policy without persisting mutations                    |
| Known limitation | A user-visible flow exists with a material security or completeness caveat     |

## Authentication and administration

| Capability                                                     | Status        | Notes                                                                                                    |
| -------------------------------------------------------------- | ------------- | -------------------------------------------------------------------------------------------------------- |
| Password login, JWT access, refresh rotation, logout           | End to end    | Access tokens stay in memory; refresh uses an `httpOnly` cookie with atomic reuse detection              |
| Forgot/reset/change password                                   | End to end    | Forgot/reset requires SMTP; password change revokes all active sessions and refresh tokens               |
| TOTP 2FA, backup recovery codes, and code-login                | End to end    | Dedicated `TOTP_ENCRYPTION_KEY`, O(1) keyed SHA-256 backup codes (`1032`), replay/lockout guards         |
| Active session management & `/account` page                    | End to end    | Device/IP tracking and per-session revocation at `/account` (all `AppLayout` users) and `/portal`        |
| WebAuthn passkey registration, sign-in, rename, delete         | End to end    | Relying-party origin and ID must match the deployed host                                                 |
| Google OAuth2 authorize, callback, link, unlink                | Conditional   | Browser-bound state cookie; routed at `/oauth2/callback` and `/oauth2/link`; requires Google credentials |
| Cloudflare Turnstile bot protection                            | Conditional   | Fail-closed on unauthenticated credential-free endpoints when `TURNSTILE_SECRET_KEY` is set              |
| Multi-company membership and active-company switching          | End to end    | Switching reissues the JWT with a new `company_id` scope                                                 |
| Company create/update/delete                                   | End to end    | `super_admin` operation with cascade wipe                                                                |
| One-time initial administrator bootstrap                       | End to end    | Explicit `bootstrap_admin` CLI; no database-seeded credentials                                           |
| User create/update/deactivate and company assignment           | End to end    | `super_admin` page (`/users`), search/pagination, privileged-user hiding for `manager`                   |
| 8 canonical roles (`super_admin`..`employee`, incl. `manager`) | End to end    | Enforced via `core::permission::Permission` and `users_roles_valid` (`1031`)                             |
| Additive company-scoped user groups & `PermissionGuard`        | End to end    | Blocks platform-only grants and `exec`/`manager`/`employee` payroll escalation                           |
| Platform Settings & Health console (`/admin/platform`)         | End to end    | `super_admin` UI for 2FA enforcement policy and live background-job health/error inspection              |
| Role matrix page (`/roles`)                                    | Informational | Displays built-in role baseline read from `GET /api/auth/permissions/matrix`                             |

## Company and employee management

| Capability                                                                 | Status     | Notes                                                                                                  |
| -------------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------ |
| Company profile, payroll defaults, attendance settings, summary statistics | End to end | Company-scoped                                                                                         |
| Employee list/search/filter, create, edit, soft delete                     | End to end | Trigram-indexed search, `date_resigned` editing, non-negative/chronological DB checks (`1032`)         |
| Employment, bank, statutory, compensation, and contact fields              | End to end | Payroll fields gated behind payroll permissions                                                        |
| Salary history with change classification                                  | End to end | `change_type` vocabulary + notes (`1025`, `1033`), round-trips through backup/restore                  |
| TP3 records                                                                | End to end | Backend, employee detail flow, and non-negative/year checks (`1033`)                                   |
| Employee portal-account linking/provisioning                               | End to end | Random initial passwords (redacted from `email_logs`) with forced first-login password change          |
| CSV/XLSX import template, validation preview, confirmation                 | End to end | Zip-bomb guarded XLSX parsing; async confirmation via `background_jobs` (`202` + `GET /api/jobs/{id}`) |
| Default payroll group selection                                            | End to end | Provisioned per company; multi-group management CRUD is on the roadmap                                 |
| Work schedules                                                             | End to end | Company default schedule and timezone; per-employee override write UI is on the roadmap                |

## Payroll and statutory processing

| Capability                                              | Status      | Notes                                                                                                                                                     |
| ------------------------------------------------------- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Manual payroll entries                                  | End to end  | Earnings/deductions added/edited/removed before processing; OT & leave links (`1033`) prevent collateral cancel deletions                                 |
| Monthly payroll calculation & preview                   | Conditional | Preview checks Minimum Wages Order 2024 RM1,700, KWSP Parts A–F (`epf_category` auto-derived unless overridden), `taxable_gross`; production fails closed |
| One active run per group and period                     | End to end  | Enforced by service logic and partial unique indexes on `payroll_runs` and `background_jobs`                                                              |
| Run detail, breakdown drawer, submit/approve/pay/delete | End to end  | Four-eyes maker-checker (`payroll_admin` vs `finance`), explainable `payroll_item_details`, `calculation_snapshot` provenance                             |
| Manual PCB adjustment before approval                   | Conditional | Recomputes totals and `payroll_item_details` under row locks on non-production runs; does not enable automatic PCB                                        |
| Cancel before payment & reverse paid runs               | End to end  | Mandatory reason; `cancel` for pre-paid states, `reverse` for paid runs; reverts staged entries/claims atomically                                         |
| Payroll overview dashboard (`/payroll`)                 | End to end  | Committed KPIs, status pipeline, prior-period variance, 12-period trend, department labour cost, action queue                                             |
| Bank payment file CSV & GL journal preview              | End to end  | Formula-neutralised CSV (`mark_payroll_paid`), balanced DR/CR preview (no ledger posting)                                                                 |
| Individual and batch payslip PDFs                       | End to end  | Generated backend PDFs rendered from `payroll_item_details`                                                                                               |
| EPF, SOCSO, EIS, PCB exports & Form EA                  | Conditional | Paths implemented and formula-neutralised; live statutory runs require verified official schedules and LHDN conformance                                   |

## Attendance, kiosk, and geofence

| Capability                                               | Status     | Notes                                                                                                                  |
| -------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| Admin-generated QR and employee QR check-in              | End to end | Multi-use QR tokens within server-supplied TTL; scoped per display surface                                             |
| Public kiosk credentials and rotating QR display         | End to end | Secrets shown once, stored hashed, and passed via fragment `/kiosk#<secret>` (scrubbed via `history.replaceState`)     |
| Check-out including overnight shifts                     | End to end | Matches newest open record within 24 hours; older open sessions prompt admin correction; never blocked by geofence     |
| Employee current-day/history view                        | End to end | Portal flow with retryable error states                                                                                |
| HR list, manual entry, correction, open-session triage   | End to end | Company-scoped, audit logged, reason required, explicit field clear flags, `open_only` filter                          |
| Per-employee attendance summary and CSV export           | End to end | Distinct local-day counts (`late > half_day > present > absent`); export defaults to current local month               |
| Geofence locations (`none`/`warn`/`enforce`)             | End to end | Schema-validated coordinates/radii; skips GPS wait when mode is `none`                                                 |
| Office-network binding (`none`/`learn`/`warn`/`enforce`) | End to end | Evaluates server-observed client IP prefixes; aggregates anchored candidates without exposing `employee_id → IP`       |
| Self-healing auto-absent catch-up                        | End to end | Daily 04:30 UTC + startup catch-up per company timezone (`1020`, `1023`), plus admin `POST /api/attendance/absent-run` |
| Face ID check-in                                         | End to end | Per-check-in WebAuthn challenge verified server-side against registered passkeys                                       |

## Employee self-service, approvals, and collaboration

| Capability                                                | Status      | Notes                                                                                                     |
| --------------------------------------------------------- | ----------- | --------------------------------------------------------------------------------------------------------- |
| Profile and payslip history/PDF download                  | End to end  | Employee-linked account required                                                                          |
| Leave types, balances, requests, cancellation, ICS export | End to end  | Standard EA 1955 leave types provisioned per company; type CRUD & year-end UI on roadmap                  |
| Claims & overtime create/edit/submit/cancel/delete        | End to end  | Receipt uploads, duration bounds, positive amount checks (`1033`), payroll consumption                    |
| Leave, claim, and overtime approval workbench             | End to end  | Maker-checker self-approval prevention and transactional balance/entry updates                            |
| Team calendar, public holidays, & working days            | End to end  | Includes `.ics` file upload and SSRF-hardened URL import (`core::http_client`)                            |
| Document metadata, categories, expiry tracking & uploads  | End to end  | Local API `uploads/` tracked in `uploaded_files`, magic-byte + 10 MB + single-segment containment (`404`) |
| Letter/email templates, preview, async outbox & logs      | Conditional | Requires SMTP for delivery; retryable outbox (`1032`), initial passwords redacted from `email_logs`       |
| Teams and team membership                                 | End to end  | Company-scoped teams and calendar integration                                                             |

## PWA, notifications, localization, legal, and operations

| Capability                                                      | Status      | Notes                                                                                                                   |
| --------------------------------------------------------------- | ----------- | ----------------------------------------------------------------------------------------------------------------------- |
| In-app notifications (list, unread count, mark read)            | End to end  | Portal UI and API                                                                                                       |
| Installable PWA & offline fallback                              | End to end  | `manifest.webmanifest`, `sw.js`, `offline.html`, `OfflineBanner`, iOS/Android/desktop `AppInstallCard`                  |
| Web Push notifications (VAPID + durable push outbox)            | Conditional | Migration `1034`, `push_service`, `NotificationActivationCard`, `/api/notifications/test-push`; requires `VAPID_*` keys |
| Reporting & workspace dashboard (`/dashboard`, `/reports`)      | End to end  | Headcount, approvals, attendance exceptions, YTD statutory breakdown, RFC-4180 formula-neutralised CSV exports          |
| Four-locale internationalization (`en`, `ms`, `zh-CN`, `zh-TW`) | End to end  | Compile-time `Messages` parity, `i18n.test.ts` script purity, `scripts/i18n-scan.mjs --ci`                              |
| Dark/light/system appearance & command palette (`⌘K`)           | End to end  | Available across `AppLayout` and `PortalLayout`                                                                         |
| Legal, PDPA & 2026 Malaysian Statutory Guide                    | End to end  | Public & in-app routes (`/terms`, `/privacy`, `/compliance`) + [legal.md](legal.md)                                     |
| Audit trail & automated daily PDPA retention purges             | End to end  | 7-year (`2,555`-day) audit retention alongside daily operational purges                                                 |
| Company JSON backup/export and restore/import                   | End to end  | `REPEATABLE READ READ ONLY` snapshot export; filename-contained & magic-byte validated restore                          |
| Readiness & health telemetry (`/api/health/ready`)              | End to end  | Reports DB, build `revision`, and all 4 background-loop heartbeats; `deploy/payroll-watchdog.sh` host monitoring        |
| PostgreSQL 19 Beta 2 migrations & CI gates                      | End to end  | Baseline `1000`–`1001` + immutable `1002`–`1034`; CI tests migrations, SQLx cache, clippy, Vitest, Docker, and audits   |

## Explicit non-features and roadmap boundaries

- **No published OpenAPI spec yet** — backend/frontend DTO parity is maintained manually in `frontend/src/types/`.
- **WebAuthn "Face ID"** verifies device-local cryptographic assertions; the server never receives or stores facial images or biometric templates.
- **Local storage and single-worker execution** — uploads live on local API disk rather than S3, and background jobs run in-process without a Redis tier.
- **Manual statutory verification gate** — bundled `1001_data.sql` rows are unverified academic fixtures; production runs require verified KWSP/PERKESO schedules and LHDN computerised-MTD conformance.
- **Deferred product items** — Form E/CP8D & CP22/CP22A exports, year-end leave carry-forward UI, per-employee work-schedule write UI, leave-type/payroll-group CRUD, automated HRD Corp levy booking, document-expiry/probation alerts, batch/portal EA forms, offboarding settlement, and Playwright browser E2E journeys remain future enhancements.
