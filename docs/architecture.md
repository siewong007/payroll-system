# System architecture

This is the living architecture reference for the implemented payroll and HR
system. Feature completeness and known UI gaps are tracked separately in
[features.md](features.md); the database baseline and upgrade contract are in
[database.md](database.md).

## System context

```mermaid
flowchart LR
    Browser["Browser / employee device / kiosk"] --> Edge["Vite proxy or CloudFront"]
    Edge --> API["Axum API /api/*"]
    API --> Handlers["Handlers"]
    Handlers --> Services["Domain services"]
    Services --> Repositories["Repositories and read models"]
    Repositories --> Database[("PostgreSQL 19 Beta 2")]
    Services --> Files["Local uploads and generated exports"]
    Services --> SMTP["SMTP provider"]
    Jobs["In-process scheduled jobs"] --> Services
```

The React single-page application calls one Axum API. During development Vite
proxies `/api`; the AWS design serves the frontend through CloudFront and runs
the API separately. All backend routes are declared in
`backend/src/routes/mod.rs`.

## Backend boundaries

| Layer | Responsibility |
| --- | --- |
| `handlers/` | HTTP extraction, authentication context, permission checks, validation, and response mapping |
| `services/` | Business rules, workflow orchestration, transaction boundaries, and audit intent |
| `repositories/` | One-table SQL operations, generally generic over a SQLx executor |
| `repositories/reads/` | Cross-table projections, reports, and other denormalized read models |
| `models/` | API/domain structures and SQLx result projections |
| `core/` | Configuration, database startup, JWT extraction, cookies, shared state, and error mapping |

Request-time SQL belongs in repositories. A service may pass either a pool or
an open transaction to repository functions. `AppResult<T>` is the common
error boundary; database errors convert to `AppError::Database`, while internal
details are logged rather than exposed in HTTP 500 responses.

The API process also owns two operational jobs:

- every 24 hours, cleanup of expired or revoked refresh tokens and of expired
  attendance QR tokens that no check-in references;
- a daily attendance evaluation at 12:30 PM Asia/Kuala_Lumpur (and once at
  startup) that marks eligible employees absent, excluding approved leave,
  non-working days, and public holidays. Each run catches up every due local
  date since the last bookmarked success, so a window missed during a deploy is
  recovered instead of skipped permanently.

These jobs are suitable for the current single API instance. Multiple replicas
would need a database lease, leader election, or a separate worker.

## Identity, roles, and tenancy

Access tokens are signed JWTs sent as bearer tokens. Refresh tokens are opaque,
rotated, hashed in PostgreSQL, and carried by an httpOnly cookie. The frontend
keeps the access token in memory; local storage contains only a display/session
hint and is not an authorization source.

Canonical roles are `super_admin`, `admin`, `payroll_admin`, `hr_manager`,
`finance`, `exec`, and `employee`. Payroll actions use explicit permissions:

| Action | Roles |
| --- | --- |
| View payroll | `super_admin`, `payroll_admin`, `finance` |
| Prepare and edit a draft | `super_admin`, `payroll_admin` |
| Submit for approval | `super_admin`, `payroll_admin` |
| Approve or return | `super_admin`, `finance` |
| Mark paid | `super_admin`, `finance` |

Company users carry an active `company_id` claim. Repository calls for
company-owned data are expected to include it. A multi-company user switches
context through `PUT /api/auth/switch-company`, which issues a new JWT instead
of trusting a client-supplied tenant on every request. PostgreSQL row-level
security is not currently used, so application-level tenant predicates and
authorization tests remain critical.

A fresh database contains no login credential. The explicit one-time
`bootstrap_admin` binary creates the initial company and super administrator,
serializes concurrent attempts, and refuses to run once an active super admin
exists.

## Payroll workflow

```mermaid
stateDiagram-v2
    [*] --> processing: create run
    processing --> processed: calculations committed
    processed --> pending_approval: submit
    pending_approval --> approved: finance approval
    pending_approval --> processed: return for changes
    approved --> paid: lock / mark paid
```

After a statutory preflight succeeds, the payroll engine executes in a
transaction and composes EPF, SOCSO, EIS, and PCB services for every eligible
employee. Money is stored as integer sen in the
database and represented with exact decimal types where fractional arithmetic
is required; payroll code must never use binary floating point.

Calculation is separated from persistence. The engine reads every applicable
statutory band, bracket, and relief once per run into an in-memory snapshot, and
the four calculators are pure functions over it, so resolving an employee costs
no database round trips. Every employee is computed before any payslip is
written: a run that cannot calculate some of its employees reports all of them
together and commits nothing. The same calculation is exposed as a preview that
writes nothing, so an operator reviews the projected payslips and any problems
before committing.

Each committed payslip stores the lines that explain it — the named allowances,
the overtime hours and rate multipliers, and each statutory deduction — and each
run records the verified rule sets and overtime configuration it was calculated
from. Both inputs are mutable and effective-dated, so without that record a
later rule import or settings change would leave historical figures
unreproducible.

The database enforces at most one non-cancelled run for a company, payroll
group, year, and month. This closes the race between the service's existence
check and concurrent inserts. A processed run permits controlled PCB edits;
submission, approval, return, payment, and eligible-run deletion each verify company scope
and legal state transitions. Outputs include payslip PDFs, batch PDFs, reports,
EA forms, and statutory export files.

Every statutory lookup row is linked to a `statutory_rule_sets` record with an
effective interval and source-verification metadata. Prototype or unlinked rows
are never eligible for automatic calculations, and missing data raises a visible
validation error instead of silently returning zero. The inherited 2024 rows in
`1001_data.sql` are explicitly unverified academic fixtures. Automatic PCB is
also disabled in production until its algorithm and input model pass LHDN
computerised-MTD conformance testing.

Tenant ownership is also enforced below the service layer for critical paths:
attendance, payroll-run/group/entry, user/employee, claims, leave, overtime,
documents, and employee schedules use company-qualified foreign keys. Payroll
items, leave balances, and team membership lack a stored company column, so
write-time constraint triggers compare their parents' companies.

## Attendance workflow

Attendance supports authenticated employee check-in, public kiosk display, and
HR administration.

- QR tokens are multi-use for their server-returned TTL. The `used` flag means
  administratively revoked, not consumed by an employee scan. Revocation is
  scoped per display surface, so kiosks never retire each other's codes.
- Check-out selects the most recent open record within 24 hours so overnight
  shifts do not break at midnight. Nothing matching but an older open session
  present is reported as a stale session needing an admin correction, rather
  than as "no check-in" — the latter sent employees in a loop between the
  check-out and check-in errors. Geofence never blocks a check-out; an off-site
  one is recorded and flagged for review.
- Geofence mode can be `none`, `warn`, or `enforce`; database checks protect
  latitude, longitude, and positive radii. The mode is published to clients so
  they skip the location prompt when it is `none`.
- The summary read model left-joins employees, so employees with no attendance
  rows still appear. Counts aggregate distinct local days (with precedence
  late > half_day > present > absent), so a split shift or an absence
  superseded by a later check-in is not double-counted. The same filters feed
  CSV export, which defaults to the current month when given no date range.
- Date filters compare the raw `timestamptz` against local-midnight bounds and
  take the company timezone as a parameter, keeping them index-usable.
- Manual records and corrections are HR/admin operations and are audit logged.
  A correction requires a reason, uses explicit clear flags rather than
  treating an omitted field as a clear, and writes its audit row in the same
  transaction as the update.
- Face ID check-in issues a per-check-in WebAuthn challenge and verifies the
  assertion server-side against the employee's registered passkeys before
  writing the record.

## Employee and approval workflows

Employee administration covers core employment, contact, payroll, bank,
statutory, salary-history, TP3, work-schedule, and portal-account data. Bulk
import is a two-phase workflow: validate an uploaded CSV/XLSX file, persist a
short-lived validation session, then confirm the accepted rows.

Employees can submit leave, claims, and overtime from the portal. HR-facing
approval screens can create, edit, approve, reject, cancel, and review those
records according to the relevant service rules. Leave balances, team calendars,
notifications, attachments, and audit records connect these workflows.

Company creation transactionally provisions the minimum configuration needed
for these flows: a default payroll group, standard leave types, Monday-Friday
working days, a default work schedule, and editable company settings. The same
idempotent database function repairs missing setup in older companies/backups.

## Frontend structure

`frontend/src/App.tsx` defines two authenticated shells:

- `AppLayout` for company administration, HR, payroll, reports, and operations;
- `PortalLayout` for employee profile, payslips, leave, claims, overtime,
  calendar, notifications, and attendance history.

Public routes handle login/password recovery and attendance kiosk/scan flows.
Role guards control navigation and page entry, while the API remains the
authoritative permission boundary. All API modules use the single Axios client
in `frontend/src/api/client.ts`. Its 401 interceptor performs one queued refresh
for concurrent failures, then clears the session if refresh fails. React Query
defaults to one retry, a 30-second stale time, and no focus refetch.

## Data and deployment

The canonical database consists of one schema migration and one reference-data
migration. It uses native UUIDv7 identifiers, targeted partial/covering/trigram
indexes, relational constraints, and multicolumn optimizer statistics. See
[database.md](database.md) for exact compatibility and verification rules.

Docker Compose and CI pin PostgreSQL 19 Beta 2. The Terraform in `infra/`
provisions only the frontend delivery path and the deploy identity: an S3
bucket (private, versioned, public access blocked), a CloudFront distribution
with an origin access control and response-headers policy, an ACM certificate,
Route53 records, and an IAM OIDC provider plus deploy role. There is no VPC,
EC2, ECR, Secrets Manager or RDS module in this repository — the API and
database run as containers on the Lightsail host, whose deployment record is in
[lightsail-pg19-beta2-upgrade-record.md](lightsail-pg19-beta2-upgrade-record.md).

Uploads are written to the API container's local `uploads/` directory; there is
no S3 upload bucket. SMTP, OAuth2, WebAuthn origins, and cloud services require
environment-specific configuration.

## Architectural invariants

- Keep handlers thin and put business rules in services.
- Keep request-time SQL in repositories or read-model modules.
- Require company scope for every tenant-owned identifier lookup.
- Use exact decimal/integer money representations, never `f64`.
- Interpret business dates and schedules in Asia/Kuala_Lumpur; store instants
  as UTC-aware timestamps.
- Preserve payroll and attendance state-machine checks at both service and
  database boundaries.
- Use the existing frontend HTTP client and role model rather than introducing
  parallel authentication state.
