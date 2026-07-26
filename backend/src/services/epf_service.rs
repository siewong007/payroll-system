use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::EpfContribution;
use crate::services::statutory_rules;
use crate::services::statutory_tables::StatutoryTables;

/// Look up EPF contribution from the Third Schedule table.
///
/// The verified rule set must contain an exact matching band. Percentage
/// fallbacks are intentionally rejected because official EPF parts have
/// different eligibility and rounding rules.
pub async fn calculate_epf(
    pool: &PgPool,
    wage: i64,      // monthly wage in sen
    category: &str, // A, B, C, D
    effective_date: NaiveDate,
) -> AppResult<EpfContribution> {
    statutory_rules::require_verified(pool, statutory_rules::EPF, effective_date).await?;
    let tables = StatutoryTables::load(pool, effective_date).await?;
    calculate_epf_with(&tables, wage, category)
}

/// Resolve EPF from an already-loaded schedule.
///
/// Pure, so a payroll run calls it once per employee without touching the
/// database; `calculate_epf` is the one-off path that loads a snapshot first.
pub(crate) fn calculate_epf_with(
    tables: &StatutoryTables,
    wage: i64,
    category: &str,
) -> AppResult<EpfContribution> {
    if !matches!(category, "A" | "B" | "C" | "D" | "E") {
        return Err(AppError::BadRequest(format!(
            "Invalid EPF category: {}",
            category
        )));
    }

    match tables.epf_band(category, wage) {
        Some(band) => Ok(EpfContribution {
            employee: band.employee_contribution,
            employer: band.employer_contribution,
        }),
        None => Err(AppError::Validation(format!(
            "Verified EPF rules contain no contribution band for category {} and wage {} sen on {}",
            category, wage, tables.effective_date
        ))),
    }
}
