use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize)]
pub struct CompanySummary {
    pub id: Uuid,
    pub name: String,
}

/// One `(user, company)` membership, as returned by the batched
/// `list_companies_for_users` read. Folded into `UserWithCompanies.companies`.
#[derive(Debug)]
pub struct UserCompanyRow {
    pub user_id: Uuid,
    pub company_id: Uuid,
    pub company_name: String,
}

#[derive(Debug, Clone, Serialize)]
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
    pub companies: Vec<CompanySummary>,
}

/// Plain row mirror of the user columns selected for `UserWithCompanies`. Needed
/// because `companies` has no corresponding column, and the compile-checked
/// `query_as!` macro requires every struct field to map to one: repos return this
/// projection and the service assembles it (filling `companies` separately).
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

/// `Debug` is hand-written rather than derived: the derived form would print the
/// plaintext password, and this struct is carried by handlers that Axum and
/// `tracing` may format on a rejection path.
#[derive(Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(
        email(message = "must be a valid email address"),
        length(max = 255, message = "email is too long")
    )]
    pub email: String,
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
    #[validate(length(min = 1, max = 255, message = "full name is required"))]
    pub full_name: String,
    pub roles: Vec<String>,
    pub company_ids: Vec<Uuid>,
}

impl std::fmt::Debug for CreateUserRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateUserRequest")
            .field("email", &crate::core::redact::email(&self.email))
            .field("password", &"***")
            .field("full_name", &self.full_name)
            .field("roles", &self.roles)
            .field("company_ids", &self.company_ids)
            .finish()
    }
}

/// `full_name` and `email` carry `min = 1` because the repository update uses
/// `COALESCE`, which only preserves the stored value for SQL NULL — an explicit
/// `""` would otherwise blank the column.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 255, message = "full name cannot be blank"))]
    pub full_name: Option<String>,
    #[validate(
        email(message = "must be a valid email address"),
        length(max = 255, message = "email is too long")
    )]
    pub email: Option<String>,
    pub roles: Option<Vec<String>>,
    pub is_active: Option<bool>,
    pub company_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct SwitchCompanyRequest {
    pub company_id: Uuid,
}
