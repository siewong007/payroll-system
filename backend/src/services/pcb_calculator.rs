use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::PcbInput;
use crate::services::statutory_rules;
use crate::services::statutory_tables::StatutoryTables;

/// Calculate the repository's legacy academic PCB approximation.
///
/// This is intentionally gated by independently verified rule metadata. It is
/// not an assertion of LHDN conformance; see `docs/database.md`.
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
    statutory_rules::require_verified(pool, statutory_rules::PCB, effective_date).await?;
    let tables = StatutoryTables::load(pool, effective_date).await?;
    calculate_pcb_with(&tables, input)
}

/// Resolve PCB from an already-loaded schedule.
///
/// Pure, and the largest win of the run-scoped snapshot: a single PCB figure
/// reads six reliefs, the bracket list and a rebate, so the previous
/// database-backed form cost roughly a dozen round trips *per employee* for
/// values that are identical across the whole run.
pub(crate) fn calculate_pcb_with(tables: &StatutoryTables, input: &PcbInput) -> AppResult<i64> {
    let tax_year = tables.tax_year;
    let current_month = input.months_worked;
    let remaining_months = 12 - current_month + 1; // including current month

    // Step 1: Annualise income
    // Total annual income = YTD gross + (current month gross × remaining months)
    let annual_income = input.ytd_gross + (input.monthly_gross * remaining_months as i64);

    // Step 2: Calculate annual reliefs
    let reliefs = calculate_reliefs(tables, input, remaining_months, tax_year)?;

    // Step 3: Chargeable income
    let chargeable_income = (annual_income - reliefs).max(0);

    // Step 4: Calculate annual tax from brackets
    let annual_tax = calculate_tax_from_brackets(tables, chargeable_income, tax_year)?;

    // Step 5: Apply tax rebate
    let rebate = if chargeable_income <= 3500000 {
        // RM35,000 = 3500000 sen
        get_rebate(tables, tax_year)?
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
            tables,
            input,
            annual_income,
            reliefs,
            chargeable_income,
            tax_year,
        )?;
        Ok(pcb + bonus_pcb)
    } else {
        Ok(pcb)
    }
}

/// Calculate annual reliefs
fn calculate_reliefs(
    tables: &StatutoryTables,
    input: &PcbInput,
    remaining_months: i32,
    tax_year: i32,
) -> AppResult<i64> {
    // Individual relief
    let individual_relief = get_relief_amount(tables, "individual", tax_year)?;

    // EPF relief (capped)
    let epf_cap = get_relief_amount(tables, "life_insurance", tax_year)?; // RM3,000
    let annual_epf = input.ytd_epf + (input.epf_employee_monthly * remaining_months as i64);
    let epf_relief = annual_epf.min(epf_cap);

    // SOCSO relief
    let socso_cap = get_relief_amount(tables, "socso_relief", tax_year)?;
    let annual_socso = input.ytd_socso + (input.socso_employee_monthly * remaining_months as i64);
    let socso_relief = annual_socso.min(socso_cap);

    // EIS relief
    let eis_cap = get_relief_amount(tables, "eis_relief", tax_year)?;
    let annual_eis = input.ytd_eis + (input.eis_employee_monthly * remaining_months as i64);
    let eis_relief = annual_eis.min(eis_cap);

    // Spouse relief (non-working spouse only)
    let spouse_relief = if input.marital_status == "married" && !input.working_spouse {
        get_relief_amount(tables, "spouse", tax_year)?
    } else {
        0
    };

    // Child relief
    let child_relief = get_relief_amount(tables, "child_under_18", tax_year)?;
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
fn calculate_tax_from_brackets(
    tables: &StatutoryTables,
    chargeable_income: i64,
    tax_year: i32,
) -> AppResult<i64> {
    let brackets = tables.pcb_brackets();

    if brackets.is_empty() {
        return Err(AppError::Validation(format!(
            "Verified PCB rules contain no tax brackets for year {}",
            tax_year
        )));
    }

    let mut tax: i64 = 0;

    for b in brackets {
        if chargeable_income > b.chargeable_income_from {
            let taxable_in_bracket =
                chargeable_income.min(b.chargeable_income_to) - b.chargeable_income_from;
            let bracket_tax =
                Decimal::from(taxable_in_bracket) * b.tax_rate_percent / Decimal::from(100);
            tax = b.cumulative_tax + bracket_tax.to_i64().unwrap_or(0);

            if chargeable_income <= b.chargeable_income_to {
                break;
            }
        }
    }

    Ok(tax)
}

/// Calculate bonus PCB using Schedule 2.
///
/// Schedule 2: Tax on (annual_income + bonus) minus tax on (annual_income without bonus)
fn calculate_bonus_pcb(
    tables: &StatutoryTables,
    input: &PcbInput,
    annual_income_without_bonus: i64,
    reliefs: i64,
    _chargeable_without_bonus: i64,
    tax_year: i32,
) -> AppResult<i64> {
    let annual_income_with_bonus = annual_income_without_bonus + input.bonus_amount;
    let chargeable_with_bonus = (annual_income_with_bonus - reliefs).max(0);

    let tax_with_bonus = calculate_tax_from_brackets(tables, chargeable_with_bonus, tax_year)?;
    let tax_without_bonus = calculate_tax_from_brackets(
        tables,
        (annual_income_without_bonus - reliefs).max(0),
        tax_year,
    )?;

    let bonus_tax = (tax_with_bonus - tax_without_bonus).max(0);
    Ok(round_up_to_ringgit(bonus_tax))
}

fn get_relief_amount(tables: &StatutoryTables, relief_type: &str, tax_year: i32) -> AppResult<i64> {
    tables.pcb_relief(relief_type).ok_or_else(|| {
        AppError::Validation(format!(
            "Verified PCB rules are missing relief '{}' for year {}",
            relief_type, tax_year
        ))
    })
}

fn get_rebate(tables: &StatutoryTables, tax_year: i32) -> AppResult<i64> {
    get_relief_amount(tables, "tax_rebate_individual", tax_year)
}

/// Round up to nearest RM (100 sen)
pub(crate) fn round_up_to_ringgit(amount_sen: i64) -> i64 {
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
