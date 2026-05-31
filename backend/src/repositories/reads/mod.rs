//! Read-model modules: multi-table joins and aggregations that belong to no single
//! table, grouped by use-case (reports, dashboard, payslip, ea_form, …). Each module
//! co-locates its bespoke denormalized result structs with the query that builds them.
//!
//! See docs/refactor-repositories-layer.md §6.

pub mod user_management;
