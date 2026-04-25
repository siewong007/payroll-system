use axum::body::{Body, to_bytes};
use axum::extract::FromRequest;
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use serde::Deserialize;
use validator::Validate;

use crate::core::auth::{AuthUser, Claims, Permission};
use crate::core::extract::ValidatedJson;

fn auth_with_role(role: &str) -> AuthUser {
    AuthUser(Claims {
        sub: uuid::Uuid::new_v4(),
        email: format!("{role}@example.invalid"),
        role: role.to_string(),
        company_id: Some(uuid::Uuid::new_v4()),
        employee_id: None,
        exp: 2_000_000_000,
        iat: 1_700_000_000,
    })
}

#[derive(Debug, Deserialize, Validate)]
struct LoginLike {
    #[validate(email(message = "must be a valid email"))]
    email: String,
    #[validate(length(min = 8, message = "must be at least 8 chars"))]
    password: String,
}

fn req(body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn valid_payload_extracts_cleanly() {
    let r = req(r#"{"email":"a@b.com","password":"longenough"}"#);
    let ValidatedJson(out) = ValidatedJson::<LoginLike>::from_request(r, &())
        .await
        .expect("should parse + validate");
    assert_eq!(out.email, "a@b.com");
}

/// Field-level failures surface as 422 with both field names present in the body.
#[tokio::test]
async fn invalid_payload_becomes_422_with_field_info() {
    let r = req(r#"{"email":"not-an-email","password":"short"}"#);
    let err = ValidatedJson::<LoginLike>::from_request(r, &())
        .await
        .expect_err("should reject");

    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let msg = body.get("error").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        msg.contains("email:"),
        "error should name 'email' field: {msg}"
    );
    assert!(
        msg.contains("password:"),
        "error should name 'password' field: {msg}"
    );
}

/// Malformed JSON is rejected as a 400 (BadRequest), not 422.
#[tokio::test]
async fn malformed_json_becomes_400() {
    let r = req("{not json}");
    let err = ValidatedJson::<LoginLike>::from_request(r, &())
        .await
        .expect_err("should reject");

    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn payroll_permissions_separate_preparation_and_approval() {
    let payroll_admin = auth_with_role("payroll_admin");
    assert!(payroll_admin.can(Permission::ManagePayrollDraft));
    assert!(payroll_admin.can(Permission::SubmitPayroll));
    assert!(!payroll_admin.can(Permission::ApprovePayroll));

    let finance = auth_with_role("finance");
    assert!(finance.can(Permission::ViewPayroll));
    assert!(finance.can(Permission::ApprovePayroll));
    assert!(finance.can(Permission::MarkPayrollPaid));
    assert!(!finance.can(Permission::ManagePayrollDraft));

    let exec = auth_with_role("exec");
    assert!(!exec.can(Permission::ViewPayroll));
}
