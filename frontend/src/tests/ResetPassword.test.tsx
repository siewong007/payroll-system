import { MemoryRouter } from 'react-router';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { validateResetToken } from '@/api/admin';

const adminMocks = vi.hoisted(() => ({
  validateResetToken: vi.fn(),
  resetPassword: vi.fn(),
}));

vi.mock('@/api/admin', () => adminMocks);

describe('ResetPassword fragment token handling', () => {
  beforeEach(() => {
    adminMocks.validateResetToken.mockReset();
    adminMocks.resetPassword.mockReset();
  });

  it('prefers #token= hash fragment and scrubs it from window.location', async () => {
    adminMocks.validateResetToken.mockResolvedValue({ valid: true });
    window.history.replaceState(null, '', '/reset-password#token=frag-secret-123');
    const { ResetPassword } = await import('@/pages/auth/ResetPassword');

    render(
      <MemoryRouter initialEntries={['/reset-password#token=frag-secret-123']}>
        <ResetPassword />
      </MemoryRouter>,
    );

    await waitFor(() => expect(validateResetToken).toHaveBeenCalledWith('frag-secret-123'));
    expect(await screen.findByLabelText('New Password')).toBeInTheDocument();
    expect(window.location.hash).toBe('');
    expect(window.location.pathname).toBe('/reset-password');
  });

  it('falls back to legacy ?token= query parameter and scrubs it from window.location', async () => {
    adminMocks.validateResetToken.mockResolvedValue({ valid: true });
    window.history.replaceState(null, '', '/reset-password?token=legacy-secret-456');
    const { ResetPassword } = await import('@/pages/auth/ResetPassword');

    render(
      <MemoryRouter initialEntries={['/reset-password?token=legacy-secret-456']}>
        <ResetPassword />
      </MemoryRouter>,
    );

    await waitFor(() => expect(validateResetToken).toHaveBeenCalledWith('legacy-secret-456'));
    expect(await screen.findByLabelText('New Password')).toBeInTheDocument();
    expect(window.location.search).toBe('');
    expect(window.location.pathname).toBe('/reset-password');
  });
});
