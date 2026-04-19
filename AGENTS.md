# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Commands

All commands run from the repo root unless noted.

### Local services
```bash
docker compose up -d        # Postgres 16 (:5432), Redis 7 (:6379), pgAdmin (:5050)
```
Requires `POSTGRES_PASSWORD` and `PGADMIN_PASSWORD` in the environment (no defaults).

### Backend (Rust / Axum, edition 2024)
```bash
cd backend
cp .env.example .env        # first time only
cargo run                   # starts API on :8080, auto-runs sqlx migrations
cargo fmt --check           # CI enforces this
cargo clippy -- -D warnings # CI enforces -D warnings
cargo test                  # integration tests require DATABASE_URL + JWT_SECRET
cargo test <name>           # run a single test by substring    
```
Migrations live in `backend/migrations/` and are embedded via `sqlx::migrate!` — they run on every `cargo run`. Add schema changes as new numbered files (`NNN_description.sql`); do not edit existing migrations.

### Frontend (React 19 + Vite 8 + TS 5.9)
```bash
cd frontend
npm install
npm run dev                 # Vite on :5173, proxies /api → :8080
npm run build               # tsc -b && vite build (CI runs both)
npm run lint                # eslint (CI enforces)
npx tsc --noEmit            # type check (CI enforces)
```
Tailwind CSS v4 is wired via `@tailwindcss/vite`. Path alias `@/*` → `src/*`.

## Architecture

### Request flow
Browser → Vite dev proxy (or CloudFront in prod) → Axum at `/api/*` → handler → service → sqlx → Postgres. All routes are defined in one place: `backend/src/routes/mod.rs`. Everything is nested under `/api`.

### Backend layering (strict, enforced by convention)
- `handlers/` — thin HTTP glue. Extract `AuthUser`, parse JSON, call a service, map to JSON response. Do not put business logic here.
- `services/` — business logic and orchestration (e.g. `payroll_engine`, `pcb_calculator`, `epf_service`, `eis_service`, `socso_service`, `attendance_service`). Services take `&PgPool` and return `AppResult<T>`.
- `models/` — data structs and sqlx queries. Naming is by domain (`employee.rs`, `payroll.rs`, `attendance.rs`, `user_company.rs`, etc.).
- `core/` — cross-cutting: `app_state` (shared `AppState { pool, config, webauthn }`), `auth` (JWT + `AuthUser` extractor), `config` (env loading), `db` (pool + migrate), `cookie`, `error` (`AppError` → HTTP via `IntoResponse`).
- `repositories/` exists but is currently empty — model files hold queries today.

Errors: every fallible path returns `AppResult<T>` (`Result<T, AppError>`). `AppError::Database` wraps `sqlx::Error` via `#[from]`, so use `?` freely. `AppError::Internal` is logged and returned as a generic 500; all other variants surface their message to the client.

Auth: JWT in `Authorization: Bearer`, refresh token in httpOnly cookie. `AuthUser` is an Axum extractor; use `auth_user.deny_exec()?` on any payroll-sensitive handler — the `exec` role is read-mostly and must not see payroll figures. Role strings in claims: `super_admin`, `admin`, `payroll_admin`, `hr_manager`, `finance`, `exec`, `employee`. Multi-company users switch active company via `PUT /api/auth/switch-company`, which re-issues the JWT with a new `company_id`.

Rate limiting is applied per-route group in `routes/mod.rs` via `tower_governor` — tighter limits on `/auth/login`, `/auth/forgot-password`, and OAuth2 endpoints.

Two background tasks spawn from `main.rs`:
1. Daily cleanup of stale `refresh_tokens` (>30 days old and expired/revoked).
2. Hourly tick that auto-marks absent employees at ~12:30 PM Asia/Kuala_Lumpur (04:30 UTC). This cron skips employees who have an approved `leave_requests` row covering that date, and skips public holidays.

### Attendance subsystem (`services/attendance_service.rs`)
Key design decisions to be aware of:
- **QR tokens are multi-use within their TTL (300 s).** The `used` flag on `attendance_qr_tokens` means *admin-revoked* (set when a new token is generated), not *employee-scanned*. Multiple employees can check in from the same displayed QR within the 5-minute window. Do not reintroduce single-use logic.
- **Check-out matches the most recent open record within 24 hours**, not by calendar date. This handles overnight / night-shift employees. The old same-day constraint has been removed.
- **`QrTokenResponse` carries `ttl_seconds`** — the frontend uses this for progress bar calculation. Do not hardcode 300 in the frontend.
- **Summary endpoint** `GET /api/attendance/summary?date_from=&date_to=` returns per-employee aggregates (`present_days`, `late_days`, `absent_days`, `half_days`, `total_hours`, `overtime_hours`). Uses a LEFT JOIN so employees with zero records still appear.
- **CSV export** `GET /api/attendance/export` streams a downloadable CSV with the active filter set.

### Payroll engine
`services/payroll_engine.rs` is the entry point. It enforces one active run per `(company, payroll_group, year, month)`, then loops employees and composes `epf_service` + `socso_service` + `eis_service` + `pcb_calculator` to produce `PayrollItem`s inside a transaction. PCB (monthly tax deduction) uses progressive rules driven by seed data in migration `007_seed_statutory_data.sql`. PDFs are produced by `payslip_pdf_service` / `pdf_helpers` (printpdf), and statutory exports (EPF/SOCSO/EIS/PCB files + EA form) by `statutory_export_service` / `ea_form_service`.

### Frontend layout
- `App.tsx` is the router. Two shells: `AppLayout` for admin/HR, `PortalLayout` for employee self-service. `RoleGuard` wraps routes that a role must not see (e.g. `exec` is blocked from `/payroll/*` and `/reports`). `/attendance/kiosk` and `/attendance/scan` are unauthenticated public routes used by the check-in kiosk.
- `api/client.ts` — single axios instance. Access token is kept in-memory only (never in `localStorage`); refresh uses the httpOnly cookie. A 401 on any non-auth endpoint triggers a single refresh attempt with a queue for concurrent requests, then redirects to `/login` if refresh fails. When adding API modules, always import from `@/api/client` — do not create a second axios instance.
- `context/AuthContext.tsx` — on mount, calls `/auth/refresh` to restore the session from the cookie; `user` is mirrored to `localStorage` for fast paint only, never for auth.
- `pages/` mirrors feature areas; `api/*.ts` has one file per backend module.
- React Query defaults: `retry: 1`, `staleTime: 30s`, no refetch-on-focus.

### Infra
`infra/` holds Terraform for AWS (RDS + EC2 + ECR + CloudFront + ACM + Route53 + S3 uploads). Prod backend is containerized via `backend/Dockerfile`; frontend build is served from S3/CloudFront.

## Conventions specific to this repo

- Money uses `rust_decimal::Decimal` end-to-end; never `f64`. Serde serializes decimals as strings (`serde-with-str`) — mirror that in TS types.
- Dates for attendance/scheduling are interpreted in `Asia/Kuala_Lumpur`; UTC is only used for storage and for the background-task scheduler.
- New schema changes: add a new file in `backend/migrations/` with the next sequence number. Update `frontend/src/types/` to match any API contract change (per CONTRIBUTING.md).
- Keep handlers thin; if you find yourself writing SQL or composing services in a handler, move it into a `service` module.
- Do not introduce a second HTTP client on the frontend — extend `api/client.ts` or add a new `api/<module>.ts` that uses it.
