import { StrictMode } from 'react';
import { MemoryRouter } from 'react-router';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const navigate = vi.fn();
vi.mock('react-router', async () => {
  const actual = await vi.importActual<typeof import('react-router')>('react-router');
  return { ...actual, useNavigate: () => navigate };
});

const setSession = vi.fn();
vi.mock('@/context/AuthContext', () => ({
  useAuth: () => ({ setSession }),
}));

/**
 * Land on the callback URL with `fragment`, as a fresh page load.
 *
 * The component memoises the fragment at module scope — it is consumed once per
 * page load — so a new load has to re-import the module to be faithful.
 */
async function loadCallbackPage(fragment: string) {
  window.history.replaceState(null, '', `/oauth2/callback${fragment}`);
  vi.resetModules();
  const { OAuth2Callback } = await import('@/pages/auth/OAuth2Callback');

  const renderOnce = () =>
    render(
      <StrictMode>
        <MemoryRouter>
          <OAuth2Callback />
        </MemoryRouter>
      </StrictMode>,
    );

  return { renderOnce, ...renderOnce() };
}

describe('OAuth2Callback', () => {
  beforeEach(() => {
    navigate.mockReset();
    setSession.mockReset();
  });

  it('shows the reason the server sent instead of a generic failure', async () => {
    const message =
      'Your Google sign-in took too long or was already used. Please try signing in again.';
    await loadCallbackPage(`#error=${encodeURIComponent(message)}`);

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(screen.queryByText(/Missing authentication data/i)).not.toBeInTheDocument();
  });

  it('clears the single-use fragment from the URL', async () => {
    await loadCallbackPage('#error=something%20went%20wrong');

    await waitFor(() => expect(window.location.hash).toBe(''));
    expect(window.location.pathname).toBe('/oauth2/callback');
  });

  // StrictMode remounts in development, which resets hooks. A ref or state guard
  // therefore ran the effect again against an already-cleared hash and replaced
  // the real outcome with "Missing authentication data" — hiding both the
  // server's message and the 2FA prompt.
  it('survives a remount after the fragment has been consumed', async () => {
    const message = 'Google sign-in was cancelled. Please try again.';
    const { renderOnce, unmount } = await loadCallbackPage(
      `#error=${encodeURIComponent(message)}`,
    );

    expect(await screen.findByText(message)).toBeInTheDocument();
    unmount();

    // Remount within the same page load, hash already gone.
    renderOnce();
    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(screen.queryByText(/Missing authentication data/i)).not.toBeInTheDocument();
  });

  it('still reports missing data when the fragment carried nothing', async () => {
    await loadCallbackPage('');

    expect(await screen.findByText(/Missing authentication data/i)).toBeInTheDocument();
  });

  it('hands a 2FA-gated login to the prompt rather than the error card', async () => {
    await loadCallbackPage('#mfa_token=pending-token');

    expect(await screen.findByText(/Two-factor authentication/i)).toBeInTheDocument();
    expect(screen.queryByText(/Missing authentication data/i)).not.toBeInTheDocument();
  });
});
