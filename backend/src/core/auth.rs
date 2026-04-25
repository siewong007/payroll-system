use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::{AppError, AppResult};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid, // user ID
    pub email: String,
    pub role: String,
    pub company_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub exp: i64,
    pub iat: i64,
}

pub fn create_token(
    user_id: Uuid,
    email: &str,
    role: &str,
    company_id: Option<Uuid>,
    employee_id: Option<Uuid>,
    secret: &str,
    expiry_hours: i64,
) -> AppResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        role: role.to_string(),
        company_id,
        employee_id,
        exp: (now + Duration::hours(expiry_hours)).timestamp(),
        iat: now.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to create token: {}", e)))
}

pub fn verify_token(token: &str, secret: &str) -> AppResult<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(format!("Invalid token: {}", e)))
}

/// Extractor for authenticated user claims from JWT
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    ViewPayroll,
    ManagePayrollDraft,
    SubmitPayroll,
    ApprovePayroll,
    MarkPayrollPaid,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
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
        Ok(AuthUser(claims))
    }
}

impl AuthUser {
    fn has_any_role(&self, roles: &[&str]) -> bool {
        roles.contains(&self.0.role.as_str())
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

    /// Returns true if the user's role is 'exec'.
    pub fn is_exec(&self) -> bool {
        self.0.role == "exec"
    }

    /// Returns true for company-scoped admin roles that can manage HR setup.
    pub fn is_hr_admin(&self) -> bool {
        self.has_any_role(&["super_admin", "admin", "hr_manager"])
    }

    /// Returns true for company admins that can change company-level settings.
    pub fn is_company_admin(&self) -> bool {
        self.has_any_role(&["super_admin", "admin"])
    }

    /// Returns true if the role can access payroll and statutory data.
    pub fn is_payroll_privileged(&self) -> bool {
        self.can(Permission::ViewPayroll)
    }

    pub fn can(&self, permission: Permission) -> bool {
        match self.0.role.as_str() {
            "super_admin" => true,
            "payroll_admin" => matches!(
                permission,
                Permission::ViewPayroll
                    | Permission::ManagePayrollDraft
                    | Permission::SubmitPayroll
            ),
            "finance" => matches!(
                permission,
                Permission::ViewPayroll | Permission::ApprovePayroll | Permission::MarkPayrollPaid
            ),
            _ => false,
        }
    }

    pub fn require_permission(&self, permission: Permission) -> AppResult<()> {
        if !self.can(permission) {
            return Err(AppError::Forbidden(
                "Not authorized for this payroll action".into(),
            ));
        }
        Ok(())
    }

    /// Rejects the request if role is 'exec'. Use to guard payroll endpoints.
    pub fn deny_exec(&self) -> AppResult<()> {
        if self.is_exec() {
            return Err(AppError::Forbidden(
                "Payroll access not available for this role".into(),
            ));
        }
        Ok(())
    }

    /// Rejects the request unless the role is super_admin.
    pub fn require_super_admin(&self) -> AppResult<()> {
        if self.0.role != "super_admin" {
            return Err(AppError::Forbidden("Super admin only".into()));
        }
        Ok(())
    }

    /// Rejects the request unless the role can manage company-level settings.
    pub fn require_company_admin(&self) -> AppResult<()> {
        if !self.is_company_admin() {
            return Err(AppError::Forbidden("Admin role required".into()));
        }
        Ok(())
    }

    /// Rejects the request unless the role can manage HR operations.
    pub fn require_hr_admin(&self) -> AppResult<()> {
        if !self.is_hr_admin() {
            return Err(AppError::Forbidden("Admin role required".into()));
        }
        Ok(())
    }

    /// Rejects the request unless the role is allowed to access payroll data.
    pub fn require_payroll_privileged(&self) -> AppResult<()> {
        self.require_permission(Permission::ViewPayroll)
    }

    /// Rejects employee self-service users from admin attendance views.
    pub fn require_non_employee(&self) -> AppResult<()> {
        if self.0.role == "employee" {
            return Err(AppError::Forbidden("Not authorized".into()));
        }
        Ok(())
    }

    /// Rejects users that cannot generate attendance QR codes.
    pub fn require_attendance_qr_generator(&self) -> AppResult<()> {
        if !self.has_any_role(&[
            "admin",
            "super_admin",
            "hr_manager",
            "payroll_admin",
            "exec",
        ]) {
            return Err(AppError::Forbidden(
                "Authorized role required to generate QR code".into(),
            ));
        }
        Ok(())
    }

    /// Rejects users that cannot manage kiosk credentials.
    pub fn require_kiosk_admin(&self) -> AppResult<()> {
        if !self.has_any_role(&["admin", "super_admin", "hr_manager", "payroll_admin"]) {
            return Err(AppError::Forbidden(
                "Authorized role required to manage kiosk credentials".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct JwtSecret(pub String);
