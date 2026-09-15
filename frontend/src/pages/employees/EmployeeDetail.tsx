import { useParams, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { ArrowLeft, Edit, DollarSign, Shield } from 'lucide-react';
import { getEmployee, getSalaryHistory } from '@/api/employees';
import { formatMYR, formatDate } from '@/lib/utils';
import { useAuth } from '@/context/AuthContext';
import { canAccessPayrollData } from '@/lib/roles';

const InfoField = ({ label, value }: { label: string; value: string | null | undefined }) => (
  <div>
    <p className="text-xs text-gray-400 uppercase tracking-wide">{label}</p>
    <p className="text-sm font-medium mt-0.5">{value || '-'}</p>
  </div>
);

export function EmployeeDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user);

  const { data: employee, isLoading } = useQuery({
    queryKey: ['employee', id],
    queryFn: () => getEmployee(id!),
    enabled: !!id,
  });

  const { data: salaryHistory } = useQuery({
    queryKey: ['salaryHistory', id],
    queryFn: () => getSalaryHistory(id!),
    enabled: !!id && canViewPayroll,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
      </div>
    );
  }

  if (!employee) {
    return <div className="text-center text-gray-500 py-12">{t('employees.notFound')}</div>;
  }

  const empType = employee.employment_type
    ? t(`enums.employmentType.${employee.employment_type}`, { defaultValue: employee.employment_type.replaceAll('_', ' ') })
    : null;
  const residency = employee.residency_status
    ? t(`enums.residency.${employee.residency_status}`, { defaultValue: employee.residency_status.replaceAll('_', ' ') })
    : null;

  return (
    <div>
      <button
        onClick={() => navigate('/employees')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4"
      >
        <ArrowLeft className="w-4 h-4" /> {t('employees.backToList')}
      </button>

      {/* Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold text-gray-900">{employee.full_name}</h1>
          <p className="text-gray-500">
            {employee.employee_number} &middot; {employee.department || t('employees.noDepartment')} &middot;{' '}
            {employee.designation || t('employees.noDesignation')}
          </p>
        </div>
        <span
          className={`px-3 py-1 rounded-full text-sm font-medium ${
            employee.is_active ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'
          }`}
        >
          {employee.is_active ? t('common.active') : t('common.inactive')}
        </span>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Personal Info */}
        <div className="bg-white rounded-2xl shadow p-6">
          <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
            <Edit className="w-4 h-4" /> {t('employees.sections.personal')}
          </h2>
          <div className="space-y-3">
            <InfoField label={t('employees.fields.nric')} value={employee.ic_number} />
            <InfoField label={t('employees.fields.dateOfBirth')} value={employee.date_of_birth ? formatDate(employee.date_of_birth) : null} />
            <InfoField label={t('employees.fields.gender')} value={employee.gender ? t(`enums.gender.${employee.gender}`, { defaultValue: employee.gender }) : null} />
            <InfoField label={t('employees.fields.race')} value={employee.race ? t(`enums.race.${employee.race}`, { defaultValue: employee.race }) : null} />
            <InfoField label={t('employees.fields.nationality')} value={employee.nationality} />
            <InfoField label={t('employees.fields.maritalStatus')} value={employee.marital_status ? t(`enums.maritalStatus.${employee.marital_status}`, { defaultValue: employee.marital_status }) : null} />
            <InfoField label={t('employees.fields.email')} value={employee.email} />
            <InfoField label={t('employees.fields.phone')} value={employee.phone} />
          </div>
        </div>

        {/* Employment & Salary */}
        <div className="bg-white rounded-2xl shadow p-6">
          <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
            <DollarSign className="w-4 h-4" /> {t('employees.sections.employment')}
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
            <InfoField label={t('employees.fields.bank')} value={employee.bank_name} />
            <InfoField label={t('employees.fields.accountNo')} value={employee.bank_account_number} />
            <InfoField label={t('employees.fields.costCentre')} value={employee.cost_centre} />
            <InfoField label={t('employees.fields.branch')} value={employee.branch} />
          </div>
        </div>

        {canViewPayroll && (
          <div className="bg-white rounded-2xl shadow p-6">
            <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
              <Shield className="w-4 h-4" /> {t('employees.sections.statutory')}
            </h2>
            <div className="space-y-3">
              <InfoField label={t('employees.fields.tin')} value={employee.tax_identification_number} />
              <InfoField label={t('employees.fields.epfNumber')} value={employee.epf_number} />
              <InfoField label={t('employees.fields.epfCategory')} value={employee.epf_category} />
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
          </div>
        )}
      </div>

      {/* Salary History */}
      {canViewPayroll && salaryHistory && salaryHistory.length > 0 && (
        <div className="bg-white rounded-2xl shadow p-6 mt-6">
          <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
            <DollarSign className="w-4 h-4" /> {t('employees.sections.salaryHistory')}
          </h2>
          <table className="w-full">
            <thead className="bg-gray-50">
              <tr>
                <th className="text-left px-4 py-2 text-xs font-medium text-gray-500 uppercase">{t('employees.salaryTable.date')}</th>
                <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">{t('employees.salaryTable.oldSalary')}</th>
                <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">{t('employees.salaryTable.newSalary')}</th>
                <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">{t('employees.salaryTable.change')}</th>
                <th className="text-left px-4 py-2 text-xs font-medium text-gray-500 uppercase">{t('employees.salaryTable.reason')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {salaryHistory.map((h) => {
                const pctChange = ((h.new_salary - h.old_salary) / h.old_salary) * 100;
                return (
                  <tr key={h.id}>
                    <td className="px-4 py-3 text-sm">{formatDate(h.effective_date)}</td>
                    <td className="px-4 py-3 text-sm text-right">{formatMYR(h.old_salary)}</td>
                    <td className="px-4 py-3 text-sm text-right font-medium">{formatMYR(h.new_salary)}</td>
                    <td className="px-4 py-3 text-sm text-right">
                      <span className={pctChange >= 0 ? 'text-green-600' : 'text-red-600'}>
                        {pctChange >= 0 ? '+' : ''}{pctChange.toFixed(1)}%
                      </span>
                    </td>
                    <td className="px-4 py-3 text-sm text-gray-500">{h.reason || '-'}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

    </div>
  );
}
