import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, UserCheck, AlertTriangle, X } from 'lucide-react';
import { createEmployee } from '@/api/employees';
import type { EmployeeAccountInfo } from '@/api/employees';
import { getPayrollGroups } from '@/api/payroll';
import { getErrorMessage } from '@/lib/utils';
import type { CreateEmployeeRequest } from '@/types';
import { useAuth } from '@/context/AuthContext';
import { canAccessPayrollData } from '@/lib/roles';

const BANKS = [
  'Maybank', 'CIMB Bank', 'Public Bank', 'RHB Bank', 'Hong Leong Bank',
  'AmBank', 'Bank Islam', 'Bank Rakyat', 'Alliance Bank', 'Affin Bank',
  'BSN', 'OCBC Bank', 'Standard Chartered', 'HSBC', 'UOB', 'Agro Bank',
  'Bank Muamalat', 'MBSB Bank', 'Al Rajhi Bank',
];

export function EmployeeCreate() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user?.role);

  const { data: payrollGroups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
    enabled: canViewPayroll,
  });

  const [form, setForm] = useState<CreateEmployeeRequest>({
    employee_number: '',
    full_name: '',
    date_joined: new Date().toISOString().split('T')[0],
    basic_salary: 0,
  });

  const [salaryDisplay, setSalaryDisplay] = useState('');
  const [accountDialog, setAccountDialog] = useState<EmployeeAccountInfo | null>(null);

  const mutation = useMutation({
    mutationFn: createEmployee,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      if (data.account) {
        setAccountDialog(data.account);
      } else {
        navigate('/employees');
      }
    },
  });

  const updateField = (field: string, value: unknown) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  const handleSalaryChange = (value: string) => {
    setSalaryDisplay(value);
    const ringgit = parseFloat(value) || 0;
    updateField('basic_salary', Math.round(ringgit * 100)); // Convert to sen
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate(form);
  };

  return (
    <div>
      <button
        onClick={() => navigate('/employees')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4"
      >
        <ArrowLeft className="w-4 h-4" /> Back to Employees
      </button>

      <h1 className="text-2xl font-bold text-gray-900 mb-6">Add New Employee</h1>

      <form onSubmit={handleSubmit} className="space-y-8 max-w-4xl">
        {mutation.isError && (
          <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-lg">
            {getErrorMessage(mutation.error, 'Failed to create employee')}
          </div>
        )}

        {/* Personal Information */}
        <section className="bg-white rounded-2xl shadow p-6">
          <h2 className="text-lg font-semibold mb-4">Personal Information</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Employee Number *
              </label>
              <input
                type="text"
                value={form.employee_number}
                onChange={(e) => updateField('employee_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                required
                placeholder="e.g., EMP001"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Full Name (as per NRIC) *
              </label>
              <input
                type="text"
                value={form.full_name}
                onChange={(e) => updateField('full_name', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                required
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">NRIC / IC Number</label>
              <input
                type="text"
                value={form.ic_number || ''}
                onChange={(e) => updateField('ic_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                placeholder="e.g., 900101-14-5678"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Date of Birth</label>
              <input
                type="date"
                value={form.date_of_birth || ''}
                onChange={(e) => updateField('date_of_birth', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Gender</label>
              <select
                value={form.gender || ''}
                onChange={(e) => updateField('gender', e.target.value || undefined)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="">Select</option>
                <option value="male">Male</option>
                <option value="female">Female</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Race</label>
              <select
                value={form.race || ''}
                onChange={(e) => updateField('race', e.target.value || undefined)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="">Select</option>
                <option value="malay">Malay</option>
                <option value="chinese">Chinese</option>
                <option value="indian">Indian</option>
                <option value="other">Other</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Marital Status</label>
              <select
                value={form.marital_status || ''}
                onChange={(e) => updateField('marital_status', e.target.value || undefined)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="">Select</option>
                <option value="single">Single</option>
                <option value="married">Married</option>
                <option value="divorced">Divorced</option>
                <option value="widowed">Widowed</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Email</label>
              <input
                type="email"
                value={form.email || ''}
                onChange={(e) => updateField('email', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Phone</label>
              <input
                type="tel"
                value={form.phone || ''}
                onChange={(e) => updateField('phone', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
          </div>
        </section>

        {/* Employment */}
        <section className="bg-white rounded-2xl shadow p-6">
          <h2 className="text-lg font-semibold mb-4">Employment Details</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Department</label>
              <input
                type="text"
                value={form.department || ''}
                onChange={(e) => updateField('department', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Designation</label>
              <input
                type="text"
                value={form.designation || ''}
                onChange={(e) => updateField('designation', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Employment Type</label>
              <select
                value={form.employment_type || 'permanent'}
                onChange={(e) => updateField('employment_type', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="permanent">Permanent</option>
                <option value="contract">Contract</option>
                <option value="part_time">Part Time</option>
                <option value="intern">Intern</option>
                <option value="daily_rated">Daily Rated</option>
                <option value="hourly_rated">Hourly Rated</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Date Joined *</label>
              <input
                type="date"
                value={form.date_joined}
                onChange={(e) => updateField('date_joined', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                required
              />
            </div>
            {canViewPayroll && (
              <>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Basic Salary (RM) *</label>
                  <input
                    type="number"
                    step="0.01"
                    value={salaryDisplay}
                    onChange={(e) => handleSalaryChange(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                    required
                    placeholder="e.g., 3000.00"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">Payroll Group</label>
                  <select
                    value={form.payroll_group_id || ''}
                    onChange={(e) => updateField('payroll_group_id', e.target.value || undefined)}
                    className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                  >
                    <option value="">Select</option>
                    {payrollGroups?.map((g) => (
                      <option key={g.id} value={g.id}>
                        {g.name}
                      </option>
                    ))}
                  </select>
                </div>
              </>
            )}
          </div>
        </section>

        {/* Banking */}
        <section className="bg-white rounded-2xl shadow p-6">
          <h2 className="text-lg font-semibold mb-4">Banking Details</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Bank Name</label>
              <select
                value={form.bank_name || ''}
                onChange={(e) => updateField('bank_name', e.target.value || undefined)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="">Select Bank</option>
                {BANKS.map((b) => (
                  <option key={b} value={b}>{b}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Account Number</label>
              <input
                type="text"
                value={form.bank_account_number || ''}
                onChange={(e) => updateField('bank_account_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
          </div>
        </section>

        {canViewPayroll && (
          <section className="bg-white rounded-2xl shadow p-6">
            <h2 className="text-lg font-semibold mb-4">Statutory & Tax</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">EPF Number</label>
              <input
                type="text"
                value={form.epf_number || ''}
                onChange={(e) => updateField('epf_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">SOCSO Number</label>
              <input
                type="text"
                value={form.socso_number || ''}
                onChange={(e) => updateField('socso_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">EIS Number</label>
              <input
                type="text"
                value={form.eis_number || ''}
                onChange={(e) => updateField('eis_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">EPF Category</label>
              <select
                value={form.epf_category || 'A'}
                onChange={(e) => updateField('epf_category', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="A">A - Citizen/PR below 60</option>
                <option value="B">B - Elected 9% (KWSP 17A)</option>
                <option value="C">C - PR 60 and above</option>
                <option value="D">D - Citizen 60 and above</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Residency Status</label>
              <select
                value={form.residency_status || 'citizen'}
                onChange={(e) => updateField('residency_status', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="citizen">Malaysian Citizen</option>
                <option value="permanent_resident">Permanent Resident</option>
                <option value="foreigner">Foreigner</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Working Spouse</label>
              <select
                value={form.working_spouse ? 'yes' : 'no'}
                onChange={(e) => updateField('working_spouse', e.target.value === 'yes')}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="no">No</option>
                <option value="yes">Yes</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Number of Children</label>
              <input
                type="number"
                min="0"
                value={form.num_children ?? 0}
                onChange={(e) => updateField('num_children', parseInt(e.target.value) || 0)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
              <div className="flex items-center gap-4 pt-6">
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={form.is_muslim || false}
                    onChange={(e) => {
                      updateField('is_muslim', e.target.checked);
                      if (!e.target.checked) {
                        updateField('zakat_eligible', false);
                        updateField('zakat_monthly_amount', undefined);
                      }
                    }}
                    className="rounded"
                  />
                  <span className="text-sm">Muslim</span>
                </label>
                {form.is_muslim && (
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={form.zakat_eligible || false}
                      onChange={(e) => updateField('zakat_eligible', e.target.checked)}
                      className="rounded"
                    />
                    <span className="text-sm">Zakat Eligible</span>
                  </label>
                )}
              </div>
            </div>
          </section>
        )}

        {/* Submit */}
        <div className="flex gap-4">
          <button
            type="submit"
            disabled={mutation.isPending}
            className="bg-black text-white px-6 py-2.5 rounded-lg font-medium hover:bg-gray-800 disabled:opacity-50 transition-colors"
          >
            {mutation.isPending ? 'Creating...' : 'Create Employee'}
          </button>
          <button
            type="button"
            onClick={() => navigate('/employees')}
            className="bg-white text-gray-700 px-6 py-2.5 rounded-lg font-medium border border-gray-200 hover:bg-gray-50 transition-colors"
          >
            Cancel
          </button>
        </div>
      </form>

      {/* Account Created Dialog */}
      {accountDialog && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-md mx-4">
            <div className="flex items-center justify-between p-6 border-b border-gray-100">
              <div className="flex items-center gap-2">
                {accountDialog.created ? (
                  <UserCheck className="w-5 h-5 text-green-600" />
                ) : (
                  <AlertTriangle className="w-5 h-5 text-amber-500" />
                )}
                <h2 className="text-lg font-semibold text-gray-900">
                  {accountDialog.created ? 'User Account Created' : 'Account Notice'}
                </h2>
              </div>
              <button
                onClick={() => navigate('/employees')}
                className="text-gray-400 hover:text-gray-700"
              >
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="p-6 space-y-4">
              {accountDialog.created ? (
                <>
                  <p className="text-sm text-gray-600">
                    A user account has been created and a welcome email has been sent to the employee.
                  </p>
                  <div className="bg-gray-50 rounded-xl p-4 space-y-3">
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-500">Email</span>
                      <span className="font-medium text-gray-900">{accountDialog.email}</span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-500">Role</span>
                      <span className="font-medium text-gray-900 capitalize">{accountDialog.role}</span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-500">Default Password</span>
                      <span className="font-mono font-medium text-gray-900">{accountDialog.default_password}</span>
                    </div>
                  </div>
                  <p className="text-xs text-amber-600 bg-amber-50 rounded-lg px-3 py-2">
                    Please advise the employee to change their password upon first login.
                  </p>
                </>
              ) : (
                <div className="bg-amber-50 rounded-xl p-4">
                  <p className="text-sm text-amber-800">{accountDialog.message}</p>
                </div>
              )}
            </div>
            <div className="flex justify-end p-6 border-t border-gray-100">
              <button
                onClick={() => navigate('/employees')}
                className="btn-primary"
              >
                Done
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
