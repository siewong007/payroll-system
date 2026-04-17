import { createContext, useContext } from 'react';
import type { User } from '@/types';

export interface AuthContextType {
  user: User | null;
  token: string | null;
  login: (email: string, password: string) => Promise<User>;
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
