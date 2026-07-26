import { Fragment, useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, Users, X } from 'lucide-react';
import { permissionsApi, type PermissionDescriptor, type PermissionKey } from '@/api/permissions';
import { userGroupsApi, type UserGroup } from '@/api/userGroups';

/**
 * User groups grant permissions on top of roles.
 *
 * Roles stay code-defined because payroll separation of duties depends on being
 * reviewable in one place; groups cover what roles cannot express — "the two
 * people who also handle the calendar", "contractors who may read attendance
 * and nothing else". Grants are additive only: a group can never remove a
 * capability a role confers, so the answer to "why can this person do X?" is
 * always a union and never an ordering.
 */
export function UserGroups() {
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState<UserGroup | null>(null);
  const [creating, setCreating] = useState(false);

  const groups = useQuery({ queryKey: ['user-groups'], queryFn: userGroupsApi.list });
  const matrix = useQuery({
    queryKey: ['auth', 'permissions', 'matrix'],
    queryFn: permissionsApi.matrix,
    staleTime: 5 * 60_000,
  });

  const remove = useMutation({
    mutationFn: userGroupsApi.remove,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['user-groups'] }),
  });

  const permissionsByGroup = useMemo(() => {
    const grouped = new Map<string, PermissionDescriptor[]>();
    for (const permission of matrix.data?.permissions ?? []) {
      const list = grouped.get(permission.group) ?? [];
      list.push(permission);
      grouped.set(permission.group, list);
    }
    return grouped;
  }, [matrix.data]);

  const labelFor = useMemo(() => {
    const labels = new Map<string, string>();
    for (const permission of matrix.data?.permissions ?? []) {
      labels.set(permission.key, permission.label);
    }
    return labels;
  }, [matrix.data]);

  if (groups.isLoading || matrix.isLoading) {
    return (
      <div className="flex min-h-40 items-center justify-center">
        <div className="spinner" />
      </div>
    );
  }

  if (groups.isError) {
    return (
      <div className="card p-5">
        <p className="text-sm text-red-600">Could not load user groups.</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="page-header flex items-start justify-between gap-4">
        <div>
          <h1 className="page-title">User Groups</h1>
          <p className="page-subtitle">
            Grant extra permissions to a set of users, on top of whatever their roles already allow
          </p>
        </div>
        <button className="btn-primary shrink-0" onClick={() => setCreating(true)}>
          <Plus className="w-4 h-4" />
          New group
        </button>
      </div>

      {groups.data?.length === 0 && (
        <div className="card p-8 text-center">
          <Users className="w-8 h-8 mx-auto text-gray-300" />
          <p className="mt-3 text-sm text-gray-500">
            No groups yet. Roles cover most cases — add a group when you need to give specific
            people one extra capability.
          </p>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {groups.data?.map((group) => (
          <div key={group.id} className="card p-5 space-y-3">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <h2 className="font-semibold text-gray-900 truncate">{group.name}</h2>
                {group.description && (
                  <p className="text-sm text-gray-500 mt-0.5">{group.description}</p>
                )}
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {!group.is_active && (
                  <span className="text-[10px] uppercase tracking-wider font-semibold text-amber-600">
                    Suspended
                  </span>
                )}
                <button
                  className="p-1.5 text-gray-400 hover:text-red-600 rounded-lg hover:bg-red-50"
                  aria-label={`Delete ${group.name}`}
                  onClick={() => {
                    // Deleting cascades to membership, revoking access from
                    // everyone who held it — worth one confirmation.
                    if (window.confirm(`Delete "${group.name}"? Members lose these permissions.`)) {
                      remove.mutate(group.id);
                    }
                  }}
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            </div>

            <div className="flex flex-wrap gap-1.5">
              {group.permissions.length === 0 ? (
                <span className="text-xs text-gray-400">Grants nothing yet</span>
              ) : (
                group.permissions.map((key) => (
                  <span
                    key={key}
                    className="text-xs px-2 py-0.5 rounded-full bg-indigo-50 text-indigo-700"
                  >
                    {labelFor.get(key) ?? key}
                  </span>
                ))
              )}
            </div>

            <div className="flex items-center justify-between pt-1">
              <span className="text-xs text-gray-500">
                {group.member_count} member{group.member_count === 1 ? '' : 's'}
              </span>
              <button className="btn-secondary text-xs" onClick={() => setEditing(group)}>
                Edit
              </button>
            </div>
          </div>
        ))}
      </div>

      {(creating || editing) && (
        <GroupEditor
          group={editing}
          permissionsByGroup={permissionsByGroup}
          onClose={() => {
            setCreating(false);
            setEditing(null);
          }}
          onSaved={() => {
            queryClient.invalidateQueries({ queryKey: ['user-groups'] });
            setCreating(false);
            setEditing(null);
          }}
        />
      )}
    </div>
  );
}

function GroupEditor({
  group,
  permissionsByGroup,
  onClose,
  onSaved,
}: {
  group: UserGroup | null;
  permissionsByGroup: Map<string, PermissionDescriptor[]>;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [name, setName] = useState(group?.name ?? '');
  const [description, setDescription] = useState(group?.description ?? '');
  const [selected, setSelected] = useState<PermissionKey[]>(group?.permissions ?? []);
  const [error, setError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: async () => {
      if (group) {
        return userGroupsApi.update(group.id, {
          name,
          description: description || undefined,
          permissions: selected,
        });
      }
      return userGroupsApi.create({ name, description: description || undefined, permissions: selected });
    },
    onSuccess: onSaved,
    onError: (err: { rateLimitMessage?: string; response?: { data?: { error?: string } } }) => {
      setError(err.rateLimitMessage ?? err.response?.data?.error ?? 'Could not save the group.');
    },
  });

  function toggle(key: PermissionKey) {
    setSelected((current) =>
      current.includes(key) ? current.filter((k) => k !== key) : [...current, key],
    );
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <div className="bg-white rounded-xl w-full max-w-2xl max-h-[85vh] flex flex-col">
        <div className="flex items-center justify-between p-5 border-b border-gray-100">
          <h2 className="font-semibold text-gray-900">{group ? 'Edit group' : 'New group'}</h2>
          <button onClick={onClose} aria-label="Close" className="p-1 text-gray-400 hover:text-gray-600">
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-5 space-y-4 overflow-y-auto">
          {error && <p className="text-sm text-red-600">{error}</p>}

          <div>
            <label htmlFor="group-name" className="block text-xs font-medium text-gray-500 mb-1">
              Name
            </label>
            <input
              id="group-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm"
            />
          </div>

          <div>
            <label htmlFor="group-desc" className="block text-xs font-medium text-gray-500 mb-1">
              Description
            </label>
            <input
              id="group-desc"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm"
            />
          </div>

          <div className="space-y-3">
            <p className="text-xs font-medium text-gray-500">Permissions granted</p>
            {[...permissionsByGroup.entries()].map(([groupName, permissions]) => (
              <Fragment key={groupName}>
                <p className="text-[10px] uppercase tracking-wider font-semibold text-gray-400">
                  {groupName}
                </p>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-1.5">
                  {permissions.map((permission) => (
                    <label
                      key={permission.key}
                      className="flex items-center gap-2 text-sm text-gray-700"
                    >
                      <input
                        type="checkbox"
                        checked={selected.includes(permission.key)}
                        onChange={() => toggle(permission.key)}
                      />
                      {permission.label}
                    </label>
                  ))}
                </div>
              </Fragment>
            ))}
          </div>
        </div>

        <div className="flex justify-end gap-2 p-5 border-t border-gray-100">
          <button className="btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn-primary"
            disabled={!name.trim() || save.isPending}
            onClick={() => {
              setError(null);
              save.mutate();
            }}
          >
            {save.isPending ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  );
}
