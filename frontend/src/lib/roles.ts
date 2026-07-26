import type { AppRole, User } from '@/types';

export type { AppRole };

type MaybeAppRole = AppRole | AppRole[] | User | undefined | null;

export interface RoleMeta {
  label: string;
  badgeClass: string;
}

/**
 * Single source of role display metadata. Typing it as `Record<AppRole, …>`
 * makes any future addition to the `AppRole` union a compile error here rather
 * than a role that silently renders as a raw identifier.
 */
export const ROLE_META: Record<AppRole, RoleMeta> = {
  super_admin: { label: 'Super Admin', badgeClass: 'bg-purple-100 text-purple-700' },
  admin: { label: 'Admin', badgeClass: 'bg-indigo-100 text-indigo-700' },
  payroll_admin: { label: 'Payroll Admin', badgeClass: 'bg-blue-100 text-blue-700' },
  hr_manager: { label: 'HR Manager', badgeClass: 'bg-green-100 text-green-700' },
  finance: { label: 'Finance', badgeClass: 'bg-amber-100 text-amber-700' },
  exec: { label: 'Executive', badgeClass: 'bg-gray-100 text-gray-700' },
  employee: { label: 'Employee', badgeClass: 'bg-sky-100 text-sky-700' },
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
  return ROLE_META[role as AppRole]?.label ?? role;
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
export const REPORT_ROLES: AppRole[] = ALL_ROLES.filter((role) => role !== 'exec');
export const SUPER_ADMIN_ROLES: AppRole[] = ['super_admin'];
export const ADMIN_DATA_ROLES: AppRole[] = ['super_admin', 'admin'];
/**
 * May read company-wide attendance (records, summary, CSV export — which
 * carry every colleague's movements and GPS coordinates). Mirrors the
 * backend's `require_attendance_viewer` allow-list exactly.
 */
export const ATTENDANCE_VIEW_ROLES: AppRole[] = [
  'super_admin',
  'admin',
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
  return roles.length > 0 && !hasOnlyEmployeeRole(roles);
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
