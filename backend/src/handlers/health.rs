use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};

use crate::core::app_state::AppState;
use crate::core::config::{BUILD_REVISION, BUILD_VERSION};
use crate::services::job_health_service;

/// Deep health check for orchestration probes, host watchdog timers, and deploy
/// verification. Verifies Postgres connectivity + applied migrations, surfaces
/// the running `BUILD_REVISION`, and reports sanitized background job staleness.
///
/// Returns 200 (`status: "ready"` or `"degraded"` when a background job is stale/errored)
/// if Postgres & migrations are up, or 503 (`status: "unready"`) if Postgres is down
/// or unmigrated.
pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let row = crate::repositories::reads::system::get_migration_status(&state.pool).await;

    match row {
        Ok((Some(_latest), _count)) => {
            let jobs = job_health_service::read_public_job_health(&state.pool)
                .await
                .ok();
            let status = if jobs.as_ref().is_some_and(|j| j.any_unhealthy()) {
                "degraded"
            } else {
                "ready"
            };
            (
                StatusCode::OK,
                Json(json!({
                    "status": status,
                    "version": BUILD_VERSION,
                    "revision": BUILD_REVISION,
                    "database": "ok",
                    "checks": {
                        "database": "ok",
                        "config": "ok"
                    },
                    "jobs": jobs,
                })),
            )
        }
        Ok((None, _)) => {
            tracing::error!("Readiness check failed: no migrations applied");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "unready",
                    "version": BUILD_VERSION,
                    "revision": BUILD_REVISION,
                    "database": "unavailable",
                })),
            )
        }
        Err(e) => {
            tracing::error!("Readiness check failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "unready",
                    "version": BUILD_VERSION,
                    "revision": BUILD_REVISION,
                    "database": "unavailable",
                })),
            )
        }
    }
}
