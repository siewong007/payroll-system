use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub company_id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct NotificationCount {
    pub unread: i64,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
pub struct NotificationQuery {
    pub unread_only: Option<bool>,
    pub limit: Option<i64>,
}

/// Rows returned when the caller names no limit — what the frontend asks for.
pub const DEFAULT_NOTIFICATION_LIMIT: i64 = 50;

/// Ceiling on `?limit`.
///
/// `notifications` is never purged, so an unbounded value loads a user's whole
/// history into memory on a route with no rate-limit group of its own; a
/// negative one reached Postgres as `LIMIT -1` (SQLSTATE 2201W), which nothing
/// in `classify_db_error` maps, so it surfaced as a 500 rather than a 400.
pub const MAX_NOTIFICATION_LIMIT: i64 = 100;

impl NotificationQuery {
    /// The row cap to hand to SQL, always inside `1..=MAX_NOTIFICATION_LIMIT`.
    ///
    /// Clamping rather than rejecting matches what the sibling list endpoints
    /// already do (`handlers::admin`'s `.clamp(1, 100)`, `handlers::email`'s
    /// `.min(100)`), and it lives on the query struct so a second caller cannot
    /// forget it.
    pub fn effective_limit(&self) -> i64 {
        self.limit
            .unwrap_or(DEFAULT_NOTIFICATION_LIMIT)
            .clamp(1, MAX_NOTIFICATION_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(limit: Option<i64>) -> NotificationQuery {
        NotificationQuery {
            unread_only: None,
            limit,
        }
    }

    #[test]
    fn absent_limit_falls_back_to_the_default() {
        assert_eq!(query(None).effective_limit(), DEFAULT_NOTIFICATION_LIMIT);
    }

    #[test]
    fn a_limit_inside_the_range_is_used_verbatim() {
        assert_eq!(query(Some(1)).effective_limit(), 1);
        assert_eq!(query(Some(25)).effective_limit(), 25);
        assert_eq!(
            query(Some(MAX_NOTIFICATION_LIMIT)).effective_limit(),
            MAX_NOTIFICATION_LIMIT
        );
    }

    #[test]
    fn a_non_positive_limit_never_reaches_postgres() {
        // `?limit=-1` is the reported 500: Postgres rejects a negative LIMIT
        // with 2201W, which classifies as an unmapped database error.
        assert_eq!(query(Some(-1)).effective_limit(), 1);
        assert_eq!(query(Some(0)).effective_limit(), 1);
        assert_eq!(query(Some(i64::MIN)).effective_limit(), 1);
    }

    #[test]
    fn an_enormous_limit_is_capped() {
        assert_eq!(
            query(Some(100_000_000)).effective_limit(),
            MAX_NOTIFICATION_LIMIT
        );
        assert_eq!(
            query(Some(i64::MAX)).effective_limit(),
            MAX_NOTIFICATION_LIMIT
        );
    }
}
