use axum::body::to_bytes;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use chrono::{Duration, NaiveDate, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use rust_decimal_macros::dec;
use serde_json::Value;
use uuid::Uuid;

use crate::core::auth::{
    AuthUser, Claims, JWT_AUDIENCE, JWT_ISSUER, Permission, create_token, verify_token,
};
use crate::core::cookie::{clear_refresh_cookie, extract_refresh_token, set_refresh_cookie};
use crate::core::error::AppError;
use crate::models::audit::AuditRequestMeta;
use crate::services::auth_service::validate_password_strength;
use crate::services::oauth2_service::{
    compute_code_challenge, generate_code_verifier, google_authorize_url,
};
use crate::services::pcb_calculator::round_up_to_ringgit;
use crate::services::pdf_helpers::sen_to_rm;
use crate::services::portal_service::calculate_prorated_days;

const TEST_SECRET: &str = "test-secret-that-is-long-enough-for-tests";

fn claims_with_roles(roles: &[&str]) -> Claims {
    Claims {
        sub: Uuid::new_v4(),
        email: "person@example.test".to_string(),
        roles: roles.iter().map(|role| (*role).to_string()).collect(),
        company_id: Some(Uuid::new_v4()),
        employee_id: Some(Uuid::new_v4()),
        exp: (Utc::now() + Duration::hours(1)).timestamp(),
        iat: Utc::now().timestamp(),
        iss: JWT_ISSUER.to_string(),
        aud: JWT_AUDIENCE.to_string(),
    }
}

fn signed_token(claims: &Claims) -> String {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
    )
    .expect("test claims should encode")
}

#[test]
fn jwt_round_trip_preserves_context_and_deduplicates_roles() {
    let user_id = Uuid::new_v4();
    let company_id = Uuid::new_v4();
    let employee_id = Uuid::new_v4();
    let roles = vec![
        "payroll_admin".to_string(),
        "finance".to_string(),
        "payroll_admin".to_string(),
    ];

    let token = create_token(
        user_id,
        "payroll@example.test",
        &roles,
        Some(company_id),
        Some(employee_id),
        TEST_SECRET,
        1,
    )
    .expect("token should be created");
    let verified = verify_token(&token, TEST_SECRET).expect("token should verify");

    assert_eq!(verified.sub, user_id);
    assert_eq!(verified.company_id, Some(company_id));
    assert_eq!(verified.employee_id, Some(employee_id));
    assert_eq!(verified.roles, ["payroll_admin", "finance"]);
    assert_eq!(verified.iss, JWT_ISSUER);
    assert_eq!(verified.aud, JWT_AUDIENCE);
}

#[test]
fn jwt_rejects_wrong_secret_issuer_audience_and_expiry() {
    let valid = claims_with_roles(&["admin"]);
    assert!(matches!(
        verify_token(&signed_token(&valid), "wrong-secret"),
        Err(AppError::Unauthorized(_))
    ));

    let mut wrong_issuer = valid.clone();
    wrong_issuer.iss = "another-service".to_string();
    assert!(matches!(
        verify_token(&signed_token(&wrong_issuer), TEST_SECRET),
        Err(AppError::Unauthorized(_))
    ));

    let mut wrong_audience = valid.clone();
    wrong_audience.aud = "another-audience".to_string();
    assert!(matches!(
        verify_token(&signed_token(&wrong_audience), TEST_SECRET),
        Err(AppError::Unauthorized(_))
    ));

    let mut expired = valid;
    expired.exp = (Utc::now() - Duration::minutes(5)).timestamp();
    assert!(matches!(
        verify_token(&signed_token(&expired), TEST_SECRET),
        Err(AppError::Unauthorized(_))
    ));
}

#[test]
fn payroll_permissions_enforce_separation_of_duties() {
    let cases = [
        ("super_admin", [true, true, true, true, true]),
        ("payroll_admin", [true, true, true, false, false]),
        ("finance", [true, false, false, true, true]),
        ("admin", [false, false, false, false, false]),
        ("exec", [false, false, false, false, false]),
        ("employee", [false, false, false, false, false]),
    ];
    let permissions = [
        Permission::ViewPayroll,
        Permission::ManagePayrollDraft,
        Permission::SubmitPayroll,
        Permission::ApprovePayroll,
        Permission::MarkPayrollPaid,
    ];

    for (role, expected) in cases {
        let auth = AuthUser(claims_with_roles(&[role]));
        let actual = permissions.map(|permission| auth.can(permission));
        assert_eq!(actual, expected, "unexpected permissions for {role}");
    }
}

#[test]
fn role_guards_cover_exec_employee_and_attendance_boundaries() {
    let exec = AuthUser(claims_with_roles(&["exec"]));
    assert!(matches!(exec.deny_exec(), Err(AppError::Forbidden(_))));
    assert!(exec.require_attendance_qr_generator().is_ok());
    assert!(matches!(
        exec.require_kiosk_admin(),
        Err(AppError::Forbidden(_))
    ));

    let employee = AuthUser(claims_with_roles(&["employee"]));
    assert!(matches!(
        employee.require_non_employee(),
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        employee.require_attendance_qr_generator(),
        Err(AppError::Forbidden(_))
    ));

    let hr = AuthUser(claims_with_roles(&["hr_manager"]));
    assert!(hr.require_hr_admin().is_ok());
    assert!(hr.require_kiosk_admin().is_ok());
    assert!(matches!(
        hr.require_company_admin(),
        Err(AppError::Forbidden(_))
    ));
}

#[test]
fn missing_company_and_employee_context_is_forbidden() {
    let mut claims = claims_with_roles(&["employee"]);
    claims.company_id = None;
    claims.employee_id = None;
    let auth = AuthUser(claims);

    assert!(matches!(auth.company_id(), Err(AppError::Forbidden(_))));
    assert!(matches!(auth.employee_id(), Err(AppError::Forbidden(_))));
}

#[test]
fn refresh_cookie_has_security_attributes_and_narrow_path() {
    let (name, production) = set_refresh_cookie("opaque-token", "https://payroll.example");
    assert_eq!(name, header::SET_COOKIE);
    assert!(production.starts_with("refresh_token=opaque-token;"));
    assert!(production.contains("; HttpOnly"));
    assert!(production.contains("; Secure"));
    assert!(production.contains("; SameSite=Strict"));
    assert!(production.contains("; Path=/api/auth"));
    assert!(production.contains("; Max-Age=2592000"));

    let (_, local) = set_refresh_cookie("opaque-token", "http://localhost:5173");
    assert!(!local.contains("; Secure"));

    let (_, cleared) = clear_refresh_cookie("https://payroll.example");
    assert!(cleared.starts_with("refresh_token=;"));
    assert!(cleared.contains("; Max-Age=0"));
    assert!(cleared.contains("; Secure"));
}

#[test]
fn refresh_cookie_extraction_matches_exact_non_empty_cookie() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static(
            "session=abc; not_refresh_token=wrong; refresh_token=right=value; theme=dark",
        ),
    );
    assert_eq!(
        extract_refresh_token(&headers).as_deref(),
        Some("right=value")
    );

    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("session=abc; refresh_token="),
    );
    assert_eq!(extract_refresh_token(&headers), None);
}

#[test]
fn password_policy_accepts_only_complete_passwords() {
    assert!(validate_password_strength("StrongPass1").is_ok());

    for invalid in [
        "Short1Aa",
        "lowercase1only",
        "UPPERCASE1ONLY",
        "NoDigitsHere",
    ] {
        assert!(
            matches!(
                validate_password_strength(invalid),
                Err(AppError::Validation(_))
            ),
            "password should be rejected: {invalid}"
        );
    }
}

async fn error_response(error: AppError) -> (StatusCode, Value) {
    let response = error.into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("error body should be readable");
    let body = serde_json::from_slice(&bytes).expect("error body should be JSON");
    (status, body)
}

#[tokio::test]
async fn internal_and_database_errors_do_not_leak_details() {
    for error in [
        AppError::Internal("JWT signing key leaked".to_string()),
        AppError::Database(sqlx::Error::RowNotFound),
    ] {
        let (status, body) = error_response(error).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"], "Internal server error");
        assert_eq!(body["status"], 500);
        assert!(!body.to_string().contains("leaked"));
        assert!(!body.to_string().contains("RowNotFound"));
    }
}

#[tokio::test]
async fn public_error_variants_keep_their_status_and_message() {
    let cases = [
        (AppError::BadRequest("bad input".into()), 400),
        (AppError::Unauthorized("sign in".into()), 401),
        (AppError::Forbidden("not allowed".into()), 403),
        (AppError::NotFound("missing".into()), 404),
        (AppError::Conflict("duplicate".into()), 409),
        (AppError::Validation("invalid field".into()), 422),
    ];

    for (error, expected_status) in cases {
        let expected_message = match &error {
            AppError::BadRequest(message)
            | AppError::Unauthorized(message)
            | AppError::Forbidden(message)
            | AppError::NotFound(message)
            | AppError::Conflict(message)
            | AppError::Validation(message) => message.clone(),
            _ => unreachable!("test cases contain only public error variants"),
        };
        let (status, body) = error_response(error).await;
        assert_eq!(status.as_u16(), expected_status);
        assert_eq!(body["error"], expected_message);
        assert_eq!(body["status"], expected_status);
    }
}

#[test]
fn pkce_challenge_matches_rfc_7636_vector() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert_eq!(
        compute_code_challenge(verifier),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn generated_pkce_verifier_has_valid_length_and_alphabet() {
    let first = generate_code_verifier();
    let second = generate_code_verifier();

    assert_eq!(first.len(), 43);
    assert!(
        first
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    );
    assert_ne!(first, second);
}

#[test]
fn google_authorize_url_round_trips_encoded_parameters() {
    let authorize_url = google_authorize_url(
        "client id/+",
        "https://payroll.example/api/oauth2/callback?tenant=a&mode=login",
        "state /+?&",
        "challenge_-",
    );
    let parsed = url::Url::parse(&authorize_url).expect("authorization URL should parse");
    let params = parsed
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(
        params.get("client_id").map(String::as_str),
        Some("client id/+")
    );
    assert_eq!(
        params.get("redirect_uri").map(String::as_str),
        Some("https://payroll.example/api/oauth2/callback?tenant=a&mode=login")
    );
    assert_eq!(params.get("state").map(String::as_str), Some("state /+?&"));
    assert_eq!(
        params.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
}

#[test]
fn leave_proration_handles_year_boundaries_and_half_day_rounding() {
    assert_eq!(
        calculate_prorated_days(
            dec!(12),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            2026,
        ),
        dec!(12)
    );
    assert_eq!(
        calculate_prorated_days(dec!(12), NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(), 2026,),
        dec!(0)
    );
    assert_eq!(
        calculate_prorated_days(
            dec!(14),
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
            2026,
        ),
        dec!(7)
    );
    assert_eq!(
        calculate_prorated_days(
            dec!(14),
            NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(),
            2026,
        ),
        dec!(1)
    );
}

#[test]
fn prototype_pcb_rounding_helper_covers_boundaries() {
    assert_eq!(round_up_to_ringgit(-1), 0);
    assert_eq!(round_up_to_ringgit(0), 0);
    assert_eq!(round_up_to_ringgit(100), 100);
    assert_eq!(round_up_to_ringgit(101), 200);
}

#[test]
fn money_formatting_handles_signs_and_grouping() {
    assert_eq!(sen_to_rm(0), "0.00");
    assert_eq!(sen_to_rm(1), "0.01");
    assert_eq!(sen_to_rm(123_456), "1,234.56");
    assert_eq!(sen_to_rm(-123_456), "-1,234.56");
}

#[test]
fn audit_metadata_prefers_forwarded_ip_and_truncates_user_agent() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static(" 203.0.113.7, 10.0.0.1 "),
    );
    headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.9"));
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_str(&"x".repeat(600)).unwrap(),
    );

    let meta = AuditRequestMeta::from_headers(&headers);
    assert_eq!(meta.ip_address.as_deref(), Some("203.0.113.7"));
    assert_eq!(meta.user_agent.as_deref().map(str::len), Some(500));
}

#[test]
fn audit_metadata_falls_back_to_real_ip_and_ignores_blank_values() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("  "));
    headers.insert("x-real-ip", HeaderValue::from_static(" 198.51.100.9 "));
    headers.insert(header::USER_AGENT, HeaderValue::from_static("  "));

    let meta = AuditRequestMeta::from_headers(&headers);
    assert_eq!(meta.ip_address.as_deref(), Some("198.51.100.9"));
    assert_eq!(meta.user_agent, None);
}
