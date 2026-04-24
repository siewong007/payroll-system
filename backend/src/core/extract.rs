//! Custom Axum extractors.
//!
//! `ValidatedJson<T>` deserializes the request body like `Json<T>` but also
//! runs `validator::Validate` on the result. Field-level violations surface
//! as a 422 `AppError::Validation`; malformed JSON surfaces as 400.

use axum::extract::FromRequest;
use axum::extract::rejection::JsonRejection;
use axum::{Json, extract::Request};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::core::error::AppError;

#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|rej: JsonRejection| AppError::BadRequest(rej.to_string()))?;

        value
            .validate()
            .map_err(|errors| AppError::Validation(format_errors(&errors)))?;

        Ok(ValidatedJson(value))
    }
}

/// Flatten `validator::ValidationErrors` into a single string like
/// `email: invalid; password: length must be ≥ 8`. Fine for internal tools
/// where the caller is also an engineer reading logs; a richer per-field
/// JSON response can replace this when a real frontend needs it.
fn format_errors(errors: &validator::ValidationErrors) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (field, field_errors) in errors.field_errors() {
        for err in field_errors {
            let detail = err
                .message
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_else(|| err.code.to_string());
            parts.push(format!("{field}: {detail}"));
        }
    }
    if parts.is_empty() {
        "validation failed".to_string()
    } else {
        parts.join("; ")
    }
}
