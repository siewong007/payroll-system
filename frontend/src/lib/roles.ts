import type { AppRole, User } from '@/types';

export type { AppRole };

type MaybeAppRole = AppRole | AppRole[] | User | undefined | null;

export const ALL_ROLES: AppRole[] = [
  'super_admin',
  'admin',
  'payroll_admin',
  'hr_manager',
  'finance',
  'exec',
  'employee',
];

export const PAYROLL_DATA_ROLES: AppRole[] = ['super_admin', 'payroll_admin', 'finance'];
export const PAYROLL_PREP_ROLES: AppRole[] = ['super_admin', 'payroll_admin'];
export const PAYROLL_APPROVAL_ROLES: AppRole[] = ['super_admin', 'finance'];
export const REPORT_ROLES: AppRole[] = ALL_ROLES.filter((role) => role !== 'exec');
export const SUPER_ADMIN_ROLES: AppRole[] = ['super_admin'];
export const ADMIN_DATA_ROLES: AppRole[] = ['super_admin', 'admin'];

export function roleList(value: MaybeAppRole): AppRole[] {
  if (!value) return [];
  if (Array.isArray(value)) return value;
  if (typeof value === 'string') return [value];
  return value.roles?.length ? value.roles : [value.role];
}

export function hasAnyRole(value: MaybeAppRole, allowedRoles: AppRole[]): boolean {
  return roleList(value).some((role) => allowedRoles.includes(role));
}

export function hasOnlyEmployeeRole(value: MaybeAppRole): boolean {
  const roles = roleList(value);
  return roles.length === 1 && roles[0] === 'employee';
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
