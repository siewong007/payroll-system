import { beforeEach, describe, expect, it, vi, afterEach } from 'vitest';
import { createElement, type ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { AuthProvider } from '@/context/AuthProvider';
import { Login } from '@/pages/auth/Login';

const apiMocks = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  put: vi.fn(),
  setAccessToken: vi.fn(),
}));

vi.mock('@/api/client', () => ({
  default: { get: apiMocks.get, post: apiMocks.post, put: apiMocks.put },
  setAccessToken: apiMocks.setAccessToken,
}));

const sessionUser = {
  id: 'user-1',
  email: 'employee@example.com',
  full_name: 'Employee User',
  roles: ['employee'],
  company_id: 'company-1',
  employee_id: 'emp-1',
};

function renderLogin() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const tree = (children: ReactNode) =>
    createElement(
      QueryClientProvider,
      { client: queryClient },
      createElement(AuthProvider, null, createElement(MemoryRouter, null, children)),
    );
  return render(tree(createElement(Login)));
}

async function expandMoreOptions(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: /more sign-in options/i }));
}

describe('code sign-in (authenticator / recovery code)', () => {
  beforeEach(() => {
    // .env.local supplies a real site key; a Turnstile-enabled test run would
    // block every submit behind a widget jsdom cannot mint.
    vi.stubEnv('VITE_TURNSTILE_SITE_KEY', '');

    apiMocks.setAccessToken.mockReset();
    apiMocks.get.mockReset().mockResolvedValue({ data: [] });
    apiMocks.put.mockReset().mockResolvedValue({ data: {} });
    apiMocks.post.mockReset().mockImplementation((url: string) => {
      switch (url) {
        case '/auth/refresh':
          return Promise.reject(new Error('No session'));
        case '/auth/login/code':
          return Promise.resolve({ data: { token: 'session-token', user: sessionUser } });
        default:
          return Promise.resolve({ data: {} });
      }
    });
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('labels the email form as Other ways to sign in and hides alternates by default', async () => {
    renderLogin();

    expect(await screen.findByText('Other ways to sign in')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /authenticator/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /recovery code/i })).not.toBeInTheDocument();
  });

  it('expands and collapses the extra sign-in methods via the link', async () => {
    const user = userEvent.setup();
    renderLogin();

    const toggle = await screen.findByRole('button', { name: /more sign-in options/i });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(await screen.findByRole('button', { name: /authenticator/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /recovery code/i })).toBeInTheDocument();

    await user.click(toggle);
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /authenticator/i })).not.toBeInTheDocument(),
    );
  });

  it('toggles password visibility', async () => {
    const user = userEvent.setup();
    renderLogin();

    const password = await screen.findByPlaceholderText('Enter your password');
    expect(password).toHaveAttribute('type', 'password');
    await user.click(screen.getByRole('button', { name: /show password/i }));
    expect(password).toHaveAttribute('type', 'text');
    await user.click(screen.getByRole('button', { name: /hide password/i }));
    expect(password).toHaveAttribute('type', 'password');
  });

  it('shows an email + authenticator code form and submits both to /auth/login/code', async () => {
    const user = userEvent.setup();
    renderLogin();

    await expandMoreOptions(user);
    await user.click(await screen.findByRole('button', { name: /^authenticator$/i }));
    await user.type(screen.getByPlaceholderText('Enter your email'), 'employee@example.com');
    await user.type(screen.getByPlaceholderText('6-digit code'), '123456');
    await user.click(screen.getByRole('button', { name: /^sign in$/i }));

    await waitFor(() =>
      expect(apiMocks.post).toHaveBeenCalledWith('/auth/login/code', {
        email: 'employee@example.com',
        code: '123456',
        turnstile_token: undefined,
      }),
    );
    // A successful code login is a complete session — the user is stored.
    await waitFor(() =>
      expect(JSON.parse(localStorage.getItem('user') ?? '{}')).toMatchObject({
        email: 'employee@example.com',
      }),
    );
  });

  it('switches the code field label and placeholder for recovery codes', async () => {
    const user = userEvent.setup();
    renderLogin();

    await expandMoreOptions(user);
    await user.click(await screen.findByRole('button', { name: /recovery code/i }));

    expect(await screen.findByText('Recovery code')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('e.g. 1A2B-3C4D')).toBeInTheDocument();
  });

  it('surfaces the server rejection instead of a generic message', async () => {
    apiMocks.post.mockImplementation((url: string) => {
      if (url === '/auth/refresh') return Promise.reject(new Error('No session'));
      if (url === '/auth/login/code') {
        return Promise.reject({
          response: { status: 401, data: { error: 'Invalid email or code' } },
        });
      }
      return Promise.resolve({ data: {} });
    });

    const user = userEvent.setup();
    renderLogin();

    await expandMoreOptions(user);
    await user.click(await screen.findByRole('button', { name: /^authenticator$/i }));
    await user.type(screen.getByPlaceholderText('Enter your email'), 'employee@example.com');
    await user.type(screen.getByPlaceholderText('6-digit code'), '000000');
    await user.click(screen.getByRole('button', { name: /^sign in$/i }));

    expect(await screen.findByText('Invalid email or code')).toBeInTheDocument();
  });

  it('returns to the default view via the back link', async () => {
    const user = userEvent.setup();
    renderLogin();

    await expandMoreOptions(user);
    await user.click(await screen.findByRole('button', { name: /^authenticator$/i }));
    await user.click(screen.getByRole('button', { name: /back to all sign-in options/i }));

    expect(await screen.findByPlaceholderText('Enter your password')).toBeInTheDocument();
    expect(screen.getByText('Other ways to sign in')).toBeInTheDocument();
  });
});
