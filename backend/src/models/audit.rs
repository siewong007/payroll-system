use std::net::IpAddr;

use axum::http::{HeaderMap, header};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::client_ip::client_ip;

#[derive(Debug, Clone, Default)]
pub struct AuditRequestMeta {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl AuditRequestMeta {
    /// Builds the metadata recorded alongside an audited change.
    ///
    /// `peer` is the TCP peer address; `trust_proxy_headers` mirrors the
    /// operator's `TRUST_PROXY_HEADERS` declaration. Both are required because
    /// the address this records is evidence: reading `X-Forwarded-For`
    /// unconditionally (as this did) meant any caller could write whatever IP
    /// they liked into the audit trail by setting one header. Resolution is
    /// delegated to `core::client_ip` so the trail and the rate limiter always
    /// agree on who the client is.
    pub fn from_request(
        headers: &HeaderMap,
        peer: Option<IpAddr>,
        trust_proxy_headers: bool,
    ) -> Self {
        // Truncated to the column width (audit_logs.ip_address is
        // varchar(45)) exactly as user_agent is below: an over-long value
        // would fail the INSERT — fatal wherever an audit row shares a
        // transaction with the change it describes. An `IpAddr` never exceeds
        // 45 characters, but the user agent is arbitrary client text.
        let ip_address = client_ip(headers, peer, trust_proxy_headers).map(|ip| ip.to_string());

        let user_agent = headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(500).collect());

        Self {
            ip_address,
            user_agent,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub entity_type: Option<String>,
    /// Scope to one record's history — "who changed this, and why".
    pub entity_id: Option<Uuid>,
    pub action: Option<String>,
    pub user_id: Option<Uuid>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditLogWithUser {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub old_values: Option<serde_json::Value>,
    pub new_values: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub user_email: Option<String>,
    pub user_full_name: Option<String>,
}
