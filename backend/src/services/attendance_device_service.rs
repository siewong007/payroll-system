//! WebCrypto ECDSA P-256 device binding, verification, and security settings
//! for attendance anti-buddy-punching.

use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD},
};
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::models::attendance::{
    AttendanceSecuritySettings, DeviceChallengeResponse, DeviceProofRequest,
    EmployeeAttendanceDevice, EmployeeAttendanceDeviceWithEmployee, EnrollDeviceRequest,
    UpdateAttendanceSecuritySettingsRequest,
};
use crate::repositories::reads::attendance as attendance_reads;
use crate::repositories::{
    attendance_device_challenges, companies, employee_attendance_devices, employees,
};
use crate::services::audit_service::{self, AuditRequestMeta};

const CHALLENGE_TTL_SECONDS: i64 = 120;

#[derive(Debug, Clone, Copy)]
pub struct DeviceVerificationOutcome {
    pub verified_device_id: Option<Uuid>,
    pub is_untrusted: Option<bool>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out.iter() {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

pub(crate) fn decode_base64_flexible(raw: &str) -> Option<Vec<u8>> {
    let trimmed = raw.trim();
    URL_SAFE_NO_PAD
        .decode(trimmed)
        .or_else(|_| URL_SAFE.decode(trimmed))
        .or_else(|_| STANDARD.decode(trimmed))
        .ok()
}

pub(crate) fn parse_p256_public_key(raw: &str) -> AppResult<(Vec<u8>, String)> {
    let bytes = decode_base64_flexible(raw)
        .ok_or_else(|| AppError::BadRequest("Invalid device public key encoding".into()))?;
    if bytes.len() != 65 || bytes[0] != 0x04 {
        return Err(AppError::BadRequest(
            "Invalid device public key format".into(),
        ));
    }
    let fingerprint = sha256_hex(&bytes);
    Ok((bytes, fingerprint))
}

pub(crate) fn verify_p256_signature(
    public_key_raw: &str,
    purpose: &str,
    nonce: &str,
    signature_b64: &str,
) -> bool {
    let Ok((pub_bytes, _)) = parse_p256_public_key(public_key_raw) else {
        return false;
    };
    let Some(sig_bytes) = decode_base64_flexible(signature_b64) else {
        return false;
    };
    if sig_bytes.len() != 64 {
        return false;
    }
    let msg = format!("payroll-att-device:v1:{purpose}:{nonce}");
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, &pub_bytes)
        .verify(msg.as_bytes(), &sig_bytes)
        .is_ok()
}

pub async fn get_security_settings(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<AttendanceSecuritySettings> {
    companies::get_attendance_security_settings(pool, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Company not found".into()))
}

pub async fn update_security_settings(
    pool: &PgPool,
    company_id: Uuid,
    req: &UpdateAttendanceSecuritySettingsRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<AttendanceSecuritySettings> {
    let old = get_security_settings(pool, company_id).await?;

    let device_mode = req
        .device_mode
        .as_deref()
        .unwrap_or(&old.device_mode)
        .trim();
    if !matches!(device_mode, "none" | "warn" | "enforce") {
        return Err(AppError::BadRequest(
            "Device binding mode must be 'none', 'warn', or 'enforce'".into(),
        ));
    }

    let photo_mode = req.photo_mode.as_deref().unwrap_or(&old.photo_mode).trim();
    if !matches!(photo_mode, "none" | "optional" | "required") {
        return Err(AppError::BadRequest(
            "Photo audit mode must be 'none', 'optional', or 'required'".into(),
        ));
    }

    let auto_approve_first = req
        .device_auto_approve_first
        .unwrap_or(old.device_auto_approve_first);

    let retention_days = req.photo_retention_days.unwrap_or(old.photo_retention_days);
    if !(7..=90).contains(&retention_days) {
        return Err(AppError::BadRequest(
            "Photo retention days must be between 7 and 90".into(),
        ));
    }

    let updated = companies::update_attendance_security_settings(
        pool,
        company_id,
        device_mode,
        auto_approve_first,
        photo_mode,
        retention_days,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "attendance_security_settings",
        Some(company_id),
        Some(serde_json::to_value(&old).unwrap_or_default()),
        Some(serde_json::to_value(&updated).unwrap_or_default()),
        Some("Attendance anti-buddy-punching settings updated"),
        audit_meta,
    )
    .await;

    Ok(updated)
}

pub async fn issue_challenge(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    employee_id: Uuid,
    purpose: &str,
) -> AppResult<DeviceChallengeResponse> {
    if !matches!(purpose, "check_in" | "check_out" | "personal_qr") {
        return Err(AppError::BadRequest(
            "Challenge purpose must be 'check_in', 'check_out', or 'personal_qr'".into(),
        ));
    }

    let mut raw = [0u8; 32];
    SystemRandom::new()
        .fill(&mut raw)
        .map_err(|_| AppError::Internal("Failed to generate device challenge".into()))?;
    let mut nonce = String::with_capacity(64);
    for b in raw {
        use std::fmt::Write;
        let _ = write!(&mut nonce, "{:02x}", b);
    }

    let expires_at = Utc::now() + Duration::seconds(CHALLENGE_TTL_SECONDS);
    let challenge_id = attendance_device_challenges::insert(
        pool,
        company_id,
        user_id,
        employee_id,
        &nonce,
        purpose,
        expires_at,
    )
    .await?;

    Ok(DeviceChallengeResponse {
        challenge_id,
        nonce,
        expires_at,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn enroll_device(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    user_id: Uuid,
    req: &EnrollDeviceRequest,
    cookie_device_id: Option<Uuid>,
    user_agent: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<EmployeeAttendanceDevice> {
    let (_, fingerprint) = parse_p256_public_key(&req.public_key_raw)?;
    let normalized_key = req.public_key_raw.trim();

    let label: String = req
        .device_label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Registered device")
        .chars()
        .take(150)
        .collect();

    let mut tx = pool.begin().await?;
    employees::lock_for_update(&mut *tx, employee_id, company_id).await?;
    if !employees::exists_in_company(&mut *tx, employee_id, company_id).await? {
        return Err(AppError::NotFound("Employee profile not found".into()));
    }

    if employee_attendance_devices::exists_approved_fingerprint_for_other_employee(
        &mut *tx,
        company_id,
        employee_id,
        &fingerprint,
    )
    .await?
    {
        return Err(AppError::Conflict(
            "This device is already registered to another employee".into(),
        ));
    }

    if let Some(existing) = employee_attendance_devices::find_non_revoked_by_fingerprint(
        &mut *tx,
        company_id,
        employee_id,
        &fingerprint,
    )
    .await?
    {
        tx.commit().await?;
        return Ok(existing);
    }

    let settings = companies::get_attendance_security_settings(&mut *tx, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Company not found".into()))?;
    let has_any =
        employee_attendance_devices::has_any_for_employee(&mut *tx, company_id, employee_id)
            .await?;
    let status = if settings.device_auto_approve_first && !has_any {
        "approved"
    } else {
        "pending"
    };
    let approved_by = if status == "approved" {
        Some(user_id)
    } else {
        None
    };

    let created = employee_attendance_devices::insert(
        &mut *tx,
        company_id,
        employee_id,
        user_id,
        normalized_key,
        &fingerprint,
        &label,
        user_agent,
        cookie_device_id,
        status,
        approved_by,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        &mut *tx,
        Some(company_id),
        Some(user_id),
        "create",
        "attendance_device",
        Some(created.id),
        None,
        Some(serde_json::json!({
            "employee_id": employee_id,
            "device_label": created.device_label,
            "status": created.status,
            "fingerprint": created.public_key_fingerprint,
        })),
        Some(if created.status == "approved" {
            "First attendance device auto-approved"
        } else {
            "Attendance device enrolled (pending HR approval)"
        }),
        audit_meta,
    )
    .await;

    tx.commit().await?;
    Ok(created)
}

pub async fn list_my_devices(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
) -> AppResult<Vec<EmployeeAttendanceDevice>> {
    employee_attendance_devices::list_for_employee(pool, company_id, employee_id).await
}

pub async fn list_company_devices(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<EmployeeAttendanceDeviceWithEmployee>> {
    if let Some(st) = status
        && !matches!(st, "pending" | "approved" | "revoked")
    {
        return Err(AppError::BadRequest(
            "Device status filter must be 'pending', 'approved', or 'revoked'".into(),
        ));
    }
    attendance_reads::list_devices_with_employee(pool, company_id, status).await
}

pub async fn approve_device(
    pool: &PgPool,
    company_id: Uuid,
    device_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<EmployeeAttendanceDevice> {
    let mut tx = pool.begin().await?;
    let existing = employee_attendance_devices::get_by_id_in_company_for_update(
        &mut *tx, device_id, company_id,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Attendance device not found".into()))?;

    if existing.status == "revoked" {
        return Err(AppError::BadRequest(
            "Revoked devices cannot be re-approved; ask the employee to enroll again.".into(),
        ));
    }
    if existing.status == "approved" {
        tx.commit().await?;
        return Ok(existing);
    }

    if employee_attendance_devices::exists_approved_fingerprint_for_other_employee(
        &mut *tx,
        company_id,
        existing.employee_id,
        &existing.public_key_fingerprint,
    )
    .await?
    {
        return Err(AppError::Conflict(
            "This device is already registered to another employee".into(),
        ));
    }

    employee_attendance_devices::revoke_approved_for_employee(
        &mut *tx,
        company_id,
        existing.employee_id,
        actor_id,
        "Replaced by newly approved device",
        existing.id,
    )
    .await?;

    let approved =
        employee_attendance_devices::mark_approved(&mut *tx, existing.id, company_id, actor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Attendance device not found".into()))?;

    let _ = audit_service::log_action_with_metadata(
        &mut *tx,
        Some(company_id),
        Some(actor_id),
        "approve",
        "attendance_device",
        Some(approved.id),
        Some(serde_json::json!({ "status": existing.status })),
        Some(serde_json::json!({
            "employee_id": approved.employee_id,
            "device_label": approved.device_label,
            "status": approved.status,
        })),
        Some("Approved employee attendance device"),
        audit_meta,
    )
    .await;

    tx.commit().await?;
    Ok(approved)
}

pub async fn revoke_device(
    pool: &PgPool,
    company_id: Uuid,
    device_id: Uuid,
    actor_id: Uuid,
    reason: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<EmployeeAttendanceDevice> {
    let clean_reason: Option<String> = reason
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(400).collect());

    let revoked = employee_attendance_devices::revoke(
        pool,
        device_id,
        company_id,
        actor_id,
        clean_reason.as_deref(),
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Attendance device not found or already revoked".into()))?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "revoke",
        "attendance_device",
        Some(revoked.id),
        None,
        Some(serde_json::json!({
            "employee_id": revoked.employee_id,
            "device_label": revoked.device_label,
            "reason": clean_reason,
        })),
        Some("Revoked employee attendance device"),
        audit_meta,
    )
    .await;

    Ok(revoked)
}

#[allow(clippy::too_many_arguments)]
pub async fn verify_device_for_action(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    user_id: Uuid,
    purpose: &str,
    proof: Option<&DeviceProofRequest>,
    cookie_device_id: Option<Uuid>,
    allow_block: bool,
) -> AppResult<DeviceVerificationOutcome> {
    let settings = get_security_settings(pool, company_id).await?;
    let mut pending_presented = false;

    if let Some(p) = proof
        && let Some(device) = employee_attendance_devices::find_for_employee_by_id(
            pool,
            p.device_id,
            company_id,
            employee_id,
        )
        .await?
        && let Some(nonce) = attendance_device_challenges::consume_for_user(
            pool,
            p.challenge_id,
            company_id,
            user_id,
            purpose,
        )
        .await?
        && verify_p256_signature(&device.public_key_raw, purpose, &nonce, &p.signature)
    {
        if device.status == "approved" {
            let _ = employee_attendance_devices::touch_last_used(pool, device.id, cookie_device_id)
                .await;
            return Ok(DeviceVerificationOutcome {
                verified_device_id: Some(device.id),
                is_untrusted: if settings.device_mode == "none" {
                    None
                } else {
                    Some(false)
                },
            });
        }
        if device.status == "pending" {
            pending_presented = true;
        }
    }

    if settings.device_mode == "none" {
        return Ok(DeviceVerificationOutcome {
            verified_device_id: None,
            is_untrusted: None,
        });
    }

    if settings.device_mode == "warn" || !allow_block {
        return Ok(DeviceVerificationOutcome {
            verified_device_id: None,
            is_untrusted: Some(true),
        });
    }

    if pending_presented {
        return Err(AppError::BadRequest(
            "Your enrolled device is pending HR approval.".into(),
        ));
    }
    let devices =
        employee_attendance_devices::list_for_employee(pool, company_id, employee_id).await?;
    if devices.iter().any(|d| d.status == "pending")
        && !devices.iter().any(|d| d.status == "approved")
    {
        return Err(AppError::BadRequest(
            "Your enrolled device is pending HR approval.".into(),
        ));
    }

    Err(AppError::BadRequest(
        "An approved attendance device is required to check in. Enroll this device and wait for HR approval.".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};

    #[test]
    fn verifies_valid_p256_signature_and_rejects_tampering() {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .expect("generate test keypair");
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref())
            .expect("load test keypair");

        let pub_b64 = URL_SAFE_NO_PAD.encode(pair.public_key().as_ref());
        let nonce = "abc123def456";
        let payload = format!("payroll-att-device:v1:check_in:{nonce}");
        let sig = pair.sign(&rng, payload.as_bytes()).expect("sign challenge");
        let sig_b64 = URL_SAFE_NO_PAD.encode(sig.as_ref());

        assert!(verify_p256_signature(&pub_b64, "check_in", nonce, &sig_b64));
        assert!(!verify_p256_signature(
            &pub_b64,
            "check_out",
            nonce,
            &sig_b64
        ));
        assert!(!verify_p256_signature(
            &pub_b64, "check_in", "wrong", &sig_b64
        ));
        assert!(parse_p256_public_key(&URL_SAFE_NO_PAD.encode([1u8; 32])).is_err());
    }
}
