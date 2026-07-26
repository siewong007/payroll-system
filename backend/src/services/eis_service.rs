use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::EisContribution;
use crate::services::statutory_rules;
use crate::services::statutory_tables::StatutoryTables;

/// Calculate EIS contribution.
///
/// Rules:
/// - 0.2% employee + 0.2% employer
/// - Wage ceiling: defined by the effective-dated verified schedule
/// - Ages 57-59 require prior-contribution eligibility data and fail closed
/// - Malaysian citizens and permanent residents only
/// - Foreigners: exempt
pub async fn calculate_eis(
    pool: &PgPool,
    wage: i64,
    age: i32,
    is_foreigner: bool,
    effective_date: NaiveDate,
) -> AppResult<EisContribution> {
    statutory_rules::require_verified(pool, statutory_rules::EIS, effective_date).await?;
    let tables = StatutoryTables::load(pool, effective_date).await?;
    calculate_eis_with(&tables, wage, age, is_foreigner)
}

/// Resolve EIS from an already-loaded schedule. See `epf_service::calculate_epf_with`.
pub(crate) fn calculate_eis_with(
    tables: &StatutoryTables,
    wage: i64,
    age: i32,
    is_foreigner: bool,
) -> AppResult<EisContribution> {
    if is_foreigner || age >= 60 {
        return Ok(EisContribution {
            employee: 0,
            employer: 0,
        });
    }

    if age >= 57 {
        return Err(AppError::Validation(
            "Automatic EIS is unavailable for employees aged 57-59 until prior EIS contribution status is recorded"
                .into(),
        ));
    }

    match tables.eis_band(wage) {
        Some(band) => Ok(EisContribution {
            employee: band.employee_contribution,
            employer: band.employer_contribution,
        }),
        None => Err(AppError::Validation(format!(
            "Verified EIS rules contain no contribution band for wage {} sen on {}",
            wage, tables.effective_date
        ))),
    }
}
