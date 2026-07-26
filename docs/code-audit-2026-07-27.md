# Code audit — 27 July 2026

Full defect audit of this repository: backend (Rust/Axum), frontend (React/TS), SQL migrations,
Terraform and deploy scripts. Roughly 71,000 lines in scope.

**As of `681b767`** (merge: authz rework, transactional integrity fixes and mobile attendance UI).

## Method

Three passes of parallel static analysis, each finding re-opened by an independent agent whose
default instruction was to *refute* it:

| Pass | Scope | Candidates | Confirmed | Refuted |
|---|---|---:|---:|---:|
| 1 | 15 domains: authz, tenant isolation, payroll math, statutory, attendance, sessions/crypto, concurrency, SQL, schema, frontend core, frontend pages, contract drift, file I/O, business logic, infra | 115 | 99 | 16 |
| 2 | 12 surfaces pass 1 under-covered: PCB/EPF/SOCSO/EIS math, statutory snapshot, YTD & EA numerics, WebAuthn/kiosk, backup data, email/letters, panic sweep, audit completeness, RoleGuard parity, test gaps, migration integrity | 103 | 66 | 37 |
| 3 | Re-check of the 47 findings whose files changed mid-audit | 47 | — | — |

**218 candidates → 165 confirmed → 127 unique defects** after merging duplicates reported by more
than one domain finder. 53 candidates were killed as factually wrong, already guarded upstream,
already constrained in the schema, unreachable by a real user, or documented in `CLAUDE.md` as
intentional.

## Status

| | Critical | High | Medium | Low | Total |
|---|---:|---:|---:|---:|---:|
| Catalogued | 11 | 26 | 57 | 33 | **127** |
| Still open | 10 | 25 | 54 | 31 | **120** |
| Fixed mid-audit | 1 | 1 | 3 | 2 | **7** |

### The tree moved while this ran

Eighteen commits landed on `main` during the audit, from a parallel session working the same
repository, and twelve files were modified in the working tree at the time of writing. Forty-seven
findings sat in files that changed after an agent had read them; each was re-opened against the
current tree. Seven are confirmed fixed and one partially fixed — they are kept below, marked, so
the record stays complete. The remaining eighty findings touch no file that changed.

Commits were still arriving as the third pass finished. Treat **FIXED** as verified at `681b767`;
anything committed after that is unaudited, and findings L5, L6 and L8 in particular may have been
addressed by `f0e7c8e` after the re-check read them.

### Read this first

Three defects are paying the wrong amount of money right now: **R2-C1** (EPF computed on an
overtime-inclusive wage), **R2-C2** (a NULL date of birth silently becoming age 30, the one value
that clears every fail-closed age guard) and **R1-C2** (a forgotten check-out becoming uncapped paid
overtime at 1.5x). EPF, SOCSO and EIS have no fail-closed gate, so all three are live.

The PCB `require_supported_calculator` hard-fail is doing more load-bearing work than it looks. It
is the only reason the bonus-annualisation, relief-cap and bracket-gap defects are not also live
over-deductions. Treat it as un-liftable until the statutory *input* layer has tests: today no test
asserts what belongs in a contributable wage, and none commits two runs in one year.

The single highest-leverage fix is one validator. Four filesystem sinks join a client-supplied
string onto a base path with no traversal check. The correct guard already exists in this codebase,
at `handlers/portal.rs:451`, and was never applied to any write, delete, or restore path.

---

## Critical (11)

Wrong money paid, cross-tenant data movement, or arbitrary file access.

### R1-C1. Claims approved after their expense month's run are never paid by any payroll run

`CRITICAL` · OPEN

**Where:** `backend/src/services/approval_service/claim.rs:255-272` · `backend/src/repositories/reads/payroll.rs:85,117,245-269` · `backend/src/services/payroll_service.rs:117-125` · `backend/src/repositories/payroll_entries.rs:144-151,387-398`

Approval stages a `claim_reimbursement` entry keyed to the *approval* month, which the engine explicitly excludes from gross/net; the only path that actually pays claims filters on `expense_date` inside the run period. A claim incurred 10 Jun and approved 3 Jul (June run already closed, and closed runs cannot be re-run) is paid by nothing, stays `approved` forever, and the July run flips the staged row to `is_processed = TRUE` so `cancel_claim_admin` then refuses it as "already included in processed payroll". Employee is told by email it will be paid.

**Fix:** make one date authoritative — either select claims by approval/staging period (or all approved-unprocessed up to `period_end`), or pay from the staged `claim_reimbursement` entries and drop the parallel `claims` read.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-C2. Forgotten check-out silently becomes paid overtime at 1.5x

`CRITICAL` · OPEN

**Where:** `backend/src/repositories/attendance_records.rs:94` · `backend/src/repositories/reads/payroll.rs:196` · `backend/src/services/payroll_engine.rs:943`

Check-out matches any open record within 24 h and writes `overtime_hours = elapsed − shift` with no cap, no approval gate, no CHECK constraint, and no preview diagnostic. Check in Mon 09:00, tap check-out Tue 08:00 → 14.00 OT hours → ~RM404 of unworked OT on an RM5,000 salary, which also inflates gross and therefore EPF/SOCSO/EIS/PCB. The same day also counts absent.

**Fix:** clamp derived `hours_worked`/`overtime_hours` to a configurable per-day ceiling, or leave OT NULL and flag the record for correction above a threshold.

<sub>No file in this finding changed during the audit.</sub>

### R1-C3. Admin-created overtime accepts unbounded/negative hours straight into payroll gross

`CRITICAL` · OPEN

**Where:** `backend/src/services/approval_service/overtime.rs:35,109` · `backend/src/repositories/reads/payroll.rs:229`

No positivity check, no cap against the declared start/end window, no DB CHECK on `hours numeric(5,2)` — while the portal path (`portal_service.rs:371-381`) enforces both. Any `ManageApprovals` holder books `hours: 999.99, ot_type: public_holiday` for a 1-hour window → ~RM65,000 staged earning on an RM4,000 salary; `hours: -50` silently reduces gross.

**Fix:** extract the portal's hours validation into `approval_service::common` and call it from both admin create/update; add `CHECK (hours > 0 AND hours <= 24)` in a new migration.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-C4. No maker-checker: one approver can raise and approve a payment to themselves

`CRITICAL` · OPEN

**Where:** `backend/src/services/approval_service/claim.rs:249` · `.../overtime.rs:35,305` · `.../leave.rs:359`

`approve_*` never compares the reviewer to the record's `employee_id` or its creator. An hr_manager POSTs a RM50,000 pending claim against their own employee record (no receipt required), approves it, and the next run pays it on top of net — bypassing gross and all statutory deductions. Audit logs it; nothing blocks it. Same for admin-created OT and leave.

**Fix:** resolve the caller's `employee_id` and return `Forbidden` when it equals the target's, mirroring the existing self-delete guards; route genuine HR-of-one overrides to `super_admin`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-C5. Client-supplied file paths are trusted at four sinks → arbitrary file write (RCE), delete, and secret exfiltration

`CRITICAL` · OPEN

**Where:** `backend/src/services/document_service.rs:54,80` · `backend/src/services/backup_service/files.rs:21,55` · `backend/src/services/portal_service.rs` · `document.rs:47` · `backup.rs:47` (claim `receipt_url`, leave `attachment_url`) models:

No validator anywhere; `Path::join` with an absolute or `../` string discards the `uploads` base, and `infra/Dockerfile` chowns `/app` to the runtime UID. - **Write/RCE:** backup import key `"/api/uploads//app/payroll-system"` overwrites the API binary; executes on restart. - **Delete:** any `ManageDocuments` holder creates a document with a crafted `file_url`, deletes it, and unlinks another tenant's upload (flat shared `uploads/`, no ownership check) or the binary itself. - **Read:** a plain `employee` sets `attachment_url = "/api/uploads/../../app/.env"`; the next super_admin export base64-embeds `JWT_SECRET` and the Postgres password into the downloadable JSON. `handlers/portal.rs:452` already implements the correct guard for the download path only.

**Fix:** validate at write time (`^/api/uploads/[A-Za-z0-9._-]+$`, single `Component::Normal`) on `file_url`/`receipt_url`/`attachment_url`, re-apply the same check at both backup sinks and the delete sink, and for restore ignore the supplied key entirely — write to a server-generated name and rewrite the row's `file_url`.

<sub>No file in this finding changed during the audit.</sub>

### R1-C6. Bulk employee import reports success while committing zero rows

`CRITICAL` · **FIXED**

**Where:** `backend/src/services/employee_import_service/confirm.rs:143-185` · `validation.rs:12` (esp. :152 `let _ =`, :165, :185)

Under the default `skip_invalid = true`, a failed row INSERT (over-length `employee_number` — never length-validated; a duplicate created between validate and confirm; a cross-tenant `payroll_group_id`) aborts the shared Postgres transaction. The loop keeps issuing statements (all 25P02), `COMMIT` on an aborted block returns `ROLLBACK` as `Ok`, so the API returns `imported_count: 49`, writes an audit row claiming 49 imports, and marks the session confirmed — with zero employees created and no retry possible.

**Fix:** wrap each row in a savepoint (or its own top-level transaction) so one bad row doesn't poison the rest; stop discarding `salary_history::insert_bulk_import_initial` errors, which poison it identically.

<sub>Fixed by a commit that landed while this audit was running; re-verified against `681b767`.</sub>

### R2-C1. EPF contributions computed on an overtime-inclusive wage

`CRITICAL` · OPEN

**Where:** `backend/src/services/payroll_engine.rs:989,993` · `backend/src/services/epf_service.rs:29`

One `gross = basic + allowances + variable + overtime` is passed to all three statutory calculators. Overtime is excluded from "wages" under EPF Act 1991 s.2 but included under ESSA 1969 — so every payslip carrying OT resolves in an inflated Third Schedule band. RM3,000 basic + 20h OT: employee EPF 38000 sen instead of 32000, employer 42000 instead of 35000. Wrong money on every OT payslip and a wrong EPF remittance file, live today.

**Fix:** compute two wage bases in `compute_payslip` — an EPF-contributable wage excluding `total_overtime`, and the OT-inclusive wage for SOCSO/EIS/PCB.

<sub>No file in this finding changed during the audit.</sub>

### R2-C2. NULL `date_of_birth` silently becomes age 30, defeating every age gate

`CRITICAL` · OPEN

**Where:** `backend/src/services/payroll_engine.rs:1301-1312,459,576` · `socso_service.rs:43-54` · `eis_service.rs:36-48` · `1000_schema.sql:506` · `employee_import_service/validation.rs:18-36`

The substituted 30 is precisely the value that clears all fail-closed guards. An imported 62-year-old on RM2,500 with no DOB is rated First Category: SOCSO employee 905 / employer 1665 instead of 0 / 1015, EIS 490/490 instead of 0/0 — an unlawful employee deduction committed to a payslip. DOB is nullable, the bulk importer doesn't require it, and the `missing_date_of_birth` diagnostic goes to `warnings` while `can_process` reads only `blocking`.

**Fix:** return `AppError::Validation` from `compute_payslip` when `date_of_birth` is `None`, so it blocks preview and commit — matching the 55-59 and foreign-worker branches.

<sub>No file in this finding changed during the audit.</sub>

### R2-C3. Terminations cannot be recorded as dates; leavers are over-paid or silently dropped

`CRITICAL` · OPEN

**Where:** `backend/src/repositories/employees.rs:230,544` · `frontend/src/pages/employees/EmployeeList.tsx:855-857` · `payroll_engine.rs:890-906` · `repositories/reads/ea_form.rs:25,95`

The engine's mid-month proration keys on `date_resigned`, which the UI never exposes (request types omit it). HR has only Active/Inactive. Leave them Active → full month's basic + full statutory for a partial month. Set Inactive → `is_active = TRUE` excludes them, the final payslip vanishes with no error and no preview diagnostic, statutory contributions go unremitted, and the EA form is a month short. `COALESCE($27, date_resigned)` also makes an API-set date unclearable.

**Fix:** select the run population on employment dates rather than the mutable flag, block on any employee excluded solely by `is_active = FALSE` with no resignation date, and expose `date_resigned` in the edit form.

<sub>No file in this finding changed during the audit.</sub>

### R2-C4. Unvalidated `file_url` reaches filesystem sinks — arbitrary file delete, and read of container secrets

`CRITICAL` · OPEN

**Where:** `backend/src/services/document_service.rs:78-82` · `models/document.rs:49` · `repositories/documents.rs:151` · `1000_schema.sql:376` · `backend/src/services/backup_service/files.rs:21,55` · `handlers/portal.rs:451-454` (guard that exists: )

`Path::new("uploads").join(file_url.strip_prefix("/api/uploads/"))` with no traversal check; an absolute component replaces the base outright. A `finance` or `exec` user (both hold `ManageDocuments`) creates a document with `file_url = "/api/uploads//etc/ssl/private/key.pem"` and deletes it. The backup export side base64-embeds whatever path the stored `file_url` names — `/api/uploads/../../proc/self/environ` exfiltrates `DATABASE_URL` and `JWT_SECRET`, converting an app role into forged tokens for every tenant. The sibling `serve_upload` performs exactly the missing check.

**Fix:** validate on write that `file_url` matches `/api/uploads/<single component>`, and on every read/delete/restore require `Path::components()` to yield exactly one `Component::Normal` before joining.

<sub>No file in this finding changed during the audit.</sub>

### R2-C5. Backup import FK remap falls back to identity — cross-tenant rows, including a payable allowance

`CRITICAL` · OPEN

**Where:** `backend/src/services/backup_service/import.rs:147-214` · `repositories/backup.rs:1192` · `1000_schema.sql:2402`

`remap` is built only from IDs inside the archive; any FK naming a row outside it passes through verbatim via `unwrap_or(&old)`. Four tables have no tenant FK or trigger: `employee_allowances`, `salary_history`, `tp3_records`, `payroll_item_details`. Appending one recurring `employee_allowances` row pointing at another tenant's `employee_id` makes *that tenant's* next run pay it — `recurring_allowance_totals` filters only on employee_id/active/recurring. The tenant-guarded siblings prove the invariant is intended.

**Fix:** make `r()` return `AppResult<Uuid>` and fail closed on an unknown id; add composite `(employee_id, company_id)` FKs or `enforce_*_company` triggers to the four unguarded tables.

<sub>No file in this finding changed during the audit.</sub>

---

## High (26)

Wrong statutory output, auth bypass, data loss, or a workflow that cannot complete.

### R1-H1. EA form and payslip PDFs print a breakdown that does not sum to their own stated total

`HIGH` · OPEN

**Where:** `backend/src/services/ea_form_service.rs:309-315,348` · `backend/src/services/payslip_pdf_service.rs:166,607` · `backend/src/repositories/reads/payroll.rs:150,175` · `frontend/src/pages/payroll/PayrollProcess.tsx:365`

`variable_earnings` (→ gross) includes every earning `item_type`, but the allowance/bonus/commission totals use narrow allow-lists. The shipped "Other Earning" UI option writes `item_type: 'manual_adjustment'`, which is in neither list. Result: payslip shows "Basic 3,000.00 / TOTAL EARNINGS 3,500.00" with no line for the RM500; the EA form prints items 1-5 summing to 60,000 under a TOTAL EMPLOYMENT INCOME of 61,000 — a statutory LHDN document that fails its own arithmetic. No CHECK, no validation, no test.

**Fix:** render both documents from the exhaustive `payroll_item_details` earning lines, or emit an explicit residual "Other income" row = `gross − (basic + allowances + bonus + commission + overtime)`.

<sub>No file in this finding changed during the audit.</sub>

### R1-H2. Statutory submission files are numerically unparseable and unescaped

`HIGH` · OPEN

**Where:** `backend/src/services/statutory_export_service.rs:57-60,98-101,139-142,174,179` · `backend/src/services/attendance_service.rs:1083` cf. correct helper at

All four exports (EPF/SOCSO/EIS CSV + CP39) format amounts with the *PDF display* helper, so every wage over RM1,000 is written `1,700.00` — importers truncate at the comma or reject the record. The CP39 builder concatenates `employee_name`/IDs into a pipe-delimited record with no escaping (`Lee|Wei` → 7 fields instead of 6; a newline splits the record), and the CSV writers omit the leading-apostrophe guard so `=HYPERLINK(...)` as a name executes in Excel. Nothing validates before the operator submits.

**Fix:** add `sen_to_plain_rm` for machine-readable output; route every free-text column through the existing `csv_field` formula-neutralizer; reject/strip `|`, `\r`, `\n` in CP39 fields.

<sub>No file in this finding changed during the audit.</sub>

### R1-H3. Attendance overtime is bucketed in hardcoded MYT → double payment and wrong-month shifts

`HIGH` · OPEN

**Where:** `backend/src/repositories/reads/payroll.rs:200,208`

`attendance_ot_hours` uses `AT TIME ZONE 'Asia/Kuala_Lumpur'` while `company_work_schedules.timezone` is admin-settable and threaded everywhere else. For an `Asia/Jakarta` tenant, a 31 Jul 23:30 WIB shift lands on 1 Aug: the approved-OT dedup join misses (paid twice — once as attendance OT, once via `approved_ot_totals`) and boundary shifts land in the wrong run. The expression also wraps the indexed column, so every run full-scans attendance history — explicitly forbidden by CLAUDE.md.

**Fix:** take a `tz` parameter from `gather_run_inputs` and use sargable half-open bounds on raw `check_in_at`.

<sub>No file in this finding changed during the audit.</sub>

### R1-H4. Recurring allowances are matched on a single date and never prorated

`HIGH` · OPEN

**Where:** `backend/src/repositories/reads/payroll.rs:29` · `payroll_engine.rs:888-892` cf

Predicate is `effective_from <= period_end AND (effective_to IS NULL OR effective_to >= period_end)`. A leaver whose allowance correctly ends on their last day gets RM0 allowance while basic is calendar-prorated; a joiner granted an allowance on the 25th is paid the full month. Both flow into gross, so EPF/SOCSO/EIS/PCB are computed off the wrong wage.

**Fix:** interval-overlap predicate plus per-line proration by the overlap with worked days.

<sub>No file in this finding changed during the audit.</sub>

### R1-H5. Company hard-delete and overwrite-restore are permanently broken by missing cascade tables (two divergent implementations)

`HIGH` · OPEN

**Where:** `backend/src/repositories/companies.rs:206,255` · `backend/src/repositories/backup.rs:468-554` · `migrations/1000_schema.sql:2234,2634,2762` (:479, :517, :541, :544)

`companies::delete_cascade` omits `payroll_item_details`, `tp3_records`, `audit_logs` (all NO ACTION FKs) → `DELETE /api/admin/companies/{id}` raises 23503 and returns an opaque 500 for any tenant that has run payroll or (in practice always) has an audit row. `backup::delete_company_cascade` — the duplicated copy — covers those but omits `email_logs`, `notifications`, `bulk_import_sessions`, so every overwrite restore fails for any tenant that ever sent one email. The two lists have drifted in opposite directions.

**Fix:** collapse into one shared wipe-order helper covering all of them, or add `ON DELETE CASCADE`/`SET NULL` to the offending FKs in a new migration; test with a seeded `email_logs` + `payroll_item_details` row.

<sub>No file in this finding changed during the audit.</sub>

### R1-H6. Production rate limiting collapses to one global bucket; every audit IP is identical

`HIGH` · **FIXED**

**Where:** `deploy/docker-compose.prod.yml:70` · `backend/src/routes/mod.rs:26-31` CLAUDE.md:63

`TRUST_PROXY_HEADERS` is never passed and the compose environment allow-list makes it unsettable via `secrets.env`, so all four unauthenticated limiters key on the docker bridge gateway. `/auth/login` becomes a single system-wide 5-request bucket: one anonymous attacker locks out logins for every tenant, and `forgot-password`/kiosk limiters are globally shared — reproducing exactly the kiosk-fleet failure CLAUDE.md claims was fixed. Every `audit_logs.ip_address` reads `172.18.0.1`.

**Fix:** add `TRUST_PROXY_HEADERS: "true"`; verify the host Caddyfile does not pass a client-supplied `X-Forwarded-For` through; correct CLAUDE.md.

<sub>Fixed by a commit that landed while this audit was running; re-verified against `681b767`.</sub>

### R1-H7. Deploy rollback crash-loops production when the failed release added a migration

`HIGH` · OPEN

**Where:** `deploy/deploy.sh:391` · `backend/src/core/db.rs:47-50` (also :228, :371)

A release that commits its migration then fails startup (bad `FRONTEND_URL`, missed 240 s health deadline) rolls back the *image only*. The N-1 binary's `reject_unknown_migration_history` asserts on the applied-but-unknown version, panics, and `restart: unless-stopped` crash-loops it. Neither image boots; api.payrollmy.com stays down pending manual intervention.

**Fix:** restore the pre-deploy dump alongside the image rollback, or relax the assert to a warning for applied versions strictly newer than the newest embedded one; at minimum print the dump path in the failure message.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-H8. `terraform apply` with committed defaults destroys production DNS and certificates

`HIGH` · OPEN

**Where:** `infra/variables.tf:33` · `infra/s3_cloudfront.tf`

`environment` defaults to `dev` → `payroll-dev` prefix matches the *live* stack in the hardcoded shared S3 backend; `domain_name` defaults to `""` with no committed tfvars. A clean-clone `terraform apply` strips the `payrollmy.com` alias from distribution ED4843A8VKOA2 and destroys the ACM cert, validation, and both Route53 A records — frontend and `api.payrollmy.com` offline.

**Fix:** commit `infra/terraform.tfvars.example` and remove the `domain_name` default so Terraform fails instead of planning a destroy; drop the `has_domain` conditionals.

<sub>No file in this finding changed during the audit.</sub>

### R1-H9. OAuth2 `state` has no browser binding → login CSRF / session fixation

`HIGH` · OPEN

**Where:** `backend/src/handlers/oauth2.rs:42,184-202` · `backend/src/repositories/oauth2_states.rs:22` · `migrations/1000_schema.sql:709`

`/authorize` sets no cookie and the state row is redeemable from any browser for 10 minutes. An attacker completes consent with his own Google account, captures `code&state`, and gets a victim payroll_admin to open the public callback — the victim's browser receives the *attacker's* JWT and refresh cookie. Every record she then creates lands in the attacker's tenant.

**Fix:** set a short-lived httpOnly `SameSite=Lax` binder cookie at `/authorize` and require it to match `state` in the callback.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-H10. Refresh-token rotation is neither atomic nor reuse-detecting

`HIGH` · OPEN

**Where:** `backend/src/services/session_service.rs:55-68` · `backend/src/services/auth_service.rs:144-156` · `backend/src/repositories/refresh_tokens.rs:38,42-48`

Revoke/insert/touch are three autocommit statements and `revoke_by_hash` has no `revoked = FALSE` predicate and discards `rows_affected`. Two tabs refreshing concurrently both validate and both mint live tokens on one `session_id` — an orphan 30-day credential the user can never see or rotate in `list_sessions`. A replayed stolen token likewise yields the attacker his own chain, and reuse of a revoked token returns a bare 401 that never invalidates the session family.

**Fix:** one transaction; make the revoke the guard (`... WHERE token_hash = $1 AND revoked = FALSE RETURNING ...`), treat 0 rows as reuse → revoke the whole session.

<sub>No file in this finding changed during the audit.</sub>

### R1-H11. hr_manager/admin can never save an edit to any employee with bank details on file

`HIGH` · OPEN

**Where:** `frontend/src/pages/employees/EmployeeList.tsx:450-454,868-895` · `frontend/src/pages/employees/EmployeeCreate.tsx:281-303` · `backend/src/handlers/employee.rs:96-98`

`stripPayrollFields` omits `bank_name`/`bank_account_number`, which the backend classifies as payroll-sensitive. Editing a phone number on an employee with banking details returns 403 "Payroll fields are not available for this role" — total edit lockout — and the always-visible Banking Details section reproduces it on create.

**Fix:** add both fields to the strip list, gate the Banking Details section on `canViewPayroll`, and derive the list from one shared constant.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-H12. One bad tenant timezone aborts the auto-absent run for every remaining tenant, permanently

`HIGH` · OPEN

**Where:** `backend/src/services/attendance_service.rs:1053` · `backend/src/services/work_schedule_service.rs:44,115`

`timezone` is stored with no validation and no CHECK. A typo (`Asia/Kuala_Lumpr`) makes Postgres reject `AT TIME ZONE`; the `?` aborts the loop before the shared `auto_absent_last_run_date` bookmark advances, so every later tick re-fails on the same date and owed dates fall past the 14-day backfill floor and are lost for all tenants — with no log line naming the cause. The same unvalidated value 500s every check-in/check-out/summary/export for that company.

**Fix:** validate the string parses as `chrono_tz::Tz` on write; per-company `match` + error log + continue in the catch-up loop; advance the bookmark only for fully-successful dates.

<sub>No file in this finding changed during the audit.</sub>

### R1-H13. Auto-absent evaluates the 12:30 cutoff in MYT but applies it to every tenant's local calendar

`HIGH` · OPEN

**Where:** `backend/src/services/attendance_service.rs:1033-1053` · `main.rs`

For a `Pacific/Honolulu` tenant the 04:30 UTC tick writes absent placeholders dated a day in the future — rows the admin `absent-run` endpoint explicitly forbids. The whole workforce shows absent in summary, dashboard, and CSV until each employee checks in.

**Fix:** resolve `today`/`now_local` per company from the already-loaded tz and keep a per-company bookmark.

<sub>No file in this finding changed during the audit.</sub>

### R1-H14. Multipart body limit blocks backup restore and employee import entirely

`HIGH` · OPEN

**Where:** `backend/src/routes/mod.rs:178,197-200,378` · `handlers/backup.rs:117` · `handlers/employee_import.rs:89` (vs :299-304)

Only `/uploads` overrides axum 0.8's 2 MB `DefaultBodyLimit`. The handlers' documented 100 MB / 20 MB ceilings are dead code: any backup containing a single base64-expanded attachment, or a 3 MB XLSX well under the 1,000-row cap, fails with a confusing `400 length limit exceeded`. A backup this system produced cannot be restored by it.

**Fix:** attach `DefaultBodyLimit::max(...)` matching each handler's constant to `/admin/backup/import`, `/employees/import/validate`, `/calendar/import-ics-file`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-H15. Overtime forms display phantom time defaults that never enter form state

`HIGH` · OPEN

**Where:** `frontend/src/pages/portal/Overtime.tsx:359,364` · `frontend/src/pages/approvals/Approvals.tsx:1626-1644,1753,1758`

`value={form.start_time || '18:00'}` renders 18:00/19:00 while state holds `''`. Editing only the end time leaves `start_time` empty, `calculateHours` returns 0, and Submit stays greyed out with no error (portal) or fails with "time range is required" contradicting the visible times (admin).

**Fix:** seed state with the real defaults and drop the display fallbacks; change `if (diff <= 0) diff += 24*60` to `< 0`.

<sub>No file in this finding changed during the audit.</sub>

### R1-H16. Employee pickers silently truncate at 100, making employees 101+ unselectable

`HIGH` · OPEN

**Where:** `frontend/src/pages/payroll/PayrollProcess.tsx:74` · `frontend/src/pages/approvals/Approvals.tsx:227` · `frontend/src/pages/teams/TeamsPage.tsx:51` · `backend/src/handlers/employee.rs:113` · `document.rs:29` document-upload and letters pickers

Pickers request `per_page` 200-500; the handler clamps to 100 ordered by `employee_number`. No search, no paging, no `total` check. In a 300-employee company, EMP250 cannot be added to a team, given a payroll allowance, granted a leave/claim/OT record on their behalf, or sent a letter.

**Fix:** use the server-side-search `EmployeePicker` pattern from `AttendancePage.tsx:90-184` (`getEmployees({ search, per_page: 20 })`); never request a `per_page` the server clamps.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-H1. PCB edit can drive net salary negative and corrupt the run total

`HIGH` · OPEN

**Where:** `backend/src/services/payroll_service.rs:182,221` · `models/payroll.rs:203-205` · `1000_schema.sql:884` · `payroll_engine.rs:1058` (guard that exists: )

`update_item_pcb` checks only `pcb_amount >= 0`, then writes `net = net - delta` unbounded and decrements `total_net`. A sen/ringgit typo produces a payslip PDF reading "NET SALARY RM -400.00" that survives submit/approve/pay — the exact state `compute_payslip` refuses to create.

**Fix:** reject the edit when `new_net_salary < 0`, and bound `pcb_amount` by `current.net_salary + current.pcb_amount`.

<sub>No file in this finding changed during the audit.</sub>

### R2-H2. Passkey and Google logins bypass the terminated-employee gate

`HIGH` · OPEN

**Where:** `backend/src/handlers/passkey.rs:151,235` · `handlers/oauth2.rs:188` · `services/auth_service.rs:73,118,149`

`linked_employee_active` is called from `login` and `refresh_session` but not from the shared `complete_login` chokepoint. Deactivating an employee leaves the `users` row untouched, so password login 401s while a passkey or Google sign-in issues a full JWT + refresh cookie — repeatable indefinitely, full portal access to payslips and documents.

**Fix:** move the `linked_employee_active` check into `complete_login` and delete the duplicate in `login`.

<sub>No file in this finding changed during the audit.</sub>

### R2-H3. New-hire IC number and initial portal password persisted in `email_logs`, readable by `exec`

`HIGH` · OPEN

**Where:** `backend/src/services/employee_service.rs:147` · `services/email_service.rs:113-127,361` · `handlers/employee.rs:42,203` · `core/permission.rs:388`

The initial password *is* the IC number, rendered into the welcome email HTML and stored verbatim by `insert_pending` before the SMTP-enabled check. `GET /api/email/logs` gates only on `ViewEmailLogs` (held by `exec`) and returns `SELECT *` including `body_html` — so the role whose IC access is deliberately nulled by `redact_personal_fields` reads every employee's IC and still-valid password in cleartext. `must_change_password` is never enforced at login.

**Fix:** store a redacted body for `welcome` letters, and project `email_logs::list` to the columns the History table renders, exposing bodies only via a separately permissioned detail endpoint.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-H4. Audit log values leak salary, TIN and statutory numbers to non-payroll roles

`HIGH` · OPEN

**Where:** `backend/src/services/employee_service.rs:79,214-215` · `services/company_service.rs:62-63` · `repositories/reads/audit.rs:28-34` · `models/audit.rs:85-86`

Every create/update stores the fully serialized entity. `admin` holds `ViewAuditLog` but no payroll permission, so `GET /api/audit-logs?entity_type=employee` returns exactly the fields `redact_payroll_fields` strips — `basic_salary`, `tax_identification_number`, EPF/SOCSO/EIS numbers — plus a before/after pair per raise, reconstructing the salary history gated behind `ViewPayroll`. Same for company statutory codes. `handlers/email.rs:26-28` already articulates this hazard.

**Fix:** project audit values through a per-entity allow-list (as `user_service::audit_snapshot` does), or filter `old_values`/`new_values` by the caller's effective permissions in `reads::audit::list_filtered`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-H5. Bonus/commission annualised as recurring income — *latent behind the PCB fail-closed gate*

`HIGH` · OPEN

**Where:** `backend/src/services/payroll_engine.rs:989,1013-1032` · `repositories/reads/payroll.rs:85,117` · `services/pcb_calculator.rs:47,82,180-200`

`entry_category_totals` excludes only overtime and claims, so `item_type` bonus/commission flows into `gross` and is multiplied by `remaining_months`. A one-off RM5,000 bonus on RM5,000/month inflates projected annual income RM65k → RM95k and that month's PCB RM175 → RM791. The Schedule-2 path is dead code (`is_bonus_month` is a literal `false` at both construction sites) and would double-count if merely flipped, because `annual_income_without_bonus` already contains the multiplied bonus. Unreachable today: `statutory_rules.rs:14-23` hard-fails PCB in non-test builds.

**Fix:** exclude bonus/commission from `monthly_gross`, pass them via `bonus_amount`/`is_bonus_month`, and correct `calculate_bonus_pcb` to receive a genuinely bonus-free annual income.

<sub>No file in this finding changed during the audit.</sub>

### R2-H6. Backup import capped at axum's 2 MiB default — disaster recovery is impossible for any real tenant

`HIGH` · OPEN

**Where:** `backend/src/routes/mod.rs:133` · `handlers/backup.rs:117` · `handlers/calendar.rs:152` · `handlers/employee_import.rs:71` (also , )

No `DefaultBodyLimit` on the import route, so the handler's 100 MB check is dead code. `payroll_items` alone is ~1.4 KB/row pretty-printed (~1,500 rows breaks it), and attachments are base64-embedded. The operator gets `400 Failed to read file data` — a stream error, not a 413 — pointing away from the cause. The sibling `/uploads` route sets the limit explicitly, proving the omission.

**Fix:** attach `DefaultBodyLimit::max(100 * 1024 * 1024)` and stream via `field.chunk()` into a size-checked buffer rather than `field.bytes()`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-H7. `rollback` abandons the post-cutover database and prints "no database action was needed"

`HIGH` · OPEN

**Where:** `deploy/migrate-database.sh:337-354` · `deploy/docker-compose.prod.yml:69,71`

After cutover the container is production. Rollback stops the backend and restarts the native unit against the pre-cutover cluster, silently stranding every approval, check-in and leave decision committed in between — with a closing log line that actively tells the operator there is nothing to reconcile.

**Fix:** `pg_dump` `payroll_db` before abandoning it, refuse without `--force` when it holds rows newer than the cutover timestamp, and replace the closing line with an explicit statement of what is stranded and where the dump lives.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-H8. `stage_verify` certifies a stale snapshot as correct

`HIGH` · OPEN

**Where:** `deploy/migrate-database.sh:156,214-246,284` · `.github/workflows/migrate-database.yml:16`

The documented hard gate compares row counts for five tables no ordinary user writes to (companies, employees, users, payroll_runs, `_sqlx_migrations`), while `backup` dumps a still-serving cluster and stages are spread across separate manual dispatches. Hours of attendance, leave, claims and audit rows written in the window are absent from the restored DB and verify prints PASS. Count comparison also cannot see UPDATEs.

**Fix:** make the snapshot atomic (read-only the native DB before `pg_dump`), or extend `tables` to every high-write table and compare `max(created_at)`/`max(updated_at)` so verify fails when the native side has advanced.

<sub>No file in this finding changed during the audit.</sub>

### R2-H9. Any tenant admin can halt auto-absent marking platform-wide with an invalid timezone

`HIGH` · OPEN

**Where:** `backend/src/services/work_schedule_service.rs:44,115` · `services/attendance_service.rs:1046-1065` · `repositories/attendance_records.rs:373-374` · `1000_schema.sql:343` · `main.rs:207-211`

No IANA validation on write and no CHECK. `AT TIME ZONE $1` raises `invalid_parameter_value`, the `?` aborts the whole catch-up before the bookmark write, so every subsequent tick re-runs and re-fails the same date for every tenant. `list_company_timezones` has no ORDER BY, so which tenants get marked before the abort is nondeterministic. Skipped dates age past the 14-day cap and are lost; recovery needs an operator to hand-fix one row.

**Fix:** validate against `chrono_tz::Tz::from_str` at the write path, and make the per-company loop log-and-continue while tracking whether the date fully succeeded before bookmarking.

<sub>No file in this finding changed during the audit.</sub>

### R2-H10. Unbounded leave date span exhausts the connection pool from a bare `employee` token

`HIGH` · OPEN

**Where:** `backend/src/services/portal_service.rs:53,60` · `services/leave_rules.rs:76-111` · `services/calendar_service.rs:201-231` · `services/approval_service/leave.rs:34,130` · `core/db.rs:4-10`

`count_working_days_between` runs before `validate_period`, which has no maximum-span rule. `{"start_date":"0001-01-01","end_date":"9999-12-31"}` becomes 9,999 sequential holiday queries and 3.65M loop iterations (chrono's `+262142-12-31` form gives ~262K queries). The route has no governor layer and the pool is capped at 10, so ~10 concurrent requests stall every tenant.

**Fix:** validate ordering and a max span (e.g. 366 days) before any calendar lookup, and fetch holidays for the whole range in one query.

<sub>No file in this finding changed during the audit.</sub>

---

## Medium (57)

Silent partial state, races, resource exhaustion, and correctness bugs with a bounded blast radius.

### R1-M1. Unpaid-leave deduction is written outside the approval transaction and can be lost forever

`MEDIUM` · **FIXED**

**Where:** `backend/src/services/approval_service/leave.rs:374,377-447`

Status + balance commit, then three un-transacted round trips stage the deduction. Any transient failure leaves the leave permanently `approved` with no `payroll_entries` row; re-approving matches only `pending`. 10 days of unpaid leave on RM6,000 → ~RM2,300 overpaid, with nothing to reverse.

**Fix:** move `insert_unpaid_leave_deduction` into the same transaction as `set_approved` (repos are executor-generic).

<sub>Fixed by a commit that landed while this audit was running; re-verified against `681b767`.</sub>

### R1-M2. Leave rejection strands reserved `pending_days` irrecoverably

`MEDIUM` · **FIXED**

**Where:** `backend/src/services/approval_service/leave.rs:546-560`

`set_rejected` and `subtract_pending` are separate autocommit statements — the one leave transition left non-transactional. A fault between them leaves `pending_days = 5` forever; no path refunds a `rejected` row, and `add_pending_within_entitlement` silently caps the employee at 9 of 14 days with no visible cause.

**Fix:** wrap both in one transaction, as `approve_leave` already does.

<sub>Fixed by a commit that landed while this audit was running; re-verified against `681b767`.</sub>

### R1-M3. Overtime rating diverges between the approval path and the engine, and can panic

`MEDIUM` · OPEN

**Where:** `backend/src/services/approval_service/overtime.rs:292-305` · `backend/src/services/payroll_engine.rs:831-837,926,966`

Approval uses double integer-truncated division and `f64` money math with `parse::<i64>()` on settings; the engine uses `Decimal` and `from_str_exact`. Default settings → RM108.12 quoted vs RM108.17 paid; `effective_hours_per_day = "7.5"` is silently rejected to 8 by the approval path → a 6.7% gap. A saved value of `"0"` (no CHECK, no service validation) divides by zero and panics the handler *after* the OT row is committed approved, leaving it approved with no staged entry. `OvertimeSettings`' doc comment claims the two paths are identical.

**Fix:** one shared `Decimal` rating function used by both; validate numeric payroll settings `> 0` in `settings_service`; mirror the engine's `.filter(|v| *v > 0)` guard.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M4. The HR-editable `unpaid_leave_divisor` moves no money; a same-named setting secretly drives the OT rate

`MEDIUM` · OPEN

**Where:** `backend/src/services/approval_service/leave.rs:405-419` · `backend/src/services/company_service.rs:44-46` · `backend/src/services/payroll_engine.rs:831-837,926`

`companies.unpaid_leave_divisor` is validated, persisted, role-gated, and read by nothing. The deduction divides by the calendar's working days (fallback 22); the `payroll/unpaid_leave_divisor` *setting* instead changes the overtime hourly rate. Setting 22 with a 21-working-day month yields RM142.85 instead of the promised RM136.36.

**Fix:** pick one source of truth (wire the column into the deduction or delete it) and rename the setting key to reflect that it is the overtime divisor.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M5. Unpaid leave is folded into "Other Deductions"; the payslip stops adding up

`MEDIUM` · OPEN

**Where:** `backend/src/services/payroll_engine.rs:1112` · `reads/payroll.rs:130-134` `payroll_items::insert` cf

`unpaid_leave` entries land in `other_deductions` and the dedicated `unpaid_leave_deduction`/`unpaid_leave_days` columns are never written, so the payslip PDF mislabels the amount and the employee portal — which renders no other-deductions row — omits it entirely, leaving itemized deductions short of Total Deductions. Same defect class already fixed for bonus/commission.

**Fix:** add an `unpaid_leave` split to the entry read, carry it on `ComputedPayslip`, pass amount + day count to `payroll_items::insert`, and keep it out of `other_deductions`.

<sub>No file in this finding changed during the audit.</sub>

### R1-M6. Overtime entries are matched by `description LIKE`, so same-date siblings collide

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/payroll_entries.rs:305,387-398`

Two OT applications on the same date/type share a description prefix. An unpaid one cannot be cancelled once its sibling is processed; cancelling either unprocessed one deletes the sibling's marker, letting an already-paid OT be cancelled and its public-holiday replacement leave clawed back.

**Fix:** add `source_id uuid` to `payroll_entries` and match exactly, as `exists_processed_claim` does.

<sub>No file in this finding changed during the audit.</sub>

### R1-M7. Concurrent OT cancels each decrement replacement-leave entitlement

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/overtime_applications.rs:156,264`

No `status <> 'cancelled'` predicate (unlike the leave path). A double-clicked cancel runs `subtract_entitled_replacement` twice, destroying a day earned by a different, still-approved application.

**Fix:** add the predicate, return `rows_affected() > 0`, and abort with "already cancelled".

<sub>No file in this finding changed during the audit.</sub>

### R1-M8. Duplicate leave requests for identical dates both pass validation

`MEDIUM` · OPEN

**Where:** `backend/src/services/portal_service.rs:72` · `backend/src/services/approval_service/leave.rs:49`

`overlaps_existing` runs on the pool before `pool.begin()`, and there is no UNIQUE/EXCLUDE on the range. Two concurrent submissions both insert and both become approvable — for unpaid leave, each approval stages its own payroll deduction for the same absence.

**Fix:** move the check inside the transaction behind `SELECT ... FOR UPDATE` on the balance row, or add an `EXCLUDE USING gist` constraint.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M9. Two concurrent demotions can leave zero active super admins, unrecoverable

`MEDIUM` · OPEN

**Where:** `backend/src/services/user_service.rs:394,476` · `backend/src/repositories/users.rs:218-228` · `backend/src/bin/bootstrap_admin.rs:35-45`

Unlocked COUNT at READ COMMITTED; disjoint row updates never conflict. Both commit → zero active super admins, and `bootstrap_admin` refuses to repair because the rows still exist (inactive, not deleted). Requires manual SQL.

**Fix:** `pg_advisory_xact_lock` on a fixed key before counting (as bootstrap already does), or `SELECT ... FOR UPDATE`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M10. Re-provisioning an employee portal account can destroy the old login with no replacement

`MEDIUM` · OPEN

**Where:** `backend/src/services/employee_service.rs:54,127-129,148-164` · `backend/src/services/user_service.rs:212`

Delete-then-insert as five autocommit statements spanning a ~300 ms bcrypt. Any failure in the window leaves the employee with no account and no API path to recreate one (`create_user` refuses employee-role creation). `create_employee` compounds it by committing the `employees` row first, so retry is then blocked by the `employee_number` uniqueness check.

**Fix:** one transaction spanning `employees::insert`, account provisioning, and `initialize_leave_balances`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M11. Payroll run holds its write transaction while acquiring a second pool connection → 10 concurrent runs deadlock the whole API

`MEDIUM` · OPEN

**Where:** `backend/src/services/payroll_engine.rs:142,296-297,633,672` · `backend/src/core/db.rs:6` · `approval_service/leave.rs:123-130` also

`gather_run_inputs` calls `StatutoryTables::load` and five `get_setting` calls against `&PgPool` while the caller holds a connection. Every in-flight run needs 2 of 10 connections; ten overlapping tx-holders all block on connection 11 for the 30 s acquire timeout, roll back runs that had already computed every payslip, and starve every unrelated request in the window.

**Fix:** load the snapshot before `pool.begin()`, or make `StatutoryTables::load`/`load_overtime_settings`/`get_setting` executor-generic and pass `&mut *tx`; drop the `pool` parameter.

<sub>No file in this finding changed during the audit.</sub>

### R1-M12. bcrypt runs inline on Tokio workers at six sites, violating the module's own documented rule

`MEDIUM` · OPEN

**Where:** `backend/src/services/auth_service.rs:65,202,209` · `password_reset_service.rs:63` · `employee_service.rs:148` · `backup_service/import.rs:63` · `totp_service.rs` (once per employee, inside an open transaction) backup-code loop

On the 1-2 vCPU Lightsail host each cost-12 verify pins a worker for 250-400 ms with no yield. Twenty 09:00 logins serialize to ~6 s and stall attendance check-in, kiosk QR refresh, and the health probe. The 2FA backup-code loop walks ten cost-10 hashes (~1 s CPU) per wrong guess, so ~3.3 req/s from a small IP pool saturates the executor.

**Fix:** route every bcrypt call through `spawn_blocking` (add a `verify_password` helper mirroring `hash_password`); move the whole backup-code loop into one closure.

<sub>No file in this finding changed during the audit.</sub>

### R1-M13. TOTP codes are replayable within the skew window

`MEDIUM` · OPEN

**Where:** `backend/src/services/totp_service.rs:160` · `migrations/1002_totp_2fa.sql`

No consumed-step column and nothing records prior acceptances, violating RFC 6238 §5.2. A code observed once (phishing proxy, shoulder-surf) plus the password yields a full session up to ~60 s later.

**Fix:** add `last_used_step bigint` and gate acceptance on an atomic `UPDATE ... WHERE last_used_step IS NULL OR last_used_step < $step`.

<sub>No file in this finding changed during the audit.</sub>

### R1-M14. 2FA backup codes have a 32-bit keyspace with no per-account attempt limit

`MEDIUM` · OPEN

**Where:** `backend/src/services/totp_service.rs:37-44,164-169` · `backend/src/routes/mod.rs:26-31`

Codes are the first 4 bytes of a UUIDv4; ten are live at once; the only throttle is 5/min per IP. A 5,000-address botnet sustains ~3.6e7 guesses/day → ~8% chance of full 2FA bypass in a month. The repo's own kiosk secret is 244 bits.

**Fix:** ≥80 bits of CSPRNG output rendered base32 as `XXXXX-XXXXX`, plus a per-account failure counter and lockout.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M15. Audit-log date filters are off by 8 hours

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/audit_logs.rs:30-31` · `backend/src/repositories/reads/audit.rs:38-39` · `backend/src/core/db.rs:4-10`

`timestamptz` compared against bare `::date` bounds resolved in the session TimeZone (UTC — never set). Filtering 1 July silently drops 00:00-07:59 MYT of 1 July and includes 00:00-07:59 MYT of 2 July. Count and page agree with each other and disagree with reality — a compliance failure.

**Fix:** thread the company timezone and bound the raw column, as `reads/attendance.rs` does.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M16. Attendance CSV export formats times at a hardcoded UTC+8 despite filtering in the company timezone

`MEDIUM` · OPEN

**Where:** `backend/src/services/attendance_service.rs:1141,1147` (`local_offset()` at :36)

An `Asia/Jakarta` tenant's 31 Jul 23:30 WIB check-in is correctly returned for `date_to=2026-07-31` but written as `2026-08-01 ... 00:30:00` — a row dated outside the requested range, inconsistent with the on-screen table. Wrong for part of every year for the DST zones in the shipped dropdown (Europe/London, Australia/Sydney). The inline comment claims the opposite.

**Fix:** parse `tz` into `chrono_tz::Tz` and `with_timezone(&tz)`; delete `local_offset()` from any path that has resolved a company timezone.

<sub>No file in this finding changed during the audit.</sub>

### R1-M17. One-sided export date range bypasses the month default and materializes the tenant's whole history

`MEDIUM` · OPEN

**Where:** `backend/src/services/attendance_service.rs:1124` · `backend/src/repositories/reads/attendance.rs:284-299` · `frontend/src/pages/attendance/AttendancePage.tsx:1157-1163`

The default applies only when *both* bounds are absent — filling one date field in the admin UI is enough. `export_rows` has no LIMIT; 400k rows become a `Vec` and then one in-memory `String` before any bytes ship.

**Fix:** default each bound independently, cap the span, and stream the CSV body.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M18. Geofence enforcement fails open when the last active location is removed

`MEDIUM` · OPEN

**Where:** `backend/src/services/geofence_service.rs:232-239`

With `geofence_mode = 'enforce'` and zero active locations, a check-in from anywhere is accepted and stored `is_outside_geofence = false`, while an on-site employee who denies GPS is still hard-rejected. Nothing guards deleting/deactivating the last location.

**Fix:** fail closed (or at least flag `is_within = false`) when mode ≠ `none` and the list is empty; block removing the last active location while enforcing.

<sub>No file in this finding changed during the audit.</sub>

### R1-M19. Duplicate auto-absent placeholders from overlapping runs

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/attendance_records.rs:361,427-432` · `migrations/1000_schema.sql:1631`

`NOT EXISTS` with no unique index or lock. A double-clicked `POST /api/attendance/absent-run`, or the daily catch-up racing a manual backfill, inserts two full sets of placeholders — visible twice in the list and CSV, contradicting the documented idempotence.

**Fix:** partial unique index on `(employee_id, local_day)` for cron-marker rows + `ON CONFLICT DO NOTHING`, or a per-(company, date) advisory lock.

<sub>No file in this finding changed during the audit.</sub>

### R1-M20. Attendance correction over ~41.7 days aborts with an opaque 500

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/attendance_records.rs:300`

Derived hours overflow `numeric(5,2)` → SQLSTATE 22003, which the 23505-only mapping at `attendance_service.rs:890` misses; the correction and its audit row roll back behind a 500. Reachable exactly in the documented "close a stale open session" workflow.

**Fix:** reject implausible spans with a 400 and/or widen the column.

<sub>No file in this finding changed during the audit.</sub>

### R1-M21. Un-rate-limited leave submission can issue ~10,000 sequential queries from one small POST

`MEDIUM` · OPEN

**Where:** `backend/src/services/portal_service.rs:53` · `backend/src/services/approval_service/leave.rs:34,129` `calendar_service::count_working_days_between`

`count_working_days_between` runs on raw client dates *before* `validate_period`, which never bounds the span. `start_date: 0001-01-01, end_date: 9999-12-31` → 9,999 sequential holiday queries holding a pool connection plus ~3.65M loop iterations, for work the rejection then discards. A handful of concurrent requests exhausts the 10-connection pool.

**Fix:** bound the span in `validate_period` and call it first; replace the per-year loop with one `WHERE date BETWEEN` query.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M22. Year-end carry-forward is ~8,000 unbatched, untransacted round trips

`MEDIUM` · OPEN

**Where:** `backend/src/services/portal_service.rs:564-591`

500 employees × 8 leave types × 2 queries, each its own implicit transaction, on an un-rate-limited endpoint. A drop mid-loop leaves half the company carried forward with no progress report; re-running overwrites recomputed values rather than resuming.

**Fix:** one set-based `INSERT ... SELECT` over `employees × leave_types` inside a single transaction.

<sub>No file in this finding changed during the audit.</sub>

### R1-M23. ICS calendar SSRF: arbitrary URL fetch, no timeout, no size cap

`MEDIUM` · OPEN

**Where:** `backend/src/services/calendar_service.rs:243-248` · `backend/src/handlers/calendar.rs:136`

Any `ManageCalendar` holder posts `http://169.254.169.254/...` or internal `10.x:5432`; error text distinguishes open from closed ports (blind internal scanning from inside the VPS). Default `reqwest::Client` has no timeout and `response.text()` buffers unbounded → OOM. VEVENT-shaped response text is persisted and readable back via `/api/calendar/holidays`.

**Fix:** https-only host allow-list (or reject loopback/link-local/RFC1918 after resolution, redirects disabled), explicit timeout, streamed body with a hard byte cap.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M24. Bulk actions swallow partial failures and leave the table showing stale statuses

`MEDIUM` · OPEN

**Where:** `frontend/src/pages/approvals/Approvals.tsx:324` · `frontend/src/App.tsx:105` portal Leave/Claims/Overtime pages

`Promise.all` with only `onSuccess`: one 409 skips `invalidateQueries` and selection reset entirely, no error surfaces anywhere (no QueryClient mutation error handler). Six cancelled rows still render "pending" and the admin re-issues cancels against them.

**Fix:** `Promise.allSettled`, invalidate in `onSettled`, report per-id failures, add `onError`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M25. Hand-rolled CSV export corrupts rows containing commas or quotes

`MEDIUM` · OPEN

**Where:** `frontend/src/pages/reports/Reports.tsx:241-243,409,467,494`

Bare comma joins with only `employee_name` pre-quoted (unescaped). Department "Sales, EMEA" shifts every subsequent column of that row; `Ali "Bob" Chan` produces a malformed record. Formula injection is unhandled.

**Fix:** one escape helper applied to every cell (`"` doubling, quote when `[",\n\r]`), plus an apostrophe prefix for `= + - @`.

<sub>No file in this finding changed during the audit.</sub>

### R1-M26. 401 interceptor destroys legitimate error states and force-logs-out on wrong-code entry

`MEDIUM` · OPEN

**Where:** `frontend/src/api/client.ts:89` · `frontend/src/components/TwoFactorSetup.tsx:48`

`PRIMARY_AUTH_ENDPOINTS` omits endpoints that 401 for a *content* reason. One mistyped TOTP digit during 2FA enrolment triggers a pointless refresh, a deterministic second 401, then a session clear and hard navigation to `/login` — the page unloads before "Invalid code" renders, the enrolment secret is abandoned, and AuthProvider bounces the user to the dashboard. `/attendance/check-in/face-id` additionally replays a WebAuthn assertion against a consumed challenge and shows a misleading error.

**Fix:** add `/auth/2fa/setup/confirm`, `/auth/2fa/disable`, `/auth/2fa/backup-codes/regenerate`, `/attendance/check-in/face-id` to the exemption list — better, have the backend mark session-expiry 401s explicitly and refresh only on that marker.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M27. Kiosk with a fast clock enters a permanent mint/429 loop

`MEDIUM` · OPEN

**Where:** `frontend/src/pages/attendance/AttendanceKioskPublic.tsx:51,120-126` · `AttendancePage.tsx:486` cf

The expiry effect depends on the `status` object whose identity changes on every mint, with a 0 ms refetch delay. A tablet >300 s ahead of the server reads every fresh token as expired → continuous `POST /kiosk/qr` until 429, backoff, repeat. The QR never renders and the shared per-IP bucket is drained for every kiosk behind that address. The admin console copy guards this with a 500 ms delay.

**Fix:** drive the countdown from `ttl_seconds` against a monotonic local start time; floor the refetch delay and track the last mint so two mints cannot occur within one TTL.

<sub>No file in this finding changed during the audit.</sub>

### R1-M28. `DataTable` never clamps its client-side page when data shrinks

`MEDIUM` · OPEN

**Where:** `frontend/src/components/ui/DataTable.tsx:77,86-88,344` PayrollList.tsx:190 Approvals.tsx portal/Leave.tsx:304 Reports.tsx

Filtering while on page 3 leaves `currentPage` past the end; the slice returns nothing *and* the `totalPages > 1` guard unmounts the pager, so the user sees "no results" for a filter that matches rows with no control to get back. Only EmployeeList escapes it, via a hand-rolled `setPage(1)`.

**Fix:** clamp to `Math.min(currentPage, Math.max(1, totalPages))` before slicing, plus a reset effect on `data` change.

<sub>No file in this finding changed during the audit.</sub>

### R1-M29. No React error boundary anywhere → any render throw blanks the whole SPA

`MEDIUM` · OPEN

**Where:** `frontend/src/main.tsx:6` · `frontend/src/App.tsx`

Vite emits content-hashed chunks and `deploy.yml` syncs with `--delete`; CloudFront's 404→200 rule returns index.html for the removed chunk. A user with the app open who navigates after a deploy gets a rejected dynamic import re-thrown during render and an unmounted root — blank white page, recoverable only by manual refresh.

**Fix:** wrap `<App />` in a boundary with a recoverable fallback; on dynamic-import failure `window.location.reload()` once.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-M30. Migration 1006 adds a duplicate index under a false comment

`MEDIUM` · OPEN

**Where:** `backend/migrations/1006_payroll_generation_rework.sql:16` · `migrations/1000_schema.sql:2016,2989-3010`

`idx_payroll_item_details_item` is definitionally identical to the baseline `idx_payroll_item_details`; `IF NOT EXISTS` matches names, not definitions. Every deployed DB maintains two identical btrees on the payroll bulk-insert path (~5,000 rows per 500-employee run), and the comment claims the column was previously unindexed.

**Fix:** drop the redundant index in a new migration and correct the comment.

<sub>No file in this finding changed during the audit.</sub>

### R1-M31. `notifications` limit is unclamped — 500 on negative, unbounded dump on large

`MEDIUM` · OPEN

**Where:** `backend/src/handlers/notification.rs:22` · `backend/src/models/notification.rs:29`

The only list endpoint with no bound. `?limit=-1` → Postgres 2201W → 500 instead of 400; `?limit=100000000` loads the caller's entire never-purged history into memory, repeatedly, on an unrate-limited route.

**Fix:** `q.limit.unwrap_or(50).clamp(1, 100)`.

<sub>No file in this finding changed during the audit.</sub>

### R2-M1. Out-of-order runs freeze a wrong YTD into payslips and skew PCB annualisation

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/reads/payroll.rs:342` · `payroll_engine.rs:613-668,1069` · `payslip_pdf_service.rs:389`

Nothing blocks creating March before February. `payroll_ytd` snapshots an incomplete YTD that is frozen into `payroll_items.ytd_*` and printed, and `remaining_months` assumes all earlier months are inside it. Processing February later recomputes nothing. The mirror-image guards already exist (`employee_has_later_run`, `run_has_later_committed_run`).

**Fix:** add a period-ordering check against the latest committed period at run creation, or derive YTD at read time instead of freezing it.

<sub>No file in this finding changed during the audit.</sub>

### R2-M2. EPF Third-Schedule part never re-derived from age or residency

`MEDIUM` · OPEN

**Where:** `backend/src/services/epf_service.rs:29-33` · `payroll_engine.rs:882-884,993` · `1000_schema.sql:542`

SOCSO and EIS both take `(wage, age, is_foreigner)` and auto-switch at 60; EPF takes only the static `epf_category` column (default 'A'). A citizen who turns 60 keeps full Part A deductions until HR manually edits the dropdown. No constraint, no diagnostic, no test.

**Fix:** derive the part from age and residency inside the service with the column as an explicit override, and warn in preview when they disagree.

<sub>No file in this finding changed during the audit.</sub>

### R2-M3. EPF relief cap read from the `life_insurance` key

`MEDIUM` · OPEN

**Where:** `backend/src/services/pcb_calculator.rs:17,108` · `1001_data.sql:326` · `statutory_tables.rs:117`

The dedicated `epf_additional` = 400000 row has no reader anywhere. A rule set keyed the way LHDN splits reliefs caps EPF relief RM1,000 low; omitting the non-mandatory `life_insurance` key aborts every employee with a misleading "missing relief" error. Latent behind the PCB gate.

**Fix:** look the EPF cap up under its own relief type and reconcile the docstring.

<sub>No file in this finding changed during the audit.</sub>

### R2-M4. PCB bracket gap or above-ceiling income silently under-withholds

`MEDIUM` · OPEN

**Where:** `backend/src/services/pcb_calculator.rs:144-175` · `1000_schema.sql:2944-2951,3399`

`calculate_tax_from_brackets` validates only non-emptiness; an uncovered income exits the loop holding the last lower band's tax and returns it. The GiST constraint prevents overlap only, never gaps. Sibling lookups fail closed (`epf_band`) or clamp (`socso_band`/`eis_band`). Lands on whoever hand-installs the verified LHDN schedule.

**Fix:** track whether a band matched and error naming the uncovered amount; or validate contiguity and open-endedness when the snapshot loads.

<sub>No file in this finding changed during the audit.</sub>

### R2-M5. `is_taxable = false` earnings still enter gross and all four statutory bases

`MEDIUM` · OPEN

**Where:** `backend/src/services/payroll_engine.rs:989,1014` · `repositories/reads/payroll.rs:26-30,54,81,113-118` · `PayslipBreakdownDrawer.tsx:115`

The flag is honoured by the breakdown line reads but not the aggregations, so a line rendered with a "Non-taxable" badge is taxed. Inconsistent with the engine's own claims handling. Latent (no write path outside restore, PCB gated).

**Fix:** add a taxable-earnings aggregate alongside gross and feed it to `PcbInput.monthly_gross`, or stop persisting and rendering the flag.

<sub>No file in this finding changed during the audit.</sub>

### R2-M6. EA picker hides soft-deleted employees, blocking a statutory form

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/reads/ea_form.rs:30` · `reads/payslip.rs:34-38` · `Reports.tsx:821` (vs `:98`, `employee_for_ea`, )

Only the picker query filters `deleted_at IS NULL`. A leaver removed after being paid keeps full payroll history and payslips but cannot be selected, so the EA form due end-February cannot be produced through the UI — the backend would generate it fine given the id.

**Fix:** drop the filter (rows are already restricted to employees with paid items) or add an explicit "include former employees" mode; make the three EA reads agree.

<sub>No file in this finding changed during the audit.</sub>

### R2-M7. Attendance CSV export renders through a hardcoded UTC+8

`MEDIUM` · OPEN

**Where:** `backend/src/services/attendance_service.rs:36-38,1141,1147` · `WorkScheduleCard.tsx:7-22`

The SQL bounds are timezone-threaded but the rendered Date/Check In/Check Out are not. Asia/Tokyo skews 1h and pushes boundary rows onto a date outside the filtered range; America/Los_Angeles is wrong on nearly every row. Contradicts the CLAUDE.md invariant and the function's own comment.

**Fix:** parse the resolved `tz` into `chrono_tz::Tz` and use `with_timezone(&tz)`, deleting `local_offset()`.

<sub>No file in this finding changed during the audit.</sub>

### R2-M8. Revoked passkey still authenticates for the challenge TTL

`MEDIUM` · OPEN

**Where:** `backend/src/handlers/passkey.rs:135` · `services/attendance_service.rs:326` · `1000_schema.sql:752`

Both ceremonies verify against the `Vec<Passkey>` snapshot in `state_json`; the follow-up loop is a counter update that no-ops when the credential is gone. Deleting a passkey on a stolen laptop does not stop an in-flight assertion from minting a full session — or a biometric-attested attendance record — up to 5 minutes later. The discoverable path re-reads from the DB and is safe.

**Fix:** require `auth_result.cred_id()` to still exist in `passkey_credentials` (fail closed on a miss), and delete outstanding challenges in the same transaction as `delete_passkey`.

<sub>No file in this finding changed during the audit.</sub>

### R2-M9. Public passkey routes unthrottled — account-existence oracle and self-amplifying DoS

`MEDIUM` · OPEN

**Where:** `backend/src/routes/mod.rs:193-210` · `handlers/passkey.rs:90,279` · `repositories/reads/passkey.rs:15-19` · `repositories/passkey_challenges.rs:10` · `1000_schema.sql:1960,1964`

Five routes merged bare into `api` while login/forgot-password/OAuth/kiosk all get a `GovernorLayer`. `/passkey/check` returns a clean true/false per email, and `/authenticate/begin` gives a distinguishable 400 — a full-speed enumeration oracle against a system holding salary, IC and bank data, defeating the password path's deliberate uniform error. Separately, `/discoverable/begin` needs no body and each call runs an unindexed `DELETE ... WHERE expires_at < NOW()`, so cost per request grows with the backlog the attacker just created, against a 10-connection pool.

**Fix:** move the five routes into a rate-limited sub-router, make `authenticate/begin` indistinguishable for unknown emails (or drop it for the discoverable flow), index `expires_at`, and sweep challenges from the daily cleanup task.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-M10. Overwrite restore silently reinstates logins it should revoke

`MEDIUM` · OPEN

**Where:** `backend/src/services/backup_service/import.rs:230` · `repositories/backup.rs:544` · `services/auth_service.rs:44-52` · `1000_schema.sql:3473-3477`

`delete_company_cascade` hard-deletes employees; the composite FK nulls `users.employee_id` instead of removing the account, and `linked_employee_active` treats NULL as active. Anyone hired after the backup date — and any leaver blocked solely by that guard — keeps `is_active`, company membership and a working login, with no warning in `ImportResult`.

**Fix:** in the same transaction, deactivate `users` rows for the company that still have `employee_id IS NULL` and roles `= ['employee']`, and report the count.

<sub>No file in this finding changed during the audit.</sub>

### R2-M11. Backup omits work schedule, timezone and geofence mode

`MEDIUM` · OPEN

**Where:** `backend/src/models/backup.rs:62` · `repositories/backup.rs:22-26,601-604` · `import.rs:415` · `1000_schema.sql:261-292`

Restore-as-new silently installs 09:00-18:00 / grace 15 / half-day 4.0 / Asia/Kuala_Lumpur and `geofence_mode = 'none'`. A restored Jakarta tenant mis-buckets local days, misclassifies lateness, runs auto-absent an hour early, and accepts off-site check-ins it previously blocked — reported as a clean success.

**Fix:** export/import `attendance_method`, `timezone`, `geofence_mode`, `company_work_schedules` and `company_locations`; surface anything genuinely unrestorable as an explicit warning.

<sub>No file in this finding changed during the audit.</sub>

### R2-M12. Restore stamps one timestamp on every row, breaking every `ORDER BY created_at ... LIMIT`

`MEDIUM` · OPEN

**Where:** `backend/src/services/backup_service/import.rs:223` · `repositories/backup.rs:906-907,940-941` · `leave_requests.rs:314-315` · `claims.rs:300` · `reads/approvals.rs:35,101,148` · `payroll_items.rs:116`

The `*Export` structs carry source timestamps; no `insert_*` binds them. 120 tied leave requests under a `LIMIT 50` return an arbitrary subset in arbitrary order with no pagination to recover the rest; claims end up with `submitted_at` years before `created_at`.

**Fix:** bind the archive's `created_at`/`updated_at`, and add `, id DESC` as a tiebreaker to the affected list queries.

<sub>No file in this finding changed during the audit.</sub>

### R2-M13. Company delete cascade omits `audit_logs` — endpoint 500s for every non-empty tenant

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/companies.rs:216-255` · `1000_schema.sql:2233-2234`

`audit_logs_company_id_fkey` has no `ON DELETE` action and the table is missing from the cascade list, so the final DELETE raises 23503 for any company that ever produced an audit row (one settings change suffices). Returns a bare 500. `tp3_records` is a second failure point.

**Fix:** decide the retention rule explicitly (null the column or archive the trail), add the missing tables, and assert the list against `information_schema` in a schema-invariant test.

<sub>No file in this finding changed during the audit.</sub>

### R2-M14. User privilege-change audit rows filed under NULL or the wrong tenant

`MEDIUM` · OPEN

**Where:** `backend/src/services/user_service.rs:436,495` · `repositories/reads/audit.rs:34` · `repositories/companies.rs:246-252`

Both pass the target's nullable `existing.company_id`. `delete_cascade` nulls that column for users homed in a deleted tenant, so a subsequent promotion or deactivation is written with `company_id = NULL` and returned by no API to anyone. Even when non-NULL, the row is stamped with the *pre-change* company, so the tenant that just gained an administrator sees nothing.

**Fix:** stamp the actor's active company and record the target's tenant in `new_values`.

<sub>No file in this finding changed during the audit.</sub>

### R2-M15. Whole-tenant backup export and destructive import write no audit row

`MEDIUM` · OPEN

**Where:** `backend/src/handlers/backup.rs:31,46,73,163-164` · `repositories/backup.rs:468-548` `services/backup_service/*`

Export streams every employee's IC, passport, bank account and TIN as a download; import runs `delete_company_cascade` over ~22 tables. Zero `audit_service` calls anywhere in the module and no compensating middleware, while the rest of the app audits far smaller actions at 87 sites. A breach investigation cannot establish that an export happened, by whom, from what IP, or against which `?company_id=`.

**Fix:** log both with `log_action_with_metadata` (record counts on export; `is_overwrite`, source metadata and counts on import), writing the import row inside its transaction.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-M16. Employee deletion is unaudited and non-transactional

`MEDIUM` · **FIXED**

**Where:** `backend/src/services/employee_service.rs:227-240` · `handlers/employee.rs:285-292` · `user_service.rs:456,493` (contrast `:167,:247` and )

The most destructive employee operation — soft-delete plus irreversible removal of `user_companies`, refresh tokens and the `users` row — takes neither an actor id nor `AuditRequestMeta`. The trail shows the hire and every edit but nothing for the deletion. The four statements also run on `&PgPool`, so a mid-sequence failure leaves a soft-deleted employee with live refresh tokens.

**Fix:** thread `deleted_by` and `audit_meta` through, wrap the four statements in one transaction, and insert an `employee`/`delete` row carrying the pre-delete snapshot.

<sub>Fixed by a commit that landed while this audit was running; re-verified against `681b767`.</sub>

### R2-M17. Self-service credential mutations are entirely unaudited

`MEDIUM` · OPEN

**Where:** `backend/src/handlers/totp.rs:45` · `handlers/passkey.rs:213-276` · `handlers/oauth2.rs:271-276` · `services/oauth2_service.rs:311,326` · `handlers/auth.rs:181-203` · `services/auth_service.rs:192-221`

Passkey register/rename/delete, Google link/unlink, session revocation and password change write nothing. An attacker on a stolen JWT can bind their own Google identity — a permanent password-less login path — and delete every passkey with no record. (TOTP disable and backup-code regeneration require a password, contrary to the original claim.)

**Fix:** audit each as entity `user_credential` with non-secret identifiers only (credential id, provider, session id).

<sub>No file in this finding changed during the audit.</sub>

### R2-M18. Approval notifications miss admins who switched company, and never reach `admin`

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/notifications.rs:128-129` · `services/user_service.rs:38-41` · `repositories/users.rs:285` · `core/permission.rs:285-286` · `services/portal_service.rs:114,254,400`

The fan-out selects on `users.company_id` — the *active* company, rewritten by switch-company — rather than `user_companies`, and against a hardcoded three-role list that omits `admin` and `exec`, both of which hold `ManageApprovals`. A company whose only HR manager is working in another tenant, or which is staffed by `admin` approvers, generates zero notifications; callers discard the `Ok(())` with `let _`, so the empty fan-out is never logged. Loss is permanent — fan-out is write-time and there is no email fallback.

**Fix:** join `user_companies` (inserting `$1` as the notification's `company_id`), derive recipients from `ManageApprovals` via `role_permissions`, and log a zero-recipient fan-out.

<sub>No file in this finding changed during the audit.</sub>

### R2-M19. Notification reads ignore company scope

`MEDIUM` · OPEN

**Where:** `backend/src/repositories/notifications.rs:16,36,49` · `services/user_service.rs:152-156,417-427` · `handlers/notification.rs:18`

Filtered on `user_id` alone. Removing a multi-company user's membership clears only sessions and tokens, so `GET /api/notifications` keeps returning the ex-tenant's rows — named employees' leave and claim details — indefinitely, and the unread count folds them into the current company's badge.

**Fix:** thread the active `company_id` claim into all three reads and into `mark_all_read`.

<sub>No file in this finding changed during the audit.</sub>

### R2-M20. `email_logs` rows orphaned as `pending` on every pre-transport failure

`MEDIUM` · OPEN

**Where:** `backend/src/services/email_service.rs:113,150,154,158,163` · `handlers/email.rs:303-307` · `handlers/employee.rs:218-240` · `LettersPage.tsx:161`

The row is committed as `pending` before four `Err` paths that never call `mark_failed`. No email-format validation exists anywhere on the backend, so any malformed recipient triggers it; the welcome-email path runs in a detached `tokio::spawn` that swallows the error, giving HR a 200 and a permanently yellow badge. No CHECK constraint, no retry, no reconciler.

**Fix:** validate addresses and build the transport before `insert_pending`, or call `mark_failed` on every error path after it.

<sub>No file in this finding changed during the audit.</sub>

### R2-M21. Frontend reports a failed letter send as success

`MEDIUM` · OPEN

**Where:** `frontend/src/pages/letters/LettersPage.tsx:123-134,522,561-581` · `backend/src/services/email_service.rs:129-131,182-184` · `handlers/email.rs:297-300,345-348`

The API returns 200 with `status: "failed"`; the mutation treats any 200 as success, closes the modal and clears the form. (The green banner and `isSuccess`-disabled button are unreachable dead code — the modal unmounts in the same batched update.) The only contradicting signal is a red badge on another tab.

**Fix:** return a non-2xx when `log.status != "sent"`, and branch on `sendMutation.data?.status`, surfacing `error_message`.

<sub>No file in this finding changed during the audit.</sub>

### R2-M22. Group-granted permissions work over the API but are unreachable in the UI

`MEDIUM` · OPEN

**Where:** `backend/src/models/user.rs:83,99-109` · `core/auth.rs:286` · `routes/mod.rs:173` · `frontend/src/lib/usePermissions.ts:9-11,19,44` · `Sidebar.tsx:85` · `App.tsx:92` · `frontend/src/api/permissions.ts:76`

`UserResponse.permissions` is role-derived only, while `AuthUser::can` is roles ∪ group grants. The one group-aware source is dead code — only `/auth/permissions/matrix` is registered. Every one of the 36 grantable permissions is silently unusable through the admin UI: sidebar hidden, route redirects to `/403`. Fails closed, so broken feature rather than escalation.

**Fix:** build the session permission list from the same union `AuthUser::permissions()` uses, or implement `GET /auth/permissions` and merge it in `AuthProvider`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-M23. Settings read path has no permission gate

`MEDIUM` · OPEN

**Where:** `backend/src/handlers/settings.rs:18,39` · `routes/mod.rs:335-339` · `tests/route_auth_tests.rs:299` (vs `:65,:95`)

`list` and `get` require only a `company_id` claim, so a sole-`employee` portal token reads the tenant's non-payroll settings. Impact is bounded — both handlers already filter `payroll`/`statutory` for non-privileged callers, leaving low-sensitivity config — but it violates the explicit CLAUDE.md rule that every handler carry a role check, and only the PUT path is tested.

**Fix:** add an explicit read gate to both handlers.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-M24. No integration test ever commits two runs for the same employee in one year

`MEDIUM` · OPEN

**Where:** `backend/src/tests/mod.rs:6` · `repositories/reads/payroll.rs:323-352` · `payroll_engine.rs:1069-1072`

`payroll_ytd` is only ever exercised returning empty, and the persisted `ytd_*` columns are never read back. Deleting `months_worked` from `PcbInput` (pinning `remaining_months` to 12) passes the entire suite; so does adding `'cancelled'` to the status allow-list or flipping `<` to `<=`.

**Fix:** add a test that processes 2024-01 then 2024-02 for one employee and asserts February's stored `ytd_gross`/`ytd_pcb` equal January's committed figures, that February's PCB matches `months_worked: 2`, and that cancelling January removes it.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-M25. Nothing tests the WebAuthn gate on Face ID check-in

`MEDIUM` · OPEN

**Where:** `backend/src/tests/attendance_tests.rs:338` · `handlers/attendance.rs:232-239` · `services/attendance_service.rs:587-620`

`check_in_face_id` takes no credential argument; the entire biometric gate is nine lines in the handler that no test exercises. Deleting them leaves `cargo test` green, and an employee could mint a record claiming verified biometric presence from a laptop. Nothing asserts `consume_for_user` rejects another user's challenge. The route-test harness already exists.

**Fix:** add route tests asserting a bogus `challenge_id` yields 400/401 with zero rows inserted, and that user A's challenge is rejected when replayed by user B.

<sub>No file in this finding changed during the audit.</sub>

### R2-M26. The only YTD-PCB offset test cannot fail

`MEDIUM` · OPEN

**Where:** `backend/src/tests/statutory_tests.rs:407,413` · `services/pcb_calculator.rs:73,216-219`

Non-strict `<=` on 12,300 vs 19,500 stays green when `- input.ytd_pcb` is deleted (both sides become 19,500) — the regression that would over-deduct every employee their full year-to-date PCB. The companion `>= 0` is a tautology given the clamp.

**Fix:** `assert_eq!` both figures and pin the 7,200 spread; delete or replace the tautology.

<sub>No file in this finding changed during the audit.</sub>

---

## Low (33)

Bounded impact, dead code, hardening gaps, and defects reachable only under unusual input.

### R1-L1. Five company-wide read handlers have no permission gate

`LOW` · OPEN

**Where:** `backend/src/handlers/dashboard.rs:9` · `handlers/company.rs:27,66` · `handlers/settings.rs:18,39`

A sole-`employee` token correctly 403'd by `GET /api/employees` still reads headcount, department breakdown, the company profile, and every non-payroll settings row.

**Fix:** gate on `ViewEmployees`/`ManageCompanySettings` via `auth.authorize(...)`, which returns the tenant id so scope cannot be written without the check.

<sub>No file in this finding changed during the audit.</sub>

### R1-L2. `Permission::ViewSalaryHistory` is enforced nowhere; revoking it is a silent no-op

`LOW` · OPEN

**Where:** `backend/src/core/permission.rs:37` · `backend/src/handlers/employee.rs:299`

The handler gates on `ViewPayroll`. Removing the grant updates the Role Management matrix and `usePermission` while the API keeps returning full salary history.

**Fix:** gate the handler on `ViewSalaryHistory` (identical roster today) or delete the variant; add a test asserting every `Permission::ALL` variant has an enforcement site.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-L3. `default_pay_date` panics on an out-of-range period

`LOW` · **FIXED**

**Where:** `backend/src/handlers/payroll.rs:29`

`period_month: 13` → `from_ymd_opt(...).unwrap()` panics before `RunPeriod::resolve` can return `BadRequest`. No `CatchPanicLayer` exists, so the client gets a dropped connection.

**Fix:** validate first or make `default_pay_date` fallible.

<sub>Fixed by a commit that landed while this audit was running; re-verified against `681b767`.</sub>

### R1-L4. EA-form download panics on an employee number containing a control character

`LOW` · OPEN

**Where:** `backend/src/handlers/report.rs:251-260`

`"EMP\n001"` (no validation forbids it) makes `HeaderValue` conversion fail and `.unwrap()` panic; that employee is permanently un-exportable.

**Fix:** sanitize the filename to `[A-Za-z0-9._-]` and build the response fallibly; validate `employee_number` on create.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-L5. `/api/health/ready` leaks raw Postgres errors and the schema revision to anonymous callers

`LOW` · OPEN

**Where:** `backend/src/handlers/health.rs:38-40` · `backend/src/core/error.rs:49-55`

Hand-serializes `AppError::to_string()`, bypassing the sanitizer: a degraded pool returns `password authentication failed for user "payroll"` or the internal host/port/role. The healthy response discloses the exact migration version. The `Ok((None, _))` branch is dead code.

**Fix:** log the error and return a fixed `"database unreachable"`; drop or gate the migration counts; delete the dead branch.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-L6. Unclamped pagination reaches SQL at seven sites (negative LIMIT/OFFSET → 500)

`LOW` · OPEN

**Where:** `backend/src/handlers/employee.rs:113` · `handlers/document.rs:29` · `handlers/email.rs:324,366` · `services/audit_service.rs:16` · `repositories/reads/attendance.rs:28` · `handlers/audit.rs:18`

`.min(100)` without `.max(1)` → `?per_page=-1` yields an opaque 500; `per_page=0` returns `{per_page: 0}` that clients divide by. `page` is unbounded above and release builds have no overflow checks, so a huge `page` wraps the offset negative. `handlers/audit.rs:18` additionally echoes the *unclamped* values in its envelope while the service clamps independently, so `?per_page=500&page=0` returns 100 rows under `{"page":0,"per_page":500}`.

**Fix:** one shared helper: `clamp(1, 100)` + `saturating_sub`/`saturating_mul` + a page ceiling; return the resolved values in the envelope.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-L7. `create_document` skips the ownership check every sibling write path performs

`LOW` · OPEN

**Where:** `backend/src/services/document_service.rs:47-55` · `backend/src/repositories/documents.rs:146`

A cross-company `employee_id` is caught only by a composite FK added `NOT VALID` on populated installs, surfacing as an opaque 500 instead of `NotFound`.

**Fix:** `employees::exists_in_company` plus a company-scoped `category_id` check before insert.

<sub>No file in this finding changed during the audit.</sub>

### R1-L8. Bulk-import audit rows are written with NULL `company_id` and are invisible to the tenant

`LOW` · **FIXED**

**Where:** `backend/src/repositories/audit_logs.rs:89` · `backend/src/services/employee_import_service/confirm.rs:148,195` · `reads/audit.rs:34`

Importing 200 employees — the product's largest bulk write — leaves no trace in `/api/audit-logs`, while a single manual create does.

**Fix:** pass `company_id` (already in scope) or use the general `audit_logs::insert`.

<sub>Fixed by a commit that landed while this audit was running; re-verified against `681b767`.</sub>

### R1-L9. Login timing oracle enumerates valid accounts

`LOW` · OPEN

**Where:** `backend/src/services/auth_service.rs:61-65`

Unknown email returns in ~3 ms; a real account pays the inline 250-400 ms bcrypt. Identical 401 bodies, ~300 ms apart — defeating the deliberate anti-enumeration wording in `forgot_password`.

**Fix:** verify against a fixed dummy hash on the unknown-email branch.

<sub>No file in this finding changed during the audit.</sub>

### R1-L10. `manual_attendance` returns 500 instead of 409 on the one-open-session constraint

`LOW` · OPEN

**Where:** `backend/src/services/attendance_service.rs:787` (cf. :890)

Backfilling an open record for an employee who already has one raises 23505 → opaque 500; the sibling update path translates it correctly.

**Fix:** mirror the 23505 → `Conflict` mapping.

<sub>No file in this finding changed during the audit.</sub>

### R1-L11. `entries_with_employee` is unpaginated with five nullable predicates

`LOW` · OPEN

**Where:** `backend/src/repositories/reads/payroll.rs:364-389` · `backend/src/handlers/payroll.rs` (`list_entries`)

`GET /api/payroll/entries?include_processed=true` full-scans and serializes every entry the tenant ever created (~54k rows for a 500-employee, 3-year company) — full scan plus external sort, hundreds of MB per request.

**Fix:** capped `limit`/`offset` + `PaginatedResponse`; require `period_year` when `include_processed`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-L12. Bulk payslip generation re-reads the same company row once per employee

`LOW` · OPEN

**Where:** `backend/src/services/payslip_pdf_service.rs:449-450`

A 500-employee run issues 500 identical company lookups plus 500 joins — 1,000 sequential round trips on one of ten pooled connections. Exactly the per-employee chatter the statutory-snapshot refactor removed from the engine.

**Fix:** hoist the company read; replace `payslip_for_run_item` with one batch read keyed on the run id.

<sub>No file in this finding changed during the audit.</sub>

### R1-L13. `user_sessions` has no retention path

`LOW` · OPEN

**Where:** `backend/migrations/1003_user_sessions.sql:4` · `backend/src/services/session_service.rs:19-38`

Rows are only ever soft-revoked; the daily cleanup purges `refresh_tokens` and QR tokens only. ~1.1M dead rows after three years for a 500-employee tenant.

**Fix:** extend the daily task with a 30-day delete, which cascades the child tokens.

<sub>No file in this finding changed during the audit.</sub>

### R1-L14. Salary-history percent change divides by zero for every bulk-imported employee

`LOW` · OPEN

**Where:** `frontend/src/pages/employees/EmployeeList.tsx:378` · `frontend/src/pages/employees/EmployeeDetail.tsx:152` · `backend/src/repositories/salary_history.rs:48`

The initial history row is written with `old_salary = 0`, so imported employees show a green up-arrow reading "+Infinity%" (or "NaN%").

**Fix:** guard the denominator and render an em dash / "Initial".

<sub>No file in this finding changed during the audit.</sub>

### R1-L15. Portal leave-balance year defaults to the literal `2026`

`LOW` · OPEN

**Where:** `backend/src/handlers/portal.rs:67` (cf. :342)

From 2027-01-01 any caller omitting `year` silently receives prior-year balances. Currently masked because the only shipped caller passes an explicit year.

**Fix:** `unwrap_or_else(|| Utc::now().year())`.

<sub>No file in this finding changed during the audit.</sub>

### R1-L16. ICS export does not escape RFC 5545 special characters

`LOW` · OPEN

**Where:** `backend/src/handlers/portal.rs:367` · `frontend/src/pages/portal/Leave.tsx:665`

The reason field is a `<textarea>`; a newline emits a bare continuation line inside VEVENT, so Google Calendar/Outlook reject the import or drop the event. Unescaped `,`/`;` split properties.

**Fix:** escape `\ ; , CR/LF` per §3.3.11 and fold at 75 octets.

<sub>No file in this finding changed during the audit.</sub>

### R1-L17. Attendance Records table renders times in the browser timezone while everything around it uses the company timezone

`LOW` · OPEN

**Where:** `frontend/src/pages/attendance/AttendancePage.tsx:74-81`

An off-zone reviewer filtering `date_from = date_to = 2026-07-27` gets rows labelled 26 July, and the StatsBar "today" tile disagrees with every date below it. Display-only — edits are by record id and aggregates are server-side.

**Fix:** pass `method?.timezone` as the `timeZone` option, defaulting to `Asia/Kuala_Lumpur`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R1-L18. SPA CSP is Report-Only with no collector

`LOW` · OPEN

**Where:** `infra/s3_cloudfront.tf:59-63,89`

The promotion gate ("flip it once reports are clean") has no telemetry source, so the XSS defense-in-depth layer will never be enabled. Hardening gap only — no reachable XSS sink today (no `dangerouslySetInnerHTML`/`eval`; the one employee-controlled HTML render at `LettersPage.tsx:553-558` is in a script-less sandboxed iframe).

**Fix:** add `report-to`/`Reporting-Endpoints`, or move the policy into `security_headers_config` and enforce now (the Vite build emits no inline scripts).

<sub>No file in this finding changed during the audit.</sub>

### R2-L1. EIS has no lower age bound

`LOW` · OPEN

**Where:** `backend/src/services/eis_service.rs:36,43` · `payroll_engine.rs:995,1301` · `1000_schema.sql:506` · `employee_import_service/validation.rs:40-54`

Only the 57/60 upper bounds are gated. A 16-year-old intern is charged 230/230 sen; a mistyped future DOB yields a negative age that also passes silently.

**Fix:** return zero below 18 and reject negative/implausible ages, mirroring the upper-bound handling.

<sub>No file in this finding changed during the audit.</sub>

### R2-L2. `epf_category` is the only enum-like employee field validated nowhere

`LOW` · OPEN

**Where:** `backend/src/services/employee_service.rs:38,178` · `employee_import_service/validation.rs:74-131` · `confirm.rs:68` · `template.rs:59-99` · `epf_service.rs:34` · `1000_schema.sql:542`

A lowercase `a` from a spreadsheet imports cleanly and weeks later aborts the entire group's run. Fails closed with named employees, so integrity nuisance rather than money.

**Fix:** add a shared A–E validator to the importer and create/update services, plus a CHECK constraint in a new migration.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-L3. EA form and payslip YTD disagree on which run statuses count

`LOW` · OPEN

**Where:** `backend/src/repositories/reads/ea_form.rs:98` · `reads/payroll.rs:343` vs

EA sums `approved`/`paid`; `payroll_ytd` also admits `processed`/`pending_approval`. Approving December while November sits `processed` makes the December payslip's YTD permanently include a month the EA form omits.

**Fix:** share one committed-run definition, or block EA generation for a year with non-approved runs.

<sub>No file in this finding changed during the audit.</sub>

### R2-L4. Payroll divisor settings accept negative and overflow-inducing values

`LOW` · OPEN

**Where:** `backend/src/services/payroll_engine.rs:819,926` · `services/settings_service.rs:67-93` · `handlers/settings.rs:58-87`

Guarded only for non-zero. `-26` yields negative overtime pay for every employee with a NULL `hourly_rate`; `1e-28` overflows `Decimal` division and panics (caught by `CatchPanicLayer` into a 500, transaction rolled back). A CHECK already exists for the sibling `companies_unpaid_leave_divisor_check`.

**Fix:** range-validate at write time and use `checked_div`, mapping `None` to a preview diagnostic.

<sub>No file in this finding changed during the audit.</sub>

### R2-L5. Kiosk revocation leaves a live QR for up to 300s

`LOW` · OPEN

**Where:** `backend/src/services/attendance_service.rs:244,435`

`revoke_kiosk_credential` stamps `revoked_at` but never retires the kiosk's outstanding tokens, and `validate_qr_token` never consults credential state. A screenshotted code from a stolen tablet keeps working for the rest of its TTL; an admin revoking a kiosk cannot actually stop check-ins for five minutes.

**Fix:** call the existing `attendance_qr_tokens::revoke_unused_for_issuer` in the same transaction.

<sub>No file in this finding changed during the audit.</sub>

### R2-L6. Passkey registration challenge is not bound to the user

`LOW` · OPEN

**Where:** `backend/src/handlers/passkey.rs:62-64` · `services/attendance_service.rs:319` (contrast )

Uses the unbound `consume` and deletes the row before verifying. Any authenticated user knowing another's `challenge_id` can burn their in-flight enrolment; a leaked begin-response lets them complete under their own token, producing a credential stored under their `user_id` but carrying the victim's WebAuthn user handle. No takeover — `save_passkey` always uses `auth.0.sub`.

**Fix:** use `consume_for_user(..., "registration", auth.0.sub)`.

<sub>No file in this finding changed during the audit.</sub>

### R2-L7. ICS `DTSTART` sliced by byte index — panics on non-ASCII

`LOW` · OPEN

**Where:** `backend/src/services/calendar_service.rs:308` · `handlers/calendar.rs:175`

`&val[..8]` guarded only by `len() >= 8`. `DTSTART:日日日` panics; `CatchPanicLayer` turns it into a 500 with a partially completed import.

**Fix:** use `val.get(..8)` and return `BadRequest` (or skip the event) on a malformed date.

<sub>No file in this finding changed during the audit.</sub>

### R2-L8. Pagination offset unclamped and multiplied without overflow check

`LOW` · **PARTIALLY FIXED**

**Where:** `backend/src/services/audit_service.rs:15-17` · `handlers/document.rs:29-30` · `handlers/admin.rs:79`

`per_page=-1` emits `LIMIT -1` (2201W); a huge `page` wraps to a negative or wrong offset in release builds. Neither SQLSTATE is handled by `classify_db_error`, so both surface as 500 instead of 400.

**Fix:** `.clamp(1, 100)` and `page.saturating_sub(1).saturating_mul(per_page)` everywhere.

<sub>Partially addressed by a commit that landed during the audit; see Appendix A for what remains.</sub>

### R2-L9. Restore reports success when attachment writes fail

`LOW` · OPEN

**Where:** `backend/src/services/backup_service/files.rs:22,56-63` · `import.rs:419-421`

Per-file errors are discarded and the warning is emitted only when zero files were written — after `tx.commit()`. A full or read-only volume yields 200 OK with committed rows whose `file_url`s all 404, and no log line anywhere in the module. The export side silently drops already-missing files.

**Fix:** collect and always report per-file failure counts (return `Vec<String>`), on both directions, and record expected file count in `BackupMetadata`.

<sub>No file in this finding changed during the audit.</sub>

### R2-L10. Calendar mutations unaudited

`LOW` · OPEN

**Where:** `backend/src/handlers/calendar.rs:39,60,83,104,136,149` · `repositories/holidays.rs:123` · `AuditTrailPage.tsx:26`

Holiday delete is an unconditional DELETE with no tombstone and no audit row, so a removed public holiday — which the auto-absent job skips — leaves no trace of who removed it. No backend code emits `entity_type = "holiday"`, making the UI filter dead.

**Fix:** audit create/update/delete/working-days/import with entities `holiday` and `working_day_config`.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-L11. `delete_unprocessed` discards `rows_affected`, so a no-op logs a delete

`LOW` · OPEN

**Where:** `backend/src/repositories/payroll_entries.rs:121-134` · `services/payroll_entry_service.rs:213,219` · `payroll_engine.rs:1183`

If a concurrent run flips `is_processed`, the DELETE matches nothing but the service writes a `delete` audit row with full `old_values` and returns 200 — the entry is still paid. Inconsistent with the sibling `update` and with `holidays.rs:117-131`.

**Fix:** return `rows_affected` and map 0 to `Conflict` before writing the audit row.

<sub>No file in this finding changed during the audit.</sub>

### R2-L12. UI write controls rendered without permission gates (5 pages, 4 roles affected)

`LOW` · OPEN

**Where:** `CompanyProfile.tsx:187-188,204,233,251` · `TeamsPage.tsx:167,258,264,278` · `CalendarPage.tsx:192,199,348,357,388` · `EmployeeList.tsx:110,119,154,164`

Guaranteed-403 dead ends: `hr_manager`/`payroll_admin`/`finance`/`exec` get editable Company cards (`manage_company_settings` is super_admin/admin only, and `/company` is their default landing page); `exec` gets Teams and Calendar write controls; `finance`/`exec` get Add/Edit/Delete Employee after filling the full form; `finance` sees Import and is redirected to `/403` because the button is gated on `canAccessPayrollData` rather than `import_employees`. Server enforces correctly — pure UX.

**Fix:** gate each on the matching `usePermission(...)` — the pattern `PayrollList`/`PayrollDetail` already use.

<sub>No file in this finding changed during the audit.</sub>

### R2-L13. `user_sessions` is never reaped

`LOW` · OPEN

**Where:** `backend/migrations/1003_user_sessions.sql:22,29-31` · `backend/src/main.rs:139-143` · `repositories/user_sessions.rs:54,64,76`

Every write path only sets `revoked_at`; the daily task deletes only `refresh_tokens`, and the `ON DELETE CASCADE` direction means that leaves parents orphaned permanently. ~100k rows/year for a 200-user tenant. No correctness or per-request cost (`is_active` probes the PK).

**Fix:** add a sessions-first delete to the daily task, ordered before the token delete.

<sub>File changed during the audit; re-opened against `681b767` and the defect is still present.</sub>

### R2-L14. Canonical PCB test docstring documents a derivation the code does not use

`LOW` · OPEN

**Where:** `backend/src/tests/statutory_tests.rs:230-233,336-350` · `services/pcb_calculator.rs:166` · `1001_data.sql:296`

The comment sums per-bracket slices to 135,971/11,330 while the code computes `cumulative_tax + partial` = 135,973/11,331. The assertion passes only because of whole-ringgit rounding — which also means the test cannot detect any annual error under ~RM12/year, and up to RM8.27 of `cumulative_tax` drift. The comment instructs maintainers to re-derive using the wrong method.

**Fix:** rewrite the derivation to `cumulative_tax + rate × (chargeable − bracket_from)` with the true intermediates, and assert an unrounded annual figure.

<sub>No file in this finding changed during the audit.</sub>

### R2-L15. Married/child relief test satisfied by equality

`LOW` · OPEN

**Where:** `backend/src/tests/statutory_tests.rs:324` · `services/pcb_calculator.rs:123-131`

`pcb_married <= pcb_single` passes if either relief branch is zeroed, and it is the only test in the repo exercising spouse or child relief. Real values are 7,400 vs 11,400 sen.

**Fix:** `assert_eq!` both.

<sub>No file in this finding changed during the audit.</sub>

---

## Systemic themes

Patterns worth fixing as a class rather than one site at a time. In almost every case the repository already contains a correct implementation of the same rule — the defect is that it was not reused.

### From pass 1

**T1. Client-supplied strings used as filesystem paths — 4 sinks, 0 validators** (`document_service.rs:54,80`; `backup_service/files.rs:21,55`; `portal_service` claim/leave URL writes). A correct guard already exists at `handlers/portal.rs:452` and was never applied to any write, read, or delete sink. *Single highest-leverage fix in the audit: one validator at write time plus one at each sink closes C5 entirely.*

**T2. Hardcoded `Asia/Kuala_Lumpur` / UTC+8 where a company timezone is already resolved — 6 instances** (`attendance_service.rs:36,1033,1141`; `reads/payroll.rs:208`; `audit_logs.rs:30`; `reads/audit.rs:38`; plus frontend `AttendancePage.tsx:79`). CLAUDE.md documents the opposite rule. Causes double-paid OT, wrong-month payroll, 8-hour audit gaps, future-dated absences, and non-sargable scans. **Also T2b: `company_work_schedules.timezone` has no validation at any layer** — an unrecognized value 500s the entire attendance subsystem for that tenant and aborts the platform-wide absent run.

**T3. Multi-statement writes outside a transaction — 7 instances** (`approval_service/leave.rs:433,554`; `employee_service.rs:54,127-129`; `session_service.rs:55`; `auth_service.rs:144`; `portal_service.rs:565`). Every one has a sibling that does it correctly in the same file or module. Failure mode is uniformly *silent, irrecoverable partial state* — stranded leave days, unpaid deductions, destroyed logins, orphaned refresh tokens.

**T4. Read-then-write races with no lock, predicate, or unique constraint — 6 instances** (`user_service.rs:394,476` last super admin; `portal_service.rs:72` + `leave.rs:49` leave overlap; `overtime_applications.rs:156,264` cancel; `attendance_records.rs:427` mark_absent; `refresh_tokens.rs:42` rotation). The correct idioms (`pg_advisory_xact_lock`, `AND status <> ...` + `rows_affected`) already exist elsewhere in the same repo.

**T5. Divergent duplicate implementations of one rule — 5 pairs.** Company cascade delete (`companies.rs` vs `backup.rs`), overtime rating (`overtime.rs` vs `payroll_engine.rs`), CSV field escaping (`attendance_service::csv_field` vs `statutory_export_service` vs `Reports.tsx`), pagination clamping (7 hand-rolled variants), salary-history percent (two copies of the same div-by-zero). Every pair has drifted; in four of five the *correct* implementation exists and was simply not reused.

**T6. Unclamped/unvalidated numeric input reaching SQL or arithmetic — 8 instances** (T-listed in L6, plus `notification.rs:22`). Uniform symptom: opaque 500 where a 400 belongs, plus unbounded memory on the upper end.

**T7. Free text interpolated into structured formats with no neutralization — 5 sinks** (`statutory_export_service` CSV + CP39 pipes; `Reports.tsx` CSV; `portal.rs` ICS; `report.rs:260` Content-Disposition). Two of these produce statutory filings; one panics the process.

**T8. bcrypt on the async runtime — 6 call sites**, against an explicit doc comment 30 lines above the first offender.

**T9. Handlers that panic on client input — 3 sites** (`payroll.rs:29`, `report.rs:260`, `overtime.rs:305`) with **no `CatchPanicLayer` installed anywhere**, so each is a dropped connection rather than a 500.

---

### From pass 2

| # | Theme | Instances |
|---|---|---|
| 1 | **Client-supplied strings reach sinks with no allow-list or CHECK** — timezone, payroll divisors, `epf_category`, `file_url` (×2 sinks), ICS dates, email addresses, pagination ints, backup FK ids | **8** (H9, L4, L2, C4, L7, M20, L8, C5) |
| 2 | **Backup/restore is not a faithful round-trip** — identity FK fallback, 2 MiB ceiling, path traversal, dropped schedule/timezone/geofence, flattened timestamps, silent file failures, reinstated logins | **7** (C5, H6, C4, M11, M12, L9, M10) |
| 3 | **Audit trail gaps and mis-scoping** — backups, employee delete, credential mutations, calendar, entry delete fidelity, NULL/wrong tenant, over-disclosure of payroll fields | **7** (M15, M16, M17, L10, L11, M14, H4) |
| 4 | **Frontend/backend permission drift** — group grants invisible, 5 pages of dead write controls, ungated settings read | **7 surfaces** (M22, L12 ×5, M23) |
| 5 | **Fail-open where a sibling path in the same codebase fails closed** — bracket gap vs `epf_band`, registration challenge vs `consume_for_user`, document delete vs `serve_upload`, NULL DOB vs the 55-59 guard, snapshot assertion vs the discoverable path, PCB edit vs `compute_payslip` | **6** (M4, L6, C4, C2, M8, H1) |
| 6 | **Statutory inputs derived from one conflated `gross` or a stale employee column** — OT in EPF wage, bonus annualisation, `is_taxable` ignored, `epf_category` static, DOB defaulted | **5** (C1, H5, M5, M2, C2) |
| 7 | **Tests that cannot fail / invariants with zero coverage** — YTD offset, married relief, docstring derivation, multi-run YTD, Face ID gate | **5** (M26, L15, L14, M24, M25) |
| 8 | **`users.company_id` (active company) used as if it were membership** | **3** (M18, M19, M14) |
| 9 | **Unbounded or unthrottled work reachable cheaply** — passkey routes, leave span, import body | **3** (M9, H10, H6) |

---

---

## Statutory coverage

**No.** The arithmetic *inside* the four calculators is now reasonably probed, but essentially every round-2 defect sits in the **inputs** to them, and that layer is close to untested.

**What is now covered:** PCB bracket arithmetic against a pinned golden value; EPF band lookup failing closed on a missing band; SOCSO/EIS fail-closed branches for known ambiguous ages (55-59, 57-59) and foreign workers; the shipped fixtures are contiguous, so the bracket-gap defect (M4) cannot fire today.

**What remains unverified:**

1. **Which wage feeds which scheme.** No test asserts what belongs in an EPF-contributable wage. C1 (overtime), M5 (`is_taxable`), and H5 (bonus/commission) are all "the wrong number entered the base" and none is caught by any test. This is the single largest gap.
2. **Which age and category apply.** No test covers a NULL DOB (C2), an over-60 or non-citizen EPF part (M2), or an under-18/negative age (L1). Every one of these is a wrong statutory deduction with no assertion behind it.
3. **YTD wiring end-to-end.** No test ever commits two runs in one year (M24), so `payroll_ytd`, `months_worked`, `remaining_months`, the persisted `ytd_*` columns, and the status allow-list are all unexercised. Deleting `months_worked` outright passes the suite.
4. **The additional-remuneration (Schedule 2) path is dead code.** `is_bonus_month` is a literal `false` at both construction sites; `calculate_bonus_pcb` has never executed. It is not merely untested — it is wrong as written (double-counts if enabled) and its existence makes the case look handled.
5. **No LHDN conformance testing.** The shipped rule sets are `status = 'prototype'` academic fixtures. There is no computerised-MTD conformance suite, and the existing golden assertion is insensitive to any annual error below ~RM12 because of the whole-ringgit round-up (L14). Reference-data correctness for EPF Third Schedule, SOCSO/EIS Acts 4/8 tables, and PCB brackets/reliefs is entirely unverified.
6. **No proration coverage.** Mid-month joiners and leavers (C3) have no test, and the leaver path is currently unreachable from the UI at all.
7. **Bracket/band edge behaviour.** No test for a gap, an above-ceiling income, or band boundaries in any of the four schemes.

**Practical read:** the PCB `require_supported_calculator` hard-fail is doing a great deal of load-bearing work — it is the only reason H5, M3, M4, M5 and L14 are not live over-deductions. **EPF, SOCSO and EIS have no such gate**, which is why C1, C2 and M2 are live money defects today. Fix C1 and C2 before anything else in this report, and treat the PCB gate as un-liftable until items 1-5 above have tests.

---

## Appendix A — fixed or partially fixed during the audit

Each was re-opened against `681b767` by an agent that read the current file and the intervening commit diffs.

### R1-C6 [critical] Bulk employee import reports success while committing zero rows

Fixed by commit bfeef8b ("fix(import): make bulk employee import atomic and auditable"). Files are committed clean — `git status` shows no uncommitted changes under backend/src/services/employee_import_service/ or backend/src/repositories/.

1) Per-row savepoint — backend/src/services/employee_import_service/confirm.rs:171-217. The loop now wraps every row:
   (&mut *tx).execute(SAVEPOINT_BEGIN).await?;   // line 171
   ... insert_bulk_import + insert_bulk_import_initial ...
   Ok  => (&mut *tx).execute(SAVEPOINT_RELEASE).await?; imported_count += 1;   // lines 194-195
   Err => (&mut *tx).execute(SAVEPOINT_ROLLBACK).await?;                        // line 198
A failing row therefore no longer leaves the connection in the 25P02 aborted state, so the subsequent rows and the final COMMIT are real.

2) The savepoint statements are the load-bearing detail and they are correct. confirm.rs:17-19 defines them as bare &str constants:
   const SAVEPOINT_BEGIN: &str = "SAVEPOINT import_row";
   const SAVEPOINT_RELEASE: &str = "RELEASE SAVEPOINT import_row";
   const SAVEPOINT_ROLLBACK: &str = "ROLLBACK TO SAVEPOINT import_row";
Issued through Executor::execute with a &str (no bind arguments), sqlx runs these on the simple query protocol — the same path its own PgTransactionManager uses. This matters: after a failed statement Postgres rejects extended-protocol Parse with 25P02, so an extended-protocol ROLLBACK TO SAVEPOINT could not have cleared the state. The comment at confirm.rs:4-16 documents exactly this.

3) The discarded error is gone — confirm.rs:181-189. Previously `let _ = salary_history::insert_bulk_import_initial(...)`; it now assigns into `result` and shares the row's savepoint, so its failure fails that row rather than poisoning the batch.

4) imported_count can no longer overstate: it increments only on the Ok arm after RELEASE (confirm.rs:195). Failed rows are pushed to failed_rows with a classified (not raw Postgres) message via e.client_response().1 (confirm.rs:208) and counted in skipped_count (confirm.rs:229).

5) The audit row can no longer claim an import that rolled back: audit_service::log_action_with_metadata is now generic over the executor (backend/src/services/audit_service.rs:54-55, `executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>`) and is called with `&mut *tx` (confirm.rs:233), committing atomically with the employees. The bespoke writer that omitted company_id is deleted — only a tombstone comment remains at backend/src/repositories/audit_logs.rs:80.

6) The session is claimed inside the same transaction (confirm.rs:223) via bulk_import_sessions::claim_for_confirmation (backend/src/repositories/bulk_import_sessions.rs:69-81), whose `UPDATE ... WHERE id = $1 AND status = 'pending'` with a `rows_affected() == 1` check closes the double-confirm race and stops a failed import from burning the session.

All three trigger cases from the report (over-length employee_number, a duplicate created between validate and confirm, a cross-tenant payroll_group_id) now degrade to one skipped row with an accurate imported_count, instead of a silently empty commit reported as success.

### R1-H6 [high] Production rate limiting collapses to one global bucket; every audit IP is identical

Fixed by commit 7e640a4 "chore(deploy): trust the proxy in production and log access" (plus the extractor rework already present in af17f71/db393dc). All three legs of the defect are closed in the current working tree; `git status` shows no uncommitted changes to any of these files (only an untracked `.mcp.json`).

1. The env var is now set, and unconditionally — not via the `secrets.env` allow-list. C:\Users\Paul\Documents\payroll-system\deploy\docker-compose.prod.yml:87 hardcodes `TRUST_PROXY_HEADERS: "true"` in the backend service `environment:` block (lines 76-86 carry the rationale comment). It is not a `${VAR:-default}` interpolation, so the compose allow-list the audit cited is no longer a gate.

2. `SmartIpKeyExtractor` is gone. C:\Users\Paul\Documents\payroll-system\backend\src\routes\mod.rs:23 now builds `let ip_key = ClientIpKeyExtractor::new(state.config.trust_proxy_headers);` and passes that same `ip_key` to all four unauthenticated limiters — `auth_rate_limit` (line 27), `forgot_rate_limit` (line 35), `oauth2_rate_limit` (line 43), `kiosk_rate_limit` (line 53). C:\Users\Paul\Documents\payroll-system\backend\src\core\config.rs:58-60 parses `TRUST_PROXY_HEADERS` (true/1, defaulting false).

3. The XFF-passthrough concern in the proposed fix is handled in code rather than only in Caddy config. C:\Users\Paul\Documents\payroll-system\backend\src\core\client_ip.rs:34-48 `forwarded_ip()` splits `x-forwarded-for` and iterates `.rev()`, taking the RIGHT-most parseable entry — the proxy-appended one — so a client-supplied left-most `X-Forwarded-For` that Caddy preserves and appends to cannot be used to forge a limiter key. Unit tests at client_ip.rs:96-104 and backend\src\core\rate_limit_key.rs:139-148 assert exactly that (`"9.9.9.9, 203.0.113.10"` -> `203.0.113.10`).

4. The audit-trail half is fixed by the same shared helper: backend\src\models\audit.rs:8,36 `AuditRequestMeta` calls `client_ip(headers, peer, trust_proxy_headers)`, fed from backend\src\core\extract.rs:46 with `state.config.trust_proxy_headers`. `audit_logs.ip_address` will therefore record the real client address, not `172.18.0.1`.

Caddy side: C:\Users\Paul\Documents\payroll-system\deploy\deploy.sh:312 still `reverse_proxy 127.0.0.1:8080` (Caddy appends the real peer to XFF), and the container publishes only `127.0.0.1:8080:8080` (docker-compose.prod.yml:69), so nothing off-host can reach Axum directly to inject a trailing XFF entry.

One cosmetic leftover, not the defect: CLAUDE.md:63 still documents "All limiters use `SmartIpKeyExtractor`", which no longer matches the code. Runtime behavior is correct regardless.

### R1-M1 [medium] Unpaid-leave deduction is written outside the approval transaction and can be lost forever

backend/src/services/approval_service/leave.rs:458-506 (approve_leave). The deduction insert is now inside the approval transaction:

458  let mut tx = pool.begin().await?;
459  let lr = leave_requests::set_approved(&mut *tx, request_id, company_id, reviewer_id, notes)
482  leave_balances::move_pending_to_taken(&mut *tx, lr.employee_id, lr.leave_type_id, lr.days, year)
491  if let Some(d) = &deduction {
492      payroll_entries::insert_unpaid_leave_deduction(
493          &mut *tx,
...
506  tx.commit().await?;

The three post-commit pool round trips described in the defect were replaced by a read-only precompute, compute_unpaid_leave_deduction (leave.rs:359-419), which issues only reads (employee_repo::basic_salary_and_company, calendar_service::count_working_days_between, calendar_service::get_working_days_in_month) and returns an UnpaidLeaveDeduction struct that is written inside the transaction. No write occurs outside the transaction, so a transient failure on the deduction insert rolls back set_approved and the leave stays 'pending', making the CAS-on-pending retry succeed — eliminating the unrecoverable approved-with-no-payroll_entries-row state.

The precompute-before-CAS ordering is guarded against the staleness window it introduces at leave.rs:466-478, which re-checks (leave_type_id, start_date, end_date, days) on the CAS-approved row against the previewed row and returns AppError::Conflict on drift.

Provenance: commit 3e659fd "fix(approvals): commit each approval's side effects with the approval" is the only commit in 8d1d88a..HEAD touching this file; git status shows no uncommitted modifications to it (only backend/.sqlx/* churn and an untracked .mcp.json).

### R1-M2 [medium] Leave rejection strands reserved `pending_days` irrecoverably

C:\Users\Paul\Documents\payroll-system\backend\src\services\approval_service\leave.rs:601-623 — `reject_leave` now opens one transaction and commits both statements together:

```rust
let mut tx = pool.begin().await?;
let lr = leave_requests::set_rejected(&mut *tx, request_id, company_id, reviewer_id, notes)
    .await?
    .ok_or_else(|| AppError::BadRequest("Leave request not found or not pending".into()))?;

// Remove from pending
let year = lr.start_date.year();
leave_balances::subtract_pending(&mut *tx, lr.employee_id, lr.leave_type_id, lr.days, year)
    .await?;
tx.commit().await?;
```

Both repository calls take `&mut *tx` (the repo fns are generic over `impl Executor`), so a fault between the status flip and the pending-days refund rolls back the rejection as well — `pending_days` can no longer be stranded. Lines 609-613 carry an explicit comment naming this exact failure mode. Notification/audit side effects happen after commit and are non-critical (`let _ =`).

Provenance: `git log --oneline 8d1d88a..HEAD -- backend/src/services/approval_service/leave.rs` returns exactly one commit, 3e659fd "fix(approvals): commit each approval's side effects with the approval". `git status --short` shows no modification to this file (only untracked `.mcp.json`), so the committed state is the working-tree state.

Scope note (not part of this defect): `approval_service/overtime.rs:470` and `approval_service/claim.rs:362` still call `set_rejected(pool, …)` non-transactionally, but neither rejection path has a paired balance refund — there is no reserved-days column to strand — so R1-M2 as described is not reachable there.

### R1-L3 [low] `default_pay_date` panics on an out-of-range period

C:/Users/Paul/Documents/payroll-system/backend/src/handlers/payroll.rs:29-43 — `default_pay_date` is now fallible and validates the month before touching chrono:

```rust
fn default_pay_date(req: &ProcessPayrollRequest) -> AppResult<chrono::NaiveDate> {
    if let Some(pay_date) = req.pay_date {
        return Ok(pay_date);
    }
    let month = u32::try_from(req.period_month)
        .ok()
        .filter(|m| (1..=12).contains(m))
        .ok_or_else(|| {
            AppError::BadRequest(format!("Invalid period month: {}", req.period_month))
        })?;

    chrono::NaiveDate::from_ymd_opt(req.period_year, month, 28)
        .or_else(|| chrono::NaiveDate::from_ymd_opt(req.period_year, month, 1))
        .ok_or_else(|| AppError::BadRequest(format!("Invalid period year: {}", req.period_year)))
}
```

No `.unwrap()` remains; both fallback paths end in `AppError::BadRequest` (400). The `as u32` cast that turned a negative month into a huge value is replaced by `u32::try_from(...).filter(1..=12)`, and an out-of-range `period_year` is likewise a 400 rather than a `None`-unwrap.

Both call sites propagate the error: `backend/src/handlers/payroll.rs:56` (`let pay_date = default_pay_date(&req)?;` in `process`) and `backend/src/handlers/payroll.rs:92` (`default_pay_date(&req)?` in `preview`).

The second half of the report — "no `CatchPanicLayer` exists, so the client gets a dropped connection" — is also no longer true: `backend/src/main.rs:9` imports `tower_http::catch_panic::CatchPanicLayer` and `backend/src/main.rs:116` applies `.layer(CatchPanicLayer::custom(panic_response))` innermost of the added layers (inside CORS), with `panic_response` at `backend/src/main.rs:27-40` logging the payload and returning a JSON 500.

Fixed by commit 0631efd "fix(payroll): reject an out-of-range period instead of panicking" (handler) and f0e7c8e "feat(errors): classify database failures as 4xx and survive handler panics" (panic layer), both landed after 8d1d88a. `git status` shows no uncommitted modifications under backend/src (only an untracked `.mcp.json`), so the committed state is the working-tree state.

### R1-L8 [low] Bulk-import audit rows are written with NULL `company_id` and are invisible to the tenant

Fixed in commit bfeef8b (reachable from HEAD 681b767); no uncommitted changes affect it (git diff HEAD empty, only untracked .mcp.json).

1. backend/src/repositories/audit_logs.rs:80-86 — the offending writer is deleted, replaced by a comment naming this exact defect: "`insert_bulk_import` used to live here. It was the only writer whose INSERT omitted `company_id`, and every read path filters `WHERE al.company_id = $1` ... The import now goes through `audit_service::log_action_with_metadata` like every other mutation." grep confirms zero remaining references to audit_logs::insert_bulk_import in backend/src/ (the two surviving `insert_bulk_import` symbols are unrelated: repositories/employees.rs:331 and repositories/salary_history.rs:38).

2. backend/src/services/employee_import_service/confirm.rs:233-249 — the sole audit write in the import path is now:
    audit_service::log_action_with_metadata(
        &mut *tx,
        Some(company_id),
        Some(user_id),
        "bulk_import",
        "employee",
        Some(req.session_id),
        ...
    ).await?;
`company_id` is the fn parameter already in scope — exactly the proposed fix. It also sits inside the import transaction (before tx.commit() at confirm.rs:251).

3. backend/src/services/audit_service.rs:54-82 — log_action_with_metadata takes `company_id: Option<Uuid>` and passes it directly to audit_logs::insert.

4. backend/src/repositories/audit_logs.rs:60-63 — audit_logs::insert's INSERT lists `company_id` as the first column, bound to $1.

5. Read paths unchanged but now satisfied: repositories/reads/audit.rs:34 (`WHERE al.company_id = $1`) and repositories/audit_logs.rs:26 (count_filtered). A bulk import therefore appears in /api/audit-logs.

6. Regression guard: backend/src/tests/import_atomicity_tests.rs:217 asserts "SELECT COUNT(*) FROM audit_logs WHERE company_id = $1 AND action = 'bulk_import'", which would fail under the old NULL-company_id behaviour.

### R2-M16 [medium] Employee deletion is unaudited and non-transactional

Fixed by commit 77f520b ("fix(employee): make employee deletion succeed, atomic and audited"), which is now on HEAD (681b767). No uncommitted changes exist under backend/src/ (`git status --porcelain -- backend/src/` is empty; the only untracked file in the tree is `.mcp.json`).

All three parts of the proposed fix are present:

1. Actor + audit metadata threaded through — C:\Users\Paul\Documents\payroll-system\backend\src\services\employee_service.rs:264-270:
```rust
pub async fn soft_delete_employee(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    deleted_by: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
```
The handler supplies both — C:\Users\Paul\Documents\payroll-system\backend\src\handlers\employee.rs:279-298: `delete` now takes an `audit_meta: AuditRequestMeta` extractor and calls `employee_service::soft_delete_employee(&state.pool, id, company_id, auth.0.sub, Some(&audit_meta))`.

2. Single transaction — employee_service.rs:274-306. `let mut tx = pool.begin().await?;` then every write goes through `&mut *tx`: `employees::soft_delete` (:275), `user_companies::delete_by_employee` (:285), `refresh_tokens::delete_by_employee` (:286), `users::soft_delete_by_employee` (:287), the audit insert (:289-304), and finally `tx.commit()` (:306). All four repository functions are generic over `impl Executor<'_, Database = Postgres>` (employees.rs:640-644, user_companies.rs:73-76, refresh_tokens.rs:88-91, users.rs:467-471), so they genuinely enlist in the transaction rather than taking a fresh pool connection. The early `rows == 0` return at :277-279 drops `tx` unrolled-back-committed, i.e. rolls back. The described failure mode — soft-deleted employee left with live refresh tokens — is no longer reachable.

3. Audit row with pre-delete snapshot — employee_service.rs:272 reads `let existing = get_employee(pool, id, company_id).await?;` before any write, and :289-304 inserts via `audit_service::log_action_with_metadata` with action `"delete_employee"`, entity_type `"employee"`, `entity_id = Some(id)`, `old_values = Some(serde_json::to_value(&existing))`, a description naming the employee, and the request metadata. `log_action_with_metadata` itself takes `impl sqlx::Executor` (audit_service.rs:54-65), so the audit row commits atomically with the deletion.

Beyond the reported scope, the `users` row is no longer hard-deleted at all: `users::soft_delete_by_employee` (repositories/users.rs:467-484) sets `is_active = FALSE, deleted_at = NOW(), deleted_by = $2` guarded by `roles <@ ARRAY['employee']`, so history-referencing FKs survive.

The second reported location, `user_service.rs:456,493`, is likewise fixed: `delete_user` (services/user_service.rs:456-461) takes `deleted_by`, opens a transaction, and performs `users::soft_delete(&mut *tx, ...)` (:483), `user_companies::delete_by_user(&mut *tx, ...)` (:491) and an audit `"delete"` insert (:493-497) inside it.

### R2-L8 [low] Pagination offset unclamped and multiplied without overflow check — partially fixed

REMAINS: 1. Change `.min(100)` to `.clamp(1, 100)` at backend/src/services/audit_service.rs:16 and backend/src/handlers/document.rs:29 (admin.rs:79 already done).
2. Replace `(page - 1) * per_page` with `page.saturating_sub(1).saturating_mul(per_page)` at audit_service.rs:17, document.rs:30 and admin.rs:80 — the release-build wraparound is unguarded at all three sites; ideally also cap `page` so a saturated offset does not silently return an empty page.
3. Add "2201W" and "2201X" to the SQLSTATE match in `classify_db_error` (backend/src/core/error.rs:151-180) so any remaining bad LIMIT/OFFSET degrades to 400/422 instead of 500. :: Only one of the three reported sites changed, and only for half the defect.

FIXED (per_page floor only):
- C:/Users/Paul/Documents/payroll-system/backend/src/handlers/admin.rs:79 now reads `let per_page = query.per_page.unwrap_or(20).clamp(1, 100);` (was `.min(100)`), so `per_page=-1` can no longer emit `LIMIT -1` on `GET /api/admin/users`. Changed by 533ccb7 / 8d1d88a-era user-management rework.

STILL PRESENT (negative LIMIT):
- backend/src/services/audit_service.rs:16 — `let per_page = query.per_page.unwrap_or(25).min(100);` with `pub per_page: Option<i64>` (backend/src/models/audit.rs:60). Value is passed unmodified to `audit_reads::list_filtered` (audit_service.rs:38) and binds directly into `LIMIT $7 OFFSET $8` at backend/src/repositories/reads/audit.rs:41. `per_page=-1` → SQLSTATE 2201W.
- backend/src/handlers/document.rs:29 — `let per_page = query.per_page.unwrap_or(20).min(100);` with `pub per_page: Option<i64>` (backend/src/models/document.rs:83), threaded through document_service::list_documents (document_service.rs:20,34) to the LIMIT/OFFSET binds. Same 2201W.

STILL PRESENT (offset multiplication) in all three:
- audit_service.rs:17, document.rs:30, admin.rs:80 all still compute `let offset = (page - 1) * per_page;` on i64 with `page` only floored by `.max(1)` and never capped. backend/Cargo.toml has no `[profile.release]` section, so `overflow-checks` is off in release: `page=9223372036854775807` wraps to a negative/garbage offset (SQLSTATE 2201X or wrong page); in debug it panics. No `.saturating_mul` / `saturating_sub` anywhere.

ERROR CLASSIFICATION STILL UNHANDLED:
- backend/src/core/error.rs:151-180 `classify_db_error` matches "23505","23503","23502","23514","22001","22003","22P02","22007","40001","40P01" — neither "2201W" (invalid_row_count_in_limit_clause) nor "2201X" (invalid_row_count_in_result_offset_clause) is listed, so both fall to `_ => return None` and surface as a generic 500 via AppError::Database (error.rs:63). The f0e7c8e "classify database failures as 4xx" commit did not cover these SQLSTATEs.

---

## Appendix B — claims that did not survive verification

Recorded so they are not re-reported. Each was raised by a finder and killed by an independent verifier that read the code, the callers, the migrations, the tests, and `CLAUDE.md`.

### Pass 1 (16)

- **authz** — *GET /api/uploads/{filename} serves every uploaded file with no authentication* — Documented intentional/accepted limitation, and the code matches the documentation exactly. SECURITY.md:65-67 lists under "Known Prototype Limitations": "Uploaded files are stored on local API disk and served through unguessable capability URLs without per-user download authorization." docs/features.md:93 repeats it as a "Known limitation" row. backend/src/handlers/portal.rs:487-490 carries an…
- **authz** — *Portal leave-balance year defaults to a hardcoded 2026* — The quoted line is genuine (backend/src/handlers/portal.rs:67 = `let year = q.year.unwrap_or(2026);`, inconsistent with :325 and :342 which use `chrono::Utc::now().year()`), but the failure scenario is unreachable and its stated harm is contradicted by the code. (1) Trigger is false: the claim says the portal Leave page calls the endpoint with no `year`.…
- **tenant-isolation** — *Passkey registration completes against a challenge issued to a different user* — Code quote verified but the defect is not reachable. backend/src/repositories/passkey_challenges.rs:42-48 does confirm `consume` omits `user_id` from its predicate, and `consume_for_user` exists unused-by-registration at lines 57-74, so the code smell is real. However the attack's sole precondition -- attacker possessing the victim's `challenge_id` -- cannot be met by a real user:…
- **payroll-math** — *Nothing blocks processing an earlier period after a later run is committed, leaving stale stored YTD* — Refuted on both the stated impact and reachability, though the bare code observation is accurate. (a) Quote verified: backend/src/services/payroll_engine.rs:612-622 does only a same-period duplicate check, and the contrast is real — payroll_service.rs:130 calls run_has_later_committed_run and payroll_service.rs:198 calls employee_has_later_run. Stored YTD is chained (payroll_engine.rs:1069-1075…
- **statutory** — *One-off bonus is annualised as recurring income in PCB, wildly over-withholding* — The code pattern is real but the failure scenario is unreachable in any deployed build, and the disabling is documented intent rather than an oversight. Evidence verified as quoted: backend/src/repositories/reads/payroll.rs:113-118 sums every category='earning' payroll_entries row except overtime/claim_reimbursement, so an item_type='bonus' row does reach variable_earnings; payroll_engine.rs:989…
- **statutory** — *PCB EPF relief cap read from the life-insurance row (RM3,000) instead of RM4,000* — The quoted code is accurate — pcb_calculator.rs:108 really does read the EPF cap from the "life_insurance" key (RM3,000) while the module doc at pcb_calculator.rs:17 says "EPF up to RM4,000". But the defect is unreachable in any deployed build, and the disable is documented as intentional. statutory_rules.rs:14-23 defines `require_supported_calculator` under `#[cfg(not(test))]` to return Err…
- **statutory** — *A zero-wage employee produces a hard band-lookup error that blocks the whole run* — The quoted code exists as cited (statutory_tables.rs:88 and :99; epf_service.rs:41-50; socso_service.rs:56-78; 1001_data.sql:64 lowest EPF band `(1, 3000, ...)`; migrations/1000_schema.sql:530 `basic_salary bigint DEFAULT 0 NOT NULL` with no CHECK; repositories/employees.rs:206-240 filters only company/group/is_active/dates). But the failure scenario is unreachable in the deployed system, and the…
- **statutory** — *The PCB conformance guard blocks every payroll run, and its own remediation advice is unreachable* — The quoted code exists verbatim (statutory_rules.rs:14-23 and :55, payroll_engine.rs:409 and :630), and the mechanism is as described — but it is an explicitly documented, intentional fail-closed regulatory gate, not a defect. docs/database.md:103-107 states "Production payroll fails closed... Automatic PCB remains disabled in production builds even if data is marked verified: the current…
- **session-crypto** — *Enabling 2FA and regenerating backup codes are not transactional, and there is no admin recovery path* — The quoted lines exist verbatim (backend/src/services/totp_service.rs:128-133 and :199-201), and the two-statement sequence is genuinely non-transactional. But the finding's load-bearing claim — "recovery is impossible... locked out until someone edits the database by hand" — is refuted by the code, so the residual issue is not a data-integrity defect. 1. The post-failure state ALWAYS includes a…
- **concurrency** — *approve_overtime commits the approval, then stages pay and replacement leave non-atomically* — The non-atomicity is real, but the claimed defect — an approved OT application that "is never paid" — is refuted by the pay path. The payroll engine does not pay overtime from the staged payroll_entries row. backend/src/repositories/reads/payroll.rs:229-234 (approved_ot_totals) sums hours directly from `overtime_applications WHERE status = 'approved'`; that feeds bulk.approved_ot…
- **schema** — *1001_data.sql provisions employee accounts with a fixed, repo-committed bcrypt hash* — The code observations are accurate but the claimed exploit is not reachable. Confirmed: 1001_data.sql:457 holds a well-formed, byte-identical, repo-committed bcrypt digest (I verified it parses as a valid $2b$12$ salt+hash via bcrypt in Docker), and must_change_password does not gate the backend login path (auth_service.rs:54-78 checks only bcrypt::verify + linked_employee_active; the flag is…
- **frontend-core** — *Kiosk retry timer is never cancelled, so a stale kiosk key keeps polling forever* — The quoted code is real (AttendanceKioskPublic.tsx:75-77 — a bare setTimeout with no handle, unlike every other timer in the file at :44/46, :89/92, :115/116, :122/125), but the claimed failure is unreachable and the mechanism is misdescribed. (a) Unmount cannot happen. /kiosk/:kioskKey is a top-level dead-end route (App.tsx:131). I read the component's entire render output…
- **contract-drift** — *Frontend calls three /admin/password-resets endpoints that routes/mod.rs never registers* — The route-absence half is factually correct (no /admin/password-resets in backend/src/routes/mod.rs; the only /admin/* routes are lines 161-178 and 454), but the failure scenario is unreachable and the design is intentional. (1) Unreachable: frontend/src/pages/admin/PasswordResets.tsx is never imported by anything. App.tsx:39-42 lazy-loads exactly four admin pages (CompanyManagement,…
- **business-logic** — *Overtime pay is computed in f64 with double integer truncation* — The quoted f64/integer-truncation code does exist at backend/src/services/approval_service/overtime.rs:304-327, but its output is never paid, so the claimed underpayment is impossible. approve_overtime only stages a payroll_entries row with item_type='overtime' (backend/src/repositories/payroll_entries.rs:337-367), and every engine read excludes that item type from gross:…
- **infra-ops** — *Deploy health gate and container healthcheck use the static /api/health string, never the readiness probe* — Refuted on three independent grounds. (1) UPSTREAM GUARD MAKES /api/health A DE FACTO DB CHECK AT DEPLOY TIME. backend/src/core/db.rs:4-10 — `create_pool` uses an eager `PgPoolOptions::connect()` with `.expect("Failed to create database pool")`; db.rs:12-23 — `run_migrations` ends in `.expect("Failed to run database migrations")`. backend/src/main.rs:46-47 calls both, and `axum::serve` is not…
- **infra-ops** — *No pre-deploy database backup ever leaves the VPS that holds the only copy of the data* — The quoted lines all exist verbatim (deploy/deploy.sh:27, :239, :242-249, :261-263; deploy/docker-compose.prod.yml:110-113 `postgres_data: driver: local`), but the finding is refuted on three independent grounds. (1) The design decision is explicitly documented in the very file cited. deploy/deploy.sh:10-11 states the script "never contacts ECR, S3, SSM, Secrets Manager, Route53, or another…

### Pass 2 (37)

- **pcb-math** — *Remaining-months divisor ignores date_resigned, annualising income a leaver will never earn* — The quoted code is real (pcb_calculator.rs:42-47, payroll_engine.rs:1025 `months_worked: month`, and models/statutory.rs:160-178 confirms PcbInput carries no resignation field), and I re-derived the claimed arithmetic independently — June PCB = (135,971 - 57,000)/7 = 11,281 sen, rounded up to 11,300 sen. The claim nonetheless fails on reachability. statutory_rules.rs:14-23 defines…
- **pcb-math** — *num_children is unvalidated and unconstrained, letting an employee record set arbitrary tax relief* — The quoted code is real but the harm is unreachable in any deployed (non-test) build. statutory_rules.rs:14-23 defines `#[cfg(not(test))] fn require_supported_calculator`, which returns Err unconditionally when `rule_code == PCB` ("Automatic PCB is disabled because the current academic calculator has not passed LHDN computerised-MTD conformance testing"). `require_all_verified` calls…
- **pcb-math** — *Manual PCB override can drive net salary negative, bypassing the engine's fail-closed guard* — Quoted code is accurate (payroll_service.rs:182-184, 220-222; payroll_engine.rs:1058-1065; payroll_runs.rs:414-419; 1000_schema.sql:884 has no CHECK on net_salary), and the arithmetic works (430000 - 500000 = -70000 sen). But the failure scenario is unreachable, and its premise is inverted. statutory_rules.rs:53-54 makes require_all_verified call require_supported_calculator(PCB) as its first…
- **pcb-math** — *Bracket walk skips the band whose floor equals the chargeable income and mis-measures band width* — Unreachable in any deployed build, and the magnitude is materially overstated. (1) Hard gate: backend/src/services/statutory_rules.rs:14-23 defines `require_supported_calculator` under `#[cfg(not(test))]` to return `AppError::Validation` unconditionally whenever `rule_code == PCB` ("Automatic PCB is disabled because the current academic calculator has not passed LHDN computerised-MTD conformance…
- **epf-socso-eis-math** — *newest_schedule drops still-effective SOCSO/EIS bands and silently lowers the wage ceiling* — The quoted code exists verbatim (backend/src/services/statutory_tables.rs:61-64 and the helper at :132-139), and the arithmetic of the claimed mechanism is correct in isolation. But the claim fails on intent, on regression status, and on reachability. (1) Documented as intentional, with a test asserting exactly this behavior. statutory_tables.rs:127-131 states the contract: "SOCSO and EIS bands…
- **epf-socso-eis-math** — *A zero contributable wage produces a hard error instead of a zero contribution, blocking the whole run* — Unreachable in the deployed system, and the behavior it objects to is documented intent. (1) backend/src/services/statutory_rules.rs:14-23 defines `require_supported_calculator` under `#[cfg(not(test))]` so it ALWAYS returns the "Automatic PCB is disabled" Validation error, and `require_all_verified` (statutory_rules.rs:54-55) begins with `require_supported_calculator(PCB)?`. `process_payroll`…
- **epf-socso-eis-math** — *is_taxable is stored on every earning line but never consulted when building the statutory wage base* — The quoted code exists verbatim (payroll_engine.rs:989 `let gross = basic + allowances_total + variable_earnings + total_overtime;`, and :1257-1259 `PayslipLine::earning(...).taxable(line.is_taxable)`), and the two reads genuinely lack an is_taxable predicate (reads/payroll.rs:26 and :113, wired in at payroll_engine.rs:172/196 -> :910/912). The arithmetic also checks out against the fixtures:…
- **epf-socso-eis-math** — *StatutoryTables::load issues six unsynchronized reads, so recorded provenance can disagree with the bands used* — The quoted code exists verbatim (backend/src/services/statutory_tables.rs:48-59 — six sequential awaits on `pool`, no transaction), but the defect is not reachable and the proposed remedy would not fix it. 1. The only code path that records provenance is `process_payroll`, and in any non-test build it cannot reach the snapshot write. `payroll_engine.rs:630` calls…
- **statutory-snapshot** — *newest_schedule() discards every band from an earlier effective_from generation in the same rule set* — REFUTED on three independent grounds. (1) NOT A REGRESSION — the Rust faithfully reproduces pre-existing SQL. `git show 1dd155e^:backend/src/repositories/socso_rates.rs` shows the old `find_rate` CTE did exactly this in SQL: `SELECT rates.effective_from, MAX(rates.wage_to) AS wage_ceiling ... GROUP BY rates.effective_from ORDER BY rates.effective_from DESC LIMIT 1`, then joined bands `ON…
- **statutory-snapshot** — *EPF band lookup has no schedule narrowing, so one payslip can mix two rate generations* — The quoted code is real (backend/src/services/statutory_tables.rs:84-91), but the defect is not. (a) The multi-candidate case cannot exist. backend/migrations/1000_schema.sql:2919-2927 defines epf_rates_no_overlapping_bands as EXCLUDE USING gist (rule_set_id WITH =, category WITH =, int8range(wage_from,wage_to,'[]') WITH &&, daterange(effective_from, COALESCE(effective_to,'infinity'),'[]') WITH…
- **statutory-snapshot** — *Statutory snapshot is read outside the run transaction and non-atomically across six statements* — The quote is accurate (backend/src/services/statutory_tables.rs:48-59 does hardcode `&PgPool` and issue six sequential queries; payroll_engine.rs:296 does call it with `pool` while the surrounding reads use `&mut *conn`). But the claim's causal mechanism is wrong, and its trigger is not reachable. (1) MISDIAGNOSIS — "inside the transaction" buys nothing here. payroll_engine.rs:633 is a bare…
- **statutory-snapshot** — *calculation_snapshot pins rule-set identity but not the rate-row generation actually used* — The claim's core assertion — that two runs using different rate-row generations produce indistinguishable snapshots — is false. backend/src/services/payroll_engine.rs:756 writes "effective_date": effective_date into the blob, and RunPeriod::resolve (payroll_engine.rs:33-62) sets effective_date = period_end, so the December 2025 run records "2025-12-31" and the January 2026 run records…
- **statutory-snapshot** — *PCB bracket/relief rows are selected by effective_year with no check that the year lies inside the parent rule set's verified interval* — REFUTED on three independent grounds. (a) The premise "the year is never validated against the set" is wrong by construction: statutory_tables.rs:49 sets `tax_year = effective_date.year()`, and the very same query at pcb_brackets.rs:24-25 / pcb_reliefs.rs:24-25 requires `rules.effective_from <= effective_date AND (rules.effective_to IS NULL OR rules.effective_to >= effective_date)`. Rows are…
- **statutory-snapshot** — *statutory_rule_sets constraints are only created when the table does not already exist* — The quoted code exists as described (backend/migrations/1000_schema.sql:2838-2884, contrasted with the guarded ALTER TABLE at 2894-2913), but the claim's premise — that databases upgraded from the historical v1-v4 chain already have statutory_rule_sets — is false, so the CREATE TABLE IF NOT EXISTS is never a no-op. Proof: `git log --all -S'statutory_rule_sets' -- backend/migrations` returns…
- **statutory-snapshot** — *Statutory snapshot is dated at period end, so a rule set covering only part of the period rates the whole period* — Refuted on three independent grounds. (a) DOCUMENTED AS INTENTIONAL. The quoted struct at backend/src/services/payroll_engine.rs:55-61 does exist, but the doc comment immediately above it at payroll_engine.rs:31-33 states the design verbatim: "`effective_date` is the period end: statutory rules and recurring allowances are effective-dated, and a run is rated as at the last day of the month it…
- **statutory-snapshot** — *PCB EPF relief cap is read from the 'life_insurance' relief key, not the EPF one* — Refuted on three independent grounds. (1) UNREACHABLE: backend/src/services/statutory_rules.rs:14-22 defines `require_supported_calculator` under `#[cfg(not(test))]` to return Err unconditionally for the PCB rule code ("Automatic PCB is disabled because the current academic calculator has not passed LHDN computerised-MTD conformance testing"). `require_all_verified` (statutory_rules.rs:55)…
- **statutory-snapshot** — *A zero gross salary has no band in any schedule, so one 0-salary employee aborts the whole company run* — The band-lookup quote is accurate (statutory_tables.rs:84-91) and wage 0 does miss every fixture band (1001_data.sql:64 EPF, :146 SOCSO), but every load-bearing element of the finding fails. (1) The cited reachability path is blocked. The scenario is "bulk-imported with no salary column so basic_salary takes the schema default of 0", but employee_import_service/validation.rs:33-38 rejects any…
- **ytd-ea-numerics** — *A leaver's final month is annualised to December, ignoring date_resigned* — The quote is accurate (payroll_engine.rs:1025 `months_worked: month`, consumed at pcb_calculator.rs:42-47 as `remaining_months = 12 - current_month + 1`), and the claimant's arithmetic is mechanically correct. But the finding fails reachability and intentionality. (1) UNREACHABLE IN THE DEPLOYED SYSTEM. backend/src/services/statutory_rules.rs:14-23 defines `#[cfg(not(test))] fn…
- **ytd-ea-numerics** — *Moving an employee between payroll groups mid-period produces two payslips for the same month, doubling YTD and EA gross* — The quoted SQL is real (backend/src/repositories/payroll_runs.rs:18-27), and I confirmed there is genuinely no cross-group per-employee-period guard: employees::list_for_payroll_run (repositories/employees.rs:213-237) filters only on company/group/active/employment window with no exclusion of employees already paid for the period; payroll_engine::process_payroll…
- **ytd-ea-numerics** — *Reachable panic in default_pay_date for an out-of-range period_month or period_year* — Both premises of the claim are false in the current working tree. 1. The quoted evidence does not exist. `backend/src/handlers/payroll.rs:29` is now `fn default_pay_date(req: &ProcessPayrollRequest) -> AppResult<chrono::NaiveDate>` — a fallible function with no `.unwrap()`. Lines 33-42 explicitly validate the month before touching chrono: `u32::try_from(req.period_month).ok().filter(|m|…
- **webauthn-kiosk** — *Sign-counter update SQL can never match a row (jsonb `->> 0` on a scalar)* — Refuted empirically against the running PG 19beta2 instance and the live table. (1) The claim's core premise is false: PostgreSQL stores a jsonb scalar as a one-element pseudo-array (JB_FSCALAR), so `'"AbCdEf"'::jsonb ->> 0` returns `AbCdEf`, NOT NULL (verified; `->> 1` returns NULL, and `'{"a":1}'::jsonb ->> 0` returns NULL — objects are the case that yields NULL, which is likely what the…
- **backup-data** — *Employee account provisioning links deactivated accounts and reports them as successfully linked* — The quoted code exists as claimed (backend/src/services/backup_service/import.rs:41-57; users::find_by_email at repositories/users.rs:14-23 selects only id, roles, company_id, is_deleted; link_to_employee at users.rs:325-331 does not touch is_active), and the LinkedExisting branch is reachable on an overwrite restore (delete_company_cascade, repositories/backup.rs:468-554, deletes employees but…
- **backup-data** — *format_version is a frozen "1.0" literal, so the version gate cannot detect an archive from a different schema* — The quoted code exists (backend/src/services/backup_service/export.rs:66 writes `format_version: "1.0".into()`, and backend/src/services/backup_service/import.rs:111-116 gates on `!= "1.0"`), but the failure scenario that gives the claim its severity is factually wrong, and the evidence cited as proof of drift actually proves the opposite. 1. The concrete scenario — "restore an archive produced…
- **email-letters** — *Recipient display name interpolated unescaped into RFC 5322 mailbox can redirect delivery* — REFUTED — the claim rests on a false premise about lettre's parser. The cited code does exist verbatim at backend/src/services/email_service.rs:140-154, and full_name really is unvalidated (models/employee.rs:87,164) and flows in from handlers/employee.rs:219, handlers/email.rs:284, approval_service/leave.rs:509. But the claimed parse behavior ("lettre's Mailbox::from_str splits on the FIRST <…
- **email-letters** — *send_letter accepts an unvalidated template_id, causing an FK-violation 500 and cross-tenant references* — Quote is accurate (backend/src/handlers/email.rs:289, :337 pass req.template_id unvalidated into repositories/email_logs.rs:74), but both claimed harms fail. (1) The "FK-violation 500" is wrong: backend/src/core/error.rs:151-156 maps SQLSTATE 23503 to 422 UNPROCESSABLE_ENTITY with "A referenced record does not exist, or is still in use elsewhere.", logged at warn. email_logs_template_id_fkey is…
- **email-letters** — *A template that has ever been used can never be deleted (FK violation surfaces as 500)* — The mechanical premise is real but the claimed harm is refuted. The quoted DELETE exists at backend/src/repositories/email_templates.rs:105-118, and backend/migrations/1000_schema.sql:2370 does declare email_logs_template_id_fkey with no ON DELETE clause, so a used template cannot be hard-deleted. However, the claim's entire substance -- "FK violation surfaces as 500", "generic 500 instead of a…
- **panic-sweep** — *Background tasks are spawned unsupervised, so a panic kills them permanently* — The quoted evidence exists verbatim (backend/src/main.rs:176-214 and :128-163) — both tasks are `tokio::spawn`ed with the handle dropped, and Cargo.toml has no `panic = "abort"` profile, so unwinding is the default. But every concrete panic trigger in the failure scenario is either absent from the call graph or unreachable, and the realistic failure mode is already handled: (1) `compute_payslip`…
- **audit-completeness** — *Payroll lifecycle audit rows are written after commit on the pool and their result is discarded* — The quote is accurate (payroll_lifecycle_service.rs:27), but the claim is refuted on four independent grounds I read directly. 1. DOCUMENTED AS INTENTIONAL, NOT AN OVERSIGHT. audit_service.rs:46-52 is an explicit doc comment on `log_action_with_metadata`: it is "Generic over the executor so a caller can pass `&pool` (best-effort, the common case) *or* `&mut *tx` ... The second form matters…
- **roleguard-parity** — *User groups can grant `exec` payroll permissions, defeating the documented exec/payroll invariant* — The quoted code exists (backend/src/core/auth.rs:285-287), but the behaviour is intentional, super_admin-gated, and crosses no privilege boundary. 1. Intentional and pinned by tests, not an oversight. backend/src/services/user_group_service.rs:1-6 documents groups as additive-only bundles; backend/src/core/auth.rs:283-284 states "Roles and groups are additive. Groups can only add, never…
- **roleguard-parity** — *Company-scoped user groups can grant platform-wide permissions, enabling cross-tenant escalation* — The quoted code is real (user_group_service.rs:29-33), and the downstream handlers are genuinely tenant-blind (company_service.rs:27 list_companies takes only &PgPool; company_service.rs:83 delete_company deletes any path id; both gated only by require_permission(ManageCompanies) at handlers/admin.rs:31 and :63), with AuthUser::can() at core/auth.rs:286 unioning group grants unfiltered. But the…
- **test-gaps** — *Every DB-gated test silently self-skips on any pool-connect failure* — Quoted code at backend/src/tests/support.rs:20-22 and 101-109 exists verbatim, and ~108 of ~140 tests do use `let Some(pool) = skip_if_no_db().await else { return };`. But the claim is refuted on four independent grounds. (1) The central CI assertion is factually false. .github/workflows/ci.yml:49-101 defines `backend-test` with DATABASE_URL at job level (line 72), then runs IN ORDER: `cargo sqlx…
- **test-gaps** — *Bulk employee import has no test at all* — Both halves of the claim fail against the files on disk. (1) "No test at all" is false. The repo tests this service inline, not via src/tests/mod.rs. backend/src/services/employee_import_service/parsing.rs:387 has 18 tests (header aliasing, quoted cells, short/wide/blank rows, empty upload, non-spreadsheet rejection); validation.rs:352 has 22 tests (every mandatory field, every…
- **test-gaps** — *Payroll state machine tested only on the happy path, with no cross-company case* — The quoted code exists at backend/src/tests/payroll_lifecycle_tests.rs:40 and the negative-path tests really are absent, but the claim's failure scenario is disproven by the repository layer. Every lifecycle transition is guarded twice — the service check AND the SQL predicate. backend/src/repositories/payroll_runs.rs:263 (set_approved) is `WHERE id = $1 AND company_id = $2 AND status =…
- **test-gaps** — *Group-permission key check is vacuous on a CI database* — The narrow observation is true but the defect built on it is false. Confirmed the quoted code at backend/src/tests/schema_invariant_tests.rs:150-169, and confirmed no seeding: grep -c "user_group_permissions" backend/migrations/1001_data.sql returns 0, migration 1007_user_groups.sql:39-49 only creates the table, and no test inserts into it — so `stored` is indeed empty in CI. However, the claim's…
- **migration-integrity** — *New unique index payroll_runs_one_active_period added to a populated table with no pre-flight duplicate check* — The quoted DDL is real and unguarded (backend/migrations/1000_schema.sql:3058-3060, in the reconciliation section that runs against existing v1-v4 data, vs. the guarded DO blocks at lines 3012-3036), but the failure scenario is not reachable. 1) The duplicate state cannot exist in any legacy database. The v1-v4 creating path is `insert_processing` at…
- **migration-integrity** — *Migration stages take no deploy lock and use a separate concurrency group, so an auto-deploy can run sqlx migrations against a half-restored database* — The lock/concurrency facts are accurate (deploy/migrate-database.sh has no flock; deploy/deploy.sh:54-55 has one; deploy-backend.yml:28 vs migrate-database.yml:32 are disjoint groups), but the claimed harm — sqlx::migrate! running against a half-restored DB — is not reachable. (1) migrate-database.sh:26-27 and :283-285 show the native backend is stopped ONLY at cutover, so throughout the entire…
- **migration-integrity** — *CI only ever migrates an empty database, so the entire live-schema reconciliation half of the rebaseline is never exercised before production* — The ci.yml:94 quote is real, but the claim's premise and consequence both fail. (a) The reconciliation section is NOT inside the bootstrap guard: `$pg19_bootstrap$` closes at backend/migrations/1000_schema.sql:2816 and "-- === LIVE SCHEMA RECONCILIATION" starts unconditionally at line 2820, so CI's empty DB does execute every statement in it — both duplicate-check DO blocks (3012-3036),…

---

## Reproducing

Generated by three `Workflow` runs of parallel review agents (294 agents, ~20.6M tokens,
5,603 tool calls) over the working tree, not over a diff. Nothing here was machine-verified by
executing the code: no test was run, no query was executed against a database, and no exploit was
attempted. Every finding is a reading of the source, adversarially checked by a second reading.
Confirm before acting on any of them.
