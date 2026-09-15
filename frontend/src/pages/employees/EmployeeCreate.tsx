import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, UserCheck, AlertTriangle, X } from 'lucide-react';
import { createEmployee } from '@/api/employees';
import type { EmployeeAccountInfo } from '@/api/employees';
import { getPayrollGroups } from '@/api/payroll';
import { getErrorMessage, todayLocalDate } from '@/lib/utils';
import { stripPayrollFields } from '@/lib/employeeFields';
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
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user);

  const { data: payrollGroups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
    enabled: canViewPayroll,
  });

  const [form, setForm] = useState<CreateEmployeeRequest>({
    employee_number: '',
    full_name: '',
    date_joined: todayLocalDate(),
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
    // Without `view_payroll` the server rejects the whole create if a single
    // payroll-sensitive field is present, so an hr_manager sends none of them.
    // `basic_salary` is required by the request type and `0` is what the
    // backend reads as "not supplied".
    mutation.mutate(
      canViewPayroll
        ? form
        : { ...stripPayrollFields(form), basic_salary: 0 } as CreateEmployeeRequest,
    );
  };

  return (
    <div>
      <button
        onClick={() => navigate('/employees')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4"
      >
        <ArrowLeft className="w-4 h-4" /> {t('employees.backToList')}
      </button>

      <h1 className="text-2xl font-bold text-gray-900 mb-6">{t('employees.createTitle')}</h1>

      <form onSubmit={handleSubmit} className="space-y-8 max-w-4xl">
        {mutation.isError && (
          <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-lg">
            {getErrorMessage(mutation.error, t('employees.createFailed'))}
          </div>
        )}

        {/* Personal Information */}
        <section className="bg-white rounded-2xl shadow p-6">
          <h2 className="text-lg font-semibold mb-4">{t('employees.form.personalInfo')}</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                {t('employees.form.employeeNumber')} *
              </label>
              <input
                type="text"
                value={form.employee_number}
                onChange={(e) => updateField('employee_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                required
                placeholder={t('employees.form.employeeNumberPlaceholder')}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                {t('employees.form.fullName')} *
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
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.form.icNumber')}</label>
              <input
                type="text"
                value={form.ic_number || ''}
                onChange={(e) => updateField('ic_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                placeholder={t('employees.form.icNumberPlaceholder')}
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.dateOfBirth')}</label>
              <input
                type="date"
                value={form.date_of_birth || ''}
                onChange={(e) => updateField('date_of_birth', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.gender')}</label>
              <select
                value={form.gender || ''}
                onChange={(e) => updateField('gender', e.target.value || undefined)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="">{t('common.select')}</option>
                <option value="male">{t('enums.gender.male')}</option>
                <option value="female">{t('enums.gender.female')}</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.race')}</label>
              <select
                value={form.race || ''}
                onChange={(e) => updateField('race', e.target.value || undefined)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="">{t('common.select')}</option>
                <option value="malay">{t('enums.race.malay')}</option>
                <option value="chinese">{t('enums.race.chinese')}</option>
                <option value="indian">{t('enums.race.indian')}</option>
                <option value="other">{t('enums.race.other')}</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.maritalStatus')}</label>
              <select
                value={form.marital_status || ''}
                onChange={(e) => updateField('marital_status', e.target.value || undefined)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="">{t('common.select')}</option>
                <option value="single">{t('enums.maritalStatus.single')}</option>
                <option value="married">{t('enums.maritalStatus.married')}</option>
                <option value="divorced">{t('enums.maritalStatus.divorced')}</option>
                <option value="widowed">{t('enums.maritalStatus.widowed')}</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('common.email')}</label>
              <input
                type="email"
                value={form.email || ''}
                onChange={(e) => updateField('email', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.phone')}</label>
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
          <h2 className="text-lg font-semibold mb-4">{t('employees.form.employmentDetails')}</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.department')}</label>
              <input
                type="text"
                value={form.department || ''}
                onChange={(e) => updateField('department', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.designation')}</label>
              <input
                type="text"
                value={form.designation || ''}
                onChange={(e) => updateField('designation', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.employmentType')}</label>
              <select
                value={form.employment_type || 'permanent'}
                onChange={(e) => updateField('employment_type', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="permanent">{t('enums.employmentType.permanent')}</option>
                <option value="contract">{t('enums.employmentType.contract')}</option>
                <option value="part_time">{t('enums.employmentType.part_time')}</option>
                <option value="intern">{t('enums.employmentType.intern')}</option>
                <option value="daily_rated">{t('enums.employmentType.daily_rated')}</option>
                <option value="hourly_rated">{t('enums.employmentType.hourly_rated')}</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.dateJoined')} *</label>
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
                  <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.form.basicSalaryRm')} *</label>
                  <input
                    type="number"
                    step="0.01"
                    value={salaryDisplay}
                    onChange={(e) => handleSalaryChange(e.target.value)}
                    className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                    required
                    placeholder={t('employees.form.salaryPlaceholder')}
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.form.payrollGroup')}</label>
                  <select
                    value={form.payroll_group_id || ''}
                    onChange={(e) => updateField('payroll_group_id', e.target.value || undefined)}
                    className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                  >
                    <option value="">{t('common.select')}</option>
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

        {/* Banking — payroll-sensitive server-side ("banking decides where
            salary lands"), so it is offered only to a role that can save it. */}
        {canViewPayroll && (
          <section className="bg-white rounded-2xl shadow p-6">
            <h2 className="text-lg font-semibold mb-4">{t('employees.form.bankingDetails')}</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.form.bankName')}</label>
                <select
                  value={form.bank_name || ''}
                  onChange={(e) => updateField('bank_name', e.target.value || undefined)}
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                >
                  <option value="">{t('employees.form.selectBank')}</option>
                  {BANKS.map((b) => (
                    <option key={b} value={b}>{b}</option>
                  ))}
                </select>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.form.accountNumber')}</label>
                <input
                  type="text"
                  value={form.bank_account_number || ''}
                  onChange={(e) => updateField('bank_account_number', e.target.value)}
                  className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
                />
              </div>
            </div>
          </section>
        )}

        {canViewPayroll && (
          <section className="bg-white rounded-2xl shadow p-6">
            <h2 className="text-lg font-semibold mb-4">{t('employees.form.statutoryTax')}</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.epfNumber')}</label>
              <input
                type="text"
                value={form.epf_number || ''}
                onChange={(e) => updateField('epf_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.socsoNumber')}</label>
              <input
                type="text"
                value={form.socso_number || ''}
                onChange={(e) => updateField('socso_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.eisNumber')}</label>
              <input
                type="text"
                value={form.eis_number || ''}
                onChange={(e) => updateField('eis_number', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.epfCategory')}</label>
              <select
                value={form.epf_category || 'A'}
                onChange={(e) => updateField('epf_category', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="A">{t('enums.epfCategory.A')}</option>
                <option value="B">{t('enums.epfCategory.B')}</option>
                <option value="C">{t('enums.epfCategory.C')}</option>
                <option value="D">{t('enums.epfCategory.D')}</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.residency')}</label>
              <select
                value={form.residency_status || 'citizen'}
                onChange={(e) => updateField('residency_status', e.target.value)}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="citizen">{t('enums.residency.citizen')}</option>
                <option value="permanent_resident">{t('enums.residency.permanent_resident')}</option>
                <option value="foreigner">{t('enums.residency.foreigner')}</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.fields.workingSpouse')}</label>
              <select
                value={form.working_spouse ? 'yes' : 'no'}
                onChange={(e) => updateField('working_spouse', e.target.value === 'yes')}
                className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none"
              >
                <option value="no">{t('common.no')}</option>
                <option value="yes">{t('common.yes')}</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">{t('employees.form.numChildren')}</label>
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
                  <span className="text-sm">{t('employees.fields.muslim')}</span>
                </label>
                {form.is_muslim && (
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={form.zakat_eligible || false}
                      onChange={(e) => updateField('zakat_eligible', e.target.checked)}
                      className="rounded"
                    />
                    <span className="text-sm">{t('employees.form.zakatEligible')}</span>
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
            {mutation.isPending ? t('common.creating') : t('employees.createSubmit')}
          </button>
          <button
            type="button"
            onClick={() => navigate('/employees')}
            className="bg-white text-gray-700 px-6 py-2.5 rounded-lg font-medium border border-gray-200 hover:bg-gray-50 transition-colors"
          >
            {t('common.cancel')}
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
                  {accountDialog.created ? t('employees.account.createdTitle') : t('employees.account.noticeTitle')}
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
                    {t('employees.account.createdBody')}
                  </p>
                  <div className="bg-gray-50 rounded-xl p-4 space-y-3">
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-500">{t('common.email')}</span>
                      <span className="font-medium text-gray-900">{accountDialog.email}</span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-500">{t('employees.account.role')}</span>
                      <span className="font-medium text-gray-900">{t(`roles.${accountDialog.role}`, { defaultValue: accountDialog.role })}</span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-500">{t('employees.account.defaultPassword')}</span>
                      <span className="font-mono font-medium text-gray-900">{accountDialog.default_password}</span>
                    </div>
                  </div>
                  <p className="text-xs text-amber-600 bg-amber-50 rounded-lg px-3 py-2">
                    {t('employees.account.adviseChange')}
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
                {t('common.done')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
