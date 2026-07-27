use lettre::address::AddressError;
use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use crate::models::email::{
    CreateEmailTemplateRequest, EmailLog, EmailLogSummary, EmailTemplate,
    UpdateEmailTemplateRequest,
};
use crate::repositories::{email_logs, email_templates};

// ── Template CRUD ──────────────────────────────────────────────────────

pub async fn list_templates(
    pool: &PgPool,
    company_id: Uuid,
    letter_type: Option<&str>,
) -> AppResult<Vec<EmailTemplate>> {
    email_templates::list(pool, company_id, letter_type).await
}

pub async fn get_template(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<EmailTemplate> {
    email_templates::get(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Template not found".into()))
}

pub async fn create_template(
    pool: &PgPool,
    company_id: Uuid,
    req: CreateEmailTemplateRequest,
    created_by: Uuid,
) -> AppResult<EmailTemplate> {
    email_templates::insert(pool, company_id, &req, created_by).await
}

pub async fn update_template(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    req: UpdateEmailTemplateRequest,
    updated_by: Uuid,
) -> AppResult<EmailTemplate> {
    email_templates::update(pool, id, company_id, &req, updated_by)
        .await?
        .ok_or_else(|| AppError::NotFound("Template not found".into()))
}

pub async fn delete_template(pool: &PgPool, id: Uuid, company_id: Uuid) -> AppResult<()> {
    if email_templates::delete(pool, id, company_id).await? == 0 {
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
) -> AppResult<(Vec<EmailLogSummary>, i64)> {
    let total = email_logs::count(pool, company_id, employee_id).await?;
    let logs = email_logs::list(pool, company_id, employee_id, limit, offset).await?;
    Ok((logs, total))
}

// ── Variable Substitution ──────────────────────────────────────────────

/// Replace template variables like {{employee_name}}, {{company_name}}, etc.
#[allow(clippy::too_many_arguments)]
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

/// A `Mailbox` built from the display name and the address separately.
///
/// Deliberately not `format!("{name} <{email}>").parse()`, which made the
/// display name part of the *address* grammar: an admin-entered recipient name
/// of `Foo <bar>` — or one carrying a comma or a newline — failed to parse, and
/// on the old ordering that failure orphaned an `email_logs` row at `pending`.
/// `Mailbox` quotes the name itself, so the header-injection shape stops being
/// expressible as a side effect.
fn mailbox(name: &str, email: &str) -> Result<Mailbox, AddressError> {
    let address: Address = email.parse()?;
    let display_name = if name.trim().is_empty() {
        None
    } else {
        Some(name.to_string())
    };
    Ok(Mailbox::new(display_name, address))
}

/// The configured SMTP relay.
///
/// Fallible, so it is built before anything is written to `email_logs` rather
/// than between the insert and the send. Only called once `smtp_enabled()` has
/// answered true, but it reads the config defensively so a half-configured
/// deployment produces a 500 with a message rather than a panic.
fn build_transport(config: &AppConfig) -> AppResult<AsyncSmtpTransport<Tokio1Executor>> {
    let smtp_host = config.smtp_host.as_deref().unwrap_or_default();
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
        .map_err(|e| AppError::Internal(format!("SMTP connection error: {}", e)))?
        .port(config.smtp_port.unwrap_or(587));

    if let (Some(user), Some(pass)) = (&config.smtp_username, &config.smtp_password) {
        builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
    }

    Ok(builder.build())
}

/// The `From:` mailbox, from config. An error here is a deployment fault, not
/// the caller's, hence `Internal` rather than `Validation`.
fn from_mailbox(config: &AppConfig) -> AppResult<Mailbox> {
    let from_name = config.smtp_from_name.as_deref().unwrap_or("PayrollMY");
    let from_email = config.smtp_from_email.as_deref().unwrap_or_default();
    mailbox(from_name, from_email)
        .map_err(|e| AppError::Internal(format!("Invalid from address: {}", e)))
}

/// Send a letter and record it, storing the message verbatim.
#[allow(clippy::too_many_arguments)]
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
    send_email_with_stored_body(
        config,
        pool,
        company_id,
        employee_id,
        template_id,
        letter_type,
        recipient_email,
        recipient_name,
        subject,
        body_html,
        body_html,
        created_by,
    )
    .await
}

/// Send a letter while recording a *different* body in `email_logs`.
///
/// Needed because the log row is written before the SMTP attempt and survives
/// it — on a deployment with SMTP disabled it is the only thing that happens —
/// so anything in the message that must not be retained has to be replaced
/// before the insert, not after. Today the welcome letter is the only caller:
/// its body carries the account's initial password.
#[allow(clippy::too_many_arguments)]
pub async fn send_email_with_stored_body(
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
    stored_body_html: &str,
    created_by: Uuid,
) -> AppResult<EmailLog> {
    // Validated before anything is written. A recipient address the mail
    // library cannot parse used to be discovered *after* `insert_pending`, on
    // an error path that never marked the row terminal — so the only trace a
    // mistyped address left was a log row stuck at `pending` for ever. There is
    // no backend validation anywhere else on the letter path: the frontend's
    // `includes('@')` was it.
    let to = mailbox(recipient_name, recipient_email).map_err(|_| {
        AppError::Validation(format!("{recipient_email} is not a valid email address"))
    })?;

    // Every remaining fallible step happens before the insert, so that nothing
    // between the row appearing and its terminal status can fail. That ordering
    // is the fix: four `Err` paths used to sit inside that gap.
    let prepared = if config.smtp_enabled() {
        let email = Message::builder()
            .from(from_mailbox(config)?)
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body_html.to_string())
            .map_err(|e| AppError::Internal(format!("Failed to build email: {}", e)))?;

        Some((email, build_transport(config)?))
    } else {
        None
    };

    let log = email_logs::insert_pending(
        pool,
        company_id,
        employee_id,
        template_id,
        letter_type,
        recipient_email,
        recipient_name,
        subject,
        stored_body_html,
        created_by,
    )
    .await?;

    // SMTP disabled is the documented production default, and the letter is
    // still recorded — it just never becomes `sent`.
    let Some((email, transport)) = prepared else {
        let log = email_logs::mark_failed_not_configured(pool, log.id).await?;
        tracing::warn!("SMTP not configured, email logged but not sent: {}", log.id);
        return Ok(log);
    };

    match transport.send(email).await {
        Ok(_) => {
            let log = email_logs::mark_sent(pool, log.id).await?;
            tracing::info!("Email sent successfully: {} to {}", log.id, recipient_email);
            Ok(log)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let log = email_logs::mark_failed(pool, log.id, &error_msg).await?;
            tracing::error!("Failed to send email {}: {}", log.id, error_msg);
            Ok(log)
        }
    }
}

// ── System Email (no DB logging, no company context) ──────────────────

pub async fn send_system_email(
    config: &AppConfig,
    recipient_email: &str,
    recipient_name: &str,
    subject: &str,
    body_html: &str,
) -> AppResult<()> {
    if !config.smtp_enabled() {
        tracing::warn!(
            "SMTP not configured, skipping system email to {}",
            recipient_email
        );
        return Ok(());
    }

    let to = mailbox(recipient_name, recipient_email).map_err(|_| {
        AppError::Validation(format!("{recipient_email} is not a valid email address"))
    })?;

    let email = Message::builder()
        .from(from_mailbox(config)?)
        .to(to)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(body_html.to_string())
        .map_err(|e| AppError::Internal(format!("Failed to build email: {}", e)))?;

    let transport = build_transport(config)?;

    transport
        .send(email)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to send email: {}", e)))?;

    tracing::info!("System email sent to {}", recipient_email);
    Ok(())
}

// ── Password Reset Email ──────────────────────────────────────────────

pub fn password_reset_html(user_name: &str, reset_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; color: #333;">
  <div style="background: #000; color: #fff; padding: 24px; border-radius: 12px 12px 0 0; text-align: center;">
    <h1 style="margin: 0; font-size: 24px;">Password Reset</h1>
  </div>
  <div style="border: 1px solid #e5e7eb; border-top: none; padding: 24px; border-radius: 0 0 12px 12px;">
    <p>Hi <strong>{user_name}</strong>,</p>
    <p>We received a request to reset your password. Click the button below to set a new password:</p>
    <p style="text-align: center; margin: 24px 0;">
      <a href="{reset_url}" style="background: #000; color: #fff; padding: 12px 32px; border-radius: 8px; text-decoration: none; font-weight: 600; display: inline-block;">
        Reset Password
      </a>
    </p>
    <p style="font-size: 13px; color: #6b7280;">This link will expire in 1 hour. If you didn't request this, you can safely ignore this email.</p>
  </div>
  <p style="text-align: center; font-size: 12px; color: #9ca3af; margin-top: 16px;">
    This is an automated message from PayrollMY.
  </p>
</body>
</html>"#,
        user_name = user_name,
        reset_url = reset_url,
    )
}

// ── Welcome Email ──────────────────────────────────────────────────────

// ── Approval Notification Email ───────────────────────────────────────

pub fn approval_email_html(
    employee_name: &str,
    company_name: &str,
    approval_type: &str, // "Leave", "Claim"
    details: &str,       // e.g. "Annual Leave from 2026-04-15 to 2026-04-17"
    extra_note: &str,    // e.g. "A salary deduction will be applied..." or ""
) -> String {
    let extra_section = if extra_note.is_empty() {
        String::new()
    } else {
        format!(
            r#"<p style="font-size: 13px; color: #d97706; background: #fffbeb; padding: 10px 14px; border-radius: 8px;">{}</p>"#,
            extra_note
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; color: #333;">
  <div style="background: #000; color: #fff; padding: 24px; border-radius: 12px 12px 0 0; text-align: center;">
    <h1 style="margin: 0; font-size: 24px;">{approval_type} Approved</h1>
  </div>
  <div style="border: 1px solid #e5e7eb; border-top: none; padding: 24px; border-radius: 0 0 12px 12px;">
    <p>Dear <strong>{employee_name}</strong>,</p>
    <p>Your {approval_type_lower} request has been <span style="color: #059669; font-weight: 600;">approved</span>.</p>
    <div style="background: #f9fafb; border: 1px solid #e5e7eb; border-radius: 8px; padding: 16px; margin: 16px 0;">
      <p style="margin: 0; font-size: 14px;">{details}</p>
    </div>
    {extra_section}
    <p style="margin-top: 24px;">Best regards,<br><strong>{company_name} HR Team</strong></p>
  </div>
  <p style="text-align: center; font-size: 12px; color: #9ca3af; margin-top: 16px;">
    This is an automated message from PayrollMY.
  </p>
</body>
</html>"#,
        approval_type = approval_type,
        approval_type_lower = approval_type.to_lowercase(),
        employee_name = employee_name,
        details = details,
        extra_section = extra_section,
        company_name = company_name,
    )
}

/// What the stored copy of a welcome letter shows where the password was.
pub const WELCOME_LOG_PASSWORD_PLACEHOLDER: &str =
    "(initial password sent to the employee — not recorded)";

/// The welcome letter as the employee receives it, credential included.
pub fn default_welcome_html(
    employee_name: &str,
    company_name: &str,
    frontend_url: &str,
    login_email: &str,
    default_password: &str,
) -> String {
    welcome_html(
        employee_name,
        company_name,
        frontend_url,
        login_email,
        default_password,
    )
}

/// The copy of the welcome letter kept in `email_logs`.
///
/// Identical to the sent message except for the password cell. The initial
/// password *is* the employee's IC number, and `email_logs` is readable by
/// every role holding `ViewEmailLogs` — including `exec`, whose view of the
/// employee record has `ic_number` stripped for exactly this reason. The log
/// still proves what was sent, to whom, and when; only the credential is
/// withheld.
pub fn welcome_log_html(
    employee_name: &str,
    company_name: &str,
    frontend_url: &str,
    login_email: &str,
) -> String {
    welcome_html(
        employee_name,
        company_name,
        frontend_url,
        login_email,
        WELCOME_LOG_PASSWORD_PLACEHOLDER,
    )
}

/// Compose and send the welcome letter for a freshly provisioned portal account.
///
/// It lives in the service because it decides *what is retained*, not merely
/// what is rendered — the handler has no business making that call.
#[allow(clippy::too_many_arguments)]
pub async fn send_welcome_email(
    config: &AppConfig,
    pool: &PgPool,
    company_id: Uuid,
    company_name: &str,
    employee_id: Uuid,
    employee_name: &str,
    login_email: &str,
    default_password: &str,
    created_by: Uuid,
) -> AppResult<EmailLog> {
    let subject = format!("Welcome to {} - PayrollMY", company_name);
    let body_html = default_welcome_html(
        employee_name,
        company_name,
        &config.frontend_url,
        login_email,
        default_password,
    );
    let stored_body_html = welcome_log_html(
        employee_name,
        company_name,
        &config.frontend_url,
        login_email,
    );

    send_email_with_stored_body(
        config,
        pool,
        company_id,
        Some(employee_id),
        None,
        "welcome",
        login_email,
        employee_name,
        &subject,
        &body_html,
        &stored_body_html,
        created_by,
    )
    .await
}

fn welcome_html(
    employee_name: &str,
    company_name: &str,
    frontend_url: &str,
    login_email: &str,
    password_cell: &str,
) -> String {
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
    <p>An account has been created for you. Here are your login details:</p>
    <div style="background: #f9fafb; border: 1px solid #e5e7eb; border-radius: 8px; padding: 16px; margin: 16px 0;">
      <table style="width: 100%; border-collapse: collapse;">
        <tr>
          <td style="padding: 4px 8px; color: #6b7280; font-size: 14px;">Email</td>
          <td style="padding: 4px 8px; font-weight: 600; font-size: 14px;">{login_email}</td>
        </tr>
        <tr>
          <td style="padding: 4px 8px; color: #6b7280; font-size: 14px;">Password</td>
          <td style="padding: 4px 8px; font-weight: 600; font-family: monospace; font-size: 14px;">{password_cell}</td>
        </tr>
      </table>
    </div>
    <p style="font-size: 13px; color: #d97706; background: #fffbeb; padding: 10px 14px; border-radius: 8px;">You will be asked to change your password on first login.</p>
    <p>You can access the employee portal to view your payslips, submit leave requests, claims, and more:</p>
    <p style="text-align: center; margin: 24px 0;">
      <a href="{frontend_url}/login" style="background: #000; color: #fff; padding: 12px 32px; border-radius: 8px; text-decoration: none; font-weight: 600; display: inline-block;">
        Login to Employee Portal
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
        login_email = login_email,
        password_cell = password_cell,
    )
}
