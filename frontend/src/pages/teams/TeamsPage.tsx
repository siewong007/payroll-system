import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { AlertTriangle, Plus, Users, Trash2, UserPlus, ChevronRight, Search, Tag, X } from 'lucide-react';
import { EmptyState } from '@/components/ui/EmptyState';
import { FormField } from '@/components/ui/FormField';
import { Modal } from '@/components/ui/Modal';
import { PageHeader } from '@/components/ui/PageHeader';
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
    bg: 'bg-purple-100 dark:bg-purple-500/15',
    text: 'text-purple-700 dark:text-purple-300',
    icon: 'bg-purple-600 dark:bg-purple-500',
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
  const [addingEmployeeId, setAddingEmployeeId] = useState<string | null>(null);

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
    enabled: showAddMemberModal && canManageTeams,
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
    onMutate: ({ employeeId }) => setAddingEmployeeId(employeeId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-members', selectedTeamId] });
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      toast.success(t('teams.memberAdded'));
    },
    onError: (err) => toast.error(getErrorMessage(err, t('teams.addMemberFailed'))),
    onSettled: () => setAddingEmployeeId(null),
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
        description: teamDescription.trim() || undefined,
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
              <Plus className="w-4 h-4" />
              {t('teams.newTeam')}
            </button>
          ) : undefined
        }
      />

      {teamsError ? (
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
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Teams List */}
          <div className="lg:col-span-1">
            <div className="bg-white dark:bg-slate-900 rounded-2xl border border-gray-200 dark:border-slate-800 overflow-hidden">
              <div className="px-5 py-4 border-b border-gray-100 dark:border-slate-800">
                <h2 className="text-sm font-semibold text-gray-900 dark:text-slate-100">{t('teams.allTeams', { count: teams.length })}</h2>
              </div>
              {isLoading ? (
                <div className="p-8 text-center text-sm text-gray-400 dark:text-slate-500">{t('common.loading')}</div>
              ) : teams.length === 0 ? (
                <div className="p-8 text-center text-sm text-gray-400 dark:text-slate-500">
                  {t('teams.empty')}
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
                        className={`w-full text-left flex items-center justify-between gap-2 px-5 py-3.5 transition-all ${
                          isSelected
                            ? 'bg-gray-100 dark:bg-slate-800'
                            : 'hover:bg-gray-50 dark:hover:bg-slate-800/60'
                        }`}
                      >
                        <div className="flex items-center gap-3 min-w-0">
                          <div className={`w-9 h-9 rounded-xl flex items-center justify-center text-white shrink-0 ${
                            team.is_active ? tagStyle.icon : 'bg-gray-300 dark:bg-slate-700'
                          }`}>
                            <Users className="w-4 h-4" />
                          </div>
                          <div className="min-w-0">
                            <p className="text-sm font-medium text-gray-900 dark:text-slate-100 truncate">{team.name}</p>
                            <p className="text-xs text-gray-500 dark:text-slate-400 truncate">
                              {t('teams.memberCount', { count: team.member_count || 0 })}
                              <span className={`ml-1.5 ${tagStyle.text}`}>
                                {team.tag}
                              </span>
                              {!team.is_active && <span className="ml-1 text-amber-600 dark:text-amber-400">{t('teams.inactiveParen')}</span>}
                            </p>
                          </div>
                        </div>
                        <ChevronRight className={`w-4 h-4 shrink-0 ${isSelected ? 'text-gray-900 dark:text-slate-100' : 'text-gray-300 dark:text-slate-600'}`} />
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
                <div className="bg-white dark:bg-slate-900 rounded-2xl border border-gray-200 dark:border-slate-800 p-5 sm:p-6">
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
                              <Tag className="w-3 h-3" />
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
                        <span className="text-xs text-gray-500 dark:text-slate-400">
                          {t('teams.memberCount', { count: selectedTeam.member_count || 0 })}
                        </span>
                      </div>
                    </div>
                    {canManageTeams && (
                      <div className="flex flex-wrap items-center gap-2 shrink-0">
                        <button
                          type="button"
                          onClick={() => openEdit(selectedTeam)}
                          className="btn-secondary !py-1.5 !px-3 text-sm"
                        >
                          {t('common.edit')}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleDelete(selectedTeam)}
                          disabled={deleteMutation.isPending}
                          className="text-sm px-3 py-1.5 rounded-lg border border-red-200 dark:border-red-500/30 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 disabled:opacity-50 transition-colors"
                        >
                          {t('common.delete')}
                        </button>
                      </div>
                    )}
                  </div>
                </div>

                {/* Members */}
                <div className="bg-white dark:bg-slate-900 rounded-2xl border border-gray-200 dark:border-slate-800 overflow-hidden">
                  <div className="px-5 sm:px-6 py-4 border-b border-gray-100 dark:border-slate-800 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                    <h3 className="text-sm font-semibold text-gray-900 dark:text-slate-100">{t('teams.members')}</h3>
                    {canManageTeams && (
                      <button
                        type="button"
                        onClick={() => { setMemberSearch(''); setShowAddMemberModal(true); }}
                        className="btn-primary !py-2 !px-3 text-sm w-full sm:w-auto"
                      >
                        <UserPlus className="w-3.5 h-3.5" />
                        {t('teams.addMember')}
                      </button>
                    )}
                  </div>
                  {loadingMembers ? (
                    <div className="p-8 text-center text-sm text-gray-400 dark:text-slate-500">{t('common.loading')}</div>
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
                    <div className="p-8 text-center text-sm text-gray-400 dark:text-slate-500">
                      {t('teams.noMembers')}
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
              <div className="bg-white dark:bg-slate-900 rounded-2xl border border-gray-200 dark:border-slate-800 p-12 text-center">
                <Users className="w-12 h-12 text-gray-300 dark:text-slate-600 mx-auto mb-3" />
                <p className="text-sm text-gray-500 dark:text-slate-400">{t('teams.selectTeam')}</p>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Create Team Modal */}
      <Modal
        open={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        title={t('teams.createTitle')}
        footer={
          <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
            <button type="button" onClick={() => setShowCreateModal(false)} className="btn-secondary">{t('common.cancel')}</button>
            <button type="submit" form={createFormId} disabled={!teamName.trim() || createMutation.isPending} className="btn-primary">
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
                      onClick={() => setTeamTag(tag)}
                      className={`text-xs px-2.5 py-1 rounded-full font-medium transition-all ${
                        teamTag === tag ? `${style.bg} ${style.text} ring-2 ring-offset-1 dark:ring-offset-slate-900 ring-gray-400 dark:ring-slate-500` : `${style.bg} ${style.text} opacity-65 hover:opacity-100`
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
        onClose={() => setShowEditModal(false)}
        title={t('teams.editTitle')}
        footer={
          <div className="flex flex-col-reverse sm:flex-row justify-end gap-3">
            <button type="button" onClick={() => setShowEditModal(false)} className="btn-secondary">{t('common.cancel')}</button>
            <button type="submit" form={editFormId} disabled={!teamName.trim() || updateMutation.isPending} className="btn-primary">
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
                      onClick={() => setTeamTag(tag)}
                      className={`text-xs px-2.5 py-1 rounded-full font-medium transition-all ${
                        teamTag === tag ? `${style.bg} ${style.text} ring-2 ring-offset-1 dark:ring-offset-slate-900 ring-gray-400 dark:ring-slate-500` : `${style.bg} ${style.text} opacity-65 hover:opacity-100`
                      }`}
                    >
                      {tag}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={teamIsActive}
              onChange={(e) => setTeamIsActive(e.target.checked)}
              className="rounded border-gray-300 dark:border-slate-700"
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
        onClose={() => setShowAddMemberModal(false)}
        title={t('teams.addMemberTo', { name: selectedTeam?.name || t('teams.title') })}
        maxWidth="max-w-lg"
      >
        <div className="space-y-3">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 dark:text-slate-500 pointer-events-none" />
            <input
              type="text"
              value={memberSearch}
              onChange={(e) => setMemberSearch(e.target.value)}
              aria-label={t('teams.searchEmployees')}
              className="form-input !pl-9 !pr-10"
              placeholder={t('teams.searchEmployees')}
            />
            {memberSearch && (
              <button
                type="button"
                onClick={() => setMemberSearch('')}
                aria-label={t('common.clear')}
                className="absolute right-1 top-1/2 -translate-y-1/2 min-h-9 min-w-9 flex items-center justify-center rounded-md text-gray-400 dark:text-slate-500 hover:text-gray-700 dark:hover:text-slate-200"
              >
                <X className="w-4 h-4" />
              </button>
            )}
          </div>
          {searchTruncated && (
            <p className="text-xs text-gray-400 dark:text-slate-500">
              {t('employees.showingPartial', { shown: employeesData?.data.length, total: employeeTotal })}
            </p>
          )}
          <div className="max-h-80 overflow-y-auto divide-y divide-gray-100 dark:divide-slate-800 border border-gray-200 dark:border-slate-800 rounded-lg">
            {loadingAvailableEmployees ? (
              <div className="p-6 text-center text-sm text-gray-400 dark:text-slate-500">
                {t('common.loading')}
              </div>
            ) : availableEmployeesError ? (
              <div className="p-6 text-center space-y-2">
                <p className="text-sm text-red-600 dark:text-red-400">
                  {getErrorMessage(availableEmployeesErrorObj, t('employees.searchFailed'))}
                </p>
                <button type="button" onClick={() => refetchAvailableEmployees()} className="btn-secondary !py-1.5 !px-3 text-xs">
                  {t('common.retry')}
                </button>
              </div>
            ) : availableEmployees.length === 0 ? (
              <div className="p-6 text-center text-sm text-gray-400 dark:text-slate-500">
                {memberSearch ? t('employees.noMatch') : t('teams.allInTeam')}
              </div>
            ) : (
              availableEmployees.map((emp) => {
                const isBusy = addingEmployeeId === emp.id && addMemberMutation.isPending;
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
                        className="text-xs px-3 py-2 sm:py-1.5 rounded border border-gray-200 dark:border-slate-700 text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-50"
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
                        className="text-xs px-3 py-2 sm:py-1.5 rounded bg-gray-900 dark:bg-slate-100 text-white dark:text-slate-900 hover:bg-gray-800 dark:hover:bg-white disabled:opacity-50"
                      >
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
            className="min-h-11 min-w-11 sm:min-h-8 sm:min-w-8 flex items-center justify-center rounded-lg text-gray-400 dark:text-slate-500 hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/15 disabled:opacity-50 transition-colors"
            aria-label={t('teams.removeMember')}
            title={t('teams.removeMember')}
          >
            <Trash2 className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  );
}
