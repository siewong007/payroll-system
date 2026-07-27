# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

All commands run from the repo root unless noted.

### Local services
```bash
cp .env.example .env        # first time only, from the repo root
docker compose up -d        # Postgres 19 Beta 2 on 127.0.0.1:5434
```
Requires `POSTGRES_PASSWORD` in the root `.env` (no default).

### Backend (Rust / Axum, edition 2024)
```bash
cd backend
cargo run                   # starts API on :8080, auto-runs sqlx migrations
cargo fmt --check           # CI enforces this
cargo clippy -- -D warnings # CI enforces -D warnings
cargo test                  # integration tests require DATABASE_URL + JWT_SECRET
cargo test <name>           # run a single test by substring    
```
Migrations live in `backend/migrations/` and are embedded via `sqlx::migrate!` — they run on every `cargo run`. Versions `1000_schema.sql` and `1001_data.sql` are the current two-file rebaseline and become immutable once deployed. Future production changes use a new numbered migration; returning to two files requires another explicit, tested rebaseline.

Data access uses the compile-time-checked `sqlx::query!`/`query_as!`/`query_scalar!` macros throughout and lives in `backend/src/repositories/` (see Backend layering below). These macros are verified against the committed `backend/.sqlx/` offline cache, so CI lint and the Docker build need no database (`SQLX_OFFLINE=true`). **After adding or changing a macro query, regenerate the cache** against a migrated DB and commit it:
```bash
DATABASE_URL=postgres://… cargo sqlx prepare   # writes backend/.sqlx/
```
Forgetting this makes the build fail with "no cached data for this query" — that's the guardrail, not a flake.

The canonical target is **PostgreSQL 19 Beta 2**. `1000_schema.sql` uses native `uuidv7()` and permits PostgreSQL 18 only for the documented AWS RDS 18.4 compatibility exception. Use the pinned `postgres:19beta2-alpine` image locally. A data volume created by an earlier major version cannot be opened directly by 19; use `pg_upgrade` or dump/restore for valuable data, or recreate a disposable local volume. See `docs/database.md`.

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
Browser → Vite dev proxy (or CloudFront in prod) → Axum at `/api/*` → handler → service → sqlx → Postgres. All routes are defined in one place: `backend/src/routes/mod.rs`. Everything is nested under `/api`.

### Backend layering (strict, enforced by convention)
- `handlers/` — thin HTTP glue. Extract `AuthUser`, parse JSON, call a service, map to JSON response. Do not put business logic here.
- `services/` — business logic and orchestration (e.g. `payroll_engine`, `pcb_calculator`, `epf_service`, `eis_service`, `socso_service`, `attendance_service`). Services compose repository calls, own transaction boundaries, and map absence to `NotFound`/`Conflict`. They take `&PgPool` and return `AppResult<T>`. No raw SQL in services.
- `models/` — data structs and bespoke read/DTO projections (no SQL). Naming is by domain (`employee.rs`, `payroll.rs`, `attendance.rs`, `user_company.rs`, etc.).
- `repositories/` — the data-access layer: one module per table (`employees.rs`, `payroll_runs.rs`, …) holding thin query functions, plus `reads/` for cross-table joins/aggregations grouped by use-case (`reads/payroll.rs`, `reads/reports.rs`, …). Request-time SQL lives here; handlers and services do not embed it. Functions are generic over `impl sqlx::Executor<'_, Database = Postgres>`, so a service passes `&pool` or composes several calls in one `&mut tx`. See `docs/architecture.md`.
- `core/` — cross-cutting: `app_state` (shared `AppState { pool, config, webauthn }`), `auth` (JWT + `AuthUser` extractor), `config` (env loading), `db` (pool + migrate), `cookie`, `error` (`AppError` → HTTP via `IntoResponse`).

Errors: every fallible path returns `AppResult<T>` (`Result<T, AppError>`). `AppError::Database` wraps `sqlx::Error` via `#[from]`, so use `?` freely. `AppError::Internal` is logged and returned as a generic 500; all other variants surface their message to the client.

Auth: JWT in `Authorization: Bearer`, refresh token in httpOnly cookie. `AuthUser` is an Axum extractor. The `exec` role is read-mostly and must not see payroll figures; that is enforced by the permission model rather than by an explicit call — `exec` is in no branch of `AuthUser::can()`, so `require_permission(Permission::ViewPayroll)` / `require_payroll_privileged()` / `is_payroll_privileged()` all deny it. `auth_user.deny_exec()?` exists for handlers that are not permission-gated. Every handler must carry an explicit gate: the operation itself needs a role check (`require_employee_manager`, `require_hr_admin`, `require_attendance_viewer`, …), not just a `company_id` lookup. Prefer an allow-list gate over the deny-list `require_non_employee` (which rejects only sole-role `employee`, so any role added later silently gains access) — `require_attendance_viewer` is the allow-list model to copy. Role strings in claims: `super_admin`, `admin`, `payroll_admin`, `hr_manager`, `finance`, `exec`, `employee`. Multi-company users switch active company via `PUT /api/auth/switch-company`, which re-issues the JWT with a new `company_id`.

Rate limiting is applied per-route group in `routes/mod.rs` via `tower_governor` — tighter limits on `/auth/login`, `/auth/forgot-password`, and OAuth2 endpoints. All limiters use `SmartIpKeyExtractor` (X-Forwarded-For / X-Real-IP, falling back to the TCP peer): behind CloudFront the default peer-IP key collapsed every client into one shared bucket, so a fleet of kiosks rate-limited each other.

Two background tasks spawn from `main.rs`:
1. Daily cleanup of stale `refresh_tokens` (>30 days old and expired/revoked) and of expired `attendance_qr_tokens` with no referencing check-in.
2. Daily auto-absent run at 12:30 PM Asia/Kuala_Lumpur (04:30 UTC), scheduled by sleeping until the next occurrence computed by `core::schedule::next_daily_run_utc` (pure, unit-tested; the returned delay is strictly positive so the loop can never arm a zero-length sleep). It also runs once at startup. Each run is a catch-up over every due local date since the last bookmarked success, so downtime during the window is recovered rather than skipped. The run skips employees who have an approved `leave_requests` row covering that date, non-working days, and public holidays. Both background tasks log a heartbeat line on every tick so a silent log stream indicates wedged runtime timers, not an idle task.

### Attendance subsystem (`services/attendance_service.rs`)
Key design decisions to be aware of:
- **QR tokens are multi-use within their TTL (300 s).** The `used` flag on `attendance_qr_tokens` means *admin-revoked* (set when a new token is generated), not *employee-scanned*. Multiple employees can check in from the same displayed QR within the 5-minute window. Do not reintroduce single-use logic. Minting is scoped per display surface (`kiosk_credential_id`, NULL = admin console), so kiosks never retire each other's codes. Expired tokens with no referencing check-in are purged by the daily cleanup task.
- **Check-out matches the most recent open record within 24 hours**, not by calendar date. This handles overnight / night-shift employees. The old same-day constraint has been removed. When nothing matches but an older open session exists in the same company, check-out reports *that* ("more than 24 hours old — ask an administrator"), rather than telling the employee to check in — which used to bounce them into the one-open-session conflict and back.
- **Check-out never fails on geofence.** An off-site or GPS-less check-out is recorded and OR-ed into `is_outside_geofence` for admin review. Blocking it would strand the employee in an open session only an admin could close.
- **`QrTokenResponse` carries `ttl_seconds`** — the frontend uses this for progress bar calculation. Do not hardcode 300 in the frontend.
- **Face ID check-in is a real WebAuthn ceremony.** `POST /api/attendance/check-in/face-id/begin` issues a challenge bound to the user (challenge type `attendance_face_id`); the completing call verifies the assertion via `webauthn_rs` before inserting. A `face_id` record must never be creatable from a bare JWT.
- **Auto-absent marking is date-parameterized and self-healing.** `mark_absent(tz, date, company_id)` targets one local date and only covers employees inside their employment window (`date_joined`/`date_resigned`) — without that guard a backfill invents absences from before a hire. The background task runs a *catch-up* from the last successful run (bookmarked in `platform_settings.auto_absent_last_run_date`, capped at 14 days) so a deploy spanning the daily window no longer skips that day forever, and it marks each company on that company's own timezone so the placeholder matches what `delete_auto_absent_today` and the reads layer bucket by. Today only counts as due after 12:30 local. `POST /api/attendance/absent-run` lets an hr_admin re-run one past date (company-scoped, bounded to 90 days, never future); it is idempotent.
- **A check-in supersedes that day's auto-absent placeholder** — deleted in the same transaction as the insert, matching only cron-written rows (`created_by IS NULL`, method `manual`, the marker note). An HR-edited row is preserved.
- **Summary endpoint** `GET /api/attendance/summary?date_from=&date_to=` returns per-employee aggregates (`present_days`, `late_days`, `absent_days`, `half_days`, `total_hours`, `overtime_hours`, `unchecked_out_days`). Counts are per **distinct local day**, not per row — a split shift is one present day, and a day with both an absent placeholder and a late check-in counts once as late (precedence: late > half_day > present > absent). Employees with zero records still appear.
- **CSV export** `GET /api/attendance/export` returns a downloadable CSV with the active filter set. With no date range it defaults to the current local month rather than the tenant's entire history.
- **Corrections require a `reason`** (persisted to the audit trail, not the record) and use tri-state field semantics: a value sets, `clear_check_out` / `clear_notes` clear, absent means keep. The record update and its audit row are written in one transaction, company-scoped so they appear in `/api/audit-logs`.
- **Attendance date filters are sargable.** The reads layer compares the raw `check_in_at timestamptz` against local-midnight bounds instead of wrapping the column in `AT TIME ZONE`, and threads the company timezone through rather than hardcoding MYT. Keep new filters in that shape or the `(company_id, check_in_at)` indexes stop being usable.

### Attendance network binding (`services/attendance_network_service.rs`)
Ties a check-in to the company's network, and *learns* which network that is. Modes on `companies.attendance_network_mode`: `none` / `learn` (observe only) / `warn` (flag) / `enforce` (block).
- **The only trusted signal is the server-observed client address**, resolved by `core::client_ip`. Browsers expose no SSID/BSSID API, so any network identifier a web client sends is a string the employee typed or a header they set — the API therefore accepts **no inbound network field at all**. The human-readable "HQ WiFi" name is an admin-typed `label` on the approved row, travelling outbound only. Do not add a client-supplied SSID parameter.
- **What it proves, and does not.** An egress prefix proves network *path*, not physical presence: an SSH tunnel or VPN exit node on any office machine passes, and a full-tunnel corporate VPN would pass every remote worker. It is an HR deterrent against casual remote check-in plus evidence for exception triage — **not an authorization boundary**. This is stated in the `1016` migration header and shown in the admin UI against Enforce; keep it there.
- **Learning never promotes itself.** `attendance_network_observations` is evidence; `company_networks` is the allow-list, written only by a holder of `ManageAttendanceNetworks`. An observation counts toward a proposal only when *anchored* — corroborated by a kiosk-minted QR token or by `geofence_service::geofence_anchor`. Use `geofence_anchor`, never `!validate_geofence`: the latter returns "inside" for a company with zero configured locations, which would anchor every check-in from anywhere.
- **Anti-lockout rules.** Enforcement is inert while no network is approved; `set_mode("enforce")` refuses on an empty allow-list, and deleting/deactivating the last active network while enforcing is refused. Check-**out** never blocks (same rationale as the geofence). An address that cannot identify a place — absent, private, loopback, CGNAT — is flagged, never denied: it means the request missed the proxy, which affects every client at once. Denied check-ins are recorded in `denied_count` so the office's new address still surfaces after an ISP change, when nothing else is being recorded.
- **PDPA.** Observations are employees' home and mobile addresses. Retention is `OBSERVATION_RETENTION_DAYS` (purged by the daily cleanup task), and no endpoint exposes the employee → address mapping — the candidates read aggregates by network and returns no `employee_id`. Keep it that way.

### Payroll engine
`services/payroll_engine.rs` is the entry point. It enforces one active run per `(company, payroll_group, year, month)`, preflights source-linked verified statutory rule sets, then composes `epf_service` + `socso_service` + `eis_service` + `pcb_calculator` inside a transaction. The rows shipped in `1001_data.sql` are unverified academic fixtures; production payroll fails closed, and automatic PCB remains disabled until the calculator passes LHDN computerised-MTD conformance. PDFs are produced by `payslip_pdf_service` / `pdf_helpers` (printpdf), and statutory exports (EPF/SOCSO/EIS/PCB files + EA form) by `statutory_export_service` / `ea_form_service`.

Key structure to preserve when changing the engine:
- **Calculation is separate from persistence.** `compute_payslip` is pure over `BulkPayrollData` + `StatutoryTables`; `persist_payslip` writes. `process_payroll` computes *every* employee before writing any of them, so a failure reports all of them and commits nothing. Do not reintroduce a loop that writes as it computes — that is what made a bad batch surface one employee per re-run.
- **Statutory rules are read once per run.** `services/statutory_tables.rs` snapshots every applicable band, bracket and relief for the run's effective date, and the four calculators are pure functions over it (`calculate_*_with`). They previously took `&PgPool` and issued ~15 queries *per employee* while the write transaction was open. New statutory lookups belong in the snapshot, not in the per-employee path.
- **`POST /api/payroll/preview`** runs the identical calculation and writes nothing, returning per-employee projections plus `blocking`/`warnings` diagnostics. Anything that makes a run fail should surface here as a diagnostic rather than only as an error from `process_payroll`.
- **Every payslip stores its own breakdown.** `payroll_item_details` rows are written in the same transaction as the `payroll_items` row; earning lines sum to gross (claims excepted — they are reimbursements paid on top of net) and deduction lines sum to `total_deductions`. `is_statutory` marks only EPF/SOCSO/EIS/PCB.
- **Runs record their provenance.** `payroll_runs.calculation_snapshot` holds the verified rule sets and the overtime settings used. The rule tables and company settings are mutable and effective-dated, so a run recomputed later need not match what was paid.

### Frontend layout
- `App.tsx` is the router. Two shells: `AppLayout` for admin/HR, `PortalLayout` for employee self-service. `RoleGuard` wraps routes that a role must not see (e.g. `exec` is blocked from `/payroll/*` and `/reports`); because `roles` is an array, a route that omits `exec` denies anyone holding it even if another of their roles is listed. `/attendance/kiosk` (admin QR display) and `/attendance/scan` (employee scan target) sit outside the two shells but still require a session — the genuinely unauthenticated kiosk surface is `/kiosk/:kioskKey`, which authenticates with a kiosk secret instead of a user.
- `api/client.ts` — single axios instance. Access token is kept in-memory only (never in `localStorage`); refresh uses the httpOnly cookie. A 401 on any non-auth endpoint triggers a single refresh attempt with a queue for concurrent requests, then redirects to `/login` if refresh fails. When adding API modules, always import from `@/api/client` — do not create a second axios instance.
- `context/AuthContext.tsx` — on mount, calls `/auth/refresh` to restore the session from the cookie; `user` is mirrored to `localStorage` for fast paint only, never for auth.
- `pages/` mirrors feature areas; `api/*.ts` has one file per backend module.
- React Query defaults: `retry: 1`, `staleTime: 30s`, no refetch-on-focus.

### Infra
`infra/` holds Terraform for the frontend delivery path and deploy identity only: S3 + CloudFront + ACM + Route53 + an IAM OIDC provider/deploy role. There is no RDS, EC2, ECR, Secrets Manager or VPC module, and no S3 uploads bucket (uploads go to the API container's local `uploads/`). Production runs the backend and database as containers on the Lightsail host, built from the repository root with `infra/Dockerfile`; the frontend build is served from S3/CloudFront.

## Conventions specific to this repo

- Money uses `rust_decimal::Decimal` end-to-end; never `f64`. Serde serializes decimals as strings (`serde-with-str`) — mirror that in TS types.
- Dates for attendance/scheduling are interpreted in `Asia/Kuala_Lumpur`; UTC is only used for storage and for the background-task scheduler.
- Treat deployed migrations as immutable. Add a new numbered file for later schema/data changes; only an explicit rebaseline may squash history. Update `frontend/src/types/` to match any API contract change (per CONTRIBUTING.md).
- Keep handlers thin; if you find yourself composing services or business logic in a handler, move it into a `service` module. SQL belongs in a `repositories/` module (per-table) or `repositories/reads/` (joins/aggregations) — never in a handler or service.
- Do not introduce a second HTTP client on the frontend — extend `api/client.ts` or add a new `api/<module>.ts` that uses it.

## codegraph

This project has a codegraph **graph index** at `graphify-out/graph.json` — every symbol, call, import and reference across the Rust backend, the React frontend, the SQL migrations and the Terraform, plus community structure and god nodes. `graphify-out/` and the `graphify` / `graphify-mcp` binaries are the backing runtime's names; the tool itself is called codegraph.

It is registered as the `codegraph` MCP server in `.mcp.json` and exposes:
- `query_graph` — BFS/DFS traversal; the default entry point for "how does X work", "what touches Y"
- `get_node` / `get_neighbors` — full detail and direct edges for one symbol
- `shortest_path` — how two concepts connect
- `god_nodes` / `graph_stats` / `get_community` — architecture overview

**Reach for codegraph before grep/glob.** Rules:
- Start codebase questions with `query_graph`. It returns a scoped subgraph, far smaller than raw search output. Narrow with `--budget` / context filters rather than dumping the whole traversal.
- Fall back to Grep/Read when the graph does not resolve a symbol, or when you need exact current file contents in order to edit.
- Cite `src=` / `loc=` from graph results as `file:line`, and re-read the file before editing — the graph is a snapshot, not live.
- After changing code, re-sync: `./scripts/codegraph update .` (AST-only, no LLM, no API key, ~15s). Add `--force` when a refactor legitimately shrinks the graph, or the shrink-guard refuses the write.
- A dirty `graphify-out/` is expected and is never a reason to skip codegraph. The directory is gitignored.
- CLI equivalents when the MCP is not connected: `./scripts/codegraph query|path|explain|affected|god-nodes`.

Runtime deps (once per machine): `pip install "graphifyy[sql,terraform,mcp]"`. Without the `sql` and `terraform` extras the migrations and `infra/` are silently dropped from the index.

### Regenerating `graph.html`

The HTML viz is capped at 5000 nodes and this graph is past that, so the cap has to be raised or the build skips the file. `GRAPHIFY_VIZ_NODE_LIMIT` is set to `8000` in the Windows user environment; raise it again when the graph outgrows that.

**Only `update` produces full node-level HTML — `export html` does not.** `exporters/html.py` resolves the cap as `limit = node_limit if node_limit is not None else _viz_node_limit()`, and `export html` passes `node_limit` explicitly, so the env var is ignored on that path and the output silently degrades to an aggregated community meta-graph (a few hundred community nodes instead of every symbol). Both write the same `graphify-out/graph.html`, so the aggregated one overwrites the detailed one with no warning.

Use `./scripts/codegraph update .` to rebuild the viz. Reach for `export html` only when the aggregated community view is what you actually want. To tell them apart, count `"id":` in `graph.html`: it matches the node count for the detailed build, and the community count for the aggregated one.
