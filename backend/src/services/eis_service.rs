use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::AppResult;
use crate::repositories::eis_rates;

/// EIS contribution result
#[derive(Debug, Clone)]
pub struct EisContribution {
    pub employee: i64, // in sen
    pub employer: i64, // in sen
}

/// Calculate EIS contribution.
///
/// Rules:
/// - 0.2% employee + 0.2% employer
/// - Wage ceiling: RM5,000/month (500000 sen)
/// - Not applicable for employees aged 57+
/// - Malaysian citizens and permanent residents only
/// - Foreigners: exempt
pub async fn calculate_eis(
    pool: &PgPool,
    wage: i64,
    age: i32,
    is_foreigner: bool,
    effective_date: NaiveDate,
) -> AppResult<EisContribution> {
    // Exempt if foreigner or age >= 57
    if is_foreigner || age >= 57 {
        return Ok(EisContribution {
            employee: 0,
            employer: 0,
        });
    }

    // Cap wage at ceiling (RM5,000 = 500000 sen)
    let capped_wage = wage.min(500000);

    let rate = eis_rates::find_rate(pool, capped_wage, effective_date).await?;

    match rate {
        Some(r) => Ok(EisContribution {
            employee: r.employee_contribution,
            employer: r.employer_contribution,
        }),
        None => Ok(EisContribution {
            employee: 0,
            employer: 0,
        }),
    }
}
