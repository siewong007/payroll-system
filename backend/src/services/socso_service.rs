use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::{SocsoCategory, SocsoContribution};
use crate::services::statutory_rules;
use crate::services::statutory_tables::StatutoryTables;

/// Calculate SOCSO contribution.
///
/// Rules:
/// - First Category: employee < 60 years old
/// - Second Category: employee >= 60 years old (employer-only, employment injury scheme)
/// - Wage ceiling: defined by the effective-dated verified schedule
/// - Foreign-worker rules require scheme-specific eligibility data not yet
///   represented by the employee model, so they fail closed
pub async fn calculate_socso(
    pool: &PgPool,
    wage: i64,
    age: i32,
    is_foreigner: bool,
    effective_date: NaiveDate,
) -> AppResult<SocsoContribution> {
    statutory_rules::require_verified(pool, statutory_rules::SOCSO, effective_date).await?;
    let tables = StatutoryTables::load(pool, effective_date).await?;
    calculate_socso_with(&tables, wage, age, is_foreigner)
}

/// Resolve SOCSO from an already-loaded schedule. See `epf_service::calculate_epf_with`.
pub(crate) fn calculate_socso_with(
    tables: &StatutoryTables,
    wage: i64,
    age: i32,
    is_foreigner: bool,
) -> AppResult<SocsoContribution> {
    if is_foreigner {
        return Err(AppError::Validation(
            "Automatic SOCSO is unavailable for foreign workers until the employee model captures PERKESO entry age and scheme eligibility"
                .into(),
        ));
    }

    if (55..60).contains(&age) {
        return Err(AppError::Validation(
            "Automatic SOCSO is unavailable for employees aged 55-59 until prior PERKESO contribution status is recorded"
                .into(),
        ));
    }

    let category = if age >= 60 {
        SocsoCategory::SecondCategory
    } else {
        SocsoCategory::FirstCategory
    };

    match tables.socso_band(wage) {
        Some(band) => match category {
            SocsoCategory::FirstCategory => Ok(SocsoContribution {
                employee: band.first_cat_employee,
                employer: band.first_cat_employer,
                category,
            }),
            SocsoCategory::SecondCategory => Ok(SocsoContribution {
                employee: 0,
                employer: band.second_cat_employer,
                category,
            }),
            SocsoCategory::Exempt => Ok(SocsoContribution {
                employee: 0,
                employer: 0,
                category,
            }),
        },
        None => Err(AppError::Validation(format!(
            "Verified SOCSO rules contain no contribution band for wage {} sen on {}",
            wage, tables.effective_date
        ))),
    }
}
