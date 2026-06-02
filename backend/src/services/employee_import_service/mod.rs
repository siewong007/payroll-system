//! Bulk employee import service.
//!
//! Split by workflow to keep parsing, validation, confirmation, and template
//! generation navigable while preserving the old public API.

mod confirm;
mod parsing;
mod template;
mod validation;
mod values;

pub use confirm::confirm_import;
pub use parsing::{parse_csv, parse_xlsx};
pub use template::{generate_template_csv, generate_template_xlsx};
pub use validation::validate_file;
