/**
 * WebAuthn browser helpers.
 *
 * The server (webauthn-rs) sends base64url-encoded binary fields.
 * The browser's navigator.credentials API needs ArrayBuffers.
 * After the browser ceremony we need to serialize back to JSON-safe objects.
 */

function base64urlToBuffer(b64: string): ArrayBuffer {
  const padding = '='.repeat((4 - (b64.length % 4)) % 4);
  const base64 = (b64 + padding).replace(/-/g, '+').replace(/_/g, '/');
  const raw = atob(base64);
  const buf = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) buf[i] = raw.charCodeAt(i);
  return buf.buffer;
}

function bufferToBase64url(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let binary = '';
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export interface PublicKeyCredentialCreationOptionsJSON extends Omit<PublicKeyCredentialCreationOptions, 'challenge' | 'user' | 'excludeCredentials'> {
  challenge: string;
  user: Omit<PublicKeyCredentialUserEntity, 'id'> & { id: string };
  excludeCredentials?: Array<Omit<PublicKeyCredentialDescriptor, 'id'> & { id: string }>;
}

export interface PublicKeyCredentialRequestOptionsJSON extends Omit<PublicKeyCredentialRequestOptions, 'challenge' | 'allowCredentials'> {
  challenge: string;
  allowCredentials?: Array<Omit<PublicKeyCredentialDescriptor, 'id'> & { id: string }>;
}

/** The serialized format we send to the server for registration */
export interface RegistrationResponseJSON {
  id: string;
  rawId: string;
  type: string;
  response: {
    attestationObject: string;
    clientDataJSON: string;
  };
}

/** The serialized format we send to the server for authentication */
export interface AuthenticationResponseJSON {
  id: string;
  rawId: string;
  type: string;
  response: {
    authenticatorData: string;
    clientDataJSON: string;
    signature: string;
    userHandle: string | null;
  };
}

/**
 * Convert server creation options → browser-ready options, call navigator.credentials.create,
 * then serialize the result back to JSON for the server.
 */
export async function createPasskeyCredential(
  publicKeyOptions: PublicKeyCredentialCreationOptionsJSON
): Promise<RegistrationResponseJSON> {
  // Convert base64url fields to ArrayBuffer for the browser API
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const publicKey: any = { ...publicKeyOptions };
  publicKey.challenge = base64urlToBuffer(publicKey.challenge);
  publicKey.user = {
    ...publicKey.user,
    id: base64urlToBuffer(publicKey.user.id),
  };
  if (publicKey.excludeCredentials) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    publicKey.excludeCredentials = publicKey.excludeCredentials.map((c: any) => ({
      ...c,
      id: base64urlToBuffer(c.id),
    }));
  }

  const credential = (await navigator.credentials.create({ publicKey })) as PublicKeyCredential;
  if (!credential) throw new Error('Passkey creation was cancelled');

  const response = credential.response as AuthenticatorAttestationResponse;
  return {
    id: credential.id,
    rawId: bufferToBase64url(credential.rawId),
    type: credential.type,
    response: {
      attestationObject: bufferToBase64url(response.attestationObject),
      clientDataJSON: bufferToBase64url(response.clientDataJSON),
    },
  };
}

/**
 * Convert server request options → browser-ready options, call navigator.credentials.get,
 * then serialize the result back to JSON for the server.
 */
export async function getPasskeyCredential(
  publicKeyOptions: PublicKeyCredentialRequestOptionsJSON
): Promise<AuthenticationResponseJSON> {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const publicKey: any = { ...publicKeyOptions };
  publicKey.challenge = base64urlToBuffer(publicKey.challenge);
  if (publicKey.allowCredentials) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    publicKey.allowCredentials = publicKey.allowCredentials.map((c: any) => ({
      ...c,
      id: base64urlToBuffer(c.id),
    }));
  }

  const credential = (await navigator.credentials.get({ publicKey })) as PublicKeyCredential;
  if (!credential) throw new Error('Passkey authentication was cancelled');

  const response = credential.response as AuthenticatorAssertionResponse;
  return {
    id: credential.id,
    rawId: bufferToBase64url(credential.rawId),
    type: credential.type,
    response: {
      authenticatorData: bufferToBase64url(response.authenticatorData),
      clientDataJSON: bufferToBase64url(response.clientDataJSON),
      signature: bufferToBase64url(response.signature),
      userHandle: response.userHandle ? bufferToBase64url(response.userHandle) : null,
    },
  };
}

/** Check if WebAuthn is supported in this browser */
export function isWebAuthnSupported(): boolean {
  return !!(window.PublicKeyCredential && navigator.credentials);
}
