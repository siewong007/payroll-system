import axios from 'axios';
import type { User } from '@/types';

function resolveApiBaseUrl(rawBaseUrl?: string) {
  if (!rawBaseUrl) {
    return '/api';
  }

  const normalized = rawBaseUrl.replace(/\/+$/, '');
  return normalized.endsWith('/api') ? normalized : `${normalized}/api`;
}

const API_BASE_URL = resolveApiBaseUrl(import.meta.env.VITE_API_URL);

const api = axios.create({
  baseURL: API_BASE_URL,
  headers: {
    'Content-Type': 'application/json',
  },
  withCredentials: true,
});

// Module-level token storage (in-memory only, not localStorage)
let accessToken: string | null = null;
let currentIdentity: { userId: string | null; companyId: string | null } = {
  userId: null,
  companyId: null,
};
type AuthRefreshedListener = (token: string, user: User) => void;
type AuthIdentityListener = (user: User | null) => void;
const authRefreshedListeners = new Set<AuthRefreshedListener>();
const authIdentityChangedListeners = new Set<AuthIdentityListener>();

export function onAuthRefreshed(listener: AuthRefreshedListener): () => void {
  authRefreshedListeners.add(listener);
  return () => {
    authRefreshedListeners.delete(listener);
  };
}

export function onAuthIdentityChanged(listener: AuthIdentityListener): () => void {
  authIdentityChangedListeners.add(listener);
  return () => {
    authIdentityChangedListeners.delete(listener);
  };
}

export function setAuthIdentity(user: Pick<User, 'id' | 'company_id'> | null) {
  currentIdentity = user
    ? { userId: user.id, companyId: user.company_id }
    : { userId: null, companyId: null };
}

export function setAccessToken(token: string | null) {
  accessToken = token;
  if (token === null) {
    currentIdentity = { userId: null, companyId: null };
  }
}

export function getAccessToken(): string | null {
  return accessToken;
}

// Add auth token to requests. Skip if the caller already set an Authorization
// header (e.g. the public kiosk endpoint sends `Authorization: Kiosk <secret>`).
api.interceptors.request.use((config) => {
  if (accessToken && !config.headers.Authorization) {
    config.headers.Authorization = `Bearer ${accessToken}`;
  }
  return config;
});

/**
 * Endpoints whose 401 is about the *content* of the request, not the session.
 *
 * A mistyped TOTP digit, a rejected passkey assertion and a failed Face ID
 * ceremony all come back as 401 from a request whose bearer token is perfectly
 * valid. Refreshing achieves nothing and the replay is actively harmful: a
 * WebAuthn challenge is consumed on first use, so the second attempt returns
 * "challenge expired" and the user is told the wrong thing.
 *
 * Matched with `includes()` against the axios-relative url, which is the form
 * every api module uses — no baseURL prefix.
 */
const NO_REFRESH_401_ENDPOINTS = [
  '/auth/login',
  '/auth/login/code',
  '/auth/refresh',
  '/auth/2fa/verify',
  '/auth/2fa/setup/confirm',
  '/auth/2fa/disable',
  '/auth/2fa/backup-codes/regenerate',
  '/auth/oauth2/providers',
  '/auth/passkey/check',
  '/auth/passkey/discoverable/begin',
  '/auth/passkey/discoverable/complete',
  '/attendance/check-in/face-id',
  '/attendance/check-in/qr-face-id',
];

/**
 * Does this 401 carry the backend's "your credential is not usable" marker?
 *
 * Inert for now: `AppError::SessionInvalid` is added by the auth cluster and
 * nothing emits `code: "session_invalid"` yet. Once it does, the refresh branch
 * below becomes an allow-list — refresh *only* when the server said the session
 * is the problem — which is the durable form of the list above, since it cannot
 * rot when someone adds a new endpoint that 401s on its payload.
 */
export function isSessionInvalid(error: unknown): boolean {
  if (typeof error !== 'object' || error === null || !('response' in error)) {
    return false;
  }
  const response = (error as { response?: { data?: { code?: string } } }).response;
  return response?.data?.code === 'session_invalid';
}

let isRefreshing = false;
let failedQueue: { resolve: (token: string) => void; reject: (err: unknown) => void }[] = [];

function processQueue(error: unknown, token: string | null) {
  failedQueue.forEach((prom) => {
    if (token) {
      prom.resolve(token);
    } else {
      prom.reject(error);
    }
  });
  failedQueue = [];
}

async function decodeBlobErrorData(error: { response?: { data?: unknown } }): Promise<void> {
  const data = error.response?.data;
  if (!(data instanceof Blob)) {
    return;
  }
  try {
    const text = await data.text();
    if (!text) return;
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed === 'object' && parsed !== null && error.response) {
      error.response.data = parsed;
    }
  } catch {
    // Non-JSON blob error body — leave as-is for fallback handling.
  }
}

// Handle 401 responses with refresh token retry (cookie-based)
api.interceptors.response.use(
  (response) => response,
  async (error) => {
    if (error.response?.data instanceof Blob) {
      await decodeBlobErrorData(error);
    }
    const originalRequest = error.config;

    // A 429 is never a session problem — reject immediately so callers can read
    // the server error message or localized Retry-After fallback via getErrorMessage().
    if (error.response?.status === 429) {
      return Promise.reject(error);
    }

    if (error.response?.status !== 401) {
      return Promise.reject(error);
    }

    const requestPath = String(originalRequest?.url ?? '').split(/[?#]/, 1)[0];

    // The public kiosk endpoint authenticates via a kiosk secret, not the user JWT.
    // A 401 there means the kiosk credential was revoked — surface it to the caller
    // verbatim, never refresh or redirect.
    const isKioskEndpoint = requestPath === '/attendance/kiosk/qr';

    // A 401 on a request that has *already* been replayed after a successful
    // refresh is by definition not a session problem: the session was just
    // renewed. Clearing the token and navigating to /login there is what logged
    // a user out over a mistyped 2FA digit, unloading the document before the
    // "Invalid authentication code" message could render and abandoning the
    // pending enrolment secret. Reject and let the caller display it; the next
    // genuine expiry refreshes, fails, and logs out normally.
    if (
      originalRequest._retry ||
      isKioskEndpoint ||
      NO_REFRESH_401_ENDPOINTS.includes(requestPath)
    ) {
      return Promise.reject(error);
    }

    if (isRefreshing) {
      return new Promise((resolve, reject) => {
        failedQueue.push({ resolve, reject });
      }).then((token) => {
        originalRequest._retry = true;
        originalRequest.headers.Authorization = `Bearer ${token}`;
        return api(originalRequest);
      });
    }

    originalRequest._retry = true;
    isRefreshing = true;

    try {
      // Refresh token is sent automatically via httpOnly cookie
      const { data } = await axios.post(`${API_BASE_URL}/auth/refresh`, {}, { withCredentials: true });
      const nextUser: User = data.user;
      const identityChanged =
        currentIdentity.userId !== null &&
        (currentIdentity.userId !== nextUser.id || currentIdentity.companyId !== nextUser.company_id);

      accessToken = data.token;
      currentIdentity = { userId: nextUser.id, companyId: nextUser.company_id };
      localStorage.setItem('user', JSON.stringify(nextUser));
      authRefreshedListeners.forEach((listener) => listener(data.token, nextUser));

      if (identityChanged) {
        authIdentityChangedListeners.forEach((listener) => listener(nextUser));
        const identityError = new Error('Session identity changed');
        processQueue(identityError, null);
        return Promise.reject(identityError);
      }

      processQueue(null, data.token);
      originalRequest.headers.Authorization = `Bearer ${data.token}`;
      return api(originalRequest);
    } catch (refreshError) {
      processQueue(refreshError, null);
      accessToken = null;
      currentIdentity = { userId: null, companyId: null };
      localStorage.removeItem('user');
      window.location.href = '/login';
      return Promise.reject(refreshError);
    } finally {
      isRefreshing = false;
    }
  }
);

export default api;
