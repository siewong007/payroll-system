use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, header},
    routing::{delete, get, post, put},
};
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::compression::{CompressionLayer, DefaultPredicate, Predicate};
use tower_http::set_header::SetResponseHeaderLayer;

#[derive(Clone, Default)]
pub struct NoStorePredicate(DefaultPredicate);

impl Predicate for NoStorePredicate {
    fn should_compress<B>(&self, response: &axum::http::Response<B>) -> bool
    where
        B: http_body::Body,
    {
        let has_no_store = response
            .headers()
            .get_all(header::CACHE_CONTROL)
            .iter()
            .any(|v| {
                v.to_str()
                    .map(|s| s.to_ascii_lowercase().contains("no-store"))
                    .unwrap_or(false)
            });
        if has_no_store {
            return false;
        }
        self.0.should_compress(response)
    }
}

pub fn compression_layer() -> CompressionLayer<NoStorePredicate> {
    CompressionLayer::new().compress_when(NoStorePredicate::default())
}

use crate::core::app_state::AppState;
use crate::core::rate_limit_key::{ClientIpKeyExtractor, SessionOrIpKeyExtractor};
use crate::handlers::{
    admin, approval, attendance, attendance_network, audit, auth, backup, calendar, company,
    dashboard, document, email, employee, employee_import, geofence, health, job, notification,
    oauth2, passkey, payroll, permission, portal, report, settings, team, totp, user_group,
    work_schedule,
};

pub fn create_router(state: AppState) -> Router {
    // Behind a proxy the TCP peer is the proxy, so peer-IP keying collapses
    // every client into one bucket (a kiosk fleet rate-limits itself). Trusting
    // forwarded headers on a directly reachable API is worse — anyone could
    // spoof one and bypass the login limiter. The operator declares which
    // deployment this is; see core::rate_limit_key.
    let ip_key = ClientIpKeyExtractor::with_header(
        state.config.trust_proxy_headers,
        state.config.client_ip_header.as_deref(),
    );

    // Rate limiter: 5 requests per 60 seconds per IP
    let auth_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ip_key.clone())
            .per_second(12)
            .burst_size(5)
            .finish()
            .expect("Failed to build rate limiter"),
    );

    // Rate limiter: 3 requests per 60 seconds per IP (1 token every 20s, burst 3)
    let forgot_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ip_key.clone())
            .per_second(20)
            .burst_size(3)
            .finish()
            .expect("Failed to build rate limiter"),
    );

    // Rate limiter: 10 requests per 60 seconds per IP (OAuth2 flow)
    let oauth2_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ip_key.clone())
            .per_second(6)
            .burst_size(10)
            .finish()
            .expect("Failed to build rate limiter"),
    );

    // Public kiosk QR endpoint (~120/min/IP — 1 token every 500ms, burst 10).
    let kiosk_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ip_key.clone())
            .per_millisecond(500)
            .burst_size(10)
            .finish()
            .expect("Failed to build rate limiter"),
    );

    // Public readiness endpoint (max 15 burst, ~30/min per IP) — L-11.
    let health_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ip_key)
            .per_second(2)
            .burst_size(15)
            .finish()
            .expect("Failed to build health rate limiter"),
    );

    // Rate-limited auth & passkey routes (L-10: no-store cache headers + BREACH exemption).
    let rate_limited_auth = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/login/code", post(auth::code_login))
        .route("/auth/reset-password", post(auth::reset_password))
        .route(
            "/auth/validate-reset-token",
            post(auth::validate_reset_token),
        )
        .route("/auth/2fa/verify", post(totp::verify_login))
        .route("/auth/passkey/check", post(passkey::check_passkey))
        .route(
            "/auth/passkey/authenticate/begin",
            post(passkey::authentication_begin),
        )
        .route(
            "/auth/passkey/authenticate/complete",
            post(passkey::authentication_complete),
        )
        .route(
            "/auth/passkey/discoverable/begin",
            post(passkey::discoverable_auth_begin),
        )
        .route(
            "/auth/passkey/discoverable/complete",
            post(passkey::discoverable_auth_complete),
        )
        .layer(GovernorLayer::new(auth_rate_limit.clone()))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::PRAGMA,
            HeaderValue::from_static("no-cache"),
        ));

    let rate_limited_forgot = Router::new()
        .route("/auth/forgot-password", post(auth::forgot_password))
        .layer(GovernorLayer::new(forgot_rate_limit.clone()))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::PRAGMA,
            HeaderValue::from_static("no-cache"),
        ));

    let rate_limited_oauth2 = Router::new()
        .route(
            "/auth/oauth2/google/authorize",
            post(oauth2::google_authorize),
        )
        .route("/auth/oauth2/google/callback", get(oauth2::google_callback))
        .layer(GovernorLayer::new(oauth2_rate_limit.clone()))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::PRAGMA,
            HeaderValue::from_static("no-cache"),
        ));

    let rate_limited_kiosk = Router::new()
        .route("/attendance/kiosk/qr", post(attendance::kiosk_qr))
        .layer(GovernorLayer::new(kiosk_rate_limit.clone()));

    let rate_limited_health = Router::new()
        .route("/health/ready", get(health::ready))
        .layer(GovernorLayer::new(health_rate_limit.clone()));

    // ─── Authenticated cost controls ───
    //
    // Everything above limits unauthenticated surfaces. The expensive
    // authenticated ones were unlimited, so a valid session could run payroll
    // in a loop, re-upload until the disk filled, or drive the SMTP relay hard
    // enough to get the sending domain blocked. These are keyed per verified
    // session rather than per IP — see `SessionOrIpKeyExtractor` for why.
    let session_key = SessionOrIpKeyExtractor::with_header(
        state.config.trust_proxy_headers,
        state.config.client_ip_header.as_deref(),
        Arc::new(state.config.jwt_secret.clone()),
    );

    let session_auth_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(session_key.clone())
            .per_second(3)
            .burst_size(20)
            .finish()
            .expect("Failed to build session auth rate limiter"),
    );

    // Outbound mail: ~10/min sustained, bursts of 20 for a letter run. The
    // sharpest limit of the group because the damage is external and
    // irreversible — spam complaints against the sending domain outlive the
    // request.
    let email_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(session_key.clone())
            .per_second(6)
            .burst_size(20)
            .finish()
            .expect("Failed to build email rate limiter"),
    );

    // Payroll runs, previews and bulk imports: each walks every employee in
    // the company inside one transaction. ~6/min with a burst of 5 is far
    // above deliberate use and far below what makes the database suffer.
    let heavy_job_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(session_key.clone())
            .per_second(10)
            .burst_size(5)
            .finish()
            .expect("Failed to build heavy job rate limiter"),
    );

    // Uploads: 10 MB apiece into the API container's local disk, with no quota
    // behind them.
    let upload_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(session_key.clone())
            .per_second(4)
            .burst_size(10)
            .finish()
            .expect("Failed to build upload rate limiter"),
    );

    let import_rate_limit = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(session_key)
            .per_second(6)
            .burst_size(15)
            .finish()
            .expect("Failed to build import rate limiter"),
    );

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let ip_limiters = vec![
            Arc::downgrade(&auth_rate_limit),
            Arc::downgrade(&forgot_rate_limit),
            Arc::downgrade(&oauth2_rate_limit),
            Arc::downgrade(&kiosk_rate_limit),
            Arc::downgrade(&health_rate_limit),
        ];
        let session_limiters = vec![
            Arc::downgrade(&session_auth_rate_limit),
            Arc::downgrade(&email_rate_limit),
            Arc::downgrade(&heavy_job_rate_limit),
            Arc::downgrade(&upload_rate_limit),
            Arc::downgrade(&import_rate_limit),
        ];
        handle.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let mut any_alive = false;
                for weak in &ip_limiters {
                    if let Some(cfg) = weak.upgrade() {
                        any_alive = true;
                        cfg.limiter().retain_recent();
                        cfg.limiter().shrink_to_fit();
                    }
                }
                for weak in &session_limiters {
                    if let Some(cfg) = weak.upgrade() {
                        any_alive = true;
                        cfg.limiter().retain_recent();
                        cfg.limiter().shrink_to_fit();
                    }
                }
                if !any_alive {
                    break;
                }
            }
        });
    }

    let rate_limited_session_auth_only = Router::new()
        .route("/auth/refresh", post(auth::refresh_token))
        .route("/auth/switch-company", put(auth::switch_company))
        .route("/auth/change-password", put(auth::change_password))
        .route("/auth/2fa/setup/begin", post(totp::setup_begin))
        .route("/auth/2fa/setup/confirm", post(totp::setup_confirm))
        .route("/auth/2fa/disable", post(totp::disable))
        .route(
            "/auth/2fa/backup-codes/regenerate",
            post(totp::regenerate_backup_codes),
        )
        .route(
            "/auth/passkey/register/begin",
            post(passkey::registration_begin),
        )
        .route(
            "/auth/passkey/register/complete",
            post(passkey::registration_complete),
        )
        .route(
            "/auth/passkeys/{id}",
            put(passkey::rename_passkey).delete(passkey::delete_passkey),
        )
        .route("/auth/oauth2/google/link", post(oauth2::link_google))
        .route(
            "/auth/oauth2/accounts/{provider}",
            delete(oauth2::unlink_provider),
        )
        .layer(GovernorLayer::new(session_auth_rate_limit.clone()))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::PRAGMA,
            HeaderValue::from_static("no-cache"),
        ));

    let rate_limited_session_ops = Router::new()
        .route("/admin/users/{id}/2fa/reset", post(admin::reset_user_2fa))
        .route(
            "/notifications/subscriptions",
            post(notification::upsert_subscription).delete(notification::delete_subscription),
        )
        .route("/notifications/test-push", post(notification::test_push))
        .route("/attendance/check-in/qr", post(attendance::check_in_qr))
        .route(
            "/attendance/check-in/face-id/begin",
            post(attendance::check_in_face_id_begin),
        )
        .route(
            "/attendance/check-in/face-id",
            post(attendance::check_in_face_id),
        )
        .route("/attendance/check-out", post(attendance::check_out))
        .layer(GovernorLayer::new(session_auth_rate_limit));

    let auth_unthrottled = Router::new()
        .route("/auth/me", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/sessions", get(auth::list_sessions))
        .route("/auth/sessions/others", delete(auth::revoke_other_sessions))
        .route("/auth/sessions/{session_id}", delete(auth::revoke_session))
        .route("/auth/my-companies", get(auth::my_companies))
        .route("/auth/permissions/matrix", get(permission::matrix))
        .route("/auth/passkeys", get(passkey::list_passkeys))
        .route("/auth/2fa/status", get(totp::status))
        .route("/auth/oauth2/providers", get(oauth2::list_providers))
        .route("/auth/oauth2/accounts", get(oauth2::my_linked_accounts))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::PRAGMA,
            HeaderValue::from_static("no-cache"),
        ));

    let rate_limited_email = Router::new()
        .route("/email/send", post(email::send_letter))
        .layer(GovernorLayer::new(email_rate_limit));

    let rate_limited_heavy = Router::new()
        .route("/payroll/run", post(payroll::process))
        .route("/payroll/preview", post(payroll::preview))
        .route(
            "/employees/import/confirm",
            post(employee_import::confirm_import),
        )
        .route("/admin/backup/export", get(backup::export_company))
        // The limit goes on the `MethodRouter`, not on the merged sub-router:
        // hung on the sub-router it would also raise the ceiling on
        // `/payroll/run` and `/payroll/preview`, which take small JSON bodies.
        .route(
            "/admin/backup/import",
            post(backup::import_company)
                .layer(DefaultBodyLimit::max(backup::BACKUP_REQUEST_MAX_BYTES)),
        )
        .layer(GovernorLayer::new(heavy_job_rate_limit));

    let rate_limited_uploads = Router::new()
        .route(
            "/uploads",
            post(portal::upload_file)
                .layer(DefaultBodyLimit::max(portal::UPLOAD_REQUEST_MAX_BYTES)),
        )
        .layer(GovernorLayer::new(upload_rate_limit));

    let rate_limited_imports = Router::new()
        .route(
            "/employees/import/validate",
            post(employee_import::validate_import).layer(DefaultBodyLimit::max(
                employee_import::IMPORT_REQUEST_MAX_BYTES,
            )),
        )
        .route("/calendar/import-ics", post(calendar::import_ics))
        .route(
            "/calendar/import-ics-file",
            post(calendar::import_ics_file)
                .layer(DefaultBodyLimit::max(calendar::ICS_REQUEST_MAX_BYTES)),
        )
        .layer(GovernorLayer::new(import_rate_limit));

    let api = Router::new()
        // Health check (no auth required, used by ALB)
        .route("/health", get(|| async { "ok" }))
        // Readiness probe: rate-limited, cached 5s, loopback-only detailed diagnostics.
        .merge(rate_limited_health)
        .merge(rate_limited_auth)
        .merge(rate_limited_forgot)
        .merge(rate_limited_oauth2)
        .merge(rate_limited_kiosk)
        .merge(rate_limited_session_auth_only)
        .merge(rate_limited_session_ops)
        .merge(rate_limited_email)
        .merge(rate_limited_heavy)
        .merge(rate_limited_uploads)
        .merge(rate_limited_imports)
        .merge(auth_unthrottled)
        // User groups: company-scoped permission bundles layered on top of roles.
        .route(
            "/user-groups",
            get(user_group::list_groups).post(user_group::create_group),
        )
        .route(
            "/user-groups/{id}",
            get(user_group::get_group)
                .put(user_group::update_group)
                .delete(user_group::delete_group),
        )
        .route(
            "/user-groups/{id}/members",
            get(user_group::list_members).post(user_group::add_member),
        )
        .route(
            "/user-groups/{group_id}/members/{user_id}",
            delete(user_group::remove_member),
        )
        // Admin (super_admin)
        .route(
            "/admin/companies",
            get(admin::list_companies).post(admin::create_company),
        )
        .route(
            "/admin/companies/{id}",
            put(admin::update_company).delete(admin::delete_company),
        )
        .route(
            "/admin/users",
            get(admin::list_users).post(admin::create_user),
        )
        .route(
            "/admin/users/{id}",
            put(admin::update_user).delete(admin::delete_user),
        )
        // Platform 2FA enforcement policy (ManagePlatformSettings): the
        // switch plus the privileged accounts it would refuse at login.
        .route(
            "/admin/platform/2fa-policy",
            get(admin::get_2fa_policy).put(admin::set_2fa_policy),
        )
        .route("/admin/platform/health", get(admin::get_platform_health))
        // Employees
        .route("/employees", get(employee::list).post(employee::create))
        .route(
            "/employees/{id}",
            get(employee::get)
                .put(employee::update)
                .delete(employee::delete),
        )
        .route(
            "/employees/{id}/salary-history",
            get(employee::salary_history),
        )
        .route("/employees/{id}/tp3", post(employee::create_tp3))
        // Employee Import (/validate and /confirm are rate-limited above)
        .route(
            "/employees/import/template",
            get(employee_import::download_template),
        )
        // Payroll Groups
        .route("/payroll-groups", get(payroll::list_groups))
        // Payroll Runs (/payroll/run and /payroll/preview are rate-limited above)
        .route(
            "/payroll/entries",
            get(payroll::list_entries).post(payroll::create_entry),
        )
        .route(
            "/payroll/entries/{id}",
            put(payroll::update_entry).delete(payroll::delete_entry),
        )
        .route("/payroll/overview", get(payroll::overview))
        .route("/payroll/runs", get(payroll::list_runs))
        .route(
            "/payroll/runs/{id}",
            get(payroll::get_run).delete(payroll::delete_run),
        )
        .route("/payroll/runs/{id}/cancel", put(payroll::cancel_run))
        .route(
            "/payroll/runs/{id}/payment-file",
            get(payroll::download_payment_file),
        )
        .route(
            "/payroll/runs/{id}/journal-preview",
            get(payroll::journal_preview),
        )
        .route(
            "/payroll/runs/{id}/audit-logs",
            get(payroll::list_run_audit_logs),
        )
        .route("/payroll/runs/{id}/items", get(payroll::get_items))
        .route(
            "/payroll/runs/{run_id}/items/{employee_id}/pcb",
            put(payroll::update_item_pcb),
        )
        .route(
            "/payroll/runs/{run_id}/items/{employee_id}/breakdown",
            get(payroll::get_payslip_breakdown),
        )
        .route(
            "/payroll/runs/{id}/submit-approval",
            put(payroll::submit_run_for_approval),
        )
        .route("/payroll/runs/{id}/approve", put(payroll::approve_run))
        .route(
            "/payroll/runs/{id}/return",
            put(payroll::return_run_for_changes),
        )
        .route("/payroll/runs/{id}/lock", put(payroll::lock_run))
        .route("/payroll/runs/{id}/reverse", put(payroll::reverse_run))
        // Background jobs — the 202 status poll for /payroll/run,
        // /employees/import/confirm and any future detached work.
        .route("/jobs/{id}", get(job::status))
        // Documents (static routes before {id})
        .route("/documents", get(document::list).post(document::create))
        .route(
            "/documents/categories",
            get(document::list_categories).post(document::create_category),
        )
        .route("/documents/expiring", get(document::expiring))
        .route(
            "/documents/{id}",
            get(document::get)
                .put(document::update)
                .delete(document::delete),
        )
        // Settings
        .route("/settings", get(settings::list).put(settings::bulk_update))
        .route(
            "/settings/{category}/{key}",
            get(settings::get).put(settings::update),
        )
        // Company
        .route("/company", get(company::get).put(company::update))
        .route("/company/stats", get(company::stats))
        // Employee Portal (self-service)
        .route("/portal/profile", get(portal::get_profile))
        .route("/portal/payslips", get(portal::list_payslips))
        .route("/portal/leave/types", get(portal::leave_types))
        .route("/portal/leave/balances", get(portal::leave_balances))
        .route(
            "/portal/leave/requests",
            get(portal::leave_requests).post(portal::create_leave),
        )
        .route("/portal/leave/requests/{id}", delete(portal::delete_leave))
        .route(
            "/portal/leave/requests/{id}/cancel",
            put(portal::cancel_leave),
        )
        .route("/portal/teams", get(portal::my_teams))
        .route("/portal/team-calendar", get(portal::team_calendar))
        .route("/portal/holidays", get(portal::portal_holidays))
        .route("/portal/working-days", get(portal::portal_working_days))
        .route(
            "/portal/claims",
            get(portal::list_claims).post(portal::create_claim),
        )
        .route("/portal/claims/{id}/submit", put(portal::submit_claim))
        .route("/portal/claims/{id}/cancel", put(portal::cancel_claim))
        .route("/portal/claims/{id}", delete(portal::delete_claim))
        // Overtime (portal)
        .route(
            "/portal/overtime",
            get(portal::list_overtime).post(portal::create_overtime),
        )
        .route("/portal/overtime/{id}/cancel", put(portal::cancel_overtime))
        .route("/portal/overtime/{id}", delete(portal::delete_overtime))
        // POST /uploads is rate-limited above (with the raised body limit).
        .route("/uploads/{filename}", get(portal::serve_upload))
        // Dashboard
        .route("/dashboard/summary", get(dashboard::summary))
        // Notifications
        .route("/notifications", get(notification::list))
        .route("/notifications/count", get(notification::count))
        .route("/notifications/read-all", put(notification::mark_all_read))
        .route("/notifications/{id}/read", put(notification::mark_read))
        .route("/notifications/push-config", get(notification::push_config))
        // Approval Workflows (admin)
        .route("/approvals/pending-count", get(approval::pending_counts))
        .route(
            "/approvals/leave",
            get(approval::list_leave_requests).post(approval::create_leave_request),
        )
        .route(
            "/approvals/leave/{id}",
            put(approval::update_leave_request).delete(approval::delete_leave_request),
        )
        .route(
            "/approvals/leave/{id}/cancel",
            put(approval::cancel_leave_request),
        )
        .route(
            "/approvals/leave/{id}/approve",
            put(approval::approve_leave),
        )
        .route("/approvals/leave/{id}/reject", put(approval::reject_leave))
        .route(
            "/approvals/claims",
            get(approval::list_claims).post(approval::create_claim),
        )
        .route(
            "/approvals/claims/{id}",
            put(approval::update_claim).delete(approval::delete_claim),
        )
        .route("/approvals/claims/{id}/cancel", put(approval::cancel_claim))
        .route(
            "/approvals/claims/{id}/approve",
            put(approval::approve_claim),
        )
        .route("/approvals/claims/{id}/reject", put(approval::reject_claim))
        // Overtime approvals (admin)
        .route(
            "/approvals/overtime",
            get(approval::list_overtime).post(approval::create_overtime),
        )
        .route(
            "/approvals/overtime/{id}",
            put(approval::update_overtime).delete(approval::delete_overtime),
        )
        .route(
            "/approvals/overtime/{id}/cancel",
            put(approval::cancel_overtime),
        )
        .route(
            "/approvals/overtime/{id}/approve",
            put(approval::approve_overtime),
        )
        .route(
            "/approvals/overtime/{id}/reject",
            put(approval::reject_overtime),
        )
        // Calendar (/import-ics and /import-ics-file are rate-limited above)
        .route(
            "/calendar/holidays",
            get(calendar::list_holidays).post(calendar::create_holiday),
        )
        .route(
            "/calendar/holidays/{id}",
            get(calendar::get_holiday)
                .put(calendar::update_holiday)
                .delete(calendar::delete_holiday),
        )
        .route(
            "/calendar/working-days",
            get(calendar::get_working_days).put(calendar::update_working_days),
        )
        .route("/calendar/month", get(calendar::get_month_calendar))
        // Teams (admin)
        .route("/teams", get(team::list_teams).post(team::create_team))
        .route(
            "/teams/{id}",
            get(team::get_team)
                .put(team::update_team)
                .delete(team::delete_team),
        )
        .route(
            "/teams/{id}/members",
            get(team::list_members).post(team::add_member),
        )
        .route(
            "/teams/{team_id}/members/{employee_id}",
            delete(team::remove_member),
        )
        // Reports (admin)
        .route("/reports/periods", get(report::report_periods))
        .route("/reports/payroll-summary", get(report::payroll_summary))
        .route(
            "/reports/payroll-department",
            get(report::payroll_by_department),
        )
        .route("/reports/leave", get(report::leave_report))
        .route("/reports/claims", get(report::claims_report))
        .route("/reports/statutory", get(report::statutory_report))
        // Email / Letters
        .route(
            "/email/templates",
            get(email::list_templates).post(email::create_template),
        )
        .route(
            "/email/templates/{id}",
            get(email::get_template)
                .put(email::update_template)
                .delete(email::delete_template),
        )
        .route("/email/preview", post(email::preview_letter))
        // POST /email/send is rate-limited above.
        .route("/email/logs", get(email::list_email_logs))
        // Audit Trail
        .route("/audit-logs", get(audit::list_audit_logs))
        // Filter vocabulary, read from the company's own rows so the dropdowns
        // cannot offer an option that matches nothing.
        .route("/audit-logs/filters", get(audit::list_filter_options))
        // Leave enhancements
        .route(
            "/employees/{id}/leave-balances/initialize",
            post(employee::initialize_balances),
        )
        .route("/leave/year-end", post(employee::process_carry_forward))
        // Portal: leave ICS export & payslip PDF
        .route("/portal/leave/export-ics", get(portal::export_leave_ics))
        .route(
            "/portal/payslips/{id}/pdf",
            get(portal::download_payslip_pdf),
        )
        // Payroll: bulk payslip PDF
        .route(
            "/payroll/runs/{run_id}/payslips/pdf",
            get(payroll::download_run_payslips_pdf),
        )
        // Statutory file exports
        .route("/reports/statutory/epf-export", get(report::export_epf))
        .route("/reports/statutory/socso-export", get(report::export_socso))
        .route("/reports/statutory/eis-export", get(report::export_eis))
        .route("/reports/statutory/pcb-export", get(report::export_pcb))
        // EA Form
        .route("/reports/ea-form/employees", get(report::list_ea_employees))
        .route("/reports/ea-form", get(report::get_ea_form))
        // ─── Attendance ───
        // Platform method control (super_admin only)
        .route(
            "/admin/platform/attendance-method",
            get(attendance::get_platform_method).put(attendance::set_platform_method),
        )
        // Effective method for caller's company (any auth)
        .route("/attendance/method", get(attendance::get_attendance_method))
        // Company-level override (admin)
        .route(
            "/attendance/company-method",
            put(attendance::set_company_method),
        )
        // QR token generation (admin/hr_manager)
        .route(
            "/attendance/qr/generate",
            post(attendance::generate_qr_token),
        )
        // Kiosk credentials (admin) — list/create + revoke. The public endpoint
        // /attendance/kiosk/qr lives in `rate_limited_kiosk` above.
        .route(
            "/attendance/kiosks",
            get(attendance::list_kiosk_credentials).post(attendance::create_kiosk_credential),
        )
        .route(
            "/attendance/kiosks/{id}",
            delete(attendance::revoke_kiosk_credential),
        )
        // Employee portal (check-in/out routes rate-limited above)
        .route("/attendance/my/today", get(attendance::my_today))
        .route("/attendance/my", get(attendance::my_attendance))
        // Summary & export (admin)
        .route("/attendance/summary", get(attendance::attendance_summary))
        .route("/attendance/export", get(attendance::export_attendance))
        // Admin management
        .route("/attendance/records", get(attendance::list_attendance))
        .route(
            "/attendance/records/{id}",
            put(attendance::update_attendance),
        )
        .route("/attendance/manual", post(attendance::manual_attendance))
        .route("/attendance/absent-run", post(attendance::absent_run))
        // ─── Work Schedules ───
        .route("/work-schedules", get(work_schedule::list_schedules))
        .route(
            "/work-schedules/default",
            get(work_schedule::get_default_schedule).put(work_schedule::upsert_default_schedule),
        )
        .route("/work-schedules/{id}", put(work_schedule::update_schedule))
        // ─── Geofencing ───
        .route(
            "/geofence/locations",
            get(geofence::list_locations).post(geofence::create_location),
        )
        .route(
            "/geofence/locations/{id}",
            put(geofence::update_location).delete(geofence::delete_location),
        )
        .route(
            "/geofence/mode",
            get(geofence::get_mode).put(geofence::set_mode),
        )
        // ─── Attendance networks ───
        // Every route gated on ManageAttendanceNetworks inside the handler.
        .route(
            "/attendance/networks",
            get(attendance_network::list_networks).post(attendance_network::create_network),
        )
        .route(
            "/attendance/networks/{id}",
            put(attendance_network::update_network).delete(attendance_network::delete_network),
        )
        .route(
            "/attendance/networks/mode",
            get(attendance_network::get_mode).put(attendance_network::set_mode),
        )
        .route(
            "/attendance/networks/candidates",
            get(attendance_network::list_candidates),
        )
        .route(
            "/attendance/networks/candidates/approve",
            post(attendance_network::approve_candidate),
        )
        .route(
            "/attendance/networks/candidates/dismiss",
            post(attendance_network::dismiss_candidate),
        )
        .route(
            "/attendance/networks/whoami",
            get(attendance_network::whoami),
        );

    Router::new().nest("/api", api).with_state(state)
}
