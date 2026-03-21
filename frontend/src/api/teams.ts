import api from './client';
import type { Team, TeamWithCount, TeamMember, CreateTeamRequest, UpdateTeamRequest, AddTeamMemberRequest } from '@/types';

export async function getTeams(): Promise<TeamWithCount[]> {
  const { data } = await api.get('/teams');
  return data;
}

export async function getTeam(id: string): Promise<Team> {
  const { data } = await api.get(`/teams/${id}`);
  return data;
}

export async function createTeam(req: CreateTeamRequest): Promise<Team> {
  const { data } = await api.post('/teams', req);
  return data;
}

export async function updateTeam(id: string, req: UpdateTeamRequest): Promise<Team> {
  const { data } = await api.put(`/teams/${id}`, req);
  return data;
}

export async function deleteTeam(id: string): Promise<void> {
  await api.delete(`/teams/${id}`);
}

export async function getTeamMembers(teamId: string): Promise<TeamMember[]> {
  const { data } = await api.get(`/teams/${teamId}/members`);
  return data;
}

export async function addTeamMember(teamId: string, req: AddTeamMemberRequest): Promise<TeamMember> {
  const { data } = await api.post(`/teams/${teamId}/members`, req);
  return data;
}

export async function removeTeamMember(teamId: string, employeeId: string): Promise<void> {
  await api.delete(`/teams/${teamId}/members/${employeeId}`);
}
