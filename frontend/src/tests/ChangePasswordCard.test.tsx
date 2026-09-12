import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ChangePasswordCard } from '@/components/ChangePasswordCard';

const putMock = vi.fn();
const logoutMock = vi.fn();
const navigateMock = vi.fn();

vi.mock('@/api/client', () => ({ default: { put: (...a: unknown[]) => putMock(...a) } }));
vi.mock('@/context/AuthContext', () => ({ useAuth: () => ({ logout: logoutMock }) }));
vi.mock('react-router', () => ({ useNavigate: () => navigateMock }));

describe('ChangePasswordCard', () => {
  beforeEach(() => { putMock.mockReset(); logoutMock.mockReset(); navigateMock.mockReset(); });

  it('rejects mismatched confirmation without calling the API', async () => {
    const typer = userEvent.setup();
    render(<ChangePasswordCard />);
    await typer.type(screen.getByLabelText('Current password'), 'old-password-1');
    await typer.type(screen.getByLabelText('New password'), 'Newpass12345');
    await typer.type(screen.getByLabelText('Confirm new password'), 'Different123');
    await typer.click(screen.getByRole('button', { name: 'Change password' }));
    expect(await screen.findByText('New passwords do not match')).toBeInTheDocument();
    expect(putMock).not.toHaveBeenCalled();
  });

  it('rejects a weak new password client-side', async () => {
    const typer = userEvent.setup();
    render(<ChangePasswordCard />);
    await typer.type(screen.getByLabelText('Current password'), 'old-password-1');
    await typer.type(screen.getByLabelText('New password'), 'short');
    await typer.type(screen.getByLabelText('Confirm new password'), 'short');
    await typer.click(screen.getByRole('button', { name: 'Change password' }));
    expect(await screen.findByText(/at least 10 characters/)).toBeInTheDocument();
    expect(putMock).not.toHaveBeenCalled();
  });

  it('submits, then logs out and navigates to /login', async () => {
    const typer = userEvent.setup();
    putMock.mockResolvedValue({ data: {} });
    render(<ChangePasswordCard />);
    await typer.type(screen.getByLabelText('Current password'), 'old-password-1');
    await typer.type(screen.getByLabelText('New password'), 'Newpass12345');
    await typer.type(screen.getByLabelText('Confirm new password'), 'Newpass12345');
    await typer.click(screen.getByRole('button', { name: 'Change password' }));
    await waitFor(() => expect(putMock).toHaveBeenCalledWith('/auth/change-password', {
      current_password: 'old-password-1',
      new_password: 'Newpass12345',
    }));
    await waitFor(() => expect(logoutMock).toHaveBeenCalled());
    expect(navigateMock).toHaveBeenCalledWith('/login');
  });

  it('surfaces the server error', async () => {
    const typer = userEvent.setup();
    putMock.mockRejectedValue({ response: { data: { error: 'Current password is incorrect' } } });
    render(<ChangePasswordCard />);
    await typer.type(screen.getByLabelText('Current password'), 'wrong');
    await typer.type(screen.getByLabelText('New password'), 'Newpass12345');
    await typer.type(screen.getByLabelText('Confirm new password'), 'Newpass12345');
    await typer.click(screen.getByRole('button', { name: 'Change password' }));
    expect(await screen.findByText('Current password is incorrect')).toBeInTheDocument();
    expect(logoutMock).not.toHaveBeenCalled();
  });
});
