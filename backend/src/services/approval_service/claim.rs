//! Claim admin CRUD + approval/reject workflow.

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::upload_path::validate_optional_file_url;
use crate::models::portal::{Claim, CreateClaimRequest, UpdateClaimRequest};
use crate::repositories::reads::approvals as approval_reads;
use crate::repositories::{claims, users as user_repo};
use crate::services::audit_service::AuditRequestMeta;
use crate::services::email_service;
use crate::services::notification_service;

use super::common::{
    Reviewer, audit_self_approval_override, ensure_employee_in_company, ensure_not_self_approval,
    ensure_positive_amount, reviewer_employee_id,
};

pub use crate::models::approval::ClaimWithEmployee;

pub async fn create_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateClaimRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Claim> {
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    ensure_positive_amount(req.amount)?;
    // `receipt_url` is joined onto a filesystem path by the backup export and
    // restore paths, so it is settled at write time rather than at every sink.
    validate_optional_file_url(req.receipt_url.as_deref())?;

    let claim = claims::insert(
        pool,
        employee_id,
        company_id,
        req.title,
        req.description,
        req.amount,
        req.category,
        req.receipt_url,
        req.receipt_file_name,
        req.expense_date,
    )
    .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "create_claim_admin",
        "claim",
        Some(claim.id),
        None,
        Some(serde_json::to_value(&claim).unwrap_or_default()),
        Some(&format!("Created claim for employee {}", claim.employee_id)),
        audit_meta,
    )
    .await;

    Ok(claim)
}

pub async fn update_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    req: UpdateClaimRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Claim> {
    let current = claims::get_editable(pool, claim_id, company_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be edited".into()))?;

    if let Some(employee_id) = req.employee_id {
        ensure_employee_in_company(pool, company_id, employee_id).await?;
    }

    if let Some(amount) = req.amount {
        ensure_positive_amount(amount)?;
    }

    validate_optional_file_url(req.receipt_url.as_deref())?;

    let updated = claims::update_full(
        pool,
        claim_id,
        company_id,
        req.employee_id,
        req.title,
        req.description,
        req.amount,
        req.category,
        req.receipt_url,
        req.receipt_file_name,
        req.expense_date,
    )
    .await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update_claim_admin",
        "claim",
        Some(updated.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&updated).unwrap_or_default()),
        Some(&format!(
            "Updated claim for employee {}",
            updated.employee_id
        )),
        audit_meta,
    )
    .await;

    Ok(updated)
}

pub async fn delete_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let current = claims::get_deletable(pool, claim_id, company_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be deleted".into()))?;

    claims::delete(pool, claim_id, company_id).await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete_claim_admin",
        "claim",
        Some(current.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        None,
        Some(&format!(
            "Deleted claim for employee {}",
            current.employee_id
        )),
        audit_meta,
    )
    .await;

    Ok(())
}

pub async fn cancel_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Claim> {
    let mut tx = pool.begin().await?;

    // No `payroll_entries` probe any more. It asked whether a staged
    // reimbursement keyed to the *approval* month had been processed — a row
    // that never carried money and that the next run flipped to processed
    // regardless of whether the claim was paid, which is what made an unpaid
    // claim uncancellable. A paid claim is now simply not 'approved': it is
    // 'processed' with a `payroll_run_id`, which `get_cancellable` excludes.
    let current = claims::get_cancellable(&mut *tx, claim_id, company_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be cancelled".into()))?;

    // Compare-and-swap: a run committing between the read above and this write
    // would otherwise have its just-paid claim cancelled out from under it.
    let cancelled = claims::set_cancelled(&mut *tx, claim_id, company_id)
        .await?
        .ok_or_else(|| {
            AppError::Conflict(
                "This claim was paid or cancelled while it was being cancelled. Reload and try again."
                    .into(),
            )
        })?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "cancel_claim_admin",
        "claim",
        Some(cancelled.id),
        Some(serde_json::to_value(&current).unwrap_or_default()),
        Some(serde_json::to_value(&cancelled).unwrap_or_default()),
        Some(&format!(
            "Cancelled claim for employee {}",
            cancelled.employee_id
        )),
        audit_meta,
    )
    .await;

    Ok(cancelled)
}

// ─── Claims Approval ───

pub async fn get_claim_with_employee_by_id(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
) -> AppResult<ClaimWithEmployee> {
    approval_reads::claim_with_employee_by_id(pool, company_id, claim_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Claim not found".into()))
}

/// One page of the claims inbox, plus the true total matching the filter.
pub async fn get_pending_claims(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<ClaimWithEmployee>, i64)> {
    let total = approval_reads::count_claims(pool, company_id, status).await?;
    let claims =
        approval_reads::list_pending_claims(pool, company_id, status, limit, offset).await?;
    Ok((claims, total))
}

pub async fn approve_claim(
    pool: &PgPool,
    config: &AppConfig,
    company_id: Uuid,
    claim_id: Uuid,
    reviewer: Reviewer,
    notes: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Claim> {
    let reviewer_id = reviewer.user_id;

    // Approval no longer stages a `payroll_entries` reimbursement. That row was
    // keyed to the approval month, was excluded from every entry read, and so
    // never carried money — while the run paid from `claims` on an unrelated
    // predicate. `claims` is the single authority now: the run selects
    // approved-and-unlinked claims and marks exactly those it paid.
    //
    // Resolved before the transaction opens; the guard itself runs inside it,
    // against the row the compare-and-swap returned.
    let reviewer_employee = reviewer_employee_id(pool, &reviewer).await?;

    let mut tx = pool.begin().await?;
    let claim = claims::set_approved(&mut *tx, claim_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("Claim not found or not pending".into()))?;

    ensure_not_self_approval(reviewer_employee, claim.employee_id, "claim")?;

    tx.commit().await?;

    audit_self_approval_override(
        pool,
        &reviewer,
        company_id,
        "claim",
        claim.id,
        claim.employee_id,
        audit_meta,
    )
    .await;

    // Notify employee
    let employee_user = user_repo::active_id_for_employee(pool, claim.employee_id).await?;

    if let Some(user_id) = employee_user {
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "claim_approved",
            "Claim Approved",
            &format!(
                "Your claim \"{}\" (RM {:.2}) has been approved and will be included in your next payroll.",
                claim.title, claim.amount as f64 / 100.0
            ),
            Some("claim"),
            Some(claim.id),
        )
        .await;
    }

    // Send approval email
    let emp_info = approval_reads::employee_email_info(pool, claim.employee_id).await?;

    if let Some(emp) = emp_info {
        let amount_rm = claim.amount as f64 / 100.0;
        let details = format!(
            "<strong>Claim:</strong> {}<br><strong>Amount:</strong> RM {:.2}<br><strong>Category:</strong> {}",
            claim.title,
            amount_rm,
            claim.category.as_deref().unwrap_or("General")
        );
        let body = email_service::approval_email_html(
            &emp.full_name,
            &emp.company_name,
            "Claim",
            &details,
            "The approved amount will be included in your next payroll.",
        );
        let _ = email_service::send_email(
            config,
            pool,
            company_id,
            Some(claim.employee_id),
            None,
            "claim_approved",
            &emp.email,
            &emp.full_name,
            &format!("Claim Approved - {} (RM {:.2})", claim.title, amount_rm),
            &body,
            reviewer_id,
        )
        .await;
    }

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(reviewer_id),
        "approve_claim",
        "claim",
        Some(claim.id),
        None,
        Some(serde_json::to_value(&claim).unwrap_or_default()),
        Some(&format!(
            "Approved claim for employee {}",
            claim.employee_id
        )),
        audit_meta,
    )
    .await;

    Ok(claim)
}

pub async fn reject_claim(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Claim> {
    let claim = claims::set_rejected(pool, claim_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("Claim not found or not pending".into()))?;

    let employee_user = user_repo::active_id_for_employee(pool, claim.employee_id).await?;

    if let Some(user_id) = employee_user {
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "claim_rejected",
            "Claim Rejected",
            &format!(
                "Your claim \"{}\" has been rejected.{}",
                claim.title,
                notes.map(|n| format!(" Reason: {}", n)).unwrap_or_default()
            ),
            Some("claim"),
            Some(claim.id),
        )
        .await;
    }

    // Audit Log
    let _ = crate::services::audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(reviewer_id),
        "reject_claim",
        "claim",
        Some(claim.id),
        None,
        Some(serde_json::to_value(&claim).unwrap_or_default()),
        Some(&format!(
            "Rejected claim for employee {}",
            claim.employee_id
        )),
        audit_meta,
    )
    .await;

    Ok(claim)
}
