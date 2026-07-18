# Database baseline and PostgreSQL policy

## Version policy

| Environment | Version | Policy |
| --- | --- | --- |
| Docker Compose | PostgreSQL 19 Beta 2 | Pinned by image tag |
| GitHub Actions database tests | PostgreSQL 19 Beta 2 | Pinned by image tag |
| Lightsail deployment | PostgreSQL 19 Beta 2 | Explicit beta deployment; see the sanitized upgrade record |
| AWS RDS Terraform | PostgreSQL 18.4 | Intentional provider exception until standard RDS offers production PostgreSQL 19 |

PostgreSQL 19 Beta 2 is a pre-release build. The
[official announcement](https://www.postgresql.org/about/news/postgresql-19-beta-2-released-3350/)
advises beta users to test it and not use it in production systems. A
major-version move requires
`pg_upgrade`, logical replication, or dump/restore; an older data volume cannot
simply be mounted by PostgreSQL 19.

AWS currently lists PostgreSQL 19 only in its Preview track, while its standard
RDS release list tops out at PostgreSQL 18.4. Preview databases are explicitly
non-production and are automatically deleted after 60 days. See the
[RDS PostgreSQL versions](https://docs.aws.amazon.com/AmazonRDS/latest/PostgreSQLReleaseNotes/postgresql-versions.html)
and [Preview environment limits](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/working-with-the-database-preview-environment.html).

The canonical schema targets PostgreSQL 19. Its minimum guard is PostgreSQL 18
only so the same application can run against the documented RDS exception;
native `uuidv7()` is available there. The current
[PostgreSQL 19 release notes](https://www.postgresql.org/docs/19/release-19.html)
also describe improved
optimizer, foreign-key checks, GIN maintenance, asynchronous I/O, and defaults
TOAST compression to LZ4, so the indexes, constraints, and wide JSON/text rows
benefit without vendor-specific application branches.

## Exactly two current SQL scripts

`backend/migrations/` contains:

1. `1000_schema.sql` — canonical fresh schema plus idempotent reconciliation for
   databases created by the retired versions 1–4.
2. `1001_data.sql` — system/reference data, explicitly unverified academic
   statutory fixtures, and idempotent legacy repairs. It intentionally contains
   no demo company, personal data, known password, or privileged account.

Both are embedded by `sqlx::migrate!("./migrations")` and recorded in
`public._sqlx_migrations`. The application permits missing files only for the
known retired versions 1–4 and refuses any other unknown applied version before
changing the schema.

These files are a one-time consolidated baseline. Once version 1000 or 1001 has
been deployed, its checksum is immutable. A later production change must use a
new numbered migration; a future squash back to two files must be an explicit
rebaseline with the same fresh-install, upgrade, checksum, and backup testing
performed here.

## Fresh and existing database behavior

On a fresh database, migration 1000 creates the complete schema and validated
constraints. On an existing v1–v4 database, it recognizes the `companies`
table, skips duplicate bootstrap DDL, and applies a safe reconciliation:

- removes the retired scalar `users.role` column and retains the roles array;
- adds soft-delete/account-link columns introduced by the old chain;
- normalizes UUID primary-key defaults to native time-ordered `uuidv7()`;
- removes the no-longer-needed `uuid-ossp` extension without `CASCADE`;
- replaces redundant indexes and creates query-aligned indexes;
- creates natural keys required by reference-data UPSERTs;
- links statutory rows to source/version verification metadata and prevents
  overlapping verified effective periods;
- installs new checks and foreign keys as `NOT VALID` for legacy rows;
- adds multicolumn optimizer statistics.

`NOT VALID` still enforces a check or foreign key for new and changed rows. It
avoids an unbounded validation scan during application startup. Operators should
audit legacy data and then run `ALTER TABLE ... VALIDATE CONSTRAINT ...` in a
controlled maintenance window. Fresh installations have no unvalidated
constraints.

Migration 1001 then inserts the legacy 2024 EPF, SOCSO, EIS, PCB, and relief
fixtures without claiming they are official schedules; closes legacy absence
rows; safely links employee-only users; creates missing employee portal users
with an unusable generated hash and forced password reset; and backfills company
memberships. It does not overwrite a privileged user merely because an email
address matches.

The retired `system_settings` table was removed: it had no runtime consumer and
its single-key uniqueness contradicted its effective-date columns. Active
tenant configuration lives in `company_settings`; platform-wide flags live in
`platform_settings`.

### Statutory data safety gate

`statutory_rule_sets` records a stable dataset key, domain, effective interval,
status, source document metadata, SHA-256 digest, and verification timestamp.
Its PostgreSQL exclusion constraint permits at most one overlapping **verified**
rule set per domain while retaining retired and prototype history. EPF, SOCSO,
EIS, PCB-bracket, and PCB-relief rows carry a foreign key to the dataset; lookup
queries join through that key and ignore unverified or unlinked rows.

The legacy 2024 rows are registered as `prototype`. Production payroll fails
closed instead of treating missing lookups as zero. Automatic PCB remains
disabled in production builds even if data is marked verified: the current
academic algorithm and input model have not passed LHDN's computerised-MTD
conformance process. Tests opt into the prototype fixture only in test builds.

Before enabling a statutory dataset, import the complete effective-dated source
schedule, attach every row to its rule-set ID, record the exact official URL,
document version and artifact SHA-256, independently verify boundary values and
eligibility logic, then change its status to `verified`. Primary references:

- [KWSP EPF Act Third Schedule index](https://www.kwsp.gov.my/en/epf-act-1991-third-schedule)
- [PERKESO contribution-rate notice](https://www.perkeso.gov.my/en/rate-of-contribution.html)
- [PERKESO foreign-worker coverage](https://www.perkeso.gov.my/en/our-services/protection/foreign-worker)
- [LHDN employer payroll specifications](https://www.hasil.gov.my/en/employers/employer-payroll-data-specification/)
- [LHDN 2026 computerised-MTD specification](https://www.hasil.gov.my/media/arvlrzh5/spesifikasi-kaedah-pengiraan-berkomputer-pcb-2026.pdf)

## Schema design rules

### Identity and relationships

- UUID row identities default to `uuidv7()` for index locality and natural time
  ordering while remaining globally unique.
- `users.employee_id` is unique when present and references `employees` with
  `ON DELETE SET NULL`.
- user deletion provenance references `users` with `ON DELETE SET NULL`.
- bulk-import sessions belong to a user and cascade when that user is removed.
- critical tenant-owned child tables use composite `(parent_id, company_id)`
  foreign keys, so valid IDs from different companies cannot be paired;
- companyless junctions for payroll items, leave balances, and team members use
  same-company constraint triggers.

### Business invariants

The database rejects, among other invalid states:

- multiple non-cancelled payroll runs for the same company/group/month;
- reversed payroll or leave periods and invalid months/days;
- negative statutory ranges, contributions, reliefs, attendance hours, or bulk
  import counters;
- check-out timestamps earlier than check-in;
- invalid coordinates or non-positive geofence radii;
- invalid effective-date ranges and unsupported import-session statuses;
- overlapping verified statutory versions or overlapping contribution/tax bands.

Service-level checks remain necessary for permission and state-machine rules,
but concurrency-sensitive uniqueness belongs in PostgreSQL.

### Index strategy

Indexes are designed around current repository predicates rather than one index
per foreign key:

- a partial unique index protects the active payroll period;
- a `pg_trgm` GIN expression index accelerates case-insensitive combined
  employee-name/number search;
- partial employee, claim, overtime, leave, notification, user, and payroll-entry
  indexes exclude irrelevant states;
- descending period/recent indexes support payroll and attendance screens;
- `INCLUDE` columns cover approved-overtime payroll reads;
- redundant indexes whose leading columns were already covered by unique or
  composite indexes were removed.

Multicolumn dependency/MCV statistics describe correlated tenant/status/period
fields for employees, attendance records, and payroll runs. PostgreSQL 19's
optimizer can use these to avoid independence-assumption errors.

## SQLx offline cache

Compile-time SQLx macros are validated by `backend/.sqlx/` in CI and container
builds. After changing a macro query:

```bash
cd backend
DATABASE_URL=postgres://... cargo sqlx prepare
SQLX_OFFLINE=true cargo check --all-targets
```

The preparation database must already contain migrations 1000 and 1001 and
must match the target PostgreSQL major version.

The application migrator enables SQLx `ignore_missing` only after rejecting
every absent applied version except retired versions 1–4. If the SQLx CLI is
invoked directly against an upgraded database, use its explicit missing-history
option; plain `cargo sqlx migrate run` does not inherit the application's guard.

## Required verification

For schema work, validate all of the following before release:

1. apply both migrations transactionally to an empty PostgreSQL 19 database;
2. apply them to a schema with the retired v1–v4 migration history;
3. run migration 1001 a second time in a disposable database and confirm stable
   reference counts;
4. compare fresh and upgraded catalogs, allowing only expected legacy
   `NOT VALID` flags;
5. confirm all UUID identity defaults are `uuidv7()` and no unexpected
   `uuid-ossp` dependency remains;
6. run integrity queries before validating legacy constraints;
7. regenerate the SQLx cache, compile offline, and run backend tests;
8. take and verify a logical backup before any major-version production move.

## Operational notes

- Do not point PostgreSQL 19 at a PostgreSQL 18 or older data directory.
- Do not run a beta-to-beta package upgrade unattended; catalog compatibility
  can change before final release.
- Keep the previous cluster or a verified logical backup until post-cutover
  application and data checks pass.
- Do not add credentials or organization data to `1001_data.sql`. Development
  fixtures belong in tests. Create the first real administrator with the
  explicit `bootstrap_admin` command documented in the README.
- Effective-dated statutory data must be reviewed and versioned; payroll runs
  should remain reproducible against the rules used at processing time.
