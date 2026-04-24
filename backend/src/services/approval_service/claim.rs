//! Claim admin CRUD + approval/reject workflow.

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::models::portal::{Claim, CreateClaimRequest, UpdateClaimRequest};
use crate::services::email_service;
use crate::services::notification_service;

use super::common::{ensure_employee_in_company, ensure_positive_amount};

pub async fn create_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    req: CreateClaimRequest,
    actor_id: Uuid,
) -> AppResult<Claim> {
    ensure_employee_in_company(pool, company_id, employee_id).await?;
    ensure_positive_amount(req.amount)?;

    let claim = sqlx::query_as::<_, Claim>(
        r#"INSERT INTO claims
            (employee_id, company_id, title, description, amount, category, receipt_url, receipt_file_name, expense_date, status, submitted_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', NOW())
        RETURNING *"#,
    )
    .bind(employee_id)
    .bind(company_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.amount)
    .bind(&req.category)
    .bind(&req.receipt_url)
    .bind(&req.receipt_file_name)
    .bind(req.expense_date)
    .fetch_one(pool)
    .await?;

    let _ = crate::services::audit_service::log_action(
        pool,
        Some(actor_id),
        "create_claim_admin",
        "claim",
        Some(claim.id),
        None,
        Some(serde_json::to_value(&claim).unwrap_or_default()),
        Some(&format!("Created claim for employee {}", claim.employee_id)),
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
) -> AppResult<Claim> {
    let current = sqlx::query_as::<_, Claim>(
        r#"SELECT * FROM claims
        WHERE id = $1 AND company_id = $2
        AND status IN ('draft', 'pending', 'rejected')"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be edited".into()))?;

    if let Some(employee_id) = req.employee_id {
        ensure_employee_in_company(pool, company_id, employee_id).await?;
    }

    if let Some(amount) = req.amount {
        ensure_positive_amount(amount)?;
    }

    let updated = sqlx::query_as::<_, Claim>(
        r#"UPDATE claims
        SET employee_id = COALESCE($3, employee_id),
            title = COALESCE($4, title),
            description = CASE WHEN $5::text IS NULL THEN description ELSE NULLIF($5, '') END,
            amount = COALESCE($6, amount),
            category = CASE WHEN $7::text IS NULL THEN category ELSE NULLIF($7, '') END,
            receipt_url = CASE WHEN $8::text IS NULL THEN receipt_url ELSE NULLIF($8, '') END,
            receipt_file_name = CASE WHEN $9::text IS NULL THEN receipt_file_name ELSE NULLIF($9, '') END,
            expense_date = COALESCE($10, expense_date),
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .bind(req.employee_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.amount)
    .bind(&req.category)
    .bind(&req.receipt_url)
    .bind(&req.receipt_file_name)
    .bind(req.expense_date)
    .fetch_one(pool)
    .await?;

    let _ = crate::services::audit_service::log_action(
        pool,
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
    )
    .await;

    Ok(updated)
}

pub async fn delete_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    actor_id: Uuid,
) -> AppResult<()> {
    let current = sqlx::query_as::<_, Claim>(
        r#"SELECT * FROM claims
        WHERE id = $1 AND company_id = $2
        AND status IN ('draft', 'cancelled')"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be deleted".into()))?;

    sqlx::query("DELETE FROM claims WHERE id = $1 AND company_id = $2")
        .bind(claim_id)
        .bind(company_id)
        .execute(pool)
        .await?;

    let _ = crate::services::audit_service::log_action(
        pool,
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
    )
    .await;

    Ok(())
}

pub async fn cancel_claim_admin(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    actor_id: Uuid,
) -> AppResult<Claim> {
    let mut tx = pool.begin().await?;

    let current = sqlx::query_as::<_, Claim>(
        r#"SELECT * FROM claims
        WHERE id = $1
          AND company_id = $2
          AND status IN ('pending', 'approved', 'rejected')"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or cannot be cancelled".into()))?;

    if current.status == "approved" {
        let staged_at = current.reviewed_at.unwrap_or_else(chrono::Utc::now);
        let description = format!("Claim: {}", current.title);
        let processed = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS(
                SELECT 1 FROM payroll_entries
                WHERE employee_id = $1
                  AND company_id = $2
                  AND period_year = $3
                  AND period_month = $4
                  AND item_type = 'claim_reimbursement'
                  AND description = $5
                  AND amount = $6
                  AND is_processed = TRUE
            )"#,
        )
        .bind(current.employee_id)
        .bind(company_id)
        .bind(staged_at.year())
        .bind(staged_at.month() as i32)
        .bind(&description)
        .bind(current.amount)
        .fetch_one(&mut *tx)
        .await?;

        if processed {
            return Err(AppError::BadRequest(
                "Approved claim already included in processed payroll and cannot be cancelled"
                    .into(),
            ));
        }

        sqlx::query(
            r#"DELETE FROM payroll_entries
            WHERE employee_id = $1
              AND company_id = $2
              AND period_year = $3
              AND period_month = $4
              AND item_type = 'claim_reimbursement'
              AND description = $5
              AND amount = $6
              AND is_processed = FALSE"#,
        )
        .bind(current.employee_id)
        .bind(company_id)
        .bind(staged_at.year())
        .bind(staged_at.month() as i32)
        .bind(&description)
        .bind(current.amount)
        .execute(&mut *tx)
        .await?;
    }

    let cancelled = sqlx::query_as::<_, Claim>(
        r#"UPDATE claims
        SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let _ = crate::services::audit_service::log_action(
        pool,
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
    )
    .await;

    Ok(cancelled)
}

// ─── Claims Approval ───

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ClaimWithEmployee {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub company_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub amount: i64,
    pub category: Option<String>,
    pub receipt_url: Option<String>,
    pub receipt_file_name: Option<String>,
    pub expense_date: chrono::NaiveDate,
    pub status: String,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub review_notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub employee_name: Option<String>,
    pub employee_number: Option<String>,
}

pub async fn get_claim_with_employee_by_id(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
) -> AppResult<ClaimWithEmployee> {
    sqlx::query_as::<_, ClaimWithEmployee>(
        r#"SELECT c.*,
            e.full_name as employee_name,
            e.employee_number
        FROM claims c
        JOIN employees e ON c.employee_id = e.id
        WHERE c.id = $1 AND c.company_id = $2"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Claim not found".into()))
}

pub async fn get_pending_claims(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
) -> AppResult<Vec<ClaimWithEmployee>> {
    let claims = sqlx::query_as::<_, ClaimWithEmployee>(
        r#"SELECT c.*,
            e.full_name as employee_name,
            e.employee_number
        FROM claims c
        JOIN employees e ON c.employee_id = e.id
        WHERE c.company_id = $1
        AND ($2::text IS NULL OR c.status = $2)
        ORDER BY c.created_at DESC
        LIMIT 100"#,
    )
    .bind(company_id)
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(claims)
}

pub async fn approve_claim(
    pool: &PgPool,
    config: &AppConfig,
    company_id: Uuid,
    claim_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Claim> {
    let claim = sqlx::query_as::<_, Claim>(
        r#"UPDATE claims SET
            status = 'approved', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .bind(reviewer_id)
    .bind(notes)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or not pending".into()))?;

    // Auto-create payroll entry for the approved claim amount
    // Stage it for the current payroll period (current month)
    let now = chrono::Utc::now();
    let period_year = now.year();
    let period_month = now.month() as i32;

    let _ = sqlx::query(
        r#"INSERT INTO payroll_entries
            (id, employee_id, company_id, period_year, period_month, category, item_type, description, amount, created_by)
        VALUES ($1, $2, $3, $4, $5, 'earning', 'claim_reimbursement', $6, $7, $8)"#,
    )
    .bind(Uuid::new_v4())
    .bind(claim.employee_id)
    .bind(company_id)
    .bind(period_year)
    .bind(period_month)
    .bind(format!("Claim: {}", claim.title))
    .bind(claim.amount)
    .bind(reviewer_id)
    .execute(pool)
    .await;

    // Notify employee
    let employee_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
    )
    .bind(claim.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((user_id,)) = employee_user {
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
    let emp_info: Option<(String, String, String)> = sqlx::query_as(
        r#"SELECT e.full_name, e.email, COALESCE(c.name, '') as company_name
        FROM employees e
        JOIN companies c ON e.company_id = c.id
        WHERE e.id = $1"#,
    )
    .bind(claim.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((emp_name, emp_email, company_name)) = emp_info {
        let amount_rm = claim.amount as f64 / 100.0;
        let details = format!(
            "<strong>Claim:</strong> {}<br><strong>Amount:</strong> RM {:.2}<br><strong>Category:</strong> {}",
            claim.title,
            amount_rm,
            claim.category.as_deref().unwrap_or("General")
        );
        let body = email_service::approval_email_html(
            &emp_name,
            &company_name,
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
            &emp_email,
            &emp_name,
            &format!("Claim Approved - {} (RM {:.2})", claim.title, amount_rm),
            &body,
            reviewer_id,
        )
        .await;
    }

    Ok(claim)
}

pub async fn reject_claim(
    pool: &PgPool,
    company_id: Uuid,
    claim_id: Uuid,
    reviewer_id: Uuid,
    notes: Option<&str>,
) -> AppResult<Claim> {
    let claim = sqlx::query_as::<_, Claim>(
        r#"UPDATE claims SET
            status = 'rejected', reviewed_by = $3, reviewed_at = NOW(),
            review_notes = $4, updated_at = NOW()
        WHERE id = $1 AND company_id = $2 AND status = 'pending'
        RETURNING *"#,
    )
    .bind(claim_id)
    .bind(company_id)
    .bind(reviewer_id)
    .bind(notes)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Claim not found or not pending".into()))?;

    let employee_user = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE employee_id = $1 AND is_active = TRUE",
    )
    .bind(claim.employee_id)
    .fetch_optional(pool)
    .await?;

    if let Some((user_id,)) = employee_user {
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

    Ok(claim)
}
