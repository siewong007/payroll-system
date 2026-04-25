import type { User } from '@/types';

export type AppRole = User['role'];
type MaybeAppRole = AppRole | undefined | null;

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

export function canAccessPayrollData(role: MaybeAppRole): boolean {
  return role === 'super_admin' || role === 'payroll_admin' || role === 'finance';
}

export function canPreparePayroll(role: MaybeAppRole): boolean {
  return role === 'super_admin' || role === 'payroll_admin';
}

export function canApprovePayroll(role: MaybeAppRole): boolean {
  return role === 'super_admin' || role === 'finance';
}
