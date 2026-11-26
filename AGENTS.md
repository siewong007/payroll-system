# AGENTS.md

This file provides unified guidance to AI coding agents and contributors working in this repository.

## Commands

All commands run from the repo root unless noted.

### Local services

```bash
cp .env.example .env        # first time only, from the repo root
docker compose up -d        # Postgres 19 Beta 2 on 127.0.0.1:5434
```

Requires `POSTGRES_PASSWORD`, `JWT_SECRET`, and `TOTP_ENCRYPTION_KEY` (dedicated AES key for stored TOTP secrets; generate with `openssl rand -hex 32`, never reuse `JWT_SECRET`) in the root `.env`.

### Root workspace (monorepo scripts & hooks)

Runtime versions are pinned via `rust-toolchain.toml` (`stable` with `rustfmt` and `clippy`) and `.bun-version` (`1.3.14`), alongside `.editorconfig` and `.prettierrc.json`. Run `bun install` at the repo root to wire Husky + `lint-staged` pre-commit hooks, then run full-repo gates from the root:

```bash
bun run lint                # cargo clippy -- -D warnings, eslint, and i18n-scan --ci
bun run format              # cargo fmt + prettier across root/docs config & markdown
bun run format:check        # verify formatting without writing
bun run typecheck           # frontend tsc -b --noEmit
bun run test                # backend cargo test + frontend Vitest suite
bun run build               # backend cargo build + frontend production build
```

### Backend (Rust / Axum, edition 2024)

```bash
cd backend
cargo run                   # starts API on :8080, auto-runs sqlx migrations
cargo fmt --check           # CI enforces this
cargo clippy -- -D warnings # CI enforces -D warnings
cargo test                  # integration tests require DATABASE_URL + JWT_SECRET
cargo test <name>           # run a single test by substring
```

Migrations live in `backend/migrations/` and are embedded via `sqlx::migrate!` — they run on every `cargo run`. Versions `1000_schema.sql` and `1001_data.sql` form the consolidated baseline, followed by immutable numbered forward migrations (`1002_*.sql` through `1034_*.sql`, with intentional gaps at `1013` and `1028`). Every deployed migration is strictly immutable (enforced by CI); future production changes must add the next numbered migration file, and returning to two files requires an explicit, tested rebaseline.

Data access lives in `backend/src/repositories/` (including `reads/` projections); handlers and services must not embed request-time SQL. Compile-time-checked `sqlx::query!`/`query_as!`/`query_scalar!` macros are verified against the committed `backend/.sqlx/` offline cache, so CI lint and the Docker build need no database (`SQLX_OFFLINE=true`). **After adding or changing a macro query, regenerate the cache** against a migrated DB and commit it:

```bash
DATABASE_URL=postgres://… cargo sqlx prepare   # writes backend/.sqlx/
```

Forgetting this makes the build fail with "no cached data for this query" — that is the guardrail, not a flake.

The canonical target is **PostgreSQL 19 Beta 2**. `1000_schema.sql` uses native `uuidv7()` and permits PostgreSQL 18 only for the documented AWS RDS 18.4 compatibility exception. Use the pinned `postgres:19beta2-alpine` image locally. A data volume created by an earlier major version cannot be opened directly by 19; use `pg_upgrade` or dump/restore for valuable data, or recreate a disposable local volume. See [docs/database.md](docs/database.md).

### Frontend (React 19 + Vite 8 + TS 7)

```bash
cd frontend
bun install
bun run dev                 # Vite on :5173, proxies /api → :8080
bun run build               # tsc -b && vite build (CI runs both)
bun run lint                # eslint (CI enforces)
bun run test                # Vitest suite (CI enforces)
bun run typecheck           # type check (CI enforces)
```

Tailwind CSS v4 is wired via `@tailwindcss/vite`. Path alias `@/*` → `src/*`.

## Architecture

### Request flow

Browser / PWA → Vite dev proxy (or Cloudflare → host Caddy in prod) → Axum at `/api/*` → handler → service → repository → Postgres. All routes are defined in one place: `backend/src/routes/mod.rs`. Everything is nested under `/api`.

### Backend layering (strict, enforced by convention)

- `handlers/` — thin HTTP glue. Extract `AuthUser`, parse JSON, call a service, map to JSON response. Do not put business logic here.
- `services/` — business logic and orchestration (e.g. `payroll_engine`, `pcb_calculator`, `epf_service`, `eis_service`, `socso_service`, `attendance_service`, `push_service`). Services compose repository calls, own transaction boundaries, map absence to `NotFound`/`Conflict`, take `&PgPool`, and return `AppResult<T>`. No raw SQL in services.
- `models/` — data structs and SQLx read/DTO projections (no SQL). Naming is by domain (`employee.rs`, `payroll.rs`, `attendance.rs`, `notification.rs`, `user_company.rs`, etc.).
- `repositories/` — per-table SQL operations (`employees.rs`, `payroll_runs.rs`, `push_subscriptions.rs`, `push_outbox.rs`, …) plus `reads/` for cross-table joins/aggregations grouped by use-case. Functions are generic over `impl sqlx::Executor<'_, Database = Postgres>` so services can pass `&pool` or compose calls in `&mut tx`. Request-time SQL lives here; see [docs/architecture.md](docs/architecture.md).
- `core/` — cross-cutting: `app_state` (`AppState { pool, config, webauthn, mailer, push }`), `auth` (JWT + `AuthUser` extractor), `permission` (`Permission` enum & role/group matrix), `config` (env loading + `BUILD_REVISION`), `db` (pool + migrate), `cookie`, `turnstile`, `http_client`, `upload_path`, `redact`, `error` (`AppError` → HTTP via `IntoResponse`).

Errors: every fallible path returns `AppResult<T>` (`Result<T, AppError>`). `AppError::Database` wraps `sqlx::Error` via `#[from]`, so use `?` freely. `AppError::Internal` is logged and returned as a generic 500; all other variants surface their message to the client.

Auth & authorization: JWT in `Authorization: Bearer`, refresh token in `httpOnly` cookie (rotated atomically with reuse detection). `AuthUser` is an Axum extractor. Always enforce allow-list capability checks via `auth.require_permission(Permission::...)` (or `auth.can(...)`) rather than role deny-lists (`require_non_employee`). Canonical role strings (`core::permission::ALL_ROLES`, enforced by `users_roles_valid`): `super_admin`, `admin`, `manager`, `payroll_admin`, `hr_manager`, `finance`, `exec`, `employee`.

- `exec` is read-only and holds no payroll/report write permissions; `admin` and `manager` hold no payroll permissions; `manager` also excludes `import_employees` and platform-only permissions but keeps `view_audit_log` (payroll and `bank_*` audit keys are redacted for callers without `view_payroll`), and `user_service::hides_privileged_users` hides `admin`/`super_admin` accounts from callers holding neither role.
- Company-scoped `user_groups` combine additively via `filter_group_permissions_for_roles`, blocking platform-only permissions (`manage_users`, `manage_companies`, `manage_backups`, `manage_platform_settings`) for all groups, blocking payroll permissions for `exec`/`employee`/`manager`, and keeping `exec`-only users read-only.
- Multi-company users switch active company via `PUT /api/auth/switch-company`, which re-issues the JWT with a new `company_id`.

Audit trail: covers credential lifecycle (login success at `issue_session`, login failures, logout, password change/reset inside rotation transactions, TOTP enable/disable/reset) and business mutations. Locked-out 2FA users are recovered via `POST /api/admin/users/{id}/2fa/reset` (`super_admin`, password re-confirmed, self-reset refused, session-revoking, audited in-transaction).

Rate limiting & body bounds: applied per route group in `routes/mod.rs` via `tower_governor` with a 60-second state-pruning task — `ClientIpKeyExtractor` (gated by `TRUST_PROXY_HEADERS`) on unauthenticated endpoints and `SessionOrIpKeyExtractor` on expensive authenticated routes. Explicit `DefaultBodyLimit::max(...)` layers protect `/uploads`, `/admin/backup/import`, `/employees/import/validate`, and `/calendar/import-ics-file`.

Bot protection: Cloudflare Turnstile (`core::turnstile::verify`) is enforced at the top of `/auth/login`, `/auth/forgot-password`, `/auth/reset-password`, `/auth/passkey/check`, `/auth/passkey/{authenticate,discoverable}/begin`, and `/auth/oauth2/google/authorize`. Unset `TURNSTILE_SECRET_KEY` skips verification (intended dev/CI state); when set, failures are fail-closed (`400` missing, `403` rejected, `502` upstream unreachable).

Four background loops spawn from `main.rs` alongside the `background_jobs` async executor (`payroll_run` and `employee_import` enqueue a row and return `202` for `GET /api/jobs/{id}` to poll, with stale jobs marked failed at startup):

1. **Daily PDPA-aligned retention cleanup** (24h, advisory lock `7_492_026_101`) of stale `refresh_tokens` (>30d), unreferenced expired `attendance_qr_tokens` (>7d), aged `attendance_network_observations`, expired network dismissals, `audit_logs` (>2,555d / 7yr per ITA 1967 s.82), `notifications` (90d read / 365d unread), `email_logs` (>180d), `user_sessions` (30d revoked / 90d inactive), expired `bulk_import_sessions`, `password_reset_requests` (>30d), expired `passkey_challenges`, expired `oauth2_states`, finished `background_jobs` (>30d), unreferenced `uploaded_files` (>24h), stale `push_subscriptions` (30d revoked / 180d inactive), and old `push_outbox` rows (>30d).
2. **Daily catch-up auto-absent task** at 12:30 PM MYT (04:30 UTC via `core::schedule::next_daily_run_utc`, plus startup pass, advisory lock `7_492_026_102`), evaluated per company timezone from the last bookmarked run while skipping approved `leave_requests`, non-working days, and public holidays.
3. **Transactional email outbox poller** (15s tick, batch 25, `FOR UPDATE SKIP LOCKED`, exponential retry backoff) reusing `AppState.mailer`.
4. **Web Push outbox poller** (5s tick, batch 50, `FOR UPDATE SKIP LOCKED`, retry backoff `[30s, 120s, 600s]`, max 4 attempts, auto-revoking `404`/`410` subscriptions) using `AppState.push` (`SharedPushClient`, RFC 8291 AES-128-GCM + RFC 8292 VAPID ES256).

Each loop records health heartbeats surfaced on `/api/health/ready` and `/api/admin/platform/health`.

### Attendance subsystem (`services/attendance_service.rs`)

- **QR tokens are multi-use within their TTL (300 s).** `used` on `attendance_qr_tokens` means _admin-revoked_ (set when a new token is generated), not _employee-scanned_. Minting is scoped per display surface (`kiosk_credential_id`, `NULL` = admin console). Do not reintroduce single-use logic.
- **Check-out matches the most recent open record within 24 hours**, not by calendar date, supporting overnight shifts. When only an older open session exists, check-out instructs the employee to contact an administrator rather than prompting check-in.
- **Check-out never fails on geofence** — an off-site or GPS-less check-out is recorded and OR-ed into `is_outside_geofence` for review.
- **`QrTokenResponse` carries `ttl_seconds`** — never hardcode `300` in the frontend.
- **Face ID check-in is a real WebAuthn ceremony** (`POST /api/attendance/check-in/face-id/begin` → verified assertion before insert).
- **Auto-absent marking is date-parameterized and self-healing**, bounded by `date_joined`/`date_resigned` and bookmarked per company timezone (capped at 14 days; `POST /api/attendance/absent-run` bounded to 90 days). Check-in supersedes that day's cron-written placeholder in the same transaction (`created_by IS NULL`, method `manual`), preserving HR-edited rows.
- **Summary endpoint** (`GET /api/attendance/summary?date_from=&date_to=`) aggregates distinct local days (`late > half_day > present > absent`) with a `LEFT JOIN` so employees with zero records still appear. `GET /api/attendance/export` defaults to the current local month when no range is provided.
- **Corrections require a `reason`** (audited in-transaction) and use explicit clear flags (`clear_check_out` / `clear_notes`).
- **Attendance date filters must remain sargable** — compare raw `check_in_at timestamptz` against local-midnight bounds parameterized by company timezone rather than wrapping the column in `AT TIME ZONE`.

### Attendance network binding (`services/attendance_network_service.rs`)

- **Only server-observed client IP is trusted** (`core::client_ip`). Never add a client-supplied SSID/BSSID input.
- **Proof boundary**: egress prefix proves network path, not physical location (stated in migration `1016` and admin UI).
- **Learning never self-promotes**: `attendance_network_observations` requires corroboration via a kiosk-minted QR token or `geofence_service::geofence_anchor` (never `!validate_geofence`), and only `ManageAttendanceNetworks` approves `company_networks`.
- **Anti-lockout & PDPA**: enforcement stays inert with zero approved networks; `set_mode("enforce")` and deleting the last active network while enforcing are refused; check-out never blocks; non-public/unroutable IPs flag rather than block; `denied_count` tracks blocked attempts; no endpoint exposes `employee_id → IP` mappings.

### Payroll engine (`services/payroll_engine.rs`)

Enforces one active run per `(company, payroll_group, year, month)`, preflights source-linked verified statutory rule sets, and composes `epf_service` (KWSP Parts A–F & s.43(1) age-75 cessation) + `socso_service` + `eis_service` (RM6,000 ceiling) + `pcb_calculator` inside a transaction, surfacing `below_minimum_wage` warnings against the RM1,700/month floor (_Minimum Wages Order 2024_). Bundled `1001_data.sql` rows are unverified academic fixtures; production payroll fails closed, and automatic PCB stays disabled until LHDN computerised-MTD conformance passes. See [docs/payroll.md](docs/payroll.md) and [docs/legal.md](docs/legal.md).

Preserve these engine invariants:

- **Calculation is separate from persistence**: `compute_payslip` is pure over `BulkPayrollData` + `StatutoryTables`; `process_payroll` computes every employee before `persist_payslip` writes anything.
- **Statutory rules are read once per run** into `StatutoryTables` (`services/statutory_tables.rs`); calculators are pure (`calculate_*_with`) with zero per-employee queries.
- **`POST /api/payroll/preview`** shares the exact calculation path without writing, returning `blocking`/`warnings` diagnostics.
- **Explainability & provenance**: `payroll_item_details` stores itemized lines, `payroll_items.taxable_gross` (`1033`) tracks taxable pay for YTD/EA accuracy, and `payroll_runs.calculation_snapshot` captures the rule sets and overtime settings used.

### Frontend layout

- `App.tsx` is the router with route constants in `lib/routes.ts` (`ROUTES`). Two authenticated shells: `AppLayout` (admin/HR/ops, plus `/account` and `super_admin` `/admin/platform`) and `PortalLayout` (employee self-service). Public routes: `/login`, `/oauth2/*`, `/forgot-password`, `/reset-password`, `/change-password`, `/attendance/scan`, `/kiosk#<secret>` (fragment-scrubbed via `history.replaceState`; legacy `/kiosk/:kioskKey`), `/terms`, `/privacy`, `/compliance`.
- Capability-based `PermissionGuard` wraps routes using `user.permissions` (`role_permissions ∪ active user_groups`, with `exec` denied payroll/report/write permissions and `manager` denied payroll).
- `api/client.ts` — single Axios instance. Access token lives in memory only; refresh uses the `httpOnly` cookie with a single queued 401 refresh. Never create a second HTTP client.
- `context/AuthProvider.tsx` restores sessions via `/auth/refresh` on mount (`localStorage` mirrors display data only). `PwaProvider`, `manifest.webmanifest`, `sw.js`, `offline.html`, `OfflineBanner`, `AppInstallCard`, and `NotificationActivationCard` handle PWA installability and Web Push subscriptions.
- **i18n**: four first-class locales (`en`, `ms`, `zh-CN`, `zh-TW`) typed via `Messages = typeof en`, `serverErrorMap.ts`, `lib/format.ts` (`intlLocale()`), `src/tests/i18n.test.ts`, and `node scripts/i18n-scan.mjs --ci` (`// i18n-ok` for intentional literals). See [docs/i18n.md](docs/i18n.md).

### Infra

`infra/` holds only `infra/Dockerfile` (multi-stage build for the backend image). There is no Terraform (AWS resources were decommissioned on 2026-09-25). Production runs on the AIC VPS behind Cloudflare: host Caddy serves the SPA from `/opt/payrollmy-site` (reference config in `deploy/payrollmy-site.Caddyfile.example`) and proxies `/api` to the backend container; uploads live on `/opt/payroll/data/uploads`; `deploy/payroll-backup.sh` keeps 14 verified local nightly `pg_dump` archives; `deploy/payroll-watchdog.sh` monitors `/api/health/ready` and disk usage.

## Conventions specific to this repo

- Money uses integer `sen` (`i64`) and `rust_decimal::Decimal` end to end; never `f64`. Serde serializes decimals as strings (`serde-with-str`) — mirror that in TS types.
- Dates for attendance/scheduling are interpreted in the company's validated IANA timezone (`Asia/Kuala_Lumpur` default); UTC is used for storage and the background scheduler.
- Treat deployed migrations as immutable. Add a new numbered file for later schema/data changes; only an explicit rebaseline may squash history. Update `frontend/src/types/` to match any API contract change.
- Keep handlers thin; keep SQL strictly in `repositories/` or `repositories/reads/`.
- Do not introduce a second HTTP client on the frontend — extend `api/client.ts` or add `api/<module>.ts` using it.

## CodeGraph

This project maintains a CodeGraph knowledge graph at `graphify-out/graph.json` (`graphify-out/` and `graphify` / `graphify-mcp` are the backing runtime names; the tool is exposed as the `codegraph` MCP server in `.mcp.json` and `./scripts/codegraph`).

- **Start codebase exploration with `codegraph`**: use `codegraph_explore` (MCP) or `./scripts/codegraph query "<question>"` / `path` / `explain` / `node` / `callers` / `callees` / `impact` before broad grep/glob sweeps, then read exact target line ranges before editing.
- **Keep the graph current after code changes**: run `./scripts/codegraph update .` (AST-only, no API key; pass `--force` if a refactor legitimately shrinks the graph).
- **Dirty `graphify-out/` files** are expected and gitignored; never skip CodeGraph because of them unless debugging stale graph output.
- **HTML visualization note**: `graphify-out/graph.html` exceeds the 5,000-node default cap (`GRAPHIFY_VIZ_NODE_LIMIT` must be raised, e.g. `8000`). Use `./scripts/codegraph update .` to regenerate the full node-level HTML; `export html` produces an aggregated community meta-graph instead.
