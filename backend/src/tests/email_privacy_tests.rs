//! `email_logs` must not become a credential store, and must not ship letter
//! bodies to the list endpoint.

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::AppConfig;
use crate::services::email_service;
use crate::tests::support::{seed_company, seed_employee, seed_user, skip_if_no_db};

/// SMTP is deliberately unconfigured: that is the production-default path where
/// the log row is written and then marked failed, which is exactly how the
/// initial password ended up persisted on deployments that send no mail at all.
fn test_config() -> AppConfig {
    AppConfig {
        database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://test".into()),
        jwt_secret: "email-privacy-test-secret".into(),
        jwt_expiry_hours: 1,
        server_host: "127.0.0.1".into(),
        server_port: 0,
        frontend_url: "http://localhost:5173".into(),
        google_client_id: None,
        google_client_secret: None,
        webauthn_rp_id: "localhost".into(),
        webauthn_rp_origin: "http://localhost:5173".into(),
        smtp_host: None,
        smtp_port: None,
        smtp_username: None,
        smtp_password: None,
        smtp_from_email: None,
        smtp_from_name: None,
        trust_proxy_headers: false,
    }
}

async fn stored_body(pool: &PgPool, log_id: Uuid) -> String {
    sqlx::query_scalar("SELECT body_html FROM email_logs WHERE id = $1")
        .bind(log_id)
        .fetch_one(pool)
        .await
        .expect("read stored body")
}

/// The two welcome bodies must differ only in the password cell: the log has to
/// stay usable as evidence of what was sent, minus the credential.
#[test]
fn the_stored_welcome_letter_drops_the_credential_and_nothing_else() {
    let ic = "880101105566";
    let sent = email_service::default_welcome_html(
        "Aminah",
        "TestCo",
        "https://app.example",
        "aminah@example.invalid",
        ic,
    );
    let stored = email_service::welcome_log_html(
        "Aminah",
        "TestCo",
        "https://app.example",
        "aminah@example.invalid",
    );

    assert!(
        sent.contains(ic),
        "the employee still receives their password"
    );
    assert!(!stored.contains(ic));
    assert!(stored.contains(email_service::WELCOME_LOG_PASSWORD_PLACEHOLDER));

    for fragment in [
        "Welcome to TestCo",
        "Aminah",
        "aminah@example.invalid",
        "https://app.example/login",
    ] {
        assert!(sent.contains(fragment), "sent letter lost {fragment}");
        assert!(stored.contains(fragment), "stored letter lost {fragment}");
    }
}

/// The initial password *is* the employee's IC number, and `email_logs` is
/// readable by every role holding `ViewEmailLogs` — including `exec`, whose
/// view of the employee record has `ic_number` stripped precisely so it cannot
/// read it.
#[tokio::test]
async fn the_welcome_email_log_never_holds_the_initial_password() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let actor_id = seed_user(&pool, company_id, "hr_manager").await;
    let ic = "880101105566";

    let log = email_service::send_welcome_email(
        &test_config(),
        &pool,
        company_id,
        "TestCo",
        employee_id,
        "Aminah",
        "aminah@example.invalid",
        ic,
        actor_id,
    )
    .await
    .expect("welcome letter is logged even with SMTP disabled");

    let body = stored_body(&pool, log.id).await;
    assert!(!body.contains(ic), "the IC must never reach email_logs");
    assert!(body.contains(email_service::WELCOME_LOG_PASSWORD_PLACEHOLDER));
    assert!(body.contains("aminah@example.invalid"));
}

/// Ordinary letters keep their stored body — the audit value of the letters
/// feature depends on it — but the *list* must never carry one.
#[tokio::test]
async fn the_email_log_list_does_not_ship_bodies() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor_id = seed_user(&pool, company_id, "hr_manager").await;
    let marker = format!("distinctive-body-{}", Uuid::new_v4());
    let body = format!("<p>{marker}</p>");

    let log = email_service::send_email(
        &test_config(),
        &pool,
        company_id,
        None,
        None,
        "general",
        "someone@example.invalid",
        "Someone",
        "A letter",
        &body,
        actor_id,
    )
    .await
    .expect("send letter");

    assert!(
        stored_body(&pool, log.id).await.contains(&marker),
        "an ordinary letter still records what was sent"
    );

    let (logs, _total) = email_service::list_email_logs(&pool, company_id, None, 50, 0)
        .await
        .expect("list logs");
    assert!(logs.iter().any(|l| l.id == log.id));

    // Serialized rather than type-checked: a future `#[serde(flatten)]` would
    // slip a body back onto the wire without changing the struct's own fields.
    let wire = serde_json::to_string(&logs).expect("serialize summaries");
    assert!(
        !wire.contains("body_html"),
        "the list must not carry bodies"
    );
    assert!(!wire.contains(&marker));
}
