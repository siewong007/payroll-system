import { useId, useMemo, useState } from 'react';
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
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { stripPayrollFields } from '@/lib/employeeFields';
import { usePermissions } from '@/lib/usePermissions';
import { useConfirm } from '@/lib/confirm';
import { useToast } from '@/lib/toast';
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
  confirmation_date?: string;
  date_resigned?: string;
  resignation_reason?: string;
};

function employeeToForm(emp: Employee): {
  form: EmployeeFormState;
  salaryDisplay: string;
  zakatDisplay: string;
  ptptnDisplay: string;
} {
  return {
    form: {
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
      confirmation_date: emp.confirmation_date ?? undefined,
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
    },
    salaryDisplay: (emp.basic_salary / 100).toFixed(2),
    zakatDisplay: emp.zakat_monthly_amount ? (emp.zakat_monthly_amount / 100).toFixed(2) : '',
    ptptnDisplay: emp.ptptn_monthly_amount ? (emp.ptptn_monthly_amount / 100).toFixed(2) : '',
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
  const { can } = usePermissions();
  const canViewPayroll = can('view_payroll');
  const queryClient = useQueryClient();

  const {
    data: existingEmployee,
    isLoading: loadingEmployee,
    isError,
    error,
    refetch,
  } = useQuery({
    queryKey: ['employee', employeeId],
    queryFn: () => getEmployee(employeeId!),
    enabled: mode === 'edit' && !!employeeId,
  });

  const { data: payrollGroups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
    enabled: canViewPayroll,
  });

  const title = mode === 'create' ? t('employees.createTitle') : t('employees.editTitle');

  if (mode === 'edit' && (loadingEmployee || isError || !existingEmployee)) {
    return (
      <Modal open onClose={onClose} title={title} maxWidth="max-w-3xl">
        {loadingEmployee ? (
          <div className="flex items-center justify-center py-12">
            <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-900 dark:border-slate-200" />
          </div>
        ) : isError ? (
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
        ) : (
          <div className="text-center py-12 text-sm text-gray-500 dark:text-slate-400">
            {t('employees.notFound')}
          </div>
        )}
      </Modal>
    );
  }

  return (
    <EmployeeFormContent
      mode={mode}
      employeeId={employeeId}
      initialData={existingEmployee}
      payrollGroups={payrollGroups}
      onClose={onClose}
      queryClient={queryClient}
      canViewPayroll={canViewPayroll}
    />
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
  const toast = useToast();
  const confirm = useConfirm();
  const formId = useId();
  const [defaultJoinedDate] = useState(() => todayLocalDate());

  const initialState = useMemo(() => {
    if (initialData) return employeeToForm(initialData);
    return {
      form: {
        employee_number: '',
        full_name: '',
        date_joined: defaultJoinedDate,
        basic_salary: 0,
      } satisfies EmployeeFormState,
      salaryDisplay: '',
      zakatDisplay: '',
      ptptnDisplay: '',
    };
  }, [initialData, defaultJoinedDate]);

  const [form, setForm] = useState<EmployeeFormState>(initialState.form);
  const [salaryDisplay, setSalaryDisplay] = useState(initialState.salaryDisplay);
  const [zakatDisplay, setZakatDisplay] = useState(initialState.zakatDisplay);
  const [ptptnDisplay, setPtptnDisplay] = useState(initialState.ptptnDisplay);
  const [accountDialog, setAccountDialog] = useState<EmployeeAccountInfo | null>(null);

  const createMutation = useMutation({
    mutationFn: (req: CreateEmployeeRequest) => createEmployee(req),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      queryClient.invalidateQueries({ queryKey: ['employees-picker'] });
      if (data.account) {
        setAccountDialog(data.account);
      } else {
        toast.success(t('employees.createSuccess'));
        onClose();
      }
    },
  });

  const updateMutation = useMutation({
    mutationFn: (req: UpdateEmployeeRequest) => updateEmployee(employeeId!, req),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      queryClient.invalidateQueries({ queryKey: ['employees-picker'] });
      queryClient.invalidateQueries({ queryKey: ['employee', employeeId] });
      queryClient.invalidateQueries({ queryKey: ['salaryHistory', employeeId] });
      toast.success(t('employees.editSuccess'));
      onClose();
    },
  });

  const mutation = mode === 'create' ? createMutation : updateMutation;

  const isDirty =
    accountDialog === null &&
    !mutation.isSuccess &&
    (JSON.stringify(form) !== JSON.stringify(initialState.form) ||
      salaryDisplay !== initialState.salaryDisplay ||
      zakatDisplay !== initialState.zakatDisplay ||
      ptptnDisplay !== initialState.ptptnDisplay);

  const handleRequestClose = async () => {
    if (mutation.isPending) return;
    if (isDirty) {
      const ok = await confirm({
        title: t('common.discard'),
        description: t('employees.form.discardChangesConfirm'),
        confirmLabel: t('common.discard'),
        variant: 'warning',
      });
      if (!ok) return;
    }
    onClose();
  };

  const updateField = <K extends keyof EmployeeFormState>(field: K, value: EmployeeFormState[K]) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  const handleSalaryChange = (value: string) => {
    setSalaryDisplay(value);
    const parsed = parseFloat(value);
    const ringgit = Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
    updateField('basic_salary', Math.round(ringgit * 100));
  };

  const handleZakatChange = (value: string) => {
    setZakatDisplay(value);
    if (!value.trim()) {
      updateField('zakat_monthly_amount', mode === 'edit' ? 0 : undefined);
      return;
    }
    const parsed = parseFloat(value);
    updateField(
      'zakat_monthly_amount',
      Number.isFinite(parsed)
        ? Math.round(Math.max(0, parsed) * 100)
        : mode === 'edit'
          ? 0
          : undefined,
    );
  };

  const handlePtptnChange = (value: string) => {
    setPtptnDisplay(value);
    if (!value.trim()) {
      updateField('ptptn_monthly_amount', mode === 'edit' ? 0 : undefined);
      return;
    }
    const parsed = parseFloat(value);
    updateField(
      'ptptn_monthly_amount',
      Number.isFinite(parsed)
        ? Math.round(Math.max(0, parsed) * 100)
        : mode === 'edit'
          ? 0
          : undefined,
    );
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
    delete payload.employee_number;
    delete payload.date_joined;
    // An omitted `date_resigned` means "keep existing" on the backend, so
    // emptying the field has to be sent as an explicit clear or the resignation
    // would be unremovable — which is what kept an un-terminated employee out of
    // every later payroll run.
    if (initialData?.date_resigned && !form.date_resigned) {
      delete payload.date_resigned;
      payload.clear_date_resigned = true;
    }
    if (canViewPayroll && initialData?.epf_category && !form.epf_category) {
      delete payload.epf_category;
      payload.clear_epf_category = true;
    }
    updateMutation.mutate(payload);
  };

  const sectionClass =
    'bg-gray-50 dark:bg-slate-800/50 rounded-xl border border-gray-100 dark:border-slate-800 p-5 sm:p-6';
  const sectionTitleClass =
    'text-xs font-bold text-gray-500 dark:text-slate-400 uppercase tracking-wider mb-4';

  return (
    <Modal
      open
      onClose={handleRequestClose}
      title={mode === 'create' ? t('employees.createTitle') : t('employees.editTitle')}
      maxWidth="max-w-3xl"
      footer={
        <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
          <button
            type="button"
            onClick={handleRequestClose}
            disabled={mutation.isPending}
            className="btn-secondary"
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form={formId}
            disabled={mutation.isPending}
            className="btn-primary"
          >
            {mutation.isPending
              ? mode === 'create'
                ? t('common.creating')
                : t('common.saving')
              : mode === 'create'
                ? t('employees.createSubmit')
                : t('common.saveChanges')}
          </button>
        </div>
      }
    >
      <form id={formId} onSubmit={handleSubmit} className="space-y-6">
        {mutation.isError && (
          <div className="bg-red-50 dark:bg-red-500/10 text-red-600 dark:text-red-300 text-sm px-4 py-3 rounded-xl border border-red-100 dark:border-red-500/30">
            {getErrorMessage(
              mutation.error,
              t(mode === 'create' ? 'employees.createFailed' : 'employees.updateFailed'),
            )}
          </div>
        )}

        {/* Personal Information */}
        <section className={sectionClass}>
          <h3 className={sectionTitleClass}>{t('employees.form.personalInfo')}</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
            <FormField
              label={t('employees.form.employeeNumber')}
              required={mode === 'create'}
              hint={mode === 'edit' ? t('employees.form.editReadOnlyHint') : undefined}
            >
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.employee_number}
                  onChange={(e) => updateField('employee_number', e.target.value)}
                  disabled={mode === 'edit'}
                  className="form-input disabled:opacity-60 disabled:cursor-not-allowed"
                  required={mode === 'create'}
                  placeholder={t('employees.form.employeeNumberPlaceholder')}
                />
              )}
            </FormField>
            <FormField label={t('employees.form.fullName')} required>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.full_name}
                  onChange={(e) => updateField('full_name', e.target.value)}
                  className="form-input"
                  required
                />
              )}
            </FormField>
            <FormField label={t('employees.form.icNumber')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.ic_number || ''}
                  onChange={(e) => updateField('ic_number', e.target.value || undefined)}
                  className="form-input"
                  placeholder={t('employees.form.icNumberPlaceholder')}
                />
              )}
            </FormField>
            <FormField label={t('employees.fields.dateOfBirth')}>
              {(aria) => (
                <input
                  {...aria}
                  type="date"
                  value={form.date_of_birth || ''}
                  onChange={(e) => updateField('date_of_birth', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('employees.fields.gender')}>
              {(aria) => (
                <select
                  {...aria}
                  value={form.gender || ''}
                  onChange={(e) => updateField('gender', e.target.value || undefined)}
                  className="form-input"
                >
                  <option value="">{t('common.select')}</option>
                  <option value="male">{t('enums.gender.male')}</option>
                  <option value="female">{t('enums.gender.female')}</option>
                </select>
              )}
            </FormField>
            <FormField label={t('employees.fields.race')}>
              {(aria) => (
                <select
                  {...aria}
                  value={form.race || ''}
                  onChange={(e) => updateField('race', e.target.value || undefined)}
                  className="form-input"
                >
                  <option value="">{t('common.select')}</option>
                  <option value="malay">{t('enums.race.malay')}</option>
                  <option value="chinese">{t('enums.race.chinese')}</option>
                  <option value="indian">{t('enums.race.indian')}</option>
                  <option value="other">{t('enums.race.other')}</option>
                </select>
              )}
            </FormField>
            <FormField label={t('employees.fields.nationality')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.nationality || ''}
                  onChange={(e) => updateField('nationality', e.target.value || undefined)}
                  placeholder={t('employees.form.nationalityPlaceholder')}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('employees.fields.maritalStatus')}>
              {(aria) => (
                <select
                  {...aria}
                  value={form.marital_status || ''}
                  onChange={(e) => updateField('marital_status', e.target.value || undefined)}
                  className="form-input"
                >
                  <option value="">{t('common.select')}</option>
                  <option value="single">{t('enums.maritalStatus.single')}</option>
                  <option value="married">{t('enums.maritalStatus.married')}</option>
                  <option value="divorced">{t('enums.maritalStatus.divorced')}</option>
                  <option value="widowed">{t('enums.maritalStatus.widowed')}</option>
                </select>
              )}
            </FormField>
            <FormField label={t('common.email')}>
              {(aria) => (
                <input
                  {...aria}
                  type="email"
                  value={form.email || ''}
                  onChange={(e) => updateField('email', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('employees.fields.phone')}>
              {(aria) => (
                <input
                  {...aria}
                  type="tel"
                  value={form.phone || ''}
                  onChange={(e) => updateField('phone', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
          </div>
        </section>

        {/* Address */}
        <section className={sectionClass}>
          <h3 className={sectionTitleClass}>{t('employees.sections.address')}</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
            <div className="md:col-span-2">
              <FormField label={t('employees.form.addressLine1')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.address_line1 || ''}
                    onChange={(e) => updateField('address_line1', e.target.value || undefined)}
                    className="form-input"
                  />
                )}
              </FormField>
            </div>
            <div className="md:col-span-2">
              <FormField label={t('employees.form.addressLine2')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.address_line2 || ''}
                    onChange={(e) => updateField('address_line2', e.target.value || undefined)}
                    className="form-input"
                  />
                )}
              </FormField>
            </div>
            <FormField label={t('employees.form.city')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.city || ''}
                  onChange={(e) => updateField('city', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('employees.form.state')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.state || ''}
                  onChange={(e) => updateField('state', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('employees.form.postcode')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.postcode || ''}
                  onChange={(e) => updateField('postcode', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
          </div>
        </section>

        {/* Employment Details */}
        <section className={sectionClass}>
          <h3 className={sectionTitleClass}>{t('employees.form.employmentDetails')}</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
            <FormField label={t('employees.fields.department')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.department || ''}
                  onChange={(e) => updateField('department', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('employees.fields.designation')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.designation || ''}
                  onChange={(e) => updateField('designation', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('employees.fields.employmentType')}>
              {(aria) => (
                <select
                  {...aria}
                  value={form.employment_type || 'permanent'}
                  onChange={(e) => updateField('employment_type', e.target.value)}
                  className="form-input"
                >
                  <option value="permanent">{t('enums.employmentType.permanent')}</option>
                  <option value="contract">{t('enums.employmentType.contract')}</option>
                  <option value="part_time">{t('enums.employmentType.part_time')}</option>
                  <option value="intern">{t('enums.employmentType.intern')}</option>
                  <option value="daily_rated">{t('enums.employmentType.daily_rated')}</option>
                  <option value="hourly_rated">{t('enums.employmentType.hourly_rated')}</option>
                </select>
              )}
            </FormField>
            <FormField
              label={t('employees.fields.dateJoined')}
              required={mode === 'create'}
              hint={mode === 'edit' ? t('employees.form.editReadOnlyHint') : undefined}
            >
              {(aria) => (
                <input
                  {...aria}
                  type="date"
                  value={form.date_joined}
                  onChange={(e) => updateField('date_joined', e.target.value)}
                  disabled={mode === 'edit'}
                  className="form-input disabled:opacity-60 disabled:cursor-not-allowed"
                  required={mode === 'create'}
                />
              )}
            </FormField>
            {mode === 'edit' && (
              <FormField label={t('employees.fields.confirmationDate')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="date"
                    value={form.confirmation_date || ''}
                    min={form.date_joined}
                    onChange={(e) => updateField('confirmation_date', e.target.value || undefined)}
                    className="form-input"
                  />
                )}
              </FormField>
            )}
            {canViewPayroll && (
              <>
                <FormField label={t('employees.form.basicSalaryRm')} required>
                  {(aria) => (
                    <input
                      {...aria}
                      type="number"
                      min="0"
                      step="0.01"
                      value={salaryDisplay}
                      onChange={(e) => handleSalaryChange(e.target.value)}
                      className="form-input"
                      required
                      placeholder={t('employees.form.salaryPlaceholder')}
                    />
                  )}
                </FormField>
                <FormField label={t('employees.form.payrollGroup')}>
                  {(aria) => (
                    <select
                      {...aria}
                      value={form.payroll_group_id || ''}
                      onChange={(e) => updateField('payroll_group_id', e.target.value || undefined)}
                      className="form-input"
                    >
                      <option value="">{t('common.select')}</option>
                      {payrollGroups?.map((g) => (
                        <option key={g.id} value={g.id}>
                          {g.name}
                        </option>
                      ))}
                    </select>
                  )}
                </FormField>
              </>
            )}
            <FormField label={t('employees.fields.costCentre')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.cost_centre || ''}
                  onChange={(e) => updateField('cost_centre', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
            <FormField label={t('employees.fields.branch')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.branch || ''}
                  onChange={(e) => updateField('branch', e.target.value || undefined)}
                  className="form-input"
                />
              )}
            </FormField>
            {mode === 'edit' && (
              <>
                <div>
                  <FormField label={t('common.status')}>
                    {(aria) => (
                      <select
                        {...aria}
                        value={form.is_active ? 'active' : 'inactive'}
                        onChange={(e) => updateField('is_active', e.target.value === 'active')}
                        className="form-input"
                      >
                        <option value="active">{t('common.active')}</option>
                        <option value="inactive">{t('common.inactive')}</option>
                      </select>
                    )}
                  </FormField>
                  {/*
                    Warn, do not block: an HR admin must still be able to represent
                    a never-started hire. The backend is what refuses the payroll
                    run, because it cannot tell whether a final payslip is owed.
                  */}
                  {!form.is_active && !form.date_resigned && (
                    <p className="mt-1.5 text-xs text-amber-700 dark:text-amber-300 flex items-start gap-1.5">
                      <AlertTriangle className="w-3.5 h-3.5 mt-px shrink-0" />
                      <span>{t('employees.form.inactiveNoResignWarning')}</span>
                    </p>
                  )}
                </div>
                <FormField
                  label={t('employees.fields.dateResigned')}
                  hint={t('employees.form.dateResignedHint')}
                >
                  {(aria) => (
                    <input
                      {...aria}
                      type="date"
                      value={form.date_resigned || ''}
                      min={form.date_joined}
                      onChange={(e) => updateField('date_resigned', e.target.value || undefined)}
                      className="form-input"
                    />
                  )}
                </FormField>
                <FormField label={t('employees.fields.resignationReason')}>
                  {(aria) => (
                    <input
                      {...aria}
                      type="text"
                      value={form.resignation_reason || ''}
                      onChange={(e) => updateField('resignation_reason', e.target.value || undefined)}
                      className="form-input"
                    />
                  )}
                </FormField>
              </>
            )}
          </div>
        </section>

        {/* Banking Details — gated like Statutory & Tax: the API classifies both
            bank fields as payroll-sensitive, so offering the editor to a role
            that cannot save them only produces a 403 on the whole request. */}
        {canViewPayroll && (
          <section className={sectionClass}>
            <h3 className={sectionTitleClass}>{t('employees.form.bankingDetails')}</h3>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
              <FormField label={t('employees.form.bankName')}>
                {(aria) => (
                  <select
                    {...aria}
                    value={form.bank_name || ''}
                    onChange={(e) => updateField('bank_name', e.target.value || undefined)}
                    className="form-input"
                  >
                    <option value="">{t('employees.form.selectBank')}</option>
                    {BANKS.map((b) => (
                      <option key={b} value={b}>
                        {b}
                      </option>
                    ))}
                  </select>
                )}
              </FormField>
              <FormField label={t('employees.form.accountNumber')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.bank_account_number || ''}
                    onChange={(e) => updateField('bank_account_number', e.target.value || undefined)}
                    className="form-input"
                  />
                )}
              </FormField>
            </div>
          </section>
        )}

        {canViewPayroll && (
          <section className={sectionClass}>
            <h3 className={sectionTitleClass}>{t('employees.form.statutoryTax')}</h3>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
              <FormField label={t('employees.fields.tin')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.tax_identification_number || ''}
                    onChange={(e) => updateField('tax_identification_number', e.target.value || undefined)}
                    className="form-input"
                  />
                )}
              </FormField>
              <FormField label={t('employees.fields.epfNumber')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.epf_number || ''}
                    onChange={(e) => updateField('epf_number', e.target.value || undefined)}
                    className="form-input"
                  />
                )}
              </FormField>
              <FormField label={t('employees.fields.socsoNumber')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.socso_number || ''}
                    onChange={(e) => updateField('socso_number', e.target.value || undefined)}
                    className="form-input"
                  />
                )}
              </FormField>
              <FormField label={t('employees.fields.eisNumber')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    value={form.eis_number || ''}
                    onChange={(e) => updateField('eis_number', e.target.value || undefined)}
                    className="form-input"
                  />
                )}
              </FormField>
              <FormField label={t('employees.fields.epfCategory')}>
                {(aria) => (
                  <select
                    {...aria}
                    value={form.epf_category ?? ''}
                    onChange={(e) => updateField('epf_category', e.target.value || undefined)}
                    className="form-input"
                  >
                    <option value="">{t('enums.epfCategory.auto')}</option>
                    <option value="A">{t('enums.epfCategory.A')}</option>
                    <option value="B">{t('enums.epfCategory.B')}</option>
                    <option value="C">{t('enums.epfCategory.C')}</option>
                    <option value="D">{t('enums.epfCategory.D')}</option>
                    <option value="E">{t('enums.epfCategory.E')}</option>
                    <option value="F">{t('enums.epfCategory.F')}</option>
                  </select>
                )}
              </FormField>
              <FormField label={t('employees.fields.residency')}>
                {(aria) => (
                  <select
                    {...aria}
                    value={form.residency_status || 'citizen'}
                    onChange={(e) => updateField('residency_status', e.target.value)}
                    className="form-input"
                  >
                    <option value="citizen">{t('enums.residency.citizen')}</option>
                    <option value="permanent_resident">{t('enums.residency.permanent_resident')}</option>
                    <option value="foreigner">{t('enums.residency.foreigner')}</option>
                  </select>
                )}
              </FormField>
              <FormField label={t('employees.fields.workingSpouse')}>
                {(aria) => (
                  <select
                    {...aria}
                    value={form.working_spouse ? 'yes' : 'no'}
                    onChange={(e) => updateField('working_spouse', e.target.value === 'yes')}
                    className="form-input"
                  >
                    <option value="no">{t('common.no')}</option>
                    <option value="yes">{t('common.yes')}</option>
                  </select>
                )}
              </FormField>
              <FormField label={t('employees.form.numChildren')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="number"
                    min="0"
                    value={form.num_children ?? 0}
                    onChange={(e) => updateField('num_children', Math.max(0, parseInt(e.target.value, 10) || 0))}
                    className="form-input"
                  />
                )}
              </FormField>
              <div className="md:col-span-2 flex flex-wrap items-center gap-6 pt-2">
                <label className="flex items-center gap-2.5 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={form.is_muslim || false}
                    onChange={(e) => {
                      updateField('is_muslim', e.target.checked);
                      if (!e.target.checked) {
                        updateField('zakat_eligible', false);
                        updateField('zakat_monthly_amount', mode === 'edit' ? 0 : undefined);
                        setZakatDisplay('');
                      }
                    }}
                    className="rounded border-gray-300 dark:border-slate-700 w-4 h-4 text-gray-900 focus:ring-black"
                  />
                  <span className="text-sm font-medium text-gray-700 dark:text-slate-200">
                    {t('employees.fields.muslim')}
                  </span>
                </label>
                {form.is_muslim && (
                  <label className="flex items-center gap-2.5 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={form.zakat_eligible || false}
                      onChange={(e) => {
                        updateField('zakat_eligible', e.target.checked);
                        if (!e.target.checked) {
                          updateField('zakat_monthly_amount', mode === 'edit' ? 0 : undefined);
                          setZakatDisplay('');
                        }
                      }}
                      className="rounded border-gray-300 dark:border-slate-700 w-4 h-4 text-gray-900 focus:ring-black"
                    />
                    <span className="text-sm font-medium text-gray-700 dark:text-slate-200">
                      {t('employees.form.zakatEligible')}
                    </span>
                  </label>
                )}
              </div>
              {form.is_muslim && form.zakat_eligible && (
                <FormField label={t('employees.fields.zakatMonthly')}>
                  {(aria) => (
                    <input
                      {...aria}
                      type="number"
                      min="0"
                      step="0.01"
                      value={zakatDisplay}
                      onChange={(e) => handleZakatChange(e.target.value)}
                      placeholder="0.00"
                      className="form-input"
                    />
                  )}
                </FormField>
              )}
              <FormField label={t('employees.fields.ptptnMonthly')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="number"
                    min="0"
                    step="0.01"
                    value={ptptnDisplay}
                    onChange={(e) => handlePtptnChange(e.target.value)}
                    placeholder="0.00"
                    className="form-input"
                  />
                )}
              </FormField>
            </div>
          </section>
        )}

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
    </Modal>
  );
}
