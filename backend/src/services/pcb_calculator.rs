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
/// 1. Annualise normal remuneration: Y = (monthly_normal_remuneration × remaining_months) + YTD_gross
/// 2. Compute annual reliefs (individual RM9,000, EPF up to RM4,000, SOCSO RM350, etc.)
/// 3. Chargeable income = Y - total_reliefs
/// 4. Apply tax brackets to get annual tax
/// 5. Apply rebate (RM400 if chargeable income ≤ RM35,000)
/// 6. Deduct zakat (ringgit-for-ringgit)
/// 7. Monthly PCB = (annual_tax_payable - YTD_pcb_paid) / remaining_months
/// 8. Round up to nearest RM
/// 9. Add the Schedule 2 differential for any additional remuneration
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
    // Total annual income = YTD gross + (this month's NORMAL remuneration ×
    // remaining months). Additional remuneration is excluded here by contract —
    // multiplying a one-off January bonus by the twelve remaining months
    // inflated projected annual income, and with it every month's deduction.
    // It comes back in at step 9 as the Schedule 2 differential.
    debug_assert!(
        input.bonus_amount >= 0,
        "additional remuneration is never negative"
    );
    let annual_income =
        input.ytd_gross + (input.monthly_normal_remuneration * remaining_months as i64);

    // Step 2: Calculate annual reliefs
    let reliefs = calculate_reliefs(tables, input, remaining_months, tax_year)?;

    // Steps 3-6: chargeable income, brackets, rebate, zakat.
    let total_zakat = input.ytd_zakat + (input.zakat_monthly * remaining_months as i64);
    let annual_tax = annual_tax_payable(tables, annual_income, reliefs, total_zakat)?;

    // Step 7: Monthly PCB = (annual_tax - YTD_pcb) / remaining_months
    let monthly_pcb = if remaining_months > 0 {
        (annual_tax - input.ytd_pcb) / remaining_months as i64
    } else {
        0
    };

    // Step 8: Round up to nearest RM (100 sen)
    let pcb = round_up_to_ringgit(monthly_pcb.max(0));

    // Step 9: additional remuneration, by the Schedule 2 differential.
    if input.bonus_amount > 0 {
        let bonus_pcb = calculate_bonus_pcb(
            tables,
            input.bonus_amount,
            annual_income,
            reliefs,
            total_zakat,
            annual_tax,
        )?;
        Ok(pcb + bonus_pcb)
    } else {
        Ok(pcb)
    }
}

/// Chargeable income at or below which the individual rebate applies (RM35,000).
const REBATE_CHARGEABLE_CEILING_SEN: i64 = 3_500_000;

/// The tax actually payable on an annualised income: brackets, then the
/// individual rebate, then zakat. Neither offset can drive the figure negative.
///
/// Both the normal-remuneration leg and the Schedule 2 differential resolve
/// through here, and they have to. The differential is *the increase in the
/// year's tax* caused by the additional remuneration; taking one side after the
/// rebate and the other straight off the brackets does not measure that. It went
/// wrong in both directions. An employee whose entire annual tax sits inside the
/// RM400 rebate owes nothing, yet a RM1,000 bonus still attracted a raw-bracket
/// differential — a deduction from someone with no liability at all. At the
/// other edge, where the additional remuneration is itself what pushes
/// chargeable income past the rebate ceiling, the rebate was granted to the
/// normal leg anyway and the year under-collected by RM400.
///
/// Differencing two figures computed the same way makes the year's total MTD —
/// the remaining months' deductions plus the differential — come to the year's
/// tax.
fn annual_tax_payable(
    tables: &StatutoryTables,
    annual_income: i64,
    reliefs: i64,
    total_zakat: i64,
) -> AppResult<i64> {
    let tax_year = tables.tax_year;
    let chargeable_income = (annual_income - reliefs).max(0);
    let tax = calculate_tax_from_brackets(tables, chargeable_income, tax_year)?;

    let rebate = if chargeable_income <= REBATE_CHARGEABLE_CEILING_SEN {
        get_rebate(tables, tax_year)?
    } else {
        0
    };

    // Zakat offsets tax ringgit-for-ringgit.
    Ok(((tax - rebate).max(0) - total_zakat).max(0))
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
/// Schedule 2: tax payable on (annual_income + bonus) minus tax payable on
/// (annual_income without bonus) — both *payable* figures, so the rebate and
/// zakat are applied to each side rather than to only one of them. See
/// `annual_tax_payable`.
///
/// `annual_income_without_bonus` MUST exclude the current month's additional
/// remuneration — the name was a lie while the caller annualised a gross that
/// already contained the bonus, so the differential was added on top of an
/// amount that had been counted twelve times. It still contains *prior* months'
/// bonuses through `ytd_gross`, which is correct: Schedule 2 works off YTD
/// actuals.
fn calculate_bonus_pcb(
    tables: &StatutoryTables,
    bonus_amount: i64,
    annual_income_without_bonus: i64,
    reliefs: i64,
    total_zakat: i64,
    tax_without_bonus: i64,
) -> AppResult<i64> {
    let tax_with_bonus = annual_tax_payable(
        tables,
        annual_income_without_bonus + bonus_amount,
        reliefs,
        total_zakat,
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
