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

pub mod attendance_kiosk_credentials;
pub mod attendance_qr_tokens;
pub mod attendance_records;
pub mod audit_logs;
pub mod clock;
pub mod companies;
pub mod company_locations;
pub mod company_settings;
pub mod company_work_schedules;
pub mod documents;
pub mod employee_work_schedules;
pub mod employees;
pub mod holidays;
pub mod oauth2_accounts;
pub mod oauth2_states;
pub mod passkey_challenges;
pub mod passkey_credentials;
pub mod password_reset_requests;
pub mod payroll_groups;
pub mod platform_settings;
pub mod refresh_tokens;
pub mod salary_history;
pub mod team_members;
pub mod teams;
pub mod tp3_records;
pub mod user_companies;
pub mod users;
pub mod working_day_config;

pub mod reads;
