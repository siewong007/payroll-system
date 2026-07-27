use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal error: {0}")]
    Internal(String),

    /// An upstream dependency (e.g. Google's OAuth2 endpoints) was unreachable or
    /// failed. Distinct from `Internal` so a third party's outage is not reported
    /// as a bug in this service — and so the message reaches the caller.
    #[error("Upstream error: {0}")]
    BadGateway(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    /// The request body crossed the ceiling declared for the route. Distinct
    /// from `BadRequest` because the two are fixed differently: a 400 says the
    /// upload was malformed, a 413 says it was fine but too big, and only the
    /// second one is answered by splitting the file or raising the limit.
    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),
}

impl AppError {
    /// The HTTP status and the message that is safe to show the caller.
    ///
    /// Logging of the underlying detail happens here, and nothing that names
    /// internals (a Postgres message, a client secret rejection) is ever part of
    /// the returned string. Callers that render an error themselves — such as the
    /// OAuth2 redirect handler, which puts this text in a URL fragment — can rely
    /// on that and cannot leak detail by accident.
    pub fn client_response(&self) -> (StatusCode, String) {
        match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Validation(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            AppError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            AppError::PayloadTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, msg.clone()),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Database(err) => match classify_db_error(err) {
                // A constraint the caller can act on. Still logged (with the
                // SQLSTATE and constraint name, so it stays greppable), but at
                // warn — it is a rejected request, not a server fault.
                Some((status, message)) => {
                    let db_err = err.as_database_error();
                    tracing::warn!(
                        sqlstate = ?db_err.and_then(|e| e.code()),
                        constraint = ?db_err.and_then(|e| e.constraint()),
                        "Rejected by a database constraint: {}",
                        err
                    );
                    (status, message)
                }
                None => {
                    tracing::error!("Database error: {}", err);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal server error".to_string(),
                    )
                }
            },
        }
    }
}

/// Human text for the constraints a caller can actually do something about.
///
/// Only constraints whose violation is a *user* mistake belong here. Anything
/// absent falls through to the SQLSTATE default in [`classify_db_error`].
fn known_constraint(constraint: &str) -> Option<(StatusCode, &'static str)> {
    let hit = match constraint {
        "employees_company_employee_number_active" => (
            StatusCode::CONFLICT,
            "An active employee already uses that employee number.",
        ),
        "attendance_one_open_per_employee" => (
            StatusCode::CONFLICT,
            "This employee already has an open attendance session. Close it before starting another.",
        ),
        "payroll_runs_one_active_period"
        | "payroll_runs_company_id_payroll_group_id_period_year_period_key" => (
            StatusCode::CONFLICT,
            "A payroll run already exists for this group and period.",
        ),
        "document_categories_company_id_name_key" => (
            StatusCode::CONFLICT,
            "A document category with that name already exists.",
        ),
        "company_settings_company_id_category_key_key" => {
            (StatusCode::CONFLICT, "That setting already exists.")
        }
        "attendance_records_checkout_order_check" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Check-out cannot be earlier than check-in.",
        ),
        "attendance_records_hours_check" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Hours worked is outside the permitted range.",
        ),
        _ => return None,
    };
    Some(hit)
}

/// Classify a database failure the caller can fix into a 4xx.
///
/// `AppError::Database` used to collapse every `sqlx::Error` into a bare 500, so
/// a duplicate employee number or a stale foreign key — both things the caller
/// can correct — arrived as `{"error":"Internal server error"}`. Services that
/// cared worked around it by hand-matching SQLSTATEs at the call site
/// (`user_service::map_user_write_error`, `payroll_engine`, `team_members`,
/// `user_groups`, `attendance_service`). Those stay: they can name the specific
/// record and give better text than anything reachable from here. This is the
/// floor beneath them, so a path nobody remembered to wrap degrades to an
/// accurate status instead of a 500.
///
/// The raw Postgres message is deliberately never forwarded — it carries table,
/// column and index names.
fn classify_db_error(err: &sqlx::Error) -> Option<(StatusCode, String)> {
    let db_err = err.as_database_error()?;

    if let Some(constraint) = db_err.constraint()
        && let Some((status, message)) = known_constraint(constraint)
    {
        return Some((status, message.to_string()));
    }

    let (status, message) = match db_err.code()?.as_ref() {
        "23505" => (StatusCode::CONFLICT, "That record already exists."),
        "23503" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "A referenced record does not exist, or is still in use elsewhere.",
        ),
        "23502" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "A required field was missing.",
        ),
        "23514" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "A value failed a validation rule.",
        ),
        "22001" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "A value was too long for the field it was written to.",
        ),
        "22003" | "22P02" | "22007" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "A value was not in the expected format or range.",
        ),
        // Serialization failure / deadlock: nothing is wrong with the request,
        // it just lost a race. Retrying it is the correct client behaviour.
        "40001" | "40P01" => (
            StatusCode::CONFLICT,
            "This record was changed concurrently. Please retry.",
        ),
        _ => return None,
    };
    Some((status, message.to_string()))
}

/// A 413 that names the ceiling instead of describing the symptom.
///
/// Every upload route declares its limit as a constant next to the handler and
/// attaches the matching `DefaultBodyLimit` in `routes/mod.rs`; this renders the
/// same number into the message so the operator does not have to guess whether
/// the file, the envelope or the parser was the problem.
pub fn payload_too_large(
    // what was being uploaded, in the words the caller used ("Backup file", …)
    what: &str,
    // the ceiling that was crossed, in bytes; always a whole number of MiB
    limit_bytes: usize,
) -> AppError {
    AppError::PayloadTooLarge(format!(
        "{what} is too large. The maximum is {} MB.",
        limit_bytes / (1024 * 1024)
    ))
}

/// Classify a multipart read failure by what actually went wrong.
///
/// The body limit rejects an oversized upload mid-stream, so it surfaces from
/// whichever multipart call happened to be reading — `next_field` or `bytes` —
/// as a parser error. Reporting that verbatim gave a 400 whose text pointed at
/// the multipart decoder while the real cause was the size, which is what made
/// a backup this system produced un-restorable *and* undiagnosable.
/// `MultipartError::status` already distinguishes the two; this lifts that
/// distinction into `AppError`.
pub fn multipart_error(
    err: &axum::extract::multipart::MultipartError,
    // which part was being read, for the malformed branch ("the file data")
    what: &str,
    // the route's request ceiling, named in the over-size branch
    limit_bytes: usize,
) -> AppError {
    if err.status() == StatusCode::PAYLOAD_TOO_LARGE {
        payload_too_large("The upload", limit_bytes)
    } else {
        AppError::BadRequest(format!("Could not read {what}: {err}"))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = self.client_response();

        let body = json!({
            "error": message,
            "status": status.as_u16(),
        });

        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
