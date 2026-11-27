import api from '@/api/client';
import type { PermissionKey } from '@/api/permissions';
import type { StepUpCredentials } from '@/types';

export interface UserGroupBase {
  id: string;
  company_id: string;
  name: string;
  description: string | null;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface UserGroup extends UserGroupBase {
  permissions: PermissionKey[];
  member_count: number;
}

export interface UserGroupMember {
  group_id: string;
  user_id: string;
  added_at: string;
  full_name: string;
  email: string;
}

export interface CreateUserGroupRequest {
  name: string;
  description?: string;
  permissions: PermissionKey[];
  actor_password?: string;
  actor_totp_code?: string;
}

export interface UpdateUserGroupRequest {
  name?: string;
  description?: string;
  is_active?: boolean;
  /** Omit to leave the permission set untouched; send to replace it wholesale. */
  permissions?: PermissionKey[];
  actor_password?: string;
  actor_totp_code?: string;
}

export const userGroupsApi = {
  list: async (): Promise<UserGroup[]> => {
    const { data } = await api.get<UserGroup[]>('/user-groups');
    return data;
  },

  create: async (req: CreateUserGroupRequest): Promise<UserGroupBase> => {
    const { data } = await api.post<UserGroupBase>('/user-groups', req);
    return data;
  },

  update: async (id: string, req: UpdateUserGroupRequest): Promise<UserGroupBase> => {
    const { data } = await api.put<UserGroupBase>(`/user-groups/${id}`, req);
    return data;
  },

  remove: async (id: string, stepUp: StepUpCredentials): Promise<void> => {
    await api.delete(`/user-groups/${id}`, { data: stepUp });
  },

  members: async (id: string): Promise<UserGroupMember[]> => {
    const { data } = await api.get<UserGroupMember[]>(`/user-groups/${id}/members`);
    return data;
  },

  addMember: async (id: string, userId: string, stepUp: StepUpCredentials): Promise<void> => {
    await api.post(`/user-groups/${id}/members`, { user_id: userId, ...stepUp });
  },

  removeMember: async (id: string, userId: string): Promise<void> => {
    await api.delete(`/user-groups/${id}/members/${userId}`);
  },
};
