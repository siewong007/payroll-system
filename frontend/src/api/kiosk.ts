import api from './client';
import type { QrTokenResponse } from './attendance';
import type { StepUpCredentials } from '@/types';

export interface KioskCredential {
  id: string;
  company_id: string;
  label: string;
  token_prefix: string;
  created_by: string;
  created_at: string;
  last_used_at: string | null;
  last_used_ip: string | null;
  revoked_at: string | null;
}

export interface CreateKioskCredentialResponse {
  credential: KioskCredential;
  /** Plaintext secret. Only returned at creation time. Show once and discard. */
  secret: string;
  public_url: string;
}

export function listKioskCredentials(): Promise<KioskCredential[]> {
  return api.get('/attendance/kiosks').then((r) => r.data);
}

export function createKioskCredential(
  label: string,
  stepUp: StepUpCredentials,
): Promise<CreateKioskCredentialResponse> {
  return api.post('/attendance/kiosks', { label, ...stepUp }).then((r) => r.data);
}

export function revokeKioskCredential(id: string, stepUp: StepUpCredentials): Promise<void> {
  return api.delete(`/attendance/kiosks/${id}`, { data: stepUp }).then(() => undefined);
}

/**
 * Public kiosk QR fetch. Sends the kiosk secret in `Authorization: Kiosk <secret>`.
 * The user-Bearer interceptor in `client.ts` only sets the header when no Authorization
 * is present (we override here), so the user JWT is never sent on this call.
 *
 * Returns the same shape as the authenticated `generateQrToken`.
 */
export interface KioskCheckInPayload {
  personal_token: string;
  photo_base64?: string;
}

export interface KioskCheckInResponse {
  action: 'check_in' | 'check_out' | string;
  employee_name: string;
  employee_number: string;
  timestamp: string;
  status: string;
  photo_captured: boolean;
}

export function fetchKioskQr(kioskKey: string): Promise<QrTokenResponse> {
  return api
    .post('/attendance/kiosk/qr', null, {
      headers: { Authorization: `Kiosk ${kioskKey}` },
    })
    .then((r) => r.data);
}

export function submitKioskCheckIn(
  kioskKey: string,
  payload: KioskCheckInPayload,
): Promise<KioskCheckInResponse> {
  return api
    .post('/attendance/kiosk/check-in', payload, {
      headers: { Authorization: `Kiosk ${kioskKey}` },
    })
    .then((r) => r.data);
}
