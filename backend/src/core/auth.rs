use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    app_state::AppState,
    error::{AppError, AppResult},
    permission,
};
use crate::repositories::user_sessions;
use crate::repositories::users;

/// Registered `iss`/`aud` claim values. Validating these on decode rejects
/// tokens minted for a different service or audience even if they were signed
/// with the same secret.
pub const JWT_ISSUER: &str = "payroll-system";
pub const JWT_AUDIENCE: &str = "payroll-system-api";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid, // user ID
    pub email: String,
    pub roles: Vec<String>,
    pub company_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub sid: Uuid,
    pub exp: i64,
    pub iat: i64,
    #[serde(default)]
    pub iss: String,
    #[serde(default)]
    pub aud: String,
}

#[allow(clippy::too_many_arguments)]
pub fn create_token(
    user_id: Uuid,
    email: &str,
    roles: &[String],
    company_id: Option<Uuid>,
    employee_id: Option<Uuid>,
    session_id: Uuid,
    secret: &str,
    expiry_hours: i64,
) -> AppResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        roles: normalized_roles(roles),
        company_id,
        employee_id,
        sid: session_id,
        exp: (now + Duration::hours(expiry_hours)).timestamp(),
        iat: now.timestamp(),
        iss: JWT_ISSUER.to_string(),
        aud: JWT_AUDIENCE.to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to create token: {}", e)))
}

fn normalized_roles(roles: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for role in roles {
        if !normalized.iter().any(|existing| existing == role) {
            normalized.push(role.clone());
        }
    }
    normalized
}

pub fn verify_token(token: &str, secret: &str) -> AppResult<Claims> {
    let mut validation = Validation::default(); // HS256 + exp validation
    validation.set_issuer(&[JWT_ISSUER]);
    validation.set_audience(&[JWT_AUDIENCE]);

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(format!("Invalid token: {}", e)))
}

/// Registered `iss`/`aud` for the short-lived token issued between primary
/// auth (password/passkey/Google) and completing a TOTP 2FA challenge. The
/// distinct issuer/audience — checked by `verify_mfa_pending_token`, never by
/// `verify_token`/`AuthUser` — means this token structurally cannot be used
/// as a bearer token anywhere else in the API.
pub const MFA_PENDING_ISSUER: &str = "payroll-system-mfa-pending";
pub const MFA_PENDING_AUDIENCE: &str = "payroll-system-mfa";
const MFA_PENDING_EXPIRY_MINUTES: i64 = 5;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MfaPendingClaims {
    sub: Uuid,
    exp: i64,
    iat: i64,
    iss: String,
    aud: String,
}

pub fn create_mfa_pending_token(user_id: Uuid, secret: &str) -> AppResult<String> {
    let now = Utc::now();
    let claims = MfaPendingClaims {
        sub: user_id,
        exp: (now + Duration::minutes(MFA_PENDING_EXPIRY_MINUTES)).timestamp(),
        iat: now.timestamp(),
        iss: MFA_PENDING_ISSUER.to_string(),
        aud: MFA_PENDING_AUDIENCE.to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to create MFA token: {}", e)))
}

/// Verifies an MFA-pending token and returns the user id it was issued for.
pub fn verify_mfa_pending_token(token: &str, secret: &str) -> AppResult<Uuid> {
    let mut validation = Validation::default();
    validation.set_issuer(&[MFA_PENDING_ISSUER]);
    validation.set_audience(&[MFA_PENDING_AUDIENCE]);

    decode::<MfaPendingClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims.sub)
    .map_err(|e| AppError::Unauthorized(format!("Invalid or expired MFA token: {}", e)))
}

/// Extractor for authenticated user claims from JWT
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

pub use super::permission::Permission;

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Invalid authorization format".to_string()))?;

        let secret = parts
            .extensions
            .get::<JwtSecret>()
            .ok_or_else(|| AppError::Internal("JWT secret not configured".to_string()))?;

        let claims = verify_token(token, &secret.0)?;
        if users::get_active_by_id(&state.pool, claims.sub)
            .await?
            .is_none()
        {
            return Err(AppError::Unauthorized(
                "User account is no longer active".into(),
            ));
        }
        if !user_sessions::is_active(&state.pool, claims.sub, claims.sid).await? {
            return Err(AppError::Unauthorized(
                "Session has expired or was revoked".into(),
            ));
        }
        Ok(AuthUser(claims))
    }
}

impl AuthUser {
    pub fn roles(&self) -> Vec<&str> {
        self.0.roles.iter().map(String::as_str).collect()
    }

    pub fn has_any_role(&self, roles: &[&str]) -> bool {
        self.roles().iter().any(|role| roles.contains(role))
    }

    /// Returns the active company ID or rejects users without company context.
    pub fn company_id(&self) -> AppResult<Uuid> {
        self.0
            .company_id
            .ok_or_else(|| AppError::Forbidden("No company assigned".into()))
    }

    /// Returns the linked employee ID or rejects users without an employee profile.
    pub fn employee_id(&self) -> AppResult<Uuid> {
        self.0
            .employee_id
            .ok_or_else(|| AppError::Forbidden("No employee profile linked".into()))
    }

    /// Returns true if the user holds the 'exec' role.
    ///
    /// Prefer a [`Permission`] check. This remains only for presentation code
    /// that varies by role rather than by capability.
    pub fn is_exec(&self) -> bool {
        self.has_any_role(&["exec"])
    }

    /// Returns true if the role can access payroll and statutory data. Used by
    /// read paths that redact figures rather than refusing the request
    /// outright (the dashboard summary, report projections).
    pub fn is_payroll_privileged(&self) -> bool {
        self.can(Permission::ViewPayroll)
    }

    /// The caller's effective permissions — the union across every role held.
    pub fn permissions(&self) -> Vec<Permission> {
        let mut effective: Vec<Permission> = Vec::new();
        for role in &self.0.roles {
            for permission in permission::role_permissions(role) {
                if !effective.contains(permission) {
                    effective.push(*permission);
                }
            }
        }
        effective
    }

    /// Whether the caller may perform `permission`.
    ///
    /// This is the only authorization predicate in the codebase. It replaced
    /// sixteen role allow-lists that had drifted apart; see
    /// [`crate::core::permission`] for the table it consults.
    pub fn can(&self, permission: Permission) -> bool {
        permission::roles_grant(&self.0.roles, permission)
    }

    /// Rejects the request unless the caller holds `permission`.
    pub fn require_permission(&self, permission: Permission) -> AppResult<()> {
        if !self.can(permission) {
            // Naming the capability makes a 403 diagnosable without reading
            // the handler; it reveals nothing the matrix endpoint does not.
            return Err(AppError::Forbidden(format!(
                "Not authorized: {} required",
                permission.label().to_lowercase()
            )));
        }
        Ok(())
    }

    /// Rejects the request unless the caller holds `permission`, returning the
    /// active company on success. Nearly every handler needs both, and pairing
    /// them means a permission check cannot be written without also scoping the
    /// query to a tenant.
    pub fn authorize(&self, permission: Permission) -> AppResult<Uuid> {
        self.require_permission(permission)?;
        self.company_id()
    }

    /// As [`Self::authorize`], additionally returning the acting user id for
    /// handlers that record `created_by` / `updated_by` or write an audit row.
    pub fn authorize_actor(&self, permission: Permission) -> AppResult<(Uuid, Uuid)> {
        let company_id = self.authorize(permission)?;
        Ok((self.0.sub, company_id))
    }
}

#[derive(Clone)]
pub struct JwtSecret(pub String);
