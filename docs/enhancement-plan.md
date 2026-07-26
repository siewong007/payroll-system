# Enhancement plan

Produced by an eight-domain audit of the repository (payroll/statutory, attendance/scheduling,
auth/security, data layer, frontend, testing/CI, ops/observability, product completeness), with
every proposal re-checked against the code before it was allowed into this document. 94 proposals
were raised, 1 was rejected as a deliberate design decision, and 17 had their impact downgraded.

File references were accurate at the time of writing; treat them as starting points, not
guarantees.

## Where this stands

This is a mature, deliberately-designed codebase, not a prototype dressed up as one. The layering
rules in `CLAUDE.md` are actually enforced in the code — SQL really does live in `repositories/`,
money really is `Decimal`/sen end to end, the payroll engine really does compute every employee
before persisting any, and the statutory tables really are snapshotted once per run. CI is a genuine
gate (fmt, clippy `-D warnings`, `sqlx prepare --check`, migrations against a pinned PG19 Beta 2
service, lint/test/typecheck/build on the frontend), and `docs/features.md` is unusually honest: it
says "Conditional", "Unwired" and "Known limitation" where a marketing-minded catalogue would say
"done".

Eight domain audits found remarkably few architectural mistakes. What they found instead is a
consistent pattern of *last-mile gaps*: a column that exists and is never written, a setting that is
seeded and never read, an endpoint that is routed and never called, a calculator that is correct and
never reached.

The single biggest thing standing between this and real-world use is **survivability**. One Lightsail
disk holds the Postgres volume, the uploads bind mount, all five retained database dumps, and
`secrets.env` — the only copy of `POSTGRES_PASSWORD`, `JWT_SECRET`, the SMTP credentials and the
Google OAuth secret. There is no scheduled backup at all: `backup_existing_database()` in
`deploy/deploy.sh:236` runs only on deploy, so the RPO is "time since the last merge to main", and
every copy it produces would be destroyed by the same event it protects against. Losing that
instance loses the company's payroll history *and* the ability to reconstruct it.

Close behind is **verifiability**. Production payroll fails closed today
(`statutory_rules.rs:14-23`), which is the right call, but the gate is a hardcoded
`#[cfg(not(test))]` constant with no defined exit criterion and a `#[cfg(test)]` bypass that means no
test ever exercises the production path. Behind that gate sit real, demonstrable arithmetic defects —
unpaid leave never reduces the EPF/SOCSO/EIS wage base, a bonus is annualised as if it recurred
monthly, a 60-year-old citizen is charged full Part A EPF, and the payslip PDF's "TOTAL EARNINGS"
does not equal the sum of the lines printed above it whenever an operator uses the UI's own "Other
Earning" option. None of these can hurt anyone today because the gate blocks every production run;
all of them will fire the moment it opens.

---

## Tier 0 — Blocks real-world use

| # | Enhancement | Why it matters | Files/modules | Impact | Effort |
|---|---|---|---|---|---|
| 1 | Scheduled, encrypted, **off-host** database + uploads backups with weekly restore verification | The only pg_dump in the repo runs on deploy and writes to the same disk as the data it protects, so a host loss is unrecoverable and the RPO is "time since last merge". | `deploy/deploy.sh:236-264` (only caller), `deploy/docker-compose.prod.yml:102,124`, new `deploy/backup.sh` + timer installed by `install_release_files()` (`deploy.sh:150-162`), new bucket in `infra/` | Critical | M |
| 2 | Contain the filename on backup **import** | `restore_backup_files` joins an attacker-supplied JSON key onto `uploads/` with no containment, so a key of `/api/uploads/../../<path>` writes anywhere the container user can write — and it bypasses the extension allow-list, 10 MB cap and magic-byte check the real upload path enforces. | `backup_service/files.rs:41-60` (and the mirrored read at `:17-26`), reuse `upload_service::safe_upload_path`, `handlers/portal.rs:239-264` for the controls being bypassed | Critical | S |
| 3 | Escrow `secrets.env` off-host | Backups are useless without the credentials to restore into. **Note:** the JWT-rotation half of this originally cited TOTP lockout as motivation — that is wrong, and it must follow item 20, not precede it (see below). | `deploy/deploy.sh:25,120-138`, `backend/src/core/config.rs:33` | Medium | M |
| 4 | Make deploy rollback safe for schema-changing releases | `run_migrations` hard-asserts on unknown migration versions (`db.rs:41-49`), so the previous image panics on boot after any release that added a migration — rollback fails and production is left with no backend. | `deploy/deploy.sh:402-419`, `backend/src/core/db.rs:12-49`, `backend/migrations/` | High | M |
| 5 | Deduct unpaid leave from the **statutory wage base**, not only from net | An approved unpaid-leave request is staged as an ordinary deduction and applied after EPF/SOCSO/EIS/PCB are computed on full gross — the employer over-remits and the employee is over-deducted; the dedicated payslip columns are never written either. | `payroll_engine.rs:989,993-1036,1043-1051`, `approval_service/leave.rs:497-510`, `repositories/payroll_items.rs:50-67`, `payslip_pdf_service.rs:253` | Critical | M |
| 6 | Derive the EPF Third-Schedule part from age + residency | EPF is the one calculator that does not fail closed on employee attributes: a citizen aged 60+ passes SOCSO/EIS correctly but is still charged full Part A employee+employer EPF. | `payroll_engine.rs:882-884,993`, `epf_service.rs:34,46-49`, `statutory_tables.rs`, `migrations/1001_data.sql:64-121` | High | M |
| 7 | Route bonus/commission/arrears through the additional-remuneration PCB path | `monthly_gross` already contains the bonus and `pcb_calculator.rs:47` multiplies it by every remaining month, so an RM50k March bonus is taxed as if it recurred ten more times; `calculate_bonus_pcb` is unreachable because `is_bonus_month` is hardcoded `false`. | `payroll_engine.rs:989,1013-1034`, `pcb_calculator.rs:47,82,180-200`, `reads/payroll.rs:104-125` | High | M |
| 8 | PCB conformance harness + relief-mapping fixes + data-driven `calculator_status` gate | The production gate has no exit criterion, and the calculator it guards reads `life_insurance` where it documents an EPF cap, double-counts SOCSO/EIS relief as RM700, ignores six seeded relief types, and is pinned by a single vector. | `statutory_rules.rs:14-30`, `pcb_calculator.rs:98-141,166,216-226`, `migrations/1001_data.sql:320-329`, `tests/statutory_tests.rs:351-371` | High | L |
| 9 | Render the payslip PDF and EA form from `payroll_item_details` | `PayrollProcess.tsx:361-365` writes `item_type: 'manual_adjustment'` for "Other Earning", which lands inside gross and on no printed line — so an ordinary operator produces a payslip whose earnings do not sum to TOTAL EARNINGS. | `payslip_pdf_service.rs:166-201`, `ea_form_service.rs:309-348`, `reads/payslip.rs:21-40`, `payroll_entry_service.rs:34-36` | High | M |
| 10 | Update the stored PCB detail line when a manual PCB adjustment is applied | Manual PCB entry is the *documented workaround* for the disabled production calculator, and it silently desyncs the stored breakdown from `total_deductions` on every use. | `payroll_service.rs:224-238`, `repositories/payroll_item_details.rs` (no update path), `payroll_engine.rs:1282` | Medium | S |
| 11 | Implement run cancellation/reversal | `paid` is terminal: no cancel, no reverse, no re-run — a company that finds an error after marking a run paid has no supported recovery, though the schema was clearly designed for one (`payroll_status` includes `cancelled`, and the period index exempts it). | `payroll_lifecycle_service.rs:42-171`, `payroll_service.rs:117-124`, `repositories/payroll_runs.rs:22`, `migrations/1000_schema.sql:96,2107` | High | M |
| 12 | Fix attendance-derived overtime: unpaid breaks, day-type multipliers, approval gate | OT is `elapsed_time − company_default_shift` rated at `multiplier_normal`: a lunch break is paid as overtime, and rest-day/public-holiday work is paid 1.5× where the same engine pays 2×/3× for an approved application. **Descoped:** the per-employee-schedule half depends on item 54 and cannot ship before it. | `repositories/attendance_records.rs:93-106,302-311`, `payroll_engine.rs:943-982`, `migrations/1000_schema.sql:334-346,480-491` | High | L |
| 13 | Expand recurring holidays in the calendar reads | The auto-absent cron correctly honours `is_recurring` but `holidays::list_for_year` does not, so leave validation charges an employee a day the cron agrees they were never expected to work — and the admin can create exactly that row from `CalendarPage.tsx:523`. | `repositories/holidays.rs:17-20`, `calendar_service.rs:121-124,157,206`, `leave_rules.rs:76-110`, `attendance_records.rs:406-417` | High | M |
| 14 | Make bulk import provision portal accounts and leave balances | The realistic first act of a new tenant is importing the whole headcount — after which nobody can log in and nobody has an entitlement, and `user_service::create_user` explicitly refuses to mint `employee` accounts, so there is no retrofit path at all. | `employee_import_service/confirm.rs:160-217`, `employee_service.rs:56-83,110-158`, `portal_service.rs:527-547`, `user_service.rs:210-217` | High | M |
| 15 | ~~Authorize `GET /api/uploads/{filename}`~~ — **done, uncommitted** | Was the only non-health route with no `AuthUser` extractor at all. Now authorized by reverse lookup against the referencing record, 404-never-403, with the frontend migrated to a token-carrying blob fetch. Needs committing and a CI run. | `handlers/portal.rs:445`, `services/upload_service.rs`, `repositories/reads/uploads.rs`, `models/upload.rs`, `migrations/1009_upload_reference_lookup.sql`, `frontend/src/api/uploads.ts`, `components/ui/AttachmentPreview.tsx` | High | — |
| 16 | Add permission gates to the four config reads that only check `company_id` | Directly violates the rule `CLAUDE.md` states — every authenticated employee can read every geofence site's exact coordinates and radius, whether enforcement is on, and the full working-hours config. | `handlers/geofence.rs:16-24,86-94`, `handlers/work_schedule.rs:16-35`, `core/permission.rs`, `tests/route_auth_tests.rs` | Medium | S |
| 17 | SSRF-harden the ICS URL import | A bare `reqwest::Client::new()` with no scheme allow-list, IP filtering, redirect cap, timeout or size limit, whose error text is returned to the caller — turning an admin endpoint into an internal port scanner on the host that also runs Postgres. | `calendar_service.rs:237-256`, `handlers/calendar.rs:136-145`, `routes/mod.rs:447` | High | M |
| 18 | Record authentication and credential-lifecycle events in the audit trail | 63 audit call sites across 18 services and **zero** cover login, failure, logout, password change, 2FA or passkey changes — the audit view cannot answer the first question of any incident. | `auth_service.rs:54-78,192`, `totp_service.rs:128,171,188`, `password_reset_service.rs:52`, `handlers/auth.rs:146-203` | High | M |
| 19 | Write audit rows for company backup export and import | The single most damaging action a compromised super_admin can take — a full tenant dump including bank accounts, ICs and TINs — leaves no trace; a 500 MB NetworkOut alarm is currently the only detection signal. | `handlers/backup.rs:16-24,31-70`, `backup_service/export.rs:41-71`, `deploy/lightsail-alarms.sh:105-107` | High | S |
| 20 | Give TOTP its own key + a super-admin 2FA break-glass reset | `crypto.rs:13-18` derives the AES-GCM key as `SHA256("totp-secret-encryption-key:" ‖ jwt_secret)`, and `decrypt_secret` runs *before* the backup-code loop — so rotating `JWT_SECRET` after a leak permanently locks out every 2FA user, with recovery only via direct SQL. **Must precede item 3.** | `core/crypto.rs:13-29`, `totp_service.rs:92,118,157,164-169`, `handlers/totp.rs:45-52`, `core/config.rs:33` | Medium | S |
| 21 | Route error boundary with chunk-load recovery | Every route is `React.lazy` under one bare `<Suspense>` with no boundary anywhere, and each deploy runs `s3 sync --delete` on the hashed chunks — so every user with a tab open across a deploy gets a blank white page on their next navigation. | `frontend/src/App.tsx:13-18,118-121`, `.github/workflows/deploy.yml:120`, new `components/RouteErrorBoundary.tsx` | High | S |
| 22 | Replace bulk employee fetches with the existing searchable `EmployeePicker` | The backend clamps `per_page` to 100 and callers ask for 200–500 and discard `total`, so above 100 active employees an operator simply cannot add a manual payroll entry for employee #101 — and `Approvals.tsx` silently pre-selects `employees[0]`. | `handlers/employee.rs:113`, `PayrollProcess.tsx:74`, `Approvals.tsx:232,1092,1350,1619`, `TeamsPage.tsx:51`, `DocumentList.tsx:39,328`, `LettersPage.tsx:102`, pattern at `AttendancePage.tsx:91-110` | High | M |

---

## Tier 1 — Hardening and scale

| # | Enhancement | Why it matters | Files/modules | Impact | Effort |
|---|---|---|---|---|---|
| 23 | Bound every unit of work: HTTP `TimeoutLayer`, pool `acquire`/`idle`/`max_lifetime`, Postgres `statement_timeout`/`lock_timeout`/`idle_in_transaction`, task cancellation tokens | Nothing bounds anything today, and the payroll write transaction holds the `payroll_runs_one_active_period` index for its whole persist loop on a 0.30-vCPU Postgres with 10 connections. | `backend/Cargo.toml:11`, `core/db.rs:4-10`, `main.rs:109-122,129,177,218-227`, `deploy/docker-compose.prod.yml:27-38` | Medium | S |
| 24 | Correlation IDs + structured JSON request logging | `TraceLayer` is unconfigured so its events emit at DEBUG under `RUST_LOG=info` — the API produces *no* per-request telemetry, and `deploy.sh:292-296` says so itself. | `main.rs:53-55,118`, `backend/Cargo.toml:11`, `core/error.rs:56-78`, `deploy/docker-compose.prod.yml:99` | High | S |
| 25 | Background-job health surface + host-side restart/OOM/disk watcher | Both daily tasks report only to stdout and nothing reads it; `lightsail-alarms.sh:127-131` names a missing gap-filler script that does not exist in the repo. | `main.rs:128-163,176-214`, `handlers/health.rs`, `repositories/platform_settings.rs`, `deploy/lightsail-alarms.sh` | Medium | M |
| 26 | Retention purges for the five append-only tables | Nothing ever deletes from `audit_logs`, `notifications`, `email_logs`, `user_sessions` or `bulk_import_sessions` — the last stores whole validated CSVs in jsonb with a `DEFAULT now()+1h` expiry that nothing enforces. | `main.rs:128-163`, `repositories/bulk_import_sessions.rs`, `migrations/1003_user_sessions.sql`, `migrations/1008_audit_log_retention.sql` (misnamed — contains no retention) | Medium | S |
| 27 | Run the company backup export in one `REPEATABLE READ READ ONLY` transaction | 22 independent reads at READ COMMITTED mean a run committing mid-export produces orphan payslips, and the import side reinserts them into a single transaction without complaint. | `backup_service/export.rs:12-40`, `repositories/backup.rs` (already executor-generic), `backup_service/import.rs:217` | Medium | S |
| 28 | Give `tp3_records` a `company_id` and add it to `delete_cascade` | `DELETE /api/admin/companies/{id}` raises an FK violation for any tenant that ever entered a TP3 row — the backup teardown path already handles the table the company-delete path forgot. | `repositories/companies.rs:215-235`, `repositories/backup.rs:541`, `migrations/1000_schema.sql:1075-1087,2761` | Medium | S |
| 29 | Honour the company timezone end to end + validate the tz string server-side | Server paths and the **admin** attendance UI hardcode UTC+8 while an admin can pick from 14 zones, so a non-MYT tenant gets exported dates that contradict the filter that selected them, and boundary-day OT paid into the wrong month. (`portal/MyAttendance.tsx` already threads `timeZone` — scope the frontend half to `AttendancePage.tsx:75-81` only.) | `attendance_service.rs:36-38,950,1005-1059,1117-1151`, `reads/payroll.rs:198,207`, `work_schedule_service.rs:44`, `AttendancePage.tsx:75-81,188-197` | Medium | M |
| 30 | DB-free golden + property harness over `compute_payslip`, plus the missing engine money tests | `compute_payslip` is pure by design but untestable because `StatutoryTables` has private fields and only `load(pool)`; nothing tests a mid-month leaver, the YTD chain across two runs, overtime multipliers from settings, or the zakat/PTPTN/claims stack. | `statutory_tables.rs:32-47,141-170`, `payroll_engine.rs:867-872`, `backend/Cargo.toml` (no `[dev-dependencies]`), `tests/payroll_tests.rs` | High | M |
| 31 | Exhaustive route-inventory auth test | Authorization lives entirely in handler bodies and only ~20 of 175 registered routes are covered — which is exactly how items 15 and 16 reached `main`. | `tests/route_auth_tests.rs`, `routes/mod.rs:143-593` | Medium | M |
| 32 | Make DB-backed tests fail loudly when the database is absent, and load `.env` in the harness | `CONTRIBUTING.md` tells contributors to run bare `cargo test`; the harness never calls `dotenvy`, so ~130 integration tests silently skip and print `ok`. | `tests/support.rs:20-22,101-109`, `main.rs:50`, `CONTRIBUTING.md:80-87`, `.github/workflows/ci.yml:71-74` | Medium | S |
| 33 | Fix the no-op `typecheck` script and bring the 4,083 test lines under type checking | `tsc --noEmit` against a `files: []` root config checks zero files, and `tsconfig.app.json` excludes every test — so the CI gate cannot fail and the API-contract assertions are never checked. | `frontend/package.json:9,12`, `frontend/tsconfig.json`, `frontend/tsconfig.app.json`, new `tsconfig.test.json` | Medium | S |
| 34 | Forward-migration CI job on seeded data + byte-identical immutability gate on deployed migrations | Migrations are only ever applied to an empty database, and nothing enforces the immutability rule both `CLAUDE.md` and `CONTRIBUTING.md` state — an edited `1000_schema.sql` passes CI and panics the container at boot. | `.github/workflows/ci.yml:57-94`, `backend/migrations/`, `tests/support.rs:128-208` | Medium | M |
| 35 | Point deploy gates at `/api/health/ready` **and** add an authenticated read-only business smoke | All three gates check a closure returning the literal `"ok"`; nothing post-deploy exercises auth, route mounting, CORS or rate-limit config, so a release broken in those ways is declared healthy. | `routes/mod.rs:145-147`, `infra/Dockerfile:50-51`, `deploy/deploy.sh:228`, `.github/workflows/deploy-backend.yml:222-233` | Medium | S |
| 36 | Enforce 2FA for privileged roles | TOTP is entirely opt-in with no policy surface, no admin visibility and no enforcement anywhere in code or schema — including for `super_admin`, the only role that can export a whole tenant's bank accounts and ICs. | `auth_service.rs:113-134`, `handlers/totp.rs:37-43`, `user_service.rs:273`, `platform_settings` | High | M |
| 37 | Apply the separation-of-duties invariants to the *effective* permission set, not just roles | The three `permission.rs` invariants iterate `ALL_ROLES` only, so a group grant is the one way to hand a sole-role `exec` `ViewPayroll` — contradicting an invariant `CLAUDE.md` calls inviolable. | `core/auth.rs:277`, `user_group_service.rs:27-39`, `core/permission.rs:489,510,528` | Medium | M |
| 38 | Offload the seven inline bcrypt calls and make backup-code lookup O(1) | `hash_password` exists specifically to keep bcrypt off the runtime and seven other sites ignore it, including a loop of up to 10 sequential verifies on a 1–2 vCPU host. | `auth_service.rs:31-40,65,202,209`, `totp_service.rs:49,57,165`, `password_reset_service.rs:63`, `employee_service.rs:163`, `backup_service/import.rs:63` | Medium | S |
| 39 | RFC-4180 CSV helper for the four Reports exports | Fields are comma-joined with no escaping and only `employee_name` is ad-hoc quoted, so a department containing a comma shifts every subsequent column; add formula-injection neutralisation and a BOM while there. | `frontend/src/pages/reports/Reports.tsx:241-250,406-500`, new `frontend/src/lib/csv.ts` | Medium | S |
| 40 | Surface mutation errors on Approvals and make bulk actions partial-failure safe | Twelve mutations declare only `onSuccess`, and both bulk actions use `Promise.all`, so one rejection skips all three `invalidateQueries` and leaves already-cancelled rows rendering as pending with no message. | `frontend/src/pages/approvals/Approvals.tsx:278-376,612-638` | Medium | S |
| 41 | Email: shared transport in `AppState`, retrying outbox, non-blocking sends | A fresh STARTTLS handshake per message (so lettre never pools), no retry on failure ever, and forgot-password awaits the whole handshake inline in the request — while `handlers/employee.rs:218-239` already shows the right pattern. | `email_service.rs:146-186,232-241`, `core/app_state.rs`, `repositories/email_logs.rs`, `handlers/auth.rs:221-235` | Medium | L |
| 42 | Add the missing money/date CHECK constraints on `employees` | `employee_service` performs no validation, so a negative `basic_salary`, `hourly_rate` or `num_children`, or `date_resigned < date_joined`, is accepted through the API and flows straight into gross, EPF/SOCSO/EIS and the PCB relief math. | `migrations/1000_schema.sql:499-556`, `employee_service.rs`, new numbered migration (`NOT VALID` then validate) | Low | S |
| 43 | Structural guard against duplicate auto-absent placeholders | `mark_absent` relies solely on `NOT EXISTS` with no unique index behind it, and the dashboard counts absent rows not distinct days — cheap insurance against a deploy that briefly overlaps containers. | `repositories/attendance_records.rs:361-441`, `reads/dashboard.rs:78` | Low | S |
| 44 | Lazy-load `html5-qrcode` | It makes the employee portal's mobile chunk 381,738 bytes — 6.4× the next-largest page — and it is downloaded even by Face ID tenants who never use it. The static import now lives in `components/attendance/CheckInCard.tsx:19` (via `QrScanSheet.tsx:2`), not in the page. | `frontend/src/components/attendance/CheckInCard.tsx:19`, `QrScanSheet.tsx:2`, `frontend/vite.config.ts` | Medium | S |
| 45 | GHA layer cache for the Docker build + digest-pin the two base images + `docker` Dependabot ecosystem | The 46k-line backend is compiled up to four times per merge with no layer cache, and the tested image and shipped image can be built from different floating base tags. | `.github/workflows/ci.yml:103-116`, `.github/workflows/deploy-backend.yml:92-102`, `infra/Dockerfile:4,30`, `.github/dependabot.yml` | Medium | S |
| 46 | Coverage on both stacks + golden files for the statutory export layouts and EA form | Nothing measures coverage, and `statutory_export_service.rs`, `ea_form_service.rs`, `payslip_pdf_service.rs`, `report_service.rs` and `backup_service/` have **zero** test references — those are the agency-facing artefacts where one wrong column is a rejected submission. | `.github/workflows/ci.yml`, `frontend/package.json`, the five untested services | Medium | M |
| 47 | Generate and drift-check an OpenAPI contract; generate the frontend types from it | No test pins a single JSON field name on any payroll, attendance or employee response, and `frontend/src/types/index.ts` is 1,004 hand-maintained lines — copy the `sqlx prepare --check` guardrail pattern the team already trusts. | `backend/src/models/`, `routes/mod.rs`, `.github/workflows/ci.yml:95-100`, `frontend/src/types/index.ts` | Medium | L |
| 48 | CloudFront access logging + a CSP reporting destination (or just enforce the policy) | The distribution has no `logging_config` at all, and the Report-Only CSP contains no `report-uri`/`report-to`, so the documented plan to "flip it to enforced once reports are clean" can never be satisfied. | `infra/s3_cloudfront.tf:59-63,86-104,107-190` | Medium | S |
| 49 | Give the two `rustsec`/`bun audit` ignores machine-enforced expiry | Both carry a hand-written "next: 2026-10" review date that nothing enforces, and both justifications depend on code facts (HS256-only JWTs, generate-only PDF) that a future change can silently invalidate. | `.github/workflows/ci.yml:134-143,192-203`, new `backend/deny.toml` | Low | S |

---

## Tier 2 — Product depth

| # | Enhancement | Why it matters | Files/modules | Impact | Effort |
|---|---|---|---|---|---|
| 50 | Bank salary-payment file export | The system computes net pay and files statutory returns but cannot pay anyone — a finance user retypes 40 transfers into Maybank2u, which is precisely where payroll errors are introduced. | new `services/bank_export_service.rs` beside `statutory_export_service.rs:28-151`, `reads/payslip.rs:23`, `routes/mod.rs:509-517`, `PayrollDetail.tsx`; warning already at `payroll_engine.rs:467-481` | High | M |
| 51 | Home dashboard for the orphaned summary endpoint | `dashboard_service::summary` is routed, permission-aware and returns headcount, last-run figures, YTD employer cost, department split and a full `needs_attention` block — and has zero frontend consumers, so an HR admin's landing page is a company profile form. | `routes/mod.rs:377`, `handlers/dashboard.rs`, `models/dashboard.rs:10-57`, new `frontend/src/api/dashboard.ts` + `pages/Dashboard.tsx`, `App.tsx:98-104`, `Sidebar.tsx:39-59` | High | M |
| 52 | Form E + CP8D, and CP22 / CP22A | The EA form does not discharge the employer's **annual** return: Form E with the CP8D employee schedule is separately mandatory, and CP22/CP22A (new-hire and cessation notifications) are the statutory pair to offboarding. Nothing in the repo mentions any of them. | new service beside `ea_form_service.rs`, `reads/ea_form.rs`, `handlers/report.rs`, `routes/mod.rs:502-520` | High | L |
| 53 | Fix year-end carry-forward, then give it a UI | `upsert_carried_forward` does `DO UPDATE SET ... entitled_days = $4`, wiping the replacement-leave days `upsert_entitled_replacement` granted; it runs N×M statements on the pool with no transaction, no year validation and no audit — and is reachable only by curl. | `portal_service.rs:554-594`, `repositories/leave_balances.rs:165-185,235-250`, `handlers/employee.rs:361-384`, `routes/mod.rs:497-501` | Medium | M |
| 54 | Per-employee roster write path + rest days | `employee_work_schedules` has a tenant FK, a unique `(employee_id, day_of_week)` key and a lateness reader — and exactly one query in the whole backend, a SELECT. A Mon/Wed/Fri part-timer is auto-marked absent every Tuesday, and `features.md:45` overclaims. **Blocks the descoped half of item 12.** | `repositories/employee_work_schedules.rs` (24 lines), `attendance_service.rs:518-534`, `attendance_records.rs:382-402`, `routes/mod.rs:579-584`, `docs/features.md:45` | High | L |
| 55 | Leave-type CRUD + service-year entitlement bands | Leave types are seeded once and immutable from the product (the repository has no write path at all), and entitlement is a flat `default_days` — so the Employment Act's tenure-stepped annual and sick leave cannot be represented. | `repositories/leave_types.rs:10-64`, `portal_service.rs:501-524`, `migrations/1000_schema.sql:650-663,3146-3157` | High | M |
| 56 | Make `is_taxable` and per-domain exemptions affect the wage bases | HR can flag an allowance non-taxable, the flag is faithfully persisted onto the payslip line, and no calculator reads it — start with PCB taxable income plus a per-domain exemption column, since one boolean cannot drive EPF/SOCSO/EIS. | `reads/payroll.rs:26-30,54,81,104-120`, `payroll_engine.rs:989,1259` | Medium | M |
| 57 | Attendance policy config: derive `half_day`, add early-leave, expose thresholds | `half_day_hours` is a first-class editable column with no consumer, and there is no undertime concept at all, so an employee who leaves after two hours is recorded `present`. | `work_schedule_service.rs:43,111-113`, `company_work_schedules.rs:123-174`, `attendance_service.rs:507-535`, `reads/attendance.rs:224` | Medium | M |
| 58 | Payroll group CRUD | Every company gets exactly one group and can never make another, so monthly salaried staff and a different-cutoff outlet crew must share one cutoff and payment day — even though employees already carry `payroll_group_id` and the importer maps it. | `repositories/payroll_groups.rs` (list + count only), `routes/mod.rs:283`, `migrations/1000_schema.sql:820-833` | Medium | M |
| 59 | Compute and report the HRD Corp levy | `hrdf_number`, `hrdf_enabled`, `hrdf_contribution`, `statutory/hrdf_rate` and a Company Profile toggle all exist; `grep -rni hrdf backend/src/services/` returns nothing, so a user switches it on and the levy is never accrued. | `migrations/1000_schema.sql:269,279,548,3200-3201`, new `services/hrdf_service.rs`, `statutory_tables.rs`, `payroll_engine.rs:335` | Medium | M |
| 60 | Turn document-expiry tracking into a real alert | Indexed `expiry_date`, a routed `/documents/expiring`, a service, and a seeded `notifications/expiry_alert_days` — with zero consumers on either side; for an employer of foreign workers a lapsed permit is an immigration liability. | `repositories/documents.rs:281-306`, `routes/mod.rs:327`, `migrations/1000_schema.sql:3207`, new daily task in `main.rs` | Medium | M |
| 61 | Probation confirmation workflow + wire up the three inert settings | `probation_alert_days`, `email_payslip` and `auto_welcome_email` have no consumer anywhere, and `SettingsPage.tsx:19` omits the `email` category entirely so one of them cannot even be rendered — while the welcome mail sends unconditionally. | `migrations/1000_schema.sql:525-527,3206-3209`, `SettingsPage.tsx:19`, `handlers/employee.rs:194-197` | Medium | M |
| 62 | Approval-workflow notifications by email | `EmailNotificationChannel` is written, resolves the address and calls `send_email` — and has zero call sites; rejections send nothing at all and overtime sends nothing in either direction. | `notification_service.rs:7-49`, `portal_service.rs:114,254,400`, `approval_service/{leave,claim,overtime}.rs` | Medium | M |
| 63 | EA form: batch generation + employee self-service | Every employer must hand every employee an EA form each February; today that is 60 manual downloads by an administrator, and the portal has payslips but no EA route despite the data being the employee's own. Follows item 9. | `handlers/report.rs:233-256`, `ea_form_service.rs:12-106`, `routes/mod.rs:502-520`, `MyPayslips.tsx` | Medium | M |
| 64 | Record kiosk and geofence site on the attendance record | Migration 1004 exists specifically for per-kiosk token scoping, yet the record keeps no trace of which kiosk or which site — a multi-site employer cannot answer "which branch did this person report to". | `migrations/1004_qr_token_kiosk_scope.sql`, `migrations/1000_schema.sql:160-185`, `attendance_service.rs:566-587`, `geofence_service.rs:241-344` | Medium | M |
| 65 | Offboarding and final settlement | Resignation is a date field: proration works, but there is no leave encashment, no notice-in-lieu, no final-settlement view and no exit EA form — the realistic path is hand-keyed manual entries. Pairs with CP22A in item 52. | `employee_service.rs:264-309`, `approval_service/leave.rs:366-428` (daily rate to reuse), `payroll_entry_service.rs` | Medium | L |

---

## Tier 3 — Polish and DX

| # | Enhancement | Why it matters | Files/modules | Impact | Effort |
|---|---|---|---|---|---|
| 66 | Single-source `AGENTS.md` against `CLAUDE.md` and fix the stale `docs/features.md` rows | `AGENTS.md` tells an agent to use the deny-list `deny_exec()` where `CLAUDE.md` mandates allow-list gates, describes RDS/EC2/ECR that do not exist, and calls the auto-absent job hourly; `features.md:25,104` describe a routed page as unrouted and a deleted page as existing. | `AGENTS.md:27,61,67,81,88`, `CLAUDE.md:94`, `docs/features.md:25,31,93,104` | Medium | S |
| 67 | Wire `htmlFor`/`id` on the 68 unlabelled form controls | Those inputs and selects have no programmatic accessible name at all; `FormField` exists precisely to guarantee this and is imported by one page. | `components/ui/FormField.tsx:17-79`, `Approvals.tsx:1201-1204`, `Login.tsx:231-247`, `CompanyManagement.tsx`, `PayrollProcess.tsx`, `portal/{Leave,Claims,Overtime}.tsx` | Medium | S |
| 68 | Keyboard-operable `DataTable` rows + migrate the 13 hand-rolled dialogs to `Modal` + a `ConfirmDialog` replacing 33 `window.confirm` / 3 `alert` | Row click is the only way to open any record detail and it is invisible to assistive tech; only 2 of ~15 dialogs carry `role="dialog"`, and destructive payroll confirmations live in untranslatable OS chrome. | `components/ui/DataTable.tsx:203-206,305-307`, `components/ui/Modal.tsx`, `AttendancePage.tsx:242,912,1024`, `CompanyManagement.tsx`, `LettersPage.tsx` | Medium | L |
| 69 | Teach `getErrorMessage` to read `rateLimitMessage` | The interceptor builds a friendly 429 message specifically so users don't see "Request failed with status code 429" — and exactly one of ~27 error sites reads it. | `frontend/src/lib/utils.ts:63-70`, `frontend/src/api/client.ts:75-104`, `AttendancePage.tsx:58-60` | Low | S |
| 70 | Expose the running build revision from `/api/health/ready` and assert it post-deploy | Nothing in the binary knows its own version, so "did the deploy take effect / which build is serving after a rollback" needs SSH — and `deploy-backend.yml` already has `$DEPLOY_SHA` in hand. | `infra/Dockerfile`, `handlers/health.rs:18-25`, `main.rs:216`, `.github/workflows/deploy-backend.yml:96,230` | Low | S |
| 71 | Keep overtime hours in `Decimal` end to end | The only place the money path violates the repo's own never-`f64` rule: exact `numeric(5,2)` is cast to `::FLOAT` and back via `unwrap_or_default()`. | `reads/payroll.rs:196,229`, `models/payroll.rs:394-395,513-524`, `payroll_engine.rs:944,964` | Low | S |
| 72 | Query-shape cleanups: push the employee/department filter into the attendance summary's derived table; default the admin attendance list to the current local month; batch the carry-forward loop | Three independent small wins in the same idiom the codebase already documents; the list default also makes the screen consistent with the export path, which already defaults. | `reads/attendance.rs:99-120,213-256`, `attendance_service.rs:731-739,1120-1132`, `portal_service.rs:554-593` | Low | S |
| 73 | Batch the payroll persist loop and prefetch bulk payslip data | 3–4 round trips per employee inside the open write transaction, two of which are run-wide; and `company_for_employee` re-resolves the same company row once per payslip. | `payroll_engine.rs:704-711,1133-1200`, `payslip_pdf_service.rs:448-450`, `reads/payslip.rs:82-117`, idiom at `payroll_item_details.rs:30-45` | Low | M |
| 74 | Grow the test seed factories to cover attendance records, runs/items, leave and claims | Four factories exist; everything else is hand-written runtime `sqlx::query` INSERTs duplicated across four test modules, which break at runtime rather than build time on a schema change. | `tests/support.rs:128-208`, `tests/{attendance,schema_invariant,payroll,approval_flow}_tests.rs` | Low | M |

---

## Sequenced roadmap

### Phase 1 — Survive and secure
**Items 1, 2, 3, 4, 5, 10, 15, 16, 17, 18, 19, 20, 21.**
**Goal:** make data loss recoverable, close the open security holes, and fix the one money bug that is live today.

Backups come first because every other item is worth less than the data it operates on, and because
secrets escrow (3) is what makes a restore actually possible — a dump you cannot decrypt into a
database you cannot authenticate to is not a backup. Item 4 belongs here too: the rollback path is
the recovery mechanism for a bad release, and it is currently guaranteed to fail for any release
carrying a migration, which describes most of them.

The security cluster (2, 15–20) is deliberately grouped: every item is independent, S or M, and
touches a different file, so they parallelise cleanly. Two ordering constraints inside it — **20
before 3** (a `JWT_SECRET_PREVIOUS` for signature validation does nothing for TOTP ciphertext at
rest, so the TOTP key must be separated first), and **2 is the highest-severity single item in the
phase** (arbitrary file write, and the containment helper it needs already exists in
`upload_service`).

Item 5 is the one payroll defect that affects money *today*, since unpaid leave is staged through an
already-working approval flow. Item 21 is an afternoon's work that stops every deploy from blanking
every open tab, and item 10 is an hour that stops the documented manual-PCB workaround from
corrupting its own audit trail.

### Phase 2 — Make the payroll number defensible
**Items 6, 7, 8, 9, 11, 12, 13, 14, 22, 30.**
**Goal:** earn the right to open the production statutory gate.

Order inside this phase is dependency-driven. **Item 30 lands first** — the DB-free golden and
property harness is the instrument every other item is measured with; without it, changing relief
mapping means changing a number nobody can check. **Item 7 must precede item 8**: the harness cannot
validate additional-remuneration vectors while `is_bonus_month` is hardcoded `false` and
`calculate_bonus_pcb` is unreachable. Item 8's three parts ship together or not at all — deleting the
`#[cfg(test)]` bypass breaks every PCB test until the data-driven `calculator_status` replaces it.
**Item 9 is a prerequisite for item 63**, or that work batch-produces the same wrong totals sixty
times.

Items 12 and 13 are the attendance-side inputs to the same payroll number and are independent of the
statutory chain, so they can run in parallel — but item 12 ships *descoped* (breaks, day-type
multipliers, approval gate) because its per-employee-schedule half cannot be built until item 54
provides a write path for `employee_work_schedules`. Pull item 54 forward into this phase if the full
OT fix is wanted in one pass.

Items 14 and 22 are the onboarding pair: import must provision accounts before a real tenant's first
day, and the picker cap must lift before any tenant above 100 staff can stage a manual entry. Item 11
closes the loop — once payroll figures are trustworthy enough to run, an operator needs a way back
out.

### Phase 3 — Operate it
**Items 23–29, 31–49.**
**Goal:** be able to see, bound and prove what production is doing.

This phase deliberately follows the correctness work because observability of wrong numbers is not
progress. Within it, telemetry (24) and timeouts (23) come first and unblock everything else: you
cannot tune a `statement_timeout` you cannot observe, and item 46's coverage work is more useful once
item 32 makes a skipped suite impossible. Items 31, 34 and 35 are the three guardrails that retire
recurring *classes* of risk rather than single bugs — route-gate drift, migration drift, and
"healthy" releases that are broken — and each pays for itself the first time it fires. Item 47
(OpenAPI) sits late because it is the largest item here and because the response DTOs it annotates
are still being changed by Phase 2.

### Phase 4 — Compete for a real SME
**Items 50–65, then 66–74.**
**Goal:** close the capability gaps that decide a purchase, then clean up.

Item 50 (bank payment file) leads because "computes payroll, cannot pay it" is the sharpest gap in
the product, and item 51 (dashboard) is the highest capability-per-effort item anywhere in this plan
— the endpoint, the permission split and the deep-link targets already exist. Item 52 (Form E/CP8D,
CP22/CP22A) is the largest genuine *compliance* hole for a product sold on Malaysian statutory
coverage. Item 53 must land before any UI is wired to year-end carry-forward: exposing a destructive,
untransacted, unaudited operation to a button is strictly worse than leaving it unreachable. Item 54
unblocks item 57 and the descoped half of item 12. Item 63 follows item 9; item 65 pairs with CP22A
in item 52.

The Tier 3 items are genuinely last — except item 66, which should be done opportunistically the
moment any drifted claim is contradicted by earlier work, since a stale `AGENTS.md` actively
misdirects future contributors.

---

## Deliberately not doing

- **Roll production back to PostgreSQL 18.x.** The beta pin is an explicit decision with the risk
  written down (`docs/database.md:5-21` already quotes upstream's do-not-use-in-production advice).
  The *actual* mitigation for that risk is item 1, not a downgrade.
- **Advisory locks and per-company bookmarks for the background jobs.** There is one replica,
  `docs/architecture.md:54-55` states the single-instance constraint, and the per-date bookmark is
  written only after all companies succeed so no date is skipped. Take item 43 (the unique index) as
  cheap insurance and revisit when horizontal scaling is actually planned.
- **Column-level PII encryption at rest.** The only available key derivation lives in the same
  root-owned file on the same host as the Postgres container, so a host compromise yields key and
  ciphertext together. It defends only against a stolen volume snapshot — which item 1's encrypted
  off-host backups address more cheaply — while costing a decrypt on every read and making the
  export/import round trip key-bound. Revisit if a managed KMS ever enters the picture.
- **Bahasa Malaysia i18n.** XL effort, no demand signal anywhere in the repo. The one genuinely
  valuable piece — replacing 33 `window.confirm` and 3 `alert` calls with an accessible
  `ConfirmDialog` — is item 68 and stands on its own merits.
- **Migrating all 16 forms to react-hook-form + zod.** A large-diff refactor of untested
  money-handling flows dressed as an accessibility fix. The accessibility value is entirely in item
  67, which is mechanical and S. Drop `@hookform/resolvers`, `@tanstack/react-table` and `zod` from
  `package.json` instead, or leave them.
- **A Playwright/Cypress E2E layer.** Correct instinct, wrong first step: the surfaces that most need
  it (kiosk, scan) depend on camera and GPS, which a headless run stubs anyway. Add MSW to
  `frontend/src/tests/setup.ts` first; reconsider a browser layer only if MSW proves insufficient.
- **An idempotency key and offline replay queue for check-in.** `resolve_open_checkin_conflict`
  (`attendance_service.rs:702-719`) already returns the existing record on replay, and
  `AttendanceScanPage.tsx:70-73,175-193` already renders a dedicated "already recorded" card. A key
  buys almost nothing and adds a column, an index and a backdating risk.
- **Aligning the statutory export status window with the engine's YTD window.** The divergence is
  correct, not accidental: the engine's YTD chain must include `processed` runs, and you must not
  file contributions for money finance has not approved. Add a comment next to `reads/statutory.rs:45`
  and `reads/ea_form.rs:29` explaining the narrow window and close it.
- **Attendance trend/department/punctuality analytics.** The actionable slice already exists
  server-side in `dashboard_service`'s `NeedsAttention` block and is simply unrendered. Ship item 51
  first.
- **Transaction-rollback test isolation (`test_tx()`).** Every service entry point these tests
  exercise takes `&PgPool`, not an executor, so a rollback-on-drop transaction cannot be threaded in
  without changing service signatures across the codebase. The seeded-suffix scheme in
  `tests/support.rs:115-127` is a documented deliberate choice.
- **Raising `work_mem`/`mem_limit` on `payroll-db` as a standalone change.** The compose header
  documents this as a *shared* Lightsail host co-hosting two other applications. Take the free half
  (item 23's timeouts) unconditionally; treat the memory half as a capacity decision requiring
  headroom measurement first.

---

## Start here

**1. Item 2 — contain the backup-import filename.** Open
`backend/src/services/backup_service/files.rs:52`. `restore_backup_files` does
`url.strip_prefix("/api/uploads/")` and then `upload_dir.join(filename)` on a key that came from
uploaded JSON, so `/api/uploads/../../<path>` escapes the uploads directory. `safe_upload_path` in
`services/upload_service.rs` already implements exactly the containment needed — make it `pub` and
call it here, skipping any entry it rejects. In the same commit, apply the extension allow-list and
size cap from `handlers/portal.rs:239-264` so a restore cannot introduce a file the upload endpoint
would have refused, and mirror the guard into `collect_backup_files` at `:17-26`. This is the
smallest Tier 0 item and the highest severity per line changed.

**2. Item 1 — scheduled off-host backups.** Open `deploy/deploy.sh`. `install_release_files()`
(lines 150-162) already writes `/etc/logrotate.d/payroll`, so it is the established place for
host-unit installation. Add a `deploy/backup.sh` that runs
`docker exec payroll-db pg_dump --format=custom --no-owner --no-acl -U payroll payroll_db` (reuse the
exact invocation at `deploy.sh:245-247` so the format matches what the rollback path expects), pipes
it through `age -r <recipient>`, and uploads to a new versioned, lifecycle-managed S3 bucket; then
install a systemd timer for it from `install_release_files()`. First commit is the script plus the
timer plus the bucket in `infra/` — nightly DB only. Uploads tarball, WAL archiving and the weekly
restore-verification job are the second and third commits.

**3. Item 5 — unpaid leave out of the statutory wage base.** Open
`backend/src/services/payroll_engine.rs`. Today `variable_deductions` (read at line 914 from
`reads/payroll.rs:104-120`) folds the `unpaid_leave` entry in with every other deduction and only
reaches the payslip at lines 1043-1051, after `gross` (line 989) has already fed
`epf_service`/`socso_service`/`eis_service` (993-995) and `PcbInput.monthly_gross` (1014). Add an
explicit unpaid-leave input to `BulkPayrollData`, split it out of `variable_deductions` in
`reads/payroll.rs` (identifiable by `item_type = 'unpaid_leave'`, written at
`repositories/payroll_entries.rs:251`), and subtract it from the wage passed to the four calculators —
coordinating with the `is_prorated` branch at 893-908 so a mid-month joiner who also took unpaid leave
is not reduced twice. In the same commit, add `unpaid_leave_deduction` and `unpaid_leave_days` to the
insert column list at `repositories/payroll_items.rs:50-67` so `payslip_pdf_service.rs:253` stops
printing zero. Ship it behind a documented cutover date — recomputed historical runs will not match
already-filed contribution files.
