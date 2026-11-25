import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from '@/i18n';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  Building2,
  Shield,
  MapPin,
  Users,
  FolderOpen,
  Calculator,
} from 'lucide-react';
import { getCompany, updateCompany, getCompanyStats } from '@/api/company';
import { getErrorMessage } from '@/lib/utils';
import { formatDateLong } from '@/lib/format';
import { EmptyState } from '@/components/ui/EmptyState';
import { Modal } from '@/components/ui/Modal';
import { FormField } from '@/components/ui/FormField';
import { PageHeader } from '@/components/ui/PageHeader';
import { KpiSkeletonGrid } from '@/components/ui/Skeleton';
import { useConfirm } from '@/lib/confirm';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { useUnsavedChanges } from '@/lib/useUnsavedChanges';
import type { UpdateCompanyRequest } from '@/types';
import { useAuth } from '@/context/AuthContext';
import { CheckInCard } from '@/components/attendance/CheckInCard';

const renderRow = (label: string, value: string | null | undefined) => (
  <div className="flex justify-between gap-4 py-2.5 border-b border-gray-100 dark:border-slate-800 last:border-none text-sm">
    <span className="text-gray-500 dark:text-slate-400 shrink-0">{label}</span>
    <span className="font-medium text-gray-800 dark:text-slate-200 text-right break-words min-w-0">
      {value || <span className="italic text-gray-400 dark:text-slate-500">{i18n.t('common.notProvided')}</span>}
    </span>
  </div>
);

function FieldInput({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <FormField label={label}>
      {(aria) => (
        <input
          {...aria}
          type="text"
          value={value ?? ''}
          onChange={(e) => onChange(e.target.value)}
          placeholder={label}
          className="form-input"
        />
      )}
    </FormField>
  );
}

type Section = 'info' | 'statutory' | 'address' | 'payroll';

const STATES = [
  'Johor', 'Kedah', 'Kelantan', 'Melaka', 'Negeri Sembilan',
  'Pahang', 'Perak', 'Perlis', 'Pulau Pinang', 'Sabah',
  'Sarawak', 'Selangor', 'Terengganu',
  'W.P. Kuala Lumpur', 'W.P. Labuan', 'W.P. Putrajaya',
];

export function CompanyProfile() {
  const { user } = useAuth();
  return (
    <div className="space-y-6">
      {user?.employee_id && <CheckInCard />}
      <CompanyDetails />
    </div>
  );
}

function CompanyDetails() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const { can } = usePermissions();
  const canManageCompany = can('manage_company_settings');
  const canViewPayroll = can('view_payroll');
  const canViewEmployees = can('view_employees');
  const toast = useToast();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [activeSection, setActiveSection] = useState<Section | null>(null);
  const [form, setForm] = useState<UpdateCompanyRequest>({});
  const [divisorInput, setDivisorInput] = useState('');
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState('');

  useUnsavedChanges(dirty && activeSection !== null);

  const {
    data: company,
    isLoading,
    isError,
    error: queryError,
    refetch,
  } = useQuery({
    queryKey: ['company'],
    queryFn: getCompany,
  });

  const { data: stats } = useQuery({
    queryKey: ['company-stats'],
    queryFn: getCompanyStats,
    enabled: Boolean(user?.company_id) && canViewEmployees,
  });

  const resetModal = () => {
    setActiveSection(null);
    setForm({});
    setDivisorInput('');
    setDirty(false);
    setError('');
  };

  const requestCloseModal = async () => {
    if (dirty && !mutation.isPending) {
      const ok = await confirm({
        title: t('common.discard'),
        description: t('common.discardConfirm'),
        confirmLabel: t('common.discard'),
        variant: 'warning',
      });
      if (!ok) return;
    }
    resetModal();
  };

  const mutation = useMutation({
    mutationFn: updateCompany,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['company'] });
      queryClient.invalidateQueries({ queryKey: ['admin-companies'] });
      toast.success(t('companyProfile.updated'));
      resetModal();
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('companyProfile.updateFailed')));
    },
  });

  const isSectionEditable = (section: Section) => {
    if (!canManageCompany) return false;
    if (section === 'statutory' || section === 'payroll') return canViewPayroll;
    return true;
  };

  const openModal = (section: Section) => {
    if (!company || !isSectionEditable(section)) return;
    setError('');
    setDirty(false);
    setActiveSection(section);

    if (section === 'info') {
      setForm({
        name: company.name,
        registration_number: company.registration_number ?? '',
        ...(canViewPayroll ? { tax_number: company.tax_number ?? '' } : {}),
        phone: company.phone ?? '',
        email: company.email ?? '',
      });
    } else if (section === 'statutory') {
      setForm({
        epf_number: company.epf_number ?? '',
        socso_code: company.socso_code ?? '',
        eis_code: company.eis_code ?? '',
        hrdf_number: company.hrdf_number ?? '',
        hrdf_enabled: company.hrdf_enabled ?? false,
      });
    } else if (section === 'address') {
      setForm({
        address_line1: company.address_line1 ?? '',
        address_line2: company.address_line2 ?? '',
        city: company.city ?? '',
        state: company.state ?? '',
        postcode: company.postcode ?? '',
        country: company.country ?? 'Malaysia',
      });
    } else if (section === 'payroll') {
      setDivisorInput(company.unpaid_leave_divisor != null ? String(company.unpaid_leave_divisor) : '');
      setForm({});
    }
  };

  const handleCardKeyDown = (e: React.KeyboardEvent<HTMLDivElement>, section: Section) => {
    if (!isSectionEditable(section)) return;
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      openModal(section);
    }
  };

  const handleSave = (e?: React.FormEvent) => {
    e?.preventDefault();
    setError('');
    if (activeSection === 'info') {
      const trimmedName = form.name?.trim() ?? '';
      if (!trimmedName) {
        setError(t('companyProfile.nameRequired'));
        return;
      }
      mutation.mutate({ ...form, name: trimmedName });
      return;
    }
    if (activeSection === 'payroll') {
      const trimmed = divisorInput.trim();
      if (!trimmed) {
        mutation.mutate({ clear_unpaid_leave_divisor: true });
        return;
      }
      const parsed = Number(trimmed);
      if (!Number.isInteger(parsed) || parsed < 1 || parsed > 31) {
        setError(t('companyProfile.divisorHint'));
        return;
      }
      mutation.mutate({ unpaid_leave_divisor: parsed });
      return;
    }
    mutation.mutate(form);
  };

  const updateField = (key: string, value: string | boolean | number) => {
    setDirty(true);
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  if (isLoading) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('companyProfile.title')} />
        <KpiSkeletonGrid count={2} />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('companyProfile.title')} />
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('companyProfile.loadFailed')}
            description={getErrorMessage(queryError, t('common.error'))}
            action={
              <button type="button" onClick={() => refetch()} className="btn-secondary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      </div>
    );
  }

  if (!company) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('companyProfile.title')} />
        <div className="card">
          <EmptyState icon={Building2} title={t('companyProfile.notFound')} />
        </div>
      </div>
    );
  }

  const canEditInfo = isSectionEditable('info');
  const canEditStatutory = isSectionEditable('statutory');
  const canEditAddress = isSectionEditable('address');
  const canEditPayroll = isSectionEditable('payroll');

  const modalTitle =
    activeSection === 'info'
      ? t('companyProfile.editInfo')
      : activeSection === 'statutory'
        ? t('companyProfile.editStatutory')
        : activeSection === 'address'
          ? t('companyProfile.editAddress')
          : activeSection === 'payroll'
            ? t('companyProfile.editPayroll')
            : undefined;

  return (
    <div>
      <PageHeader
        title={t('companyProfile.title')}
        subtitle={canViewPayroll ? t('companyProfile.subtitleFull') : t('companyProfile.subtitleBasic')}
      />

      {/* Stats Cards */}
      {stats && (
        <div className="grid grid-cols-2 gap-4 mb-6">
          <div className="card !p-4">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-[var(--accent-soft)] flex items-center justify-center">
                <Users className="w-5 h-5 text-[var(--accent-1)]" />
              </div>
              <div>
                <p className="text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">{stats.total_employees}</p>
                <p className="text-xs text-gray-500 dark:text-slate-400">{t('companyProfile.activeEmployees')}</p>
              </div>
            </div>
          </div>
          <div className="card !p-4">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-violet-500/10 flex items-center justify-center">
                <FolderOpen className="w-5 h-5 text-violet-600 dark:text-violet-400" />
              </div>
              <div>
                <p className="text-2xl font-bold tabular-nums text-gray-900 dark:text-slate-100">{stats.total_departments}</p>
                <p className="text-xs text-gray-500 dark:text-slate-400">{t('companyProfile.departments')}</p>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Company Details Cards Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Company Information */}
        <div
          role={canEditInfo ? 'button' : undefined}
          tabIndex={canEditInfo ? 0 : undefined}
          onClick={canEditInfo ? () => openModal('info') : undefined}
          onKeyDown={canEditInfo ? (e) => handleCardKeyDown(e, 'info') : undefined}
          className={`card ${
            canEditInfo
              ? 'hover-lift cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]'
              : ''
          }`}
        >
          <div className="flex justify-between items-center gap-2 mb-3">
            <h2 className="font-semibold text-gray-800 dark:text-slate-100 flex items-center gap-2">
              <Building2 className="w-5 h-5 text-[var(--accent-1)] shrink-0" /> {t('companyProfile.infoSection')}
            </h2>
            {canEditInfo ? (
              <span className="text-xs font-medium text-[var(--accent-1)] shrink-0">{t('common.clickToEdit')}</span>
            ) : (
              <span className="text-xs text-gray-400 dark:text-slate-500 shrink-0">{t('companyProfile.readOnly')}</span>
            )}
          </div>
          {renderRow(t('companies.name'), company.name)}
          {renderRow(t('companies.regNo'), company.registration_number)}
          {renderRow(t('companies.taxNo'), company.tax_number)}
          {renderRow(t('companies.phone'), company.phone)}
          {renderRow(t('common.email'), company.email)}
        </div>

        {canViewPayroll && (
          <div
            role={canEditStatutory ? 'button' : undefined}
            tabIndex={canEditStatutory ? 0 : undefined}
            onClick={canEditStatutory ? () => openModal('statutory') : undefined}
            onKeyDown={canEditStatutory ? (e) => handleCardKeyDown(e, 'statutory') : undefined}
            className={`card ${
              canEditStatutory
                ? 'hover-lift cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]'
                : ''
            }`}
          >
            <div className="flex justify-between items-center gap-2 mb-3">
              <h2 className="font-semibold text-gray-800 dark:text-slate-100 flex items-center gap-2">
                <Shield className="w-5 h-5 text-[var(--accent-1)] shrink-0" /> {t('portal.profile.statutoryDetails')}
              </h2>
              {canEditStatutory && (
                <span className="text-xs font-medium text-[var(--accent-1)] shrink-0">{t('common.clickToEdit')}</span>
              )}
            </div>
            {renderRow(t('companyProfile.epfNo'), company.epf_number)}
            {renderRow(t('companies.socsoCode'), company.socso_code)}
            {renderRow(t('companies.eisCode'), company.eis_code)}
            {renderRow(t('companyProfile.hrdfNo'), company.hrdf_number)}
            <div className="flex justify-between gap-4 py-2.5 border-b border-gray-100 dark:border-slate-800 last:border-none text-sm">
              <span className="text-gray-500 dark:text-slate-400">{t('companyProfile.hrdfEnabled')}</span>
              <span
                className={`inline-flex px-2 py-0.5 rounded-full text-xs font-medium ${
                  company.hrdf_enabled
                    ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300'
                    : 'bg-gray-100 text-gray-500 dark:bg-slate-800 dark:text-slate-400'
                }`}
              >
                {company.hrdf_enabled ? t('common.yes') : t('common.no')}
              </span>
            </div>
          </div>
        )}

        {/* Address */}
        <div
          role={canEditAddress ? 'button' : undefined}
          tabIndex={canEditAddress ? 0 : undefined}
          onClick={canEditAddress ? () => openModal('address') : undefined}
          onKeyDown={canEditAddress ? (e) => handleCardKeyDown(e, 'address') : undefined}
          className={`card ${
            canEditAddress
              ? 'hover-lift cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]'
              : ''
          }`}
        >
          <div className="flex justify-between items-center gap-2 mb-3">
            <h2 className="font-semibold text-gray-800 dark:text-slate-100 flex items-center gap-2">
              <MapPin className="w-5 h-5 text-[var(--accent-1)] shrink-0" /> {t('portal.profile.address')}
            </h2>
            {canEditAddress && (
              <span className="text-xs font-medium text-[var(--accent-1)] shrink-0">{t('common.clickToEdit')}</span>
            )}
          </div>
          {renderRow(t('employees.form.addressLine1'), company.address_line1)}
          {renderRow(t('employees.form.addressLine2'), company.address_line2)}
          {renderRow(t('employees.form.city'), company.city)}
          {renderRow(t('employees.form.state'), company.state)}
          {renderRow(t('employees.form.postcode'), company.postcode)}
          {renderRow(t('employees.form.country'), company.country)}
        </div>

        {canViewPayroll && (
          <div
            role={canEditPayroll ? 'button' : undefined}
            tabIndex={canEditPayroll ? 0 : undefined}
            onClick={canEditPayroll ? () => openModal('payroll') : undefined}
            onKeyDown={canEditPayroll ? (e) => handleCardKeyDown(e, 'payroll') : undefined}
            className={`card ${
              canEditPayroll
                ? 'hover-lift cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]'
                : ''
            }`}
          >
            <div className="flex justify-between items-center gap-2 mb-3">
              <h2 className="font-semibold text-gray-800 dark:text-slate-100 flex items-center gap-2">
                <Calculator className="w-5 h-5 text-[var(--accent-1)] shrink-0" /> {t('companyProfile.payrollSection')}
              </h2>
              {canEditPayroll && (
                <span className="text-xs font-medium text-[var(--accent-1)] shrink-0">{t('common.clickToEdit')}</span>
              )}
            </div>
            {renderRow(
              t('companyProfile.unpaidLeaveDivisor'),
              company.unpaid_leave_divisor != null
                ? String(company.unpaid_leave_divisor)
                : t('companyProfile.divisorDefault'),
            )}
            {renderRow(t('common.status'), company.is_active ? t('common.active') : t('common.inactive'))}
            {renderRow(t('companyProfile.created'), formatDateLong(company.created_at))}
            {renderRow(t('companyProfile.lastUpdated'), formatDateLong(company.updated_at))}
          </div>
        )}
      </div>

      {/* Accessible Edit Modal */}
      <Modal
        open={activeSection !== null}
        onClose={() => { void requestCloseModal(); }}
        title={modalTitle}
        maxWidth="max-w-lg"
        footer={
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" onClick={() => { void requestCloseModal(); }} className="btn-secondary">
              {t('common.cancel')}
            </button>
            <button type="submit" form="company-section-form" disabled={mutation.isPending} className="btn-primary">
              {mutation.isPending ? t('common.saving') : t('common.save')}
            </button>
          </div>
        }
      >
        <form id="company-section-form" onSubmit={handleSave} className="space-y-4">
          {error && (
            <div
              role="alert"
              className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-xl border border-red-100 dark:border-red-500/25"
            >
              {error}
            </div>
          )}

          {activeSection === 'info' && (
            <>
              <FieldInput label={t('companies.name')} value={form.name ?? ''} onChange={(v) => updateField('name', v)} />
              <FieldInput label={t('companies.regNoSsm')} value={form.registration_number ?? ''} onChange={(v) => updateField('registration_number', v)} />
              {canViewPayroll && (
                <FieldInput label={t('companies.taxNoLhdn')} value={form.tax_number ?? ''} onChange={(v) => updateField('tax_number', v)} />
              )}
              <FieldInput label={t('companies.phone')} value={form.phone ?? ''} onChange={(v) => updateField('phone', v)} />
              <FieldInput label={t('common.email')} value={form.email ?? ''} onChange={(v) => updateField('email', v)} />
            </>
          )}

          {activeSection === 'statutory' && (
            <>
              <FieldInput label={t('companyProfile.epfNoKwsp')} value={form.epf_number ?? ''} onChange={(v) => updateField('epf_number', v)} />
              <FieldInput label={t('companyProfile.socsoCodePerkeso')} value={form.socso_code ?? ''} onChange={(v) => updateField('socso_code', v)} />
              <FieldInput label={t('companies.eisCode')} value={form.eis_code ?? ''} onChange={(v) => updateField('eis_code', v)} />
              <FieldInput label={t('companyProfile.hrdfNo')} value={form.hrdf_number ?? ''} onChange={(v) => updateField('hrdf_number', v)} />
              <div className="flex items-center justify-between pt-1">
                <span className="form-label !mb-0">{t('companyProfile.hrdfEnabled')}</span>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={form.hrdf_enabled ?? false}
                    onChange={(e) => updateField('hrdf_enabled', e.target.checked)}
                    aria-label={t('companyProfile.hrdfEnabled')}
                    className="sr-only peer"
                  />
                  <div className="w-9 h-5 bg-gray-200 dark:bg-slate-700 peer-focus:ring-2 peer-focus:ring-[var(--ring)] rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-[var(--accent-1)]" />
                </label>
              </div>
            </>
          )}

          {activeSection === 'address' && (
            <>
              <FieldInput label={t('employees.form.addressLine1')} value={form.address_line1 ?? ''} onChange={(v) => updateField('address_line1', v)} />
              <FieldInput label={t('employees.form.addressLine2')} value={form.address_line2 ?? ''} onChange={(v) => updateField('address_line2', v)} />
              <FieldInput label={t('employees.form.city')} value={form.city ?? ''} onChange={(v) => updateField('city', v)} />
              <FormField label={t('employees.form.state')}>
                {(aria) => (
                  <select
                    {...aria}
                    value={form.state ?? ''}
                    onChange={(e) => updateField('state', e.target.value)}
                    className="form-input"
                  >
                    <option value="">{t('companyProfile.selectState')}</option>
                    {STATES.map((s) => (
                      <option key={s} value={s}>{s}</option>
                    ))}
                  </select>
                )}
              </FormField>
              <FieldInput label={t('employees.form.postcode')} value={form.postcode ?? ''} onChange={(v) => updateField('postcode', v)} />
              <FieldInput label={t('employees.form.country')} value={form.country ?? ''} onChange={(v) => updateField('country', v)} />
            </>
          )}

          {activeSection === 'payroll' && (
            <FormField
              label={t('companyProfile.unpaidLeaveDivisor')}
              hint={t('companyProfile.divisorHint')}
            >
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  inputMode="numeric"
                  value={divisorInput}
                  placeholder={t('companyProfile.divisorDefault')}
                  onChange={(e) => {
                    setDirty(true);
                    setDivisorInput(e.target.value);
                  }}
                  className="form-input"
                />
              )}
            </FormField>
          )}
        </form>
      </Modal>
    </div>
  );
}
