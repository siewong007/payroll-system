use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::EpfContribution;
use crate::repositories::epf_rates;
use crate::services::statutory_rules;

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
    calculate_epf_after_preflight(pool, wage, category, effective_date).await
}

pub(crate) async fn calculate_epf_after_preflight(
    pool: &PgPool,
    wage: i64,
    category: &str,
    effective_date: NaiveDate,
) -> AppResult<EpfContribution> {
    if !matches!(category, "A" | "B" | "C" | "D" | "E") {
        return Err(AppError::BadRequest(format!(
            "Invalid EPF category: {}",
            category
        )));
    }

    // Try table lookup first
    let rate = epf_rates::find_contribution(pool, category, wage, effective_date).await?;

    match rate {
        Some(r) => Ok(EpfContribution {
            employee: r.employee_contribution,
            employer: r.employer_contribution,
        }),
        None => Err(AppError::Validation(format!(
            "Verified EPF rules contain no contribution band for category {} and wage {} sen on {}",
            category, wage, effective_date
        ))),
    }
}
