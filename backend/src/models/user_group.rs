use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserGroup {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// List projection: the group plus what it grants and how many hold it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserGroupWithDetail {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub permissions: Vec<String>,
    pub member_count: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserGroupMember {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub added_at: DateTime<Utc>,
    pub full_name: String,
    pub email: String,
}

#[derive(Deserialize, Validate)]
pub struct CreateUserGroupRequest {
    #[validate(length(min = 1, max = 100, message = "must be 1-100 characters"))]
    pub name: String,
    pub description: Option<String>,
    /// Permission keys, validated against `Permission::as_str()` by the service.
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub actor_password: Option<String>,
    #[serde(default)]
    pub actor_totp_code: Option<String>,
}

impl std::fmt::Debug for CreateUserGroupRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateUserGroupRequest")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("permissions", &self.permissions)
            .field(
                "actor_password",
                &self.actor_password.as_ref().map(|_| "***"),
            )
            .field(
                "actor_totp_code",
                &self.actor_totp_code.as_ref().map(|_| "***"),
            )
            .finish()
    }
}

#[derive(Deserialize, Validate)]
pub struct UpdateUserGroupRequest {
    #[validate(length(min = 1, max = 100, message = "must be 1-100 characters"))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    /// Absent leaves the permission set untouched; present replaces it
    /// wholesale, including with an empty list.
    pub permissions: Option<Vec<String>>,
    #[serde(default)]
    pub actor_password: Option<String>,
    #[serde(default)]
    pub actor_totp_code: Option<String>,
}

impl std::fmt::Debug for UpdateUserGroupRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateUserGroupRequest")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("is_active", &self.is_active)
            .field("permissions", &self.permissions)
            .field(
                "actor_password",
                &self.actor_password.as_ref().map(|_| "***"),
            )
            .field(
                "actor_totp_code",
                &self.actor_totp_code.as_ref().map(|_| "***"),
            )
            .finish()
    }
}

#[derive(Deserialize)]
pub struct AddUserGroupMemberRequest {
    pub user_id: Uuid,
    #[serde(default)]
    pub actor_password: Option<String>,
    #[serde(default)]
    pub actor_totp_code: Option<String>,
}

impl std::fmt::Debug for AddUserGroupMemberRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AddUserGroupMemberRequest")
            .field("user_id", &self.user_id)
            .field(
                "actor_password",
                &self.actor_password.as_ref().map(|_| "***"),
            )
            .field(
                "actor_totp_code",
                &self.actor_totp_code.as_ref().map(|_| "***"),
            )
            .finish()
    }
}
