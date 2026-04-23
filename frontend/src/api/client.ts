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

// Handle 401 responses with refresh token retry (cookie-based)
api.interceptors.response.use(
  (response) => response,
  async (error) => {
    const originalRequest = error.config;

    // The public kiosk endpoint authenticates via a kiosk secret, not the user JWT.
    // A 401 there means the kiosk credential was revoked — surface it to the caller
    // verbatim, never refresh or redirect.
    const isKioskEndpoint = originalRequest.url === '/attendance/kiosk/qr';

    // Don't retry refresh or login requests
    if (
      error.response?.status !== 401 ||
      originalRequest._retry ||
      originalRequest.url === '/auth/login' ||
      originalRequest.url === '/auth/refresh' ||
      isKioskEndpoint
    ) {
      // Only redirect for 401 on regular API calls, not auth endpoints
      if (
        error.response?.status === 401 &&
        originalRequest.url !== '/auth/login' &&
        originalRequest.url !== '/auth/refresh' &&
        originalRequest.url !== '/auth/oauth2/providers' &&
        !isKioskEndpoint
      ) {
        accessToken = null;
        localStorage.removeItem('user');
        window.location.href = '/login';
      }
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
