import { useParams, useNavigate } from 'react-router-dom';
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
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user?.role);

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
    return <div className="text-center text-gray-500 py-12">Employee not found</div>;
  }

  return (
    <div>
      <button
        onClick={() => navigate('/employees')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4"
      >
        <ArrowLeft className="w-4 h-4" /> Back to Employees
      </button>

      {/* Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-6">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold text-gray-900">{employee.full_name}</h1>
          <p className="text-gray-500">
            {employee.employee_number} &middot; {employee.department || 'No Department'} &middot;{' '}
            {employee.designation || 'No Designation'}
          </p>
        </div>
        <span
          className={`px-3 py-1 rounded-full text-sm font-medium ${
            employee.is_active ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'
          }`}
        >
          {employee.is_active ? 'Active' : 'Inactive'}
        </span>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Personal Info */}
        <div className="bg-white rounded-2xl shadow p-6">
          <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
            <Edit className="w-4 h-4" /> Personal
          </h2>
          <div className="space-y-3">
            <InfoField label="NRIC" value={employee.ic_number} />
            <InfoField label="Date of Birth" value={employee.date_of_birth ? formatDate(employee.date_of_birth) : null} />
            <InfoField label="Gender" value={employee.gender} />
            <InfoField label="Race" value={employee.race} />
            <InfoField label="Nationality" value={employee.nationality} />
            <InfoField label="Marital Status" value={employee.marital_status} />
            <InfoField label="Email" value={employee.email} />
            <InfoField label="Phone" value={employee.phone} />
          </div>
        </div>

        {/* Employment & Salary */}
        <div className="bg-white rounded-2xl shadow p-6">
          <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
            <DollarSign className="w-4 h-4" /> Employment & Salary
          </h2>
          <div className="space-y-3">
            <InfoField label="Employment Type" value={employee.employment_type?.replace('_', ' ')} />
            <InfoField label="Date Joined" value={formatDate(employee.date_joined)} />
            <InfoField label="Confirmation Date" value={employee.confirmation_date ? formatDate(employee.confirmation_date) : null} />
            {canViewPayroll && <InfoField label="Basic Salary" value={formatMYR(employee.basic_salary)} />}
            <InfoField label="Bank" value={employee.bank_name} />
            <InfoField label="Account No" value={employee.bank_account_number} />
            <InfoField label="Cost Centre" value={employee.cost_centre} />
            <InfoField label="Branch" value={employee.branch} />
          </div>
        </div>

        {canViewPayroll && (
          <div className="bg-white rounded-2xl shadow p-6">
            <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
              <Shield className="w-4 h-4" /> Statutory
            </h2>
            <div className="space-y-3">
              <InfoField label="TIN" value={employee.tax_identification_number} />
              <InfoField label="EPF Number" value={employee.epf_number} />
              <InfoField label="EPF Category" value={employee.epf_category} />
              <InfoField label="SOCSO Number" value={employee.socso_number} />
              <InfoField label="EIS Number" value={employee.eis_number} />
              <InfoField label="Residency" value={employee.residency_status?.replace('_', ' ')} />
              <InfoField label="Working Spouse" value={employee.working_spouse ? 'Yes' : 'No'} />
              <InfoField label="Children" value={String(employee.num_children ?? 0)} />
              <InfoField label="Muslim" value={employee.is_muslim ? 'Yes' : 'No'} />
              {employee.zakat_eligible && (
                <InfoField label="Zakat (Monthly)" value={formatMYR(employee.zakat_monthly_amount ?? 0)} />
              )}
              {(employee.ptptn_monthly_amount ?? 0) > 0 && (
                <InfoField label="PTPTN (Monthly)" value={formatMYR(employee.ptptn_monthly_amount!)} />
              )}
            </div>
          </div>
        )}
      </div>

      {/* Salary History */}
      {canViewPayroll && salaryHistory && salaryHistory.length > 0 && (
        <div className="bg-white rounded-2xl shadow p-6 mt-6">
          <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
            <DollarSign className="w-4 h-4" /> Salary History
          </h2>
          <table className="w-full">
            <thead className="bg-gray-50">
              <tr>
                <th className="text-left px-4 py-2 text-xs font-medium text-gray-500 uppercase">Date</th>
                <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">Old Salary</th>
                <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">New Salary</th>
                <th className="text-right px-4 py-2 text-xs font-medium text-gray-500 uppercase">Change</th>
                <th className="text-left px-4 py-2 text-xs font-medium text-gray-500 uppercase">Reason</th>
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
