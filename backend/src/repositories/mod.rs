//! Data-access layer: one module per table holding thin, single-statement query
//! functions, plus `reads/` for cross-table joins/aggregations.
//!
//! Conventions (see docs/refactor-repositories-layer.md):
//! - One logical DB operation per function; no business logic, no cross-service calls.
//! - Functions are generic over `impl sqlx::Executor<'_, Database = Postgres>` so a
//!   service can pass `&pool` or compose several calls in one `&mut tx`.
//! - Reads return `Option`/`Vec`; mutations return rows-affected or the `RETURNING`
//!   row. Mapping absence to `NotFound`/`Conflict` is the service's job.
//! - All SQL lives here; handlers and services never embed `sqlx::query*` directly.

pub mod employees;
pub mod refresh_tokens;
pub mod salary_history;
pub mod tp3_records;
pub mod user_companies;
pub mod users;

pub mod reads;
