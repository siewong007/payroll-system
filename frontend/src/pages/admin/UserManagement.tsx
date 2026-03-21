import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Users, Search, X, Building2, Plus, Pencil, Trash2 } from 'lucide-react';
import { listUsers, listCompanies, createUser, updateUser, deleteUser } from '@/api/admin';
import type { CreateUserRequest, UpdateUserRequest, UserWithCompanies } from '@/types';

const ALL_ROLES = [
  { value: 'super_admin', label: 'Super Admin' },
  { value: 'admin', label: 'Admin' },
  { value: 'payroll_admin', label: 'Payroll Admin' },
  { value: 'hr_manager', label: 'HR Manager' },
  { value: 'finance', label: 'Finance' },
  { value: 'exec', label: 'Executive' },
  { value: 'employee', label: 'Employee' },
] as const;

const roleBadge = (role: string) => {
  const cls: Record<string, string> = {
    super_admin: 'bg-purple-100 text-purple-700',
    admin: 'bg-indigo-100 text-indigo-700',
    payroll_admin: 'bg-blue-100 text-blue-700',
    hr_manager: 'bg-green-100 text-green-700',
    finance: 'bg-amber-100 text-amber-700',
    exec: 'bg-gray-100 text-gray-700',
    employee: 'bg-sky-100 text-sky-700',
  };
  const label: Record<string, string> = Object.fromEntries(ALL_ROLES.map((r) => [r.value, r.label]));
  return (
    <span className={`text-xs px-2.5 py-1 rounded-full font-medium ${cls[role] || 'bg-gray-100 text-gray-600'}`}>
      {label[role] || role}
    </span>
  );
};

export function UserManagement() {
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [roleFilter, setRoleFilter] = useState('all');
  const [companyFilter, setCompanyFilter] = useState('all');
  const [showCreate, setShowCreate] = useState(false);
  const [editUser, setEditUser] = useState<UserWithCompanies | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<UserWithCompanies | null>(null);

  const { data: users, isLoading } = useQuery({
    queryKey: ['admin-users'],
    queryFn: listUsers,
  });

  const { data: companies } = useQuery({
    queryKey: ['admin-companies'],
    queryFn: listCompanies,
  });

  const deleteMutation = useMutation({
    mutationFn: (userId: string) => deleteUser(userId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-users'] });
      setDeleteTarget(null);
    },
  });

  const filtered = (users ?? []).filter((u) => {
    if (search) {
      const q = search.toLowerCase();
      if (!u.full_name.toLowerCase().includes(q) && !u.email.toLowerCase().includes(q)) return false;
    }
    if (roleFilter !== 'all' && u.role !== roleFilter) return false;
    if (companyFilter !== 'all') {
      if (!u.companies.some((c) => c.id === companyFilter)) return false;
    }
    return true;
  });

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
          <h1 className="page-title">Users</h1>
          <p className="page-subtitle">All registered accounts across companies</p>
        </div>
        <button onClick={() => setShowCreate(true)} className="btn-primary w-full sm:w-auto">
          <Plus className="w-4 h-4" /> Add User
        </button>
      </div>

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative flex-1 min-w-[200px] max-w-sm">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="form-input pl-9"
            placeholder="Search by name or email..."
          />
        </div>
        <select value={roleFilter} onChange={(e) => setRoleFilter(e.target.value)} className="form-input w-auto">
          <option value="all">All Roles</option>
          {ALL_ROLES.map((r) => (
            <option key={r.value} value={r.value}>{r.label}</option>
          ))}
        </select>
        {companies && companies.length > 1 && (
          <select value={companyFilter} onChange={(e) => setCompanyFilter(e.target.value)} className="form-input w-auto">
            <option value="all">All Companies</option>
            {companies.map((c) => (
              <option key={c.id} value={c.id}>{c.name}</option>
            ))}
          </select>
        )}
        <span className="text-sm text-gray-400">{filtered.length} users</span>
      </div>

      {/* Users Table */}
      <div className="card p-0 overflow-hidden">
        {filtered.length === 0 ? (
          <div className="text-center py-16 text-gray-400">
            <Users className="w-10 h-10 mx-auto mb-3 opacity-40" />
            <p>No users found</p>
          </div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Email</th>
                <th>Role</th>
                <th>Company</th>
                <th className="text-center">Status</th>
                <th className="text-center">Actions</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((u) => (
                <tr key={u.id}>
                  <td><span className="font-semibold text-gray-900">{u.full_name}</span></td>
                  <td className="text-gray-500">{u.email}</td>
                  <td>{roleBadge(u.role)}</td>
                  <td>
                    <div className="flex flex-wrap gap-1">
                      {u.companies.length === 0 ? (
                        <span className="text-gray-300">&mdash;</span>
                      ) : (
                        u.companies.map((c) => (
                          <span key={c.id} className="text-xs px-2 py-0.5 bg-gray-100 rounded-full text-gray-600">
                            {c.name}
                          </span>
                        ))
                      )}
                    </div>
                  </td>
                  <td className="text-center">
                    <span className={`badge ${u.is_active !== false ? 'badge-approved' : 'badge-rejected'}`}>
                      {u.is_active !== false ? 'Active' : 'Inactive'}
                    </span>
                  </td>
                  <td className="text-center">
                    <div className="flex items-center justify-center gap-1">
                      <button
                        onClick={() => setEditUser(u)}
                        className="text-sm text-gray-500 hover:text-gray-900 px-2 py-1 rounded hover:bg-gray-100 transition-colors inline-flex items-center gap-1"
                      >
                        <Pencil className="w-3.5 h-3.5" /> Edit
                      </button>
                      <button
                        onClick={() => setDeleteTarget(u)}
                        className="text-sm text-gray-500 hover:text-red-600 px-2 py-1 rounded hover:bg-red-50 transition-colors inline-flex items-center gap-1"
                      >
                        <Trash2 className="w-3.5 h-3.5" /> Delete
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Create User Modal */}
      {showCreate && (
        <CreateUserModal
          companies={companies ?? []}
          onClose={() => setShowCreate(false)}
          onCreated={() => {
            queryClient.invalidateQueries({ queryKey: ['admin-users'] });
            setShowCreate(false);
          }}
        />
      )}

      {/* Edit User Modal */}
      {editUser && (
        <EditUserModal
          user={editUser}
          companies={companies ?? []}
          onClose={() => setEditUser(null)}
          onUpdated={() => {
            queryClient.invalidateQueries({ queryKey: ['admin-users'] });
            setEditUser(null);
          }}
        />
      )}

      {/* Delete Confirmation */}
      {deleteTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
            <h2 className="text-lg font-semibold text-gray-900 mb-2">Delete User</h2>
            <p className="text-sm text-gray-500 mb-5">
              Are you sure you want to delete <span className="font-medium text-gray-900">{deleteTarget.full_name}</span>? This action cannot be undone.
            </p>
            <div className="flex justify-end gap-3">
              <button onClick={() => setDeleteTarget(null)} className="btn-secondary">Cancel</button>
              <button
                onClick={() => deleteMutation.mutate(deleteTarget.id)}
                disabled={deleteMutation.isPending}
                className="px-4 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-xl transition-colors disabled:opacity-50"
              >
                {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/* ─── Create User Modal ─── */
function CreateUserModal({
  companies,
  onClose,
  onCreated,
}: {
  companies: { id: string; name: string }[];
  onClose: () => void;
  onCreated: () => void;
}) {
  const [form, setForm] = useState<CreateUserRequest>({
    email: '',
    password: '',
    full_name: '',
    role: 'payroll_admin',
    company_ids: [],
  });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: createUser,
    onSuccess: onCreated,
    onError: (err: any) => setError(err.response?.data?.error || 'Failed to create user'),
  });

  const isSingleCompany = form.role === 'exec' || form.role === 'employee';

  const toggleCompany = (id: string) => {
    if (isSingleCompany) {
      setForm((p) => ({ ...p, company_ids: [id] }));
    } else {
      setForm((p) => ({
        ...p,
        company_ids: p.company_ids.includes(id)
          ? p.company_ids.filter((c) => c !== id)
          : [...p.company_ids, id],
      }));
    }
  };

  const handleSubmit = () => {
    if (!form.email || !form.password || !form.full_name) {
      setError('All fields are required');
      return;
    }
    if (form.company_ids.length === 0) {
      setError('Select at least one company');
      return;
    }
    mutation.mutate(form);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-lg mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-6 border-b border-gray-100">
          <h2 className="text-lg font-semibold text-gray-900">Add User</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-700">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="p-6 space-y-4">
          {error && <div className="p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100">{error}</div>}
          <div>
            <label className="form-label">Full Name *</label>
            <input
              value={form.full_name}
              onChange={(e) => setForm((p) => ({ ...p, full_name: e.target.value }))}
              className="form-input"
              placeholder="John Doe"
            />
          </div>
          <div>
            <label className="form-label">Email *</label>
            <input
              type="email"
              value={form.email}
              onChange={(e) => setForm((p) => ({ ...p, email: e.target.value }))}
              className="form-input"
              placeholder="john@example.com"
            />
          </div>
          <div>
            <label className="form-label">Password *</label>
            <input
              type="password"
              value={form.password}
              onChange={(e) => setForm((p) => ({ ...p, password: e.target.value }))}
              className="form-input"
              placeholder="Minimum 6 characters"
            />
          </div>
          <div>
            <label className="form-label">Role *</label>
            <select
              value={form.role}
              onChange={(e) => {
                const role = e.target.value as CreateUserRequest['role'];
                setForm((p) => ({
                  ...p,
                  role,
                  company_ids: (role === 'exec' || role === 'employee') ? p.company_ids.slice(0, 1) : p.company_ids,
                }));
              }}
              className="form-input"
            >
              {ALL_ROLES.map((r) => (
                <option key={r.value} value={r.value}>{r.label}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="form-label">
              Assign Companies * {isSingleCompany && <span className="text-gray-400 font-normal">(max 1)</span>}
            </label>
            <div className="space-y-2 mt-2 max-h-48 overflow-y-auto">
              {companies.length === 0 ? (
                <p className="text-sm text-gray-400">No companies available. Create a company first.</p>
              ) : (
                companies.map((c) => (
                  <label
                    key={c.id}
                    className={`flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${
                      form.company_ids.includes(c.id)
                        ? 'border-black bg-gray-50'
                        : 'border-gray-200 hover:border-gray-300'
                    }`}
                  >
                    <input
                      type={isSingleCompany ? 'radio' : 'checkbox'}
                      name="company"
                      checked={form.company_ids.includes(c.id)}
                      onChange={() => toggleCompany(c.id)}
                      className="accent-black"
                    />
                    <Building2 className="w-4 h-4 text-gray-400" />
                    <span className="text-sm font-medium text-gray-700">{c.name}</span>
                  </label>
                ))
              )}
            </div>
          </div>
        </div>
        <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
          <button onClick={onClose} className="btn-secondary">Cancel</button>
          <button onClick={handleSubmit} disabled={mutation.isPending} className="btn-primary">
            {mutation.isPending ? 'Creating...' : 'Create User'}
          </button>
        </div>
      </div>
    </div>
  );
}

/* ─── Edit User Modal ─── */
function EditUserModal({
  user,
  companies,
  onClose,
  onUpdated,
}: {
  user: UserWithCompanies;
  companies: { id: string; name: string }[];
  onClose: () => void;
  onUpdated: () => void;
}) {
  const [form, setForm] = useState<UpdateUserRequest>({
    full_name: user.full_name,
    email: user.email,
    role: user.role,
    is_active: user.is_active !== false,
    company_ids: user.companies.map((c) => c.id),
  });
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: () => updateUser(user.id, form),
    onSuccess: onUpdated,
    onError: (err: any) => setError(err.response?.data?.error || 'Failed to update user'),
  });

  const isSingleCompany = form.role === 'exec' || form.role === 'employee';

  const toggleCompany = (id: string) => {
    if (isSingleCompany) {
      setForm((p) => ({ ...p, company_ids: [id] }));
    } else {
      setForm((p) => ({
        ...p,
        company_ids: (p.company_ids ?? []).includes(id)
          ? (p.company_ids ?? []).filter((c) => c !== id)
          : [...(p.company_ids ?? []), id],
      }));
    }
  };

  const handleSubmit = () => {
    if (!form.full_name || !form.email) {
      setError('Name and email are required');
      return;
    }
    if (!form.company_ids || form.company_ids.length === 0) {
      setError('Select at least one company');
      return;
    }
    mutation.mutate();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-lg mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-6 border-b border-gray-100">
          <h2 className="text-lg font-semibold text-gray-900">Edit User</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-700">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="p-6 space-y-4">
          {error && <div className="p-3 bg-red-50 text-red-700 text-sm rounded-lg border border-red-100">{error}</div>}
          <div>
            <label className="form-label">Full Name *</label>
            <input
              value={form.full_name || ''}
              onChange={(e) => setForm((p) => ({ ...p, full_name: e.target.value }))}
              className="form-input"
            />
          </div>
          <div>
            <label className="form-label">Email *</label>
            <input
              type="email"
              value={form.email || ''}
              onChange={(e) => setForm((p) => ({ ...p, email: e.target.value }))}
              className="form-input"
            />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="form-label">Role</label>
              <select
                value={form.role || user.role}
                onChange={(e) => {
                  const role = e.target.value;
                  setForm((p) => ({
                    ...p,
                    role,
                    company_ids: (role === 'exec' || role === 'employee') ? (p.company_ids ?? []).slice(0, 1) : p.company_ids,
                  }));
                }}
                className="form-input"
              >
                {ALL_ROLES.map((r) => (
                  <option key={r.value} value={r.value}>{r.label}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="form-label">Status</label>
              <select
                value={form.is_active === false ? 'inactive' : 'active'}
                onChange={(e) => setForm((p) => ({ ...p, is_active: e.target.value === 'active' }))}
                className="form-input"
              >
                <option value="active">Active</option>
                <option value="inactive">Inactive</option>
              </select>
            </div>
          </div>
          <div>
            <label className="form-label">
              Assign Companies * {isSingleCompany && <span className="text-gray-400 font-normal">(max 1)</span>}
            </label>
            <div className="space-y-2 mt-2 max-h-48 overflow-y-auto">
              {companies.map((c) => (
                <label
                  key={c.id}
                  className={`flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${
                    (form.company_ids ?? []).includes(c.id)
                      ? 'border-black bg-gray-50'
                      : 'border-gray-200 hover:border-gray-300'
                  }`}
                >
                  <input
                    type={isSingleCompany ? 'radio' : 'checkbox'}
                    name="company"
                    checked={(form.company_ids ?? []).includes(c.id)}
                    onChange={() => toggleCompany(c.id)}
                    className="accent-black"
                  />
                  <Building2 className="w-4 h-4 text-gray-400" />
                  <span className="text-sm font-medium text-gray-700">{c.name}</span>
                </label>
              ))}
            </div>
          </div>
        </div>
        <div className="flex justify-end gap-3 p-6 border-t border-gray-100">
          <button onClick={onClose} className="btn-secondary">Cancel</button>
          <button onClick={handleSubmit} disabled={mutation.isPending} className="btn-primary">
            {mutation.isPending ? 'Saving...' : 'Save Changes'}
          </button>
        </div>
      </div>
    </div>
  );
}
