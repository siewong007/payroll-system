use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::models::email::{
    CreateEmailTemplateRequest, EmailLog, EmailTemplate, UpdateEmailTemplateRequest,
};

// ── Template CRUD ──────────────────────────────────────────────────────

pub async fn list_templates(
    pool: &PgPool,
    company_id: Uuid,
    letter_type: Option<&str>,
) -> AppResult<Vec<EmailTemplate>> {
    let templates = sqlx::query_as::<_, EmailTemplate>(
        r#"SELECT * FROM email_templates
        WHERE company_id = $1 AND ($2::text IS NULL OR letter_type = $2)
        ORDER BY letter_type, name"#,
    )
    .bind(company_id)
    .bind(letter_type)
    .fetch_all(pool)
    .await?;
    Ok(templates)
}

pub async fn get_template(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
) -> AppResult<EmailTemplate> {
    sqlx::query_as::<_, EmailTemplate>(
        "SELECT * FROM email_templates WHERE id = $1 AND company_id = $2",
    )
    .bind(id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Template not found".into()))
}

pub async fn create_template(
    pool: &PgPool,
    company_id: Uuid,
    req: CreateEmailTemplateRequest,
    created_by: Uuid,
) -> AppResult<EmailTemplate> {
    let template = sqlx::query_as::<_, EmailTemplate>(
        r#"INSERT INTO email_templates (company_id, name, letter_type, subject, body_html, created_by, updated_by)
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING *"#,
    )
    .bind(company_id)
    .bind(&req.name)
    .bind(&req.letter_type)
    .bind(&req.subject)
    .bind(&req.body_html)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(template)
}

pub async fn update_template(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    req: UpdateEmailTemplateRequest,
    updated_by: Uuid,
) -> AppResult<EmailTemplate> {
    let template = sqlx::query_as::<_, EmailTemplate>(
        r#"UPDATE email_templates SET
            name = COALESCE($3, name),
            subject = COALESCE($4, subject),
            body_html = COALESCE($5, body_html),
            is_active = COALESCE($6, is_active),
            updated_by = $7,
            updated_at = NOW()
        WHERE id = $1 AND company_id = $2
        RETURNING *"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(&req.name)
    .bind(&req.subject)
    .bind(&req.body_html)
    .bind(req.is_active)
    .bind(updated_by)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Template not found".into()))?;
    Ok(template)
}

pub async fn delete_template(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<()> {
    let result = sqlx::query(
        "DELETE FROM email_templates WHERE id = $1 AND company_id = $2",
    )
    .bind(id)
    .bind(company_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Template not found".into()));
    }
    Ok(())
}

// ── Email Logs ─────────────────────────────────────────────────────────

pub async fn list_email_logs(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<EmailLog>, i64)> {
    let total: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM email_logs
        WHERE company_id = $1 AND ($2::uuid IS NULL OR employee_id = $2)"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .fetch_one(pool)
    .await?;

    let logs = sqlx::query_as::<_, EmailLog>(
        r#"SELECT * FROM email_logs
        WHERE company_id = $1 AND ($2::uuid IS NULL OR employee_id = $2)
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((logs, total.0))
}

// ── Variable Substitution ──────────────────────────────────────────────

/// Replace template variables like {{employee_name}}, {{company_name}}, etc.
pub fn substitute_variables(
    template: &str,
    employee_name: &str,
    employee_number: &str,
    employee_email: &str,
    designation: &str,
    department: &str,
    date_joined: &str,
    company_name: &str,
) -> String {
    template
        .replace("{{employee_name}}", employee_name)
        .replace("{{employee_number}}", employee_number)
        .replace("{{employee_email}}", employee_email)
        .replace("{{designation}}", designation)
        .replace("{{department}}", department)
        .replace("{{date_joined}}", date_joined)
        .replace("{{company_name}}", company_name)
}

// ── Send Email via SMTP ────────────────────────────────────────────────

pub async fn send_email(
    config: &AppConfig,
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Option<Uuid>,
    template_id: Option<Uuid>,
    letter_type: &str,
    recipient_email: &str,
    recipient_name: &str,
    subject: &str,
    body_html: &str,
    created_by: Uuid,
) -> AppResult<EmailLog> {
    // Create log entry first as pending
    let log = sqlx::query_as::<_, EmailLog>(
        r#"INSERT INTO email_logs
        (company_id, employee_id, template_id, letter_type, recipient_email, recipient_name, subject, body_html, status, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', $9)
        RETURNING *"#,
    )
    .bind(company_id)
    .bind(employee_id)
    .bind(template_id)
    .bind(letter_type)
    .bind(recipient_email)
    .bind(recipient_name)
    .bind(subject)
    .bind(body_html)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    if !config.smtp_enabled() {
        // Mark as failed if SMTP not configured
        let log = sqlx::query_as::<_, EmailLog>(
            r#"UPDATE email_logs SET status = 'failed', error_message = 'SMTP not configured'
            WHERE id = $1 RETURNING *"#,
        )
        .bind(log.id)
        .fetch_one(pool)
        .await?;
        tracing::warn!("SMTP not configured, email logged but not sent: {}", log.id);
        return Ok(log);
    }

    let smtp_host = config.smtp_host.as_deref().unwrap();
    let from_email = config.smtp_from_email.as_deref().unwrap();
    let from_name = config.smtp_from_name.as_deref().unwrap_or("PayrollMY");

    // Build email message
    let from_addr = format!("{} <{}>", from_name, from_email);
    let to_addr = if recipient_name.is_empty() {
        recipient_email.to_string()
    } else {
        format!("{} <{}>", recipient_name, recipient_email)
    };

    let email = Message::builder()
        .from(from_addr.parse().map_err(|e| AppError::Internal(format!("Invalid from address: {}", e)))?)
        .to(to_addr.parse().map_err(|e| AppError::Internal(format!("Invalid to address: {}", e)))?)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(body_html.to_string())
        .map_err(|e| AppError::Internal(format!("Failed to build email: {}", e)))?;

    // Build SMTP transport
    let port = config.smtp_port.unwrap_or(587);
    let mut transport_builder = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
        .map_err(|e| AppError::Internal(format!("SMTP connection error: {}", e)))?
        .port(port);

    if let (Some(user), Some(pass)) = (&config.smtp_username, &config.smtp_password) {
        transport_builder = transport_builder.credentials(Credentials::new(user.clone(), pass.clone()));
    }

    let transport = transport_builder.build();

    // Send
    match transport.send(email).await {
        Ok(_) => {
            let log = sqlx::query_as::<_, EmailLog>(
                r#"UPDATE email_logs SET status = 'sent', sent_at = NOW()
                WHERE id = $1 RETURNING *"#,
            )
            .bind(log.id)
            .fetch_one(pool)
            .await?;
            tracing::info!("Email sent successfully: {} to {}", log.id, recipient_email);
            Ok(log)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let log = sqlx::query_as::<_, EmailLog>(
                r#"UPDATE email_logs SET status = 'failed', error_message = $2
                WHERE id = $1 RETURNING *"#,
            )
            .bind(log.id)
            .bind(&error_msg)
            .fetch_one(pool)
            .await?;
            tracing::error!("Failed to send email {}: {}", log.id, error_msg);
            Ok(log)
        }
    }
}

// ── Welcome Email ──────────────────────────────────────────────────────

pub fn default_welcome_html(employee_name: &str, company_name: &str, frontend_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; color: #333;">
  <div style="background: #000; color: #fff; padding: 24px; border-radius: 12px 12px 0 0; text-align: center;">
    <h1 style="margin: 0; font-size: 24px;">Welcome to {company_name}</h1>
  </div>
  <div style="border: 1px solid #e5e7eb; border-top: none; padding: 24px; border-radius: 0 0 12px 12px;">
    <p>Dear <strong>{employee_name}</strong>,</p>
    <p>Welcome aboard! We're excited to have you join the team at <strong>{company_name}</strong>.</p>
    <p>You can access the employee portal to view your payslips, submit leave requests, claims, and more:</p>
    <p style="text-align: center; margin: 24px 0;">
      <a href="{frontend_url}/portal" style="background: #000; color: #fff; padding: 12px 32px; border-radius: 8px; text-decoration: none; font-weight: 600; display: inline-block;">
        Go to Employee Portal
      </a>
    </p>
    <p>If you have any questions, please reach out to your HR department.</p>
    <p style="margin-top: 24px;">Best regards,<br><strong>{company_name} HR Team</strong></p>
  </div>
  <p style="text-align: center; font-size: 12px; color: #9ca3af; margin-top: 16px;">
    This is an automated message from PayrollMY.
  </p>
</body>
</html>"#,
        company_name = company_name,
        employee_name = employee_name,
        frontend_url = frontend_url,
    )
}
