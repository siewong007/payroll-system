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
use crate::services::approval_service::ensure_overtime_hours_within_window;
use crate::services::auth_service::validate_password_strength;
use crate::services::oauth2_service::{
    STALE_GRANT_MESSAGE, classify_token_exchange_error, compute_code_challenge,
    generate_code_verifier, google_authorize_url,
};
use crate::services::pcb_calculator::round_up_to_ringgit;
use crate::services::pdf_helpers::{sen_to_rm, unclassified_earnings};
use crate::services::portal_service::calculate_prorated_days;

const TEST_SECRET: &str = "test-secret-that-is-long-enough-for-tests";

fn claims_with_roles(roles: &[&str]) -> Claims {
    Claims {
        sub: Uuid::new_v4(),
        email: "person@example.test".to_string(),
        roles: roles.iter().map(|role| (*role).to_string()).collect(),
        company_id: Some(Uuid::new_v4()),
        employee_id: Some(Uuid::new_v4()),
        sid: Uuid::new_v4(),
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
        Uuid::new_v4(),
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
        let auth = AuthUser(claims_with_roles(&[role]), Vec::new());
        let actual = permissions.map(|permission| auth.can(permission));
        assert_eq!(actual, expected, "unexpected permissions for {role}");
    }
}

#[test]
fn role_guards_cover_exec_employee_and_attendance_boundaries() {
    let exec = AuthUser(claims_with_roles(&["exec"]), Vec::new());
    // exec is read-mostly: may view attendance data, but generating a QR is a
    // write (it retires the console's live token) and is denied. Correcting
    // team membership and managing kiosks are writes too.
    assert!(exec.can(Permission::ViewAttendance));
    assert!(!exec.can(Permission::GenerateAttendanceQr));
    assert!(!exec.can(Permission::ManageKiosks));
    assert!(!exec.can(Permission::ManageAttendance));
    assert!(exec.can(Permission::ViewTeams));
    assert!(!exec.can(Permission::ManageTeams));
    assert!(matches!(
        exec.require_permission(Permission::ViewPayroll),
        Err(AppError::Forbidden(_))
    ));

    let employee = AuthUser(claims_with_roles(&["employee"]), Vec::new());
    assert!(!employee.can(Permission::ViewEmployees));
    assert!(!employee.can(Permission::ViewAttendance));
    assert!(!employee.can(Permission::GenerateAttendanceQr));
    assert!(!employee.can(Permission::ViewDocuments));

    let finance = AuthUser(claims_with_roles(&["finance"]), Vec::new());
    assert!(finance.can(Permission::ViewAttendance));
    assert!(!finance.can(Permission::GenerateAttendanceQr));

    let hr = AuthUser(claims_with_roles(&["hr_manager"]), Vec::new());
    assert!(hr.can(Permission::ManageWorkSchedules));
    assert!(hr.can(Permission::ManageKiosks));
    assert!(hr.can(Permission::GenerateAttendanceQr));
    assert!(matches!(
        hr.require_permission(Permission::ManageCompanySettings),
        Err(AppError::Forbidden(_))
    ));
}

/// A user holding several roles gets the union of their grants. The old
/// deny-list guard `require_non_employee` inverted this for one specific
/// combination, so it is worth pinning explicitly.
#[test]
fn multiple_roles_grant_the_union_of_their_permissions() {
    let auth = AuthUser(claims_with_roles(&["finance", "hr_manager"]), Vec::new());
    assert!(auth.can(Permission::ApprovePayroll), "from finance");
    assert!(auth.can(Permission::ManageAttendance), "from hr_manager");
    assert!(!auth.can(Permission::ManageUsers), "from neither");

    // Holding `employee` alongside a privileged role must not subtract from it.
    let mixed = AuthUser(claims_with_roles(&["hr_manager", "employee"]), Vec::new());
    assert!(mixed.can(Permission::ViewEmployees));
    assert!(mixed.can(Permission::ManageAttendance));
}

/// Group membership adds permissions on top of roles and can never take one
/// away — the property that keeps "why can this person do X?" answerable by
/// union rather than by replaying an ordered set of allows and denies.
#[test]
fn group_grants_add_to_role_grants_and_never_subtract() {
    let roles_only = AuthUser(claims_with_roles(&["hr_manager"]), Vec::new());
    assert!(!roles_only.can(Permission::ViewAuditLog));

    let with_group = AuthUser(
        claims_with_roles(&["hr_manager"]),
        vec![Permission::ViewAuditLog],
    );
    assert!(with_group.can(Permission::ViewAuditLog), "granted by group");
    assert!(
        with_group.can(Permission::ManageAttendance),
        "the role's own grants survive"
    );
    assert!(
        !with_group.can(Permission::ViewPayroll),
        "a group grants only what it lists"
    );

    // A group that happens to list something the role already confers is a
    // no-op, not a conflict.
    let overlapping = AuthUser(
        claims_with_roles(&["hr_manager"]),
        vec![Permission::ManageAttendance],
    );
    assert!(overlapping.can(Permission::ManageAttendance));

    // The effective set is de-duplicated across both sources.
    let effective = overlapping.permissions();
    let occurrences = effective
        .iter()
        .filter(|p| **p == Permission::ManageAttendance)
        .count();
    assert_eq!(occurrences, 1, "permissions() must not repeat a grant");
}

/// An employee-only account holds nothing by role, so a group is the only way
/// it could gain a capability — and must gain exactly that one.
#[test]
fn a_group_can_grant_a_roleless_account_a_single_capability() {
    let auth = AuthUser(
        claims_with_roles(&["employee"]),
        vec![Permission::ViewCalendar],
    );
    assert!(auth.can(Permission::ViewCalendar));
    assert_eq!(auth.permissions(), vec![Permission::ViewCalendar]);
    assert!(!auth.can(Permission::ViewPayroll));
    assert!(!auth.can(Permission::ManageUsers));
}

#[test]
fn missing_company_and_employee_context_is_forbidden() {
    let mut claims = claims_with_roles(&["employee"]);
    claims.company_id = None;
    claims.employee_id = None;
    let auth = AuthUser(claims, Vec::new());

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
fn expired_authorization_code_is_the_users_problem_not_a_server_error() {
    // A code that timed out or was replayed is the single most common OAuth2
    // failure. Reporting it as a 500 told the user the server was broken and
    // hid the one action that actually helps: start the sign-in again.
    let err = classify_token_exchange_error(
        400,
        r#"{"error":"invalid_grant","error_description":"Bad Request"}"#,
    );

    assert!(matches!(err, AppError::BadRequest(_)));
    let (status, message) = err.client_response();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(message, STALE_GRANT_MESSAGE);
}

#[test]
fn misconfigured_credentials_stay_a_generic_500() {
    // client_id/secret problems are ours, not the caller's: the body names our
    // deployment's configuration and must never be echoed back.
    for code in [
        "invalid_client",
        "unauthorized_client",
        "invalid_scope",
        "unsupported_grant_type",
        "invalid_request",
    ] {
        let body = format!(r#"{{"error":"{code}","error_description":"secret-ish detail"}}"#);
        let err = classify_token_exchange_error(401, &body);

        assert!(
            matches!(err, AppError::Internal(_)),
            "{code} should be treated as a server misconfiguration"
        );
        let (status, message) = err.client_response();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(message, "Internal server error");
        assert!(!message.contains("secret-ish detail"));
    }
}

#[test]
fn google_outage_is_reported_as_a_gateway_failure() {
    // A 5xx with no OAuth2 error code is Google failing, not this service.
    let err = classify_token_exchange_error(503, "<html>Service Unavailable</html>");

    assert!(matches!(err, AppError::BadGateway(_)));
    let (status, message) = err.client_response();
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(message.contains("temporarily unavailable"));
}

#[test]
fn unrecognised_4xx_body_does_not_leak_and_is_not_retryable_advice() {
    // Unparseable or unexpected 4xx: no guessing. Log it, return a generic 500
    // rather than telling the user to retry something that will fail again.
    let err = classify_token_exchange_error(418, "not json at all");

    assert!(matches!(err, AppError::Internal(_)));
    assert_eq!(err.client_response().0, StatusCode::INTERNAL_SERVER_ERROR);
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

/// A fully classified payslip has no residual, so no extra line is printed.
#[test]
fn a_fully_classified_payslip_has_no_unclassified_earnings() {
    // basic 300_000 + allowances 30_000 + overtime 5_000 + bonus 100_000
    // + commission 20_000 = gross 455_000.
    assert_eq!(
        unclassified_earnings(455_000, 300_000, 30_000, 5_000, 100_000, 20_000),
        0
    );
    assert_eq!(unclassified_earnings(0, 0, 0, 0, 0, 0), 0);
}

/// An earning staged under an `item_type` outside the four allow-lists reaches
/// gross without reaching any of the five printed categories. This is the gap
/// that made the payslip and EA form disagree with their own totals.
#[test]
fn earnings_outside_the_named_categories_surface_as_the_residual() {
    assert_eq!(
        unclassified_earnings(350_000, 300_000, 0, 0, 0, 0),
        50_000,
        "RM500 staged as 'manual_adjustment' must be named, not silently absorbed"
    );

    // Only reachable from hand-edited data, but reported rather than hidden:
    // the total is authoritative and a visible negative line is legible.
    assert_eq!(unclassified_earnings(300_000, 350_000, 0, 0, 0, 0), -50_000);
}

fn peer(value: &str) -> Option<std::net::IpAddr> {
    Some(value.parse().expect("valid ip"))
}

#[test]
fn audit_metadata_ignores_forwarded_headers_when_the_proxy_is_not_trusted() {
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

    // This is the production configuration (`TRUST_PROXY_HEADERS` defaults to
    // false). The recorded address must be the TCP peer, not anything the
    // caller put in a header — an audit trail whose IP column is caller-chosen
    // is worse than useless, because it looks like evidence.
    let meta = AuditRequestMeta::from_request(&headers, peer("192.0.2.44"), false);
    assert_eq!(meta.ip_address.as_deref(), Some("192.0.2.44"));
    assert_eq!(meta.user_agent.as_deref().map(str::len), Some(500));
}

#[test]
fn audit_metadata_takes_the_proxy_appended_entry_when_trusted() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static(" 203.0.113.7, 10.0.0.1 "),
    );

    // Behind a trusted proxy the right-most entry is the one the proxy
    // appended; 203.0.113.7 is whatever the client claimed.
    let meta = AuditRequestMeta::from_request(&headers, peer("192.0.2.44"), true);
    assert_eq!(meta.ip_address.as_deref(), Some("10.0.0.1"));
}

#[test]
fn audit_metadata_falls_back_to_real_ip_and_ignores_blank_values() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("  "));
    headers.insert("x-real-ip", HeaderValue::from_static(" 198.51.100.9 "));
    headers.insert(header::USER_AGENT, HeaderValue::from_static("  "));

    let meta = AuditRequestMeta::from_request(&headers, peer("192.0.2.44"), true);
    assert_eq!(meta.ip_address.as_deref(), Some("198.51.100.9"));
    assert_eq!(meta.user_agent, None);
}

#[test]
fn audit_metadata_without_any_address_records_none() {
    let headers = HeaderMap::new();
    let meta = AuditRequestMeta::from_request(&headers, None, false);
    assert_eq!(meta.ip_address, None);
}

fn hhmm(value: &str) -> chrono::NaiveTime {
    chrono::NaiveTime::parse_from_str(value, "%H:%M").expect("valid time")
}

/// `hours` is multiplied by an hourly rate to stage a payroll earning, and the
/// three admin paths took it straight from the request. The exploit in the
/// report is 999.99 hours declared over a one-hour window.
#[test]
fn overtime_hours_cannot_exceed_the_declared_window() {
    assert!(
        ensure_overtime_hours_within_window(dec!(999.99), hhmm("09:00"), hhmm("10:00")).is_err(),
        "999.99 hours over a one-hour window must be refused"
    );
    assert!(ensure_overtime_hours_within_window(dec!(8), hhmm("09:00"), hhmm("17:00")).is_ok());
}

#[test]
fn overtime_hours_must_be_positive() {
    // A negative value stages a negative earning; zero stages nothing but is
    // not a meaningful application either.
    assert!(ensure_overtime_hours_within_window(dec!(0), hhmm("09:00"), hhmm("17:00")).is_err());
    assert!(ensure_overtime_hours_within_window(dec!(-50), hhmm("09:00"), hhmm("17:00")).is_err());
}

/// The wrap past midnight is what a night shift needs — a naive `end > start`
/// check would reject every one of them.
#[test]
fn overtime_window_wraps_past_midnight() {
    assert!(ensure_overtime_hours_within_window(dec!(8), hhmm("22:00"), hhmm("06:00")).is_ok());
    assert!(
        ensure_overtime_hours_within_window(dec!(9), hhmm("22:00"), hhmm("06:00")).is_err(),
        "the wrapped window is still a bound, not an exemption"
    );
}

#[test]
fn overtime_window_boundary_is_inclusive() {
    assert!(ensure_overtime_hours_within_window(dec!(2), hhmm("09:00"), hhmm("11:00")).is_ok());
    assert!(ensure_overtime_hours_within_window(dec!(2.01), hhmm("09:00"), hhmm("11:00")).is_err());
}

/// The wrap makes the declared window at most 24 h by construction, so the
/// service rule already entails the `hours <= 24` database CHECK rather than
/// contradicting it.
#[test]
fn overtime_window_never_admits_more_than_a_day() {
    // One minute past midnight from one minute past is the widest wrapped
    // window that is still a window: 23 h 59 m.
    assert!(ensure_overtime_hours_within_window(dec!(23), hhmm("09:00"), hhmm("08:59")).is_ok());
    assert!(ensure_overtime_hours_within_window(dec!(24), hhmm("09:00"), hhmm("08:59")).is_err());
}

/// A zero-length window is a half-filled form, not a night shift. Wrapping it
/// past midnight turned "09:00 to 09:00" into 24 declared hours, which is the
/// widest possible overtime claim reachable without declaring anything at all.
#[test]
fn overtime_window_of_zero_length_declares_nothing() {
    for hours in [dec!(0.5), dec!(8), dec!(24)] {
        assert!(
            ensure_overtime_hours_within_window(hours, hhmm("09:00"), hhmm("09:00")).is_err(),
            "{hours} hours over a zero-length window must be refused"
        );
    }
}

// ─── Upload ceilings ───

/// Each route declares two numbers: what the file may weigh, and what the whole
/// request may weigh. The second is the `DefaultBodyLimit` and has to leave room
/// for the multipart envelope, or the layer rejects a file that the handler's
/// own check would have accepted — which is the drift the paired constants exist
/// to prevent.
#[test]
fn every_request_ceiling_leaves_room_above_its_file_ceiling() {
    use crate::handlers::{backup, calendar, employee_import};

    for (what, file_max, request_max) in [
        (
            "backup",
            backup::BACKUP_FILE_MAX_BYTES,
            backup::BACKUP_REQUEST_MAX_BYTES,
        ),
        (
            "employee import",
            employee_import::IMPORT_FILE_MAX_BYTES,
            employee_import::IMPORT_REQUEST_MAX_BYTES,
        ),
        (
            "ics import",
            calendar::ICS_FILE_MAX_BYTES,
            calendar::ICS_REQUEST_MAX_BYTES,
        ),
    ] {
        assert!(
            request_max > file_max,
            "{what}: the request ceiling must exceed the file ceiling"
        );
        // Every message renders the limit as whole megabytes.
        assert_eq!(file_max % (1024 * 1024), 0, "{what}: file ceiling");
        assert_eq!(request_max % (1024 * 1024), 0, "{what}: request ceiling");
        // axum's default is 2 MiB; a ceiling at or below it is not a ceiling.
        assert!(
            request_max > 2 * 1024 * 1024,
            "{what}: below axum's default, so the layer would be a no-op"
        );
    }
}

/// A 413 is what an operator can act on; a 400 quoting the multipart decoder is
/// not. The variant has to render as one and name the number.
#[test]
fn an_over_limit_upload_renders_as_a_413_naming_the_limit() {
    use crate::core::error::payload_too_large;

    let err = payload_too_large("The backup file", 100 * 1024 * 1024);
    let (status, message) = err.client_response();
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(
        message.contains("100 MB"),
        "the message must name the ceiling: {message}"
    );
}
