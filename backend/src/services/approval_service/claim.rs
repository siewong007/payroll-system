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
    Reviewer, audit_self_approval_override, ensure_employee_in_company,
    ensure_not_creator_or_editor, ensure_not_self_approval_on, ensure_positive_amount,
};
use crate::models::upload::UploadAccess;
use crate::services::upload_service;

pub use crate::models::approval::ClaimWithEmployee;

pub async fn create_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateClaimRequest,
    actor_id: Uuid,
    access: &UploadAccess,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<Claim> {
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    ensure_positive_amount(req.amount)?;
    // `receipt_url` is joined onto a filesystem path by the backup export and
    // restore paths, so it is settled at write time rather than at every sink.
    validate_optional_file_url(req.receipt_url.as_deref())?;
    upload_service::ensure_upload_attach_authorized(
        pool,
        access,
        req.receipt_url.as_deref(),
        false,
    )
    .await?;

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
        actor_id,
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
    access: &UploadAccess,
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

    let non_empty_receipt = req.receipt_url.as_deref().filter(|s| !s.is_empty());
    validate_optional_file_url(non_empty_receipt)?;
    upload_service::ensure_upload_attach_authorized(pool, access, non_empty_receipt, false).await?;

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
        actor_id,
    )
    .await?
    .ok_or_else(|| {
        AppError::Conflict(
            "This claim changed state while it was being updated. Reload and try again.".into(),
        )
    })?;

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

    let rows = claims::delete(pool, claim_id, company_id).await?;
    if rows == 0 {
        return Err(AppError::Conflict(
            "This claim changed state while it was being deleted. Reload and try again.".into(),
        ));
    }

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
    let mailer = email_service::SharedMailer::from_config(config);
    approve_claim_with_mailer(
        pool, &mailer, company_id, claim_id, reviewer, notes, audit_meta,
    )
    .await
}

pub async fn approve_claim_with_mailer(
    pool: &PgPool,
    mailer: &email_service::SharedMailer,
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
    // Self-approval guard runs inside the transaction against `claim.employee_id`.
    let mut tx = pool.begin().await?;
    let claim = claims::set_approved(&mut *tx, claim_id, company_id, reviewer_id, notes)
        .await?
        .ok_or_else(|| AppError::BadRequest("Claim not found or not pending".into()))?;

    ensure_not_self_approval_on(&mut *tx, &reviewer, company_id, claim.employee_id, "claim")
        .await?;
    ensure_not_creator_or_editor(
        &reviewer,
        claim.created_by,
        claim.updated_by,
        &claim.editor_ids,
        "claim",
    )?;

    tx.commit().await?;

    audit_self_approval_override(
        pool,
        &reviewer,
        company_id,
        "claim",
        claim.id,
        claim.employee_id,
        claim.created_by,
        claim.updated_by,
        &claim.editor_ids,
        audit_meta,
    )
    .await;

    // Notify employee
    let employee_user = user_repo::active_id_for_employee(pool, claim.employee_id).await?;
    let amount_rm = crate::services::pdf_helpers::sen_to_rm(claim.amount);

    if let Some(user_id) = employee_user {
        let _ = notification_service::create_notification(
            pool,
            user_id,
            company_id,
            "claim_approved",
            "Claim Approved",
            &format!(
                "Your claim \"{}\" (RM {}) has been approved and will be included in your next payroll.",
                claim.title, amount_rm
            ),
            Some("claim"),
            Some(claim.id),
        )
        .await;
    }

    // Send approval email
    let emp_info = approval_reads::employee_email_info(pool, claim.employee_id).await?;

    if let Some(emp) = emp_info
        && let Some(recipient_email) = emp.email.as_deref().filter(|e| !e.trim().is_empty())
    {
        let amount_str = format!("RM {}", amount_rm);
        let category = claim.category.as_deref().unwrap_or("General");
        let body = email_service::approval_email_html(
            &emp.full_name,
            &emp.company_name,
            "Claim",
            &[
                ("Claim", &claim.title),
                ("Amount", &amount_str),
                ("Category", category),
            ],
            "The approved amount will be included in your next payroll.",
        );
        let _ = email_service::enqueue_email_with_mailer(
            mailer,
            pool,
            company_id,
            Some(claim.employee_id),
            None,
            "claim_approved",
            recipient_email,
            &emp.full_name,
            &format!("Claim Approved - {} (RM {})", claim.title, amount_rm),
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
