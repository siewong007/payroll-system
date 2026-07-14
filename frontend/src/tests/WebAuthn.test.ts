import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createPasskeyCredential,
  getPasskeyCredential,
  isWebAuthnSupported,
  type PublicKeyCredentialCreationOptionsJSON,
  type PublicKeyCredentialRequestOptionsJSON,
} from '@/lib/webauthn';

const createCredential = vi.fn();
const getCredential = vi.fn();

function buffer(...values: number[]): ArrayBuffer {
  return new Uint8Array(values).buffer;
}

function bytes(value: BufferSource): number[] {
  if (value instanceof ArrayBuffer) return Array.from(new Uint8Array(value));
  return Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
}

describe('WebAuthn helpers', () => {
  beforeEach(() => {
    createCredential.mockReset();
    getCredential.mockReset();
    Object.defineProperty(navigator, 'credentials', {
      configurable: true,
      value: { create: createCredential, get: getCredential },
    });
    Object.defineProperty(window, 'PublicKeyCredential', {
      configurable: true,
      value: class PublicKeyCredential {},
    });
  });

  it('decodes registration options and serializes the created credential as base64url', async () => {
    const options: PublicKeyCredentialCreationOptionsJSON = {
      challenge: 'AQID',
      rp: { name: 'Payroll System' },
      user: {
        id: 'BAU',
        name: 'employee@example.com',
        displayName: 'Employee User',
      },
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
      excludeCredentials: [{ type: 'public-key', id: 'Bgc' }],
    };
    createCredential.mockResolvedValue({
      id: 'credential-1',
      rawId: buffer(251, 255),
      type: 'public-key',
      response: {
        attestationObject: buffer(0, 255),
        clientDataJSON: buffer(250, 251, 252),
      },
    } as PublicKeyCredential);

    const result = await createPasskeyCredential(options);
    const browserOptions = createCredential.mock.calls[0][0] as CredentialCreationOptions;

    expect(bytes(browserOptions.publicKey?.challenge as BufferSource)).toEqual([1, 2, 3]);
    expect(bytes(browserOptions.publicKey?.user.id as BufferSource)).toEqual([4, 5]);
    expect(bytes(browserOptions.publicKey?.excludeCredentials?.[0].id as BufferSource)).toEqual([6, 7]);
    expect(result).toEqual({
      id: 'credential-1',
      rawId: '-_8',
      type: 'public-key',
      response: {
        attestationObject: 'AP8',
        clientDataJSON: '-vv8',
      },
    });
  });

  it('decodes authentication options and serializes assertion fields including userHandle', async () => {
    const options: PublicKeyCredentialRequestOptionsJSON = {
      challenge: '_-4',
      allowCredentials: [{ type: 'public-key', id: 'CAk' }],
    };
    getCredential.mockResolvedValue({
      id: 'credential-2',
      rawId: buffer(1, 2, 3),
      type: 'public-key',
      response: {
        authenticatorData: buffer(10, 11, 12),
        clientDataJSON: buffer(13, 14),
        signature: buffer(255, 254),
        userHandle: buffer(16, 17),
      },
    } as PublicKeyCredential);

    const result = await getPasskeyCredential(options);
    const browserOptions = getCredential.mock.calls[0][0] as CredentialRequestOptions;

    expect(bytes(browserOptions.publicKey?.challenge as BufferSource)).toEqual([255, 238]);
    expect(bytes(browserOptions.publicKey?.allowCredentials?.[0].id as BufferSource)).toEqual([8, 9]);
    expect(result).toEqual({
      id: 'credential-2',
      rawId: 'AQID',
      type: 'public-key',
      response: {
        authenticatorData: 'CgsM',
        clientDataJSON: 'DQ4',
        signature: '__4',
        userHandle: 'EBE',
      },
    });
  });

  it('keeps a missing assertion userHandle as null', async () => {
    getCredential.mockResolvedValue({
      id: 'credential-3',
      rawId: buffer(1),
      type: 'public-key',
      response: {
        authenticatorData: buffer(2),
        clientDataJSON: buffer(3),
        signature: buffer(4),
        userHandle: null,
      },
    } as PublicKeyCredential);

    const result = await getPasskeyCredential({ challenge: 'AQ' });
    expect(result.response.userHandle).toBeNull();
  });

  it('reports clear cancellation errors', async () => {
    createCredential.mockResolvedValueOnce(null);
    getCredential.mockResolvedValueOnce(null);

    await expect(createPasskeyCredential({
      challenge: 'AQ',
      rp: { name: 'Payroll System' },
      user: { id: 'Ag', name: 'user', displayName: 'User' },
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
    })).rejects.toThrow('Passkey creation was cancelled');
    await expect(getPasskeyCredential({ challenge: 'AQ' }))
      .rejects.toThrow('Passkey authentication was cancelled');
  });

  it('detects whether both required browser APIs are available', () => {
    expect(isWebAuthnSupported()).toBe(true);

    Object.defineProperty(window, 'PublicKeyCredential', {
      configurable: true,
      value: undefined,
    });
    expect(isWebAuthnSupported()).toBe(false);

    Object.defineProperty(window, 'PublicKeyCredential', {
      configurable: true,
      value: class PublicKeyCredential {},
    });
    Object.defineProperty(navigator, 'credentials', {
      configurable: true,
      value: undefined,
    });
    expect(isWebAuthnSupported()).toBe(false);
  });
});
