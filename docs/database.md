# Database baseline, schema, and PostgreSQL policy

## Version policy

| Environment                   | Version              | Policy                                                  |
| ----------------------------- | -------------------- | ------------------------------------------------------- |
| Docker Compose                | PostgreSQL 19 Beta 2 | Pinned by `postgres:19beta2-alpine` image tag           |
| GitHub Actions database tests | PostgreSQL 19 Beta 2 | Pinned by `postgres:19beta2-alpine` service image       |
| Production (AIC VPS)          | PostgreSQL 19 Beta 2 | Pinned `postgres:19beta2` container in production stack |

There is no Terraform or managed-database module in this repository (AWS infrastructure was retired and removed in September 2026). The PostgreSQL 18 allowance in `1000_schema.sql` remains documented only for an optional managed-Postgres exception (such as AWS RDS 18.4) that does not yet offer production PostgreSQL 19.

PostgreSQL 19 Beta 2 is a pre-release build: upstream advises against unattended beta upgrades because catalog formats can change prior to 19.0 GA. Any major-version move requires `pg_upgrade`, logical replication, or `pg_dump`/`pg_restore`; a data volume created by an older major version cannot be mounted directly by PostgreSQL 19.

## Consolidated baseline and forward migration chain

`backend/migrations/` contains:

1. `1000_schema.sql` — canonical fresh schema baseline plus idempotent reconciliation for databases created by the retired versions `1`–`4`.
2. `1001_data.sql` — system/reference defaults, explicitly unverified academic statutory fixtures, and idempotent legacy repairs. Contains **no** demo company, personal data, known password, or privileged account.
3. **Immutable forward migrations `1002`–`1034`** (intentional numbering gaps exist at `1013` and `1028`):
   - `1002`–`1010`: TOTP 2FA, user sessions, kiosk-scoped QR tokens, attendance & payroll generation reworks, user groups, audit retention FKs, `uploaded_files` foundation, and office-network binding (`attendance_network_observations`, `company_networks`).
   - `1011`–`1024`: tenant composite FKs, upload URL guards, audit filter indexes, network denial evidence, claim payment authority, overtime bounds, payroll population index, timezone validation + auto-absent bookmark, payroll item bounds, overtime divisor rename, auto-absent placeholder uniqueness, and `unpaid_break_minutes`.
   - `1025`–`1027`: payroll ops (`payroll_runs` cancel metadata, `salary_history.change_type`), async `background_jobs`, and cancelled-run timestamp backfill.
   - `1029_epf_part_f_and_statutory_defaults.sql`: expands `employees_epf_category_check` to `A`–`F` (KWSP Part F foreign-worker 2%+2%) and seeds `statutory/minimum_wage_monthly` (`"1700"`) and `statutory/epf_foreign_worker_rate` (`"2"`).
   - `1030_security_hardening.sql`: `uploaded_files` ownership registry + URL scheme checks, TOTP replay/lockout columns, `payroll_runs.submitted_by` & actor FKs, `user_group_permissions` platform-only check, `email_logs` tenant FKs, immutable `company_id` triggers, membership cleanup trigger, and SHA-256 hashing of stored `attendance_qr_tokens.token`.
   - `1031_manager_role.sql`: widens `users_roles_valid` CHECK constraint to include `'manager'`.
   - `1032_phase3_hardening.sql`: keyed SHA-256 `code_lookup_hash` on `user_totp_backup_codes`, email outbox retry columns (`attempts`, `max_attempts`, `next_retry_at`), and 10 non-negative/chronological CHECK constraints on `employees`.
   - `1033_audit_bugfixes.sql`: relaxes `salary_history_approval_pair_check` for `ON DELETE SET NULL`, adds `refresh_tokens.revoked_at`, adds `payroll_items.taxable_gross`, links `payroll_entries` to `overtime_application_id`/`leave_request_id`, drops `DEFAULT 'A'` on `employees.epf_category` for auto-derivation, adds CHECK constraints on `claims` and `tp3_records`, enforces unique `(user_id, provider)` on `oauth2_accounts`, and adds retention/listing indexes.
   - `1034_push_notifications.sql`: creates `push_subscriptions` and `push_outbox` tables with active-subscription, endpoint, and retry queue indexes.

All migrations are embedded via `sqlx::migrate!("./migrations")` and recorded in `public._sqlx_migrations`. `core::db::run_migrations` permits missing files only for retired versions `1`–`4` (and warns without aborting on strictly newer ahead-of-binary versions during rollback), while panicking on divergent mid-chain gaps. Once deployed, every migration file is **strictly immutable** (enforced in CI by `backend-migrations`).

## Core domain tables and invariants

### Payroll and statutory tables

- `payroll_runs` — period, group, totals, lifecycle timestamps, `submitted_by`, `cancelled_by`/`cancelled_at`/`cancel_reason` with consistency checks, `calculation_snapshot`, and partial unique index `payroll_runs_one_active_period` (`status <> 'cancelled'`).
- `payroll_items` — per-employee committed figures including `taxable_gross`, bounds checks (`1021`), and frozen `ytd_*` totals.
- `payroll_item_details` — explainable earning/deduction/statutory line items behind each payslip.
- `payroll_entries` — staged manual, OT, or unpaid-leave entries (`overtime_application_id`, `leave_request_id`, `is_processed`), reverted on run deletion, cancellation, or reversal.
- `payroll_groups`, `employee_allowances`, and `salary_history` (`change_type` vocabulary + `notes` + `approved_by`/`approved_at`).
- `background_jobs` (`1026`) — tracks async `payroll_run` and `employee_import` jobs with a partial unique index preventing concurrent runs for `(company_id, payroll_group_id, period_year, period_month)`.
- `statutory_rule_sets` + `epf_rates` (`A`–`F`), `socso_rates`, `eis_rates`, `pcb_brackets`, `pcb_reliefs`, and `tp3_records`.

### Statutory data safety gate

`statutory_rule_sets` records a stable dataset key, domain, effective interval, status (`prototype` / `verified` / `retired`), source document metadata, SHA-256 digest, and verification timestamp, guarded by an exclusion constraint allowing at most one overlapping `verified` rule set per domain. Lookup queries join through `statutory_rule_sets` and fail closed in production if verified data is missing. Automatic PCB stays disabled (`platform_settings.pcb_calculator_status <> 'enabled'`) until the calculator passes LHDN computerised-MTD conformance. See [legal.md#part-iii-malaysian-statutory-compliance-guide-2026](legal.md#part-iii-malaysian-statutory-compliance-guide-2026) and [payroll.md](payroll.md).

### Notifications, uploads, and tenancy

- `push_subscriptions` & `push_outbox` (`1034`) store browser Web Push keys (`endpoint`, `p256dh`, `auth`), failure tracking, revocation timestamps, and durable delivery retries.
- `uploaded_files` (`1030`) tracks managed `/api/uploads/<name>` ownership per tenant alongside single-component path/scheme checks.
- Critical tenant-owned child tables enforce composite `(parent_id, company_id)` foreign keys and immutable `company_id` triggers; companyless junctions (`payroll_items`, `leave_balances`, `team_members`) enforce same-company constraint triggers.
- UUID primary keys default to native `uuidv7()`; `pg_trgm` GIN expression indexes accelerate employee search; partial indexes exclude terminal/irrelevant rows; multicolumn statistics guide PostgreSQL 19 query planning.

## SQLx offline cache

Compile-time SQLx macros (`query!`, `query_as!`, `query_scalar!`) are verified against `backend/.sqlx/` in CI and Docker builds (`SQLX_OFFLINE=true`). After adding or modifying a macro query against a migrated PostgreSQL 19 database:

```bash
cd backend
DATABASE_URL=postgres://... cargo sqlx prepare
SQLX_OFFLINE=true cargo check --all-targets
```

## Backup, restore, and PostgreSQL 19 operations

- **Nightly & pre-deploy dumps**: `deploy/payroll-backup.sh` writes local custom-format dumps (`pg_dump --format=custom --no-owner --no-acl -U payroll payroll_db`), verifies archive readability with `pg_restore --list`, and retains 14 dumps in `/opt/payroll/backups` (`chmod 0600`). Copy dumps off-host manually before risky maintenance.
- **Restore / container migration**: `deploy/migrate-database.sh` and `pg_restore` against a clean volume handle controlled restores. Never mount an older PostgreSQL major-version data directory directly under PostgreSQL 19, and keep a verified logical backup before any beta-to-RC/GA upgrade.
- **No seed credentials**: Do not add organization data or credentials to `1001_data.sql`; create the initial administrator with `cargo run --bin bootstrap_admin`.
