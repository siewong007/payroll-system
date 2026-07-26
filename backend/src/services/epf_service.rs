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
///
/// `contributable_wage` is EPF-contributable wages in sen as defined by EPF Act
/// 1991 s.2 — overtime, gratuity, travelling allowance, service charge and
/// payment in lieu of notice are NOT part of it. Callers must not pass the
/// payslip's gross, which includes overtime: SOCSO and EIS rate on that figure,
/// EPF does not.
pub async fn calculate_epf(
    pool: &PgPool,
    contributable_wage: i64, // monthly EPF-contributable wage in sen
    category: &str,          // A, B, C, D
    effective_date: NaiveDate,
) -> AppResult<EpfContribution> {
    statutory_rules::require_verified(pool, statutory_rules::EPF, effective_date).await?;
    let tables = StatutoryTables::load(pool, effective_date).await?;
    calculate_epf_with(&tables, contributable_wage, category)
}

/// Resolve EPF from an already-loaded schedule.
///
/// Pure, so a payroll run calls it once per employee without touching the
/// database; `calculate_epf` is the one-off path that loads a snapshot first.
///
/// `contributable_wage` carries the same EPF Act 1991 s.2 meaning as on
/// `calculate_epf` — not the payslip gross.
pub(crate) fn calculate_epf_with(
    tables: &StatutoryTables,
    contributable_wage: i64,
    category: &str,
) -> AppResult<EpfContribution> {
    if !matches!(category, "A" | "B" | "C" | "D" | "E") {
        return Err(AppError::BadRequest(format!(
            "Invalid EPF category: {}",
            category
        )));
    }

    match tables.epf_band(category, contributable_wage) {
        Some(band) => Ok(EpfContribution {
            employee: band.employee_contribution,
            employer: band.employer_contribution,
        }),
        None => Err(AppError::Validation(format!(
            "Verified EPF rules contain no contribution band for category {} and wage {} sen on {}",
            category, contributable_wage, tables.effective_date
        ))),
    }
}
