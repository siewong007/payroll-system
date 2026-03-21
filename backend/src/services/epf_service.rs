use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};

/// EPF contribution result
#[derive(Debug, Clone)]
pub struct EpfContribution {
    pub employee: i64, // in sen
    pub employer: i64, // in sen
}

/// Look up EPF contribution from the Third Schedule table.
///
/// For wages above the table range, compute using flat percentage:
/// - Category A: 11% employee, 13% (<=RM5000) or 12% (>RM5000) employer
/// - Category D (citizen >= 60): 0% employee, employer varies
pub async fn calculate_epf(
    pool: &PgPool,
    wage: i64,              // monthly wage in sen
    category: &str,         // A, B, C, D
    effective_date: NaiveDate,
) -> AppResult<EpfContribution> {
    // Try table lookup first
    let rate = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT employee_contribution, employer_contribution
        FROM epf_rates
        WHERE category = $1
          AND wage_from <= $2 AND wage_to >= $2
          AND effective_from <= $3
          AND (effective_to IS NULL OR effective_to >= $3)
        ORDER BY effective_from DESC
        LIMIT 1
        "#,
    )
    .bind(category)
    .bind(wage)
    .bind(effective_date)
    .fetch_optional(pool)
    .await?;

    if let Some((employee, employer)) = rate {
        return Ok(EpfContribution { employee, employer });
    }

    // If wage exceeds table range, use flat percentage calculation
    // Round to nearest ringgit (100 sen)
    let (emp_pct, er_pct) = match category {
        "A" => {
            if wage <= 500000 {
                // RM5,000 = 500000 sen
                (11.0_f64, 13.0_f64)
            } else {
                (11.0_f64, 12.0_f64)
            }
        }
        "B" => {
            // Elected lower rate (Form KWSP 17A)
            if wage <= 500000 {
                (9.0_f64, 13.0_f64)
            } else {
                (9.0_f64, 12.0_f64)
            }
        }
        "D" => {
            // Citizen >= 60: no mandatory employee contribution
            if wage <= 500000 {
                (0.0_f64, 4.0_f64)
            } else {
                (0.0_f64, 4.0_f64)
            }
        }
        "C" => {
            // PR >= 60
            (0.0_f64, 4.0_f64)
        }
        _ => return Err(AppError::BadRequest(format!("Invalid EPF category: {}", category))),
    };

    let employee = round_to_nearest_ringgit((wage as f64 * emp_pct / 100.0) as i64);
    let employer = round_to_nearest_ringgit((wage as f64 * er_pct / 100.0) as i64);

    Ok(EpfContribution { employee, employer })
}

/// EPF amounts are rounded to the nearest ringgit (RM1 = 100 sen)
fn round_to_nearest_ringgit(amount_sen: i64) -> i64 {
    let remainder = amount_sen % 100;
    if remainder >= 50 {
        amount_sen + (100 - remainder)
    } else {
        amount_sen - remainder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_to_nearest_ringgit() {
        assert_eq!(round_to_nearest_ringgit(1050), 1100); // RM10.50 -> RM11
        assert_eq!(round_to_nearest_ringgit(1049), 1000); // RM10.49 -> RM10
        assert_eq!(round_to_nearest_ringgit(1000), 1000); // RM10.00 -> RM10
        assert_eq!(round_to_nearest_ringgit(1099), 1100); // RM10.99 -> RM11
    }
}
