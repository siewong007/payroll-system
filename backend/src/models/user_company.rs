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
    pub roles: Vec<String>,
    pub company_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
    // populated separately
    #[sqlx(skip)]
    pub companies: Vec<CompanySummary>,
}

/// Plain row mirror of the user columns selected for `UserWithCompanies`. Needed
/// because `UserWithCompanies` has a `#[sqlx(skip)]` `companies` field that the
/// compile-checked `query_as!` macro cannot populate: repos return this projection
/// and the service assembles it (filling `companies` separately).
#[derive(Debug)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub roles: Vec<String>,
    pub company_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub is_active: Option<bool>,
    pub created_at: DateTime<Utc>,
}

impl UserRow {
    pub fn into_user(self) -> UserWithCompanies {
        UserWithCompanies {
            id: self.id,
            email: self.email,
            full_name: self.full_name,
            roles: self.roles,
            company_id: self.company_id,
            employee_id: self.employee_id,
            is_active: self.is_active,
            created_at: self.created_at,
            companies: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub full_name: String,
    pub roles: Vec<String>,
    pub company_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub full_name: Option<String>,
    pub email: Option<String>,
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
