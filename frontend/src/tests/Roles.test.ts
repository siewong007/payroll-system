import { describe, expect, it } from 'vitest';
import type { User } from '@/types';
import {
  ADMIN_DATA_ROLES,
  ALL_ROLES,
  PAYROLL_DATA_ROLES,
  REPORT_ROLES,
  canAccessPayrollData,
  canApprovePayroll,
  canPreparePayroll,
  hasAnyRole,
  hasOnlyEmployeeRole,
  roleList,
} from '@/lib/roles';

const multiRoleUser: User = {
  id: 'user-1',
  email: 'finance@example.com',
  full_name: 'Finance Admin',
  roles: ['admin', 'finance'],
  company_id: 'company-1',
  employee_id: null,
};

describe('role helpers', () => {
  it('normalizes absent, scalar, array, and user role inputs', () => {
    expect(roleList(null)).toEqual([]);
    expect(roleList(undefined)).toEqual([]);
    expect(roleList('admin')).toEqual(['admin']);
    expect(roleList(['admin', 'finance'])).toEqual(['admin', 'finance']);
    expect(roleList(multiRoleUser)).toEqual(['admin', 'finance']);
  });

  it('authorizes a user when any one of multiple assigned roles is allowed', () => {
    expect(hasAnyRole(multiRoleUser, PAYROLL_DATA_ROLES)).toBe(true);
    expect(hasAnyRole(multiRoleUser, ['employee'])).toBe(false);
    expect(hasAnyRole(null, ALL_ROLES)).toBe(false);
  });

  it('recognizes only a strictly employee-only identity as employee-only', () => {
    expect(hasOnlyEmployeeRole('employee')).toBe(true);
    expect(hasOnlyEmployeeRole(['employee', 'hr_manager'])).toBe(false);
    expect(hasOnlyEmployeeRole([])).toBe(false);
  });

  it('keeps payroll preparation and approval duties separated', () => {
    expect(canAccessPayrollData('payroll_admin')).toBe(true);
    expect(canPreparePayroll('payroll_admin')).toBe(true);
    expect(canApprovePayroll('payroll_admin')).toBe(false);

    expect(canAccessPayrollData('finance')).toBe(true);
    expect(canPreparePayroll('finance')).toBe(false);
    expect(canApprovePayroll('finance')).toBe(true);

    expect(canPreparePayroll('super_admin')).toBe(true);
    expect(canApprovePayroll('super_admin')).toBe(true);
    expect(canAccessPayrollData('exec')).toBe(false);
  });

  it('excludes exec from reports and limits admin data to admins', () => {
    expect(REPORT_ROLES).not.toContain('exec');
    expect(REPORT_ROLES).toContain('employee');
    expect(ADMIN_DATA_ROLES).toEqual(['super_admin', 'admin']);
  });
});
