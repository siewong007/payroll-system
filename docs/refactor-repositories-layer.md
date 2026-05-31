# Refactor: introduce a `repositories/` data-access layer

Status: **planned** · Driver: separation & consistency · Target: `backend/`

## 1. Context

Today, SQL is **not** where CLAUDE.md says it is:

- `models/` files are **pure structs** (table-row structs + request DTOs). They contain no SQL. The one exception is `models/attendance_kiosk.rs` (5 inline `query!` calls).
- **All other SQL lives in `services/`** (≈360 `query!`/`query_as!`/`query_scalar!` calls) and, in violation of the "thin handler" doctrine, in **`handlers/`** (≈38 calls — `payroll.rs` alone has 22, plus `auth.rs`, `passkey.rs`, `dashboard.rs`, `oauth2.rs`, `portal.rs`).
- The runtime→macro migration is effectively **done** (400 macro calls vs 17 runtime, the 17 being test setup + `core/db.rs` migrate + the health `SELECT 1`).

So CLAUDE.md is stale on three points: "models hold queries" (false), "`repositories/` exists but is empty" (it does not exist), and "macro migration is incremental" (it's complete).

### Smells this refactor targets
1. **Fat services** — `backup_service` has 69 queries, `portal_service` 34, `employee_service`/`attendance_service`/`user_service` 19 each.
2. **SQL ⇄ business logic tangled** — e.g. `employee_service::create_employee` interleaves a dup-check query, an INSERT, `bcrypt`, writes to `users`/`user_companies`, and calls to `portal_service` + `audit_service`.
3. **Duplicated projections** — the 60-column `Employee` SELECT/RETURNING list (carrying `gender::text AS "gender?"`-style enum casts) is copy-pasted **4×** in one file.

## 2. Goal & non-goals

**Goal:** move all SQL out of `services/` and `handlers/` into a dedicated `repositories/` layer, leaving services as orchestration and `models/` as shared structs. **Behavior-preserving** — no logic, query text, or transaction behavior changes during the move.

**Non-goals (explicitly deferred — see §10):** introducing transactions, adopting Rust enum types, any automated enforcement tooling.

## 3. Target layering

```
handler   thin: extract AuthUser, parse JSON, call a service, map response  — NEVER touches the data layer
  └─ service   business logic, orchestration, NotFound/Conflict mapping, bcrypt, audit, cross-service calls, tx boundary
       └─ repositories   pure data access (one module per table) + repositories/reads/ (joins / aggregations)
            └─ Postgres

models/   shared structs: canonical table-row structs + request DTOs
```

## 4. Design decisions (locked)

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| 1 | Form | Plain free-function modules — **no traits, no DI**, `AppState` unchanged | Driver is separation, not testability; sqlx already compile-checks queries |
| 2 | Repo boundary | **Thin data-mapper**: one fn = one logical DB operation (one statement). No business branching, no `bcrypt`, no cross-service calls, no audit, no `NotFound`/`Conflict` | Crispest rule; scales mechanically to ~400 sites |
| 3 | Returns / errors | Reads → `AppResult<Option<T>>` / `AppResult<Vec<T>>`; mutations → `AppResult<u64>` (rows-affected) or `AppResult<Option<T>>`; `INSERT…RETURNING` → `AppResult<T>`. **Service** maps absence→`NotFound`, dup→`Conflict`. Keep `AppResult` (so `?` works via `#[from] sqlx::Error`) | A missing row is sometimes valid; that decision is the service's |
| 4 | DB handle | Generic over `impl sqlx::Executor<'_, Database = sqlx::Postgres>` | Pass `&pool` normally; `&mut *tx` to compose atomically later. One-statement-per-fn means "executor consumed once" never bites |
| 5 | Granularity | **One repo per table** — `repositories/<table>.rs`, named for the table (plural): `employees.rs`, `salary_history.rs`, `users.rs`, … (~48 modules) | Purest data-access split |
| 6 | Joins (89) | **Read-model modules** in `repositories/reads/<usecase>.rs` (CQRS-lite). Per-table repos stay pure single-table CRUD | A join belongs to no single table |
| 7 | Layering | **Strict**: handler SQL is in scope and moves into services; handlers never touch the data layer. Trivial reads get a thin pass-through service fn | Matches the documented thin-handler doctrine; one rule |
| 8 | Column-list duplication | **Accept + co-locate** in the per-table repo (with a sync comment). Real fix (enum types) is deferred | Keeps this refactor bounded to relocation; no serde/frontend churn |
| 9 | Read-model structs | **Co-located** in the read module that produces them. `models/` stays table-rows-only | Read-models are use-case DTOs; keeps `models/`↔tables clean |
| 10 | Sequencing | **Big-bang single PR**, structured as **one commit per table-domain**, with the **`employees` slice first** as a checkpoint | Solo repo (no merge-conflict risk) + behavior-preserving + cache-neutral; commit structure keeps it reviewable |
| 11 | Enforcement | **Convention + review only**, documented in CLAUDE.md (no automated guard) | Owner's call. ⚠️ Risk: the codebase already drifted (38 handler queries); nothing mechanical prevents recurrence |
| 12 | Insert/update params | Repo fns take the **existing `models/` request DTOs** (`CreateEmployeeRequest`, `UpdateEmployeeRequest`); server-set values (`id`, `created_by`) as separate args | SQL is already written against `req.field`; near-literal move, zero new structs |

## 5. Conventions

- **File:** `repositories/<table_name_plural>.rs`. Read-models: `repositories/reads/<usecase>.rs`.
- **Function verbs:** `get` (by id → `Option`), `list` (→ `Vec`), `insert`, `update`, `soft_delete` / `delete`, `exists_*` (→ `bool`), `count_*` (→ `i64`).
- **Signature shape:**
  ```rust
  pub async fn get(
      ex: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
      id: Uuid,
      company_id: Uuid,
  ) -> AppResult<Option<Employee>> { /* fetch_optional */ }
  ```
- **Return structs:** per-table repos return the **existing grouped `models/` structs** (e.g. `repositories/salary_history.rs` → `models::employee::SalaryHistory`). `models/` is **not** split per table.
- **Read-model structs** are defined in the read module next to the query.

## 6. Read-model modules (`repositories/reads/`)

The 89 join/aggregation queries move here, grouped by use-case rather than table. Initial set:
`reads/reports.rs`, `reads/dashboard.rs`, `reads/payslip.rs`, `reads/ea_form.rs`, `reads/statutory_export.rs`, `reads/backup.rs`, plus the mixed-domain reads pulled from `portal`, `approval_service/*`, `document`, `attendance`. Each defines its own denormalized result structs.

## 7. What moves / special cases

- `models/attendance_kiosk.rs` (the lone model with 5 inline queries) → move them into the appropriate repo(s).
- Rate/lookup tables (`epf_rates`, `socso_rates`, `eis_rates`, `pcb_brackets`, `pcb_reliefs`) get tiny per-table repos; the calculator services call them.
- **Stays put (infra, not domain SQL):** `core/db.rs` migrate, the health `SELECT 1`, and `src/tests/` runtime-`query` setup.

## 8. Execution plan

Big-bang single PR, **one commit per table-domain**, each commit keeping `cargo build`, `cargo clippy -- -D warnings`, `cargo fmt --check`, and `cargo test` green.

**Cache-neutral:** the `.sqlx` offline cache is keyed by query *text* (hashed), not file location. Relocating a query verbatim — or splitting a multi-statement fn into separate repo fns whose individual statements are unchanged — reuses the same cache entries. **No `cargo sqlx prepare` is needed** unless query text changes (and decision #8 ensures it won't).

### Sequencing note: tables are shared across domains
With per-table repos, a single table is touched by many domains (`employees` by 22 files, `users` by 17). So a per-table repo file is **built incrementally**: each domain commit adds the functions *that domain needs* and switches *its own* call sites. `repositories/employees.rs` grows across several commits; that's expected, not a smell.

Suggested commit order (after the `employees` checkpoint): user/auth (`users`, `user_companies`, `refresh_tokens`, sessions, passkey, oauth2, password_reset) → company/settings/team → attendance (+ kiosk/qr/work_schedule/geofence/calendar) → payroll (runs/items/entries/details/groups + rate tables; lift `handlers/payroll.rs`'s 22 queries into a service) → approval (leave/overtime/claim) → documents/notifications/email → `reads/` modules → final CLAUDE.md update.

## 9. The `employees` slice (first commit — pattern checkpoint)

Scope = the **`employee_service` vertical**, end-to-end. Not "the whole `employees` table" (its other 21 callers come in their own commits and will reuse these repo files).

**Create repo files (populated with only what `employee_service` needs):**

| File | Functions (from `employee_service`) |
|------|-------------------------------------|
| `repositories/employees.rs` | `exists_by_number`, `get`, `list` (+ count), `insert`, `update`, `soft_delete` |
| `repositories/salary_history.rs` | `insert` (salary-change row), `list_by_employee` |
| `repositories/tp3_records.rs` | `upsert` (the `ON CONFLICT` create_tp3) |
| `repositories/users.rs` *(partial)* | `find_by_email`, `link_to_employee`, `insert_employee_user`, `delete` |
| `repositories/user_companies.rs` *(partial)* | `insert`, `delete_by_user` |
| `repositories/refresh_tokens.rs` *(partial)* | `delete_by_user` |

**Refactor:**
- `services/employee_service.rs` → orchestration only. Every `sqlx::` call replaced by a repo call. Keeps: the `Conflict` dup-check decision, `NotFound` mapping, `bcrypt`, the existing-user branching in `create_user_for_employee`, the `portal_service` + `audit_service` calls.
- `handlers/employee.rs` → **no change** (already thin — confirms the target shape).

**Wire up:** add `repositories/mod.rs` (declare the new modules + `pub mod reads;`) and `pub mod repositories;` in `lib.rs`.

**Verify:** build + clippy + fmt + test green; confirm `git diff backend/.sqlx` is empty (cache-neutral).

**Out of this slice:** `employee_allowances` (not touched by `employee_service` — comes with company/payroll commits, even though `EmployeeAllowance` lives in `models/employee.rs`); all other callers of these tables; all `reads/` modules.

### Worked example
```rust
// repositories/employees.rs  — pure data access, executor-generic
pub async fn exists_by_number(ex: impl Executor<'_, Database = Postgres>, company_id: Uuid, number: &str) -> AppResult<bool> { … }
pub async fn get(ex: …, id: Uuid, company_id: Uuid) -> AppResult<Option<Employee>> { … }            // fetch_optional
pub async fn insert(ex: …, company_id: Uuid, req: &CreateEmployeeRequest, id: Uuid, by: Uuid) -> AppResult<Employee> { … }  // RETURNING
pub async fn soft_delete(ex: …, id: Uuid, company_id: Uuid) -> AppResult<u64> { … }                 // rows_affected

// services/employee_service.rs  — orchestration; owns the 409/404 + side effects
pub async fn create_employee(pool, company_id, req, by, meta) -> AppResult<(Employee, Option<EmployeeAccountInfo>)> {
    if repositories::employees::exists_by_number(pool, company_id, &req.employee_number).await? {
        return Err(AppError::Conflict(format!("Employee number '{}' already exists…", req.employee_number)));
    }
    let emp = repositories::employees::insert(pool, company_id, &req, Uuid::now_v7(), by).await?;
    let account = create_user_for_employee(pool, &emp).await?;   // uses users / user_companies repos + bcrypt
    let _ = portal_service::initialize_leave_balances(…).await;  // best-effort, outside any tx
    let _ = audit_service::log_action_with_metadata(…).await;    // best-effort
    Ok((emp, account))
}
```

## 10. Deferred follow-ups (separate PRs)

1. **Enum types** — add `#[derive(sqlx::Type)]` enums for `gender`/`race`/`residency_status`/`marital_status`/`employment_type`, drop the `::text` casts, collapse projections to `SELECT *`/plain INSERT (kills the duplication at the root). Touches serde output + `frontend/src/types`.
2. **Transactions** — use the executor-generic signatures to wrap multi-write flows (e.g. fix `create_employee`'s orphan-on-failure: employee + user inserts in one tx, best-effort calls outside).
3. **Automated enforcement** — a grep test/CI step forbidding `sqlx::query*` outside `repositories/`, if drift recurs.

## 11. CLAUDE.md changes required (in the final commit)

- Replace "`models/` — data structs and sqlx queries" / "model files hold queries today" with: `models/` = shared structs; **all SQL lives in `repositories/`** (one module per table) **+ `repositories/reads/`** (joins/aggregations).
- Remove "`repositories/` exists but is currently empty."
- Update the request-flow line to `handler → service → repositories → Postgres`.
- Document the convention: **no `sqlx::query*` outside `repositories/`** (handlers and services call repos, never SQL directly); thin data-mapper rule; executor-generic signatures; return-type/error-mapping rule.
- Drop the "other modules still use runtime `query` and are being migrated" note.
