import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from '@/i18n';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { motion, AnimatePresence } from 'framer-motion';
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
import type { UpdateCompanyRequest } from '@/types';
import { useAuth } from '@/context/AuthContext';
import { canAccessPayrollData } from '@/lib/roles';
import { CheckInCard } from '@/components/attendance/CheckInCard';

const renderRow = (label: string, value: string | null | undefined) => (
  <div className="flex justify-between py-2.5 border-b border-gray-100 last:border-none text-sm">
    <span className="text-gray-500">{label}</span>
    <span className="font-medium text-gray-800">
      {value || <span className="italic text-gray-400">{i18n.t('common.notProvided')}</span>}
    </span>
  </div>
);

function FieldInput({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <div>
      <label className="block text-sm text-gray-500 mb-1">{label}</label>
      <input
        type="text"
        value={value ?? ''}
        onChange={(e) => onChange(e.target.value)}
        placeholder={label}
        className="w-full border p-2 rounded-lg text-sm focus:border-black outline-none transition-colors"
      />
    </div>
  );
}

type Section = 'info' | 'statutory' | 'address' | 'payroll';

const STATES = [
  'Johor', 'Kedah', 'Kelantan', 'Melaka', 'Negeri Sembilan',
  'Pahang', 'Perak', 'Perlis', 'Pulau Pinang', 'Sabah',
  'Sarawak', 'Selangor', 'Terengganu',
  'W.P. Kuala Lumpur', 'W.P. Labuan', 'W.P. Putrajaya',
];

/**
 * The admin landing screen — `/` redirects here for every non-super_admin — so
 * it is also where a supervisor who punches a clock arrives on a cold open.
 *
 * The check-in card is mounted *before* the company fetch, deliberately: the
 * portal home already refuses to put a spinner between an employee and the one
 * action they open the app to perform twice a day, and this surface serves the
 * same people. Keyed on the employee link rather than on the `employee` role, so
 * it appears for exactly the accounts that have something to check into.
 */
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
  const [error, setError] = useState('');

  const { data: company, isLoading } = useQuery({
    queryKey: ['company'],
    queryFn: getCompany,
  });

  const { data: stats } = useQuery({
    queryKey: ['company-stats'],
    queryFn: getCompanyStats,
  });

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

  const closeModal = () => {
    setActiveSection(null);
    setForm({});
    setError('');
  };

  const handleSave = () => {
    mutation.mutate(form);
  };

  const updateField = (key: string, value: string | boolean | number) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-black" />
      </div>
    );
  }

  if (!company) {
    return <div className="text-center text-gray-500 py-12">{t('companyProfile.notFound')}</div>;
  }


  return (
    <div>
      {/* Page Header */}
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900">{t('companyProfile.title')}</h1>
        <p className="text-gray-500 text-sm mt-1">
          {canViewPayroll
            ? t('companyProfile.subtitleFull')
            : t('companyProfile.subtitleBasic')}
        </p>
      </div>

      {/* Stats Cards */}
      {stats && (
        <div className="grid grid-cols-2 gap-4 mb-6">
          <div className="bg-white rounded-2xl shadow p-4">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-lg bg-gray-100 flex items-center justify-center">
                <Users className="w-5 h-5 text-gray-700" />
              </div>
              <div>
                <p className="text-2xl font-bold text-gray-900">{stats.total_employees}</p>
                <p className="text-xs text-gray-500">{t('companyProfile.activeEmployees')}</p>
              </div>
            </div>
          </div>
          <div className="bg-white rounded-2xl shadow p-4">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-lg bg-purple-50 flex items-center justify-center">
                <FolderOpen className="w-5 h-5 text-purple-600" />
              </div>
              <div>
                <p className="text-2xl font-bold text-gray-900">{stats.total_departments}</p>
                <p className="text-xs text-gray-500">{t('companyProfile.departments')}</p>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Clickable Cards Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Company Information */}
        <div
          onClick={() => openModal('info')}
          className="bg-white rounded-2xl shadow-sm hover:shadow-md transition cursor-pointer p-5"
        >
          <div className="flex justify-between items-center mb-3">
            <h2 className="font-semibold text-gray-800 flex items-center gap-2">
              <Building2 className="w-5 h-5 text-gray-600" /> {t('companyProfile.infoSection')}
            </h2>
            <span className="text-xs text-gray-400">{t('common.clickToEdit')}</span>
          </div>
          {renderRow(t('companies.name'), company.name)}
          {renderRow(t('companies.regNo'), company.registration_number)}
          {renderRow(t('companies.taxNo'), company.tax_number)}
          {renderRow(t('companies.phone'), company.phone)}
          {renderRow(t('common.email'), company.email)}
        </div>

        {canViewPayroll && (
          <div
            onClick={() => openModal('statutory')}
            className="bg-white rounded-2xl shadow-sm hover:shadow-md transition cursor-pointer p-5"
          >
            <div className="flex justify-between items-center mb-3">
              <h2 className="font-semibold text-gray-800 flex items-center gap-2">
                <Shield className="w-5 h-5 text-gray-600" /> {t('portal.profile.statutoryDetails')}
              </h2>
              <span className="text-xs text-gray-400">{t('common.clickToEdit')}</span>
            </div>
            {renderRow(t('companyProfile.epfNo'), company.epf_number)}
            {renderRow(t('companies.socsoCode'), company.socso_code)}
            {renderRow(t('companies.eisCode'), company.eis_code)}
            {renderRow(t('companyProfile.hrdfNo'), company.hrdf_number)}
            <div className="flex justify-between py-2.5 border-b border-gray-100 last:border-none text-sm">
              <span className="text-gray-500">{t('companyProfile.hrdfEnabled')}</span>
              <span
                className={`inline-flex px-2 py-0.5 rounded-full text-xs font-medium ${
                  company.hrdf_enabled ? 'bg-green-50 text-green-700' : 'bg-gray-100 text-gray-500'
                }`}
              >
                {company.hrdf_enabled ? t('common.yes') : t('common.no')}
              </span>
            </div>
          </div>
        )}

        {/* Address */}
        <div
          onClick={() => openModal('address')}
          className="bg-white rounded-2xl shadow-sm hover:shadow-md transition cursor-pointer p-5"
        >
          <div className="flex justify-between items-center mb-3">
            <h2 className="font-semibold text-gray-800 flex items-center gap-2">
              <MapPin className="w-5 h-5 text-gray-600" /> {t('portal.profile.address')}
            </h2>
            <span className="text-xs text-gray-400">{t('common.clickToEdit')}</span>
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
            onClick={() => openModal('payroll')}
            className="bg-white rounded-2xl shadow-sm hover:shadow-md transition cursor-pointer p-5"
          >
            <div className="flex justify-between items-center mb-3">
              <h2 className="font-semibold text-gray-800 flex items-center gap-2">
                <Calculator className="w-5 h-5 text-gray-600" /> {t('companyProfile.payrollSection')}
              </h2>
              <span className="text-xs text-gray-400">{t('common.clickToEdit')}</span>
            </div>
            {renderRow(t('companyProfile.unpaidLeaveDivisor'), String(company.unpaid_leave_divisor ?? 26))}
            {renderRow(t('common.status'), company.is_active ? t('common.active') : t('common.inactive'))}
            {renderRow(t('companyProfile.created'), formatDateLong(company.created_at))}
            {renderRow(t('companyProfile.lastUpdated'), formatDateLong(company.updated_at))}
          </div>
        )}
      </div>

      {/* Edit Modal */}
      <AnimatePresence>
        {activeSection && (
          <>
            {/* Overlay */}
            <motion.div
              className="fixed top-0 left-0 z-50 w-screen h-screen bg-black/40 backdrop-blur-sm"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              onClick={closeModal}
            />

            {/* Centering container */}
            <motion.div
              className="fixed top-0 left-0 z-50 w-screen h-screen grid place-items-center p-4 pointer-events-none"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
            >
              <motion.div
                className="bg-white rounded-2xl shadow-2xl w-full max-w-lg p-6 pointer-events-auto"
                initial={{ scale: 0.95, opacity: 0, y: 20 }}
                animate={{ scale: 1, opacity: 1, y: 0 }}
                exit={{ scale: 0.95, opacity: 0, y: 20 }}
                transition={{ duration: 0.2 }}
                onClick={(e) => e.stopPropagation()}
              >
              <h2 className="text-lg font-semibold mb-4">
                {activeSection === 'info' && t('companyProfile.editInfo')}
                {activeSection === 'statutory' && t('companyProfile.editStatutory')}
                {activeSection === 'address' && t('companyProfile.editAddress')}
                {activeSection === 'payroll' && t('companyProfile.editPayroll')}
              </h2>

              {error && (
                <div className="mb-4 p-3 bg-red-50 text-red-700 text-sm rounded-lg">{error}</div>
              )}

              <div className="flex flex-col gap-3 max-h-[60vh] overflow-auto">
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
                    <div>
                      <label className="block text-sm text-gray-500 mb-1">{t('companyProfile.hrdfEnabled')}</label>
                      <label className="relative inline-flex items-center cursor-pointer">
                        <input
                          type="checkbox"
                          checked={form.hrdf_enabled ?? false}
                          onChange={(e) => updateField('hrdf_enabled', e.target.checked)}
                          className="sr-only peer"
                        />
                        <div className="w-9 h-5 bg-gray-200 peer-focus:ring-1 peer-focus:ring-black rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-black" />
                      </label>
                    </div>
                  </>
                )}

                {activeSection === 'address' && (
                  <>
                    <FieldInput label={t('employees.form.addressLine1')} value={form.address_line1 ?? ''} onChange={(v) => updateField('address_line1', v)} />
                    <FieldInput label={t('employees.form.addressLine2')} value={form.address_line2 ?? ''} onChange={(v) => updateField('address_line2', v)} />
                    <FieldInput label={t('employees.form.city')} value={form.city ?? ''} onChange={(v) => updateField('city', v)} />
                    <div>
                      <label className="block text-sm text-gray-500 mb-1">{t('employees.form.state')}</label>
                      <select
                        value={form.state ?? ''}
                        onChange={(e) => updateField('state', e.target.value)}
                        className="w-full border p-2 rounded-lg text-sm focus:border-black outline-none transition-colors"
                      >
                        <option value="">{t('companyProfile.selectState')}</option>
                        {STATES.map((s) => (
                          <option key={s} value={s}>{s}</option>
                        ))}
                      </select>
                    </div>
                    <FieldInput label={t('employees.form.postcode')} value={form.postcode ?? ''} onChange={(v) => updateField('postcode', v)} />
                    <FieldInput label={t('employees.form.country')} value={form.country ?? ''} onChange={(v) => updateField('country', v)} />
                  </>
                )}

                {activeSection === 'payroll' && (
                  <>
                    <div>
                      <label className="block text-sm text-gray-500 mb-1">{t('companyProfile.unpaidLeaveDivisor')}</label>
                      <input
                        type="number"
                        value={form.unpaid_leave_divisor ?? ''}
                        onChange={(e) => updateField('unpaid_leave_divisor', Number(e.target.value))}
                        className="w-full border p-2 rounded-lg text-sm focus:border-black outline-none transition-colors"
                      />
                      <p className="text-xs text-gray-400 mt-1.5">
                        {t('companyProfile.divisorHint')}
                      </p>
                    </div>
                  </>
                )}
              </div>

              <div className="flex gap-2 mt-5">
                <button
                  onClick={handleSave}
                  disabled={mutation.isPending}
                  className="flex-1 bg-black text-white py-2 rounded-xl font-medium hover:bg-gray-800 disabled:opacity-50 transition-colors"
                >
                  {mutation.isPending ? t('common.saving') : t('common.save')}
                </button>
                <button
                  onClick={closeModal}
                  className="flex-1 border py-2 rounded-xl font-medium text-gray-600 hover:bg-gray-50 transition-colors"
                >
                  {t('common.cancel')}
                </button>
              </div>
            </motion.div>
          </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}
