import api from '@/api/client';
import type { AppRole } from '@/types';

/**
 * Permission keys mirror `Permission::as_str()` in `backend/src/core/permission.rs`.
 *
 * This union is the one piece of the authorization model still restated on the
 * frontend, and only so that `usePermission('manage_teams')` is a compile error
 * when misspelled. The *grants* — which role holds which permission — are never
 * duplicated here; they come from `/auth/permissions` at runtime. Two
 * hand-maintained copies of the grants is what put the old Role Management
 * table out of step with the API.
 */
export type PermissionKey =
  | 'view_payroll'
  | 'manage_payroll_draft'
  | 'submit_payroll'
  | 'approve_payroll'
  | 'mark_payroll_paid'
  | 'view_employees'
  | 'manage_employees'
  | 'import_employees'
  | 'view_salary_history'
  | 'view_attendance'
  | 'manage_attendance'
  | 'generate_attendance_qr'
  | 'manage_kiosks'
  | 'set_company_attendance_method'
  | 'view_teams'
  | 'manage_teams'
  | 'view_calendar'
  | 'manage_calendar'
  | 'view_approvals'
  | 'manage_approvals'
  | 'view_reports'
  | 'view_statutory_exports'
  | 'view_documents'
  | 'manage_documents'
  | 'view_email_logs'
  | 'manage_email_templates'
  | 'send_letters'
  | 'manage_company_settings'
  | 'manage_work_schedules'
  | 'manage_geofence'
  | 'view_user_directory'
  | 'manage_users'
  | 'manage_companies'
  | 'view_audit_log'
  | 'manage_backups'
  | 'manage_platform_settings';

export interface MyPermissions {
  roles: AppRole[];
  permissions: PermissionKey[];
}

export interface PermissionDescriptor {
  key: PermissionKey;
  label: string;
  group: string;
}

export interface RoleDescriptor {
  key: AppRole;
  permissions: PermissionKey[];
}

export interface PermissionMatrix {
  permissions: PermissionDescriptor[];
  roles: RoleDescriptor[];
}

export const permissionsApi = {
  /** The caller's own effective permissions. */
  mine: async (): Promise<MyPermissions> => {
    const { data } = await api.get<MyPermissions>('/auth/permissions');
    return data;
  },

  /** The full role-to-permission matrix, for the Role Management screen. */
  matrix: async (): Promise<PermissionMatrix> => {
    const { data } = await api.get<PermissionMatrix>('/auth/permissions/matrix');
    return data;
  },
};
