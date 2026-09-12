import { MemoryRouter } from 'react-router';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { linkGoogleAccount } from '@/api/oauth2';

const navigate = vi.fn();
vi.mock('react-router', async () => {
  const actual = await vi.importActual<typeof import('react-router')>('react-router');
  return { ...actual, useNavigate: () => navigate };
});

const authState = { isAuthenticated: true, isLoading: false };
vi.mock('@/context/AuthContext', () => ({
  useAuth: () => authState,
}));

vi.mock('@/api/oauth2', () => ({
  linkGoogleAccount: vi.fn(),
}));

const linkGoogle = vi.mocked(linkGoogleAccount);

/**
 * Land on the link URL with `search`, as a fresh page load.
 *
 * The component memoises the query params at module scope — they are stripped
 * from the URL once read — so a faithful new load re-imports the module.
 */
async function loadLinkPage(search: string) {
  window.history.replaceState(null, '', `/oauth2/link${search}`);
  vi.resetModules();
  const { OAuth2Link } = await import('@/pages/auth/OAuth2Link');
  return render(
    <MemoryRouter>
      <OAuth2Link />
    </MemoryRouter>,
  );
}

describe('OAuth2Link', () => {
  beforeEach(() => {
    navigate.mockReset();
    linkGoogle.mockReset();
    sessionStorage.clear();
    authState.isAuthenticated = true;
    authState.isLoading = false;
  });

  it('posts the code and state, then returns to the page that started the flow', async () => {
    sessionStorage.setItem('oauth2_link_return', '/settings');
    linkGoogle.mockResolvedValue({
      id: 'acc-1',
      provider: 'google',
      provider_email: 'a@b.c',
      provider_name: null,
      avatar_url: null,
      linked_at: '2026-09-12T00:00:00Z',
    });

    await loadLinkPage('?code=auth-code&state=state-hash');

    await waitFor(() => expect(linkGoogle).toHaveBeenCalledWith('auth-code', 'state-hash'));
    await waitFor(() => expect(navigate).toHaveBeenCalledWith('/settings', { replace: true }));
    expect(sessionStorage.getItem('oauth2_link_return')).toBeNull();
  });

  it('strips the credentials from the URL once read', async () => {
    linkGoogle.mockResolvedValue({
      id: 'acc-1',
      provider: 'google',
      provider_email: null,
      provider_name: null,
      avatar_url: null,
      linked_at: '2026-09-12T00:00:00Z',
    });

    await loadLinkPage('?code=auth-code&state=state-hash');

    await waitFor(() => expect(linkGoogle).toHaveBeenCalled());
    expect(window.location.search).toBe('');
    expect(window.location.pathname).toBe('/oauth2/link');
  });

  it('shows the provider error Google sent back', async () => {
    await loadLinkPage('?error=access_denied');

    expect(
      await screen.findByText(/cancelled/i),
    ).toBeInTheDocument();
    expect(linkGoogle).not.toHaveBeenCalled();
    expect(navigate).not.toHaveBeenCalled();
  });

  it('reports missing parameters instead of posting', async () => {
    await loadLinkPage('');

    expect(
      await screen.findByText(/Missing authentication data/i),
    ).toBeInTheDocument();
    expect(linkGoogle).not.toHaveBeenCalled();
  });

  it('does not post while the session refresh is still in flight', async () => {
    authState.isLoading = true;
    await loadLinkPage('?code=auth-code&state=state-hash');

    expect(linkGoogle).not.toHaveBeenCalled();
    expect(await screen.findByText(/Linking your Google account/i)).toBeInTheDocument();
  });

  it('asks for sign-in when the session could not be restored', async () => {
    authState.isAuthenticated = false;
    await loadLinkPage('?code=auth-code&state=state-hash');

    expect(await screen.findByText(/sign in/i)).toBeInTheDocument();
    expect(linkGoogle).not.toHaveBeenCalled();
  });

  it('surfaces a failed link instead of navigating', async () => {
    linkGoogle.mockRejectedValue(new Error('This Google account is already linked to another user'));

    await loadLinkPage('?code=auth-code&state=state-hash');

    expect(
      await screen.findByText(/already linked to another user/i),
    ).toBeInTheDocument();
    expect(navigate).not.toHaveBeenCalled();
  });
});
