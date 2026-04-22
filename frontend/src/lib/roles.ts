import type { User } from '@/types';

type AppRole = User['role'] | undefined | null;

export function canAccessPayrollData(role: AppRole): boolean {
  return role === 'super_admin' || role === 'payroll_admin';
}
