import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle } from 'lucide-react';
import {
  createEmployee,
  getEmployee,
  updateEmployee,
  type EmployeeAccountInfo,
} from '@/api/employees';
import { getPayrollGroups } from '@/api/payroll';
import { Modal } from '@/components/ui/Modal';
import { useAuth } from '@/context/AuthContext';
import { stripPayrollFields } from '@/lib/employeeFields';
import { canAccessPayrollData } from '@/lib/roles';
import { getErrorMessage, todayLocalDate } from '@/lib/utils';
import type { CreateEmployeeRequest, Employee, UpdateEmployeeRequest } from '@/types';

const BANKS = [
  'Maybank',
  'CIMB Bank',
  'Public Bank',
  'RHB Bank',
  'Hong Leong Bank',
  'AmBank',
  'Bank Islam',
  'Bank Rakyat',
  'Alliance Bank',
  'Affin Bank',
  'BSN',
  'OCBC Bank',
  'Standard Chartered',
  'HSBC',
  'UOB',
  'Agro Bank',
  'Bank Muamalat',
  'MBSB Bank',
  'Al Rajhi Bank',
];

/**
 * The edit modal also drives the employment lifecycle, which `CreateEmployeeRequest`
 * has no fields for — a new hire is not resigned.
 */
type EmployeeFormState = CreateEmployeeRequest & {
  date_resigned?: string;
  resignation_reason?: string;
};

function employeeToForm(emp: Employee): EmployeeFormState & { salaryDisplay: string } {
  return {
    employee_number: emp.employee_number,
    full_name: emp.full_name,
    ic_number: emp.ic_number ?? undefined,
    date_of_birth: emp.date_of_birth ?? undefined,
    gender: emp.gender ?? undefined,
    race: emp.race ?? undefined,
    nationality: emp.nationality ?? undefined,
    marital_status: emp.marital_status ?? undefined,
    email: emp.email ?? undefined,
    phone: emp.phone ?? undefined,
    address_line1: emp.address_line1 ?? undefined,
    address_line2: emp.address_line2 ?? undefined,
    city: emp.city ?? undefined,
    state: emp.state ?? undefined,
    postcode: emp.postcode ?? undefined,
    department: emp.department ?? undefined,
    designation: emp.designation ?? undefined,
    cost_centre: emp.cost_centre ?? undefined,
    branch: emp.branch ?? undefined,
    employment_type: emp.employment_type,
    date_joined: emp.date_joined,
    basic_salary: emp.basic_salary,
    bank_name: emp.bank_name ?? undefined,
    bank_account_number: emp.bank_account_number ?? undefined,
    tax_identification_number: emp.tax_identification_number ?? undefined,
    epf_number: emp.epf_number ?? undefined,
    socso_number: emp.socso_number ?? undefined,
    eis_number: emp.eis_number ?? undefined,
    epf_category: emp.epf_category ?? undefined,
    residency_status: emp.residency_status,
    working_spouse: emp.working_spouse ?? undefined,
    num_children: emp.num_children ?? undefined,
    is_muslim: emp.is_muslim ?? undefined,
    zakat_eligible: emp.zakat_eligible ?? undefined,
    zakat_monthly_amount: emp.zakat_monthly_amount ?? undefined,
    ptptn_monthly_amount: emp.ptptn_monthly_amount ?? undefined,
    payroll_group_id: emp.payroll_group_id ?? undefined,
    is_active: emp.is_active ?? undefined,
    date_resigned: emp.date_resigned ?? undefined,
    resignation_reason: emp.resignation_reason ?? undefined,
    salaryDisplay: (emp.basic_salary / 100).toFixed(2),
  };
}

export function EmployeeFormModal({
  mode,
  employeeId,
  onClose,
}: {
  mode: 'create' | 'edit';
  employeeId?: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const { user } = useAuth();
  const canViewPayroll = canAccessPayrollData(user);
  const queryClient = useQueryClient();

  const { data: existingEmployee, isLoading: loadingEmployee } = useQuery({
    queryKey: ['employee', employeeId],
    queryFn: () => getEmployee(employeeId!),
    enabled: mode === 'edit' && !!employeeId,
  });

  const { data: payrollGroups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
    enabled: canViewPayroll,
  });

  const isReady = mode === 'create' || !!existingEmployee;

  return (
    <Modal
      open
      onClose={onClose}
      title={mode === 'create' ? t('employees.createTitle') : t('employees.editTitle')}
      maxWidth="max-w-3xl"
    >
      {!isReady || loadingEmployee ? (
        <div className="flex items-center justify-center py-12">
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900" />
        </div>
      ) : (
        <EmployeeFormContent
          mode={mode}
          employeeId={employeeId}
          initialData={existingEmployee}
          payrollGroups={payrollGroups}
          onClose={onClose}
          queryClient={queryClient}
          canViewPayroll={canViewPayroll}
        />
      )}
    </Modal>
  );
}

function EmployeeFormContent({
  mode,
  employeeId,
  initialData,
  payrollGroups,
  onClose,
  queryClient,
  canViewPayroll,
}: {
  mode: 'create' | 'edit';
  employeeId?: string;
  initialData?: Employee;
  payrollGroups?: { id: string; name: string }[];
  onClose: () => void;
  queryClient: ReturnType<typeof useQueryClient>;
  canViewPayroll?: boolean;
}) {
  const { t } = useTranslation();
  const defaults = initialData ? employeeToForm(initialData) : null;

  const [form, setForm] = useState<EmployeeFormState>(
    defaults ?? {
      employee_number: '',
      full_name: '',
      date_joined: todayLocalDate(),
      basic_salary: 0,
    },
  );

  const [salaryDisplay, setSalaryDisplay] = useState(defaults?.salaryDisplay ?? '');

  const [accountDialog, setAccountDialog] = useState<EmployeeAccountInfo | null>(null);

  const createMutation = useMutation({
    mutationFn: (req: CreateEmployeeRequest) => createEmployee(req),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      if (data.account) {
        setAccountDialog(data.account);
      } else {
        onClose();
      }
    },
  });

  const updateMutation = useMutation({
    mutationFn: (req: UpdateEmployeeRequest) => updateEmployee(employeeId!, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      queryClient.invalidateQueries({ queryKey: ['employee', employeeId] });
      onClose();
    },
  });

  const mutation = mode === 'create' ? createMutation : updateMutation;

  const updateField = (field: string, value: unknown) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  const handleSalaryChange = (value: string) => {
    setSalaryDisplay(value);
    const ringgit = parseFloat(value) || 0;
    updateField('basic_salary', Math.round(ringgit * 100));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (mode === 'create') {
      const payload = canViewPayroll
        ? form
        : ({ ...stripPayrollFields(form), basic_salary: 0 } as CreateEmployeeRequest);
      createMutation.mutate(payload);
      return;
    }

    const payload: UpdateEmployeeRequest = canViewPayroll
      ? { ...form }
      : stripPayrollFields(form);
    // An omitted `date_resigned` means "keep existing" on the backend, so
    // emptying the field has to be sent as an explicit clear or the resignation
    // would be unremovable — which is what kept an un-terminated employee out of
    // every later payroll run.
    if (initialData?.date_resigned && !form.date_resigned) {
      delete payload.date_resigned;
      payload.clear_date_resigned = true;
    }
    updateMutation.mutate(payload);
  };

  const inputClass =
    'w-full px-4 py-2.5 border border-gray-200 rounded-lg focus:ring-1 focus:ring-black outline-none text-sm bg-white transition-colors';
  const labelClass = 'block text-sm font-medium text-gray-600 mb-1.5';
  const sectionTitleClass = 'text-xs font-bold text-gray-500 uppercase tracking-wider mb-4';

  return (
    <form onSubmit={handleSubmit} className="space-y-8">
      {mutation.isError && (
        <div className="bg-red-50 text-red-600 text-sm px-4 py-3 rounded-xl border border-red-100">
          {getErrorMessage(
            mutation.error,
            t(mode === 'create' ? 'employees.createFailed' : 'employees.updateFailed'),
          )}
        </div>
      )}

      {/* Personal Information */}
      <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
        <h3 className={sectionTitleClass}>{t('employees.form.personalInfo')}</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
          <div>
            <label className={labelClass}>{t('employees.form.employeeNumber')} *</label>
            <input
              type="text"
              value={form.employee_number}
              onChange={(e) => updateField('employee_number', e.target.value)}
              className={inputClass}
              required
              placeholder={t('employees.form.employeeNumberPlaceholder')}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.form.fullName')} *</label>
            <input
              type="text"
              value={form.full_name}
              onChange={(e) => updateField('full_name', e.target.value)}
              className={inputClass}
              required
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.form.icNumber')}</label>
            <input
              type="text"
              value={form.ic_number || ''}
              onChange={(e) => updateField('ic_number', e.target.value)}
              className={inputClass}
              placeholder={t('employees.form.icNumberPlaceholder')}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.fields.dateOfBirth')}</label>
            <input
              type="date"
              value={form.date_of_birth || ''}
              onChange={(e) => updateField('date_of_birth', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.fields.gender')}</label>
            <select
              value={form.gender || ''}
              onChange={(e) => updateField('gender', e.target.value || undefined)}
              className={inputClass}
            >
              <option value="">{t('common.select')}</option>
              <option value="male">{t('enums.gender.male')}</option>
              <option value="female">{t('enums.gender.female')}</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>{t('employees.fields.race')}</label>
            <select
              value={form.race || ''}
              onChange={(e) => updateField('race', e.target.value || undefined)}
              className={inputClass}
            >
              <option value="">{t('common.select')}</option>
              <option value="malay">{t('enums.race.malay')}</option>
              <option value="chinese">{t('enums.race.chinese')}</option>
              <option value="indian">{t('enums.race.indian')}</option>
              <option value="other">{t('enums.race.other')}</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>{t('employees.fields.maritalStatus')}</label>
            <select
              value={form.marital_status || ''}
              onChange={(e) => updateField('marital_status', e.target.value || undefined)}
              className={inputClass}
            >
              <option value="">{t('common.select')}</option>
              <option value="single">{t('enums.maritalStatus.single')}</option>
              <option value="married">{t('enums.maritalStatus.married')}</option>
              <option value="divorced">{t('enums.maritalStatus.divorced')}</option>
              <option value="widowed">{t('enums.maritalStatus.widowed')}</option>
            </select>
          </div>
          <div>
            <label className={labelClass}>{t('common.email')}</label>
            <input
              type="email"
              value={form.email || ''}
              onChange={(e) => updateField('email', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.fields.phone')}</label>
            <input
              type="tel"
              value={form.phone || ''}
              onChange={(e) => updateField('phone', e.target.value)}
              className={inputClass}
            />
          </div>
        </div>
      </section>

      {/* Address */}
      <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
        <h3 className={sectionTitleClass}>{t('employees.sections.address')}</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
          <div className="md:col-span-2">
            <label className={labelClass}>{t('employees.form.addressLine1')}</label>
            <input
              type="text"
              value={form.address_line1 || ''}
              onChange={(e) => updateField('address_line1', e.target.value)}
              className={inputClass}
            />
          </div>
          <div className="md:col-span-2">
            <label className={labelClass}>{t('employees.form.addressLine2')}</label>
            <input
              type="text"
              value={form.address_line2 || ''}
              onChange={(e) => updateField('address_line2', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.form.city')}</label>
            <input
              type="text"
              value={form.city || ''}
              onChange={(e) => updateField('city', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.form.state')}</label>
            <input
              type="text"
              value={form.state || ''}
              onChange={(e) => updateField('state', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.form.postcode')}</label>
            <input
              type="text"
              value={form.postcode || ''}
              onChange={(e) => updateField('postcode', e.target.value)}
              className={inputClass}
            />
          </div>
        </div>
      </section>

      {/* Employment Details */}
      <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
        <h3 className={sectionTitleClass}>{t('employees.form.employmentDetails')}</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
          <div>
            <label className={labelClass}>{t('employees.fields.department')}</label>
            <input
              type="text"
              value={form.department || ''}
              onChange={(e) => updateField('department', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.fields.designation')}</label>
            <input
              type="text"
              value={form.designation || ''}
              onChange={(e) => updateField('designation', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.fields.employmentType')}</label>
            <select
              value={form.employment_type || 'permanent'}
              onChange={(e) => updateField('employment_type', e.target.value)}
              className={inputClass}
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
            <label className={labelClass}>{t('employees.fields.dateJoined')} *</label>
            <input
              type="date"
              value={form.date_joined}
              onChange={(e) => updateField('date_joined', e.target.value)}
              className={inputClass}
              required
            />
          </div>
          {canViewPayroll && (
            <>
              <div>
                <label className={labelClass}>{t('employees.form.basicSalaryRm')} *</label>
                <input
                  type="number"
                  step="0.01"
                  value={salaryDisplay}
                  onChange={(e) => handleSalaryChange(e.target.value)}
                  className={inputClass}
                  required
                  placeholder={t('employees.form.salaryPlaceholder')}
                />
              </div>
              <div>
                <label className={labelClass}>{t('employees.form.payrollGroup')}</label>
                <select
                  value={form.payroll_group_id || ''}
                  onChange={(e) => updateField('payroll_group_id', e.target.value || undefined)}
                  className={inputClass}
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
          <div>
            <label className={labelClass}>{t('employees.fields.costCentre')}</label>
            <input
              type="text"
              value={form.cost_centre || ''}
              onChange={(e) => updateField('cost_centre', e.target.value)}
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>{t('employees.fields.branch')}</label>
            <input
              type="text"
              value={form.branch || ''}
              onChange={(e) => updateField('branch', e.target.value)}
              className={inputClass}
            />
          </div>
          {mode === 'edit' && (
            <>
              <div>
                <label className={labelClass}>{t('common.status')}</label>
                <select
                  value={form.is_active ? 'active' : 'inactive'}
                  onChange={(e) => updateField('is_active', e.target.value === 'active')}
                  className={inputClass}
                >
                  <option value="active">{t('common.active')}</option>
                  <option value="inactive">{t('common.inactive')}</option>
                </select>
                {/*
                  Warn, do not block: an HR admin must still be able to represent
                  a never-started hire. The backend is what refuses the payroll
                  run, because it cannot tell whether a final payslip is owed.
                */}
                {!form.is_active && !form.date_resigned && (
                  <p className="mt-1.5 text-xs text-amber-700 flex items-start gap-1.5">
                    <AlertTriangle className="w-3.5 h-3.5 mt-px shrink-0" />
                    <span>{t('employees.form.inactiveNoResignWarning')}</span>
                  </p>
                )}
              </div>
              <div>
                <label className={labelClass}>{t('employees.fields.dateResigned')}</label>
                <input
                  type="date"
                  value={form.date_resigned || ''}
                  min={form.date_joined}
                  onChange={(e) => updateField('date_resigned', e.target.value || undefined)}
                  className={inputClass}
                />
                <p className="mt-1.5 text-xs text-gray-500">{t('employees.form.dateResignedHint')}</p>
              </div>
              <div>
                <label className={labelClass}>{t('employees.fields.resignationReason')}</label>
                <input
                  type="text"
                  value={form.resignation_reason || ''}
                  onChange={(e) => updateField('resignation_reason', e.target.value || undefined)}
                  className={inputClass}
                />
              </div>
            </>
          )}
        </div>
      </section>

      {/* Banking Details — gated like Statutory & Tax: the API classifies both
          bank fields as payroll-sensitive, so offering the editor to a role
          that cannot save them only produces a 403 on the whole request. */}
      {canViewPayroll && (
        <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
          <h3 className={sectionTitleClass}>{t('employees.form.bankingDetails')}</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
            <div>
              <label className={labelClass}>{t('employees.form.bankName')}</label>
              <select
                value={form.bank_name || ''}
                onChange={(e) => updateField('bank_name', e.target.value || undefined)}
                className={inputClass}
              >
                <option value="">{t('employees.form.selectBank')}</option>
                {BANKS.map((b) => (
                  <option key={b} value={b}>
                    {b}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className={labelClass}>{t('employees.form.accountNumber')}</label>
              <input
                type="text"
                value={form.bank_account_number || ''}
                onChange={(e) => updateField('bank_account_number', e.target.value)}
                className={inputClass}
              />
            </div>
          </div>
        </section>
      )}

      {canViewPayroll && (
        <section className="bg-gray-50 rounded-xl border border-gray-100 p-6">
          <h3 className={sectionTitleClass}>{t('employees.form.statutoryTax')}</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-5">
            <div>
              <label className={labelClass}>{t('employees.fields.tin')}</label>
              <input
                type="text"
                value={form.tax_identification_number || ''}
                onChange={(e) => updateField('tax_identification_number', e.target.value)}
                className={inputClass}
              />
            </div>
            <div>
              <label className={labelClass}>{t('employees.fields.epfNumber')}</label>
              <input
                type="text"
                value={form.epf_number || ''}
                onChange={(e) => updateField('epf_number', e.target.value)}
                className={inputClass}
              />
            </div>
            <div>
              <label className={labelClass}>{t('employees.fields.socsoNumber')}</label>
              <input
                type="text"
                value={form.socso_number || ''}
                onChange={(e) => updateField('socso_number', e.target.value)}
                className={inputClass}
              />
            </div>
            <div>
              <label className={labelClass}>{t('employees.fields.eisNumber')}</label>
              <input
                type="text"
                value={form.eis_number || ''}
                onChange={(e) => updateField('eis_number', e.target.value)}
                className={inputClass}
              />
            </div>
            <div>
              <label className={labelClass}>{t('employees.fields.epfCategory')}</label>
              <select
                value={form.epf_category ?? ''}
                onChange={(e) => updateField('epf_category', e.target.value || undefined)}
                className={inputClass}
              >
                <option value="">{t('enums.epfCategory.auto')}</option>
                <option value="A">{t('enums.epfCategory.A')}</option>
                <option value="B">{t('enums.epfCategory.B')}</option>
                <option value="C">{t('enums.epfCategory.C')}</option>
                <option value="D">{t('enums.epfCategory.D')}</option>
                <option value="E">{t('enums.epfCategory.E')}</option>
                <option value="F">{t('enums.epfCategory.F')}</option>
              </select>
            </div>
            <div>
              <label className={labelClass}>{t('employees.fields.residency')}</label>
              <select
                value={form.residency_status || 'citizen'}
                onChange={(e) => updateField('residency_status', e.target.value)}
                className={inputClass}
              >
                <option value="citizen">{t('enums.residency.citizen')}</option>
                <option value="permanent_resident">{t('enums.residency.permanent_resident')}</option>
                <option value="foreigner">{t('enums.residency.foreigner')}</option>
              </select>
            </div>
            <div>
              <label className={labelClass}>{t('employees.fields.workingSpouse')}</label>
              <select
                value={form.working_spouse ? 'yes' : 'no'}
                onChange={(e) => updateField('working_spouse', e.target.value === 'yes')}
                className={inputClass}
              >
                <option value="no">{t('common.no')}</option>
                <option value="yes">{t('common.yes')}</option>
              </select>
            </div>
            <div>
              <label className={labelClass}>{t('employees.form.numChildren')}</label>
              <input
                type="number"
                min="0"
                value={form.num_children ?? 0}
                onChange={(e) => updateField('num_children', parseInt(e.target.value) || 0)}
                className={inputClass}
              />
            </div>
            <div className="md:col-span-2 flex items-center gap-6 pt-2">
              <label className="flex items-center gap-2.5 cursor-pointer">
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
                  className="rounded border-gray-200 w-4 h-4 text-gray-900 focus:ring-black"
                />
                <span className="text-sm font-medium text-gray-700">
                  {t('employees.fields.muslim')}
                </span>
              </label>
              {form.is_muslim && (
                <label className="flex items-center gap-2.5 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={form.zakat_eligible || false}
                    onChange={(e) => updateField('zakat_eligible', e.target.checked)}
                    className="rounded border-gray-200 w-4 h-4 text-gray-900 focus:ring-black"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    {t('employees.form.zakatEligible')}
                  </span>
                </label>
              )}
            </div>
          </div>
        </section>
      )}

      {/* Footer */}
      <div className="flex justify-end gap-3 pt-2">
        <button
          type="button"
          onClick={onClose}
          className="px-5 py-2.5 text-sm text-gray-600 hover:bg-gray-100 rounded-lg font-medium border border-gray-200 transition-colors"
        >
          {t('common.cancel')}
        </button>
        <button type="submit" disabled={mutation.isPending} className="btn-primary">
          {mutation.isPending
            ? mode === 'create'
              ? t('common.creating')
              : t('common.saving')
            : mode === 'create'
              ? t('employees.createSubmit')
              : t('common.saveChanges')}
        </button>
      </div>

      {/* Account Created Dialog */}
      <Modal
        open={accountDialog !== null}
        onClose={() => {
          setAccountDialog(null);
          onClose();
        }}
        title={
          accountDialog?.created
            ? t('employees.account.createdTitle')
            : t('employees.account.noticeTitle')
        }
        maxWidth="max-w-md"
        footer={
          <div className="flex justify-end">
            <button
              type="button"
              onClick={() => {
                setAccountDialog(null);
                onClose();
              }}
              className="btn-primary"
            >
              {t('common.done')}
            </button>
          </div>
        }
      >
        {accountDialog && (
          <div className="space-y-4">
            {accountDialog.created ? (
              <>
                <p className="text-sm text-gray-600 dark:text-slate-300">
                  {t('employees.account.createdBody')}
                </p>
                <div className="bg-gray-50 dark:bg-slate-950/60 rounded-xl p-4 space-y-3">
                  <div className="flex justify-between text-sm">
                    <span className="text-gray-500 dark:text-slate-400">{t('common.email')}</span>
                    <span className="font-medium text-gray-900 dark:text-slate-100">
                      {accountDialog.email}
                    </span>
                  </div>
                  <div className="flex justify-between text-sm">
                    <span className="text-gray-500 dark:text-slate-400">
                      {t('employees.account.role')}
                    </span>
                    <span className="font-medium text-gray-900 dark:text-slate-100">
                      {t(`roles.${accountDialog.role}`, { defaultValue: accountDialog.role })}
                    </span>
                  </div>
                  <div className="flex justify-between text-sm">
                    <span className="text-gray-500 dark:text-slate-400">
                      {t('employees.account.defaultPassword')}
                    </span>
                    <span className="font-mono font-medium text-gray-900 dark:text-slate-100">
                      {accountDialog.default_password}
                    </span>
                  </div>
                </div>
                <p className="text-xs text-amber-600 dark:text-amber-300 bg-amber-50 dark:bg-amber-500/10 rounded-lg px-3 py-2">
                  {t('employees.account.adviseChange')}
                </p>
              </>
            ) : (
              <div className="bg-amber-50 dark:bg-amber-500/10 rounded-xl p-4">
                <p className="text-sm text-amber-800 dark:text-amber-200">
                  {accountDialog.message}
                </p>
              </div>
            )}
          </div>
        )}
      </Modal>
    </form>
  );
}
