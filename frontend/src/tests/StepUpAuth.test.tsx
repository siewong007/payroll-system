import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const totpMocks = vi.hoisted(() => ({
  totpStatus: vi.fn(),
  totpSetupBegin: vi.fn(),
  totpSetupConfirm: vi.fn(),
  totpDisable: vi.fn(),
  totpRegenerateBackupCodes: vi.fn(),
}));

const passkeyMocks = vi.hoisted(() => ({
  listPasskeys: vi.fn(),
  deletePasskey: vi.fn(),
  renamePasskey: vi.fn(),
  passkeyRegisterBegin: vi.fn(),
  passkeyRegisterComplete: vi.fn(),
}));

const webauthnMocks = vi.hoisted(() => ({
  isWebAuthnSupported: vi.fn(() => true),
  createPasskeyCredential: vi.fn(),
}));

vi.mock('@/api/totp', () => totpMocks);
vi.mock('@/api/passkey', () => passkeyMocks);
vi.mock('@/lib/webauthn', () => webauthnMocks);

import { TwoFactorSetup } from '@/components/TwoFactorSetup';
import { PasskeyManagement } from '@/components/PasskeyManagement';

function renderWithClient(ui: ReactNode) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

describe('Step-up password confirmation on credential enrollment', () => {
  beforeEach(() => {
    Object.values(totpMocks).forEach((fn) => fn.mockReset());
    Object.values(passkeyMocks).forEach((fn) => fn.mockReset());
    totpMocks.totpStatus.mockResolvedValue({ enabled: false });
    webauthnMocks.isWebAuthnSupported.mockReturnValue(true);
    webauthnMocks.createPasskeyCredential.mockReset();
  });

  it('requires current password alongside TOTP code before calling totpSetupConfirm', async () => {
    const typer = userEvent.setup();
    totpMocks.totpStatus.mockResolvedValue({ enabled: false });
    totpMocks.totpSetupBegin.mockResolvedValue({
      secret: 'JBSWY3DPEHPK3PXP',
      otpauth_url: 'otpauth://totp/PayrollMY:test',
      qr_code_base64: 'AA==',
    });
    totpMocks.totpSetupConfirm.mockResolvedValue({ backup_codes: ['ABCD-1234'] });

    renderWithClient(<TwoFactorSetup />);
    await typer.click(await screen.findByRole('button', { name: 'Enable 2FA' }));

    const confirmBtn = await screen.findByRole('button', { name: 'Confirm & Enable' });
    expect(confirmBtn).toBeDisabled();

    await typer.type(screen.getByPlaceholderText('123456'), '654321');
    expect(confirmBtn).toBeDisabled();

    await typer.type(screen.getByPlaceholderText('Current password'), 'MyStepUpPass1!');
    expect(confirmBtn).toBeEnabled();

    await typer.click(confirmBtn);
    await waitFor(() =>
      expect(totpMocks.totpSetupConfirm).toHaveBeenCalledWith('654321', 'MyStepUpPass1!'),
    );
  });

  it('requires current password before initiating passkey registration', async () => {
    const typer = userEvent.setup();
    passkeyMocks.listPasskeys.mockResolvedValue([]);
    passkeyMocks.passkeyRegisterBegin.mockResolvedValue({
      challenge_id: 'chal-1',
      options: { publicKey: {} },
    });
    webauthnMocks.createPasskeyCredential.mockResolvedValue({ id: 'cred-1' });
    passkeyMocks.passkeyRegisterComplete.mockResolvedValue({ message: 'ok' });

    renderWithClient(<PasskeyManagement />);
    await typer.click(await screen.findByRole('button', { name: 'Add a passkey' }));

    const registerBtn = await screen.findByRole('button', { name: 'Register' });
    expect(registerBtn).toBeDisabled();

    await typer.type(
      screen.getByPlaceholderText('Passkey name (e.g., MacBook, iPhone)'),
      'Work Laptop',
    );
    expect(registerBtn).toBeDisabled();

    await typer.type(screen.getByPlaceholderText('Current password'), 'MyStepUpPass1!');
    expect(registerBtn).toBeEnabled();

    await typer.click(registerBtn);
    await waitFor(() =>
      expect(passkeyMocks.passkeyRegisterBegin).toHaveBeenCalledWith('MyStepUpPass1!'),
    );
    await waitFor(() =>
      expect(passkeyMocks.passkeyRegisterComplete).toHaveBeenCalledWith(
        'chal-1',
        { id: 'cred-1' },
        'Work Laptop',
      ),
    );
  });

  it('requires password alongside 2FA code to disable 2FA or delete a passkey when 2FA is enabled', async () => {
    const typer = userEvent.setup();
    totpMocks.totpStatus.mockResolvedValue({ enabled: true });
    totpMocks.totpDisable.mockResolvedValue(undefined);
    passkeyMocks.listPasskeys.mockResolvedValue([
      {
        id: 'pk-1',
        credential_name: 'Work Laptop',
        created_at: '2026-01-01T00:00:00Z',
        last_used_at: null,
      },
    ]);
    passkeyMocks.deletePasskey.mockResolvedValue(undefined);

    renderWithClient(<TwoFactorSetup />);
    await typer.click(await screen.findByRole('button', { name: 'Disable 2FA' }));
    const confirmDisable = await screen.findByRole('button', { name: 'Confirm' });
    expect(confirmDisable).toBeDisabled();
    await typer.type(screen.getByPlaceholderText('Current password'), 'MyStepUpPass1!');
    expect(confirmDisable).toBeDisabled();
    await typer.type(screen.getByPlaceholderText('123456'), '654321');
    expect(confirmDisable).toBeEnabled();
    await typer.click(confirmDisable);
    await waitFor(() =>
      expect(totpMocks.totpDisable).toHaveBeenCalledWith('MyStepUpPass1!', '654321'),
    );

    renderWithClient(<PasskeyManagement />);
    await typer.click(await screen.findByRole('button', { name: 'Delete' }));
    const deleteSubmit = await screen.findByRole('button', { name: 'Delete passkey' });
    expect(deleteSubmit).toBeDisabled();
    await typer.type(screen.getAllByPlaceholderText('Current password').at(-1)!, 'MyStepUpPass1!');
    expect(deleteSubmit).toBeDisabled();
    await typer.type(screen.getAllByPlaceholderText('123456').at(-1)!, '654321');
    expect(deleteSubmit).toBeEnabled();
    await typer.click(deleteSubmit);
    await waitFor(() =>
      expect(passkeyMocks.deletePasskey).toHaveBeenCalledWith('pk-1', 'MyStepUpPass1!', '654321'),
    );
  });
});
