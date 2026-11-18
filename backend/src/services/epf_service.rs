use chrono::NaiveDate;
use sqlx::PgPool;

use crate::core::error::{AppError, AppResult};
use crate::models::statutory::EpfContribution;
use crate::services::statutory_rules;
use crate::services::statutory_tables::StatutoryTables;

/// Effective date of mandatory EPF Third Schedule Part F (2% employee + 2%
/// employer) for non-Malaysian citizen employees under age 75 holding valid
/// work passes (*Employees Provident Fund (Amendment) Act 2025*).
pub const PART_F_EFFECTIVE_FROM: NaiveDate = match NaiveDate::from_ymd_opt(2025, 10, 1) {
    Some(d) => d,
    None => unreachable!(),
};

/// Upper statutory age limit under *EPF Act 1991 (Act 452) s.43(1)*: once an
/// employee attains 75 years of age, mandatory statutory EPF contributions
/// cease (`0` employee, `0` employer) unless an explicit voluntary category is
/// set on the employee record.
pub const STATUTORY_MAX_AGE: i32 = 75;

/// Derive the applicable KWSP Third Schedule Part (`A`–`F`) from age,
/// residency status, and payroll effective date, or `None` when statutory EPF
/// ceases at age 75 under *EPF Act 1991 s.43(1)*.
///
/// Official KWSP Third Schedule mapping:
/// - **Part A**: Malaysian citizens & Permanent Residents (and pre-1 Aug 1998
///   non-citizen members) below age 60.
/// - **Part B**: Non-Malaysian citizens (registered on/after 1 Aug 1998) below
///   age 60 prior to 1 October 2025 (or under legacy election).
/// - **Part C**: Permanent Residents (and pre-1 Aug 1998 non-citizen members)
///   aged 60 to 74.
/// - **Part D**: Non-Malaysian citizens (registered on/after 1 Aug 1998) aged
///   60 to 74 prior to 1 October 2025 (or under legacy election).
/// - **Part E**: Malaysian citizens aged 60 to 74 (0% employee, 4% employer).
/// - **Part F**: Non-Malaysian citizen employees below age 75 holding valid
///   work passes from **1 October 2025** onward (2% employee + 2% employer).
pub(crate) fn derive_category_for(
    age: i32,
    residency_status: &str,
    effective_date: NaiveDate,
) -> Option<&'static str> {
    if age >= STATUTORY_MAX_AGE {
        return None;
    }

    match residency_status.trim().to_ascii_lowercase().as_str() {
        "foreigner" => {
            if effective_date >= PART_F_EFFECTIVE_FROM {
                Some("F")
            } else if age >= 60 {
                Some("D")
            } else {
                Some("B")
            }
        }
        "permanent_resident" | "pr" => {
            if age >= 60 {
                Some("C")
            } else {
                Some("A")
            }
        }
        _ => {
            // Malaysian citizen (default)
            if age >= 60 { Some("E") } else { Some("A") }
        }
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
    residency_status: &str,
    override_category: Option<&str>, // employees.epf_category, when HR has set one
    effective_date: NaiveDate,
) -> AppResult<EpfContribution> {
    statutory_rules::require_verified(pool, statutory_rules::EPF, effective_date).await?;
    let tables = StatutoryTables::load(pool, effective_date).await?;
    calculate_epf_with(
        &tables,
        contributable_wage,
        age,
        residency_status,
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
/// pre-August-1998 elections, domestic-worker exemptions, and voluntary-rate
/// cases no derivation can see. The resolved part comes back on the
/// contribution so the caller can notice the disagreement — `preview_payroll`
/// raises it as a warning — and so the run's provenance records what was
/// actually applied.
pub(crate) fn calculate_epf_with(
    tables: &StatutoryTables,
    contributable_wage: i64,
    age: i32,
    residency_status: &str,
    override_category: Option<&str>,
) -> AppResult<EpfContribution> {
    let derived = derive_category_for(age, residency_status, tables.effective_date);
    let category = match override_category.map(str::trim).filter(|c| !c.is_empty()) {
        Some(explicit) => {
            if !matches!(explicit, "A" | "B" | "C" | "D" | "E" | "F") {
                return Err(AppError::BadRequest(format!(
                    "Invalid EPF category: {}",
                    explicit
                )));
            }
            explicit
        }
        None => match derived {
            Some(part) => part,
            None => {
                // EPF Act 1991 s.43(1): mandatory contribution ceases at age 75.
                return Ok(EpfContribution {
                    employee: 0,
                    employer: 0,
                    category: "EXEMPT".to_string(),
                });
            }
        },
    };

    let normalized_residency = residency_status.trim().to_ascii_lowercase();
    match tables.epf_band(category, contributable_wage) {
        Some(band) => Ok(EpfContribution {
            employee: band.employee_contribution,
            employer: band.employer_contribution,
            category: category.to_string(),
        }),
        // Names the part that was actually applied and where it came from. With
        // an age/residency-derived part this is often C, E, or F, and a verified
        // schedule that carries only Part A is the likely cause rather than the wage.
        None => {
            let source = if override_category
                .map(str::trim)
                .is_some_and(|c| !c.is_empty())
            {
                "set explicitly on the employee record".to_string()
            } else {
                let residency_note = match normalized_residency.as_str() {
                    "foreigner" => ", non-citizen",
                    "permanent_resident" | "pr" => ", permanent resident",
                    _ => ", citizen",
                };
                format!("derived from age {age}{residency_note}")
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
