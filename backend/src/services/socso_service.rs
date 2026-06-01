use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::AppResult;
use crate::repositories::socso_rates;

/// SOCSO contribution result
#[derive(Debug, Clone)]
pub struct SocsoContribution {
    pub employee: i64, // in sen (0 for Second Category)
    pub employer: i64, // in sen
    pub category: SocsoCategory,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SocsoCategory {
    FirstCategory,  // Employment Injury + Invalidity (< 60)
    SecondCategory, // Employment Injury only (>= 60)
    Exempt,         // Not applicable (foreigner, etc.)
}

/// Calculate SOCSO contribution.
///
/// Rules:
/// - First Category: employee < 60 years old
/// - Second Category: employee >= 60 years old (employer-only, employment injury scheme)
/// - Wage ceiling: RM6,000/month (600000 sen)
/// - Foreigners: exempt from SOCSO
pub async fn calculate_socso(
    pool: &PgPool,
    wage: i64,
    age: i32,
    is_foreigner: bool,
    effective_date: NaiveDate,
) -> AppResult<SocsoContribution> {
    if is_foreigner {
        return Ok(SocsoContribution {
            employee: 0,
            employer: 0,
            category: SocsoCategory::Exempt,
        });
    }

    // Cap wage at ceiling (RM6,000 = 600000 sen)
    let capped_wage = wage.min(600000);

    let category = if age >= 60 {
        SocsoCategory::SecondCategory
    } else {
        SocsoCategory::FirstCategory
    };

    let rate = socso_rates::find_rate(pool, capped_wage, effective_date).await?;

    match rate {
        Some(r) => match category {
            SocsoCategory::FirstCategory => Ok(SocsoContribution {
                employee: r.first_cat_employee,
                employer: r.first_cat_employer,
                category,
            }),
            SocsoCategory::SecondCategory => Ok(SocsoContribution {
                employee: 0,
                employer: r.second_cat_employer,
                category,
            }),
            SocsoCategory::Exempt => Ok(SocsoContribution {
                employee: 0,
                employer: 0,
                category,
            }),
        },
        None => Ok(SocsoContribution {
            employee: 0,
            employer: 0,
            category,
        }),
    }
}
