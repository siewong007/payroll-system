//! Binding attendance to the company's network, and learning which network
//! that is.
//!
//! ## What is trusted
//!
//! Exactly one thing: the client address the *server* resolved, via
//! [`crate::core::client_ip`] — the right-most `X-Forwarded-For` entry when the
//! operator has declared a trusted proxy, the TCP peer otherwise. Nothing the
//! client says about its network is accepted, because nothing it could say is
//! checkable: browsers expose no API for reading a WiFi SSID or BSSID, so any
//! such value would be a string the employee typed or a header they set.
//!
//! The friendly name — "HQ WiFi" — is a label an administrator attaches to a
//! prefix they approved. It describes a verified thing rather than asserting an
//! unverifiable one.
//!
//! ## Modes
//!
//! `none` → off. `learn` → observe only, never block or flag. `warn` → check,
//! allow, flag. `enforce` → check, refuse.
//!
//! `learn` exists because a network allow-list, unlike a geofence, cannot be
//! configured from a map before it is switched on. Someone has to discover what
//! the office egress address actually is, and the honest way to discover it is
//! to watch.
//!
//! ## Why learning cannot promote itself
//!
//! An observation only counts toward a proposal when it is *anchored* — the
//! same check-in was independently corroborated by the geofence or by a QR
//! token minted from a kiosk credential, which means a device physically in the
//! building displayed the code. Unanchored observations are still surfaced, but
//! flagged as uncorroborated and held to a higher bar, and in no case is
//! anything approved without an administrator's explicit action. Otherwise the
//! first employee to check in from home during rollout teaches the system that
//! their living room is the office.

use std::net::IpAddr;

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};
use crate::core::ip_prefix::{IpPrefix, is_identifying};
use crate::models::company_network::{
    CompanyNetwork, CreateNetworkRequest, NetworkCheckResult, ScoredCandidate, UpdateNetworkRequest,
};
use crate::repositories::reads::attendance_networks as network_reads;
use crate::repositories::{attendance_network_observations, companies, company_networks};
use crate::services::audit_service::{self, AuditRequestMeta};

// ─── Learning thresholds ───

/// Distinct people who must have been seen on a block before it may be
/// proposed, when their check-ins were corroborated by the geofence or a kiosk.
const ANCHORED_MIN_EMPLOYEES: i64 = 2;

/// Anchored observations required alongside that. Two people once each is a
/// coincidence; the count is what makes it a place of work.
const ANCHORED_MIN_OBSERVATIONS: i64 = 4;

/// Distinct people required when *nothing* corroborates the observations —
/// the company runs no geofence and no kiosks, so the only evidence is that a
/// lot of employees keep appearing from the same address. Deliberately much
/// higher: this is the weakest signal the system will act on, and even here it
/// only produces a suggestion for a human to judge.
const UNANCHORED_MIN_EMPLOYEES: i64 = 5;

/// Total observations required for an uncorroborated proposal.
const UNANCHORED_MIN_OBSERVATIONS: i64 = 20;

/// How long an observation is kept. Long enough to accumulate a proposal across
/// a couple of working weeks; short enough that the table is not a standing
/// archive of where employees live.
pub const OBSERVATION_RETENTION_DAYS: i32 = 30;

/// Candidate width for IPv4: the exact address.
const CANDIDATE_V4_PREFIX: u8 = 32;

/// Candidate width for IPv6: the /64 the client rotates its address within.
///
/// Recording the full /128 would produce a fresh candidate every time privacy
/// extensions cycle, and nothing would ever reach a threshold.
const CANDIDATE_V6_PREFIX: u8 = 64;

// ─── Mode ───

pub async fn get_mode(pool: &PgPool, company_id: Uuid) -> AppResult<String> {
    let mode = companies::get_attendance_network_mode(pool, company_id).await?;
    Ok(mode.unwrap_or_else(|| "none".to_string()))
}

pub async fn set_mode(
    pool: &PgPool,
    company_id: Uuid,
    mode: &str,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    if !matches!(mode, "none" | "learn" | "warn" | "enforce") {
        return Err(AppError::BadRequest(
            "Network mode must be 'none', 'learn', 'warn' or 'enforce'".into(),
        ));
    }

    // Turning on enforcement with an empty allow-list is the single most
    // effective way to lock a whole company out of attendance. The check-in
    // path also refuses to enforce against an empty list, so this is belt and
    // braces — but failing here gives the administrator the actionable error at
    // the moment they made the decision, rather than a silently inert setting.
    if mode == "enforce" {
        let approved = company_networks::list_active(pool, company_id).await?;
        if approved.is_empty() {
            return Err(AppError::BadRequest(
                "Approve at least one office network before switching to Enforce, \
                 or nobody will be able to check in."
                    .into(),
            ));
        }
    }

    let old_mode = get_mode(pool, company_id).await?;
    companies::set_attendance_network_mode(pool, company_id, mode).await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "attendance_network_mode",
        Some(company_id),
        Some(serde_json::json!({ "mode": old_mode })),
        Some(serde_json::json!({ "mode": mode })),
        Some("Attendance network mode updated"),
        audit_meta,
    )
    .await;

    Ok(())
}

// ─── Allow-list CRUD ───

pub async fn list_networks(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<CompanyNetwork>> {
    company_networks::list_for_company(pool, company_id).await
}

pub async fn create_network(
    pool: &PgPool,
    company_id: Uuid,
    req: &CreateNetworkRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<CompanyNetwork> {
    let label = validate_label(&req.label)?;
    // `parse_approvable` is the policy gate: it refuses private, loopback,
    // link-local and carrier-NAT addresses, and blocks wider than the family's
    // floor. Every path that writes to the allow-list goes through it.
    let prefix = IpPrefix::parse_approvable(&req.cidr)?;

    insert_approved(
        pool, company_id, &label, prefix, actor_id, false, audit_meta,
    )
    .await
}

pub async fn update_network(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    req: &UpdateNetworkRequest,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<CompanyNetwork> {
    let existing = company_networks::get(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Network not found".into()))?;

    let label = match req.label.as_deref() {
        Some(value) => validate_label(value)?,
        None => existing.label.clone(),
    };
    let is_active = req.is_active.unwrap_or(existing.is_active);

    // Deactivating the last active network while enforcing would lock everyone
    // out just as surely as enabling enforce with an empty list.
    if existing.is_active && !is_active {
        ensure_not_last_active_while_enforcing(pool, company_id, id).await?;
    }

    let updated = company_networks::update(pool, id, company_id, &label, is_active).await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "update",
        "company_network",
        Some(updated.id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        Some(serde_json::to_value(&updated).unwrap_or_default()),
        Some("Attendance network updated"),
        audit_meta,
    )
    .await;

    Ok(updated)
}

pub async fn delete_network(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let existing = company_networks::get(pool, id, company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Network not found".into()))?;

    if existing.is_active {
        ensure_not_last_active_while_enforcing(pool, company_id, id).await?;
    }

    let removed = company_networks::delete(pool, id, company_id).await?;
    if removed == 0 {
        return Err(AppError::NotFound("Network not found".into()));
    }

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "delete",
        "company_network",
        Some(id),
        Some(serde_json::to_value(&existing).unwrap_or_default()),
        None,
        Some("Attendance network removed"),
        audit_meta,
    )
    .await;

    Ok(())
}

/// Refuse to remove the last thing standing between the company and a
/// company-wide check-in outage.
async fn ensure_not_last_active_while_enforcing(
    pool: &PgPool,
    company_id: Uuid,
    excluding: Uuid,
) -> AppResult<()> {
    if get_mode(pool, company_id).await? != "enforce" {
        return Ok(());
    }
    let remaining = company_networks::list_active(pool, company_id)
        .await?
        .into_iter()
        .filter(|n| n.id != excluding)
        .count();
    if remaining == 0 {
        return Err(AppError::BadRequest(
            "This is the only approved network and the mode is Enforce. \
             Switch the mode to Warn first, or nobody will be able to check in."
                .into(),
        ));
    }
    Ok(())
}

async fn insert_approved(
    pool: &PgPool,
    company_id: Uuid,
    label: &str,
    prefix: IpPrefix,
    actor_id: Uuid,
    learned: bool,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<CompanyNetwork> {
    let network = prefix.network().to_string();
    let prefix_len = prefix.prefix_len() as i16;

    if company_networks::exists(pool, company_id, &network, prefix_len).await? {
        return Err(AppError::Conflict(format!(
            "{prefix} is already on the approved list."
        )));
    }

    let created = company_networks::insert(
        pool, company_id, label, &network, prefix_len, actor_id, learned,
    )
    .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "create",
        "company_network",
        Some(created.id),
        None,
        Some(serde_json::to_value(&created).unwrap_or_default()),
        Some(if learned {
            "Learned attendance network approved"
        } else {
            "Attendance network added"
        }),
        audit_meta,
    )
    .await;

    Ok(created)
}

fn validate_label(label: &str) -> AppResult<String> {
    let label = label.trim();
    if label.is_empty() || label.chars().count() > 150 {
        return Err(AppError::BadRequest(
            "Label must be 1–150 characters".into(),
        ));
    }
    Ok(label.to_string())
}

// ─── Candidates ───

pub async fn list_candidates(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<ScoredCandidate>> {
    let candidates = network_reads::list_candidates(pool, company_id).await?;
    Ok(candidates.into_iter().map(score_candidate).collect())
}

/// Approve a learned candidate.
///
/// The block is re-validated against the same policy as a typed-in one — being
/// observed confers no exemption from the carrier-NAT and prefix-width rules —
/// and the evidence is discarded once it has served its purpose.
pub async fn approve_candidate(
    pool: &PgPool,
    company_id: Uuid,
    cidr: &str,
    label: &str,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<CompanyNetwork> {
    let label = validate_label(label)?;
    let prefix = IpPrefix::parse_approvable(cidr)?;
    let network = prefix.network().to_string();
    let prefix_len = prefix.prefix_len() as i16;

    // Approving something never observed is not an error worth blocking — an
    // administrator may legitimately pre-approve a block they know about — but
    // it must not be recorded as *learned*, because nothing was learned.
    let observed = network_reads::get_candidate(pool, company_id, &network, prefix_len)
        .await?
        .is_some();

    let created = insert_approved(
        pool, company_id, &label, prefix, actor_id, observed, audit_meta,
    )
    .await?;

    attendance_network_observations::delete_for_network(pool, company_id, &network, prefix_len)
        .await?;

    Ok(created)
}

pub async fn dismiss_candidate(
    pool: &PgPool,
    company_id: Uuid,
    cidr: &str,
    actor_id: Uuid,
    audit_meta: Option<&AuditRequestMeta>,
) -> AppResult<()> {
    let prefix = IpPrefix::parse(cidr)?;
    let network = prefix.network().to_string();
    let prefix_len = prefix.prefix_len() as i16;

    attendance_network_observations::dismiss(pool, company_id, &network, prefix_len, actor_id)
        .await?;
    attendance_network_observations::delete_for_network(pool, company_id, &network, prefix_len)
        .await?;

    let _ = audit_service::log_action_with_metadata(
        pool,
        Some(company_id),
        Some(actor_id),
        "dismiss",
        "attendance_network_candidate",
        None,
        None,
        Some(serde_json::json!({ "network": prefix.to_string() })),
        Some("Attendance network candidate dismissed"),
        audit_meta,
    )
    .await;

    Ok(())
}

// ─── Check-in / check-out evaluation ───

/// Evaluate a client address against the allow-list and decide what the
/// check-in path should do.
///
/// Returns `Ok(Some(true))` when the record should be flagged as off-network,
/// `Ok(Some(false))` when it matched, and `Ok(None)` when the network was not
/// evaluated at all (mode `none` or `learn`) — which is stored as NULL so a
/// report can tell "on-network" from "never checked".
///
/// Errors only in `enforce`, and only to refuse the check-in.
pub async fn validate_for_checkin(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    client_ip: Option<IpAddr>,
) -> AppResult<Option<bool>> {
    let mode = get_mode(pool, company_id).await?;
    if mode == "none" || mode == "learn" {
        return Ok(None);
    }

    let result = check_network(pool, company_id, client_ip).await?;

    // Nothing approved yet: there is no allow-list to be outside of. Enforcing
    // against an empty list would block every check-in in the company, so the
    // check is inert until an administrator approves something.
    if !result.has_approved_networks {
        return Ok(None);
    }

    if result.is_approved {
        return Ok(Some(false));
    }

    // An address we cannot make sense of is a deployment fault, not an
    // employee. Absent, private, loopback or carrier-NAT means the request did
    // not arrive through the proxy the deployment assumes, or TRUST_PROXY_HEADERS
    // disagrees with reality — in which case *every* client resolves the same
    // way and denying would take the whole company's attendance down.
    //
    // Failing closed here would buy nothing anyway: the container is published
    // on 127.0.0.1:8080 (deploy/docker-compose.prod.yml), so anyone able to
    // present a stripped header has already reached it directly, and could just
    // as easily present a matching one. Flag it for review instead.
    let unverifiable = match client_ip {
        None => true,
        Some(addr) => !is_identifying(addr),
    };
    if unverifiable {
        tracing::warn!(
            company_id = %company_id,
            "attendance network check could not resolve a usable client address; \
             flagging instead of denying"
        );
        return Ok(Some(true));
    }

    if mode == "enforce" {
        // Record the refusal before returning it. On the morning the office
        // address changes this is the only trace left anywhere — the success
        // path that normally feeds learning never runs.
        if let Some(addr) = client_ip
            && let Ok(prefix) = candidate_prefix(addr)
            && let Err(e) = attendance_network_observations::record_denial(
                pool,
                company_id,
                employee_id,
                &prefix.network().to_string(),
                prefix.prefix_len() as i16,
            )
            .await
        {
            tracing::warn!("Failed to record attendance network denial: {}", e);
        }

        return Err(AppError::BadRequest(
            "You're not on an approved office network. Connect to the office Wi-Fi \
             and try again — if you're already on it, ask an administrator to check \
             the approved networks."
                .into(),
        ));
    }

    Ok(Some(true))
}

/// Evaluate for a check-out, which never fails.
///
/// Same rationale as the geofence: refusing a check-out leaves an open session
/// only an administrator can close, which is a worse outcome than an employee
/// closing their shift from the car park. The verdict ORs into the record's
/// flag for review.
pub async fn flag_for_checkout(
    pool: &PgPool,
    company_id: Uuid,
    client_ip: Option<IpAddr>,
) -> AppResult<bool> {
    let mode = get_mode(pool, company_id).await?;
    if mode == "none" || mode == "learn" {
        return Ok(false);
    }
    let result = check_network(pool, company_id, client_ip).await?;
    if !result.has_approved_networks {
        return Ok(false);
    }
    Ok(!result.is_approved)
}

/// Does this address match any active approved network?
pub async fn check_network(
    pool: &PgPool,
    company_id: Uuid,
    client_ip: Option<IpAddr>,
) -> AppResult<NetworkCheckResult> {
    let approved = company_networks::list_active(pool, company_id).await?;
    let has_approved_networks = !approved.is_empty();

    // No resolvable address is treated as "not on the network", never as a
    // pass. An absent address is exactly what a stripped `X-Forwarded-For`
    // looks like, so failing open here would be the bypass.
    let Some(addr) = client_ip else {
        return Ok(NetworkCheckResult {
            is_approved: false,
            matched_label: None,
            has_approved_networks,
        });
    };

    for entry in &approved {
        // A row whose stored block no longer parses is skipped rather than
        // fatal: one corrupt row must not take attendance down for everyone.
        let Ok(prefix) = IpPrefix::new(
            match entry.network.parse() {
                Ok(addr) => addr,
                Err(_) => {
                    tracing::warn!(
                        network = %entry.network,
                        "approved attendance network is unparseable; skipping"
                    );
                    continue;
                }
            },
            entry.prefix_len as u8,
        ) else {
            continue;
        };

        if prefix.contains(addr) {
            return Ok(NetworkCheckResult {
                is_approved: true,
                matched_label: Some(entry.label.clone()),
                has_approved_networks,
            });
        }
    }

    Ok(NetworkCheckResult {
        is_approved: false,
        matched_label: None,
        has_approved_networks,
    })
}

/// Record what we saw, if the company is learning.
///
/// Best-effort by construction: the caller ignores the result. A failure to
/// record evidence must never fail a check-in that has otherwise succeeded.
pub async fn observe(
    pool: &PgPool,
    company_id: Uuid,
    employee_id: Uuid,
    client_ip: Option<IpAddr>,
    anchored: bool,
) -> AppResult<()> {
    let mode = get_mode(pool, company_id).await?;
    if mode == "none" {
        return Ok(());
    }

    let Some(addr) = client_ip else {
        return Ok(());
    };

    // Never learn an address that could not identify a place anyway — a
    // private or carrier-NAT candidate would only ever be refused at approval
    // time, so storing it is pure privacy cost for no benefit.
    if !is_identifying(addr) {
        return Ok(());
    }

    let prefix = candidate_prefix(addr)?;

    attendance_network_observations::record(
        pool,
        company_id,
        employee_id,
        &prefix.network().to_string(),
        prefix.prefix_len() as i16,
        anchored,
    )
    .await
}

/// The block an observation is recorded against: the exact IPv4 address, or the
/// IPv6 /64 the client's rotating addresses all sit inside.
pub(crate) fn candidate_prefix(addr: IpAddr) -> AppResult<IpPrefix> {
    let len = match crate::core::ip_prefix::canonical(addr) {
        IpAddr::V4(_) => CANDIDATE_V4_PREFIX,
        IpAddr::V6(_) => CANDIDATE_V6_PREFIX,
    };
    IpPrefix::new(addr, len)
}

/// Decide whether a candidate has enough behind it to be worth showing as a
/// suggestion, and say why not when it does not.
pub(crate) fn score_candidate(
    candidate: crate::models::company_network::NetworkCandidate,
) -> ScoredCandidate {
    let is_anchored =
        candidate.anchored_count >= ANCHORED_MIN_OBSERVATIONS && anchored_employees_met(&candidate);

    let is_proposable = is_anchored
        || (candidate.distinct_employees >= UNANCHORED_MIN_EMPLOYEES
            && candidate.observation_count >= UNANCHORED_MIN_OBSERVATIONS);

    let blocked_reason = if is_proposable {
        None
    } else if candidate.anchored_count > 0 {
        Some(format!(
            "Seen {} time(s) from {} employee(s) with {} corroborated by a kiosk or geofence — \
             needs {ANCHORED_MIN_OBSERVATIONS} corroborated from {ANCHORED_MIN_EMPLOYEES} employees.",
            candidate.observation_count, candidate.distinct_employees, candidate.anchored_count,
        ))
    } else {
        Some(format!(
            "No corroborating kiosk or geofence signal. Needs {UNANCHORED_MIN_EMPLOYEES} employees \
             and {UNANCHORED_MIN_OBSERVATIONS} check-ins before it can be suggested \
             (currently {} and {}).",
            candidate.distinct_employees, candidate.observation_count,
        ))
    };

    ScoredCandidate {
        candidate,
        is_anchored,
        is_proposable,
        blocked_reason,
    }
}

/// The anchored path also needs several *different* people. One employee with a
/// kiosk-minted token and a habit of checking in twice a day would otherwise
/// clear the observation count on their own.
fn anchored_employees_met(candidate: &crate::models::company_network::NetworkCandidate) -> bool {
    candidate.distinct_employees >= ANCHORED_MIN_EMPLOYEES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::company_network::NetworkCandidate;
    use chrono::Utc;

    fn candidate(employees: i64, observations: i64, anchored: i64) -> NetworkCandidate {
        NetworkCandidate {
            network: "203.0.113.7".into(),
            prefix_len: 32,
            distinct_employees: employees,
            observation_count: observations,
            anchored_count: anchored,
            denied_count: 0,
            first_seen_at: Utc::now(),
            last_seen_at: Utc::now(),
        }
    }

    #[test]
    fn a_corroborated_office_becomes_proposable() {
        let scored = score_candidate(candidate(3, 12, 8));
        assert!(scored.is_anchored);
        assert!(scored.is_proposable);
        assert!(scored.blocked_reason.is_none());
    }

    #[test]
    fn one_employees_home_never_becomes_proposable_however_often_they_check_in() {
        // The poisoning case: a single person checking in from their flat every
        // day for a month. Volume alone must not promote it.
        let scored = score_candidate(candidate(1, 60, 0));
        assert!(!scored.is_proposable, "one employee is not an office");
        assert!(scored.blocked_reason.is_some());
    }

    #[test]
    fn one_employee_cannot_clear_the_anchored_bar_alone() {
        // Even with a kiosk-minted token every time, one person is one person.
        let scored = score_candidate(candidate(1, 40, 40));
        assert!(!scored.is_anchored);
        assert!(!scored.is_proposable);
    }

    #[test]
    fn uncorroborated_networks_are_held_to_a_much_higher_bar() {
        // Enough people to clear the anchored threshold, but nothing
        // corroborating them — not proposable yet.
        let scored = score_candidate(candidate(3, 12, 0));
        assert!(!scored.is_proposable);

        // At the uncorroborated thresholds it becomes a suggestion — still only
        // a suggestion, since approval is always a human action.
        let scored = score_candidate(candidate(
            UNANCHORED_MIN_EMPLOYEES,
            UNANCHORED_MIN_OBSERVATIONS,
            0,
        ));
        assert!(scored.is_proposable);
        assert!(!scored.is_anchored, "still uncorroborated");
    }

    #[test]
    fn the_anchored_thresholds_are_exactly_inclusive() {
        let at = score_candidate(candidate(
            ANCHORED_MIN_EMPLOYEES,
            ANCHORED_MIN_OBSERVATIONS,
            ANCHORED_MIN_OBSERVATIONS,
        ));
        assert!(at.is_proposable, "the threshold itself must qualify");

        let below = score_candidate(candidate(
            ANCHORED_MIN_EMPLOYEES,
            ANCHORED_MIN_OBSERVATIONS,
            ANCHORED_MIN_OBSERVATIONS - 1,
        ));
        assert!(!below.is_proposable, "one short must not qualify");
    }

    #[test]
    fn ipv4_candidates_are_recorded_as_exact_addresses() {
        let prefix = candidate_prefix("203.0.113.7".parse().unwrap()).unwrap();
        assert_eq!(prefix.to_string(), "203.0.113.7/32");
    }

    #[test]
    fn ipv6_candidates_are_recorded_as_the_rotating_slash_64() {
        // Privacy extensions change the host half on a timer. Recording /128
        // would make every check-in a brand-new candidate and nothing would
        // ever accumulate.
        let a = candidate_prefix("2001:db8:1:2:aaaa:bbbb:cccc:dddd".parse().unwrap()).unwrap();
        let b = candidate_prefix("2001:db8:1:2:1111:2222:3333:4444".parse().unwrap()).unwrap();
        assert_eq!(a, b, "the same /64 must produce the same candidate");
        assert_eq!(a.to_string(), "2001:db8:1:2::/64");
    }

    #[test]
    fn a_v4_mapped_v6_client_records_as_its_ipv4_candidate() {
        let prefix = candidate_prefix("::ffff:203.0.113.7".parse().unwrap()).unwrap();
        assert_eq!(prefix.to_string(), "203.0.113.7/32");
    }

    #[test]
    fn denials_alone_never_make_a_block_proposable() {
        // The recovery case: the office ISP changed, everyone is being turned
        // away from the new address, and nothing else is being recorded. The
        // administrator must be *shown* this block — but it must not qualify on
        // denial volume, or an attacker hammering check-in from one address
        // would nominate their own network.
        let mut c = candidate(0, 0, 0);
        c.denied_count = 500;
        let scored = score_candidate(c);
        assert!(!scored.is_proposable, "denials are not corroboration");
        assert!(!scored.is_anchored);
    }

    #[test]
    fn the_blocked_reason_distinguishes_uncorroborated_from_merely_thin() {
        let thin = score_candidate(candidate(2, 3, 2));
        assert!(
            thin.blocked_reason.unwrap().contains("corroborated"),
            "a partially corroborated candidate should say how far short it is"
        );

        let none = score_candidate(candidate(2, 3, 0));
        assert!(
            none.blocked_reason.unwrap().contains("No corroborating"),
            "an uncorroborated candidate should say so plainly"
        );
    }
}
