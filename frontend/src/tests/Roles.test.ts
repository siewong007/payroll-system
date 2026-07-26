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
  canUseAdminConsole,
  hasAnyRole,
  hasOnlyEmployeeRole,
  roleList,
  ROLE_META,
  CREATABLE_ROLES,
  roleLabel,
  roleBadgeClass,
  toggleRole,
  isSingleCompanyRoleSet,
  normalizeCompanySelection,
  sameIdSet,
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

  it('offers the admin console to everyone AppLayout keeps out of the portal', () => {
    // The portal's way back must be offered on exactly the condition that
    // AppLayout will not redirect them out again — a second role is what holds
    // someone in the admin shell, so it is also what earns the link.
    expect(canUseAdminConsole(['employee', 'hr_manager'])).toBe(true);
    expect(canUseAdminConsole(multiRoleUser)).toBe(true);
    expect(canUseAdminConsole('employee')).toBe(false);
    // Not a mirror of hasOnlyEmployeeRole at the edges: a roleless identity has
    // no console to return to, though AppLayout would not have redirected it.
    expect(canUseAdminConsole([])).toBe(false);
    expect(canUseAdminConsole(null)).toBe(false);
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

describe('role selection rules (mirrors backend user_service)', () => {
  it('covers every AppRole with display metadata', () => {
    expect(Object.keys(ROLE_META).sort()).toEqual([...ALL_ROLES].sort());
    for (const role of ALL_ROLES) {
      expect(roleLabel(role)).not.toBe(role);
      expect(roleBadgeClass(role)).toContain('bg-');
    }
  });

  it('falls back to the raw value for an unknown role', () => {
    expect(roleLabel('mystery')).toBe('mystery');
    expect(roleBadgeClass('mystery')).toBe('bg-gray-100 text-gray-600');
  });

  it('excludes employee from the roles an admin may create', () => {
    expect(CREATABLE_ROLES).not.toContain('employee');
    expect(CREATABLE_ROLES).toContain('super_admin');
  });

  it('never removes the last remaining role', () => {
    expect(toggleRole(['admin'], 'admin')).toEqual(['admin']);
  });

  it('removes a role when others remain', () => {
    expect(toggleRole(['admin', 'finance'], 'admin')).toEqual(['finance']);
  });

  it('replaces the whole selection when exec or employee is chosen', () => {
    expect(toggleRole(['admin', 'finance'], 'exec')).toEqual(['exec']);
    expect(toggleRole(['admin', 'finance'], 'employee')).toEqual(['employee']);
  });

  it('clears exec and employee when a normal role is added', () => {
    expect(toggleRole(['exec'], 'admin')).toEqual(['admin']);
    expect(toggleRole(['employee'], 'finance')).toEqual(['finance']);
  });

  it('flags only exec and employee as single-company role sets', () => {
    expect(isSingleCompanyRoleSet(['exec'])).toBe(true);
    expect(isSingleCompanyRoleSet(['employee'])).toBe(true);
    expect(isSingleCompanyRoleSet(['admin', 'finance'])).toBe(false);
  });

  it('truncates a company selection to one for single-company roles only', () => {
    expect(normalizeCompanySelection(['exec'], ['a', 'b', 'c'])).toEqual(['a']);
    expect(normalizeCompanySelection(['admin'], ['a', 'b', 'c'])).toEqual(['a', 'b', 'c']);
  });

  it('compares id sets without regard to order', () => {
    expect(sameIdSet(['a', 'b'], ['b', 'a'])).toBe(true);
    expect(sameIdSet([], [])).toBe(true);
    expect(sameIdSet(['a'], ['a', 'b'])).toBe(false);
    expect(sameIdSet(['a'], ['b'])).toBe(false);
  });
});
