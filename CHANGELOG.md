# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
