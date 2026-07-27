import api from './client';

/** "none" — off. "learn" — observe only. "warn" — flag. "enforce" — block. */
export type NetworkMode = 'none' | 'learn' | 'warn' | 'enforce';

export interface CompanyNetwork {
  id: string;
  company_id: string;
  label: string;
  /** Canonical network address, host bits already cleared. */
  network: string;
  prefix_len: number;
  is_active: boolean;
  approved_by: string | null;
  approved_at: string;
  /** True when this entry began as a learned proposal rather than a typed-in block. */
  learned_from_observation: boolean;
  created_at: string;
  updated_at: string;
}

export interface NetworkCandidate {
  network: string;
  prefix_len: number;
  distinct_employees: number;
  observation_count: number;
  /** Observations corroborated by a kiosk-minted QR token or the geofence. */
  anchored_count: number;
  /**
   * Check-ins refused from this block. Never evidence for approving it — this
   * is how the office's new address surfaces after an ISP change, when
   * everyone is being turned away and nothing else is being recorded.
   */
  denied_count: number;
  first_seen_at: string;
  last_seen_at: string;
  is_anchored: boolean;
  is_proposable: boolean;
  /** Why it is not yet proposable. Null when it is. */
  blocked_reason: string | null;
}

export interface NetworkWhoami {
  /** The address the *server* resolved for this request. Null if none could be. */
  client_ip: string | null;
  /** The block the learner would record for this address — what to approve. */
  suggested_cidr: string | null;
  is_approved: boolean;
  matched_label: string | null;
  has_approved_networks: boolean;
  /**
   * Whether the API believes forwarded headers. A private `client_ip` while
   * this is true means the API is being reached without the proxy that is
   * supposed to sit in front of it — the one misconfiguration that would make
   * this control meaningless.
   */
  trust_proxy_headers: boolean;
}

/** Render a stored network as CIDR. */
export function toCidr(n: { network: string; prefix_len: number }): string {
  return `${n.network}/${n.prefix_len}`;
}

// ─── Mode ───

export function getNetworkMode(): Promise<{ mode: NetworkMode }> {
  return api.get('/attendance/networks/mode').then(r => r.data);
}

export function setNetworkMode(mode: NetworkMode): Promise<void> {
  return api.put('/attendance/networks/mode', { mode }).then(r => r.data);
}

// ─── Allow-list ───

export function listNetworks(): Promise<CompanyNetwork[]> {
  return api.get('/attendance/networks').then(r => r.data);
}

export function createNetwork(data: { label: string; cidr: string }): Promise<CompanyNetwork> {
  return api.post('/attendance/networks', data).then(r => r.data);
}

export function updateNetwork(
  id: string,
  data: { label?: string; is_active?: boolean }
): Promise<CompanyNetwork> {
  return api.put(`/attendance/networks/${id}`, data).then(r => r.data);
}

export function deleteNetwork(id: string): Promise<void> {
  return api.delete(`/attendance/networks/${id}`).then(r => r.data);
}

// ─── Learned candidates ───

export function listCandidates(): Promise<NetworkCandidate[]> {
  return api.get('/attendance/networks/candidates').then(r => r.data);
}

export function approveCandidate(data: { cidr: string; label: string }): Promise<CompanyNetwork> {
  return api.post('/attendance/networks/candidates/approve', data).then(r => r.data);
}

export function dismissCandidate(cidr: string): Promise<void> {
  return api.post('/attendance/networks/candidates/dismiss', { cidr }).then(r => r.data);
}

// ─── Diagnostics ───

export function getNetworkWhoami(): Promise<NetworkWhoami> {
  return api.get('/attendance/networks/whoami').then(r => r.data);
}
