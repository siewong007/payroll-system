import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Users, Trash2, UserPlus, ChevronRight, Search, Tag } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { getTeams, createTeam, updateTeam, deleteTeam, getTeamMembers, addTeamMember, removeTeamMember } from '@/api/teams';
import { getEmployees } from '@/api/employees';
import type { TeamWithCount, TeamMember, CreateTeamRequest, UpdateTeamRequest } from '@/types';

const TAG_COLORS: Record<string, { bg: string; text: string; icon: string }> = {
  engineering: { bg: 'bg-blue-100', text: 'text-blue-700', icon: 'bg-blue-600' },
  sales: { bg: 'bg-green-100', text: 'text-green-700', icon: 'bg-green-600' },
  marketing: { bg: 'bg-purple-100', text: 'text-purple-700', icon: 'bg-purple-600' },
  support: { bg: 'bg-orange-100', text: 'text-orange-700', icon: 'bg-orange-600' },
  corporate: { bg: 'bg-slate-100', text: 'text-slate-700', icon: 'bg-slate-600' },
  project: { bg: 'bg-pink-100', text: 'text-pink-700', icon: 'bg-pink-600' },
  general: { bg: 'bg-gray-100', text: 'text-gray-700', icon: 'bg-gray-500' },
};

function getTagStyle(tag: string) {
  return TAG_COLORS[tag.toLowerCase()] || TAG_COLORS.general;
}

export function TeamsPage() {
  const queryClient = useQueryClient();
  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showEditModal, setShowEditModal] = useState(false);
  const [showAddMemberModal, setShowAddMemberModal] = useState(false);
  const [editingTeam, setEditingTeam] = useState<TeamWithCount | null>(null);
  const [memberSearch, setMemberSearch] = useState('');

  // Form state
  const [teamName, setTeamName] = useState('');
  const [teamDescription, setTeamDescription] = useState('');
  const [teamTag, setTeamTag] = useState('general');
  const [teamIsActive, setTeamIsActive] = useState(true);

  const { data: teams = [], isLoading } = useQuery({
    queryKey: ['teams'],
    queryFn: getTeams,
  });

  const { data: members = [], isLoading: loadingMembers } = useQuery({
    queryKey: ['team-members', selectedTeamId],
    queryFn: () => getTeamMembers(selectedTeamId!),
    enabled: !!selectedTeamId,
  });

  const { data: employeesData } = useQuery({
    queryKey: ['employees', 'all'],
    queryFn: () => getEmployees({ per_page: 500, is_active: true }),
    enabled: showAddMemberModal,
  });

  const createMutation = useMutation({
    mutationFn: (data: CreateTeamRequest) => createTeam(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      setShowCreateModal(false);
      resetForm();
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateTeamRequest }) => updateTeam(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      setShowEditModal(false);
      resetForm();
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteTeam(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['teams'] });
      if (selectedTeamId) setSelectedTeamId(null);
    },
  });

  const addMemberMutation = useMutation({
    mutationFn: ({ teamId, employeeId, role }: { teamId: string; employeeId: string; role: string }) =>
      addTeamMember(teamId, { employee_id: employeeId, role }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-members', selectedTeamId] });
      queryClient.invalidateQueries({ queryKey: ['teams'] });
    },
  });

  const removeMemberMutation = useMutation({
    mutationFn: ({ teamId, employeeId }: { teamId: string; employeeId: string }) =>
      removeTeamMember(teamId, employeeId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-members', selectedTeamId] });
      queryClient.invalidateQueries({ queryKey: ['teams'] });
    },
  });

  const resetForm = () => {
    setTeamName('');
    setTeamDescription('');
    setTeamTag('general');
    setTeamIsActive(true);
    setEditingTeam(null);
  };

  const openCreate = () => {
    resetForm();
    setShowCreateModal(true);
  };

  const openEdit = (team: TeamWithCount) => {
    setEditingTeam(team);
    setTeamName(team.name);
    setTeamDescription(team.description || '');
    setTeamTag(team.tag);
    setTeamIsActive(team.is_active);
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

  const handleDelete = (team: TeamWithCount) => {
    if (confirm(`Delete team "${team.name}"? All members will be removed.`)) {
      deleteMutation.mutate(team.id);
    }
  };

  const selectedTeam = teams.find((t) => t.id === selectedTeamId);

  // Unique tags from existing teams for suggestions
  const existingTags = [...new Set(teams.map((t) => t.tag))].sort();

  // Filter employees not already in the team
  const memberIds = new Set(members.map((m) => m.employee_id));
  const availableEmployees = (employeesData?.data || []).filter(
    (e) => !memberIds.has(e.id) && (!memberSearch || e.full_name.toLowerCase().includes(memberSearch.toLowerCase()) || e.employee_number.toLowerCase().includes(memberSearch.toLowerCase()))
  );

  return (
    <div className="space-y-6">
      <div className="page-header flex items-center justify-between">
        <div>
          <h1 className="page-title">Teams</h1>
          <p className="page-subtitle">Organize employees into teams</p>
        </div>
        <button onClick={openCreate} className="btn-primary flex items-center gap-2">
          <Plus className="w-4 h-4" />
          New Team
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Teams List */}
        <div className="lg:col-span-1">
          <div className="bg-white rounded-2xl border border-gray-200">
            <div className="px-5 py-4 border-b border-gray-100">
              <h3 className="text-sm font-semibold text-gray-900">All Teams ({teams.length})</h3>
            </div>
            {isLoading ? (
              <div className="p-8 text-center text-sm text-gray-400">Loading...</div>
            ) : teams.length === 0 ? (
              <div className="p-8 text-center text-sm text-gray-400">
                No teams yet. Create one to get started.
              </div>
            ) : (
              <div className="divide-y divide-gray-100">
                {teams.map((team) => {
                  const tagStyle = getTagStyle(team.tag);
                  return (
                    <div
                      key={team.id}
                      onClick={() => setSelectedTeamId(team.id)}
                      className={`flex items-center justify-between px-5 py-3.5 cursor-pointer transition-all ${
                        selectedTeamId === team.id ? 'bg-gray-50' : 'hover:bg-gray-50'
                      }`}
                    >
                      <div className="flex items-center gap-3 min-w-0">
                        <div className={`w-9 h-9 rounded-xl flex items-center justify-center text-white ${
                          team.is_active ? tagStyle.icon : 'bg-gray-300'
                        }`}>
                          <Users className="w-4 h-4" />
                        </div>
                        <div className="min-w-0">
                          <p className="text-sm font-medium text-gray-900 truncate">{team.name}</p>
                          <p className="text-xs text-gray-400">
                            {team.member_count || 0} member{(team.member_count || 0) !== 1 ? 's' : ''}
                            <span className={`ml-1.5 ${tagStyle.text}`}>
                              {team.tag}
                            </span>
                            {!team.is_active && <span className="ml-1 text-amber-500">(inactive)</span>}
                          </p>
                        </div>
                      </div>
                      <ChevronRight className={`w-4 h-4 shrink-0 ${selectedTeamId === team.id ? 'text-gray-900' : 'text-gray-300'}`} />
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* Team Detail & Members */}
        <div className="lg:col-span-2">
          {selectedTeam ? (
            <div className="space-y-4">
              {/* Team Info */}
              <div className="bg-white rounded-2xl border border-gray-200 p-6">
                <div className="flex items-start justify-between">
                  <div>
                    <h2 className="text-lg font-semibold text-gray-900">{selectedTeam.name}</h2>
                    {selectedTeam.description && (
                      <p className="text-sm text-gray-500 mt-1">{selectedTeam.description}</p>
                    )}
                    <div className="flex items-center gap-3 mt-2">
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
                        selectedTeam.is_active ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-500'
                      }`}>
                        {selectedTeam.is_active ? 'Active' : 'Inactive'}
                      </span>
                      <span className="text-xs text-gray-400">
                        {selectedTeam.member_count || 0} members
                      </span>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => openEdit(selectedTeam)}
                      className="text-sm px-3 py-1.5 rounded-lg border border-gray-200 text-gray-700 hover:bg-gray-50"
                    >
                      Edit
                    </button>
                    <button
                      onClick={() => handleDelete(selectedTeam)}
                      className="text-sm px-3 py-1.5 rounded-lg border border-red-200 text-red-600 hover:bg-red-50"
                    >
                      Delete
                    </button>
                  </div>
                </div>
              </div>

              {/* Members */}
              <div className="bg-white rounded-2xl border border-gray-200">
                <div className="px-6 py-4 border-b border-gray-100 flex items-center justify-between">
                  <h3 className="text-sm font-semibold text-gray-900">Members</h3>
                  <button
                    onClick={() => { setMemberSearch(''); setShowAddMemberModal(true); }}
                    className="text-sm flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-gray-900 text-white hover:bg-gray-800"
                  >
                    <UserPlus className="w-3.5 h-3.5" />
                    Add Member
                  </button>
                </div>
                {loadingMembers ? (
                  <div className="p-8 text-center text-sm text-gray-400">Loading...</div>
                ) : members.length === 0 ? (
                  <div className="p-8 text-center text-sm text-gray-400">
                    No members yet. Add employees to this team.
                  </div>
                ) : (
                  <div className="divide-y divide-gray-100">
                    {members.map((member) => (
                      <MemberRow
                        key={member.id}
                        member={member}
                        onRemove={() =>
                          removeMemberMutation.mutate({
                            teamId: selectedTeamId!,
                            employeeId: member.employee_id,
                          })
                        }
                        onChangeRole={(role) =>
                          addMemberMutation.mutate({
                            teamId: selectedTeamId!,
                            employeeId: member.employee_id,
                            role,
                          })
                        }
                        isRemoving={removeMemberMutation.isPending}
                      />
                    ))}
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="bg-white rounded-2xl border border-gray-200 p-12 text-center">
              <Users className="w-12 h-12 text-gray-300 mx-auto mb-3" />
              <p className="text-sm text-gray-400">Select a team to view details and manage members</p>
            </div>
          )}
        </div>
      </div>

      {/* Create Team Modal */}
      <Modal
        open={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        title="Create Team"
        footer={
          <div className="flex justify-end gap-3">
            <button onClick={() => setShowCreateModal(false)} className="btn-secondary">Cancel</button>
            <button onClick={handleCreate} disabled={!teamName.trim() || createMutation.isPending} className="btn-primary">
              {createMutation.isPending ? 'Creating...' : 'Create Team'}
            </button>
          </div>
        }
      >
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Team Name *</label>
            <input
              type="text"
              value={teamName}
              onChange={(e) => setTeamName(e.target.value)}
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900"
              placeholder="e.g. Backend Squad, Sales Team"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Description</label>
            <textarea
              value={teamDescription}
              onChange={(e) => setTeamDescription(e.target.value)}
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900"
              rows={3}
              placeholder="Optional description"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Tag</label>
            <input
              type="text"
              value={teamTag}
              onChange={(e) => setTeamTag(e.target.value)}
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900"
              placeholder="e.g. engineering, sales, project"
            />
            {existingTags.length > 0 && (
              <div className="flex flex-wrap gap-1.5 mt-2">
                {existingTags.map((tag) => {
                  const style = getTagStyle(tag);
                  return (
                    <button
                      key={tag}
                      type="button"
                      onClick={() => setTeamTag(tag)}
                      className={`text-xs px-2 py-0.5 rounded-full font-medium transition-all ${
                        teamTag === tag ? `${style.bg} ${style.text} ring-2 ring-offset-1 ring-gray-400` : `${style.bg} ${style.text} opacity-60 hover:opacity-100`
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
            <p className="text-sm text-red-600">{(createMutation.error as Error).message || 'Failed to create team'}</p>
          )}
        </div>
      </Modal>

      {/* Edit Team Modal */}
      <Modal
        open={showEditModal}
        onClose={() => setShowEditModal(false)}
        title="Edit Team"
        footer={
          <div className="flex justify-end gap-3">
            <button onClick={() => setShowEditModal(false)} className="btn-secondary">Cancel</button>
            <button onClick={handleUpdate} disabled={!teamName.trim() || updateMutation.isPending} className="btn-primary">
              {updateMutation.isPending ? 'Saving...' : 'Save Changes'}
            </button>
          </div>
        }
      >
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Team Name *</label>
            <input
              type="text"
              value={teamName}
              onChange={(e) => setTeamName(e.target.value)}
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Description</label>
            <textarea
              value={teamDescription}
              onChange={(e) => setTeamDescription(e.target.value)}
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900"
              rows={3}
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Tag</label>
            <input
              type="text"
              value={teamTag}
              onChange={(e) => setTeamTag(e.target.value)}
              className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900"
              placeholder="e.g. engineering, sales, project"
            />
            {existingTags.length > 0 && (
              <div className="flex flex-wrap gap-1.5 mt-2">
                {existingTags.map((tag) => {
                  const style = getTagStyle(tag);
                  return (
                    <button
                      key={tag}
                      type="button"
                      onClick={() => setTeamTag(tag)}
                      className={`text-xs px-2 py-0.5 rounded-full font-medium transition-all ${
                        teamTag === tag ? `${style.bg} ${style.text} ring-2 ring-offset-1 ring-gray-400` : `${style.bg} ${style.text} opacity-60 hover:opacity-100`
                      }`}
                    >
                      {tag}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="team-active"
              checked={teamIsActive}
              onChange={(e) => setTeamIsActive(e.target.checked)}
              className="rounded border-gray-300"
            />
            <label htmlFor="team-active" className="text-sm text-gray-700">Active</label>
          </div>
          {updateMutation.isError && (
            <p className="text-sm text-red-600">{(updateMutation.error as Error).message || 'Failed to update team'}</p>
          )}
        </div>
      </Modal>

      {/* Add Member Modal */}
      <Modal
        open={showAddMemberModal}
        onClose={() => setShowAddMemberModal(false)}
        title={`Add Member to ${selectedTeam?.name || 'Team'}`}
        maxWidth="max-w-lg"
      >
        <div className="space-y-3">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              type="text"
              value={memberSearch}
              onChange={(e) => setMemberSearch(e.target.value)}
              className="w-full border border-gray-300 rounded-lg pl-9 pr-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900"
              placeholder="Search employees..."
            />
          </div>
          <div className="max-h-80 overflow-y-auto divide-y divide-gray-100 border border-gray-200 rounded-lg">
            {availableEmployees.length === 0 ? (
              <div className="p-6 text-center text-sm text-gray-400">
                {memberSearch ? 'No matching employees' : 'All employees are already in this team'}
              </div>
            ) : (
              availableEmployees.map((emp) => (
                <div key={emp.id} className="flex items-center justify-between px-4 py-3 hover:bg-gray-50">
                  <div>
                    <p className="text-sm font-medium text-gray-900">{emp.full_name}</p>
                    <p className="text-xs text-gray-400">
                      {emp.employee_number}
                      {emp.department ? ` \u2022 ${emp.department}` : ''}
                      {emp.designation ? ` \u2022 ${emp.designation}` : ''}
                    </p>
                  </div>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => {
                        addMemberMutation.mutate({
                          teamId: selectedTeamId!,
                          employeeId: emp.id,
                          role: 'lead',
                        });
                      }}
                      disabled={addMemberMutation.isPending}
                      className="text-xs px-2 py-1 rounded border border-gray-200 text-gray-600 hover:bg-gray-100"
                    >
                      Add as Lead
                    </button>
                    <button
                      onClick={() => {
                        addMemberMutation.mutate({
                          teamId: selectedTeamId!,
                          employeeId: emp.id,
                          role: 'member',
                        });
                      }}
                      disabled={addMemberMutation.isPending}
                      className="text-xs px-2.5 py-1 rounded bg-gray-900 text-white hover:bg-gray-800"
                    >
                      Add
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </Modal>
    </div>
  );
}

function MemberRow({
  member,
  onRemove,
  onChangeRole: _onChangeRole,
  isRemoving,
}: {
  member: TeamMember;
  onRemove: () => void;
  onChangeRole: (role: string) => void;
  isRemoving: boolean;
}) {
  return (
    <div className="flex items-center justify-between px-6 py-3">
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center text-xs font-bold text-gray-600">
          {(member.employee_name || '?')[0]}
        </div>
        <div>
          <p className="text-sm font-medium text-gray-900">{member.employee_name || 'Unknown'}</p>
          <p className="text-xs text-gray-400">
            {member.employee_number}
            {member.department ? ` \u2022 ${member.department}` : ''}
            {member.designation ? ` \u2022 ${member.designation}` : ''}
          </p>
        </div>
      </div>
      <div className="flex items-center gap-3">
        <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${
          member.role === 'lead' ? 'bg-amber-100 text-amber-700' : 'bg-gray-100 text-gray-600'
        }`}>
          {member.role}
        </span>
        <button
          onClick={onRemove}
          disabled={isRemoving}
          className="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50 transition-colors"
          title="Remove member"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
