import { useState } from 'react';
import { useParams, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { AlertTriangle, DollarSign, Shield, Pencil, Lock, User } from 'lucide-react';
import { getEmployee, getSalaryHistory } from '@/api/employees';
import { formatMYR, formatDate, getErrorMessage } from '@/lib/utils';
import { usePermissions } from '@/lib/usePermissions';
import { EmptyState } from '@/components/ui/EmptyState';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { EmployeeFormModal } from './EmployeeFormModal';

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

export function EmployeeDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { can } = usePermissions();
  const canViewPayroll = can('view_payroll');
  const canManageEmployees = can('manage_employees');
  const canViewSalaryHistory = can('view_salary_history');
  const showPersonalSensitive = canViewPayroll || canManageEmployees;
  const [showEdit, setShowEdit] = useState(false);

  const { data: employee, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['employee', id],
    queryFn: () => getEmployee(id!),
    enabled: !!id,
  });

  const {
    data: salaryHistory,
    isLoading: loadingSalaryHistory,
    isError: salaryHistoryError,
    error: salaryHistoryErrorObj,
    refetch: refetchSalaryHistory,
  } = useQuery({
    queryKey: ['salaryHistory', id],
    queryFn: () => getSalaryHistory(id!),
    enabled: !!id && canViewSalaryHistory,
  });

  const breadcrumbs = [
    { label: t('nav.employees'), href: '/employees' },
    { label: employee?.full_name || t('common.details') },
  ];

  if (isLoading) {
    return (
      <div className="space-y-6" role="status" aria-busy="true" aria-label={t('common.loading')}>
        <PageHeader className="mb-0" title={t('common.loading')} breadcrumbs={breadcrumbs} />
        <div className={`grid grid-cols-1 gap-6 ${canViewPayroll ? 'md:grid-cols-2 xl:grid-cols-3' : 'md:grid-cols-2'}`}>
          {(canViewPayroll ? [0, 1, 2] : [0, 1]).map((idx) => (
            <div key={idx} className="card space-y-3">
              <Skeleton className="h-5 w-40" />
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-5/6" />
              <Skeleton className="h-4 w-3/4" />
            </div>
          ))}
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="space-y-6">
        <PageHeader className="mb-0" title={t('employees.loadFailed')} breadcrumbs={breadcrumbs} />
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('employees.loadFailed')}
            description={getErrorMessage(error, t('employees.loadFailed'))}
            action={
              <button type="button" onClick={() => refetch()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      </div>
    );
  }

  if (!employee) {
    return (
      <div className="space-y-6">
        <PageHeader className="mb-0" title={t('employees.notFound')} breadcrumbs={breadcrumbs} />
        <div className="card">
          <EmptyState
            icon={User}
            title={t('employees.notFound')}
            action={
              <button type="button" onClick={() => navigate('/employees')} className="btn-secondary">
                {t('employees.backToList')}
              </button>
            }
          />
        </div>
      </div>
    );
  }

  const empType = employee.employment_type
    ? t(`enums.employmentType.${employee.employment_type}`, { defaultValue: employee.employment_type.replaceAll('_', ' ') })
    : null;
  const residency = employee.residency_status
    ? t(`enums.residency.${employee.residency_status}`, { defaultValue: employee.residency_status.replaceAll('_', ' ') })
    : null;
  const addressParts = [
    employee.address_line1,
    employee.address_line2,
    employee.city,
    employee.state,
    employee.postcode,
  ].filter(Boolean);

  return (
    <div className="space-y-6">
      <PageHeader
        className="mb-0"
        title={employee.full_name}
        subtitle={`${employee.employee_number} \u00B7 ${employee.department || t('employees.noDepartment')} \u00B7 ${employee.designation || t('employees.noDesignation')}`}
        breadcrumbs={breadcrumbs}
        actions={
          <div className="flex flex-wrap items-center gap-2 shrink-0">
            <span
              className={`inline-flex px-3 py-1 rounded-full text-sm font-medium ${
                employee.is_active
                  ? 'bg-green-50 text-green-700 dark:bg-emerald-500/15 dark:text-emerald-300'
                  : 'bg-red-50 text-red-700 dark:bg-red-500/15 dark:text-red-300'
              }`}
            >
              {employee.is_active ? t('common.active') : t('common.inactive')}
            </span>
            {canManageEmployees && (
              <button
                type="button"
                onClick={() => setShowEdit(true)}
                className="btn-primary w-full sm:w-auto"
              >
                <Pencil className="w-4 h-4" aria-hidden="true" />
                {t('employees.editEmployee')}
              </button>
            )}
          </div>
        }
      />

      <div className={`grid grid-cols-1 gap-6 ${canViewPayroll ? 'md:grid-cols-2 xl:grid-cols-3' : 'md:grid-cols-2'}`}>
        {/* Personal Info */}
        <section className="card">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4 flex items-center gap-2">
            <User className="w-4 h-4" aria-hidden="true" /> {t('employees.sections.personal')}
          </h2>
          <div className="space-y-3">
            <InfoField label={t('employees.fields.nric')} value={employee.ic_number} restricted={!showPersonalSensitive} />
            <InfoField label={t('employees.fields.dateOfBirth')} value={employee.date_of_birth ? formatDate(employee.date_of_birth) : null} restricted={!showPersonalSensitive} />
            <InfoField label={t('employees.fields.gender')} value={employee.gender ? t(`enums.gender.${employee.gender}`, { defaultValue: employee.gender }) : null} />
            <InfoField label={t('employees.fields.race')} value={employee.race ? t(`enums.race.${employee.race}`, { defaultValue: employee.race }) : null} />
            <InfoField label={t('employees.fields.nationality')} value={employee.nationality} />
            <InfoField label={t('employees.fields.maritalStatus')} value={employee.marital_status ? t(`enums.maritalStatus.${employee.marital_status}`, { defaultValue: employee.marital_status }) : null} />
            <InfoField label={t('employees.fields.email')} value={employee.email} />
            <InfoField label={t('employees.fields.phone')} value={employee.phone} restricted={!showPersonalSensitive} />
            {(addressParts.length > 0 || !showPersonalSensitive) && (
              <div className="pt-1 border-t border-gray-100 dark:border-slate-800">
                <InfoField
                  label={t('employees.sections.address')}
                  value={addressParts.join(', ')}
                  restricted={!showPersonalSensitive}
                />
              </div>
            )}
          </div>
        </section>

        {/* Employment & Salary */}
        <section className="card">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4 flex items-center gap-2">
            <DollarSign className="w-4 h-4" aria-hidden="true" /> {t('employees.sections.employment')}
          </h2>
          <div className="space-y-3">
            <InfoField label={t('employees.fields.employmentType')} value={empType} />
            <InfoField label={t('employees.fields.dateJoined')} value={formatDate(employee.date_joined)} />
            <InfoField label={t('employees.fields.confirmationDate')} value={employee.confirmation_date ? formatDate(employee.confirmation_date) : null} />
            {/* A termination should be visible without opening the edit modal —
                it decides whether payroll still owes them a final payslip. */}
            <InfoField label={t('employees.fields.dateResigned')} value={employee.date_resigned ? formatDate(employee.date_resigned) : null} />
            {employee.resignation_reason && (
              <InfoField label={t('employees.fields.resignationReason')} value={employee.resignation_reason} />
            )}
            {canViewPayroll && <InfoField label={t('employees.fields.basicSalary')} value={formatMYR(employee.basic_salary)} />}
            <InfoField label={t('employees.fields.bank')} value={employee.bank_name} restricted={!canViewPayroll} />
            <InfoField label={t('employees.fields.accountNo')} value={employee.bank_account_number} restricted={!canViewPayroll} />
            <InfoField label={t('employees.fields.costCentre')} value={employee.cost_centre} />
            <InfoField label={t('employees.fields.branch')} value={employee.branch} />
          </div>
        </section>

        {canViewPayroll && (
          <section className="card md:col-span-2 xl:col-span-1">
            <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4 flex items-center gap-2">
              <Shield className="w-4 h-4" aria-hidden="true" /> {t('employees.sections.statutory')}
            </h2>
            <div className="space-y-3">
              <InfoField label={t('employees.fields.tin')} value={employee.tax_identification_number} />
              <InfoField label={t('employees.fields.epfNumber')} value={employee.epf_number} />
              <InfoField
                label={t('employees.fields.epfCategory')}
                value={
                  employee.epf_category
                    ? t(`enums.epfCategory.${employee.epf_category}`, { defaultValue: employee.epf_category })
                    : null
                }
              />
              <InfoField label={t('employees.fields.socsoNumber')} value={employee.socso_number} />
              <InfoField label={t('employees.fields.eisNumber')} value={employee.eis_number} />
              <InfoField label={t('employees.fields.residency')} value={residency} />
              <InfoField label={t('employees.fields.workingSpouse')} value={employee.working_spouse ? t('common.yes') : t('common.no')} />
              <InfoField label={t('employees.fields.children')} value={String(employee.num_children ?? 0)} />
              <InfoField label={t('employees.fields.muslim')} value={employee.is_muslim ? t('common.yes') : t('common.no')} />
              {employee.zakat_eligible && (
                <InfoField label={t('employees.fields.zakatMonthly')} value={formatMYR(employee.zakat_monthly_amount ?? 0)} />
              )}
              {(employee.ptptn_monthly_amount ?? 0) > 0 && (
                <InfoField label={t('employees.fields.ptptnMonthly')} value={formatMYR(employee.ptptn_monthly_amount!)} />
              )}
            </div>
          </section>
        )}
      </div>

      {/* Salary History */}
      {canViewSalaryHistory && (loadingSalaryHistory || salaryHistoryError || (salaryHistory && salaryHistory.length > 0)) && (
        <section className="card">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4 flex items-center gap-2">
            <DollarSign className="w-4 h-4" aria-hidden="true" /> {t('employees.sections.salaryHistory')}
          </h2>
          {loadingSalaryHistory ? (
            <div role="status" aria-busy="true" aria-label={t('common.loading')} className="space-y-3">
              <Skeleton className="h-10 w-full rounded-lg" />
              <Skeleton className="h-10 w-full rounded-lg" />
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
              <div className="sm:hidden space-y-3">
                {salaryHistory!.map((h) => {
                  const pctChange = h.old_salary > 0
                    ? ((h.new_salary - h.old_salary) / h.old_salary) * 100
                    : null;
                  return (
                    <div key={h.id} className="rounded-xl border border-gray-100 dark:border-slate-800 p-3.5 space-y-2 bg-gray-50/60 dark:bg-slate-800/40">
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-xs font-medium text-gray-500 dark:text-slate-400 tabular-nums">{formatDate(h.effective_date)}</span>
                        {pctChange !== null && (
                          <span className={`text-xs font-semibold tabular-nums ${pctChange >= 0 ? 'text-green-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'}`}>
                            {pctChange >= 0 ? '+' : ''}{pctChange.toFixed(1)}%
                          </span>
                        )}
                      </div>
                      <div className="flex items-baseline justify-between gap-2 text-sm tabular-nums">
                        <span className="text-gray-500 dark:text-slate-400">{formatMYR(h.old_salary)}</span>
                        <span className="text-gray-400 dark:text-slate-500" aria-hidden="true">{'\u2192'}</span>
                        <span className="font-semibold text-gray-900 dark:text-slate-100">{formatMYR(h.new_salary)}</span>
                      </div>
                      {(h.change_type || h.notes || h.reason) && (
                        <div className="flex flex-wrap items-center gap-1.5 text-xs text-gray-600 dark:text-slate-300">
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
              <div className="hidden sm:block overflow-x-auto rounded-xl border border-gray-100 dark:border-slate-800">
                <table className="w-full text-sm">
                  <thead className="bg-gray-50 dark:bg-slate-800/70">
                    <tr>
                      <th className="text-left px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('employees.salaryTable.date')}</th>
                      <th className="text-right px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('employees.salaryTable.oldSalary')}</th>
                      <th className="text-right px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('employees.salaryTable.newSalary')}</th>
                      <th className="text-right px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase whitespace-nowrap">{t('employees.salaryTable.change')}</th>
                      <th className="text-left px-4 py-2.5 text-xs font-medium text-gray-500 dark:text-slate-400 uppercase">{t('employees.salaryTable.reason')}</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-gray-100 dark:divide-slate-800">
                    {salaryHistory!.map((h) => {
                      const pctChange = h.old_salary > 0
                        ? ((h.new_salary - h.old_salary) / h.old_salary) * 100
                        : null;
                      return (
                        <tr key={h.id}>
                          <td className="px-4 py-3 text-gray-700 dark:text-slate-300 whitespace-nowrap tabular-nums">{formatDate(h.effective_date)}</td>
                          <td className="px-4 py-3 text-right text-gray-600 dark:text-slate-400 tabular-nums whitespace-nowrap">{formatMYR(h.old_salary)}</td>
                          <td className="px-4 py-3 text-right font-medium text-gray-900 dark:text-slate-100 tabular-nums whitespace-nowrap">{formatMYR(h.new_salary)}</td>
                          <td className="px-4 py-3 text-right tabular-nums whitespace-nowrap">
                            {pctChange === null ? (
                              <span className="text-gray-400 dark:text-slate-500">{'\u2014'}</span>
                            ) : (
                              <span className={pctChange >= 0 ? 'text-green-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'}>
                                {pctChange >= 0 ? '+' : ''}{pctChange.toFixed(1)}%
                              </span>
                            )}
                          </td>
                          <td className="px-4 py-3 text-gray-500 dark:text-slate-400">
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

      {showEdit && (
        <EmployeeFormModal
          mode="edit"
          employeeId={employee.id}
          onClose={() => setShowEdit(false)}
        />
      )}
    </div>
  );
}
