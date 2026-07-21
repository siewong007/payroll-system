import { createContext, useContext } from 'react';
import type { User } from '@/types';

// login() can't always finish in one step — when the account has TOTP 2FA
// enabled, the backend returns a pending mfaToken instead of a session, and
// the caller must collect a code and call completeTwoFactorLogin.
export type LoginResult =
  | { status: 'success'; user: User }
  | { status: 'mfa_required'; mfaToken: string };

export interface AuthContextType {
  user: User | null;
  token: string | null;
  login: (email: string, password: string) => Promise<LoginResult>;
  completeTwoFactorLogin: (mfaToken: string, code: string) => Promise<User>;
  logout: () => Promise<void>;
  switchCompany: (companyId: string) => Promise<void>;
  setSession: (token: string, user: User) => void;
  isAuthenticated: boolean;
  isLoading: boolean;
}

export const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}
