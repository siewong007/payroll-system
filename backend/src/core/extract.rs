//! Custom Axum extractors.
//!
//! `ValidatedJson<T>` deserializes the request body like `Json<T>` but also
//! runs `validator::Validate` on the result. Field-level violations surface
//! as a 422 `AppError::Validation`; malformed JSON surfaces as 400.
//!
//! `AuditRequestMeta` resolves the caller's IP and user agent for the audit
//! trail. It is an extractor rather than a helper a handler calls on a
//! `HeaderMap` because getting it right needs two things a handler does not
//! otherwise hold — the TCP peer address and the operator's
//! `TRUST_PROXY_HEADERS` setting — and the old header-only helper silently
//! recorded a forgeable address instead.
//!
//! `ClientIp` is the same address as a parsed `IpAddr` rather than a display
//! string, for the one caller that has to *compare* it instead of record it:
//! the attendance network check. It deliberately shares
//! [`crate::core::client_ip::client_ip`] with the audit trail and the rate
//! limiter, so a deployment can never enforce attendance against one notion of
//! the client address while logging another.

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, FromRequest, FromRequestParts};
use axum::http::request::Parts;
use axum::{Json, extract::Request};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::core::app_state::AppState;
use crate::core::client_ip::client_ip;
use crate::core::error::AppError;
use crate::models::audit::AuditRequestMeta;

/// The address the server resolved for this request, if any.
///
/// `None` means the address could not be established at all — no
/// `ConnectInfo` and no trusted forwarded header. Callers that gate on the
/// network must treat that as "not on an approved network", never as a pass:
/// an unknown address is exactly what a stripped header looks like.
#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub Option<IpAddr>);

impl FromRequestParts<AppState> for ClientIp {
    /// Never fails, for the same reason `AuditRequestMeta` never fails — the
    /// absence of an address is information the handler should act on, not a
    /// reason to reject the request before it reaches one.
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip());

        Ok(ClientIp(client_ip(
            &parts.headers,
            peer,
            state.config.trust_proxy_headers,
        )))
    }
}

impl FromRequestParts<AppState> for AuditRequestMeta {
    /// Never fails: a request with no resolvable address still gets an audit
    /// row, just without an IP. Refusing the request instead would mean a
    /// missing `ConnectInfo` could block an otherwise valid payroll change.
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip());

        Ok(AuditRequestMeta::from_request(
            &parts.headers,
            peer,
            state.config.trust_proxy_headers,
        ))
    }
}

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
