//! Read-model modules: multi-table joins and aggregations that belong to no single
//! table, grouped by use-case (reports, dashboard, payslip, ea_form, …). Each module
//! co-locates its bespoke denormalized result structs with the query that builds them.
//!
//! See `docs/architecture.md` for the data-access boundaries.

pub mod approvals;
pub mod attendance;
pub mod audit;
pub mod dashboard;
pub mod ea_form;
pub mod oauth2;
pub mod passkey;
pub mod payroll;
pub mod payslip;
pub mod portal;
pub mod reports;
pub mod statutory;
pub mod system;
pub mod teams;
pub mod user_management;
