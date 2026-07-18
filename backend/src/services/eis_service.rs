use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::EisContribution;
use crate::repositories::eis_rates;
use crate::services::statutory_rules;

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
    calculate_eis_after_preflight(pool, wage, age, is_foreigner, effective_date).await
}

pub(crate) async fn calculate_eis_after_preflight(
    pool: &PgPool,
    wage: i64,
    age: i32,
    is_foreigner: bool,
    effective_date: NaiveDate,
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

    let rate = eis_rates::find_rate(pool, wage, effective_date).await?;

    match rate {
        Some(r) => Ok(EisContribution {
            employee: r.employee_contribution,
            employer: r.employer_contribution,
        }),
        None => Err(AppError::Validation(format!(
            "Verified EIS rules contain no contribution band for wage {} sen on {}",
            wage, effective_date
        ))),
    }
}
