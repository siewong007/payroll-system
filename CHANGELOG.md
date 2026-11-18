# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Legal, PDPA & 2026 Malaysian statutory compliance suite** — added
  [docs/legal/terms-and-conditions.md](docs/legal/terms-and-conditions.md),
  bilingual English/Bahasa Malaysia
  [docs/legal/privacy-and-pdpa-notice.md](docs/legal/privacy-and-pdpa-notice.md)
  (aligned with the _Personal Data Protection (Amendment) Act 2024 [Act A1727]_
  DPO, 72-hour data breach notification, data portability, and biometric
  WebAuthn boundaries),
  [docs/legal/malaysian-statutory-compliance.md](docs/legal/malaysian-statutory-compliance.md),
  and localized in-app `/terms`, `/privacy`, and `/compliance` pages across
  `en`, `ms`, `zh-CN`, and `zh-TW`.
- **KWSP Third Schedule Parts A–F & Minimum Wages Order 2024 alignment** —
  expanded `epf_service` and `employees_epf_category_check` (`1029`) to support
  Malaysian citizen Part E (aged 60–74: 0% employee / 4% employer), PR Part C
  (aged 60–74), mandatory foreign-worker Part F (2% + 2% effective 1 October
  2025 under _EPF Amendment Act 2025_), and statutory cessation at age 75
  (_EPF Act 1991 s.43(1)_); added `below_minimum_wage` preview warnings for
  full-time salaries below the RM1,700/month floor (_Minimum Wages Order 2024
  [P.U. (A) 376/2024]_); and normalized `permanent_resident` / `epf_category`
  validation in bulk employee imports.
- **Workspace UX & security hardening** — enhanced workspace dashboard, dark
  mode, global command palette, dialog UX, upload ownership registry
  (`uploaded_files`), TOTP replay/lockout guards, and local nightly encrypted
  backups on the AIC VPS following AWS infrastructure decommission.

### Changed

- Upgraded Rust backend (`calamine`, `rust_xlsxwriter`, `tower-http`) and
  frontend (`axios`, `zod`, `tailwindcss`, `vite`, `eslint`, `vitest`)
  dependencies and synchronized architecture, database, feature, security, and
  payroll documentation with migrations `1000`–`1030`.

## [0.2.0] - 2026-09-20

### Added

- **Background jobs** — payroll processing and whole-headcount employee imports
  now run as background jobs (`background_jobs` table): the endpoints enqueue a
  row and answer 202, an in-process executor writes throttled progress and the
  result for `GET /api/jobs/{id}` to poll, a partial unique index dedups
  concurrent submissions of the same (company, group, period) into a friendly
  409, and stale pending/running rows are marked failed at boot.

### Fixed

- **Deploy pipeline** — restored `1025_payroll_ops.sql` to the bytes production
  installed and moved the post-merge edit's intent into
  `1027_cancelled_run_timestamp_backfill.sql` (legacy cancelled runs get
  `cancelled_at` backfilled from `updated_at`; the consistency check now
  requires the timestamp only). The in-place edit had changed the installed
  checksum, so every post-merge deploy panicked on `VersionMismatch(1025)` —
  and, having applied the out-of-order `1024` first, crash-looped the rollback
  image too, taking the production API down.
- **CI migration gate** — the immutability check diffed `origin/main...HEAD`,
  which on `push` events compares the pushed commit to itself; it now diffs the
  pre-push tip on pushes and supports an explicit `[migration-override]`
  commit-message marker for deliberate restores.
- **Deploy smoke** — the post-deploy login probe now also accepts 400/403,
  the fail-closed statuses of the Turnstile gate for a token-less request.

## [0.1.0] - 2026-09-20

First release.

### Added

- **Payroll engine** — run preview, payslip provenance, per-run statutory rule
  loading, run cancellation and paid-run reversal, payment files, journal
  preview, and an operational overview.
- **Statutory services** — EPF, SOCSO, EIS, and PCB calculation; statutory
  export files; payslip PDFs; EA form generation.
- **Attendance** — QR kiosk check-in with multi-use expiring tokens, Face ID
  (WebAuthn) check-in, geofencing, office-network binding with
  learn/warn/enforce modes, self-healing auto-absent marking, audited
  corrections, per-employee summaries, and CSV export.
- **Auth & security** — JWT sessions with refresh cookies and device/IP
  tracking, TOTP 2FA enforced for privileged roles, passkeys, Google OAuth2
  account linking, Cloudflare Turnstile bot protection, per-route and
  per-session rate limiting, and a user-groups permission layer.
- **Audit trail** — covers the authentication and credential lifecycle,
  payroll, attendance, admin actions, and company/settings changes.
- **Employee management** — bulk import with portal-account provisioning and
  leave balances, leave requests with approval inboxes, HR letters, reports,
  and user management with search and pagination.
- **Employee portal** — self-service check-in, leave, payslips, and profile.
- **Multi-company** — users can belong to several companies and switch the
  active company, re-issuing the JWT scope.
- **i18n** — full `en`, `ms`, `zh-CN`, and `zh-TW` locale support.
- **Ops** — encrypted nightly off-host backups with secrets escrow, retention
  purges for append-only tables, correlation IDs, per-request JSON telemetry,
  graceful shutdown, and AWS infrastructure as Terraform.

### Notes

- Requires PostgreSQL 19 Beta 2 (native `uuidv7()`); PostgreSQL 18 is supported
  only under the documented AWS RDS 18.4 compatibility exception.
- Bundled statutory rule rows are unverified academic fixtures: production
  payroll fails closed, and automatic PCB stays disabled until the calculator
  passes LHDN computerised-MTD conformance.

[0.2.0]: https://github.com/siewong007/payroll-system/releases/tag/v0.2.0
[0.1.0]: https://github.com/siewong007/payroll-system/releases/tag/v0.1.0
