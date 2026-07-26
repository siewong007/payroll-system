import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Users, Search, Plus, Pencil, Trash2 } from 'lucide-react';
import { listUsers, listCompanies, deleteUser } from '@/api/admin';
import { useAuth } from '@/context/AuthContext';
import { DataTable, type Column } from '@/components/ui/DataTable';
import { Modal } from '@/components/ui/Modal';
import { UserFormModal } from './UserFormModal';
import { ALL_ROLES, roleBadgeClass, roleLabel } from '@/lib/roles';
import { getErrorMessage } from '@/lib/utils';
import type { AppRole, UserWithCompanies } from '@/types';

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
  const queryClient = useQueryClient();
  const { user: currentUser } = useAuth();

  const [search, setSearch] = useState('');
  const [roleFilter, setRoleFilter] = useState('all');
  const [companyFilter, setCompanyFilter] = useState('all');
  const [page, setPage] = useState(1);
  const [showCreate, setShowCreate] = useState(false);
  const [editUser, setEditUser] = useState<UserWithCompanies | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<UserWithCompanies | null>(null);
  const [deleteError, setDeleteError] = useState('');

  const { data: usersPage, isLoading } = useQuery({
    queryKey: ['admin-users', companyFilter, search, page],
    queryFn: () =>
      listUsers({
        companyId: companyFilter === 'all' ? undefined : companyFilter,
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

  const deleteMutation = useMutation({
    mutationFn: (userId: string) => deleteUser(userId),
    onSuccess: () => {
      refreshUsers();
      closeDelete();
    },
    onError: (err: unknown) => setDeleteError(getErrorMessage(err, 'Failed to delete user')),
  });

  // Search, company filter and pagination are resolved by the server; the role
  // filter narrows within the fetched page, which is bounded to PER_PAGE.
  const rows = (usersPage?.data ?? []).filter(
    (u) => roleFilter === 'all' || (u.roles ?? []).includes(roleFilter as AppRole),
  );

  const resetToFirstPage = <T,>(setter: (value: T) => void) => (value: T) => {
    setter(value);
    setPage(1);
  };

  const columns: Column<UserWithCompanies>[] = [
    {
      key: 'full_name',
      header: 'Name',
      primary: true,
      render: (u) => <span className="font-semibold text-gray-900">{u.full_name}</span>,
    },
    { key: 'email', header: 'Email', primary: true, render: (u) => <span className="text-gray-500">{u.email}</span> },
    {
      key: 'roles',
      header: 'Role',
      render: (u) => <div className="flex flex-wrap gap-1.5">{(u.roles ?? []).map(roleBadge)}</div>,
    },
    {
      key: 'companies',
      header: 'Company',
      render: (u) =>
        u.companies.length === 0 ? (
          <span className="text-gray-300">&mdash;</span>
        ) : (
          <div className="flex flex-wrap gap-1">
            {u.companies.map((c) => (
              <span key={c.id} className="text-xs px-2 py-0.5 bg-gray-100 rounded-full text-gray-600">
                {c.name}
              </span>
            ))}
          </div>
        ),
    },
    {
      key: 'is_active',
      header: 'Status',
      align: 'center',
      render: (u) => (
        <span className={`badge ${u.is_active !== false ? 'badge-approved' : 'badge-rejected'}`}>
          {u.is_active !== false ? 'Active' : 'Inactive'}
        </span>
      ),
    },
  ];

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="page-header">
          <h1 className="page-title">Users</h1>
          <p className="page-subtitle">All registered accounts across companies</p>
        </div>
        <button onClick={() => setShowCreate(true)} className="btn-primary w-full sm:w-auto">
          <Plus className="w-4 h-4" aria-hidden="true" /> Add User
        </button>
      </div>

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative flex-1 min-w-[200px] max-w-sm">
          <Search
            className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400"
            aria-hidden="true"
          />
          <input
            type="search"
            aria-label="Search users by name or email"
            value={search}
            onChange={(e) => resetToFirstPage(setSearch)(e.target.value)}
            className="form-input pl-9"
            placeholder="Search by name or email..."
          />
        </div>
        <select
          aria-label="Filter by role"
          value={roleFilter}
          onChange={(e) => setRoleFilter(e.target.value)}
          className="form-input w-auto"
        >
          <option value="all">All Roles</option>
          {ALL_ROLES.map((role) => (
            <option key={role} value={role}>
              {roleLabel(role)}
            </option>
          ))}
        </select>
        {companies && companies.length > 1 && (
          <select
            aria-label="Filter by company"
            value={companyFilter}
            onChange={(e) => resetToFirstPage(setCompanyFilter)(e.target.value)}
            className="form-input w-auto"
          >
            <option value="all">All Companies</option>
            {companies.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        )}
      </div>

      <DataTable
        columns={columns}
        data={rows}
        total={usersPage?.total ?? 0}
        page={page}
        onPageChange={setPage}
        perPage={PER_PAGE}
        isLoading={isLoading}
        emptyMessage="No users found"
        emptyIcon={<Users className="w-10 h-10 opacity-40" aria-hidden="true" />}
        disableRowClick
        renderActions={(u) => (
          <div className="flex items-center justify-center gap-1">
            <button
              onClick={() => setEditUser(u)}
              aria-label={`Edit ${u.full_name}`}
              className="text-sm text-gray-500 hover:text-gray-900 px-2 py-1 rounded hover:bg-gray-100 transition-colors inline-flex items-center gap-1"
            >
              <Pencil className="w-3.5 h-3.5" aria-hidden="true" /> Edit
            </button>
            {/* The backend refuses self-deletion; hiding it keeps that 400 unreachable. */}
            {u.id !== currentUser?.id && (
              <button
                onClick={() => {
                  setDeleteError('');
                  setDeleteTarget(u);
                }}
                aria-label={`Delete ${u.full_name}`}
                className="text-sm text-gray-500 hover:text-red-600 px-2 py-1 rounded hover:bg-red-50 transition-colors inline-flex items-center gap-1"
              >
                <Trash2 className="w-3.5 h-3.5" aria-hidden="true" /> Delete
              </button>
            )}
          </div>
        )}
      />

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
            setEditUser(null);
          }}
        />
      )}

      <Modal
        open={deleteTarget !== null}
        onClose={closeDelete}
        title="Delete User"
        maxWidth="max-w-sm"
        footer={
          <div className="flex justify-end gap-3">
            <button onClick={closeDelete} className="btn-secondary">
              Cancel
            </button>
            <button
              onClick={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
              disabled={deleteMutation.isPending}
              className="px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-xl transition-colors disabled:opacity-50"
            >
              {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
            </button>
          </div>
        }
      >
        {deleteError && (
          <div
            role="alert"
            className="mb-4 p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100"
          >
            {deleteError}
          </div>
        )}
        <p className="text-sm text-gray-500">
          Delete <span className="font-medium text-gray-900">{deleteTarget?.full_name}</span>? Their
          access is revoked immediately and this account will not be restored by an employee backup.
        </p>
      </Modal>
    </div>
  );
}
