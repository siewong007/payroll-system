import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { User } from '@/types';
import { AuthProvider } from '../context/AuthProvider';
import { useAuth } from '../context/AuthContext';

const apiMocks = vi.hoisted(() => ({
  post: vi.fn(),
  put: vi.fn(),
  setAccessToken: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  default: {
    post: apiMocks.post,
    put: apiMocks.put,
  },
  setAccessToken: apiMocks.setAccessToken,
}));

const user: User = {
  id: 'user-1',
  email: 'admin@example.com',
  full_name: 'Admin User',
  roles: ['admin'],
  company_id: 'company-1',
  employee_id: null,
};

const switchedUser: User = {
  ...user,
  roles: ['payroll_admin'],
  company_id: 'company-2',
};

function renderAuth() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>{children}</AuthProvider>
    </QueryClientProvider>
  );
  const hook = renderHook(() => useAuth(), { wrapper: Wrapper });
  return { ...hook, queryClient };
}

describe('AuthProvider', () => {
  beforeEach(() => {
    apiMocks.post.mockReset();
    apiMocks.put.mockReset();
    apiMocks.setAccessToken.mockReset();
    apiMocks.post.mockRejectedValue(new Error('No session'));
    window.history.replaceState(null, '', '/');
    localStorage.clear();
  });

  it('restores an authenticated session from the refresh cookie', async () => {
    apiMocks.post.mockResolvedValueOnce({ data: { token: 'restored-token', user } });
    const { result } = renderAuth();

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(apiMocks.post).toHaveBeenCalledWith('/auth/refresh', {});
    expect(apiMocks.setAccessToken).toHaveBeenCalledWith('restored-token');
    expect(result.current.user).toEqual(user);
    expect(result.current.token).toBe('restored-token');
    expect(result.current.isAuthenticated).toBe(true);
    expect(JSON.parse(localStorage.getItem('user') ?? 'null')).toEqual(user);
  });

  it('finishes unauthenticated and removes stale display data when restoration fails', async () => {
    localStorage.setItem('user', JSON.stringify(user));
    const { result } = renderAuth();

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(apiMocks.setAccessToken).toHaveBeenCalledWith(null);
    expect(result.current.user).toBeNull();
    expect(result.current.token).toBeNull();
    expect(result.current.isAuthenticated).toBe(false);
    expect(localStorage.getItem('user')).toBeNull();
  });

  it('does not churn the refresh endpoint on a public kiosk route', async () => {
    window.history.replaceState(null, '', '/kiosk/tablet-key');
    const { result } = renderAuth();

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(apiMocks.post).not.toHaveBeenCalled();
    expect(apiMocks.setAccessToken).not.toHaveBeenCalled();
    expect(result.current.isAuthenticated).toBe(false);
  });

  it('logs in and stores only the display user in localStorage', async () => {
    const { result } = renderAuth();
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    apiMocks.post.mockReset();
    apiMocks.post.mockResolvedValueOnce({ data: { token: 'login-token', user } });

    let loginResult: Awaited<ReturnType<typeof result.current.login>> | undefined;
    await act(async () => {
      loginResult = await result.current.login('admin@example.com', 'secret-password');
    });

    expect(apiMocks.post).toHaveBeenCalledWith('/auth/login', {
      email: 'admin@example.com',
      password: 'secret-password',
    });
    expect(loginResult).toEqual({ status: 'success', user });
    expect(apiMocks.setAccessToken).toHaveBeenLastCalledWith('login-token');
    expect(result.current.isAuthenticated).toBe(true);
    expect(localStorage.getItem('access_token')).toBeNull();
    expect(JSON.parse(localStorage.getItem('user') ?? 'null')).toEqual(user);
  });

  it('clears local auth and private query data even when server logout fails', async () => {
    const { result, queryClient } = renderAuth();
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    apiMocks.post.mockReset();
    apiMocks.post.mockRejectedValueOnce(new Error('Network unavailable'));
    queryClient.setQueryData(['private-payroll'], { total: '1000.00' });
    act(() => result.current.setSession('active-token', user));

    await act(async () => result.current.logout());

    expect(apiMocks.post).toHaveBeenCalledWith('/auth/logout');
    expect(apiMocks.setAccessToken).toHaveBeenLastCalledWith(null);
    expect(result.current.user).toBeNull();
    expect(result.current.token).toBeNull();
    expect(localStorage.getItem('user')).toBeNull();
    expect(queryClient.getQueryData(['private-payroll'])).toBeUndefined();
  });

  it('switches company and invalidates all company-scoped query state', async () => {
    const { result, queryClient } = renderAuth();
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    apiMocks.put.mockResolvedValueOnce({
      data: { token: 'company-2-token', user: switchedUser },
    });
    const removeQueries = vi.spyOn(queryClient, 'removeQueries');
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries');

    await act(async () => result.current.switchCompany('company-2'));

    expect(apiMocks.put).toHaveBeenCalledWith('/auth/switch-company', {
      company_id: 'company-2',
    });
    expect(apiMocks.setAccessToken).toHaveBeenLastCalledWith('company-2-token');
    expect(result.current.user).toEqual(switchedUser);
    expect(result.current.token).toBe('company-2-token');
    expect(removeQueries).toHaveBeenCalledOnce();
    expect(invalidateQueries).toHaveBeenCalledOnce();
    expect(JSON.parse(localStorage.getItem('user') ?? 'null')).toEqual(switchedUser);
  });
});
