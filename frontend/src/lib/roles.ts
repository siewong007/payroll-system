import type { PermissionKey } from '@/api/permissions';
import type { AppRole, User } from '@/types';
import i18n from '@/i18n';

export type { AppRole };

type MaybeAppRole = AppRole | AppRole[] | User | undefined | null;

export interface RoleMeta {
  badgeClass: string;
}

/**
 * Single source of role display metadata. Typing it as `Record<AppRole, …>`
 * makes any future addition to the `AppRole` union a compile error here rather
 * than a role that silently renders as a raw identifier.
 */
export const ROLE_META: Record<AppRole, RoleMeta> = {
  super_admin: { badgeClass: 'bg-zinc-900 text-white border border-zinc-900 dark:bg-zinc-100 dark:text-zinc-950 dark:border-zinc-100' },
  admin: { badgeClass: 'bg-[#1B2E4B]/10 text-[#1B2E4B] border border-[#1B2E4B]/20 dark:bg-[#93B4DC]/15 dark:text-[#93B4DC] dark:border-[#93B4DC]/25' },
  manager: { badgeClass: 'bg-zinc-100 text-zinc-800 border border-zinc-200 dark:bg-zinc-800 dark:text-zinc-200 dark:border-zinc-700' },
  payroll_admin: { badgeClass: 'bg-blue-50 text-blue-800 border border-blue-200 dark:bg-blue-500/12 dark:text-blue-300 dark:border-blue-500/25' },
  hr_manager: { badgeClass: 'bg-emerald-50 text-emerald-800 border border-emerald-200 dark:bg-emerald-500/12 dark:text-emerald-300 dark:border-emerald-500/25' },
  finance: { badgeClass: 'bg-amber-50 text-amber-800 border border-amber-200 dark:bg-amber-500/12 dark:text-amber-300 dark:border-amber-500/25' },
  exec: { badgeClass: 'bg-zinc-100 text-zinc-700 border border-zinc-200 dark:bg-zinc-800/80 dark:text-zinc-300 dark:border-zinc-700' },
  employee: { badgeClass: 'bg-sky-50 text-sky-800 border border-sky-200 dark:bg-sky-500/12 dark:text-sky-300 dark:border-sky-500/25' },
};

export const ALL_ROLES: AppRole[] = Object.keys(ROLE_META) as AppRole[];

/**
 * Roles restricted to a single company. Mirrors the backend's
 * `SINGLE_COMPANY_ROLES` in `user_service.rs`.
 */
export const SINGLE_COMPANY_ROLES: AppRole[] = ['exec', 'employee'];

/**
 * Roles an administrator may assign when creating an account. `employee`
 * accounts are provisioned automatically alongside an employee record, so the
 * backend rejects them on `POST /admin/users`.
 */
export const CREATABLE_ROLES: AppRole[] = ALL_ROLES.filter((role) => role !== 'employee');

export function roleLabel(role: string): string {
  if (!(role in ROLE_META)) return role;
  return i18n.t(`roles.${role}`);
}

export function roleBadgeClass(role: string): string {
  return ROLE_META[role as AppRole]?.badgeClass ?? 'bg-gray-100 text-gray-600';
}

export function isSingleCompanyRoleSet(roles: AppRole[]): boolean {
  return roles.some((role) => SINGLE_COMPANY_ROLES.includes(role));
}

/**
 * Toggle one role within a selection, enforcing the backend's two rules: a user
 * always holds at least one role, and `exec`/`employee` are exclusive.
 */
export function toggleRole(roles: AppRole[], role: AppRole): AppRole[] {
  if (roles.includes(role)) {
    return roles.length === 1 ? roles : roles.filter((existing) => existing !== role);
  }
  if (SINGLE_COMPANY_ROLES.includes(role)) {
    return [role];
  }
  return [...roles.filter((existing) => !SINGLE_COMPANY_ROLES.includes(existing)), role];
}

/** Truncates a company selection to one entry when the role set demands it. */
export function normalizeCompanySelection(roles: AppRole[], companyIds: string[]): string[] {
  return isSingleCompanyRoleSet(roles) ? companyIds.slice(0, 1) : companyIds;
}

/** Order-insensitive set comparison; mirrors the backend's `same_set`. */
export function sameIdSet(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && [...a].sort().join() === [...b].sort().join();
}

export const PAYROLL_DATA_ROLES: AppRole[] = ['super_admin', 'payroll_admin', 'finance'];
export const PAYROLL_PREP_ROLES: AppRole[] = ['super_admin', 'payroll_admin'];
export const PAYROLL_APPROVAL_ROLES: AppRole[] = ['super_admin', 'finance'];
export const REPORT_ROLES: AppRole[] = ALL_ROLES.filter((role) => role !== 'exec' && role !== 'employee');
export const SUPER_ADMIN_ROLES: AppRole[] = ['super_admin'];
export const ADMIN_DATA_ROLES: AppRole[] = ['super_admin', 'admin'];
/**
 * Roles hidden from any directory viewer who holds none of them. Mirrors the
 * backend's `user_service::hides_privileged_users`; the API already filters
 * these users out, this only keeps the UI from offering them as a filter.
 */
export const PRIVILEGED_USER_ROLES: AppRole[] = ['super_admin', 'admin'];

export function hidesPrivilegedUsers(value: MaybeAppRole): boolean {
  return !hasAnyRole(value, PRIVILEGED_USER_ROLES);
}
/**
 * May read company-wide attendance (records, summary, CSV export — which
 * carry every colleague's movements and GPS coordinates). Mirrors the
 * backend's `require_attendance_viewer` allow-list exactly.
 */
export const ATTENDANCE_VIEW_ROLES: AppRole[] = [
  'super_admin',
  'admin',
  'manager',
  'hr_manager',
  'payroll_admin',
  'finance',
  'exec',
];

export function roleList(value: MaybeAppRole): AppRole[] {
  if (!value) return [];
  if (Array.isArray(value)) return value;
  if (typeof value === 'string') return [value];
  return value.roles ?? [];
}

export function hasAnyRole(value: MaybeAppRole, allowedRoles: AppRole[]): boolean {
  return roleList(value).some((role) => allowedRoles.includes(role));
}

export function hasOnlyEmployeeRole(value: MaybeAppRole): boolean {
  const roles = roleList(value);
  return roles.length === 1 && roles[0] === 'employee';
}

/**
 * Whether this identity has any business in the admin shell.
 *
 * Deliberately derived from `hasOnlyEmployeeRole` rather than restating the rule:
 * it is the exact complement of the redirect in `AppLayout`, so the portal can
 * offer a way back to the console on precisely the condition that `AppLayout`
 * will not bounce them straight out again. Note the two are *not* mirror images
 * on an empty role set — `AppLayout` lets that through, but an offer to visit a
 * console is only worth making to someone who holds a role.
 */
export function canUseAdminConsole(value: MaybeAppRole): boolean {
  const roles = roleList(value);
  if (roles.length > 0 && !hasOnlyEmployeeRole(roles)) {
    return true;
  }
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return (value.permissions?.length ?? 0) > 0;
  }
  return false;
}

const ADMIN_HOME_FALLBACKS: ReadonlyArray<{ requires: PermissionKey[]; path: string }> = [
  { requires: ['view_employees'], path: '/dashboard' },
  { requires: ['view_payroll'], path: '/payroll' },
  { requires: ['manage_payroll_draft'], path: '/payroll/process' },
  { requires: ['view_attendance'], path: '/attendance' },
  { requires: ['view_approvals'], path: '/approvals' },
  { requires: ['view_teams', 'manage_teams'], path: '/teams' },
  { requires: ['view_calendar'], path: '/calendar' },
  { requires: ['view_reports'], path: '/reports' },
  { requires: ['view_documents'], path: '/documents' },
  { requires: ['view_email_logs', 'manage_email_templates', 'send_letters'], path: '/letters' },
  { requires: ['manage_company_settings'], path: '/settings' },
  { requires: ['view_user_directory', 'manage_users'], path: '/user-groups' },
  { requires: ['view_audit_log'], path: '/audit-trail' },
];

export function defaultAdminHome(user: User | null | undefined): string {
  if (!user) return '/login';
  if (!canUseAdminConsole(user)) return '/portal/profile';
  if (hasAnyRole(user, SUPER_ADMIN_ROLES)) return '/companies';
  const perms = user.permissions ?? [];
  if (user.company_id) {
    for (const entry of ADMIN_HOME_FALLBACKS) {
      if (entry.requires.some((perm) => perms.includes(perm))) {
        return entry.path;
      }
    }
    return '/company';
  }
  return '/account';
}

export function canAccessPayrollData(role: MaybeAppRole): boolean {
  return hasAnyRole(role, PAYROLL_DATA_ROLES);
}

export function canPreparePayroll(role: MaybeAppRole): boolean {
  return hasAnyRole(role, PAYROLL_PREP_ROLES);
}

export function canApprovePayroll(role: MaybeAppRole): boolean {
  return hasAnyRole(role, PAYROLL_APPROVAL_ROLES);
}
