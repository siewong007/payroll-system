import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { User, LoginResponse } from '@/types';
import api, { setAccessToken } from '@/api/client';

interface AuthContextType {
  user: User | null;
  token: string | null;
  login: (email: string, password: string) => Promise<User>;
  logout: () => Promise<void>;
  switchCompany: (companyId: string) => Promise<void>;
  setSession: (token: string, user: User) => void;
  isAuthenticated: boolean;
  isLoading: boolean;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const queryClient = useQueryClient();

  // On mount, try to restore session via cookie-based refresh
  useEffect(() => {
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

  const login = async (email: string, password: string): Promise<User> => {
    const { data } = await api.post<LoginResponse>('/auth/login', { email, password });
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

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}
