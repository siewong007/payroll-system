//! Read-model modules: multi-table joins and aggregations that belong to no single
//! table, grouped by use-case (reports, dashboard, payslip, ea_form, …). Each module
//! co-locates its bespoke denormalized result structs with the query that builds them.
//!
//! See docs/refactor-repositories-layer.md §6.

pub mod attendance;
pub mod oauth2;
pub mod passkey;
pub mod payroll;
pub mod teams;
pub mod user_management;
