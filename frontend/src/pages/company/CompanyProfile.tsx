import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from '@/i18n';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
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
import { Modal } from '@/components/ui/Modal';
import { FormField } from '@/components/ui/FormField';
import { PageHeader } from '@/components/ui/PageHeader';
import { KpiSkeletonGrid } from '@/components/ui/Skeleton';
import { useUnsavedChanges } from '@/lib/useUnsavedChanges';
import type { UpdateCompanyRequest } from '@/types';
import { useAuth } from '@/context/AuthContext';
import { canAccessPayrollData } from '@/lib/roles';
import { CheckInCard } from '@/components/attendance/CheckInCard';

const renderRow = (label: string, value: string | null | undefined) => (
  <div className="flex justify-between py-2.5 border-b border-gray-100 dark:border-slate-800 last:border-none text-sm">
    <span className="text-gray-500 dark:text-slate-400">{label}</span>
    <span className="font-medium text-gray-800 dark:text-slate-200">
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
  const canViewPayroll = canAccessPayrollData(user);
  const queryClient = useQueryClient();
  const [activeSection, setActiveSection] = useState<Section | null>(null);
  const [form, setForm] = useState<UpdateCompanyRequest>({});
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState('');

  useUnsavedChanges(dirty && activeSection !== null);

  const { data: company, isLoading } = useQuery({
    queryKey: ['company'],
    queryFn: getCompany,
  });

  const { data: stats } = useQuery({
    queryKey: ['company-stats'],
    queryFn: getCompanyStats,
  });

  const closeModal = () => {
    setActiveSection(null);
    setForm({});
    setDirty(false);
    setError('');
  };

  const mutation = useMutation({
    mutationFn: updateCompany,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['company'] });
      closeModal();
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('companyProfile.updateFailed')));
    },
  });

  const openModal = (section: Section) => {
    if (!company) return;
    setError('');
    setDirty(false);
    setActiveSection(section);

    if (section === 'info') {
      setForm({
        name: company.name,
        registration_number: company.registration_number ?? '',
        tax_number: company.tax_number ?? '',
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
      setForm({
        unpaid_leave_divisor: company.unpaid_leave_divisor ?? 26,
      });
    }
  };

  const handleCardKeyDown = (e: React.KeyboardEvent<HTMLDivElement>, section: Section) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      openModal(section);
    }
  };

  const handleSave = () => {
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

  if (!company) {
    return <div className="text-center text-gray-500 py-12">{t('companyProfile.notFound')}</div>;
  }

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

      {/* Clickable Cards Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Company Information */}
        <div
          role="button"
          tabIndex={0}
          onClick={() => openModal('info')}
          onKeyDown={(e) => handleCardKeyDown(e, 'info')}
          className="card hover-lift cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
        >
          <div className="flex justify-between items-center mb-3">
            <h2 className="font-semibold text-gray-800 dark:text-slate-100 flex items-center gap-2">
              <Building2 className="w-5 h-5 text-[var(--accent-1)]" /> {t('companyProfile.infoSection')}
            </h2>
            <span className="text-xs font-medium text-[var(--accent-1)]">{t('common.clickToEdit')}</span>
          </div>
          {renderRow(t('companies.name'), company.name)}
          {renderRow(t('companies.regNo'), company.registration_number)}
          {renderRow(t('companies.taxNo'), company.tax_number)}
          {renderRow(t('companies.phone'), company.phone)}
          {renderRow(t('common.email'), company.email)}
        </div>

        {canViewPayroll && (
          <div
            role="button"
            tabIndex={0}
            onClick={() => openModal('statutory')}
            onKeyDown={(e) => handleCardKeyDown(e, 'statutory')}
            className="card hover-lift cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
          >
            <div className="flex justify-between items-center mb-3">
              <h2 className="font-semibold text-gray-800 dark:text-slate-100 flex items-center gap-2">
                <Shield className="w-5 h-5 text-[var(--accent-1)]" /> {t('portal.profile.statutoryDetails')}
              </h2>
              <span className="text-xs font-medium text-[var(--accent-1)]">{t('common.clickToEdit')}</span>
            </div>
            {renderRow(t('companyProfile.epfNo'), company.epf_number)}
            {renderRow(t('companies.socsoCode'), company.socso_code)}
            {renderRow(t('companies.eisCode'), company.eis_code)}
            {renderRow(t('companyProfile.hrdfNo'), company.hrdf_number)}
            <div className="flex justify-between py-2.5 border-b border-gray-100 dark:border-slate-800 last:border-none text-sm">
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
          role="button"
          tabIndex={0}
          onClick={() => openModal('address')}
          onKeyDown={(e) => handleCardKeyDown(e, 'address')}
          className="card hover-lift cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
        >
          <div className="flex justify-between items-center mb-3">
            <h2 className="font-semibold text-gray-800 dark:text-slate-100 flex items-center gap-2">
              <MapPin className="w-5 h-5 text-[var(--accent-1)]" /> {t('portal.profile.address')}
            </h2>
            <span className="text-xs font-medium text-[var(--accent-1)]">{t('common.clickToEdit')}</span>
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
            role="button"
            tabIndex={0}
            onClick={() => openModal('payroll')}
            onKeyDown={(e) => handleCardKeyDown(e, 'payroll')}
            className="card hover-lift cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
          >
            <div className="flex justify-between items-center mb-3">
              <h2 className="font-semibold text-gray-800 dark:text-slate-100 flex items-center gap-2">
                <Calculator className="w-5 h-5 text-[var(--accent-1)]" /> {t('companyProfile.payrollSection')}
              </h2>
              <span className="text-xs font-medium text-[var(--accent-1)]">{t('common.clickToEdit')}</span>
            </div>
            {renderRow(t('companyProfile.unpaidLeaveDivisor'), String(company.unpaid_leave_divisor ?? 26))}
            {renderRow(t('common.status'), company.is_active ? t('common.active') : t('common.inactive'))}
            {renderRow(t('companyProfile.created'), formatDateLong(company.created_at))}
            {renderRow(t('companyProfile.lastUpdated'), formatDateLong(company.updated_at))}
          </div>
        )}
      </div>

      {/* Accessible Edit Modal */}
      <Modal
        open={activeSection !== null}
        onClose={closeModal}
        title={modalTitle}
        maxWidth="max-w-lg"
        footer={
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" onClick={closeModal} className="btn-secondary">
              {t('common.cancel')}
            </button>
            <button type="button" onClick={handleSave} disabled={mutation.isPending} className="btn-primary">
              {mutation.isPending ? t('common.saving') : t('common.save')}
            </button>
          </div>
        }
      >
        <div className="space-y-4">
          {error && (
            <div className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-xl border border-red-100 dark:border-red-500/25">
              {error}
            </div>
          )}

          {activeSection === 'info' && (
            <>
              <FieldInput label={t('companies.name')} value={form.name ?? ''} onChange={(v) => updateField('name', v)} />
              <FieldInput label={t('companies.regNoSsm')} value={form.registration_number ?? ''} onChange={(v) => updateField('registration_number', v)} />
              <FieldInput label={t('companies.taxNoLhdn')} value={form.tax_number ?? ''} onChange={(v) => updateField('tax_number', v)} />
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
                  type="number"
                  value={form.unpaid_leave_divisor ?? ''}
                  onChange={(e) => updateField('unpaid_leave_divisor', Number(e.target.value))}
                  className="form-input"
                />
              )}
            </FormField>
          )}
        </div>
      </Modal>
    </div>
  );
}
