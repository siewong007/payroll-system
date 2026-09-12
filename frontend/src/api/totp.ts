import api from './client';
import type { LoginResponse } from '@/types';

export interface TotpSetupResponse {
  secret: string;
  otpauth_url: string;
  qr_code_base64: string;
}

export interface TotpConfirmResponse {
  backup_codes: string[];
}

// Management (authenticated)
export async function totpStatus(): Promise<{ enabled: boolean }> {
  const { data } = await api.get('/auth/2fa/status');
  return data;
}

export async function totpSetupBegin(): Promise<TotpSetupResponse> {
  const { data } = await api.post('/auth/2fa/setup/begin');
  return data;
}

export async function totpSetupConfirm(code: string): Promise<TotpConfirmResponse> {
  const { data } = await api.post('/auth/2fa/setup/confirm', { code });
  return data;
}

export async function totpDisable(password: string): Promise<void> {
  await api.post('/auth/2fa/disable', { password });
}

export async function totpRegenerateBackupCodes(password: string): Promise<TotpConfirmResponse> {
  const { data } = await api.post('/auth/2fa/backup-codes/regenerate', { password });
  return data;
}

// Login flow (unauthenticated — completes a pending 2FA challenge)
export async function verifyTwoFactorLogin(mfaToken: string, code: string): Promise<LoginResponse> {
  const { data } = await api.post('/auth/2fa/verify', { mfa_token: mfaToken, code });
  return data;
}

/**
 * Passwordless sign-in: email + a TOTP authenticator code or one-time backup
 * code as the whole credential. Only works for accounts with 2FA enabled —
 * everyone else gets the same generic rejection as a wrong code.
 */
export async function codeLogin(
  email: string,
  code: string,
  turnstileToken?: string,
): Promise<LoginResponse> {
  const { data } = await api.post('/auth/login/code', {
    email,
    code,
    turnstile_token: turnstileToken,
  });
  return data;
}
