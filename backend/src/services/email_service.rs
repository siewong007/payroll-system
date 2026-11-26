use chrono::Utc;
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
    mut req: CreateEmailTemplateRequest,
    created_by: Uuid,
) -> AppResult<EmailTemplate> {
    req.body_html = sanitize_email_html(&req.body_html);
    email_templates::insert(pool, company_id, &req, created_by).await
}

pub async fn update_template(
    pool: &PgPool,
    id: Uuid,
    company_id: Uuid,
    mut req: UpdateEmailTemplateRequest,
    updated_by: Uuid,
) -> AppResult<EmailTemplate> {
    if let Some(ref mut body_html) = req.body_html {
        *body_html = sanitize_email_html(body_html);
    }
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

/// Escape HTML meta-characters before interpolating untrusted values into HTML bodies or attributes.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

fn sanitize_header_text(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\r' | '\n') && !c.is_control())
        .collect()
}

/// Sanitize user-authored HTML email templates/bodies via `ammonia`, stripping
/// scripts, event handlers, forms, iframes, stylesheets, and unsafe URI schemes
/// while preserving standard semantic formatting and `https`/`http`/`mailto` links.
pub fn sanitize_email_html(html: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder.url_schemes(std::collections::HashSet::from(["https", "http", "mailto"]));
    builder.clean(html).to_string()
}

fn redact_smtp_error(raw: &str) -> &'static str {
    if raw == "SMTP not configured" {
        "SMTP not configured"
    } else if raw.starts_with("Invalid recipient address") {
        "Invalid recipient email address"
    } else {
        "SMTP delivery failed"
    }
}

/// Replace template variables like {{employee_name}}, {{company_name}}, etc.
///
/// Single-pass scan so a substituted value that itself contains `{{...}}` is
/// never expanded recursively on a subsequent replacement pass.
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
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after_open = &rest[start..];
        if let Some(end_rel) = after_open[2..].find("}}") {
            let token_len = end_rel + 4;
            let token = &after_open[..token_len];
            let replacement = match token {
                "{{employee_name}}" => Some(employee_name),
                "{{employee_number}}" => Some(employee_number),
                "{{employee_email}}" => Some(employee_email),
                "{{designation}}" => Some(designation),
                "{{department}}" => Some(department),
                "{{date_joined}}" => Some(date_joined),
                "{{company_name}}" => Some(company_name),
                _ => None,
            };
            if let Some(val) = replacement {
                out.push_str(val);
                rest = &after_open[token_len..];
            } else {
                out.push_str("{{");
                rest = &after_open[2..];
            }
        } else {
            out.push_str(after_open);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Substitute variables into an HTML letter template, escaping dynamic values and sanitizing markup.
#[allow(clippy::too_many_arguments)]
pub fn substitute_variables_html(
    template: &str,
    employee_name: &str,
    employee_number: &str,
    employee_email: &str,
    designation: &str,
    department: &str,
    date_joined: &str,
    company_name: &str,
) -> String {
    let substituted = substitute_variables(
        template,
        &escape_html(employee_name),
        &escape_html(employee_number),
        &escape_html(employee_email),
        &escape_html(designation),
        &escape_html(department),
        &escape_html(date_joined),
        &escape_html(company_name),
    );
    sanitize_email_html(&substituted)
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
    let clean_name = sanitize_header_text(name);
    let display_name = if clean_name.trim().is_empty() {
        None
    } else {
        Some(clean_name)
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

#[derive(Clone)]
struct MailerInner {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

/// Shared, pooled SMTP transport built once at boot and cloned cheaply via
/// `AppState`.
#[derive(Clone)]
pub struct SharedMailer {
    inner: Option<MailerInner>,
    init_error: Option<String>,
}

impl SharedMailer {
    pub fn from_config(config: &AppConfig) -> Self {
        if !config.smtp_enabled() {
            return Self {
                inner: None,
                init_error: None,
            };
        }
        let from = match from_mailbox(config) {
            Ok(m) => m,
            Err(e) => {
                let msg = format!("{e}");
                tracing::error!(error = %msg, "Invalid SMTP From address; mailer disabled");
                return Self {
                    inner: None,
                    init_error: Some(msg),
                };
            }
        };
        let transport = match build_transport(config) {
            Ok(t) => t,
            Err(e) => {
                let msg = format!("{e}");
                tracing::error!(error = %msg, "Failed to initialize SMTP transport; mailer disabled");
                return Self {
                    inner: None,
                    init_error: Some(msg),
                };
            }
        };
        Self {
            inner: Some(MailerInner { transport, from }),
            init_error: None,
        }
    }

    pub fn disabled() -> Self {
        Self {
            inner: None,
            init_error: None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    async fn send_raw(
        &self,
        to_name: &str,
        to_email: &str,
        subject: &str,
        body_html: &str,
    ) -> Result<(), String> {
        let Some(ref inner) = self.inner else {
            return Err(self
                .init_error
                .clone()
                .unwrap_or_else(|| "SMTP not configured".to_string()));
        };
        let clean_subject = sanitize_header_text(subject);
        let to =
            mailbox(to_name, to_email).map_err(|e| format!("Invalid recipient address: {e}"))?;
        let email = Message::builder()
            .from(inner.from.clone())
            .to(to)
            .subject(&clean_subject)
            .header(ContentType::TEXT_HTML)
            .body(body_html.to_string())
            .map_err(|e| format!("Failed to build email: {e}"))?;
        inner
            .transport
            .send(email)
            .await
            .map(|_| ())
            .map_err(|e| format!("{e}"))
    }
}

/// Send a letter using the shared mailer and record it in `email_logs`.
#[allow(clippy::too_many_arguments)]
pub async fn send_email(
    mailer: &SharedMailer,
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
    let clean_subject = sanitize_header_text(subject);
    let clean_name = sanitize_header_text(recipient_name);
    let _ = mailbox(&clean_name, recipient_email).map_err(|_| {
        AppError::Validation(format!("{recipient_email} is not a valid email address"))
    })?;

    if !config.smtp_enabled() || !mailer.is_enabled() {
        let log = email_logs::insert_with_delivery_state(
            pool,
            company_id,
            employee_id,
            template_id,
            letter_type,
            recipient_email,
            &clean_name,
            &clean_subject,
            body_html,
            "failed",
            Some("SMTP not configured"),
            0,
            0,
            None,
            created_by,
        )
        .await?;
        tracing::warn!("SMTP not configured, email logged but not sent: {}", log.id);
        return Ok(log);
    }

    let log = email_logs::insert_with_delivery_state(
        pool,
        company_id,
        employee_id,
        template_id,
        letter_type,
        recipient_email,
        &clean_name,
        &clean_subject,
        body_html,
        "pending",
        None,
        1,
        3,
        Some(Utc::now() + chrono::Duration::minutes(5)),
        created_by,
    )
    .await?;

    match mailer
        .send_raw(&clean_name, recipient_email, &clean_subject, body_html)
        .await
    {
        Ok(()) => {
            let log = email_logs::mark_sent(pool, log.id).await?.unwrap_or(log);
            tracing::info!(
                "Email sent successfully: {} to {}",
                log.id,
                crate::core::redact::email(recipient_email)
            );
            Ok(log)
        }
        Err(error_msg) => {
            let next_retry = Utc::now() + chrono::Duration::seconds(60);
            let safe_err = redact_smtp_error(&error_msg);
            let log = email_logs::schedule_retry(pool, log.id, 1, next_retry, safe_err)
                .await?
                .unwrap_or(log);
            tracing::warn!(
                email_id = %log.id,
                error = %error_msg,
                "Initial SMTP delivery failed; queued in outbox for retry"
            );
            Ok(log)
        }
    }
}

/// Queue an email in `email_logs` for background outbox delivery without
/// blocking the caller on SMTP, checking the prebuilt [`SharedMailer`] rather
/// than re-evaluating transport initialization per request.
#[allow(clippy::too_many_arguments)]
pub async fn enqueue_email_with_mailer(
    mailer: &SharedMailer,
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
    let clean_subject = sanitize_header_text(subject);
    let clean_name = sanitize_header_text(recipient_name);
    let _ = mailbox(&clean_name, recipient_email).map_err(|_| {
        AppError::Validation(format!("{recipient_email} is not a valid email address"))
    })?;

    if !mailer.is_enabled() {
        return email_logs::insert_with_delivery_state(
            pool,
            company_id,
            employee_id,
            template_id,
            letter_type,
            recipient_email,
            &clean_name,
            &clean_subject,
            body_html,
            "failed",
            Some("SMTP not configured"),
            0,
            0,
            None,
            created_by,
        )
        .await;
    }

    email_logs::insert_with_delivery_state(
        pool,
        company_id,
        employee_id,
        template_id,
        letter_type,
        recipient_email,
        &clean_name,
        &clean_subject,
        body_html,
        "pending",
        None,
        0,
        3,
        Some(Utc::now()),
        created_by,
    )
    .await
}

/// Queue an email in `email_logs` for background outbox delivery without
/// blocking the caller on SMTP.
#[allow(clippy::too_many_arguments)]
pub async fn enqueue_email(
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
    let mailer = SharedMailer::from_config(config);
    enqueue_email_with_mailer(
        &mailer,
        pool,
        company_id,
        employee_id,
        template_id,
        letter_type,
        recipient_email,
        recipient_name,
        subject,
        body_html,
        created_by,
    )
    .await
}

/// Deliver a batch of due outbox emails with exponential retry backoff
/// (`60s` -> `300s`, failing permanently on attempt `>= max_attempts`).
pub async fn process_outbox_batch(
    pool: &PgPool,
    mailer: &SharedMailer,
    limit: i64,
) -> AppResult<(usize, usize)> {
    if !mailer.is_enabled() {
        return Ok((0, 0));
    }
    let rows = email_logs::claim_due_outbox(pool, limit).await?;
    let mut sent = 0usize;
    let mut failed = 0usize;

    for row in rows {
        let recipient_name = row.recipient_name.as_deref().unwrap_or("");
        match mailer
            .send_raw(
                recipient_name,
                &row.recipient_email,
                &row.subject,
                &row.body_html,
            )
            .await
        {
            Ok(()) => {
                email_logs::mark_sent(pool, row.id).await?;
                sent += 1;
            }
            Err(err) => {
                failed += 1;
                let safe_err = redact_smtp_error(&err);
                if row.attempts >= row.max_attempts {
                    email_logs::mark_failed(pool, row.id, safe_err).await?;
                    tracing::error!(
                        email_id = %row.id,
                        attempts = row.attempts,
                        "Outbox email exceeded max attempts and was marked failed: {}",
                        err
                    );
                } else {
                    let backoff_secs =
                        60i64 * 5i64.saturating_pow((row.attempts - 1).max(0) as u32);
                    let next_retry = Utc::now() + chrono::Duration::seconds(backoff_secs);
                    email_logs::schedule_retry(pool, row.id, row.attempts, next_retry, safe_err)
                        .await?;
                    tracing::warn!(
                        email_id = %row.id,
                        attempts = row.attempts,
                        next_retry_at = %next_retry,
                        "Outbox email send failed; scheduled retry: {}",
                        err
                    );
                }
            }
        }
    }

    Ok((sent, failed))
}

// ── System Email (no DB logging, no company context) ──────────────────

pub async fn send_system_email_with_retry(
    mailer: SharedMailer,
    recipient_email: String,
    recipient_name: String,
    subject: String,
    body_html: String,
) {
    if !mailer.is_enabled() {
        tracing::warn!(
            "SMTP not configured, skipping system email to {}",
            crate::core::redact::email(&recipient_email)
        );
        return;
    }

    const BACKOFF_SECS: [u64; 3] = [0, 5, 20];
    let mut last_err = String::new();
    for (idx, delay) in BACKOFF_SECS.iter().enumerate() {
        if *delay > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(*delay)).await;
        }
        match mailer
            .send_raw(&recipient_name, &recipient_email, &subject, &body_html)
            .await
        {
            Ok(()) => {
                tracing::info!(
                    attempt = idx + 1,
                    "System email sent to {}",
                    crate::core::redact::email(&recipient_email)
                );
                return;
            }
            Err(err) => {
                last_err = err;
                tracing::warn!(
                    attempt = idx + 1,
                    to = %crate::core::redact::email(&recipient_email),
                    error = %last_err,
                    "System email attempt failed"
                );
            }
        }
    }
    tracing::error!(
        to = %crate::core::redact::email(&recipient_email),
        error = %last_err,
        "Exhausted retries sending system email"
    );
}

// ── Password Reset Email ──────────────────────────────────────────────

pub fn password_reset_html(user_name: &str, reset_url: &str) -> String {
    let user_name = escape_html(user_name);
    let reset_url = escape_html(reset_url);
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
    details: &[(&str, &str)],
    extra_note: &str, // e.g. "A salary deduction will be applied..." or ""
) -> String {
    let extra_section = if extra_note.is_empty() {
        String::new()
    } else {
        format!(
            r#"<p style="font-size: 13px; color: #d97706; background: #fffbeb; padding: 10px 14px; border-radius: 8px;">{}</p>"#,
            escape_html(extra_note)
        )
    };

    let approval_type_escaped = escape_html(approval_type);
    let approval_type_lower = escape_html(&approval_type.to_lowercase());
    let employee_name = escape_html(employee_name);
    let details = details
        .iter()
        .map(|(label, value)| {
            format!(
                "<strong>{}:</strong> {}",
                escape_html(label),
                escape_html(value)
            )
        })
        .collect::<Vec<_>>()
        .join("<br>");
    let company_name = escape_html(company_name);

    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; color: #333;">
  <div style="background: #000; color: #fff; padding: 24px; border-radius: 12px 12px 0 0; text-align: center;">
    <h1 style="margin: 0; font-size: 24px;">{approval_type_escaped} Approved</h1>
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
/// Always stores the password-redacted body in `email_logs` with `max_attempts = 1`
/// so the DB outbox worker can never re-send a redacted letter; when SMTP is
/// enabled, transient retries happen solely in-memory with the credential.
#[allow(clippy::too_many_arguments)]
pub async fn send_welcome_email(
    mailer: &SharedMailer,
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
    let clean_subject = sanitize_header_text(&format!("Welcome to {} - PayrollMY", company_name));
    let clean_name = sanitize_header_text(employee_name);
    let _ = mailbox(&clean_name, login_email)
        .map_err(|_| AppError::Validation(format!("{login_email} is not a valid email address")))?;

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

    // Notice `next_retry_at = NULL` and `max_attempts = 1`: DB outbox never
    // picks up redacted welcome logs; retries run in-memory below.
    if !config.smtp_enabled() || !mailer.is_enabled() {
        return email_logs::insert_with_delivery_state(
            pool,
            company_id,
            Some(employee_id),
            None,
            "welcome",
            login_email,
            &clean_name,
            &clean_subject,
            &stored_body_html,
            "failed",
            Some("SMTP not configured"),
            0,
            1,
            None,
            created_by,
        )
        .await;
    }

    let log = email_logs::insert_with_delivery_state(
        pool,
        company_id,
        Some(employee_id),
        None,
        "welcome",
        login_email,
        &clean_name,
        &clean_subject,
        &stored_body_html,
        "pending",
        None,
        0,
        1,
        None,
        created_by,
    )
    .await?;

    let mailer = mailer.clone();
    let pool = pool.clone();
    let log_id = log.id;
    let to_email = login_email.to_string();
    let subject = clean_subject;
    tokio::spawn(async move {
        const BACKOFF_SECS: [u64; 3] = [0, 5, 20];
        let mut last_err = String::new();
        for delay in BACKOFF_SECS {
            if delay > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            }
            match mailer
                .send_raw(&clean_name, &to_email, &subject, &body_html)
                .await
            {
                Ok(()) => {
                    let _ = email_logs::mark_sent(&pool, log_id).await;
                    tracing::info!(
                        email_id = %log_id,
                        to = %crate::core::redact::email(&to_email),
                        "Welcome email sent"
                    );
                    return;
                }
                Err(err) => {
                    last_err = err;
                }
            }
        }
        let safe_err = redact_smtp_error(&last_err);
        let _ = email_logs::mark_failed(&pool, log_id, safe_err).await;
        tracing::error!(
            email_id = %log_id,
            error = %last_err,
            "Failed to send welcome email after retries"
        );
    });

    Ok(log)
}

fn welcome_html(
    employee_name: &str,
    company_name: &str,
    frontend_url: &str,
    login_email: &str,
    password_cell: &str,
) -> String {
    let employee_name = escape_html(employee_name);
    let company_name = escape_html(company_name);
    let frontend_url = escape_html(frontend_url.trim_end_matches('/'));
    let login_email = escape_html(login_email);
    let password_cell = escape_html(password_cell);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_email_html_strips_scripts_handlers_and_javascript_urls() {
        let dirty = concat!(
            r#"<p><strong>Hello</strong> <a href="https://payrollmy.com">Portal</a>"#,
            r#" <a href="mailto:hr@example.com">HR</a></p>"#,
            r#"<script>alert(1)</script>"#,
            r#"<img src="https://example.com/a.png" onerror="alert(1)">"#,
            r#"<a href="javascript:alert(1)">bad-link</a>"#,
            r#"<iframe src="https://evil.example/frame"></iframe>"#,
        );
        let cleaned = sanitize_email_html(dirty);

        assert!(!cleaned.contains("<script"));
        assert!(!cleaned.contains("alert(1)"));
        assert!(!cleaned.contains("onerror"));
        assert!(!cleaned.to_ascii_lowercase().contains("javascript:"));
        assert!(!cleaned.contains("<iframe"));

        assert!(cleaned.contains("<strong>Hello</strong>"));
        assert!(cleaned.contains("https://payrollmy.com"));
        assert!(cleaned.contains("mailto:hr@example.com"));
    }

    #[test]
    fn substitute_variables_is_single_pass() {
        let rendered = substitute_variables(
            "Hello {{employee_name}} at {{company_name}}",
            "{{company_name}}",
            "EMP-001",
            "ali@example.com",
            "Engineer",
            "Engineering",
            "2026-01-01",
            "Acme Sdn Bhd",
        );
        assert_eq!(rendered, "Hello {{company_name}} at Acme Sdn Bhd");
    }

    #[test]
    fn redact_smtp_error_hides_upstream_details() {
        assert_eq!(
            redact_smtp_error("SMTP not configured"),
            "SMTP not configured"
        );
        assert_eq!(
            redact_smtp_error("Invalid recipient address: bad@example"),
            "Invalid recipient email address"
        );
        assert_eq!(
            redact_smtp_error("535 5.7.8 auth failed user=admin@corp.internal host=10.0.0.5"),
            "SMTP delivery failed"
        );
    }
}
