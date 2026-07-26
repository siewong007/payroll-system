import type { PermissionKey } from '@/api/permissions';
import type { AppRole, User } from '@/types';

/**
 * Test-only mirror of the role-to-permission table in
 * `backend/src/core/permission.rs`.
 *
 * Production code deliberately holds no copy of these grants — the API ships a
 * user's effective permissions on the session. This fixture exists solely so a
 * component test can say "render as an admin" without hand-listing thirty
 * permission strings.
 *
 * It is *not* the authority on who may do what, and nothing here proves the
 * backend agrees. The assertions that pin role grants live in
 * `core::permission`'s unit tests and in `route_auth_tests`, where they can be
 * checked against the real gates. If this fixture drifts, frontend tests
 * describe a system that does not exist — so keep it in step when the table
 * changes.
 */
const ROLE_PERMISSIONS: Record<AppRole, PermissionKey[]> = {
  super_admin: [
    'view_payroll', 'manage_payroll_draft', 'submit_payroll', 'approve_payroll', 'mark_payroll_paid',
    'view_employees', 'manage_employees', 'import_employees', 'view_salary_history',
    'view_attendance', 'manage_attendance', 'generate_attendance_qr', 'manage_kiosks',
    'set_company_attendance_method', 'view_teams', 'manage_teams', 'view_calendar', 'manage_calendar',
    'view_approvals', 'manage_approvals', 'view_reports', 'view_statutory_exports',
    'view_documents', 'manage_documents', 'view_email_logs', 'manage_email_templates', 'send_letters',
    'manage_company_settings', 'manage_work_schedules', 'manage_geofence',
    'view_user_directory', 'manage_users', 'manage_companies', 'view_audit_log', 'manage_backups',
    'manage_platform_settings',
  ],
  admin: [
    'view_employees', 'manage_employees',
    'view_attendance', 'manage_attendance', 'generate_attendance_qr', 'manage_kiosks',
    'set_company_attendance_method', 'view_teams', 'manage_teams', 'view_calendar', 'manage_calendar',
    'view_approvals', 'manage_approvals', 'view_reports',
    'view_documents', 'manage_documents', 'view_email_logs', 'manage_email_templates', 'send_letters',
    'manage_company_settings', 'manage_work_schedules', 'manage_geofence',
    'view_user_directory', 'view_audit_log',
  ],
  payroll_admin: [
    'view_payroll', 'manage_payroll_draft', 'submit_payroll',
    'view_employees', 'manage_employees', 'import_employees', 'view_salary_history',
    'view_attendance', 'generate_attendance_qr', 'manage_kiosks',
    'view_teams', 'manage_teams', 'view_calendar', 'manage_calendar',
    'view_approvals', 'manage_approvals', 'view_reports', 'view_statutory_exports',
    'view_documents', 'manage_documents', 'view_email_logs', 'manage_email_templates', 'send_letters',
  ],
  hr_manager: [
    'view_employees', 'manage_employees',
    'view_attendance', 'manage_attendance', 'generate_attendance_qr', 'manage_kiosks',
    'view_teams', 'manage_teams', 'view_calendar', 'manage_calendar',
    'view_approvals', 'manage_approvals', 'view_reports',
    'view_documents', 'manage_documents', 'view_email_logs', 'manage_email_templates', 'send_letters',
    'manage_work_schedules', 'manage_geofence',
  ],
  finance: [
    'view_payroll', 'approve_payroll', 'mark_payroll_paid',
    'view_employees', 'view_salary_history', 'view_attendance',
    'view_reports', 'view_statutory_exports',
    'view_documents', 'manage_documents', 'view_email_logs', 'manage_email_templates', 'send_letters',
  ],
  exec: [
    'view_employees', 'view_attendance', 'view_teams', 'view_calendar',
    'view_approvals', 'manage_approvals',
    'view_documents', 'manage_documents', 'view_email_logs', 'manage_email_templates', 'send_letters',
  ],
  employee: [],
};

/** The union of every listed role's grants, matching the backend's semantics. */
export function permissionsForRoles(roles: AppRole[]): PermissionKey[] {
  const granted = new Set<PermissionKey>();
  for (const role of roles) {
    for (const permission of ROLE_PERMISSIONS[role] ?? []) granted.add(permission);
  }
  return [...granted];
}

/** A session user carrying the permissions its roles imply. */
export function userWithRoles(roles: AppRole[], overrides: Partial<User> = {}): User {
  return {
    id: 'user-1',
    email: 'person@example.com',
    full_name: 'Test Person',
    roles,
    company_id: 'company-1',
    employee_id: null,
    permissions: permissionsForRoles(roles),
    ...overrides,
  };
}
