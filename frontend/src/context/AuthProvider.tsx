import { useState, useEffect, useCallback, useMemo, useRef, type ReactNode } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { User, LoginResponse, MfaRequiredResponse } from '@/types';
import api, { onAuthIdentityChanged, onAuthRefreshed, setAccessToken, setAuthIdentity } from '@/api/client';
import { deletePushSubscription } from '@/api/notifications';
import { verifyTwoFactorLogin } from '@/api/totp';
import { getCurrentPushEndpoint, unsubscribeFromPush, updateAppBadge } from '@/lib/pwa';
import { AuthContext, type LoginResult } from './AuthContext';

function shouldSkipSessionRestore(pathname: string): boolean {
  return pathname === '/kiosk' || pathname.startsWith('/kiosk/');
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const queryClient = useQueryClient();
  const identityRef = useRef<{ id: string; companyId: string | null } | null>(null);

  useEffect(() => {
    identityRef.current = user ? { id: user.id, companyId: user.company_id } : null;
  }, [user]);

  useEffect(() => {
    const unsubRefresh = onAuthRefreshed((newToken, newUser) => {
      setToken(newToken);
      setUser(newUser);
    });
    const unsubIdentity = onAuthIdentityChanged((nextUser) => {
      void queryClient.cancelQueries();
      queryClient.clear();
      setUser(nextUser);
    });
    return () => {
      unsubRefresh();
      unsubIdentity();
    };
  }, [queryClient]);

  useEffect(() => {
    const handleStorage = (event: StorageEvent) => {
      if (event.key !== null && event.key !== 'user') return;

      if (!event.newValue) {
        void queryClient.cancelQueries();
        queryClient.clear();
        setAccessToken(null);
        setAuthIdentity(null);
        setToken(null);
        setUser(null);
        try {
          sessionStorage.clear();
        } catch {
          // Ignore storage restrictions
        }
        return;
      }

      try {
        const nextUser = JSON.parse(event.newValue) as User;
        if (!nextUser || typeof nextUser.id !== 'string') return;
        const current = identityRef.current;
        if (!current || current.id !== nextUser.id || current.companyId !== nextUser.company_id) {
          void queryClient.cancelQueries();
          queryClient.clear();
          setAccessToken(null);
          setAuthIdentity(nextUser);
          setToken(null);
          setUser(nextUser);
          void api
            .post<LoginResponse>('/auth/refresh', {})
            .then(({ data }) => {
              setAccessToken(data.token);
              setAuthIdentity(data.user);
              setToken(data.token);
              setUser(data.user);
              localStorage.setItem('user', JSON.stringify(data.user));
            })
            .catch(() => {
              setAccessToken(null);
              setAuthIdentity(null);
              setToken(null);
              setUser(null);
              localStorage.removeItem('user');
            });
        }
      } catch {
        // Ignore malformed cross-tab user payload
      }
    };

    window.addEventListener('storage', handleStorage);
    return () => window.removeEventListener('storage', handleStorage);
  }, [queryClient]);

  // On mount, try to restore session via cookie-based refresh.
  // Skip on public kiosk tablet routes so they don't churn /auth/refresh.
  useEffect(() => {
    if (shouldSkipSessionRestore(window.location.pathname)) {
      setIsLoading(false);
      return;
    }
    const restoreSession = async () => {
      try {
        // Refresh token is sent automatically via httpOnly cookie
        const { data } = await api.post<LoginResponse>('/auth/refresh', {});
        setAccessToken(data.token);
        setAuthIdentity(data.user);
        setToken(data.token);
        setUser(data.user);
        localStorage.setItem('user', JSON.stringify(data.user));
      } catch {
        // No valid session — user needs to log in
        setAccessToken(null);
        setAuthIdentity(null);
        setToken(null);
        setUser(null);
        localStorage.removeItem('user');
      } finally {
        setIsLoading(false);
      }
    };

    void restoreSession();
  }, []);

  const setSession = useCallback(
    (newToken: string, newUser: User) => {
      queryClient.clear();
      setAccessToken(newToken);
      setAuthIdentity(newUser);
      setToken(newToken);
      setUser(newUser);
      localStorage.setItem('user', JSON.stringify(newUser));
    },
    [queryClient],
  );

  const login = useCallback(
    async (email: string, password: string, turnstileToken?: string): Promise<LoginResult> => {
      const { data } = await api.post<LoginResponse | MfaRequiredResponse>('/auth/login', {
        email,
        password,
        turnstile_token: turnstileToken,
      });

      if ('requires_2fa' in data && data.requires_2fa) {
        return { status: 'mfa_required', mfaToken: data.mfa_token };
      }

      // Refresh token is set as httpOnly cookie by the server
      setSession(data.token, data.user);
      return { status: 'success', user: data.user };
    },
    [setSession],
  );

  const completeTwoFactorLogin = useCallback(
    async (mfaToken: string, code: string, turnstileToken?: string): Promise<User> => {
      const data = await verifyTwoFactorLogin(mfaToken, code, turnstileToken);
      // Refresh token is set as httpOnly cookie by the server
      setSession(data.token, data.user);
      return data.user;
    },
    [setSession],
  );

  const logout = useCallback(async (options?: { skipRemotePushDelete?: boolean }) => {
    try {
      const endpoint = await getCurrentPushEndpoint();
      if (endpoint) {
        if (!options?.skipRemotePushDelete) {
          await deletePushSubscription(endpoint).catch(() => undefined);
        }
        await unsubscribeFromPush();
      }
      updateAppBadge(0);
    } catch {
      // Never block sign-out on push cleanup failures.
    }
    try {
      // Server reads refresh token from cookie and revokes it
      await api.post('/auth/logout', {});
    } catch {
      // Ignore errors on logout — still clear local state
    }
    setAccessToken(null);
    setAuthIdentity(null);
    setToken(null);
    setUser(null);
    localStorage.removeItem('user');
    sessionStorage.clear();
    queryClient.clear();
  }, [queryClient]);

  const switchCompany = useCallback(
    async (companyId: string) => {
      const { data } = await api.put<LoginResponse>('/auth/switch-company', { company_id: companyId });
      setAccessToken(data.token);
      setAuthIdentity(data.user);
      setToken(data.token);
      setUser(data.user);
      localStorage.setItem('user', JSON.stringify(data.user));
      await queryClient.cancelQueries();
      queryClient.removeQueries();
      await queryClient.invalidateQueries({ refetchType: 'none' });
    },
    [queryClient],
  );

  const value = useMemo(
    () => ({
      user,
      token,
      login,
      completeTwoFactorLogin,
      logout,
      switchCompany,
      setSession,
      isAuthenticated: !!token,
      isLoading,
    }),
    [user, token, login, completeTwoFactorLogin, logout, switchCompany, setSession, isLoading],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
