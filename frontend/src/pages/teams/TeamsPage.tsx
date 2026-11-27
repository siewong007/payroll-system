import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle, ChevronRight, Loader2, Plus, Search, Tag, Trash2, UserPlus, Users, X } from 'lucide-react';
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { useConfirm } from '@/lib/confirm';
import { usePermissions } from '@/lib/usePermissions';
import { useToast } from '@/lib/toast';
import { getErrorMessage } from '@/lib/utils';
import { getTeams, createTeam, updateTeam, deleteTeam, getTeamMembers, addTeamMember, removeTeamMember } from '@/api/teams';
import { getEmployees } from '@/api/employees';
import type { TeamWithCount, TeamMember, CreateTeamRequest, UpdateTeamRequest } from '@/types';

const TAG_COLORS: Record<string, { bg: string; text: string; icon: string }> = {
  engineering: {
    bg: 'bg-blue-100 dark:bg-blue-500/15',
    text: 'text-blue-700 dark:text-blue-300',
    icon: 'bg-blue-600 dark:bg-blue-500',
  },
  sales: {
    bg: 'bg-green-100 dark:bg-emerald-500/15',
    text: 'text-green-700 dark:text-emerald-300',
    icon: 'bg-green-600 dark:bg-emerald-500',
  },
  marketing: {
    bg: 'bg-zinc-100 dark:bg-zinc-800',
    text: 'text-zinc-700 dark:text-zinc-300',
    icon: 'bg-zinc-600 dark:bg-zinc-500',
  },
  support: {
    bg: 'bg-orange-100 dark:bg-orange-500/15',
    text: 'text-orange-700 dark:text-orange-300',
    icon: 'bg-orange-600 dark:bg-orange-500',
  },
  corporate: {
    bg: 'bg-slate-100 dark:bg-slate-700/60',
    text: 'text-slate-700 dark:text-slate-200',
    icon: 'bg-slate-600 dark:bg-slate-500',
  },
  project: {
    bg: 'bg-pink-100 dark:bg-pink-500/15',
    text: 'text-pink-700 dark:text-pink-300',
    icon: 'bg-pink-600 dark:bg-pink-500',
  },
  general: {
    bg: 'bg-gray-100 dark:bg-slate-800',
    text: 'text-gray-700 dark:text-slate-300',
    icon: 'bg-gray-500 dark:bg-slate-600',
  },
};

function getTagStyle(tag: string) {
  return TAG_COLORS[tag.toLowerCase()] || TAG_COLORS.general;
}

export function TeamsPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const confirmDialog = useConfirm();
  const { can } = usePermissions();
  const canManageTeams = can('manage_teams');
  const canViewEmployees = can('view_employees');
  const queryClient = useQueryClient();
  const createFormId = useId();
  const editFormId = useId();
  const detailRef = useRef<HTMLDivElement>(null);

  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showEditModal, setShowEditModal] = useState(false);
  const [showAddMemberModal, setShowAddMemberModal] = useState(false);
  const [editingTeam, setEditingTeam] = useState<TeamWithCount | null>(null);
  const [memberSearch, setMemberSearch] = useState('');
  const [debouncedMemberSearch, setDebouncedMemberSearch] = useState('');
  const [removingEmployeeId, setRemovingEmployeeId] = useState<string | null>(null);
  const [addingEmployeeIds, setAddingEmployeeIds] = useState<Set<string>>(() => new Set());

  // Form state
  const [teamName, setTeamName] = useState('');
  const [teamDescription, setTeamDescription] = useState('');
  const [teamTag, setTeamTag] = useState('general');
  const [teamIsActive, setTeamIsActive] = useState(true);

  const {
    data: teams = [],
    isLoading,
    isError: teamsError,
    error: teamsErrorObj,
    refetch: refetchTeams,
  } = useQuery({
    queryKey: ['teams'],
    queryFn: getTeams,
  });

  const {
    data: members = [],
    isLoading: loadingMembers,
    isError: membersError,
    error: membersErrorObj,
    refetch: refetchMembers,
  } = useQuery({
    queryKey: ['team-members', selectedTeamId],
    queryFn: () => getTeamMembers(selectedTeamId!),
    enabled: !!selectedTeamId,
  });

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedMemberSearch(memberSearch), 250);
    return () => clearTimeout(timer);
  }, [memberSearch]);

  // The search runs server-side. `per_page: 500` was clamped to 100 by
  // `handlers/employee.rs`, so filtering client-side searched only the
  // alphabetically-first 100 employee numbers — silently.
  const {
    data: employeesData,
    isLoading: loadingAvailableEmployees,
    isError: availableEmployeesError,
    error: availableEmployeesErrorObj,
    refetch: refetchAvailableEmployees,
  } = useQuery({
    queryKey: ['team-employee-search', selectedTeamId, debouncedMemberSearch],
    queryFn: () => getEmployees({
      search: debouncedMemberSearch || undefined,
      is_active: true,
      per_page: 50,
    }),
    enabled: showAddMemberModal && canManageTeams && canViewEmployees,
  });

  const resetForm = () => {
    setTeamName('');
    setTeamDescription('');
    setTeamTag('general');
    setTeamIsActive(true);
    setEditingTeam(null);
  };

  const createMutation = useMutation({
    mutationFn: (data: CreateTeamRequest) => createTeam(data),
    onSuccess: (created) => {
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      setShowCreateModal(false);
      resetForm();
      setSelectedTeamId(created.id);
      toast.success(t('teams.createSuccess'));
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateTeamRequest }) => updateTeam(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      setShowEditModal(false);
      resetForm();
      toast.success(t('teams.updateSuccess'));
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteTeam(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      if (selectedTeamId) setSelectedTeamId(null);
      toast.success(t('teams.deleteSuccess'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('teams.deleteFailed'))),
  });

  const addMemberMutation = useMutation({
    mutationFn: ({ teamId, employeeId, role }: { teamId: string; employeeId: string; role: string }) =>
      addTeamMember(teamId, { employee_id: employeeId, role }),
    onMutate: ({ employeeId }) =>
      setAddingEmployeeIds((prev) => {
        const next = new Set(prev);
        next.add(employeeId);
        return next;
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-members', selectedTeamId] });
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      toast.success(t('teams.memberAdded'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('teams.addMemberFailed'))),
    onSettled: (_data, _err, { employeeId }) =>
      setAddingEmployeeIds((prev) => {
        const next = new Set(prev);
        next.delete(employeeId);
        return next;
      }),
  });

  const removeMemberMutation = useMutation({
    mutationFn: ({ teamId, employeeId }: { teamId: string; employeeId: string }) =>
      removeTeamMember(teamId, employeeId),
    onMutate: ({ employeeId }) => setRemovingEmployeeId(employeeId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-members', selectedTeamId] });
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      toast.success(t('teams.memberRemoved'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('teams.removeMemberFailed'))),
    onSettled: () => setRemovingEmployeeId(null),
  });

  const handleSelectTeam = (teamId: string) => {
    setSelectedTeamId(teamId);
    if (typeof window !== 'undefined' && window.innerWidth < 1024) {
      window.setTimeout(() => {
        detailRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }, 40);
    }
  };

  const openCreate = () => {
    resetForm();
    createMutation.reset();
    setShowCreateModal(true);
  };

  const openEdit = (team: TeamWithCount) => {
    setEditingTeam(team);
    setTeamName(team.name);
    setTeamDescription(team.description || '');
    setTeamTag(team.tag);
    setTeamIsActive(team.is_active);
    updateMutation.reset();
    setShowEditModal(true);
  };

  const handleCreate = () => {
    if (!teamName.trim()) return;
    createMutation.mutate({
      name: teamName.trim(),
      description: teamDescription.trim() || undefined,
      tag: teamTag.trim().toLowerCase() || 'general',
    });
  };

  const handleUpdate = () => {
    if (!editingTeam || !teamName.trim()) return;
    updateMutation.mutate({
      id: editingTeam.id,
      data: {
        name: teamName.trim(),
        description: teamDescription.trim(),
        tag: teamTag.trim().toLowerCase() || 'general',
        is_active: teamIsActive,
      },
    });
  };

  const handleDelete = async (team: TeamWithCount) => {
    const ok = await confirmDialog({
      description: t('teams.deleteConfirm', { name: team.name }),
      variant: 'danger',
    });
    if (ok) {
      deleteMutation.mutate(team.id);
    }
  };

  const selectedTeam = teams.find((t) => t.id === selectedTeamId);

  // Unique tags from existing teams for suggestions
  const existingTags = [...new Set(teams.map((t) => t.tag))].sort();

  // Membership is the one thing the server cannot filter on, so it stays here.
  const memberIds = new Set(members.map((m) => m.employee_id));
  const availableEmployees = (employeesData?.data || []).filter((e) => !memberIds.has(e.id));
  const employeeTotal = employeesData?.total ?? 0;
  const searchTruncated = employeeTotal > (employeesData?.data.length ?? 0);

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('teams.title')}
        subtitle={t('teams.subtitle')}
        actions={
          canManageTeams ? (
            <button type="button" onClick={openCreate} className="btn-primary w-full sm:w-auto">
              <Plus className="w-4 h-4" aria-hidden="true" />
              {t('teams.newTeam')}
            </button>
          ) : undefined
        }
      />

      {teamsError ? (
        <div className="card">
          <EmptyState
            icon={AlertTriangle}
            title={t('teams.loadFailed')}
            description={getErrorMessage(teamsErrorObj, t('teams.loadFailed'))}
            action={
              <button type="button" onClick={() => refetchTeams()} className="btn-primary">
                {t('common.retry')}
              </button>
            }
          />
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Teams List */}
          <div className="lg:col-span-1">
            <div className="card !p-0 overflow-hidden">
              <div className="px-5 py-4 border-b border-gray-100 dark:border-slate-800">
                <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100 tabular-nums">
                  {t('teams.allTeams', { count: teams.length })}
                </h2>
              </div>
              {isLoading ? (
                <div
                  role="status"
                  aria-busy="true"
                  aria-label={t('common.loading')}
                  className="p-4 space-y-3"
                >
                  {[0, 1, 2, 3].map((i) => (
                    <div key={i} className="flex items-center gap-3">
                      <Skeleton className="h-9 w-9 rounded-xl shrink-0" />
                      <div className="flex-1 space-y-1.5">
                        <Skeleton className="h-4 w-2/3" />
                        <Skeleton className="h-3 w-1/2" />
                      </div>
                    </div>
                  ))}
                </div>
              ) : teams.length === 0 ? (
                <div className="p-4">
                  <EmptyState compact icon={Users} title={t('teams.empty')} />
                </div>
              ) : (
                <div className="divide-y divide-gray-100 dark:divide-slate-800">
                  {teams.map((team) => {
                    const tagStyle = getTagStyle(team.tag);
                    const isSelected = selectedTeamId === team.id;
                    return (
                      <button
                        type="button"
                        key={team.id}
                        aria-current={isSelected ? 'true' : undefined}
                        onClick={() => handleSelectTeam(team.id)}
                        className={`min-h-11 w-full text-left flex items-center justify-between gap-2 px-5 py-3.5 transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--ring)] ${
                          isSelected
                            ? 'bg-gray-100 dark:bg-slate-800'
                            : 'hover:bg-gray-50 dark:hover:bg-slate-800/60'
                        }`}
                      >
                        <div className="flex items-center gap-3 min-w-0">
                          <div className={`w-9 h-9 rounded-xl flex items-center justify-center text-white shrink-0 ${
                            team.is_active ? tagStyle.icon : 'bg-gray-300 dark:bg-slate-700'
                          }`}>
                            <Users className="w-4 h-4" aria-hidden="true" />
                          </div>
                          <div className="min-w-0">
                            <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{team.name}</p>
                            <p className="text-xs text-gray-500 dark:text-slate-400 truncate tabular-nums">
                              {t('teams.memberCount', { count: team.member_count || 0 })}
                              <span className={`ml-1.5 ${tagStyle.text}`}>
                                {team.tag}
                              </span>
                              {!team.is_active && <span className="ml-1 text-amber-600 dark:text-amber-400">{t('teams.inactiveParen')}</span>}
                            </p>
                          </div>
                        </div>
                        <ChevronRight className={`w-4 h-4 shrink-0 ${isSelected ? 'text-gray-900 dark:text-slate-100' : 'text-gray-400 dark:text-slate-500'}`} aria-hidden="true" />
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          {/* Team Detail & Members */}
          <div ref={detailRef} className="lg:col-span-2">
            {selectedTeam ? (
              <div className="space-y-4">
                {/* Team Info */}
                <div className="card">
                  <div className="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
                    <div className="min-w-0">
                      <h2 className="text-lg font-semibold text-gray-900 dark:text-slate-100 break-words">{selectedTeam.name}</h2>
                      {selectedTeam.description && (
                        <p className="text-sm text-gray-500 dark:text-slate-400 mt-1 break-words">{selectedTeam.description}</p>
                      )}
                      <div className="flex flex-wrap items-center gap-2.5 mt-2.5">
                        {(() => {
                          const tagStyle = getTagStyle(selectedTeam.tag);
                          return (
                            <span className={`text-xs px-2 py-0.5 rounded-full font-medium inline-flex items-center gap-1 ${tagStyle.bg} ${tagStyle.text}`}>
                              <Tag className="w-3 h-3" aria-hidden="true" />
                              {selectedTeam.tag}
                            </span>
                          );
                        })()}
                        <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${
                          selectedTeam.is_active
                            ? 'bg-green-100 text-green-700 dark:bg-emerald-500/15 dark:text-emerald-300'
                            : 'bg-gray-100 text-gray-600 dark:bg-slate-800 dark:text-slate-400'
                        }`}>
                          {selectedTeam.is_active ? t('common.active') : t('common.inactive')}
                        </span>
                        <span className="text-xs text-gray-500 dark:text-slate-400 tabular-nums">
                          {t('teams.memberCount', { count: selectedTeam.member_count || 0 })}
                        </span>
                      </div>
                    </div>
                    {canManageTeams && (
                      <div className="flex flex-wrap items-center gap-2 shrink-0">
                        <button
                          type="button"
                          onClick={() => openEdit(selectedTeam)}
                          className="btn-secondary min-h-11 sm:!min-h-9 !py-2 !px-3.5 text-sm"
                        >
                          {t('common.edit')}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleDelete(selectedTeam)}
                          disabled={deleteMutation.isPending}
                          className="min-h-11 sm:min-h-9 inline-flex items-center justify-center gap-1.5 rounded-xl px-3.5 py-2 text-sm font-medium border border-red-200 dark:border-red-500/30 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                        >
                          {deleteMutation.isPending && (
                            <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                          )}
                          {deleteMutation.isPending ? t('common.deleting') : t('common.delete')}
                        </button>
                      </div>
                    )}
                  </div>
                </div>

                {/* Members */}
                <div className="card !p-0 overflow-hidden">
                  <div className="px-5 sm:px-6 py-4 border-b border-gray-100 dark:border-slate-800 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                    <h3 className="text-sm font-semibold text-gray-900 dark:text-slate-100">{t('teams.members')}</h3>
                    {canManageTeams && (
                      <button
                        type="button"
                        onClick={() => { setMemberSearch(''); setShowAddMemberModal(true); }}
                        className="btn-primary min-h-11 sm:!min-h-9 !py-2 !px-3.5 text-sm w-full sm:w-auto"
                      >
                        <UserPlus className="w-3.5 h-3.5" aria-hidden="true" />
                        {t('teams.addMember')}
                      </button>
                    )}
                  </div>
                  {loadingMembers ? (
                    <div
                      role="status"
                      aria-busy="true"
                      aria-label={t('common.loading')}
                      className="p-5 sm:p-6 space-y-3"
                    >
                      {[0, 1, 2].map((i) => (
                        <div key={i} className="flex items-center justify-between gap-3">
                          <div className="flex items-center gap-3 flex-1">
                            <Skeleton className="h-8 w-8 rounded-full shrink-0" />
                            <div className="flex-1 space-y-1.5">
                              <Skeleton className="h-4 w-1/3" />
                              <Skeleton className="h-3 w-1/2" />
                            </div>
                          </div>
                          <Skeleton className="h-6 w-16 rounded-full" />
                        </div>
                      ))}
                    </div>
                  ) : membersError ? (
                    <div className="p-6">
                      <EmptyState
                        icon={AlertTriangle}
                        title={t('teams.membersLoadFailed')}
                        description={getErrorMessage(membersErrorObj, t('teams.membersLoadFailed'))}
                        action={
                          <button type="button" onClick={() => refetchMembers()} className="btn-primary">
                            {t('common.retry')}
                          </button>
                        }
                      />
                    </div>
                  ) : members.length === 0 ? (
                    <div className="p-6">
                      <EmptyState compact icon={Users} title={t('teams.noMembers')} />
                    </div>
                  ) : (
                    <div className="divide-y divide-gray-100 dark:divide-slate-800">
                      {members.map((member) => (
                        <MemberRow
                          key={member.id}
                          member={member}
                          canManage={canManageTeams}
                          onRemove={async () => {
                            const ok = await confirmDialog({
                              description: t('teams.removeMemberConfirm', {
                                name: member.employee_name || t('common.unknown'),
                              }),
                              confirmLabel: t('common.remove'),
                              variant: 'danger',
                            });
                            if (ok) {
                              removeMemberMutation.mutate({
                                teamId: selectedTeamId!,
                                employeeId: member.employee_id,
                              });
                            }
                          }}
                          isRemoving={removingEmployeeId === member.employee_id && removeMemberMutation.isPending}
                        />
                      ))}
                    </div>
                  )}
                </div>
              </div>
            ) : (
              <div className="card">
                <EmptyState compact icon={Users} title={t('teams.selectTeam')} />
              </div>
            )}
          </div>
        </div>
      )}

      {/* Create Team Modal */}
      <Modal
        open={showCreateModal}
        onClose={() => {
          if (!createMutation.isPending) {
            setShowCreateModal(false);
          }
        }}
        title={t('teams.createTitle')}
        footer={
          <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
            <button
              type="button"
              onClick={() => setShowCreateModal(false)}
              disabled={createMutation.isPending}
              className="btn-secondary"
            >
              {t('common.cancel')}
            </button>
            <button type="submit" form={createFormId} disabled={!teamName.trim() || createMutation.isPending} className="btn-primary">
              {createMutation.isPending && <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />}
              {createMutation.isPending ? t('common.creating') : t('teams.createTitle')}
            </button>
          </div>
        }
      >
        <form
          id={createFormId}
          onSubmit={(e) => {
            e.preventDefault();
            handleCreate();
          }}
          className="space-y-4"
        >
          <FormField label={t('teams.teamName')} required>
            {(aria) => (
              <input
                {...aria}
                type="text"
                value={teamName}
                onChange={(e) => setTeamName(e.target.value)}
                className="form-input"
                placeholder={t('teams.teamNamePlaceholder')}
                required
              />
            )}
          </FormField>
          <FormField label={t('common.description')}>
            {(aria) => (
              <textarea
                {...aria}
                value={teamDescription}
                onChange={(e) => setTeamDescription(e.target.value)}
                className="form-input"
                rows={3}
                placeholder={t('teams.descriptionPlaceholder')}
              />
            )}
          </FormField>
          <div>
            <FormField label={t('teams.tag')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={teamTag}
                  onChange={(e) => setTeamTag(e.target.value)}
                  className="form-input"
                  placeholder={t('teams.tagPlaceholder')}
                />
              )}
            </FormField>
            {existingTags.length > 0 && (
              <div className="flex flex-wrap gap-1.5 mt-2">
                {existingTags.map((tag) => {
                  const style = getTagStyle(tag);
                  return (
                    <button
                      key={tag}
                      type="button"
                      aria-pressed={teamTag === tag}
                      onClick={() => setTeamTag(tag)}
                      className={`min-h-11 sm:min-h-0 text-xs px-3 py-1.5 sm:py-1 rounded-full font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
                        teamTag === tag ? `${style.bg} ${style.text} ring-2 ring-offset-1 dark:ring-offset-slate-900 ring-gray-400 dark:ring-slate-500` : `${style.bg} ${style.text} opacity-75 hover:opacity-100`
                      }`}
                    >
                      {tag}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
          {createMutation.isError && (
            <p role="alert" className="text-sm text-red-600 dark:text-red-400">
              {getErrorMessage(createMutation.error, t('teams.createFailed'))}
            </p>
          )}
        </form>
      </Modal>

      {/* Edit Team Modal */}
      <Modal
        open={showEditModal}
        onClose={() => {
          if (!updateMutation.isPending) {
            setShowEditModal(false);
          }
        }}
        title={t('teams.editTitle')}
        footer={
          <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
            <button
              type="button"
              onClick={() => setShowEditModal(false)}
              disabled={updateMutation.isPending}
              className="btn-secondary"
            >
              {t('common.cancel')}
            </button>
            <button type="submit" form={editFormId} disabled={!teamName.trim() || updateMutation.isPending} className="btn-primary">
              {updateMutation.isPending && <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />}
              {updateMutation.isPending ? t('common.saving') : t('common.saveChanges')}
            </button>
          </div>
        }
      >
        <form
          id={editFormId}
          onSubmit={(e) => {
            e.preventDefault();
            handleUpdate();
          }}
          className="space-y-4"
        >
          <FormField label={t('teams.teamName')} required>
            {(aria) => (
              <input
                {...aria}
                type="text"
                value={teamName}
                onChange={(e) => setTeamName(e.target.value)}
                className="form-input"
                required
              />
            )}
          </FormField>
          <FormField label={t('common.description')}>
            {(aria) => (
              <textarea
                {...aria}
                value={teamDescription}
                onChange={(e) => setTeamDescription(e.target.value)}
                className="form-input"
                rows={3}
              />
            )}
          </FormField>
          <div>
            <FormField label={t('teams.tag')}>
              {(aria) => (
                <input
                  {...aria}
                  type="text"
                  value={teamTag}
                  onChange={(e) => setTeamTag(e.target.value)}
                  className="form-input"
                  placeholder={t('teams.tagPlaceholder')}
                />
              )}
            </FormField>
            {existingTags.length > 0 && (
              <div className="flex flex-wrap gap-1.5 mt-2">
                {existingTags.map((tag) => {
                  const style = getTagStyle(tag);
                  return (
                    <button
                      key={tag}
                      type="button"
                      aria-pressed={teamTag === tag}
                      onClick={() => setTeamTag(tag)}
                      className={`min-h-11 sm:min-h-0 text-xs px-3 py-1.5 sm:py-1 rounded-full font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)] ${
                        teamTag === tag ? `${style.bg} ${style.text} ring-2 ring-offset-1 dark:ring-offset-slate-900 ring-gray-400 dark:ring-slate-500` : `${style.bg} ${style.text} opacity-75 hover:opacity-100`
                      }`}
                    >
                      {tag}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
          <label className="min-h-11 sm:min-h-0 flex items-center gap-2.5 cursor-pointer">
            <input
              type="checkbox"
              checked={teamIsActive}
              onChange={(e) => setTeamIsActive(e.target.checked)}
              className="w-4 h-4 rounded border-gray-300 dark:border-slate-700 accent-[var(--accent-1)]"
            />
            <span className="text-sm text-gray-700 dark:text-slate-200">{t('common.active')}</span>
          </label>
          {updateMutation.isError && (
            <p role="alert" className="text-sm text-red-600 dark:text-red-400">
              {getErrorMessage(updateMutation.error, t('teams.updateFailed'))}
            </p>
          )}
        </form>
      </Modal>

      {/* Add Member Modal */}
      <Modal
        open={showAddMemberModal}
        onClose={() => {
          if (addingEmployeeIds.size === 0) {
            setShowAddMemberModal(false);
          }
        }}
        title={t('teams.addMemberTo', { name: selectedTeam?.name || t('teams.title') })}
        maxWidth="max-w-lg"
      >
        <div className="space-y-3">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500 dark:text-slate-400 pointer-events-none" aria-hidden="true" />
            <input
              type="text"
              value={memberSearch}
              onChange={(e) => setMemberSearch(e.target.value)}
              aria-label={t('teams.searchEmployees')}
              className="form-input pl-9 pr-11"
              placeholder={t('teams.searchEmployees')}
            />
            {memberSearch && (
              <button
                type="button"
                onClick={() => setMemberSearch('')}
                aria-label={t('common.clear')}
                className="absolute right-0.5 top-1/2 -translate-y-1/2 min-h-11 min-w-11 sm:min-h-9 sm:min-w-9 flex items-center justify-center rounded-md text-gray-500 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
              >
                <X className="w-4 h-4" aria-hidden="true" />
              </button>
            )}
          </div>
          {searchTruncated && (
            <p className="text-xs text-gray-500 dark:text-slate-400 tabular-nums">
              {t('employees.showingPartial', { shown: employeesData?.data.length, total: employeeTotal })}
            </p>
          )}
          <div className="max-h-80 overflow-y-auto divide-y divide-gray-100 dark:divide-slate-800 border border-gray-200 dark:border-slate-800 rounded-lg">
            {!canViewEmployees ? (
              <div className="p-6 text-center text-sm text-gray-500 dark:text-slate-400">
                {t('teams.employeeDirectoryRequired')}
              </div>
            ) : loadingAvailableEmployees ? (
              <div
                role="status"
                aria-busy="true"
                aria-label={t('common.loading')}
                className="p-4 space-y-3"
              >
                {[0, 1, 2].map((i) => (
                  <div key={i} className="flex items-center justify-between gap-3">
                    <div className="flex-1 space-y-1.5">
                      <Skeleton className="h-4 w-2/5" />
                      <Skeleton className="h-3 w-3/5" />
                    </div>
                    <Skeleton className="h-8 w-24 rounded-lg" />
                  </div>
                ))}
              </div>
            ) : availableEmployeesError ? (
              <div className="p-6 text-center space-y-2">
                <p role="alert" className="text-sm text-red-600 dark:text-red-400">
                  {getErrorMessage(availableEmployeesErrorObj, t('employees.searchFailed'))}
                </p>
                <button type="button" onClick={() => refetchAvailableEmployees()} className="btn-secondary !py-1.5 !px-3 text-xs">
                  {t('common.retry')}
                </button>
              </div>
            ) : availableEmployees.length === 0 ? (
              <div className="p-4">
                <EmptyState compact title={memberSearch ? t('employees.noMatch') : t('teams.allInTeam')} />
              </div>
            ) : (
              availableEmployees.map((emp) => {
                const isBusy = addingEmployeeIds.has(emp.id);
                return (
                  <div key={emp.id} className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 px-4 py-3 hover:bg-gray-50 dark:hover:bg-slate-800/60">
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{emp.full_name}</p>
                      <p className="text-xs text-gray-500 dark:text-slate-400 truncate">
                        {emp.employee_number}
                        {emp.department ? ` \u2022 ${emp.department}` : ''}
                        {emp.designation ? ` \u2022 ${emp.designation}` : ''}
                      </p>
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <button
                        type="button"
                        onClick={() => {
                          addMemberMutation.mutate({
                            teamId: selectedTeamId!,
                            employeeId: emp.id,
                            role: 'lead',
                          });
                        }}
                        disabled={isBusy}
                        className="min-h-11 sm:min-h-8 text-xs px-3 py-2 sm:py-1.5 rounded-lg border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                      >
                        {t('teams.addAsLead')}
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          addMemberMutation.mutate({
                            teamId: selectedTeamId!,
                            employeeId: emp.id,
                            role: 'member',
                          });
                        }}
                        disabled={isBusy}
                        className="min-h-11 sm:min-h-8 inline-flex items-center gap-1 text-xs px-3 py-2 sm:py-1.5 rounded-lg bg-gray-900 dark:bg-slate-100 text-white dark:text-slate-900 hover:bg-gray-800 dark:hover:bg-white disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
                      >
                        {isBusy && <Loader2 className="w-3 h-3 animate-spin" aria-hidden="true" />}
                        {t('common.add')}
                      </button>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>
      </Modal>
    </div>
  );
}

function MemberRow({
  member,
  canManage,
  onRemove,
  isRemoving,
}: {
  member: TeamMember;
  canManage: boolean;
  onRemove: () => void;
  isRemoving: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-between gap-3 px-5 sm:px-6 py-3">
      <div className="flex items-center gap-3 min-w-0">
        <div className="w-8 h-8 rounded-full bg-gray-100 dark:bg-slate-800 flex items-center justify-center text-xs font-bold text-gray-600 dark:text-slate-300 shrink-0">
          {(member.employee_name || '?')[0]}
        </div>
        <div className="min-w-0">
          <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{member.employee_name || t('common.unknown')}</p>
          <p className="text-xs text-gray-500 dark:text-slate-400 truncate">
            {member.employee_number}
            {member.department ? ` \u2022 ${member.department}` : ''}
            {member.designation ? ` \u2022 ${member.designation}` : ''}
          </p>
        </div>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${
          member.role === 'lead'
            ? 'bg-amber-100 text-amber-700 dark:bg-amber-500/15 dark:text-amber-300'
            : 'bg-gray-100 text-gray-600 dark:bg-slate-800 dark:text-slate-300'
        }`}>
          {t(`teams.roles.${member.role}`, { defaultValue: member.role })}
        </span>
        {canManage && (
          <button
            type="button"
            onClick={onRemove}
            disabled={isRemoving}
            className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg text-gray-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 disabled:opacity-50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
            aria-label={t('teams.removeMember')}
            title={t('teams.removeMember')}
          >
            {isRemoving ? (
              <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
            ) : (
              <Trash2 className="w-4 h-4" aria-hidden="true" />
            )}
          </button>
        )}
      </div>
    </div>
  );
}
