use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EpfRate {
    pub id: Uuid,
    pub wage_from: i64,
    pub wage_to: i64,
    pub employee_contribution: i64,
    pub employer_contribution: i64,
    pub category: String,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SocsoRate {
    pub id: Uuid,
    pub wage_from: i64,
    pub wage_to: i64,
    pub first_cat_employee: i64,
    pub first_cat_employer: i64,
    pub second_cat_employer: i64,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EisRate {
    pub id: Uuid,
    pub wage_from: i64,
    pub wage_to: i64,
    pub employee_contribution: i64,
    pub employer_contribution: i64,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PcbBracket {
    pub id: Uuid,
    pub chargeable_income_from: i64,
    pub chargeable_income_to: i64,
    pub tax_rate_percent: rust_decimal::Decimal,
    pub cumulative_tax: i64,
    pub effective_year: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PcbRelief {
    pub id: Uuid,
    pub relief_type: String,
    pub amount: i64,
    pub effective_year: i32,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Result of statutory calculation for one employee
#[derive(Debug, Clone, Serialize, Default)]
pub struct StatutoryResult {
    pub epf_employee: i64,
    pub epf_employer: i64,
    pub socso_employee: i64,
    pub socso_employer: i64,
    pub eis_employee: i64,
    pub eis_employer: i64,
    pub pcb_amount: i64,
    pub zakat_amount: i64,
}

#[derive(Debug)]
pub struct EpfContributionRate {
    pub employee_contribution: i64,
    pub employer_contribution: i64,
}

#[derive(Debug)]
pub struct EisContributionRate {
    pub employee_contribution: i64,
    pub employer_contribution: i64,
}

#[derive(Debug)]
pub struct SocsoContributionRate {
    pub first_cat_employee: i64,
    pub first_cat_employer: i64,
    pub second_cat_employer: i64,
}

#[derive(Debug)]
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
    pub monthly_gross: i64,
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
    pub is_bonus_month: bool,
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
