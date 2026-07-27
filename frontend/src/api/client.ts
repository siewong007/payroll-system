import axios from 'axios';

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

export function setAccessToken(token: string | null) {
  accessToken = token;
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
  '/auth/refresh',
  '/auth/2fa/verify',
  '/auth/2fa/setup/confirm',
  '/auth/2fa/disable',
  '/auth/2fa/backup-codes/regenerate',
  '/auth/oauth2/providers',
  '/auth/passkey/check',
  '/auth/passkey/authenticate/begin',
  '/auth/passkey/authenticate/complete',
  '/auth/passkey/discoverable/begin',
  '/auth/passkey/discoverable/complete',
  '/attendance/check-in/face-id',
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

/**
 * Turns a 429 into a message a person can act on.
 *
 * The backend limits expensive endpoints per session — payroll runs, bulk
 * imports, uploads, outbound mail. Without this the raw axios error surfaces as
 * "Request failed with status code 429", which reads like a bug and invites the
 * user to retry immediately, which is the one thing that keeps them limited.
 */
function describeRateLimit(error: {
  response?: { status?: number; headers?: Record<string, unknown> };
  rateLimitMessage?: string;
}) {
  const retryAfter = Number(error.response?.headers?.['retry-after']);
  const wait =
    Number.isFinite(retryAfter) && retryAfter > 0
      ? `Try again in ${retryAfter} second${retryAfter === 1 ? '' : 's'}.`
      : 'Try again shortly.';
  return `Too many requests. ${wait}`;
}

// Handle 401 responses with refresh token retry (cookie-based)
api.interceptors.response.use(
  (response) => response,
  async (error) => {
    const originalRequest = error.config;

    // Attach before any other handling: a 429 is never a session problem, so it
    // must not fall through to the refresh-and-redirect path below.
    if (error.response?.status === 429) {
      error.rateLimitMessage = describeRateLimit(error);
      return Promise.reject(error);
    }

    if (error.response?.status !== 401) {
      return Promise.reject(error);
    }

    // The public kiosk endpoint authenticates via a kiosk secret, not the user JWT.
    // A 401 there means the kiosk credential was revoked — surface it to the caller
    // verbatim, never refresh or redirect.
    const isKioskEndpoint = originalRequest.url === '/attendance/kiosk/qr';

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
      NO_REFRESH_401_ENDPOINTS.includes(originalRequest.url ?? '')
    ) {
      return Promise.reject(error);
    }

    if (isRefreshing) {
      return new Promise((resolve, reject) => {
        failedQueue.push({ resolve, reject });
      }).then((token) => {
        originalRequest.headers.Authorization = `Bearer ${token}`;
        return api(originalRequest);
      });
    }

    originalRequest._retry = true;
    isRefreshing = true;

    try {
      // Refresh token is sent automatically via httpOnly cookie
      const { data } = await axios.post(`${API_BASE_URL}/auth/refresh`, {}, { withCredentials: true });

      accessToken = data.token;
      localStorage.setItem('user', JSON.stringify(data.user));

      processQueue(null, data.token);
      originalRequest.headers.Authorization = `Bearer ${data.token}`;
      return api(originalRequest);
    } catch (refreshError) {
      processQueue(refreshError, null);
      accessToken = null;
      localStorage.removeItem('user');
      window.location.href = '/login';
      return Promise.reject(refreshError);
    } finally {
      isRefreshing = false;
    }
  }
);

export default api;
