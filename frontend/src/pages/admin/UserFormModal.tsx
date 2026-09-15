import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
import { Building2 } from 'lucide-react';
import { createUser, updateUser } from '@/api/admin';
import { Modal } from '@/components/ui/Modal';
import { FieldGroup, FormField } from '@/components/ui/FormField';
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
  const isSingleCompany = isSingleCompanyRoleSet(form.roles);
  const selectableRoles = isCreate ? CREATABLE_ROLES : ALL_ROLES;

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
      onClose={onClose}
      title={isCreate ? t('users.addUser') : t('users.editUser')}
      maxWidth="max-w-lg"
      footer={
        <div className="flex flex-col gap-3">
          {willRevokeSessions && (
            <p className="text-xs text-amber-700">
              {t('users.revokeSessionsWarning')}
            </p>
          )}
          <div className="flex justify-end gap-3">
            <button type="button" onClick={onClose} className="btn-secondary">
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              form="user-form"
              disabled={submitDisabled}
              className="btn-primary disabled:opacity-50"
            >
              {mutation.isPending
                ? isCreate
                  ? t('common.creating')
                  : t('common.saving')
                : isCreate
                  ? t('users.createUser')
                  : t('common.saveChanges')}
            </button>
          </div>
        </div>
      }
    >
      <form id="user-form" onSubmit={handleSubmit} className="space-y-4" noValidate>
        {error && (
          <div
            role="alert"
            className="p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100"
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
          <div className="grid grid-cols-2 gap-2 mt-2">
            {selectableRoles.map((role) => (
              <label
                key={role}
                className="flex items-center gap-2 rounded-lg border border-gray-200 px-3 py-2 text-sm"
              >
                <input
                  type="checkbox"
                  checked={form.roles.includes(role)}
                  onChange={() => setRoles(role)}
                  className="accent-black"
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
          note={isSingleCompany ? t('users.maxOne') : undefined}
        >
          <div className="space-y-2 mt-2 max-h-48 overflow-y-auto">
            {companiesLoading ? (
              <p className="text-sm text-gray-400">{t('users.loadingCompanies')}</p>
            ) : companiesError ? (
              <div className="text-sm text-red-600">
                {t('users.companiesLoadFailed')}{' '}
                {onRetryCompanies && (
                  <button type="button" onClick={onRetryCompanies} className="underline">
                    {t('common.retry')}
                  </button>
                )}
              </div>
            ) : companies.length === 0 ? (
              <p className="text-sm text-gray-400">
                {t('users.noCompanies')}
              </p>
            ) : (
              companies.map((company) => (
                <label
                  key={company.id}
                  className={`flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${
                    form.company_ids.includes(company.id)
                      ? 'border-black bg-gray-50'
                      : 'border-gray-200 hover:border-gray-300'
                  }`}
                >
                  <input
                    type={isSingleCompany ? 'radio' : 'checkbox'}
                    name="company"
                    checked={form.company_ids.includes(company.id)}
                    onChange={() => toggleCompany(company.id)}
                    className="accent-black"
                  />
                  <Building2 className="w-4 h-4 text-gray-400" aria-hidden="true" />
                  <span className="text-sm font-medium text-gray-700">{company.name}</span>
                </label>
              ))
            )}
          </div>
        </FieldGroup>
      </form>
    </Modal>
  );
}
