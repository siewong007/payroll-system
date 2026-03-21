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
