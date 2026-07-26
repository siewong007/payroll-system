import api from './client';
import type {
  RegistrationResponseJSON,
  AuthenticationResponseJSON,
  PublicKeyCredentialCreationOptionsJSON,
  PublicKeyCredentialRequestOptionsJSON
} from '@/lib/webauthn';
import type { LoginResponse, MfaRequiredResponse } from '@/types';

export interface PasskeyInfo {
  id: string;
  credential_name: string;
  created_at: string;
  last_used_at: string | null;
}

// Check if email has passkeys registered
export async function checkPasskey(email: string): Promise<{ has_passkey: boolean }> {
  const { data } = await api.post('/auth/passkey/check', { email });
  return data;
}

/**
 * webauthn-rs serializes `RequestChallengeResponse` / `CreationChallengeResponse`
 * as `{ publicKey: { ... } }` (plus an optional sibling `mediation`), so the wire
 * shape is a wrapper — not the bare options object the browser API and our
 * `createPasskeyCredential`/`getPasskeyCredential` helpers take. Callers must
 * unwrap `.publicKey`; typing these as the bare options let a caller pass the
 * wrapper straight through and still typecheck.
 */
export interface PasskeyRequestOptionsEnvelope {
  publicKey: PublicKeyCredentialRequestOptionsJSON;
}

export interface PasskeyCreationOptionsEnvelope {
  publicKey: PublicKeyCredentialCreationOptionsJSON;
}

// Authentication (login) flow
export async function passkeyAuthBegin(email: string) {
  const { data } = await api.post('/auth/passkey/authenticate/begin', { email });
  return data as { challenge_id: string; options: PasskeyRequestOptionsEnvelope };
}

export async function passkeyAuthComplete(
  challengeId: string,
  credential: AuthenticationResponseJSON,
): Promise<LoginResponse | MfaRequiredResponse> {
  const { data } = await api.post('/auth/passkey/authenticate/complete', {
    challenge_id: challengeId,
    credential,
  });
  return data;
}

// Discoverable authentication (no email required)
export async function passkeyDiscoverableBegin() {
  const { data } = await api.post('/auth/passkey/discoverable/begin');
  return data as { challenge_id: string; options: PasskeyRequestOptionsEnvelope };
}

export async function passkeyDiscoverableComplete(
  challengeId: string,
  credential: AuthenticationResponseJSON,
): Promise<LoginResponse | MfaRequiredResponse> {
  const { data } = await api.post('/auth/passkey/discoverable/complete', {
    challenge_id: challengeId,
    credential,
  });
  return data;
}

// Registration flow (authenticated)
export async function passkeyRegisterBegin() {
  const { data } = await api.post('/auth/passkey/register/begin');
  return data as { challenge_id: string; options: PasskeyCreationOptionsEnvelope };
}

export async function passkeyRegisterComplete(challengeId: string, credential: RegistrationResponseJSON, name?: string) {
  const { data } = await api.post('/auth/passkey/register/complete', {
    challenge_id: challengeId,
    credential,
    name,
  });
  return data;
}

// Management (authenticated)
export async function listPasskeys(): Promise<PasskeyInfo[]> {
  const { data } = await api.get('/auth/passkeys');
  return data;
}

export async function renamePasskey(id: string, name: string): Promise<void> {
  await api.put(`/auth/passkeys/${id}`, { name });
}

export async function deletePasskey(id: string): Promise<void> {
  await api.delete(`/auth/passkeys/${id}`);
}
