use std::time::Duration;

use axum::{
    Router,
    routing::{delete, get, post, put},
};
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};

use crate::core::app_state::AppState;
use crate::handlers::{
    admin, approval, attendance, audit, auth, backup, calendar, company, dashboard, document,
    email, employee, employee_import, geofence, health, notification, oauth2, passkey, payroll,
    portal, report, settings, team, work_schedule,
};

pub fn create_router(state: AppState) -> Router {
    // Rate limiter: 5 requests per 60 seconds per IP
    let auth_rate_limit = GovernorConfigBuilder::default()
        .per_second(12)
        .burst_size(5)
        .finish()
        .expect("Failed to build rate limiter");

    // Rate limiter: 3 requests per 60 seconds per IP (stricter for forgot-password)
    let forgot_rate_limit = GovernorConfigBuilder::default()
        .period(Duration::from_secs(60))
        .burst_size(3)
        .finish()
        .expect("Failed to build rate limiter");

    // Rate limiter: 10 requests per 60 seconds per IP (OAuth2 flow)
    let oauth2_rate_limit = GovernorConfigBuilder::default()
        .per_second(6)
        .burst_size(10)
        .finish()
        .expect("Failed to build rate limiter");

    // Public kiosk QR endpoint (~120/min/IP — comfortable for a tablet refreshing
    // every minute, far below what a guesser would need to be effective against the
    // ≥244-bit secret space).
    let kiosk_rate_limit = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(10)
        .finish()
        .expect("Failed to build rate limiter");

    // Rate-limited auth routes
    let rate_limited_auth = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/reset-password", post(auth::reset_password))
        .layer(GovernorLayer::new(auth_rate_limit));

    let rate_limited_forgot = Router::new()
        .route("/auth/forgot-password", post(auth::forgot_password))
        .layer(GovernorLayer::new(forgot_rate_limit));

    let rate_limited_oauth2 = Router::new()
        .route(
            "/auth/oauth2/google/authorize",
            get(oauth2::google_authorize),
        )
        .route("/auth/oauth2/google/callback", get(oauth2::google_callback))
        .layer(GovernorLayer::new(oauth2_rate_limit));

    let rate_limited_kiosk = Router::new()
        .route("/attendance/kiosk/qr", post(attendance::kiosk_qr))
        .layer(GovernorLayer::new(kiosk_rate_limit));

    let api = Router::new()
        // Health check (no auth required, used by ALB)
        .route("/health", get(|| async { "ok" }))
        // Readiness probe: checks DB connectivity and migration state.
        .route("/health/ready", get(health::ready))
        .merge(rate_limited_auth)
        .merge(rate_limited_forgot)
        .merge(rate_limited_oauth2)
        .merge(rate_limited_kiosk)
        // Auth (non-rate-limited)
        .route("/auth/me", get(auth::me))
        .route("/auth/refresh", post(auth::refresh_token))
        .route("/auth/logout", post(auth::logout))
        .route(
            "/auth/validate-reset-token",
            post(auth::validate_reset_token),
        )
        .route("/auth/change-password", put(auth::change_password))
        .route(
            "/auth/skip-change-password",
            put(auth::skip_change_password),
        )
        .route("/auth/switch-company", put(auth::switch_company))
        .route("/auth/my-companies", get(auth::my_companies))
        // Passkey (WebAuthn) — unauthenticated
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
        // Passkey (WebAuthn) — authenticated
        .route(
            "/auth/passkey/register/begin",
            post(passkey::registration_begin),
        )
        .route(
            "/auth/passkey/register/complete",
            post(passkey::registration_complete),
        )
        .route("/auth/passkeys", get(passkey::list_passkeys))
        .route(
            "/auth/passkeys/{id}",
            put(passkey::rename_passkey).delete(passkey::delete_passkey),
        )
        // OAuth2 (non-rate-limited routes)
        .route("/auth/oauth2/providers", get(oauth2::list_providers))
        .route("/auth/oauth2/google/link", post(oauth2::link_google))
        .route("/auth/oauth2/accounts", get(oauth2::my_linked_accounts))
        .route(
            "/auth/oauth2/accounts/{provider}",
            delete(oauth2::unlink_provider),
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
        .route(
            "/admin/users/{id}/companies",
            put(admin::update_user_companies),
        )
        // Backup / Data Migration
        .route("/admin/backup/export", get(backup::export_company))
        .route("/admin/backup/import", post(backup::import_company))
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
        // Employee Import
        .route(
            "/employees/import/template",
            get(employee_import::download_template),
        )
        .route(
            "/employees/import/validate",
            post(employee_import::validate_import),
        )
        .route(
            "/employees/import/confirm",
            post(employee_import::confirm_import),
        )
        // Payroll Groups
        .route("/payroll-groups", get(payroll::list_groups))
        // Payroll Runs
        .route("/payroll/run", post(payroll::process))
        .route(
            "/payroll/entries",
            get(payroll::list_entries).post(payroll::create_entry),
        )
        .route(
            "/payroll/entries/{id}",
            put(payroll::update_entry).delete(payroll::delete_entry),
        )
        .route("/payroll/runs", get(payroll::list_runs))
        .route(
            "/payroll/runs/{id}",
            get(payroll::get_run).delete(payroll::delete_run),
        )
        .route("/payroll/runs/{id}/items", get(payroll::get_items))
        .route(
            "/payroll/runs/{run_id}/items/{employee_id}/pcb",
            put(payroll::update_item_pcb),
        )
        .route("/payroll/runs/{id}/approve", put(payroll::approve_run))
        .route("/payroll/runs/{id}/lock", put(payroll::lock_run))
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
        // File uploads
        .route("/uploads", post(portal::upload_file))
        .route("/uploads/{filename}", get(portal::serve_upload))
        // Dashboard
        .route("/dashboard/summary", get(dashboard::summary))
        // Notifications
        .route("/notifications", get(notification::list))
        .route("/notifications/count", get(notification::count))
        .route("/notifications/read-all", put(notification::mark_all_read))
        .route("/notifications/{id}/read", put(notification::mark_read))
        // Approval Workflows (admin)
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
        // Calendar (admin)
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
        .route("/calendar/import-ics", post(calendar::import_ics))
        .route("/calendar/import-ics-file", post(calendar::import_ics_file))
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
        .route("/email/send", post(email::send_letter))
        .route("/email/logs", get(email::list_email_logs))
        // Audit Trail
        .route("/audit-logs", get(audit::list_audit_logs))
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
        // Employee check-in
        .route("/attendance/check-in/qr", post(attendance::check_in_qr))
        .route(
            "/attendance/check-in/face-id",
            post(attendance::check_in_face_id),
        )
        .route("/attendance/check-out", post(attendance::check_out))
        // Employee portal
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
        );

    Router::new().nest("/api", api).with_state(state)
}
