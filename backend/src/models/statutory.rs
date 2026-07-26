use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One row of the effective-dated EPF Third Schedule.
///
/// The band rows below back `services::statutory_tables`, which loads every
/// applicable band once per payroll run instead of issuing one wage lookup per
/// employee. `effective_from` is carried because band selection is
/// newest-schedule-wins, exactly as the per-wage SQL lookups resolve it.
#[derive(Debug, Clone)]
pub struct EpfBand {
    pub category: String,
    pub wage_from: i64,
    pub wage_to: i64,
    pub employee_contribution: i64,
    pub employer_contribution: i64,
    pub effective_from: NaiveDate,
}

#[derive(Debug, Clone)]
pub struct SocsoBand {
    pub wage_from: i64,
    pub wage_to: i64,
    pub first_cat_employee: i64,
    pub first_cat_employer: i64,
    pub second_cat_employer: i64,
    pub effective_from: NaiveDate,
}

#[derive(Debug, Clone)]
pub struct EisBand {
    pub wage_from: i64,
    pub wage_to: i64,
    pub employee_contribution: i64,
    pub employer_contribution: i64,
    pub effective_from: NaiveDate,
}

/// A verified rule set covering the run's effective date, recorded on the run so
/// the figures stay explainable after the rule tables change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedRuleSet {
    pub rule_code: String,
    pub rule_set_id: Uuid,
    pub dataset_key: String,
    pub source_version: Option<String>,
    pub source_sha256: Option<String>,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
}

#[derive(Debug, Clone)]
pub struct PcbBracketLookup {
    pub chargeable_income_from: i64,
    pub chargeable_income_to: i64,
    pub tax_rate_percent: rust_decimal::Decimal,
    pub cumulative_tax: i64,
}

#[derive(Debug, Clone)]
pub struct EpfContribution {
    pub employee: i64,
    pub employer: i64,
}

#[derive(Debug, Clone)]
pub struct EisContribution {
    pub employee: i64,
    pub employer: i64,
}

#[derive(Debug, Clone)]
pub struct SocsoContribution {
    pub employee: i64,
    pub employer: i64,
    pub category: SocsoCategory,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SocsoCategory {
    FirstCategory,
    SecondCategory,
    Exempt,
}

#[derive(Debug, Clone)]
pub struct PcbInput {
    /// This month's *normal* remuneration — the part that recurs and is
    /// therefore meaningful to annualise. Deliberately not named `monthly_gross`
    /// any more: the field's contract changed, and a silent semantic change on
    /// an unchanged name is how the annualised-bonus defect would come back.
    pub monthly_normal_remuneration: i64,
    pub epf_employee_monthly: i64,
    pub socso_employee_monthly: i64,
    pub eis_employee_monthly: i64,
    pub zakat_monthly: i64,
    pub marital_status: String,
    pub working_spouse: bool,
    pub num_children: i32,
    pub months_worked: i32,
    pub ytd_gross: i64,
    pub ytd_pcb: i64,
    pub ytd_epf: i64,
    pub ytd_socso: i64,
    pub ytd_eis: i64,
    pub ytd_zakat: i64,
    /// This month's *additional* remuneration (bonus, commission), taxed by the
    /// Schedule 2 differential rather than annualised.
    ///
    /// There is no companion `is_bonus_month` flag: it was a boolean whose only
    /// two writers both said `false`, so the Schedule 2 path was unreachable and
    /// the guard read as a feature rather than as the trap it was. `> 0` is the
    /// same question with nothing to get out of step with.
    pub bonus_amount: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct StatutoryRow {
    pub employee_name: String,
    pub ic_number: Option<String>,
    pub tax_identification_number: Option<String>,
    pub epf_number: Option<String>,
    pub socso_number: Option<String>,
    pub eis_number: Option<String>,
    pub gross_salary: i64,
    pub epf_employee: i64,
    pub epf_employer: i64,
    pub socso_employee: i64,
    pub socso_employer: i64,
    pub eis_employee: i64,
    pub eis_employer: i64,
    pub pcb_amount: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CompanyStatutoryInfo {
    pub name: String,
    pub epf_number: Option<String>,
    pub socso_code: Option<String>,
    pub eis_code: Option<String>,
    pub tax_number: Option<String>,
}
