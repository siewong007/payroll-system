import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createElement, type ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import {
  createPasskeyCredential,
  getPasskeyCredential,
  isWebAuthnSupported,
  type PublicKeyCredentialCreationOptionsJSON,
  type PublicKeyCredentialRequestOptionsJSON,
} from '@/lib/webauthn';
import { AuthProvider } from '@/context/AuthProvider';
import { Login } from '@/pages/auth/Login';

const apiMocks = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  put: vi.fn(),
  setAccessToken: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  default: { get: apiMocks.get, post: apiMocks.post, put: apiMocks.put },
  setAccessToken: apiMocks.setAccessToken,
}));

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

// The tests above hand `getPasskeyCredential` an already-flat options object, so
// they cannot see a caller that forgets to unwrap. webauthn-rs sends
// `{ challenge_id, options: { publicKey: {...} } }`; passing the envelope
// through leaves `challenge` undefined and throws inside base64urlToBuffer
// before navigator.credentials.get is ever reached — i.e. passkey login is dead
// at runtime while still typechecking. These drive the real Login screen with
// the real wire payload so the unwrap is pinned at the call site.
describe('passkey login call sites', () => {
  const sessionUser = {
    id: 'user-1',
    email: 'employee@example.com',
    full_name: 'Employee User',
    roles: ['employee'],
    company_id: 'company-1',
    employee_id: 'emp-1',
  };

  // Shapes as serialized by webauthn-rs RequestChallengeResponse.
  const discoverableBegin = {
    challenge_id: 'challenge-discoverable',
    options: {
      publicKey: {
        challenge: 'AQID',
        rpId: 'localhost',
        allowCredentials: [],
        userVerification: 'preferred',
      },
    },
  };

  const emailBegin = {
    challenge_id: 'challenge-email',
    options: {
      publicKey: {
        challenge: '_-4',
        rpId: 'localhost',
        allowCredentials: [{ type: 'public-key', id: 'CAk' }],
        userVerification: 'preferred',
      },
    },
  };

  const assertion = {
    id: 'credential-login',
    rawId: buffer(1, 2, 3),
    type: 'public-key',
    response: {
      authenticatorData: buffer(10, 11, 12),
      clientDataJSON: buffer(13, 14),
      signature: buffer(255, 254),
      userHandle: null,
    },
  } as unknown as PublicKeyCredential;

  function renderLogin() {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const tree = (children: ReactNode) =>
      createElement(
        QueryClientProvider,
        { client: queryClient },
        createElement(AuthProvider, null, createElement(MemoryRouter, null, children)),
      );
    return render(tree(createElement(Login)));
  }

  beforeEach(() => {
    getCredential.mockReset().mockResolvedValue(assertion);
    createCredential.mockReset();
    apiMocks.setAccessToken.mockReset();
    apiMocks.get.mockReset().mockResolvedValue({ data: [] });
    apiMocks.put.mockReset().mockResolvedValue({ data: {} });
    apiMocks.post.mockReset().mockImplementation((url: string) => {
      switch (url) {
        case '/auth/refresh':
          return Promise.reject(new Error('No session'));
        case '/auth/passkey/check':
          return Promise.resolve({ data: { has_passkey: true } });
        case '/auth/passkey/discoverable/begin':
          return Promise.resolve({ data: discoverableBegin });
        case '/auth/passkey/authenticate/begin':
          return Promise.resolve({ data: emailBegin });
        case '/auth/passkey/discoverable/complete':
        case '/auth/passkey/authenticate/complete':
          return Promise.resolve({ data: { token: 'session-token', user: sessionUser } });
        default:
          return Promise.resolve({ data: {} });
      }
    });

    Object.defineProperty(navigator, 'credentials', {
      configurable: true,
      value: { create: createCredential, get: getCredential },
    });
    Object.defineProperty(window, 'PublicKeyCredential', {
      configurable: true,
      value: class PublicKeyCredential {},
    });
  });

  it('unwraps options.publicKey for the discoverable flow before the browser ceremony', async () => {
    const user = userEvent.setup();
    renderLogin();

    await user.click(await screen.findByRole('button', { name: /sign in with passkey/i }));

    await waitFor(() => expect(getCredential).toHaveBeenCalled());
    const browserOptions = getCredential.mock.calls[0][0] as CredentialRequestOptions;

    // The envelope must not survive into the ceremony: a nested publicKey here
    // means the caller passed the wrapper.
    expect(browserOptions.publicKey).not.toHaveProperty('publicKey');
    expect(bytes(browserOptions.publicKey?.challenge as BufferSource)).toEqual([1, 2, 3]);

    await waitFor(() =>
      expect(apiMocks.post).toHaveBeenCalledWith(
        '/auth/passkey/discoverable/complete',
        expect.objectContaining({ challenge_id: 'challenge-discoverable' }),
      ),
    );
    expect(screen.queryByText(/passkey authentication failed/i)).toBeNull();
  });

  it('unwraps options.publicKey for the email flow, decoding allowCredentials', async () => {
    const user = userEvent.setup();
    renderLogin();

    await user.type(screen.getByPlaceholderText('Enter your email'), 'employee@example.com');
    // The passkey lookup is debounced 500ms; the email branch is only taken once
    // it has reported back.
    await waitFor(
      () =>
        expect(apiMocks.post).toHaveBeenCalledWith('/auth/passkey/check', {
          email: 'employee@example.com',
        }),
      { timeout: 3000 },
    );
    await act(async () => {
      await Promise.resolve();
    });

    await user.click(screen.getByRole('button', { name: /sign in with passkey/i }));

    await waitFor(() => expect(getCredential).toHaveBeenCalled());
    const browserOptions = getCredential.mock.calls[0][0] as CredentialRequestOptions;

    expect(browserOptions.publicKey).not.toHaveProperty('publicKey');
    expect(bytes(browserOptions.publicKey?.challenge as BufferSource)).toEqual([255, 238]);
    expect(bytes(browserOptions.publicKey?.allowCredentials?.[0].id as BufferSource)).toEqual([8, 9]);

    await waitFor(() =>
      expect(apiMocks.post).toHaveBeenCalledWith(
        '/auth/passkey/authenticate/complete',
        expect.objectContaining({ challenge_id: 'challenge-email' }),
      ),
    );
  });
});
