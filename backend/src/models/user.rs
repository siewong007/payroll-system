use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::core::permission::{Permission, role_permissions};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub full_name: String,
    pub roles: Vec<String>,
    pub company_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub is_active: Option<bool>,
    pub must_change_password: bool,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "must be a valid email address"))]
    pub email: String,
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub user: UserResponse,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub roles: Vec<String>,
    pub company_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub must_change_password: bool,
    /// Effective permissions, derived from `roles`.
    ///
    /// Shipped with the session rather than fetched separately so the frontend
    /// can decide what to render synchronously — a route guard that had to await
    /// a second request would either flash a 403 or block every navigation on a
    /// spinner. It is a *rendering* input only; every permission is re-checked
    /// server-side on the request that acts on it.
    pub permissions: Vec<&'static str>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ExistingUser {
    pub id: Uuid,
    pub roles: Vec<String>,
    pub company_id: Option<Uuid>,
    pub is_deleted: bool,
}

#[derive(Debug)]
pub struct UserContact {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        let permissions = effective_permissions(&u.roles);
        Self {
            id: u.id,
            email: u.email,
            full_name: u.full_name,
            roles: u.roles,
            company_id: u.company_id,
            employee_id: u.employee_id,
            must_change_password: u.must_change_password,
            permissions,
        }
    }
}

/// The union of every role's grants, de-duplicated, in `Permission::ALL` order
/// so the response is stable regardless of how the roles were ordered.
fn effective_permissions(roles: &[String]) -> Vec<&'static str> {
    Permission::ALL
        .iter()
        .filter(|permission| {
            roles
                .iter()
                .any(|role| role_permissions(role).contains(permission))
        })
        .map(|permission| permission.as_str())
        .collect()
}
