import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
import { Building2, Loader2 } from 'lucide-react';
import { createUser, updateUser } from '@/api/admin';
import { Modal } from '@/components/ui/Modal';
import { FieldGroup, FormField } from '@/components/ui/FormField';
import { Skeleton } from '@/components/ui/Skeleton';
import { passwordPolicyHint, validatePassword } from '@/lib/password';
import {
  ALL_ROLES,
  CREATABLE_ROLES,
  isSingleCompanyRoleSet,
  normalizeCompanySelection,
  roleLabel,
  sameIdSet,
  toggleRole,
} from '@/lib/roles';
import { getErrorMessage } from '@/lib/utils';
import type {
  AppRole,
  CompanySummary,
  CreateUserRequest,
  UpdateUserRequest,
  UserWithCompanies,
} from '@/types';

interface UserFormModalProps {
  mode: 'create' | 'edit';
  open: boolean;
  /** Required when `mode === 'edit'`. */
  user?: UserWithCompanies;
  companies: CompanySummary[];
  companiesLoading?: boolean;
  companiesError?: boolean;
  onRetryCompanies?: () => void;
  onClose: () => void;
  onSaved: () => void;
}

interface FormState {
  full_name: string;
  email: string;
  password: string;
  roles: AppRole[];
  is_active: boolean;
  company_ids: string[];
}

function initialState(mode: 'create' | 'edit', user?: UserWithCompanies): FormState {
  if (mode === 'edit' && user) {
    return {
      full_name: user.full_name,
      email: user.email,
      password: '',
      roles: user.roles ?? [],
      is_active: user.is_active !== false,
      company_ids: user.companies.map((company) => company.id),
    };
  }
  return {
    full_name: '',
    email: '',
    password: '',
    roles: ['payroll_admin'],
    is_active: true,
    company_ids: [],
  };
}

/**
 * One form for both creating and editing a user. The two modals this replaces
 * were near-identical copies whose validation had already drifted apart.
 */
export function UserFormModal({
  mode,
  open,
  user,
  companies,
  companiesLoading = false,
  companiesError = false,
  onRetryCompanies,
  onClose,
  onSaved,
}: UserFormModalProps) {
  const { t } = useTranslation();
  const [form, setForm] = useState<FormState>(() => initialState(mode, user));
  const [error, setError] = useState('');

  // Re-seed when the dialog is reopened, or pointed at a different user.
  useEffect(() => {
    if (open) {
      setForm(initialState(mode, user));
      setError('');
    }
  }, [open, mode, user]);

  const isCreate = mode === 'create';
  const hasEmployeeLink = !isCreate && Boolean(user?.employee_id);
  const isSingleCompany = isSingleCompanyRoleSet(form.roles) || hasEmployeeLink;
  const selectableRoles = isCreate || !hasEmployeeLink ? CREATABLE_ROLES : ALL_ROLES;

  const originalCompanyIds = user?.companies.map((company) => company.id) ?? [];
  const companiesChanged = !isCreate && !sameIdSet(form.company_ids, originalCompanyIds);
  const rolesChanged = !isCreate && !sameIdSet(form.roles, user?.roles ?? []);
  const deactivating = !isCreate && !form.is_active && user?.is_active !== false;
  const willRevokeSessions = companiesChanged || rolesChanged || deactivating;

  const mutation = useMutation({
    mutationFn: () => {
      if (isCreate) {
        const payload: CreateUserRequest = {
          email: form.email.trim(),
          password: form.password,
          full_name: form.full_name.trim(),
          roles: form.roles,
          company_ids: form.company_ids,
        };
        return createUser(payload);
      }

      // Send only what changed. In particular, omitting an unchanged
      // `company_ids` is what stops a rename from signing the user out of
      // every device.
      const payload: UpdateUserRequest = {
        full_name: form.full_name.trim(),
        email: form.email.trim(),
        roles: form.roles,
        is_active: form.is_active,
      };
      if (companiesChanged) payload.company_ids = form.company_ids;
      return updateUser(user!.id, payload);
    },
    onSuccess: onSaved,
    onError: (err: unknown) =>
      setError(getErrorMessage(err, isCreate ? t('users.createFailed') : t('users.updateFailed'))),
  });

  const setRoles = (role: AppRole) => {
    setForm((prev) => {
      const roles = toggleRole(prev.roles, role);
      return { ...prev, roles, company_ids: normalizeCompanySelection(roles, prev.company_ids) };
    });
  };

  const toggleCompany = (id: string) => {
    setForm((prev) => {
      if (isSingleCompanyRoleSet(prev.roles)) return { ...prev, company_ids: [id] };
      return {
        ...prev,
        company_ids: prev.company_ids.includes(id)
          ? prev.company_ids.filter((existing) => existing !== id)
          : [...prev.company_ids, id],
      };
    });
  };

  const validate = (): string | null => {
    if (!form.full_name.trim()) return t('users.errors.fullNameRequired');
    if (!form.email.trim()) return t('users.errors.emailRequired');
    if (isCreate) {
      const passwordError = validatePassword(form.password);
      if (passwordError) return passwordError;
    }
    if (form.company_ids.length === 0) return t('users.errors.companyRequired');
    return null;
  };

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }
    setError('');
    mutation.mutate();
  };

  const submitDisabled = mutation.isPending || companies.length === 0;

  return (
    <Modal
      open={open}
      onClose={() => { if (!mutation.isPending) onClose(); }}
      title={isCreate ? t('users.addUser') : t('users.editUser')}
      maxWidth="max-w-lg"
      footer={
        <div className="flex flex-col gap-3">
          {willRevokeSessions && (
            <p className="text-xs text-amber-700 dark:text-amber-400">
              {t('users.revokeSessionsWarning')}
            </p>
          )}
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" disabled={mutation.isPending} onClick={onClose} className="btn-secondary">
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              form="user-form"
              disabled={submitDisabled}
              className="btn-primary inline-flex items-center justify-center gap-2 disabled:opacity-50"
            >
              {mutation.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                  {isCreate ? t('common.creating') : t('common.saving')}
                </>
              ) : isCreate ? (
                t('users.createUser')
              ) : (
                t('common.saveChanges')
              )}
            </button>
          </div>
        </div>
      }
    >
      <form id="user-form" onSubmit={handleSubmit} className="space-y-4" noValidate>
        {error && (
          <div
            role="alert"
            className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/25"
          >
            {error}
          </div>
        )}

        <FormField label={t('users.fullName')} required>
          {(aria) => (
            <input
              {...aria}
              value={form.full_name}
              onChange={(e) => setForm((p) => ({ ...p, full_name: e.target.value }))}
              className="form-input"
              placeholder={t('users.fullNamePlaceholder')}
              autoComplete="name"
            />
          )}
        </FormField>

        <FormField label={t('common.email')} required>
          {(aria) => (
            <input
              {...aria}
              type="email"
              value={form.email}
              onChange={(e) => setForm((p) => ({ ...p, email: e.target.value }))}
              className="form-input"
              placeholder={t('users.emailPlaceholder')}
              autoComplete="email"
            />
          )}
        </FormField>

        {isCreate && (
          <FormField label={t('common.password')} required hint={passwordPolicyHint()}>
            {(aria) => (
              <input
                {...aria}
                type="password"
                value={form.password}
                onChange={(e) => setForm((p) => ({ ...p, password: e.target.value }))}
                className="form-input"
                autoComplete="new-password"
              />
            )}
          </FormField>
        )}

        <FieldGroup legend={t('users.roles')} required>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 mt-2">
            {selectableRoles.map((role) => (
              <label
                key={role}
                className="flex items-center gap-2.5 min-h-11 rounded-lg border border-gray-200 dark:border-slate-700 px-3 py-2 text-sm text-gray-800 dark:text-slate-200 hover:bg-gray-50 dark:hover:bg-slate-800/60 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-[var(--ring)] cursor-pointer transition-colors"
              >
                <input
                  type="checkbox"
                  checked={form.roles.includes(role)}
                  onChange={() => setRoles(role)}
                  className="accent-black dark:accent-[var(--accent-1)]"
                />
                {roleLabel(role)}
              </label>
            ))}
          </div>
        </FieldGroup>

        {!isCreate && (
          <FormField label={t('common.status')}>
            {(aria) => (
              <select
                {...aria}
                value={form.is_active ? 'active' : 'inactive'}
                onChange={(e) => setForm((p) => ({ ...p, is_active: e.target.value === 'active' }))}
                className="form-input"
              >
                <option value="active">{t('common.active')}</option>
                <option value="inactive">{t('common.inactive')}</option>
              </select>
            )}
          </FormField>
        )}

        <FieldGroup
          legend={t('users.assignCompanies')}
          required
          note={hasEmployeeLink ? t('users.employeeCompanyLocked') : isSingleCompany ? t('users.maxOne') : undefined}
        >
          <div className="space-y-2 mt-2 max-h-48 overflow-y-auto">
            {companiesLoading ? (
              <div role="status" className="space-y-2">
                <span className="sr-only">{t('users.loadingCompanies')}</span>
                <Skeleton className="h-11 w-full rounded-lg" />
                <Skeleton className="h-11 w-full rounded-lg" />
              </div>
            ) : companiesError ? (
              <div role="alert" className="text-sm text-red-700 dark:text-red-300 flex items-center justify-between gap-2">
                <span>{t('users.companiesLoadFailed')}</span>
                {onRetryCompanies && (
                  <button type="button" onClick={onRetryCompanies} className="btn-secondary min-h-9 px-3 py-1 text-xs shrink-0">
                    {t('common.retry')}
                  </button>
                )}
              </div>
            ) : companies.length === 0 ? (
              <p className="text-sm text-gray-600 dark:text-slate-400">
                {t('users.noCompanies')}
              </p>
            ) : (
              companies.map((company) => (
                <label
                  key={company.id}
                  className={`flex items-center gap-3 min-h-11 p-3 rounded-lg border has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-[var(--ring)] transition-colors ${
                    hasEmployeeLink ? 'opacity-65 cursor-not-allowed' : 'cursor-pointer'
                  } ${
                    form.company_ids.includes(company.id)
                      ? 'border-black dark:border-[var(--accent-1)] bg-gray-50 dark:bg-slate-800'
                      : 'border-gray-200 dark:border-slate-700 hover:border-gray-300 dark:hover:border-slate-600'
                  }`}
                >
                  <input
                    type={isSingleCompany ? 'radio' : 'checkbox'}
                    name="company"
                    disabled={hasEmployeeLink}
                    checked={form.company_ids.includes(company.id)}
                    onChange={() => {
                      if (!hasEmployeeLink) toggleCompany(company.id);
                    }}
                    className="accent-black dark:accent-[var(--accent-1)]"
                  />
                  <Building2 className="w-4 h-4 text-gray-400 dark:text-slate-500" aria-hidden="true" />
                  <span className="text-sm font-medium text-gray-700 dark:text-slate-200">{company.name}</span>
                </label>
              ))
            )}
          </div>
        </FieldGroup>
      </form>
    </Modal>
  );
}
