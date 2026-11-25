import { useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle, Plus, Building2, Pencil, Trash2 } from 'lucide-react';
import { listCompanies, createCompany, updateCompanyAdmin, deleteCompany } from '@/api/admin';
import { EmptyState } from '@/components/ui/EmptyState';
import { Modal } from '@/components/ui/Modal';
import { FormField } from '@/components/ui/FormField';
import { PageHeader } from '@/components/ui/PageHeader';
import { useAuth } from '@/context/AuthContext';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';
import type { Company, CreateCompanyRequest, UpdateCompanyRequest } from '@/types';

export function CompanyManagement() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { setSession } = useAuth();
  const toast = useToast();
  const [showCreate, setShowCreate] = useState(false);
  const [editCompany, setEditCompany] = useState<Company | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Company | null>(null);
  const [deleteConfirmName, setDeleteConfirmName] = useState('');
  const [form, setForm] = useState<CreateCompanyRequest>({ name: '' });
  const [error, setError] = useState('');

  const {
    data: companies,
    isLoading,
    isError,
    error: queryError,
    refetch,
  } = useQuery({
    queryKey: ['admin-companies'],
    queryFn: listCompanies,
  });

  const createMutation = useMutation({
    mutationFn: createCompany,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-companies'] });
      queryClient.invalidateQueries({ queryKey: ['my-companies'] });
      toast.success(t('companies.createSuccess'));
      setShowCreate(false);
      setForm({ name: '' });
      setError('');
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('companies.createFailed')));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteCompany,
    onSuccess: (data) => {
      if (data?.token && data?.user) {
        setSession(data.token, data.user);
      }
      queryClient.invalidateQueries({ queryKey: ['admin-companies'] });
      queryClient.invalidateQueries({ queryKey: ['my-companies'] });
      toast.success(t('companies.deleteSuccess'));
      setDeleteTarget(null);
      setDeleteConfirmName('');
      setError('');
    },
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('companies.deleteFailed')));
    },
  });

  const handleSubmit = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (!form.name.trim()) {
      setError(t('companies.nameRequired'));
      return;
    }
    setError('');
    createMutation.mutate(form);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="spinner" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('companies.title')}
        subtitle={t('companies.subtitle')}
        actions={
          <button
            type="button"
            onClick={() => {
              setForm({ name: '' });
              setError('');
              setShowCreate(true);
            }}
            className="btn-primary w-full sm:w-auto"
          >
            <Plus className="w-4 h-4" /> {t('companies.add')}
          </button>
        }
      />

      {/* Companies Table */}
      <div className="card p-0 overflow-hidden">
        {isError ? (
          <EmptyState
            icon={AlertTriangle}
            title={t('companies.loadFailed')}
            description={getErrorMessage(queryError, t('common.error'))}
            action={
              <button type="button" onClick={() => refetch()} className="btn-secondary">
                {t('common.retry')}
              </button>
            }
          />
        ) : !companies || companies.length === 0 ? (
          <EmptyState
            icon={Building2}
            title={t('companies.empty')}
            action={
              <button
                type="button"
                onClick={() => {
                  setForm({ name: '' });
                  setError('');
                  setShowCreate(true);
                }}
                className="btn-primary"
              >
                <Plus className="w-4 h-4" /> {t('companies.add')}
              </button>
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="data-table">
              <thead>
                <tr>
                  <th>{t('companies.name')}</th>
                  <th>{t('companies.regNo')}</th>
                  <th>{t('companies.taxNo')}</th>
                  <th>{t('common.email')}</th>
                  <th>{t('companies.phone')}</th>
                  <th className="text-center">{t('common.status')}</th>
                  <th className="text-center">{t('common.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {companies.map((c) => (
                  <tr key={c.id}>
                    <td><span className="font-semibold text-gray-900 dark:text-slate-100">{c.name}</span></td>
                    <td className="text-gray-500 dark:text-slate-400">{c.registration_number || '—'}</td>
                    <td className="text-gray-500 dark:text-slate-400">{c.tax_number || '—'}</td>
                    <td className="text-gray-500 dark:text-slate-400">{c.email || '—'}</td>
                    <td className="text-gray-500 dark:text-slate-400">{c.phone || '—'}</td>
                    <td className="text-center">
                      <span className={`badge ${c.is_active !== false ? 'badge-approved' : 'badge-rejected'}`}>
                        {c.is_active !== false ? t('common.active') : t('common.inactive')}
                      </span>
                    </td>
                    <td className="text-center whitespace-nowrap">
                      <div className="inline-flex items-center justify-center gap-1">
                        <button
                          type="button"
                          onClick={() => setEditCompany(c)}
                          aria-label={t('common.editName', { name: c.name })}
                          className="text-sm text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-slate-100 min-h-10 min-w-10 sm:min-h-8 sm:min-w-8 px-2.5 py-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors inline-flex items-center gap-1"
                        >
                          <Pencil className="w-3.5 h-3.5" aria-hidden="true" />
                          <span>{t('common.edit')}</span>
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setError('');
                            setDeleteConfirmName('');
                            setDeleteTarget(c);
                          }}
                          aria-label={t('common.deleteName', { name: c.name })}
                          className="text-sm text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 min-h-10 min-w-10 sm:min-h-8 sm:min-w-8 px-2.5 py-1.5 rounded-lg hover:bg-red-50 dark:hover:bg-red-500/10 transition-colors inline-flex items-center gap-1"
                        >
                          <Trash2 className="w-3.5 h-3.5" aria-hidden="true" />
                          <span>{t('common.delete')}</span>
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Create Company Modal */}
      <Modal
        open={showCreate}
        onClose={() => { setShowCreate(false); setForm({ name: '' }); setError(''); }}
        title={t('companies.createTitle')}
        maxWidth="max-w-lg"
        footer={
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" onClick={() => { setShowCreate(false); setForm({ name: '' }); setError(''); }} className="btn-secondary">{t('common.cancel')}</button>
            <button type="submit" form="create-company-form" disabled={createMutation.isPending} className="btn-primary">
              {createMutation.isPending ? t('common.creating') : t('companies.createTitle')}
            </button>
          </div>
        }
      >
        <form id="create-company-form" onSubmit={handleSubmit} className="space-y-4">
          {error && <div role="alert" className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/25">{error}</div>}
          <FormField label={t('companies.name')} required>
            {(aria) => (
              <input
                {...aria}
                value={form.name}
                onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))}
                className="form-input"
                placeholder={t('companies.namePlaceholder')}
              />
            )}
          </FormField>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <FormField label={t('companies.regNoSsm')}>
              {(aria) => (
                <input
                  {...aria}
                  value={form.registration_number || ''}
                  onChange={(e) => setForm((p) => ({ ...p, registration_number: e.target.value || undefined }))}
                  className="form-input"
                  placeholder={t('companies.regNoPlaceholder')}
                />
              )}
            </FormField>
            <FormField label={t('companies.taxNoLhdn')}>
              {(aria) => (
                <input
                  {...aria}
                  value={form.tax_number || ''}
                  onChange={(e) => setForm((p) => ({ ...p, tax_number: e.target.value || undefined }))}
                  className="form-input"
                  placeholder={t('companies.taxNoPlaceholder')}
                />
              )}
            </FormField>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <FormField label={t('common.email')}>
              {(aria) => (
                <input
                  {...aria}
                  value={form.email || ''}
                  onChange={(e) => setForm((p) => ({ ...p, email: e.target.value || undefined }))}
                  className="form-input"
                  placeholder={t('companies.emailPlaceholder')}
                />
              )}
            </FormField>
            <FormField label={t('companies.phone')}>
              {(aria) => (
                <input
                  {...aria}
                  value={form.phone || ''}
                  onChange={(e) => setForm((p) => ({ ...p, phone: e.target.value || undefined }))}
                  className="form-input"
                  placeholder={t('companies.phonePlaceholder')}
                />
              )}
            </FormField>
          </div>
        </form>
      </Modal>

      {/* Delete Company Confirmation Modal */}
      <Modal
        open={deleteTarget !== null}
        onClose={() => { setDeleteTarget(null); setDeleteConfirmName(''); setError(''); }}
        title={t('companies.deleteTitle')}
        maxWidth="max-w-md"
        footer={
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" onClick={() => { setDeleteTarget(null); setDeleteConfirmName(''); setError(''); }} className="btn-secondary">{t('common.cancel')}</button>
            <button
              type="button"
              onClick={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
              disabled={!deleteTarget || deleteConfirmName !== deleteTarget.name || deleteMutation.isPending}
              className="px-4 py-2 min-h-[44px] text-sm font-semibold text-white bg-red-600 rounded-xl hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {deleteMutation.isPending ? t('common.deleting') : t('companies.deleteTitle')}
            </button>
          </div>
        }
      >
        {deleteTarget && (
          <div className="space-y-4">
            {error && (
              <div role="alert" className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/25">
                {error}
              </div>
            )}
            <div className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/25">
              <Trans i18nKey="companies.deleteWarning" values={{ name: deleteTarget.name }} components={{ b: <strong /> }} />
            </div>
            <div>
              <label htmlFor="delete-company-confirm" className="form-label">
                <Trans i18nKey="companies.deleteConfirmLabel" values={{ name: deleteTarget.name }} components={{ b: <strong /> }} />
              </label>
              <input
                id="delete-company-confirm"
                value={deleteConfirmName}
                onChange={(e) => setDeleteConfirmName(e.target.value)}
                className="form-input"
                placeholder={deleteTarget.name}
              />
            </div>
          </div>
        )}
      </Modal>

      {/* Edit Company Modal */}
      {editCompany && (
        <EditCompanyModal
          company={editCompany}
          onClose={() => setEditCompany(null)}
          onUpdated={() => {
            queryClient.invalidateQueries({ queryKey: ['admin-companies'] });
            queryClient.invalidateQueries({ queryKey: ['my-companies'] });
            toast.success(t('companies.updateSuccess'));
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
    registration_number: company.registration_number ?? '',
    tax_number: company.tax_number ?? '',
    epf_number: company.epf_number ?? '',
    socso_code: company.socso_code ?? '',
    eis_code: company.eis_code ?? '',
    hrdf_number: company.hrdf_number ?? '',
    address_line1: company.address_line1 ?? '',
    address_line2: company.address_line2 ?? '',
    city: company.city ?? '',
    state: company.state ?? '',
    postcode: company.postcode ?? '',
    country: company.country ?? '',
    phone: company.phone ?? '',
    email: company.email ?? '',
  });
  const { t } = useTranslation();
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: () => updateCompanyAdmin(company.id, form),
    onSuccess: onUpdated,
    onError: (err: unknown) => setError(getErrorMessage(err, t('companies.updateFailed'))),
  });

  const set = (key: keyof UpdateCompanyRequest, value: string) =>
    setForm((p) => ({ ...p, [key]: value }));

  return (
    <Modal
      open
      onClose={onClose}
      title={t('companies.editTitle')}
      maxWidth="max-w-2xl"
      footer={
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <button type="button" onClick={onClose} className="btn-secondary">{t('common.cancel')}</button>
          <button type="submit" form="edit-company-form" disabled={mutation.isPending} className="btn-primary">
            {mutation.isPending ? t('common.saving') : t('common.saveChanges')}
          </button>
        </div>
      }
    >
      <form
        id="edit-company-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!form.name?.trim()) {
            setError(t('companies.nameRequired'));
            return;
          }
          setError('');
          mutation.mutate();
        }}
        className="space-y-5"
      >
        {error && <div role="alert" className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/25">{error}</div>}

        {/* Basic Info */}
        <div>
          <h3 className="text-sm font-semibold text-gray-900 dark:text-slate-100 mb-3">{t('companies.basicInfo')}</h3>
          <div className="space-y-4">
            <FormField label={t('companies.name')} required>
              {(aria) => <input {...aria} value={form.name || ''} onChange={(e) => set('name', e.target.value)} className="form-input" />}
            </FormField>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <FormField label={t('companies.regNoSsm')}>
                {(aria) => <input {...aria} value={form.registration_number || ''} onChange={(e) => set('registration_number', e.target.value)} className="form-input" />}
              </FormField>
              <FormField label={t('companies.taxNoLhdn')}>
                {(aria) => <input {...aria} value={form.tax_number || ''} onChange={(e) => set('tax_number', e.target.value)} className="form-input" />}
              </FormField>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <FormField label={t('common.email')}>
                {(aria) => <input {...aria} value={form.email || ''} onChange={(e) => set('email', e.target.value)} className="form-input" />}
              </FormField>
              <FormField label={t('companies.phone')}>
                {(aria) => <input {...aria} value={form.phone || ''} onChange={(e) => set('phone', e.target.value)} className="form-input" />}
              </FormField>
            </div>
          </div>
        </div>

        {/* Statutory */}
        <div>
          <h3 className="text-sm font-semibold text-gray-900 dark:text-slate-100 mb-3">{t('companies.statutoryNumbers')}</h3>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <FormField label={t('employees.fields.epfNumber')}>
              {(aria) => <input {...aria} value={form.epf_number || ''} onChange={(e) => set('epf_number', e.target.value)} className="form-input" />}
            </FormField>
            <FormField label={t('companies.socsoCode')}>
              {(aria) => <input {...aria} value={form.socso_code || ''} onChange={(e) => set('socso_code', e.target.value)} className="form-input" />}
            </FormField>
            <FormField label={t('companies.eisCode')}>
              {(aria) => <input {...aria} value={form.eis_code || ''} onChange={(e) => set('eis_code', e.target.value)} className="form-input" />}
            </FormField>
            <FormField label={t('companies.hrdfNumber')}>
              {(aria) => <input {...aria} value={form.hrdf_number || ''} onChange={(e) => set('hrdf_number', e.target.value)} className="form-input" />}
            </FormField>
          </div>
        </div>

        {/* Address */}
        <div>
          <h3 className="text-sm font-semibold text-gray-900 dark:text-slate-100 mb-3">{t('portal.profile.address')}</h3>
          <div className="space-y-4">
            <FormField label={t('employees.form.addressLine1')}>
              {(aria) => <input {...aria} value={form.address_line1 || ''} onChange={(e) => set('address_line1', e.target.value)} className="form-input" />}
            </FormField>
            <FormField label={t('employees.form.addressLine2')}>
              {(aria) => <input {...aria} value={form.address_line2 || ''} onChange={(e) => set('address_line2', e.target.value)} className="form-input" />}
            </FormField>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <FormField label={t('employees.form.city')}>
                {(aria) => <input {...aria} value={form.city || ''} onChange={(e) => set('city', e.target.value)} className="form-input" />}
              </FormField>
              <FormField label={t('employees.form.state')}>
                {(aria) => <input {...aria} value={form.state || ''} onChange={(e) => set('state', e.target.value)} className="form-input" />}
              </FormField>
              <FormField label={t('employees.form.postcode')}>
                {(aria) => <input {...aria} value={form.postcode || ''} onChange={(e) => set('postcode', e.target.value)} className="form-input" />}
              </FormField>
            </div>
          </div>
        </div>
      </form>
    </Modal>
  );
}
