import { Fragment } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Check, X } from 'lucide-react';
import { permissionsApi, type PermissionDescriptor, type RoleDescriptor } from '@/api/permissions';
import { ROLE_META, SINGLE_COMPANY_ROLES, roleBadgeClass, roleLabel } from '@/lib/roles';
import type { AppRole } from '@/types';

/**
 * Descriptions are presentation-only. Everything that determines *access* —
 * which roles exist and what each one grants — comes from
 * `/auth/permissions/matrix`, i.e. from `backend/src/core/permission.rs`.
 *
 * This page previously hardcoded the whole matrix. It had drifted: it showed
 * `hr_manager` with payroll access and `exec` with reports access, neither of
 * which the API ever allowed. A table nobody can verify is worse than no table,
 * because it gets used to answer "who can see payroll?".
 */
const ROLE_DESCRIPTION_KEYS: Record<AppRole, string> = {
  super_admin: 'roles.descriptions.super_admin',
  admin: 'roles.descriptions.admin',
  payroll_admin: 'roles.descriptions.payroll_admin',
  hr_manager: 'roles.descriptions.hr_manager',
  finance: 'roles.descriptions.finance',
  exec: 'roles.descriptions.exec',
  employee: 'roles.descriptions.employee',
};

const groupKey = (group: string) => group.toLowerCase().replace(/[^a-z0-9]+/g, '_');

function groupOrder(permissions: PermissionDescriptor[]): string[] {
  const seen: string[] = [];
  for (const permission of permissions) {
    if (!seen.includes(permission.group)) seen.push(permission.group);
  }
  return seen;
}

export function RoleManagement() {
  const { t } = useTranslation();
  const { data, isLoading, isError, error } = useQuery({
    queryKey: ['auth', 'permissions', 'matrix'],
    queryFn: permissionsApi.matrix,
    staleTime: 5 * 60_000,
  });

  if (isLoading) {
    return (
      <div className="flex min-h-40 items-center justify-center">
        <div className="spinner" />
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="card p-5">
        <p className="text-sm text-red-600">
          {t('roles.matrixLoadFailed')}{error instanceof Error ? `: ${error.message}` : '.'}
        </p>
      </div>
    );
  }

  const roles: RoleDescriptor[] = data.roles;
  const grantedBy = new Map(roles.map((role) => [role.key, new Set(role.permissions)]));
  const groups = groupOrder(data.permissions);

  return (
    <div className="space-y-6">
      <div className="page-header">
        <h1 className="page-title">{t('roles.title')}</h1>
        <p className="page-subtitle">
          {t('roles.subtitle')}
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {roles.map((role) => {
          const multiCompany = !SINGLE_COMPANY_ROLES.includes(role.key);
          return (
            <div key={role.key} className="card p-5 space-y-3">
              <div className="flex items-center justify-between">
                <span className={`text-xs px-2.5 py-1 rounded-full font-medium ${roleBadgeClass(role.key)}`}>
                  {roleLabel(role.key)}
                </span>
                {multiCompany ? (
                  <span className="text-[10px] uppercase tracking-wider text-green-600 font-semibold">
                    {t('roles.multiCompany')}
                  </span>
                ) : (
                  <span className="text-[10px] uppercase tracking-wider text-gray-400 font-semibold">
                    {t('roles.singleCompany')}
                  </span>
                )}
              </div>
              <p className="text-sm text-gray-500">{ROLE_DESCRIPTION_KEYS[role.key] ? t(ROLE_DESCRIPTION_KEYS[role.key]) : ''}</p>
              <p className="text-xs text-gray-400">
                {role.permissions.length === 0
                  ? t('roles.noPermissions')
                  : t('roles.permissionCount', { count: role.permissions.length })}
              </p>
            </div>
          );
        })}
      </div>

      <div className="card p-0 overflow-hidden">
        <div className="p-5 border-b border-gray-100">
          <h2 className="text-base font-semibold text-gray-900">{t('roles.matrixTitle')}</h2>
        </div>
        {/* Wide table: scrolls within its own container so the page body never
            scrolls sideways on a narrow viewport. */}
        <div className="overflow-x-auto">
          <table className="data-table">
            <thead>
              <tr>
                <th className="sticky left-0 bg-gray-50 z-10">{t('roles.permission')}</th>
                {roles.map((role) => (
                  <th key={role.key} className="text-center whitespace-nowrap">
                    <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${roleBadgeClass(role.key)}`}>
                      {roleLabel(role.key)}
                    </span>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {groups.map((group) => (
                <Fragment key={group}>
                  <tr>
                    <td
                      className="sticky left-0 bg-gray-50 z-10 text-xs font-semibold uppercase tracking-wider text-gray-500"
                      colSpan={roles.length + 1}
                    >
                      {t(`permissions.groups.${groupKey(group)}`, { defaultValue: group })}
                    </td>
                  </tr>
                  {data.permissions
                    .filter((permission) => permission.group === group)
                    .map((permission) => (
                      <tr key={permission.key}>
                        <td className="sticky left-0 bg-white z-10 font-medium text-gray-700">
                          {t(`permissions.labels.${permission.key}`, { defaultValue: permission.label })}
                        </td>
                        {roles.map((role) => (
                          <td key={role.key} className="text-center">
                            {grantedBy.get(role.key)?.has(permission.key) ? (
                              <Check
                                className="w-4 h-4 text-green-500 mx-auto"
                                aria-label={t('roles.can', { role: roleLabel(role.key), permission: t(`permissions.labels.${permission.key}`, { defaultValue: permission.label }) })}
                              />
                            ) : (
                              <X
                                className="w-4 h-4 text-gray-200 mx-auto"
                                aria-label={t('roles.cannot', { role: roleLabel(role.key), permission: t(`permissions.labels.${permission.key}`, { defaultValue: permission.label }) })}
                              />
                            )}
                          </td>
                        ))}
                      </tr>
                    ))}
                </Fragment>
              ))}
              <tr>
                <td className="sticky left-0 bg-white z-10 font-medium text-gray-700">{t('roles.multiCompany')}</td>
                {roles.map((role) => (
                  <td key={role.key} className="text-center">
                    {SINGLE_COMPANY_ROLES.includes(role.key) ? (
                      <X className="w-4 h-4 text-gray-200 mx-auto" />
                    ) : (
                      <Check className="w-4 h-4 text-green-500 mx-auto" />
                    )}
                  </td>
                ))}
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      {Object.keys(ROLE_META).length !== roles.length && (
        <p className="text-xs text-amber-600">
          {t('roles.countMismatch', { api: roles.length, known: Object.keys(ROLE_META).length })}
        </p>
      )}
    </div>
  );
}
