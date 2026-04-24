use crate::models::company_location::{CreateLocationRequest, UpdateLocationRequest};
use crate::models::work_schedule::{CreateWorkScheduleRequest, UpdateWorkScheduleRequest};
use crate::services::{attendance_service, geofence_service, work_schedule_service};
use crate::tests::support::{seed_company, seed_user, skip_if_no_db};

async fn audit_count(pool: &sqlx::PgPool, user_id: uuid::Uuid, entity_type: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE user_id = $1 AND entity_type = $2")
        .bind(user_id)
        .bind(entity_type)
        .fetch_one(pool)
        .await
        .expect("count audit logs")
}

async fn audit_count_for_action(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    entity_type: &str,
    action: &str,
) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE user_id = $1 AND entity_type = $2 AND action = $3",
    )
    .bind(user_id)
    .bind(entity_type)
    .bind(action)
    .fetch_one(pool)
    .await
    .expect("count audit logs for action")
}

#[tokio::test]
async fn attendance_settings_and_kiosk_mutations_write_audit_logs() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor_id = seed_user(&pool, company_id, "super_admin").await;

    attendance_service::set_platform_attendance_method(&pool, "face_id", true, actor_id, None)
        .await
        .expect("set platform attendance method");
    attendance_service::set_company_attendance_method(
        &pool,
        company_id,
        Some("qr_code"),
        actor_id,
        None,
    )
    .await
    .expect("set company attendance method");
    let (credential, secret) = attendance_service::create_kiosk_credential(
        &pool,
        company_id,
        "Lobby kiosk",
        actor_id,
        None,
    )
    .await
    .expect("create kiosk credential");
    assert!(
        !secret.is_empty(),
        "secret should be returned once at creation"
    );
    attendance_service::revoke_kiosk_credential(&pool, credential.id, company_id, actor_id, None)
        .await
        .expect("revoke kiosk credential");

    assert_eq!(
        audit_count(&pool, actor_id, "platform_attendance_method").await,
        1
    );
    assert_eq!(
        audit_count(&pool, actor_id, "company_attendance_method").await,
        1
    );
    assert_eq!(
        audit_count_for_action(&pool, actor_id, "attendance_kiosk_credential", "create").await,
        1
    );
    assert_eq!(
        audit_count_for_action(&pool, actor_id, "attendance_kiosk_credential", "revoke").await,
        1
    );
}

#[tokio::test]
async fn geofence_and_work_schedule_mutations_write_audit_logs() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let actor_id = seed_user(&pool, company_id, "admin").await;

    let location = geofence_service::create_location(
        &pool,
        company_id,
        &CreateLocationRequest {
            name: "HQ".to_string(),
            latitude: 3.139,
            longitude: 101.6869,
            radius_meters: Some(200),
        },
        actor_id,
        None,
    )
    .await
    .expect("create geofence location");
    geofence_service::update_location(
        &pool,
        company_id,
        location.id,
        &UpdateLocationRequest {
            name: Some("HQ Main".to_string()),
            latitude: None,
            longitude: None,
            radius_meters: Some(250),
            is_active: Some(true),
        },
        actor_id,
        None,
    )
    .await
    .expect("update geofence location");
    geofence_service::set_geofence_mode(&pool, company_id, "warn", actor_id, None)
        .await
        .expect("set geofence mode");
    geofence_service::delete_location(&pool, company_id, location.id, actor_id, None)
        .await
        .expect("delete geofence location");

    let schedule = work_schedule_service::upsert_default_schedule(
        &pool,
        company_id,
        &CreateWorkScheduleRequest {
            name: Some("Default".to_string()),
            start_time: "09:00".to_string(),
            end_time: "18:00".to_string(),
            grace_minutes: Some(15),
            half_day_hours: Some(4.0),
            timezone: Some("Asia/Kuala_Lumpur".to_string()),
        },
        actor_id,
        None,
    )
    .await
    .expect("create default schedule");
    work_schedule_service::update_schedule(
        &pool,
        company_id,
        schedule.id,
        &UpdateWorkScheduleRequest {
            name: Some("Default updated".to_string()),
            start_time: Some("08:30".to_string()),
            end_time: None,
            grace_minutes: Some(10),
            half_day_hours: None,
            timezone: None,
        },
        actor_id,
        None,
    )
    .await
    .expect("update work schedule");

    assert_eq!(
        audit_count_for_action(&pool, actor_id, "company_location", "create").await,
        1
    );
    assert_eq!(
        audit_count_for_action(&pool, actor_id, "company_location", "update").await,
        1
    );
    assert_eq!(
        audit_count_for_action(&pool, actor_id, "company_location", "delete").await,
        1
    );
    assert_eq!(audit_count(&pool, actor_id, "geofence_mode").await, 1);
    assert_eq!(
        audit_count_for_action(&pool, actor_id, "work_schedule", "create").await,
        1
    );
    assert_eq!(
        audit_count_for_action(&pool, actor_id, "work_schedule", "update").await,
        1
    );
}
