use std::sync::Arc;

use axum::Extension;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;
use url::Url;
use webauthn_rs::prelude::*;

use crate::core::app_state::AppState;
use crate::core::auth::{JwtSecret, create_token};
use crate::core::config::AppConfig;
use crate::routes;
use crate::tests::support::{seed_company, seed_user, skip_if_no_db};

const JWT_SECRET: &str = "route-auth-test-secret";

fn test_config(database_url: String) -> AppConfig {
    AppConfig {
        database_url,
        jwt_secret: JWT_SECRET.to_string(),
        jwt_expiry_hours: 1,
        server_host: "127.0.0.1".to_string(),
        server_port: 0,
        frontend_url: "http://localhost:5173".to_string(),
        google_client_id: None,
        google_client_secret: None,
        webauthn_rp_id: "localhost".to_string(),
        webauthn_rp_origin: "http://localhost:5173".to_string(),
        smtp_host: None,
        smtp_port: None,
        smtp_username: None,
        smtp_password: None,
        smtp_from_email: None,
        smtp_from_name: None,
    }
}

fn test_webauthn() -> Arc<Webauthn> {
    let origin = Url::parse("http://localhost:5173").expect("valid origin");
    Arc::new(
        WebauthnBuilder::new("localhost", &origin)
            .expect("build webauthn")
            .rp_name("PayrollMY Test")
            .build()
            .expect("finish webauthn"),
    )
}

fn request(method: &str, uri: &str, token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-forwarded-for", "203.0.113.10, 10.0.0.1")
        .header(header::USER_AGENT, "PayrollRouteTest/1.0")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

async fn app_for(pool: sqlx::PgPool) -> axum::Router {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://test".to_string());
    let config = test_config(database_url);
    let state = AppState {
        pool,
        config: config.clone(),
        webauthn: test_webauthn(),
    };

    routes::create_router(state).layer(Extension(JwtSecret(config.jwt_secret)))
}

async fn token_for(pool: &sqlx::PgPool, company_id: uuid::Uuid, role: &str) -> String {
    token_and_user_for(pool, company_id, role).await.0
}

async fn token_and_user_for(
    pool: &sqlx::PgPool,
    company_id: uuid::Uuid,
    role: &str,
) -> (String, uuid::Uuid) {
    let user_id = seed_user(pool, company_id, role).await;
    let token = create_token(
        user_id,
        "route-test@example.invalid",
        role,
        Some(company_id),
        None,
        JWT_SECRET,
        1,
    )
    .expect("create jwt");
    (token, user_id)
}

#[tokio::test]
async fn non_admin_cannot_change_company_attendance_method() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "PUT",
            "/api/attendance/company-method",
            &token,
            r#"{"method":"face_id"}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_hr_admin_cannot_create_geofence_location() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "POST",
            "/api/geofence/locations",
            &token,
            r#"{"name":"HQ","latitude":3.139,"longitude":101.6869,"radius_meters":200}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn non_hr_admin_cannot_update_default_work_schedule() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let token = token_for(&pool, company_id, "finance").await;

    let response = app_for(pool)
        .await
        .oneshot(request(
            "PUT",
            "/api/work-schedules/default",
            &token,
            r#"{"name":"Default","start_time":"09:00","end_time":"18:00"}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn audited_route_writes_request_metadata() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let (token, user_id) = token_and_user_for(&pool, company_id, "admin").await;

    let response = app_for(pool.clone())
        .await
        .oneshot(request(
            "POST",
            "/api/geofence/locations",
            &token,
            r#"{"name":"HQ","latitude":3.139,"longitude":101.6869,"radius_meters":200}"#,
        ))
        .await
        .expect("route response");

    assert_eq!(response.status(), StatusCode::OK);

    let row: (Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT ip_address, user_agent
           FROM audit_logs
           WHERE user_id = $1 AND entity_type = 'company_location' AND action = 'create'
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("audit metadata row");

    assert_eq!(row.0.as_deref(), Some("203.0.113.10"));
    assert_eq!(row.1.as_deref(), Some("PayrollRouteTest/1.0"));
}
