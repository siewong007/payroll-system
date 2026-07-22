import api from './client';

export interface UserSession {
  id: string;
  user_agent: string | null;
  created_at: string;
  last_seen_at: string;
  expires_at: string;
  current: boolean;
}

export async function getSessions(): Promise<UserSession[]> {
  const { data } = await api.get('/auth/sessions');
  return data;
}

export async function revokeSession(sessionId: string): Promise<void> {
  await api.delete(`/auth/sessions/${sessionId}`);
}

export async function revokeOtherSessions(): Promise<void> {
  await api.delete('/auth/sessions/others');
}
