use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use sqlx::PgPool;

use crate::core::error::AppResult;

/// Input parameters for PCB calculation
#[derive(Debug, Clone)]
pub struct PcbInput {
    pub monthly_gross: i64,          // in sen
    pub epf_employee_monthly: i64,   // in sen (for relief)
    pub socso_employee_monthly: i64, // in sen
    pub eis_employee_monthly: i64,   // in sen
    pub zakat_monthly: i64,          // in sen (offsets PCB)
    pub marital_status: String,      // single, married
    pub working_spouse: bool,
    pub num_children: i32,
    pub months_worked: i32, // current month number in tax year (1-12)
    pub ytd_gross: i64,     // YTD gross excluding current month (sen)
    pub ytd_pcb: i64,       // YTD PCB already deducted (sen)
    pub ytd_epf: i64,       // YTD EPF employee already deducted (sen)
    pub ytd_socso: i64,     // YTD SOCSO employee (sen)
    pub ytd_eis: i64,       // YTD EIS employee (sen)
    pub ytd_zakat: i64,     // YTD zakat already deducted (sen)
    pub is_bonus_month: bool,
    pub bonus_amount: i64, // in sen (Schedule 2 for bonus)
}

/// Calculate PCB/MTD using LHDN Kaedah Pengiraan Berkomputer (Computerised Calculation Method).
///
/// This implements Schedule 1 (regular monthly remuneration) formula:
///
/// 1. Annualise monthly remuneration: Y = (monthly_gross × remaining_months) + YTD_gross
/// 2. Compute annual reliefs (individual RM9,000, EPF up to RM4,000, SOCSO RM350, etc.)
/// 3. Chargeable income = Y - total_reliefs
/// 4. Apply tax brackets to get annual tax
/// 5. Apply rebate (RM400 if chargeable income ≤ RM35,000)
/// 6. Monthly PCB = (annual_tax - YTD_pcb_paid) / remaining_months
/// 7. Deduct zakat (ringgit-for-ringgit)
/// 8. Round up to nearest RM
pub async fn calculate_pcb(
    pool: &PgPool,
    input: &PcbInput,
    effective_date: NaiveDate,
) -> AppResult<i64> {
    let tax_year = effective_date.year();
    let current_month = input.months_worked;
    let remaining_months = 12 - current_month + 1; // including current month

    // Step 1: Annualise income
    // Total annual income = YTD gross + (current month gross × remaining months)
    let annual_income = input.ytd_gross + (input.monthly_gross * remaining_months as i64);

    // Step 2: Calculate annual reliefs
    let reliefs = calculate_reliefs(pool, input, remaining_months, tax_year).await?;

    // Step 3: Chargeable income
    let chargeable_income = (annual_income - reliefs).max(0);

    // Step 4: Calculate annual tax from brackets
    let annual_tax = calculate_tax_from_brackets(pool, chargeable_income, tax_year).await?;

    // Step 5: Apply tax rebate
    let rebate = if chargeable_income <= 3500000 {
        // RM35,000 = 3500000 sen
        get_rebate(pool, tax_year).await?
    } else {
        0
    };
    let annual_tax_after_rebate = (annual_tax - rebate).max(0);

    // Step 6: Deduct zakat (ringgit-for-ringgit offset from tax)
    let total_zakat = input.ytd_zakat + (input.zakat_monthly * remaining_months as i64);
    let annual_tax_after_zakat = (annual_tax_after_rebate - total_zakat).max(0);

    // Step 7: Monthly PCB = (annual_tax - YTD_pcb) / remaining_months
    let monthly_pcb = if remaining_months > 0 {
        (annual_tax_after_zakat - input.ytd_pcb) / remaining_months as i64
    } else {
        0
    };

    // Step 8: Round up to nearest RM (100 sen)
    let pcb = round_up_to_ringgit(monthly_pcb.max(0));

    // If bonus month, add Schedule 2 computation
    if input.is_bonus_month && input.bonus_amount > 0 {
        let bonus_pcb = calculate_bonus_pcb(
            pool,
            input,
            annual_income,
            reliefs,
            chargeable_income,
            tax_year,
        )
        .await?;
        Ok(pcb + bonus_pcb)
    } else {
        Ok(pcb)
    }
}

/// Calculate annual reliefs
async fn calculate_reliefs(
    pool: &PgPool,
    input: &PcbInput,
    remaining_months: i32,
    tax_year: i32,
) -> AppResult<i64> {
    // Individual relief
    let individual_relief = get_relief_amount(pool, "individual", tax_year).await?;

    // EPF relief (capped)
    let epf_cap = get_relief_amount(pool, "life_insurance", tax_year).await?; // RM3,000
    let annual_epf = input.ytd_epf + (input.epf_employee_monthly * remaining_months as i64);
    let epf_relief = annual_epf.min(epf_cap);

    // SOCSO relief
    let socso_cap = get_relief_amount(pool, "socso_relief", tax_year).await?;
    let annual_socso = input.ytd_socso + (input.socso_employee_monthly * remaining_months as i64);
    let socso_relief = annual_socso.min(socso_cap);

    // EIS relief
    let eis_cap = get_relief_amount(pool, "eis_relief", tax_year).await?;
    let annual_eis = input.ytd_eis + (input.eis_employee_monthly * remaining_months as i64);
    let eis_relief = annual_eis.min(eis_cap);

    // Spouse relief (non-working spouse only)
    let spouse_relief = if input.marital_status == "married" && !input.working_spouse {
        get_relief_amount(pool, "spouse", tax_year).await?
    } else {
        0
    };

    // Child relief
    let child_relief = get_relief_amount(pool, "child_under_18", tax_year).await?;
    let total_child_relief = child_relief * input.num_children as i64;

    let total_reliefs = individual_relief
        + epf_relief
        + socso_relief
        + eis_relief
        + spouse_relief
        + total_child_relief;

    Ok(total_reliefs)
}

/// Look up tax from brackets
async fn calculate_tax_from_brackets(
    pool: &PgPool,
    chargeable_income: i64,
    tax_year: i32,
) -> AppResult<i64> {
    let brackets = sqlx::query_as::<_, (i64, i64, rust_decimal::Decimal, i64)>(
        r#"
        SELECT chargeable_income_from, chargeable_income_to, tax_rate_percent, cumulative_tax
        FROM pcb_brackets
        WHERE effective_year = $1
        ORDER BY chargeable_income_from ASC
        "#,
    )
    .bind(tax_year)
    .fetch_all(pool)
    .await?;

    if brackets.is_empty() {
        return Ok(0);
    }

    let mut tax: i64 = 0;

    for (from, to, rate_pct, cumulative) in &brackets {
        if chargeable_income > *from {
            let taxable_in_bracket = chargeable_income.min(*to) - from;
            let rate = *rate_pct;
            let bracket_tax = Decimal::from(taxable_in_bracket) * rate / Decimal::from(100);
            tax = cumulative + bracket_tax.to_i64().unwrap_or(0);

            if chargeable_income <= *to {
                break;
            }
        }
    }

    Ok(tax)
}

/// Calculate bonus PCB using Schedule 2.
///
/// Schedule 2: Tax on (annual_income + bonus) minus tax on (annual_income without bonus)
async fn calculate_bonus_pcb(
    pool: &PgPool,
    input: &PcbInput,
    annual_income_without_bonus: i64,
    reliefs: i64,
    _chargeable_without_bonus: i64,
    tax_year: i32,
) -> AppResult<i64> {
    let annual_income_with_bonus = annual_income_without_bonus + input.bonus_amount;
    let chargeable_with_bonus = (annual_income_with_bonus - reliefs).max(0);

    let tax_with_bonus = calculate_tax_from_brackets(pool, chargeable_with_bonus, tax_year).await?;
    let tax_without_bonus = calculate_tax_from_brackets(
        pool,
        (annual_income_without_bonus - reliefs).max(0),
        tax_year,
    )
    .await?;

    let bonus_tax = (tax_with_bonus - tax_without_bonus).max(0);
    Ok(round_up_to_ringgit(bonus_tax))
}

async fn get_relief_amount(pool: &PgPool, relief_type: &str, tax_year: i32) -> AppResult<i64> {
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT amount FROM pcb_reliefs WHERE relief_type = $1 AND effective_year = $2",
    )
    .bind(relief_type)
    .bind(tax_year)
    .fetch_optional(pool)
    .await?;

    Ok(result.unwrap_or(0))
}

async fn get_rebate(pool: &PgPool, tax_year: i32) -> AppResult<i64> {
    get_relief_amount(pool, "tax_rebate_individual", tax_year).await
}

/// Round up to nearest RM (100 sen)
fn round_up_to_ringgit(amount_sen: i64) -> i64 {
    if amount_sen <= 0 {
        return 0;
    }
    let remainder = amount_sen % 100;
    if remainder > 0 {
        amount_sen + (100 - remainder)
    } else {
        amount_sen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_up_to_ringgit() {
        assert_eq!(round_up_to_ringgit(1001), 1100); // RM10.01 -> RM11
        assert_eq!(round_up_to_ringgit(1000), 1000); // RM10.00 -> RM10
        assert_eq!(round_up_to_ringgit(1099), 1100); // RM10.99 -> RM11
        assert_eq!(round_up_to_ringgit(0), 0);
        assert_eq!(round_up_to_ringgit(-100), 0);
    }
}
