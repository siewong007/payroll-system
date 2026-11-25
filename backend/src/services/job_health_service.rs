use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::core::error::AppResult;
use crate::repositories::platform_settings;

pub const JOB_RETENTION_CLEANUP: &str = "job_health_retention_cleanup";
pub const JOB_AUTO_ABSENT: &str = "job_health_auto_absent";
pub const JOB_EMAIL_OUTBOX: &str = "job_health_email_outbox";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobHeartbeatRecord {
    last_run_at: DateTime<Utc>,
    ok: bool,
    last_error: Option<String>,
    processed: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicJobHealth {
    pub last_run_at: Option<DateTime<Utc>>,
    pub ok: Option<bool>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminJobHealth {
    pub last_run_at: Option<DateTime<Utc>>,
    pub ok: Option<bool>,
    pub stale: bool,
    pub last_error: Option<String>,
    pub processed: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicJobsStatus {
    pub retention_cleanup: PublicJobHealth,
    pub auto_absent: PublicJobHealth,
    pub email_outbox: PublicJobHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminJobsStatus {
    pub retention_cleanup: AdminJobHealth,
    pub auto_absent: AdminJobHealth,
    pub email_outbox: AdminJobHealth,
}

impl PublicJobsStatus {
    pub fn any_unhealthy(&self) -> bool {
        [
            &self.retention_cleanup,
            &self.auto_absent,
            &self.email_outbox,
        ]
        .into_iter()
        .any(|j| j.stale || j.ok == Some(false))
    }
}

/// Record a background job tick outcome in `platform_settings`.
pub async fn record_job_run(
    pool: &PgPool,
    job_key: &str,
    ok: bool,
    processed: u64,
    last_error: Option<&str>,
) {
    let sanitized_error = last_error.map(|e| {
        e.replace(['\r', '\n'], " ")
            .chars()
            .take(500)
            .collect::<String>()
    });
    let payload = JobHeartbeatRecord {
        last_run_at: Utc::now(),
        ok,
        last_error: sanitized_error,
        processed,
    };
    let Ok(serialized) = serde_json::to_string(&payload) else {
        return;
    };
    if let Err(err) = platform_settings::set_value(pool, job_key, &serialized, None).await {
        tracing::warn!(job_key, error = %err, "Failed to persist background job heartbeat");
    }
}

fn eval_admin_health(
    raw: Option<&str>,
    stale_after: Duration,
    now: DateTime<Utc>,
) -> AdminJobHealth {
    let Some(raw) = raw else {
        return AdminJobHealth {
            last_run_at: None,
            ok: None,
            stale: false,
            last_error: None,
            processed: None,
        };
    };

    match serde_json::from_str::<JobHeartbeatRecord>(raw) {
        Ok(rec) => AdminJobHealth {
            last_run_at: Some(rec.last_run_at),
            ok: Some(rec.ok),
            stale: now.signed_duration_since(rec.last_run_at) > stale_after,
            last_error: rec.last_error,
            processed: Some(rec.processed),
        },
        Err(_) => AdminJobHealth {
            last_run_at: None,
            ok: Some(false),
            stale: true,
            last_error: Some("Corrupt job health heartbeat record".into()),
            processed: None,
        },
    }
}

fn to_public(admin: AdminJobHealth) -> PublicJobHealth {
    PublicJobHealth {
        last_run_at: admin.last_run_at,
        ok: admin.ok,
        stale: admin.stale,
    }
}

pub async fn read_admin_job_health(pool: &PgPool) -> AppResult<AdminJobsStatus> {
    let pairs = platform_settings::get_values_for_keys(
        pool,
        &[JOB_RETENTION_CLEANUP, JOB_AUTO_ABSENT, JOB_EMAIL_OUTBOX],
    )
    .await?;
    let find = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    let now = Utc::now();

    Ok(AdminJobsStatus {
        retention_cleanup: eval_admin_health(find(JOB_RETENTION_CLEANUP), Duration::hours(36), now),
        auto_absent: eval_admin_health(find(JOB_AUTO_ABSENT), Duration::hours(36), now),
        email_outbox: eval_admin_health(find(JOB_EMAIL_OUTBOX), Duration::minutes(5), now),
    })
}

pub async fn read_public_job_health(pool: &PgPool) -> AppResult<PublicJobsStatus> {
    let admin = read_admin_job_health(pool).await?;
    Ok(PublicJobsStatus {
        retention_cleanup: to_public(admin.retention_cleanup),
        auto_absent: to_public(admin.auto_absent),
        email_outbox: to_public(admin.email_outbox),
    })
}
