use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A network an administrator has approved as "the office".
///
/// `network` is the canonical address with host bits cleared and `prefix_len`
/// its width; together they are the CIDR block. They are kept apart rather than
/// stored as one string so a malformed value cannot be written by anything that
/// bypasses the parser, and so the width can be constrained in SQL.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyNetwork {
    pub id: Uuid,
    pub company_id: Uuid,
    pub label: String,
    pub network: String,
    pub prefix_len: i16,
    pub is_active: bool,
    pub approved_by: Option<Uuid>,
    pub approved_at: DateTime<Utc>,
    /// True when this entry started life as a learned proposal rather than a
    /// typed-in block. Worth surfacing when auditing why a network is trusted.
    pub learned_from_observation: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNetworkRequest {
    pub label: String,
    /// `203.0.113.0/24`, or a bare address for a single host.
    pub cidr: String,
}

/// Only the label and the active flag are editable.
///
/// Changing the *block* of an existing entry is deliberately not offered:
/// it would silently redefine what "the office" means while keeping the row's
/// approval provenance and audit history, which read as though the original
/// block were still the one in force. Delete and re-approve instead.
#[derive(Debug, Deserialize)]
pub struct UpdateNetworkRequest {
    pub label: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SetNetworkModeRequest {
    /// "none", "learn", "warn" or "enforce"
    pub mode: String,
}

/// Approve a learned candidate. The block is echoed back by the caller so an
/// administrator cannot approve a different network from the one they were
/// shown by a stale page.
#[derive(Debug, Deserialize)]
pub struct ApproveCandidateRequest {
    pub cidr: String,
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct DismissCandidateRequest {
    pub cidr: String,
}

/// A network the system has observed but nobody has approved.
///
/// Aggregated across employees on purpose: the per-employee rows behind this
/// are home and mobile addresses, and no endpoint exposes which employee was
/// seen where.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct NetworkCandidate {
    pub network: String,
    pub prefix_len: i16,
    /// How many different people have checked in from here. The number that
    /// distinguishes an office from somebody's flat.
    pub distinct_employees: i64,
    pub observation_count: i64,
    /// Observations corroborated by a signal the employee does not control —
    /// inside the geofence, or a QR token minted by a kiosk in the building.
    pub anchored_count: i64,
    /// Check-ins *refused* from this block. Never evidence for approving it;
    /// this is how the office's new address surfaces after an ISP change, when
    /// everyone is being turned away and nothing else is being recorded.
    pub denied_count: i64,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

/// A candidate plus the verdict on whether it is worth proposing.
#[derive(Debug, Serialize)]
pub struct ScoredCandidate {
    #[serde(flatten)]
    pub candidate: NetworkCandidate,
    /// Corroborated by the geofence or a kiosk-minted token often enough to
    /// clear the anchored threshold. These are the ones safe to suggest.
    pub is_anchored: bool,
    /// Clears the proposal thresholds and can be offered to an administrator.
    /// Never a licence to approve automatically — a human still decides.
    pub is_proposable: bool,
    /// Why it does not yet qualify, for the admin screen. `None` when it does.
    pub blocked_reason: Option<String>,
}

/// The outcome of checking a client address against the allow-list.
#[derive(Debug, Serialize)]
pub struct NetworkCheckResult {
    pub is_approved: bool,
    /// The label of the network that matched, for the confirmation message.
    pub matched_label: Option<String>,
    /// False when the company has approved no networks at all. Enforcement is
    /// skipped in that state — see `attendance_network_service`.
    pub has_approved_networks: bool,
}
