use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CompanySummary {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserWithCompanies {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub roles: Vec<String>,
    pub company_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
    // populated separately
    #[sqlx(skip)]
    pub companies: Vec<CompanySummary>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub full_name: String,
    pub role: String,
    pub roles: Option<Vec<String>>,
    pub company_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub roles: Option<Vec<String>>,
    pub is_active: Option<bool>,
    pub company_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserCompaniesRequest {
    pub company_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SwitchCompanyRequest {
    pub company_id: Uuid,
}
