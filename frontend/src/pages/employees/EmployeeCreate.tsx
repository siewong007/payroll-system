import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Loader2 } from 'lucide-react';
import { createEmployee } from '@/api/employees';
import type { EmployeeAccountInfo } from '@/api/employees';
import { getPayrollGroups } from '@/api/payroll';
import { getErrorMessage, todayLocalDate } from '@/lib/utils';
import { stripPayrollFields } from '@/lib/employeeFields';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { UnsavedChangesBar } from '@/components/ui/UnsavedChangesBar';
import { useUnsavedChanges } from '@/lib/useUnsavedChanges';
import type { CreateEmployeeRequest } from '@/types';

const BANKS = [
  'Maybank', 'CIMB Bank', 'Public Bank', 'RHB Bank', 'Hong Leong Bank',
  'AmBank', 'Bank Islam', 'Bank Rakyat', 'Alliance Bank', 'Affin Bank',
  'BSN', 'OCBC Bank', 'Standard Chartered', 'HSBC', 'UOB', 'Agro Bank',
  'Bank Muamalat', 'MBSB Bank', 'Al Rajhi Bank',
];

export function EmployeeCreate() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const toast = useToast();
  const queryClient = useQueryClient();
  const { can } = usePermissions();
  const canViewPayroll = can('view_payroll');

  const { data: payrollGroups } = useQuery({
    queryKey: ['payrollGroups'],
    queryFn: getPayrollGroups,
    enabled: canViewPayroll,
  });

  const [initialJoinedDate] = useState(() => todayLocalDate());
  const initialForm = useMemo<CreateEmployeeRequest>(() => ({
    employee_number: '',
    full_name: '',
    date_joined: initialJoinedDate,
    basic_salary: 0,
  }), [initialJoinedDate]);

  const [form, setForm] = useState<CreateEmployeeRequest>(initialForm);
  const [salaryDisplay, setSalaryDisplay] = useState('');
  const [zakatDisplay, setZakatDisplay] = useState('');
  const [ptptnDisplay, setPtptnDisplay] = useState('');
  const [accountDialog, setAccountDialog] = useState<EmployeeAccountInfo | null>(null);
  const errorRef = useRef<HTMLDivElement>(null);

  const mutation = useMutation({
    mutationFn: createEmployee,
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      queryClient.invalidateQueries({ queryKey: ['employees-picker'] });
      if (data.account) {
        setAccountDialog(data.account);
      } else {
        toast.success(t('employees.createSuccess'));
        navigate('/employees');
      }
    },
    onError: (err: unknown) => toast.error(getErrorMessage(err, t('employees.createFailed'))),
  });

  useEffect(() => {
    if (mutation.isError) {
      errorRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }, [mutation.isError]);

  const isDirty =
    (JSON.stringify(form) !== JSON.stringify(initialForm) ||
      salaryDisplay !== '' ||
      zakatDisplay !== '' ||
      ptptnDisplay !== '') &&
    accountDialog === null &&
    !mutation.isSuccess;
  useUnsavedChanges(isDirty);

  const updateField = <K extends keyof CreateEmployeeRequest>(
    field: K,
    value: CreateEmployeeRequest[K],
  ) => {
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
      updateField('zakat_monthly_amount', undefined);
      return;
    }
    const parsed = parseFloat(value);
    updateField(
      'zakat_monthly_amount',
      Number.isFinite(parsed) ? Math.round(Math.max(0, parsed) * 100) : undefined,
    );
  };

  const handlePtptnChange = (value: string) => {
    setPtptnDisplay(value);
    if (!value.trim()) {
      updateField('ptptn_monthly_amount', undefined);
      return;
    }
    const parsed = parseFloat(value);
    updateField(
      'ptptn_monthly_amount',
      Number.isFinite(parsed) ? Math.round(Math.max(0, parsed) * 100) : undefined,
    );
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
        : ({ ...stripPayrollFields(form), basic_salary: 0 } as CreateEmployeeRequest),
    );
  };

  const cardClass = 'card';

  return (
    <div className="max-w-4xl space-y-6">
      <PageHeader
        className="mb-0"
        title={t('employees.createTitle')}
        breadcrumbs={[
          { label: t('nav.employees'), href: '/employees' },
          { label: t('employees.createTitle') },
        ]}
      />

      <form onSubmit={handleSubmit} className="space-y-6">
        {mutation.isError && (
          <div
            ref={errorRef}
            role="alert"
            className="bg-red-50 dark:bg-red-500/10 text-red-600 dark:text-red-300 border border-red-200 dark:border-red-500/30 text-sm px-4 py-3 rounded-lg"
          >
            {getErrorMessage(mutation.error, t('employees.createFailed'))}
          </div>
        )}

        {/* Personal Information */}
        <section className={cardClass}>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4">{t('employees.form.personalInfo')}</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <FormField label={t('employees.form.employeeNumber')} required>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={form.employee_number}
                  onChange={(e) => updateField('employee_number', e.target.value)}
                  className="form-input"
                  required
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
        <section className={cardClass}>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4">{t('employees.sections.address')}</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
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

        {/* Employment */}
        <section className={cardClass}>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4">{t('employees.form.employmentDetails')}</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
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
            <FormField label={t('employees.fields.dateJoined')} required>
              {(aria) => (
                <input
                  {...aria}
                  type="date"
                  value={form.date_joined}
                  onChange={(e) => updateField('date_joined', e.target.value)}
                  className="form-input"
                  required
                />
              )}
            </FormField>
            {canViewPayroll && (
              <>
                <FormField label={t('employees.form.basicSalaryRm')} required>
                  {(aria) => (
                    <input
                      {...aria}
                      type="number"
                      inputMode="decimal"
                      min="0"
                      step="0.01"
                      value={salaryDisplay}
                      onChange={(e) => handleSalaryChange(e.target.value)}
                      className="form-input tabular-nums"
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
          </div>
        </section>

        {/* Banking — payroll-sensitive server-side ("banking decides where
            salary lands"), so it is offered only to a role that can save it. */}
        {canViewPayroll && (
          <section className={cardClass}>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4">{t('employees.form.bankingDetails')}</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
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
                      <option key={b} value={b}>{b}</option>
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
          <section className={cardClass}>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4">{t('employees.form.statutoryTax')}</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
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
                    inputMode="numeric"
                    min="0"
                    value={form.num_children ?? 0}
                    onChange={(e) => updateField('num_children', Math.max(0, parseInt(e.target.value, 10) || 0))}
                    className="form-input tabular-nums"
                  />
                )}
              </FormField>
              <div className="md:col-span-2 flex flex-wrap items-center gap-6 pt-2">
                <label className="min-h-11 sm:min-h-0 flex items-center gap-2.5 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={form.is_muslim || false}
                    onChange={(e) => {
                      updateField('is_muslim', e.target.checked);
                      if (!e.target.checked) {
                        updateField('zakat_eligible', false);
                        updateField('zakat_monthly_amount', undefined);
                        setZakatDisplay('');
                      }
                    }}
                    className="w-4 h-4 rounded border-gray-300 dark:border-slate-700 accent-[var(--accent-1)]"
                  />
                  <span className="text-sm text-gray-700 dark:text-slate-200">{t('employees.fields.muslim')}</span>
                </label>
                {form.is_muslim && (
                  <label className="min-h-11 sm:min-h-0 flex items-center gap-2.5 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={form.zakat_eligible || false}
                      onChange={(e) => {
                        updateField('zakat_eligible', e.target.checked);
                        if (!e.target.checked) {
                          updateField('zakat_monthly_amount', undefined);
                          setZakatDisplay('');
                        }
                      }}
                      className="w-4 h-4 rounded border-gray-300 dark:border-slate-700 accent-[var(--accent-1)]"
                    />
                    <span className="text-sm text-gray-700 dark:text-slate-200">{t('employees.form.zakatEligible')}</span>
                  </label>
                )}
              </div>
              {form.is_muslim && form.zakat_eligible && (
                <FormField label={t('employees.fields.zakatMonthly')}>
                  {(aria) => (
                    <input
                      {...aria}
                      type="number"
                      inputMode="decimal"
                      min="0"
                      step="0.01"
                      value={zakatDisplay}
                      onChange={(e) => handleZakatChange(e.target.value)}
                      placeholder="0.00"
                      className="form-input tabular-nums"
                    />
                  )}
                </FormField>
              )}
              <FormField label={t('employees.fields.ptptnMonthly')}>
                {(aria) => (
                  <input
                    {...aria}
                    type="number"
                    inputMode="decimal"
                    min="0"
                    step="0.01"
                    value={ptptnDisplay}
                    onChange={(e) => handlePtptnChange(e.target.value)}
                    placeholder="0.00"
                    className="form-input tabular-nums"
                  />
                )}
              </FormField>
            </div>
          </section>
        )}

        {/* Submit — hidden once UnsavedChangesBar is active to prevent duplicate Cancel/Save bars */}
        {!isDirty && (
          <div className="flex flex-col-reverse sm:flex-row gap-3">
            <button
              type="button"
              onClick={() => navigate('/employees')}
              disabled={mutation.isPending}
              className="btn-secondary"
            >
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              disabled={mutation.isPending}
              className="btn-primary"
            >
              {mutation.isPending && <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />}
              {mutation.isPending ? t('common.creating') : t('employees.createSubmit')}
            </button>
          </div>
        )}

        <UnsavedChangesBar
          isDirty={isDirty}
          onDiscard={() => navigate('/employees')}
          submitForm
          saveLabel={t('employees.createSubmit')}
          isSaving={mutation.isPending}
        />
      </form>

      {/* Account Created Dialog */}
      <Modal
        open={accountDialog !== null}
        onClose={() => navigate('/employees')}
        title={accountDialog?.created ? t('employees.account.createdTitle') : t('employees.account.noticeTitle')}
        maxWidth="max-w-md"
        footer={
          <div className="flex justify-end">
            <button type="button" onClick={() => navigate('/employees')} className="btn-primary w-full sm:w-auto">
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
                  <div className="flex items-start justify-between gap-4 text-sm">
                    <span className="text-gray-500 dark:text-slate-400 shrink-0">{t('common.email')}</span>
                    <span className="font-medium text-gray-900 dark:text-slate-100 min-w-0 break-all text-right">
                      {accountDialog.email}
                    </span>
                  </div>
                  <div className="flex items-start justify-between gap-4 text-sm">
                    <span className="text-gray-500 dark:text-slate-400 shrink-0">{t('employees.account.role')}</span>
                    <span className="font-medium text-gray-900 dark:text-slate-100 min-w-0 break-all text-right">
                      {t(`roles.${accountDialog.role}`, { defaultValue: accountDialog.role })}
                    </span>
                  </div>
                  <div className="flex items-start justify-between gap-4 text-sm">
                    <span className="text-gray-500 dark:text-slate-400 shrink-0">{t('employees.account.defaultPassword')}</span>
                    <span className="font-mono font-medium text-gray-900 dark:text-slate-100 min-w-0 break-all text-right">
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
                <p className="text-sm text-amber-800 dark:text-amber-200">{accountDialog.message}</p>
              </div>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
}
