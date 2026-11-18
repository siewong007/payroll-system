use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};

use crate::core::app_state::AppState;

/// Deep health check for orchestration probes (Kubernetes readiness, ALB
/// target-group health). Verifies the database is reachable and that at least
/// one migration has been applied successfully — that second check catches
/// pointing the app at a fresh/empty database by mistake.
///
/// Returns 200 + `{status: "ready", ...}` when healthy, 503 otherwise. The
/// lightweight `/health` endpoint is intentionally left alone so quick liveness
/// probes stay cheap.
pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let row = crate::repositories::reads::system::get_migration_status(&state.pool).await;

    match row {
        Ok((Some(_latest), _count)) => (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "database": "ok",
            })),
        ),
        Ok((None, _)) => {
            tracing::error!("Readiness check failed: no migrations applied");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "unready",
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
                    "database": "unavailable",
                })),
            )
        }
    }
}
