//! The single definition of what each role may do.
//!
//! Authorization used to live in two incompatible shapes: a `Permission` enum
//! covering five payroll actions, and roughly sixteen ad-hoc role allow-lists —
//! some methods on `AuthUser`, some private `require_admin` helpers copied into
//! seven different handler modules, each with a slightly different roster. The
//! frontend then re-typed the same rules a third time in `lib/roles.ts`, and the
//! Role Management screen a fourth time as a hand-written table that had already
//! drifted out of agreement with all three.
//!
//! Everything now resolves through [`Permission`] and [`role_permissions`], and
//! the frontend reads the matrix from `GET /api/auth/permissions/matrix` rather
//! than restating it. Adding a role means adding one row here.
//!
//! The grants below reproduce the behaviour the API had before this module
//! existed, quirks included — this is a refactor of *where* the rules live, not
//! a change to who can do what. Two exceptions are called out inline where the
//! previous code was unreachable rather than merely odd.

use serde::Serialize;

/// A single capability. Handlers gate on these, never on role strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // ─── Payroll ───
    ViewPayroll,
    ManagePayrollDraft,
    SubmitPayroll,
    ApprovePayroll,
    MarkPayrollPaid,

    // ─── Employees ───
    ViewEmployees,
    ManageEmployees,
    ImportEmployees,
    ViewSalaryHistory,

    // ─── Attendance ───
    ViewAttendance,
    ManageAttendance,
    GenerateAttendanceQr,
    ManageKiosks,
    SetCompanyAttendanceMethod,
    ManageAttendanceNetworks,

    // ─── Teams ───
    ViewTeams,
    ManageTeams,

    // ─── Calendar ───
    ViewCalendar,
    ManageCalendar,

    // ─── Leave / claims / overtime approvals ───
    ViewApprovals,
    ManageApprovals,

    // ─── Reports ───
    ViewReports,
    ViewStatutoryExports,

    // ─── Documents ───
    ViewDocuments,
    ManageDocuments,

    // ─── Email / letters ───
    ViewEmailLogs,
    ManageEmailTemplates,
    SendLetters,

    // ─── Company configuration ───
    ManageCompanySettings,
    ManageWorkSchedules,
    ManageGeofence,

    // ─── Platform administration ───
    ViewUserDirectory,
    ManageUsers,
    ManageCompanies,
    ViewAuditLog,
    ManageBackups,
    ManagePlatformSettings,
}

impl Permission {
    /// Every permission, in declaration order. Drives the matrix endpoint, so
    /// a new variant appears in the frontend without a second edit there.
    pub const ALL: &'static [Permission] = &[
        Permission::ViewPayroll,
        Permission::ManagePayrollDraft,
        Permission::SubmitPayroll,
        Permission::ApprovePayroll,
        Permission::MarkPayrollPaid,
        Permission::ViewEmployees,
        Permission::ManageEmployees,
        Permission::ImportEmployees,
        Permission::ViewSalaryHistory,
        Permission::ViewAttendance,
        Permission::ManageAttendance,
        Permission::GenerateAttendanceQr,
        Permission::ManageKiosks,
        Permission::SetCompanyAttendanceMethod,
        Permission::ManageAttendanceNetworks,
        Permission::ViewTeams,
        Permission::ManageTeams,
        Permission::ViewCalendar,
        Permission::ManageCalendar,
        Permission::ViewApprovals,
        Permission::ManageApprovals,
        Permission::ViewReports,
        Permission::ViewStatutoryExports,
        Permission::ViewDocuments,
        Permission::ManageDocuments,
        Permission::ViewEmailLogs,
        Permission::ManageEmailTemplates,
        Permission::SendLetters,
        Permission::ManageCompanySettings,
        Permission::ManageWorkSchedules,
        Permission::ManageGeofence,
        Permission::ViewUserDirectory,
        Permission::ManageUsers,
        Permission::ManageCompanies,
        Permission::ViewAuditLog,
        Permission::ManageBackups,
        Permission::ManagePlatformSettings,
    ];

    /// Stable wire identifier. Matches the `serde` representation; the two are
    /// kept in agreement by `permission_wire_names_match_serde` below.
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::ViewPayroll => "view_payroll",
            Permission::ManagePayrollDraft => "manage_payroll_draft",
            Permission::SubmitPayroll => "submit_payroll",
            Permission::ApprovePayroll => "approve_payroll",
            Permission::MarkPayrollPaid => "mark_payroll_paid",
            Permission::ViewEmployees => "view_employees",
            Permission::ManageEmployees => "manage_employees",
            Permission::ImportEmployees => "import_employees",
            Permission::ViewSalaryHistory => "view_salary_history",
            Permission::ViewAttendance => "view_attendance",
            Permission::ManageAttendance => "manage_attendance",
            Permission::GenerateAttendanceQr => "generate_attendance_qr",
            Permission::ManageKiosks => "manage_kiosks",
            Permission::SetCompanyAttendanceMethod => "set_company_attendance_method",
            Permission::ManageAttendanceNetworks => "manage_attendance_networks",
            Permission::ViewTeams => "view_teams",
            Permission::ManageTeams => "manage_teams",
            Permission::ViewCalendar => "view_calendar",
            Permission::ManageCalendar => "manage_calendar",
            Permission::ViewApprovals => "view_approvals",
            Permission::ManageApprovals => "manage_approvals",
            Permission::ViewReports => "view_reports",
            Permission::ViewStatutoryExports => "view_statutory_exports",
            Permission::ViewDocuments => "view_documents",
            Permission::ManageDocuments => "manage_documents",
            Permission::ViewEmailLogs => "view_email_logs",
            Permission::ManageEmailTemplates => "manage_email_templates",
            Permission::SendLetters => "send_letters",
            Permission::ManageCompanySettings => "manage_company_settings",
            Permission::ManageWorkSchedules => "manage_work_schedules",
            Permission::ManageGeofence => "manage_geofence",
            Permission::ViewUserDirectory => "view_user_directory",
            Permission::ManageUsers => "manage_users",
            Permission::ManageCompanies => "manage_companies",
            Permission::ViewAuditLog => "view_audit_log",
            Permission::ManageBackups => "manage_backups",
            Permission::ManagePlatformSettings => "manage_platform_settings",
        }
    }

    /// Human-readable label. Lives here rather than in the frontend so the two
    /// cannot describe the same permission differently.
    pub fn label(self) -> &'static str {
        match self {
            Permission::ViewPayroll => "View payroll",
            Permission::ManagePayrollDraft => "Prepare payroll drafts",
            Permission::SubmitPayroll => "Submit payroll for approval",
            Permission::ApprovePayroll => "Approve payroll",
            Permission::MarkPayrollPaid => "Mark payroll paid",
            Permission::ViewEmployees => "View employees",
            Permission::ManageEmployees => "Manage employees",
            Permission::ImportEmployees => "Bulk-import employees",
            Permission::ViewSalaryHistory => "View salary history",
            Permission::ViewAttendance => "View attendance",
            Permission::ManageAttendance => "Correct attendance records",
            Permission::GenerateAttendanceQr => "Generate attendance QR codes",
            Permission::ManageKiosks => "Manage kiosk credentials",
            Permission::SetCompanyAttendanceMethod => "Set company attendance method",
            Permission::ManageAttendanceNetworks => "Approve attendance office networks",
            Permission::ViewTeams => "View teams",
            Permission::ManageTeams => "Manage teams",
            Permission::ViewCalendar => "View calendar",
            Permission::ManageCalendar => "Manage calendar & holidays",
            Permission::ViewApprovals => "View leave / claims / overtime",
            Permission::ManageApprovals => "Approve leave / claims / overtime",
            Permission::ViewReports => "View reports",
            Permission::ViewStatutoryExports => "View statutory exports",
            Permission::ViewDocuments => "View documents",
            Permission::ManageDocuments => "Manage documents",
            Permission::ViewEmailLogs => "View email logs",
            Permission::ManageEmailTemplates => "Manage email templates",
            Permission::SendLetters => "Send letters",
            Permission::ManageCompanySettings => "Manage company settings",
            Permission::ManageWorkSchedules => "Manage work schedules",
            Permission::ManageGeofence => "Manage geofencing",
            Permission::ViewUserDirectory => "View user directory",
            Permission::ManageUsers => "Manage users",
            Permission::ManageCompanies => "Manage companies",
            Permission::ViewAuditLog => "View audit trail",
            Permission::ManageBackups => "Export & import company data",
            Permission::ManagePlatformSettings => "Manage platform settings",
        }
    }

    /// Grouping used to lay the matrix out in the UI.
    pub fn group(self) -> &'static str {
        match self {
            Permission::ViewPayroll
            | Permission::ManagePayrollDraft
            | Permission::SubmitPayroll
            | Permission::ApprovePayroll
            | Permission::MarkPayrollPaid => "Payroll",
            Permission::ViewEmployees
            | Permission::ManageEmployees
            | Permission::ImportEmployees
            | Permission::ViewSalaryHistory => "Employees",
            Permission::ViewAttendance
            | Permission::ManageAttendance
            | Permission::GenerateAttendanceQr
            | Permission::ManageKiosks
            | Permission::SetCompanyAttendanceMethod
            | Permission::ManageAttendanceNetworks => "Attendance",
            Permission::ViewTeams | Permission::ManageTeams => "Teams",
            Permission::ViewCalendar | Permission::ManageCalendar => "Calendar",
            Permission::ViewApprovals | Permission::ManageApprovals => "Approvals",
            Permission::ViewReports | Permission::ViewStatutoryExports => "Reports",
            Permission::ViewDocuments | Permission::ManageDocuments => "Documents",
            Permission::ViewEmailLogs
            | Permission::ManageEmailTemplates
            | Permission::SendLetters => "Email & letters",
            Permission::ManageCompanySettings
            | Permission::ManageWorkSchedules
            | Permission::ManageGeofence => "Company configuration",
            Permission::ViewUserDirectory
            | Permission::ManageUsers
            | Permission::ManageCompanies
            | Permission::ViewAuditLog
            | Permission::ManageBackups
            | Permission::ManagePlatformSettings => "Platform administration",
        }
    }

    /// Capabilities reserved strictly for platform `super_admin`s and never
    /// delegable via company-scoped user groups.
    pub fn is_platform_only(self) -> bool {
        matches!(
            self,
            Permission::ManageUsers
                | Permission::ManageCompanies
                | Permission::ManageBackups
                | Permission::ManagePlatformSettings
        )
    }

    /// Capabilities that expose or manipulate payroll / statutory figures
    /// (including `ImportEmployees`, which writes salaries and bank details).
    pub fn is_payroll_capability(self) -> bool {
        matches!(
            self,
            Permission::ViewPayroll
                | Permission::ManagePayrollDraft
                | Permission::SubmitPayroll
                | Permission::ApprovePayroll
                | Permission::MarkPayrollPaid
                | Permission::ImportEmployees
                | Permission::ViewStatutoryExports
                | Permission::ViewSalaryHistory
        )
    }

    /// Capabilities that mutate state or perform privileged actions (as opposed
    /// to read-only `View*` capabilities). Used to keep `exec` read-only.
    pub fn is_write_capability(self) -> bool {
        !matches!(
            self,
            Permission::ViewPayroll
                | Permission::ViewEmployees
                | Permission::ViewSalaryHistory
                | Permission::ViewAttendance
                | Permission::ViewTeams
                | Permission::ViewCalendar
                | Permission::ViewApprovals
                | Permission::ViewReports
                | Permission::ViewStatutoryExports
                | Permission::ViewDocuments
                | Permission::ViewEmailLogs
                | Permission::ViewUserDirectory
                | Permission::ViewAuditLog
        )
    }
}

/// Filters company-scoped `user_group` permission keys consistently across both
/// request authentication (`AuthUser`) and login/profile responses (`UserResponse`).
pub fn filter_group_permissions_for_roles(roles: &[String], keys: &[String]) -> Vec<Permission> {
    let has_restricted_payroll = roles
        .iter()
        .any(|r| r == "exec" || r == "employee" || r == "manager");
    let is_exec_only =
        roles.iter().any(|r| r == "exec") && !roles.iter().any(|r| r != "exec" && r != "employee");

    keys.iter()
        .filter_map(|key| Permission::ALL.iter().find(|p| p.as_str() == key).copied())
        .filter(|p| !p.is_platform_only())
        .filter(|p| !has_restricted_payroll || !p.is_payroll_capability())
        .filter(|p| !is_exec_only || !p.is_write_capability())
        .collect()
}

/// Every role string the `users_roles_valid` CHECK constraint accepts, in
/// descending order of privilege. Keep in step with the constraint in
/// `1000_schema.sql`; `schema_invariant_tests` asserts they agree.
pub const ALL_ROLES: &[&str] = &[
    "super_admin",
    "admin",
    "manager",
    "payroll_admin",
    "hr_manager",
    "finance",
    "exec",
    "employee",
];

use Permission as P;

/// Platform owner. Passes every gate in the API.
const SUPER_ADMIN: &[Permission] = P::ALL;

/// Company administrator. Note the absence of every payroll permission: the
/// old `can()` never granted `admin` a payroll capability, so this role has
/// always been unable to read payroll figures despite the Role Management
/// screen claiming otherwise.
const ADMIN: &[Permission] = &[
    P::ViewEmployees,
    P::ManageEmployees,
    P::ViewAttendance,
    P::ManageAttendance,
    P::GenerateAttendanceQr,
    P::ManageKiosks,
    P::SetCompanyAttendanceMethod,
    P::ManageAttendanceNetworks,
    P::ViewTeams,
    P::ManageTeams,
    P::ViewCalendar,
    P::ManageCalendar,
    P::ViewApprovals,
    P::ManageApprovals,
    P::ViewReports,
    P::ViewDocuments,
    P::ManageDocuments,
    P::ViewEmailLogs,
    P::ManageEmailTemplates,
    P::SendLetters,
    P::ManageCompanySettings,
    P::ManageWorkSchedules,
    P::ManageGeofence,
    P::ViewUserDirectory,
    P::ViewAuditLog,
];

/// Company manager: every company-management capability except payroll, and
/// excluding the platform-only set that only `super_admin` holds. Also
/// excludes `ImportEmployees`, because a bulk import writes `basic_salary` and
/// bank details with no payroll-privilege check, and `ViewAuditLog`: the audit
/// trail records payroll, settings and user-administration activity that a
/// company manager is not meant to review.
///
/// The second half of the role is a visibility rule, not a permission: a
/// caller who holds neither `admin` nor `super_admin` is never shown users who
/// hold those roles in the user directory or group member lists (see
/// `user_service::hides_privileged_users`).
const MANAGER: &[Permission] = &[
    P::ViewEmployees,
    P::ManageEmployees,
    P::ViewAttendance,
    P::ManageAttendance,
    P::GenerateAttendanceQr,
    P::ManageKiosks,
    P::SetCompanyAttendanceMethod,
    P::ManageAttendanceNetworks,
    P::ViewTeams,
    P::ManageTeams,
    P::ViewCalendar,
    P::ManageCalendar,
    P::ViewApprovals,
    P::ManageApprovals,
    P::ViewReports,
    P::ViewDocuments,
    P::ManageDocuments,
    P::ViewEmailLogs,
    P::ManageEmailTemplates,
    P::SendLetters,
    P::ManageCompanySettings,
    P::ManageWorkSchedules,
    P::ManageGeofence,
    P::ViewUserDirectory,
];

/// Prepares payroll but cannot approve it — the separation of duties the
/// payroll lifecycle depends on.
const PAYROLL_ADMIN: &[Permission] = &[
    P::ViewPayroll,
    P::ManagePayrollDraft,
    P::SubmitPayroll,
    P::ViewEmployees,
    P::ManageEmployees,
    P::ImportEmployees,
    P::ViewSalaryHistory,
    P::ViewAttendance,
    P::GenerateAttendanceQr,
    P::ManageKiosks,
    P::ViewTeams,
    P::ManageTeams,
    P::ViewCalendar,
    P::ManageCalendar,
    P::ViewApprovals,
    P::ManageApprovals,
    P::ViewReports,
    P::ViewStatutoryExports,
    P::ViewDocuments,
    P::ManageDocuments,
    P::ViewEmailLogs,
    P::ManageEmailTemplates,
    P::SendLetters,
];

/// HR operations. No payroll access at all.
const HR_MANAGER: &[Permission] = &[
    P::ViewEmployees,
    P::ManageEmployees,
    P::ViewAttendance,
    P::ManageAttendance,
    P::GenerateAttendanceQr,
    P::ManageKiosks,
    P::ManageAttendanceNetworks,
    P::ViewTeams,
    P::ManageTeams,
    P::ViewCalendar,
    P::ManageCalendar,
    P::ViewApprovals,
    P::ManageApprovals,
    P::ViewReports,
    P::ViewDocuments,
    P::ManageDocuments,
    P::ViewEmailLogs,
    P::ManageEmailTemplates,
    P::SendLetters,
    P::ManageWorkSchedules,
    P::ManageGeofence,
];

/// Approves and pays payroll it did not prepare. Deliberately excluded from
/// the approvals, calendar and teams surfaces, matching the previous
/// allow-lists in `approval.rs`, `calendar.rs` and `team.rs`.
const FINANCE: &[Permission] = &[
    P::ViewPayroll,
    P::ApprovePayroll,
    P::MarkPayrollPaid,
    P::ViewEmployees,
    P::ViewSalaryHistory,
    P::ViewAttendance,
    P::ViewReports,
    P::ViewStatutoryExports,
    P::ViewDocuments,
    P::ManageDocuments,
    P::ViewEmailLogs,
    P::ManageEmailTemplates,
    P::SendLetters,
];

/// Read-mostly company overview. Never payroll and holds no write capabilities.
const EXEC: &[Permission] = &[
    P::ViewEmployees,
    P::ViewAttendance,
    P::ViewTeams,
    P::ViewCalendar,
    P::ViewApprovals,
    P::ViewDocuments,
    P::ViewEmailLogs,
];

/// Self-service only. Portal routes authorize on the caller's own
/// `employee_id`, not on a permission, so this set is empty by design.
const EMPLOYEE: &[Permission] = &[];

/// The permissions a single role grants.
pub fn role_permissions(role: &str) -> &'static [Permission] {
    match role {
        "super_admin" => SUPER_ADMIN,
        "admin" => ADMIN,
        "manager" => MANAGER,
        "payroll_admin" => PAYROLL_ADMIN,
        "hr_manager" => HR_MANAGER,
        "finance" => FINANCE,
        "exec" => EXEC,
        "employee" => EMPLOYEE,
        // An unknown role grants nothing. The DB CHECK constraint makes this
        // unreachable for stored users, but a token minted before a role was
        // removed must fail closed rather than panic.
        _ => &[],
    }
}

/// Whether any of `roles` grants `permission`. Roles are additive: a user
/// holding several gets the union.
pub fn roles_grant(roles: &[String], permission: Permission) -> bool {
    roles
        .iter()
        .any(|role| role_permissions(role).contains(&permission))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn perms(role: &str) -> HashSet<Permission> {
        role_permissions(role).iter().copied().collect()
    }

    #[test]
    fn all_lists_every_variant_exactly_once() {
        let unique: HashSet<_> = Permission::ALL.iter().copied().collect();
        assert_eq!(
            unique.len(),
            Permission::ALL.len(),
            "Permission::ALL contains a duplicate"
        );
        // Every role's grants must be drawn from ALL; catches a variant added
        // to the enum and to a role but forgotten in ALL, which would hide it
        // from the matrix endpoint and therefore from the UI.
        for role in ALL_ROLES {
            for granted in role_permissions(role) {
                assert!(
                    Permission::ALL.contains(granted),
                    "{role} grants {granted:?}, which is missing from Permission::ALL"
                );
            }
        }
    }

    #[test]
    fn permission_wire_names_match_serde() {
        for permission in Permission::ALL {
            let json = serde_json::to_string(permission).expect("serialize permission");
            assert_eq!(
                json,
                format!("\"{}\"", permission.as_str()),
                "as_str() and the serde representation disagree for {permission:?}"
            );
        }
    }

    #[test]
    fn no_role_grants_a_duplicate() {
        for role in ALL_ROLES {
            let granted = role_permissions(role);
            let unique: HashSet<_> = granted.iter().copied().collect();
            assert_eq!(unique.len(), granted.len(), "{role} lists a duplicate");
        }
    }

    #[test]
    fn super_admin_is_a_superset_of_every_other_role() {
        let all = perms("super_admin");
        for role in ALL_ROLES.iter().filter(|r| **r != "super_admin") {
            assert!(
                perms(role).is_subset(&all),
                "super_admin must pass every gate {role} does"
            );
        }
    }

    /// The `exec` role is read-mostly and must never see payroll figures. This
    /// was previously an emergent property of `can()` having no `exec` branch;
    /// making it an explicit test means a future edit to the table cannot undo
    /// it silently.
    #[test]
    fn exec_holds_no_payroll_permission() {
        let payroll = [
            P::ViewPayroll,
            P::ManagePayrollDraft,
            P::SubmitPayroll,
            P::ApprovePayroll,
            P::MarkPayrollPaid,
            P::ViewStatutoryExports,
            P::ViewSalaryHistory,
        ];
        for permission in payroll {
            assert!(
                !role_permissions("exec").contains(&permission),
                "exec must not hold {permission:?}"
            );
        }
    }

    #[test]
    fn exec_holds_no_write_or_platform_permission() {
        for permission in [
            P::ManageApprovals,
            P::ManageDocuments,
            P::ManageEmailTemplates,
            P::SendLetters,
            P::ManageEmployees,
            P::ImportEmployees,
            P::ManageAttendance,
            P::GenerateAttendanceQr,
            P::ManageKiosks,
            P::ManageCompanySettings,
            P::ManageUsers,
            P::ManageCompanies,
            P::ManageBackups,
            P::ManagePlatformSettings,
        ] {
            assert!(
                !role_permissions("exec").contains(&permission),
                "exec must remain read-only and never hold {permission:?}"
            );
        }
    }

    /// `admin` is a company administrator, not a payroll operator. Payroll
    /// separation of duties depends on this.
    #[test]
    fn admin_holds_no_payroll_permission() {
        for permission in [
            P::ViewPayroll,
            P::ManagePayrollDraft,
            P::SubmitPayroll,
            P::ApprovePayroll,
            P::MarkPayrollPaid,
        ] {
            assert!(
                !role_permissions("admin").contains(&permission),
                "admin must not hold {permission:?}"
            );
        }
    }

    /// Preparing and approving a payroll run must not be held by one role, or
    /// the four-eyes control in `payroll_lifecycle_service` is decorative.
    #[test]
    fn no_company_role_both_prepares_and_approves_payroll() {
        for role in ALL_ROLES.iter().filter(|r| **r != "super_admin") {
            let granted = perms(role);
            assert!(
                !(granted.contains(&P::SubmitPayroll) && granted.contains(&P::ApprovePayroll)),
                "{role} can both submit and approve payroll"
            );
        }
    }

    /// `manager` holds every company-management capability but no payroll or
    /// salary figure, and nothing reserved for the platform owner.
    #[test]
    fn manager_holds_no_payroll_or_platform_permission() {
        for permission in role_permissions("manager") {
            assert!(
                !permission.is_payroll_capability(),
                "manager must not hold payroll capability {permission:?}"
            );
            assert!(
                !permission.is_platform_only(),
                "manager must not hold platform-only {permission:?}"
            );
        }
        assert!(
            !role_permissions("manager").contains(&P::ImportEmployees),
            "bulk import writes salaries without a payroll check"
        );
        assert!(
            !role_permissions("manager").contains(&P::ViewAuditLog),
            "the audit trail is not part of the manager role"
        );
    }

    /// Every non-payroll, non-platform capability `admin` has, `manager` has,
    /// except the audit trail.
    #[test]
    fn manager_covers_every_admin_management_permission() {
        let manager = perms("manager");
        for permission in role_permissions("admin")
            .iter()
            .filter(|p| **p != P::ViewAuditLog)
        {
            assert!(
                manager.contains(permission),
                "manager is missing {permission:?}"
            );
        }
    }

    #[test]
    fn the_employee_role_grants_nothing() {
        assert!(
            role_permissions("employee").is_empty(),
            "portal routes authorize on employee_id, not permissions"
        );
    }

    #[test]
    fn an_unknown_role_grants_nothing() {
        assert!(role_permissions("wharf_inspector").is_empty());
    }

    #[test]
    fn roles_are_additive() {
        let roles = vec!["finance".to_string(), "hr_manager".to_string()];
        assert!(roles_grant(&roles, P::ApprovePayroll), "from finance");
        assert!(roles_grant(&roles, P::ManageAttendance), "from hr_manager");
        assert!(
            !roles_grant(&roles, P::ManageUsers),
            "neither role grants user management"
        );
    }
}
