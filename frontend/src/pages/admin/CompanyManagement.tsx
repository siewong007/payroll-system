import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Building2, X, Pencil, Trash2 } from 'lucide-react';
import { listCompanies, createCompany, updateCompanyAdmin, deleteCompany } from '@/api/admin';
import { getErrorMessage } from '@/lib/utils';
import type { Company, CreateCompanyRequest, UpdateCompanyRequest } from '@/types';

export function CompanyManagement() {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [editCompany, setEditCompany] = useState<Company | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Company | null>(null);
  const [deleteConfirmName, setDeleteConfirmName] = useState('');
  const [form, setForm] = useState<CreateCompanyRequest>({ name: '' });
  const [error, setError] = useState('');

  const { data: companies, isLoading } = useQuery({
    queryKey: ['admin-companies'],
    queryFn: listCompanies,
  });

  const createMutation = useMutation({
    mutationFn: createCompany,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-companies'] });
      setShowCreate(false);
      setForm({ name: '' });
      setError('');
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, 'Failed to create company'));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteCompany,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-companies'] });
      setDeleteTarget(null);
      setDeleteConfirmName('');
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, 'Failed to delete company'));
    },
  });

  const handleSubmit = () => {
    if (!form.name.trim()) {
      setError('Company name is required');
      return;
    }
    createMutation.mutate(form);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-900" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <h1 className="page-title">Companies</h1>
          <p className="page-subtitle">Manage registered companies</p>
        </div>
        <button onClick={() => setShowCreate(true)} className="btn-primary w-full sm:w-auto">
          <Plus className="w-4 h-4" /> Add Company
        </button>
      </div>

      {/* Companies Table */}
      <div className="card p-0 overflow-hidden">
        {!companies || companies.length === 0 ? (
          <div className="text-center py-16 text-gray-400">
            <Building2 className="w-10 h-10 mx-auto mb-3 opacity-40" />
            <p>No companies registered yet</p>
          </div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>Company Name</th>
                <th>Registration No.</th>
                <th>Tax No.</th>
                <th>Email</th>
                <th>Phone</th>
                <th className="text-center">Status</th>
                <th className="text-center">Actions</th>
              </tr>
            </thead>
            <tbody>
              {companies.map((c) => (
                <tr key={c.id}>
                  <td><span className="font-semibold text-gray-900">{c.name}</span></td>
                  <td className="text-gray-500">{c.registration_number || '—'}</td>
                  <td className="text-gray-500">{c.tax_number || '—'}</td>
                  <td className="text-gray-500">{c.email || '—'}</td>
                  <td className="text-gray-500">{c.phone || '—'}</td>
                  <td className="text-center">
                    <span className={`badge ${c.is_active !== false ? 'badge-approved' : 'badge-rejected'}`}>
                      {c.is_active !== false ? 'Active' : 'Inactive'}
                    </span>
                  </td>
                  <td className="text-center">
                    <button
                      onClick={() => setEditCompany(c)}
                      className="text-sm text-gray-500 hover:text-gray-900 px-2 py-1 rounded hover:bg-gray-100 transition-colors inline-flex items-center gap-1"
                    >
                      <Pencil className="w-3.5 h-3.5" /> Edit
                    </button>
                    <button
                      onClick={() => setDeleteTarget(c)}
                      className="text-sm text-red-500 hover:text-red-700 px-2 py-1 rounded hover:bg-red-50 transition-colors inline-flex items-center gap-1 ml-1"
                    >
                      <Trash2 className="w-3.5 h-3.5" /> Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Create Company Modal */}
      {showCreate && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-lg mx-4">
            <div className="flex items-center justify-between p-6 border-b border-gray-100">
              <h2 className="text-lg font-semibold text-gray-900">Create Company</h2>
              <button onClick={() => { setShowCreate(false); setError(''); }} className="text-gray-400 hover:text-gray-700">
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="p-6 space-y-4">
              {error && <div className="p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100">{error}</div>}
              <div>
                <label className="form-label">Company Name *</label>
                <input
                  value={form.name}
                  onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))}
                  className="form-input"
                  placeholder="e.g., Acme Sdn Bhd"
                />
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="form-label">Registration No. (SSM)</label>
                  <input
                    value={form.registration_number || ''}
                    onChange={(e) => setForm((p) => ({ ...p, registration_number: e.target.value || undefined }))}
                    className="form-input"
                    placeholder="e.g., 202401001234"
                  />
                </div>
                <div>
                  <label className="form-label">Tax No. (LHDN)</label>
                  <input
                    value={form.tax_number || ''}
                    onChange={(e) => setForm((p) => ({ ...p, tax_number: e.target.value || undefined }))}
                    className="form-input"
                    placeholder="e.g., C-1234567890"
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="form-label">Email</label>
                  <input
                    value={form.email || ''}
                    onChange={(e) => setForm((p) => ({ ...p, email: e.target.value || undefined }))}
                    className="form-input"
                    placeholder="company@example.com"
                  />
                </div>
                <div>
                  <label className="form-label">Phone</label>
                  <input
                    value={form.phone || ''}
                    onChange={(e) => setForm((p) => ({ ...p, phone: e.target.value || undefined }))}
                    className="form-input"
                    placeholder="03-12345678"
                  />
                </div>
              </div>
            </div>
            <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
              <button onClick={() => { setShowCreate(false); setError(''); }} className="btn-secondary">Cancel</button>
              <button onClick={handleSubmit} disabled={createMutation.isPending} className="btn-primary">
                {createMutation.isPending ? 'Creating...' : 'Create Company'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Delete Company Confirmation Modal */}
      {deleteTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-md mx-4">
            <div className="flex items-center justify-between p-6 border-b border-gray-100">
              <h2 className="text-lg font-semibold text-red-600">Delete Company</h2>
              <button onClick={() => { setDeleteTarget(null); setDeleteConfirmName(''); }} className="text-gray-400 hover:text-gray-700">
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="p-6 space-y-4">
              <div className="p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100">
                This will permanently delete <strong>{deleteTarget.name}</strong> and all related data including employees, payroll records, documents, leave requests, and claims. This action cannot be undone.
              </div>
              <div>
                <label className="form-label">
                  Type <strong>{deleteTarget.name}</strong> to confirm
                </label>
                <input
                  value={deleteConfirmName}
                  onChange={(e) => setDeleteConfirmName(e.target.value)}
                  className="form-input"
                  placeholder={deleteTarget.name}
                />
              </div>
            </div>
            <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
              <button onClick={() => { setDeleteTarget(null); setDeleteConfirmName(''); }} className="btn-secondary">Cancel</button>
              <button
                onClick={() => deleteMutation.mutate(deleteTarget.id)}
                disabled={deleteConfirmName !== deleteTarget.name || deleteMutation.isPending}
                className="px-4 py-2 text-sm font-medium text-white bg-red-600 rounded-lg hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {deleteMutation.isPending ? 'Deleting...' : 'Delete Company'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Edit Company Modal */}
      {editCompany && (
        <EditCompanyModal
          company={editCompany}
          onClose={() => setEditCompany(null)}
          onUpdated={() => {
            queryClient.invalidateQueries({ queryKey: ['admin-companies'] });
            setEditCompany(null);
          }}
        />
      )}
    </div>
  );
}

/* ─── Edit Company Modal ─── */
function EditCompanyModal({
  company,
  onClose,
  onUpdated,
}: {
  company: Company;
  onClose: () => void;
  onUpdated: () => void;
}) {
  const [form, setForm] = useState<UpdateCompanyRequest>({
    name: company.name,
    registration_number: company.registration_number ?? undefined,
    tax_number: company.tax_number ?? undefined,
    epf_number: company.epf_number ?? undefined,
    socso_code: company.socso_code ?? undefined,
    eis_code: company.eis_code ?? undefined,
    hrdf_number: company.hrdf_number ?? undefined,
    address_line1: company.address_line1 ?? undefined,
    address_line2: company.address_line2 ?? undefined,
    city: company.city ?? undefined,
    state: company.state ?? undefined,
    postcode: company.postcode ?? undefined,
    country: company.country ?? undefined,
    phone: company.phone ?? undefined,
    email: company.email ?? undefined,
  });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: () => updateCompanyAdmin(company.id, form),
    onSuccess: onUpdated,
    onError: (err: unknown) => setError(getErrorMessage(err, 'Failed to update company')),
  });

  const set = (key: keyof UpdateCompanyRequest, value: string) =>
    setForm((p) => ({ ...p, [key]: value || undefined }));

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-6 border-b border-gray-100">
          <h2 className="text-lg font-semibold text-gray-900">Edit Company</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-700">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="p-6 space-y-5">
          {error && <div className="p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100">{error}</div>}

          {/* Basic Info */}
          <div>
            <h3 className="text-sm font-semibold text-gray-900 mb-3">Basic Information</h3>
            <div className="space-y-4">
              <div>
                <label className="form-label">Company Name *</label>
                <input value={form.name || ''} onChange={(e) => set('name', e.target.value)} className="form-input" />
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="form-label">Registration No. (SSM)</label>
                  <input value={form.registration_number || ''} onChange={(e) => set('registration_number', e.target.value)} className="form-input" />
                </div>
                <div>
                  <label className="form-label">Tax No. (LHDN)</label>
                  <input value={form.tax_number || ''} onChange={(e) => set('tax_number', e.target.value)} className="form-input" />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="form-label">Email</label>
                  <input value={form.email || ''} onChange={(e) => set('email', e.target.value)} className="form-input" />
                </div>
                <div>
                  <label className="form-label">Phone</label>
                  <input value={form.phone || ''} onChange={(e) => set('phone', e.target.value)} className="form-input" />
                </div>
              </div>
            </div>
          </div>

          {/* Statutory */}
          <div>
            <h3 className="text-sm font-semibold text-gray-900 mb-3">Statutory Numbers</h3>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="form-label">EPF Number</label>
                <input value={form.epf_number || ''} onChange={(e) => set('epf_number', e.target.value)} className="form-input" />
              </div>
              <div>
                <label className="form-label">SOCSO Code</label>
                <input value={form.socso_code || ''} onChange={(e) => set('socso_code', e.target.value)} className="form-input" />
              </div>
              <div>
                <label className="form-label">EIS Code</label>
                <input value={form.eis_code || ''} onChange={(e) => set('eis_code', e.target.value)} className="form-input" />
              </div>
              <div>
                <label className="form-label">HRDF Number</label>
                <input value={form.hrdf_number || ''} onChange={(e) => set('hrdf_number', e.target.value)} className="form-input" />
              </div>
            </div>
          </div>

          {/* Address */}
          <div>
            <h3 className="text-sm font-semibold text-gray-900 mb-3">Address</h3>
            <div className="space-y-4">
              <div>
                <label className="form-label">Address Line 1</label>
                <input value={form.address_line1 || ''} onChange={(e) => set('address_line1', e.target.value)} className="form-input" />
              </div>
              <div>
                <label className="form-label">Address Line 2</label>
                <input value={form.address_line2 || ''} onChange={(e) => set('address_line2', e.target.value)} className="form-input" />
              </div>
              <div className="grid grid-cols-3 gap-4">
                <div>
                  <label className="form-label">City</label>
                  <input value={form.city || ''} onChange={(e) => set('city', e.target.value)} className="form-input" />
                </div>
                <div>
                  <label className="form-label">State</label>
                  <input value={form.state || ''} onChange={(e) => set('state', e.target.value)} className="form-input" />
                </div>
                <div>
                  <label className="form-label">Postcode</label>
                  <input value={form.postcode || ''} onChange={(e) => set('postcode', e.target.value)} className="form-input" />
                </div>
              </div>
            </div>
          </div>
        </div>
        <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
          <button onClick={onClose} className="btn-secondary">Cancel</button>
          <button onClick={() => mutation.mutate()} disabled={mutation.isPending} className="btn-primary">
            {mutation.isPending ? 'Saving...' : 'Save Changes'}
          </button>
        </div>
      </div>
    </div>
  );
}
