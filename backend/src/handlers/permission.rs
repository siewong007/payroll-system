//! Exposes the authorization model so the frontend can consume it instead of
//! restating it.
//!
//! Before this existed, the frontend held two independent hand-maintained
//! copies of the backend's rules — the allow-lists in `lib/roles.ts` and the
//! Role Management table — and both had drifted out of agreement with the API
//! and with each other. Neither had any mechanism that would catch the drift.

use axum::{Json, extract::State};
use serde::Serialize;

use crate::core::app_state::AppState;
use crate::core::auth::AuthUser;
use crate::core::error::AppResult;
use crate::core::permission::{ALL_ROLES, Permission, role_permissions};

#[derive(Debug, Serialize)]
pub struct PermissionDescriptor {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RoleDescriptor {
    pub key: &'static str,
    pub permissions: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct PermissionMatrix {
    pub permissions: Vec<PermissionDescriptor>,
    pub roles: Vec<RoleDescriptor>,
}

/// The full role-to-permission matrix rendered by the Role Management screen.
///
/// The caller's *own* permissions are not served here — they ride along on
/// `UserResponse`, so route guards can decide synchronously instead of
/// awaiting a second request.
///
/// Gated on `ViewUserDirectory` — the same permission that reveals who holds
/// which role — since the matrix is what makes that roster meaningful.
pub async fn matrix(
    State(_state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<PermissionMatrix>> {
    auth.require_permission(Permission::ViewUserDirectory)?;

    Ok(Json(PermissionMatrix {
        permissions: Permission::ALL
            .iter()
            .map(|p| PermissionDescriptor {
                key: p.as_str(),
                label: p.label(),
                group: p.group(),
            })
            .collect(),
        roles: ALL_ROLES
            .iter()
            .map(|role| RoleDescriptor {
                key: role,
                permissions: role_permissions(role).iter().map(|p| p.as_str()).collect(),
            })
            .collect(),
    }))
}
