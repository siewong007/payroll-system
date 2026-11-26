import { useTranslation } from 'react-i18next';
import { Link } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { AlertTriangle, DollarSign, ExternalLink, Lock, MapPin, Shield, TrendingDown, TrendingUp, User } from 'lucide-react';
import { getEmployee, getSalaryHistory } from '@/api/employees';
import { usePermissions } from '@/lib/usePermissions';
import { EmptyState } from '@/components/ui/EmptyState';
import { Skeleton } from '@/components/ui/Skeleton';
import { formatDate, formatMYR, getErrorMessage } from '@/lib/utils';

function InfoField({
  label,
  value,
  restricted = false,
}: {
  label: string;
  value: string | null | undefined;
  restricted?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div className="min-w-0">
      <p className="text-xs text-gray-500 dark:text-slate-400 uppercase tracking-wide">{label}</p>
      {restricted ? (
        <span className="inline-flex items-center gap-1 mt-1 px-2 py-0.5 rounded-md text-xs font-medium bg-gray-100 text-gray-600 dark:bg-slate-800 dark:text-slate-300">
          <Lock className="w-3 h-3" aria-hidden="true" />
          {t('employees.restricted')}
        </span>
      ) : (
        <p className="text-sm font-medium text-gray-900 dark:text-slate-100 mt-0.5 break-words tabular-nums">{value || '\u2014'}</p>
      )}
    </div>
  );
}

export function EmployeeProfile({ employeeId }: { employeeId: string }) {
  const { t } = useTranslation();
  const { can } = usePermissions();
  const canViewPayroll = can('view_payroll');
  const canManageEmployees = can('manage_employees');
  const canViewSalaryHistory = can('view_salary_history');
  const showPersonalSensitive = canViewPayroll || canManageEmployees;

  const { data: employee, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['employee', employeeId],
    queryFn: () => getEmployee(employeeId),
  });

  const {
    data: salaryHistory,
    isLoading: loadingSalaryHistory,
    isError: salaryHistoryError,
    error: salaryHistoryErrorObj,
    refetch: refetchSalaryHistory,
  } = useQuery({
    queryKey: ['salaryHistory', employeeId],
    queryFn: () => getSalaryHistory(employeeId),
    enabled: canViewSalaryHistory,
  });

  if (isLoading) {
    return (
      <div role="status" aria-busy="true" aria-label={t('common.loading')} className="space-y-5 py-2">
        <Skeleton className="h-6 w-48" />
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <Skeleton key={i} className="h-12 w-full rounded-lg" />
          ))}
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <EmptyState
        compact
        icon={AlertTriangle}
        title={t('employees.loadFailed')}
        description={getErrorMessage(error, t('employees.loadFailed'))}
        action={
          <button type="button" onClick={() => refetch()} className="btn-primary">
            {t('common.retry')}
          </button>
        }
      />
    );
  }

  if (!employee) {
    return (
      <EmptyState compact icon={User} title={t('employees.notFound')} />
    );
  }

  return (
    <div className="space-y-7">
      {/* Metadata + Status + Link to full profile (modal header already shows full_name) */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-sm text-gray-500 dark:text-slate-400 break-words min-w-0">
          {employee.employee_number} &middot;{' '}
          {employee.department || t('employees.noDepartment')} &middot;{' '}
          {employee.designation || t('employees.noDesignation')}
        </p>
        <div className="flex items-center gap-2 shrink-0">
          <span
            className={`inline-flex px-2.5 py-1 rounded-full text-xs font-medium shrink-0 ${
              employee.is_active
                ? 'bg-green-50 text-green-700 dark:bg-emerald-500/15 dark:text-emerald-300'
                : 'bg-red-50 text-red-700 dark:bg-red-500/15 dark:text-red-300'
            }`}
          >
            {employee.is_active ? t('common.active') : t('common.inactive')}
          </span>
          <Link
            to={`/employees/${employee.id}`}
            className="btn-secondary min-h-11 sm:min-h-8 !py-1 !px-2.5 sm:text-xs"
          >
            {t('common.view')}
            <ExternalLink className="w-3 h-3" aria-hidden="true" />
          </Link>
        </div>
      </div>

      {/* Personal */}
      <section>
        <h4 className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-4 flex items-center gap-2">
          <User className="w-3.5 h-3.5" aria-hidden="true" /> {t('employees.sections.personal')}
        </h4>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-4">
          <InfoField label={t('employees.fields.nric')} value={employee.ic_number} restricted={!showPersonalSensitive} />
          <InfoField
            label={t('employees.fields.dateOfBirth')}
            value={employee.date_of_birth ? formatDate(employee.date_of_birth) : null}
            restricted={!showPersonalSensitive}
          />
          <InfoField
            label={t('employees.fields.gender')}
            value={
              employee.gender
                ? t(`enums.gender.${employee.gender}`, { defaultValue: employee.gender })
                : null
            }
          />
          <InfoField
            label={t('employees.fields.race')}
            value={
              employee.race
                ? t(`enums.race.${employee.race}`, { defaultValue: employee.race })
                : null
            }
          />
          <InfoField label={t('employees.fields.nationality')} value={employee.nationality} />
          <InfoField
            label={t('employees.fields.maritalStatus')}
            value={
              employee.marital_status
                ? t(`enums.maritalStatus.${employee.marital_status}`, {
                    defaultValue: employee.marital_status,
                  })
                : null
            }
          />
          <InfoField label={t('employees.fields.email')} value={employee.email} />
          <InfoField label={t('employees.fields.phone')} value={employee.phone} restricted={!showPersonalSensitive} />
        </div>
      </section>

      {/* Address */}
      {(!showPersonalSensitive || employee.address_line1 || employee.city || employee.state) && (
        <section>
          <h4 className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-4 flex items-center gap-2">
            <MapPin className="w-3.5 h-3.5" aria-hidden="true" /> {t('employees.sections.address')}
          </h4>
          {!showPersonalSensitive ? (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-xs font-medium bg-gray-100 text-gray-600 dark:bg-slate-800 dark:text-slate-300">
              <Lock className="w-3 h-3" aria-hidden="true" />
              {t('employees.restricted')}
            </span>
          ) : (
            <p className="text-sm text-gray-900 dark:text-slate-100 break-words">
              {[
                employee.address_line1,
                employee.address_line2,
                employee.city,
                employee.state,
                employee.postcode,
              ]
                .filter(Boolean)
                .join(', ')}
            </p>
          )}
        </section>
      )}

      {/* Employment & Salary */}
      <section>
        <h4 className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-4 flex items-center gap-2">
          <DollarSign className="w-3.5 h-3.5" aria-hidden="true" /> {t('employees.sections.employment')}
        </h4>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-4">
          <InfoField
            label={t('employees.fields.employmentType')}
            value={
              employee.employment_type
                ? t(`enums.employmentType.${employee.employment_type}`, {
                    defaultValue: employee.employment_type.replaceAll('_', ' '),
                  })
                : null
            }
          />
          <InfoField
            label={t('employees.fields.dateJoined')}
            value={formatDate(employee.date_joined)}
          />
          <InfoField
            label={t('employees.fields.confirmationDate')}
            value={employee.confirmation_date ? formatDate(employee.confirmation_date) : null}
          />
          {employee.date_resigned && (
            <InfoField
              label={t('employees.fields.dateResigned')}
              value={formatDate(employee.date_resigned)}
            />
          )}
          {employee.resignation_reason && (
            <InfoField
              label={t('employees.fields.resignationReason')}
              value={employee.resignation_reason}
            />
          )}
          {canViewPayroll && (
            <InfoField
              label={t('employees.fields.basicSalary')}
              value={formatMYR(employee.basic_salary)}
            />
          )}
          <InfoField label={t('employees.fields.bank')} value={employee.bank_name} restricted={!canViewPayroll} />
          <InfoField label={t('employees.fields.accountNo')} value={employee.bank_account_number} restricted={!canViewPayroll} />
          <InfoField label={t('employees.fields.costCentre')} value={employee.cost_centre} />
          <InfoField label={t('employees.fields.branch')} value={employee.branch} />
        </div>
      </section>

      {canViewPayroll && (
        <section>
          <h4 className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-4 flex items-center gap-2">
            <Shield className="w-3.5 h-3.5" aria-hidden="true" /> {t('employees.sections.statutory')}
          </h4>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-4">
            <InfoField
              label={t('employees.fields.tin')}
              value={employee.tax_identification_number}
            />
            <InfoField label={t('employees.fields.epfNumber')} value={employee.epf_number} />
            <InfoField
              label={t('employees.fields.epfCategory')}
              value={
                employee.epf_category
                  ? t(`enums.epfCategory.${employee.epf_category}`, {
                      defaultValue: employee.epf_category,
                    })
                  : null
              }
            />
            <InfoField label={t('employees.fields.socsoNumber')} value={employee.socso_number} />
            <InfoField label={t('employees.fields.eisNumber')} value={employee.eis_number} />
            <InfoField
              label={t('employees.fields.residency')}
              value={
                employee.residency_status
                  ? t(`enums.residency.${employee.residency_status}`, {
                      defaultValue: employee.residency_status.replaceAll('_', ' '),
                    })
                  : null
              }
            />
            <InfoField
              label={t('employees.fields.workingSpouse')}
              value={employee.working_spouse ? t('common.yes') : t('common.no')}
            />
            <InfoField
              label={t('employees.fields.children')}
              value={String(employee.num_children ?? 0)}
            />
            <InfoField
              label={t('employees.fields.muslim')}
              value={employee.is_muslim ? t('common.yes') : t('common.no')}
            />
            {employee.zakat_eligible && (
              <InfoField
                label={t('employees.fields.zakatMonthly')}
                value={formatMYR(employee.zakat_monthly_amount ?? 0)}
              />
            )}
            {(employee.ptptn_monthly_amount ?? 0) > 0 && (
              <InfoField
                label={t('employees.fields.ptptnMonthly')}
                value={formatMYR(employee.ptptn_monthly_amount!)}
              />
            )}
          </div>
        </section>
      )}

      {/* Salary History */}
      {canViewSalaryHistory && (loadingSalaryHistory || salaryHistoryError || (salaryHistory && salaryHistory.length > 0)) && (
        <section>
          <h4 className="text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-4 flex items-center gap-2">
            <DollarSign className="w-3.5 h-3.5" aria-hidden="true" /> {t('employees.sections.salaryHistory')}
          </h4>
          {loadingSalaryHistory ? (
            <div role="status" aria-busy="true" aria-label={t('common.loading')} className="space-y-2.5">
              <Skeleton className="h-12 w-full rounded-xl" />
              <Skeleton className="h-12 w-full rounded-xl" />
            </div>
          ) : salaryHistoryError ? (
            <EmptyState
              compact
              icon={AlertTriangle}
              title={t('employees.loadFailed')}
              description={getErrorMessage(salaryHistoryErrorObj, t('employees.loadFailed'))}
              action={
                <button type="button" onClick={() => void refetchSalaryHistory()} className="btn-secondary">
                  {t('common.retry')}
                </button>
              }
            />
          ) : (
            <>
              {/* Mobile Cards */}
              <div className="sm:hidden space-y-2.5">
                {salaryHistory!.map((h) => {
                  const pctChange = h.old_salary > 0
                    ? ((h.new_salary - h.old_salary) / h.old_salary) * 100
                    : null;
                  const isIncrease = pctChange !== null && pctChange >= 0;
                  return (
                    <div key={h.id} className="rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-3.5 space-y-2">
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-xs font-medium text-gray-500 dark:text-slate-400 tabular-nums">{formatDate(h.effective_date)}</span>
                        {pctChange !== null && (
                          <span
                            className={`inline-flex items-center gap-0.5 text-xs font-semibold tabular-nums ${
                              isIncrease ? 'text-green-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'
                            }`}
                          >
                            {isIncrease ? <TrendingUp className="w-3 h-3" aria-hidden="true" /> : <TrendingDown className="w-3 h-3" aria-hidden="true" />}
                            {isIncrease ? '+' : ''}{pctChange.toFixed(1)}%
                          </span>
                        )}
                      </div>
                      <div className="flex items-baseline justify-between gap-2 text-sm tabular-nums">
                        <span className="text-gray-500 dark:text-slate-400">{formatMYR(h.old_salary)}</span>
                        <span className="text-gray-400 dark:text-slate-500" aria-hidden="true">{'\u2192'}</span>
                        <span className="font-semibold text-gray-900 dark:text-slate-100">{formatMYR(h.new_salary)}</span>
                      </div>
                      {(h.change_type || h.notes || h.reason) && (
                        <div className="flex flex-wrap items-center gap-1.5 text-xs text-gray-500 dark:text-slate-400">
                          {h.change_type && (
                            <span className="inline-flex rounded-md bg-gray-100 dark:bg-slate-800 px-1.5 py-0.5 font-medium text-gray-700 dark:text-slate-200">
                              {t(`employees.salaryChangeTypes.${h.change_type}`, {
                                defaultValue: h.change_type.replace(/_/g, ' '),
                              })}
                            </span>
                          )}
                          {(h.notes || h.reason) && <span>{h.notes || h.reason}</span>}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>

              {/* Desktop Table */}
              <div className="hidden sm:block overflow-x-auto rounded-2xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900">
                <table className="w-full text-sm">
                  <thead className="bg-gray-50 dark:bg-slate-800/70">
                    <tr>
                      <th className="text-left px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">
                        {t('employees.salaryTable.date')}
                      </th>
                      <th className="text-right px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">
                        {t('employees.salaryTable.old')}
                      </th>
                      <th className="text-right px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">
                        {t('employees.salaryTable.new')}
                      </th>
                      <th className="text-right px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">
                        {t('employees.salaryTable.change')}
                      </th>
                      <th className="text-left px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase">
                        {t('employees.salaryTable.reason')}
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                    {salaryHistory!.map((h) => {
                      const pctChange = h.old_salary > 0
                        ? ((h.new_salary - h.old_salary) / h.old_salary) * 100
                        : null;
                      const isIncrease = pctChange !== null && pctChange >= 0;
                      return (
                        <tr key={h.id}>
                          <td className="px-4 py-2.5 text-gray-700 dark:text-slate-300 whitespace-nowrap tabular-nums">{formatDate(h.effective_date)}</td>
                          <td className="px-4 py-2.5 text-right text-gray-500 dark:text-slate-400 tabular-nums whitespace-nowrap">
                            {formatMYR(h.old_salary)}
                          </td>
                          <td className="px-4 py-2.5 text-right font-medium text-gray-900 dark:text-slate-100 tabular-nums whitespace-nowrap">
                            {formatMYR(h.new_salary)}
                          </td>
                          <td className="px-4 py-2.5 text-right tabular-nums whitespace-nowrap">
                            {pctChange === null ? (
                              <span className="text-gray-400 dark:text-slate-500">{'\u2014'}</span>
                            ) : (
                              <span
                                className={`inline-flex items-center gap-0.5 ${
                                  isIncrease ? 'text-green-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'
                                }`}
                              >
                                {isIncrease ? (
                                  <TrendingUp className="w-3 h-3" aria-hidden="true" />
                                ) : (
                                  <TrendingDown className="w-3 h-3" aria-hidden="true" />
                                )}
                                {isIncrease ? '+' : ''}
                                {pctChange.toFixed(1)}%
                              </span>
                            )}
                          </td>
                          <td className="px-4 py-2.5 text-gray-500 dark:text-slate-400">
                            <div className="flex flex-wrap items-center gap-1.5">
                              {h.change_type && (
                                <span className="inline-flex rounded-md bg-gray-100 dark:bg-slate-800 px-1.5 py-0.5 text-xs font-medium text-gray-700 dark:text-slate-200">
                                  {t(`employees.salaryChangeTypes.${h.change_type}`, {
                                    defaultValue: h.change_type.replace(/_/g, ' '),
                                  })}
                                </span>
                              )}
                              {(h.notes || h.reason) ? (
                                <span>{h.notes || h.reason}</span>
                              ) : !h.change_type ? (
                                <span>{'\u2014'}</span>
                              ) : null}
                            </div>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </section>
      )}
    </div>
  );
}
