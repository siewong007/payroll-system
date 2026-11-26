import { Fragment } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { AlertTriangle, Check, X } from 'lucide-react';
import { permissionsApi, type PermissionDescriptor, type RoleDescriptor } from '@/api/permissions';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { ROLE_META, SINGLE_COMPANY_ROLES, roleBadgeClass, roleLabel } from '@/lib/roles';
import { getErrorMessage } from '@/lib/utils';
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
  manager: 'roles.descriptions.manager',
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
  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['auth', 'permissions', 'matrix'],
    queryFn: permissionsApi.matrix,
    staleTime: 5 * 60_000,
  });

  if (isLoading) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('roles.title')} subtitle={t('roles.subtitle')} />
        <div role="status" aria-label={t('common.loading')} className="space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4">
            {[0, 1, 2, 3].map((i) => (
              <div key={i} className="card p-5 space-y-3">
                <Skeleton className="h-6 w-28 rounded-full" />
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-3/4" />
              </div>
            ))}
          </div>
          <div className="card space-y-3">
            <Skeleton className="h-6 w-48" />
            <Skeleton className="h-64 w-full rounded-xl" />
          </div>
        </div>
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('roles.title')} subtitle={t('roles.subtitle')} />
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('roles.matrixLoadFailed')}
            description={getErrorMessage(error, t('common.error'))}
            action={
              <button type="button" onClick={() => refetch()} className="btn-secondary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      </div>
    );
  }

  const roles: RoleDescriptor[] = data.roles;
  const grantedBy = new Map(roles.map((role) => [role.key, new Set(role.permissions)]));
  const groups = groupOrder(data.permissions);

  return (
    <div className="space-y-6">
      <PageHeader title={t('roles.title')} subtitle={t('roles.subtitle')} />

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4">
        {roles.map((role) => {
          const multiCompany = !SINGLE_COMPANY_ROLES.includes(role.key);
          return (
            <div key={role.key} className="card p-5 space-y-3">
              <div className="flex items-center justify-between gap-2">
                <span className={`text-xs px-2.5 py-1 rounded-full font-medium ${roleBadgeClass(role.key)}`}>
                  {roleLabel(role.key)}
                </span>
                {multiCompany ? (
                  <span className="text-[11px] uppercase tracking-wider text-green-700 dark:text-emerald-400 font-semibold">
                    {t('roles.multiCompany')}
                  </span>
                ) : (
                  <span className="text-[11px] uppercase tracking-wider text-gray-600 dark:text-slate-400 font-semibold">
                    {t('roles.singleCompany')}
                  </span>
                )}
              </div>
              <p className="text-sm text-gray-600 dark:text-slate-300">{ROLE_DESCRIPTION_KEYS[role.key] ? t(ROLE_DESCRIPTION_KEYS[role.key]) : ''}</p>
              <p className="text-xs text-gray-600 dark:text-slate-400">
                {role.permissions.length === 0
                  ? t('roles.noPermissions')
                  : t('roles.permissionCount', { count: role.permissions.length })}
              </p>
            </div>
          );
        })}
      </div>

      <div className="card p-0 overflow-hidden">
        <div className="p-5 border-b border-gray-100 dark:border-slate-800">
          <h2 className="text-base font-semibold text-gray-900 dark:text-slate-100">{t('roles.matrixTitle')}</h2>
        </div>
        {/* Wide table: scrolls within its own container so the page body never
            scrolls sideways on a narrow viewport. */}
        <div className="overflow-x-auto max-h-[70vh]">
          <table className="data-table">
            <thead>
              <tr>
                <th className="sticky left-0 top-0 bg-gray-50 dark:bg-slate-800 z-30">{t('roles.permission')}</th>
                {roles.map((role) => (
                  <th key={role.key} className="sticky top-0 bg-gray-50 dark:bg-slate-800 z-20 text-center whitespace-nowrap">
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
                  <tr className="bg-gray-50 dark:bg-slate-800">
                    <th
                      scope="colgroup"
                      className="p-0 text-left bg-gray-50 dark:bg-slate-800"
                      colSpan={roles.length + 1}
                    >
                      <span className="sticky left-0 z-10 inline-block bg-gray-50 dark:bg-slate-800 px-4 py-2 text-xs font-semibold uppercase tracking-wider text-gray-600 dark:text-slate-300">
                        {t(`permissions.groups.${groupKey(group)}`, { defaultValue: group })}
                      </span>
                    </th>
                  </tr>
                  {data.permissions
                    .filter((permission) => permission.group === group)
                    .map((permission) => {
                      const permLabel = t(`permissions.labels.${permission.key}`, { defaultValue: permission.label });
                      return (
                        <tr key={permission.key}>
                          <th scope="row" className="sticky left-0 bg-white dark:bg-slate-900 z-10 text-left font-medium text-gray-800 dark:text-slate-200">
                            {permLabel}
                          </th>
                          {roles.map((role) => {
                            const has = Boolean(grantedBy.get(role.key)?.has(permission.key));
                            const cellSummary = t(has ? 'roles.can' : 'roles.cannot', {
                              role: roleLabel(role.key),
                              permission: permLabel,
                            });
                            return (
                              <td key={role.key} className="text-center">
                                {has ? (
                                  <Check
                                    className="w-4 h-4 text-green-600 dark:text-emerald-400 mx-auto"
                                    aria-hidden="true"
                                  />
                                ) : (
                                  <X
                                    className="w-4 h-4 text-gray-300 dark:text-slate-600 mx-auto"
                                    aria-hidden="true"
                                  />
                                )}
                                <span className="sr-only">{cellSummary}</span>
                              </td>
                            );
                          })}
                        </tr>
                      );
                    })}
                </Fragment>
              ))}
              <tr>
                <th scope="row" className="sticky left-0 bg-white dark:bg-slate-900 z-10 text-left font-medium text-gray-800 dark:text-slate-200">
                  {t('roles.multiCompany')}
                </th>
                {roles.map((role) => {
                  const multi = !SINGLE_COMPANY_ROLES.includes(role.key);
                  return (
                    <td key={role.key} className="text-center">
                      {multi ? (
                        <Check className="w-4 h-4 text-green-600 dark:text-emerald-400 mx-auto" aria-hidden="true" />
                      ) : (
                        <X className="w-4 h-4 text-gray-300 dark:text-slate-600 mx-auto" aria-hidden="true" />
                      )}
                      <span className="sr-only">
                        {t(multi ? 'roles.can' : 'roles.cannot', {
                          role: roleLabel(role.key),
                          permission: t('roles.multiCompany'),
                        })}
                      </span>
                    </td>
                  );
                })}
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      {Object.keys(ROLE_META).length !== roles.length && (
        <p className="text-xs text-amber-700 dark:text-amber-400">
          {t('roles.countMismatch', { api: roles.length, known: Object.keys(ROLE_META).length })}
        </p>
      )}
    </div>
  );
}
