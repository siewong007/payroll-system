import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { DollarSign, Edit, MapPin, Shield, TrendingDown, TrendingUp } from 'lucide-react';
import { getEmployee, getSalaryHistory } from '@/api/employees';
import { useAuth } from '@/context/AuthContext';
import { canAccessPayrollData } from '@/lib/roles';
import { formatDate, formatMYR } from '@/lib/utils';

function InfoField({ label, value }: { label: string; value: string | null | undefined }) {
  return (
    <div>
      <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
      <p className="text-sm font-medium text-gray-900 mt-0.5">{value || '-'}</p>
    </div>
  );
}

export function EmployeeProfile({ employeeId }: { employeeId: string }) {
  const { t } = useTranslation();
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user);
  const { data: employee, isLoading } = useQuery({
    queryKey: ['employee', employeeId],
    queryFn: () => getEmployee(employeeId),
  });

  const { data: salaryHistory } = useQuery({
    queryKey: ['salaryHistory', employeeId],
    queryFn: () => getSalaryHistory(employeeId),
    enabled: canViewPayroll,
  });

  if (isLoading || !employee) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-7">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-lg font-bold text-gray-900">{employee.full_name}</h3>
          <p className="text-sm text-gray-500">
            {employee.employee_number} &middot;{' '}
            {employee.department || t('employees.noDepartment')} &middot;{' '}
            {employee.designation || t('employees.noDesignation')}
          </p>
        </div>
        <span
          className={`inline-flex px-2.5 py-1 rounded-full text-xs font-medium ${
            employee.is_active ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'
          }`}
        >
          {employee.is_active ? t('common.active') : t('common.inactive')}
        </span>
      </div>

      {/* Personal */}
      <section>
        <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
          <Edit className="w-3.5 h-3.5" /> {t('employees.sections.personal')}
        </h4>
        <div className="grid grid-cols-2 gap-x-8 gap-y-4">
          <InfoField label={t('employees.fields.nric')} value={employee.ic_number} />
          <InfoField
            label={t('employees.fields.dateOfBirth')}
            value={employee.date_of_birth ? formatDate(employee.date_of_birth) : null}
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
          <InfoField label={t('employees.fields.phone')} value={employee.phone} />
        </div>
      </section>

      {/* Address */}
      {(employee.address_line1 || employee.city || employee.state) && (
        <section>
          <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
            <MapPin className="w-3.5 h-3.5" /> {t('employees.sections.address')}
          </h4>
          <p className="text-sm text-gray-900">
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
        </section>
      )}

      {/* Employment & Salary */}
      <section>
        <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
          <DollarSign className="w-3.5 h-3.5" /> {t('employees.sections.employment')}
        </h4>
        <div className="grid grid-cols-2 gap-x-8 gap-y-4">
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
          {canViewPayroll && (
            <InfoField
              label={t('employees.fields.basicSalary')}
              value={formatMYR(employee.basic_salary)}
            />
          )}
          <InfoField label={t('employees.fields.bank')} value={employee.bank_name} />
          <InfoField label={t('employees.fields.accountNo')} value={employee.bank_account_number} />
          <InfoField label={t('employees.fields.costCentre')} value={employee.cost_centre} />
          <InfoField label={t('employees.fields.branch')} value={employee.branch} />
        </div>
      </section>

      {canViewPayroll && (
        <section>
          <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
            <Shield className="w-3.5 h-3.5" /> {t('employees.sections.statutory')}
          </h4>
          <div className="grid grid-cols-2 gap-x-8 gap-y-4">
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
      {canViewPayroll && salaryHistory && salaryHistory.length > 0 && (
        <section>
          <h4 className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-2">
            <DollarSign className="w-3.5 h-3.5" /> {t('employees.sections.salaryHistory')}
          </h4>
          <div className="rounded-2xl shadow overflow-hidden">
            <table className="w-full text-sm">
              <thead className="bg-gray-50">
                <tr>
                  <th className="text-left px-4 py-2 text-xs font-medium text-gray-500 uppercase">
                    {t('employees.salaryTable.date')}
                  </th>
                  <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">
                    {t('employees.salaryTable.old')}
                  </th>
                  <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">
                    {t('employees.salaryTable.new')}
                  </th>
                  <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">
                    {t('employees.salaryTable.change')}
                  </th>
                  <th className="text-left px-4 py-2 text-xs font-medium text-gray-500 uppercase">
                    {t('employees.salaryTable.reason')}
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {salaryHistory.map((h) => {
                  const pctChange = ((h.new_salary - h.old_salary) / h.old_salary) * 100;
                  const isIncrease = pctChange >= 0;
                  return (
                    <tr key={h.id}>
                      <td className="px-4 py-2.5">{formatDate(h.effective_date)}</td>
                      <td className="px-4 py-2.5 text-right text-gray-500">
                        {formatMYR(h.old_salary)}
                      </td>
                      <td className="px-4 py-2.5 text-right font-medium">
                        {formatMYR(h.new_salary)}
                      </td>
                      <td className="px-4 py-2.5 text-right">
                        <span
                          className={`inline-flex items-center gap-0.5 ${
                            isIncrease ? 'text-green-600' : 'text-red-600'
                          }`}
                        >
                          {isIncrease ? (
                            <TrendingUp className="w-3 h-3" />
                          ) : (
                            <TrendingDown className="w-3 h-3" />
                          )}
                          {isIncrease ? '+' : ''}
                          {pctChange.toFixed(1)}%
                        </span>
                      </td>
                      <td className="px-4 py-2.5 text-gray-500">{h.reason || '-'}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}
