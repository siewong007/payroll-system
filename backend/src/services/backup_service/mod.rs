//! Whole-company backup export/import service.
//!
//! The public API is unchanged for callers (`backup_service::export_company`
//! and `backup_service::import_company`), while the implementation is grouped by
//! export/import workflow.

mod export;
mod files;
mod import;

pub use export::export_company;
pub use import::import_company;
