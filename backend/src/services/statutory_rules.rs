//! Fail-closed guard for statutory calculations.

use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::repositories::statutory_rule_sets;

pub const EPF: &str = "epf";
pub const SOCSO: &str = "socso";
pub const EIS: &str = "eis";
pub const PCB: &str = "pcb";

#[cfg(not(test))]
fn require_supported_calculator(rule_code: &str) -> AppResult<()> {
    if rule_code == PCB {
        return Err(AppError::Validation(
            "Automatic PCB is disabled because the current academic calculator has not passed LHDN computerised-MTD conformance testing. Record an independently reviewed manual PCB amount outside automatic payroll until the calculator is replaced."
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
fn require_supported_calculator(_rule_code: &str) -> AppResult<()> {
    // Tests exercise the legacy academic fixture deterministically; production
    // builds never receive this bypass.
    Ok(())
}

/// Refuse automatic payroll when the applicable rules have not been verified.
/// Returning a validation error makes the configuration problem visible to the
/// operator instead of silently treating a missing lookup as a zero deduction.
pub async fn require_verified(
    pool: &PgPool,
    rule_code: &str,
    effective_date: NaiveDate,
) -> AppResult<()> {
    require_supported_calculator(rule_code)?;

    if statutory_rule_sets::is_verified_for_date(pool, rule_code, effective_date).await? {
        return Ok(());
    }

    Err(AppError::Validation(format!(
        "No verified {} statutory rule set covers {}. Automatic payroll is disabled until official source data is imported and independently verified.",
        rule_code.to_uppercase(),
        effective_date
    )))
}

/// Verify all domains once before processing a payroll run.
pub async fn require_all_verified(pool: &PgPool, effective_date: NaiveDate) -> AppResult<()> {
    require_supported_calculator(PCB)?;

    let missing = statutory_rule_sets::missing_required_for_date(pool, effective_date).await?;
    if missing.is_empty() {
        return Ok(());
    }

    Err(AppError::Validation(format!(
        "Automatic payroll is disabled for {} because these statutory rule sets are not verified: {}",
        effective_date,
        missing.join(", ").to_uppercase()
    )))
}
