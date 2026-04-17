import { useState } from 'react';
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
import type { UpdateCompanyRequest } from '@/types';

const renderRow = (label: string, value: string | null | undefined) => (
  <div className="flex justify-between py-2.5 border-b border-gray-100 last:border-none text-sm">
    <span className="text-gray-500">{label}</span>
    <span className="font-medium text-gray-800">
      {value || <span className="italic text-gray-400">Not provided</span>}
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

export function CompanyProfile() {
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
      setError(getErrorMessage(err, 'Failed to update company settings. Please try again.'));
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
    return <div className="text-center text-gray-500 py-12">Company not found</div>;
  }


  return (
    <div>
      {/* Page Header */}
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900">Company Profile</h1>
        <p className="text-gray-500 text-sm mt-1">Manage your company information and statutory details</p>
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
                <p className="text-xs text-gray-500">Active Employees</p>
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
                <p className="text-xs text-gray-500">Departments</p>
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
              <Building2 className="w-5 h-5 text-gray-600" /> Company Information
            </h2>
            <span className="text-xs text-gray-400">Click to edit</span>
          </div>
          {renderRow('Company Name', company.name)}
          {renderRow('Registration No.', company.registration_number)}
          {renderRow('Tax No.', company.tax_number)}
          {renderRow('Phone', company.phone)}
          {renderRow('Email', company.email)}
        </div>

        {/* Statutory Details */}
        <div
          onClick={() => openModal('statutory')}
          className="bg-white rounded-2xl shadow-sm hover:shadow-md transition cursor-pointer p-5"
        >
          <div className="flex justify-between items-center mb-3">
            <h2 className="font-semibold text-gray-800 flex items-center gap-2">
              <Shield className="w-5 h-5 text-gray-600" /> Statutory Details
            </h2>
            <span className="text-xs text-gray-400">Click to edit</span>
          </div>
          {renderRow('EPF No.', company.epf_number)}
          {renderRow('SOCSO Code', company.socso_code)}
          {renderRow('EIS Code', company.eis_code)}
          {renderRow('HRDF No.', company.hrdf_number)}
          <div className="flex justify-between py-2.5 border-b border-gray-100 last:border-none text-sm">
            <span className="text-gray-500">HRDF Enabled</span>
            <span
              className={`inline-flex px-2 py-0.5 rounded-full text-xs font-medium ${
                company.hrdf_enabled ? 'bg-green-50 text-green-700' : 'bg-gray-100 text-gray-500'
              }`}
            >
              {company.hrdf_enabled ? 'Yes' : 'No'}
            </span>
          </div>
        </div>

        {/* Address */}
        <div
          onClick={() => openModal('address')}
          className="bg-white rounded-2xl shadow-sm hover:shadow-md transition cursor-pointer p-5"
        >
          <div className="flex justify-between items-center mb-3">
            <h2 className="font-semibold text-gray-800 flex items-center gap-2">
              <MapPin className="w-5 h-5 text-gray-600" /> Address
            </h2>
            <span className="text-xs text-gray-400">Click to edit</span>
          </div>
          {renderRow('Address Line 1', company.address_line1)}
          {renderRow('Address Line 2', company.address_line2)}
          {renderRow('City', company.city)}
          {renderRow('State', company.state)}
          {renderRow('Postcode', company.postcode)}
          {renderRow('Country', company.country)}
        </div>

        {/* Payroll Configuration */}
        <div
          onClick={() => openModal('payroll')}
          className="bg-white rounded-2xl shadow-sm hover:shadow-md transition cursor-pointer p-5"
        >
          <div className="flex justify-between items-center mb-3">
            <h2 className="font-semibold text-gray-800 flex items-center gap-2">
              <Calculator className="w-5 h-5 text-gray-600" /> Payroll Configuration
            </h2>
            <span className="text-xs text-gray-400">Click to edit</span>
          </div>
          {renderRow('Unpaid Leave Divisor', String(company.unpaid_leave_divisor ?? 26))}
          {renderRow('Status', company.is_active ? 'Active' : 'Inactive')}
          {renderRow('Created', new Date(company.created_at).toLocaleDateString('en-MY', { day: 'numeric', month: 'long', year: 'numeric' }))}
          {renderRow('Last Updated', new Date(company.updated_at).toLocaleDateString('en-MY', { day: 'numeric', month: 'long', year: 'numeric' }))}
        </div>
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
                {activeSection === 'info' && 'Edit Company Information'}
                {activeSection === 'statutory' && 'Edit Statutory Details'}
                {activeSection === 'address' && 'Edit Address'}
                {activeSection === 'payroll' && 'Edit Payroll Configuration'}
              </h2>

              {error && (
                <div className="mb-4 p-3 bg-red-50 text-red-700 text-sm rounded-lg">{error}</div>
              )}

              <div className="flex flex-col gap-3 max-h-[60vh] overflow-auto">
                {activeSection === 'info' && (
                  <>
                    <FieldInput label="Company Name" value={form.name ?? ''} onChange={(v) => updateField('name', v)} />
                    <FieldInput label="Registration No. (SSM)" value={form.registration_number ?? ''} onChange={(v) => updateField('registration_number', v)} />
                    <FieldInput label="Tax No. (LHDN)" value={form.tax_number ?? ''} onChange={(v) => updateField('tax_number', v)} />
                    <FieldInput label="Phone" value={form.phone ?? ''} onChange={(v) => updateField('phone', v)} />
                    <FieldInput label="Email" value={form.email ?? ''} onChange={(v) => updateField('email', v)} />
                  </>
                )}

                {activeSection === 'statutory' && (
                  <>
                    <FieldInput label="EPF No. (KWSP)" value={form.epf_number ?? ''} onChange={(v) => updateField('epf_number', v)} />
                    <FieldInput label="SOCSO Code (PERKESO)" value={form.socso_code ?? ''} onChange={(v) => updateField('socso_code', v)} />
                    <FieldInput label="EIS Code" value={form.eis_code ?? ''} onChange={(v) => updateField('eis_code', v)} />
                    <FieldInput label="HRDF No." value={form.hrdf_number ?? ''} onChange={(v) => updateField('hrdf_number', v)} />
                    <div>
                      <label className="block text-sm text-gray-500 mb-1">HRDF Enabled</label>
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
                    <FieldInput label="Address Line 1" value={form.address_line1 ?? ''} onChange={(v) => updateField('address_line1', v)} />
                    <FieldInput label="Address Line 2" value={form.address_line2 ?? ''} onChange={(v) => updateField('address_line2', v)} />
                    <FieldInput label="City" value={form.city ?? ''} onChange={(v) => updateField('city', v)} />
                    <div>
                      <label className="block text-sm text-gray-500 mb-1">State</label>
                      <select
                        value={form.state ?? ''}
                        onChange={(e) => updateField('state', e.target.value)}
                        className="w-full border p-2 rounded-lg text-sm focus:border-black outline-none transition-colors"
                      >
                        <option value="">Select State</option>
                        {STATES.map((s) => (
                          <option key={s} value={s}>{s}</option>
                        ))}
                      </select>
                    </div>
                    <FieldInput label="Postcode" value={form.postcode ?? ''} onChange={(v) => updateField('postcode', v)} />
                    <FieldInput label="Country" value={form.country ?? ''} onChange={(v) => updateField('country', v)} />
                  </>
                )}

                {activeSection === 'payroll' && (
                  <>
                    <div>
                      <label className="block text-sm text-gray-500 mb-1">Unpaid Leave Divisor</label>
                      <input
                        type="number"
                        value={form.unpaid_leave_divisor ?? ''}
                        onChange={(e) => updateField('unpaid_leave_divisor', Number(e.target.value))}
                        className="w-full border p-2 rounded-lg text-sm focus:border-black outline-none transition-colors"
                      />
                      <p className="text-xs text-gray-400 mt-1.5">
                        Number of working days used to calculate daily rate for unpaid leave deductions (typically 26 or 30)
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
                  {mutation.isPending ? 'Saving...' : 'Save'}
                </button>
                <button
                  onClick={closeModal}
                  className="flex-1 border py-2 rounded-xl font-medium text-gray-600 hover:bg-gray-50 transition-colors"
                >
                  Cancel
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

