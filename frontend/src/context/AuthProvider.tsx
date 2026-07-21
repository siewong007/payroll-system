import { useState, useEffect, useCallback, type ReactNode } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { User, LoginResponse, MfaRequiredResponse } from '@/types';
import api, { setAccessToken } from '@/api/client';
import { verifyTwoFactorLogin } from '@/api/totp';
import { AuthContext, type LoginResult } from './AuthContext';

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const queryClient = useQueryClient();

  // On mount, try to restore session via cookie-based refresh.
  // Skip on the public kiosk path so a tablet without a session doesn't churn /auth/refresh.
  useEffect(() => {
    if (window.location.pathname.startsWith('/kiosk/')) {
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

    restoreSession();
  }, []);

  const setSession = useCallback((newToken: string, newUser: User) => {
    setAccessToken(newToken);
    setToken(newToken);
    setUser(newUser);
    localStorage.setItem('user', JSON.stringify(newUser));
  }, []);

  const login = async (email: string, password: string): Promise<LoginResult> => {
    const { data } = await api.post<LoginResponse | MfaRequiredResponse>('/auth/login', {
      email,
      password,
    });

    if ('requires_2fa' in data && data.requires_2fa) {
      return { status: 'mfa_required', mfaToken: data.mfa_token };
    }

    // Refresh token is set as httpOnly cookie by the server
    setSession(data.token, data.user);
    return { status: 'success', user: data.user };
  };

  const completeTwoFactorLogin = async (mfaToken: string, code: string): Promise<User> => {
    const data = await verifyTwoFactorLogin(mfaToken, code);
    // Refresh token is set as httpOnly cookie by the server
    setSession(data.token, data.user);
    return data.user;
  };

  const logout = async () => {
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
    queryClient.clear();
  };

  const switchCompany = async (companyId: string) => {
    const { data } = await api.put<LoginResponse>('/auth/switch-company', { company_id: companyId });
    setAccessToken(data.token);
    setToken(data.token);
    setUser(data.user);
    localStorage.setItem('user', JSON.stringify(data.user));
    queryClient.removeQueries();
    await queryClient.invalidateQueries();
  };

  return (
    <AuthContext.Provider
      value={{
        user,
        token,
        login,
        completeTwoFactorLogin,
        logout,
        switchCompany,
        setSession,
        isAuthenticated: !!token,
        isLoading,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}
