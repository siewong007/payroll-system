use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserTotp {
    pub id: Uuid,
    pub user_id: Uuid,
    pub secret_encrypted: String,
    pub enabled: bool,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TotpBackupCode {
    pub id: Uuid,
    pub user_id: Uuid,
    pub code_hash: String,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Returned when starting enrollment. The raw secret and QR are only ever
/// shown at this step — nothing after confirmation can retrieve them again.
#[derive(Debug, Serialize)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub otpauth_url: String,
    pub qr_code_base64: String,
}

#[derive(Debug, Deserialize, validator::Validate)]
pub struct TotpConfirmRequest {
    #[validate(length(min = 6, max = 8, message = "code must be 6-8 characters"))]
    pub code: String,
}

/// One-time reveal of the backup codes generated at successful setup.
#[derive(Debug, Serialize)]
pub struct TotpConfirmResponse {
    pub backup_codes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TotpStatusResponse {
    pub enabled: bool,
}

#[derive(Debug, Deserialize, validator::Validate)]
pub struct TotpVerifyLoginRequest {
    #[validate(length(min = 1, message = "mfa_token is required"))]
    pub mfa_token: String,
    #[validate(length(min = 6, max = 12, message = "code must be 6-12 characters"))]
    pub code: String,
}

#[derive(Debug, Deserialize, validator::Validate)]
pub struct TotpDisableRequest {
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
}

/// Super-admin break-glass reset: the caller re-presents their own password
/// so a stolen super_admin session cannot strip 2FA off other accounts.
pub type AdminTotpResetRequest = TotpDisableRequest;

#[derive(Debug, Deserialize, validator::Validate)]
pub struct TotpRegenerateBackupCodesRequest {
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
}

/// Platform 2FA enforcement policy (plan item 36). `grace_until` is a UTC
/// calendar date through which privileged users without an enrolment may
/// still sign in; each such login writes a `two_factor_overdue` audit row
/// so the operator can see exactly who will be locked out at cutover.
#[derive(Debug, Serialize)]
pub struct TwoFactorPolicyResponse {
    pub enforce: bool,
    pub grace_until: Option<NaiveDate>,
    /// Privileged-role accounts with no enabled enrolment — the exact set a
    /// hard cut would refuse. Emails only: this endpoint is platform-admin
    /// gated and exists precisely to name them.
    pub non_compliant: Vec<String>,
}

#[derive(Debug, Deserialize, validator::Validate)]
pub struct TwoFactorPolicyRequest {
    pub enforce: bool,
    /// `Some(date)` sets/extends the grace window; `None` means a hard cut —
    /// refused while any privileged account still lacks an enrolment.
    pub grace_until: Option<NaiveDate>,
}
