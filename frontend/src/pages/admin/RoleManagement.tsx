import { Check, X } from 'lucide-react';

interface RoleDefinition {
  key: string;
  label: string;
  description: string;
  badgeClass: string;
  multiCompany: boolean;
  permissions: Record<string, boolean>;
}

const PERMISSIONS = [
  { key: 'dashboard', label: 'Dashboard' },
  { key: 'company_profile', label: 'Company Profile' },
  { key: 'employees', label: 'Manage Employees' },
  { key: 'payroll', label: 'Payroll Processing' },
  { key: 'teams', label: 'Teams' },
  { key: 'calendar', label: 'Calendar & Holidays' },
  { key: 'approvals', label: 'Leave / Claims / OT Approvals' },
  { key: 'reports', label: 'Reports' },
  { key: 'documents', label: 'Documents' },
  { key: 'settings', label: 'Settings' },
  { key: 'company_management', label: 'Company Management' },
  { key: 'user_management', label: 'User Management' },
  { key: 'role_management', label: 'Role Management' },
];

const ROLES: RoleDefinition[] = [
  {
    key: 'super_admin',
    label: 'Super Admin',
    description: 'Full system access. Manages companies, users, and roles.',
    badgeClass: 'bg-purple-100 text-purple-700',
    multiCompany: true,
    permissions: {
      dashboard: false,
      company_profile: false,
      employees: false,
      payroll: false,
      teams: false,
      calendar: false,
      approvals: false,
      reports: false,
      documents: false,
      settings: false,
      company_management: true,
      user_management: true,
      role_management: true,
    },
  },
  {
    key: 'admin',
    label: 'Admin',
    description: 'Full access including payroll. Can be assigned to multiple companies with company switching.',
    badgeClass: 'bg-indigo-100 text-indigo-700',
    multiCompany: true,
    permissions: {
      dashboard: true,
      company_profile: true,
      employees: true,
      payroll: true,
      teams: true,
      calendar: true,
      approvals: true,
      reports: true,
      documents: true,
      settings: true,
      company_management: false,
      user_management: false,
      role_management: false,
    },
  },
  {
    key: 'payroll_admin',
    label: 'Payroll Admin',
    description: 'Full payroll processing, employee management, and reporting access.',
    badgeClass: 'bg-blue-100 text-blue-700',
    multiCompany: true,
    permissions: {
      dashboard: true,
      company_profile: true,
      employees: true,
      payroll: true,
      teams: true,
      calendar: true,
      approvals: true,
      reports: true,
      documents: true,
      settings: true,
      company_management: false,
      user_management: false,
      role_management: false,
    },
  },
  {
    key: 'hr_manager',
    label: 'HR Manager',
    description: 'Employee management, leave/claims approvals, and reporting.',
    badgeClass: 'bg-green-100 text-green-700',
    multiCompany: true,
    permissions: {
      dashboard: true,
      company_profile: true,
      employees: true,
      payroll: true,
      teams: true,
      calendar: true,
      approvals: true,
      reports: true,
      documents: true,
      settings: true,
      company_management: false,
      user_management: false,
      role_management: false,
    },
  },
  {
    key: 'finance',
    label: 'Finance',
    description: 'Financial reporting, payroll review, and claims approval.',
    badgeClass: 'bg-amber-100 text-amber-700',
    multiCompany: true,
    permissions: {
      dashboard: true,
      company_profile: true,
      employees: true,
      payroll: true,
      teams: true,
      calendar: true,
      approvals: true,
      reports: true,
      documents: true,
      settings: true,
      company_management: false,
      user_management: false,
      role_management: false,
    },
  },
  {
    key: 'exec',
    label: 'Executive',
    description: 'View-only access for company overview. No payroll access. Limited to one company.',
    badgeClass: 'bg-gray-100 text-gray-700',
    multiCompany: false,
    permissions: {
      dashboard: true,
      company_profile: true,
      employees: true,
      payroll: false,
      teams: true,
      calendar: true,
      approvals: true,
      reports: true,
      documents: true,
      settings: true,
      company_management: false,
      user_management: false,
      role_management: false,
    },
  },
  {
    key: 'employee',
    label: 'Employee',
    description: 'Self-service portal access only. View payslips, submit leave/claims/overtime.',
    badgeClass: 'bg-sky-100 text-sky-700',
    multiCompany: false,
    permissions: {
      dashboard: false,
      company_profile: false,
      employees: false,
      payroll: false,
      teams: false,
      calendar: false,
      approvals: false,
      reports: false,
      documents: false,
      settings: false,
      company_management: false,
      user_management: false,
      role_management: false,
    },
  },
];

export function RoleManagement() {
  return (
    <div className="space-y-6">
      <div className="page-header">
        <h1 className="page-title">Roles & Permissions</h1>
        <p className="page-subtitle">Overview of system roles and their access levels</p>
      </div>

      {/* Role Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {ROLES.map((role) => (
          <div key={role.key} className="card p-5 space-y-3">
            <div className="flex items-center justify-between">
              <span className={`text-xs px-2.5 py-1 rounded-full font-medium ${role.badgeClass}`}>
                {role.label}
              </span>
              {role.multiCompany ? (
                <span className="text-[10px] uppercase tracking-wider text-green-600 font-semibold">Multi-Company</span>
              ) : (
                <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">Single Company</span>
              )}
            </div>
            <p className="text-sm text-gray-500">{role.description}</p>
          </div>
        ))}
      </div>

      {/* Permissions Matrix */}
      <div className="card p-0 overflow-hidden">
        <div className="p-5 border-b border-gray-100">
          <h2 className="text-base font-semibold text-gray-900">Permission Matrix</h2>
        </div>
        <div className="overflow-x-auto">
          <table className="data-table">
            <thead>
              <tr>
                <th className="sticky left-0 bg-gray-50 z-10">Permission</th>
                {ROLES.map((r) => (
                  <th key={r.key} className="text-center whitespace-nowrap">
                    <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${r.badgeClass}`}>
                      {r.label}
                    </span>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {PERMISSIONS.map((perm) => (
                <tr key={perm.key}>
                  <td className="sticky left-0 bg-white z-10 font-medium text-gray-700">{perm.label}</td>
                  {ROLES.map((role) => (
                    <td key={role.key} className="text-center">
                      {role.permissions[perm.key] ? (
                        <Check className="w-4 h-4 text-green-500 mx-auto" />
                      ) : (
                        <X className="w-4 h-4 text-gray-200 mx-auto" />
                      )}
                    </td>
                  ))}
                </tr>
              ))}
              <tr>
                <td className="sticky left-0 bg-white z-10 font-medium text-gray-700">Multi-Company</td>
                {ROLES.map((role) => (
                  <td key={role.key} className="text-center">
                    {role.multiCompany ? (
                      <Check className="w-4 h-4 text-green-500 mx-auto" />
                    ) : (
                      <X className="w-4 h-4 text-gray-200 mx-auto" />
                    )}
                  </td>
                ))}
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
