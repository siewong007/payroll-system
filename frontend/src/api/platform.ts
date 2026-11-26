import api from './client';

export interface TwoFactorPolicyResponse {
  enforce: boolean;
  grace_until: string | null;
  non_compliant: string[];
}

export interface TwoFactorPolicyRequest {
  enforce: boolean;
  grace_until: string | null;
  actor_password?: string;
  actor_totp_code?: string;
}

export interface AdminJobHealth {
  last_run_at: string | null;
  ok: boolean | null;
  stale: boolean;
  last_error: string | null;
  processed: number | null;
}

export interface AdminJobsStatus {
  retention_cleanup: AdminJobHealth;
  auto_absent: AdminJobHealth;
  email_outbox: AdminJobHealth;
  push_outbox: AdminJobHealth;
}

export interface OperationalQueueCounts {
  email_outbox_pending: number;
  email_outbox_failed_24h: number;
  push_outbox_pending: number;
  push_outbox_failed_24h: number;
  background_jobs_failed_24h: number;
}

export interface PlatformHealthResponse {
  version: string;
  revision: string;
  latest_migration_version: number | null;
  migration_count: number;
  jobs: AdminJobsStatus;
  queues: OperationalQueueCounts;
}

export async function getTwoFactorPolicy(): Promise<TwoFactorPolicyResponse> {
  const { data } = await api.get<TwoFactorPolicyResponse>('/admin/platform/2fa-policy');
  return data;
}

export async function setTwoFactorPolicy(
  req: TwoFactorPolicyRequest,
): Promise<TwoFactorPolicyResponse> {
  const { data } = await api.put<TwoFactorPolicyResponse>('/admin/platform/2fa-policy', req);
  return data;
}

export async function getPlatformHealth(): Promise<PlatformHealthResponse> {
  const { data } = await api.get<PlatformHealthResponse>('/admin/platform/health');
  return data;
}
