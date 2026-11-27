import { useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle, KeyRound, Loader2, Users, Search, Plus, Pencil, Trash2 } from 'lucide-react';
import { listUsers, listCompanies, deleteUser, resetUser2fa } from '@/api/admin';
import { useAuth } from '@/context/AuthContext';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { StepUpConfirmModal } from '@/components/ui/StepUpConfirmModal';
import { UserFormModal } from './UserFormModal';
import {
  ALL_ROLES,
  PRIVILEGED_USER_ROLES,
  hasAnyRole,
  hidesPrivilegedUsers,
  roleBadgeClass,
  roleLabel,
} from '@/lib/roles';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';
import type { StepUpCredentials, UserWithCompanies } from '@/types';

const PER_PAGE = 20;

const roleBadge = (role: string) => (
  <span
    key={role}
    className={`text-xs px-2.5 py-1 rounded-full font-medium ${roleBadgeClass(role)}`}
  >
    {roleLabel(role)}
  </span>
);

export function UserManagement() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();
  const { user: currentUser } = useAuth();
  const isSuperAdmin = hasAnyRole(currentUser, ['super_admin']);
  const actorTwoFactorEnabled = Boolean(currentUser?.two_factor_enabled);

  const [search, setSearch] = useState('');
  const [roleFilter, setRoleFilter] = useState('all');
  const [companyFilter, setCompanyFilter] = useState('all');
  const [page, setPage] = useState(1);
  const [showCreate, setShowCreate] = useState(false);
  const [editUser, setEditUser] = useState<UserWithCompanies | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<UserWithCompanies | null>(null);
  const [deleteError, setDeleteError] = useState('');
  const [reset2faTarget, setReset2faTarget] = useState<UserWithCompanies | null>(null);
  const [reset2faPassword, setReset2faPassword] = useState('');
  const [reset2faTotpCode, setReset2faTotpCode] = useState('');
  const [reset2faError, setReset2faError] = useState('');

  const {
    data: usersPage,
    isLoading,
    isError,
    error: queryError,
    refetch,
  } = useQuery({
    queryKey: ['admin-users', companyFilter, roleFilter, search, page],
    queryFn: () =>
      listUsers({
        companyId: companyFilter === 'all' ? undefined : companyFilter,
        role: roleFilter === 'all' ? undefined : roleFilter,
        search: search || undefined,
        page,
        perPage: PER_PAGE,
      }),
  });

  const {
    data: companies,
    isLoading: companiesLoading,
    isError: companiesError,
    refetch: refetchCompanies,
  } = useQuery({
    queryKey: ['admin-companies'],
    queryFn: listCompanies,
  });

  const refreshUsers = () => queryClient.invalidateQueries({ queryKey: ['admin-users'] });

  const closeDelete = () => {
    setDeleteTarget(null);
    setDeleteError('');
  };

  const closeReset2fa = () => {
    setReset2faTarget(null);
    setReset2faPassword('');
    setReset2faTotpCode('');
    setReset2faError('');
  };

  const deleteMutation = useMutation({
    mutationFn: ({ userId, stepUp }: { userId: string; stepUp: StepUpCredentials }) =>
      deleteUser(userId, stepUp),
    onSuccess: () => {
      refreshUsers();
      toast.success(t('users.deleteSuccess'));
      closeDelete();
    },
    onError: (err: unknown) => setDeleteError(getErrorMessage(err, t('users.deleteFailed'))),
  });

  const reset2faMutation = useMutation({
    mutationFn: ({ userId, password, totpCode }: { userId: string; password: string; totpCode?: string }) =>
      resetUser2fa(userId, password, ...(totpCode ? [totpCode] : [])),
    onSuccess: () => {
      toast.success(t('users.reset2faSuccess'));
      closeReset2fa();
    },
    onError: (err: unknown) => setReset2faError(getErrorMessage(err, t('users.reset2faFailed'))),
  });

  const rows = usersPage?.data ?? [];

  const resetToFirstPage = <T,>(setter: (value: T) => void) => (value: T) => {
    setter(value);
    setPage(1);
  };

  const columns: Column<UserWithCompanies>[] = [
    {
      key: 'full_name',
      header: t('common.name'),
      primary: true,
      render: (u) => <span className="font-semibold text-gray-900 dark:text-slate-100">{u.full_name}</span>,
    },
    { key: 'email', header: t('common.email'), primary: true, render: (u) => <span className="text-gray-600 dark:text-slate-300 break-all">{u.email}</span> },
    {
      key: 'roles',
      header: t('users.role'),
      render: (u) => <div className="flex flex-wrap gap-1.5">{(u.roles ?? []).map(roleBadge)}</div>,
    },
    {
      key: 'companies',
      header: t('common.company'),
      render: (u) =>
        u.companies.length === 0 ? (
          <span className="text-gray-400 dark:text-slate-500">&mdash;</span>
        ) : (
          <div className="flex flex-wrap gap-1">
            {u.companies.map((c) => (
              <span key={c.id} className="text-xs px-2 py-0.5 bg-gray-100 dark:bg-slate-800 rounded-full text-gray-700 dark:text-slate-300">
                {c.name}
              </span>
            ))}
          </div>
        ),
    },
    {
      key: 'is_active',
      header: t('common.status'),
      align: 'center',
      render: (u) => (
        <span className={`badge ${u.is_active !== false ? 'badge-approved' : 'badge-rejected'}`}>
          {u.is_active !== false ? t('common.active') : t('common.inactive')}
        </span>
      ),
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('users.title')}
        subtitle={t('users.subtitle')}
        actions={
          <button type="button" onClick={() => setShowCreate(true)} className="btn-primary w-full sm:w-auto">
            <Plus className="w-4 h-4" aria-hidden="true" /> {t('users.addUser')}
          </button>
        }
      />

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative w-full sm:flex-1 sm:min-w-[200px] sm:max-w-sm">
          <Search
            className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 dark:text-slate-500"
            aria-hidden="true"
          />
          <input
            type="search"
            aria-label={t('users.searchLabel')}
            value={search}
            onChange={(e) => resetToFirstPage(setSearch)(e.target.value)}
            className="form-input pl-9"
            placeholder={t('users.searchPlaceholder')}
          />
        </div>
        <select
          aria-label={t('users.filterByRole')}
          value={roleFilter}
          onChange={(e) => resetToFirstPage(setRoleFilter)(e.target.value)}
          className="form-input w-full sm:w-auto min-h-11 sm:min-h-10"
        >
          <option value="all">{t('users.allRoles')}</option>
          {ALL_ROLES.filter(
            (role) => !hidesPrivilegedUsers(currentUser) || !PRIVILEGED_USER_ROLES.includes(role),
          ).map((role) => (
            <option key={role} value={role}>
              {roleLabel(role)}
            </option>
          ))}
        </select>
        {companies && companies.length > 1 && (
          <select
            aria-label={t('users.filterByCompany')}
            value={companyFilter}
            onChange={(e) => resetToFirstPage(setCompanyFilter)(e.target.value)}
            className="form-input w-full sm:w-auto min-h-11 sm:min-h-10"
          >
            <option value="all">{t('users.allCompanies')}</option>
            {companies.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        )}
      </div>

      {isError ? (
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('users.loadFailed')}
            description={getErrorMessage(queryError, t('common.error'))}
            action={
              <button type="button" onClick={() => refetch()} className="btn-secondary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      ) : (
        <DataTable
          columns={columns}
          data={rows}
          total={usersPage?.total ?? 0}
          page={page}
          onPageChange={setPage}
          perPage={PER_PAGE}
          isLoading={isLoading}
          emptyMessage={t('users.empty')}
          emptyIcon={<Users className="w-10 h-10 opacity-40" aria-hidden="true" />}
          disableRowClick
          renderActions={(u) => (
            <div className="flex flex-wrap items-center justify-center gap-1">
              <button
                type="button"
                onClick={() => setEditUser(u)}
                aria-label={t('common.editName', { name: u.full_name })}
                className="text-sm text-gray-600 dark:text-slate-300 hover:text-gray-900 dark:hover:text-slate-100 min-h-11 sm:min-h-8 px-2.5 py-1 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors inline-flex items-center gap-1"
              >
                <Pencil className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.edit')}
              </button>
              {isSuperAdmin && u.id !== currentUser?.id && (
                <button
                  type="button"
                  onClick={() => {
                    setReset2faTarget(u);
                    setReset2faPassword('');
                    setReset2faError('');
                  }}
                  aria-label={t('users.reset2faFor', { name: u.full_name })}
                  className="text-sm text-amber-700 dark:text-amber-400 hover:text-amber-800 dark:hover:text-amber-300 min-h-11 sm:min-h-8 px-2.5 py-1 rounded-lg hover:bg-amber-50 dark:hover:bg-amber-500/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors inline-flex items-center gap-1"
                >
                  <KeyRound className="w-3.5 h-3.5" aria-hidden="true" /> {t('users.reset2fa')}
                </button>
              )}
              {/* The backend refuses self-deletion; hiding it keeps that 400 unreachable. */}
              {u.id !== currentUser?.id && (
                <button
                  type="button"
                  onClick={() => {
                    setDeleteError('');
                    setDeleteTarget(u);
                  }}
                  aria-label={t('common.deleteName', { name: u.full_name })}
                  className="text-sm text-gray-600 dark:text-slate-300 hover:text-red-700 dark:hover:text-red-400 min-h-11 sm:min-h-8 px-2.5 py-1 rounded-lg hover:bg-red-50 dark:hover:bg-red-500/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors inline-flex items-center gap-1"
                >
                  <Trash2 className="w-3.5 h-3.5" aria-hidden="true" /> {t('common.delete')}
                </button>
              )}
            </div>
          )}
        />
      )}

      <UserFormModal
        mode="create"
        open={showCreate}
        companies={companies ?? []}
        companiesLoading={companiesLoading}
        companiesError={companiesError}
        onRetryCompanies={() => void refetchCompanies()}
        onClose={() => setShowCreate(false)}
        onSaved={() => {
          refreshUsers();
          toast.success(t('users.createSuccess'));
          setShowCreate(false);
        }}
      />

      {editUser && (
        <UserFormModal
          mode="edit"
          open
          user={editUser}
          companies={companies ?? []}
          companiesLoading={companiesLoading}
          companiesError={companiesError}
          onRetryCompanies={() => void refetchCompanies()}
          onClose={() => setEditUser(null)}
          onSaved={() => {
            refreshUsers();
            toast.success(t('users.updateSuccess'));
            setEditUser(null);
          }}
        />
      )}

      <Modal
        open={reset2faTarget !== null}
        onClose={() => { if (!reset2faMutation.isPending) closeReset2fa(); }}
        title={t('users.reset2faTitle')}
        maxWidth="max-w-md"
        footer={
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button type="button" disabled={reset2faMutation.isPending} onClick={closeReset2fa} className="btn-secondary">
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              form="reset-2fa-form"
              disabled={
                !reset2faPassword ||
                (actorTwoFactorEnabled && !reset2faTotpCode.trim()) ||
                reset2faMutation.isPending
              }
              className="inline-flex items-center justify-center gap-2 px-4 py-2 min-h-[44px] text-sm font-medium text-white bg-amber-600 hover:bg-amber-700 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 transition-colors disabled:opacity-50"
            >
              {reset2faMutation.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                  {t('common.saving')}
                </>
              ) : (
                t('users.reset2fa')
              )}
            </button>
          </div>
        }
      >
        {reset2faTarget && (
          <form
            id="reset-2fa-form"
            onSubmit={(e) => {
              e.preventDefault();
              if (!reset2faPassword || (actorTwoFactorEnabled && !reset2faTotpCode.trim())) return;
              setReset2faError('');
              reset2faMutation.mutate({
                userId: reset2faTarget.id,
                password: reset2faPassword,
                ...(actorTwoFactorEnabled && reset2faTotpCode.trim()
                  ? { totpCode: reset2faTotpCode.trim() }
                  : {}),
              });
            }}
            className="space-y-4"
          >
            {reset2faError && (
              <div
                role="alert"
                className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/25"
              >
                {reset2faError}
              </div>
            )}
            <p className="text-sm text-gray-600 dark:text-slate-300">
              <Trans
                i18nKey="users.reset2faDescription"
                values={{ name: reset2faTarget.full_name }}
                components={{ b: <span className="font-medium text-gray-900 dark:text-slate-100" /> }}
              />
            </p>
            <FormField label={t('users.confirmWithYourPassword')} required>
              {(aria) => (
                <input
                  {...aria}
                  type="password"
                  value={reset2faPassword}
                  onChange={(e) => setReset2faPassword(e.target.value)}
                  className="form-input"
                  autoComplete="current-password"
                />
              )}
            </FormField>
            {actorTwoFactorEnabled && (
              <FormField label={t('auth.twoFactor.title')} required>
                {(aria) => (
                  <input
                    {...aria}
                    type="text"
                    inputMode="text"
                    autoComplete="one-time-code"
                    value={reset2faTotpCode}
                    onChange={(e) => setReset2faTotpCode(e.target.value)}
                    placeholder="123456"
                    className="form-input text-center tracking-widest"
                  />
                )}
              </FormField>
            )}
          </form>
        )}
      </Modal>

      <StepUpConfirmModal
        open={deleteTarget !== null}
        onClose={closeDelete}
        title={t('users.deleteTitle')}
        description={
          <Trans
            i18nKey="users.deleteBody"
            values={{ name: deleteTarget?.full_name }}
            components={{ b: <span className="font-medium text-gray-900 dark:text-slate-100" /> }}
          />
        }
        confirmLabel={t('common.delete')}
        pendingLabel={t('common.deleting')}
        variant="danger"
        pending={deleteMutation.isPending}
        error={deleteError}
        onConfirm={(stepUp) => {
          if (deleteTarget) {
            setDeleteError('');
            deleteMutation.mutate({ userId: deleteTarget.id, stepUp });
          }
        }}
      />
    </div>
  );
}
