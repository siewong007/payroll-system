use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::EpfContribution;
use crate::services::statutory_rules;
use crate::services::statutory_tables::StatutoryTables;

/// The EPF Third-Schedule part an employee falls in, from the two facts the
/// record already carries.
///
/// Part A is citizens and permanent residents below 60; Part C is the same
/// people at 60 and over, when the employee rate drops away. Parts B and D are
/// their non-citizen counterparts. `is_foreigner` is `residency_status ==
/// "foreigner"`, so a permanent resident is correctly grouped with a citizen.
///
/// Derived rather than read from `employees.epf_category`, because that column is
/// free text with a default of 'A' and nothing ever changed it: a citizen who
/// turned 60 mid-employment kept a full Part A employee deduction indefinitely,
/// unless somebody happened to notice a dropdown. SOCSO and EIS have taken `age`
/// and `is_foreigner` all along; this gives EPF the same inputs.
pub(crate) fn derive_category(age: i32, is_foreigner: bool) -> &'static str {
    match (is_foreigner, age >= 60) {
        (false, false) => "A",
        (false, true) => "C",
        (true, false) => "B",
        (true, true) => "D",
    }
}

/// Look up EPF contribution from the Third Schedule table.
///
/// The verified rule set must contain an exact matching band. Percentage
/// fallbacks are intentionally rejected because official EPF parts have
/// different eligibility and rounding rules.
///
/// `contributable_wage` is EPF-contributable wages in sen as defined by EPF Act
/// 1991 s.2 — overtime, gratuity, travelling allowance, service charge and
/// payment in lieu of notice are NOT part of it. Callers must not pass the
/// payslip's gross, which includes overtime: SOCSO and EIS rate on that figure,
/// EPF does not.
pub async fn calculate_epf(
    pool: &PgPool,
    contributable_wage: i64, // monthly EPF-contributable wage in sen
    age: i32,
    is_foreigner: bool,
    override_category: Option<&str>, // employees.epf_category, when HR has set one
    effective_date: NaiveDate,
) -> AppResult<EpfContribution> {
    statutory_rules::require_verified(pool, statutory_rules::EPF, effective_date).await?;
    let tables = StatutoryTables::load(pool, effective_date).await?;
    calculate_epf_with(
        &tables,
        contributable_wage,
        age,
        is_foreigner,
        override_category,
    )
}

/// Resolve EPF from an already-loaded schedule.
///
/// Pure, so a payroll run calls it once per employee without touching the
/// database; `calculate_epf` is the one-off path that loads a snapshot first.
///
/// `contributable_wage` carries the same EPF Act 1991 s.2 meaning as on
/// `calculate_epf` — not the payslip gross.
///
/// `override_category` is honoured when set, because HR legitimately knows about
/// pre-August-1998 elections and voluntary-rate cases no derivation can see. The
/// resolved part comes back on the contribution so the caller can notice the
/// disagreement — `preview_payroll` raises it as a warning — and so the run's
/// provenance records what was actually applied.
pub(crate) fn calculate_epf_with(
    tables: &StatutoryTables,
    contributable_wage: i64,
    age: i32,
    is_foreigner: bool,
    override_category: Option<&str>,
) -> AppResult<EpfContribution> {
    let derived = derive_category(age, is_foreigner);
    let category = match override_category.map(str::trim).filter(|c| !c.is_empty()) {
        Some(explicit) => {
            if !matches!(explicit, "A" | "B" | "C" | "D" | "E") {
                return Err(AppError::BadRequest(format!(
                    "Invalid EPF category: {}",
                    explicit
                )));
            }
            explicit
        }
        None => derived,
    };

    match tables.epf_band(category, contributable_wage) {
        Some(band) => Ok(EpfContribution {
            employee: band.employee_contribution,
            employer: band.employer_contribution,
            category: category.to_string(),
        }),
        // Names the part that was actually applied and where it came from. With
        // an age-derived part this is very often C or D, and a verified schedule
        // that carries only Part A is the likely cause rather than the wage.
        None => {
            let source = if override_category.is_some() {
                "set explicitly on the employee record".to_string()
            } else {
                let residency = if is_foreigner { ", non-citizen" } else { "" };
                format!("derived from age {age}{residency}")
            };
            Err(AppError::Validation(format!(
                "The verified EPF schedule for {} contains no Part {} band covering a wage of {} \
                 sen. Part {} is what applies here ({}); load an EPF rule set that carries that \
                 part, or set an explicit EPF category on the employee record.",
                tables.effective_date, category, contributable_wage, category, source,
            )))
        }
    }
}
