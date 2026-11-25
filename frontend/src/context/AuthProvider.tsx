import { useState, useEffect, useCallback, useMemo, type ReactNode } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { User, LoginResponse, MfaRequiredResponse } from '@/types';
import api, { onAuthRefreshed, setAccessToken } from '@/api/client';
import { verifyTwoFactorLogin } from '@/api/totp';
import { AuthContext, type LoginResult } from './AuthContext';

function shouldSkipSessionRestore(pathname: string): boolean {
  return pathname === '/kiosk' || pathname.startsWith('/kiosk/');
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const queryClient = useQueryClient();

  useEffect(() => {
    return onAuthRefreshed((newToken, newUser) => {
      setToken(newToken);
      setUser(newUser);
    });
  }, []);

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
        setToken(data.token);
        setUser(data.user);
        localStorage.setItem('user', JSON.stringify(data.user));
      } catch {
        // No valid session — user needs to log in
        setAccessToken(null);
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
    async (mfaToken: string, code: string): Promise<User> => {
      const data = await verifyTwoFactorLogin(mfaToken, code);
      // Refresh token is set as httpOnly cookie by the server
      setSession(data.token, data.user);
      return data.user;
    },
    [setSession],
  );

  const logout = useCallback(async () => {
    try {
      // Server reads refresh token from cookie and revokes it
      await api.post('/auth/logout');
    } catch {
      // Ignore errors on logout — still clear local state
    }
    setAccessToken(null);
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
      setToken(data.token);
      setUser(data.user);
      localStorage.setItem('user', JSON.stringify(data.user));
      queryClient.removeQueries();
      await queryClient.invalidateQueries();
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
