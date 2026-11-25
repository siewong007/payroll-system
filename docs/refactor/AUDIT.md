# Audit Report: Payroll System

## Stack & Architecture Summary

Payroll System (`PayrollMY`) is a full-stack Malaysian SME payroll, attendance, statutory compliance, and HR workflow application.

- **Backend**: Rust 2024, Axum 0.8, Tokio, SQLx 0.8 (compile-time checked offline cache in `backend/.sqlx/`), Tower HTTP, `tower_governor`, `rust_decimal`, `printpdf`, `rust_xlsxwriter`, `calamine`, `lettre`, `webauthn-rs`, `totp-rs`, `aes-gcm`.
- **Frontend**: React 19, TypeScript 7 (`strict: true` across `tsconfig.app.json`, `tsconfig.node.json`, `tsconfig.test.json`), Vite 8, Tailwind CSS v4, React Router 8, TanStack Query 5, React Hook Form + Zod, i18next (`en`, `ms`, `zh-CN`, `zh-TW`), Axios (single client in `frontend/src/api/client.ts`).
- **Database**: PostgreSQL 19 Beta 2 (`uuidv7()`, consolidated baseline `1000_schema.sql` + `1001_data.sql` and immutable forward migrations `1002`–`1033`).
- **Authentication & Security**: HS256 JWT access token (in-memory on frontend) + rotated httpOnly refresh cookie, bcrypt password hashing, AES-GCM encrypted TOTP 2FA + backup recovery codes + code login, WebAuthn passkeys ("Face ID" check-in and sign-in), Google OAuth2, Cloudflare Turnstile bot protection, per-IP and per-session `tower_governor` rate limiting, strict company-scoped tenant checks + database foreign-key/trigger guards.
- **Hosting & Tooling**: Docker Compose (`postgres:19beta2-alpine`), multi-stage `infra/Dockerfile` (`cargo-chef` + `debian:bookworm-slim`), GitHub Actions CI (`ci.yml`, `deploy.yml`, `deploy-backend.yml`, `migrate-database.yml`), AIC VPS behind Cloudflare + host Caddy serving the SPA from `/opt/payrollmy-site`, Bun `1.3.14` + Husky + `lint-staged` + Prettier + ESLint + `scripts/i18n-scan.mjs`.

---

## Feature & Architecture Map

### Frontend Routes (`frontend/src/App.tsx`)

- **Public & Auth**: `/login`, `/oauth2/callback`, `/oauth2/link`, `/forgot-password`, `/reset-password`, `/change-password`, `/attendance/scan`, `/kiosk`, `/kiosk/:kioskKey`, `/terms`, `/privacy`, `/compliance`, `/403`, `*` (`NotFoundPage`).
- **Admin / Management Shell (`AppLayout` + `PermissionGuard`)**: `/`, `/dashboard`, `/company`, `/employees`, `/employees/new`, `/employees/import`, `/employees/:id`, `/payroll`, `/payroll/runs`, `/payroll/process`, `/payroll/:id`, `/documents`, `/calendar`, `/teams`, `/approvals`, `/reports`, `/letters`, `/settings`, `/companies`, `/users`, `/roles`, `/user-groups`, `/backup`, `/audit-trail`, `/attendance`, `/admin/attendance-settings`, `/my/attendance`.
- **Employee Self-Service Shell (`PortalLayout`)**: `/portal` -> `/portal/profile`, `/portal/payslips`, `/portal/leave`, `/portal/claims`, `/portal/overtime`, `/portal/team-calendar`, `/portal/notifications`, `/portal/attendance`.

### Backend API Modules (`backend/src/routes/mod.rs`, nested under `/api`)

- **Health**: `GET /health`, `GET /health/ready`
- **Auth, 2FA, Passkeys, OAuth2, Sessions & Groups**: `/auth/*`, `/auth/2fa/*`, `/auth/passkey/*`, `/auth/oauth2/*`, `/auth/sessions*`, `/auth/permissions/matrix`, `/user-groups*`
- **Platform Admin**: `/admin/companies*`, `/admin/users*`, `/admin/users/{id}/2fa/reset`, `/admin/platform/2fa-policy`, `/admin/platform/attendance-method`, `/admin/backup/*`
- **Employees & Bulk Import**: `/employees*`, `/employees/{id}/salary-history`, `/employees/{id}/tp3`, `/employees/{id}/leave-balances/initialize`, `/employees/import/*`, `/leave/year-end`
- **Payroll & Jobs**: `/payroll-groups`, `/payroll/overview`, `/payroll/preview`, `/payroll/run` (`202`), `/payroll/runs*`, `/payroll/entries*`, `/jobs/{id}`
- **Portal & Uploads**: `/portal/profile`, `/portal/payslips*`, `/portal/leave/*`, `/portal/teams`, `/portal/team-calendar`, `/portal/holidays`, `/portal/claims*`, `/portal/overtime*`, `/uploads` (`POST`), `/uploads/{filename}` (`GET`)
- **Approvals**: `/approvals/leave*`, `/approvals/claims*`, `/approvals/overtime*`
- **Attendance, Work Schedules, Geofence & Networks**: `/attendance/method`, `/attendance/company-method`, `/attendance/qr/generate`, `/attendance/kiosks*`, `/attendance/kiosk/qr`, `/attendance/check-in/*`, `/attendance/check-out`, `/attendance/my*`, `/attendance/summary`, `/attendance/export`, `/attendance/records*`, `/attendance/manual`, `/attendance/absent-run`, `/attendance/networks*`, `/work-schedules*`, `/geofence/*`
- **Calendar, Teams, Documents, Email, Reports & Audit**: `/calendar/*`, `/teams*`, `/documents*`, `/settings*`, `/company*`, `/dashboard/summary`, `/notifications*`, `/reports/*`, `/email/*`, `/audit-logs*`

### Data Models, Background Jobs & Roles

- **Domain Models (`backend/src/models/*`)**: `approval`, `attendance`, `attendance_kiosk`, `audit`, `audit_filter`, `background_job`, `backup`, `calendar`, `company`, `company_location`, `company_network`, `dashboard`, `document`, `ea_form`, `email`, `employee`, `employee_import`, `notification`, `oauth2`, `pagination`, `passkey`, `payroll`, `payslip`, `portal`, `report`, `session`, `setting`, `statutory`, `team`, `totp`, `upload`, `user`, `user_company`, `user_group`, `work_schedule`.
- **Background Jobs (`backend/src/main.rs` + `services/job_service.rs`)**:
  1. Daily PDPA retention purge (advisory lock `7_492_026_101`) across `refresh_tokens`, `attendance_qr_tokens`, `attendance_network_observations`, `audit_logs` (2,555 days / 7 years), `notifications`, `email_logs`, `user_sessions`, `bulk_import_sessions`, `password_reset_requests`, `passkey_challenges`, `oauth2_states`, and `background_jobs`.
  2. Daily auto-absent catch-up at 04:30 UTC (12:30 PM MYT) via `core::schedule::next_daily_run_utc` + startup recovery, evaluating per-company timezone while skipping leave/holidays/non-working days.
  3. In-process `background_jobs` async executor (`payroll_run`, `employee_import`) with boot-time recovery of stale in-flight jobs.
- **Canonical Roles & Capabilities**: `super_admin`, `admin`, `payroll_admin`, `hr_manager`, `finance`, `exec`, `employee`, plus additive company-scoped `user_groups`.

---

## Problems Found (Grouped by Severity)

### Critical

- None observed. Zero hardcoded secret patterns exist in tracked files (`git grep`), SQL queries are parameterized via SQLx, and company tenancy is checked at both application and schema levels.

### High

1. **Incomplete placeholder-secret rejection in production (`backend/src/core/config.rs`)**: `validate_production_secrets` checks `WEAK_PREFIXES` (`change-me`, `secret`, `your-`, `payroll_secret`) and length `< 32`, but `.env.example` ships `replace-with-at-least-32-random-characters` (39 chars) and `replace-with-a-different-at-least-32-random-characters` (51 chars), which currently bypass the guard.
2. **PII email leakage in email service logs & byte-length bug in redactor (`backend/src/services/email_service.rs`, `backend/src/core/redact.rs`)**: `email_service.rs` logs unredacted `recipient_email` in `tracing::info!` / `tracing::warn!` (lines 302, 325, 351), and `redact::email` checks `local.len() == 1` (UTF-8 byte length instead of char count), exposing multi-byte single-char local parts as `李***@...` instead of `*@...`.

### Medium

3. **Phase 1 tooling gaps on initial `refactor/standards` setup**: `.lintstagedrc.json` was listed in `.gitignore`, `.husky/pre-commit` used deprecated Husky v8 sourcing, `cargo fmt --` failed when passed file paths, `.bun-version` / `rust-toolchain.toml` / `.prettierrc.json` / root lifecycle scripts were missing, and unused prettier ESLint packages were installed at root. _(Addressed in Phase 1 `chore:` commit.)_
4. **Architecture & layering violations (`CLAUDE.md` boundary)**: Four SQL queries live outside `backend/src/repositories/` (`main.rs:250`, `services/auth_service.rs:572`, `services/auth_service.rs:681`, `services/statutory_rules.rs:52`), retention windows at `main.rs:250, 269, 353, 375` use bare integer literals, and `handlers/employee.rs::create` performs a `company_service::get_company` + `tokio::spawn` email dispatch inside the HTTP handler.
5. **Type-safety & lint-suppression debt in frontend**: `frontend/src/lib/webauthn.ts` contains 4 `any` casts with `eslint-disable-next-line no-restricted-syntax`; `frontend/src/pages/portal/TeamCalendar.tsx` has 4 uncommented `eslint-disable-next-line react-hooks/exhaustive-deps` suppressions because `intlLocale()` did not accept `i18n.language`. _(Addressed in Phase 1 `refactor:` commit.)_
6. **Oversized frontend page files (>300 lines)**: `Approvals.tsx` (1,814 lines), `AttendancePage.tsx` (1,684 lines), `EmployeeList.tsx` (1,171 lines), and `Reports.tsx` (918 lines) bundle multiple modals, tabs, and sub-components in single files.
7. **Missing UI states, offline awareness, and mobile accessibility gaps**:
   - `frontend/src/pages/portal/TeamCalendar.tsx` omits `isLoading`/`isError`/`error`/`refetch` handling on both `useQuery` calls.
   - No global offline indicator exists when `navigator.onLine` is false.
   - `AppLayout.tsx` and `PortalLayout.tsx` lack a keyboard skip-to-content link targeting `#main-content`, and mobile top-bar icon buttons in `AppLayout.tsx` (`p-2` around 16–20px icons) fall below 44x44px touch targets.
   - Route string literals are duplicated inline across `App.tsx` instead of referencing a shared constant map.

### Low

8. **Undocumented env vars in `.env.example`**: `RUST_ENV`, `LOG_FORMAT`, and `VITE_TURNSTILE_SITE_KEY` are read by code/workflows but omitted from `.env.example`.
9. **Transitive dev-only advisory**: `picomatch <=4.0.7` (`GHSA-fc5q-39xc-vjpf`, via `vite`/`vitest` -> `tinyglobby` -> `fdir`) has no upstream fix yet and is documented/ignored in `.github/workflows/ci.yml` (never bundled for production).
10. **Intentional domain constraints (no action without user decision)**: `1001_data.sql` statutory seed rows are unverified academic fixtures and production PCB stays fail-closed until LHDN computerised-MTD conformance; external error monitoring (Sentry) and Playwright E2E browser journeys are on the project roadmap.

---

## Phase 5 Checklist Status

### Project Essentials

- `README.md` (overview, stack, setup, env vars, scripts, deployment, structure): **present** (will update with root workspace scripts & version pins in Phase 6)
- `.env.example`, `CHANGELOG.md`, `LICENSE`: **present** (`.env.example` completed in Phase 4)
- CI workflow (`.github/workflows/ci.yml` running install, lint, typecheck, test, build, migration checks, security audits): **present**
- Tests (unit & integration suites in `backend/src/tests/` and `frontend/src/tests/`, plus characterization tests for modularized pages): **partial** (632+ backend tests + 34 frontend suites exist; Playwright browser E2E is deferred on roadmap pending user approval)
- Database migrations & seed data (`backend/migrations/`): **present** (immutable, verified in CI)
- Error monitoring & PII-free structured logging (`tracing` JSON + `ErrorBoundary` + `core::redact::email`): **partial** (structured logging & redaction hardened in Phase 4; external Sentry/OTLP SaaS requires user decision)
- Health-check endpoints (`GET /api/health`, `GET /api/health/ready`): **present**
- Documented environments (local Docker Compose + production AIC VPS Caddy/Compose) & deployment scripts: **present**
- `CLAUDE.md` / `AGENTS.md`: **present** (will update with root convenience scripts & runtime pins in Phase 6)

### App and UX Essentials

- Loading, empty, error, and offline states: **present** (`TeamCalendar.tsx` loading/error/retry fix + global `OfflineBanner` implemented in Phase 5)
- Form validation & confirmation before destructive actions (`ConfirmProvider`, `react-hook-form` + `zod`, Rust `validator`): **present**
- Accessibility (screen-reader labels, contrast, >=44px mobile touch targets, skip link, scalable layouts): **present** (skip-to-content link + >=44px mobile header buttons added in `AppLayout`/`PortalLayout` in Phase 5)
- Responsive layouts & safe areas (`viewport-fit=cover`, responsive grids, mobile drawer/bottom nav): **present**
- Shared design tokens & shared component set (`frontend/src/index.css`, `frontend/src/components/ui/*`): **present**
- User-facing text centralized for translation (4 locales + `i18n.test.ts` + `i18n-scan.mjs --ci`): **present**
- Correct handling of time zones and date formats (`Asia/Kuala_Lumpur` / tenant timezone + `intlLocale` + `lib/format.ts`): **present**
- App icon, metadata, not-found page, and favicon (`frontend/index.html`, `NotFoundPage`, SVG branding): **present** (`description`, `theme-color`, `apple-touch-icon` metadata added in Phase 5)
- Notifications (`notifications` backend/portal module + `email_service` SMTP): **present**

### Domain Lifecycle Equivalents ("Rides and events" template mapping)

This repository is a Malaysian Payroll & HR system (not a rides/events app). The template's rides/events items are **N/A by domain**, and their direct domain equivalents are already implemented:

- **Payroll lifecycle**: `processing` -> `processed` -> `pending_approval` -> `approved` -> `paid` / `cancelled`, with single-active-run DB constraint, four-eyes approval (`payroll_admin` vs `finance`), and fail-closed statutory rule verification.
- **Attendance & Leave/Claims/Overtime approval lifecycles**: multi-use kiosk QR + WebAuthn Face ID check-in, overnight 24h check-out window, self-healing auto-absent catch-up, self-approval prevention, and single-payout claim/OT locking.
- **Legal, PDPA & Account Governance**: `/terms`, `/privacy` (bilingual EN/BM PDPA notice), `/compliance`, 7-year audit log + daily PDPA retention purges, admin user deactivation & session revocation.

### Items Requiring User Decision Before Implementation

1. External error-monitoring service integration (e.g., Sentry DSN / OTLP sink) and alert destination.
2. Importing official verified 2026 KWSP/PERKESO statutory datasets and enabling automatic LHDN MTD/PCB calculation.
3. Adding an end-to-end Playwright browser workflow suite (`tests/e2e/`) and CI execution job.
4. Any future database schema/migration changes, authentication/permission policy edits, or major dependency upgrades.

---

## Refactor Plan (Phases 0–6)

- **Phase 0 (Understand & Audit)**: Complete architecture/feature audit, severity breakdown, and Phase 5 gap checklist (`docs/refactor/AUDIT.md`).
- **Phase 1 (Tooling & Standards)**: Fix `.husky/pre-commit`, `.lintstagedrc.json`, `.gitignore`, `.dockerignore`; pin runtimes (`.bun-version`, `rust-toolchain.toml`); configure Prettier + root workspace scripts; eliminate `any` in `webauthn.ts` and unnecessary `eslint-disable` comments in `TeamCalendar.tsx`.
- **Phase 2 (Structure & Naming)**: Move remaining non-test SQL out of `main.rs`, `services/auth_service.rs`, and `services/statutory_rules.rs` into `repositories/` and refresh `backend/.sqlx/`; centralize frontend route paths in `frontend/src/lib/routes.ts`.
- **Phase 3 (Code Quality)**: Add characterization tests for `Approvals`, `Reports`, and `EmployeeList`, then split oversized frontend pages (`Approvals.tsx`, `AttendancePage.tsx`, `EmployeeList.tsx`, `Reports.tsx`) into cohesive sub-modules and extract welcome-email dispatch from `handlers/employee.rs` into `services/employee_service.rs`.
- **Phase 4 (Security, Privacy & Performance)**: Reject `"replace-with"` `.env.example` placeholder secrets in `AppConfig::validate_production_secrets`, fix multi-byte single-char email masking in `core::redact::email`, redact recipient emails in `services/email_service.rs` logs, and document remaining env vars in `.env.example`.
- **Phase 5 (Fill the Gaps)**: Add loading/error/retry states in `TeamCalendar.tsx`, add global `OfflineBanner`, add keyboard skip-to-content links and >=44px mobile header buttons in layouts, complete `<head>` metadata in `frontend/index.html`, and cover all additions with unit/component + i18n parity tests.
- **Phase 6 (Final Check)**: Sync `codegraph`, update `README.md` / `CLAUDE.md` / `AGENTS.md` / `docs/refactor/PROGRESS.md` final summary, run full clean install + format + lint + SQLx cache + typecheck + test + build suite, and leave branch `refactor/standards` clean and unpushed for review.
