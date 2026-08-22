//! Fail-closed guard for statutory calculations.

use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::repositories::statutory_rule_sets;

pub const EPF: &str = "epf";
pub const SOCSO: &str = "socso";
pub const EIS: &str = "eis";
pub const PCB: &str = "pcb";

/// The platform setting that opens the automatic PCB gate (plan item 8).
///
/// The gate used to be a hardcoded compile-time constant with a `#[cfg(test)]`
/// bypass, which meant no test ever exercised the production path and there
/// was no defined exit criterion. It is now data: an operator flips
/// `pcb_calculator_status` to `enabled` only after the regression vectors in
/// `src/tests/fixtures/pcb_regression_vectors.json` — replaced, at conformance
/// time, by official computerised-MTD vectors — all pass against the verified
/// rule tables. Absent or any other value stays closed.
pub const PCB_CALCULATOR_STATUS_KEY: &str = "pcb_calculator_status";

async fn require_supported_calculator(pool: &PgPool) -> AppResult<()> {
    let status =
        crate::repositories::platform_settings::get_value(pool, PCB_CALCULATOR_STATUS_KEY).await?;
    if status.as_deref() == Some("enabled") {
        return Ok(());
    }
    Err(AppError::Validation(
        "Automatic PCB is disabled because the current academic calculator has not passed LHDN computerised-MTD conformance testing. Record an independently reviewed manual PCB amount outside automatic payroll. To open the gate after conformance, set the platform setting 'pcb_calculator_status' to 'enabled'."
            .into(),
    ))
}

/// Refuse automatic payroll when the applicable rules have not been verified.
/// Returning a validation error makes the configuration problem visible to the
/// operator instead of silently treating a missing lookup as a zero deduction.
pub async fn require_verified(
    pool: &PgPool,
    rule_code: &str,
    effective_date: NaiveDate,
) -> AppResult<()> {
    require_supported_calculator(pool).await?;

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
    require_supported_calculator(pool).await?;

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
