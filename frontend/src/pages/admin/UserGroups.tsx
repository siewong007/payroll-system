import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle, Loader2, Plus, Search, Trash2, UserPlus, Users } from 'lucide-react';
import { listUsers } from '@/api/admin';
import { permissionsApi, type PermissionDescriptor, type PermissionKey } from '@/api/permissions';
import { userGroupsApi, type UserGroup, type UserGroupMember } from '@/api/userGroups';
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { StepUpConfirmModal } from '@/components/ui/StepUpConfirmModal';
import { useAuth } from '@/context/AuthContext';
import { useConfirm } from '@/lib/confirm';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';
import type { StepUpCredentials } from '@/types';

const PLATFORM_ONLY_PERMISSIONS = new Set<PermissionKey>([
  'manage_users',
  'manage_companies',
  'manage_backups',
  'manage_platform_settings',
]);
const PAYROLL_PREPARE_PERMISSIONS = new Set<PermissionKey>([
  'manage_payroll_draft',
  'submit_payroll',
]);

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
  const { t } = useTranslation();
  const toast = useToast();
  const { can } = usePermissions();
  const canManageUsers = can('manage_users');
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState<UserGroup | null>(null);
  const [creating, setCreating] = useState(false);
  const [membersGroup, setMembersGroup] = useState<UserGroup | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<UserGroup | null>(null);
  const [deleteStepUpError, setDeleteStepUpError] = useState('');
  const [deletingGroupId, setDeletingGroupId] = useState<string | null>(null);

  const groups = useQuery({ queryKey: ['user-groups'], queryFn: userGroupsApi.list });
  const matrix = useQuery({
    queryKey: ['auth', 'permissions', 'matrix'],
    queryFn: permissionsApi.matrix,
    staleTime: 5 * 60_000,
  });

  const remove = useMutation({
    mutationFn: ({ groupId, stepUp }: { groupId: string; stepUp: StepUpCredentials }) =>
      userGroupsApi.remove(groupId, stepUp),
    onMutate: ({ groupId }) => setDeletingGroupId(groupId),
    onSettled: () => setDeletingGroupId(null),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['user-groups'] });
      toast.success(t('userGroups.deleteSuccess'));
      setDeleteTarget(null);
      setDeleteStepUpError('');
    },
    onError: (err: unknown) => {
      const msg = getErrorMessage(err, t('userGroups.deleteFailed'));
      setDeleteStepUpError(msg);
      toast.error(msg);
    },
  });

  const permissionsByGroup = useMemo(() => {
    const grouped = new Map<string, PermissionDescriptor[]>();
    for (const permission of matrix.data?.permissions ?? []) {
      if (PLATFORM_ONLY_PERMISSIONS.has(permission.key)) continue;
      const list = grouped.get(permission.group) ?? [];
      list.push(permission);
      grouped.set(permission.group, list);
    }
    return grouped;
  }, [matrix.data]);

  const labelFor = useMemo(() => {
    const labels = new Map<string, string>();
    for (const permission of matrix.data?.permissions ?? []) {
      labels.set(permission.key, t(`permissions.labels.${permission.key}`, { defaultValue: permission.label }));
    }
    return labels;
  }, [matrix.data, t]);

  if (groups.isLoading || matrix.isLoading) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('userGroups.title')} subtitle={t('userGroups.subtitle')} />
        <div role="status" aria-label={t('common.loading')} className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {[0, 1].map((i) => (
            <div key={i} className="card p-5 space-y-3">
              <Skeleton className="h-5 w-40" />
              <Skeleton className="h-4 w-64" />
              <div className="flex gap-2 pt-1">
                <Skeleton className="h-6 w-24 rounded-full" />
                <Skeleton className="h-6 w-28 rounded-full" />
              </div>
            </div>
          ))}
        </div>
      </div>
    );
  }

  if (groups.isError || matrix.isError) {
    return (
      <div className="space-y-6">
        <PageHeader title={t('userGroups.title')} subtitle={t('userGroups.subtitle')} />
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('userGroups.loadFailed')}
            description={getErrorMessage(groups.error ?? matrix.error, t('common.error'))}
            action={
              <button
                type="button"
                onClick={() => {
                  if (groups.isError) void groups.refetch();
                  if (matrix.isError) void matrix.refetch();
                }}
                className="btn-secondary"
              >
                {t('common.retry')}
              </button>
            }
          />
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('userGroups.title')}
        subtitle={t('userGroups.subtitle')}
        actions={
          canManageUsers ? (
            <button type="button" className="btn-primary w-full sm:w-auto" onClick={() => setCreating(true)}>
              <Plus className="w-4 h-4" aria-hidden="true" />
              {t('userGroups.newGroup')}
            </button>
          ) : undefined
        }
      />

      {groups.data?.length === 0 && (
        <div className="card">
          <EmptyState
            icon={Users}
            title={t('userGroups.empty')}
            action={
              canManageUsers ? (
                <button type="button" className="btn-primary" onClick={() => setCreating(true)}>
                  <Plus className="w-4 h-4" aria-hidden="true" />
                  {t('userGroups.newGroup')}
                </button>
              ) : undefined
            }
          />
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {groups.data?.map((group) => (
          <div key={group.id} className="card p-5 space-y-3">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <h2 className="font-semibold text-gray-900 dark:text-slate-100 truncate">{group.name}</h2>
                {group.description && (
                  <p className="text-sm text-gray-500 dark:text-slate-400 mt-0.5 break-words">{group.description}</p>
                )}
              </div>
              <div className="flex items-center gap-2 shrink-0">
                {!group.is_active && (
                  <span className="text-[11px] uppercase tracking-wider font-semibold text-amber-700 dark:text-amber-400">
                    {t('userGroups.suspended')}
                  </span>
                )}
                {canManageUsers && (
                  <button
                    type="button"
                    className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 inline-flex items-center justify-center p-1.5 text-gray-500 dark:text-slate-400 hover:text-red-700 dark:hover:text-red-400 rounded-lg hover:bg-red-50 dark:hover:bg-red-500/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] disabled:opacity-50 transition-colors"
                    aria-label={t('userGroups.deleteGroup', { name: group.name })}
                    disabled={deletingGroupId === group.id}
                    onClick={() => {
                      setDeleteStepUpError('');
                      setDeleteTarget(group);
                    }}
                  >
                    {deletingGroupId === group.id ? (
                      <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                    ) : (
                      <Trash2 className="w-4 h-4" aria-hidden="true" />
                    )}
                  </button>
                )}
              </div>
            </div>

            <div className="flex flex-wrap gap-1.5">
              {group.permissions.length === 0 ? (
                <span className="text-xs text-gray-600 dark:text-slate-400">{t('userGroups.grantsNothing')}</span>
              ) : (
                group.permissions.map((key) => (
                  <span
                    key={key}
                    className="text-xs px-2 py-0.5 rounded-full bg-[var(--accent-soft)] text-[var(--accent-1)] border border-[var(--border)]"
                  >
                    {labelFor.get(key) ?? key}
                  </span>
                ))
              )}
            </div>

            <div className="flex flex-wrap items-center justify-between gap-2 pt-1">
              <span className="text-xs text-gray-600 dark:text-slate-400">
                {t('teams.memberCount', { count: group.member_count })}
              </span>
              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  className="btn-secondary text-xs min-h-11 sm:min-h-9"
                  aria-label={t('userGroups.manageMembersFor', { name: group.name })}
                  onClick={() => setMembersGroup(group)}
                >
                  <Users className="w-3.5 h-3.5" aria-hidden="true" />
                  {t('userGroups.members')}
                </button>
                {canManageUsers && (
                  <button
                    type="button"
                    className="btn-secondary text-xs min-h-11 sm:min-h-9"
                    aria-label={t('common.editName', { name: group.name })}
                    onClick={() => setEditing(group)}
                  >
                    {t('common.edit')}
                  </button>
                )}
              </div>
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
          onSaved={(wasEditing) => {
            queryClient.invalidateQueries({ queryKey: ['user-groups'] });
            toast.success(wasEditing ? t('userGroups.updateSuccess') : t('userGroups.createSuccess'));
            setCreating(false);
            setEditing(null);
          }}
        />
      )}

      {membersGroup && (
        <GroupMembersModal
          group={membersGroup}
          canManageUsers={canManageUsers}
          onClose={() => setMembersGroup(null)}
        />
      )}

      <StepUpConfirmModal
        open={deleteTarget !== null}
        onClose={() => {
          if (!remove.isPending) {
            setDeleteTarget(null);
            setDeleteStepUpError('');
          }
        }}
        title={deleteTarget ? t('userGroups.deleteGroup', { name: deleteTarget.name }) : ''}
        description={
          deleteTarget ? t('userGroups.deleteConfirm', { name: deleteTarget.name }) : undefined
        }
        confirmLabel={t('common.delete')}
        pendingLabel={t('common.deleting')}
        variant="danger"
        pending={remove.isPending}
        error={deleteStepUpError}
        onConfirm={(stepUp) => {
          if (deleteTarget) {
            setDeleteStepUpError('');
            remove.mutate({ groupId: deleteTarget.id, stepUp });
          }
        }}
      />
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
  onSaved: (wasEditing: boolean) => void;
}) {
  const { t } = useTranslation();
  const { user: currentUser } = useAuth();
  const actorTwoFactorEnabled = Boolean(currentUser?.two_factor_enabled);
  const [name, setName] = useState(group?.name ?? '');
  const [description, setDescription] = useState(group?.description ?? '');
  const [isActive, setIsActive] = useState(group?.is_active ?? true);
  const [selected, setSelected] = useState<PermissionKey[]>(group?.permissions ?? []);
  const [actorPassword, setActorPassword] = useState('');
  const [actorTotpCode, setActorTotpCode] = useState('');
  const [error, setError] = useState<string | null>(null);

  const hasPreparePermission = selected.some((k) => PAYROLL_PREPARE_PERMISSIONS.has(k));
  const hasApprovePermission = selected.includes('approve_payroll');

  const isPermissionDisabled = (key: PermissionKey) =>
    (key === 'approve_payroll' && hasPreparePermission && !hasApprovePermission) ||
    (PAYROLL_PREPARE_PERMISSIONS.has(key) && hasApprovePermission && !selected.includes(key));

  const save = useMutation({
    mutationFn: async () => {
      const trimmedName = name.trim();
      const trimmedDesc = description.trim();
      const stepUp: StepUpCredentials = {
        actor_password: actorPassword,
        ...(actorTwoFactorEnabled && actorTotpCode.trim()
          ? { actor_totp_code: actorTotpCode.trim() }
          : {}),
      };
      if (group) {
        return userGroupsApi.update(group.id, {
          name: trimmedName,
          description: trimmedDesc,
          is_active: isActive,
          permissions: selected,
          ...stepUp,
        });
      }
      return userGroupsApi.create({
        name: trimmedName,
        description: trimmedDesc || undefined,
        permissions: selected,
        ...stepUp,
      });
    },
    onSuccess: () => onSaved(Boolean(group)),
    onError: (err: unknown) => {
      setError(getErrorMessage(err, t('userGroups.saveFailed')));
    },
  });

  function toggle(key: PermissionKey) {
    if (isPermissionDisabled(key)) return;
    setSelected((current) =>
      current.includes(key) ? current.filter((k) => k !== key) : [...current, key],
    );
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setError(t('validation.required'));
      return;
    }
    if (!actorPassword) {
      setError(t('users.errors.actorPasswordRequired'));
      return;
    }
    if (actorTwoFactorEnabled && !actorTotpCode.trim()) {
      setError(t('users.errors.actorTotpRequired'));
      return;
    }
    setError(null);
    save.mutate();
  };

  const selectGroup = (groupPermissions: PermissionDescriptor[]) => {
    setSelected((current) => {
      const next = [...current];
      for (const p of groupPermissions) {
        if (next.includes(p.key)) continue;
        const hasPrep = next.some((k) => PAYROLL_PREPARE_PERMISSIONS.has(k));
        const hasAppr = next.includes('approve_payroll');
        if (p.key === 'approve_payroll' && hasPrep) continue;
        if (PAYROLL_PREPARE_PERMISSIONS.has(p.key) && hasAppr) continue;
        next.push(p.key);
      }
      return next;
    });
  };

  const clearGroup = (groupPermissions: PermissionDescriptor[]) => {
    const groupKeys = new Set(groupPermissions.map((p) => p.key));
    setSelected((current) => current.filter((k) => !groupKeys.has(k)));
  };

  return (
    <Modal
      open
      onClose={() => { if (!save.isPending) onClose(); }}
      title={group ? t('userGroups.editGroup') : t('userGroups.newGroup')}
      maxWidth="max-w-2xl"
      footer={
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <button type="button" disabled={save.isPending} className="btn-secondary" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            form="group-editor-form"
            className="btn-primary inline-flex items-center justify-center gap-2"
            disabled={
              !name.trim() ||
              !actorPassword ||
              (actorTwoFactorEnabled && !actorTotpCode.trim()) ||
              save.isPending
            }
          >
            {save.isPending ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                {t('common.saving')}
              </>
            ) : (
              t('common.save')
            )}
          </button>
        </div>
      }
    >
      <form id="group-editor-form" onSubmit={handleSubmit} className="space-y-4">
        {error && (
          <div
            role="alert"
            className="p-3 bg-red-50 dark:bg-red-500/10 text-red-700 dark:text-red-300 text-sm rounded-lg border border-red-100 dark:border-red-500/25"
          >
            {error}
          </div>
        )}

        <FormField label={t('common.name')} required>
          {(aria) => (
            <input
              {...aria}
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="form-input"
            />
          )}
        </FormField>

        <FormField label={t('common.description')}>
          {(aria) => (
            <input
              {...aria}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="form-input"
            />
          )}
        </FormField>

        {group && (
          <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-slate-200 cursor-pointer">
            <input
              type="checkbox"
              checked={isActive}
              onChange={(e) => setIsActive(e.target.checked)}
              className="rounded border-gray-300 dark:border-slate-600 text-[var(--accent-1)] focus:ring-[var(--ring)]"
            />
            {t('userGroups.activeLabel')}
          </label>
        )}

        <div className="space-y-3">
          <div className="space-y-1">
            <p className="text-xs font-medium text-gray-600 dark:text-slate-300">{t('userGroups.permissionsGranted')}</p>
            {(hasPreparePermission || hasApprovePermission) && (
              <p className="text-xs text-amber-700 dark:text-amber-400">{t('userGroups.payrollSeparationHint')}</p>
            )}
          </div>
          {[...permissionsByGroup.entries()].map(([groupName, permissions]) => {
            const localizedGroup = t(`permissions.groups.${groupName.toLowerCase().replace(/[^a-z0-9]+/g, '_')}`, { defaultValue: groupName });
            const hasAnySelected = permissions.some((p) => selected.includes(p.key));
            return (
              <fieldset key={groupName} className="rounded-xl border border-gray-200 dark:border-slate-700/80 p-3.5 space-y-2.5">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <legend className="text-[11px] uppercase tracking-wider font-semibold text-gray-600 dark:text-slate-300">
                    {localizedGroup}
                  </legend>
                  <div className="flex items-center gap-1">
                    <button
                      type="button"
                      onClick={() => selectGroup(permissions)}
                      aria-label={`${t('common.selectAll')} — ${localizedGroup}`}
                      className="min-h-8 px-2 py-1 text-xs font-medium text-[var(--accent-1)] hover:bg-[var(--accent-soft)] rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
                    >
                      {t('common.selectAll')}
                    </button>
                    {hasAnySelected && (
                      <button
                        type="button"
                        onClick={() => clearGroup(permissions)}
                        aria-label={`${t('common.clear')} — ${localizedGroup}`}
                        className="min-h-8 px-2 py-1 text-xs font-medium text-gray-600 dark:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] transition-colors"
                      >
                        {t('common.clear')}
                      </button>
                    )}
                  </div>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-1.5">
                  {permissions.map((permission) => {
                    const disabled = isPermissionDisabled(permission.key);
                    return (
                      <label
                        key={permission.key}
                        className={`flex items-center gap-2 min-h-10 sm:min-h-9 rounded-lg px-2 py-1.5 text-sm text-gray-800 dark:text-slate-200 hover:bg-gray-50 dark:hover:bg-slate-800/60 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-[var(--ring)] transition-colors ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}`}
                      >
                        <input
                          type="checkbox"
                          checked={selected.includes(permission.key)}
                          disabled={disabled}
                          onChange={() => toggle(permission.key)}
                          className="rounded border-gray-300 dark:border-slate-600 text-[var(--accent-1)] focus:ring-[var(--ring)]"
                        />
                        {t(`permissions.labels.${permission.key}`, { defaultValue: permission.label })}
                      </label>
                    );
                  })}
                </div>
              </fieldset>
            );
          })}
        </div>

        <div className="space-y-4 pt-3 border-t border-gray-200 dark:border-slate-700">
          <FormField label={t('users.confirmWithYourPassword')} required>
            {(aria) => (
              <input
                {...aria}
                type="password"
                value={actorPassword}
                onChange={(e) => setActorPassword(e.target.value)}
                placeholder={t('auth.twoFactor.currentPasswordPlaceholder')}
                className="form-input"
                autoComplete="current-password"
                required
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
                  value={actorTotpCode}
                  onChange={(e) => setActorTotpCode(e.target.value)}
                  placeholder="123456"
                  className="form-input text-center tracking-widest"
                  required
                />
              )}
            </FormField>
          )}
        </div>
      </form>
    </Modal>
  );
}

function GroupMembersModal({
  group,
  canManageUsers,
  onClose,
}: {
  group: UserGroup;
  canManageUsers: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();
  const confirmDialog = useConfirm();
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [addingUserId, setAddingUserId] = useState<string | null>(null);
  const [removingUserId, setRemovingUserId] = useState<string | null>(null);
  const [pendingAddUser, setPendingAddUser] = useState<{ id: string; full_name: string } | null>(null);
  const [addStepUpError, setAddStepUpError] = useState('');

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedSearch(search.trim()), 250);
    return () => window.clearTimeout(timer);
  }, [search]);

  const membersQuery = useQuery({
    queryKey: ['user-group-members', group.id],
    queryFn: () => userGroupsApi.members(group.id),
  });

  const usersQuery = useQuery({
    queryKey: ['admin-users', 'group-picker', group.company_id, debouncedSearch],
    queryFn: () => listUsers({ companyId: group.company_id, search: debouncedSearch || undefined, perPage: 50 }),
    enabled: canManageUsers,
  });

  const addMutation = useMutation({
    mutationFn: ({ userId, stepUp }: { userId: string; stepUp: StepUpCredentials }) =>
      userGroupsApi.addMember(group.id, userId, stepUp),
    onMutate: ({ userId }) => setAddingUserId(userId),
    onSettled: () => setAddingUserId(null),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['user-group-members', group.id] });
      queryClient.invalidateQueries({ queryKey: ['user-groups'] });
      toast.success(t('userGroups.memberAdded'));
      setPendingAddUser(null);
      setAddStepUpError('');
    },
    onError: (err: unknown) => {
      const msg = getErrorMessage(err, t('userGroups.addMemberFailed'));
      setAddStepUpError(msg);
      toast.error(msg);
    },
  });

  const removeMutation = useMutation({
    mutationFn: (userId: string) => userGroupsApi.removeMember(group.id, userId),
    onMutate: (userId) => setRemovingUserId(userId),
    onSettled: () => setRemovingUserId(null),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['user-group-members', group.id] });
      queryClient.invalidateQueries({ queryKey: ['user-groups'] });
      toast.success(t('userGroups.memberRemoved'));
    },
    onError: (err: unknown) => {
      toast.error(getErrorMessage(err, t('userGroups.removeMemberFailed')));
    },
  });

  const members: UserGroupMember[] = useMemo(() => membersQuery.data ?? [], [membersQuery.data]);
  const memberIds = useMemo(() => new Set(members.map((m) => m.user_id)), [members]);

  const candidates = useMemo(() => {
    const q = search.trim().toLowerCase();
    return (usersQuery.data?.data ?? []).filter((u) => {
      if (u.is_active === false || memberIds.has(u.id)) return false;
      if (!q) return true;
      return u.full_name.toLowerCase().includes(q) || u.email.toLowerCase().includes(q);
    });
  }, [usersQuery.data, memberIds, search]);

  const handleRemove = async (member: UserGroupMember) => {
    const ok = await confirmDialog({
      title: t('userGroups.removeMember'),
      description: t('userGroups.removeMemberConfirm', { name: member.full_name }),
      confirmLabel: t('common.remove'),
      variant: 'danger',
    });
    if (ok) {
      removeMutation.mutate(member.user_id);
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={t('userGroups.membersTitle', { name: group.name })}
      maxWidth="max-w-lg"
      footer={
        <div className="flex justify-end">
          <button type="button" onClick={onClose} className="btn-secondary">
            {t('common.close')}
          </button>
        </div>
      }
    >
      <div className="space-y-5">
        <div>
          <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-600 dark:text-slate-400 mb-2">
            {t('userGroups.currentMembers', { count: members.length })}
          </h3>
          {membersQuery.isLoading ? (
            <div role="status" aria-label={t('common.loading')} className="space-y-2 py-2">
              <Skeleton className="h-12 w-full rounded-xl" />
              <Skeleton className="h-12 w-full rounded-xl" />
            </div>
          ) : membersQuery.isError ? (
            <EmptyState
              compact
              icon={AlertTriangle}
              title={t('userGroups.membersLoadFailed')}
              description={getErrorMessage(membersQuery.error, t('common.error'))}
              action={
                <button type="button" onClick={() => void membersQuery.refetch()} className="btn-secondary">
                  {t('common.retry')}
                </button>
              }
            />
          ) : members.length === 0 ? (
            <p className="text-sm text-gray-600 dark:text-slate-400 py-2">{t('userGroups.noMembers')}</p>
          ) : (
            <ul className="divide-y divide-gray-100 dark:divide-slate-800 max-h-56 overflow-y-auto border border-gray-100 dark:border-slate-800 rounded-xl">
              {members.map((m) => (
                <li key={m.user_id} className="flex items-center justify-between gap-3 px-3 py-2.5">
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{m.full_name}</p>
                    <p className="text-xs text-gray-600 dark:text-slate-400 truncate">{m.email}</p>
                  </div>
                  {canManageUsers && (
                    <button
                      type="button"
                      disabled={removingUserId === m.user_id}
                      onClick={() => { void handleRemove(m); }}
                      aria-label={t('userGroups.removeMemberName', { name: m.full_name })}
                      className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 inline-flex items-center justify-center p-1.5 text-gray-500 dark:text-slate-400 hover:text-red-700 dark:hover:text-red-400 rounded-lg hover:bg-red-50 dark:hover:bg-red-500/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] disabled:opacity-50 transition-colors"
                    >
                      {removingUserId === m.user_id ? (
                        <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                      ) : (
                        <Trash2 className="w-4 h-4" aria-hidden="true" />
                      )}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>

        {canManageUsers && (
          <div className="space-y-2.5 pt-2 border-t border-gray-100 dark:border-slate-800">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-600 dark:text-slate-400">
              {t('userGroups.addMember')}
            </h3>
            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 dark:text-slate-500" aria-hidden="true" />
              <input
                type="search"
                aria-label={t('users.searchLabel')}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="form-input pl-9"
                placeholder={t('users.searchPlaceholder')}
              />
            </div>

            {usersQuery.isLoading ? (
              <div role="status" aria-label={t('common.loading')} className="space-y-2 py-2">
                <Skeleton className="h-11 w-full rounded-xl" />
                <Skeleton className="h-11 w-full rounded-xl" />
              </div>
            ) : usersQuery.isError ? (
              <EmptyState
                compact
                icon={AlertTriangle}
                title={t('users.loadFailed')}
                description={getErrorMessage(usersQuery.error, t('common.error'))}
                action={
                  <button type="button" onClick={() => void usersQuery.refetch()} className="btn-secondary">
                    {t('common.retry')}
                  </button>
                }
              />
            ) : candidates.length === 0 ? (
              <p className="text-sm text-gray-600 dark:text-slate-400 py-2">
                {search.trim() ? t('common.noResults') : t('userGroups.allUsersAdded')}
              </p>
            ) : (
              <ul className="divide-y divide-gray-100 dark:divide-slate-800 max-h-48 overflow-y-auto border border-gray-100 dark:border-slate-800 rounded-xl">
                {candidates.map((u) => (
                  <li key={u.id} className="flex items-center justify-between gap-3 px-3 py-2">
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{u.full_name}</p>
                      <p className="text-xs text-gray-600 dark:text-slate-400 truncate">{u.email}</p>
                    </div>
                    <button
                      type="button"
                      disabled={addingUserId === u.id}
                      onClick={() => {
                        setAddStepUpError('');
                        setPendingAddUser({ id: u.id, full_name: u.full_name });
                      }}
                      aria-label={t('userGroups.addMemberName', { name: u.full_name })}
                      className="btn-secondary text-xs min-h-11 sm:min-h-9 shrink-0 inline-flex items-center gap-1.5"
                    >
                      {addingUserId === u.id ? (
                        <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                      ) : (
                        <UserPlus className="w-3.5 h-3.5" aria-hidden="true" />
                      )}
                      {addingUserId === u.id ? t('common.adding') : t('common.add')}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>

      <StepUpConfirmModal
        open={pendingAddUser !== null}
        onClose={() => {
          if (!addMutation.isPending) {
            setPendingAddUser(null);
            setAddStepUpError('');
          }
        }}
        title={t('userGroups.confirmAddMemberTitle')}
        description={
          pendingAddUser
            ? t('userGroups.addMemberConfirm', { name: pendingAddUser.full_name })
            : undefined
        }
        confirmLabel={t('common.add')}
        pendingLabel={t('common.adding')}
        pending={addMutation.isPending}
        error={addStepUpError}
        onConfirm={(stepUp) => {
          if (pendingAddUser) {
            setAddStepUpError('');
            addMutation.mutate({ userId: pendingAddUser.id, stepUp });
          }
        }}
      />
    </Modal>
  );
}
