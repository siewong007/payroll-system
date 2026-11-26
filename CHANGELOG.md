# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **PWA installability & Web Push notifications (`1034_push_notifications.sql`)** —
  added web manifest (`frontend/public/manifest.webmanifest`), service worker
  (`frontend/public/sw.js`) with offline fallback (`/offline.html`) and
  `OfflineBanner`, iOS/Android/desktop install & notification activation cards
  (`AppInstallCard`, `NotificationActivationCard`), `push_subscriptions` and
  durable `push_outbox` tables, RFC 8291 AES-128-GCM + RFC 8292 VAPID ES256
  delivery worker (`push_service.rs`, `scripts/generate-vapid-keys.mjs`),
  `/api/notifications/{push-config,subscriptions,test-push}`, and reference host
  Caddy config (`deploy/payrollmy-site.Caddyfile.example`).
- **`manager` role (`1031_manager_role.sql`) & platform/account consoles** —
  added the `manager` company-management role (excluding payroll, bulk import,
  and privileged `admin`/`super_admin` directory visibility), added
  `PlatformSettingsPage` (`/admin/platform`) for super-admin 2FA policy and
  background-job health inspection, and added `AccountSecurityPage`
  (`/account`) for self-service security, sessions, and PWA settings.
- **Legal, PDPA & 2026 Malaysian statutory compliance suite** — added
  consolidated [docs/legal.md](docs/legal.md) (Terms of Use, bilingual
  English/Bahasa Malaysia PDPA Notice aligned with the _Personal Data Protection
  (Amendment) Act 2024 [Act A1727]_, and 2026 Malaysian Statutory Compliance
  Guide) alongside localized in-app `/terms`, `/privacy`, and `/compliance`
  pages across `en`, `ms`, `zh-CN`, and `zh-TW`.
- **KWSP Third Schedule Parts A–F & Minimum Wages Order 2024 alignment** —
  expanded `epf_service` and `employees_epf_category_check` (`1029`) to support
  Malaysian citizen Part E (aged 60–74: 0% employee / 4% employer), PR Part C
  (aged 60–74), mandatory foreign-worker Part F (2% + 2% effective 1 October
  2025 under _EPF Amendment Act 2025_), and statutory cessation at age 75
  (_EPF Act 1991 s.43(1)_); added `below_minimum_wage` preview warnings for
  salaries below the RM1,700/month floor (_Minimum Wages Order 2024_); and
  normalized `permanent_resident` / `epf_category` handling.
- **Monorepo standards & modularity** — added root workspace scripts
  (`package.json`), Husky + `lint-staged`, Prettier, pinned runtimes
  (`.bun-version`, `rust-toolchain.toml`), centralized frontend route constants
  (`frontend/src/lib/routes.ts`), modularized oversized page modules, and added
  accessibility enhancements (skip-to-content links, 44px mobile touch targets).

### Changed

- **Phase 3 hardening & full-application audit fixes (`1032`, `1033`)** —
  added O(1) keyed SHA-256 TOTP backup-code lookup, retryable email outbox
  columns and worker, 10 employee domain CHECK constraints, `payroll_items.taxable_gross`
  for YTD PCB/EA accuracy, explicit `overtime_application_id`/`leave_request_id`
  FKs on `payroll_entries`, dropped `DEFAULT 'A'` on `employees.epf_category`
  for automatic category derivation, added `claims`/`tp3_records` CHECKs and
  unique `(user_id, provider)` index on `oauth2_accounts`, moved all remaining
  inline SQL into `repositories/`, rejected `.env.example` placeholder secrets in
  production, and redacted PII emails in `email_service` logs.
- **CI, deployment & documentation consolidation** — enabled CI/deploy
  workflows on both `main` and `master`, upgraded Rust & frontend dependencies,
  synchronized documentation with migrations `1000`–`1034`, consolidated docs to
  one canonical file per document type (`AGENTS.md`, `docs/*.md`), and erased
  implemented and stale plans/audits.

### Security

- **Bank details are payroll-only** — employee `bank_*` fields are now redacted
  with the payroll figures (`redact_payroll_fields`) on every employee
  list/detail/create/update response for callers without `view_payroll`
  (e.g. `hr_manager`, `manager`); audit diffs redact any `bank_*` key;
  `clear_bank_name`/`clear_bank_account_number` are rejected for those roles so
  an HR save cannot wipe stored banking; the employee detail and profile
  drawer show banking as restricted. Payroll roles and the employee's own
  portal profile are unchanged.

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
  `1027_cancelled_run_timestamp_backfill.sql`.
- **CI migration gate** — fixed push-event diffing against the pre-push tip and
  added `[migration-override]` marker support for deliberate restores.
- **Deploy smoke** — accepted 400/403 fail-closed Turnstile responses on
  token-less login probes.

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
- **Audit trail, employee management, portal, multi-company & i18n** —
  credential/workflow audit trail, bulk employee import, self-service portal,
  multi-company switching, four-locale i18n (`en`, `ms`, `zh-CN`, `zh-TW`), and
  production automation.

### Notes

- Requires PostgreSQL 19 Beta 2 (native `uuidv7()`); PostgreSQL 18 is supported
  only under the documented AWS RDS 18.4 compatibility exception.
- Bundled statutory rule rows are unverified academic fixtures: production
  payroll fails closed, and automatic PCB stays disabled until the calculator
  passes LHDN computerised-MTD conformance.

[0.2.0]: https://github.com/siewong007/payroll-system/releases/tag/v0.2.0
[0.1.0]: https://github.com/siewong007/payroll-system/releases/tag/v0.1.0
